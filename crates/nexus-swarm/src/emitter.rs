//! `CoordinatorEmitter` — production emitter that publishes through the
//! coordinator's broadcast channel as `SwarmEvent` variants. The
//! `EventEmitter` trait + `recording::RecordingEmitter` test fixture
//! both live in `nexus-swarm-core` so agent crates can import them
//! without depending on this crate. Existing call sites keep using the
//! `nexus_swarm::emitter::*` paths via the re-exports below.

use crate::events::{NodeRef, SwarmEvent};
use async_trait::async_trait;
use nexus_swarm_core::context::NodeBudget;
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

pub use nexus_swarm_core::emitter::recording;
pub use nexus_swarm_core::emitter::EventEmitter;

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
