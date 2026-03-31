//! Cryptographic token construction and verification.
//!
//! `CapabilityBudget` — Ed25519-signed, monotonically decreasing, child-derivable.

use std::collections::HashMap;

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Capability budget — cryptographically sealed, monotonically decreasing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityBudget {
    pub agent_id: String,
    pub allocations: HashMap<String, u64>,
    pub initial_hash: String,
    pub current_hash: String,
    pub authority_signature: Vec<u8>,
    pub version: u64,
}

impl CapabilityBudget {
    /// Create a new budget for an agent, signed by the oracle authority.
    pub fn new(
        agent_id: String,
        allocations: HashMap<String, u64>,
        identity: &CryptoIdentity,
    ) -> Self {
        let initial_hash = Self::compute_hash(&agent_id, &allocations, 0);
        let signature = identity.sign(initial_hash.as_bytes()).unwrap_or_default();

        Self {
            agent_id,
            allocations,
            initial_hash: initial_hash.clone(),
            current_hash: initial_hash,
            authority_signature: signature,
            version: 0,
        }
    }

    /// Spend tokens from a capability category.
    pub fn spend(
        &mut self,
        capability: &str,
        amount: u64,
        identity: &CryptoIdentity,
    ) -> Result<(), BudgetError> {
        let current = self
            .allocations
            .get(capability)
            .copied()
            .ok_or_else(|| BudgetError::UnknownCapability(capability.to_string()))?;

        if current < amount {
            return Err(BudgetError::InsufficientBudget {
                capability: capability.to_string(),
                requested: amount,
                available: current,
            });
        }

        self.allocations
            .insert(capability.to_string(), current - amount);
        self.version += 1;
        self.current_hash = Self::compute_hash(&self.agent_id, &self.allocations, self.version);
        let signature = identity
            .sign(self.current_hash.as_bytes())
            .map_err(|e| BudgetError::IntegrityViolation(e.to_string()))?;
        self.authority_signature = signature;
        Ok(())
    }

    /// Derive a child budget. Child always gets <= parent's remaining budget.
    pub fn derive_child(
        &self,
        child_agent_id: String,
        fraction: f64,
        identity: &CryptoIdentity,
    ) -> Result<CapabilityBudget, BudgetError> {
        if !(0.0..=1.0).contains(&fraction) {
            return Err(BudgetError::InvalidFraction(fraction));
        }

        let child_allocations: HashMap<String, u64> = self
            .allocations
            .iter()
            .map(|(k, v)| (k.clone(), (*v as f64 * fraction).floor() as u64))
            .collect();

        Ok(CapabilityBudget::new(
            child_agent_id,
            child_allocations,
            identity,
        ))
    }

    /// Verify budget integrity against the authority signature.
    pub fn verify(&self, verifying_key: &[u8]) -> Result<(), BudgetError> {
        let expected_hash = Self::compute_hash(&self.agent_id, &self.allocations, self.version);
        if expected_hash != self.current_hash {
            return Err(BudgetError::IntegrityViolation("Hash mismatch".to_string()));
        }

        let ok = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            verifying_key,
            self.current_hash.as_bytes(),
            &self.authority_signature,
        )
        .map_err(|e| BudgetError::IntegrityViolation(e.to_string()))?;

        if !ok {
            return Err(BudgetError::IntegrityViolation(
                "Signature verification failed".into(),
            ));
        }

        Ok(())
    }

    /// Deterministic hash over agent_id + sorted allocations + version.
    pub fn compute_hash(
        agent_id: &str,
        allocations: &HashMap<String, u64>,
        version: u64,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agent_id.as_bytes());
        let mut sorted: Vec<_> = allocations.iter().collect();
        sorted.sort_by_key(|(k, _)| (*k).clone());
        for (k, v) in sorted {
            hasher.update(k.as_bytes());
            hasher.update(v.to_le_bytes());
        }
        hasher.update(version.to_le_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("Unknown capability: {0}")]
    UnknownCapability(String),
    #[error("Insufficient budget for {capability}: requested {requested}, available {available}")]
    InsufficientBudget {
        capability: String,
        requested: u64,
        available: u64,
    },
    #[error("Invalid fraction: {0}")]
    InvalidFraction(f64),
    #[error("Budget integrity violation: {0}")]
    IntegrityViolation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> CryptoIdentity {
        CryptoIdentity::generate(SignatureAlgorithm::Ed25519).expect("keygen should succeed")
    }

    fn sample_allocations() -> HashMap<String, u64> {
        let mut m = HashMap::new();
        m.insert("llm.query".into(), 1000);
        m.insert("fs.write".into(), 500);
        m
    }

    #[test]
    fn test_budget_creation_and_spending() {
        let id = test_identity();
        let vk = id.verifying_key().to_vec();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &id);

        assert_eq!(budget.allocations["llm.query"], 1000);
        assert!(budget.verify(&vk).is_ok());

        budget.spend("llm.query", 100, &id).unwrap();
        assert_eq!(budget.allocations["llm.query"], 900);
        assert_eq!(budget.version, 1);
        assert!(budget.verify(&vk).is_ok());
    }

    #[test]
    fn test_budget_insufficient_funds() {
        let id = test_identity();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &id);

        let result = budget.spend("llm.query", 2000, &id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BudgetError::InsufficientBudget { .. }
        ));
    }

    #[test]
    fn test_budget_child_derivation() {
        let id = test_identity();
        let vk = id.verifying_key().to_vec();
        let parent = CapabilityBudget::new("parent".into(), sample_allocations(), &id);
        let child = parent.derive_child("child".into(), 0.5, &id).unwrap();

        assert_eq!(child.allocations["llm.query"], 500); // 50% of 1000
        assert_eq!(child.allocations["fs.write"], 250); // 50% of 500
        assert!(child.verify(&vk).is_ok());

        // Child budget always <= parent
        for (k, v) in &child.allocations {
            assert!(*v <= parent.allocations[k]);
        }
    }

    #[test]
    fn test_budget_integrity_verification() {
        let id = test_identity();
        let vk = id.verifying_key().to_vec();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &id);
        assert!(budget.verify(&vk).is_ok());

        // Tamper with allocation
        budget.allocations.insert("llm.query".into(), 9999);
        assert!(budget.verify(&vk).is_err());
    }

    #[test]
    fn test_sealed_token_roundtrip() {
        let id = test_identity();
        let vk = id.verifying_key().to_vec();
        let budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &id);
        let hash = budget.current_hash.clone();

        // Verify the budget can be serialized and deserialized
        let json = serde_json::to_string(&budget).unwrap();
        let deserialized: CapabilityBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.current_hash, hash);
        assert!(deserialized.verify(&vk).is_ok());
    }

    #[test]
    fn test_token_signature_verification() {
        let id = test_identity();
        let vk = id.verifying_key().to_vec();
        let budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &id);
        assert!(budget.verify(&vk).is_ok());

        // Verify with wrong key fails
        let wrong_id = test_identity();
        let wrong_vk = wrong_id.verifying_key().to_vec();
        assert!(budget.verify(&wrong_vk).is_err());
    }
}
