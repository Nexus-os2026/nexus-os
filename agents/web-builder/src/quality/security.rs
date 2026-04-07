//! Security checker — XSS, CSP, and OWASP static analysis.
//!
//! Checks: inline script safety, CSP meta, target="_blank" rel, javascript: URLs,
//! eval()/document.write(), external script integrity, iframe sandbox.

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use scraper::{Html, Selector};

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_csp_meta(&doc, &mut issues);
    check_target_blank(&doc, &mut issues);
    check_javascript_urls(&doc, &mut issues);
    check_dangerous_js(&input.html, &mut issues);
    check_external_script_integrity(&doc, &mut issues);
    check_iframe_sandbox(&doc, &mut issues);
    check_mixed_content(&doc, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "security".into(),
        check_name: "Security".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_csp_meta(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel =
        Selector::parse("meta[http-equiv=\"Content-Security-Policy\"]").expect("valid selector");
    if doc.select(&sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: "No Content-Security-Policy meta tag — adds defense-in-depth against XSS"
                .into(),
            section_id: None,
            element: Some("head".into()),
            fix: Some(AutoFix::MetaFix {
                name: "Content-Security-Policy".into(),
                content: "default-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; script-src 'self' 'unsafe-inline'".into(),
                description: "Add a reasonable Content-Security-Policy".into(),
            }),
        });
    }
}

fn check_target_blank(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a[target=\"_blank\"]").expect("valid selector");
    for el in doc.select(&sel) {
        let rel = el.value().attr("rel").unwrap_or("");
        if !rel.contains("noopener") || !rel.contains("noreferrer") {
            let href = el.value().attr("href").unwrap_or("unknown");
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!(
                    "Link with target=\"_blank\" missing rel=\"noopener noreferrer\": {}",
                    truncate(href, 50)
                ),
                section_id: None,
                element: Some(format!("a[href=\"{}\"]", truncate(href, 40))),
                fix: Some(AutoFix::AttributeFix {
                    selector: format!("a[href=\"{}\"][target=\"_blank\"]", truncate(href, 40)),
                    attribute: "rel".into(),
                    value: "noopener noreferrer".into(),
                    description: "Add rel=\"noopener noreferrer\" for security".into(),
                }),
            });
        }
    }
}

fn check_javascript_urls(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a[href]").expect("valid selector");
    for el in doc.select(&sel) {
        let href = el.value().attr("href").unwrap_or("");
        if href.trim().to_lowercase().starts_with("javascript:") {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: "Link uses javascript: URL — XSS risk".into(),
                section_id: None,
                element: Some("a[href^=javascript]".into()),
                fix: None,
            });
        }
    }
}

fn check_dangerous_js(html: &str, issues: &mut Vec<QualityIssue>) {
    let lower = html.to_lowercase();

    // Find inline scripts and check for dangerous patterns
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<script") {
        let abs_start = pos + start;
        // Skip external scripts (they have src=)
        if let Some(close) = lower[abs_start..].find('>') {
            let tag = &lower[abs_start..abs_start + close];
            if tag.contains("src=") {
                pos = abs_start + close;
                continue;
            }
        }

        if let Some(end) = lower[abs_start..].find("</script>") {
            let script_content = &html[abs_start..abs_start + end];
            let script_lower = script_content.to_lowercase();

            if script_lower.contains("eval(") {
                issues.push(QualityIssue {
                    severity: Severity::Error,
                    message: "Inline script uses eval() — security risk".into(),
                    section_id: None,
                    element: Some("script".into()),
                    fix: None,
                });
            }

            if script_lower.contains("document.write(") {
                issues.push(QualityIssue {
                    severity: Severity::Error,
                    message: "Inline script uses document.write() — security and performance risk"
                        .into(),
                    section_id: None,
                    element: Some("script".into()),
                    fix: None,
                });
            }

            pos = abs_start + end + 9;
        } else {
            break;
        }
    }
}

fn check_external_script_integrity(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("script[src]").expect("valid selector");
    for el in doc.select(&sel) {
        let src = el.value().attr("src").unwrap_or("");
        // Only flag external (CDN) scripts, not local ones
        if (src.starts_with("http://") || src.starts_with("https://"))
            && el.value().attr("integrity").is_none()
        {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!(
                    "External script without integrity hash: {}",
                    truncate(src, 50)
                ),
                section_id: None,
                element: Some(format!("script[src=\"{}\"]", truncate(src, 40))),
                fix: None,
            });
        }
    }
}

fn check_iframe_sandbox(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("iframe").expect("valid selector");
    for el in doc.select(&sel) {
        if el.value().attr("sandbox").is_none() {
            let src = el.value().attr("src").unwrap_or("unknown");
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!("iframe without sandbox attribute: {}", truncate(src, 50)),
                section_id: None,
                element: Some("iframe".into()),
                fix: Some(AutoFix::AttributeFix {
                    selector: format!("iframe[src=\"{}\"]", truncate(src, 40)),
                    attribute: "sandbox".into(),
                    value: "allow-scripts allow-same-origin".into(),
                    description: "Add sandbox attribute to iframe".into(),
                }),
            });
        }
    }
}

fn check_mixed_content(doc: &Html, issues: &mut Vec<QualityIssue>) {
    // Check for http:// URLs in src/href attributes
    let selectors = ["img[src]", "script[src]", "link[href]", "iframe[src]"];
    for sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in doc.select(&sel) {
                let attr = if sel_str.contains("[src]") {
                    "src"
                } else {
                    "href"
                };
                let val = el.value().attr(attr).unwrap_or("");
                if val.starts_with("http://") && !val.starts_with("http://localhost") {
                    issues.push(QualityIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Insecure HTTP resource (mixed content risk): {}",
                            truncate(val, 50)
                        ),
                        section_id: None,
                        element: Some(sel_str.into()),
                        fix: None,
                    });
                }
            }
        }
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
    fn test_detects_missing_csp() {
        let result = check(&input("<html><head></head><body></body></html>")).unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("Content-Security-Policy")));
        let csp = result
            .issues
            .iter()
            .find(|i| i.message.contains("Content-Security-Policy"))
            .unwrap();
        assert!(csp.fix.is_some());
    }

    #[test]
    fn test_detects_target_blank_no_rel() {
        let result = check(&input(
            "<html><body><a href=\"https://example.com\" target=\"_blank\">Link</a></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("noopener")));
    }

    #[test]
    fn test_detects_javascript_url() {
        let result = check(&input(
            "<html><body><a href=\"javascript:alert(1)\">XSS</a></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("javascript:")));
        let js_issue = result
            .issues
            .iter()
            .find(|i| i.message.contains("javascript:"))
            .unwrap();
        assert_eq!(js_issue.severity, Severity::Error);
    }

    #[test]
    fn test_detects_eval_in_script() {
        let result = check(&input(
            "<html><body><script>eval('alert(1)')</script></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("eval()")));
    }

    #[test]
    fn test_clean_template_scores_100() {
        // Static HTML with no forms, no external scripts, no target=_blank
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta http-equiv="Content-Security-Policy" content="default-src 'self'"><title>Test</title><style>body{margin:0}</style></head><body><h1>Hello</h1><p>Content.</p></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert_eq!(
            result.score, 100,
            "Clean static template should score 100, got {}. Issues: {:?}",
            result.score, result.issues
        );
    }
}
