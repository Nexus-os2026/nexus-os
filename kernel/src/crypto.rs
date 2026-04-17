//! Encryption at rest for Nexus OS data stores.
//!
//! Uses AES-256-GCM with keys derived via Argon2id from a user-provided
//! password or master secret.  Keys are zeroized on drop to prevent
//! leaking sensitive material in memory.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
const ENCRYPTED_HEADER: &[u8] = b"NEXUS_ENC_V1";

/// Public access to the header length for other modules (e.g. backup).
pub const ENCRYPTED_HEADER_LEN: usize = 12; // b"NEXUS_ENC_V1".len()
/// Public access to the header bytes for other modules.
pub const ENCRYPTED_HEADER_BYTES: &[u8] = ENCRYPTED_HEADER;

// ── Error ──────────────────────────────────────────────────────────────

/// Errors from the crypto subsystem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("key source not available: {0}")]
    KeySourceUnavailable(String),
}

// ── EncryptionKey ──────────────────────────────────────────────────────

/// An AES-256 encryption key that is zeroized on drop.
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct EncryptionKey {
    key: [u8; 32],
}

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionKey")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

impl Clone for EncryptionKey {
    fn clone(&self) -> Self {
        Self { key: self.key }
    }
}

impl EncryptionKey {
    /// Derive an encryption key from a password and salt via Argon2id.
    ///
    /// Parameters: 64 MiB memory, 3 iterations, 4 lanes — OWASP-recommended
    /// minimums for Argon2id.
    pub fn derive(password: &[u8], salt: &[u8; SALT_LEN]) -> Result<Self, CryptoError> {
        let params = argon2::Params::new(65536, 3, 4, Some(32))
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
        let argon2 =
            argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password, salt, &mut key)
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
        Ok(Self { key })
    }

    /// Load encryption key from the `NEXUS_ENCRYPTION_KEY` environment variable.
    ///
    /// The env var is expected to contain a hex-encoded 32-byte key or a
    /// passphrase (hashed with SHA-256 to produce the key bytes).
    pub fn from_env() -> Result<Self, CryptoError> {
        let raw = std::env::var("NEXUS_ENCRYPTION_KEY").map_err(|_| {
            CryptoError::KeySourceUnavailable(
                "NEXUS_ENCRYPTION_KEY environment variable not set".into(),
            )
        })?;

        // If it looks like a 64-char hex string, decode as raw key bytes.
        if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut key = [0u8; 32];
            for (i, chunk) in raw.as_bytes().chunks(2).enumerate() {
                let byte_str = std::str::from_utf8(chunk)
                    .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
                key[i] = u8::from_str_radix(byte_str, 16)
                    .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
            }
            return Ok(Self { key });
        }

        // Otherwise treat it as a passphrase — SHA-256 hash to get 32 bytes.
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let digest = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        Ok(Self { key })
    }

    /// Load encryption key from a file (e.g. Docker/K8s secret mount).
    pub fn from_file(path: &Path) -> Result<Self, CryptoError> {
        let contents =
            std::fs::read(path).map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;

        if contents.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&contents);
            return Ok(Self { key });
        }

        // Treat file contents as a passphrase.
        let mut hasher = Sha256::new();
        hasher.update(&contents);
        let digest = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest);
        Ok(Self { key })
    }

    /// Load key using the configured source.
    pub fn from_config(config: &EncryptionConfig) -> Result<Self, CryptoError> {
        if !config.enabled {
            return Err(CryptoError::KeySourceUnavailable(
                "encryption_at_rest is disabled".into(),
            ));
        }
        match config.key_source.as_str() {
            "env" => Self::from_env(),
            "file" => {
                let path = config.key_file.as_deref().ok_or_else(|| {
                    CryptoError::KeySourceUnavailable("encryption_key_file not configured".into())
                })?;
                Self::from_file(Path::new(path))
            }
            other => Err(CryptoError::KeySourceUnavailable(format!(
                "unknown key_source: {other}"
            ))),
        }
    }

    /// Raw key bytes (for passing to AES-256-GCM).
    fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

// ── Encrypt / Decrypt helpers ──────────────────────────────────────────

/// Encrypt arbitrary data with AES-256-GCM.
///
/// Output format: `NEXUS_ENC_V1 || nonce(12) || ciphertext`.
pub fn encrypt_data(key: &EncryptionKey, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| CryptoError::Encryption(e.to_string()))?;

    let mut out = Vec::with_capacity(ENCRYPTED_HEADER.len() + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(ENCRYPTED_HEADER);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by [`encrypt_data`].
pub fn decrypt_data(key: &EncryptionKey, blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let header_len = ENCRYPTED_HEADER.len();
    let min_len = header_len + NONCE_LEN + 1;
    if blob.len() < min_len {
        return Err(CryptoError::InvalidData("ciphertext too short".into()));
    }
    if &blob[..header_len] != ENCRYPTED_HEADER {
        return Err(CryptoError::InvalidData(
            "missing NEXUS_ENC_V1 header — not encrypted or wrong format".into(),
        ));
    }

    let nonce_array: [u8; NONCE_LEN] = blob[header_len..header_len + NONCE_LEN]
        .try_into()
        .map_err(|_| CryptoError::InvalidData("nonce length mismatch".into()))?;
    let nonce = Nonce::from(nonce_array);
    let ciphertext = &blob[header_len + NONCE_LEN..];

    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| CryptoError::Decryption(e.to_string()))?;

    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| CryptoError::Decryption("decryption failed (wrong key or corrupted)".into()))
}

