//! The custody core's plain data: caller-supplied instants, core-assigned
//! identities, completion facts, failure classes, evidence records, requests
//! and the bounded snapshot the control side reads.
//!
//! Nothing in this module holds a native owner or can construct one. An
//! identity ([`ActionId`], [`EntryId`], [`RecordId`]) locates something the
//! core holds; its fields are private to the custody module, so no caller can
//! mint one, and no operation turns an identity, a name, a path, a process id
//! or a record back into an owner.

use std::sync::Arc;

/// A monotonic instant supplied by the caller, in milliseconds of one
/// monotonic clock. The core never reads a clock: every operation takes the
/// caller's `now`, and an instant before one already seen is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u64);

impl Tick {
    /// This instant plus `millis` (saturating).
    pub fn plus(self, millis: u64) -> Tick {
        Tick(self.0.saturating_add(millis))
    }
}

/// One custody instance's generation: a freshness binding the integration
/// chooses at random for each owner. It is never authentication. Transport
/// peers are authenticated outside this module, and quoting the right
/// generation earns a request nothing but its checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Generation([u8; 16]);

impl Generation {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn bytes(&self) -> [u8; 16] {
        self.0
    }
}

/// One native action of one generation, numbered by the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId {
    pub(super) generation: Generation,
    pub(super) seq: u64,
}

/// One custody entry (a held owner) of one generation, numbered by the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntryId {
    pub(super) generation: Generation,
    pub(super) seq: u64,
}

/// One evidence record of one generation, numbered by the core in the order
/// the recorder must make them durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordId {
    pub(super) generation: Generation,
    pub(super) seq: u64,
}

impl ActionId {
    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }
}

impl EntryId {
    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }
}

impl RecordId {
    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }
}

/// The caller's number for one validation case (data, not authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CaseId(pub u32);

/// What kind of owner an entry holds. A group holds at most one of each: the
/// native process tree, the workspace it uses and the fixture it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    Process,
    Workspace,
    Fixture,
}

/// What a case expects of its native cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expectation {
    /// Every owner the case creates is ended by its first confirming attempts;
    /// a retained owner is an unexpected failure.
    Clean,
    /// An expected negative control: one native operation's own finalization
    /// is expected to leave a retained process owner, and the first cleanup
    /// attempt on it must confirm its end.
    RetainedBoundary,
}

/// One completion fact: confirmed once (and when), never revoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fact {
    Pending,
    Confirmed(Tick),
}

impl Fact {
    pub fn confirmed(&self) -> bool {
        matches!(self, Fact::Confirmed(_))
    }
}

/// Whether a process owner's output evidence is complete: drained to its end,
/// or explicitly given up (detached) and recorded as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFact {
    Pending,
    Complete(Tick),
    Detached(Tick),
}

impl OutputFact {
    pub fn settled(&self) -> bool {
        !matches!(self, OutputFact::Pending)
    }
}

/// What one cleanup attempt observed about one completion fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The attempt did not look.
    NotLooked,
    /// The attempt looked; the fact does not hold yet.
    StillPending,
    /// The attempt established the fact.
    Confirmed,
}

/// What one cleanup attempt observed about a process owner's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputObserved {
    NotLooked,
    StillPending,
    Complete,
    Detached,
}

/// The separate facts one cleanup attempt reports. A process owner is ended
/// only when its native subtree is gone (`subtree`) *and* its direct child is
/// reaped (`reaped`); its entry finishes only when its output is also settled.
/// A workspace or fixture owner finishes only when its removal is confirmed.
/// No fact implies another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupReport {
    pub subtree: Observed,
    pub reaped: Observed,
    pub output: OutputObserved,
    pub removed: Observed,
}

impl CleanupReport {
    /// An attempt that established nothing.
    pub const NOTHING: CleanupReport = CleanupReport {
        subtree: Observed::NotLooked,
        reaped: Observed::NotLooked,
        output: OutputObserved::NotLooked,
        removed: Observed::NotLooked,
    };
}

/// The facts an operation reports when it ended natively without leaving an
/// owner (its own finalization confirmed the end). A process operation's end
/// is confirmed only by `subtree`, `reaped` and a settled `output` together; a
/// workspace or fixture operation's only by `removed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndFacts {
    pub subtree: bool,
    pub reaped: bool,
    pub output: OutputObserved,
    pub removed: bool,
}

/// Where a held owner came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Created for its case's use; ended when the case ends.
    Created,
    /// Returned by an operation whose own finalization could not confirm its
    /// end: cleanup is required at once.
    Retained,
    /// Returned where custody did not expect it (another kind, an occupied
    /// slot, or an operation already resolved). Retained for cleanup and
    /// counted as a failure, never dropped.
    Unexpected,
}

