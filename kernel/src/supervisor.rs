use crate::audit::{AuditTrail, EventType};
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
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
    pub autonomy_guard: AutonomyGuard,
    pub consent_runtime: ConsentRuntime,
    pub autonomy_level: u8,
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
        let autonomy_level = AutonomyLevel::from_manifest(manifest.autonomy_level);
        let consent_runtime = ConsentRuntime::from_manifest(
            manifest.consent_policy_path.as_deref(),
            manifest.requester_id.as_deref(),
            manifest.name.as_str(),
        )?;
        let period_id = BudgetPeriodId::new(
            manifest
                .fuel_period_id
                .clone()
                .unwrap_or_else(|| "period.default".to_string()),
        );
        let monthly_cap = manifest.monthly_fuel_cap.unwrap_or(manifest.fuel_budget);

        let handle = AgentHandle {
            id,
            manifest,
            autonomy_guard: AutonomyGuard::new(autonomy_level),
            consent_runtime,
            autonomy_level: autonomy_level_numeric(autonomy_level),
            state: AgentState::Created,
            remaining_fuel: monthly_cap.min(u64::MAX),
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
            EventType::StateChange,
            json!({
                "event": "autonomy.level_initialized",
                "level": autonomy_level.as_str(),
            }),
        );

        {
            let entry = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            entry.state = transition_state(entry.state, AgentState::Starting)?;
        }
        self.consume_fuel(id, "supervisor.start")?;

        let entry = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        entry.state = transition_state(entry.state, AgentState::Running)?;
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
        let state = self
            .agents
            .get(&id)
            .map(|agent| agent.state)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        if matches!(state, AgentState::Running | AgentState::Paused) {
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
            .consent_runtime
            .enforce_operation(
                GovernedOperation::ToolCall,
                id,
                b"supervisor.tool_call",
                &mut self.audit_trail,
            )
            .map_err(AgentError::from)?;
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
            .consent_runtime
            .enforce_operation(
                GovernedOperation::MultiAgentOrchestrate,
                id,
                b"supervisor.multi_agent",
                &mut self.audit_trail,
            )
            .map_err(AgentError::from)?;
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
            .consent_runtime
            .enforce_operation(
                GovernedOperation::SelfMutationApply,
                id,
                b"supervisor.self_modification",
                &mut self.audit_trail,
            )
            .map_err(AgentError::from)?;
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
            .consent_runtime
            .enforce_operation(
                GovernedOperation::DistributedEnable,
                id,
                b"supervisor.distributed",
                &mut self.audit_trail,
            )
            .map_err(AgentError::from)?;
        handle
            .autonomy_guard
            .require_distributed(id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn require_consent(
        &mut self,
        id: AgentId,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .consent_runtime
            .enforce_operation(operation, id, payload, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn approve_consent(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .consent_runtime
            .approve(request_id, approver_id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn deny_consent(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle
            .consent_runtime
            .deny(request_id, approver_id, &mut self.audit_trail)
            .map_err(AgentError::from)
    }

    pub fn pending_consent_requests(
        &self,
        id: AgentId,
    ) -> Result<Vec<ApprovalRequest>, AgentError> {
        let handle = self
            .agents
            .get(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        Ok(handle.consent_runtime.pending_requests())
    }

    fn consume_fuel(&mut self, id: AgentId, reason: &str) -> Result<(), AgentError> {
        let model_name = {
            let handle = self
                .agents
                .get_mut(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            if handle.remaining_fuel == 0 {
                return Err(self.apply_fuel_violation(
                    id,
                    FuelViolation::OverMonthlyCap,
                    "runtime fuel exhausted",
                ));
            }
            handle.remaining_fuel -= 1;
            handle
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

        if let Some(handle) = self.agents.get_mut(&id) {
            handle.autonomy_level = 0;
            handle.remaining_fuel = 0;
            handle.autonomy_guard.downgrade(
                id,
                AutonomyLevel::L0,
                "fuel_violation",
                reason,
                &mut self.audit_trail,
            );
        }

        AgentError::FuelViolation {
            violation,
            reason: reason.to_string(),
        }
    }
}

fn autonomy_level_numeric(level: AutonomyLevel) -> u8 {
    match level {
        AutonomyLevel::L0 => 0,
        AutonomyLevel::L1 => 1,
        AutonomyLevel::L2 => 2,
        AutonomyLevel::L3 => 3,
        AutonomyLevel::L4 => 4,
        AutonomyLevel::L5 => 5,
    }
}
