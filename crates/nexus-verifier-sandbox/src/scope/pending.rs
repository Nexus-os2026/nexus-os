//! Ownership of a scope operation whose remote effect is not yet known
//! (P2-V1-R3B-I4).
//!
//! One execution's scope is a [`ScopeBoundary`]:
//!
//! - `None`: no operation exists.
//! - `Pending`: a [`PendingScope`], owned from before StartTransientUnit is
//!   issued until either every proof passed or settling confirmed that
//!   nothing the request may have created can still hold a process.
//! - `Proven`: a [`Scope`], the exact cgroup, retained by descriptor, that
//!   holds the helper with exactly the requested limits and policy.
//!
//! The only transitions are None → Pending (the operation is prepared and
//! owned before its request is issued), Pending → Proven (every proof
//! passed; in place, so the owner never lets go) and Pending → gone
//! ([`PendingScope::settle`] confirmed it). A unit name, a process id or a
//! cgroup path never creates, promotes or confirms one.
//!
//! What confirms a pending operation gone:
//!
//! - nothing was issued: nothing can exist;
//! - the manager refused the name as already loaded (`UnitExists`): the
//!   request had no effect, so the unit of that name is not this request's
//!   and is never stopped or claimed; confirmed once the kernel reports the
//!   helper in a cgroup not of that name;
//! - a candidate cgroup is retained (the kernel reported the helper in it,
//!   so the start job has run): it is empty or removed, exactly as for a
//!   proven scope;
//! - no candidate: the manager answers on the issuing connection, after the
//!   request (one connection's calls are answered in order), that no unit
//!   of the name is loaded, so neither the unit nor its start job remains;
//!   and then the kernel reports the helper in a cgroup not of that name.
//!
//! A timeout is evidence of uncertainty, never of absence. A StopUnit reply
//! confirms nothing, and a failed or timed-out one never proves that
//! nothing was stopped: after every attempt what remains is observed again.
//! Whatever cannot be confirmed stays owned, and settling can be retried
//! for as long as the owner keeps it, after the [`ScopeManager`] that
//! started it is gone. Dropping an unresolved operation is defense in depth
//! only: it ends what its candidate holds and confirms nothing.
//!
//! The helper bound to an unresolved operation must stay unreaped: its
//! process id stays reserved, so a still-queued start job cannot attach a
//! reused id, and its kernel membership stays observable.
//!
//! [`ScopeManager`]: super::ScopeManager

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use super::manager::{Presence, Remote, ScopeRequest, Started};
use super::native::CgroupDir;
use super::{occupancy, read_populated, verify_limits, Controller, Occupancy, Scope, ScopeError};
use crate::fault::{self, Fault, FaultPoint};
use crate::launcher::Helper;
use crate::policy::ResourcePolicy;

/// A scope operation that may have had an effect the backend has neither
/// proven nor confirmed gone: the owner of everything its request may have
/// created. Never constructed from a name, a process id or a path, never
/// serialized and never copied.
pub struct PendingScope {
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
    /// The cgroup the kernel reported the helper in, retained by descriptor
    /// as soon as it was seen: cleanup ownership only, never scope
    /// authority.
    candidate: Option<Box<dyn CgroupDir>>,
    /// StopUnit attempts, and the last one's outcome: diagnostics only,
    /// never evidence.
    stops: u32,
    last_stop: Option<Remote<()>>,
    /// Confirmed gone: nothing is left to own.
    settled: bool,
}

