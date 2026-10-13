//! Tables, boundary tests and named witnesses (contract section 7.6).
//!
//! Each test is deterministic: every verb answers from a script, and none
//! sleeps or waits on a clock. Where a test forces a state through the
//! core's private fields, the forced state is not called reachable.

use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::rc::Rc;

// ---- a scripted shell ---------------------------------------------------------------------------

/// The payload of a scripted unwind.
struct Unwind;

/// Answers each verb from its queue; an empty queue answers success. The
/// verb called at position `unwind_at` unwinds instead of answering.
#[derive(Default)]
struct Script {
    open: VecDeque<Read<()>>,
    facts: VecDeque<Read<MemberFacts>>,
    insert: VecDeque<Insert>,
    commit: VecDeque<Commit>,
    rollback: VecDeque<Rollback>,
    row: VecDeque<Read<Option<Row>>>,
    explained: VecDeque<Read<Option<Seq>>>,
    acknowledgement: VecDeque<Read<Option<Seq>>>,
    frontier: VecDeque<Read<Seq>>,
    survey: VecDeque<Read<SetFacts>>,
    recheck: VecDeque<Read<SetFacts>>,
    present: VecDeque<Read<Option<Seq>>>,
    run: VecDeque<bool>,
    unwind_at: Option<usize>,
    calls: Vec<&'static str>,
}

const CLEAN: MemberFacts = MemberFacts {
    row: None,
    explained: None,
    corrected: None,
    holder: None,
    prepared: true,
};

impl Script {
    fn new() -> Script {
        Script::default()
    }

    fn unwinding_at(at: usize) -> Script {
        Script {
            unwind_at: Some(at),
            ..Script::default()
        }
    }

    fn call(&mut self, verb: &'static str) {
        if self.unwind_at == Some(self.calls.len()) {
            self.calls.push("unwind");
            resume_unwind(Box::new(Unwind));
        }
        self.calls.push(verb);
    }

    fn inserted(&self) -> Seq {
        100 + self.calls.iter().filter(|c| **c == "insert").count() as Seq
    }
}

impl Txn for Script {
    fn open(&mut self) -> Read<()> {
        self.call("open");
        self.open.pop_front().unwrap_or(Read::Got(()))
    }

    fn facts(&mut self, _index: u16, _member: &Member) -> Read<MemberFacts> {
        self.call("facts");
        self.facts.pop_front().unwrap_or(Read::Got(CLEAN))
    }

    fn insert(&mut self, _index: u16, _member: &Member) -> Insert {
        self.call("insert");
        let seq = self.inserted();
        self.insert.pop_front().unwrap_or(Insert::Inserted(seq))
    }

    fn commit(&mut self) -> Commit {
        self.call("commit");
        self.commit.pop_front().unwrap_or(Commit::Durable)
    }

    fn rollback(&mut self) -> Rollback {
        self.call("rollback");
        self.rollback.pop_front().unwrap_or(Rollback::Undone)
    }
}

impl Reads for Script {
    fn row(&mut self, _id: &[u8; ID_LEN]) -> Read<Option<Row>> {
        self.call("row");
        self.row.pop_front().unwrap_or(Read::Got(None))
    }

    fn explained(&mut self, _key: &RecordKey) -> Read<Option<Seq>> {
        self.call("explained");
        self.explained.pop_front().unwrap_or(Read::Got(None))
    }

    fn acknowledgement(&mut self, _set: &SetFacts, _nonce: &[u8; 16]) -> Read<Option<Seq>> {
        self.call("acknowledgement");
        self.acknowledgement.pop_front().unwrap_or(Read::Got(None))
    }

    fn frontier(&mut self) -> Read<Seq> {
        self.call("frontier");
        self.frontier.pop_front().unwrap_or(Read::Got(0))
    }
}

impl AckTxn for Script {
    fn survey(&mut self) -> Read<SetFacts> {
        self.call("survey");
        self.survey.pop_front().unwrap_or(Read::Got(EMPTY_SET))
    }

    fn open(&mut self) -> Read<()> {
        self.call("open");
        self.open.pop_front().unwrap_or(Read::Got(()))
    }

    fn recheck(&mut self, _above: Seq, _upto: Seq) -> Read<SetFacts> {
        self.call("recheck");
        self.recheck.pop_front().unwrap_or(Read::Got(EMPTY_SET))
    }

    fn insert(&mut self, _snapshot: &Snapshot) -> Insert {
        self.call("insert");
        let seq = self.inserted();
        self.insert.pop_front().unwrap_or(Insert::Inserted(seq))
    }

    fn commit(&mut self) -> Commit {
        self.call("commit");
        self.commit.pop_front().unwrap_or(Commit::Durable)
    }

    fn rollback(&mut self) -> Rollback {
        self.call("rollback");
        self.rollback.pop_front().unwrap_or(Rollback::Undone)
    }
}

impl CellTxn for Script {
    fn open(&mut self) -> Read<()> {
        self.call("open");
        self.open.pop_front().unwrap_or(Read::Got(()))
    }

    fn present(&mut self, _occupant: &Occupant) -> Read<Option<Seq>> {
        self.call("present");
        self.present.pop_front().unwrap_or(Read::Got(None))
    }

    fn insert(&mut self, _occupant: &Occupant) -> Insert {
        self.call("insert");
        let seq = self.inserted();
        self.insert.pop_front().unwrap_or(Insert::Inserted(seq))
    }

    fn commit(&mut self) -> Commit {
        self.call("commit");
        self.commit.pop_front().unwrap_or(Commit::Durable)
    }

    fn rollback(&mut self) -> Rollback {
        self.call("rollback");
        self.rollback.pop_front().unwrap_or(Rollback::Undone)
    }
}

impl BarrierRun for Script {
    fn run(&mut self) -> bool {
        self.call("run");
        self.run.pop_front().unwrap_or(true)
    }
}

/// Runs an operation; `None` if a scripted verb unwound. Any other panic
/// is resumed.
fn caught<T>(op: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(value) => Some(value),
        Err(payload) if payload.is::<Unwind>() => None,
        Err(payload) => resume_unwind(payload),
    }
}

// ---- records and ledgers ------------------------------------------------------------------------

const EMPTY_SET: SetFacts = SetFacts {
    frontier: 0,
    head: 0,
    top: 0,
    count: 0,
    digest: [0; 32],
};

fn bytes(a: u8, b: u8) -> [u8; 32] {
    let mut out = [0; 32];
    out[0] = a;
    out[1] = b;
    out
}

fn key(n: u8) -> RecordKey {
    RecordKey {
        id: bytes(1, n),
        digest: bytes(2, n),
    }
}

/// The same identity as `key(n)`, with other content.
fn other_content(n: u8) -> RecordKey {
    RecordKey {
        id: bytes(1, n),
        digest: bytes(3, n),
    }
}

fn fact(n: u8) -> FactRecord {
    FactRecord::new(Member {
        key: key(n),
        part: None,
    })
}

fn fact_with(record: RecordKey, commitment: u8, role: Role) -> FactRecord {
    FactRecord::new(Member {
        key: record,
        part: Some(Part {
            commitment: bytes(9, commitment),
            role,
        }),
    })
}

fn gating(n: u8) -> GatingRecord {
    GatingRecord::new(Member {
        key: key(n),
        part: None,
    })
}

fn holder_facts() -> MemberFacts {
    MemberFacts {
        holder: Some(Holder {
            id: bytes(7, 1),
            seq: 3,
        }),
        ..CLEAN
    }
}

fn limits() -> Limits {
    Limits {
        tickets: 8,
        refusals: 4,
    }
}

/// An open ledger and its worker.
fn open_core() -> (Core, Health, Worker) {
    let (mut core, health) = Core::new(limits());
    assert!(core.session_opened());
    let worker = core.take_worker().expect("the worker, once");
    (core, health, worker)
}

fn not_written(conflict_member: Option<u16>) -> Counted {
    Counted {
        persistence: Persistence::NotWritten,
        conflict_member,
    }
}

fn uncertain() -> Counted {
    Counted {
        persistence: Persistence::Uncertain,
        conflict_member: None,
    }
}

/// Places an explanation of `key(n)` into X, by a fact append that finds
/// its role already held.
fn explain_into_x(core: &mut Core, health: &Health, n: u8) -> CellToken {
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        holder: Some(Holder {
            id: bytes(7, n),
            seq: 3,
        }),
        ..CLEAN
    }));
    let outcome = core.append_facts(&mut ticket, &[fact_with(key(n), n, Role::Started)], &mut io);
    assert!(
        matches!(outcome, FactOutcome::NotApplied { .. }),
        "{outcome:?}"
    );
    ticket.confirm();
    core.agenda().x.expect("X holds the explanation").0
}

/// Leaves a gating record in U, its append having returned Uncertain.
fn gating_uncertain(core: &mut Core, n: u8) {
    let mut io = Script::new();
    io.commit.push_back(Commit::Unknown);
    let outcome = core.append_gating(&gating(n), &mut io);
    assert_eq!(
        outcome,
        GatingOutcome::Refused {
            persistence: Persistence::Uncertain,
            reason: Reason::Storage
        }
    );
}

fn pass_barrier(core: &mut Core, worker: &mut Worker) {
    assert_eq!(
        core.barrier(worker, &mut Script::new()),
        BarrierOutcome::Passed
    );
}

fn u_token(core: &Core) -> CellToken {
    core.agenda().u.expect("U is occupied")
}

fn c_token(core: &Core) -> CellToken {
    core.agenda().c.expect("C is occupied")
}

fn set(frontier: Seq, head: Seq, top: Seq, count: u64, digest: u8) -> SetFacts {
    SetFacts {
        frontier,
        head,
        top,
        count,
        digest: bytes(digest, 0),
    }
}

/// A snapshot of `set`, taken from this ledger.
fn snapshot_of(core: &mut Core, set: SetFacts) -> Snapshot {
    let mut io = Script::new();
    io.survey.push_back(Read::Got(set));
    match core.snapshot([7; 16], &mut io) {
        SnapshotOutcome::Taken(snapshot) => snapshot,
        other => panic!("no snapshot: {other:?}"),
    }
}

// ---- T-MATRIX -----------------------------------------------------------------------------------

const SESSIONS: [Session; 3] = [Session::Bootstrap, Session::Open, Session::Sealed];
const PHASES: [Phase; 3] = [Phase::Healthy, Phase::NeedBarrier, Phase::Resolving];

fn force(core: &mut Core, session: Session, latched: bool, phase: Phase) {
    core.session = session;
    core.phase = phase;
    if latched {
        core.latch = Some(LatchCause::Corrupt);
    }
}

/// The first failing entry test of 5.3, left to right, for operations that
/// need the session open.
fn open_entry(session: Session, latched: bool, phase_ok: bool) -> Option<Entry> {
    if session != Session::Open {
        Some(Entry::NotOpen)
    } else if latched {
        Some(Entry::Latched)
    } else if !phase_ok {
        Some(Entry::NotHealthy)
    } else {
        None
    }
}

