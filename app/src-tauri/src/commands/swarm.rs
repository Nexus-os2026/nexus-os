//! Swarm Tauri commands — surface the `nexus-swarm` crate to the frontend.
//!
//! Nine commands:
//!   - `swarm_plan` — intent → `PlannedSwarmJson` (dag + ticket identifiers)
//!   - `swarm_approve` — start running an approved plan by ticket id
//!   - `swarm_reject` — discard a proposed plan
//!   - `swarm_cancel` — stop an active run
//!   - `swarm_cancel_node` — cancel a single node (Phase 1: equivalent to
//!     cancelling the whole run; per-node cancel lands in Phase 2)
//!   - `swarm_state` — fetch run state snapshot
//!   - `swarm_provider_health` — cached health snapshot
//!   - `swarm_refresh_provider_health` — probe every provider afresh
//!   - `swarm_audit_tail` — oracle-anchored event trail for a run
//!
//! All events are relayed through the single channel `"swarm:event"` with
//! tagged JSON. The forwarder task that bridges the broadcast → Tauri emit
//! is spawned lazily on first use and also feeds the per-run audit store
//! that backs `swarm_audit_tail`.

use nexus_swarm::adapters::{
    ArtisanAdapter, BrokerAdapter, HeraldAdapter, ProspectorStub, ScoutStub, WatchdogStub,
};
use nexus_swarm::events::{ProviderHealth, SwarmEvent};
use nexus_swarm::oracle_bridge::{OracleBridge, SwarmOracleBridge};
use nexus_swarm::provider::Provider;
use nexus_swarm::providers::{
    AnthropicProvider, CodexCliProvider, HuggingFaceProvider, OllamaSwarmProvider,
    OpenAiSwarmProvider, OpenRouterSwarmProvider,
};
use nexus_swarm::{
    Budget, CapabilityRegistry, PlannedSwarm, Router, SwarmCoordinator, SwarmDirector,
    SwarmRunHandle,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;
use tauri::Emitter;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::AppState;

/// Entry in the oracle-anchored audit tail surfaced through
/// `swarm_audit_tail`. Every provider-touching `SwarmEvent` that carries a
/// `ticket_nonce` produces one entry, in broadcast order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub event_kind: String,
    pub ticket_nonce: Uuid,
    /// Bug AL/AM precursor (Track C #1): node identifier when the
    /// underlying `SwarmEvent` references one, else `None`. Populated
    /// per-variant in `event_to_audit_entry`. The frontend audit
    /// viewer filters on this; serializes as `null` when absent
    /// (Tauri's `Option<String>` representation).
    pub node_id: Option<String>,
    pub timestamp: SystemTime,
    pub payload_summary: String,
}

/// Cross-command state for the swarm subsystem. The pending-plan store now
/// holds full `PlannedSwarm` values (not bare DAGs) so `swarm_approve` can
/// hand the ticket through to the coordinator.
struct SwarmInner {
    registry: Arc<CapabilityRegistry>,
    router: Arc<Router>,
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
    health_snapshot: Arc<Mutex<HashMap<String, ProviderHealth>>>,
    events: broadcast::Sender<SwarmEvent>,
    runs: Arc<Mutex<HashMap<Uuid, SwarmRunHandle>>>,
    pending_plans: Arc<Mutex<HashMap<Uuid, PlannedSwarm>>>,
    audit: Arc<Mutex<HashMap<Uuid, Vec<AuditEntry>>>>,
    /// Monotonic sequence counter used when building `AuditEntry`.
    audit_seq: Arc<std::sync::atomic::AtomicU64>,
}

static STATE: OnceLock<Arc<SwarmInner>> = OnceLock::new();
static FORWARDER_SPAWNED: OnceLock<()> = OnceLock::new();

