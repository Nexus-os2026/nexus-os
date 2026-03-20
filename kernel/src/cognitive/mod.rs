//! Cognitive Agent Runtime — the perceive→reason→plan→act→reflect→learn loop.
//!
//! This module provides the core cognitive loop that transforms agents from passive
//! state machines into active goal-pursuing entities. Each agent runs through a
//! structured cognitive cycle, planning steps via LLM, executing them with governance
//! checks, and learning from outcomes.

pub mod algorithms;
pub mod evolution;
pub mod hivemind;
pub mod loop_runtime;
pub mod memory_manager;
pub mod planner;
pub mod scheduler;
pub mod types;

pub use algorithms::{
    AdversarialArena, EvolutionEngine, PlanEvolutionEngine, SwarmCoordinator, WorldModel,
};
pub use evolution::{
    hash_strategy, EvolutionLlm, EvolutionMetrics, EvolutionTracker, StrategyInfo, StrategyScore,
    StrategyStore,
};
pub use hivemind::{
    AgentInfo, HivemindCoordinator, HivemindEvent, HivemindEventEmitter, HivemindLlm,
    HivemindSession, HivemindStatus, SubTask, SubTaskStatus,
};
pub use loop_runtime::{
    ActionExecutor, CognitiveRuntime, EventEmitter, LlmProvider, NoOpEmitter, RegistryExecutor,
};
pub use memory_manager::{AgentMemoryManager, MemoryEntry, MemoryStore};
pub use planner::{CognitivePlanner, PlannerLlm};
pub use scheduler::{AgentScheduler, ScheduledAgent, ScheduledGoalExecutor};
pub use types::{
    AgentGoal, AgentStep, CognitiveEvent, CognitivePhase, CognitiveStatusResponse, CycleResult,
    GoalStatus, LoopConfig, PlannedAction, PlanningContext, StepStatus,
};
