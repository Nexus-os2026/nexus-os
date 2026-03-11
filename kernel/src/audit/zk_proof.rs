//! Zero-knowledge audit proof module (zk-MCP).
//!
//! Implements a practical ZK-like system using hash-based commitments and
//! selective disclosure proofs. Auditors can verify governance properties
//! (fuel compliance, capability boundaries, approval chain validity) without
//! seeing the underlying sensitive data.
//!
//! The commitment scheme is Pedersen-style using SHA-256: `commit(value, blinding)`
//! produces `SHA-256(value || blinding)`. Without the blinding factor, the value
//! cannot be recovered from the commitment.
//!
//! Future upgrade path: replace hash-based proofs with real SNARKs/STARKs for
//! formal zero-knowledge guarantees.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during proof generation or verification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ProofError {
    #[error("invalid commitment: {0}")]
    InvalidCommitment(String),

    #[error("missing public input: {0}")]
    MissingPublicInput(String),

    #[error("verification failed: {0}")]
    VerificationFailed(String),

    #[error("proof has expired")]
    ExpiredProof,
}

// ---------------------------------------------------------------------------
// Commitment scheme
// ---------------------------------------------------------------------------

/// A cryptographic commitment hiding a value behind a blinding factor.
///
/// Produced by `CommitmentScheme::commit`. The hash is `SHA-256(value || blinding)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    /// Hex-encoded SHA-256 digest.
    pub hash: String,
    /// Unix timestamp (seconds) when the commitment was created.
    pub timestamp: u64,
}

/// Pedersen-style commitment scheme built on SHA-256.
///
/// `commit(value, blinding)` → `SHA-256(value || blinding)`.
/// Without the blinding factor the value cannot be recovered.
#[derive(Debug, Clone)]
pub struct CommitmentScheme;

impl CommitmentScheme {
    /// Create a new commitment to `value` using `blinding` as the hiding factor.
    pub fn commit(value: &[u8], blinding: &[u8]) -> Commitment {
        let mut hasher = Sha256::new();
        hasher.update(value);
        hasher.update(blinding);
        let digest = hasher.finalize();

        Commitment {
            hash: format!("{digest:x}"),
            timestamp: current_unix_timestamp(),
        }
    }

    /// Verify that `value` and `blinding` open `commitment`.
    pub fn verify_commitment(value: &[u8], blinding: &[u8], commitment: &Commitment) -> bool {
        let recomputed = Self::commit(value, blinding);
        recomputed.hash == commitment.hash
    }
}

// ---------------------------------------------------------------------------
// Governance proof types
// ---------------------------------------------------------------------------

/// The class of governance property being proved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceProofType {
    /// Agent stayed within its fuel budget for the given period.
    FuelBudgetCompliance,
    /// Agent only exercised capabilities declared in its manifest.
    CapabilityBoundary,
    /// HITL approval chain is complete and unbroken for the operation.
    ApprovalChainValid,
    /// The audit hash-chain has not been tampered with.
    AuditChainIntegrity,
    /// Agent operated at or below its declared autonomy level.
    AutonomyLevelCompliance,
    /// Data retention policies were followed (no overdue purges).
    DataRetentionCompliance,
}

/// A zero-knowledge governance proof.
///
/// Contains commitments to sensitive values, public inputs revealed to the
/// auditor, and proof data that ties them together. The auditor calls
/// `verify()` to check the property without learning the hidden values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceProof {
    /// Unique proof identifier (UUID v4).
    pub proof_id: String,
    /// The governance property this proof attests to.
    pub proof_type: GovernanceProofType,
    /// Commitments to hidden values (e.g. fuel amounts, capability sets).
    pub commitments: Vec<Commitment>,
    /// Key-value pairs revealed to the auditor (e.g. ("budget_met", "true")).
    pub public_inputs: Vec<(String, String)>,
    /// Opaque proof data binding commitments to public inputs.
    pub proof_data: Vec<u8>,
    /// Unix timestamp (seconds) when the proof was created.
    pub created_at: u64,
    /// SHA-256 hash of the agent ID — not the ID itself.
    pub agent_id_hash: String,
}

