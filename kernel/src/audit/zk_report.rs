//! Zero-knowledge audit report for external auditors.
//!
//! Generates a complete compliance report containing ZK governance proofs that
//! auditors can verify without accessing sensitive data (prompts, agent
//! reasoning, payload content). The report is Ed25519-signed for authenticity.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::zk_proof::{GovernanceProof, ProofError, ProofGenerator};
use super::{AuditTrail, EventType};
use crate::hardware_security::{KeyHandle, KeyManager, SignatureBytes};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportError {
    #[error("proof generation failed: {0}")]
    ProofGenerationFailed(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("verification failed: {0}")]
    VerificationFailed(String),

    #[error("audit trail error: {0}")]
    AuditTrailError(String),
}

impl From<ProofError> for ReportError {
    fn from(e: ProofError) -> Self {
        ReportError::VerificationFailed(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for generating a ZK compliance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConfig {
    /// Maximum fuel budget for the reporting period.
    pub fuel_cap: u64,
    /// Maximum allowed autonomy level (0-5).
    pub max_autonomy_level: u8,
    /// Data retention policy in days.
    pub retention_policy_days: u64,
    /// Minimum HITL approval tier required.
    pub required_approval_tier: u8,
}

// ---------------------------------------------------------------------------
// Verification result
// ---------------------------------------------------------------------------

/// Result of verifying all proofs in a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Overall pass/fail.
    pub all_passed: bool,
    /// Per-proof results: (proof_id, proof_type_name, passed, error_message).
    pub proof_results: Vec<ProofVerificationEntry>,
}

/// Individual proof verification outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofVerificationEntry {
    pub proof_id: String,
    pub proof_type: String,
    pub passed: bool,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Report summary
// ---------------------------------------------------------------------------

/// High-level summary embedded in the report. Contains no sensitive data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZkReportSummary {
    /// Number of proofs in this report.
    pub total_proofs: usize,
    /// Whether every proof verified as compliant.
    pub all_compliant: bool,
    /// Per-category compliance status: (category_name, passed).
    pub compliance_categories: Vec<(String, bool)>,
    /// SHA-256 commitment to the number of agents — not the count itself.
    pub agent_count_hash: String,
}

// ---------------------------------------------------------------------------
// ZK Audit Report
// ---------------------------------------------------------------------------

/// A zero-knowledge compliance report that auditors can verify.
///
/// Contains governance proofs, a summary, and an Ed25519 signature.
/// Sensitive data (prompts, agent reasoning, payload content) is NEVER
/// included — only commitments and public compliance signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkAuditReport {
    pub report_id: String,
    pub generated_at: u64,
    pub report_period_start: u64,
    pub report_period_end: u64,
    pub proofs: Vec<GovernanceProof>,
    pub summary: ZkReportSummary,
    pub signature: Vec<u8>,
    pub signer_public_key: Vec<u8>,
}

impl ZkAuditReport {
    /// Generate a ZK compliance report from an audit trail.
    ///
    /// Walks the trail to extract governance data, generates proofs for each
    /// property, and assembles the report. No sensitive data is included.
    pub fn generate(
        audit_trail: &AuditTrail,
        compliance_config: &ComplianceConfig,
    ) -> Result<Self, ReportError> {
        let events = audit_trail.events();
        if events.is_empty() {
            return Err(ReportError::AuditTrailError("audit trail is empty".into()));
        }

        let blinding = ProofGenerator::generate_blinding();

        // Collect distinct agent IDs from the trail
        let mut agent_ids: Vec<Uuid> = events.iter().map(|e| e.agent_id).collect();
        agent_ids.sort();
        agent_ids.dedup();

        // Commitment to agent count (hidden from auditor)
        let agent_count_hash = {
            let mut hasher = Sha256::new();
            hasher.update(agent_ids.len().to_le_bytes());
            hasher.update(&blinding);
            format!("{:x}", hasher.finalize())
        };

        let mut proofs = Vec::new();
        let mut category_results = Vec::new();

        // --- 1. Fuel budget compliance (per agent) ---
        let mut fuel_all_compliant = true;
        for agent_id in &agent_ids {
            let fuel_spent = events
                .iter()
                .filter(|e| &e.agent_id == agent_id && e.event_type == EventType::LlmCall)
                .count() as u64;

            let proof = ProofGenerator::prove_fuel_compliance(
                agent_id,
                compliance_config.fuel_cap,
                fuel_spent,
                &blinding,
            );
            if !proof.verify().map_err(ReportError::from)? {
                fuel_all_compliant = false;
            }
            proofs.push(proof);
        }
        category_results.push(("fuel_budget".into(), fuel_all_compliant));

        // --- 2. Audit chain integrity ---
        let chain_valid = audit_trail.verify_integrity();
        let chain_length = events.len() as u64;
        let genesis_hash = &events[0].previous_hash;
        let final_hash = &events[events.len() - 1].hash;

        // Use a system-level agent ID for chain-level proofs
        let system_agent_id = Uuid::nil();

        if chain_valid {
            let proof = ProofGenerator::prove_audit_chain_integrity(
                &system_agent_id,
                chain_length,
                genesis_hash,
                final_hash,
                &blinding,
            );
            proofs.push(proof);
        }
        category_results.push(("audit_chain_integrity".into(), chain_valid));

        // --- 3. Autonomy level compliance (per agent) ---
        let mut autonomy_all_compliant = true;
        for agent_id in &agent_ids {
            // Derive observed autonomy from event types:
            // LlmCall presence implies at least L1, ToolCall implies L2+
            let has_tool = events
                .iter()
                .any(|e| &e.agent_id == agent_id && e.event_type == EventType::ToolCall);
            let has_llm = events
                .iter()
                .any(|e| &e.agent_id == agent_id && e.event_type == EventType::LlmCall);
            let observed_level = if has_tool {
                2
            } else if has_llm {
                1
            } else {
                0
            };

            let proof = ProofGenerator::prove_autonomy_compliance(
                agent_id,
                observed_level,
                compliance_config.max_autonomy_level,
                &blinding,
            );
            if !proof.verify().map_err(ReportError::from)? {
                autonomy_all_compliant = false;
            }
            proofs.push(proof);
        }
        category_results.push(("autonomy_level".into(), autonomy_all_compliant));

        // --- 4. Approval chain (from UserAction events) ---
        let approval_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::UserAction)
            .collect();

