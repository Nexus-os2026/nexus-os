use crate::patch_lang::{
    apply_patch, parse_patch, validate_patch, PatchLangError, PatchProgram, RuntimePatchState,
};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::kill_gates::{GateStatus, KillGateRegistry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationError {
    PatchNotFound(String),
    VerifierBoundaryViolation,
    ValidationFailed(String),
    ReplayFailed(String),
    ReplayRequired,
    HumanApprovalRequired,
    KillGateFrozen(&'static str),
    KillGateHalted(&'static str),
    AuditFailure(String),
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutationError::PatchNotFound(id) => write!(f, "patch '{id}' not found"),
            MutationError::VerifierBoundaryViolation => {
                write!(f, "patch violates fixed verifier boundary")
            }
            MutationError::ValidationFailed(reason) => write!(f, "validation failed: {reason}"),
            MutationError::ReplayFailed(reason) => write!(f, "A/B replay failed: {reason}"),
            MutationError::ReplayRequired => write!(f, "A/B replay must pass before approval"),
            MutationError::HumanApprovalRequired => {
                write!(f, "human approval is required before apply")
            }
            MutationError::KillGateFrozen(subsystem) => {
                write!(f, "kill gate is frozen for subsystem '{subsystem}'")
            }
            MutationError::KillGateHalted(subsystem) => {
                write!(f, "kill gate is halted for subsystem '{subsystem}'")
            }
            MutationError::AuditFailure(reason) => {
                write!(f, "audit failure: {reason}")
            }
        }
    }
}

impl From<nexus_kernel::audit::AuditError> for MutationError {
    fn from(value: nexus_kernel::audit::AuditError) -> Self {
        MutationError::AuditFailure(value.to_string())
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
    records: HashMap<String, MutationRecord>,
    kill_gates: KillGateRegistry,
    agent_id: Uuid,
}

impl MutationLifecycle {
    pub fn new() -> Self {
        Self {
            state: RuntimePatchState::default(),
            audit_trail: AuditTrail::new(),
            records: HashMap::new(),
            kill_gates: KillGateRegistry::default(),
            agent_id: Uuid::nil(),
        }
    }

    pub fn with_agent_id(mut self, agent_id: Uuid) -> Self {
        self.agent_id = agent_id;
        self
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

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "mutation_proposed",
                "patch_id": patch_id,
                "proposed_by": proposed_by,
            }),
        )?;

        Ok(patch_id)
    }

    pub fn validate(&mut self, patch_id: &str) -> Result<(), MutationError> {
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        validate_patch(&record.patch).map_err(map_patch_error)?;
        record.validated = true;

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "mutation_validated",
                "patch_id": patch_id,
            }),
        )?;
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
        if let Err(error) = run_replay_checks(&patched_state, corpus) {
            let _ =
                self.kill_gates
                    .check_gate("mutation", 1.0, self.agent_id, &mut self.audit_trail);
            return Err(error);
        }

        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        record.replay_passed = true;

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "mutation_replay_passed",
                "patch_id": patch_id,
                "cases": corpus.len(),
            }),
        )?;
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

        self.audit_trail.append_event(
            self.agent_id,
            EventType::UserAction,
            json!({
                "event": "mutation_approved",
                "patch_id": patch_id,
                "human_approved": true,
            }),
        )?;
        Ok(())
    }

    pub fn apply(&mut self, patch_id: &str) -> Result<(), MutationError> {
        match self.kill_gates.gate_status("mutation") {
            Some(GateStatus::Frozen) => return Err(MutationError::KillGateFrozen("mutation")),
            Some(GateStatus::Halted) => return Err(MutationError::KillGateHalted("mutation")),
            _ => {}
        }

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

        apply_patch(&patch, &mut self.state).map_err(map_patch_error)?;

        let attestation_hash = sha256_hex(patch.source.as_bytes());
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| MutationError::PatchNotFound(patch_id.to_string()))?;
        record.applied = true;

        self.audit_trail.append_event(
            self.agent_id,
            EventType::StateChange,
            json!({
                "event": "mutation_attested",
                "patch_id": patch_id,
                "attestation_hash": attestation_hash,
            }),
        )?;
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

    pub fn mutation_gate_status(&self) -> Option<GateStatus> {
        self.kill_gates.gate_status("mutation")
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
    use super::{MutationLifecycle, ReplayCase, ReplayExpectation};

    #[test]
    fn test_mutation_lifecycle() {
        let mut lifecycle = MutationLifecycle::new();
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

        let applied = lifecycle.apply(patch_id.as_str());
        assert!(applied.is_ok());
        assert!(lifecycle.has_attestation(patch_id.as_str()));
    }
}
