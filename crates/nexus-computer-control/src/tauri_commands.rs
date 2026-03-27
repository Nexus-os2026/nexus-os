use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::actions::ComputerAction;
use crate::engine::{ActionResult, GovernedControlEngine, ScreenContext, VerificationResult};

/// Shared state for Tauri integration.
pub struct ControlState {
    pub engine: RwLock<GovernedControlEngine>,
}

impl ControlState {
    pub fn new(workspace_root: String) -> Self {
        Self {
            engine: RwLock::new(GovernedControlEngine::new(workspace_root)),
        }
    }
}

impl Default for ControlState {
    fn default() -> Self {
        Self::new("/home/nexus".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionHistoryEntry {
    pub entry_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub action_label: String,
    pub success: bool,
    pub error: Option<String>,
    pub token_cost: f64,
    pub balance_after: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub agent_id: String,
    pub balance: f64,
    pub total_spent: f64,
    pub actions_executed: usize,
    pub actions_denied: usize,
}

pub fn execute_action(
    state: &ControlState,
    agent_id: &str,
    autonomy_level: u8,
    capabilities: &[String],
    action: &ComputerAction,
) -> Result<ActionResult, String> {
    let mut engine = state.engine.write().map_err(|e| e.to_string())?;
    engine
        .execute_action(agent_id, autonomy_level, capabilities, action)
        .map_err(|e| e.to_string())
}

pub fn get_action_history(
    state: &ControlState,
    agent_id: &str,
) -> Result<Vec<ActionHistoryEntry>, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    let entries = engine.get_action_history(agent_id);
    Ok(entries
        .into_iter()
        .map(|e| ActionHistoryEntry {
            entry_id: e.entry_id.clone(),
            timestamp: e.timestamp,
            agent_id: e.agent_id.clone(),
            action_label: e.action_label.clone(),
            success: e.success,
            error: e.error.clone(),
            token_cost: e.token_cost as f64 / 1_000_000.0,
            balance_after: e.balance_after as f64 / 1_000_000.0,
        })
        .collect())
}

pub fn get_budget(state: &ControlState, agent_id: &str) -> Result<BudgetSummary, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    let budget = engine.get_budget(agent_id);
    Ok(BudgetSummary {
        agent_id: budget.agent_id,
        balance: budget.balance_micro as f64 / 1_000_000.0,
        total_spent: budget.total_spent_micro as f64 / 1_000_000.0,
        actions_executed: budget.actions_executed,
        actions_denied: budget.actions_denied,
    })
}

pub fn verify_sequence(state: &ControlState, agent_id: &str) -> Result<VerificationResult, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    Ok(engine.verify_action_sequence(agent_id))
}

pub fn get_screen_context(state: &ControlState, agent_id: &str) -> Result<ScreenContext, String> {
    let engine = state.engine.read().map_err(|e| e.to_string())?;
    Ok(engine.get_screen_context(agent_id))
}
