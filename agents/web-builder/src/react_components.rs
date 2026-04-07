//! Section → React Component Generator
//!
//! Transforms a SectionSchema + SectionContent + layout variant into a typed
//! React/TSX component file. Every component:
//! - Has a typed props interface matching the slot schema (camelCase)
//! - Uses Tailwind classes referencing CSS custom property tokens
//! - Preserves `data-nexus-section` and `data-nexus-slot` attributes
//! - Injects content from ContentPayload as default prop values

use crate::content_payload::SectionContent;
use crate::slot_schema::{SectionSchema, SlotConstraint, SlotType};
use std::fmt::Write;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ComponentGenError {
    #[error("section '{0}' not found in content payload")]
    SectionNotFound(String),
}

// ─── Types ──────────────────────────────────────────────────────────────────

/// A generated project file.
#[derive(Debug, Clone)]
pub struct ProjectFile {
    pub path: String,
    pub content: String,
}

// ─── Slot → Prop Mapping ────────────────────────────────────────────────────

/// Convert snake_case slot name to camelCase prop name.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_uppercase().next().unwrap_or(ch));
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert snake_case section_id to PascalCase component name.
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect()
}

/// Map SlotType to TypeScript type string.
fn slot_type_to_ts(slot_type: SlotType) -> &'static str {
    match slot_type {
        SlotType::Text
        | SlotType::Cta
        | SlotType::Url
        | SlotType::ImagePrompt
        | SlotType::VideoEmbed
        | SlotType::RichText
        | SlotType::IconKey => "string",
        SlotType::Number => "number",
    }
}

// ─── Props Interface Generation ─────────────────────────────────────────────

/// Generate the TypeScript interface for a section's props.
fn generate_props_interface(section: &SectionSchema) -> String {
    let name = format!("{}SectionProps", to_pascal_case(&section.section_id));
    let mut iface = format!("interface {name} {{\n");
    for (slot_name, constraint) in &section.slots {
        let prop_name = to_camel_case(slot_name);
        let ts_type = slot_type_to_ts(constraint.slot_type);
        let optional = if constraint.required { "" } else { "?" };
        let _ = writeln!(iface, "  {prop_name}{optional}: {ts_type}");
    }
    iface.push_str("}\n");
    iface
}

// ─── Component Body Generation ──────────────────────────────────────────────

/// Template-specific section styling. Returns Tailwind class strings for the
/// section container and inner wrapper.
fn section_classes(section_id: &str, template_id: &str) -> (&'static str, &'static str) {
    match (template_id, section_id) {
        (_, "hero") => (
            "relative min-h-[80vh] flex items-center bg-hero-bg text-hero-text",
            "max-w-7xl mx-auto px-6 py-section",
        ),
        (_, "features") => (
            "bg-section-bg text-section-text py-section",
            "max-w-7xl mx-auto px-6",
        ),
        (_, "pricing") => (
            "bg-bg-secondary text-section-text py-section",
            "max-w-7xl mx-auto px-6",
        ),
        (_, "testimonials") => (
            "bg-section-bg text-section-text py-section",
            "max-w-7xl mx-auto px-6",
        ),
        (_, "cta") => (
            "bg-primary/10 text-section-text py-section",
            "max-w-4xl mx-auto px-6 text-center",
        ),
        (_, "footer") => (
            "bg-footer-bg text-footer-text py-xl",
            "max-w-7xl mx-auto px-6",
        ),
        (_, "sidebar" | "sidebar_nav") => (
            "w-64 min-h-screen bg-bg-secondary border-r border-border",
            "p-md",
        ),
        (_, "header") => (
            "bg-bg border-b border-border",
            "flex items-center justify-between px-lg py-md",
        ),
        (_, "stats") => (
            "bg-section-bg py-lg",
            "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-md px-lg",
        ),
        (_, "charts") => (
            "bg-section-bg py-lg",
            "grid grid-cols-1 lg:grid-cols-2 gap-lg px-lg",
        ),
        (_, "data_table") => ("bg-section-bg py-lg", "px-lg"),
        (_, "services") => (
            "bg-section-bg text-section-text py-section",
            "max-w-7xl mx-auto px-6",
        ),
        (_, "gallery") => ("bg-bg-secondary py-section", "max-w-7xl mx-auto px-6"),
        (_, "map") => ("bg-section-bg py-lg", "max-w-7xl mx-auto px-6"),
        (_, "hours") => ("bg-bg-secondary py-section", "max-w-7xl mx-auto px-6"),
        (_, "categories") => ("bg-section-bg py-section", "max-w-7xl mx-auto px-6"),
        (_, "products") => ("bg-section-bg py-section", "max-w-7xl mx-auto px-6"),
        (_, "reviews") => ("bg-bg-secondary py-section", "max-w-7xl mx-auto px-6"),
        (_, "newsletter") => (
            "bg-primary/5 py-section",
            "max-w-2xl mx-auto px-6 text-center",
        ),
        (_, "projects") => ("bg-section-bg py-section", "max-w-7xl mx-auto px-6"),
        (_, "about") => ("bg-bg-secondary py-section", "max-w-4xl mx-auto px-6"),
        (_, "skills") => ("bg-section-bg py-section", "max-w-4xl mx-auto px-6"),
        (_, "contact") => (
            "bg-bg-secondary py-section",
            "max-w-2xl mx-auto px-6 text-center",
        ),
        (_, "search") => ("bg-bg border-b border-border", "px-lg py-md"),
        (_, "content") => ("bg-bg flex-1 py-lg", "max-w-4xl mx-auto px-lg"),
        (_, "code_blocks") => ("bg-bg-secondary py-lg", "max-w-4xl mx-auto px-lg"),
        _ => (
            "bg-section-bg text-section-text py-section",
            "max-w-7xl mx-auto px-6",
        ),
    }
}

