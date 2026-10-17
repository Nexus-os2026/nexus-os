//! The core and this shell together, on a real file: each operation's
//! ordinary paths, and what reopening the file shows.

use super::*;

fn fact_ok(store: &mut Store, batch: &[Built<FactRecord>]) -> FactOutcome {
    let mut ticket = store.health().arm().unwrap();
    let outcome = store.append_facts(&mut ticket, batch).unwrap();
    ticket.confirm();
    outcome
}

#[test]
fn a_committed_batch_is_present_and_survives_reopening() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let batch = [
        store.fact(b"p", part(1, Role::Prepared), b"prepared"),
        store.fact(b"s", part(1, Role::Started), b"started"),
        store.fact(b"e", part(1, Role::End), b"ended"),
    ];
    assert_eq!(fact_ok(&mut store, &batch), FactOutcome::Committed);
    for built in &batch {
        assert!(matches!(store.lookup(&built.key()), Lookup::Present(_)));
    }
    let keys: Vec<RecordKey> = batch.iter().map(Built::key).collect();
    drop(store);
    let mut store = Store::open(&path, LIMITS).unwrap();
    assert_eq!(store.open_session(), Opening::Opened);
    for key in &keys {
        assert!(matches!(store.lookup(key), Lookup::Present(_)));
    }
}

/// Offering the same batch again writes nothing and answers Committed.
#[test]
fn a_batch_offered_again_is_already_present() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let batch = [store.fact(b"a", None, b"1"), store.fact(b"b", None, b"2")];
    assert_eq!(fact_ok(&mut store, &batch), FactOutcome::Committed);
    let before = rows_on_disk_open(&store);
    assert_eq!(fact_ok(&mut store, &batch), FactOutcome::Committed);
    assert_eq!(rows_on_disk_open(&store), before);
}

fn rows_on_disk_open(store: &Store) -> i64 {
    store
        .conn
        .query_row("SELECT count(*) FROM ledger", [], |row| row.get(0))
        .unwrap()
}

/// Same identity, different content: a conflict whose explanation the
/// worker appends; explain then finds it in the ledger, and the explanation
/// requires acknowledgement.
#[test]
fn an_identity_conflict_is_explained_in_the_ledger() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let mut worker = store.take_worker().unwrap();
    let first = store.fact(b"x", None, b"one");
    assert_eq!(
        fact_ok(&mut store, std::slice::from_ref(&first)),
        FactOutcome::Committed
    );
    let second = store.fact(b"x", None, b"two");
    let mut ticket = store.health().arm().unwrap();
    let outcome = store
        .append_facts(&mut ticket, std::slice::from_ref(&second))
        .unwrap();
    assert!(matches!(
        outcome,
        FactOutcome::NotApplied {
            counted: Counted {
                persistence: Persistence::NotWritten,
                conflict_member: Some(0)
            },
            reason: Reason::Conflict {
                member: 0,
                known: Known::Cause(_)
            }
        }
    ));
    ticket.confirm();
    assert_eq!(store.explain(&second.key()), Explain::Pending);
    let agenda = drive(&mut store, &mut worker, 8);
    assert!(agenda.x.is_none() && agenda.latch.is_none(), "{agenda:?}");
    let Explain::Explained(seq) = store.explain(&second.key()) else {
        panic!()
    };
    assert!(
        matches!(store.lookup(&second.key()), Lookup::Conflict(Known::Explained(s)) if s == seq)
    );
    let SnapshotOutcome::Taken(snapshot) = store.snapshot() else {
        panic!("the explanation requires acknowledgement")
    };
    assert_eq!(snapshot.set().top, seq);
    assert_eq!(snapshot.set().count, 1);
}

#[test]
fn a_missing_prepared_and_a_duplicate_role_are_conflicts() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let mut worker = store.take_worker().unwrap();
    let orphan = store.fact(b"s", part(2, Role::Started), b"c");
    let outcome = fact_ok(&mut store, &[orphan]);
    assert!(matches!(
        outcome,
        FactOutcome::NotApplied {
            reason: Reason::Conflict { known: Known::Cause(cause), .. },
            ..
        } if matches!(cause.kind(), CauseKind::MissingPrepared { .. })
    ));
    drive(&mut store, &mut worker, 8);
    let prepared = store.fact(b"p", part(3, Role::Prepared), b"c");
    assert_eq!(fact_ok(&mut store, &[prepared]), FactOutcome::Committed);
    let again = store.fact(b"p2", part(3, Role::Prepared), b"c");
    let outcome = fact_ok(&mut store, &[again]);
    assert!(matches!(
        outcome,
        FactOutcome::NotApplied {
            reason: Reason::Conflict { known: Known::Cause(cause), .. },
            ..
        } if matches!(cause.kind(), CauseKind::DuplicateRole { .. })
    ));
}

