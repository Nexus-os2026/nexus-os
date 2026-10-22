//! The ledger's protocol core (D2A-1; contract P3-C11-D2A1-CONTRACT-02-R1).
//!
//! Its state, its rules, its tickets and its published word. It touches no
//! storage: every storage step is one verb of the traits below, supplied by
//! a shell that does not exist yet, and every decision about a verb's answer
//! is made here (section 4.1). Nothing outside `ledger` names this module.
//!
//! The word is touched in exactly two functions, `load` and `exchange`
//! (rule R1). A store operation publishes once, by the guard it creates
//! first (`Publish`, rules R2 and R7). Nothing here can unwind except a call
//! to a storage verb (rule R8), and a broken invariant latches (rule R9).
//!
//! Storage assumptions A-1 and A-2 (section 4.1) are stated, not discharged.

#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic
    )
)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod unit_tests;

pub(super) const ID_LEN: usize = 32;
pub(super) const BATCH_MAX: usize = 256;
pub(super) type Seq = u64;

// ---- records, as the core sees them ----------------------------------------

/// An original: a record's identity together with its digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RecordKey {
    pub(super) id: [u8; ID_LEN],
    pub(super) digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    Prepared,
    Started,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Part {
    pub(super) commitment: [u8; ID_LEN],
    pub(super) role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Member {
    pub(super) key: RecordKey,
    pub(super) part: Option<Part>,
}

/// A record a first writer or settlement appends. Not a gating record: the
/// two are different types, so neither can be offered through the other's
/// entry point (section 5.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FactRecord(Member);

/// A record an ordering point appends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GatingRecord(Member);

impl FactRecord {
    pub(super) fn new(member: Member) -> FactRecord {
        FactRecord(member)
    }

    pub(super) fn member(&self) -> &Member {
        &self.0
    }
}

impl GatingRecord {
    pub(super) fn new(member: Member) -> GatingRecord {
        GatingRecord(member)
    }

    pub(super) fn member(&self) -> &Member {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Limits {
    pub(super) tickets: u32,
    pub(super) refusals: u32,
}

// ---- the published word ------------------------------------------------------

/// A ledger's one shared allocation. "Same ledger" means the same allocation
/// (`Arc::ptr_eq`), never equal contents. It is created once, in `Core::new`,
/// and never replaced.
#[derive(Debug)]
struct Shared {
    word: AtomicU64,
    tickets: u32,
}

/// The lock-free face of the ledger: `view` and `arm`.
#[derive(Debug)]
pub(super) struct Health {
    shared: Arc<Shared>,
}

/// One load of the word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HealthView {
    word: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Healthy,
    NeedBarrier,
    Resolving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Session {
    Bootstrap,
    Open,
    Sealed,
}

// The word's layout (section 4.2). The low 16 bits are written only by a
// store operation's publication, from the core's state, except the sticky
// handover fault, which belongs to ticket accounting. Unused bits are zero.
const PHASE_NEED_BARRIER: u64 = 0x1;
const PHASE_RESOLVING: u64 = 0x2;
const PHASE_MASK: u64 = 0x3;
const SESSION_OPEN: u64 = 0x4;
const SESSION_SEALED: u64 = 0x8;
const SESSION_MASK: u64 = 0xC;
const LATCHED: u64 = 0x10;
const U_OCCUPIED: u64 = 0x20;
const C_OCCUPIED: u64 = 0x40;
const X_OCCUPIED: u64 = 0x80;
const HANDOVER_FAULT: u64 = 0x100;
const FLAGS: u64 = 0xFFFF;
/// `live`, tickets alive: 24 bits from bit 16.
const LIVE_SHIFT: u32 = 16;
/// `unapplied`, results not yet taken over: 24 bits from bit 40.
const UNAPPLIED_SHIFT: u32 = 40;
const COUNT_MAX: u64 = 0xFF_FFFF;
const TICKET_LIMIT_MAX: u32 = 0xFF_FFFF;

/// A count field of `word`.
fn field(word: u64, shift: u32) -> u64 {
    word.checked_shr(shift).unwrap_or(0) & COUNT_MAX
}

/// `word` with a count field replaced.
fn with_field(word: u64, shift: u32, value: u64) -> u64 {
    let mask = COUNT_MAX.checked_shl(shift).unwrap_or(0);
    (word & !mask) | (value.checked_shl(shift).unwrap_or(0) & mask)
}

/// One checked step of a count. A step that would overflow or underflow
/// sets the handover fault and leaves the count as it is (section 4.3).
fn count_step(word: u64, shift: u32, up: bool) -> u64 {
    let now = field(word, shift);
    let next = if up {
        now.checked_add(1).filter(|next| *next <= COUNT_MAX)
    } else {
        now.checked_sub(1)
    };
    match next {
        Some(next) => with_field(word, shift, next),
        None => word | HANDOVER_FAULT,
    }
}

/// The one load of the word (rule R1). Only `view` calls it.
fn load(shared: &Shared) -> u64 {
    let word = shared.word.load(Ordering::SeqCst);
    #[cfg(test)]
    hook::report(hook::Event::Loaded(word));
    word
}

/// The one compare-and-exchange loop on the word (rule R1). It starts from
/// a guess and takes the present value from a failed exchange; it never
/// loads. `step` gives the value to publish for a present value, and must
/// not unwind (rule R8). Returns the value published.
fn exchange(shared: &Shared, guess: u64, mut step: impl FnMut(u64) -> u64) -> u64 {
    let mut present = guess;
    loop {
        let next = step(present);
        match shared
            .word
            .compare_exchange(present, next, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => {
                #[cfg(test)]
                hook::report(hook::Event::Exchanged(present, next));
                return next;
            }
            Err(found) => {
                #[cfg(test)]
                hook::report(hook::Event::Failed(found));
                present = found;
            }
        }
    }
}

impl Health {
    /// One load. Admission takes every store-side condition from one view
    /// (rule R5).
    pub(super) fn view(&self) -> HealthView {
        HealthView {
            word: load(&self.shared),
        }
    }

    /// One exchange: `live` + 1 below the limit, and an Armed ticket; at
    /// the limit, the handover fault, and nothing.
    pub(super) fn arm(&self) -> Option<Ticket> {
        let limit = u64::from(self.shared.tickets);
        let mut armed = false;
        exchange(&self.shared, 0, |word| {
            let live = field(word, LIVE_SHIFT);
            armed = live < limit;
            if armed {
                with_field(word, LIVE_SHIFT, live.saturating_add(1))
            } else {
                word | HANDOVER_FAULT
            }
        });
        if armed {
            Some(Ticket {
                shared: Arc::clone(&self.shared),
                state: TicketState::Armed,
            })
        } else {
            None
        }
    }
}

impl HealthView {
    /// Exactly Healthy and open with nothing else set, and no result
    /// waiting to be taken over.
    pub(super) fn admits(self) -> bool {
        self.word & FLAGS == SESSION_OPEN && field(self.word, UNAPPLIED_SHIFT) == 0
    }

    pub(super) fn phase(self) -> Phase {
        match self.word & PHASE_MASK {
            0 => Phase::Healthy,
            PHASE_NEED_BARRIER => Phase::NeedBarrier,
            _ => Phase::Resolving,
        }
    }

    pub(super) fn session(self) -> Session {
        match self.word & SESSION_MASK {
            0 => Session::Bootstrap,
            SESSION_OPEN => Session::Open,
            _ => Session::Sealed,
        }
    }

    pub(super) fn latched(self) -> bool {
        self.word & LATCHED != 0
    }

    pub(super) fn handover_fault(self) -> bool {
        self.word & HANDOVER_FAULT != 0
    }

    pub(super) fn u_occupied(self) -> bool {
        self.word & U_OCCUPIED != 0
    }

    pub(super) fn c_occupied(self) -> bool {
        self.word & C_OCCUPIED != 0
    }

    pub(super) fn x_occupied(self) -> bool {
        self.word & X_OCCUPIED != 0
    }

    pub(super) fn unapplied(self) -> u32 {
        u32::try_from(field(self.word, UNAPPLIED_SHIFT)).unwrap_or(u32::MAX)
    }

    pub(super) fn live(self) -> u32 {
        u32::try_from(field(self.word, LIVE_SHIFT)).unwrap_or(u32::MAX)
    }
}

// ---- tickets -------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Persistence {
    NotWritten,
    Uncertain,
}

/// What a Counted ticket says: whether anything can have been written, and
/// for a conflict, which member. A named member implies Not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Counted {
    pub(super) persistence: Persistence,
    pub(super) conflict_member: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TicketState {
    Armed,
    Counted(Counted),
    /// Confirmed: its drop does nothing.
    Settled,
}

/// One attempt in flight or one handover not yet confirmed. It cannot be
/// cloned or copied.
#[derive(Debug)]
pub(super) struct Ticket {
    shared: Arc<Shared>,
    state: TicketState,
}

impl Ticket {
    pub(super) fn counted(&self) -> Option<Counted> {
        match self.state {
            TicketState::Counted(counted) => Some(counted),
            TicketState::Armed | TicketState::Settled => None,
        }
    }

    /// One exchange on the ticket's own ledger. Counted: `unapplied` and
    /// `live` each - 1. Armed: `live` - 1.
    pub(super) fn confirm(mut self) {
        match self.state {
            TicketState::Armed => {
                exchange(&self.shared, 0, |word| count_step(word, LIVE_SHIFT, false));
            }
            TicketState::Counted(_) => {
                exchange(&self.shared, 0, |word| {
                    count_step(count_step(word, UNAPPLIED_SHIFT, false), LIVE_SHIFT, false)
                });
            }
            TicketState::Settled => {}
        }
        self.state = TicketState::Settled;
    }

    /// For the shell only: the wait to enter the store ran out. An Armed
    /// ticket is counted as Not written; any other is a handover fault.
    pub(super) fn expire(&mut self) -> FactOutcome {
        if self.state == TicketState::Armed {
            exchange(&self.shared, 0, |word| {
                count_step(word, UNAPPLIED_SHIFT, true)
            });
            let counted = Counted {
                persistence: Persistence::NotWritten,
                conflict_member: None,
            };
            self.state = TicketState::Counted(counted);
            FactOutcome::NotApplied {
                counted,
                reason: Reason::Entry(Entry::Expired),
            }
        } else {
            exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);
            FactOutcome::Rejected(TicketFault::NotArmed)
        }
    }
}

impl Drop for Ticket {
    /// Armed: `live` - 1. Counted and unconfirmed: the handover fault, and
    /// both counts left as they are, so the ticket keeps its place in the
    /// limit. Confirmed: nothing.
    fn drop(&mut self) {
        match self.state {
            TicketState::Armed => {
                exchange(&self.shared, 0, |word| count_step(word, LIVE_SHIFT, false));
            }
            TicketState::Counted(_) => {
                exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);
            }
            TicketState::Settled => {}
        }
    }
}