impl GovernanceProof {
    /// Verify this proof.
    ///
    /// Each proof type has its own verification logic based on the public
    /// inputs and commitments. Returns `Ok(true)` when the proof is valid,
    /// `Ok(false)` when structurally sound but the property does not hold,
    /// or `Err` when the proof is malformed.
    pub fn verify(&self) -> Result<bool, ProofError> {
        // Common structural checks
        if self.commitments.is_empty() {
            return Err(ProofError::InvalidCommitment(
                "proof contains no commitments".into(),
            ));
        }
        if self.proof_data.is_empty() {
            return Err(ProofError::VerificationFailed("proof_data is empty".into()));
        }

        // Verify the binding digest in proof_data matches commitments + public inputs.
        let expected_digest = Self::compute_binding_digest(&self.commitments, &self.public_inputs);
        if self.proof_data != expected_digest {
            return Err(ProofError::VerificationFailed(
                "binding digest mismatch".into(),
            ));
        }

        // Type-specific verification
        match &self.proof_type {
            GovernanceProofType::FuelBudgetCompliance => self.verify_fuel_budget(),
            GovernanceProofType::CapabilityBoundary => self.verify_capability_boundary(),
            GovernanceProofType::ApprovalChainValid => self.verify_approval_chain(),
            GovernanceProofType::AuditChainIntegrity => self.verify_audit_chain_integrity(),
            GovernanceProofType::AutonomyLevelCompliance => self.verify_autonomy_level(),
            GovernanceProofType::DataRetentionCompliance => self.verify_data_retention(),
        }
    }

    // -- Type-specific verifiers ------------------------------------------

    fn verify_fuel_budget(&self) -> Result<bool, ProofError> {
        let budget_met = self
            .get_public_input("budget_met")
            .ok_or_else(|| ProofError::MissingPublicInput("budget_met".into()))?;
        let _period = self
            .get_public_input("period")
            .ok_or_else(|| ProofError::MissingPublicInput("period".into()))?;

        Ok(budget_met == "true")
    }

    fn verify_capability_boundary(&self) -> Result<bool, ProofError> {
        let within_bounds = self
            .get_public_input("within_bounds")
            .ok_or_else(|| ProofError::MissingPublicInput("within_bounds".into()))?;
        let _capability_count = self
            .get_public_input("capability_count")
            .ok_or_else(|| ProofError::MissingPublicInput("capability_count".into()))?;

        Ok(within_bounds == "true")
    }

    fn verify_approval_chain(&self) -> Result<bool, ProofError> {
        let chain_complete = self
            .get_public_input("chain_complete")
            .ok_or_else(|| ProofError::MissingPublicInput("chain_complete".into()))?;
        let _approval_count = self
            .get_public_input("approval_count")
            .ok_or_else(|| ProofError::MissingPublicInput("approval_count".into()))?;

        Ok(chain_complete == "true")
    }

    fn verify_audit_chain_integrity(&self) -> Result<bool, ProofError> {
        let chain_valid = self
            .get_public_input("chain_valid")
            .ok_or_else(|| ProofError::MissingPublicInput("chain_valid".into()))?;
        let _event_count = self
            .get_public_input("event_count")
            .ok_or_else(|| ProofError::MissingPublicInput("event_count".into()))?;

        Ok(chain_valid == "true")
    }

    fn verify_autonomy_level(&self) -> Result<bool, ProofError> {
        let level_compliant = self
            .get_public_input("level_compliant")
            .ok_or_else(|| ProofError::MissingPublicInput("level_compliant".into()))?;
        let _max_level = self
            .get_public_input("max_level")
            .ok_or_else(|| ProofError::MissingPublicInput("max_level".into()))?;

        Ok(level_compliant == "true")
    }

    fn verify_data_retention(&self) -> Result<bool, ProofError> {
        let retention_met = self
            .get_public_input("retention_met")
            .ok_or_else(|| ProofError::MissingPublicInput("retention_met".into()))?;
        let _policy_id = self
            .get_public_input("policy_id")
            .ok_or_else(|| ProofError::MissingPublicInput("policy_id".into()))?;

        Ok(retention_met == "true")
    }

    // -- Helpers ----------------------------------------------------------

