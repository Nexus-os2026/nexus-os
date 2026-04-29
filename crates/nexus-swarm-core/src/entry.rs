//! `SwarmAgentEntry` — the contract Phase 4b agent crates implement.
//!
//! Phase 4a wired `AgentExecutionContext` and phase emission into the
//! three existing adapters (Artisan / Herald / Broker), which still call
//! the resolved provider directly with a prompt-flavoured request. Phase
//! 4b gives `coder-agent` and `social-poster-agent` public types
//! implementing this trait, and the adapters dispatch to
//! `crate.execute(input, ctx)` instead of running their own prompt shell.
//!
//! `descriptor()` deliberately stays out of the trait: the swarm-side
//! adapter (`SwarmCapability` impl) holds the descriptor for the
//! registry. Agent crates only own the execution surface.

use crate::context::AgentExecutionContext;
use crate::error::AgentError;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait SwarmAgentEntry: Send + Sync {
    /// Execute one swarm-scheduled invocation. Emits phase events through
    /// `ctx.emit`, checks `ctx.cancelled()` between phases, records
    /// per-node tokens through `ctx.budget`.
    async fn execute(&self, input: Value, ctx: &AgentExecutionContext)
        -> Result<Value, AgentError>;
}