#[test]
fn a_gating_record_commits_and_is_committed_again_when_offered_again() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let gating = store.gating(b"g", part(4, Role::Prepared), b"c");
    let GatingOutcome::Committed(seq) = store.append_gating(&gating).unwrap() else {
        panic!()
    };
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(seq)
    );
    assert!(store.agenda().u.is_none());
}

/// A record built by one store is refused by another before the core is
/// asked; so is a record of the same file opened as another store.
#[test]
fn a_record_of_another_store_is_refused() {
    let scratch = Scratch::new();
    let mut a = opened(&scratch.path("a.db"));
    let mut b = opened(&scratch.path("b.db"));
    let from_a = a.fact(b"f", None, b"c");
    let mut ticket = b.health().arm().unwrap();
    assert_eq!(
        b.append_facts(&mut ticket, std::slice::from_ref(&from_a)),
        Err(OtherStore)
    );
    ticket.confirm();
    let gating = a.gating(b"g", None, b"c");
    assert_eq!(b.append_gating(&gating), Err(OtherStore));

    let path = scratch.path("c.db");
    let first = opened(&path);
    let stale = first.fact(b"f", None, b"c");
    drop(first);
    let mut again = Store::open(&path, LIMITS).unwrap();
    assert_eq!(again.open_session(), Opening::Opened);
    let mut ticket = again.health().arm().unwrap();
    assert_eq!(again.append_facts(&mut ticket, &[stale]), Err(OtherStore));
    ticket.confirm();
    let _ = &mut a;
}

/// The acknowledgement round: snapshot, append, the qualified frontier; a
/// second snapshot finds nothing.
#[test]
fn an_acknowledgement_raises_both_frontiers() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let mut worker = store.take_worker().unwrap();
    let first = store.fact(b"x", None, b"one");
    fact_ok(&mut store, &[first]);
    let second = store.fact(b"x", None, b"two");
    fact_ok(&mut store, &[second]);
    drive(&mut store, &mut worker, 8);
    let SnapshotOutcome::Taken(snapshot) = store.snapshot() else {
        panic!()
    };
    let top = snapshot.set().top;
    assert_eq!(
        store.append_acknowledgement(snapshot),
        AckOutcome::Committed(top)
    );
    assert_eq!(store.agenda().frontier, top);
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.frontier(), Read::Got(top));
    assert!(matches!(store.snapshot(), SnapshotOutcome::Nothing));
}

/// A snapshot whose set changed before its append is refused, and consumed.
#[test]
fn a_stale_snapshot_is_refused() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let mut worker = store.take_worker().unwrap();
    let x1 = store.fact(b"x", None, b"one");
    fact_ok(&mut store, &[x1]);
    let x2 = store.fact(b"x", None, b"two");
    fact_ok(&mut store, &[x2]);
    drive(&mut store, &mut worker, 8);
    let SnapshotOutcome::Taken(old) = store.snapshot() else {
        panic!()
    };
    let SnapshotOutcome::Taken(newer) = store.snapshot() else {
        panic!()
    };
    let top = newer.set().top;
    assert_eq!(
        store.append_acknowledgement(newer),
        AckOutcome::Committed(top)
    );
    assert_eq!(
        store.append_acknowledgement(old),
        AckOutcome::Refused(AckRefusal::Stale)
    );
}

/// A snapshot of another store is refused as such.
#[test]
fn a_snapshot_of_another_store_is_refused() {
    let scratch = Scratch::new();
    let mut a = opened(&scratch.path("a.db"));
    let mut b = opened(&scratch.path("b.db"));
    let mut worker = a.take_worker().unwrap();
    let x1 = a.fact(b"x", None, b"one");
    fact_ok(&mut a, &[x1]);
    let x2 = a.fact(b"x", None, b"two");
    fact_ok(&mut a, &[x2]);
    drive(&mut a, &mut worker, 8);
    let SnapshotOutcome::Taken(snapshot) = a.snapshot() else {
        panic!()
    };
    assert_eq!(
        b.append_acknowledgement(snapshot),
        AckOutcome::Refused(AckRefusal::OtherStore)
    );
}

/// A corrupt row met by an operation latches the store with cause Corrupt.
#[test]
fn a_corrupt_read_latches_the_store() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let built = store.fact(b"f", None, b"c");
    fact_ok(&mut store, std::slice::from_ref(&built));
    store
        .conn
        .execute_batch(&format!(
            "DROP TRIGGER ledger_no_update; UPDATE ledger SET body = x'00' WHERE family = 1; {}",
            SCHEMA[4]
        ))
        .unwrap();
    let outcome = fact_ok(&mut store, &[built]);
    assert!(matches!(
        outcome,
        FactOutcome::NotApplied {
            reason: Reason::Storage,
            ..
        }
    ));
    assert_eq!(store.agenda().latch, Some(LatchCause::Corrupt));
    assert!(!store.health().view().admits());
}
