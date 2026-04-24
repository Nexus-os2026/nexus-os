//! Shared swarm execution surface.
//!
//! Extracted from `nexus-swarm` so agent crates (coder, social-poster,
//! collaboration) can implement `SwarmAgentEntry` without a circular
//! dependency on the orchestration crate. `nexus-swarm` re-exports
//! everything in this crate so existing call sites stay unchanged.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod cancel;
pub mod context;
pub mod emitter;
pub mod entry;
pub mod error;
pub mod profile_classes;
pub mod provider;
pub mod provider_health;

pub use cancel::CancelToken;
pub use context::{AgentExecutionContext, NodeBudget};
pub use emitter::EventEmitter;
pub use entry::SwarmAgentEntry;
pub use error::AgentError;
pub use profile_classes::{CostClass, PrivacyClass, ReasoningTier};
pub use provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
pub use provider_health::{ProviderHealth, ProviderHealthStatus};