/// Generate a slot render expression for the component body.
fn render_slot(
    prop_name: &str,
    constraint: &SlotConstraint,
    _section_id: &str,
    slot_name: &str,
) -> String {
    match constraint.slot_type {
        SlotType::Text => {
            if constraint.required {
                format!("<span data-nexus-slot=\"{slot_name}\">{{{prop_name}}}</span>")
            } else {
                format!(
                    "{{{prop_name} && <span data-nexus-slot=\"{slot_name}\">{{{prop_name}}}</span>}}"
                )
            }
        }
        SlotType::Cta => {
            if constraint.required {
                format!(
                    "<a href=\"#\" className=\"inline-block bg-btn-bg text-btn-text px-lg py-md rounded-md font-semibold transition-colors duration-fast hover:opacity-90\" data-nexus-slot=\"{slot_name}\">{{{prop_name}}}</a>"
                )
            } else {
                format!(
                    "{{{prop_name} && <a href=\"#\" className=\"inline-block bg-btn-bg text-btn-text px-lg py-md rounded-md font-semibold transition-colors duration-fast hover:opacity-90\" data-nexus-slot=\"{slot_name}\">{{{prop_name}}}</a>}}"
                )
            }
        }
        SlotType::Url => {
            format!("{{{prop_name} && <a href={{{prop_name}}} data-nexus-slot=\"{slot_name}\" className=\"text-primary hover:underline\">{{{prop_name}}}</a>}}")
        }
        SlotType::ImagePrompt => {
            format!(
                "<div className=\"bg-bg-secondary rounded-lg aspect-video flex items-center justify-center text-text-secondary text-sm\" data-nexus-slot=\"{slot_name}\" aria-label={{{prop_name} || 'Image placeholder'}}>{{{prop_name} || 'Image'}}</div>"
            )
        }
        SlotType::RichText => {
            if constraint.required {
                format!(
                    "<div data-nexus-slot=\"{slot_name}\" dangerouslySetInnerHTML={{{{ __html: {prop_name} }}}} />"
                )
            } else {
                format!(
                    "{{{prop_name} && <div data-nexus-slot=\"{slot_name}\" dangerouslySetInnerHTML={{{{ __html: {prop_name} }}}} />}}"
                )
            }
        }
        SlotType::Number => {
            format!("<span data-nexus-slot=\"{slot_name}\">{{{prop_name}}}</span>")
        }
        SlotType::IconKey => {
            format!(
                "<span className=\"text-xl\" data-nexus-slot=\"{slot_name}\" aria-hidden=\"true\">{{{prop_name}}}</span>"
            )
        }
        SlotType::VideoEmbed => {
            format!(
                "{{{prop_name} && <iframe src={{{prop_name}}} className=\"w-full aspect-video rounded-lg\" data-nexus-slot=\"{slot_name}\" title=\"Video\" />}}"
            )
        }
    }
}

