//! Variant Selection — picks palette, typography, layout, and motion for a template.
//!
//! Currently uses sensible defaults (first palette, "modern" typography, first layout
//! per section, "subtle" motion). A smarter LLM-guided selection can come later.

use crate::self_improve::SystemDefaults;
use crate::variant::{layouts_for_section, palettes_for_template, MotionProfile, VariantSelection};
use std::collections::HashMap;

/// Select a default variant for a given template.
///
/// Checks `SystemDefaults` for learned rankings (from self-improvement) before
/// falling back to hardcoded heuristics. If SystemDefaults is empty, behaviour
/// is identical to the original implementation.
pub fn select_variant(template_id: &str, _brief: &str) -> VariantSelection {
    let defaults = crate::self_improve::load_system_defaults();
    select_variant_with_defaults(template_id, _brief, &defaults)
}

/// Inner function that accepts explicit defaults (testable without disk I/O).
pub fn select_variant_with_defaults(
    template_id: &str,
    _brief: &str,
    defaults: &SystemDefaults,
) -> VariantSelection {
    // Pick palette: prefer learned ranking, fall back to first for template
    let palettes = palettes_for_template(template_id);
    let palette_id = defaults
        .palette_rankings
        .get(template_id)
        .and_then(|ranked| ranked.first().cloned())
        .unwrap_or_else(|| {
            palettes
                .first()
                .map(|p| p.id.to_string())
                .unwrap_or_else(|| "saas_midnight".to_string())
        });

    // Typography: prefer learned ranking, fall back to "modern"
    let typography_id = defaults
        .typography_rankings
        .get(template_id)
        .and_then(|ranked| ranked.first().cloned())
        .unwrap_or_else(|| "modern".to_string());

    // Pick first layout variant for each section that has layout options
    let section_ids: &[&str] = match template_id {
        "saas_landing" => &[
            "hero",
            "features",
            "pricing",
            "testimonials",
            "cta",
            "footer",
        ],
        "docs_site" => &["sidebar_nav", "search", "content", "code_blocks", "footer"],
        "portfolio" => &["hero", "projects", "about", "skills", "contact", "footer"],
        "local_business" => &[
            "hero",
            "services",
            "gallery",
            "testimonials",
            "map",
            "hours",
            "footer",
        ],
        "ecommerce" => &[
            "hero",
            "categories",
            "products",
            "reviews",
            "newsletter",
            "footer",
        ],
        "dashboard" => &[
            "sidebar",
            "header",
            "stats",
            "charts",
            "data_table",
            "footer",
        ],
        _ => &[],
    };

    let mut layout = HashMap::new();
    let layout_overrides = defaults.layout_rankings.get(template_id);
    for &section_id in section_ids {
        // Prefer learned layout ranking if available
        let learned = layout_overrides
            .and_then(|sections| sections.get(section_id))
            .and_then(|ranked| ranked.first().cloned());
        if let Some(lid) = learned {
            layout.insert(section_id.to_string(), lid);
        } else {
            let variants = layouts_for_section(section_id);
            if let Some(first) = variants.first() {
                layout.insert(section_id.to_string(), first.variant_id.to_string());
            }
        }
    }

    VariantSelection {
        palette_id,
        typography_id,
        layout,
        motion: MotionProfile::Subtle,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_variant_returns_valid_selection() {
        let v = select_variant("saas_landing", "AI writing tool");
        assert!(!v.palette_id.is_empty(), "palette_id should be set");
        assert!(!v.typography_id.is_empty(), "typography_id should be set");
        assert!(!v.layout.is_empty(), "layout should have entries");
        assert_eq!(v.motion, MotionProfile::Subtle);

        // Should produce a valid TokenSet
        let token_set = v.to_token_set();
        assert!(
            token_set.is_some(),
            "variant should produce a valid TokenSet"
        );
    }

    #[test]
    fn test_select_variant_all_six_templates() {
        let templates = [
            "saas_landing",
            "docs_site",
            "portfolio",
            "local_business",
            "ecommerce",
            "dashboard",
        ];
        for template_id in &templates {
            let v = select_variant(template_id, "test brief");
            assert!(!v.palette_id.is_empty(), "{template_id}: palette_id empty");
            assert!(
                !v.typography_id.is_empty(),
                "{template_id}: typography_id empty"
            );
            assert_eq!(v.motion, MotionProfile::Subtle);
            assert!(
                v.to_token_set().is_some(),
                "{template_id}: variant should produce valid TokenSet"
            );
        }
    }

    #[test]
    fn test_select_variant_uses_template_specific_palette() {
        let saas = select_variant("saas_landing", "test");
        let docs = select_variant("docs_site", "test");
        // Different templates should get different palette IDs
        assert_ne!(
            saas.palette_id, docs.palette_id,
            "different templates should use different palettes"
        );
    }

    #[test]
    fn test_improvement_affects_variant_selection() {
        // With empty defaults, should use hardcoded first palette
        let empty = SystemDefaults::default();
        let v1 = select_variant_with_defaults("saas_landing", "test", &empty);

        // With learned palette ranking, should use the top-ranked palette
        let mut improved = SystemDefaults::default();
        improved
            .palette_rankings
            .insert("saas_landing".into(), vec!["saas_ocean".into()]);
        improved
            .typography_rankings
            .insert("saas_landing".into(), vec!["editorial".into()]);
        let v2 = select_variant_with_defaults("saas_landing", "test", &improved);

        assert_eq!(v2.palette_id, "saas_ocean");
        assert_eq!(v2.typography_id, "editorial");
        // Original should use hardcoded default
        assert_ne!(v1.palette_id, "saas_ocean");
    }

    #[test]
    fn test_empty_defaults_same_as_original() {
        let empty = SystemDefaults::default();
        let v = select_variant_with_defaults("saas_landing", "test", &empty);
        // Should match the hardcoded default (first palette for saas_landing)
        let palettes = palettes_for_template("saas_landing");
        assert_eq!(v.palette_id, palettes.first().unwrap().id);
        assert_eq!(v.typography_id, "modern");
    }
}
