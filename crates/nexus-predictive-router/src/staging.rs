//! Proactive model staging logic.

use crate::difficulty_estimator::TaskDifficultyEstimate;
use crate::model_capability::ModelCapabilityProfile;
use serde::{Deserialize, Serialize};

/// Staging recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagingRecommendation {
    pub should_stage: bool,
    pub current_model: String,
    pub recommended_model: Option<String>,
    pub reason: String,
    pub ceiling_proximity: f64,
}

/// Check if a model should be staged up based on task difficulty and threshold.
pub fn check_staging(
    current_model: &ModelCapabilityProfile,
    estimate: &TaskDifficultyEstimate,
    staging_threshold: f64,
) -> StagingRecommendation {
    let dominant = estimate.dominant_vector;
    let task_diff = estimate.difficulty_for(dominant);
    let model_cap = current_model.vector_scores.score_for(dominant);

    let proximity = if model_cap > 0.0 {
        task_diff / model_cap
    } else {
        1.0
    };

    if proximity >= staging_threshold {
        StagingRecommendation {
            should_stage: true,
            current_model: current_model.model_id.clone(),
            recommended_model: None, // Caller picks the next tier
            reason: format!(
                "Task difficulty {task_diff:.2} is at {:.0}% of model ceiling {model_cap:.2}",
                proximity * 100.0
            ),
            ceiling_proximity: proximity,
        }
    } else {
        StagingRecommendation {
            should_stage: false,
            current_model: current_model.model_id.clone(),
            recommended_model: None,
            reason: format!(
                "Task difficulty {task_diff:.2} at {:.0}% of ceiling — within threshold",
                proximity * 100.0
            ),
            ceiling_proximity: proximity,
        }
    }
}
