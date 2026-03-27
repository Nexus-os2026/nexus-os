//! Cryptographic token construction and verification.
//!
//! `CapabilityBudget` — Ed25519-signed, monotonically decreasing, child-derivable.

use std::collections::HashMap;

use ed25519_dalek::{Signer, Verifier};
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
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        let initial_hash = Self::compute_hash(&agent_id, &allocations, 0);
        let signature = signing_key.sign(initial_hash.as_bytes());

        Self {
            agent_id,
            allocations,
            initial_hash: initial_hash.clone(),
            current_hash: initial_hash,
            authority_signature: signature.to_bytes().to_vec(),
            version: 0,
        }
    }

    /// Spend tokens from a capability category.
    pub fn spend(
        &mut self,
        capability: &str,
        amount: u64,
        signing_key: &ed25519_dalek::SigningKey,
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
        let signature = signing_key.sign(self.current_hash.as_bytes());
        self.authority_signature = signature.to_bytes().to_vec();
        Ok(())
    }

    /// Derive a child budget. Child always gets <= parent's remaining budget.
    pub fn derive_child(
        &self,
        child_agent_id: String,
        fraction: f64,
        signing_key: &ed25519_dalek::SigningKey,
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
            signing_key,
        ))
    }

    /// Verify budget integrity against the authority signature.
    pub fn verify(&self, verifying_key: &ed25519_dalek::VerifyingKey) -> Result<(), BudgetError> {
        let expected_hash = Self::compute_hash(&self.agent_id, &self.allocations, self.version);
        if expected_hash != self.current_hash {
            return Err(BudgetError::IntegrityViolation("Hash mismatch".to_string()));
        }

        let sig_bytes: [u8; 64] = self
            .authority_signature
            .as_slice()
            .try_into()
            .map_err(|_| BudgetError::IntegrityViolation("Invalid signature bytes".into()))?;
        let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);

        verifying_key
            .verify(self.current_hash.as_bytes(), &signature)
            .map_err(|_| BudgetError::IntegrityViolation("Signature verification failed".into()))?;

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

    fn test_keypair() -> (ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey) {
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn sample_allocations() -> HashMap<String, u64> {
        let mut m = HashMap::new();
        m.insert("llm.query".into(), 1000);
        m.insert("fs.write".into(), 500);
        m
    }

    #[test]
    fn test_budget_creation_and_spending() {
        let (sk, vk) = test_keypair();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &sk);

        assert_eq!(budget.allocations["llm.query"], 1000);
        assert!(budget.verify(&vk).is_ok());

        budget.spend("llm.query", 100, &sk).unwrap();
        assert_eq!(budget.allocations["llm.query"], 900);
        assert_eq!(budget.version, 1);
        assert!(budget.verify(&vk).is_ok());
    }

    #[test]
    fn test_budget_insufficient_funds() {
        let (sk, _vk) = test_keypair();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &sk);

        let result = budget.spend("llm.query", 2000, &sk);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BudgetError::InsufficientBudget { .. }
        ));
    }

    #[test]
    fn test_budget_child_derivation() {
        let (sk, vk) = test_keypair();
        let parent = CapabilityBudget::new("parent".into(), sample_allocations(), &sk);
        let child = parent.derive_child("child".into(), 0.5, &sk).unwrap();

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
        let (sk, vk) = test_keypair();
        let mut budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &sk);
        assert!(budget.verify(&vk).is_ok());

        // Tamper with allocation
        budget.allocations.insert("llm.query".into(), 9999);
        assert!(budget.verify(&vk).is_err());
    }

    #[test]
    fn test_sealed_token_roundtrip() {
        let (sk, vk) = test_keypair();
        let budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &sk);
        let hash = budget.current_hash.clone();

        // Verify the budget can be serialized and deserialized
        let json = serde_json::to_string(&budget).unwrap();
        let deserialized: CapabilityBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.current_hash, hash);
        assert!(deserialized.verify(&vk).is_ok());
    }

    #[test]
    fn test_token_signature_verification() {
        let (sk, vk) = test_keypair();
        let budget = CapabilityBudget::new("agent-1".into(), sample_allocations(), &sk);
        assert!(budget.verify(&vk).is_ok());

        // Verify with wrong key fails
        let (_, wrong_vk) = test_keypair();
        assert!(budget.verify(&wrong_vk).is_err());
    }
}
