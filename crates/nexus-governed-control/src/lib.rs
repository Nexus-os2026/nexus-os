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
//! Each absence is checked by naming the method as a path, never by calling
//! it, so the block fails only because the method is absent, whatever its
//! signature would be. The same form compiles for the front door's own
//! methods (the owner's side), and for the door's:
//!
//! ```
//! use nexus_governed_control::governed::{AgentDoor, GovernedControl};
//! let _ = GovernedControl::authority;
//! let _ = GovernedControl::open_owner_run;
//! let _ = GovernedControl::authorize;
//! let _ = GovernedControl::execute;
//! let _ = GovernedControl::request_grant;
//! let _ = GovernedControl::resume;
//! let _ = GovernedControl::emergency_stop;
//! let _ = GovernedControl::issue_agent_door;
//! let _ = AgentDoor::open_run;
//! let _ = AgentDoor::confirm_held_mission;
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::authority; // no authority
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::control; // no front door
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::open_owner_run; // no owner run
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::authorize; // no authorization
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::execute; // no execution
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::request_grant; // no grant
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::resume; // no lift of the emergency stop
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::emergency_stop; // not the owner's stop
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::issue_agent_door; // no issuer
//! ```
//!
//! ```compile_fail,E0599
//! let _ = nexus_governed_control::governed::AgentDoor::close; // no close
//! ```
//!
//! The same absences, as probes that compile only while each method is
//! absent (Codex re-check: a generic method would make a path negative
//! fail for another reason, and stable rustdoc does not check error codes).
//! Method lookup tries the door's own methods taking `self`, `&self` and
//! `&mut self` before this trait's `&mut self` one, so a real `AgentDoor`
//! method of any of these names, of any signature or generics, is chosen
//! instead and breaks this build:
//!
//! ```
//! use nexus_governed_control::governed::AgentDoor;
//! struct Absent;
//! trait Probe {
//!     fn authority(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn control(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn open_owner_run(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn authorize(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn execute(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn request_grant(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn resume(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn emergency_stop(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn issue_agent_door(&mut self) -> Absent {
//!         Absent
//!     }
//!     fn close(&mut self) -> Absent {
//!         Absent
//!     }
//! }
//! impl Probe for AgentDoor {}
//! fn probe(mut door: AgentDoor) {
//!     let _: Absent = door.authority();
//!     let _: Absent = door.control();
//!     let _: Absent = door.open_owner_run();
//!     let _: Absent = door.authorize();
//!     let _: Absent = door.execute();
//!     let _: Absent = door.request_grant();
//!     let _: Absent = door.resume();
//!     let _: Absent = door.emergency_stop();
//!     let _: Absent = door.issue_agent_door();
//!     let _: Absent = door.close();
//! }
//! ```
//!
//! The probes cannot see through `Deref`, and a `Deref` to the front door
//! would be the worst escape: `AgentDoor` implements neither `Deref` nor
//! `DerefMut` (with either one, the marker below is ambiguous and this
//! build breaks):
//!
//! ```
//! use nexus_governed_control::governed::AgentDoor;
//! trait AmbiguousIfDeref<A> {
//!     fn some_item() {}
//! }
//! impl<T: ?Sized> AmbiguousIfDeref<()> for T {}
//! struct InvalidDeref;
//! impl<T: ?Sized + std::ops::Deref> AmbiguousIfDeref<InvalidDeref> for T {}
//! let _ = <AgentDoor as AmbiguousIfDeref<_>>::some_item;
//!
//! trait AmbiguousIfDerefMut<A> {
//!     fn some_item() {}
//! }
//! impl<T: ?Sized> AmbiguousIfDerefMut<()> for T {}
//! struct InvalidDerefMut;
//! impl<T: ?Sized + std::ops::DerefMut> AmbiguousIfDerefMut<InvalidDerefMut> for T {}
//! let _ = <AgentDoor as AmbiguousIfDerefMut<_>>::some_item;
//! ```
//!
//! A door and the owner's mission and resume approvals cannot be built
//! outside the crate, by a literal or a crate-private constructor, and a
//! door cannot be copied (A1-b):
//!
//! The struct-update form compiles on a type whose fields are public, so a
//! literal block fails only for privacy, whatever the fields are called:
//!
//! ```
//! use nexus_governed_control::authority::approval::MissionResumeConfirmation;
//! fn rebuild(c: MissionResumeConfirmation) -> MissionResumeConfirmation {
//!     MissionResumeConfirmation { ..c }
//! }
//! ```
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
//! fn forge(approval: MissionResumeApproval) -> MissionResumeApproval {
//!     MissionResumeApproval { ..approval } // the fields are private
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
//! fn forge(approval: MissionApproval) -> MissionApproval {
//!     MissionApproval { ..approval } // the fields are private
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
#[cfg(test)]
pub(crate) mod test_hooks;
