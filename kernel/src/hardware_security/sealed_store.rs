//! Sealed key storage: AES-256-GCM encryption at rest with HKDF-derived sealing key.
//!
//! Mirrors the TEE sealed-storage pattern in software: keys are encrypted with a
//! machine-derived sealing key before touching disk. Each key file is stored as
//! `{handle_id}.sealed` containing `nonce(12) || ciphertext`.

use crate::hardware_security::types::KeyError;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use std::path::PathBuf;

const NONCE_LEN: usize = 12;
const SEAL_INFO: &[u8] = b"nexus.hardware_security.sealed_store.v1";

/// Manages a directory of AES-256-GCM sealed key files.
#[derive(Debug, Clone)]
pub struct SealedKeyStore {
    dir: PathBuf,
    cipher: SealingCipher,
}

/// Wraps the derived AES-256-GCM key so we don't store the raw bytes directly.
#[derive(Clone)]
struct SealingCipher {
    key_bytes: [u8; 32],
}

impl std::fmt::Debug for SealingCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealingCipher")
            .field("key_bytes", &"[REDACTED]")
            .finish()
    }
}

impl SealingCipher {
    fn new(key_bytes: [u8; 32]) -> Self {
        Self { key_bytes }
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new((&self.key_bytes).into())
    }
}

impl SealedKeyStore {
    /// Create a new store at `dir`, deriving the sealing key from `master_secret`
    /// via HKDF-SHA256.
    pub fn new(dir: impl Into<PathBuf>, master_secret: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(None, master_secret);
        let mut okm = [0u8; 32];
        if let Err(e) = hk.expand(SEAL_INFO, &mut okm) {
            eprintln!("HKDF-SHA256 expand failed (should never happen for 32 bytes): {e}");
        }

        Self {
            dir: dir.into(),
            cipher: SealingCipher::new(okm),
        }
    }

    /// Encrypt `key_bytes` and write to `{handle_id}.sealed` in the store directory.
    pub fn seal_key(&self, handle_id: &str, key_bytes: &[u8]) -> Result<(), KeyError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| KeyError::BackendFailure(format!("sealed store: create dir: {e}")))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from(nonce_bytes);

        let ciphertext = self
            .cipher
            .cipher()
            .encrypt(&nonce, key_bytes)
            .map_err(|e| KeyError::BackendFailure(format!("sealed store: encrypt: {e}")))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);

        let path = self.sealed_path(handle_id);
        std::fs::write(&path, &blob).map_err(|e| {
            KeyError::BackendFailure(format!("sealed store: write {}: {e}", path.display()))
        })?;

        Ok(())
    }

    /// Read and decrypt the sealed key for `handle_id`.
    pub fn unseal_key(&self, handle_id: &str) -> Result<Vec<u8>, KeyError> {
        let path = self.sealed_path(handle_id);
        let blob = std::fs::read(&path).map_err(|e| {
            KeyError::BackendFailure(format!("sealed store: read {}: {e}", path.display()))
        })?;

        if blob.len() < NONCE_LEN + 1 {
            return Err(KeyError::InvalidKeyMaterial(
                "sealed file too short".to_string(),
            ));
        }

        let nonce_array: [u8; NONCE_LEN] = blob[..NONCE_LEN]
            .try_into()
            .map_err(|_| KeyError::InvalidKeyMaterial("nonce length mismatch".to_string()))?;
        let nonce = Nonce::from(nonce_array);
        let ciphertext = &blob[NONCE_LEN..];

        self.cipher
            .cipher()
            .decrypt(&nonce, ciphertext)
            .map_err(|_| {
                KeyError::InvalidKeyMaterial(
                    "sealed store: decryption failed (wrong sealing key or corrupted file)"
                        .to_string(),
                )
            })
    }

    /// List all sealed handle IDs present in the store directory.
    pub fn list_sealed(&self) -> Result<Vec<String>, KeyError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| KeyError::BackendFailure(format!("sealed store: read dir: {e}")))?;
        for entry in entries {
            let entry = entry
                .map_err(|e| KeyError::BackendFailure(format!("sealed store: dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sealed") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }
        Ok(ids)
    }

    /// Remove a sealed key file.
    pub fn remove_sealed(&self, handle_id: &str) -> Result<(), KeyError> {
        let path = self.sealed_path(handle_id);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                KeyError::BackendFailure(format!("sealed store: remove {}: {e}", path.display()))
            })?;
        }
        Ok(())
    }

    fn sealed_path(&self, handle_id: &str) -> PathBuf {
        self.dir.join(format!("{handle_id}.sealed"))
    }
}

/// Derive a master secret from machine identity.
///
/// On Linux, reads `/etc/machine-id`. Falls back to a deterministic value
/// derived from the hostname. This is not true TEE security — it prevents
/// casual file theft but not a determined attacker with root on the same machine.
pub fn derive_machine_secret() -> Vec<u8> {
    use sha2::Digest as _;

    // Try /etc/machine-id first (systemd machines).
    if let Ok(mid) = std::fs::read_to_string("/etc/machine-id") {
        let trimmed = mid.trim();
        if !trimmed.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(b"nexus.machine_secret:");
            hasher.update(trimmed.as_bytes());
            return hasher.finalize().to_vec();
        }
    }

    // Fallback: /etc/hostname or a static salt.
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "nexus-unknown-host".to_string());

    let mut hasher = Sha256::new();
    hasher.update(b"nexus.machine_secret.hostname:");
    hasher.update(hostname.as_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SealedKeyStore::new(dir.path(), b"test-master-secret");

        let key_material = b"this-is-a-32-byte-ed25519-seed!!";
        store.seal_key("test-handle-001", key_material).unwrap();

        let recovered = store.unseal_key("test-handle-001").unwrap();
        assert_eq!(recovered.as_slice(), key_material);
    }

    #[test]
    fn unseal_with_wrong_secret_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store_a = SealedKeyStore::new(dir.path(), b"secret-a");
        let store_b = SealedKeyStore::new(dir.path(), b"secret-b");

        store_a.seal_key("key-001", b"sensitive-data").unwrap();
        let result = store_b.unseal_key("key-001");
        assert!(result.is_err());
    }

    #[test]
    fn list_sealed_returns_stored_handles() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SealedKeyStore::new(dir.path(), b"test-secret");

        store.seal_key("handle-a", b"data-a").unwrap();
        store.seal_key("handle-b", b"data-b").unwrap();

        let mut ids = store.list_sealed().unwrap();
        ids.sort();
        assert_eq!(ids, vec!["handle-a", "handle-b"]);
    }

    #[test]
    fn remove_sealed_deletes_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = SealedKeyStore::new(dir.path(), b"test-secret");

        store.seal_key("handle-x", b"data").unwrap();
        assert_eq!(store.list_sealed().unwrap().len(), 1);

        store.remove_sealed("handle-x").unwrap();
        assert_eq!(store.list_sealed().unwrap().len(), 0);
    }

    #[test]
    fn machine_secret_is_deterministic() {
        let a = derive_machine_secret();
        let b = derive_machine_secret();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }
}
