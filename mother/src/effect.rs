//! The EffectPort: Mother's only way to the world.
//!
//! Every file, network, process, model-tool or spend effect a lane asks for
//! is an [`EffectRequest`] (data, never authority). The coordinator checks
//! it against the mission's state, consent, forbidden effects and budget,
//! journals it, and only then hands it to the port. The trait is sealed:
//! outside this crate only the adapter over the governed-control broker
//! ([`crate::adapter::GovernedPort`]) is a port.
//!
//! M2c: the port holds only a broker-issued agent door. Through it the Owner
//! is asked, natively and through the confirmer the host bound to the
//! door, to consent to a mission and to resume a held one; the port reports
//! what became of an action that waits for the Owner's native approval.
//! Mother never approves an action herself: the Owner does, at the broker.
//! Nothing reachable from the port leads to the broker's front door or its
//! authority.

use crate::contract::EffectKind;
use crate::evidence::BrokerClass;
use crate::journal::MissionId;
use nexus_governed_control::authority::approval::{
    MissionApproval, MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
};
use nexus_governed_control::authority::commitment::PublicationClass;
use nexus_governed_control::authority::file_session::{FileAttachment, FileSessionError, HaltCell};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::RecordBatch;
use nexus_governed_control::inference::{
    CallDeadline, CallMeter, InvocationObserver, ModelIdentityView, ModelReply, ModelRequest,
};
use nexus_governed_control::tool::ToolIntent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// An effect a lane asks for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum EffectRequest {
    /// A path relative to the requested folder: a request, never a path.
    File { relative_path: String, write: bool },
    /// Explicit lite File create. Legacy File requests retain their refusal.
    CreateFile(nexus_governed_control::files::FileCreate),
    /// A network request (the broker's egress route).
    Network(EgressIntent),
    /// A governed process (the broker's tool route).
    Process(ToolIntent),
    /// An API tool a model asks for (the broker's connector route).
    ModelTool(ConnectorIntent),
    /// Spending money. M1's money budget is zero, so it never leaves Mother.
    Spend { amount_minor: u64, purpose: String },
}

impl EffectRequest {
    pub fn kind(&self) -> EffectKind {
        match self {
            EffectRequest::File { .. } | EffectRequest::CreateFile(_) => EffectKind::File,
            EffectRequest::Network(_) => EffectKind::Network,
            EffectRequest::Process(_) => EffectKind::Process,
            EffectRequest::ModelTool(_) => EffectKind::ModelTool,
            EffectRequest::Spend { .. } => EffectKind::Spend,
        }
    }
}

/// Exact consent binding for this process, never reconstructed from replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub contract_hash: String,
    pub consent_request: String,
    pub folder: String,
}

/// One broker read: publication proof and the corresponding display state.
/// M3-Db: the broker's receipt of this exact commitment, if it published.
pub struct FileSnapshot {
    pub action: PendingAction,
    pub publication: PublicationClass,
    pub state: Option<nexus_governed_control::authority::commitment::CommitmentState>,
    pub receipt: Option<nexus_governed_control::files::FileReceipt>,
}

/// What a performed effect came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectOutcome {
    /// The effect ran; its bounded text output, if any.
    Done { text: Option<String> },
    /// The broker committed it; the Owner approves or denies it natively.
    AwaitingApproval,
}

/// What became of the action a mission waits on, as the broker says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// The Owner has not answered yet (or the approved action is still
    /// running at the broker).
    Waiting,
    /// The Owner approved it and it ran, once, at the broker.
    Approved,
    /// The Owner denied it: it never ran.
    Denied,
    /// Its approval expired: it never ran.
    Expired,
    /// It ended otherwise (revoked, failed, cancelled, or its outcome is
    /// unknown): only a stable class name.
    Lapsed { class: String },
}

/// The identity of an action a mission waits on at the broker: opaque
/// names of its run and commitment (never authority: the port checks them
/// against the broker's own records before using them), and the port's
/// monotonic deadline for it, never later than the broker's. Mother
/// journals it so that a rebuilt port can still withdraw the action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingAction {
    pub run: String,
    pub commitment: String,
    pub deadline_mono_ms: u64,
}

/// M2e-C1b: an attempt's record was not confirmed (owed, or unknown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordDebt {
    Owed,
    Uncertain,
}

/// M2e-C1b (A5): one attempted model call. Its record is read first: only a
/// recorded attempt releases its reply (untrusted data).
#[derive(Debug)]
pub enum ModelAttempt<R = ModelReply> {
    /// Recorded, with its reply.
    Replied(R),
    /// Recorded; the call ended without a reply (its class).
    Failed(BrokerClass),
    /// Not confirmed recorded: any reply was discarded.
    Unrecorded(RecordDebt),
}

