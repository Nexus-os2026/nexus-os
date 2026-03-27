//! Nexus OS Capability Measurement Framework
//!
//! A formal evaluation system that measures agent capabilities across four vectors
//! using structured test batteries with asymmetric scoring, gaming detection, and
//! repeatability guarantees.
//!
//! This is NOT a benchmark — it's a measurement instrument that distinguishes
//! genuine capability from pattern matching.

pub mod battery;
pub mod darwin_bridge;
pub mod evaluation;
pub mod framework;
pub mod reporting;
pub mod scoring;
pub mod tauri_commands;
pub mod vectors;

pub use darwin_bridge::{run_measurement_feedback, EvolutionFitnessProvider, FeedbackResult};
pub use evaluation::ab_validation::{ABComparisonResult, AgentABComparison};
pub use evaluation::agent_adapter::AgentAdapter;
pub use evaluation::batch::{BatchEvaluator, BatchResult, DarwinUploadSummary};
pub use evaluation::runner::EvaluationRunner;
pub use evaluation::validation_run::{
    ValidationRunConfig, ValidationRunOutput, ValidationRunSummary,
};
pub use framework::*;
pub use reporting::scorecard::AgentScorecard;
