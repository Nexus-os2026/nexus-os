//! SwarmAgentEntry — the contract Phase 4b implements.
//!
//! Phase 4a (this phase) wires `AgentExecutionContext` and phase emission
//! into the three existing adapters (Artisan / Herald / Broker), which
//! still call the resolved provider directly with a prompt-flavoured
//! request. Phase 4b will give `coder-agent`, `social-poster-agent`, and
//! `nexus-collaboration` public implementations of this trait, and the
//! adapters will dispatch to `crate.execute(input, ctx)` instead of
//! running their own prompt shell. The emitters fire from inside the
//! agent crate once that's wired.
//!
//! This file defines the trait + `AgentError` so Phase 4b has a target.
//! No implementations live here — intentional. Adding an implementation
//! is Phase 4b's first commit.

use crate::context::AgentExecutionContext;
use crate::provider::ProviderError;
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::capability::AgentCapabilityDescriptor;

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
}

/// Phase 4b target. Agent crates implement this to plug into the swarm
/// coordinator without going through the prompt-shell adapters. The
/// adapter in Phase 4b becomes a thin `impl SwarmCapability` that
/// forwards `run_with_context(invocation, ctx)` to
/// `self.entry.execute(invocation.inputs, ctx)`.
#[async_trait]
pub trait SwarmAgentEntry: Send + Sync {
    /// Execute one swarm-scheduled invocation. Emits phase events through
    /// `ctx.emit`, checks `ctx.cancel.is_cancelled()` between phases,
    /// records per-node tokens through `ctx.budget`.
    async fn execute(&self, input: Value, ctx: &AgentExecutionContext)
        -> Result<Value, AgentError>;

    /// Same descriptor shape the adapters return today. When a Phase 4b
    /// adapter wraps a crate impl, it delegates to this.
    fn descriptor(&self) -> AgentCapabilityDescriptor;
}
