//! The fault harness, verb by verb, on a real file (CONTRACT-02-R1 section
//! 13: "the fault harness, callback by callback").
//!
//! Every scenario is run once for each position of a verb call and each
//! fault, with one fault per run. The harness wraps this shell's real verbs:
//! a fault either replaces the verb (it is not performed) or follows it (it
//! was performed and its answer is lost). After the faulted operation the
//! worker brings the store to rest without faults, and the answers are
//! checked against what the file holds once the store is dropped and the
//! file reopened (and verified):
//!
//! - Committed: the record is in the file;
//! - Not written: it is not (assumption A-1 on these paths);
//! - Uncertain, or an unwind: either, and the worker's correction is in the
//!   file exactly when the record is;
//! - the file always reopens, its chain whole.
//!
//! These faults are at the verb boundary. They are not SQLite's own failures
//! inside a verb; `verb_tests` and `crash_tests` cover the real ones it can.

use super::*;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    /// Not performed: a read or `open` fails, an insert fails, a commit or a
    /// rollback is not issued and answers `Unknown`.
    Fail,
    /// Not performed: a read answers `Corrupt`.
    Corrupt,
    /// Not performed: an insert answers `Constraint`.
    Constraint,
    /// Not performed: the verb unwinds.
    Unwind,
    /// Performed, then its answer is lost: `Failed` or `Unknown`.
    Lost,
    /// Performed, then the verb unwinds.
    LostUnwind,
}

const FAULTS: [Fault; 6] = [
    Fault::Fail,
    Fault::Corrupt,
    Fault::Constraint,
    Fault::Unwind,
    Fault::Lost,
    Fault::LostUnwind,
];

/// The payload of an injected unwind.
struct Injected;

/// Which call gets the fault (and, for a pair, which later call gets a
/// second), and what the run saw.
struct Plan {
    at: usize,
    fault: Fault,
    second: Option<(usize, Fault)>,
    calls: usize,
    hit: Option<&'static str>,
    hit_second: Option<&'static str>,
}

impl Plan {
    fn new(at: usize, fault: Fault) -> Plan {
        Plan {
            at,
            fault,
            second: None,
            calls: 0,
            hit: None,
            hit_second: None,
        }
    }

    fn pair(at: usize, fault: Fault, later: usize, second: Fault) -> Plan {
        Plan {
            second: Some((later, second)),
            ..Plan::new(at, fault)
        }
    }

    /// No fault: the run is clean.
    fn none() -> Plan {
        Plan::new(usize::MAX, Fault::Fail)
    }

    fn next(&mut self, verb: &'static str) -> Option<Fault> {
        let n = self.calls;
        self.calls += 1;
        if n == self.at {
            self.hit = Some(verb);
            return Some(self.fault);
        }
        match self.second {
            Some((later, fault)) if later == n => {
                self.hit_second = Some(verb);
                Some(fault)
            }
            _ => None,
        }
    }
}

struct Faulty<'p, T> {
    inner: T,
    plan: &'p mut Plan,
}

fn unwind() -> ! {
    resume_unwind(Box::new(Injected))
}

impl<T> Faulty<'_, T> {
    fn read<R>(&mut self, verb: &'static str, real: impl FnOnce(&mut T) -> Read<R>) -> Read<R> {
        match self.plan.next(verb) {
            None => real(&mut self.inner),
            Some(Fault::Fail | Fault::Constraint) => Read::Failed,
            Some(Fault::Corrupt) => Read::Corrupt,
            Some(Fault::Unwind) => unwind(),
            Some(Fault::Lost) => {
                let _ = real(&mut self.inner);
                Read::Failed
            }
            Some(Fault::LostUnwind) => {
                let _ = real(&mut self.inner);
                unwind()
            }
        }
    }

    fn insert(&mut self, real: impl FnOnce(&mut T) -> Insert) -> Insert {
        match self.plan.next("insert") {
            None => real(&mut self.inner),
            Some(Fault::Fail | Fault::Corrupt) => Insert::Failed,
            Some(Fault::Constraint) => Insert::Constraint,
            Some(Fault::Unwind) => unwind(),
            Some(Fault::Lost) => {
                let _ = real(&mut self.inner);
                Insert::Failed
            }
            Some(Fault::LostUnwind) => {
                let _ = real(&mut self.inner);
                unwind()
            }
        }
    }

    fn commit(&mut self, real: impl FnOnce(&mut T) -> Commit) -> Commit {
        match self.plan.next("commit") {
            None => real(&mut self.inner),
            Some(Fault::Fail | Fault::Corrupt | Fault::Constraint) => Commit::Unknown,
            Some(Fault::Unwind) => unwind(),
            Some(Fault::Lost) => {
                let _ = real(&mut self.inner);
                Commit::Unknown
            }
            Some(Fault::LostUnwind) => {
                let _ = real(&mut self.inner);
                unwind()
            }
        }
    }

    fn rollback(&mut self, real: impl FnOnce(&mut T) -> Rollback) -> Rollback {
        match self.plan.next("rollback") {
            None => real(&mut self.inner),
            Some(Fault::Fail | Fault::Corrupt | Fault::Constraint) => Rollback::Unknown,
            Some(Fault::Unwind) => unwind(),
            Some(Fault::Lost) => {
                let _ = real(&mut self.inner);
                Rollback::Unknown
            }
            Some(Fault::LostUnwind) => {
                let _ = real(&mut self.inner);
                unwind()
            }
        }
    }
}

