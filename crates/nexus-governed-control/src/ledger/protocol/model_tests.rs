//! The exhaustive exploration of the protocol core (contract section 7).
//!
//! It explores the real core. It simulates the ledger's content, implements
//! every verb over that simulation and plays every caller. Every interleaving
//! of whole steps is visited breadth first, with a visited set and a parent
//! table, so that a violation has a shortest trace.
//!
//! A state is a whole world with its own word. To try a step, a world is
//! rebuilt from its key: a new shared allocation holding the same word value,
//! and the core, the health handle, every ticket, the worker, every retained
//! token and every snapshot made to refer to it. Two branches never share one
//! word. The key holds every field of the world; addresses are not part of it.
//!
//! Inside one step, every answer a verb can give within the fault budget is
//! tried, by replaying the step with each sequence of choices. An unwinding
//! verb really unwinds (`resume_unwind`, without the panic hook); the step
//! catches it, so the guard of rule R7 is exercised, not imitated.
//!
//! Every run ends PASS, FAIL (with its shortest trace) or INCOMPLETE. An
//! INCOMPLETE run is never PASS.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Limits in every configuration (section 7.1). Neither is reached in
/// exploration; the limits are T-LIMIT's subject.
const TICKET_LIMIT: u32 = 8;
const REFUSAL_LIMIT: u32 = 4;

// ---- configurations (sections 8.2 and 8.3) ------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub(super) struct Config {
    name: &'static str,
    /// Facts F1, F2 (one or two).
    facts: u8,
    /// Which facts conflict.
    conflict: [bool; 2],
    /// ENV on: F1's conflict is a missing Prepared record, which ENV supplies once.
    env: bool,
    /// One writer offers every fact as one batch.
    batch: bool,
    faults: u8,
    admissions: u8,
    /// Gating callers G1, G2, each with its own record, each offered once.
    gating: u8,
    /// G2's identity is in the initial ledger with other content.
    gating_conflict: bool,
    /// Acknowledgement actors, and attempts each.
    acks: u8,
    ack_attempts: u8,
    /// Malformed snapshots, forced through private state (F-6).
    malformed: bool,
    /// A cell's own append can meet a constraint (F-4).
    cell_constraint: bool,
    /// Worker verbs unwind (F-W).
    worker_unwinds: bool,
    /// DROP on (F-D).
    drop: bool,
    /// REPLAY on, this many times (F-R).
    replays: u8,
    /// Actor change A4: the drop paths record a blanket Uncertain (F-PT).
    blanket_uncertain: bool,
    /// L1 is claimed.
    l1: bool,
    /// The state cap.
    cap: u64,
    /// The time cap.
    time: Duration,
}

const DEFAULT_CAP: u64 = 250_000;
const DEFAULT_TIME: Duration = Duration::from_secs(180);
const FULL_CAP: u64 = 30_000_000;
const FULL_TIME: Duration = Duration::from_secs(45 * 60);

const BASE: Config = Config {
    name: "",
    facts: 2,
    conflict: [false, false],
    env: false,
    batch: false,
    faults: 0,
    admissions: 0,
    gating: 0,
    gating_conflict: false,
    acks: 0,
    ack_attempts: 0,
    malformed: false,
    cell_constraint: false,
    worker_unwinds: false,
    drop: false,
    replays: 0,
    blanket_uncertain: false,
    l1: true,
    cap: FULL_CAP,
    time: FULL_TIME,
};

const fn default_run(config: Config) -> Config {
    Config {
        cap: DEFAULT_CAP,
        time: DEFAULT_TIME,
        ..config
    }
}

/// X-1: two facts, two writers, settlement, one admission, 2 faults.
const X1: Config = default_run(Config {
    name: "X-1",
    faults: 2,
    admissions: 1,
    ..BASE
});
/// X-2: one fact, two gating callers, the worker, one admission, 1 fault.
const X2: Config = default_run(Config {
    name: "X-2",
    facts: 1,
    faults: 1,
    admissions: 1,
    gating: 2,
    ..BASE
});
/// X-3: two facts, one conflicting, ENV on, 1 fault.
const X3: Config = default_run(Config {
    name: "X-3",
    conflict: [true, false],
    env: true,
    faults: 1,
    ..BASE
});
/// X-4: one conflicting fact, one acknowledgement, one admission, 1 fault.
const X4: Config = default_run(Config {
    name: "X-4",
    facts: 1,
    conflict: [true, false],
    faults: 1,
    admissions: 1,
    acks: 1,
    ack_attempts: 1,
    ..BASE
});
/// X-5: one clean batch of two, 2 faults.
const X5: Config = default_run(Config {
    name: "X-5",
    batch: true,
    faults: 2,
    ..BASE
});

const F1: Config = Config {
    name: "F-1",
    faults: 3,
    admissions: 2,
    gating: 2,
    ..BASE
};
const F2: Config = Config {
    name: "F-2",
    conflict: [false, true],
    faults: 2,
    admissions: 2,
    gating: 2,
    ..BASE
};
const F3: Config = Config {
    name: "F-3",
    conflict: [true, true],
    env: true,
    faults: 2,
    admissions: 2,
    gating: 2,
    ..BASE
};
const F4: Config = Config {
    name: "F-4",
    cell_constraint: true,
    ..F2
};
const F5: Config = Config {
    name: "F-5",
    conflict: [true, true],
    faults: 2,
    admissions: 2,
    acks: 1,
    ack_attempts: 2,
    ..BASE
};
const F6: Config = Config {
    name: "F-6",
    malformed: true,
    ..F5
};
const F7: Config = Config {
    name: "F-7",
    conflict: [true, true],
    faults: 1,
    admissions: 1,
    acks: 2,
    ack_attempts: 1,
    ..BASE
};
const F8: Config = Config {
    name: "F-8",
    conflict: [true, false],
    faults: 2,
    admissions: 2,
    gating: 1,
    acks: 1,
    ack_attempts: 1,
    ..BASE
};
const FPT: Config = Config {
    name: "F-PT",
    blanket_uncertain: true,
    ..F3
};
const FB: Config = Config {
    name: "F-B",
    batch: true,
    ..F3
};
const FB2: Config = Config {
    name: "F-B2",
    batch: true,
    faults: 3,
    admissions: 1,
    ..BASE
};
const FG: Config = Config {
    name: "F-G",
    facts: 1,
    faults: 2,
    admissions: 2,
    gating: 2,
    gating_conflict: true,
    ..BASE
};
const FW: Config = Config {
    name: "F-W",
    worker_unwinds: true,
    ..F2
};
const FD: Config = Config {
    name: "F-D",
    faults: 2,
    drop: true,
    l1: false,
    ..F1
};
const FR: Config = Config {
    name: "F-R",
    replays: 3,
    ..F2
};

// ---- records, identities and the simulated ledger -----------------------------------------------

/// A 32-byte value whose first three bytes carry its meaning.
fn bytes(a: u8, b: u8, c: u8) -> [u8; 32] {
    let mut out = [0; 32];
    out[0] = a;
    out[1] = b;
    out[2] = c;
    out
}

fn fact_key(f: u8) -> RecordKey {
    RecordKey {
        id: bytes(1, f, 0),
        digest: bytes(1, f, 1),
    }
}

fn gating_key(g: u8) -> RecordKey {
    RecordKey {
        id: bytes(2, g, 0),
        digest: bytes(2, g, 1),
    }
}

fn commitment(f: u8) -> [u8; ID_LEN] {
    bytes(9, f, 0)
}

fn fact_member(f: u8) -> Member {
    Member {
        key: fact_key(f),
        part: Some(Part {
            commitment: commitment(f),
            role: Role::Started,
        }),
    }
}

fn gating_member(g: u8) -> Member {
    Member {
        key: gating_key(g),
        part: None,
    }
}

/// Originals that can be explained: facts 0 and 1, gating records 2 and 3.
fn original_of(key: &RecordKey) -> Option<u8> {
    (0..2)
        .find(|&f| fact_key(f) == *key)
        .or_else(|| (0..2).find(|&g| gating_key(g) == *key).map(|g| g + 2))
}

fn gating_of(key: &RecordKey) -> Option<u8> {
    (0..2).find(|&g| gating_key(g) == *key)
}

fn exit_code(exit: Exit) -> u8 {
    match exit {
        Exit::InCall => 0,
        Exit::ReturnedUncertain => 1,
        Exit::Unwound => 2,
    }
}

fn exit_from(code: u8) -> Exit {
    match code {
        0 => Exit::InCall,
        1 => Exit::ReturnedUncertain,
        _ => Exit::Unwound,
    }
}

/// One row of the simulated ledger. Its sequence is its position + 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimRow {
    /// F's Prepared record.
    Prepared(u8),
    /// Another record holding F's role (Started) for F's commitment.
    Holder(u8),
    /// G's identity with other content.
    Other(u8),
    Fact(u8),
    Gating(u8),
    /// An explanation of an original (see `original_of`); it requires
    /// acknowledgement.
    Explanation(u8),
    /// A correction about gating record G.
    Correction(u8, Exit),
    Acknowledgement {
        set: SetFacts,
        nonce: [u8; 16],
    },
}

impl SimRow {
    fn key(&self) -> RecordKey {
        match *self {
            SimRow::Prepared(f) => RecordKey {
                id: bytes(4, f, 0),
                digest: bytes(4, f, 1),
            },
            SimRow::Holder(f) => RecordKey {
                id: bytes(3, f, 0),
                digest: bytes(3, f, 1),
            },
            SimRow::Other(g) => RecordKey {
                id: gating_key(g).id,
                digest: bytes(2, g, 9),
            },
            SimRow::Fact(f) => fact_key(f),
            SimRow::Gating(g) => gating_key(g),
            SimRow::Explanation(o) => RecordKey {
                id: bytes(5, o, 0),
                digest: bytes(5, o, 1),
            },
            SimRow::Correction(g, exit) => RecordKey {
                id: bytes(6, g, exit_code(exit)),
                digest: bytes(6, g, 1),
            },
            SimRow::Acknowledgement { nonce, .. } => RecordKey {
                id: bytes(7, nonce[0], nonce[1]),
                digest: bytes(7, 0, 1),
            },
        }
    }

    fn part(&self) -> Option<Part> {
        let part = |f, role| {
            Some(Part {
                commitment: commitment(f),
                role,
            })
        };
        match *self {
            SimRow::Prepared(f) => part(f, Role::Prepared),
            SimRow::Holder(f) | SimRow::Fact(f) => part(f, Role::Started),
            _ => None,
        }
    }
}

fn seq_at(index: usize) -> Seq {
    index as Seq + 1
}

fn find_seq(rows: &[SimRow], wanted: impl Fn(&SimRow) -> bool) -> Option<Seq> {
    rows.iter().position(wanted).map(seq_at)
}

fn row_by_id(rows: &[SimRow], id: &[u8; ID_LEN]) -> Option<Row> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| row.key().id == *id)
        .map(|(at, row)| Row {
            seq: seq_at(at),
            digest: row.key().digest,
        })
}

/// The N of the last acknowledgement row, or zero.
fn durable_frontier(rows: &[SimRow]) -> Seq {
    rows.iter()
        .rev()
        .find_map(|row| match row {
            SimRow::Acknowledgement { set, .. } => Some(set.top),
            _ => None,
        })
        .unwrap_or(0)
}

/// The count and the digest (a mask of originals) of the explanations with
/// a sequence above `above` and at most `upto`.
fn explanations(rows: &[SimRow], above: Seq, upto: Seq) -> (u64, [u8; 32]) {
    let mut mask = 0u8;
    let mut count = 0;
    for (at, row) in rows.iter().enumerate() {
        let seq = seq_at(at);
        if let SimRow::Explanation(o) = row {
            if seq > above && seq <= upto {
                mask |= 1 << o;
                count += 1;
            }
        }
    }
    (count, bytes(mask, 0, 0))
}

fn survey(rows: &[SimRow]) -> SetFacts {
    let frontier = durable_frontier(rows);
    let head = rows.len() as Seq;
    let top = rows
        .iter()
        .rposition(|row| matches!(row, SimRow::Explanation(_)))
        .map_or(0, seq_at);
    let (count, digest) = explanations(rows, frontier, top);
    SetFacts {
        frontier,
        head,
        top,
        count,
        digest,
    }
}

fn member_facts(rows: &[SimRow], member: &Member) -> MemberFacts {
    let key = member.key;
    let holder = member.part.and_then(|part| {
        rows.iter()
            .enumerate()
            .find(|(_, row)| row.part() == Some(part))
            .map(|(at, row)| Holder {
                id: row.key().id,
                seq: seq_at(at),
            })
    });
    let prepared = member.part.is_some_and(|part| {
        let wanted = Some(Part {
            commitment: part.commitment,
            role: Role::Prepared,
        });
        rows.iter().any(|row| row.part() == wanted)
    });
    MemberFacts {
        row: row_by_id(rows, &key.id),
        explained: original_of(&key)
            .and_then(|o| find_seq(rows, |row| *row == SimRow::Explanation(o))),
        corrected: gating_of(&key).and_then(|g| {
            find_seq(
                rows,
                |row| matches!(row, SimRow::Correction(h, _) if *h == g),
            )
        }),
        holder,
        prepared,
    }
}

fn row_for_member(member: &Member) -> SimRow {
    if let Some(f) = (0..2).find(|&f| fact_key(f) == member.key) {
        return SimRow::Fact(f);
    }
    if let Some(g) = gating_of(&member.key) {
        return SimRow::Gating(g);
    }
    panic!("a record outside the configuration: {member:?}");
}

fn row_for_occupant(occupant: &Occupant) -> SimRow {
    match occupant {
        Occupant::Correction { about, exit, .. } => SimRow::Correction(
            gating_of(about).expect("a correction about a gating record"),
            *exit,
        ),
        Occupant::Explanation { original, .. } => {
            SimRow::Explanation(original_of(original).expect("an explanation of a known original"))
        }
        other => panic!("a cell's own append of {other:?}"),
    }
}

// ---- the choice oracle --------------------------------------------------------------------------

/// The choices of one step. A step is replayed with each sequence of
/// choices; a fault option is offered only while the budget lasts.
struct Oracle {
    prefix: Vec<u8>,
    /// (choice, options) at each choice point.
    taken: Vec<(u8, u8)>,
    faults: u8,
    fault: bool,
}

impl Oracle {
    fn new(prefix: Vec<u8>, faults: u8) -> Oracle {
        Oracle {
            prefix,
            taken: Vec::new(),
            faults,
            fault: false,
        }
    }

    /// One choice among options, each marked as a fault or not. Option 0
    /// must not be a fault.
    fn choose(&mut self, fault: &[bool]) -> usize {
        assert!(!fault[0], "option 0 is the fault-free one");
        let offered: Vec<usize> = (0..fault.len())
            .filter(|&k| !fault[k] || self.faults > 0)
            .collect();
        let at = self.taken.len();
        let pick = self.prefix.get(at).map_or(0, |&p| p as usize);
        assert!(pick < offered.len(), "a replayed choice is out of range");
        self.taken.push((pick as u8, offered.len() as u8));
        let option = offered[pick];
        if fault[option] {
            self.faults -= 1;
            self.fault = true;
        }
        option
    }

    /// A choice among `n` options, none a fault.
    fn branch(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        self.choose(&vec![false; n])
    }

    /// The next sequence of choices, depth first, or none.
    fn next(&self) -> Option<Vec<u8>> {
        let at = self
            .taken
            .iter()
            .rposition(|&(choice, options)| choice + 1 < options)?;
        let mut prefix: Vec<u8> = self.taken[..at].iter().map(|&(choice, _)| choice).collect();
        prefix.push(self.taken[at].0 + 1);
        Some(prefix)
    }

    fn sequence(&self) -> Vec<u8> {
        self.taken.iter().map(|&(choice, _)| choice).collect()
    }
}

// ---- the verbs, over the simulation -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Open,
    Facts,
    Insert,
    Commit,
    Rollback,
    Row,
    Explained,
    Acknowledgement,
    Frontier,
    Survey,
    Recheck,
    Present,
    Run,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Ok,
    Failed,
    Corrupt,
    Constraint,
    Unknown,
    Unwound,
}

/// The payload of an unwinding verb.
struct Unwound;

/// The storage verbs, implemented over the simulated ledger. Each answer
/// is a choice of the oracle.
struct Io<'a> {
    rows: &'a mut Vec<SimRow>,
    /// Rows issued in the open transaction, not yet durable, each marked if
    /// its insert answered `Failed` or unwound: such a row was issued, and
    /// whether the transaction holds it is not known.
    pending: Vec<(SimRow, bool)>,
    oracle: &'a mut Oracle,
    log: Vec<(Verb, Answer)>,
    /// The verbs of this operation may unwind.
    unwinds: bool,
    /// An insert of this operation may answer Constraint.
    constraint: bool,
}

impl<'a> Io<'a> {
    fn new(
        rows: &'a mut Vec<SimRow>,
        oracle: &'a mut Oracle,
        unwinds: bool,
        constraint: bool,
    ) -> Io<'a> {
        Io {
            rows,
            pending: Vec::new(),
            oracle,
            log: Vec::new(),
            unwinds,
            constraint,
        }
    }

    fn pick(&mut self, verb: Verb, faults: &[Answer]) -> Answer {
        let mut options = vec![Answer::Ok];
        options.extend_from_slice(faults);
        if self.unwinds {
            options.push(Answer::Unwound);
        }
        let fault: Vec<bool> = options.iter().map(|&a| a != Answer::Ok).collect();
        let answer = options[self.oracle.choose(&fault)];
        self.log.push((verb, answer));
        if answer == Answer::Unwound {
            self.undecided();
            resume_unwind(Box::new(Unwound));
        }
        answer
    }

    /// What the transaction may hold, and so what its records may be: the
    /// rows whose inserts succeeded; and, if one answered `Failed` or
    /// unwound, every issued row.
    fn holdings(&mut self) -> Vec<Vec<SimRow>> {
        let pending = std::mem::take(&mut self.pending);
        let mut holdings = vec![pending
            .iter()
            .filter(|(_, unknown)| !unknown)
            .map(|(row, _)| *row)
            .collect()];
        if pending.iter().any(|(_, unknown)| *unknown) {
            holdings.push(pending.iter().map(|(row, _)| *row).collect());
        }
        holdings
    }

    /// After `Commit::Unknown`, after `Rollback::Unknown` once an insert was
    /// issued, and after an unwind once an insert was issued: the records are
    /// present or absent, and both are explored. Every holding of the
    /// transaction is explored as present.
    fn undecided(&mut self) {
        let mut outcomes = self.holdings();
        outcomes.retain(|rows| !rows.is_empty());
        outcomes.insert(0, Vec::new());
        let pick = self.oracle.branch(outcomes.len());
        self.rows.append(&mut outcomes.swap_remove(pick));
    }

    fn read<T>(&mut self, verb: Verb, value: impl FnOnce(&[SimRow]) -> T) -> Read<T> {
        match self.pick(verb, &[Answer::Failed, Answer::Corrupt]) {
            Answer::Ok => Read::Got(value(self.rows)),
            Answer::Failed => Read::Failed,
            _ => Read::Corrupt,
        }
    }

    fn insert_row(&mut self, row: SimRow) -> Insert {
        // The row is issued, and marked until its insert succeeds: if this
        // insert unwinds, it may be written.
        self.pending.push((row, true));
        let faults: &[Answer] = if self.constraint {
            &[Answer::Constraint, Answer::Failed]
        } else {
            &[Answer::Failed]
        };
        match self.pick(Verb::Insert, faults) {
            Answer::Ok => {
                if let Some((_, unknown)) = self.pending.last_mut() {
                    *unknown = false;
                }
                Insert::Inserted((self.rows.len() + self.pending.len()) as Seq)
            }
            Answer::Constraint => {
                // A constraint refused this row.
                self.pending.pop();
                Insert::Constraint
            }
            // Issued, and not refused: it may be written. It stays marked.
            _ => Insert::Failed,
        }
    }

    fn do_commit(&mut self) -> Commit {
        match self.pick(Verb::Commit, &[Answer::Unknown]) {
            Answer::Ok => {
                // A-2: the records. Every holding is explored.
                let mut holdings = self.holdings();
                let pick = self.oracle.branch(holdings.len());
                self.rows.append(&mut holdings.swap_remove(pick));
                Commit::Durable
            }
            _ => {
                self.undecided();
                Commit::Unknown
            }
        }
    }

    fn do_rollback(&mut self) -> Rollback {
        match self.pick(Verb::Rollback, &[Answer::Unknown]) {
            Answer::Ok => {
                // A-1: no COMMIT issued and the rollback Undone: nothing.
                self.pending.clear();
                Rollback::Undone
            }
            _ => {
                self.undecided();
                Rollback::Unknown
            }
        }
    }

    fn do_open(&mut self) -> Read<()> {
        self.read(Verb::Open, |_| ())
    }
}

impl Txn for Io<'_> {
    fn open(&mut self) -> Read<()> {
        self.do_open()
    }

    fn facts(&mut self, _index: u16, member: &Member) -> Read<MemberFacts> {
        self.read(Verb::Facts, |rows| member_facts(rows, member))
    }

    fn insert(&mut self, _index: u16, member: &Member) -> Insert {
        self.insert_row(row_for_member(member))
    }

    fn commit(&mut self) -> Commit {
        self.do_commit()
    }

    fn rollback(&mut self) -> Rollback {
        self.do_rollback()
    }
}

impl Reads for Io<'_> {
    fn row(&mut self, id: &[u8; ID_LEN]) -> Read<Option<Row>> {
        self.read(Verb::Row, |rows| row_by_id(rows, id))
    }

    fn explained(&mut self, key: &RecordKey) -> Read<Option<Seq>> {
        self.read(Verb::Explained, |rows| {
            original_of(key).and_then(|o| find_seq(rows, |row| *row == SimRow::Explanation(o)))
        })
    }

    fn acknowledgement(&mut self, set: &SetFacts, nonce: &[u8; 16]) -> Read<Option<Seq>> {
        let wanted = SimRow::Acknowledgement {
            set: *set,
            nonce: *nonce,
        };
        self.read(Verb::Acknowledgement, |rows| {
            find_seq(rows, |row| *row == wanted)
        })
    }

    fn frontier(&mut self) -> Read<Seq> {
        self.read(Verb::Frontier, durable_frontier)
    }
}

impl AckTxn for Io<'_> {
    fn survey(&mut self) -> Read<SetFacts> {
        self.read(Verb::Survey, survey)
    }

    fn open(&mut self) -> Read<()> {
        self.do_open()
    }

    fn recheck(&mut self, above: Seq, upto: Seq) -> Read<SetFacts> {
        self.read(Verb::Recheck, |rows| {
            let (count, digest) = explanations(rows, above, upto);
            SetFacts {
                frontier: durable_frontier(rows),
                head: rows.len() as Seq,
                top: upto,
                count,
                digest,
            }
        })
    }

    fn insert(&mut self, snapshot: &Snapshot) -> Insert {
        self.insert_row(SimRow::Acknowledgement {
            set: snapshot.set(),
            nonce: snapshot.nonce(),
        })
    }

    fn commit(&mut self) -> Commit {
        self.do_commit()
    }

    fn rollback(&mut self) -> Rollback {
        self.do_rollback()
    }
}

impl CellTxn for Io<'_> {
    fn open(&mut self) -> Read<()> {
        self.do_open()
    }

    fn present(&mut self, occupant: &Occupant) -> Read<Option<Seq>> {
        let id = row_for_occupant(occupant).key().id;
        self.read(Verb::Present, |rows| {
            row_by_id(rows, &id).map(|row| row.seq)
        })
    }

    fn insert(&mut self, occupant: &Occupant) -> Insert {
        self.insert_row(row_for_occupant(occupant))
    }

    fn commit(&mut self) -> Commit {
        self.do_commit()
    }

    fn rollback(&mut self) -> Rollback {
        self.do_rollback()
    }
}

impl BarrierRun for Io<'_> {
    fn run(&mut self) -> bool {
        self.pick(Verb::Run, &[Answer::Failed]) == Answer::Ok
    }
}

/// Runs one store operation, catching an unwinding verb. Any other panic
/// is a failed test and is resumed.
fn run_op<T>(op: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(value) => Some(value),
        Err(payload) if payload.is::<Unwound>() => None,
        Err(payload) => resume_unwind(payload),
    }
}

// ---- the world ----------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Unattempted,
    Refused,
    Uncertain,
    Conflict,
    Paid,
    Retired,
}

fn is_debt(class: Class) -> bool {
    matches!(class, Class::Refused | Class::Uncertain | Class::Conflict)
}

/// The test's obligation table: a class per fact, the debt counter, H and A.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Table {
    class: [Class; 2],
    debt: u8,
    /// The highest sequence of a committed explanation the worker reported.
    h: Seq,
    /// The layer's copy of the acknowledgement frontier.
    a: Seq,
}

/// What a writer holds after its call, before it records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Committed,
    Counted(Counted),
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FwPos {
    Start,
    NoTicket,
    Armed,
    Got(Held),
    Recorded,
    Dropping,
    DropConfirm,
    Done,
}

