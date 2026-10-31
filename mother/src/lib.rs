//! Mother Nexus M1: the mission contract and the coordinator core.
//!
//! Mother turns an Owner goal into a [`contract::MissionContract`]: the goal
//! and the exact steps she would take, each a broker intent (data, never
//! authority). The [`coordinator::Coordinator`] moves a mission forward one
//! step at a time, and only after the Owner consented to that exact
//! contract. It holds no authority: every effect goes through the existing
//! broker, `GovernedControl`, as an agent's action in an agent's run, under
//! the Owner's grants and native approvals. Stop is honoured from every
//! state. See `README.md` for the invariants and the non-claims.
//!
//! The broker trait is sealed: outside this crate only `GovernedControl`
//! can be Mother's broker.
//!
//! ```compile_fail,E0277
//! use nexus_mother::broker::{Acted, Broker, Refusal};
//! use nexus_governed_control::authority::commitment::CommitmentState;
//! use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
//! use nexus_governed_control::governed::Intent;
//! struct Elsewhere;
//! impl Broker for Elsewhere { // the trait is sealed
//!     fn open_run(&self, _: &AgentId) -> Result<RunId, Refusal> { unimplemented!() }
//!     fn act(&self, _: &AgentId, _: RunId, _: &Intent) -> Result<Acted, Refusal> { unimplemented!() }
//!     fn commitment_state(&self, _: CommitmentId) -> Option<CommitmentState> { None }
//!     fn cancel_run(&self, _: RunId) -> Result<(), Refusal> { Ok(()) }
//!     fn finish_when_settled(&self, _: RunId) {}
//!     fn emergency_stopped(&self) -> bool { true }
//! }
//! ```

#![deny(unsafe_code)]

pub mod broker;
pub mod contract;
pub mod coordinator;

#[cfg(test)]
mod oracles;
