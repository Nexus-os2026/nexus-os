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
}
impl ModelSlot {
    #[cfg(test)]
    pub(super) fn installed(&self) -> bool {
        self.state.lock().unwrap().registration.is_some()
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
impl Owner {
    pub(super) fn complete(mut self, record: &'static str) -> Result<(), SessionError> {
        let core = self.core.clone();
        // Retry only completion metadata, bounded by the original close limit.
        // Admission/handoff holds remain nonblocking and never use this loop.
        let deadline = SessionIngress { core: core.clone() }
            .close_deadline()
            .map_or(Instant::now() + Duration::from_millis(100), |d| {
                d.min(Instant::now() + Duration::from_millis(100))
            });
        let mut s = loop {
            match core.model.state.try_lock() {
                Ok(state) => break state,
                Err(TryLockError::Poisoned(_)) => return Err(SessionError::Unavailable),
                Err(TryLockError::WouldBlock) => {
                    #[cfg(test)]
                    crate::authority::file_session::probe(
                        Some(&core.cell),
                        "model.complete.contended",
                    );
                    if Instant::now() >= deadline {
                        return Err(SessionError::Unavailable);
                    }
                    std::thread::park_timeout(
                        Duration::from_millis(1)
                            .min(deadline.saturating_duration_since(Instant::now())),
                    );
                }
            }
        };
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "SB-2a: private, wired by SB-3; session route unavailable"
        )
    )]
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
fn model_error(error: ModelError) -> SessionError {
    SessionError::Backend(crate::inference::model_class(error))
}

// SB-2b accounting only: no transport, C1 meter or session dispatch route.
use crate::authority::evidence::EndStatus;
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::{GroupId, RunRegistry, SessionAttempt};
use crate::authority::Authority;
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "SB-2b: private accounting, wired by SB-3; session route unavailable"
        )
    )]
    pub(super) fn acquire(
        core: &Arc<Core>,
        authority: &Authority,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        identity: [u8; 32],
    ) -> Result<Self, SessionError> {
        let (mut owner, previous, model_digest) = {
            let _producers = core
                .producers
                .try_lock()
                .map_err(|_| SessionError::Unavailable)?;
            let mut s = hold(core)?;
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
                return Err(SessionError::Restricted);
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
        let ticket = match authority
            .runs()
            .admit_session_attempt(run, agent, group, identity, &core.cell)
        {
            Ok(ticket) => ticket,
            Err(error) => {
                #[cfg(test)]
                crate::authority::file_session::probe(Some(&core.cell), "model.attempt.refused");
                let mut s = hold(core)?;
                (s.attempt, s.owner_complete, s.child_complete, s.record) = previous;
                s.active = false; // Conclusive pre-ticket refusal: no result owed.
                owner.done = true;
                return Err(match error {
                    crate::authority::AuthorityError::ModelCall(e) => model_error(e),
                    _ => SessionError::Restricted,
                });
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "SB-2b: private accounting, wired by SB-3; session route unavailable"
        )
    )]
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
        hold(&self.owner.core)?.child_complete = true;
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
        hold(&self.owner.core)?.record = Some(record.as_str());
        Ok(record)
    }
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "SB-2b: private accounting, wired by SB-3; session route unavailable"
        )
    )]
    pub(super) fn complete(mut self) -> Result<(), SessionError> {
        if !matches!(self.record, Some(EndStatus::Recorded { .. })) {
            return Err(SessionError::Unavailable);
        }
        // Keep the slot active across exact pin return. Seal/reuse still refuse.
        self.runs
            .settle_session_attempt(self.ticket.take().ok_or(SessionError::Restricted)?)
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