/// FW: a first writer of one fact or of one batch.
#[derive(Debug)]
struct Fw {
    members: Vec<u8>,
    ticket: Option<Ticket>,
    pos: FwPos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Claim {
    Append,
    Lookup,
    Explain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeHeld {
    Fact(Held),
    Lookup(Lookup),
    Explain(Explain),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SePos {
    Idle,
    Claimed(u8, Claim),
    NoTicket(u8),
    Armed(u8),
    Got(u8, SeHeld),
    Dropping(u8),
    Confirm(u8),
}

/// SE: settlement.
#[derive(Debug)]
struct Se {
    ticket: Option<Ticket>,
    pos: SePos,
}

#[derive(Clone, Debug)]
enum Action {
    Nothing,
    Barrier,
    Resolve(CellToken),
    AppendC(CellToken),
    AppendX(CellToken),
    EmptyX(CellToken, Seq),
}

#[derive(Clone, Debug)]
enum WkPos {
    Idle,
    /// The agenda was read: its frontier and the step chosen.
    Seen(Seq, Action),
    Act(Action),
    /// Under actor change A3 only: H raised after X was emptied.
    RaiseH(Seq),
}

/// WK: the worker. `last` is its last cell operation, kept for REPLAY.
#[derive(Debug)]
struct Wk {
    worker: Option<Worker>,
    pos: WkPos,
    last: Option<Action>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GcPos {
    Start,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AckPos {
    Idle,
    /// A snapshot is held; the Owner confirms or declines.
    Holding,
    Confirmed,
    Applying(Seq),
}

#[derive(Debug)]
struct Ack {
    attempts: u8,
    snapshot: Option<Snapshot>,
    pos: AckPos,
}

/// History facts the properties read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Ghost {
    /// Fact f was once refused as a conflict, by an append or a lookup.
    conflicted: u8,
    /// ... by a batch append or a lookup (E6).
    conflicted_e6: u8,
    /// A failing result for fact f exists and has not been recorded.
    unrecorded: u8,
    /// Gating attempt g issued an insert.
    issued: u8,
    /// Gating caller g was told Committed.
    told: u8,
    /// Counted tickets dropped.
    dropped: u8,
}

struct World {
    core: Core,
    health: Health,
    rows: Vec<SimRow>,
    fw: Vec<Fw>,
    se: Se,
    wk: Wk,
    gc: Vec<GcPos>,
    ack: Vec<Ack>,
    /// Admissions left.
    adm: u8,
    /// Actor change A2 only: the bits kept by the first step.
    adm_kept: Option<u64>,
    env_done: bool,
    replays: u8,
    table: Table,
    faults: u8,
    ghost: Ghost,
}

impl Drop for World {
    /// A world that is discarded is not a step: its tickets are settled
    /// first, so that their drop touches no word.
    fn drop(&mut self) {
        let tickets = self
            .fw
            .iter_mut()
            .filter_map(|fw| fw.ticket.as_mut())
            .chain(self.se.ticket.as_mut());
        for ticket in tickets {
            ticket.state = TicketState::Settled;
        }
    }
}

fn bit(i: u8) -> u8 {
    1 << i
}

fn initial_rows(config: &Config) -> Vec<SimRow> {
    let mut rows = Vec::new();
    for f in 0..config.facts {
        let missing_prepared = config.conflict[f as usize] && config.env && f == 0;
        if !missing_prepared {
            rows.push(SimRow::Prepared(f));
        }
        if config.conflict[f as usize] && !missing_prepared {
            rows.push(SimRow::Holder(f));
        }
    }
    if config.gating_conflict {
        rows.push(SimRow::Other(1));
    }
    rows
}

fn initial(config: &Config) -> World {
    let (mut core, health) = Core::new(Limits {
        tickets: TICKET_LIMIT,
        refusals: REFUSAL_LIMIT,
    });
    assert!(core.session_opened());
    let worker = core.take_worker();
    let fw = if config.batch {
        vec![Fw {
            members: (0..config.facts).collect(),
            ticket: None,
            pos: FwPos::Start,
        }]
    } else {
        (0..config.facts)
            .map(|f| Fw {
                members: vec![f],
                ticket: None,
                pos: FwPos::Start,
            })
            .collect()
    };
    let world = World {
        core,
        health,
        rows: initial_rows(config),
        fw,
        se: Se {
            ticket: None,
            pos: SePos::Idle,
        },
        wk: Wk {
            worker,
            pos: WkPos::Idle,
            last: None,
        },
        gc: vec![GcPos::Start; config.gating as usize],
        ack: (0..config.acks)
            .map(|_| Ack {
                attempts: config.ack_attempts,
                snapshot: None,
                pos: AckPos::Idle,
            })
            .collect(),
        adm: config.admissions,
        adm_kept: None,
        env_done: false,
        replays: config.replays,
        table: Table {
            class: [Class::Unattempted; 2],
            debt: 0,
            h: 0,
            a: 0,
        },
        faults: config.faults,
        ghost: Ghost::default(),
    };
    hook::take_counts();
    world
}

// ---- the state key ------------------------------------------------------------------------------

/// The fields of the state key, listed in every receipt.
const KEY_FIELDS: &str = "word (low 16 bits, live, unapplied; recomposed exactly); core: phase, session, latch \
cause, U/C/X occupancy numbers and occupants (records, exits, sets, nonces, causes, stage), the three \
occupancy counters, every refusal-table entry with its cause, the qualified frontier, worker_taken, and the \
fixed limits (asserted); the simulated ledger's rows beyond the initial ones; every ticket's state and holder \
(FW1, FW2 or the batch writer, SE); the worker's presence; every retained token as cell and occupancy number \
(the worker's agenda step and its last token for REPLAY); every snapshot's values and nonce; each actor's \
position and any result it holds but has not applied; the test's table (classes, debt counter, H, A); every \
remaining budget (faults, admissions, attempts, replays); the A2 kept bits; ENV fired; history facts \
(conflicted, conflicted for E6, unrecorded results, gating insert issued, gating told Committed, Counted \
tickets dropped)";

struct Enc(Vec<u8>);

impl Enc {
    fn u(&mut self, mut value: u64) {
        loop {
            let low = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.0.push(low);
                return;
            }
            self.0.push(low | 0x80);
        }
    }

    fn b(&mut self, value: bool) {
        self.0.push(u8::from(value));
    }

    fn byte(&mut self, value: u8) {
        self.0.push(value);
    }

    /// A 32-byte value of this simulation: only its first three bytes may
    /// be set.
    fn x32(&mut self, value: &[u8; 32]) {
        assert!(
            value[3..].iter().all(|&b| b == 0),
            "a value outside the encoding"
        );
        self.0.extend_from_slice(&value[..3]);
    }

    fn nonce(&mut self, value: &[u8; 16]) {
        assert!(
            value[2..].iter().all(|&b| b == 0),
            "a nonce outside the encoding"
        );
        self.0.extend_from_slice(&value[..2]);
    }

    fn key(&mut self, key: &RecordKey) {
        self.x32(&key.id);
        self.x32(&key.digest);
    }

    fn part(&mut self, part: &Option<Part>) {
        match part {
            None => self.byte(0),
            Some(part) => {
                self.byte(1 + role_code(part.role));
                self.x32(&part.commitment);
            }
        }
    }

    fn member(&mut self, member: &Member) {
        self.key(&member.key);
        self.part(&member.part);
    }

    fn set(&mut self, set: &SetFacts) {
        self.u(set.frontier);
        self.u(set.head);
        self.u(set.top);
        self.u(set.count);
        self.x32(&set.digest);
    }

    fn cause(&mut self, cause: &Cause) {
        match cause.kind() {
            CauseKind::IdentityContent {
                id,
                existing,
                offered,
            } => {
                self.byte(0);
                self.x32(&id);
                self.u(existing.seq);
                self.x32(&existing.digest);
                self.x32(&offered);
            }
            CauseKind::DuplicateRole {
                part,
                holder,
                offered,
            } => {
                self.byte(1);
                self.part(&Some(part));
                self.x32(&holder.id);
                self.u(holder.seq);
                self.key(&offered);
            }
            CauseKind::MissingPrepared { part, offered } => {
                self.byte(2);
                self.part(&Some(part));
                self.key(&offered);
            }
        }
    }

    fn occupant(&mut self, occupant: &Occupant) {
        match occupant {
            Occupant::Gating { record, exit } => {
                self.byte(0);
                self.member(record.member());
                self.byte(exit_code(*exit));
            }
            Occupant::Acknowledgement { set, nonce } => {
                self.byte(1);
                self.set(set);
                self.nonce(nonce);
            }
            Occupant::Correction { about, seq, exit } => {
                self.byte(2);
                self.key(about);
                self.u(*seq);
                self.byte(exit_code(*exit));
            }
            Occupant::Explanation { original, cause } => {
                self.byte(3);
                self.key(original);
                self.cause(cause);
            }
        }
    }

    fn token(&mut self, shared: &Arc<Shared>, token: &CellToken) {
        assert!(
            Arc::ptr_eq(&token.ledger, shared),
            "a token of another world"
        );
        self.byte(cell_code(token.cell));
        self.u(token.occupancy);
    }

    fn ticket(&mut self, shared: &Arc<Shared>, ticket: &Option<Ticket>) {
        match ticket {
            None => self.byte(0),
            Some(ticket) => {
                assert!(
                    Arc::ptr_eq(&ticket.shared, shared),
                    "a ticket of another world"
                );
                match ticket.state {
                    TicketState::Armed => self.byte(1),
                    TicketState::Settled => self.byte(2),
                    TicketState::Counted(counted) => {
                        self.byte(3);
                        self.counted(&counted);
                    }
                }
            }
        }
    }

    fn counted(&mut self, counted: &Counted) {
        self.byte(u8::from(counted.persistence == Persistence::Uncertain));
        match counted.conflict_member {
            None => self.byte(0),
            Some(member) => {
                self.byte(1);
                self.u(u64::from(member));
            }
        }
    }

    fn held(&mut self, held: &Held) {
        match held {
            Held::Committed => self.byte(0),
            Held::Rejected => self.byte(1),
            Held::Counted(counted) => {
                self.byte(2);
                self.counted(counted);
            }
        }
    }

    fn action(&mut self, shared: &Arc<Shared>, action: &Action) {
        match action {
            Action::Nothing => self.byte(0),
            Action::Barrier => self.byte(1),
            Action::Resolve(token) => {
                self.byte(2);
                self.token(shared, token);
            }
            Action::AppendC(token) => {
                self.byte(3);
                self.token(shared, token);
            }
            Action::AppendX(token) => {
                self.byte(4);
                self.token(shared, token);
            }
            Action::EmptyX(token, seq) => {
                self.byte(5);
                self.token(shared, token);
                self.u(*seq);
            }
        }
    }

    fn known(&mut self, known: &Known) {
        match known {
            Known::Cause(cause) => {
                self.byte(0);
                self.cause(cause);
            }
            Known::Explained(seq) => {
                self.byte(1);
                self.u(*seq);
            }
        }
    }

    fn se_held(&mut self, held: &SeHeld) {
        match held {
            SeHeld::Fact(held) => {
                self.byte(0);
                self.held(held);
            }
            SeHeld::Lookup(lookup) => {
                self.byte(1);
                match lookup {
                    Lookup::Present(seq) => {
                        self.byte(0);
                        self.u(*seq);
                    }
                    Lookup::Absent => self.byte(1),
                    Lookup::Conflict(known) => {
                        self.byte(2);
                        self.known(known);
                    }
                    Lookup::Refused(entry) => {
                        self.byte(3);
                        self.byte(entry_code(*entry));
                    }
                    Lookup::Failed => self.byte(4),
                }
            }
            SeHeld::Explain(explain) => {
                self.byte(2);
                match explain {
                    Explain::Explained(seq) => {
                        self.byte(0);
                        self.u(*seq);
                    }
                    Explain::Pending => self.byte(1),
                    Explain::Busy => self.byte(2),
                    Explain::Unknown => self.byte(3),
                    Explain::Refused(entry) => {
                        self.byte(4);
                        self.byte(entry_code(*entry));
                    }
                }
            }
        }
    }
}

fn role_code(role: Role) -> u8 {
    match role {
        Role::Prepared => 0,
        Role::Started => 1,
        Role::End => 2,
    }
}

fn cell_code(cell: Cell) -> u8 {
    match cell {
        Cell::U => 0,
        Cell::C => 1,
        Cell::X => 2,
    }
}

fn entry_code(entry: Entry) -> u8 {
    match entry {
        Entry::NotOpen => 0,
        Entry::Latched => 1,
        Entry::NotHealthy => 2,
        Entry::CellsBusy => 3,
        Entry::Malformed => 4,
        Entry::Expired => 5,
    }
}

/// Reads the word without a step: the explorer's own access, outside the
/// two functions of rule R1 and invisible to the hook.
fn word_of(shared: &Shared) -> u64 {
    shared.word.load(std::sync::atomic::Ordering::SeqCst)
}

/// Every field of the core but its shared allocation, each with its presence
/// and its length marked, so that two cores encode alike only if they are
/// equal: phase, session, latch and cause; each cell's presence, occupancy
/// number and occupant, and X's stage with its sequence; the three occupancy
/// counters; the refusal table's size and each entry with its retained
/// cause; the qualified frontier; whether the worker was taken. The state
/// key and E8 both use it.
fn encode_core(e: &mut Enc, core: &Core) {
    e.byte(core.phase as u8);
    e.byte(core.session as u8);
    match core.latch {
        None => e.byte(0),
        Some(cause) => e.byte(1 + cause as u8),
    }
    for cell in [&core.u, &core.c] {
        match cell {
            None => e.byte(0),
            Some((n, occupant)) => {
                e.byte(1);
                e.u(*n);
                e.occupant(occupant);
            }
        }
    }
    match &core.x {
        None => e.byte(0),
        Some((n, occupant, stage)) => {
            e.byte(1);
            e.u(*n);
            e.occupant(occupant);
            match stage {
                Stage::Built => e.byte(0),
                Stage::Committed(seq) => {
                    e.byte(1);
                    e.u(*seq);
                }
            }
        }
    }
    for n in core.occupancies {
        e.u(n);
    }
    e.u(core.refusals.len() as u64);
    for (key, cause) in &core.refusals {
        e.key(key);
        e.cause(cause);
    }
    e.u(core.frontier);
    e.b(core.worker_taken);
}

fn encode(config: &Config, w: &World) -> Vec<u8> {
    let mut e = Enc(Vec::with_capacity(96));
    let shared = &w.core.shared;
    assert!(
        Arc::ptr_eq(&w.health.shared, shared),
        "the health handle of another world"
    );
    assert_eq!(shared.tickets, TICKET_LIMIT);
    assert_eq!(w.core.refusal_limit, REFUSAL_LIMIT);
    let word = word_of(shared);
    let low = word & FLAGS;
    let live = field(word, LIVE_SHIFT);
    let unapplied = field(word, UNAPPLIED_SHIFT);
    assert_eq!(
        low | with_field(0, LIVE_SHIFT, live) | with_field(0, UNAPPLIED_SHIFT, unapplied),
        word,
        "the word has bits outside its fields"
    );
    e.u(low);
    e.u(live);
    e.u(unapplied);
    // The core.
    encode_core(&mut e, &w.core);
    // The simulated ledger beyond its initial rows.
    let initial = initial_rows(config);
    assert_eq!(
        &w.rows[..initial.len()],
        &initial[..],
        "the initial rows changed"
    );
    let appended = &w.rows[initial.len()..];
    e.u(appended.len() as u64);
    for row in appended {
        match row {
            SimRow::Prepared(f) => {
                e.byte(0);
                e.byte(*f);
            }
            SimRow::Fact(f) => {
                e.byte(1);
                e.byte(*f);
            }
            SimRow::Gating(g) => {
                e.byte(2);
                e.byte(*g);
            }
            SimRow::Explanation(o) => {
                e.byte(3);
                e.byte(*o);
            }
            SimRow::Correction(g, exit) => {
                e.byte(4);
                e.byte(*g);
                e.byte(exit_code(*exit));
            }
            SimRow::Acknowledgement { set, nonce } => {
                e.byte(5);
                e.set(set);
                e.nonce(nonce);
            }
            other => panic!("an initial row appended: {other:?}"),
        }
    }
    // The actors, and every bound value they hold.
    for fw in &w.fw {
        e.ticket(shared, &fw.ticket);
        match fw.pos {
            FwPos::Start => e.byte(0),
            FwPos::NoTicket => e.byte(1),
            FwPos::Armed => e.byte(2),
            FwPos::Got(held) => {
                e.byte(3);
                e.held(&held);
            }
            FwPos::Recorded => e.byte(4),
            FwPos::Dropping => e.byte(5),
            FwPos::DropConfirm => e.byte(6),
            FwPos::Done => e.byte(7),
        }
    }
    e.ticket(shared, &w.se.ticket);
    match w.se.pos {
        SePos::Idle => e.byte(0),
        SePos::Claimed(f, claim) => {
            e.byte(1);
            e.byte(f);
            e.byte(claim as u8);
        }
        SePos::NoTicket(f) => {
            e.byte(2);
            e.byte(f);
        }
        SePos::Armed(f) => {
            e.byte(3);
            e.byte(f);
        }
        SePos::Got(f, held) => {
            e.byte(4);
            e.byte(f);
            e.se_held(&held);
        }
        SePos::Dropping(f) => {
            e.byte(5);
            e.byte(f);
        }
        SePos::Confirm(f) => {
            e.byte(6);
            e.byte(f);
        }
    }
    match &w.wk.worker {
        None => e.byte(0),
        Some(worker) => {
            assert!(
                Arc::ptr_eq(&worker.ledger, shared),
                "a worker of another world"
            );
            e.byte(1);
        }
    }
    match &w.wk.pos {
        WkPos::Idle => e.byte(0),
        WkPos::Seen(frontier, action) => {
            e.byte(1);
            e.u(*frontier);
            e.action(shared, action);
        }
        WkPos::Act(action) => {
            e.byte(2);
            e.action(shared, action);
        }
        WkPos::RaiseH(seq) => {
            e.byte(3);
            e.u(*seq);
        }
    }
    match &w.wk.last {
        None => e.byte(0),
        Some(action) => {
            e.byte(1);
            e.action(shared, action);
        }
    }
    for gc in &w.gc {
        e.byte(*gc as u8);
    }
    for ack in &w.ack {
        e.byte(ack.attempts);
        match &ack.snapshot {
            None => e.byte(0),
            Some(snapshot) => {
                assert!(
                    Arc::ptr_eq(&snapshot.ledger, shared),
                    "a snapshot of another world"
                );
                e.byte(1);
                e.set(&snapshot.set);
                e.nonce(&snapshot.nonce);
            }
        }
        match ack.pos {
            AckPos::Idle => e.byte(0),
            AckPos::Holding => e.byte(1),
            AckPos::Confirmed => e.byte(2),
            AckPos::Applying(seq) => {
                e.byte(3);
                e.u(seq);
            }
        }
    }
    e.byte(w.adm);
    match w.adm_kept {
        None => e.byte(0),
        Some(bits) => {
            e.byte(1);
            e.u(bits);
        }
    }
    e.b(w.env_done);
    e.byte(w.replays);
    // The test's table, the budget and the history.
    for class in w.table.class {
        e.byte(class as u8);
    }
    e.byte(w.table.debt);
    e.u(w.table.h);
    e.u(w.table.a);
    e.byte(w.faults);
    let g = &w.ghost;
    for value in [
        g.conflicted,
        g.conflicted_e6,
        g.unrecorded,
        g.issued,
        g.told,
        g.dropped,
    ] {
        e.byte(value);
    }
    e.0
}

struct Dec<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Dec<'_> {
    fn u(&mut self) -> u64 {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.byte();
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }

    fn byte(&mut self) -> u8 {
        let byte = self.bytes[self.at];
        self.at += 1;
        byte
    }

    fn b(&mut self) -> bool {
        self.byte() != 0
    }

    fn x32(&mut self) -> [u8; 32] {
        let (a, b, c) = (self.byte(), self.byte(), self.byte());
        bytes(a, b, c)
    }

    fn nonce(&mut self) -> [u8; 16] {
        let mut nonce = [0; 16];
        nonce[0] = self.byte();
        nonce[1] = self.byte();
        nonce
    }

    fn key(&mut self) -> RecordKey {
        RecordKey {
            id: self.x32(),
            digest: self.x32(),
        }
    }

    fn part(&mut self) -> Option<Part> {
        match self.byte() {
            0 => None,
            code => {
                let role = [Role::Prepared, Role::Started, Role::End][(code - 1) as usize];
                Some(Part {
                    commitment: self.x32(),
                    role,
                })
            }
        }
    }

    fn member(&mut self) -> Member {
        Member {
            key: self.key(),
            part: self.part(),
        }
    }

    fn set(&mut self) -> SetFacts {
        SetFacts {
            frontier: self.u(),
            head: self.u(),
            top: self.u(),
            count: self.u(),
            digest: self.x32(),
        }
    }

    fn cause(&mut self) -> Cause {
        let kind = match self.byte() {
            0 => CauseKind::IdentityContent {
                id: self.x32(),
                existing: Row {
                    seq: self.u(),
                    digest: self.x32(),
                },
                offered: self.x32(),
            },
            1 => {
                let part = self.part().expect("a part");
                let holder = Holder {
                    id: self.x32(),
                    seq: self.u(),
                };
                CauseKind::DuplicateRole {
                    part,
                    holder,
                    offered: self.key(),
                }
            }
            _ => CauseKind::MissingPrepared {
                part: self.part().expect("a part"),
                offered: self.key(),
            },
        };
        Cause { kind }
    }

    fn occupant(&mut self) -> Occupant {
        match self.byte() {
            0 => Occupant::Gating {
                record: GatingRecord::new(self.member()),
                exit: exit_from(self.byte()),
            },
            1 => Occupant::Acknowledgement {
                set: self.set(),
                nonce: self.nonce(),
            },
            2 => Occupant::Correction {
                about: self.key(),
                seq: self.u(),
                exit: exit_from(self.byte()),
            },
            _ => Occupant::Explanation {
                original: self.key(),
                cause: self.cause(),
            },
        }
    }

    fn token(&mut self, shared: &Arc<Shared>) -> CellToken {
        let cell = [Cell::U, Cell::C, Cell::X][self.byte() as usize];
        CellToken {
            ledger: Arc::clone(shared),
            cell,
            occupancy: self.u(),
        }
    }

    fn ticket(&mut self, shared: &Arc<Shared>) -> Option<Ticket> {
        let state = match self.byte() {
            0 => return None,
            1 => TicketState::Armed,
            2 => TicketState::Settled,
            _ => TicketState::Counted(self.counted()),
        };
        Some(Ticket {
            shared: Arc::clone(shared),
            state,
        })
    }

    fn counted(&mut self) -> Counted {
        let persistence = if self.b() {
            Persistence::Uncertain
        } else {
            Persistence::NotWritten
        };
        let conflict_member = if self.b() {
            Some(self.u() as u16)
        } else {
            None
        };
        Counted {
            persistence,
            conflict_member,
        }
    }

    fn held(&mut self) -> Held {
        match self.byte() {
            0 => Held::Committed,
            1 => Held::Rejected,
            _ => Held::Counted(self.counted()),
        }
    }

    fn action(&mut self, shared: &Arc<Shared>) -> Action {
        match self.byte() {
            0 => Action::Nothing,
            1 => Action::Barrier,
            2 => Action::Resolve(self.token(shared)),
            3 => Action::AppendC(self.token(shared)),
            4 => Action::AppendX(self.token(shared)),
            _ => {
                let token = self.token(shared);
                Action::EmptyX(token, self.u())
            }
        }
    }

    fn entry(&mut self) -> Entry {
        [
            Entry::NotOpen,
            Entry::Latched,
            Entry::NotHealthy,
            Entry::CellsBusy,
            Entry::Malformed,
            Entry::Expired,
        ][self.byte() as usize]
    }

    fn known(&mut self) -> Known {
        match self.byte() {
            0 => Known::Cause(self.cause()),
            _ => Known::Explained(self.u()),
        }
    }

    fn se_held(&mut self) -> SeHeld {
        match self.byte() {
            0 => SeHeld::Fact(self.held()),
            1 => SeHeld::Lookup(match self.byte() {
                0 => Lookup::Present(self.u()),
                1 => Lookup::Absent,
                2 => Lookup::Conflict(self.known()),
                3 => Lookup::Refused(self.entry()),
                _ => Lookup::Failed,
            }),
            _ => SeHeld::Explain(match self.byte() {
                0 => Explain::Explained(self.u()),
                1 => Explain::Pending,
                2 => Explain::Busy,
                3 => Explain::Unknown,
                _ => Explain::Refused(self.entry()),
            }),
        }
    }
}

const PHASES: [Phase; 3] = [Phase::Healthy, Phase::NeedBarrier, Phase::Resolving];
const SESSIONS: [Session; 3] = [Session::Bootstrap, Session::Open, Session::Sealed];
const LATCHES: [LatchCause; 7] = [
    LatchCause::Constraint,
    LatchCause::TableFull,
    LatchCause::TicketFault,
    LatchCause::CellInvariant,
    LatchCause::Inconsistent,
    LatchCause::Corrupt,
    LatchCause::Overflow,
];
const CLASSES: [Class; 6] = [
    Class::Unattempted,
    Class::Refused,
    Class::Uncertain,
    Class::Conflict,
    Class::Paid,
    Class::Retired,
];
const CLAIMS: [Claim; 3] = [Claim::Append, Claim::Lookup, Claim::Explain];

/// Builds a new, independent world from a key: a new shared allocation
/// holding the same word value, and everything made to refer to it.
fn decode(config: &Config, key: &[u8]) -> World {
    let mut d = Dec { bytes: key, at: 0 };
    let low = d.u();
    let live = d.u();
    let unapplied = d.u();
    let word = low | with_field(0, LIVE_SHIFT, live) | with_field(0, UNAPPLIED_SHIFT, unapplied);
    let shared = Arc::new(Shared {
        word: AtomicU64::new(word),
        tickets: TICKET_LIMIT,
    });
    let phase = PHASES[d.byte() as usize];
    let session = SESSIONS[d.byte() as usize];
    let latch = match d.byte() {
        0 => None,
        code => Some(LATCHES[(code - 1) as usize]),
    };
    let u = d.b().then(|| (d.u(), d.occupant()));
    let c = d.b().then(|| (d.u(), d.occupant()));
    let x = d.b().then(|| {
        let n = d.u();
        let occupant = d.occupant();
        let stage = if d.b() {
            Stage::Committed(d.u())
        } else {
            Stage::Built
        };
        (n, occupant, stage)
    });
    let occupancies = [d.u(), d.u(), d.u()];
    let mut refusals = BTreeMap::new();
    for _ in 0..d.u() {
        let key = d.key();
        refusals.insert(key, d.cause());
    }
    let frontier = d.u();
    let worker_taken = d.b();
    let core = Core {
        shared: Arc::clone(&shared),
        phase,
        session,
        latch,
        u,
        c,
        x,
        occupancies,
        refusals,
        refusal_limit: REFUSAL_LIMIT,
        frontier,
        worker_taken,
    };
    let mut rows = initial_rows(config);
    for _ in 0..d.u() {
        rows.push(match d.byte() {
            0 => SimRow::Prepared(d.byte()),
            1 => SimRow::Fact(d.byte()),
            2 => SimRow::Gating(d.byte()),
            3 => SimRow::Explanation(d.byte()),
            4 => SimRow::Correction(d.byte(), exit_from(d.byte())),
            _ => SimRow::Acknowledgement {
                set: d.set(),
                nonce: d.nonce(),
            },
        });
    }
    let writers = if config.batch { 1 } else { config.facts };
    let mut fw = Vec::new();
    for f in 0..writers {
        let ticket = d.ticket(&shared);
        let pos = match d.byte() {
            0 => FwPos::Start,
            1 => FwPos::NoTicket,
            2 => FwPos::Armed,
            3 => FwPos::Got(d.held()),
            4 => FwPos::Recorded,
            5 => FwPos::Dropping,
            6 => FwPos::DropConfirm,
            _ => FwPos::Done,
        };
        let members = if config.batch {
            (0..config.facts).collect()
        } else {
            vec![f]
        };
        fw.push(Fw {
            members,
            ticket,
            pos,
        });
    }
    let se_ticket = d.ticket(&shared);
    let se_pos = match d.byte() {
        0 => SePos::Idle,
        1 => {
            let f = d.byte();
            SePos::Claimed(f, CLAIMS[d.byte() as usize])
        }
        2 => SePos::NoTicket(d.byte()),
        3 => SePos::Armed(d.byte()),
        4 => {
            let f = d.byte();
            SePos::Got(f, d.se_held())
        }
        5 => SePos::Dropping(d.byte()),
        _ => SePos::Confirm(d.byte()),
    };
    let worker = d.b().then(|| Worker {
        ledger: Arc::clone(&shared),
    });
    let wk_pos = match d.byte() {
        0 => WkPos::Idle,
        1 => {
            let frontier = d.u();
            WkPos::Seen(frontier, d.action(&shared))
        }
        2 => WkPos::Act(d.action(&shared)),
        _ => WkPos::RaiseH(d.u()),
    };
    let last = d.b().then(|| d.action(&shared));
    let gc = (0..config.gating)
        .map(|_| {
            if d.byte() == 0 {
                GcPos::Start
            } else {
                GcPos::Done
            }
        })
        .collect();
    let mut ack = Vec::new();
    for _ in 0..config.acks {
        let attempts = d.byte();
        let snapshot = d.b().then(|| Snapshot {
            ledger: Arc::clone(&shared),
            set: d.set(),
            nonce: d.nonce(),
        });
        let pos = match d.byte() {
            0 => AckPos::Idle,
            1 => AckPos::Holding,
            2 => AckPos::Confirmed,
            _ => AckPos::Applying(d.u()),
        };
        ack.push(Ack {
            attempts,
            snapshot,
            pos,
        });
    }
    let adm = d.byte();
    let adm_kept = d.b().then(|| d.u());
    let env_done = d.b();
    let replays = d.byte();
    let class = [CLASSES[d.byte() as usize], CLASSES[d.byte() as usize]];
    let table = Table {
        class,
        debt: d.byte(),
        h: d.u(),
        a: d.u(),
    };
    let faults = d.byte();
    let ghost = Ghost {
        conflicted: d.byte(),
        conflicted_e6: d.byte(),
        unrecorded: d.byte(),
        issued: d.byte(),
        told: d.byte(),
        dropped: d.byte(),
    };
    assert_eq!(d.at, key.len(), "the key was not read whole");
    World {
        core,
        health: Health { shared },
        rows,
        fw,
        se: Se {
            ticket: se_ticket,
            pos: se_pos,
        },
        wk: Wk {
            worker,
            pos: wk_pos,
            last,
        },
        gc,
        ack,
        adm,
        adm_kept,
        env_done,
        replays,
        table,
        faults,
        ghost,
    }
}

// ---- observations of one step -------------------------------------------------------------------

/// What one step did, for E2, coverage, traces and properties.
#[derive(Default)]
struct Obs {
    /// Word operations the step's own actions account for (E2).
    views: u64,
    store_ops: u64,
    ticket_steps: u64,
    /// Originals whose explanation the store saw in the ledger in this step:
    /// X's own append committing or finding it present, or explain answering
    /// Explained (E11).
    explained: Vec<RecordKey>,
    /// Branch labels, for coverage.
    branches: Vec<String>,
    /// Violations, as (property, detail).
    violations: Vec<(&'static str, String)>,
    /// The step's description, when tracing.
    trace: Option<String>,
}

impl Obs {
    fn fail(&mut self, property: &'static str, detail: impl Into<String>) {
        self.violations.push((property, detail.into()));
    }

    fn note(&mut self, text: impl FnOnce() -> String) {
        if let Some(trace) = self.trace.as_mut() {
            if !trace.ends_with(": ") && !trace.is_empty() {
                trace.push_str("; ");
            }
            trace.push_str(&text());
        }
    }

    fn branch(&mut self, label: String) {
        self.branches.push(label);
    }
}

fn reason_label(reason: &Reason) -> String {
    match reason {
        Reason::Entry(entry) => format!("Entry({entry:?})"),
        Reason::Conflict { known, .. } => match known {
            Known::Cause(cause) => {
                format!("Conflict(new or retained cause {})", cause_label(cause))
            }
            Known::Explained(_) => "Conflict(explained)".to_string(),
        },
        Reason::SessionClosed { .. } => "SessionClosed".to_string(),
        Reason::Corrected { .. } => "Corrected".to_string(),
        Reason::TableFull { .. } => "TableFull".to_string(),
        Reason::Constraint => "Constraint".to_string(),
        Reason::Storage => "Storage".to_string(),
    }
}

fn cause_label(cause: &Cause) -> &'static str {
    match cause.kind() {
        CauseKind::IdentityContent { .. } => "IdentityContent",
        CauseKind::DuplicateRole { .. } => "DuplicateRole",
        CauseKind::MissingPrepared { .. } => "MissingPrepared",
    }
}

fn log_label(log: &[(Verb, Answer)]) -> String {
    log.iter()
        .map(|(verb, answer)| format!("{verb:?}:{answer:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// E5: the order of the verbs of one transactional operation.
fn verb_order(log: &[(Verb, Answer)], members: usize) -> Result<(), String> {
    if let Some(at) = log
        .iter()
        .position(|(_, answer)| *answer == Answer::Unwound)
    {
        if at + 1 != log.len() {
            return Err("a verb after an unwind".into());
        }
    }
    match log.first() {
        None => return Ok(()),
        Some((Verb::Open, Answer::Ok)) => {}
        Some((Verb::Open, _)) => {
            return if log.len() == 1 {
                Ok(())
            } else {
                Err("a verb after a failed open".into())
            };
        }
        Some(other) => return Err(format!("the first verb is {other:?}")),
    }
    let (mut reads, mut inserting, mut after_failure, mut ended, mut rollbacks) =
        (0, false, false, false, 0);
    let mut durable = false;
    for &(verb, answer) in &log[1..] {
        if ended {
            return Err(format!("{verb:?} after the transaction ended"));
        }
        if after_failure && verb != Verb::Rollback {
            return Err(format!(
                "{verb:?} after a failure, before or instead of one rollback"
            ));
        }
        match verb {
            Verb::Facts | Verb::Recheck | Verb::Present => {
                if inserting {
                    return Err(format!("{verb:?} after an insert"));
                }
                reads += 1;
                after_failure |= matches!(answer, Answer::Failed | Answer::Corrupt);
            }
            Verb::Insert => {
                if reads < members {
                    return Err("an insert before every member is classified".into());
                }
                inserting = true;
                after_failure |= matches!(answer, Answer::Failed | Answer::Constraint);
            }
            Verb::Commit => {
                durable = answer == Answer::Ok;
                ended = durable;
                after_failure |= answer == Answer::Unknown;
            }
            Verb::Rollback => {
                rollbacks += 1;
                ended = true;
            }
            other => return Err(format!("{other:?} inside a transaction")),
        }
        if answer == Answer::Unwound {
            ended = true;
        }
    }
    let unwound = log
        .last()
        .is_some_and(|(_, answer)| *answer == Answer::Unwound);
    if !unwound && !durable && rollbacks != 1 {
        return Err(format!("{rollbacks} rollbacks where exactly one is owed"));
    }
    if durable && rollbacks != 0 {
        return Err("a rollback after a durable commit".into());
    }
    Ok(())
}

// ---- the actors ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Actor {
    Fw(u8),
    Se,
    Wk,
    Gc(u8),
    Ack(u8),
    Adm,
    Env,
    Replay,
}

fn actors(config: &Config) -> Vec<Actor> {
    let writers = if config.batch { 1 } else { config.facts };
    let mut all: Vec<Actor> = (0..writers).map(Actor::Fw).collect();
    all.push(Actor::Se);
    all.push(Actor::Wk);
    all.extend((0..config.gating).map(Actor::Gc));
    all.extend((0..config.acks).map(Actor::Ack));
    all.push(Actor::Adm);
    if config.env {
        all.push(Actor::Env);
    }
    if config.replays > 0 {
        all.push(Actor::Replay);
    }
    all
}

/// Sets a fact's class in the test's table, keeping its debt counter.
fn set_class(table: &mut Table, f: u8, class: Class) {
    let old = table.class[f as usize];
    match (is_debt(old), is_debt(class)) {
        (false, true) => table.debt += 1,
        (true, false) => table.debt -= 1,
        _ => {}
    }
    table.class[f as usize] = class;
}

/// Section 4.3: the classes a Counted ticket records, member by member.
fn record_counted(w: &mut World, members: &[u8], counted: Counted) {
    for (k, &f) in members.iter().enumerate() {
        let class = match (counted.persistence, counted.conflict_member) {
            (Persistence::Uncertain, _) => Class::Uncertain,
            (Persistence::NotWritten, Some(member)) if member as usize == k => Class::Conflict,
            (Persistence::NotWritten, _) => Class::Refused,
        };
        set_class(&mut w.table, f, class);
        w.ghost.unrecorded &= !bit(f);
    }
}

/// A drop path: classes from the ticket, or Uncertain if it is not Counted.
/// Under actor change A4: a blanket Uncertain.
fn record_dropped(config: &Config, w: &mut World, members: &[u8], ticket: Option<Counted>) {
    match ticket {
        Some(counted) if !config.blanket_uncertain => record_counted(w, members, counted),
        _ => {
            for &f in members {
                set_class(&mut w.table, f, Class::Uncertain);
                w.ghost.unrecorded &= !bit(f);
            }
        }
    }
}

/// A failing fact result: its members are unrecorded, and a named conflict
/// is remembered (P6, E6).
fn note_fact_result(config: &Config, w: &mut World, members: &[u8], counted: Counted) {
    for (k, &f) in members.iter().enumerate() {
        w.ghost.unrecorded |= bit(f);
        if counted.conflict_member == Some(k as u16) {
            w.ghost.conflicted |= bit(f);
            if config.batch {
                w.ghost.conflicted_e6 |= bit(f);
            }
        }
    }
}

fn fact_names(members: &[u8]) -> Vec<String> {
    members.iter().map(|f| format!("F{}", f + 1)).collect()
}

/// One fact append, with E4 and E5 checked and its branch labelled.
fn fact_append(
    config: &Config,
    w: &mut World,
    o: &mut Oracle,
    obs: &mut Obs,
    ticket: &mut Ticket,
    members: &[u8],
) -> Option<FactOutcome> {
    let batch: Vec<FactRecord> = members
        .iter()
        .map(|&f| FactRecord::new(fact_member(f)))
        .collect();
    let before = w.rows.clone();
    let mut io = Io::new(&mut w.rows, o, true, true);
    let core = &mut w.core;
    let outcome = run_op(|| core.append_facts(ticket, &batch, &mut io));
    let log = std::mem::take(&mut io.log);
    drop(io);
    let issued = log.iter().any(|(verb, _)| *verb == Verb::Insert);
    obs.store_ops += 1;
    if let Err(order) = verb_order(&log, batch.len()) {
        obs.fail("E5", format!("append_facts: {order}: {}", log_label(&log)));
    }
    let counted = match &outcome {
        Some(FactOutcome::NotApplied { counted, .. }) => Some(*counted),
        None => ticket.counted(),
        _ => None,
    };
    let not_written = matches!(outcome, Some(FactOutcome::Rejected(_)))
        || counted.is_some_and(|c| c.persistence == Persistence::NotWritten);
    if not_written && w.rows != before {
        obs.fail(
            "E4",
            format!(
                "append_facts said Not written but the ledger changed: {}",
                log_label(&log)
            ),
        );
    }
    if outcome == Some(FactOutcome::Committed) {
        for &f in members {
            if !w.rows.contains(&SimRow::Fact(f)) {
                obs.fail(
                    "E4",
                    format!("append_facts said Committed but F{} is absent", f + 1),
                );
            }
        }
    }
    if let Some(counted) = counted {
        if counted.conflict_member.is_some() && counted.persistence != Persistence::NotWritten {
            obs.fail(
                "E4",
                "a result that names a conflict does not say Not written",
            );
        }
    }
    if let Some(FactOutcome::NotApplied { reason, .. }) = &outcome {
        let refused = matches!(
            reason,
            Reason::Conflict { .. }
                | Reason::SessionClosed { .. }
                | Reason::TableFull { .. }
                | Reason::Corrected { .. }
        );
        if refused && issued {
            obs.fail("E5", "an insert was issued although a member was refused");
        }
    }
    let label = match &outcome {
        Some(FactOutcome::Committed) if issued => "Committed (inserted)".to_string(),
        Some(FactOutcome::Committed) => "Committed (already present)".to_string(),
        Some(FactOutcome::NotApplied { counted, reason }) => {
            format!(
                "NotApplied {:?} {}",
                counted.persistence,
                reason_label(reason)
            )
        }
        Some(FactOutcome::Rejected(fault)) => format!("Rejected({fault:?})"),
        None if issued => "unwound after an insert was issued".to_string(),
        None => "unwound before any insert".to_string(),
    };
    obs.branch(format!("append_facts: {label}"));
    obs.note(|| {
        format!(
            "append_facts{:?} -> {label} [{}]",
            fact_names(members),
            log_label(&log)
        )
    });
    if let Some(counted) = counted {
        note_fact_result(config, w, members, counted);
    }
    outcome
}

/// FW's choice that replaces the call: the wait to enter the store
/// expires (a fault).
fn expires(o: &mut Oracle) -> bool {
    o.choose(&[false, true]) == 1
}

/// FW: section 7.2.
fn step_fw(config: &Config, w: &mut World, i: usize, o: &mut Oracle, obs: &mut Obs) -> bool {
    let members = w.fw[i].members.clone();
    match w.fw[i].pos {
        FwPos::Done => return false,
        FwPos::Start => {
            obs.ticket_steps += 1;
            match w.health.arm() {
                Some(ticket) => {
                    w.fw[i].ticket = Some(ticket);
                    w.fw[i].pos = FwPos::Armed;
                    obs.note(|| "arm -> Armed".into());
                }
                None => {
                    w.fw[i].pos = FwPos::NoTicket;
                    obs.note(|| "arm -> nothing".into());
                }
            }
        }
        FwPos::NoTicket => {
            for &f in &members {
                set_class(&mut w.table, f, Class::Refused);
            }
            w.fw[i].pos = FwPos::Done;
            obs.note(|| "[T] every member Refused".into());
        }
        FwPos::Armed => {
            let mut ticket = w.fw[i]
                .ticket
                .take()
                .expect("an armed writer holds its ticket");
            if expires(o) {
                obs.ticket_steps += 1;
                let outcome = ticket.expire();
                let FactOutcome::NotApplied { counted, .. } = outcome else {
                    panic!("an Armed ticket expired as {outcome:?}");
                };
                note_fact_result(config, w, &members, counted);
                w.fw[i].pos = FwPos::Got(Held::Counted(counted));
                obs.branch("expire: Armed -> Counted".into());
                obs.note(|| "the wait expires: expire".into());
            } else {
                let outcome = fact_append(config, w, o, obs, &mut ticket, &members);
                w.fw[i].pos = match outcome {
                    Some(FactOutcome::Committed) => FwPos::Got(Held::Committed),
                    Some(FactOutcome::NotApplied { counted, .. }) => {
                        FwPos::Got(Held::Counted(counted))
                    }
                    Some(FactOutcome::Rejected(_)) => FwPos::Got(Held::Rejected),
                    None => FwPos::Dropping,
                };
            }
            w.fw[i].ticket = Some(ticket);
        }
        FwPos::Got(held) => {
            let counted_ticket = matches!(held, Held::Counted(_));
            let options: &[bool] = if config.drop && counted_ticket {
                &[false, true, false]
            } else {
                &[false, true]
            };
            match o.choose(options) {
                0 => {
                    match held {
                        Held::Committed => {
                            for &f in &members {
                                set_class(&mut w.table, f, Class::Paid);
                            }
                        }
                        Held::Counted(counted) => record_counted(w, &members, counted),
                        Held::Rejected => {
                            for &f in &members {
                                set_class(&mut w.table, f, Class::Uncertain);
                                w.ghost.unrecorded &= !bit(f);
                            }
                        }
                    }
                    w.fw[i].pos = FwPos::Recorded;
                    obs.note(|| format!("[T] records {held:?}"));
                }
                1 => {
                    w.fw[i].pos = FwPos::Dropping;
                    obs.note(|| "unwinds before recording".into());
                }
                _ => {
                    // DROP: the Counted ticket is dropped unrecorded and
                    // unconfirmed.
                    obs.ticket_steps += 1;
                    drop(w.fw[i].ticket.take());
                    w.ghost.dropped += 1;
                    w.fw[i].pos = FwPos::Done;
                    obs.branch("DROP: a Counted ticket dropped".into());
                    obs.note(|| "DROP: drops its Counted ticket".into());
                }
            }
        }
        FwPos::Recorded => {
            if o.choose(&[false, true]) == 0 {
                obs.ticket_steps += 1;
                w.fw[i]
                    .ticket
                    .take()
                    .expect("a recorded writer holds its ticket")
                    .confirm();
                w.fw[i].pos = FwPos::Done;
                obs.note(|| "confirm".into());
            } else {
                w.fw[i].pos = FwPos::Dropping;
                obs.note(|| "unwinds before confirming".into());
            }
        }
        FwPos::Dropping => {
            let counted = w.fw[i].ticket.as_ref().and_then(Ticket::counted);
            record_dropped(config, w, &members, counted);
            w.fw[i].pos = FwPos::DropConfirm;
            obs.note(|| format!("[T] drop path records from {counted:?}"));
        }
        FwPos::DropConfirm => {
            obs.ticket_steps += 1;
            w.fw[i]
                .ticket
                .take()
                .expect("the drop path holds its ticket")
                .confirm();
            w.fw[i].pos = FwPos::Done;
            obs.note(|| "confirm".into());
        }
    }
    true
}

/// SE's guards (section 7.2), from one view and the class.
fn claimable(view: HealthView, class: Class) -> Option<Claim> {
    if view.handover_fault() || view.latched() {
        return None;
    }
    match class {
        Class::Refused if view.phase() == Phase::Healthy => Some(Claim::Append),
        Class::Uncertain if view.phase() != Phase::NeedBarrier => Some(Claim::Lookup),
        Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => {
            Some(Claim::Explain)
        }
        _ => None,
    }
}

/// One settlement read (lookup or explain), with its branch labelled.
fn settlement_read(
    w: &mut World,
    f: u8,
    claim: Claim,
    o: &mut Oracle,
    obs: &mut Obs,
) -> Option<SeHeld> {
    let key = fact_key(f);
    let mut io = Io::new(&mut w.rows, o, true, false);
    let core = &mut w.core;
    let (outcome, label, name) = if claim == Claim::Lookup {
        let outcome = run_op(|| core.lookup(&key, &mut io));
        let label = match outcome {
            Some(Lookup::Conflict(Known::Cause(_))) => "Conflict(cause)".to_string(),
            Some(Lookup::Conflict(Known::Explained(_))) => "Conflict(explained)".to_string(),
            Some(Lookup::Present(_)) => "Present".to_string(),
            Some(other) => format!("{other:?}"),
            None => "unwound".to_string(),
        };
        (outcome.map(SeHeld::Lookup), label, "lookup")
    } else {
        let outcome = run_op(|| core.explain(&key, &mut io));
        let label = match outcome {
            Some(Explain::Explained(_)) => "Explained".to_string(),
            Some(other) => format!("{other:?}"),
            None => "unwound".to_string(),
        };
        (outcome.map(SeHeld::Explain), label, "explain")
    };
    let log = std::mem::take(&mut io.log);
    drop(io);
    obs.store_ops += 1;
    obs.branch(format!("{name}: {label}"));
    obs.note(|| format!("{name} F{} -> {label} [{}]", f + 1, log_label(&log)));
    match outcome {
        Some(SeHeld::Lookup(Lookup::Conflict(_))) => {
            w.ghost.conflicted |= bit(f);
            w.ghost.conflicted_e6 |= bit(f);
        }
        Some(SeHeld::Explain(Explain::Explained(_))) => obs.explained.push(key),
        _ => {}
    }
    outcome
}

/// SE: section 7.2.
fn step_se(config: &Config, w: &mut World, o: &mut Oracle, obs: &mut Obs) -> bool {
    match w.se.pos {
        SePos::Idle => {
            obs.views += 1;
            let view = w.health.view();
            let eligible: Vec<(u8, Claim)> = (0..config.facts)
                .filter_map(|f| claimable(view, w.table.class[f as usize]).map(|c| (f, c)))
                .collect();
            if eligible.is_empty() {
                return false;
            }
            let (f, claim) = eligible[o.branch(eligible.len())];
            w.se.pos = SePos::Claimed(f, claim);
            obs.note(|| format!("[T] claims F{} for {claim:?}", f + 1));
        }
        SePos::Claimed(f, Claim::Append) => {
            obs.ticket_steps += 1;
            match w.health.arm() {
                Some(ticket) => {
                    w.se.ticket = Some(ticket);
                    w.se.pos = SePos::Armed(f);
                    obs.note(|| "arm -> Armed".into());
                }
                None => {
                    w.se.pos = SePos::NoTicket(f);
                    obs.note(|| "arm -> nothing".into());
                }
            }
        }
        SePos::Claimed(f, claim) => {
            w.se.pos = match settlement_read(w, f, claim, o, obs) {
                Some(held) => SePos::Got(f, held),
                None => SePos::Dropping(f),
            };
        }
        SePos::NoTicket(_) => {
            w.se.pos = SePos::Idle;
            obs.note(|| "[T] releases its claim".into());
        }
        SePos::Armed(f) => {
            let mut ticket =
                w.se.ticket
                    .take()
                    .expect("an armed settlement holds its ticket");
            let outcome = fact_append(config, w, o, obs, &mut ticket, &[f]);
            w.se.pos = match outcome {
                Some(FactOutcome::Committed) => SePos::Got(f, SeHeld::Fact(Held::Committed)),
                Some(FactOutcome::NotApplied { counted, .. }) => {
                    SePos::Got(f, SeHeld::Fact(Held::Counted(counted)))
                }
                Some(FactOutcome::Rejected(_)) => SePos::Got(f, SeHeld::Fact(Held::Rejected)),
                None => SePos::Dropping(f),
            };
            w.se.ticket = Some(ticket);
        }
        SePos::Got(f, held) => {
            match held {
                SeHeld::Fact(Held::Committed) | SeHeld::Lookup(Lookup::Present(_)) => {
                    set_class(&mut w.table, f, Class::Paid)
                }
                SeHeld::Lookup(Lookup::Absent) => set_class(&mut w.table, f, Class::Refused),
                SeHeld::Lookup(Lookup::Conflict(_)) => set_class(&mut w.table, f, Class::Conflict),
                SeHeld::Explain(Explain::Explained(_)) => {
                    set_class(&mut w.table, f, Class::Retired)
                }
                SeHeld::Fact(Held::Counted(counted)) => record_counted(w, &[f], counted),
                _ => {}
            }
            w.se.pos = if w.se.ticket.is_some() {
                SePos::Confirm(f)
            } else {
                SePos::Idle
            };
            obs.note(|| format!("[T] applies {held:?}; releases"));
        }
        SePos::Dropping(f) => {
            let counted = w.se.ticket.as_ref().and_then(Ticket::counted);
            if counted.is_some() || config.blanket_uncertain {
                record_dropped(config, w, &[f], counted);
            }
            w.se.pos = if w.se.ticket.is_some() {
                SePos::Confirm(f)
            } else {
                SePos::Idle
            };
            obs.note(|| format!("[T] drop path records from {counted:?}; releases"));
        }
        SePos::Confirm(_) => {
            obs.ticket_steps += 1;
            w.se.ticket
                .take()
                .expect("settlement holds its ticket")
                .confirm();
            w.se.pos = SePos::Idle;
            obs.note(|| "confirm".into());
        }
    }
    true
}

/// The steps an agenda allows the worker (sections 4.7 and 5.3).
fn allowed(agenda: &Agenda) -> Vec<Action> {
    let mut steps = Vec::new();
    let usable = agenda.session == Session::Open && agenda.latch.is_none();
    if agenda.phase == Phase::NeedBarrier {
        steps.push(Action::Barrier);
    }
    if agenda.phase == Phase::Resolving && usable {
        if let Some(u) = &agenda.u {
            steps.push(Action::Resolve(u.clone()));
        }
    }
    if agenda.phase == Phase::Healthy && usable {
        if let Some(c) = &agenda.c {
            steps.push(Action::AppendC(c.clone()));
        }
        if let Some((x, Stage::Built)) = &agenda.x {
            steps.push(Action::AppendX(x.clone()));
        }
    }
    if let Some((x, Stage::Committed(seq))) = &agenda.x {
        steps.push(Action::EmptyX(x.clone(), *seq));
    }
    steps
}

/// Whether a token names its cell's present occupancy, read from the core.
fn present_token(core: &Core, token: &CellToken) -> bool {
    Arc::ptr_eq(&token.ledger, &core.shared) && core.occupancy(token.cell) == Some(token.occupancy)
}

/// The core, encoded, for E8: every field the state key holds, and the two
/// limits fixed at creation. Which allocation the core refers to, and the
/// word in it, E8 compares on their own.
fn core_key(core: &Core) -> Vec<u8> {
    let mut e = Enc(Vec::new());
    encode_core(&mut e, core);
    e.u(u64::from(core.refusal_limit));
    e.u(u64::from(core.shared.tickets));
    e.0
}

/// One worker operation, with E4, E5 and E8 checked.
fn worker_op(config: &Config, w: &mut World, action: &Action, o: &mut Oracle, obs: &mut Obs) {
    let token = match action {
        Action::Resolve(t) | Action::AppendC(t) | Action::AppendX(t) | Action::EmptyX(t, _) => {
            Some(t.clone())
        }
        Action::Nothing | Action::Barrier => None,
    };
    let stale = token.as_ref().is_some_and(|t| !present_token(&w.core, t));
    let before_core = stale.then(|| (core_key(&w.core), Arc::as_ptr(&w.core.shared)));
    let before_word = word_of(&w.core.shared);
    let before_rows = w.rows.clone();
    let x_original = match &w.core.x {
        Some((_, Occupant::Explanation { original, .. }, _)) => Some(*original),
        _ => None,
    };
    let Some(worker) = w.wk.worker.as_mut() else {
        return;
    };
    let mut io = Io::new(
        &mut w.rows,
        o,
        config.worker_unwinds,
        config.cell_constraint,
    );
    let core = &mut w.core;
    let (label, stale_answer) = match action {
        Action::Nothing => return,
        Action::Barrier => {
            let outcome = run_op(|| core.barrier(worker, &mut io));
            (
                format!(
                    "barrier: {}",
                    outcome.map_or("unwound".into(), |o| format!("{o:?}"))
                ),
                false,
            )
        }
        Action::Resolve(token) => {
            let token = token.clone();
            let outcome = run_op(|| core.resolve(worker, token, &mut io));
            let stale_answer = matches!(outcome, Some(Resolve::Stale));
            let label = match outcome {
                Some(Resolve::Corrected(_)) => "Corrected".to_string(),
                Some(Resolve::ConflictPlaced(_)) => "ConflictPlaced".to_string(),
                Some(Resolve::Acknowledged(_)) => "Acknowledged".to_string(),
                Some(other) => format!("{other:?}"),
                None => "unwound".to_string(),
            };
            (format!("resolve: {label}"), stale_answer)
        }
        Action::AppendC(token) | Action::AppendX(token) => {
            let token = token.clone();
            let cell = token.cell;
            let outcome = run_op(|| core.append_cell(worker, token, &mut io));
            let stale_answer = outcome == Some(CellAppend::Stale);
            let log = io.log.clone();
            if !stale {
                if let Err(order) = verb_order(&log, 1) {
                    obs.fail("E5", format!("append_cell: {order}: {}", log_label(&log)));
                }
            }
            let inserted = log.iter().any(|(verb, _)| *verb == Verb::Insert);
            let label = match outcome {
                Some(CellAppend::Committed(_)) if inserted => "Committed (inserted)".to_string(),
                Some(CellAppend::Committed(_)) if log.is_empty() => {
                    "Committed (already marked)".to_string()
                }
                Some(CellAppend::Committed(_)) => "Committed (present)".to_string(),
                Some(other) => format!("{other:?}"),
                None => "unwound".to_string(),
            };
            if cell == Cell::X && label.starts_with("Committed (") && !label.ends_with("marked)") {
                if let Some(original) = x_original {
                    obs.explained.push(original);
                }
            }
            (format!("append_cell {cell:?}: {label}"), stale_answer)
        }
        Action::EmptyX(token, _) => {
            let token = token.clone();
            let outcome = run_op(|| core.empty(worker, token));
            let stale_answer = outcome == Some(Emptying::Stale);
            (
                format!(
                    "empty: {}",
                    outcome.map_or("unwound".into(), |o| format!("{o:?}"))
                ),
                stale_answer,
            )
        }
    };
    let log = std::mem::take(&mut io.log);
    drop(io);
    obs.store_ops += 1;
    obs.branch(label.clone());
    obs.note(|| format!("{label} [{}]", log_label(&log)));
    let unchanged =
        label.ends_with("NotWritten") || label.ends_with("Stale") || label.contains("Refused");
    if unchanged && w.rows != before_rows {
        obs.fail("E4", format!("{label}, but the ledger changed"));
    }
    if let Some((before, ledger)) = before_core {
        if !stale_answer
            || core_key(&w.core) != before
            || Arc::as_ptr(&w.core.shared) != ledger
            || word_of(&w.core.shared) != before_word
        {
            obs.fail(
                "E8",
                format!(
                    "a stale token: {label}; the answer is not stale, or the core or word changed"
                ),
            );
        }
    }
}

/// WK: section 7.2.
fn step_wk(config: &Config, w: &mut World, o: &mut Oracle, obs: &mut Obs) -> bool {
    match w.wk.pos.clone() {
        WkPos::Idle => {
            if w.wk.worker.is_none() {
                return false;
            }
            let agenda = w.core.agenda();
            let steps = allowed(&agenda);
            if steps.is_empty() && agenda.frontier <= w.table.a {
                return false;
            }
            let action = if steps.is_empty() {
                Action::Nothing
            } else {
                steps[o.branch(steps.len())].clone()
            };
            obs.note(|| {
                format!(
                    "reads the agenda (frontier {}), chooses {action:?}",
                    agenda.frontier
                )
            });
            w.wk.pos = WkPos::Seen(agenda.frontier, action);
        }
        WkPos::Seen(frontier, action) => {
            w.table.a = w.table.a.max(frontier);
            if let Action::EmptyX(_, seq) = &action {
                w.table.h = w.table.h.max(*seq);
            }
            obs.note(|| {
                "[T] A becomes the larger of A and the agenda's frontier; H before X is emptied"
                    .into()
            });
            w.wk.pos = if matches!(action, Action::Nothing) {
                WkPos::Idle
            } else {
                WkPos::Act(action)
            };
        }
        WkPos::Act(action) => {
            worker_op(config, w, &action, o, obs);
            if config.replays > 0 && !matches!(action, Action::Barrier | Action::Nothing) {
                w.wk.last = Some(action);
            }
            w.wk.pos = WkPos::Idle;
        }
        WkPos::RaiseH(seq) => {
            w.table.h = w.table.h.max(seq);
            obs.note(|| "[T] H raised".into());
            w.wk.pos = WkPos::Idle;
        }
    }
    true
}

/// GC: one gating append of its own record, offered once.
fn step_gc(w: &mut World, g: u8, o: &mut Oracle, obs: &mut Obs) -> bool {
    if w.gc[g as usize] == GcPos::Done {
        return false;
    }
    let record = GatingRecord::new(gating_member(g));
    let key = gating_key(g);
    let known_before =
        w.core.known(&key, None).is_some() || w.rows.contains(&SimRow::Explanation(g + 2));
    let before = w.rows.clone();
    let mut io = Io::new(&mut w.rows, o, true, true);
    let core = &mut w.core;
    let outcome = run_op(|| core.append_gating(&record, &mut io));
    let log = std::mem::take(&mut io.log);
    drop(io);
    obs.store_ops += 1;
    let issued = log.iter().any(|(verb, _)| *verb == Verb::Insert);
    if issued {
        w.ghost.issued |= bit(g);
    }
    if let Err(order) = verb_order(&log, 1) {
        obs.fail("E5", format!("append_gating: {order}: {}", log_label(&log)));
    }
    let u_holds_it =
        matches!(&w.core.u, Some((_, Occupant::Gating { record: r, .. })) if *r == record);
    let label = match &outcome {
        Some(GatingOutcome::Committed(_)) if issued => "Committed (inserted)".to_string(),
        Some(GatingOutcome::Committed(_)) => "Committed (already present)".to_string(),
        Some(GatingOutcome::Refused {
            persistence,
            reason,
        }) => {
            format!("Refused {persistence:?} {}", reason_label(reason))
        }
        None if u_holds_it => "unwound after an insert (U kept, Unwound)".to_string(),
        None => "unwound before any insert (U emptied)".to_string(),
    };
    obs.branch(format!("append_gating: {label}"));
    obs.note(|| format!("append_gating G{} -> {label} [{}]", g + 1, log_label(&log)));
    match &outcome {
        Some(GatingOutcome::Committed(_)) => {
            w.ghost.told |= bit(g);
            if !w.rows.contains(&SimRow::Gating(g)) {
                obs.fail(
                    "E4",
                    "append_gating said Committed but the record is absent",
                );
            }
        }
        Some(GatingOutcome::Refused {
            persistence,
            reason,
        }) => {
            if *persistence == Persistence::NotWritten && w.rows != before {
                obs.fail(
                    "E4",
                    "append_gating said Not written but the ledger changed",
                );
            }
            if let Reason::Conflict {
                known: Known::Cause(_),
                ..
            } = reason
            {
                if !known_before {
                    let x_holds = matches!(&w.core.x,
                        Some((_, Occupant::Explanation { original, .. }, _)) if *original == key);
                    if w.core.u.is_some() || !x_holds || w.core.refusals.contains_key(&key) {
                        obs.fail(
                            "E7",
                            "after a gating conflict with a new cause: U, X or the table is wrong",
                        );
                    }
                }
            }
        }
        None => {
            if !u_holds_it && w.rows != before {
                obs.fail(
                    "E4",
                    "an unwound gating append emptied U but the ledger changed",
                );
            }
        }
    }
    w.gc[g as usize] = GcPos::Done;
    true
}

/// Under F-6, the malformations forced through the snapshot's private state.
const MALFORMATIONS: [&str; 3] = ["future", "set", "empty"];

fn malform(snapshot: &mut Snapshot, which: usize) {
    let set = &mut snapshot.set;
    match which {
        // N beyond the snapshot's head: test 4 refuses it.
        0 => set.top = set.head + 2,
        // The Owner is shown one explanation fewer than the range holds:
        // test 5 refuses it.
        1 => {
            let mask = set.digest[0];
            let lowest = mask & mask.wrapping_neg();
            set.digest[0] = mask & !lowest;
            set.count = set.count.saturating_sub(1).max(1);
        }
        // Nothing to acknowledge: test 3 refuses it.
        _ => set.count = 0,
    }
}

/// ACK: snapshot; the Owner confirms or declines; the append; on Committed,
/// [T] A becomes the larger of A and N.
fn step_ack(config: &Config, w: &mut World, k: u8, o: &mut Oracle, obs: &mut Obs) -> bool {
    let i = k as usize;
    match w.ack[i].pos {
        AckPos::Idle => {
            if w.ack[i].attempts == 0 {
                return false;
            }
            let attempt = config.ack_attempts - w.ack[i].attempts;
            w.ack[i].attempts -= 1;
            let mut nonce = [0; 16];
            nonce[0] = k + 1;
            nonce[1] = attempt;
            let mut io = Io::new(&mut w.rows, o, true, true);
            let core = &mut w.core;
            let outcome = run_op(|| core.snapshot(nonce, &mut io));
            let log = std::mem::take(&mut io.log);
            drop(io);
            obs.store_ops += 1;
            let label = match &outcome {
                Some(SnapshotOutcome::Taken(_)) => "Taken".to_string(),
                Some(other) => format!("{other:?}"),
                None => "unwound".to_string(),
            };
            obs.branch(format!("snapshot: {label}"));
            obs.note(|| format!("snapshot -> {label} [{}]", log_label(&log)));
            if let Some(SnapshotOutcome::Taken(snapshot)) = outcome {
                w.ack[i].snapshot = Some(snapshot);
                w.ack[i].pos = AckPos::Holding;
            }
        }
        AckPos::Holding => {
            let options = if config.malformed {
                2 + MALFORMATIONS.len()
            } else {
                2
            };
            match o.branch(options) {
                0 => {
                    w.ack[i].pos = AckPos::Confirmed;
                    obs.note(|| "the Owner confirms".into());
                }
                1 => {
                    w.ack[i].snapshot = None;
                    w.ack[i].pos = AckPos::Idle;
                    obs.note(|| "the Owner declines".into());
                }
                which => {
                    let which = which - 2;
                    malform(w.ack[i].snapshot.as_mut().expect("a held snapshot"), which);
                    w.ack[i].pos = AckPos::Confirmed;
                    obs.branch(format!("malformed snapshot: {}", MALFORMATIONS[which]));
                    obs.note(|| {
                        format!(
                            "the snapshot is malformed ({}); the Owner confirms",
                            MALFORMATIONS[which]
                        )
                    });
                }
            }
        }
        AckPos::Confirmed => {
            let snapshot = w.ack[i].snapshot.take().expect("a confirmed snapshot");
            let row = SimRow::Acknowledgement {
                set: snapshot.set(),
                nonce: snapshot.nonce(),
            };
            let before = w.rows.clone();
            let mut io = Io::new(&mut w.rows, o, true, true);
            let core = &mut w.core;
            let outcome = run_op(|| core.append_acknowledgement(snapshot, &mut io));
            let log = std::mem::take(&mut io.log);
            drop(io);
            obs.store_ops += 1;
            if let Err(order) = verb_order(&log, 1) {
                obs.fail(
                    "E5",
                    format!("append_acknowledgement: {order}: {}", log_label(&log)),
                );
            }
            let u_kept = matches!(&w.core.u, Some((_, Occupant::Acknowledgement { .. })));
            let label = match &outcome {
                Some(AckOutcome::Committed(_)) => "Committed".to_string(),
                Some(other) => format!("{other:?}"),
                None if u_kept => "unwound after an insert (U kept)".to_string(),
                None => "unwound before any insert (U emptied)".to_string(),
            };
            obs.branch(format!("append_acknowledgement: {label}"));
            obs.note(|| format!("append_acknowledgement -> {label} [{}]", log_label(&log)));
            let not_written = matches!(
                outcome,
                Some(AckOutcome::Refused(_))
                    | Some(AckOutcome::NotApplied(Persistence::NotWritten))
            ) || (outcome.is_none() && !u_kept);
            if not_written && w.rows != before {
                obs.fail(
                    "E4",
                    "append_acknowledgement said Not written but the ledger changed",
                );
            }
            w.ack[i].pos = AckPos::Idle;
            if let Some(AckOutcome::Committed(n)) = outcome {
                if !w.rows.contains(&row) {
                    obs.fail(
                        "E4",
                        "append_acknowledgement said Committed but its row is absent",
                    );
                }
                w.ack[i].pos = AckPos::Applying(n);
            }
        }
        AckPos::Applying(n) => {
            w.table.a = w.table.a.max(n);
            w.ack[i].pos = AckPos::Idle;
            obs.note(|| format!("[T] A becomes the larger of A and {n}"));
        }
    }
    true
}

/// ADM: [T] one view; admit if `admits`, no debt, and H is not above A.
fn step_adm(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    if w.adm == 0 {
        return false;
    }
    obs.views += 1;
    let view = w.health.view();
    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;
    w.adm -= 1;
    obs.branch(format!(
        "admission: {}",
        if admitted { "admitted" } else { "refused" }
    ));
    obs.note(|| {
        format!(
            "[T] admission: {}",
            if admitted { "ADMITTED" } else { "refused" }
        )
    });
    if admitted {
        admission_properties(config, w, obs);
    }
    true
}

/// P1, P12, P15, E6 and E9 at a successful admission.
fn admission_properties(config: &Config, w: &World, obs: &mut Obs) {
    if w.ghost.unrecorded != 0 || w.core.u.is_some() || w.core.c.is_some() || w.core.x.is_some() {
        obs.fail(
            "P1",
            "a result is unrecorded, or U, C or X is occupied, at a successful admission",
        );
        if config.batch {
            obs.fail("E6", "P1 fails for a member of a batch");
        }
    }
    let covered: u8 = w
        .rows
        .iter()
        .filter_map(|row| match row {
            SimRow::Acknowledgement { set, .. } => Some(set.digest[0]),
            _ => None,
        })
        .fold(0, |mask, digest| mask | digest);
    let durable = durable_frontier(&w.rows);
    for (at, row) in w.rows.iter().enumerate() {
        if let SimRow::Explanation(o) = row {
            if covered & bit(*o) == 0 {
                obs.fail(
                    "P12",
                    format!(
                        "the explanation of original {o} is covered by no durable acknowledgement"
                    ),
                );
            }
            if seq_at(at) > durable {
                obs.fail(
                    "P15",
                    format!(
                        "an explanation at {} is above the durable frontier {durable}",
                        seq_at(at)
                    ),
                );
            }
        }
    }
    if w.core.latch.is_some() || word_of(&w.core.shared) & HANDOVER_FAULT != 0 {
        obs.fail(
            "E9",
            "an admission succeeded after a latch or a handover fault",
        );
    }
}

/// ENV: once, the Prepared record that F1 was missing appears.
fn step_env(w: &mut World, obs: &mut Obs) -> bool {
    if w.env_done {
        return false;
    }
    w.rows.push(SimRow::Prepared(0));
    w.env_done = true;
    obs.note(|| "F1's Prepared record appears".into());
    true
}

/// REPLAY: the worker repeats its last cell operation with a token that is
/// not the present one for its cell.
fn step_replay(config: &Config, w: &mut World, o: &mut Oracle, obs: &mut Obs) -> bool {
    if w.replays == 0 || !matches!(w.wk.pos, WkPos::Idle) {
        return false;
    }
    let Some(last) = w.wk.last.clone() else {
        return false;
    };
    let token = match &last {
        Action::Resolve(t) | Action::AppendC(t) | Action::AppendX(t) | Action::EmptyX(t, _) => t,
        _ => return false,
    };
    if present_token(&w.core, token) {
        return false;
    }
    w.replays -= 1;
    obs.note(|| "the worker repeats its last operation with an old token".into());
    worker_op(config, w, &last, o, obs);
    true
}

/// One step of one actor. False if the actor has no enabled step.
fn step(config: &Config, w: &mut World, actor: Actor, o: &mut Oracle, obs: &mut Obs) -> bool {
    if let Some(trace) = obs.trace.as_mut() {
        let _ = write!(trace, "{actor:?}: ");
    }
    match actor {
        Actor::Fw(i) => step_fw(config, w, i as usize, o, obs),
        Actor::Se => step_se(config, w, o, obs),
        Actor::Wk => step_wk(config, w, o, obs),
        Actor::Gc(g) => step_gc(w, g, o, obs),
        Actor::Ack(k) => step_ack(config, w, k, o, obs),
        Actor::Adm => step_adm(config, w, obs),
        Actor::Env => step_env(w, obs),
        Actor::Replay => step_replay(config, w, o, obs),
    }
}

// ---- properties ---------------------------------------------------------------------------------

/// The word's phase, session, latch and cell bits, computed by the test
/// from the core's fields (E1).
fn expected_flags(core: &Core) -> u64 {
    let phase = match core.phase {
        Phase::Healthy => 0,
        Phase::NeedBarrier => 0x1,
        Phase::Resolving => 0x2,
    };
    let session = match core.session {
        Session::Bootstrap => 0,
        Session::Open => 0x4,
        Session::Sealed => 0x8,
    };
    let mut flags = phase | session;
    for (set, bit) in [
        (core.latch.is_some(), 0x10),
        (core.u.is_some(), 0x20),
        (core.c.is_some(), 0x40),
        (core.x.is_some(), 0x80),
    ] {
        if set {
            flags |= bit;
        }
    }
    flags
}

/// Properties of every state.
fn state_properties(config: &Config, w: &World, obs: &mut Obs) {
    let word = word_of(&w.core.shared);
    if word & FLAGS & !HANDOVER_FAULT != expected_flags(&w.core) {
        obs.fail(
            "E1",
            format!(
                "word bits {:#x}, core {:#x}",
                word & FLAGS,
                expected_flags(&w.core)
            ),
        );
    }
    let tickets = w.fw.iter().map(|fw| &fw.ticket).chain([&w.se.ticket]);
    let (mut armed, mut counted) = (0u64, 0u64);
    for ticket in tickets.flatten() {
        match ticket.state {
            TicketState::Armed => armed += 1,
            TicketState::Counted(_) => counted += 1,
            TicketState::Settled => {}
        }
    }
    let dropped = u64::from(w.ghost.dropped);
    let (live, unapplied) = (field(word, LIVE_SHIFT), field(word, UNAPPLIED_SHIFT));
    if unapplied != counted + dropped
        || live != armed + counted + dropped
        || live > u64::from(TICKET_LIMIT)
    {
        obs.fail(
            "E3",
            format!("live {live}, unapplied {unapplied}; armed {armed}, counted {counted}, dropped {dropped}"),
        );
    }
    let debts = w.table.class[..config.facts as usize]
        .iter()
        .filter(|c| is_debt(**c))
        .count();
    if usize::from(w.table.debt) != debts {
        obs.fail("P2", "the debt counter is not the number of debts");
    }
    if w.core.u.is_some() && (w.core.c.is_some() || w.core.x.is_some()) {
        obs.fail("P4", "U is occupied together with C or X");
    }
    for f in 0..config.facts {
        let class = w.table.class[f as usize];
        if class == Class::Paid && !w.rows.contains(&SimRow::Fact(f)) {
            obs.fail("P5", format!("F{} is Paid and absent", f + 1));
        }
        if class == Class::Retired && !w.rows.contains(&SimRow::Explanation(f)) {
            obs.fail(
                "P5",
                format!("F{} is Retired and its explanation is absent", f + 1),
            );
        }
        if class == Class::Paid && w.ghost.conflicted & bit(f) != 0 {
            obs.fail(
                "P6",
                format!("F{} was refused as a conflict and is Paid", f + 1),
            );
            if w.ghost.conflicted_e6 & bit(f) != 0 {
                obs.fail(
                    "E6",
                    format!(
                        "P6 fails for F{}, a batch member or a lookup conflict",
                        f + 1
                    ),
                );
            }
        }
    }
    for g in 0..config.gating {
        let record_in = w.rows.contains(&SimRow::Gating(g));
        if w.ghost.told & bit(g) != 0 && !record_in {
            obs.fail("P7", format!("G{} was told Committed and is absent", g + 1));
        }
        if w.ghost.issued & bit(g) != 0 && w.ghost.told & bit(g) == 0 {
            let key = gating_key(g);
            let in_u = matches!(&w.core.u, Some((_, Occupant::Gating { record, .. })) if record.member().key == key);
            let in_c =
                matches!(&w.core.c, Some((_, Occupant::Correction { about, .. })) if *about == key);
            let corrected = w
                .rows
                .iter()
                .any(|row| matches!(row, SimRow::Correction(h, _) if *h == g));
            if record_in && !in_u && !in_c && !corrected {
                obs.fail(
                    "P8",
                    format!("G{}'s attempt is written, uncorrected and not in U", g + 1),
                );
            }
        }
    }
    if w.core.latch == Some(LatchCause::CellInvariant) {
        obs.fail("P9", "the core latched for a broken cell invariant");
    }
    let durable = durable_frontier(&w.rows);
    let head = w.rows.len() as Seq;
    if !(w.table.a <= w.core.frontier && w.core.frontier <= durable && durable <= head) {
        obs.fail(
            "P13",
            format!(
                "A {} qualified {} durable {durable} head {head}",
                w.table.a, w.core.frontier
            ),
        );
    }
    if w.core.refusals.len() > REFUSAL_LIMIT as usize {
        obs.fail("E11", "the refusal table exceeds its limit");
    }
}

/// Properties of a transition: E2, E9 and E11.
fn transition_properties(before: &World, after: &World, obs: &mut Obs, counts: hook::Counts) {
    if counts.loads != obs.views
        || counts.exchanges < obs.ticket_steps
        || counts.exchanges > obs.ticket_steps + obs.store_ops
    {
        obs.fail(
            "E2",
            format!(
                "loads {}, exchanges {}; views {}, ticket steps {}, store operations {}",
                counts.loads, counts.exchanges, obs.views, obs.ticket_steps, obs.store_ops
            ),
        );
    }
    let (wb, wa) = (word_of(&before.core.shared), word_of(&after.core.shared));
    if (before.core.latch.is_some() && after.core.latch.is_none())
        || (wb & HANDOVER_FAULT != 0 && wa & HANDOVER_FAULT == 0)
    {
        obs.fail("E9", "a latch or a handover fault was cleared");
    }
    for (key, cause) in &before.core.refusals {
        match after.core.refusals.get(key) {
            Some(now) if now != cause => obs.fail("E11", "a retained cause changed"),
            Some(_) => {}
            None => {
                let o = original_of(key).unwrap_or(u8::MAX);
                if !obs.explained.contains(key) || !after.rows.contains(&SimRow::Explanation(o)) {
                    obs.fail("E11", "a table entry left in a step that did not see its explanation in the ledger");
                }
            }
        }
    }
}

/// D1: a state with no enabled step is terminal.
fn terminal(config: &Config, w: &World) -> bool {
    let actors_done = w.fw.iter().all(|fw| fw.pos == FwPos::Done)
        && w.se.pos == SePos::Idle
        && matches!(w.wk.pos, WkPos::Idle)
        && w.gc.iter().all(|gc| *gc == GcPos::Done)
        && w.ack
            .iter()
            .all(|ack| ack.pos == AckPos::Idle && ack.attempts == 0)
        && w.adm == 0
        && (!config.env || w.env_done);
    let tickets_held = w.fw.iter().any(|fw| fw.ticket.is_some()) || w.se.ticket.is_some();
    let latched = w.core.latch.is_some();
    let fault = word_of(&w.core.shared) & HANDOVER_FAULT != 0;
    let settled = w.core.u.is_none()
        && w.core.c.is_none()
        && w.core.x.is_none()
        && w.core.phase == Phase::Healthy
        && w.table.class[..config.facts as usize]
            .iter()
            .all(|c| matches!(c, Class::Paid | Class::Retired));
    actors_done && !tickets_held && (latched || fault || settled)
}

// ---- the explorer -------------------------------------------------------------------------------

/// An edge as stored: the actor and the step's choices, 3 bits each.
fn pack(actor: usize, choices: &[u8]) -> u64 {
    assert!(actor < 16, "more than 16 actors");
    assert!(choices.len() <= 20, "a step with more than 20 choices");
    let mut packed = actor as u64;
    for (k, &choice) in choices.iter().enumerate() {
        assert!(choice < 7, "a choice index above 6");
        packed |= u64::from(choice + 1) << (4 + 3 * k);
    }
    packed
}

fn unpack(packed: u64) -> (usize, Vec<u8>) {
    let actor = (packed & 0xf) as usize;
    let mut choices = Vec::new();
    let mut rest = packed >> 4;
    while rest & 0x7 != 0 {
        choices.push((rest & 0x7) as u8 - 1);
        rest >>= 3;
    }
    (actor, choices)
}

/// The visited set: keys in one arena, found through an open-addressing
/// table of state numbers. Whole keys are compared, never only hashes.
struct Visited {
    arena: Vec<u8>,
    ends: Vec<u64>,
    slots: Vec<u32>,
}

const EMPTY: u32 = u32::MAX;

fn hash(key: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for chunk in key.chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        h = (h ^ u64::from_le_bytes(word)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h ^= h >> 29;
    }
    h
}

impl Visited {
    fn new() -> Visited {
        Visited {
            arena: Vec::new(),
            ends: Vec::new(),
            slots: vec![EMPTY; 1 << 16],
        }
    }

    fn len(&self) -> usize {
        self.ends.len()
    }

    fn key(&self, id: u32) -> &[u8] {
        let end = self.ends[id as usize] as usize;
        let start = if id == 0 {
            0
        } else {
            self.ends[id as usize - 1] as usize
        };
        &self.arena[start..end]
    }

    /// The state's number, and whether it is new.
    fn insert(&mut self, key: &[u8]) -> (u32, bool) {
        if self.len() * 10 >= self.slots.len() * 6 {
            self.grow();
        }
        let mask = self.slots.len() - 1;
        let mut at = hash(key) as usize & mask;
        loop {
            match self.slots[at] {
                EMPTY => {
                    let id = self.len() as u32;
                    assert!(id != EMPTY, "state numbers exhausted");
                    self.arena.extend_from_slice(key);
                    self.ends.push(self.arena.len() as u64);
                    self.slots[at] = id;
                    return (id, true);
                }
                id if self.key(id) == key => return (id, false),
                _ => at = (at + 1) & mask,
            }
        }
    }

    fn grow(&mut self) {
        let mut slots = vec![EMPTY; self.slots.len() * 2];
        let mask = slots.len() - 1;
        for id in 0..self.len() as u32 {
            let mut at = hash(self.key(id)) as usize & mask;
            while slots[at] != EMPTY {
                at = (at + 1) & mask;
            }
            slots[at] = id;
        }
        self.slots = slots;
    }

    fn bytes(&self) -> usize {
        self.arena.capacity() + self.ends.capacity() * 8 + self.slots.capacity() * 4
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    Pass,
    Fail,
    Incomplete,
}

pub(super) struct Report {
    pub(super) verdict: Verdict,
    pub(super) text: String,
    pub(super) failed: BTreeSet<&'static str>,
}

const PROPERTIES: [&str; 25] = [
    "P1", "P2", "P4", "P5", "P6", "P7", "P8", "P9", "P12", "P13", "P15", "D1", "L1", "E1", "E2",
    "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "KEY",
];

/// Every branch label the receipts account for, by operation. A label not
/// taken in a run is listed as not taken.
const BRANCHES: &[&str] = &[
    "append_facts: Committed (inserted)",
    "append_facts: Committed (already present)",
    "append_facts: NotApplied NotWritten Entry(NotHealthy)",
    "append_facts: NotApplied NotWritten Entry(Latched)",
    "append_facts: NotApplied NotWritten Storage",
    "append_facts: NotApplied NotWritten Constraint",
    "append_facts: NotApplied Uncertain Constraint",
    "append_facts: NotApplied Uncertain Storage",
    "append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)",
    "append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)",
    "append_facts: NotApplied NotWritten Conflict(explained)",
    "append_facts: unwound before any insert",
    "append_facts: unwound after an insert was issued",
    "expire: Armed -> Counted",
    "append_gating: Committed (inserted)",
    "append_gating: Refused NotWritten Entry(NotHealthy)",
    "append_gating: Refused NotWritten Entry(CellsBusy)",
    "append_gating: Refused NotWritten Entry(Latched)",
    "append_gating: Refused NotWritten Storage",
    "append_gating: Refused NotWritten Constraint",
    "append_gating: Refused Uncertain Storage",
    "append_gating: Refused Uncertain Constraint",
    "append_gating: Refused NotWritten Conflict(new or retained cause IdentityContent)",
    "append_gating: unwound after an insert (U kept, Unwound)",
    "append_gating: unwound before any insert (U emptied)",
    "lookup: Present",
    "lookup: Absent",
    "lookup: Conflict(cause)",
    "lookup: Conflict(explained)",
    "lookup: Refused(NotHealthy)",
    "lookup: Refused(Latched)",
    "lookup: Failed",
    "lookup: unwound",
    "explain: Explained",
    "explain: Pending",
    "explain: Busy",
    "explain: Unknown",
    "explain: Refused(Latched)",
    "explain: unwound",
    "snapshot: Taken",
    "snapshot: Nothing",
    "snapshot: NotNow(NotHealthy)",
    "snapshot: NotNow(Latched)",
    "snapshot: Failed",
    "snapshot: unwound",
    "append_acknowledgement: Committed",
    "append_acknowledgement: Refused(Stale)",
    "append_acknowledgement: Refused(Empty)",
    "append_acknowledgement: Refused(Future)",
    "append_acknowledgement: Refused(Set)",
    "append_acknowledgement: Refused(NotNow(NotHealthy))",
    "append_acknowledgement: Refused(NotNow(CellsBusy))",
    "append_acknowledgement: Refused(NotNow(Latched))",
    "append_acknowledgement: NotApplied(NotWritten)",
    "append_acknowledgement: NotApplied(Uncertain)",
    "append_acknowledgement: unwound after an insert (U kept)",
    "append_acknowledgement: unwound before any insert (U emptied)",
    "barrier: Passed",
    "barrier: Failed",
    "barrier: unwound",
    "resolve: Absent",
    "resolve: Corrected",
    "resolve: ConflictPlaced",
    "resolve: Acknowledged",
    "resolve: Retained",
    "resolve: Inconsistent",
    "resolve: Stale",
    "resolve: Refused(Latched)",
    "resolve: unwound",
    "append_cell C: Committed (inserted)",
    "append_cell C: Committed (present)",
    "append_cell C: NotWritten",
    "append_cell C: Uncertain",
    "append_cell C: Stale",
    "append_cell C: unwound",
    "append_cell X: Committed (inserted)",
    "append_cell X: Committed (present)",
    "append_cell X: NotWritten",
    "append_cell X: Uncertain",
    "append_cell X: Stale",
    "append_cell X: Refused(Latched)",
    "append_cell X: unwound",
    "empty: Done",
    "empty: Stale",
    "admission: admitted",
    "admission: refused",
    "DROP: a Counted ticket dropped",
    "malformed snapshot: future",
    "malformed snapshot: set",
    "malformed snapshot: empty",
];

/// One successor of a state.
struct Successor {
    key: Vec<u8>,
    fault: bool,
    edge: u64,
    violations: Vec<(&'static str, String)>,
    branches: Vec<String>,
}

/// Every successor of one state, every choice sequence of every actor, and
/// the violations of the state itself.
fn successors(
    config: &Config,
    all: &[Actor],
    key: &[u8],
) -> (Vec<Successor>, Vec<(&'static str, String)>) {
    let base = decode(config, key);
    let mut state = Obs::default();
    state_properties(config, &base, &mut state);
    if encode(config, &base) != key {
        state.fail("KEY", "a key does not survive decoding");
    }
    let mut out = Vec::new();
    for (index, &actor) in all.iter().enumerate() {
        let mut prefix = Vec::new();
        loop {
            let mut w = decode(config, key);
            let mut oracle = Oracle::new(prefix, w.faults);
            let mut obs = Obs::default();
            hook::take_counts();
            let enabled = step(config, &mut w, actor, &mut oracle, &mut obs);
            let counts = hook::take_counts();
            if !enabled {
                break;
            }
            w.faults = oracle.faults;
            transition_properties(&base, &w, &mut obs, counts);
            out.push(Successor {
                key: encode(config, &w),
                fault: oracle.fault,
                edge: pack(index, &oracle.sequence()),
                violations: obs.violations,
                branches: obs.branches,
            });
            match oracle.next() {
                Some(next) => prefix = next,
                None => break,
            }
        }
    }
    (out, state.violations)
}

/// Replays a path of packed edges from the initial state, describing each
/// step.
fn trace(config: &Config, edges: &[u64]) -> String {
    let all = actors(config);
    let mut key = encode(config, &initial(config));
    let mut text = String::new();
    for (n, &edge) in edges.iter().enumerate() {
        let (actor, choices) = unpack(edge);
        let mut w = decode(config, &key);
        let mut oracle = Oracle::new(choices, w.faults);
        let mut obs = Obs {
            trace: Some(String::new()),
            ..Obs::default()
        };
        step(config, &mut w, all[actor], &mut oracle, &mut obs);
        w.faults = oracle.faults;
        let _ = writeln!(
            text,
            "    {:>3}. {}{}",
            n + 1,
            obs.trace.unwrap_or_default(),
            if oracle.fault { "   [fault]" } else { "" }
        );
        key = encode(config, &w);
    }
    let w = decode(config, &key);
    let _ = writeln!(
        text,
        "    end: phase {:?}, latch {:?}, U {:?}, C {:?}, X {:?}, table {:?}, qualified frontier {}, ledger {:?}",
        w.core.phase, w.core.latch, w.core.u, w.core.c, w.core.x, w.table, w.core.frontier, w.rows
    );
    text
}

/// Explores one configuration (sections 7 and 8).
pub(super) fn explore(config: &Config) -> Report {
    let start = Instant::now();
    let all = actors(config);
    let mut visited = Visited::new();
    let mut parent: Vec<(u32, u64)> = Vec::new();
    let mut nonfault_from: Vec<u64> = vec![0];
    let mut nonfault: Vec<u32> = Vec::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let (mut transitions, mut fault_edges, mut no_step) = (0u64, 0u64, 0u64);
    let mut failure: Option<(&'static str, String, Vec<u64>)> = None;
    let mut incomplete: Option<String> = None;
    let init = encode(config, &initial(config));
    visited.insert(&init);
    parent.push((EMPTY, 0));
    let path_to = |parent: &[(u32, u64)], mut id: u32| {
        let mut edges = Vec::new();
        while parent[id as usize].0 != EMPTY {
            edges.push(parent[id as usize].1);
            id = parent[id as usize].0;
        }
        edges.reverse();
        edges
    };
    let mut id = 0u32;
    while (id as usize) < visited.len() {
        if visited.len() as u64 > config.cap {
            incomplete = Some(format!("the state cap of {} was reached", config.cap));
            break;
        }
        if start.elapsed() > config.time {
            incomplete = Some(format!("the time cap of {:?} was reached", config.time));
            break;
        }
        let key = visited.key(id).to_vec();
        let (next, state_violations) = successors(config, &all, &key);
        if let Some((property, detail)) = state_violations.into_iter().next() {
            failure = Some((property, detail, path_to(&parent, id)));
            break;
        }
        if next.is_empty() {
            no_step += 1;
            if !terminal(config, &decode(config, &key)) {
                failure = Some((
                    "D1",
                    "a state with no enabled step is not terminal".into(),
                    path_to(&parent, id),
                ));
                break;
            }
        }
        for successor in next {
            transitions += 1;
            taken.extend(successor.branches);
            if let Some((property, detail)) = successor.violations.into_iter().next() {
                let mut edges = path_to(&parent, id);
                edges.push(successor.edge);
                failure = Some((property, detail, edges));
                break;
            }
            let (to, new) = visited.insert(&successor.key);
            if new {
                parent.push((id, successor.edge));
            }
            if successor.fault {
                fault_edges += 1;
            } else {
                nonfault.push(to);
            }
        }
        if failure.is_some() {
            break;
        }
        nonfault_from.push(nonfault.len() as u64);
        id += 1;
    }
    // L1: the graph of steps that are not faults has no cycle. Its end
    // states are the states with no enabled step, each checked by D1.
    let mut l1 = "not claimed for this configuration".to_string();
    if failure.is_none() && incomplete.is_none() && config.l1 {
        let n = visited.len();
        let mut indegree = vec![0u32; n];
        for &to in &nonfault {
            indegree[to as usize] += 1;
        }
        let mut queue: Vec<u32> = (0..n as u32)
            .filter(|&s| indegree[s as usize] == 0)
            .collect();
        let mut removed = 0usize;
        while let Some(s) = queue.pop() {
            removed += 1;
            let (from, to) = (
                nonfault_from[s as usize] as usize,
                nonfault_from[s as usize + 1] as usize,
            );
            for &t in &nonfault[from..to] {
                indegree[t as usize] -= 1;
                if indegree[t as usize] == 0 {
                    queue.push(t);
                }
            }
        }
        if removed == n {
            l1 = format!(
                "HOLDS: {} non-fault edges, no cycle; {no_step} end states, each terminal (D1)",
                nonfault.len()
            );
        } else {
            let cyclic = (0..n)
                .find(|&s| indegree[s] > 0)
                .expect("a state on a cycle") as u32;
            failure = Some((
                "L1",
                format!(
                    "{} states lie on or behind a cycle of non-fault steps; one is reached by",
                    n - removed
                ),
                path_to(&parent, cyclic),
            ));
        }
    }
    let elapsed = start.elapsed();
    let verdict = if failure.is_some() {
        Verdict::Fail
    } else if incomplete.is_some() {
        Verdict::Incomplete
    } else {
        Verdict::Pass
    };
    let mut failed = BTreeSet::new();
    let mut text = String::new();
    let _ = writeln!(text, "EXPLORATION {}", config.name);
    let _ = writeln!(
        text,
        "  outcome: {}",
        match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Incomplete => "INCOMPLETE",
        }
    );
    let _ = writeln!(
        text,
        "  bounds: facts {} conflicting {:?} ENV {} batch {} faults {} admissions {} gating callers {} \
         gating conflict {} acknowledgement actors {} attempts each {} malformed snapshots {} cell constraint {} \
         worker verbs unwind {} DROP {} REPLAY {} A4 {}; ticket limit {TICKET_LIMIT}, refusal limit \
         {REFUSAL_LIMIT}; settlement and the worker on; caps {} states, {:?}",
        config.facts,
        config.conflict,
        config.env,
        config.batch,
        config.faults,
        config.admissions,
        config.gating,
        config.gating_conflict,
        config.acks,
        config.ack_attempts,
        config.malformed,
        config.cell_constraint,
        config.worker_unwinds,
        config.drop,
        config.replays,
        config.blanket_uncertain,
        config.cap,
        config.time
    );
    let _ = writeln!(text, "  actors: {all:?}");
    let _ = writeln!(
        text,
        "  states {} (expanded {}), transitions {transitions} (with a fault {fault_edges}), \
         states with no enabled step {no_step}",
        visited.len(),
        id
    );
    let _ = writeln!(
        text,
        "  elapsed {:.1} s; explorer tables {} MiB",
        elapsed.as_secs_f64(),
        visited.bytes() >> 20
    );
    if let Some(reason) = &incomplete {
        let _ = writeln!(
            text,
            "  INCOMPLETE: {reason}. No property is established by this run."
        );
    }
    match &failure {
        Some((property, detail, edges)) => {
            failed.insert(*property);
            let _ = writeln!(text, "  FAIL: {property}: {detail}");
            let _ = writeln!(text, "  shortest trace ({} steps):", edges.len());
            text.push_str(&trace(config, edges));
        }
        None => {
            for property in PROPERTIES {
                let verdict = if incomplete.is_some() {
                    "NOT ESTABLISHED (incomplete)".to_string()
                } else if property == "L1" {
                    l1.clone()
                } else if property == "E10" && !config.worker_unwinds {
                    "not applicable here (worker verbs do not unwind)".to_string()
                } else {
                    "HOLDS".to_string()
                };
                let _ = writeln!(text, "  {property}: {verdict}");
            }
        }
    }
    let _ = writeln!(text, "  branches:");
    for label in BRANCHES {
        let mark = if taken.contains(*label) {
            "taken    "
        } else {
            "NOT TAKEN"
        };
        let _ = writeln!(text, "    [{mark}] {label}");
    }
    for label in taken
        .iter()
        .filter(|label| !BRANCHES.contains(&label.as_str()))
    {
        let _ = writeln!(text, "    [taken    ] {label} (not in the listed set)");
    }
    let _ = writeln!(text, "  state key fields: {KEY_FIELDS}");
    Report {
        verdict,
        text,
        failed,
    }
}

/// The five default explorations run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn default_exploration(config: &Config) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let report = explore(config);
    println!("{}", report.text);
    assert_eq!(
        report.verdict,
        Verdict::Pass,
        "failed: {:?}\n{}",
        report.failed,
        report.text
    );
}

fn full_exploration(config: &Config) {
    let report = explore(config);
    println!("{}", report.text);
    assert_eq!(
        report.verdict,
        Verdict::Pass,
        "failed: {:?}\n{}",
        report.failed,
        report.text
    );
}

#[test]
fn x1_two_facts_two_writers_one_admission() {
    default_exploration(&X1);
}

#[test]
fn x2_one_fact_two_gating_callers() {
    default_exploration(&X2);
}

#[test]
fn x3_one_conflicting_fact_with_env() {
    default_exploration(&X3);
}

#[test]
fn x4_one_conflicting_fact_one_acknowledgement() {
    default_exploration(&X4);
}

#[test]
fn x5_one_clean_batch_of_two() {
    default_exploration(&X5);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f1() {
    full_exploration(&Config {
        cap: 60_000_000,
        ..F1
    });
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f2() {
    full_exploration(&F2);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f3() {
    full_exploration(&F3);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f4() {
    full_exploration(&F4);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f5() {
    full_exploration(&F5);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f6() {
    full_exploration(&F6);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f7() {
    full_exploration(&F7);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_f8() {
    full_exploration(&F8);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fpt() {
    full_exploration(&FPT);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fb() {
    full_exploration(&FB);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fb2() {
    full_exploration(&FB2);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fg() {
    full_exploration(&FG);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fw() {
    full_exploration(&FW);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fd() {
    full_exploration(&FD);
}

#[test]
#[ignore = "full reviewed set: run explicitly, optimised, on the exact final commit (section 8.3)"]
fn full_fr() {
    full_exploration(&FR);
}

/// The explorer's own machinery: the packing of an edge, the oracle's
/// enumeration and the key's round trip into an independent world.
#[test]
fn the_explorer_machinery_is_sound() {
    let edge = pack(5, &[0, 1, 2, 3, 4, 5, 0, 1]);
    assert_eq!(unpack(edge), (5, vec![0, 1, 2, 3, 4, 5, 0, 1]));
    let mut seen = BTreeSet::new();
    let mut prefix = Vec::new();
    loop {
        let mut oracle = Oracle::new(prefix, 1);
        let a = oracle.choose(&[false, true, true]);
        let b = oracle.branch(2);
        assert!(seen.insert((a, b)), "a sequence visited twice");
        match oracle.next() {
            Some(next) => prefix = next,
            None => break,
        }
    }
    assert_eq!(seen.len(), 6, "every sequence visited once");
    let mut oracle = Oracle::new(Vec::new(), 0);
    assert_eq!(oracle.choose(&[false, true]), 0);
    assert!(
        oracle.next().is_none(),
        "no fault is offered without budget"
    );
    let w = initial(&F8);
    let key = encode(&F8, &w);
    let copy = decode(&F8, &key);
    assert_eq!(encode(&F8, &copy), key);
    assert!(!Arc::ptr_eq(&w.core.shared, &copy.core.shared));
    assert!(Arc::ptr_eq(&copy.core.shared, &copy.health.shared));
    assert_eq!(word_of(&w.core.shared), word_of(&copy.core.shared));
}

// ---- the simulated storage, and the key E8 compares ---------------------------------------------

/// One simulated outcome of a store operation: its verbs with their
/// answers, and the rows the ledger gained.
type Outcome = (Vec<(Verb, Answer)>, Vec<SimRow>);

/// Every simulated outcome of one fact append of `members`, from the
/// initial world of `config`, within its fault budget.
fn append_outcomes(config: &Config, members: &[u8]) -> Vec<Outcome> {
    let batch: Vec<FactRecord> = members
        .iter()
        .map(|&f| FactRecord::new(fact_member(f)))
        .collect();
    let mut outcomes = Vec::new();
    let mut prefix = Vec::new();
    loop {
        let mut w = initial(config);
        let before = w.rows.len();
        let mut oracle = Oracle::new(prefix, config.faults);
        let mut ticket = w.health.arm().expect("a ticket below the limit");
        let mut io = Io::new(&mut w.rows, &mut oracle, true, true);
        let core = &mut w.core;
        let _ = run_op(|| core.append_facts(&mut ticket, &batch, &mut io));
        let log = std::mem::take(&mut io.log);
        drop(io);
        // Not a step: settled, so that its drop touches no word.
        ticket.state = TicketState::Settled;
        outcomes.push((log, w.rows[before..].to_vec()));
        match oracle.next() {
            Some(next) => prefix = next,
            None => break,
        }
    }
    outcomes
}

/// One expectation: after verbs that answered exactly so, the rows gained
/// are, as a set of histories, exactly these.
type Case<'a> = (Vec<(Verb, Answer)>, &'a [&'a [SimRow]]);

/// Checks every case, printing what was explored for each, and fails
/// naming every case that differs.
fn assert_histories(outcomes: &[Outcome], cases: &[Case]) {
    let mut wrong = Vec::new();
    for (log, expected) in cases {
        let mut got: Vec<Vec<SimRow>> = Vec::new();
        for (taken, rows) in outcomes {
            if taken == log && !got.contains(rows) {
                got.push(rows.clone());
            }
        }
        let same = got.len() == expected.len()
            && expected
                .iter()
                .all(|rows| got.iter().any(|g| g.as_slice() == *rows));
        println!(
            "{} {}: explored {got:?}; expected {expected:?}",
            if same { "same " } else { "DIFFERS" },
            log_label(log)
        );
        if !same {
            wrong.push(log_label(log));
        }
    }
    assert!(wrong.is_empty(), "the histories differ after: {wrong:?}");
}

fn ok(verb: Verb) -> (Verb, Answer) {
    (verb, Answer::Ok)
}

/// An insert that answers `Failed` was issued (section 4.1: any failure but
/// a constraint). With the rollback `Unknown`, or unwinding, the records are
/// present or absent and both are explored (section 7.1); with the rollback
/// `Undone`, nothing was written (A-1). One member.
#[test]
fn a_failed_insert_may_have_written_unless_the_rollback_undid_it() {
    let after = |tail: &[(Verb, Answer)]| [&[ok(Verb::Open), ok(Verb::Facts)][..], tail].concat();
    let failed = (Verb::Insert, Answer::Failed);
    let f1: &[SimRow] = &[SimRow::Fact(0)];
    assert_histories(
        &append_outcomes(&X1, &[0]),
        &[
            (after(&[failed, ok(Verb::Rollback)]), &[&[]]),
            (
                after(&[failed, (Verb::Rollback, Answer::Unknown)]),
                &[&[], f1],
            ),
            (
                after(&[failed, (Verb::Rollback, Answer::Unwound)]),
                &[&[], f1],
            ),
        ],
    );
}

/// The same, for a batch of two. When the second insert fails, the first
/// member alone may also be what the transaction holds.
#[test]
fn a_failed_insert_in_a_batch_may_have_written_unless_the_rollback_undid_it() {
    let read = [ok(Verb::Open), ok(Verb::Facts), ok(Verb::Facts)];
    let after = |tail: &[(Verb, Answer)]| [&read[..], tail].concat();
    let failed = (Verb::Insert, Answer::Failed);
    let (f1, both): (&[SimRow], &[SimRow]) =
        (&[SimRow::Fact(0)], &[SimRow::Fact(0), SimRow::Fact(1)]);
    let (unknown, unwound) = (
        (Verb::Rollback, Answer::Unknown),
        (Verb::Rollback, Answer::Unwound),
    );
    let inserted = ok(Verb::Insert);
    assert_histories(
        &append_outcomes(&X5, &[0, 1]),
        &[
            (after(&[failed, ok(Verb::Rollback)]), &[&[]]),
            (after(&[failed, unknown]), &[&[], f1]),
            (after(&[failed, unwound]), &[&[], f1]),
            (after(&[inserted, failed, ok(Verb::Rollback)]), &[&[]]),
            (after(&[inserted, failed, unknown]), &[&[], f1, both]),
            (after(&[inserted, failed, unwound]), &[&[], f1, both]),
        ],
    );
}

/// An insert that unwinds was issued, and whether the transaction holds its
/// row is not known, as after `Failed`: the records are present or absent
/// and both are explored (section 7.1). When the second insert of a batch
/// unwinds, the first member alone may also be what the transaction holds.
/// A commit that unwinds after every insert succeeded leaves all or none.
#[test]
fn an_unwinding_insert_in_a_batch_may_have_written_its_row_or_not() {
    let read = [ok(Verb::Open), ok(Verb::Facts), ok(Verb::Facts)];
    let after = |tail: &[(Verb, Answer)]| [&read[..], tail].concat();
    let (f1, both): (&[SimRow], &[SimRow]) =
        (&[SimRow::Fact(0)], &[SimRow::Fact(0), SimRow::Fact(1)]);
    let (inserted, unwound) = (ok(Verb::Insert), (Verb::Insert, Answer::Unwound));
    assert_histories(
        &append_outcomes(&X5, &[0, 1]),
        &[
            (after(&[unwound]), &[&[], f1]),
            (after(&[inserted, unwound]), &[&[], f1, both]),
            (
                after(&[inserted, inserted, (Verb::Commit, Answer::Unwound)]),
                &[&[], both],
            ),
        ],
    );
}

/// The other answers keep their meaning. A durable commit wrote the records
/// (A-2). A commit answering `Unknown` leaves them present or absent,
/// whatever the rollback then says; so does an unwinding insert. A
/// constraint failure rejected its own row: the rollback decides only for
/// the rows inserted before it, and `Undone` leaves nothing (A-1).
#[test]
fn the_other_storage_answers_keep_their_meaning() {
    let (f1, both): (&[SimRow], &[SimRow]) =
        (&[SimRow::Fact(0)], &[SimRow::Fact(0), SimRow::Fact(1)]);
    let after = |tail: &[(Verb, Answer)]| [&[ok(Verb::Open), ok(Verb::Facts)][..], tail].concat();
    let (inserted, unknown) = (ok(Verb::Insert), (Verb::Commit, Answer::Unknown));
    let constraint = (Verb::Insert, Answer::Constraint);
    assert_histories(
        &append_outcomes(&X1, &[0]),
        &[
            (after(&[inserted, ok(Verb::Commit)]), &[f1]),
            (after(&[inserted, unknown, ok(Verb::Rollback)]), &[&[], f1]),
            (
                after(&[inserted, unknown, (Verb::Rollback, Answer::Unknown)]),
                &[&[], f1],
            ),
            (after(&[(Verb::Insert, Answer::Unwound)]), &[&[], f1]),
            (after(&[constraint, ok(Verb::Rollback)]), &[&[]]),
            (
                after(&[constraint, (Verb::Rollback, Answer::Unknown)]),
                &[&[]],
            ),
            (
                after(&[constraint, (Verb::Rollback, Answer::Unwound)]),
                &[&[]],
            ),
        ],
    );
    let read = [ok(Verb::Open), ok(Verb::Facts), ok(Verb::Facts), inserted];
    let after = |tail: &[(Verb, Answer)]| [&read[..], tail].concat();
    assert_histories(
        &append_outcomes(&X5, &[0, 1]),
        &[
            (after(&[constraint, ok(Verb::Rollback)]), &[&[]]),
            (
                after(&[constraint, (Verb::Rollback, Answer::Unknown)]),
                &[&[], f1],
            ),
            (
                after(&[constraint, (Verb::Rollback, Answer::Unwound)]),
                &[&[], f1],
            ),
            (after(&[inserted, ok(Verb::Commit)]), &[both]),
        ],
    );
}

/// The real core on that history: a gating append whose insert answers
/// `Failed` and whose rollback is `Unknown` is Uncertain and keeps its
/// record in U. Where the record was in fact written, the barrier and
/// resolve place its correction in C; where it was not, resolve empties U.
/// Every state property, P8 among them, holds after each step.
#[test]
fn a_failed_gating_insert_that_was_written_is_corrected() {
    let config = Config { gating: 1, ..X1 };
    let wanted = [
        ok(Verb::Open),
        ok(Verb::Facts),
        (Verb::Insert, Answer::Failed),
        (Verb::Rollback, Answer::Unknown),
    ];
    let record = GatingRecord::new(gating_member(0));
    let check = |w: &World| {
        let mut obs = Obs::default();
        state_properties(&config, w, &mut obs);
        assert!(obs.violations.is_empty(), "{:?}", obs.violations);
    };
    let (mut written, mut absent) = (false, false);
    let mut prefix = Vec::new();
    loop {
        let mut w = initial(&config);
        let mut oracle = Oracle::new(prefix, config.faults);
        let mut io = Io::new(&mut w.rows, &mut oracle, true, true);
        let core = &mut w.core;
        let outcome = run_op(|| core.append_gating(&record, &mut io));
        let log = std::mem::take(&mut io.log);
        drop(io);
        if log == wanted {
            assert_eq!(
                outcome,
                Some(GatingOutcome::Refused {
                    persistence: Persistence::Uncertain,
                    reason: Reason::Storage,
                })
            );
            assert!(matches!(
                &w.core.u,
                Some((
                    _,
                    Occupant::Gating {
                        exit: Exit::ReturnedUncertain,
                        ..
                    }
                ))
            ));
            w.ghost.issued |= bit(0);
            check(&w);
            let in_ledger = w.rows.contains(&SimRow::Gating(0));
            let mut quiet = Oracle::new(Vec::new(), 0);
            let worker = w.wk.worker.as_mut().expect("the worker");
            let mut io = Io::new(&mut w.rows, &mut quiet, false, false);
            assert_eq!(w.core.barrier(worker, &mut io), BarrierOutcome::Passed);
            let token = w.core.agenda().u.expect("U is kept");
            let resolved = w.core.resolve(worker, token, &mut io);
            drop(io);
            if in_ledger {
                written = true;
                assert!(matches!(resolved, Resolve::Corrected(_)), "{resolved:?}");
                assert!(matches!(
                    &w.core.c,
                    Some((
                        _,
                        Occupant::Correction {
                            exit: Exit::ReturnedUncertain,
                            ..
                        }
                    ))
                ));
            } else {
                absent = true;
                assert!(matches!(resolved, Resolve::Absent), "{resolved:?}");
                assert!(w.core.c.is_none());
            }
            assert!(w.core.u.is_none());
            check(&w);
        }
        match oracle.next() {
            Some(next) => prefix = next,
            None => break,
        }
    }
    assert!(written, "the written history is not explored");
    assert!(absent, "the absent history is not explored");
}

const LIMITS: Limits = Limits {
    tickets: TICKET_LIMIT,
    refusals: REFUSAL_LIMIT,
};

fn duplicate_role(f: u8, seq: Seq) -> Cause {
    Cause {
        kind: CauseKind::DuplicateRole {
            part: Part {
                commitment: commitment(f),
                role: Role::Started,
            },
            holder: Holder {
                id: SimRow::Holder(f).key().id,
                seq,
            },
            offered: fact_key(f),
        },
    }
}

fn explanation_of(f: u8, seq: Seq) -> Occupant {
    Occupant::Explanation {
        original: fact_key(f),
        cause: duplicate_role(f, seq),
    }
}

/// Sets every field of `core` away from its initial value, through private
/// state. A forced state is not called reachable.
fn force_sample(core: &mut Core) {
    core.phase = Phase::Resolving;
    core.latch = Some(LatchCause::Overflow);
    core.u = Some((
        3,
        Occupant::Gating {
            record: GatingRecord::new(gating_member(0)),
            exit: Exit::ReturnedUncertain,
        },
    ));
    core.c = Some((
        2,
        Occupant::Correction {
            about: gating_key(1),
            seq: 7,
            exit: Exit::Unwound,
        },
    ));
    core.x = Some((4, explanation_of(0, 2), Stage::Committed(5)));
    core.occupancies = [3, 2, 4];
    core.refusals.insert(fact_key(1), duplicate_role(1, 2));
    core.frontier = 9;
}

fn sample_core(limits: Limits) -> Core {
    let (mut core, _health) = Core::new(limits);
    assert!(core.session_opened());
    assert!(core.take_worker().is_some());
    force_sample(&mut core);
    core
}

/// Two cores that differ only in the sequence X was committed with are
/// different cores, and E8's key tells them apart.
#[test]
fn e8_key_tells_committed_sequences_apart() {
    let (mut five, mut six) = (sample_core(LIMITS), sample_core(LIMITS));
    five.x = Some((4, explanation_of(0, 2), Stage::Committed(5)));
    six.x = Some((4, explanation_of(0, 2), Stage::Committed(6)));
    assert_ne!(core_key(&five), core_key(&six));
}

/// E8's key marks whether each cell is occupied. These two cores differ in
/// which of U and C is occupied; without the mark, their bytes are equal.
#[test]
fn e8_key_marks_which_cells_are_occupied() {
    let mut one = sample_core(LIMITS);
    one.u = None;
    one.c = Some((
        0,
        Occupant::Correction {
            about: RecordKey {
                id: bytes(0, 0, 2),
                digest: bytes(0, 1, 0),
            },
            seq: u64::from(exit_code(Exit::ReturnedUncertain)),
            exit: Exit::InCall,
        },
    ));
    one.x = None;
    let mut two = sample_core(LIMITS);
    two.u = Some((
        0,
        Occupant::Gating {
            record: GatingRecord::new(gating_member(0)),
            exit: Exit::ReturnedUncertain,
        },
    ));
    two.c = None;
    two.x = None;
    assert_ne!(core_key(&one), core_key(&two));
}

/// A field of the core, named, and a change to it.
type Change = (&'static str, fn(&mut Core));

/// A change to any field of the core changes E8's key, and the key holds
/// no address: equal cores of two ledgers have one key.
#[test]
fn e8_key_covers_every_field_of_the_core() {
    let base = core_key(&sample_core(LIMITS));
    assert_eq!(core_key(&sample_core(LIMITS)), base);
    let changes: Vec<Change> = vec![
        ("phase", |c| c.phase = Phase::Healthy),
        ("session", |c| c.session = Session::Sealed),
        ("latch cleared", |c| c.latch = None),
        ("latch cause", |c| c.latch = Some(LatchCause::Corrupt)),
        ("U emptied", |c| c.u = None),
        ("U occupancy number", |c| {
            if let Some((n, _)) = c.u.as_mut() {
                *n += 1;
            }
        }),
        ("U occupant", |c| {
            if let Some((_, Occupant::Gating { exit, .. })) = c.u.as_mut() {
                *exit = Exit::Unwound;
            }
        }),
        ("C emptied", |c| c.c = None),
        ("C occupancy number", |c| {
            if let Some((n, _)) = c.c.as_mut() {
                *n += 1;
            }
        }),
        ("C occupant", |c| {
            if let Some((_, Occupant::Correction { seq, .. })) = c.c.as_mut() {
                *seq += 1;
            }
        }),
        ("X emptied", |c| c.x = None),
        ("X occupancy number", |c| {
            if let Some((n, _, _)) = c.x.as_mut() {
                *n += 1;
            }
        }),
        ("X occupant", |c| {
            if let Some((_, occupant, _)) = c.x.as_mut() {
                *occupant = explanation_of(0, 3);
            }
        }),
        ("X stage", |c| {
            if let Some((_, _, stage)) = c.x.as_mut() {
                *stage = Stage::Built;
            }
        }),
        ("X committed sequence", |c| {
            if let Some((_, _, stage)) = c.x.as_mut() {
                *stage = Stage::Committed(6);
            }
        }),
        ("U occupancy counter", |c| c.occupancies[0] += 1),
        ("C occupancy counter", |c| c.occupancies[1] += 1),
        ("X occupancy counter", |c| c.occupancies[2] += 1),
        ("refusal entry added", |c| {
            c.refusals.insert(fact_key(0), duplicate_role(0, 2));
        }),
        ("refusal entry removed", |c| c.refusals.clear()),
        ("retained cause", |c| {
            c.refusals.insert(fact_key(1), duplicate_role(1, 3));
        }),
        ("qualified frontier", |c| c.frontier += 1),
        ("worker taken", |c| c.worker_taken = false),
        ("refusal limit", |c| c.refusal_limit += 1),
    ];
    let mut unseen: Vec<&str> = changes
        .into_iter()
        .filter(|(_, change)| {
            let mut core = sample_core(LIMITS);
            change(&mut core);
            core_key(&core) == base
        })
        .map(|(field, _)| field)
        .collect();
    let tickets = Limits {
        tickets: TICKET_LIMIT + 1,
        ..LIMITS
    };
    if core_key(&sample_core(tickets)) == base {
        unseen.push("ticket limit");
    }
    assert!(
        unseen.is_empty(),
        "changes E8's key does not see: {unseen:?}"
    );
}

/// E8 on the unchanged core: each worker operation given a token of an old
/// occupancy answers stale, and leaves the core, as its key sees it, and
/// the word exactly as they were. The forced state's flags are published
/// first, by a seal the core refuses.
#[test]
fn e8_holds_for_stale_worker_operations() {
    let config = FR;
    let mut w = initial(&config);
    force_sample(&mut w.core);
    assert!(!w.core.session_sealed());
    let old = |cell, occupancy| CellToken {
        ledger: Arc::clone(&w.core.shared),
        cell,
        occupancy,
    };
    let actions = [
        Action::Resolve(old(Cell::U, 2)),
        Action::AppendC(old(Cell::C, 1)),
        Action::AppendX(old(Cell::X, 3)),
        Action::EmptyX(old(Cell::X, 3), 5),
    ];
    for action in actions {
        let mut oracle = Oracle::new(Vec::new(), config.faults);
        let mut obs = Obs::default();
        worker_op(&config, &mut w, &action, &mut oracle, &mut obs);
        assert!(
            obs.violations.is_empty(),
            "{action:?}: {:?}",
            obs.violations
        );
        assert!(
            obs.branches.iter().all(|label| label.ends_with("Stale")),
            "{action:?}: {:?}",
            obs.branches
        );
    }
}

// ---- experimental: the admission observer quotient ----------------------------------------------
//
// P3-C11-DR01-D2A1-ADM-QUOTIENT-01, a bounded experiment. It is not the
// contract's explorer and not evidence for the contract; `explore` above is
// unchanged and stays the reference.
//
// ADM changes only its own counter, and nothing reads that counter but
// `terminal` (`adm == 0`). So a state reachable with admissions spent is
// reachable with none spent, and the admission step from a state depends on
// that state alone. The quotient keeps the counter at its configured value,
// never schedules ADM, runs ADM's one step at every expanded state as an
// observer and discards its successor, and judges D1 with the admissions
// spent. Its preconditions are asserted at every state: a run that breaks
// one aborts and gives no verdict. A2, whose admission keeps bits in the
// world between two steps, breaks them and needs the original explorer.

/// An admission step: `step_adm`, or a deliberately defective one in a
/// negative case.
type AdmStep = fn(&Config, &mut World, &mut Obs) -> bool;

type Violations = Vec<(&'static str, String)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Every actor is scheduled, ADM included: the original semantics.
    Full,
    /// ADM is evaluated at every state as an observer and never scheduled.
    Quotient,
}

/// One item of a state's expansion.
enum Next {
    /// A successor, as `successors` yields it.
    Edge(Successor),
    /// Under the quotient: the admission step from this state. Its
    /// successor is not a state of the quotient.
    Admission(Successor),
}

/// One step of one actor, with ADM played by `adm`.
fn step_with(
    config: &Config,
    w: &mut World,
    actor: Actor,
    o: &mut Oracle,
    obs: &mut Obs,
    adm: AdmStep,
) -> bool {
    if actor != Actor::Adm {
        return step(config, w, actor, o, obs);
    }
    if let Some(trace) = obs.trace.as_mut() {
        let _ = write!(trace, "{actor:?}: ");
    }
    adm(config, w, obs)
}

/// The state of `key` with `adm` admissions left.
fn with_admissions(config: &Config, key: &[u8], adm: u8) -> Vec<u8> {
    let mut w = decode(config, key);
    w.adm = adm;
    encode(config, &w)
}

/// Under the quotient, the admission step from `key`, run as `successors`
/// runs a step, with the reduction's preconditions asserted: ADM is enabled
/// exactly when admissions are configured, it spends one and changes nothing
/// else, and it keeps no bits (A2).
fn observe_admission(
    config: &Config,
    base: &World,
    key: &[u8],
    index: usize,
    adm: AdmStep,
) -> Option<Successor> {
    let mut w = decode(config, key);
    let mut obs = Obs::default();
    hook::take_counts();
    let enabled = adm(config, &mut w, &mut obs);
    let counts = hook::take_counts();
    assert_eq!(
        enabled,
        config.admissions > 0,
        "quotient inapplicable: ADM is not enabled exactly when admissions are configured"
    );
    if !enabled {
        return None;
    }
    transition_properties(base, &w, &mut obs, counts);
    let successor = encode(config, &w);
    assert_eq!(
        w.adm.checked_add(1),
        Some(config.admissions),
        "quotient inapplicable: ADM did not spend exactly one admission"
    );
    assert!(
        w.adm_kept.is_none(),
        "quotient inapplicable: ADM kept bits in the world (A2)"
    );
    w.adm = config.admissions;
    assert!(
        encode(config, &w) == key,
        "quotient inapplicable: ADM changed the world beyond its counter"
    );
    Some(Successor {
        key: successor,
        fault: false,
        edge: pack(index, &[]),
        violations: obs.violations,
        branches: obs.branches,
    })
}

/// `successors`, with ADM played by `adm`; under the quotient, ADM is an
/// observation and not an edge, and every state must hold its configured
/// admissions and no A2 bits.
fn expand(
    config: &Config,
    all: &[Actor],
    key: &[u8],
    adm: AdmStep,
    mode: Mode,
) -> (Vec<Next>, Violations) {
    let base = decode(config, key);
    if mode == Mode::Quotient {
        assert!(
            base.adm == config.admissions && base.adm_kept.is_none(),
            "quotient inapplicable: a state with admissions spent or bits kept"
        );
    }
    let mut state = Obs::default();
    state_properties(config, &base, &mut state);
    if encode(config, &base) != key {
        state.fail("KEY", "a key does not survive decoding");
    }
    let mut out = Vec::new();
    for (index, &actor) in all.iter().enumerate() {
        if mode == Mode::Quotient && actor == Actor::Adm {
            out.extend(observe_admission(config, &base, key, index, adm).map(Next::Admission));
            continue;
        }
        let mut prefix = Vec::new();
        loop {
            let mut w = decode(config, key);
            let mut oracle = Oracle::new(prefix, w.faults);
            let mut obs = Obs::default();
            hook::take_counts();
            let enabled = step_with(config, &mut w, actor, &mut oracle, &mut obs, adm);
            let counts = hook::take_counts();
            if !enabled {
                break;
            }
            w.faults = oracle.faults;
            transition_properties(&base, &w, &mut obs, counts);
            out.push(Next::Edge(Successor {
                key: encode(config, &w),
                fault: oracle.fault,
                edge: pack(index, &oracle.sequence()),
                violations: obs.violations,
                branches: obs.branches,
            }));
            match oracle.next() {
                Some(next) => prefix = next,
                None => break,
            }
        }
    }
    (out, state.violations)
}

/// `trace`, with ADM played by `adm`.
fn trace_with(config: &Config, edges: &[u64], adm: AdmStep) -> String {
    let all = actors(config);
    let mut key = encode(config, &initial(config));
    let mut text = String::new();
    for (n, &edge) in edges.iter().enumerate() {
        let (actor, choices) = unpack(edge);
        let mut w = decode(config, &key);
        let mut oracle = Oracle::new(choices, w.faults);
        let mut obs = Obs {
            trace: Some(String::new()),
            ..Obs::default()
        };
        step_with(config, &mut w, all[actor], &mut oracle, &mut obs, adm);
        w.faults = oracle.faults;
        let _ = writeln!(
            text,
            "    {:>3}. {}{}",
            n + 1,
            obs.trace.unwrap_or_default(),
            if oracle.fault { "   [fault]" } else { "" }
        );
        key = encode(config, &w);
    }
    let w = decode(config, &key);
    let _ = writeln!(
        text,
        "    end: phase {:?}, latch {:?}, U {:?}, C {:?}, X {:?}, table {:?}, qualified frontier {}, ledger {:?}",
        w.core.phase, w.core.latch, w.core.u, w.core.c, w.core.x, w.table, w.core.frontier, w.rows
    );
    text
}

/// What an experimental run counted. Under the quotient, states and
/// transitions are the quotient's, not the original graph's.
#[derive(Debug, Default)]
struct Tally {
    states: u64,
    expanded: u64,
    transitions: u64,
    fault_edges: u64,
    nonfault_edges: u64,
    no_step: u64,
    evaluations: u64,
    admitted: u64,
    taken: BTreeSet<String>,
    trace: Option<Vec<u64>>,
}

/// `explore`, with ADM played by `adm`, in either mode. In `Mode::Full`
/// with `step_adm` it is the original explorer, and the differential tests
/// hold its report to `explore`'s.
fn explore_experimental(config: &Config, adm: AdmStep, mode: Mode) -> (Report, Tally) {
    let start = Instant::now();
    let all = actors(config);
    let adm_index = all
        .iter()
        .position(|&actor| actor == Actor::Adm)
        .expect("ADM is an actor");
    let mut visited = Visited::new();
    let mut parent: Vec<(u32, u64)> = Vec::new();
    let mut nonfault_from: Vec<u64> = vec![0];
    let mut nonfault: Vec<u32> = Vec::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let (mut transitions, mut fault_edges, mut no_step) = (0u64, 0u64, 0u64);
    let (mut evaluations, mut admitted) = (0u64, 0u64);
    let mut failure: Option<(&'static str, String, Vec<u64>)> = None;
    let mut incomplete: Option<String> = None;
    let init = encode(config, &initial(config));
    visited.insert(&init);
    parent.push((EMPTY, 0));
    let path_to = |parent: &[(u32, u64)], mut id: u32| {
        let mut edges = Vec::new();
        while parent[id as usize].0 != EMPTY {
            edges.push(parent[id as usize].1);
            id = parent[id as usize].0;
        }
        edges.reverse();
        edges
    };
    let mut id = 0u32;
    while (id as usize) < visited.len() {
        if visited.len() as u64 > config.cap {
            incomplete = Some(format!("the state cap of {} was reached", config.cap));
            break;
        }
        if start.elapsed() > config.time {
            incomplete = Some(format!("the time cap of {:?} was reached", config.time));
            break;
        }
        let key = visited.key(id).to_vec();
        let (next, state_violations) = expand(config, &all, &key, adm, mode);
        if let Some((property, detail)) = state_violations.into_iter().next() {
            failure = Some((property, detail, path_to(&parent, id)));
            break;
        }
        if !next.iter().any(|item| matches!(item, Next::Edge(_))) {
            no_step += 1;
            // Under the quotient the state still holds its admissions: the
            // state with no enabled step is this one with them spent.
            let mut end = decode(config, &key);
            if mode == Mode::Quotient {
                end.adm = 0;
            }
            if !terminal(config, &end) {
                let mut edges = path_to(&parent, id);
                if mode == Mode::Quotient {
                    let spend = usize::from(config.admissions);
                    edges.extend(std::iter::repeat_n(pack(adm_index, &[]), spend));
                }
                failure = Some((
                    "D1",
                    "a state with no enabled step is not terminal".into(),
                    edges,
                ));
                break;
            }
        }
        for item in next {
            let successor = match item {
                Next::Edge(successor) => successor,
                Next::Admission(observed) => {
                    evaluations += 1;
                    if observed
                        .branches
                        .iter()
                        .any(|label| label == "admission: admitted")
                    {
                        admitted += 1;
                    }
                    taken.extend(observed.branches);
                    if let Some((property, detail)) = observed.violations.into_iter().next() {
                        let mut edges = path_to(&parent, id);
                        edges.push(observed.edge);
                        failure = Some((property, detail, edges));
                        break;
                    }
                    continue;
                }
            };
            transitions += 1;
            taken.extend(successor.branches);
            if let Some((property, detail)) = successor.violations.into_iter().next() {
                let mut edges = path_to(&parent, id);
                edges.push(successor.edge);
                failure = Some((property, detail, edges));
                break;
            }
            let (to, new) = visited.insert(&successor.key);
            if new {
                parent.push((id, successor.edge));
            }
            if successor.fault {
                fault_edges += 1;
            } else {
                nonfault.push(to);
            }
        }
        if failure.is_some() {
            break;
        }
        nonfault_from.push(nonfault.len() as u64);
        id += 1;
    }
    let mut l1 = "not claimed for this configuration".to_string();
    if failure.is_none() && incomplete.is_none() && config.l1 {
        let n = visited.len();
        let mut indegree = vec![0u32; n];
        for &to in &nonfault {
            indegree[to as usize] += 1;
        }
        let mut queue: Vec<u32> = (0..n as u32)
            .filter(|&s| indegree[s as usize] == 0)
            .collect();
        let mut removed = 0usize;
        while let Some(s) = queue.pop() {
            removed += 1;
            let (from, to) = (
                nonfault_from[s as usize] as usize,
                nonfault_from[s as usize + 1] as usize,
            );
            for &t in &nonfault[from..to] {
                indegree[t as usize] -= 1;
                if indegree[t as usize] == 0 {
                    queue.push(t);
                }
            }
        }
        if removed == n {
            l1 = format!(
                "HOLDS: {} non-fault edges, no cycle; {no_step} end states, each terminal (D1)",
                nonfault.len()
            );
        } else {
            let cyclic = (0..n)
                .find(|&s| indegree[s] > 0)
                .expect("a state on a cycle") as u32;
            failure = Some((
                "L1",
                format!(
                    "{} states lie on or behind a cycle of non-fault steps; one is reached by",
                    n - removed
                ),
                path_to(&parent, cyclic),
            ));
        }
    }
    let elapsed = start.elapsed();
    let verdict = if failure.is_some() {
        Verdict::Fail
    } else if incomplete.is_some() {
        Verdict::Incomplete
    } else {
        Verdict::Pass
    };
    let mut failed = BTreeSet::new();
    let mut text = String::new();
    match mode {
        Mode::Full => {
            let _ = writeln!(text, "EXPLORATION {}", config.name);
        }
        Mode::Quotient => {
            let _ = writeln!(
                text,
                "EXPLORATION {} (admission observer quotient: experimental, not the contract's explorer)",
                config.name
            );
        }
    }
    let _ = writeln!(
        text,
        "  outcome: {}",
        match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Incomplete => "INCOMPLETE",
        }
    );
    let _ = writeln!(
        text,
        "  bounds: facts {} conflicting {:?} ENV {} batch {} faults {} admissions {} gating callers {} \
         gating conflict {} acknowledgement actors {} attempts each {} malformed snapshots {} cell constraint {} \
         worker verbs unwind {} DROP {} REPLAY {} A4 {}; ticket limit {TICKET_LIMIT}, refusal limit \
         {REFUSAL_LIMIT}; settlement and the worker on; caps {} states, {:?}",
        config.facts,
        config.conflict,
        config.env,
        config.batch,
        config.faults,
        config.admissions,
        config.gating,
        config.gating_conflict,
        config.acks,
        config.ack_attempts,
        config.malformed,
        config.cell_constraint,
        config.worker_unwinds,
        config.drop,
        config.replays,
        config.blanket_uncertain,
        config.cap,
        config.time
    );
    let _ = writeln!(text, "  actors: {all:?}");
    if mode == Mode::Quotient {
        let _ = writeln!(
            text,
            "  quotient: ADM is not scheduled; its one step is evaluated at every expanded state ({evaluations} \
             evaluations, {admitted} admitted) and its successor discarded; every key holds {} admissions left; \
             D1 is judged with them spent. The states and transitions below are the quotient's, not the \
             original graph's, and the state cap counts quotient states.",
            config.admissions
        );
    }
    let _ = writeln!(
        text,
        "  states {} (expanded {}), transitions {transitions} (with a fault {fault_edges}), \
         states with no enabled step {no_step}",
        visited.len(),
        id
    );
    let _ = writeln!(
        text,
        "  elapsed {:.1} s; explorer tables {} MiB",
        elapsed.as_secs_f64(),
        visited.bytes() >> 20
    );
    if let Some(reason) = &incomplete {
        let _ = writeln!(
            text,
            "  INCOMPLETE: {reason}. No property is established by this run."
        );
    }
    let mut trace = None;
    match &failure {
        Some((property, detail, edges)) => {
            failed.insert(*property);
            let _ = writeln!(text, "  FAIL: {property}: {detail}");
            let _ = writeln!(text, "  shortest trace ({} steps):", edges.len());
            text.push_str(&trace_with(config, edges, adm));
            trace = Some(edges.clone());
        }
        None => {
            for property in PROPERTIES {
                let verdict = if incomplete.is_some() {
                    "NOT ESTABLISHED (incomplete)".to_string()
                } else if property == "L1" {
                    l1.clone()
                } else if property == "E10" && !config.worker_unwinds {
                    "not applicable here (worker verbs do not unwind)".to_string()
                } else {
                    "HOLDS".to_string()
                };
                let _ = writeln!(text, "  {property}: {verdict}");
            }
        }
    }
    let _ = writeln!(text, "  branches:");
    for label in BRANCHES {
        let mark = if taken.contains(*label) {
            "taken    "
        } else {
            "NOT TAKEN"
        };
        let _ = writeln!(text, "    [{mark}] {label}");
    }
    for label in taken
        .iter()
        .filter(|label| !BRANCHES.contains(&label.as_str()))
    {
        let _ = writeln!(text, "    [taken    ] {label} (not in the listed set)");
    }
    let _ = writeln!(text, "  state key fields: {KEY_FIELDS}");
    let tally = Tally {
        states: visited.len() as u64,
        expanded: u64::from(id),
        transitions,
        fault_edges,
        nonfault_edges: nonfault.len() as u64,
        no_step,
        evaluations,
        admitted,
        taken,
        trace,
    };
    (
        Report {
            verdict,
            text,
            failed,
        },
        tally,
    )
}

fn quotient_full_exploration(config: &Config) {
    let (report, _) = explore_experimental(config, step_adm, Mode::Quotient);
    println!("{}", report.text);
    assert_eq!(
        report.verdict,
        Verdict::Pass,
        "failed: {:?}\n{}",
        report.failed,
        report.text
    );
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f1() {
    quotient_full_exploration(&F1);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f2() {
    quotient_full_exploration(&F2);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f3() {
    quotient_full_exploration(&F3);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f4() {
    quotient_full_exploration(&F4);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f5() {
    quotient_full_exploration(&F5);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f6() {
    quotient_full_exploration(&F6);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f7() {
    quotient_full_exploration(&F7);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_f8() {
    quotient_full_exploration(&F8);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fpt() {
    quotient_full_exploration(&FPT);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fb() {
    quotient_full_exploration(&FB);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fb2() {
    quotient_full_exploration(&FB2);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fg() {
    quotient_full_exploration(&FG);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fw() {
    quotient_full_exploration(&FW);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fd() {
    quotient_full_exploration(&FD);
}

#[test]
#[ignore = "experimental admission observer quotient: run only when an Architect mission names it"]
fn quotient_full_fr() {
    quotient_full_exploration(&FR);
}

// ---- experimental: the quotient held to the original --------------------------------------------

/// One item of an expansion, compared whole.
#[derive(Debug, PartialEq, Eq)]
struct Item {
    admission: bool,
    edge: u64,
    fault: bool,
    key: Vec<u8>,
    violations: Violations,
    branches: Vec<String>,
}

fn items(next: Vec<Next>) -> Vec<Item> {
    next.into_iter()
        .map(|item| {
            let (admission, s) = match item {
                Next::Edge(s) => (false, s),
                Next::Admission(s) => (true, s),
            };
            Item {
                admission,
                edge: s.edge,
                fault: s.fault,
                key: s.key,
                violations: s.violations,
                branches: s.branches,
            }
        })
        .collect()
}

/// A whole graph, kept: every key reached through edges (not through
/// admission observations), and each key's expansion and own violations.
struct Graph {
    keys: Vec<Vec<u8>>,
    ids: BTreeMap<Vec<u8>, usize>,
    items: Vec<Vec<Item>>,
    state: Vec<Violations>,
}

fn whole_graph(config: &Config, expand_one: impl Fn(&[u8]) -> (Vec<Item>, Violations)) -> Graph {
    let init = encode(config, &initial(config));
    let mut graph = Graph {
        keys: vec![init.clone()],
        ids: BTreeMap::from([(init, 0)]),
        items: Vec::new(),
        state: Vec::new(),
    };
    let mut at = 0;
    while at < graph.keys.len() {
        assert!(
            graph.keys.len() as u64 <= DEFAULT_CAP,
            "a differential graph beyond the default cap"
        );
        let (items, state) = expand_one(&graph.keys[at]);
        for item in &items {
            if !item.admission && !graph.ids.contains_key(&item.key) {
                graph.ids.insert(item.key.clone(), graph.keys.len());
                graph.keys.push(item.key.clone());
            }
        }
        graph.items.push(items);
        graph.state.push(state);
        at += 1;
    }
    graph
}

/// The whole numbers among a line's words, after its trailing `,` `)` `;`.
fn numbers(text: &str, start: &str) -> Vec<u64> {
    let line = text
        .lines()
        .find(|line| line.starts_with(start))
        .unwrap_or_else(|| panic!("no line starts with {start:?}"));
    line.split_whitespace()
        .filter_map(|word| word.trim_end_matches([',', ')', ';']).parse().ok())
        .collect()
}

/// A report's lines from `from` up to, not including, `to`.
fn section<'a>(text: &'a str, from: &str, to: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|line| !line.starts_with(from))
        .take_while(|line| !line.starts_with(to))
        .collect()
}

/// Holds the quotient to the reduction on one configuration, with the real
/// admission step: the whole original graph against the whole quotient,
/// state by state and edge by edge, and `explore`'s report against both
/// experimental reports.
fn quotient_differential(config: &Config) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let all = actors(config);
    let n = config.admissions;
    let adm_index = all
        .iter()
        .position(|&actor| actor == Actor::Adm)
        .expect("ADM is an actor");
    let original = whole_graph(config, |key| {
        let (next, state) = successors(config, &all, key);
        (items(next.into_iter().map(Next::Edge).collect()), state)
    });
    let quotient = whole_graph(config, |key| {
        let (next, state) = expand(config, &all, key, step_adm, Mode::Quotient);
        (items(next), state)
    });

    // 0. The experimental expansion in its full mode is `successors`.
    for (id, key) in original.keys.iter().enumerate() {
        let (next, state) = expand(config, &all, key, step_adm, Mode::Full);
        assert!(
            items(next) == original.items[id] && state == original.state[id],
            "the full mode differs from `successors` at state {id}"
        );
    }

    // 1. The product: every original state projects to a quotient state,
    //    and every quotient state, with any number of admissions left, is an
    //    original state.
    for key in &original.keys {
        assert!(
            quotient.ids.contains_key(&with_admissions(config, key, n)),
            "an original state outside the quotient"
        );
    }
    for key in &quotient.keys {
        for a in 0..=n {
            assert!(
                original.ids.contains_key(&with_admissions(config, key, a)),
                "a quotient state with {a} admissions left is not an original state"
            );
        }
    }

    // 2-5. Each original state against its class: its own violations; its
    //      non-ADM edges, in order; its ADM edge against the observation; and
    //      D1.
    let (mut adm_edges, mut adm_admitted, mut dead) = (0u64, 0u64, 0u64);
    for (id, key) in original.keys.iter().enumerate() {
        let a = decode(config, key).adm;
        let q = quotient.ids[&with_admissions(config, key, n)];
        assert_eq!(
            original.state[id], quotient.state[q],
            "state violations differ"
        );
        let (adm_items, others): (Vec<&Item>, Vec<&Item>) = original.items[id]
            .iter()
            .partition(|item| unpack(item.edge).0 == adm_index);
        let (observed, edges): (Vec<&Item>, Vec<&Item>) =
            quotient.items[q].iter().partition(|item| item.admission);
        assert_eq!(others.len(), edges.len(), "non-ADM edges differ in number");
        for (g, e) in others.iter().zip(&edges) {
            assert!(
                g.edge == e.edge
                    && g.fault == e.fault
                    && g.violations == e.violations
                    && g.branches == e.branches,
                "a non-ADM edge differs"
            );
            assert_eq!(
                decode(config, &g.key).adm,
                a,
                "a non-ADM edge spent an admission"
            );
            assert_eq!(
                with_admissions(config, &g.key, n),
                e.key,
                "a non-ADM edge leads elsewhere"
            );
        }
        assert_eq!(observed.len(), usize::from(n > 0), "an observation missing");
        if a > 0 {
            let [g] = adm_items.as_slice() else {
                panic!("not exactly one ADM edge with {a} admissions left");
            };
            let e = observed[0];
            assert!(
                !g.fault
                    && g.edge == pack(adm_index, &[])
                    && g.key == with_admissions(config, key, a - 1),
                "the ADM edge is not a counter step"
            );
            assert!(
                e.edge == g.edge
                    && e.violations == g.violations
                    && e.branches == g.branches
                    && with_admissions(config, &e.key, a - 1) == g.key,
                "the observation differs from the ADM edge"
            );
            adm_edges += 1;
            if g.branches
                .iter()
                .any(|label| label == "admission: admitted")
            {
                adm_admitted += 1;
            }
        } else {
            assert!(adm_items.is_empty(), "an ADM edge with no admission left");
        }
        let no_step = original.items[id].is_empty();
        assert_eq!(no_step, a == 0 && edges.is_empty(), "D1's states differ");
        if no_step {
            dead += 1;
            let mut end = decode(config, &quotient.keys[q]);
            end.adm = 0;
            assert_eq!(
                terminal(config, &decode(config, key)),
                terminal(config, &end),
                "terminal differs"
            );
        }
    }

    // 6. The reports: `explore`, the full mode and the quotient.
    let report = explore(config);
    let (full, full_tally) = explore_experimental(config, step_adm, Mode::Full);
    let timeless = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| !line.starts_with("  elapsed "))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        timeless(&report.text),
        timeless(&full.text),
        "the full mode's report differs from `explore`'s"
    );
    let (projected, tally) = explore_experimental(config, step_adm, Mode::Quotient);
    println!("{}\n{}", report.text, projected.text);
    assert_eq!(report.verdict, Verdict::Pass);
    assert_eq!(projected.verdict, Verdict::Pass);
    assert_eq!(full_tally.taken, tally.taken, "the branches taken differ");
    let [states, expanded, transitions, faulted, no_step] = numbers(&report.text, "  states ")[..]
    else {
        panic!("the original's states line");
    };
    let [nonfault, ends] = numbers(&report.text, "  L1: ")[..] else {
        panic!("the original's L1 line");
    };
    let (k, s) = (u64::from(n) + 1, tally.states);
    assert_eq!(states, original.keys.len() as u64);
    assert_eq!(tally.states, quotient.keys.len() as u64);
    assert_eq!((expanded, tally.expanded), (states, s));
    assert_eq!(states, k * s);
    assert_eq!(transitions, k * tally.transitions + u64::from(n) * s);
    assert_eq!(faulted, k * tally.fault_edges);
    assert_eq!(nonfault, k * tally.nonfault_edges + u64::from(n) * s);
    assert_eq!(
        (no_step, ends, dead),
        (tally.no_step, tally.no_step, tally.no_step)
    );
    assert_eq!(tally.evaluations, if n > 0 { s } else { 0 });
    assert_eq!(adm_edges, u64::from(n) * tally.evaluations);
    assert_eq!(adm_admitted, u64::from(n) * tally.admitted);
    let lines = |text: &str| -> Vec<String> {
        section(text, "  P1:", "  state key fields")
            .into_iter()
            .filter(|line| !line.starts_with("  L1: "))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        lines(&report.text),
        lines(&projected.text),
        "properties or branches differ"
    );
    assert_eq!(
        numbers(&projected.text, "  L1: "),
        [tally.nonfault_edges, tally.no_step],
        "the quotient's L1 line"
    );
    assert_eq!(
        section(&report.text, "  state key fields", "\u{0}"),
        section(&projected.text, "  state key fields", "\u{0}")
    );
    if n == 0 {
        // No admission: the two reports differ only in the header, the
        // quotient's line and the time.
        let plain = |text: &str| -> Vec<String> {
            timeless(text)
                .into_iter()
                .skip(1)
                .filter(|line| !line.starts_with("  quotient: "))
                .collect()
        };
        assert_eq!(plain(&report.text), plain(&projected.text));
        assert_eq!(
            original.keys, quotient.keys,
            "with no admission the graphs are one"
        );
    }
    println!(
        "DIFFERENTIAL {}: admissions {n}. Original: {states} states, {transitions} transitions ({faulted} with \
         a fault, {nonfault} without), {adm_edges} ADM edges ({adm_admitted} admitting), {no_step} with no \
         enabled step. Quotient: {s} states, {} transitions ({} with a fault, {} without), {} admission \
         evaluations ({} admitting), {} with no enabled step. Matched: every original state to its class, \
         every non-ADM edge, every ADM edge to its observation, every state with no enabled step and its \
         terminal judgement, every property line, every branch, and (full mode) explore's whole report.",
        config.name,
        tally.transitions,
        tally.fault_edges,
        tally.nonfault_edges,
        tally.evaluations,
        tally.admitted,
        tally.no_step
    );
}

#[test]
fn quotient_x1_matches_the_original() {
    quotient_differential(&X1);
}

#[test]
fn quotient_x2_matches_the_original() {
    quotient_differential(&X2);
}

#[test]
fn quotient_x3_matches_the_original() {
    quotient_differential(&X3);
}

#[test]
fn quotient_x4_matches_the_original() {
    quotient_differential(&X4);
}

#[test]
fn quotient_x5_matches_the_original() {
    quotient_differential(&X5);
}

// ---- experimental: negative cases ---------------------------------------------------------------

/// `step_adm` with its decision replaced: a deliberately defective
/// admission, for the negative cases only.
fn admit_by(
    config: &Config,
    w: &mut World,
    obs: &mut Obs,
    admits: impl Fn(&World, HealthView) -> bool,
) -> bool {
    if w.adm == 0 {
        return false;
    }
    obs.views += 1;
    let view = w.health.view();
    let admitted = admits(w, view);
    w.adm -= 1;
    let label = if admitted { "admitted" } else { "refused" };
    obs.branch(format!("admission: {label}"));
    obs.note(|| format!("[T] admission (defective): {label}"));
    if admitted {
        admission_properties(config, w, obs);
    }
    true
}

/// Admits without looking at `unapplied`.
fn adm_ignoring_unapplied(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    admit_by(config, w, obs, |w, view| {
        view.word & FLAGS == SESSION_OPEN && w.table.debt == 0 && w.table.h <= w.table.a
    })
}

/// Admits although recovery is required (H above A).
fn adm_ignoring_recovery(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    admit_by(config, w, obs, |w, view| view.admits() && w.table.debt == 0)
}

/// Admits although the core is latched, and whatever the debt the latch
/// leaves.
fn adm_ignoring_the_latch(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    admit_by(config, w, obs, |w, view| {
        view.word & FLAGS & !LATCHED == SESSION_OPEN
            && field(view.word, UNAPPLIED_SHIFT) == 0
            && w.table.h <= w.table.a
    })
}

/// The real admission, and then a second load in the same step.
fn adm_viewing_twice(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    let enabled = step_adm(config, w, obs);
    if enabled {
        let _ = w.health.view();
    }
    enabled
}

/// The real admission, and then its view's low bits kept in the world, as
/// A2's first step keeps them.
fn adm_keeping_bits(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    let enabled = step_adm(config, w, obs);
    if enabled {
        w.adm_kept = Some(word_of(&w.core.shared) & FLAGS);
    }
    enabled
}

/// The real admission, without spending the attempt.
fn adm_keeping_its_counter(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    let enabled = step_adm(config, w, obs);
    if enabled {
        w.adm += 1;
    }
    enabled
}

/// The real admission, and then a write to the test's table.
fn adm_writing_the_table(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    let enabled = step_adm(config, w, obs);
    if enabled {
        w.table.a += 1;
    }
    enabled
}

/// Replays `edges` in the original semantics, with `adm` as admission, each
/// step enabled and taking exactly its recorded choices; returns the
/// properties its last step violates.
fn last_step_violations(config: &Config, edges: &[u64], adm: AdmStep) -> Vec<&'static str> {
    let all = actors(config);
    let mut key = encode(config, &initial(config));
    let mut last = Vec::new();
    for &edge in edges {
        let (actor, choices) = unpack(edge);
        let base = decode(config, &key);
        let mut w = decode(config, &key);
        let mut oracle = Oracle::new(choices.clone(), w.faults);
        let mut obs = Obs::default();
        hook::take_counts();
        let enabled = step_with(config, &mut w, all[actor], &mut oracle, &mut obs, adm);
        let counts = hook::take_counts();
        assert!(enabled, "a replayed step is not enabled");
        assert_eq!(
            oracle.sequence(),
            choices,
            "a replayed step took other choices"
        );
        w.faults = oracle.faults;
        transition_properties(&base, &w, &mut obs, counts);
        last = obs
            .violations
            .iter()
            .map(|(property, _)| *property)
            .collect();
        key = encode(config, &w);
    }
    last
}

/// A defective admission fails `property` in both modes, with shortest
/// traces of one length ending in ADM; the quotient's trace is an original
/// trace: replayed with the same admission, its last step violates
/// `property`.
fn negative_case(config: &Config, adm: AdmStep, property: &'static str) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let adm_index = actors(config)
        .iter()
        .position(|&actor| actor == Actor::Adm)
        .expect("ADM is an actor");
    let (full, full_tally) = explore_experimental(config, adm, Mode::Full);
    let (projected, tally) = explore_experimental(config, adm, Mode::Quotient);
    println!("{}\n{}", full.text, projected.text);
    for report in [&full, &projected] {
        assert_eq!(report.verdict, Verdict::Fail, "{}", report.text);
        assert_eq!(report.failed, BTreeSet::from([property]), "{}", report.text);
    }
    let (full_trace, trace) = (
        full_tally.trace.expect("a trace"),
        tally.trace.expect("a trace"),
    );
    assert_eq!(
        full_trace.len(),
        trace.len(),
        "shortest traces differ in length"
    );
    for edges in [&full_trace, &trace] {
        assert_eq!(edges.last().map(|&edge| unpack(edge).0), Some(adm_index));
    }
    assert!(
        last_step_violations(config, &trace, adm).contains(&property),
        "the quotient's trace does not replay to its violation"
    );
    println!(
        "NEGATIVE {}: FAIL {property} in both modes; shortest traces {} steps; the quotient's trace \
         replays in the original semantics to {property}",
        config.name,
        trace.len()
    );
}

#[test]
fn quotient_negative_p1_admission_ignores_unapplied() {
    negative_case(&X1, adm_ignoring_unapplied, "P1");
}

#[test]
fn quotient_negative_p12_admission_ignores_recovery() {
    negative_case(&X4, adm_ignoring_recovery, "P12");
}

#[test]
fn quotient_negative_e9_admission_ignores_the_latch() {
    negative_case(&X1, adm_ignoring_the_latch, "E9");
}

#[test]
fn quotient_negative_e2_admission_views_twice() {
    negative_case(&X1, adm_viewing_twice, "E2");
}

/// The observer on forced states (not reachable): it reports what the
/// admission step reports, and only when the step admits.
#[test]
fn quotient_observer_reports_the_admission_properties() {
    let observe = |config: &Config, w: &World| -> Successor {
        let index = actors(config)
            .iter()
            .position(|&actor| actor == Actor::Adm)
            .expect("ADM is an actor");
        let key = encode(config, w);
        let base = decode(config, &key);
        observe_admission(config, &base, &key, index, step_adm).expect("ADM is enabled")
    };
    let named = |s: &Successor| -> Vec<&'static str> {
        s.violations.iter().map(|(property, _)| *property).collect()
    };
    // P1, and E6 for a batch: a result is unrecorded.
    let batch = Config {
        admissions: 1,
        ..X5
    };
    let mut w = initial(&batch);
    w.ghost.unrecorded = bit(0);
    let s = observe(&batch, &w);
    assert_eq!(named(&s), ["P1", "E6"]);
    assert_eq!(s.branches, ["admission: admitted"]);
    // P12 and P15: an explanation no acknowledgement covers.
    let mut w = initial(&X4);
    w.rows.push(SimRow::Explanation(0));
    assert_eq!(named(&observe(&X4, &w)), ["P12", "P15"]);
    // E9: a latch, not yet published.
    let mut w = initial(&X1);
    w.core.latch = Some(LatchCause::Constraint);
    assert_eq!(named(&observe(&X1, &w)), ["E9"]);
    // A debt: refused, and nothing is reported, although a result is
    // unrecorded.
    let mut w = initial(&X1);
    w.ghost.unrecorded = bit(0);
    set_class(&mut w.table, 0, Class::Refused);
    let s = observe(&X1, &w);
    assert!(s.violations.is_empty(), "{:?}", s.violations);
    assert_eq!(s.branches, ["admission: refused"]);
}

#[test]
#[should_panic(expected = "quotient inapplicable: ADM kept bits in the world (A2)")]
fn quotient_refuses_an_admission_that_keeps_bits() {
    let _ = explore_experimental(&X1, adm_keeping_bits, Mode::Quotient);
}

#[test]
#[should_panic(expected = "quotient inapplicable: ADM did not spend exactly one admission")]
fn quotient_refuses_an_admission_that_keeps_its_counter() {
    let _ = explore_experimental(&X1, adm_keeping_its_counter, Mode::Quotient);
}

#[test]
#[should_panic(expected = "quotient inapplicable: ADM changed the world beyond its counter")]
fn quotient_refuses_an_admission_that_writes_the_table() {
    let _ = explore_experimental(&X1, adm_writing_the_table, Mode::Quotient);
}

/// Why the counter must fall: an admission that keeps it makes a non-fault
/// self-loop, which the original semantics reports as L1.
#[test]
fn an_admission_that_keeps_its_counter_fails_l1_in_the_original() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (report, _) = explore_experimental(&X1, adm_keeping_its_counter, Mode::Full);
    assert_eq!(report.verdict, Verdict::Fail, "{}", report.text);
    assert_eq!(report.failed, BTreeSet::from(["L1"]), "{}", report.text);
}

// ---- experimental: the gating-caller symmetry quotient ------------------------------------------
//
// P3-C11-DR01-D2A1-GC-SYMMETRY-01, a bounded experiment. It is not the
// contract's explorer and not evidence for the contract; `explore` and
// `explore_experimental` above are unchanged and stay the references.
//
// G1 and G2 run one program, each with its own record. Exchanging them
// renames every value that names a caller: its record's identity and
// digests, its rows in the ledger (its record, a correction about it, an
// explanation of it), the cells' occupants and the causes that hold them,
// the refusal table, GC's two positions and the history bits `issued` and
// `told`. Rows keep their places, so every sequence keeps its meaning. With
// no acknowledgement actor (a snapshot's digest is a mask indexed by
// original, and a malformed one loses its lowest bit) and no gating conflict
// (G2's identity alone is seeded with other content), the exchange fixes the
// initial state and maps every step to a step with the same choices, fault,
// branches and property names: an automorphism of the graph. The quotient
// keeps one key for each pair {k, mirror(k)}, the smaller. It expands each
// key it keeps and that key's mirror, and holds the second expansion to the
// exchange of the first; a difference aborts the run with no verdict. A path
// of the quotient is unfolded into the original path it stands for, with the
// callers' own names, and replayed in the original semantics.

/// A supplementary configuration for the differential: X-2's two callers
/// beside one conflicting fact, whose Prepared record ENV supplies, as F1's
/// in F-3.
const SYM1: Config = default_run(Config {
    name: "SYM-1",
    facts: 1,
    conflict: [true, false],
    env: true,
    faults: 1,
    admissions: 1,
    gating: 2,
    ..BASE
});

/// The configurations this experiment showed symmetric: X-2 and SYM-1 by the
/// exhaustive differential below, F-3 by the analysis of its mission; SYM-PT,
/// SYM-B and SYM-BI by the exhaustive differential at the end of this file,
/// F-PT and F-B by the analysis of P3-C11-DR01-D2A1-CLOSURE-PREP-01. A
/// configuration is accepted only with exactly one of these shapes and a cap
/// no higher than its own.
const SYMMETRIC: [Config; 8] = [X2, SYM1, F3, SYM_PT, SYM_B, SYM_BI, FPT, FB];

/// A configuration without its caps.
fn shape(config: &Config) -> String {
    format!(
        "{:?}",
        Config {
            cap: 0,
            time: Duration::ZERO,
            ..*config
        }
    )
}

/// Refuses a configuration whose callers this experiment did not show
/// exchangeable, and one whose initial state is not its own mirror.
fn assert_symmetric(config: &Config) {
    assert!(
        config.gating == 2,
        "symmetry inapplicable: {} has not exactly two gating callers",
        config.name
    );
    assert!(
        !config.gating_conflict,
        "symmetry inapplicable: {} seeds a conflict for G2 alone",
        config.name
    );
    assert!(
        config.acks == 0 && config.ack_attempts == 0 && !config.malformed,
        "symmetry inapplicable: {} has acknowledgements, whose digest is a mask indexed by original",
        config.name
    );
    let Some(known) = SYMMETRIC.iter().find(|known| shape(known) == shape(config)) else {
        panic!(
            "symmetry inapplicable: {} is not a configuration this experiment showed symmetric",
            config.name
        );
    };
    assert!(
        config.cap <= known.cap && config.time <= known.time,
        "symmetry inapplicable: {} raises its cap",
        config.name
    );
    let init = encode(config, &initial(config));
    assert!(
        mirror(config, &init) == init,
        "symmetry inapplicable: the initial state of {} is not its own mirror",
        config.name
    );
}

/// The other gating caller.
fn other_caller(g: u8) -> u8 {
    assert!(g < 2, "symmetry inapplicable: gating caller {g}");
    1 - g
}

/// A 32-byte value with G1 and G2 exchanged. Its first byte says what it
/// names (`bytes`, `SimRow::key`): 2, a gating record or its identity with
/// other content; 6, a correction about one; 5, an explanation of original
/// o, where G1 and G2 are originals 2 and 3; 1, 3, 4 and 9, a fact, a
/// holder, a Prepared record and a commitment, which name no caller.
fn mirror_x32(value: &[u8; 32]) -> [u8; 32] {
    assert!(
        value[3..].iter().all(|&b| b == 0),
        "symmetry inapplicable: a value outside the encoding"
    );
    let (kind, which, low) = (value[0], value[1], value[2]);
    let which = match (kind, which) {
        (2 | 6, _) => other_caller(which),
        (5, 0 | 1) | (1 | 3 | 4 | 9, _) => which,
        (5, 2 | 3) => 5 - which,
        _ => panic!(
            "symmetry inapplicable: a value the exchange does not know: {:?}",
            &value[..3]
        ),
    };
    bytes(kind, which, low)
}

fn mirror_key(key: &RecordKey) -> RecordKey {
    RecordKey {
        id: mirror_x32(&key.id),
        digest: mirror_x32(&key.digest),
    }
}

fn mirror_part(part: &Part) -> Part {
    Part {
        commitment: mirror_x32(&part.commitment),
        role: part.role,
    }
}

fn mirror_member(member: &Member) -> Member {
    Member {
        key: mirror_key(&member.key),
        part: member.part.as_ref().map(mirror_part),
    }
}

/// A cause with every identity and digest exchanged and every sequence kept.
fn mirror_cause(cause: &Cause) -> Cause {
    let kind = match cause.kind() {
        CauseKind::IdentityContent {
            id,
            existing,
            offered,
        } => CauseKind::IdentityContent {
            id: mirror_x32(&id),
            existing: Row {
                seq: existing.seq,
                digest: mirror_x32(&existing.digest),
            },
            offered: mirror_x32(&offered),
        },
        CauseKind::DuplicateRole {
            part,
            holder,
            offered,
        } => CauseKind::DuplicateRole {
            part: mirror_part(&part),
            holder: Holder {
                id: mirror_x32(&holder.id),
                seq: holder.seq,
            },
            offered: mirror_key(&offered),
        },
        CauseKind::MissingPrepared { part, offered } => CauseKind::MissingPrepared {
            part: mirror_part(&part),
            offered: mirror_key(&offered),
        },
    };
    Cause { kind }
}

fn mirror_occupant(occupant: &Occupant) -> Occupant {
    match occupant {
        Occupant::Gating { record, exit } => Occupant::Gating {
            record: GatingRecord::new(mirror_member(record.member())),
            exit: *exit,
        },
        Occupant::Correction { about, seq, exit } => Occupant::Correction {
            about: mirror_key(about),
            seq: *seq,
            exit: *exit,
        },
        Occupant::Explanation { original, cause } => Occupant::Explanation {
            original: mirror_key(original),
            cause: mirror_cause(cause),
        },
        Occupant::Acknowledgement { .. } => {
            panic!("symmetry inapplicable: an acknowledgement, whose digest is a mask indexed by original")
        }
    }
}

/// A row with G1 and G2 exchanged; it keeps its place in the ledger.
fn mirror_row(row: SimRow) -> SimRow {
    match row {
        SimRow::Prepared(_) | SimRow::Holder(_) | SimRow::Fact(_) => row,
        SimRow::Other(g) => SimRow::Other(other_caller(g)),
        SimRow::Gating(g) => SimRow::Gating(other_caller(g)),
        SimRow::Correction(g, exit) => SimRow::Correction(other_caller(g), exit),
        SimRow::Explanation(o) => SimRow::Explanation(match o {
            0 | 1 => o,
            2 | 3 => 5 - o,
            _ => panic!("symmetry inapplicable: original {o}"),
        }),
        SimRow::Acknowledgement { .. } => {
            panic!("symmetry inapplicable: an acknowledgement, whose digest is a mask indexed by original")
        }
    }
}

/// History bits with G1's and G2's exchanged.
fn mirror_bits(bits: u8) -> u8 {
    assert!(
        bits < 4,
        "symmetry inapplicable: a history bit beyond two callers"
    );
    ((bits & 1) << 1) | (bits >> 1)
}

/// `w` with G1 and G2 exchanged, in place. The world, the core, settlement
/// and the history are destructured whole, so that a field added to any of
/// them must be placed here before this compiles. The writers' tickets and
/// positions, the worker's agenda step and tokens (a cell and an occupancy
/// number), the table, the budgets and every sequence name no caller.
fn mirror_world(w: &mut World) {
    let World {
        core,
        health: _,
        rows,
        fw: _,
        se,
        wk: _,
        gc,
        ack,
        adm: _,
        adm_kept,
        env_done: _,
        replays: _,
        table: _,
        faults: _,
        ghost,
    } = w;
    let Core {
        shared: _,
        phase: _,
        session: _,
        latch: _,
        u,
        c,
        x,
        occupancies: _,
        refusals,
        refusal_limit: _,
        frontier: _,
        worker_taken: _,
    } = core;
    if let Some((_, occupant)) = u.as_mut() {
        *occupant = mirror_occupant(occupant);
    }
    if let Some((_, occupant)) = c.as_mut() {
        *occupant = mirror_occupant(occupant);
    }
    if let Some((_, occupant, _)) = x.as_mut() {
        *occupant = mirror_occupant(occupant);
    }
    let table = std::mem::take(refusals);
    *refusals = table
        .iter()
        .map(|(key, cause)| (mirror_key(key), mirror_cause(cause)))
        .collect();
    assert_eq!(
        refusals.len(),
        table.len(),
        "symmetry inapplicable: the exchange merged two refusal entries"
    );
    for row in rows.iter_mut() {
        *row = mirror_row(*row);
    }
    let Se { ticket: _, pos } = se;
    if let SePos::Got(_, SeHeld::Lookup(Lookup::Conflict(Known::Cause(cause)))) = pos {
        *cause = mirror_cause(cause);
    }
    assert_eq!(gc.len(), 2, "symmetry inapplicable: not two gating callers");
    gc.swap(0, 1);
    assert!(
        ack.is_empty(),
        "symmetry inapplicable: an acknowledgement actor"
    );
    assert!(
        adm_kept.is_none(),
        "symmetry inapplicable: A2's kept bits, a mutation control's"
    );
    let Ghost {
        conflicted: _,
        conflicted_e6: _,
        unrecorded: _,
        issued,
        told,
        dropped: _,
    } = ghost;
    *issued = mirror_bits(*issued);
    *told = mirror_bits(*told);
}

/// The key of `key`'s world with G1 and G2 exchanged.
fn mirror(config: &Config, key: &[u8]) -> Vec<u8> {
    let mut w = decode(config, key);
    mirror_world(&mut w);
    encode(config, &w)
}

/// The key that stands for a key and its mirror: the smaller.
fn canonical(key: Vec<u8>, mirrored: Vec<u8>) -> Vec<u8> {
    if mirrored < key {
        mirrored
    } else {
        key
    }
}

/// An edge with G1's and G2's actors exchanged and its choices kept.
fn mirror_edge(all: &[Actor], edge: u64) -> u64 {
    let (index, choices) = unpack(edge);
    let index = match all[index] {
        Actor::Gc(g) => all
            .iter()
            .position(|&actor| actor == Actor::Gc(other_caller(g)))
            .expect("both gating callers are actors"),
        _ => index,
    };
    pack(index, &choices)
}

/// An item as the exchange compares it: its kind, edge, fault, successor,
/// the names of the properties it violates (sorted: their order follows the
/// callers' order) and its branches.
#[derive(Debug, PartialEq, Eq)]
struct Image {
    admission: bool,
    edge: u64,
    fault: bool,
    key: Vec<u8>,
    names: Vec<&'static str>,
    branches: Vec<String>,
}

fn names(violations: &Violations) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = violations.iter().map(|(property, _)| *property).collect();
    names.sort_unstable();
    names
}

/// An expansion by actor, each actor's items in their order.
fn by_actor(images: impl Iterator<Item = Image>) -> BTreeMap<usize, Vec<Image>> {
    let mut out: BTreeMap<usize, Vec<Image>> = BTreeMap::new();
    for image in images {
        out.entry(unpack(image.edge).0).or_default().push(image);
    }
    out
}

/// Holds a key's expansion and its mirror's to the exchange: the mirror's
/// items are the key's, actor by actor with G1's and G2's exchanged, each
/// with the same choices, fault, property names and branches and the mirror
/// of the key's successor; the two states violate properties of the same
/// names. Returns the mirrors of the key's successors.
fn hold(
    config: &Config,
    all: &[Actor],
    own: (&[Item], &Violations),
    other: (&[Item], &Violations),
) -> Result<Vec<Vec<u8>>, String> {
    let mirrored: Vec<Vec<u8>> = own.0.iter().map(|item| mirror(config, &item.key)).collect();
    if names(own.1) != names(other.1) {
        return Err(format!(
            "a state violates {:?} and its mirror {:?}",
            names(own.1),
            names(other.1)
        ));
    }
    let expected = by_actor(own.0.iter().zip(&mirrored).map(|(item, key)| Image {
        admission: item.admission,
        edge: mirror_edge(all, item.edge),
        fault: item.fault,
        key: key.clone(),
        names: names(&item.violations),
        branches: item.branches.clone(),
    }));
    let found = by_actor(other.0.iter().map(|item| Image {
        admission: item.admission,
        edge: item.edge,
        fault: item.fault,
        key: item.key.clone(),
        names: names(&item.violations),
        branches: item.branches.clone(),
    }));
    if expected != found {
        let actor = expected
            .keys()
            .chain(found.keys())
            .find(|actor| expected.get(*actor) != found.get(*actor))
            .copied();
        return Err(format!(
            "an expansion is not the exchange of its mirror's (first at actor {:?})",
            actor.map(|index| all[index])
        ));
    }
    Ok(mirrored)
}

/// One step from `key` along `edge` in the original semantics, ADM played by
/// `adm`: enabled, and taking exactly the edge's choices.
fn replay(config: &Config, key: &[u8], edge: u64, adm: AdmStep) -> Vec<u8> {
    let all = actors(config);
    let (actor, choices) = unpack(edge);
    let mut w = decode(config, key);
    let mut oracle = Oracle::new(choices.clone(), w.faults);
    let mut obs = Obs::default();
    let enabled = step_with(config, &mut w, all[actor], &mut oracle, &mut obs, adm);
    assert!(enabled, "unfold: a replayed step is not enabled");
    assert_eq!(
        oracle.sequence(),
        choices,
        "unfold: a replayed step took other choices"
    );
    w.faults = oracle.faults;
    encode(config, &w)
}

/// A path of the quotient as the original path it stands for. At each step
/// the original path stands at the quotient's key or at its mirror; from the
/// mirror, the edge is taken with G1's and G2's actors exchanged. Each step
/// is replayed in the original semantics and must reach the quotient's next
/// key or its mirror.
fn unfold(config: &Config, edges: &[u64], adm: AdmStep) -> Vec<u64> {
    let all = actors(config);
    let mut key = encode(config, &initial(config));
    let mut state = key.clone();
    let mut out = Vec::new();
    for &edge in edges {
        let taken = if state == key {
            edge
        } else {
            assert!(
                state == mirror(config, &key),
                "unfold: the original path left its key's pair"
            );
            mirror_edge(&all, edge)
        };
        let next = replay(config, &key, edge, adm);
        let next_mirror = mirror(config, &next);
        state = replay(config, &state, taken, adm);
        assert!(
            state == next || state == next_mirror,
            "unfold: a step left its pair"
        );
        key = canonical(next, next_mirror);
        out.push(taken);
    }
    out
}

/// A quotient path, unfolded and described step by step.
fn witness(config: &Config, edges: &[u64], adm: AdmStep) -> String {
    let unfolded = unfold(config, edges, adm);
    format!(
        "  quotient path {edges:?}\n  unfolded ({} steps):\n{}",
        unfolded.len(),
        trace_with(config, &unfolded, adm)
    )
}

/// Kahn's algorithm as `explore` runs it for L1, over `n` states whose
/// non-fault successors are `to[from[s]..from[s + 1]]`: the number of states
/// removed, and the in-degrees left. A self-loop is an in-degree its state
/// never loses, so it is found as a cycle.
fn kahn(n: usize, from: &[u64], to: &[u32]) -> (usize, Vec<u32>) {
    let mut indegree = vec![0u32; n];
    for &t in to {
        indegree[t as usize] += 1;
    }
    let mut queue: Vec<u32> = (0..n as u32)
        .filter(|&s| indegree[s as usize] == 0)
        .collect();
    let mut removed = 0usize;
    while let Some(s) = queue.pop() {
        removed += 1;
        let (lo, hi) = (from[s as usize] as usize, from[s as usize + 1] as usize);
        for &t in &to[lo..hi] {
            indegree[t as usize] -= 1;
            if indegree[t as usize] == 0 {
                queue.push(t);
            }
        }
    }
    (removed, indegree)
}

/// What the symmetry quotient counted beyond `Tally`: its expanded keys that
/// are their own mirror and their share of each count, from which the graph
/// it stands for is derived (a pair of distinct keys counts twice, a key
/// that is its own mirror once); and its shortest trace before unfolding.
#[derive(Debug, Default, PartialEq, Eq)]
struct Mirrors {
    fixed: u64,
    fixed_transitions: u64,
    fixed_fault_edges: u64,
    fixed_nonfault_edges: u64,
    fixed_no_step: u64,
    fixed_evaluations: u64,
    fixed_admitted: u64,
    folded: Option<Vec<u64>>,
}

/// The quotient's keys and parent edges, kept for the differential tests.
type Kept = (Vec<Vec<u8>>, Vec<(u32, u64)>);

fn explore_symmetric(config: &Config, adm: AdmStep, mode: Mode) -> (Report, Tally, Mirrors) {
    let (report, tally, mirrors, _) = explore_symmetric_kept(config, adm, mode, false);
    (report, tally, mirrors)
}

/// `explore_experimental` over the symmetry quotient, in either mode: each
/// key stands for itself and its mirror. Every expanded key's mirror is
/// expanded too and held to the exchange; a difference, a key that is not
/// its mirror's mirror, or a configuration outside `SYMMETRIC` aborts the
/// run with no verdict. A failing trace is unfolded to the callers' own
/// names.
fn explore_symmetric_kept(
    config: &Config,
    adm: AdmStep,
    mode: Mode,
    keep: bool,
) -> (Report, Tally, Mirrors, Option<Kept>) {
    assert_symmetric(config);
    let start = Instant::now();
    let all = actors(config);
    let adm_index = all
        .iter()
        .position(|&actor| actor == Actor::Adm)
        .expect("ADM is an actor");
    let mut visited = Visited::new();
    let mut parent: Vec<(u32, u64)> = Vec::new();
    let mut nonfault_from: Vec<u64> = vec![0];
    let mut nonfault: Vec<u32> = Vec::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let (mut transitions, mut fault_edges, mut no_step) = (0u64, 0u64, 0u64);
    let (mut evaluations, mut admitted) = (0u64, 0u64);
    let mut mirrors = Mirrors::default();
    let mut failure: Option<(&'static str, String, Vec<u64>)> = None;
    let mut incomplete: Option<String> = None;
    let init = encode(config, &initial(config));
    visited.insert(&init);
    parent.push((EMPTY, 0));
    let path_to = |parent: &[(u32, u64)], mut id: u32| {
        let mut edges = Vec::new();
        while parent[id as usize].0 != EMPTY {
            edges.push(parent[id as usize].1);
            id = parent[id as usize].0;
        }
        edges.reverse();
        edges
    };
    let mut id = 0u32;
    while (id as usize) < visited.len() {
        if visited.len() as u64 > config.cap {
            incomplete = Some(format!("the state cap of {} was reached", config.cap));
            break;
        }
        if start.elapsed() > config.time {
            incomplete = Some(format!("the time cap of {:?} was reached", config.time));
            break;
        }
        let key = visited.key(id).to_vec();
        let mirrored = mirror(config, &key);
        if mirror(config, &mirrored) != key {
            panic!(
                "symmetry inapplicable: a key is not its mirror's mirror, at a key reached by\n{}",
                witness(config, &path_to(&parent, id), adm)
            );
        }
        let fixed = mirrored == key;
        let (next, state_violations) = expand(config, &all, &key, adm, mode);
        let (other, other_violations) = expand(config, &all, &mirrored, adm, mode);
        let (next, other) = (items(next), items(other));
        let successor_mirrors = match hold(
            config,
            &all,
            (&next, &state_violations),
            (&other, &other_violations),
        ) {
            Ok(successor_mirrors) => successor_mirrors,
            Err(detail) => panic!(
                "symmetry inapplicable: {detail}, at a key reached by\n{}",
                witness(config, &path_to(&parent, id), adm)
            ),
        };
        if fixed {
            mirrors.fixed += 1;
        }
        if let Some((property, detail)) = state_violations.into_iter().next() {
            failure = Some((property, detail, path_to(&parent, id)));
            break;
        }
        if !next.iter().any(|item| !item.admission) {
            no_step += 1;
            if fixed {
                mirrors.fixed_no_step += 1;
            }
            // Under the admission quotient the state still holds its
            // admissions: the state with no enabled step is this one with
            // them spent.
            let (mut end, mut other_end) = (decode(config, &key), decode(config, &mirrored));
            if mode == Mode::Quotient {
                end.adm = 0;
                other_end.adm = 0;
            }
            let judged = terminal(config, &end);
            assert!(
                judged == terminal(config, &other_end),
                "symmetry inapplicable: D1 judges a state and its mirror apart"
            );
            if !judged {
                let mut edges = path_to(&parent, id);
                if mode == Mode::Quotient {
                    let spend = usize::from(config.admissions);
                    edges.extend(std::iter::repeat_n(pack(adm_index, &[]), spend));
                }
                failure = Some((
                    "D1",
                    "a state with no enabled step is not terminal".into(),
                    edges,
                ));
                break;
            }
        }
        for (item, successor_mirror) in next.into_iter().zip(successor_mirrors) {
            if item.admission {
                evaluations += 1;
                let admits = item
                    .branches
                    .iter()
                    .any(|label| label == "admission: admitted");
                if admits {
                    admitted += 1;
                }
                if fixed {
                    mirrors.fixed_evaluations += 1;
                    mirrors.fixed_admitted += u64::from(admits);
                }
                taken.extend(item.branches);
                if let Some((property, detail)) = item.violations.into_iter().next() {
                    let mut edges = path_to(&parent, id);
                    edges.push(item.edge);
                    failure = Some((property, detail, edges));
                    break;
                }
                continue;
            }
            transitions += 1;
            if fixed {
                mirrors.fixed_transitions += 1;
            }
            taken.extend(item.branches);
            if let Some((property, detail)) = item.violations.into_iter().next() {
                let mut edges = path_to(&parent, id);
                edges.push(item.edge);
                failure = Some((property, detail, edges));
                break;
            }
            let (to, new) = visited.insert(&canonical(item.key, successor_mirror));
            if new {
                parent.push((id, item.edge));
            }
            if item.fault {
                fault_edges += 1;
                if fixed {
                    mirrors.fixed_fault_edges += 1;
                }
            } else {
                nonfault.push(to);
                if fixed {
                    mirrors.fixed_nonfault_edges += 1;
                }
            }
        }
        if failure.is_some() {
            break;
        }
        nonfault_from.push(nonfault.len() as u64);
        id += 1;
    }
    let mut l1 = "not claimed for this configuration".to_string();
    if failure.is_none() && incomplete.is_none() && config.l1 {
        let n = visited.len();
        let (removed, indegree) = kahn(n, &nonfault_from, &nonfault);
        if removed == n {
            l1 = format!(
                "HOLDS: {} non-fault edges, no cycle; {no_step} end states, each terminal (D1)",
                nonfault.len()
            );
        } else {
            let cyclic = (0..n)
                .find(|&s| indegree[s] > 0)
                .expect("a state on a cycle") as u32;
            failure = Some((
                "L1",
                format!(
                    "{} states lie on or behind a cycle of non-fault steps; one is reached by",
                    n - removed
                ),
                path_to(&parent, cyclic),
            ));
        }
    }
    let elapsed = start.elapsed();
    let verdict = if failure.is_some() {
        Verdict::Fail
    } else if incomplete.is_some() {
        Verdict::Incomplete
    } else {
        Verdict::Pass
    };
    let mut failed = BTreeSet::new();
    let mut text = String::new();
    let what = match mode {
        Mode::Full => "gating-caller symmetry quotient",
        Mode::Quotient => {
            "gating-caller symmetry quotient composed with the admission observer quotient"
        }
    };
    let _ = writeln!(
        text,
        "EXPLORATION {} ({what}: experimental, not the contract's explorer)",
        config.name
    );
    let _ = writeln!(
        text,
        "  outcome: {}",
        match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Incomplete => "INCOMPLETE",
        }
    );
    let _ = writeln!(
        text,
        "  bounds: facts {} conflicting {:?} ENV {} batch {} faults {} admissions {} gating callers {} \
         gating conflict {} acknowledgement actors {} attempts each {} malformed snapshots {} cell constraint {} \
         worker verbs unwind {} DROP {} REPLAY {} A4 {}; ticket limit {TICKET_LIMIT}, refusal limit \
         {REFUSAL_LIMIT}; settlement and the worker on; caps {} states, {:?}",
        config.facts,
        config.conflict,
        config.env,
        config.batch,
        config.faults,
        config.admissions,
        config.gating,
        config.gating_conflict,
        config.acks,
        config.ack_attempts,
        config.malformed,
        config.cell_constraint,
        config.worker_unwinds,
        config.drop,
        config.replays,
        config.blanket_uncertain,
        config.cap,
        config.time
    );
    let _ = writeln!(text, "  actors: {all:?}");
    if mode == Mode::Quotient {
        let _ = writeln!(
            text,
            "  quotient: ADM is not scheduled; its one step is evaluated at every expanded state ({evaluations} \
             evaluations, {admitted} admitted) and its successor discarded; every key holds {} admissions left; \
             D1 is judged with them spent. The states and transitions below are the quotient's, not the \
             original graph's, and the state cap counts quotient states.",
            config.admissions
        );
    }
    let _ = writeln!(
        text,
        "  symmetry: G1 and G2 exchanged; each key stands for itself and its mirror, the smaller kept; every \
         expanded key's mirror was expanded and held to the exchange; {} expanded keys are their own mirror. \
         The states and transitions below are the symmetry quotient's, and the state cap counts them.",
        mirrors.fixed
    );
    let _ = writeln!(
        text,
        "  states {} (expanded {}), transitions {transitions} (with a fault {fault_edges}), \
         states with no enabled step {no_step}",
        visited.len(),
        id
    );
    let _ = writeln!(
        text,
        "  elapsed {:.1} s; explorer tables {} MiB",
        elapsed.as_secs_f64(),
        visited.bytes() >> 20
    );
    if let Some(reason) = &incomplete {
        let _ = writeln!(
            text,
            "  INCOMPLETE: {reason}. No property is established by this run."
        );
    }
    let mut trace = None;
    match &failure {
        Some((property, detail, edges)) => {
            failed.insert(*property);
            let unfolded = unfold(config, edges, adm);
            let _ = writeln!(text, "  FAIL: {property}: {detail}");
            let _ = writeln!(
                text,
                "  shortest trace ({} steps, unfolded to the callers' own names):",
                unfolded.len()
            );
            text.push_str(&trace_with(config, &unfolded, adm));
            mirrors.folded = Some(edges.clone());
            trace = Some(unfolded);
        }
        None => {
            for property in PROPERTIES {
                let verdict = if incomplete.is_some() {
                    "NOT ESTABLISHED (incomplete)".to_string()
                } else if property == "L1" {
                    l1.clone()
                } else if property == "E10" && !config.worker_unwinds {
                    "not applicable here (worker verbs do not unwind)".to_string()
                } else {
                    "HOLDS".to_string()
                };
                let _ = writeln!(text, "  {property}: {verdict}");
            }
        }
    }
    let _ = writeln!(text, "  branches:");
    for label in BRANCHES {
        let mark = if taken.contains(*label) {
            "taken    "
        } else {
            "NOT TAKEN"
        };
        let _ = writeln!(text, "    [{mark}] {label}");
    }
    for label in taken
        .iter()
        .filter(|label| !BRANCHES.contains(&label.as_str()))
    {
        let _ = writeln!(text, "    [taken    ] {label} (not in the listed set)");
    }
    let _ = writeln!(text, "  state key fields: {KEY_FIELDS}");
    let kept = keep.then(|| {
        (
            (0..visited.len() as u32)
                .map(|id| visited.key(id).to_vec())
                .collect(),
            parent,
        )
    });
    let tally = Tally {
        states: visited.len() as u64,
        expanded: u64::from(id),
        transitions,
        fault_edges,
        nonfault_edges: nonfault.len() as u64,
        no_step,
        evaluations,
        admitted,
        taken,
        trace,
    };
    (
        Report {
            verdict,
            text,
            failed,
        },
        tally,
        mirrors,
        kept,
    )
}

/// The full F-3, in the symmetry quotient composed with the admission
/// observer quotient. Selectable only by name; not run by this mission.
#[test]
#[ignore = "experimental gating-caller symmetry quotient: run only when an Architect mission names it"]
fn gc_symmetry_f3() {
    let (report, _, _) = explore_symmetric(&F3, step_adm, Mode::Quotient);
    println!("{}", report.text);
    assert_eq!(
        report.verdict,
        Verdict::Pass,
        "failed: {:?}\n{}",
        report.failed,
        report.text
    );
}

/// A bounded prefix of F-3 in the composed quotient, a diagnostic of the
/// exchange on F-3's states: at most 100,000 keys (the cap check stops one
/// key past the cap), each expanded key's mirror expanded and held to the
/// exchange. INCOMPLETE by construction; it establishes nothing about F-3.
#[test]
#[ignore = "experimental gating-caller symmetry quotient: a bounded F-3 prefix, run only when an Architect mission names it"]
fn gc_symmetry_prefix_f3() {
    let config = Config { cap: 99_999, ..F3 };
    let (report, tally, mirrors) = explore_symmetric(&config, step_adm, Mode::Quotient);
    println!("{}", report.text);
    println!(
        "PREFIX F-3 (INCOMPLETE by construction): {} keys ({} expanded, {} of them their own mirror), {} \
         transitions ({} with a fault, {} without), {} admission evaluations ({} admitted), {} with no enabled \
         step. Every expanded key's mirror was expanded and held to the exchange.",
        tally.states,
        tally.expanded,
        mirrors.fixed,
        tally.transitions,
        tally.fault_edges,
        tally.nonfault_edges,
        tally.evaluations,
        tally.admitted,
        tally.no_step
    );
    assert!(tally.states <= 100_000, "the prefix exceeded 100,000 keys");
    assert_eq!(
        report.verdict,
        Verdict::Incomplete,
        "the prefix is not INCOMPLETE: {:?}\n{}",
        report.failed,
        report.text
    );
}

// ---- experimental: the symmetry quotient held to the original -----------------------------------

/// Every value the exchange renames is renamed consistently: a row's key is
/// the exchange of its key, originals and callers map as the exchange says,
/// and every map is an involution.
#[test]
fn gc_symmetry_renames_consistently() {
    let rows = [
        SimRow::Prepared(0),
        SimRow::Prepared(1),
        SimRow::Holder(0),
        SimRow::Holder(1),
        SimRow::Fact(0),
        SimRow::Fact(1),
        SimRow::Other(0),
        SimRow::Other(1),
        SimRow::Gating(0),
        SimRow::Gating(1),
        SimRow::Explanation(0),
        SimRow::Explanation(1),
        SimRow::Explanation(2),
        SimRow::Explanation(3),
        SimRow::Correction(0, Exit::InCall),
        SimRow::Correction(1, Exit::ReturnedUncertain),
        SimRow::Correction(0, Exit::Unwound),
    ];
    for row in rows {
        let image = mirror_row(row);
        assert_eq!(mirror_row(image), row, "{row:?}");
        assert_eq!(image.key(), mirror_key(&row.key()), "{row:?}");
        assert_eq!(
            image.part(),
            row.part().as_ref().map(mirror_part),
            "{row:?}"
        );
        let key = row.key();
        assert_eq!(mirror_key(&mirror_key(&key)), key);
        assert_eq!(
            original_of(&mirror_key(&key)),
            original_of(&key).map(|o| if o < 2 { o } else { 5 - o }),
            "{row:?}"
        );
        assert_eq!(
            gating_of(&mirror_key(&key)),
            gating_of(&key).map(other_caller),
            "{row:?}"
        );
    }
    for g in 0..2 {
        assert_eq!(mirror_key(&gating_key(g)), gating_key(1 - g));
        assert_eq!(
            mirror_member(&gating_member(g)),
            gating_member(1 - g),
            "G{}",
            g + 1
        );
    }
    for f in 0..2 {
        assert_eq!(mirror_member(&fact_member(f)), fact_member(f));
        assert_eq!(mirror_cause(&duplicate_role(f, 2)), duplicate_role(f, 2));
    }
    let cause = Cause {
        kind: CauseKind::IdentityContent {
            id: gating_key(1).id,
            existing: Row {
                seq: 4,
                digest: SimRow::Other(1).key().digest,
            },
            offered: gating_key(1).digest,
        },
    };
    assert_eq!(
        mirror_cause(&cause),
        Cause {
            kind: CauseKind::IdentityContent {
                id: gating_key(0).id,
                existing: Row {
                    seq: 4,
                    digest: SimRow::Other(0).key().digest,
                },
                offered: gating_key(0).digest,
            },
        }
    );
    assert_eq!(mirror_cause(&mirror_cause(&cause)), cause);
    assert_eq!(mirror_bits(0b01), 0b10);
    assert_eq!(mirror_bits(0b11), 0b11);
    assert_eq!(mirror_bits(0), 0);
    let all = actors(&F3);
    let g1 = pack(4, &[0, 1, 2]);
    assert_eq!(all[4], Actor::Gc(0));
    assert_eq!(mirror_edge(&all, g1), pack(5, &[0, 1, 2]));
    assert_eq!(mirror_edge(&all, pack(2, &[1])), pack(2, &[1]));
}

/// S1 and S14: the initial states of the configurations shown symmetric are
/// their own mirrors; F-G's is not, because its ledger seeds G2's identity
/// alone with other content.
#[test]
fn gc_symmetry_initial_states() {
    for config in SYMMETRIC {
        assert_symmetric(&config);
        let init = encode(&config, &initial(&config));
        assert_eq!(mirror(&config, &init), init, "{}", config.name);
    }
    let seeded = initial_rows(&FG);
    let exchanged: Vec<SimRow> = seeded.iter().map(|&row| mirror_row(row)).collect();
    assert!(seeded.contains(&SimRow::Other(1)));
    assert_ne!(exchanged, seeded, "F-G's ledger is not its own mirror");
}

/// Holds the symmetry quotient to the exchange on one configuration: every
/// state of `graph` (the whole original graph, or the whole admission
/// quotient, as `mode` says) against its mirror; the quotient's `kept` keys
/// against the pairs; every key of the quotient against an unfolded
/// original path to it. Returns, as the quotient counts them, the graph's
/// states that are their own mirror and their shares of each count.
fn pairs_of(config: &Config, mode: Mode, graph: &Graph, kept: &Kept) -> Mirrors {
    let all = actors(config);
    let mut counted = Mirrors::default();
    let mut pairs: BTreeMap<Vec<u8>, BTreeSet<usize>> = BTreeMap::new();
    for (id, key) in graph.keys.iter().enumerate() {
        let image = mirror(config, key);
        assert_eq!(
            &mirror(config, &image),
            key,
            "the exchange is not an involution"
        );
        let Some(&other) = graph.ids.get(&image) else {
            panic!("a state's mirror is not in the graph");
        };
        // Every item of the state against its mirror's: the edges by actor
        // with their multiplicities, faults, property names and branches,
        // the successors' mirrors, the observations, the state's own
        // violations.
        let own = (graph.items[id].as_slice(), &graph.state[id]);
        let theirs = (graph.items[other].as_slice(), &graph.state[other]);
        if let Err(detail) = hold(config, &all, own, theirs) {
            panic!("{detail}");
        }
        let dead = !graph.items[id].iter().any(|item| !item.admission);
        assert_eq!(
            dead,
            !graph.items[other].iter().any(|item| !item.admission),
            "D1's states differ"
        );
        if dead {
            let (mut end, mut other_end) = (decode(config, key), decode(config, &image));
            if mode == Mode::Quotient {
                end.adm = 0;
                other_end.adm = 0;
            }
            assert_eq!(
                terminal(config, &end),
                terminal(config, &other_end),
                "terminal differs"
            );
        }
        if image == *key {
            counted.fixed += 1;
            counted.fixed_no_step += u64::from(dead);
            for item in &graph.items[id] {
                if item.admission {
                    counted.fixed_evaluations += 1;
                    counted.fixed_admitted += u64::from(
                        item.branches
                            .iter()
                            .any(|label| label == "admission: admitted"),
                    );
                } else {
                    counted.fixed_transitions += 1;
                    if item.fault {
                        counted.fixed_fault_edges += 1;
                    } else {
                        counted.fixed_nonfault_edges += 1;
                    }
                }
            }
        }
        pairs
            .entry(canonical(key.clone(), image))
            .or_default()
            .insert(id);
    }
    // Each key that stands for a pair stands for exactly {k, mirror(k)}, so
    // that no two pairs share a key; and it is one of the two.
    for (key, ids) in &pairs {
        let first = &graph.keys[*ids.first().expect("a pair")];
        let expected = BTreeSet::from([graph.ids[first], graph.ids[&mirror(config, first)]]);
        assert_eq!(ids, &expected, "a key stands for other than one pair");
        assert!(ids.iter().any(|&i| graph.keys[i] == *key));
    }
    assert_eq!(
        2 * pairs.len() as u64 - counted.fixed,
        graph.keys.len() as u64,
        "the pairs do not cover the graph"
    );
    // The quotient's keys are exactly the pairs' keys.
    let (keys, parents) = kept;
    let folded: BTreeSet<&Vec<u8>> = keys.iter().collect();
    assert_eq!(folded.len(), keys.len());
    assert!(
        folded.iter().copied().eq(pairs.keys()),
        "the quotient's keys are not the pairs' keys"
    );
    // Every key of the quotient: its path, unfolded to the callers' own
    // names, replays in the original semantics into its pair, and is an
    // original path of the graph.
    let mut exchanged_paths = 0;
    let mut example = None;
    for id in 0..keys.len() as u32 {
        let mut edges = Vec::new();
        let mut at = id;
        while parents[at as usize].0 != EMPTY {
            edges.push(parents[at as usize].1);
            at = parents[at as usize].0;
        }
        edges.reverse();
        let unfolded = unfold(config, &edges, step_adm);
        if unfolded != edges {
            exchanged_paths += 1;
            if example.is_none() {
                let callers = |path: &[u64]| -> Vec<Actor> {
                    path.iter().map(|&edge| all[unpack(edge).0]).collect()
                };
                example = Some(format!(
                    "  the first such path: quotient {:?}, unfolded {:?}\n{}",
                    callers(&edges),
                    callers(&unfolded),
                    trace_with(config, &unfolded, step_adm)
                ));
            }
        }
        let mut state = encode(config, &initial(config));
        for &edge in &unfolded {
            state = replay(config, &state, edge, step_adm);
            assert!(
                graph.ids.contains_key(&state),
                "an unfolded path left the graph"
            );
        }
        let key = &keys[id as usize];
        assert!(
            state == *key || state == mirror(config, key),
            "an unfolded path ends outside its pair"
        );
    }
    assert!(
        exchanged_paths > 0,
        "no unfolded path needed the exchange: the unfolding is not exercised"
    );
    println!(
        "PAIRS {} ({mode:?}): {} states in {} pairs ({} their own mirror); every state's expansion is the \
         exchange of its mirror's; {} keys of the quotient, each reached by a path that unfolds into an \
         original path ({exchanged_paths} of them through the exchange)\n{}",
        config.name,
        graph.keys.len(),
        pairs.len(),
        counted.fixed,
        keys.len(),
        example.expect("a path through the exchange")
    );
    counted
}

/// Holds the symmetry quotient to the exchange on one configuration, with the
/// real admission step, alone and composed with the admission quotient: the
/// whole original graph, the whole admission quotient, each against its
/// mirror and against the symmetry quotient's keys and paths; and the
/// reports, whose counts must be the pairs' counts and whose properties and
/// branches must be the original's.
fn symmetry_differential(config: &Config) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_symmetric(config);
    let all = actors(config);
    let n = u64::from(config.admissions);
    let lines = |text: &str| -> Vec<String> {
        section(text, "  P1:", "  state key fields")
            .into_iter()
            .filter(|line| !line.starts_with("  L1: "))
            .map(str::to_string)
            .collect()
    };
    for mode in [Mode::Full, Mode::Quotient] {
        let graph = whole_graph(config, |key| match mode {
            Mode::Full => {
                let (next, state) = successors(config, &all, key);
                (items(next.into_iter().map(Next::Edge).collect()), state)
            }
            Mode::Quotient => {
                let (next, state) = expand(config, &all, key, step_adm, Mode::Quotient);
                (items(next), state)
            }
        });
        let (report, reference) = match mode {
            Mode::Full => (explore(config), None),
            Mode::Quotient => {
                let (report, tally) = explore_experimental(config, step_adm, Mode::Quotient);
                (report, Some(tally))
            }
        };
        let (folded, tally, mirrors, kept) = explore_symmetric_kept(config, step_adm, mode, true);
        let counted = pairs_of(config, mode, &graph, &kept.expect("kept"));
        println!("{}\n{}", report.text, folded.text);
        assert_eq!(report.verdict, Verdict::Pass);
        assert_eq!(folded.verdict, Verdict::Pass);
        // The quotient's own count of keys that are their own mirror, and
        // their shares, are the graph's.
        assert_eq!(mirrors, counted);
        // The reference's counts are the pairs': a pair counts twice, a key
        // that is its own mirror once.
        let [states, expanded, transitions, faulted, no_step] =
            numbers(&report.text, "  states ")[..]
        else {
            panic!("the reference's states line");
        };
        let [nonfault, ends] = numbers(&report.text, "  L1: ")[..] else {
            panic!("the reference's L1 line");
        };
        let double = |folded: u64, fixed: u64| 2 * folded - fixed;
        assert_eq!(states, graph.keys.len() as u64);
        assert_eq!(expanded, states);
        assert_eq!(tally.expanded, tally.states);
        assert_eq!(states, double(tally.states, mirrors.fixed));
        assert_eq!(
            transitions,
            double(tally.transitions, mirrors.fixed_transitions)
        );
        assert_eq!(
            faulted,
            double(tally.fault_edges, mirrors.fixed_fault_edges)
        );
        assert_eq!(
            nonfault,
            double(tally.nonfault_edges, mirrors.fixed_nonfault_edges)
        );
        assert_eq!(no_step, double(tally.no_step, mirrors.fixed_no_step));
        assert_eq!(ends, no_step);
        if let Some(reference) = &reference {
            assert_eq!(
                reference.evaluations,
                double(tally.evaluations, mirrors.fixed_evaluations)
            );
            assert_eq!(
                reference.admitted,
                double(tally.admitted, mirrors.fixed_admitted)
            );
            assert_eq!(reference.evaluations, if n > 0 { states } else { 0 });
            assert_eq!(reference.taken, tally.taken, "the branches taken differ");
        }
        assert_eq!(
            numbers(&folded.text, "  L1: "),
            [tally.nonfault_edges, tally.no_step],
            "the quotient's L1 line"
        );
        assert_eq!(
            lines(&report.text),
            lines(&folded.text),
            "properties or branches differ"
        );
        assert_eq!(
            section(&report.text, "  state key fields", "\u{0}"),
            section(&folded.text, "  state key fields", "\u{0}")
        );
        println!(
            "DIFFERENTIAL {} ({mode:?}): reference {states} states, {transitions} transitions ({faulted} with a \
             fault, {nonfault} without), {no_step} with no enabled step; symmetry quotient {} keys ({} their own \
             mirror), {} transitions ({} with a fault, {} without), {} with no enabled step, {} admission \
             evaluations ({} admitted). Matched: every state's expansion to its mirror's under the exchange, \
             every key of the quotient to exactly one pair, every count to the pairs' counts, every property \
             line, every branch, and every key's path unfolded and replayed in the original semantics.",
            config.name,
            tally.states,
            mirrors.fixed,
            tally.transitions,
            tally.fault_edges,
            tally.nonfault_edges,
            tally.no_step,
            tally.evaluations,
            tally.admitted
        );
    }
}

#[test]
fn gc_symmetry_x2_matches_the_original() {
    symmetry_differential(&X2);
}

/// A forcing of a world, named.
type Forcing = (&'static str, fn(&mut World));

/// S8 where a property fails: X-2 violates nothing, so its states are
/// forced, through private state, into the per-caller properties and the
/// admission's (forced states are not called reachable). For every state of
/// X-2 and each forcing, the forced state and its mirror violate properties
/// of the same names, in `state_properties` and in `admission_properties`.
#[test]
fn gc_symmetry_properties_follow_the_exchange() {
    let config = X2;
    let all = actors(&config);
    let graph = whole_graph(&config, |key| {
        let (next, state) = successors(&config, &all, key);
        (items(next.into_iter().map(Next::Edge).collect()), state)
    });
    let forcings: [Forcing; 6] = [
        ("G1 told, record absent (P7)", |w| w.ghost.told |= bit(0)),
        ("G2 told, record absent (P7)", |w| w.ghost.told |= bit(1)),
        ("G1 issued, written, uncorrected (P8)", |w| {
            w.ghost.issued |= bit(0);
            w.ghost.told &= !bit(0);
            w.rows.push(SimRow::Gating(0));
        }),
        ("G2 issued, written, uncorrected (P8)", |w| {
            w.ghost.issued |= bit(1);
            w.ghost.told &= !bit(1);
            w.rows.push(SimRow::Gating(1));
        }),
        ("G1's explanation, unacknowledged (P12, P15)", |w| {
            w.rows.push(SimRow::Explanation(2));
        }),
        ("G2's correction, with G2's record (P8 cleared)", |w| {
            w.ghost.issued |= bit(1);
            w.rows.push(SimRow::Gating(1));
            w.rows.push(SimRow::Correction(1, Exit::ReturnedUncertain));
        }),
    ];
    let named = |w: &World| -> (Vec<&'static str>, Vec<&'static str>) {
        let (mut state, mut admission) = (Obs::default(), Obs::default());
        state_properties(&config, w, &mut state);
        admission_properties(&config, w, &mut admission);
        (names(&state.violations), names(&admission.violations))
    };
    let mut violated: BTreeMap<&str, u64> = BTreeMap::new();
    for key in &graph.keys {
        for (name, force) in &forcings {
            let mut w = decode(&config, key);
            force(&mut w);
            let forced = encode(&config, &w);
            let image = mirror(&config, &forced);
            let (own, other) = (
                named(&decode(&config, &forced)),
                named(&decode(&config, &image)),
            );
            assert_eq!(own, other, "{name}: a forced state and its mirror differ");
            if !own.0.is_empty() || !own.1.is_empty() {
                *violated.entry(name).or_default() += 1;
            }
        }
    }
    println!(
        "FORCED X-2: {} states x {} forcings; forced states violating a property, by forcing: {violated:?}; \
         each forced state and its mirror violate properties of the same names",
        graph.keys.len(),
        forcings.len()
    );
    for (name, _) in &forcings[..5] {
        assert!(
            violated.get(name).is_some_and(|&n| n > 0),
            "{name} never violates"
        );
    }
}

#[test]
fn gc_symmetry_sym1_matches_the_original() {
    symmetry_differential(&SYM1);
}

// ---- experimental: L1 under the exchange, on finite graphs --------------------------------------

/// A finite graph explored from state 0 as the explorers explore, breadth
/// first, each successor kept as itself or, folded, as the smaller of
/// itself and its image under `p`; then Kahn's algorithm over what was kept.
/// The states kept, and whether a cycle was found.
fn explore_finite(edges: &[Vec<u32>], p: &[u32], fold: bool) -> (Vec<u32>, bool) {
    let keep = |s: u32| if fold { s.min(p[s as usize]) } else { s };
    let mut ids: BTreeMap<u32, u32> = BTreeMap::from([(0, 0)]);
    let mut states = vec![0u32];
    let (mut from, mut to) = (vec![0u64], Vec::new());
    let mut at = 0;
    while at < states.len() {
        for &t in &edges[states[at] as usize] {
            let t = keep(t);
            let next = states.len() as u32;
            let id = *ids.entry(t).or_insert(next);
            if id == next {
                states.push(t);
            }
            to.push(id);
        }
        from.push(to.len() as u64);
        at += 1;
    }
    let (removed, _) = kahn(states.len(), &from, &to);
    let cyclic = removed < states.len();
    (states, cyclic)
}

/// Whether the part of a finite graph reachable from state 0 has a cycle, by
/// depth-first search, independently of Kahn's algorithm.
fn reachable_cycle(edges: &[Vec<u32>]) -> bool {
    let mut colour = vec![0u8; edges.len()];
    let mut stack = vec![(0usize, 0usize)];
    colour[0] = 1;
    while let Some((s, k)) = stack.pop() {
        if k < edges[s].len() {
            stack.push((s, k + 1));
            let t = edges[s][k] as usize;
            match colour[t] {
                1 => return true,
                0 => {
                    colour[t] = 1;
                    stack.push((t, 0));
                }
                _ => {}
            }
        } else {
            colour[s] = 2;
        }
    }
    false
}

/// A graph from a list of edges, closed under the involution `p`.
fn closed(n: usize, p: &[u32], list: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut sets = vec![BTreeSet::new(); n];
    for &(u, v) in list {
        sets[u as usize].insert(v);
        sets[p[u as usize] as usize].insert(p[v as usize]);
    }
    sets.into_iter()
        .map(|set| set.into_iter().collect())
        .collect()
}

/// S10 on finite graphs: whether the part reachable from 0 has a cycle is
/// the same in a graph closed under an involution fixing 0 and in its
/// quotient, explored and judged as the explorers judge L1. Deliberate
/// cycles: a state that steps into its mirror (a self-loop in the quotient),
/// and a cycle that closes only after two laps of the quotient's; and
/// acyclic graphs whose folding merges branches. Then 20,000 generated
/// graphs.
#[test]
fn gc_symmetry_l1_on_finite_graphs() {
    let check = |n: usize, p: &[u32], list: &[(u32, u32)]| -> bool {
        let edges = closed(n, p, list);
        let truth = reachable_cycle(&edges);
        let (reached, full) = explore_finite(&edges, p, false);
        let (kept, folded) = explore_finite(&edges, p, true);
        assert_eq!(full, truth, "Kahn's algorithm differs from the search");
        assert_eq!(
            folded, truth,
            "the quotient's L1 differs: {list:?} under {p:?}"
        );
        let pairs: BTreeSet<u32> = reached.iter().map(|&s| s.min(p[s as usize])).collect();
        let kept: BTreeSet<u32> = kept.into_iter().collect();
        assert_eq!(
            kept, pairs,
            "the quotient keeps other than one state per pair"
        );
        truth
    };
    // A state that steps into its mirror: 1 -> 2 = p(1), so 2 -> 1; the
    // quotient keeps a self-loop at 1.
    assert!(check(3, &[0, 2, 1], &[(0, 1), (1, 2)]));
    // A cycle that closes after two laps: 1 -> 4 -> 2 -> 3 -> 1, folded
    // 1 -> 3 -> 1.
    assert!(check(5, &[0, 2, 1, 4, 3], &[(0, 1), (1, 4), (4, 2)]));
    // A fixed state's self-loop.
    assert!(check(2, &[0, 1], &[(0, 1), (1, 1)]));
    // Acyclic, mirrored branches.
    assert!(!check(5, &[0, 2, 1, 4, 3], &[(0, 1), (1, 3)]));
    // Acyclic, folding merges two branches into a double edge.
    assert!(!check(4, &[0, 2, 1, 3], &[(0, 1), (1, 3)]));
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut cyclic, mut acyclic) = (0, 0);
    for _ in 0..20_000 {
        let n = 2 + (next() % 8) as usize;
        let mut p: Vec<u32> = (0..n as u32).collect();
        let mut free: Vec<u32> = (1..n as u32).collect();
        while free.len() >= 2 {
            let a = free.swap_remove((next() % free.len() as u64) as usize);
            let b = free.swap_remove((next() % free.len() as u64) as usize);
            if next() % 3 != 0 {
                p[a as usize] = b;
                p[b as usize] = a;
            }
        }
        let list: Vec<(u32, u32)> = (0..n as u32)
            .flat_map(|u| (0..n as u32).map(move |v| (u, v)))
            .filter(|_| next() % (n as u64 + 1) == 0)
            .collect();
        if check(n, &p, &list) {
            cyclic += 1;
        } else {
            acyclic += 1;
        }
    }
    println!("L1 FINITE: 20,000 generated graphs, {cyclic} with a reachable cycle, {acyclic} without; each judged alike in the graph and its quotient");
    assert!(cyclic > 1000 && acyclic > 1000, "too few of one kind");
}

// ---- experimental: the symmetry quotient's negative cases ---------------------------------------

/// A defective admission fails `property` in the original semantics and in
/// the symmetry quotient, alone and composed with the admission quotient,
/// with shortest traces of one length; each of the quotients' traces,
/// unfolded to the callers' own names, replays in the original semantics to
/// `property`.
fn symmetry_negative_case(config: &Config, adm: AdmStep, property: &'static str) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for mode in [Mode::Full, Mode::Quotient] {
        let (reference, reference_tally) = explore_experimental(config, adm, mode);
        let (folded, tally, mirrors) = explore_symmetric(config, adm, mode);
        println!("{}\n{}", reference.text, folded.text);
        for report in [&reference, &folded] {
            assert_eq!(report.verdict, Verdict::Fail, "{}", report.text);
            assert_eq!(report.failed, BTreeSet::from([property]), "{}", report.text);
        }
        let (expected, trace) = (
            reference_tally.trace.expect("a trace"),
            tally.trace.expect("a trace"),
        );
        let path = mirrors.folded.expect("a quotient path");
        assert_eq!(
            expected.len(),
            trace.len(),
            "shortest traces differ in length"
        );
        assert_eq!(unfold(config, &path, adm), trace);
        assert!(
            last_step_violations(config, &trace, adm).contains(&property),
            "the unfolded trace does not replay to its violation"
        );
        println!(
            "NEGATIVE {} ({mode:?}): FAIL {property} in the reference and the symmetry quotient; shortest traces \
             {} steps; the quotient's path unfolds into an original trace that replays to {property}",
            config.name,
            trace.len()
        );
    }
}

#[test]
fn gc_symmetry_negative_p1_admission_ignores_unapplied() {
    symmetry_negative_case(&X2, adm_ignoring_unapplied, "P1");
}

#[test]
fn gc_symmetry_negative_e9_admission_ignores_the_latch() {
    symmetry_negative_case(&X2, adm_ignoring_the_latch, "E9");
}

#[test]
fn gc_symmetry_negative_e2_admission_views_twice() {
    symmetry_negative_case(&X2, adm_viewing_twice, "E2");
}

/// A deliberate cycle in the model: an admission that keeps its counter
/// makes a non-fault self-loop; the symmetry quotient reports L1 as the
/// original does, and its path unfolds into an original path.
#[test]
fn gc_symmetry_finds_the_original_l1_cycle() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (reference, _) = explore_experimental(&X2, adm_keeping_its_counter, Mode::Full);
    let (folded, tally, mirrors) = explore_symmetric(&X2, adm_keeping_its_counter, Mode::Full);
    println!("{}\n{}", reference.text, folded.text);
    for report in [&reference, &folded] {
        assert_eq!(report.verdict, Verdict::Fail, "{}", report.text);
        assert_eq!(report.failed, BTreeSet::from(["L1"]), "{}", report.text);
    }
    let path = mirrors.folded.expect("a quotient path");
    assert_eq!(
        unfold(&X2, &path, adm_keeping_its_counter),
        tally.trace.expect("a trace")
    );
}

/// The real admission, refusing whenever G1 alone was told Committed: a
/// deliberately asymmetric step, for the guard's negative case only.
fn adm_favouring_g2(config: &Config, w: &mut World, obs: &mut Obs) -> bool {
    admit_by(config, w, obs, |w, view| {
        view.admits() && w.table.debt == 0 && w.table.h <= w.table.a && w.ghost.told != bit(0)
    })
}

/// The guard: an asymmetric step, met in exploration, aborts the run with
/// no verdict.
#[test]
#[should_panic(
    expected = "symmetry inapplicable: an expansion is not the exchange of its mirror's"
)]
fn gc_symmetry_refuses_an_asymmetric_step() {
    let _ = explore_symmetric(&X2, adm_favouring_g2, Mode::Full);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: F-G seeds a conflict for G2 alone")]
fn gc_symmetry_refuses_f_g() {
    let _ = explore_symmetric(&FG, step_adm, Mode::Full);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: F-1 is not a configuration this experiment showed symmetric"
)]
fn gc_symmetry_refuses_a_configuration_not_shown_symmetric() {
    let _ = explore_symmetric(&F1, step_adm, Mode::Full);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: F-3 is not a configuration this experiment showed symmetric"
)]
fn gc_symmetry_refuses_another_shape_under_a_known_name() {
    let config = Config {
        blanket_uncertain: true,
        ..F3
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: F-3 raises its cap")]
fn gc_symmetry_refuses_a_raised_cap() {
    let config = Config {
        cap: FULL_CAP + 1,
        ..F3
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: X-2 has acknowledgements")]
fn gc_symmetry_refuses_acknowledgements() {
    let config = Config {
        acks: 1,
        ack_attempts: 1,
        ..X2
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Full);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: X-4 has not exactly two gating callers")]
fn gc_symmetry_refuses_other_callers() {
    let _ = explore_symmetric(&X4, step_adm, Mode::Full);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: A2's kept bits")]
fn gc_symmetry_refuses_kept_bits() {
    let _ = explore_symmetric(&X2, adm_keeping_bits, Mode::Full);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: an acknowledgement")]
fn gc_symmetry_refuses_an_acknowledgement_row() {
    let _ = mirror_row(SimRow::Acknowledgement {
        set: SetFacts {
            frontier: 0,
            head: 3,
            top: 3,
            count: 1,
            digest: bytes(0b1000, 0, 0),
        },
        nonce: [0; 16],
    });
}

// ---- experimental: F-PT and F-B under the exchange ----------------------------------------------
//
// P3-C11-DR01-D2A1-CLOSURE-PREP-01, a bounded experiment. It is not the
// contract's explorer and not evidence for the contract; `explore`,
// `explore_experimental` and `explore_symmetric_kept` above are unchanged.
//
// F-PT and F-B are F-3 with one change each, and each change touches facts
// only. Under A4 the drop paths record a blanket Uncertain, from the members
// and the ticket alone; the batch writer offers both facts in one append,
// whose members are compared with each other only by fact identity. Neither
// reads or writes a value that names a gating caller, so the exchange of G1
// and G2 stays an automorphism of each graph and of its admission quotient.
// Both join the allow-list with their constants unchanged. The fixtures
// below hold the exchange to the original on small graphs of the same
// shapes, and each fixture must take the steps it is there to test. F-B's
// own facts with a fault exceed the differential's bound, so the batch has
// two fixtures: F-B's facts, which name each member as the conflict, and a
// clean batch with a fault, which reaches its inserts.

/// F-PT's change on SYM-1's bounds: actor change A4, two gating callers, one
/// conflicting fact whose Prepared record ENV supplies, one fault and one
/// admission.
const SYM_PT: Config = Config {
    name: "SYM-PT",
    blanket_uncertain: true,
    ..SYM1
};

/// F-B's facts with two gating callers and one admission: one writer offers
/// both facts as one batch, both conflicting, F1's Prepared record supplied
/// by ENV. No fault.
const SYM_B: Config = default_run(Config {
    name: "SYM-B",
    conflict: [true, true],
    env: true,
    batch: true,
    admissions: 1,
    gating: 2,
    ..BASE
});

/// A clean batch of two with two gating callers, one fault and one
/// admission: the batch reaches its inserts.
const SYM_BI: Config = default_run(Config {
    name: "SYM-BI",
    batch: true,
    faults: 1,
    admissions: 1,
    gating: 2,
    ..BASE
});

/// The allow-list is exactly these shapes. F-PT is F-3 with A4 and nothing
/// else, F-B is F-3 with one batch writer and nothing else, and each keeps
/// the contract's bounds and caps (section 8.3); the fixtures keep the
/// default caps and have an admission and both callers, and all but SYM-B a
/// fault.
#[test]
fn gc_symmetry_allow_list_is_exact() {
    let names: Vec<&str> = SYMMETRIC.iter().map(|config| config.name).collect();
    assert_eq!(
        names,
        ["X-2", "SYM-1", "F-3", "SYM-PT", "SYM-B", "SYM-BI", "F-PT", "F-B"]
    );
    for config in [F3, FPT, FB] {
        assert_eq!(
            (
                config.facts,
                config.conflict,
                config.env,
                config.faults,
                config.admissions,
                config.gating,
                config.cap,
                config.time,
            ),
            (2, [true, true], true, 2, 2, 2, FULL_CAP, FULL_TIME),
            "{}",
            config.name
        );
    }
    let as_f3 = |config: Config| Config {
        name: "F-3",
        blanket_uncertain: false,
        batch: false,
        ..config
    };
    assert_eq!((FPT.blanket_uncertain, FPT.batch), (true, false));
    assert_eq!((FB.blanket_uncertain, FB.batch), (false, true));
    assert_eq!(shape(&as_f3(FPT)), shape(&F3));
    assert_eq!(shape(&as_f3(FB)), shape(&F3));
    assert_eq!(
        shape(&Config {
            name: "SYM-1",
            blanket_uncertain: false,
            ..SYM_PT
        }),
        shape(&SYM1)
    );
    for config in [SYM_PT, SYM_B, SYM_BI] {
        assert_eq!((config.cap, config.time), (DEFAULT_CAP, DEFAULT_TIME));
        assert!(config.admissions > 0 && config.gating == 2);
    }
    assert_eq!((SYM_PT.faults, SYM_B.faults, SYM_BI.faults), (1, 0, 1));
    assert_eq!(
        (SYM_B.batch, SYM_B.conflict, SYM_B.env, SYM_BI.batch),
        (true, FB.conflict, FB.env, true)
    );
}

#[test]
fn gc_symmetry_sym_pt_matches_the_original() {
    symmetry_differential(&SYM_PT);
}

#[test]
fn gc_symmetry_sym_b_matches_the_original() {
    symmetry_differential(&SYM_B);
}

#[test]
fn gc_symmetry_sym_bi_matches_the_original() {
    symmetry_differential(&SYM_BI);
}

/// The whole original graph of `config`, every actor scheduled.
fn original_graph(config: &Config) -> Graph {
    let all = actors(config);
    whole_graph(config, |key| {
        let (next, state) = successors(config, &all, key);
        (items(next.into_iter().map(Next::Edge).collect()), state)
    })
}

/// What a fixture's original graph shows of the exchange, whatever its
/// shape, each required: states that are not their own mirror; a state in
/// which G1 alone was told Committed, and one in which G2 alone was; and
/// states with no enabled step, each judged terminal (D1). Returns those
/// counts and the fault edges.
fn exchange_exercised(config: &Config, graph: &Graph) -> (u64, u64, u64) {
    let (mut moved, mut told, mut faulted, mut ends) = (0u64, [false; 2], 0u64, 0u64);
    for (id, key) in graph.keys.iter().enumerate() {
        let w = decode(config, key);
        moved += u64::from(mirror(config, key) != *key);
        if w.ghost.told == bit(0) {
            told[0] = true;
        }
        if w.ghost.told == bit(1) {
            told[1] = true;
        }
        faulted += graph.items[id].iter().filter(|item| item.fault).count() as u64;
        if graph.items[id].is_empty() {
            assert!(
                terminal(config, &w),
                "a state with no enabled step is not terminal"
            );
            ends += 1;
        }
    }
    assert!(moved > 0, "{}: every state is its own mirror", config.name);
    assert_eq!(
        told,
        [true, true],
        "{}: a caller is never told Committed alone",
        config.name
    );
    assert!(
        ends > 0,
        "{}: no state without an enabled step",
        config.name
    );
    (moved, faulted, ends)
}

/// SYM-PT takes the steps that only A4 takes, from states that are not their
/// own mirror. A writer's drop path holding a Counted ticket that says Not
/// written records Uncertain (without A4: Refused or Conflict, by the
/// ticket). Settlement's drop path, holding no ticket, records Uncertain
/// over another class (without A4: nothing). Each needs the fault.
#[test]
fn gc_symmetry_sym_pt_takes_the_a4_steps() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = SYM_PT;
    let all = actors(&config);
    let graph = original_graph(&config);
    let (moved, faulted, ends) = exchange_exercised(&config, &graph);
    let (mut writer, mut settlement) = (0u64, 0u64);
    for (id, key) in graph.keys.iter().enumerate() {
        if mirror(&config, key) == *key {
            continue;
        }
        let before = decode(&config, key);
        for item in &graph.items[id] {
            let after = decode(&config, &item.key);
            match all[unpack(item.edge).0] {
                Actor::Fw(i) => {
                    let fw = &before.fw[i as usize];
                    let not_written =
                        fw.pos == FwPos::Dropping
                            && fw.ticket.as_ref().and_then(Ticket::counted).is_some_and(
                                |counted| counted.persistence == Persistence::NotWritten,
                            );
                    let uncertain = fw
                        .members
                        .iter()
                        .all(|&f| after.table.class[f as usize] == Class::Uncertain);
                    writer += u64::from(not_written && uncertain);
                }
                Actor::Se => {
                    if let SePos::Dropping(f) = before.se.pos {
                        let f = f as usize;
                        settlement += u64::from(
                            before.se.ticket.is_none()
                                && before.table.class[f] != Class::Uncertain
                                && after.table.class[f] == Class::Uncertain,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    println!(
        "EXERCISED {}: {} states ({moved} not their own mirror), {faulted} fault edges, {ends} with no enabled \
         step, each terminal; both callers told Committed alone; from states not their own mirror, {writer} \
         writer drops recording a Not-written Counted ticket as Uncertain (A4) and {settlement} settlement \
         drops without a ticket recording Uncertain over another class (A4)",
        config.name,
        graph.keys.len()
    );
    assert!(faulted > 0, "no fault is taken");
    assert!(writer > 0, "A4's writer drop path is not taken");
    assert!(settlement > 0, "A4's settlement drop path is not taken");
}

/// The batch writer's steps in one original graph, from states that are not
/// their own mirror: appends whose result names member 1 or member 2 as the
/// conflict; records of the named member as Conflict beside the other as
/// Refused; appends whose ledger gained both members, or the first alone;
/// the writer's fault edges; and states with E6's history marked.
#[derive(Debug, Default)]
struct BatchSteps {
    named: [u64; 2],
    mixed: u64,
    both: u64,
    first_alone: u64,
    writer_faults: u64,
    marked: u64,
}

fn batch_steps(config: &Config, graph: &Graph) -> BatchSteps {
    let all = actors(config);
    assert_eq!(all[0], Actor::Fw(0));
    assert!(!all.contains(&Actor::Fw(1)), "one writer for the batch");
    let mut steps = BatchSteps::default();
    for (id, key) in graph.keys.iter().enumerate() {
        if mirror(config, key) == *key {
            continue;
        }
        let before = decode(config, key);
        assert_eq!(before.fw.len(), 1);
        assert_eq!(before.fw[0].members, [0, 1]);
        steps.marked += u64::from(before.ghost.conflicted_e6 != 0);
        for item in &graph.items[id] {
            if all[unpack(item.edge).0] != Actor::Fw(0) {
                continue;
            }
            steps.writer_faults += u64::from(item.fault);
            let after = decode(config, &item.key);
            let (from, to) = (before.fw[0].pos, after.fw[0].pos);
            if from == FwPos::Armed {
                let gained = &after.rows[before.rows.len()..];
                steps.both += u64::from(gained == [SimRow::Fact(0), SimRow::Fact(1)]);
                steps.first_alone += u64::from(gained == [SimRow::Fact(0)]);
            }
            if let (FwPos::Armed, FwPos::Got(Held::Counted(counted))) = (from, to) {
                if let Some(member) = counted.conflict_member {
                    if counted.persistence == Persistence::NotWritten {
                        steps.named[member as usize] += 1;
                    }
                }
            }
            if let (FwPos::Got(Held::Counted(counted)), FwPos::Recorded) = (from, to) {
                if let Some(member) = counted.conflict_member {
                    let (named, other) = (member as usize, 1 - member as usize);
                    steps.mixed += u64::from(
                        after.table.class[named] == Class::Conflict
                            && after.table.class[other] == Class::Refused,
                    );
                }
            }
        }
    }
    steps
}

/// SYM-B takes F-B's batch steps from states that are not their own mirror:
/// the one writer's append names each member as the conflict (member 2 only
/// after member 1 was classified for insert, so after ENV), recording names
/// that member Conflict and the other Refused, and E6's history is marked.
#[test]
fn gc_symmetry_sym_b_takes_the_batch_steps() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = SYM_B;
    let graph = original_graph(&config);
    let (moved, faulted, ends) = exchange_exercised(&config, &graph);
    let steps = batch_steps(&config, &graph);
    println!(
        "EXERCISED {}: {} states ({moved} not their own mirror), {faulted} fault edges, {ends} with no enabled \
         step, each terminal; both callers told Committed alone; from states not their own mirror: {steps:?}",
        config.name,
        graph.keys.len()
    );
    assert!(steps.named[0] > 0, "the batch never names its first member");
    assert!(
        steps.named[1] > 0,
        "the batch never names its second member"
    );
    assert!(
        steps.mixed > 0,
        "no record of a named member beside an unnamed one"
    );
    assert!(steps.marked > 0, "E6's history is never marked");
}

/// SYM-BI takes the batch's inserts from states that are not their own
/// mirror: an append writes both members, and an append whose second insert
/// unwinds leaves the first alone as one of its histories; the batch writer
/// takes the fault.
#[test]
fn gc_symmetry_sym_bi_takes_the_batch_inserts() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = SYM_BI;
    let graph = original_graph(&config);
    let (moved, faulted, ends) = exchange_exercised(&config, &graph);
    let steps = batch_steps(&config, &graph);
    println!(
        "EXERCISED {}: {} states ({moved} not their own mirror), {faulted} fault edges, {ends} with no enabled \
         step, each terminal; both callers told Committed alone; from states not their own mirror: {steps:?}",
        config.name,
        graph.keys.len()
    );
    assert!(steps.both > 0, "no batch append writes both members");
    assert!(
        steps.first_alone > 0,
        "no batch history holds the first member alone"
    );
    assert!(steps.writer_faults > 0, "the batch writer takes no fault");
}

/// Counterexamples on the new shapes: a defective admission fails its
/// property in the original semantics and in the symmetry quotient, alone and
/// composed, and the quotients' paths unfold into original traces that replay
/// to it. On SYM-B an admission that ignores `unapplied` fails nothing: with
/// no fault every result of the batch names a conflict, whose explanation
/// raises H above A for good (no acknowledgement), so no admission is made
/// while a result is unrecorded; one that ignores recovery fails P12 there.
#[test]
fn gc_symmetry_negative_p1_on_sym_pt() {
    symmetry_negative_case(&SYM_PT, adm_ignoring_unapplied, "P1");
}

#[test]
fn gc_symmetry_negative_p12_on_sym_b() {
    symmetry_negative_case(&SYM_B, adm_ignoring_recovery, "P12");
}

#[test]
fn gc_symmetry_negative_p1_on_sym_bi() {
    symmetry_negative_case(&SYM_BI, adm_ignoring_unapplied, "P1");
}

/// The guard on the new shapes: an asymmetric step, met in exploration,
/// aborts the run with no verdict.
#[test]
#[should_panic(
    expected = "symmetry inapplicable: an expansion is not the exchange of its mirror's"
)]
fn gc_symmetry_refuses_an_asymmetric_step_on_sym_pt() {
    let _ = explore_symmetric(&SYM_PT, adm_favouring_g2, Mode::Quotient);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: an expansion is not the exchange of its mirror's"
)]
fn gc_symmetry_refuses_an_asymmetric_step_on_sym_b() {
    let _ = explore_symmetric(&SYM_B, adm_favouring_g2, Mode::Quotient);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: an expansion is not the exchange of its mirror's"
)]
fn gc_symmetry_refuses_an_asymmetric_step_on_sym_bi() {
    let _ = explore_symmetric(&SYM_BI, adm_favouring_g2, Mode::Quotient);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: F-3 is not a configuration this experiment showed symmetric"
)]
fn gc_symmetry_refuses_a_batch_under_f3s_name() {
    let config = Config { batch: true, ..F3 };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: F-PT is not a configuration this experiment showed symmetric"
)]
fn gc_symmetry_refuses_f_pt_with_a_lower_fault_budget() {
    let config = Config { faults: 1, ..FPT };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(
    expected = "symmetry inapplicable: F-B is not a configuration this experiment showed symmetric"
)]
fn gc_symmetry_refuses_f_b_with_another_actor_change() {
    let config = Config {
        blanket_uncertain: true,
        ..FB
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: F-PT raises its cap")]
fn gc_symmetry_refuses_f_pt_with_a_raised_cap() {
    let config = Config {
        cap: FULL_CAP + 1,
        ..FPT
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

#[test]
#[should_panic(expected = "symmetry inapplicable: F-B raises its cap")]
fn gc_symmetry_refuses_f_b_with_a_raised_time_cap() {
    let config = Config {
        time: FULL_TIME + Duration::from_secs(1),
        ..FB
    };
    let _ = explore_symmetric(&config, step_adm, Mode::Quotient);
}

/// One full configuration in the symmetry quotient composed with the
/// admission observer quotient, required to PASS.
fn symmetric_full_exploration(config: &Config) {
    let (report, _, _) = explore_symmetric(config, step_adm, Mode::Quotient);
    println!("{}", report.text);
    assert_eq!(
        report.verdict,
        Verdict::Pass,
        "failed: {:?}\n{}",
        report.failed,
        report.text
    );
}

/// The full F-PT, in the symmetry quotient composed with the admission
/// observer quotient. Selectable only by name; not run by this mission.
#[test]
#[ignore = "experimental gating-caller symmetry quotient: run only when an Architect mission names it"]
fn gc_symmetry_fpt() {
    symmetric_full_exploration(&FPT);
}

/// The full F-B, in the symmetry quotient composed with the admission
/// observer quotient. Selectable only by name; not run by this mission.
#[test]
#[ignore = "experimental gating-caller symmetry quotient: run only when an Architect mission names it"]
fn gc_symmetry_fb() {
    symmetric_full_exploration(&FB);
}

// ---- named-property witnesses for controls C1 and C25 -------------------------------------------

/// One scheduled step of a witness: the actor; every choice its oracle must
/// take, in order; and the branch labels the step must show on any core, or
/// `None` at the step whose outcome the control changes.
type Scheduled = (Actor, &'static [u8], Option<&'static [&'static str]>);

/// A fixed schedule that makes one control's named property fail, where the
/// explorer reports another property first (contract 7.5, controls C1 and
/// C25). Experimental: the form the AD-1-R1 draft proposes, not adopted.
struct Witness {
    control: &'static str,
    config: Config,
    steps: &'static [Scheduled],
    /// The named property, and the step at which it must be observed: in
    /// that step's own violations, or in the state after it.
    named: &'static str,
    at: usize,
    in_step: bool,
    /// At that step: the branch the healthy core shows, and the branch the
    /// named violation needs.
    healthy: &'static str,
    changed: &'static str,
}

/// What one replayed step showed.
struct Replayed {
    /// The step's description, as `trace` writes it.
    note: String,
    fault: bool,
    branches: Vec<String>,
    /// Violations pushed by the step itself (E4 to E9, and P1, P12, P15 at
    /// an admission) and its transition properties (E2, E9, E11).
    step: Violations,
    /// Violations of the state after it, as `successors` finds them when it
    /// expands that state: its state properties and its key's round trip.
    after: Violations,
    word: u64,
    rows: Vec<SimRow>,
}

/// The violations `successors` finds at the state of `key` before any of
/// its steps.
fn violations_at(config: &Config, key: &[u8]) -> Violations {
    let base = decode(config, key);
    let mut state = Obs::default();
    state_properties(config, &base, &mut state);
    if encode(config, &base) != key {
        state.fail("KEY", "a key does not survive decoding");
    }
    state.violations
}

/// Replays `steps` from the initial world of `config` in the original
/// semantics, as `trace` and `last_step_violations` do: each step runs on a
/// world decoded from the last key, with `step_adm` as admission, and must be
/// enabled and take exactly its scheduled choices; anything else is a fixture
/// failure and kills nothing. Unlike the explorer it does not stop at the
/// first violation: it keeps every violation of every step and of every state
/// on the path. Returns the initial state's violations and each step's.
fn replay_schedule(config: &Config, steps: &[Scheduled]) -> (Violations, Vec<Replayed>) {
    let all = actors(config);
    let mut key = encode(config, &initial(config));
    let initial_violations = violations_at(config, &key);
    let mut replayed = Vec::new();
    for (n, &(actor, choices, _)) in steps.iter().enumerate() {
        assert!(
            all.contains(&actor),
            "WITNESS FIXTURE FAILURE (kills nothing): step {} names {actor:?}, not an actor of {}",
            n + 1,
            config.name
        );
        let base = decode(config, &key);
        let mut w = decode(config, &key);
        let mut oracle = Oracle::new(choices.to_vec(), w.faults);
        let mut obs = Obs {
            trace: Some(String::new()),
            ..Obs::default()
        };
        hook::take_counts();
        let enabled = step(config, &mut w, actor, &mut oracle, &mut obs);
        let counts = hook::take_counts();
        assert!(
            enabled,
            "WITNESS FIXTURE FAILURE (kills nothing): step {} ({actor:?}) is not enabled",
            n + 1
        );
        assert_eq!(
            oracle.sequence(),
            choices,
            "WITNESS FIXTURE FAILURE (kills nothing): step {} ({actor:?}) took other choices",
            n + 1
        );
        w.faults = oracle.faults;
        transition_properties(&base, &w, &mut obs, counts);
        key = encode(config, &w);
        replayed.push(Replayed {
            note: obs.trace.unwrap_or_default(),
            fault: oracle.fault,
            branches: obs.branches,
            step: obs.violations,
            after: violations_at(config, &key),
            word: word_of(&w.core.shared),
            rows: w.rows.clone(),
        });
    }
    (initial_violations, replayed)
}

/// Runs a witness on whatever core this build holds. Every step must be
/// enabled, take exactly its choices and show its fixed branches, or the
/// replay is a fixture failure. Every step and every violation is printed.
/// With no violation the core behaved as the healthy core, and the changed
/// step must show the healthy branch. With any violation the test fails; its
/// message says whether the named property was observed where the control
/// must produce it, and lists every property violated, each separately: a
/// property the explorer reports first is never counted as the named one.
fn run_witness(witness: &Witness) {
    let config = &witness.config;
    let total = witness.steps.len();
    let (initial_violations, replayed) = replay_schedule(config, witness.steps);
    let mut lines = Vec::new();
    for (property, detail) in &initial_violations {
        lines.push(format!("initial state: {property}: {detail}"));
    }
    for (n, (record, &(actor, choices, fixed))) in replayed.iter().zip(witness.steps).enumerate() {
        println!(
            "{} WITNESS {} step {:>2}/{total} {actor:?} choices {choices:?}: {}{}",
            witness.control,
            config.name,
            n + 1,
            record.note,
            if record.fault { "   [fault]" } else { "" }
        );
        println!(
            "    branches {:?}; word after {:#x} (live {}, unapplied {})",
            record.branches,
            record.word,
            field(record.word, LIVE_SHIFT),
            field(record.word, UNAPPLIED_SHIFT)
        );
        println!("    ledger after {:?}", record.rows);
        for (place, violations) in [("step", &record.step), ("after", &record.after)] {
            for (property, detail) in violations {
                println!("    VIOLATION {place}: {property}: {detail}");
                lines.push(format!("step {} {place}: {property}: {detail}", n + 1));
            }
        }
        if let Some(fixed) = fixed {
            assert_eq!(
                record.branches,
                fixed,
                "WITNESS FIXTURE FAILURE (kills nothing): step {} shows other branches",
                n + 1
            );
        }
    }
    let at = &replayed[witness.at - 1];
    let place = if witness.in_step { &at.step } else { &at.after };
    let named_seen = at.branches.iter().any(|label| label == witness.changed)
        && place.iter().any(|(property, _)| *property == witness.named);
    if lines.is_empty() {
        assert_eq!(
            at.branches,
            [witness.healthy],
            "WITNESS: no violation, but step {} does not show the healthy core's branch",
            witness.at
        );
        println!(
            "{} WITNESS {}: {total} steps, each enabled and taking its choices; no violation; step {} shows \
             the healthy core's '{}'",
            witness.control, config.name, witness.at, witness.healthy
        );
        return;
    }
    let mut properties: Vec<&str> = initial_violations
        .iter()
        .chain(replayed.iter().flat_map(|r| r.step.iter().chain(&r.after)))
        .map(|(property, _)| *property)
        .collect();
    properties.sort_unstable();
    properties.dedup();
    let others: Vec<&str> = properties
        .iter()
        .copied()
        .filter(|property| *property != witness.named)
        .collect();
    let seen = if named_seen {
        "OBSERVED"
    } else {
        "NOT OBSERVED"
    };
    let locus = if witness.in_step {
        "at"
    } else {
        "on the world after"
    };
    panic!(
        "{} WITNESS {}: {total} steps, each enabled and taking its choices; {} violations\n\
         NAMED PROPERTY {}: {seen} {locus} step {} ({})\n\
         OTHER PROPERTIES VIOLATED, each reported separately and not counted as {}: {others:?}\n\
         every violation:\n  {}",
        witness.control,
        config.name,
        lines.len(),
        witness.named,
        witness.at,
        witness.changed,
        witness.named,
        lines.join("\n  ")
    );
}

/// C1, "the count becomes a flag", named P1 on F-1. Two writers' results are
/// Counted at once (each wait expires); the second is recorded and confirmed;
/// settlement pays its fact; then admission. On the healthy core `unapplied`
/// is still 1 for the first writer's unrecorded result, so admission is
/// refused. With a flag, the confirm cleared it: admission succeeds with a
/// result unrecorded, which P1 forbids. Under C1, E3 also fails on this
/// path, from step 4; it is reported as itself, never as P1.
const C1_WITNESS: Witness = Witness {
    control: "C1",
    config: F1,
    steps: &[
        (Actor::Fw(0), &[], Some(&[])),
        (Actor::Fw(0), &[1], Some(&["expire: Armed -> Counted"])),
        (Actor::Fw(1), &[], Some(&[])),
        (Actor::Fw(1), &[1], Some(&["expire: Armed -> Counted"])),
        (Actor::Fw(1), &[0], Some(&[])),
        (Actor::Fw(1), &[0], Some(&[])),
        (Actor::Se, &[], Some(&[])),
        (Actor::Se, &[], Some(&[])),
        (
            Actor::Se,
            &[0, 0, 0, 0],
            Some(&["append_facts: Committed (inserted)"]),
        ),
        (Actor::Se, &[], Some(&[])),
        (Actor::Adm, &[], None),
    ],
    named: "P1",
    at: 11,
    in_step: true,
    healthy: "admission: refused",
    changed: "admission: admitted",
};

/// C25, "a gating append answers Committed on `Commit::Unknown`", named P7 on
/// F-1. One gating append whose commit answers Unknown, with the record
/// absent afterwards. The healthy core answers Uncertain and keeps the record
/// in U. Answering Committed tells the caller so while the record is absent:
/// E4 fails on the step, and P7 on the world it leaves.
const C25_WITNESS: Witness = Witness {
    control: "C25",
    config: F1,
    steps: &[(Actor::Gc(0), &[0, 0, 0, 1, 0, 0], None)],
    named: "P7",
    at: 1,
    in_step: false,
    healthy: "append_gating: Refused Uncertain Storage",
    changed: "append_gating: Committed (inserted)",
};

/// On the healthy core the C1 schedule violates nothing and its admission is
/// refused. Under control C1 the test fails, with P1 at the admission.
#[test]
fn c1_witness_p1_at_a_successful_admission() {
    run_witness(&C1_WITNESS);
}

/// On the healthy core the C25 schedule violates nothing and its append is
/// Uncertain. Under control C25 the test fails, with P7 after the append.
#[test]
fn c25_witness_p7_on_the_resulting_world() {
    run_witness(&C25_WITNESS);
}

// ---- named-property witness for control C4 ------------------------------------------------------

/// One step of a C4 scenario that every core takes alike: the actor; the
/// choices offered to its oracle, in order; every choice it must consume,
/// each with the number of options it was offered; the branch labels it
/// must show; and the fault budget it must leave.
struct C4Step {
    actor: Actor,
    offered: &'static [u8],
    consumed: &'static [(u8, u8)],
    branches: &'static [&'static str],
    faults: u8,
}

/// One planned end of the acknowledgement append that control C4 changes:
/// the choices it consumes, each with its option count; its branch label;
/// and the fault budget it leaves.
struct C4Ending {
    consumed: &'static [(u8, u8)],
    branch: &'static str,
    faults: u8,
}

/// A C4 scenario on F-8 (contract 7.5: "acknowledgement test 1 removed",
/// P4 and P8, on F-8). Its steps, which every core takes alike, are
/// followed by one append of Ack(0)'s confirmed snapshot. At that append,
/// and only there, the two cores consume the offered choices differently:
/// the healthy core refuses at test 1, before any storage verb, and consumes
/// none; with test 1 removed the append proceeds and consumes every one.
/// Experimental: the C4 route proposed beside the AD-1-R3 draft, not adopted.
struct C4Scenario {
    named: &'static str,
    steps: &'static [C4Step],
    offered: &'static [u8],
    healthy: C4Ending,
    changed: C4Ending,
    /// On the world after the append: what the healthy refusal must keep,
    /// and what the named property's locus needs; each described.
    kept: fn(&C4After) -> bool,
    keeps: &'static str,
    lost: fn(&C4After) -> bool,
    locus: &'static str,
}

/// What the C4 verdicts read in the world after a step: the cells, the
/// phase and the latch; and for G1, every fact P8 reads.
struct C4After {
    u: Option<Occupant>,
    c: Option<Occupant>,
    x: Option<(Occupant, Stage)>,
    phase: Phase,
    latch: Option<LatchCause>,
    g1_issued: bool,
    g1_told: bool,
    g1_written: bool,
    g1_in_u: bool,
    g1_in_c: bool,
    g1_corrected: bool,
    word: u64,
    rows: Vec<SimRow>,
}

fn c4_after(w: &World) -> C4After {
    let key = gating_key(0);
    C4After {
        u: w.core.u.as_ref().map(|(_, occupant)| occupant.clone()),
        c: w.core.c.as_ref().map(|(_, occupant)| occupant.clone()),
        x: w.core
            .x
            .as_ref()
            .map(|(_, occupant, stage)| (occupant.clone(), *stage)),
        phase: w.core.phase,
        latch: w.core.latch,
        g1_issued: w.ghost.issued & bit(0) != 0,
        g1_told: w.ghost.told & bit(0) != 0,
        g1_written: w.rows.contains(&SimRow::Gating(0)),
        g1_in_u: matches!(&w.core.u, Some((_, Occupant::Gating { record, .. })) if record.member().key == key),
        g1_in_c: matches!(&w.core.c, Some((_, Occupant::Correction { about, .. })) if *about == key),
        g1_corrected: w
            .rows
            .iter()
            .any(|row| matches!(row, SimRow::Correction(0, _))),
        word: word_of(&w.core.shared),
        rows: w.rows.clone(),
    }
}

/// X holds F1's explanation.
fn c4_x_explains_f1(after: &C4After) -> bool {
    matches!(&after.x, Some((Occupant::Explanation { original, .. }, _)) if *original == fact_key(0))
}

/// The healthy refusal of the P4 scenario leaves U empty, X as it was.
fn c4_p4_kept(after: &C4After) -> bool {
    after.u.is_none() && c4_x_explains_f1(after)
}

/// P4's locus: U holds the acknowledgement while X holds F1's explanation.
fn c4_p4_lost(after: &C4After) -> bool {
    matches!(&after.u, Some(Occupant::Acknowledgement { .. })) && c4_x_explains_f1(after)
}

/// The healthy refusal of the P8 scenario keeps G1's written attempt in U.
fn c4_p8_kept(after: &C4After) -> bool {
    after.g1_issued && after.g1_written && !after.g1_told && after.g1_in_u
}

/// P8's locus: G1's attempt issued its insert, is written, was not told
/// Committed, and is neither in U nor corrected, in C or in the ledger.
fn c4_p8_lost(after: &C4After) -> bool {
    after.g1_issued
        && after.g1_written
        && !after.g1_told
        && !after.g1_in_u
        && !after.g1_in_c
        && !after.g1_corrected
}

/// What one replayed C4 step showed.
struct C4Replayed {
    actor: Actor,
    offered: &'static [u8],
    /// The choices the step consumed, each with its option count, as the
    /// oracle took them. Beyond `offered` the oracle would take zero.
    taken: Vec<(u8, u8)>,
    enabled: bool,
    faults: u8,
    fault: bool,
    note: String,
    branches: Vec<String>,
    /// Violations pushed by the step itself, and its transition properties.
    step: Violations,
    /// The state properties and key round trip of the world it leaves.
    after: Violations,
    world: C4After,
}

/// Replays one step from the world of `key` in the original semantics, as
/// `replay_schedule` does: the original `step` on a world decoded from the
/// key, between two takes of the hook's counts; then, if it was enabled, its
/// transition properties, and the state properties and key round trip of
/// the world it leaves. It asserts nothing about the choices: it keeps the
/// oracle's record of what the step consumed. Returns the record and the
/// key of the world after the step.
fn c4_replay_step(
    config: &Config,
    key: &[u8],
    actor: Actor,
    offered: &'static [u8],
) -> (C4Replayed, Vec<u8>) {
    let base = decode(config, key);
    let mut w = decode(config, key);
    let mut oracle = Oracle::new(offered.to_vec(), w.faults);
    let mut obs = Obs {
        trace: Some(String::new()),
        ..Obs::default()
    };
    hook::take_counts();
    let enabled = step(config, &mut w, actor, &mut oracle, &mut obs);
    let counts = hook::take_counts();
    w.faults = oracle.faults;
    if enabled {
        transition_properties(&base, &w, &mut obs, counts);
    }
    let next = encode(config, &w);
    let after = if enabled {
        violations_at(config, &next)
    } else {
        Vec::new()
    };
    let record = C4Replayed {
        actor,
        offered,
        taken: oracle.taken.clone(),
        enabled,
        faults: oracle.faults,
        fault: oracle.fault,
        note: obs.trace.unwrap_or_default(),
        branches: obs.branches,
        step: obs.violations,
        after,
        world: c4_after(&w),
    };
    (record, next)
}

/// One replayed C4 scenario: the initial state's violations; every step
/// replayed; which planned end the changed append took (`Some(false)` the
/// healthy refusal, `Some(true)` the append without test 1); and the fixture
/// failure that ended the scenario, if any.
struct C4Run {
    initial: Violations,
    steps: Vec<C4Replayed>,
    changed: Option<bool>,
    fixture: Option<String>,
}

/// The choices of a consumption, without their option counts.
fn c4_choices(consumed: &[(u8, u8)]) -> Vec<u8> {
    consumed.iter().map(|&(choice, _)| choice).collect()
}

/// Whether the schedule itself is as frozen: at most 32 actor steps; each
/// step offered exactly the choices it must consume; the healthy end
/// consumes nothing; the changed end consumes every offered choice.
fn c4_frozen(scenario: &C4Scenario) -> Option<String> {
    if scenario.steps.len() + 1 > 32 {
        return Some("has more than 32 actor steps".to_string());
    }
    for (n, planned) in scenario.steps.iter().enumerate() {
        if c4_choices(planned.consumed) != planned.offered {
            return Some(format!(
                "step {} offers other choices than it must consume",
                n + 1
            ));
        }
    }
    if !scenario.healthy.consumed.is_empty() {
        return Some("plans a healthy end that consumes a choice".to_string());
    }
    if c4_choices(scenario.changed.consumed) != scenario.offered {
        return Some("plans a changed end that does not consume every offered choice".to_string());
    }
    None
}

/// Why a replayed step's consumption is not allowed, if it is not: a step
/// that is not enabled; consumption beyond the offered choices, which the
/// oracle would answer with zero; or a consumed choice that is not the
/// offered one.
fn c4_consumption(record: &C4Replayed) -> Option<String> {
    if !record.enabled {
        return Some("is not enabled".to_string());
    }
    if record.taken.len() > record.offered.len() {
        return Some(format!(
            "consumed {} choices, beyond the {} offered: {:?}",
            record.taken.len(),
            record.offered.len(),
            record.taken
        ));
    }
    if c4_choices(&record.taken) != record.offered[..record.taken.len()] {
        return Some(format!(
            "consumed {:?}, not a prefix of the offered {:?}",
            record.taken, record.offered
        ));
    }
    None
}

/// Replays `scenario` from the unchanged initial world of `config`. Every
/// step must be enabled and consume exactly its offered choices, with the
/// frozen option counts, show its frozen branches and leave its frozen fault
/// budget. The changed append must end as one of its two planned ends.
/// Anything else is a fixture failure: it ends the scenario and kills
/// nothing. No violation ends the replay.
fn c4_replay(config: &Config, scenario: &C4Scenario) -> C4Run {
    let mut key = encode(config, &initial(config));
    let mut run = C4Run {
        initial: violations_at(config, &key),
        steps: Vec::new(),
        changed: None,
        fixture: None,
    };
    let total = scenario.steps.len() + 1;
    if let Some(failure) = c4_frozen(scenario) {
        run.fixture = Some(format!("scenario {} {failure}", scenario.named));
        return run;
    }
    for (n, planned) in scenario.steps.iter().enumerate() {
        let (record, next) = c4_replay_step(config, &key, planned.actor, planned.offered);
        let failure = if let Some(failure) = c4_consumption(&record) {
            Some(failure)
        } else if record.taken.as_slice() != planned.consumed {
            Some(format!(
                "consumed {:?}, not {:?}",
                record.taken, planned.consumed
            ))
        } else if record.branches != planned.branches {
            Some(format!(
                "shows branches {:?}, not {:?}",
                record.branches, planned.branches
            ))
        } else if record.faults != planned.faults {
            Some(format!(
                "leaves {} faults, not {}",
                record.faults, planned.faults
            ))
        } else {
            None
        };
        run.steps.push(record);
        key = next;
        if let Some(failure) = failure {
            run.fixture = Some(format!(
                "scenario {} step {}/{total} ({:?}) {failure}",
                scenario.named,
                n + 1,
                planned.actor
            ));
            return run;
        }
    }
    let (record, _) = c4_replay_step(config, &key, Actor::Ack(0), scenario.offered);
    let ends_as = |end: &C4Ending| {
        record.taken.as_slice() == end.consumed
            && record.branches == [end.branch]
            && record.faults == end.faults
    };
    let failure = if let Some(failure) = c4_consumption(&record) {
        Some(failure)
    } else if ends_as(&scenario.healthy) {
        run.changed = Some(false);
        None
    } else if ends_as(&scenario.changed) {
        run.changed = Some(true);
        None
    } else {
        Some(format!(
            "ends as neither planned end: consumed {:?}, branches {:?}, {} faults left",
            record.taken, record.branches, record.faults
        ))
    };
    run.steps.push(record);
    if let Some(failure) = failure {
        run.fixture = Some(format!(
            "scenario {} step {total}/{total} (Ack(0)) {failure}",
            scenario.named
        ));
    }
    run
}

/// Runs every C4 scenario on whatever core this build holds, and prints
/// every step: its offered, consumed and unused choices, its branches, the
/// cells, G1's facts, the ledger and every violation. A fixture failure in
/// any scenario kills nothing; it is reported after every scenario has run.
/// With no violation in any, each changed append must have ended as the
/// healthy refusal and kept what its scenario names. With any violation the
/// test fails, after every scenario has run; its message says, for each
/// named property, whether it was observed at its own locus, and lists every
/// other property violated, separately: a property is never counted as
/// another.
fn run_c4_witness(config: &Config, scenarios: &[C4Scenario]) {
    let mut lines = Vec::new();
    let mut fixtures = Vec::new();
    let mut verdicts = Vec::new();
    let mut healthy = Vec::new();
    let mut kept_all = true;
    for scenario in scenarios {
        let total = scenario.steps.len() + 1;
        println!(
            "C4 WITNESS {} {}: {total} steps from the initial world; step {total}, Ack(0)'s append, is \
             offered {:?}: the healthy core must consume {:?} and show '{}' ({} faults left); without \
             test 1 it must consume {:?} and show '{}' ({} faults left)",
            config.name,
            scenario.named,
            scenario.offered,
            scenario.healthy.consumed,
            scenario.healthy.branch,
            scenario.healthy.faults,
            scenario.changed.consumed,
            scenario.changed.branch,
            scenario.changed.faults
        );
        let run = c4_replay(config, scenario);
        for (property, detail) in &run.initial {
            println!("    VIOLATION initial: {property}: {detail}");
            lines.push(format!(
                "{} initial state: {property}: {detail}",
                scenario.named
            ));
        }
        for (n, record) in run.steps.iter().enumerate() {
            let unused = record.offered.get(record.taken.len()..).unwrap_or(&[]);
            println!(
                "C4 WITNESS {} {} step {:>2}/{total} {:?} offered {:?} consumed {:?} unused {:?} faults \
                 left {}: {}{}",
                config.name,
                scenario.named,
                n + 1,
                record.actor,
                record.offered,
                record.taken,
                unused,
                record.faults,
                record.note,
                if record.fault { "   [fault]" } else { "" }
            );
            let after = &record.world;
            println!(
                "    branches {:?}; word after {:#x} (live {}, unapplied {}); phase {:?}, latch {:?}",
                record.branches,
                after.word,
                field(after.word, LIVE_SHIFT),
                field(after.word, UNAPPLIED_SHIFT),
                after.phase,
                after.latch
            );
            println!("    U {:?}; C {:?}; X {:?}", after.u, after.c, after.x);
            println!(
                "    G1: insert issued {}, written {}, told Committed {}, in U {}, correction in C {}, \
                 correction in the ledger {}",
                after.g1_issued,
                after.g1_written,
                after.g1_told,
                after.g1_in_u,
                after.g1_in_c,
                after.g1_corrected
            );
            println!("    ledger after {:?}", after.rows);
            for (place, violations) in [("step", &record.step), ("after", &record.after)] {
                for (property, detail) in violations {
                    println!("    VIOLATION {place}: {property}: {detail}");
                    lines.push(format!(
                        "{} step {} {place}: {property}: {detail}",
                        scenario.named,
                        n + 1
                    ));
                }
            }
        }
        if let Some(failure) = &run.fixture {
            println!("    WITNESS FIXTURE FAILURE (kills nothing): {failure}");
            fixtures.push(failure.clone());
            kept_all = false;
            continue;
        }
        let Some(append) = run.steps.last() else {
            fixtures.push(format!("scenario {} replayed no step", scenario.named));
            kept_all = false;
            continue;
        };
        let named_seen = run.changed == Some(true)
            && append
                .after
                .iter()
                .any(|(property, _)| *property == scenario.named)
            && (scenario.lost)(&append.world);
        let kept = run.changed == Some(false) && (scenario.kept)(&append.world);
        kept_all &= kept;
        let mut properties: Vec<&str> = run
            .initial
            .iter()
            .chain(run.steps.iter().flat_map(|r| r.step.iter().chain(&r.after)))
            .map(|(property, _)| *property)
            .collect();
        properties.sort_unstable();
        properties.dedup();
        let others: Vec<&str> = properties
            .iter()
            .copied()
            .filter(|property| *property != scenario.named)
            .collect();
        let seen = if named_seen {
            "OBSERVED"
        } else {
            "NOT OBSERVED"
        };
        verdicts.push(format!(
            "NAMED PROPERTY {}: {seen} on the world after step {total} of scenario {} ({}; {})\n\
             OTHER PROPERTIES VIOLATED in scenario {}, each reported separately and not counted as {}: \
             {others:?}",
            scenario.named,
            scenario.named,
            scenario.changed.branch,
            scenario.locus,
            scenario.named,
            scenario.named
        ));
        healthy.push(format!(
            "C4 WITNESS {} {}: {total} steps, each enabled and consuming its choices as frozen; no \
             violation; step {total} shows the healthy core's '{}', consuming 0 of its {} offered choices; {}",
            config.name,
            scenario.named,
            scenario.healthy.branch,
            scenario.offered.len(),
            scenario.keeps
        ));
    }
    if !fixtures.is_empty() {
        panic!(
            "WITNESS FIXTURE FAILURE (kills nothing): {}",
            fixtures.join("; ")
        );
    }
    if lines.is_empty() {
        assert!(
            kept_all,
            "WITNESS: no violation, but a changed append did not end as the healthy refusal keeping \
             what its scenario names"
        );
        for line in &healthy {
            println!("{line}");
        }
        println!(
            "C4 WITNESS {}: {} scenarios healthy; no violation",
            config.name,
            scenarios.len()
        );
        return;
    }
    panic!(
        "C4 WITNESS {}: {} scenarios, every step enabled and consuming its choices as planned; {} \
         violations\n{}\nevery violation:\n  {}",
        config.name,
        scenarios.len(),
        lines.len(),
        verdicts.join("\n"),
        lines.join("\n  ")
    );
}

/// Fw(0) arms its ticket.
const C4_ARM: C4Step = C4Step {
    actor: Actor::Fw(0),
    offered: &[],
    consumed: &[],
    branches: &[],
    faults: 2,
};

/// Fw(0) appends F1 (its wait does not expire; open and facts Ok): F1's
/// Holder holds its role, so the append is refused as a DuplicateRole
/// conflict, after one rollback, Undone. F1's explanation is placed in X.
const C4_CONFLICT: C4Step = C4Step {
    actor: Actor::Fw(0),
    offered: &[0, 0, 0, 0],
    consumed: &[(0, 2), (0, 4), (0, 4), (0, 3)],
    branches: &[
        "append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)",
    ],
    faults: 2,
};

/// A worker step that consumes no choice and shows no branch: it reads an
/// agenda that allows exactly one operation, or it is the [T] step after.
const C4_WK_QUIET: C4Step = C4Step {
    actor: Actor::Wk,
    offered: &[],
    consumed: &[],
    branches: &[],
    faults: 2,
};

/// The worker appends X (open, present: absent, insert, commit, each Ok):
/// Explanation(0) is committed at 4 and X is marked committed.
const C4_APPEND_X: C4Step = C4Step {
    actor: Actor::Wk,
    offered: &[0, 0, 0, 0],
    consumed: &[(0, 3), (0, 3), (0, 2), (0, 2)],
    branches: &["append_cell X: Committed (inserted)"],
    faults: 2,
};

/// Ack(0) takes a snapshot of the explanation at 4 (survey Ok).
const C4_SNAPSHOT: C4Step = C4Step {
    actor: Actor::Ack(0),
    offered: &[0],
    consumed: &[(0, 4)],
    branches: &["snapshot: Taken"],
    faults: 2,
};

/// The simulated Owner confirms the snapshot. A model actor's choice: not
/// an approval, and nothing outside the model.
const C4_CONFIRM: C4Step = C4Step {
    actor: Actor::Ack(0),
    offered: &[0],
    consumed: &[(0, 2)],
    branches: &[],
    faults: 2,
};

/// The worker empties X, which is marked committed.
const C4_EMPTY_X: C4Step = C4Step {
    actor: Actor::Wk,
    offered: &[],
    consumed: &[],
    branches: &["empty: Done"],
    faults: 2,
};

/// Gc(0) appends G1: open, facts and insert Ok; commit Unknown (one fault),
/// with the record present; the one rollback Undone. Refused Uncertain
/// Storage: the phase is barrier needed and U keeps G1's attempt.
const C4_GATING: C4Step = C4Step {
    actor: Actor::Gc(0),
    offered: &[0, 0, 0, 1, 1, 0],
    consumed: &[(0, 4), (0, 4), (0, 4), (1, 3), (1, 2), (0, 3)],
    branches: &["append_gating: Refused Uncertain Storage"],
    faults: 1,
};

/// P4: after steps 1 to 7, Ack(0) appends while X holds F1's explanation.
/// The healthy core refuses at test 1, X being occupied. Without test 1 the
/// acknowledgement is placed in U; open, recheck and insert Ok; commit
/// Unknown (one fault), absent; the one rollback Undone: Uncertain, and U
/// kept while X is occupied.
const C4_P4: C4Scenario = C4Scenario {
    named: "P4",
    steps: &[
        C4_ARM,
        C4_CONFLICT,
        C4_WK_QUIET,
        C4_WK_QUIET,
        C4_APPEND_X,
        C4_SNAPSHOT,
        C4_CONFIRM,
    ],
    offered: &[0, 0, 0, 1, 0, 0],
    healthy: C4Ending {
        consumed: &[],
        branch: "append_acknowledgement: Refused(NotNow(CellsBusy))",
        faults: 2,
    },
    changed: C4Ending {
        consumed: &[(0, 4), (0, 4), (0, 4), (1, 3), (0, 2), (0, 3)],
        branch: "append_acknowledgement: NotApplied(Uncertain)",
        faults: 1,
    },
    kept: c4_p4_kept,
    keeps: "U stays empty and X holds F1's explanation",
    lost: c4_p4_lost,
    locus: "U holds the acknowledgement while X holds F1's explanation",
};

/// P8: after steps 1 to 7, the worker empties X (agenda, [T], empty), and
/// G1's append ends Uncertain with its record written, kept in U. Then
/// Ack(0) appends. The healthy core refuses at test 1, the phase not being
/// Healthy, and U keeps G1's attempt. Without test 1 the acknowledgement displaces it from U;
/// open, recheck, insert and commit Ok: Committed, and U emptied, with G1's
/// record written and corrected nowhere.
const C4_P8: C4Scenario = C4Scenario {
    named: "P8",
    steps: &[
        C4_ARM,
        C4_CONFLICT,
        C4_WK_QUIET,
        C4_WK_QUIET,
        C4_APPEND_X,
        C4_SNAPSHOT,
        C4_CONFIRM,
        C4_WK_QUIET,
        C4_WK_QUIET,
        C4_EMPTY_X,
        C4_GATING,
    ],
    offered: &[0, 0, 0, 0],
    healthy: C4Ending {
        consumed: &[],
        branch: "append_acknowledgement: Refused(NotNow(NotHealthy))",
        faults: 1,
    },
    changed: C4Ending {
        consumed: &[(0, 4), (0, 4), (0, 4), (0, 3)],
        branch: "append_acknowledgement: Committed",
        faults: 1,
    },
    kept: c4_p8_kept,
    keeps: "G1's written attempt stays in U",
    lost: c4_p8_lost,
    locus: "G1's written attempt, not told Committed, is neither in U nor corrected in C or in the ledger",
};

const C4_SCENARIOS: [C4Scenario; 2] = [C4_P4, C4_P8];

/// On the healthy core both C4 scenarios violate nothing: the P4
/// scenario's append is refused while X is occupied, and the P8 scenario's
/// while the phase is not Healthy, G1's attempt staying in U. Under control
/// C4 the test fails after both have run, with P4 and P8 each observed at
/// its own locus.
#[test]
fn c4_witness_p4_and_p8_on_f8() {
    run_c4_witness(&F8, &C4_SCENARIOS);
}
