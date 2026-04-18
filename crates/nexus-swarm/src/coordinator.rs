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
use crate::director::PlannedSwarm;
use crate::error::SwarmError;
use crate::events::{NodeRef, ProviderHealth, SwarmEvent};
use crate::oracle_bridge::{dag_content_hash, OracleBridge, SwarmSummary, SwarmTicket};
use crate::oracle_policy::{HighRiskEvent, HighRiskPolicy};
use crate::provider::Provider;
use crate::registry::CapabilityRegistry;
use crate::routing::{RouteCandidate, Router};
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
    /// Oracle bridge used for runtime high-risk checks and finalization.
    /// Concrete type varies — production wires `SwarmOracleBridge`; tests
    /// typically wire `NullSwarmOracleBridge`.
    pub bridge: Arc<dyn OracleBridge>,
    pub highrisk_policy: HighRiskPolicy,
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
        bridge: Arc<dyn OracleBridge>,
    ) -> Self {
        Self {
            events,
            router,
            registry,
            providers,
            health_snapshot,
            bridge,
            highrisk_policy: HighRiskPolicy::new(),
        }
    }

    /// Spawn the run loop and return immediately with a handle. Accepts a
    /// `PlannedSwarm` produced by `Director::plan` so the ticket is bound
    /// to the DAG that was approved.
    pub async fn run(
        self: Arc<Self>,
        planned: PlannedSwarm,
        budget: Budget,
    ) -> Result<SwarmRunHandle, SwarmError> {
        let run_id = Uuid::new_v4();
        let cancel = CancelToken::new();
        let handle = SwarmRunHandle {
            run_id,
            cancel: cancel.clone(),
        };

        // Surface the ticket identifiers to the event stream so any UI can
        // tie this run to an oracle audit entry. Full ticket body stays
        // server-side in `coord.run`'s scope.
        let _ = self.events.send(SwarmEvent::OracleTicketIssued {
            ticket_id: planned.ticket.ticket_id,
            budget_hash: planned.ticket.budget_hash.clone(),
            dag_content_hash: planned.ticket.dag_content_hash.clone(),
        });

        let coord = Arc::clone(&self);
        tokio::spawn(async move {
            let mut dag = planned.dag;
            let ticket = planned.ticket;
            let mut budget = budget;
            let initial_budget = budget;
            let mut summary = SwarmSummary {
                run_id,
                completed_nodes: 0,
                failed_nodes: 0,
                cancelled: false,
            };
            let cancelled = match coord
                .execute_loop(
                    run_id,
                    &mut dag,
                    &mut budget,
                    &initial_budget,
                    &ticket,
                    cancel.clone(),
                    &mut summary,
                )
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
                        ticket_nonce: ticket.nonce,
                    });
                    false
                }
            };
            summary.cancelled = cancelled;
            if cancelled {
                let _ = coord.events.send(SwarmEvent::SwarmCancelled { run_id });
            } else {
                let _ = coord.events.send(SwarmEvent::SwarmCompleted { run_id });
            }

            // Fire-and-forget finalize. The bridge logs on failure; we do
            // not await it because the coordinator's job is done.
            let bridge = Arc::clone(&coord.bridge);
            tokio::spawn(async move {
                bridge.finalize(ticket, summary).await;
            });
        });

        Ok(handle)
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_loop(
        &self,
        run_id: Uuid,
        dag: &mut ExecutionDag,
        budget: &mut Budget,
        initial_budget: &Budget,
        ticket: &SwarmTicket,
        cancel: CancelToken,
        summary: &mut SwarmSummary,
    ) -> Result<bool, SwarmError> {
        while !dag.is_complete() {
            if cancel.is_cancelled() {
                return Ok(true);
            }

            // Plan-drift check: the ticket captured the DAG shape at
            // approval time. Any structural mutation since then is a
            // governance violation.
            let current_hash = dag_content_hash(dag);
            if current_hash != ticket.dag_content_hash {
                let drift_event = HighRiskEvent::PlanDrift {
                    original_hash: ticket.dag_content_hash.clone(),
                    current_hash: current_hash.clone(),
                };
                self.run_highrisk_check(ticket, drift_event, "(plan-drift)")
                    .await?;
                return Err(SwarmError::OraclePolicyDenied {
                    hints: vec![format!(
                        "plan drift: approved hash {} ≠ current {current_hash}",
                        ticket.dag_content_hash
                    )],
                });
            }

            // Budget soft-limit check: once past 80% of the smallest
            // resource axis, pulse the oracle. The run continues on
            // approval; on denial, abort cleanly.
            let consumed_pct = budget_consumed_pct(initial_budget, budget);
            if consumed_pct >= HighRiskPolicy::BUDGET_SOFT_LIMIT_PCT {
                let event = HighRiskEvent::BudgetSoftLimitApproach { consumed_pct };
                self.run_highrisk_check(ticket, event, "(budget-soft-limit)")
                    .await?;
            }

            let ready = dag.ready_nodes();
            if ready.is_empty() {
                break;
            }
            for id in &ready {
                dag.mark_running(id);
            }

            let health_map = self.health_snapshot.lock().await.clone();
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
                        summary.failed_nodes += 1;
                        continue;
                    }
                };

                // Route resolution is now serial (before spawning) so we
                // can evaluate high-risk hooks against the resolved route
                // before committing the node to execution.
                let route = match self.router.resolve(
                    &node.capability_id,
                    &node.profile,
                    budget,
                    &health_map,
                ) {
                    Ok(r) => r,
                    Err(denied) => {
                        let _ = self.events.send(SwarmEvent::RouteDenied {
                            r#ref: NodeRef {
                                run_id,
                                node_id: node_id.clone(),
                            },
                            denied: denied.clone(),
                        });
                        dag.mark_failed_and_cascade(&node_id, denied.to_string());
                        let _ = self.events.send(SwarmEvent::NodeFailed {
                            r#ref: NodeRef {
                                run_id,
                                node_id: node_id.clone(),
                            },
                            reason: denied.to_string(),
                            ticket_nonce: ticket.nonce,
                        });
                        summary.failed_nodes += 1;
                        continue;
                    }
                };

                // High-risk: cloud-call cost threshold.
                if let Some(provider) = self.providers.get(&route.provider_id) {
                    let est_cents = estimate_route_cents(provider, &route);
                    let event = HighRiskEvent::CloudCallAboveThreshold {
                        provider_id: route.provider_id.clone(),
                        estimated_cents: est_cents,
                    };
                    if self.highrisk_policy.should_recheck(&event) {
                        match self.bridge.check_highrisk(ticket, event.clone()).await {
                            Ok(decision) => {
                                let _ = self.events.send(SwarmEvent::OracleRuntimeCheck {
                                    ticket_nonce: ticket.nonce,
                                    highrisk_event: event,
                                    decision,
                                });
                            }
                            Err(denial) => {
                                let _ = self.events.send(SwarmEvent::OracleRuntimeDenial {
                                    ticket_nonce: ticket.nonce,
                                    hints: denial.hints.clone(),
                                    node_id: node_id.clone(),
                                });
                                dag.mark_failed_and_cascade(
                                    &node_id,
                                    format!(
                                        "oracle denied cloud call: {}",
                                        denial.hints.join(", ")
                                    ),
                                );
                                let _ = self.events.send(SwarmEvent::NodeFailed {
                                    r#ref: NodeRef {
                                        run_id,
                                        node_id: node_id.clone(),
                                    },
                                    reason: denial.hints.join(", "),
                                    ticket_nonce: ticket.nonce,
                                });
                                summary.failed_nodes += 1;
                                continue;
                            }
                        }
                    }
                }

                let max_parallel = cap.descriptor().max_parallel.max(1);
                let sem = Arc::new(Semaphore::new(max_parallel as usize));
                let events = self.events.clone();
                let parent_outs = dag.parent_outputs(&node_id);
                let capability_id = node.capability_id.clone();
                let node_inputs = node.inputs.clone();
                let node_id_cloned = node_id.clone();
                let ticket_nonce = ticket.nonce;

                set.spawn(async move {
                    let _permit = sem.acquire_owned().await.ok();
                    let node_ref = NodeRef {
                        run_id,
                        node_id: node_id_cloned.clone(),
                    };

                    let _ = events.send(SwarmEvent::NodeStarted {
                        r#ref: node_ref.clone(),
                        capability_id: capability_id.clone(),
                        provider_id: route.provider_id.clone(),
                        model_id: route.model_id.clone(),
                        ticket_nonce,
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
                        summary.completed_nodes += 1;
                        let _ = self.events.send(SwarmEvent::NodeCompleted {
                            r#ref: NodeRef { run_id, node_id },
                            result: value,
                            ticket_nonce: ticket.nonce,
                        });
                    }
                    NodeOutcome::Failed { node_id, reason } => {
                        dag.mark_failed_and_cascade(&node_id, reason.clone());
                        summary.failed_nodes += 1;
                        let _ = self.events.send(SwarmEvent::NodeFailed {
                            r#ref: NodeRef { run_id, node_id },
                            reason,
                            ticket_nonce: ticket.nonce,
                        });
                    }
                }
                let _ = self.events.send(SwarmEvent::BudgetUpdate {
                    run_id,
                    tokens_remaining: budget.tokens,
                    cents_remaining: budget.cost_cents,
                    wall_ms_remaining: budget.wall_ms,
                    ticket_nonce: ticket.nonce,
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

    /// Issue a high-risk check and emit the corresponding event. Returns
    /// `Ok(())` on oracle approval; callers decide whether to propagate the
    /// denial (the `SwarmError` path) or just emit and keep running.
    async fn run_highrisk_check(
        &self,
        ticket: &SwarmTicket,
        event: HighRiskEvent,
        node_id: &str,
    ) -> Result<(), SwarmError> {
        match self.bridge.check_highrisk(ticket, event.clone()).await {
            Ok(decision) => {
                let _ = self.events.send(SwarmEvent::OracleRuntimeCheck {
                    ticket_nonce: ticket.nonce,
                    highrisk_event: event,
                    decision,
                });
                Ok(())
            }
            Err(denial) => {
                let _ = self.events.send(SwarmEvent::OracleRuntimeDenial {
                    ticket_nonce: ticket.nonce,
                    hints: denial.hints.clone(),
                    node_id: node_id.to_string(),
                });
                Err(SwarmError::OraclePolicyDenied {
                    hints: denial.hints,
                })
            }
        }
    }
}

fn budget_consumed_pct(initial: &Budget, current: &Budget) -> u8 {
    let tokens_pct = pct_consumed_u64(initial.tokens, current.tokens);
    let cost_pct = pct_consumed_u64(initial.cost_cents as u64, current.cost_cents as u64);
    let wall_pct = pct_consumed_u64(initial.wall_ms, current.wall_ms);
    tokens_pct.max(cost_pct).max(wall_pct)
}

fn pct_consumed_u64(initial: u64, remaining: u64) -> u8 {
    if initial == 0 {
        return 0;
    }
    let consumed = initial.saturating_sub(remaining);
    let pct = (consumed.saturating_mul(100)) / initial;
    pct.min(100) as u8
}

fn estimate_route_cents(provider: &Arc<dyn Provider>, _route: &RouteCandidate) -> u32 {
    // Phase 1.5b: we have no per-node InvokeRequest at routing time, so we
    // estimate against a canonical 2K-token invocation. Providers that
    // override `estimate_cents` with actual pricing will return their
    // realistic number; the default heuristic scales by cost class.
    let req = crate::provider::InvokeRequest {
        model_id: String::new(),
        prompt: String::new(),
        max_tokens: 2_048,
        temperature: None,
        metadata: serde_json::Value::Null,
    };
    provider.estimate_cents(&req)
}

enum NodeOutcome {
    Done { node_id: String, value: Value },
    Failed { node_id: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{AgentCapabilityDescriptor, CapabilityInvocation, SwarmCapability};
    use crate::oracle_bridge::{most_restrictive_privacy, SwarmTicket};
    use crate::oracle_bridge::{testing::NullSwarmOracleBridge, OracleBridge};
    use std::time::SystemTime;

    fn test_bridge() -> Arc<dyn OracleBridge> {
        Arc::new(NullSwarmOracleBridge::new())
    }

    fn plan_dag(dag: ExecutionDag) -> PlannedSwarm {
        // Fabricate a matching ticket for coordinator-only tests; in
        // production the director produces this via the bridge.
        let dag_content_hash = crate::oracle_bridge::dag_content_hash(&dag);
        let privacy_envelope = most_restrictive_privacy(&dag);
        let ticket = SwarmTicket {
            ticket_id: Uuid::new_v4(),
            nonce: Uuid::new_v4(),
            budget_hash: String::new(),
            privacy_envelope,
            dag_content_hash,
            issued_at: SystemTime::now(),
            token: nexus_governance_oracle::SealedToken {
                payload: vec![],
                signature: vec![],
                token_id: Uuid::nil().to_string(),
            },
        };
        PlannedSwarm { dag, ticket }
    }
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
            test_bridge(),
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
        let _h = coord
            .run(plan_dag(dag), Budget::unlimited_for_tests())
            .await
            .unwrap();
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
            test_bridge(),
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

        let _h = coord
            .run(plan_dag(dag), Budget::unlimited_for_tests())
            .await
            .unwrap();
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
            test_bridge(),
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
        let _h = coord
            .run(plan_dag(dag), Budget::unlimited_for_tests())
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
}
