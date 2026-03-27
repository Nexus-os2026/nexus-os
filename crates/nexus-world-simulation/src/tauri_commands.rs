use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::engine::{SimulationConfig, SimulationEngine};
use crate::governance::SimulationPolicy;
use crate::outcome::SimulationResult;
use crate::rollback::RollbackManager;
use crate::scenario::{Scenario, SimulatedAction};

pub struct SimulationState {
    pub engine: RwLock<SimulationEngine>,
    pub rollback: RwLock<RollbackManager>,
    pub policy: SimulationPolicy,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            engine: RwLock::new(SimulationEngine::new(SimulationConfig::default())),
            rollback: RwLock::new(RollbackManager::default()),
            policy: SimulationPolicy::default(),
        }
    }
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSummary {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub step_count: usize,
    pub created_at: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySummary {
    pub min_autonomy_level: u8,
    pub max_steps: u32,
    pub max_concurrent_per_agent: usize,
    pub allow_branching: bool,
    pub cost_per_step: f64,
    pub base_cost: f64,
}

pub fn submit_scenario(
    state: &SimulationState,
    agent_id: &str,
    description: &str,
    actions: Vec<SimulatedAction>,
) -> Result<String, String> {
    state
        .policy
        .check_authorization(3)
        .map_err(|e| e.to_string())?;

    let scenario = Scenario::new(agent_id.into(), description.into(), actions);
    let mut engine = state.engine.write().map_err(|e| e.to_string())?;
    engine.submit(scenario).map_err(|e| e.to_string())
}

pub fn run_scenario(
    state: &SimulationState,
    scenario_id: &str,
) -> Result<SimulationResult, String> {
    let mut engine = state.engine.write().map_err(|e| e.to_string())?;
    engine
        .run_simulation(scenario_id)
        .map_err(|e| e.to_string())
}

pub fn get_result(state: &SimulationState, scenario_id: &str) -> Result<SimulationResult, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    engine
        .get_result(scenario_id)
        .cloned()
        .ok_or_else(|| format!("No result for scenario {scenario_id}"))
}

pub fn get_history(
    state: &SimulationState,
    agent_id: &str,
) -> Result<Vec<ScenarioSummary>, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    let scenarios = engine.agent_history(agent_id);
    Ok(scenarios
        .into_iter()
        .map(|s| ScenarioSummary {
            id: s.id.clone(),
            agent_id: s.agent_id.clone(),
            description: s.description.clone(),
            step_count: s.actions.len(),
            created_at: s.created_at,
            status: match &s.status {
                crate::scenario::ScenarioStatus::Pending => "Pending".into(),
                crate::scenario::ScenarioStatus::Running => "Running".into(),
                crate::scenario::ScenarioStatus::Completed { .. } => "Completed".into(),
                crate::scenario::ScenarioStatus::Failed { reason } => {
                    format!("Failed: {}", &reason[..reason.len().min(50)])
                }
            },
        })
        .collect())
}

pub fn get_policy(state: &SimulationState) -> PolicySummary {
    PolicySummary {
        min_autonomy_level: state.policy.min_autonomy_level,
        max_steps: state.policy.max_steps,
        max_concurrent_per_agent: state.policy.max_concurrent_per_agent,
        allow_branching: state.policy.allow_branching,
        cost_per_step: state.policy.cost_per_step as f64 / 1_000_000.0,
        base_cost: state.policy.base_cost as f64 / 1_000_000.0,
    }
}

pub fn create_branch(
    state: &SimulationState,
    parent_id: &str,
    diverge_at_step: u32,
    alternative: SimulatedAction,
    remaining: Vec<SimulatedAction>,
) -> Result<String, String> {
    let mut engine = state.engine.write().map_err(|e| e.to_string())?;
    engine
        .create_branch(parent_id, diverge_at_step, alternative, remaining)
        .map_err(|e| e.to_string())
}

pub fn get_risk(
    state: &SimulationState,
    scenario_id: &str,
) -> Result<crate::outcome::RiskAssessment, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    engine
        .get_result(scenario_id)
        .map(|r| r.risk_assessment.clone())
        .ok_or_else(|| format!("No result for scenario {scenario_id}"))
}
