//! Authentication error types.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("OIDC discovery failed: {0}")]
    DiscoveryFailed(String),

    #[error("token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("token validation failed: {0}")]
    TokenValidationFailed(String),

    #[error("token expired")]
    TokenExpired,

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session expired")]
    SessionExpired,

    #[error("insufficient role: required {required:?}, found {found:?}")]
    InsufficientRole {
        required: crate::roles::UserRole,
        found: crate::roles::UserRole,
    },

    #[error("no authenticated user")]
    NotAuthenticated,

    #[error("PKCE verification failed")]
    PkceVerificationFailed,

    #[error("invalid state parameter")]
    InvalidState,

    #[error("provider not configured")]
    ProviderNotConfigured,

    #[error("role mapping failed: unknown IdP group '{0}'")]
    RoleMappingFailed(String),

    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("HTTP error: {0}")]
    Http(String),
}

impl From<reqwest::Error> for AuthError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.to_string())
    }
}
