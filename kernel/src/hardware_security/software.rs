//! Software key backend with optional sealed (encrypted-at-rest) persistence.
//!
//! Keys are generated with [`OsRng`] and stored in memory. When a
//! [`SealedKeyStore`] is attached, each key is also encrypted with
//! AES-256-GCM and written to disk so it survives restarts.

use crate::hardware_security::sealed_store::SealedKeyStore;
use crate::hardware_security::types::{
    deterministic_attestation, short_hash_prefix, AttestationReport, KeyBackend, KeyError,
    KeyHandle, KeyPurpose, PublicKeyBytes, SignatureBytes,
};
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Metadata stored alongside each sealed key so we can reconstruct state.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SealedKeyMeta {
    purpose: KeyPurpose,
    deprecated: bool,
}

#[derive(Clone)]
struct StoredSoftwareKey {
    purpose: KeyPurpose,
    signing_key: SigningKey,
    deprecated: bool,
}

/// Production-quality software key backend.
///
/// * Keys generated with cryptographic randomness ([`OsRng`]).
/// * Optional sealed persistence via [`SealedKeyStore`] — keys encrypted at
///   rest with AES-256-GCM, sealing key derived from machine identity via HKDF.
/// * Falls back to in-memory-only when no sealed store is configured (tests).
#[derive(Default)]
pub struct SoftwareBackend {
    keys: BTreeMap<String, StoredSoftwareKey>,
    generation_counter: u64,
    sealed_store: Option<SealedKeyStore>,
}

impl Clone for SoftwareBackend {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            generation_counter: self.generation_counter,
            sealed_store: self.sealed_store.clone(),
        }
    }
}

impl SoftwareBackend {
    /// Create a backend with sealed persistence. Existing sealed keys are
    /// loaded from disk immediately.
    pub fn with_sealed_store(store: SealedKeyStore) -> Result<Self, KeyError> {
        let mut backend = Self {
            keys: BTreeMap::new(),
            generation_counter: 0,
            sealed_store: Some(store),
        };
        backend.load_sealed_keys()?;
        Ok(backend)
    }

    /// Load all previously-sealed keys from disk into memory.
    fn load_sealed_keys(&mut self) -> Result<(), KeyError> {
        let store = match &self.sealed_store {
            Some(s) => s.clone(),
            None => return Ok(()),
        };

        let handle_ids = store.list_sealed()?;
        for handle_id in handle_ids {
            // Sealed blob format: meta_len(4 LE) || meta_json || seed(32)
            let plaintext = store.unseal_key(&handle_id)?;
            let (meta, signing_key) = Self::decode_sealed_payload(&plaintext)?;

            // Track highest sequence number to avoid handle ID collisions.
            if let Some(seq) = Self::parse_sequence_from_handle(&handle_id) {
                if seq > self.generation_counter {
                    self.generation_counter = seq;
                }
            }

            self.keys.insert(
                handle_id,
                StoredSoftwareKey {
                    purpose: meta.purpose,
                    signing_key,
                    deprecated: meta.deprecated,
                },
            );
        }
        Ok(())
    }

    /// Encode key material + metadata into a single blob for sealing.
    ///
    /// Format: `meta_len(4 LE) || meta_json || seed(32)`
    fn encode_sealed_payload(meta: &SealedKeyMeta, seed: &[u8; 32]) -> Vec<u8> {
        let meta_json = serde_json::to_vec(meta).expect("SealedKeyMeta is always serializable");
        let meta_len = (meta_json.len() as u32).to_le_bytes();
        let mut buf = Vec::with_capacity(4 + meta_json.len() + 32);
        buf.extend_from_slice(&meta_len);
        buf.extend_from_slice(&meta_json);
        buf.extend_from_slice(seed);
        buf
    }

