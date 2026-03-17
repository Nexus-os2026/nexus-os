//! Dream Forge — overnight agent self-improvement system.
//!
//! When the system is idle, agents replay the day's work, find patterns,
//! run experiments in simulation, and produce finished work by morning.

pub mod auto_queue;
pub mod engine;
pub mod report;
pub mod scheduler;
pub mod types;

pub use auto_queue::queue_dreams_from_interaction;
pub use engine::DreamEngine;
pub use report::MorningBriefing;
pub use scheduler::DreamScheduler;
pub use types::{DreamOutcome, DreamResult, DreamTask, DreamType};
