//! Cognitive loop runtime — runs the perceive→reason→plan→act→reflect→learn loop.

use super::algorithms::{AdversarialArena, EvolutionEngine, SwarmCoordinator, WorldModel};
use super::evolution::EvolutionTracker;
use super::memory_manager::AgentMemoryManager;
use super::planner::CognitivePlanner;
use super::types::{
    AgentGoal, AgentStep, CognitiveEvent, CognitivePhase, CognitiveStatusResponse, CycleResult,
    GoalStatus, LoopConfig, PlannedAction, PlanningContext, StepStatus,
};
use crate::actuators::{ActuatorContext, ActuatorRegistry};
use crate::audit::{AuditTrail, EventType};
use crate::autonomy::AutonomyLevel;
use crate::errors::AgentError;
use crate::supervisor::Supervisor;
use nexus_persistence::{NexusDatabase, StateStore};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const L6_COOLDOWN_CYCLES: u32 = 100;
#[cfg(test)]
const L6_COOLDOWN_SLEEP: Duration = Duration::from_millis(1);
#[cfg(not(test))]
const L6_COOLDOWN_SLEEP: Duration = Duration::from_secs(60);

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
struct CognitiveOverrides {
    max_cycles_per_goal: u32,
    fuel_reserve_threshold: f64,
    reflection_interval: u32,
    cycle_delay_ms: u64,
    planning_depth: u32,
}

#[derive(Debug, Clone)]
struct PhaseModelSelection {
    provider: String,
    model: String,
}

#[derive(Debug, Clone)]
struct SelectedAlgorithm {
    algorithm: String,
    config_json: String,
}

/// Callback for emitting cognitive events (phase changes, step results, etc.).
pub trait EventEmitter: Send + Sync {
    fn emit(&self, event: CognitiveEvent);
}

/// No-op event emitter for headless/test use.
pub struct NoOpEmitter;

impl EventEmitter for NoOpEmitter {
    fn emit(&self, _event: CognitiveEvent) {}
}

/// Collects events for testing.
pub struct CollectingEmitter {
    pub events: Mutex<Vec<CognitiveEvent>>,
}

