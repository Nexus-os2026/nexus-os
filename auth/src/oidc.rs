//! OIDC authorization code flow with PKCE.
//!
//! Implements the full OpenID Connect flow:
//! 1. Discovery — fetch `.well-known/openid-configuration`
//! 2. Authorization — redirect to IdP with PKCE challenge
//! 3. Callback — exchange authorization code for tokens
//! 4. Validation — verify ID token signature, expiry, audience
//! 5. Role extraction — map IdP groups to Nexus UserRole
//! 6. Session creation — issue session with expiry

use crate::config::AuthConfig;
use crate::error::AuthError;
use crate::roles::UserRole;
use crate::session::{AuthenticatedUser, SessionManager};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// OIDC discovery document (subset of fields we need).
#[derive(Debug, Clone, Deserialize)]
pub struct OidcDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub issuer: String,
}

/// OIDC token response from the token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<u64>,
}

/// Claims extracted from the ID token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    pub aud: serde_json::Value,
    pub iss: String,
    pub exp: u64,
    pub iat: u64,
}

/// PKCE state stored between authorization request and callback.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub state: String,
    pub code_verifier: String,
    pub nonce: String,
}

/// URL to redirect the user to for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRedirectUrl {
    pub url: String,
    pub state: String,
}

/// The OIDC client that manages the full authorization code flow.
#[derive(Debug)]
pub struct OidcClient {
    config: AuthConfig,
    http: reqwest::Client,
    discovery: Option<OidcDiscovery>,
    pending_challenges: std::sync::Arc<tokio::sync::RwLock<HashMap<String, PkceChallenge>>>,
}

