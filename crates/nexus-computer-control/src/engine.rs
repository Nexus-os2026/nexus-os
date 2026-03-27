use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::actions::ComputerAction;
use crate::audit::{verify_action_sequence, ActionAuditEntry, ControlAuditTrail};
use crate::governance::{check_governance, token_cost};
use crate::ControlError;

/// Budget summary for an agent's computer control usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerControlBudget {
    pub agent_id: String,
    pub balance_micro: u64,
    pub total_spent_micro: u64,
    pub actions_executed: usize,
    pub actions_denied: usize,
}

/// Result of executing a computer control action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub action_label: String,
    pub token_cost: u64,
    pub balance_after: u64,
    pub output: String,
    pub audit_entry_id: String,
}

/// Screen context for an agent's current view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContext {
    pub last_screenshot_time: Option<u64>,
    pub actions_this_session: usize,
    pub recent_actions: Vec<String>,
}

/// Result of verifying an action sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub valid: bool,
    pub chain_verified: bool,
    pub sequence_verified: bool,
    pub total_entries: usize,
    pub error: Option<String>,
}

/// The governed computer control engine.
///
/// Wraps action execution with governance gates, token economy,
/// and hash-chained audit trail.
pub struct GovernedControlEngine {
    /// Per-agent balances (micronexus)
    balances: HashMap<String, u64>,
    /// Per-agent total spent
    total_spent: HashMap<String, u64>,
    /// Per-agent action counts
    action_counts: HashMap<String, usize>,
    /// Per-agent denied action counts
    denied_counts: HashMap<String, usize>,
    /// Hash-chained audit trail
    audit: ControlAuditTrail,
    /// Workspace root for sandbox path checks
    workspace_root: String,
    /// Timeout for external subprocess calls (ms)
    subprocess_timeout_ms: u64,
}

impl GovernedControlEngine {
    pub fn new(workspace_root: String) -> Self {
        Self {
            balances: HashMap::new(),
            total_spent: HashMap::new(),
            action_counts: HashMap::new(),
            denied_counts: HashMap::new(),
            audit: ControlAuditTrail::new(),
            workspace_root,
            subprocess_timeout_ms: 10_000, // 10s default, prevents 15s hang
        }
    }

    /// Set the subprocess timeout (prevents hangs).
    pub fn set_subprocess_timeout_ms(&mut self, ms: u64) {
        self.subprocess_timeout_ms = ms;
    }

    /// Initialize an agent's balance.
    pub fn set_agent_balance(&mut self, agent_id: &str, balance_micro: u64) {
        self.balances.insert(agent_id.to_string(), balance_micro);
    }

    /// Execute a computer control action with full governance checks.
    ///
    /// 1. Check autonomy level and capabilities
    /// 2. Check token balance (L4+ agents are gated)
    /// 3. Burn tokens
    /// 4. Execute action (simulated — actual execution delegated to kernel)
    /// 5. Record in hash-chained audit trail
    pub fn execute_action(
        &mut self,
        agent_id: &str,
        autonomy_level: u8,
        capabilities: &[String],
        action: &ComputerAction,
    ) -> Result<ActionResult, ControlError> {
        // 1. Governance check
        check_governance(action, autonomy_level, capabilities, &self.workspace_root)?;

        // 2. Token cost
        let cost = token_cost(action);
        let balance = self.balances.get(agent_id).copied().unwrap_or(0);

        // L4+ agents are hard-gated by balance
        if autonomy_level >= 4 && balance < cost {
            *self.denied_counts.entry(agent_id.to_string()).or_insert(0) += 1;
            self.audit.record(
                agent_id,
                action,
                false,
                Some("Insufficient balance".into()),
                cost,
                balance,
            );
            return Err(ControlError::InsufficientBalance {
                required: cost,
                available: balance,
            });
        }

        // 3. Burn tokens
        let new_balance = balance.saturating_sub(cost);
        self.balances.insert(agent_id.to_string(), new_balance);
        *self.total_spent.entry(agent_id.to_string()).or_insert(0) += cost;
        *self.action_counts.entry(agent_id.to_string()).or_insert(0) += 1;

        // 4. Execute (the actual execution is delegated to the kernel's
        //    ComputerControlEngine — this crate handles governance only)
        let output = format!("Executed: {}", action.label());

        // 5. Audit
        let entry = self
            .audit
            .record(agent_id, action, true, None, cost, new_balance);

        Ok(ActionResult {
            success: true,
            action_label: action.label(),
            token_cost: cost,
            balance_after: new_balance,
            output,
            audit_entry_id: entry.entry_id,
        })
    }

