//! Observation only. These facts never authorize a call or discharge resources.
use super::*;
use std::sync::atomic::AtomicU8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    NotEntered,
    Pending,
    Completed,
    Refused,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantClosure {
    NotIssued,
    Unclosed,
    Closed,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreObservation {
    pub request: u64,
    pub target: Option<u64>,
    pub handed_off: bool,
    pub outcome: RestoreOutcome,
    pub grant: GrantClosure,
    pub interrupted: bool,
}
#[derive(Debug, Default)]
pub(super) struct RestoreFacts {
    outcome: AtomicU8,
    recording: AtomicU8,
    grant: AtomicU8,
}
impl RestoreFacts {
    pub(crate) fn entered(&self) {
        self.recording.store(1, Ordering::SeqCst);
        self.outcome.store(1, Ordering::SeqCst);
    }
    fn recording(&self) -> RecordingStatus {
        match self.recording.load(Ordering::SeqCst) {
            0 => RecordingStatus::NotStarted,
            2 => RecordingStatus::Recorded,
            3 => RecordingStatus::Owed,
            _ => RecordingStatus::Unknown,
        }
    }
    pub(crate) fn returned(&self, result: RestoreOutcome, recording: RecordingStatus) {
        self.recording.store(
            match recording {
                RecordingStatus::NotStarted => 0,
                RecordingStatus::Recorded => 2,
                RecordingStatus::Owed => 3,
                RecordingStatus::Unknown => 1,
            },
            Ordering::SeqCst,
        );
        self.outcome.store(
            match result {
                RestoreOutcome::Completed => 2,
                RestoreOutcome::Refused => 3,
                _ => 4,
            },
            Ordering::SeqCst,
        );
    }
    pub(crate) fn grant(&self, state: GrantClosure) {
        self.grant.store(
            match state {
                GrantClosure::NotIssued => 0,
                GrantClosure::Unclosed => 1,
                GrantClosure::Closed => 2,
                GrantClosure::Unknown => 3,
            },
            Ordering::SeqCst,
        );
    }
}
impl SessionRequest {
    pub fn restore_observation(&self) -> RestoreObservation {
        let outcome = match self.scope.restore.outcome.load(Ordering::SeqCst) {
            0 => RestoreOutcome::NotEntered,
            1 => RestoreOutcome::Pending,
            2 => RestoreOutcome::Completed,
            3 => RestoreOutcome::Refused,
            _ => RestoreOutcome::Uncertain,
        };
        RestoreObservation {
            request: self.sequence,
            target: self.context(),
            handed_off: outcome != RestoreOutcome::NotEntered,
            outcome,
            grant: match self.scope.restore.grant.load(Ordering::SeqCst) {
                0 => GrantClosure::NotIssued,
                1 => GrantClosure::Unclosed,
                2 => GrantClosure::Closed,
                _ => GrantClosure::Unknown,
            },
            interrupted: self.core.cell.session_word() / EPOCH_STEP != self.generation(),
        }
    }
}
impl RequestScope {
    pub(crate) fn restore_entered(&self) {
        self.restore.entered();
    }
    pub(crate) fn restore_returned(&self, outcome: RestoreOutcome, recording: RecordingStatus) {
        self.restore.returned(outcome, recording);
    }
    pub(crate) fn restore_grant(&self, state: GrantClosure) {
        self.restore.grant(state);
    }
}

pub(super) struct UndoClaim {
    core: Arc<Core>,
    returned: bool,
}
impl UndoClaim {
    pub(super) fn take(core: &Arc<Core>, target: u64) -> Result<Self, SessionFailure> {
        core.undo_context
            .compare_exchange(target, 0, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| crate::authority::AuthorityError::Halted)?;
        Ok(Self {
            core: core.clone(),
            returned: false,
        })
    }
    pub(super) fn returned(mut self) {
        self.returned = true;
    }
}
impl Drop for UndoClaim {
    fn drop(&mut self) {
        if !self.returned {
            SessionIngress {
                core: self.core.clone(),
            }
            .fail();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingStatus {
    NotStarted,
    Recorded,
    Owed,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acknowledgement {
    Pending,
    Verified,
}
#[derive(Clone, Debug)]
pub struct ActionObservation {
    pub target: u64,
    pub run: crate::authority::ids::RunId,
    pub mission: Option<u64>,
    pub effect: Option<u64>,
    pub commitment: Option<crate::authority::ids::CommitmentId>,
    pub publication: crate::authority::commitment::PublicationClass,
    pub mother: RecordingStatus,
    pub mother_sequence: Option<u64>,
    pub mother_hash: Option<[u8; 32]>,
    pub acknowledgement: Acknowledgement,
}
/// Read-only facts, never a permit or a cleanup/acknowledgement capability.
/// Missing observations stay unknown. Resources describe the complete session
/// inventory; action rows are selected by the immutable request/target only.
#[derive(Clone, Debug)]
pub struct OperationObservation {
    pub session: SessionIdentity,
    pub request: u64,
    pub target: Option<u64>,
    pub admitted_steps: u64,
    pub calls: Vec<CallObservation>,
    pub primary_health: crate::evidence_store::EvidenceHealth,
    pub apply_handed_off: bool,
    pub restore: RestoreObservation,
    pub ingresses: u64,
    pub first_restriction: Option<(crate::authority::file_session::HaltCause, Option<u16>)>,
    pub kernel: RecordingStatus,
    pub primary: RecordingStatus,
    pub primary_reference: Option<crate::authority::evidence::ExecutionEvidence>,
    pub actions: Vec<ActionObservation>,
    pub inventory: Option<crate::files::SessionInventory>,
    pub resources: Option<ResourceObligations>,
}
#[derive(Debug, Default)]
pub(super) struct RequestFacts {
    pub(super) admitted: AtomicU64,
    apply_entered: std::sync::atomic::AtomicBool,
    apply_completed: std::sync::atomic::AtomicBool,
    calls: [CallCounters; 21],
    pub(super) primary: OnceLock<Option<crate::authority::evidence::ExecutionEvidence>>,
}
impl RequestScope {
    pub(crate) fn apply_entered(&self) {
        self.facts.apply_entered.store(true, Ordering::SeqCst);
    }
    pub(crate) fn apply_returned(&self, completed: bool) {
        // Native apply Ok includes its required durable completion record.
        // Errors may still have published: they never prove absent/recorded.
        self.facts
            .apply_completed
            .store(completed, Ordering::SeqCst);
    }
}
impl HostSession {
    pub fn operation_observation(
        &self,
        request: &SessionRequest,
    ) -> Result<OperationObservation, SessionError> {
        if !Arc::ptr_eq(&self.core, &request.core) {
            return Err(SessionError::Restricted);
        }
        let retained = self
            .core
            .actions
            .lock()
            .map_err(|_| {
                self.ingress().fail();
                SessionError::Restricted
            })?
            .iter()
            .filter(|action| {
                if request.kind == CommandKind::Approve {
                    action.preparation == request.target
                } else {
                    action.request == request.sequence
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        let actions = retained
            .iter()
            .map(|action| {
                let publication = action.commitment.and_then(|id| {
                    self.control
                        .authority()
                        .commitments()
                        .publication(id)
                        .ok()
                        .flatten()
                        .filter(|(view, _)| view.run == action.run && view.id == id)
                        .map(|(_, publication)| publication)
                });
                action.observation(publication)
            })
            .collect();
        let restore = request.restore_observation();
        let apply_handed_off = request.scope.facts.apply_entered.load(Ordering::SeqCst);
        let primary_reference = request.scope.facts.primary.get().copied().flatten();
        use crate::authority::evidence::EndStatus;
        let primary = match primary_reference.map(|e| e.end) {
            Some(EndStatus::Recorded { .. }) => RecordingStatus::Recorded,
            Some(EndStatus::Owed) => RecordingStatus::Owed,
            Some(EndStatus::Uncertain) => RecordingStatus::Unknown,
            None if request.scope.facts.admitted.load(Ordering::SeqCst) == 0 => {
                RecordingStatus::NotStarted
            }
            None => RecordingStatus::Unknown,
        };
        let inventory = self.inventory().ok();
        let resources = self.resource_obligations().ok();
        if inventory.is_none() || resources.is_none() {
            self.ingress().fail();
        }
        Ok(OperationObservation {
            session: self.identity(),
            request: request.sequence,
            target: request.context(),
            admitted_steps: request.scope.facts.admitted.load(Ordering::SeqCst),
            calls: request
                .scope
                .facts
                .calls
                .iter()
                .enumerate()
                .filter_map(|(index, counters)| {
                    let entered = counters.entered.load(Ordering::SeqCst);
                    (entered > 0).then(|| CallObservation {
                        call: CALL_NAMES[index],
                        entered,
                        returned: counters.returned.load(Ordering::SeqCst),
                        failed: counters.failed.load(Ordering::SeqCst),
                    })
                })
                .collect(),
            primary_health: self.evidence.health(),
            apply_handed_off,
            restore,
            ingresses: self.core.cell.session_word() / EPOCH_STEP - request.generation(),
            first_restriction: self
                .core
                .cell
                .reason()
                .map(|(cause, reason)| (cause, reason.map(|r| r.get()))),
            kernel: if restore.handed_off {
                request.scope.restore.recording()
            } else if request.scope.facts.apply_completed.load(Ordering::SeqCst) {
                RecordingStatus::Recorded
            } else if request.scope.facts.admitted.load(Ordering::SeqCst) != 0 {
                RecordingStatus::Unknown
            } else {
                RecordingStatus::NotStarted
            },
            primary,
            primary_reference,
            actions,
            inventory,
            resources,
        })
    }

    /// The exact retained native action selects its take-once Owner detail.
    /// No raw broker, caller-selected run, or legacy conversion is exposed.
    pub fn file_working_detail(
        &self,
        request: &SessionRequest,
        id: crate::authority::ids::CommitmentId,
    ) -> Result<Option<crate::files::WorkingDetail>, SessionError> {
        if !Arc::ptr_eq(&self.core, &request.core)
            || request.kind != CommandKind::Detail
            || self.state() != SessionState::Normal
        {
            return Err(SessionError::Restricted);
        }
        let actions = self
            .core
            .actions
            .lock()
            .map_err(|_| SessionError::Restricted)?;
        if !actions
            .iter()
            .any(|a| a.session == self.identity() && a.commitment == Some(id))
        {
            return Err(SessionError::NoTarget);
        }
        Ok(self.control.files_working_detail(id))
    }
}

/// Closed native call vocabulary. Memory observations grant nothing and do not
/// turn a returned error or a missing return into durable completion.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeCall {
    ProjectSelect,
    ReadGrant,
    RunConstruct,
    StagingGrant,
    Snapshot,
    Edit,
    StructuralVerify,
    PageVerify,
    Review,
    RevalidationReview,
    RequestApproval,
    PageRecheck,
    WriteGrant,
    Apply,
    RequestRestore,
    UndoGrant,
    Restore,
    RetryCleanup,
    DiscardStaging,
    Cancel,
    FinalizationSync,
}
const CALL_NAMES: [&str; 21] = [
    "project_select",
    "read_grant",
    "run_construct",
    "staging_grant",
    "snapshot",
    "edit",
    "structural_verify",
    "page_verify",
    "review",
    "revalidation_review",
    "request_approval",
    "page_recheck",
    "write_grant",
    "apply",
    "request_restore",
    "undo_grant",
    "restore",
    "retry_cleanup",
    "discard_staging",
    "cancel",
    "finalization_sync",
];
impl NativeCall {
    pub(crate) fn from_event(event: &str) -> Option<Self> {
        match event {
            "session.entered.project_select" => Some(Self::ProjectSelect),
            "session.entered.read_grant" => Some(Self::ReadGrant),
            "session.entered.run_construct" => Some(Self::RunConstruct),
            "session.entered.staging_grant" => Some(Self::StagingGrant),
            "session.entered.snapshot" => Some(Self::Snapshot),
            "session.entered.edit" => Some(Self::Edit),
            "session.entered.structural_verify" => Some(Self::StructuralVerify),
            "session.entered.page_verify" => Some(Self::PageVerify),
            "session.entered.review" => Some(Self::Review),
            "session.entered.revalidation_review" => Some(Self::RevalidationReview),
            "session.entered.request_approval" => Some(Self::RequestApproval),
            "session.entered.page_recheck" => Some(Self::PageRecheck),
            "session.entered.write_grant" => Some(Self::WriteGrant),
            "session.entered.apply" => Some(Self::Apply),
            "session.entered.request_restore" => Some(Self::RequestRestore),
            "session.entered.undo_grant" => Some(Self::UndoGrant),
            "session.entered.restore" => Some(Self::Restore),
            "session.entered.retry_cleanup" => Some(Self::RetryCleanup),
            "session.entered.discard_staging" => Some(Self::DiscardStaging),
            "session.entered.cancel" => Some(Self::Cancel),
            "session.entered.finalization_sync" => Some(Self::FinalizationSync),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CallObservation {
    pub call: &'static str,
    pub entered: u64,
    pub returned: u64,
    pub failed: u64,
}
#[derive(Debug, Default)]
struct CallCounters {
    entered: AtomicU64,
    returned: AtomicU64,
    failed: AtomicU64,
}
impl RequestScope {
    pub(crate) fn native_entered(&self, call: NativeCall) {
        self.facts.calls[call as usize]
            .entered
            .fetch_add(1, Ordering::SeqCst);
    }
    pub(crate) fn native_returned(&self, call: NativeCall, failed: bool) {
        let fact = &self.facts.calls[call as usize];
        if failed {
            fact.failed.fetch_add(1, Ordering::SeqCst);
        }
        fact.returned.fetch_add(1, Ordering::SeqCst);
    }
}
