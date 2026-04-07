//! Variant Generator — produces N diverse, fully-assembled variant previews.
//!
//! Three variant levels:
//! 1. **Token variants** (instant): swap palette + typography + motion → different CSS
//! 2. **Section layout variants** (instant): swap layout variant for a section
//! 3. **Content variants** (< 15s): regenerate copy via gemma4:e4b with tone hints
//!
//! All variant generation is $0 (local model or purely algorithmic).

use crate::assembler;
use crate::content_gen;
use crate::content_payload::ContentPayload;
use crate::slot_schema::{get_template_schema, TemplateSchema};
use crate::templates;
use crate::tokens::TokenSet;
use crate::variant::VariantSelection;
use crate::variant_select_diverse::{
    palette_name_for_id, select_diverse_variants, typography_name_for_id, variant_label,
};
use nexus_connectors_llm::providers::LlmProvider;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VariantGenError {
    #[error("template not found: {0}")]
    TemplateNotFound(String),
    #[error("schema not found for template: {0}")]
    SchemaNotFound(String),
    #[error("assembly failed: {0}")]
    AssemblyFailed(String),
    #[error("content generation failed: {0}")]
    ContentGenFailed(String),
    #[error("no base content for variant generation")]
    NoBaseContent,
    #[error("invalid section: {0}")]
    InvalidSection(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// A set of generated variants ready for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantSet {
    pub variants: Vec<GeneratedVariant>,
    pub timestamp: String,
}

/// A single fully-assembled variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedVariant {
    pub id: String,
    pub label: String,
    pub variant_selection: VariantSelection,
    pub content_payload: ContentPayload,
    pub assembled_html: String,
    pub token_set: TokenSet,
}

/// What kind of section-level variant to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionVariantType {
    Layout,
    Content,
    Palette,
}

// ─── Tone Hints ─────────────────────────────────────────────────────────────

/// Map typography preset to a copy tone hint for content variant generation.
fn variant_tone_hint(variant: &VariantSelection) -> &'static str {
    match variant.typography_id.as_str() {
        "tech" => "Write in a precise, confident, professional tone.",
        "editorial" => "Write in a warm, literary, storytelling tone.",
        "modern" => "Write in a clean, friendly, approachable tone.",
        "clean" => "Write in a minimal, direct, no-nonsense tone.",
        _ => "",
    }
}

// ─── Variant ID/Label Helpers ─────────────────────────────────��─────────────

const VARIANT_LETTERS: &[&str] = &["a", "b", "c", "d", "e", "f"];

fn variant_id(index: usize) -> String {
    format!("variant_{}", VARIANT_LETTERS.get(index).unwrap_or(&"x"))
}

// ─── Full-Page Variant Generation ───────────────────────────────────────────

/// Generate N full-page variants with diverse visual and content differentiation.
///
/// Each variant gets:
/// 1. A different VariantSelection (palette + typography + layout + motion)
/// 2. Its own TokenSet from the VariantSelection
/// 3. Optionally different content (if provider is Some and content variants desired)
/// 4. Fully assembled HTML ready for iframe preview
pub fn generate_page_variants(
    brief: &str,
    template_id: &str,
    base_variant: &VariantSelection,
    base_content: &ContentPayload,
    count: usize,
    provider: Option<&dyn LlmProvider>,
) -> Result<VariantSet, VariantGenError> {
    let count = count.min(6); // Cap at 6

    let schema = get_template_schema(template_id)
        .ok_or_else(|| VariantGenError::SchemaNotFound(template_id.to_string()))?;

    let template = templates::get_template(template_id)
        .ok_or_else(|| VariantGenError::TemplateNotFound(template_id.to_string()))?;

    // Step 1: Get diverse variant selections
    let selections = select_diverse_variants(template_id, base_variant, count);

    // Step 2: For each selection, build token set + content + assemble
    let mut variants = Vec::with_capacity(count);

    for (i, selection) in selections.iter().enumerate() {
        let token_set = selection.to_token_set().unwrap_or_default();

        // Content: try LLM-based content variant if provider available, else reuse base
        let content = if let Some(prov) = provider {
            let tone = variant_tone_hint(selection);
            let augmented_brief = if tone.is_empty() {
                brief.to_string()
            } else {
                format!("{brief}\n\nTONE: {tone}")
            };
            match content_gen::generate_content(
                &augmented_brief,
                template_id,
                &schema,
                selection,
                prov,
            ) {
                Ok(payload) => payload,
                Err(_) => {
                    // Fallback to base content with updated variant
                    ContentPayload {
                        template_id: template_id.to_string(),
                        variant: selection.clone(),
                        sections: base_content.sections.clone(),
                    }
                }
            }
        } else {
            // Token-only variant: reuse base content
            ContentPayload {
                template_id: template_id.to_string(),
                variant: selection.clone(),
                sections: base_content.sections.clone(),
            }
        };

        // Assemble HTML
        let assembled_html = assembler::assemble(&content, template.html, &token_set, &schema)
            .map_err(|e| VariantGenError::AssemblyFailed(e.to_string()))?;

        let palette_name = palette_name_for_id(&selection.palette_id);
        let typo_name = typography_name_for_id(&selection.typography_id);

        variants.push(GeneratedVariant {
            id: variant_id(i),
            label: variant_label(palette_name, typo_name),
            variant_selection: selection.clone(),
            content_payload: content,
            assembled_html,
            token_set,
        });
    }

    Ok(VariantSet {
        variants,
        timestamp: chrono_timestamp(),
    })
}

