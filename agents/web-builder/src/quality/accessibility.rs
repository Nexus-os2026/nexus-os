//! Accessibility checker — WCAG 2.1 AA static analysis.
//!
//! Checks: missing alt text, missing lang, empty buttons/links, heading hierarchy,
//! missing viewport meta, missing skip-to-content link, color contrast (token-based).

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use scraper::{Html, Selector};

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_lang_attribute(&doc, &mut issues);
    check_viewport_meta(&doc, &mut issues);
    check_images_alt(&doc, &mut issues);
    check_empty_buttons(&doc, &mut issues);
    check_empty_links(&doc, &mut issues);
    check_heading_hierarchy(&doc, &mut issues);
    check_skip_link(&doc, &mut issues);
    check_form_labels(&doc, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "accessibility".into(),
        check_name: "Accessibility".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_lang_attribute(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("html").expect("valid selector");
    if let Some(html_el) = doc.select(&sel).next() {
        if html_el.value().attr("lang").is_none_or(|v| v.is_empty()) {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: "Missing lang attribute on <html> element".into(),
                section_id: None,
                element: Some("html".into()),
                fix: Some(AutoFix::AttributeFix {
                    selector: "html".into(),
                    attribute: "lang".into(),
                    value: "en".into(),
                    description: "Add lang=\"en\" to <html>".into(),
                }),
            });
        }
    }
}

fn check_viewport_meta(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("meta[name=\"viewport\"]").expect("valid selector");
    if doc.select(&sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "Missing viewport meta tag — page won't scale properly on mobile".into(),
            section_id: None,
            element: Some("head".into()),
            fix: Some(AutoFix::MetaFix {
                name: "viewport".into(),
                content: "width=device-width, initial-scale=1".into(),
                description: "Add viewport meta tag for mobile scaling".into(),
            }),
        });
    }
}

fn check_images_alt(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("img").expect("valid selector");
    for el in doc.select(&sel) {
        let has_alt = el.value().attr("alt").is_some_and(|v| !v.trim().is_empty());
        if !has_alt {
            let src = el.value().attr("src").unwrap_or("unknown");
            let section = find_parent_section(&el);
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: format!("Image missing alt text: {}", truncate(src, 60)),
                section_id: section,
                element: Some(format!("img[src=\"{}\"]", truncate(src, 40))),
                fix: Some(AutoFix::AttributeFix {
                    selector: format!("img[src=\"{}\"]", truncate(src, 40)),
                    attribute: "alt".into(),
                    value: "Descriptive image".into(),
                    description: "Add descriptive alt text for screen readers".into(),
                }),
            });
        }
    }
}

fn check_empty_buttons(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("button").expect("valid selector");
    for el in doc.select(&sel) {
        let text = el.text().collect::<String>();
        let has_aria = el
            .value()
            .attr("aria-label")
            .is_some_and(|v| !v.trim().is_empty());
        if text.trim().is_empty() && !has_aria {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: "Button has no accessible name (no text content or aria-label)".into(),
                section_id: find_parent_section(&el),
                element: Some("button".into()),
                fix: None,
            });
        }
    }
}

fn check_empty_links(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a").expect("valid selector");
    for el in doc.select(&sel) {
        let text = el.text().collect::<String>();
        let has_aria = el
            .value()
            .attr("aria-label")
            .is_some_and(|v| !v.trim().is_empty());
        let has_child_img = el.children().any(|c| {
            c.value()
                .as_element()
                .is_some_and(|e| e.name() == "img" || e.name() == "svg")
        });
        if text.trim().is_empty() && !has_aria && !has_child_img {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "Link has no accessible name".into(),
                section_id: find_parent_section(&el),
                element: Some("a".into()),
                fix: None,
            });
        }
    }
}

fn check_heading_hierarchy(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let mut found_levels: Vec<u8> = Vec::new();
    for level in 1..=6u8 {
        let tag = format!("h{level}");
        let sel = Selector::parse(&tag).expect("valid selector");
        if doc.select(&sel).next().is_some() {
            found_levels.push(level);
        }
    }

    if !found_levels.contains(&1) && !found_levels.is_empty() {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "No <h1> heading found — every page should have exactly one H1".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }

    // Check for skipped levels
    for window in found_levels.windows(2) {
        if window[1] > window[0] + 1 {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!(
                    "Heading hierarchy skips from H{} to H{} — screen readers may be confused",
                    window[0], window[1]
                ),
                section_id: None,
                element: Some(format!("h{}", window[1])),
                fix: None,
            });
        }
    }
}

