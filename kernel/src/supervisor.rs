use crate::audit::{AuditTrail, EventType};
use crate::autonomy::{AutonomyGuard, AutonomyLevel};
use crate::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation};
use crate::errors::AgentError;
use crate::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelViolation,
};

/// Maximum fuel cost per action type.  Used by the reserve-then-commit pattern
/// to lock the worst-case amount before execution begins.
pub fn max_fuel_cost(action_type: &str) -> u64 {
    match action_type {
        "llm_inference_local" => 2_000,
        "llm_inference_cloud" => 10_000,
        "filesystem_read" => 200,
        "filesystem_write" => 1_000,
        "network_request" => 2_000,
        "agent_to_agent" => 500,
        "mcp_tool_call" => 5_000,
        "wasm_execution" => 1_000,
        "supervisor.start" | "supervisor.restart" => 10,
        "supervisor.stop" => 10,
        // A2A protocol costs — external agent delegation is expensive
        "a2a_delegate" => 2_000,
        "a2a_discover" => 500,
        "a2a_status_check" => 200,
        // Integration provider costs — messaging vs ticket-creation
        "integration_slack"
        | "integration_teams"
        | "integration_discord"
        | "integration_telegram" => 500,
        "integration_jira" | "integration_github" => 1_000,
        "integration_webhook" => 300,
        _ => 1_000, // Conservative default
    }
}

/// Runtime-configurable supervisor settings.
///
/// Caps on how many L5 (Sovereign) and L6 (Transcendent) agents may be
/// simultaneously active. Defaults are conservative-but-usable on a
/// high-end workstation (62GB RAM, single GPU); raise via env vars for
/// larger hardware.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub max_l5_agents: usize,
    pub max_l6_agents: usize,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            max_l5_agents: 1,
            max_l6_agents: 6,
        }
    }
}

impl SupervisorConfig {
    /// Load from environment variables, falling back to defaults.
    /// `NEXUS_MAX_L5_AGENTS` and `NEXUS_MAX_L6_AGENTS`.
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(val) = std::env::var("NEXUS_MAX_L5_AGENTS") {
            match val.parse::<usize>() {
                Ok(n) => {
                    config.max_l5_agents = n;
                    eprintln!(
                        "[supervisor] config: max_l5_agents = {n} (from NEXUS_MAX_L5_AGENTS)"
                    );
                    if n == 0 {
                        eprintln!("[supervisor] WARNING: max_l5_agents = 0 — no L5 agents can run");
                    } else if n > 100 {
                        eprintln!("[supervisor] WARNING: max_l5_agents = {n} is unusually high");
                    }
                }
                Err(_) => {
                    eprintln!(
                        "[supervisor] WARNING: NEXUS_MAX_L5_AGENTS='{val}' is not a valid number, using default {}",
                        config.max_l5_agents
                    );
                }
            }
        }
        if let Ok(val) = std::env::var("NEXUS_MAX_L6_AGENTS") {
            match val.parse::<usize>() {
                Ok(n) => {
                    config.max_l6_agents = n;
                    eprintln!(
                        "[supervisor] config: max_l6_agents = {n} (from NEXUS_MAX_L6_AGENTS)"
                    );
                    if n == 0 {
                        eprintln!("[supervisor] WARNING: max_l6_agents = 0 — no L6 agents can run");
                    } else if n > 100 {
                        eprintln!("[supervisor] WARNING: max_l6_agents = {n} is unusually high");
                    }
                }
                Err(_) => {
                    eprintln!(
                        "[supervisor] WARNING: NEXUS_MAX_L6_AGENTS='{val}' is not a valid number, using default {}",
                        config.max_l6_agents
                    );
                }
            }
        }
        config
    }
}

/// A fuel reservation held against an agent's balance in the Supervisor.
///
/// The reserved amount is immediately subtracted from the agent's
/// `remaining_fuel`.  Call [`Supervisor::commit_fuel`] to finalise or
/// [`Supervisor::cancel_fuel`] to return the fuel.
#[derive(Debug, Clone)]
pub struct SupervisorFuelReservation {
    pub id: Uuid,
    pub agent_id: AgentId,
    pub reserved_amount: u64,
    pub action_type: String,
}
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

