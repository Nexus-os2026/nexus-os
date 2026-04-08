//! Phase 1.4 Deliverable 5 — classifier 5-rule table tests.

use std::sync::{Arc, Mutex};

use nexus_ui_repair::governance::calibration::CalibrationLog;
use nexus_ui_repair::specialists::classifier::{
    Classification, Classifier, ClassifierInput, DomMutation, IpcEvent,
};
use nexus_ui_repair::specialists::vision_judge::{VisionVerdict, VisionVerdictKind};
use tempfile::tempdir;

fn build_classifier() -> (Classifier, tempfile::TempDir, Arc<Mutex<CalibrationLog>>) {
    let dir = tempdir().expect("tempdir");
    let log = Arc::new(Mutex::new(CalibrationLog::new(
        dir.path().join("cal.jsonl"),
    )));
    let c = Classifier::new(Arc::clone(&log));
    (c, dir, log)
}

fn verdict(kind: VisionVerdictKind) -> VisionVerdict {
    VisionVerdict {
        verdict: kind,
        confidence: 0.9,
        reasoning: "test".into(),
        detected_changes: vec![],
    }
}

fn base_input() -> ClassifierInput {
    ClassifierInput {
        vision_verdict: verdict(VisionVerdictKind::Changed),
        console_errors: vec![],
        ipc_traffic: vec![IpcEvent {
            command: "x".into(),
        }],
        dom_mutations: vec![DomMutation {
            selector: "#x".into(),
        }],
        elapsed_ms: 800,
        signal_change_after_action: true,
    }
}

#[test]
fn rule1_console_error_yields_error() {
    let (c, _d, _l) = build_classifier();
    let mut i = base_input();
    i.console_errors.push("TypeError: x is undefined".into());
    assert_eq!(c.classify(&i), Classification::Error);
}

#[test]
fn rule1_no_console_error_does_not_yield_error() {
    let (c, _d, _l) = build_classifier();
    let i = base_input();
    assert_ne!(c.classify(&i), Classification::Error);
}

#[test]
fn rule2_long_elapsed_no_signal_yields_hang() {
    let (c, _d, _l) = build_classifier();
    let mut i = base_input();
    i.elapsed_ms = 15_000;
    i.signal_change_after_action = false;
    assert_eq!(c.classify(&i), Classification::Hang);
}

#[test]
fn rule2_long_elapsed_with_signal_does_not_yield_hang() {
    let (c, _d, _l) = build_classifier();
    let mut i = base_input();
    i.elapsed_ms = 15_000;
    i.signal_change_after_action = true;
    assert_ne!(c.classify(&i), Classification::Hang);
}

#[test]
fn rule3_no_signal_anywhere_yields_dead() {
    let (c, _d, _l) = build_classifier();
    let i = ClassifierInput {
        vision_verdict: verdict(VisionVerdictKind::Unchanged),
        console_errors: vec![],
        ipc_traffic: vec![],
        dom_mutations: vec![],
        elapsed_ms: 800,
        signal_change_after_action: false,
    };
    assert_eq!(c.classify(&i), Classification::Dead);
}

#[test]
fn rule3_with_dom_mutation_does_not_yield_dead() {
    let (c, _d, _l) = build_classifier();
    let mut i = base_input();
    i.vision_verdict = verdict(VisionVerdictKind::Unchanged);
    i.ipc_traffic.clear();
    // dom_mutations still non-empty from base_input
    assert_ne!(c.classify(&i), Classification::Dead);
}

#[test]
fn rule4_changed_with_mutations_yields_pass() {
    let (c, _d, _l) = build_classifier();
    let i = base_input();
    assert_eq!(c.classify(&i), Classification::Pass);
}

#[test]
fn rule4_changed_but_too_fast_does_not_pass() {
    let (c, _d, _l) = build_classifier();
    let mut i = base_input();
    i.elapsed_ms = 100; // below default 500ms window
    assert_ne!(c.classify(&i), Classification::Pass);
}

#[test]
fn rule5_everything_else_is_ambiguous() {
    let (c, _d, _l) = build_classifier();
    let i = ClassifierInput {
        vision_verdict: verdict(VisionVerdictKind::Ambiguous),
        console_errors: vec![],
        ipc_traffic: vec![IpcEvent {
            command: "x".into(),
        }],
        dom_mutations: vec![],
        elapsed_ms: 800,
        signal_change_after_action: true,
    };
    assert_eq!(c.classify(&i), Classification::Ambiguous);
}

#[test]
fn rule5_clearly_pass_is_not_ambiguous() {
    let (c, _d, _l) = build_classifier();
    let i = base_input();
    assert_ne!(c.classify(&i), Classification::Ambiguous);
}

#[test]
fn calibration_log_receives_entry_per_classify() {
    let (c, _d, log) = build_classifier();
    c.classify(&base_input());
    c.classify(&base_input());
    let entries = log.lock().unwrap().entries().expect("entries");
    assert_eq!(entries.len(), 2);
}
