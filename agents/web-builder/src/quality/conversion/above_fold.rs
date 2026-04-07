//! Above-the-Fold Assessment — checks if the value proposition is clear in the first viewport.

use super::ConversionInput;
use crate::quality::{compute_score, CheckResult, QualityError, QualityIssue, Severity};

/// Run above-the-fold assessment.
pub fn check(input: &ConversionInput) -> Result<CheckResult, QualityError> {
    let html = &input.quality_input.html;
    let lower = html.to_lowercase();
    let mut issues = Vec::new();

    let doc = scraper::Html::parse_document(html);

    // Check 1: Headline in first section (hero)
    let hero_sel = scraper::Selector::parse("[data-nexus-section='hero']")
        .unwrap_or_else(|_| scraper::Selector::parse("*").expect("universal selector"));
    let hero = doc.select(&hero_sel).next();

    let has_headline = if let Some(hero_el) = hero {
        let hero_html = hero_el.html().to_lowercase();
        hero_html.contains("<h1") || hero_html.contains("<h2")
    } else {
        // No hero section at all
        false
    };

    if !has_headline {
        issues.push(QualityIssue {
            severity: Severity::Error,
            message:
                "No headline (H1/H2) above the fold — visitors can't understand your value proposition"
                    .into(),
            section_id: Some("hero".into()),
            element: None,
            fix: None,
        });
    }

    // Check 2: Value proposition visible — headline + subheadline
    let has_subheadline = if let Some(hero_el) = doc.select(&hero_sel).next() {
        let hero_html = hero_el.html().to_lowercase();
        let has_h = hero_html.contains("<h1") || hero_html.contains("<h2");
        let has_p = hero_html.contains("<p");
        let has_sub_h = hero_html.contains("<h2") || hero_html.contains("<h3");
        has_h && (has_p || (hero_html.contains("<h1") && has_sub_h))
    } else {
        false
    };

    if has_headline && !has_subheadline {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message:
                "Headline without subheadline — add a supporting line to explain what you offer"
                    .into(),
            section_id: Some("hero".into()),
            element: None,
            fix: None,
        });
    }

    // Check 3: No excessive whitespace before content
    // Heuristic: if data-nexus-section="hero" appears very late in the HTML, content may be pushed down
    if let Some(hero_pos) = lower.find("data-nexus-section=\"hero\"") {
        let before_hero = &lower[..hero_pos];
        let non_tag_chars: usize = before_hero
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '<' && *c != '>')
            .count();
        // If there's very little actual content before hero, that's fine
        // If there's a huge amount, something may be pushing it down
        if non_tag_chars > 2000 {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "Large amount of content before hero section — value proposition may be pushed below the fold".into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    // Check 4: Media above fold
    if let Some(hero_el) = doc.select(&hero_sel).next() {
        let hero_html = hero_el.html().to_lowercase();
        let has_media = hero_html.contains("<img")
            || hero_html.contains("<video")
            || hero_html.contains("<svg")
            || hero_html.contains("background-image");
        if !has_media {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: "No visual media in hero — images or illustrations increase engagement"
                    .into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    // Check 5: Navigation is present
    let has_nav = lower.contains("<nav") || lower.contains("data-nexus-section=\"sidebar");
    if !has_nav {
        issues.push(QualityIssue {
            severity: Severity::Warning,
            message: "No navigation found — visitors can't orient themselves on the page".into(),
            section_id: None,
            element: None,
            fix: None,
        });
    }

    // Check 6: Loading speed proxy — check above-fold content size
    if let Some(hero_el) = doc.select(&hero_sel).next() {
        let hero_size = hero_el.html().len();
        if hero_size > 100_000 {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: "Hero section is very large (>100KB) — may slow initial render".into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    let score = compute_score(&issues);

    Ok(CheckResult {
        check_id: "above_fold".into(),
        check_name: "Above the Fold".into(),
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
            brief: Some("AI tool".into()),
        }
    }

    #[test]
    fn test_headline_present_above_fold() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Nav</nav>
            <div data-nexus-section="hero"><h1>Write Faster</h1><p>Subheadline</p></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(result.score >= 90, "score was {}", result.score);
    }

    #[test]
    fn test_missing_headline_penalized() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Nav</nav>
            <div data-nexus-section="hero"><p>Just a paragraph</p></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result.issues.iter().any(|i| i.severity == Severity::Error),
            "should have Error for missing headline"
        );
    }

    #[test]
    fn test_value_proposition_complete() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Nav</nav>
            <div data-nexus-section="hero">
                <h1>Main Headline</h1>
                <p>Supporting subheadline with more detail</p>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("subheadline")),
            "should not penalize when subheadline present"
        );
    }

    #[test]
    fn test_headline_only_penalized() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Nav</nav>
            <div data-nexus-section="hero"><h1>Just a headline</h1></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("subheadline")),
            "should warn about missing subheadline"
        );
    }

    #[test]
    fn test_nav_present() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <nav>Navigation</nav>
            <div data-nexus-section="hero"><h1>Title</h1><p>Sub</p></div>
            </body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("navigation")),
            "should not penalize when nav present"
        );
    }
}
