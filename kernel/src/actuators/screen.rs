use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::autonomy::AutonomyLevel;
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use crate::computer_control::{
    append_audit_log, capture_and_analyze_screen, capture_and_store_screen,
    capture_and_store_window,
};
use serde_json::json;

#[derive(Debug, Clone, Default)]
pub struct ScreenCaptureActuator;

impl ScreenCaptureActuator {
    fn ensure_capture_allowed(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L2 {
            return Err(ActuatorError::CapabilityDenied(
                "screen capture requires L2+ autonomy".to_string(),
            ));
        }
        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "screen.capture",
        ) {
            return Err(ActuatorError::CapabilityDenied(
                "screen.capture".to_string(),
            ));
        }
        Ok(())
    }

    fn ensure_analysis_allowed(context: &ActuatorContext) -> Result<(), ActuatorError> {
        if context.autonomy_level < AutonomyLevel::L3 {
            return Err(ActuatorError::CapabilityDenied(
                "screen analysis requires L3+ autonomy".to_string(),
            ));
        }
        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "screen.analyze",
        ) {
            return Err(ActuatorError::CapabilityDenied(
                "screen.analyze".to_string(),
            ));
        }
        Ok(())
    }
}

impl Actuator for ScreenCaptureActuator {
    fn name(&self) -> &str {
        "screen_capture_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["screen.capture".to_string(), "screen.analyze".to_string()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        match action {
            PlannedAction::CaptureScreen { region } => {
                Self::ensure_capture_allowed(context)?;
                let path = capture_and_store_screen(
                    &context.working_dir,
                    region.as_ref(),
                    "capture-screen",
                )
                .map_err(ActuatorError::IoError)?;
                Ok(ActionResult {
                    success: true,
                    output: path.display().to_string(),
                    fuel_cost: 4.0,
                    side_effects: vec![SideEffect::FileCreated { path }],
                })
            }
            PlannedAction::CaptureWindow { window_title } => {
                Self::ensure_capture_allowed(context)?;
                let path = capture_and_store_window(&context.working_dir, window_title)
                    .map_err(ActuatorError::IoError)?;
                Ok(ActionResult {
                    success: true,
                    output: path.display().to_string(),
                    fuel_cost: 6.0,
                    side_effects: vec![SideEffect::FileCreated { path }],
                })
            }
            PlannedAction::AnalyzeScreen { query } => {
                Self::ensure_analysis_allowed(context)?;
                let analysis = capture_and_analyze_screen(&context.working_dir, query, None)
                    .map_err(ActuatorError::IoError)?;
                append_audit_log(
                    &context.working_dir,
                    "screen.analysis.result",
                    json!({
                        "model": analysis.model,
                        "path": analysis.screenshot_path,
                    }),
                )
                .map_err(ActuatorError::IoError)?;
                Ok(ActionResult {
                    success: true,
                    output: analysis.output,
                    fuel_cost: 12.0,
                    side_effects: vec![SideEffect::FileCreated {
                        path: analysis.screenshot_path,
                    }],
                })
            }
            _ => Err(ActuatorError::ActionNotHandled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computer_control::audit_log_path;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn ctx(tmp: &TempDir) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("screen.capture".to_string());
        caps.insert("screen.analyze".to_string());
        ActuatorContext {
            agent_id: uuid::Uuid::new_v4().to_string(),
            agent_name: "screen-agent".to_string(),
            working_dir: tmp.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L3,
            capabilities: caps,
            fuel_remaining: 100.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn verify_capability_check() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx(&tmp);
        ctx.capabilities.remove("screen.capture");
        let actuator = ScreenCaptureActuator;
        let err = actuator
            .execute(&PlannedAction::CaptureScreen { region: None }, &ctx)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn verify_audit_logging_after_capture() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(&tmp);
        let actuator = ScreenCaptureActuator;
        let _ = actuator.execute(&PlannedAction::CaptureScreen { region: None }, &ctx);
        let audit_path = audit_log_path(tmp.path());
        if audit_path.exists() {
            let contents = std::fs::read_to_string(audit_path).unwrap();
            assert!(contents.contains("screen.capture"));
        }
    }
}
