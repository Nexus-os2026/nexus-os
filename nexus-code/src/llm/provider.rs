use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::types::{LlmRequest, LlmResponse, StreamChunk};
use crate::error::NxError;

/// Configuration for an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name (e.g., "anthropic", "openai").
    pub name: String,
    /// Environment variable name for the API key.
    pub api_key_env: String,
    /// API base URL.
    pub base_url: String,
    /// Default model for this provider.
    pub default_model: String,
}

/// Trait implemented by all LLM providers.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g., "anthropic").
    fn name(&self) -> &str;

    /// Send a non-streaming request.
    async fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, NxError>;

    /// Send a streaming request. Chunks are sent via the mpsc channel.
    async fn stream(
        &self,
        request: &LlmRequest,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), NxError>;

    /// List available models for this provider.
    fn available_models(&self) -> Vec<&str>;

    /// Check if the provider is configured (API key available).
    fn is_configured(&self) -> bool;

    /// Return the raw HTTP response for streaming tool-call collection.
    /// Default: returns None (provider doesn't support raw streaming).
    /// Override to enable streaming tool detection in the agent loop.
    async fn stream_raw(
        &self,
        _request: &super::types::LlmRequest,
    ) -> Result<Option<reqwest::Response>, NxError> {
        Ok(None)
    }
}

/// Registry of all LLM providers.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn LlmProvider>>,
}

impl ProviderRegistry {
    /// Create an empty provider registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn LlmProvider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub fn get(&self, name: &str) -> Option<&dyn LlmProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// List all registered provider names.
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    /// List only configured (API key available) provider names.
    pub fn configured(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|(_, p)| p.is_configured())
            .map(|(k, _)| k.as_str())
            .collect()
    }
}
