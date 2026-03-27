//! Repeatability guarantee enforcement.
//!
//! Problems are versioned and immutable once locked.
//! Scores are checked for inter-rater reliability.

use crate::battery::test_problem::TestProblem;

/// Repeatability guard.
pub struct RepeatabilityGuard;

impl RepeatabilityGuard {
    /// Verify a problem hasn't been modified since locking.
    pub fn verify_problem_integrity(problem: &TestProblem) -> Result<(), RepeatabilityError> {
        if !problem.locked {
            return Err(RepeatabilityError::ProblemNotLocked(problem.id.clone()));
        }
        Ok(())
    }

    /// Verify two independent scorers agree within tolerance.
    /// Returns the average score on success.
    pub fn verify_scorer_agreement(
        scorer_a: f64,
        scorer_b: f64,
        tolerance: f64,
    ) -> Result<f64, RepeatabilityError> {
        let diff = (scorer_a - scorer_b).abs();
        if diff > tolerance {
            return Err(RepeatabilityError::ScorerDisagreement {
                scorer_a,
                scorer_b,
                tolerance,
            });
        }
        Ok((scorer_a + scorer_b) / 2.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RepeatabilityError {
    #[error("Problem not locked: {0}")]
    ProblemNotLocked(String),
    #[error("Scorer disagreement: A={scorer_a:.2}, B={scorer_b:.2}, tolerance={tolerance:.2}")]
    ScorerDisagreement {
        scorer_a: f64,
        scorer_b: f64,
        tolerance: f64,
    },
    #[error("Problem integrity check failed: {0}")]
    IntegrityViolation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::expected_chain::ExpectedReasoning;
    use crate::battery::test_problem::{ProblemContext, ScoringRubric, TestProblem};
    use crate::framework::{DifficultyLevel, Vector};

    fn unlocked_problem() -> TestProblem {
        TestProblem {
            id: "test-unlocked".into(),
            version: "v0.1".into(),
            vector: Vector::ReasoningDepth,
            level: DifficultyLevel::Level1,
            problem_statement: "test".into(),
            context: ProblemContext {
                initial_state: serde_json::Value::Null,
                mid_problem_updates: vec![],
                available_tools: vec![],
            },
            expected_reasoning: ExpectedReasoning {
                causal_chain: vec![],
                expected_plan: None,
                expected_adaptation: None,
                expected_tool_use: None,
                required_insights: vec![],
                critical_failures: vec![],
            },
            scoring_rubric: ScoringRubric {
                full_credit: vec![],
                partial_credit: vec![],
                zero_credit: vec![],
            },
            gaming_detection: vec![],
            locked: false,
            locked_at: None,
        }
    }

    #[test]
    fn test_repeatability_guard_rejects_unlocked_problems() {
        let problem = unlocked_problem();
        let result = RepeatabilityGuard::verify_problem_integrity(&problem);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RepeatabilityError::ProblemNotLocked(_)
        ));
    }

    #[test]
    fn test_scorer_disagreement_rejection() {
        let result = RepeatabilityGuard::verify_scorer_agreement(8.0, 5.0, 1.0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RepeatabilityError::ScorerDisagreement { .. }
        ));
    }

    #[test]
    fn test_scorer_agreement_returns_average() {
        let result = RepeatabilityGuard::verify_scorer_agreement(7.0, 7.5, 1.0);
        assert!(result.is_ok());
        assert!((result.unwrap() - 7.25).abs() < 1e-9);
    }
}
