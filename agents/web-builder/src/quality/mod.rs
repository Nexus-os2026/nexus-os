//! Quality Critic — six automated quality checks on every build.
//!
//! Checks: Accessibility, SEO, Performance, Security, HTML Validity, Responsive Design.
//! Each produces a 0-100 score. Issues with deterministic fixes get an AutoFix.
//! The quality report is signed and included in governance exports.

pub mod accessibility;
pub mod auto_fix;
// Phase 9B: Conversion Critic — four checks for conversion effectiveness
pub mod conversion;
pub mod html_validity;
pub mod performance;
pub mod responsive;
pub mod security;
pub mod seo;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

// ─── Errors ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum QualityError {
    #[error("check failed: {0}")]
    CheckFailed(String),
    #[error("html parse error: {0}")]
    ParseError(String),
    #[error("auto-fix error: {0}")]
    AutoFixError(String),
}

// ─── Shared Types ──────────────────────────────────────────────────────────

/// Input for all quality checks.
#[derive(Debug, Clone)]
pub struct QualityInput {
    pub html: String,
    pub output_dir: Option<PathBuf>,
    pub template_id: String,
    pub sections: Vec<String>,
}

/// Result of a single quality check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub check_id: String,
    pub check_name: String,
    pub score: u32,
    pub max_score: u32,
    pub issues: Vec<QualityIssue>,
    pub passed: bool,
}

/// A single quality issue found during a check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub severity: Severity,
    pub message: String,
    pub section_id: Option<String>,
    pub element: Option<String>,
    pub fix: Option<AutoFix>,
}

/// Issue severity. Score penalties: Error=-10, Warning=-5, Info=-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn penalty(self) -> u32 {
        match self {
            Self::Error => 10,
            Self::Warning => 5,
            Self::Info => 2,
        }
    }
}

/// A deterministic auto-fix for an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AutoFix {
    /// CSS token fix — Tier 1, instant, $0
    TokenFix {
        token_name: String,
        value: String,
        description: String,
    },
    /// HTML attribute fix — direct string replacement, $0
    AttributeFix {
        selector: String,
        attribute: String,
        value: String,
        description: String,
    },
    /// Meta tag fix — add/modify meta tag in <head>
    MetaFix {
        name: String,
        content: String,
        description: String,
    },
    /// Content fix — replace slot text (CTA, headline, etc.)
    ContentFix {
        slot_name: String,
        section_id: String,
        suggested_text: String,
        description: String,
    },
}

// ─── Quality Report ────────────────────────────────────────────────────────

/// Complete quality report across all six checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub checks: Vec<CheckResult>,
    pub overall_score: u32,
    pub overall_pass: bool,
    pub total_issues: usize,
    pub auto_fixable_count: usize,
    pub timestamp: String,
    pub build_hash: String,
    pub signature: Option<String>,
}

// ─── Score Computation ─────────────────────────────────────────────────────

/// Compute score from issues. Starts at 100, deducts per issue severity. Min 0.
pub fn compute_score(issues: &[QualityIssue]) -> u32 {
    let total_penalty: u32 = issues.iter().map(|i| i.severity.penalty()).sum();
    100u32.saturating_sub(total_penalty)
}

// ─── Orchestrator ──────────────────────────────────────────────────────────

/// Run all six quality checks and produce a QualityReport.
pub fn run_quality_checks(input: &QualityInput) -> Result<QualityReport, QualityError> {
    let checks = vec![
        accessibility::check(input)?,
        seo::check(input)?,
        performance::check(input)?,
        security::check(input)?,
        html_validity::check(input)?,
        responsive::check(input)?,
    ];

    let total_issues: usize = checks.iter().map(|c| c.issues.len()).sum();
    let auto_fixable_count = checks
        .iter()
        .flat_map(|c| &c.issues)
        .filter(|i| i.fix.is_some())
        .count();

    let overall_score = if checks.is_empty() {
        0
    } else {
        let sum: u32 = checks.iter().map(|c| c.score).sum();
        sum / checks.len() as u32
    };

    Ok(QualityReport {
        checks,
        overall_score,
        overall_pass: overall_score >= 70,
        total_issues,
        auto_fixable_count,
        timestamp: crate::deploy::now_iso8601(),
        build_hash: String::new(),
        signature: None,
    })
}

