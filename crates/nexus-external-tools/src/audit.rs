use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::execution::ToolCallResult;

/// Hash-chained audit trail for all external tool calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAuditTrail {
    entries: Vec<ToolAuditEntry>,
    latest_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAuditEntry {
    pub entry_id: String,
    pub agent_id: String,
    pub tool_id: String,
    pub action: String,
    pub success: bool,
    pub has_side_effects: bool,
    pub cost: u64,
    pub timestamp: u64,
    pub previous_hash: String,
    pub entry_hash: String,
}

impl ToolAuditTrail {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest_hash: "genesis-tools".into(),
        }
    }

    pub fn record(&mut self, result: &ToolCallResult, action: &str) {
        let mut hasher = Sha256::new();
        hasher.update(self.latest_hash.as_bytes());
        hasher.update(result.agent_id.as_bytes());
        hasher.update(result.tool_id.as_bytes());
        hasher.update(action.as_bytes());
        hasher.update(result.cost.to_le_bytes());
        hasher.update(if result.success {
            b"ok".as_slice()
        } else {
            b"err".as_slice()
        });
        let hash = format!("{:x}", hasher.finalize());

        self.entries.push(ToolAuditEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            agent_id: result.agent_id.clone(),
            tool_id: result.tool_id.clone(),
            action: action.into(),
            success: result.success,
            has_side_effects: result.has_side_effects,
            cost: result.cost,
            timestamp: result.timestamp,
            previous_hash: self.latest_hash.clone(),
            entry_hash: hash.clone(),
        });
        self.latest_hash = hash;
    }

    pub fn verify_chain(&self) -> Result<(), String> {
        let mut expected = "genesis-tools".to_string();
        for entry in &self.entries {
            if entry.previous_hash != expected {
                return Err(format!("Chain broken at {}", entry.entry_id));
            }
            let mut hasher = Sha256::new();
            hasher.update(entry.previous_hash.as_bytes());
            hasher.update(entry.agent_id.as_bytes());
            hasher.update(entry.tool_id.as_bytes());
            hasher.update(entry.action.as_bytes());
            hasher.update(entry.cost.to_le_bytes());
            hasher.update(if entry.success {
                b"ok".as_slice()
            } else {
                b"err".as_slice()
            });
            let computed = format!("{:x}", hasher.finalize());
            if computed != entry.entry_hash {
                return Err(format!("Hash mismatch at {}", entry.entry_id));
            }
            expected = entry.entry_hash.clone();
        }
        Ok(())
    }

    pub fn entries(&self) -> &[ToolAuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ToolAuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(tool_id: &str, success: bool) -> ToolCallResult {
        ToolCallResult {
            tool_id: tool_id.into(),
            tool_name: tool_id.into(),
            agent_id: "agent-1".into(),
            success,
            status_code: if success { 200 } else { 500 },
            response_body: "{}".into(),
            duration_ms: 100,
            cost: 2_000_000,
            has_side_effects: true,
            timestamp: 1000,
        }
    }

    #[test]
    fn test_audit_chain_integrity() {
        let mut trail = ToolAuditTrail::new();
        trail.record(&make_result("github", true), "create_issue");
        trail.record(&make_result("slack", true), "send_message");
        trail.record(&make_result("web_search", false), "search");
        assert_eq!(trail.len(), 3);
        assert!(trail.verify_chain().is_ok());
    }

    #[test]
    fn test_audit_tamper_detection() {
        let mut trail = ToolAuditTrail::new();
        trail.record(&make_result("github", true), "create_issue");
        trail.record(&make_result("slack", true), "send_message");

        // Tamper with an entry
        trail.entries[0].agent_id = "tampered".into();
        assert!(trail.verify_chain().is_err());
    }
}
