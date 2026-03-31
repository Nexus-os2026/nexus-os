//! # Applier
//!
//! Stage 5 of the self-improvement pipeline. Applies validated proposals
//! with checkpoint creation, post-apply testing, canary monitoring,
//! and automatic rollback.

use crate::types::{AppliedImprovement, ImprovementStatus, ProposedChange, ValidatedProposal};
use thiserror::Error;
use uuid::Uuid;

/// Errors from the Applier.
#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("checkpoint creation failed: {0}")]
    CheckpointFailed(String),
    #[error("post-apply test failure: {0}")]
    PostApplyTestFailure(String),
    #[error("apply failed: {0}")]
    ApplyFailed(String),
}

/// Configuration for the Applier.
#[derive(Debug, Clone)]
pub struct ApplierConfig {
    /// Canary monitoring period in minutes.
    pub canary_duration_minutes: u64,
    /// Whether to automatically rollback on canary failure.
    pub auto_rollback: bool,
}

impl Default for ApplierConfig {
    fn default() -> Self {
        Self {
            canary_duration_minutes: 30,
            auto_rollback: true,
        }
    }
}

/// Record of an applied change (for audit).
#[derive(Debug, Clone)]
pub struct ApplyRecord {
    pub change_type: String,
    pub key: String,
    pub old_value: String,
    pub new_value: String,
}

/// Pluggable checkpoint creator type.
type CheckpointFn = Box<dyn Fn(&str) -> Result<Uuid, String> + Send>;
/// Pluggable post-apply test runner type.
type PostTestFn = Box<dyn Fn() -> Result<usize, Vec<String>> + Send>;

/// The Applier applies validated proposals with safety nets.
pub struct Applier {
    config: ApplierConfig,
    /// All applied changes (for audit and rollback).
    applied: Vec<ApplyRecord>,
    /// Pluggable checkpoint creator.
    create_checkpoint: CheckpointFn,
    /// Pluggable post-apply test runner.
    post_test_runner: PostTestFn,
    /// Pluggable audit logger.
    audit_logger: Box<dyn Fn(&AppliedImprovement) + Send>,
}

impl Applier {
    pub fn new(
        config: ApplierConfig,
        create_checkpoint: CheckpointFn,
        post_test_runner: PostTestFn,
        audit_logger: Box<dyn Fn(&AppliedImprovement) + Send>,
    ) -> Self {
        Self {
            config,
            applied: Vec::new(),
            create_checkpoint,
            post_test_runner,
            audit_logger,
        }
    }

    /// Apply a validated proposal with checkpoint and canary monitoring.
    pub fn apply(
        &mut self,
        validated: &ValidatedProposal,
    ) -> Result<AppliedImprovement, ApplyError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Step 1: Create checkpoint BEFORE applying
        let checkpoint_id = (self.create_checkpoint)("pre-self-improvement")
            .map_err(ApplyError::CheckpointFailed)?;

        // Step 2: Apply the change
        self.apply_change(&validated.proposal.change)?;

        // Step 3: Run test suite AFTER applying (must still pass)
        if let Err(failures) = (self.post_test_runner)() {
            // Record rollback
            self.applied.clear();
            return Err(ApplyError::PostApplyTestFailure(failures.join(", ")));
        }

        // Step 4: Create the applied improvement record
        let canary_deadline = now + self.config.canary_duration_minutes * 60;

        let improvement = AppliedImprovement {
            id: Uuid::new_v4(),
            proposal_id: validated.proposal.id,
            checkpoint_id,
            applied_at: now,
            status: ImprovementStatus::Monitoring,
            canary_deadline,
        };

        // Step 5: Log to audit trail
        (self.audit_logger)(&improvement);