/// Generate N section-level variants.
#[allow(clippy::too_many_arguments)]
pub fn generate_section_variants(
    section_id: &str,
    brief: &str,
    template_id: &str,
    base_variant: &VariantSelection,
    base_content: &ContentPayload,
    variant_type: SectionVariantType,
    count: usize,
    provider: Option<&dyn LlmProvider>,
) -> Result<VariantSet, VariantGenError> {
    let count = count.min(6);

    let schema = get_template_schema(template_id)
        .ok_or_else(|| VariantGenError::SchemaNotFound(template_id.to_string()))?;

    let template = templates::get_template(template_id)
        .ok_or_else(|| VariantGenError::TemplateNotFound(template_id.to_string()))?;

    // Verify section exists in schema
    if !schema.sections.iter().any(|s| s.section_id == section_id) {
        return Err(VariantGenError::InvalidSection(section_id.to_string()));
    }

    match variant_type {
        SectionVariantType::Layout => generate_layout_variants(
            section_id,
            template_id,
            base_variant,
            base_content,
            count,
            &schema,
            template.html,
        ),
        SectionVariantType::Palette => generate_palette_variants(
            template_id,
            base_variant,
            base_content,
            count,
            &schema,
            template.html,
        ),
        SectionVariantType::Content => generate_content_variants(
            section_id,
            brief,
            template_id,
            base_variant,
            base_content,
            count,
            &schema,
            template.html,
            provider,
        ),
    }
}

/// Layout variants: swap layout variant_id for a specific section.
fn generate_layout_variants(
    section_id: &str,
    template_id: &str,
    base_variant: &VariantSelection,
    base_content: &ContentPayload,
    count: usize,
    schema: &TemplateSchema,
    template_html: &str,
) -> Result<VariantSet, VariantGenError> {
    let section_layouts = crate::variant::layouts_for_section(section_id);
    let mut variants = Vec::with_capacity(count);

    for (i, layout_variant) in section_layouts.iter().take(count).enumerate() {
        let mut selection = base_variant.clone();
        selection.layout.insert(
            section_id.to_string(),
            layout_variant.variant_id.to_string(),
        );

        let token_set = selection.to_token_set().unwrap_or_default();
        let content = ContentPayload {
            template_id: template_id.to_string(),
            variant: selection.clone(),
            sections: base_content.sections.clone(),
        };

        let assembled_html = assembler::assemble(&content, template_html, &token_set, schema)
            .map_err(|e| VariantGenError::AssemblyFailed(e.to_string()))?;

        variants.push(GeneratedVariant {
            id: variant_id(i),
            label: format!("{} — {}", section_id, layout_variant.name),
            variant_selection: selection,
            content_payload: content,
            assembled_html,
            token_set,
        });
    }

    Ok(VariantSet {
        variants,
        timestamp: chrono_timestamp(),
    })
}

/// Palette variants: swap palette for the whole page (token-only, instant).
fn generate_palette_variants(
    template_id: &str,
    base_variant: &VariantSelection,
    base_content: &ContentPayload,
    count: usize,
    schema: &TemplateSchema,
    template_html: &str,
) -> Result<VariantSet, VariantGenError> {
    let selections = select_diverse_variants(template_id, base_variant, count);
    let mut variants = Vec::with_capacity(count);

    for (i, selection) in selections.iter().enumerate() {
        let token_set = selection.to_token_set().unwrap_or_default();
        let content = ContentPayload {
            template_id: template_id.to_string(),
            variant: selection.clone(),
            sections: base_content.sections.clone(),
        };

        let assembled_html = assembler::assemble(&content, template_html, &token_set, schema)
            .map_err(|e| VariantGenError::AssemblyFailed(e.to_string()))?;

        let palette_name = palette_name_for_id(&selection.palette_id);
        let typo_name = typography_name_for_id(&selection.typography_id);

        variants.push(GeneratedVariant {
            id: variant_id(i),
            label: variant_label(palette_name, typo_name),
            variant_selection: selection.clone(),
            content_payload: content,
            assembled_html,
            token_set,
        });
    }

    Ok(VariantSet {
        variants,
        timestamp: chrono_timestamp(),
    })
}

