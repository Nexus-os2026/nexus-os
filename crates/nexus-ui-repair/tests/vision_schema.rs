//! Phase 1.4 Deliverable 3 — vision verdict schema.

use nexus_ui_repair::specialists::vision_schema::{
    write_schema_to_disk, SCHEMA_VERSION, VISION_VERDICT_SCHEMA,
};
use tempfile::tempdir;

#[test]
fn schema_is_valid_json() {
    let v: serde_json::Value =
        serde_json::from_str(VISION_VERDICT_SCHEMA).expect("schema parses as JSON");
    assert_eq!(v["title"], "VisionVerdict");
}

#[test]
fn write_schema_to_disk_creates_file_and_round_trips() {
    let dir = tempdir().expect("tempdir");
    let nested = dir.path().join("subdir").join("schema.json");
    write_schema_to_disk(&nested).expect("write schema");
    assert!(nested.exists());
    let bytes = std::fs::read(&nested).expect("read schema");
    let parsed: serde_json::Value =
        serde_json::from_slice(&bytes).expect("written file is valid JSON");
    assert!(parsed["properties"]["verdict"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "Ambiguous"));
}

#[test]
fn schema_version_is_set() {
    assert!(!SCHEMA_VERSION.is_empty());
    assert!(SCHEMA_VERSION.starts_with('v'));
}
