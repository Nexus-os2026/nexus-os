//! The custody core: one execution owner's custody of native owners, native
//! operations, evidence and failures, and the control side's view of it.
//!
//! [`Custody`] lives with the execution owner (one thread) and is never
//! shared. It holds every native owner, as a value, from the moment a
//! completion deposits it until its end is confirmed, and then hands it back
//! ([`Custody::take_released`]). [`Control`] is the only shared part: the
//! admission gate, the lease, the request queue (one slot) and the last
//! published [`Snapshot`], each under its own lock, never two at once, and
//! none held across a callback, a cleanup attempt, a record submission or a
//! wait. Reading status renews nothing.
//!
//! Admission. One native action runs through [`Custody::reserve`] (capacity
//! and evidence space reserved, its start record issued), the recorder's
//! acknowledgement of that start record, and [`Custody::admit`]: the
//! admission linearization point, one critical section on the gate, decides
//! it against any closure. The gate closes for good on a control-side
//! cancellation or on the run's first actual failure (fail-stop), whatever
//! reported it, including an expected condition that can no longer be met;
//! nothing reopens it, and a closed gate also ends the run's cases.
//! Operations admitted before the closure may still deliver owners, which
//! are adopted for cleanup.
//!
//! Finalization. A run whose native state is resolved becomes a completion
//! candidate. Its terminal commitment is made only once every earlier record
//! is acknowledged, in one critical section on the gate: the commitment
//! point, after which the run's outcome is immutable. A cancellation (an
//! explicit one, an observed lease expiry) accepted before it is part of
//! that outcome: the run cannot pass. One after it closes nothing and changes
//! nothing, and its receipt says it came too late. The terminal record (the
//! run's verdict) is issued at the commitment, right after the record of a
//! closure first observed there, and the run is final only when that terminal
//! record is acknowledged; no pass is published before. From the commitment
//! on, the run's verdict and failures are fixed: a later failure is a fault of
//! this custody, and a later owner a late incident with its own identity and
//! records. The custody closes only when nothing is held or pending and every
//! record is acknowledged: failed evidence keeps it open, with no exception.
//!
//! Limits. A callback's unwinding panic is contained at the callback (the
//! owner stays held), and so is a panic in dropping its payload; an abort, a
//! panic while panicking, a payload whose drop panics again, stack overflow,
//! allocation failure and the destruction of this process are not. Owners are
//! lent by `&mut`, which does not stop a callback from replacing or altering
//! one; custody cannot detect that (an integration obligation).

use std::collections::VecDeque;
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard};

use super::codec;
use super::model::*;

/// An owner's own declaration of the slot it occupies. Custody asks it with
/// the owner borrowed: if the declaration panics, the owner is still held
/// (with an unknown kind, treated as a native process tree).
pub trait Resource {
    fn kind(&self) -> SlotKind;
}

/// One bounded cleanup attempt on a held owner.
///
/// The owner is lent by `&mut`: custody keeps the value, and a panic inside
/// an attempt unwinds through the attempt's frames only. A mutable borrow
/// does not stop the attempt from replacing or altering the owner (with
/// `mem::replace`, `mem::swap` or the owner's own methods); custody cannot
/// detect that, and an attempt must not do it. The report states each fact
/// separately, and the core confirms nothing the report does not establish.
/// An attempt must be bounded: the control side stays responsive while it
/// runs, the execution owner does not.
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
/// layer), asked about exactly one incident binding before the run starts.
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
/// must carry. Not `Clone`, and never constructed outside the core. It is
/// borrowed, not consumed, by [`Custody::complete`]: an unknown result keeps
/// it valid, and a result delivered after the operation retired (a duplicate,
/// or a late one) still needs a binding to be held under.
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
    /// Held in its slot, under its operation's records.
    Deposited(EntryId),
    /// Held under the open operation's own records where custody did not
    /// expect an owner: a failure, never a dropped owner.
    Unexpected(EntryId),
    /// Delivered for an operation already retired: held as a late incident
    /// with its own identity and records, never under the retired action's.
    Late {
        incident: IncidentId,
        entry: EntryId,
    },
    /// The operation ended natively, its output complete.
    Ended,
    /// The operation ended natively, but its output was detached: output
    /// loss (a failure unless its case declared it).
    OutputLost,
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
    /// The operation is already retired and the result carries no owner.
    Stale,
    /// A no-effect proof bound to another operation.
    UnboundProof,
    /// A late owner custody cannot hold: as many late incidents as it may
    /// are held, or their evidence cannot be reserved. It is returned to its
    /// caller, never dropped. Its arrival is still a failure (a fault after
    /// the terminal record), but no record identifies it: the integration
    /// must keep it and dispose of it outside this custody.
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
    /// The borrower panicked: an actual failure; the owner is still held.
    Panicked,
    ClockRegression,
    /// Lending serves a run that is still running or recovering.
    Phase(RunPhase),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushError {
    ClockRegression,
    /// The sink panicked (an actual failure); the record it was given is
    /// still unsent.
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

/// A closed custody: the outcome its acknowledged evidence records, and
/// every owner still awaiting its caller.
pub struct Closed<R> {
    /// The run's acknowledged terminal record (none for a custody whose run
    /// never started).
    pub terminal: Option<RecordId>,
    /// The verdict that terminal record carries (`Pending` without one).
    pub verdict: Verdict,
    pub resolved_by: Option<u32>,
    /// Records issued in all, every one acknowledged.
    pub records: u64,
    pub failures: Vec<Failure>,
    pub failure_counts: [u32; FailureClass::COUNT],
    pub failure_overflow: u32,
    /// Faults after the terminal record (late incidents, evidence faults):
    /// recorded, never part of the run's verdict.
    pub faults: Vec<Failure>,
    pub fault_counts: [u32; FailureClass::COUNT],
    pub fault_overflow: u32,
    pub released: Vec<Released<R>>,
}

impl<R> fmt::Debug for Closed<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closed")
            .field("terminal", &self.terminal)
            .field("verdict", &self.verdict)
            .field("resolved_by", &self.resolved_by)
            .field("records", &self.records)
            .field("failure_counts", &self.failure_counts)
            .field("fault_counts", &self.fault_counts)
            .field("released", &self.released.len())
            .finish_non_exhaustive()
    }
}

/// What a cancellation came to, decided in the gate's critical section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReceipt {
    /// It closed admission, before the terminal commitment: the run cannot
    /// pass.
    Accepted(Closure),
    /// Admission was already closed (by an earlier cancellation or an actual
    /// failure), before the commitment: this request changed nothing, and the
    /// run cannot pass either way.
    AlreadyClosed(Closure),
    /// The run's outcome was already committed: this request closed nothing,
    /// changed nothing and did not cancel the run. (The outcome is published
    /// only with its acknowledged terminal record, never here.)
    Late(Commitment),
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run one adapter or owner callback, containing an unwinding panic in it:
/// only the callback's frames unwind, never the core's. The panic's payload
/// is dropped under containment too (a payload whose drop panics again is
/// beyond it).
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
    committed: Option<Commitment>,
}

struct Lease {
    id: Option<u64>,
    state: LeaseState,
    renewals: u64,
    last: Tick,
}

/// Fixture only: a rendezvous at one point of one custody's commitment
/// primitive ([`Shared::commit`]). Reaching it, the commitment tells the
/// fixture and waits, at most [`BOUNDARY_WATCHDOG`], for the fixture's
/// release; it calls nothing else and hands nothing out. Never set by
/// [`Custody::new`] (see `Custody::install_commit_boundary`).
#[cfg(test)]
struct CommitBoundary {
    point: CommitPoint,
    reached: std::sync::mpsc::SyncSender<CommitPoint>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
    /// The release did not come in time: a fixture failure, never evidence.
    missed: std::sync::atomic::AtomicBool,
}

