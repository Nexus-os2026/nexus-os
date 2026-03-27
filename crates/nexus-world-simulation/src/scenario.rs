use serde::{Deserialize, Serialize};

use crate::outcome::SimulationResult;

/// A simulation scenario — a hypothetical sequence of actions
/// with predicted outcomes and risk assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub actions: Vec<SimulatedAction>,
    pub preconditions: Vec<Condition>,
    pub expected_outcome: Option<String>,
    pub created_at: u64,
    pub status: ScenarioStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScenarioStatus {
    Pending,
    Running,
    Completed { result: SimulationResult },
    Failed { reason: String },
}

/// An action within a simulation — mirrors real actions but executes in sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedAction {
    pub step: u32,
    pub action_type: SimActionType,
    pub description: String,
    pub depends_on: Vec<u32>,
    pub predicted_outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SimActionType {
    TerminalCommand {
        command: String,
        working_dir: Option<String>,
    },
    FileWrite {
        path: String,
        content: String,
    },
    FileDelete {
        path: String,
    },
    HttpRequest {
        method: String,
        url: String,
        body: Option<String>,
    },
    Deploy {
        target: String,
        artifact: String,
    },
    AgentMessage {
        target_agent: String,
        message: String,
    },
    LlmCall {
        model: String,
        prompt: String,
    },
    Custom {
        action_name: String,
        parameters: serde_json::Value,
    },
}

/// A condition that must be true for the scenario to be valid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub description: String,
    pub check_type: ConditionCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionCheck {
    FileExists(String),
    FileNotExists(String),
    EnvVarSet(String),
    ServiceReachable { host: String, port: u16 },
    SufficientBudget { minimum: u64 },
    HasCapability(String),
    Custom(String),
}

/// Branching — what-if with alternatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioBranch {
    pub branch_id: String,
    pub parent_scenario: String,
    pub diverge_at_step: u32,
    pub alternative_action: SimulatedAction,
    pub remaining_actions: Vec<SimulatedAction>,
    pub outcome: Option<SimulationResult>,
}

impl Scenario {
    pub fn new(agent_id: String, description: String, actions: Vec<SimulatedAction>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id,
            description,
            actions,
            preconditions: Vec::new(),
            expected_outcome: None,
            created_at: epoch_secs(),
            status: ScenarioStatus::Pending,
        }
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
