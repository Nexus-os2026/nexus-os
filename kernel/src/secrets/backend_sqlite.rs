//! SqliteEnvelope backend.
//!
//! Persists encrypted credentials in `nexus_persistence`'s `secrets`
//! table (added in Commit 1). AES-256-GCM under a 32-byte domain key
//! derived via HKDF-SHA-256 from the kernel `EncryptionKey`, with
//! domain separator `nexus.secrets.v1`. Plaintext bytes never enter
//! the row format; only `(nonce, ciphertext)` are stored.
//!
//! M0-FIX F4 row format: every ciphertext is bound to its secret
//! identity. The `nonce` column holds one format byte (`0x02`) followed
//! by the 12-byte random nonce, and the AES-GCM associated data is a
//! domain label, the format byte and the length-prefixed scope and name
//! (see `bound_aad`). A ciphertext copied into another row fails
//! authentication. Rows written before F4 hold exactly the 12-byte nonce
//! and no associated data; the length tells the two formats apart
//! without any trial decryption. The read path refuses a legacy row
//! with `SecretError::LegacyUnbound`; only the one-time legacy upgrade
//! (`migrate::plan_legacy_upgrade`), run by startup before anything reads
//! the vault, decrypts it, to re-encrypt it under its own identity.
//! A vault that already holds a bound row is never upgraded again.
//!
//! Master-key sourcing is delegated entirely to
//! `crate::crypto::EncryptionKey::from_config` — env primary, file
//! fallback. OS keyring is intentionally NOT a master-key source
//! (circular bootstrap; see ADR 0004 Decision section).

use super::{ResolvedFrom, SecretBackend, SecretError};
use crate::crypto::EncryptionKey;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use std::sync::Arc;
use zeroize::{Zeroize, Zeroizing};

const DOMAIN_INFO: &[u8] = b"nexus.secrets.v1";
const NONCE_LEN: usize = 12;
/// Format byte that prefixes the nonce of an identity-bound row.
const FORMAT_BOUND_V2: u8 = 0x02;
/// Domain label at the start of the associated data of a bound row.
const AAD_LABEL: &[u8] = b"nexus.secrets.row";

/// The stored format of a row, decided by the `nonce` column alone.
enum RowFormat {
    /// Pre-F4: exactly the 12-byte nonce, no associated data.
    Legacy([u8; NONCE_LEN]),
    /// Format byte `0x02` + 12-byte nonce, identity-bound associated data.
    BoundV2([u8; NONCE_LEN]),
}

fn row_format(stored: &[u8]) -> Result<RowFormat, SecretError> {
    let mut nonce = [0u8; NONCE_LEN];
    if stored.len() == NONCE_LEN {
        nonce.copy_from_slice(stored);
        Ok(RowFormat::Legacy(nonce))
    } else if stored.len() == NONCE_LEN + 1 && stored[0] == FORMAT_BOUND_V2 {
        nonce.copy_from_slice(&stored[1..]);
        Ok(RowFormat::BoundV2(nonce))
    } else {
        Err(SecretError::Crypto("unknown secret row format".into()))
    }
}

/// Associated data that binds a ciphertext to `(scope, name)`: the domain
/// label, a NUL, the format byte, then the scope and the name, each
/// prefixed with its byte length (u64 big-endian) so no two identities
/// produce the same bytes.
fn bound_aad(scope: &str, name: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_LABEL.len() + 18 + scope.len() + name.len());
    aad.extend_from_slice(AAD_LABEL);
    aad.push(0);
    aad.push(FORMAT_BOUND_V2);
    for part in [scope, name] {
        aad.extend_from_slice(&(part.len() as u64).to_be_bytes());
        aad.extend_from_slice(part.as_bytes());
    }
    aad
}

pub struct SqliteEnvelopeBackend {
    db: Arc<nexus_persistence::NexusDatabase>,
    /// HKDF-derived domain key. `Zeroize::zeroize` runs on drop via
    /// the manual Drop impl below.
    domain_key: [u8; 32],
}

impl SqliteEnvelopeBackend {
    pub fn new(db: Arc<nexus_persistence::NexusDatabase>, master: &EncryptionKey) -> Self {
        let domain_key = master.derive_subkey(DOMAIN_INFO);
        Self { db, domain_key }
    }

    fn cipher(&self) -> Result<Aes256Gcm, SecretError> {
        Aes256Gcm::new_from_slice(&self.domain_key)
            .map_err(|e| SecretError::Crypto(format!("aes init: {e}")))
    }

    /// Bug AK Commit 2: domain-key access for the migration
    /// helper. Returns the underlying `NexusDatabase` so
    /// migrate_config_to_vault can call
    /// `db.migrate_credential_vault_v1` directly, preserving
    /// Refinement B's transactional contract.
    pub(crate) fn db(&self) -> &Arc<nexus_persistence::NexusDatabase> {
        &self.db
    }