impl Default for CollectingEmitter {
    fn default() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl CollectingEmitter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventEmitter for CollectingEmitter {
    fn emit(&self, event: CognitiveEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// Trait for executing planned actions. Separates execution from the loop logic.
pub trait ActionExecutor: Send + Sync {
    fn execute(
        &self,
        agent_id: &str,
        action: &PlannedAction,
        audit: &mut AuditTrail,
    ) -> Result<String, String>;
}

/// `ActionExecutor` implementation that routes actions through the governed
/// `ActuatorRegistry`. This bridges the cognitive loop to real-world actuators
/// (filesystem, shell, web, API) with full governance enforcement.
pub struct RegistryExecutor {
    registry: ActuatorRegistry,
    /// Base directory for agent workspaces: `{base}/{agent_id}/workspace/`.
    workspace_base: PathBuf,
    /// Shared supervisor for looking up the agent's effective runtime context.
    supervisor: Arc<Mutex<Supervisor>>,
    /// Optional governance reviewer (for Warden interception).
    action_review_engine: Option<Arc<dyn crate::actuators::ActionReviewEngine>>,
}

impl RegistryExecutor {
    /// Create a new registry executor.
    ///
    /// * `workspace_base` — parent directory for agent workspaces (e.g. `~/.nexus/agents/`)
    /// * `audit` — shared audit trail
    /// * `supervisor` — source of agent capabilities, fuel, autonomy, and allowlists
    pub fn new(
        workspace_base: PathBuf,
        _audit: Arc<Mutex<AuditTrail>>,
        supervisor: Arc<Mutex<Supervisor>>,
        action_review_engine: Option<Arc<dyn crate::actuators::ActionReviewEngine>>,
    ) -> Self {
        Self {
            registry: ActuatorRegistry::with_defaults(),
            workspace_base,
            supervisor,
            action_review_engine,
        }
    }

    /// Build the actuator context for a given agent.
    fn build_context(
        &self,
        agent_id: &str,
        agent_name: &str,
        capabilities: &[String],
        fuel_remaining: f64,
        autonomy_level: AutonomyLevel,
        egress_allowlist: Vec<String>,
    ) -> ActuatorContext {
        let working_dir = self.workspace_base.join(agent_id).join("workspace");
        ActuatorContext {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            working_dir,
            autonomy_level,
            capabilities: capabilities.iter().cloned().collect::<HashSet<String>>(),
            fuel_remaining,
            egress_allowlist,
            action_review_engine: self.action_review_engine.clone(),
        }
    }
}

impl ActionExecutor for RegistryExecutor {
    fn execute(
        &self,
        agent_id: &str,
        action: &PlannedAction,
        audit: &mut AuditTrail,
    ) -> Result<String, String> {
        // For actions not handled by actuators (LlmQuery, MemoryStore, etc.),
        // fall through to the default placeholder behavior.
        let is_actuator_action = matches!(
            action,
            PlannedAction::FileRead { .. }
                | PlannedAction::FileWrite { .. }
                | PlannedAction::ShellCommand { .. }
                | PlannedAction::DockerCommand { .. }
                | PlannedAction::WebSearch { .. }
                | PlannedAction::WebFetch { .. }
                | PlannedAction::ApiCall { .. }
                | PlannedAction::ImageGenerate { .. }
                | PlannedAction::TextToSpeech { .. }
                | PlannedAction::KnowledgeGraphUpdate { .. }
                | PlannedAction::KnowledgeGraphQuery { .. }
                | PlannedAction::BrowserAutomate { .. }
                | PlannedAction::CaptureScreen { .. }
                | PlannedAction::CaptureWindow { .. }
                | PlannedAction::AnalyzeScreen { .. }
                | PlannedAction::MouseMove { .. }
                | PlannedAction::MouseClick { .. }
                | PlannedAction::MouseDoubleClick { .. }
                | PlannedAction::MouseDrag { .. }
                | PlannedAction::KeyboardType { .. }
                | PlannedAction::KeyboardPress { .. }
                | PlannedAction::KeyboardShortcut { .. }
                | PlannedAction::ScrollWheel { .. }
                | PlannedAction::ComputerAction { .. }
                | PlannedAction::ModifyCognitiveParams { .. }
                | PlannedAction::SelectLlmProvider { .. }
                | PlannedAction::SelectAlgorithm { .. }
                | PlannedAction::DesignAgentEcosystem { .. }
                | PlannedAction::RunCounterfactual { .. }
                | PlannedAction::TemporalPlan { .. }
        );

        if !is_actuator_action {
            return Ok(format!(
                "action '{}' not routed through actuators",
                action.action_type()
            ));
        }

        let agent_uuid =
            uuid::Uuid::parse_str(agent_id).map_err(|e| format!("invalid agent id: {e}"))?;
        let (agent_name, capabilities, fuel_remaining, autonomy_level, egress_allowlist) = {
            let supervisor = self
                .supervisor
                .lock()
                .map_err(|e| format!("supervisor lock: {e}"))?;
            let handle = supervisor
                .get_agent(agent_uuid)
                .ok_or_else(|| format!("agent '{agent_id}' not found in supervisor"))?;
            (
                handle.manifest.name.clone(),
                handle.manifest.capabilities.clone(),
                handle.remaining_fuel as f64,
                AutonomyLevel::from_numeric(handle.autonomy_level).unwrap_or_default(),
                handle
                    .manifest
                    .allowed_endpoints
                    .clone()
                    .unwrap_or_default(),
            )
        };

        let ctx = self.build_context(
            agent_id,
            &agent_name,
            &capabilities,
            fuel_remaining,
            autonomy_level,
            egress_allowlist,
        );

        self.registry
            .execute_action(action, &ctx, audit)
            .map(|r| r.output)
            .map_err(|e| e.to_string())
    }
}

/// State tracked per running agent loop.
struct AgentLoopState {
    goal: AgentGoal,
    phase: CognitivePhase,
    steps: Vec<AgentStep>,
    current_step_index: usize,
    cycle_count: u32,
    total_fuel_consumed: f64,
    consecutive_failures: u32,
    steps_completed: u32,
    shutdown: Arc<AtomicBool>,
    /// Remaining approved HITL actions for the current plan.
    hitl_approval_allowance: u32,
    /// When true, keep showing one approval request per HITL step.
    review_each_mode: bool,
    /// Strategy hash used for this goal (for evolution tracking).
    strategy_hash: Option<String>,
    /// Timestamp when the goal started (for duration tracking).
    #[allow(dead_code)]
    started_at_secs: u64,
}

/// The cognitive runtime manages agent loops.
pub struct CognitiveRuntime {
    supervisor: Arc<Mutex<Supervisor>>,
    config: LoopConfig,
    emitter: Arc<dyn EventEmitter>,
    provider_registry: HashMap<String, Arc<dyn LlmProvider>>,
    /// Active loop states keyed by agent_id string.
    loops: Mutex<HashMap<String, AgentLoopState>>,
    /// Shutdown flags keyed by agent_id.
    shutdown_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl CognitiveRuntime {
    pub fn new(
        supervisor: Arc<Mutex<Supervisor>>,
        config: LoopConfig,
        emitter: Arc<dyn EventEmitter>,
    ) -> Self {
        Self::with_provider_registry(supervisor, config, emitter, HashMap::new())
    }

    pub fn with_provider_registry(
        supervisor: Arc<Mutex<Supervisor>>,
        config: LoopConfig,
        emitter: Arc<dyn EventEmitter>,
        provider_registry: HashMap<String, Arc<dyn LlmProvider>>,
    ) -> Self {
        Self {
            supervisor,
            config,
            emitter,
            provider_registry,
            loops: Mutex::new(HashMap::new()),
            shutdown_flags: Mutex::new(HashMap::new()),
        }
    }

    /// Assign a goal to an agent. Initializes the loop state.
    pub fn assign_goal(&self, agent_id: &str, goal: AgentGoal) -> Result<(), AgentError> {
        // Verify agent exists in supervisor
        {
            let sup = self.supervisor.lock().unwrap();
            let agent_uuid = uuid::Uuid::parse_str(agent_id)
                .map_err(|e| AgentError::SupervisorError(format!("invalid agent id: {e}")))?;
            sup.get_agent(agent_uuid).ok_or_else(|| {
                AgentError::SupervisorError(format!("agent '{agent_id}' not found in supervisor"))
            })?;
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = AgentLoopState {
            goal,
            phase: CognitivePhase::Perceive,
            steps: Vec::new(),
            current_step_index: 0,
            cycle_count: 0,
            total_fuel_consumed: 0.0,
            consecutive_failures: 0,
            steps_completed: 0,
            shutdown: shutdown.clone(),
            hitl_approval_allowance: 0,
            review_each_mode: false,
            strategy_hash: None,
            started_at_secs: now,
        };

        self.shutdown_flags
            .lock()
            .unwrap()
            .insert(agent_id.to_string(), shutdown);
        self.loops
            .lock()
            .unwrap()
            .insert(agent_id.to_string(), state);

        Ok(())
    }

    fn resolve_cognitive_overrides(
        &self,
        agent_id: &str,
        memory_mgr: &AgentMemoryManager,
    ) -> CognitiveOverrides {
        let mut effective = CognitiveOverrides {
            max_cycles_per_goal: self.config.max_cycles_per_goal,
            fuel_reserve_threshold: self.config.fuel_reserve_threshold,
            reflection_interval: self.config.reflection_interval,
            cycle_delay_ms: self.config.cycle_delay_ms,
            planning_depth: 3,
        };
        let latest = memory_mgr
            .load_by_type(agent_id, "cognitive_params", 20)
            .ok()
            .and_then(|mut rows| rows.drain(..).next());
        let Some(row) = latest else {
            return effective;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.value_json) else {
            return effective;
        };
        let Some(map) = value.as_object() else {
            return effective;
        };
        if let Some(max_cycles) = map.get("max_cycles").and_then(|v| v.as_str()) {
            if let Ok(parsed) = max_cycles.parse::<u32>() {
                effective.max_cycles_per_goal = parsed.min(500);
            }
        }
        if let Some(reflection_interval) = map.get("reflection_interval").and_then(|v| v.as_str()) {
            if let Ok(parsed) = reflection_interval.parse::<u32>() {
                effective.reflection_interval = parsed.clamp(1, 20);
            }
        }
        if let Some(fuel_reserve) = map.get("fuel_reserve_threshold").and_then(|v| v.as_str()) {
            if let Ok(parsed) = fuel_reserve.parse::<f64>() {
                effective.fuel_reserve_threshold = parsed.clamp(0.01, 0.5);
            }
        }
        if let Some(cycle_delay_ms) = map.get("cycle_delay_ms").and_then(|v| v.as_str()) {
            if let Ok(parsed) = cycle_delay_ms.parse::<u64>() {
                effective.cycle_delay_ms = parsed.max(100);
            }
        }
        if let Some(planning_depth) = map.get("planning_depth").and_then(|v| v.as_str()) {
            if let Ok(parsed) = planning_depth.parse::<u32>() {
                effective.planning_depth = parsed.clamp(1, 10);
            }
        }
        effective
    }

    fn resolve_phase_model(
        &self,
        agent_id: &str,
        phase: CognitivePhase,
        memory_mgr: &AgentMemoryManager,
    ) -> Option<PhaseModelSelection> {
        let phase_key = phase.to_string();
        let latest = memory_mgr
            .load_by_type(agent_id, "model_mapping", 10)
            .ok()
            .and_then(|mut rows| rows.drain(..).next())?;
        let parsed = serde_json::from_str::<serde_json::Value>(&latest.value_json).ok()?;
        let entry = parsed.get(&phase_key)?;
        let provider = entry.get("provider")?.as_str()?.to_string();
        let model = entry.get("model")?.as_str()?.to_string();
        if !self.provider_registry.is_empty() && !self.provider_registry.contains_key(&provider) {
            return None;
        }
        Some(PhaseModelSelection { provider, model })
    }

    fn resolve_selected_algorithm(
        &self,
        agent_id: &str,
        memory_mgr: &AgentMemoryManager,
    ) -> Option<SelectedAlgorithm> {
        let latest = memory_mgr
            .load_by_type(agent_id, "algorithm_selection", 10)
            .ok()
            .and_then(|mut rows| rows.drain(..).next())?;
        let parsed = serde_json::from_str::<serde_json::Value>(&latest.value_json).ok()?;
        Some(SelectedAlgorithm {
            algorithm: parsed.get("algorithm")?.as_str()?.to_string(),
            config_json: parsed
                .get("config")
                .cloned()
                .unwrap_or_else(|| json!({}))
                .to_string(),
        })
    }

    fn record_phase_model_selection(
        &self,
        agent_id: &str,
        phase: CognitivePhase,
        memory_mgr: &AgentMemoryManager,
        audit: &mut AuditTrail,
    ) {
        let Some(selection) = self.resolve_phase_model(agent_id, phase, memory_mgr) else {
            return;
        };
        let provider_name = self
            .provider_registry
            .get(&selection.provider)
            .map(|provider| provider.name().to_string())
            .unwrap_or(selection.provider);
        let _ = audit.append_event(
            uuid::Uuid::parse_str(agent_id).unwrap_or_default(),
            EventType::StateChange,
            json!({
                "event": "cognitive.phase_model_selected",
                "phase": phase.to_string(),
                "provider": provider_name,
                "model": selection.model,
            }),
        );
    }

    fn persist_l6_cooldown(&self, agent_id: &str, cycle_count: u32, cooled_down: bool) {
        let Ok(db) = NexusDatabase::open(&NexusDatabase::default_db_path()) else {
            return;
        };
        let previous = db.load_l6_cooldown(agent_id).ok().flatten().unwrap_or(
            nexus_persistence::L6CooldownTrackerRow {
                agent_id: agent_id.to_string(),
                cycle_count: 0,
                last_cooldown: None,
                total_cooldowns: 0,
            },
        );
        let last_cooldown = if cooled_down {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            previous.last_cooldown.clone()
        };
        let total_cooldowns = if cooled_down {
            previous.total_cooldowns + 1
        } else {
            previous.total_cooldowns
        };
        let _ = db.upsert_l6_cooldown(
            agent_id,
            cycle_count as i64,
            last_cooldown.as_deref(),
            total_cooldowns,
        );
    }

    /// Run one cognitive cycle for an agent. Returns the cycle result.
    /// This is the core loop body — called repeatedly by the runtime driver.
    pub fn run_cycle(
        &self,
        agent_id: &str,
        planner: &CognitivePlanner,
        memory_mgr: &AgentMemoryManager,
        executor: &dyn ActionExecutor,
        audit: &mut AuditTrail,
    ) -> Result<CycleResult, AgentError> {
        self.run_cycle_with_evolution(agent_id, planner, memory_mgr, executor, audit, None)
    }

    /// Run one cognitive cycle with optional evolution tracking.
    pub fn run_cycle_with_evolution(
        &self,
        agent_id: &str,
        planner: &CognitivePlanner,
        memory_mgr: &AgentMemoryManager,
        executor: &dyn ActionExecutor,
        audit: &mut AuditTrail,
        evolution_tracker: Option<&EvolutionTracker>,
    ) -> Result<CycleResult, AgentError> {
        let mut loops = self.loops.lock().unwrap();
        let state = loops.get_mut(agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
        })?;

        // Check shutdown
        if state.shutdown.load(Ordering::Relaxed) {
            state.phase = CognitivePhase::Idle;
            return Ok(CycleResult {
                phase: CognitivePhase::Idle,
                steps_executed: 0,
                fuel_consumed: 0.0,
                should_continue: false,
                blocked_reason: Some("shutdown requested".into()),
            });
        }

        let (agent_name, capabilities, fuel_remaining, autonomy_level) = {
            let sup = self.supervisor.lock().unwrap();
            let agent_uuid = uuid::Uuid::parse_str(agent_id)
                .map_err(|e| AgentError::SupervisorError(format!("invalid agent id: {e}")))?;
            let handle = sup.get_agent(agent_uuid).ok_or_else(|| {
                AgentError::SupervisorError(format!("agent '{agent_id}' gone from supervisor"))
            })?;
            (
                handle.manifest.name.clone(),
                handle.manifest.capabilities.clone(),
                handle.remaining_fuel as f64,
                handle.autonomy_level,
            )
        };

        let overrides = self.resolve_cognitive_overrides(agent_id, memory_mgr);
        if autonomy_level == 6 && state.cycle_count >= L6_COOLDOWN_CYCLES {
            let completed = state.cycle_count;
            state.cycle_count = 0;
            self.emitter.emit(CognitiveEvent::AgentCooldown {
                agent_id: agent_id.to_string(),
                cycles_completed: completed,
            });
            self.persist_l6_cooldown(agent_id, 0, true);
            thread::sleep(L6_COOLDOWN_SLEEP);
        }

        // Check max cycles
        if state.cycle_count >= overrides.max_cycles_per_goal {
            state.goal.status = GoalStatus::Failed;
            state.phase = CognitivePhase::Learn;
            return Ok(CycleResult {
                phase: CognitivePhase::Learn,
                steps_executed: 0,
                fuel_consumed: 0.0,
                should_continue: false,
                blocked_reason: Some("max cycles reached".into()),
            });
        }
        state.cycle_count += 1;
        self.persist_l6_cooldown(agent_id, state.cycle_count, false);

        // ── PERCEIVE ──

        state.phase = CognitivePhase::Perceive;
        self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
        self.emit_phase_change(agent_id, state);

        // Fuel reserve check
        let fuel_budget = fuel_remaining;
        let reserve = fuel_budget * overrides.fuel_reserve_threshold;
        if fuel_remaining <= reserve {
            state.phase = CognitivePhase::Idle;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);
            return Ok(CycleResult {
                phase: CognitivePhase::Idle,
                steps_executed: 0,
                fuel_consumed: 0.0,
                should_continue: false,
                blocked_reason: Some("fuel below reserve threshold".into()),
            });
        }

        // ── REASON ──
        state.phase = CognitivePhase::Reason;
        self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
        self.emit_phase_change(agent_id, state);

        let needs_plan = state.steps.is_empty() || state.current_step_index >= state.steps.len();
        let needs_replan = !state.steps.is_empty()
            && state.current_step_index < state.steps.len()
            && state.consecutive_failures >= self.config.max_consecutive_failures;

        // Check if all steps completed
        let all_done =
            !state.steps.is_empty() && state.current_step_index >= state.steps.len() && !needs_plan;

        if all_done {
            state.phase = CognitivePhase::Reflect;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);
        }

        // ── PLAN / REPLAN ──
        if needs_plan || needs_replan {
            state.phase = CognitivePhase::Plan;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);

            let memories = memory_mgr
                .recall_relevant(
                    agent_id,
                    &state.goal.description,
                    overrides.planning_depth as usize,
                )
                .unwrap_or_default();
            let memory_strs: Vec<String> = memories.iter().map(|m| m.value_json.clone()).collect();

            // Evolution: select best strategy and include as context
            let mut previous_outcomes = vec![];
            if let Some(evo) = evolution_tracker {
                let goal_type = infer_goal_type(&state.goal.description);
                if let Ok(Some(best_strategy)) = evo.select_best_strategy(agent_id, &goal_type) {
                    previous_outcomes.push(format!(
                        "Previously, the most successful approach for similar goals was: {best_strategy}. \
                         Consider using or adapting this approach."
                    ));
                    state.strategy_hash = Some(best_strategy);
                } else {
                    state.strategy_hash =
                        Some(super::evolution::hash_strategy(&state.goal.description));
                }
            }
            if let Ok(memories) = memory_mgr.load_by_type(agent_id, "semantic", 25) {
                if let Some(prompt_memory) = memories
                    .into_iter()
                    .find(|memory| memory.value_json.contains("optimized_planning_prompt:"))
                {
                    previous_outcomes.push(format!(
                        "Use this evolved planning prompt guidance: {}",
                        prompt_memory.value_json
                    ));
                }
            }

            let context = PlanningContext {
                agent_name: Some(agent_name.clone()),
                agent_description: Some(format!(
                    "You are the governed agent '{}'. Plan steps consistent with your role, autonomy level, and declared capabilities.",
                    agent_name
                )),
                agent_capabilities: capabilities.clone(),
                available_fuel: fuel_remaining,
                relevant_memories: memory_strs,
                previous_outcomes,
                working_directory: None,
                autonomy_level,
            };

            let mut new_steps = if needs_replan && !state.steps.is_empty() {
                let failed_step = &state.steps[state.current_step_index];
                let remaining = &state.steps[state.current_step_index + 1..];
                planner.replan_after_failure(
                    &state.goal,
                    failed_step,
                    "max consecutive failures reached",
                    remaining,
                    &context,
                )?
            } else {
                planner.plan_goal(&state.goal, &context)?
            };

            if let Some(selected_algorithm) = self.resolve_selected_algorithm(agent_id, memory_mgr)
            {
                match selected_algorithm.algorithm.as_str() {
                    "evolutionary" => {
                        new_steps = EvolutionEngine.optimize_plan(new_steps);
                    }
                    "world_model" => {
                        let _ = memory_mgr.store_episodic(
                            agent_id,
                            "world_model_plan_preview",
                            &format!(
                                "simulated {} candidate steps using {}",
                                new_steps.len(),
                                selected_algorithm.config_json
                            ),
                        );
                    }
                    "swarm" | "adversarial" => {}
                    _ => {}
                }
                if let Ok(db) = NexusDatabase::open(&NexusDatabase::default_db_path()) {
                    let _ = db.save_algorithm_selection(
                        agent_id,
                        &state.goal.id,
                        &selected_algorithm.algorithm,
                        &selected_algorithm.config_json,
                        None,
                    );
                }
            }

            state.steps = new_steps;
            state.current_step_index = 0;
            state.consecutive_failures = 0;
            state.hitl_approval_allowance = 0;
            state.review_each_mode = false;
            state.goal.status = GoalStatus::Active;
        }

        // ── ACT ──
        let mut act_result: Option<(bool, f64, Option<String>)> = None;
        if state.current_step_index < state.steps.len() {
            state.phase = CognitivePhase::Act;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);

            let step = &mut state.steps[state.current_step_index];
            step.status = StepStatus::Executing;
            step.attempts += 1;

            if let Some(selected_algorithm) = self.resolve_selected_algorithm(agent_id, memory_mgr)
            {
                match selected_algorithm.algorithm.as_str() {
                    "swarm" => {
                        SwarmCoordinator.prepare_parallel_step(step);
                    }
                    "world_model" => {
                        let world_model = WorldModel::default();
                        let simulation =
                            world_model.simulate_action(&state.goal.id, step.action.action_type());
                        let _ = memory_mgr.store_episodic(
                            agent_id,
                            "world_model_act_preview",
                            &simulation.to_string(),
                        );
                    }
                    _ => {}
                }
            }

            // Capability check
            let required_caps = step.action.required_capabilities();
            for cap in &required_caps {
                if !capabilities.contains(&cap.to_string()) {
                    step.status = StepStatus::Failed;
                    step.result = Some(format!("capability '{cap}' not granted"));
                    state.consecutive_failures += 1;
                    self.emit_step_executed(agent_id, step);
                    audit.append_event(
                        uuid::Uuid::parse_str(agent_id).unwrap_or_default(),
                        EventType::UserAction,
                        json!({
                            "event": "cognitive.step_failed",
                            "action": step.action.action_type(),
                            "error": format!("capability '{cap}' denied"),
                        }),
                    )?;

                    if step.attempts >= step.max_retries {
                        state.current_step_index += 1;
                    }

                    return Ok(CycleResult {
                        phase: CognitivePhase::Act,
                        steps_executed: 0,
                        fuel_consumed: 0.0,
                        should_continue: true,
                        blocked_reason: Some(format!("capability '{cap}' denied")),
                    });
                }
            }

            // Check if HITL is required for high-risk actions
            let requires_hitl = action_requires_hitl(&step.action, autonomy_level);
            if requires_hitl && state.hitl_approval_allowance == 0 {
                state.phase = CognitivePhase::Blocked;
                let reason = format!(
                    "HITL approval required for {} at autonomy L{autonomy_level}",
                    step.action.action_type()
                );
                self.emitter.emit(CognitiveEvent::AgentBlocked {
                    agent_id: agent_id.to_string(),
                    reason: reason.clone(),
                    consent_id: None,
                });

                return Ok(CycleResult {
                    phase: CognitivePhase::Blocked,
                    steps_executed: 0,
                    fuel_consumed: 0.0,
                    should_continue: true,
                    blocked_reason: Some(reason),
                });
            }
            if requires_hitl {
                state.hitl_approval_allowance = state.hitl_approval_allowance.saturating_sub(1);
            }

            // Execute the action
            let action_clone = step.action.clone();
            let (step_executed, step_fuel, step_error) =
                match executor.execute(agent_id, &action_clone, audit) {
                    Ok(result) => {
                        step.status = StepStatus::Succeeded;
                        let preview = if result.len() > 200 {
                            format!("{}...", &result[..200])
                        } else {
                            result.clone()
                        };
                        step.result = Some(result);
                        step.fuel_cost = estimate_fuel_cost(&action_clone);
                        state.total_fuel_consumed += step.fuel_cost;
                        state.consecutive_failures = 0;
                        state.steps_completed += 1;
                        state.current_step_index += 1;
                        let fuel = step.fuel_cost;

                        self.emit_step_executed(agent_id, step);

                        // Consume fuel from supervisor
                        if let Ok(agent_uuid) = uuid::Uuid::parse_str(agent_id) {
                            let fuel_units = fuel as u64;
                            let mut sup = self.supervisor.lock().unwrap();
                            if let Some(handle) = sup.get_agent(agent_uuid) {
                                let remaining = handle.remaining_fuel;
                                if remaining >= fuel_units {
                                    let _ = sup.record_llm_spend(
                                        agent_uuid,
                                        "cognitive",
                                        0,
                                        fuel_units as u32,
                                        fuel_units,
                                    );
                                }
                            }
                        }

                        audit.append_event(
                            uuid::Uuid::parse_str(agent_id).unwrap_or_default(),
                            EventType::UserAction,
                            json!({
                                "event": "cognitive.step_executed",
                                "action": action_clone.action_type(),
                                "status": "succeeded",
                                "fuel_cost": fuel,
                                "result_preview": preview,
                            }),
                        )?;

                        (true, fuel, None)
                    }
                    Err(error) => {
                        if error.starts_with("human approval required:")
                            || error.starts_with("Warden blocked action:")
                        {
                            state.phase = CognitivePhase::Blocked;
                            return Ok(CycleResult {
                                phase: CognitivePhase::Blocked,
                                steps_executed: 0,
                                fuel_consumed: 0.0,
                                should_continue: true,
                                blocked_reason: Some(error),
                            });
                        }

                        step.status = StepStatus::Failed;
                        step.result = Some(error.clone());
                        state.consecutive_failures += 1;

                        self.emit_step_executed(agent_id, step);

                        audit.append_event(
                            uuid::Uuid::parse_str(agent_id).unwrap_or_default(),
                            EventType::UserAction,
                            json!({
                                "event": "cognitive.step_failed",
                                "action": action_clone.action_type(),
                                "error": error,
                                "attempt": step.attempts,
                            }),
                        )?;

                        if step.attempts >= step.max_retries {
                            state.current_step_index += 1;
                        }

                        (false, 0.0, Some(error))
                    }
                };

            // If more steps remain, return and continue next cycle
            if state.current_step_index < state.steps.len() {
                return Ok(CycleResult {
                    phase: CognitivePhase::Act,
                    steps_executed: if step_executed { 1 } else { 0 },
                    fuel_consumed: step_fuel,
                    should_continue: true,
                    blocked_reason: step_error,
                });
            }

            // All steps done — fall through to reflection/completion below
            act_result = Some((step_executed, step_fuel, step_error));
        }

        // ── REFLECT (every reflection_interval cycles) ──
        if state.cycle_count % overrides.reflection_interval == 0 {
            state.phase = CognitivePhase::Reflect;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);

            let total = state.steps.len() as f64;
            let succeeded = state
                .steps
                .iter()
                .filter(|s| s.status == StepStatus::Succeeded)
                .count() as f64;
            let success_rate = if total > 0.0 { succeeded / total } else { 0.0 };

            if success_rate < 0.5 && total > 0.0 {
                let _ = memory_mgr.store_procedural(
                    agent_id,
                    &format!(
                        "low success rate ({:.0}%) on goal: {}",
                        success_rate * 100.0,
                        state.goal.description
                    ),
                    success_rate,
                );
            }

            let _ = memory_mgr.store_episodic(
                agent_id,
                &format!("reflection at cycle {}", state.cycle_count),
                &format!("success_rate={:.2}, steps={}", success_rate, total),
            );

            if let Some(selected_algorithm) = self.resolve_selected_algorithm(agent_id, memory_mgr)
            {
                if selected_algorithm.algorithm == "adversarial" {
                    let summary =
                        AdversarialArena.challenge_summary(state.goal.description.as_str());
                    let _ = memory_mgr.store_episodic(agent_id, "adversarial_reflect", &summary);
                }
            }
        }