/// Fixture only: where a commitment boundary sits.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitPoint {
    /// The commitment was called; it holds nothing yet.
    Entry,
    /// It holds the gate and has read the closure that decides its outcome;
    /// the commitment is not recorded yet.
    Decided,
}

/// Fixture only: how long a boundary waits for its release.
#[cfg(test)]
const BOUNDARY_WATCHDOG: std::time::Duration = std::time::Duration::from_secs(30);

/// The only state shared with the control side. Each lock guards one field,
/// is taken alone, and is held only for a copy or an assignment.
struct Shared {
    generation: Generation,
    lease_millis: u64,
    gate: Mutex<Gate>,
    lease: Mutex<Lease>,
    snapshot: Mutex<Arc<Snapshot>>,
    queue: Mutex<Option<Request>>,
    /// Fixture only: the commitment primitive's boundary, if one was
    /// installed before anything else held this state.
    #[cfg(test)]
    commit_boundary: Option<CommitBoundary>,
}

impl Shared {
    /// Close admission before the terminal commitment, once: a later request
    /// returns the first closure. After the commitment nothing closes: the
    /// request is late, and its receipt says so.
    fn close(&self, reason: ClosureReason, at: Tick) -> CancelReceipt {
        let mut gate = lock(&self.gate);
        if let Some(commitment) = gate.committed {
            return CancelReceipt::Late(commitment);
        }
        match gate.closure {
            Some(closure) => CancelReceipt::AlreadyClosed(closure),
            None => {
                let closure = Closure {
                    reason,
                    at,
                    after: gate.admitted,
                };
                gate.closure = Some(closure);
                CancelReceipt::Accepted(closure)
            }
        }
    }

    /// The terminal commitment point, in the gate's own critical section (the
    /// one every closure takes): the closure made before it, if any, is part
    /// of the outcome it fixes, and none can be made after it. Made once.
    /// The closure that decides the outcome is read, and the commitment
    /// recorded, under one guard. (The fixture points, compiled for tests
    /// only, do nothing unless a boundary was installed.)
    fn commit(&self, at: Tick) -> Option<Closure> {
        #[cfg(test)]
        self.reach(CommitPoint::Entry);
        let mut gate = lock(&self.gate);
        let closure = gate.closure;
        #[cfg(test)]
        self.reach(CommitPoint::Decided);
        gate.committed.get_or_insert(Commitment { at });
        closure
    }

    fn closure(&self) -> Option<Closure> {
        lock(&self.gate).closure
    }

    /// Fixture only: meet the fixture at `point` if this custody's boundary
    /// sits there (tell it, then wait for its release, bounded); otherwise
    /// nothing.
    #[cfg(test)]
    fn reach(&self, point: CommitPoint) {
        let Some(boundary) = self
            .commit_boundary
            .as_ref()
            .filter(|boundary| boundary.point == point)
        else {
            return;
        };
        let released = boundary.reached.send(point).is_ok()
            && lock(&boundary.release)
                .recv_timeout(BOUNDARY_WATCHDOG)
                .is_ok();
        if !released {
            boundary
                .missed
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
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
                committed: gate.committed,
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

    /// Accept cancellation before the run's terminal commitment: admission
    /// closes at once, for good, and the run cannot pass. It does not claim
    /// that anything in flight has ended. After the commitment it is late: it
    /// changes nothing, and the receipt says so.
    pub fn cancel(&self, reason: CancelReason, now: Tick) -> CancelReceipt {
        self.shared.close(ClosureReason::Cancelled(reason), now)
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
        self.shared
            .close(ClosureReason::Cancelled(CancelReason::LeaseLost), now);
        Err(LeaseError::Expired)
    }

    /// Expire the lease if its deadline has passed: losing it closes
    /// admission (a cancellation, so only before the terminal commitment).
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
            self.shared
                .close(ClosureReason::Cancelled(CancelReason::LeaseLost), now);
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
    Late(usize),
}

fn slot_of(kind: SlotKind) -> SlotRef {
    match kind {
        SlotKind::Process => SlotRef::Process,
        SlotKind::Workspace => SlotRef::Workspace,
        SlotKind::Fixture => SlotRef::Fixture,
    }
}

/// An action's failure record: still reserved, or issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureRecord {
    Reserved,
    Issued,
}

/// What an entry is held under, and so which records settle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hold {
    /// An action's own owner: the action's failure record (still reserved, or
    /// issued) and its settlement record (reserved).
    Action { failure: FailureRecord },
    /// A late incident: its opening record (issued) and its settlement
    /// record (reserved).
    Incident { incident: IncidentId },
}

/// Where a new entry comes from and what it is held under.
struct Binding {
    action: ActionId,
    case: Option<CaseId>,
    kind: SlotKind,
    origin: Origin,
    hold: Hold,
}

struct Entry<R> {
    id: EntryId,
    action: ActionId,
    case: Option<CaseId>,
    kind: SlotKind,
    origin: Origin,
    hold: Hold,
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
    unresolved_noted: bool,
}

