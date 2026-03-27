use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::actions::ComputerAction;

/// Hash-chained audit entry for a computer control action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionAuditEntry {
    pub entry_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub action: ComputerAction,
    pub action_label: String,
    pub success: bool,
    pub error: Option<String>,
    pub token_cost: u64,
    pub balance_after: u64,
    pub previous_hash: String,
    pub entry_hash: String,
}

/// Hash-chained audit trail for computer control actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAuditTrail {
    entries: Vec<ActionAuditEntry>,
    latest_hash: String,
}

impl ControlAuditTrail {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest_hash: "genesis-computer-control".to_string(),
        }
    }

    /// Record an action in the audit trail with hash chaining.
    pub fn record(
        &mut self,
        agent_id: &str,
        action: &ComputerAction,
        success: bool,
        error: Option<String>,
        token_cost: u64,
        balance_after: u64,
    ) -> ActionAuditEntry {
        let mut hasher = Sha256::new();
        hasher.update(self.latest_hash.as_bytes());
        hasher.update(agent_id.as_bytes());
        hasher.update(action.label().as_bytes());
        hasher.update(token_cost.to_le_bytes());
        hasher.update(if success {
            b"ok".as_slice()
        } else {
            b"err".as_slice()
        });
        let entry_hash = format!("{:x}", hasher.finalize());

        let entry = ActionAuditEntry {
            entry_id: uuid::Uuid::new_v4().to_string(),
            timestamp: epoch_secs(),
            agent_id: agent_id.to_string(),
            action: action.clone(),
            action_label: action.label(),
            success,
            error,
            token_cost,
            balance_after,
            previous_hash: self.latest_hash.clone(),
            entry_hash: entry_hash.clone(),
        };

        self.latest_hash = entry_hash;
        self.entries.push(entry.clone());
        entry
    }

    /// Verify the entire chain integrity.
    pub fn verify_chain(&self) -> Result<(), String> {
        let mut expected_prev = "genesis-computer-control".to_string();

        for entry in &self.entries {
            if entry.previous_hash != expected_prev {
                return Err(format!("Chain broken at {}", entry.entry_id));
            }
            let mut hasher = Sha256::new();
            hasher.update(entry.previous_hash.as_bytes());
            hasher.update(entry.agent_id.as_bytes());
            hasher.update(entry.action_label.as_bytes());
            hasher.update(entry.token_cost.to_le_bytes());
            hasher.update(if entry.success {
                b"ok".as_slice()
            } else {
                b"err".as_slice()
            });
            let computed = format!("{:x}", hasher.finalize());
            if computed != entry.entry_hash {
                return Err(format!("Hash mismatch at {}", entry.entry_id));
            }
            expected_prev = entry.entry_hash.clone();
        }
        Ok(())
    }

    /// Get entries for a specific agent.
    pub fn entries_for_agent(&self, agent_id: &str) -> Vec<&ActionAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.agent_id == agent_id)
            .collect()
    }

    pub fn entries(&self) -> &[ActionAuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ControlAuditTrail {
    fn default() -> Self {
        Self::new()
    }
}

/// Verify an action sequence is consistent — every entry's balance_after
/// equals the previous balance minus the token cost.
pub fn verify_action_sequence(entries: &[ActionAuditEntry]) -> Result<(), String> {
    for window in entries.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        if prev.agent_id == curr.agent_id && curr.success {
            let expected_balance = prev.balance_after.saturating_sub(curr.token_cost);
            if curr.balance_after != expected_balance {
                return Err(format!(
                    "Balance inconsistency at {}: expected {expected_balance}, got {}",
                    curr.entry_id, curr.balance_after,
                ));
            }
        }
    }
    Ok(())
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
    use crate::actions::ComputerAction;

    #[test]
    fn test_action_hash_chained_in_audit() {
        let mut trail = ControlAuditTrail::new();

        trail.record(
            "agent-1",
            &ComputerAction::Screenshot { region: None },
            true,
            None,
            1_000_000,
            99_000_000,
        );
        trail.record(
            "agent-1",
            &ComputerAction::MouseMove { x: 100, y: 200 },
            true,
            None,
            1_000_000,
            98_000_000,
        );
        trail.record(
            "agent-1",
            &ComputerAction::ReadClipboard,
            true,
            None,
            1_000_000,
            97_000_000,
        );

        assert_eq!(trail.len(), 3);
        assert!(trail.verify_chain().is_ok());
    }

    #[test]
    fn test_action_sequence_integrity() {
        let mut trail = ControlAuditTrail::new();

        trail.record(
            "a1",
            &ComputerAction::Screenshot { region: None },
            true,
            None,
            1_000_000,
            99_000_000,
        );
        trail.record(
            "a1",
            &ComputerAction::ReadClipboard,
            true,
            None,
            1_000_000,
            98_000_000,
        );

        let entries = trail.entries_for_agent("a1");
        let owned: Vec<ActionAuditEntry> = entries.into_iter().cloned().collect();
        assert!(verify_action_sequence(&owned).is_ok());
    }

    #[test]
    fn test_tamper_detection() {
        let mut trail = ControlAuditTrail::new();
        trail.record(
            "a1",
            &ComputerAction::Screenshot { region: None },
            true,
            None,
            1_000_000,
            99_000_000,
        );
        trail.record(
            "a1",
            &ComputerAction::ReadClipboard,
            true,
            None,
            1_000_000,
            98_000_000,
        );

        // Tamper with the first entry's cost
        trail.entries.get_mut(0).unwrap().token_cost = 999;

        assert!(trail.verify_chain().is_err());
    }
}
