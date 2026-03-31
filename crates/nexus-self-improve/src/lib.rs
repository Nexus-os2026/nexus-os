#![forbid(unsafe_code)]
//! # Nexus Self-Improvement Engine
//!
//! Governed self-improvement pipeline for Nexus OS. Allows the system to improve
//! itself (prompts, configs, policies, scheduling) while 10 hard invariants ensure
//! the governance kernel remains immutable.
//!
//! ## Architecture
//!
//! Five-stage pipeline: **Observer → Analyzer → Proposer → Validator → Applier**
//!
//! Every change must pass all 10 hard invariants and receive Tier3 HITL approval
//! before application. Every applied change gets a checkpoint and enters a canary
//! monitoring period with automatic rollback on anomaly.

pub mod analyzer;
pub mod applier;
pub mod config_optimizer;
pub mod envelope;
pub mod guardian;
pub mod invariants;
pub mod observer;
pub mod pipeline;
pub mod policy_optimizer;
pub mod prompt_optimizer;
pub mod proposer;
pub mod report;
pub mod scheduler;
pub mod trajectory;
pub mod types;
pub mod validator;

pub use pipeline::SelfImprovementPipeline;
pub use types::*;