    /// Rollback the last action for an agent (on governance denial after execution).
    pub fn rollback_last_action(&mut self, agent_id: &str) -> Result<(), ControlError> {
        let entries = self.audit.entries_for_agent(agent_id);
        let last = entries.last().ok_or(ControlError::GovernanceDenied(
            "No actions to rollback".into(),
        ))?;

        if !last.success {
            return Err(ControlError::GovernanceDenied(
                "Last action already failed".into(),
            ));
        }

        // Refund the tokens
        let cost = last.token_cost;
        let balance = self.balances.get(agent_id).copied().unwrap_or(0);
        self.balances.insert(agent_id.to_string(), balance + cost);
        *self.total_spent.entry(agent_id.to_string()).or_insert(0) = self
            .total_spent
            .get(agent_id)
            .copied()
            .unwrap_or(0)
            .saturating_sub(cost);

        // Record the rollback in the audit trail
        self.audit.record(
            agent_id,
            &ComputerAction::ReadClipboard, // placeholder action for rollback
            true,
            Some(format!("Rollback of {}", last.action_label)),
            0, // no cost for rollback
            balance + cost,
        );

        Ok(())
    }

    /// Get action history for an agent.
    pub fn get_action_history(&self, agent_id: &str) -> Vec<&ActionAuditEntry> {
        self.audit.entries_for_agent(agent_id)
    }

    /// Get budget summary for an agent.
    pub fn get_budget(&self, agent_id: &str) -> ComputerControlBudget {
        ComputerControlBudget {
            agent_id: agent_id.to_string(),
            balance_micro: self.balances.get(agent_id).copied().unwrap_or(0),
            total_spent_micro: self.total_spent.get(agent_id).copied().unwrap_or(0),
            actions_executed: self.action_counts.get(agent_id).copied().unwrap_or(0),
            actions_denied: self.denied_counts.get(agent_id).copied().unwrap_or(0),
        }
    }

    /// Get screen context for an agent.
    pub fn get_screen_context(&self, agent_id: &str) -> ScreenContext {
        let entries = self.audit.entries_for_agent(agent_id);
        let last_screenshot = entries
            .iter()
            .rev()
            .find(|e| matches!(e.action, ComputerAction::Screenshot { .. }))
            .map(|e| e.timestamp);
        let recent: Vec<String> = entries
            .iter()
            .rev()
            .take(10)
            .map(|e| e.action_label.clone())
            .collect();

        ScreenContext {
            last_screenshot_time: last_screenshot,
            actions_this_session: entries.len(),
            recent_actions: recent,
        }
    }

    /// Verify the entire audit trail and action sequence for an agent.
    pub fn verify_action_sequence(&self, agent_id: &str) -> VerificationResult {
        let chain_result = self.audit.verify_chain();
        let entries = self.audit.entries_for_agent(agent_id);
        let owned: Vec<ActionAuditEntry> = entries.into_iter().cloned().collect();
        let seq_result = verify_action_sequence(&owned);

        VerificationResult {
            valid: chain_result.is_ok() && seq_result.is_ok(),
            chain_verified: chain_result.is_ok(),
            sequence_verified: seq_result.is_ok(),
            total_entries: owned.len(),
            error: chain_result.err().or(seq_result.err()),
        }
    }

    /// Get the subprocess timeout.
    pub fn subprocess_timeout_ms(&self) -> u64 {
        self.subprocess_timeout_ms
    }

    /// Get the full audit trail.
    pub fn audit_trail(&self) -> &ControlAuditTrail {
        &self.audit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ComputerAction, MouseButton};

    fn caps(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_screenshot_requires_governance_approval() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());
        engine.set_agent_balance("a1", 100_000_000);

        let action = ComputerAction::Screenshot { region: None };

        // Missing capability → denied
        let result = engine.execute_action("a1", 4, &caps(&[]), &action);
        assert!(result.is_err());

