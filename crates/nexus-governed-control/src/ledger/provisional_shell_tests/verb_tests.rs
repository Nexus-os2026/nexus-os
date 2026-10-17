//! Each verb on a real file: the fact it reports, exactly as section 4.1 of
//! CONTRACT-02-R1 defines it, and the corruption it must report instead of
//! a wrong fact (CONTRACT-02-R1 section 13, first row).

use super::*;

fn io<'a>(store: &'a Store, staged: &'a [Staged]) -> AppendIo<'a> {
    AppendIo {
        conn: &store.conn,
        store: store.identity(),
        staged,
    }
}

fn staged(built: &Built<FactRecord>) -> Staged {
    Staged {
        member: *built.record().member(),
        draft: built.draft.clone(),
    }
}

/// Commits one fact through the core and confirms its ticket.
fn commit_fact(store: &mut Store, built: &Built<FactRecord>) {
    let mut ticket = store.health().arm().unwrap();
    assert_eq!(
        store
            .append_facts(&mut ticket, std::slice::from_ref(built))
            .unwrap(),
        FactOutcome::Committed
    );
    ticket.confirm();
}

/// Lets a test write a row the way corruption would: around the triggers,
/// on the store's own connection.
fn tamper(store: &Store, sql: &str) {
    store
        .conn
        .execute_batch(&format!(
            "DROP TRIGGER ledger_no_update; {sql}; {}",
            SCHEMA[4]
        ))
        .unwrap();
}

#[test]
fn open_takes_a_write_transaction_and_refuses_a_second() {
    let scratch = Scratch::new();
    let store = opened(&scratch.path("l.db"));
    let staged = [];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert!(!store.conn.is_autocommit());
    assert_eq!(Txn::open(&mut io), Read::Failed);
    assert_eq!(Txn::rollback(&mut io), Rollback::Undone);
}

/// `Undone` needs ROLLBACK's success and no open transaction afterwards. A
/// ROLLBACK with no transaction fails, so it is `Unknown`.
#[test]
fn rollback_is_undone_only_when_it_succeeded_and_closed_the_transaction() {
    let scratch = Scratch::new();
    let store = opened(&scratch.path("l.db"));
    let staged = [];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::rollback(&mut io), Rollback::Unknown);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(Txn::rollback(&mut io), Rollback::Undone);
    assert!(store.conn.is_autocommit());
}

#[test]
fn commit_is_durable_only_when_it_succeeded() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let store = opened(&path);
    let built = store.fact(b"f", None, b"c");
    let staged = [staged(&built)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::commit(&mut io), Commit::Unknown, "no transaction");
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert!(matches!(
        Txn::insert(&mut io, 0, built.record().member()),
        Insert::Inserted(_)
    ));
    assert_eq!(Txn::commit(&mut io), Commit::Durable);
    drop(store);
    assert!(on_disk(&path, &built.key()));
}

/// A rolled-back insert leaves nothing (assumption A-1 on the ordinary path,
/// not a crash).
#[test]
fn an_undone_insert_leaves_nothing() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let store = opened(&path);
    let built = store.fact(b"f", None, b"c");
    let staged = [staged(&built)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert!(matches!(
        Txn::insert(&mut io, 0, built.record().member()),
        Insert::Inserted(_)
    ));
    assert_eq!(Txn::rollback(&mut io), Rollback::Undone);
    drop(store);
    assert!(!on_disk(&path, &built.key()));
}

#[test]
fn insert_assigns_the_next_sequence_and_chains_to_the_head() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let a = store.fact(b"a", None, b"1");
    let b = store.fact(b"b", None, b"2");
    commit_fact(&mut store, &a);
    commit_fact(&mut store, &b);
    let rows = select(
        &store.conn,
        &store.identity(),
        &format!("SELECT {COLUMNS} FROM ledger ORDER BY seq"),
        [],
    );
    let Read::Got(rows) = rows else {
        panic!("{rows:?}")
    };
    // The opening record, then a and b.
    assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(rows[0].prev, [0; 32]);
    assert_eq!(rows[1].prev, rows[0].tag);
    assert_eq!(rows[2].prev, rows[1].tag);
    assert_eq!(rows[1].id, a.key().id);
    assert_eq!(rows[2].id, b.key().id);
}