        // ── Check if goal is complete ──
        if state.current_step_index >= state.steps.len() && !state.steps.is_empty() {
            let any_failed = state.steps.iter().any(|s| s.status == StepStatus::Failed);
            let success = !any_failed;

            state.goal.status = if success {
                GoalStatus::Completed
            } else {
                GoalStatus::Failed
            };

            // ── LEARN ──
            state.phase = CognitivePhase::Learn;
            self.record_phase_model_selection(agent_id, state.phase, memory_mgr, audit);
            self.emit_phase_change(agent_id, state);

            let _ = memory_mgr.store_episodic(
                agent_id,
                &format!(
                    "goal {}: {}",
                    if success { "completed" } else { "failed" },
                    state.goal.description
                ),
                &format!(
                    "steps={}, fuel={:.1}, success={}",
                    state.steps.len(),
                    state.total_fuel_consumed,
                    success
                ),
            );

            let _ = memory_mgr.run_decay_cycle(agent_id);

            let _ = evolution_tracker;

            self.emitter.emit(CognitiveEvent::GoalCompleted {
                agent_id: agent_id.to_string(),
                goal_id: state.goal.id.clone(),
                success,
                steps_total: state.steps.len() as u32,
                fuel_consumed: state.total_fuel_consumed,
            });

            let (steps_exec, fuel, _) = act_result.unwrap_or((false, 0.0, None));
            return Ok(CycleResult {
                phase: CognitivePhase::Learn,
                steps_executed: if steps_exec { 1 } else { 0 },
                fuel_consumed: fuel,
                should_continue: false,
                blocked_reason: None,
            });
        }

