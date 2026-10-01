//! The custody core: one execution owner's custody of native owners, native
//! operations, evidence and failures, and the control side's view of it.
//!
//! [`Custody`] lives with the execution owner (one thread) and is never
//! shared: it holds every native owner, as a value, from the moment an
//! operation's completion deposits it until its end is confirmed, and then
//! hands it back ([`Custody::take_released`]). [`Control`] is the only shared
//! part: the admission gate, the lease, the request queue (one slot) and the
//! last published [`Snapshot`], each under its own lock, never two at once,
//! and none ever held across a callback, a cleanup attempt, a record
//! submission or a wait. Reading status renews nothing.
//!
//! One native action runs through: [`Custody::reserve`] (capacity and evidence
//! space are reserved and its start record is issued; nothing may start), the
//! recorder's acknowledgement of that start record, [`Custody::admit`] (the
//! admission linearization point: one critical section on the gate decides it
//! against any closure), the native work itself (outside this module), and
//! [`Custody::complete`] with the operation's ticket. A result that arrives
//! after cancellation is still adopted; a result that is unknown stays unknown
//! until a completion bound to the same ticket resolves it.
//!
//! Owners are only lent: to cleanup adapters ([`Cleanup`]), to the case's own
//! code ([`Custody::lend`]) and to the owner's own [`Resource::kind`]. A panic
//! inside any of them unwinds through the callback's frames only, and the
//! owner stays where custody keeps it. No path here drops, forgets, leaks or
//! exits with an unresolved owner: a failed attempt keeps it, an exhausted
//! budget keeps it, reporting borrows it, a refused shutdown keeps it, and
//! [`Custody::close`] hands the same custody back when it cannot close. What
//! this module cannot do is survive the destruction of its own process, or of
//! a [`Custody`] value the integration drops: that remains the integration's
//! obligation.

use std::collections::VecDeque;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use sha2::{Digest, Sha256};

use super::model::*;

/// An owner's own declaration of the slot it occupies. Custody asks it with
/// the owner borrowed: a panic in it leaves the owner held (as an unexpected
/// owner of unknown kind, treated as a native process tree).
pub trait Resource {
    fn kind(&self) -> SlotKind;
}

/// One bounded cleanup attempt on a held owner.
///
/// The owner is only borrowed: an attempt cannot consume it, and a panic
/// inside one unwinds through the attempt's frames while the owner stays where
/// custody keeps it. An attempt must not replace the owner (for example with
/// `mem::replace`): the value custody holds is the authority, and that same
/// value is the one released. The report states each fact separately, and the
/// core confirms nothing the report does not establish. An attempt must be
/// bounded: the control side stays responsive while it runs, the execution
/// owner does not.
///
/// Production cleanup calls that consume their owner
/// (`RetainedBoundary::retry(self) -> Result<(), Self>`,
/// `Workspace::remove(self) -> Result<(), RetainedWorkspace>`,
/// `RetainedWorkspace::retry(self) -> Result<(), RetainedWorkspace>`) are to be
/// wrapped by the integration in an owner type that holds the production value
/// in an `Option`. Such a wrapper's attempt takes the value, makes the
/// consuming call, and puts back whatever the call returns before it returns;
/// it reports a fact confirmed only when the consuming call returned `Ok`; and
/// it relies on the consuming call never unwinding (the call must contain its
/// own panics, as those production calls do). A wrapper whose `Option` is empty
/// after a call that did not return `Ok` has lost its owner and must report
/// every fact unconfirmed, forever. A boolean claim that an owner was retained
/// is never proof of it.
pub trait Cleanup<R> {
    fn attempt(&mut self, owner: &mut R, entry: &EntryView) -> CleanupReport;
}

/// Where issued evidence records go to be made durable. Submitting a record
/// is not durability: the recorder acknowledges each record later
/// ([`Custody::acknowledge`]) or reports that it failed
/// ([`Custody::record_failed`]). This module implements no storage.
pub trait RecordSink {
    fn submit(&mut self, intent: &RecordIntent);
}

/// The external authority on prior incidents' dispositions (the records
/// layer), asked about exactly one incident binding.
pub trait DispositionValidator {
    fn validate(&self, binding: &IncidentBinding) -> Option<ValidatedDisposition>;
}

/// One action's reserved capacity and evidence, before anything exists
/// natively. Consumed by [`Custody::admit`]; not `Clone`, and never
/// constructed outside the core.
#[derive(Debug)]
pub struct Reservation {
    action: ActionId,
}

impl Reservation {
    pub fn action(&self) -> ActionId {
        self.action
    }
}

/// The permit for one admitted native operation, and the binding its result
/// must carry. Not `Clone`, and never constructed outside the core.
#[derive(Debug)]
pub struct OpTicket {
    action: ActionId,
    kind: SlotKind,
}

impl OpTicket {
    pub fn action(&self) -> ActionId {
        self.action
    }

    pub fn kind(&self) -> SlotKind {
        self.kind
    }
}

/// An adapter's proof that one admitted operation had no native effect (for
/// example, a refusal its service made before acting). Bound to that
/// operation's ticket: a proof for another operation is rejected.
#[derive(Debug)]
pub struct NoEffectProof {
    action: ActionId,
    reason: &'static str,
}

impl NoEffectProof {
    pub fn for_ticket(ticket: &OpTicket, reason: &'static str) -> Self {
        Self {
            action: ticket.action,
            reason,
        }
    }

    pub fn reason(&self) -> &'static str {
        self.reason
    }
}

/// What one admitted native operation came to.
pub enum NativeOutcome<R> {
    /// It created an owner for its case's use.
    Created(R),
    /// Its own finalization could not confirm its end: the owner is retained.
    Retained(R),
    /// It ended without leaving an owner; these facts confirm the end, or (if
    /// they do not) native authority was lost.
    Ended(EndFacts),
    /// It had no native effect, as the bound proof shows.
    NoEffect(NoEffectProof),
    /// Its result is unknown (for example, a timed-out request): nothing may be
    /// concluded, not refusal, not absence and not permission.
    Unknown,
}

impl<R> NativeOutcome<R> {
    fn into_owner(self) -> Option<R> {
        match self {
            NativeOutcome::Created(owner) | NativeOutcome::Retained(owner) => Some(owner),
            NativeOutcome::Ended(_) | NativeOutcome::NoEffect(_) | NativeOutcome::Unknown => None,
        }
    }
}

/// The run's lease, held by the run's creator: only it can renew the lease.
/// Not `Clone`, and never constructed outside the core.
#[derive(Debug)]
pub struct LeaseCap {
    generation: Generation,
    id: u64,
}

/// The control queue's one slot is taken: the request is returned, neither
/// accepted nor consumed.
#[derive(Debug)]
pub struct Busy(pub Request);

/// An owner whose end custody confirmed, handed back to the caller.
pub struct Released<R> {
    pub entry: EntryId,
    pub owner: R,
}

impl<R> fmt::Debug for Released<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Released")
            .field("entry", &self.entry)
            .finish_non_exhaustive()
    }
}

/// What custody did with a completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Deposited(EntryId),
    /// Held, but where custody did not expect an owner: a failure, never a
    /// dropped owner.
    Unexpected(EntryId),
    Ended,
    NoEffect,
    StillUnknown,
    AuthorityLost,
}

/// A completion custody refused. Every owner it carried is in it.
pub enum Rejected<R> {
    /// Another generation's ticket: the owner (if any) is returned untouched.
    Foreign { owner: Option<R> },
    /// An instant before one already seen: the owner is returned untouched.
    ClockRegression { owner: Option<R> },
    /// The operation is already resolved and the result carries no owner.
    Stale,
    /// A no-effect proof bound to another operation.
    UnboundProof,
    /// Custody already holds as many unexpected owners as it may (or cannot
    /// reserve their evidence): this one is returned to its caller, never
    /// dropped.
    CustodyFull { owner: R },
}

impl<R> Rejected<R> {
    pub fn into_owner(self) -> Option<R> {
        match self {
            Rejected::Foreign { owner } | Rejected::ClockRegression { owner } => owner,
            Rejected::CustodyFull { owner } => Some(owner),
            Rejected::Stale | Rejected::UnboundProof => None,
        }
    }
}

impl<R> fmt::Debug for Rejected<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (name, owner) = match self {
            Rejected::Foreign { owner } => ("Foreign", owner.is_some()),
            Rejected::ClockRegression { owner } => ("ClockRegression", owner.is_some()),
            Rejected::Stale => ("Stale", false),
            Rejected::UnboundProof => ("UnboundProof", false),
            Rejected::CustodyFull { .. } => ("CustodyFull", true),
        };
        write!(f, "{name} {{ owner returned: {owner} }}")
    }
}

