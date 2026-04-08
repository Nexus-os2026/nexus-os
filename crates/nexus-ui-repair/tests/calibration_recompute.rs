//! Phase 1.4 Deliverable 6 — calibration recompute tests.

use std::path::PathBuf;

use std::sync::{Arc, Mutex};

use nexus_ui_repair::governance::calibration::{
    predict_with_thresholds, CalibrationError, CalibrationLog, RecomputeReport,
};
use nexus_ui_repair::specialists::classifier::{
    Classification, Classifier, ClassifierInput, DomMutation, IpcEvent, DEFAULT_HANG_THRESHOLD_MS,
    DEFAULT_MIN_ACTION_WINDOW_MS,
};
use nexus_ui_repair::specialists::vision_judge::{VisionVerdict, VisionVerdictKind};
use tempfile::tempdir;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn synthetic_log(dir: &std::path::Path) -> CalibrationLog {
    let path = dir.join("cal.jsonl");
    std::fs::copy(fixture("calibration_synthetic.jsonl"), &path).expect("copy fixture");
    CalibrationLog::new(path)
}

#[test]
fn recompute_on_synthetic_picks_thresholds_near_defaults() {
    let dir = tempdir().expect("tempdir");
    let log = synthetic_log(dir.path());
    let gt = fixture("calibration_ground_truth.jsonl");
    let out = dir.path().join("report.json");
    let report = log.recompute_thresholds(&gt, &out).expect("recompute");
    // Defaults are min=500, hang=10000. Allow ±100ms / ±1000ms slack.
    assert!(
        (report.min_action_window_ms as i64 - 500).abs() <= 100,
        "min_action_window_ms={} not within 100 of 500",
        report.min_action_window_ms
    );
    assert!(
        (report.hang_threshold_ms as i64 - 10_000).abs() <= 1000,
        "hang_threshold_ms={} not within 1000 of 10000",
        report.hang_threshold_ms
    );
    assert!(
        report.f1_score > 0.8,
        "f1_score={} too low on synthetic fixture",
        report.f1_score
    );
    assert!(out.exists());
    let parsed: RecomputeReport =
        serde_json::from_slice(&std::fs::read(&out).unwrap()).expect("parse report");
    assert_eq!(parsed.joined_entries, 20);
}

#[test]
fn recompute_missing_ground_truth_returns_error() {
    let dir = tempdir().expect("tempdir");
    let log = synthetic_log(dir.path());
    let out = dir.path().join("r.json");
    let err = log
        .recompute_thresholds(&dir.path().join("nope.jsonl"), &out)
        .unwrap_err();
    assert!(matches!(err, CalibrationError::GroundTruthMissing));
}

/// Guard against silent drift between `Classifier::classify` (the
/// production rule evaluator) and `calibration::predict_with_thresholds`
/// (the sweep-time re-implementation). See Phase 1.4 Group B Deviation 2:
/// the rule table is duplicated by hand until a future refactor lifts
/// it to a free function; until then, this test is the only thing
/// catching a rule change made in one place and not the other.
#[test]
fn classifier_rules_match_sweep_predictor() {
    let tmp = tempdir().expect("tempdir");
    let log_path = tmp.path().join("cal_sync.jsonl");
    let log = Arc::new(Mutex::new(CalibrationLog::new(log_path)));
    let classifier = Classifier::new(log);

    let min = DEFAULT_MIN_ACTION_WINDOW_MS;
    let hang = DEFAULT_HANG_THRESHOLD_MS;

    fn verdict(kind: VisionVerdictKind) -> VisionVerdict {
        VisionVerdict {
            verdict: kind,
            confidence: 0.9,
            reasoning: "t".into(),
            detected_changes: vec![],
        }
    }

    let cases: Vec<(&str, ClassifierInput)> = vec![
        (
            "error_wins_on_console",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Changed),
                console_errors: vec!["TypeError: x".into()],
                ipc_traffic: vec![IpcEvent {
                    command: "cmd".into(),
                }],
                dom_mutations: vec![DomMutation {
                    selector: "#n".into(),
                }],
                elapsed_ms: 800,
                signal_change_after_action: true,
            },
        ),
        (
            "hang_on_long_no_signal",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Unchanged),
                console_errors: vec![],
                ipc_traffic: vec![],
                dom_mutations: vec![],
                elapsed_ms: hang + 500,
                signal_change_after_action: false,
            },
        ),
        (
            "dead_on_no_activity",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Unchanged),
                console_errors: vec![],
                ipc_traffic: vec![],
                dom_mutations: vec![],
                elapsed_ms: 300,
                signal_change_after_action: false,
            },
        ),
        (
            "pass_on_changed_with_dom",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Changed),
                console_errors: vec![],
                ipc_traffic: vec![IpcEvent {
                    command: "cmd".into(),
                }],
                dom_mutations: vec![DomMutation {
                    selector: "#m".into(),
                }],
                elapsed_ms: min + 10,
                signal_change_after_action: true,
            },
        ),
        (
            "ambiguous_fallthrough",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Ambiguous),
                console_errors: vec![],
                ipc_traffic: vec![IpcEvent {
                    command: "cmd".into(),
                }],
                dom_mutations: vec![DomMutation {
                    selector: "#a".into(),
                }],
                elapsed_ms: 200,
                signal_change_after_action: true,
            },
        ),
        (
            "edge_elapsed_equals_min_window",
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Changed),
                console_errors: vec![],
                ipc_traffic: vec![IpcEvent {
                    command: "cmd".into(),
                }],
                dom_mutations: vec![DomMutation {
                    selector: "#e".into(),
                }],
                elapsed_ms: min,
                signal_change_after_action: true,
            },
        ),
        (
            "edge_elapsed_equals_hang_threshold",
            // elapsed_ms > hang_threshold is the rule, so equal must NOT
            // trigger Hang. Combined with no vision change / no DOM,
            // this should fall to Dead.
            ClassifierInput {
                vision_verdict: verdict(VisionVerdictKind::Unchanged),
                console_errors: vec![],
                ipc_traffic: vec![],
                dom_mutations: vec![],
                elapsed_ms: hang,
                signal_change_after_action: false,
            },
        ),
    ];

    for (name, input) in &cases {
        let production: Classification = classifier.classify(input);
        let sweep: Classification = predict_with_thresholds(input, min, hang);
        assert_eq!(
            production, sweep,
            "rule drift on case '{}': classifier.classify => {:?}, \
             calibration::predict_with_thresholds => {:?}",
            name, production, sweep
        );
    }
}

#[test]
fn recompute_empty_calibration_log_returns_error() {
    let dir = tempdir().expect("tempdir");
    let log = CalibrationLog::new(dir.path().join("empty.jsonl"));
    // Touch the file so it exists but is empty.
    std::fs::write(dir.path().join("empty.jsonl"), b"").unwrap();
    let gt = fixture("calibration_ground_truth.jsonl");
    let out = dir.path().join("r.json");
    let err = log.recompute_thresholds(&gt, &out).unwrap_err();
    assert!(matches!(err, CalibrationError::CalibrationLogEmpty));
}
