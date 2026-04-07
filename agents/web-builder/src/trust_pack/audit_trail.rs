//! Audit Trail — consolidates all governance events into a sorted timeline.
//!
//! Events are collected from project state, quality reports, visual edits,
//! deploy history, and other governance data scattered across project files.

use crate::project::ProjectState;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Types ──────────────────────────────────────────────────────────────────

/// A single governance event in the audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub event_type: AuditEventType,
    pub description: String,
    pub details: serde_json::Value,
}

/// The type of governance event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    BuildStarted,
    BuildCompleted,
    QualityCheck,
    AutoFix,
    VisualEdit,
    TextEdit,
    ThemeChange,
    VariantGenerated,
    VariantSelected,
    BackendGenerated,
    Deployed,
    Rollback,
    ImageGenerated,
    DesignImported,
    Exported,
    Archived,
    // Phase 14: Collaboration events
    CollabSessionStarted,
    CollabSessionEnded,
    CollabComment,
    CollabRoleChanged,
    // Phase 16: Self-Improvement events
    ImprovementAnalysisRun,
    ImprovementProposed,
    ImprovementValidated,
    ImprovementApplied,
    ImprovementRejected,
    ImprovementRolledBack,
    ImprovementReset,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildStarted => write!(f, "Build Started"),
            Self::BuildCompleted => write!(f, "Build Completed"),
            Self::QualityCheck => write!(f, "Quality Check"),
            Self::AutoFix => write!(f, "Auto-Fix Applied"),
            Self::VisualEdit => write!(f, "Visual Edit"),
            Self::TextEdit => write!(f, "Text Edit"),
            Self::ThemeChange => write!(f, "Theme Change"),
            Self::VariantGenerated => write!(f, "Variant Generated"),
            Self::VariantSelected => write!(f, "Variant Selected"),
            Self::BackendGenerated => write!(f, "Backend Generated"),
            Self::Deployed => write!(f, "Deployed"),
            Self::Rollback => write!(f, "Rollback"),
            Self::ImageGenerated => write!(f, "Image Generated"),
            Self::DesignImported => write!(f, "Design Imported"),
            Self::Exported => write!(f, "Exported"),
            Self::Archived => write!(f, "Archived"),
            Self::CollabSessionStarted => write!(f, "Collab Session Started"),
            Self::CollabSessionEnded => write!(f, "Collab Session Ended"),
            Self::CollabComment => write!(f, "Comment Added"),
            Self::CollabRoleChanged => write!(f, "Role Changed"),
            Self::ImprovementAnalysisRun => write!(f, "Improvement Analysis Run"),
            Self::ImprovementProposed => write!(f, "Improvement Proposed"),
            Self::ImprovementValidated => write!(f, "Improvement Validated"),
            Self::ImprovementApplied => write!(f, "Improvement Applied"),
            Self::ImprovementRejected => write!(f, "Improvement Rejected"),
            Self::ImprovementRolledBack => write!(f, "Improvement Rolled Back"),
            Self::ImprovementReset => write!(f, "Improvement Reset"),
        }
    }
}

// ─── Collection ─────────────────────────────────────────────────────────────

