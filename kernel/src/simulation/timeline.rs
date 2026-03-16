use super::persona::PersonaAction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Timeline {
    #[serde(default)]
    pub ticks: Vec<WorldTick>,
    pub current_tick: u64,
    #[serde(default)]
    pub events_per_tick: Vec<Vec<WorldEvent>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorldTick {
    pub tick_number: u64,
    #[serde(default)]
    pub events: Vec<WorldEvent>,
    #[serde(default)]
    pub variable_injections: Vec<(String, String)>,
    #[serde(default)]
    pub belief_shifts: Vec<(String, String, f64)>,
    #[serde(default)]
    pub emergent_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldEvent {
    pub tick: u64,
    pub actor_id: String,
    pub action: PersonaAction,
    #[serde(default)]
    pub observers: Vec<String>,
    pub impact: f64,
}
