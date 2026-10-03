//! Ownership of a scope operation whose remote effect is not yet known
//! (P2-V1-R3B-I4, -R1, -R2, -R3, -R3-R1).
//!
//! One execution's scope is a [`ScopeBoundary`]:
//!
//! - `None`: no operation exists.
//! - `Pending`: a [`PendingScope`], owned from before StartTransientUnit is
//!   issued until either every proof passed or settling confirmed that
//!   nothing the request may have created can still hold a process.
//! - `Proven`: a [`Scope`], the exact cgroup, retained by descriptor, that
//!   holds the helper with exactly the requested limits and policy and that
//!   the manager reports as exactly the control group of the unit
//!   invocation this request started.
//!
//! The only transitions are None → Pending (the operation is prepared and
//! owned before its request is issued), Pending → Proven (every proof
//! passed; in place, so the owner never lets go) and Pending → gone
//! (settling confirmed it). A unit name, a process id or a cgroup path
//! never creates, promotes or confirms one. Nothing outside the crate holds
//! a pending operation: it lives only in an owner that also owns its helper
//! (an execution's, its retained boundary, or the live harness's scoped
//! helper), and it is settled only by the execution's finalizer, which
//! kills that helper, unreaped, first.
//!
//! **Retained identity, cleanup authority and scope authority
//! (P2-V1-R3B-I4-R3-R1).** Observing the helper inside a cgroup is not by
//! itself authority over that cgroup: a foreign unit's cgroup can hold the
//! helper (a refusal as already loaded whose answer was lost, or a unit
//! loaded under the same name once this request's was gone). Three states
//! are kept apart:
//!
//! - an *observed candidate*: the cgroup the kernel reported the bound
//!   helper in, of the unit's name, retained by descriptor (its native
//!   identity) with the path it was opened from. No cleanup and no launch
//!   authority: it is never killed or proven, and its emptiness confirms
//!   nothing;
//! - an *owned candidate* (cleanup authority): the observed candidate bound
//!   to the unit invocation this request's start began
//!   ([`PendingScope::bind`]). Only an accepted start whose identity was
//!   captured from its own job (`super::manager`) can bind one: the kernel
//!   reports the bound helper at exactly the path the descriptor was opened
//!   from (read after the open, the directory not removed after that read),
//!   and the manager's unit for the name has exactly the captured
//!   `InvocationID`, its `Id` exactly the generated name, its `ControlGroup`
//!   exactly that path and its `ControlGroupId` exactly the descriptor's own
//!   kernel cgroup ID, the identity read again last. Identities are random
//!   and never reused, so the reads between two identical ones were
//!   answered for that one invocation. Only an owned candidate receives
//!   `cgroup.kill`, and only its emptiness confirms the operation;
//! - the *proven scope* (scope authority): an owned candidate that is also
//!   populated, lists the helper in `cgroup.procs` and carries exactly the
//!   requested limits, of a unit with exactly the requested runtime backstop
//!   and out-of-memory policy, the identity read again after them. Only a
//!   proven scope is launched into. A policy that fails after ownership
//!   refuses the launch and leaves the cleanup to the owned candidate.
//!
//! What confirms a pending operation gone:
//!
//! - nothing was issued, or the request was never sent: nothing can exist;
//! - the manager refused the name as already loaded (`UnitExists`): the
//!   request had no effect, so the unit of that name is not this request's
//!   and is never opened, killed or claimed; confirmed once the kernel
//!   reports the helper in a cgroup not of that name;
//! - an owned candidate is empty or removed, exactly as for a proven scope;
//! - without an owned candidate, after StartTransientUnit's reply (its job
//!   path) was delivered and recorded: GetUnit answers `NoSuchUnit` (it was
//!   sent after that reply arrived, so after the request was handled, and a
//!   unit that still has a job is not unloaded), and then the kernel reports
//!   the helper in a cgroup not of that name.
//!
//! Without a delivered and recorded reply (a timeout, a broken transport,
//! an unexpected error, a reply that does not decode, or a panic before the
//! reply was recorded) nothing identifies what the request may have
//! started: such an operation never opens, owns, ends or proves a
//! candidate (P2-V1-R3B-I4-R3-R1), and nothing confirms it. D-Bus delivers
//! one peer's messages to another in the order they were sent, but a
//! recipient need not process or answer calls in that order, so a later
//! `NoSuchUnit` does not show that the request has had, or will have, no
//! effect. It stays owned, its helper unreaped and its execution's cleanup
//! unconfirmed, possibly for the life of the backend process: the accepted
//! availability cost. An accepted start whose identity could not be
//! captured likewise never owns a candidate; only the manager's absence
//! confirms it.
//!
//! **No name actuation (P2-V1-R3B-I4-R2, -R3).** No pending operation is
//! ever acted upon by its unit name: nothing is stopped, killed or changed
//! through the manager. The name is a locator and evidence, never a
//! retained identity, however random. A recorded start reply shows that the
//! manager created a unit for this request at that moment; it does not bind
//! a unit loaded under the name later (this one may have been unloaded and
//! a foreign one loaded under the same name), so it changes what the
//! manager's absence can confirm (above), never what may be acted upon. An
//! uncertain start may as well have been refused as already loaded with that
//! answer lost, and a collision's unit is foreign. What an operation created
//! is ended only through its owned candidate (`cgroup.kill` through the
//! descriptor) and confirmed only once that cgroup is observed empty or
//! removed. A unit loaded under the name can delay that confirmation; it is
//! never acted upon.
//!
//! A timeout is evidence of uncertainty, never of absence. Whatever cannot
//! be confirmed stays owned, and settling can be retried for as long as the
//! owner keeps it, after the [`ScopeManager`] that started it is gone.
//! Dropping an unresolved operation is defense in depth only: it ends what
//! an owned candidate holds and confirms nothing.
//!
//! The helper bound to an unresolved operation must stay unreaped: its
//! process id stays reserved, so a still-queued start job cannot attach a
//! reused id, and its kernel membership stays observable.
//!
//! [`ScopeManager`]: super::ScopeManager

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use super::manager::{Presence, Remote, ScopeRequest, Started, UnitInstance};
use super::native::CgroupDir;
use super::{occupancy, read_populated, verify_limits, Controller, Occupancy, Scope, ScopeError};
use crate::fault::{self, Fault, FaultPoint};
use crate::launcher::Helper;
use crate::policy::ResourcePolicy;

