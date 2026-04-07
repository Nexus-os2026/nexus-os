//! Section Detector — identify page sections from HTML structure.
//!
//! Uses scraper for DOM traversal. Heuristic-based, no LLM.

use scraper::{Html, Selector};

/// A detected page section.
#[derive(Debug, Clone)]
pub struct DetectedSection {
    pub element: String,
    pub suggested_id: String,
    pub content_summary: String,
    pub html_fragment: String,
}

/// Detect sections from sanitized HTML.
pub fn detect_sections(html: &str) -> Vec<DetectedSection> {
    let document = Html::parse_document(html);
    let mut sections = Vec::new();
    let mut section_counter = 0u32;

    // Try semantic elements first
    detect_semantic_sections(&document, &mut sections, &mut section_counter);

    // If no semantic sections found, split by major block elements
    if sections.is_empty() {
        detect_block_sections(&document, &mut sections, &mut section_counter);
    }

    // If still empty, wrap the entire body as one section
    if sections.is_empty() {
        if let Ok(sel) = Selector::parse("body") {
            if let Some(body) = document.select(&sel).next() {
                sections.push(DetectedSection {
                    element: "body".into(),
                    suggested_id: "content".into(),
                    content_summary: summarize_text(&body.text().collect::<String>()),
                    html_fragment: body.inner_html(),
                });
            }
        }

        // If even body is missing, use the whole HTML
        if sections.is_empty() && !html.trim().is_empty() {
            sections.push(DetectedSection {
                element: "div".into(),
                suggested_id: "content".into(),
                content_summary: summarize_text(html),
                html_fragment: html.to_string(),
            });
        }
    }

    sections
}

/// Detect sections from semantic HTML elements.
fn detect_semantic_sections(
    document: &Html,
    sections: &mut Vec<DetectedSection>,
    counter: &mut u32,
) {
    // Header/Nav
    if let Ok(sel) = Selector::parse("header, nav") {
        for el in document.select(&sel) {
            let text: String = el.text().collect();
            if !text.trim().is_empty() {
                sections.push(DetectedSection {
                    element: el.value().name().to_string(),
                    suggested_id: "nav".into(),
                    content_summary: summarize_text(&text),
                    html_fragment: el.html(),
                });
                break; // only first header/nav
            }
        }
    }

    // Sections
    if let Ok(sel) = Selector::parse("section") {
        for el in document.select(&sel) {
            let text: String = el.text().collect();
            let suggested_id = infer_section_type(&text, el, *counter);
            *counter += 1;

            sections.push(DetectedSection {
                element: "section".into(),
                suggested_id,
                content_summary: summarize_text(&text),
                html_fragment: el.html(),
            });
        }
    }

    // Main (if no sections found inside it)
    if sections.len() <= 1 {
        if let Ok(sel) = Selector::parse("main") {
            for el in document.select(&sel) {
                let text: String = el.text().collect();
                if !text.trim().is_empty() {
                    // Check if main contains sections we already found
                    let inner_sections = el
                        .select(
                            &Selector::parse("section")
                                .unwrap_or_else(|_| Selector::parse("*").unwrap()),
                        )
                        .count();
                    if inner_sections == 0 {
                        sections.push(DetectedSection {
                            element: "main".into(),
                            suggested_id: "content".into(),
                            content_summary: summarize_text(&text),
                            html_fragment: el.inner_html(),
                        });
                    }
                }
            }
        }
    }

    // Footer
    if let Ok(sel) = Selector::parse("footer") {
        if let Some(el) = document.select(&sel).next() {
            let text: String = el.text().collect();
            sections.push(DetectedSection {
                element: "footer".into(),
                suggested_id: "footer".into(),
                content_summary: summarize_text(&text),
                html_fragment: el.html(),
            });
        }
    }
}