/// An insert of an identity already in the ledger fails on its constraint.
#[test]
fn a_duplicate_identity_is_a_constraint_failure() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let built = store.fact(b"f", None, b"c");
    commit_fact(&mut store, &built);
    let staged = [staged(&built)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(
        Txn::insert(&mut io, 0, built.record().member()),
        Insert::Constraint
    );
    Txn::rollback(&mut io);
}

#[test]
fn a_duplicate_role_is_a_constraint_failure() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let first = store.fact(b"p1", part(7, Role::Prepared), b"c");
    commit_fact(&mut store, &first);
    let second = store.fact(b"p2", part(7, Role::Prepared), b"c");
    let staged = [staged(&second)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(
        Txn::insert(&mut io, 0, second.record().member()),
        Insert::Constraint
    );
    Txn::rollback(&mut io);
}

/// Insert takes only the record staged at its position: a member the core
/// did not stage is refused rather than written.
#[test]
fn insert_refuses_a_member_that_was_not_staged() {
    let scratch = Scratch::new();
    let store = opened(&scratch.path("l.db"));
    let a = store.fact(b"a", None, b"1");
    let b = store.fact(b"b", None, b"2");
    let staged = [staged(&a)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(Txn::insert(&mut io, 0, b.record().member()), Insert::Failed);
    assert_eq!(Txn::insert(&mut io, 1, a.record().member()), Insert::Failed);
    assert_eq!(Txn::facts(&mut io, 0, b.record().member()), Read::Failed);
    Txn::rollback(&mut io);
}

#[test]
fn facts_report_row_holder_and_prepared() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let prepared = store.fact(b"p", part(1, Role::Prepared), b"c");
    commit_fact(&mut store, &prepared);
    let started = store.fact(b"s", part(1, Role::Started), b"c");
    let staged_all = [staged(&prepared), staged(&started)];
    let mut io = io(&store, &staged_all);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    let Read::Got(facts) = Txn::facts(&mut io, 0, prepared.record().member()) else {
        panic!()
    };
    let row = facts.row.expect("its row");
    assert_eq!(row.digest, prepared.key().digest);
    assert_eq!(
        facts.holder,
        Some(Holder {
            id: prepared.key().id,
            seq: row.seq
        })
    );
    assert!(facts.prepared);
    assert_eq!(facts.explained, None);
    assert_eq!(facts.corrected, None);
    let Read::Got(facts) = Txn::facts(&mut io, 1, started.record().member()) else {
        panic!()
    };
    assert_eq!(facts.row, None);
    assert_eq!(facts.holder, None);
    assert!(facts.prepared, "its commitment has a Prepared row");
    Txn::rollback(&mut io);
}

#[test]
fn a_tampered_row_reads_as_corrupt() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let built = store.fact(b"f", None, b"content");
    commit_fact(&mut store, &built);
    tamper(&store, "UPDATE ledger SET body = x'00' WHERE family = 1");
    let staged = [staged(&built)];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(
        Txn::facts(&mut io, 0, built.record().member()),
        Read::Corrupt
    );
    Txn::rollback(&mut io);
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.row(&built.key().id), Read::Corrupt);
}

/// A row moved to another sequence, or carried over from another store,
/// fails its tag.
#[test]
fn a_moved_row_and_a_foreign_row_read_as_corrupt() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let built = store.fact(b"f", None, b"c");
    commit_fact(&mut store, &built);
    tamper(&store, "UPDATE ledger SET seq = 99 WHERE family = 1");
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.row(&built.key().id), Read::Corrupt);

    let mut other = opened(&scratch.path("m.db"));
    let foreign = other.fact(b"g", None, b"c");
    commit_fact(&mut other, &foreign);
    let row: Raw = other
        .conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM ledger WHERE id = ?1"),
            params![foreign.key().id.as_slice()],
            raw,
        )
        .unwrap();
    let (_, family, id, digest, commitment, role, about, acked, needs_ack, body, prev, tag) = row;
    store
        .conn
        .execute(
            &format!("INSERT INTO ledger ({COLUMNS}) VALUES (100, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"),
            params![family, id, digest, commitment, role, about, acked, needs_ack, body, prev, tag],
        )
        .unwrap();
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.row(&foreign.key().id), Read::Corrupt);
}

