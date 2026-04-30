//! SqliteEnvelope backend.
//!
//! Persists encrypted credentials in `nexus_persistence`'s `secrets`
//! table (added in Commit 1). AES-256-GCM under a 32-byte domain key
//! derived via HKDF-SHA-256 from the kernel `EncryptionKey`, with
//! domain separator `nexus.secrets.v1`. Plaintext bytes never enter
//! the row format; only `(nonce, ciphertext)` are stored.
//!
//! Master-key sourcing is delegated entirely to
//! `crate::crypto::EncryptionKey::from_config` — env primary, file
//! fallback. OS keyring is intentionally NOT a master-key source
//! (circular bootstrap; see ADR 0004 Decision section).

use super::{ResolvedFrom, SecretBackend, SecretError};
use crate::crypto::EncryptionKey;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use std::sync::Arc;
use zeroize::{Zeroize, Zeroizing};

const DOMAIN_INFO: &[u8] = b"nexus.secrets.v1";
const NONCE_LEN: usize = 12;

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

    /// Bug AK Commit 2: encrypt-only path for migration. Pre-
    /// encrypts a plaintext value with the same domain key
    /// `set` would use, returning `(nonce, ciphertext)` for
    /// caller-controlled atomic writes.
    pub(crate) fn encrypt_for_migration(
        &self,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), SecretError> {
        let cipher = self.cipher()?;
        let mut nonce_arr = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_arr);
        let ciphertext = cipher
            .encrypt(&Nonce::from(nonce_arr), plaintext)
            .map_err(|_| SecretError::EncryptionFailed)?;
        Ok((nonce_arr.to_vec(), ciphertext))
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
        let Some((nonce_bytes, ciphertext)) = row else {
            return Err(SecretError::NotFound);
        };
        if nonce_bytes.len() != NONCE_LEN {
            return Err(SecretError::Crypto("nonce length mismatch".into()));
        }
        let mut nonce_arr = [0u8; NONCE_LEN];
        nonce_arr.copy_from_slice(&nonce_bytes);
        let nonce = Nonce::from(nonce_arr);
        let cipher = self.cipher()?;
        let plaintext = cipher
            .decrypt(&nonce, ciphertext.as_ref())
            .map_err(|_| SecretError::DecryptionFailed)?;
        let s = String::from_utf8(plaintext)
            .map_err(|_| SecretError::Crypto("ciphertext is not utf-8".into()))?;
        Ok(Zeroizing::new(s))
    }

    fn set(&self, scope: &str, name: &str, value: Zeroizing<String>) -> Result<(), SecretError> {
        let cipher = self.cipher()?;
        let mut nonce_arr = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_arr);
        let ciphertext = cipher
            .encrypt(&Nonce::from(nonce_arr), value.as_bytes())
            .map_err(|_| SecretError::EncryptionFailed)?;
        self.db
            .record_secret(scope, name, &nonce_arr, &ciphertext)
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
