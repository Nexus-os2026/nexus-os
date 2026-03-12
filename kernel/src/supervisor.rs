use crate::audit::{AuditTrail, EventType};
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
use crate::errors::AgentError;
use crate::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelViolation,
};
use crate::kill_gates::KillGateError;
use crate::lifecycle::{transition_state, AgentState};
use crate::manifest::AgentManifest;
use crate::permissions::{
    CapabilityRequest, PermissionCategory, PermissionHistoryEntry, PermissionManager,
};
use crate::policy_engine::PolicyEngine;
use crate::safety_supervisor::{KpiKind, SafetyAction, SafetySupervisor};
use crate::speculative::{SimulationResult, SpeculativeEngine};
use crate::time_machine::TimeMachine;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
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

#[derive(Debug)]
pub struct Supervisor {
    agents: HashMap<AgentId, AgentHandle>,
    fuel_ledgers: HashMap<AgentId, AgentFuelLedger>,
    audit_trail: AuditTrail,
    safety_supervisor: SafetySupervisor,
    speculative_engine: SpeculativeEngine,
    permission_manager: PermissionManager,
    policy_engine: PolicyEngine,
    time_machine: TimeMachine,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            fuel_ledgers: HashMap::new(),
            audit_trail: AuditTrail::new(),
            safety_supervisor: SafetySupervisor::default(),
            speculative_engine: SpeculativeEngine::new(),
            permission_manager: PermissionManager::new(),
            policy_engine: PolicyEngine::default(),
            time_machine: TimeMachine::default(),
        }
    }

    /// Create a supervisor with a policy engine loaded from the given directory.
    pub fn with_policy_dir(dir: impl Into<PathBuf>) -> Self {
        let mut engine = PolicyEngine::new(dir);
        let _ = engine.load_policies();
        Self {
            agents: HashMap::new(),
            fuel_ledgers: HashMap::new(),
            audit_trail: AuditTrail::new(),
            safety_supervisor: SafetySupervisor::default(),
            speculative_engine: SpeculativeEngine::new(),
            permission_manager: PermissionManager::new(),
            policy_engine: engine,
            time_machine: TimeMachine::default(),
        }
    }

    /// Replace the policy engine (useful for testing or runtime reload).
    pub fn set_policy_engine(&mut self, engine: PolicyEngine) {
        self.policy_engine = engine;
    }

    /// Reload policies from the configured directory.
    pub fn reload_policies(&mut self) -> Result<usize, crate::policy_engine::PolicyError> {
        self.policy_engine.load_policies()
    }

    /// Access the policy engine.
    pub fn policy_engine(&self) -> &PolicyEngine {
        &self.policy_engine
    }

    /// Access the time machine.
    pub fn time_machine(&self) -> &TimeMachine {
        &self.time_machine
    }

    /// Mutable access to the time machine.
    pub fn time_machine_mut(&mut self) -> &mut TimeMachine {
        &mut self.time_machine
    }

    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let id = Uuid::new_v4();
        let autonomy_level = AutonomyLevel::from_manifest(manifest.autonomy_level);
        let mut consent_runtime = ConsentRuntime::from_manifest(
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

        // Attach Cedar policy engine to the consent runtime so it can
        // pre-check policies before falling back to hardcoded tiers.
        if self.policy_engine.has_policies() {
            consent_runtime.set_cedar_engine(self.policy_engine.clone());
        }

        let handle = AgentHandle {
            id,
            manifest,
            autonomy_guard: AutonomyGuard::new(autonomy_level),
            consent_runtime,
            autonomy_level: autonomy_level_numeric(autonomy_level),
            state: AgentState::Created,
            remaining_fuel: monthly_cap,
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

        self.audit_trail.append_event(
            id,
            EventType::StateChange,
            json!({
                "event_kind": "fuel.period_set",
                "agent_id": id,
                "period": period_id.0,
                "cap_units": monthly_cap,
                "spent_units": 0,
            }),
        )?;
        self.audit_trail.append_event(
            id,
            EventType::StateChange,
            json!({
                "event": "autonomy.level_initialized",
                "level": autonomy_level.as_str(),
            }),
        )?;

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

    pub fn subsystem_gate_status(&self, subsystem: &str) -> Option<crate::kill_gates::GateStatus> {
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
            .map_err(|error: KillGateError| AgentError::SupervisorError(error.to_string()))?;
        Ok(())
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
            .map_err(|error: KillGateError| AgentError::SupervisorError(error.to_string()))?;
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

        self.audit_trail.append_event(
            id,
            EventType::Error,
            json!({
                "event_kind": "killgate.halted",
                "subsystem": "agent_runtime",
                "reason": reason,
                "by": operator_id,
            }),
        )?;
        let action = self.safety_supervisor.force_halt(
            id,
            format!("manual override halt by {operator_id}: {reason}"),
            &mut self.audit_trail,
        );
        self.apply_safety_action(id, action)
    }

    pub fn require_tool_call(&mut self, id: AgentId) -> Result<(), AgentError> {
        let policy_ref = if self.policy_engine.has_policies() {
            Some(self.policy_engine.clone())
        } else {
            None
        };
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
            .map_err(AgentError::from)?;
        // Policy engine autonomy override (only when policies exist)
        if let Some(ref pe) = policy_ref {
            let handle = self.agents.get_mut(&id).unwrap();
            let default_min = handle.autonomy_guard.level();
            handle
                .autonomy_guard
                .require_level_with_policy(
                    id,
                    &mut self.audit_trail,
                    default_min,
                    "tool_call",
                    Some(pe),
                )
                .map_err(AgentError::from)?;
        }
        Ok(())
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

    /// Request consent with automatic speculative simulation for Tier2+ operations.
    ///
    /// When the operation requires approval and would normally block, the engine
    /// forks a shadow state, simulates the action, and attaches the preview to
    /// the approval request so the human reviewer sees predicted outcomes.
    pub fn require_consent_with_simulation(
        &mut self,
        id: AgentId,
        operation: GovernedOperation,
        payload: &[u8],
    ) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        let tier = handle
            .consent_runtime
            .policy_engine()
            .required_tier(operation);

        let result =
            handle
                .consent_runtime
                .enforce_operation(operation, id, payload, &mut self.audit_trail);

        match result {
            Err(crate::consent::ConsentError::ApprovalRequired {
                request_id,
                operation: op,
                required_tier,
            }) => {
                // Auto-simulate for Tier2+ operations
                if SpeculativeEngine::should_simulate(tier) {
                    let handle = self.agents.get(&id).unwrap();
                    let snapshot = self.speculative_engine.fork_state(
                        id,
                        handle.remaining_fuel,
                        AutonomyLevel::from_numeric(handle.autonomy_level).unwrap_or_default(),
                        handle.manifest.capabilities.clone(),
                        self.audit_trail.events().len(),
                    );
                    let sim_result = self.speculative_engine.simulate(
                        &snapshot,
                        op,
                        required_tier,
                        payload,
                        &mut self.audit_trail,
                    );
                    self.speculative_engine
                        .attach_to_request(&request_id, sim_result.simulation_id);
                }
                Err(AgentError::ApprovalRequired { request_id })
            }
            Err(e) => Err(AgentError::from(e)),
            Ok(()) => {
                // Action was auto-approved — commit any prior simulation if one existed
                Ok(())
            }
        }
    }

    /// Approve consent and commit the associated simulation.
    pub fn approve_consent_with_simulation(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        self.approve_consent(id, request_id, approver_id)?;
        self.speculative_engine.commit(request_id);
        Ok(())
    }

    /// Deny consent and rollback the associated simulation.
    pub fn deny_consent_with_simulation(
        &mut self,
        id: AgentId,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), AgentError> {
        self.deny_consent(id, request_id, approver_id)?;
        self.speculative_engine
            .rollback(request_id, &mut self.audit_trail);
        Ok(())
    }

    /// Get the simulation preview for a pending approval request.
    pub fn simulation_for_request(&self, request_id: &str) -> Option<&SimulationResult> {
        self.speculative_engine.get_for_request(request_id)
    }

    /// List all pending speculative simulations.
    pub fn pending_simulations(&self) -> Vec<(&str, &SimulationResult)> {
        self.speculative_engine.pending_simulations()
    }

    // ── Permission Dashboard API ──

    /// Get all permission categories for an agent (for the permission dashboard).
    pub fn get_agent_permissions(
        &self,
        id: AgentId,
    ) -> Result<Vec<PermissionCategory>, AgentError> {
        let handle = self
            .agents
            .get(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        Ok(self
            .permission_manager
            .get_permissions(id, &handle.manifest))
    }

    /// Update a single permission for an agent — modifies real capabilities.
    pub fn update_agent_permission(
        &mut self,
        id: AgentId,
        capability_key: &str,
        enabled: bool,
        changed_by: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError> {
        let manifest = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.manifest.clone()
        };

        let updated = self.permission_manager.update_permission(
            id,
            &manifest,
            capability_key,
            enabled,
            changed_by,
            reason,
            &mut self.audit_trail,
        )?;

        // Apply updated manifest to the agent handle
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.manifest = updated;
        Ok(())
    }

    /// Bulk update permissions for an agent.
    pub fn bulk_update_agent_permissions(
        &mut self,
        id: AgentId,
        updates: &[(String, bool)],
        changed_by: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError> {
        let manifest = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.manifest.clone()
        };

        let updated = self.permission_manager.bulk_update_permissions(
            id,
            &manifest,
            updates,
            changed_by,
            reason,
            &mut self.audit_trail,
        )?;

        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.manifest = updated;
        Ok(())
    }

    /// Get permission change history for an agent.
    pub fn get_permission_history(
        &self,
        id: AgentId,
    ) -> Result<Vec<PermissionHistoryEntry>, AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }
        Ok(self.permission_manager.get_history(id))
    }

    /// Get pending capability requests for an agent.
    pub fn get_capability_requests(
        &self,
        id: AgentId,
    ) -> Result<Vec<CapabilityRequest>, AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }
        Ok(self.permission_manager.get_capability_requests(id))
    }

    /// Lock a capability so users cannot toggle it.
    pub fn lock_agent_capability(
        &mut self,
        id: AgentId,
        capability_key: &str,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }
        self.permission_manager
            .lock_capability(id, capability_key, &mut self.audit_trail);
        Ok(())
    }

    /// Unlock a capability for user toggling.
    pub fn unlock_agent_capability(
        &mut self,
        id: AgentId,
        capability_key: &str,
    ) -> Result<(), AgentError> {
        if !self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' not found"
            )));
        }
        self.permission_manager
            .unlock_capability(id, capability_key, &mut self.audit_trail);
        Ok(())
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

    fn apply_safety_action(&mut self, id: AgentId, action: SafetyAction) -> Result<(), AgentError> {
        match action {
            SafetyAction::Continue => Ok(()),
            SafetyAction::Degraded { reason } => {
                self.audit_trail.append_event(
                    id,
                    EventType::UserAction,
                    json!({
                        "event_kind": "safety.degraded_notice",
                        "agent_id": id,
                        "reason": reason,
                    }),
                )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::GovernedOperation;
    use crate::manifest::AgentManifest;

    fn test_manifest() -> AgentManifest {
        AgentManifest {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec!["llm.query".to_string(), "fs.read".to_string()],
            fuel_budget: 10000,
            autonomy_level: Some(2),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    fn setup_supervisor_with_agent() -> (Supervisor, AgentId) {
        let mut sup = Supervisor::new();
        let id = sup.start_agent(test_manifest()).unwrap();
        (sup, id)
    }

    #[test]
    fn require_consent_with_simulation_tier1_no_simulation() {
        let (mut sup, id) = setup_supervisor_with_agent();
        // ToolCall defaults to Tier1 → auto-approved, no simulation
        let result = sup.require_consent_with_simulation(id, GovernedOperation::ToolCall, b"test");
        assert!(result.is_ok());
        assert!(sup.pending_simulations().is_empty());
    }

    #[test]
    fn require_consent_with_simulation_tier2_creates_simulation() {
        let (mut sup, id) = setup_supervisor_with_agent();
        // TerminalCommand defaults to Tier2 → requires approval → triggers simulation
        let result = sup.require_consent_with_simulation(
            id,
            GovernedOperation::TerminalCommand,
            b"rm -rf /tmp/test",
        );
        // Should fail with ApprovalRequired
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::ApprovalRequired { request_id } => {
                // Simulation should be attached to this request
                let sim = sup.simulation_for_request(&request_id);
                assert!(sim.is_some(), "simulation should be attached to request");
                let sim = sim.unwrap();
                assert_eq!(sim.operation, GovernedOperation::TerminalCommand);
                assert!(!sim.predicted_changes.is_empty());
                assert!(sim.resource_impact.fuel_cost > 0);
            }
            other => panic!("expected ApprovalRequired, got: {other:?}"),
        }
    }

    #[test]
    fn require_consent_with_simulation_tier3_creates_critical_simulation() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let result = sup.require_consent_with_simulation(
            id,
            GovernedOperation::SelfMutationApply,
            b"mutation payload",
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::ApprovalRequired { request_id } => {
                let sim = sup.simulation_for_request(&request_id).unwrap();
                assert_eq!(
                    sim.risk_level,
                    crate::speculative::RiskLevel::Critical,
                    "Tier3 should produce Critical risk"
                );
            }
            other => panic!("expected ApprovalRequired, got: {other:?}"),
        }
    }

    #[test]
    fn approve_consent_with_simulation_commits() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let result = sup.require_consent_with_simulation(
            id,
            GovernedOperation::TerminalCommand,
            b"echo hello",
        );
        let request_id = match result.unwrap_err() {
            AgentError::ApprovalRequired { request_id } => request_id,
            other => panic!("expected ApprovalRequired, got: {other:?}"),
        };

        assert!(sup.simulation_for_request(&request_id).is_some());

        // Approve the consent
        sup.approve_consent_with_simulation(id, &request_id, "admin")
            .unwrap();

        // Simulation should be cleaned up
        assert!(sup.simulation_for_request(&request_id).is_none());
    }

    #[test]
    fn deny_consent_with_simulation_rollbacks() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let result = sup.require_consent_with_simulation(
            id,
            GovernedOperation::SocialPostPublish,
            b"post content",
        );
        let request_id = match result.unwrap_err() {
            AgentError::ApprovalRequired { request_id } => request_id,
            other => panic!("expected ApprovalRequired, got: {other:?}"),
        };

        assert!(sup.simulation_for_request(&request_id).is_some());

        // Deny the consent
        sup.deny_consent_with_simulation(id, &request_id, "admin")
            .unwrap();

        // Simulation should be cleaned up
        assert!(sup.simulation_for_request(&request_id).is_none());
    }

    #[test]
    fn pending_simulations_lists_multiple() {
        let (mut sup, id) = setup_supervisor_with_agent();

        // Create two pending simulations
        let _ =
            sup.require_consent_with_simulation(id, GovernedOperation::TerminalCommand, b"cmd1");
        let _ =
            sup.require_consent_with_simulation(id, GovernedOperation::SocialPostPublish, b"post");

        let pending = sup.pending_simulations();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn simulation_does_not_modify_agent_fuel() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let fuel_before = sup.agents.get(&id).unwrap().remaining_fuel;

        let _ = sup.require_consent_with_simulation(
            id,
            GovernedOperation::TerminalCommand,
            b"heavy command",
        );

        let fuel_after = sup.agents.get(&id).unwrap().remaining_fuel;
        assert_eq!(
            fuel_before, fuel_after,
            "simulation must not consume real fuel"
        );
    }

    #[test]
    fn simulation_audit_trail_records_event() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let events_before = sup.audit_trail.events().len();

        let _ =
            sup.require_consent_with_simulation(id, GovernedOperation::TerminalCommand, b"test");

        // Simulation should add audit events (consent request + simulation record)
        assert!(
            sup.audit_trail.events().len() > events_before,
            "simulation should append audit events"
        );
    }

    // ── PolicyEngine integration tests ──

    #[test]
    fn custom_policy_overrides_default_consent_tier() {
        use crate::policy_engine::{Policy, PolicyConditions, PolicyEffect, PolicyEngine};

        // TerminalCommand defaults to Tier2 (requires approval).
        // Create a policy that explicitly allows it for all agents.
        let allow_terminal = Policy {
            policy_id: "allow-terminal".to_string(),
            description: String::new(),
            effect: PolicyEffect::Allow,
            principal: "*".to_string(),
            action: "terminal_command".to_string(),
            resource: "*".to_string(),
            priority: 10,
            conditions: PolicyConditions::default(),
        };

        let pe = PolicyEngine::with_policies(vec![allow_terminal]);
        let mut sup = Supervisor::new();
        sup.set_policy_engine(pe);
        let id = sup.start_agent(test_manifest()).unwrap();

        // With the policy, TerminalCommand should be auto-allowed (no approval needed).
        let result = sup.require_consent(id, GovernedOperation::TerminalCommand, b"echo hello");
        assert!(
            result.is_ok(),
            "policy should override default Tier2 to Allow: {result:?}"
        );
    }

    #[test]
    fn custom_policy_denies_normally_allowed_operation() {
        use crate::policy_engine::{Policy, PolicyConditions, PolicyEffect, PolicyEngine};

        // ToolCall defaults to Tier1 (auto-approved).
        // Create a policy that explicitly denies it.
        let deny_tools = Policy {
            policy_id: "deny-tools".to_string(),
            description: "tools blocked by enterprise policy".to_string(),
            effect: PolicyEffect::Deny,
            principal: "*".to_string(),
            action: "tool_call".to_string(),
            resource: "*".to_string(),
            priority: 10,
            conditions: PolicyConditions::default(),
        };

        let pe = PolicyEngine::with_policies(vec![deny_tools]);
        let mut sup = Supervisor::new();
        sup.set_policy_engine(pe);
        let id = sup.start_agent(test_manifest()).unwrap();

        let result = sup.require_consent(id, GovernedOperation::ToolCall, b"test");
        assert!(
            result.is_err(),
            "policy should deny normally auto-approved ToolCall"
        );
    }

    #[test]
    fn no_custom_policy_falls_back_to_defaults() {
        // Empty policy engine — should behave identically to no engine.
        let pe = PolicyEngine::with_policies(vec![]);
        let mut sup = Supervisor::new();
        sup.set_policy_engine(pe);
        let id = sup.start_agent(test_manifest()).unwrap();

        // ToolCall at Tier1 → auto-approved (default behavior).
        let result = sup.require_consent(id, GovernedOperation::ToolCall, b"test");
        assert!(result.is_ok(), "empty engine should fall back to defaults");

        // TerminalCommand at Tier2 → requires approval (default behavior).
        let result = sup.require_consent(id, GovernedOperation::TerminalCommand, b"cmd");
        assert!(
            result.is_err(),
            "empty engine should preserve default Tier2 approval requirement"
        );
    }

    #[test]
    fn custom_policy_requires_higher_autonomy() {
        use crate::policy_engine::{Policy, PolicyConditions, PolicyEffect, PolicyEngine};

        // Create a policy that requires min_autonomy_level=3 for tool_call.
        // The allow policy with min_autonomy=3 means agents below L3 won't
        // match, falling through to default deny in the policy engine.
        let strict_tool = Policy {
            policy_id: "strict-tool".to_string(),
            description: String::new(),
            effect: PolicyEffect::Allow,
            principal: "*".to_string(),
            action: "tool_call".to_string(),
            resource: "*".to_string(),
            priority: 10,
            conditions: PolicyConditions {
                min_autonomy_level: Some(3),
                ..PolicyConditions::default()
            },
        };

        let pe = PolicyEngine::with_policies(vec![strict_tool]);
        let mut sup = Supervisor::new();
        sup.set_policy_engine(pe);

        // Agent at L2 — policy condition not met, falls to default deny in
        // the cedar engine, which returns PolicyDenied.
        let id = sup.start_agent(test_manifest()).unwrap();
        let result = sup.require_consent(id, GovernedOperation::ToolCall, b"test");
        assert!(
            result.is_err(),
            "L2 agent should be denied when policy requires min_autonomy_level=3"
        );
    }

    #[test]
    fn policy_engine_reload_picks_up_new_files() {
        let dir = tempfile::tempdir().unwrap();

        let mut sup = Supervisor::with_policy_dir(dir.path());
        assert_eq!(sup.policy_engine().policies().len(), 0);

        // Write a policy file after initialization
        let policy_toml = r#"
policy_id = "runtime-added"
effect = "allow"
principal = "*"
action = "tool_call"
resource = "*"
priority = 50
"#;
        std::fs::write(dir.path().join("new-policy.toml"), policy_toml).unwrap();

        // Reload should pick it up
        let count = sup.reload_policies().unwrap();
        assert_eq!(count, 1);
        assert_eq!(sup.policy_engine().policies()[0].policy_id, "runtime-added");
    }
}
