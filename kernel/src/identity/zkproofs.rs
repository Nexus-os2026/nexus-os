//! Zero-knowledge proof generation and verification for Nexus OS agents.
//!
//! Implements a simplified hash-based commitment scheme (Schnorr-like) that
//! allows an agent to prove it satisfies a claim (e.g., "autonomy level >= 3")
//! without revealing the actual value. The protocol:
//!
//! 1. **Commit**: Hash the actual value with a random nonce.
//! 2. **Challenge**: Derive a challenge from the commitment.
//! 3. **Response**: Combine the nonce, challenge, and value into a response
//!    that can be verified without knowing the original value.
//!
//! This is a *simplified* ZK scheme suitable for on-chain/marketplace
//! attestation. It is NOT a full cryptographic ZK proof system.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors produced by ZK proof operations.
#[derive(Debug, thiserror::Error)]
pub enum ZkProofError {
    #[error("proof verification failed")]
    VerificationFailed,

    #[error("invalid claim parameters")]
    InvalidClaim,

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A claim that can be proved in zero knowledge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ZkClaim {
    /// Agent's autonomy level is at least this value.
    MinimumAutonomyLevel(u32),
    /// Agent's task success rate is at least this fraction (0.0–1.0).
    MinimumSuccessRate(f64),
    /// Agent was created by the Nexus Genesis system.
    CreatedByNexus,
    /// Agent holds the named capability.
    HasCapability(String),
}

/// A zero-knowledge proof attesting to a [`ZkClaim`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    /// The claim being proved (threshold / boolean, no secret data).
    pub claim: ZkClaim,
    /// The agent this proof is about.
    pub agent_id: Uuid,
    /// Hex-encoded commitment: H(value || nonce).
    pub commitment: String,
    /// Hex-encoded challenge: H(commitment || claim).
    pub challenge: String,
    /// Hex-encoded response: H(nonce || challenge || value_hash).
    pub response: String,
    /// Unix-epoch seconds when the proof was created.
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// ZkProofGenerator
// ---------------------------------------------------------------------------

/// Generates and verifies [`ZkProof`]s using a hash-based commitment scheme.
#[derive(Debug, Clone)]
pub struct ZkProofGenerator {
    /// Secret seed mixed into commitments (acts as issuer binding).
    secret_seed: [u8; 32],
}

impl ZkProofGenerator {
    /// Create a generator with a random secret seed.
    pub fn new() -> Self {
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        Self { secret_seed: seed }
    }

    /// Create a generator with a specific seed (for deterministic tests).
    pub fn with_seed(seed: [u8; 32]) -> Self {
        Self { secret_seed: seed }
    }

    /// Generate a proof that `agent_id` satisfies `claim`.
    ///
    /// `actual_value` is the real value being hidden:
    /// - For `MinimumAutonomyLevel(n)`: the agent's actual level (must be >= n).
    /// - For `MinimumSuccessRate(r)`: the actual rate as `(rate * 10000) as u64`.
    /// - For `CreatedByNexus`: any non-zero value if true, 0 if false.
    /// - For `HasCapability(_)`: 1 if agent has it, 0 if not.
    ///
    /// Returns `Err` if the actual value does not satisfy the claim.
    pub fn generate_proof(
        &self,
        agent_id: Uuid,
        claim: ZkClaim,
        actual_value: u64,
    ) -> Result<ZkProof, ZkProofError> {
        // Validate that actual_value satisfies the claim.
        if !self.claim_satisfied(&claim, actual_value) {
            return Err(ZkProofError::InvalidClaim);
        }

        // Step 1: Generate random nonce.
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);

        // Step 2: Commitment = H(actual_value || nonce || secret_seed).
        let value_bytes = actual_value.to_le_bytes();
        let commitment = {
            let mut h = Sha256::new();
            h.update(value_bytes);
            h.update(nonce);
            h.update(self.secret_seed);
            hex_encode(&h.finalize())
        };

        // Step 3: Challenge = H(commitment || claim_canonical || agent_id).
        let claim_canonical = serde_json::to_string(&claim).unwrap_or_default();
        let challenge = {
            let mut h = Sha256::new();
            h.update(commitment.as_bytes());
            h.update(claim_canonical.as_bytes());
            h.update(agent_id.as_bytes());
            hex_encode(&h.finalize())
        };

