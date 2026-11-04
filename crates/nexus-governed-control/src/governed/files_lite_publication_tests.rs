//! M2d-C round 4 (settlement design v2, rule 1; pre-registered before the
//! field exists): a door's view of a commitment carries an explicit
//! publication class. Only `ConfirmedNotPublished` says nothing was
//! written: a commitment never handed to the kernel's apply, or refused at
//! or before its final check with the kernel's proof. Executing (or not
//! yet started) is `Pending`; an uncertain outcome, or an execution that
//! ended after its handoff without proof either way, is `Uncertain`. A
//! child of the B2 oracles, for their fixture.

use super::*;
use crate::authority::commitment::{CommitmentState, PublicationClass};
use crate::governed::Lookup;

fn class_of(f: &Fx, id: CommitmentId) -> PublicationClass {
    match f.door.publication(id).expect("typed readable publication") {
        Lookup::Mine(publication) => publication,
        other => panic!("the door's own commitment: {other:?}"),
    }
}

/// A plain success, and one interrupted after its final check: published,
/// with the codes the success reported; recovery not required.
#[test]
fn lite_p_a_publication_is_published_with_its_codes() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.execute(&view).expect("published");
    assert_eq!(
        class_of(&f, view.id),
        PublicationClass::Published {
            recovery_required: false,
            interrupted: vec![],
        }
    );
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    Cause::Cancel.arm(&f, at::FILE_AFTER_FINAL);
    f.execute(&view).expect("the one publication completes");
    assert_eq!(
        class_of(&f, view.id),
        PublicationClass::Published {
            recovery_required: false,
            interrupted: vec!["cancelled"],
        }
    );
}

/// Each cause at the final check refuses the publication with the kernel's
/// proof: confirmed not published.
#[test]
fn lite_p_refused_at_the_final_check_is_confirmed_not_published() {
    for cause in CAUSES {
        let f = cause.fixture();
        let view = f.ready(NAME, CONTENT);
        cause.arm(&f, at::FILE_BEFORE_FINAL);
        f.execute(&view).expect_err("never published");
        assert!(!f.ws.join(NAME).exists(), "{cause:?}");
        assert_eq!(
            class_of(&f, view.id),
            PublicationClass::ConfirmedNotPublished,
            "{cause:?}"
        );
    }
}

/// Never handed off: denied, revoked before it started, failed before the
/// apply (the name exists by then): confirmed not published.
#[test]
fn lite_p_never_handed_off_is_confirmed_not_published() {
    let f = Fx::new();
    let denied = f.propose(NAME, CONTENT).unwrap();
    f.gov.deny(denied.id, &mother(), f.run).unwrap();
    assert_eq!(
        class_of(&f, denied.id),
        PublicationClass::ConfirmedNotPublished
    );

    let f = Fx::new();
    let revoked = f.ready(NAME, CONTENT);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    assert_eq!(
        f.gov
            .authority()
            .commitments()
            .view(revoked.id)
            .unwrap()
            .state,
        CommitmentState::Revoked
    );
    assert_eq!(
        class_of(&f, revoked.id),
        PublicationClass::ConfirmedNotPublished
    );

    let f = Fx::new();
    let raced = f.ready(NAME, CONTENT);
    std::fs::write(f.ws.join(NAME), b"someone else's").unwrap();
    f.execute(&raced).expect_err("the name exists");
    assert_eq!(
        class_of(&f, raced.id),
        PublicationClass::ConfirmedNotPublished
    );
}

/// Not final: prepared, authorized, and executing (held before its final
/// check) are pending, never confirmed either way.
#[test]
fn lite_p_not_final_is_pending() {
    let f = Fx::new();
    let view = f.propose(NAME, CONTENT).unwrap();
    assert_eq!(class_of(&f, view.id), PublicationClass::Pending);
    f.authorize(&view).unwrap();
    assert_eq!(class_of(&f, view.id), PublicationClass::Pending);
    let (gate, worker) = held(&f, &view, at::FILE_BEFORE_FINAL);
    assert_eq!(class_of(&f, view.id), PublicationClass::Pending);
    gate.open();
    assert!(worker.join().unwrap());
    assert!(matches!(
        class_of(&f, view.id),
        PublicationClass::Published { .. }
    ));
}

/// An execution that ends without a result after its handoff (a panic
/// inside the kernel's apply, before its final check) has no proof either
/// way: uncertain, never "not published". One that ends so before its
/// handoff (at its start) is confirmed not published.
#[test]
fn lite_p_an_abandoned_execution_is_uncertain_after_its_handoff() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_BEFORE_FINAL, || panic!("inside the apply"));
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.execute(&view)));
    assert!(unwound.is_err(), "the hook unwound the execution");
    assert_eq!(
        f.gov.authority().commitments().view(view.id).unwrap().state,
        CommitmentState::Failed
    );
    assert_eq!(class_of(&f, view.id), PublicationClass::Uncertain);

    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_EXECUTE_START, || panic!("before the handoff"));
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.execute(&view)));
    assert!(unwound.is_err());
    assert_eq!(
        class_of(&f, view.id),
        PublicationClass::ConfirmedNotPublished
    );
}

/// The kernel's uncertain publication (review 1, H2) is uncertain.
#[test]
fn lite_p_an_uncertain_publication_is_uncertain() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    let (state, ws) = (f.state(), f.ws.clone());
    let held: Arc<Mutex<Option<rusqlite::Connection>>> = Arc::default();
    let holding = held.clone();
    f.gov.files().hooks().arm(at::FILE_AFTER_FINAL, move || {
        let ledger = LedgerFile::open(&state.join("ledger.db")).unwrap();
        let mut temp = None;
        for id in ledger.run_ids().unwrap() {
            for record in ledger.store().verified_records(id.ledger_key()).unwrap() {
                if record.event_kind == "apply.prepared" {
                    let payload: serde_json::Value = serde_json::from_str(&record.payload).unwrap();
                    temp = payload["files"][0]["temp_id"].as_str().map(str::to_string);
                }
            }
        }
        let temp = temp.expect("the planned temp id");
        std::fs::write(
            ws.join(format!(".nexus-coding-run-tmp-{temp}")),
            b"the compensation's name, taken",
        )
        .unwrap();
        let conn = rusqlite::Connection::open(state.join("ledger.db")).unwrap();
        conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
        *holding.lock().unwrap() = Some(conn);
    });
    let _ = f.execute(&view);
    drop(held.lock().unwrap().take());
    assert_eq!(f.finished(view.id).outcome, Some("uncertain"));
    assert_eq!(class_of(&f, view.id), PublicationClass::Uncertain);
}

#[test]
fn lite_p_unreadable_publication_is_a_typed_error() {
    for poison_runs in [false, true] {
        let f = Fx::new();
        let file = f.ready(NAME, CONTENT);
        std::thread::scope(|scope| {
            let _ = scope
                .spawn(|| {
                    if poison_runs {
                        f.gov.authority().runs().panic_holding_the_runs_lock();
                    } else {
                        f.gov.authority().commitments().panic_holding_entries();
                    }
                })
                .join();
        });
        assert!(f.door.publication(file.id).is_err());
    }
}
