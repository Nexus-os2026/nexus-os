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
    compare_reports, create_world_nonblocking, estimate_simulation_fuel, evolve_simulation,
    run_parallel_simulations, MetaSimulationAnalysis, OptimalSimConfig, PersistedSimulationState,
    SimulationControl, SimulationLiveEvent, SimulationObserver, SimulationProgress,
    SimulationRuntime,
};
pub use seed::{parse_seed, SeedEntity, SeedRelationship, SeedVariable, WorldSeed};
pub use timeline::{Timeline, WorldEvent, WorldTick};
pub use world::{
    SimulatedWorld, SimulationStatus, SimulationSummary, WorldEnvironment, WorldStatus,
};

use crate::cognitive::PlannerLlm;
use crate::errors::AgentError;
use crate::hm::{DispatchContext, Principal, PromptBuilder};
use serde_json::Value;
use std::sync::Arc;

/// Mints the explicit `System` context of one simulation model call.
pub type SimulationContexts = dyn Fn() -> Result<DispatchContext, AgentError> + Send + Sync;

/// The simulation's model (class XS, P3-HM-CONTRACT-02 §12.1, §12.2,
/// §12.8). Every call runs under its own explicit `System` context from
/// `contexts`, never a thread-local default, so it is admitted as a plain
/// request: no retained record is included, and a non-local route needs an
/// unrestricted context. A simulation prompt is backend text built from the
/// simulation's own world (an excluded store, §19), never a retained
/// record.
#[derive(Clone)]
pub struct SimulationLlm {
    planner: Arc<dyn PlannerLlm>,
    contexts: Arc<SimulationContexts>,
}

impl SimulationLlm {
    pub fn new(planner: Arc<dyn PlannerLlm>, contexts: Arc<SimulationContexts>) -> Self {
        Self { planner, contexts }
    }

    pub fn planner(&self) -> &dyn PlannerLlm {
        self.planner.as_ref()
    }

    /// A fresh context for one call; anything but a `System` context is
    /// refused.
    pub fn context(&self) -> Result<DispatchContext, AgentError> {
        let ctx = (self.contexts)()?;
        if !matches!(ctx.principal(), Principal::System(_)) {
            return Err(AgentError::SupervisorError(
                "simulation: refused: a simulation call runs only under a System context"
                    .to_string(),
            ));
        }
        Ok(ctx)
    }

    /// One model call with `prompt` as backend text.
    pub fn ask(&self, prompt: &str) -> Result<String, AgentError> {
        let mut ctx = self.context()?;
        self.planner
            .plan_query(&mut ctx, &mut |c: &mut DispatchContext| {
                let mut b = PromptBuilder::new(c);
                b.text(prompt);
                Some(b.build())
            })
            .map(|reply| reply.text().to_string())
    }
}

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
