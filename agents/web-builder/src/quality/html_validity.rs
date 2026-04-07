//! HTML validity checker — structural validation of the generated HTML.
//!
//! Checks: DOCTYPE, structure, duplicate IDs, deprecated elements, missing charset,
//! nested <a> tags, block-in-inline.

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use scraper::{Html, Selector};
use std::collections::HashSet;

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_doctype(&input.html, &mut issues);
    check_html_structure(&doc, &mut issues);
    check_charset(&doc, &mut issues);
    check_title_present(&doc, &mut issues);
    check_duplicate_ids(&doc, &mut issues);
    check_deprecated_elements(&doc, &mut issues);
    check_nested_links(&doc, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "html_validity".into(),
        check_name: "HTML Validity".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_doctype(html: &str, issues: &mut Vec<QualityIssue>) {
    let trimmed = html.trim();
    if !trimmed.to_lowercase().starts_with("<!doctype html>")
        && !trimmed.to_lowercase().starts_with("<!doctype html ")
    {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing <!DOCTYPE html> — required for standards mode rendering".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }
}

fn check_html_structure(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let html_sel = Selector::parse("html").expect("valid");
    let head_sel = Selector::parse("head").expect("valid");
    let body_sel = Selector::parse("body").expect("valid");

    if doc.select(&html_sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing <html> element".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }
    if doc.select(&head_sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing <head> element".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }
    if doc.select(&body_sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing <body> element".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }

    // Check if head is empty
    if let Some(head) = doc.select(&head_sel).next() {
        let children: Vec<_> = head.children().collect();
        let has_elements = children.iter().any(|c| c.value().as_element().is_some());
        if !has_elements {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: "Empty <head> element — should contain at least charset and title".into(),
                section_id: None,
                element: Some("head".into()),
                fix: None,
            });
        }
    }
}

fn check_charset(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("meta[charset]").expect("valid");
    if doc.select(&sel).next().is_none() {
        // Also check http-equiv Content-Type
        let http_sel = Selector::parse("meta[http-equiv=\"Content-Type\"]").expect("valid");
        if doc.select(&http_sel).next().is_none() {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "Missing <meta charset> — character encoding should be specified".into(),
                section_id: None,
                element: Some("head".into()),
                fix: Some(AutoFix::MetaFix {
                    name: "charset".into(),
                    content: "UTF-8".into(),
                    description: "Add <meta charset=\"UTF-8\">".into(),
                }),
            });
        }
    }
}

fn check_title_present(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("title").expect("valid");
    if doc.select(&sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "Missing <title> element".into(),
            section_id: None,
            element: Some("head".into()),
            fix: None,
        });
    }
}

fn check_duplicate_ids(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("[id]").expect("valid");
    let mut seen = HashSet::new();
    for el in doc.select(&sel) {
        if let Some(id) = el.value().attr("id") {
            if !id.is_empty() && !seen.insert(id.to_string()) {
                issues.push(QualityIssue {
                    severity: Severity::Error,
                    message: format!("Duplicate ID: #{id}"),
                    section_id: None,
                    element: Some(format!("#{id}")),
                    fix: None,
                });
            }
        }
    }
}

fn check_deprecated_elements(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let deprecated = ["center", "font", "marquee", "blink", "spacer", "strike"];
    for tag in deprecated {
        if let Ok(sel) = Selector::parse(tag) {
            if doc.select(&sel).next().is_some() {
                issues.push(QualityIssue {
                    severity: Severity::Warning,
                    message: format!("Deprecated <{tag}> element — use CSS instead"),
                    section_id: None,
                    element: Some(tag.into()),
                    fix: None,
                });
            }
        }
    }
}

fn check_nested_links(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a a").expect("valid");
    if doc.select(&sel).next().is_some() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Nested <a> tags are invalid HTML".into(),
            section_id: None,
            element: Some("a > a".into()),
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
    fn test_detects_missing_doctype() {
        let result = check(&input("<html><head></head><body></body></html>")).unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("DOCTYPE")));
    }

    #[test]
    fn test_detects_duplicate_ids() {
        let result = check(&input(
            "<!DOCTYPE html><html><head></head><body><div id=\"foo\">A</div><div id=\"foo\">B</div></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Duplicate ID")));
    }

    #[test]
    fn test_detects_missing_charset() {
        let result = check(&input(
            "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("charset")));
        let charset = result
            .issues
            .iter()
            .find(|i| i.message.contains("charset"))
            .unwrap();
        assert!(charset.fix.is_some());
    }

    #[test]
    fn test_detects_deprecated_elements() {
        let result = check(&input(
            "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><title>T</title></head><body><center>Hi</center></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Deprecated")));
    }

    #[test]
    fn test_clean_template_valid() {
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>Valid Page</title></head><body><h1>Hello</h1></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(
            result.score >= 90,
            "Valid template should score >= 90, got {}. Issues: {:?}",
            result.score,
            result.issues
        );
    }
}
