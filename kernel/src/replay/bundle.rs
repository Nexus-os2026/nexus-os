use crate::audit::{AuditEvent, AuditTrail};
use crate::autonomy::AutonomyLevel;
use crate::consent::{ApprovalDecision, GovernedOperation, HitlTier};
use crate::manifest::AgentManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const BUNDLE_FORMAT_VERSION: u8 = 2;

/// Snapshot of the governance policy active during the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySnapshot {
    pub autonomy_level: AutonomyLevel,
    pub consent_tiers: BTreeMap<String, HitlTier>,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
}

impl Default for PolicySnapshot {
    fn default() -> Self {
        Self {
            autonomy_level: AutonomyLevel::L0,
            consent_tiers: BTreeMap::new(),
            capabilities: Vec::new(),
            fuel_budget: 0,
        }
    }
}

/// An approval record that pairs an output event-id with the decision that authorised it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub output_event_id: Uuid,
    pub operation: GovernedOperation,
    pub decision: ApprovalDecision,
}

/// A self-contained, independently verifiable evidence bundle for a single agent run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub version: u8,
    pub bundle_id: Uuid,
    pub agent_id: Uuid,
    pub run_id: Uuid,
    pub manifest_hash: String,
    pub policy_snapshot: PolicySnapshot,
    pub audit_events: Vec<AuditEvent>,
    pub chain_root_hash: String,
    pub inputs: Vec<serde_json::Value>,
    pub outputs: Vec<serde_json::Value>,
    pub fuel_consumed: u64,
    pub fuel_budget: u64,
    pub autonomy_level: AutonomyLevel,
    pub approval_records: Vec<ApprovalRecord>,
    pub exported_at: u64,
    pub bundle_digest: String,
}

/// Compute the SHA-256 manifest hash from the canonical TOML representation.
pub fn compute_manifest_hash(manifest: &AgentManifest) -> String {
    let canonical = serde_json::to_vec(manifest).expect("manifest serialization must not fail");
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    format!("{:x}", hasher.finalize())
}

impl EvidenceBundle {
    /// Build an evidence bundle from a completed agent run.
    ///
    /// The caller provides all run artefacts; this function validates the
    /// audit trail integrity and computes the bundle digest.
    #[allow(clippy::too_many_arguments)]
    pub fn export(
        agent_id: Uuid,
        run_id: Uuid,
        manifest: &AgentManifest,
        policy: PolicySnapshot,
        trail: &AuditTrail,
        inputs: Vec<serde_json::Value>,
        outputs: Vec<serde_json::Value>,
        fuel_consumed: u64,
        fuel_budget: u64,
        autonomy_level: AutonomyLevel,
        approval_records: Vec<ApprovalRecord>,
    ) -> Result<Self, BundleError> {
        let events = trail.events();
        if events.is_empty() {
            return Err(BundleError::EmptyTrail);
        }

        if !trail.verify_integrity() {
            return Err(BundleError::IntegrityViolation(
                "audit trail hash chain is broken".to_string(),
            ));
        }

        let chain_root_hash = events.last().map(|e| e.hash.clone()).unwrap_or_default();

        let manifest_hash = compute_manifest_hash(manifest);

        let bundle_id = Uuid::new_v4();
        let exported_at = current_unix_timestamp();
        let audit_events = events.to_vec();

        let bundle_digest = compute_bundle_digest(
            bundle_id,
            agent_id,
            run_id,
            &manifest_hash,
            &chain_root_hash,
            &audit_events,
            exported_at,
        );

        Ok(Self {
            version: BUNDLE_FORMAT_VERSION,
            bundle_id,
            agent_id,
            run_id,
            manifest_hash,
            policy_snapshot: policy,
            audit_events,
            chain_root_hash,
            inputs,
            outputs,
            fuel_consumed,
            fuel_budget,
            autonomy_level,
            approval_records,
            exported_at,
            bundle_digest,
        })
    }
}

fn compute_bundle_digest(
    bundle_id: Uuid,
    agent_id: Uuid,
    run_id: Uuid,
    manifest_hash: &str,
    chain_root_hash: &str,
    events: &[AuditEvent],
    exported_at: u64,
) -> String {
    #[derive(Serialize)]
    struct DigestInput<'a> {
        bundle_id: String,
        agent_id: String,
        run_id: String,
        manifest_hash: &'a str,
        chain_root_hash: &'a str,
        event_count: usize,
        exported_at: u64,
    }

    let input = DigestInput {
        bundle_id: bundle_id.to_string(),
        agent_id: agent_id.to_string(),
        run_id: run_id.to_string(),
        manifest_hash,
        chain_root_hash,
        event_count: events.len(),
        exported_at,
    };

    let canonical = serde_json::to_vec(&input).expect("bundle digest serialization must not fail");

    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    for event in events {
        hasher.update(event.hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleError {
    #[error("cannot export empty audit trail")]
    EmptyTrail,
    #[error("integrity violation: {0}")]
    IntegrityViolation(String),
    #[error("format error: {0}")]
    FormatError(String),
    #[error("verification failed: {}", .0.join("; "))]
    VerificationFailed(Vec<String>),
}