/// Whether a held owner is in use or awaiting its confirmed end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryPhase {
    Live,
    CleanupRequired,
}

/// A native operation's state while it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpPhase {
    /// Reserved, with its start evidence issued; not authorized to start.
    Reserved,
    /// Admitted (`admitted` is its admission number): native work may be
    /// running.
    InFlight { since: Tick, admitted: u64 },
    /// Admitted, and its result is unknown: the request may have reached the
    /// external service. Only a bound completion resolves it.
    Unknown { since: Tick, admitted: u64 },
}

/// The kinds of failure a run can record. They are kept apart: an expected
/// injected condition is not a failure at all, and a later operational
/// recovery never removes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailureClass {
    /// An assertion or observation of the case itself failed.
    Assertion,
    /// A native operation's own cleanup was unconfirmed in a case that did not
    /// expect it.
    UnexpectedRetained,
    /// A cleanup attempt that was required to confirm an end did not.
    UnexpectedCleanup,
    /// An expected condition did not occur, or did not meet its requirement.
    ExpectedConditionUnmet,
    /// An owner arrived where custody did not expect one.
    UnexpectedOwner,
    /// A native outcome stayed unknown when its case ended.
    UnknownOutcome,
    /// An operation reported an end without the facts that confirm it and
    /// without an owner: native authority was lost.
    AuthorityLost,
    /// An evidence record could not be made durable.
    RecordFailed,
    /// The run was cancelled before it finished.
    Cancelled,
}

impl FailureClass {
    pub const ALL: [FailureClass; 9] = [
        FailureClass::Assertion,
        FailureClass::UnexpectedRetained,
        FailureClass::UnexpectedCleanup,
        FailureClass::ExpectedConditionUnmet,
        FailureClass::UnexpectedOwner,
        FailureClass::UnknownOutcome,
        FailureClass::AuthorityLost,
        FailureClass::RecordFailed,
        FailureClass::Cancelled,
    ];

    pub fn index(self) -> usize {
        FailureClass::ALL
            .iter()
            .position(|class| *class == self)
            .unwrap_or(0)
    }
}

/// One recorded failure, with a bounded, escaped detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub class: FailureClass,
    pub case: Option<CaseId>,
    pub at: Tick,
    pub detail: String,
}

/// How an action settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    /// Everything it created was confirmed ended.
    Confirmed,
    /// It had no native effect.
    NothingCreated,
    /// It was never admitted.
    NotAdmitted,
}

/// Why admission closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Requested,
    LeaseLost,
    Shutdown,
    Stop,
    RunBudget,
    /// The recorder could not make a record durable: nothing further may be
    /// admitted, since nothing further could be recorded.
    RecordFailed,
}

/// A control fact that must itself become evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlFact {
    AdmissionClosed { reason: CancelReason, after: u64 },
    ShutdownRefused,
}

/// What an evidence record says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    RunStarted {
        dispositioned: u32,
    },
    CaseStarted {
        case: CaseId,
        expectation: Expectation,
    },
    ActionStarted {
        action: ActionId,
        kind: SlotKind,
    },
    ActionFailed {
        action: ActionId,
    },
    ActionSettled {
        action: ActionId,
        how: Settlement,
    },
    CaseEnded {
        case: CaseId,
        passed: bool,
    },
    Control {
        fact: ControlFact,
    },
    RecoveryRequired,
    RecoveryAttempt {
        attempt: u32,
        resolved: bool,
    },
    /// `resolved_by`: the explicit recovery attempt that resolved the run's
    /// native state, if one did. It never changes the verdict.
    RunEnded {
        verdict: Verdict,
        resolved_by: Option<u32>,
    },
}

/// One record the recorder must make durable, in `id` order. The digest binds
/// the acknowledgement to exactly this record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordIntent {
    pub id: RecordId,
    pub at: Tick,
    pub kind: RecordKind,
    pub digest: [u8; 32],
}

/// The recorder's acknowledgement that one record is durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordAck {
    pub id: RecordId,
    pub digest: [u8; 32],
}

impl RecordAck {
    /// The acknowledgement of exactly `intent`.
    pub fn of(intent: &RecordIntent) -> RecordAck {
        RecordAck {
            id: intent.id,
            digest: intent.digest,
        }
    }
}

/// What became of an acknowledgement (or a failure report).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckOutcome {
    /// The record is now acknowledged as durable.
    Acknowledged,
    /// The record was already acknowledged with the same digest: no effect.
    Duplicate,
    /// The digest is not the record's: rejected, no effect.
    Conflict,
    /// An earlier record is not acknowledged yet: rejected, no effect.
    OutOfOrder,
    /// No such record was issued: rejected, no effect.
    NotIssued,
    /// Another generation's record: rejected, no effect.
    Foreign,
    /// Recording already failed: nothing further is acknowledged.
    LedgerFailed,
    /// The recorder's failure to make this record durable is now recorded.
    FailureRecorded,
    /// The instant is before one already seen: rejected, no effect.
    ClockRegression,
}

