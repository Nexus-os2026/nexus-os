//! Error types for the integration subsystem.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum IntegrationError {
    #[error("provider '{provider}' not configured")]
    NotConfigured { provider: String },

    #[error("provider '{provider}' is disabled")]
    Disabled { provider: String },

    #[error("rate limited on '{provider}': retry after {retry_after_ms} ms")]
    RateLimited {
        provider: String,
        retry_after_ms: u64,
    },

    #[error("HTTP request failed for '{provider}': {status} — {body}")]
    HttpError {
        provider: String,
        status: u16,
        body: String,
    },

    #[error("connection error for '{provider}': {message}")]
    ConnectionError { provider: String, message: String },

    #[error("authentication failed for '{provider}': {message}")]
    AuthError { provider: String, message: String },

    #[error("missing credential: {env_var}")]
    MissingCredential { env_var: String },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("agent lacks capability '{capability}' for integration '{provider}'")]
    CapabilityDenied {
        provider: String,
        capability: String,
    },

    #[error("webhook delivery failed after {attempts} attempts: {message}")]
    WebhookDeliveryFailed { attempts: u32, message: String },

    #[error("HITL denied integration send via '{provider}': {detail}")]
    HitlDenied { provider: String, detail: String },
}