    /// Decode a sealed payload back into metadata + signing key.
    fn decode_sealed_payload(plaintext: &[u8]) -> Result<(SealedKeyMeta, SigningKey), KeyError> {
        if plaintext.len() < 4 + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "sealed payload too short".to_string(),
            ));
        }
        let meta_len = u32::from_le_bytes(plaintext[..4].try_into().expect("4 bytes")) as usize;
        if plaintext.len() < 4 + meta_len + 32 {
            return Err(KeyError::InvalidKeyMaterial(
                "sealed payload truncated".to_string(),
            ));
        }
        let meta: SealedKeyMeta =
            serde_json::from_slice(&plaintext[4..4 + meta_len]).map_err(|e| {
                KeyError::InvalidKeyMaterial(format!("sealed metadata parse error: {e}"))
            })?;
        let seed_start = 4 + meta_len;
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&plaintext[seed_start..seed_start + 32]);
        let signing_key = SigningKey::from_bytes(&seed);
        Ok((meta, signing_key))
    }

    /// Parse the sequence number from a handle ID like `sw-agent_identity-00000003-abcdef01`.
    fn parse_sequence_from_handle(handle_id: &str) -> Option<u64> {
        let parts: Vec<&str> = handle_id.split('-').collect();
        // Format: sw-{purpose_part(s)}-{hex_sequence}-{hash}
        // Find the part that looks like an 8-char hex sequence number.
        for part in &parts {
            if part.len() == 8 && u64::from_str_radix(part, 16).is_ok() {
                return u64::from_str_radix(part, 16).ok();
            }
        }
        None
    }

    fn next_sequence(&mut self) -> u64 {
        self.generation_counter = self.generation_counter.saturating_add(1);
        self.generation_counter
    }

    /// Seal a key to disk if a sealed store is configured.
    fn seal_if_configured(
        &self,
        handle_id: &str,
        meta: &SealedKeyMeta,
        seed: &[u8; 32],
    ) -> Result<(), KeyError> {
        if let Some(store) = &self.sealed_store {
            let payload = Self::encode_sealed_payload(meta, seed);
            store.seal_key(handle_id, &payload)?;
        }
        Ok(())
    }
}

impl KeyBackend for SoftwareBackend {
    fn backend_name(&self) -> &'static str {
        "software"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn generate_ed25519(&mut self, purpose: KeyPurpose) -> Result<KeyHandle, KeyError> {
        let sequence = self.next_sequence();

        // Generate with cryptographic randomness.
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);

        let public = signing_key.verifying_key().to_bytes();
        let handle_id = format!(
            "sw-{}-{sequence:08x}-{}",
            purpose.as_str(),
            short_hash_prefix(&public)
        );

        let meta = SealedKeyMeta {
            purpose,
            deprecated: false,
        };
        self.seal_if_configured(&handle_id, &meta, &seed)?;

        let handle = KeyHandle {
            id: handle_id.clone(),
            purpose,
        };
        self.keys.insert(
            handle_id,
            StoredSoftwareKey {
                purpose,
                signing_key,
                deprecated: false,
            },
        );
        Ok(handle)
    }

    fn public_key(&self, handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError> {
        let stored = self
            .keys
            .get(handle.id.as_str())
            .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;
        if stored.purpose != handle.purpose {
            return Err(KeyError::PurposeMismatch {
                expected: handle.purpose,
                actual: stored.purpose,
            });
        }
        Ok(PublicKeyBytes(
            stored.signing_key.verifying_key().to_bytes().to_vec(),
        ))
    }

    fn sign(&self, handle: &KeyHandle, msg: &[u8]) -> Result<SignatureBytes, KeyError> {
        let stored = self
            .keys
            .get(handle.id.as_str())
            .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;
        if stored.purpose != handle.purpose {
            return Err(KeyError::PurposeMismatch {
                expected: handle.purpose,
                actual: stored.purpose,
            });
        }
        let signature = stored.signing_key.sign(msg);
        Ok(SignatureBytes(signature.to_bytes().to_vec()))
    }

    fn rotate(&mut self, handle: &KeyHandle) -> Result<KeyHandle, KeyError> {
        let purpose = {
            let stored = self
                .keys
                .get_mut(handle.id.as_str())
                .ok_or_else(|| KeyError::KeyNotFound(handle.id.clone()))?;
            if stored.purpose != handle.purpose {
                return Err(KeyError::PurposeMismatch {
                    expected: handle.purpose,
                    actual: stored.purpose,
                });
            }
            stored.deprecated = true;

            // Re-seal deprecated state if persistence is enabled.
            if let Some(store) = &self.sealed_store {
                let meta = SealedKeyMeta {
                    purpose: stored.purpose,
                    deprecated: true,
                };
                let seed = stored.signing_key.to_bytes();
                let handle_id = handle.id.clone();
                let payload = Self::encode_sealed_payload(&meta, &seed);
                store.seal_key(&handle_id, &payload)?;
            }

            stored.purpose
        };
        self.generate_ed25519(purpose)
    }

    fn attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.backend_name(),
            self.is_available(),
            nonce,
        ))
    }
}