/// The port or the broker refused: only a class of the closed registry
/// (P0: a class outside it is [`BrokerClass::Unknown`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectRefusal {
    pub class: BrokerClass,
}

/// M2c PR-B: the broker records a call still owes (a door halt's or a
/// cancellation's, made in memory first). It is written by the call that
/// made it, once that call holds no Mother lock, or by no one, and then the
/// broker counts its records lost. It has no `Drop` of its own.
#[derive(Default)]
pub struct Owed(Vec<RecordBatch>, bool);

impl Owed {
    /// Typed File lifecycle record result. Try every batch; an owed or
    /// uncertain record remains observable to the caller.
    pub(crate) fn write_checked(self) -> Result<(), AuthorityError> {
        #[cfg(test)]
        if !self.is_empty() {
            held::assert_none();
        }
        let mut failed = self.1;
        for batch in self.0 {
            // The batch owns its records. On unwind its Drop counts the
            // in-flight record uncertain and the remainder lost. Preserve
            // that native accounting, try the other owned batches, and return
            // the recording failure through the typed File lifecycle.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| batch.write())) {
                Ok(outcome) => failed |= outcome.owed != 0 || outcome.uncertain != 0,
                Err(_) => failed = true,
            }
        }
        if failed {
            Err(AuthorityError::EvidenceUnavailable)
        } else {
            Ok(())
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.1 && self.0.iter().all(RecordBatch::is_empty)
    }

    pub(crate) fn refuse(&mut self) {
        self.1 = true;
    }

    pub(crate) fn push(&mut self, batch: RecordBatch) {
        self.0.push(batch);
    }

    pub(crate) fn extend(&mut self, other: Owed) {
        self.1 |= other.1;
        self.0.extend(other.0);
    }

    /// Write every record, in order. Never called with a Mother lock held
    /// (tests check it on every write).
    pub fn write(self) {
        #[cfg(test)]
        if !self.is_empty() {
            held::assert_none();
        }
        for batch in self.0 {
            let _ = batch.write();
        }
    }
}

/// M2c PR-B (tests): which Mother locks this thread holds, so that every
/// batch write can check it holds none.
#[cfg(test)]
pub(crate) mod held {
    use std::cell::Cell;
    use std::thread::LocalKey;

    thread_local! {
        static COORDINATOR: Cell<bool> = const { Cell::new(false) };
        static PORT: Cell<bool> = const { Cell::new(false) };
    }

    /// Set while a lock is held; restored when dropped (a thread-local
    /// flag: no I/O, no lock).
    pub(crate) struct Held {
        key: &'static LocalKey<Cell<bool>>,
        was: bool,
    }

    impl Held {
        fn set(key: &'static LocalKey<Cell<bool>>) -> Self {
            let was = key.with(|k| k.replace(true));
            Self { key, was }
        }

        pub(crate) fn coordinator() -> Self {
            Self::set(&COORDINATOR)
        }

        pub(crate) fn port() -> Self {
            Self::set(&PORT)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            let was = self.was;
            self.key.with(|k| k.set(was));
        }
    }