fn state() -> Arc<SwarmInner> {
    STATE
        .get_or_init(|| {
            let providers_vec: Vec<Arc<dyn Provider>> = vec![
                Arc::new(OllamaSwarmProvider::from_env()),
                Arc::new(CodexCliProvider::new()),
                Arc::new(OpenAiSwarmProvider::new()),
                Arc::new(AnthropicProvider::new()),
                Arc::new(OpenRouterSwarmProvider::new()),
                Arc::new(HuggingFaceProvider::new()),
            ];
            let mut providers_map: HashMap<String, Arc<dyn Provider>> = HashMap::new();
            for p in &providers_vec {
                providers_map.insert(p.id().to_string(), Arc::clone(p));
            }
            let providers = Arc::new(providers_map);

            // Bug W: Herald needs an Arc<dyn PublishStateHandle> for
            // per-channel post-count compliance reads. The swarm STATE
            // initializer is a module-level OnceLock with no AppState
            // in scope, so it opens its own NexusDatabase against the
            // default path. WAL mode means rusqlite tolerates the
            // concurrent connection; AppState's own NexusDatabase
            // points at the same file.
            let publish_state: Arc<
                dyn social_poster_agent::publish_state::PublishStateHandle,
            > = match nexus_persistence::NexusDatabase::open(
                &nexus_persistence::NexusDatabase::default_db_path(),
            ) {
                Ok(db) => Arc::new(social_poster_agent::publish_state::SqlitePublishState::new(
                    Arc::new(db),
                )),
                Err(e) => {
                    eprintln!(
                        "[swarm] WARNING: NexusDatabase open failed ({e}); Herald falling back to InMemoryPublishState (post-count history will not survive restart)"
                    );
                    Arc::new(social_poster_agent::publish_state::InMemoryPublishState::new())
                }
            };

            let mut registry = CapabilityRegistry::new();
            registry.register(Arc::new(ArtisanAdapter::new(Arc::clone(&providers))));
            registry.register(Arc::new(HeraldAdapter::new(
                Arc::clone(&providers),
                Arc::clone(&publish_state),
            )));
            registry.register(Arc::new(BrokerAdapter::new(Arc::clone(&providers))));
            registry.register(Arc::new(ScoutStub));
            registry.register(Arc::new(WatchdogStub));
            registry.register(Arc::new(ProspectorStub));
            let registry = Arc::new(registry);

            let mut router = Router::new();
            for p in providers.values() {
                router.register_provider(Arc::clone(p));
            }
            for policy in nexus_swarm::routing_defaults::load_policies() {
                router.set_policy(policy);
            }
            let router = Arc::new(router);

            let health_snapshot = Arc::new(Mutex::new(HashMap::<String, ProviderHealth>::new()));
            let (events, _rx) = broadcast::channel::<SwarmEvent>(256);

            Arc::new(SwarmInner {
                registry,
                router,
                providers,
                health_snapshot,
                events,
                runs: Arc::new(Mutex::new(HashMap::new())),
                pending_plans: Arc::new(Mutex::new(HashMap::new())),
                audit: Arc::new(Mutex::new(HashMap::new())),
                audit_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            })
        })
        .clone()
}

fn ensure_forwarder(app: tauri::AppHandle) {
    if FORWARDER_SPAWNED.set(()).is_err() {
        return;
    }
    let s = state();
    let mut rx = s.events.subscribe();
    let audit = Arc::clone(&s.audit);
    let audit_seq = Arc::clone(&s.audit_seq);
    tauri::async_runtime::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            if let Some((run_id, entry)) = event_to_audit_entry(&ev, &audit_seq) {
                let mut store = audit.lock().await;
                store.entry(run_id).or_default().push(entry);
            }
            let _ = app.emit("swarm:event", &ev);
        }
    });
}

