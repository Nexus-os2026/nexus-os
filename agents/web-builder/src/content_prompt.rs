//! Content Prompt — builds the prompt sent to gemma4:e4b for slot content generation.
//!
//! Given a brief, template schema, and variant selection, produces a deterministic
//! prompt that instructs the LLM to fill every slot with contextually relevant content.

use crate::slot_schema::{SlotType, TemplateSchema};
use crate::variant::VariantSelection;
use std::fmt::Write;

/// Template type descriptions for prompt context.
fn template_description(template_id: &str) -> &'static str {
    match template_id {
        "saas_landing" => "a SaaS product landing page with hero, features, pricing tiers, testimonials, and CTA",
        "docs_site" => "a documentation site with sidebar navigation, search, content sections, and code examples",
        "portfolio" => "a personal portfolio with hero intro, project showcase, about section, skills, and contact",
        "local_business" => "a local business website with hero, services, gallery, testimonials, map, and hours",
        "ecommerce" => "an e-commerce store with hero, product categories, product grid, reviews, and newsletter",
        "dashboard" => "an admin dashboard with sidebar nav, stat cards, charts, data table, and footer",
        _ => "a website",
    }
}

/// Build the content generation prompt for gemma4:e4b.
///
/// Deterministic: same inputs always produce the same prompt.
/// Appends any learned content hints from SystemDefaults (Phase 16).
pub fn build_content_prompt(
    brief: &str,
    template_id: &str,
    schema: &TemplateSchema,
    variant: &VariantSelection,
) -> String {
    let defaults = crate::self_improve::load_system_defaults();
    build_content_prompt_with_defaults(brief, template_id, schema, variant, &defaults)
}

