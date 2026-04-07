//! Smart Iteration for React Projects — 3-tier edit system.
//!
//! - **Tier 1 (CSS-only, $0, < 200ms):** Token value changes in index.css
//!   Parses CSS, finds the `:root` block, replaces the token value.
//!   No regex — uses structured string search on known token format.
//! - **Tier 2 (Component, $0, < 5s):** Single component file edit via gemma4:e4b
//! - **Tier 3 (Full page, $0.15, < 30s):** Full page regeneration via Sonnet

use crate::react_gen::{ReactProject, ReactProjectFile};
use crate::tokens::TokenSet;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ────────────────────────────���────────────────────────────────────

#[derive(Debug, Error)]
pub enum ReactIterateError {
    #[error("file not found in project: {0}")]
    FileNotFound(String),
    #[error("token not found in CSS: {0}")]
    TokenNotFound(String),
    #[error("model error: {0}")]
    ModelError(String),
}

// ─── Edit Tiers ─────────────────────���───────────────────────────────────────

/// React-specific edit tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactEditTier {
    /// Tier 1: CSS token value change ($0, < 200ms).
    Css { token_name: String, value: String },
    /// Tier 2: Single component edit ($0 via gemma4:e4b, < 5s).
    Component {
        component_path: String,
        edit_prompt: String,
    },
    /// Tier 3: Full page regeneration ($0.15 via Sonnet, < 30s).
    FullPage { page_name: String, prompt: String },
}

// ─── Tier 1: CSS Token Edit ───��─────────────────────────────────────────────

/// Apply a Tier 1 CSS-only edit: update a token value in src/index.css.
///
/// Finds `--{token_name}: {old_value};` in the `:root` block and replaces the value.
/// No regex — uses structured search on the known CSS custom property format.
pub fn apply_css_token_edit(
    project: &mut ReactProject,
    token_name: &str,
    new_value: &str,
    token_set: &mut TokenSet,
) -> Result<Vec<ReactProjectFile>, ReactIterateError> {
    // Update the TokenSet
    let _ = token_set.set_foundation(token_name, new_value);

    // Find and update src/index.css
    let css_file = project
        .files
        .iter_mut()
        .find(|f| f.path == "src/index.css")
        .ok_or_else(|| ReactIterateError::FileNotFound("src/index.css".into()))?;

    // Find the token declaration: `--{token_name}: ...;`
    let search = format!("--{token_name}:");
    if let Some(start) = css_file.content.find(&search) {
        let after_colon = start + search.len();
        if let Some(semi) = css_file.content[after_colon..].find(';') {
            let end = after_colon + semi;
            let new_content = format!(
                "{} {new_value}{}",
                &css_file.content[..after_colon],
                &css_file.content[end..]
            );
            css_file.content = new_content;
            return Ok(vec![css_file.clone()]);
        }
    }

    Err(ReactIterateError::TokenNotFound(token_name.into()))
}

/// Classify an edit request for a React project.
pub fn classify_react_edit(request: &str) -> ReactEditTier {
    let lower = request.to_lowercase();

    // CSS-only patterns
    let css_patterns = [
        "change color",
        "change the color",
        "make it",
        "change primary",
        "change accent",
        "change background",
        "change font",
        "make the background",
        "use a darker",
        "use a lighter",
    ];
    if css_patterns.iter().any(|p| lower.contains(p)) {
        // Try to extract token and value (heuristic)
        return ReactEditTier::Css {
            token_name: "color-primary".into(),
            value: "#3b82f6".into(), // placeholder — real classification would parse this
        };
    }

    // Component patterns
    let component_patterns = [
        "change the hero",
        "update the features",
        "modify the pricing",
        "edit the",
        "in the hero",
        "in the features",
        "in the footer",
        "make the features",
        "add a button to",
    ];
    if component_patterns.iter().any(|p| lower.contains(p)) {
        return ReactEditTier::Component {
            component_path: "src/components/HeroSection.tsx".into(),
            edit_prompt: request.into(),
        };
    }

    // Default: full page
    ReactEditTier::FullPage {
        page_name: "Home".into(),
        prompt: request.into(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::ContentPayload;
    use crate::react_gen::generate_react_project;
    use crate::slot_schema::get_template_schema;
    use crate::variant::{MotionProfile, VariantSelection};
    use std::collections::HashMap;

    fn test_variant() -> VariantSelection {
        VariantSelection {
            palette_id: "saas_midnight".into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn test_project() -> (ReactProject, TokenSet) {
        let schema = get_template_schema("saas_landing").unwrap();
        let payload = ContentPayload {
            template_id: "saas_landing".into(),
            variant: test_variant(),
            sections: vec![],
        };
        let variant = test_variant();
        let ts = variant.to_token_set().unwrap();
        let project =
            generate_react_project(&payload, &schema, &variant, &ts, "Test", None).unwrap();
        (project, ts)
    }

    #[test]
    fn test_css_tier_updates_token_in_index_css() {
        let (mut project, mut ts) = test_project();

        let result = apply_css_token_edit(&mut project, "color-primary", "#ff0000", &mut ts);
        assert!(result.is_ok(), "CSS edit failed: {result:?}");

        let changed = result.unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].path, "src/index.css");
        assert!(
            changed[0].content.contains("#ff0000"),
            "index.css should contain new color"
        );
    }

    #[test]
    fn test_css_tier_no_llm_call() {
        // Tier 1 is purely deterministic — no model client needed.
        // This test proves it works without any provider.
        let (mut project, mut ts) = test_project();
        let result = apply_css_token_edit(&mut project, "color-accent", "#22d3ee", &mut ts);
        assert!(result.is_ok());
    }

    #[test]
    fn test_iterate_preserves_data_nexus_attributes() {
        let (mut project, mut ts) = test_project();

        // Apply a CSS edit
        let _ = apply_css_token_edit(&mut project, "color-primary", "#ff0000", &mut ts);

        // Verify data-nexus-section attributes survive in component files
        for file in &project.files {
            if file.path.starts_with("src/components/") && file.path.ends_with("Section.tsx") {
                assert!(
                    file.content.contains("data-nexus-section"),
                    "Component {} lost data-nexus-section after CSS edit",
                    file.path
                );
            }
        }
    }

    #[test]
    fn test_classify_css_edit() {
        let tier = classify_react_edit("change the color to blue");
        assert!(matches!(tier, ReactEditTier::Css { .. }));
    }

    #[test]
    fn test_classify_component_edit() {
        let tier = classify_react_edit("update the features section with icons");
        assert!(matches!(tier, ReactEditTier::Component { .. }));
    }

    #[test]
    fn test_classify_full_page_edit() {
        let tier = classify_react_edit("completely redesign the website with a new structure");
        assert!(matches!(tier, ReactEditTier::FullPage { .. }));
    }
}
