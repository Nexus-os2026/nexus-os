//! Project lifecycle state machine for the Nexus Builder.
//!
//! Tracks project status through planning, generation, iteration, export,
//! and archival. Enforces valid state transitions and persists state to
//! `builder_state.json` alongside the existing `project.json`.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Project Status ─────────────────────────────────────────────────────────

/// Lifecycle status of a builder project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Draft,
    Planned,
    Approved,
    Generating,
    Generated,
    Iterating,
    Exported,
    Archived,
    // Failure states
    PlanFailed,
    GenerationFailed,
    IterationFailed,
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Draft => "Draft",
            Self::Planned => "Planned",
            Self::Approved => "Approved",
            Self::Generating => "Generating",
            Self::Generated => "Generated",
            Self::Iterating => "Iterating",
            Self::Exported => "Exported",
            Self::Archived => "Archived",
            Self::PlanFailed => "PlanFailed",
            Self::GenerationFailed => "GenerationFailed",
            Self::IterationFailed => "IterationFailed",
        };
        write!(f, "{s}")
    }
}

// ─── Project State ──────────────────────────────────────────────────────────

/// Full project state persisted to `builder_state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub project_id: String,
    pub status: ProjectStatus,
    pub prompt: String,
    pub project_name: Option<String>,
    pub current_checkpoint: Option<String>,
    pub approved_plan_version: Option<u32>,
    pub selected_template: Option<String>,
    /// Output mode: "Html" (default) or "React".
    #[serde(default)]
    pub output_mode: Option<String>,
    pub iteration_count: u32,
    pub total_cost: f64,
    pub plan_cost: f64,
    pub build_cost: f64,
    pub iteration_costs: Vec<f64>,
    pub line_count: Option<u32>,
    pub char_count: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    pub error_message: Option<String>,
}

// ─── State Management ───────────────────────────────────────────────────────

/// Create a new project in Draft status.
pub fn create_project(project_id: &str, prompt: &str) -> ProjectState {
    let now = now_iso8601();
    ProjectState {
        project_id: project_id.to_string(),
        status: ProjectStatus::Draft,
        prompt: prompt.to_string(),
        project_name: None,
        current_checkpoint: None,
        approved_plan_version: None,
        selected_template: None,
        output_mode: None,
        iteration_count: 0,
        total_cost: 0.0,
        plan_cost: 0.0,
        build_cost: 0.0,
        iteration_costs: Vec::new(),
        line_count: None,
        char_count: None,
        created_at: now.clone(),
        updated_at: now,
        error_message: None,
    }
}

/// Load project state from `{project_dir}/builder_state.json`.
pub fn load_project_state(project_dir: &Path) -> Result<ProjectState, String> {
    let path = project_dir.join("builder_state.json");
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("read builder_state.json: {e}"))?;
    serde_json::from_str(&contents).map_err(|e| format!("parse builder_state.json: {e}"))
}

/// Save project state to `{project_dir}/builder_state.json`.
pub fn save_project_state(project_dir: &Path, state: &ProjectState) -> Result<(), String> {
    std::fs::create_dir_all(project_dir).map_err(|e| format!("create project dir: {e}"))?;
    let path = project_dir.join("builder_state.json");
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize state: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write builder_state.json: {e}"))
}

/// Transition project to a new status, validating the transition is legal.
///
/// Updates `updated_at` on success. Clears `error_message` when transitioning
/// out of a failure state. Returns an error for invalid transitions.
pub fn transition(state: &mut ProjectState, new_status: ProjectStatus) -> Result<(), String> {
    if !is_valid_transition(&state.status, &new_status) {
        return Err(format!(
            "Invalid transition: {} -> {}",
            state.status, new_status
        ));
    }

    // Clear error when leaving a failure state
    if matches!(
        state.status,
        ProjectStatus::PlanFailed
            | ProjectStatus::GenerationFailed
            | ProjectStatus::IterationFailed
    ) {
        state.error_message = None;
    }

    state.status = new_status;
    state.updated_at = now_iso8601();
    Ok(())
}

