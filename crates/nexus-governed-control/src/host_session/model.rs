//! One private Bootstrap registration; no session inference route.
use super::*;
use crate::inference::{
    AbsoluteDeadline, CallDeadline, ModelError, ModelIdentityView, ModelPinSpec, SystemClocks,
};
use nexus_kernel::emergency_stop::StopWatch;
use nexus_kernel::inference::{
    CallCancel, CancelCause, LoopbackEndpoint, RequestKind, VerifiedOllama,
};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, TryLockError};

#[derive(Default)]
pub(super) struct ModelSlot {
    pub(super) state: Mutex<State>,
    pub(super) uncertain: AtomicBool,
    /// SB-3B1b test seams: the ordered gate trace, the backstop clock skew
    /// and the uncharged/spent chat-latch replays.
    #[cfg(test)]
    pub(super) trace: Mutex<Vec<&'static str>>,
    #[cfg(test)]
    pub(super) backstop_skew: Arc<std::sync::atomic::AtomicU64>,
    #[cfg(test)]
    pub(super) chat_replay: AtomicBool,
    #[cfg(test)]
    pub(super) chat_early: AtomicBool,
}
#[derive(Default)]
pub(super) struct State {
    attempted: bool,
    attempt: bool,
    active: bool,
    owner_complete: bool,
    child_complete: bool,
    record: Option<&'static str>,
    registration: Option<RegistrationReceipt>,
}
// These exact retained facts have no operational reader until SB-3.
struct RegistrationReceipt {
    #[expect(
        dead_code,
        reason = "SB-2a: private, wired by SB-3; session route unavailable"
    )]
    sequence: u64,
    digest: [u8; 32],
    model: Arc<crate::inference::RegisteredModel>,
    watch: StopWatch,
    primary: Arc<crate::evidence_store::PrimaryEvidence>,
}
impl ModelSlot {
    #[cfg(test)]
    pub(super) fn installed(&self) -> bool {
        self.state.lock().unwrap().registration.is_some()
    }
    /// SB-3B1b test seam: re-read the installed M identity (no authority).
    #[cfg(test)]
    pub(super) fn registered_identity_for_test(&self) -> Option<[u8; 32]> {
        let s = self.state.lock().unwrap();
        s.registration.as_ref().map(|r| r.digest)
    }
    pub(super) fn ready(&self) -> Result<bool, SessionError> {
        let s = self.state.try_lock().map_err(|e| {
            if matches!(e, TryLockError::Poisoned(_)) {
                self.uncertain.store(true, Ordering::SeqCst);
            }
            SessionError::Unavailable
        })?;
        Ok(!self.uncertain.load(Ordering::SeqCst)
            && !s.active
            && (!s.attempted || s.registration.is_some()))
    }
    pub(super) fn quiescent(&self) -> bool {
        self.snapshot().is_some_and(|o| {
            !o.active
                && o.owner_complete
                && o.child_complete
                && !o.uncertain
                && matches!(o.record, "not_required" | "recorded")
        })
    }
    pub(super) fn snapshot(&self) -> Option<close::ModelObligations> {
        let s = match self.state.try_lock() {
            Ok(s) => s,
            Err(TryLockError::Poisoned(_)) => {
                self.uncertain.store(true, Ordering::SeqCst);
                return None;
            }
            Err(TryLockError::WouldBlock) => return None,
        };
        Some(close::ModelObligations {
            operation: if s.attempt {
                "attempt"
            } else if s.attempted {
                "registration"
            } else {
                "none"
            },
            active: s.active,
            owner_complete: !(s.attempted || s.attempt) || s.owner_complete,
            child_complete: !(s.attempted || s.attempt) || s.child_complete,
            record: s.record.unwrap_or("not_required"),
            uncertain: self.uncertain.load(Ordering::SeqCst),
        })
    }
}
fn hold(core: &Arc<Core>) -> Result<MutexGuard<'_, State>, SessionError> {
    core.model.state.try_lock().map_err(|e| {
        if matches!(e, TryLockError::Poisoned(_)) {
            core.model.uncertain.store(true, Ordering::SeqCst);
            SessionIngress { core: core.clone() }.fail();
        }
        SessionError::Unavailable
    })
}
pub(super) struct Owner {
    core: Arc<Core>,
    done: bool,
}
pub(super) fn reserve(core: &Arc<Core>, scope: &Arc<RequestScope>) -> Result<Owner, SessionError> {
    // Same ordering as readiness and seal; never held across slow work.
    let _producers = core
        .producers
        .try_lock()
        .map_err(|_| SessionError::Unavailable)?;
    let mut s = hold(core)?;
    if core.cell.session_word() & SEALED != 0
        || s.attempted
        || core.model.uncertain.load(Ordering::SeqCst)
    {
        return Err(SessionError::Restricted);
    }
    s.attempted = true;
    s.owner_complete = true;
    s.child_complete = true;
    if !matches!(scope.kind, ScopeKind::Bootstrap)
        || scope.session != core.identity
        || !scope.core.ptr_eq(&Arc::downgrade(core))
        || scope.admit(&core.cell).is_err()
    {
        return Err(SessionError::Restricted);
    }
    s.active = true;
    s.owner_complete = false;
    s.child_complete = false;
    s.record = Some("pending");
    Ok(Owner {
        core: core.clone(),
        done: false,
    })
}
fn completion_deadline(core: &Arc<Core>) -> Instant {
    SessionIngress { core: core.clone() }
        .close_deadline()
        .map_or(Instant::now() + Duration::from_millis(100), |d| {
            d.min(Instant::now() + Duration::from_millis(100))
        })
}
fn completion_hold(core: &Arc<Core>) -> Result<MutexGuard<'_, State>, SessionError> {
    crate::authority::run::settlement_hold(&core.model.state, &core.cell, completion_deadline(core))
        .map_err(|_| {
            core.model.uncertain.store(true, Ordering::SeqCst);
            SessionIngress { core: core.clone() }.fail();
            SessionError::Unavailable
        })
}
impl Owner {
    pub(super) fn complete(mut self, record: &'static str) -> Result<(), SessionError> {
        let core = self.core.clone();
        let mut s = completion_hold(&core)?;
        self.complete_held(record, &mut s)
    }
    fn complete_held(&mut self, record: &'static str, s: &mut State) -> Result<(), SessionError> {
        s.child_complete = true;
        s.owner_complete = true;
        s.record = Some(record);
        if !matches!(record, "not_required" | "recorded")
            || self.core.model.uncertain.load(Ordering::SeqCst)
        {
            return Err(SessionError::Unavailable);
        }
        s.active = false;
        self.done = true;
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if !self.done {
            self.core.model.uncertain.store(true, Ordering::SeqCst);
            SessionIngress {
                core: self.core.clone(),
            }
            .fail();
        }
    }
}
pub(super) struct RegistrationGate<'a> {
    pub(super) core: &'a Arc<Core>,
    pub(super) scope: &'a Arc<RequestScope>,
    pub(super) primary: &'a crate::evidence_store::PrimaryEvidence,
    pub(super) watch: &'a StopWatch,
    pub(super) deadline: &'a dyn CallDeadline,
}
impl RegistrationGate<'_> {
    fn validate(&self) -> Result<(), ModelError> {
        self.watch.check_now().map_err(|_| ModelError::Stopped)?;
        if self.deadline.expired()
            || Instant::now() >= self.scope.deadline
            || (SessionIngress {
                core: self.core.clone(),
            })
            .close_deadline()
            .is_some_and(|d| Instant::now() >= d)
        {
            return Err(ModelError::Timeout);
        }
        if self.core.model.uncertain.load(Ordering::SeqCst)
            || !self.scope.live(self.core.cell.session_word())
            || !self.primary.try_health().is_some_and(|h| {
                !h.latched
                    && h.owed == 0
                    && !h.recovery_required
                    && h.unsettled == 0
                    && h.unclean_sessions == 0
            })
        {
            return Err(ModelError::Cancelled);
        }
        Ok(())
    }
    fn admission(&self, state: &State) -> Result<(), ModelError> {
        if !state.active || state.owner_complete {
            return Err(ModelError::Cancelled);
        }
        self.validate()?;
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&self.core.cell), "model.gate.checked");
        self.scope
            .admit(&self.core.cell)
            .map_err(|_| ModelError::Cancelled)
    }
}
impl CallCancel for RegistrationGate<'_> {
    fn cancelled(&self) -> Option<CancelCause> {
        // Deny-only polling; deliberately does not acquire the owning hold.
        (self.core.model.uncertain.load(Ordering::SeqCst)
            || !self.scope.live(self.core.cell.session_word()))
        .then_some(CancelCause::Cancelled)
    }
    fn admit_request(&self, kind: RequestKind) -> Result<(), ModelError> {
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&self.core.cell), "model.gate.before");
        if kind != RequestKind::Metadata {
            return Err(ModelError::ConfigMismatch);
        }
        let state = hold(self.core).map_err(|_| ModelError::Cancelled)?;
        self.admission(&state)
    }
}
impl HostSession {
    pub(super) fn register_model_private(
        &self,
        endpoint: &str,
        spec: &ModelPinSpec,
    ) -> Result<ModelIdentityView, SessionError> {
        let deadline = AbsoluteDeadline::within(SystemClocks, Duration::from_secs(30));
        let mut owner = reserve(&self.core, &self.bootstrap.scope)?;
        let original_watch = self.control.kernel_stop_authority().admit();
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&self.core.cell), "model.reserved");
        let mut record = "not_required";
        let result = (|| {
            self.startup_report().require_ready()?;
            let watch = original_watch.map_err(|_| SessionError::Restricted)?;
            let gate = RegistrationGate {
                core: &self.core,
                scope: &self.bootstrap.scope,
                primary: &self.evidence,
                watch: &watch,
                deadline: &deadline,
            };
            let endpoint = LoopbackEndpoint::parse(endpoint).map_err(model_error)?;
            let transport = VerifiedOllama::verify(endpoint, spec, &gate, &deadline, &watch)
                .map_err(model_error)?;
            let model = crate::inference::RegisteredModel::verified(transport);
            let (identity, view) = (model.identity, model.view.clone());
            hold(&self.core)?.child_complete = true;
            // The active owner prevents seal while append runs outside the model hold.
            record = "unknown";
            #[cfg(test)]
            crate::authority::file_session::probe(Some(&self.core.cell), "model.record.before");
            let sequence = self
                .control
                .authority()
                .record_session_model_registered(vec![
                    ("model_digest".into(), view.digest.clone()),
                    ("identity".into(), view.identity.clone()),
                ])
                .map_err(|_| SessionError::Backend("evidence_unavailable"))?;
            record = "recorded";
            #[cfg(test)]
            crate::authority::file_session::probe(Some(&self.core.cell), "model.install.before");
            let mut s = hold(&self.core)?;
            gate.admission(&s).map_err(model_error)?;
            s.registration = Some(RegistrationReceipt {
                sequence,
                digest: identity,
                model: Arc::new(model),
                watch: watch.clone(),
                primary: self.evidence.clone(),
            });
            #[cfg(test)]
            crate::authority::file_session::probe(Some(&self.core.cell), "model.install.after");
            #[cfg(test)]
            crate::authority::file_session::probe(Some(&self.core.cell), "model.completion.before");
            owner.complete_held(record, &mut s)?;
            Ok(view)
        })();
        let completion = if owner.done {
            Ok(())
        } else {
            owner.complete(record)
        };
        match result {
            Ok(view) => {
                completion?;
                Ok(view)
            }
            Err(error) => Err(error),
        }
    }
}
fn poisoned_grant(error: &AuthorityError, cell: &HaltCell) {
    use crate::authority::run::GRANT_POISONED;
    if matches!(error, AuthorityError::Unavailable(why) if *why == GRANT_POISONED) {
        cell.fail_session();
    }
}
pub(super) fn model_error(error: ModelError) -> SessionError {
    SessionError::Backend(crate::inference::model_class(error))
}

