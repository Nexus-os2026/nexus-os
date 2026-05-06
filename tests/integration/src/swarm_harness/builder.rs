//! `ScenarioBuilder` + `Scenario` — the fluent assembly layer scenario
//! tests use to wire a fresh `Director` + `CapabilityRegistry` + null
//! oracle bridge in one expression.

use crate::swarm_harness::capabilities::SyntheticCapability;
use crate::swarm_harness::herald_harness::HeraldHarnessCapability;
use crate::swarm_harness::oracles::oracle_returning;
use crate::swarm_harness::providers::SyntheticPlannerProvider;
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_governance_oracle::GovernanceDecision;
use nexus_kernel::audit::AuditTrail;
use nexus_swarm::dag::ExecutionDag;
use nexus_swarm::events::{ProviderHealth, ProviderHealthStatus, SwarmEvent};
use nexus_swarm::oracle_bridge::testing::NullSwarmOracleBridge;
use nexus_swarm::oracle_bridge::OracleBridge;
use nexus_swarm::provider::Provider;
use nexus_swarm::routing::{RouteCandidate, Router, RoutingPolicy};
use nexus_swarm::{
    Budget, CapabilityRegistry, Director, PlannedSwarm, SwarmCoordinator, SwarmError,
    SwarmRunHandle,
};
use social_poster_agent::swarm_entry::PublishExecutor;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Default swarm-event broadcast channel depth. Generous so scenarios
/// don't drop events under load even when no receiver is draining.
const SCENARIO_EVENT_CHANNEL_DEPTH: usize = 256;

/// Default planner model id used in `InvokeRequest.model_id`. Must be
/// present in the synthetic provider's `ModelDescriptor` list (it is —
/// `SyntheticPlannerProvider` advertises this id).
const SYNTHETIC_PLANNER_MODEL_ID: &str = "synthetic-planner";

/// Provider id of the single synthetic provider the harness wires for
/// both planning and node-execution routing. Mirrors
/// `SyntheticPlannerProvider::id`.
const SYNTHETIC_PROVIDER_ID: &str = "synthetic-planner";

/// Type alias for a DAG mutator closure stored on the builder.
type DagMutator = Arc<dyn Fn(&mut ExecutionDag) + Send + Sync>;

/// Fluent builder for a `Scenario`.
///
/// Defaults: `NullSwarmOracleBridge`, `Budget::unlimited_for_tests`, a
/// freshly-generated Ed25519 `CryptoIdentity`, and an empty
/// `CapabilityRegistry`. Callers register capabilities by chaining
/// `.with_synthetic_capability(name)`.
pub struct ScenarioBuilder {
    canned_plan_json: Option<String>,
    capability_names: Vec<String>,
    budget: Budget,
    oracle_decision: Option<GovernanceDecision>,
    dag_mutator: Option<DagMutator>,
    /// Bug BL.2: scripted PublishExecutor for the
    /// "herald" capability. When Some, build()
    /// registers a HeraldHarnessCapability instead of
    /// a SyntheticCapability for capability_id="herald".
    publish_executor: Option<Arc<dyn PublishExecutor>>,
}

impl ScenarioBuilder {
    pub fn new() -> Self {
        Self {
            canned_plan_json: None,
            capability_names: Vec::new(),
            budget: Budget::unlimited_for_tests(),
            oracle_decision: None,
            dag_mutator: None,
            publish_executor: None,
        }
    }

    /// Provide the canned `PlanSchema` JSON the synthetic planner
    /// should return on every Director::plan invocation.
    pub fn with_canned_plan(mut self, plan_json: impl Into<String>) -> Self {
        self.canned_plan_json = Some(plan_json.into());
        self
    }

    /// Register one synthetic capability under `name`. The Director's
    /// canned plan must reference this exact `capability_id` for the
    /// plan to validate.
    pub fn with_synthetic_capability(mut self, name: impl Into<String>) -> Self {
        self.capability_names.push(name.into());
        self
    }

