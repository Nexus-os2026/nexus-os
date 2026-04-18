//! Swarm Tauri commands — surface the `nexus-swarm` crate to the frontend.
//!
//! Eight commands:
//!   - `swarm_plan` — intent → ExecutionDag (as JSON)
//!   - `swarm_approve` — start running an approved DAG
//!   - `swarm_reject` — discard a proposed DAG
//!   - `swarm_cancel` — stop an active run
//!   - `swarm_cancel_node` — cancel a single node (Phase 1: equivalent to
//!     cancelling the whole run; per-node cancel lands in Phase 2)
//!   - `swarm_state` — fetch run state snapshot
//!   - `swarm_provider_health` — cached health snapshot
//!   - `swarm_refresh_provider_health` — probe every provider afresh
//!
//! All events are relayed through the single channel `"swarm:event"` with
//! tagged JSON. The forwarder task that bridges the broadcast → Tauri emit
//! is spawned lazily on first use.

use nexus_swarm::adapters::{
    ArtisanAdapter, BrokerAdapter, HeraldAdapter, ProspectorStub, ScoutStub, WatchdogStub,
};
use nexus_swarm::events::{ProviderHealth, SwarmEvent};
use nexus_swarm::provider::Provider;
use nexus_swarm::providers::{
    AnthropicProvider, CodexCliProvider, HuggingFaceProvider, OllamaSwarmProvider,
    OpenAiSwarmProvider, OpenRouterSwarmProvider,
};
use nexus_swarm::{
    Budget, CapabilityRegistry, ExecutionDag, Router, SwarmCoordinator, SwarmDirector,
    SwarmRunHandle,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::Emitter;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

struct SwarmInner {
    registry: Arc<CapabilityRegistry>,
    router: Arc<Router>,
    providers: Arc<HashMap<String, Arc<dyn Provider>>>,
    health_snapshot: Arc<Mutex<HashMap<String, ProviderHealth>>>,
    events: broadcast::Sender<SwarmEvent>,
    runs: Arc<Mutex<HashMap<Uuid, SwarmRunHandle>>>,
    pending_plans: Arc<Mutex<HashMap<Uuid, ExecutionDag>>>,
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

            let mut registry = CapabilityRegistry::new();
            registry.register(Arc::new(ArtisanAdapter::new(Arc::clone(&providers))));
            registry.register(Arc::new(HeraldAdapter::new(Arc::clone(&providers))));
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
    tauri::async_runtime::spawn(async move {
        while let Ok(ev) = rx.recv().await {
            let _ = app.emit("swarm:event", &ev);
        }
    });
}

fn build_coordinator() -> Arc<SwarmCoordinator> {
    let s = state();
    Arc::new(SwarmCoordinator::new(
        Arc::clone(&s.registry),
        Arc::clone(&s.router),
        Arc::clone(&s.providers),
        Arc::clone(&s.health_snapshot),
        s.events.clone(),
    ))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProposedPlan {
    pub run_id: Uuid,
    pub dag: Value,
}

fn director_model() -> Option<String> {
    std::env::var("NEXUS_SWARM_DIRECTOR_MODEL").ok()
}

#[tauri::command]
pub async fn swarm_plan(intent: String) -> Result<ProposedPlan, String> {
    let s = state();
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
        // Sensible default when planner is ollama.
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
    let dag = director
        .plan(&intent, &s.registry, &budget)
        .await
        .map_err(|e| e.to_string())?;
    let run_id = Uuid::new_v4();
    let snapshot = dag.to_json();
    s.pending_plans.lock().await.insert(run_id, dag);
    let _ = s.events.send(SwarmEvent::PlanProposed {
        run_id,
        dag_json: snapshot.clone(),
    });
    Ok(ProposedPlan {
        run_id,
        dag: snapshot,
    })
}

#[tauri::command]
pub async fn swarm_approve(app: tauri::AppHandle, run_id: Uuid) -> Result<Uuid, String> {
    ensure_forwarder(app);
    let s = state();
    let dag = s
        .pending_plans
        .lock()
        .await
        .remove(&run_id)
        .ok_or_else(|| format!("no proposed plan with id {run_id}"))?;
    let coord = build_coordinator();
    let budget = Budget::new(200_000, 200, 300_000);
    let _ = s.events.send(SwarmEvent::PlanApproved { run_id });
    let handle = coord.run(dag, budget).await.map_err(|e| e.to_string())?;
    let actual_id = handle.run_id;
    s.runs.lock().await.insert(actual_id, handle);
    Ok(actual_id)
}

#[tauri::command]
pub async fn swarm_reject(run_id: Uuid, reason: Option<String>) -> Result<(), String> {
    let s = state();
    s.pending_plans.lock().await.remove(&run_id);
    let _ = s.events.send(SwarmEvent::PlanRejected {
        run_id,
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
