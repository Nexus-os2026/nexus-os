//! CTA Placement Analysis — checks CTA visibility, positioning, contrast, and urgency.

use super::ConversionInput;
use crate::quality::{compute_score, AutoFix, CheckResult, QualityError, QualityIssue, Severity};

/// Run CTA placement analysis.
pub fn check(input: &ConversionInput) -> Result<CheckResult, QualityError> {
    let html = &input.quality_input.html;
    let lower = html.to_lowercase();
    let mut issues = Vec::new();

    // Parse with scraper for structured checks
    let doc = scraper::Html::parse_document(html);

    // Check 1: CTA exists in hero section
    let hero_sel = scraper::Selector::parse("[data-nexus-section='hero']")
        .unwrap_or_else(|_| scraper::Selector::parse("*").expect("universal selector"));
    let hero = doc.select(&hero_sel).next();

    let has_hero_cta = if let Some(hero_el) = hero {
        let hero_html = hero_el.html().to_lowercase();
        hero_html.contains("btn")
            || hero_html.contains("cta")
            || hero_html.contains("<button")
            || (hero_html.contains("<a") && hero_html.contains("href"))
    } else {
        false
    };

    if !has_hero_cta {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message: "No CTA found in hero section — visitors need a clear action above the fold"
                .into(),
            section_id: Some("hero".into()),
            element: None,
            fix: Some(AutoFix::ContentFix {
                slot_name: "cta_primary".into(),
                section_id: "hero".into(),
                suggested_text: "Get Started Free".into(),
                description: "Add a primary CTA button to the hero section".into(),
            }),
        });
    }

    // Check 2: CTA is a button or link (not plain text)
    let has_button_cta = lower.contains("<button")
        || lower.contains("class=\"btn")
        || lower.contains("class='btn")
        || (lower.contains("<a") && (lower.contains("btn") || lower.contains("padding")));

    if has_hero_cta && !has_button_cta {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "CTA appears as plain text — use a styled button for better click-through"
                .into(),
            section_id: Some("hero".into()),
            element: None,
            fix: None,
        });
    }

    // Check 3: CTA contrast ratio (heuristic — check for light text on light bg or dark on dark)
    // We check if the CTA has explicit color styling
    if has_hero_cta {
        let has_explicit_colors = lower.contains("background")
            && (lower.contains("color:#") || lower.contains("color: #"));
        if !has_explicit_colors && lower.contains("btn") {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message:
                    "CTA button may lack sufficient contrast — verify button colors are distinct"
                        .into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    // Check 4: CTA size — check for adequate padding
    let has_adequate_padding =
        lower.contains("padding") && (lower.contains("padding:") || lower.contains("padding-"));

    if has_hero_cta && !has_adequate_padding {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "CTA button may be too small — ensure at least 44px touch target".into(),
            section_id: Some("hero".into()),
            element: None,
            fix: None,
        });
    }

    // Check 5: Multiple CTAs — check if CTA appears in both hero and closing section
    let has_closing_cta = lower.contains("data-nexus-section=\"cta\"")
        || lower.contains("data-nexus-section='cta'")
        || lower.contains("data-nexus-section=\"newsletter\"")
        || lower.contains("data-nexus-section='newsletter'");

    if has_hero_cta && !has_closing_cta {
        issues.push(QualityIssue {
            severity: Severity::Info,
            message:
                "CTA only in hero — add a closing CTA section so users don't have to scroll back"
                    .into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }

    // Check 6: CTA above the fold (estimate: within first ~800px of content)
    // Heuristic: if hero section is within the first 2000 chars, it's likely above fold
    if let Some(hero_pos) = lower.find("data-nexus-section=\"hero\"") {
        if hero_pos > 3000 {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message:
                    "Hero section (with CTA) may be below the fold — too much content before it"
                        .into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    // Check 7: Secondary CTA exists
    if has_hero_cta {
        let cta_count = lower.matches("btn").count() + lower.matches("<button").count();
        if cta_count <= 1 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: "Only one CTA option — add a secondary CTA (e.g., 'Learn More') for hesitant visitors".into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    let score = compute_score(&issues);

    Ok(CheckResult {
        check_id: "cta_placement".into(),
        check_name: "CTA Placement".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 60,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::ContentPayload;
    use crate::quality::conversion::ConversionInput;
    use crate::quality::QualityInput;
    use crate::variant::VariantSelection;

    fn make_input(html: &str) -> ConversionInput {
        ConversionInput {
            quality_input: QualityInput {
                html: html.to_string(),
                output_dir: None,
                template_id: "saas_landing".into(),
                sections: vec![],
            },
            content_payload: ContentPayload {
                template_id: "saas_landing".into(),
                variant: VariantSelection::default(),
                sections: vec![],
            },
            template_id: "saas_landing".into(),
            brief: Some("AI writing tool".into()),
        }
    }

    #[test]
    fn test_hero_with_cta_scores_high() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Write Faster</h1>
                <a class="btn" style="padding:14px 28px;background:#6366f1;color:#fff;" href="#">Start Free Trial</a>
                <a class="btn-secondary" href="#">Learn More</a>
            </div>
            <div data-nexus-section="cta"><a class="btn" href="#">Get Started</a></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(result.score >= 90, "score was {}", result.score);
    }

    #[test]
    fn test_hero_without_cta_penalized() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero"><h1>Hello World</h1><p>Some text</p></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result.issues.iter().any(|i| i.severity == Severity::Error),
            "should have Error for missing CTA"
        );
        assert!(
            result.score <= 90,
            "score was {} — should be penalized",
            result.score
        );
    }

    #[test]
    fn test_cta_contrast_check() {
        // CTA with btn class but no explicit color styling
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <a class="btn" href="#">Click</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        // Should have an info about contrast
        assert!(
            result.issues.iter().any(|i| i.message.contains("contrast")),
            "should mention contrast"
        );
    }

    #[test]
    fn test_closing_cta_section_bonus() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <a class="btn" style="padding:12px;background:#000;color:#fff;" href="#">Start</a>
                <a class="btn" href="#">Learn</a>
            </div>
            <div data-nexus-section="cta"><a class="btn" href="#">Go</a></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("closing CTA")),
            "should not penalize when closing CTA exists"
        );
    }

    #[test]
    fn test_cta_above_fold() {
        // Hero early in the document — should be above fold
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Nav</nav>
            <div data-nexus-section="hero">
                <a class="btn" style="padding:12px;background:#000;color:#fff;" href="#">Go</a>
                <a class="btn" href="#">More</a>
            </div>
            <div data-nexus-section="cta"><a class="btn" href="#">CTA</a></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("below the fold")),
            "hero is early, should not be flagged as below fold"
        );
    }
}
