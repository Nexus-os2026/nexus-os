//! Phase 1.4 Deliverable 3 — JSON schema for `VisionVerdict`.
//!
//! This is the schema passed to `codex exec --output-schema FILE` so
//! Codex CLI's structured output is byte-shape-validated before it
//! reaches Rust. Audit log entries record [`SCHEMA_VERSION`] alongside
//! every call so future migrations can be replayed against the schema
//! that produced them.

use std::path::Path;

/// Schema version recorded in audit log entries. Bump on any
/// breaking change to the schema string.
pub const SCHEMA_VERSION: &str = "v1.0.0";

/// JSON Schema (draft 2020-12 compatible) for the `VisionVerdict`
/// structure that `vision_judge` expects from Codex CLI.
pub const VISION_VERDICT_SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "VisionVerdict",
  "type": "object",
  "additionalProperties": false,
  "required": ["verdict", "confidence", "reasoning", "detected_changes"],
  "properties": {
    "verdict": {
      "type": "string",
      "enum": ["Changed", "Unchanged", "Error", "Ambiguous"]
    },
    "confidence": {
      "type": "number",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "reasoning": {
      "type": "string"
    },
    "detected_changes": {
      "type": "array",
      "items": { "type": "string" }
    }
  }
}"#;

/// Write the schema string to `path`, creating parent directories if
/// they do not already exist.
pub fn write_schema_to_disk(path: &Path) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, VISION_VERDICT_SCHEMA)?;
    Ok(())
}