/// The phase of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    NotStarted,
    Running,
    /// Native or evidence state stayed unresolved after the bounded automatic
    /// attempts: only explicit recovery requests proceed.
    RecoveryRequired,
    /// Everything native is resolved and the run's end record is issued.
    Complete,
    Closed,
}

/// The run's verdict. A failure makes it `Failed` for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pending,
    Passed,
    Failed,
}

/// Why admission or another operation was refused. A refusal changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    ClockRegression,
    Phase(RunPhase),
    PriorUnresolved,
    AdmissionClosed(Closure),
    EvidenceFailed,
    EvidenceCapacity,
    EvidencePending,
    CaseActive,
    NoCase,
    GroupUnresolved,
    SlotOccupied(SlotKind),
    OperationPending,
    NotReserved,
    UnknownEntry,
    DependencyUnresolved,
    NotRecoveryRequired,
    StaleEpoch,
    BudgetExhausted,
    TooSoon,
    Capacity,
    Undisposable,
}

/// The one closure of admission: why, when, and after how many admissions.
/// Actions numbered up to `after` were admitted before it; none after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closure {
    pub reason: CancelReason,
    pub at: Tick,
    pub after: u64,
}

/// The run's lease, as the control side holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    NotIssued,
    Active { deadline: Tick },
    Lost { at: Tick },
}

/// Why a lease renewal was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    Foreign,
    NotIssued,
    ClockRegression,
    Expired,
    Lost,
}

/// A request from the control side that changes state, bound to the
/// generation and to a strictly increasing sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub generation: Generation,
    pub seq: u64,
    pub op: RequestOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOp {
    /// One explicit recovery attempt, decided against recovery `epoch`.
    Retry { epoch: u64 },
    /// An application shutdown request.
    Shutdown,
}

/// What a request came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestOutcome {
    Executed(Response),
    /// The same request (same sequence number and payload) again: its first
    /// response, nothing executed.
    Duplicate(Response),
    /// The sequence number was used with a different payload: nothing executed.
    Conflict,
    /// An old sequence number whose response is no longer retained: never
    /// executed again.
    Replayed,
    /// A sequence number that skips one: nothing executed.
    Gap,
    /// Another generation's request: nothing executed.
    Foreign,
    /// Refused before sequencing (for example, a clock regression).
    Refused(Refusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Retried {
        attempt: u32,
        resolved: bool,
        epoch: u64,
    },
    RetryRefused(Refusal),
    Shutdown(ShutdownDecision),
}

/// Whether the application may shut down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownDecision {
    Permitted,
    /// Nothing native is held, but the run's evidence cannot be completed:
    /// shutting down leaves its durable state outstanding, which a later
    /// generation must refuse until it is dispositioned.
    PermittedWithoutDurableCompletion,
    Refused(Vec<Unresolved>),
}

/// What keeps the application from shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    RunActive,
    RecoveryRequired,
    CaseActive,
    OwnersHeld(u32),
    OperationPending,
    OutcomeUnknown,
    AuthorityLost(u32),
    EvidencePending(u64),
}

/// A prior generation's incident, as the records layer bound it (owner, last
/// record and outstanding set, digested). Data, not authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncidentBinding([u8; 32]);

