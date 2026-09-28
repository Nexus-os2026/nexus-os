//! Credential storage for deploy providers.
//!
//! Final Gate items A and H: this store is read-only in Phase Zero.
//!
//! The legacy store, `~/.nexus/deploy_credentials.json`, is not encrypted. Each
//! entry is the credential JSON XORed with a repeating 32-byte key,
//! SHA-256("nexus-deploy-credential-key:" ‖ HOSTNAME ‖ ":" ‖ USER), then
//! hex-encoded. The key is derived from the host and account names only, and
//! every entry starts with the known text `{"provider":"<name>","token":"`, so
//! the key stream can be recovered from a stored entry without any secret.
//! The file was written with default permissions.
//!
//! No approved secret store exists for these credentials, so:
//! - new or changed credentials are never stored ([`store_credentials`]
//!   refuses without touching the file);
//! - entries already stored stay readable through the explicit legacy read
//!   path ([`load_credentials`]), and are never deleted, rewritten or
//!   migrated by Nexus on its own;
//! - refusing new writes does not protect the entries already on disk.
//!
//! Credentials never appear in Debug output, audit logs, governance exports
//! or error messages (see the custom Debug on `Credentials`).

use super::{Credentials, DeployError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Why a deploy credential is not stored (Final Gate items A and H).
const STORAGE_REFUSED: &str =
    "deploy credential storage is unavailable in Phase Zero: no approved secret store";

/// Stored credential entry (serialized to disk).
#[derive(Serialize, Deserialize)]
struct CredentialStore {
    /// Map of provider_id -> encrypted credential blob (hex-encoded).
    entries: HashMap<String, StoredEntry>,
}

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    /// The credential JSON, XOR-obfuscated with the legacy machine key, then
    /// hex-encoded. This is obfuscation, not encryption.
    data: String,
    /// Provider name for display purposes.
    provider: String,
}

/// The credential store under the validated identity home (P0-002C5B).
pub(crate) fn credentials_path() -> Result<PathBuf, DeployError> {
    nexus_kernel::identity_home::nexus_state_path("deploy_credentials.json")
        .map_err(|e| DeployError::Credential(e.to_string()))
}

/// The legacy obfuscation key for this host and account, used only to read
/// entries stored before Phase Zero closed the store.
fn machine_key() -> Vec<u8> {
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "nexus-host".into());
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "nexus-user".into());
    legacy_machine_key(&hostname, &user)
}