/// Collect all governance events from a project into a sorted timeline.
pub fn collect_audit_trail(project_dir: &Path, state: &ProjectState) -> Vec<AuditEvent> {
    let mut events = Vec::new();
    let mut event_id = 1u32;

    // 1. Build events from project state
    if state.status != crate::project::ProjectStatus::Draft {
        events.push(AuditEvent {
            id: format!("evt-{:04}", event_id),
            timestamp: state.created_at.clone(),
            event_type: AuditEventType::BuildStarted,
            description: format!(
                "Build started: template={}, mode={}",
                state.selected_template.as_deref().unwrap_or("unknown"),
                state.output_mode.as_deref().unwrap_or("Html"),
            ),
            details: serde_json::json!({
                "template_id": state.selected_template,
                "output_mode": state.output_mode,
                "plan_cost": state.plan_cost,
            }),
        });
        event_id += 1;

        // Build completed
        events.push(AuditEvent {
            id: format!("evt-{:04}", event_id),
            timestamp: state.updated_at.clone(),
            event_type: AuditEventType::BuildCompleted,
            description: format!(
                "Build completed: cost=${:.4}, iterations={}",
                state.total_cost, state.iteration_count
            ),
            details: serde_json::json!({
                "total_cost": state.total_cost,
                "build_cost": state.build_cost,
                "iteration_count": state.iteration_count,
                "line_count": state.line_count,
            }),
        });
        event_id += 1;
    }

    // 2. Quality report events
    if let Some(report) = load_quality_report(project_dir) {
        events.push(AuditEvent {
            id: format!("evt-{:04}", event_id),
            timestamp: report.timestamp.clone(),
            event_type: AuditEventType::QualityCheck,
            description: format!(
                "Quality check: score={}/100, {} issues found",
                report.overall_score, report.total_issues
            ),
            details: serde_json::json!({
                "overall_score": report.overall_score,
                "total_issues": report.total_issues,
                "auto_fixable": report.auto_fixable_count,
                "passed": report.overall_pass,
            }),
        });
        event_id += 1;

        if report.auto_fixable_count > 0 {
            events.push(AuditEvent {
                id: format!("evt-{:04}", event_id),
                timestamp: report.timestamp.clone(),
                event_type: AuditEventType::AutoFix,
                description: format!("{} auto-fixes applied", report.auto_fixable_count),
                details: serde_json::json!({
                    "fixes_applied": report.auto_fixable_count,
                }),
            });
            event_id += 1;
        }
    }

    // 3. Visual edit events
    if let Some(edit_state) = load_visual_edits(project_dir) {
        for (name, value) in &edit_state.foundation_overrides {
            events.push(AuditEvent {
                id: format!("evt-{:04}", event_id),
                timestamp: state.updated_at.clone(),
                event_type: AuditEventType::VisualEdit,
                description: format!("Token edit: --{name}: {value}"),
                details: serde_json::json!({
                    "token_name": name,
                    "value": value,
                    "layer": "foundation",
                }),
            });
            event_id += 1;
        }

        for edit in &edit_state.text_edits {
            events.push(AuditEvent {
                id: format!("evt-{:04}", event_id),
                timestamp: state.updated_at.clone(),
                event_type: AuditEventType::TextEdit,
                description: format!("Text edit: {}.{} updated", edit.section_id, edit.slot_name),
                details: serde_json::json!({
                    "section_id": edit.section_id,
                    "slot_name": edit.slot_name,
                }),
            });
            event_id += 1;
        }
    }

    // 4. Deploy history events
    if let Some(history) = load_deploy_history(project_dir) {
        for entry in &history.entries {
            events.push(AuditEvent {
                id: format!("evt-{:04}", event_id),
                timestamp: entry.timestamp.clone(),
                event_type: if entry.status == crate::deploy::history::DeployStatus::RolledBack {
                    AuditEventType::Rollback
                } else {
                    AuditEventType::Deployed
                },
                description: format!("Deployed to {} — {}", entry.provider, entry.url),
                details: serde_json::json!({
                    "provider": entry.provider,
                    "url": entry.url,
                    "build_hash": entry.build_hash,
                    "status": entry.status,
                }),
            });
            event_id += 1;
        }
    }

    // 5. Export/Archive events
    if state.status == crate::project::ProjectStatus::Exported {
        events.push(AuditEvent {
            id: format!("evt-{:04}", event_id),
            timestamp: state.updated_at.clone(),
            event_type: AuditEventType::Exported,
            description: "Project exported as ZIP".into(),
            details: serde_json::json!({}),
        });
        event_id += 1;
    }
    if state.status == crate::project::ProjectStatus::Archived {
        events.push(AuditEvent {
            id: format!("evt-{:04}", event_id),
            timestamp: state.updated_at.clone(),
            event_type: AuditEventType::Archived,
            description: "Project archived".into(),
            details: serde_json::json!({}),
        });
        event_id += 1;
    }

    // 6. Collaboration events
    if let Some(session) = crate::collab::load_session(project_dir) {
        events.push(AuditEvent {
            id: format!("evt-{event_id:04}"),
            timestamp: session.created_at.clone(),
            event_type: AuditEventType::CollabSessionStarted,
            description: format!(
                "Collaboration session started with {} participant(s)",
                session.participants.len()
            ),
            details: serde_json::json!({
                "session_id": session.session_id,
                "participants": session.participants.len(),
            }),
        });
        event_id += 1;
    }

    let comment_store = crate::collab::comments::load_comments(project_dir);
    for comment in &comment_store.comments {
        events.push(AuditEvent {
            id: format!("evt-{event_id:04}"),
            timestamp: comment.timestamp.clone(),
            event_type: AuditEventType::CollabComment,
            description: format!("{}: {}", comment.author_name, comment.text),
            details: serde_json::json!({
                "section_id": comment.section_id,
                "author": comment.author,
                "resolved": comment.resolved,
            }),
        });
        event_id += 1;
    }

    let _ = event_id; // suppress unused warning

    // Sort by timestamp
    events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    events
}

// ─── Filtering ──────────────────────────────────────────────────────────────

