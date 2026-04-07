//! SpecialistCall + audit log integration tests. See v1.1 §4.1 / I-5.

use nexus_ui_repair::governance::audit::{AuditEntry, AuditLog};
use nexus_ui_repair::specialists::specialist_call::SpecialistCall;

#[test]
fn records_two_specialist_calls_with_chained_hashes() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut log = AuditLog::new(path.clone());

    let call1 = SpecialistCall::new(
        "enumerator",
        serde_json::json!({ "page": "/builder/teams" }),
        serde_json::json!({ "elements_count": 7 }),
    );
    let call2 = SpecialistCall::new(
        "vision_judge",
        serde_json::json!({ "before_hash": "abc", "after_hash": "def" }),
        serde_json::json!({ "verdict": "Changed", "similarity": 0.42 }),
    );

    log.record_specialist_call(call1).expect("record 1");
    log.record_specialist_call(call2).expect("record 2");

    let contents = std::fs::read_to_string(&path).expect("read log");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "two audit lines expected");

    let first: AuditEntry = serde_json::from_str(lines[0]).expect("parse first");
    let second: AuditEntry = serde_json::from_str(lines[1]).expect("parse second");

    // Chain integrity: second.prev_hash == first.hash.
    assert_eq!(second.prev_hash, first.hash);
    assert_ne!(first.hash, "0".repeat(64));

    // Both must carry the specialist name.
    assert_eq!(first.specialist.as_deref(), Some("enumerator"));
    assert_eq!(second.specialist.as_deref(), Some("vision_judge"));
    assert_eq!(first.state, "specialist_call");
}

#[test]
fn output_capture_makes_replay_byte_identical() {
    // With identical inputs AND identical outputs (and the Phase 1.3
    // placeholder timestamp), two separate audit logs produce
    // byte-identical files. This is the core I-5 determinism claim.
    let dir = tempfile::TempDir::new().unwrap();
    let path1 = dir.path().join("audit1.jsonl");
    let path2 = dir.path().join("audit2.jsonl");

    let inputs = serde_json::json!({ "input": "same" });
    let output = serde_json::json!({ "output": "same" });

    let mut log1 = AuditLog::new(path1.clone());
    let mut log2 = AuditLog::new(path2.clone());

    log1.record_specialist_call(SpecialistCall::new("test", inputs.clone(), output.clone()))
        .expect("record 1");
    log2.record_specialist_call(SpecialistCall::new("test", inputs, output))
        .expect("record 2");

    let c1 = std::fs::read_to_string(&path1).expect("read 1");
    let c2 = std::fs::read_to_string(&path2).expect("read 2");
    assert_eq!(
        c1, c2,
        "same (inputs, output, timestamp) must produce byte-identical audit lines"
    );
}
