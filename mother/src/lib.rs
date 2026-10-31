//! Mother Nexus M1 and M2a: the mission contract and the coordinator core.
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
//!     fn broker(&self) -> &nexus_governed_control::authority::Authority { todo!() }
//!     fn approval(&self, _: MissionId) -> nexus_mother::effect::ApprovalStatus { todo!() }
//! }
//! ```
//!
//! M2a: a consent is made only from the broker's `MissionApproval`, which
//! cannot be built or copied outside the broker; only this crate turns one
//! into a consent.
//!
//! ```compile_fail
//! use nexus_governed_control::authority::approval::MissionApproval;
//! fn twice(a: MissionApproval) -> (MissionApproval, MissionApproval) { (a.clone(), a) }
//! ```
//!
//! ```compile_fail
//! let _ = nexus_governed_control::authority::approval::MissionApproval::confirmed;
//! ```
//!
//! ```compile_fail
//! let _ = nexus_mother::consent::Consent::from_broker;
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
