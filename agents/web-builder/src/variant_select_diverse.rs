//! Diverse Variant Selection — picks maximally different VariantSelections.
//!
//! Ensures no two variants share the same palette or typography preset,
//! varies hero/primary section layouts, and mixes motion profiles.

use crate::variant::{
    all_typography_presets, layouts_for_section, palettes_for_template, MotionProfile,
    VariantSelection,
};

/// Human-readable label for a variant, e.g. "Midnight Tech".
pub fn variant_label(palette_name: &str, typography_name: &str) -> String {
    format!("{} {}", palette_name, typography_name)
}

/// Select `count` maximally diverse VariantSelections for a template.
///
/// Rules:
/// - No two variants share the same palette (when enough palettes exist)
/// - No two variants share the same typography preset (when enough exist)
/// - At least one variant has a different hero/primary section layout
/// - Motion profiles vary (not all identical)
/// - Deterministic given a seed (for reproducible tests)
pub fn select_diverse_variants(
    template_id: &str,
    base_variant: &VariantSelection,
    count: usize,
) -> Vec<VariantSelection> {
    select_diverse_variants_seeded(template_id, base_variant, count, 42)
}

/// Seeded version for deterministic test output.
pub fn select_diverse_variants_seeded(
    template_id: &str,
    base_variant: &VariantSelection,
    count: usize,
    seed: u64,
) -> Vec<VariantSelection> {
    let palettes = palettes_for_template(template_id);
    let typography = all_typography_presets();

    if count == 0 || palettes.is_empty() || typography.is_empty() {
        return vec![];
    }

    // Collect palette IDs excluding the base
    let mut other_palette_ids: Vec<&str> = palettes
        .iter()
        .map(|p| p.id)
        .filter(|id| *id != base_variant.palette_id)
        .collect();
    // Simple deterministic shuffle based on seed
    deterministic_shuffle(&mut other_palette_ids, seed);

    // Collect typography IDs excluding the base
    let mut other_typo_ids: Vec<&str> = typography
        .iter()
        .map(|t| t.id)
        .filter(|id| *id != base_variant.typography_id)
        .collect();
    deterministic_shuffle(&mut other_typo_ids, seed.wrapping_add(1));

    // Motion profile rotation: Subtle, Expressive, None (skip base's profile first)
    let motion_pool = [
        MotionProfile::Subtle,
        MotionProfile::Expressive,
        MotionProfile::None,
    ];
    let mut motion_cycle: Vec<MotionProfile> = motion_pool
        .iter()
        .copied()
        .filter(|m| *m != base_variant.motion)
        .collect();
    // Add base motion back at the end for overflow
    motion_cycle.push(base_variant.motion);

    // Find the primary section for this template (hero or first section with layouts)
    let primary_section = primary_section_for_template(template_id);

    let mut results = Vec::with_capacity(count);

    for i in 0..count {
        // Pick palette: cycle through other palettes, wrap if needed
        let palette_id = if !other_palette_ids.is_empty() {
            other_palette_ids[i % other_palette_ids.len()].to_string()
        } else {
            base_variant.palette_id.clone()
        };

        // Pick typography: cycle through other typography, wrap if needed
        let typography_id = if !other_typo_ids.is_empty() {
            other_typo_ids[i % other_typo_ids.len()].to_string()
        } else {
            base_variant.typography_id.clone()
        };

        // Pick motion: cycle through alternatives
        let motion = motion_cycle[i % motion_cycle.len()];

        // Build layout map: start with base, then vary primary section for some variants
        let mut layout = base_variant.layout.clone();

        if let Some(section_id) = primary_section {
            let section_layouts = layouts_for_section(section_id);
            if section_layouts.len() > 1 {
                // Pick a different layout for variants after the first
                let layout_idx = (i + 1) % section_layouts.len();
                layout.insert(
                    section_id.to_string(),
                    section_layouts[layout_idx].variant_id.to_string(),
                );
            }
        }

        results.push(VariantSelection {
            palette_id,
            typography_id,
            layout,
            motion,
        });
    }

    results
}

/// Get the primary section ID that should have layout variation.
fn primary_section_for_template(template_id: &str) -> Option<&'static str> {
    match template_id {
        "saas_landing" => Some("hero"),
        "portfolio" => Some("projects"),
        "local_business" => Some("hero"),
        "ecommerce" => Some("products"),
        "docs_site" => Some("content"),
        "dashboard" => Some("stats"),
        _ => None,
    }
}

/// Simple deterministic shuffle using a seed (Fisher-Yates with LCG).
fn deterministic_shuffle<T>(items: &mut [T], seed: u64) {
    let len = items.len();
    if len <= 1 {
        return;
    }
    let mut state = seed;
    for i in (1..len).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 33) as usize % (i + 1);
        items.swap(i, j);
    }
}

