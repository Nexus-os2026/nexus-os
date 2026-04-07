//! Performance checker — static analysis of HTML output for performance issues.
//!
//! Checks: file size, inline CSS size, external resources count, missing image dimensions,
//! missing defer/async on scripts, render-blocking CSS.

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use scraper::{Html, Selector};

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_html_size(&input.html, &mut issues);
    check_inline_css_size(&input.html, &mut issues);
    check_external_resources(&doc, &mut issues);
    check_image_dimensions(&doc, &mut issues);
    check_script_defer(&doc, &mut issues);
    check_render_blocking_css(&doc, &mut issues);
    check_font_display(&input.html, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "performance".into(),
        check_name: "Performance".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_html_size(html: &str, issues: &mut Vec<QualityIssue>) {
    let size_kb = html.len() / 1024;
    if size_kb > 200 {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: format!("HTML file is {size_kb}KB — consider splitting or lazy-loading content (>200KB threshold)"),
            section_id: None,
            element: None,
            fix: None,
        });
    }
}

fn check_inline_css_size(html: &str, issues: &mut Vec<QualityIssue>) {
    let mut total_css = 0usize;
    let lower = html.to_lowercase();
    let mut start = 0;
    while let Some(open) = lower[start..].find("<style") {
        let abs_open = start + open;
        if let Some(close) = lower[abs_open..].find("</style>") {
            let inner_start = lower[abs_open..].find('>').map(|p| abs_open + p + 1);
            if let Some(inner) = inner_start {
                let inner_end = abs_open + close;
                if inner_end > inner {
                    total_css += inner_end - inner;
                }
            }
            start = abs_open + close + 8;
        } else {
            break;
        }
    }

    let css_kb = total_css / 1024;
    if css_kb > 50 {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: format!(
                "Inline CSS is {css_kb}KB — consider extracting to external stylesheet (>50KB threshold)"
            ),
            section_id: None,
            element: Some("style".into()),
            fix: None,
        });
    }
}

fn check_external_resources(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let mut count = 0u32;

    // External stylesheets
    if let Ok(sel) = Selector::parse("link[rel=\"stylesheet\"]") {
        count += doc.select(&sel).count() as u32;
    }

    // External scripts
    if let Ok(sel) = Selector::parse("script[src]") {
        count += doc.select(&sel).count() as u32;
    }

    // External fonts (preconnect hints)
    if let Ok(sel) = Selector::parse("link[rel=\"preconnect\"]") {
        count += doc.select(&sel).count() as u32;
    }

    if count > 10 {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: format!(
                "{count} external resources — consider bundling or reducing (>10 threshold)"
            ),
            section_id: None,
            element: None,
            fix: None,
        });
    }
}

fn check_image_dimensions(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("img").expect("valid selector");
    for el in doc.select(&sel) {
        let has_width = el.value().attr("width").is_some();
        let has_height = el.value().attr("height").is_some();
        if !has_width || !has_height {
            let src = el.value().attr("src").unwrap_or("unknown");
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!(
                    "Image missing width/height attributes — causes layout shift: {}",
                    truncate(src, 50)
                ),
                section_id: None,
                element: Some(format!("img[src=\"{}\"]", truncate(src, 40))),
                fix: Some(AutoFix::AttributeFix {
                    selector: format!("img[src=\"{}\"]", truncate(src, 40)),
                    attribute: if !has_width { "width" } else { "height" }.into(),
                    value: "auto".into(),
                    description: "Add explicit dimensions to prevent layout shift".into(),
                }),
            });
        }
    }
}

fn check_script_defer(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("script[src]").expect("valid selector");
    for el in doc.select(&sel) {
        let has_defer = el.value().attr("defer").is_some();
        let has_async = el.value().attr("async").is_some();
        let has_type_module = el.value().attr("type") == Some("module");
        if !has_defer && !has_async && !has_type_module {
            let src = el.value().attr("src").unwrap_or("unknown");
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!(
                    "Script without defer/async blocks rendering: {}",
                    truncate(src, 50)
                ),
                section_id: None,
                element: Some(format!("script[src=\"{}\"]", truncate(src, 40))),
                fix: Some(AutoFix::AttributeFix {
                    selector: format!("script[src=\"{}\"]", truncate(src, 40)),
                    attribute: "defer".into(),
                    value: String::new(),
                    description: "Add defer attribute to non-blocking script".into(),
                }),
            });
        }
    }
}

fn check_render_blocking_css(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("link[rel=\"stylesheet\"]").expect("valid selector");
    for el in doc.select(&sel) {
        let has_media = el
            .value()
            .attr("media")
            .is_some_and(|m| m != "all" && !m.is_empty());
        if !has_media {
            let href = el.value().attr("href").unwrap_or("unknown");
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!("Render-blocking stylesheet: {}", truncate(href, 50)),
                section_id: None,
                element: Some("link[rel=stylesheet]".into()),
                fix: None,
            });
        }
    }
}

fn check_font_display(html: &str, issues: &mut Vec<QualityIssue>) {
    if html.contains("@font-face") && !html.contains("font-display") {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: "Missing font-display: swap on @font-face — may cause FOIT".into(),
            section_id: None,
            element: Some("@font-face".into()),
            fix: None,
        });
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
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
    fn test_flags_large_html() {
        let big = "x".repeat(250 * 1024);
        let html = format!("<html><body>{big}</body></html>");
        let result = check(&input(&html)).unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("KB")));
    }

    #[test]
    fn test_flags_missing_image_dimensions() {
        let result = check(&input(
            "<html><body><img src=\"photo.jpg\" alt=\"Photo\"></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("width/height")));
    }

    #[test]
    fn test_flags_render_blocking_css() {
        let result = check(&input(
            "<html><head><link rel=\"stylesheet\" href=\"style.css\"></head><body></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Render-blocking")));
    }

    #[test]
    fn test_clean_template_scores_high() {
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Test</title><style>body { margin: 0; }</style></head><body><h1>Hello</h1></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(
            result.score >= 90,
            "Clean template should score >= 90, got {}",
            result.score
        );
    }
}
