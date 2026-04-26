//! Phase 4b-coder: end-to-end Artisan → CoderEntry → fake provider.
//!
//! The unit tests in `agents/coder/src/swarm_entry.rs` exercise
//! `CoderEntry::execute` directly. These integration tests go through
//! the swarm-side adapter (`ArtisanAdapter::run_with_context`) so we
//! cover the wiring: input passthrough, error mapping
//! (`AgentError::Cancelled` → `SwarmError::Cancelled`,
//! `AgentError::InvalidInput` → `SwarmError::DirectorParse`), and the
//! shared output schema.

use async_trait::async_trait;
use nexus_swarm::adapters::{ArtisanAdapter, HeraldAdapter};
use nexus_swarm::capability::{CapabilityInvocation, SwarmCapability};
use nexus_swarm::coordinator::CancelToken;
use nexus_swarm::emitter::recording::{Recorded, RecordingEmitter};
use nexus_swarm::{
    AgentExecutionContext, NodeBudget, ProviderHealth, ProviderHealthStatus, SwarmError,
};
use nexus_swarm_core::{
    CostClass, InvokeRequest, InvokeResponse, ModelDescriptor, PrivacyClass, Provider,
    ProviderCapabilities, ProviderError, ReasoningTier,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

struct StaticProvider {
    text: String,
}

#[async_trait]
impl Provider for StaticProvider {
    fn id(&self) -> &str {
        "fake"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: "fake-small".into(),
                param_count_b: Some(1),
                tier: ReasoningTier::Light,
                context_window: 4096,
            }],
            supports_tool_use: false,
            supports_streaming: false,
            max_context: 4096,
            cost_class: CostClass::Free,
            privacy_class: PrivacyClass::Public,
        }
    }
    async fn health_check(&self) -> ProviderHealth {
        ProviderHealth {
            provider_id: "fake".into(),
            status: ProviderHealthStatus::Ok,
            latency_ms: Some(1),
            models: vec!["fake-small".into()],
            notes: String::new(),
            checked_at_secs: 0,
        }
    }
    async fn invoke(&self, _req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        Ok(InvokeResponse {
            text: self.text.clone(),
            tokens_in: 11,
            tokens_out: 4,
            cost_cents: 0,
            latency_ms: 0,
            model_id: "fake-small".into(),
        })
    }
}

fn providers() -> Arc<HashMap<String, Arc<dyn Provider>>> {
    let mut m: HashMap<String, Arc<dyn Provider>> = HashMap::new();
    m.insert(
        "fake".into(),
        Arc::new(StaticProvider {
            text: "```rust:src/lib.rs\npub fn add(a: i32, b: i32) -> i32 { a + b }\n```\n".into(),
        }),
    );
    Arc::new(m)
}

fn invocation_with_task(task: &str) -> CapabilityInvocation {
    CapabilityInvocation {
        inputs: json!({
            "node_inputs": { "task": task, "style": { "language": "rust" } },
            "route": { "provider_id": "fake", "model_id": "fake-small" },
        }),
        parent_outputs: Default::default(),
    }
}

fn malformed_invocation() -> CapabilityInvocation {
    // Missing `task` — CoderEntry should reject with InvalidInput.
    CapabilityInvocation {
        inputs: json!({
            "node_inputs": { "style": { "language": "rust" } },
            "route": { "provider_id": "fake", "model_id": "fake-small" },
        }),
        parent_outputs: Default::default(),
    }
}

fn mk_ctx(rec: Arc<RecordingEmitter>, cancel: CancelToken) -> AgentExecutionContext {
    AgentExecutionContext {
        ticket_nonce: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        node_id: "n-artisan".into(),
        capability_id: "artisan".into(),
        provider: Arc::new(StaticProvider {
            text: "```rust:src/lib.rs\npub fn x() {}\n```\n".into(),
        }),
        model_id: "fake-small".into(),
        emit: rec,
        cancel,
        budget: Arc::new(Mutex::new(NodeBudget::new())),
    }
}

#[tokio::test]
async fn artisan_delegates_to_coder_entry_and_returns_files_array() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec.clone(), CancelToken::new());
    let adapter = ArtisanAdapter::new(providers());
    let out = adapter
        .run_with_context(invocation_with_task("hello"), &ctx)
        .await
        .expect("ok");
    let files = out
        .get("files")
        .and_then(|v| v.as_array())
        .expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].get("path").and_then(|v| v.as_str()),
        Some("src/lib.rs")
    );
    assert!(out.get("summary").is_some());
    assert_eq!(
        out.get("tokens_used").and_then(|v| v.as_u64()),
        Some(15) /* 11 + 4 */
    );
}

#[tokio::test]
async fn artisan_propagates_cancellation_through_coder_entry() {
    let rec = Arc::new(RecordingEmitter::new());
    let cancel = CancelToken::new();
    let ctx = mk_ctx(rec, cancel.clone());
    cancel.cancel();
    let adapter = ArtisanAdapter::new(providers());
    let err = adapter
        .run_with_context(invocation_with_task("anything"), &ctx)
        .await
        .expect_err("should cancel");
    match err {
        SwarmError::Cancelled => {}
        other => panic!("expected SwarmError::Cancelled, got {other:?}"),
    }
}

#[tokio::test]
async fn artisan_maps_invalid_input_to_typed_agent_error() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec, CancelToken::new());
    let adapter = ArtisanAdapter::new(providers());
    let err = adapter
        .run_with_context(malformed_invocation(), &ctx)
        .await
        .expect_err("should reject malformed input");
    match err {
        SwarmError::AgentInvalidInput { agent, detail } => {
            assert_eq!(agent, "artisan");
            assert!(detail.contains("CoderInput"));
        }
        other => panic!("expected AgentInvalidInput, got {other:?}"),
    }
}

