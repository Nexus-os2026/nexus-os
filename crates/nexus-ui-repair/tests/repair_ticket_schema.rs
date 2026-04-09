//! Phase 1.5 Group A — integration test that writes the repair ticket
//! JSON schema document to `docs/schemas/repair_ticket_v1.schema.json`.
//!
//! This test has a side effect by design: it creates (and keeps) the
//! schema document on disk so Claude Code can consume it from the
//! repository. The file is tracked in git.

use std::path::PathBuf;

use nexus_ui_repair::repair_ticket::schema::write_schema_to_disk;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn writes_repair_ticket_schema_to_docs_schemas() {
    let root = repo_root();
    write_schema_to_disk(&root).expect("write_schema_to_disk must succeed");

    let schema_path = root
        .join("docs")
        .join("schemas")
        .join("repair_ticket_v1.schema.json");
    assert!(
        schema_path.exists(),
        "schema file must exist at {:?}",
        schema_path
    );

    let bytes = std::fs::read(&schema_path).expect("read schema file");
    let parsed: serde_json::Value =
        serde_json::from_slice(&bytes).expect("schema file must be valid JSON");

    assert_eq!(parsed["schema_version"], "1.0.0");
    assert!(parsed["repair_ticket_fields"]["severity"].is_string());
    assert!(parsed["fields"]["tickets"].is_string());
}
