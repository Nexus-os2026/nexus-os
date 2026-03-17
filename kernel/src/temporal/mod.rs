//! Temporal Engine — fork parallel timelines, simulate outcomes, commit the best future.
//!
//! ## Subsystems
//!
//! - **types** — core data structures (forks, decisions, sessions, checkpoints).
//! - **engine** — fork-and-evaluate logic, strategy selection.
//! - **dilation** — time-dilated iterative work sessions (create→critique loops).
//! - **checkpoints** — pre-fork snapshots for rollback integration.

pub mod checkpoints;
pub mod dilation;
pub mod engine;
pub mod types;

pub use checkpoints::TemporalCheckpointManager;
pub use dilation::TimeDilator;
pub use engine::TemporalEngine;
pub use types::{
    Artifact, EvalStrategy, ForkStatus, TemporalCheckpoint, TemporalConfig, TemporalDecision,
    TemporalError, TimeDilatedSession, TimelineFork, TimelineStep,
};
