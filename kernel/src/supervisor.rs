use crate::audit::{AuditTrail, EventType};
use crate::errors::AgentError;
use crate::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelViolation,
};
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
    pub state: AgentState,
    pub remaining_fuel: u64,
    pub autonomy_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub id: AgentId,
    pub state: AgentState,
    pub remaining_fuel: u64,
    pub autonomy_level: u8,
}

#[derive(Debug, Default)]
pub struct Supervisor {
    agents: HashMap<AgentId, AgentHandle>,
    fuel_ledgers: HashMap<AgentId, AgentFuelLedger>,
    audit_trail: AuditTrail,
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            fuel_ledgers: HashMap::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let id = Uuid::new_v4();
        let autonomy_level = manifest.autonomy_level.unwrap_or(0).min(5);
        let period_id = BudgetPeriodId::new(
            manifest
                .fuel_period_id
                .clone()
                .unwrap_or_else(|| "period.default".to_string()),
        );
        let monthly_cap = manifest.monthly_fuel_cap.unwrap_or(manifest.fuel_budget);

        let handle = AgentHandle {
            id,
            remaining_fuel: manifest.fuel_budget,
            manifest,
            state: AgentState::Created,
            autonomy_level,
        };

        self.agents.insert(id, handle);
        self.fuel_ledgers.insert(
            id,
            AgentFuelLedger::new(
                period_id.clone(),
                monthly_cap,
                BurnAnomalyDetector::default(),
            ),
        );

        let _ = self.audit_trail.append_event(
            id,
            EventType::StateChange,
            json!({
                "event_kind": "fuel.period_set",
                "agent_id": id,
                "period": period_id.0,
                "cap_units": monthly_cap,
                "spent_units": 0,
            }),
        );
        let _ = self.audit_trail.append_event(
            id,
            EventType::UserAction,
            json!({
                "event_kind": "autonomy.level_initialized",
                "agent_id": id,
                "level": autonomy_level,
            }),
        );

        let start_result = (|| -> Result<(), AgentError> {
            {
                let entry = self.agents.get_mut(&id).ok_or_else(|| {
                    AgentError::SupervisorError(format!("agent '{id}' not found"))
                })?;
                entry.state = transition_state(entry.state, AgentState::Starting)?;
            }

            self.consume_fuel(id, "supervisor.start")?;

            let entry = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            entry.state = transition_state(entry.state, AgentState::Running)?;
            Ok(())
        })();

        if let Err(error) = start_result {
            self.agents.remove(&id);
            self.fuel_ledgers.remove(&id);
            return Err(error);
        }

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

