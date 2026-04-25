//! Verify each real adapter emits plan → act → observe and records
//! per-node budget when called via `run_with_context`.

use async_trait::async_trait;
use nexus_swarm::adapters::{ArtisanAdapter, BrokerAdapter, HeraldAdapter};
use nexus_swarm::capability::{CapabilityInvocation, SwarmCapability};
use nexus_swarm::coordinator::CancelToken;
use nexus_swarm::emitter::recording::{Recorded, RecordingEmitter};
use nexus_swarm::events::{ProviderHealth, ProviderHealthStatus};
use nexus_swarm::profile::{CostClass, PrivacyClass, ReasoningTier};
use nexus_swarm::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use nexus_swarm::{AgentExecutionContext, NodeBudget};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

struct CannedProvider {
    tokens_in: u32,
    tokens_out: u32,
    cost_cents: u32,
}

impl Default for CannedProvider {
    fn default() -> Self {
        Self {
            tokens_in: 7,
            tokens_out: 5,
            cost_cents: 1,
        }
    }
}

#[async_trait]
impl Provider for CannedProvider {
    fn id(&self) -> &str {
        "canned"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: "canned-small".into(),
                param_count_b: Some(1),
                tier: ReasoningTier::Light,
                context_window: 4096,
            }],
            supports_tool_use: true,
            supports_streaming: false,
            max_context: 4096,
            cost_class: CostClass::Free,
            privacy_class: PrivacyClass::Public,
        }
    }
    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth {
            provider_id: "canned".into(),
            status: ProviderHealthStatus::Ok,
            latency_ms: Some(1),
            models: vec!["canned-small".into()],
            notes: String::new(),
            checked_at_secs: 0,
        }
    }
    async fn invoke(&self, _req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        Ok(InvokeResponse {
            text: "canned response text".into(),
            tokens_in: self.tokens_in,
            tokens_out: self.tokens_out,
            cost_cents: self.cost_cents,
            latency_ms: 0,
            model_id: "canned-small".into(),
        })
    }
}

fn providers() -> Arc<HashMap<String, Arc<dyn Provider>>> {
    let mut m: HashMap<String, Arc<dyn Provider>> = HashMap::new();
    m.insert("canned".into(), Arc::new(CannedProvider::default()));
    Arc::new(m)
}

fn mk_context(
    capability_id: &str,
    emitter: Arc<RecordingEmitter>,
    cancel: CancelToken,
) -> AgentExecutionContext {
    AgentExecutionContext {
        ticket_nonce: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        node_id: format!("n-{capability_id}"),
        capability_id: capability_id.into(),
        provider: Arc::new(CannedProvider::default()),
        model_id: "canned-small".into(),
        emit: emitter,
        cancel,
        budget: Arc::new(Mutex::new(NodeBudget::new())),
    }
}

fn artisan_invocation() -> CapabilityInvocation {
    // Phase 4b: Artisan now delegates to coder-agent's `CoderEntry`,
    // which expects `task` (not the legacy `instruction`) and an
    // optional `style.language`.
    CapabilityInvocation {
        inputs: json!({
            "node_inputs": {
                "task": "write fn add",
                "style": { "language": "rust" }
            },
            "route": {"provider_id": "canned", "model_id": "canned-small"},
        }),
        parent_outputs: Default::default(),
    }
}

fn herald_invocation() -> CapabilityInvocation {
    // Phase 4b-herald: Herald now delegates to social-poster-agent's
    // SocialPosterEntry, which expects { channel, audience, message }.
    // Tests omit `dry_run` to exercise the default-true path.
    CapabilityInvocation {
        inputs: json!({
            "node_inputs": {
                "channel": "X",
                "audience": "Rust developers",
                "message": "ship it",
                "tone": "neutral"
            },
            "route": {"provider_id": "canned", "model_id": "canned-small"},
        }),
        parent_outputs: Default::default(),
    }
}

fn broker_invocation() -> CapabilityInvocation {
    CapabilityInvocation {
        inputs: json!({
            "node_inputs": {"directive": "dispatch"},
            "route": {"provider_id": "canned", "model_id": "canned-small"},
        }),
        parent_outputs: Default::default(),
    }
}

