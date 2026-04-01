//! LLM provider abstraction, streaming, and multi-slot routing.

pub mod provider;
pub mod providers;
pub mod router;
pub mod streaming;
pub mod types;

pub use provider::{LlmProvider, ProviderConfig, ProviderRegistry};
pub use router::{ModelRouter, ModelSlot, SlotConfig};
pub use types::{LlmRequest, LlmResponse, Message, Role, StreamChunk, TokenUsage};