/// Collect all auto-fixable issues from a report.
pub fn collect_auto_fixes(report: &QualityReport) -> Vec<&AutoFix> {
    report
        .checks
        .iter()
        .flat_map(|c| &c.issues)
        .filter_map(|i| i.fix.as_ref())
        .collect()
}

/// Save quality report to project directory.
pub fn save_report(project_dir: &std::path::Path, report: &QualityReport) -> Result<(), String> {
    let path = project_dir.join("quality_report.json");
    let json =
        serde_json::to_string_pretty(report).map_err(|e| format!("serialize report: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write report: {e}"))
}

/// Load quality report from project directory.
pub fn load_report(project_dir: &std::path::Path) -> Option<QualityReport> {
    let path = project_dir.join("quality_report.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

// ─── HTML Parsing Helpers ──────────────────────────────────────────────────

/// Extract all data-nexus-section IDs from the HTML.
pub fn extract_sections(html: &str) -> Vec<String> {
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("[data-nexus-section]").unwrap_or_else(|_| {
        scraper::Selector::parse("*").expect("universal selector always parses")
    });
    doc.select(&sel)
        .filter_map(|el| el.value().attr("data-nexus-section").map(String::from))
        .collect()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input(html: &str) -> QualityInput {
        QualityInput {
            html: html.to_string(),
            output_dir: None,
            template_id: "test".into(),
            sections: vec![],
        }
    }

    #[test]
    fn test_run_all_checks_returns_six_results() {
        let input = sample_input("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Test</title></head><body><h1>Hello</h1></body></html>");
        let report = run_quality_checks(&input).unwrap();
        assert_eq!(report.checks.len(), 6, "Expected 6 check results");
    }

    #[test]
    fn test_overall_score_is_average() {
        let input = sample_input("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Test Page</title><meta name=\"description\" content=\"A test page for quality checking with enough content to pass.\"></head><body><h1>Test</h1><p>Content here.</p></body></html>");
        let report = run_quality_checks(&input).unwrap();
        let expected = report.checks.iter().map(|c| c.score).sum::<u32>() / 6;
        assert_eq!(report.overall_score, expected);
    }

    #[test]
    fn test_overall_pass_threshold() {
        let input = sample_input("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Good Page</title><meta name=\"description\" content=\"This is a well-structured test page with proper meta tags.\"></head><body><h1>Hello</h1><p>Good content.</p></body></html>");
        let report = run_quality_checks(&input).unwrap();
        assert_eq!(report.overall_pass, report.overall_score >= 70);
    }

    #[test]
    fn test_auto_fixable_count() {
        let input = sample_input("<!DOCTYPE html><html><head><title>Test</title></head><body><h1>Hello</h1></body></html>");
        let report = run_quality_checks(&input).unwrap();
        let manual_count = report
            .checks
            .iter()
            .flat_map(|c| &c.issues)
            .filter(|i| i.fix.is_some())
            .count();
        assert_eq!(report.auto_fixable_count, manual_count);
    }

    #[test]
    fn test_compute_score_no_issues() {
        assert_eq!(compute_score(&[]), 100);
    }

    #[test]
    fn test_compute_score_with_issues() {
        let issues = vec![
            QualityIssue {
                severity: Severity::Error,
                message: "err".into(),
                section_id: None,
                element: None,
                fix: None,
            },
            QualityIssue {
                severity: Severity::Warning,
                message: "warn".into(),
                section_id: None,
                element: None,
                fix: None,
            },
        ];
        // 100 - 10 - 5 = 85
        assert_eq!(compute_score(&issues), 85);
    }

    #[test]
    fn test_extract_sections() {
        let html =
            r#"<div data-nexus-section="hero">x</div><div data-nexus-section="features">y</div>"#;
        let sections = extract_sections(html);
        assert_eq!(sections, vec!["hero", "features"]);
    }
}
