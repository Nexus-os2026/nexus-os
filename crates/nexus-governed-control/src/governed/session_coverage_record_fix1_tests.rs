//! F11 concrete admitted native record-failure executions. The failure is a
//! real SQLite rejection after this exact broker handoff, not a model outcome.
use super::*;

#[test]
fn fix1_f11_native_record_outcomes_are_executed_after_exact_handoffs() {
    for (sink, entered, returned, event, executing) in [
        (
            "staging_grant",
            "session.entered.staging_grant",
            "session.after.staging_grant",
            "run.granted",
            false,
        ),
        (
            "snapshot",
            "session.entered.snapshot",
            "session.after.snapshot",
            "op.outcome",
            false,
        ),
        (
            "edit",
            "session.entered.edit",
            "session.after.edit",
            "op.outcome",
            false,
        ),
        (
            "structural_verify",
            "session.entered.structural_verify",
            "session.after.structural_verify",
            "verify.structural",
            false,
        ),
        (
            "review",
            "session.entered.review",
            "session.after.review",
            "review.computed",
            false,
        ),
        (
            "revalidation_review",
            "session.entered.revalidation_review",
            "session.after.revalidation_review",
            "review.computed",
            true,
        ),
        (
            "request_approval",
            "session.entered.request_approval",
            "session.after.request_approval",
            "approval.granted",
            true,
        ),
        (
            "apply",
            "session.entered.apply",
            "session.after.apply",
            "apply.completed",
            true,
        ),
    ] {
        for fail_record in [false, true] {
            for emergency_after in [false, true] {
                let f = Fixture::new();
                let pending = executing.then(|| f.pending());
                let ledger = f.ledger_path();
                let before = Arc::new(std::sync::Mutex::new(None));
                let seen = before.clone();
                let path = ledger.clone();
                f.gov.files().hooks().arm(entered, move || {
                    *seen.lock().unwrap() = Some(matching(&events(&path), event, None));
                    if fail_record {
                        let connection = rusqlite::Connection::open(path).unwrap();
                        let sql = format!("CREATE TRIGGER fix1_coverage_record_failure BEFORE INSERT ON coding_run_ledger WHEN NEW.event_kind='{event}' BEGIN SELECT RAISE(ABORT, 'registered native outcome failure'); END;");
                        connection.execute_batch(&sql).unwrap();
                    }
                });
                let returned_count = Arc::new(AtomicUsize::new(0));
                let outcome_count = Arc::new(std::sync::Mutex::new(None));
                let outcome_seen = outcome_count.clone();
                let path = ledger.clone();
                let seen = returned_count.clone();
                let ingress = f._session.ingress();
                f.gov.files().hooks().arm(returned, move || {
                    seen.fetch_add(1, Ordering::SeqCst);
                    *outcome_seen.lock().unwrap() = Some(matching(&events(&path), event, None));
                    if emergency_after {
                        ingress.emergency().unwrap();
                    }
                });
                let success = if let Some(id) = pending {
                    f.approve(id).is_ok()
                } else {
                    f.propose().is_ok()
                };
                let previous = before
                    .lock()
                    .unwrap()
                    .expect("exact native handoff reached");
                assert_eq!(returned_count.load(Ordering::SeqCst), 1, "{sink}");
                let recorded = outcome_count
                    .lock()
                    .unwrap()
                    .expect("exact call return observed")
                    - previous;
                assert_eq!(
                    recorded,
                    usize::from(!fail_record),
                    "{sink}: missing record independently read from verified ledger"
                );
                let target = f._dirs.0.path().join("selected/workspace/session.txt");
                if fail_record {
                    assert!(
                        !success,
                        "{sink}: native record failure cannot be successful"
                    );
                    assert!(!target.exists(), "{sink}: failed preparation/approval cannot publish; failed apply compensates in this fixture");
                    assert!(
                        f._session.resource_obligations().unwrap().unresolved > 0,
                        "{sink}: failed call stays accounted for"
                    );
                } else if !emergency_after {
                    assert!(success, "{sink}: successful positive control");
                }
                println!("F11-NATIVE/{sink}/record_failed={fail_record}/emergency_after={emergency_after}: handed_off=1 returned=1 outcome_records={recorded} target_exists={} unresolved={} conforms=true", target.exists(), f._session.resource_obligations().unwrap().unresolved);
            }
        }
    }
}