/// An admission custody refused. A transient refusal returns the reservation
/// for a later attempt; a final one settles the action as never admitted.
#[derive(Debug)]
pub struct AdmitRefused {
    pub refusal: Refusal,
    pub reservation: Option<Reservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LendError {
    UnknownEntry,
    Panicked,
    ClockRegression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushError {
    ClockRegression,
    /// The sink panicked; the record it was given is still unsent.
    SinkPanicked {
        sent: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaseOutcome {
    pub case: CaseId,
    pub passed: bool,
    /// Nothing native is held or pending once the case ended.
    pub resolved: bool,
    /// The run stopped: no further case may begin.
    pub stopped: bool,
}

/// A closed custody: its verdict, its failures and every owner still
/// awaiting its caller.
pub struct Closed<R> {
    pub verdict: Verdict,
    pub resolved_by: Option<u32>,
    /// Every issued record was acknowledged as durable.
    pub durable: bool,
    pub failures: Vec<Failure>,
    pub failure_counts: [u32; 9],
    pub failure_overflow: u32,
    pub released: Vec<Released<R>>,
}

impl<R> fmt::Debug for Closed<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closed")
            .field("verdict", &self.verdict)
            .field("resolved_by", &self.resolved_by)
            .field("durable", &self.durable)
            .field("failure_counts", &self.failure_counts)
            .field("released", &self.released.len())
            .finish_non_exhaustive()
    }
}

/// Admission's closure, and whether this request made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelReceipt {
    pub closure: Closure,
    pub first: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run one adapter or owner callback, containing a panic in it: only the
/// callback's frames unwind, never the core's. The panic's payload is dropped
/// under containment too (a payload whose drop panics again is beyond it).
fn contained<T>(callback: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(value) => Some(value),
        Err(payload) => {
            let _ = catch_unwind(AssertUnwindSafe(move || drop(payload)));
            None
        }
    }
}

struct Gate {
    admitted: u64,
    closure: Option<Closure>,
}

struct Lease {
    id: Option<u64>,
    state: LeaseState,
    renewals: u64,
    last: Tick,
}

/// The only state shared with the control side. Each lock guards one field,
/// is taken alone, and is held only for a copy or an assignment.
struct Shared {
    generation: Generation,
    lease_millis: u64,
    gate: Mutex<Gate>,
    lease: Mutex<Lease>,
    snapshot: Mutex<Arc<Snapshot>>,
    queue: Mutex<Option<Request>>,
}

impl Shared {
    /// Close admission, once: a later request returns the first closure.
    fn close(&self, reason: CancelReason, at: Tick) -> CancelReceipt {
        let mut gate = lock(&self.gate);
        match gate.closure {
            Some(closure) => CancelReceipt {
                closure,
                first: false,
            },
            None => {
                let closure = Closure {
                    reason,
                    at,
                    after: gate.admitted,
                };
                gate.closure = Some(closure);
                CancelReceipt {
                    closure,
                    first: true,
                }
            }
        }
    }

    fn closure(&self) -> Option<Closure> {
        lock(&self.gate).closure
    }
}

/// The control side's handle: status, cancellation, the lease and the request
/// queue. It holds no native owner and never waits for the execution owner.
#[derive(Clone)]
pub struct Control {
    shared: Arc<Shared>,
}

impl Control {
    /// Read-only status: the last published snapshot and the control side's
    /// own facts. It changes nothing; in particular it never renews the lease.
    pub fn status(&self) -> StatusView {
        let snapshot = Arc::clone(&lock(&self.shared.snapshot));
        let admission = {
            let gate = lock(&self.shared.gate);
            AdmissionView {
                admitted: gate.admitted,
                closure: gate.closure,
            }
        };
        let lease = {
            let lease = lock(&self.shared.lease);
            LeaseView {
                state: lease.state,
                renewals: lease.renewals,
            }
        };
        let queued = lock(&self.shared.queue).is_some();
        StatusView {
            snapshot,
            control: ControlView {
                admission,
                lease,
                queued,
            },
        }
    }

    /// Accept cancellation: admission closes at once, for good. It does not
    /// claim that anything in flight has ended.
    pub fn cancel(&self, reason: CancelReason, now: Tick) -> CancelReceipt {
        self.shared.close(reason, now)
    }

    /// Renew the lease. Only the holder of the run's [`LeaseCap`] can, and only
    /// before the lease expires.
    pub fn renew_lease(&self, cap: &LeaseCap, now: Tick) -> Result<Tick, LeaseError> {
        if cap.generation != self.shared.generation {
            return Err(LeaseError::Foreign);
        }
        {
            let mut lease = lock(&self.shared.lease);
            if lease.id != Some(cap.id) {
                return Err(LeaseError::NotIssued);
            }
            if now < lease.last {
                return Err(LeaseError::ClockRegression);
            }
            lease.last = now;
            match lease.state {
                LeaseState::Active { deadline } if now < deadline => {
                    let deadline = now.plus(self.shared.lease_millis);
                    lease.state = LeaseState::Active { deadline };
                    lease.renewals += 1;
                    return Ok(deadline);
                }
                LeaseState::Active { .. } => lease.state = LeaseState::Lost { at: now },
                LeaseState::Lost { .. } => return Err(LeaseError::Lost),
                LeaseState::NotIssued => return Err(LeaseError::NotIssued),
            }
        }
        // The lease's lock is released before the gate's is taken.
        self.shared.close(CancelReason::LeaseLost, now);
        Err(LeaseError::Expired)
    }

    /// Expire the lease if its deadline has passed: losing it closes
    /// admission.
    pub fn check_lease(&self, now: Tick) -> LeaseState {
        let (state, lost) = {
            let mut lease = lock(&self.shared.lease);
            if now > lease.last {
                lease.last = now;
            }
            match lease.state {
                LeaseState::Active { deadline } if now >= deadline => {
                    lease.state = LeaseState::Lost { at: now };
                    (lease.state, true)
                }
                state => (state, false),
            }
        };
        if lost {
            self.shared.close(CancelReason::LeaseLost, now);
        }
        state
    }

    /// Queue a request for the execution owner. The queue holds one: when it
    /// is full the request is returned, not accepted.
    pub fn submit(&self, request: Request) -> Result<(), Busy> {
        let mut queue = lock(&self.shared.queue);
        if queue.is_some() {
            return Err(Busy(request));
        }
        *queue = Some(request);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotRef {
    Process,
    Workspace,
    Fixture,
    Unexpected(usize),
}

/// An action's failure record: still reserved, or issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureRecord {
    Reserved,
    Issued,
}

struct Entry<R> {
    id: EntryId,
    action: ActionId,
    kind: SlotKind,
    origin: Origin,
    phase: EntryPhase,
    owner: R,
    since: Tick,
    subtree: Fact,
    reaped: Fact,
    output: OutputFact,
    removed: Fact,
    automatic: u32,
    explicit: u32,
    first: Option<bool>,
    /// The action's failure record (its settlement record is reserved until
    /// the entry finishes).
    failure: FailureRecord,
    unresolved_noted: bool,
}

impl<R> Entry<R> {
    fn new(
        id: EntryId,
        action: ActionId,
        kind: SlotKind,
        origin: Origin,
        failure: FailureRecord,
        owner: R,
        since: Tick,
    ) -> Self {
        Self {
            id,
            action,
            kind,
            origin,
            phase: EntryPhase::CleanupRequired,
            owner,
            since,
            subtree: Fact::Pending,
            reaped: Fact::Pending,
            output: OutputFact::Pending,
            removed: Fact::Pending,
            automatic: 0,
            explicit: 0,
            first: None,
            failure,
            unresolved_noted: false,
        }
    }

    /// The native end: a process tree's subtree is gone *and* its direct
    /// child is reaped; a workspace or fixture is removed.
    fn natively_ended(&self) -> bool {
        match self.kind {
            SlotKind::Process => self.subtree.confirmed() && self.reaped.confirmed(),
            SlotKind::Workspace | SlotKind::Fixture => self.removed.confirmed(),
        }
    }

    /// Everything custody requires before it releases the owner: the native
    /// end, and for a process tree its output settled too.
    fn finished(&self) -> bool {
        match self.kind {
            SlotKind::Process => self.natively_ended() && self.output.settled(),
            SlotKind::Workspace | SlotKind::Fixture => self.natively_ended(),
        }
    }

    /// Apply the facts one attempt established. A fact is confirmed once and
    /// never revoked, and no fact implies another.
    fn apply(&mut self, report: &CleanupReport, now: Tick) {
        let confirm = |fact: &mut Fact, observed: Observed| {
            if observed == Observed::Confirmed && !fact.confirmed() {
                *fact = Fact::Confirmed(now);
            }
        };
        match self.kind {
            SlotKind::Process => {
                confirm(&mut self.subtree, report.subtree);
                confirm(&mut self.reaped, report.reaped);
                if self.output == OutputFact::Pending {
                    match report.output {
                        OutputObserved::Complete => self.output = OutputFact::Complete(now),
                        OutputObserved::Detached => self.output = OutputFact::Detached(now),
                        OutputObserved::NotLooked | OutputObserved::StillPending => {}
                    }
                }
            }
            SlotKind::Workspace | SlotKind::Fixture => confirm(&mut self.removed, report.removed),
        }
    }

    fn view(&self) -> EntryView {
        EntryView {
            entry: self.id,
            action: self.action,
            kind: self.kind,
            origin: self.origin,
            phase: self.phase,
            since: self.since,
            subtree: self.subtree,
            reaped: self.reaped,
            output: self.output,
            removed: self.removed,
            automatic_attempts: self.automatic,
            explicit_attempts: self.explicit,
            first_attempt_confirmed: self.first,
            failure_recorded: self.failure == FailureRecord::Issued,
        }
    }
}

/// The one open native operation of the group. It holds its action's
/// failure and settlement reservations until it resolves.
struct Op {
    action: ActionId,
    kind: SlotKind,
    case: CaseId,
    start: RecordId,
    phase: OpPhase,
    failure: FailureRecord,
    unknown_noted: bool,
}

/// The resource group: one process tree, the workspace and the fixture it
/// uses, owners that arrived unexpectedly, and the one open operation.
struct Group<R> {
    process: Option<Entry<R>>,
    workspace: Option<Entry<R>>,
    fixture: Option<Entry<R>>,
    unexpected: Vec<Entry<R>>,
    op: Option<Op>,
}

impl<R> Group<R> {
    fn new() -> Self {
        Self {
            process: None,
            workspace: None,
            fixture: None,
            unexpected: Vec::new(),
            op: None,
        }
    }

    fn slot(&self, kind: SlotKind) -> &Option<Entry<R>> {
        match kind {
            SlotKind::Process => &self.process,
            SlotKind::Workspace => &self.workspace,
            SlotKind::Fixture => &self.fixture,
        }
    }

