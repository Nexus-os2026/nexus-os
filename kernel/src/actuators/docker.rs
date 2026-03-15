use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;

const MAX_OUTPUT_BYTES: usize = 100 * 1024;
const FUEL_COST_DOCKER: f64 = 8.0;
const ALLOWED_SUBCOMMANDS: &[&str] = &["run", "ps", "stop", "logs"];

#[derive(Debug, Clone)]
pub struct DockerActuator;

impl DockerActuator {
    fn validate(subcommand: &str, args: &[String]) -> Result<(), ActuatorError> {
        let normalized = subcommand.to_lowercase();
        if !ALLOWED_SUBCOMMANDS.contains(&normalized.as_str()) {
            return Err(ActuatorError::CommandBlocked(format!(
                "docker subcommand '{subcommand}' is not allowed"
            )));
        }

        if normalized == "run"
            && args.iter().any(|arg| {
                let lowered = arg.to_lowercase();
                lowered == "--privileged" || lowered.starts_with("--privileged=")
            })
        {
            return Err(ActuatorError::CommandBlocked(
                "docker privileged mode is not allowed".to_string(),
            ));
        }

        if args
            .iter()
            .any(|arg| arg == "rm" || arg == "-f" || arg == "--force")
        {
            return Err(ActuatorError::CommandBlocked(
                "docker force-removal is not allowed".to_string(),
            ));
        }

        Ok(())
    }
}

impl Actuator for DockerActuator {
    fn name(&self) -> &str {
        "docker_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["docker_run".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (subcommand, args) = match action {
            PlannedAction::DockerCommand { subcommand, args } => (subcommand, args),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "docker_run",
        ) {
            return Err(ActuatorError::CapabilityDenied("docker_run".into()));
        }

        Self::validate(subcommand, args)?;

        let output = std::process::Command::new("docker")
            .arg(subcommand)
            .args(args)
            .current_dir(&context.working_dir)
            .output()
            .map_err(|error| ActuatorError::IoError(format!("spawn docker: {error}")))?;

        let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&stderr);
        }
        if combined.len() > MAX_OUTPUT_BYTES {
            combined.truncate(MAX_OUTPUT_BYTES);
            combined.push_str("\n... [output truncated]");
        }

        let mut command_parts = vec!["docker".to_string(), subcommand.clone()];
        command_parts.extend(args.iter().cloned());

        Ok(ActionResult {
            success: output.status.success(),
            output: combined,
            fuel_cost: FUEL_COST_DOCKER,
            side_effects: vec![SideEffect::CommandExecuted {
                command: command_parts.join(" "),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context() -> (TempDir, ActuatorContext) {
        let tempdir = TempDir::new().unwrap();
        let working_dir = tempdir.path().to_path_buf();
        let mut capabilities = HashSet::new();
        capabilities.insert("docker_run".to_string());
        (
            tempdir,
            ActuatorContext {
                agent_id: "agent".into(),
                working_dir,
                autonomy_level: AutonomyLevel::L2,
                capabilities,
                fuel_remaining: 100.0,
                egress_allowlist: vec![],
            },
        )
    }

    #[test]
    fn blocks_non_allowlisted_subcommands() {
        let error = DockerActuator::validate("rm", &[]).unwrap_err();
        assert!(matches!(error, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn blocks_privileged_mode() {
        let error = DockerActuator::validate("run", &[String::from("--privileged")]).unwrap_err();
        assert!(matches!(error, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn requires_capability() {
        let (_tempdir, mut context) = make_context();
        context.capabilities.clear();
        let action = PlannedAction::DockerCommand {
            subcommand: "ps".into(),
            args: vec![],
        };
        let error = DockerActuator.execute(&action, &context).unwrap_err();
        assert!(matches!(error, ActuatorError::CapabilityDenied(_)));
    }
}
