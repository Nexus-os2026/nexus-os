//! Actuator subsystem — governed real-world action execution.
//!
//! Provides the `ActuatorRegistry` which routes `PlannedAction`s to the correct
//! governed actuator (filesystem, shell, web, API), enforcing capability checks,
//! fuel budgets, and audit trails before every execution.

pub mod agent_lifecycle;
pub mod api;
pub mod browser;
pub mod cognitive_param;
pub mod computer_use;
pub mod docker;
pub mod filesystem;
pub mod governance_policy;
pub mod image_gen;
pub mod input;
pub mod knowledge_graph;
pub mod screen;
pub mod self_evolution;
pub mod shell;
pub mod tts;
pub mod types;
pub mod web;

pub use agent_lifecycle::AgentLifecycleActuator;
pub use api::GovernedApiClient;
pub use browser::BrowserActuator;
pub use cognitive_param::{
    AlgorithmSelectionActuator, CognitiveParamActuator, CounterfactualActuator,
    EcosystemDesignActuator, ModelOrchestrationActuator, TemporalPlanActuator,
};
pub use computer_use::ComputerUseActuator;
pub use docker::DockerActuator;
pub use filesystem::GovernedFilesystem;
pub use governance_policy::GovernancePolicyActuator;
pub use image_gen::ImageGenActuator;
pub use input::InputControlActuator;
pub use knowledge_graph::KnowledgeGraphActuator;
pub use screen::ScreenCaptureActuator;
pub use self_evolution::SelfEvolutionActuator;
pub use shell::GovernedShell;
pub use tts::TtsActuator;
pub use types::{
    ActionResult, ActionReviewDecision, ActionReviewEngine, Actuator, ActuatorContext,
    ActuatorError, SideEffect,
};
pub use web::GovernedWeb;

use crate::audit::{AuditTrail, EventType};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use serde_json::json;
use std::collections::HashMap;

/// Registry of governed actuators. Routes planned actions to the correct
/// actuator and enforces governance checks (capabilities, fuel, audit).
pub struct ActuatorRegistry {
    actuators: HashMap<String, Box<dyn Actuator>>,
}

