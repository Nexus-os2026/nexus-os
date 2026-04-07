//! SEO checker — static rules for search engine optimization.
//!
//! Checks: title, meta description, H1, Open Graph tags, link text quality.
//! LLM evaluation is optional (skipped if Ollama not running).

use super::{compute_score, AutoFix, CheckResult, QualityInput, QualityIssue, Severity};
use scraper::{Html, Selector};

pub fn check(input: &QualityInput) -> Result<CheckResult, super::QualityError> {
    let doc = Html::parse_document(&input.html);
    let mut issues = Vec::new();

    check_title(&doc, &mut issues);
    check_meta_description(&doc, &mut issues);
    check_h1(&doc, &mut issues);
    check_og_tags(&doc, &mut issues);
    check_canonical(&doc, &mut issues);
    check_link_text(&doc, &mut issues);
    check_images_alt_seo(&doc, &mut issues);

    let score = compute_score(&issues);
    Ok(CheckResult {
        check_id: "seo".into(),
        check_name: "SEO".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 70,
    })
}

fn check_title(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("title").expect("valid selector");
    match doc.select(&sel).next() {
        None => {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: "Missing <title> tag — critical for SEO".into(),
                section_id: None,
                element: Some("head".into()),
                fix: Some(AutoFix::MetaFix {
                    name: "title".into(),
                    content: "My Website".into(),
                    description: "Add a title tag".into(),
                }),
            });
        }
        Some(el) => {
            let text = el.text().collect::<String>();
            let len = text.trim().len();
            if len == 0 {
                issues.push(QualityIssue {
                    severity: Severity::Error,
                    message: "Empty <title> tag".into(),
                    section_id: None,
                    element: Some("title".into()),
                    fix: None,
                });
            } else if len < 30 {
                issues.push(QualityIssue {
                    severity: Severity::Info,
                    message: format!(
                        "Title is short ({len} chars) — aim for 30-60 characters for best SEO"
                    ),
                    section_id: None,
                    element: Some("title".into()),
                    fix: None,
                });
            } else if len > 60 {
                issues.push(QualityIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Title is long ({len} chars) — may be truncated in search results (aim for ≤60)"
                    ),
                    section_id: None,
                    element: Some("title".into()),
                    fix: None,
                });
            }
        }
    }
}

fn check_meta_description(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("meta[name=\"description\"]").expect("valid selector");
    match doc.select(&sel).next() {
        None => {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "Missing meta description — important for search result snippets".into(),
                section_id: None,
                element: Some("head".into()),
                fix: Some(AutoFix::MetaFix {
                    name: "description".into(),
                    content: "A modern, high-quality website built with Nexus Builder.".into(),
                    description: "Add meta description for search results".into(),
                }),
            });
        }
        Some(el) => {
            let content = el.value().attr("content").unwrap_or("");
            let len = content.trim().len();
            if len < 120 && len > 0 {
                issues.push(QualityIssue {
                    severity: Severity::Info,
                    message: format!("Meta description is short ({len} chars) — aim for 120-160"),
                    section_id: None,
                    element: Some("meta[name=description]".into()),
                    fix: None,
                });
            } else if len > 160 {
                issues.push(QualityIssue {
                    severity: Severity::Info,
                    message: format!(
                        "Meta description is long ({len} chars) — may be truncated (aim for ≤160)"
                    ),
                    section_id: None,
                    element: Some("meta[name=description]".into()),
                    fix: None,
                });
            }
        }
    }
}

fn check_h1(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("h1").expect("valid selector");
    let h1s: Vec<_> = doc.select(&sel).collect();
    if h1s.is_empty() {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "No <h1> tag found — each page should have exactly one H1 for SEO".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    } else if h1s.len() > 1 {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: format!(
                "Multiple H1 tags found ({}) — consider using one H1 per page",
                h1s.len()
            ),
            section_id: None,
            element: Some("h1".into()),
            fix: None,
        });
    }
}

