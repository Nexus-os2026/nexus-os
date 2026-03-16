pub mod persona;
pub mod report;
pub mod runtime;
pub mod seed;
pub mod timeline;
pub mod world;

pub use persona::{
    generate_personas, persona_decide, Persona, PersonaAction, PersonaActionEnvelope,
    PersonaMemory, PersonalityProfile,
};
pub use report::{
    generate_prediction_report, Coalition, Finding, OpinionShift, PredictionReport, TurningPoint,
};
pub use runtime::{
    compare_reports, estimate_simulation_fuel, evolve_simulation, run_parallel_simulations,
    MetaSimulationAnalysis, OptimalSimConfig, PersistedSimulationState, SimulationControl,
    SimulationLiveEvent, SimulationObserver, SimulationProgress, SimulationRuntime,
};
pub use seed::{parse_seed, SeedEntity, SeedRelationship, SeedVariable, WorldSeed};
pub use timeline::{Timeline, WorldEvent, WorldTick};
pub use world::{
    SimulatedWorld, SimulationStatus, SimulationSummary, WorldEnvironment, WorldStatus,
};

use crate::errors::AgentError;
use serde_json::Value;

pub fn extract_json_value(text: &str) -> Result<Value, AgentError> {
    serde_json::from_str::<Value>(text)
        .or_else(|_| extract_delimited_json(text, '[', ']'))
        .or_else(|_| extract_delimited_json(text, '{', '}'))
        .map_err(|error| {
            AgentError::SupervisorError(format!("unable to parse json response: {error}"))
        })
}

fn extract_delimited_json(text: &str, open: char, close: char) -> Result<Value, serde_json::Error> {
    let start = text.find(open).unwrap_or(0);
    let end = text
        .rfind(close)
        .map(|index| index + 1)
        .unwrap_or(text.len());
    serde_json::from_str::<Value>(&text[start..end])
}