    fn slot_mut(&mut self, kind: SlotKind) -> &mut Option<Entry<R>> {
        match kind {
            SlotKind::Process => &mut self.process,
            SlotKind::Workspace => &mut self.workspace,
            SlotKind::Fixture => &mut self.fixture,
        }
    }

    fn entries(&self) -> impl Iterator<Item = &Entry<R>> {
        self.process
            .iter()
            .chain(self.workspace.iter())
            .chain(self.fixture.iter())
            .chain(self.unexpected.iter())
    }

    fn find(&self, id: EntryId) -> Option<SlotRef> {
        let at = |slot: &Option<Entry<R>>| slot.as_ref().is_some_and(|entry| entry.id == id);
        if at(&self.process) {
            Some(SlotRef::Process)
        } else if at(&self.workspace) {
            Some(SlotRef::Workspace)
        } else if at(&self.fixture) {
            Some(SlotRef::Fixture)
        } else {
            self.unexpected
                .iter()
                .position(|entry| entry.id == id)
                .map(SlotRef::Unexpected)
        }
    }

    fn entry(&self, slot: SlotRef) -> Option<&Entry<R>> {
        match slot {
            SlotRef::Process => self.process.as_ref(),
            SlotRef::Workspace => self.workspace.as_ref(),
            SlotRef::Fixture => self.fixture.as_ref(),
            SlotRef::Unexpected(index) => self.unexpected.get(index),
        }
    }

    fn entry_mut(&mut self, slot: SlotRef) -> Option<&mut Entry<R>> {
        match slot {
            SlotRef::Process => self.process.as_mut(),
            SlotRef::Workspace => self.workspace.as_mut(),
            SlotRef::Fixture => self.fixture.as_mut(),
            SlotRef::Unexpected(index) => self.unexpected.get_mut(index),
        }
    }

    /// Take an entry out of the group: only for an entry whose end custody
    /// has confirmed.
    fn take(&mut self, slot: SlotRef) -> Option<Entry<R>> {
        match slot {
            SlotRef::Process => self.process.take(),
            SlotRef::Workspace => self.workspace.take(),
            SlotRef::Fixture => self.fixture.take(),
            SlotRef::Unexpected(index) => {
                (index < self.unexpected.len()).then(|| self.unexpected.remove(index))
            }
        }
    }

    fn held(&self) -> usize {
        self.entries().count()
    }

    /// Entries holding native process trees (`native`) or the rest.
    fn ids(&self, native: bool) -> Vec<EntryId> {
        self.entries()
            .filter(|entry| (entry.kind == SlotKind::Process) == native)
            .map(|entry| entry.id)
            .collect()
    }

    /// Whether anything must be ended before another action may start: an
    /// entry awaiting its confirmed end, or any unexpected owner.
    fn awaiting_cleanup(&self) -> bool {
        !self.unexpected.is_empty()
            || self
                .entries()
                .any(|entry| entry.phase == EntryPhase::CleanupRequired)
    }
}

/// The evidence model: capacity reservations, issued records in the order the
/// recorder must make them durable, and their acknowledgements. Not storage.
struct Ledger {
    generation: Generation,
    general_capacity: u32,
    control_capacity: u32,
    general_issued: u32,
    general_reserved: u32,
    control_issued: u32,
    digests: Vec<[u8; 32]>,
    /// The sequence number of the next record to be acknowledged (from 1).
    next_ack: u64,
    unsent: VecDeque<RecordIntent>,
    failed: Option<(RecordId, Tick)>,
}

impl Ledger {
    fn new(generation: Generation, config: &Config) -> Self {
        Self {
            generation,
            general_capacity: config.record_capacity - config.control_reserve,
            control_capacity: config.control_reserve,
            general_issued: 0,
            general_reserved: 0,
            control_issued: 0,
            digests: Vec::new(),
            next_ack: 1,
            unsent: VecDeque::new(),
            failed: None,
        }
    }

    /// Reserve space for `count` future records, all or nothing.
    fn reserve(&mut self, count: u32) -> bool {
        let wanted =
            u64::from(self.general_issued) + u64::from(self.general_reserved) + u64::from(count);
        if wanted <= u64::from(self.general_capacity) {
            self.general_reserved += count;
            true
        } else {
            false
        }
    }

    fn release(&mut self, count: u32) {
        self.general_reserved = self.general_reserved.saturating_sub(count);
    }

    /// Issue a record against one reservation the caller holds.
    fn issue_reserved(&mut self, at: Tick, kind: RecordKind) -> RecordId {
        self.general_reserved = self.general_reserved.saturating_sub(1);
        self.general_issued = self.general_issued.saturating_add(1);
        self.push(at, kind)
    }

    /// Issue a control fact from the control reserve, which action records
    /// never use.
    fn issue_control(&mut self, at: Tick, kind: RecordKind) -> Option<RecordId> {
        if self.control_issued < self.control_capacity {
            self.control_issued += 1;
            Some(self.push(at, kind))
        } else {
            None
        }
    }

    fn push(&mut self, at: Tick, kind: RecordKind) -> RecordId {
        let id = RecordId {
            generation: self.generation,
            seq: self.issued() + 1,
        };
        let digest = record_digest(id, at, &kind);
        self.digests.push(digest);
        self.unsent.push_back(RecordIntent {
            id,
            at,
            kind,
            digest,
        });
        id
    }

    fn issued(&self) -> u64 {
        self.digests.len() as u64
    }

    fn is_acknowledged(&self, id: RecordId) -> bool {
        id.generation == self.generation && id.seq < self.next_ack
    }

    fn unacknowledged(&self) -> u64 {
        self.issued() + 1 - self.next_ack
    }

    /// Acknowledgements bind to the exact record and arrive in order.
    fn acknowledge(&mut self, ack: &RecordAck) -> AckOutcome {
        if ack.id.generation != self.generation {
            return AckOutcome::Foreign;
        }
        let seq = ack.id.seq;
        if seq == 0 || seq > self.issued() {
            return AckOutcome::NotIssued;
        }
        let expected = self.digests[(seq - 1) as usize];
        if seq < self.next_ack {
            return if ack.digest == expected {
                AckOutcome::Duplicate
            } else {
                AckOutcome::Conflict
            };
        }
        if self.failed.is_some() {
            return AckOutcome::LedgerFailed;
        }
        if seq > self.next_ack {
            return AckOutcome::OutOfOrder;
        }
        if ack.digest != expected {
            return AckOutcome::Conflict;
        }
        self.next_ack += 1;
        AckOutcome::Acknowledged
    }

    /// The recorder could not make the next record durable.
    fn fail(&mut self, id: RecordId, now: Tick) -> AckOutcome {
        if id.generation != self.generation {
            return AckOutcome::Foreign;
        }
        if self.failed.is_some() {
            return AckOutcome::LedgerFailed;
        }
        if id.seq == 0 || id.seq > self.issued() {
            return AckOutcome::NotIssued;
        }
        if id.seq < self.next_ack {
            return AckOutcome::Conflict;
        }
        if id.seq > self.next_ack {
            return AckOutcome::OutOfOrder;
        }
        self.failed = Some((id, now));
        AckOutcome::FailureRecorded
    }

    fn view(&self, capacity: u32) -> EvidenceView {
        EvidenceView {
            capacity,
            issued: self.issued(),
            acknowledged: self.next_ack - 1,
            reserved: self.general_reserved,
            control_left: self.control_capacity - self.control_issued,
            unsent: self.unsent.len(),
            failed: self.failed,
        }
    }
}

/// Failures: kept in detail up to a bound, beyond it only classified and
/// counted. Nothing ever removes one.
struct FailureLog {
    limit: usize,
    details: Vec<Failure>,
    overflow: u32,
    counts: [u32; 9],
}

impl FailureLog {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            details: Vec::new(),
            overflow: 0,
            counts: [0; 9],
        }
    }

    fn push(&mut self, failure: Failure) {
        let count = &mut self.counts[failure.class.index()];
        *count = count.saturating_add(1);
        if self.details.len() < self.limit {
            self.details.push(failure);
        } else {
            self.overflow = self.overflow.saturating_add(1);
        }
    }

    fn total(&self) -> u64 {
        self.counts.iter().map(|count| u64::from(*count)).sum()
    }
}

struct Receipt {
    seq: u64,
    digest: [u8; 32],
    response: Response,
}

/// Sequencing of state-changing requests: strictly increasing sequence
/// numbers, and a bounded set of receipts for exact repeats. Safety does not
/// depend on the receipts: an old number without one is never executed.
struct RequestGate {
    last: u64,
    receipts: VecDeque<Receipt>,
}

impl RequestGate {
    fn receipt(&self, seq: u64) -> Option<&Receipt> {
        self.receipts.iter().find(|receipt| receipt.seq == seq)
    }

    fn remember(&mut self, seq: u64, digest: [u8; 32], response: Response, limit: usize) {
        if limit == 0 {
            return;
        }
        if self.receipts.len() >= limit {
            self.receipts.pop_front();
        }
        self.receipts.push_back(Receipt {
            seq,
            digest,
            response,
        });
    }
}

struct CaseState {
    id: CaseId,
    expectation: Expectation,
    since: Tick,
    failed: bool,
    expected_observed: bool,
    retained_entry: Option<EntryId>,
    retained_first: Option<bool>,
}

impl CaseState {
    fn view(&self) -> CaseView {
        CaseView {
            case: self.id,
            expectation: self.expectation,
            since: self.since,
            failed: self.failed,
            expected_observed: self.expected_observed,
            retained_first_confirmed: self.retained_first,
        }
    }
}

struct Incident {
    binding: IncidentBinding,
    outcome: PriorOutcome,
    disposition: Option<DispositionReason>,
}

impl Incident {
    /// An unresolved or malformed incident blocks the run until an external
    /// validator dispositions it.
    fn blocks(&self) -> bool {
        self.outcome != PriorOutcome::Resolved && self.disposition.is_none()
    }

