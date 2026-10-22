//! The session's opening and end (CONTRACT-02-R1 section 4.9, corrected by
//! RF-01; REVIEW-02 section 5.1), with real write failures: the store's own
//! connection is made read-only, so SQLite refuses the write.

use super::*;

fn read_only(store: &Store, on: bool) {
    store
        .conn
        .execute_batch(&format!(
            "PRAGMA query_only = {}",
            if on { "ON" } else { "OFF" }
        ))
        .unwrap();
}

/// The session is open only once its opening record is confirmed: a failed
/// write leaves it in bootstrap, where every append is refused; a later
/// attempt writes the record and opens it.
#[test]
fn the_session_opens_only_on_a_confirmed_opening_record() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = Store::create(&path, LIMITS).unwrap();
    read_only(&store, true);
    assert_eq!(store.open_session(), Opening::NotWritten);
    assert_eq!(store.agenda().session, Session::Bootstrap);
    let built = store.fact(b"f", None, b"c");
    let mut ticket = store.health().arm().unwrap();
    assert!(matches!(
        store.append_facts(&mut ticket, &[built]).unwrap(),
        FactOutcome::NotApplied {
            reason: Reason::Entry(Entry::NotOpen),
            ..
        }
    ));
    ticket.confirm();
    read_only(&store, false);
    assert_eq!(store.open_session(), Opening::Opened);
    assert_eq!(store.agenda().session, Session::Open);
    assert_eq!(store.open_session(), Opening::NotBootstrap);
    let opening = store.session_draft(Family::Opened);
    drop(store);
    assert!(on_disk(&path, &opening.key()));
}

