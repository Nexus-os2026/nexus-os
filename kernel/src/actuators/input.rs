use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use crate::computer_control::{
    analyze_stored_screenshot, append_audit_log, capture_and_store_screen,
    emergency_kill_switch_active, execute_input_action, screen_analysis_indicates_sensitive_field,
    text_looks_sensitive, InputAction, MouseButton,
};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct InputControlActuator;

fn input_timestamps() -> &'static Mutex<VecDeque<u64>> {
    static INPUT_TIMESTAMPS: OnceLock<Mutex<VecDeque<u64>>> = OnceLock::new();
    INPUT_TIMESTAMPS.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl InputControlActuator {
    fn ensure_input_capability(
        context: &ActuatorContext,
        required: &str,
    ) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L3 {
            return Err(ActuatorError::CapabilityDenied(
                "input control requires L3+ autonomy".to_string(),
            ));
        }
        if !has_capability(context.capabilities.iter().map(String::as_str), required) {
            return Err(ActuatorError::CapabilityDenied(required.to_string()));
        }
        if !(context.autonomy_level >= AutonomyLevel::L4
            && has_capability(
                context.capabilities.iter().map(String::as_str),
                "input.autonomous",
            ))
        {
            return Err(ActuatorError::HumanApprovalRequired(
                "input control requires HITL approval unless the agent has input.autonomous at L4+"
                    .to_string(),
            ));
        }
        if emergency_kill_switch_active() {
            return Err(ActuatorError::CommandBlocked(
                "input control blocked by emergency kill switch".to_string(),
            ));
        }
        Ok(())
    }

    fn check_rate_limit() -> Result<(), ActuatorError> {
        let mut guard = input_timestamps()
            .lock()
            .map_err(|_| ActuatorError::IoError("input rate limiter lock poisoned".to_string()))?;
        let window_start = now_secs().saturating_sub(60);
        while guard.front().is_some_and(|ts| *ts < window_start) {
            guard.pop_front();
        }
        if guard.len() >= 100 {
            return Err(ActuatorError::CommandBlocked(
                "input rate limit exceeded (100 actions/minute)".to_string(),
            ));
        }
        guard.push_back(now_secs());
        Ok(())
    }

    fn parse_button(button: &str) -> Result<MouseButton, ActuatorError> {
        match button.to_ascii_lowercase().as_str() {
            "left" => Ok(MouseButton::Left),
            "right" => Ok(MouseButton::Right),
            "middle" => Ok(MouseButton::Middle),
            other => Err(ActuatorError::IoError(format!(
                "unsupported mouse button '{other}'"
            ))),
        }
    }

    fn take_input_audit_screenshot(
        context: &ActuatorContext,
        label: &str,
    ) -> Result<std::path::PathBuf, ActuatorError> {
        capture_and_store_screen(&context.working_dir, None, label).map_err(ActuatorError::IoError)
    }

    fn guard_keyboard_input(context: &ActuatorContext, text: &str) -> Result<(), ActuatorError> {
        if text_looks_sensitive(text) {
            return Err(ActuatorError::CommandBlocked(
                "refusing to type text that looks like a password or secret".to_string(),
            ));
        }

        let screenshot = Self::take_input_audit_screenshot(context, "keyboard-guard-before")?;
        if let Ok(analysis) = analyze_stored_screenshot(
            &screenshot,
            "Does this screenshot show a password, secret, or credential input field? Answer briefly.",
            None,
        ) {
            if screen_analysis_indicates_sensitive_field(&analysis) {
                return Err(ActuatorError::CommandBlocked(
                    "refusing to type into a password or sensitive field".to_string(),
                ));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn execute_input_action_for_test(
        context: &ActuatorContext,
        action: InputAction,
    ) -> Result<ActionResult, ActuatorError> {
        Self::execute_input_action_internal(context, action)
    }

    fn execute_input_action_internal(
        context: &ActuatorContext,
        action: InputAction,
    ) -> Result<ActionResult, ActuatorError> {
        Self::check_rate_limit()?;

        let before = Self::take_input_audit_screenshot(context, "input-before")?;
        execute_input_action(&action).map_err(ActuatorError::IoError)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        let after = Self::take_input_audit_screenshot(context, "input-after")?;
        append_audit_log(
            &context.working_dir,
            "input.execute",
            serde_json::json!({
                "action": action.label(),
                "before": before,
                "after": after,
            }),
        )
        .map_err(ActuatorError::IoError)?;

        Ok(ActionResult {
            success: true,
            output: format!("executed {}", action.label()),
            fuel_cost: 5.0,
            side_effects: vec![
                SideEffect::FileCreated { path: before },
                SideEffect::FileCreated { path: after },
            ],
        })
    }
}

impl Actuator for InputControlActuator {
    fn name(&self) -> &str {
        "input_control_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec![
            "input.mouse".to_string(),
            "input.keyboard".to_string(),
            "input.autonomous".to_string(),
        ]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        match action {
            PlannedAction::MouseMove { x, y } => {
                Self::ensure_input_capability(context, "input.mouse")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::MoveMouse { x: *x, y: *y },
                )
            }
            PlannedAction::MouseClick { x, y, button } => {
                Self::ensure_input_capability(context, "input.mouse")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::Click {
                        x: *x,
                        y: *y,
                        button: Self::parse_button(button)?,
                    },
                )
            }
            PlannedAction::MouseDoubleClick { x, y } => {
                Self::ensure_input_capability(context, "input.mouse")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::DoubleClick { x: *x, y: *y },
                )
            }
            PlannedAction::MouseDrag {
                from_x,
                from_y,
                to_x,
                to_y,
            } => {
                Self::ensure_input_capability(context, "input.mouse")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::Drag {
                        from_x: *from_x,
                        from_y: *from_y,
                        to_x: *to_x,
                        to_y: *to_y,
                    },
                )
            }
            PlannedAction::KeyboardType { text } => {
                Self::ensure_input_capability(context, "input.keyboard")?;
                Self::guard_keyboard_input(context, text)?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::Type { text: text.clone() },
                )
            }
            PlannedAction::KeyboardPress { key } => {
                Self::ensure_input_capability(context, "input.keyboard")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::KeyPress {
                        key: key.clone(),
                        modifiers: vec![],
                    },
                )
            }
            PlannedAction::KeyboardShortcut { keys } => {
                Self::ensure_input_capability(context, "input.keyboard")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::Shortcut { keys: keys.clone() },
                )
            }
            PlannedAction::ScrollWheel { direction, amount } => {
                Self::ensure_input_capability(context, "input.mouse")?;
                Self::execute_input_action_internal(
                    context,
                    InputAction::Scroll {
                        direction: direction.clone(),
                        amount: *amount,
                    },
                )
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer_control::{activate_emergency_kill_switch, reset_emergency_kill_switch};
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn ctx(tmp: &TempDir, autonomy: AutonomyLevel, autonomous: bool) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("input.mouse".to_string());
        caps.insert("input.keyboard".to_string());
        if autonomous {
            caps.insert("input.autonomous".to_string());
        }
        ActuatorContext {
            agent_id: uuid::Uuid::new_v4().to_string(),
            agent_name: "input-agent".to_string(),
            working_dir: tmp.path().to_path_buf(),
            autonomy_level: autonomy,
            capabilities: caps,
            fuel_remaining: 100.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn verify_rate_limit_100_actions_per_minute() {
        let guard = input_timestamps();
        let mut timestamps = guard.lock().unwrap();
        timestamps.clear();
        let now = now_secs();
        for _ in 0..100 {
            timestamps.push_back(now);
        }
        drop(timestamps);
        let err = InputControlActuator::check_rate_limit().unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
        guard.lock().unwrap().clear();
    }

    #[test]
    fn verify_password_field_detection_rejects_typing() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp, AutonomyLevel::L4, true);
        let err = InputControlActuator::execute_input_action_for_test(
            &ctx,
            InputAction::Type {
                text: "password=secret".into(),
            },
        );
        assert!(err.is_ok(), "raw execution helper should not block");

        let err = InputControlActuator
            .execute(
                &PlannedAction::KeyboardType {
                    text: "password=secret".into(),
                },
                &ctx,
            )
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn verify_emergency_kill_switch_stops_all_input() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp, AutonomyLevel::L4, true);
        activate_emergency_kill_switch();
        let err = InputControlActuator
            .execute(&PlannedAction::MouseMove { x: 1, y: 2 }, &ctx)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
        reset_emergency_kill_switch();
    }

    #[test]
    fn verify_l3_requirement() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp, AutonomyLevel::L2, true);
        let err = InputControlActuator
            .execute(&PlannedAction::MouseMove { x: 1, y: 2 }, &ctx)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }
}