// ─── Component File Generation ──────────────────────────────────────────────

/// Generate a React component file for a template section.
pub fn generate_section_component(
    section_schema: &SectionSchema,
    section_content: Option<&SectionContent>,
    _variant_layout: &str,
    template_id: &str,
) -> Result<ProjectFile, ComponentGenError> {
    let section_id = &section_schema.section_id;
    let component_name = format!("{}Section", to_pascal_case(section_id));
    let (section_cls, inner_cls) = section_classes(section_id, template_id);

    let mut tsx = String::with_capacity(2048);

    // Props interface
    let _ = write!(tsx, "{}", generate_props_interface(section_schema));
    let _ = writeln!(tsx);

    // Component function
    let _ = writeln!(
        tsx,
        "export default function {component_name}(props: {component_name}Props) {{"
    );

    // Destructure props
    let prop_names: Vec<String> = section_schema
        .slots
        .keys()
        .map(|s| to_camel_case(s))
        .collect();
    let _ = writeln!(tsx, "  const {{ {} }} = props", prop_names.join(", "));
    let _ = writeln!(tsx);

    // Return JSX
    let _ = writeln!(tsx, "  return (");
    let _ = writeln!(
        tsx,
        "    <section data-nexus-section=\"{section_id}\" data-nexus-editable=\"true\" className=\"{section_cls}\">"
    );
    let _ = writeln!(tsx, "      <div className=\"{inner_cls}\">");

    // Render slots with sensible layout grouping
    generate_section_body(&mut tsx, section_schema, section_id, template_id);

    let _ = writeln!(tsx, "      </div>");
    let _ = writeln!(tsx, "    </section>");
    let _ = writeln!(tsx, "  )");
    let _ = writeln!(tsx, "}}");

    // Default props export (from content payload)
    if let Some(content) = section_content {
        let _ = writeln!(tsx);
        let _ = writeln!(
            tsx,
            "export const default{component_name}Props: {component_name}Props = {{"
        );
        for (slot_name, constraint) in &section_schema.slots {
            let prop_name = to_camel_case(slot_name);
            if let Some(value) = content.slots.get(slot_name) {
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'")
                    .replace('\n', "\\n");
                if constraint.slot_type == SlotType::Number {
                    let _ = writeln!(tsx, "  {prop_name}: {escaped},");
                } else {
                    let _ = writeln!(tsx, "  {prop_name}: '{escaped}',");
                }
            } else if constraint.required {
                let _ = writeln!(tsx, "  {prop_name}: '',");
            }
        }
        let _ = writeln!(tsx, "}}");
    }

    let path = format!("src/components/{}Section.tsx", to_pascal_case(section_id));
    Ok(ProjectFile { path, content: tsx })
}