    /// Encrypt `plaintext` bound to `(scope, name)` under a fresh random
    /// nonce. Returns the stored `nonce` column (format byte + nonce) and
    /// the ciphertext.
    fn seal(
        &self,
        scope: &str,
        name: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), SecretError> {
        let cipher = self.cipher()?;
        let mut nonce_arr = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_arr);
        let aad = bound_aad(scope, name);
        let ciphertext = cipher
            .encrypt(
                &Nonce::from(nonce_arr),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| SecretError::EncryptionFailed)?;
        let mut stored = Vec::with_capacity(NONCE_LEN + 1);
        stored.push(FORMAT_BOUND_V2);
        stored.extend_from_slice(&nonce_arr);
        Ok((stored, ciphertext))
    }

    /// Bug AK Commit 2: encrypt-only path for migration. Pre-
    /// encrypts a plaintext value with the same domain key and
    /// identity binding `set` would use, returning the stored
    /// `(nonce, ciphertext)` for caller-controlled atomic writes.
    pub(crate) fn encrypt_for_migration(
        &self,
        scope: &str,
        name: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), SecretError> {
        self.seal(scope, name, plaintext)
    }

    /// M0-FIX F4: whether a stored `nonce` column is a legacy, unbound row.
    pub(crate) fn is_legacy_row(stored_nonce: &[u8]) -> bool {
        matches!(row_format(stored_nonce), Ok(RowFormat::Legacy(_)))
    }

    /// M0-FIX F4: whether a stored `nonce` column is an identity-bound row.
    pub(crate) fn is_bound_row(stored_nonce: &[u8]) -> bool {
        matches!(row_format(stored_nonce), Ok(RowFormat::BoundV2(_)))
    }

    /// M0-FIX F4: the stored `nonce` column length and prefix of a bound
    /// row, for the upgrade's in-transaction "no bound row yet" check.
    pub(crate) fn bound_row_shape() -> (usize, &'static [u8]) {
        (NONCE_LEN + 1, &[FORMAT_BOUND_V2])
    }

    /// M0-FIX F4: decrypt a legacy (unbound) row. Called only by the
    /// one-time legacy upgrade (`migrate::plan_legacy_upgrade`), never by
    /// `get`.
    /// Anything that is not a legacy row is refused.
    pub(crate) fn decrypt_legacy_for_upgrade(
        &self,
        stored_nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, SecretError> {
        let RowFormat::Legacy(nonce) = row_format(stored_nonce)? else {
            return Err(SecretError::Crypto("not a legacy secret row".into()));
        };
        let plaintext = self
            .cipher()?
            .decrypt(&Nonce::from(nonce), ciphertext)
            .map_err(|_| SecretError::DecryptionFailed)?;
        Ok(Zeroizing::new(plaintext))
    }
}

impl Drop for SqliteEnvelopeBackend {
    fn drop(&mut self) {
        self.domain_key.zeroize();
    }
}

impl SecretBackend for SqliteEnvelopeBackend {
    fn id(&self) -> ResolvedFrom {
        ResolvedFrom::Sqlite
    }

    fn get(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretError> {
        let row = self
            .db
            .lookup_secret(scope, name)
            .map_err(|e| SecretError::Storage(format!("{e}")))?;
        let Some((stored_nonce, ciphertext)) = row else {
            return Err(SecretError::NotFound);
        };
        let nonce = match row_format(&stored_nonce)? {
            // Never decrypted here: a legacy row carries no identity
            // binding, so it could be another secret's ciphertext.
            RowFormat::Legacy(_) => return Err(SecretError::LegacyUnbound),
            RowFormat::BoundV2(nonce) => nonce,
        };
        let aad = bound_aad(scope, name);
        let plaintext = self
            .cipher()?
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: ciphertext.as_ref(),
                    aad: &aad,
                },
            )
            .map_err(|_| SecretError::DecryptionFailed)?;
        let s = String::from_utf8(plaintext).map_err(|e| {
            e.into_bytes().zeroize();
            SecretError::Crypto("ciphertext is not utf-8".into())
        })?;
        Ok(Zeroizing::new(s))
    }

    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError> {
        let (stored_nonce, ciphertext) = self.seal(scope, name, value.as_bytes())?;
        self.db
            .record_secret(scope, name, &stored_nonce, &ciphertext)
            .map_err(|e| SecretError::Storage(format!("{e}")))
    }

    fn delete(&self, scope: &str, name: &str) -> Result<(), SecretError> {
        self.db
            .delete_secret(scope, name)
            .map_err(|e| SecretError::Storage(format!("{e}")))
    }

    fn list(&self, scope: &str) -> Result<Vec<String>, SecretError> {
        self.db
            .list_secrets(scope)
            .map_err(|e| SecretError::Storage(format!("{e}")))
    }
}
