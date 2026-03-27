//! Post-task feedback loop — learns from routing outcomes.

use serde::{Deserialize, Serialize};

use crate::router::{RoutingDecision, RoutingReason};

/// Feedback processor — analyzes routing outcomes.
#[derive(Default)]
pub struct RoutingFeedback {
    accuracy_history: Vec<RoutingAccuracySnapshot>,
}

impl RoutingFeedback {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a batch of routing decisions with outcomes.
    pub fn analyze(&mut self, decisions: &[RoutingDecision]) -> FeedbackAnalysis {
        let with_outcomes: Vec<_> = decisions
            .iter()
            .filter_map(|d| d.outcome.as_ref().map(|o| (d, o)))
            .collect();

        let mut over_estimates = 0;
        let mut under_estimates = 0;
        let mut accurate = 0;

        for (decision, outcome) in &with_outcomes {
            if outcome.model_was_sufficient
                && matches!(decision.reason, RoutingReason::ProactiveStagingUp { .. })
            {
                over_estimates += 1;
            } else if !outcome.model_was_sufficient && outcome.should_have_staged {
                under_estimates += 1;
            } else {
                accurate += 1;
            }
        }

        let threshold_recommendation = if under_estimates > over_estimates {
            ThresholdRecommendation::Lower {
                current: 0.95,
                suggested: 0.90,
                reason: format!("{under_estimates} tasks failed that should have staged up"),
            }
        } else if over_estimates > under_estimates * 2 {
            ThresholdRecommendation::Raise {
                current: 0.95,
                suggested: 0.97,
                reason: format!("{over_estimates} unnecessary staging events"),
            }
        } else {
            ThresholdRecommendation::KeepCurrent
        };

        let analysis = FeedbackAnalysis {
            total_analyzed: with_outcomes.len(),
            accurate,
            over_estimated: over_estimates,
            under_estimated: under_estimates,
            threshold_recommendation,
        };

        self.accuracy_history.push(RoutingAccuracySnapshot {
            total: with_outcomes.len(),
            accurate,
        });

        analysis
    }

    pub fn history_len(&self) -> usize {
        self.accuracy_history.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoutingAccuracySnapshot {
    total: usize,
    accurate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackAnalysis {
    pub total_analyzed: usize,
    pub accurate: usize,
    pub over_estimated: usize,
    pub under_estimated: usize,
    pub threshold_recommendation: ThresholdRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThresholdRecommendation {
    Lower {
        current: f64,
        suggested: f64,
        reason: String,
    },
    Raise {
        current: f64,
        suggested: f64,
        reason: String,
    },
    KeepCurrent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty_estimator::{EstimationMethod, TaskDifficultyEstimate};
    use crate::router::{RoutingDecision, RoutingOutcome, RoutingReason};
    use nexus_capability_measurement::framework::Vector;

    fn make_decision(reason: RoutingReason, outcome: RoutingOutcome) -> RoutingDecision {
        RoutingDecision {
            decision_id: "d1".into(),
            agent_id: "a1".into(),
            task_summary: "test".into(),
            difficulty_estimate: TaskDifficultyEstimate {
                reasoning_difficulty: 0.5,
                planning_difficulty: 0.5,
                adaptation_difficulty: 0.5,
                tool_use_difficulty: 0.5,
                dominant_vector: Vector::ReasoningDepth,
                confidence: 0.7,
                method: EstimationMethod::Heuristic,
            },
            selected_model: "model-a".into(),
            reason,
            alternatives_considered: vec![],
            timestamp: 0,
            outcome: Some(outcome),
        }
    }

    #[test]
    fn test_feedback_threshold_recommendation() {
        let mut feedback = RoutingFeedback::new();

        // 5 under-estimates (should have staged) vs 1 over-estimate
        let mut decisions = Vec::new();
        for _ in 0..5 {
            decisions.push(make_decision(
                RoutingReason::WithinCapability,
                RoutingOutcome {
                    success: false,
                    actual_difficulty: None,
                    model_was_sufficient: false,
                    should_have_staged: true,
                },
            ));
        }
        decisions.push(make_decision(
            RoutingReason::ProactiveStagingUp {
                current_model: "small".into(),
                ceiling_proximity: 0.96,
            },
            RoutingOutcome {
                success: true,
                actual_difficulty: None,
                model_was_sufficient: true,
                should_have_staged: false,
            },
        ));

        let analysis = feedback.analyze(&decisions);
        assert!(matches!(
            analysis.threshold_recommendation,
            ThresholdRecommendation::Lower { .. }
        ));
        assert_eq!(analysis.under_estimated, 5);
    }
}