// ---- causes ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub(super) seq: Seq,
    pub(super) digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Holder {
    pub(super) id: [u8; ID_LEN],
    pub(super) seq: Seq,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CauseKind {
    IdentityContent {
        id: [u8; ID_LEN],
        existing: Row,
        offered: [u8; 32],
    },
    DuplicateRole {
        part: Part,
        holder: Holder,
        offered: RecordKey,
    },
    MissingPrepared {
        part: Part,
        offered: RecordKey,
    },
}

/// Why an original conflicts. Built only by the core, from committed rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cause {
    kind: CauseKind,
}

impl Cause {
    pub(super) fn kind(&self) -> CauseKind {
        self.kind
    }
}

// ---- the storage verbs the shell supplies ----------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Read<T> {
    Got(T),
    Failed,
    Corrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Insert {
    Inserted(Seq),
    Constraint,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Commit {
    Durable,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Rollback {
    Undone,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MemberFacts {
    pub(super) row: Option<Row>,
    pub(super) explained: Option<Seq>,
    pub(super) corrected: Option<Seq>,
    pub(super) holder: Option<Holder>,
    pub(super) prepared: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SetFacts {
    pub(super) frontier: Seq,
    pub(super) head: Seq,
    pub(super) top: Seq,
    pub(super) count: u64,
    pub(super) digest: [u8; 32],
}

pub(super) trait Txn {
    fn open(&mut self) -> Read<()>;
    fn facts(&mut self, index: u16, member: &Member) -> Read<MemberFacts>;
    fn insert(&mut self, index: u16, member: &Member) -> Insert;
    fn commit(&mut self) -> Commit;
    fn rollback(&mut self) -> Rollback;
}

pub(super) trait Reads {
    fn row(&mut self, id: &[u8; ID_LEN]) -> Read<Option<Row>>;
    fn explained(&mut self, key: &RecordKey) -> Read<Option<Seq>>;
    fn acknowledgement(&mut self, set: &SetFacts, nonce: &[u8; 16]) -> Read<Option<Seq>>;
    fn frontier(&mut self) -> Read<Seq>;
}

pub(super) trait AckTxn {
    fn survey(&mut self) -> Read<SetFacts>;
    fn open(&mut self) -> Read<()>;
    fn recheck(&mut self, above: Seq, upto: Seq) -> Read<SetFacts>;
    fn insert(&mut self, snapshot: &Snapshot) -> Insert;
    fn commit(&mut self) -> Commit;
    fn rollback(&mut self) -> Rollback;
}

pub(super) trait CellTxn {
    fn open(&mut self) -> Read<()>;
    fn present(&mut self, occupant: &Occupant) -> Read<Option<Seq>>;
    fn insert(&mut self, occupant: &Occupant) -> Insert;
    fn commit(&mut self) -> Commit;
    fn rollback(&mut self) -> Rollback;
}

pub(super) trait BarrierRun {
    fn run(&mut self) -> bool;
}

// ---- results -------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    NotOpen,
    Latched,
    NotHealthy,
    CellsBusy,
    Malformed,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TicketFault {
    OtherLedger,
    NotArmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Known {
    Cause(Cause),
    Explained(Seq),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    Entry(Entry),
    Conflict { member: u16, known: Known },
    SessionClosed { member: u16 },
    Corrected { seq: Seq },
    TableFull { member: u16 },
    Constraint,
    Storage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FactOutcome {
    Committed,
    NotApplied { counted: Counted, reason: Reason },
    Rejected(TicketFault),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GatingOutcome {
    Committed(Seq),
    Refused {
        persistence: Persistence,
        reason: Reason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lookup {
    Present(Seq),
    Absent,
    Conflict(Known),
    Refused(Entry),
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Explain {
    Explained(Seq),
    Pending,
    Busy,
    Unknown,
    Refused(Entry),
}

// ---- acknowledgement -----------------------------------------------------------------

/// The set the Owner is shown. It cannot be cloned, and its append consumes
/// it whatever the append answers.
#[derive(Debug)]
pub(super) struct Snapshot {
    ledger: Arc<Shared>,
    set: SetFacts,
    nonce: [u8; 16],
}

impl Snapshot {
    pub(super) fn set(&self) -> SetFacts {
        self.set
    }

    pub(super) fn nonce(&self) -> [u8; 16] {
        self.nonce
    }
}

#[derive(Debug)]
pub(super) enum SnapshotOutcome {
    Taken(Snapshot),
    Nothing,
    NotNow(Entry),
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AckRefusal {
    OtherStore,
    NotNow(Entry),
    Stale,
    Empty,
    Future,
    Set,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AckOutcome {
    Committed(Seq),
    Refused(AckRefusal),
    NotApplied(Persistence),
}

// ---- cells and the worker ---------------------------------------------------------------

/// The one worker of a ledger. It cannot be cloned.
#[derive(Debug)]
pub(super) struct Worker {
    ledger: Arc<Shared>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cell {
    U,
    C,
    X,
}

/// A name for one occupancy of one cell of one ledger. Cloning it only
/// copies the name.
#[derive(Clone, Debug)]
pub(super) struct CellToken {
    ledger: Arc<Shared>,
    cell: Cell,
    occupancy: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Built,
    Committed(Seq),
}

/// How a gating attempt left U (finding PC-03). `InCall` exists only while
/// the call runs; outside a call it is a broken invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Exit {
    InCall,
    ReturnedUncertain,
    Unwound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Occupant {
    Gating {
        record: GatingRecord,
        exit: Exit,
    },
    Acknowledgement {
        set: SetFacts,
        nonce: [u8; 16],
    },
    Correction {
        about: RecordKey,
        seq: Seq,
        exit: Exit,
    },
    Explanation {
        original: RecordKey,
        cause: Cause,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LatchCause {
    Constraint,
    TableFull,
    TicketFault,
    CellInvariant,
    Inconsistent,
    Corrupt,
    Overflow,
}

#[derive(Clone, Debug)]
pub(super) struct Agenda {
    pub(super) phase: Phase,
    pub(super) session: Session,
    pub(super) latch: Option<LatchCause>,
    pub(super) u: Option<CellToken>,
    pub(super) c: Option<CellToken>,
    pub(super) x: Option<(CellToken, Stage)>,
    pub(super) frontier: Seq,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BarrierOutcome {
    Passed,
    Failed,
    NotNeeded,
    OtherLedger,
}

#[derive(Clone, Debug)]
pub(super) enum Resolve {
    Absent,
    Corrected(CellToken),
    Acknowledged(Seq),
    ConflictPlaced(CellToken),
    Retained,
    Inconsistent,
    Stale,
    Refused(Entry),
    OtherLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CellAppend {
    Committed(Seq),
    NotWritten,
    Uncertain,
    Stale,
    Refused(Entry),
    OtherLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Emptying {
    Done,
    NotCommitted,
    Stale,
    OtherLedger,
}

// ---- the core ------------------------------------------------------------------------------

/// The store's state. Each operation takes it by exclusive reference: in
/// the shell that comes from the store's one lock (rule R6).
#[derive(Debug)]
pub(super) struct Core {
    shared: Arc<Shared>,
    phase: Phase,
    session: Session,
    latch: Option<LatchCause>,
    u: Option<(u64, Occupant)>,
    c: Option<(u64, Occupant)>,
    x: Option<(u64, Occupant, Stage)>,
    occupancies: [u64; 3],
    refusals: BTreeMap<RecordKey, Cause>,
    refusal_limit: u32,
    frontier: Seq,
    worker_taken: bool,
}

/// Classification of one member (section 4.4, step 3).
enum Verdict {
    Present(Seq),
    Insert,
    Refused(Reason, Option<u16>),
}

/// What the guard does with U when the operation ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hold {
    Nothing,
    Gating,
    Acknowledgement,
}

/// A member's position in its batch. A well-formed batch has at most 256.
fn member_index(index: usize) -> u16 {
    u16::try_from(index).unwrap_or(u16::MAX)
}

/// 1 to 256 members; no identity twice; no commitment and role twice; and
/// no member needing a Prepared record before the Prepared member of its own
/// commitment (section 4.4).
fn well_formed(batch: &[FactRecord]) -> bool {
    if batch.is_empty() || batch.len() > BATCH_MAX {
        return false;
    }
    for (index, record) in batch.iter().enumerate() {
        let member = &record.0;
        for (other_index, other) in batch.iter().enumerate() {
            if other_index == index {
                continue;
            }
            let other = &other.0;
            if other.key.id == member.key.id {
                return false;
            }
            if let (Some(part), Some(other_part)) = (member.part, other.part) {
                if part == other_part {
                    return false;
                }
                if part.commitment == other_part.commitment
                    && part.role != Role::Prepared
                    && other_part.role == Role::Prepared
                    && other_index > index
                {
                    return false;
                }
            }
        }
    }
    true
}

/// Row d of the classification: a constraint the member would break, in
/// order: its identity with different content; its role already held; a
/// role that needs a Prepared record which neither the ledger nor an
/// earlier member has.
fn constraint<'m>(
    member: &Member,
    facts: &MemberFacts,
    mut earlier: impl Iterator<Item = &'m Member>,
) -> Option<CauseKind> {
    if let Some(existing) = facts.row {
        return Some(CauseKind::IdentityContent {
            id: member.key.id,
            existing,
            offered: member.key.digest,
        });
    }
    let part = member.part?;
    if let Some(holder) = facts.holder {
        return Some(CauseKind::DuplicateRole {
            part,
            holder,
            offered: member.key,
        });
    }
    let prepared = Some(Part {
        commitment: part.commitment,
        role: Role::Prepared,
    });
    if part.role != Role::Prepared && !facts.prepared && !earlier.any(|m| m.part == prepared) {
        return Some(CauseKind::MissingPrepared {
            part,
            offered: member.key,
        });
    }
    None
}

impl Cell {
    fn index(self) -> usize {
        match self {
            Cell::U => 0,
            Cell::C => 1,
            Cell::X => 2,
        }
    }
}

impl Core {
    pub(super) fn new(limits: Limits) -> (Core, Health) {
        let tickets = limits.tickets.clamp(1, TICKET_LIMIT_MAX);
        let shared = Arc::new(Shared {
            word: AtomicU64::new(0),
            tickets,
        });
        let core = Core {
            shared: Arc::clone(&shared),
            phase: Phase::Healthy,
            session: Session::Bootstrap,
            latch: None,
            u: None,
            c: None,
            x: None,
            occupancies: [0; 3],
            refusals: BTreeMap::new(),
            refusal_limit: limits.refusals.max(1),
            frontier: 0,
            worker_taken: false,
        };
        (core, Health { shared })
    }

    /// The one worker, once.
    pub(super) fn take_worker(&mut self) -> Option<Worker> {
        if self.worker_taken {
            return None;
        }
        self.worker_taken = true;
        Some(Worker {
            ledger: Arc::clone(&self.shared),
        })
    }

    /// Bootstrap to open. A bare transition in D2A-1 (section 4.9).
    pub(super) fn session_opened(&mut self) -> bool {
        let mut op = Publish::new(self);
        let opened = op.core.session == Session::Bootstrap;
        if opened {
            op.core.session = Session::Open;
        }
        op.done = true;
        opened
    }

    /// Open to sealed, only when quiescent. A bare transition in D2A-1.
    pub(super) fn session_sealed(&mut self) -> bool {
        let mut op = Publish::new(self);
        let core = &mut *op.core;
        let sealed = core.session == Session::Open
            && core.phase == Phase::Healthy
            && core.latch.is_none()
            && core.u.is_none()
            && core.c.is_none()
            && core.x.is_none();
        if sealed {
            core.session = Session::Sealed;
        }
        op.done = true;
        sealed
    }

    /// One read of the core's own state, by shared reference. It does not
    /// read the word.
    pub(super) fn agenda(&self) -> Agenda {
        Agenda {
            phase: self.phase,
            session: self.session,
            latch: self.latch,
            u: self.u.as_ref().map(|(n, _)| self.token(Cell::U, *n)),
            c: self.c.as_ref().map(|(n, _)| self.token(Cell::C, *n)),
            x: self
                .x
                .as_ref()
                .map(|(n, _, stage)| (self.token(Cell::X, *n), *stage)),
            frontier: self.frontier,
        }
    }

    pub(super) fn append_facts(
        &mut self,
        ticket: &mut Ticket,
        batch: &[FactRecord],
        io: &mut impl Txn,
    ) -> FactOutcome {
        let mut op = Publish::new(self);
        let outcome = op.facts(ticket, batch, io);
        op.done = true;
        outcome
    }

    pub(super) fn append_gating(
        &mut self,
        record: &GatingRecord,
        io: &mut impl Txn,
    ) -> GatingOutcome {
        let mut op = Publish::new(self);
        let outcome = op.gating(record, io);
        op.done = true;
        outcome
    }

    pub(super) fn lookup(&mut self, key: &RecordKey, io: &mut impl Reads) -> Lookup {
        let mut op = Publish::new(self);
        let outcome = op.lookup(key, io);
        op.done = true;
        outcome
    }

    pub(super) fn explain(&mut self, key: &RecordKey, io: &mut impl Reads) -> Explain {
        let mut op = Publish::new(self);
        let outcome = op.explain(key, io);
        op.done = true;
        outcome
    }

    pub(super) fn snapshot(&mut self, nonce: [u8; 16], io: &mut impl AckTxn) -> SnapshotOutcome {
        let mut op = Publish::new(self);
        let outcome = op.snapshot(nonce, io);
        op.done = true;
        outcome
    }

    pub(super) fn append_acknowledgement(
        &mut self,
        snapshot: Snapshot,
        io: &mut impl AckTxn,
    ) -> AckOutcome {
        let mut op = Publish::new(self);
        let outcome = op.acknowledgement(&snapshot, io);
        op.done = true;
        outcome
    }

    pub(super) fn barrier(
        &mut self,
        worker: &mut Worker,
        io: &mut impl BarrierRun,
    ) -> BarrierOutcome {
        let mut op = Publish::new(self);
        let outcome = op.barrier(worker, io);
        op.done = true;
        outcome
    }

    pub(super) fn resolve(
        &mut self,
        worker: &mut Worker,
        token: CellToken,
        io: &mut impl Reads,
    ) -> Resolve {
        let mut op = Publish::new(self);
        let outcome = op.resolve(worker, &token, io);
        op.done = true;
        outcome
    }

    pub(super) fn append_cell(
        &mut self,
        worker: &mut Worker,
        token: CellToken,
        io: &mut impl CellTxn,
    ) -> CellAppend {
        let mut op = Publish::new(self);
        let outcome = op.cell(worker, &token, io);
        op.done = true;
        outcome
    }

    pub(super) fn empty(&mut self, worker: &mut Worker, token: CellToken) -> Emptying {
        let mut op = Publish::new(self);
        let outcome = op.empty(worker, &token);
        op.done = true;
        outcome
    }

    // ---- private ----

    /// The phase, session, latch and cell bits of the word, from the core.
    fn flags(&self) -> u64 {
        let phase = match self.phase {
            Phase::Healthy => 0,
            Phase::NeedBarrier => PHASE_NEED_BARRIER,
            Phase::Resolving => PHASE_RESOLVING,
        };
        let session = match self.session {
            Session::Bootstrap => 0,
            Session::Open => SESSION_OPEN,
            Session::Sealed => SESSION_SEALED,
        };
        let bit = |set: bool, bit: u64| if set { bit } else { 0 };
        phase
            | session
            | bit(self.latch.is_some(), LATCHED)
            | bit(self.u.is_some(), U_OCCUPIED)
            | bit(self.c.is_some(), C_OCCUPIED)
            | bit(self.x.is_some(), X_OCCUPIED)
    }

    /// Latches the store. A latch is never cleared; its first cause stays.
    fn latch(&mut self, cause: LatchCause) {
        if self.latch.is_none() {
            self.latch = Some(cause);
        }
    }

    fn token(&self, cell: Cell, occupancy: u64) -> CellToken {
        CellToken {
            ledger: Arc::clone(&self.shared),
            cell,
            occupancy,
        }
    }

    fn occupancy(&self, cell: Cell) -> Option<u64> {
        match cell {
            Cell::U => self.u.as_ref().map(|(n, _)| *n),
            Cell::C => self.c.as_ref().map(|(n, _)| *n),
            Cell::X => self.x.as_ref().map(|(n, _, _)| *n),
        }
    }

    /// Whether `token` names this ledger's `cell` at its present occupancy.
    fn names(&self, token: &CellToken, cell: Cell) -> bool {
        Arc::ptr_eq(&token.ledger, &self.shared)
            && token.cell == cell
            && self.occupancy(cell) == Some(token.occupancy)
    }

    /// A new occupancy number for `cell`. A number that would overflow
    /// latches the store and gives none.
    fn next_occupancy(&mut self, cell: Cell) -> Option<u64> {
        let next = self
            .occupancies
            .get(cell.index())
            .and_then(|n| n.checked_add(1));
        match (next, self.occupancies.get_mut(cell.index())) {
            (Some(next), Some(slot)) => {
                *slot = next;
                Some(next)
            }
            _ => {
                self.latch(LatchCause::Overflow);
                None
            }
        }
    }

    /// Places an occupant into an empty U, C or X. False if the occupancy
    /// number ran out (the store is then latched).
    fn place(&mut self, cell: Cell, occupant: Occupant) -> bool {
        let Some(occupancy) = self.next_occupancy(cell) else {
            return false;
        };
        match cell {
            Cell::U => self.u = Some((occupancy, occupant)),
            Cell::C => self.c = Some((occupancy, occupant)),
            Cell::X => self.x = Some((occupancy, occupant, Stage::Built)),
        }
        true
    }

    fn explanation(&mut self, original: RecordKey, cause: Cause) -> bool {
        self.place(Cell::X, Occupant::Explanation { original, cause })
    }

    fn table_full(&self) -> bool {
        self.refusals.len() >= usize::try_from(self.refusal_limit).unwrap_or(usize::MAX)
    }

    /// What the store already knows of `key` as a conflict: its retained
    /// cause, or X's explanation of it, or its explanation's sequence.
    fn known(&self, key: &RecordKey, explained: Option<Seq>) -> Option<Known> {
        if let Some(cause) = self.refusals.get(key) {
            return Some(Known::Cause(*cause));
        }
        if let Some((_, Occupant::Explanation { original, cause }, stage)) = &self.x {
            if original == key {
                return Some(match stage {
                    Stage::Built => Known::Cause(*cause),
                    Stage::Committed(seq) => Known::Explained(*seq),
                });
            }
        }
        explained.map(Known::Explained)
    }

    /// Classifies one member, in the order of section 4.4, step 3. A gating
    /// record is first tested for a correction (section 4.5); its conflict
    /// is explained in X and makes no table entry.
    fn classify<'m>(
        &mut self,
        at: u16,
        member: &Member,
        facts: &MemberFacts,
        earlier: impl Iterator<Item = &'m Member>,
        gating: bool,
    ) -> Verdict {
        if gating {
            if let Some(seq) = facts.corrected {
                return Verdict::Refused(Reason::Corrected { seq }, None);
            }
        }
        if let Some(row) = facts.row {
            if row.digest == member.key.digest {
                return Verdict::Present(row.seq);
            }
        }
        if let Some(known) = self.known(&member.key, facts.explained) {
            return Verdict::Refused(Reason::Conflict { member: at, known }, Some(at));
        }
        if self.session == Session::Sealed {
            return Verdict::Refused(Reason::SessionClosed { member: at }, None);
        }
        let Some(kind) = constraint(member, facts, earlier) else {
            return Verdict::Insert;
        };
        let cause = Cause { kind };
        let conflict = Reason::Conflict {
            member: at,
            known: Known::Cause(cause),
        };
        if gating {
            if self.x.is_none() {
                self.explanation(member.key, cause);
            }
            return Verdict::Refused(conflict, Some(at));
        }
        if self.table_full() {
            self.latch(LatchCause::TableFull);
            return Verdict::Refused(Reason::TableFull { member: at }, Some(at));
        }
        self.refusals.insert(member.key, cause);
        if self.x.is_none() {
            self.explanation(member.key, cause);
        }
        Verdict::Refused(conflict, Some(at))
    }
}

// ---- the guard of a store operation ---------------------------------------------------------

/// Created first by every store operation, before its ticket check and its
/// entry rule (rule R7). Its drop publishes once, on return and on unwind
/// alike; on unwind it first sets the phase to barrier needed and the
/// persistence by section 4.4. The count rises in the exchange and the lent
/// ticket's state is written next, with nothing between that can unwind.
struct Publish<'a> {
    core: &'a mut Core,
    /// The flags the core last published. Its counts are not known to the
    /// core; a failed exchange supplies them.
    guess: u64,
    /// The lent ticket, once it passed the check of step 0.
    ticket: Option<&'a mut Ticket>,
    /// How the lent ticket is counted, once decided.
    count: Option<Counted>,
    /// An insert has been issued.
    issued: bool,
    /// The member already found to conflict.
    conflict: Option<u16>,
    /// What the operation placed in U for the length of the call.
    hold: Hold,
    /// The operation's result keeps U's occupant.
    keep: bool,
    /// The operation returned.
    done: bool,
}

impl<'a> Publish<'a> {
    fn new(core: &'a mut Core) -> Publish<'a> {
        let guess = core.flags();
        Publish {
            core,
            guess,
            ticket: None,
            count: None,
            issued: false,
            conflict: None,
            hold: Hold::Nothing,
            keep: false,
            done: false,
        }
    }

    /// A failing verb: the phase becomes barrier needed.
    fn failed(&mut self) {
        self.core.phase = Phase::NeedBarrier;
    }

    /// A failed or corrupt read: the phase becomes barrier needed, and a
    /// corrupt read latches.
    fn read_failed(&mut self, corrupt: bool) {
        self.failed();
        if corrupt {
            self.core.latch(LatchCause::Corrupt);
        }
    }

    /// The one rollback's answer. It can make the phase worse; it never
    /// makes the persistence better.
    fn rolled(&mut self, answer: Rollback) -> Persistence {
        match answer {
            Rollback::Undone => Persistence::NotWritten,
            Rollback::Unknown => {
                self.failed();
                Persistence::Uncertain
            }
        }
    }

    fn not_applied(&mut self, persistence: Persistence, reason: Reason) -> FactOutcome {
        let counted = Counted {
            persistence,
            conflict_member: self.conflict,
        };
        self.count = Some(counted);
        FactOutcome::NotApplied { counted, reason }
    }

    /// Section 4.4.
    fn facts(
        &mut self,
        ticket: &'a mut Ticket,
        batch: &[FactRecord],
        io: &mut impl Txn,
    ) -> FactOutcome {
        // Step 0: the ticket belongs to this ledger and is Armed.
        let fault = if !Arc::ptr_eq(&ticket.shared, &self.core.shared) {
            Some(TicketFault::OtherLedger)
        } else if ticket.state != TicketState::Armed {
            Some(TicketFault::NotArmed)
        } else {
            None
        };
        if let Some(fault) = fault {
            self.core.latch(LatchCause::TicketFault);
            return FactOutcome::Rejected(fault);
        }
        self.ticket = Some(ticket);
        // Step 1: the entry rule.
        let entry = if self.core.session == Session::Bootstrap {
            Some(Entry::NotOpen)
        } else if self.core.latch.is_some() {
            Some(Entry::Latched)
        } else if self.core.phase != Phase::Healthy {
            Some(Entry::NotHealthy)
        } else if !well_formed(batch) {
            Some(Entry::Malformed)
        } else {
            None
        };
        if let Some(entry) = entry {
            return self.not_applied(Persistence::NotWritten, Reason::Entry(entry));
        }
        // Step 2.
        if io.open() != Read::Got(()) {
            self.failed();
            return self.not_applied(Persistence::NotWritten, Reason::Storage);
        }
        // Step 3: every member classified before any insert.
        let mut pending = [false; BATCH_MAX];
        let mut any = false;
        let members = || batch.iter().map(FactRecord::member);
        for (index, (member, slot)) in members().zip(pending.iter_mut()).enumerate() {
            let at = member_index(index);
            let facts = match io.facts(at, member) {
                Read::Got(facts) => facts,
                failed => {
                    self.read_failed(failed == Read::Corrupt);
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, Reason::Storage);
                }
            };
            match self
                .core
                .classify(at, member, &facts, members().take(index), false)
            {
                Verdict::Present(_) => {}
                Verdict::Insert => {
                    *slot = true;
                    any = true;
                }
                Verdict::Refused(reason, conflict) => {
                    self.conflict = conflict;
                    self.rolled(io.rollback());
                    return self.not_applied(Persistence::NotWritten, reason);
                }
            }
        }
        // Step 4.
        if !any {
            self.rolled(io.rollback());
            return FactOutcome::Committed;
        }
        // Step 5.
        for (index, (member, slot)) in members().zip(pending.iter()).enumerate() {
            if !*slot {
                continue;
            }
            self.issued = true;
            let reason = match io.insert(member_index(index), member) {
                Insert::Inserted(_) => continue,
                Insert::Constraint => {
                    self.core.latch(LatchCause::Constraint);
                    Reason::Constraint
                }
                Insert::Failed => Reason::Storage,
            };
            self.failed();
            let persistence = self.rolled(io.rollback());
            return self.not_applied(persistence, reason);
        }
        // Step 6.
        match io.commit() {
            Commit::Durable => FactOutcome::Committed,
            Commit::Unknown => {
                self.failed();
                self.rolled(io.rollback());
                self.not_applied(Persistence::Uncertain, Reason::Storage)
            }
        }
    }

    /// Section 4.5.
    fn gating(&mut self, record: &GatingRecord, io: &mut impl Txn) -> GatingOutcome {
        let refused = |persistence, reason| GatingOutcome::Refused {
            persistence,
            reason,
        };
        let entry = |entry| refused(Persistence::NotWritten, Reason::Entry(entry));
        let core = &mut *self.core;
        if core.session != Session::Open {
            return entry(Entry::NotOpen);
        }
        if core.latch.is_some() {
            return entry(Entry::Latched);
        }
        if core.phase != Phase::Healthy {
            return entry(Entry::NotHealthy);
        }
        if core.c.is_some() || core.x.is_some() {
            return entry(Entry::CellsBusy);
        }
        if core.u.is_some() {
            core.latch(LatchCause::CellInvariant);
            return entry(Entry::Latched);
        }
        let occupant = Occupant::Gating {
            record: record.clone(),
            exit: Exit::InCall,
        };
        if !core.place(Cell::U, occupant) {
            return entry(Entry::Latched);
        }
        self.hold = Hold::Gating;
        if io.open() != Read::Got(()) {
            self.failed();
            return refused(Persistence::NotWritten, Reason::Storage);
        }
        let member = record.member();
        let facts = match io.facts(0, member) {
            Read::Got(facts) => facts,
            failed => {
                self.read_failed(failed == Read::Corrupt);
                self.rolled(io.rollback());
                return refused(Persistence::NotWritten, Reason::Storage);
            }
        };
        match self.core.classify(0, member, &facts, [].iter(), true) {
            Verdict::Present(seq) => {
                self.rolled(io.rollback());
                return GatingOutcome::Committed(seq);
            }
            Verdict::Refused(reason, _) => {
                self.rolled(io.rollback());
                return refused(Persistence::NotWritten, reason);
            }
            Verdict::Insert => {}
        }
        self.issued = true;
        let reason = match io.insert(0, member) {
            Insert::Inserted(seq) => match io.commit() {
                Commit::Durable => return GatingOutcome::Committed(seq),
                Commit::Unknown => {
                    self.failed();
                    self.rolled(io.rollback());
                    self.keep = true;
                    return refused(Persistence::Uncertain, Reason::Storage);
                }
            },
            Insert::Constraint => {
                self.core.latch(LatchCause::Constraint);
                Reason::Constraint
            }
            Insert::Failed => Reason::Storage,
        };
        self.failed();
        let persistence = self.rolled(io.rollback());
        self.keep = persistence == Persistence::Uncertain;
        refused(persistence, reason)
    }

    /// Section 4.6.
    fn lookup(&mut self, key: &RecordKey, io: &mut impl Reads) -> Lookup {
        if self.core.session != Session::Open {
            return Lookup::Refused(Entry::NotOpen);
        }
        if self.core.latch.is_some() {
            return Lookup::Refused(Entry::Latched);
        }
        if self.core.phase == Phase::NeedBarrier {
            return Lookup::Refused(Entry::NotHealthy);
        }
        let existing = match io.row(&key.id) {
            Read::Got(None) => return Lookup::Absent,
            Read::Got(Some(row)) if row.digest == key.digest => return Lookup::Present(row.seq),
            Read::Got(Some(row)) => row,
            Read::Failed => {
                self.failed();
                return Lookup::Failed;
            }
            Read::Corrupt => {
                self.core.latch(LatchCause::Corrupt);
                return Lookup::Refused(Entry::Latched);
            }
        };
        // A conflict observation. Lookup never fills X.
        if let Some(known) = self.core.known(key, None) {
            return Lookup::Conflict(known);
        }
        match io.explained(key) {
            Read::Got(Some(seq)) => Lookup::Conflict(Known::Explained(seq)),
            Read::Got(None) => {
                if self.core.table_full() {
                    self.core.latch(LatchCause::TableFull);
                    return Lookup::Refused(Entry::Latched);
                }
                let cause = Cause {
                    kind: CauseKind::IdentityContent {
                        id: key.id,
                        existing,
                        offered: key.digest,
                    },
                };
                self.core.refusals.insert(*key, cause);
                Lookup::Conflict(Known::Cause(cause))
            }
            Read::Failed => {
                self.failed();
                Lookup::Failed
            }
            Read::Corrupt => {
                self.core.latch(LatchCause::Corrupt);
                Lookup::Refused(Entry::Latched)
            }
        }
    }

    /// Section 4.6.
    fn explain(&mut self, key: &RecordKey, io: &mut impl Reads) -> Explain {
        if self.core.session != Session::Open {
            return Explain::Refused(Entry::NotOpen);
        }
        if self.core.latch.is_some() {
            return Explain::Refused(Entry::Latched);
        }
        if self.core.phase != Phase::Healthy {
            return Explain::Busy;
        }
        match io.explained(key) {
            Read::Got(Some(seq)) => {
                self.core.refusals.remove(key);
                return Explain::Explained(seq);
            }
            Read::Got(None) => {}
            failed => {
                self.read_failed(failed == Read::Corrupt);
                return Explain::Busy;
            }
        }
        let core = &mut *self.core;
        match &core.x {
            Some((_, Occupant::Explanation { original, .. }, _)) if original == key => {
                Explain::Pending
            }
            Some(_) => Explain::Busy,
            None => match core.refusals.get(key).copied() {
                Some(cause) => {
                    if core.explanation(*key, cause) {
                        Explain::Pending
                    } else {
                        Explain::Refused(Entry::Latched)
                    }
                }
                None => Explain::Unknown,
            },
        }
    }

    /// Section 4.8.
    fn snapshot(&mut self, nonce: [u8; 16], io: &mut impl AckTxn) -> SnapshotOutcome {
        if self.core.session != Session::Open {
            return SnapshotOutcome::NotNow(Entry::NotOpen);
        }
        if self.core.latch.is_some() {
            return SnapshotOutcome::NotNow(Entry::Latched);
        }
        if self.core.phase != Phase::Healthy {
            return SnapshotOutcome::NotNow(Entry::NotHealthy);
        }
        match io.survey() {
            Read::Got(set) => {
                if set.top > set.frontier && set.count > 0 {
                    SnapshotOutcome::Taken(Snapshot {
                        ledger: Arc::clone(&self.core.shared),
                        set,
                        nonce,
                    })
                } else {
                    SnapshotOutcome::Nothing
                }
            }
            failed => {
                self.read_failed(failed == Read::Corrupt);
                SnapshotOutcome::Failed
            }
        }
    }

    /// Section 4.8: six tests, in order, then the steps of 4.4.
    fn acknowledgement(&mut self, snapshot: &Snapshot, io: &mut impl AckTxn) -> AckOutcome {
        let not_now = |entry| AckOutcome::Refused(AckRefusal::NotNow(entry));
        // Test 0.
        if !Arc::ptr_eq(&snapshot.ledger, &self.core.shared) {
            return AckOutcome::Refused(AckRefusal::OtherStore);
        }
        // Test 1.
        let core = &mut *self.core;
        if core.session != Session::Open {
            return not_now(Entry::NotOpen);
        }
        if core.latch.is_some() {
            return not_now(Entry::Latched);
        }
        if core.phase != Phase::Healthy {
            return not_now(Entry::NotHealthy);
        }
        if core.c.is_some() || core.x.is_some() {
            return not_now(Entry::CellsBusy);
        }
        if core.u.is_some() {
            core.latch(LatchCause::CellInvariant);
            return not_now(Entry::Latched);
        }
        let set = snapshot.set;
        let occupant = Occupant::Acknowledgement {
            set,
            nonce: snapshot.nonce,
        };
        if !core.place(Cell::U, occupant) {
            return not_now(Entry::Latched);
        }
        self.hold = Hold::Acknowledgement;
        if io.open() != Read::Got(()) {
            self.failed();
            return AckOutcome::NotApplied(Persistence::NotWritten);
        }
        let now = match io.recheck(set.frontier, set.top) {
            Read::Got(now) => now,
            failed => {
                self.read_failed(failed == Read::Corrupt);
                self.rolled(io.rollback());
                return AckOutcome::NotApplied(Persistence::NotWritten);
            }
        };
        let refusal = if set.frontier != now.frontier {
            Some(AckRefusal::Stale)
        } else if set.top <= set.frontier || set.count < 1 {
            Some(AckRefusal::Empty)
        } else if set.top > set.head || set.head > now.head {
            Some(AckRefusal::Future)
        } else if now.count != set.count || now.digest != set.digest {
            Some(AckRefusal::Set)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            self.rolled(io.rollback());
            return AckOutcome::Refused(refusal);
        }
        self.issued = true;
        match io.insert(snapshot) {
            Insert::Inserted(_) => {}
            failed => {
                if failed == Insert::Constraint {
                    self.core.latch(LatchCause::Constraint);
                }
                self.failed();
                let persistence = self.rolled(io.rollback());
                self.keep = persistence == Persistence::Uncertain;
                return AckOutcome::NotApplied(persistence);
            }
        }
        match io.commit() {
            Commit::Durable => {
                if set.top > self.core.frontier {
                    self.core.frontier = set.top;
                }
                AckOutcome::Committed(set.top)
            }
            Commit::Unknown => {
                self.failed();
                self.rolled(io.rollback());
                self.keep = true;
                AckOutcome::NotApplied(Persistence::Uncertain)
            }
        }
    }

    fn worker_belongs(&self, worker: &Worker) -> bool {
        Arc::ptr_eq(&worker.ledger, &self.core.shared)
    }

    /// Section 4.7.
    fn barrier(&mut self, worker: &Worker, io: &mut impl BarrierRun) -> BarrierOutcome {
        if !self.worker_belongs(worker) {
            return BarrierOutcome::OtherLedger;
        }
        if self.core.phase != Phase::NeedBarrier {
            return BarrierOutcome::NotNeeded;
        }
        if !io.run() {
            return BarrierOutcome::Failed;
        }
        self.core.phase = if self.core.u.is_some() {
            Phase::Resolving
        } else {
            Phase::Healthy
        };
        BarrierOutcome::Passed
    }

    /// Section 4.7: the store does the lookup itself.
    fn resolve(&mut self, worker: &Worker, token: &CellToken, io: &mut impl Reads) -> Resolve {
        if !self.worker_belongs(worker) {
            return Resolve::OtherLedger;
        }
        let core = &mut *self.core;
        if !core.names(token, Cell::U) {
            return Resolve::Stale;
        }
        if core.session != Session::Open {
            return Resolve::Refused(Entry::NotOpen);
        }
        if core.latch.is_some() {
            return Resolve::Refused(Entry::Latched);
        }
        if core.phase != Phase::Resolving {
            return Resolve::Refused(Entry::NotHealthy);
        }
        let occupant = core.u.as_ref().map(|(_, occupant)| occupant.clone());
        let room = core.c.is_none() && core.x.is_none();
        match occupant {
            Some(Occupant::Gating { record, exit }) if room && exit != Exit::InCall => {
                self.resolve_gating(&record, exit, io)
            }
            Some(Occupant::Acknowledgement { set, nonce }) if room => {
                self.resolve_ack(&set, &nonce, io)
            }
            _ => {
                self.core.latch(LatchCause::CellInvariant);
                Resolve::Refused(Entry::Latched)
            }
        }
    }

    fn resolve_gating(
        &mut self,
        record: &GatingRecord,
        exit: Exit,
        io: &mut impl Reads,
    ) -> Resolve {
        let key = record.member().key;
        let resolved = match io.row(&key.id) {
            Read::Got(None) => Resolve::Absent,
            Read::Got(Some(row)) if row.digest == key.digest => {
                let correction = Occupant::Correction {
                    about: key,
                    seq: row.seq,
                    exit,
                };
                if !self.core.place(Cell::C, correction) {
                    return Resolve::Refused(Entry::Latched);
                }
                let Some(n) = self.core.occupancy(Cell::C) else {
                    return Resolve::Refused(Entry::Latched);
                };
                Resolve::Corrected(self.core.token(Cell::C, n))
            }
            Read::Got(Some(existing)) => {
                let cause = Cause {
                    kind: CauseKind::IdentityContent {
                        id: key.id,
                        existing,
                        offered: key.digest,
                    },
                };
                if !self.core.explanation(key, cause) {
                    return Resolve::Refused(Entry::Latched);
                }
                let Some(n) = self.core.occupancy(Cell::X) else {
                    return Resolve::Refused(Entry::Latched);
                };
                Resolve::ConflictPlaced(self.core.token(Cell::X, n))
            }
            Read::Failed => {
                self.failed();
                return Resolve::Retained;
            }
            Read::Corrupt => {
                self.core.latch(LatchCause::Corrupt);
                return Resolve::Refused(Entry::Latched);
            }
        };
        self.core.u = None;
        self.core.phase = Phase::Healthy;
        resolved
    }

    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {
        match io.acknowledgement(set, nonce) {
            Read::Got(None) => {
                self.core.u = None;
                self.core.phase = Phase::Healthy;
                Resolve::Absent
            }
            Read::Got(Some(_)) => match io.frontier() {
                Read::Got(durable) if durable >= set.top => {
                    if set.top > self.core.frontier {
                        self.core.frontier = set.top;
                    }
                    self.core.u = None;
                    self.core.phase = Phase::Healthy;
                    Resolve::Acknowledged(set.top)
                }
                Read::Got(_) => {
                    self.core.latch(LatchCause::Inconsistent);
                    Resolve::Inconsistent
                }
                Read::Failed => {
                    self.failed();
                    Resolve::Retained
                }
                Read::Corrupt => {
                    self.core.latch(LatchCause::Corrupt);
                    Resolve::Refused(Entry::Latched)
                }
            },
            Read::Failed => {
                self.failed();
                Resolve::Retained
            }
            Read::Corrupt => {
                self.core.latch(LatchCause::Corrupt);
                Resolve::Refused(Entry::Latched)
            }
        }
    }

    /// Section 4.7: appends C's or X's own occupant.
    fn cell(&mut self, worker: &Worker, token: &CellToken, io: &mut impl CellTxn) -> CellAppend {
        if !self.worker_belongs(worker) {
            return CellAppend::OtherLedger;
        }
        let core = &mut *self.core;
        if !(core.names(token, Cell::C) || core.names(token, Cell::X)) {
            return CellAppend::Stale;
        }
        if core.session != Session::Open {
            return CellAppend::Refused(Entry::NotOpen);
        }
        if core.latch.is_some() {
            return CellAppend::Refused(Entry::Latched);
        }
        if core.phase != Phase::Healthy {
            return CellAppend::Refused(Entry::NotHealthy);
        }
        let cell = token.cell;
        let occupant = match (&core.c, &core.x) {
            (Some((_, occupant)), _) if cell == Cell::C => occupant.clone(),
            (_, Some((_, _, Stage::Committed(seq)))) if cell == Cell::X => {
                return CellAppend::Committed(*seq)
            }
            (_, Some((_, occupant, Stage::Built))) if cell == Cell::X => occupant.clone(),
            _ => {
                core.latch(LatchCause::CellInvariant);
                return CellAppend::Refused(Entry::Latched);
            }
        };
        if io.open() != Read::Got(()) {
            self.failed();
            return CellAppend::NotWritten;
        }
        match io.present(&occupant) {
            Read::Got(Some(seq)) => {
                self.rolled(io.rollback());
                self.settle(cell, seq);
                return CellAppend::Committed(seq);
            }
            Read::Got(None) => {}
            failed => {
                self.read_failed(failed == Read::Corrupt);
                self.rolled(io.rollback());
                return CellAppend::NotWritten;
            }
        }
        self.issued = true;
        let seq = match io.insert(&occupant) {
            Insert::Inserted(seq) => seq,
            failed => {
                if failed == Insert::Constraint {
                    self.core.latch(LatchCause::Constraint);
                }
                self.failed();
                return match self.rolled(io.rollback()) {
                    Persistence::NotWritten => CellAppend::NotWritten,
                    Persistence::Uncertain => CellAppend::Uncertain,
                };
            }
        };
        match io.commit() {
            Commit::Durable => {
                self.settle(cell, seq);
                CellAppend::Committed(seq)
            }
            Commit::Unknown => {
                self.failed();
                self.rolled(io.rollback());
                CellAppend::Uncertain
            }
        }
    }

    /// C's occupant in the ledger: C emptied. X's: X marked committed, and
    /// the original's table entry removed.
    fn settle(&mut self, cell: Cell, seq: Seq) {
        if cell == Cell::C {
            self.core.c = None;
            return;
        }
        if let Some((_, occupant, stage)) = self.core.x.as_mut() {
            *stage = Stage::Committed(seq);
            if let Occupant::Explanation { original, .. } = occupant {
                let original = *original;
                self.core.refusals.remove(&original);
            }
        }
    }

    /// Section 4.7: X is emptied only once it is marked committed.
    fn empty(&mut self, worker: &Worker, token: &CellToken) -> Emptying {
        if !self.worker_belongs(worker) {
            return Emptying::OtherLedger;
        }
        if !self.core.names(token, Cell::X) {
            return Emptying::Stale;
        }
        match self.core.x {
            Some((_, _, Stage::Committed(_))) => {
                self.core.x = None;
                Emptying::Done
            }
            _ => Emptying::NotCommitted,
        }
    }
}

impl Drop for Publish<'_> {
    fn drop(&mut self) {
        if !self.done {
            // Unwound: a state at least as closed as the truth.
            self.core.phase = Phase::NeedBarrier;
            if self.ticket.is_some() && self.count.is_none() {
                let persistence = if self.issued && self.conflict.is_none() {
                    Persistence::Uncertain
                } else {
                    Persistence::NotWritten
                };
                self.count = Some(Counted {
                    persistence,
                    conflict_member: self.conflict,
                });
            }
            self.keep = self.issued;
        }
        if self.hold != Hold::Nothing {
            if !self.keep {
                self.core.u = None;
            } else if let Some((_, Occupant::Gating { exit, .. })) = self.core.u.as_mut() {
                *exit = if self.done {
                    Exit::ReturnedUncertain
                } else {
                    Exit::Unwound
                };
            }
        }
        let flags = self.core.flags();
        let counted = self.ticket.is_some() && self.count.is_some();
        exchange(&self.core.shared, self.guess, |word| {
            let carried = (word & !FLAGS) | (word & HANDOVER_FAULT) | flags;
            if counted {
                count_step(carried, UNAPPLIED_SHIFT, true)
            } else {
                carried
            }
        });
        if let (Some(ticket), Some(counted)) = (self.ticket.as_mut(), self.count) {
            ticket.state = TicketState::Counted(counted);
        }
    }
}

// ---- test-only instrumentation ----------------------------------------------------------------

/// The one test-only item in production text (section 6.4). In a test
/// build, `load` and `exchange` report each load, each failed exchange and
/// each successful exchange here. It makes no store publication and never
/// re-enters the core; a callback is not called again while it runs.
#[cfg(test)]
mod hook {
    use std::cell::{Cell, RefCell};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Event {
        /// The value loaded.
        Loaded(u64),
        /// The present value a failed exchange returned.
        Failed(u64),
        /// The value replaced and the value published.
        Exchanged(u64, u64),
    }

    /// Loads, failed exchanges and successful exchanges on this thread.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(super) struct Counts {
        pub(super) loads: u64,
        pub(super) failed: u64,
        pub(super) exchanges: u64,
    }

    type Callback = Box<dyn FnMut(Event)>;

    thread_local! {
        static COUNTS: Cell<Counts> = const {
            Cell::new(Counts { loads: 0, failed: 0, exchanges: 0 })
        };
        static CALLBACK: RefCell<Option<Callback>> = const { RefCell::new(None) };
    }

    pub(super) fn report(event: Event) {
        COUNTS.with(|counts| {
            let mut now = counts.get();
            match event {
                Event::Loaded(_) => now.loads += 1,
                Event::Failed(_) => now.failed += 1,
                Event::Exchanged(..) => now.exchanges += 1,
            }
            counts.set(now);
        });
        let callback = CALLBACK.with(|slot| slot.borrow_mut().take());
        if let Some(mut callback) = callback {
            callback(event);
            CALLBACK.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.is_none() {
                    *slot = Some(callback);
                }
            });
        }
    }

    /// The counts since the last call, which resets them.
    pub(super) fn take_counts() -> Counts {
        COUNTS.with(|counts| counts.replace(Counts::default()))
    }

    /// Installs a callback, replacing any.
    pub(super) fn install(callback: impl FnMut(Event) + 'static) {
        CALLBACK.with(|slot| *slot.borrow_mut() = Some(Box::new(callback)));
    }

    pub(super) fn remove() {
        CALLBACK.with(|slot| *slot.borrow_mut() = None);
    }
}
