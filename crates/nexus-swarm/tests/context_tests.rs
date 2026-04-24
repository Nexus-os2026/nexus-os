//! Context + emitter unit tests.

use async_trait::async_trait;
use nexus_swarm::coordinator::CancelToken;
use nexus_swarm::emitter::CoordinatorEmitter;
use nexus_swarm::events::{ProviderHealth, ProviderHealthStatus, SwarmEvent};
use nexus_swarm::profile::{CostClass, PrivacyClass, ReasoningTier};
use nexus_swarm::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use nexus_swarm::{AgentExecutionContext, NodeBudget};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

struct StubProvider;

#[async_trait]
impl Provider for StubProvider {
    fn id(&self) -> &str {
        "stub"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: "stub-small".into(),
                param_count_b: Some(1),
                tier: ReasoningTier::Light,
                context_window: 2048,
            }],
            supports_tool_use: false,
            supports_streaming: false,
            max_context: 2048,
            cost_class: CostClass::Free,
            privacy_class: PrivacyClass::Public,
        }
    }
    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth {
            provider_id: "stub".into(),
            status: ProviderHealthStatus::Ok,
            latency_ms: Some(1),
            models: vec!["stub-small".into()],
            notes: String::new(),
            checked_at_secs: 0,
        }
    }
    async fn invoke(&self, _req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        Ok(InvokeResponse {
            text: "ok".into(),
            tokens_in: 1,
            tokens_out: 1,
            cost_cents: 0,
            latency_ms: 0,
            model_id: "stub-small".into(),
        })
    }
}

fn mk_ctx(
    sender: broadcast::Sender<SwarmEvent>,
    ticket_nonce: Uuid,
    run_id: Uuid,
    node_id: &str,
) -> AgentExecutionContext {
    let emitter = Arc::new(CoordinatorEmitter::new(
        sender,
        ticket_nonce,
        run_id,
        node_id.to_string(),
    ));
    AgentExecutionContext {
        ticket_nonce,
        run_id,
        node_id: node_id.to_string(),
        capability_id: "cap".into(),
        provider: Arc::new(StubProvider),
        model_id: "stub-small".into(),
        emit: emitter,
        cancel: CancelToken::new(),
        budget: Arc::new(Mutex::new(NodeBudget::new())),
    }
}

#[tokio::test]
async fn node_budget_record_accumulates_and_saturates() {
    let mut b = NodeBudget::new();
    b.record(10, 2);
    b.record(20, 3);
    assert_eq!(b.tokens_consumed, 30);
    assert_eq!(b.cost_cents, 5);
}

#[tokio::test]
async fn coordinator_emitter_tags_node_event_with_ticket_and_node() {
    let (tx, mut rx) = broadcast::channel(8);
    let ticket = Uuid::new_v4();
    let run = Uuid::new_v4();
    let ctx = mk_ctx(tx, ticket, run, "n-tag");
    ctx.emit
        .emit_phase("plan", serde_json::json!({"tool": "cap"}))
        .await;
    let ev = rx.recv().await.expect("one event");
    match ev {
        SwarmEvent::NodeEvent {
            r#ref,
            phase,
            ticket_nonce,
            ..
        } => {
            assert_eq!(r#ref.run_id, run);
            assert_eq!(r#ref.node_id, "n-tag");
            assert_eq!(ticket_nonce, ticket);
            assert_eq!(phase, "plan");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn coordinator_emitter_budget_update_carries_node_delta() {
    let (tx, mut rx) = broadcast::channel(8);
    let ticket = Uuid::new_v4();
    let run = Uuid::new_v4();
    let ctx = mk_ctx(tx, ticket, run, "n-budget");
    let delta = NodeBudget {
        tokens_consumed: 42,
        cost_cents: 3,
    };
    ctx.emit.emit_budget_update(delta).await;
    let ev = rx.recv().await.expect("one event");
    match ev {
        SwarmEvent::BudgetUpdate {
            node_id,
            node_tokens_consumed,
            node_cost_cents_consumed,
            run_id,
            ..
        } => {
            assert_eq!(run_id, run);
            assert_eq!(node_id.as_deref(), Some("n-budget"));
            assert_eq!(node_tokens_consumed, Some(42));
            assert_eq!(node_cost_cents_consumed, Some(3));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn cancel_token_clone_propagates() {
    let tok = CancelToken::new();
    let clone = tok.clone();
    assert!(!tok.is_cancelled());
    assert!(!clone.is_cancelled());
    tok.cancel();
    assert!(tok.is_cancelled());
    assert!(clone.is_cancelled());
}

#[tokio::test]
async fn agent_execution_context_cancelled_reflects_token() {
    let (tx, _rx) = broadcast::channel(4);
    let ctx = mk_ctx(tx, Uuid::new_v4(), Uuid::new_v4(), "n-cancel");
    assert!(!ctx.cancelled());
    ctx.cancel.cancel();
    assert!(ctx.cancelled());
}