impl Txn for Faulty<'_, AppendIo<'_>> {
    fn open(&mut self) -> Read<()> {
        self.read("open", Txn::open)
    }
    fn facts(&mut self, index: u16, member: &Member) -> Read<MemberFacts> {
        self.read("facts", |io| io.facts(index, member))
    }
    fn insert(&mut self, index: u16, member: &Member) -> Insert {
        Faulty::insert(self, |io| Txn::insert(io, index, member))
    }
    fn commit(&mut self) -> Commit {
        Faulty::commit(self, Txn::commit)
    }
    fn rollback(&mut self) -> Rollback {
        Faulty::rollback(self, Txn::rollback)
    }
}

impl Reads for Faulty<'_, ReadIo<'_>> {
    fn row(&mut self, id: &[u8; ID_LEN]) -> Read<Option<Row>> {
        self.read("row", |io| io.row(id))
    }
    fn explained(&mut self, key: &RecordKey) -> Read<Option<Seq>> {
        self.read("explained", |io| io.explained(key))
    }
    fn acknowledgement(&mut self, set: &SetFacts, nonce: &[u8; 16]) -> Read<Option<Seq>> {
        self.read("acknowledgement", |io| io.acknowledgement(set, nonce))
    }
    fn frontier(&mut self) -> Read<Seq> {
        self.read("frontier", |io| io.frontier())
    }
}

impl BarrierRun for Faulty<'_, ReadIo<'_>> {
    fn run(&mut self) -> bool {
        match self.read("run", |io| {
            if io.run() {
                Read::Got(())
            } else {
                Read::Failed
            }
        }) {
            Read::Got(()) => true,
            Read::Failed | Read::Corrupt => false,
        }
    }
}

impl AckTxn for Faulty<'_, AckIo<'_>> {
    fn survey(&mut self) -> Read<SetFacts> {
        self.read("survey", |io| io.survey())
    }
    fn open(&mut self) -> Read<()> {
        self.read("open", AckTxn::open)
    }
    fn recheck(&mut self, above: Seq, upto: Seq) -> Read<SetFacts> {
        self.read("recheck", |io| io.recheck(above, upto))
    }
    fn insert(&mut self, snapshot: &Snapshot) -> Insert {
        Faulty::insert(self, |io| AckTxn::insert(io, snapshot))
    }
    fn commit(&mut self) -> Commit {
        Faulty::commit(self, AckTxn::commit)
    }
    fn rollback(&mut self) -> Rollback {
        Faulty::rollback(self, AckTxn::rollback)
    }
}

impl CellTxn for Faulty<'_, CellIo<'_>> {
    fn open(&mut self) -> Read<()> {
        self.read("open", CellTxn::open)
    }
    fn present(&mut self, occupant: &Occupant) -> Read<Option<Seq>> {
        self.read("present", |io| io.present(occupant))
    }
    fn insert(&mut self, occupant: &Occupant) -> Insert {
        Faulty::insert(self, |io| CellTxn::insert(io, occupant))
    }
    fn commit(&mut self) -> Commit {
        Faulty::commit(self, CellTxn::commit)
    }
    fn rollback(&mut self) -> Rollback {
        Faulty::rollback(self, CellTxn::rollback)
    }
}

/// Runs `op`, catching only an injected unwind.
fn caught<T>(op: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(value) => Some(value),
        Err(payload) if payload.is::<Injected>() => None,
        Err(payload) => resume_unwind(payload),
    }
}

/// What a store holds for one record: present (and where), and how many
/// corrections of it.
fn holds(store: &Store, key: &RecordKey) -> (Option<Seq>, usize) {
    let store_id = store.identity();
    let present = match by_id(&store.conn, &store_id, &key.id) {
        Read::Got(Some(row)) if row.digest == key.digest => Some(row.seq),
        Read::Got(_) => None,
        other => panic!("reading the record: {other:?}"),
    };
    let corrections = [Exit::ReturnedUncertain, Exit::Unwound]
        .into_iter()
        .filter(|exit| {
            matches!(
                by_id(
                    &store.conn,
                    &store_id,
                    &correction_id(&store_id, key, exit_code(*exit))
                ),
                Read::Got(Some(_))
            )
        })
        .count();
    (present, corrections)
}