/// Filter events by type.
pub fn filter_by_type(events: &[AuditEvent], event_type: &AuditEventType) -> Vec<AuditEvent> {
    events
        .iter()
        .filter(|e| &e.event_type == event_type)
        .cloned()
        .collect()
}

/// Search events by text (case-insensitive).
pub fn search_events(events: &[AuditEvent], query: &str) -> Vec<AuditEvent> {
    let lower_query = query.to_lowercase();
    events
        .iter()
        .filter(|e| {
            e.description.to_lowercase().contains(&lower_query)
                || e.event_type
                    .to_string()
                    .to_lowercase()
                    .contains(&lower_query)
        })
        .cloned()
        .collect()
}

// ─── Export ─────────────────────────────────────────────────────────────────

/// Export audit trail as JSON.
pub fn export_json(events: &[AuditEvent]) -> String {
    serde_json::to_string_pretty(events).unwrap_or_else(|_| "[]".to_string())
}

/// Export audit trail as CSV.
pub fn export_csv(events: &[AuditEvent]) -> String {
    let mut csv = String::from("id,timestamp,event_type,description\n");
    for event in events {
        csv.push_str(&format!(
            "{},{},{},\"{}\"\n",
            event.id,
            event.timestamp,
            event.event_type,
            event.description.replace('"', "\"\""),
        ));
    }
    csv
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn load_quality_report(project_dir: &Path) -> Option<crate::quality::QualityReport> {
    let path = project_dir.join("quality_report.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

fn load_visual_edits(project_dir: &Path) -> Option<crate::visual_edit::VisualEditState> {
    crate::visual_edit::load_visual_edit_state(project_dir).ok()
}

fn load_deploy_history(project_dir: &Path) -> Option<crate::deploy::history::DeployHistory> {
    let history = crate::deploy::history::load_history(project_dir);
    if history.entries.is_empty() {
        None
    } else {
        Some(history)
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_events() -> Vec<AuditEvent> {
        vec![
            AuditEvent {
                id: "evt-0001".into(),
                timestamp: "2026-04-04T12:00:00Z".into(),
                event_type: AuditEventType::BuildStarted,
                description: "Build started: template=saas_landing".into(),
                details: serde_json::json!({}),
            },
            AuditEvent {
                id: "evt-0002".into(),
                timestamp: "2026-04-04T12:00:05Z".into(),
                event_type: AuditEventType::BuildCompleted,
                description: "Build completed: cost=$0.15".into(),
                details: serde_json::json!({}),
            },
            AuditEvent {
                id: "evt-0003".into(),
                timestamp: "2026-04-04T12:00:10Z".into(),
                event_type: AuditEventType::QualityCheck,
                description: "Quality check: score=91/100".into(),
                details: serde_json::json!({}),
            },
        ]
    }

    #[test]
    fn test_collect_returns_sorted_events() {
        let dir = std::env::temp_dir().join(format!("nexus-at-sort-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut state = crate::project::create_project("test", "test brief");
        state.status = crate::project::ProjectStatus::Generated;
        state.selected_template = Some("saas_landing".into());
        crate::project::save_project_state(&dir, &state).unwrap();

        let events = collect_audit_trail(&dir, &state);
        // Events should be sorted by timestamp
        for window in events.windows(2) {
            assert!(
                window[0].timestamp <= window[1].timestamp,
                "events not sorted: {} > {}",
                window[0].timestamp,
                window[1].timestamp
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_export_csv_format() {
        let events = make_events();
        let csv = export_csv(&events);

        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,timestamp,event_type,description");
        assert!(lines.len() >= 4); // header + 3 events
        assert!(lines[1].contains("evt-0001"));
    }

    #[test]
    fn test_export_json_format() {
        let events = make_events();
        let json = export_json(&events);

        let parsed: Result<Vec<AuditEvent>, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok(), "invalid JSON: {parsed:?}");
        assert_eq!(parsed.unwrap().len(), 3);
    }

    #[test]
    fn test_filter_by_event_type() {
        let events = make_events();
        let filtered = filter_by_type(&events, &AuditEventType::BuildCompleted);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].event_type, AuditEventType::BuildCompleted);
    }

    #[test]
    fn test_search_by_text() {
        let events = make_events();
        let found = search_events(&events, "quality");
        assert_eq!(found.len(), 1);
        assert!(found[0].description.contains("Quality"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let events = make_events();
        let found = search_events(&events, "BUILD");
        assert_eq!(found.len(), 2); // BuildStarted + BuildCompleted
    }

    #[test]
    fn test_empty_events_export() {
        let empty: Vec<AuditEvent> = vec![];
        assert_eq!(export_json(&empty), "[]");
        assert_eq!(export_csv(&empty), "id,timestamp,event_type,description\n");
    }
}