/// Look up palette name by ID.
pub fn palette_name_for_id(palette_id: &str) -> &'static str {
    crate::variant::all_palette_presets()
        .iter()
        .find(|p| p.id == palette_id)
        .map(|p| p.name)
        .unwrap_or("Custom")
}

/// Look up typography name by ID.
pub fn typography_name_for_id(typography_id: &str) -> &'static str {
    crate::variant::all_typography_presets()
        .iter()
        .find(|t| t.id == typography_id)
        .map(|t| t.name)
        .unwrap_or("Custom")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base_variant(palette: &str) -> VariantSelection {
        VariantSelection {
            palette_id: palette.into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    #[test]
    fn test_diversity_no_duplicate_palettes() {
        let base = base_variant("saas_midnight");
        let variants = select_diverse_variants("saas_landing", &base, 3);
        assert_eq!(variants.len(), 3);
        let palette_ids: Vec<&str> = variants.iter().map(|v| v.palette_id.as_str()).collect();
        // All 3 should be unique (saas_landing has 4 palettes, base excluded = 3 available)
        let mut unique = palette_ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            3,
            "palettes should be unique: {palette_ids:?}"
        );
    }

    #[test]
    fn test_diversity_no_duplicate_typography() {
        let base = base_variant("saas_midnight");
        let variants = select_diverse_variants("saas_landing", &base, 3);
        let typo_ids: Vec<&str> = variants.iter().map(|v| v.typography_id.as_str()).collect();
        let mut unique = typo_ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 3, "typography should be unique: {typo_ids:?}");
    }

    #[test]
    fn test_diversity_hero_layout_varies() {
        let mut base = base_variant("saas_midnight");
        base.layout.insert("hero".into(), "centered".into());
        let variants = select_diverse_variants("saas_landing", &base, 3);
        let hero_layouts: Vec<Option<&String>> =
            variants.iter().map(|v| v.layout.get("hero")).collect();
        // At least one should differ from the base
        assert!(
            hero_layouts
                .iter()
                .any(|l| l.map(|s| s.as_str()) != Some("centered")),
            "at least one variant should have a different hero layout: {hero_layouts:?}"
        );
    }

    #[test]
    fn test_diversity_motion_varies() {
        let base = base_variant("saas_midnight");
        let variants = select_diverse_variants("saas_landing", &base, 3);
        let motions: Vec<MotionProfile> = variants.iter().map(|v| v.motion).collect();
        // Not all identical
        assert!(
            motions.windows(2).any(|w| w[0] != w[1]),
            "motion profiles should vary: {motions:?}"
        );
    }

    #[test]
    fn test_diversity_with_limited_options() {
        // docs_site also has 4 palettes, but test with a base that's already one of them
        let base = base_variant("docs_clean");
        let variants = select_diverse_variants("docs_site", &base, 3);
        assert_eq!(variants.len(), 3);
        // Should still produce valid variants
        for v in &variants {
            assert!(!v.palette_id.is_empty());
            assert!(!v.typography_id.is_empty());
        }
    }

    #[test]
    fn test_diversity_deterministic_given_seed() {
        let base = base_variant("saas_midnight");
        let v1 = select_diverse_variants_seeded("saas_landing", &base, 3, 99);
        let v2 = select_diverse_variants_seeded("saas_landing", &base, 3, 99);
        for (a, b) in v1.iter().zip(v2.iter()) {
            assert_eq!(a.palette_id, b.palette_id);
            assert_eq!(a.typography_id, b.typography_id);
            assert_eq!(a.motion, b.motion);
        }
    }

    #[test]
    fn test_variant_label_human_readable() {
        let label = variant_label("Midnight", "Tech");
        assert_eq!(label, "Midnight Tech");
        let label2 = variant_label("Ocean", "Editorial");
        assert_eq!(label2, "Ocean Editorial");
    }

    #[test]
    fn test_diversity_all_six_templates() {
        let template_palettes = [
            ("saas_landing", "saas_midnight"),
            ("docs_site", "docs_clean"),
            ("portfolio", "port_monochrome"),
            ("local_business", "biz_warm"),
            ("ecommerce", "ecom_luxe"),
            ("dashboard", "dash_pro"),
        ];
        for (template_id, palette_id) in &template_palettes {
            let base = base_variant(palette_id);
            let variants = select_diverse_variants(template_id, &base, 3);
            assert_eq!(
                variants.len(),
                3,
                "{template_id}: should produce 3 variants"
            );
            for v in &variants {
                assert!(
                    v.to_token_set().is_some(),
                    "{template_id}: variant should produce valid TokenSet"
                );
            }
        }
    }
}