/// The whole file, reopened and verified, and the record's state in it.
fn reopened(path: &Path, key: &RecordKey) -> (Option<Seq>, usize) {
    let store = Store::open(path, LIMITS).expect("the file reopens with its chain whole");
    holds(&store, key)
}

/// Every (position, fault) pair until the positions run out.
fn every_case(mut run: impl FnMut(usize, Fault) -> Option<&'static str>) -> usize {
    let mut cases = 0;
    for fault in FAULTS {
        for at in 0.. {
            match run(at, fault) {
                Some(_) => cases += 1,
                None => break,
            }
            assert!(at < 64, "a scenario with more than 64 verb calls");
        }
    }
    cases
}

/// Where a scenario starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Start {
    /// An empty, open store: the append inserts.
    Fresh,
    /// The record already committed: the clean path rolls back an empty
    /// transaction.
    Present,
    /// A record of the same identity with other content committed: the
    /// clean path refuses a conflict and rolls back.
    Conflicting,
}

const STARTS: [Start; 3] = [Start::Fresh, Start::Present, Start::Conflicting];

/// The verbs a run's faults landed on.
#[derive(Clone, Copy, Debug)]
struct Hits {
    first: &'static str,
    second: Option<&'static str>,
}

impl Plan {
    fn hits(&self) -> Option<Hits> {
        Some(Hits {
            first: self.hit?,
            second: self.hit_second,
        })
    }
}

/// Every single fault at every position.
fn every_single(mut run: impl FnMut(Plan) -> Option<Hits>) -> usize {
    every_case(|at, fault| run(Plan::new(at, fault)).map(|hits| hits.first))
}

/// Every pair: a first fault that lets the operation continue (it does not
/// unwind), at every position, and any second fault at every later position.
fn every_pair(mut run: impl FnMut(Plan) -> Option<Hits>) -> usize {
    let mut cases = 0;
    for first in [Fault::Fail, Fault::Lost, Fault::Constraint, Fault::Corrupt] {
        for at in 0.. {
            if run(Plan::new(at, first)).is_none() {
                break;
            }
            for second in FAULTS {
                for later in at + 1.. {
                    match run(Plan::pair(at, first, later, second)) {
                        Some(Hits {
                            second: Some(_), ..
                        }) => cases += 1,
                        _ => break,
                    }
                    assert!(later < 64);
                }
            }
            assert!(at < 64);
        }
    }
    cases
}

/// One gating append from `start` under `plan`, brought to rest and checked
/// against the reopened file.
fn gating_case(start: Start, mut plan: Plan) -> Option<Hits> {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let mut worker = store.take_worker().unwrap();
    let gating = store.gating(b"g", part(1, Role::Prepared), b"c");
    match start {
        Start::Fresh => {}
        Start::Present => {
            assert!(matches!(
                store.append_gating(&gating).unwrap(),
                GatingOutcome::Committed(_)
            ));
        }
        Start::Conflicting => {
            let other = store.gating(b"g", part(1, Role::Prepared), b"other");
            assert!(matches!(
                store.append_gating(&other).unwrap(),
                GatingOutcome::Committed(_)
            ));
        }
    }
    let before = holds(&store, &gating.key());
    let outcome = gating_with(&mut store, &gating, &mut plan);
    let hits = plan.hits()?;
    let context = format!(
        "{start:?}; {hits:?} ({:?}): {outcome:?}",
        (plan.at, plan.fault, plan.second)
    );
    let rest = drive(&mut store, &mut worker, 16);
    let (present, corrections) = holds(&store, &gating.key());
    match outcome {
        Some(GatingOutcome::Committed(seq)) => {
            assert_eq!(present, Some(seq), "{context}");
            assert_eq!(corrections, 0, "{context}");
        }
        Some(GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            ..
        }) => {
            assert_eq!((present, corrections), before, "{context}");
        }
        Some(GatingOutcome::Refused {
            persistence: Persistence::Uncertain,
            ..
        })
        | None => {
            if rest.latch.is_none() {
                let written = present.is_some() && before.0.is_none();
                assert_eq!(corrections, usize::from(written), "{context}");
            }
        }
    }
    if start == Start::Conflicting {
        assert_eq!(
            present, None,
            "{context}: a conflicting record is never written"
        );
    }
    if rest.latch.is_none() {
        assert_eq!(rest.phase, Phase::Healthy, "{context}");
        assert!(
            rest.u.is_none() && rest.c.is_none() && rest.x.is_none(),
            "{context}"
        );
        match (start, store.append_gating(&gating).unwrap()) {
            (
                _,
                GatingOutcome::Refused {
                    persistence: Persistence::NotWritten,
                    reason: Reason::Corrected { .. },
                },
            ) => assert_eq!(corrections, 1, "{context}"),
            (Start::Fresh | Start::Present, GatingOutcome::Committed(_)) => {
                assert_eq!(corrections, 0, "{context}")
            }
            (
                Start::Conflicting,
                GatingOutcome::Refused {
                    persistence: Persistence::NotWritten,
                    reason: Reason::Conflict { .. },
                },
            ) => {}
            (_, other) => panic!("{context}; offered again: {other:?}"),
        }
    }
    let held = holds(&store, &gating.key());
    drop(store);
    assert_eq!(reopened(&path, &gating.key()), held, "{context}");
    Some(hits)
}