/// T-MATRIX: section 5.3 and 4.9 in every session state, latch state and
/// phase, with the order of refusals, and each wrong-ledger case first.
#[test]
fn t_matrix_every_operation_in_every_state() {
    for session in SESSIONS {
        for latched in [false, true] {
            for phase in PHASES {
                let state = format!("{session:?} latched={latched} {phase:?}");
                let healthy = phase == Phase::Healthy;

                // Fact append: bound value; session (bootstrap); latch; phase.
                let (mut core, health, _worker) = open_core();
                let (_other_core, other_health) = Core::new(limits());
                force(&mut core, session, latched, phase);
                let mut foreign = other_health.arm().expect("a ticket");
                let outcome = core.append_facts(&mut foreign, &[fact(1)], &mut Script::new());
                assert_eq!(
                    outcome,
                    FactOutcome::Rejected(TicketFault::OtherLedger),
                    "{state}"
                );
                assert!(core.latch.is_some(), "{state}: a foreign ticket latches");
                let (mut core, health2, _worker) = open_core();
                drop(health);
                force(&mut core, session, latched, phase);
                let mut ticket = health2.arm().expect("a ticket");
                let mut io = Script::new();
                let outcome = core.append_facts(&mut ticket, &[fact(1)], &mut io);
                let expected = if session == Session::Bootstrap {
                    Some(Entry::NotOpen)
                } else if latched {
                    Some(Entry::Latched)
                } else if !healthy {
                    Some(Entry::NotHealthy)
                } else {
                    None
                };
                match expected {
                    Some(entry) => {
                        assert_eq!(
                            outcome,
                            FactOutcome::NotApplied {
                                counted: not_written(None),
                                reason: Reason::Entry(entry)
                            },
                            "{state}"
                        );
                        assert!(io.calls.is_empty(), "{state}: no transaction");
                    }
                    None if session == Session::Sealed => assert_eq!(
                        outcome,
                        FactOutcome::NotApplied {
                            counted: not_written(None),
                            reason: Reason::SessionClosed { member: 0 }
                        },
                        "{state}"
                    ),
                    None => assert_eq!(outcome, FactOutcome::Committed, "{state}"),
                }
                ticket.confirm();

                // Gating append.
                let (mut core, _health, _worker) = open_core();
                force(&mut core, session, latched, phase);
                let outcome = core.append_gating(&gating(1), &mut Script::new());
                match open_entry(session, latched, healthy) {
                    Some(entry) => assert_eq!(
                        outcome,
                        GatingOutcome::Refused {
                            persistence: Persistence::NotWritten,
                            reason: Reason::Entry(entry)
                        },
                        "{state}"
                    ),
                    None => assert_eq!(outcome, GatingOutcome::Committed(101), "{state}"),
                }

                // Lookup: the phase must not be barrier needed.
                let (mut core, _health, _worker) = open_core();
                force(&mut core, session, latched, phase);
                let outcome = core.lookup(&key(1), &mut Script::new());
                match open_entry(session, latched, phase != Phase::NeedBarrier) {
                    Some(entry) => assert_eq!(outcome, Lookup::Refused(entry), "{state}"),
                    None => assert_eq!(outcome, Lookup::Absent, "{state}"),
                }

                // Explain: refused when not open or latched; Busy when not Healthy.
                let (mut core, _health, _worker) = open_core();
                force(&mut core, session, latched, phase);
                let outcome = core.explain(&key(1), &mut Script::new());
                match open_entry(session, latched, true) {
                    Some(entry) => assert_eq!(outcome, Explain::Refused(entry), "{state}"),
                    None if !healthy => assert_eq!(outcome, Explain::Busy, "{state}"),
                    None => assert_eq!(outcome, Explain::Unknown, "{state}"),
                }

                // Snapshot.
                let (mut core, _health, _worker) = open_core();
                force(&mut core, session, latched, phase);
                let outcome = core.snapshot([1; 16], &mut Script::new());
                match open_entry(session, latched, healthy) {
                    Some(entry) => assert!(
                        matches!(outcome, SnapshotOutcome::NotNow(e) if e == entry),
                        "{state}"
                    ),
                    None => assert!(matches!(outcome, SnapshotOutcome::Nothing), "{state}"),
                }

                // Acknowledgement: a snapshot of another store first.
                let (mut other, _other_health, _other_worker) = open_core();
                let foreign = snapshot_of(&mut other, set(0, 2, 2, 1, 1));
                let (mut core, _health, _worker) = open_core();
                let own = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
                force(&mut core, session, latched, phase);
                assert_eq!(
                    core.append_acknowledgement(foreign, &mut Script::new()),
                    AckOutcome::Refused(AckRefusal::OtherStore),
                    "{state}"
                );
                let mut io = Script::new();
                io.recheck.push_back(Read::Got(set(0, 2, 2, 1, 1)));
                let outcome = core.append_acknowledgement(own, &mut io);
                match open_entry(session, latched, healthy) {
                    Some(entry) => assert_eq!(
                        outcome,
                        AckOutcome::Refused(AckRefusal::NotNow(entry)),
                        "{state}"
                    ),
                    None => assert_eq!(outcome, AckOutcome::Committed(2), "{state}"),
                }

                // Barrier: any session, any latch; only when barrier needed.
                let (mut core, _health, mut worker) = open_core();
                let (mut other, _other_health) = Core::new(limits());
                let mut foreign_worker = other.take_worker().expect("a worker");
                force(&mut core, session, latched, phase);
                assert_eq!(
                    core.barrier(&mut foreign_worker, &mut Script::new()),
                    BarrierOutcome::OtherLedger,
                    "{state}"
                );
                let expected = if phase == Phase::NeedBarrier {
                    BarrierOutcome::Passed
                } else {
                    BarrierOutcome::NotNeeded
                };
                assert_eq!(
                    core.barrier(&mut worker, &mut Script::new()),
                    expected,
                    "{state}"
                );
                if !latched {
                    assert!(core.latch.is_none(), "{state}: a barrier never latches");
                }

                // Resolve, with U's token: worker; token; session; latch; phase resolving.
                let (mut core, _health, mut worker) = open_core();
                gating_uncertain(&mut core, 1);
                let token = u_token(&core);
                force(&mut core, session, latched, phase);
                assert!(
                    matches!(
                        core.resolve(&mut foreign_worker, token.clone(), &mut Script::new()),
                        Resolve::OtherLedger
                    ),
                    "{state}"
                );
                let outcome = core.resolve(&mut worker, token, &mut Script::new());
                match open_entry(session, latched, phase == Phase::Resolving) {
                    Some(entry) => assert!(
                        matches!(outcome, Resolve::Refused(e) if e == entry),
                        "{state}: {outcome:?}"
                    ),
                    None => assert!(matches!(outcome, Resolve::Absent), "{state}: {outcome:?}"),
                }

                // Cell append, with C's token.
                let (mut core, _health, mut worker) = open_core();
                gating_uncertain(&mut core, 1);
                pass_barrier(&mut core, &mut worker);
                let mut io = Script::new();
                io.row.push_back(Read::Got(Some(Row {
                    seq: 9,
                    digest: key(1).digest,
                })));
                assert!(matches!(
                    core.resolve(&mut worker, u_token(&core), &mut io),
                    Resolve::Corrected(_)
                ));
                let token = c_token(&core);
                force(&mut core, session, latched, phase);
                assert_eq!(
                    core.append_cell(&mut foreign_worker, token.clone(), &mut Script::new()),
                    CellAppend::OtherLedger,
                    "{state}"
                );
                let outcome = core.append_cell(&mut worker, token, &mut Script::new());
                match open_entry(session, latched, healthy) {
                    Some(entry) => assert_eq!(outcome, CellAppend::Refused(entry), "{state}"),
                    None => assert_eq!(outcome, CellAppend::Committed(101), "{state}"),
                }

                // Empty X: any session, latch and phase, once X is committed.
                let (mut core, health, mut worker) = open_core();
                let token = explain_into_x(&mut core, &health, 1);
                assert_eq!(
                    core.append_cell(&mut worker, token.clone(), &mut Script::new()),
                    CellAppend::Committed(101)
                );
                force(&mut core, session, latched, phase);
                assert_eq!(
                    core.empty(&mut foreign_worker, token.clone()),
                    Emptying::OtherLedger,
                    "{state}"
                );
                assert_eq!(core.empty(&mut worker, token), Emptying::Done, "{state}");

                // The agenda, view, arm, confirm, expire and take_worker: any state.
                let (mut core, health) = Core::new(limits());
                force(&mut core, session, latched, phase);
                let agenda = core.agenda();
                assert_eq!(
                    (agenda.session, agenda.phase, agenda.latch.is_some()),
                    (session, phase, latched)
                );
                let mut ticket = health.arm().expect("arm in any state");
                assert!(matches!(ticket.expire(), FactOutcome::NotApplied { .. }));
                ticket.confirm();
                health.arm().expect("arm in any state").confirm();
                assert_eq!(health.view().live(), 0, "{state}");
                assert!(core.take_worker().is_some(), "{state}");
                assert!(core.take_worker().is_none(), "{state}: once");

                // Opened (4.9): only from bootstrap.
                let (mut core, health) = Core::new(limits());
                force(&mut core, session, latched, phase);
                assert_eq!(
                    core.session_opened(),
                    session == Session::Bootstrap,
                    "{state}"
                );
                let expected = if session == Session::Bootstrap {
                    Session::Open
                } else {
                    session
                };
                assert_eq!(health.view().session(), expected, "{state}");

                // Sealed (4.9): open, Healthy, not latched, cells empty.
                let (mut core, health) = Core::new(limits());
                force(&mut core, session, latched, phase);
                let sealed = session == Session::Open && healthy && !latched;
                assert_eq!(core.session_sealed(), sealed, "{state}");
                assert_eq!(
                    health.view().session() == Session::Sealed,
                    sealed || session == Session::Sealed
                );
            }
        }
    }
}

/// T-MATRIX, the cells column: C or X occupied refuses a gating append and
/// an acknowledgement; U occupied while Healthy is a broken invariant and
/// latches; sealing needs empty cells.
#[test]
fn t_matrix_cells() {
    // C occupied.
    let (mut core, _health, mut worker) = open_core();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    let mut io = Script::new();
    io.row.push_back(Read::Got(Some(Row {
        seq: 9,
        digest: key(1).digest,
    })));
    assert!(matches!(
        core.resolve(&mut worker, u_token(&core), &mut io),
        Resolve::Corrected(_)
    ));
    let busy = GatingOutcome::Refused {
        persistence: Persistence::NotWritten,
        reason: Reason::Entry(Entry::CellsBusy),
    };
    assert_eq!(core.append_gating(&gating(2), &mut Script::new()), busy);
    let snapshot = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut Script::new()),
        AckOutcome::Refused(AckRefusal::NotNow(Entry::CellsBusy))
    );
    assert!(!core.session_sealed());
    // X occupied.
    let (mut core, health, _worker) = open_core();
    explain_into_x(&mut core, &health, 1);
    assert_eq!(core.append_gating(&gating(2), &mut Script::new()), busy);
    let snapshot = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut Script::new()),
        AckOutcome::Refused(AckRefusal::NotNow(Entry::CellsBusy))
    );
    assert!(!core.session_sealed());
    // U occupied while Healthy: forced; a broken invariant latches.
    let (mut core, _health, _worker) = open_core();
    gating_uncertain(&mut core, 1);
    core.phase = Phase::Healthy;
    assert_eq!(
        core.append_gating(&gating(2), &mut Script::new()),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Entry(Entry::Latched)
        }
    );
    assert_eq!(core.latch, Some(LatchCause::CellInvariant));
    let (mut core, _health, _worker) = open_core();
    let snapshot = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
    gating_uncertain(&mut core, 1);
    core.phase = Phase::Healthy;
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut Script::new()),
        AckOutcome::Refused(AckRefusal::NotNow(Entry::Latched))
    );
    assert_eq!(core.latch, Some(LatchCause::CellInvariant));
    // A sealed store refuses every operation of section 4 but the listed
    // ones; a fact append answers rows a to c and writes nothing.
    let (mut core, health, mut worker) = open_core();
    assert!(core.session_sealed());
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        row: Some(Row {
            seq: 4,
            digest: key(1).digest,
        }),
        ..CLEAN
    }));
    io.facts.push_back(Read::Got(CLEAN));
    assert_eq!(
        core.append_facts(&mut ticket, &[fact(1)], &mut io),
        FactOutcome::Committed
    );
    let outcome = core.append_facts(&mut ticket, &[fact(1)], &mut io);
    assert_eq!(
        outcome,
        FactOutcome::NotApplied {
            counted: not_written(None),
            reason: Reason::SessionClosed { member: 0 }
        }
    );
    assert!(!io.calls.contains(&"insert"));
    assert_eq!(
        core.barrier(&mut worker, &mut Script::new()),
        BarrierOutcome::NotNeeded
    );
    ticket.confirm();
}

// ---- T-PREC -------------------------------------------------------------------------------------

