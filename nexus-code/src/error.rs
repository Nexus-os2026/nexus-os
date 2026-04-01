use std::fmt;

/// Unified error type for Nexus Code.
#[derive(Debug)]
pub enum NxError {
    // Governance
    /// A required capability was not granted.
    CapabilityDenied { capability: String, reason: String },
    /// User denied consent for an action.
    ConsentDenied { action: String },
    /// Consent is required before this action can proceed.
    ConsentRequired {
        request: crate::governance::consent::ConsentRequest,
    },
    /// Fuel budget exhausted.
    FuelExhausted { remaining: u64, required: u64 },
    /// Audit trail integrity violation detected.
    AuditIntegrityViolation {
        expected_hash: String,
        actual_hash: String,
    },
    /// Ed25519 identity error.
    IdentityError(String),

    // LLM
    /// LLM provider returned an error.
    ProviderError { provider: String, message: String },
    /// Error during SSE streaming.
    StreamingError(String),
    /// No provider configured for the requested slot.
    NoProviderConfigured { slot: String },

    // Config
    /// Configuration error.
    ConfigError(String),

    // IO
    /// IO error.
    Io(std::io::Error),
    /// JSON serialization/deserialization error.
    SerdeJson(serde_json::Error),
    /// HTTP request error.
    Http(reqwest::Error),
}

impl fmt::Display for NxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityDenied { capability, reason } => {
                write!(f, "Capability denied: {} — {}", capability, reason)
            }
            Self::ConsentDenied { action } => {
                write!(f, "Consent denied for action: {}", action)
            }
            Self::ConsentRequired { request } => {
                write!(
                    f,
                    "Consent required for: {} (Tier {:?})",
                    request.action, request.tier
                )
            }
            Self::FuelExhausted {
                remaining,
                required,
            } => write!(
                f,
                "Fuel exhausted: {} remaining, {} required",
                remaining, required
            ),
            Self::AuditIntegrityViolation {
                expected_hash,
                actual_hash,
            } => write!(
                f,
                "Audit integrity violation: expected {}, got {}",
                expected_hash, actual_hash
            ),
            Self::IdentityError(msg) => write!(f, "Identity error: {}", msg),
            Self::ProviderError { provider, message } => {
                write!(f, "Provider error ({}): {}", provider, message)
            }
            Self::StreamingError(msg) => write!(f, "Streaming error: {}", msg),
            Self::NoProviderConfigured { slot } => {
                write!(f, "No provider configured for slot: {}", slot)
            }
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::SerdeJson(err) => write!(f, "JSON error: {}", err),
            Self::Http(err) => write!(f, "HTTP error: {}", err),
        }
    }
}

impl std::error::Error for NxError {}

impl From<std::io::Error> for NxError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for NxError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerdeJson(err)
    }
}

impl From<reqwest::Error> for NxError {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err)
    }
}