// SB-2b accounting only: no transport, C1 meter or session dispatch route.
use crate::authority::evidence::EndStatus;
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::{GroupId, OriginalGrant, RunRegistry, SessionAttempt};
use crate::authority::{Authority, AuthorityError};
use crate::inference::{CallMeter, Charged, InvocationObserver, MeterRefusal, ModelRequest};
pub(super) struct Attempt {
    owner: Owner,
    runs: Arc<RunRegistry>,
    ticket: Option<SessionAttempt>,
    record_started: bool,
    record: Option<EndStatus>,
    result: Option<Result<(), ModelError>>,
    model_digest: Option<String>,
}
impl Attempt {
    pub(super) fn acquire(
        core: &Arc<Core>,
        authority: &Authority,
        (run, agent, group, identity): (RunId, &AgentId, GroupId, [u8; 32]),
        original: &HaltCell,
        grant: &OriginalGrant<'_>,
        invocation: &InvocationObserver,
    ) -> Result<Self, AuthorityError> {
        let busy = |_| AuthorityError::Unavailable("model slot unavailable");
        let (mut owner, previous, model_digest) = {
            let _producers = core.producers.try_lock().map_err(|_| busy(()))?;
            let mut s = hold(core).map_err(|_| busy(()))?;
            if core.cell.session_word() & (SEALED | FAILED) != 0
                || s.active
                || core.model.uncertain.load(Ordering::SeqCst)
                || s.record
                    .is_some_and(|r| !matches!(r, "recorded" | "not_required"))
                || (s.attempted && s.registration.is_none())
                || s.registration
                    .as_ref()
                    .is_some_and(|r| r.digest != identity || r.watch.check_now().is_err())
            {
                return Err(AuthorityError::Halted);
            }
            let previous = (s.attempt, s.owner_complete, s.child_complete, s.record);
            let digest = s.registration.as_ref().map(|r| r.model.view.digest.clone());
            s.attempt = true;
            s.active = true;
            s.owner_complete = false;
            s.child_complete = false;
            s.record = Some("pending");
            (
                Owner {
                    core: core.clone(),
                    done: false,
                },
                previous,
                digest,
            )
        }; // Release producer and slot holds BEFORE the run table.
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "model.attempt.reserved");
        let ticket = match authority.runs().admit_session_attempt(
            (run, agent, group, identity),
            original,
            grant,
            invocation,
        ) {
            Ok(ticket) => ticket,
            Err(error) => {
                poisoned_grant(&error, original); // After the run table is released.
                #[cfg(test)]
                crate::authority::file_session::probe(Some(&core.cell), "model.attempt.refused");
                let mut s = completion_hold(core).map_err(|_| busy(()))?;
                (s.attempt, s.owner_complete, s.child_complete, s.record) = previous;
                s.active = false; // Conclusive pre-ticket refusal: no result owed.
                owner.done = true;
                return Err(error);
            }
        };
        Ok(Self {
            owner,
            runs: authority.runs().clone(),
            ticket: Some(ticket),
            record_started: false,
            record: None,
            result: None,
            model_digest,
        })
    }
    pub(super) fn record_return(
        &mut self,
        authority: &Authority,
        result: Result<(), ModelError>,
    ) -> Result<EndStatus, SessionError> {
        if self.record_started || !Arc::ptr_eq(&self.runs, authority.runs()) {
            return Err(SessionError::Restricted);
        }
        self.record_started = true; // A panic must not permit a second write.
        self.result = Some(result);
        let ticket = self.ticket.as_ref().ok_or(SessionError::Restricted)?;
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&self.owner.core.cell), "model.attempt.return");
        let mut detail = vec![
            ("op".into(), "model_call".into()),
            ("call".into(), ticket.ordinal.to_string()),
            ("door".into(), ticket.door.clone()),
            (
                "contract_hash".into(),
                ticket.admission.contract_hash.clone(),
            ),
            ("request".into(), ticket.admission.request.clone()),
            ("identity".into(), hex::encode(ticket.admission.identity)),
            (
                "outcome".into(),
                match self.result.as_ref().unwrap() {
                    Ok(()) => "ok",
                    Err(e) => e.code(),
                }
                .into(),
            ),
        ];
        if let Some(digest) = &self.model_digest {
            detail.push(("model_digest".into(), digest.clone()));
        }
        #[cfg(test)]
        crate::authority::file_session::probe(
            Some(&self.owner.core.cell),
            "model.attempt.record.before",
        );
        // No producer, model or run-table hold crosses the existing end writer.
        let record = authority.record_model_call(ticket.run, &ticket.agent, detail);
        self.record = Some(record);
        let mut s = completion_hold(&self.owner.core)?;
        s.child_complete = true;
        s.record = Some(record.as_str());
        Ok(record)
    }
    pub(super) fn complete(mut self) -> Result<(), SessionError> {
        if !matches!(self.record, Some(EndStatus::Recorded { .. })) {
            return Err(SessionError::Unavailable);
        }
        // Keep the slot active across exact pin return. Seal/reuse still refuse.
        self.runs
            .settle_session_attempt(
                self.ticket.take().ok_or(SessionError::Restricted)?,
                completion_deadline(&self.owner.core),
            )
            .map_err(|_| SessionError::Unavailable)?;
        self.owner.complete("recorded")
    }
}
// Report-only classification; missing durable association can never imply success.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "SB-2b: private accounting, wired by SB-3; session route unavailable"
    )
)]
pub(super) fn historical_outcome(end: Option<EndStatus>) -> &'static str {
    match end {
        Some(end) => end.as_str(),
        None => "unknown",
    }
}

