//! Errors returned by `SwarmAgentEntry::execute`. The `From<ProviderError>`
//! impl lets agent crates use `?` on `ctx.provider.invoke(...)` directly.

use crate::provider::ProviderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("cancelled by user")]
    Cancelled,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("publish failed ({reason}, retryable={retryable})")]
    PublishFailed {
        reason: String,
        retryable: bool,
        retry_after_secs: Option<u64>,
    },
}