/// SHA-256("nexus-deploy-credential-key:" ‖ hostname ‖ ":" ‖ user): the
/// legacy key, unchanged.
fn legacy_machine_key(hostname: &str, user: &str) -> Vec<u8> {
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

/// The store for a rewrite. A missing file is an empty store; a file that
/// cannot be read or parsed refuses the rewrite, so its other entries are
/// never dropped.
fn load_store_for_rewrite(path: &Path) -> Result<CredentialStore, DeployError> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CredentialStore {
                entries: HashMap::new(),
            })
        }
        Err(_) => {
            return Err(DeployError::Credential(
                "the stored deploy credentials cannot be read, so they are not rewritten".into(),
            ))
        }
    };
    serde_json::from_str(&json).map_err(|_| {
        DeployError::Credential(
            "the stored deploy credentials cannot be parsed, so they are not rewritten".into(),
        )
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

/// Final Gate items A and H: a new or changed deploy credential is never
/// stored. The file is not touched.
pub(crate) fn store_to_path(
    _path: &Path,
    _provider: &str,
    _credentials: &Credentials,
) -> Result<(), DeployError> {
    Err(DeployError::Credential(STORAGE_REFUSED.into()))
}

fn load_from_path(path: &Path, provider: &str) -> Result<Option<Credentials>, DeployError> {
    load_from_path_with_key(path, provider, &machine_key())
}

/// The explicit legacy read path: one stored entry, de-obfuscated with the
/// legacy key.
fn load_from_path_with_key(
    path: &Path,
    provider: &str,
    key: &[u8],
) -> Result<Option<Credentials>, DeployError> {
    let store = load_store_from(path);
    let entry = match store.entries.get(provider) {
        Some(e) => e,
        None => return Ok(None),
    };

    let obfuscated =
        hex::decode(&entry.data).map_err(|e| DeployError::Credential(format!("decode: {e}")))?;
    let plain = xor_obfuscate(&obfuscated, key);
    let json =
        String::from_utf8(plain).map_err(|e| DeployError::Credential(format!("utf8: {e}")))?;
    let creds: Credentials = serde_json::from_str(&json)
        .map_err(|e| DeployError::Credential(format!("deserialize: {e}")))?;

    Ok(Some(creds))
}

fn delete_from_path(path: &Path, provider: &str) -> Result<(), DeployError> {
    let mut store = load_store_for_rewrite(path)?;
    store.entries.remove(provider);
    save_store_to(path, &store)
}

// ─── Public API (uses default credentials_path) ───────────────────────────

/// Store credentials for a provider: refused in Phase Zero (Final Gate items
/// A and H). No approved secret store exists, and nothing is written.
pub fn store_credentials(provider: &str, credentials: &Credentials) -> Result<(), DeployError> {
    store_to_path(&credentials_path()?, provider, credentials)
}

/// Load credentials for a provider stored before Phase Zero closed the store.
/// Returns None if not stored.
pub fn load_credentials(provider: &str) -> Result<Option<Credentials>, DeployError> {
    load_from_path(&credentials_path()?, provider)
}

/// Delete stored credentials for a provider. A store that cannot be read is
/// left as it is.
pub fn delete_credentials(provider: &str) -> Result<(), DeployError> {
    delete_from_path(&credentials_path()?, provider)
}

/// Check if credentials exist for a provider (without loading the full token).
pub fn has_credentials(provider: &str) -> bool {
    credentials_path()
        .map(|path| load_store_from(&path).entries.contains_key(provider))
        .unwrap_or(false)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cred_path() -> PathBuf {
        std::env::temp_dir().join(format!("nexus-cred-test-{}.json", uuid::Uuid::new_v4()))
    }

    /// SHA-256("nexus-deploy-credential-key:synthetic-host:synthetic-user"),
    /// computed independently with sha256sum.
    const LEGACY_KEY_VECTOR: &str =
        "e1734f2ae3088656f5bef85e2b1d4354ca307ffa91be29681dddecde88afed73";

    fn legacy_key() -> Vec<u8> {
        hex::decode(LEGACY_KEY_VECTOR).unwrap()
    }

    /// A store file exactly as the pre-Phase-Zero writer produced it, built
    /// without the code under test.
    fn write_legacy_store(path: &Path, entries: &[Credentials], key: &[u8]) -> String {
        let mut store = serde_json::Map::new();
        for credentials in entries {
            let json = serde_json::to_string(credentials).unwrap();
            let data: Vec<u8> = json
                .bytes()
                .enumerate()
                .map(|(i, b)| b ^ key[i % key.len()])
                .collect();
            store.insert(
                credentials.provider.clone(),
                serde_json::json!({"data": hex::encode(data), "provider": credentials.provider}),
            );
        }
        let text = serde_json::to_string_pretty(&serde_json::json!({ "entries": store })).unwrap();
        std::fs::write(path, &text).unwrap();
        text
    }

    fn credentials(provider: &str, token: &str) -> Credentials {
        Credentials {
            provider: provider.into(),
            token: token.into(),
            account_id: None,
            expires_at: None,
        }
    }

    #[test]
    fn p0_fg_a_the_legacy_key_derivation_is_unchanged() {
        assert_eq!(
            hex::encode(legacy_machine_key("synthetic-host", "synthetic-user")),
            LEGACY_KEY_VECTOR
        );
    }

    /// Final Gate items A and H. Replaces the tests that asserted a new token
    /// was stored (`test_store_and_load_credentials`,
    /// `test_stored_file_does_not_contain_plaintext_token`): nothing new is
    /// stored, and an existing store is left byte for byte as it was.
    #[test]
    fn p0_fg_a_new_deploy_credentials_are_never_stored() {
        let path = temp_cred_path();
        let refused = store_to_path(&path, "netlify", &credentials("netlify", "synthetic-token"));
        assert_eq!(
            refused.unwrap_err().to_string(),
            format!("credential error: {STORAGE_REFUSED}")
        );
        assert!(!path.exists());

        let text = write_legacy_store(
            &path,
            &[credentials("vercel", "synthetic-vercel")],
            &legacy_key(),
        );
        for provider in ["vercel", "netlify"] {
            assert!(store_to_path(&path, provider, &credentials(provider, "new")).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Final Gate items A and H: entries stored before Phase Zero stay
    /// readable through the explicit legacy read path.
    #[test]
    fn p0_fg_a_legacy_deploy_credentials_still_read() {
        let path = temp_cred_path();
        let mut cloudflare = credentials("cloudflare", "synthetic-cf-token");
        cloudflare.account_id = Some("acct-123".into());
        cloudflare.expires_at = Some("2027-01-01T00:00:00Z".into());
        let text = write_legacy_store(
            &path,
            &[credentials("netlify", "synthetic-netlify"), cloudflare],
            &legacy_key(),
        );
        assert!(!text.contains("synthetic-netlify"));
        let loaded = load_from_path_with_key(&path, "cloudflare", &legacy_key())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.token, "synthetic-cf-token");
        assert_eq!(loaded.account_id.as_deref(), Some("acct-123"));
        assert_eq!(loaded.expires_at.as_deref(), Some("2027-01-01T00:00:00Z"));
        assert_eq!(
            load_from_path_with_key(&path, "netlify", &legacy_key())
                .unwrap()
                .unwrap()
                .token,
            "synthetic-netlify"
        );
        assert!(load_from_path_with_key(&path, "vercel", &legacy_key())
            .unwrap()
            .is_none());
        // Reading never rewrites the store.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
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
        write_legacy_store(
            &path,
            &[
                credentials("vercel", "token-abc"),
                credentials("netlify", "token-def"),
            ],
            &legacy_key(),
        );
        assert!(load_from_path_with_key(&path, "vercel", &legacy_key())
            .unwrap()
            .is_some());

        delete_from_path(&path, "vercel").unwrap();
        assert!(load_from_path_with_key(&path, "vercel", &legacy_key())
            .unwrap()
            .is_none());
        assert_eq!(
            load_from_path_with_key(&path, "netlify", &legacy_key())
                .unwrap()
                .unwrap()
                .token,
            "token-def"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Final Gate item A: a store that cannot be parsed is never rewritten, so
    /// deleting one provider cannot drop the others.
    #[test]
    fn p0_fg_a_an_unreadable_store_is_not_rewritten() {
        let path = temp_cred_path();
        std::fs::write(&path, "{ not json").unwrap();
        assert!(delete_from_path(&path, "vercel").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
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
    fn test_xor_obfuscate_roundtrip() {
        let key = machine_key();
        let data = b"hello world secret token";
        let obfuscated = xor_obfuscate(data, &key);
        assert_ne!(&obfuscated, data);
        let restored = xor_obfuscate(&obfuscated, &key);
        assert_eq!(&restored, data);
    }
}