fn check_og_tags(doc: &Html, issues: &mut Vec<QualityIssue>) {
    // Use meta[property] selector to find all OG tags, then check which are present.
    let sel = Selector::parse("meta[property]").expect("valid selector");
    let present: Vec<String> = doc
        .select(&sel)
        .filter_map(|el| el.value().attr("property").map(String::from))
        .collect();

    let required_og = [
        (
            "og:title",
            "Open Graph title — improves social media sharing",
        ),
        ("og:description", "Open Graph description"),
        ("og:image", "Open Graph image — adds preview when shared"),
    ];

    for (property, desc) in required_og {
        if !present.iter().any(|p| p == property) {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!("Missing {property} — {desc}"),
                section_id: None,
                element: Some("head".into()),
                fix: if property != "og:image" {
                    Some(AutoFix::MetaFix {
                        name: property.to_string(),
                        content: match property {
                            "og:title" => "My Website".into(),
                            "og:description" => "Built with Nexus Builder".into(),
                            _ => String::new(),
                        },
                        description: format!("Add {property} meta tag"),
                    })
                } else {
                    None
                },
            });
        }
    }
}

fn check_canonical(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("link[rel=\"canonical\"]").expect("valid selector");
    if doc.select(&sel).next().is_none() {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message: "No canonical URL — helps prevent duplicate content issues".into(),
            section_id: None,
            element: Some("head".into()),
            fix: None,
        });
    }
}

fn check_link_text(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("a").expect("valid selector");
    let bad_texts = ["click here", "here", "read more", "more", "link"];
    for el in doc.select(&sel) {
        let text = el.text().collect::<String>();
        let lower = text.trim().to_lowercase();
        if bad_texts.contains(&lower.as_str()) {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!(
                    "Link text \"{}\" is not descriptive — use meaningful text for accessibility and SEO",
                    text.trim()
                ),
                section_id: None,
                element: Some("a".into()),
                fix: None,
            });
        }
    }
}

fn check_images_alt_seo(doc: &Html, issues: &mut Vec<QualityIssue>) {
    let sel = Selector::parse("img[alt]").expect("valid selector");
    for el in doc.select(&sel) {
        let alt = el.value().attr("alt").unwrap_or("");
        if alt.len() > 125 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!(
                    "Image alt text is very long ({} chars) — consider shortening",
                    alt.len()
                ),
                section_id: None,
                element: Some("img".into()),
                fix: None,
            });
        }
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
    fn test_detects_missing_title() {
        let result = check(&input("<html><head></head><body></body></html>")).unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("title")));
        let title_issue = result
            .issues
            .iter()
            .find(|i| i.message.contains("Missing <title>"))
            .unwrap();
        assert!(title_issue.fix.is_some());
    }

    #[test]
    fn test_detects_long_title() {
        let long_title = "A".repeat(70);
        let html = format!("<html><head><title>{long_title}</title></head><body></body></html>");
        let result = check(&input(&html)).unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("long")));
    }

    #[test]
    fn test_detects_missing_meta_description() {
        let result = check(&input(
            "<html><head><title>Test Page</title></head><body></body></html>",
        ))
        .unwrap();
        assert!(result
            .issues
            .iter()
            .any(|i| i.message.contains("meta description")));
        let desc_issue = result
            .issues
            .iter()
            .find(|i| i.message.contains("meta description"))
            .unwrap();
        assert!(desc_issue.fix.is_some());
    }

    #[test]
    fn test_detects_missing_og_tags() {
        let result = check(&input(
            "<html><head><title>Test</title></head><body></body></html>",
        ))
        .unwrap();
        assert!(result.issues.iter().any(|i| i.message.contains("og:")));
    }

    #[test]
    fn test_clean_template_scores_high() {
        let html = r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><title>My Great SaaS Product - Launch Today</title><meta name="description" content="Discover the best SaaS product for teams. Boost productivity by 10x with our AI-powered platform. Try free today."><meta property="og:title" content="My Great SaaS"><meta property="og:description" content="Best SaaS product"><meta property="og:image" content="https://example.com/og.png"><link rel="canonical" href="https://example.com"></head><body><h1>Welcome to My SaaS</h1><p>Great content here.</p></body></html>"#;
        let result = check(&input(html)).unwrap();
        assert!(
            result.score >= 80,
            "Clean SEO template should score >= 80, got {}",
            result.score
        );
    }
}