/// Inner function that accepts explicit defaults (testable without disk I/O).
pub fn build_content_prompt_with_defaults(
    brief: &str,
    template_id: &str,
    schema: &TemplateSchema,
    variant: &VariantSelection,
    defaults: &crate::self_improve::SystemDefaults,
) -> String {
    let mut prompt = String::with_capacity(4096);

    // System instruction
    let _ = write!(
        prompt,
        "You are a professional copywriter. Generate website content for the following brief. \
         Respond ONLY with a JSON object, no markdown, no explanation.\n\n"
    );

    // Brief
    let _ = write!(prompt, "BRIEF: {brief}\n\n");

    // Template context
    let desc = template_description(template_id);
    let _ = write!(prompt, "TEMPLATE: This is {desc}.\n\n");

    // Variant context
    let _ = write!(
        prompt,
        "STYLE: palette=\"{}\", typography=\"{}\", motion=\"{:?}\". Match tone to design.\n\n",
        variant.palette_id, variant.typography_id, variant.motion
    );

    // Section slot specifications
    let _ = writeln!(prompt, "SECTIONS:");
    for section in &schema.sections {
        let _ = write!(prompt, "\nSection \"{}\":\n", section.section_id);
        for (slot_name, constraint) in &section.slots {
            let type_label = match constraint.slot_type {
                SlotType::Text => "text",
                SlotType::Cta => "cta",
                SlotType::Url => "url",
                SlotType::ImagePrompt => "image_prompt",
                SlotType::VideoEmbed => "video_url",
                SlotType::RichText => "rich_text",
                SlotType::Number => "number",
                SlotType::IconKey => "icon_key",
            };
            let req = if constraint.required {
                "required"
            } else {
                "optional"
            };
            let mut spec = format!("  - \"{slot_name}\": {type_label}, {req}");
            if let Some(max) = constraint.max_chars {
                let _ = write!(spec, ", max {max} chars");
            }
            if let Some(min) = constraint.min_chars {
                let _ = write!(spec, ", min {min} chars");
            }
            if let Some(hint) = &constraint.validation_hint {
                let _ = write!(spec, ", {hint}");
            }
            // CTA-specific guidance
            if constraint.slot_type == SlotType::Cta {
                spec.push_str(", must start with action verb");
            }
            let _ = writeln!(prompt, "{spec}");
        }
    }

    // Expected JSON format
    let _ = write!(
        prompt,
        "\nRESPONSE FORMAT (JSON only):\n{{\n  \"sections\": [\n"
    );
    for (i, section) in schema.sections.iter().enumerate() {
        let _ = write!(
            prompt,
            "    {{\n      \"section_id\": \"{}\",\n      \"slots\": {{",
            section.section_id
        );
        let slot_names: Vec<&str> = section.slots.keys().map(|s| s.as_str()).collect();
        for (j, name) in slot_names.iter().enumerate() {
            if j > 0 {
                let _ = write!(prompt, ",");
            }
            let _ = write!(prompt, " \"{name}\": \"...\"");
        }
        let _ = write!(prompt, " }}");
        let _ = write!(prompt, "\n    }}");
        if i < schema.sections.len() - 1 {
            let _ = write!(prompt, ",");
        }
        let _ = writeln!(prompt);
    }
    let _ = write!(prompt, "  ]\n}}\n\n");

    // Constraints reminder
    let _ = writeln!(
        prompt,
        "RULES: Respect all character limits. CTA text must start with an action verb \
         (Get, Start, Try, Join, etc.). Do not include HTML tags in text fields. \
         Rich text allows only <strong>, <em>, <br>, <a href>. \
         Fill all required slots. Optional slots can be empty strings. \
         icon_key slots use short icon names (e.g. rocket, shield, chart, star, globe, zap)."
    );

    // Phase 16: Append learned content hints from self-improvement
    if !defaults.content_prompt_hints.is_empty() {
        let _ = writeln!(
            prompt,
            "\nADDITIONAL STYLE HINTS (learned from past projects):"
        );
        for hint in &defaults.content_prompt_hints {
            let _ = writeln!(prompt, "- {hint}");
        }
    }

    prompt
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot_schema::get_template_schema;
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

    #[test]
    fn test_prompt_contains_brief() {
        let schema = get_template_schema("saas_landing").unwrap();
        let prompt = build_content_prompt(
            "AI writing tool for marketers",
            "saas_landing",
            &schema,
            &default_variant("saas_midnight"),
        );
        assert!(prompt.contains("AI writing tool for marketers"));
    }

    #[test]
    fn test_prompt_contains_all_required_slots() {
        let schema = get_template_schema("saas_landing").unwrap();
        let prompt = build_content_prompt(
            "test brief",
            "saas_landing",
            &schema,
            &default_variant("saas_midnight"),
        );
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    assert!(
                        prompt.contains(slot_name.as_str()),
                        "prompt missing required slot '{slot_name}' from section '{}'",
                        section.section_id
                    );
                }
            }
        }
    }

    #[test]
    fn test_prompt_contains_char_limits() {
        let schema = get_template_schema("saas_landing").unwrap();
        let prompt = build_content_prompt(
            "test brief",
            "saas_landing",
            &schema,
            &default_variant("saas_midnight"),
        );
        // hero headline max is 80
        assert!(
            prompt.contains("max 80"),
            "should contain 'max 80' for headline"
        );
        // hero subtitle max is 160
        assert!(
            prompt.contains("max 160"),
            "should contain 'max 160' for subtitle"
        );
    }

    #[test]
    fn test_prompt_requests_json_only() {
        let schema = get_template_schema("saas_landing").unwrap();
        let prompt = build_content_prompt(
            "test brief",
            "saas_landing",
            &schema,
            &default_variant("saas_midnight"),
        );
        assert!(prompt.contains("JSON"));
        assert!(prompt.contains("no markdown"));
    }

    #[test]
    fn test_prompt_under_token_limit() {
        // Rough proxy: 1 token ≈ 4 chars, so 2000 tokens ≈ 8000 chars
        let schema = get_template_schema("saas_landing").unwrap();
        let prompt = build_content_prompt(
            "AI writing tool for marketers",
            "saas_landing",
            &schema,
            &default_variant("saas_midnight"),
        );
        assert!(
            prompt.len() < 8000,
            "prompt is {} chars, should be < 8000",
            prompt.len()
        );
    }

    #[test]
    fn test_improvement_affects_content_prompt() {
        let schema = get_template_schema("saas_landing").unwrap();
        let variant = default_variant("saas_midnight");

        // Without hints
        let empty_defaults = crate::self_improve::SystemDefaults::default();
        let p1 = build_content_prompt_with_defaults(
            "test brief",
            "saas_landing",
            &schema,
            &variant,
            &empty_defaults,
        );
        assert!(!p1.contains("ADDITIONAL STYLE HINTS"));

        // With hints
        let mut improved = crate::self_improve::SystemDefaults::default();
        improved
            .content_prompt_hints
            .push("Use specific numbers in headlines".into());
        let p2 = build_content_prompt_with_defaults(
            "test brief",
            "saas_landing",
            &schema,
            &variant,
            &improved,
        );
        assert!(p2.contains("ADDITIONAL STYLE HINTS"));
        assert!(p2.contains("Use specific numbers in headlines"));
    }

    #[test]
    fn test_prompt_all_six_templates() {
        let palettes = [
            ("saas_landing", "saas_midnight"),
            ("docs_site", "docs_clean"),
            ("portfolio", "port_monochrome"),
            ("local_business", "biz_warm"),
            ("ecommerce", "ecom_luxe"),
            ("dashboard", "dash_pro"),
        ];
        for (template_id, palette_id) in &palettes {
            let schema = get_template_schema(template_id)
                .unwrap_or_else(|| panic!("schema not found for {template_id}"));
            let prompt = build_content_prompt(
                "test brief for template",
                template_id,
                &schema,
                &default_variant(palette_id),
            );
            assert!(!prompt.is_empty(), "prompt empty for {template_id}");
            assert!(
                prompt.contains("sections"),
                "prompt for {template_id} missing 'sections'"
            );
            // Verify under token limit for all templates
            assert!(
                prompt.len() < 8000,
                "{template_id} prompt is {} chars, should be < 8000",
                prompt.len()
            );
        }
    }
}
