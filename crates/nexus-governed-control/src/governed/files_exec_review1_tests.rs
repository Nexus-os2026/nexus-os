//! Oracles for Codex review 1 of PR #64 (M2d-B2), pre-registered before the
//! repairs, with the Architect's decisions: H1 (superseded: see below), H2
//! (an explicit publication outcome; an uncertain one is never a plain
//! failure), H3 (an interruption after the handoff is reported beside the
//! success and ends Files for the run), H4 (the undo's lifetime is taken
//! before its cleanup), M1 (a failed staging cleanup stays owned, is
//! reported and has the Owner's bounded retry).
//! A child of the B2 oracles, for their fixture.

use super::*;

// ---------------------------------------------------------------- H1

// H1 (amended): the two broker-return oracles first registered here
// (m2d_b2_r1_h1_a_stop_returns_while_the_store_is_blocked and
// m2d_b2_r1_h1_b_a_failing_store_is_surfaced_and_recorded_on_recovery)
// required the asynchronous Stop records writer the Architect's H1
// decision (Option B) rejected; they are superseded, not satisfied. Their
// replacements are Codex's broker rows 2-6 in files_exec_review2_tests.rs:
// m2d_b2_r2_row2_the_halt_is_engaged_while_another_writer_holds_the_store,
// m2d_b2_r2_row3_stop_held_in_its_own_write_refuses_the_handoff_and_the_undo,
// m2d_b2_r2_row4_a_stalled_publication_or_cleanup_does_not_hold_stop,
// m2d_b2_r2_row5_stop_transfers_a_pending_effect_without_cleanup and
// m2d_b2_r2_row6_stop_and_resume_revive_neither_the_old_publication_nor_the_old_undo;
// the terminal acknowledgement rows 7-8 are M2d-D's.

// ---------------------------------------------------------------- H2

/// H2: the kernel publishes, its `apply.op` record fails (the ledger is
/// held by another writer), and the compensating unpublication fails (its
/// planned temporary name is taken): the file is present. The broker
/// reports that as uncertain, recovery required, never as a plain failure;
/// the run is kept; the Owner's detail names the file.
#[test]
fn m2d_b2_r1_h2_an_uncertain_publication_is_reported_and_kept() {
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
    let result = f.execute(&view);
    drop(held.lock().unwrap().take());
    assert_eq!(class(result), "recovery_required", "never a plain failure");
    assert_eq!(
        std::fs::read(f.ws.join(NAME)).unwrap(),
        CONTENT.as_bytes(),
        "the file is present"
    );
    let end = f.finished(view.id);
    assert_eq!(end.outcome, Some("uncertain"));
    assert_eq!(f.gov.files().kept_for_test(), 1, "the run is kept");
    let detail = f.detail(view.id).expect("the Owner's detail");
    let all: String = detail.iter().map(|(l, v)| format!("{l}: {v}\n")).collect();
    assert!(all.contains(NAME), "{all}");
    assert!(all.contains("may be present"), "{all}");
}

// ---------------------------------------------------------------- H3

/// H3 / M4′: a publication whose authority a cause ended after the handoff
/// is reported as the success it is plus a controlled interruption code,
/// in the result and in the evidence; and no further Files request of that
/// run is accepted, even once the authority is restored.
#[test]
fn m2d_b2_r1_h3_an_interruption_after_the_handoff_is_reported_and_ends_files_for_the_run() {
    for (cause, code) in [
        (Cause::Revoke, "revoked"),
        (Cause::GrantExpiry, "expired"),
        (Cause::DeadlineMonotonic, "commitment_expired"),
        (Cause::DeadlineWall, "commitment_expired"),
        (Cause::EmergencyStop, "emergency_stop"),
    ] {
        let f = cause.fixture();
        let view = f.ready(NAME, CONTENT);
        cause.arm(&f, at::FILE_AFTER_FINAL);
        let output = f.execute(&view).expect("the one publication completes");
        let interrupted = ("interrupted".to_string(), code.to_string());
        assert!(
            output.meta.contains(&interrupted),
            "{cause:?}: {:?}",
            output.meta
        );
        assert!(
            f.finished(view.id).detail.contains(&interrupted),
            "{cause:?}: in the evidence"
        );
        match cause {
            Cause::EmergencyStop => f.gov.resume(&*f.owner).unwrap(),
            Cause::Revoke | Cause::GrantExpiry => {
                f.gov
                    .request_grant(
                        &GrantRequest::Files {
                            workspace: f.gov.workspace().unwrap().id,
                        },
                        LONG,
                        &*f.owner,
                    )
                    .unwrap();
            }
            _ => {}
        }
        assert!(
            f.propose("after.txt", "a\n").is_err(),
            "{cause:?}: no further Files request for the run"
        );
        assert!(!f.ws.join("after.txt").exists());
    }
}

// ---------------------------------------------------------------- H4

/// H4 / H2′: an undo's lifetime is taken when it starts, before any
/// cleanup it does: a Stop and the Owner's resume while that cleanup runs
/// refuse this undo, with no dialog and nothing touched; a new undo request
/// is then offered and completes.
#[test]
fn m2d_b2_r1_h4_the_undo_lifetime_is_taken_before_its_cleanup() {
    let f = Fx::new();
    f.create(NAME, CONTENT).unwrap();
    let _pending = f.propose("pending.txt", "p\n").unwrap();
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let (gov, owner) = (f.gov.clone(), f.owner.clone());
    f.gov.files().hooks().arm(at::FILE_CLEANUP, move || {
        gov.emergency_stop();
        gov.resume(&*owner).unwrap();
    });
    assert!(
        f.gov.files_undo(&*f.owner).is_err(),
        "the undo began before the Stop"
    );
    assert!(f.owner.undos().is_empty(), "no dialog for it");
    assert_eq!(f.entries(), vec![NAME.to_string()], "nothing touched");
    f.gov
        .files_undo(&*f.owner)
        .expect("a new undo request after the resume");
    assert!(f.entries().is_empty());
}

// ---------------------------------------------------------------- M1

/// M1 / A6: a staging cleanup that fails (its directory cannot be emptied)
/// keeps the run owned and is reported; the Owner's bounded retry leaves it
/// owed while the fault lasts and settles it once the fault clears.
#[test]
fn m2d_b2_r1_m1_a_failed_staging_cleanup_stays_owned_reported_and_retried() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fx::new();
    let _pending = f.propose(NAME, CONTENT).unwrap();
    let staging = f.state().join("coding-runs").join(&f.staged()[0]);
    let _ = f.door.cancel_run(f.run).unwrap().write();
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o500)).unwrap();
    f.gov.settle();
    assert_eq!(f.staged().len(), 1, "not removed");
    assert_eq!(f.gov.files().kept_for_test(), 1, "still owned");
    let listed = |gov: &GovernedControl| {
        gov.files_recovery_report().unwrap().iter().any(|entry| {
            entry
                .findings
                .contains(&RecoveryFinding::StagingNotDiscarded)
        })
    };
    assert!(listed(&f.gov), "reported");
    let retry = f.gov.files_retry_cleanup().unwrap();
    assert_eq!((retry.settled, retry.still_owed), (0, 1), "the fault lasts");
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
    let retry = f.gov.files_retry_cleanup().unwrap();
    assert_eq!(
        (retry.settled, retry.still_owed),
        (1, 0),
        "the fault cleared"
    );
    assert!(f.staged().is_empty());
    assert_eq!(f.gov.files().kept_for_test(), 0);
    assert!(!listed(&f.gov));
}