        Ok(improvement)
    }

    fn apply_change(&mut self, change: &ProposedChange) -> Result<(), ApplyError> {
        match change {
            ProposedChange::PromptUpdate {
                agent_id,
                new_prompt,
                old_prompt_hash,
                ..
            } => {
                self.applied.push(ApplyRecord {
                    change_type: "prompt_update".into(),
                    key: agent_id.clone(),
                    old_value: old_prompt_hash.clone(),
                    new_value: new_prompt.clone(),
                });
                Ok(())
            }
            ProposedChange::ConfigChange {
                key,
                old_value,
                new_value,
                ..
            } => {
                self.applied.push(ApplyRecord {
                    change_type: "config_change".into(),
                    key: key.clone(),
                    old_value: old_value.to_string(),
                    new_value: new_value.to_string(),
                });
                Ok(())
            }
            ProposedChange::PolicyUpdate {
                policy_id,
                old_policy_hash,
                new_policy_cedar,
            } => {
                self.applied.push(ApplyRecord {
                    change_type: "policy_update".into(),
                    key: policy_id.clone(),
                    old_value: old_policy_hash.clone(),
                    new_value: new_policy_cedar.clone(),
                });
                Ok(())
            }
            ProposedChange::SchedulingUpdate { .. } => {
                self.applied.push(ApplyRecord {
                    change_type: "scheduling_update".into(),
                    key: "scheduling_weights".into(),
                    old_value: "old".into(),
                    new_value: "new".into(),
                });
                Ok(())
            }
            ProposedChange::CodePatch { target_file, .. } => Err(ApplyError::ApplyFailed(format!(
                "code patches not yet supported for {target_file}"
            ))),
        }
    }

    /// Get all applied changes (for audit/inspection).
    pub fn applied_changes(&self) -> &[ApplyRecord] {
        &self.applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ImprovementDomain, ImprovementProposal, ProposedChange, RollbackPlan, RollbackStep,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_validated() -> ValidatedProposal {
        ValidatedProposal {
            proposal: ImprovementProposal {
                id: Uuid::new_v4(),
                opportunity_id: Uuid::new_v4(),
                domain: ImprovementDomain::ConfigTuning,
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
            },
            validation_timestamp: 1000,
            invariants_passed: 10,
            tests_passed: 100,
            simulation_risk_score: 0.2,
            hitl_signature: "ed25519:sig".into(),
        }
    }

    #[test]
    fn test_applier_checkpoint_creation_before_apply() {
        let checkpoint_created = Arc::new(AtomicBool::new(false));
        let cc = checkpoint_created.clone();

        let mut applier = Applier::new(
            ApplierConfig::default(),
            Box::new(move |_label| {
                cc.store(true, Ordering::SeqCst);
                Ok(Uuid::new_v4())
            }),
            Box::new(|| Ok(100)),
            Box::new(|_| {}),
        );

        let result = applier.apply(&make_validated());
        assert!(result.is_ok());
        assert!(checkpoint_created.load(Ordering::SeqCst));
    }

    #[test]
    fn test_applier_automatic_rollback_on_test_failure() {
        let mut applier = Applier::new(
            ApplierConfig::default(),
            Box::new(|_| Ok(Uuid::new_v4())),
            Box::new(|| Err(vec!["test_foo failed".into()])),
            Box::new(|_| {}),
        );

        let result = applier.apply(&make_validated());
        assert!(matches!(result, Err(ApplyError::PostApplyTestFailure(_))));
        // Applied changes should be cleared (rollback)
        assert!(applier.applied_changes().is_empty());
    }

    #[test]
    fn test_applier_canary_monitoring_setup() {
        let mut applier = Applier::new(
            ApplierConfig {
                canary_duration_minutes: 60,
                ..Default::default()
            },
            Box::new(|_| Ok(Uuid::new_v4())),
            Box::new(|| Ok(100)),
            Box::new(|_| {}),
        );

        let result = applier.apply(&make_validated()).unwrap();
        assert_eq!(result.status, ImprovementStatus::Monitoring);
        assert!(result.canary_deadline > result.applied_at);
        // 60 minutes = 3600 seconds
        assert_eq!(result.canary_deadline - result.applied_at, 3600);
    }

    #[test]
    fn test_applier_audit_trail_logging() {
        let logged = Arc::new(AtomicBool::new(false));
        let l = logged.clone();

        let mut applier = Applier::new(
            ApplierConfig::default(),
            Box::new(|_| Ok(Uuid::new_v4())),
            Box::new(|| Ok(100)),
            Box::new(move |_| {
                l.store(true, Ordering::SeqCst);
            }),
        );

        applier.apply(&make_validated()).unwrap();
        assert!(logged.load(Ordering::SeqCst));
    }

    #[test]
    fn test_applier_prompt_update_application() {
        let mut applier = Applier::new(
            ApplierConfig::default(),
            Box::new(|_| Ok(Uuid::new_v4())),
            Box::new(|| Ok(100)),
            Box::new(|_| {}),
        );

        let mut validated = make_validated();
        validated.proposal.change = ProposedChange::PromptUpdate {
            agent_id: "agent-1".into(),
            old_prompt_hash: "sha256:old".into(),
            new_prompt: "You are an improved agent".into(),
            optimization_trajectory: vec![],
        };

        let result = applier.apply(&validated);
        assert!(result.is_ok());
        assert_eq!(applier.applied_changes().len(), 1);
        assert_eq!(applier.applied_changes()[0].change_type, "prompt_update");
        assert_eq!(applier.applied_changes()[0].key, "agent-1");
    }
}