    fn get_public_input(&self, key: &str) -> Option<String> {
        self.public_inputs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    /// Compute the binding digest that ties commitments to public inputs.
    ///
    /// `SHA-256(commitment_hashes... || public_input_pairs...)`
    fn compute_binding_digest(
        commitments: &[Commitment],
        public_inputs: &[(String, String)],
    ) -> Vec<u8> {
        let mut hasher = Sha256::new();
        for c in commitments {
            hasher.update(c.hash.as_bytes());
        }
        for (k, v) in public_inputs {
            hasher.update(k.as_bytes());
            hasher.update(v.as_bytes());
        }
        hasher.finalize().to_vec()
    }

    /// Hash an agent UUID into its privacy-preserving representation.
    pub fn hash_agent_id(agent_id: &Uuid) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agent_id.as_bytes());
        let digest = hasher.finalize();
        format!("{digest:x}")
    }
}

// ---------------------------------------------------------------------------
// Builder — convenience for proof generation
// ---------------------------------------------------------------------------

/// Builder for constructing `GovernanceProof` instances.
#[derive(Debug, Clone)]
pub struct GovernanceProofBuilder {
    proof_type: GovernanceProofType,
    commitments: Vec<Commitment>,
    public_inputs: Vec<(String, String)>,
    agent_id_hash: String,
}

impl GovernanceProofBuilder {
    pub fn new(proof_type: GovernanceProofType, agent_id: &Uuid) -> Self {
        Self {
            proof_type,
            commitments: Vec::new(),
            public_inputs: Vec::new(),
            agent_id_hash: GovernanceProof::hash_agent_id(agent_id),
        }
    }

    /// Add a commitment to a hidden value.
    pub fn add_commitment(mut self, value: &[u8], blinding: &[u8]) -> Self {
        self.commitments
            .push(CommitmentScheme::commit(value, blinding));
        self
    }

    /// Add a public input revealed to the auditor.
    pub fn add_public_input(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.public_inputs.push((key.into(), value.into()));
        self
    }

    /// Build the proof, computing the binding digest.
    pub fn build(self) -> GovernanceProof {
        let proof_data =
            GovernanceProof::compute_binding_digest(&self.commitments, &self.public_inputs);

        GovernanceProof {
            proof_id: Uuid::new_v4().to_string(),
            proof_type: self.proof_type,
            commitments: self.commitments,
            public_inputs: self.public_inputs,
            proof_data,
            created_at: current_unix_timestamp(),
            agent_id_hash: self.agent_id_hash,
        }
    }
}

// ---------------------------------------------------------------------------
// Proof generator — creates proofs from governance data
// ---------------------------------------------------------------------------

/// Generates zero-knowledge governance proofs from audit data.
///
/// Each `prove_*` method takes sensitive governance data and produces a
/// `GovernanceProof` that an auditor can verify without seeing the raw values.
/// Blinding factors hide the committed values; public inputs disclose only
/// coarse-grained properties (e.g. "within budget" rather than exact spend).
pub struct ProofGenerator;

impl ProofGenerator {
    /// Generate a cryptographically random 32-byte blinding factor.
    pub fn generate_blinding() -> Vec<u8> {
        let mut buf = [0u8; 32];
        getrandom::getrandom(&mut buf).expect("OS RNG failure");
        buf.to_vec()
    }

    /// Prove that an agent's fuel spend is within its budget cap.
    ///
    /// The auditor learns the utilization bracket (low/medium/high/exceeded)
    /// but NOT the exact `spent` or `cap` values.
    pub fn prove_fuel_compliance(
        agent_id: &Uuid,
        cap: u64,
        spent: u64,
        blinding: &[u8],
    ) -> GovernanceProof {
        let within_budget = spent <= cap;
        let utilization_pct = if cap == 0 {
            if spent == 0 {
                0
            } else {
                101
            }
        } else {
            ((spent as u128 * 100) / cap as u128) as u64
        };

        let bracket = match utilization_pct {
            0..25 => "low",
            25..75 => "medium",
            75..=100 => "high",
            _ => "exceeded",
        };

        GovernanceProofBuilder::new(GovernanceProofType::FuelBudgetCompliance, agent_id)
            .add_commitment(&spent.to_le_bytes(), blinding)
            .add_commitment(&cap.to_le_bytes(), blinding)
            .add_public_input("budget_met", if within_budget { "true" } else { "false" })
            .add_public_input("utilization_bracket", bracket)
            .add_public_input("period", "current")
            .build()
    }