/// Runs one fact append on a fresh open ledger; returns the outcome (or
/// `None` if it unwound), the ticket's count, the view and the verbs.
fn append_once(
    batch: &[FactRecord],
    io: &mut Script,
    prepare: impl FnOnce(&mut Core),
) -> (
    Option<FactOutcome>,
    Option<Counted>,
    HealthView,
    Phase,
    Option<LatchCause>,
) {
    let (mut core, health, _worker) = open_core();
    prepare(&mut core);
    let mut ticket = health.arm().expect("a ticket");
    let outcome = caught(|| core.append_facts(&mut ticket, batch, io));
    let counted = ticket.counted();
    let view = health.view();
    let result = (outcome, counted, view, core.phase, core.latch);
    ticket.confirm();
    assert_eq!(health.view().live(), 0);
    assert_eq!(health.view().unapplied(), 0);
    result
}

/// T-PREC: section 4.4's persistence table, every row, for one member and
/// for a batch, and every malformed shape. Every refused or failed append
/// is counted, including one refused at entry.
#[test]
fn t_prec_persistence_of_every_ending() {
    let one = [fact_with(key(1), 1, Role::Started)];
    let two = [
        fact_with(key(1), 1, Role::Started),
        fact_with(key(2), 2, Role::Started),
    ];
    for batch in [&one[..], &two[..]] {
        let last = (batch.len() - 1) as u16;
        // Refused at entry: Not written, counted, no transaction.
        let mut io = Script::new();
        let (outcome, counted, view, phase, _) =
            append_once(batch, &mut io, |core| core.phase = Phase::NeedBarrier);
        assert_eq!(
            outcome,
            Some(FactOutcome::NotApplied {
                counted: not_written(None),
                reason: Reason::Entry(Entry::NotHealthy)
            })
        );
        assert_eq!(
            counted,
            Some(not_written(None)),
            "an append refused at entry is counted"
        );
        assert_eq!(
            view.unapplied(),
            1,
            "an append refused at entry is counted in the word"
        );
        assert_eq!(phase, Phase::NeedBarrier);
        assert!(io.calls.is_empty());

        // Refused in step 3, whatever the rollback answers: Not written, the
        // member named, no insert.
        for rollback in [Rollback::Undone, Rollback::Unknown] {
            let mut io = Script::new();
            if batch.len() == 2 {
                io.facts.push_back(Read::Got(CLEAN));
            }
            io.facts.push_back(Read::Got(holder_facts()));
            io.rollback.push_back(rollback);
            let (outcome, counted, view, phase, _) = append_once(batch, &mut io, |_| {});
            assert!(
                matches!(outcome, Some(FactOutcome::NotApplied { reason: Reason::Conflict { member, .. }, .. }) if member == last)
            );
            assert_eq!(counted, Some(not_written(Some(last))));
            assert_eq!(view.unapplied(), 1);
            assert!(!io.calls.contains(&"insert"));
            assert_eq!(io.calls.iter().filter(|c| **c == "rollback").count(), 1);
            let expected = if rollback == Rollback::Unknown {
                Phase::NeedBarrier
            } else {
                Phase::Healthy
            };
            assert_eq!(
                phase, expected,
                "a failed rollback can make the phase worse"
            );
        }

        // A failure in step 5: Not written if the rollback was Undone,
        // Uncertain if not; a constraint latches.
        for (insert, reason) in [
            (Insert::Failed, Reason::Storage),
            (Insert::Constraint, Reason::Constraint),
        ] {
            for (rollback, persistence) in [
                (Rollback::Undone, Persistence::NotWritten),
                (Rollback::Unknown, Persistence::Uncertain),
            ] {
                let mut io = Script::new();
                if batch.len() == 2 {
                    io.insert.push_back(Insert::Inserted(50));
                }
                io.insert.push_back(insert);
                io.rollback.push_back(rollback);
                let (outcome, counted, _, phase, latch) = append_once(batch, &mut io, |_| {});
                let expected = Counted {
                    persistence,
                    conflict_member: None,
                };
                assert_eq!(
                    outcome,
                    Some(FactOutcome::NotApplied {
                        counted: expected,
                        reason
                    })
                );
                assert_eq!(counted, Some(expected));
                assert_eq!(phase, Phase::NeedBarrier);
                assert_eq!(latch.is_some(), insert == Insert::Constraint);
                assert!(
                    !io.calls.contains(&"commit"),
                    "no commit after a failed insert"
                );
            }
        }

        // A commit answered Unknown: Uncertain, whatever a rollback then reports.
        for rollback in [Rollback::Undone, Rollback::Unknown] {
            let mut io = Script::new();
            io.commit.push_back(Commit::Unknown);
            io.rollback.push_back(rollback);
            let (outcome, counted, _, phase, _) = append_once(batch, &mut io, |_| {});
            assert_eq!(
                outcome,
                Some(FactOutcome::NotApplied {
                    counted: uncertain(),
                    reason: Reason::Storage
                })
            );
            assert_eq!(counted, Some(uncertain()));
            assert_eq!(phase, Phase::NeedBarrier);
        }

        // A durable commit: Committed, not counted, no rollback.
        let mut io = Script::new();
        let (outcome, counted, view, _, _) = append_once(batch, &mut io, |_| {});
        assert_eq!(outcome, Some(FactOutcome::Committed));
        assert_eq!(counted, None);
        assert_eq!(view.unapplied(), 0);
        assert!(!io.calls.contains(&"rollback"));

        // Every member already present: one rollback of the empty transaction.
        let present = MemberFacts {
            row: Some(Row {
                seq: 4,
                digest: key(1).digest,
            }),
            ..CLEAN
        };
        let mut io = Script::new();
        for (n, _) in batch.iter().enumerate() {
            let mut facts = present;
            facts.row = Some(Row {
                seq: 4 + n as Seq,
                digest: key(1 + n as u8).digest,
            });
            io.facts.push_back(Read::Got(facts));
        }
        let (outcome, counted, _, _, _) = append_once(batch, &mut io, |_| {});
        assert_eq!(outcome, Some(FactOutcome::Committed));
        assert_eq!(counted, None);
        assert_eq!(io.calls.last(), Some(&"rollback"));
        assert!(!io.calls.contains(&"insert"));

        // A verb unwound before the first insert: Not written. Once an insert
        // was issued: Uncertain. The guard counts the ticket either way.
        let verbs = io_positions(batch.len());
        for (at, verb) in verbs.iter().enumerate() {
            let mut io = Script::unwinding_at(at);
            let (outcome, counted, view, phase, _) = append_once(batch, &mut io, |_| {});
            assert_eq!(outcome, None, "{verb} unwinds");
            let issued = verbs[..=at].contains(&"insert");
            let expected = if issued {
                uncertain()
            } else {
                not_written(None)
            };
            assert_eq!(counted, Some(expected), "unwound at {verb}");
            assert_eq!(view.unapplied(), 1);
            assert_eq!(phase, Phase::NeedBarrier);
            assert_eq!(io.calls.last(), Some(&"unwind"), "no verb after an unwind");
        }

        // A conflict already found is kept when the rollback then unwinds.
        let mut io = Script::unwinding_at(batch.len() + 1);
        if batch.len() == 2 {
            io.facts.push_back(Read::Got(CLEAN));
        }
        io.facts.push_back(Read::Got(holder_facts()));
        let (outcome, counted, _, _, _) = append_once(batch, &mut io, |_| {});
        assert_eq!(outcome, None);
        assert_eq!(counted, Some(not_written(Some(last))));
    }

    // An expiry before entry: Not written, counted.
    let (_core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    assert_eq!(
        ticket.expire(),
        FactOutcome::NotApplied {
            counted: not_written(None),
            reason: Reason::Entry(Entry::Expired)
        }
    );
    assert_eq!(health.view().unapplied(), 1);
    ticket.confirm();

    // Every malformed shape: refused whole, names no conflict, tables
    // nothing, no transaction.
    let too_many: Vec<FactRecord> = (0..=BATCH_MAX)
        .map(|n| {
            FactRecord::new(Member {
                key: RecordKey {
                    id: bytes(1, (n % 250) as u8).map(|b| b ^ (n / 250) as u8),
                    digest: bytes(2, 0),
                },
                part: None,
            })
        })
        .collect();
    let shapes: Vec<(&str, Vec<FactRecord>)> = vec![
        ("empty", Vec::new()),
        ("257 members", too_many),
        (
            "one identity under two contents",
            vec![
                fact(1),
                FactRecord::new(Member {
                    key: other_content(1),
                    part: None,
                }),
            ],
        ),
        (
            "one commitment and role twice",
            vec![
                fact_with(key(1), 5, Role::Started),
                fact_with(key(2), 5, Role::Started),
            ],
        ),
        (
            "Started before its Prepared",
            vec![
                fact_with(key(1), 5, Role::Started),
                fact_with(key(2), 5, Role::Prepared),
            ],
        ),
        (
            "End before its Prepared",
            vec![
                fact_with(key(1), 5, Role::End),
                fact_with(key(2), 5, Role::Prepared),
            ],
        ),
    ];
    for (shape, batch) in shapes {
        let (mut core, health, _worker) = open_core();
        let mut ticket = health.arm().expect("a ticket");
        let mut io = Script::new();
        let outcome = core.append_facts(&mut ticket, &batch, &mut io);
        assert_eq!(
            outcome,
            FactOutcome::NotApplied {
                counted: not_written(None),
                reason: Reason::Entry(Entry::Malformed)
            },
            "{shape}"
        );
        assert!(
            io.calls.is_empty(),
            "{shape}: refused before any transaction"
        );
        assert!(
            core.refusals.is_empty() && core.x.is_none(),
            "{shape}: tables nothing"
        );
        assert_eq!(core.phase, Phase::Healthy, "{shape}");
        ticket.confirm();
    }
    // Well formed: Prepared before Started and End of its commitment.
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let batch = [
        fact_with(key(1), 5, Role::Prepared),
        fact_with(key(2), 5, Role::Started),
        fact_with(key(3), 5, Role::End),
    ];
    let mut io = Script::new();
    for _ in 0..3 {
        io.facts.push_back(Read::Got(MemberFacts {
            prepared: false,
            ..CLEAN
        }));
    }
    assert_eq!(
        core.append_facts(&mut ticket, &batch, &mut io),
        FactOutcome::Committed
    );
    ticket.confirm();
}

/// The verbs of a fact append that inserts every member, in order.
fn io_positions(members: usize) -> Vec<&'static str> {
    let mut verbs = vec!["open"];
    verbs.extend(std::iter::repeat_n("facts", members));
    verbs.extend(std::iter::repeat_n("insert", members));
    verbs.push("commit");
    verbs
}

// ---- T-OWN --------------------------------------------------------------------------------------

/// Resolves to one implementation only for a type that is not `Clone`: for
/// a cloneable type the call below is ambiguous and the file does not
/// build (E0283).
trait NotClone<Marker> {
    fn check() {}
}

impl<T: ?Sized> NotClone<()> for T {}

impl<T: Clone> NotClone<u8> for T {}

/// T-OWN: a ticket, a snapshot and the worker cannot be cloned; and a
/// ticket is armed, lent, counted and confirmed.
#[test]
fn t_own_tickets_snapshots_and_the_worker_cannot_be_cloned() {
    <Ticket as NotClone<_>>::check();
    <Snapshot as NotClone<_>>::check();
    <Worker as NotClone<_>>::check();
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("armed");
    assert_eq!((health.view().live(), health.view().unapplied()), (1, 0));
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    let outcome = core.append_facts(&mut ticket, &[fact(1)], &mut io);
    assert!(matches!(outcome, FactOutcome::NotApplied { .. }));
    assert_eq!(ticket.counted(), Some(not_written(None)));
    assert_eq!((health.view().live(), health.view().unapplied()), (1, 1));
    ticket.confirm();
    assert_eq!((health.view().live(), health.view().unapplied()), (0, 0));
    assert!(!health.view().handover_fault());
}

// ---- T-BIND -------------------------------------------------------------------------------------