fn assert_plan_act_observe(log: &[Recorded]) {
    let phases: Vec<&str> = log
        .iter()
        .filter_map(|r| match r {
            Recorded::Phase { phase, .. } => Some(phase.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        phases,
        vec!["plan", "act", "observe"],
        "expected plan → act → observe, got {phases:?}"
    );
}

/// Phase 4b: Artisan now delegates to `coder_agent::CoderEntry`, which
/// emits a 5-phase semantic sequence (no `writing_files` when the
/// invocation didn't supply `output_dir`).
fn assert_coder_phases_no_output_dir(log: &[Recorded]) {
    let phases: Vec<&str> = log
        .iter()
        .filter_map(|r| match r {
            Recorded::Phase { phase, .. } => Some(phase.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        phases,
        vec![
            "parsing_input",
            "planning",
            "generating",
            "parsing_response",
            "complete"
        ],
        "expected coder 5-phase sequence, got {phases:?}"
    );
}

#[tokio::test]
async fn artisan_emits_coder_phase_sequence() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_context("artisan", rec.clone(), CancelToken::new());
    let adapter = ArtisanAdapter::new(providers());
    adapter
        .run_with_context(artisan_invocation(), &ctx)
        .await
        .expect("ok");
    assert_coder_phases_no_output_dir(&rec.snapshot().await);
}

/// Phase 4b-herald: Herald now delegates to
/// `social_poster_agent::SocialPosterEntry`, which emits a 6-phase
/// semantic sequence: parsing_input → drafting → parsing_response →
/// reviewing → publishing → complete.
fn assert_social_poster_phase_sequence(log: &[Recorded]) {
    let phases: Vec<&str> = log
        .iter()
        .filter_map(|r| match r {
            Recorded::Phase { phase, .. } => Some(phase.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        phases,
        vec![
            "parsing_input",
            "drafting",
            "parsing_response",
            "reviewing",
            "publishing",
            "complete"
        ],
        "expected social-poster 6-phase sequence, got {phases:?}"
    );
}

#[tokio::test]
async fn herald_emits_social_poster_phase_sequence() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_context("herald", rec.clone(), CancelToken::new());
    let adapter = HeraldAdapter::new(providers());
    adapter
        .run_with_context(herald_invocation(), &ctx)
        .await
        .expect("ok");
    assert_social_poster_phase_sequence(&rec.snapshot().await);
}

#[tokio::test]
async fn broker_emits_plan_act_observe() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_context("broker", rec.clone(), CancelToken::new());
    let adapter = BrokerAdapter::new(providers());
    adapter
        .run_with_context(broker_invocation(), &ctx)
        .await
        .expect("ok");
    assert_plan_act_observe(&rec.snapshot().await);
}

#[tokio::test]
async fn adapter_records_per_node_budget_update() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_context("artisan", rec.clone(), CancelToken::new());
    let adapter = ArtisanAdapter::new(providers());
    adapter
        .run_with_context(artisan_invocation(), &ctx)
        .await
        .expect("ok");
    let log = rec.snapshot().await;
    let budgets: Vec<&NodeBudget> = log
        .iter()
        .filter_map(|r| match r {
            Recorded::Budget { delta } => Some(delta),
            _ => None,
        })
        .collect();
    assert_eq!(budgets.len(), 1, "expected exactly one budget emission");
    // tokens_in + tokens_out from the CannedProvider default = 7 + 5 = 12.
    assert_eq!(budgets[0].tokens_consumed, 12);
    assert_eq!(budgets[0].cost_cents, 1);
}

#[tokio::test]
async fn cancellation_between_phases_returns_cancelled() {
    let rec = Arc::new(RecordingEmitter::new());
    let cancel = CancelToken::new();
    let ctx = mk_context("artisan", rec.clone(), cancel.clone());
    // Cancel before the adapter ever runs.
    cancel.cancel();
    let adapter = ArtisanAdapter::new(providers());
    let err = adapter
        .run_with_context(artisan_invocation(), &ctx)
        .await
        .expect_err("should cancel");
    match err {
        nexus_swarm::SwarmError::Cancelled => {}
        other => panic!("expected Cancelled, got {other:?}"),
    }
}

#[tokio::test]
async fn observe_payload_contains_response_digest_and_tokens() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_context("broker", rec.clone(), CancelToken::new());
    let adapter = BrokerAdapter::new(providers());
    adapter
        .run_with_context(broker_invocation(), &ctx)
        .await
        .expect("ok");
    let log = rec.snapshot().await;
    let observe = log
        .iter()
        .find_map(|r| match r {
            Recorded::Phase { phase, payload } if phase == "observe" => Some(payload.clone()),
            _ => None,
        })
        .expect("observe phase emitted");
    assert!(observe.get("response_digest").is_some());
    assert_eq!(
        observe.get("completion_tokens").and_then(|v| v.as_u64()),
        Some(5)
    );
    assert_eq!(
        observe.get("prompt_tokens").and_then(|v| v.as_u64()),
        Some(7)
    );
}