/// A correction counts only when it is bound to the record's exact identity,
/// digest and row: one naming another sequence is a contradiction.
#[test]
fn a_correction_not_bound_to_its_row_reads_as_corrupt() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let gating = store.gating(b"g", None, b"c");
    assert!(matches!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Committed(_)
    ));
    // A correction about the record but naming a sequence that is not its row.
    let occupant = Occupant::Correction {
        about: gating.key(),
        seq: 999,
        exit: Exit::ReturnedUncertain,
    };
    let draft = cell_draft(&store.identity(), &occupant).unwrap();
    assert!(matches!(
        append(&store.conn, &store.identity(), &draft),
        Insert::Inserted(_)
    ));
    let staged = [Staged {
        member: *gating.record().member(),
        draft: gating.draft.clone(),
    }];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    assert_eq!(
        Txn::facts(&mut io, 0, gating.record().member()),
        Read::Corrupt
    );
    Txn::rollback(&mut io);
}

#[test]
fn a_correction_bound_to_its_row_is_reported() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let gating = store.gating(b"g", None, b"c");
    let GatingOutcome::Committed(seq) = store.append_gating(&gating).unwrap() else {
        panic!()
    };
    let occupant = Occupant::Correction {
        about: gating.key(),
        seq,
        exit: Exit::Unwound,
    };
    let draft = cell_draft(&store.identity(), &occupant).unwrap();
    let Insert::Inserted(correction) = append(&store.conn, &store.identity(), &draft) else {
        panic!()
    };
    let staged = [Staged {
        member: *gating.record().member(),
        draft: gating.draft.clone(),
    }];
    let mut io = io(&store, &staged);
    assert_eq!(Txn::open(&mut io), Read::Got(()));
    let Read::Got(facts) = Txn::facts(&mut io, 0, gating.record().member()) else {
        panic!()
    };
    assert_eq!(facts.corrected, Some(correction));
    Txn::rollback(&mut io);
}

/// Verify before filter (REVIEW-02 section 5.3): an acknowledgement-requiring
/// row whose flag was cleared is not silently left out of the set; the
/// survey reads it, it fails its tag, and the survey is corrupt.
#[test]
fn a_cleared_acknowledgement_flag_is_corrupt_not_omitted() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let occupant = Occupant::Explanation {
        original: RecordKey {
            id: [1; 32],
            digest: [2; 32],
        },
        cause: explanation_cause(),
    };
    let draft = cell_draft(&store.identity(), &occupant).unwrap();
    assert!(matches!(
        append(&store.conn, &store.identity(), &draft),
        Insert::Inserted(_)
    ));
    let mut ack = AckIo {
        conn: &store.conn,
        store: store.identity(),
    };
    let Read::Got(set) = ack.survey() else {
        panic!()
    };
    assert_eq!(set.count, 1);
    tamper(&store, "UPDATE ledger SET needs_ack = 0 WHERE family = 4");
    let mut ack = AckIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(ack.survey(), Read::Corrupt);
    let _ = &mut store;
}

/// A cause the core built, obtained the only way a cause can be: by a
/// conflicting append through the core.
pub(super) fn explanation_cause() -> Cause {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("cause.db"));
    let first = store.fact(b"x", None, b"one");
    commit_fact(&mut store, &first);
    let second = store.fact(b"x", None, b"two");
    let mut ticket = store.health().arm().unwrap();
    let outcome = store.append_facts(&mut ticket, &[second]).unwrap();
    ticket.confirm();
    match outcome {
        FactOutcome::NotApplied {
            reason:
                Reason::Conflict {
                    known: Known::Cause(cause),
                    ..
                },
            ..
        } => cause,
        other => panic!("{other:?}"),
    }
}

