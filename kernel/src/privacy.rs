use crate::errors::AgentError;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserKey {
    pub id: String,
    pub bytes: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedField {
    pub key_id: String,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct PrivacyManager {
    known_keys: HashMap<String, [u8; 32]>,
    destroyed_keys: HashSet<String>,
}

impl PrivacyManager {
    pub fn new() -> Self {
        Self {
            known_keys: HashMap::new(),
            destroyed_keys: HashSet::new(),
        }
    }

    pub fn encrypt_field(
        &mut self,
        plaintext: &[u8],
        user_key: &UserKey,
    ) -> Result<EncryptedField, AgentError> {
        self.assert_key_is_active(&user_key.id)?;
        self.known_keys.insert(user_key.id.clone(), user_key.bytes);

        let cipher = Aes256Gcm::new_from_slice(&user_key.bytes)
            .map_err(|_| AgentError::SupervisorError("invalid AES-256 key length".to_string()))?;
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| AgentError::SupervisorError("encryption failure".to_string()))?;

        Ok(EncryptedField {
            key_id: user_key.id.clone(),
            nonce,
            ciphertext,
        })
    }

    pub fn decrypt_field(
        &self,
        encrypted: &EncryptedField,
        user_key: &UserKey,
    ) -> Result<Vec<u8>, AgentError> {
        if encrypted.key_id != user_key.id {
            return Err(AgentError::SupervisorError(
                "ciphertext key identifier mismatch".to_string(),
            ));
        }
        self.assert_key_is_active(&user_key.id)?;

        let cipher = Aes256Gcm::new_from_slice(&user_key.bytes)
            .map_err(|_| AgentError::SupervisorError("invalid AES-256 key length".to_string()))?;
        cipher
            .decrypt(
                Nonce::from_slice(&encrypted.nonce),
                encrypted.ciphertext.as_ref(),
            )
            .map_err(|_| AgentError::SupervisorError("decryption failure".to_string()))
    }

    pub fn erase_key(&mut self, user_key_id: &str) {
        self.destroyed_keys.insert(user_key_id.to_string());
        self.known_keys.remove(user_key_id);
    }

    fn assert_key_is_active(&self, user_key_id: &str) -> Result<(), AgentError> {
        if self.destroyed_keys.contains(user_key_id) {
            return Err(AgentError::KeyDestroyed(user_key_id.to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PrivacyManager, UserKey};
    use crate::errors::AgentError;

    fn sample_key(id: &str) -> UserKey {
        UserKey {
            id: id.to_string(),
            bytes: [7_u8; 32],
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut privacy = PrivacyManager::new();
        let key = sample_key("user-001");
        let plaintext = b"top-secret-data";

        let encrypted = privacy
            .encrypt_field(plaintext, &key)
            .expect("encryption should succeed");
        let decrypted = privacy
            .decrypt_field(&encrypted, &key)
            .expect("decryption should succeed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_crypto_erasure() {
        let mut privacy = PrivacyManager::new();
        let key = sample_key("user-erase");
        let plaintext = b"erase-me";

        let encrypted = privacy
            .encrypt_field(plaintext, &key)
            .expect("encryption should succeed before erasure");
        privacy.erase_key(&key.id);

        let result = privacy.decrypt_field(&encrypted, &key);
        assert_eq!(
            result,
            Err(AgentError::KeyDestroyed("user-erase".to_string()))
        );
    }
}