pub type AgentId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Built-in agent managed by Tauri commands (no WASM binary).
    Native,
    /// WASM-sandboxed agent with binary at the given path.
    Wasm { binary_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    pub manifest: AgentManifest,
    pub autonomy_guard: AutonomyGuard,
    pub consent_runtime: ConsentRuntime,
    pub autonomy_level: u8,
    pub state: AgentState,
    pub remaining_fuel: u64,
    pub execution_mode: ExecutionMode,
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
    config: SupervisorConfig,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self::with_config(SupervisorConfig::from_env())
    }

    /// Create a supervisor with an explicit [`SupervisorConfig`].
    ///
    /// Use this in tests when you need deterministic L5/L6 caps. Production
    /// code should use [`Supervisor::new`], which loads caps from env vars.
    pub fn with_config(config: SupervisorConfig) -> Self {
        Self {
            agents: HashMap::new(),
            fuel_ledgers: HashMap::new(),
            audit_trail: AuditTrail::new(),
            safety_supervisor: SafetySupervisor::default(),
            speculative_engine: SpeculativeEngine::new(),
            permission_manager: PermissionManager::new(),
            policy_engine: PolicyEngine::default(),
            time_machine: TimeMachine::default(),
            config,
        }
    }

    /// Create a supervisor with a policy engine loaded from the given directory.
    pub fn with_policy_dir(dir: impl Into<PathBuf>) -> Self {
        let mut engine = PolicyEngine::new(dir);
        // Best-effort: failure to load policies does not block supervisor creation
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
            config: SupervisorConfig::from_env(),
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

    fn active_sovereign_agents(&self) -> Vec<(&AgentId, &AgentHandle)> {
        self.agents
            .iter()
            .filter(|(_, handle)| {
                handle.autonomy_level == 5
                    && matches!(
                        handle.state,
                        AgentState::Running | AgentState::Starting | AgentState::Paused
                    )
            })
            .collect()
    }

    fn active_transcendent_agents(&self) -> Vec<(&AgentId, &AgentHandle)> {
        self.agents
            .iter()
            .filter(|(_, handle)| {
                handle.autonomy_level == 6
                    && matches!(
                        handle.state,
                        AgentState::Running | AgentState::Starting | AgentState::Paused
                    )
            })
            .collect()
    }

    pub fn start_agent(&mut self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let id = Uuid::new_v4();
        self.start_agent_with_id(id, manifest)
    }

    /// Register an agent in the supervisor without starting it.
    ///
    /// Inserts a new entry into the agent registry with the given initial state.
    /// Does NOT enforce L5/L6 caps — caps are enforced only when transitioning
    /// to Running via `transition_to_running`.
    ///
    /// Returns `Err` if the UUID is already registered (duplicate registration is a bug).
    pub fn register_agent(
        &mut self,
        id: AgentId,
        manifest: AgentManifest,
        initial_state: AgentState,
    ) -> Result<AgentId, AgentError> {
        if self.agents.contains_key(&id) {
            return Err(AgentError::SupervisorError(format!(
                "agent '{id}' is already registered"
            )));
        }

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

        // Determine execution mode: WASM if manifest declares a binary, Native otherwise.
        let execution_mode = if let Some(wasm_path) = manifest
            .capabilities
            .iter()
            .find(|c| c.starts_with("wasm.binary:"))
        {
            let path = wasm_path.strip_prefix("wasm.binary:").unwrap_or_default();
            ExecutionMode::Wasm {
                binary_path: PathBuf::from(path),
            }
        } else {
            ExecutionMode::Native
        };

        let handle = AgentHandle {
            id,
            manifest,
            autonomy_guard: AutonomyGuard::new(autonomy_level),
            consent_runtime,
            autonomy_level: autonomy_level_numeric(autonomy_level),
            state: initial_state,
            remaining_fuel: monthly_cap,
            execution_mode: execution_mode.clone(),
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
                "execution_mode": format!("{execution_mode:?}"),
            }),
        )?;

        Ok(id)
    }

    /// Transition a registered agent to Running state.
    ///
    /// Enforces L5 singleton and L6 cap (max 2) before allowing the transition.
    /// If the agent is already Running, returns Ok (idempotent).
    pub fn transition_to_running(&mut self, id: AgentId) -> Result<(), AgentError> {
        let (autonomy_level, current_state) = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            (
                AutonomyLevel::from_manifest(Some(handle.autonomy_level)),
                handle.state,
            )
        };

        // Idempotent: already running
        if current_state == AgentState::Running {
            return Ok(());
        }

        // L5 cap enforcement (default singleton, configurable via NEXUS_MAX_L5_AGENTS).
        if autonomy_level == AutonomyLevel::L5 {
            let active_l5 = self.active_sovereign_agents();
            let others: Vec<_> = active_l5.iter().filter(|(aid, _)| **aid != id).collect();
            if others.len() >= self.config.max_l5_agents {
                let mut names = others
                    .iter()
                    .map(|(_, handle)| handle.manifest.name.clone())
                    .collect::<Vec<_>>();
                names.sort();
                return Err(AgentError::SupervisorError(format!(
                    "Maximum {} L5 agents allowed. Currently active: {}. Stop one first.",
                    self.config.max_l5_agents,
                    names.join(", ")
                )));
            }
        }

        if autonomy_level == AutonomyLevel::L6 {
            let active_l6 = self.active_transcendent_agents();
            let others: Vec<_> = active_l6.iter().filter(|(aid, _)| **aid != id).collect();
            if others.len() >= self.config.max_l6_agents {
                let mut names = others
                    .iter()
                    .map(|(_, handle)| handle.manifest.name.clone())
                    .collect::<Vec<_>>();
                names.sort();
                return Err(AgentError::SupervisorError(format!(
                    "Maximum {} Transcendent agents allowed. Currently active: {}. Stop one first.",
                    self.config.max_l6_agents,
                    names.join(", ")
                )));
            }
        }

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

        // Create a time machine checkpoint for agent start
        let mut builder = self
            .time_machine
            .begin_checkpoint("agent_lifecycle", Some(id.to_string()));
        builder.record_agent_state(
            &id.to_string(),
            "status",
            json!("not_running"),
            json!("running"),
        );
        let checkpoint = builder.build();
        // Best-effort: failure to record checkpoint does not block agent start
        let _ = self.time_machine.commit_checkpoint(checkpoint);

        Ok(())
    }

    pub fn start_agent_with_id(
        &mut self,
        id: AgentId,
        manifest: AgentManifest,
    ) -> Result<AgentId, AgentError> {
        self.register_agent(id, manifest, AgentState::Created)?;
        self.transition_to_running(id)?;
        Ok(id)
    }

    /// Sole legal entry point for mutating `AgentHandle.state`.
    ///
    /// Wraps the FSM helper in [`crate::lifecycle::transition_state`] and, on
    /// success, commits the new state plus a tamper-evident audit record
    /// (`event_kind = "agent.state_transition"`). Any call-site that touches
    /// `handle.state` directly bypasses audit and causes the UI to drift —
    /// do not add new direct mutations.
    ///
    /// Returns `Err` on illegal FSM transitions or unknown agent ids.
    pub fn transition_agent_state(
        &mut self,
        id: AgentId,
        new_state: AgentState,
        reason: &str,
    ) -> Result<AgentState, AgentError> {
        let old_state = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.state
        };
        let next = transition_state(old_state, new_state)?;
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.state = next;
        // Best-effort audit: a hash-chain failure must not roll back the
        // committed state transition.
        let _ = self.audit_trail.append_event(
            id,
            EventType::UserAction,
            json!({
                "event_kind": "agent.state_transition",
                "agent_id": id.to_string(),
                "old_state": old_state.to_string(),
                "new_state": next.to_string(),
                "reason": reason,
                "forced": false,
            }),
        );
        eprintln!("[supervisor] agent {id} transition {old_state} -> {next} ({reason})");
        Ok(next)
    }

    /// Bypasses the FSM legality check and force-sets the agent state.
    ///
    /// WARNING: this exists **solely** for the safety-halt fallback where a
    /// corrupt or mid-transition state must be crushed to `Stopped` to
    /// contain a runaway agent. It should never be used by lifecycle
    /// commands, Tauri handlers, or time-machine undo/redo — those must go
    /// through [`Supervisor::transition_agent_state`] so FSM guarantees and
    /// the audit trail remain intact.
    pub fn force_transition_agent_state(
        &mut self,
        id: AgentId,
        new_state: AgentState,
        reason: &str,
    ) -> Result<AgentState, AgentError> {
        let old_state = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.state
        };
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.state = new_state;
        let _ = self.audit_trail.append_event(
            id,
            EventType::UserAction,
            json!({
                "event_kind": "agent.state_transition",
                "agent_id": id.to_string(),
                "old_state": old_state.to_string(),
                "new_state": new_state.to_string(),
                "reason": reason,
                "forced": true,
            }),
        );
        eprintln!(
            "[supervisor] agent {id} FORCED transition {old_state} -> {new_state} ({reason})"
        );
        Ok(new_state)
    }

    pub fn stop_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let current_state = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.state
        };
        match current_state {
            AgentState::Running | AgentState::Paused => {
                // Consume fuel for stop operation (symmetric with start)
                self.consume_fuel_units(id, "supervisor.stop", 2)?;
                self.transition_agent_state(id, AgentState::Stopping, "stop")?;
                self.transition_agent_state(id, AgentState::Stopped, "stop")?;

                // Create a time machine checkpoint for agent stop
                let mut builder = self
                    .time_machine
                    .begin_checkpoint("agent_lifecycle", Some(id.to_string()));
                builder.record_agent_state(
                    &id.to_string(),
                    "status",
                    json!("running"),
                    json!("stopped"),
                );
                let checkpoint = builder.build();
                // Best-effort: failure to record checkpoint does not block agent stop
                let _ = self.time_machine.commit_checkpoint(checkpoint);

                Ok(())
            }
            AgentState::Stopping | AgentState::Stopped | AgentState::Destroyed => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: current_state,
                to: AgentState::Stopping,
            }),
        }
    }

    pub fn pause_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let current_state = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.state
        };
        match current_state {
            AgentState::Running => {
                self.transition_agent_state(id, AgentState::Paused, "pause")?;
                Ok(())
            }
            AgentState::Paused => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: current_state,
                to: AgentState::Paused,
            }),
        }
    }

    pub fn resume_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let current_state = {
            let handle = self
                .agents
                .get(&id)
                .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
            handle.state
        };
        match current_state {
            AgentState::Paused => {
                self.transition_agent_state(id, AgentState::Running, "resume")?;
                Ok(())
            }
            AgentState::Running => Ok(()),
            _ => Err(AgentError::InvalidTransition {
                from: current_state,
                to: AgentState::Running,
            }),
        }
    }

    pub fn restart_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        let (state, autonomy_level) = self
            .agents
            .get(&id)
            .map(|agent| {
                (
                    agent.state,
                    AutonomyLevel::from_numeric(agent.autonomy_level).unwrap_or_default(),
                )
            })
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        if autonomy_level == AutonomyLevel::L5 {
            let others: Vec<_> = self
                .active_sovereign_agents()
                .into_iter()
                .filter(|(existing_id, _)| **existing_id != id)
                .collect();
            if others.len() >= self.config.max_l5_agents {
                let mut names = others
                    .iter()
                    .map(|(_, handle)| handle.manifest.name.clone())
                    .collect::<Vec<_>>();
                names.sort();
                return Err(AgentError::SupervisorError(format!(
                    "Maximum {} L5 agents allowed. Currently active: {}. Stop one first.",
                    self.config.max_l5_agents,
                    names.join(", ")
                )));
            }
        }

        if autonomy_level == AutonomyLevel::L6 {
            let others: Vec<_> = self
                .active_transcendent_agents()
                .into_iter()
                .filter(|(existing_id, _)| **existing_id != id)
                .collect();
            if others.len() >= self.config.max_l6_agents {
                let mut names = others
                    .iter()
                    .map(|(_, handle)| handle.manifest.name.clone())
                    .collect::<Vec<_>>();
                names.sort();
                return Err(AgentError::SupervisorError(format!(
                    "Maximum {} Transcendent agents allowed. Currently active: {}. Stop one first.",
                    self.config.max_l6_agents,
                    names.join(", ")
                )));
            }
        }

        if matches!(state, AgentState::Running | AgentState::Paused) {
            self.stop_agent(id)?;
        }
        self.consume_fuel(id, "supervisor.restart")?;
        self.transition_agent_state(id, AgentState::Starting, "restart")?;
        self.transition_agent_state(id, AgentState::Running, "restart")?;
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
        // Use cost_units as the authoritative cost (not output_tokens alone).
        // Reserve the full cost_units, then commit the same amount.
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        if handle.remaining_fuel < cost_units {
            return Err(self.apply_fuel_violation(
                id,
                FuelViolation::OverMonthlyCap,
                "LLM token spend exceeded remaining fuel",
            ));
        }
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.remaining_fuel -= cost_units;

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
            Err(violation) => {
                // Refund the deducted fuel since the ledger rejected the spend
                if let Some(handle) = self.agents.get_mut(&id) {
                    handle.remaining_fuel += cost_units;
                }
                Err(self.apply_fuel_violation(
                    id,
                    violation,
                    "fuel hardening violation from LLM spend",
                ))
            }
        }
    }

    pub fn fuel_audit_report(&self, id: AgentId) -> Option<FuelAuditReport> {
        self.fuel_ledgers.get(&id).map(|ledger| ledger.snapshot(id))
    }

    pub fn restore_fuel_report(
        &mut self,
        id: AgentId,
        report: FuelAuditReport,
        remaining_fuel: u64,
    ) -> Result<(), AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        handle.remaining_fuel = remaining_fuel;
        self.fuel_ledgers
            .insert(id, AgentFuelLedger::from_report(&report));
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

    /// Mutable access to an agent handle (for runtime adjustments like autonomy level).
    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut AgentHandle> {
        self.agents.get_mut(&id)
    }

    /// Remove all agents and their fuel ledgers from in-memory state.
    pub fn clear_all_agents(&mut self) {
        self.agents.clear();
        self.fuel_ledgers.clear();
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
            let handle = self.agents.get_mut(&id).ok_or_else(|| {
                AgentError::SupervisorError(format!("agent '{id}' not found during policy check"))
            })?;
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
                    let handle = self.agents.get(&id).ok_or_else(|| {
                        AgentError::SupervisorError(format!(
                            "agent '{id}' not found during speculative simulation"
                        ))
                    })?;
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
        let old_caps = handle.manifest.capabilities.clone();
        handle.manifest = updated;

        // Create a time machine checkpoint for permission change
        let mut builder = self
            .time_machine
            .begin_checkpoint("permission_change", Some(id.to_string()));
        builder.record_agent_state(
            &id.to_string(),
            "capabilities",
            json!(old_caps),
            json!(handle.manifest.capabilities),
        );
        let checkpoint = builder.build();
        // Best-effort: failure to record checkpoint does not block permission update
        let _ = self.time_machine.commit_checkpoint(checkpoint);

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
        self.consume_fuel_units(id, reason, 1)
    }

    fn consume_fuel_units(
        &mut self,
        id: AgentId,
        reason: &str,
        units: u64,
    ) -> Result<(), AgentError> {
        // Reserve-then-commit: reserve the exact amount, execute, commit.
        let reservation = self.reserve_fuel(id, units, reason)?;
        // For internal supervisor ops the actual cost equals the requested cost,
        // so commit immediately with the full amount.
        self.commit_fuel(reservation, units)
    }

    /// Reserve fuel BEFORE execution.  Returns `Err` if the agent does not have
    /// enough remaining fuel for `max_cost`.  The reserved amount is immediately
    /// subtracted from the agent's balance and held until [`commit_fuel`] or
    /// [`cancel_fuel`] is called.
    pub fn reserve_fuel(
        &mut self,
        id: AgentId,
        max_cost: u64,
        action_type: &str,
    ) -> Result<SupervisorFuelReservation, AgentError> {
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;

        if handle.remaining_fuel < max_cost {
            return Err(self.apply_fuel_violation(
                id,
                FuelViolation::OverMonthlyCap,
                &format!(
                    "insufficient fuel for {action_type}: need {max_cost}, have {}",
                    // re-fetch since apply_fuel_violation may mutate
                    self.agents.get(&id).map(|h| h.remaining_fuel).unwrap_or(0)
                ),
            ));
        }

        // Lock the fuel — deduct from available immediately
        let handle = self
            .agents
            .get_mut(&id)
            .ok_or_else(|| AgentError::SupervisorError(format!("agent '{id}' not found")))?;
        let fuel_before = handle.remaining_fuel;
        handle.remaining_fuel -= max_cost;

        // Record reservation in time machine
        let mut builder = self
            .time_machine
            .begin_checkpoint("fuel_reservation", Some(id.to_string()));
        builder.record_agent_state(
            &id.to_string(),
            "fuel",
            json!(fuel_before),
            json!(handle.remaining_fuel),
        );
        let checkpoint = builder.build();
        // Best-effort: failure to record checkpoint does not block fuel reservation
        let _ = self.time_machine.commit_checkpoint(checkpoint);

        Ok(SupervisorFuelReservation {
            id: Uuid::new_v4(),
            agent_id: id,
            reserved_amount: max_cost,
            action_type: action_type.to_string(),
        })
    }

    /// Commit actual cost after execution.  Refunds unused reservation back to
    /// the agent's balance.  `actual_cost` must be ≤ `reservation.reserved_amount`.
    pub fn commit_fuel(
        &mut self,
        reservation: SupervisorFuelReservation,
        actual_cost: u64,
    ) -> Result<(), AgentError> {
        let refund = reservation.reserved_amount.saturating_sub(actual_cost);
        if refund > 0 {
            if let Some(handle) = self.agents.get_mut(&reservation.agent_id) {
                handle.remaining_fuel += refund;
            }
        }

        if actual_cost > reservation.reserved_amount {
            eprintln!(
                "FUEL OVERRUN: reserved {} but actual was {} for {}",
                reservation.reserved_amount, actual_cost, reservation.action_type
            );
        }

        // Record spend in ledger and time machine
        let model_name = self
            .agents
            .get(&reservation.agent_id)
            .and_then(|h| h.manifest.llm_model.clone())
            .unwrap_or_else(|| "runtime".to_string());

        let ledger = self
            .fuel_ledgers
            .get_mut(&reservation.agent_id)
            .ok_or_else(|| {
                AgentError::SupervisorError(format!(
                    "agent '{}' missing fuel ledger",
                    reservation.agent_id
                ))
            })?;
        match ledger.record_llm_spend(
            reservation.agent_id,
            model_name.as_str(),
            0,
            actual_cost as u32,
            actual_cost,
            &mut self.audit_trail,
        ) {
            Ok(()) => Ok(()),
            Err(violation) => Err(self.apply_fuel_violation(
                reservation.agent_id,
                violation,
                &reservation.action_type,
            )),
        }
    }

    /// Cancel a reservation — return all reserved fuel to the agent.
    pub fn cancel_fuel(&mut self, reservation: SupervisorFuelReservation) {
        if let Some(handle) = self.agents.get_mut(&reservation.agent_id) {
            handle.remaining_fuel += reservation.reserved_amount;
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
                let old_state = self.agents.get(&id).map(|agent| agent.state);
                if let Some(AgentState::Running | AgentState::Paused | AgentState::Starting) =
                    old_state
                {
                    if self
                        .transition_agent_state(id, AgentState::Stopping, "safety-halt")
                        .is_err()
                    {
                        let _ = self.force_transition_agent_state(
                            id,
                            AgentState::Stopping,
                            "safety-halt-forced",
                        );
                    }
                    if self
                        .transition_agent_state(id, AgentState::Stopped, "safety-halt")
                        .is_err()
                    {
                        let _ = self.force_transition_agent_state(
                            id,
                            AgentState::Stopped,
                            "safety-halt-forced",
                        );
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
        AutonomyLevel::L6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::{GovernedOperation, HitlTier};
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
            default_goal: None,
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
        // Configure allowed approvers for operations used in tests
        if let Some(handle) = sup.agents.get_mut(&id) {
            let policy = handle.consent_runtime.policy_engine_mut();
            policy.set_policy(
                GovernedOperation::TerminalCommand,
                HitlTier::Tier2,
                vec!["admin".to_string()],
            );
            policy.set_policy(
                GovernedOperation::SocialPostPublish,
                HitlTier::Tier2,
                vec!["admin".to_string()],
            );
            policy.set_policy(
                GovernedOperation::SelfMutationApply,
                HitlTier::Tier3,
                vec!["admin".to_string(), "admin2".to_string()],
            );
        }
        (sup, id)
    }

    fn l5_manifest(name: &str) -> AgentManifest {
        let mut manifest = test_manifest();
        manifest.name = name.to_string();
        manifest.autonomy_level = Some(5);
        manifest
    }

    fn l6_manifest(name: &str) -> AgentManifest {
        let mut manifest = test_manifest();
        manifest.name = name.to_string();
        manifest.autonomy_level = Some(6);
        manifest
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

        // SECURITY INVARIANT: Cedar Allow can only auto-approve Tier0/Tier1
        // operations. Tier2+ (e.g., TerminalCommand) ALWAYS require human
        // consent — Cedar Allow cannot override this.

        // TerminalCommand defaults to Tier2 (requires approval).
        // Even with a Cedar Allow policy, Tier2 must still require approval.
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

        let pe = PolicyEngine::with_policies(vec![allow_terminal.clone()]);
        let mut sup = Supervisor::new();
        sup.set_policy_engine(pe);
        let id = sup.start_agent(test_manifest()).unwrap();

        // Tier2 operations CANNOT be auto-approved by Cedar Allow.
        let result = sup.require_consent(id, GovernedOperation::TerminalCommand, b"echo hello");
        assert!(
            result.is_err(),
            "Cedar Allow must NOT auto-approve Tier2 operations: {result:?}"
        );

        // But Cedar Allow CAN auto-approve Tier1 operations (e.g., ToolCall).
        let allow_tool = Policy {
            policy_id: "allow-tool".to_string(),
            description: String::new(),
            effect: PolicyEffect::Allow,
            principal: "*".to_string(),
            action: "tool_call".to_string(),
            resource: "*".to_string(),
            priority: 10,
            conditions: PolicyConditions::default(),
        };

        let pe2 = PolicyEngine::with_policies(vec![allow_terminal, allow_tool]);
        let mut sup2 = Supervisor::new();
        sup2.set_policy_engine(pe2);
        let id2 = sup2.start_agent(test_manifest()).unwrap();

        let result2 = sup2.require_consent(id2, GovernedOperation::ToolCall, b"test");
        assert!(
            result2.is_ok(),
            "Cedar Allow should auto-approve Tier1 ToolCall: {result2:?}"
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

    /// Test helper: supervisor with the historical cap of max_l6_agents=2.
    /// Use this in tests that predate configurable caps so their semantics
    /// (cap enforcement with N=2) are preserved explicitly.
    fn sup_with_l6_cap_2() -> Supervisor {
        Supervisor::with_config(SupervisorConfig {
            max_l5_agents: 1,
            max_l6_agents: 2,
        })
    }

    #[test]
    fn only_one_l5_agent_can_be_active() {
        let mut sup = sup_with_l6_cap_2();
        let first = sup.start_agent(l5_manifest("nexus-sovereign"));
        assert!(first.is_ok());

        let second = sup.start_agent(l5_manifest("nexus-infinity"));
        let err = second.unwrap_err();
        assert_eq!(
            err.to_string(),
            "supervisor error: Maximum 1 L5 agents allowed. Currently active: nexus-sovereign. Stop one first."
        );
    }

    #[test]
    fn l5_slot_reopens_after_stop() {
        let mut sup = sup_with_l6_cap_2();
        let first = sup.start_agent(l5_manifest("nexus-sovereign")).unwrap();
        sup.stop_agent(first).unwrap();

        let second = sup.start_agent(l5_manifest("nexus-infinity"));
        assert!(second.is_ok());
    }

    #[test]
    fn maximum_two_l6_agents_can_be_active() {
        let mut sup = sup_with_l6_cap_2();
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());

        let err = sup.start_agent(l6_manifest("oracle-supreme")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "supervisor error: Maximum 2 Transcendent agents allowed. Currently active: architect-prime, ascendant. Stop one first."
        );
    }

    #[test]
    fn l5_and_l6_limits_do_not_interfere() {
        let mut sup = sup_with_l6_cap_2();
        assert!(sup.start_agent(l5_manifest("nexus-sovereign")).is_ok());
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());
    }

    #[test]
    fn restart_agent_enforces_l5_singleton() {
        let mut sup = sup_with_l6_cap_2();
        let first = sup.start_agent(l5_manifest("nexus-sovereign")).unwrap();
        let second = sup.start_agent(test_manifest()).unwrap();
        {
            let handle = sup
                .agents
                .get_mut(&second)
                .expect("second agent should exist");
            handle.manifest = l5_manifest("nexus-infinity");
            handle.autonomy_guard = AutonomyGuard::new(AutonomyLevel::L5);
            handle.autonomy_level = 5;
            handle.state = AgentState::Stopped;
        }

        let err = sup.restart_agent(second).unwrap_err();
        assert_eq!(
            err.to_string(),
            "supervisor error: Maximum 1 L5 agents allowed. Currently active: nexus-sovereign. Stop one first."
        );
        sup.stop_agent(first).unwrap();
    }

    #[test]
    fn restart_agent_enforces_l6_limit() {
        let mut sup = sup_with_l6_cap_2();
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());
        let third = sup.start_agent(test_manifest()).unwrap();
        {
            let handle = sup
                .agents
                .get_mut(&third)
                .expect("third agent should exist");
            handle.manifest = l6_manifest("oracle-supreme");
            handle.autonomy_guard = AutonomyGuard::new(AutonomyLevel::L6);
            handle.autonomy_level = 6;
            handle.state = AgentState::Stopped;
        }

        let err = sup.restart_agent(third).unwrap_err();
        assert_eq!(
            err.to_string(),
            "supervisor error: Maximum 2 Transcendent agents allowed. Currently active: architect-prime, ascendant. Stop one first."
        );
    }

    // ── Fuel reservation (reserve-then-commit) tests ──

    #[test]
    fn fuel_reservation_blocks_if_insufficient() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let remaining = sup.agents.get(&id).unwrap().remaining_fuel;
        // Try to reserve more than available
        let result = sup.reserve_fuel(id, remaining + 1, "test_action");
        assert!(result.is_err());
    }

    #[test]
    fn fuel_reservation_deducts_on_reserve_and_refunds_on_cancel() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let initial = sup.agents.get(&id).unwrap().remaining_fuel;

        let reservation = sup.reserve_fuel(id, 500, "test_action").unwrap();
        assert_eq!(
            sup.agents.get(&id).unwrap().remaining_fuel,
            initial - 500,
            "reserve should deduct immediately"
        );

        sup.cancel_fuel(reservation);
        assert_eq!(
            sup.agents.get(&id).unwrap().remaining_fuel,
            initial,
            "cancel should refund fully"
        );
    }

    #[test]
    fn fuel_reservation_commit_refunds_unused() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let initial = sup.agents.get(&id).unwrap().remaining_fuel;

        let reservation = sup.reserve_fuel(id, 800, "test_action").unwrap();
        // Actual cost is only 300 — should refund 500
        sup.commit_fuel(reservation, 300).unwrap();

        // Account for the 1-unit cost of supervisor.start (consumed during setup)
        // and the 300 actual units committed here.
        let expected = initial - 300;
        assert_eq!(
            sup.agents.get(&id).unwrap().remaining_fuel,
            expected,
            "commit should refund unused reservation (reserved 800, used 300)"
        );
    }

    #[test]
    fn fuel_reservation_commit_exact_no_refund() {
        let (mut sup, id) = setup_supervisor_with_agent();
        let initial = sup.agents.get(&id).unwrap().remaining_fuel;

        let reservation = sup.reserve_fuel(id, 400, "test_action").unwrap();
        sup.commit_fuel(reservation, 400).unwrap();

        assert_eq!(
            sup.agents.get(&id).unwrap().remaining_fuel,
            initial - 400,
            "exact commit should consume all reserved fuel"
        );
    }

    #[test]
    fn max_fuel_cost_returns_conservative_defaults() {
        assert_eq!(max_fuel_cost("llm_inference_cloud"), 10_000);
        assert_eq!(max_fuel_cost("filesystem_read"), 200);
        assert_eq!(max_fuel_cost("unknown_action"), 1_000);
    }

    // ── register_agent / transition_to_running tests ──

    #[test]
    fn test_register_agent_does_not_enforce_l6_cap() {
        let mut sup = sup_with_l6_cap_2();
        // Start 2 L6 agents in Running state via start_agent (cap is full).
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());

        // Register 10 more L6 agents in Stopped state — cap must NOT fire.
        let names = [
            "nexus-arbiter",
            "nexus-continuum",
            "nexus-genesis-prime",
            "nexus-mirror",
            "nexus-weaver",
            "nexus-prime",
            "nexus-oracle-omega",
            "nexus-oracle-supreme",
            "nexus-warden",
            "nexus-legion",
        ];
        for name in &names {
            let id = Uuid::new_v4();
            assert!(
                sup.register_agent(id, l6_manifest(name), AgentState::Stopped)
                    .is_ok(),
                "register_agent should not enforce L6 cap for Stopped agents: {name}"
            );
        }

        assert_eq!(sup.agents.len(), 12, "all 12 agents should be registered");
    }

    #[test]
    fn test_transition_to_running_enforces_l6_cap() {
        let mut sup = sup_with_l6_cap_2();
        // 2 running L6 agents.
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());

        // Register a stopped L6 agent.
        let stopped_id = Uuid::new_v4();
        assert!(sup
            .register_agent(
                stopped_id,
                l6_manifest("oracle-supreme"),
                AgentState::Stopped
            )
            .is_ok());

        // Transition to running must fail — cap is full.
        let err = sup.transition_to_running(stopped_id).unwrap_err();
        assert_eq!(
            err.to_string(),
            "supervisor error: Maximum 2 Transcendent agents allowed. Currently active: architect-prime, ascendant. Stop one first."
        );

        // Agent should still be in Stopped state.
        assert_eq!(
            sup.agents.get(&stopped_id).unwrap().state,
            AgentState::Stopped
        );
    }

    #[test]
    fn test_transition_to_running_succeeds_for_l5_when_l6_cap_full() {
        let mut sup = sup_with_l6_cap_2();
        // Fill L6 cap.
        assert!(sup.start_agent(l6_manifest("ascendant")).is_ok());
        assert!(sup.start_agent(l6_manifest("architect-prime")).is_ok());

        // Register a stopped L5 agent.
        let l5_id = Uuid::new_v4();
        assert!(sup
            .register_agent(l5_id, l5_manifest("sentinel"), AgentState::Stopped)
            .is_ok());

        // L5 transition should succeed — L6 cap does not affect L5.
        assert!(sup.transition_to_running(l5_id).is_ok());
        assert_eq!(sup.agents.get(&l5_id).unwrap().state, AgentState::Running);
    }

    #[test]
    fn test_stop_l6_then_start_another_l6_succeeds() {
        let mut sup = sup_with_l6_cap_2();
        let first = sup.start_agent(l6_manifest("ascendant")).unwrap();
        let _second = sup.start_agent(l6_manifest("architect-prime")).unwrap();

        // Register 10 stopped L6 agents.
        let mut stopped_ids = Vec::new();
        for i in 0..10 {
            let id = Uuid::new_v4();
            assert!(sup
                .register_agent(id, l6_manifest(&format!("agent-{i}")), AgentState::Stopped)
                .is_ok());
            stopped_ids.push(id);
        }

        // Stop one of the running L6 agents.
        assert!(sup.stop_agent(first).is_ok());

        // Now transition one of the stopped L6 agents to Running.
        assert!(sup.transition_to_running(stopped_ids[0]).is_ok());
        assert_eq!(
            sup.agents.get(&stopped_ids[0]).unwrap().state,
            AgentState::Running
        );

        // Count running L6: should be exactly 2 (architect-prime + agent-0).
        let active = sup.active_transcendent_agents();
        assert_eq!(active.len(), 2);

        // A third L6 start should still fail.
        let err = sup.transition_to_running(stopped_ids[1]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Maximum 2 Transcendent agents allowed"));
    }

    #[test]
    fn test_register_duplicate_uuid_returns_error() {
        let mut sup = Supervisor::new();
        let id = Uuid::new_v4();
        assert!(sup
            .register_agent(id, test_manifest(), AgentState::Stopped)
            .is_ok());

        let err = sup
            .register_agent(id, test_manifest(), AgentState::Stopped)
            .unwrap_err();
        assert!(
            err.to_string().contains("already registered"),
            "expected duplicate registration error, got: {err}"
        );
    }

    #[test]
    fn test_transition_to_running_idempotent_for_already_running() {
        let mut sup = Supervisor::new();
        let id = sup.start_agent(test_manifest()).unwrap();
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Running);

        // Calling transition_to_running on an already-running agent should be a no-op.
        assert!(sup.transition_to_running(id).is_ok());
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Running);
    }

    // ── SupervisorConfig tests ──

    #[test]
    fn test_supervisor_config_default_values() {
        let config = SupervisorConfig::default();
        assert_eq!(config.max_l5_agents, 1);
        assert_eq!(config.max_l6_agents, 6);
    }

    // NOTE: Env-var-driven tests (below) mutate process-global state. Rust runs
    // tests in parallel by default, so these are marked `#[ignore]` to avoid
    // flakiness with other tests that read the same env vars. Run explicitly:
    //   cargo test -p nexus-kernel -- --ignored supervisor_config_from_env
    #[test]
    #[ignore]
    fn test_supervisor_config_from_env_with_valid_values() {
        std::env::set_var("NEXUS_MAX_L6_AGENTS", "10");
        let config = SupervisorConfig::from_env();
        assert_eq!(config.max_l6_agents, 10);
        std::env::remove_var("NEXUS_MAX_L6_AGENTS");
    }

    #[test]
    #[ignore]
    fn test_supervisor_config_from_env_with_invalid_value_uses_default() {
        std::env::set_var("NEXUS_MAX_L6_AGENTS", "not_a_number");
        let config = SupervisorConfig::from_env();
        assert_eq!(config.max_l6_agents, 6);
        std::env::remove_var("NEXUS_MAX_L6_AGENTS");
    }

    #[test]
    fn test_supervisor_with_config_l6_cap_six() {
        let mut sup = Supervisor::with_config(SupervisorConfig {
            max_l5_agents: 1,
            max_l6_agents: 6,
        });
        for i in 0..6 {
            assert!(
                sup.start_agent(l6_manifest(&format!("l6-{i}"))).is_ok(),
                "agent {i} should start"
            );
        }
        let err = sup.start_agent(l6_manifest("l6-7")).unwrap_err();
        assert!(
            err.to_string()
                .contains("Maximum 6 Transcendent agents allowed"),
            "expected cap error for 7th L6, got: {err}"
        );
    }

    #[test]
    fn test_supervisor_with_config_l6_cap_zero_blocks_all() {
        let mut sup = Supervisor::with_config(SupervisorConfig {
            max_l5_agents: 1,
            max_l6_agents: 0,
        });
        let id = Uuid::new_v4();
        sup.register_agent(id, l6_manifest("blocked"), AgentState::Stopped)
            .unwrap();
        let err = sup.transition_to_running(id).unwrap_err();
        assert!(
            err.to_string()
                .contains("Maximum 0 Transcendent agents allowed"),
            "expected cap=0 error, got: {err}"
        );
    }

    #[test]
    fn test_supervisor_with_config_custom_l5_cap() {
        let mut sup = Supervisor::with_config(SupervisorConfig {
            max_l5_agents: 3,
            max_l6_agents: 6,
        });
        assert!(sup.start_agent(l5_manifest("a")).is_ok());
        assert!(sup.start_agent(l5_manifest("b")).is_ok());
        assert!(sup.start_agent(l5_manifest("c")).is_ok());
        let err = sup.start_agent(l5_manifest("d")).unwrap_err();
        assert!(
            err.to_string().contains("Maximum 3 L5 agents allowed"),
            "expected cap=3 error, got: {err}"
        );
    }

    // ── Bug D2: state-transition chokepoint tests ──

    fn last_transition_event(sup: &Supervisor) -> Option<&serde_json::Value> {
        sup.audit_trail.events().iter().rev().find_map(|event| {
            let payload = &event.payload;
            if payload.get("event_kind").and_then(|v| v.as_str()) == Some("agent.state_transition")
            {
                Some(payload)
            } else {
                None
            }
        })
    }

    #[test]
    fn test_transition_agent_state_legal() {
        let (mut sup, id) = setup_supervisor_with_agent();
        // start_agent leaves the agent in Running; we can move Running -> Paused.
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Running);

        let events_before = sup.audit_trail.events().len();
        let next = sup
            .transition_agent_state(id, AgentState::Paused, "test")
            .expect("legal transition should succeed");

        assert_eq!(next, AgentState::Paused);
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Paused);
        assert!(
            sup.audit_trail.events().len() > events_before,
            "audit trail should have grown after transition_agent_state"
        );
        let payload = last_transition_event(&sup).expect("transition audit event must exist");
        assert_eq!(
            payload.get("forced").and_then(|v| v.as_bool()),
            Some(false),
            "forced flag should be false for legal transition"
        );
        assert_eq!(payload.get("reason").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(
            payload.get("new_state").and_then(|v| v.as_str()),
            Some("Paused")
        );
    }

    #[test]
    fn test_transition_agent_state_unknown_agent_errors() {
        let mut sup = Supervisor::new();
        let ghost = Uuid::new_v4();
        let err = sup
            .transition_agent_state(ghost, AgentState::Stopped, "test")
            .unwrap_err();
        match err {
            AgentError::SupervisorError(msg) => {
                assert!(msg.contains("not found"), "unexpected error message: {msg}");
            }
            other => panic!("expected SupervisorError, got {other:?}"),
        }
    }

    #[test]
    fn test_transition_agent_state_illegal_transition_rejects() {
        let (mut sup, id) = setup_supervisor_with_agent();
        // Drive the agent to Stopped through the normal path so the FSM agrees.
        sup.stop_agent(id).expect("stop should succeed");
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Stopped);

        let events_before = sup.audit_trail.events().len();
        let err = sup
            .transition_agent_state(id, AgentState::Running, "test")
            .unwrap_err();
        assert!(matches!(err, AgentError::InvalidTransition { .. }));
        assert_eq!(
            sup.agents.get(&id).unwrap().state,
            AgentState::Stopped,
            "state must remain Stopped after rejected transition"
        );
        assert_eq!(
            sup.audit_trail.events().len(),
            events_before,
            "no audit event should be written when the FSM rejects the transition"
        );
    }

    #[test]
    fn test_force_transition_agent_state_accepts_illegal() {
        let (mut sup, id) = setup_supervisor_with_agent();
        sup.stop_agent(id).expect("stop should succeed");
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Stopped);

        let next = sup
            .force_transition_agent_state(id, AgentState::Running, "test-force")
            .expect("force path should accept illegal transition");
        assert_eq!(next, AgentState::Running);
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Running);

        let payload = last_transition_event(&sup).expect("forced audit event must exist");
        assert_eq!(payload.get("forced").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            payload.get("reason").and_then(|v| v.as_str()),
            Some("test-force")
        );
    }

    #[test]
    fn test_safety_halt_path_transitions_through_chokepoint() {
        let (mut sup, id) = setup_supervisor_with_agent();
        assert_eq!(sup.agents.get(&id).unwrap().state, AgentState::Running);

        // manual_halt_agent drives SafetyAction::Halted and returns Err(SupervisorError).
        let err = sup
            .manual_halt_agent(id, "admin", "integration test")
            .unwrap_err();
        assert!(matches!(err, AgentError::SupervisorError(_)));
        assert_eq!(
            sup.agents.get(&id).unwrap().state,
            AgentState::Stopped,
            "safety-halt must crush agent to Stopped"
        );

        let has_halt_event = sup.audit_trail.events().iter().any(|event| {
            let payload = &event.payload;
            payload.get("event_kind").and_then(|v| v.as_str()) == Some("agent.state_transition")
                && payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.contains("safety-halt"))
                    .unwrap_or(false)
        });
        assert!(
            has_halt_event,
            "audit trail should contain at least one safety-halt state transition"
        );
    }
}
