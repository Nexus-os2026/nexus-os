//! Encryption at rest for Nexus OS data stores.
//!
//! AES-256-GCM. A key comes from one of these sources:
//! - [`EncryptionKey::derive`]: Argon2id over a password and a salt.
//! - `NEXUS_ENCRYPTION_KEY` ([`EncryptionKey::from_env`]): 64 hexadecimal
//!   characters are the raw 256-bit key; any other value is hashed once with
//!   SHA-256.
//! - A vault key file ([`EncryptionKey::from_file`]): exactly 32 bytes are the
//!   raw key; any other contents are hashed once with SHA-256.
//!
//! The environment and file sources apply no salt and no stretching, and
//! Nexus measures no key strength: it refuses only an empty or
//! whitespace-only value (Final Gate item A), so a short or guessable value
//! gives a correspondingly weak key. Keys are zeroized on drop.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};

const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
const ENCRYPTED_HEADER: &[u8] = b"NEXUS_ENC_V1";
/// The only environment variable the vault key is read from.
const DEFAULT_KEY_ENV: &str = "NEXUS_ENCRYPTION_KEY";
/// Largest vault key file accepted, in bytes (Final Gate item E).
pub const MAX_KEY_FILE_BYTES: u64 = 4096;

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
    /// 64 hexadecimal characters are the raw 256-bit key; any other value is
    /// hashed once with SHA-256. An unset, empty or whitespace-only value is
    /// refused (Final Gate item A): it would give a constant key.
    pub fn from_env() -> Result<Self, CryptoError> {
        let raw = Zeroizing::new(std::env::var(DEFAULT_KEY_ENV).map_err(|_| {
            CryptoError::KeySourceUnavailable(
                "NEXUS_ENCRYPTION_KEY environment variable not set".into(),
            )
        })?);
        Self::from_env_value(&raw)
    }

    /// [`EncryptionKey::from_env`] for a value already read.
    fn from_env_value(raw: &str) -> Result<Self, CryptoError> {
        if raw.trim().is_empty() {
            return Err(CryptoError::KeySourceUnavailable(
                "NEXUS_ENCRYPTION_KEY is empty".into(),
            ));
        }

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

    /// Load the vault key from an operator-controlled key file (Final Gate
    /// item E).
    ///
    /// The file is opened once. The checks apply to the file actually opened
    /// and read, never to a path resolved again:
    /// - the last path component must not be a symbolic link;
    /// - it must be a regular file owned by the user running Nexus OS, with
    ///   no access for group or others;
    /// - it must hold 1 to [`MAX_KEY_FILE_BYTES`] bytes, not all whitespace,
    ///   and must not change while it is read.
    ///
    /// Exactly 32 bytes are the raw key; any other contents are hashed once
    /// with SHA-256, as before. Intermediate directories, their permissions
    /// and macOS extended ACLs are not examined. Key files are supported on
    /// Linux and macOS only: elsewhere, where ownership and access cannot be
    /// checked this way, the source is refused.
    ///
    /// A failure names a bounded reason, never the key file's location or
    /// contents.
    pub fn from_file(path: &Path) -> Result<Self, CryptoError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let file = key_file::open(path).map_err(KeyFileRejection::into_error)?;
            Self::from_opened_key_file(&file)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = path;
            Err(KeyFileRejection::UnsupportedPlatform.into_error())
        }
    }

    /// [`EncryptionKey::from_file`] for a key file already opened by
    /// `key_file::open`: checks, reads and derives from that same file.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn from_opened_key_file(file: &std::fs::File) -> Result<Self, CryptoError> {
        let contents = key_file::read(file).map_err(KeyFileRejection::into_error)?;
        key_from_file_contents(&contents).map_err(KeyFileRejection::into_error)
    }

    /// Load key using the configured source.
    pub fn from_config(config: &EncryptionConfig) -> Result<Self, CryptoError> {
        if !config.enabled {
            return Err(CryptoError::KeySourceUnavailable(
                "encryption_at_rest is disabled".into(),
            ));
        }
        match config.key_source.as_str() {
            "env" => {
                // Final Gate item A: the variable read is the variable
                // configured. Another name is refused rather than silently
                // replaced by NEXUS_ENCRYPTION_KEY.
                if config.key_env != DEFAULT_KEY_ENV {
                    return Err(CryptoError::KeySourceUnavailable(
                        "key_env must be NEXUS_ENCRYPTION_KEY".into(),
                    ));
                }
                Self::from_env()
            }
            "file" => {
                let path = config.key_file.as_deref().ok_or_else(|| {
                    CryptoError::KeySourceUnavailable("encryption_key_file not configured".into())
                })?;
                // P0-002C5B: an operator-configured absolute path only; a
                // relative one would resolve against the working directory.
                if !Path::new(path).is_absolute() {
                    return Err(CryptoError::KeySourceUnavailable(
                        "encryption_key_file must be an absolute path".into(),
                    ));
                }
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

    /// Test helper: build an `EncryptionKey` from a fixed 32-byte
    /// array. Crate-private — tests in sibling modules
    /// (e.g. `kernel/src/secrets/tests.rs`) need a deterministic
    /// master key without depending on env vars or filesystem.
    #[cfg(test)]
    pub(crate) fn from_raw_for_test(bytes: [u8; 32]) -> Self {
        Self { key: bytes }
    }

    /// Bug AK: derive a 32-byte domain subkey from the master key
    /// using HKDF-SHA-256. `info` is the domain separator
    /// (e.g. `b"nexus.secrets.v1"`). The master bytes never leave
    /// `EncryptionKey`; the derived key is returned as a fresh
    /// array. Caller should treat the returned key as sensitive
    /// (wrap in `Zeroizing` if held).
    pub fn derive_subkey(&self, info: &[u8]) -> [u8; 32] {
        // No salt — domain separator alone is sufficient for
        // domain isolation under a single master key.
        let hk = hkdf::Hkdf::<Sha256>::new(None, &self.key);
        let mut out = [0u8; 32];
        hk.expand(info, &mut out)
            .expect("32 bytes < 255 * HashLen for SHA-256");
        out
    }
}

// ── Vault key file (Final Gate item E) ─────────────────────────────────

/// Why a vault key file was refused. The reasons carry no path and no
/// contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFileRejection {
    /// It could not be opened or read.
    Unavailable,
    /// The last path component is a symbolic link.
    Redirected,
    /// It is a directory, a FIFO, a device or another non-regular file.
    NotRegularFile,
    /// It is owned by another user.
    ForeignOwner,
    /// Group or other users have some access to it.
    AccessibleToOthers,
    /// It is empty or holds only whitespace.
    Empty,
    /// It is larger than [`MAX_KEY_FILE_BYTES`].
    TooLarge,
    /// It changed while it was read.
    Changed,
    /// This platform does not support key files.
    UnsupportedPlatform,
}

impl KeyFileRejection {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Unavailable => "the key file could not be opened or read",
            Self::Redirected => "the key file is a symbolic link",
            Self::NotRegularFile => "the key file is not a regular file",
            Self::ForeignOwner => "the key file is not owned by the user running Nexus OS",
            Self::AccessibleToOthers => {
                "the key file is accessible to group or other users (use mode 0600 or 0400)"
            }
            Self::Empty => "the key file is empty or holds only whitespace",
            Self::TooLarge => "the key file is larger than 4096 bytes",
            Self::Changed => "the key file changed while it was read",
            Self::UnsupportedPlatform => {
                "key files are supported on Linux and macOS only; use key_source = \"env\""
            }
        }
    }

    fn into_error(self) -> CryptoError {
        CryptoError::KeySourceUnavailable(format!(
            "encryption key file rejected: {}",
            self.reason()
        ))
    }
}

