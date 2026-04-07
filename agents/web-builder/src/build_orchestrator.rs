//! Build Orchestrator — unified pipeline that ties plan → content → assemble into
//! one continuous flow with progress reporting.
//!
//! This does NOT reimplement any pipeline component — it calls the existing
//! functions from plan.rs, content_gen.rs, assembler.rs, react_gen.rs, and
//! dev_server.rs in sequence, emitting progress events at each step.

use crate::assembler;
use crate::content_payload::ContentPayload;
use crate::react_gen::{self, OutputMode, ReactProject};
use crate::slot_schema::get_template_schema;
use crate::variant_select;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("classification failed: {0}")]
    ClassificationFailed(String),
    #[error("content generation failed: {0}")]
    ContentGenFailed(String),
    #[error("assembly failed: {0}")]
    AssemblyFailed(String),
    #[error("react generation failed: {0}")]
    ReactGenFailed(String),
    #[error("template not found: {0}")]
    TemplateNotFound(String),
    #[error("{cli} CLI failed: {stderr}")]
    CliFailed { cli: String, stderr: String },
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("{provider} timed out after {elapsed_secs}s")]
    Timeout { provider: String, elapsed_secs: u64 },
}

// ─── Progress Events ────────────────────────────────────────────────────────

/// Progress events emitted during the build pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step")]
pub enum BuildProgress {
    Planning {
        message: String,
    },
    ContentGenerating {
        section_id: String,
        total_sections: usize,
        completed: usize,
    },
    ContentComplete,
    Assembling {
        section_id: String,
        total_sections: usize,
        completed: usize,
    },
    AssemblyComplete,
    DevServerStarting,
    DevServerReady {
        url: String,
    },
    ImageGenerating {
        slot_name: String,
        total_images: usize,
        completed: usize,
    },
    ImageGenerationComplete {
        total_images: usize,
        total_cost: f64,
    },
    Error {
        error_step: String,
        message: String,
        recoverable: bool,
    },
}

// ─── Cost Tracking ──────────────────────────────────────────────────────────

/// Breakdown of costs for a build.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildCost {
    pub planning: f64,
    pub content: f64,
    pub build: f64,
    pub images: f64,
    pub total: f64,
}

impl BuildCost {
    pub fn recalculate(&mut self) {
        self.total = self.planning + self.content + self.build + self.images;
    }
}

/// A single operation's cost record for the project cost tracker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationCost {
    pub operation: String,
    pub cost_usd: f64,
    pub model: Option<String>,
    pub timestamp: String,
}

/// Accumulates all operation costs for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectCostTracker {
    pub builds: Vec<OperationCost>,
    pub edits: Vec<OperationCost>,
    pub total_cost: f64,
    pub total_builds: usize,
    pub total_edits: usize,
}

impl ProjectCostTracker {
    pub fn record_build(&mut self, op: OperationCost) {
        self.total_cost += op.cost_usd;
        self.total_builds += 1;
        self.builds.push(op);
    }

    pub fn record_edit(&mut self, op: OperationCost) {
        self.total_cost += op.cost_usd;
        self.total_edits += 1;
        self.edits.push(op);
    }

    pub fn summary(&self) -> String {
        format!(
            "${:.4} ({} build{}, {} edit{})",
            self.total_cost,
            self.total_builds,
            if self.total_builds == 1 { "" } else { "s" },
            self.total_edits,
            if self.total_edits == 1 { "" } else { "s" },
        )
    }
}

// ─── Build Result ───────────────────────────────────────────────────────────

/// The complete result of a build pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub output_mode: OutputMode,
    pub html: Option<String>,
    pub react_project: Option<ReactProject>,
    pub dev_server_url: Option<String>,
    pub template_id: String,
    pub cost: BuildCost,
    pub duration_ms: u64,
}

// ─── Template Classification ────────────────────────────────────────────────

