//! SwarmCoordinator — runs a DAG to completion, respecting parallelism
//! limits, budgets, routing decisions, and governance.
//!
//! Each ready node is spawned as a tokio task gated by a semaphore sized to
//! the capability's `max_parallel`. The route is resolved through the
//! `Router` using the current provider-health snapshot. On `RouteDenied` the
//! node is marked Failed and descendants cascade to Skipped. On successful
//! `Provider` invocation the adapter owns the provider call itself — the
//! coordinator passes the chosen (provider_id, model_id) through the
//! `CapabilityInvocation.inputs.route` so the adapter can look the provider
//! up in its own held registry.

use crate::budget::Budget;
use crate::dag::{DagNodeStatus, ExecutionDag};
use crate::error::SwarmError;
use crate::events::{NodeRef, ProviderHealth, SwarmEvent};
use crate::provider::Provider;
use crate::registry::CapabilityRegistry;
use crate::routing::{RouteDenied, Router};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, Mutex, Semaphore};
use tokio::task::JoinSet;
use uuid::Uuid;

/// A tiny cancellation token built on `watch::channel(bool)` — we avoid
/// pulling in `tokio-util` just for this.
#[derive(Clone)]
pub struct CancelToken {
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl CancelToken {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }
    pub fn cancel(&self) {
        let _ = self.tx.send(true);
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SwarmCoordinator {
    pub events: broadcast::Sender<SwarmEvent>,
    pub router: Arc<Router>,
    pub registry: Arc<CapabilityRegistry>,
    pub providers: Arc<HashMap<String, Arc<dyn Provider>>>,
    pub health_snapshot: Arc<Mutex<HashMap<String, ProviderHealth>>>,
}

pub struct SwarmRunHandle {
    pub run_id: Uuid,
    cancel: CancelToken,
}

impl SwarmRunHandle {
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    pub fn token(&self) -> CancelToken {
        self.cancel.clone()
    }
}

impl SwarmCoordinator {
    pub fn new(
        registry: Arc<CapabilityRegistry>,
        router: Arc<Router>,
        providers: Arc<HashMap<String, Arc<dyn Provider>>>,
        health_snapshot: Arc<Mutex<HashMap<String, ProviderHealth>>>,
        events: broadcast::Sender<SwarmEvent>,
    ) -> Self {
        Self {
            events,
            router,
            registry,
            providers,
            health_snapshot,
        }
    }

    /// Spawn the run loop and return immediately with a handle.
    pub async fn run(
        self: Arc<Self>,
        dag: ExecutionDag,
        budget: Budget,
    ) -> Result<SwarmRunHandle, SwarmError> {
        let run_id = Uuid::new_v4();
        let cancel = CancelToken::new();
        let handle = SwarmRunHandle {
            run_id,
            cancel: cancel.clone(),
        };

        let coord = Arc::clone(&self);
        tokio::spawn(async move {
            let mut dag = dag;
            let mut budget = budget;
            let cancelled = match coord
                .execute_loop(run_id, &mut dag, &mut budget, cancel.clone())
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = coord.events.send(SwarmEvent::NodeFailed {
                        r#ref: NodeRef {
                            run_id,
                            node_id: "(coordinator)".into(),
                        },
                        reason: e.to_string(),
                    });
                    false
                }
            };
            if cancelled {
                let _ = coord.events.send(SwarmEvent::SwarmCancelled { run_id });
            } else {
                let _ = coord.events.send(SwarmEvent::SwarmCompleted { run_id });
            }
        });

