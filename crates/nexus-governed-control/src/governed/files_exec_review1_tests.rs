//! Oracles for Codex review 1 of PR #64 (M2d-B2), pre-registered before the
//! repairs, with the Architect's decisions: H1 (Stop's acknowledgement does
//! not wait on evidence storage), H2 (an explicit publication outcome; an
//! uncertain one is never a plain failure), H3 (an interruption after the
//! handoff is reported beside the success and ends Files for the run), H4
//! (the undo's lifetime is taken before its cleanup), M1 (a failed staging
//! cleanup stays owned, is reported and has the Owner's bounded retry).
//! A child of the B2 oracles, for their fixture.

use super::*;
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::{Fault, PrimaryEvidence};

/// The bound within which a Stop must return while the store is blocked.
const STOP_ACKNOWLEDGED_WITHIN: Duration = Duration::from_secs(1);

/// A front door on the evidence store `primary`.
fn on_store(primary: &Arc<PrimaryEvidence>, root: &TempRoot) -> Arc<GovernedControl> {
    Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            primary.clone(),
            Arc::new(ManualClock::default()),
        )
        .unwrap(),
    )
}

/// Wait (bounded) until no Stop record is waiting to be written.
fn settled_records(gov: &GovernedControl) {
    assert!(
        gov.stop_records_idle_for_test(Duration::from_secs(10)),
        "the Stop records' writer settles"
    );
}

// ---------------------------------------------------------------- H1

/// H1: with the evidence store blocked inside its mutex at COMMIT (a slow
/// disk), the Owner's Stop returns within 1 s with the halt engaged and its
/// record pending; once the store is released the record is written.
#[test]
fn m2d_b2_r1_h1_a_stop_returns_while_the_store_is_blocked() {
    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(10_000));
    let primary = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let root = temp_root("m2d-b2-r1-h1");
    let gov = on_store(&primary, &root);
    let stall = Gate::new();
    primary.store().plant_fault(Fault::Stall(stall.clone()));
    let (tx, rx) = mpsc::channel();
    let stopping = gov.clone();
    let worker = std::thread::spawn(move || {
        stopping.emergency_stop();
        tx.send(()).unwrap();
    });
    stall.reached();
    rx.recv_timeout(STOP_ACKNOWLEDGED_WITHIN)
        .expect("Stop returns within 1 s while the store is blocked");
    let status = gov.status();
    assert!(status.emergency_stopped, "the halt is engaged");
    assert!(status.stop_records.pending >= 1, "its record is pending");
    assert_eq!(status.stop_records.state(), "stop_record_pending");
    assert!(!recent
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
    stall.open();
    worker.join().unwrap();
    settled_records(&gov);
    let status = gov.status();
    assert_eq!(status.stop_records.pending, 0);
    assert!(status.stop_records.written >= 1);
    assert_eq!(status.stop_records.state(), "stop_records_written");
    assert!(recent
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
}

/// H1: a store that fails its Stop record yields the surfaced failure
/// status (never swallowed); one that cannot commit keeps it owed (pending)
/// and records it when storage recovers.
#[test]
fn m2d_b2_r1_h1_b_a_failing_store_is_surfaced_and_recorded_on_recovery() {
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let root = temp_root("m2d-b2-r1-h1b");
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            evidence.clone(),
            Arc::new(ManualClock::default()),
        )
        .unwrap(),
    );
    evidence.fail_from_now();
    gov.emergency_stop();
    settled_records(&gov);
    let status = gov.status();
    assert!(status.emergency_stopped);
    assert!(status.stop_records.failed >= 1, "surfaced, not swallowed");
    assert_eq!(status.stop_records.state(), "stop_record_failed");

    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(10_000));
    let primary = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let root = temp_root("m2d-b2-r1-h1c");
    let gov = on_store(&primary, &root);
    primary.store().refuse_inserts(true);
    gov.emergency_stop();
    settled_records(&gov);
    let status = gov.status();
    assert!(status.stop_records.owed >= 1, "owed while storage fails");
    assert_eq!(status.stop_records.state(), "stop_record_pending");
    primary.store().refuse_inserts(false);
    gov.resume(&*Owner::new())
        .expect("the resume pays what is owed first");
    assert!(recent
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::EmergencyStop));
}

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