/// T-BIND: everything that carries authority is bound to one ledger by its
/// shared allocation. No counter of ledgers exists, so there is none to
/// bring near its end: the case of finding PC-01 is an old worker and an
/// old token kept after their core is dropped.
#[test]
fn t_bind_every_bound_value_belongs_to_one_ledger() {
    // A ticket of another ledger: refused, latched, nothing counted.
    let (mut core, health, mut worker) = open_core();
    let (_other, other_health) = Core::new(limits());
    let mut foreign = other_health.arm().expect("a ticket");
    let mut io = Script::new();
    assert_eq!(
        core.append_facts(&mut foreign, &[fact(1)], &mut io),
        FactOutcome::Rejected(TicketFault::OtherLedger)
    );
    assert!(io.calls.is_empty());
    assert_eq!(core.latch, Some(LatchCause::TicketFault));
    assert_eq!(foreign.counted(), None);
    assert_eq!(
        (health.view().unapplied(), other_health.view().unapplied()),
        (0, 0)
    );
    foreign.confirm();
    // A Counted ticket lent again: refused, latched.
    let (mut core2, health2, _worker2) = open_core();
    let mut ticket = health2.arm().expect("a ticket");
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    assert!(matches!(
        core2.append_facts(&mut ticket, &[fact(1)], &mut io),
        FactOutcome::NotApplied { .. }
    ));
    core2.phase = Phase::Healthy;
    assert_eq!(
        core2.append_facts(&mut ticket, &[fact(1)], &mut Script::new()),
        FactOutcome::Rejected(TicketFault::NotArmed)
    );
    assert_eq!(core2.latch, Some(LatchCause::TicketFault));
    assert_eq!(
        health2.view().unapplied(),
        1,
        "the second lending counted nothing"
    );
    ticket.confirm();
    // A snapshot of another ledger.
    let (mut core3, _health3, _worker3) = open_core();
    let snapshot = snapshot_of(&mut core3, set(0, 2, 2, 1, 1));
    let (mut core4, _health4, _worker4) = open_core();
    assert_eq!(
        core4.append_acknowledgement(snapshot, &mut Script::new()),
        AckOutcome::Refused(AckRefusal::OtherStore)
    );
    // A worker of another ledger, for every worker operation.
    let (mut core5, health5, mut worker5) = open_core();
    let token = explain_into_x(&mut core5, &health5, 1);
    assert_eq!(
        core5.barrier(&mut worker, &mut Script::new()),
        BarrierOutcome::OtherLedger
    );
    assert_eq!(
        core5.append_cell(&mut worker, token.clone(), &mut Script::new()),
        CellAppend::OtherLedger
    );
    assert_eq!(
        core5.empty(&mut worker, token.clone()),
        Emptying::OtherLedger
    );
    // A token of another ledger, of another cell, and of an old occupancy.
    let (mut core6, _health6, mut worker6) = open_core();
    gating_uncertain(&mut core6, 1);
    pass_barrier(&mut core6, &mut worker6);
    let own_u = u_token(&core6);
    assert_eq!(
        own_u.occupancy, token.occupancy,
        "the same occupancy number in another ledger"
    );
    assert!(matches!(
        core6.resolve(&mut worker6, token.clone(), &mut Script::new()),
        Resolve::Stale
    ));
    assert_eq!(
        core6.append_cell(&mut worker6, token.clone(), &mut Script::new()),
        CellAppend::Stale
    );
    assert!(
        core6.u.is_some() && core6.latch.is_none(),
        "a stale token changes nothing"
    );
    drop(core);
    assert!(matches!(
        core5.resolve(&mut worker5, token.clone(), &mut Script::new()),
        Resolve::Stale
    ));
    assert_eq!(
        core5.append_cell(&mut worker5, token.clone(), &mut Script::new()),
        CellAppend::Committed(101)
    );
    assert_eq!(core5.empty(&mut worker5, token.clone()), Emptying::Done);
    let refilled = explain_into_x(&mut core5, &health5, 2);
    assert_eq!(refilled.occupancy, token.occupancy + 1);
    assert_eq!(
        core5.append_cell(&mut worker5, token.clone(), &mut Script::new()),
        CellAppend::Stale
    );
    assert_eq!(core5.empty(&mut worker5, token), Emptying::Stale);

    // Finding PC-01: keep a worker and a token of ledger one; drop ledger
    // one's core; create and drop many ledgers; bring a new ledger to the
    // same cell and the same occupancy number.
    let (mut one, one_health, old_worker) = open_core();
    let old_token = explain_into_x(&mut one, &one_health, 1);
    let (cell, occupancy) = (old_token.cell, old_token.occupancy);
    drop(one);
    drop(one_health);
    for _ in 0..1000 {
        drop(Core::new(limits()));
    }
    let (mut new, new_health, mut new_worker) = open_core();
    let token = explain_into_x(&mut new, &new_health, 1);
    assert_eq!(
        (token.cell, token.occupancy),
        (cell, occupancy),
        "the same cell and occupancy number"
    );
    let mut old_worker = old_worker;
    assert_eq!(
        new.barrier(&mut old_worker, &mut Script::new()),
        BarrierOutcome::OtherLedger
    );
    assert_eq!(
        new.append_cell(&mut old_worker, token.clone(), &mut Script::new()),
        CellAppend::OtherLedger
    );
    assert_eq!(new.empty(&mut old_worker, token), Emptying::OtherLedger);
    assert_eq!(
        new.append_cell(&mut new_worker, old_token.clone(), &mut Script::new()),
        CellAppend::Stale,
        "the old token is stale against the new ledger's own worker"
    );
    assert_eq!(new.empty(&mut new_worker, old_token), Emptying::Stale);
    assert!(new.x.is_some(), "a stale token changed nothing");
}

// ---- T-GATE -------------------------------------------------------------------------------------

/// T-GATE: section 4.5's table of a record offered again, every row.
#[test]
fn t_gate_every_re_offer() {
    let committed = MemberFacts {
        row: Some(Row {
            seq: 8,
            digest: key(1).digest,
        }),
        ..CLEAN
    };
    // Committed, no correction; offered again while Healthy, cells empty:
    // Committed again with the row's sequence, as already present. U is
    // filled and emptied inside the call; nothing is written.
    let (mut core, _health, _worker) = open_core();
    let mut io = Script::new();
    io.facts.push_back(Read::Got(committed));
    let before = core.occupancies;
    assert_eq!(
        core.append_gating(&gating(1), &mut io),
        GatingOutcome::Committed(8)
    );
    assert_eq!(io.calls, ["open", "facts", "rollback"]);
    assert!(core.u.is_none() && core.c.is_none() && core.x.is_none());
    assert_eq!(
        core.occupancies[0],
        before[0] + 1,
        "U was filled and emptied in the call"
    );
    // Committed; offered again while an entry rule refuses: the entry
    // refusal; U not touched; no correction exists or is made.
    for refuse in [Phase::NeedBarrier, Phase::Resolving] {
        let (mut core, _health, _worker) = open_core();
        core.phase = refuse;
        let mut io = Script::new();
        io.facts.push_back(Read::Got(committed));
        assert_eq!(
            core.append_gating(&gating(1), &mut io),
            GatingOutcome::Refused {
                persistence: Persistence::NotWritten,
                reason: Reason::Entry(Entry::NotHealthy)
            }
        );
        assert_eq!(core.occupancies, [0, 0, 0], "U not touched");
        assert!(core.c.is_none(), "no correction");
    }
    // Ended Uncertain or unwound, still in U: the entry refusal at any time.
    for unwound in [false, true] {
        let (mut core, _health, mut worker) = open_core();
        if unwound {
            let mut io = Script::unwinding_at(3);
            assert_eq!(caught(|| core.append_gating(&gating(1), &mut io)), None);
        } else {
            gating_uncertain(&mut core, 1);
        }
        let refusal = GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Entry(Entry::NotHealthy),
        };
        assert_eq!(core.append_gating(&gating(1), &mut Script::new()), refusal);
        pass_barrier(&mut core, &mut worker);
        assert_eq!(core.append_gating(&gating(1), &mut Script::new()), refusal);
    }
    // Resolved as present: its correction in C: the entry refusal.
    let (mut core, _health, mut worker) = open_core();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    let mut io = Script::new();
    io.row.push_back(Read::Got(Some(Row {
        seq: 8,
        digest: key(1).digest,
    })));
    let Resolve::Corrected(c) = core.resolve(&mut worker, u_token(&core), &mut io) else {
        panic!("a correction");
    };
    assert_eq!(
        core.append_gating(&gating(1), &mut Script::new()),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Entry(Entry::CellsBusy)
        }
    );
    // Its correction committed: refused, Not written, corrected; tested
    // before the ordinary Already present path.
    assert_eq!(
        core.append_cell(&mut worker, c, &mut Script::new()),
        CellAppend::Committed(101)
    );
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        corrected: Some(101),
        ..committed
    }));
    assert_eq!(
        core.append_gating(&gating(1), &mut io),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Corrected { seq: 101 }
        }
    );
    assert!(!io.calls.contains(&"insert"));
    assert!(core.u.is_none() && core.c.is_none());
    // A fact record has no correction and is not affected.
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        corrected: Some(101),
        ..committed
    }));
    assert_eq!(
        core.append_facts(&mut ticket, &[fact(1)], &mut io),
        FactOutcome::Committed
    );
    ticket.confirm();
    // Resolved as absent: a fresh attempt.
    let (mut core, _health, mut worker) = open_core();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    assert!(matches!(
        core.resolve(&mut worker, u_token(&core), &mut Script::new()),
        Resolve::Absent
    ));
    let mut io = Script::new();
    assert_eq!(
        core.append_gating(&gating(1), &mut io),
        GatingOutcome::Committed(101)
    );
    assert!(io.calls.contains(&"insert"));
    // Ended Not written: a fresh attempt.
    let (mut core, _health, mut worker) = open_core();
    let mut io = Script::new();
    io.insert.push_back(Insert::Failed);
    assert_eq!(
        core.append_gating(&gating(1), &mut io),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Storage
        }
    );
    assert!(core.u.is_none());
    pass_barrier(&mut core, &mut worker);
    assert_eq!(
        core.append_gating(&gating(1), &mut Script::new()),
        GatingOutcome::Committed(101)
    );
    // Refused as a conflict: Conflict again, as already explained or with a
    // cause built again from the same rows.
    let other = MemberFacts {
        row: Some(Row {
            seq: 2,
            digest: other_content(1).digest,
        }),
        ..CLEAN
    };
    let (mut core, _health, mut worker) = open_core();
    let mut io = Script::new();
    io.facts.push_back(Read::Got(other));
    let first = core.append_gating(&gating(1), &mut io);
    let GatingOutcome::Refused {
        persistence: Persistence::NotWritten,
        reason: Reason::Conflict {
            known: Known::Cause(cause),
            ..
        },
    } = first
    else {
        panic!("a conflict: {first:?}");
    };
    assert!(
        core.refusals.is_empty(),
        "a gating conflict makes no table entry"
    );
    let x = core.agenda().x.expect("X holds its explanation").0;
    assert_eq!(
        core.append_cell(&mut worker, x.clone(), &mut Script::new()),
        CellAppend::Committed(101)
    );
    assert_eq!(core.empty(&mut worker, x), Emptying::Done);
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        explained: Some(101),
        ..other
    }));
    assert_eq!(
        core.append_gating(&gating(1), &mut io),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Conflict {
                member: 0,
                known: Known::Explained(101)
            }
        }
    );
    let (mut core, _health, _worker) = open_core();
    let mut io = Script::new();
    io.facts.push_back(Read::Got(other));
    let again = core.append_gating(&gating(1), &mut io);
    assert_eq!(
        again,
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Conflict {
                member: 0,
                known: Known::Cause(cause)
            }
        },
        "the same cause, built again from the same rows"
    );
}

// ---- T-CELL -------------------------------------------------------------------------------------