/// Encrypt a file on disk in-place.
pub fn encrypt_file(key: &EncryptionKey, path: &Path) -> Result<(), CryptoError> {
    let plaintext =
        std::fs::read(path).map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;

    // Skip if already encrypted.
    if plaintext.len() >= ENCRYPTED_HEADER.len()
        && &plaintext[..ENCRYPTED_HEADER.len()] == ENCRYPTED_HEADER
    {
        return Ok(());
    }

    let encrypted = encrypt_data(key, &plaintext)?;
    std::fs::write(path, &encrypted)
        .map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

/// Decrypt a file on disk in-place.
pub fn decrypt_file(key: &EncryptionKey, path: &Path) -> Result<(), CryptoError> {
    let blob =
        std::fs::read(path).map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;

    // Skip if not encrypted.
    if blob.len() < ENCRYPTED_HEADER.len() || &blob[..ENCRYPTED_HEADER.len()] != ENCRYPTED_HEADER {
        return Ok(());
    }

    let plaintext = decrypt_data(key, &blob)?;
    std::fs::write(path, &plaintext)
        .map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

/// Generate a random 16-byte salt suitable for Argon2id.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Re-encrypt all database files under `data_dir` with a new key.
///
/// 1. Reads each `.db` file with `old_key`.
/// 2. Writes to a `.db.new` temp file with `new_key`.
/// 3. Verifies the new file can be decrypted.
/// 4. Atomically replaces the old file.
pub fn rotate_encryption_key(
    old_key: &EncryptionKey,
    new_key: &EncryptionKey,
    data_dir: &Path,
) -> Result<Vec<PathBuf>, CryptoError> {
    let mut rotated = Vec::new();

    let entries = std::fs::read_dir(data_dir)
        .map_err(|e| CryptoError::Io(format!("{}: {e}", data_dir.display())))?;

    for entry in entries {
        let entry = entry.map_err(|e| CryptoError::Io(e.to_string()))?;
        let path = entry.path();

        let is_target = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "db" || ext == "sqlite")
            .unwrap_or(false);

        if !is_target || !path.is_file() {
            continue;
        }

        let blob = std::fs::read(&path)
            .map_err(|e| CryptoError::Io(format!("{}: {e}", path.display())))?;

        // Decrypt with old key (skip if not encrypted).
        let plaintext = if blob.len() >= ENCRYPTED_HEADER.len()
            && &blob[..ENCRYPTED_HEADER.len()] == ENCRYPTED_HEADER
        {
            decrypt_data(old_key, &blob)?
        } else {
            blob
        };

        // Re-encrypt with new key.
        let new_blob = encrypt_data(new_key, &plaintext)?;

        // Write to temp, verify, then replace.
        let tmp_path = path.with_extension("db.rotating");
        std::fs::write(&tmp_path, &new_blob)
            .map_err(|e| CryptoError::Io(format!("{}: {e}", tmp_path.display())))?;

        // Verify round-trip.
        let verify_blob = std::fs::read(&tmp_path)
            .map_err(|e| CryptoError::Io(format!("{}: {e}", tmp_path.display())))?;
        // Best-effort: verify re-encryption round-trip; discard plaintext, only check decryptability
        let _ = decrypt_data(new_key, &verify_blob)?;

        std::fs::rename(&tmp_path, &path).map_err(|e| CryptoError::Io(format!("rename: {e}")))?;

        rotated.push(path);
    }

    Ok(rotated)
}

// ── Configuration ──────────────────────────────────────────────────────

/// Encryption-at-rest configuration (embedded in `NexusConfig`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EncryptionConfig {
    #[serde(default)]
    pub enabled: bool,

    /// `"env"` or `"file"`.
    #[serde(default = "default_key_source")]
    pub key_source: String,

    /// Environment variable name (default `NEXUS_ENCRYPTION_KEY`).
    #[serde(default = "default_key_env")]
    pub key_env: String,

    /// Path to key file (for `key_source = "file"`).
    #[serde(default)]
    pub key_file: Option<String>,
}

fn default_key_source() -> String {
    "env".to_string()
}