/// Content variants: regenerate copy for a section via gemma4:e4b.
#[allow(clippy::too_many_arguments)]
fn generate_content_variants(
    _section_id: &str,
    brief: &str,
    template_id: &str,
    base_variant: &VariantSelection,
    base_content: &ContentPayload,
    count: usize,
    schema: &TemplateSchema,
    template_html: &str,
    provider: Option<&dyn LlmProvider>,
) -> Result<VariantSet, VariantGenError> {
    let token_set = base_variant.to_token_set().unwrap_or_default();
    let mut variants = Vec::with_capacity(count);

    // Different content angles
    let angles = [
        "Focus on benefits and outcomes. Emphasize what the user gains.",
        "Focus on trust and credibility. Emphasize reliability and social proof.",
        "Focus on urgency and action. Emphasize why to act now.",
        "Focus on simplicity and ease. Emphasize how easy it is to get started.",
    ];

    for i in 0..count {
        let content = if let Some(prov) = provider {
            let angle = angles.get(i).unwrap_or(&angles[0]);
            let augmented_brief = format!("{brief}\n\nCONTENT ANGLE: {angle}");
            match content_gen::generate_content(
                &augmented_brief,
                template_id,
                schema,
                base_variant,
                prov,
            ) {
                Ok(payload) => payload,
                Err(_) => ContentPayload {
                    template_id: template_id.to_string(),
                    variant: base_variant.clone(),
                    sections: base_content.sections.clone(),
                },
            }
        } else {
            // No provider — just reuse base content
            ContentPayload {
                template_id: template_id.to_string(),
                variant: base_variant.clone(),
                sections: base_content.sections.clone(),
            }
        };

        let assembled_html = assembler::assemble(&content, template_html, &token_set, schema)
            .map_err(|e| VariantGenError::AssemblyFailed(e.to_string()))?;

        let angle_label = match i {
            0 => "Benefits",
            1 => "Trust",
            2 => "Urgency",
            _ => "Simplicity",
        };

        variants.push(GeneratedVariant {
            id: variant_id(i),
            label: format!("Copy — {}", angle_label),
            variant_selection: base_variant.clone(),
            content_payload: content,
            assembled_html,
            token_set: token_set.clone(),
        });
    }

    Ok(VariantSet {
        variants,
        timestamp: chrono_timestamp(),
    })
}

/// Simple ISO-8601 timestamp without pulling in chrono.
fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", now.as_secs())
}

// ─── Serializable Payload for Tauri ─────────────────────────────────────────

/// Frontend-friendly payload sent over the Tauri command bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantSetPayload {
    pub variants: Vec<VariantPayload>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantPayload {
    pub id: String,
    pub label: String,
    pub palette_id: String,
    pub typography_id: String,
    pub assembled_html: String,
}