impl<R> Entry<R> {
    fn new(id: EntryId, binding: Binding, owner: R, since: Tick) -> Self {
        Self {
            id,
            action: binding.action,
            case: binding.case,
            kind: binding.kind,
            origin: binding.origin,
            hold: binding.hold,
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
    /// end, and for a process tree its output settled (complete or detached).
    fn finished(&self) -> bool {
        match self.kind {
            SlotKind::Process => self.natively_ended() && self.output.settled(),
            SlotKind::Workspace | SlotKind::Fixture => self.natively_ended(),
        }
    }

    /// Every fact confirmed: the native end, and for a process tree complete
    /// output (a detached output is not).
    fn confirmed(&self) -> bool {
        match self.kind {
            SlotKind::Process => self.natively_ended() && self.output.complete(),
            SlotKind::Workspace | SlotKind::Fixture => self.natively_ended(),
        }
    }

    /// How a finished entry settled.
    fn settlement(&self) -> Settlement {
        if matches!(self.output, OutputFact::Detached(_)) {
            Settlement::OutputLost
        } else {
            Settlement::Confirmed
        }
    }

    fn incident(&self) -> Option<IncidentId> {
        match self.hold {
            Hold::Incident { incident } => Some(incident),
            Hold::Action { .. } => None,
        }
    }

    /// Apply the facts one attempt established, and say whether it detached
    /// the output. A fact is confirmed once and never revoked, and no fact
    /// implies another.
    fn apply(&mut self, report: &CleanupReport, now: Tick) -> bool {
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
                        OutputObserved::Detached => {
                            self.output = OutputFact::Detached(now);
                            return true;
                        }
                        OutputObserved::NotLooked | OutputObserved::StillPending => {}
                    }
                }
            }
            SlotKind::Workspace | SlotKind::Fixture => confirm(&mut self.removed, report.removed),
        }
        false
    }

    fn view(&self) -> EntryView {
        EntryView {
            entry: self.id,
            action: self.action,
            incident: self.incident(),
            case: self.case,
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
            failure_recorded: self.hold
                == Hold::Action {
                    failure: FailureRecord::Issued,
                },
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
/// uses, an open operation's unexpected owner, late incidents, and the one
/// open operation.
struct Group<R> {
    process: Option<Entry<R>>,
    workspace: Option<Entry<R>>,
    fixture: Option<Entry<R>>,
    unexpected: Vec<Entry<R>>,
    late: Vec<Entry<R>>,
    op: Option<Op>,
}

impl<R> Group<R> {
    fn new() -> Self {
        Self {
            process: None,
            workspace: None,
            fixture: None,
            unexpected: Vec::new(),
            late: Vec::new(),
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
            .chain(self.late.iter())
    }

    fn find(&self, id: EntryId) -> Option<SlotRef> {
        let at = |slot: &Option<Entry<R>>| slot.as_ref().is_some_and(|entry| entry.id == id);
        if at(&self.process) {
            Some(SlotRef::Process)
        } else if at(&self.workspace) {
            Some(SlotRef::Workspace)
        } else if at(&self.fixture) {
            Some(SlotRef::Fixture)
        } else if let Some(index) = self.unexpected.iter().position(|entry| entry.id == id) {
            Some(SlotRef::Unexpected(index))
        } else {
            self.late
                .iter()
                .position(|entry| entry.id == id)
                .map(SlotRef::Late)
        }
    }

    fn entry(&self, slot: SlotRef) -> Option<&Entry<R>> {
        match slot {
            SlotRef::Process => self.process.as_ref(),
            SlotRef::Workspace => self.workspace.as_ref(),
            SlotRef::Fixture => self.fixture.as_ref(),
            SlotRef::Unexpected(index) => self.unexpected.get(index),
            SlotRef::Late(index) => self.late.get(index),
        }
    }

    fn entry_mut(&mut self, slot: SlotRef) -> Option<&mut Entry<R>> {
        match slot {
            SlotRef::Process => self.process.as_mut(),
            SlotRef::Workspace => self.workspace.as_mut(),
            SlotRef::Fixture => self.fixture.as_mut(),
            SlotRef::Unexpected(index) => self.unexpected.get_mut(index),
            SlotRef::Late(index) => self.late.get_mut(index),
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
            SlotRef::Late(index) => (index < self.late.len()).then(|| self.late.remove(index)),
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
    /// entry awaiting its confirmed end, an unexpected owner or a late
    /// incident.
    fn awaiting_cleanup(&self) -> bool {
        !self.unexpected.is_empty()
            || !self.late.is_empty()
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
        let digest = codec::record_digest(id, at, &kind);
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
    counts: [u32; FailureClass::COUNT],
}

impl FailureLog {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            details: Vec::new(),
            overflow: 0,
            counts: [0; FailureClass::COUNT],
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
    /// The expected condition can no longer be met (its failure recorded).
    unmet: bool,
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

struct Prior {
    binding: IncidentBinding,
    outcome: PriorOutcome,
    disposition: Option<DispositionReason>,
}

impl Prior {
    /// An unresolved or malformed prior incident blocks the run until an
    /// external validator dispositions it.
    fn blocks(&self) -> bool {
        self.outcome != PriorOutcome::Resolved && self.disposition.is_none()
    }

    fn view(&self) -> PriorView {
        PriorView {
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

/// The run's own records still reserved: its end (the terminal record),
/// recovery-required, and one per explicit recovery attempt it may still
/// make (the run's or its late incidents').
struct RunRecords {
    ended: bool,
    recovery: bool,
    retries: u32,
}

/// The issued terminal record and the verdict fixed in it.
#[derive(Debug, Clone, Copy)]
struct Terminal {
    record: RecordId,
    verdict: Verdict,
}

/// One execution owner's custody. Not shared; see the module documentation.
pub struct Custody<R> {
    config: Config,
    generation: Generation,
    shared: Arc<Shared>,
    clock: Tick,
    phase: RunPhase,
    next_action: u64,
    next_entry: u64,
    next_incident: u64,
    case: Option<CaseState>,
    cases_passed: u32,
    cases_failed: u32,
    group: Group<R>,
    lost: Vec<ActionId>,
    released: Vec<Released<R>>,
    settling: Vec<SettlingView>,
    ledger: Ledger,
    failures: FailureLog,
    faults: FailureLog,
    recovery: RecoveryState,
    requests: RequestGate,
    prior: Vec<Prior>,
    run_records: RunRecords,
    terminal: Option<Terminal>,
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
            .field("terminal", &self.terminal)
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
                committed: None,
            }),
            lease: Mutex::new(Lease {
                id: None,
                state: LeaseState::NotIssued,
                renewals: 0,
                last: now,
            }),
            snapshot: Mutex::new(Arc::new(blank_snapshot(generation, now, &config))),
            queue: Mutex::new(None),
            #[cfg(test)]
            commit_boundary: None,
        });
        let mut custody = Custody {
            config,
            generation,
            shared: Arc::clone(&shared),
            clock: now,
            phase: RunPhase::NotStarted,
            next_action: 1,
            next_entry: 1,
            next_incident: 1,
            case: None,
            cases_passed: 0,
            cases_failed: 0,
            group: Group::new(),
            lost: Vec::new(),
            released: Vec::new(),
            settling: Vec::new(),
            ledger: Ledger::new(generation, &config),
            failures: FailureLog::new(config.failure_detail_limit),
            faults: FailureLog::new(config.failure_detail_limit),
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
            prior: prior
                .into_iter()
                .map(|incident| Prior {
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
            terminal: None,
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

    /// Fixture only: install a commitment boundary, possible only while this
    /// custody alone holds its shared state (no control handle exists), so it
    /// can never be put into a custody the control side already uses. Refused
    /// otherwise, with the boundary handed back.
    #[cfg(test)]
    fn install_commit_boundary(&mut self, boundary: CommitBoundary) -> Result<(), CommitBoundary> {
        match Arc::get_mut(&mut self.shared) {
            Some(shared) => {
                shared.commit_boundary = Some(boundary);
                Ok(())
            }
            None => Err(boundary),
        }
    }

    /// Before the run starts: ask the validator about one prior incident.
    /// Only a disposition bound to exactly that incident is accepted; the
    /// incident's own outcome is kept beside it.
    pub fn apply_disposition<V: DispositionValidator>(
        &mut self,
        binding: &IncidentBinding,
        validator: &V,
        now: Tick,
    ) -> Result<(), Refusal> {
        self.advance(now)?;
        if self.phase != RunPhase::NotStarted {
            self.publish();
            return Err(Refusal::Phase(self.phase));
        }
        let Some(index) = self
            .prior
            .iter()
            .position(|prior| prior.binding == *binding && prior.blocks())
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
        // A panicking validator validates nothing: the incident still blocks.
        let validated = contained(|| validator.validate(binding)).flatten();
        self.busy = None;
        let result = match validated {
            Some(disposition) if disposition.binding == *binding => {
                self.prior[index].disposition = Some(disposition.reason);
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
        if self.prior.iter().any(Prior::blocks) {
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
            .prior
            .iter()
            .filter(|prior| prior.disposition.is_some())
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

    /// Begin a case: only while running, with nothing halting the run (no
    /// cancellation and no actual failure), recording intact, nothing of an
    /// earlier case unresolved and every earlier record acknowledged.
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
        // A halted run never begins another case, however clean its last one
        // ended.
        if let Some(closure) = self.halted(now) {
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
        if self.ledger.unacknowledged() > 0 {
            return Err(Refusal::EvidencePending);
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
            unmet: false,
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

    /// Report an admitted operation's result, bound to its ticket, in any
    /// phase. An owner it brings is held from here on (even after
    /// cancellation, a failure or the terminal record); an owner custody
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
        self.settle_idle(now);
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
            // The operation is retired (action numbers are never reused and
            // only one is open): a late or duplicate result acts on nothing,
            // and an owner it brings becomes a late incident.
            return match outcome.into_owner() {
                Some(owner) => self.retain_late(ticket.action, owner, now),
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
                incident: None,
                record,
                how,
            });
        }
        match how {
            Settlement::Confirmed => Completion::Ended,
            Settlement::OutputLost => Completion::OutputLost,
            Settlement::NothingCreated | Settlement::NotAdmitted => Completion::NoEffect,
        }
    }

    /// An end without an owner is confirmed only by its facts (a detached
    /// output is output loss, recorded as such); without them, native
    /// authority is lost: never a clean end, and never resolvable here.
    fn resolve_ended(&mut self, facts: EndFacts, now: Tick) -> Completion {
        let Some((kind, case)) = self.group.op.as_ref().map(|op| (op.kind, op.case)) else {
            return Completion::StillUnknown;
        };
        let output = matches!(
            facts.output,
            OutputObserved::Complete | OutputObserved::Detached
        );
        let ended = match kind {
            SlotKind::Process => facts.subtree && facts.reaped && output,
            SlotKind::Workspace | SlotKind::Fixture => facts.removed,
        };
        if ended {
            let lost = kind == SlotKind::Process && facts.output == OutputObserved::Detached;
            let how = if lost {
                Settlement::OutputLost
            } else {
                Settlement::Confirmed
            };
            let completion = self.resolve_without_owner(how, now);
            if lost {
                self.note_output_lost(Some(case), now);
            }
            return completion;
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
    /// or for a slot already held, is held as unexpected under the
    /// operation's own records (a failure), never dropped and never put in
    /// place of the held one. The owner is stored before any bookkeeping.
    fn deposit(
        &mut self,
        ticket: &OpTicket,
        owner: R,
        origin: Origin,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        let Some(op) = self.group.op.take() else {
            return self.retain_late(ticket.action, owner, now);
        };
        // The owner's own declaration, asked with the owner borrowed.
        let declared = contained(|| owner.kind());
        let id = self.new_entry_id();
        let fits = declared == Some(op.kind) && self.group.slot(op.kind).is_none();
        let binding = Binding {
            action: op.action,
            case: Some(op.case),
            kind: if fits {
                op.kind
            } else {
                declared.unwrap_or(SlotKind::Process)
            },
            origin: if fits { origin } else { Origin::Unexpected },
            hold: Hold::Action {
                failure: op.failure,
            },
        };
        let mut entry = Entry::new(id, binding, owner, now);
        if !fits {
            self.group.unexpected.push(entry);
            let slot = SlotRef::Unexpected(self.group.unexpected.len() - 1);
            self.issue_entry_failure(slot, now);
            self.fail(
                FailureClass::UnexpectedOwner,
                "an operation returned an owner custody did not reserve a slot for",
                now,
            );
            return Ok(Completion::Unexpected(id));
        }
        if origin == Origin::Created && self.case.is_some() && self.shared.closure().is_none() {
            entry.phase = EntryPhase::Live;
        }
        let expected = origin == Origin::Retained
            && op.kind == SlotKind::Process
            && self.case.as_ref().is_some_and(|case| {
                case.id == op.case
                    && case.expectation == Expectation::RetainedBoundary
                    && case.retained_entry.is_none()
            });
        *self.group.slot_mut(op.kind) = Some(entry);
        if origin == Origin::Retained {
            match self.case.as_mut() {
                Some(case) if expected => {
                    case.expected_observed = true;
                    case.retained_entry = Some(id);
                }
                _ => {
                    self.issue_entry_failure(slot_of(op.kind), now);
                    self.fail(
                        FailureClass::UnexpectedRetained,
                        "an operation's own cleanup was unconfirmed where none was expected",
                        now,
                    );
                }
            }
        }
        Ok(Completion::Deposited(id))
    }

    /// An owner delivered for an operation already retired: a late incident
    /// with its own identity and records (never the retired action's), held
    /// while custody may hold another and can reserve its evidence, otherwise
    /// returned to the caller. Either way it is a failure: before the terminal
    /// record an actual failure of the run (and a completion candidate holding
    /// it requires recovery again); after it, a fault of this custody that
    /// leaves the run's outcome as recorded.
    fn retain_late(
        &mut self,
        action: ActionId,
        owner: R,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        // A candidate's recovery-required record, if its own was used.
        let recovery = self.phase == RunPhase::Candidate && !self.run_records.recovery;
        let records = 2 + u32::from(recovery);
        if self.group.late.len() >= self.config.late_limit || !self.ledger.reserve(records) {
            self.fail(
                FailureClass::LateOwner,
                "an operation already retired delivered another owner; returned, not held",
                now,
            );
            return Err(Rejected::CustodyFull { owner });
        }
        if recovery {
            self.run_records.recovery = true;
        }
        let declared = contained(|| owner.kind());
        let incident = IncidentId {
            generation: self.generation,
            seq: self.next_incident,
        };
        self.next_incident += 1;
        let id = self.new_entry_id();
        let binding = Binding {
            action,
            case: None,
            kind: declared.unwrap_or(SlotKind::Process),
            origin: Origin::Late,
            hold: Hold::Incident { incident },
        };
        self.group.late.push(Entry::new(id, binding, owner, now));
        self.ledger.issue_reserved(
            now,
            RecordKind::IncidentOpened {
                incident,
                action,
                kind: declared,
            },
        );
        self.fail(
            FailureClass::LateOwner,
            "an operation already retired delivered another owner",
            now,
        );
        if self.phase == RunPhase::Candidate {
            // No terminal record yet: the run holds an owner again.
            self.enter_recovery(now);
        }
        Ok(Completion::Late {
            incident,
            entry: id,
        })
    }

    /// Issue an action's failure record from its reservation, once.
    fn issue_failure(&mut self, record: &mut FailureRecord, action: ActionId, now: Tick) {
        if *record == FailureRecord::Reserved {
            *record = FailureRecord::Issued;
            self.ledger
                .issue_reserved(now, RecordKind::ActionFailed { action });
        }
    }

    /// Issue a held entry's action failure record, once (a late incident's
    /// own records tell its story instead).
    fn issue_entry_failure(&mut self, slot: SlotRef, now: Tick) {
        let reserved = self.group.entry(slot).and_then(|entry| match entry.hold {
            Hold::Action {
                failure: FailureRecord::Reserved,
            } => Some(entry.action),
            Hold::Action { .. } | Hold::Incident { .. } => None,
        });
        if let Some(action) = reserved {
            self.ledger
                .issue_reserved(now, RecordKind::ActionFailed { action });
            if let Some(entry) = self.group.entry_mut(slot) {
                entry.hold = Hold::Action {
                    failure: FailureRecord::Issued,
                };
            }
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

    /// Lend a held owner to the run's own code (for evidence), while the run
    /// is running or recovering. The owner stays in custody whatever `f`
    /// does, but `f` must not replace it (see [`Cleanup`]); a panic in `f` is
    /// contained and is an actual failure.
    pub fn lend<T>(
        &mut self,
        entry: EntryId,
        now: Tick,
        f: impl FnOnce(&mut R) -> T,
    ) -> Result<T, LendError> {
        if self.advance(now).is_err() {
            return Err(LendError::ClockRegression);
        }
        if !matches!(self.phase, RunPhase::Running | RunPhase::RecoveryRequired) {
            return Err(LendError::Phase(self.phase));
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
        let lent = self
            .group
            .entry_mut(slot)
            .map(|held| contained(|| f(&mut held.owner)));
        self.busy = None;
        let result = match lent {
            Some(Some(value)) => Ok(value),
            Some(None) => {
                self.fail(
                    FailureClass::Assertion,
                    "a borrower panicked; its owner is still held",
                    now,
                );
                Err(LendError::Panicked)
            }
            None => Err(LendError::UnknownEntry),
        };
        self.settle_idle(now);
        self.publish();
        result
    }

    /// Record an actual failed assertion or observation. Only while the
    /// run's outcome is open (running, recovering or a completion candidate);
    /// once the terminal record is issued it is refused and changes nothing.
    pub fn record_assertion_failure(&mut self, detail: &str, now: Tick) -> Result<(), Refusal> {
        self.advance(now)?;
        if !matches!(
            self.phase,
            RunPhase::Running | RunPhase::RecoveryRequired | RunPhase::Candidate
        ) {
            self.publish();
            return Err(Refusal::Phase(self.phase));
        }
        self.fail(FailureClass::Assertion, detail, now);
        self.settle_idle(now);
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
    /// run stops if anything stays unresolved, the run is halted (cancelled,
    /// or failed by this or an earlier failure) or recording failed.
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
        if self.phase != RunPhase::Running {
            return Err(Refusal::Phase(self.phase));
        }
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
        let halted = self.halted(now).is_some();
        let stopped = !resolved || halted || self.ledger.failed.is_some();
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
    /// issued (if it has one and it was not already) and the failure counted,
    /// once.
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
        self.issue_entry_failure(slot, now);
        self.fail(
            FailureClass::UnexpectedCleanup,
            "a required cleanup attempt did not confirm its end",
            now,
        );
    }

    /// A process tree's output was detached instead of drained: output loss,
    /// an actual failure unless its own case declared exactly that (once).
    fn note_output_lost(&mut self, case: Option<CaseId>, now: Tick) {
        let declared = match self.case.as_mut() {
            Some(active)
                if Some(active.id) == case
                    && active.expectation == Expectation::OutputDetached
                    && !active.expected_observed =>
            {
                active.expected_observed = true;
                true
            }
            _ => false,
        };
        if !declared {
            self.fail(
                FailureClass::OutputLost,
                "a process tree's output was detached instead of drained",
                now,
            );
        }
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
        match expectation {
            Expectation::Clean => {}
            Expectation::RetainedBoundary if !observed => {
                self.expectation_unmet("the expected retained boundary was not observed", now)
            }
            Expectation::RetainedBoundary if first != Some(true) => self.expectation_unmet(
                "the retained boundary was not confirmed by its first cleanup attempt",
                now,
            ),
            Expectation::RetainedBoundary => {}
            Expectation::OutputDetached if !observed => {
                self.expectation_unmet("the declared output detachment was not observed", now)
            }
            Expectation::OutputDetached => {}
        }
        let failed = self.case.as_ref().is_some_and(|case| case.failed);
        !failed && !self.unresolved()
    }

    /// The active case's expected condition can no longer be met: an actual
    /// failure, recorded once by whichever point recognizes it first (a
    /// retained boundary's first cleanup attempt, or the case's end), which
    /// closes execution admission at once (fail-stop).
    fn expectation_unmet(&mut self, detail: &str, now: Tick) {
        let first = match self.case.as_mut() {
            Some(case) if !case.unmet => {
                case.unmet = true;
                true
            }
            _ => false,
        };
        if first {
            self.fail(FailureClass::ExpectedConditionUnmet, detail, now);
        }
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
    /// (as a control fact), a cancellation outside a case becomes the run's
    /// failure at once (while it recovers or is a completion candidate too),
    /// and an idle running run stops.
    pub fn observe_control(&mut self, now: Tick) -> Result<Option<Closure>, Refusal> {
        self.advance(now)?;
        let closure = self.observe_closure(now);
        self.note_cancellation(closure, now);
        self.settle_idle(now);
        self.publish();
        Ok(closure)
    }

    /// A running run with no case and admission closed (cancelled, or failed
    /// by any entry point) stops at once: fail-stop needs no later call.
    fn settle_idle(&mut self, now: Tick) {
        if self.phase == RunPhase::Running && self.case.is_none() && self.shared.closure().is_some()
        {
            self.stop(now);
        }
    }

    fn observe_closure(&mut self, now: Tick) -> Option<Closure> {
        let closure = self.shared.closure();
        if let Some(closure) = closure {
            self.record_closure(closure, now);
        }
        closure
    }

    /// The closure is recorded once, as a control fact, when the execution
    /// owner first observes it.
    fn record_closure(&mut self, closure: Closure, now: Tick) {
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

    /// An accepted cancellation is a failure of the run, recognized once
    /// (never an actual failure's closure), wherever the execution owner sees
    /// it outside a case before the terminal commitment: when the run stops,
    /// when it observes control while recovering or as a completion
    /// candidate, and at the latest at the commitment, with the closure the
    /// commitment itself saw.
    fn note_cancellation(&mut self, closure: Option<Closure>, now: Tick) {
        let Some(closure) = closure else {
            return;
        };
        let open = matches!(
            self.phase,
            RunPhase::Running | RunPhase::RecoveryRequired | RunPhase::Candidate
        );
        if !open || self.case.is_some() || self.terminal.is_some() {
            return;
        }
        self.record_closure(closure, now);
        if matches!(closure.reason, ClosureReason::Cancelled(_)) && !self.cancel_noted {
            self.cancel_noted = true;
            self.fail(
                FailureClass::Cancelled,
                "the run was cancelled before it finished",
                now,
            );
        }
    }

    /// The closure that halts the run, if any (observed and recorded): a
    /// control-side cancellation or an actual failure. No case begins after
    /// it, and ending a case stops the run on it.
    fn halted(&mut self, now: Tick) -> Option<Closure> {
        self.observe_closure(now)
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
    /// tree held (in its slot, unexpected or late) has ended natively.
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
                .chain(self.group.late.iter())
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
    /// in the adapter is contained, the owner stays held and the panic is an
    /// actual failure. A detached output is output loss.
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
        let (finished, first, detached) = match self.group.entry_mut(slot) {
            Some(entry) => {
                if explicit {
                    entry.explicit = entry.explicit.saturating_add(1);
                } else {
                    entry.automatic = entry.automatic.saturating_add(1);
                }
                entry.phase = EntryPhase::CleanupRequired;
                let detached = entry.apply(&report, now);
                let finished = entry.finished();
                let first = if entry.first.is_none() {
                    let confirmed = entry.confirmed();
                    entry.first = Some(confirmed);
                    Some(confirmed)
                } else {
                    None
                };
                (finished, first, detached)
            }
            None => (false, None, false),
        };
        let mut unmet = false;
        if let Some(confirmed) = first {
            if let Some(case) = self.case.as_mut() {
                if case.retained_entry == Some(view.entry) {
                    case.retained_first = Some(confirmed);
                    unmet = !confirmed;
                }
            }
        }
        if detached {
            self.note_output_lost(view.case, now);
        }
        if panicked {
            self.fail(
                FailureClass::UnexpectedCleanup,
                "a cleanup adapter panicked; its owner is still held",
                now,
            );
        }
        // The expected retained boundary's first attempt did not confirm
        // every fact: the expectation can never be met. It fails now, closing
        // admission before anything else can be admitted; the owner's cleanup
        // goes on within its budget, and a later success changes nothing.
        if unmet {
            self.expectation_unmet(
                "the retained boundary was not confirmed by its first cleanup attempt",
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
    /// caller first, then its settlement record (which says whether its
    /// output was complete or lost) is issued.
    fn finish(&mut self, slot: SlotRef, now: Tick) {
        let Some(entry) = self.group.take(slot) else {
            return;
        };
        let how = entry.settlement();
        let Entry {
            id,
            action,
            owner,
            hold,
            ..
        } = entry;
        // Handed over first; the bookkeeping follows.
        self.released.push(Released { entry: id, owner });
        let (record, incident) = match hold {
            Hold::Action { failure } => {
                let record = self
                    .ledger
                    .issue_reserved(now, RecordKind::ActionSettled { action, how });
                if failure == FailureRecord::Reserved {
                    self.ledger.release(1);
                }
                (record, None)
            }
            Hold::Incident { incident } => (
                self.ledger
                    .issue_reserved(now, RecordKind::IncidentSettled { incident, how }),
                Some(incident),
            ),
        };
        self.settling.push(SettlingView {
            action,
            entry: Some(id),
            incident,
            record,
            how,
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

    /// No further case: the run requires recovery, or becomes a completion
    /// candidate. A control-side cancellation is itself a failure; an actual
    /// failure is never recorded as a cancellation.
    fn stop(&mut self, now: Tick) {
        if self.phase != RunPhase::Running {
            return;
        }
        let closure = self.observe_closure(now);
        self.note_cancellation(closure, now);
        if self.unresolved() {
            self.enter_recovery(now);
        } else {
            self.become_candidate(now, None);
        }
    }

    fn enter_recovery(&mut self, now: Tick) {
        self.phase = RunPhase::RecoveryRequired;
        if self.run_records.recovery {
            self.run_records.recovery = false;
            self.ledger
                .issue_reserved(now, RecordKind::RecoveryRequired);
        }
    }

    /// The run's native state is resolved: it is a completion candidate,
    /// finalized as soon as its evidence allows.
    fn become_candidate(&mut self, now: Tick, resolved_by: Option<u32>) {
        self.recovery.resolved_by = resolved_by;
        self.phase = RunPhase::Candidate;
        self.try_finalize(now);
    }

    /// The terminal commitment of a completion candidate, once every earlier
    /// record is acknowledged (and recording has not failed): no outcome is
    /// decided while required evidence is pending. The commitment point is
    /// one critical section on the gate, the one every closure takes: a
    /// cancellation accepted before it is recorded with the outcome (its
    /// control fact first, if this is its first observation) and fails the
    /// run; none can be accepted after it. The terminal record, issued in the
    /// same step, carries the verdict from here on.
    fn try_finalize(&mut self, now: Tick) {
        if self.phase != RunPhase::Candidate
            || self.ledger.failed.is_some()
            || self.ledger.unacknowledged() > 0
            || self.unresolved()
        {
            return;
        }
        let closure = self.shared.commit(now);
        self.note_cancellation(closure, now);
        let verdict = self.run_verdict();
        if self.run_records.recovery {
            self.run_records.recovery = false;
            self.ledger.release(1);
        }
        self.run_records.ended = false;
        let record = self.ledger.issue_reserved(
            now,
            RecordKind::RunEnded {
                verdict,
                resolved_by: self.recovery.resolved_by,
            },
        );
        self.terminal = Some(Terminal { record, verdict });
        self.phase = RunPhase::Finalizing;
    }

    /// The terminal record is acknowledged: the run is final.
    fn note_finalized(&mut self) {
        let acknowledged = self
            .terminal
            .is_some_and(|terminal| self.ledger.is_acknowledged(terminal.record));
        if self.phase == RunPhase::Finalizing && acknowledged {
            self.phase = RunPhase::Finalized;
        }
    }

    /// The run's verdict as its failures stand.
    fn run_verdict(&self) -> Verdict {
        if self.failures.total() > 0 || self.cases_failed > 0 {
            Verdict::Failed
        } else {
            Verdict::Passed
        }
    }

    /// The published verdict: failed from the first actual failure; passed
    /// only once a terminal record carrying a pass is acknowledged.
    fn verdict(&self) -> Verdict {
        if self.run_verdict() == Verdict::Failed {
            return Verdict::Failed;
        }
        match self.terminal {
            Some(terminal) if self.ledger.is_acknowledged(terminal.record) => terminal.verdict,
            _ => Verdict::Pending,
        }
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
        let digest = codec::request_digest(request);
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

    /// Test observation only: the digest the retained receipt for request
    /// `seq` holds, if one is retained. It reads this custody's real receipt
    /// and changes nothing.
    #[cfg(test)]
    pub(crate) fn retained_request_digest(&self, seq: u64) -> Option<[u8; 32]> {
        self.requests.receipt(seq).map(|receipt| receipt.digest)
    }

    /// One explicit recovery attempt: while the run requires recovery, or
    /// (after its terminal record) while late incidents are held; decided
    /// against the current epoch, within the budget and the spacing, with its
    /// record reserved since the run started. Each held owner gets one
    /// attempt (native trees first). Nothing is removed because a budget is
    /// spent, and no earlier failure is forgotten.
    fn retry<C: Cleanup<R>>(&mut self, epoch: u64, cleanup: &mut C, now: Tick) -> Response {
        let incidents = self.terminal.is_some() && !self.group.late.is_empty();
        if self.phase != RunPhase::RecoveryRequired && !incidents {
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
        if resolved && self.phase == RunPhase::RecoveryRequired {
            self.become_candidate(now, Some(attempt));
        }
        Response::Retried {
            attempt,
            resolved,
            epoch: self.recovery.epoch,
        }
    }

    /// An application shutdown request: never an exit, a release or a drop.
    /// While the run is running it closes admission through the same gate as
    /// any cancellation (necessarily before the commitment; the shutdown waits
    /// until everything is resolved). Otherwise it only decides: it never
    /// commits, finalizes or cancels, and a completion candidate's own
    /// commitment is what it waits for. While anything is unresolved it is
    /// refused, and the first refusal becomes evidence.
    fn shutdown_requested(&mut self, now: Tick) -> ShutdownDecision {
        if self.phase == RunPhase::Running {
            self.shared
                .close(ClosureReason::Cancelled(CancelReason::Shutdown), now);
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

    /// Whether the custody may close now (it changes nothing): only with the
    /// run final (or never started), nothing held or pending, and every
    /// issued record acknowledged. Failed or pending evidence refuses it,
    /// with no exception.
    pub fn shutdown_decision(&self) -> ShutdownDecision {
        let unresolved = self.unresolved_now();
        if unresolved.is_empty() {
            ShutdownDecision::Permitted
        } else {
            ShutdownDecision::Refused(unresolved)
        }
    }

    fn unresolved_now(&self) -> Vec<Unresolved> {
        let mut unresolved = Vec::new();
        match (self.phase, self.terminal) {
            (RunPhase::Running, _) => unresolved.push(Unresolved::RunActive),
            (RunPhase::RecoveryRequired, _) => unresolved.push(Unresolved::RecoveryRequired),
            (RunPhase::Candidate, _) => unresolved.push(Unresolved::RunEnding),
            (RunPhase::Finalizing, Some(terminal)) => {
                unresolved.push(Unresolved::TerminalPending(terminal.record))
            }
            (RunPhase::Finalizing, None) => unresolved.push(Unresolved::RunEnding),
            (RunPhase::NotStarted | RunPhase::Finalized | RunPhase::Closed, _) => {}
        }
        if self.case.is_some() {
            unresolved.push(Unresolved::CaseActive);
        }
        let late = self.group.late.len();
        let held = self.group.held() - late;
        if held > 0 {
            unresolved.push(Unresolved::OwnersHeld(held as u32));
        }
        if late > 0 {
            unresolved.push(Unresolved::LateIncidents(late as u32));
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
        match self.ledger.failed {
            Some((record, at)) => unresolved.push(Unresolved::EvidenceFailed { record, at }),
            None => {
                let pending = self.ledger.unacknowledged();
                if pending > 0 {
                    unresolved.push(Unresolved::EvidencePending(pending));
                }
            }
        }
        unresolved
    }

    /// Close the custody if nothing is unresolved; otherwise the same custody
    /// comes back, unchanged in what it holds. The closed result carries the
    /// acknowledged terminal record and its verdict.
    pub fn close(mut self, now: Tick) -> Result<Closed<R>, Box<Custody<R>>> {
        if self.advance(now).is_err() {
            return Err(Box::new(self));
        }
        if self.shutdown_decision() != ShutdownDecision::Permitted {
            self.publish();
            return Err(Box::new(self));
        }
        let released = std::mem::take(&mut self.released);
        let verdict = self.verdict();
        self.phase = RunPhase::Closed;
        self.publish();
        Ok(Closed {
            terminal: self.terminal.map(|terminal| terminal.record),
            verdict,
            resolved_by: self.recovery.resolved_by,
            records: self.ledger.issued(),
            failures: self.failures.details.clone(),
            failure_counts: self.failures.counts,
            failure_overflow: self.failures.overflow,
            faults: self.faults.details.clone(),
            fault_counts: self.faults.counts,
            fault_overflow: self.faults.overflow,
            released,
        })
    }

    /// Hand the issued, unsent records to the recorder, in order. A record the
    /// sink panics on stays unsent, and the panic is an actual failure.
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
                self.fail(
                    FailureClass::RecorderFault,
                    "the recorder adapter panicked; the record is still unsent",
                    now,
                );
                self.settle_idle(now);
                self.publish();
                return Err(FlushError::SinkPanicked { sent });
            }
            self.ledger.unsent.pop_front();
            sent += 1;
        }
        self.publish();
        Ok(sent)
    }

    /// The recorder's acknowledgement that one record is durable. The last
    /// earlier record's acknowledgement lets a completion candidate issue its
    /// terminal record; the terminal record's makes the run final.
    pub fn acknowledge(&mut self, ack: &RecordAck, now: Tick) -> AckOutcome {
        if self.advance(now).is_err() {
            return AckOutcome::ClockRegression;
        }
        let outcome = self.ledger.acknowledge(ack);
        if outcome == AckOutcome::Acknowledged {
            let ledger = &self.ledger;
            self.settling
                .retain(|settling| !ledger.is_acknowledged(settling.record));
            self.try_finalize(now);
            self.note_finalized();
        }
        self.publish();
        outcome
    }

    /// The recorder's report that the next record could not be made durable:
    /// an actual failure (fail-stop). Recording stays failed: no later
    /// acknowledgement is accepted, no terminal record is issued after it, and
    /// the custody never closes.
    pub fn record_failed(&mut self, id: RecordId, now: Tick) -> AckOutcome {
        if self.advance(now).is_err() {
            return AckOutcome::ClockRegression;
        }
        let outcome = self.ledger.fail(id, now);
        if outcome == AckOutcome::FailureRecorded {
            self.fail(
                FailureClass::RecordFailed,
                "an evidence record could not be made durable",
                now,
            );
            self.settle_idle(now);
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

    /// Record an actual failure, from whichever entry point recognized it,
    /// and close execution admission for good (fail-stop). Before the
    /// terminal record it is the run's failure (and its case's); after it,
    /// the run's outcome is fixed and the failure is a fault of this custody
    /// (the commitment left nothing to close).
    fn fail(&mut self, class: FailureClass, detail: &str, now: Tick) {
        let detail = bounded(detail, self.config.detail_chars);
        if self.terminal.is_some() {
            self.faults.push(Failure {
                class,
                case: None,
                at: now,
                detail,
            });
        } else {
            let case = self.case.as_mut().map(|case| {
                case.failed = true;
                case.id
            });
            self.failures.push(Failure {
                class,
                case,
                at: now,
                detail,
            });
        }
        self.shared.close(ClosureReason::Failed(class), now);
        self.observe_closure(now);
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
            verdict: self.verdict(),
            terminal: self.terminal.map(|terminal| TerminalView {
                record: terminal.record,
                verdict: terminal.verdict,
                acknowledged: self.ledger.is_acknowledged(terminal.record),
            }),
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
            faults: self.faults.details.clone(),
            fault_overflow: self.faults.overflow,
            fault_counts: self.faults.counts,
            prior: self.prior.iter().map(Prior::view).collect(),
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
        terminal: None,
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
        failure_counts: [0; FailureClass::COUNT],
        faults: Vec::new(),
        fault_overflow: 0,
        fault_counts: [0; FailureClass::COUNT],
        prior: Vec::new(),
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

/// R2A: the terminal commitment's atomicity, at the primitive's own
/// boundary. Each control drives a real run through the public API to its
/// last earlier acknowledgement; that `acknowledge`, on the execution owner's
/// thread, reaches `try_finalize` and [`Shared::commit`], where a boundary
/// pauses it at one point while this thread, the control side, looks at the
/// gate without blocking (`try_lock`) and cancels through the real
/// [`Control::cancel`] (so [`Shared::close`]) only when it can take the gate:
/// it never waits for what the commitment excludes. Channels order the two
/// threads, every wait is bounded, and the commitment is always released and
/// joined before anything is asserted. Nothing here imitates the commitment.
#[cfg(test)]
mod commitment_boundary {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::TryLockError;
    use std::thread;

    const GENERATION: Generation = Generation::new([0x5a; 16]);

    /// These runs admit no native action, so no owner of this type exists.
    struct Never;

    impl Resource for Never {
        fn kind(&self) -> SlotKind {
            SlotKind::Process
        }
    }

    struct NoCleanup;

    impl Cleanup<Never> for NoCleanup {
        fn attempt(&mut self, _owner: &mut Never, _entry: &EntryView) -> CleanupReport {
            CleanupReport::NOTHING
        }
    }

    /// The recorder stand-in: it keeps what it is given, in memory.
    #[derive(Default)]
    struct Journal(Vec<RecordIntent>);

    impl RecordSink for Journal {
        fn submit(&mut self, intent: &RecordIntent) {
            self.0.push(intent.clone());
        }
    }

    /// The gate as the control side found it at the boundary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Found {
        /// Held by the commitment: a cancellation could only have waited.
        Held,
        /// Free: the control side could take it, and cancelled.
        Free,
        Poisoned,
        /// The commitment never reached its boundary.
        NotReached,
    }

    /// A boundary at `point`, with the fixture's ends of its channels.
    fn boundary_at(point: CommitPoint) -> (CommitBoundary, Receiver<CommitPoint>, Sender<()>) {
        let (reached, reached_rx) = mpsc::sync_channel(1);
        let (release, release_rx) = mpsc::channel();
        let boundary = CommitBoundary {
            point,
            reached,
            release: Mutex::new(release_rx),
            missed: AtomicBool::new(false),
        };
        (boundary, reached_rx, release)
    }

    fn acknowledge(
        custody: &mut Custody<Never>,
        journal: &Journal,
        index: usize,
        at: u64,
    ) -> AckOutcome {
        custody.acknowledge(&RecordAck::of(&journal.0[index]), Tick(at))
    }

    /// What happened, in order, and what came of it.
    struct Trace {
        point: CommitPoint,
        met: Option<CommitPoint>,
        found: Found,
        at_boundary: Option<CancelReceipt>,
        acknowledged: AckOutcome,
        missed: bool,
        after: CancelReceipt,
        terminal: Option<(u64, Verdict)>,
        published: Verdict,
        failures: Vec<FailureClass>,
        admission: AdmissionView,
    }

    impl Trace {
        fn describe(&self) -> String {
            let at_boundary = match self.at_boundary {
                Some(receipt) => format!("{receipt:?}"),
                None => "not attempted (the gate was not free)".to_string(),
            };
            format!(
                "1. execution owner: acknowledge(last earlier record) -> try_finalize -> \
                 Shared::commit, boundary at {:?}, met: {:?}; 2. control side: \
                 gate.try_lock() -> {:?}; 3. control side: cancel at the boundary -> \
                 {at_boundary}; 4. control side: release; 5. execution owner: acknowledge \
                 -> {:?} (release missed: {}); 6. control side: cancel after the \
                 commitment -> {:?}; outcome: terminal record {:?}, published {:?}, \
                 failures {:?}, admission {:?}",
                self.point,
                self.met,
                self.found,
                self.acknowledged,
                self.missed,
                self.after,
                self.terminal,
                self.published,
                self.failures,
                self.admission
            )
        }

        /// The fixture did its part: the commitment met it where it was set,
        /// the gate was observed, and the release came in time.
        fn fixture_held(&self) -> bool {
            self.met == Some(self.point)
                && matches!(self.found, Found::Held | Found::Free)
                && self.acknowledged == AckOutcome::Acknowledged
                && !self.missed
        }

        /// Every accepted cancellation is in the committed outcome (the run
        /// failed, the cancellation its one failure and the admission closure
        /// its own); with none accepted, the run passed and nothing closed.
        fn consistent(&self) -> bool {
            let accepted = [self.at_boundary, Some(self.after)]
                .into_iter()
                .flatten()
                .find_map(|receipt| match receipt {
                    CancelReceipt::Accepted(closure) => Some(closure),
                    CancelReceipt::AlreadyClosed(_) | CancelReceipt::Late(_) => None,
                });
            let verdict = self.terminal.map(|(_, verdict)| verdict);
            match accepted {
                Some(closure) => {
                    verdict == Some(Verdict::Failed)
                        && self.published == Verdict::Failed
                        && self.failures == vec![FailureClass::Cancelled]
                        && self.admission.closure == Some(closure)
                }
                None => {
                    verdict == Some(Verdict::Passed)
                        && self.published == Verdict::Passed
                        && self.failures.is_empty()
                        && self.admission.closure.is_none()
                }
            }
        }

        /// The cancellation after the commitment was late, naming it.
        fn late_after(&self) -> bool {
            matches!(self.after, CancelReceipt::Late(commitment)
                if self.admission.committed == Some(commitment))
        }
    }

    /// A run with its boundary at `point`, through its terminal commitment
    /// and the acknowledgement of everything it recorded.
    fn run(point: CommitPoint) -> Trace {
        let (boundary, reached, release) = boundary_at(point);
        let (mut custody, control) =
            Custody::<Never>::new(Config::LIVE, GENERATION, Vec::new(), Tick(0))
                .expect("a valid configuration");
        drop(control);
        assert!(
            custody.install_commit_boundary(boundary).is_ok(),
            "nothing else holds the shared state yet"
        );
        let control = custody.control();
        let mut journal = Journal::default();
        custody.start_run(Tick(1)).expect("the run starts");
        custody
            .flush_records(&mut journal, Tick(2))
            .expect("the journal accepts every record");
        assert_eq!(
            acknowledge(&mut custody, &journal, 0, 3),
            AckOutcome::Acknowledged
        );
        custody
            .begin_case(CaseId(1), Expectation::Clean, Tick(4))
            .expect("the case begins");
        assert!(
            custody
                .end_case(&mut NoCleanup, Tick(5))
                .expect("the case ends")
                .passed
        );
        assert_eq!(custody.finish_run(Tick(6)), Ok(RunPhase::Candidate));
        custody
            .flush_records(&mut journal, Tick(7))
            .expect("the journal accepts every record");
        assert_eq!(
            acknowledge(&mut custody, &journal, 1, 8),
            AckOutcome::Acknowledged
        );
        assert_eq!(
            journal.0.len(),
            3,
            "the run's start, its case's start and end"
        );
        assert_eq!(control.status().control.admission.committed, None);
        let last = RecordAck::of(&journal.0[2]);

        // 1. The last earlier acknowledgement, on the execution owner's thread.
        let executor = thread::spawn(move || {
            let acknowledged = custody.acknowledge(&last, Tick(9));
            (custody, acknowledged)
        });
        // 2-4. The control side, at the boundary.
        let met = reached.recv_timeout(BOUNDARY_WATCHDOG).ok();
        let found = match met {
            None => Found::NotReached,
            Some(_) => match control.shared.gate.try_lock() {
                Ok(_) => Found::Free,
                Err(TryLockError::WouldBlock) => Found::Held,
                Err(TryLockError::Poisoned(_)) => Found::Poisoned,
            },
        };
        let at_boundary =
            (found == Found::Free).then(|| control.cancel(CancelReason::Requested, Tick(10)));
        let _ = release.send(());
        // 5. The commitment, completed.
        let (mut custody, acknowledged) = executor.join().expect("the execution owner");
        let missed = control
            .shared
            .commit_boundary
            .as_ref()
            .is_some_and(|boundary| boundary.missed.load(Ordering::SeqCst));
        // 6. A cancellation after it.
        let after = control.cancel(CancelReason::Requested, Tick(11));
        // The remaining evidence, acknowledged: the outcome as published.
        custody
            .flush_records(&mut journal, Tick(12))
            .expect("the journal accepts every record");
        for (index, at) in (3..journal.0.len()).zip(13..) {
            assert_eq!(
                acknowledge(&mut custody, &journal, index, at),
                AckOutcome::Acknowledged
            );
        }
        let snapshot = custody.snapshot();
        Trace {
            point,
            met,
            found,
            at_boundary,
            acknowledged,
            missed,
            after,
            terminal: snapshot
                .terminal
                .map(|terminal| (terminal.record.seq(), terminal.verdict)),
            published: snapshot.verdict,
            failures: snapshot
                .failures
                .iter()
                .map(|failure| failure.class)
                .collect(),
            admission: control.status().control.admission,
        }
    }

    /// Cancellation first, at the latest instant: accepted at the
    /// commitment's entry (`try_finalize` has decided to commit; the
    /// primitive holds nothing yet), it is part of the committed outcome: the
    /// run fails, and a later cancellation is late.
    #[test]
    fn r2a_cancellation_accepted_at_the_commitment_entry_fails_the_run() {
        let trace = run(CommitPoint::Entry);
        println!("{}", trace.describe());
        assert!(
            trace.fixture_held(),
            "fixture failure: {}",
            trace.describe()
        );
        assert!(
            trace.found == Found::Free
                && matches!(trace.at_boundary, Some(CancelReceipt::Accepted(_)))
                && trace.consistent(),
            "[cancel-first] an accepted cancellation is missing from the committed outcome: {}",
            trace.describe()
        );
        assert!(trace.late_after(), "{}", trace.describe());
    }

    /// Commitment first, and exclusion at the boundary itself: while the
    /// commitment holds the closure that decides its outcome, the control
    /// side cannot take the gate, so its cancellation can only follow the
    /// commitment: late, the outcome as committed (passed). A split
    /// commitment that read the closure under one guard and recorded itself
    /// under another would let that cancellation be accepted in between and
    /// then leave it out of the outcome.
    #[test]
    fn r2a_commitment_excludes_cancellation_between_its_decision_and_its_record() {
        let trace = run(CommitPoint::Decided);
        println!("{}", trace.describe());
        assert!(
            trace.fixture_held(),
            "fixture failure: {}",
            trace.describe()
        );
        assert!(
            trace.consistent(),
            "[commit-atomic] an accepted cancellation is missing from the committed outcome: {}",
            trace.describe()
        );
        assert_eq!(
            trace.found,
            Found::Held,
            "[commit-excludes] the control side took the gate inside the commitment: {}",
            trace.describe()
        );
        assert!(
            trace.at_boundary.is_none()
                && trace.late_after()
                && trace.terminal.map(|(_, verdict)| verdict) == Some(Verdict::Passed),
            "commitment first: {}",
            trace.describe()
        );
    }

    /// The boundary's activation: `Custody::new` installs none, and one can be
    /// installed only while nothing else holds the custody's shared state.
    #[test]
    fn r2a_commit_boundary_installs_only_before_the_custody_is_shared() {
        let (mut custody, control) =
            Custody::<Never>::new(Config::LIVE, GENERATION, Vec::new(), Tick(0))
                .expect("a valid configuration");
        assert!(
            control.shared.commit_boundary.is_none(),
            "a custody from the public constructor has no boundary"
        );
        let clone = control.clone();
        let (boundary, _reached, _release) = boundary_at(CommitPoint::Decided);
        let boundary = custody
            .install_commit_boundary(boundary)
            .expect_err("refused while control handles exist");
        drop(control);
        let boundary = custody
            .install_commit_boundary(boundary)
            .expect_err("refused while any control handle exists");
        drop(clone);
        assert!(custody.install_commit_boundary(boundary).is_ok());
        assert!(custody.control().shared.commit_boundary.is_some());
    }
}