/// T-CELL: section 4.7's two tables, row by row.
#[test]
fn t_cell_worker_operations_and_cell_appends() {
    // Barrier: passed to resolving if U is occupied, else Healthy; failed:
    // no change; not needed otherwise.
    let (mut core, _health, mut worker) = open_core();
    assert_eq!(
        core.barrier(&mut worker, &mut Script::new()),
        BarrierOutcome::NotNeeded
    );
    gating_uncertain(&mut core, 1);
    let mut io = Script::new();
    io.run.push_back(false);
    assert_eq!(core.barrier(&mut worker, &mut io), BarrierOutcome::Failed);
    assert_eq!(core.phase, Phase::NeedBarrier);
    pass_barrier(&mut core, &mut worker);
    assert_eq!(core.phase, Phase::Resolving);
    let (mut core, _health, mut worker) = open_core();
    core.phase = Phase::NeedBarrier;
    pass_barrier(&mut core, &mut worker);
    assert_eq!(core.phase, Phase::Healthy);

    // Appending C's occupant: present, or inserted and committed: C emptied.
    for present in [true, false] {
        let (mut core, _health, mut worker) = open_core();
        gating_uncertain(&mut core, 1);
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.row.push_back(Read::Got(Some(Row {
            seq: 8,
            digest: key(1).digest,
        })));
        let Resolve::Corrected(c) = core.resolve(&mut worker, u_token(&core), &mut io) else {
            panic!("a correction");
        };
        let mut io = Script::new();
        if present {
            io.present.push_back(Read::Got(Some(55)));
        }
        let expected = if present { 55 } else { 101 };
        assert_eq!(
            core.append_cell(&mut worker, c, &mut io),
            CellAppend::Committed(expected)
        );
        assert!(core.c.is_none(), "C emptied");
        let expected_calls: &[&str] = if present {
            &["open", "present", "rollback"]
        } else {
            &["open", "present", "insert", "commit"]
        };
        assert_eq!(io.calls, expected_calls);
    }

    // Appending X's occupant: present, or inserted and committed: X marked
    // committed with its sequence, the original's table entry removed.
    for present in [true, false] {
        let (mut core, health, mut worker) = open_core();
        let x = explain_into_x(&mut core, &health, 1);
        assert!(core.refusals.contains_key(&key(1)));
        let mut io = Script::new();
        if present {
            io.present.push_back(Read::Got(Some(55)));
        }
        let seq = if present { 55 } else { 101 };
        assert_eq!(
            core.append_cell(&mut worker, x.clone(), &mut io),
            CellAppend::Committed(seq)
        );
        assert!(matches!(core.x, Some((_, _, Stage::Committed(s))) if s == seq));
        assert!(
            !core.refusals.contains_key(&key(1)),
            "the table entry removed"
        );
        // Already marked: the same sequence again, no verb issued.
        let mut io = Script::new();
        assert_eq!(
            core.append_cell(&mut worker, x.clone(), &mut io),
            CellAppend::Committed(seq)
        );
        assert!(io.calls.is_empty());
        assert_eq!(core.empty(&mut worker, x.clone()), Emptying::Done);
        assert!(core.x.is_none());
        assert_eq!(
            core.empty(&mut worker, x),
            Emptying::Stale,
            "a repeated empty is stale"
        );
    }

    // Not written: the cell unchanged, phase as 4.4. Uncertain: unchanged,
    // barrier needed. A constraint: latched, Not written or Uncertain by
    // the rollback.
    let cases = [
        (
            "open failed",
            Some(Read::Failed),
            None,
            None,
            None,
            CellAppend::NotWritten,
            false,
        ),
        (
            "present failed",
            None,
            Some(Read::Failed),
            None,
            None,
            CellAppend::NotWritten,
            false,
        ),
        (
            "present corrupt",
            None,
            Some(Read::Corrupt),
            None,
            None,
            CellAppend::NotWritten,
            true,
        ),
        (
            "insert failed, undone",
            None,
            None,
            Some(Insert::Failed),
            None,
            CellAppend::NotWritten,
            false,
        ),
        (
            "insert failed, unknown",
            None,
            None,
            Some(Insert::Failed),
            Some(Rollback::Unknown),
            CellAppend::Uncertain,
            false,
        ),
        (
            "constraint, undone",
            None,
            None,
            Some(Insert::Constraint),
            None,
            CellAppend::NotWritten,
            true,
        ),
        (
            "constraint, unknown",
            None,
            None,
            Some(Insert::Constraint),
            Some(Rollback::Unknown),
            CellAppend::Uncertain,
            true,
        ),
    ];
    for (case, open, present, insert, rollback, expected, latches) in cases {
        let (mut core, health, mut worker) = open_core();
        let x = explain_into_x(&mut core, &health, 1);
        let mut io = Script::new();
        io.open.extend(open);
        io.present.extend(present);
        io.insert.extend(insert);
        io.rollback.extend(rollback);
        assert_eq!(
            core.append_cell(&mut worker, x, &mut io),
            expected,
            "{case}"
        );
        assert!(
            matches!(core.x, Some((_, _, Stage::Built))),
            "{case}: the cell unchanged"
        );
        assert_eq!(core.phase, Phase::NeedBarrier, "{case}");
        assert_eq!(core.latch.is_some(), latches, "{case}");
    }
    let (mut core, health, mut worker) = open_core();
    let x = explain_into_x(&mut core, &health, 1);
    let mut io = Script::new();
    io.commit.push_back(Commit::Unknown);
    assert_eq!(
        core.append_cell(&mut worker, x.clone(), &mut io),
        CellAppend::Uncertain
    );
    assert!(matches!(core.x, Some((_, _, Stage::Built))));
    assert_eq!(core.phase, Phase::NeedBarrier);

    // Empty X before it is marked committed: not committed, no change.
    assert_eq!(core.empty(&mut worker, x), Emptying::NotCommitted);
    assert!(core.x.is_some());
}

// ---- T-RES --------------------------------------------------------------------------------------

/// T-RES: resolve, every row, including the inconsistent one.
#[test]
fn t_res_every_resolve_row() {
    // A gating record: absent; present with an equal digest; present with a
    // different digest.
    let rows = [
        (None, "absent"),
        (Some(key(1).digest), "present"),
        (Some(other_content(1).digest), "different"),
    ];
    for (row, case) in rows {
        let (mut core, _health, mut worker) = open_core();
        gating_uncertain(&mut core, 1);
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.row
            .push_back(Read::Got(row.map(|digest| Row { seq: 8, digest })));
        let outcome = core.resolve(&mut worker, u_token(&core), &mut io);
        assert!(core.u.is_none(), "{case}: U emptied");
        assert_eq!(core.phase, Phase::Healthy, "{case}");
        match case {
            "absent" => assert!(matches!(outcome, Resolve::Absent)),
            "present" => {
                let Resolve::Corrected(token) = outcome else {
                    panic!("{outcome:?}");
                };
                assert_eq!(token.cell, Cell::C);
                assert_eq!(
                    core.c.as_ref().map(|(_, occupant)| occupant.clone()),
                    Some(Occupant::Correction {
                        about: key(1),
                        seq: 8,
                        exit: Exit::ReturnedUncertain
                    })
                );
            }
            _ => {
                let Resolve::ConflictPlaced(token) = outcome else {
                    panic!("{outcome:?}");
                };
                assert_eq!(token.cell, Cell::X);
                let Some((_, Occupant::Explanation { original, cause }, Stage::Built)) = &core.x
                else {
                    panic!("an explanation in X");
                };
                assert_eq!(*original, key(1));
                let CauseKind::IdentityContent {
                    id,
                    existing,
                    offered,
                } = cause.kind()
                else {
                    panic!("an identity conflict");
                };
                assert_eq!(
                    (id, existing.seq, existing.digest, offered),
                    (key(1).id, 8, other_content(1).digest, key(1).digest)
                );
                assert!(core.refusals.is_empty());
            }
        }
    }
    // An acknowledgement: absent; present with the durable frontier at
    // least N; present with the durable frontier below N (storage
    // contradicts itself: latched, U kept, the phase stays resolving).
    for (frontier, case) in [
        (None, "absent"),
        (Some(5), "at N"),
        (Some(9), "above N"),
        (Some(4), "below N"),
    ] {
        let (mut core, _health, mut worker) = open_core();
        ack_uncertain(&mut core, set(0, 5, 5, 1, 1));
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.acknowledgement.push_back(Read::Got(frontier.map(|_| 6)));
        io.frontier.extend(frontier.map(Read::Got));
        let outcome = core.resolve(&mut worker, u_token(&core), &mut io);
        match case {
            "absent" => {
                assert!(matches!(outcome, Resolve::Absent));
                assert!(core.u.is_none());
                assert_eq!(core.frontier, 0);
            }
            "below N" => {
                assert!(matches!(outcome, Resolve::Inconsistent));
                assert_eq!(core.latch, Some(LatchCause::Inconsistent));
                assert!(core.u.is_some(), "U kept");
                assert_eq!(core.phase, Phase::Resolving);
                assert_eq!(core.frontier, 0, "the qualified frontier does not rise");
            }
            _ => {
                assert!(matches!(outcome, Resolve::Acknowledged(5)));
                assert!(core.u.is_none());
                assert_eq!(core.frontier, 5, "the qualified frontier rises to N");
                assert_eq!(core.phase, Phase::Healthy);
            }
        }
    }
    // Either: the read failed: U kept, barrier needed. Corrupt: latched,
    // U kept, refused.
    for (read, latched) in [(Read::Failed, false), (Read::Corrupt, true)] {
        let (mut core, _health, mut worker) = open_core();
        gating_uncertain(&mut core, 1);
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.row.push_back(read);
        let outcome = core.resolve(&mut worker, u_token(&core), &mut io);
        assert!(core.u.is_some());
        if latched {
            assert!(matches!(outcome, Resolve::Refused(Entry::Latched)));
            assert_eq!(core.latch, Some(LatchCause::Corrupt));
        } else {
            assert!(matches!(outcome, Resolve::Retained));
            assert_eq!(core.phase, Phase::NeedBarrier);
        }
        let (mut core, _health, mut worker) = open_core();
        ack_uncertain(&mut core, set(0, 5, 5, 1, 1));
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.acknowledgement.push_back(Read::Got(Some(6)));
        io.frontier.push_back(match read {
            Read::Failed => Read::Failed,
            _ => Read::Corrupt,
        });
        let outcome = core.resolve(&mut worker, u_token(&core), &mut io);
        assert!(core.u.is_some());
        assert_eq!(matches!(outcome, Resolve::Refused(Entry::Latched)), latched);
        assert_eq!(matches!(outcome, Resolve::Retained), !latched);
    }
    // No room (forced): latched, U kept, the phase stays resolving.
    let (mut core, health, mut worker) = open_core();
    explain_into_x(&mut core, &health, 2);
    let saved = core.x.take();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    core.x = saved;
    let outcome = core.resolve(&mut worker, u_token(&core), &mut Script::new());
    assert!(matches!(outcome, Resolve::Refused(Entry::Latched)));
    assert_eq!(core.latch, Some(LatchCause::CellInvariant));
    assert!(core.u.is_some());
    assert_eq!(core.phase, Phase::Resolving);
    // A record left in call (forced): resolve latches.
    let (mut core, _health, mut worker) = open_core();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    if let Some((_, Occupant::Gating { exit, .. })) = core.u.as_mut() {
        *exit = Exit::InCall;
    }
    let outcome = core.resolve(&mut worker, u_token(&core), &mut Script::new());
    assert!(matches!(outcome, Resolve::Refused(Entry::Latched)));
    assert_eq!(core.latch, Some(LatchCause::CellInvariant));
}

/// Leaves an acknowledgement in U, its append having returned Uncertain.
fn ack_uncertain(core: &mut Core, set: SetFacts) {
    let snapshot = snapshot_of(core, set);
    let mut io = Script::new();
    io.recheck.push_back(Read::Got(set));
    io.commit.push_back(Commit::Unknown);
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut io),
        AckOutcome::NotApplied(Persistence::Uncertain)
    );
    assert!(matches!(
        core.u,
        Some((_, Occupant::Acknowledgement { .. }))
    ));
}

// ---- T-ACK --------------------------------------------------------------------------------------

