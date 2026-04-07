//! Copy Clarity Scoring — evaluates headline specificity, CTA verbs, and benefit language.

use super::ConversionInput;
use crate::quality::{compute_score, AutoFix, CheckResult, QualityError, QualityIssue, Severity};

/// Common action verbs for CTAs.
const CTA_ACTION_VERBS: &[&str] = &[
    "start",
    "get",
    "try",
    "join",
    "sign",
    "download",
    "learn",
    "discover",
    "build",
    "create",
    "launch",
    "explore",
    "request",
    "book",
    "schedule",
    "shop",
    "buy",
    "subscribe",
    "claim",
    "unlock",
    "upgrade",
    "view",
    "watch",
    "read",
    "contact",
    "apply",
    "reserve",
];

/// Placeholder/generic headline patterns that indicate unfinished content.
const PLACEHOLDER_PATTERNS: &[&str] = &[
    "your amazing product",
    "welcome to our site",
    "lorem ipsum",
    "your company name",
    "headline goes here",
    "enter your headline",
    "placeholder",
    "coming soon",
    "under construction",
    "website title",
    "your business name",
    "sample text",
    "insert title",
];

/// Benefit-oriented words that suggest outcome-focused copy.
const BENEFIT_WORDS: &[&str] = &[
    "you",
    "your",
    "save",
    "faster",
    "easier",
    "better",
    "increase",
    "reduce",
    "improve",
    "grow",
    "boost",
    "free",
    "instant",
    "simple",
    "powerful",
    "effortless",
    "automated",
    "revenue",
    "profit",
    "time",
];

/// Run copy clarity scoring.
pub fn check(input: &ConversionInput) -> Result<CheckResult, QualityError> {
    let html = &input.quality_input.html;
    let mut issues = Vec::new();

    let doc = scraper::Html::parse_document(html);

    // Extract headline text from hero section
    let hero_sel = scraper::Selector::parse("[data-nexus-section='hero']")
        .unwrap_or_else(|_| scraper::Selector::parse("*").expect("universal selector"));

    let headline_text = extract_heading_text(&doc, &hero_sel);
    let cta_text = extract_cta_text(&doc, &hero_sel);

    // Check 1: Headline length
    if let Some(ref headline) = headline_text {
        let len = headline.len();
        if len < 20 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!(
                    "Headline is very short ({len} chars) — may be too vague to communicate value"
                ),
                section_id: Some("hero".into()),
                element: Some("h1".into()),
                fix: None,
            });
        } else if len > 80 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message: format!(
                    "Headline is long ({len} chars) — consider shortening for immediate clarity"
                ),
                section_id: Some("hero".into()),
                element: Some("h1".into()),
                fix: None,
            });
        }
    }

    // Check 2: Headline is not placeholder
    if let Some(ref headline) = headline_text {
        let lower_headline = headline.to_lowercase();
        if PLACEHOLDER_PATTERNS
            .iter()
            .any(|p| lower_headline.contains(p))
        {
            issues.push(QualityIssue {
                severity: Severity::Error,
                message: format!(
                    "Headline appears to be placeholder text: \"{}\" — replace with a specific value proposition",
                    truncate(headline, 50)
                ),
                section_id: Some("hero".into()),
                element: Some("h1".into()),
                fix: Some(AutoFix::ContentFix {
                    slot_name: "headline".into(),
                    section_id: "hero".into(),
                    suggested_text: "Write 10x Faster with AI-Powered Tools".into(),
                    description: "Replace placeholder headline with specific value proposition"
                        .into(),
                }),
            });
        }
    }

    // Check 3: CTA uses action verb
    if let Some(ref cta) = cta_text {
        let first_word = cta.split_whitespace().next().unwrap_or("").to_lowercase();
        let has_verb = CTA_ACTION_VERBS.iter().any(|v| first_word == *v);
        if !has_verb && !cta.is_empty() {
            issues.push(QualityIssue {
                severity: Severity::Warning,
                message: format!(
                    "CTA \"{}\" doesn't start with an action verb — try \"Start\", \"Get\", or \"Try\"",
                    truncate(cta, 30)
                ),
                section_id: Some("hero".into()),
                element: None,
                fix: Some(AutoFix::ContentFix {
                    slot_name: "cta_primary".into(),
                    section_id: "hero".into(),
                    suggested_text: format!("Get {}", cta),
                    description: "Add action verb to CTA for clearer call-to-action".into(),
                }),
            });
        }
    }

    // Check 4: Feature descriptions have benefit language
    let lower = html.to_lowercase();
    let features_sel = scraper::Selector::parse("[data-nexus-section='features']")
        .unwrap_or_else(|_| scraper::Selector::parse("*").expect("universal selector"));
    if let Some(features_el) = doc.select(&features_sel).next() {
        let features_text = features_el.text().collect::<String>().to_lowercase();
        let benefit_count = BENEFIT_WORDS
            .iter()
            .filter(|w| features_text.contains(*w))
            .count();
        if benefit_count < 2 && features_text.len() > 50 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message:
                    "Feature descriptions lack benefit language — mention outcomes (save time, increase revenue) not just features"
                        .into(),
                section_id: Some("features".into()),
                element: None,
                fix: None,
            });
        }
    }

    // Check 5: Consistent messaging — headline and CTA keyword overlap
    if let (Some(ref headline), Some(ref cta)) = (&headline_text, &cta_text) {
        let headline_words: Vec<&str> = headline.split_whitespace().collect();
        let cta_words: Vec<&str> = cta.split_whitespace().collect();
        let headline_lower: Vec<String> = headline_words.iter().map(|w| w.to_lowercase()).collect();
        let cta_lower: Vec<String> = cta_words.iter().map(|w| w.to_lowercase()).collect();

        // Also check subheadline text
        let has_overlap = headline_lower
            .iter()
            .any(|w| cta_lower.contains(w) && w.len() > 3)
            || lower.contains(&cta.to_lowercase());

        if !has_overlap && headline_words.len() > 3 && cta_words.len() > 1 {
            issues.push(QualityIssue {
                severity: Severity::Info,
                message:
                    "Headline and CTA have no shared keywords — consistent messaging improves conversion"
                        .into(),
                section_id: Some("hero".into()),
                element: None,
                fix: None,
            });
        }
    }

    let score = compute_score(&issues);

    Ok(CheckResult {
        check_id: "copy_clarity".into(),
        check_name: "Copy Clarity".into(),
        score,
        max_score: 100,
        issues,
        passed: score >= 60,
    })
}