    fn view(&self) -> IncidentView {
        IncidentView {
            binding: self.binding,
            outcome: self.outcome,
            disposition: self.disposition,
        }
    }
}

struct RecoveryState {
    attempts: u32,
    epoch: u64,
    last: Option<Tick>,
    resolved_by: Option<u32>,
}

/// The run's own records still reserved: its end, recovery-required, and one
/// per explicit recovery attempt it may still make.
struct RunRecords {
    ended: bool,
    recovery: bool,
    retries: u32,
}

/// One execution owner's custody. Not shared; see the module documentation.
pub struct Custody<R> {
    config: Config,
    generation: Generation,
    shared: Arc<Shared>,
    clock: Tick,
    phase: RunPhase,
    verdict: Verdict,
    next_action: u64,
    next_entry: u64,
    case: Option<CaseState>,
    cases_passed: u32,
    cases_failed: u32,
    group: Group<R>,
    lost: Vec<ActionId>,
    released: Vec<Released<R>>,
    settling: Vec<SettlingView>,
    ledger: Ledger,
    failures: FailureLog,
    recovery: RecoveryState,
    requests: RequestGate,
    incidents: Vec<Incident>,
    run_records: RunRecords,
    cancel_observed: Option<Tick>,
    cancel_noted: bool,
    shutdown_refusal_recorded: bool,
    busy: Option<BusyView>,
    published: u64,
}

impl<R> fmt::Debug for Custody<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Custody")
            .field("generation", &self.generation)
            .field("phase", &self.phase)
            .field("verdict", &self.verdict)
            .field("held", &self.group.held())
            .field("open_operation", &self.group.op.is_some())
            .field("lost", &self.lost.len())
            .finish_non_exhaustive()
    }
}