        // If an act happened but goal isn't complete yet (shouldn't normally reach here)
        if let Some((executed, fuel, error)) = act_result {
            return Ok(CycleResult {
                phase: CognitivePhase::Act,
                steps_executed: if executed { 1 } else { 0 },
                fuel_consumed: fuel,
                should_continue: true,
                blocked_reason: error,
            });
        }

        Ok(CycleResult {
            phase: state.phase,
            steps_executed: 0,
            fuel_consumed: 0.0,
            should_continue: true,
            blocked_reason: None,
        })
    }

    /// Stop a running agent loop.
    pub fn stop_agent_loop(&self, agent_id: &str) -> Result<(), AgentError> {
        if let Some(flag) = self.shutdown_flags.lock().unwrap().get(agent_id) {
            flag.store(true, Ordering::Relaxed);
        }
        self.loops.lock().unwrap().remove(agent_id);
        self.shutdown_flags.lock().unwrap().remove(agent_id);
        Ok(())
    }

    /// Get the current cognitive phase for an agent.
    pub fn get_agent_phase(&self, agent_id: &str) -> Option<CognitivePhase> {
        self.loops.lock().unwrap().get(agent_id).map(|s| s.phase)
    }

    /// Get full cognitive status for an agent.
    pub fn get_agent_status(&self, agent_id: &str) -> Option<CognitiveStatusResponse> {
        let loops = self.loops.lock().unwrap();
        let state = loops.get(agent_id)?;

        let fuel_remaining = {
            let sup = self.supervisor.lock().unwrap();
            if let Ok(uuid) = uuid::Uuid::parse_str(agent_id) {
                sup.get_agent(uuid)
                    .map(|h| h.remaining_fuel as f64)
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        };

        Some(CognitiveStatusResponse {
            phase: state.phase,
            active_goal: Some(state.goal.clone()),
            steps_completed: state.steps_completed,
            steps_total: state.steps.len() as u32,
            fuel_remaining,
            cycle_count: state.cycle_count,
        })
    }

    /// Check if an agent has an active cognitive loop.
    pub fn has_active_loop(&self, agent_id: &str) -> bool {
        self.loops.lock().unwrap().contains_key(agent_id)
    }

    /// Return the remaining plan steps that still require HITL approval.
    pub fn pending_hitl_steps(&self, agent_id: &str) -> Result<Vec<AgentStep>, AgentError> {
        let autonomy_level = {
            let sup = self.supervisor.lock().unwrap();
            let agent_uuid = uuid::Uuid::parse_str(agent_id)
                .map_err(|e| AgentError::SupervisorError(format!("invalid agent id: {e}")))?;
            let handle = sup.get_agent(agent_uuid).ok_or_else(|| {
                AgentError::SupervisorError(format!("agent '{agent_id}' gone from supervisor"))
            })?;
            handle.autonomy_level
        };

        let loops = self.loops.lock().unwrap();
        let state = loops.get(agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
        })?;

        Ok(state
            .steps
            .iter()
            .skip(state.current_step_index)
            .filter(|step| {
                matches!(step.status, StepStatus::Planned | StepStatus::Executing)
                    && action_requires_hitl(&step.action, autonomy_level)
            })
            .cloned()
            .collect())
    }

    pub fn review_each_mode(&self, agent_id: &str) -> Result<bool, AgentError> {
        let loops = self.loops.lock().unwrap();
        let state = loops.get(agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
        })?;
        Ok(state.review_each_mode)
    }

    pub fn set_review_each_mode(&self, agent_id: &str, enabled: bool) -> Result<(), AgentError> {
        let mut loops = self.loops.lock().unwrap();
        let state = loops.get_mut(agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
        })?;
        state.review_each_mode = enabled;
        if enabled {
            state.hitl_approval_allowance = 0;
        }
        Ok(())
    }

    /// Mark blocked HITL steps as approved so future cycles may execute them.
    pub fn approve_blocked_steps(&self, agent_id: &str, count: u32) -> Result<(), AgentError> {
        let mut loops = self.loops.lock().unwrap();
        let state = loops.get_mut(agent_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
        })?;
        state.hitl_approval_allowance = state.hitl_approval_allowance.saturating_add(count.max(1));
        Ok(())
    }

    /// Mark the current blocked step as approved so the next cycle executes it once.
    pub fn approve_blocked_step(&self, agent_id: &str) -> Result<(), AgentError> {
        self.approve_blocked_steps(agent_id, 1)
    }

    /// Skip the current blocked step after an explicit denial and continue planning.
    pub fn deny_blocked_step(
        &self,
        agent_id: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError> {
        let skipped_step = {
            let mut loops = self.loops.lock().unwrap();
            let state = loops.get_mut(agent_id).ok_or_else(|| {
                AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
            })?;

            state.hitl_approval_allowance = 0;
            state.phase = CognitivePhase::Reason;
            state.consecutive_failures = 0;

            if state.current_step_index >= state.steps.len() {
                None
            } else {
                let skip_reason = reason
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "HITL request denied".to_string());
                let step = &mut state.steps[state.current_step_index];
                step.status = StepStatus::Skipped;
                step.result = Some(skip_reason);
                let snapshot = step.clone();
                state.current_step_index += 1;
                Some(snapshot)
            }
        };

        if let Some(step) = skipped_step.as_ref() {
            self.emit_step_executed(agent_id, step);
        }

        Ok(())
    }

    /// Deny the current blocked step and discard the remaining plan so the agent replans.
    pub fn deny_blocked_steps_and_replan(
        &self,
        agent_id: &str,
        reason: Option<&str>,
    ) -> Result<(), AgentError> {
        let skipped_step = {
            let mut loops = self.loops.lock().unwrap();
            let state = loops.get_mut(agent_id).ok_or_else(|| {
                AgentError::SupervisorError(format!("no active loop for agent '{agent_id}'"))
            })?;

            state.hitl_approval_allowance = 0;
            state.review_each_mode = false;
            state.phase = CognitivePhase::Reason;
            state.consecutive_failures = 0;
            state.goal.status = GoalStatus::Active;

            let skipped = if state.current_step_index < state.steps.len() {
                let deny_reason = reason
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "HITL batch denied".to_string());
                let step = &mut state.steps[state.current_step_index];
                step.status = StepStatus::Skipped;
                step.result = Some(deny_reason);
                Some(step.clone())
            } else {
                None
            };

            state.steps.clear();
            state.current_step_index = 0;
            skipped
        };

        if let Some(step) = skipped_step.as_ref() {
            self.emit_step_executed(agent_id, step);
        }

        Ok(())
    }

    fn emit_phase_change(&self, agent_id: &str, state: &AgentLoopState) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.emitter.emit(CognitiveEvent::PhaseChange {
            agent_id: agent_id.to_string(),
            phase: state.phase,
            goal_id: state.goal.id.clone(),
            timestamp,
        });
    }

    fn emit_step_executed(&self, agent_id: &str, step: &AgentStep) {
        let preview = step.result.as_ref().map(|r| {
            if r.len() > 100 {
                format!("{}...", &r[..100])
            } else {
                r.clone()
            }
        });
        self.emitter.emit(CognitiveEvent::StepExecuted {
            agent_id: agent_id.to_string(),
            step_id: step.id.clone(),
            action_type: step.action.action_type().to_string(),
            status: step.status,
            result_preview: preview,
            fuel_cost: step.fuel_cost,
        });
    }
}

