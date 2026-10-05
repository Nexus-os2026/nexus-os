//! Nexus OS Phase Three: governed real-world control.
//!
//! One authority architecture for every real-world effect an agent or a
//! command may cause (processes, network, browser, perception, input,
//! connectors and credentials):
//!
//! 1. A request (model output, command text, a selector) is data. It is
//!    classified and *prepared* by a domain actuator into a canonical target
//!    identity and a parameter digest.
//! 2. The authority core records an [`authority::commitment`] for it, bound
//!    to the agent, the run, the effect class, the grants and credential
//!    leases it relies on and the policy generation.
//! 3. R0 and R1 commitments need a live owner grant that covers them; R2
//!    commitments also need the owner's native approval of exactly that
//!    commitment.
//! 4. The actuator revalidates the target and consumes the commitment once,
//!    performs the bounded effect while observing cancellation, and the
//!    commitment is finalized and evidenced whatever happens.
//!
//! The authority is used through the [`control::Control`] pipeline; every
//! path the negative examples below use is public:
//!
//! ```
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::clock::SystemClock;
//! use nexus_governed_control::authority::commitment::ExecutionGuard;
//! use nexus_governed_control::authority::evidence::MemoryEvidence;
//! use nexus_governed_control::authority::ids::{AgentId, CommitmentId, Digest, RunId};
//! use nexus_governed_control::authority::Authority;
//! use nexus_governed_control::control::Control;
//! use std::sync::Arc;
//!
//! let control = Control::new(Authority::new(
//!     Arc::new(MemoryEvidence::new(16)),
//!     Arc::new(SystemClock::default()),
//! ));
//! let unknown = CommitmentId::parse("cmt-00000000000000000000000000000000").unwrap();
//! assert!(control.authority().commitments().view(unknown).is_none());
//! fn names(approval: &R2Approval, guard: &ExecutionGuard) -> (CommitmentId, CommitmentId) {
//!     (approval.commitment(), guard.commitment())
//! }
//! let _ = (names, AgentId::new("agent"), Digest::of("d", &[]), None::<RunId>);
//! let _ = serde_json::to_string(&1);
//! ```
//!
//! The authority's types cannot be forged from outside the crate:
//!
//! ```compile_fail
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::ids::{CommitmentId, Digest};
//! fn forge(id: CommitmentId, binding: Digest) -> R2Approval {
//!     R2Approval { commitment: id, binding } // the fields are private
//! }
//! ```
//!
//! ```compile_fail
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::ids::{CommitmentId, Digest};
//! fn forge(id: CommitmentId, binding: Digest) -> R2Approval {
//!     R2Approval::confirmed(id, binding) // crate-private
//! }
//! ```
//!
//! ```compile_fail
//! use nexus_governed_control::authority::approval::R2Approval;
//! fn twice(approval: R2Approval) -> (R2Approval, R2Approval) {
//!     (approval.clone(), approval) // not Clone
//! }
//! ```
//!
//! ```compile_fail
//! let _: nexus_governed_control::authority::approval::R2Approval =
//!     serde_json::from_str("{}").unwrap(); // no deserializer
//! ```
//!
//! ```compile_fail
//! use nexus_governed_control::authority::ids::CommitmentId;
//! let _ = CommitmentId::fresh(); // only the registry creates identities
//! ```
//!
//! ```compile_fail
//! use nexus_governed_control::authority::commitment::ExecutionGuard;
//! fn twice(guard: ExecutionGuard) -> (ExecutionGuard, ExecutionGuard) {
//!     (guard.clone(), guard) // one-shot: not Clone
//! }
//! ```
//!
//! ```compile_fail
//! use nexus_governed_control::authority::Authority;
//! use nexus_governed_control::authority::ids::{AgentId, CommitmentId, Digest, RunId};
//! fn start(a: &Authority, id: CommitmentId, agent: &AgentId, run: RunId, d: &Digest) {
//!     // Only the pipeline starts an effect: the lifecycle is crate-private.
//!     let _ = a.commitments().begin(id, agent, run, d, d);
//! }
//! ```

pub mod authority;
pub mod broker;
pub mod connector;
pub mod control;
pub mod egress;

#[cfg(test)]
mod harness_tests;
