use crate::audit::{AuditTrail, EventType};
use crate::config::{config_path, load_config_from_path};
use crate::errors::AgentError;
use crate::kill_gates::{GateStatus, KillGateConfig};
use crate::lifecycle::{transition_state, AgentState};
use crate::manifest::AgentManifest;
use crate::safety_supervisor::{
    default_thresholds, KpiKind, OperatingMode, SafetyAction, SafetySupervisor,
};
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
    safety_supervisor: SafetySupervisor,
    operation_count: HashMap<AgentId, u64>,
    error_count: HashMap<AgentId, u64>,
}

impl Supervisor {
    pub fn new() -> Self {
        let kill_gate_config = Self::load_kill_gate_config();
        Self {
            agents: HashMap::new(),
            audit_trail: AuditTrail::new(),
            safety_supervisor: SafetySupervisor::with_kill_gates_config(
                default_thresholds(),
                10,
                &kill_gate_config,
            ),
            operation_count: HashMap::new(),
            error_count: HashMap::new(),
        }
    }

    fn load_kill_gate_config() -> KillGateConfig {
        let path = config_path();
        if !path.exists() {
            return KillGateConfig::default();
        }

        match load_config_from_path(path.as_path()) {
            Ok(config) => KillGateConfig {
                screen_poster_freeze_threshold: f64::from(
                    config.kill_gates.screen_poster_freeze_bps,
                ) / 100.0,
                screen_poster_halt_threshold: f64::from(config.kill_gates.screen_poster_halt_bps)
                    / 100.0,
                mutation_freeze_threshold: f64::from(config.kill_gates.mutation_freeze_signal),
                mutation_halt_threshold: if config.kill_gates.mutation_halt_signal == u32::MAX {
                    f64::INFINITY
                } else {
                    f64::from(config.kill_gates.mutation_halt_signal)
                },
                cluster_freeze_threshold: f64::from(config.kill_gates.cluster_freeze_signal),
                cluster_halt_threshold: if config.kill_gates.cluster_halt_signal == u32::MAX {
                    f64::INFINITY
                } else {
                    f64::from(config.kill_gates.cluster_halt_signal)
                },
                bft_freeze_threshold: if config.kill_gates.bft_freeze_signal == u32::MAX {
                    f64::INFINITY
                } else {
                    f64::from(config.kill_gates.bft_freeze_signal)
                },
                bft_halt_threshold: f64::from(config.kill_gates.bft_halt_signal),
            },
            Err(_) => KillGateConfig::default(),
        }
    }

    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let id = Uuid::new_v4();
        let mut handle = AgentHandle {
            id,
            remaining_fuel: manifest.fuel_budget,
            manifest,
            state: AgentState::Created,
        };

        handle.state = transition_state(handle.state, AgentState::Starting)?;
        Self::consume_fuel(&mut handle)?;
        handle.state = transition_state(handle.state, AgentState::Running)?;

        self.agents.insert(id, handle);
        self.track_operation(id, true);
        self.run_heartbeat(id)?;

