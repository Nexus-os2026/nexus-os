//! Governed fine-tuning pipeline with safety checks and human approval gates.

use nexus_kernel::audit::{AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyCheck {
    PiiCheck,
    HarmCheck,
    AccuracyCheck { threshold: f64 },
    SafetyAlignmentCheck { max_divergence: f64 },
}

impl SafetyCheck {
    pub fn name(&self) -> &str {
        match self {
            Self::PiiCheck => "PiiCheck",
            Self::HarmCheck => "HarmCheck",
            Self::AccuracyCheck { .. } => "AccuracyCheck",
            Self::SafetyAlignmentCheck { .. } => "SafetyAlignmentCheck",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub check: String,
    pub passed: bool,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Approved,
    Training,
    Evaluating,
    Completed,
    Failed { reason: String },
    Rejected { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub epochs: u32,
    pub learning_rate: f64,
    pub max_samples: u64,
}

#[derive(Debug, Clone)]
pub struct FineTuningJob {
    pub id: Uuid,
    pub base_model: String,
    pub training_data_hash: String,
    pub training_config: TrainingConfig,
    pub safety_checks: Vec<SafetyCheck>,
    pub status: JobStatus,
    pub created_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub created_at: u64,
    pub audit_trail: AuditTrail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FineTuningError {
    NotFound,
    InvalidTransition { from: String, to: String },
}

impl std::fmt::Display for FineTuningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "job not found"),
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid status transition: {from} -> {to}")
            }
        }
    }
}

#[derive(Debug)]
pub struct FineTuningManager {
    jobs: HashMap<Uuid, FineTuningJob>,
}

