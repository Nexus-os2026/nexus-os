//! Markdown bug report writer. See v1.1 §5.0 for the contract.
//!
//! Every bug entry has **two** sections with an explicit wall between
//! them:
//!
//! - `Observed (deterministic — trust this)` — a fixed allowlist of
//!   field names, every one of which is reproducible from the session
//!   audit log. A unit test (`tests/report_format.rs`) parses generated
//!   reports and asserts that the Observed section contains **only**
//!   field names from [`OBSERVED_FIELD_ALLOWLIST`]. Any other field
//!   name fails the test.
//! - `LLM analysis (Codex CLI guess — verify before trusting)` — the
//!   report_writer specialist's interpretation of the observed facts.
//!   In Phase 1.2 this section is always `None`; the specialist that
//!   fills it lands in Phase 1.4. When present, the header string
//!   contains the literal phrase "verify before trusting" so Phase B
//!   readers cannot mistake it for ground truth.
//!
//! The writer computes the target path under `self.root`, runs it
//! through the ACL (I-2 Layer 1 enforcement), and then writes the
//! rendered markdown. As of Phase 1.4 the date segment is produced
//! by `chrono::Utc::now()`.

use std::fmt::Write as _;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::governance::acl::Acl;

/// A scout bug report — one page per file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugReport {
    pub page: String,
    pub session_id: String,
    pub bugs: Vec<BugEntry>,
}

/// A single `BUG-XXX` entry inside a page report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugEntry {
    /// Canonical identifier, e.g. `"BUG-001"`.
    pub id: String,
    /// Short human title, e.g. `"Edit team button does nothing"`.
    pub title: String,
    pub observed: ObservedSection,
    /// Omitted from the rendered markdown when `None`. Phase 1.2 always
    /// emits `None`; Phase 1.4 begins populating it.
    pub llm_analysis: Option<LlmAnalysisSection>,
    /// Ordered reproduction steps.
    pub reproduction: Vec<String>,
}

/// The deterministic half of a bug entry. Every field here is
/// reproducible from the session audit log. The `report_writer` is
/// **forbidden** from placing speculation in this section (§5.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedSection {
    pub element: String,
    pub bounds: String,
    pub action: String,
    pub screenshot_before: String,
    pub screenshot_after: String,
    pub vision_diff_similarity: f64,
    pub console_errors: Vec<String>,
    pub tauri_commands_emitted: Vec<String>,
    pub network_requests: Vec<String>,
    pub focused_element_after: String,
    pub dom_mutations: Vec<String>,
}

/// The speculative half of a bug entry. `Codex CLI` output in Phase
/// 1.4; `None` in Phase 1.2 because the specialist that populates it
/// does not yet exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAnalysisSection {
    pub likely_cause: String,
    pub reasoning: String,
    pub suggested_files: Vec<String>,
    /// One of `"low" | "medium" | "high"`.
    pub confidence: String,
}

/// Allowlist of field names permitted inside the Observed section.
///
/// `tests/report_format.rs` parses generated markdown and asserts that
/// every `- **<name>:**` bullet in the Observed section is a member of
/// this slice. Any addition here is a deliberate contract change and
/// requires a matching update to both the `ObservedSection` struct and
/// `render_markdown`.
pub const OBSERVED_FIELD_ALLOWLIST: &[&str] = &[
    "Element",
    "Bounds",
    "Action",
    "Screenshot before",
    "Screenshot after",
    "Vision diff similarity",
    "Console errors during action window",
    "Tauri commands emitted on IPC bridge",
    "Network requests during action window",
    "Focused element after click",
    "DOM mutations within 2s",
];

/// Markdown report writer. Owns the root directory under which reports
/// are placed.
#[derive(Debug, Clone)]
pub struct ReportWriter {
    pub root: PathBuf,
}

