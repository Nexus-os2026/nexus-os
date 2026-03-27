//! Hash-chained decision audit log. Every decision links to the previous,
//! creating a tamper-evident chain. Genesis block starts with "genesis".

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use nexus_governance_oracle::{CapabilityRequest, GovernanceDecision};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAuditLog {
    entries: Vec<AuditEntry>,
    latest_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub entry_id: String,
    pub agent_id: String,
    pub capability: String,
    pub decision: String,
    pub governance_version: String,
    pub timestamp: u64,
    pub previous_hash: String,
    pub entry_hash: String,
}

impl Default for DecisionAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest_hash: "genesis".to_string(),
        }
    }

    pub fn record(
        &mut self,
        request: &CapabilityRequest,
        decision: &GovernanceDecision,
        governance_version: &str,
    ) {
        let decision_str = match decision {
            GovernanceDecision::Approved { .. } => "approved",
            GovernanceDecision::Denied => "denied",
        };

        let mut hasher = Sha256::new();
        hasher.update(self.latest_hash.as_bytes());
        hasher.update(request.agent_id.as_bytes());
        hasher.update(request.capability.as_bytes());
        hasher.update(decision_str.as_bytes());
        hasher.update(governance_version.as_bytes());
        let entry_hash = format!("{:x}", hasher.finalize());

        let entry = AuditEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            agent_id: request.agent_id.clone(),
            capability: request.capability.clone(),
            decision: decision_str.to_string(),
            governance_version: governance_version.to_string(),
            timestamp: epoch_secs(),
            previous_hash: self.latest_hash.clone(),
            entry_hash: entry_hash.clone(),
        };

        self.latest_hash = entry_hash;
        self.entries.push(entry);
    }

    /// Verify the entire chain integrity.
    pub fn verify_chain(&self) -> Result<(), String> {
        let mut expected_prev = "genesis".to_string();
        for entry in &self.entries {
            if entry.previous_hash != expected_prev {
                return Err(format!(
                    "Chain broken at entry {}: expected prev {expected_prev}, got {}",
                    entry.entry_id, entry.previous_hash
                ));
            }

            let mut hasher = Sha256::new();
            hasher.update(entry.previous_hash.as_bytes());
            hasher.update(entry.agent_id.as_bytes());
            hasher.update(entry.capability.as_bytes());
            hasher.update(entry.decision.as_bytes());
            hasher.update(entry.governance_version.as_bytes());
            let computed = format!("{:x}", hasher.finalize());

            if computed != entry.entry_hash {
                return Err(format!(
                    "Hash mismatch at entry {}: computed {computed}, stored {}",
                    entry.entry_id, entry.entry_hash
                ));
            }
            expected_prev = entry.entry_hash.clone();
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn latest_hash(&self) -> &str {
        &self.latest_hash
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(agent: &str, cap: &str) -> CapabilityRequest {
        CapabilityRequest {
            agent_id: agent.into(),
            capability: cap.into(),
            parameters: serde_json::Value::Null,
            budget_hash: String::new(),
            request_nonce: "n".into(),
        }
    }

    #[test]
    fn test_audit_log_chain_integrity() {
        let mut log = DecisionAuditLog::new();
        log.record(
            &req("a1", "llm.query"),
            &GovernanceDecision::Approved {
                capability_token: "t".into(),
            },
            "v1",
        );
        log.record(&req("a2", "fs.write"), &GovernanceDecision::Denied, "v1");
        log.record(
            &req("a1", "process.exec"),
            &GovernanceDecision::Denied,
            "v1",
        );

        assert_eq!(log.len(), 3);
        assert!(log.verify_chain().is_ok());
    }

    #[test]
    fn test_audit_log_tamper_detection() {
        let mut log = DecisionAuditLog::new();
        log.record(
            &req("a1", "llm.query"),
            &GovernanceDecision::Approved {
                capability_token: "t".into(),
            },
            "v1",
        );
        log.record(&req("a2", "fs.write"), &GovernanceDecision::Denied, "v1");

        // Tamper with a decision
        log.entries.get_mut(0).unwrap().decision = "denied".into();
        assert!(log.verify_chain().is_err());
    }
}
