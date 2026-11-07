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
#[test]
fn sb2_close_report_has_one_model_line() {
    let none = nexus_governed_control::host_session::close::ModelObligations {
        operation: "none",
        active: false,
        owner_complete: true,
        child_complete: true,
        record: "not_required",
        uncertain: false,
    };
    for snapshot in [None, Some(none)] {
        for failures in 0..=2 {
            let report = format!(
                "{}{}",
                CLOSE_UNKNOWN.repeat(failures),
                model_report(snapshot)
            );
            assert_eq!(report.matches("CLOSE MODEL ").count(), 1, "{report}");
        }
    }
}