/// Generate the inner JSX body for a section based on its slot patterns.
fn generate_section_body(
    tsx: &mut String,
    schema: &SectionSchema,
    section_id: &str,
    _template_id: &str,
) {
    let slots: Vec<(&String, &SlotConstraint)> = schema.slots.iter().collect();

    // Group slots by pattern for better layout
    let headings: Vec<_> = slots
        .iter()
        .filter(|(name, c)| {
            (name.as_str() == "heading"
                || name.as_str() == "headline"
                || name.as_str() == "main_heading")
                && c.slot_type == SlotType::Text
        })
        .collect();

    let subheadings: Vec<_> = slots
        .iter()
        .filter(|(name, _)| {
            name.as_str() == "subheading"
                || name.as_str() == "subtitle"
                || name.as_str() == "subtext"
                || name.as_str() == "tagline"
                || name.as_str() == "bio"
        })
        .collect();

    let ctas: Vec<_> = slots
        .iter()
        .filter(|(_, c)| c.slot_type == SlotType::Cta)
        .collect();

    // Render heading
    for (name, _) in &headings {
        let prop = to_camel_case(name);
        let _ = writeln!(
            tsx,
            "        <h2 className=\"text-4xl font-heading font-bold mb-md\" data-nexus-slot=\"{name}\">{{{prop}}}</h2>"
        );
    }

    // Render subheading
    for (name, _) in &subheadings {
        let prop = to_camel_case(name);
        let _ = writeln!(
            tsx,
            "        <p className=\"text-lg text-text-secondary mb-xl\" data-nexus-slot=\"{name}\">{{{prop}}}</p>"
        );
    }

    // Detect repeating groups (feature_1_*, feature_2_*, etc.)
    let groups = detect_repeating_groups(&slots);
    if !groups.is_empty() {
        let _ = writeln!(
            tsx,
            "        <div className=\"grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-lg mt-xl\">"
        );
        for group in &groups {
            let _ = writeln!(tsx, "          <div className=\"bg-card-bg border border-card-border rounded-lg p-lg\">");
            for (slot_name, constraint) in &group.slots {
                let prop = to_camel_case(slot_name);
                if slot_name.contains("icon") {
                    let _ = writeln!(
                        tsx,
                        "            {}",
                        render_slot(&prop, constraint, section_id, slot_name)
                    );
                } else if slot_name.contains("title") || slot_name.contains("name") {
                    let _ = writeln!(
                        tsx,
                        "            <h3 className=\"text-xl font-heading font-semibold mt-sm\" data-nexus-slot=\"{slot_name}\">{{{prop}}}</h3>"
                    );
                } else if slot_name.contains("desc")
                    || slot_name.contains("quote")
                    || slot_name.contains("text")
                {
                    let _ = writeln!(
                        tsx,
                        "            <p className=\"text-sm text-text-secondary mt-xs\" data-nexus-slot=\"{slot_name}\">{{{prop}}}</p>"
                    );
                } else if slot_name.contains("price") {
                    let _ = writeln!(
                        tsx,
                        "            <div className=\"text-3xl font-bold text-primary mt-sm\" data-nexus-slot=\"{slot_name}\">{{{prop}}}</div>"
                    );
                } else if slot_name.contains("image") {
                    let _ = writeln!(
                        tsx,
                        "            {}",
                        render_slot(&prop, constraint, section_id, slot_name)
                    );
                } else if slot_name.contains("author") || slot_name.contains("role") {
                    let _ = writeln!(
                        tsx,
                        "            <span className=\"text-xs text-text-secondary\" data-nexus-slot=\"{slot_name}\">{{{prop}}}</span>"
                    );
                } else {
                    let _ = writeln!(
                        tsx,
                        "            {}",
                        render_slot(&prop, constraint, section_id, slot_name)
                    );
                }
            }
            let _ = writeln!(tsx, "          </div>");
        }
        let _ = writeln!(tsx, "        </div>");
    }

    // Render CTAs
    if !ctas.is_empty() {
        let _ = writeln!(tsx, "        <div className=\"flex gap-md mt-xl\">");
        for (name, constraint) in &ctas {
            let prop = to_camel_case(name);
            let _ = writeln!(
                tsx,
                "          {}",
                render_slot(&prop, constraint, section_id, name)
            );
        }
        let _ = writeln!(tsx, "        </div>");
    }

    // Render remaining slots not covered by groups, headings, subheadings, or CTAs
    let covered: std::collections::HashSet<&str> = headings
        .iter()
        .chain(subheadings.iter())
        .chain(ctas.iter())
        .map(|(name, _)| name.as_str())
        .chain(groups.iter().flat_map(|g| g.slots.iter().map(|(n, _)| *n)))
        .collect();

    for (name, constraint) in &slots {
        if covered.contains(name.as_str()) {
            continue;
        }
        let prop = to_camel_case(name);
        let _ = writeln!(
            tsx,
            "        {}",
            render_slot(&prop, constraint, section_id, name)
        );
    }
}

// ─── Repeating Group Detection ──────────────────────────────────────────────

struct SlotGroup<'a> {
    slots: Vec<(&'a str, &'a SlotConstraint)>,
}

/// Detect repeating slot groups like feature_1_*, feature_2_*, etc.
fn detect_repeating_groups<'a>(slots: &[(&'a String, &'a SlotConstraint)]) -> Vec<SlotGroup<'a>> {
    let mut prefixes: std::collections::BTreeMap<String, Vec<(&'a str, &'a SlotConstraint)>> =
        std::collections::BTreeMap::new();

    for (name, constraint) in slots {
        // Match patterns like "feature_1_title", "tier_2_price", "product_3_name"
        if let Some(prefix_end) = find_numeric_prefix(name) {
            let prefix = &name[..prefix_end];
            prefixes
                .entry(prefix.to_string())
                .or_default()
                .push((name.as_str(), *constraint));
        }
    }

    prefixes
        .into_values()
        .filter(|group| group.len() >= 2) // Only groups with 2+ slots
        .map(|slots| SlotGroup { slots })
        .collect()
}

