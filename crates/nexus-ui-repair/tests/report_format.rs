//! §5.0 bug report contract tests.
//!
//! The Observed section is allowlist-only; the LLM analysis section is
//! optional and, when present, must carry the "verify before trusting"
//! warning in its header. Writes outside the sandbox must be rejected
//! by the ACL gate in `ReportWriter::write`.

use std::path::PathBuf;

use nexus_ui_repair::governance::acl::Acl;
use nexus_ui_repair::specialists::report_writer::{
    BugEntry, BugReport, LlmAnalysisSection, ObservedSection, ReportWriter,
    OBSERVED_FIELD_ALLOWLIST,
};
use nexus_ui_repair::Error;
use tempfile::tempdir;

/// Guard that restores the original `HOME` when dropped. Shares a
/// process-wide resource with `tests/acl.rs`, so tests in this binary
/// must not run in parallel with anything that also touches `HOME`.
/// (Cargo gives each `tests/` file its own binary, so this is fine.)
struct HomeGuard {
    original: Option<String>,
}

impl HomeGuard {
    fn set(new_home: &std::path::Path) -> Self {
        let original = std::env::var("HOME").ok();
        std::env::set_var("HOME", new_home);
        Self { original }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

fn sample_observed() -> ObservedSection {
    ObservedSection {
        element: "button#edit-team-btn".to_string(),
        bounds: "x=240 y=180 w=80 h=32".to_string(),
        action: "click at (280, 196)".to_string(),
        screenshot_before: "bug-001-before.png".to_string(),
        screenshot_after: "bug-001-after.png".to_string(),
        vision_diff_similarity: 0.99,
        console_errors: vec![],
        tauri_commands_emitted: vec![],
        network_requests: vec![],
        focused_element_after: "unchanged from before click".to_string(),
        dom_mutations: vec![],
    }
}

fn sample_bug(llm: Option<LlmAnalysisSection>) -> BugEntry {
    BugEntry {
        id: "BUG-001".to_string(),
        title: "Edit team button does nothing".to_string(),
        observed: sample_observed(),
        llm_analysis: llm,
        reproduction: vec![
            "Navigate to /builder/teams".to_string(),
            "Click \"Edit team\" on any team row".to_string(),
            "Observe: nothing happens".to_string(),
        ],
    }
}

fn sample_report(llm: Option<LlmAnalysisSection>) -> BugReport {
    BugReport {
        page: "builder_teams".to_string(),
        session_id: "ses_8a3f".to_string(),
        bugs: vec![sample_bug(llm)],
    }
}

/// Build a ReportWriter rooted inside a temp HOME sandbox and return
/// the writer, ACL, and the tempdir (which must be kept alive by the
/// caller so it isn't dropped mid-test).
fn build_sandboxed_writer() -> (ReportWriter, Acl, tempfile::TempDir, HomeGuard, PathBuf) {
    let home_dir = tempdir().expect("create temp HOME");
    let home_path = home_dir.path().to_path_buf();

    let reports = home_path.join(".nexus").join("ui-repair").join("reports");
    let sessions = home_path.join(".nexus").join("ui-repair").join("sessions");
    std::fs::create_dir_all(&reports).expect("mkdir reports");
    std::fs::create_dir_all(&sessions).expect("mkdir sessions");

    let guard = HomeGuard::set(&home_path);
    let acl = Acl::default_scout();
    let writer = ReportWriter::new(reports.clone());
    (writer, acl, home_dir, guard, reports)
}

#[test]
fn writes_report_to_correct_path() {
    let (writer, acl, _home_dir, _guard, reports_root) = build_sandboxed_writer();

    let report = sample_report(None);
    let written = writer.write(&report, &acl).expect("write report");

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let expected = reports_root.join(&today).join("builder_teams.md");
    assert_eq!(written, expected);
    assert!(written.exists(), "report file must exist on disk");

    let content = std::fs::read_to_string(&written).expect("read written report");
    assert!(content.contains("# builder_teams — QA scout report"));
    assert!(content.contains("**Session:** ses_8a3f"));
    assert!(content.contains("### BUG-001: Edit team button does nothing"));
}

#[test]
fn observed_section_only_uses_allowlisted_field_names() {
    let (writer, _acl, _home_dir, _guard, _root) = build_sandboxed_writer();

    let report = sample_report(None);
    let markdown = writer.render_markdown(&report);

    // Find the Observed section boundaries.
    let observed_header = "#### Observed (deterministic — trust this)";
    let start = markdown
        .find(observed_header)
        .expect("Observed header must be present");
    let tail = &markdown[start + observed_header.len()..];
    // Observed section ends at the next `#### ` header or EOF.
    let end = tail.find("\n#### ").unwrap_or(tail.len());
    let observed_body = &tail[..end];

    // Extract every `- **<name>:**` bullet.
    let allowlist: std::collections::HashSet<&str> =
        OBSERVED_FIELD_ALLOWLIST.iter().copied().collect();
    let mut seen = 0usize;
    for line in observed_body.lines() {
        let trimmed = line.trim_start();
        let rest = match trimmed.strip_prefix("- **") {
            Some(r) => r,
            None => continue,
        };
        let name_end = rest
            .find(":**")
            .unwrap_or_else(|| panic!("malformed Observed bullet (no `:**`): {}", line));
        let name = &rest[..name_end];
        assert!(
            allowlist.contains(name),
            "field name {:?} is not in OBSERVED_FIELD_ALLOWLIST",
            name
        );
        seen += 1;
    }
    assert_eq!(
        seen,
        OBSERVED_FIELD_ALLOWLIST.len(),
        "expected every allowlisted field to appear exactly once"
    );
}

#[test]
fn llm_analysis_section_is_omitted_when_none() {
    let (writer, _acl, _home_dir, _guard, _root) = build_sandboxed_writer();

    let report = sample_report(None);
    let markdown = writer.render_markdown(&report);

    assert!(
        !markdown.contains("#### LLM analysis"),
        "LLM analysis section must not appear when llm_analysis is None"
    );
}

#[test]
fn llm_analysis_section_has_warning_header_when_present() {
    let (writer, _acl, _home_dir, _guard, _root) = build_sandboxed_writer();

    let llm = LlmAnalysisSection {
        likely_cause: "onClick handler missing".to_string(),
        reasoning: "click produced no effect at any layer".to_string(),
        suggested_files: vec!["app/src/components/builder/TeamsPanel.tsx".to_string()],
        confidence: "low".to_string(),
    };
    let report = sample_report(Some(llm));
    let markdown = writer.render_markdown(&report);

    assert!(
        markdown.contains("#### LLM analysis"),
        "LLM analysis header must appear when Some"
    );
    assert!(
        markdown.contains("verify before trusting"),
        "LLM analysis header must carry the `verify before trusting` warning"
    );
}

#[test]
fn write_outside_sandbox_is_rejected() {
    // Set up HOME so Acl::default_scout() has legitimate roots, then
    // point the writer at a root *outside* those.
    let home_dir = tempdir().expect("create temp HOME");
    let home_path = home_dir.path().to_path_buf();
    let reports = home_path.join(".nexus").join("ui-repair").join("reports");
    let sessions = home_path.join(".nexus").join("ui-repair").join("sessions");
    std::fs::create_dir_all(&reports).expect("mkdir reports");
    std::fs::create_dir_all(&sessions).expect("mkdir sessions");

    let _guard = HomeGuard::set(&home_path);
    let acl = Acl::default_scout();

    // Writer rooted at an out-of-sandbox directory.
    let out_of_bounds = home_path.join(".nexus").join("somewhere_else");
    let writer = ReportWriter::new(out_of_bounds);

    let report = sample_report(None);
    match writer.write(&report, &acl) {
        Err(Error::AclDenied(_)) => {}
        Err(other) => panic!("expected AclDenied, got {:?}", other),
        Ok(path) => panic!(
            "out-of-sandbox write must be rejected, but succeeded with path {:?}",
            path
        ),
    }
}
