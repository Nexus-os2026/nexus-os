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
//! Outside the crate the authority is reached only through
//! [`governed::GovernedControl`]: the pipeline, the preparations and the
//! commitment lifecycle are crate-private. Every path the negative examples
//! below use is public:
//!
//! ```
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::clock::SystemClock;
//! use nexus_governed_control::authority::commitment::ExecutionGuard;
//! use nexus_governed_control::authority::evidence::MemoryEvidence;
//! use nexus_governed_control::authority::ids::{AgentId, CommitmentId, Digest, RunId};
//! use nexus_governed_control::authority::Authority;
//! use std::sync::Arc;
//!
//! let authority = Authority::new(
//!     Arc::new(MemoryEvidence::new(16)),
//!     Arc::new(SystemClock::default()),
//! );
//! let unknown = CommitmentId::parse("cmt-00000000000000000000000000000000").unwrap();
//! assert!(authority.commitments().view(unknown).is_none());
//! fn names(approval: &R2Approval, guard: &ExecutionGuard) -> (CommitmentId, CommitmentId) {
//!     (approval.commitment(), guard.commitment())
//! }
//! let _ = (names, AgentId::new("agent"), Digest::of("d", &[]), None::<RunId>);
//! let _ = serde_json::to_string(&1);
//! ```
//!
//! The authority's types cannot be forged from outside the crate (each
//! block fails with the error its tag names; `p3_g6_15` pins them):
//!
//! ```compile_fail,E0451
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::ids::{CommitmentId, Digest};
//! fn forge(id: CommitmentId, binding: Digest) -> R2Approval {
//!     R2Approval { commitment: id, binding } // the fields are private
//! }
//! ```
//!
//! ```compile_fail,E0624
//! use nexus_governed_control::authority::approval::R2Approval;
//! use nexus_governed_control::authority::ids::{CommitmentId, Digest};
//! fn forge(id: CommitmentId, binding: Digest) -> R2Approval {
//!     R2Approval::confirmed(id, binding) // crate-private
//! }
//! ```
//!
//! ```compile_fail,E0599
//! use nexus_governed_control::authority::approval::R2Approval;
//! fn twice(approval: R2Approval) -> (R2Approval, R2Approval) {
//!     (approval.clone(), approval) // not Clone
//! }
//! ```
//!
//! ```compile_fail,E0277
//! let _: nexus_governed_control::authority::approval::R2Approval =
//!     serde_json::from_str("{}").unwrap(); // no deserializer
//! ```
//!
//! ```compile_fail,E0624
//! use nexus_governed_control::authority::ids::CommitmentId;
//! let _ = CommitmentId::fresh(); // only the registry creates identities
//! ```
//!
//! ```compile_fail,E0599
//! use nexus_governed_control::authority::commitment::ExecutionGuard;
//! fn twice(guard: ExecutionGuard) -> (ExecutionGuard, ExecutionGuard) {
//!     (guard.clone(), guard) // one-shot: not Clone
//! }
//! ```
//!
//! ```compile_fail,E0624
//! use nexus_governed_control::authority::Authority;
//! use nexus_governed_control::authority::ids::{AgentId, CommitmentId, Digest, RunId};
//! fn start(a: &Authority, id: CommitmentId, agent: &AgentId, run: RunId, d: &Digest) {
//!     // Only the pipeline starts an effect: the lifecycle is crate-private.
//!     let _ = a.commitments().begin(id, agent, run, d, d);
//! }
//! ```
//!
//! M2c-A: an agent acts only through an `AgentDoor`, which the front door
//! issues after the owner's native yes. A door is bound to its agent and to
//! the runs it opened or adopted; nothing reachable from it leads to the
//! front door, the authority, the issuer or an owner API (A1-a: each block
//! fails with the error its tag names). Its compiling twin uses what a door
//! has:
//!
//! ```
//! use nexus_governed_control::authority::approval::{
//!     MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
//! };
//! use nexus_governed_control::authority::ids::{CommitmentId, RunId};
//! use nexus_governed_control::governed::{AgentDoor, Intent, Lookup};
//! fn uses(
//!     door: &AgentDoor,
//!     run: RunId,
//!     id: CommitmentId,
//!     intent: &Intent,
//!     mission: &MissionConfirmation,
//!     held: &MissionResumeConfirmation,
//! ) {
//!     let _ = (door.agent(), door.open_run(), door.agent_action(run, intent));
//!     let _ = (door.cancel_run(run), door.finish_when_settled(run));
//!     let _ = door.adopt(run, id);
//!     let _: Lookup<_> = door.commitment(id);
//!     let _ = (door.emergency_stopped(), door.clock().wall_ms());
//!     let _ = door.confirm_mission(mission);
//!     let approval: Result<MissionResumeApproval, _> = door.confirm_held_mission(held);
//!     let _ = approval.map(|a| (a.contract_hash().len(), a.request().len(), a.evidence_seq()));
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.authority(); // no authority
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.control(); // no front door
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.open_owner_run(); // no owner run
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.authorize(); // no authorization
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.execute(); // no execution
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.request_grant(); // no grant
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.resume(); // no lift of the emergency stop
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.emergency_stop(); // not the owner's stop
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.issue_agent_door(); // no issuer
//! }
//! ```
//!
//! ```compile_fail,E0599
//! fn reach(door: &nexus_governed_control::governed::AgentDoor) {
//!     let _ = door.close(); // no close
//! }
//! ```
//!
//! A door and the owner's mission and resume approvals cannot be built
//! outside the crate, by a literal or a crate-private constructor, and a
//! door cannot be copied (A1-b):
//!
//! ```compile_fail,E0451
//! use nexus_governed_control::governed::AgentDoor;
//! fn forge(door: AgentDoor) -> AgentDoor {
//!     AgentDoor { ..door } // the fields are private
//! }
//! ```
//!
//! ```compile_fail,E0624
//! let _ = nexus_governed_control::governed::AgentDoor::new; // crate-private
//! ```
//!
//! ```compile_fail,E0599
//! use nexus_governed_control::governed::AgentDoor;
//! fn twice(door: AgentDoor) -> (AgentDoor, AgentDoor) {
//!     (door.clone(), door) // not Clone
//! }
//! ```
//!
//! ```compile_fail,E0451
//! use nexus_governed_control::authority::approval::MissionResumeApproval;
//! fn forge() -> MissionResumeApproval {
//!     MissionResumeApproval { contract_hash: String::new(), request: String::new(), evidence_seq: 1 }
//! }
//! ```
//!
//! ```compile_fail,E0624
//! let _ = nexus_governed_control::authority::approval::MissionResumeApproval::confirmed;
//! ```
//!
//! ```compile_fail,E0599
//! use nexus_governed_control::authority::approval::MissionResumeApproval;
//! fn twice(a: MissionResumeApproval) -> (MissionResumeApproval, MissionResumeApproval) {
//!     (a.clone(), a) // not Clone
//! }
//! ```
//!
//! ```compile_fail,E0451
//! use nexus_governed_control::authority::approval::MissionApproval;
//! fn forge() -> MissionApproval {
//!     MissionApproval { contract_hash: String::new(), request: String::new(), evidence_seq: 1 }
//! }
//! ```
//!
//! ```compile_fail,E0624
//! let _ = nexus_governed_control::authority::approval::MissionApproval::confirmed;
//! ```

#![deny(unsafe_code)]

pub mod authority;
pub mod broker;
pub mod browser;
pub mod connector;
pub mod control;
pub mod display;
pub mod egress;
pub mod evidence_store;
pub mod executable;
pub mod governed;
pub mod ingress;
mod launcher;
pub(crate) mod ledger;
pub mod planned;
pub mod runtime_root;
pub mod synthetic;
pub mod tool;

#[cfg(test)]
mod harness_tests;