impl std::fmt::Debug for ActuatorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActuatorRegistry")
            .field("actuator_count", &self.actuators.len())
            .field("actuators", &self.actuators.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for ActuatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ActuatorRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            actuators: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all standard actuators.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(GovernedFilesystem));
        registry.register(Box::new(GovernedShell));
        registry.register(Box::new(DockerActuator));
        registry.register(Box::new(GovernedWeb));
        registry.register(Box::new(GovernedApiClient));
        registry.register(Box::new(ImageGenActuator));
        registry.register(Box::new(TtsActuator));
        registry.register(Box::new(KnowledgeGraphActuator::default()));
        registry.register(Box::new(BrowserActuator));
        registry.register(Box::new(ScreenCaptureActuator));
        registry.register(Box::new(InputControlActuator));
        registry.register(Box::new(ComputerUseActuator));
        registry.register(Box::new(SelfEvolutionActuator));
        registry.register(Box::new(AgentLifecycleActuator));
        registry.register(Box::new(GovernancePolicyActuator));
        registry.register(Box::new(CognitiveParamActuator));
        registry.register(Box::new(ModelOrchestrationActuator));
        registry.register(Box::new(AlgorithmSelectionActuator));
        registry.register(Box::new(EcosystemDesignActuator));
        registry.register(Box::new(CounterfactualActuator));
        registry.register(Box::new(TemporalPlanActuator));
        registry
    }

    /// Register an actuator by name.
    pub fn register(&mut self, actuator: Box<dyn Actuator>) {
        self.actuators.insert(actuator.name().to_string(), actuator);
    }

    /// Execute a planned action through the appropriate actuator, with full
    /// governance enforcement:
    ///
    /// 1. Verify agent has required capabilities
    /// 2. Verify fuel budget >= estimated cost
    /// 3. Execute through actuator
    /// 4. Append audit event
    pub fn execute_action(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        audit: &mut AuditTrail,
    ) -> Result<ActionResult, ActuatorError> {
        // Find the right actuator by trying each one
        let actuator = self.find_actuator_for(action)?;

        // 1. Verify capabilities
        let required_caps = action.required_capabilities();
        for cap in &required_caps {
            if !has_capability(context.capabilities.iter().map(String::as_str), cap) {
                let _ = self.audit_action(
                    action,
                    context,
                    false,
                    &format!("capability '{cap}' denied"),
                    0.0,
                    audit,
                );
                return Err(ActuatorError::CapabilityDenied(cap.to_string()));
            }
        }

        // 2. Verify fuel budget (estimate cost before execution)
        let estimated_cost = estimate_action_cost(action);
        if context.fuel_remaining < estimated_cost {
            let _ = self.audit_action(
                action,
                context,
                false,
                "insufficient fuel",
                estimated_cost,
                audit,
            );
            return Err(ActuatorError::InsufficientFuel {
                needed: estimated_cost,
                available: context.fuel_remaining,
            });
        }

        // 3. Optional governance review for higher-risk actions.
        if should_apply_governance_review(action) {
            if let Some(review_engine) = &context.action_review_engine {
                match review_engine.review(&context.agent_id, &context.agent_name, action) {
                    Ok(ActionReviewDecision::Allow { reason }) => {
                        let _ = audit.append_event(
                            uuid::Uuid::parse_str(&context.agent_id)
                                .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                            EventType::StateChange,
                            json!({
                                "event_kind": "warden.review",
                                "decision": "allow",
                                "action_type": action.action_type(),
                                "reason": reason,
                                "agent_id": context.agent_id,
                                "agent_name": context.agent_name,
                            }),
                        );
                    }
                    Ok(ActionReviewDecision::Deny { reason }) => {
                        let _ = audit.append_event(
                            uuid::Uuid::parse_str(&context.agent_id)
                                .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                            EventType::StateChange,
                            json!({
                                "event_kind": "warden.review",
                                "decision": "deny",
                                "action_type": action.action_type(),
                                "reason": reason,
                                "agent_id": context.agent_id,
                                "agent_name": context.agent_name,
                            }),
                        );
                        return Err(ActuatorError::HumanApprovalRequired(format!(
                            "Warden blocked action: {reason}"
                        )));
                    }
                    Err(error) => {
                        let _ = audit.append_event(
                            uuid::Uuid::parse_str(&context.agent_id)
                                .unwrap_or_else(|_| uuid::Uuid::new_v4()),
                            EventType::Error,
                            json!({
                                "event_kind": "warden.review",
                                "decision": "error",
                                "action_type": action.action_type(),
                                "reason": error,
                                "agent_id": context.agent_id,
                                "agent_name": context.agent_name,
                            }),
                        );
                        return Err(ActuatorError::GovernanceReviewFailed(error));
                    }
                }
            }
        }

        // 4. Execute
        let result = actuator.execute(action, context)?;

        // 5. Audit
        let _ = self.audit_action(
            action,
            context,
            result.success,
            &result.output.chars().take(200).collect::<String>(),
            result.fuel_cost,
            audit,
        );

        Ok(result)
    }

    /// Find the actuator that handles a given action type.
    fn find_actuator_for(&self, action: &PlannedAction) -> Result<&dyn Actuator, ActuatorError> {
        let actuator_name = match action {
            PlannedAction::FileRead { .. } | PlannedAction::FileWrite { .. } => {
                "governed_filesystem"
            }
            PlannedAction::ShellCommand { .. } => "governed_shell",
            PlannedAction::DockerCommand { .. } => "docker_actuator",
            PlannedAction::WebSearch { .. } | PlannedAction::WebFetch { .. } => "governed_web",
            PlannedAction::ApiCall { .. } => "governed_api_client",
            PlannedAction::ImageGenerate { .. } => "image_gen_actuator",
            PlannedAction::TextToSpeech { .. } => "tts_actuator",
            PlannedAction::KnowledgeGraphUpdate { .. }
            | PlannedAction::KnowledgeGraphQuery { .. } => "knowledge_graph_actuator",
            PlannedAction::BrowserAutomate { .. } => "browser_actuator",
            PlannedAction::CaptureScreen { .. }
            | PlannedAction::CaptureWindow { .. }
            | PlannedAction::AnalyzeScreen { .. } => "screen_capture_actuator",
            PlannedAction::MouseMove { .. }
            | PlannedAction::MouseClick { .. }
            | PlannedAction::MouseDoubleClick { .. }
            | PlannedAction::MouseDrag { .. }
            | PlannedAction::KeyboardType { .. }
            | PlannedAction::KeyboardPress { .. }
            | PlannedAction::KeyboardShortcut { .. }
            | PlannedAction::ScrollWheel { .. } => "input_control_actuator",
            PlannedAction::ComputerAction { .. } => "computer_use_actuator",
            PlannedAction::SelfModifyDescription { .. }
            | PlannedAction::SelfModifyStrategy { .. }
            | PlannedAction::RunEvolutionTournament { .. } => "self_evolution",
            PlannedAction::CreateSubAgent { .. } | PlannedAction::DestroySubAgent { .. } => {
                "agent_lifecycle"
            }
            PlannedAction::ModifyGovernancePolicy { .. }
            | PlannedAction::AllocateEcosystemFuel { .. } => "governance_policy",
            PlannedAction::ModifyCognitiveParams { .. } => "cognitive_param",
            PlannedAction::SelectLlmProvider { .. } => "model_orchestration",
            PlannedAction::SelectAlgorithm { .. } => "algorithm_selection",
            PlannedAction::DesignAgentEcosystem { .. } => "ecosystem_design",
            PlannedAction::RunCounterfactual { .. } => "counterfactual",
            PlannedAction::TemporalPlan { .. } => "temporal_plan",
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        self.actuators
            .get(actuator_name)
            .map(|a| a.as_ref())
            .ok_or(ActuatorError::ActionNotHandled)
    }

    /// Append an audit event for an actuator execution.
    fn audit_action(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
        success: bool,
        detail: &str,
        fuel_cost: f64,
        audit: &mut AuditTrail,
    ) -> Result<(), ActuatorError> {
        let agent_uuid =
            uuid::Uuid::parse_str(&context.agent_id).unwrap_or_else(|_| uuid::Uuid::new_v4());

        audit
            .append_event(
                agent_uuid,
                EventType::ToolCall,
                json!({
                    "event_kind": "actuator.execute",
                    "action_type": action.action_type(),
                    "success": success,
                    "detail": detail,
                    "fuel_cost": fuel_cost,
                    "agent_id": context.agent_id,
                }),
            )
            .map_err(|e| ActuatorError::IoError(format!("audit: {e}")))?;

        Ok(())
    }
}

