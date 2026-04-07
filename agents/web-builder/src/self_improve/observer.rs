//! Observer — collects metrics from completed projects.
//!
//! Only collects from projects that reached `Exported` or `Archived` status
//! (i.e. the user considered them done). In-progress projects are excluded.

use crate::project::{load_project_state, ProjectStatus};
use crate::self_improve::metrics::ProjectMetrics;
use crate::trust_pack::audit_trail::{collect_audit_trail, AuditEventType};
use std::collections::HashMap;
use std::path::Path;

/// Collect `ProjectMetrics` from a completed project directory.
///
/// Returns `None` if the project is not in a terminal state (Exported/Archived/Generated).
pub fn collect_project_metrics(project_dir: &Path) -> Option<ProjectMetrics> {
    let state = load_project_state(project_dir).ok()?;

    // Only collect from completed projects
    match state.status {
        ProjectStatus::Exported | ProjectStatus::Archived | ProjectStatus::Generated => {}
        _ => return None,
    }

    let template_id = state.selected_template.clone().unwrap_or_default();
    let audit_events = collect_audit_trail(project_dir, &state);

    // Determine palette and typography from the project
    // We inspect visual edits and variant selection events
    let edit_state = crate::visual_edit::load_visual_edit_state(project_dir).ok();

    let mut palette_id = String::new();
    let mut typography_id = "modern".to_string();
    let mut palette_was_changed = false;
    let mut typography_was_changed = false;
    let layout_selections: HashMap<String, String> = HashMap::new();

    // Check variant selection from audit events
    for event in &audit_events {
        if event.event_type == AuditEventType::VariantSelected {
            if let Some(pid) = event.details.get("palette_id").and_then(|v| v.as_str()) {
                palette_id = pid.to_string();
            }
            if let Some(tid) = event.details.get("typography_id").and_then(|v| v.as_str()) {
                typography_id = tid.to_string();
            }
        }
        if event.event_type == AuditEventType::ThemeChange {
            palette_was_changed = true;
        }
    }

    // If no palette found from events, use a default for the template
    if palette_id.is_empty() {
        let palettes = crate::variant::palettes_for_template(&template_id);
        palette_id = palettes
            .first()
            .map(|p| p.id.to_string())
            .unwrap_or_default();
    }

    // Collect section edits
    let mut section_edits: HashMap<String, u32> = HashMap::new();
    let mut tokens_changed = Vec::new();

    if let Some(ref edits) = edit_state {
        for edit in &edits.instance_overrides {
            *section_edits.entry(edit.section_id.clone()).or_insert(0) += 1;
        }
        for edit in &edits.text_edits {
            *section_edits.entry(edit.section_id.clone()).or_insert(0) += 1;
        }
        tokens_changed = edits.foundation_overrides.keys().cloned().collect();
        if !edits.foundation_overrides.is_empty() {
            // If foundation tokens were changed, palette/typography may have changed
            palette_was_changed = palette_was_changed
                || edits.foundation_overrides.keys().any(|k| {
                    k.contains("primary") || k.contains("secondary") || k.contains("accent")
                });
            typography_was_changed = edits
                .foundation_overrides
                .keys()
                .any(|k| k.contains("font") || k.contains("text-"));
        }
    }

    // Quality and conversion scores from report files
    let quality_score = crate::quality::load_report(project_dir)
        .map(|r| r.overall_score)
        .unwrap_or(0);

    let conversion_score = load_conversion_score(project_dir).unwrap_or(0);

    // Count auto-fixes from audit events
    let auto_fixes_applied = audit_events
        .iter()
        .filter(|e| e.event_type == AuditEventType::AutoFix)
        .count();

    // Variant selected index from audit events
    let variant_selected_index = audit_events
        .iter()
        .find(|e| e.event_type == AuditEventType::VariantSelected)
        .and_then(|e| e.details.get("variant_index"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    Some(ProjectMetrics {
        project_id: state.project_id.clone(),
        template_id,
        palette_id,
        typography_id,
        layout_selections,
        palette_was_changed,
        typography_was_changed,
        section_edits,
        tokens_changed,
        quality_score,
        conversion_score,
        iteration_count: state.iteration_count,
        time_to_deploy_seconds: None,
        variant_selected_index,
        auto_fixes_applied,
        build_cost: state.total_cost,
        completed_at: state.updated_at.clone(),
    })
}

/// Scan a base directory for project subdirectories and collect metrics from each.
pub fn collect_all_metrics(base_dir: &Path) -> Vec<ProjectMetrics> {
    let mut metrics = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(m) = collect_project_metrics(&path) {
                    metrics.push(m);
                }
            }
        }
    }
    metrics
}

fn load_conversion_score(project_dir: &Path) -> Option<u32> {
    let path = project_dir.join("conversion_report.json");
    let json = std::fs::read_to_string(path).ok()?;
    let report: serde_json::Value = serde_json::from_str(&json).ok()?;
    report.get("overall_score")?.as_u64().map(|v| v as u32)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{create_project, save_project_state, ProjectStatus};

    fn setup_project(status: ProjectStatus) -> (std::path::PathBuf, crate::project::ProjectState) {
        let dir = std::env::temp_dir().join(format!("nexus-si-obs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut state = create_project("test-proj", "test brief");
        state.status = status;
        state.selected_template = Some("saas_landing".into());
        state.iteration_count = 3;
        state.total_cost = 0.05;
        save_project_state(&dir, &state).unwrap();
        (dir, state)
    }

    #[test]
    fn test_collect_metrics_from_exported_project() {
        let (dir, _) = setup_project(ProjectStatus::Exported);
        let metrics = collect_project_metrics(&dir);
        assert!(metrics.is_some());
        let m = metrics.unwrap();
        assert_eq!(m.template_id, "saas_landing");
        assert_eq!(m.iteration_count, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_collect_metrics_from_generated_project() {
        let (dir, _) = setup_project(ProjectStatus::Generated);
        let metrics = collect_project_metrics(&dir);
        assert!(metrics.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ignores_incomplete_projects() {
        let (dir, _) = setup_project(ProjectStatus::Draft);
        let metrics = collect_project_metrics(&dir);
        assert!(metrics.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ignores_generating_projects() {
        let (dir, _) = setup_project(ProjectStatus::Generating);
        let metrics = collect_project_metrics(&dir);
        assert!(metrics.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_collect_all_metrics_scans_dirs() {
        let base = std::env::temp_dir().join(format!("nexus-si-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();

        // Create 2 exported + 1 draft
        for i in 0..3 {
            let sub = base.join(format!("proj-{i}"));
            std::fs::create_dir_all(&sub).unwrap();
            let mut state = create_project(&format!("proj-{i}"), "brief");
            state.status = if i < 2 {
                ProjectStatus::Exported
            } else {
                ProjectStatus::Draft
            };
            state.selected_template = Some("saas_landing".into());
            save_project_state(&sub, &state).unwrap();
        }

        let all = collect_all_metrics(&base);
        assert_eq!(all.len(), 2); // only exported ones
        let _ = std::fs::remove_dir_all(&base);
    }
}