        Ok(handle)
    }

    async fn execute_loop(
        &self,
        run_id: Uuid,
        dag: &mut ExecutionDag,
        budget: &mut Budget,
        cancel: CancelToken,
    ) -> Result<bool, SwarmError> {
        while !dag.is_complete() {
            if cancel.is_cancelled() {
                return Ok(true);
            }

            let ready = dag.ready_nodes();
            if ready.is_empty() {
                break;
            }
            for id in &ready {
                dag.mark_running(id);
            }

            let mut set: JoinSet<NodeOutcome> = JoinSet::new();
            for node_id in ready {
                let node = match dag.get(&node_id).cloned() {
                    Some(n) => n,
                    None => continue,
                };
                let cap = match self.registry.get(&node.capability_id) {
                    Some(c) => c,
                    None => {
                        dag.mark_failed_and_cascade(
                            &node_id,
                            format!("capability `{}` not registered", node.capability_id),
                        );
                        continue;
                    }
                };
                let max_parallel = cap.descriptor().max_parallel.max(1);
                let sem = Arc::new(Semaphore::new(max_parallel as usize));
                let events = self.events.clone();
                let router = Arc::clone(&self.router);
                let health = Arc::clone(&self.health_snapshot);
                let parent_outs = dag.parent_outputs(&node_id);
                let budget_snapshot = *budget;
                let profile = node.profile;
                let capability_id = node.capability_id.clone();
                let node_inputs = node.inputs.clone();
                let node_id_cloned = node_id.clone();

                set.spawn(async move {
                    let _permit = sem.acquire_owned().await.ok();
                    let node_ref = NodeRef {
                        run_id,
                        node_id: node_id_cloned.clone(),
                    };

                    let health_map = health.lock().await.clone();
                    let route = match router.resolve(
                        &capability_id,
                        &profile,
                        &budget_snapshot,
                        &health_map,
                    ) {
                        Ok(r) => r,
                        Err(denied) => {
                            let _ = events.send(SwarmEvent::RouteDenied {
                                r#ref: node_ref.clone(),
                                denied: denied.clone(),
                            });
                            return NodeOutcome::RouteDenied {
                                node_id: node_id_cloned,
                                denied,
                            };
                        }
                    };

                    let _ = events.send(SwarmEvent::NodeStarted {
                        r#ref: node_ref.clone(),
                        capability_id: capability_id.clone(),
                        provider_id: route.provider_id.clone(),
                        model_id: route.model_id.clone(),
                    });

                    let invocation = crate::capability::CapabilityInvocation {
                        inputs: serde_json::json!({
                            "node_inputs": node_inputs,
                            "route": {
                                "provider_id": route.provider_id,
                                "model_id": route.model_id,
                            },
                        }),
                        parent_outputs: parent_outs,
                    };
                    match cap.run(invocation).await {
                        Ok(v) => NodeOutcome::Done {
                            node_id: node_id_cloned,
                            value: v,
                        },
                        Err(e) => NodeOutcome::Failed {
                            node_id: node_id_cloned,
                            reason: e.to_string(),
                        },
                    }
                });
            }

            while let Some(j) = set.join_next().await {
                let outcome = match j {
                    Ok(o) => o,
                    Err(je) => {
                        tracing::error!(
                            target: "nexus_swarm::coordinator",
                            "join error: {je}"
                        );
                        continue;
                    }
                };
                match outcome {
                    NodeOutcome::Done { node_id, value } => {
                        dag.mark_done(&node_id, value.clone());
                        let _ = self.events.send(SwarmEvent::NodeCompleted {
                            r#ref: NodeRef { run_id, node_id },
                            result: value,
                        });
                    }
                    NodeOutcome::Failed { node_id, reason } => {
                        dag.mark_failed_and_cascade(&node_id, reason.clone());
                        let _ = self.events.send(SwarmEvent::NodeFailed {
                            r#ref: NodeRef { run_id, node_id },
                            reason,
                        });
                    }
                    NodeOutcome::RouteDenied { node_id, denied } => {
                        dag.mark_failed_and_cascade(&node_id, denied.to_string());
                        let _ = self.events.send(SwarmEvent::NodeFailed {
                            r#ref: NodeRef { run_id, node_id },
                            reason: denied.to_string(),
                        });
                    }
                }
                let _ = self.events.send(SwarmEvent::BudgetUpdate {
                    run_id,
                    tokens_remaining: budget.tokens,
                    cents_remaining: budget.cost_cents,
                    wall_ms_remaining: budget.wall_ms,
                });
            }

            let any_runnable = dag.ready_nodes().iter().any(|id| {
                dag.get(id)
                    .map(|n| matches!(n.status, DagNodeStatus::Pending | DagNodeStatus::Ready))
                    .unwrap_or(false)
            });
            if !any_runnable && !dag.is_complete() {
                break;
            }
        }
        Ok(cancel.is_cancelled())
    }
}