/// Extract the text content of the first H1 or H2 in the hero section.
fn extract_heading_text(doc: &scraper::Html, hero_sel: &scraper::Selector) -> Option<String> {
    let hero = doc.select(hero_sel).next()?;
    let h1_sel = scraper::Selector::parse("h1").ok()?;
    if let Some(h1) = hero.select(&h1_sel).next() {
        let text = h1.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    let h2_sel = scraper::Selector::parse("h2").ok()?;
    if let Some(h2) = hero.select(&h2_sel).next() {
        let text = h2.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Extract the text of the first CTA-like element (button or link with btn class).
fn extract_cta_text(doc: &scraper::Html, hero_sel: &scraper::Selector) -> Option<String> {
    let hero = doc.select(hero_sel).next()?;

    // Try buttons first
    if let Ok(btn_sel) = scraper::Selector::parse("button") {
        if let Some(btn) = hero.select(&btn_sel).next() {
            let text = btn.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    // Try links with btn class
    if let Ok(a_sel) = scraper::Selector::parse("a") {
        for a in hero.select(&a_sel) {
            let classes = a.value().attr("class").unwrap_or("");
            if classes.contains("btn") || classes.contains("cta") {
                let text = a.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }

    // Try any link in hero as fallback
    if let Ok(a_sel) = scraper::Selector::parse("a[href]") {
        if let Some(a) = hero.select(&a_sel).next() {
            let text = a.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    None
}

/// Truncate a string to max_len, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
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
    fn test_action_verb_cta_passes() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Write 10x Faster with AI</h1>
                <a class="btn" href="#">Start Free Trial</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("action verb")),
            "should not penalize CTA starting with action verb"
        );
    }

    #[test]
    fn test_non_verb_cta_penalized() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Write 10x Faster with AI</h1>
                <a class="btn" href="#">Free Trial</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.message.contains("action verb")),
            "should penalize CTA without action verb"
        );
    }

    #[test]
    fn test_placeholder_headline_detected() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Your Amazing Product</h1>
                <a class="btn" href="#">Get Started</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.severity == Severity::Error && i.message.contains("placeholder")),
            "should detect placeholder headline"
        );
    }

    #[test]
    fn test_specific_headline_passes() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Write 10x Faster with AI</h1>
                <a class="btn" href="#">Start Writing</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("placeholder")),
            "should not flag specific headline as placeholder"
        );
    }

    #[test]
    fn test_headline_length_sweet_spot() {
        // 30-70 chars — sweet spot, no penalty
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Write Professional Content 10x Faster</h1>
                <a class="btn" href="#">Start Writing</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("short") || i.message.contains("long")),
            "should not penalize headline in sweet spot"
        );
    }

    #[test]
    fn test_headline_too_short() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Hi there</h1>
                <a class="btn" href="#">Start</a>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            result.issues.iter().any(|i| i.message.contains("short")),
            "should flag short headline"
        );
    }

    #[test]
    fn test_benefit_language_detected() {
        let input = make_input(
            r##"<!DOCTYPE html><html><head><title>T</title></head><body>
            <div data-nexus-section="hero">
                <h1>Save 10 Hours Per Week with AI Writing</h1>
                <a class="btn" href="#">Start Saving</a>
            </div>
            <div data-nexus-section="features">
                <p>Save time on every draft. Your content gets better automatically. Boost your productivity and reduce editing time.</p>
            </div></body></html>"##,
        );
        let result = check(&input).unwrap();
        assert!(
            !result
                .issues
                .iter()
                .any(|i| i.message.contains("benefit language")),
            "should not penalize when benefit language present"
        );
    }
}