/// T-ACK: section 4.8: each of the six tests refusing alone; values at the
/// ends of 64 bits.
#[test]
fn t_ack_each_test_refuses_alone() {
    let max = u64::MAX;
    // (the snapshot's set, the recheck's set, the expected answer)
    let cases: Vec<(&str, SetFacts, SetFacts, AckOutcome)> = vec![
        (
            "stale",
            set(3, 9, 7, 2, 6),
            set(4, 9, 7, 2, 6),
            AckOutcome::Refused(AckRefusal::Stale),
        ),
        (
            "empty: N not above the frontier",
            set(7, 9, 7, 2, 6),
            set(7, 9, 7, 2, 6),
            AckOutcome::Refused(AckRefusal::Empty),
        ),
        (
            "empty: no record",
            set(3, 9, 7, 0, 6),
            set(3, 9, 7, 0, 6),
            AckOutcome::Refused(AckRefusal::Empty),
        ),
        (
            "future: N above the snapshot's head",
            set(3, 6, 7, 2, 6),
            set(3, 9, 7, 2, 6),
            AckOutcome::Refused(AckRefusal::Future),
        ),
        (
            "future: the head above the head now",
            set(3, 9, 7, 2, 6),
            set(3, 8, 7, 2, 6),
            AckOutcome::Refused(AckRefusal::Future),
        ),
        (
            "set: count",
            set(3, 9, 7, 2, 6),
            set(3, 9, 7, 3, 6),
            AckOutcome::Refused(AckRefusal::Set),
        ),
        (
            "set: digest",
            set(3, 9, 7, 2, 6),
            set(3, 9, 7, 2, 4),
            AckOutcome::Refused(AckRefusal::Set),
        ),
        (
            "passes",
            set(3, 9, 7, 2, 6),
            set(3, 12, 7, 2, 6),
            AckOutcome::Committed(7),
        ),
        (
            "ends of 64 bits",
            set(max - 1, max, max, max, 6),
            set(max - 1, max, max, max, 6),
            AckOutcome::Committed(max),
        ),
        (
            "stale at the ends",
            set(max - 1, max, max, 1, 6),
            set(max, max, max, 1, 6),
            AckOutcome::Refused(AckRefusal::Stale),
        ),
        (
            "future at the ends",
            set(0, max - 1, max, 1, 6),
            set(0, max, max, 1, 6),
            AckOutcome::Refused(AckRefusal::Future),
        ),
    ];
    for (case, snap, now, expected) in cases {
        let (mut core, _health, _worker) = open_core();
        // The snapshot's values are forced through private state when the
        // survey could not give them.
        let mut snapshot = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
        snapshot.set = snap;
        let mut io = Script::new();
        io.recheck.push_back(Read::Got(now));
        assert_eq!(
            core.append_acknowledgement(snapshot, &mut io),
            expected,
            "{case}"
        );
        assert!(core.u.is_none(), "{case}: U emptied");
        if let AckOutcome::Committed(n) = expected {
            assert_eq!(core.frontier, n, "{case}");
            assert!(!io.calls.contains(&"rollback"));
        } else {
            assert_eq!(
                io.calls,
                ["open", "recheck", "rollback"],
                "{case}: one rollback, before any insert"
            );
            assert_eq!(core.frontier, 0);
        }
    }
    // Tests 0 and 1 change nothing (outside a broken invariant).
    let (mut other, _h, _w) = open_core();
    let foreign = snapshot_of(&mut other, set(0, 2, 2, 1, 1));
    let (mut core, _health, _worker) = open_core();
    let before = core.occupancies;
    let mut io = Script::new();
    assert_eq!(
        core.append_acknowledgement(foreign, &mut io),
        AckOutcome::Refused(AckRefusal::OtherStore)
    );
    core.phase = Phase::NeedBarrier;
    let own = snapshot_of_forced(&mut core);
    assert_eq!(
        core.append_acknowledgement(own, &mut io),
        AckOutcome::Refused(AckRefusal::NotNow(Entry::NotHealthy))
    );
    assert!(io.calls.is_empty());
    assert_eq!(core.occupancies, before, "U not touched");
    // The snapshot: nothing to acknowledge; failed; values kept.
    let (mut core, _health, _worker) = open_core();
    let mut io = Script::new();
    io.survey.push_back(Read::Got(set(5, 9, 5, 1, 1)));
    io.survey.push_back(Read::Got(set(3, 9, 7, 0, 0)));
    io.survey.push_back(Read::Failed);
    io.survey.push_back(Read::Got(set(3, 9, 7, 2, 6)));
    assert!(matches!(
        core.snapshot([1; 16], &mut io),
        SnapshotOutcome::Nothing
    ));
    assert!(matches!(
        core.snapshot([1; 16], &mut io),
        SnapshotOutcome::Nothing
    ));
    assert!(matches!(
        core.snapshot([1; 16], &mut io),
        SnapshotOutcome::Failed
    ));
    assert_eq!(core.phase, Phase::NeedBarrier);
    core.phase = Phase::Healthy;
    let SnapshotOutcome::Taken(snapshot) = core.snapshot([4; 16], &mut io) else {
        panic!("a snapshot");
    };
    assert_eq!(
        (snapshot.set(), snapshot.nonce()),
        (set(3, 9, 7, 2, 6), [4; 16])
    );
    // The qualified frontier never falls.
    core.frontier = 9;
    let mut io = Script::new();
    io.recheck.push_back(Read::Got(set(3, 9, 7, 2, 6)));
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut io),
        AckOutcome::Committed(7)
    );
    assert_eq!(core.frontier, 9);
}

/// A snapshot of this ledger, its values forced.
fn snapshot_of_forced(core: &mut Core) -> Snapshot {
    Snapshot {
        ledger: Arc::clone(&core.shared),
        set: set(0, 2, 2, 1, 1),
        nonce: [2; 16],
    }
}

// ---- T-LIMIT ------------------------------------------------------------------------------------

/// T-LIMIT: arm at the limit; the table at its limit, for an append and a
/// lookup; counts at their ends; a cell's occupancy number at its end. Each
/// end is forced through private state; a forced state is not called
/// reachable.
#[test]
fn t_limit_every_end() {
    // The limits are brought into range.
    for (asked, kept) in [(0, 1), (1, 1), (8, 8), (u32::MAX, 0xFF_FFFF)] {
        let (core, _health) = Core::new(Limits {
            tickets: asked,
            refusals: 0,
        });
        assert_eq!((core.shared.tickets, core.refusal_limit), (kept, 1));
    }
    // arm at the limit: the handover fault, and nothing.
    let (_core, health) = Core::new(Limits {
        tickets: 2,
        refusals: 4,
    });
    let a = health.arm().expect("one");
    let b = health.arm().expect("two");
    assert!(health.arm().is_none());
    assert!(health.view().handover_fault());
    assert_eq!(health.view().live(), 2, "live is left as it is");
    a.confirm();
    b.confirm();
    assert!(
        health.view().handover_fault(),
        "the handover fault is sticky"
    );

    // The table at its limit, for an append: latched, table full, nothing
    // tabled or built into X, the member still named.
    let (mut core, health, _worker) = open_core();
    for n in 0..4 {
        core.refusals.insert(
            key(50 + n),
            Cause {
                kind: CauseKind::MissingPrepared {
                    part: Part {
                        commitment: bytes(9, n),
                        role: Role::Started,
                    },
                    offered: key(50 + n),
                },
            },
        );
    }
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(holder_facts()));
    assert_eq!(
        core.append_facts(&mut ticket, &[fact_with(key(1), 1, Role::Started)], &mut io),
        FactOutcome::NotApplied {
            counted: not_written(Some(0)),
            reason: Reason::TableFull { member: 0 }
        }
    );
    assert_eq!(core.latch, Some(LatchCause::TableFull));
    assert_eq!(core.refusals.len(), 4);
    assert!(core.x.is_none());
    ticket.confirm();
    // ... and for a lookup: latched, refused.
    let (mut core, _health, _worker) = open_core();
    for n in 0..4 {
        core.refusals.insert(
            key(50 + n),
            Cause {
                kind: CauseKind::MissingPrepared {
                    part: Part {
                        commitment: bytes(9, n),
                        role: Role::Started,
                    },
                    offered: key(50 + n),
                },
            },
        );
    }
    let mut io = Script::new();
    io.row.push_back(Read::Got(Some(Row {
        seq: 2,
        digest: other_content(1).digest,
    })));
    assert_eq!(
        core.lookup(&key(1), &mut io),
        Lookup::Refused(Entry::Latched)
    );
    assert_eq!(core.latch, Some(LatchCause::TableFull));
    assert_eq!(core.refusals.len(), 4);

    // Counts at their ends: a publication's +1 at the end of `unapplied`
    // sets the handover fault and leaves the count; the operation still
    // returns its result and still marks the ticket Counted.
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let full = with_field(word(&core), UNAPPLIED_SHIFT, COUNT_MAX);
    set_word(&core, full);
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    let outcome = core.append_facts(&mut ticket, &[fact(1)], &mut io);
    assert!(matches!(outcome, FactOutcome::NotApplied { .. }));
    assert_eq!(ticket.counted(), Some(not_written(None)));
    let view = health.view();
    assert!(view.handover_fault());
    assert_eq!(u64::from(view.unapplied()), COUNT_MAX);
    // ... `expire` at the same end.
    let mut other = health.arm().expect("a ticket");
    assert!(matches!(other.expire(), FactOutcome::NotApplied { .. }));
    assert_eq!(u64::from(health.view().unapplied()), COUNT_MAX);
    // ... `confirm` at zero underflows: the handover fault, the count left.
    set_word(
        &core,
        with_field(with_field(word(&core), UNAPPLIED_SHIFT, 0), LIVE_SHIFT, 0) & !HANDOVER_FAULT,
    );
    ticket.confirm();
    let view = health.view();
    assert!(view.handover_fault());
    assert_eq!((view.unapplied(), view.live()), (0, 0));
    set_word(&core, word(&core) & !HANDOVER_FAULT);
    other.confirm();
    assert!(health.view().handover_fault());

    // A cell's occupancy number at its end: latched (overflow); nothing placed.
    let (mut core, _health, _worker) = open_core();
    core.occupancies[0] = u64::MAX;
    assert_eq!(
        core.append_gating(&gating(1), &mut Script::new()),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Entry(Entry::Latched)
        }
    );
    assert_eq!(core.latch, Some(LatchCause::Overflow));
    assert!(core.u.is_none());
    let (mut core, health, _worker) = open_core();
    core.occupancies[2] = u64::MAX;
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(holder_facts()));
    let outcome = core.append_facts(&mut ticket, &[fact_with(key(1), 1, Role::Started)], &mut io);
    assert!(
        matches!(outcome, FactOutcome::NotApplied { counted, .. } if counted.conflict_member == Some(0))
    );
    assert_eq!(core.latch, Some(LatchCause::Overflow));
    assert!(core.x.is_none());
    ticket.confirm();
}

/// The word, read by the test outside the core's two functions.
fn word(core: &Core) -> u64 {
    core.shared.word.load(std::sync::atomic::Ordering::SeqCst)
}

/// Forces the word (test access to private state).
fn set_word(core: &Core, value: u64) {
    core.shared
        .word
        .store(value, std::sync::atomic::Ordering::SeqCst);
}

// ---- T-PB1 to T-PB5 -----------------------------------------------------------------------------

/// Removes the hook's callback when the test ends, also on a failure.
struct HookGuard;

impl Drop for HookGuard {
    fn drop(&mut self) {
        hook::remove();
    }
}

fn health_of(core: &Core) -> Health {
    Health {
        shared: Arc::clone(&core.shared),
    }
}

/// T-PB1: just after an append's publication, `view` already refuses
/// admission.
#[test]
fn t_pb1_after_the_publication_view_refuses() {
    let _guard = HookGuard;
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    assert!(health.view().admits());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (record, probe) = (Rc::clone(&seen), health_of(&core));
    hook::install(move |event| {
        if let hook::Event::Exchanged(..) = event {
            record.borrow_mut().push(probe.view().admits());
        }
    });
    let mut io = Script::new();
    io.commit.push_back(Commit::Unknown);
    let outcome = core.append_facts(&mut ticket, &[fact(1)], &mut io);
    hook::remove();
    assert!(matches!(outcome, FactOutcome::NotApplied { .. }));
    assert_eq!(
        *seen.borrow(),
        [false],
        "one publication; admission already refused just after it"
    );
    ticket.confirm();
}