impl FineTuningManager {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
        }
    }

    pub fn create_job(
        &mut self,
        base_model: &str,
        training_data_hash: &str,
        config: TrainingConfig,
        safety_checks: Vec<SafetyCheck>,
        created_by: Uuid,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut audit_trail = AuditTrail::new();

        audit_trail.append_event(
            created_by,
            EventType::UserAction,
            json!({
                "event": "fine_tuning.job_created",
                "job_id": id.to_string(),
                "base_model": base_model,
                "training_data_hash": training_data_hash,
                "status": "Pending",
            }),
        );

        let job = FineTuningJob {
            id,
            base_model: base_model.to_string(),
            training_data_hash: training_data_hash.to_string(),
            training_config: config,
            safety_checks,
            status: JobStatus::Pending,
            created_by,
            approved_by: None,
            created_at: unix_now(),
            audit_trail,
        };

        self.jobs.insert(id, job);
        id
    }

    pub fn approve_job(&mut self, job_id: Uuid, approver_id: Uuid) -> Result<(), FineTuningError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(FineTuningError::NotFound)?;

        if job.status != JobStatus::Pending {
            return Err(FineTuningError::InvalidTransition {
                from: format!("{:?}", job.status),
                to: "Approved".to_string(),
            });
        }

        job.status = JobStatus::Approved;
        job.approved_by = Some(approver_id);

        job.audit_trail.append_event(
            approver_id,
            EventType::UserAction,
            json!({
                "event": "fine_tuning.job_approved",
                "job_id": job_id.to_string(),
                "approver": approver_id.to_string(),
                "status": "Approved",
            }),
        );

        Ok(())
    }

    pub fn reject_job(
        &mut self,
        job_id: Uuid,
        approver_id: Uuid,
        reason: &str,
    ) -> Result<(), FineTuningError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(FineTuningError::NotFound)?;

        if job.status != JobStatus::Pending {
            return Err(FineTuningError::InvalidTransition {
                from: format!("{:?}", job.status),
                to: "Rejected".to_string(),
            });
        }

        job.status = JobStatus::Rejected {
            reason: reason.to_string(),
        };

        job.audit_trail.append_event(
            approver_id,
            EventType::UserAction,
            json!({
                "event": "fine_tuning.job_rejected",
                "job_id": job_id.to_string(),
                "approver": approver_id.to_string(),
                "reason": reason,
                "status": "Rejected",
            }),
        );

        Ok(())
    }

    pub fn start_training(&mut self, job_id: Uuid) -> Result<(), FineTuningError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(FineTuningError::NotFound)?;

        if job.status != JobStatus::Approved {
            return Err(FineTuningError::InvalidTransition {
                from: format!("{:?}", job.status),
                to: "Training".to_string(),
            });
        }

        job.status = JobStatus::Training;

        job.audit_trail.append_event(
            job.created_by,
            EventType::StateChange,
            json!({
                "event": "fine_tuning.training_started",
                "job_id": job_id.to_string(),
                "status": "Training",
            }),
        );

        Ok(())
    }

    pub fn run_safety_evaluation(
        &mut self,
        job_id: Uuid,
        check_results: Vec<CheckResult>,
    ) -> Result<(), FineTuningError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(FineTuningError::NotFound)?;

        if job.status != JobStatus::Training {
            return Err(FineTuningError::InvalidTransition {
                from: format!("{:?}", job.status),
                to: "Evaluating".to_string(),
            });
        }

        job.status = JobStatus::Evaluating;

        let all_passed = check_results.iter().all(|r| r.passed);
        let results_json: Vec<_> = check_results
            .iter()
            .map(|r| {
                json!({
                    "check": r.check,
                    "passed": r.passed,
                    "details": r.details,
                })
            })
            .collect();

        let created_by = job.created_by;

        if all_passed {
            job.status = JobStatus::Completed;
            job.audit_trail.append_event(
                created_by,
                EventType::StateChange,
                json!({
                    "event": "fine_tuning.evaluation_passed",
                    "job_id": job_id.to_string(),
                    "status": "Completed",
                    "results": results_json,
                }),
            );
        } else {
            let failed_checks: Vec<&str> = check_results
                .iter()
                .filter(|r| !r.passed)
                .map(|r| r.check.as_str())
                .collect();
            let reason = format!("Safety checks failed: {}", failed_checks.join(", "));
            job.status = JobStatus::Failed {
                reason: reason.clone(),
            };
            job.audit_trail.append_event(
                created_by,
                EventType::StateChange,
                json!({
                    "event": "fine_tuning.evaluation_failed",
                    "job_id": job_id.to_string(),
                    "status": "Failed",
                    "reason": reason,
                    "results": results_json,
                }),
            );
        }

        Ok(())
    }

    pub fn get_job(&self, job_id: Uuid) -> Option<&FineTuningJob> {
        self.jobs.get(&job_id)
    }

    pub fn list_jobs(&self) -> Vec<&FineTuningJob> {
        self.jobs.values().collect()
    }

    pub fn job_audit_trail(&self, job_id: Uuid) -> Option<&AuditTrail> {
        self.jobs.get(&job_id).map(|j| &j.audit_trail)
    }
}