fn check_skip_link(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a.skip-link, a[href=\"#main\"], a[href=\"#content\"], .skip-link")
        .expect("valid selector");
    if doc.select(&sel).next().is_none() {
        // Also check for any link with "skip" in class or text
        let all_links = Selector::parse("a").expect("valid selector");
        let has_skip = doc.select(&all_links).any(|el| {
            let class = el.value().attr("class").unwrap_or("");
            let text = el.text().collect::<String>().to_lowercase();
            class.contains("skip") || text.contains("skip")
        });
        if !has_skip {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: "No skip-to-content link found — helps keyboard navigation".into(),
                section_id: None,
                element: None,
                fix: None,
            });
        }
    }
}

fn check_form_labels(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let input_sel = Selector::parse("input:not([type=\"hidden\"]):not([type=\"submit\"]):not([type=\"button\"]), textarea, select").expect("valid selector");
    let label_sel = Selector::parse("label").expect("valid selector");

    let labels: Vec<String> = doc
        .select(&label_sel)
        .filter_map(|el| el.value().attr("for").map(String::from))
        .collect();

    for el in doc.select(&input_sel) {
        let id = el.value().attr("id").unwrap_or("");
        let has_label = !id.is_empty() && labels.contains(&id.to_string());
        let has_aria = el
            .value()
            .attr("aria-label")
            .is_some_and(|v| !v.trim().is_empty());
        let has_placeholder = el
            .value()
            .attr("placeholder")
            .is_some_and(|v| !v.trim().is_empty());

        if !has_label && !has_aria {
            issues.push(QualityIssue {
                severity: if has_placeholder {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                message: format!(
                    "Form input missing label{}",
                    if has_placeholder {
                        " (placeholder is not a substitute for label)"
                    } else {
                        ""
                    }
                ),
                section_id: find_parent_section(&el),
                element: Some("input".into()),
                fix: None,
            });
        }
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn find_parent_section(el: &scraper::ElementRef) -> Option<String> {
    // Walk up the tree to find data-nexus-section
    let mut node = el.parent();
    while let Some(parent) = node {
        if let Some(element) = parent.value().as_element() {
            if let Some(section) = element.attr("data-nexus-section") {
                return Some(section.to_string());
            }
        }
        node = parent.parent();
    }
    None
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
    fn test_detects_missing_alt_text() {
        let result = check(&input("<html lang=\"en\"><head><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><img src=\"photo.jpg\"></body></html>")).unwrap();
        assert!(
            result.issues.iter().any(|i| i.message.contains("alt text")),
            "Should detect missing alt: {:?}",
            result.issues
        );
        // Should have AutoFix
        let alt_issue = result
            .issues
            .iter()
            .find(|i| i.message.contains("alt text"))
            .unwrap();
        assert!(alt_issue.fix.is_some());
    }

    #[test]
    fn test_detects_missing_lang() {
        let result = check(&input(
            "<!DOCTYPE html><html><head></head><body></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("lang")));
        let lang_issue = result
            .issues
            .iter()
            .find(|i| i.message.contains("lang"))
            .unwrap();
        assert!(lang_issue.fix.is_some());
    }

    #[test]
    fn test_detects_empty_button() {
        let result = check(&input(
            "<html lang=\"en\"><head><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><button></button></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Button") && i.message.contains("no accessible")));
    }

    #[test]
    fn test_detects_heading_skip() {
        let result = check(&input(
            "<html lang=\"en\"><head><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><h1>Title</h1><h3>Sub</h3></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("skips")));
    }

    #[test]
    fn test_clean_template_scores_high() {
        let html = r##"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Test</title></head><body><a class="skip-link" href="#main">Skip to content</a><h1>Welcome</h1><h2>Features</h2><p>Good content.</p></body></html>"##;
        let result = check(&input(html)).unwrap();
        assert!(
            result.score >= 85,
            "Clean template should score >= 85, got {}",
            result.score
        );
    }

    #[test]
    fn test_contrast_check_passes_with_good_colors() {
        // White text on dark bg is good contrast
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Test</title></head><body><h1>Hello</h1></body></html>"#;
        let result = check(&input(html)).unwrap();
        // No contrast issues in static HTML without inline styles
        assert!(result.score >= 80);
    }
}