        let approvals: Vec<(String, String, bool)> = approval_events
            .iter()
            .map(|e| {
                let approved = e
                    .payload
                    .get("approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                (
                    GovernanceProof::hash_agent_id(&e.agent_id),
                    "operation".into(),
                    approved,
                )
            })
            .collect();

        let approval_compliant = if approvals.is_empty() {
            true
        } else {
            let proof = ProofGenerator::prove_approval_chain(
                &system_agent_id,
                &approvals,
                compliance_config.required_approval_tier,
                &blinding,
            );
            let result = proof.verify().map_err(ReportError::from)?;
            proofs.push(proof);
            result
        };
        category_results.push(("approval_chain".into(), approval_compliant));

        // --- 5. Data retention ---
        let oldest_timestamp = events.iter().map(|e| e.timestamp).min().unwrap_or(0);
        let now = current_unix_timestamp();
        let retention_days = if oldest_timestamp > 0 {
            now.saturating_sub(oldest_timestamp) / 86400
        } else {
            0
        };

        let retention_proof = ProofGenerator::prove_data_retention(
            &system_agent_id,
            retention_days,
            compliance_config.retention_policy_days,
            0, // erasure count from trail metadata
            &blinding,
        );
        let retention_compliant = retention_proof.verify().map_err(ReportError::from)?;
        proofs.push(retention_proof);
        category_results.push(("data_retention".into(), retention_compliant));

        let all_compliant = category_results.iter().all(|(_, passed)| *passed);
        let total_proofs = proofs.len();

        let period_start = events.iter().map(|e| e.timestamp).min().unwrap_or(0);
        let period_end = events.iter().map(|e| e.timestamp).max().unwrap_or(0);