// SB-3A R2: the private original-door inference binding; no public route.
impl SessionDoor {
    pub(super) fn bind_inference_private(
        &self,
        run: RunId,
        contract_hash: &str,
        request: &str,
        expected_identity: [u8; 32],
    ) -> Result<(), crate::authority::AuthorityError> {
        use crate::authority::AuthorityError as E;
        let DoorKind::Original { file_grant } = self.kind else {
            return Err(E::Halted); // A continuation never binds.
        };
        if !self.context.scope.live(self.core.cell.session_word()) {
            return Err(E::Halted);
        }
        // Installed M, observed without waiting; released before the run table.
        let identity = {
            let s = hold(&self.core).map_err(|_| E::Unavailable("model slot unavailable"))?;
            let r = s.registration.as_ref().ok_or(E::ModelNotRegistered)?;
            if self.core.model.uncertain.load(Ordering::SeqCst) || r.watch.check_now().is_err() {
                return Err(E::Halted);
            }
            r.digest
        };
        if identity != expected_identity {
            return Err(E::ModelIdentityMismatch);
        }
        let backend = self.door.session_backend();
        let q_run = match self.context.scope.mother_run.get() {
            Some((control, known)) if control.ptr_eq(&backend) => Some(*known),
            Some(_) => return Err(E::NotMember),
            None => None,
        };
        let control = backend.upgrade().ok_or(E::Halted)?;
        let authority = control.authority();
        let clock = (
            authority.clock().monotonic_ms(),
            authority.clock().wall_ms(),
        );
        authority
            .runs()
            .admit_session_inference(crate::authority::run::SessionBinding {
                run,
                agent: self.door.agent(),
                group: self.door.origin(),
                contract_hash,
                request,
                identity,
                original: &self.context.cell,
                q_run,
                grants: authority.grants(),
                grant: file_grant,
                clock,
            })
            .inspect_err(|e| poisoned_grant(e, &self.context.cell))
    }
}

