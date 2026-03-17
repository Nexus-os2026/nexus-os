//! ActionExecutor — governed computer actions with kill-switch safety.

use super::{OmniscienceError, OmniscienceResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// Global kill switch: any key press sets this to cancel all agent control.
static KILL_SWITCH_ACTIVE: AtomicBool = AtomicBool::new(false);

// ── Types ───────────────────────────────────────────────────────────────

/// The type of computer action to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Type text at the current cursor position.
    TypeText,
    /// Click at a screen coordinate.
    Click,
    /// Press a keyboard key or shortcut.
    KeyPress,
    /// Navigate to a URL or file path.
    Navigate,
    /// Scroll in a direction.
    Scroll,
}

/// Lifecycle status of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    /// Waiting for approval or execution.
    Pending,
    /// Approved by governance / HITL.
    Approved,
    /// Currently being executed.
    Executing,
    /// Successfully completed.
    Complete,
    /// Cancelled by user or kill switch.
    Cancelled,
    /// Execution failed.
    Failed,
}

/// A governed computer action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerAction {
    /// Unique identifier.
    pub id: Uuid,
    /// The type of action.
    pub action_type: ActionType,
    /// Target of the action (e.g. element ID, coordinates, URL).
    pub target: String,
    /// Action-specific parameters.
    pub parameters: Value,
    /// Whether this action requires explicit HITL approval.
    pub requires_approval: bool,
    /// Current lifecycle status.
    pub status: ActionStatus,
}

// ── ActionExecutor ──────────────────────────────────────────────────────

/// Executes governed computer actions with kill-switch safety.
///
/// All actions pass through governance checks. A global kill switch
/// (triggered by any key press) immediately cancels all pending and
/// executing actions to ensure user safety.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionExecutor {
    /// All tracked actions.
    actions: HashMap<Uuid, ComputerAction>,
    /// Whether the executor is enabled.
    enabled: bool,
}

