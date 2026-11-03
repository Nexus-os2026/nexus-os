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
use nexus_governed_control::authority::halt_cell::{HaltCell, HaltCells};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::files::FileCreate;
use nexus_governed_control::governed::RecordBatch;
use nexus_governed_control::tool::ToolIntent;
use serde::{Deserialize, Serialize};

/// An effect a lane asks for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum EffectRequest {
    /// M2d: create one new file at the top level of the Owner's workspace
    /// (the broker's Files route): a request, never a path.
    File(FileCreate),
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
            EffectRequest::File(_) => EffectKind::File,
            EffectRequest::Network(_) => EffectKind::Network,
            EffectRequest::Process(_) => EffectKind::Process,
            EffectRequest::ModelTool(_) => EffectKind::ModelTool,
            EffectRequest::Spend { .. } => EffectKind::Spend,
        }
    }
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
    /// M2d (§11 M4′): it ran, and a cause ended its authority after the
    /// broker's final check: the broker's controlled codes, in order.
    Overtaken { causes: Vec<&'static str> },
    /// M2d (Codex review 1 of PR #78, HIGH 2): it ran, and the broker
    /// requires recovery (the file is published; its directory sync, or
    /// its write grant's closure, failed). Round 5 (D6): with the codes of
    /// a cause that also ended its authority after its final check.
    RecoveryRequired { causes: Vec<&'static str> },
    /// M2d-C round 4 (addendum 3): a File whose outcome the broker cannot
    /// confirm either way (uncertain, unreadable or unknown): a file may
    /// exist. Never "not published".
    Uncertain,
}

/// M2d-C round 4 (settlement design v2, rule 1): what the broker's pinned
/// record says of a File's publication, as the port read it once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Publication {
    Published {
        recovery_required: bool,
        interrupted: Vec<&'static str>,
    },
    /// Only this says nothing was written.
    ConfirmedNotPublished,
    /// Not final (not answered, or executing).
    Pending,
    Uncertain,
}

/// M2d (I7): what binds a mission's broker run to the Owner's consent, once
/// per consent lifetime, before its first File effect: the consented
/// contract hash, the request the Owner said yes to in this process, and
/// the lifetime (the journal sequence of that consent or resume).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub contract_hash: String,
    pub consent_request: String,
    pub generation: u64,
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
pub struct Owed(Vec<RecordBatch>);

impl Owed {
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(RecordBatch::is_empty)
    }

    pub(crate) fn push(&mut self, batch: RecordBatch) {
        self.0.push(batch);
    }

    pub(crate) fn extend(&mut self, other: Owed) {
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
    #[cfg(test)]
    impl Sealed for crate::oracles::RecordingPort {}
}

/// The calls Mother makes to reach the world.
pub trait EffectPort: Send + Sync + sealed::Sealed {
    /// Perform one effect for `mission`, under its admission `generation`
    /// (I7): once admitted, a perform of another generation is refused.
    fn perform(
        &self,
        mission: MissionId,
        generation: u64,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal>;
    /// M2d (I7's `admit`): bind the mission's run to the Owner's consent
    /// before its first File effect. No effect. (Named `admit_files`: the
    /// kernel's workspace guard `hm_m3_guards` reserves two-argument `admit`
    /// calls for its dispatch issuer.)
    fn admit_files(&self, mission: MissionId, admission: &Admission) -> Result<(), EffectRefusal>;
    /// M2d: the display name of the workspace the host gave the port, if
    /// any (data, never authority: the broker binds the workspace).
    fn workspace(&self) -> Option<String>;
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
    /// M2d-C round 4 (settlement design v2, rules 1 and 5): one snapshot
    /// of the File action `mission` dispatched last at this port: its
    /// identity and what the broker's pinned record says of it. `None`:
    /// the port holds no such action. An error: the broker could not be
    /// read. No effect, no wait, never a panic.
    fn publication(
        &self,
        mission: MissionId,
    ) -> Result<Option<(PendingAction, Publication)>, EffectRefusal>;
    /// M2d-C round 4 (rule 2, addendum 4): the coordinator journaled a
    /// conclusive outcome of File action `pending`: the broker may release
    /// its pin. Idempotent; one that fails is kept and tried again with
    /// the next acknowledgement (and on every reopen, from the journal).
    fn ack(&self, mission: MissionId, pending: &PendingAction) -> Result<(), EffectRefusal>;
    /// M2d-C round 5 (C1): the agent's partition of the broker's halt
    /// table (taken once, when the coordinator opens; not on a Stop path).
    fn halt_cells(&self) -> HaltCells;
    /// M2d-C round 5 (H2, C1): the mission's halt cell, bound before the
    /// mission is exposed: its run is opened with it, and an adoption for
    /// it attaches it.
    fn bind_halt(&self, mission: MissionId, cell: HaltCell);
    /// M2d-C round 5 (C2, C4): the emergency's door-wide memory stop
    /// (lock-free, no I/O, no panic), with its cohort's cells.
    fn emergency_halt(&self);
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
