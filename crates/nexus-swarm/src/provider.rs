//! Provider abstraction shared by all six swarm providers.
//!
//! Separate from the synchronous `nexus_connectors_llm::providers::LlmProvider`
//! trait — this one is async, carries privacy classification, and exposes
//! health-checking + capabilities metadata for the Router.

use crate::events::ProviderHealth;
use crate::profile::{CostClass, PrivacyClass, ReasoningTier};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub param_count_b: Option<u32>,
    pub tier: ReasoningTier,
    pub context_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub models: Vec<ModelDescriptor>,
    pub supports_tool_use: bool,
    pub supports_streaming: bool,
    pub max_context: u32,
    pub cost_class: CostClass,
    /// Hard constraint on what the provider is allowed to touch. Cloud
    /// providers MUST set this to `Public`.
    pub privacy_class: PrivacyClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    pub model_id: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    /// Free-form metadata for the provider — e.g. a governance capability
    /// token issued by the oracle.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    pub text: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: u32,
    pub latency_ms: u64,
    pub model_id: String,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider `{0}` is not configured (missing key/binary)")]
    NotConfigured(String),
    #[error("authentication failed for provider `{0}`")]
    AuthFailed(String),
    #[error("rate limit hit on provider `{provider}` (retry after {retry_after_ms}ms)")]
    RateLimited {
        provider: String,
        retry_after_ms: u64,
    },
    #[error("provider `{0}` timed out after {1}ms")]
    Timeout(String, u64),
    #[error("provider `{0}` returned HTTP {1}: {2}")]
    Http(String, u16, String),
    #[error("provider `{0}` returned malformed response: {1}")]
    Malformed(String, String),
    #[error("model `{model}` is not available on provider `{provider}`")]
    UnknownModel { provider: String, model: String },
    #[error("Anthropic Haiku-only invariant violated: got `{0}`")]
    HaikuOnly(String),
    #[error("Anthropic spend cap exceeded: spent ${spent:.2} / cap ${cap:.2}")]
    SpendCapExceeded { spent: f64, cap: f64 },
    #[error("transport error on provider `{0}`: {1}")]
    Transport(String, String),
    #[error("io error on provider `{0}`: {1}")]
    Io(String, String),
}

/// Live provider interface used by the Router and Coordinator.
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn health_check(&self) -> ProviderHealth;

    async fn invoke(&self, req: InvokeRequest) -> Result<InvokeResponse, ProviderError>;
}
