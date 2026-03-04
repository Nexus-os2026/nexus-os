use crate::patch_lang::{
    apply_patch, parse_patch, validate_patch, PatchLangError, PatchProgram, RuntimePatchState,
};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::consent::{
    ApprovalQueue, ApprovalRequest, ConsentError, ConsentPolicyEngine, ConsentRuntime,
    GovernedOperation,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationError {
    PatchNotFound(String),
    VerifierBoundaryViolation,
    AutonomyDenied(String),
    ApprovalRequired(String),
    ConsentDenied(String),
    ValidationFailed(String),
    ReplayFailed(String),
    ReplayRequired,
    HumanApprovalRequired,
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutationError::PatchNotFound(id) => write!(f, "patch '{id}' not found"),
            MutationError::VerifierBoundaryViolation => {
                write!(f, "patch violates fixed verifier boundary")
            }
            MutationError::AutonomyDenied(reason) => write!(f, "autonomy denied: {reason}"),
            MutationError::ApprovalRequired(request_id) => {
                write!(f, "approval required: request_id='{request_id}'")
            }
            MutationError::ConsentDenied(reason) => write!(f, "consent denied: {reason}"),
            MutationError::ValidationFailed(reason) => write!(f, "validation failed: {reason}"),
            MutationError::ReplayFailed(reason) => write!(f, "A/B replay failed: {reason}"),
            MutationError::ReplayRequired => write!(f, "A/B replay must pass before approval"),
            MutationError::HumanApprovalRequired => {
                write!(f, "human approval is required before apply")
            }
        }
    }
}

impl std::error::Error for MutationError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReplayExpectation {
    ConfigEquals { key: String, expected: String },
    EndpointStartsWith { service: String, prefix: String },
    ParameterInRange { name: String, min: f64, max: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayCase {
    pub name: String,
    pub expectation: ReplayExpectation,
}

#[derive(Debug, Clone)]
struct MutationRecord {
    patch: PatchProgram,
    validated: bool,
    replay_passed: bool,
    approved_by_human: bool,
    applied: bool,
}

pub struct MutationLifecycle {
    state: RuntimePatchState,
    audit_trail: AuditTrail,
    autonomy_guard: AutonomyGuard,
    consent_runtime: ConsentRuntime,
    actor_id: Uuid,
    records: HashMap<String, MutationRecord>,
}

impl MutationLifecycle {
    pub fn new() -> Self {
        Self::with_autonomy_level(AutonomyLevel::L0)
    }

    pub fn with_autonomy_level(level: AutonomyLevel) -> Self {
        Self {
            state: RuntimePatchState::default(),
            audit_trail: AuditTrail::new(),
            autonomy_guard: AutonomyGuard::new(level),
            consent_runtime: ConsentRuntime::new(
                ConsentPolicyEngine::default(),
                ApprovalQueue::in_memory(),
                "mutation.lifecycle".to_string(),
            ),
            actor_id: Uuid::nil(),
            records: HashMap::new(),
        }
    }

    pub fn propose(
        &mut self,
        patch_source: &str,
        proposed_by: &str,
    ) -> Result<String, MutationError> {
        let patch = parse_patch(patch_source).map_err(map_patch_error)?;
        let patch_id = format!("mut-{}", Uuid::new_v4());
        self.records.insert(
            patch_id.clone(),
            MutationRecord {
                patch,
                validated: false,
                replay_passed: false,
                approved_by_human: false,
                applied: false,
            },
        );

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "mutation_proposed",
                "patch_id": patch_id,
                "proposed_by": proposed_by,
            }),
        );

        Ok(patch_id)
    }

    pub fn validate(&mut self, patch_id: &str) -> Result<(), MutationError> {
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        validate_patch(&record.patch).map_err(map_patch_error)?;
        record.validated = true;

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "mutation_validated",
                "patch_id": patch_id,
            }),
        );
        Ok(())
    }

    pub fn replay_ab(
        &mut self,
        patch_id: &str,
        corpus: &[ReplayCase],
    ) -> Result<(), MutationError> {
        let patch = {
            let record = self
                .records
                .get(patch_id)
                .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
            if !record.validated {
                return Err(MutationError::ValidationFailed(
                    "patch must be validated before A/B replay".to_string(),
                ));
            }
            record.patch.clone()
        };

        let mut patched_state = self.state.clone();
        apply_patch(&patch, &mut patched_state).map_err(map_patch_error)?;
        run_replay_checks(&patched_state, corpus)?;

        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        record.replay_passed = true;

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "mutation_replay_passed",
                "patch_id": patch_id,
                "cases": corpus.len(),
            }),
        );
        Ok(())
    }

    pub fn approve(&mut self, patch_id: &str, human_approved: bool) -> Result<(), MutationError> {
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        if !record.replay_passed {
            return Err(MutationError::ReplayRequired);
        }
        if !human_approved {
            return Err(MutationError::HumanApprovalRequired);
        }
        record.approved_by_human = true;

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "mutation_approved",
                "patch_id": patch_id,
                "human_approved": true,
            }),
        );
        Ok(())
    }

    pub fn apply(&mut self, patch_id: &str) -> Result<(), MutationError> {
        self.autonomy_guard
            .require_self_modification(self.actor_id, &mut self.audit_trail)
            .map_err(|error| MutationError::AutonomyDenied(error.to_string()))?;

        let patch = {
            let record = self
                .records
                .get(patch_id)
                .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
            if !record.approved_by_human {
                return Err(MutationError::HumanApprovalRequired);
            }
            record.patch.clone()
        };
        self.consent_runtime
            .enforce_operation(
                GovernedOperation::SelfMutationApply,
                self.actor_id,
                patch.source.as_bytes(),
                &mut self.audit_trail,
            )
            .map_err(map_consent_error)?;

        apply_patch(&patch, &mut self.state).map_err(map_patch_error)?;

        let attestation_hash = sha256_hex(patch.source.as_bytes());
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        record.applied = true;

        let _ = self.audit_trail.append_event(
            Uuid::nil(),
            EventType::StateChange,
            json!({
                "event": "mutation_attested",
                "patch_id": patch_id,
                "attestation_hash": attestation_hash,
            }),
        );
        Ok(())
    }

    pub fn has_attestation(&self, patch_id: &str) -> bool {
        self.audit_trail.events().iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("mutation_attested")
                && event
                    .payload
                    .get("patch_id")
                    .and_then(|value| value.as_str())
                    == Some(patch_id)
        })
    }

    pub fn state(&self) -> &RuntimePatchState {
        &self.state
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn pending_consent_requests(&self) -> Vec<ApprovalRequest> {
        self.consent_runtime.pending_requests()
    }

    pub fn approve_consent(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), MutationError> {
        self.consent_runtime
            .approve(request_id, approver_id, &mut self.audit_trail)
            .map_err(map_consent_error)
    }

    pub fn deny_consent(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), MutationError> {
        self.consent_runtime
            .deny(request_id, approver_id, &mut self.audit_trail)
            .map_err(map_consent_error)
    }
}