// SB-3C R1: an original door's read-only view of the installed model.
impl SessionDoor {
    pub(super) fn model_identity_private(&self) -> Option<ModelIdentityView> {
        let DoorKind::Original { .. } = self.kind else {
            return None; // A continuation sees no model.
        };
        if !self.context.scope.live(self.core.cell.session_word()) {
            return None;
        }
        // Installed M, observed without waiting (poison latches, as elsewhere).
        let s = hold(&self.core).ok()?;
        let r = s.registration.as_ref()?;
        if self.core.model.uncertain.load(Ordering::SeqCst) || r.watch.check_now().is_err() {
            return None;
        }
        Some(r.model.view.clone())
    }
}

// SB-3B1a R3: the private original-door attempt admission; no public route.
impl SessionDoor {
    pub(super) fn admit_attempt_private(
        &self,
        run: RunId,
        request: &ModelRequest,
        invocation: &InvocationObserver,
    ) -> Result<Attempt, AuthorityError> {
        let DoorKind::Original { file_grant } = self.kind else {
            return Err(AuthorityError::Halted); // A continuation never calls.
        };
        if request.is_conversation() {
            return Err(AuthorityError::ModelCall(ModelError::ConfigMismatch));
        }
        if !self.context.scope.live(self.core.cell.session_word()) || invocation.is_revoked() {
            return Err(AuthorityError::Halted);
        }
        // Installed M, observed without waiting; released before the run table.
        let identity = {
            let s = hold(&self.core).map_err(|_| AuthorityError::Unavailable("model slot"))?;
            let r = s
                .registration
                .as_ref()
                .ok_or(AuthorityError::ModelNotRegistered)?;
            if self.core.model.uncertain.load(Ordering::SeqCst) || r.watch.check_now().is_err() {
                return Err(AuthorityError::Halted);
            }
            r.digest
        };
        let backend = self.door.session_backend();
        let q = self.context.scope.mother_run.get();
        if !q.is_some_and(|(control, known)| control.ptr_eq(&backend) && *known == run) {
            return Err(AuthorityError::NotMember); // Q's own File run only.
        }
        let control = backend.upgrade().ok_or(AuthorityError::Halted)?;
        let authority = control.authority();
        let clock = authority.clock();
        let grant = OriginalGrant {
            grants: authority.grants(),
            grant: file_grant,
            clock: (clock.monotonic_ms(), clock.wall_ms()),
        };
        let key = (run, self.door.agent(), self.door.origin(), identity);
        Attempt::acquire(
            &self.core,
            authority,
            key,
            &self.context.cell,
            &grant,
            invocation,
        )
    }
}