impl IncidentBinding {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorOutcome {
    Resolved,
    Unresolved { outstanding: u32 },
    Malformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorIncident {
    pub binding: IncidentBinding,
    pub outcome: PriorOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispositionReason {
    OwnerDestroyed,
    HostRebooted,
    RecordsMalformed,
    CompletionNotRecorded,
    Other,
}

/// An external validator's verdict that an incident was dispositioned. The
/// core accepts one only from a [`super::core::DispositionValidator`] it asked
/// about exactly that incident, and only for that incident's binding; it never
/// derives one from a request, a filename or a string. A disposition allows
/// later admission; it never turns the incident into a confirmed cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedDisposition {
    pub(super) binding: IncidentBinding,
    pub(super) reason: DispositionReason,
}

impl ValidatedDisposition {
    /// For validators only (the records layer, or a fixture stand-in).
    pub fn new(binding: IncidentBinding, reason: DispositionReason) -> Self {
        Self { binding, reason }
    }
}

/// The core's bounds. [`Config::LIVE`] is the live harness's: one process
/// owner, one workspace and one fixture per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Automatic cleanup attempts per entry (the case's own and its end).
    pub automatic_attempts: u32,
    /// Explicit recovery attempts per run.
    pub recovery_budget: u32,
    /// The least time between two explicit recovery attempts.
    pub recovery_spacing_millis: u64,
    pub lease_millis: u64,
    /// Evidence records this generation may issue in all.
    pub record_capacity: u32,
    /// Of those, reserved for control facts (never action evidence).
    pub control_reserve: u32,
    /// Retained responses of executed requests.
    pub receipt_limit: usize,
    /// Owners from operations already resolved that custody will still hold
    /// as unexpected; further ones are returned to their caller. An open
    /// operation's own misfit owner is always held (so the unexpected list
    /// holds at most one more than this).
    pub unexpected_limit: usize,
    /// Failures kept in detail (beyond it, only classified and counted).
    pub failure_detail_limit: usize,
    /// Characters of one failure detail.
    pub detail_chars: usize,
    /// Prior incidents the core accepts.
    pub incident_limit: usize,
}

impl Config {
    pub const LIVE: Config = Config {
        automatic_attempts: 3,
        recovery_budget: 32,
        recovery_spacing_millis: 5_000,
        lease_millis: 30_000,
        record_capacity: 512,
        control_reserve: 2,
        receipt_limit: 16,
        unexpected_limit: 2,
        failure_detail_limit: 32,
        detail_chars: 512,
        incident_limit: 16,
    };
}

/// One held owner, as the snapshot shows it (no handle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryView {
    pub entry: EntryId,
    pub action: ActionId,
    pub kind: SlotKind,
    pub origin: Origin,
    pub phase: EntryPhase,
    pub since: Tick,
    pub subtree: Fact,
    pub reaped: Fact,
    pub output: OutputFact,
    pub removed: Fact,
    pub automatic_attempts: u32,
    pub explicit_attempts: u32,
    pub first_attempt_confirmed: Option<bool>,
    pub failure_recorded: bool,
}

/// The open native operation, as the snapshot shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpView {
    pub action: ActionId,
    pub kind: SlotKind,
    pub case: CaseId,
    pub phase: OpPhase,
    pub start: RecordId,
    pub start_acknowledged: bool,
}

/// An action physically settled whose settlement record is not yet durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlingView {
    pub action: ActionId,
    pub entry: Option<EntryId>,
    pub record: RecordId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseView {
    pub case: CaseId,
    pub expectation: Expectation,
    pub since: Tick,
    pub failed: bool,
    pub expected_observed: bool,
    pub retained_first_confirmed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceView {
    pub capacity: u32,
    pub issued: u64,
    pub acknowledged: u64,
    pub reserved: u32,
    pub control_left: u32,
    pub unsent: usize,
    pub failed: Option<(RecordId, Tick)>,
}

/// Explicit recovery so far. `resolved_by` is the attempt that resolved the
/// run's native state, if one did: a fact about that attempt, never a flag
/// that clears or excuses an earlier failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryView {
    pub attempts: u32,
    pub budget: u32,
    pub epoch: u64,
    pub last: Option<Tick>,
    pub resolved_by: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentView {
    pub binding: IncidentBinding,
    pub outcome: PriorOutcome,
    pub disposition: Option<DispositionReason>,
}

/// An adapter call in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusyWhat {
    Cleanup,
    Record,
    Lend,
    Disposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusyView {
    pub what: BusyWhat,
    pub since: Tick,
    pub entry: Option<EntryId>,
}

/// The execution owner's last published state: plain data, no handle, each
/// list bounded. It is a past observation (`published`, `at`), never a claim
/// about the present native state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: Generation,
    pub published: u64,
    pub at: Tick,
    pub phase: RunPhase,
    pub verdict: Verdict,
    pub cases_passed: u32,
    pub cases_failed: u32,
    pub case: Option<CaseView>,
    pub entries: Vec<EntryView>,
    pub operation: Option<OpView>,
    pub lost: Vec<ActionId>,
    pub settling: Vec<SettlingView>,
    pub released_waiting: usize,
    pub evidence: EvidenceView,
    pub recovery: RecoveryView,
    pub failures: Vec<Failure>,
    pub failure_overflow: u32,
    pub failure_counts: [u32; 9],
    pub incidents: Vec<IncidentView>,
    pub busy: Option<BusyView>,
    pub cancel_observed: Option<Tick>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionView {
    pub admitted: u64,
    pub closure: Option<Closure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseView {
    pub state: LeaseState,
    pub renewals: u64,
}

/// The control side's own facts, read fresh (each under its own lock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlView {
    pub admission: AdmissionView,
    pub lease: LeaseView,
    pub queued: bool,
}

/// What a read-only status request returns: the execution owner's last
/// snapshot and the control side's own facts, each with its own time. Reading
/// it changes nothing (in particular, it never renews the lease).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusView {
    pub snapshot: Arc<Snapshot>,
    pub control: ControlView,
}
