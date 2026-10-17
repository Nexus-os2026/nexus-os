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

/// Which call gets the fault, and what the run saw.
struct Plan {
    at: usize,
    fault: Fault,
    calls: usize,
    hit: Option<&'static str>,
}

impl Plan {
    fn new(at: usize, fault: Fault) -> Plan {
        Plan {
            at,
            fault,
            calls: 0,
            hit: None,
        }
    }

    /// No fault: the run is clean.
    fn none() -> Plan {
        Plan::new(usize::MAX, Fault::Fail)
    }

    fn next(&mut self, verb: &'static str) -> Option<Fault> {
        let n = self.calls;
        self.calls += 1;
        (n == self.at).then(|| {
            self.hit = Some(verb);
            self.fault
        })
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

/// A gating append, faulted at every verb, then brought to rest.
#[test]
fn gating_append_answers_truthfully_under_every_single_fault() {
    let cases = every_case(|at, fault| {
        let scratch = Scratch::new();
        let path = scratch.path("l.db");
        let mut store = opened(&path);
        let mut worker = store.take_worker().unwrap();
        let gating = store.gating(b"g", part(1, Role::Prepared), b"c");
        let staged = [Staged {
            member: *gating.record().member(),
            draft: gating.draft.clone(),
        }];
        let mut plan = Plan::new(at, fault);
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
            caught(|| core.append_gating(gating.record(), &mut io))
        };
        let hit = plan.hit?;
        let context = format!("fault {fault:?} at call {at} ({hit}): {outcome:?}");
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
                assert_eq!(present, None, "{context}");
                assert_eq!(corrections, 0, "{context}");
            }
            Some(GatingOutcome::Refused {
                persistence: Persistence::Uncertain,
                ..
            })
            | None => {
                if rest.latch.is_none() {
                    assert_eq!(corrections, usize::from(present.is_some()), "{context}");
                }
            }
        }
        if rest.latch.is_none() {
            assert_eq!(rest.phase, Phase::Healthy, "{context}");
            assert!(
                rest.u.is_none() && rest.c.is_none() && rest.x.is_none(),
                "{context}"
            );
            // Offered again: corrected, or committed (present or fresh).
            match store.append_gating(&gating).unwrap() {
                GatingOutcome::Refused {
                    persistence: Persistence::NotWritten,
                    reason: Reason::Corrected { .. },
                } => assert_eq!(corrections, 1, "{context}"),
                GatingOutcome::Committed(_) => assert_eq!(corrections, 0, "{context}"),
                other => panic!("{context}; offered again: {other:?}"),
            }
        }
        let held = holds(&store, &gating.key());
        drop(store);
        assert_eq!(reopened(&path, &gating.key()), held, "{context}");
        Some(hit)
    });
    eprintln!("gating append: {cases} faulted runs");
    assert!(cases > 20);
}

/// A batch of two facts, faulted at every verb. A batch is one transaction:
/// both members or neither.
#[test]
fn fact_append_answers_truthfully_under_every_single_fault() {
    let cases = every_case(|at, fault| {
        let scratch = Scratch::new();
        let path = scratch.path("l.db");
        let mut store = opened(&path);
        let mut worker = store.take_worker().unwrap();
        let batch = [
            store.fact(b"p", part(1, Role::Prepared), b"c"),
            store.fact(b"s", part(1, Role::Started), b"c"),
        ];
        let staged: Vec<Staged> = batch
            .iter()
            .map(|built| Staged {
                member: *built.record().member(),
                draft: built.draft.clone(),
            })
            .collect();
        let records: Vec<FactRecord> = batch.iter().map(|b| b.record().clone()).collect();
        let mut ticket = store.health().arm().unwrap();
        let mut plan = Plan::new(at, fault);
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
        let hit = plan.hit?;
        let context = format!("fault {fault:?} at call {at} ({hit}): {outcome:?} {counted:?}");
        let rest = drive(&mut store, &mut worker, 16);
        let present: Vec<bool> = batch
            .iter()
            .map(|built| holds(&store, &built.key()).0.is_some())
            .collect();
        assert!(present[0] == present[1], "atomic: {context}; {present:?}");
        match (outcome, counted) {
            (Some(FactOutcome::Committed), _) => assert!(present[0], "{context}"),
            (Some(FactOutcome::NotApplied { counted, .. }), _) | (None, Some(counted))
                if counted.persistence == Persistence::NotWritten =>
            {
                assert!(!present[0], "{context}")
            }
            (Some(FactOutcome::NotApplied { .. }), _) | (None, Some(_)) => {}
            other => panic!("{context}: {other:?}"),
        }
        if rest.latch.is_none() {
            assert_eq!(rest.phase, Phase::Healthy, "{context}");
            let mut ticket = store.health().arm().unwrap();
            assert_eq!(
                store.append_facts(&mut ticket, &batch).unwrap(),
                FactOutcome::Committed,
                "{context}: offered again"
            );
            ticket.confirm();
        }
        assert!(!store.health().view().handover_fault(), "{context}");
        let held: Vec<_> = batch.iter().map(|b| holds(&store, &b.key())).collect();
        drop(store);
        for (built, held) in batch.iter().zip(held) {
            assert_eq!(reopened(&path, &built.key()), held, "{context}");
        }
        Some(hit)
    });
    eprintln!("fact append: {cases} faulted runs");
    assert!(cases > 20);
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
