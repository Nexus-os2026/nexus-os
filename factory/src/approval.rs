use crate::code_gen::{passes_nex_safety_checks, GeneratedAgentCode};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::manifest::parse_manifest;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
    pub approved: bool,
    pub deployed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub approved: bool,
    pub deployed: bool,
    pub agent_id: Option<String>,
    pub state: Option<AgentState>,
}

#[derive(Debug, Default)]
pub struct ApprovalFlow {
    supervisor: Supervisor,
    audit_trail: AuditTrail,
}

impl ApprovalFlow {
    pub fn new() -> Self {
        Self {
            supervisor: Supervisor::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn present_for_review(
        &mut self,
        capabilities: Vec<String>,
        fuel_budget: u64,
    ) -> ApprovalRequest {
        let _ = self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "factory_approval_presented",
                "capabilities": capabilities,
                "fuel_budget": fuel_budget,
            }),
        );

        ApprovalRequest {
            capabilities,
            fuel_budget,
            approved: false,
            deployed: false,
        }
    }

    pub fn approve_and_deploy(
        &mut self,
        request: &ApprovalRequest,
        manifest_toml: &str,
        generated_code: &GeneratedAgentCode,
        user_approved: bool,
    ) -> Result<DeploymentResult, AgentError> {
        if !user_approved {
            let _ = self.audit_trail.append_event(
                uuid::Uuid::nil(),
                EventType::UserAction,
                json!({"event": "factory_approval_rejected"}),
            );

            return Ok(DeploymentResult {
                approved: false,
                deployed: false,
                agent_id: None,
                state: None,
            });
        }

        if !passes_nex_safety_checks(generated_code) {
            return Err(AgentError::SupervisorError(
                "generated code failed safety checks".to_string(),
            ));
        }

        let manifest = parse_manifest(manifest_toml)?;
        let agent_id = self.supervisor.start_agent(manifest)?;

        let state = self
            .supervisor
            .get_agent(agent_id)
            .map(|agent| agent.state)
            .unwrap_or(AgentState::Created);

        let _ = self.audit_trail.append_event(
            agent_id,
            EventType::StateChange,
            json!({
                "event": "factory_deployed",
                "capabilities": request.capabilities,
                "fuel_budget": request.fuel_budget,
                "state": format!("{state}"),
            }),
        );

        Ok(DeploymentResult {
            approved: true,
            deployed: true,
            agent_id: Some(agent_id.to_string()),
            state: Some(state),
        })
    }

    pub fn agent_state(&self, id: AgentId) -> Option<AgentState> {
        self.supervisor.get_agent(id).map(|agent| agent.state)
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }
}

#[cfg(test)]
mod tests {
    use super::ApprovalFlow;
    use crate::capabilities::map_intent_to_capabilities;
    use crate::code_gen::generate_agent_code;
    use crate::intent::{ParsedIntent, TaskType};
    use crate::manifest_gen::generate_manifest_toml;

    #[test]
    fn test_approval_required() {
        let intent = ParsedIntent {
            task_type: TaskType::ContentPosting,
            platforms: vec!["twitter".to_string()],
            schedule: "daily".to_string(),
            content_topic: "ai".to_string(),
            raw_request: "Post about AI on Twitter daily".to_string(),
        };
        let capabilities = map_intent_to_capabilities(&intent);
        let generated_manifest = generate_manifest_toml(&intent, &capabilities);
        let generated_code = generate_agent_code(&intent);

        let mut flow = ApprovalFlow::new();
        let request = flow.present_for_review(
            capabilities.required.clone(),
            generated_manifest.fuel_budget,
        );

        let denied = flow.approve_and_deploy(
            &request,
            generated_manifest.toml.as_str(),
            &generated_code,
            false,
        );
        assert!(denied.is_ok());
        if let Ok(denied) = denied {
            assert!(!denied.deployed);
        }

        let approved = flow.approve_and_deploy(
            &request,
            generated_manifest.toml.as_str(),
            &generated_code,
            true,
        );
        assert!(approved.is_ok());
        if let Ok(approved) = approved {
            assert!(approved.deployed);
            assert!(approved.agent_id.is_some());
        }
    }
}