fn default_key_env() -> String {
    "NEXUS_ENCRYPTION_KEY".to_string()
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            key_source: default_key_source(),
            key_env: default_key_env(),
            key_file: None,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_KEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let salt = generate_salt();
        let key = EncryptionKey::derive(b"test-password-123", &salt).unwrap();
        let plaintext = b"sensitive agent data that must be protected";

        let encrypted = encrypt_data(&key, plaintext).unwrap();
        assert_ne!(encrypted.as_slice(), plaintext.as_slice());
        assert!(encrypted.starts_with(ENCRYPTED_HEADER));

        let decrypted = decrypt_data(&key, &encrypted).unwrap();
        assert_eq!(decrypted.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let salt = generate_salt();
        let key_a = EncryptionKey::derive(b"password-a", &salt).unwrap();
        let key_b = EncryptionKey::derive(b"password-b", &salt).unwrap();

        let encrypted = encrypt_data(&key_a, b"secret").unwrap();
        let result = decrypt_data(&key_b, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn key_derivation_is_deterministic() {
        let salt = [42u8; SALT_LEN];
        let key_a = EncryptionKey::derive(b"same-password", &salt).unwrap();
        let key_b = EncryptionKey::derive(b"same-password", &salt).unwrap();
        assert_eq!(key_a.key, key_b.key);
    }

    #[test]
    fn different_salts_produce_different_keys() {
        let salt_a = [1u8; SALT_LEN];
        let salt_b = [2u8; SALT_LEN];
        let key_a = EncryptionKey::derive(b"password", &salt_a).unwrap();
        let key_b = EncryptionKey::derive(b"password", &salt_b).unwrap();
        assert_ne!(key_a.key, key_b.key);
    }

    #[test]
    fn encrypt_decrypt_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.db");
        let original = b"database contents here";
        std::fs::write(&file_path, original).unwrap();

        let salt = generate_salt();
        let key = EncryptionKey::derive(b"file-password", &salt).unwrap();

        encrypt_file(&key, &file_path).unwrap();
        let on_disk = std::fs::read(&file_path).unwrap();
        assert_ne!(on_disk.as_slice(), original.as_slice());

        decrypt_file(&key, &file_path).unwrap();
        let restored = std::fs::read(&file_path).unwrap();
        assert_eq!(restored.as_slice(), original.as_slice());
    }

    #[test]
    fn encrypt_file_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.db");
        std::fs::write(&file_path, b"data").unwrap();

        let salt = generate_salt();
        let key = EncryptionKey::derive(b"password", &salt).unwrap();

        encrypt_file(&key, &file_path).unwrap();
        let first = std::fs::read(&file_path).unwrap();

        // Encrypting again should be a no-op (already encrypted).
        encrypt_file(&key, &file_path).unwrap();
        let second = std::fs::read(&file_path).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn key_rotation_works() {
        let dir = tempfile::tempdir().unwrap();

        // Create two "database" files.
        let db1 = dir.path().join("agents.db");
        let db2 = dir.path().join("audit.db");
        std::fs::write(&db1, b"agents data").unwrap();
        std::fs::write(&db2, b"audit data").unwrap();

        let salt = generate_salt();
        let old_key = EncryptionKey::derive(b"old-master", &salt).unwrap();

        // Encrypt with old key.
        encrypt_file(&old_key, &db1).unwrap();
        encrypt_file(&old_key, &db2).unwrap();

        // Rotate to new key.
        let new_key = EncryptionKey::derive(b"new-master", &salt).unwrap();
        let rotated = rotate_encryption_key(&old_key, &new_key, dir.path()).unwrap();
        assert_eq!(rotated.len(), 2);

        // Old key should no longer work.
        let blob = std::fs::read(&db1).unwrap();
        assert!(decrypt_data(&old_key, &blob).is_err());

        // New key should work.
        let plaintext = decrypt_data(&new_key, &blob).unwrap();
        assert_eq!(plaintext.as_slice(), b"agents data");
    }

    #[test]
    fn from_env_hex_key() {
        let _guard = ENV_KEY_LOCK.lock().unwrap();
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        std::env::set_var("NEXUS_ENCRYPTION_KEY", hex);
        let key = EncryptionKey::from_env().unwrap();
        assert_eq!(key.key[0], 0x01);
        assert_eq!(key.key[15], 0xef);
        std::env::remove_var("NEXUS_ENCRYPTION_KEY");
    }

    #[test]
    fn from_env_passphrase() {
        let _guard = ENV_KEY_LOCK.lock().unwrap();
        std::env::set_var("NEXUS_ENCRYPTION_KEY", "my-strong-passphrase");
        let key = EncryptionKey::from_env().unwrap();
        assert_eq!(key.key.len(), 32);
        std::env::remove_var("NEXUS_ENCRYPTION_KEY");
    }

    #[test]
    fn truncated_ciphertext_rejected() {
        let salt = generate_salt();
        let key = EncryptionKey::derive(b"password", &salt).unwrap();
        let encrypted = encrypt_data(&key, b"data").unwrap();

        // Truncate.
        let truncated = &encrypted[..ENCRYPTED_HEADER.len() + 5];
        assert!(decrypt_data(&key, truncated).is_err());
    }

    #[test]
    fn invalid_header_rejected() {
        let salt = generate_salt();
        let key = EncryptionKey::derive(b"password", &salt).unwrap();

        let garbage = b"NOT_NEXUS_HEADER_plus_some_more_data_here_to_pass_length";
        assert!(decrypt_data(&key, garbage).is_err());
    }
}
