use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::provider::ProviderRegistry;
use super::types::{LlmRequest, LlmResponse, StreamChunk};
use crate::error::NxError;

/// Named slots for routing different types of LLM requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelSlot {
    /// Primary coding model.
    Execution,
    /// Planning/reasoning model.
    Thinking,
    /// Self-verification model.
    Critique,
    /// Summarization/compaction model.
    Compact,
    /// Multimodal model.
    Vision,
}

impl std::fmt::Display for ModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ModelSlot::Execution => "execution",
            ModelSlot::Thinking => "thinking",
            ModelSlot::Critique => "critique",
            ModelSlot::Compact => "compact",
            ModelSlot::Vision => "vision",
        };
        write!(f, "{}", s)
    }
}

/// Configuration for a model slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotConfig {
    /// Provider name (e.g., "anthropic").
    pub provider: String,
    /// Model name (e.g., "claude-sonnet-4-20250514").
    pub model: String,
}

/// Routes LLM requests through named model slots.
pub struct ModelRouter {
    slots: HashMap<ModelSlot, SlotConfig>,
    registry: Arc<ProviderRegistry>,
}

impl ModelRouter {
    /// Create a new model router.
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self {
            slots: HashMap::new(),
            registry,
        }
    }

    /// Configure a slot.
    pub fn set_slot(&mut self, slot: ModelSlot, config: SlotConfig) {
        self.slots.insert(slot, config);
    }

    /// Get the slot configuration.
    pub fn get_slot(&self, slot: ModelSlot) -> Option<&SlotConfig> {
        self.slots.get(&slot)
    }

    /// Resolve a slot to its provider and model.
    pub fn resolve(
        &self,
        slot: ModelSlot,
    ) -> Result<(&dyn super::provider::LlmProvider, &str), NxError> {
        let config = self
            .slots
            .get(&slot)
            .ok_or_else(|| NxError::NoProviderConfigured {
                slot: slot.to_string(),
            })?;
        let provider =
            self.registry
                .get(&config.provider)
                .ok_or_else(|| NxError::NoProviderConfigured {
                    slot: format!("{} (provider '{}' not registered)", slot, config.provider),
                })?;
        Ok((provider, &config.model))
    }

    /// Send a request through a specific slot.
    pub async fn complete(
        &self,
        slot: ModelSlot,
        request: &LlmRequest,
    ) -> Result<LlmResponse, NxError> {
        let (provider, model) = self.resolve(slot)?;
        let mut req = request.clone();
        req.model = model.to_string();
        provider.complete(&req).await
    }

    /// Stream through a specific slot.
    pub async fn stream(
        &self,
        slot: ModelSlot,
        request: &LlmRequest,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), NxError> {
        let (provider, model) = self.resolve(slot)?;
        let mut req = request.clone();
        req.model = model.to_string();
        provider.stream(&req, tx).await
    }

    /// Get a raw streaming response through a specific slot.
    /// Returns None if the provider doesn't support raw streaming.
    pub async fn stream_raw(
        &self,
        slot: ModelSlot,
        request: &LlmRequest,
    ) -> Result<Option<reqwest::Response>, NxError> {
        let (provider, model) = self.resolve(slot)?;
        let mut req = request.clone();
        req.model = model.to_string();
        req.stream = true;
        provider.stream_raw(&req).await
    }
}
