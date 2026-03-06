//! API key authentication with SHA-256 hashed storage.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub key_id: String,
    pub key_hash: String,
    pub tenant_id: Uuid,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub revoked: bool,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash_key(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_raw_key() -> String {
    format!("nxk_{}", Uuid::new_v4().as_hyphenated())
}

#[derive(Debug)]
pub struct AuthManager {
    keys: HashMap<String, ApiKey>, // key_id -> ApiKey
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// Create a new API key for the given tenant. Returns (key_id, raw_key).
    /// The raw key is only returned once — only the hash is stored.
    pub fn create_key(&mut self, tenant_id: Uuid) -> (String, String) {
        let raw_key = generate_raw_key();
        let key_id = format!("kid_{}", Uuid::new_v4().as_simple());
        let api_key = ApiKey {
            key_id: key_id.clone(),
            key_hash: hash_key(&raw_key),
            tenant_id,
            created_at: unix_now(),
            expires_at: None,
            revoked: false,
        };
        self.keys.insert(key_id.clone(), api_key);
        (key_id, raw_key)
    }

    /// Create a key with an explicit expiration timestamp.
    pub fn create_key_with_expiry(&mut self, tenant_id: Uuid, expires_at: u64) -> (String, String) {
        let (key_id, raw_key) = self.create_key(tenant_id);
        if let Some(key) = self.keys.get_mut(&key_id) {
            key.expires_at = Some(expires_at);
        }
        (key_id, raw_key)
    }

    /// Verify a raw API key. Returns the tenant_id if the key is valid,
    /// not revoked, and not expired.
    pub fn verify_key(&self, raw_key: &str) -> Option<Uuid> {
        let h = hash_key(raw_key);
        let now = unix_now();

        for key in self.keys.values() {
            if key.key_hash == h {
                if key.revoked {
                    return None;
                }
                if let Some(exp) = key.expires_at {
                    if now > exp {
                        return None;
                    }
                }
                return Some(key.tenant_id);
            }
        }
        None
    }

    pub fn revoke_key(&mut self, key_id: &str) -> bool {
        if let Some(key) = self.keys.get_mut(key_id) {
            key.revoked = true;
            true
        } else {
            false
        }
    }

    pub fn list_keys(&self, tenant_id: Uuid) -> Vec<&ApiKey> {
        self.keys
            .values()
            .filter(|k| k.tenant_id == tenant_id)
            .collect()
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_verify_key() {
        let mut auth = AuthManager::new();
        let tenant = Uuid::new_v4();
        let (key_id, raw_key) = auth.create_key(tenant);

        assert!(!key_id.is_empty());
        assert!(raw_key.starts_with("nxk_"));

        let result = auth.verify_key(&raw_key);
        assert_eq!(result, Some(tenant));
    }

    #[test]
    fn raw_key_is_never_stored() {
        let mut auth = AuthManager::new();
        let tenant = Uuid::new_v4();
        let (_key_id, raw_key) = auth.create_key(tenant);

        // No stored ApiKey should contain the raw key
        for key in auth.keys.values() {
            assert_ne!(key.key_hash, raw_key);
            assert!(!key.key_hash.contains("nxk_"));
            // The hash should be a 64-char hex string (SHA-256)
            assert_eq!(key.key_hash.len(), 64);
            assert!(key.key_hash.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn invalid_key_rejected() {
        let auth = AuthManager::new();
        assert_eq!(auth.verify_key("nxk_bogus-key-value"), None);
    }

    #[test]
    fn revoked_key_rejected() {
        let mut auth = AuthManager::new();
        let tenant = Uuid::new_v4();
        let (key_id, raw_key) = auth.create_key(tenant);

        assert!(auth.revoke_key(&key_id));
        assert_eq!(auth.verify_key(&raw_key), None);
    }

    #[test]
    fn revoke_nonexistent_returns_false() {
        let mut auth = AuthManager::new();
        assert!(!auth.revoke_key("kid_doesnotexist"));
    }

    #[test]
    fn expired_key_rejected() {
        let mut auth = AuthManager::new();
        let tenant = Uuid::new_v4();
        // Expire in the past
        let (_key_id, raw_key) = auth.create_key_with_expiry(tenant, 1);

        assert_eq!(auth.verify_key(&raw_key), None);
    }

    #[test]
    fn list_keys_filters_by_tenant() {
        let mut auth = AuthManager::new();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        auth.create_key(tenant_a);
        auth.create_key(tenant_a);
        auth.create_key(tenant_b);

        assert_eq!(auth.list_keys(tenant_a).len(), 2);
        assert_eq!(auth.list_keys(tenant_b).len(), 1);
        assert_eq!(auth.list_keys(Uuid::new_v4()).len(), 0);
    }
}
