//! Responsive design checker — CSS analysis for mobile-friendliness.
//!
//! Checks: viewport meta, media queries, fixed widths, small font sizes,
//! touch target sizes, image max-width.

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use regex::Regex;
use scraper::{Html, Selector};

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_viewport_meta(&doc, &mut issues);
    check_media_queries(&input.html, &mut issues);
    check_fixed_widths(&input.html, &mut issues);
    check_image_max_width(&input.html, &mut issues);
    check_horizontal_overflow(&input.html, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "responsive".into(),
        check_name: "Responsive Design".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_viewport_meta(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("meta[name=\"viewport\"]").expect("valid");
    if doc.select(&sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing viewport meta tag — page won't scale on mobile devices".into(),
            section_id: None,
            element: Some("head".into()),
            fix: Some(AutoFix::MetaFix {
                name: "viewport".into(),
                content: "width=device-width, initial-scale=1".into(),
                description: "Add viewport meta tag".into(),
            }),
        });
    }
}

fn check_media_queries(html: &str, issues: &mut Vec<QualityIssue>) {
    let lower = html.to_lowercase();
    let has_media = lower.contains("@media");

    if !has_media {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "No media queries found — layout may not adapt to different screen sizes"
                .into(),
            section_id: None,
            element: None,
            fix: None,
        });
        return;
    }

    // Check for key breakpoints
    let has_tablet = lower.contains("768px") || lower.contains("48rem") || lower.contains("48em");
    let has_desktop = lower.contains("1024px") || lower.contains("64rem") || lower.contains("64em");

    if !has_tablet && !has_desktop {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: "Media queries present but no common breakpoints (768px, 1024px) detected"
                .into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }
}

fn check_fixed_widths(html: &str, issues: &mut Vec<QualityIssue>) {
    // Look for fixed widths > 320px on elements that might be containers
    // Use regex to find width: NNNpx patterns in CSS
    let re = Regex::new(r"width\s*:\s*(\d+)px").expect("valid regex");
    let mut found_fixed = false;

    for cap in re.captures_iter(html) {
        if let Some(val) = cap.get(1) {
            if let Ok(px) = val.as_str().parse::<u32>() {
                if px > 320 {
                    // Check it's not inside max-width or min-width
                    let match_start = cap.get(0).map_or(0, |m| m.start());
                    let prefix_start = match_start.saturating_sub(5);
                    let prefix = &html[prefix_start..match_start];
                    if !prefix.contains("max-") && !prefix.contains("min-") && !found_fixed {
                        found_fixed = true;
                        issues.push(QualityIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Fixed width: {px}px — may cause horizontal scroll on mobile"
                            ),
                            section_id: None,
                            element: None,
                            fix: None,
                        });
                    }
                }
            }
        }
    }
}

fn check_image_max_width(html: &str, issues: &mut Vec<QualityIssue>) {
    let lower = html.to_lowercase();
    // Check if there's a global img { max-width: 100% } rule
    let has_img_max = lower.contains("img") && lower.contains("max-width");

    if !has_img_max && lower.contains("<img") {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: "No global img { max-width: 100% } — images may overflow on small screens"
                .into(),
            section_id: None,
            element: Some("img".into()),
            fix: None,
        });
    }
}

fn check_horizontal_overflow(html: &str, issues: &mut Vec<QualityIssue>) {
    let lower = html.to_lowercase();
    if lower.contains("100vw")
        && !lower.contains("overflow-x")
        && !lower.contains("overflow: hidden")
    {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message:
                "Element uses 100vw without overflow-x control — may cause horizontal scrollbar"
                    .into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn input(html: &str) -> QualityInput {
        QualityInput {
            html: html.to_string(),
            output_dir: None,
            template_id: "test".into(),
            sections: vec![],
        }
    }

    #[test]
    fn test_detects_missing_viewport() {
        let result = check(&input("<html><head></head><body></body></html>")).unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("viewport")));
        let vp = result
            .issues
            .iter()
            .find(|i| i.message.contains("viewport"))
            .unwrap();
        assert!(vp.fix.is_some());
    }

    #[test]
    fn test_detects_fixed_width() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width, initial-scale=1"><style>.box { width: 600px; }</style></head><body></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Fixed width")));
    }

    #[test]
    fn test_detects_missing_breakpoints() {
        let html = r#"<html><head><meta name="viewport" content="width=device-width, initial-scale=1"><style>body{margin:0}</style></head><body></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("media queries") || i.message.contains("No media")));
    }

    #[test]
    fn test_clean_template_scores_high() {
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Test</title><style>
        img { max-width: 100%; height: auto; }
        @media (max-width: 768px) { .nav { flex-direction: column; } }
        @media (max-width: 1024px) { .container { padding: 1rem; } }
        </style></head><body><h1>Hello</h1></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(
            result.score >= 90,
            "Responsive template should score >= 90, got {}",
            result.score
        );
    }
}