// SB-3B1b R5: the private original-door call; no public route.
/// Test seam: the backstop's clocks, skewed by `backstop_skew` ms.
#[cfg(test)]
struct SkewClock(Arc<std::sync::atomic::AtomicU64>);
#[cfg(test)]
impl crate::inference::DeadlineClock for SkewClock {
    fn wall_ms(&self) -> u64 {
        SystemClocks.wall_ms() + self.0.load(Ordering::SeqCst)
    }
    fn monotonic_ms(&self) -> u64 {
        SystemClocks.monotonic_ms() + self.0.load(Ordering::SeqCst)
    }
}
type Refusal = (&'static str, ModelError);
/// R5 override (sheet §7.2), the C1 meter noting the charge, the deadline.
struct CallGate<'a> {
    door: &'a SessionDoor,
    authority: &'a Authority,
    ticket: &'a SessionAttempt,
    grant: crate::authority::ids::GrantId,
    watch: &'a StopWatch,
    primary: &'a crate::evidence_store::PrimaryEvidence,
    backstop: &'a dyn CallDeadline,
    deadline: &'a dyn CallDeadline,
    meter: &'a dyn CallMeter,
    charged: AtomicBool,
    spent: AtomicBool,
    invocation: &'a InvocationObserver,
}
impl CallMeter for CallGate<'_> {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let charged = self.meter.charge()?;
        self.charged.store(true, Ordering::SeqCst);
        Ok(charged)
    }
    fn settle(&self, call: u32) {
        self.meter.settle(call);
    }
}
impl CallDeadline for CallGate<'_> {
    fn expired(&self) -> bool {
        // Scope first, then always the caller: a scope expiry returned here
        // has its observation in the caller's latch.
        let scope = self.lapsed(self.sample());
        self.deadline.expired() || scope
    }
}
impl CallGate<'_> {
    fn sample(&self) -> crate::host_session::Sample {
        let limits = self.door.context.scope.mission_deadlines.get();
        let clocks = limits.map(|l| (l.clock.monotonic_ms(), l.clock.wall_ms()));
        (Instant::now(), clocks)
    }
    fn lapsed(&self, at: crate::host_session::Sample) -> bool {
        self.backstop.expired() || self.door.context.scope.expired_at(at)
    }
    fn admission(&self, kind: RequestKind) -> Result<(), ModelError> {
        let result = self.predicates(kind);
        #[cfg(test)]
        self.b1b_note(
            result
                .as_ref()
                .err()
                .map_or("admitted", |refusal| refusal.0),
        );
        result.map_err(|refusal| refusal.1)
    }
    fn predicates(&self, kind: RequestKind) -> Result<(), Refusal> {
        use {
            AuthorityError as E,
            ModelError::{Cancelled, Stopped, Timeout},
        };
        let (core, scope) = (&self.door.core, &self.door.context.scope);
        // Every clock this gate uses is read before any hold (SB3B-R5 (3)).
        let clock = self.authority.clock();
        let grant = OriginalGrant {
            grants: self.authority.grants(),
            grant: self.grant,
            clock: (clock.monotonic_ms(), clock.wall_ms()),
        };
        let at = self.sample();
        let expired = self.lapsed(at);
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "model.call.sampled");
        let runs = self.authority.runs();
        let hold = runs.try_request_hold().map_err(|poisoned| {
            if poisoned {
                core.model.uncertain.store(true, Ordering::SeqCst);
                self.door.context.fail();
            }
            ("refused:hold", Cancelled) // Benign contention latches nothing.
        })?;
        if expired {
            return Err(("refused:1", Timeout));
        }
        // SB3B-R18/R20: one reading of Q and the mission limits against the
        // pre-hold sample; a lapse it finds (narrowing included) is expiry.
        let judged = scope.judge(Some(at));
        if judged == Err(crate::host_session::Lapse::Expired) {
            return Err(("refused:1", Timeout));
        }
        let word = core.cell.session_word();
        if core.model.uncertain.load(Ordering::SeqCst) || word != scope.snapshot || judged.is_err()
        {
            return Err(("refused:1", Cancelled));
        }
        self.watch.check_now().map_err(|_| ("refused:2", Stopped))?;
        if !self
            .primary
            .try_health()
            .is_some_and(|h| !h.latched && h.owed == 0 && !h.recovery_required && h.unsettled == 0)
        {
            return Err(("refused:3", Cancelled));
        }
        if self.ticket.ended() {
            return Err(("refused:4", Cancelled));
        }
        let _held = match hold.live(self.ticket, &grant) {
            Ok(held) => held,
            Err(error) => {
                drop(hold); // Then fail: no clock is read under a hold.
                let poisoned = crate::authority::run::GRANT_POISONED;
                if matches!(error, AuthorityError::Unavailable(w) if w == poisoned) {
                    core.model.uncertain.store(true, Ordering::SeqCst);
                    self.door.context.fail();
                }
                let grant = matches!(error, E::GrantNotLive | E::Unavailable(_));
                return Err((if grant { "refused:5" } else { "refused:4" }, Cancelled));
            }
        };
        let unspent = self.charged.load(Ordering::SeqCst) && !self.spent.load(Ordering::SeqCst);
        if kind == RequestKind::Chat && !unspent {
            return Err(("refused:6", Cancelled));
        }
        if self.invocation.is_revoked() {
            return Err(("refused:7", Cancelled));
        }
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "model.call.observed");
        if let Err((error, lapse)) = scope.admit_judged(&self.door.context.cell, Some(at)) {
            drop((_held, hold)); // An exhausted counter fails the session after release.
            if matches!(error, E::Capacity) {
                self.door.context.fail();
                return Err(("refused:8", Cancelled));
            }
            // SB3B-R18/R20: the CAS's own eligibility reading found expiry.
            let expired = lapse == Some(crate::host_session::Lapse::Expired);
            return Err(("refused:8", if expired { Timeout } else { Cancelled }));
        }
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "model.call.admitted");
        if kind == RequestKind::Chat {
            self.spent.store(true, Ordering::SeqCst);
        }
        Ok(()) // Drops the grant guard, then the run table.
    }
}
impl CallCancel for CallGate<'_> {
    fn cancelled(&self) -> Option<CancelCause> {
        // Deny-only polling; deliberately does not acquire the owning hold.
        let core = &self.door.core;
        #[cfg(test)]
        crate::authority::file_session::probe(Some(&core.cell), "model.call.polled");
        let (at, scope) = (self.sample(), &self.door.context.scope);
        let cancelled = (!self.lapsed(at)
            && (core.model.uncertain.load(Ordering::SeqCst)
                || core.cell.session_word() != scope.snapshot
                || !scope.eligible(Some(at))
                || self.ticket.ended()))
        .then_some(CancelCause::Cancelled);
        #[cfg(test)]
        self.b1b_note(["poll:none", "poll:cancelled"][usize::from(cancelled.is_some())]);
        cancelled
    }
    fn admit_request(&self, kind: RequestKind) -> Result<(), ModelError> {
        #[cfg(test)]
        self.b1b_gate_entry(kind);
        let admitted = self.admission(kind);
        #[cfg(test)]
        self.b1b_gate_exit(kind, admitted.is_ok());
        admitted
    }
}
/// Test seams around each call gate: the gate trace and the chat-latch probes.
#[cfg(test)]
impl CallGate<'_> {
    fn b1b_note(&self, at: &'static str) {
        self.door.core.model.trace.lock().unwrap().push(at);
    }
    /// Test seam: an admission whose chat latch is uncharged or spent.
    fn b1b_latch_probe(&self, seam: &'static str) {
        self.b1b_note(seam);
        let class = match self.admission(RequestKind::Chat) {
            Ok(()) => "probe:admitted",
            Err(ModelError::Cancelled) => "probe:model_cancelled",
            Err(_) => "probe:other",
        };
        self.b1b_note(class);
    }
    fn b1b_gate_entry(&self, kind: RequestKind) {
        let core = &self.door.core;
        crate::authority::file_session::probe(Some(&core.cell), "model.call.gate");
        let chat = kind == RequestKind::Chat;
        self.b1b_note(if chat { "gate:chat" } else { "gate:metadata" });
        if !chat && core.model.chat_early.swap(false, Ordering::SeqCst) {
            self.b1b_latch_probe("early");
        }
    }
    fn b1b_gate_exit(&self, kind: RequestKind, admitted: bool) {
        let core = &self.door.core;
        let chat = admitted && kind == RequestKind::Chat;
        if chat && core.model.chat_replay.swap(false, Ordering::SeqCst) {
            crate::authority::file_session::probe(Some(&core.cell), "model.call.replay");
            self.b1b_latch_probe("replay");
        }
    }
}
impl SessionDoor {
    pub(super) fn model_call_private(
        &self,
        run: RunId,
        request: &ModelRequest,
        meter: &dyn CallMeter,
        deadline: &dyn CallDeadline,
        invocation: &InvocationObserver,
    ) -> Result<crate::inference::ModelCallOutcome, AuthorityError> {
        let DoorKind::Original { file_grant } = self.kind else {
            return Err(AuthorityError::Halted); // A continuation never calls.
        };
        let installed = {
            let s = hold(&self.core).map_err(|_| AuthorityError::Unavailable("model slot"))?;
            let r = s.registration.as_ref();
            r.map(|r| (r.model.clone(), r.watch.clone(), r.primary.clone()))
        };
        let control = self.door.session_backend().upgrade();
        let mut attempt = self.admit_attempt_private(run, request, invocation)?;
        let (Some((model, watch, primary)), Some(control)) = (installed, control) else {
            return Err(AuthorityError::Halted); // Dropped ticket: debt latches.
        };
        let authority = control.authority();
        #[cfg(test)]
        let clock = SkewClock(self.core.model.backstop_skew.clone());
        #[cfg(not(test))]
        let clock = SystemClocks;
        // One 180 s backstop per attempt, fixed once; it only shortens.
        let backstop = AbsoluteDeadline::within(clock, nexus_kernel::inference::CALL_HARD_TIMEOUT);
        let result = attempt
            .ticket
            .as_ref()
            .map_or(Err(ModelError::Cancelled), |ticket| {
                let gate = CallGate {
                    door: self,
                    authority,
                    ticket,
                    grant: file_grant,
                    watch: &watch,
                    primary: &primary,
                    backstop: &backstop,
                    deadline,
                    meter,
                    charged: AtomicBool::new(false),
                    spent: AtomicBool::new(false),
                    invocation,
                };
                crate::inference::session_transport(&model, request, (&gate, &gate, &gate), &watch)
            });
        // One native writer; no hold crosses it. Debt is never upgraded.
        let returned = result.as_ref().map(|_| ()).map_err(|e| *e);
        let record = match attempt.record_return(authority, returned) {
            Ok(EndStatus::Recorded { seq }) => match attempt.complete() {
                Ok(()) => EndStatus::Recorded { seq },
                Err(_) => EndStatus::Uncertain,
            },
            Ok(owed) => owed,
            Err(_) => EndStatus::Uncertain,
        };
        let result = result.map_err(AuthorityError::ModelCall);
        Ok(crate::inference::session_outcome(result, record))
    }
}
