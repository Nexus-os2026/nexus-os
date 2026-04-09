//! Phase 1.5 Group A — ticket writer specialist.
//!
//! Writes [`crate::repair_ticket::schema::TicketFile`] records to disk
//! as pretty-printed JSON. Supports a DryRun mode that returns the
//! serialised payload without touching the filesystem — used by tests
//! and by scout runs that have not yet obtained HITL approval.

use std::path::{Path, PathBuf};

use crate::repair_ticket::schema::TicketFile;

/// Output mode for the ticket writer.
#[derive(Debug, Clone)]
pub enum WriteMode {
    /// Write the JSON payload to the given path.
    File(PathBuf),
    /// Do not touch the filesystem. Used by tests and unapproved runs.
    DryRun,
}

/// Successful write outcome.
#[derive(Debug, Clone)]
pub struct WriteOutcome {
    /// Path the ticket file was (or would have been) written to.
    pub path: Option<PathBuf>,
    /// Pretty-printed JSON payload.
    pub payload: String,
}

/// Error returned by [`write_ticket_file`].
#[derive(Debug)]
pub enum WriteError {
    SerializeError(String),
    IoError(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::SerializeError(m) => write!(f, "serialize error: {m}"),
            WriteError::IoError(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Serialises the ticket file and dispatches based on `mode`.
pub fn write_ticket_file(
    ticket: &TicketFile,
    mode: &WriteMode,
) -> Result<WriteOutcome, WriteError> {
    let payload = serde_json::to_string_pretty(ticket)
        .map_err(|e| WriteError::SerializeError(e.to_string()))?;

    match mode {
        WriteMode::File(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| WriteError::IoError(e.to_string()))?;
                }
            }
            std::fs::write(path, &payload).map_err(|e| WriteError::IoError(e.to_string()))?;
            Ok(WriteOutcome {
                path: Some(path.clone()),
                payload,
            })
        }
        WriteMode::DryRun => Ok(WriteOutcome {
            path: None,
            payload,
        }),
    }
}

/// Convenience helper: write to a path given as `&Path`.
pub fn write_ticket_file_to_path(
    ticket: &TicketFile,
    path: &Path,
) -> Result<WriteOutcome, WriteError> {
    write_ticket_file(ticket, &WriteMode::File(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repair_ticket::schema::{
        ComparisonSummary, DomContext, FixCategory, RepairTicket, Severity,
    };

    fn sample_ticket_file() -> TicketFile {
        TicketFile {
            schema_version: "1.0.0".into(),
            generated_at: "2026-04-09T00:00:00Z".into(),
            scout_version: "0.5.0".into(),
            page: "chat".into(),
            tickets: vec![RepairTicket {
                id: "GT-001".into(),
                page: "chat".into(),
                sub_view: "compare".into(),
                severity: Severity::High,
                dom_context: DomContext {
                    selector: "#compare".into(),
                    surrounding_markup: "<div id=\"compare\"></div>".into(),
                },
                error_strings: vec!["404 page not found".into()],
                screenshot_path: None,
                suggested_fix_category: FixCategory::MissingEndpoint,
                component_file_hint: "app/src/pages/Chat.tsx".into(),
                reproduction_steps: vec!["open compare".into()],
            }],
            halt: None,
            comparison_summary: ComparisonSummary {
                confirmed_match_count: 1,
                unknown_new_count: 0,
                confirmed_miss_count: 0,
                f1_score: Some(1.0),
                human_triage_required: 0,
                is_partial: false,
            },
        }
    }

    #[test]
    fn test_dry_run_does_not_write_file() {
        let outcome = write_ticket_file(&sample_ticket_file(), &WriteMode::DryRun).unwrap();
        assert!(outcome.path.is_none());
        assert!(outcome.payload.contains("\"GT-001\""));
    }

    #[test]
    fn test_file_mode_writes_and_roundtrips() {
        let tmp =
            std::env::temp_dir().join(format!("nexus-ui-repair-test-{}.json", std::process::id()));
        let outcome = write_ticket_file_to_path(&sample_ticket_file(), &tmp).unwrap();
        assert_eq!(outcome.path.as_deref(), Some(tmp.as_path()));

        let on_disk = std::fs::read_to_string(&tmp).unwrap();
        assert_eq!(on_disk, outcome.payload);

        let back: TicketFile = serde_json::from_str(&on_disk).unwrap();
        assert_eq!(back.tickets.len(), 1);
        assert_eq!(back.tickets[0].id, "GT-001");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_file_mode_creates_missing_parent_dirs() {
        let tmp_dir =
            std::env::temp_dir().join(format!("nexus-ui-repair-nested-{}", std::process::id()));
        let nested = tmp_dir.join("sub").join("ticket.json");
        let outcome = write_ticket_file_to_path(&sample_ticket_file(), &nested).unwrap();
        assert!(nested.exists());
        assert!(outcome.payload.contains("GT-001"));
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
