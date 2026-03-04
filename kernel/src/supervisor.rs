use crate::audit::AuditTrail;
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::errors::AgentError;
use crate::lifecycle::{transition_state, AgentState};
use crate::manifest::AgentManifest;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub type AgentId = Uuid;

#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub autonomy_guard: AutonomyGuard,
    pub state: AgentState,
    pub remaining_fuel: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub id: AgentId,
    pub state: AgentState,
    pub remaining_fuel: u64,
}

#[derive(Debug, Default)]
pub struct Supervisor {
    agents: HashMap<AgentId, AgentHandle>,
    audit_trail: AuditTrail,
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let id = Uuid::new_v4();
        let autonomy_level = AutonomyLevel::from_manifest(manifest.autonomy_level);
        let mut handle = AgentHandle {
            id,
            remaining_fuel: manifest.fuel_budget,
            autonomy_guard: AutonomyGuard::new(autonomy_level),
            manifest,
            state: AgentState::Created,
        };

        let _ = self.audit_trail.append_event(
            id,
            crate::audit::EventType::StateChange,
            json!({
                "event": "autonomy.level_initialized",
                "level": autonomy_level.as_str(),
            }),
        );

        handle.state = transition_state(handle.state, AgentState::Starting)?;
        Self::consume_fuel(&mut handle)?;
        handle.state = transition_state(handle.state, AgentState::Running)?;

        self.agents.insert(id, handle);
        Ok(id)
    }

    pub fn stop_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        match handle.state {
            AgentState::Running | AgentState::Paused => {
                handle.state = transition_state(handle.state, AgentState::Stopping)?;
                handle.state = transition_state(handle.state, AgentState::Stopped)?;
                Ok(())
            }
            AgentState::Stopping | AgentState::Stopped | AgentState::Destroyed => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: handle.state,
                to: AgentState::Stopping,
            }),
        }
    }

    pub fn pause_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        match handle.state {
            AgentState::Running => {
                handle.state = transition_state(handle.state, AgentState::Paused)?;
                Ok(())
            }
            AgentState::Paused => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: handle.state,
                to: AgentState::Paused,
            }),
        }
    }

    pub fn resume_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        match handle.state {
            AgentState::Paused => {
                handle.state = transition_state(handle.state, AgentState::Running)?;
                Ok(())
            }
            AgentState::Running => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: handle.state,
                to: AgentState::Running,
            }),
        }
    }

    pub fn restart_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let current_state = self
            .agents
            .get(&id)
            .map(|agent| agent.state)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        if matches!(current_state, AgentState::Running | AgentState::Paused) {
            self.stop_agent(id)?;
        }

        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        Self::consume_fuel(handle)?;
        handle.state = transition_state(handle.state, AgentState::Starting)?;
        handle.state = transition_state(handle.state, AgentState::Running)?;
        Ok(())
    }

    pub fn health_check(&self) -> Vec<AgentStatus> {
        let mut statuses = self
            .agents
            .values()
            .map(|agent| AgentStatus {
                id: agent.id,
                state: agent.state,
                remaining_fuel: agent.remaining_fuel,
            })
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| status.id);
        statuses
    }

    pub fn get_agent(&self, id: AgentId) -> Option<&AgentHandle> {
        self.agents.get(&id)
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn require_tool_call(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .autonomy_guard
            .require_tool_call(id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn require_multi_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .autonomy_guard
            .require_multi_agent(id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn require_self_modification(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .autonomy_guard
            .require_self_modification(id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn require_distributed(&mut self, id: AgentId) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .autonomy_guard
            .require_distributed(id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    fn consume_fuel(agent: &mut AgentHandle) -> Result<(), AgentError> {
        if agent.remaining_fuel == 0 {
            return Err(AgentError::FuelExhausted);
        }
        agent.remaining_fuel -= 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Supervisor;
    use crate::errors::AgentError;
    use crate::lifecycle::AgentState;
    use crate::manifest::AgentManifest;

    fn sample_manifest(fuel_budget: u64) -> AgentManifest {
        AgentManifest {
            name: "my-social-poster".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["web.search".to_string(), "llm.query".to_string()],
            fuel_budget,
            autonomy_level: None,
            schedule: None,
            llm_model: Some("ollama/llama3".to_string()),
        }
    }

    #[test]
    fn test_start_and_stop_agent() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(10))
            .expect("agent should start successfully");

        let started = supervisor
            .get_agent(id)
            .expect("started agent should exist");
        assert_eq!(started.state, AgentState::Running);

        let stopped = supervisor.stop_agent(id);
        assert!(stopped.is_ok());

        let status = supervisor
            .get_agent(id)
            .expect("stopped agent should exist");
        assert_eq!(status.state, AgentState::Stopped);
    }

    #[test]
    fn test_fuel_exhaustion_prevents_restart() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(1))
            .expect("initial start should consume only available fuel");

        let restart_result = supervisor.restart_agent(id);
        assert_eq!(restart_result, Err(AgentError::FuelExhausted));
    }

    #[test]
    fn test_pause_and_resume_agent() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(10))
            .expect("agent should start");

        let paused = supervisor.pause_agent(id);
        assert!(paused.is_ok());
        let paused_status = supervisor.get_agent(id).expect("paused agent should exist");
        assert_eq!(paused_status.state, AgentState::Paused);

        let resumed = supervisor.resume_agent(id);
        assert!(resumed.is_ok());
        let running_status = supervisor
            .get_agent(id)
            .expect("resumed agent should exist");
        assert_eq!(running_status.state, AgentState::Running);
    }

    #[test]
    fn test_pause_requires_running_state() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(10))
            .expect("agent should start");
        let stopped = supervisor.stop_agent(id);
        assert!(stopped.is_ok());

        let paused = supervisor.pause_agent(id);
        assert_eq!(
            paused,
            Err(AgentError::InvalidTransition {
                from: AgentState::Stopped,
                to: AgentState::Paused,
            })
        );
    }

    #[test]
    fn test_start_agent_emits_autonomy_initialized_event() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(5))
            .expect("agent should start successfully");

        let initialized = supervisor.audit_trail().events().iter().find(|event| {
            event.agent_id == id
                && event.payload.get("event").and_then(|value| value.as_str())
                    == Some("autonomy.level_initialized")
        });
        let initialized = initialized.expect("autonomy initialized event should be present");
        assert_eq!(
            initialized
                .payload
                .get("level")
                .and_then(|value| value.as_str()),
            Some("L0")
        );
    }
}