/// A [`super::ScopeManager::start`] that proved no scope.
#[derive(Debug)]
pub struct StartFailed {
    /// Why no scope was proven.
    pub error: ScopeError,
    /// What the attempt may have created and could not be confirmed gone:
    /// owned here, with the helper kept unreaped, until
    /// [`PendingScope::settle`] confirms it. `None` once confirmed.
    pub unresolved: Option<Box<PendingScope>>,
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
            candidate: None,
            stops: 0,
            last_stop: None,
            settled: false,
        }
    }

    /// The backend-generated unit name (display and diagnostics only).
    pub fn unit(&self) -> &str {
        &self.unit
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

    /// Settle this operation: end whatever it may have created and confirm
    /// it gone. `true` once nothing it may have created can still hold or
    /// receive a process; `false` keeps it owned here, for another attempt.
    /// `helper` must be the retained, unreaped helper it was started for
    /// (another is ignored); killed first, it lets its cgroup empty.
    pub fn settle(&mut self, helper: &Helper) -> bool {
        self.reconcile(Some(helper), None)
    }

    /// [`Self::settle`], with the panic controls' fault injection.
    pub(crate) fn reconcile(&mut self, helper: Option<&Helper>, fault: Option<Fault>) -> bool {
        if !self.unresolved() {
            return true;
        }
        let controller = Arc::clone(&self.controller);
        let helper = helper.filter(|helper| self.binds(helper));
        if self.collided {
            // Refused before any effect: the unit of that name is not this
            // request's, so it is never stopped or claimed.
            fault::at(fault, FaultPoint::ScopeReconcile);
            self.settled = outside(&controller, helper, &self.unit);
            return self.settled;
        }
        // Cleanup ownership: everything in the candidate is ended.
        if let Some(candidate) = &self.candidate {
            let _ = candidate.kill();
        }
        self.acquire(&controller, helper);
        fault::at(fault, FaultPoint::ScopeReconcile);
        if self.gone(&controller, helper) {
            self.settled = true;
            return true;
        }
        fault::at(fault, FaultPoint::BeforeScopeStop);
        self.stops = self.stops.saturating_add(1);
        // Neither reply is a confirmation, nor a failure proof that nothing
        // was stopped: it is kept for diagnostics only, and what remains is
        // observed again.
        self.last_stop = Some(controller.manager.stop_unit(&self.unit));
        fault::at(fault, FaultPoint::AfterScopeStop);
        let start = Instant::now();
        loop {
            self.acquire(&controller, helper);
            fault::at(fault, FaultPoint::ScopeReconcile);
            if self.gone(&controller, helper) {
                self.settled = true;
                return true;
            }
            if start.elapsed() >= controller.timing.settle {
                return false;
            }
            std::thread::sleep(controller.timing.poll);
        }
    }

    /// Retain the cgroup the kernel now reports the bound helper in, if it
    /// is of this operation's unit and none is retained yet, and end
    /// everything in it.
    fn acquire(&mut self, controller: &Controller, helper: Option<&Helper>) {
        if self.candidate.is_some() {
            return;
        }
        let Some(helper) = helper else {
            return;
        };
        if let Ok(Some(path)) = controller.native.membership(helper) {
            if names_unit(&path, &self.unit) {
                if let Ok(candidate) = controller.native.open(&path) {
                    let _ = candidate.kill();
                    self.candidate = Some(candidate);
                }
            }
        }
    }

    /// Whether nothing this operation may have created can still hold or
    /// receive a process (see the module documentation).
    fn gone(&self, controller: &Controller, helper: Option<&Helper>) -> bool {
        match &self.candidate {
            Some(candidate) => matches!(
                occupancy(candidate.as_ref()),
                Ok(Occupancy::Empty | Occupancy::Removed)
            ),
            None => absent(controller, &self.unit, helper),
        }
    }

    /// End what the candidate holds, without waiting or calling the
    /// manager. Defense in depth only, never a confirmation.
    pub(crate) fn end_now(&self) {
        if self.unresolved() {
            if let Some(candidate) = &self.candidate {
                let _ = candidate.kill();
            }
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
        let uncertain = match started {
            Started::Accepted => None,
            Started::Collision => {
                self.collided = true;
                return Err(ScopeError::Bus(
                    "StartTransientUnit: the unit exists".into(),
                ));
            }
            Started::Uncertain(reason) => {
                // The request may or may not have taken effect. It ended
                // without one only if the manager now answers that no unit
                // of the name is loaded and the helper is outside it;
                // otherwise it is discovered and proven like an accepted
                // one, or settled.
                fault::at(fault, FaultPoint::ScopeReconcile);
                if absent(&controller, &self.unit, Some(helper)) {
                    return Err(ScopeError::Bus(reason));
                }
                Some(reason)
            }
        };
        // A helper never placed after an uncertain request: the request's
        // own failure is what is reported.
        self.prove(&controller, helper, fault)
            .map_err(|error| match (error, uncertain) {
                (ScopeError::NotPlaced, Some(reason)) => ScopeError::Bus(reason),
                (error, _) => error,
            })
    }

    /// Prove the scope: the kernel reports the helper in a cgroup of the
    /// unit, retained at once by descriptor; it is a populated cgroup v2
    /// directory holding the helper with exactly the requested limits; and
    /// the manager has the unit loaded with the requested runtime backstop
    /// and out-of-memory policy.
    fn prove(
        &mut self,
        controller: &Controller,
        helper: &Helper,
        fault: Option<Fault>,
    ) -> Result<(), ScopeError> {
        let path = wait_for_placement(controller, helper, &self.unit)?;
        fault::at(fault, FaultPoint::ScopeProof);
        // Cleanup ownership first: the cgroup the helper is in is retained
        // before any later check can fail, so none can lose it.
        let candidate = &**self.candidate.insert(controller.native.open(&path)?);
        fault::at(fault, FaultPoint::ScopeCandidate);
        // A live, non-root cgroup: its `cgroup.events` exists and reports
        // the helper's presence. Only then can a later absence of that file
        // mean removal.
        if read_populated(candidate).map_err(ScopeError::Io)? != Occupancy::Populated {
            return Err(ScopeError::Mismatch("scope not populated"));
        }
        let procs = candidate.read("cgroup.procs").map_err(ScopeError::Io)?;
        if !procs
            .lines()
            .any(|line| line.trim() == helper.pid().to_string())
        {
            return Err(ScopeError::Mismatch("helper not in the scope"));
        }
        verify_limits(candidate, &self.limits)?;
        let unit_path = match controller.manager.get_unit(&self.unit) {
            Remote::Answered(Presence::Present(path)) => path,
            // The kernel and the manager disagree: nothing is proven.
            Remote::Answered(Presence::Absent) => {
                return Err(ScopeError::Mismatch("unit not loaded"))
            }
            Remote::Uncertain(reason) => return Err(ScopeError::Bus(reason)),
        };
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
        Ok(())
    }

    /// What tests observe about the operation.
    #[cfg(test)]
    pub(crate) fn state(&self) -> PendingState {
        PendingState {
            issued: self.issued,
            collided: self.collided,
            candidate: self.candidate.is_some(),
            stops: self.stops,
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
            .field("candidate", &self.candidate.is_some())
            .field("stops", &self.stops)
            .field("last_stop", &self.last_stop)
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
    pub candidate: bool,
    pub stops: u32,
    pub settled: bool,
}

/// Whether the cgroup `path` (as the kernel reports it) is the unit's own.
fn names_unit(path: &str, unit: &str) -> bool {
    path.rsplit('/').next() == Some(unit) && !path.contains("..")
}

/// The kernel reports the bound helper in a cgroup not of the unit.
fn outside(controller: &Controller, helper: Option<&Helper>, unit: &str) -> bool {
    helper.is_some_and(|helper| {
        matches!(controller.native.membership(helper), Ok(Some(path)) if !names_unit(&path, unit))
    })
}

/// The manager answers, on the issuing connection, that no unit of the name
/// is loaded, and then the kernel reports the bound helper outside it.
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
    /// whatever it retained.
    pub(crate) fn establish(
        &mut self,
        helper: &Helper,
        fault: Option<Fault>,
    ) -> Result<(), ScopeError> {
        let Self::Pending(pending) = self else {
            return Err(ScopeError::Mismatch("scope operation"));
        };
        pending.issue_and_prove(helper, fault)?;
        // Every proof passed on the retained candidate: it is the scope.
        let Some(dir) = pending.candidate.take() else {
            return Err(ScopeError::Mismatch("scope not proven"));
        };
        pending.settled = true;
        let unit = std::mem::take(&mut pending.unit);
        *self = Self::Proven(Scope { unit, dir });
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

    /// The pending operation, if one is held.
    pub(crate) fn into_pending(self) -> Option<Box<PendingScope>> {
        match self {
            Self::Pending(pending) => Some(pending),
            _ => None,
        }
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
