use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use crate::computer_control::{
    analyze_stored_screenshot, append_audit_log, capture_and_store_screen, checkpoints_path,
    execute_input_action, write_bytes_to_workspace_file, InputAction,
};
use crate::time_machine::{Checkpoint, TimeMachine};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct ComputerUseActuator;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ComputerUseDecision {
    action: String,
    x: Option<u32>,
    y: Option<u32>,
    text: Option<String>,
    key: Option<String>,
    direction: Option<String>,
    amount: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CheckpointLedger {
    checkpoints: Vec<Checkpoint>,
}

impl ComputerUseActuator {
    fn ensure_allowed(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L4 {
            return Err(ActuatorError::CapabilityDenied(
                "computer use requires L4+ autonomy".to_string(),
            ));
        }
        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "computer.use",
        ) {
            return Err(ActuatorError::CapabilityDenied("computer.use".to_string()));
        }
        Ok(())
    }

    fn persist_checkpoint(
        context: &ActuatorContext,
        description: &str,
    ) -> Result<Checkpoint, ActuatorError> {
        let mut tm = TimeMachine::default();
        let mut builder = tm.begin_checkpoint("computer_use", Some(context.agent_id.clone()));
        builder.record_config_change(
            "computer.use.goal",
            serde_json::Value::Null,
            json!(description),
        );
        let checkpoint = builder.build();
        tm.commit_checkpoint(checkpoint.clone())
            .map_err(|e| ActuatorError::IoError(format!("commit computer_use checkpoint: {e}")))?;

        let ledger_path = checkpoints_path(&context.working_dir);
        if let Some(parent) = ledger_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ActuatorError::IoError(format!("create checkpoint dir: {e}")))?;
        }
        let mut ledger = if ledger_path.exists() {
            let raw = std::fs::read_to_string(&ledger_path)
                .map_err(|e| ActuatorError::IoError(format!("read checkpoint ledger: {e}")))?;
            serde_json::from_str::<CheckpointLedger>(&raw)
                .map_err(|e| ActuatorError::IoError(format!("parse checkpoint ledger: {e}")))?
        } else {
            CheckpointLedger::default()
        };
        ledger.checkpoints.push(checkpoint.clone());
        std::fs::write(
            &ledger_path,
            serde_json::to_string_pretty(&ledger)
                .map_err(|e| ActuatorError::IoError(format!("serialize checkpoint ledger: {e}")))?,
        )
        .map_err(|e| ActuatorError::IoError(format!("write checkpoint ledger: {e}")))?;
        Ok(checkpoint)
    }

    fn parse_decision(raw: &str) -> Result<ComputerUseDecision, ActuatorError> {
        let trimmed = raw.trim();
        let candidate = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        serde_json::from_str(candidate)
            .map_err(|e| ActuatorError::IoError(format!("parse computer-use decision: {e}")))
    }

    fn screenshot_side_effect(path: &std::path::Path) -> SideEffect {
        SideEffect::FileCreated {
            path: path.to_path_buf(),
        }
    }

    fn execute_decision(
        _context: &ActuatorContext,
        decision: &ComputerUseDecision,
    ) -> Result<String, ActuatorError> {
        match decision.action.as_str() {
            "click" => {
                let x = decision
                    .x
                    .ok_or_else(|| ActuatorError::IoError("decision missing x".to_string()))?;
                let y = decision
                    .y
                    .ok_or_else(|| ActuatorError::IoError("decision missing y".to_string()))?;
                execute_input_action(&InputAction::Click {
                    x,
                    y,
                    button: crate::computer_control::MouseButton::Left,
                })
                .map_err(ActuatorError::IoError)?;
                Ok(format!("click({x},{y})"))
            }
            "type" => {
                let text = decision
                    .text
                    .clone()
                    .ok_or_else(|| ActuatorError::IoError("decision missing text".to_string()))?;
                execute_input_action(&InputAction::Type { text: text.clone() })
                    .map_err(ActuatorError::IoError)?;
                Ok(format!("type({text})"))
            }
            "key" => {
                let key = decision
                    .key
                    .clone()
                    .ok_or_else(|| ActuatorError::IoError("decision missing key".to_string()))?;
                execute_input_action(&InputAction::KeyPress {
                    key: key.clone(),
                    modifiers: vec![],
                })
                .map_err(ActuatorError::IoError)?;
                Ok(format!("key({key})"))
            }
            "scroll" => {
                let direction = decision
                    .direction
                    .clone()
                    .unwrap_or_else(|| "down".to_string());
                let amount = decision.amount.unwrap_or(1);
                execute_input_action(&InputAction::Scroll {
                    direction: direction.clone(),
                    amount,
                })
                .map_err(ActuatorError::IoError)?;
                Ok(format!("scroll({direction}, {amount})"))
            }
            "done" => Ok("done".to_string()),
            other => Err(ActuatorError::IoError(format!(
                "unsupported computer-use action '{other}'"
            ))),
        }
    }

    #[cfg_attr(test, allow(unused_variables))]
    fn decide_next_action(
        screenshot: &std::path::Path,
        description: &str,
        previous_action: Option<&str>,
    ) -> Result<ComputerUseDecision, ActuatorError> {
        #[cfg(test)]
        {
            if previous_action.is_none() && description.contains("click once") {
                return Ok(ComputerUseDecision {
                    action: "click".to_string(),
                    x: Some(10),
                    y: Some(10),
                    text: None,
                    key: None,
                    direction: None,
                    amount: None,
                });
            }
            return Ok(ComputerUseDecision {
                action: "done".to_string(),
                x: None,
                y: None,
                text: None,
                key: None,
                direction: None,
                amount: None,
            });
        }

        #[allow(unreachable_code)]
        {
            let prompt = if let Some(previous_action) = previous_action {
                format!(
                    "You are controlling a computer. Previous action: {previous_action}. New screen: [screenshot]. Goal: {description}. Is the goal complete? If not, what's the next action? Respond with JSON: {{\"action\":\"click|type|key|scroll|done\",\"x\":number,\"y\":number,\"text\":string,\"key\":string,\"direction\":string,\"amount\":number}}"
                )
            } else {
                format!(
                    "You are controlling a computer. Current screen: [screenshot]. Goal: {description}. What is the next mouse/keyboard action to take? Respond with JSON: {{\"action\":\"click|type|key|scroll|done\",\"x\":number,\"y\":number,\"text\":string,\"key\":string,\"direction\":string,\"amount\":number}}"
                )
            };
            let raw = analyze_stored_screenshot(screenshot, &prompt, None)
                .map_err(ActuatorError::IoError)?;
            Self::parse_decision(&raw)
        }
    }

    fn run_loop(
        context: &ActuatorContext,
        description: &str,
        max_steps: u32,
    ) -> Result<ActionResult, ActuatorError> {
        Self::ensure_allowed(context)?;
        let checkpoint = Self::persist_checkpoint(context, description)?;
        append_audit_log(
            &context.working_dir,
            "computer.use.start",
            json!({
                "description": description,
                "max_steps": max_steps,
                "checkpoint_id": checkpoint.id,
            }),
        )
        .map_err(ActuatorError::IoError)?;

        let mut side_effects = Vec::new();
        let mut actions = Vec::new();
        let mut previous_action: Option<String> = None;
        let max_steps = max_steps.max(1);

        for step in 0..max_steps {
            let before = capture_and_store_screen(
                &context.working_dir,
                None,
                &format!("computer-use-step-{step}-before"),
            )
            .map_err(ActuatorError::IoError)?;
            side_effects.push(Self::screenshot_side_effect(&before));
            let decision =
                Self::decide_next_action(&before, description, previous_action.as_deref())?;
            append_audit_log(
                &context.working_dir,
                "computer.use.decision",
                json!({
                    "step": step,
                    "decision": decision,
                    "screenshot": before,
                }),
            )
            .map_err(ActuatorError::IoError)?;

            if decision.action == "done" {
                let final_capture =
                    capture_and_store_screen(&context.working_dir, None, "computer-use-final")
                        .map_err(ActuatorError::IoError)?;
                side_effects.push(Self::screenshot_side_effect(&final_capture));
                let summary = format!(
                    "Computer action completed after {} steps. Final screenshot: {}. Actions: {}",
                    actions.len(),
                    final_capture.display(),
                    if actions.is_empty() {
                        "none".to_string()
                    } else {
                        actions.join(", ")
                    }
                );
                return Ok(ActionResult {
                    success: true,
                    output: summary,
                    fuel_cost: 20.0 + actions.len() as f64 * 5.0,
                    side_effects,
                });
            }

            let action_label = Self::execute_decision(context, &decision)?;
            actions.push(action_label.clone());
            previous_action = Some(action_label);
            std::thread::sleep(std::time::Duration::from_millis(500));

            let after = capture_and_store_screen(
                &context.working_dir,
                None,
                &format!("computer-use-step-{step}-after"),
            )
            .map_err(ActuatorError::IoError)?;
            side_effects.push(Self::screenshot_side_effect(&after));
        }

        let last_path = write_bytes_to_workspace_file(
            &context.working_dir,
            "computer-use-max-steps",
            b"max_steps_reached",
        )
        .map_err(ActuatorError::IoError)?;
        side_effects.push(Self::screenshot_side_effect(&last_path));
        Ok(ActionResult {
            success: false,
            output: format!(
                "Computer action stopped after reaching max_steps={max_steps}. Actions: {}",
                actions.join(", ")
            ),
            fuel_cost: 20.0 + max_steps as f64 * 5.0,
            side_effects,
        })
    }
}