#[test]
fn fix1_f11_retry_cleanup_record_failure_retains_debt() {
    const NAME: &str = "governed::session_routes::coverage_record_fix1::fix1_f11_retry_cleanup_record_failure_retains_debt";
    let Ok(stop_at) = std::env::var("M2DD_RETRY_RECORD_CASE") else {
        let root = temp_root("session-retry-preload");
        let lib = root.0.path().join("faults.so");
        let build = std::process::Command::new("cc")
            .args([
                "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror", "-o",
            ])
            .arg(&lib)
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../mother/host/tests/support/faults.c"),
            )
            .args(["-ldl"])
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        for point in ["control", "record_failed", "record_failed_stop"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--nocapture"])
                .env("LD_PRELOAD", &lib)
                .env("M2DD_FAULT", "restore-owed")
                .env("M2DD_RETRY_RECORD_CASE", point)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while child.try_wait().unwrap().is_none() {
                if std::time::Instant::now() >= until {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("bounded retry child: {point}");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{point}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        return;
    };
    let f = Fixture::new();
    let id = f.pending();
    f.approve(id).unwrap();
    let workspace = f._dirs.0.path().join("selected/workspace");
    assert!(f.undo().is_err());
    let before = resources(&workspace);
    assert!(
        !before.is_empty(),
        "failed native restore retains actual owned temporary"
    );
    assert!(!workspace.join("session.txt").exists());
    let entered = Arc::new(AtomicUsize::new(0));
    let seen = entered.clone();
    f.gov
        .files()
        .hooks()
        .arm("session.entered.retry_cleanup", move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
    let discards = Arc::new(AtomicUsize::new(0));
    let seen = discards.clone();
    f.gov
        .files()
        .hooks()
        .arm("session.entered.discard_staging", move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
    let fail_record = stop_at != "control";
    if fail_record {
        let connection = rusqlite::Connection::open(f.ledger_path()).unwrap();
        connection.execute_batch("CREATE TRIGGER fix1_retry_completion_failure BEFORE INSERT ON coding_run_ledger WHEN NEW.event_kind='restore.completed' BEGIN SELECT RAISE(ABORT, 'registered retry record failure'); END;").unwrap();
    }
    if stop_at == "record_failed_stop" {
        let ingress = f._session.ingress();
        f.gov
            .files()
            .hooks()
            .arm("session.after.retry_cleanup", move || {
                ingress.emergency().unwrap();
            });
    }
    // Close retires the old undo index; only its already-owned temporary is
    // eligible. It cannot call restore or remove a published Owner path.
    let quit = f._session.ingress().accept(CommandKind::Quit, 10).unwrap();
    let _result = f._session.settle_close_resources(&quit);
    let expected_retry = 1;
    assert_eq!(entered.load(Ordering::SeqCst), expected_retry, "{stop_at}");
    assert_eq!(
        discards.load(Ordering::SeqCst),
        usize::from(!fail_record),
        "{stop_at}: retry permission cannot authorize discard"
    );
    let after = resources(&workspace);
    if expected_retry == 0 {
        assert_eq!(before, after);
    } else {
        assert!(after.is_empty());
    }
    assert_eq!(
        matching(&events(&f.ledger_path()), "restore.completed", None),
        usize::from(!fail_record)
    );
    if fail_record {
        assert!(f._session.resource_obligations().unwrap().unresolved > 0);
        assert!(f
            ._session
            .inventory()
            .unwrap()
            .entries
            .iter()
            .any(|entry| !entry.discharged));
    }
    println!(
        "F11-RETRY/{stop_at}: native_workers=0 retry={expected_retry} remaining_temporaries={} conforms=true",
        after.len()
    );
    let staging = f.gov.files().state_dir_for_test().join("coding-runs");
    let staged = resources(&staging);
    drop(quit);
    let Fixture {
        gov,
        door,
        _session,
        _root,
        _dirs,
        ..
    } = f;
    drop(door);
    drop(gov);
    drop(_session);
    assert_eq!(
        resources(&workspace),
        after,
        "last-owner drop cannot destroy an owed OwnedTemp"
    );
    assert_eq!(
        resources(&staging),
        staged,
        "last-owner drop cannot discard the retained staging lease"
    );
    drop((_root, _dirs));
}