#[tokio::test]
async fn artisan_emissions_reach_recording_emitter_through_coder_entry() {
    // Confirms that emissions from inside coder-agent's swarm_entry
    // module flow through `ctx.emit` (the coordinator-bound emitter) and
    // are observable by a recording fixture handed in via context.
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec.clone(), CancelToken::new());
    let adapter = ArtisanAdapter::new(providers());
    adapter
        .run_with_context(invocation_with_task("emit smoke"), &ctx)
        .await
        .expect("ok");
    let log = rec.snapshot().await;
    let phase_names: Vec<&str> = log
        .iter()
        .filter_map(|r| match r {
            Recorded::Phase { phase, .. } => Some(phase.as_str()),
            _ => None,
        })
        .collect();
    assert!(phase_names.contains(&"generating"));
    assert!(phase_names.contains(&"complete"));
    let budget_emissions = log
        .iter()
        .filter(|r| matches!(r, Recorded::Budget { .. }))
        .count();
    assert_eq!(budget_emissions, 1);
}

// ── Herald → SocialPosterEntry ──────────────────────────────────────────────

fn herald_invocation_with_message(msg: &str, dry_run: Option<bool>) -> CapabilityInvocation {
    let mut node_inputs = serde_json::json!({
        "channel": "X",
        "audience": "Rust devs",
        "message": msg,
    });
    if let Some(b) = dry_run {
        node_inputs["dry_run"] = serde_json::Value::Bool(b);
    }
    CapabilityInvocation {
        inputs: serde_json::json!({
            "node_inputs": node_inputs,
            "route": { "provider_id": "fake", "model_id": "fake-small" },
        }),
        parent_outputs: Default::default(),
    }
}

fn malformed_herald_invocation() -> CapabilityInvocation {
    // Missing `message`.
    CapabilityInvocation {
        inputs: serde_json::json!({
            "node_inputs": { "channel": "X", "audience": "devs" },
            "route": { "provider_id": "fake", "model_id": "fake-small" },
        }),
        parent_outputs: Default::default(),
    }
}

fn herald_providers() -> Arc<HashMap<String, Arc<dyn Provider>>> {
    let mut m: HashMap<String, Arc<dyn Provider>> = HashMap::new();
    m.insert(
        "fake".into(),
        Arc::new(StaticProvider {
            text: "Tokio 1.50 ships better spawn_blocking. Faster, fewer deadlocks.".into(),
        }),
    );
    Arc::new(m)
}

#[tokio::test]
async fn herald_delegates_to_social_poster_entry_dry_run_default() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec.clone(), CancelToken::new());
    let adapter = HeraldAdapter::new(
        herald_providers(),
        Arc::new(social_poster_agent::publish_state::InMemoryPublishState::new()),
    );
    let out = adapter
        .run_with_context(herald_invocation_with_message("ship it", None), &ctx)
        .await
        .expect("ok");
    assert_eq!(out.get("dry_run").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        out.get("publish_status").and_then(|v| v.as_str()),
        Some("skipped_dry_run")
    );
    assert!(out.get("draft").and_then(|v| v.as_str()).is_some());
}

#[tokio::test]
async fn herald_dry_run_false_returns_publish_deferred() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec, CancelToken::new());
    let adapter = HeraldAdapter::new(
        herald_providers(),
        Arc::new(social_poster_agent::publish_state::InMemoryPublishState::new()),
    );
    let out = adapter
        .run_with_context(
            herald_invocation_with_message("real run", Some(false)),
            &ctx,
        )
        .await
        .expect("ok");
    assert_eq!(out.get("dry_run").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        out.get("publish_status").and_then(|v| v.as_str()),
        Some("deferred"),
        "Phase 5 publish wiring is intentionally out of scope; expect Deferred"
    );
}

#[tokio::test]
async fn herald_propagates_cancellation_through_social_poster_entry() {
    let rec = Arc::new(RecordingEmitter::new());
    let cancel = CancelToken::new();
    let ctx = mk_ctx(rec, cancel.clone());
    cancel.cancel();
    let adapter = HeraldAdapter::new(
        herald_providers(),
        Arc::new(social_poster_agent::publish_state::InMemoryPublishState::new()),
    );
    let err = adapter
        .run_with_context(herald_invocation_with_message("anything", None), &ctx)
        .await
        .expect_err("should cancel");
    match err {
        SwarmError::Cancelled => {}
        other => panic!("expected SwarmError::Cancelled, got {other:?}"),
    }
}

#[tokio::test]
async fn herald_maps_invalid_input_to_typed_agent_error_with_agent_name() {
    let rec = Arc::new(RecordingEmitter::new());
    let ctx = mk_ctx(rec, CancelToken::new());
    let adapter = HeraldAdapter::new(
        herald_providers(),
        Arc::new(social_poster_agent::publish_state::InMemoryPublishState::new()),
    );
    let err = adapter
        .run_with_context(malformed_herald_invocation(), &ctx)
        .await
        .expect_err("should reject");
    match err {
        SwarmError::AgentInvalidInput { agent, detail } => {
            assert_eq!(agent, "herald");
            assert!(detail.contains("SocialPosterInput") || detail.contains("missing"));
        }
        other => panic!("expected AgentInvalidInput, got {other:?}"),
    }
}
