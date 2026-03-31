//! Outcome evaluation primitives for governed AI agents.
//!
//! Three components:
//! 1. **Outcome Specification** — define success criteria before execution
//! 2. **Outcome Assessment** — evaluate results against specifications
//! 3. **Outcome Artifact** — compliance-ready proof of goal → actions → verdict
//!
//! # Example
//!
//! ```
//! use nexus_outcome_eval::builder::OutcomeSpecBuilder;
//! use nexus_outcome_eval::evaluator::OutcomeEvaluator;
//! use nexus_outcome_eval::artifact::OutcomeArtifactGenerator;
//! use nexus_outcome_eval::types::MatchMode;
//!
//! let spec = OutcomeSpecBuilder::new("task-1", "agent-1", "Summarize the document")
//!     .must_contain("Output mentions key topics", vec!["AI".into(), "agents".into()], MatchMode::Any)
//!     .must_complete_within(300)
//!     .build();
//!
//! let evaluator = OutcomeEvaluator::new();
//! let assessment = evaluator.evaluate(&spec, "This report covers AI agent frameworks", &serde_json::json!({"duration_seconds": 45}));
//!
//! assert_eq!(assessment.verdict, nexus_outcome_eval::types::OutcomeVerdict::Success);
//! ```

pub mod artifact;
pub mod builder;
pub mod evaluator;
#[cfg(test)]
mod tests;
pub mod types;
