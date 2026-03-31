//! # Validator
//!
//! Stage 4 of the self-improvement pipeline. Enforces all 10 hard invariants,
//! runs tests, performs simulation, and requires Tier3 HITL consent.

use crate::invariants::{validate_all_invariants, InvariantCheckState, InvariantViolation};
use crate::types::{ImprovementProposal, ValidatedProposal};
use thiserror::Error;

/// Errors from the Validator.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("invariant violation: {0:?}")]
    InvariantViolation(Vec<InvariantViolation>),
    #[error("test suite failure: {0}")]
    TestFailure(String),
    #[error("simulation risk too high: score={0}")]
    SimulationRisk(f64),
    #[error("security issue: {0}")]
    SecurityIssue(String),
    #[error("HITL denied: {0}")]
    HitlDenied(String),
}

/// Configuration for the Validator.
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Maximum acceptable simulation risk score (0.0–1.0).
    pub max_risk_score: f64,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_risk_score: 0.7,
        }
    }
}

/// Simulated test results.
#[derive(Debug, Clone)]
pub struct TestResults {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

impl TestResults {
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

/// Simulated risk assessment result.
#[derive(Debug, Clone)]
pub struct SimulationRiskResult {
    pub risk_score: f64,
    pub summary: String,
}

/// Pluggable HITL consent gate — returns Ok(signature) or Err(reason).
type HitlGateFn = Box<dyn Fn(&ImprovementProposal) -> Result<String, String> + Send>;

/// The Validator checks all safety constraints before applying an improvement.
pub struct Validator {
    config: ValidatorConfig,
    /// Pluggable test runner — returns test results.
    test_runner: Box<dyn Fn() -> TestResults + Send>,
    /// Pluggable simulation — returns risk assessment.
    simulator: Box<dyn Fn(&ImprovementProposal) -> SimulationRiskResult + Send>,
    /// Pluggable HITL consent gate.
    hitl_gate: HitlGateFn,
}

impl Validator {
    pub fn new(
        config: ValidatorConfig,
        test_runner: Box<dyn Fn() -> TestResults + Send>,
        simulator: Box<dyn Fn(&ImprovementProposal) -> SimulationRiskResult + Send>,
        hitl_gate: HitlGateFn,
    ) -> Self {
        Self {
            config,
            test_runner,
            simulator,
            hitl_gate,
        }
    }

    /// Validate a proposal against all safety checks.
    pub fn validate(
        &self,
        proposal: &ImprovementProposal,
        state: &InvariantCheckState,
    ) -> Result<ValidatedProposal, ValidationError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Step 1: Check all 10 hard invariants
        validate_all_invariants(proposal, state).map_err(ValidationError::InvariantViolation)?;

        // Step 2: Run test suite (must all pass)
        let test_results = (self.test_runner)();
        if !test_results.all_passed() {
            return Err(ValidationError::TestFailure(
                test_results.failures.join(", "),
            ));
        }

        // Step 3: Dry-run simulation
        let sim_result = (self.simulator)(proposal);
        if sim_result.risk_score > self.config.max_risk_score {
            return Err(ValidationError::SimulationRisk(sim_result.risk_score));
        }

        // Step 4: REQUIRE Tier3 HITL consent (NON-NEGOTIABLE)
        let signature = (self.hitl_gate)(proposal).map_err(ValidationError::HitlDenied)?;

        Ok(ValidatedProposal {
            proposal: proposal.clone(),
            validation_timestamp: now,
            invariants_passed: 10,
            tests_passed: test_results.passed,
            simulation_risk_score: sim_result.risk_score,
            hitl_signature: signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::InvariantCheckState;
    use crate::types::{ProposedChange, RollbackPlan, RollbackStep};
    use uuid::Uuid;

    fn make_proposal() -> ImprovementProposal {
        ImprovementProposal {
            id: Uuid::new_v4(),
            opportunity_id: Uuid::new_v4(),
            domain: crate::types::ImprovementDomain::ConfigTuning,
            description: "test".into(),
            change: ProposedChange::ConfigChange {
                key: "timeout".into(),
                old_value: serde_json::json!(5000),
                new_value: serde_json::json!(3000),
                justification: "faster".into(),
            },
            rollback_plan: RollbackPlan {
                checkpoint_id: Uuid::new_v4(),
                steps: vec![RollbackStep {
                    description: "revert".into(),
                    action: serde_json::json!({}),
                }],
                estimated_rollback_time_ms: 100,
                automatic: true,
            },
            expected_tests: vec![],
            proof: None,
            generated_by: "test".into(),
            fuel_cost: 50,
        }
    }

    fn passing_state() -> InvariantCheckState {
        InvariantCheckState {
            audit_chain_valid: true,
            test_suite_passing: true,
            hitl_approved: true,
            fuel_remaining: 10_000,
            fuel_budget: 10_000,
        }
    }

    fn passing_tests() -> TestResults {
        TestResults {
            passed: 100,
            failed: 0,
            failures: vec![],
        }
    }

    fn low_risk(_: &ImprovementProposal) -> SimulationRiskResult {
        SimulationRiskResult {
            risk_score: 0.2,
            summary: "low risk".into(),
        }
    }

    fn approve(_: &ImprovementProposal) -> Result<String, String> {
        Ok("ed25519:test_signature".into())
    }

    #[test]
    fn test_validator_invariant_violation_rejection() {
        let validator = Validator::new(
            ValidatorConfig::default(),
            Box::new(passing_tests),
            Box::new(low_risk),
            Box::new(approve),
        );
        let proposal = make_proposal();
        let mut state = passing_state();
        state.audit_chain_valid = false; // invariant #2 will fail
        let result = validator.validate(&proposal, &state);
        assert!(matches!(
            result,
            Err(ValidationError::InvariantViolation(_))
        ));
    }

    #[test]
    fn test_validator_test_failure_rejection() {
        let validator = Validator::new(
            ValidatorConfig::default(),
            Box::new(|| TestResults {
                passed: 99,
                failed: 1,
                failures: vec!["test_foo".into()],
            }),
            Box::new(low_risk),
            Box::new(approve),
        );
        let result = validator.validate(&make_proposal(), &passing_state());
        assert!(matches!(result, Err(ValidationError::TestFailure(_))));
    }

    #[test]
    fn test_validator_simulation_risk_rejection() {
        let validator = Validator::new(
            ValidatorConfig {
                max_risk_score: 0.5,
            },
            Box::new(passing_tests),
            Box::new(|_| SimulationRiskResult {
                risk_score: 0.9,
                summary: "high risk".into(),
            }),
            Box::new(approve),
        );
        let result = validator.validate(&make_proposal(), &passing_state());
        assert!(matches!(result, Err(ValidationError::SimulationRisk(_))));
    }

    #[test]
    fn test_validator_hitl_denial_handling() {
        let validator = Validator::new(
            ValidatorConfig::default(),
            Box::new(passing_tests),
            Box::new(low_risk),
            Box::new(|_| Err("user declined".into())),
        );
        let result = validator.validate(&make_proposal(), &passing_state());
        assert!(matches!(result, Err(ValidationError::HitlDenied(_))));
    }

    #[test]
    fn test_validator_successful_validation_flow() {
        let validator = Validator::new(
            ValidatorConfig::default(),
            Box::new(passing_tests),
            Box::new(low_risk),
            Box::new(approve),
        );
        let result = validator.validate(&make_proposal(), &passing_state());
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.invariants_passed, 10);
        assert_eq!(validated.tests_passed, 100);
        assert!(validated.simulation_risk_score < 0.7);
        assert!(validated.hitl_signature.contains("ed25519"));
    }
}