/// Detect sections from major block elements when no semantic elements exist.
fn detect_block_sections(document: &Html, sections: &mut Vec<DetectedSection>, counter: &mut u32) {
    if let Ok(sel) = Selector::parse("body > div, body > article, body > aside") {
        for el in document.select(&sel) {
            let text: String = el.text().collect();
            if text.trim().len() < 5 {
                continue; // skip near-empty divs
            }
            let suggested_id = format!("section_{}", *counter);
            *counter += 1;

            sections.push(DetectedSection {
                element: el.value().name().to_string(),
                suggested_id,
                content_summary: summarize_text(&text),
                html_fragment: el.html(),
            });
        }
    }
}

/// Infer section type from text content heuristics.
fn infer_section_type(text: &str, el: scraper::ElementRef<'_>, counter: u32) -> String {
    let lower = text.to_lowercase();

    // Check for hero pattern: large heading + CTA-like text
    if let Ok(h1_sel) = Selector::parse("h1") {
        if el.select(&h1_sel).next().is_some() {
            let has_cta = lower.contains("get started")
                || lower.contains("sign up")
                || lower.contains("try ")
                || lower.contains("learn more");
            if has_cta || counter == 0 {
                return "hero".into();
            }
        }
    }

    // Feature patterns
    if lower.contains("feature") || lower.contains("benefit") || lower.contains("why ") {
        return "features".into();
    }

    // Pricing patterns
    if lower.contains("pricing") || lower.contains("plan") || lower.contains("/mo") {
        return "pricing".into();
    }

    // Testimonial patterns
    if lower.contains("testimonial") || lower.contains("review") || lower.contains("said") {
        return "testimonials".into();
    }

    // CTA patterns
    if lower.contains("ready") && (lower.contains("start") || lower.contains("join")) {
        return "cta".into();
    }

    // Contact patterns
    if lower.contains("contact") || lower.contains("get in touch") {
        return "contact".into();
    }

    format!("section_{counter}")
}

/// Summarize text content (first 80 chars).
fn summarize_text(text: &str) -> String {
    let trimmed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.len() <= 80 {
        trimmed
    } else {
        format!("{}...", &trimmed[..77])
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_header_as_nav() {
        let html = "<header><nav><a href='/'>Home</a></nav></header><p>Body</p>";
        let sections = detect_sections(html);
        assert!(sections.iter().any(|s| s.suggested_id == "nav"));
    }

    #[test]
    fn test_detects_footer() {
        let html = "<section><h1>Hi</h1></section><footer><p>Copyright</p></footer>";
        let sections = detect_sections(html);
        assert!(sections.iter().any(|s| s.suggested_id == "footer"));
    }

    #[test]
    fn test_detects_multiple_sections() {
        let html = "<section><h1>One</h1></section><section><h2>Two</h2></section><section><h2>Three</h2></section>";
        let sections = detect_sections(html);
        assert!(
            sections.len() >= 3,
            "should detect 3+ sections, got {}",
            sections.len()
        );
    }

    #[test]
    fn test_detects_hero_pattern() {
        let html = "<section><h1>Welcome to Our Product</h1><p>The best solution</p><a href='#'>Get Started</a></section>";
        let sections = detect_sections(html);
        assert!(
            sections.iter().any(|s| s.suggested_id == "hero"),
            "should detect hero section"
        );
    }

    #[test]
    fn test_handles_flat_html() {
        let html = "<div>Block one content here</div><div>Block two content here</div>";
        let sections = detect_sections(html);
        assert!(
            !sections.is_empty(),
            "should detect at least one section from flat HTML"
        );
    }

    #[test]
    fn test_handles_empty_html() {
        let sections = detect_sections("");
        assert!(sections.is_empty() || sections.len() == 1);
    }

    #[test]
    fn test_section_has_html_fragment() {
        let html = "<section><h2>Features</h2><p>Great stuff</p></section>";
        let sections = detect_sections(html);
        assert!(!sections.is_empty());
        assert!(!sections[0].html_fragment.is_empty());
        assert!(sections[0].html_fragment.contains("Features"));
    }
}
