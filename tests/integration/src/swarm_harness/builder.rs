//! `ScenarioBuilder` + `Scenario` — the fluent assembly layer scenario
//! tests use to wire a fresh `Director` + `CapabilityRegistry` + null
//! oracle bridge in one expression.

use crate::swarm_harness::capabilities::SyntheticCapability;
use crate::swarm_harness::providers::SyntheticPlannerProvider;
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_kernel::audit::AuditTrail;
use nexus_swarm::events::SwarmEvent;
use nexus_swarm::oracle_bridge::testing::NullSwarmOracleBridge;
use nexus_swarm::provider::Provider;
use nexus_swarm::{Budget, CapabilityRegistry, Director, PlannedSwarm, SwarmError};
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

/// Fluent builder for a `Scenario`.
///
/// Defaults: `NullSwarmOracleBridge`, `Budget::unlimited_for_tests`, a
/// freshly-generated Ed25519 `CryptoIdentity`, and an empty
/// `CapabilityRegistry`. Callers register capabilities by chaining
/// `.with_synthetic_capability(name)`.
pub struct ScenarioBuilder {
    canned_plan_json: Option<String>,
    capabilities: Vec<SyntheticCapability>,
    budget: Budget,
}

impl ScenarioBuilder {
    pub fn new() -> Self {
        Self {
            canned_plan_json: None,
            capabilities: Vec::new(),
            budget: Budget::unlimited_for_tests(),
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
        self.capabilities.push(SyntheticCapability::named(name));
        self
    }

    /// Override the default unlimited test budget. B.1 doesn't exercise
    /// budget arithmetic; B.2 will.
    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
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
        let mut providers: HashMap<String, Arc<dyn Provider>> = HashMap::new();
        providers.insert(
            planner_provider.id().to_string(),
            Arc::clone(&planner_provider) as Arc<dyn Provider>,
        );
        for cap in self.capabilities {
            registry.register(Arc::new(cap));
        }

        let bridge = Arc::new(NullSwarmOracleBridge::new());
        let caller = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).expect("Ed25519 keygen");
        let audit = Arc::new(Mutex::new(AuditTrail::new()));
        let (events_tx, _events_rx) = broadcast::channel(SCENARIO_EVENT_CHANNEL_DEPTH);

        Scenario {
            director,
            planner_provider,
            registry,
            providers,
            bridge,
            budget: self.budget,
            caller,
            audit,
            events_tx,
        }
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A fully-wired scenario ready to drive `Director::plan` (B.1) and,
/// in B.2, `SwarmCoordinator::run`.
pub struct Scenario {
    pub director: Director,
    /// Held so tests can observe planner invocation counts.
    pub planner_provider: Arc<SyntheticPlannerProvider>,
    pub registry: CapabilityRegistry,
    pub providers: HashMap<String, Arc<dyn Provider>>,
    pub bridge: Arc<NullSwarmOracleBridge>,
    pub budget: Budget,
    pub caller: CryptoIdentity,
    pub audit: Arc<Mutex<AuditTrail>>,
    pub events_tx: broadcast::Sender<SwarmEvent>,
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
}
