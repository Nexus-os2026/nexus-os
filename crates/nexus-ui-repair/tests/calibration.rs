//! Calibration log tests. See v1.1 amendment §8 Q3.

use nexus_ui_repair::governance::{CalibrationEntry, CalibrationLog};

fn make_entry(call_id: &str, similarity: f64) -> CalibrationEntry {
    CalibrationEntry {
        call_id: call_id.to_string(),
        timestamp: "2026-04-07T12:00:00Z".to_string(),
        similarity,
        threshold: 0.85,
        verdict: "Changed".to_string(),
        ground_truth: None,
        classifier_input: None,
    }
}

#[test]
fn records_and_reads_back_one_entry() {
    let dir = tempfile::TempDir::new().unwrap();
    let log = CalibrationLog::new(dir.path().join("calibration.jsonl"));

    let entry = make_entry("call_001", 0.87);
    log.record(&entry).expect("record");

    let read = log.entries().expect("read entries");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].call_id, "call_001");
    assert!((read[0].similarity - 0.87).abs() < f64::EPSILON);
}

#[test]
fn records_multiple_entries_in_order() {
    let dir = tempfile::TempDir::new().unwrap();
    let log = CalibrationLog::new(dir.path().join("calibration.jsonl"));

    for i in 0..5 {
        log.record(&make_entry(
            &format!("call_{:03}", i),
            0.8 + (i as f64) * 0.01,
        ))
        .expect("record");
    }

    let read = log.entries().expect("read entries");
    assert_eq!(read.len(), 5);
    for (i, entry) in read.iter().enumerate() {
        assert_eq!(entry.call_id, format!("call_{:03}", i));
    }
}

#[test]
fn empty_log_returns_empty_vec() {
    let dir = tempfile::TempDir::new().unwrap();
    let log = CalibrationLog::new(dir.path().join("nonexistent.jsonl"));
    assert_eq!(log.entries().expect("empty").len(), 0);
}