/// One fact batch (two members, one transaction) from `start` under `plan`.
fn fact_case(start: Start, mut plan: Plan) -> Option<Hits> {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let mut worker = store.take_worker().unwrap();
    let batch = [
        store.fact(b"p", part(1, Role::Prepared), b"c"),
        store.fact(b"s", part(1, Role::Started), b"c"),
    ];
    let commit = |store: &mut Store, batch: &[Built<FactRecord>]| {
        let mut ticket = store.health().arm().unwrap();
        assert_eq!(
            store.append_facts(&mut ticket, batch).unwrap(),
            FactOutcome::Committed
        );
        ticket.confirm();
    };
    match start {
        Start::Fresh => {}
        Start::Present => commit(&mut store, &batch),
        Start::Conflicting => {
            let other = store.fact(b"p", part(2, Role::Prepared), b"other");
            commit(&mut store, &[other]);
        }
    }
    let before: Vec<_> = batch.iter().map(|b| holds(&store, &b.key())).collect();
    let staged: Vec<Staged> = batch
        .iter()
        .map(|built| Staged {
            member: *built.record().member(),
            draft: built.draft.clone(),
        })
        .collect();
    let records: Vec<FactRecord> = batch.iter().map(|b| b.record().clone()).collect();
    let mut ticket = store.health().arm().unwrap();
    let outcome = {
        let Store {
            conn,
            core,
            binding,
            ..
        } = &mut store;
        let mut io = Faulty {
            inner: AppendIo {
                conn,
                store: binding.identity,
                staged: &staged,
            },
            plan: &mut plan,
        };
        caught(|| core.append_facts(&mut ticket, &records, &mut io))
    };
    let counted = ticket.counted();
    ticket.confirm();
    let hits = plan.hits()?;
    let context = format!(
        "{start:?}; {hits:?} ({:?}): {outcome:?} {counted:?}",
        (plan.at, plan.fault, plan.second)
    );
    let rest = drive(&mut store, &mut worker, 16);
    let now: Vec<_> = batch.iter().map(|b| holds(&store, &b.key())).collect();
    let present: Vec<bool> = now.iter().map(|h| h.0.is_some()).collect();
    assert!(present[0] == present[1], "atomic: {context}; {present:?}");
    match (outcome, counted) {
        (Some(FactOutcome::Committed), _) => assert!(present[0], "{context}"),
        (Some(FactOutcome::NotApplied { counted, .. }), _) | (None, Some(counted))
            if counted.persistence == Persistence::NotWritten =>
        {
            assert_eq!(now, before, "{context}")
        }
        (Some(FactOutcome::NotApplied { .. }), _) | (None, Some(_)) => {}
        other => panic!("{context}: {other:?}"),
    }
    if start == Start::Conflicting {
        assert!(
            !present[0],
            "{context}: a conflicting batch is never written"
        );
    }
    if rest.latch.is_none() {
        assert_eq!(rest.phase, Phase::Healthy, "{context}");
        let mut ticket = store.health().arm().unwrap();
        let again = store.append_facts(&mut ticket, &batch).unwrap();
        ticket.confirm();
        match (start, again) {
            (Start::Fresh | Start::Present, FactOutcome::Committed) => {}
            (
                Start::Conflicting,
                FactOutcome::NotApplied {
                    reason: Reason::Conflict { member: 0, .. },
                    ..
                },
            ) => {}
            (_, other) => panic!("{context}: offered again: {other:?}"),
        }
    }
    assert!(!store.health().view().handover_fault(), "{context}");
    let held: Vec<_> = batch.iter().map(|b| holds(&store, &b.key())).collect();
    drop(store);
    for (built, held) in batch.iter().zip(held) {
        assert_eq!(reopened(&path, &built.key()), held, "{context}");
    }
    Some(hits)
}

/// A gating append from each start, faulted at every verb.
#[test]
fn gating_append_answers_truthfully_under_every_single_fault() {
    for start in STARTS {
        let cases = every_single(|plan| gating_case(start, plan));
        eprintln!("gating append from {start:?}: {cases} faulted runs");
        assert!(cases >= 18);
    }
}

/// A fact batch from each start, faulted at every verb.
#[test]
fn fact_append_answers_truthfully_under_every_single_fault() {
    for start in STARTS {
        let cases = every_single(|plan| fact_case(start, plan));
        eprintln!("fact append from {start:?}: {cases} faulted runs");
        assert!(cases >= 18);
    }
}