impl ActionExecutor {
    /// Create a new `ActionExecutor`.
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            enabled: true,
        }
    }

    /// Check if the kill switch is active.
    pub fn is_kill_switch_active() -> bool {
        KILL_SWITCH_ACTIVE.load(Ordering::SeqCst)
    }

    /// Activate the kill switch — cancels all agent control.
    pub fn activate_kill_switch() {
        KILL_SWITCH_ACTIVE.store(true, Ordering::SeqCst);
    }

    /// Reset the kill switch (re-enable agent control).
    pub fn reset_kill_switch() {
        KILL_SWITCH_ACTIVE.store(false, Ordering::SeqCst);
    }

    /// Enable or disable the executor.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether the executor is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Queue a new action for execution.
    ///
    /// The action starts in `Pending` status. If `requires_approval` is true,
    /// it must be explicitly approved before execution.
    pub fn queue_action(
        &mut self,
        action_type: ActionType,
        target: String,
        parameters: Value,
        requires_approval: bool,
    ) -> OmniscienceResult<Uuid> {
        if Self::is_kill_switch_active() {
            return Err(OmniscienceError::KillSwitchActivated);
        }
        if !self.enabled {
            return Err(OmniscienceError::ActionDenied {
                reason: "executor is disabled".into(),
            });
        }

        let action = ComputerAction {
            id: Uuid::new_v4(),
            action_type,
            target,
            parameters,
            requires_approval,
            status: ActionStatus::Pending,
        };

        let id = action.id;
        self.actions.insert(id, action);
        Ok(id)
    }

    /// Approve a pending action (HITL gate).
    pub fn approve_action(&mut self, id: Uuid) -> OmniscienceResult<&ComputerAction> {
        if Self::is_kill_switch_active() {
            return Err(OmniscienceError::KillSwitchActivated);
        }
        let action = self
            .actions
            .get_mut(&id)
            .ok_or(OmniscienceError::ActionNotFound { id: id.to_string() })?;
        if action.status != ActionStatus::Pending {
            return Err(OmniscienceError::ActionDenied {
                reason: format!("action is {:?}, not Pending", action.status),
            });
        }
        action.status = ActionStatus::Approved;
        Ok(action)
    }

    /// Execute an action.
    ///
    /// The action must be `Approved` (if it requires approval) or `Pending`
    /// (if it does not). In production this would dispatch to platform-specific
    /// input simulation; here it transitions the action to `Complete`.
    pub fn execute_action(&mut self, id: Uuid) -> OmniscienceResult<&ComputerAction> {
        if Self::is_kill_switch_active() {
            // Cancel all pending/executing actions when kill switch is active
            self.cancel_all_active();
            return Err(OmniscienceError::KillSwitchActivated);
        }

        let action = self
            .actions
            .get_mut(&id)
            .ok_or(OmniscienceError::ActionNotFound { id: id.to_string() })?;

        match action.status {
            ActionStatus::Pending if !action.requires_approval => {}
            ActionStatus::Approved => {}
            ActionStatus::Pending => {
                return Err(OmniscienceError::ActionDenied {
                    reason: "action requires approval before execution".into(),
                });
            }
            other => {
                return Err(OmniscienceError::ActionDenied {
                    reason: format!("action is {:?}, cannot execute", other),
                });
            }
        }

        action.status = ActionStatus::Executing;

        // In production: dispatch to platform input simulation here.
        // For now, immediately mark as complete.
        action.status = ActionStatus::Complete;

        Ok(action)
    }

    /// Cancel a specific action.
    pub fn cancel_action(&mut self, id: Uuid) -> OmniscienceResult<&ComputerAction> {
        let action = self
            .actions
            .get_mut(&id)
            .ok_or(OmniscienceError::ActionNotFound { id: id.to_string() })?;

        match action.status {
            ActionStatus::Complete | ActionStatus::Failed | ActionStatus::Cancelled => {
                return Err(OmniscienceError::ActionDenied {
                    reason: format!("action is {:?}, cannot cancel", action.status),
                });
            }
            _ => {
                action.status = ActionStatus::Cancelled;
            }
        }

        Ok(action)
    }

    /// Cancel all active (pending, approved, executing) actions.
    pub fn cancel_all_active(&mut self) {
        for action in self.actions.values_mut() {
            if matches!(
                action.status,
                ActionStatus::Pending | ActionStatus::Approved | ActionStatus::Executing
            ) {
                action.status = ActionStatus::Cancelled;
            }
        }
    }

    /// Get an action by ID.
    pub fn get_action(&self, id: &Uuid) -> Option<&ComputerAction> {
        self.actions.get(id)
    }

    /// Return all actions with a given status.
    pub fn actions_by_status(&self, status: ActionStatus) -> Vec<&ComputerAction> {
        self.actions
            .values()
            .filter(|a| a.status == status)
            .collect()
    }

    /// Return the total number of tracked actions.
    pub fn total_count(&self) -> usize {
        self.actions.len()
    }

    /// Clear all actions.
    pub fn clear(&mut self) {
        self.actions.clear();
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Reset kill switch before each test to avoid cross-test interference
    fn setup() {
        ActionExecutor::reset_kill_switch();
    }

    #[test]
    fn new_executor() {
        setup();
        let exec = ActionExecutor::new();
        assert!(exec.is_enabled());
        assert_eq!(exec.total_count(), 0);
    }

    #[test]
    fn queue_and_execute_no_approval() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(
                ActionType::TypeText,
                "input-field".into(),
                json!({"text": "hello"}),
                false,
            )
            .unwrap();

        assert_eq!(exec.get_action(&id).unwrap().status, ActionStatus::Pending);

        let action = exec.execute_action(id).unwrap();
        assert_eq!(action.status, ActionStatus::Complete);
    }

    #[test]
    fn queue_and_execute_with_approval() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(
                ActionType::Click,
                "button".into(),
                json!({"x": 100, "y": 200}),
                true,
            )
            .unwrap();

        // Cannot execute without approval
        let err = exec.execute_action(id);
        assert!(err.is_err());

        // Approve then execute
        exec.approve_action(id).unwrap();
        assert_eq!(exec.get_action(&id).unwrap().status, ActionStatus::Approved);

        let action = exec.execute_action(id).unwrap();
        assert_eq!(action.status, ActionStatus::Complete);
    }

    #[test]
    fn cancel_action() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(ActionType::KeyPress, "enter".into(), json!({}), false)
            .unwrap();

        let action = exec.cancel_action(id).unwrap();
        assert_eq!(action.status, ActionStatus::Cancelled);

        // Cannot cancel again
        assert!(exec.cancel_action(id).is_err());
    }

    #[test]
    fn kill_switch_blocks_queue() {
        setup();
        let mut exec = ActionExecutor::new();
        ActionExecutor::activate_kill_switch();
        let result = exec.queue_action(ActionType::TypeText, "x".into(), json!({}), false);
        assert!(result.is_err());
        setup(); // reset for other tests
    }

    #[test]
    fn kill_switch_blocks_execution() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(ActionType::Navigate, "url".into(), json!({}), false)
            .unwrap();

        ActionExecutor::activate_kill_switch();
        let result = exec.execute_action(id);
        assert!(result.is_err());

        // Action should be cancelled
        assert_eq!(
            exec.get_action(&id).unwrap().status,
            ActionStatus::Cancelled
        );
        setup();
    }

    #[test]
    fn cancel_all_active() {
        setup();
        let mut exec = ActionExecutor::new();
        let id1 = exec
            .queue_action(ActionType::TypeText, "a".into(), json!({}), false)
            .unwrap();
        let id2 = exec
            .queue_action(ActionType::Click, "b".into(), json!({}), false)
            .unwrap();

        exec.cancel_all_active();
        assert_eq!(
            exec.get_action(&id1).unwrap().status,
            ActionStatus::Cancelled
        );
        assert_eq!(
            exec.get_action(&id2).unwrap().status,
            ActionStatus::Cancelled
        );
    }

    #[test]
    fn disabled_executor_rejects() {
        setup();
        let mut exec = ActionExecutor::new();
        exec.set_enabled(false);
        let result = exec.queue_action(ActionType::Scroll, "page".into(), json!({}), false);
        assert!(result.is_err());
    }

    #[test]
    fn actions_by_status() {
        setup();
        let mut exec = ActionExecutor::new();
        exec.queue_action(ActionType::TypeText, "a".into(), json!({}), false)
            .unwrap();
        let id2 = exec
            .queue_action(ActionType::Click, "b".into(), json!({}), false)
            .unwrap();
        exec.execute_action(id2).unwrap();

        let pending = exec.actions_by_status(ActionStatus::Pending);
        assert_eq!(pending.len(), 1);
        let complete = exec.actions_by_status(ActionStatus::Complete);
        assert_eq!(complete.len(), 1);
    }

    #[test]
    fn action_not_found() {
        setup();
        let mut exec = ActionExecutor::new();
        assert!(exec.execute_action(Uuid::new_v4()).is_err());
        assert!(exec.cancel_action(Uuid::new_v4()).is_err());
        assert!(exec.approve_action(Uuid::new_v4()).is_err());
    }

    #[test]
    fn cannot_approve_non_pending() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(ActionType::TypeText, "a".into(), json!({}), false)
            .unwrap();
        exec.execute_action(id).unwrap();
        assert!(exec.approve_action(id).is_err());
    }

    #[test]
    fn cannot_execute_completed_action() {
        setup();
        let mut exec = ActionExecutor::new();
        let id = exec
            .queue_action(ActionType::TypeText, "a".into(), json!({}), false)
            .unwrap();
        exec.execute_action(id).unwrap();
        assert!(exec.execute_action(id).is_err());
    }

    #[test]
    fn clear_actions() {
        setup();
        let mut exec = ActionExecutor::new();
        exec.queue_action(ActionType::TypeText, "a".into(), json!({}), false)
            .unwrap();
        exec.clear();
        assert_eq!(exec.total_count(), 0);
    }

    #[test]
    fn action_serialization() {
        let action = ComputerAction {
            id: Uuid::new_v4(),
            action_type: ActionType::Click,
            target: "button".into(),
            parameters: json!({"x": 100}),
            requires_approval: true,
            status: ActionStatus::Pending,
        };
        let json_str = serde_json::to_string(&action).unwrap();
        let deser: ComputerAction = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deser.action_type, ActionType::Click);
        assert!(deser.requires_approval);
    }

    #[test]
    fn action_type_variants_serialize() {
        for at in [
            ActionType::TypeText,
            ActionType::Click,
            ActionType::KeyPress,
            ActionType::Navigate,
            ActionType::Scroll,
        ] {
            let json_str = serde_json::to_string(&at).unwrap();
            let deser: ActionType = serde_json::from_str(&json_str).unwrap();
            assert_eq!(deser, at);
        }
    }

    #[test]
    fn action_status_variants_serialize() {
        for status in [
            ActionStatus::Pending,
            ActionStatus::Approved,
            ActionStatus::Executing,
            ActionStatus::Complete,
            ActionStatus::Cancelled,
            ActionStatus::Failed,
        ] {
            let json_str = serde_json::to_string(&status).unwrap();
            let deser: ActionStatus = serde_json::from_str(&json_str).unwrap();
            assert_eq!(deser, status);
        }
    }
}