impl From<VariantSet> for VariantSetPayload {
    fn from(vs: VariantSet) -> Self {
        VariantSetPayload {
            variants: vs
                .variants
                .into_iter()
                .map(|v| VariantPayload {
                    id: v.id,
                    label: v.label,
                    palette_id: v.variant_selection.palette_id,
                    typography_id: v.variant_selection.typography_id,
                    assembled_html: v.assembled_html,
                })
                .collect(),
            timestamp: vs.timestamp,
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::SectionContent;
    use crate::variant::MotionProfile;
    use std::collections::HashMap;

    fn default_variant(palette: &str) -> VariantSelection {
        VariantSelection {
            palette_id: palette.into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn mock_saas_content() -> ContentPayload {
        ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant("saas_midnight"),
            sections: vec![
                SectionContent {
                    section_id: "hero".into(),
                    slots: HashMap::from([
                        ("badge".into(), "New in v2".into()),
                        ("headline".into(), "Build Faster with AI Power".into()),
                        (
                            "subtitle".into(),
                            "The platform that helps developers ship 10x faster.".into(),
                        ),
                        ("cta_primary".into(), "Start Free Trial".into()),
                        ("cta_secondary".into(), "Watch Demo".into()),
                        (
                            "media".into(),
                            "Product screenshot showing dashboard".into(),
                        ),
                    ]),
                },
                SectionContent {
                    section_id: "features".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Powerful Features".into()),
                        ("subheading".into(), "Everything you need".into()),
                        ("feature_1_icon".into(), "rocket".into()),
                        ("feature_1_title".into(), "Lightning Fast".into()),
                        ("feature_1_desc".into(), "Deploy in seconds.".into()),
                        ("feature_2_icon".into(), "shield".into()),
                        ("feature_2_title".into(), "Secure".into()),
                        ("feature_2_desc".into(), "Bank-grade encryption.".into()),
                        ("feature_3_icon".into(), "chart".into()),
                        ("feature_3_title".into(), "Analytics".into()),
                        ("feature_3_desc".into(), "Real-time dashboards.".into()),
                    ]),
                },
                SectionContent {
                    section_id: "pricing".into(),
                    slots: HashMap::from([
                        ("heading".into(), "Simple Pricing".into()),
                        ("tier_1_name".into(), "Starter".into()),
                        ("tier_1_price".into(), "$9/mo".into()),
                        ("tier_1_features".into(), "5 projects<br>1GB storage".into()),
                        ("tier_2_name".into(), "Pro".into()),
                        ("tier_2_price".into(), "$29/mo".into()),
                        ("tier_2_features".into(), "Unlimited projects".into()),
                        ("tier_2_badge".into(), "Popular".into()),
                        ("tier_3_name".into(), "Enterprise".into()),
                        ("tier_3_price".into(), "$99/mo".into()),
                        ("tier_3_features".into(), "Everything in Pro<br>SSO".into()),
                    ]),
                },
                SectionContent {
                    section_id: "testimonials".into(),
                    slots: HashMap::from([
                        ("heading".into(), "What Users Say".into()),
                        ("testimonial_1_quote".into(), "Saved us hours.".into()),
                        ("testimonial_1_author".into(), "Jane Smith".into()),
                        ("testimonial_1_role".into(), "CTO at TechCorp".into()),
                        ("testimonial_2_quote".into(), "Best tool ever.".into()),
                        ("testimonial_2_author".into(), "Alex Johnson".into()),
                        ("testimonial_2_role".into(), "Lead Engineer".into()),
                        ("testimonial_3_quote".into(), "Incredible.".into()),
                        ("testimonial_3_author".into(), "Maria Garcia".into()),
                        ("testimonial_3_role".into(), "VP Engineering".into()),
                    ]),
                },
                SectionContent {
                    section_id: "cta".into(),
                    slots: HashMap::from([
                        ("headline".into(), "Ready to Ship Faster?".into()),
                        ("body".into(), "Join thousands of developers.".into()),
                        ("cta_button".into(), "Get Started Free".into()),
                    ]),
                },
                SectionContent {
                    section_id: "footer".into(),
                    slots: HashMap::from([
                        ("brand".into(), "AcmeAI".into()),
                        ("copyright".into(), "2026 AcmeAI Inc.".into()),
                    ]),
                },
            ],
        }
    }

    #[test]
    fn test_generate_page_variants_returns_three() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let result =
            generate_page_variants("AI writing tool", "saas_landing", &base, &content, 3, None);
        assert!(result.is_ok(), "generate_page_variants failed: {result:?}");
        let vs = result.unwrap();
        assert_eq!(vs.variants.len(), 3);
    }