/// The cgroup the kernel reported the bound helper in, retained by
/// descriptor as soon as it was seen (see the module documentation). It
/// exists only as opened once: never reconstructed from its path, and its
/// ownership belongs to that very descriptor.
struct Candidate {
    /// The retained directory: the cgroup's native identity.
    dir: Box<dyn CgroupDir>,
    /// The path it was opened from, as the kernel reported it for the
    /// helper: what the manager's `ControlGroup` must be exactly.
    path: String,
    /// Cleanup authority: bound to the unit invocation this operation's
    /// start began ([`PendingScope::bind`]). Only then may `cgroup.kill`
    /// reach it. Never launch authority: that is [`ScopeBoundary::Proven`].
    owned: bool,
}

/// A scope operation that may have had an effect the backend has neither
/// proven nor confirmed gone: the owner of everything its request may have
/// created. Never constructed from a name, a process id or a path, never
/// serialized and never copied; crate-private, held only beside its helper.
pub(crate) struct PendingScope {
    /// The manager connection that issued the request, and the kernel
    /// observations: shared, so settling never needs the [`ScopeManager`].
    ///
    /// [`ScopeManager`]: super::ScopeManager
    controller: Arc<Controller>,
    /// The backend-generated unit name: the request's locator, no
    /// authority.
    unit: String,
    /// The retained helper this operation is bound to: its locator, and its
    /// identity within the backend process.
    helper_pid: u32,
    helper_serial: u64,
    limits: ResourcePolicy,
    /// StartTransientUnit may have been dispatched.
    issued: bool,
    /// The manager refused the request: a unit of that name was already
    /// loaded, and it is not this request's.
    collided: bool,
    /// The manager accepted the request: its reply, a job path, was
    /// delivered and recorded. Until then (an uncertain outcome, or a panic
    /// before it was recorded) the manager's absence confirms nothing. It
    /// changes only what that absence can confirm, never what may be acted
    /// upon (P2-V1-R3B-I4-R3).
    accepted: bool,
    /// The identity of the unit invocation this operation's start began,
    /// captured from that start's own job (`super::manager`) and recorded
    /// only with its delivered reply: never from a name, a path or a later
    /// read. The only thing a candidate can be bound to.
    instance: Option<UnitInstance>,
    /// The candidate, observed or owned (see the module documentation).
    candidate: Option<Candidate>,
    /// Confirmed gone: nothing is left to own.
    settled: bool,
}