        Ok(Self {
            report_id: Uuid::new_v4().to_string(),
            generated_at: now,
            report_period_start: period_start,
            report_period_end: period_end,
            proofs,
            summary: ZkReportSummary {
                total_proofs,
                all_compliant,
                compliance_categories: category_results,
                agent_count_hash,
            },
            signature: Vec::new(),
            signer_public_key: Vec::new(),
        })
    }

    /// Sign this report with Ed25519 using the given key handle.
    pub fn sign(
        &mut self,
        key_manager: &KeyManager,
        key_handle: &KeyHandle,
    ) -> Result<(), ReportError> {
        let digest = self.compute_report_digest();
        let SignatureBytes(sig_bytes) = key_manager
            .sign_with_key(key_handle, &digest)
            .map_err(|e| ReportError::SigningFailed(e.to_string()))?;
        self.signature = sig_bytes;

        let pub_key = key_manager
            .public_key_bytes(key_handle)
            .map_err(|e| ReportError::SigningFailed(e.to_string()))?;
        self.signer_public_key = pub_key.0;

        Ok(())
    }

    /// Verify the Ed25519 signature on this report.
    pub fn verify_signature(&self) -> Result<bool, ReportError> {
        if self.signature.is_empty() || self.signer_public_key.is_empty() {
            return Ok(false);
        }

        let pub_bytes: [u8; 32] = self
            .signer_public_key
            .as_slice()
            .try_into()
            .map_err(|_| ReportError::VerificationFailed("invalid public key length".into()))?;

        let verifying_key = VerifyingKey::from_bytes(&pub_bytes)
            .map_err(|e| ReportError::VerificationFailed(format!("invalid public key: {e}")))?;

        let sig_bytes: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| ReportError::VerificationFailed("invalid signature length".into()))?;

        let signature = Signature::from_bytes(&sig_bytes);
        let digest = self.compute_report_digest();

        Ok(verifying_key.verify(&digest, &signature).is_ok())
    }

    /// Verify every proof in this report.
    pub fn verify_all_proofs(&self) -> Result<VerificationResult, ReportError> {
        let mut proof_results = Vec::new();
        let mut all_passed = true;

        for proof in &self.proofs {
            let (passed, detail) = match proof.verify() {
                Ok(true) => (true, "verified".into()),
                Ok(false) => {
                    all_passed = false;
                    (false, "property does not hold".into())
                }
                Err(e) => {
                    all_passed = false;
                    (false, e.to_string())
                }
            };

            proof_results.push(ProofVerificationEntry {
                proof_id: proof.proof_id.clone(),
                proof_type: format!("{:?}", proof.proof_type),
                passed,
                detail,
            });
        }

        Ok(VerificationResult {
            all_passed,
            proof_results,
        })
    }

    /// Serialize the report to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a report from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Compute SHA-256 digest over report content (excluding signature fields).
    fn compute_report_digest(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.report_id.as_bytes());
        hasher.update(self.generated_at.to_le_bytes());
        hasher.update(self.report_period_start.to_le_bytes());
        hasher.update(self.report_period_end.to_le_bytes());

        for proof in &self.proofs {
            hasher.update(proof.proof_id.as_bytes());
            for c in &proof.commitments {
                hasher.update(c.hash.as_bytes());
            }
            hasher.update(&proof.proof_data);
        }

        hasher.update(self.summary.total_proofs.to_le_bytes());
        hasher.update([u8::from(self.summary.all_compliant)]);
        hasher.update(self.summary.agent_count_hash.as_bytes());

        hasher.finalize().to_vec()
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditTrail, EventType};
    use crate::hardware_security::{KeyBackendKind, KeyManagerConfig, KeyPurpose};
    use serde_json::json;

    fn sample_trail(event_count: usize) -> AuditTrail {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        for i in 0..event_count {
            let event_type = match i % 3 {
                0 => EventType::StateChange,
                1 => EventType::LlmCall,
                _ => EventType::ToolCall,
            };
            trail
                .append_event(agent_id, event_type, json!({"seq": i}))
                .expect("append");
        }
        trail
    }

    fn default_config() -> ComplianceConfig {
        ComplianceConfig {
            fuel_cap: 10000,
            max_autonomy_level: 3,
            retention_policy_days: 365,
            required_approval_tier: 1,
        }
    }

    fn make_key_manager() -> (KeyManager, KeyHandle) {
        let config = KeyManagerConfig {
            preferred_backend: KeyBackendKind::Software,
            enable_hardware: false,
            sealed_store_dir: None,
        };
        let mut km = KeyManager::from_config(config);
        let mut audit = AuditTrail::new();
        let handle = km
            .generate_key(KeyPurpose::AuditSigning, &mut audit, Uuid::new_v4())
            .expect("generate key");
        (km, handle)
    }

    #[test]
    fn generate_report_from_trail() {
        let trail = sample_trail(30);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        assert!(!report.report_id.is_empty());
        assert!(!report.proofs.is_empty());
        assert!(report.summary.total_proofs > 0);
        assert!(!report.summary.agent_count_hash.is_empty());
        assert_eq!(report.summary.agent_count_hash.len(), 64);
    }

    #[test]
    fn report_empty_trail_errors() {
        let trail = AuditTrail::new();
        let config = default_config();
        let result = ZkAuditReport::generate(&trail, &config);

        assert!(result.is_err());
        assert!(matches!(result, Err(ReportError::AuditTrailError(_))));
    }

    #[test]
    fn report_all_proofs_verify() {
        let trail = sample_trail(15);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let result = report.verify_all_proofs().expect("verify");
        assert!(result.all_passed);
        assert!(result.proof_results.iter().all(|r| r.passed));
    }

    #[test]
    fn report_categories_present() {
        let trail = sample_trail(10);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let categories: Vec<&str> = report
            .summary
            .compliance_categories
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();

        assert!(categories.contains(&"fuel_budget"));
        assert!(categories.contains(&"audit_chain_integrity"));
        assert!(categories.contains(&"autonomy_level"));
        assert!(categories.contains(&"data_retention"));
    }

    #[test]
    fn report_sign_and_verify() {
        let trail = sample_trail(10);
        let config = default_config();
        let mut report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let (km, handle) = make_key_manager();
        report.sign(&km, &handle).expect("sign");

        assert!(!report.signature.is_empty());
        assert!(!report.signer_public_key.is_empty());
        assert_eq!(report.signature.len(), 64);
        assert_eq!(report.signer_public_key.len(), 32);

        assert_eq!(report.verify_signature(), Ok(true));
    }

    #[test]
    fn unsigned_report_verify_returns_false() {
        let trail = sample_trail(10);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        assert_eq!(report.verify_signature(), Ok(false));
    }

    #[test]
    fn tampered_report_signature_fails() {
        let trail = sample_trail(10);
        let config = default_config();
        let mut report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let (km, handle) = make_key_manager();
        report.sign(&km, &handle).expect("sign");

        // Tamper with report content
        report.report_id = Uuid::new_v4().to_string();

        assert_eq!(report.verify_signature(), Ok(false));
    }

    #[test]
    fn report_json_roundtrip() {
        let trail = sample_trail(10);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let json = report.to_json().expect("to_json");
        let deserialized = ZkAuditReport::from_json(&json).expect("from_json");

        assert_eq!(report.report_id, deserialized.report_id);
        assert_eq!(report.proofs.len(), deserialized.proofs.len());
        assert_eq!(
            report.summary.all_compliant,
            deserialized.summary.all_compliant
        );

        // Deserialized proofs still verify
        let result = deserialized.verify_all_proofs().expect("verify");
        assert!(result.all_passed);
    }

    #[test]
    fn signed_report_json_roundtrip() {
        let trail = sample_trail(10);
        let config = default_config();
        let mut report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let (km, handle) = make_key_manager();
        report.sign(&km, &handle).expect("sign");

        let json = report.to_json().expect("to_json");
        let deserialized = ZkAuditReport::from_json(&json).expect("from_json");

        assert_eq!(deserialized.verify_signature(), Ok(true));
    }

    #[test]
    fn report_with_multiple_agents() {
        let mut trail = AuditTrail::new();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();

        for i in 0..5 {
            trail
                .append_event(agent_a, EventType::LlmCall, json!({"i": i}))
                .expect("append");
            trail
                .append_event(agent_b, EventType::ToolCall, json!({"i": i}))
                .expect("append");
        }

        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        // Should have fuel proofs for each agent + chain + autonomy per agent + retention
        // 2 fuel + 1 chain + 2 autonomy + 1 retention = 6
        assert!(report.proofs.len() >= 6);

        let result = report.verify_all_proofs().expect("verify");
        assert!(result.all_passed);
    }

    #[test]
    fn report_with_user_actions() {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        trail
            .append_event(
                agent_id,
                EventType::UserAction,
                json!({"approved": true, "tier": 1}),
            )
            .expect("append");
        trail
            .append_event(agent_id, EventType::StateChange, json!({"status": "ok"}))
            .expect("append");

        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let categories: Vec<&str> = report
            .summary
            .compliance_categories
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(categories.contains(&"approval_chain"));

        let result = report.verify_all_proofs().expect("verify");
        assert!(result.all_passed);
    }

    #[test]
    fn report_period_matches_trail_bounds() {
        let trail = sample_trail(5);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        let events = trail.events();
        let min_ts = events.iter().map(|e| e.timestamp).min().unwrap();
        let max_ts = events.iter().map(|e| e.timestamp).max().unwrap();

        assert_eq!(report.report_period_start, min_ts);
        assert_eq!(report.report_period_end, max_ts);
    }

    #[test]
    fn report_summary_accurate() {
        let trail = sample_trail(20);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");

        // summary.total_proofs must match actual proof count
        assert_eq!(report.summary.total_proofs, report.proofs.len());

        // Verify each proof individually and compare to summary.all_compliant
        let verification = report.verify_all_proofs().expect("verify");
        assert_eq!(report.summary.all_compliant, verification.all_passed);

        // Each category in summary must be present and consistent
        for (category, passed) in &report.summary.compliance_categories {
            assert!(!category.is_empty(), "empty category name in summary");
            // If summary says all compliant, no category should be false
            if report.summary.all_compliant {
                assert!(
                    passed,
                    "category {category} is false but summary says all_compliant"
                );
            }
        }

        // Agent count hash must be a valid SHA-256 hex string
        assert_eq!(report.summary.agent_count_hash.len(), 64);
    }
}