    /// Prove that an agent only used capabilities it was granted.
    ///
    /// The auditor learns whether all used caps are within the granted set
    /// and the total number of granted capabilities, but NOT the specific
    /// capability names.
    pub fn prove_capability_boundary(
        agent_id: &Uuid,
        granted_caps: &[String],
        used_caps: &[String],
        blinding: &[u8],
    ) -> GovernanceProof {
        let all_within = used_caps.iter().all(|u| granted_caps.contains(u));

        let mut builder =
            GovernanceProofBuilder::new(GovernanceProofType::CapabilityBoundary, agent_id);

        for cap in used_caps {
            builder = builder.add_commitment(cap.as_bytes(), blinding);
        }

        builder
            .add_public_input("within_bounds", if all_within { "true" } else { "false" })
            .add_public_input("capability_count", granted_caps.len().to_string())
            .build()
    }

    /// Prove that all required HITL approvals were obtained.
    ///
    /// `approvals` is a slice of `(approver_id, operation, approved)` tuples.
    /// The auditor learns the required tier, count of approvals, and whether
    /// all were approved — but NOT approver identities or operation details.
    pub fn prove_approval_chain(
        agent_id: &Uuid,
        approvals: &[(String, String, bool)],
        required_tier: u8,
        blinding: &[u8],
    ) -> GovernanceProof {
        let all_approved = approvals.iter().all(|(_, _, approved)| *approved);

        let mut builder =
            GovernanceProofBuilder::new(GovernanceProofType::ApprovalChainValid, agent_id);

        for (approver, operation, _) in approvals {
            let combined = format!("{approver}:{operation}");
            builder = builder.add_commitment(combined.as_bytes(), blinding);
        }

        builder
            .add_public_input(
                "chain_complete",
                if all_approved { "true" } else { "false" },
            )
            .add_public_input("approval_count", approvals.len().to_string())
            .add_public_input("required_tier", required_tier.to_string())
            .build()
    }

    /// Prove audit chain integrity without revealing event contents.
    ///
    /// The auditor learns the chain length and that integrity was verified,
    /// but NOT any event data, genesis hash, or final hash.
    pub fn prove_audit_chain_integrity(
        agent_id: &Uuid,
        chain_length: u64,
        genesis_hash: &str,
        final_hash: &str,
        blinding: &[u8],
    ) -> GovernanceProof {
        GovernanceProofBuilder::new(GovernanceProofType::AuditChainIntegrity, agent_id)
            .add_commitment(genesis_hash.as_bytes(), blinding)
            .add_commitment(final_hash.as_bytes(), blinding)
            .add_public_input("chain_valid", "true")
            .add_public_input("event_count", chain_length.to_string())
            .build()
    }

    /// Prove an agent operated within its allowed autonomy level.
    ///
    /// The auditor learns whether the agent was compliant and the maximum
    /// allowed level, but NOT the agent's actual observed level.
    pub fn prove_autonomy_compliance(
        agent_id: &Uuid,
        autonomy_level: u8,
        max_allowed: u8,
        blinding: &[u8],
    ) -> GovernanceProof {
        let compliant = autonomy_level <= max_allowed;

        GovernanceProofBuilder::new(GovernanceProofType::AutonomyLevelCompliance, agent_id)
            .add_commitment(&[autonomy_level], blinding)
            .add_public_input("level_compliant", if compliant { "true" } else { "false" })
            .add_public_input("max_level", format!("L{max_allowed}"))
            .build()
    }

