use super::persona::Persona;
use super::seed::WorldSeed;
use super::timeline::Timeline;
use crate::cognitive::algorithms::WorldModel;
use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type KnowledgeGraph = WorldModel;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldStatus {
    Building,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SimulationStatus {
    Draft,
    AwaitingApproval,
    Ready,
    Running,
    Paused,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorldEnvironment {
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub global_state: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulatedWorld {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub personas: Vec<Persona>,
    pub environment: WorldEnvironment,
    pub timeline: Timeline,
    pub knowledge_graph: KnowledgeGraph,
    pub tick_count: u64,
    pub status: WorldStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationSummary {
    pub id: String,
    pub name: String,
    pub status: SimulationStatus,
    pub tick_count: u64,
    pub persona_count: usize,
    pub created_at: DateTime<Utc>,
    pub prediction_summary: Option<String>,
}

impl SimulatedWorld {
    pub fn from_seed(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        seed: &WorldSeed,
        personas: Vec<Persona>,
        llm: &dyn PlannerLlm,
    ) -> Result<Self, AgentError> {
        let knowledge_graph = WorldModel::build_from_seed(&seed.scenario, llm)?;
        let mut global_state = HashMap::new();
        global_state.insert("scenario".to_string(), seed.scenario.clone());
        for variable in &seed.variables {
            global_state.insert(variable.key.clone(), "unknown".to_string());
        }
        Ok(Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            personas,
            environment: WorldEnvironment {
                rules: vec![
                    "personas form opinions based on information they receive".to_string(),
                    "relationships influence persuasion".to_string(),
                    "memories bias future decisions".to_string(),
                ],
                variables: HashMap::new(),
                global_state,
            },
            timeline: Timeline::default(),
            knowledge_graph,
            tick_count: 0,
            status: WorldStatus::Building,
            created_at: Utc::now(),
        })
    }

    pub fn inject_variable(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        self.environment
            .variables
            .insert(key.clone(), value.clone());
        self.environment.global_state.insert(key, value);
    }

    pub fn summary(&self) -> SimulationSummary {
        let status = match self.status {
            WorldStatus::Building => SimulationStatus::Draft,
            WorldStatus::Running => SimulationStatus::Running,
            WorldStatus::Paused => SimulationStatus::Paused,
            WorldStatus::Completed => SimulationStatus::Completed,
        };
        SimulationSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            status,
            tick_count: self.tick_count,
            persona_count: self.personas.len(),
            created_at: self.created_at,
            prediction_summary: None,
        }
    }
}