        Ok(id)
    }

    pub fn stop_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let result = {
            let handle = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")));

            match handle {
                Ok(handle) => match handle.state {
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
                },
                Err(error) => Err(error),
            }
        };

        self.finalize_operation(id, result)
    }

    pub fn pause_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let result = {
            let handle = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")));

            match handle {
                Ok(handle) => match handle.state {
                    AgentState::Running => {
                        handle.state = transition_state(handle.state, AgentState::Paused)?;
                        Ok(())
                    }
                    AgentState::Paused => Ok(()),
                    _ => Err(AgentError::InvalidTransition {
                        from: handle.state,
                        to: AgentState::Paused,
                    }),
                },
                Err(error) => Err(error),
            }
        };

        self.finalize_operation(id, result)
    }

    pub fn resume_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let result = {
            let handle = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")));

            match handle {
                Ok(handle) => match handle.state {
                    AgentState::Paused => {
                        handle.state = transition_state(handle.state, AgentState::Running)?;
                        Ok(())
                    }
                    AgentState::Running => Ok(()),
                    _ => Err(AgentError::InvalidTransition {
                        from: handle.state,
                        to: AgentState::Running,
                    }),
                },
                Err(error) => Err(error),
            }
        };

        self.finalize_operation(id, result)
    }

    pub fn restart_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let result = {
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
        };

        self.finalize_operation(id, result)
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

    pub fn safety_mode(&self, id: AgentId) -> OperatingMode {
        self.safety_supervisor.mode_for_agent(id)
    }

    pub fn heartbeat(&mut self, id: AgentId) -> Result<(), AgentError> {
        self.run_heartbeat(id)
    }

    pub fn record_tool_call(&mut self, id: AgentId) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        self.track_operation(id, true);
        let readings = self.collect_kpi_readings(id);
        let action = self
            .safety_supervisor
            .observe_tool_call(id, &readings, &mut self.audit_trail);
        self.apply_safety_action(id, action)
    }

    pub fn record_workflow_node_completion(&mut self, id: AgentId) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        self.track_operation(id, true);
        let readings = self.collect_kpi_readings(id);
        let action = self.safety_supervisor.observe_workflow_node_completion(
            id,
            &readings,
            &mut self.audit_trail,
        );
        self.apply_safety_action(id, action)
    }

    pub fn record_llm_metrics(
        &mut self,
        id: AgentId,
        latency_ms: u64,
        governance_overhead_pct: f64,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        self.track_operation(id, true);
        let action = self.safety_supervisor.observe_llm_response(
            id,
            latency_ms,
            governance_overhead_pct,
            &mut self.audit_trail,
        );
        self.apply_safety_action(id, action)
    }

    pub fn record_subsystem_metric(
        &mut self,
        id: AgentId,
        kind: KpiKind,
        value: f64,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        let action = self
            .safety_supervisor
            .heartbeat(id, &[(kind, value)], &mut self.audit_trail);
        self.apply_safety_action(id, action)
    }

    pub fn subsystem_gate_status(&self, subsystem: &str) -> Option<GateStatus> {
        self.safety_supervisor.kill_gate_status(subsystem)
    }

    pub fn manual_freeze_subsystem(
        &mut self,
        id: AgentId,
        subsystem: &str,
        operator_id: &str,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        self.safety_supervisor
            .manual_freeze_subsystem(subsystem, operator_id, id, &mut self.audit_trail)
            .map_err(|error| AgentError::SupervisorError(error.to_string()))?;
        Ok(())
    }

    pub fn manual_halt_subsystem(
        &mut self,
        id: AgentId,
        subsystem: &str,
        operator_id: &str,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        let action = self
            .safety_supervisor
            .manual_halt_subsystem(subsystem, operator_id, id, &mut self.audit_trail)
            .map_err(|error| AgentError::SupervisorError(error.to_string()))?;
        self.apply_safety_action(id, action)
    }

    pub fn manual_unfreeze_subsystem(
        &mut self,
        id: AgentId,
        subsystem: &str,
        operator_id: &str,
        hitl_tier: u8,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        self.safety_supervisor
            .manual_unfreeze_subsystem(subsystem, operator_id, hitl_tier, id, &mut self.audit_trail)
            .map_err(|error| AgentError::SupervisorError(error.to_string()))?;
        Ok(())
    }

    pub fn manual_halt_agent(
        &mut self,
        id: AgentId,
        operator_id: &str,
        reason: &str,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }

        let _ = self.audit_trail.append_event(
            id,
            EventType::Error,
            json!({
                "event_kind": "killgate.halted",
                "subsystem": "agent_runtime",
                "reason": reason,
                "by": operator_id,
            }),
        );

        let action = self.safety_supervisor.force_halt(
            id,
            format!("manual override halt by {operator_id}: {reason}"),
            &mut self.audit_trail,
        );
        self.apply_safety_action(id, action)
    }

    fn finalize_operation(
        &mut self,
        id: AgentId,
        result: Result<(), AgentError>,
    ) -> Result<(), AgentError> {
        if self.agents.contains_key(&id) {
            self.track_operation(id, result.is_ok());
            if result.is_ok() {
                self.run_heartbeat(id)?;
            }
        }

        result
    }

    fn track_operation(&mut self, id: AgentId, success: bool) {
        let operation_entry = self.operation_count.entry(id).or_insert(0);
        *operation_entry = operation_entry.saturating_add(1);

        if !success {
            let error_entry = self.error_count.entry(id).or_insert(0);
            *error_entry = error_entry.saturating_add(1);
        }
    }

    fn collect_kpi_readings(&self, id: AgentId) -> Vec<(KpiKind, f64)> {
        let Some(agent) = self.agents.get(&id) else {
            return Vec::new();
        };

        let budget = agent.manifest.fuel_budget.max(1);
        let spent = budget.saturating_sub(agent.remaining_fuel);
        let budget_ratio_pct = (spent as f64 * 100.0) / budget as f64;

        let operations = self.operation_count.get(&id).copied().unwrap_or(0).max(1);
        let errors = self.error_count.get(&id).copied().unwrap_or(0);
        let error_rate_pct = (errors as f64 * 100.0) / operations as f64;

        let audit_integrity = if self.audit_trail.verify_integrity() {
            0.0
        } else {
            1.0
        };

        vec![
            (KpiKind::GovernanceOverhead, 0.0),
            (KpiKind::LlmLatency, 0.0),
            (KpiKind::AuditChainIntegrity, audit_integrity),
            (KpiKind::FuelBurnRate, budget_ratio_pct),
            (KpiKind::AgentErrorRate, error_rate_pct),
            (KpiKind::BudgetCompliance, budget_ratio_pct),
            (KpiKind::BanRate, 0.0),
            (KpiKind::ReplayMismatch, 0.0),
            (KpiKind::Divergence, 0.0),
            (KpiKind::QuorumInvariant, 0.0),
        ]
    }

    fn run_heartbeat(&mut self, id: AgentId) -> Result<(), AgentError> {
        let readings = self.collect_kpi_readings(id);
        let action = self
            .safety_supervisor
            .heartbeat(id, &readings, &mut self.audit_trail);
        self.apply_safety_action(id, action)
    }

    fn apply_safety_action(&mut self, id: AgentId, action: SafetyAction) -> Result<(), AgentError> {
        match action {
            SafetyAction::Continue => Ok(()),
            SafetyAction::Degraded { reason } => {
                let _ = self.audit_trail.append_event(
                    id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "safety.degraded_notice",
                        "agent_id": id,
                        "reason": reason,
                    }),
                );
                Ok(())
            }
            SafetyAction::Halted { reason, report_id } => {
                if let Some(agent) = self.agents.get_mut(&id) {
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

                Err(AgentError::SupervisorError(format!(
                    "safety supervisor halted agent '{id}': {reason} (report_id={report_id})"
                )))
            }
        }
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
    use crate::safety_supervisor::OperatingMode;

    fn sample_manifest(fuel_budget: u64) -> AgentManifest {
        AgentManifest {
            name: "my-social-poster".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["web.search".to_string(), "llm.query".to_string()],
            fuel_budget,
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
    fn test_supervisor_halts_after_three_consecutive_critical_llm_metrics() {
        let mut supervisor = Supervisor::new();
        let id = supervisor
            .start_agent(sample_manifest(100))
            .expect("agent should start");

        let _ = supervisor.record_llm_metrics(id, 20_000, 0.0);
        let _ = supervisor.record_llm_metrics(id, 20_000, 0.0);
        let third = supervisor.record_llm_metrics(id, 20_000, 0.0);

        assert!(matches!(third, Err(AgentError::SupervisorError(_))));
        assert!(matches!(
            supervisor.safety_mode(id),
            OperatingMode::Halted(_)
        ));

        let halted = supervisor
            .get_agent(id)
            .expect("agent should still exist for inspection");
        assert_eq!(halted.state, AgentState::Stopped);
    }
}