fn should_apply_governance_review(action: &PlannedAction) -> bool {
    !matches!(
        action,
        PlannedAction::FileRead { .. }
            | PlannedAction::WebSearch { .. }
            | PlannedAction::WebFetch { .. }
            | PlannedAction::MemoryRecall { .. }
            | PlannedAction::KnowledgeGraphQuery { .. }
            | PlannedAction::Noop
    )
}

/// Estimate the fuel cost of an action before execution.
fn estimate_action_cost(action: &PlannedAction) -> f64 {
    match action {
        PlannedAction::FileRead { .. } => 1.0,
        PlannedAction::FileWrite { .. } => 2.0,
        PlannedAction::ShellCommand { .. } => 5.0,
        PlannedAction::DockerCommand { .. } => 8.0,
        PlannedAction::WebSearch { .. } => 3.0,
        PlannedAction::WebFetch { .. } => 2.0,
        PlannedAction::ApiCall { .. } => 3.0,
        PlannedAction::ImageGenerate { .. } => 12.0,
        PlannedAction::TextToSpeech { .. } => 4.0,
        PlannedAction::KnowledgeGraphUpdate { .. } => 4.0,
        PlannedAction::KnowledgeGraphQuery { .. } => 2.0,
        PlannedAction::BrowserAutomate { .. } => 10.0,
        PlannedAction::CaptureScreen { .. } => 4.0,
        PlannedAction::CaptureWindow { .. } => 6.0,
        PlannedAction::AnalyzeScreen { .. } => 12.0,
        PlannedAction::MouseMove { .. } => 3.0,
        PlannedAction::MouseClick { .. } => 5.0,
        PlannedAction::MouseDoubleClick { .. } => 6.0,
        PlannedAction::MouseDrag { .. } => 7.0,
        PlannedAction::KeyboardType { text } => 5.0 + (text.chars().count() as f64 * 0.1),
        PlannedAction::KeyboardPress { .. } => 4.0,
        PlannedAction::KeyboardShortcut { keys } => 4.0 + keys.len() as f64,
        PlannedAction::ScrollWheel { amount, .. } => 3.0 + *amount as f64 * 0.1,
        PlannedAction::ComputerAction { max_steps, .. } => 20.0 + *max_steps as f64,
        PlannedAction::LlmQuery { .. } => 10.0,
        PlannedAction::SelfModifyDescription { .. } => 15.0,
        PlannedAction::SelfModifyStrategy { .. } => 10.0,
        PlannedAction::CreateSubAgent { .. } => 20.0,
        PlannedAction::DestroySubAgent { .. } => 5.0,
        PlannedAction::RunEvolutionTournament {
            variants, rounds, ..
        } => (variants.len() as f64) * (*rounds as f64) * 5.0,
        PlannedAction::ModifyGovernancePolicy { .. } => 10.0,
        PlannedAction::AllocateEcosystemFuel { .. } => 5.0,
        PlannedAction::ModifyCognitiveParams { .. } => 8.0,
        PlannedAction::SelectLlmProvider { .. } => 5.0,
        PlannedAction::SelectAlgorithm { .. } => 4.0,
        PlannedAction::DesignAgentEcosystem { .. } => 20.0,
        PlannedAction::RunCounterfactual { alternatives, .. } => 2.0 + alternatives.len() as f64,
        PlannedAction::TemporalPlan { .. } => 4.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("fs.read".into());
        caps.insert("fs.write".into());
        caps.insert("process.exec".into());
        caps.insert("web.search".into());
        caps.insert("web.read".into());
        caps.insert("mcp.call".into());
        caps.insert("screen.capture".into());
        caps.insert("screen.analyze".into());
        caps.insert("input.mouse".into());
        caps.insert("input.keyboard".into());
        caps.insert("input.autonomous".into());
        caps.insert("computer.use".into());
        ActuatorContext {
            agent_id: uuid::Uuid::new_v4().to_string(),
            agent_name: "registry-test-agent".to_string(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec!["https://api.example.com".into()],
            action_review_engine: None,
        }
    }

    #[test]
    fn registry_with_defaults() {
        let registry = ActuatorRegistry::with_defaults();
        assert_eq!(registry.actuators.len(), 21);
        assert!(registry.actuators.contains_key("governed_filesystem"));
        assert!(registry.actuators.contains_key("governed_shell"));
        assert!(registry.actuators.contains_key("docker_actuator"));
        assert!(registry.actuators.contains_key("governed_web"));
        assert!(registry.actuators.contains_key("governed_api_client"));
        assert!(registry.actuators.contains_key("image_gen_actuator"));
        assert!(registry.actuators.contains_key("tts_actuator"));
        assert!(registry.actuators.contains_key("knowledge_graph_actuator"));
        assert!(registry.actuators.contains_key("browser_actuator"));
        assert!(registry.actuators.contains_key("screen_capture_actuator"));
        assert!(registry.actuators.contains_key("input_control_actuator"));
        assert!(registry.actuators.contains_key("computer_use_actuator"));
        assert!(registry.actuators.contains_key("self_evolution"));
        assert!(registry.actuators.contains_key("agent_lifecycle"));
        assert!(registry.actuators.contains_key("governance_policy"));
        assert!(registry.actuators.contains_key("cognitive_param"));
        assert!(registry.actuators.contains_key("model_orchestration"));
        assert!(registry.actuators.contains_key("algorithm_selection"));
        assert!(registry.actuators.contains_key("ecosystem_design"));
        assert!(registry.actuators.contains_key("counterfactual"));
        assert!(registry.actuators.contains_key("temporal_plan"));
    }

    #[test]
    fn capability_check_rejects_unauthorized() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.capabilities.remove("fs.read");
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        let action = PlannedAction::FileRead {
            path: "any.txt".into(),
        };
        let err = registry
            .execute_action(&action, &ctx, &mut audit)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn fuel_deduction_after_execution() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        // Write a file
        let action = PlannedAction::FileWrite {
            path: "test.txt".into(),
            content: "hello".into(),
        };
        let result = registry.execute_action(&action, &ctx, &mut audit).unwrap();
        assert!(result.success);
        assert!(result.fuel_cost > 0.0);
    }

    #[test]
    fn audit_event_appended() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        let action = PlannedAction::FileWrite {
            path: "audited.txt".into(),
            content: "data".into(),
        };
        registry.execute_action(&action, &ctx, &mut audit).unwrap();

        let events = audit.events();
        assert!(!events.is_empty());
        let last = events.last().unwrap();
        assert_eq!(
            last.payload.get("event_kind").and_then(|v| v.as_str()),
            Some("actuator.execute")
        );
    }

    #[test]
    fn insufficient_fuel_rejected() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.fuel_remaining = 0.5; // Not enough for shell (5.0)
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["hi".into()],
        };
        let err = registry
            .execute_action(&action, &ctx, &mut audit)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::InsufficientFuel { .. }));
    }

    #[test]
    fn full_cycle_write_read() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        // Write
        let write = PlannedAction::FileWrite {
            path: "cycle.txt".into(),
            content: "full cycle test".into(),
        };
        let r = registry.execute_action(&write, &ctx, &mut audit).unwrap();
        assert!(r.success);

        // Read back
        let read = PlannedAction::FileRead {
            path: "cycle.txt".into(),
        };
        let r = registry.execute_action(&read, &ctx, &mut audit).unwrap();
        assert!(r.success);
        assert_eq!(r.output, "full cycle test");

        // Verify audit events
        assert!(audit.events().len() >= 2);
    }

    #[test]
    fn action_not_handled_for_noop() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        let action = PlannedAction::Noop;
        let err = registry
            .execute_action(&action, &ctx, &mut audit)
            .unwrap_err();
        assert!(matches!(err, ActuatorError::ActionNotHandled));
    }

    #[test]
    fn shell_through_registry() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let registry = ActuatorRegistry::with_defaults();
        let mut audit = AuditTrail::new();

        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["registry_test".into()],
        };
        let r = registry.execute_action(&action, &ctx, &mut audit).unwrap();
        assert!(r.success);
        assert!(r.output.contains("registry_test"));
    }

    #[test]
    fn new_planned_action_variants_are_routed() {
        let registry = ActuatorRegistry::with_defaults();
        let actions = vec![
            PlannedAction::CaptureScreen { region: None },
            PlannedAction::CaptureWindow {
                window_title: "Firefox".into(),
            },
            PlannedAction::AnalyzeScreen {
                query: "what is visible?".into(),
            },
            PlannedAction::MouseMove { x: 1, y: 2 },
            PlannedAction::MouseClick {
                x: 1,
                y: 2,
                button: "left".into(),
            },
            PlannedAction::MouseDoubleClick { x: 1, y: 2 },
            PlannedAction::MouseDrag {
                from_x: 1,
                from_y: 2,
                to_x: 3,
                to_y: 4,
            },
            PlannedAction::KeyboardType {
                text: "hello".into(),
            },
            PlannedAction::KeyboardPress {
                key: "Enter".into(),
            },
            PlannedAction::KeyboardShortcut {
                keys: vec!["Ctrl".into(), "L".into()],
            },
            PlannedAction::ScrollWheel {
                direction: "down".into(),
                amount: 2,
            },
            PlannedAction::ComputerAction {
                description: "click once".into(),
                max_steps: 2,
            },
        ];

        for action in actions {
            assert!(
                registry.find_actuator_for(&action).is_ok(),
                "missing actuator for {}",
                action.action_type()
            );
        }
    }
}