impl<R: Resource> Custody<R> {
    /// A new custody for one generation, and its control handle. Prior
    /// generations' incidents (from the records layer) block the run until
    /// each is resolved or dispositioned.
    pub fn new(
        config: Config,
        generation: Generation,
        prior: Vec<PriorIncident>,
        now: Tick,
    ) -> Result<(Custody<R>, Control), Refusal> {
        if prior.len() > config.incident_limit
            || config.control_reserve < 2
            || config.control_reserve > config.record_capacity
            || config.automatic_attempts == 0
        {
            return Err(Refusal::Capacity);
        }
        let shared = Arc::new(Shared {
            generation,
            lease_millis: config.lease_millis,
            gate: Mutex::new(Gate {
                admitted: 0,
                closure: None,
            }),
            lease: Mutex::new(Lease {
                id: None,
                state: LeaseState::NotIssued,
                renewals: 0,
                last: now,
            }),
            snapshot: Mutex::new(Arc::new(blank_snapshot(generation, now, &config))),
            queue: Mutex::new(None),
        });
        let mut custody = Custody {
            config,
            generation,
            shared: Arc::clone(&shared),
            clock: now,
            phase: RunPhase::NotStarted,
            verdict: Verdict::Pending,
            next_action: 1,
            next_entry: 1,
            case: None,
            cases_passed: 0,
            cases_failed: 0,
            group: Group::new(),
            lost: Vec::new(),
            released: Vec::new(),
            settling: Vec::new(),
            ledger: Ledger::new(generation, &config),
            failures: FailureLog::new(config.failure_detail_limit),
            recovery: RecoveryState {
                attempts: 0,
                epoch: 0,
                last: None,
                resolved_by: None,
            },
            requests: RequestGate {
                last: 0,
                receipts: VecDeque::new(),
            },
            incidents: prior
                .into_iter()
                .map(|incident| Incident {
                    binding: incident.binding,
                    outcome: incident.outcome,
                    disposition: None,
                })
                .collect(),
            run_records: RunRecords {
                ended: false,
                recovery: false,
                retries: 0,
            },
            cancel_observed: None,
            cancel_noted: false,
            shutdown_refusal_recorded: false,
            busy: None,
            published: 0,
        };
        custody.publish();
        Ok((custody, Control { shared }))
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn control(&self) -> Control {
        Control {
            shared: Arc::clone(&self.shared),
        }
    }

    /// The last published snapshot (the same one the control side reads).
    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&lock(&self.shared.snapshot))
    }

    /// Ask the validator about one prior incident. Only a disposition bound
    /// to exactly that incident is accepted; the incident's own outcome is
    /// kept beside it.
    pub fn apply_disposition<V: DispositionValidator>(
        &mut self,
        binding: &IncidentBinding,
        validator: &V,
        now: Tick,
    ) -> Result<(), Refusal> {
        self.advance(now)?;
        let Some(index) = self
            .incidents
            .iter()
            .position(|incident| incident.binding == *binding && incident.blocks())
        else {
            self.publish();
            return Err(Refusal::Undisposable);
        };
        self.busy = Some(BusyView {
            what: BusyWhat::Disposition,
            since: now,
            entry: None,
        });
        self.publish();
        let validated = contained(|| validator.validate(binding)).flatten();
        self.busy = None;
        let result = match validated {
            Some(disposition) if disposition.binding == *binding => {
                self.incidents[index].disposition = Some(disposition.reason);
                Ok(())
            }
            _ => Err(Refusal::Undisposable),
        };
        self.publish();
        result
    }

    /// Start the run: its start record is issued, its end, recovery-required
    /// and every recovery attempt's records are reserved, and the lease is
    /// issued to the caller (the run's creator).
    pub fn start_run(&mut self, now: Tick) -> Result<LeaseCap, Refusal> {
        self.advance(now)?;
        let result = self.try_start(now);
        self.publish();
        result
    }

    fn try_start(&mut self, now: Tick) -> Result<LeaseCap, Refusal> {
        if self.phase != RunPhase::NotStarted {
            return Err(Refusal::Phase(self.phase));
        }
        if self.incidents.iter().any(Incident::blocks) {
            return Err(Refusal::PriorUnresolved);
        }
        if self.ledger.failed.is_some() {
            return Err(Refusal::EvidenceFailed);
        }
        if let Some(closure) = self.observe_closure(now) {
            return Err(Refusal::AdmissionClosed(closure));
        }
        let retries = self.config.recovery_budget;
        if !self.ledger.reserve(retries.saturating_add(3)) {
            return Err(Refusal::EvidenceCapacity);
        }
        let dispositioned = self
            .incidents
            .iter()
            .filter(|incident| incident.disposition.is_some())
            .count() as u32;
        self.ledger
            .issue_reserved(now, RecordKind::RunStarted { dispositioned });
        self.run_records = RunRecords {
            ended: true,
            recovery: true,
            retries,
        };
        self.phase = RunPhase::Running;
        let id = 1;
        {
            let mut lease = lock(&self.shared.lease);
            lease.id = Some(id);
            lease.state = LeaseState::Active {
                deadline: now.plus(self.config.lease_millis),
            };
            lease.last = now;
        }
        Ok(LeaseCap {
            generation: self.generation,
            id,
        })
    }

    /// Begin a case: only while running, with admission open, recording
    /// intact and nothing of an earlier case unresolved.
    pub fn begin_case(
        &mut self,
        case: CaseId,
        expectation: Expectation,
        now: Tick,
    ) -> Result<(), Refusal> {
        self.advance(now)?;
        let result = self.try_begin(case, expectation, now);
        self.publish();
        result
    }

    fn try_begin(
        &mut self,
        id: CaseId,
        expectation: Expectation,
        now: Tick,
    ) -> Result<(), Refusal> {
        if let Some(closure) = self.observe_closure(now) {
            return Err(Refusal::AdmissionClosed(closure));
        }
        if self.phase != RunPhase::Running {
            return Err(Refusal::Phase(self.phase));
        }
        if self.case.is_some() {
            return Err(Refusal::CaseActive);
        }
        if self.ledger.failed.is_some() {
            return Err(Refusal::EvidenceFailed);
        }
        if self.unresolved() {
            return Err(Refusal::GroupUnresolved);
        }
        if !self.ledger.reserve(2) {
            return Err(Refusal::EvidenceCapacity);
        }
        self.ledger.issue_reserved(
            now,
            RecordKind::CaseStarted {
                case: id,
                expectation,
            },
        );
        self.case = Some(CaseState {
            id,
            expectation,
            since: now,
            failed: false,
            expected_observed: false,
            retained_entry: None,
            retained_first: None,
        });
        Ok(())
    }

    /// Reserve one native action of `kind`: capacity is checked and evidence
    /// space for its start, failure and settlement records is reserved before
    /// anything exists natively, and its start record is issued. Nothing may
    /// start until [`Self::admit`].
    pub fn reserve(&mut self, kind: SlotKind, now: Tick) -> Result<Reservation, Refusal> {
        self.advance(now)?;
        let result = self.try_reserve(kind, now);
        self.publish();
        result
    }

    fn try_reserve(&mut self, kind: SlotKind, now: Tick) -> Result<Reservation, Refusal> {
        if let Some(closure) = self.observe_closure(now) {
            return Err(Refusal::AdmissionClosed(closure));
        }
        if self.phase != RunPhase::Running {
            return Err(Refusal::Phase(self.phase));
        }
        let case = self.case.as_ref().ok_or(Refusal::NoCase)?.id;
        if self.ledger.failed.is_some() {
            return Err(Refusal::EvidenceFailed);
        }
        if self.group.op.is_some() {
            return Err(Refusal::OperationPending);
        }
        if !self.lost.is_empty() || self.group.awaiting_cleanup() {
            return Err(Refusal::GroupUnresolved);
        }
        if self.group.slot(kind).is_some() {
            return Err(Refusal::SlotOccupied(kind));
        }
        if !self.ledger.reserve(3) {
            return Err(Refusal::EvidenceCapacity);
        }
        let action = ActionId {
            generation: self.generation,
            seq: self.next_action,
        };
        self.next_action += 1;
        let start = self
            .ledger
            .issue_reserved(now, RecordKind::ActionStarted { action, kind });
        self.group.op = Some(Op {
            action,
            kind,
            case,
            start,
            phase: OpPhase::Reserved,
            failure: FailureRecord::Reserved,
            unknown_noted: false,
        });
        Ok(Reservation { action })
    }

    /// Admit a reserved action. Its start record must be durable, and the
    /// gate decides, in one critical section, against any closure: an action
    /// is admitted (and numbered) before a closure, or refused after it. Only
    /// an admitted action may start natively.
    pub fn admit(&mut self, reservation: Reservation, now: Tick) -> Result<OpTicket, AdmitRefused> {
        if let Err(refusal) = self.advance(now) {
            return Err(AdmitRefused {
                refusal,
                reservation: Some(reservation),
            });
        }
        self.observe_closure(now);
        let result = self.try_admit(reservation, now);
        self.publish();
        result
    }

    fn try_admit(&mut self, reservation: Reservation, now: Tick) -> Result<OpTicket, AdmitRefused> {
        let (kind, start) = match &self.group.op {
            Some(op) if op.action == reservation.action && op.phase == OpPhase::Reserved => {
                (op.kind, op.start)
            }
            _ => {
                return Err(AdmitRefused {
                    refusal: Refusal::NotReserved,
                    reservation: Some(reservation),
                })
            }
        };
        if self.ledger.failed.is_some() {
            self.settle_unadmitted(now);
            return Err(AdmitRefused {
                refusal: Refusal::EvidenceFailed,
                reservation: None,
            });
        }
        let start_acked = self.ledger.is_acknowledged(start);
        if !start_acked {
            return Err(AdmitRefused {
                refusal: Refusal::EvidencePending,
                reservation: Some(reservation),
            });
        }
        // The admission linearization point.
        let admitted = {
            let mut gate = lock(&self.shared.gate);
            match gate.closure {
                Some(closure) => Err(closure),
                None => {
                    gate.admitted += 1;
                    Ok(gate.admitted)
                }
            }
        };
        match admitted {
            Err(closure) => {
                self.settle_unadmitted(now);
                Err(AdmitRefused {
                    refusal: Refusal::AdmissionClosed(closure),
                    reservation: None,
                })
            }
            Ok(admitted) => {
                if let Some(op) = self.group.op.as_mut() {
                    op.phase = OpPhase::InFlight {
                        since: now,
                        admitted,
                    };
                }
                Ok(OpTicket {
                    action: reservation.action,
                    kind,
                })
            }
        }
    }

    /// Settle the open, never-admitted action: nothing was created.
    fn settle_unadmitted(&mut self, now: Tick) {
        if matches!(&self.group.op, Some(op) if op.phase == OpPhase::Reserved) {
            self.resolve_without_owner(Settlement::NotAdmitted, now);
        }
    }

    /// Report an admitted operation's result, bound to its ticket. An owner it
    /// brings is held from here on (even after cancellation); an owner custody
    /// refuses is in the rejection, never dropped.
    pub fn complete(
        &mut self,
        ticket: &OpTicket,
        outcome: NativeOutcome<R>,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        if ticket.action.generation != self.generation {
            return Err(Rejected::Foreign {
                owner: outcome.into_owner(),
            });
        }
        if now < self.clock {
            return Err(Rejected::ClockRegression {
                owner: outcome.into_owner(),
            });
        }
        self.clock = now;
        let result = self.apply_completion(ticket, outcome, now);
        self.publish();
        result
    }

    fn apply_completion(
        &mut self,
        ticket: &OpTicket,
        outcome: NativeOutcome<R>,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        let open = matches!(
            &self.group.op,
            Some(op) if op.action == ticket.action
                && matches!(op.phase, OpPhase::InFlight { .. } | OpPhase::Unknown { .. })
        );
        if !open {
            // A late, stale or duplicate result of an operation already
            // resolved acts on nothing; an owner it brings is still held.
            return match outcome.into_owner() {
                Some(owner) => self.retain_stray(ticket.action, owner, now),
                None => Err(Rejected::Stale),
            };
        }
        match outcome {
            NativeOutcome::Unknown => Ok(self.mark_unknown(now)),
            NativeOutcome::NoEffect(proof) if proof.action != ticket.action => {
                Err(Rejected::UnboundProof)
            }
            NativeOutcome::NoEffect(_) => {
                Ok(self.resolve_without_owner(Settlement::NothingCreated, now))
            }
            NativeOutcome::Ended(facts) => Ok(self.resolve_ended(facts, now)),
            NativeOutcome::Created(owner) => self.deposit(ticket, owner, Origin::Created, now),
            NativeOutcome::Retained(owner) => self.deposit(ticket, owner, Origin::Retained, now),
        }
    }

    /// The operation's result is unknown: it stays open, admission of
    /// anything else stays blocked, and nothing it might have created may be
    /// discarded.
    fn mark_unknown(&mut self, now: Tick) -> Completion {
        if let Some(op) = self.group.op.as_mut() {
            if let OpPhase::InFlight { admitted, .. } = op.phase {
                op.phase = OpPhase::Unknown {
                    since: now,
                    admitted,
                };
            }
        }
        Completion::StillUnknown
    }

    /// Resolve the open operation as having left no owner.
    fn resolve_without_owner(&mut self, how: Settlement, now: Tick) -> Completion {
        if let Some(op) = self.group.op.take() {
            let record = self.ledger.issue_reserved(
                now,
                RecordKind::ActionSettled {
                    action: op.action,
                    how,
                },
            );
            if op.failure == FailureRecord::Reserved {
                self.ledger.release(1);
            }
            self.settling.push(SettlingView {
                action: op.action,
                entry: None,
                record,
            });
        }
        match how {
            Settlement::Confirmed => Completion::Ended,
            Settlement::NothingCreated | Settlement::NotAdmitted => Completion::NoEffect,
        }
    }

    /// An end without an owner is confirmed only by its facts; without them,
    /// native authority is lost: never a clean end, and never resolvable here.
    fn resolve_ended(&mut self, facts: EndFacts, now: Tick) -> Completion {
        let output = matches!(
            facts.output,
            OutputObserved::Complete | OutputObserved::Detached
        );
        let confirmed = match self.group.op.as_ref().map(|op| op.kind) {
            Some(SlotKind::Process) => facts.subtree && facts.reaped && output,
            Some(SlotKind::Workspace | SlotKind::Fixture) => facts.removed,
            None => false,
        };
        if confirmed {
            return self.resolve_without_owner(Settlement::Confirmed, now);
        }
        if let Some(mut op) = self.group.op.take() {
            // Its settlement record stays reserved: it can never settle here.
            self.issue_failure(&mut op.failure, op.action, now);
            self.lost.push(op.action);
        }
        self.fail(
            FailureClass::AuthorityLost,
            "an operation reported an end without the facts that confirm it, and without its owner",
            now,
        );
        Completion::AuthorityLost
    }

    /// Hold the owner the open operation returned. An owner of another kind,
    /// or for a slot already held, is held as unexpected (a failure), never
    /// dropped and never put in place of the held one.
    fn deposit(
        &mut self,
        ticket: &OpTicket,
        owner: R,
        origin: Origin,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        let Some(mut op) = self.group.op.take() else {
            return self.retain_stray(ticket.action, owner, now);
        };
        // The owner's own declaration, asked with the owner borrowed.
        let declared = contained(|| owner.kind());
        let id = self.new_entry_id();
        if declared != Some(op.kind) || self.group.slot(op.kind).is_some() {
            self.issue_failure(&mut op.failure, op.action, now);
            let kind = declared.unwrap_or(SlotKind::Process);
            let entry = Entry::new(
                id,
                op.action,
                kind,
                Origin::Unexpected,
                op.failure,
                owner,
                now,
            );
            self.group.unexpected.push(entry);
            self.fail(
                FailureClass::UnexpectedOwner,
                "an operation returned an owner custody did not reserve a slot for",
                now,
            );
            return Ok(Completion::Unexpected(id));
        }
        let mut entry = Entry::new(id, op.action, op.kind, origin, op.failure, owner, now);
        let open = self.shared.closure().is_none();
        if origin == Origin::Created && self.case.is_some() && open {
            entry.phase = EntryPhase::Live;
        }
        if origin == Origin::Retained {
            let expected = op.kind == SlotKind::Process
                && self.case.as_ref().is_some_and(|case| {
                    case.id == op.case
                        && case.expectation == Expectation::RetainedBoundary
                        && case.retained_entry.is_none()
                });
            match self.case.as_mut() {
                Some(case) if expected => {
                    case.expected_observed = true;
                    case.retained_entry = Some(id);
                }
                _ => {
                    self.issue_failure(&mut entry.failure, op.action, now);
                    self.fail(
                        FailureClass::UnexpectedRetained,
                        "an operation's own cleanup was unconfirmed where none was expected",
                        now,
                    );
                }
            }
        }
        *self.group.slot_mut(op.kind) = Some(entry);
        Ok(Completion::Deposited(id))
    }

    /// An owner from an operation already resolved: held as unexpected while
    /// custody may hold more (and can reserve its evidence), otherwise
    /// returned to the caller. If the run had already completed, it requires
    /// recovery again.
    fn retain_stray(
        &mut self,
        action: ActionId,
        owner: R,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        let reopen = self.phase == RunPhase::Complete;
        // Its failure and settlement records; reopening a completed run also
        // needs recovery-required, a new end and the remaining attempts'.
        let retries = self
            .config
            .recovery_budget
            .saturating_sub(self.recovery.attempts);
        let records = if reopen { retries.saturating_add(4) } else { 2 };
        if self.group.unexpected.len() >= self.config.unexpected_limit
            || !self.ledger.reserve(records)
        {
            return Err(Rejected::CustodyFull { owner });
        }
        let declared = contained(|| owner.kind());
        let id = self.new_entry_id();
        let mut failure = FailureRecord::Reserved;
        self.issue_failure(&mut failure, action, now);
        let kind = declared.unwrap_or(SlotKind::Process);
        let entry = Entry::new(id, action, kind, Origin::Unexpected, failure, owner, now);
        self.group.unexpected.push(entry);
        self.fail(
            FailureClass::UnexpectedOwner,
            "an operation already resolved returned another owner",
            now,
        );
        if reopen {
            // The run had ended holding nothing; it holds an owner again.
            self.phase = RunPhase::RecoveryRequired;
            self.run_records = RunRecords {
                ended: true,
                recovery: false,
                retries,
            };
            self.ledger
                .issue_reserved(now, RecordKind::RecoveryRequired);
        }
        Ok(Completion::Unexpected(id))
    }

    /// Issue an action's failure record from its reservation, once.
    fn issue_failure(&mut self, record: &mut FailureRecord, action: ActionId, now: Tick) {
        if *record == FailureRecord::Reserved {
            *record = FailureRecord::Issued;
            self.ledger
                .issue_reserved(now, RecordKind::ActionFailed { action });
        }
    }

    fn new_entry_id(&mut self) -> EntryId {
        let id = EntryId {
            generation: self.generation,
            seq: self.next_entry,
        };
        self.next_entry += 1;
        id
    }

    /// Lend a held owner to the case's own code (for evidence). The owner
    /// stays in custody whatever `f` does; a panic in `f` is contained. `f`
    /// must not replace the owner.
    pub fn lend<T>(
        &mut self,
        entry: EntryId,
        now: Tick,
        f: impl FnOnce(&mut R) -> T,
    ) -> Result<T, LendError> {
        if self.advance(now).is_err() {
            return Err(LendError::ClockRegression);
        }
        let Some(slot) = self.group.find(entry) else {
            return Err(LendError::UnknownEntry);
        };
        self.busy = Some(BusyView {
            what: BusyWhat::Lend,
            since: now,
            entry: Some(entry),
        });
        self.publish();
        let result = match self.group.entry_mut(slot) {
            Some(held) => contained(|| f(&mut held.owner)).ok_or(LendError::Panicked),
            None => Err(LendError::UnknownEntry),
        };
        self.busy = None;
        self.publish();
        result
    }

    /// Record an actual failed assertion or observation of the case.
    pub fn record_assertion_failure(&mut self, detail: &str, now: Tick) -> Result<(), Refusal> {
        self.advance(now)?;
        self.fail(FailureClass::Assertion, detail, now);
        self.publish();
        Ok(())
    }

    /// End one held owner now (the case is done with it), within its
    /// automatic attempt budget. A dependency is ended only once the native
    /// tree that used it has ended and no operation is open. `Ok(false)`: the
    /// owner is still held.
    pub fn end_entry<C: Cleanup<R>>(
        &mut self,
        entry: EntryId,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<bool, Refusal> {
        self.advance(now)?;
        let result = self.try_end_entry(entry, cleanup, now);
        self.publish();
        result
    }

    fn try_end_entry<C: Cleanup<R>>(
        &mut self,
        id: EntryId,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<bool, Refusal> {
        if self.phase != RunPhase::Running {
            return Err(Refusal::Phase(self.phase));
        }
        if self.case.is_none() {
            return Err(Refusal::NoCase);
        }
        let slot = self.group.find(id).ok_or(Refusal::UnknownEntry)?;
        let kind = self
            .group
            .entry(slot)
            .map(|entry| entry.kind)
            .ok_or(Refusal::UnknownEntry)?;
        if kind != SlotKind::Process && !self.dependencies_releasable() {
            return Err(Refusal::DependencyUnresolved);
        }
        if let Some(entry) = self.group.entry_mut(slot) {
            entry.phase = EntryPhase::CleanupRequired;
        }
        self.attempt_until(id, cleanup, false, now);
        let finished = self.group.find(id).is_none();
        if !finished {
            self.note_unresolved(id, now);
        }
        Ok(finished)
    }

    /// The case's code is done: an unadmitted action is settled, an admitted
    /// one still open becomes unknown, every held owner's end becomes
    /// required, the automatic attempts run (native trees first, a dependency
    /// only after the tree that used it ended), and the case is judged. The
    /// run stops if anything stays unresolved, admission is closed or
    /// recording failed.
    pub fn end_case<C: Cleanup<R>>(
        &mut self,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<CaseOutcome, Refusal> {
        self.advance(now)?;
        let result = self.try_end_case(cleanup, now);
        self.publish();
        result
    }

    fn try_end_case<C: Cleanup<R>>(
        &mut self,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<CaseOutcome, Refusal> {
        let Some(case) = self.case.as_ref().map(|case| case.id) else {
            return Err(Refusal::NoCase);
        };
        self.observe_closure(now);
        match self.group.op.as_ref().map(|op| op.phase) {
            Some(OpPhase::Reserved) => self.settle_unadmitted(now),
            Some(OpPhase::InFlight { .. }) | Some(OpPhase::Unknown { .. }) => {
                self.mark_unknown(now);
                self.note_unknown(now);
            }
            None => {}
        }
        for kind in [SlotKind::Process, SlotKind::Workspace, SlotKind::Fixture] {
            if let Some(entry) = self.group.slot_mut(kind).as_mut() {
                entry.phase = EntryPhase::CleanupRequired;
            }
        }
        self.cleanup_round(cleanup, false, now);
        for id in self
            .group
            .entries()
            .map(|entry| entry.id)
            .collect::<Vec<_>>()
        {
            self.note_unresolved(id, now);
        }
        let passed = self.judge_case(now);
        self.ledger
            .issue_reserved(now, RecordKind::CaseEnded { case, passed });
        if passed {
            self.cases_passed = self.cases_passed.saturating_add(1);
        } else {
            self.cases_failed = self.cases_failed.saturating_add(1);
        }
        self.case = None;
        let resolved = !self.unresolved();
        let closed = self.observe_closure(now).is_some();
        let stopped = !resolved || closed || self.ledger.failed.is_some();
        if stopped {
            self.stop(now);
        }
        Ok(CaseOutcome {
            case,
            passed,
            resolved,
            stopped,
        })
    }

    /// An admitted operation still open when its case ended: its outcome is
    /// unknown, which is a failure (recorded once).
    fn note_unknown(&mut self, now: Tick) {
        let Some(mut op) = self.group.op.take() else {
            return;
        };
        let first = !op.unknown_noted;
        op.unknown_noted = true;
        if first {
            self.issue_failure(&mut op.failure, op.action, now);
        }
        self.group.op = Some(op);
        if first {
            self.fail(
                FailureClass::UnknownOutcome,
                "a native outcome was still unknown when its case ended",
                now,
            );
        }
    }

    /// An entry still held after required attempts: its failure record is
    /// issued (if it was not already) and the failure counted, once.
    fn note_unresolved(&mut self, id: EntryId, now: Tick) {
        let Some(slot) = self.group.find(id) else {
            return;
        };
        let Some(entry) = self.group.entry_mut(slot) else {
            return;
        };
        if entry.unresolved_noted || entry.automatic.saturating_add(entry.explicit) == 0 {
            return;
        }
        entry.unresolved_noted = true;
        let action = entry.action;
        let mut failure = entry.failure;
        self.issue_failure(&mut failure, action, now);
        if let Some(entry) = self.group.entry_mut(slot) {
            entry.failure = failure;
        }
        self.fail(
            FailureClass::UnexpectedCleanup,
            "a required cleanup attempt did not confirm its end",
            now,
        );
    }

    fn judge_case(&mut self, now: Tick) -> bool {
        let Some((expectation, observed, first)) = self.case.as_ref().map(|case| {
            (
                case.expectation,
                case.expected_observed,
                case.retained_first,
            )
        }) else {
            return false;
        };
        if expectation == Expectation::RetainedBoundary {
            if !observed {
                self.fail(
                    FailureClass::ExpectedConditionUnmet,
                    "the expected retained boundary was not observed",
                    now,
                );
            } else if first != Some(true) {
                self.fail(
                    FailureClass::ExpectedConditionUnmet,
                    "the retained boundary was not confirmed by its first cleanup attempt",
                    now,
                );
            }
        }
        let failed = self.case.as_ref().is_some_and(|case| case.failed);
        !failed && !self.unresolved()
    }

    /// The run is told no further case will begin.
    pub fn finish_run(&mut self, now: Tick) -> Result<RunPhase, Refusal> {
        self.advance(now)?;
        self.observe_closure(now);
        let result = if self.phase != RunPhase::Running {
            Err(Refusal::Phase(self.phase))
        } else if self.case.is_some() {
            Err(Refusal::CaseActive)
        } else {
            self.stop(now);
            Ok(self.phase)
        };
        self.publish();
        result
    }

    /// Observe the control side's closure at a safe point: it is recorded once
    /// (as a control fact), and an idle run stops.
    pub fn observe_control(&mut self, now: Tick) -> Result<Option<Closure>, Refusal> {
        self.advance(now)?;
        let closure = self.observe_closure(now);
        if closure.is_some() && self.phase == RunPhase::Running && self.case.is_none() {
            self.stop(now);
        }
        self.publish();
        Ok(closure)
    }

    fn observe_closure(&mut self, now: Tick) -> Option<Closure> {
        let closure = self.shared.closure();
        if let Some(closure) = closure {
            if self.cancel_observed.is_none() {
                self.cancel_observed = Some(now);
                self.ledger.issue_control(
                    now,
                    RecordKind::Control {
                        fact: ControlFact::AdmissionClosed {
                            reason: closure.reason,
                            after: closure.after,
                        },
                    },
                );
            }
        }
        closure
    }

    /// Native owners first; a dependency only once the native tree that used
    /// it has ended and no operation is open.
    fn cleanup_round<C: Cleanup<R>>(&mut self, cleanup: &mut C, explicit: bool, now: Tick) {
        for id in self.group.ids(true) {
            self.attempt_until(id, cleanup, explicit, now);
        }
        if self.dependencies_releasable() {
            for id in self.group.ids(false) {
                self.attempt_until(id, cleanup, explicit, now);
            }
        }
    }

    /// Whether dependencies may be released: no operation is open (an unknown
    /// one may still be using them), no authority was lost, and every process
    /// tree has ended natively.
    fn dependencies_releasable(&self) -> bool {
        let process_ended = self
            .group
            .process
            .as_ref()
            .is_none_or(Entry::natively_ended);
        self.group.op.is_none()
            && self.lost.is_empty()
            && process_ended
            && self
                .group
                .unexpected
                .iter()
                .all(|entry| entry.kind != SlotKind::Process || entry.natively_ended())
    }

    /// Explicitly: one attempt. Automatically: until the entry is finished or
    /// its automatic budget is spent.
    fn attempt_until<C: Cleanup<R>>(
        &mut self,
        id: EntryId,
        cleanup: &mut C,
        explicit: bool,
        now: Tick,
    ) {
        loop {
            let Some(slot) = self.group.find(id) else {
                return;
            };
            let spent = self
                .group
                .entry(slot)
                .is_none_or(|entry| entry.automatic >= self.config.automatic_attempts);
            if !explicit && spent {
                return;
            }
            let finished = self.attempt(slot, cleanup, explicit, now);
            if finished || explicit {
                return;
            }
        }
    }

    /// One attempt on one held owner. The owner is lent, never moved; a panic
    /// in the adapter is contained and the owner stays held.
    fn attempt<C: Cleanup<R>>(
        &mut self,
        slot: SlotRef,
        cleanup: &mut C,
        explicit: bool,
        now: Tick,
    ) -> bool {
        let Some(view) = self.group.entry(slot).map(Entry::view) else {
            return false;
        };
        self.busy = Some(BusyView {
            what: BusyWhat::Cleanup,
            since: now,
            entry: Some(view.entry),
        });
        self.publish();
        let report = match self.group.entry_mut(slot) {
            Some(entry) => contained(|| cleanup.attempt(&mut entry.owner, &view)),
            None => Some(CleanupReport::NOTHING),
        };
        self.busy = None;
        let panicked = report.is_none();
        let report = report.unwrap_or(CleanupReport::NOTHING);
        let (finished, first) = match self.group.entry_mut(slot) {
            Some(entry) => {
                if explicit {
                    entry.explicit = entry.explicit.saturating_add(1);
                } else {
                    entry.automatic = entry.automatic.saturating_add(1);
                }
                entry.phase = EntryPhase::CleanupRequired;
                entry.apply(&report, now);
                let finished = entry.finished();
                let first = entry.first.is_none();
                if first {
                    entry.first = Some(finished);
                }
                (finished, first)
            }
            None => (false, false),
        };
        if first {
            if let Some(case) = self.case.as_mut() {
                if case.retained_entry == Some(view.entry) {
                    case.retained_first = Some(finished);
                }
            }
        }
        if panicked {
            self.fail(
                FailureClass::UnexpectedCleanup,
                "a cleanup adapter panicked; its owner is still held",
                now,
            );
        }
        if finished {
            self.finish(slot, now);
        }
        self.publish();
        finished
    }

    /// Release an owner whose end is confirmed: it is handed back to the
    /// caller, and its settlement record is issued.
    fn finish(&mut self, slot: SlotRef, now: Tick) {
        let Some(entry) = self.group.take(slot) else {
            return;
        };
        let Entry {
            id,
            action,
            owner,
            failure,
            ..
        } = entry;
        // Handed over first; the bookkeeping follows.
        self.released.push(Released { entry: id, owner });
        let record = self.ledger.issue_reserved(
            now,
            RecordKind::ActionSettled {
                action,
                how: Settlement::Confirmed,
            },
        );
        if failure == FailureRecord::Reserved {
            self.ledger.release(1);
        }
        self.settling.push(SettlingView {
            action,
            entry: Some(id),
            record,
        });
    }

    /// Owners whose end custody confirmed, handed back. They hold nothing
    /// native any more; the caller decides what becomes of them.
    pub fn take_released(&mut self) -> Vec<Released<R>> {
        let released = std::mem::take(&mut self.released);
        self.publish();
        released
    }

    fn unresolved(&self) -> bool {
        self.group.held() > 0 || self.group.op.is_some() || !self.lost.is_empty()
    }

    /// No further case: the run either completes or requires recovery.
    fn stop(&mut self, now: Tick) {
        if self.phase != RunPhase::Running {
            return;
        }
        if let Some(closure) = self.observe_closure(now) {
            if closure.reason != CancelReason::RecordFailed && !self.cancel_noted {
                self.cancel_noted = true;
                self.fail(
                    FailureClass::Cancelled,
                    "the run was cancelled before it finished",
                    now,
                );
            }
        }
        if self.unresolved() {
            self.phase = RunPhase::RecoveryRequired;
            self.verdict = Verdict::Failed;
            if self.run_records.recovery {
                self.run_records.recovery = false;
                self.ledger
                    .issue_reserved(now, RecordKind::RecoveryRequired);
            }
        } else {
            self.complete_run(now, None);
        }
    }

    /// The run's native state is resolved: its end is recorded. A verdict once
    /// failed stays failed.
    fn complete_run(&mut self, now: Tick, resolved_by: Option<u32>) {
        self.recovery.resolved_by = resolved_by;
        if self.failures.total() > 0 || self.cases_failed > 0 {
            self.verdict = Verdict::Failed;
        }
        if self.verdict != Verdict::Failed {
            self.verdict = Verdict::Passed;
        }
        if self.run_records.recovery {
            self.run_records.recovery = false;
            self.ledger.release(1);
        }
        self.ledger.release(self.run_records.retries);
        self.run_records.retries = 0;
        if self.run_records.ended {
            self.run_records.ended = false;
            let verdict = self.verdict;
            self.ledger.issue_reserved(
                now,
                RecordKind::RunEnded {
                    verdict,
                    resolved_by,
                },
            );
        }
        self.phase = RunPhase::Complete;
    }

    /// Serve the queued request, if any. State-changing requests are bound to
    /// this generation and to strictly increasing sequence numbers.
    pub fn serve<C: Cleanup<R>>(
        &mut self,
        cleanup: &mut C,
        now: Tick,
    ) -> Option<(Request, RequestOutcome)> {
        let request = lock(&self.shared.queue).take()?;
        let outcome = match self.advance(now) {
            Err(refusal) => RequestOutcome::Refused(refusal),
            Ok(()) => {
                self.observe_closure(now);
                self.sequence(&request, cleanup, now)
            }
        };
        self.publish();
        Some((request, outcome))
    }

    fn sequence<C: Cleanup<R>>(
        &mut self,
        request: &Request,
        cleanup: &mut C,
        now: Tick,
    ) -> RequestOutcome {
        if request.generation != self.generation {
            return RequestOutcome::Foreign;
        }
        let digest = request_digest(request);
        let last = self.requests.last;
        let known = self
            .requests
            .receipt(request.seq)
            .map(|receipt| (receipt.digest == digest, receipt.response.clone()));
        match known {
            Some((true, response)) => return RequestOutcome::Duplicate(response),
            Some((false, _)) => return RequestOutcome::Conflict,
            None if request.seq <= last => return RequestOutcome::Replayed,
            None => {}
        }
        if Some(request.seq) != last.checked_add(1) {
            return RequestOutcome::Gap;
        }
        self.execute(request, digest, cleanup, now)
    }

    fn execute<C: Cleanup<R>>(
        &mut self,
        request: &Request,
        digest: [u8; 32],
        cleanup: &mut C,
        now: Tick,
    ) -> RequestOutcome {
        let response = match request.op {
            RequestOp::Retry { epoch } => self.retry(epoch, cleanup, now),
            RequestOp::Shutdown => Response::Shutdown(self.shutdown_requested(now)),
        };
        self.requests.remember(
            request.seq,
            digest,
            response.clone(),
            self.config.receipt_limit,
        );
        self.requests.last = self.requests.last.max(request.seq);
        RequestOutcome::Executed(response)
    }

    /// One explicit recovery attempt: only while recovery is required, decided
    /// against the current epoch, within the budget and the spacing, and with
    /// its record's space reserved (since the run started). Each unresolved
    /// owner gets one attempt (native trees first). Nothing is removed because
    /// a budget is spent, and no earlier failure is forgotten.
    fn retry<C: Cleanup<R>>(&mut self, epoch: u64, cleanup: &mut C, now: Tick) -> Response {
        if self.phase != RunPhase::RecoveryRequired {
            return Response::RetryRefused(Refusal::NotRecoveryRequired);
        }
        if epoch != self.recovery.epoch {
            return Response::RetryRefused(Refusal::StaleEpoch);
        }
        if self.recovery.attempts >= self.config.recovery_budget {
            return Response::RetryRefused(Refusal::BudgetExhausted);
        }
        if let Some(last) = self.recovery.last {
            if now < last.plus(self.config.recovery_spacing_millis) {
                return Response::RetryRefused(Refusal::TooSoon);
            }
        }
        if self.run_records.retries == 0 {
            return Response::RetryRefused(Refusal::EvidenceCapacity);
        }
        self.run_records.retries -= 1;
        self.recovery.attempts += 1;
        self.recovery.epoch += 1;
        self.recovery.last = Some(now);
        let attempt = self.recovery.attempts;
        self.cleanup_round(cleanup, true, now);
        let resolved = !self.unresolved();
        self.ledger
            .issue_reserved(now, RecordKind::RecoveryAttempt { attempt, resolved });
        if resolved {
            self.complete_run(now, Some(attempt));
        }
        Response::Retried {
            attempt,
            resolved,
            epoch: self.recovery.epoch,
        }
    }

    /// An application shutdown request: never an exit, a release or a drop.
    /// While the run is active it closes admission (the shutdown waits until
    /// everything is resolved); while anything is unresolved it is refused,
    /// and the first refusal becomes evidence.
    fn shutdown_requested(&mut self, now: Tick) -> ShutdownDecision {
        if self.phase == RunPhase::Running {
            self.shared.close(CancelReason::Shutdown, now);
            self.observe_closure(now);
            if self.case.is_none() {
                self.stop(now);
            }
        }
        let decision = self.shutdown_decision();
        if matches!(decision, ShutdownDecision::Refused(_)) && !self.shutdown_refusal_recorded {
            self.shutdown_refusal_recorded = true;
            self.ledger.issue_control(
                now,
                RecordKind::Control {
                    fact: ControlFact::ShutdownRefused,
                },
            );
        }
        decision
    }

    /// Whether the application may shut down now (it changes nothing).
    pub fn shutdown_decision(&self) -> ShutdownDecision {
        let mut unresolved = Vec::new();
        match self.phase {
            RunPhase::Running => unresolved.push(Unresolved::RunActive),
            RunPhase::RecoveryRequired => unresolved.push(Unresolved::RecoveryRequired),
            RunPhase::NotStarted | RunPhase::Complete | RunPhase::Closed => {}
        }
        if self.case.is_some() {
            unresolved.push(Unresolved::CaseActive);
        }
        let held = self.group.held() as u32;
        if held > 0 {
            unresolved.push(Unresolved::OwnersHeld(held));
        }
        match self.group.op.as_ref().map(|op| op.phase) {
            Some(OpPhase::Unknown { .. }) => unresolved.push(Unresolved::OutcomeUnknown),
            Some(OpPhase::Reserved | OpPhase::InFlight { .. }) => {
                unresolved.push(Unresolved::OperationPending)
            }
            None => {}
        }
        if !self.lost.is_empty() {
            unresolved.push(Unresolved::AuthorityLost(self.lost.len() as u32));
        }
        if self.ledger.failed.is_none() {
            let pending = self.ledger.unacknowledged();
            if pending > 0 {
                unresolved.push(Unresolved::EvidencePending(pending));
            }
        }
        if !unresolved.is_empty() {
            ShutdownDecision::Refused(unresolved)
        } else if self.ledger.failed.is_some() {
            ShutdownDecision::PermittedWithoutDurableCompletion
        } else {
            ShutdownDecision::Permitted
        }
    }

    /// Close the custody if nothing is unresolved; otherwise the same custody
    /// comes back, unchanged in what it holds.
    pub fn close(mut self, now: Tick) -> Result<Closed<R>, Box<Custody<R>>> {
        if self.advance(now).is_err() {
            return Err(Box::new(self));
        }
        let durable = match self.shutdown_decision() {
            ShutdownDecision::Permitted => true,
            ShutdownDecision::PermittedWithoutDurableCompletion => false,
            ShutdownDecision::Refused(_) => {
                self.publish();
                return Err(Box::new(self));
            }
        };
        let released = std::mem::take(&mut self.released);
        self.phase = RunPhase::Closed;
        self.publish();
        Ok(Closed {
            verdict: self.verdict,
            resolved_by: self.recovery.resolved_by,
            durable,
            failures: self.failures.details.clone(),
            failure_counts: self.failures.counts,
            failure_overflow: self.failures.overflow,
            released,
        })
    }

    /// Hand the issued, unsent records to the recorder, in order. A record the
    /// sink panics on stays unsent.
    pub fn flush_records<S: RecordSink>(
        &mut self,
        sink: &mut S,
        now: Tick,
    ) -> Result<usize, FlushError> {
        if self.advance(now).is_err() {
            return Err(FlushError::ClockRegression);
        }
        let mut sent = 0;
        while let Some(intent) = self.ledger.unsent.front().cloned() {
            self.busy = Some(BusyView {
                what: BusyWhat::Record,
                since: now,
                entry: None,
            });
            self.publish();
            let submitted = contained(|| sink.submit(&intent));
            self.busy = None;
            if submitted.is_none() {
                self.publish();
                return Err(FlushError::SinkPanicked { sent });
            }
            self.ledger.unsent.pop_front();
            sent += 1;
        }
        self.publish();
        Ok(sent)
    }

    /// The recorder's acknowledgement that one record is durable.
    pub fn acknowledge(&mut self, ack: &RecordAck, now: Tick) -> AckOutcome {
        if self.advance(now).is_err() {
            return AckOutcome::ClockRegression;
        }
        let outcome = self.ledger.acknowledge(ack);
        if outcome == AckOutcome::Acknowledged {
            let ledger = &self.ledger;
            self.settling
                .retain(|settling| !ledger.is_acknowledged(settling.record));
        }
        self.publish();
        outcome
    }

    /// The recorder's report that the next record could not be made durable.
    /// Recording stays failed: admission closes for good, and completion can
    /// never be claimed durable.
    pub fn record_failed(&mut self, id: RecordId, now: Tick) -> AckOutcome {
        if self.advance(now).is_err() {
            return AckOutcome::ClockRegression;
        }
        let outcome = self.ledger.fail(id, now);
        if outcome == AckOutcome::FailureRecorded {
            self.shared.close(CancelReason::RecordFailed, now);
            self.observe_closure(now);
            self.fail(
                FailureClass::RecordFailed,
                "an evidence record could not be made durable",
                now,
            );
            if self.phase == RunPhase::Running && self.case.is_none() {
                self.stop(now);
            }
        }
        self.publish();
        outcome
    }

    fn advance(&mut self, now: Tick) -> Result<(), Refusal> {
        if now < self.clock {
            return Err(Refusal::ClockRegression);
        }
        self.clock = now;
        Ok(())
    }

    fn fail(&mut self, class: FailureClass, detail: &str, now: Tick) {
        let case = self.case.as_mut().map(|case| {
            case.failed = true;
            case.id
        });
        let detail = bounded(detail, self.config.detail_chars);
        self.failures.push(Failure {
            class,
            case,
            at: now,
            detail,
        });
        self.verdict = Verdict::Failed;
    }

    /// Publish a new snapshot. The lock is held only to swap the pointer.
    fn publish(&mut self) {
        self.published += 1;
        let snapshot = Arc::new(self.build_snapshot());
        let previous = std::mem::replace(&mut *lock(&self.shared.snapshot), snapshot);
        drop(previous);
    }

    fn build_snapshot(&self) -> Snapshot {
        Snapshot {
            generation: self.generation,
            published: self.published,
            at: self.clock,
            phase: self.phase,
            verdict: self.verdict,
            cases_passed: self.cases_passed,
            cases_failed: self.cases_failed,
            case: self.case.as_ref().map(CaseState::view),
            entries: self.group.entries().map(Entry::view).collect(),
            operation: self.group.op.as_ref().map(|op| OpView {
                action: op.action,
                kind: op.kind,
                case: op.case,
                phase: op.phase,
                start: op.start,
                start_acknowledged: self.ledger.is_acknowledged(op.start),
            }),
            lost: self.lost.clone(),
            settling: self.settling.clone(),
            released_waiting: self.released.len(),
            evidence: self.ledger.view(self.config.record_capacity),
            recovery: RecoveryView {
                attempts: self.recovery.attempts,
                budget: self.config.recovery_budget,
                epoch: self.recovery.epoch,
                last: self.recovery.last,
                resolved_by: self.recovery.resolved_by,
            },
            failures: self.failures.details.clone(),
            failure_overflow: self.failures.overflow,
            failure_counts: self.failures.counts,
            incidents: self.incidents.iter().map(Incident::view).collect(),
            busy: self.busy,
            cancel_observed: self.cancel_observed,
        }
    }
}

fn blank_snapshot(generation: Generation, at: Tick, config: &Config) -> Snapshot {
    Snapshot {
        generation,
        published: 0,
        at,
        phase: RunPhase::NotStarted,
        verdict: Verdict::Pending,
        cases_passed: 0,
        cases_failed: 0,
        case: None,
        entries: Vec::new(),
        operation: None,
        lost: Vec::new(),
        settling: Vec::new(),
        released_waiting: 0,
        evidence: EvidenceView {
            capacity: config.record_capacity,
            issued: 0,
            acknowledged: 0,
            reserved: 0,
            control_left: config.control_reserve,
            unsent: 0,
            failed: None,
        },
        recovery: RecoveryView {
            attempts: 0,
            budget: config.recovery_budget,
            epoch: 0,
            last: None,
            resolved_by: None,
        },
        failures: Vec::new(),
        failure_overflow: 0,
        failure_counts: [0; 9],
        incidents: Vec::new(),
        busy: None,
        cancel_observed: None,
    }
}

/// `text` made safe and short: control characters (and quotes and
/// backslashes) escaped, at most `limit` characters.
fn bounded(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for (count, c) in text.chars().flat_map(char::escape_debug).enumerate() {
        if count == limit {
            out.push_str("...");
            break;
        }
        out.push(c);
    }
    out
}

fn record_digest(id: RecordId, at: Tick, kind: &RecordKind) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"nexus-phase2-custody-record\0");
    hasher.update(id.generation.bytes());
    hasher.update(id.seq.to_be_bytes());
    hasher.update(at.0.to_be_bytes());
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.finalize().into()
}

fn request_digest(request: &Request) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"nexus-phase2-custody-request\0");
    hasher.update(request.generation.bytes());
    hasher.update(request.seq.to_be_bytes());
    hasher.update(format!("{:?}", request.op).as_bytes());
    hasher.finalize().into()
}