/// Two faults in one gating append: a first that lets it continue, then any
/// second, including on the one rollback a failure leads to.
#[test]
fn gating_append_answers_truthfully_under_every_pair_of_faults() {
    for start in STARTS {
        let cases = every_pair(|plan| gating_case(start, plan));
        eprintln!("gating append from {start:?}: {cases} runs with two faults");
        assert!(cases > 0);
    }
}

#[test]
fn fact_append_answers_truthfully_under_every_pair_of_faults() {
    for start in STARTS {
        let cases = every_pair(|plan| fact_case(start, plan));
        eprintln!("fact append from {start:?}: {cases} runs with two faults");
        assert!(cases > 0);
    }
}

/// The rollback verb is faulted: by pairs after a failure, and by single
/// faults where the clean path itself rolls back (Present, Conflicting).
#[test]
fn the_rollback_verb_is_among_the_faulted_verbs() {
    let mut rollbacks = 0;
    for start in STARTS {
        every_single(|plan| {
            let hits = gating_case(start, plan)?;
            rollbacks += usize::from(hits.first == "rollback");
            Some(hits)
        });
        every_pair(|plan| {
            let hits = fact_case(start, plan)?;
            rollbacks += usize::from(hits.second == Some("rollback"));
            Some(hits)
        });
    }
    eprintln!("runs with a faulted rollback: {rollbacks}");
    assert!(rollbacks >= 12);
}

/// A store with one explanation in the ledger, ready to acknowledge.
fn with_explanation(path: &Path) -> (Store, Worker) {
    let mut store = opened(path);
    let mut worker = store.take_worker().unwrap();
    for content in [b"one".as_slice(), b"two"] {
        let built = store.fact(b"x", None, content);
        let mut ticket = store.health().arm().unwrap();
        store.append_facts(&mut ticket, &[built]).unwrap();
        ticket.confirm();
    }
    let rest = drive(&mut store, &mut worker, 8);
    assert!(rest.x.is_none() && rest.latch.is_none());
    (store, worker)
}

/// An acknowledgement, faulted at every verb. The qualified frontier rises
/// exactly when the acknowledgement is known to be in the file.
#[test]
fn acknowledgement_answers_truthfully_under_every_single_fault() {
    let cases = every_case(|at, fault| {
        let scratch = Scratch::new();
        let path = scratch.path("l.db");
        let (mut store, mut worker) = with_explanation(&path);
        let SnapshotOutcome::Taken(snapshot) = store.snapshot() else {
            panic!("a snapshot")
        };
        let (set, nonce) = (snapshot.set(), snapshot.nonce());
        let mut plan = Plan::new(at, fault);
        let outcome = {
            let Store {
                conn,
                core,
                binding,
                ..
            } = &mut store;
            let mut io = Faulty {
                inner: AckIo {
                    conn,
                    store: binding.identity,
                },
                plan: &mut plan,
            };
            caught(|| core.append_acknowledgement(snapshot, &mut io))
        };
        let hit = plan.hit?;
        let context = format!("fault {fault:?} at call {at} ({hit}): {outcome:?}");
        let rest = drive(&mut store, &mut worker, 16);
        let key = ack_draft(&store.identity(), &set, &nonce).key();
        let (present, _) = holds(&store, &key);
        match outcome {
            Some(AckOutcome::Committed(n)) => {
                assert_eq!(n, set.top, "{context}");
                assert!(present.is_some(), "{context}");
            }
            Some(AckOutcome::Refused(_) | AckOutcome::NotApplied(Persistence::NotWritten)) => {
                assert!(present.is_none(), "{context}")
            }
            Some(AckOutcome::NotApplied(Persistence::Uncertain)) | None => {}
        }
        if rest.latch.is_none() {
            assert_eq!(rest.phase, Phase::Healthy, "{context}");
            assert!(rest.u.is_none(), "{context}");
            let qualified = if present.is_some() { set.top } else { 0 };
            assert_eq!(rest.frontier, qualified, "{context}");
        }
        drop(store);
        assert_eq!(
            reopened(&path, &key).0.is_some(),
            present.is_some(),
            "{context}"
        );
        Some(hit)
    });
    eprintln!("acknowledgement: {cases} faulted runs");
    assert!(cases > 20);
}

