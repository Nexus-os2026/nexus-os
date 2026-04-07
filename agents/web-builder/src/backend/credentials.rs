//! Supabase Credential Storage — encrypted at rest, machine-bound.
//!
//! Uses the same XOR obfuscation + machine-specific key pattern as deploy credentials.
//! Service role key is the most sensitive credential — never logged, never in manifests.

use crate::deploy::{credentials as deploy_creds, Credentials as DeployCreds, DeployError};
use serde::{Deserialize, Serialize};

/// Supabase project credentials.
#[derive(Clone, Serialize, Deserialize)]
pub struct SupabaseCredentials {
    pub project_url: String,
    pub anon_key: String,
    pub service_role_key: Option<String>,
}

/// Custom Debug that redacts keys.
impl std::fmt::Debug for SupabaseCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupabaseCredentials")
            .field("project_url", &self.project_url)
            .field("anon_key", &"***REDACTED***")
            .field("service_role_key", &"***REDACTED***")
            .finish()
    }
}

const PROVIDER_KEY: &str = "supabase";

/// Store Supabase credentials using the deploy credential encryption.
pub fn store_supabase_credentials(creds: &SupabaseCredentials) -> Result<(), DeployError> {
    // Store as a deploy Credentials with project_url in account_id field
    let deploy_cred = DeployCreds {
        provider: PROVIDER_KEY.into(),
        token: serde_json::to_string(creds).map_err(|e| DeployError::Credential(e.to_string()))?,
        account_id: Some(creds.project_url.clone()),
        expires_at: None,
    };
    deploy_creds::store_credentials(PROVIDER_KEY, &deploy_cred)
}

/// Load Supabase credentials.
pub fn load_supabase_credentials() -> Result<Option<SupabaseCredentials>, DeployError> {
    match deploy_creds::load_credentials(PROVIDER_KEY)? {
        Some(cred) => {
            let parsed: SupabaseCredentials = serde_json::from_str(&cred.token)
                .map_err(|e| DeployError::Credential(format!("parse failed: {e}")))?;
            Ok(Some(parsed))
        }
        None => Ok(None),
    }
}

/// Check if Supabase credentials are stored.
pub fn has_supabase_credentials() -> bool {
    deploy_creds::has_credentials(PROVIDER_KEY)
}

/// Delete stored Supabase credentials.
pub fn delete_supabase_credentials() -> Result<(), DeployError> {
    deploy_creds::delete_credentials(PROVIDER_KEY)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_redacts_keys() {
        let creds = SupabaseCredentials {
            project_url: "https://example.supabase.co".into(),
            anon_key: "super-secret-anon-key".into(),
            service_role_key: Some("super-secret-service-key".into()),
        };
        let debug_output = format!("{creds:?}");
        assert!(!debug_output.contains("super-secret"));
        assert!(debug_output.contains("REDACTED"));
        assert!(debug_output.contains("example.supabase.co"));
    }

    #[test]
    fn test_credentials_serialize_roundtrip() {
        let creds = SupabaseCredentials {
            project_url: "https://test.supabase.co".into(),
            anon_key: "anon-key-123".into(),
            service_role_key: Some("service-key-456".into()),
        };
        let json = serde_json::to_string(&creds).unwrap();
        let parsed: SupabaseCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_url, creds.project_url);
        assert_eq!(parsed.anon_key, creds.anon_key);
        assert_eq!(parsed.service_role_key, creds.service_role_key);
    }

    #[test]
    fn test_credentials_without_service_key() {
        let creds = SupabaseCredentials {
            project_url: "https://test.supabase.co".into(),
            anon_key: "anon-key".into(),
            service_role_key: None,
        };
        let json = serde_json::to_string(&creds).unwrap();
        let parsed: SupabaseCredentials = serde_json::from_str(&json).unwrap();
        assert!(parsed.service_role_key.is_none());
    }
}