/// An opening record found already present (a retry after an uncertain
/// write) counts as confirmed.
#[test]
fn an_opening_record_already_present_opens_the_session() {
    let scratch = Scratch::new();
    let mut store = Store::create(&scratch.path("l.db"), LIMITS).unwrap();
    let draft = store.session_draft(Family::Opened);
    assert_eq!(store.write_private(&draft), PrivateWrite::Confirmed);
    assert_eq!(store.agenda().session, Session::Bootstrap);
    let count = |store: &Store| -> i64 {
        store
            .conn
            .query_row("SELECT count(*) FROM ledger", [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(count(&store), 1);
    assert_eq!(store.open_session(), Opening::Opened);
    assert_eq!(count(&store), 1, "nothing written twice");
}

/// Sealing needs quiescence and changes nothing otherwise.
#[test]
fn sealing_needs_quiescence() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let x1 = store.fact(b"x", None, b"one");
    let x2 = store.fact(b"x", None, b"two");
    for built in [x1, x2] {
        let mut ticket = store.health().arm().unwrap();
        store.append_facts(&mut ticket, &[built]).unwrap();
        ticket.confirm();
    }
    assert!(store.agenda().x.is_some(), "an explanation waits in X");
    assert_eq!(store.seal_and_close(), Sealing::NotQuiescent);
    assert_eq!(store.session_end(), (false, false, false));
    assert_eq!(store.agenda().session, Session::Open);
}

/// Sealed first and for good; the close record is a separate attempt. A
/// failed close leaves the session sealed, and a later attempt confirms it.
#[test]
fn a_failed_close_leaves_the_session_sealed() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    read_only(&store, true);
    assert_eq!(store.seal_and_close(), Sealing::SealedCloseNotWritten);
    assert_eq!(store.session_end(), (true, true, false));
    assert_eq!(store.agenda().session, Session::Sealed);
    let gating = store.gating(b"g", None, b"c");
    assert_eq!(
        store.append_gating(&gating).unwrap(),
        GatingOutcome::Refused {
            persistence: Persistence::NotWritten,
            reason: Reason::Entry(Entry::NotOpen)
        }
    );
    read_only(&store, false);
    assert_eq!(store.seal_and_close(), Sealing::Closed);
    assert_eq!(store.session_end(), (true, true, true));
    assert_eq!(store.agenda().session, Session::Sealed);
    let closing = store.session_draft(Family::Closed);
    drop(store);
    assert!(on_disk(&path, &closing.key()));
}

/// After the seal a fact append can still answer rows a to c of section 4.4
/// and writes nothing: a record already present is Committed; a new one is
/// refused as session closed.
#[test]
fn after_the_seal_a_fact_append_writes_nothing() {
    let scratch = Scratch::new();
    let mut store = opened(&scratch.path("l.db"));
    let present = store.fact(b"p", None, b"c");
    let mut ticket = store.health().arm().unwrap();
    assert_eq!(
        store
            .append_facts(&mut ticket, std::slice::from_ref(&present))
            .unwrap(),
        FactOutcome::Committed
    );
    ticket.confirm();
    assert_eq!(store.seal_and_close(), Sealing::Closed);
    let count_before: i64 = store
        .conn
        .query_row("SELECT count(*) FROM ledger", [], |row| row.get(0))
        .unwrap();
    let mut ticket = store.health().arm().unwrap();
    assert_eq!(
        store.append_facts(&mut ticket, &[present]).unwrap(),
        FactOutcome::Committed
    );
    ticket.confirm();
    let new = store.fact(b"n", None, b"c");
    let mut ticket = store.health().arm().unwrap();
    assert!(matches!(
        store.append_facts(&mut ticket, &[new]).unwrap(),
        FactOutcome::NotApplied {
            reason: Reason::SessionClosed { member: 0 },
            ..
        }
    ));
    ticket.confirm();
    let count_after: i64 = store
        .conn
        .query_row("SELECT count(*) FROM ledger", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count_before, count_after);
}

/// Each store value has its own session: reopening the file starts in
/// bootstrap and writes its own opening record beside the old session's.
#[test]
fn a_reopened_store_opens_a_new_session() {
    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let mut store = opened(&path);
    assert_eq!(store.seal_and_close(), Sealing::Closed);
    drop(store);
    let mut store = Store::open(&path, LIMITS).unwrap();
    assert_eq!(store.agenda().session, Session::Bootstrap);
    assert_eq!(store.open_session(), Opening::Opened);
    let count: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM ledger WHERE family IN (6, 7)",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 3, "open, close, open");
}

/// The wait to enter the store runs out while another holds it: the ticket
/// is expired and counted Not written, and nothing is written.
#[test]
fn a_wait_that_runs_out_expires_the_ticket() {
    use std::sync::mpsc;
    use std::time::Duration;

    let scratch = Scratch::new();
    let path = scratch.path("l.db");
    let store = opened(&path);
    let built = store.fact(b"f", None, b"c");
    // Tickets are armed before the store moves into the gate; each holds its
    // ledger's shared state.
    let mut ticket = store.health().arm().unwrap();
    let mut second = store.health().arm().unwrap();
    let gate = Arc::new(Gate::new(store));
    let unapplied = |gate: &Gate| {
        gate.with(Instant::now() + Duration::from_secs(30), |store| {
            store.health().view().unapplied()
        })
        .unwrap()
    };
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    let holder = {
        let gate = Arc::clone(&gate);
        std::thread::spawn(move || {
            gate.with(Instant::now() + Duration::from_secs(30), |_| {
                entered.send(()).unwrap();
                wait_release.recv().unwrap();
            })
            .expect("the holder enters");
        })
    };
    wait_entered.recv().unwrap();
    let outcome = gate
        .append_facts(
            Instant::now() + Duration::from_millis(50),
            &mut ticket,
            std::slice::from_ref(&built),
        )
        .unwrap();
    assert_eq!(
        outcome,
        FactOutcome::NotApplied {
            counted: Counted {
                persistence: Persistence::NotWritten,
                conflict_member: None
            },
            reason: Reason::Entry(Entry::Expired)
        }
    );
    release.send(()).unwrap();
    holder.join().unwrap();
    assert_eq!(unapplied(&gate), 1);
    ticket.confirm();
    assert_eq!(unapplied(&gate), 0);
    assert_eq!(
        gate.append_facts(
            Instant::now() + Duration::from_secs(30),
            &mut second,
            std::slice::from_ref(&built)
        )
        .unwrap(),
        FactOutcome::Committed
    );
    second.confirm();
    drop(gate);
    assert!(on_disk(&path, &built.key()));
}