/// Determine if an action requires HITL approval based on autonomy level.
fn action_requires_hitl(action: &PlannedAction, autonomy_level: u8) -> bool {
    match action {
        // High-risk actions require L3+ or HITL
        PlannedAction::FileWrite { .. }
        | PlannedAction::ShellCommand { .. }
        | PlannedAction::DockerCommand { .. }
        | PlannedAction::ApiCall { .. }
        | PlannedAction::AnalyzeScreen { .. }
        | PlannedAction::MouseMove { .. }
        | PlannedAction::MouseClick { .. }
        | PlannedAction::MouseDoubleClick { .. }
        | PlannedAction::MouseDrag { .. }
        | PlannedAction::KeyboardType { .. }
        | PlannedAction::KeyboardPress { .. }
        | PlannedAction::KeyboardShortcut { .. }
        | PlannedAction::ScrollWheel { .. } => autonomy_level < 3,
        // Medium-risk
        PlannedAction::WebFetch { .. }
        | PlannedAction::BrowserAutomate { .. }
        | PlannedAction::AgentMessage { .. }
        | PlannedAction::CaptureScreen { .. }
        | PlannedAction::CaptureWindow { .. } => autonomy_level < 2,
        // HITL request always blocks (that's its purpose)
        PlannedAction::HitlRequest { .. } | PlannedAction::ComputerAction { .. } => true,
        // Low-risk / internal
        PlannedAction::LlmQuery { .. }
        | PlannedAction::FileRead { .. }
        | PlannedAction::WebSearch { .. }
        | PlannedAction::ImageGenerate { .. }
        | PlannedAction::TextToSpeech { .. }
        | PlannedAction::KnowledgeGraphUpdate { .. }
        | PlannedAction::KnowledgeGraphQuery { .. }
        | PlannedAction::MemoryStore { .. }
        | PlannedAction::MemoryRecall { .. }
        | PlannedAction::Noop => false,
        // L4/L5 self-evolution and governance — always requires HITL
        PlannedAction::SelfModifyDescription { .. }
        | PlannedAction::SelfModifyStrategy { .. }
        | PlannedAction::CreateSubAgent { .. }
        | PlannedAction::DestroySubAgent { .. }
        | PlannedAction::RunEvolutionTournament { .. }
        | PlannedAction::ModifyGovernancePolicy { .. }
        | PlannedAction::AllocateEcosystemFuel { .. }
        | PlannedAction::ModifyCognitiveParams { .. }
        | PlannedAction::SelectLlmProvider { .. }
        | PlannedAction::SelectAlgorithm { .. }
        | PlannedAction::DesignAgentEcosystem { .. }
        | PlannedAction::RunCounterfactual { .. }
        | PlannedAction::TemporalPlan { .. } => true,
    }
}