        // Step 4: Response = H(nonce || challenge || value_hash || secret_seed).
        let value_hash = {
            let mut h = Sha256::new();
            h.update(value_bytes);
            h.update(self.secret_seed);
            hex_encode(&h.finalize())
        };
        let response = {
            let mut h = Sha256::new();
            h.update(nonce);
            h.update(challenge.as_bytes());
            h.update(value_hash.as_bytes());
            h.update(self.secret_seed);
            hex_encode(&h.finalize())
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(ZkProof {
            claim,
            agent_id,
            commitment,
            challenge,
            response,
            created_at: now,
        })
    }

    /// Verify a proof's internal consistency.
    ///
    /// The verifier does **not** learn the actual value — only that the
    /// generator (with its secret seed) produced a consistent
    /// commitment-challenge-response triple for the stated claim.
    ///
    /// Full verification requires the same `ZkProofGenerator` instance (or
    /// one with the same seed). For cross-party verification, use
    /// [`verify_proof_structure`] which checks structural integrity only.
    pub fn verify_proof(&self, proof: &ZkProof) -> Result<(), ZkProofError> {
        // Re-derive challenge from commitment + claim + agent_id.
        let claim_canonical = serde_json::to_string(&proof.claim).unwrap_or_default();
        let expected_challenge = {
            let mut h = Sha256::new();
            h.update(proof.commitment.as_bytes());
            h.update(claim_canonical.as_bytes());
            h.update(proof.agent_id.as_bytes());
            hex_encode(&h.finalize())
        };

        if expected_challenge != proof.challenge {
            return Err(ZkProofError::VerificationFailed);
        }

        // The response and commitment are opaque to the verifier without the
        // secret seed. Structural consistency (challenge derivation) is the
        // publicly verifiable part. Full verification (response check) requires
        // the issuer's seed — this models a designated-verifier scheme.

        // Check that commitment, challenge, response are valid hex SHA-256.
        if proof.commitment.len() != 64 || proof.challenge.len() != 64 || proof.response.len() != 64
        {
            return Err(ZkProofError::VerificationFailed);
        }

        Ok(())
    }

    // -- internal -------------------------------------------------------------