impl Actuator for ComputerUseActuator {
    fn name(&self) -> &str {
        "computer_use_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["computer.use".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        match action {
            PlannedAction::ComputerAction {
                description,
                max_steps,
            } => Self::run_loop(context, description, *max_steps),
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn ctx(tmp: &TempDir) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("computer.use".to_string());
        ActuatorContext {
            agent_id: uuid::Uuid::new_v4().to_string(),
            agent_name: "computer-use-agent".to_string(),
            working_dir: tmp.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L4,
            capabilities: caps,
            fuel_remaining: 500.0,
            egress_allowlist: vec![],
            action_review_engine: None,
        }
    }

    #[test]
    fn verify_max_steps_limit() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp);
        let result = ComputerUseActuator::run_loop(&ctx, "click once", 1).unwrap();
        assert!(!result.success);
        assert!(result.output.contains("max_steps=1"));
    }

    #[test]
    fn verify_screenshot_taken_before_and_after_each_action() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp);
        let result = ComputerUseActuator::run_loop(&ctx, "click once", 2).unwrap();
        let screenshot_count = result
            .side_effects
            .iter()
            .filter(|effect| matches!(effect, SideEffect::FileCreated { .. }))
            .count();
        assert!(screenshot_count >= 3);
    }

    #[test]
    fn verify_time_machine_checkpoint_created() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp);
        let _ = ComputerUseActuator::run_loop(&ctx, "click once", 2).unwrap();
        let ledger = std::fs::read_to_string(checkpoints_path(tmp.path())).unwrap();
        assert!(ledger.contains("computer_use"));
    }
}