impl PendingScope {
    pub(super) fn new(
        controller: Arc<Controller>,
        unit: String,
        helper: &Helper,
        limits: &ResourcePolicy,
    ) -> Self {
        Self {
            controller,
            unit,
            helper_pid: helper.pid(),
            helper_serial: helper.serial(),
            limits: *limits,
            issued: false,
            collided: false,
            accepted: false,
            instance: None,
            candidate: None,
            settled: false,
        }
    }

    /// Whether `helper` is exactly the retained helper this operation is
    /// bound to.
    fn binds(&self, helper: &Helper) -> bool {
        helper.serial() == self.helper_serial && helper.pid() == self.helper_pid
    }

    /// Whether something this operation may have created can still exist.
    pub(crate) fn unresolved(&self) -> bool {
        self.issued && !self.settled
    }

    /// Settle this operation: end whatever it owns and confirm it gone.
    /// `true` once nothing it may have created can still hold or receive a
    /// process; `false` keeps it owned, for another attempt. Only the
    /// execution's finalizer settles, after killing `helper`, the bound
    /// helper its owner keeps unreaped (another helper is ignored). Nothing
    /// is acted upon by the unit's name, and nothing is ended but an owned
    /// candidate (see the module documentation).
    pub(crate) fn reconcile(&mut self, helper: Option<&Helper>, fault: Option<Fault>) -> bool {
        if !self.unresolved() {
            return true;
        }
        let controller = Arc::clone(&self.controller);
        let helper = helper.filter(|helper| self.binds(helper));
        if self.collided {
            // Refused before any effect: the unit of that name is not this
            // request's, so it is never opened, killed or claimed.
            fault::at(fault, FaultPoint::ScopeReconcile);
            self.settled = outside(&controller, helper, &self.unit);
            return self.settled;
        }
        // Cleanup authority: everything in an owned candidate is ended, and
        // nothing elsewhere. Nothing is acted upon by the unit's name
        // (P2-V1-R3B-I4-R3): what the operation created is ended only through
        // its owned candidate, by descriptor, and confirmed only by
        // observation.
        self.end_owned();
        self.observe(&controller, helper, fault)
    }

    /// Observe, within the settling bound, until nothing this operation may
    /// have created can still hold or receive a process: a candidate is
    /// retained as soon as the kernel reports the bound helper in a cgroup
    /// of the unit, its ownership is sought once per attempt, an owned one
    /// is ended through its descriptor and must be seen empty or removed
    /// (without one, an accepted operation needs the manager's absence of
    /// the unit with the helper outside). Only observation and reads:
    /// nothing is acted upon by the unit's name. `false` keeps the
    /// operation owned, for another attempt.
    fn observe(
        &mut self,
        controller: &Controller,
        helper: Option<&Helper>,
        fault: Option<Fault>,
    ) -> bool {
        let start = Instant::now();
        let mut sought = false;
        loop {
            self.acquire(controller, helper);
            if !sought && self.candidate.is_some() {
                sought = true;
                if self.own(controller, helper) {
                    self.end_owned();
                }
            }
            fault::at(fault, FaultPoint::ScopeReconcile);
            if self.gone(controller, helper) {
                self.settled = true;
                return true;
            }
            if start.elapsed() >= controller.timing.settle {
                return false;
            }
            std::thread::sleep(controller.timing.poll);
        }
    }