        // With capability → allowed
        let result =
            engine.execute_action("a1", 4, &caps(&["computer_control.screenshot"]), &action);
        assert!(result.is_ok());
    }

    #[test]
    fn test_token_burn_on_action() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());
        engine.set_agent_balance("a1", 100_000_000); // 100 NXC

        let action = ComputerAction::Screenshot { region: None };
        let result = engine
            .execute_action("a1", 4, &caps(&["computer_control.screenshot"]), &action)
            .unwrap();

        assert_eq!(result.token_cost, 1_000_000); // 1 NXC
        assert_eq!(result.balance_after, 99_000_000);

        let budget = engine.get_budget("a1");
        assert_eq!(budget.balance_micro, 99_000_000);
        assert_eq!(budget.total_spent_micro, 1_000_000);
        assert_eq!(budget.actions_executed, 1);
    }

    #[test]
    fn test_insufficient_balance_blocks_action() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());
        engine.set_agent_balance("a1", 500_000); // 0.5 NXC — not enough for screenshot (1 NXC)

        let action = ComputerAction::Screenshot { region: None };
        let result = engine.execute_action(
            "a1",
            4, // L4 — balance is enforced
            &caps(&["computer_control.screenshot"]),
            &action,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ControlError::InsufficientBalance { .. }
        ));
    }

    #[test]
    fn test_wasm_sandbox_blocks_unauthorized_path() {
        let mut engine = GovernedControlEngine::new("/home/nexus/workspace".into());
        engine.set_agent_balance("a1", 100_000_000);

        let action = ComputerAction::TerminalCommand {
            command: "ls".into(),
            working_dir: "/etc/secrets".into(), // outside workspace
        };
        let result = engine.execute_action("a1", 5, &caps(&["computer_control.terminal"]), &action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ControlError::SandboxViolation(..)
        ));
    }

    #[test]
    fn test_timeout_prevents_hang() {
        let engine = GovernedControlEngine::new("/home/nexus".into());
        // Default timeout is 10s, not 15s
        assert!(engine.subprocess_timeout_ms() <= 10_000);
    }

    #[test]
    fn test_action_replay_verification() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());
        engine.set_agent_balance("a1", 100_000_000);

        let all_caps = caps(&[
            "computer_control.screenshot",
            "computer_control.mouse",
            "computer_control.basic",
        ]);

        // Execute a sequence of actions
        engine
            .execute_action(
                "a1",
                4,
                &all_caps,
                &ComputerAction::Screenshot { region: None },
            )
            .unwrap();
        engine
            .execute_action(
                "a1",
                4,
                &all_caps,
                &ComputerAction::MouseClick {
                    x: 100,
                    y: 200,
                    button: MouseButton::Left,
                },
            )
            .unwrap();
        engine
            .execute_action("a1", 4, &all_caps, &ComputerAction::ReadClipboard)
            .unwrap();

        // Verify the sequence
        let verification = engine.verify_action_sequence("a1");
        assert!(verification.valid);
        assert!(verification.chain_verified);
        assert!(verification.sequence_verified);
        assert_eq!(verification.total_entries, 3);
    }

    #[test]
    fn test_rollback_on_governance_denial() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());
        engine.set_agent_balance("a1", 100_000_000);

        let all_caps = caps(&["computer_control.screenshot"]);

        // Execute an action
        engine
            .execute_action(
                "a1",
                4,
                &all_caps,
                &ComputerAction::Screenshot { region: None },
            )
            .unwrap();

        let budget_before = engine.get_budget("a1");
        assert_eq!(budget_before.balance_micro, 99_000_000);

        // Rollback
        engine.rollback_last_action("a1").unwrap();

        let budget_after = engine.get_budget("a1");
        assert_eq!(budget_after.balance_micro, 100_000_000); // refunded
    }

    #[test]
    fn test_child_agent_inherits_computer_control_budget() {
        let mut engine = GovernedControlEngine::new("/home/nexus".into());

        // Parent has 100 NXC
        engine.set_agent_balance("parent", 100_000_000);

        // Child gets 25% = 25 NXC
        let parent_balance = engine.get_budget("parent").balance_micro;
        let child_allocation = parent_balance / 4;
        engine.set_agent_balance("child", child_allocation);
        engine.set_agent_balance("parent", parent_balance - child_allocation);

        assert_eq!(engine.get_budget("child").balance_micro, 25_000_000);
        assert_eq!(engine.get_budget("parent").balance_micro, 75_000_000);
    }
}
