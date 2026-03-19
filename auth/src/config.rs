//! Authentication configuration.

use crate::roles::UserRole;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication provider type.
    pub provider: AuthProvider,
    /// OIDC issuer URL (e.g. `https://keycloak.example.com/realms/nexus`).
    pub issuer_url: String,
    /// OAuth2 client ID registered with the IdP.
    pub client_id: String,
    /// Client secret — loaded from env var in production.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Environment variable name for client secret (preferred over inline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_env: Option<String>,
    /// OAuth2 redirect URI for the authorization code callback.
    pub redirect_uri: String,
    /// Requested OIDC scopes.
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    /// Maximum session duration in hours.
    #[serde(default = "default_session_hours")]
    pub session_duration_hours: u64,
    /// Mapping from IdP group/role names to Nexus UserRole.
    #[serde(default)]
    pub role_mapping: HashMap<String, UserRole>,
    /// OIDC claim that contains the user's roles/groups.
    #[serde(default = "default_roles_claim")]
    pub roles_claim: String,
}

/// Authentication provider type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthProvider {
    /// OpenID Connect (Keycloak, Azure AD, Okta, Google).
    Oidc,
    /// SAML 2.0 (enterprise federation).
    Saml,
    /// Local desktop mode — OS-level user identity, no external IdP.
    Local,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "profile".to_string(),
        "email".to_string(),
    ]
}

fn default_session_hours() -> u64 {
    8
}

fn default_roles_claim() -> String {
    "roles".to_string()
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            provider: AuthProvider::Local,
            issuer_url: String::new(),
            client_id: String::new(),
            client_secret: None,
            client_secret_env: None,
            redirect_uri: "http://localhost:1420/auth/callback".to_string(),
            scopes: default_scopes(),
            session_duration_hours: default_session_hours(),
            role_mapping: HashMap::new(),
            roles_claim: default_roles_claim(),
        }
    }
}

impl AuthConfig {
    /// Resolve the client secret from environment variable if configured.
    pub fn resolve_client_secret(&self) -> Option<String> {
        if let Some(ref secret) = self.client_secret {
            return Some(secret.clone());
        }
        if let Some(ref env_var) = self.client_secret_env {
            return std::env::var(env_var).ok();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_local() {
        let cfg = AuthConfig::default();
        assert_eq!(cfg.provider, AuthProvider::Local);
        assert_eq!(cfg.session_duration_hours, 8);
        assert_eq!(cfg.scopes.len(), 3);
    }

    #[test]
    fn resolve_inline_secret() {
        let cfg = AuthConfig {
            client_secret: Some("inline-secret".to_string()),
            ..Default::default()
        };
        assert_eq!(
            cfg.resolve_client_secret(),
            Some("inline-secret".to_string())
        );
    }

    #[test]
    fn resolve_env_secret() {
        std::env::set_var("TEST_NEXUS_OIDC_SECRET", "env-secret");
        let cfg = AuthConfig {
            client_secret_env: Some("TEST_NEXUS_OIDC_SECRET".to_string()),
            ..Default::default()
        };
        assert_eq!(cfg.resolve_client_secret(), Some("env-secret".to_string()));
        std::env::remove_var("TEST_NEXUS_OIDC_SECRET");
    }

    #[test]
    fn serde_roundtrip() {
        let mut mapping = std::collections::HashMap::new();
        mapping.insert("nexus-admin".to_string(), UserRole::Admin);
        let cfg = AuthConfig {
            provider: AuthProvider::Oidc,
            issuer_url: "https://idp.example.com".to_string(),
            client_id: "nexus-os".to_string(),
            role_mapping: mapping,
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: AuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, AuthProvider::Oidc);
        assert_eq!(parsed.role_mapping.len(), 1);
    }
}
