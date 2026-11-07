//! O4: formatter snapshots only; composed worker timeout remains pending SB-3.
use super::*;
#[test]
fn sb2_shutdown_fallback_reports_model_unknown_when_close_worker_is_skipped() {
    let report = model_report(None);
    for fact in [
        "operation=unknown",
        "owner=unknown",
        "child=unknown",
        "record=unknown",
        "uncertain=true",
    ] {
        assert!(report.contains(fact), "fallback {fact}");
        assert!(close_fallback_report().contains(fact), "fallback {fact}");
    }
    assert!(!report.contains("SESSION CLOSED recorded"));
}
#[test]
fn sb2_completed_registration_report_preserves_record_fact() {
    let report = model_report(Some(
        nexus_governed_control::host_session::close::ModelObligations {
            operation: "registration",
            active: false,
            owner_complete: true,
            child_complete: true,
            record: "recorded",
            uncertain: false,
        },
    ));
    assert!(
        report.contains("record=recorded")
            && report.contains("owner=true")
            && report.contains("child=true")
    );
}
#[test]
fn sb2_failed_startup_reports_model_unknown() {
    assert!(failed_startup_report().contains(MODEL_UNKNOWN));
}
// Crate-local O4 error snapshots; real close_report still performs its reads.
thread_local! { static SNAPSHOT_FAILURES: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }
pub(super) fn observed<T, E>(index: u8, result: Result<T, E>) -> Result<T, ()> {
    if SNAPSHOT_FAILURES.with(|f| f.get() & (1 << index) != 0) {
        Err(())
    } else {
        result.map_err(|_| ())
    }
}

#[test]
fn sb2_close_report_has_one_model_line() {
    use std::sync::{
        atomic::{AtomicBool, AtomicU64},
        mpsc, Condvar,
    };
    let fixture = crate::stop_tests::support::Fixture::new();
    let seed = SessionSeed::new().unwrap();
    let (output, _frames) = mpsc::sync_channel(64);
    let screen = Arc::new(Shared {
        stop: crate::stop::StopRoute::new(seed.ingress()),
        input: Mutex::default(),
        sequence: AtomicU64::new(0),
        changed: Condvar::new(),
        output,
        end: AtomicBool::new(false),
        input_end: AtomicBool::new(false),
        dialogue: Mutex::new(()),
        queued_output: AtomicU64::new(0),
        delivered_output: AtomicU64::new(0),
        output_failed: AtomicBool::new(false),
    });
    let confirmer = Arc::new(crate::terminal::TerminalConfirmer::new(screen.clone()));
    let home = Arc::new(SessionServices::open(seed, &fixture.home, confirmer.clone()).unwrap());
    let runtime = Runtime {
        home,
        screen,
        confirmer,
        used: false,
        mission: None,
        config: Config {
            home: fixture.home.clone(),
            workspace: fixture.workspace.clone(),
            name: "report.txt".into(),
            text: String::new(),
            hold: Duration::ZERO,
        },
    };
    for unknown in [false, true] {
        for failures in 0..=2 {
            SNAPSHOT_FAILURES.with(|f| f.set(((1 << failures) - 1) | if unknown { 4 } else { 0 }));
            let report = runtime.close_report("test");
            SNAPSHOT_FAILURES.with(|f| f.set(0));
            assert_eq!(report.matches("CLOSE MODEL ").count(), 1, "{report}");
            assert_eq!(report.contains(MODEL_UNKNOWN), unknown);
        }
    }
}

#[test]
fn sb2_attempt_fallback_keeps_all_debt_statuses() {
    for record in ["pending", "recorded", "owed", "uncertain", "unknown"] {
        let report = model_report(Some(
            nexus_governed_control::host_session::close::ModelObligations {
                operation: "attempt",
                active: true,
                owner_complete: false,
                child_complete: true,
                record,
                uncertain: record != "recorded",
            },
        ));
        assert!(report.contains(&format!("record={record}")));
        assert!(report.contains("operation=attempt active=true owner=false child=true"));
        assert!(!report.contains("SESSION CLOSED recorded"));
    }
    assert!(close_fallback_report().contains("record=unknown"));
}
