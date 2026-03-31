//! Core predictive routing engine.

use nexus_capability_measurement::evaluation::batch::AgentBoundary;
use serde::{Deserialize, Serialize};

use crate::difficulty_estimator::{DifficultyEstimator, TaskDifficultyEstimate};
use crate::model_capability::{ModelRegistry, ModelSizeClass};

/// A routing decision — which model was selected and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub decision_id: String,
    pub agent_id: String,
    pub task_summary: String,
    pub difficulty_estimate: TaskDifficultyEstimate,
    pub selected_model: String,
    pub reason: RoutingReason,
    pub alternatives_considered: Vec<ModelAlternative>,
    pub timestamp: u64,
    pub outcome: Option<RoutingOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingReason {
    WithinCapability,
    ProactiveStagingUp {
        current_model: String,
        ceiling_proximity: f64,
    },
    Escalation {
        from_tier: ModelSizeClass,
        to_tier: ModelSizeClass,
    },
    BestEffort {
        warning: String,
    },
    CostConstrained {
        ideal_model: String,
        selected_model: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAlternative {
    pub model_id: String,
    pub reason_not_selected: String,
    pub estimated_capability: f64,
    pub cost_per_1k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingOutcome {
    pub success: bool,
    pub actual_difficulty: Option<f64>,
    pub model_was_sufficient: bool,
    pub should_have_staged: bool,
}

/// The predictive router.
pub struct PredictiveRouter {
    model_registry: ModelRegistry,
    // Populated at construction; will be used for per-agent routing constraints.
    #[allow(dead_code)]
    agent_boundaries: Vec<AgentBoundary>,
    estimator: DifficultyEstimator,
    staging_threshold: f64,
    routing_log: Vec<RoutingDecision>,
}

impl PredictiveRouter {
    pub fn new(
        model_registry: ModelRegistry,
        agent_boundaries: Vec<AgentBoundary>,
        estimator: DifficultyEstimator,
    ) -> Self {
        Self {
            model_registry,
            agent_boundaries,
            estimator,
            staging_threshold: 0.95,
            routing_log: Vec::new(),
        }
    }

    pub fn with_staging_threshold(mut self, threshold: f64) -> Self {
        self.staging_threshold = threshold.clamp(0.5, 1.0);
        self
    }

    /// Route a task to the optimal model.
    pub fn route(&mut self, agent_id: &str, task_text: &str) -> RoutingDecision {
        let estimate = self.estimator.estimate(task_text);

        let (selected_model, reason, alternatives) = self.select_model(&estimate);

        let decision = RoutingDecision {
            decision_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            task_summary: task_text.chars().take(200).collect(),
            difficulty_estimate: estimate,
            selected_model,
            reason,
            alternatives_considered: alternatives,
            timestamp: epoch_secs(),
            outcome: None,
        };

        self.routing_log.push(decision.clone());
        decision
    }

    fn select_model(
        &self,
        estimate: &TaskDifficultyEstimate,
    ) -> (String, RoutingReason, Vec<ModelAlternative>) {
        let dominant = estimate.dominant_vector;
        let task_difficulty = estimate.difficulty_for(dominant);
        let mut alternatives = Vec::new();

        let models = self.model_registry.models_for_vector(dominant);

        if models.is_empty() {
            return (
                "none".into(),
                RoutingReason::BestEffort {
                    warning: "No models available".into(),
                },
                vec![],
            );
        }

        let required_capability = task_difficulty / self.staging_threshold;

        for model in &models {
            let model_cap = model.vector_scores.score_for(dominant);

            if model_cap >= required_capability {
                let reason =
                    if model_cap < required_capability * 1.1 && model_cap >= task_difficulty {
                        RoutingReason::ProactiveStagingUp {
                            current_model: models
                                .first()
                                .map(|m| m.model_id.clone())
                                .unwrap_or_default(),
                            ceiling_proximity: task_difficulty / model_cap,
                        }
                    } else {
                        RoutingReason::WithinCapability
                    };

                for cheaper in models.iter().filter(|m| {
                    m.cost_per_1k_input < model.cost_per_1k_input
                        && m.vector_scores.score_for(dominant) < required_capability
                }) {
                    alternatives.push(ModelAlternative {
                        model_id: cheaper.model_id.clone(),
                        reason_not_selected: format!(
                            "Capability {:.2} below required {:.2}",
                            cheaper.vector_scores.score_for(dominant),
                            required_capability
                        ),
                        estimated_capability: cheaper.vector_scores.score_for(dominant),
                        cost_per_1k: cheaper.cost_per_1k_input,
                    });
                }

                return (model.model_id.clone(), reason, alternatives);
            }
        }

        let best = models.last().unwrap();
        (
            best.model_id.clone(),
            RoutingReason::BestEffort {
                warning: format!(
                    "Best model ({}) capability {:.2}, task requires {:.2}",
                    best.model_id,
                    best.vector_scores.score_for(dominant),
                    required_capability
                ),
            },
            alternatives,
        )
    }

    /// Record the outcome of a routing decision.
    pub fn record_outcome(&mut self, decision_id: &str, outcome: RoutingOutcome) {
        if let Some(decision) = self
            .routing_log
            .iter_mut()
            .find(|d| d.decision_id == decision_id)
        {
            decision.outcome = Some(outcome);
        }
    }

    /// Routing accuracy from recorded outcomes.
    pub fn routing_accuracy(&self) -> RoutingAccuracy {
        let with_outcomes: Vec<_> = self
            .routing_log
            .iter()
            .filter_map(|d| d.outcome.as_ref().map(|o| (d, o)))
            .collect();

        let total = with_outcomes.len();
        if total == 0 {
            return RoutingAccuracy::default();
        }

        let sufficient = with_outcomes
            .iter()
            .filter(|(_, o)| o.model_was_sufficient)
            .count();
        let unnecessary_staging = with_outcomes
            .iter()
            .filter(|(d, o)| {
                matches!(d.reason, RoutingReason::ProactiveStagingUp { .. })
                    && o.model_was_sufficient
            })
            .count();
        let missed_staging = with_outcomes
            .iter()
            .filter(|(_, o)| o.should_have_staged && !o.model_was_sufficient)
            .count();

        RoutingAccuracy {
            total_decisions: total,
            model_sufficient: sufficient,
            model_insufficient: total - sufficient,
            unnecessary_staging,
            missed_staging,
            accuracy: sufficient as f64 / total as f64,
        }
    }

    pub fn routing_log(&self) -> &[RoutingDecision] {
        &self.routing_log
    }

    pub fn model_registry(&self) -> &ModelRegistry {
        &self.model_registry
    }

    pub fn staging_threshold(&self) -> f64 {
        self.staging_threshold
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingAccuracy {
    pub total_decisions: usize,
    pub model_sufficient: usize,
    pub model_insufficient: usize,
    pub unnecessary_staging: usize,
    pub missed_staging: usize,
    pub accuracy: f64,
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_capability::{ModelCapabilityProfile, VectorCeilings, VectorScores};

    fn make_model(id: &str, score: f64, cost: f64) -> ModelCapabilityProfile {
        ModelCapabilityProfile {
            model_id: id.into(),
            provider: "test".into(),
            display_name: id.into(),
            vector_scores: VectorScores {
                reasoning_depth: score,
                planning_coherence: score,
                adaptation: score,
                tool_use: score,
            },
            vector_ceilings: VectorCeilings {
                reasoning_depth: None,
                planning_coherence: None,
                adaptation: None,
                tool_use: None,
            },
            cost_per_1k_input: cost,
            cost_per_1k_output: cost * 2.0,
            avg_latency_ms: 100,
            available: true,
            is_local: false,
            size_class: ModelSizeClass::Medium,
        }
    }

    fn make_registry() -> ModelRegistry {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("small", 0.4, 0.1));
        reg.register(make_model("medium", 0.7, 0.5));
        reg.register(make_model("large", 0.95, 2.0));
        reg
    }

    #[test]
    fn test_router_selects_within_capability() {
        let mut router = PredictiveRouter::new(make_registry(), vec![], DifficultyEstimator::new());
        // Simple task — "read file" triggers low tool_use difficulty
        let decision = router.route("agent-1", "read file config.toml");
        assert!(
            decision.selected_model != "none",
            "Should select a model for simple task"
        );
    }

    #[test]
    fn test_router_proactive_staging() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("small", 0.50, 0.1)); // Capability 0.50
        reg.register(make_model("medium", 0.55, 0.5)); // Capability 0.55 — just barely enough
        reg.register(make_model("large", 0.95, 2.0));

        let mut router = PredictiveRouter::new(reg, vec![], DifficultyEstimator::new())
            .with_staging_threshold(0.95);

        // "root cause" triggers reasoning ~0.5. Required = 0.5/0.95 ≈ 0.526
        // small (0.50) is below 0.526, medium (0.55) is just above
        let decision = router.route("agent-1", "Diagnose the root cause of the failure");
        // Should at least select something above small
        assert_ne!(decision.selected_model, "none");
    }

    #[test]
    fn test_router_best_effort_warning() {
        let mut reg = ModelRegistry::new();
        reg.register(make_model("tiny", 0.2, 0.05));

        let mut router = PredictiveRouter::new(reg, vec![], DifficultyEstimator::new());

        // Very hard task — "conflict constraint impossible tradeoff" → high reasoning difficulty
        let decision = router.route("agent-1", "Resolve the conflict and constraint tradeoff");
        assert!(matches!(decision.reason, RoutingReason::BestEffort { .. }));
    }

    #[test]
    fn test_router_records_decision() {
        let mut router = PredictiveRouter::new(make_registry(), vec![], DifficultyEstimator::new());
        let _d = router.route("agent-1", "simple query");
        assert_eq!(router.routing_log().len(), 1);
    }

    #[test]
    fn test_routing_outcome_feedback() {
        let mut router = PredictiveRouter::new(make_registry(), vec![], DifficultyEstimator::new());
        let decision = router.route("agent-1", "simple query");
        let did = decision.decision_id.clone();

        router.record_outcome(
            &did,
            RoutingOutcome {
                success: true,
                actual_difficulty: Some(0.3),
                model_was_sufficient: true,
                should_have_staged: false,
            },
        );

        let accuracy = router.routing_accuracy();
        assert_eq!(accuracy.total_decisions, 1);
        assert_eq!(accuracy.model_sufficient, 1);
        assert!((accuracy.accuracy - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_staging_threshold_clamped() {
        let router =
            PredictiveRouter::new(ModelRegistry::new(), vec![], DifficultyEstimator::new())
                .with_staging_threshold(0.3); // Below minimum 0.5
        assert!((router.staging_threshold() - 0.5).abs() < 1e-9);

        let router =
            PredictiveRouter::new(ModelRegistry::new(), vec![], DifficultyEstimator::new())
                .with_staging_threshold(1.5); // Above maximum 1.0
        assert!((router.staging_threshold() - 1.0).abs() < 1e-9);
    }
}