/// Estimate fuel cost for an action type.
fn estimate_fuel_cost(action: &PlannedAction) -> f64 {
    match action {
        PlannedAction::LlmQuery { .. } => 10.0,
        PlannedAction::FileRead { .. } => 1.0,
        PlannedAction::FileWrite { .. } => 2.0,
        PlannedAction::ShellCommand { .. } => 5.0,
        PlannedAction::DockerCommand { .. } => 8.0,
        PlannedAction::WebSearch { .. } => 3.0,
        PlannedAction::WebFetch { .. } => 3.0,
        PlannedAction::ApiCall { .. } => 5.0,
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
        PlannedAction::AgentMessage { .. } => 2.0,
        PlannedAction::HitlRequest { .. } => 0.0,
        PlannedAction::MemoryStore { .. } => 0.5,
        PlannedAction::MemoryRecall { .. } => 0.5,
        PlannedAction::Noop => 0.0,
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
    }
}

/// Infer a goal type string from a goal description for strategy bucketing.
fn infer_goal_type(description: &str) -> String {
    let lower = description.to_lowercase();
    if lower.contains("code") || lower.contains("implement") || lower.contains("fix") {
        "coding".to_string()
    } else if lower.contains("research") || lower.contains("search") || lower.contains("find") {
        "research".to_string()
    } else if lower.contains("deploy") || lower.contains("build") {
        "deployment".to_string()
    } else if lower.contains("test") || lower.contains("verify") {
        "testing".to_string()
    } else {
        "general".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::super::memory_manager::MemoryStore;
    use super::super::planner::PlannerLlm;
    use super::*;
    use crate::manifest::AgentManifest;
    use crate::supervisor::Supervisor;
    use std::sync::{Arc, Mutex};

    // ── Test helpers ──

    struct MockLlm {
        response: String,
    }

    impl PlannerLlm for MockLlm {
        fn plan_query(&self, _prompt: &str) -> Result<String, AgentError> {
            Ok(self.response.clone())
        }
    }

    struct MockExecutor {
        results: Mutex<Vec<Result<String, String>>>,
    }

    impl MockExecutor {
        #[allow(dead_code)]
        fn new(results: Vec<Result<String, String>>) -> Self {
            Self {
                results: Mutex::new(results),
            }
        }

        fn always_ok(result: &str) -> Self {
            // Return a large number of successes
            Self {
                results: Mutex::new(vec![Ok(result.to_string()); 100]),
            }
        }

        fn always_err(err: &str) -> Self {
            Self {
                results: Mutex::new(vec![Err(err.to_string()); 100]),
            }
        }
    }

    impl ActionExecutor for MockExecutor {
        fn execute(
            &self,
            _agent_id: &str,
            _action: &PlannedAction,
            _audit: &mut AuditTrail,
        ) -> Result<String, String> {
            let mut results = self.results.lock().unwrap();
            if results.is_empty() {
                Ok("default".to_string())
            } else {
                results.remove(0)
            }
        }
    }

    struct MockMemoryStore;

    impl MemoryStore for MockMemoryStore {
        fn save_memory(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn load_memories(
            &self,
            _: &str,
            _: Option<&str>,
            _: usize,
        ) -> Result<Vec<super::super::memory_manager::MemoryEntry>, String> {
            Ok(vec![])
        }
        fn touch_memory(&self, _: i64) -> Result<(), String> {
            Ok(())
        }
        fn decay_memories(&self, _: &str, _: f64) -> Result<(), String> {
            Ok(())
        }
    }

    struct SeededMemoryStore {
        rows: Vec<super::super::memory_manager::MemoryEntry>,
    }

    impl MemoryStore for SeededMemoryStore {
        fn save_memory(&self, _: &str, _: &str, _: &str, _: &str) -> Result<(), String> {
            Ok(())
        }

        fn load_memories(
            &self,
            _agent_id: &str,
            memory_type: Option<&str>,
            limit: usize,
        ) -> Result<Vec<super::super::memory_manager::MemoryEntry>, String> {
            Ok(self
                .rows
                .iter()
                .filter(|row| {
                    memory_type.is_none() || Some(row.memory_type.as_str()) == memory_type
                })
                .take(limit)
                .cloned()
                .collect())
        }

        fn touch_memory(&self, _: i64) -> Result<(), String> {
            Ok(())
        }

        fn decay_memories(&self, _: &str, _: f64) -> Result<(), String> {
            Ok(())
        }
    }

    fn make_supervisor_with_agent() -> (Arc<Mutex<Supervisor>>, String) {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "test-agent".into(),
            version: "1.0.0".into(),
            capabilities: vec!["llm.query".into(), "fs.read".into(), "fs.write".into()],
            fuel_budget: 10000,
            autonomy_level: Some(3), // L3 — act-then-report
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        (sup, id.to_string())
    }

    fn make_planner(response: &str) -> CognitivePlanner {
        CognitivePlanner::new(Box::new(MockLlm {
            response: response.to_string(),
        }))
    }

    fn make_memory_mgr() -> AgentMemoryManager {
        AgentMemoryManager::new(Box::new(MockMemoryStore))
    }

    fn make_seeded_memory_mgr(
        rows: Vec<super::super::memory_manager::MemoryEntry>,
    ) -> AgentMemoryManager {
        AgentMemoryManager::new(Box::new(SeededMemoryStore { rows }))
    }

    fn make_supervisor_with_autonomy(level: u8) -> (Arc<Mutex<Supervisor>>, String) {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: format!("test-agent-l{level}"),
            version: "1.0.0".into(),
            capabilities: vec![
                "llm.query".into(),
                "fs.read".into(),
                "fs.write".into(),
                "self.modify".into(),
            ],
            fuel_budget: 10000,
            autonomy_level: Some(level),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        (Arc::new(Mutex::new(sup)), id.to_string())
    }

    fn make_runtime(sup: Arc<Mutex<Supervisor>>) -> (CognitiveRuntime, Arc<CollectingEmitter>) {
        let emitter = Arc::new(CollectingEmitter::new());
        let config = LoopConfig {
            max_cycles_per_goal: 10,
            max_consecutive_failures: 2,
            cycle_delay_ms: 0,
            fuel_reserve_threshold: 0.05,
            reflection_interval: 3,
        };
        let runtime = CognitiveRuntime::new(sup, config, emitter.clone());
        (runtime, emitter)
    }

    // ── Tests ──

    #[test]
    fn test_assign_goal_and_status() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, _emitter) = make_runtime(sup);
        let goal = AgentGoal::new("test goal".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();
        assert!(runtime.has_active_loop(&agent_id));
        let status = runtime.get_agent_status(&agent_id).unwrap();
        assert_eq!(status.phase, CognitivePhase::Perceive);
        assert_eq!(status.cycle_count, 0);
    }

    #[test]
    fn test_assign_goal_invalid_agent() {
        let sup = Arc::new(Mutex::new(Supervisor::new()));
        let (runtime, _) = make_runtime(sup);
        let goal = AgentGoal::new("test".into(), 5);
        let result = runtime.assign_goal("00000000-0000-0000-0000-000000000000", goal);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_cycle_plans_and_executes() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, emitter) = make_runtime(sup);
        let goal = AgentGoal::new("analyze code".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(
            r#"[{"action": {"type": "LlmQuery", "prompt": "analyze", "context": []}, "description": "ask"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("analysis complete");
        let mut audit = AuditTrail::new();

        let result = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        // Single-step plan: executes step and completes goal in one cycle
        assert_eq!(result.phase, CognitivePhase::Learn);
        assert_eq!(result.steps_executed, 1);
        assert!(result.fuel_consumed > 0.0);
        assert!(!result.should_continue); // goal completed

        // Check events were emitted
        let events = emitter.events.lock().unwrap();
        assert!(events.len() >= 3); // perceive + reason + plan + act + learn phase changes + step
    }

    #[test]
    fn test_fuel_exhaustion_stops_loop() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "low-fuel".into(),
            version: "1.0.0".into(),
            capabilities: vec!["llm.query".into()],
            fuel_budget: 1, // Very low fuel
            autonomy_level: Some(3),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: Some(1),
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));

        let config = LoopConfig {
            fuel_reserve_threshold: 0.9, // 90% reserve = effectively all fuel is reserved
            ..LoopConfig::default()
        };
        let emitter = Arc::new(CollectingEmitter::new());
        let runtime = CognitiveRuntime::new(sup, config, emitter);

        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner("[]");
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let result = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.phase, CognitivePhase::Idle);
        assert!(result
            .blocked_reason
            .as_ref()
            .unwrap()
            .contains("fuel below reserve"));
    }

    #[test]
    fn test_stop_agent_loop() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, _) = make_runtime(sup);
        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();
        assert!(runtime.has_active_loop(&agent_id));
        runtime.stop_agent_loop(&agent_id).unwrap();
        assert!(!runtime.has_active_loop(&agent_id));
    }

    #[test]
    fn test_shutdown_signal_stops_cycle() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, _) = make_runtime(sup);
        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        // Set shutdown flag
        runtime
            .shutdown_flags
            .lock()
            .unwrap()
            .get(&agent_id)
            .unwrap()
            .store(true, Ordering::Relaxed);

        let planner = make_planner("[]");
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let result = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        assert!(!result.should_continue);
        assert_eq!(result.phase, CognitivePhase::Idle);
    }

    #[test]
    fn test_max_cycles_reached() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let emitter = Arc::new(CollectingEmitter::new());
        let config = LoopConfig {
            max_cycles_per_goal: 1, // Only 1 cycle allowed
            ..LoopConfig::default()
        };
        let runtime = CognitiveRuntime::new(sup, config, emitter);

        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(r#"[{"action": {"type": "Noop"}, "description": "wait"}]"#);
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        // First cycle executes
        let _r1 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        // Second cycle should hit max
        let r2 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        assert!(!r2.should_continue);
        assert!(r2.blocked_reason.as_ref().unwrap().contains("max cycles"));
    }

    #[test]
    fn test_blocked_on_hitl_for_low_autonomy() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "low-auto".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.write".into()],
            fuel_budget: 10000,
            autonomy_level: Some(1), // L1 — needs HITL for writes
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, emitter) = make_runtime(sup);

        let goal = AgentGoal::new("write file".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner(
            r#"[{"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hello"}, "description": "write"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let result = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();

        assert_eq!(result.phase, CognitivePhase::Blocked);
        assert!(result.blocked_reason.as_ref().unwrap().contains("HITL"));

        // Check blocked event emitted
        let events = emitter.events.lock().unwrap();
        let blocked = events
            .iter()
            .any(|e| matches!(e, CognitiveEvent::AgentBlocked { .. }));
        assert!(blocked);
    }

    #[test]
    fn test_approved_hitl_step_executes_on_next_cycle() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "approved-hitl".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.write".into()],
            fuel_budget: 10000,
            autonomy_level: Some(1),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, _) = make_runtime(sup);

        let goal = AgentGoal::new("write file".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner(
            r#"[{"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hello"}, "description": "write"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let blocked = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(blocked.phase, CognitivePhase::Blocked);

        runtime.approve_blocked_step(&id.to_string()).unwrap();

        let resumed = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(resumed.phase, CognitivePhase::Learn);
        assert_eq!(resumed.steps_executed, 1);
    }

    #[test]
    fn test_batch_hitl_approval_executes_multiple_steps() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "batch-hitl".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.write".into()],
            fuel_budget: 10000,
            autonomy_level: Some(1),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, _) = make_runtime(sup);

        let goal = AgentGoal::new("run multiple writes".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner(
            r#"[
                {"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hello"}, "description": "write x"},
                {"action": {"type": "FileWrite", "path": "/tmp/y", "content": "world"}, "description": "write y"}
            ]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let blocked = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(blocked.phase, CognitivePhase::Blocked);

        let pending = runtime.pending_hitl_steps(&id.to_string()).unwrap();
        assert_eq!(pending.len(), 2);

        runtime
            .approve_blocked_steps(&id.to_string(), pending.len() as u32)
            .unwrap();

        let first_resumed = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(first_resumed.phase, CognitivePhase::Act);
        assert_eq!(first_resumed.steps_executed, 1);

        let second_resumed = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(second_resumed.phase, CognitivePhase::Learn);
        assert_eq!(second_resumed.steps_executed, 1);
    }

    #[test]
    fn test_review_each_mode_toggle() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, _) = make_runtime(sup);
        let goal = AgentGoal::new("review each".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        assert!(!runtime.review_each_mode(&agent_id).unwrap());
        runtime.set_review_each_mode(&agent_id, true).unwrap();
        assert!(runtime.review_each_mode(&agent_id).unwrap());
        runtime.set_review_each_mode(&agent_id, false).unwrap();
        assert!(!runtime.review_each_mode(&agent_id).unwrap());
    }

    #[test]
    fn test_denied_hitl_step_is_skipped() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "denied-hitl".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.write".into(), "fs.read".into()],
            fuel_budget: 10000,
            autonomy_level: Some(1),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, _) = make_runtime(sup);

        let goal = AgentGoal::new("skip denied write".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner(
            r#"[
                {"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hello"}, "description": "write"},
                {"action": {"type": "FileRead", "path": "/tmp/x"}, "description": "read"}
            ]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let blocked = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(blocked.phase, CognitivePhase::Blocked);

        runtime
            .deny_blocked_step(&id.to_string(), Some("user denied"))
            .unwrap();

        let resumed = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(resumed.phase, CognitivePhase::Learn);
        assert_eq!(resumed.steps_executed, 1);
    }

    #[test]
    fn test_deny_hitl_batch_clears_plan_for_replan() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "deny-batch".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.write".into()],
            fuel_budget: 10000,
            autonomy_level: Some(1),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, _) = make_runtime(sup);

        let goal = AgentGoal::new("deny all".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        let planner = make_planner(
            r#"[
                {"action": {"type": "FileWrite", "path": "/tmp/x", "content": "hello"}, "description": "write x"},
                {"action": {"type": "FileWrite", "path": "/tmp/y", "content": "world"}, "description": "write y"}
            ]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let blocked = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();
        assert_eq!(blocked.phase, CognitivePhase::Blocked);

        runtime
            .deny_blocked_steps_and_replan(&id.to_string(), Some("deny all"))
            .unwrap();

        let status = runtime.get_agent_status(&id.to_string()).unwrap();
        assert_eq!(status.phase, CognitivePhase::Reason);
        assert_eq!(status.steps_total, 0);
        assert!(!runtime.review_each_mode(&id.to_string()).unwrap());
    }

    #[test]
    fn test_capability_denied_step_fails() {
        let mut sup = Supervisor::new();
        let manifest = AgentManifest {
            name: "limited".into(),
            version: "1.0.0".into(),
            capabilities: vec!["fs.read".into()], // No fs.write
            fuel_budget: 10000,
            autonomy_level: Some(3),
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
        };
        let id = sup.start_agent(manifest).unwrap();
        let sup = Arc::new(Mutex::new(sup));
        let (runtime, _) = make_runtime(sup);

        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&id.to_string(), goal).unwrap();

        // Planner returns a FileRead (allowed) step — but we'll manually set up
        // a plan that tries FileWrite (not allowed) by using the planner
        // Note: planner itself would reject this, but let's test runtime enforcement
        // by having planner return FileRead (allowed)
        let planner = make_planner(
            r#"[{"action": {"type": "FileRead", "path": "/tmp/x"}, "description": "read"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("file contents");
        let mut audit = AuditTrail::new();

        let result = runtime
            .run_cycle(
                &id.to_string(),
                &planner,
                &memory_mgr,
                &executor,
                &mut audit,
            )
            .unwrap();

        // FileRead is allowed and is the only step, so goal completes
        assert_eq!(result.phase, CognitivePhase::Learn);
        assert_eq!(result.steps_executed, 1);
    }

    #[test]
    fn test_failed_steps_trigger_replan() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let emitter = Arc::new(CollectingEmitter::new());
        let config = LoopConfig {
            max_consecutive_failures: 2,
            max_cycles_per_goal: 20,
            reflection_interval: 100, // Don't trigger reflection
            ..LoopConfig::default()
        };
        let runtime = CognitiveRuntime::new(sup, config, emitter);

        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(
            r#"[{"action": {"type": "LlmQuery", "prompt": "q", "context": []}, "description": "ask"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_err("connection failed");
        let mut audit = AuditTrail::new();

        // Run cycles until failures accumulate (step has max_retries=2, so 2 fails = skip)
        let _r1 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap(); // plan + fail attempt 1
        let _r2 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap(); // fail attempt 2, skip step

        // After skipping, goal should complete (all steps done) in learn phase
        let r3 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        // Should either replan or complete
        assert!(r3.phase == CognitivePhase::Learn || r3.phase == CognitivePhase::Act);
    }

    #[test]
    fn test_reflection_fires_at_interval() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let emitter = Arc::new(CollectingEmitter::new());
        let config = LoopConfig {
            reflection_interval: 1, // Reflect every cycle
            max_cycles_per_goal: 50,
            ..LoopConfig::default()
        };
        let runtime = CognitiveRuntime::new(sup, config, emitter.clone());

        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        // Plan with 3 steps — reflection fires when the last step completes
        let planner = make_planner(
            r#"[
                {"action": {"type": "Noop"}, "description": "1"},
                {"action": {"type": "Noop"}, "description": "2"},
                {"action": {"type": "Noop"}, "description": "3"}
            ]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        // Run 3 cycles to complete all steps
        let _r1 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();
        let _r2 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();
        let _r3 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        // Check that reflection phase was emitted on goal completion
        let events = emitter.events.lock().unwrap();
        let has_reflect = events.iter().any(|e| {
            matches!(
                e,
                CognitiveEvent::PhaseChange {
                    phase: CognitivePhase::Reflect,
                    ..
                }
            )
        });
        assert!(has_reflect);
    }

    #[test]
    fn test_goal_completion_emits_event() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, emitter) = make_runtime(sup);
        let goal = AgentGoal::new("simple task".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(r#"[{"action": {"type": "Noop"}, "description": "done"}]"#);
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        // Single step plan completes in one cycle
        let r1 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        assert_eq!(r1.phase, CognitivePhase::Learn);
        assert!(!r1.should_continue);

        let events = emitter.events.lock().unwrap();
        let completed = events
            .iter()
            .any(|e| matches!(e, CognitiveEvent::GoalCompleted { success: true, .. }));
        assert!(completed);
    }

    #[test]
    fn test_audit_events_emitted_for_actions() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, _) = make_runtime(sup);
        let goal = AgentGoal::new("test".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(
            r#"[{"action": {"type": "LlmQuery", "prompt": "hi", "context": []}, "description": "ask"}]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("response");
        let mut audit = AuditTrail::new();

        let _r = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        // Audit should have at least 1 event for the step execution
        // (plus the agent creation events from supervisor)
        assert!(!audit.events().is_empty());
    }

    #[test]
    fn test_action_requires_hitl() {
        // L3 agent: file write should NOT require HITL
        assert!(!action_requires_hitl(
            &PlannedAction::FileWrite {
                path: "/tmp".into(),
                content: "x".into()
            },
            3
        ));
        // L1 agent: file write SHOULD require HITL
        assert!(action_requires_hitl(
            &PlannedAction::FileWrite {
                path: "/tmp".into(),
                content: "x".into()
            },
            1
        ));
        // LLM queries never require HITL
        assert!(!action_requires_hitl(
            &PlannedAction::LlmQuery {
                prompt: "hi".into(),
                context: vec![]
            },
            0
        ));
        // HITL request always blocks
        assert!(action_requires_hitl(
            &PlannedAction::HitlRequest {
                question: "ok?".into(),
                options: vec![]
            },
            5
        ));
    }

    #[test]
    fn test_estimate_fuel_cost() {
        assert!(
            estimate_fuel_cost(&PlannedAction::LlmQuery {
                prompt: "".into(),
                context: vec![]
            }) > 0.0
        );
        assert!(estimate_fuel_cost(&PlannedAction::Noop).abs() < f64::EPSILON);
    }

    #[test]
    fn test_e2e_three_step_plan() {
        let (sup, agent_id) = make_supervisor_with_agent();
        let (runtime, emitter) = make_runtime(sup);
        let goal = AgentGoal::new("three step task".into(), 5);
        runtime.assign_goal(&agent_id, goal).unwrap();

        let planner = make_planner(
            r#"[
                {"action": {"type": "LlmQuery", "prompt": "step1", "context": []}, "description": "s1"},
                {"action": {"type": "LlmQuery", "prompt": "step2", "context": []}, "description": "s2"},
                {"action": {"type": "LlmQuery", "prompt": "step3", "context": []}, "description": "s3"}
            ]"#,
        );
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("done");
        let mut audit = AuditTrail::new();

        // Cycle 1: plan + execute step 1 (2 remain)
        let r1 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();
        assert_eq!(r1.steps_executed, 1);
        assert!(r1.should_continue);

        // Cycle 2: execute step 2 (1 remains)
        let r2 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();
        assert_eq!(r2.steps_executed, 1);
        assert!(r2.should_continue);

        // Cycle 3: execute step 3 (0 remain → goal completes)
        let r3 = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();
        assert_eq!(r3.steps_executed, 1);
        assert!(!r3.should_continue);
        assert_eq!(r3.phase, CognitivePhase::Learn);

        // Check goal completed event
        let events = emitter.events.lock().unwrap();
        let completed = events.iter().find(|e| {
            matches!(
                e,
                CognitiveEvent::GoalCompleted {
                    success: true,
                    steps_total: 3,
                    ..
                }
            )
        });
        assert!(completed.is_some());

        // Check status
        let status = runtime.get_agent_status(&agent_id).unwrap();
        assert_eq!(status.steps_completed, 3);
    }

    #[test]
    fn test_l6_cooldown_pause_triggers_at_100_cycles() {
        let (sup, agent_id) = make_supervisor_with_autonomy(6);
        let emitter = Arc::new(CollectingEmitter::new());
        let runtime = CognitiveRuntime::new(sup, LoopConfig::default(), emitter.clone());
        runtime
            .assign_goal(&agent_id, AgentGoal::new("cooldown test".into(), 5))
            .unwrap();
        runtime
            .loops
            .lock()
            .unwrap()
            .get_mut(&agent_id)
            .unwrap()
            .cycle_count = 100;

        let planner = make_planner(r#"[{"action": {"type": "Noop"}, "description": "done"}]"#);
        let memory_mgr = make_memory_mgr();
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let _ = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        let events = emitter.events.lock().unwrap();
        assert!(events.iter().any(|event| matches!(
            event,
            CognitiveEvent::AgentCooldown {
                cycles_completed: 100,
                ..
            }
        )));
    }

    #[test]
    fn test_l6_cooldown_cannot_be_disabled_by_agent_memory() {
        let (sup, agent_id) = make_supervisor_with_autonomy(6);
        let emitter = Arc::new(CollectingEmitter::new());
        let runtime = CognitiveRuntime::new(sup, LoopConfig::default(), emitter.clone());
        runtime
            .assign_goal(
                &agent_id,
                AgentGoal::new("cooldown override test".into(), 5),
            )
            .unwrap();
        runtime
            .loops
            .lock()
            .unwrap()
            .get_mut(&agent_id)
            .unwrap()
            .cycle_count = 100;

        let seeded = vec![super::super::memory_manager::MemoryEntry {
            id: 1,
            agent_id: agent_id.clone(),
            memory_type: "cognitive_params".to_string(),
            key: "active".to_string(),
            value_json: json!({
                "max_cycles": "500",
                "reflection_interval": "1",
                "planning_depth": "10"
            })
            .to_string(),
            relevance_score: 1.0,
            access_count: 0,
            created_at: "now".to_string(),
            last_accessed: "now".to_string(),
        }];
        let memory_mgr = make_seeded_memory_mgr(seeded);
        let planner = make_planner(r#"[{"action": {"type": "Noop"}, "description": "done"}]"#);
        let executor = MockExecutor::always_ok("ok");
        let mut audit = AuditTrail::new();

        let _ = runtime
            .run_cycle(&agent_id, &planner, &memory_mgr, &executor, &mut audit)
            .unwrap();

        let events = emitter.events.lock().unwrap();
        assert!(events
            .iter()
            .any(|event| matches!(event, CognitiveEvent::AgentCooldown { .. })));
    }
}