/// The worker's own verbs, faulted at every call while it resolves an
/// uncertain gating append whose record did commit: the correction is in the
/// file exactly once in the end, whatever failed on the way.
#[test]
fn resolving_an_uncertain_gating_append_survives_every_single_fault() {
    let cases = every_case(|at, fault| {
        let scratch = Scratch::new();
        let path = scratch.path("l.db");
        let mut store = opened(&path);
        let mut worker = store.take_worker().unwrap();
        let gating = store.gating(b"g", None, b"c");
        // The commit is performed and its answer lost.
        let staged = [Staged {
            member: *gating.record().member(),
            draft: gating.draft.clone(),
        }];
        {
            let Store {
                conn,
                core,
                binding,
                ..
            } = &mut store;
            let mut lost = Plan::new(3, Fault::Lost);
            let mut io = Faulty {
                inner: AppendIo {
                    conn,
                    store: binding.identity,
                    staged: &staged,
                },
                plan: &mut lost,
            };
            let outcome = core.append_gating(gating.record(), &mut io);
            assert_eq!(lost.hit, Some("commit"));
            assert!(matches!(
                outcome,
                GatingOutcome::Refused {
                    persistence: Persistence::Uncertain,
                    ..
                }
            ));
        }
        // The worker's steps, one fault among all their verbs.
        let mut plan = Plan::new(at, fault);
        for _ in 0..12 {
            let agenda = store.agenda();
            if agenda.latch.is_some() {
                break;
            }
            let Store {
                conn,
                core,
                binding,
                ..
            } = &mut store;
            let step = match agenda.phase {
                Phase::NeedBarrier => {
                    let mut io = Faulty {
                        inner: ReadIo {
                            conn,
                            store: binding.identity,
                        },
                        plan: &mut plan,
                    };
                    caught(|| {
                        core.barrier(&mut worker, &mut io);
                    })
                }
                Phase::Resolving => {
                    let token = agenda.u.expect("U is occupied while resolving");
                    let mut io = Faulty {
                        inner: ReadIo {
                            conn,
                            store: binding.identity,
                        },
                        plan: &mut plan,
                    };
                    caught(|| {
                        core.resolve(&mut worker, token, &mut io);
                    })
                }
                Phase::Healthy => match (agenda.c, agenda.x) {
                    (Some(token), _) | (None, Some((token, Stage::Built))) => {
                        let mut io = Faulty {
                            inner: CellIo {
                                conn,
                                store: binding.identity,
                            },
                            plan: &mut plan,
                        };
                        caught(|| {
                            core.append_cell(&mut worker, token, &mut io);
                        })
                    }
                    (None, Some((token, Stage::Committed(_)))) => {
                        core.empty(&mut worker, token);
                        Some(())
                    }
                    (None, None) => break,
                },
            };
            let _ = step;
        }
        let hit = plan.hit?;
        let context = format!("fault {fault:?} at worker call {at} ({hit})");
        let rest = drive(&mut store, &mut worker, 16);
        let (present, corrections) = holds(&store, &gating.key());
        assert!(present.is_some(), "{context}: the commit was performed");
        if rest.latch.is_none() {
            assert_eq!(corrections, 1, "{context}");
            assert!(matches!(
                store.append_gating(&gating).unwrap(),
                GatingOutcome::Refused {
                    reason: Reason::Corrected { .. },
                    ..
                }
            ));
        } else {
            assert!(corrections <= 1, "{context}");
        }
        let held = holds(&store, &gating.key());
        drop(store);
        assert_eq!(reopened(&path, &gating.key()), held, "{context}");
        Some(hit)
    });
    eprintln!("worker resolution: {cases} faulted runs");
    assert!(cases > 20);
}

/// The clean run of the harness itself: no fault, every answer Committed.
#[test]
fn the_harness_without_a_fault_changes_nothing() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let gating = store.gating(b"g", None, b"c");
    let staged = [Staged {
        member: *gating.record().member(),
        draft: gating.draft.clone(),
    }];
    let mut plan = Plan::none();
    let Store {
        conn,
        core,
        binding,
        ..
    } = &mut store;
    let mut io = Faulty {
        inner: AppendIo {
            conn,
            store: binding.identity,
            staged: &staged,
        },
        plan: &mut plan,
    };
    assert!(matches!(
        core.append_gating(gating.record(), &mut io),
        GatingOutcome::Committed(_)
    ));
    assert_eq!(plan.hit, None);
    assert_eq!(plan.calls, 4, "open, facts, insert, commit");
}

/// One gating append through the harness.
fn gating_with(
    store: &mut Store,
    built: &Built<GatingRecord>,
    plan: &mut Plan,
) -> Option<GatingOutcome> {
    let staged = [Staged {
        member: *built.record().member(),
        draft: built.draft.clone(),
    }];
    let Store {
        conn,
        core,
        binding,
        ..
    } = store;
    let mut io = Faulty {
        inner: AppendIo {
            conn,
            store: binding.identity,
            staged: &staged,
        },
        plan,
    };
    caught(|| core.append_gating(built.record(), &mut io))
}

fn row_count(store: &Store) -> i64 {
    store
        .conn
        .query_row("SELECT count(*) FROM ledger", [], |row| row.get(0))
        .unwrap()
}

