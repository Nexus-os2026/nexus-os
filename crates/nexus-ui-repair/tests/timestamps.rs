//! Phase 1.4 Deliverable 1 — chrono-backed timestamps.
//!
//! Asserts that constructing a `SpecialistCall` produces an RFC3339
//! timestamp within one minute of the current wall clock.

use chrono::{DateTime, Utc};
use nexus_ui_repair::specialists::SpecialistCall;

#[test]
fn specialist_call_timestamp_is_real_rfc3339_within_one_minute() {
    let before = Utc::now();
    let call = SpecialistCall::new(
        "vision_judge",
        serde_json::json!({"prompt": "did the click do anything"}),
        serde_json::json!({"verdict": "Changed"}),
    );
    let after = Utc::now();

    let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(&call.timestamp)
        .expect("timestamp parses as RFC3339")
        .with_timezone(&Utc);

    let one_minute = chrono::Duration::minutes(1);
    assert!(
        parsed >= before - one_minute && parsed <= after + one_minute,
        "timestamp {} not within one minute of [{}, {}]",
        parsed,
        before,
        after
    );
}