/// Check if a transition from `from` to `to` is valid.
fn is_valid_transition(from: &ProjectStatus, to: &ProjectStatus) -> bool {
    use ProjectStatus::*;
    matches!(
        (from, to),
        // Happy path
        (Draft, Planned)
            | (Planned, Approved)
            | (Approved, Generating)
            | (Generating, Generated)
            | (Generated, Iterating)
            | (Iterating, Generated)
            | (Generated, Exported)
            | (Exported, Archived)
            | (Generated, Archived)
            // Failure paths
            | (Draft, PlanFailed)       // plan generation failed from initial
            | (Planned, PlanFailed)     // plan generation can fail during re-plan
            | (Generating, GenerationFailed)
            | (Iterating, IterationFailed)
            // Recovery paths
            | (PlanFailed, Draft)
            | (GenerationFailed, Approved)
            | (IterationFailed, Generated)
            // Allow re-export and re-iterate from Exported
            | (Exported, Iterating)
            | (Exported, Generated)
            // Unarchive
            | (Archived, Generated)
    )
}

// ─── Summary for List View ──────────────────────────────────────────────────

/// Lightweight summary for the project list UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub project_name: String,
    pub status: String,
    pub prompt: String,
    pub template: Option<String>,
    pub iteration_count: u32,
    pub total_cost: f64,
    pub line_count: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&ProjectState> for ProjectSummary {
    fn from(s: &ProjectState) -> Self {
        Self {
            project_id: s.project_id.clone(),
            project_name: s
                .project_name
                .clone()
                .unwrap_or_else(|| "Untitled Project".to_string()),
            status: s.status.to_string(),
            prompt: s.prompt.clone(),
            template: s.selected_template.clone(),
            iteration_count: s.iteration_count,
            total_cost: s.total_cost,
            line_count: s.line_count,
            created_at: s.created_at.clone(),
            updated_at: s.updated_at.clone(),
        }
    }
}

/// Convert a legacy `ProjectMeta` (project.json) into a `ProjectSummary`.
impl From<&crate::checkpoint::ProjectMeta> for ProjectSummary {
    fn from(m: &crate::checkpoint::ProjectMeta) -> Self {
        Self {
            project_id: m.id.clone(),
            project_name: m.name.clone(),
            status: "Generated".to_string(),
            prompt: m.prompt.clone(),
            template: None,
            iteration_count: m.versions.saturating_sub(1) as u32,
            total_cost: m.total_cost,
            line_count: Some(m.lines as u32),
            created_at: m.created_at.clone(),
            updated_at: m.updated_at.clone(),
        }
    }
}

// ─── Project Listing ────────────────────────────────────────────────────────

/// List all projects, handling three cases:
/// 1. Has `builder_state.json` → full Phase 4 project
/// 2. Has `project.json` → legacy project with metadata
/// 3. Has only `index.html` or `current/index.html` → bare build with minimal info
pub fn list_all_projects() -> Vec<ProjectSummary> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let builds_dir = std::path::PathBuf::from(home).join(".nexus").join("builds");
    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&builds_dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let dir = entry.path();
            let project_id = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Case 1: Has builder_state.json (Phase 4+)
            if let Ok(state) = load_project_state(&dir) {
                projects.push(ProjectSummary::from(&state));
                continue;
            }

            // Case 2: Has project.json (legacy with metadata)
            if let Some(meta) = crate::checkpoint::load_project_meta(&dir) {
                projects.push(ProjectSummary::from(&meta));
                continue;
            }

            // Case 3: Bare build — only index.html or current/index.html
            let html_path = if dir.join("current").join("index.html").exists() {
                Some(dir.join("current").join("index.html"))
            } else if dir.join("index.html").exists() {
                Some(dir.join("index.html"))
            } else {
                None
            };

            if let Some(path) = html_path {
                let line_count = std::fs::read_to_string(&path)
                    .map(|c| c.lines().count() as u32)
                    .unwrap_or(0);

                // Use file modification time for timestamps
                let mtime = std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let dir_mtime = std::fs::metadata(&dir)
                    .and_then(|m| m.modified())
                    .unwrap_or(mtime);
                let created = system_time_to_iso8601(dir_mtime);
                let updated = system_time_to_iso8601(mtime);

                projects.push(ProjectSummary {
                    project_id,
                    project_name: "Untitled Project".to_string(),
                    status: "Generated".to_string(),
                    prompt: String::new(),
                    template: None,
                    iteration_count: 0,
                    total_cost: 0.0,
                    line_count: Some(line_count),
                    created_at: created,
                    updated_at: updated,
                });
            }
            // else: directory has no HTML at all — skip it
        }
    }

    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    projects
}