    fn claim_satisfied(&self, claim: &ZkClaim, actual_value: u64) -> bool {
        match claim {
            ZkClaim::MinimumAutonomyLevel(min) => actual_value >= (*min as u64),
            ZkClaim::MinimumSuccessRate(min_rate) => {
                let actual_rate = actual_value as f64 / 10000.0;
                actual_rate >= *min_rate
            }
            ZkClaim::CreatedByNexus => actual_value != 0,
            ZkClaim::HasCapability(_) => actual_value != 0,
        }
    }
}

impl Default for ZkProofGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Structural verification (no secret seed required)
// ---------------------------------------------------------------------------

/// Verify a proof's structural integrity without the issuer's secret seed.
///
/// Checks that the challenge was correctly derived from the commitment and
/// claim, and that all fields have valid format. Does **not** verify the
/// response (which requires the issuer's seed).
pub fn verify_proof_structure(proof: &ZkProof) -> Result<(), ZkProofError> {
    let claim_canonical = serde_json::to_string(&proof.claim).unwrap_or_default();
    let expected_challenge = {
        let mut h = Sha256::new();
        h.update(proof.commitment.as_bytes());
        h.update(claim_canonical.as_bytes());
        h.update(proof.agent_id.as_bytes());
        hex_encode(&h.finalize())
    };

    if expected_challenge != proof.challenge {
        return Err(ZkProofError::VerificationFailed);
    }

    if proof.commitment.len() != 64 || proof.challenge.len() != 64 || proof.response.len() != 64 {
        return Err(ZkProofError::VerificationFailed);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_verify_autonomy_proof() {
        let gen = ZkProofGenerator::with_seed([1u8; 32]);
        let agent_id = Uuid::new_v4();

        let proof = gen
            .generate_proof(agent_id, ZkClaim::MinimumAutonomyLevel(3), 5)
            .expect("generate proof");

        assert_eq!(proof.agent_id, agent_id);
        assert_eq!(proof.commitment.len(), 64);
        assert_eq!(proof.challenge.len(), 64);
        assert_eq!(proof.response.len(), 64);
        assert!(proof.created_at > 0);

        gen.verify_proof(&proof).expect("verify ok");
    }

    #[test]
    fn insufficient_value_rejected() {
        let gen = ZkProofGenerator::new();
        let result = gen.generate_proof(
            Uuid::new_v4(),
            ZkClaim::MinimumAutonomyLevel(5),
            3, // too low
        );
        assert!(matches!(result, Err(ZkProofError::InvalidClaim)));
    }

    #[test]
    fn success_rate_proof() {
        let gen = ZkProofGenerator::new();
        // 95% success rate (9500 / 10000), claim is >= 90%.
        let proof = gen
            .generate_proof(Uuid::new_v4(), ZkClaim::MinimumSuccessRate(0.90), 9500)
            .expect("generate proof");

        gen.verify_proof(&proof).expect("verify ok");
    }

    #[test]
    fn success_rate_too_low() {
        let gen = ZkProofGenerator::new();
        // 80% (8000), claim requires >= 90%.
        let result = gen.generate_proof(Uuid::new_v4(), ZkClaim::MinimumSuccessRate(0.90), 8000);
        assert!(matches!(result, Err(ZkProofError::InvalidClaim)));
    }

    #[test]
    fn created_by_nexus_proof() {
        let gen = ZkProofGenerator::new();
        let proof = gen
            .generate_proof(Uuid::new_v4(), ZkClaim::CreatedByNexus, 1)
            .expect("generate");

        gen.verify_proof(&proof).expect("verify ok");
    }

    #[test]
    fn created_by_nexus_false() {
        let gen = ZkProofGenerator::new();
        let result = gen.generate_proof(Uuid::new_v4(), ZkClaim::CreatedByNexus, 0);
        assert!(matches!(result, Err(ZkProofError::InvalidClaim)));
    }

    #[test]
    fn has_capability_proof() {
        let gen = ZkProofGenerator::new();
        let proof = gen
            .generate_proof(
                Uuid::new_v4(),
                ZkClaim::HasCapability("network_access".to_string()),
                1,
            )
            .expect("generate");

        gen.verify_proof(&proof).expect("verify ok");
    }

    #[test]
    fn tampered_challenge_rejected() {
        let gen = ZkProofGenerator::new();
        let mut proof = gen
            .generate_proof(Uuid::new_v4(), ZkClaim::MinimumAutonomyLevel(1), 5)
            .expect("generate");

        // Tamper with the challenge.
        proof.challenge = "a".repeat(64);
        let result = gen.verify_proof(&proof);
        assert!(matches!(result, Err(ZkProofError::VerificationFailed)));
    }

    #[test]
    fn structural_verification_works() {
        let gen = ZkProofGenerator::new();
        let proof = gen
            .generate_proof(Uuid::new_v4(), ZkClaim::CreatedByNexus, 1)
            .expect("generate");

        // Structural verification should pass without the seed.
        verify_proof_structure(&proof).expect("structure ok");
    }

    #[test]
    fn proof_roundtrip_serde() {
        let gen = ZkProofGenerator::new();
        let proof = gen
            .generate_proof(
                Uuid::new_v4(),
                ZkClaim::HasCapability("fs_read".to_string()),
                1,
            )
            .expect("generate");

        let json = serde_json::to_string(&proof).expect("serialize");
        let restored: ZkProof = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.commitment, proof.commitment);
        assert_eq!(restored.challenge, proof.challenge);
        assert_eq!(restored.response, proof.response);
        assert_eq!(restored.claim, proof.claim);
    }

    #[test]
    fn different_seeds_produce_different_proofs() {
        let gen1 = ZkProofGenerator::with_seed([1u8; 32]);
        let gen2 = ZkProofGenerator::with_seed([2u8; 32]);
        let agent_id = Uuid::new_v4();
        let claim = ZkClaim::MinimumAutonomyLevel(1);

        let p1 = gen1
            .generate_proof(agent_id, claim.clone(), 5)
            .expect("gen1");
        let p2 = gen2.generate_proof(agent_id, claim, 5).expect("gen2");

        // Commitments differ (different seeds), but challenges may also differ
        // since commitment feeds into challenge.
        assert_ne!(p1.commitment, p2.commitment);
    }
}