enum NodeOutcome {
    Done {
        node_id: String,
        value: Value,
    },
    Failed {
        node_id: String,
        reason: String,
    },
    RouteDenied {
        node_id: String,
        denied: RouteDenied,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
    use crate::dag::DagNode;
    use crate::events::ProviderHealthStatus;
    use crate::profile::{CostClass, PrivacyClass, TaskProfile};
    use crate::provider::{
        InvokeRequest, InvokeResponse, ModelDescriptor, ProviderCapabilities, ProviderError,
    };
    use crate::routing::{RouteCandidate, RoutingPolicy};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Noop {
        id: String,
        counter: Arc<AtomicU32>,
    }

    #[async_trait]
    impl SwarmCapability for Noop {
        fn descriptor(&self) -> AgentCapabilityDescriptor {
            AgentCapabilityDescriptor {
                id: self.id.clone(),
                name: self.id.clone(),
                role: "test".into(),
                task_profile_default: TaskProfile::local_light(),
                input_schema: serde_json::json!({}),
                output_schema: serde_json::json!({}),
                max_parallel: 4,
                cost_class: CostClass::Free,
                todo_reason: None,
            }
        }
        async fn run(&self, _: CapabilityInvocation) -> Result<serde_json::Value, SwarmError> {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"k": self.id}))
        }
    }

    struct Failing;
    #[async_trait]
    impl SwarmCapability for Failing {
        fn descriptor(&self) -> AgentCapabilityDescriptor {
            AgentCapabilityDescriptor {
                id: "fail".into(),
                name: "fail".into(),
                role: "test".into(),
                task_profile_default: TaskProfile::local_light(),
                input_schema: serde_json::json!({}),
                output_schema: serde_json::json!({}),
                max_parallel: 1,
                cost_class: CostClass::Free,
                todo_reason: None,
            }
        }
        async fn run(&self, _: CapabilityInvocation) -> Result<serde_json::Value, SwarmError> {
            Err(SwarmError::DirectorParse("boom".into()))
        }
    }

    struct LocalMock;
    #[async_trait]
    impl Provider for LocalMock {
        fn id(&self) -> &str {
            "ollama"
        }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                models: vec![ModelDescriptor {
                    id: "m".into(),
                    param_count_b: Some(7),
                    tier: crate::profile::ReasoningTier::Medium,
                    context_window: 8192,
                }],
                supports_tool_use: false,
                supports_streaming: false,
                max_context: 8192,
                cost_class: CostClass::Free,
                privacy_class: PrivacyClass::StrictLocal,
            }
        }
        async fn health_check(&self) -> ProviderHealth {
            unreachable!()
        }
        async fn invoke(&self, _: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
            Ok(InvokeResponse {
                text: String::new(),
                tokens_in: 0,
                tokens_out: 0,
                cost_cents: 0,
                latency_ms: 0,
                model_id: "m".into(),
            })
        }
    }

    fn ready_router(agents: &[&str]) -> (Arc<Router>, Arc<Mutex<HashMap<String, ProviderHealth>>>) {
        let mut r = Router::new();
        r.register_provider(Arc::new(LocalMock));
        for a in agents {
            r.set_policy(RoutingPolicy {
                agent_id: (*a).into(),
                preference_order: vec![RouteCandidate {
                    provider_id: "ollama".into(),
                    model_id: "m".into(),
                    est_cost_cents: 0,
                }],
            });
        }
        let mut h = HashMap::new();
        h.insert(
            "ollama".into(),
            ProviderHealth {
                provider_id: "ollama".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(1),
                models: vec!["m".into()],
                notes: String::new(),
                checked_at_secs: 0,
            },
        );
        (Arc::new(r), Arc::new(Mutex::new(h)))
    }

    fn empty_providers() -> Arc<HashMap<String, Arc<dyn Provider>>> {
        Arc::new(HashMap::new())
    }

    #[tokio::test]
    async fn parallelism_under_500ms_for_four_50ms_nodes() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut reg = CapabilityRegistry::new();
        for id in ["a", "b", "c", "d"] {
            reg.register(Arc::new(Noop {
                id: id.into(),
                counter: Arc::clone(&counter),
            }));
        }
        let reg = Arc::new(reg);
        let (router, health) = ready_router(&["a", "b", "c", "d"]);
        let (tx, _rx) = broadcast::channel(256);
        let coord = Arc::new(SwarmCoordinator::new(
            reg,
            router,
            empty_providers(),
            health,
            tx,
        ));

        let mut dag = ExecutionDag::new();
        for id in ["a", "b", "c", "d"] {
            dag.add_node(DagNode {
                id: id.into(),
                capability_id: id.into(),
                profile: TaskProfile::local_light(),
                inputs: serde_json::json!({}),
                status: DagNodeStatus::Pending,
            })
            .unwrap();
        }

        let start = std::time::Instant::now();
        let _h = coord.run(dag, Budget::unlimited_for_tests()).await.unwrap();
        for _ in 0..60 {
            if counter.load(Ordering::SeqCst) == 4 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let elapsed = start.elapsed();
        assert_eq!(counter.load(Ordering::SeqCst), 4);
        assert!(
            elapsed.as_millis() < 500,
            "expected parallel exec, got {}ms",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn failure_cascades_to_descendants() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(Failing));
        let counter = Arc::new(AtomicU32::new(0));
        reg.register(Arc::new(Noop {
            id: "child".into(),
            counter: Arc::clone(&counter),
        }));
        let reg = Arc::new(reg);
        let (router, health) = ready_router(&["fail", "child"]);
        let (tx, _rx) = broadcast::channel(256);
        let coord = Arc::new(SwarmCoordinator::new(
            reg,
            router,
            empty_providers(),
            health,
            tx,
        ));

        let mut dag = ExecutionDag::new();
        dag.add_node(DagNode {
            id: "root".into(),
            capability_id: "fail".into(),
            profile: TaskProfile::local_light(),
            inputs: serde_json::json!({}),
            status: DagNodeStatus::Pending,
        })
        .unwrap();
        dag.add_node(DagNode {
            id: "leaf".into(),
            capability_id: "child".into(),
            profile: TaskProfile::local_light(),
            inputs: serde_json::json!({}),
            status: DagNodeStatus::Pending,
        })
        .unwrap();
        dag.add_edge("root", "leaf").unwrap();

        let _h = coord.run(dag, Budget::unlimited_for_tests()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0, "child must never run");
    }

    #[tokio::test]
    async fn route_denied_produces_node_failed_without_running_cap() {
        let counter = Arc::new(AtomicU32::new(0));
        let mut reg = CapabilityRegistry::new();
        reg.register(Arc::new(Noop {
            id: "alpha".into(),
            counter: Arc::clone(&counter),
        }));
        let reg = Arc::new(reg);
        // Router with a policy that names an unregistered provider.
        let mut router = Router::new();
        router.set_policy(RoutingPolicy {
            agent_id: "alpha".into(),
            preference_order: vec![RouteCandidate {
                provider_id: "ghost".into(),
                model_id: "m".into(),
                est_cost_cents: 0,
            }],
        });
        let health = Arc::new(Mutex::new(HashMap::new()));
        let (tx, _rx) = broadcast::channel(256);
        let coord = Arc::new(SwarmCoordinator::new(
            reg,
            Arc::new(router),
            empty_providers(),
            health,
            tx,
        ));
        let mut dag = ExecutionDag::new();
        dag.add_node(DagNode {
            id: "n".into(),
            capability_id: "alpha".into(),
            profile: TaskProfile::local_light(),
            inputs: serde_json::json!({}),
            status: DagNodeStatus::Pending,
        })
        .unwrap();
        let _h = coord.run(dag, Budget::unlimited_for_tests()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
