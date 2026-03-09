use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HitlTier {
    Tier0,
    Tier1,
    Tier2,
    Tier3,
}

impl HitlTier {
    pub fn as_str(self) -> &'static str {
        match self {
            HitlTier::Tier0 => "tier0",
            HitlTier::Tier1 => "tier1",
            HitlTier::Tier2 => "tier2",
            HitlTier::Tier3 => "tier3",
        }
    }

    fn approvals_required(self) -> usize {
        match self {
            HitlTier::Tier0 | HitlTier::Tier1 => 0,
            HitlTier::Tier2 => 1,
            HitlTier::Tier3 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GovernedOperation {
    ToolCall,
    TerminalCommand,
    SocialPostPublish,
    SelfMutationApply,
    MultiAgentOrchestrate,
    DistributedEnable,
}

impl GovernedOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            GovernedOperation::ToolCall => "tool_call",
            GovernedOperation::TerminalCommand => "terminal_command",
            GovernedOperation::SocialPostPublish => "social_post_publish",
            GovernedOperation::SelfMutationApply => "self_mutation_apply",
            GovernedOperation::MultiAgentOrchestrate => "multi_agent_orchestrate",
            GovernedOperation::DistributedEnable => "distributed_enable",
        }
    }

    fn from_policy_key(value: &str) -> Option<Self> {
        match value {
            "tool_call" => Some(GovernedOperation::ToolCall),
            "terminal_command" => Some(GovernedOperation::TerminalCommand),
            "social_post_publish" => Some(GovernedOperation::SocialPostPublish),
            "self_mutation_apply" => Some(GovernedOperation::SelfMutationApply),
            "multi_agent_orchestrate" => Some(GovernedOperation::MultiAgentOrchestrate),
            "distributed_enable" => Some(GovernedOperation::DistributedEnable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationConsentPolicy {
    pub required_tier: HitlTier,
    #[serde(default)]
    pub allowed_approvers: Vec<String>,
}

impl OperationConsentPolicy {
    fn normalized(mut self) -> Self {
        self.allowed_approvers.sort();
        self.allowed_approvers.dedup();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentPolicyEngine {
    policies: BTreeMap<GovernedOperation, OperationConsentPolicy>,
}

impl Default for ConsentPolicyEngine {
    fn default() -> Self {
        let mut policies = BTreeMap::new();
        policies.insert(
            GovernedOperation::ToolCall,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier1,
                allowed_approvers: Vec::new(),
            },
        );
        policies.insert(
            GovernedOperation::TerminalCommand,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier2,
                allowed_approvers: Vec::new(),
            },
        );
        policies.insert(
            GovernedOperation::SocialPostPublish,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier2,
                allowed_approvers: Vec::new(),
            },
        );
        policies.insert(
            GovernedOperation::SelfMutationApply,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier3,
                allowed_approvers: Vec::new(),
            },
        );
        policies.insert(
            GovernedOperation::MultiAgentOrchestrate,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier2,
                allowed_approvers: Vec::new(),
            },
        );
        policies.insert(
            GovernedOperation::DistributedEnable,
            OperationConsentPolicy {
                required_tier: HitlTier::Tier3,
                allowed_approvers: Vec::new(),
            },
        );
        Self { policies }
    }
}