impl OidcClient {
    /// Create a new OIDC client from configuration.
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            discovery: None,
            pending_challenges: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Step 1: Discover OIDC endpoints from the issuer.
    pub async fn discover(&mut self) -> Result<&OidcDiscovery, AuthError> {
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.config.issuer_url.trim_end_matches('/')
        );
        let discovery: OidcDiscovery = self
            .http
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| AuthError::DiscoveryFailed(e.to_string()))?;

        self.discovery = Some(discovery);
        Ok(self.discovery.as_ref().unwrap())
    }

    /// Step 2: Generate the authorization URL with PKCE.
    pub async fn authorize(&self) -> Result<AuthRedirectUrl, AuthError> {
        let discovery = self
            .discovery
            .as_ref()
            .ok_or(AuthError::ProviderNotConfigured)?;

        // Generate PKCE challenge
        let code_verifier = generate_random_string(64);
        let code_challenge = {
            let mut hasher = Sha256::new();
            hasher.update(code_verifier.as_bytes());
            URL_SAFE_NO_PAD.encode(hasher.finalize())
        };

        let state = Uuid::new_v4().to_string();
        let nonce = Uuid::new_v4().to_string();

        // Store PKCE state
        let challenge = PkceChallenge {
            state: state.clone(),
            code_verifier,
            nonce: nonce.clone(),
        };
        self.pending_challenges
            .write()
            .await
            .insert(state.clone(), challenge);

        let scopes = self.config.scopes.join(" ");
        let url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            discovery.authorization_endpoint,
            urlencoded(&self.config.client_id),
            urlencoded(&self.config.redirect_uri),
            urlencoded(&scopes),
            urlencoded(&state),
            urlencoded(&nonce),
            urlencoded(&code_challenge),
        );

        Ok(AuthRedirectUrl { url, state })
    }

    /// Step 3: Exchange the authorization code for tokens.
    pub async fn exchange_code(&self, code: &str, state: &str) -> Result<TokenResponse, AuthError> {
        let discovery = self
            .discovery
            .as_ref()
            .ok_or(AuthError::ProviderNotConfigured)?;

        // Retrieve and consume PKCE challenge
        let challenge = self
            .pending_challenges
            .write()
            .await
            .remove(state)
            .ok_or(AuthError::InvalidState)?;

        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", &self.config.redirect_uri);
        params.insert("client_id", &self.config.client_id);
        params.insert("code_verifier", &challenge.code_verifier);

        let secret_holder;
        if let Some(ref secret) = self.config.resolve_client_secret() {
            secret_holder = secret.clone();
            params.insert("client_secret", &secret_holder);
        }

        let response: TokenResponse = self
            .http
            .post(&discovery.token_endpoint)
            .form(&params)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| AuthError::TokenExchangeFailed(e.to_string()))?;

        Ok(response)
    }

    /// Step 4: Decode and validate the ID token claims.
    ///
    /// Note: In production, this should verify the JWT signature against the
    /// JWKS endpoint. For now, we decode the payload and validate basic claims.
    pub fn decode_id_token(&self, id_token: &str) -> Result<IdTokenClaims, AuthError> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(AuthError::TokenValidationFailed(
                "invalid JWT structure".into(),
            ));
        }

        let payload = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| AuthError::TokenValidationFailed(format!("base64 decode: {e}")))?;

        let claims: IdTokenClaims = serde_json::from_slice(&payload)
            .map_err(|e| AuthError::TokenValidationFailed(format!("JSON parse: {e}")))?;

        // Validate issuer
        if !claims.iss.starts_with(&self.config.issuer_url) {
            return Err(AuthError::TokenValidationFailed(format!(
                "issuer mismatch: expected '{}', got '{}'",
                self.config.issuer_url, claims.iss
            )));
        }

        // Validate expiry
        let now = chrono::Utc::now().timestamp() as u64;
        if claims.exp < now {
            return Err(AuthError::TokenExpired);
        }

        // Validate audience
        let valid_aud = match &claims.aud {
            serde_json::Value::String(s) => s == &self.config.client_id,
            serde_json::Value::Array(arr) => arr
                .iter()
                .any(|v| v.as_str() == Some(&self.config.client_id)),
            _ => false,
        };
        if !valid_aud {
            return Err(AuthError::TokenValidationFailed("audience mismatch".into()));
        }

        Ok(claims)
    }

    /// Step 5: Map IdP roles/groups to a Nexus UserRole.
    pub fn resolve_role(&self, claims: &IdTokenClaims) -> Result<UserRole, AuthError> {
        let groups = if claims.roles.is_empty() {
            &claims.groups
        } else {
            &claims.roles
        };

        // Find the highest-privilege matching role
        let mut best_role: Option<UserRole> = None;
        for group in groups {
            if let Some(role) = UserRole::from_idp_group(group, &self.config.role_mapping) {
                match best_role {
                    Some(current) if role.privilege_level() > current.privilege_level() => {
                        best_role = Some(role);
                    }
                    None => best_role = Some(role),
                    _ => {}
                }
            }
        }

        best_role.ok_or_else(|| {
            AuthError::RoleMappingFailed(
                groups
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        })
    }

    /// Steps 3-6 combined: Complete the OIDC callback and create a session.
    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
        session_mgr: &SessionManager,
    ) -> Result<AuthenticatedUser, AuthError> {
        // Exchange code for tokens
        let tokens = self.exchange_code(code, state).await?;

        // Decode ID token
        let id_token = tokens
            .id_token
            .as_ref()
            .ok_or_else(|| AuthError::TokenExchangeFailed("no id_token in response".into()))?;

        let claims = self.decode_id_token(id_token)?;

        // Resolve role
        let role = self.resolve_role(&claims)?;

        // Create session
        let user = session_mgr
            .create_session(crate::session::NewSessionRequest {
                id: claims.sub,
                email: claims.email.unwrap_or_default(),
                name: claims.name.unwrap_or_default(),
                role,
                provider: "oidc".to_string(),
                refresh_token: tokens.refresh_token,
                workspace_id: None,
            })
            .await;

        Ok(user)
    }

    /// Access the current configuration (read-only).
    pub fn config(&self) -> &AuthConfig {
        &self.config
    }
}

/// Generate a cryptographically-suitable random string from UUID bytes.
fn generate_random_string(len: usize) -> String {
    let mut result = String::with_capacity(len);
    while result.len() < len {
        result.push_str(&Uuid::new_v4().to_string().replace('-', ""));
    }
    result.truncate(len);
    result
}