    #[test]
    fn test_generate_page_variants_all_different_palettes() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();
        let palettes: Vec<&str> = vs
            .variants
            .iter()
            .map(|v| v.variant_selection.palette_id.as_str())
            .collect();
        let mut unique = palettes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 3, "palettes should be unique: {palettes:?}");
    }

    #[test]
    fn test_generate_page_variants_all_different_typography() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();
        let typos: Vec<&str> = vs
            .variants
            .iter()
            .map(|v| v.variant_selection.typography_id.as_str())
            .collect();
        let mut unique = typos.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 3, "typography should be unique: {typos:?}");
    }

    #[test]
    fn test_generate_page_variants_assembled_html_valid() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();
        for v in &vs.variants {
            assert!(
                !v.assembled_html.is_empty(),
                "HTML should not be empty for {}",
                v.id
            );
            // No remaining {{PLACEHOLDER}} patterns — all should be resolved
            let remaining: Vec<&str> = v
                .assembled_html
                .match_indices("{{")
                .filter_map(|(i, _)| {
                    let rest = &v.assembled_html[i + 2..];
                    rest.find("}}").map(|end| &rest[..end])
                })
                .collect();
            assert!(
                remaining.is_empty(),
                "variant {} has unfilled placeholders: {:?}",
                v.id,
                remaining
            );
        }
    }

    #[test]
    fn test_generate_page_variants_all_six_templates() {
        let template_data = [
            ("saas_landing", "saas_midnight"),
            ("docs_site", "docs_clean"),
            ("portfolio", "port_monochrome"),
            ("local_business", "biz_warm"),
            ("ecommerce", "ecom_luxe"),
            ("dashboard", "dash_pro"),
        ];
        for (template_id, palette_id) in &template_data {
            let base = default_variant(palette_id);
            // Use empty content — templates should handle missing slots gracefully
            let content = ContentPayload {
                template_id: template_id.to_string(),
                variant: base.clone(),
                sections: vec![],
            };
            let result = generate_page_variants("test", template_id, &base, &content, 3, None);
            assert!(
                result.is_ok(),
                "{template_id} variant generation failed: {result:?}"
            );
            let vs = result.unwrap();
            assert_eq!(
                vs.variants.len(),
                3,
                "{template_id} should produce 3 variants"
            );
        }
    }

    #[test]
    fn test_generate_section_variants_layout() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let result = generate_section_variants(
            "hero",
            "test",
            "saas_landing",
            &base,
            &content,
            SectionVariantType::Layout,
            3,
            None,
        );
        assert!(result.is_ok(), "section layout variants failed: {result:?}");
        let vs = result.unwrap();
        assert!(
            !vs.variants.is_empty(),
            "should produce at least one layout variant"
        );
        // hero has 3 layouts: centered, split_image, video_bg
        assert!(vs.variants.len() <= 3);
    }

    #[test]
    fn test_generate_section_variants_palette() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let result = generate_section_variants(
            "hero",
            "test",
            "saas_landing",
            &base,
            &content,
            SectionVariantType::Palette,
            3,
            None,
        );
        assert!(
            result.is_ok(),
            "section palette variants failed: {result:?}"
        );
        let vs = result.unwrap();
        assert_eq!(vs.variants.len(), 3);
        // Different CSS token values
        let css_values: Vec<String> = vs.variants.iter().map(|v| v.token_set.to_css()).collect();
        assert!(
            css_values.windows(2).any(|w| w[0] != w[1]),
            "palette variants should produce different CSS"
        );
    }

    #[test]
    fn test_variant_labels_human_readable() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();
        for v in &vs.variants {
            // Labels should be like "Ocean Tech", not "palette_1_typo_2"
            assert!(
                !v.label.contains("palette_"),
                "label should be human-readable: {}",
                v.label
            );
            assert!(
                !v.label.contains("typo_"),
                "label should be human-readable: {}",
                v.label
            );
            assert!(
                v.label.contains(' '),
                "label should have space (two words): {}",
                v.label
            );
        }
    }

    #[test]
    fn test_variant_generation_and_selection_roundtrip() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();

        // Simulate user selecting variant_b
        let selected = vs.variants.iter().find(|v| v.id == "variant_b").unwrap();

        // Verify selected variant can produce a valid TokenSet
        assert!(selected.variant_selection.to_token_set().is_some());
        assert!(!selected.assembled_html.is_empty());
        assert_ne!(selected.variant_selection.palette_id, base.palette_id);
    }

    #[test]
    fn test_section_variant_preserves_other_sections() {
        let mut base = default_variant("saas_midnight");
        base.layout.insert("features".into(), "card_grid".into());
        base.layout.insert("hero".into(), "centered".into());
        let content = mock_saas_content();

        let vs = generate_section_variants(
            "hero",
            "test",
            "saas_landing",
            &base,
            &content,
            SectionVariantType::Layout,
            3,
            None,
        )
        .unwrap();

        for v in &vs.variants {
            // features layout should be preserved from base
            assert_eq!(
                v.variant_selection
                    .layout
                    .get("features")
                    .map(|s| s.as_str()),
                Some("card_grid"),
                "features layout should be preserved"
            );
        }
    }

    #[test]
    fn test_variant_set_payload_conversion() {
        let base = default_variant("saas_midnight");
        let content = mock_saas_content();
        let vs = generate_page_variants("test", "saas_landing", &base, &content, 3, None).unwrap();
        let payload: VariantSetPayload = vs.into();
        assert_eq!(payload.variants.len(), 3);
        for vp in &payload.variants {
            assert!(!vp.id.is_empty());
            assert!(!vp.label.is_empty());
            assert!(!vp.assembled_html.is_empty());
        }
    }
}