impl ReportWriter {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Render `report` to markdown and write it to
    /// `<root>/<YYYY-MM-DD>/<page-slug>.md`.
    ///
    /// I-2 enforcement runs **first** via `acl.ensure_parent_dirs` —
    /// no directory is ever created outside the sandbox.
    pub fn write(&self, report: &BugReport, acl: &Acl) -> crate::Result<PathBuf> {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let slug = report.page.replace('/', "_");
        let path = self.root.join(&date).join(format!("{}.md", slug));

        acl.ensure_parent_dirs(&path)?;

        let markdown = self.render_markdown(report);
        std::fs::write(&path, markdown)?;

        Ok(path)
    }

    /// Render a `BugReport` to its §5.0-compliant markdown form.
    pub fn render_markdown(&self, report: &BugReport) -> String {
        let mut out = String::new();

        // Header preamble.
        let _ = writeln!(out, "# {} — QA scout report", report.page);
        let _ = writeln!(out);
        let _ = writeln!(out, "**Session:** {}", report.session_id);
        let _ = writeln!(out, "**Driver:** nexus-ui-repair v0.1.0 (scout mode)");
        let _ = writeln!(out);

        for bug in &report.bugs {
            let _ = writeln!(out, "### {}: {}", bug.id, bug.title);
            let _ = writeln!(out);

            // Observed section — allowlist-only.
            let _ = writeln!(out, "#### Observed (deterministic — trust this)");
            let _ = writeln!(out, "- **Element:** {}", bug.observed.element);
            let _ = writeln!(out, "- **Bounds:** {}", bug.observed.bounds);
            let _ = writeln!(out, "- **Action:** {}", bug.observed.action);
            let _ = writeln!(
                out,
                "- **Screenshot before:** {}",
                bug.observed.screenshot_before
            );
            let _ = writeln!(
                out,
                "- **Screenshot after:** {}",
                bug.observed.screenshot_after
            );
            let _ = writeln!(
                out,
                "- **Vision diff similarity:** {}",
                bug.observed.vision_diff_similarity
            );
            let _ = writeln!(
                out,
                "- **Console errors during action window:** {}",
                render_list_inline(&bug.observed.console_errors)
            );
            let _ = writeln!(
                out,
                "- **Tauri commands emitted on IPC bridge:** {}",
                render_list_inline(&bug.observed.tauri_commands_emitted)
            );
            let _ = writeln!(
                out,
                "- **Network requests during action window:** {}",
                render_list_inline(&bug.observed.network_requests)
            );
            let _ = writeln!(
                out,
                "- **Focused element after click:** {}",
                bug.observed.focused_element_after
            );
            let _ = writeln!(
                out,
                "- **DOM mutations within 2s:** {}",
                render_list_inline(&bug.observed.dom_mutations)
            );
            let _ = writeln!(out);

            // LLM analysis section — only if present.
            if let Some(llm) = &bug.llm_analysis {
                let _ = writeln!(
                    out,
                    "#### LLM analysis (Codex CLI guess — verify before trusting)"
                );
                let _ = writeln!(out, "- **Likely cause:** {}", llm.likely_cause);
                let _ = writeln!(out, "- **Reasoning:** {}", llm.reasoning);
                let _ = writeln!(
                    out,
                    "- **Suggested files to check (UNVERIFIED, may be wrong):**"
                );
                for file in &llm.suggested_files {
                    let _ = writeln!(out, "  - {}", file);
                }
                let _ = writeln!(out, "- **Confidence:** {}", llm.confidence);
                let _ = writeln!(out);
            }

            // Reproduction steps.
            let _ = writeln!(out, "#### Reproduction steps");
            for (i, step) in bug.reproduction.iter().enumerate() {
                let _ = writeln!(out, "{}. {}", i + 1, step);
            }
            let _ = writeln!(out);
        }

        out
    }
}

/// Render an inline list for the Observed section: `"none"` when
/// empty, otherwise a comma-separated concatenation.
fn render_list_inline(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}