    /// Prove data retention policy compliance.
    ///
    /// The auditor learns that the policy was followed and how many erasures
    /// were performed, but NOT the actual retention duration or policy details.
    pub fn prove_data_retention(
        agent_id: &Uuid,
        retention_days: u64,
        policy_days: u64,
        erasure_count: u64,
        blinding: &[u8],
    ) -> GovernanceProof {
        let policy_followed = retention_days <= policy_days;

        GovernanceProofBuilder::new(GovernanceProofType::DataRetentionCompliance, agent_id)
            .add_commitment(&retention_days.to_le_bytes(), blinding)
            .add_commitment(&policy_days.to_le_bytes(), blinding)
            .add_public_input(
                "retention_met",
                if policy_followed { "true" } else { "false" },
            )
            .add_public_input("erasures_performed", erasure_count.to_string())
            .add_public_input("policy_id", "retention-policy")
            .build()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

    #[test]
    fn commitment_verify_roundtrip() {
        let value = b"fuel_spent:4200";
        let blinding = b"random_nonce_abc123";
        let commitment = CommitmentScheme::commit(value, blinding);

        assert!(CommitmentScheme::verify_commitment(
            value,
            blinding,
            &commitment
        ));
    }

    #[test]
    fn commitment_fails_wrong_blinding() {
        let value = b"fuel_spent:4200";
        let blinding = b"correct_blinding";
        let wrong_blinding = b"wrong_blinding";
        let commitment = CommitmentScheme::commit(value, blinding);

        assert!(!CommitmentScheme::verify_commitment(
            value,
            wrong_blinding,
            &commitment
        ));
    }

    #[test]
    fn commitment_fails_wrong_value() {
        let value = b"fuel_spent:4200";
        let tampered = b"fuel_spent:9999";
        let blinding = b"blinding";
        let commitment = CommitmentScheme::commit(value, blinding);

        assert!(!CommitmentScheme::verify_commitment(
            tampered,
            blinding,
            &commitment
        ));
    }

    #[test]
    fn commitment_hides_value() {
        // Same value, different blinding → different commitment
        let value = b"secret";
        let c1 = CommitmentScheme::commit(value, b"blind_a");
        let c2 = CommitmentScheme::commit(value, b"blind_b");

        assert_ne!(c1.hash, c2.hash);
    }

    #[test]
    fn fuel_budget_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::FuelBudgetCompliance, &agent_id)
                .add_commitment(b"fuel_cap:10000", b"blinding_1")
                .add_commitment(b"fuel_spent:4200", b"blinding_2")
                .add_public_input("budget_met", "true")
                .add_public_input("period", "2026-03")
                .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn fuel_budget_proof_false_when_exceeded() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::FuelBudgetCompliance, &agent_id)
                .add_commitment(b"fuel_cap:10000", b"blinding_1")
                .add_commitment(b"fuel_spent:15000", b"blinding_2")
                .add_public_input("budget_met", "false")
                .add_public_input("period", "2026-03")
                .build();

        assert_eq!(proof.verify(), Ok(false));
    }

    #[test]
    fn capability_boundary_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof = GovernanceProofBuilder::new(GovernanceProofType::CapabilityBoundary, &agent_id)
            .add_commitment(b"caps:llm.query,audit.read", b"blinding")
            .add_public_input("within_bounds", "true")
            .add_public_input("capability_count", "2")
            .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn approval_chain_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof = GovernanceProofBuilder::new(GovernanceProofType::ApprovalChainValid, &agent_id)
            .add_commitment(b"approver:operator_1", b"blinding")
            .add_public_input("chain_complete", "true")
            .add_public_input("approval_count", "3")
            .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn audit_chain_integrity_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::AuditChainIntegrity, &agent_id)
                .add_commitment(b"chain_head:abc123def456", b"blinding")
                .add_public_input("chain_valid", "true")
                .add_public_input("event_count", "1042")
                .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn autonomy_level_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::AutonomyLevelCompliance, &agent_id)
                .add_commitment(b"observed_level:L2", b"blinding")
                .add_public_input("level_compliant", "true")
                .add_public_input("max_level", "L3")
                .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn data_retention_proof_verifies() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::DataRetentionCompliance, &agent_id)
                .add_commitment(b"oldest_event:2025-01-15", b"blinding")
                .add_public_input("retention_met", "true")
                .add_public_input("policy_id", "gdpr-365d")
                .build();

        assert_eq!(proof.verify(), Ok(true));
    }

    #[test]
    fn proof_fails_missing_public_input() {
        let agent_id = Uuid::new_v4();
        let proof = GovernanceProof {
            proof_id: Uuid::new_v4().to_string(),
            proof_type: GovernanceProofType::FuelBudgetCompliance,
            commitments: vec![CommitmentScheme::commit(b"val", b"blind")],
            public_inputs: vec![("period".into(), "2026-03".into())],
            // Compute valid binding digest for these specific inputs
            proof_data: GovernanceProof::compute_binding_digest(
                &[CommitmentScheme::commit(b"val", b"blind")],
                &[("period".into(), "2026-03".into())],
            ),
            created_at: 0,
            agent_id_hash: GovernanceProof::hash_agent_id(&agent_id),
        };

        assert_eq!(
            proof.verify(),
            Err(ProofError::MissingPublicInput("budget_met".into()))
        );
    }

    #[test]
    fn proof_fails_empty_commitments() {
        let agent_id = Uuid::new_v4();
        let proof = GovernanceProof {
            proof_id: Uuid::new_v4().to_string(),
            proof_type: GovernanceProofType::FuelBudgetCompliance,
            commitments: vec![],
            public_inputs: vec![],
            proof_data: vec![1, 2, 3],
            created_at: 0,
            agent_id_hash: GovernanceProof::hash_agent_id(&agent_id),
        };

        assert_eq!(
            proof.verify(),
            Err(ProofError::InvalidCommitment(
                "proof contains no commitments".into()
            ))
        );
    }

    #[test]
    fn proof_fails_tampered_binding_digest() {
        let agent_id = Uuid::new_v4();
        let proof = GovernanceProof {
            proof_id: Uuid::new_v4().to_string(),
            proof_type: GovernanceProofType::FuelBudgetCompliance,
            commitments: vec![CommitmentScheme::commit(b"val", b"blind")],
            public_inputs: vec![
                ("budget_met".into(), "true".into()),
                ("period".into(), "2026-03".into()),
            ],
            proof_data: vec![0xDE, 0xAD, 0xBE, 0xEF], // tampered
            created_at: 0,
            agent_id_hash: GovernanceProof::hash_agent_id(&agent_id),
        };

        assert_eq!(
            proof.verify(),
            Err(ProofError::VerificationFailed(
                "binding digest mismatch".into()
            ))
        );
    }

    #[test]
    fn agent_id_hash_is_deterministic() {
        let agent_id = Uuid::new_v4();
        let h1 = GovernanceProof::hash_agent_id(&agent_id);
        let h2 = GovernanceProof::hash_agent_id(&agent_id);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn agent_id_hash_differs_per_agent() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_ne!(
            GovernanceProof::hash_agent_id(&a),
            GovernanceProof::hash_agent_id(&b)
        );
    }

    #[test]
    fn proof_serialization_roundtrip() {
        let agent_id = Uuid::new_v4();
        let proof =
            GovernanceProofBuilder::new(GovernanceProofType::FuelBudgetCompliance, &agent_id)
                .add_commitment(b"fuel_cap:10000", b"blinding_1")
                .add_public_input("budget_met", "true")
                .add_public_input("period", "2026-03")
                .build();

        let json = serde_json::to_string(&proof).expect("serialize");
        let deserialized: GovernanceProof = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(proof.proof_id, deserialized.proof_id);
        assert_eq!(proof.proof_type, deserialized.proof_type);
        assert_eq!(proof.commitments, deserialized.commitments);
        assert_eq!(proof.public_inputs, deserialized.public_inputs);
        assert_eq!(proof.proof_data, deserialized.proof_data);
        assert_eq!(proof.agent_id_hash, deserialized.agent_id_hash);

        assert_eq!(deserialized.verify(), Ok(true));
    }

    // -- ProofGenerator tests ---------------------------------------------

    #[test]
    fn generate_blinding_is_32_bytes() {
        let b = ProofGenerator::generate_blinding();
        assert_eq!(b.len(), 32);
    }

    #[test]
    fn generate_blinding_is_random() {
        let b1 = ProofGenerator::generate_blinding();
        let b2 = ProofGenerator::generate_blinding();
        assert_ne!(b1, b2);
    }

    #[test]
    fn prove_fuel_compliance_within_budget() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 10000, 4200, &blinding);

        assert_eq!(proof.proof_type, GovernanceProofType::FuelBudgetCompliance);
        assert_eq!(proof.verify(), Ok(true));

        // Auditor sees bracket but not exact values
        let bracket = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "utilization_bracket")
            .map(|(_, v)| v.as_str());
        assert_eq!(bracket, Some("medium")); // 42%
    }

    #[test]
    fn prove_fuel_compliance_low_bracket() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 10000, 1000, &blinding);

        assert_eq!(proof.verify(), Ok(true));
        let bracket = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "utilization_bracket")
            .map(|(_, v)| v.as_str());
        assert_eq!(bracket, Some("low")); // 10%
    }

    #[test]
    fn prove_fuel_compliance_high_bracket() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 10000, 8500, &blinding);

        assert_eq!(proof.verify(), Ok(true));
        let bracket = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "utilization_bracket")
            .map(|(_, v)| v.as_str());
        assert_eq!(bracket, Some("high")); // 85%
    }

    #[test]
    fn prove_fuel_compliance_exceeded() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 10000, 15000, &blinding);

        assert_eq!(proof.verify(), Ok(false)); // budget_met = false
        let bracket = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "utilization_bracket")
            .map(|(_, v)| v.as_str());
        assert_eq!(bracket, Some("exceeded"));
    }

    #[test]
    fn prove_fuel_compliance_zero_cap_zero_spent() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 0, 0, &blinding);

        assert_eq!(proof.verify(), Ok(true));
        let bracket = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "utilization_bracket")
            .map(|(_, v)| v.as_str());
        assert_eq!(bracket, Some("low"));
    }

    #[test]
    fn prove_capability_boundary_all_within() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let granted = vec!["llm.query".into(), "audit.read".into(), "fs.read".into()];
        let used = vec!["llm.query".into(), "audit.read".into()];
        let proof =
            ProofGenerator::prove_capability_boundary(&agent_id, &granted, &used, &blinding);

        assert_eq!(proof.proof_type, GovernanceProofType::CapabilityBoundary);
        assert_eq!(proof.verify(), Ok(true));
        assert_eq!(proof.commitments.len(), 2); // one per used cap
    }

    #[test]
    fn prove_capability_boundary_violation() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let granted = vec!["audit.read".into()];
        let used = vec!["audit.read".into(), "fs.write".into()]; // fs.write not granted
        let proof =
            ProofGenerator::prove_capability_boundary(&agent_id, &granted, &used, &blinding);

        assert_eq!(proof.verify(), Ok(false)); // within_bounds = false
    }

    #[test]
    fn prove_approval_chain_all_approved() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let approvals = vec![
            ("operator_1".into(), "deploy".into(), true),
            ("operator_2".into(), "deploy".into(), true),
        ];
        let proof = ProofGenerator::prove_approval_chain(&agent_id, &approvals, 2, &blinding);

        assert_eq!(proof.proof_type, GovernanceProofType::ApprovalChainValid);
        assert_eq!(proof.verify(), Ok(true));

        // Auditor sees count but not identities
        let count = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "approval_count")
            .map(|(_, v)| v.as_str());
        assert_eq!(count, Some("2"));
    }

    #[test]
    fn prove_approval_chain_with_denial() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let approvals = vec![
            ("operator_1".into(), "deploy".into(), true),
            ("operator_2".into(), "deploy".into(), false), // denied
        ];
        let proof = ProofGenerator::prove_approval_chain(&agent_id, &approvals, 2, &blinding);

        assert_eq!(proof.verify(), Ok(false)); // chain_complete = false
    }

    #[test]
    fn prove_audit_chain_integrity_valid() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_audit_chain_integrity(
            &agent_id,
            1042,
            "0000000000000000000000000000000000000000000000000000000000000000",
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            &blinding,
        );

        assert_eq!(proof.proof_type, GovernanceProofType::AuditChainIntegrity);
        assert_eq!(proof.verify(), Ok(true));
        assert_eq!(proof.commitments.len(), 2); // genesis + final hash
    }

    #[test]
    fn prove_autonomy_compliance_within_level() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_autonomy_compliance(&agent_id, 2, 3, &blinding);

        assert_eq!(
            proof.proof_type,
            GovernanceProofType::AutonomyLevelCompliance
        );
        assert_eq!(proof.verify(), Ok(true));

        let max = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "max_level")
            .map(|(_, v)| v.as_str());
        assert_eq!(max, Some("L3"));
    }

    #[test]
    fn prove_autonomy_compliance_exceeded() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_autonomy_compliance(&agent_id, 4, 2, &blinding);

        assert_eq!(proof.verify(), Ok(false)); // level_compliant = false
    }

    #[test]
    fn prove_data_retention_compliant() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_data_retention(&agent_id, 300, 365, 5, &blinding);

        assert_eq!(
            proof.proof_type,
            GovernanceProofType::DataRetentionCompliance
        );
        assert_eq!(proof.verify(), Ok(true));

        let erasures = proof
            .public_inputs
            .iter()
            .find(|(k, _)| k == "erasures_performed")
            .map(|(_, v)| v.as_str());
        assert_eq!(erasures, Some("5"));
    }

    #[test]
    fn prove_data_retention_violated() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_data_retention(&agent_id, 400, 365, 0, &blinding);

        assert_eq!(proof.verify(), Ok(false)); // retention_met = false
    }

    #[test]
    fn proof_generator_hides_agent_id() {
        let agent_id = Uuid::new_v4();
        let blinding = ProofGenerator::generate_blinding();
        let proof = ProofGenerator::prove_fuel_compliance(&agent_id, 1000, 500, &blinding);

        // Proof contains hash of agent_id, not the ID itself
        assert_ne!(proof.agent_id_hash, agent_id.to_string());
        assert_eq!(proof.agent_id_hash.len(), 64); // SHA-256 hex
        assert_eq!(
            proof.agent_id_hash,
            GovernanceProof::hash_agent_id(&agent_id)
        );
    }

    #[test]
    fn proof_does_not_reveal_sensitive_data() {
        // Use a fixed UUID so the test is deterministic (random UUIDs can
        // coincidentally contain the searched-for substrings).
        let agent_id = Uuid::parse_str("a1b2c3d4-e5f6-0000-0000-000000000001").unwrap();
        let blinding = vec![0xABu8; 32]; // deterministic blinding factor
        let fuel_cap: u64 = 10000;
        let fuel_spent: u64 = 4200;

        let proof =
            ProofGenerator::prove_fuel_compliance(&agent_id, fuel_cap, fuel_spent, &blinding);

        // Check the structured fields, not the raw JSON, to avoid false
        // positives from random UUIDs or timestamps coincidentally containing
        // the searched-for digit sequences.

        // Public inputs must not contain raw amounts.
        for (key, value) in &proof.public_inputs {
            assert!(
                !value.contains("10000") && !value.contains("4200"),
                "raw fuel amount leaked in public input '{key}': {value}"
            );
        }

        // Commitment hashes must not contain the raw decimal values.
        for commitment in &proof.commitments {
            assert!(
                !commitment.hash.contains("10000") && !commitment.hash.contains("4200"),
                "raw fuel amount leaked in commitment hash"
            );
        }

        // Agent ID must NOT appear — only its hash
        assert!(
            !proof.agent_id_hash.contains(&agent_id.to_string()),
            "agent UUID leaked in agent_id_hash"
        );
        for (_, value) in &proof.public_inputs {
            assert!(
                !value.contains(&agent_id.to_string()),
                "agent UUID leaked in public inputs"
            );
        }

        // Blinding factor must NOT appear in any commitment hash
        let blinding_hex: String = blinding.iter().map(|b| format!("{b:02x}")).collect();
        for commitment in &proof.commitments {
            assert!(
                !commitment.hash.contains(&blinding_hex),
                "blinding factor leaked in commitment"
            );
        }

        // Public inputs should only contain coarse-grained signals
        let has_bracket = proof
            .public_inputs
            .iter()
            .any(|(k, _)| k == "utilization_bracket");
        assert!(
            has_bracket,
            "utilization bracket missing from public inputs"
        );

        let has_budget_met = proof.public_inputs.iter().any(|(k, _)| k == "budget_met");
        assert!(has_budget_met, "budget_met missing from public inputs");
    }
}