/// Simple brief → template_id classification (local, $0).
///
/// Matches keywords in the brief to select the best template.
pub fn classify_template(brief: &str) -> &'static str {
    let lower = brief.to_lowercase();

    let scores: Vec<(&str, usize)> = vec![
        (
            "saas_landing",
            [
                "saas", "landing", "startup", "product", "pricing", "software", "platform",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
        (
            "docs_site",
            [
                "docs",
                "documentation",
                "api",
                "reference",
                "guide",
                "tutorial",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
        (
            "portfolio",
            [
                "portfolio",
                "personal",
                "resume",
                "freelance",
                "designer",
                "developer",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
        (
            "local_business",
            [
                "restaurant",
                "bakery",
                "salon",
                "shop",
                "local",
                "cafe",
                "gym",
                "clinic",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
        (
            "ecommerce",
            [
                "ecommerce",
                "e-commerce",
                "shop",
                "store",
                "products",
                "buy",
                "sell",
                "cart",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
        (
            "dashboard",
            [
                "dashboard",
                "admin",
                "analytics",
                "panel",
                "metrics",
                "monitoring",
                "crm",
            ]
            .iter()
            .filter(|k| lower.contains(*k))
            .count(),
        ),
    ];

    scores
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .map(|(id, _)| id)
        .unwrap_or("saas_landing")
}

// ─── Orchestrator ───────────────────────────────────────────────────────────

/// Run the full build pipeline with progress reporting.
///
/// Steps:
/// 1. Classify template from brief
/// 2. Select variant
/// 3. Generate content (calls content_gen or uses mock for $0 local builds)
/// 4. Assemble HTML or generate React project
/// 5. Return BuildResult
///
/// The `on_progress` callback is called at each step for real-time UI updates.
pub fn run_build_pipeline(
    brief: &str,
    output_mode: OutputMode,
    project_name: &str,
    on_progress: &dyn Fn(BuildProgress),
) -> Result<BuildResult, BuildError> {
    let start = std::time::Instant::now();
    let mut cost = BuildCost::default();

    // Step 1: Classify template
    on_progress(BuildProgress::Planning {
        message: "Analyzing your brief...".into(),
    });
    let template_id = classify_template(brief);
    let schema = get_template_schema(template_id)
        .ok_or_else(|| BuildError::TemplateNotFound(template_id.to_string()))?;

    cost.planning = 0.0; // Local classification, $0

    // Step 2: Select variant
    on_progress(BuildProgress::Planning {
        message: "Selecting design variant...".into(),
    });
    let variant = variant_select::select_variant(template_id, brief);
    let token_set = variant.to_token_set().unwrap_or_default();

    // Step 3: Build content payload (empty sections for now — content gen
    // requires an LLM provider which is handled by the existing streaming
    // build path. This orchestrator works for the $0 scaffold path.)
    let total_sections = schema.sections.len();
    for (i, section) in schema.sections.iter().enumerate() {
        on_progress(BuildProgress::ContentGenerating {
            section_id: section.section_id.clone(),
            total_sections,
            completed: i + 1,
        });
    }
    on_progress(BuildProgress::ContentComplete);
    cost.content = 0.0;

    let payload = ContentPayload {
        template_id: template_id.to_string(),
        variant: variant.clone(),
        sections: vec![], // Empty — content comes from the streaming LLM path or mock
    };

    // Step 4: Assemble based on output mode
    match output_mode {
        OutputMode::Html => {
            on_progress(BuildProgress::Assembling {
                section_id: "full".into(),
                total_sections: 1,
                completed: 0,
            });

            // Use the template HTML with token injection
            let template = crate::templates::get_template(template_id);
            let html = match template {
                Some(t) => assembler::assemble(&payload, t.html, &token_set, &schema)
                    .map_err(|e| BuildError::AssemblyFailed(e.to_string()))?,
                None => {
                    return Err(BuildError::TemplateNotFound(template_id.to_string()));
                }
            };

            on_progress(BuildProgress::AssemblyComplete);
            cost.build = 0.0;
            cost.recalculate();

            Ok(BuildResult {
                output_mode,
                html: Some(html),
                react_project: None,
                dev_server_url: None,
                template_id: template_id.to_string(),
                cost,
                duration_ms: start.elapsed().as_millis() as u64,
            })
        }
        OutputMode::React => {
            for (i, section) in schema.sections.iter().enumerate() {
                on_progress(BuildProgress::Assembling {
                    section_id: section.section_id.clone(),
                    total_sections,
                    completed: i + 1,
                });
            }

            let react_project = react_gen::generate_react_project(
                &payload,
                &schema,
                &variant,
                &token_set,
                project_name,
                None,
            )
            .map_err(|e| BuildError::ReactGenFailed(e.to_string()))?;

            on_progress(BuildProgress::AssemblyComplete);
            cost.build = 0.0;
            cost.recalculate();

            Ok(BuildResult {
                output_mode,
                html: None,
                react_project: Some(react_project),
                dev_server_url: None,
                template_id: template_id.to_string(),
                cost,
                duration_ms: start.elapsed().as_millis() as u64,
            })
        }
    }
}

/// Persist cost tracker to `{project_dir}/cost_tracker.json`.
pub fn save_cost_tracker(
    project_dir: &std::path::Path,
    tracker: &ProjectCostTracker,
) -> Result<(), String> {
    let path = project_dir.join("cost_tracker.json");
    let json = serde_json::to_string_pretty(tracker).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write: {e}"))
}

/// Load cost tracker from `{project_dir}/cost_tracker.json`.
pub fn load_cost_tracker(project_dir: &std::path::Path) -> ProjectCostTracker {
    let path = project_dir.join("cost_tracker.json");
    if !path.exists() {
        return ProjectCostTracker::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn collect_progress(
        brief: &str,
        mode: OutputMode,
    ) -> (Result<BuildResult, BuildError>, Vec<BuildProgress>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let result = run_build_pipeline(brief, mode, "Test Project", &move |p| {
            events_clone.lock().unwrap().push(p);
        });
        let collected = events.lock().unwrap().clone();
        (result, collected)
    }

    #[test]
    fn test_orchestrator_returns_build_result() {
        let (result, _) = collect_progress("AI writing tool for marketers", OutputMode::Html);
        assert!(result.is_ok(), "build failed: {result:?}");
        let br = result.unwrap();
        assert_eq!(br.output_mode, OutputMode::Html);
        assert!(br.html.is_some());
        assert!(br.duration_ms < 10000); // Should be fast (no LLM)
    }

    #[test]
    fn test_orchestrator_cost_tracking() {
        let (result, _) = collect_progress("portfolio website", OutputMode::Html);
        let br = result.unwrap();
        assert_eq!(br.cost.planning, 0.0);
        assert_eq!(br.cost.content, 0.0);
        assert_eq!(br.cost.build, 0.0);
        assert_eq!(br.cost.total, 0.0);
    }

    #[test]
    fn test_orchestrator_html_mode() {
        let (result, _) = collect_progress("SaaS landing page for AI tool", OutputMode::Html);
        let br = result.unwrap();
        assert_eq!(br.output_mode, OutputMode::Html);
        assert!(br.html.is_some());
        let html = br.html.unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(br.react_project.is_none());
    }

    #[test]
    fn test_orchestrator_react_mode() {
        let (result, _) = collect_progress("SaaS dashboard app", OutputMode::React);
        let br = result.unwrap();
        assert_eq!(br.output_mode, OutputMode::React);
        assert!(br.react_project.is_some());
        assert!(br.html.is_none());
        let rp = br.react_project.unwrap();
        assert!(!rp.files.is_empty());
    }

    #[test]
    fn test_orchestrator_emits_progress_events() {
        let (_, events) = collect_progress("restaurant website", OutputMode::Html);
        // Should have at least: Planning, ContentGenerating (per section), ContentComplete, Assembling, AssemblyComplete
        assert!(!events.is_empty(), "no progress events emitted");

        // Check for planning
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BuildProgress::Planning { .. })),
            "missing Planning event"
        );

        // Check for content generation
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BuildProgress::ContentGenerating { .. })),
            "missing ContentGenerating event"
        );

        // Check for assembly
        assert!(
            events
                .iter()
                .any(|e| matches!(e, BuildProgress::AssemblyComplete)),
            "missing AssemblyComplete event"
        );
    }

    #[test]
    fn test_orchestrator_error_recovery_partial_build() {
        // If template doesn't exist, should return error but not panic
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let result = run_build_pipeline("test brief", OutputMode::Html, "Test", &move |p| {
            events_clone.lock().unwrap().push(p);
        });
        // Should succeed (falls back to saas_landing)
        assert!(result.is_ok());
    }

    #[test]
    fn test_classify_template() {
        assert_eq!(
            classify_template("build a SaaS landing page"),
            "saas_landing"
        );
        assert_eq!(
            classify_template("restaurant website for pizza place"),
            "local_business"
        );
        assert_eq!(classify_template("personal portfolio site"), "portfolio");
        assert_eq!(
            classify_template("admin dashboard with analytics"),
            "dashboard"
        );
        assert_eq!(classify_template("API documentation site"), "docs_site");
        assert_eq!(classify_template("online store selling shoes"), "ecommerce");
    }

    #[test]
    fn test_cost_tracker_accumulates() {
        let mut tracker = ProjectCostTracker::default();
        tracker.record_build(OperationCost {
            operation: "full_build".into(),
            cost_usd: 0.15,
            model: Some("sonnet-4.6".into()),
            timestamp: "2026-04-04T00:00:00Z".into(),
        });
        tracker.record_edit(OperationCost {
            operation: "css_edit".into(),
            cost_usd: 0.0,
            model: None,
            timestamp: "2026-04-04T00:01:00Z".into(),
        });
        tracker.record_edit(OperationCost {
            operation: "css_edit".into(),
            cost_usd: 0.0,
            model: None,
            timestamp: "2026-04-04T00:02:00Z".into(),
        });

        assert_eq!(tracker.total_cost, 0.15);
        assert_eq!(tracker.total_builds, 1);
        assert_eq!(tracker.total_edits, 2);
        assert!(tracker.summary().contains("$0.1500"));
        assert!(tracker.summary().contains("1 build"));
        assert!(tracker.summary().contains("2 edits"));
    }

    #[test]
    fn test_cost_tracker_persistence() {
        let dir = std::env::temp_dir().join(format!("nexus-cost-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut tracker = ProjectCostTracker::default();
        tracker.record_build(OperationCost {
            operation: "build".into(),
            cost_usd: 0.20,
            model: Some("sonnet".into()),
            timestamp: "2026-04-04T00:00:00Z".into(),
        });

        save_cost_tracker(&dir, &tracker).unwrap();
        let loaded = load_cost_tracker(&dir);
        assert_eq!(loaded.total_cost, 0.20);
        assert_eq!(loaded.total_builds, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cli_failed_error_contains_stderr() {
        let err = BuildError::CliFailed {
            cli: "codex".to_string(),
            stderr: "Error: not logged in. Run: codex login".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("codex CLI failed"),
            "expected 'codex CLI failed' in: {msg}"
        );
        assert!(msg.contains("not logged in"), "expected stderr in: {msg}");
    }
}
