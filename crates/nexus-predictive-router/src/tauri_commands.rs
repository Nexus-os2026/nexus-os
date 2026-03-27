//! Frontend integration types and handler logic.

use std::sync::RwLock;

use crate::difficulty_estimator::{DifficultyEstimator, TaskDifficultyEstimate};
use crate::feedback::{FeedbackAnalysis, RoutingFeedback};
use crate::model_capability::{ModelCapabilityProfile, ModelRegistry};
use crate::router::{PredictiveRouter, RoutingAccuracy, RoutingDecision, RoutingOutcome};

/// In-memory router state held by the Tauri app.
pub struct RouterState {
    pub router: RwLock<PredictiveRouter>,
    pub feedback: RwLock<RoutingFeedback>,
}

impl RouterState {
    pub fn new() -> Self {
        Self {
            router: RwLock::new(PredictiveRouter::new(
                ModelRegistry::new(),
                vec![],
                DifficultyEstimator::new(),
            )),
            feedback: RwLock::new(RoutingFeedback::new()),
        }
    }
}

impl Default for RouterState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn route_task(
    state: &RouterState,
    agent_id: &str,
    task_text: &str,
) -> Result<RoutingDecision, String> {
    let mut router = state.router.write().map_err(|e| format!("lock: {e}"))?;
    Ok(router.route(agent_id, task_text))
}

pub fn record_outcome(
    state: &RouterState,
    decision_id: &str,
    outcome: RoutingOutcome,
) -> Result<(), String> {
    let mut router = state.router.write().map_err(|e| format!("lock: {e}"))?;
    router.record_outcome(decision_id, outcome);
    Ok(())
}

pub fn get_accuracy(state: &RouterState) -> Result<RoutingAccuracy, String> {
    let router = state.router.read().map_err(|e| format!("lock: {e}"))?;
    Ok(router.routing_accuracy())
}

pub fn get_model_registry(state: &RouterState) -> Result<Vec<ModelCapabilityProfile>, String> {
    let router = state.router.read().map_err(|e| format!("lock: {e}"))?;
    Ok(router.model_registry().models.clone())
}

pub fn estimate_difficulty(
    state: &RouterState,
    task_text: &str,
) -> Result<TaskDifficultyEstimate, String> {
    let _router = state.router.read().map_err(|e| format!("lock: {e}"))?;
    let estimator = DifficultyEstimator::new();
    Ok(estimator.estimate(task_text))
}

pub fn get_feedback_analysis(state: &RouterState) -> Result<FeedbackAnalysis, String> {
    let router = state.router.read().map_err(|e| format!("lock: {e}"))?;
    let mut feedback = state.feedback.write().map_err(|e| format!("lock: {e}"))?;
    Ok(feedback.analyze(router.routing_log()))
}