/// A real storage failure: a full database. The insert fails as `Failed`;
/// the rollback's answer is recorded as observed, and the core's persistence
/// follows it.
#[test]
fn a_full_database_fails_the_insert() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let pages: i64 = store
        .conn
        .query_row("PRAGMA page_count", [], |row| row.get(0))
        .unwrap();
    let limit: i64 = store
        .conn
        .query_row(&format!("PRAGMA max_page_count = {pages}"), [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(limit, pages);
    let big = store.fact(b"big", None, &vec![7u8; 64 * 1024]);
    let mut ticket = store.health().arm().unwrap();
    let outcome = store
        .append_facts(&mut ticket, std::slice::from_ref(&big))
        .unwrap();
    ticket.confirm();
    match outcome {
        FactOutcome::NotApplied {
            counted,
            reason: Reason::Storage,
        } => {
            eprintln!("full database: persistence {:?}", counted.persistence);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(store.agenda().phase, Phase::NeedBarrier);
    drop(store);
    assert!(!on_disk(&path, &big.key()));
}

/// A database a byte of which was damaged on disk is refused when opened.
#[test]
fn a_damaged_row_on_disk_is_refused_at_open() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let marker = b"MARKER-0123456789-MARKER";
    let built = store.fact(b"f", None, marker);
    commit_fact(&mut store, &built);
    drop(store);
    // Fold the WAL into the main file, then damage the marker's bytes there.
    let raw = Connection::open(&path).unwrap();
    raw.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
    raw.execute_batch("PRAGMA journal_mode = DELETE").unwrap();
    drop(raw);
    let mut bytes = std::fs::read(&path).unwrap();
    let at = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("the marker is in the file");
    bytes[at] ^= 0x01;
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(
        Store::open(&path, LIMITS),
        Err(StoreError::Corrupt)
    ));
}

/// The barrier rolls back a transaction an unwind left open, and verifies
/// the chain.
#[test]
fn the_barrier_closes_a_transaction_left_open() {
    let scratch = Scratch::new();
    let store = opened(&scratch.path("l.db"));
    store.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut io = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert!(io.run());
    assert!(store.conn.is_autocommit());
}

#[test]
fn the_barrier_fails_on_a_tampered_row() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let built = store.fact(b"f", None, b"c");
    commit_fact(&mut store, &built);
    tamper(&store, "UPDATE ledger SET body = x'01' WHERE family = 1");
    let mut io = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert!(!io.run());
}

/// A gap in the chain (a row cut out) fails verification at open.
#[test]
fn a_cut_row_fails_the_chain() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    let a = store.fact(b"a", None, b"1");
    let b = store.fact(b"b", None, b"2");
    commit_fact(&mut store, &a);
    commit_fact(&mut store, &b);
    store
        .conn
        .execute_batch(&format!(
            "DROP TRIGGER ledger_no_delete; DELETE FROM ledger WHERE seq = 2; {}",
            SCHEMA[5]
        ))
        .unwrap();
    assert_eq!(verify_all(&store.conn, &store.identity()), Read::Corrupt);
    drop(store);
    assert!(matches!(
        Store::open(&path, LIMITS),
        Err(StoreError::Corrupt)
    ));
}

/// The frontier is the N of the last acknowledgement row.
#[test]
fn the_frontier_is_the_last_acknowledgement() {
    let scratch = Scratch::new();
    let store = opened(&scratch.path("l.db"));
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.frontier(), Read::Got(0));
    for (top, nonce) in [(5u64, [1u8; 16]), (9, [2; 16])] {
        let set = SetFacts {
            frontier: 0,
            head: top,
            top,
            count: 1,
            digest: [0; 32],
        };
        let draft = ack_draft(&store.identity(), &set, &nonce);
        assert!(matches!(
            append(&store.conn, &store.identity(), &draft),
            Insert::Inserted(_)
        ));
    }
    let mut reads = ReadIo {
        conn: &store.conn,
        store: store.identity(),
    };
    assert_eq!(reads.frontier(), Read::Got(9));
}