/// T-PB2: just before that publication, the word is exactly as it was
/// before the call.
#[test]
fn t_pb2_before_the_publication_the_word_is_unchanged() {
    let _guard = HookGuard;
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let before = word(&core);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (record, probe) = (Rc::clone(&seen), health_of(&core));
    hook::install(move |event| match event {
        hook::Event::Failed(present) => {
            record
                .borrow_mut()
                .push(("failed", present, probe.view().word))
        }
        hook::Event::Exchanged(old, _) => record.borrow_mut().push(("exchanged", old, 0)),
        hook::Event::Loaded(_) => {}
    });
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    core.append_facts(&mut ticket, &[fact(1)], &mut io);
    hook::remove();
    let seen = seen.borrow();
    assert!(
        seen.len() >= 2,
        "the guess missed the counts once: {seen:?}"
    );
    for (kind, value, viewed) in &seen[..seen.len() - 1] {
        assert_eq!(
            (*kind, *value, *viewed),
            ("failed", before, before),
            "just before the publication"
        );
    }
    assert_eq!(seen.last().map(|s| (s.0, s.1)), Some(("exchanged", before)));
    assert_ne!(word(&core), before);
    ticket.confirm();
}

/// T-PB3: `expire`: the count is up when its one exchange has succeeded,
/// and only then.
#[test]
fn t_pb3_expire_counts_at_its_exchange() {
    let _guard = HookGuard;
    let (core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (record, probe) = (Rc::clone(&seen), health_of(&core));
    hook::install(move |event| {
        let unapplied = probe.view().unapplied();
        match event {
            hook::Event::Failed(_) => record.borrow_mut().push(("failed", unapplied)),
            hook::Event::Exchanged(..) => record.borrow_mut().push(("exchanged", unapplied)),
            hook::Event::Loaded(_) => {}
        }
    });
    hook::take_counts();
    ticket.expire();
    let counts = hook::take_counts();
    hook::remove();
    assert_eq!(counts.exchanges, 1, "one exchange");
    let seen = seen.borrow();
    assert!(
        seen.iter().filter(|s| s.0 == "failed").all(|s| s.1 == 0),
        "{seen:?}"
    );
    assert_eq!(seen.last(), Some(&("exchanged", 1)));
    ticket.confirm();
}

/// T-PB4: another ticket's `confirm`, run from the hook after a
/// publication's first exchange has failed, is not lost: the loop tries
/// again and both changes stand.
#[test]
fn t_pb4_a_concurrent_confirm_is_not_lost() {
    let _guard = HookGuard;
    let (mut core, health, _worker) = open_core();
    let mut lent = health.arm().expect("a ticket");
    let other = health.arm().expect("another");
    let mut other = Some(other);
    let failures = Rc::new(RefCell::new(0));
    let count = Rc::clone(&failures);
    hook::install(move |event| {
        if let hook::Event::Failed(_) = event {
            *count.borrow_mut() += 1;
            if let Some(ticket) = other.take() {
                ticket.confirm();
            }
        }
    });
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    core.append_facts(&mut lent, &[fact(1)], &mut io);
    hook::remove();
    assert!(
        *failures.borrow() >= 2,
        "the loop failed, tried again and failed again before it succeeded"
    );
    let view = health.view();
    assert_eq!(
        (view.live(), view.unapplied()),
        (1, 1),
        "the confirm and the count both stand"
    );
    assert_eq!(view.phase(), Phase::NeedBarrier);
    lent.confirm();
    assert_eq!((health.view().live(), health.view().unapplied()), (0, 0));
}

/// T-PB5: each verb of each operation unwinds: exactly one publication, by
/// the guard, and no load.
#[test]
fn t_pb5_every_unwinding_verb_leaves_one_publication() {
    fn once(name: &str, verbs: usize, mut op: impl FnMut(Script) -> (Core, Health)) {
        for at in 0..verbs {
            hook::take_counts();
            let (core, health) = op(Script::unwinding_at(at));
            let counts = hook::take_counts();
            assert_eq!(
                (counts.loads, counts.exchanges),
                (0, 1),
                "{name}: unwound at verb {at}"
            );
            assert_eq!(
                core.phase,
                Phase::NeedBarrier,
                "{name}: unwound at verb {at}"
            );
            assert_eq!(
                health.view().phase(),
                Phase::NeedBarrier,
                "{name}: published"
            );
        }
    }
    once("append_facts", 4, |mut io| {
        let (mut core, health, _worker) = open_core();
        let mut ticket = health.arm().expect("a ticket");
        hook::take_counts();
        assert!(caught(|| core.append_facts(&mut ticket, &[fact(1)], &mut io)).is_none());
        let counts = hook::take_counts();
        assert_eq!((counts.loads, counts.exchanges), (0, 1));
        hook::report(hook::Event::Exchanged(0, 0));
        ticket.counted().expect("counted by the guard");
        let settled = Ticket {
            shared: Arc::clone(&core.shared),
            state: TicketState::Settled,
        };
        drop(settled);
        std::mem::forget(ticket);
        (core, health)
    });
    once("append_facts, rollback", 3, |mut io| {
        let (mut core, health, _worker) = open_core();
        let mut ticket = health.arm().expect("a ticket");
        io.insert.push_back(Insert::Failed);
        io.unwind_at = Some(3);
        hook::take_counts();
        assert!(caught(|| core.append_facts(&mut ticket, &[fact(1)], &mut io)).is_none());
        assert_eq!(ticket.counted(), Some(uncertain()));
        ticket.state = TicketState::Settled;
        (core, health)
    });
    once("append_gating", 4, |mut io| {
        let (mut core, health, _worker) = open_core();
        hook::take_counts();
        assert!(caught(|| core.append_gating(&gating(1), &mut io)).is_none());
        (core, health)
    });
    once("lookup", 2, |mut io| {
        let (mut core, health, _worker) = open_core();
        io.row.push_back(Read::Got(Some(Row {
            seq: 2,
            digest: other_content(1).digest,
        })));
        hook::take_counts();
        assert!(caught(|| core.lookup(&key(1), &mut io)).is_none());
        (core, health)
    });
    once("explain", 1, |mut io| {
        let (mut core, health, _worker) = open_core();
        hook::take_counts();
        assert!(caught(|| core.explain(&key(1), &mut io)).is_none());
        (core, health)
    });
    once("snapshot", 1, |mut io| {
        let (mut core, health, _worker) = open_core();
        hook::take_counts();
        assert!(caught(|| core.snapshot([1; 16], &mut io)).is_none());
        (core, health)
    });
    once("append_acknowledgement", 4, |mut io| {
        let (mut core, health, _worker) = open_core();
        let snapshot = snapshot_of(&mut core, set(0, 2, 2, 1, 1));
        io.recheck.push_back(Read::Got(set(0, 2, 2, 1, 1)));
        hook::take_counts();
        assert!(caught(|| core.append_acknowledgement(snapshot, &mut io)).is_none());
        (core, health)
    });
    once("barrier", 1, |mut io| {
        let (mut core, health, mut worker) = open_core();
        core.phase = Phase::NeedBarrier;
        set_word(&core, word(&core) | PHASE_NEED_BARRIER);
        hook::take_counts();
        assert!(caught(|| core.barrier(&mut worker, &mut io)).is_none());
        (core, health)
    });
    once("resolve, gating", 1, |mut io| {
        let (mut core, health, mut worker) = open_core();
        gating_uncertain(&mut core, 1);
        pass_barrier(&mut core, &mut worker);
        let token = u_token(&core);
        hook::take_counts();
        assert!(caught(|| core.resolve(&mut worker, token, &mut io)).is_none());
        (core, health)
    });
    once("resolve, acknowledgement", 2, |mut io| {
        let (mut core, health, mut worker) = open_core();
        ack_uncertain(&mut core, set(0, 5, 5, 1, 1));
        pass_barrier(&mut core, &mut worker);
        io.acknowledgement.push_back(Read::Got(Some(6)));
        let token = u_token(&core);
        hook::take_counts();
        assert!(caught(|| core.resolve(&mut worker, token, &mut io)).is_none());
        (core, health)
    });
    once("append_cell", 4, |mut io| {
        let (mut core, health, mut worker) = open_core();
        let x = explain_into_x(&mut core, &health, 1);
        hook::take_counts();
        assert!(caught(|| core.append_cell(&mut worker, x, &mut io)).is_none());
        (core, health)
    });
}

// ---- named witnesses ----------------------------------------------------------------------------

/// W1, review finding D03-01: two writers each get a failing result; one
/// records and confirms; admission must stay refused until the other has
/// too. Both orders.
#[test]
fn w1_admission_waits_for_every_handover() {
    for first in [0, 1] {
        let (mut core, health, mut worker) = open_core();
        let mut tickets = [health.arm().expect("one"), health.arm().expect("two")];
        for (n, ticket) in tickets.iter_mut().enumerate() {
            let mut io = Script::new();
            io.open.push_back(Read::Failed);
            assert!(matches!(
                core.append_facts(ticket, &[fact(n as u8)], &mut io),
                FactOutcome::NotApplied { .. }
            ));
        }
        pass_barrier(&mut core, &mut worker);
        let [a, b] = tickets;
        let (one, two) = if first == 0 { (a, b) } else { (b, a) };
        one.confirm();
        assert!(
            !health.view().admits(),
            "admission stays refused until the other has confirmed too"
        );
        two.confirm();
        assert!(health.view().admits());
    }
}

/// W2: the scratch model's first P6 failure: a fact is refused as a
/// conflict while X holds another explanation; its writer unwinds; a lookup
/// finds it absent; ENV fires; a retry is offered. It must not be Paid.
#[test]
fn w2_a_refused_conflict_is_remembered_across_env() {
    let (mut core, health, _worker) = open_core();
    explain_into_x(&mut core, &health, 9);
    let f1 = fact_with(key(1), 1, Role::Started);
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        prepared: false,
        ..CLEAN
    }));
    let outcome = core.append_facts(&mut ticket, std::slice::from_ref(&f1), &mut io);
    assert!(matches!(
        outcome,
        FactOutcome::NotApplied {
            reason: Reason::Conflict { .. },
            ..
        }
    ));
    assert!(
        core.refusals.contains_key(&key(1)),
        "tabled although X holds another explanation"
    );
    // The writer unwinds: its drop path records from the Counted ticket.
    assert_eq!(ticket.counted(), Some(not_written(Some(0))));
    ticket.confirm();
    assert_eq!(core.lookup(&key(1), &mut Script::new()), Lookup::Absent);
    // ENV: the Prepared record appears; the retry is classified again.
    let mut retry = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(CLEAN));
    let outcome = core.append_facts(&mut retry, std::slice::from_ref(&f1), &mut io);
    assert!(
        matches!(
            outcome,
            FactOutcome::NotApplied {
                reason: Reason::Conflict {
                    known: Known::Cause(_),
                    ..
                },
                ..
            }
        ),
        "the retained cause answers; it is not Paid: {outcome:?}"
    );
    assert!(!io.calls.contains(&"insert"));
    retry.confirm();
}

/// W3, review finding D03-03: a snapshot is taken; a second explanation
/// commits; the acknowledgement is appended. The second explanation stays
/// above the frontier and admission stays refused.
#[test]
fn w3_a_later_explanation_stays_above_the_frontier() {
    let (mut core, _health, _worker) = open_core();
    let snapshot = snapshot_of(&mut core, set(0, 5, 5, 1, 1));
    // The second explanation commits at 7.
    let mut io = Script::new();
    io.recheck.push_back(Read::Got(set(0, 7, 5, 1, 1)));
    assert_eq!(
        core.append_acknowledgement(snapshot, &mut io),
        AckOutcome::Committed(5)
    );
    assert_eq!(core.agenda().frontier, 5);
    let (h, a) = (7, core.agenda().frontier);
    assert!(h > a, "recovery is still required: admission stays refused");
}

