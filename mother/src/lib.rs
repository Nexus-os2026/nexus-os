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
//! use nexus_governed_control::authority::approval::{
//!     MissionApproval, MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
//! };
//! use nexus_governed_control::authority::AuthorityError;
//! use nexus_mother::effect::{EffectOutcome, EffectPort, EffectRefusal, EffectRequest};
//! use nexus_mother::journal::MissionId;
//! struct Elsewhere;
//! impl EffectPort for Elsewhere { // the trait is sealed
//!     fn perform(&self, _: MissionId, _: &EffectRequest) -> Result<EffectOutcome, EffectRefusal> { todo!() }
//!     fn cancel(&self, _: MissionId) {}
//!     fn finish(&self, _: MissionId) {}
//!     fn emergency_stopped(&self) -> bool { false }
//!     fn confirm_mission(&self, _: &MissionConfirmation) -> Result<MissionApproval, AuthorityError> { todo!() }
//!     fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> Result<MissionResumeApproval, AuthorityError> { todo!() }
//!     fn approval(&self, _: MissionId) -> nexus_mother::effect::ApprovalStatus { todo!() }
//!     fn pending(&self, _: MissionId) -> Option<nexus_mother::effect::PendingAction> { None }
//!     fn adopt(&self, _: MissionId, _: &nexus_mother::effect::PendingAction) {}
//!     fn settled(&self, _: &nexus_mother::effect::PendingAction) -> bool { false }
//!     fn withdraw(&self, _: MissionId) -> Result<(), EffectRefusal> { Ok(()) }
//!     fn fence(&self) {}
//!     fn hand(&self, _: MissionId, _: &nexus_mother::effect::PendingAction) {}
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
//!
//! M2c-A: the port goes over a broker-issued agent door, never over the
//! front door (A1-c: each block fails with the error its tag names, and
//! only for the property it names: an absent method is named as a path,
//! and the constructor's type is checked as a function pointer). The same
//! forms compile for what the port does have:
//!
//! ```
//! use nexus_governed_control::governed::AgentDoor;
//! use nexus_mother::adapter::GovernedPort;
//! use nexus_mother::effect::EffectPort;
//! let _: fn(AgentDoor) -> GovernedPort = GovernedPort::new;
//! let _ = GovernedPort::run_of;
//! fn reach<P: EffectPort>() {
//!     let _ = P::confirm_mission;
//! }
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_mother::adapter::GovernedPort::control; // no front door
//! ```
//!
//! ```compile_fail,E0599
//! use nexus_mother::effect::EffectPort;
//! fn reach<P: EffectPort>() {
//!     let _ = P::broker; // no authority
//! }
//! ```
//!
//! ```compile_fail,E0308
//! use nexus_governed_control::governed::GovernedControl;
//! use nexus_mother::adapter::GovernedPort;
//! use std::sync::Arc;
//! let _: fn(Arc<GovernedControl>) -> GovernedPort = GovernedPort::new; // a door, not the front door
//! ```
//!
//! Mother builds no broker: the host does (A1-d).
//!
//! ```
//! use nexus_mother::adapter::{FileJournal, GovernedPort, MOTHER_AGENT};
//! ```
//!
//! ```compile_fail,E0432
//! use nexus_mother::adapter::Home;
//! ```
//!
//! Consent asks through the port's door, whose confirmer the host bound;
//! the coordinator takes no confirmer (A1-e).
//!
//! ```
//! use nexus_mother::adapter::GovernedPort;
//! use nexus_mother::coordinator::Coordinator;
//! use nexus_mother::journal::MissionId;
//! fn ask(mother: &Coordinator<GovernedPort>, id: MissionId) {
//!     let _ = mother.request_consent(id);
//! }
//! ```
//!
//! ```compile_fail,E0061
//! use nexus_governed_control::authority::approval::ControlConfirmer;
//! use nexus_mother::adapter::GovernedPort;
//! use nexus_mother::coordinator::Coordinator;
//! use nexus_mother::journal::MissionId;
//! fn ask(mother: &Coordinator<GovernedPort>, id: MissionId, owner: &dyn ControlConfirmer) {
//!     let _ = mother.request_consent(id, owner);
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
#[cfg(test)]
mod test_home;
