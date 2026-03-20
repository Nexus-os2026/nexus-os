use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultUserKey {
    pub key_id: String,
    pub bytes: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedSecret {
    key_id: String,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct SecretsVault {
    secrets: HashMap<String, EncryptedSecret>,
    audit_trail: AuditTrail,
}

impl SecretsVault {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn store_secret(
        &mut self,
        name: &str,
        value: &str,
        user_key: &VaultUserKey,
    ) -> Result<(), AgentError> {
        let cipher = self.cipher_from_key(user_key)?;
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(&Nonce::from(nonce), value.as_bytes())
            .map_err(|_| AgentError::SupervisorError("failed to encrypt secret".to_string()))?;

        self.secrets.insert(
            name.to_string(),
            EncryptedSecret {
                key_id: user_key.key_id.clone(),
                nonce,
                ciphertext,
            },
        );

        self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "secret_stored",
                "secret_name": name
            }),
        )?;

        Ok(())
    }

    pub fn get_secret(
        &mut self,
        name: &str,
        user_key: &VaultUserKey,
    ) -> Result<String, AgentError> {
        let encrypted = self
            .secrets
            .get(name)
            .ok_or_else(|| AgentError::SupervisorError(format!("secret '{name}' not found")))?;

        if encrypted.key_id != user_key.key_id {
            return Err(AgentError::CapabilityDenied(format!(
                "secret '{name}' cannot be decrypted with the provided key"
            )));
        }

        let cipher = self.cipher_from_key(user_key)?;
        let plaintext = cipher
            .decrypt(&Nonce::from(encrypted.nonce), encrypted.ciphertext.as_ref())
            .map_err(|_| AgentError::SupervisorError("failed to decrypt secret".to_string()))?;

        let decoded = String::from_utf8(plaintext)
            .map_err(|_| AgentError::SupervisorError("secret payload is not utf-8".to_string()))?;

        self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "secret_accessed",
                "secret_name": name
            }),
        )?;

        Ok(decoded)
    }

    pub fn delete_secret(&mut self, name: &str) {
        self.secrets.remove(name);
        if let Err(e) = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "secret_deleted",
                "secret_name": name
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }

    pub fn list_secrets(&self) -> Vec<String> {
        let mut names = self.secrets.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    fn cipher_from_key(&self, user_key: &VaultUserKey) -> Result<Aes256Gcm, AgentError> {
        Aes256Gcm::new_from_slice(&user_key.bytes)
            .map_err(|_| AgentError::SupervisorError("invalid AES-256 key".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{SecretsVault, VaultUserKey};

    fn sample_key() -> VaultUserKey {
        VaultUserKey {
            key_id: "user-key-1".to_string(),
            bytes: [42_u8; 32],
        }
    }

    #[test]
    fn test_store_and_retrieve_secret() {
        let mut vault = SecretsVault::new();
        let key = sample_key();

        let store = vault.store_secret("github_token", "ghp_abc123", &key);
        assert!(store.is_ok());

        let secret = vault.get_secret("github_token", &key);
        assert_eq!(secret, Ok("ghp_abc123".to_string()));
    }

    #[test]
    fn test_secret_never_in_audit() {
        let mut vault = SecretsVault::new();
        let key = sample_key();
        let secret_value = "ghp_abc123";

        let store = vault.store_secret("github_token", secret_value, &key);
        assert!(store.is_ok());
        let get = vault.get_secret("github_token", &key);
        assert!(get.is_ok());

        let mut accessed_logged = false;
        let mut leaked_secret = false;

        for event in vault.audit_trail().events() {
            let payload_text: String = serde_json::to_string(&event.payload).unwrap_or_default();
            if payload_text.contains("secret_accessed") {
                accessed_logged = true;
            }
            if payload_text.contains(secret_value) {
                leaked_secret = true;
            }
        }

        assert!(accessed_logged);
        assert!(!leaked_secret);
    }
}
