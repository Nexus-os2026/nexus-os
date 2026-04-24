//! EventEmitter — the abstraction over `SwarmEvent` emission that
//! adapters use. In production `CoordinatorEmitter` wraps the shared
//! `broadcast::Sender` and auto-tags every emission with the context's
//! `ticket_nonce`, `run_id`, and `node_id`. In tests, a recording emitter
//! captures every call for assertion.
//!
//! Splitting emission behind a trait keeps the adapter code testable
//! without spinning up the full coordinator.

use crate::context::NodeBudget;
use crate::events::{NodeRef, SwarmEvent};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Adapter-facing emitter. Phase names ("plan" / "act" / "observe") are
/// convention, not enforced — the trait just hands bytes through.
#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit a `NodeEvent` with the given phase + payload. Called by
    /// adapters at semantically-meaningful boundaries.
    async fn emit_phase(&self, phase: &str, payload: Value);

    /// Emit a per-node `BudgetUpdate`. The emitter fills in `node_id`
    /// from the context it was bound to; callers only supply the
    /// delta accounting.
    async fn emit_budget_update(&self, delta: NodeBudget);
}

/// Production emitter that publishes through the coordinator's
/// `broadcast::Sender`. Automatically tags events with the bound
/// `ticket_nonce` / `run_id` / `node_id`.
pub struct CoordinatorEmitter {
    sender: broadcast::Sender<SwarmEvent>,
    ticket_nonce: Uuid,
    run_id: Uuid,
    node_id: String,
}

impl CoordinatorEmitter {
    pub fn new(
        sender: broadcast::Sender<SwarmEvent>,
        ticket_nonce: Uuid,
        run_id: Uuid,
        node_id: String,
    ) -> Self {
        Self {
            sender,
            ticket_nonce,
            run_id,
            node_id,
        }
    }

    fn node_ref(&self) -> NodeRef {
        NodeRef {
            run_id: self.run_id,
            node_id: self.node_id.clone(),
        }
    }
}

#[async_trait]
impl EventEmitter for CoordinatorEmitter {
    async fn emit_phase(&self, phase: &str, payload: Value) {
        let _ = self.sender.send(SwarmEvent::NodeEvent {
            r#ref: self.node_ref(),
            phase: phase.to_string(),
            payload,
            ticket_nonce: self.ticket_nonce,
        });
    }

    async fn emit_budget_update(&self, delta: NodeBudget) {
        // Per-node BudgetUpdate: remaining-budget fields stay at 0 since
        // they're run-scoped accounting the coordinator maintains
        // separately. The node-specific delta lives in the
        // `node_tokens_consumed` / `node_cost_cents_consumed` fields that
        // the frontend routes to per-node state.
        let _ = self.sender.send(SwarmEvent::BudgetUpdate {
            run_id: self.run_id,
            tokens_remaining: 0,
            cents_remaining: 0,
            wall_ms_remaining: 0,
            ticket_nonce: self.ticket_nonce,
            node_id: Some(self.node_id.clone()),
            node_tokens_consumed: Some(delta.tokens_consumed),
            node_cost_cents_consumed: Some(delta.cost_cents),
        });
    }
}

/// Recording emitter for tests. Captures every emission in an in-memory
/// vec protected by a `tokio::sync::Mutex` so tests assert after an
/// adapter call without threading channels. Exposed unconditionally so
/// integration tests under `tests/` (which are separate crates and can't
/// see `cfg(test)`) can use it.
pub mod recording {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Recorded {
        Phase { phase: String, payload: Value },
        Budget { delta: NodeBudget },
    }

    #[derive(Default)]
    pub struct RecordingEmitter {
        pub log: Arc<Mutex<Vec<Recorded>>>,
    }

    impl RecordingEmitter {
        pub fn new() -> Self {
            Self {
                log: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub async fn snapshot(&self) -> Vec<Recorded> {
            self.log.lock().await.clone()
        }
    }

    #[async_trait]
    impl EventEmitter for RecordingEmitter {
        async fn emit_phase(&self, phase: &str, payload: Value) {
            self.log.lock().await.push(Recorded::Phase {
                phase: phase.to_string(),
                payload,
            });
        }

        async fn emit_budget_update(&self, delta: NodeBudget) {
            self.log.lock().await.push(Recorded::Budget { delta });
        }
    }
}