impl ConsentPolicyEngine {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AgentError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|error| {
            AgentError::ManifestError(format!(
                "failed reading consent policy '{}': {error}",
                path.display()
            ))
        })?;

        let raw: RawConsentPolicy = toml::from_str(content.as_str()).map_err(|error| {
            AgentError::ManifestError(format!("invalid consent policy: {error}"))
        })?;

        let mut engine = Self::default();
        for (operation_key, raw_policy) in raw.operations {
            let operation =
                GovernedOperation::from_policy_key(operation_key.as_str()).ok_or_else(|| {
                    AgentError::ManifestError(format!(
                        "unknown governed operation '{operation_key}' in consent policy"
                    ))
                })?;
            let current =
                engine
                    .policies
                    .get(&operation)
                    .cloned()
                    .unwrap_or(OperationConsentPolicy {
                        required_tier: HitlTier::Tier2,
                        allowed_approvers: Vec::new(),
                    });
            engine.policies.insert(
                operation,
                OperationConsentPolicy {
                    required_tier: raw_policy.required_tier.unwrap_or(current.required_tier),
                    allowed_approvers: if raw_policy.allowed_approvers.is_empty() {
                        current.allowed_approvers
                    } else {
                        raw_policy.allowed_approvers
                    },
                }
                .normalized(),
            );
        }

        Ok(engine)
    }

    pub fn required_tier(&self, operation: GovernedOperation) -> HitlTier {
        self.policies
            .get(&operation)
            .map(|policy| policy.required_tier)
            .unwrap_or(HitlTier::Tier2)
    }

    pub fn allowed_approvers(&self, operation: GovernedOperation) -> &[String] {
        self.policies
            .get(&operation)
            .map(|policy| policy.allowed_approvers.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_policy(
        &mut self,
        operation: GovernedOperation,
        required_tier: HitlTier,
        allowed_approvers: Vec<String>,
    ) {
        self.policies.insert(
            operation,
            OperationConsentPolicy {
                required_tier,
                allowed_approvers,
            }
            .normalized(),
        );
    }
}

#[derive(Debug, Deserialize)]
struct RawConsentPolicy {
    #[serde(default)]
    operations: BTreeMap<String, RawOperationPolicy>,
}

#[derive(Debug, Deserialize)]
struct RawOperationPolicy {
    required_tier: Option<HitlTier>,
    #[serde(default)]
    allowed_approvers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub operation: GovernedOperation,
    pub agent_id: String,
    pub payload_hash: String,
    pub requested_by: String,
    pub required_tier: HitlTier,
    pub created_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalVerdict {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub id: String,
    pub approver_id: String,
    pub decision: ApprovalVerdict,
    pub signature: Option<String>,
    pub decision_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsentError {
    #[error("approval required for operation '{}' at tier '{}' (request_id='{}')", operation.as_str(), required_tier.as_str(), request_id)]
    ApprovalRequired {
        request_id: String,
        operation: GovernedOperation,
        required_tier: HitlTier,
    },
    #[error("approval request '{request_id}' has been denied")]
    RequestDenied { request_id: String },
    #[error("approval request '{0}' not found")]
    RequestNotFound(String),
    #[error("approver '{approver_id}' already recorded for request '{request_id}'")]
    DuplicateApprover {
        request_id: String,
        approver_id: String,
    },
    #[error("approver '{approver_id}' is not allowed for request '{request_id}'")]
    ApproverNotAllowed {
        request_id: String,
        approver_id: String,
    },
    #[error("approver '{approver_id}' cannot self-approve request '{request_id}'")]
    SelfApprovalRejected {
        request_id: String,
        approver_id: String,
    },
    #[error("approval queue storage error: {0}")]
    QueueStorage(String),
}

impl From<ConsentError> for AgentError {
    fn from(value: ConsentError) -> Self {
        match value {
            ConsentError::ApprovalRequired { request_id, .. } => {
                AgentError::ApprovalRequired { request_id }
            }
            other => AgentError::SupervisorError(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalQueue {
    records: BTreeMap<String, ApprovalRecord>,
    fingerprint_index: BTreeMap<String, String>,
    storage_path: Option<PathBuf>,
}

impl Default for ApprovalQueue {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ApprovalQueue {
    pub fn in_memory() -> Self {
        Self {
            records: BTreeMap::new(),
            fingerprint_index: BTreeMap::new(),
            storage_path: None,
        }
    }

    pub fn file_backed(path: impl AsRef<Path>) -> Result<Self, ConsentError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Ok(Self {
                storage_path: Some(path),
                ..Self::in_memory()
            });
        }

        let raw = fs::read_to_string(path.as_path())
            .map_err(|error| ConsentError::QueueStorage(error.to_string()))?;
        let snapshot: QueueSnapshot = serde_json::from_str(raw.as_str())
            .map_err(|error| ConsentError::QueueStorage(error.to_string()))?;
        let mut records = BTreeMap::new();
        let mut fingerprint_index = BTreeMap::new();
        for record in snapshot.records {
            if !record.executed {
                fingerprint_index.insert(record.fingerprint.clone(), record.request.id.clone());
            }
            records.insert(record.request.id.clone(), record);
        }
        Ok(Self {
            records,
            fingerprint_index,
            storage_path: Some(path),
        })
    }

    pub fn pending_requests(&self) -> Vec<ApprovalRequest> {
        self.records
            .values()
            .filter(|record| !record.executed)
            .map(|record| record.request.clone())
            .collect()
    }

    pub fn request(&self, request_id: &str) -> Option<&ApprovalRequest> {
        self.records.get(request_id).map(|record| &record.request)
    }

    pub fn request_or_get(
        &mut self,
        operation: GovernedOperation,
        agent_id: Uuid,
        payload_hash: String,
        requested_by: String,
        required_tier: HitlTier,
        audit: &mut AuditTrail,
    ) -> Result<ApprovalRequest, ConsentError> {
        let fingerprint = compute_fingerprint(
            operation,
            agent_id,
            payload_hash.as_str(),
            requested_by.as_str(),
            required_tier,
        );
        if let Some(existing_id) = self.fingerprint_index.get(fingerprint.as_str()) {
            if let Some(record) = self.records.get(existing_id) {
                return Ok(record.request.clone());
            }
        }

        let created_seq = next_audit_sequence(audit);
        let request_id = derive_request_id(
            created_seq,
            operation,
            agent_id,
            payload_hash.as_str(),
            requested_by.as_str(),
            required_tier,
        );
        let request = ApprovalRequest {
            id: request_id.clone(),
            operation,
            agent_id: agent_id.to_string(),
            payload_hash,
            requested_by,
            required_tier,
            created_seq,
        };
        let record = ApprovalRecord {
            request: request.clone(),
            approvals: BTreeSet::new(),
            denials: BTreeSet::new(),
            decisions: Vec::new(),
            executed: false,
            fingerprint: fingerprint.clone(),
        };
        self.records.insert(request_id.clone(), record);
        self.fingerprint_index.insert(fingerprint, request_id);
        append_requested_event(audit, &request);
        self.persist()?;
        Ok(request)
    }

    pub fn submit_decision(
        &mut self,
        request_id: &str,
        approver_id: &str,
        decision: ApprovalVerdict,
        allowed_approvers: &[String],
        audit: &mut AuditTrail,
    ) -> Result<ApprovalDecision, ConsentError> {
        let record = self
            .records
            .get_mut(request_id)
            .ok_or_else(|| ConsentError::RequestNotFound(request_id.to_string()))?;
        if approver_id == record.request.requested_by {
            return Err(ConsentError::SelfApprovalRejected {
                request_id: request_id.to_string(),
                approver_id: approver_id.to_string(),
            });
        }
        if !allowed_approvers.is_empty()
            && !allowed_approvers.iter().any(|item| item == approver_id)
        {
            return Err(ConsentError::ApproverNotAllowed {
                request_id: request_id.to_string(),
                approver_id: approver_id.to_string(),
            });
        }
        if record.approvals.contains(approver_id) || record.denials.contains(approver_id) {
            return Err(ConsentError::DuplicateApprover {
                request_id: request_id.to_string(),
                approver_id: approver_id.to_string(),
            });
        }

        match decision {
            ApprovalVerdict::Approve => {
                record.approvals.insert(approver_id.to_string());
            }
            ApprovalVerdict::Deny => {
                record.denials.insert(approver_id.to_string());
            }
        }
        let decision_event = ApprovalDecision {
            id: request_id.to_string(),
            approver_id: approver_id.to_string(),
            decision,
            signature: None,
            decision_seq: next_audit_sequence(audit),
        };
        record.decisions.push(decision_event.clone());
        append_decision_event(audit, &record.request, &decision_event);
        self.persist()?;
        Ok(decision_event)
    }

    pub fn approval_state(&self, request_id: &str) -> Result<ApprovalState, ConsentError> {
        let record = self
            .records
            .get(request_id)
            .ok_or_else(|| ConsentError::RequestNotFound(request_id.to_string()))?;

        if !record.denials.is_empty() {
            return Ok(ApprovalState::Denied);
        }

        let required = record.request.required_tier.approvals_required();
        if required == 0 || record.approvals.len() >= required {
            return Ok(ApprovalState::Approved);
        }

        Ok(ApprovalState::Pending)
    }

    pub fn mark_executed(
        &mut self,
        request_id: &str,
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError> {
        let state = self.approval_state(request_id)?;
        if state == ApprovalState::Denied {
            return Err(ConsentError::RequestDenied {
                request_id: request_id.to_string(),
            });
        }

        let record = self
            .records
            .get_mut(request_id)
            .ok_or_else(|| ConsentError::RequestNotFound(request_id.to_string()))?;
        if record.executed {
            return Ok(());
        }

        let required = record.request.required_tier.approvals_required();
        if required > 0 && record.approvals.len() < required {
            return Err(ConsentError::ApprovalRequired {
                request_id: request_id.to_string(),
                operation: record.request.operation,
                required_tier: record.request.required_tier,
            });
        }

        record.executed = true;
        self.fingerprint_index.remove(record.fingerprint.as_str());
        append_executed_event(audit, &record.request, record.approver_ids().as_slice());
        self.persist()?;
        Ok(())
    }

    fn persist(&self) -> Result<(), ConsentError> {
        let Some(path) = &self.storage_path else {
            return Ok(());
        };
        let snapshot = QueueSnapshot {
            records: self.records.values().cloned().collect(),
        };
        let encoded = serde_json::to_string_pretty(&snapshot)
            .map_err(|error| ConsentError::QueueStorage(error.to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| ConsentError::QueueStorage(error.to_string()))?;
        }
        fs::write(path, encoded).map_err(|error| ConsentError::QueueStorage(error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone)]
pub struct ConsentRuntime {
    policy_engine: ConsentPolicyEngine,
    approval_queue: ApprovalQueue,
    requester_id: String,
}

impl Default for ConsentRuntime {
    fn default() -> Self {
        Self::new(
            ConsentPolicyEngine::default(),
            ApprovalQueue::in_memory(),
            "consent.runtime".to_string(),
        )
    }
}

impl ConsentRuntime {
    pub fn new(
        policy_engine: ConsentPolicyEngine,
        approval_queue: ApprovalQueue,
        requester_id: String,
    ) -> Self {
        Self {
            policy_engine,
            approval_queue,
            requester_id,
        }
    }

    pub fn from_manifest(
        consent_policy_path: Option<&str>,
        requester_id: Option<&str>,
        default_requester: &str,
    ) -> Result<Self, AgentError> {
        let requester = requester_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| default_requester.to_string());

        if let Some(path) = consent_policy_path {
            let policy_engine = ConsentPolicyEngine::load(path)?;
            let queue_path = Path::new(path).with_extension("consent-queue.json");
            let approval_queue = ApprovalQueue::file_backed(queue_path)
                .map_err(|error| AgentError::ManifestError(error.to_string()))?;
            return Ok(Self::new(policy_engine, approval_queue, requester));
        }

        Ok(Self::new(
            ConsentPolicyEngine::default(),
            ApprovalQueue::in_memory(),
            requester,
        ))
    }

    pub fn policy_engine(&self) -> &ConsentPolicyEngine {
        &self.policy_engine
    }

    pub fn policy_engine_mut(&mut self) -> &mut ConsentPolicyEngine {
        &mut self.policy_engine
    }

    pub fn pending_requests(&self) -> Vec<ApprovalRequest> {
        self.approval_queue.pending_requests()
    }

    pub fn enforce_operation(
        &mut self,
        operation: GovernedOperation,
        agent_id: Uuid,
        payload: &[u8],
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError> {
        let required_tier = self.policy_engine.required_tier(operation);
        if required_tier == HitlTier::Tier0 {
            return Ok(());
        }

        let payload_hash = hash_payload(payload);
        let request = self.approval_queue.request_or_get(
            operation,
            agent_id,
            payload_hash,
            self.requester_id.clone(),
            required_tier,
            audit,
        )?;

        match required_tier {
            HitlTier::Tier1 => self
                .approval_queue
                .mark_executed(request.id.as_str(), audit),
            HitlTier::Tier2 | HitlTier::Tier3 => {
                match self.approval_queue.approval_state(request.id.as_str())? {
                    ApprovalState::Approved => self
                        .approval_queue
                        .mark_executed(request.id.as_str(), audit),
                    ApprovalState::Denied => Err(ConsentError::RequestDenied {
                        request_id: request.id,
                    }),
                    ApprovalState::Pending => Err(ConsentError::ApprovalRequired {
                        request_id: request.id,
                        operation,
                        required_tier,
                    }),
                }
            }
            HitlTier::Tier0 => Ok(()),
        }
    }

    pub fn approve(
        &mut self,
        request_id: &str,
        approver_id: &str,
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError> {
        let request = self
            .approval_queue
            .request(request_id)
            .ok_or_else(|| ConsentError::RequestNotFound(request_id.to_string()))?
            .clone();
        let allowed = self
            .policy_engine
            .allowed_approvers(request.operation)
            .to_vec();
        self.approval_queue.submit_decision(
            request_id,
            approver_id,
            ApprovalVerdict::Approve,
            allowed.as_slice(),
            audit,
        )?;
        Ok(())
    }

    pub fn deny(
        &mut self,
        request_id: &str,
        approver_id: &str,
        audit: &mut AuditTrail,
    ) -> Result<(), ConsentError> {
        let request = self
            .approval_queue
            .request(request_id)
            .ok_or_else(|| ConsentError::RequestNotFound(request_id.to_string()))?
            .clone();
        let allowed = self
            .policy_engine
            .allowed_approvers(request.operation)
            .to_vec();
        self.approval_queue.submit_decision(
            request_id,
            approver_id,
            ApprovalVerdict::Deny,
            allowed.as_slice(),
            audit,
        )?;
        Ok(())
    }
}

fn append_requested_event(audit: &mut AuditTrail, request: &ApprovalRequest) {
    audit
        .append_event(
            parse_or_nil_uuid(request.agent_id.as_str()),
            EventType::UserAction,
            json!({
                "event": "consent.requested",
                "request_id": request.id,
                "operation": request.operation.as_str(),
                "required_tier": request.required_tier.as_str(),
                "payload_hash": request.payload_hash,
                "approver_ids": [],
                "decision_result": "pending",
            }),
        )
        .expect("audit: fail-closed");
}

fn append_decision_event(
    audit: &mut AuditTrail,
    request: &ApprovalRequest,
    decision: &ApprovalDecision,
) {
    let decision_result = match decision.decision {
        ApprovalVerdict::Approve => "approved",
        ApprovalVerdict::Deny => "denied",
    };
    let event_name = match decision.decision {
        ApprovalVerdict::Approve => "consent.approved",
        ApprovalVerdict::Deny => "consent.denied",
    };
    audit
        .append_event(
            parse_or_nil_uuid(request.agent_id.as_str()),
            EventType::UserAction,
            json!({
                "event": event_name,
                "request_id": request.id,
                "operation": request.operation.as_str(),
                "required_tier": request.required_tier.as_str(),
                "payload_hash": request.payload_hash,
                "approver_ids": [decision.approver_id.clone()],
                "decision_result": decision_result,
            }),
        )
        .expect("audit: fail-closed");
}

fn append_executed_event(
    audit: &mut AuditTrail,
    request: &ApprovalRequest,
    approver_ids: &[String],
) {
    audit
        .append_event(
            parse_or_nil_uuid(request.agent_id.as_str()),
            EventType::StateChange,
            json!({
                "event": "consent.executed",
                "request_id": request.id,
                "operation": request.operation.as_str(),
                "required_tier": request.required_tier.as_str(),
                "payload_hash": request.payload_hash,
                "approver_ids": approver_ids,
                "decision_result": "executed",
            }),
        )
        .expect("audit: fail-closed");
}

fn next_audit_sequence(audit: &AuditTrail) -> u64 {
    audit.events().len() as u64 + 1
}

fn derive_request_id(
    created_seq: u64,
    operation: GovernedOperation,
    agent_id: Uuid,
    payload_hash: &str,
    requested_by: &str,
    required_tier: HitlTier,
) -> String {
    let canonical = json!({
        "created_seq": created_seq,
        "operation": operation.as_str(),
        "agent_id": agent_id.to_string(),
        "payload_hash": payload_hash,
        "requested_by": requested_by,
        "required_tier": required_tier.as_str(),
    });
    let encoded = serde_json::to_vec(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    let digest = hasher.finalize();
    format!("req-{:x}", digest)
}

fn compute_fingerprint(
    operation: GovernedOperation,
    agent_id: Uuid,
    payload_hash: &str,
    requested_by: &str,
    required_tier: HitlTier,
) -> String {
    let canonical = json!({
        "operation": operation.as_str(),
        "agent_id": agent_id.to_string(),
        "payload_hash": payload_hash,
        "requested_by": requested_by,
        "required_tier": required_tier.as_str(),
    });
    let encoded = serde_json::to_vec(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    format!("{:x}", hasher.finalize())
}

pub fn hash_payload(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
}

fn parse_or_nil_uuid(raw: &str) -> Uuid {
    Uuid::parse_str(raw).unwrap_or(Uuid::nil())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRecord {
    request: ApprovalRequest,
    approvals: BTreeSet<String>,
    denials: BTreeSet<String>,
    decisions: Vec<ApprovalDecision>,
    executed: bool,
    fingerprint: String,
}

impl ApprovalRecord {
    fn approver_ids(&self) -> Vec<String> {
        self.approvals.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueSnapshot {
    records: Vec<ApprovalRecord>,
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalQueue, ConsentError, ConsentPolicyEngine, ConsentRuntime, GovernedOperation,
        HitlTier,
    };
    use crate::audit::AuditTrail;
    use uuid::Uuid;

    #[test]
    fn tier0_operation_executes_without_approval() {
        let mut policy = ConsentPolicyEngine::default();
        policy.set_policy(GovernedOperation::ToolCall, HitlTier::Tier0, Vec::new());
        let mut runtime =
            ConsentRuntime::new(policy, ApprovalQueue::in_memory(), "agent.test".to_string());
        let mut audit = AuditTrail::new();

        let result = runtime.enforce_operation(
            GovernedOperation::ToolCall,
            Uuid::new_v4(),
            b"read:file.rs",
            &mut audit,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn tier2_blocks_without_approval_and_emits_request() {
        let mut policy = ConsentPolicyEngine::default();
        policy.set_policy(
            GovernedOperation::ToolCall,
            HitlTier::Tier2,
            vec!["approver.a".to_string()],
        );
        let mut runtime =
            ConsentRuntime::new(policy, ApprovalQueue::in_memory(), "agent.test".to_string());
        let mut audit = AuditTrail::new();

        let result = runtime.enforce_operation(
            GovernedOperation::ToolCall,
            Uuid::new_v4(),
            b"write:file.rs",
            &mut audit,
        );

        let request_id = match result {
            Err(ConsentError::ApprovalRequired { request_id, .. }) => request_id,
            other => panic!("expected approval required, got: {other:?}"),
        };
        assert!(!request_id.is_empty());

        let requested = audit.events().iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("consent.requested")
        });
        assert!(requested);
    }

    #[test]
    fn tier2_approves_then_executes() {
        let mut policy = ConsentPolicyEngine::default();
        policy.set_policy(
            GovernedOperation::ToolCall,
            HitlTier::Tier2,
            vec!["approver.a".to_string()],
        );
        let mut runtime =
            ConsentRuntime::new(policy, ApprovalQueue::in_memory(), "agent.test".to_string());
        let mut audit = AuditTrail::new();
        let actor = Uuid::new_v4();

        let request_id = match runtime.enforce_operation(
            GovernedOperation::ToolCall,
            actor,
            b"terminal:cargo test",
            &mut audit,
        ) {
            Err(ConsentError::ApprovalRequired { request_id, .. }) => request_id,
            other => panic!("expected approval required, got: {other:?}"),
        };

        let approved = runtime.approve(request_id.as_str(), "approver.a", &mut audit);
        assert!(approved.is_ok());

        let execute = runtime.enforce_operation(
            GovernedOperation::ToolCall,
            actor,
            b"terminal:cargo test",
            &mut audit,
        );
        assert!(execute.is_ok());

        let approved_event = audit.events().iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("consent.approved")
        });
        let executed_event = audit.events().iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str()) == Some("consent.executed")
        });
        assert!(approved_event);
        assert!(executed_event);
    }

    #[test]
    fn tier3_two_person_rule_is_enforced() {
        let mut policy = ConsentPolicyEngine::default();
        policy.set_policy(
            GovernedOperation::SelfMutationApply,
            HitlTier::Tier3,
            vec!["approver.a".to_string(), "approver.b".to_string()],
        );
        let mut runtime =
            ConsentRuntime::new(policy, ApprovalQueue::in_memory(), "agent.test".to_string());
        let mut audit = AuditTrail::new();
        let actor = Uuid::new_v4();

        let request_id = match runtime.enforce_operation(
            GovernedOperation::SelfMutationApply,
            actor,
            b"patch:1",
            &mut audit,
        ) {
            Err(ConsentError::ApprovalRequired { request_id, .. }) => request_id,
            other => panic!("expected approval required, got: {other:?}"),
        };

        assert!(runtime
            .approve(request_id.as_str(), "approver.a", &mut audit)
            .is_ok());
        let still_blocked = runtime.enforce_operation(
            GovernedOperation::SelfMutationApply,
            actor,
            b"patch:1",
            &mut audit,
        );
        assert!(matches!(
            still_blocked,
            Err(ConsentError::ApprovalRequired { .. })
        ));

        let duplicate = runtime.approve(request_id.as_str(), "approver.a", &mut audit);
        assert!(matches!(
            duplicate,
            Err(ConsentError::DuplicateApprover { .. })
        ));

        assert!(runtime
            .approve(request_id.as_str(), "approver.b", &mut audit)
            .is_ok());
        let execute = runtime.enforce_operation(
            GovernedOperation::SelfMutationApply,
            actor,
            b"patch:1",
            &mut audit,
        );
        assert!(execute.is_ok());
    }

    #[test]
    fn payload_is_hashed_not_logged_raw() {
        let mut policy = ConsentPolicyEngine::default();
        policy.set_policy(GovernedOperation::ToolCall, HitlTier::Tier2, Vec::new());
        let mut runtime =
            ConsentRuntime::new(policy, ApprovalQueue::in_memory(), "agent.test".to_string());
        let mut audit = AuditTrail::new();

        let payload = b"secret=top-secret-token";
        let _ = runtime.enforce_operation(
            GovernedOperation::ToolCall,
            Uuid::new_v4(),
            payload,
            &mut audit,
        );

        let requested = audit
            .events()
            .iter()
            .find(|event| {
                event.payload.get("event").and_then(|value| value.as_str())
                    == Some("consent.requested")
            })
            .expect("consent.requested event should be present");
        let payload_hash = requested
            .payload
            .get("payload_hash")
            .and_then(|value| value.as_str())
            .expect("payload hash should be present");
        assert!(!payload_hash.is_empty());

        let serialized = requested.payload.to_string();
        assert!(!serialized.contains("top-secret-token"));
    }
}