/// Every row of CONTRACT-02-R1 section 4.5's table of re-offers, on a real
/// file (REVIEW-02, condition 6: "a test of every re-offer row is still
/// owed"). Each row reaches its earlier attempt's state through real verbs,
/// with at most one injected fault.
#[test]
fn every_re_offer_row_of_section_4_5_on_a_real_file() {
    let fresh = || {
        let scratch = Scratch::new();
        let mut store = opened(&scratch.path("l.db"));
        let worker = store.take_worker().unwrap();
        let gating = store.gating(b"g", part(9, Role::Prepared), b"c");
        (scratch, store, worker, gating)
    };
    let refused = |reason| GatingOutcome::Refused {
        persistence: Persistence::NotWritten,
        reason,
    };

    // 1. Committed, no correction, offered while Healthy with cells empty:
    //    Committed again with its sequence; nothing is written.
    let (_s, mut store, _w, gating) = fresh();
    let GatingOutcome::Committed(seq) = store.append_gating(&gating).unwrap() else {
        panic!()
    };
    let rows = row_count(&store);
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(seq)
    );
    assert_eq!(row_count(&store), rows);
    assert!(store.agenda().u.is_none());

    // 2. Committed, offered while an entry rule refuses: the entry refusal;
    //    U untouched; no correction exists or is made.
    let (_s, mut store, mut worker, gating) = fresh();
    assert!(matches!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(_)
    ));
    let mut failing = Plan::new(0, Fault::Fail);
    {
        let Store {
            conn,
            core,
            binding,
            ..
        } = &mut store;
        let mut io = Faulty {
            inner: ReadIo {
                conn,
                store: binding.identity,
            },
            plan: &mut failing,
        };
        assert_eq!(core.lookup(&gating.key(), &mut io), Lookup::Failed);
    }
    assert_eq!(store.agenda().phase, Phase::NeedBarrier);
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        refused(Reason::Entry(Entry::NotHealthy))
    );
    assert!(store.agenda().u.is_none());
    drive(&mut store, &mut worker, 8);
    assert_eq!(holds(&store, &gating.key()).1, 0);

    // 3. Ended Uncertain, still in U: the entry refusal.
    let (_s, mut store, _w, gating) = fresh();
    let outcome = gating_with(&mut store, &gating, &mut Plan::new(3, Fault::Fail));
    assert!(matches!(
        outcome,
        Some(GatingOutcome::Refused {
            persistence: Persistence::Uncertain,
            ..
        })
    ));
    assert!(store.agenda().u.is_some());
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        refused(Reason::Entry(Entry::NotHealthy))
    );

    // 4. Resolved as present, its correction in C: the entry refusal (C is
    //    occupied).
    let (_s, mut store, mut worker, gating) = fresh();
    gating_with(&mut store, &gating, &mut Plan::new(3, Fault::Lost));
    assert_eq!(store.barrier(&mut worker), BarrierOutcome::Passed);
    let token = store.agenda().u.unwrap();
    assert!(matches!(
        store.resolve(&mut worker, token),
        Resolve::Corrected(_)
    ));
    assert!(store.agenda().c.is_some());
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        refused(Reason::Entry(Entry::CellsBusy))
    );

    // 5. Its correction committed: refused, Not written, reason corrected,
    //    with the correction's sequence.
    drive(&mut store, &mut worker, 8);
    let store_id = store.identity();
    let correction = [Exit::ReturnedUncertain, Exit::Unwound]
        .into_iter()
        .find_map(|exit| {
            match by_id(
                &store.conn,
                &store_id,
                &correction_id(&store_id, &gating.key(), exit_code(exit)),
            ) {
                Read::Got(Some(row)) => Some(row.seq),
                _ => None,
            }
        })
        .expect("the correction is in the ledger");
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        refused(Reason::Corrected { seq: correction })
    );

    // 6. Resolved as absent: a fresh attempt.
    let (_s, mut store, mut worker, gating) = fresh();
    gating_with(&mut store, &gating, &mut Plan::new(3, Fault::Fail));
    let rest = drive(&mut store, &mut worker, 8);
    assert!(rest.u.is_none() && rest.c.is_none() && rest.phase == Phase::Healthy);
    assert_eq!(holds(&store, &gating.key()), (None, 0));
    assert!(matches!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(_)
    ));

    // 7. Ended Not written: a fresh attempt.
    let (_s, mut store, mut worker, gating) = fresh();
    let outcome = gating_with(&mut store, &gating, &mut Plan::new(2, Fault::Fail));
    assert!(matches!(
        outcome,
        Some(GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            ..
        })
    ));
    drive(&mut store, &mut worker, 8);
    assert!(matches!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(_)
    ));

    // 8. Refused as a conflict: a conflict again, as already explained.
    let (_s, mut store, mut worker, gating) = fresh();
    assert!(matches!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(_)
    ));
    let other = store.gating(b"g", part(9, Role::Prepared), b"other content");
    assert!(matches!(
        store.append_gating(&other).unwrap(),
        GatingOutcome::Refused {
            reason: Reason::Conflict {
                known: Known::Cause(_),
                ..
            },
            ..
        }
    ));
    drive(&mut store, &mut worker, 8);
    assert!(matches!(
        store.append_gating(&other).unwrap(),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Conflict {
                known: Known::Explained(_),
                ..
            }
        }
    ));
}

