use crate::hardware_security::types::{
    deterministic_attestation, sha256_bytes, short_hash_prefix, AttestationReport, KeyBackend,
    KeyError, KeyHandle, KeyPurpose, PublicKeyBytes, SignatureBytes,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::BTreeMap;

#[derive(Clone)]
struct StoredSoftwareKey {
    purpose: KeyPurpose,
    signing_key: SigningKey,
    deprecated: bool,
}

#[derive(Clone, Default)]
pub struct SoftwareBackend {
    keys: BTreeMap<String, StoredSoftwareKey>,
    generation_counter: u64,
}

impl SoftwareBackend {
    fn derive_seed(purpose: KeyPurpose, sequence: u64) -> [u8; 32] {
        let mut input = Vec::new();
        input.extend_from_slice(b"nexus.software.key");
        input.push(b':');
        input.extend_from_slice(purpose.as_str().as_bytes());
        input.push(b':');
        input.extend_from_slice(sequence.to_string().as_bytes());
        sha256_bytes(input.as_slice())
    }

    fn next_sequence(&mut self) -> u64 {
        self.generation_counter = self.generation_counter.saturating_add(1);
        self.generation_counter
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
        let seed = Self::derive_seed(purpose, sequence);
        let signing_key = SigningKey::from_bytes(&seed);
        let public = signing_key.verifying_key().to_bytes();
        let handle_id = format!(
            "sw-{}-{sequence:08x}-{}",
            purpose.as_str(),
            short_hash_prefix(&public)
        );
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
            stored.purpose
        };
        self.generate_ed25519(purpose)
    }

    fn attest(&self) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.backend_name(),
            self.is_available(),
        ))
    }
}
