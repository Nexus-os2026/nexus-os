//! Phase 1.5 Group A — repair ticket schema.
//!
//! Serde types describing the JSON ticket file that the scout writes
//! after a comparison run. Consumed by Claude Code during the human +
//! Claude Code repair phase.

use serde::{Deserialize, Serialize};

/// Severity levels for a repair ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// Broad category hinting at what kind of fix is needed.
/// Used by Claude Code to prioritise which files to open first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixCategory {
    Wiring,
    MissingEndpoint,
    MissingComponent,
    Labeling,
    UxPolish,
}

/// Minimal DOM context captured at scout time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomContext {
    pub selector: String,
    pub surrounding_markup: String,
}

/// One structured repair ticket produced by the scout for a confirmed
/// or suspected bug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairTicket {
    /// GT-NNN identifier matching the ground truth entry, or SCOUT-NNN
    /// for unknown_new findings.
    pub id: String,
    pub page: String,
    pub sub_view: String,
    pub severity: Severity,
    pub dom_context: DomContext,
    pub error_strings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    pub suggested_fix_category: FixCategory,
    /// Best-guess relative path to the React or Rust file most likely
    /// to contain the fix.
    pub component_file_hint: String,
    pub reproduction_steps: Vec<String>,
}

/// Summary produced by the comparison harness and embedded in the
/// ticket file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub confirmed_match_count: usize,
    pub unknown_new_count: usize,
    pub confirmed_miss_count: usize,
    /// None when the run was partial or when confirmed_match +
    /// confirmed_miss == 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f1_score: Option<f64>,
    pub human_triage_required: usize,
    pub is_partial: bool,
}

/// Reason the scout halted before completing a full run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HaltReason {
    pub reason: String,
    pub halt_at_step: String,
}

/// The top-level ticket file written to disk by the scout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TicketFile {
    pub schema_version: String,
    /// RFC3339 timestamp.
    pub generated_at: String,
    pub scout_version: String,
    pub page: String,
    pub tickets: Vec<RepairTicket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halt: Option<HaltReason>,
    pub comparison_summary: ComparisonSummary,
}

/// Writes a JSON Schema document for [`TicketFile`] to
/// `docs/schemas/repair_ticket_v1.schema.json` relative to the
/// repository root passed in as `repo_root`.
///
/// The schema document is a human-readable reference for Claude Code,
/// not a machine-validated JSON Schema draft.
pub fn write_schema_to_disk(repo_root: &std::path::Path) -> Result<(), String> {
    let schemas_dir = repo_root.join("docs").join("schemas");
    std::fs::create_dir_all(&schemas_dir)
        .map_err(|e| format!("failed to create docs/schemas dir: {e}"))?;

    let schema = serde_json::json!({
        "description": "Nexus OS UI repair ticket file produced by nexus-ui-repair scout v1.0.0. Consumed by Claude Code during the human + Claude Code repair phase.",
        "schema_version": "1.0.0",
        "fields": {
            "schema_version": "Semver string identifying this schema revision.",
            "generated_at": "RFC3339 timestamp recording when the ticket file was written to disk.",
            "scout_version": "Version string of the nexus-ui-repair scout that produced this file.",
            "page": "The Nexus OS page or sub-system this ticket batch targets (e.g. 'chat', 'cli_provider').",
            "tickets": "Array of RepairTicket objects. One per confirmed or suspected bug.",
            "halt": "Optional HaltReason. Present only when the scout halted before finishing a full run.",
            "comparison_summary": "ComparisonSummary produced by the comparison harness. Reports confirmed_match / unknown_new / confirmed_miss counts, F1 score, and whether the run was partial."
        },
        "repair_ticket_fields": {
            "id": "GT-NNN when the ticket matches a ground-truth entry, otherwise SCOUT-NNN for unknown_new findings.",
            "page": "Page identifier — matches TicketFile.page.",
            "sub_view": "Sub-view within the page (e.g. 'sidebar', 'composer').",
            "severity": "One of critical, high, medium, low.",
            "dom_context": "DomContext with a CSS selector and a small slice of surrounding markup.",
            "error_strings": "Array of error strings captured from the DOM or console at scout time.",
            "screenshot_path": "Optional filesystem path to a screenshot captured at scout time.",
            "suggested_fix_category": "One of wiring, missing_endpoint, missing_component, labeling, ux_polish.",
            "component_file_hint": "Best-guess relative path to the React or Rust file most likely to contain the fix.",
            "reproduction_steps": "Ordered array of reproduction step strings."
        }
    });

    let path = schemas_dir.join("repair_ticket_v1.schema.json");
    let pretty = serde_json::to_string_pretty(&schema)
        .map_err(|e| format!("failed to serialise schema: {e}"))?;
    std::fs::write(&path, pretty).map_err(|e| format!("failed to write schema file: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ticket() -> RepairTicket {
        RepairTicket {
            id: "GT-001".into(),
            page: "chat".into(),
            sub_view: "sidebar".into(),
            severity: Severity::High,
            dom_context: DomContext {
                selector: "#new-chat".into(),
                surrounding_markup: "<button id=\"new-chat\">+</button>".into(),
            },
            error_strings: vec!["onClick is not a function".into()],
            screenshot_path: Some("/tmp/shot.png".into()),
            suggested_fix_category: FixCategory::Wiring,
            component_file_hint: "app/src/pages/Chat.tsx".into(),
            reproduction_steps: vec!["Open chat".into(), "Click +".into()],
        }
    }

    fn sample_summary() -> ComparisonSummary {
        ComparisonSummary {
            confirmed_match_count: 1,
            unknown_new_count: 0,
            confirmed_miss_count: 0,
            f1_score: Some(1.0),
            human_triage_required: 0,
            is_partial: false,
        }
    }

    #[test]
    fn test_severity_serialises_to_snake_case() {
        let s = serde_json::to_string(&Severity::Critical).unwrap();
        assert_eq!(s, "\"critical\"");
    }

    #[test]
    fn test_fix_category_serialises_to_snake_case() {
        let s = serde_json::to_string(&FixCategory::MissingEndpoint).unwrap();
        assert_eq!(s, "\"missing_endpoint\"");
    }

    #[test]
    fn test_repair_ticket_round_trip() {
        let t = sample_ticket();
        let json = serde_json::to_string(&t).unwrap();
        let back: RepairTicket = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn test_ticket_file_halt_none_omitted() {
        let tf = TicketFile {
            schema_version: "1.0.0".into(),
            generated_at: "2026-04-09T00:00:00Z".into(),
            scout_version: "0.5.0".into(),
            page: "chat".into(),
            tickets: vec![sample_ticket()],
            halt: None,
            comparison_summary: sample_summary(),
        };
        let json = serde_json::to_string(&tf).unwrap();
        assert!(
            !json.contains("\"halt\""),
            "halt: None should be omitted from JSON, got: {json}"
        );
    }
}