/// The worker's append of an explanation (cell X), faulted at every verb:
/// in the end the explanation is in the file exactly once, X is empty and
/// explain finds it.
#[test]
fn cell_append_answers_truthfully_under_every_single_fault() {
    let cases = every_case(|at, fault| {
        let scratch = Scratch::new();
        let path = scratch.path("l.db");
        let mut store = opened(&path);
        let mut worker = store.take_worker().unwrap();
        let mut keys = Vec::new();
        for content in [b"one".as_slice(), b"two"] {
            let built = store.fact(b"x", None, content);
            keys.push(built.key());
            let mut ticket = store.health().arm().unwrap();
            store.append_facts(&mut ticket, &[built]).unwrap();
            ticket.confirm();
        }
        let conflicting = keys[1];
        let Some((token, Stage::Built)) = store.agenda().x else {
            panic!("an explanation waits in X")
        };
        let mut plan = Plan::new(at, fault);
        let outcome = {
            let Store {
                conn,
                core,
                binding,
                ..
            } = &mut store;
            let mut io = Faulty {
                inner: CellIo {
                    conn,
                    store: binding.identity,
                },
                plan: &mut plan,
            };
            caught(|| core.append_cell(&mut worker, token, &mut io))
        };
        let hit = plan.hit?;
        let context = format!("fault {fault:?} at call {at} ({hit}): {outcome:?}");
        let store_id = store.identity();
        let explanation = explanation_id(&store_id, &conflicting);
        let in_file = |store: &Store| {
            matches!(
                by_id(&store.conn, &store_id, &explanation),
                Read::Got(Some(_))
            )
        };
        match outcome {
            Some(CellAppend::Committed(_)) => assert!(in_file(&store), "{context}"),
            Some(CellAppend::NotWritten) => assert!(!in_file(&store), "{context}"),
            _ => {}
        }
        let rest = drive(&mut store, &mut worker, 16);
        if rest.latch.is_none() {
            assert!(rest.x.is_none(), "{context}");
            assert!(in_file(&store), "{context}");
            assert!(
                matches!(store.explain(&conflicting), Explain::Explained(_)),
                "{context}"
            );
        }
        let explanations: i64 = store
            .conn
            .query_row("SELECT count(*) FROM ledger WHERE family = 4", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(explanations <= 1, "{context}");
        drop(store);
        Store::open(&path, LIMITS).expect("the file reopens");
        Some(hit)
    });
    eprintln!("cell append: {cases} faulted runs");
    assert!(cases > 10);
}

/// Lookup and explain, faulted at every read: they write nothing, a failed
/// read moves the phase to barrier needed, and a corrupt one latches.
#[test]
fn lookup_and_explain_write_nothing_under_every_single_fault() {
    for which in ["lookup", "explain"] {
        let cases = every_case(|at, fault| {
            let scratch = Scratch::new();
            let mut store = opened(&scratch.path("l.db"));
            let first = store.fact(b"x", None, b"one");
            let mut ticket = store.health().arm().unwrap();
            store.append_facts(&mut ticket, &[first]).unwrap();
            ticket.confirm();
            let probe = store.fact(b"x", None, b"two").key();
            let rows = row_count(&store);
            let mut plan = Plan::new(at, fault);
            {
                let Store {
                    conn,
                    core,
                    binding,
                    ..
                } = &mut store;
                let mut io = Faulty {
                    inner: ReadIo {
                        conn,
                        store: binding.identity,
                    },
                    plan: &mut plan,
                };
                let _ = caught(|| {
                    if which == "lookup" {
                        format!("{:?}", core.lookup(&probe, &mut io))
                    } else {
                        format!("{:?}", core.explain(&probe, &mut io))
                    }
                });
            }
            let hit = plan.hit?;
            let context = format!("{which}: fault {fault:?} at call {at} ({hit})");
            assert_eq!(row_count(&store), rows, "{context}");
            let agenda = store.agenda();
            match fault {
                Fault::Corrupt => assert_eq!(agenda.latch, Some(LatchCause::Corrupt), "{context}"),
                Fault::Unwind
                | Fault::LostUnwind
                | Fault::Fail
                | Fault::Lost
                | Fault::Constraint => {
                    assert_eq!(agenda.latch, None, "{context}");
                    assert_eq!(agenda.phase, Phase::NeedBarrier, "{context}");
                }
            }
            Some(hit)
        });
        eprintln!("{which}: {cases} faulted runs");
        assert!(cases >= 6);
    }
}