    /// Override the default unlimited test budget. B.1 doesn't exercise
    /// budget arithmetic; B.2 will.
    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// Replace the default `NullSwarmOracleBridge` with a real
    /// `SwarmOracleBridge` whose backing `GovernanceOracle` always
    /// returns `decision`. Use `GovernanceDecision::Approved {...}`
    /// for happy-path scenarios and `GovernanceDecision::Denied` for
    /// denial scenarios.
    pub fn with_oracle_decision(mut self, decision: GovernanceDecision) -> Self {
        self.oracle_decision = Some(decision);
        self
    }

    /// Register a DAG mutator that runs in `Scenario::run` AFTER
    /// `Director::plan` returns the approved `PlannedSwarm` but BEFORE
    /// the coordinator sees it. The mutation invalidates the
    /// approved `ticket.dag_content_hash`, which the coordinator's
    /// per-iteration drift check detects and surfaces as a
    /// `HighRiskEvent::PlanDrift`.
    ///
    /// The closure runs once per `Scenario::run` call. It takes
    /// `&mut ExecutionDag`; use the public mutation API
    /// (`add_node`, `get_mut`, `add_edge`) to invalidate the hash.
    pub fn with_dag_mutator<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut ExecutionDag) + Send + Sync + 'static,
    {
        self.dag_mutator = Some(Arc::new(f));
        self
    }

    /// Bug BL.2: register the "herald" capability with
    /// a scripted PublishExecutor. The capability runs
    /// through SocialPosterEntry::with_publish_executor
    /// wrapped in RetryingPublishExecutor (BK.2/BK.3),
    /// so retry_attempt NodeEvents fire on retries.
    /// Adds "herald" to the capability list if not
    /// already present.
    pub fn with_publish_capability(mut self, executor: Arc<dyn PublishExecutor>) -> Self {
        self.publish_executor = Some(executor);
        if !self.capability_names.iter().any(|n| n == "herald") {
            self.capability_names.push("herald".into());
        }
        self
    }

    /// Build the scenario. Panics if no canned plan was provided —
    /// scenario tests are deterministic by construction.
    pub fn build(self) -> Scenario {
        let canned_plan_json = self
            .canned_plan_json
            .expect("ScenarioBuilder::build called without with_canned_plan");

        let planner_provider =
            Arc::new(SyntheticPlannerProvider::with_canned_plan(canned_plan_json));
        let director = Director::new(
            Arc::clone(&planner_provider) as Arc<dyn Provider>,
            SYNTHETIC_PLANNER_MODEL_ID.to_string(),
        );

        let mut registry = CapabilityRegistry::new();
        let capability_call_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut providers: HashMap<String, Arc<dyn Provider>> = HashMap::new();
        providers.insert(
            planner_provider.id().to_string(),
            Arc::clone(&planner_provider) as Arc<dyn Provider>,
        );

        let mut router = Router::new();
        router.register_provider(Arc::clone(&planner_provider) as Arc<dyn Provider>);

        for name in &self.capability_names {
            // Bug BL.2: route capability_id="herald" to
            // HeraldHarnessCapability when a scripted
            // PublishExecutor is present. The herald
            // capability's observability flows through
            // NodeEvent emission (BK.3), not the
            // capability_call_log; non-herald entries
            // continue to use SyntheticCapability with
            // the shared log.
            if name == "herald" {
                if let Some(executor) = self.publish_executor.as_ref() {
                    registry.register(Arc::new(HeraldHarnessCapability::new(Arc::clone(executor))));
                } else {
                    registry.register(Arc::new(SyntheticCapability::with_shared_log(
                        name.clone(),
                        Arc::clone(&capability_call_log),
                    )));
                }
            } else {
                registry.register(Arc::new(SyntheticCapability::with_shared_log(
                    name.clone(),
                    Arc::clone(&capability_call_log),
                )));
            }
            // Every capability routes through the synthetic provider.
            // Free cost class + StrictLocal privacy on the provider
            // means the Router accepts any local_light task profile
            // without privacy or budget denial.
            router.set_policy(RoutingPolicy {
                agent_id: name.clone(),
                preference_order: vec![RouteCandidate {
                    provider_id: SYNTHETIC_PROVIDER_ID.to_string(),
                    model_id: SYNTHETIC_PLANNER_MODEL_ID.to_string(),
                    est_cost_cents: 0,
                }],
            });
        }

        let mut health_map = HashMap::new();
        health_map.insert(
            SYNTHETIC_PROVIDER_ID.to_string(),
            ProviderHealth {
                provider_id: SYNTHETIC_PROVIDER_ID.to_string(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(0),
                models: vec![SYNTHETIC_PLANNER_MODEL_ID.to_string()],
                notes: String::new(),
                checked_at_secs: 0,
            },
        );
        let health_snapshot = Arc::new(tokio::sync::Mutex::new(health_map));

        let bridge: Arc<dyn OracleBridge> = match self.oracle_decision {
            Some(decision) => oracle_returning(decision),
            None => Arc::new(NullSwarmOracleBridge::new()),
        };

        let caller = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).expect("Ed25519 keygen");
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let (events_tx, _events_rx) = broadcast::channel(SCENARIO_EVENT_CHANNEL_DEPTH);

        Scenario {
            director,
            planner_provider,
            registry: Arc::new(registry),
            providers,
            router: Arc::new(router),
            health_snapshot,
            bridge,
            budget: self.budget,
            caller,
            audit,
            events_tx,
            capability_call_log,
            dag_mutator: self.dag_mutator,
        }
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A fully-wired scenario ready to drive `Director::plan` and
/// `SwarmCoordinator::run`.
pub struct Scenario {
    pub director: Director,
    /// Held so tests can observe planner invocation counts.
    pub planner_provider: Arc<SyntheticPlannerProvider>,
    pub registry: Arc<CapabilityRegistry>,
    pub providers: HashMap<String, Arc<dyn Provider>>,
    pub router: Arc<Router>,
    pub health_snapshot: Arc<tokio::sync::Mutex<HashMap<String, ProviderHealth>>>,
    pub bridge: Arc<dyn OracleBridge>,
    pub budget: Budget,
    pub caller: CryptoIdentity,
    pub audit: Arc<Mutex<AuditTrail>>,
    pub events_tx: broadcast::Sender<SwarmEvent>,
    /// Shared log every registered `SyntheticCapability` writes its
    /// name into when invoked. Order reflects the coordinator's
    /// topological execution.
    pub capability_call_log: Arc<Mutex<Vec<String>>>,
    /// Optional DAG mutator. Applied in `run()` after
    /// `Director::plan` returns the approved `PlannedSwarm` but
    /// before the coordinator sees it. Used by scenario D to
    /// trigger plan-drift detection.
    pub dag_mutator: Option<DagMutator>,
}

impl Scenario {
    /// Run `Director::plan` against the wired registry/budget/caller/
    /// bridge. Mirrors the exact signature of `Director::plan`.
    pub async fn plan(&self, intent: &str) -> Result<PlannedSwarm, SwarmError> {
        self.director
            .plan(
                intent,
                &self.registry,
                &self.budget,
                &self.caller,
                &*self.bridge,
            )
            .await
    }

    /// Execute an approved `PlannedSwarm` through a freshly-constructed
    /// `SwarmCoordinator`. Mirrors `SwarmCoordinator::run`. The
    /// coordinator's `events` channel is the same `events_tx` exposed
    /// on `Scenario`, so a receiver subscribed before this call sees
    /// every event the run emits.
    ///
    /// If a `dag_mutator` was registered on the builder, it runs here
    /// — between `Director::plan` (which returned `planned` against
    /// the original DAG) and `SwarmCoordinator::run` (which will see
    /// the mutated DAG). The coordinator's drift check then surfaces
    /// the divergence.
    pub async fn run(&self, mut planned: PlannedSwarm) -> Result<SwarmRunHandle, SwarmError> {
        if let Some(mutator) = &self.dag_mutator {
            mutator(&mut planned.dag);
        }
        let coordinator = Arc::new(SwarmCoordinator::new(
            Arc::clone(&self.registry),
            Arc::clone(&self.router),
            Arc::new(self.providers.clone()),
            Arc::clone(&self.health_snapshot),
            self.events_tx.clone(),
            Arc::clone(&self.bridge),
        ));
        coordinator.run(planned, self.budget).await
    }

    /// Snapshot of the cross-capability invocation log. Returns a
    /// freshly-cloned `Vec<String>` so callers can assert on order
    /// without holding the lock.
    pub fn capability_call_log(&self) -> Vec<String> {
        self.capability_call_log.lock().unwrap().clone()
    }
}