/// W4, the fourth revision's case: an acknowledgement's append is Uncertain
/// and written; resolve finds it present; before the worker applies it, a
/// second does the same. The agenda's frontier covers both.
#[test]
fn w4_the_agenda_frontier_covers_both_acknowledgements() {
    let (mut core, _health, mut worker) = open_core();
    for (n, set) in [(5, set(0, 5, 5, 1, 1)), (8, set(5, 8, 8, 1, 2))] {
        ack_uncertain(&mut core, set);
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.acknowledgement.push_back(Read::Got(Some(n + 1)));
        io.frontier.push_back(Read::Got(n));
        assert!(
            matches!(core.resolve(&mut worker, u_token(&core), &mut io), Resolve::Acknowledged(m) if m == n)
        );
    }
    assert_eq!(
        core.agenda().frontier,
        8,
        "the agenda carries the larger frontier"
    );
}

/// W5, finding AM-01: a batch whose second insert fails on a constraint,
/// then a failed rollback: latched; no member named; every member
/// Uncertain.
#[test]
fn w5_a_late_constraint_and_a_failed_rollback() {
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.insert.extend([Insert::Inserted(10), Insert::Constraint]);
    io.rollback.push_back(Rollback::Unknown);
    let outcome = core.append_facts(&mut ticket, &[fact(1), fact(2)], &mut io);
    assert_eq!(
        outcome,
        FactOutcome::NotApplied {
            counted: uncertain(),
            reason: Reason::Constraint
        }
    );
    assert_eq!(core.latch, Some(LatchCause::Constraint));
    ticket.confirm();
}

/// W6, finding AM-01: a batch with one identity under two contents is
/// malformed; each content, offered alone afterwards, gets its real answer.
#[test]
fn w6_one_identity_under_two_contents() {
    let (mut core, health, _worker) = open_core();
    let first = fact(1);
    let second = FactRecord::new(Member {
        key: other_content(1),
        part: None,
    });
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    assert_eq!(
        core.append_facts(&mut ticket, &[first.clone(), second.clone()], &mut io),
        FactOutcome::NotApplied {
            counted: not_written(None),
            reason: Reason::Entry(Entry::Malformed)
        }
    );
    assert!(io.calls.is_empty());
    ticket.confirm();
    let mut ticket = health.arm().expect("a ticket");
    assert_eq!(
        core.append_facts(&mut ticket, &[first], &mut Script::new()),
        FactOutcome::Committed
    );
    ticket.confirm();
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.facts.push_back(Read::Got(MemberFacts {
        row: Some(Row {
            seq: 101,
            digest: key(1).digest,
        }),
        ..CLEAN
    }));
    let outcome = core.append_facts(&mut ticket, &[second], &mut io);
    let FactOutcome::NotApplied {
        reason: Reason::Conflict {
            known: Known::Cause(cause),
            ..
        },
        ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert!(
        matches!(cause.kind(), CauseKind::IdentityContent { existing, .. } if existing.seq == 101)
    );
    ticket.confirm();
}

/// W7, finding AM-03: X emptied and refilled; the old token is stale.
#[test]
fn w7_an_old_token_is_stale() {
    let (mut core, health, mut worker) = open_core();
    let old = explain_into_x(&mut core, &health, 1);
    assert_eq!(
        core.append_cell(&mut worker, old.clone(), &mut Script::new()),
        CellAppend::Committed(101)
    );
    assert_eq!(core.empty(&mut worker, old.clone()), Emptying::Done);
    let new = explain_into_x(&mut core, &health, 2);
    assert_ne!(new.occupancy, old.occupancy);
    let before = format!("{:?}", core.agenda());
    assert_eq!(
        core.append_cell(&mut worker, old.clone(), &mut Script::new()),
        CellAppend::Stale
    );
    assert_eq!(core.empty(&mut worker, old), Emptying::Stale);
    assert_eq!(format!("{:?}", core.agenda()), before, "nothing changed");
}

/// W8, finding AM-05: the worker unwinds after U was emptied for a present
/// acknowledgement; the next agenda still carries the frontier.
#[test]
fn w8_a_resolved_acknowledgement_survives_the_worker() {
    let (mut core, _health, mut worker) = open_core();
    ack_uncertain(&mut core, set(0, 5, 5, 1, 1));
    pass_barrier(&mut core, &mut worker);
    let mut io = Script::new();
    io.acknowledgement.push_back(Read::Got(Some(6)));
    io.frontier.push_back(Read::Got(5));
    let resolved = core.resolve(&mut worker, u_token(&core), &mut io);
    assert!(core.u.is_none());
    // The worker unwinds before it applies the result: the result is lost.
    drop(resolved);
    let agenda = core.agenda();
    assert_eq!(agenda.frontier, 5, "the store keeps it");
    assert!(agenda.u.is_none() && agenda.phase == Phase::Healthy);
}

/// W9, finding AM-06: a Counted ticket dropped: a sticky handover fault;
/// the ticket keeps its place in the limit.
#[test]
fn w9_a_dropped_counted_ticket() {
    let (mut core, health, mut worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.open.push_back(Read::Failed);
    core.append_facts(&mut ticket, &[fact(1)], &mut io);
    drop(ticket);
    let view = health.view();
    assert!(view.handover_fault());
    assert_eq!(
        (view.live(), view.unapplied()),
        (1, 1),
        "unapplied and live left as they are"
    );
    pass_barrier(&mut core, &mut worker);
    assert!(
        !health.view().admits(),
        "no admission after a handover fault"
    );
    // An Armed ticket's drop lowers live; a confirmed ticket's drop does nothing.
    let armed = health.arm().expect("a ticket");
    assert_eq!(health.view().live(), 2);
    drop(armed);
    assert_eq!(health.view().live(), 1);
}

/// W10: an expiry before entry.
#[test]
fn w10_an_expiry_before_entry() {
    let (_core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    assert_eq!(
        ticket.expire(),
        FactOutcome::NotApplied {
            counted: not_written(None),
            reason: Reason::Entry(Entry::Expired)
        }
    );
    assert_eq!(health.view().unapplied(), 1);
    assert_eq!(
        ticket.expire(),
        FactOutcome::Rejected(TicketFault::NotArmed)
    );
    assert!(
        health.view().handover_fault(),
        "an expiry of a ticket that is not Armed"
    );
    ticket.confirm();
    assert_eq!((health.view().unapplied(), health.view().live()), (0, 0));
}

/// W11: a commit that is not durable, followed by a clean rollback:
/// Uncertain.
#[test]
fn w11_an_unknown_commit_and_a_clean_rollback() {
    let (mut core, health, _worker) = open_core();
    let mut ticket = health.arm().expect("a ticket");
    let mut io = Script::new();
    io.commit.push_back(Commit::Unknown);
    io.rollback.push_back(Rollback::Undone);
    assert_eq!(
        core.append_facts(&mut ticket, &[fact(1)], &mut io),
        FactOutcome::NotApplied {
            counted: uncertain(),
            reason: Reason::Storage
        }
    );
    assert_eq!(io.calls, ["open", "facts", "insert", "commit", "rollback"]);
    ticket.confirm();
}

/// W12: a lookup that finds different content, then explain.
#[test]
fn w12_a_lookup_conflict_then_explain() {
    let (mut core, _health, mut worker) = open_core();
    let different = Read::Got(Some(Row {
        seq: 3,
        digest: other_content(1).digest,
    }));
    let mut io = Script::new();
    io.row.push_back(different);
    let Lookup::Conflict(Known::Cause(cause)) = core.lookup(&key(1), &mut io) else {
        panic!("a conflict observation");
    };
    assert_eq!(
        core.refusals.get(&key(1)),
        Some(&cause),
        "entered in the table"
    );
    assert!(core.x.is_none(), "lookup never fills X");
    let mut io = Script::new();
    io.row.push_back(different);
    assert_eq!(
        core.lookup(&key(1), &mut io),
        Lookup::Conflict(Known::Cause(cause)),
        "the retained cause"
    );
    assert_eq!(core.explain(&key(1), &mut Script::new()), Explain::Pending);
    assert_eq!(core.explain(&key(1), &mut Script::new()), Explain::Pending);
    assert_eq!(core.explain(&key(2), &mut Script::new()), Explain::Busy);
    let x = core.agenda().x.expect("X holds the explanation").0;
    assert_eq!(
        core.append_cell(&mut worker, x.clone(), &mut Script::new()),
        CellAppend::Committed(101)
    );
    assert!(core.refusals.is_empty());
    assert_eq!(core.empty(&mut worker, x), Emptying::Done);
    let mut io = Script::new();
    io.explained.push_back(Read::Got(Some(101)));
    assert_eq!(core.explain(&key(1), &mut io), Explain::Explained(101));
    let mut io = Script::new();
    io.row.push_back(different);
    io.explained.push_back(Read::Got(Some(101)));
    assert_eq!(
        core.lookup(&key(1), &mut io),
        Lookup::Conflict(Known::Explained(101))
    );
    assert!(
        core.refusals.is_empty(),
        "an explained original is not tabled again"
    );
    // Explain of an original nobody refused: Unknown.
    assert_eq!(core.explain(&key(5), &mut Script::new()), Explain::Unknown);
}

/// W13, finding PC-03: the same gating record, written, its append
/// returning Uncertain in one run and its verb unwinding in the other.
/// After the barrier and resolve, the two corrections differ in their exit
/// and in nothing else.
#[test]
fn w13_two_histories_two_exits() {
    let mut corrections = Vec::new();
    for unwinds in [false, true] {
        let (mut core, _health, mut worker) = open_core();
        if unwinds {
            let mut io = Script::unwinding_at(3);
            assert_eq!(caught(|| core.append_gating(&gating(1), &mut io)), None);
        } else {
            gating_uncertain(&mut core, 1);
        }
        let Some((_, Occupant::Gating { exit, .. })) = &core.u else {
            panic!("U keeps the record");
        };
        assert_ne!(*exit, Exit::InCall, "InCall never survives a call");
        pass_barrier(&mut core, &mut worker);
        let mut io = Script::new();
        io.row.push_back(Read::Got(Some(Row {
            seq: 101,
            digest: key(1).digest,
        })));
        assert!(matches!(
            core.resolve(&mut worker, u_token(&core), &mut io),
            Resolve::Corrected(_)
        ));
        corrections.push(core.c.take().map(|(_, occupant)| occupant));
    }
    let correction = |exit| {
        Some(Occupant::Correction {
            about: key(1),
            seq: 101,
            exit,
        })
    };
    assert_eq!(
        corrections,
        [
            correction(Exit::ReturnedUncertain),
            correction(Exit::Unwound)
        ]
    );
}

/// The view's fields, read whole.
#[test]
fn the_view_reads_every_field() {
    let (mut core, health, _worker) = open_core();
    let view = health.view();
    assert!(view.admits());
    assert_eq!(
        (view.phase(), view.session(), view.latched()),
        (Phase::Healthy, Session::Open, false)
    );
    assert_eq!(
        (view.u_occupied(), view.c_occupied(), view.x_occupied()),
        (false, false, false)
    );
    gating_uncertain(&mut core, 1);
    let view = health.view();
    assert!(view.u_occupied() && !view.admits());
    assert_eq!(view.phase(), Phase::NeedBarrier);
    assert_eq!(view.word & SESSION_MASK, SESSION_OPEN);
    let (mut core, health, mut worker) = open_core();
    gating_uncertain(&mut core, 1);
    pass_barrier(&mut core, &mut worker);
    let mut io = Script::new();
    io.row.push_back(Read::Got(Some(Row {
        seq: 8,
        digest: key(1).digest,
    })));
    core.resolve(&mut worker, u_token(&core), &mut io);
    assert!(health.view().c_occupied());
    let (mut core, health, _worker) = open_core();
    explain_into_x(&mut core, &health, 1);
    assert!(health.view().x_occupied());
    core.session_sealed();
    assert_eq!(
        health.view().session(),
        Session::Open,
        "not sealed while X is occupied"
    );
}