        self.consume_fuel(id, "supervisor.restart")?;

        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.state = transition_state(handle.state, AgentState::Starting)?;
        handle.state = transition_state(handle.state, AgentState::Running)?;
        Ok(())
    }

    pub fn record_llm_spend(
        &mut self,
        id: AgentId,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_units: u64,
    ) -> Result<(), AgentError> {
        let output_units = u64::from(output_tokens);
        {
            let handle = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            if handle.remaining_fuel < output_units {
                return Err(self.apply_fuel_violation(
                    id,
                    FuelViolation::OverMonthlyCap,
                    "LLM token spend exceeded remaining fuel",
                ));
            }
            handle.remaining_fuel -= output_units;
        }

        let ledger = self.fuel_ledgers.get_mut(&id).ok_or_else(|| {
            AgentError::SupervisorError(format!("agent '{id}' missing fuel ledger"))
        })?;

        match ledger.record_llm_spend(
            id,
            model,
            input_tokens,
            output_tokens,
            cost_units,
            &mut self.audit_trail,
        ) {
            Ok(()) => Ok(()),
            Err(violation) => Err(self.apply_fuel_violation(
                id,
                violation,
                "fuel hardening violation from LLM spend",
            )),
        }
    }

    pub fn fuel_audit_report(&self, id: AgentId) -> Option<FuelAuditReport> {
        self.fuel_ledgers.get(&id).map(|ledger| ledger.snapshot(id))
    }

    pub fn health_check(&self) -> Vec<AgentStatus> {
        let mut statuses = self
            .agents
            .values()
            .map(|agent| AgentStatus {
                id: agent.id,
                state: agent.state,
                remaining_fuel: agent.remaining_fuel,
                autonomy_level: agent.autonomy_level,
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

    fn consume_fuel(&mut self, id: AgentId, reason: &str) -> Result<(), AgentError> {
        let model_name = {
            let agent = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

            if agent.remaining_fuel == 0 {
                return Err(self.apply_fuel_violation(
                    id,
                    FuelViolation::OverMonthlyCap,
                    "runtime fuel exhausted",
                ));
            }
            agent.remaining_fuel -= 1;
            agent
                .manifest
                .llm_model
                .clone()
                .unwrap_or_else(|| "runtime".to_string())
        };

        let ledger = self.fuel_ledgers.get_mut(&id).ok_or_else(|| {
            AgentError::SupervisorError(format!("agent '{id}' missing fuel ledger"))
        })?;

        match ledger.record_llm_spend(id, model_name.as_str(), 0, 1, 1, &mut self.audit_trail) {
            Ok(()) => Ok(()),
            Err(violation) => Err(self.apply_fuel_violation(id, violation, reason)),
        }
    }

    fn apply_fuel_violation(
        &mut self,
        id: AgentId,
        violation: FuelViolation,
        reason: &str,
    ) -> AgentError {
        if let Some(ledger) = self.fuel_ledgers.get_mut(&id) {
            ledger.register_violation(id, violation.clone(), reason, &mut self.audit_trail);
        }

        let mut previous_level = 0_u8;
        if let Some(agent) = self.agents.get_mut(&id) {
            previous_level = agent.autonomy_level;
            if agent.autonomy_level != 0 {
                agent.autonomy_level = 0;
            }

            match agent.state {
                AgentState::Running | AgentState::Paused | AgentState::Starting => {
                    if let Ok(next) = transition_state(agent.state, AgentState::Stopping) {
                        agent.state = next;
                    }
                    if let Ok(next) = transition_state(agent.state, AgentState::Stopped) {
                        agent.state = next;
                    } else {
                        agent.state = AgentState::Stopped;
                    }
                }
                _ => {}
            }
        }

        if previous_level != 0 {
            let _ = self.audit_trail.append_event(
                id,
                EventType::UserAction,
                json!({
                    "event_kind": "autonomy.level_changed",
                    "agent_id": id,
                    "previous_level": previous_level,
                    "new_level": 0,
                    "reason": reason,
                }),
            );
        }

        AgentError::FuelViolation {
            violation,
            reason: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Supervisor;
    use crate::errors::AgentError;
    use crate::fuel_hardening::FuelViolation;
    use crate::lifecycle::AgentState;
    use crate::manifest::AgentManifest;

    fn sample_manifest(fuel_budget: u64) -> AgentManifest {
        AgentManifest {
            name: "my-social-poster".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["web.search".to_string(), "llm.query".to_string()],
            fuel_budget,
            schedule: None,
            llm_model: Some("ollama/llama3".to_string()),
            autonomy_level: None,
            fuel_period_id: Some("2026-03".to_string()),
            monthly_fuel_cap: Some(10_000),
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
        assert!(matches!(
            restart_result,
            Err(AgentError::FuelViolation {
                violation: FuelViolation::OverMonthlyCap,
                ..
            })
        ));
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
    fn test_exhaustion_downgrades_autonomy_and_emits_event() {
        let mut supervisor = Supervisor::new();
        let mut manifest = sample_manifest(1);
        manifest.autonomy_level = Some(3);
        manifest.monthly_fuel_cap = Some(1);

        let id = supervisor
            .start_agent(manifest)
            .expect("agent should start with one unit");

        let restart_result = supervisor.restart_agent(id);
        assert!(matches!(
            restart_result,
            Err(AgentError::FuelViolation {
                violation: FuelViolation::OverMonthlyCap,
                ..
            })
        ));

        let agent = supervisor
            .get_agent(id)
            .expect("agent should still be tracked after violation");
        assert_eq!(agent.autonomy_level, 0);

        let found_event = supervisor.audit_trail().events().iter().any(|event| {
            event
                .payload
                .get("event_kind")
                .and_then(|value| value.as_str())
                == Some("autonomy.level_changed")
        });
        assert!(found_event);
    }
}