impl Default for MutationLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

fn map_patch_error(error: PatchLangError) -> MutationError {
    match error {
        PatchLangError::VerifierBoundaryViolation => MutationError::VerifierBoundaryViolation,
        other => MutationError::ValidationFailed(other.to_string()),
    }
}

fn map_consent_error(error: ConsentError) -> MutationError {
    match error {
        ConsentError::ApprovalRequired { request_id, .. } => {
            MutationError::ApprovalRequired(request_id)
        }
        other => MutationError::ConsentDenied(other.to_string()),
    }
}

fn run_replay_checks(
    state: &RuntimePatchState,
    corpus: &[ReplayCase],
) -> Result<(), MutationError> {
    for case in corpus {
        match &case.expectation {
            ReplayExpectation::ConfigEquals { key, expected } => {
                let actual = state.config.get(key).cloned().unwrap_or_default();
                if &actual != expected {
                    return Err(MutationError::ReplayFailed(format!(
                        "case '{}' expected config '{}'='{}' got '{}'",
                        case.name, key, expected, actual
                    )));
                }
            }
            ReplayExpectation::EndpointStartsWith { service, prefix } => {
                let actual = state.endpoints.get(service).cloned().unwrap_or_default();
                if !actual.starts_with(prefix) {
                    return Err(MutationError::ReplayFailed(format!(
                        "case '{}' expected endpoint '{}' to start with '{}'",
                        case.name, service, prefix
                    )));
                }
            }
            ReplayExpectation::ParameterInRange { name, min, max } => {
                let actual = state.parameters.get(name).cloned().unwrap_or(f64::NAN);
                if !(actual >= *min && actual <= *max) {
                    return Err(MutationError::ReplayFailed(format!(
                        "case '{}' expected parameter '{}' in [{}, {}], got {}",
                        case.name, name, min, max, actual
                    )));
                }
            }
        }
    }
    Ok(())
}

fn sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{MutationError, MutationLifecycle, ReplayCase, ReplayExpectation};
    use nexus_kernel::autonomy::AutonomyLevel;

    #[test]
    fn test_mutation_lifecycle() {
        let mut lifecycle = MutationLifecycle::with_autonomy_level(AutonomyLevel::L4);
        let patch_id = lifecycle
            .propose(
                r#"
config.request_timeout_ms = "2500"
endpoint.social_api = "https://api.social.example/v2"
param.retry_backoff = 1.5
"#,
                "human.researcher",
            )
            .expect("propose should succeed");

        let validated = lifecycle.validate(patch_id.as_str());
        assert!(validated.is_ok());

        let replay = lifecycle.replay_ab(
            patch_id.as_str(),
            &[
                ReplayCase {
                    name: "timeout applied".to_string(),
                    expectation: ReplayExpectation::ConfigEquals {
                        key: "request_timeout_ms".to_string(),
                        expected: "2500".to_string(),
                    },
                },
                ReplayCase {
                    name: "endpoint preserved".to_string(),
                    expectation: ReplayExpectation::EndpointStartsWith {
                        service: "social_api".to_string(),
                        prefix: "https://".to_string(),
                    },
                },
            ],
        );
        assert!(replay.is_ok());

        let approved = lifecycle.approve(patch_id.as_str(), true);
        assert!(approved.is_ok());

        let request_id = match lifecycle.apply(patch_id.as_str()) {
            Err(MutationError::ApprovalRequired(request_id)) => request_id,
            other => panic!("expected approval requirement, got: {other:?}"),
        };

        assert!(lifecycle
            .approve_consent(request_id.as_str(), "approver.a")
            .is_ok());
        let still_blocked = lifecycle.apply(patch_id.as_str());
        assert!(matches!(
            still_blocked,
            Err(MutationError::ApprovalRequired(_))
        ));

        assert!(lifecycle
            .approve_consent(request_id.as_str(), "approver.b")
            .is_ok());
        let applied = lifecycle.apply(patch_id.as_str());
        assert!(applied.is_ok());
        assert!(lifecycle.has_attestation(patch_id.as_str()));
    }
}