/// The key a key file's contents give: exactly 32 bytes are the raw key, any
/// other contents are hashed once with SHA-256. Contents that are empty or
/// all whitespace are refused.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn key_from_file_contents(contents: &[u8]) -> Result<EncryptionKey, KeyFileRejection> {
    if contents.iter().all(u8::is_ascii_whitespace) {
        return Err(KeyFileRejection::Empty);
    }
    let mut key = [0u8; 32];
    if contents.len() == 32 {
        key.copy_from_slice(contents);
    } else {
        key.copy_from_slice(&Sha256::digest(contents));
    }
    Ok(EncryptionKey { key })
}

/// The checks on the opened file's own metadata: a regular file owned by
/// `euid`, with no access for group or others, of 1 to
/// [`MAX_KEY_FILE_BYTES`] bytes.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn check_opened_key_file(
    regular: bool,
    owner: u32,
    euid: u32,
    mode: u32,
    len: u64,
) -> Result<(), KeyFileRejection> {
    if !regular {
        return Err(KeyFileRejection::NotRegularFile);
    }
    if owner != euid {
        return Err(KeyFileRejection::ForeignOwner);
    }
    if mode & 0o077 != 0 {
        return Err(KeyFileRejection::AccessibleToOthers);
    }
    if len == 0 {
        return Err(KeyFileRejection::Empty);
    }
    if len > MAX_KEY_FILE_BYTES {
        return Err(KeyFileRejection::TooLarge);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod key_file {
    use super::{check_opened_key_file, KeyFileRejection, MAX_KEY_FILE_BYTES};
    use std::fs::{File, Metadata, OpenOptions};
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;
    use zeroize::Zeroizing;

    /// Opens the key file once. A symbolic link as the last component is
    /// refused (`O_NOFOLLOW`), and opening a FIFO or a device never blocks or
    /// acquires a terminal (`O_NONBLOCK`, `O_NOCTTY`); its type is refused
    /// from the opened descriptor.
    pub(super) fn open(path: &Path) -> Result<File, KeyFileRejection> {
        OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK | nix::libc::O_NOCTTY)
            .open(path)
            .map_err(|error| {
                if error.raw_os_error() == Some(nix::libc::ELOOP) {
                    KeyFileRejection::Redirected
                } else {
                    KeyFileRejection::Unavailable
                }
            })
    }

    /// The contents of the opened file, after checking that same file. The
    /// read is bounded, and the file must be unchanged afterwards.
    pub(super) fn read(file: &File) -> Result<Zeroizing<Vec<u8>>, KeyFileRejection> {
        let before = file.metadata().map_err(|_| KeyFileRejection::Unavailable)?;
        // SAFETY: geteuid has no preconditions and cannot fail.
        let euid = unsafe { nix::libc::geteuid() };
        check_opened_key_file(
            before.file_type().is_file(),
            before.uid(),
            euid,
            before.mode(),
            before.len(),
        )?;
        let mut contents = Zeroizing::new(Vec::with_capacity(MAX_KEY_FILE_BYTES as usize + 1));
        file.take(MAX_KEY_FILE_BYTES + 1)
            .read_to_end(&mut contents)
            .map_err(|_| KeyFileRejection::Unavailable)?;
        let after = file.metadata().map_err(|_| KeyFileRejection::Unavailable)?;
        if contents.len() as u64 != before.len() || !unchanged(&before, &after) {
            return Err(KeyFileRejection::Changed);
        }
        Ok(contents)
    }

    /// Whether the file's identity, size, contents time, mode and owner are
    /// the same in both snapshots.
    pub(super) fn unchanged(before: &Metadata, after: &Metadata) -> bool {
        let state = |m: &Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.mode(),
                m.uid(),
            )
        };
        state(before) == state(after)
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
pub(crate) mod tests {
    use super::*;

    /// Serializes tests that set or remove NEXUS_ENCRYPTION_KEY.
    pub(crate) static ENV_KEY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        let _guard = ENV_KEY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let _key = crate::secrets::tests::EnvVarGuard::set("NEXUS_ENCRYPTION_KEY", hex);
        let key = EncryptionKey::from_env().unwrap();
        assert_eq!(key.key[0], 0x01);
        assert_eq!(key.key[15], 0xef);
    }

    #[test]
    fn from_env_passphrase() {
        let _guard = ENV_KEY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let _key =
            crate::secrets::tests::EnvVarGuard::set("NEXUS_ENCRYPTION_KEY", "my-strong-passphrase");
        let key = EncryptionKey::from_env().unwrap();
        assert_eq!(key.key.len(), 32);
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

    #[test]
    fn p0_002c5b_key_files_are_absolute_and_errors_omit_their_location() {
        let refused = |path: &str| {
            let config = EncryptionConfig {
                enabled: true,
                key_source: "file".into(),
                key_env: default_key_env(),
                key_file: Some(path.into()),
            };
            match EncryptionKey::from_config(&config) {
                Err(CryptoError::KeySourceUnavailable(message)) => message,
                Err(other) => panic!("{path:?}: {other}"),
                Ok(_) => panic!("{path:?}: a key was loaded"),
            }
        };
        for path in ["key.bin", "./key.bin", "../key.bin", "~/key.bin", ""] {
            assert_eq!(
                refused(path),
                "encryption_key_file must be an absolute path",
                "{path:?}"
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("marker-key-file");
        match EncryptionKey::from_file(&missing) {
            Err(error) => assert!(!error.to_string().contains("marker-key-file")),
            Ok(_) => panic!("a missing key file loaded"),
        }
    }

    /// Final Gate item A: empty or whitespace-only vault key material would
    /// give a constant key and is refused; the accepted forms keep their
    /// derivations.
    #[test]
    fn p0_fg_a_empty_vault_environment_keys_are_refused() {
        for value in ["", " ", "\t\r\n"] {
            match EncryptionKey::from_env_value(value) {
                Err(CryptoError::KeySourceUnavailable(message)) => {
                    assert_eq!(message, "NEXUS_ENCRYPTION_KEY is empty", "{value:?}")
                }
                Err(other) => panic!("{value:?}: {other}"),
                Ok(_) => panic!("{value:?}: a key was derived"),
            }
        }
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            EncryptionKey::from_env_value(hex).unwrap().key.to_vec(),
            hex_bytes(hex)
        );
        // SHA-256("synthetic passphrase\n"), computed with sha256sum.
        assert_eq!(
            EncryptionKey::from_env_value("synthetic passphrase\n")
                .unwrap()
                .key
                .to_vec(),
            hex_bytes(PASSPHRASE_VECTOR)
        );
    }

    /// Final Gate item A: the configured variable is the variable read.
    #[test]
    fn p0_fg_a_a_key_env_other_than_nexus_encryption_key_is_refused() {
        for key_env in ["OTHER_KEY", "", "nexus_encryption_key"] {
            let config = EncryptionConfig {
                enabled: true,
                key_source: "env".into(),
                key_env: key_env.into(),
                key_file: None,
            };
            match EncryptionKey::from_config(&config) {
                Err(CryptoError::KeySourceUnavailable(message)) => {
                    assert_eq!(
                        message, "key_env must be NEXUS_ENCRYPTION_KEY",
                        "{key_env:?}"
                    )
                }
                Err(other) => panic!("{key_env:?}: {other}"),
                Ok(_) => panic!("{key_env:?}: a key was loaded"),
            }
        }
    }

    /// SHA-256("synthetic passphrase\n").
    const PASSPHRASE_VECTOR: &str =
        "bc257a67d8a3fdef45bae7c9c88fc7defa64762353cdbefdf86f467ae58d1ffa";

    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
            .collect()
    }

    /// Final Gate item E: the vault key file is validated on the file that is
    /// opened and read.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    mod key_files {
        use super::{hex_bytes, PASSPHRASE_VECTOR};
        use crate::crypto::{
            check_opened_key_file, key_file, CryptoError, EncryptionConfig, EncryptionKey,
            KeyFileRejection, MAX_KEY_FILE_BYTES,
        };
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        use std::path::{Path, PathBuf};

        /// A key file with exactly `mode`, created by this test (so owned by
        /// the user running it).
        fn key_file_at(dir: &Path, name: &str, contents: &[u8], mode: u32) -> PathBuf {
            let path = dir.join(name);
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(contents).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        }

        fn rejection(path: &Path) -> String {
            match EncryptionKey::from_file(path) {
                Err(CryptoError::KeySourceUnavailable(message)) => message,
                Err(other) => panic!("unexpected error: {other}"),
                Ok(_) => panic!("a key was loaded"),
            }
        }

        fn rejected(reason: KeyFileRejection) -> String {
            format!("encryption key file rejected: {}", reason.reason())
        }

        #[test]
        fn p0_fg_e_a_private_regular_key_file_keeps_its_derivation() {
            let dir = tempfile::tempdir().unwrap();
            let raw: Vec<u8> = (0..32).collect();
            for mode in [0o600, 0o400] {
                let path = key_file_at(dir.path(), &format!("raw-{mode:o}"), &raw, mode);
                assert_eq!(EncryptionKey::from_file(&path).unwrap().key.to_vec(), raw);
            }
            let path = key_file_at(dir.path(), "passphrase", b"synthetic passphrase\n", 0o600);
            assert_eq!(
                EncryptionKey::from_file(&path).unwrap().key.to_vec(),
                hex_bytes(PASSPHRASE_VECTOR)
            );
            // Through the configured file source as well.
            let config = EncryptionConfig {
                enabled: true,
                key_source: "file".into(),
                key_env: "NEXUS_ENCRYPTION_KEY".into(),
                key_file: Some(path.to_str().unwrap().into()),
            };
            assert_eq!(
                EncryptionKey::from_config(&config).unwrap().key.to_vec(),
                hex_bytes(PASSPHRASE_VECTOR)
            );
        }

        #[test]
        fn p0_fg_e_key_files_accessible_to_others_are_refused() {
            let dir = tempfile::tempdir().unwrap();
            for mode in [0o640, 0o644, 0o604, 0o660, 0o606, 0o610, 0o601, 0o620] {
                let path =
                    key_file_at(dir.path(), &format!("key-{mode:o}"), b"synthetic key", mode);
                assert_eq!(
                    rejection(&path),
                    rejected(KeyFileRejection::AccessibleToOthers),
                    "{mode:o}"
                );
            }
        }

        #[test]
        fn p0_fg_e_a_symlinked_key_file_is_refused() {
            let dir = tempfile::tempdir().unwrap();
            let target = key_file_at(dir.path(), "target", b"synthetic key", 0o600);
            assert!(EncryptionKey::from_file(&target).is_ok());
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert_eq!(rejection(&link), rejected(KeyFileRejection::Redirected));
            // A dangling link and a link to a directory are refused alike.
            let dangling = dir.path().join("dangling");
            std::os::unix::fs::symlink(dir.path().join("absent"), &dangling).unwrap();
            assert_eq!(rejection(&dangling), rejected(KeyFileRejection::Redirected));
            let to_dir = dir.path().join("to-dir");
            std::os::unix::fs::symlink(dir.path(), &to_dir).unwrap();
            assert_eq!(rejection(&to_dir), rejected(KeyFileRejection::Redirected));
            // Non-claim: only the last component is checked, so a key file
            // reached through a symlinked directory is opened.
            let real_dir = dir.path().join("real");
            std::fs::create_dir(&real_dir).unwrap();
            key_file_at(&real_dir, "key", b"synthetic key", 0o600);
            let dir_link = dir.path().join("dir-link");
            std::os::unix::fs::symlink(&real_dir, &dir_link).unwrap();
            assert!(EncryptionKey::from_file(&dir_link.join("key")).is_ok());
        }

        #[test]
        fn p0_fg_e_non_regular_key_sources_are_refused_without_blocking() {
            let dir = tempfile::tempdir().unwrap();
            assert_eq!(
                rejection(dir.path()),
                rejected(KeyFileRejection::NotRegularFile)
            );
            assert_eq!(
                rejection(Path::new("/dev/null")),
                rejected(KeyFileRejection::NotRegularFile)
            );
            // A FIFO with no writer: opening it must neither block nor be
            // accepted. The result arrives through a channel with a deadline,
            // so a regression fails instead of hanging.
            let fifo = dir.path().join("fifo");
            let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
            // SAFETY: `name` is a valid NUL-terminated path for mkfifo.
            assert_eq!(unsafe { nix::libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            let (sender, receiver) = std::sync::mpsc::channel();
            let opened = fifo.clone();
            std::thread::spawn(move || {
                let _ = sender.send(EncryptionKey::from_file(&opened).map(|_| ()));
            });
            let result = receiver
                .recv_timeout(std::time::Duration::from_secs(30))
                .expect("opening a FIFO key file blocked");
            assert_eq!(
                result.unwrap_err(),
                CryptoError::KeySourceUnavailable(rejected(KeyFileRejection::NotRegularFile))
            );
        }

        #[test]
        fn p0_fg_e_empty_whitespace_and_oversized_key_files_are_refused() {
            let dir = tempfile::tempdir().unwrap();
            for (name, contents) in [
                ("empty", &b""[..]),
                ("newline", &b"\n"[..]),
                ("blank", &b" \t\r\n"[..]),
            ] {
                let path = key_file_at(dir.path(), name, contents, 0o600);
                assert_eq!(
                    rejection(&path),
                    rejected(KeyFileRejection::Empty),
                    "{name}"
                );
            }
            // A sparse file one byte over the bound is refused before any read.
            let path = key_file_at(dir.path(), "large", b"", 0o600);
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(MAX_KEY_FILE_BYTES + 1)
                .unwrap();
            assert_eq!(rejection(&path), rejected(KeyFileRejection::TooLarge));
            // Exactly the bound is a passphrase file.
            let bound = vec![b'k'; MAX_KEY_FILE_BYTES as usize];
            let path = key_file_at(dir.path(), "bound", &bound, 0o600);
            assert!(EncryptionKey::from_file(&path).is_ok());
        }

        #[test]
        fn p0_fg_e_the_key_comes_from_the_file_that_was_opened() {
            let dir = tempfile::tempdir().unwrap();
            let raw: Vec<u8> = (0..32).collect();
            let path = key_file_at(dir.path(), "key", &raw, 0o600);
            // After the open, the path is replaced by another valid key file:
            // the opened file is still the one read.
            let opened = key_file::open(&path).unwrap();
            let other = key_file_at(dir.path(), "other", &[7u8; 32], 0o600);
            std::fs::rename(&other, &path).unwrap();
            let key = EncryptionKey::from_opened_key_file(&opened).unwrap();
            assert_eq!(key.key.to_vec(), raw);
            // The same after the path becomes a symlink.
            let path = key_file_at(dir.path(), "second", &raw, 0o600);
            let opened = key_file::open(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(dir.path().join("key"), &path).unwrap();
            let key = EncryptionKey::from_opened_key_file(&opened).unwrap();
            assert_eq!(key.key.to_vec(), raw);
            // A file that changes after it was opened but before it is read
            // is checked and read as it is then.
            let path = key_file_at(dir.path(), "third", &raw, 0o600);
            let opened = key_file::open(&path).unwrap();
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(b"appended")
                .unwrap();
            let mut expected = raw.clone();
            expected.extend_from_slice(b"appended");
            let key = EncryptionKey::from_opened_key_file(&opened).unwrap();
            assert_eq!(key.key.to_vec(), sha256(&expected));
        }

        /// A change of size, contents time, mode or owner between the checks
        /// and the end of the read is detected.
        #[test]
        fn p0_fg_e_a_key_file_changed_during_the_read_is_detected() {
            let dir = tempfile::tempdir().unwrap();
            let path = key_file_at(dir.path(), "key", b"synthetic key", 0o600);
            let before = std::fs::metadata(&path).unwrap();
            assert!(key_file::unchanged(
                &before,
                &std::fs::metadata(&path).unwrap()
            ));
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(b"x")
                .unwrap();
            assert!(!key_file::unchanged(
                &before,
                &std::fs::metadata(&path).unwrap()
            ));
            let before = std::fs::metadata(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
            assert!(!key_file::unchanged(
                &before,
                &std::fs::metadata(&path).unwrap()
            ));
        }

        fn sha256(bytes: &[u8]) -> Vec<u8> {
            use sha2::Digest;
            sha2::Sha256::digest(bytes).to_vec()
        }

        #[test]
        fn p0_fg_e_opened_key_file_checks_cover_owner_mode_type_and_size() {
            let regular = |owner, mode, len| check_opened_key_file(true, owner, 1000, mode, len);
            assert_eq!(regular(1000, 0o100600, 32), Ok(()));
            assert_eq!(regular(1000, 0o100400, 1), Ok(()));
            assert_eq!(
                regular(1001, 0o100600, 32),
                Err(KeyFileRejection::ForeignOwner)
            );
            assert_eq!(
                regular(0, 0o100600, 32),
                Err(KeyFileRejection::ForeignOwner)
            );
            assert_eq!(
                regular(1000, 0o100640, 32),
                Err(KeyFileRejection::AccessibleToOthers)
            );
            assert_eq!(regular(1000, 0o100600, 0), Err(KeyFileRejection::Empty));
            assert_eq!(
                regular(1000, 0o100600, MAX_KEY_FILE_BYTES + 1),
                Err(KeyFileRejection::TooLarge)
            );
            assert_eq!(
                check_opened_key_file(false, 1000, 1000, 0o100600, 32),
                Err(KeyFileRejection::NotRegularFile)
            );
        }

        #[test]
        fn p0_fg_e_rejections_never_name_or_quote_the_key_file() {
            let dir = tempfile::tempdir().unwrap();
            let secret = b"marker-secret-contents";
            let path = key_file_at(dir.path(), "marker-key-file", secret, 0o644);
            let link = dir.path().join("marker-link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            for candidate in [path.as_path(), link.as_path(), dir.path()] {
                let message = rejection(candidate);
                for marker in ["marker-key-file", "marker-link", "marker-secret", "/tmp"] {
                    assert!(!message.contains(marker), "{marker}: {message}");
                }
            }
        }
    }

    /// Final Gate item E: where ownership and access cannot be checked this
    /// way, a key file is refused.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn p0_fg_e_key_files_are_refused_on_this_platform() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key");
        std::fs::write(&path, [7u8; 32]).unwrap();
        let expected = CryptoError::KeySourceUnavailable(format!(
            "encryption key file rejected: {}",
            KeyFileRejection::UnsupportedPlatform.reason()
        ));
        assert_eq!(EncryptionKey::from_file(&path).unwrap_err(), expected);
        let config = EncryptionConfig {
            enabled: true,
            key_source: "file".into(),
            key_env: default_key_env(),
            key_file: Some(path.to_str().unwrap().into()),
        };
        assert_eq!(EncryptionKey::from_config(&config).unwrap_err(), expected);
    }
}
