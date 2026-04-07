//! Credential storage for deploy providers.
//!
//! Stores credentials as encrypted JSON in `~/.nexus/deploy_credentials.json`.
//! Uses XOR obfuscation with a machine-specific key derived from
//! hostname + username + a fixed salt. This matches the codebase pattern where
//! API keys are stored in `~/.nexus/config.toml` — but deploy credentials
//! get an extra obfuscation layer since they control public site deployments.
//!
//! **Security invariants:**
//! - Credentials NEVER appear in Debug output (see custom Debug on Credentials)
//! - Credentials NEVER appear in audit logs, governance exports, or error messages
//! - The encryption key is derived per-machine, so credentials aren't portable

use super::{Credentials, DeployError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Stored credential entry (serialized to disk).
#[derive(Serialize, Deserialize)]
struct CredentialStore {
    /// Map of provider_id -> encrypted credential blob (hex-encoded).
    entries: HashMap<String, StoredEntry>,
}

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    /// The credential JSON, XOR-obfuscated with the machine key then hex-encoded.
    /// This is not military-grade encryption but prevents casual reading of tokens
    /// from the JSON file, matching the security posture of the rest of the codebase
    /// (which stores API keys in plaintext TOML).
    data: String,
    /// Provider name for display purposes.
    provider: String,
}

fn credentials_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".nexus")
        .join("deploy_credentials.json")
}

/// Derive a machine-specific key for obfuscating stored credentials.
fn machine_key() -> Vec<u8> {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "nexus-host".into());
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "nexus-user".into());

    let mut hasher = Sha256::new();
    hasher.update(b"nexus-deploy-credential-key:");
    hasher.update(hostname.as_bytes());
    hasher.update(b":");
    hasher.update(user.as_bytes());
    hasher.finalize().to_vec()
}

/// XOR-obfuscate data with a repeating key.
fn xor_obfuscate(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect()
}

fn load_store_from(path: &Path) -> CredentialStore {
    if !path.exists() {
        return CredentialStore {
            entries: HashMap::new(),
        };
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or(CredentialStore {
            entries: HashMap::new(),
        })
}

fn save_store_to(path: &Path, store: &CredentialStore) -> Result<(), DeployError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| DeployError::Credential(format!("serialize: {e}")))?;
    std::fs::write(path, json)?;
    Ok(())
}

fn store_to_path(
    path: &Path,
    provider: &str,
    credentials: &Credentials,
) -> Result<(), DeployError> {
    let mut store = load_store_from(path);
    let key = machine_key();

    let cred_json = serde_json::to_string(credentials)
        .map_err(|e| DeployError::Credential(format!("serialize: {e}")))?;
    let obfuscated = xor_obfuscate(cred_json.as_bytes(), &key);

    store.entries.insert(
        provider.to_string(),
        StoredEntry {
            data: hex::encode(obfuscated),
            provider: provider.to_string(),
        },
    );

    save_store_to(path, &store)
}

fn load_from_path(path: &Path, provider: &str) -> Result<Option<Credentials>, DeployError> {
    let store = load_store_from(path);
    let entry = match store.entries.get(provider) {
        Some(e) => e,
        None => return Ok(None),
    };

    let key = machine_key();
    let obfuscated =
        hex::decode(&entry.data).map_err(|e| DeployError::Credential(format!("decode: {e}")))?;
    let plain = xor_obfuscate(&obfuscated, &key);
    let json =
        String::from_utf8(plain).map_err(|e| DeployError::Credential(format!("utf8: {e}")))?;
    let creds: Credentials = serde_json::from_str(&json)
        .map_err(|e| DeployError::Credential(format!("deserialize: {e}")))?;

    Ok(Some(creds))
}

fn delete_from_path(path: &Path, provider: &str) -> Result<(), DeployError> {
    let mut store = load_store_from(path);
    store.entries.remove(provider);
    save_store_to(path, &store)
}

// ─── Public API (uses default credentials_path) ───────────────────────────

/// Store credentials for a provider. Overwrites any existing credentials.
pub fn store_credentials(provider: &str, credentials: &Credentials) -> Result<(), DeployError> {
    store_to_path(&credentials_path(), provider, credentials)
}

/// Load credentials for a provider. Returns None if not stored.
pub fn load_credentials(provider: &str) -> Result<Option<Credentials>, DeployError> {
    load_from_path(&credentials_path(), provider)
}

/// Delete stored credentials for a provider.
pub fn delete_credentials(provider: &str) -> Result<(), DeployError> {
    delete_from_path(&credentials_path(), provider)
}

/// Check if credentials exist for a provider (without loading the full token).
pub fn has_credentials(provider: &str) -> bool {
    let store = load_store_from(&credentials_path());
    store.entries.contains_key(provider)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cred_path() -> PathBuf {
        std::env::temp_dir().join(format!("nexus-cred-test-{}.json", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_store_and_load_credentials() {
        let path = temp_cred_path();
        let creds = Credentials {
            provider: "netlify".into(),
            token: "my-secret-token-12345".into(),
            account_id: None,
            expires_at: Some("2027-01-01T00:00:00Z".into()),
        };

        store_to_path(&path, "netlify", &creds).unwrap();
        let loaded = load_from_path(&path, "netlify").unwrap().unwrap();

        assert_eq!(loaded.provider, "netlify");
        assert_eq!(loaded.token, "my-secret-token-12345");
        assert_eq!(loaded.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_nonexistent_credentials() {
        let path = temp_cred_path();
        let result = load_from_path(&path, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_credentials() {
        let path = temp_cred_path();
        let creds = Credentials {
            provider: "vercel".into(),
            token: "token-abc".into(),
            account_id: None,
            expires_at: None,
        };
        store_to_path(&path, "vercel", &creds).unwrap();
        assert!(load_from_path(&path, "vercel").unwrap().is_some());

        delete_from_path(&path, "vercel").unwrap();
        assert!(load_from_path(&path, "vercel").unwrap().is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_credentials_never_in_debug_output() {
        let creds = Credentials {
            provider: "cloudflare".into(),
            token: "super-secret-cf-token".into(),
            account_id: Some("acct-123".into()),
            expires_at: None,
        };
        let debug = format!("{creds:?}");
        assert!(
            !debug.contains("super-secret"),
            "Token visible in Debug: {debug}"
        );
        assert!(debug.contains("REDACTED"));
        assert!(debug.contains("cloudflare"));
    }

    #[test]
    fn test_stored_file_does_not_contain_plaintext_token() {
        let path = temp_cred_path();
        let creds = Credentials {
            provider: "netlify".into(),
            token: "plaintext-visible-token".into(),
            account_id: None,
            expires_at: None,
        };
        store_to_path(&path, "netlify", &creds).unwrap();

        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            !on_disk.contains("plaintext-visible-token"),
            "Token stored in plaintext on disk!"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_xor_obfuscate_roundtrip() {
        let key = machine_key();
        let data = b"hello world secret token";
        let obfuscated = xor_obfuscate(data, &key);
        assert_ne!(&obfuscated, data);
        let restored = xor_obfuscate(&obfuscated, &key);
        assert_eq!(&restored, data);
    }
}
