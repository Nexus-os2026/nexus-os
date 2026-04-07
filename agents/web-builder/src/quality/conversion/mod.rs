//! Conversion Critic — four checks that evaluate whether a site will convert visitors.
//!
//! Checks: CTA Placement, Above-the-Fold, Trust Signals, Copy Clarity.
//! Each produces a 0-100 score. Static analysis always runs; LLM (gemma4:e4b)
//! enhances scores when Ollama is available but is never required.

pub mod above_fold;
pub mod copy_clarity;
pub mod cta_placement;
pub mod trust_signals;

use super::{CheckResult, QualityError, QualityInput, QualityIssue};
use crate::content_payload::ContentPayload;
use serde::{Deserialize, Serialize};

// ─── Conversion Input ─────────────────────────────────────────────────────

/// Extended input for conversion checks — includes content and business context.
#[derive(Debug, Clone)]
pub struct ConversionInput {
    pub quality_input: QualityInput,
    pub content_payload: ContentPayload,
    pub template_id: String,
    pub brief: Option<String>,
}

// ─── Conversion Report ────────────────────────────────────────────────────

/// Complete conversion report across all four checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionReport {
    pub checks: Vec<CheckResult>,
    pub overall_score: u32,
    pub overall_pass: bool,
    pub total_issues: usize,
    pub auto_fixable_count: usize,
    pub top_recommendation: String,
    pub template_context: String,
}

// ─── Orchestrator ─────────────────────────────────────────────────────────

/// Run all four conversion checks and produce a ConversionReport.
pub fn run_conversion_checks(input: &ConversionInput) -> Result<ConversionReport, QualityError> {
    let checks = vec![
        cta_placement::check(input)?,
        above_fold::check(input)?,
        trust_signals::check(input)?,
        copy_clarity::check(input)?,
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

    // Top recommendation: highest-severity fixable issue
    let top_recommendation = find_top_recommendation(&checks);

    Ok(ConversionReport {
        checks,
        overall_score,
        overall_pass: overall_score >= 60,
        total_issues,
        auto_fixable_count,
        top_recommendation,
        template_context: input.template_id.clone(),
    })
}

/// Find the single most impactful improvement suggestion.
fn find_top_recommendation(checks: &[CheckResult]) -> String {
    // Prefer fixable Error > fixable Warning > any Error > any Warning
    let all_issues: Vec<&QualityIssue> = checks.iter().flat_map(|c| &c.issues).collect();

    // First: fixable errors
    if let Some(issue) = all_issues
        .iter()
        .find(|i| i.severity == super::Severity::Error && i.fix.is_some())
    {
        return issue.message.clone();
    }
    // Then: fixable warnings
    if let Some(issue) = all_issues
        .iter()
        .find(|i| i.severity == super::Severity::Warning && i.fix.is_some())
    {
        return issue.message.clone();
    }
    // Then: any error
    if let Some(issue) = all_issues
        .iter()
        .find(|i| i.severity == super::Severity::Error)
    {
        return issue.message.clone();
    }
    // Then: any warning
    if let Some(issue) = all_issues
        .iter()
        .find(|i| i.severity == super::Severity::Warning)
    {
        return issue.message.clone();
    }

    if all_issues.is_empty() {
        "Great job! No conversion issues found.".into()
    } else {
        all_issues[0].message.clone()
    }
}

/// Save conversion report to project directory.
pub fn save_report(project_dir: &std::path::Path, report: &ConversionReport) -> Result<(), String> {
    let path = project_dir.join("conversion_report.json");
    let json =
        serde_json::to_string_pretty(report).map_err(|e| format!("serialize report: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write report: {e}"))
}

/// Load conversion report from project directory.
pub fn load_report(project_dir: &std::path::Path) -> Option<ConversionReport> {
    let path = project_dir.join("conversion_report.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::ContentPayload;
    use crate::variant::VariantSelection;

    fn sample_conversion_input(html: &str, template_id: &str) -> ConversionInput {
        ConversionInput {
            quality_input: QualityInput {
                html: html.to_string(),
                output_dir: None,
                template_id: template_id.to_string(),
                sections: vec![],
            },
            content_payload: ContentPayload {
                template_id: template_id.to_string(),
                variant: VariantSelection::default(),
                sections: vec![],
            },
            template_id: template_id.to_string(),
            brief: Some("AI writing tool for marketers".into()),
        }
    }

    #[test]
    fn test_four_checks_run() {
        let input = sample_conversion_input(
            r##"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Test</title></head>
            <body><div data-nexus-section="hero"><h1>Write 10x Faster with AI</h1><p>The best writing tool</p>
            <a class="btn" style="padding:12px 24px;background:#6366f1;color:#fff;" href="#">Start Free Trial</a></div></body></html>"##,
            "saas_landing",
        );
        let report = run_conversion_checks(&input).unwrap();
        assert_eq!(report.checks.len(), 4, "Expected 4 conversion checks");
    }

    #[test]
    fn test_top_recommendation_populated() {
        let input = sample_conversion_input(
            "<!DOCTYPE html><html><head><title>T</title></head><body><div data-nexus-section=\"hero\"></div></body></html>",
            "saas_landing",
        );
        let report = run_conversion_checks(&input).unwrap();
        assert!(
            !report.top_recommendation.is_empty(),
            "top_recommendation should be non-empty when issues exist"
        );
    }

    #[test]
    fn test_template_context_set() {
        let input = sample_conversion_input(
            "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>",
            "ecommerce",
        );
        let report = run_conversion_checks(&input).unwrap();
        assert_eq!(report.template_context, "ecommerce");
    }

    #[test]
    fn test_conversion_degrades_gracefully() {
        // Static checks only — no LLM. Should still produce valid scores.
        let input = sample_conversion_input(
            r##"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Test</title></head>
            <body><div data-nexus-section="hero"><h1>Great Product Here</h1>
            <a class="btn" href="#">Get Started</a></div></body></html>"##,
            "saas_landing",
        );
        let report = run_conversion_checks(&input).unwrap();
        assert!(report.overall_score > 0);
        for check in &report.checks {
            assert!(check.score <= 100);
        }
    }
}
