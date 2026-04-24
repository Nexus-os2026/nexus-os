//! AgentExecutionContext — the bundle every adapter invocation receives.
//!
//! Carries the identity of the node (run + node + ticket), the resolved
//! provider handle, a shared event emitter, a cooperative cancel token,
//! and a per-node budget tracker. Adapters emit phase events through
//! `ctx.emit`; the emitter auto-tags `ticket_nonce` and `node_id` so
//! adapter code stays compact.

use crate::cancel::CancelToken;
use crate::emitter::EventEmitter;
use crate::provider::Provider;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Per-node budget accounting. Saturating arithmetic — the coordinator
/// hands a fresh `NodeBudget` per node task, so overflow only matters
/// under genuinely adversarial token counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeBudget {
    pub tokens_consumed: u64,
    pub cost_cents: u32,
}

impl NodeBudget {
    pub const fn new() -> Self {
        Self {
            tokens_consumed: 0,
            cost_cents: 0,
        }
    }

    pub fn record(&mut self, tokens: u64, cost_cents: u32) {
        self.tokens_consumed = self.tokens_consumed.saturating_add(tokens);
        self.cost_cents = self.cost_cents.saturating_add(cost_cents);
    }
}

/// Context passed into every adapter invocation. Cheap to clone (all
/// heap state is behind `Arc`).
#[derive(Clone)]
pub struct AgentExecutionContext {
    pub ticket_nonce: Uuid,
    pub run_id: Uuid,
    pub node_id: String,
    pub capability_id: String,
    pub provider: Arc<dyn Provider>,
    pub model_id: String,
    pub emit: Arc<dyn EventEmitter>,
    pub cancel: CancelToken,
    pub budget: Arc<Mutex<NodeBudget>>,
}

impl AgentExecutionContext {
    /// Short-circuit helper — adapters call `ctx.cancelled()` between
    /// phases rather than re-reading the cancel field.
    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_budget_record_accumulates() {
        let mut b = NodeBudget::new();
        b.record(100, 5);
        b.record(200, 10);
        assert_eq!(b.tokens_consumed, 300);
        assert_eq!(b.cost_cents, 15);
    }

    #[test]
    fn node_budget_record_saturates() {
        let mut b = NodeBudget {
            tokens_consumed: u64::MAX - 10,
            cost_cents: u32::MAX - 2,
        };
        b.record(100, 100);
        assert_eq!(b.tokens_consumed, u64::MAX);
        assert_eq!(b.cost_cents, u32::MAX);
    }
}