/// Find the end index of a numeric-prefixed slot name (e.g., "feature_1_" → 10).
fn find_numeric_prefix(name: &str) -> Option<usize> {
    let parts: Vec<&str> = name.split('_').collect();
    if parts.len() >= 3 {
        // Check if the second-to-last or an inner part is numeric
        for (i, part) in parts.iter().enumerate() {
            if i > 0 && i < parts.len() - 1 && part.chars().all(|c| c.is_ascii_digit()) {
                // Return index right after the number part + underscore
                let idx: usize = parts[..=i].iter().map(|p| p.len() + 1).sum();
                return Some(idx);
            }
        }
    }
    None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_payload::SectionContent;
    use crate::slot_schema::get_template_schema;
    use std::collections::HashMap;

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("cta_primary"), "ctaPrimary");
        assert_eq!(to_camel_case("headline"), "headline");
        assert_eq!(to_camel_case("feature_1_title"), "feature1Title");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("hero"), "Hero");
        assert_eq!(to_pascal_case("saas_landing"), "SaasLanding");
        assert_eq!(to_pascal_case("sidebar_nav"), "SidebarNav");
    }

    #[test]
    fn test_generate_section_component_has_data_nexus_section() {
        let schema = get_template_schema("saas_landing").unwrap();
        let hero_schema = &schema.sections[0];
        let file =
            generate_section_component(hero_schema, None, "centered", "saas_landing").unwrap();
        assert!(file.content.contains("data-nexus-section=\"hero\""));
        assert!(file.content.contains("data-nexus-editable=\"true\""));
    }

    #[test]
    fn test_generate_section_component_has_typed_props() {
        let schema = get_template_schema("saas_landing").unwrap();
        let hero_schema = &schema.sections[0];
        let file =
            generate_section_component(hero_schema, None, "centered", "saas_landing").unwrap();
        // Required props should not have ?
        assert!(file.content.contains("headline: string"));
        assert!(file.content.contains("subtitle: string"));
        assert!(file.content.contains("ctaPrimary: string"));
        // Optional props should have ?
        assert!(file.content.contains("badge?: string"));
        assert!(file.content.contains("ctaSecondary?: string"));
    }

    #[test]
    fn test_generate_section_component_has_data_nexus_slot() {
        let schema = get_template_schema("saas_landing").unwrap();
        let hero_schema = &schema.sections[0];
        let file =
            generate_section_component(hero_schema, None, "centered", "saas_landing").unwrap();
        assert!(file.content.contains("data-nexus-slot="));
    }

    #[test]
    fn test_generate_all_saas_sections() {
        let schema = get_template_schema("saas_landing").unwrap();
        for section in &schema.sections {
            let file =
                generate_section_component(section, None, "default", "saas_landing").unwrap();
            assert!(
                file.content
                    .contains(&format!("data-nexus-section=\"{}\"", section.section_id)),
                "Missing data-nexus-section for {}",
                section.section_id
            );
            assert!(
                file.path.ends_with(".tsx"),
                "File should be .tsx: {}",
                file.path
            );
        }
    }

    #[test]
    fn test_generate_component_with_content() {
        let schema = get_template_schema("saas_landing").unwrap();
        let hero_schema = &schema.sections[0];
        let content = SectionContent {
            section_id: "hero".into(),
            slots: HashMap::from([
                ("headline".into(), "Build Faster".into()),
                ("subtitle".into(), "The modern platform.".into()),
                ("cta_primary".into(), "Start Free".into()),
            ]),
        };
        let file =
            generate_section_component(hero_schema, Some(&content), "centered", "saas_landing")
                .unwrap();
        assert!(file.content.contains("'Build Faster'"));
        assert!(file.content.contains("defaultHeroSectionProps"));
    }

    #[test]
    fn test_generate_components_all_six_templates() {
        let template_ids = [
            "saas_landing",
            "docs_site",
            "portfolio",
            "local_business",
            "ecommerce",
            "dashboard",
        ];
        for tid in &template_ids {
            let schema = get_template_schema(tid).unwrap();
            for section in &schema.sections {
                let file = generate_section_component(section, None, "default", tid);
                assert!(
                    file.is_ok(),
                    "Failed to generate component for {tid}/{}: {:?}",
                    section.section_id,
                    file.err()
                );
            }
        }
    }
}