    /// Retain, without acting on it, the cgroup the kernel now reports the
    /// bound helper in, if it is of this operation's unit and none is
    /// retained yet: an observed candidate. Only an operation whose start's
    /// identity was captured retains one; no other could ever own it.
    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {
        if self.candidate.is_some() || self.instance.is_none() {
            return;
        }
        let Some(helper) = helper else {
            return;
        };
        if let Ok(Some(path)) = controller.native.membership(helper) {
            if names_unit(&path, &self.unit) {
                if let Ok(dir) = controller.native.open(&path) {
                    self.candidate = Some(Candidate {
                        dir,
                        path,
                        owned: false,
                    });
                }
            }
        }
    }

    /// Seek cleanup ownership of the observed candidate ([`Self::bind`],
    /// reads only); `true` only when this call established it.
    fn own(&mut self, controller: &Controller, helper: Option<&Helper>) -> bool {
        let Some(helper) = helper else {
            return false;
        };
        if self
            .candidate
            .as_ref()
            .is_none_or(|candidate| candidate.owned)
        {
            return false;
        }
        self.bind(controller, helper, None).is_ok()
    }

    /// Bind the observed candidate to the unit invocation this operation's
    /// start began: cleanup ownership (see the module documentation). Reads
    /// only; nothing is ended here. On success the candidate is owned and
    /// the unit's object path (GetUnit's answer) is returned for the policy
    /// proof; on any failure, uncertainty included, it stays observed.
    fn bind(
        &mut self,
        controller: &Controller,
        helper: &Helper,
        fault: Option<Fault>,
    ) -> Result<String, ScopeError> {
        let Some(instance) = self.instance else {
            return Err(ScopeError::Mismatch("unit instance"));
        };
        let Some(candidate) = self.candidate.as_mut() else {
            return Err(ScopeError::Mismatch("scope candidate"));
        };
        // The bound helper is inside the retained directory: the kernel
        // reports it at exactly the path the directory was opened from, read
        // after the open, and the directory is not removed after that read
        // (a removed cgroup never returns, and a path names one live cgroup
        // at a time).
        match controller
            .native
            .membership(helper)
            .map_err(ScopeError::Io)?
        {
            Some(path) if path == candidate.path => {}
            _ => return Err(ScopeError::Mismatch("helper not in the candidate")),
        }
        read_populated(candidate.dir.as_ref()).map_err(ScopeError::Io)?;
        let unit_path = match controller.manager.get_unit(&self.unit) {
            Remote::Answered(Presence::Present(path)) => path,
            // The kernel and the manager disagree: nothing is bound.
            Remote::Answered(Presence::Absent) => {
                return Err(ScopeError::Mismatch("unit not loaded"))
            }
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        };
        fault::at(fault, FaultPoint::ScopeBinding);
        // The unit at that object path is the invocation this start began,
        // and stays it through every read below: its identity is compared
        // exactly before and after them.
        same_instance(controller, &unit_path, instance)?;
        match controller.manager.unit_id(&unit_path) {
            Remote::Answered(Some(id)) if id == self.unit => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit id")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        // Its control group is exactly the path the kernel reports the
        // helper in and the candidate was opened from, never merely one with
        // the same last component; and exactly the directory retained: the
        // manager's record of its kernel cgroup ID is the descriptor's own.
        match controller.manager.control_group(&unit_path) {
            Remote::Answered(Some(group)) if group == candidate.path => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        let retained = candidate.dir.cgroup_id().map_err(ScopeError::Io)?;
        match controller.manager.control_group_id(&unit_path) {
            Remote::Answered(Some(id)) if id == retained => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("unit control group id")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        same_instance(controller, &unit_path, instance)?;
        candidate.owned = true;
        Ok(unit_path)
    }

    /// The owned candidate's directory, if the candidate is owned.
    fn owned_dir(&self) -> Option<&dyn CgroupDir> {
        self.candidate
            .as_ref()
            .filter(|candidate| candidate.owned)
            .map(|candidate| candidate.dir.as_ref())
    }

    /// Whether nothing this operation may have created can still hold or
    /// receive a process (see the module documentation).
    fn gone(&self, controller: &Controller, helper: Option<&Helper>) -> bool {
        if let Some(owned) = self.owned_dir() {
            return matches!(occupancy(owned), Ok(Occupancy::Empty | Occupancy::Removed));
        }
        // An observed candidate's emptiness confirms nothing, and without a
        // delivered reply the manager's absence proves nothing.
        self.accepted && absent(controller, &self.unit, helper)
    }

    /// End everything in the owned candidate, without waiting or calling
    /// the manager: the only `cgroup.kill` of a pending operation. An
    /// observed candidate is never ended.
    fn end_owned(&self) {
        if let Some(owned) = self.owned_dir() {
            let _ = owned.kill();
        }
    }

    /// End what an owned candidate holds, without waiting or calling the
    /// manager. Defense in depth only, never a confirmation.
    pub(crate) fn end_now(&self) {
        if self.unresolved() {
            self.end_owned();
        }
    }

    /// Issue the request and prove what it created.
    fn issue_and_prove(&mut self, helper: &Helper, fault: Option<Fault>) -> Result<(), ScopeError> {
        if self.issued || !self.binds(helper) {
            return Err(ScopeError::Mismatch("scope operation"));
        }
        let controller = Arc::clone(&self.controller);
        fault::at(fault, FaultPoint::BeforeScopeStart);
        // From here the request may have an effect: it is owned.
        self.issued = true;
        let started = controller.manager.start_scope(&ScopeRequest {
            unit: &self.unit,
            helper_pid: helper.pid(),
            limits: &self.limits,
        });
        fault::at(fault, FaultPoint::AfterScopeStart);
        match started {
            Started::Accepted(captured) => {
                self.accepted = true;
                // The identity of the invocation this start began, captured
                // from its own job, or none: then nothing it created is ever
                // owned or proven, and only the manager's absence confirms it.
                self.instance = Some(captured.map_err(ScopeError::Bus)?);
            }
            Started::Collision => {
                self.collided = true;
                return Err(ScopeError::Bus(
                    "StartTransientUnit: the unit exists".into(),
                ));
            }
            // The request may or may not have taken effect, or take it
            // later, and nothing identifies what it may have started: it is
            // never proven, and nothing of it is ever opened, owned or ended
            // (P2-V1-R3B-I4-R3-R1).
            Started::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
            // Never sent: nothing can exist.
            Started::NotIssued(reason) => {
                self.settled = true;
                return Err(ScopeError::Bus(reason));
            }
        }
        self.prove(&controller, helper, fault)
    }

    /// Prove the scope: the kernel reports the helper in a cgroup of the
    /// unit, retained at once by descriptor (observed); it is a populated
    /// cgroup v2 directory listing the helper; it is bound to the unit
    /// invocation this start began (owned); it carries exactly the
    /// requested limits, and the unit exactly the requested runtime backstop
    /// and out-of-memory policy, still that invocation.
    fn prove(
        &mut self,
        controller: &Controller,
        helper: &Helper,
        fault: Option<Fault>,
    ) -> Result<(), ScopeError> {
        let Some(instance) = self.instance else {
            return Err(ScopeError::Mismatch("unit instance"));
        };
        let path = wait_for_placement(controller, helper, &self.unit)?;
        fault::at(fault, FaultPoint::ScopeProof);
        // Retained first, before any later check can fail, so that none can
        // lose it: observed, with no authority yet.
        let dir = controller.native.open(&path)?;
        let candidate = &*self.candidate.insert(Candidate {
            dir,
            path,
            owned: false,
        });
        fault::at(fault, FaultPoint::ScopeCandidate);
        // A live, non-root cgroup: its `cgroup.events` exists and reports
        // the helper's presence. Only then can a later absence of that file
        // mean removal.
        let dir = candidate.dir.as_ref();
        if read_populated(dir).map_err(ScopeError::Io)? != Occupancy::Populated {
            return Err(ScopeError::Mismatch("scope not populated"));
        }
        let procs = dir.read("cgroup.procs").map_err(ScopeError::Io)?;
        if !procs
            .lines()
            .any(|line| line.trim() == helper.pid().to_string())
        {
            return Err(ScopeError::Mismatch("helper not in the scope"));
        }
        // Cleanup authority before any policy is proven: from here a failure
        // leaves what the start created to be ended through the descriptor.
        let unit_path = self.bind(controller, helper, fault)?;
        fault::at(fault, FaultPoint::ScopeOwned);
        // Scope authority: the requested sandbox policy, on the owned
        // candidate only.
        let Some(owned) = self.owned_dir() else {
            return Err(ScopeError::Mismatch("scope not proven"));
        };
        verify_limits(owned, &self.limits)?;
        fault::at(fault, FaultPoint::ScopeProperties);
        match controller.manager.runtime_max_usec(&unit_path) {
            Remote::Answered(Some(backstop))
                if backstop == self.limits.runtime_backstop_secs * 1_000_000 => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("runtime backstop")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        match controller.manager.oom_policy(&unit_path) {
            Remote::Answered(Some(policy)) if policy == "continue" => {}
            Remote::Answered(_) => return Err(ScopeError::Mismatch("out-of-memory policy")),
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        }
        // Those answers were the captured invocation's.
        same_instance(controller, &unit_path, instance)
    }

    /// What tests observe about the operation.
    #[cfg(test)]
    pub(crate) fn state(&self) -> PendingState {
        PendingState {
            issued: self.issued,
            collided: self.collided,
            accepted: self.accepted,
            instance: self.instance.is_some(),
            candidate: self.candidate.is_some(),
            owned: self.owned_dir().is_some(),
            settled: self.settled,
        }
    }
}

impl fmt::Debug for PendingScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingScope")
            .field("unit", &self.unit)
            .field("helper_pid", &self.helper_pid)
            .field("issued", &self.issued)
            .field("collided", &self.collided)
            .field("accepted", &self.accepted)
            .field("instance", &self.instance.is_some())
            .field("candidate", &self.candidate.is_some())
            .field("owned", &self.owned_dir().is_some())
            .field("settled", &self.settled)
            .finish_non_exhaustive()
    }
}

impl Drop for PendingScope {
    /// Defense in depth for an operation dropped unresolved (see
    /// [`Self::end_now`]). Never a confirmation; owners keep an unresolved
    /// operation until settling succeeds.
    fn drop(&mut self) {
        self.end_now();
    }
}

/// What tests observe about a pending scope operation.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingState {
    pub issued: bool,
    pub collided: bool,
    pub accepted: bool,
    /// The start's own identity was captured.
    pub instance: bool,
    /// A candidate is retained, observed or owned.
    pub candidate: bool,
    /// The candidate is owned: cleanup authority.
    pub owned: bool,
    pub settled: bool,
}

/// The manager's unit at `unit_path` is exactly the captured invocation:
/// its `InvocationID`, the 16 bytes compared as they are.
fn same_instance(
    controller: &Controller,
    unit_path: &str,
    instance: UnitInstance,
) -> Result<(), ScopeError> {
    match controller.manager.unit_instance(unit_path) {
        Remote::Answered(Some(current)) if current == instance => Ok(()),
        Remote::Answered(_) => Err(ScopeError::Mismatch("unit instance")),
        Remote::Uncertain(reason) => Err(ScopeError::Bus(reason)),
    }
}

/// Whether the cgroup `path` (as the kernel reports it, relative to the
/// cgroup root) is absolute, in normal form (no empty, `.` or `..`
/// component) and named for the unit: a locator of the unit's possible
/// cgroup, never a binding to the unit.
fn names_unit(path: &str, unit: &str) -> bool {
    path.strip_prefix('/')
        .is_some_and(|relative| relative.split('/').all(|part| !matches!(part, "" | ".")))
        && path.rsplit('/').next() == Some(unit)
        && !path.contains("..")
}

/// The kernel reports the bound helper in a cgroup not of the unit.
fn outside(controller: &Controller, helper: Option<&Helper>, unit: &str) -> bool {
    helper.is_some_and(|helper| {
        matches!(controller.native.membership(helper), Ok(Some(path)) if !names_unit(&path, unit))
    })
}

/// The manager answers that no unit of the name is loaded, and then the
/// kernel reports the bound helper outside it: evidence only after a
/// delivered and recorded StartTransientUnit reply (see the module
/// documentation).
fn absent(controller: &Controller, unit: &str, helper: Option<&Helper>) -> bool {
    matches!(
        controller.manager.get_unit(unit),
        Remote::Answered(Presence::Absent)
    ) && outside(controller, helper, unit)
}

/// Wait until the kernel reports the helper in a cgroup whose last
/// component is `unit`, and return that cgroup's path (relative to the
/// cgroup root).
fn wait_for_placement(
    controller: &Controller,
    helper: &Helper,
    unit: &str,
) -> Result<String, ScopeError> {
    let start = Instant::now();
    loop {
        if let Some(path) = controller
            .native
            .membership(helper)
            .map_err(ScopeError::Io)?
        {
            if names_unit(&path, unit) {
                return Ok(path);
            }
        }
        if start.elapsed() >= controller.timing.placement {
            return Err(ScopeError::NotPlaced);
        }
        std::thread::sleep(controller.timing.poll);
    }
}

/// One execution's scope (see the module documentation).
#[derive(Debug, Default)]
pub(crate) enum ScopeBoundary {
    #[default]
    None,
    Pending(Box<PendingScope>),
    Proven(Scope),
}

impl ScopeBoundary {
    /// Issue the pending operation's request for `helper` and prove its
    /// scope. On success the operation has become the proven scope, in
    /// place; otherwise (a panic included) it stays pending here, with
    /// whatever it retained and owns.
    pub(crate) fn establish(
        &mut self,
        helper: &Helper,
        fault: Option<Fault>,
    ) -> Result<(), ScopeError> {
        let Self::Pending(pending) = self else {
            return Err(ScopeError::Mismatch("scope operation"));
        };
        pending.issue_and_prove(helper, fault)?;
        // Every proof passed on the owned candidate. Until the move below
        // the operation stays pending, its candidate owned: a panic here
        // launches nothing and leaves the cleanup to that candidate.
        fault::at(fault, FaultPoint::ScopePromotion);
        let Some(instance) = pending.instance else {
            return Err(ScopeError::Mismatch("scope not proven"));
        };
        let Some(candidate) = pending.candidate.take_if(|candidate| candidate.owned) else {
            return Err(ScopeError::Mismatch("scope not proven"));
        };
        pending.settled = true;
        let unit = std::mem::take(&mut pending.unit);
        *self = Self::Proven(Scope {
            unit,
            dir: candidate.dir,
            instance,
        });
        Ok(())
    }

    /// The proven scope, if there is one.
    pub(crate) fn proven(&self) -> Option<&Scope> {
        match self {
            Self::Proven(scope) => Some(scope),
            _ => None,
        }
    }

    pub(crate) fn is_proven(&self) -> bool {
        matches!(self, Self::Proven(_))
    }

    /// Whether a scope, or an operation that may have created one, is held.
    pub(crate) fn holds(&self) -> bool {
        match self {
            Self::None => false,
            Self::Pending(pending) => pending.unresolved(),
            Self::Proven(_) => true,
        }
    }

    /// Whether an unresolved operation is held: its helper must stay
    /// unreaped.
    pub(crate) fn unresolved(&self) -> bool {
        matches!(self, Self::Pending(pending) if pending.unresolved())
    }

    /// Best effort and without waiting: end what is held. Defense in depth
    /// only, never a confirmation.
    pub(crate) fn end_now(&self) {
        match self {
            Self::None => {}
            Self::Pending(pending) => pending.end_now(),
            Self::Proven(scope) => {
                let _ = scope.kill();
            }
        }
    }
}