impl Default for FineTuningManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> TrainingConfig {
        TrainingConfig {
            epochs: 3,
            learning_rate: 0.001,
            max_samples: 10_000,
        }
    }

    fn make_checks() -> Vec<SafetyCheck> {
        vec![
            SafetyCheck::PiiCheck,
            SafetyCheck::HarmCheck,
            SafetyCheck::AccuracyCheck { threshold: 0.9 },
        ]
    }

    #[test]
    fn full_lifecycle_pending_to_completed() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();
        let approver = Uuid::new_v4();

        let job_id = mgr.create_job(
            "llama-3",
            "sha256:abc123",
            make_config(),
            make_checks(),
            user,
        );
        assert_eq!(mgr.get_job(job_id).unwrap().status, JobStatus::Pending);

        mgr.approve_job(job_id, approver).unwrap();
        let job = mgr.get_job(job_id).unwrap();
        assert_eq!(job.status, JobStatus::Approved);
        assert_eq!(job.approved_by, Some(approver));

        mgr.start_training(job_id).unwrap();
        assert_eq!(mgr.get_job(job_id).unwrap().status, JobStatus::Training);

        let results = vec![
            CheckResult {
                check: "PiiCheck".to_string(),
                passed: true,
                details: "No PII found".to_string(),
            },
            CheckResult {
                check: "HarmCheck".to_string(),
                passed: true,
                details: "Clean".to_string(),
            },
            CheckResult {
                check: "AccuracyCheck".to_string(),
                passed: true,
                details: "0.95 > 0.9".to_string(),
            },
        ];
        mgr.run_safety_evaluation(job_id, results).unwrap();
        assert_eq!(mgr.get_job(job_id).unwrap().status, JobStatus::Completed);
    }

    #[test]
    fn reject_moves_to_rejected() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();
        let approver = Uuid::new_v4();

        let job_id = mgr.create_job("gpt-4", "sha256:def456", make_config(), make_checks(), user);
        mgr.reject_job(job_id, approver, "Insufficient data quality")
            .unwrap();

        let job = mgr.get_job(job_id).unwrap();
        assert!(matches!(job.status, JobStatus::Rejected { .. }));
    }

    #[test]
    fn start_training_fails_on_non_approved() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();

        let job_id = mgr.create_job("model", "hash", make_config(), make_checks(), user);

        // Still Pending — should fail
        let result = mgr.start_training(job_id);
        assert!(matches!(
            result,
            Err(FineTuningError::InvalidTransition { .. })
        ));

        // Reject it, then try to start — should also fail
        mgr.reject_job(job_id, user, "no").unwrap();
        let result = mgr.start_training(job_id);
        assert!(matches!(
            result,
            Err(FineTuningError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn safety_eval_fails_job_if_any_check_fails() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();
        let approver = Uuid::new_v4();

        let job_id = mgr.create_job("model", "hash", make_config(), make_checks(), user);
        mgr.approve_job(job_id, approver).unwrap();
        mgr.start_training(job_id).unwrap();

        let results = vec![
            CheckResult {
                check: "PiiCheck".to_string(),
                passed: true,
                details: "ok".to_string(),
            },
            CheckResult {
                check: "HarmCheck".to_string(),
                passed: false,
                details: "harmful content detected".to_string(),
            },
            CheckResult {
                check: "AccuracyCheck".to_string(),
                passed: true,
                details: "ok".to_string(),
            },
        ];
        mgr.run_safety_evaluation(job_id, results).unwrap();

        let job = mgr.get_job(job_id).unwrap();
        match &job.status {
            JobStatus::Failed { reason } => {
                assert!(reason.contains("HarmCheck"));
            }
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn every_status_transition_has_audit_event() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();
        let approver = Uuid::new_v4();

        let job_id = mgr.create_job("model", "hash", make_config(), make_checks(), user);
        // 1 event: job_created

        mgr.approve_job(job_id, approver).unwrap();
        // 2 events: job_created, job_approved

        mgr.start_training(job_id).unwrap();
        // 3 events: + training_started

        let results = vec![CheckResult {
            check: "PiiCheck".to_string(),
            passed: true,
            details: "ok".to_string(),
        }];
        mgr.run_safety_evaluation(job_id, results).unwrap();
        // 4 events: + evaluation_passed

        let trail = mgr.job_audit_trail(job_id).unwrap();
        let events = trail.events();
        assert_eq!(events.len(), 4);

        let event_names: Vec<&str> = events
            .iter()
            .map(|e| e.payload.get("event").unwrap().as_str().unwrap())
            .collect();
        assert_eq!(
            event_names,
            vec![
                "fine_tuning.job_created",
                "fine_tuning.job_approved",
                "fine_tuning.training_started",
                "fine_tuning.evaluation_passed",
            ]
        );
    }

    #[test]
    fn approve_requires_pending_status() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();
        let approver = Uuid::new_v4();

        let job_id = mgr.create_job("model", "hash", make_config(), make_checks(), user);
        mgr.approve_job(job_id, approver).unwrap();

        // Already approved — cannot approve again
        let result = mgr.approve_job(job_id, approver);
        assert!(matches!(
            result,
            Err(FineTuningError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn list_jobs_returns_all() {
        let mut mgr = FineTuningManager::new();
        let user = Uuid::new_v4();

        mgr.create_job("a", "h1", make_config(), vec![], user);
        mgr.create_job("b", "h2", make_config(), vec![], user);

        assert_eq!(mgr.list_jobs().len(), 2);
    }

    #[test]
    fn get_nonexistent_job_returns_none() {
        let mgr = FineTuningManager::new();
        assert!(mgr.get_job(Uuid::new_v4()).is_none());
        assert!(mgr.job_audit_trail(Uuid::new_v4()).is_none());
    }
}