    pub(crate) fn assert_none() {
        assert!(
            !COORDINATOR.with(Cell::get) && !PORT.with(Cell::get),
            "a broker record batch written while a Mother lock is held"
        );
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::adapter::GovernedPort {}
    impl Sealed for crate::adapter::SessionPort {}
    #[cfg(test)]
    impl Sealed for crate::oracles::RecordingPort {}
}

/// The calls Mother makes to reach the world.
pub trait EffectPort: Send + Sync + sealed::Sealed {
    fn prepare_file_wait(
        &self,
        _: MissionId,
        _: u64,
        _: &PendingAction,
    ) -> Result<Option<nexus_governed_control::host_session::approval::PreparedWait>, EffectRefusal>
    {
        Ok(None)
    }
    fn publish_file_wait(
        &self,
        _: nexus_governed_control::host_session::approval::PreparedWait,
        _: u64,
    ) -> Result<(), EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NotAdmitted,
        })
    }
    /// Present only for a mandatory, broker-issued host session port.
    fn session_context(&self) -> Option<nexus_governed_control::host_session::SessionContext> {
        None
    }
    /// Register before replay. The default port has no File eligibility.
    fn file_workspace(&self) -> Option<&str> {
        None
    }
    fn file_attach(&self) -> Result<Option<Arc<FileAttachment>>, FileSessionError> {
        Ok(None)
    }
    fn file_claim(&self, _: Arc<FileAttachment>) -> Result<HaltCell, FileSessionError> {
        Err(FileSessionError::WrongOwner)
    }
    fn file_admit(&self, _: MissionId, _: &Admission) -> Result<(), EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NoBrokerRoute,
        })
    }
    /// File dispatch identity is supplied by the coordinator, before proposal.
    fn perform_file(
        &self,
        _: MissionId,
        _: u64,
        _: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NotAdmitted,
        })
    }
    /// Read only the commitment belonging to this exact File effect. None
    /// proves this port has not begun that effect; an in-flight proposal errors.
    fn file_publication_for(
        &self,
        _: MissionId,
        _: u64,
    ) -> Result<Option<FileSnapshot>, EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NotAdmitted,
        })
    }
    fn file_publication(&self, _: MissionId) -> Result<Option<FileSnapshot>, EffectRefusal> {
        Ok(None)
    }
    fn file_deadline_elapsed(&self, _: &PendingAction) -> bool {
        false
    }
    fn file_ack(&self, _: &PendingAction) -> Result<(), EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NoBrokerRoute,
        })
    }
    /// M2e-C1b: the registered model, if this port has a model route.
    fn model_identity(&self) -> Option<ModelIdentityView> {
        None
    }
    /// M2e-C1b: bind the mission's run, once, to the cached yes and `identity`.
    fn admit_inference(
        &self,
        _: MissionId,
        _contract_hash: &str,
        _request: &str,
        _identity: [u8; 32],
    ) -> Result<(), EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NoBrokerRoute,
        })
    }
    /// M2e-C1b: one metered call. `Err`: refused before any attempt.
    /// SB-3 (O1): `invocation` is the sealed caller's own deny-only observer.
    fn model_call(
        &self,
        _: MissionId,
        _: &ModelRequest,
        _: &dyn CallMeter,
        _: &dyn CallDeadline,
        _invocation: &InvocationObserver,
    ) -> Result<ModelAttempt, EffectRefusal> {
        Err(EffectRefusal {
            class: BrokerClass::NoBrokerRoute,
        })
    }

    /// Perform one effect for `mission`.
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal>;
    /// The mission stopped, was denied, blocked or failed: nothing more of
    /// it may start, and what runs sees its cancellation.
    fn cancel(&self, mission: MissionId);
    /// The mission is done: finish what it opened once nothing waits.
    fn finish(&self, mission: MissionId);
    /// Whether the Owner's emergency stop holds.
    fn emergency_stopped(&self) -> bool;
    /// M2c: ask the Owner, natively, to consent to a mission: the exact
    /// contract hash and request. The approval exists only once the broker
    /// recorded the yes. No effect.
    fn confirm_mission(
        &self,
        request: &MissionConfirmation,
    ) -> Result<MissionApproval, AuthorityError>;
    /// M2c: ask the Owner, natively, to resume a held mission: the exact
    /// contract hash and request. The approval records the yes; it resumes
    /// nothing (the coordinator does, under its lock, if the request is
    /// still current). No effect.
    fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError>;
    /// What became of the action `mission` waits on (an effect that came
    /// back [`EffectOutcome::AwaitingApproval`]). Never approves it.
    fn approval(&self, mission: MissionId) -> ApprovalStatus;
    /// The identity of the action `mission` waits on, if the port knows it.
    fn pending(&self, mission: MissionId) -> Option<PendingAction>;
    /// A reopened coordinator hands back the journaled action of a mission
    /// that has not ended, so that this port can still withdraw it (cancel)
    /// and ask about it. The port uses it only if the broker's records
    /// agree: Mother's agent run, and that run's commitment.
    fn adopt(&self, mission: MissionId, pending: &PendingAction);
    /// Whether the broker no longer holds `pending` open: it is Mother's
    /// and in a final state, or the broker holds no such action. Only then
    /// does Mother journal it closed.
    fn settled(&self, pending: &PendingAction) -> bool;
    /// Cancel the mission at the port, as [`EffectPort::cancel`], and say
    /// whether the broker took the cancellation. Recovery and every end of
    /// a mission use it, so that a withdrawal that failed is an error.
    fn withdraw(&self, mission: MissionId) -> Result<(), EffectRefusal>;
    /// M2a-S: the coordinator broke (a journal append failed): halt the
    /// port's door, cancel every broker run this port holds, and keep no run
    /// opened or taken back later. M2c PR-B: all of it in memory; the
    /// records are returned, for the caller to write once it holds no lock.
    fn fence(&self) -> Owed;
    /// M2a-S: a journaled action this port is not given back (claimed by
    /// two missions): recorded, so that withdrawing it is an error while
    /// the broker still holds it open.
    fn hand(&self, mission: MissionId, pending: &PendingAction);
}