fn event_to_audit_entry(
    ev: &SwarmEvent,
    audit_seq: &std::sync::atomic::AtomicU64,
) -> Option<(Uuid, AuditEntry)> {
    let (run_id, ticket_nonce, kind, summary, node_id) = match ev {
        SwarmEvent::NodeStarted {
            r#ref,
            capability_id,
            provider_id,
            model_id,
            ticket_nonce,
        } => (
            r#ref.run_id,
            *ticket_nonce,
            "node_started",
            format!("{capability_id} via {provider_id}/{model_id}"),
            Some(r#ref.node_id.clone()),
        ),
        SwarmEvent::NodeEvent {
            r#ref,
            phase,
            ticket_nonce,
            ..
        } => (
            r#ref.run_id,
            *ticket_nonce,
            "node_event",
            format!("phase={phase}"),
            Some(r#ref.node_id.clone()),
        ),
        SwarmEvent::NodeCompleted {
            r#ref,
            ticket_nonce,
            ..
        } => (
            r#ref.run_id,
            *ticket_nonce,
            "node_completed",
            format!("node={}", r#ref.node_id),
            Some(r#ref.node_id.clone()),
        ),
        SwarmEvent::NodeFailed {
            r#ref,
            reason,
            ticket_nonce,
        } => (
            r#ref.run_id,
            *ticket_nonce,
            "node_failed",
            format!("node={} reason={reason}", r#ref.node_id),
            Some(r#ref.node_id.clone()),
        ),
        SwarmEvent::BudgetUpdate {
            run_id,
            tokens_remaining,
            cents_remaining,
            wall_ms_remaining,
            ticket_nonce,
            node_id,
            node_tokens_consumed,
            node_cost_cents_consumed,
        } => {
            let scope = match node_id {
                Some(id) => format!(
                    " node={id} node_tokens={} node_cents={}",
                    node_tokens_consumed.unwrap_or(0),
                    node_cost_cents_consumed.unwrap_or(0),
                ),
                None => String::new(),
            };
            (
                *run_id,
                *ticket_nonce,
                "budget_update",
                format!(
                    "tokens={tokens_remaining} cents={cents_remaining} wall_ms={wall_ms_remaining}{scope}"
                ),
                node_id.clone(),
            )
        }
        SwarmEvent::OracleRuntimeCheck {
            ticket_nonce,
            highrisk_event,
            decision,
        } => (
            // OracleRuntimeCheck doesn't carry a run_id directly; we key
            // by ticket_nonce-derived zero UUID namespace for audit
            // purposes. In practice the frontend looks up by run_id AND
            // we index by `ticket_nonce` here to make post-run audit
            // queries cheaper.
            Uuid::nil(),
            *ticket_nonce,
            "oracle_runtime_check",
            format!(
                "event={highrisk_event:?} approved={} token_id={:?}",
                decision.approved, decision.token_id
            ),
            // No node_id on this variant — runtime checks fire on
            // ticket_nonce, not on a specific node.
            None,
        ),
        SwarmEvent::OracleRuntimeDenial {
            ticket_nonce,
            hints,
            node_id,
        } => (
            Uuid::nil(),
            *ticket_nonce,
            "oracle_runtime_denial",
            format!("node={node_id} hints=[{}]", hints.join("; ")),
            // OracleRuntimeDenial always denies *for* a specific node.
            // Surfacing it through the audit `node_id` field lets the
            // frontend filter chip find denials by node without
            // payload_summary string-parsing.
            Some(node_id.clone()),
        ),
        _ => return None,
    };

    let seq = audit_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Some((
        run_id,
        AuditEntry {
            seq,
            event_kind: kind.into(),
            ticket_nonce,
            node_id,
            timestamp: SystemTime::now(),
            payload_summary: summary,
        },
    ))
}

fn build_coordinator(bridge: Arc<dyn OracleBridge>) -> Arc<SwarmCoordinator> {
    let s = state();
    Arc::new(SwarmCoordinator::new(
        Arc::clone(&s.registry),
        Arc::clone(&s.router),
        Arc::clone(&s.providers),
        Arc::clone(&s.health_snapshot),
        s.events.clone(),
        bridge,
    ))
}

/// The JSON-safe projection of a `PlannedSwarm` that crosses the Tauri
/// boundary. The server retains the full `PlannedSwarm` (including the
/// `SealedToken` body) keyed by `ticket_id`.
#[derive(Debug, Serialize, Deserialize)]
pub struct PlannedSwarmJson {
    pub dag: Value,
    pub ticket_id: Uuid,
    pub budget_hash: String,
    pub privacy_envelope: String,
}

fn director_model() -> Option<String> {
    std::env::var("NEXUS_SWARM_DIRECTOR_MODEL").ok()
}

fn privacy_label(p: nexus_swarm::PrivacyClass) -> &'static str {
    match p {
        nexus_swarm::PrivacyClass::Public => "Public",
        nexus_swarm::PrivacyClass::Sensitive => "Sensitive",
        nexus_swarm::PrivacyClass::StrictLocal => "StrictLocal",
    }
}

#[tauri::command]
pub async fn swarm_plan(
    state: tauri::State<'_, AppState>,
    intent: String,
) -> Result<PlannedSwarmJson, String> {
    let s = self::state();
    // Pick Director's planner — prefer ollama if available, else first
    // registered public provider.
    let planner: Arc<dyn Provider> = s
        .providers
        .get("ollama")
        .cloned()
        .or_else(|| s.providers.get("openrouter").cloned())
        .or_else(|| s.providers.values().next().cloned())
        .ok_or_else(|| "no providers configured".to_string())?;
    let model = director_model().unwrap_or_else(|| {
        if planner.id() == "ollama" {
            planner
                .capabilities()
                .models
                .first()
                .map(|m| m.id.clone())
                .unwrap_or_else(|| "gemma4:e2b".to_string())
        } else {
            "claude-haiku-4-5-20251001".to_string()
        }
    });
    let director = SwarmDirector::new(planner, model);
    let budget = Budget::new(200_000, 200, 120_000);

    // Build the bridge against the live oracle. Cheap — it's just an Arc
    // clone + struct literal. Bug O: caller identity is now persistent
    // across requests within a session (and across app restarts) so audit
    // trails can correlate plans by caller. Loaded from
    // `~/.nexus/swarm_caller_identity.key` at AppState init.
    let bridge = Arc::new(SwarmOracleBridge::new(state.oracle()));
    let caller_arc = state.swarm_caller_identity();

    let planned = director
        .plan(
            &intent,
            &s.registry,
            &budget,
            caller_arc.as_ref(),
            bridge.as_ref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    let dag_json = planned.dag.to_json();
    let ticket_id = planned.ticket.ticket_id;
    let budget_hash = planned.ticket.budget_hash.clone();
    let privacy_envelope = privacy_label(planned.ticket.privacy_envelope).to_string();

    // Emit the legacy PlanProposed event for frontend back-compat; the
    // new OracleTicketIssued event is emitted by the coordinator on run()
    // so it always accompanies a live run.
    let _ = s.events.send(SwarmEvent::PlanProposed {
        run_id: ticket_id,
        dag_json: dag_json.clone(),
    });
    s.pending_plans.lock().await.insert(ticket_id, planned);

    Ok(PlannedSwarmJson {
        dag: dag_json,
        ticket_id,
        budget_hash,
        privacy_envelope,
    })
}

#[tauri::command]
pub async fn swarm_approve(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ticket_id: Uuid,
) -> Result<Uuid, String> {
    ensure_forwarder(app);
    let s = self::state();
    let planned = s
        .pending_plans
        .lock()
        .await
        .remove(&ticket_id)
        .ok_or_else(|| format!("no proposed plan with ticket_id {ticket_id}"))?;

    // Rebuild the bridge so the coordinator's high-risk checks + finalize
    // can reach the live oracle.
    let bridge = Arc::new(SwarmOracleBridge::new(state.oracle())) as Arc<dyn OracleBridge>;
    let coord = build_coordinator(bridge);
    let budget = Budget::new(200_000, 200, 300_000);
    let _ = s
        .events
        .send(SwarmEvent::PlanApproved { run_id: ticket_id });
    let handle = coord
        .run(planned, budget)
        .await
        .map_err(|e| e.to_string())?;
    let actual_id = handle.run_id;
    s.runs.lock().await.insert(actual_id, handle);
    Ok(actual_id)
}

#[tauri::command]
pub async fn swarm_reject(ticket_id: Uuid, reason: Option<String>) -> Result<(), String> {
    let s = state();
    s.pending_plans.lock().await.remove(&ticket_id);
    let _ = s.events.send(SwarmEvent::PlanRejected {
        run_id: ticket_id,
        reason: reason.unwrap_or_else(|| "user rejected".into()),
    });
    Ok(())
}

#[tauri::command]
pub async fn swarm_cancel(run_id: Uuid) -> Result<(), String> {
    let s = state();
    if let Some(h) = s.runs.lock().await.get(&run_id) {
        h.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn swarm_cancel_node(run_id: Uuid, node_id: String) -> Result<(), String> {
    // Phase 1: per-node cancel not implemented; treat as full-run cancel and
    // emit a node-scoped failure so the UI can reflect it.
    let s = state();
    if let Some(h) = s.runs.lock().await.get(&run_id) {
        h.cancel();
    }
    let _ = s.events.send(SwarmEvent::NodeFailed {
        r#ref: nexus_swarm::events::NodeRef { run_id, node_id },
        reason: "cancelled by user".into(),
        ticket_nonce: Uuid::nil(),
    });
    Ok(())
}

#[tauri::command]
pub async fn swarm_state(run_id: Uuid) -> Result<Value, String> {
    let s = state();
    let runs = s.runs.lock().await;
    let present = runs.contains_key(&run_id);
    Ok(serde_json::json!({
        "run_id": run_id,
        "present": present,
    }))
}

#[tauri::command]
pub async fn swarm_provider_health() -> Vec<ProviderHealth> {
    let s = state();
    let snap = s.health_snapshot.lock().await;
    snap.values().cloned().collect()
}

#[tauri::command]
pub async fn swarm_refresh_provider_health(
    app: tauri::AppHandle,
) -> Result<Vec<ProviderHealth>, String> {
    ensure_forwarder(app);
    let s = state();
    let mut futures = Vec::new();
    for p in s.providers.values() {
        let p = Arc::clone(p);
        futures.push(tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), p.health_check())
                .await
                .unwrap_or_else(|_| ProviderHealth {
                    provider_id: p.id().to_string(),
                    status: nexus_swarm::events::ProviderHealthStatus::Unhealthy,
                    latency_ms: None,
                    models: vec![],
                    notes: "timeout".into(),
                    checked_at_secs: 0,
                })
        }));
    }
    let mut results = Vec::new();
    for f in futures {
        if let Ok(h) = f.await {
            results.push(h);
        }
    }
    let mut snap = s.health_snapshot.lock().await;
    snap.clear();
    for h in &results {
        snap.insert(h.provider_id.clone(), h.clone());
    }
    drop(snap);
    let _ = s.events.send(SwarmEvent::ProviderHealthUpdate {
        providers: results.clone(),
    });
    Ok(results)
}

/// Return the oracle-anchored audit tail for a run. Read-only; the server
/// keeps these in memory for the life of the process. Empty vec if the
/// run_id was never seen.
#[tauri::command]
pub async fn swarm_audit_tail(run_id: Uuid) -> Result<Vec<AuditEntry>, String> {
    let s = state();
    let store = s.audit.lock().await;
    Ok(store.get(&run_id).cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_swarm::events::NodeRef;

    fn fresh_seq() -> std::sync::atomic::AtomicU64 {
        std::sync::atomic::AtomicU64::new(0)
    }

    #[test]
    fn event_to_audit_entry_node_started_populates_node_id() {
        // Track C #1: per-variant node_id extraction. NodeStarted
        // carries `r#ref.node_id`; the audit entry must surface it
        // so the frontend filter chip works without payload string-
        // parsing.
        let run_id = Uuid::new_v4();
        let nonce = Uuid::new_v4();
        let ev = SwarmEvent::NodeStarted {
            r#ref: NodeRef {
                run_id,
                node_id: "node-7".into(),
            },
            capability_id: "herald".into(),
            provider_id: "anthropic".into(),
            model_id: "claude-haiku".into(),
            ticket_nonce: nonce,
        };
        let seq = fresh_seq();
        let (out_run, entry) =
            event_to_audit_entry(&ev, &seq).expect("NodeStarted produces an audit entry");
        assert_eq!(out_run, run_id);
        assert_eq!(entry.event_kind, "node_started");
        assert_eq!(entry.node_id.as_deref(), Some("node-7"));
        assert_eq!(entry.ticket_nonce, nonce);
    }

    #[test]
    fn event_to_audit_entry_run_scoped_budget_update_has_no_node_id() {
        // Run-scoped BudgetUpdate (node_id: None on the event) must
        // produce an AuditEntry with node_id: None. Per-node
        // BudgetUpdates DO carry node_id; a separate test covers that.
        let run_id = Uuid::new_v4();
        let nonce = Uuid::new_v4();
        let ev = SwarmEvent::BudgetUpdate {
            run_id,
            tokens_remaining: 1000,
            cents_remaining: 250,
            wall_ms_remaining: 60_000,
            ticket_nonce: nonce,
            node_id: None,
            node_tokens_consumed: None,
            node_cost_cents_consumed: None,
        };
        let seq = fresh_seq();
        let (out_run, entry) =
            event_to_audit_entry(&ev, &seq).expect("BudgetUpdate produces an audit entry");
        assert_eq!(out_run, run_id);
        assert_eq!(entry.event_kind, "budget_update");
        assert_eq!(entry.node_id, None);
    }

    #[test]
    fn event_to_audit_entry_per_node_budget_update_carries_node_id() {
        // Per-node BudgetUpdate path (Phase 4a). The audit row must
        // surface the node_id so per-node budget activity is
        // filterable.
        let run_id = Uuid::new_v4();
        let nonce = Uuid::new_v4();
        let ev = SwarmEvent::BudgetUpdate {
            run_id,
            tokens_remaining: 800,
            cents_remaining: 200,
            wall_ms_remaining: 50_000,
            ticket_nonce: nonce,
            node_id: Some("node-3".into()),
            node_tokens_consumed: Some(50),
            node_cost_cents_consumed: Some(5),
        };
        let seq = fresh_seq();
        let (_, entry) =
            event_to_audit_entry(&ev, &seq).expect("per-node BudgetUpdate produces entry");
        assert_eq!(entry.node_id.as_deref(), Some("node-3"));
    }

    #[test]
    fn event_to_audit_entry_oracle_runtime_denial_carries_node_id() {
        // OracleRuntimeDenial always denies *for* a specific node.
        // The audit entry must expose that so denial-by-node filtering
        // doesn't require payload_summary parsing.
        let nonce = Uuid::new_v4();
        let ev = SwarmEvent::OracleRuntimeDenial {
            ticket_nonce: nonce,
            hints: vec!["budget".into(), "policy".into()],
            node_id: "node-9".into(),
        };
        let seq = fresh_seq();
        let (_, entry) =
            event_to_audit_entry(&ev, &seq).expect("OracleRuntimeDenial produces entry");
        assert_eq!(entry.event_kind, "oracle_runtime_denial");
        assert_eq!(entry.node_id.as_deref(), Some("node-9"));
    }
}
