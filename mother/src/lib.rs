//! Mother Nexus M1: the mission contract and the coordinator core.
//!
//! Mother turns an Owner's goal into a [`contract::MissionContract`] (the
//! Owner's words verbatim, a restatement, the requested folder, a budget,
//! forbidden effects, answered questions and pass criteria, all hashed).
//! The Owner consents to that exact hash ([`consent`]); the
//! [`coordinator::Coordinator`] then walks the mission through the
//! [`state`] machine, calling [`lane`]s and sending the effects they ask
//! for through the [`effect::EffectPort`], whose only implementation is the
//! [`adapter`] over the governed-control broker. Every transition and lane
//! call is in the hash-chained [`journal`], from which a restarted
//! coordinator resumes. Stop works from every state.
//!
//! See `README.md` for the invariants and the non-claims.
//!
//! The EffectPort is sealed: outside this crate only the adapter is one.
//!
//! ```compile_fail
//! use nexus_mother::effect::{EffectOutcome, EffectPort, EffectRefusal, EffectRequest};
//! use nexus_mother::journal::MissionId;
//! struct Elsewhere;
//! impl EffectPort for Elsewhere { // the trait is sealed
//!     fn perform(&self, _: MissionId, _: &EffectRequest) -> Result<EffectOutcome, EffectRefusal> { todo!() }
//!     fn cancel(&self, _: MissionId) {}
//!     fn finish(&self, _: MissionId) {}
//!     fn emergency_stopped(&self) -> bool { false }
//! }
//! ```

#![deny(unsafe_code)]

pub mod adapter;
pub mod consent;
pub mod contract;
pub mod coordinator;
pub mod effect;
pub mod journal;
pub mod lane;
pub mod state;

#[cfg(test)]
mod oracles;