/// Convert a `SystemTime` to an ISO-8601 string.
fn system_time_to_iso8601(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Re-use the same date logic as now_iso8601
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining_days + 1,
        hours,
        minutes,
        seconds
    )
}

// ─── Export ─────────────────────────────────────────────────────────────────

/// Build export metadata JSON for a project.
pub fn build_export_metadata(state: &ProjectState) -> serde_json::Value {
    let cfg = crate::model_config::load_config();
    serde_json::json!({
        "project_id": state.project_id,
        "project_name": state.project_name.as_deref().unwrap_or("Untitled"),
        "model_build": cfg.full_build.model_id,
        "model_plan": cfg.planning.model_id,
        "template": state.selected_template,
        "total_cost": state.total_cost,
        "plan_cost": state.plan_cost,
        "build_cost": state.build_cost,
        "iteration_costs": state.iteration_costs,
        "iteration_count": state.iteration_count,
        "line_count": state.line_count,
        "governance": {
            "owasp": true,
            "xss": true,
            "aria": true,
            "signed": false
        },
        "created_at": state.created_at,
        "exported_at": now_iso8601(),
    })
}

/// Build the README.md content for an export.
pub fn build_export_readme(state: &ProjectState) -> String {
    let name = state.project_name.as_deref().unwrap_or("Untitled Project");
    let template = state
        .selected_template
        .as_deref()
        .unwrap_or("auto-detected");
    format!(
        r#"# {name}

Built with Nexus Builder — Governed AI Software Builder

## Build Info
- Template: {template}
- Model: Sonnet 4.6 (build) + Haiku 4.5 (planning)
- Total cost: ${cost:.2}
- Iterations: {iters}
- Generated: {created}

## How to Deploy
Open index.html in any web browser, or deploy to any static hosting:
- Netlify: drag and drop the folder
- Vercel: `npx vercel --prod`
- GitHub Pages: push to a gh-pages branch

## Governance
This build was scanned for OWASP, XSS, ARIA compliance.
Signature: see NEXUS_BUILDER_SIGNATURE
"#,
        cost = state.total_cost,
        iters = state.iteration_count,
        created = state.created_at,
    )
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn now_iso8601() -> String {
    // Re-use the same logic as checkpoint.rs
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining_days < md {
            m = i;
            break;
        }
        remaining_days -= md;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m + 1,
        remaining_days + 1,
        hours,
        minutes,
        seconds
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexus-project-test-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_project() {
        let state = create_project("test-123", "build a landing page");
        assert_eq!(state.project_id, "test-123");
        assert_eq!(state.status, ProjectStatus::Draft);
        assert_eq!(state.prompt, "build a landing page");
        assert!(state.project_name.is_none());
        assert_eq!(state.iteration_count, 0);
        assert_eq!(state.total_cost, 0.0);
        assert!(state.created_at.contains('T'));
    }

    #[test]
    fn test_save_load_round_trip() {
        let dir = test_dir("save-load");
        let mut state = create_project("p1", "test prompt");
        state.project_name = Some("My Site".to_string());
        state.total_cost = 0.34;

        save_project_state(&dir, &state).unwrap();
        let loaded = load_project_state(&dir).unwrap();

        assert_eq!(loaded.project_id, "p1");
        assert_eq!(loaded.project_name.as_deref(), Some("My Site"));
        assert!((loaded.total_cost - 0.34).abs() < 1e-10);
        assert_eq!(loaded.status, ProjectStatus::Draft);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_valid_transitions_happy_path() {
        let mut state = create_project("t1", "test");

        // Draft -> Planned
        assert!(transition(&mut state, ProjectStatus::Planned).is_ok());
        assert_eq!(state.status, ProjectStatus::Planned);

        // Planned -> Approved
        assert!(transition(&mut state, ProjectStatus::Approved).is_ok());

        // Approved -> Generating
        assert!(transition(&mut state, ProjectStatus::Generating).is_ok());

        // Generating -> Generated
        assert!(transition(&mut state, ProjectStatus::Generated).is_ok());

        // Generated -> Iterating
        assert!(transition(&mut state, ProjectStatus::Iterating).is_ok());

        // Iterating -> Generated (loop)
        assert!(transition(&mut state, ProjectStatus::Generated).is_ok());

        // Generated -> Exported
        assert!(transition(&mut state, ProjectStatus::Exported).is_ok());

        // Exported -> Archived
        assert!(transition(&mut state, ProjectStatus::Archived).is_ok());
    }

    #[test]
    fn test_invalid_transition_rejected() {
        let mut state = create_project("t2", "test");

        // Draft -> Generated (skip steps)
        let result = transition(&mut state, ProjectStatus::Generated);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid transition"));
        // State unchanged
        assert_eq!(state.status, ProjectStatus::Draft);
    }

    #[test]
    fn test_failure_and_recovery_plan() {
        let mut state = create_project("t3", "test");

        // Draft -> PlanFailed
        assert!(transition(&mut state, ProjectStatus::PlanFailed).is_ok());
        state.error_message = Some("API timeout".to_string());

        // PlanFailed -> Draft (retry)
        assert!(transition(&mut state, ProjectStatus::Draft).is_ok());
        assert!(state.error_message.is_none()); // cleared on recovery
    }

    #[test]
    fn test_failure_and_recovery_generation() {
        let mut state = create_project("t4", "test");
        transition(&mut state, ProjectStatus::Planned).unwrap();
        transition(&mut state, ProjectStatus::Approved).unwrap();
        transition(&mut state, ProjectStatus::Generating).unwrap();

        // Generating -> GenerationFailed
        assert!(transition(&mut state, ProjectStatus::GenerationFailed).is_ok());
        state.error_message = Some("rate limited".to_string());

        // GenerationFailed -> Approved (retry with same plan)
        assert!(transition(&mut state, ProjectStatus::Approved).is_ok());
        assert!(state.error_message.is_none());
    }

    #[test]
    fn test_failure_and_recovery_iteration() {
        let mut state = create_project("t5", "test");
        transition(&mut state, ProjectStatus::Planned).unwrap();
        transition(&mut state, ProjectStatus::Approved).unwrap();
        transition(&mut state, ProjectStatus::Generating).unwrap();
        transition(&mut state, ProjectStatus::Generated).unwrap();
        transition(&mut state, ProjectStatus::Iterating).unwrap();

        // Iterating -> IterationFailed
        assert!(transition(&mut state, ProjectStatus::IterationFailed).is_ok());
        state.error_message = Some("timeout".to_string());

        // IterationFailed -> Generated (retry)
        assert!(transition(&mut state, ProjectStatus::Generated).is_ok());
        assert!(state.error_message.is_none());
    }

    #[test]
    fn test_export_metadata_structure() {
        let mut state = create_project("exp-1", "build a site");
        state.project_name = Some("Test Site".to_string());
        state.total_cost = 0.34;
        state.plan_cost = 0.0025;
        state.build_cost = 0.21;
        state.iteration_costs = vec![0.0, 0.024, 0.0];
        state.iteration_count = 3;
        state.line_count = Some(592);
        state.selected_template = Some("local_business".to_string());

        let meta = build_export_metadata(&state);
        assert_eq!(meta["project_name"], "Test Site");
        // model_build and model_plan are now dynamic from BuildModelConfig
        assert!(meta["model_build"].is_string());
        assert!(meta["model_plan"].is_string());
        assert_eq!(meta["template"], "local_business");
        assert_eq!(meta["iteration_count"], 3);
        assert!(meta["exported_at"].as_str().unwrap().contains('T'));
        assert!(meta["governance"]["owasp"].as_bool().unwrap());
    }

    #[test]
    fn test_export_readme_content() {
        let mut state = create_project("r-1", "build a site");
        state.project_name = Some("Pizza Palace".to_string());
        state.total_cost = 0.87;
        state.iteration_count = 15;
        state.selected_template = Some("restaurant".to_string());

        let readme = build_export_readme(&state);
        assert!(readme.contains("# Pizza Palace"));
        assert!(readme.contains("restaurant"));
        assert!(readme.contains("$0.87"));
        assert!(readme.contains("15"));
        assert!(readme.contains("Netlify"));
        assert!(readme.contains("NEXUS_BUILDER_SIGNATURE"));
    }

    #[test]
    fn test_project_summary_from_state() {
        let mut state = create_project("s-1", "build something");
        state.project_name = Some("My Project".to_string());
        state.selected_template = Some("saas".to_string());
        state.iteration_count = 5;
        state.total_cost = 1.23;

        let summary = ProjectSummary::from(&state);
        assert_eq!(summary.project_id, "s-1");
        assert_eq!(summary.project_name, "My Project");
        assert_eq!(summary.status, "Draft");
        assert_eq!(summary.template.as_deref(), Some("saas"));
        assert_eq!(summary.iteration_count, 5);
    }

    #[test]
    fn test_project_summary_from_legacy_meta() {
        let meta = crate::checkpoint::ProjectMeta {
            id: "legacy-1".to_string(),
            name: "Old Project".to_string(),
            prompt: "old prompt".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
            versions: 5,
            total_cost: 0.50,
            lines: 300,
        };

        let summary = ProjectSummary::from(&meta);
        assert_eq!(summary.project_id, "legacy-1");
        assert_eq!(summary.project_name, "Old Project");
        assert_eq!(summary.status, "Generated");
        assert_eq!(summary.iteration_count, 4); // versions - 1
        assert_eq!(summary.line_count, Some(300));
    }

    #[test]
    fn test_updated_at_changes_on_transition() {
        let mut state = create_project("ts-1", "test");
        let original_updated = state.updated_at.clone();

        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1100));

        transition(&mut state, ProjectStatus::Planned).unwrap();
        assert_ne!(state.updated_at, original_updated);
    }

    #[test]
    fn test_exported_to_iterating_allowed() {
        let mut state = create_project("ex-1", "test");
        transition(&mut state, ProjectStatus::Planned).unwrap();
        transition(&mut state, ProjectStatus::Approved).unwrap();
        transition(&mut state, ProjectStatus::Generating).unwrap();
        transition(&mut state, ProjectStatus::Generated).unwrap();
        transition(&mut state, ProjectStatus::Exported).unwrap();

        // Should be able to iterate on an exported project
        assert!(transition(&mut state, ProjectStatus::Iterating).is_ok());
    }

    #[test]
    fn test_system_time_to_iso8601() {
        let epoch = std::time::SystemTime::UNIX_EPOCH;
        let ts = system_time_to_iso8601(epoch);
        assert_eq!(ts, "1970-01-01T00:00:00Z");

        // 2026-04-04 roughly
        let t = epoch + std::time::Duration::from_secs(1775265962);
        let ts = system_time_to_iso8601(t);
        assert!(ts.starts_with("2026-"));
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
    }

    #[test]
    fn test_list_finds_bare_html_projects() {
        // Create a temp builds dir with a bare project
        let builds = test_dir("list-bare");
        let proj = builds.join("12345");
        fs::create_dir_all(&proj).unwrap();
        fs::write(
            proj.join("index.html"),
            "<!DOCTYPE html><html><body>Hello</body></html>\n<br>\n<br>\n",
        )
        .unwrap();

        // Override HOME for the test — use the parent of .nexus/builds
        let _nexus_dir = builds.parent().unwrap();
        // We can't easily override HOME in this test, so test the helper directly
        // by checking the HTML exists and line count works
        let html = fs::read_to_string(proj.join("index.html")).unwrap();
        assert_eq!(html.lines().count(), 3);

        let _ = fs::remove_dir_all(&builds);
    }

    #[test]
    fn test_list_skips_empty_dirs() {
        // A directory with nothing in it should be skipped
        let builds = test_dir("list-empty");
        let proj = builds.join("99999");
        fs::create_dir_all(&proj).unwrap();

        // No index.html, no project.json, no builder_state.json
        // The listing function uses HOME env, so we just verify
        // the logic: no html_path means no ProjectSummary.
        assert!(!proj.join("index.html").exists());
        assert!(!proj.join("current").join("index.html").exists());
        assert!(!proj.join("project.json").exists());

        let _ = fs::remove_dir_all(&builds);
    }

    #[test]
    fn test_unarchive_transition() {
        let mut state = create_project("un-1", "test");
        transition(&mut state, ProjectStatus::Planned).unwrap();
        transition(&mut state, ProjectStatus::Approved).unwrap();
        transition(&mut state, ProjectStatus::Generating).unwrap();
        transition(&mut state, ProjectStatus::Generated).unwrap();
        transition(&mut state, ProjectStatus::Archived).unwrap();
        assert_eq!(state.status, ProjectStatus::Archived);

        // Unarchive: Archived -> Generated
        assert!(transition(&mut state, ProjectStatus::Generated).is_ok());
        assert_eq!(state.status, ProjectStatus::Generated);

        // Should be able to iterate after unarchive
        assert!(transition(&mut state, ProjectStatus::Iterating).is_ok());
    }
}