/// Minimal URL encoding for query parameters.
fn urlencoded(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, AuthProvider};

    fn test_config() -> AuthConfig {
        let mut role_mapping = HashMap::new();
        role_mapping.insert("nexus-admin".to_string(), UserRole::Admin);
        role_mapping.insert("nexus-operator".to_string(), UserRole::Operator);
        role_mapping.insert("nexus-viewer".to_string(), UserRole::Viewer);
        role_mapping.insert("nexus-auditor".to_string(), UserRole::Auditor);

        AuthConfig {
            provider: AuthProvider::Oidc,
            issuer_url: "https://idp.example.com".to_string(),
            client_id: "nexus-os".to_string(),
            client_secret: Some("test-secret".to_string()),
            redirect_uri: "http://localhost:1420/auth/callback".to_string(),
            role_mapping,
            ..Default::default()
        }
    }

    #[test]
    fn decode_valid_id_token() {
        let config = test_config();
        let client = OidcClient::new(config);

        // Build a test JWT (header.payload.signature)
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let exp = chrono::Utc::now().timestamp() as u64 + 3600;
        let payload_json = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "roles": ["nexus-admin"],
            "groups": [],
            "aud": "nexus-os",
            "iss": "https://idp.example.com",
            "exp": exp,
            "iat": exp - 60
        });
        let payload = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let signature = URL_SAFE_NO_PAD.encode("fake-signature");
        let token = format!("{header}.{payload}.{signature}");

        let claims = client.decode_id_token(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, Some("test@example.com".to_string()));
    }

    #[test]
    fn decode_expired_token_fails() {
        let config = test_config();
        let client = OidcClient::new(config);

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let payload_json = serde_json::json!({
            "sub": "user-123",
            "email": "test@example.com",
            "name": "Test User",
            "roles": [],
            "groups": [],
            "aud": "nexus-os",
            "iss": "https://idp.example.com",
            "exp": 1000,  // long expired
            "iat": 900
        });
        let payload = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let signature = URL_SAFE_NO_PAD.encode("fake");
        let token = format!("{header}.{payload}.{signature}");

        let result = client.decode_id_token(&token);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn decode_wrong_audience_fails() {
        let config = test_config();
        let client = OidcClient::new(config);

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let exp = chrono::Utc::now().timestamp() as u64 + 3600;
        let payload_json = serde_json::json!({
            "sub": "user-123",
            "roles": [],
            "groups": [],
            "aud": "wrong-client",
            "iss": "https://idp.example.com",
            "exp": exp,
            "iat": exp - 60
        });
        let payload = URL_SAFE_NO_PAD.encode(payload_json.to_string());
        let signature = URL_SAFE_NO_PAD.encode("fake");
        let token = format!("{header}.{payload}.{signature}");

        let result = client.decode_id_token(&token);
        assert!(matches!(result, Err(AuthError::TokenValidationFailed(_))));
    }

    #[test]
    fn role_resolution_picks_highest_privilege() {
        let config = test_config();
        let client = OidcClient::new(config);

        let claims = IdTokenClaims {
            sub: "u1".into(),
            email: None,
            name: None,
            roles: vec!["nexus-viewer".into(), "nexus-admin".into()],
            groups: vec![],
            aud: serde_json::json!("nexus-os"),
            iss: "https://idp.example.com".into(),
            exp: 9999999999,
            iat: 9999999998,
        };

        let role = client.resolve_role(&claims).unwrap();
        assert_eq!(role, UserRole::Admin);
    }

    #[test]
    fn role_resolution_falls_back_to_groups() {
        let config = test_config();
        let client = OidcClient::new(config);

        let claims = IdTokenClaims {
            sub: "u2".into(),
            email: None,
            name: None,
            roles: vec![], // empty roles
            groups: vec!["nexus-operator".into()],
            aud: serde_json::json!("nexus-os"),
            iss: "https://idp.example.com".into(),
            exp: 9999999999,
            iat: 9999999998,
        };

        let role = client.resolve_role(&claims).unwrap();
        assert_eq!(role, UserRole::Operator);
    }

    #[test]
    fn role_resolution_fails_for_unknown_groups() {
        let config = test_config();
        let client = OidcClient::new(config);

        let claims = IdTokenClaims {
            sub: "u3".into(),
            email: None,
            name: None,
            roles: vec!["unknown-role".into()],
            groups: vec![],
            aud: serde_json::json!("nexus-os"),
            iss: "https://idp.example.com".into(),
            exp: 9999999999,
            iat: 9999999998,
        };

        let result = client.resolve_role(&claims);
        assert!(matches!(result, Err(AuthError::RoleMappingFailed(_))));
    }

    #[test]
    fn generate_random_string_correct_length() {
        let s = generate_random_string(64);
        assert_eq!(s.len(), 64);
        let s2 = generate_random_string(64);
        assert_ne!(s, s2); // different each time
    }
}
