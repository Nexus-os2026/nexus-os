//! Asymmetric scoring engine — failures in different directions have different weights.

use serde::{Deserialize, Serialize};

use crate::framework::Vector;

/// Primary score with asymmetric penalty adjustments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryScore {
    pub raw_score: f64,
    pub penalties: Vec<Penalty>,
    pub adjusted_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub reason: String,
    pub severity: PenaltySeverity,
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PenaltySeverity {
    /// Suboptimal but functional.
    Minor,
    /// Missing a dependency or causal link.
    Major,
    /// Score goes to zero (hallucinating tool output).
    Critical,
    /// Score zero AND flags for review (confident wrong answer at Level 5).
    Catastrophic,
}

/// Asymmetric weights per vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricWeights {
    pub gap_weight: f64,
    pub redundancy_weight: f64,
    pub hallucination_weight: f64,
}

/// Get the asymmetric weights for a given vector.
pub fn asymmetric_weights(vector: Vector) -> AsymmetricWeights {
    match vector {
        Vector::ReasoningDepth => AsymmetricWeights {
            gap_weight: 1.5,
            redundancy_weight: 0.5,
            hallucination_weight: 3.0,
        },
        Vector::PlanningCoherence => AsymmetricWeights {
            gap_weight: 2.0,
            redundancy_weight: 0.5,
            hallucination_weight: 3.0,
        },
        Vector::AdaptationUnderUncertainty => AsymmetricWeights {
            gap_weight: 2.0,
            redundancy_weight: 0.8,
            hallucination_weight: 3.0,
        },
        Vector::ToolUseIntegrity => AsymmetricWeights {
            // Hallucinating tool output is the most dangerous failure mode.
            gap_weight: 1.0,
            redundancy_weight: 0.3,
            hallucination_weight: 5.0,
        },
    }
}

/// Apply penalties to a raw score, returning the adjusted score.
pub fn apply_penalties(raw_score: f64, penalties: &[Penalty]) -> f64 {
    let mut adjusted = raw_score;
    for penalty in penalties {
        match penalty.severity {
            PenaltySeverity::Critical | PenaltySeverity::Catastrophic => return 0.0,
            _ => adjusted -= penalty.weight,
        }
    }
    adjusted.max(0.0)
}

/// Compute the primary score for an agent response given detected gaps,
/// redundancies, and hallucinations.
pub fn compute_primary_score(
    vector: Vector,
    coverage: f64,
    gap_count: usize,
    redundancy_count: usize,
    hallucination_count: usize,
) -> PrimaryScore {
    let weights = asymmetric_weights(vector);
    let mut penalties = Vec::new();

    for _ in 0..gap_count {
        penalties.push(Penalty {
            reason: "Missing required element".into(),
            severity: PenaltySeverity::Major,
            weight: weights.gap_weight * 0.1,
        });
    }
    for _ in 0..redundancy_count {
        penalties.push(Penalty {
            reason: "Redundant element".into(),
            severity: PenaltySeverity::Minor,
            weight: weights.redundancy_weight * 0.05,
        });
    }
    for _ in 0..hallucination_count {
        penalties.push(Penalty {
            reason: "Fabricated information".into(),
            severity: PenaltySeverity::Critical,
            weight: weights.hallucination_weight * 0.2,
        });
    }

    let adjusted = apply_penalties(coverage, &penalties);

    PrimaryScore {
        raw_score: coverage,
        penalties,
        adjusted_score: adjusted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asymmetric_scoring_gaps_worse_than_redundancy() {
        for vector in [
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ] {
            let w = asymmetric_weights(vector);
            assert!(
                w.gap_weight > w.redundancy_weight,
                "{vector:?}: gap_weight ({}) should be > redundancy_weight ({})",
                w.gap_weight,
                w.redundancy_weight,
            );
        }
    }

    #[test]
    fn test_hallucination_penalty_is_highest() {
        for vector in [
            Vector::ReasoningDepth,
            Vector::PlanningCoherence,
            Vector::AdaptationUnderUncertainty,
            Vector::ToolUseIntegrity,
        ] {
            let w = asymmetric_weights(vector);
            assert!(
                w.hallucination_weight >= w.gap_weight,
                "{vector:?}: hallucination_weight ({}) should be >= gap_weight ({})",
                w.hallucination_weight,
                w.gap_weight,
            );
            assert!(
                w.hallucination_weight >= w.redundancy_weight,
                "{vector:?}: hallucination_weight ({}) should be >= redundancy_weight ({})",
                w.hallucination_weight,
                w.redundancy_weight,
            );
        }
    }

    #[test]
    fn test_tool_use_hallucination_is_catastrophic() {
        let score = compute_primary_score(Vector::ToolUseIntegrity, 0.9, 0, 0, 1);
        assert_eq!(
            score.adjusted_score, 0.0,
            "Tool output fabrication must score zero"
        );
    }

    #[test]
    fn critical_penalty_zeros_score() {
        let penalties = vec![Penalty {
            reason: "fabricated".into(),
            severity: PenaltySeverity::Critical,
            weight: 1.0,
        }];
        assert_eq!(apply_penalties(0.95, &penalties), 0.0);
    }

    #[test]
    fn minor_penalty_reduces_score() {
        let penalties = vec![Penalty {
            reason: "redundant".into(),
            severity: PenaltySeverity::Minor,
            weight: 0.1,
        }];
        let result = apply_penalties(0.9, &penalties);
        assert!((result - 0.8).abs() < 1e-9);
    }
}
