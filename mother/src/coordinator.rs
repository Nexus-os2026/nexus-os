//! The coordinator: each mission's state, moved one step at a time.
//!
//! Nothing reaches the broker for a mission before the Owner consents to
//! its exact contract. A step is reserved under the lock, run with the lock
//! released, and completed only if its reservation still holds: a Stop, an
//! expiry or the Owner's emergency stop that came meanwhile wins. Stop is
//! honoured from every state and never waits for a broker call.

use crate::broker::{Broker, Refusal};
use crate::contract::{ContractDigest, ContractError, EffectCategory, MissionContract};
use nexus_governed_control::authority::clock::Clock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

/// The most missions one coordinator keeps.
pub const MAX_MISSIONS: usize = 1024;
/// The most journal entries kept (the oldest are dropped first).
pub const MAX_JOURNAL: usize = 1024;

/// A mission's identity: a name, never authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MissionId(u64);

impl fmt::Display for MissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "msn-{:08}", self.0)
    }
}

/// Where a mission is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionState {
    /// Drafted; the Owner has not consented. Nothing reaches the broker.
    Drafted,
    /// Consented; `next_step` runs next.
    Approved { next_step: usize },
    /// A step is with the broker.
    Running { step: usize },
    /// The broker committed an R2 step that waits for the Owner's native
    /// approval.
    AwaitingApproval {
        step: usize,
        commitment: CommitmentId,
    },
    Completed,
    /// The broker refused or ended a step (or opening the run).
    Failed { step: usize, class: &'static str },
    /// The Owner declined consent.
    Declined,
    /// The consent's lifetime passed.
    Expired,
    Stopped,
}

impl MissionState {
    pub fn is_terminal(&self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }
}

/// One step as the Owner is shown it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepSummary {
    pub index: usize,
    pub domain: &'static str,
    pub categories: BTreeSet<EffectCategory>,
    pub declared_cost_minor: u64,
    /// The step's broker intent, in full, as canonical JSON.
    pub detail: String,
}

/// Exactly what the Owner consents to, built by the coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsentRequest {
    pub mission: MissionId,
    pub digest: ContractDigest,
    pub goal: String,
    pub steps: Vec<StepSummary>,
    pub categories: BTreeSet<EffectCategory>,
    pub total_declared_cost_minor: u64,
    pub consent_ttl_ms: u64,
}

/// The Owner's answer to a mission's consent request. The host provides
/// it (M2: a native dialog); Mother has no implementation of her own.
pub trait OwnerConsent {
    fn confirm_mission(&self, request: &ConsentRequest) -> bool;
}

/// What one step came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Done { text: Option<String> },
    AwaitingApproval { commitment: CommitmentId },
}

/// The report of one step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepReport {
    pub step: usize,
    pub outcome: StepOutcome,
}

/// Why the coordinator refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatorError {
    UnknownMission,
    Contract(ContractError),
    Capacity,
    /// Consent is asked only of a drafted mission.
    NotDrafted(MissionState),
    /// A step runs only in an approved mission.
    NotApproved(MissionState),
    /// Only a step awaiting approval is refreshed.
    NotAwaiting(MissionState),
    Declined,
    Expired,
    Stopped,
    Refused(Refusal),
}

/// What the journal records: identities, digests, indexes and classes only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalEvent {
    Drafted { digest: ContractDigest },
    ConsentAsked { digest: ContractDigest },
    Consented { digest: ContractDigest },
    Declined,
    /// An answer came after the mission had moved on (a Stop won).
    ConsentOvertaken,
    StepReserved { step: usize },
    RunOpened { run: RunId },
    StepDone { step: usize },
    StepAwaiting { step: usize, commitment: CommitmentId },
    StepRefused { step: usize, class: &'static str },
    /// A broker result came after the mission had moved on.
    StepOvertaken { step: usize },
    Expired,
    Stopped { reason: &'static str },
    RunCancelled { run: RunId },
    RunCancelFailed { run: RunId, class: &'static str },
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub seq: u64,
    pub mission: MissionId,
    pub wall_ms: u64,
    pub event: JournalEvent,
}

/// The coordinator.
pub struct Coordinator<B: Broker = GovernedControl> {
    broker: Arc<B>,
    clock: Arc<dyn Clock>,
}

impl<B: Broker> Coordinator<B> {
    pub fn new(broker: Arc<B>, clock: Arc<dyn Clock>) -> Self {
        let _ = (&broker, &clock);
        todo!("M1 implementation follows the oracle commit")
    }

    /// Draft a mission from a contract. Nothing reaches the broker.
    pub fn draft(&self, contract: MissionContract) -> Result<MissionId, CoordinatorError> {
        let _ = (contract, &self.broker, &self.clock);
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn state(&self, id: MissionId) -> Option<MissionState> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn digest(&self, id: MissionId) -> Option<ContractDigest> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// The broker run the mission opened, if any.
    pub fn run_of(&self, id: MissionId) -> Option<RunId> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// What the Owner would be shown. Nothing reaches the broker.
    pub fn consent_request(&self, id: MissionId) -> Result<ConsentRequest, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Ask the Owner. No lock is held while the Owner answers; a Stop that
    /// comes meanwhile wins over a yes.
    pub fn request_consent(
        &self,
        id: MissionId,
        owner: &dyn OwnerConsent,
    ) -> Result<(), CoordinatorError> {
        let _ = (id, owner);
        todo!("M1 implementation follows the oracle commit")
    }

    /// Run the next step through the broker.
    pub fn step(&self, id: MissionId) -> Result<StepReport, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Learn what became of a step awaiting the Owner's native approval.
    pub fn refresh(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Stop a mission, from whatever state it is in. Returns the state it
    /// is left in.
    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let _ = id;
        todo!("M1 implementation follows the oracle commit")
    }

    /// Stop every mission. Returns how many were not yet terminal.
    pub fn stop_all(&self) -> usize {
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn journal(&self) -> Vec<JournalEntry> {
        todo!("M1 implementation follows the oracle commit")
    }
}
