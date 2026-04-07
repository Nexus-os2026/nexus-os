//! Template engine for Nexus Builder Phase 2.
//!
//! Provides 6 production-quality HTML skeletons with section anchors (`data-nexus-section`)
//! and content slots (`data-nexus-slot` + `{{PLACEHOLDER}}`). Sonnet customizes these
//! scaffolds instead of hallucinating structure from zero.

pub mod modifiers;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Phase 1 Legacy Types (preserved for backward compat) ───────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateCategory {
    Hero,
    Features,
    Testimonials,
    Pricing,
    Contact,
    Navigation,
    Footer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub category: TemplateCategory,
    pub label: String,
    pub component_source: String,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateEngine {
    templates: HashMap<String, TemplateDefinition>,
}

impl TemplateEngine {
    pub fn new() -> Self {
        let mut engine = Self::default();
        for template in default_templates() {
            engine.templates.insert(template.id.clone(), template);
        }
        engine
    }

    pub fn get(&self, id: &str) -> Option<&TemplateDefinition> {
        self.templates.get(id)
    }

    pub fn by_category(&self, category: TemplateCategory) -> Vec<&TemplateDefinition> {
        let mut matches = self
            .templates
            .values()
            .filter(|template| template.category == category)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.id.cmp(&right.id));
        matches
    }

    pub fn render_component(&self, id: &str, title: &str, body: &str) -> Option<String> {
        let template = self.get(id)?;
        Some(
            template
                .component_source
                .replace("{{title}}", title)
                .replace("{{body}}", body),
        )
    }
}

pub fn default_template_engine() -> TemplateEngine {
    TemplateEngine::new()
}

fn default_templates() -> Vec<TemplateDefinition> {
    vec![
        template("hero-split", TemplateCategory::Hero, "Hero Split", "<section className=\"grid gap-8 md:grid-cols-2\"><div><h1 className=\"text-5xl font-display\">{{title}}</h1><p className=\"mt-4 text-lg\">{{body}}</p></div><div className=\"rounded-3xl border border-white/10 bg-black/20 p-6\">Visual slot</div></section>"),
        template("hero-centered", TemplateCategory::Hero, "Hero Centered", "<section className=\"py-24 text-center\"><h1 className=\"text-6xl font-display\">{{title}}</h1><p className=\"mx-auto mt-6 max-w-2xl\">{{body}}</p></section>"),
        template("hero-video-bg", TemplateCategory::Hero, "Hero Video", "<section className=\"relative min-h-[70vh] overflow-hidden\"><div className=\"absolute inset-0 bg-black/50\" /><div className=\"relative z-10 p-12\"><h1>{{title}}</h1><p>{{body}}</p></div></section>"),
        template("hero-3d-product", TemplateCategory::Hero, "Hero 3D", "<section className=\"grid gap-8 md:grid-cols-2\"><div><h1 className=\"text-5xl font-display\">{{title}}</h1><p>{{body}}</p></div><div aria-label=\"3d-scene\" className=\"h-[420px] rounded-3xl border border-cyan-300/50\">3D Scene</div></section>"),
        template("hero-particles", TemplateCategory::Hero, "Hero Particles", "<section className=\"relative overflow-hidden py-24\"><div className=\"absolute inset-0\" id=\"particles\" /><h1>{{title}}</h1><p>{{body}}</p></section>"),
        template("hero-gradient", TemplateCategory::Hero, "Hero Gradient", "<section className=\"rounded-[2rem] bg-gradient-to-br from-fuchsia-500/40 to-cyan-400/30 p-16\"><h1>{{title}}</h1><p>{{body}}</p></section>"),
        template("hero-minimal", TemplateCategory::Hero, "Hero Minimal", "<section className=\"py-20\"><h1 className=\"text-4xl tracking-tight\">{{title}}</h1><p className=\"mt-4 text-zinc-300\">{{body}}</p></section>"),
        template("hero-editorial", TemplateCategory::Hero, "Hero Editorial", "<section className=\"grid gap-10 lg:grid-cols-[2fr_1fr]\"><article><h1>{{title}}</h1><p>{{body}}</p></article><aside>Highlights</aside></section>"),
        template("hero-glass", TemplateCategory::Hero, "Hero Glass", "<section className=\"rounded-3xl border border-white/20 bg-white/10 p-12 backdrop-blur\"><h1>{{title}}</h1><p>{{body}}</p></section>"),
        template("hero-brutalist", TemplateCategory::Hero, "Hero Brutalist", "<section className=\"border-4 border-black bg-yellow-300 p-12\"><h1>{{title}}</h1><p>{{body}}</p></section>"),
        template("features-card-grid", TemplateCategory::Features, "Features Cards", "<section className=\"grid gap-6 md:grid-cols-3\"><article className=\"rounded-2xl border p-6\">Feature A</article><article className=\"rounded-2xl border p-6\">Feature B</article><article className=\"rounded-2xl border p-6\">Feature C</article></section>"),
        template("features-alternating", TemplateCategory::Features, "Features Alternating", "<section className=\"space-y-12\"><div className=\"grid md:grid-cols-2\">Feature Block</div><div className=\"grid md:grid-cols-2\">Feature Block</div></section>"),
        template("testimonials-carousel", TemplateCategory::Testimonials, "Testimonials Carousel", "<section className=\"overflow-hidden\"><blockquote>{{body}}</blockquote></section>"),
        template("testimonials-grid", TemplateCategory::Testimonials, "Testimonials Grid", "<section className=\"grid gap-4 md:grid-cols-3\"><blockquote>Quote</blockquote></section>"),
        template("pricing-tiered", TemplateCategory::Pricing, "Pricing Table", "<section className=\"grid gap-4 md:grid-cols-3\"><article className=\"rounded-2xl border p-6\">Starter</article></section>"),
        template("contact-split-form", TemplateCategory::Contact, "Contact Split", "<section className=\"grid gap-8 md:grid-cols-2\"><form aria-label=\"contact form\">Form</form><aside>Details</aside></section>"),
        template("menu-two-column", TemplateCategory::Features, "Menu Columns", "<section className=\"grid gap-8 md:grid-cols-2\"><article>Drinks</article><article>Food</article></section>"),
        template("nav-fixed", TemplateCategory::Navigation, "Navigation Fixed", "<nav className=\"sticky top-0 flex items-center justify-between bg-black/50 p-4\" aria-label=\"main navigation\">Brand</nav>"),
        template("nav-hamburger", TemplateCategory::Navigation, "Navigation Hamburger", "<nav className=\"flex items-center justify-between p-4\"><button aria-label=\"open navigation\">Menu</button></nav>"),
        template("footer-simple", TemplateCategory::Footer, "Footer Simple", "<footer className=\"border-t border-white/20 py-10\">{{body}}</footer>"),
        template("footer-newsletter", TemplateCategory::Footer, "Footer Newsletter", "<footer className=\"space-y-4 border-t border-white/20 py-10\"><p>{{body}}</p><form aria-label=\"newsletter\">Subscribe</form></footer>"),
    ]
}

fn template(id: &str, category: TemplateCategory, label: &str, source: &str) -> TemplateDefinition {
    TemplateDefinition {
        id: id.to_string(),
        category,
        label: label.to_string(),
        component_source: source.to_string(),
    }
}

// ─── Phase 2: Full-Page Template Skeletons ──────────────────────────────────

/// A full-page HTML template skeleton with section anchors and content slots.
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub html: &'static str,
    pub sections: &'static [&'static str],
    pub keywords: &'static [&'static str],
}

static SAAS_LANDING: Template = Template {
    id: "saas_landing",
    name: "SaaS Landing Page",
    description: "Modern dark SaaS landing page with hero, features grid, pricing tiers, testimonials, and CTA",
    html: include_str!("saas_landing.html"),
    sections: &["hero", "features", "pricing", "testimonials", "cta", "footer"],
    keywords: &["saas", "landing", "startup", "product", "pricing", "features", "software", "app", "platform"],
};

static DOCS_SITE: Template = Template {
    id: "docs_site",
    name: "Documentation Site",
    description: "Clean light documentation site with sidebar navigation, search bar, code blocks, and callout boxes",
    html: include_str!("docs_site.html"),
    sections: &["sidebar_nav", "search", "content", "code_blocks", "footer"],
    keywords: &["docs", "documentation", "api", "reference", "guide", "tutorial", "manual", "wiki"],
};

static PORTFOLIO: Template = Template {
    id: "portfolio",
    name: "Personal Portfolio",
    description:
        "Minimal elegant portfolio with project grid, about section, skills tags, and contact form",
    html: include_str!("portfolio.html"),
    sections: &["hero", "projects", "about", "skills", "contact", "footer"],
    keywords: &[
        "portfolio",
        "personal",
        "resume",
        "cv",
        "freelance",
        "projects",
        "developer",
        "designer",
    ],
};

static LOCAL_BUSINESS: Template = Template {
    id: "local_business",
    name: "Local Business",
    description:
        "Warm inviting local business site with services, gallery, testimonials, map, and hours",
    html: include_str!("local_business.html"),
    sections: &[
        "hero",
        "services",
        "gallery",
        "testimonials",
        "map",
        "hours",
        "footer",
    ],
    keywords: &[
        "restaurant",
        "bakery",
        "salon",
        "shop",
        "store",
        "local",
        "business",
        "cafe",
        "gym",
        "clinic",
        "dental",
        "spa",
    ],
};

static ECOMMERCE: Template = Template {
    id: "ecommerce",
    name: "E-Commerce Store",
    description:
        "Clean commerce layout with categories, product grid, star ratings, reviews, and newsletter",
    html: include_str!("ecommerce.html"),
    sections: &[
        "hero",
        "categories",
        "products",
        "reviews",
        "newsletter",
        "footer",
    ],
    keywords: &[
        "ecommerce",
        "e-commerce",
        "shop",
        "store",
        "products",
        "buy",
        "sell",
        "cart",
        "marketplace",
    ],
};

static DASHBOARD: Template = Template {
    id: "dashboard",
    name: "Admin Dashboard",
    description:
        "Dark professional dashboard with sidebar, stat cards, data table, and chart placeholders",
    html: include_str!("dashboard.html"),
    sections: &[
        "sidebar",
        "header",
        "stats",
        "charts",
        "data_table",
        "footer",
    ],
    keywords: &[
        "dashboard",
        "admin",
        "analytics",
        "panel",
        "metrics",
        "data",
        "monitoring",
        "crm",
    ],
};

/// All available full-page templates.
static ALL_TEMPLATES: &[&Template] = &[
    &SAAS_LANDING,
    &DOCS_SITE,
    &PORTFOLIO,
    &LOCAL_BUSINESS,
    &ECOMMERCE,
    &DASHBOARD,
];

impl Template {
    /// Return a compact section spec suitable for LLM prompts.
    ///
    /// Instead of embedding the full 30-40KB HTML scaffold, this produces a
    /// terse description (~200-400 chars) listing section IDs, required
    /// `data-nexus-*` attributes, and key structural hints extracted from
    /// the template metadata.
    pub fn compact_spec(&self) -> String {
        let sections_csv = self.sections.join(", ");
        format!(
            "Template: {id} — {desc}\n\
             Sections: [{sections}]\n\
             Each <section> MUST have data-nexus-section=\"<id>\" and data-nexus-editable=\"true\".\n\
             Use semantic HTML5 elements. Expand cards/grids to match the plan's section list.",
            id = self.id,
            desc = self.description,
            sections = sections_csv,
        )
    }
}

/// Get a template by ID.
pub fn get_template(id: &str) -> Option<&'static Template> {
    ALL_TEMPLATES.iter().find(|t| t.id == id).copied()
}

/// Get all available templates.
pub fn all_templates() -> &'static [&'static Template] {
    ALL_TEMPLATES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_templates_count() {
        assert_eq!(all_templates().len(), 6);
    }

    #[test]
    fn test_get_template_found() {
        let t = get_template("saas_landing").unwrap();
        assert_eq!(t.name, "SaaS Landing Page");
        assert!(t.html.contains("<!DOCTYPE html>"));
        assert!(t.html.contains("data-nexus-section"));
        assert!(t.html.contains("data-nexus-editable=\"true\""));
    }

    #[test]
    fn test_get_template_not_found() {
        assert!(get_template("nonexistent").is_none());
    }

    #[test]
    fn test_all_templates_have_required_attributes() {
        for t in all_templates() {
            assert!(
                t.html.contains("<!DOCTYPE html>"),
                "{} missing DOCTYPE",
                t.id
            );
            assert!(
                t.html.contains("data-nexus-section"),
                "{} missing data-nexus-section",
                t.id
            );
            assert!(
                t.html.contains("data-nexus-editable=\"true\""),
                "{} missing data-nexus-editable",
                t.id
            );
            assert!(
                t.html.contains("data-nexus-slot"),
                "{} missing data-nexus-slot",
                t.id
            );
            assert!(t.html.contains("aria-label"), "{} missing aria-label", t.id);
            assert!(
                t.html.contains("@media"),
                "{} missing responsive breakpoints",
                t.id
            );
            assert!(
                t.html.contains("--primary"),
                "{} missing CSS custom properties",
                t.id
            );
            assert!(!t.sections.is_empty(), "{} has no sections", t.id);
            assert!(!t.keywords.is_empty(), "{} has no keywords", t.id);
        }
    }

    #[test]
    fn test_template_sections_match_html() {
        for t in all_templates() {
            for section_id in t.sections {
                let attr = format!("data-nexus-section=\"{section_id}\"");
                assert!(
                    t.html.contains(&attr),
                    "Template {} missing section anchor: {section_id}",
                    t.id
                );
            }
        }
    }

    // Legacy Phase 1 tests
    #[test]
    fn test_legacy_template_engine() {
        let engine = TemplateEngine::new();
        assert!(engine.get("hero-split").is_some());
        assert!(engine.get("nonexistent").is_none());
    }

    #[test]
    fn test_legacy_by_category() {
        let engine = TemplateEngine::new();
        let heroes = engine.by_category(TemplateCategory::Hero);
        assert!(heroes.len() >= 5);
    }

    #[test]
    fn test_legacy_render_component() {
        let engine = TemplateEngine::new();
        let rendered = engine
            .render_component("hero-centered", "Hello", "World")
            .unwrap();
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("World"));
    }

    // ── Phase 6.2: saas_landing template validation tests ─────────────

    #[test]
    fn test_saas_landing_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("saas_landing").unwrap();
        let schema = slot_schema::get_template_schema("saas_landing").unwrap();
        // Every section in the schema must have a data-nexus-section in the HTML
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "saas_landing HTML missing section anchor for schema section '{}'",
                section.section_id
            );
        }
        // Every section in the template metadata must exist in the schema
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "saas_landing template section '{}' not found in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_saas_landing_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("saas_landing").unwrap();
        let schema = slot_schema::get_template_schema("saas_landing").unwrap();
        // Check that every required slot in the schema has a corresponding placeholder in the HTML.
        // Slots may appear as:
        //   data-nexus-slot="slot_name"  or  data-nexus-slot="section_slot_name"
        //   {{SLOT_NAME}}               or  {{SECTION_SLOT_NAME}}
        // The HTML convention is to prefix shared slot names (like "heading") with the section ID.
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    // Direct form
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_placeholder = template.html.contains(&direct_upper);

                    // Section-prefixed form (e.g., features_heading for section=features, slot=heading)
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_placeholder = template.html.contains(&prefixed_upper);

                    assert!(
                        has_direct_attr || has_direct_placeholder || has_prefixed_attr || has_prefixed_placeholder,
                        "saas_landing missing required slot '{}' (section '{}') — looked for '{}' or '{}' in data-nexus-slot or {{{{}}}} placeholders",
                        slot_name, section.section_id, slot_name, prefixed
                    );
                }
            }
        }
    }

    #[test]
    fn test_saas_landing_zero_hardcoded_colors_outside_root() {
        let template = get_template("saas_landing").unwrap();
        let html = template.html;
        // Extract CSS content from <style> block
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];

        // Find the :root block end — colors inside :root are allowed
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        let outside_root = &style_content[root_end..];

        // Scan for raw hex colors (#xxx, #xxxxxx, #xxxxxxxx)
        let hex_re_3 = regex_lite_hex_scan(outside_root);
        assert!(
            hex_re_3.is_empty(),
            "saas_landing CSS has hardcoded hex colors outside :root: {:?}",
            hex_re_3
        );
    }

    /// Simple scanner for hex color patterns outside :root.
    /// Returns found hex values. Skips things inside comments, data-theme blocks,
    /// @media prefers-color-scheme blocks, and [data-theme] blocks.
    fn regex_lite_hex_scan(css: &str) -> Vec<String> {
        let mut found = Vec::new();
        // We need to be careful: dark mode and data-theme blocks legitimately contain hex values.
        // Skip any block that starts with `@media (prefers-color-scheme`, `[data-theme=`, or `/* `
        let mut i = 0;
        let bytes = css.as_bytes();
        let mut in_allowed_block = false;
        let mut brace_depth: i32 = 0;
        let mut allowed_depth: i32 = 0;

        while i < bytes.len() {
            let ch = bytes[i] as char;
            // Track braces
            if ch == '{' {
                brace_depth += 1;
                if in_allowed_block {
                    allowed_depth = brace_depth;
                }
            } else if ch == '}' {
                if in_allowed_block && brace_depth <= allowed_depth {
                    in_allowed_block = false;
                }
                brace_depth -= 1;
            }

            // Check if entering an allowed block
            if !in_allowed_block {
                let remaining = &css[i..];
                if remaining.starts_with("@media (prefers-color-scheme")
                    || remaining.starts_with("[data-theme")
                {
                    in_allowed_block = true;
                }
            }

            // Skip comments
            if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                if let Some(end) = css[i + 2..].find("*/") {
                    i = i + 2 + end + 2;
                    continue;
                }
            }

            // Check for # followed by hex digits (but not inside allowed blocks)
            if ch == '#' && !in_allowed_block {
                let hex_start = i + 1;
                let mut hex_len = 0;
                while hex_start + hex_len < bytes.len() {
                    let hch = bytes[hex_start + hex_len];
                    if hch.is_ascii_hexdigit() {
                        hex_len += 1;
                    } else {
                        break;
                    }
                }
                if hex_len >= 3 && hex_len <= 8 {
                    // Check it's a standalone color, not part of a word like #features
                    // CSS selectors start with a letter after #, colors start with digits or a-f
                    let first_hex = bytes[hex_start];
                    if first_hex.is_ascii_digit()
                        || (b'a'..=b'f').contains(&first_hex)
                        || (b'A'..=b'F').contains(&first_hex)
                    {
                        // Verify it's not an anchor link by checking context
                        let is_href_anchor = i > 0 && {
                            let before = &css[..i];
                            before.ends_with("href=\"") || before.ends_with("href='")
                        };
                        if !is_href_anchor {
                            found.push(format!("#{}", &css[hex_start..hex_start + hex_len]));
                        }
                    }
                }
            }
            i += 1;
        }
        found
    }

    #[test]
    fn test_saas_landing_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("saas_landing").unwrap();
        let html = template.html;
        // Scan for var(--xxx) and verify each reference is a known token
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6; // skip "var(--"
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                // Handle nested var() like color-mix
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                // Check if it's a known token
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "saas_landing references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_saas_landing_responsive_breakpoints() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("768px"),
            "saas_landing missing 768px breakpoint"
        );
        assert!(
            template.html.contains("1024px"),
            "saas_landing missing 1024px breakpoint"
        );
    }

    #[test]
    fn test_saas_landing_no_fixed_width_containers() {
        let template = get_template("saas_landing").unwrap();
        // Check that no CSS contains width: Npx where N > 320
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        // Simple check: no "width: " followed by a number > 320
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                // Skip max-width, min-width (those are fine)
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                // Skip percentage and rem/em values
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                {
                    continue;
                }
                // Check for px values > 320
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(
                            num <= 320.0,
                            "saas_landing has fixed width: {}px — use max-width or responsive units",
                            num
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_saas_landing_dark_mode_support() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "saas_landing missing prefers-color-scheme dark media query"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "saas_landing missing [data-theme='dark'] manual toggle support"
        );
    }

    #[test]
    fn test_saas_landing_reduced_motion() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "saas_landing missing prefers-reduced-motion support"
        );
    }

    #[test]
    fn test_saas_landing_scroll_reveal() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("nexus-reveal"),
            "saas_landing missing nexus-reveal scroll animation class"
        );
        assert!(
            template.html.contains("IntersectionObserver"),
            "saas_landing missing IntersectionObserver for scroll reveals"
        );
        assert!(
            template.html.contains("is-visible"),
            "saas_landing missing is-visible reveal class"
        );
    }

    #[test]
    fn test_saas_landing_accessibility() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "saas_landing missing skip-to-content link"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "saas_landing missing aria-expanded on mobile nav"
        );
        assert!(
            template.html.contains("<main"),
            "saas_landing missing <main> element"
        );
        assert!(
            template.html.contains("<nav"),
            "saas_landing missing <nav> element"
        );
        assert!(
            template.html.contains("<footer"),
            "saas_landing missing <footer> element"
        );
        // Focus visible
        assert!(
            template.html.contains("focus-visible"),
            "saas_landing missing :focus-visible styles"
        );
    }

    #[test]
    fn test_saas_landing_glassmorphism_with_fallback() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("backdrop-filter: blur"),
            "saas_landing missing backdrop-filter for glassmorphism"
        );
        assert!(
            template.html.contains("@supports not (backdrop-filter")
                || template
                    .html
                    .contains("@supports not (-webkit-backdrop-filter"),
            "saas_landing missing @supports fallback for backdrop-filter"
        );
    }

    #[test]
    fn test_saas_landing_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("saas_landing");
        assert_eq!(
            palettes.len(),
            4,
            "expected 4 palette presets for saas_landing"
        );
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "tech".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(
                token_set.is_some(),
                "Palette '{}' failed to produce a TokenSet",
                palette.id
            );
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' CSS missing --color-primary",
                palette.id
            );
            assert!(
                css.contains("--btn-bg: var(--color-primary)"),
                "Palette '{}' CSS missing semantic btn-bg",
                palette.id
            );
        }
    }

    #[test]
    fn test_saas_landing_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        let schema_sections = [
            "hero",
            "features",
            "pricing",
            "testimonials",
            "cta",
            "footer",
        ];
        for section_id in &schema_sections {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "Section '{}' should have at least 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_saas_landing_mobile_navigation() {
        let template = get_template("saas_landing").unwrap();
        assert!(
            template.html.contains("nav__hamburger"),
            "saas_landing missing hamburger button"
        );
        assert!(
            template.html.contains("nav__mobile"),
            "saas_landing missing mobile nav overlay"
        );
        assert!(
            template.html.contains("aria-controls"),
            "saas_landing hamburger missing aria-controls"
        );
    }

    #[test]
    fn test_saas_landing_token_driven_css() {
        let template = get_template("saas_landing").unwrap();
        // Foundation tokens must be present
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        // Semantic tokens must be present
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--hero-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    // ── Phase 6.2 Session 2: portfolio template validation tests ──────

    #[test]
    fn test_portfolio_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("portfolio").unwrap();
        let schema = slot_schema::get_template_schema("portfolio").unwrap();
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "portfolio HTML missing section anchor for schema section '{}'",
                section.section_id
            );
        }
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "portfolio template section '{}' not found in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_portfolio_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("portfolio").unwrap();
        let schema = slot_schema::get_template_schema("portfolio").unwrap();
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_placeholder = template.html.contains(&direct_upper);
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_placeholder = template.html.contains(&prefixed_upper);
                    assert!(
                        has_direct_attr || has_direct_placeholder || has_prefixed_attr || has_prefixed_placeholder,
                        "portfolio missing required slot '{}' (section '{}') — looked for '{}' or '{}'",
                        slot_name, section.section_id, slot_name, prefixed
                    );
                }
            }
        }
    }

    #[test]
    fn test_portfolio_zero_hardcoded_colors_outside_root() {
        let template = get_template("portfolio").unwrap();
        let html = template.html;
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let outside_root = &style_content[root_end..];
        let found = regex_lite_hex_scan(outside_root);
        assert!(
            found.is_empty(),
            "portfolio CSS has hardcoded hex colors outside :root: {:?}",
            found
        );
    }

    #[test]
    fn test_portfolio_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("portfolio").unwrap();
        let html = template.html;
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6;
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "portfolio references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_portfolio_responsive_breakpoints() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("768px"),
            "portfolio missing 768px breakpoint"
        );
        assert!(
            template.html.contains("1024px"),
            "portfolio missing 1024px breakpoint"
        );
    }

    #[test]
    fn test_portfolio_no_fixed_width_containers() {
        let template = get_template("portfolio").unwrap();
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                {
                    continue;
                }
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(
                            num <= 320.0,
                            "portfolio has fixed width: {}px — use max-width or responsive units",
                            num
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_portfolio_dark_mode_support() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "portfolio missing prefers-color-scheme dark media query"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "portfolio missing [data-theme='dark'] manual toggle support"
        );
    }

    #[test]
    fn test_portfolio_reduced_motion() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "portfolio missing prefers-reduced-motion support"
        );
    }

    #[test]
    fn test_portfolio_scroll_reveal() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("nexus-reveal"),
            "portfolio missing nexus-reveal scroll animation class"
        );
        assert!(
            template.html.contains("IntersectionObserver"),
            "portfolio missing IntersectionObserver for scroll reveals"
        );
        assert!(
            template.html.contains("is-visible"),
            "portfolio missing is-visible reveal class"
        );
    }

    #[test]
    fn test_portfolio_accessibility() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "portfolio missing skip-to-content link"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "portfolio missing aria-expanded on mobile nav"
        );
        assert!(
            template.html.contains("<main"),
            "portfolio missing <main> element"
        );
        assert!(
            template.html.contains("<nav"),
            "portfolio missing <nav> element"
        );
        assert!(
            template.html.contains("<footer"),
            "portfolio missing <footer> element"
        );
        assert!(
            template.html.contains("focus-visible"),
            "portfolio missing :focus-visible styles"
        );
    }

    #[test]
    fn test_portfolio_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("portfolio");
        assert_eq!(
            palettes.len(),
            4,
            "expected 4 palette presets for portfolio"
        );
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "editorial".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(
                token_set.is_some(),
                "Palette '{}' failed to produce a TokenSet",
                palette.id
            );
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' CSS missing --color-primary",
                palette.id
            );
        }
    }

    #[test]
    fn test_portfolio_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        // Portfolio sections with layout variants per variant.rs
        let sections_with_variants = ["projects", "about", "skills", "contact"];
        for section_id in &sections_with_variants {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "Portfolio section '{}' should have at least 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_portfolio_mobile_navigation() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("nav__hamburger"),
            "portfolio missing hamburger button"
        );
        assert!(
            template.html.contains("nav__mobile"),
            "portfolio missing mobile nav overlay"
        );
        assert!(
            template.html.contains("aria-controls"),
            "portfolio hamburger missing aria-controls"
        );
    }

    #[test]
    fn test_portfolio_token_driven_css() {
        let template = get_template("portfolio").unwrap();
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--hero-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    #[test]
    fn test_portfolio_project_cards_accessible() {
        let template = get_template("portfolio").unwrap();
        // Project cards should be articles (semantic) and keyboard-focusable
        assert!(
            template.html.contains("<article class=\"project-card\""),
            "portfolio project cards should use <article> elements"
        );
        assert!(
            template.html.contains("tabindex=\"0\""),
            "portfolio project cards should be keyboard-focusable"
        );
    }

    #[test]
    fn test_portfolio_transparent_nav_scroll() {
        let template = get_template("portfolio").unwrap();
        assert!(
            template.html.contains("is-scrolled"),
            "portfolio missing is-scrolled nav state class"
        );
    }

    // ── Phase 6.2 Session 3: ecommerce template validation tests ──────

    #[test]
    fn test_ecommerce_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("ecommerce").unwrap();
        let schema = slot_schema::get_template_schema("ecommerce").unwrap();
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "ecommerce HTML missing section anchor for '{}'",
                section.section_id
            );
        }
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "ecommerce template section '{}' not in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_ecommerce_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("ecommerce").unwrap();
        let schema = slot_schema::get_template_schema("ecommerce").unwrap();
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_ph = template.html.contains(&direct_upper);
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_ph = template.html.contains(&prefixed_upper);
                    assert!(
                        has_direct_attr || has_direct_ph || has_prefixed_attr || has_prefixed_ph,
                        "ecommerce missing required slot '{}' (section '{}') — looked for '{}' or '{}'",
                        slot_name, section.section_id, slot_name, prefixed
                    );
                }
            }
        }
    }

    #[test]
    fn test_ecommerce_zero_hardcoded_colors_outside_root() {
        let template = get_template("ecommerce").unwrap();
        let html = template.html;
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let outside_root = &style_content[root_end..];
        let found = regex_lite_hex_scan(outside_root);
        assert!(
            found.is_empty(),
            "ecommerce CSS has hardcoded hex colors outside :root: {:?}",
            found
        );
    }

    #[test]
    fn test_ecommerce_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("ecommerce").unwrap();
        let html = template.html;
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6;
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "ecommerce references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_ecommerce_responsive_breakpoints() {
        let template = get_template("ecommerce").unwrap();
        assert!(template.html.contains("768px"), "ecommerce missing 768px");
        assert!(template.html.contains("1024px"), "ecommerce missing 1024px");
    }

    #[test]
    fn test_ecommerce_no_fixed_width_containers() {
        let template = get_template("ecommerce").unwrap();
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                    || after_width.starts_with("none")
                {
                    continue;
                }
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(num <= 320.0, "ecommerce has fixed width: {}px", num);
                    }
                }
            }
        }
    }

    #[test]
    fn test_ecommerce_dark_mode_support() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "ecommerce missing prefers-color-scheme dark"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "ecommerce missing [data-theme='dark']"
        );
    }

    #[test]
    fn test_ecommerce_reduced_motion() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "ecommerce missing prefers-reduced-motion"
        );
    }

    #[test]
    fn test_ecommerce_scroll_reveal() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("nexus-reveal"),
            "missing nexus-reveal"
        );
        assert!(
            template.html.contains("IntersectionObserver"),
            "missing IntersectionObserver"
        );
        assert!(template.html.contains("is-visible"), "missing is-visible");
    }

    #[test]
    fn test_ecommerce_accessibility() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "ecommerce missing skip-to-content"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "ecommerce missing aria-expanded"
        );
        assert!(template.html.contains("<main"), "ecommerce missing <main>");
        assert!(template.html.contains("<nav"), "ecommerce missing <nav>");
        assert!(
            template.html.contains("<footer"),
            "ecommerce missing <footer>"
        );
        assert!(
            template.html.contains("focus-visible"),
            "ecommerce missing :focus-visible"
        );
    }

    #[test]
    fn test_ecommerce_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("ecommerce");
        assert_eq!(
            palettes.len(),
            4,
            "expected 4 palette presets for ecommerce"
        );
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "modern".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(
                token_set.is_some(),
                "Palette '{}' failed to produce a TokenSet",
                palette.id
            );
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' missing --color-primary",
                palette.id
            );
        }
    }

    #[test]
    fn test_ecommerce_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        let sections_with_variants = ["categories", "products", "reviews", "newsletter"];
        for section_id in &sections_with_variants {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "Ecommerce section '{}' should have >= 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_ecommerce_mobile_navigation() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("nav__hamburger"),
            "missing hamburger"
        );
        assert!(template.html.contains("nav__mobile"), "missing mobile nav");
        assert!(
            template.html.contains("aria-controls"),
            "missing aria-controls"
        );
    }

    #[test]
    fn test_ecommerce_token_driven_css() {
        let template = get_template("ecommerce").unwrap();
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--hero-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    #[test]
    fn test_ecommerce_product_cards_hover_image() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("product-card__image--hover"),
            "ecommerce missing hover image swap structure"
        );
        assert!(
            template.html.contains("product-card__image-wrap"),
            "ecommerce missing image wrapper for hover"
        );
    }

    #[test]
    fn test_ecommerce_sticky_mobile_cta() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("sticky-cta"),
            "ecommerce missing sticky mobile CTA structure"
        );
        // CSS must have the fixed positioning
        assert!(
            template.html.contains("position: fixed") && template.html.contains("sticky-cta"),
            "ecommerce sticky CTA missing fixed positioning in CSS"
        );
    }

    #[test]
    fn test_ecommerce_star_ratings_accessible() {
        let template = get_template("ecommerce").unwrap();
        assert!(
            template.html.contains("star-rating"),
            "ecommerce missing star-rating class"
        );
        assert!(
            template.html.contains("aria-label") && template.html.contains("out of 5 stars"),
            "ecommerce star ratings missing accessible aria-label"
        );
        assert!(
            template.html.contains("<svg") && template.html.contains("star-rating"),
            "ecommerce should use SVG for star ratings, not unicode"
        );
    }

    // ── Phase 6.2 Session 4: dashboard template validation tests ──────

    #[test]
    fn test_dashboard_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("dashboard").unwrap();
        let schema = slot_schema::get_template_schema("dashboard").unwrap();
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "dashboard HTML missing section anchor for '{}'",
                section.section_id
            );
        }
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "dashboard template section '{}' not in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_dashboard_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("dashboard").unwrap();
        let schema = slot_schema::get_template_schema("dashboard").unwrap();
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_ph = template.html.contains(&direct_upper);
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_ph = template.html.contains(&prefixed_upper);
                    // Also check header_ prefix for user_name etc
                    let header_prefixed = format!("header_{}", slot_name);
                    let has_header_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{header_prefixed}\""));
                    let header_upper = format!("{{{{{}}}}}", header_prefixed.to_uppercase());
                    let has_header_ph = template.html.contains(&header_upper);
                    // Footer-prefixed
                    let footer_prefixed = format!("footer_{}", slot_name);
                    let has_footer_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{footer_prefixed}\""));
                    let footer_upper = format!("{{{{{}}}}}", footer_prefixed.to_uppercase());
                    let has_footer_ph = template.html.contains(&footer_upper);
                    assert!(
                        has_direct_attr
                            || has_direct_ph
                            || has_prefixed_attr
                            || has_prefixed_ph
                            || has_header_attr
                            || has_header_ph
                            || has_footer_attr
                            || has_footer_ph,
                        "dashboard missing required slot '{}' (section '{}')",
                        slot_name,
                        section.section_id
                    );
                }
            }
        }
    }

    #[test]
    fn test_dashboard_zero_hardcoded_colors_outside_root() {
        let template = get_template("dashboard").unwrap();
        let html = template.html;
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let outside_root = &style_content[root_end..];
        let found = regex_lite_hex_scan(outside_root);
        assert!(
            found.is_empty(),
            "dashboard CSS has hardcoded hex colors outside :root: {:?}",
            found
        );
    }

    #[test]
    fn test_dashboard_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("dashboard").unwrap();
        let html = template.html;
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
            "code-font-family",
            // Dashboard-specific custom props
            "sidebar-width",
            "sidebar-collapsed-width",
            "status-active",
            "status-warning",
            "status-error",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6;
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "dashboard references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_dashboard_responsive_breakpoints() {
        let template = get_template("dashboard").unwrap();
        assert!(template.html.contains("768px"), "dashboard missing 768px");
        assert!(template.html.contains("1024px"), "dashboard missing 1024px");
    }

    #[test]
    fn test_dashboard_no_fixed_width_containers() {
        let template = get_template("dashboard").unwrap();
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                    || after_width.starts_with("none")
                {
                    continue;
                }
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(num <= 320.0, "dashboard has fixed width: {}px", num);
                    }
                }
            }
        }
    }

    #[test]
    fn test_dashboard_dark_mode_support() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "dashboard missing prefers-color-scheme dark"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "dashboard missing [data-theme='dark']"
        );
    }

    #[test]
    fn test_dashboard_reduced_motion() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "dashboard missing prefers-reduced-motion"
        );
    }

    #[test]
    fn test_dashboard_scroll_reveal() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("nexus-reveal"),
            "missing nexus-reveal"
        );
        assert!(
            template.html.contains("IntersectionObserver"),
            "missing IntersectionObserver"
        );
        assert!(template.html.contains("is-visible"), "missing is-visible");
    }

    #[test]
    fn test_dashboard_accessibility() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "dashboard missing skip-to-content"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "dashboard missing aria-expanded"
        );
        assert!(
            template.html.contains("<nav") || template.html.contains("role=\"navigation\""),
            "dashboard missing nav element"
        );
        assert!(
            template.html.contains("<footer") || template.html.contains("dash-footer"),
            "dashboard missing footer"
        );
        assert!(
            template.html.contains("focus-visible"),
            "dashboard missing :focus-visible"
        );
    }

    #[test]
    fn test_dashboard_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("dashboard");
        assert_eq!(
            palettes.len(),
            4,
            "expected 4 palette presets for dashboard"
        );
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "tech".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(
                token_set.is_some(),
                "Palette '{}' failed to produce a TokenSet",
                palette.id
            );
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' missing --color-primary",
                palette.id
            );
        }
    }

    #[test]
    fn test_dashboard_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        let sections_with_variants = ["sidebar", "stats", "charts", "data_table"];
        for section_id in &sections_with_variants {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "Dashboard section '{}' should have >= 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_dashboard_token_driven_css() {
        let template = get_template("dashboard").unwrap();
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    #[test]
    fn test_dashboard_sidebar_collapse() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("sidebar-collapsed"),
            "dashboard missing sidebar-collapsed class"
        );
        assert!(
            template.html.contains("grid-template-columns"),
            "dashboard missing grid-template-columns for sidebar layout"
        );
        assert!(
            template.html.contains("sidebar__toggle"),
            "dashboard missing sidebar toggle button"
        );
    }

    #[test]
    fn test_dashboard_skeleton_loading() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("@keyframes shimmer"),
            "dashboard missing @keyframes shimmer animation"
        );
        assert!(
            template.html.contains("skeleton"),
            "dashboard missing skeleton loading classes"
        );
    }

    #[test]
    fn test_dashboard_data_table_accessible() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("<table"),
            "dashboard missing <table> element"
        );
        assert!(
            template.html.contains("<thead"),
            "dashboard missing <thead> element"
        );
        assert!(
            template.html.contains("scope=\"col\""),
            "dashboard table missing th scope='col'"
        );
        assert!(
            template.html.contains("aria-sort"),
            "dashboard table missing aria-sort on sortable columns"
        );
    }

    #[test]
    fn test_dashboard_status_indicators_accessible() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("status-badge__dot"),
            "dashboard missing status dot indicators"
        );
        // Status badges must have text labels (not color-alone)
        assert!(
            template.html.contains("status-badge--active")
                && (template.html.contains(">Active<") || template.html.contains(">active<")),
            "dashboard status indicators must pair color with text label"
        );
    }

    #[test]
    fn test_dashboard_mobile_drawer() {
        let template = get_template("dashboard").unwrap();
        assert!(
            template.html.contains("aria-expanded"),
            "dashboard mobile drawer missing aria-expanded"
        );
        assert!(
            template.html.contains("sidebar-backdrop"),
            "dashboard missing sidebar backdrop for mobile drawer"
        );
        assert!(
            template.html.contains("position: fixed"),
            "dashboard mobile sidebar missing fixed positioning"
        );
    }

    // ── Phase 6.2 Session 5: local_business template validation tests ─

    #[test]
    fn test_local_business_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("local_business").unwrap();
        let schema = slot_schema::get_template_schema("local_business").unwrap();
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "local_business HTML missing section anchor for '{}'",
                section.section_id
            );
        }
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "local_business template section '{}' not in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_local_business_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("local_business").unwrap();
        let schema = slot_schema::get_template_schema("local_business").unwrap();
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_ph = template.html.contains(&direct_upper);
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_ph = template.html.contains(&prefixed_upper);
                    // Also check hero_ prefix and footer_ prefix and hours_ prefix
                    let hero_prefixed = format!("hero_{}", slot_name);
                    let has_hero_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{hero_prefixed}\""));
                    let footer_prefixed = format!("footer_{}", slot_name);
                    let has_footer_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{footer_prefixed}\""));
                    let footer_upper = format!("{{{{{}}}}}", footer_prefixed.to_uppercase());
                    let has_footer_ph = template.html.contains(&footer_upper);
                    let hours_prefixed = format!("hours_{}", slot_name);
                    let has_hours_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{hours_prefixed}\""));
                    let hours_upper = format!("{{{{{}}}}}", hours_prefixed.to_uppercase());
                    let has_hours_ph = template.html.contains(&hours_upper);
                    assert!(
                        has_direct_attr
                            || has_direct_ph
                            || has_prefixed_attr
                            || has_prefixed_ph
                            || has_hero_attr
                            || has_footer_attr
                            || has_footer_ph
                            || has_hours_attr
                            || has_hours_ph,
                        "local_business missing required slot '{}' (section '{}')",
                        slot_name,
                        section.section_id
                    );
                }
            }
        }
    }

    #[test]
    fn test_local_business_zero_hardcoded_colors_outside_root() {
        let template = get_template("local_business").unwrap();
        let html = template.html;
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let outside_root = &style_content[root_end..];
        let found = regex_lite_hex_scan(outside_root);
        assert!(
            found.is_empty(),
            "local_business CSS has hardcoded hex colors outside :root: {:?}",
            found
        );
    }

    #[test]
    fn test_local_business_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("local_business").unwrap();
        let html = template.html;
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6;
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "local_business references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_local_business_responsive_breakpoints() {
        let template = get_template("local_business").unwrap();
        assert!(template.html.contains("768px"), "missing 768px breakpoint");
        assert!(
            template.html.contains("1024px"),
            "missing 1024px breakpoint"
        );
    }

    #[test]
    fn test_local_business_no_fixed_width_containers() {
        let template = get_template("local_business").unwrap();
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                    || after_width.starts_with("none")
                {
                    continue;
                }
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(num <= 320.0, "local_business has fixed width: {}px", num);
                    }
                }
            }
        }
    }

    #[test]
    fn test_local_business_dark_mode_support() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "missing prefers-color-scheme dark"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "missing [data-theme='dark']"
        );
    }

    #[test]
    fn test_local_business_reduced_motion() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "missing prefers-reduced-motion"
        );
    }

    #[test]
    fn test_local_business_scroll_reveal() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("nexus-reveal"),
            "missing nexus-reveal"
        );
        assert!(
            template.html.contains("IntersectionObserver"),
            "missing IntersectionObserver"
        );
        assert!(template.html.contains("is-visible"), "missing is-visible");
    }

    #[test]
    fn test_local_business_accessibility() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "missing skip-to-content"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "missing aria-expanded"
        );
        assert!(template.html.contains("<main"), "missing <main>");
        assert!(template.html.contains("<nav"), "missing <nav>");
        assert!(template.html.contains("<footer"), "missing <footer>");
        assert!(
            template.html.contains("focus-visible"),
            "missing :focus-visible"
        );
    }

    #[test]
    fn test_local_business_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("local_business");
        assert_eq!(palettes.len(), 4, "expected 4 palette presets");
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "editorial".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(token_set.is_some(), "Palette '{}' failed", palette.id);
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' missing --color-primary",
                palette.id
            );
        }
    }

    #[test]
    fn test_local_business_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        let sections_with_variants = ["services", "gallery", "hours"];
        for section_id in &sections_with_variants {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "local_business section '{}' should have >= 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_local_business_mobile_navigation() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("nav__hamburger"),
            "missing hamburger"
        );
        assert!(template.html.contains("nav__mobile"), "missing mobile nav");
        assert!(
            template.html.contains("aria-controls"),
            "missing aria-controls"
        );
    }

    #[test]
    fn test_local_business_token_driven_css() {
        let template = get_template("local_business").unwrap();
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--hero-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    #[test]
    fn test_local_business_gallery_lightbox() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("gallery-lightbox"),
            "missing lightbox overlay structure"
        );
        assert!(
            template.html.contains("is-active"),
            "missing .is-active lightbox state"
        );
        assert!(
            template.html.contains("gallery-lightbox__close"),
            "missing lightbox close button"
        );
    }

    #[test]
    fn test_local_business_sticky_booking_cta() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("sticky-cta"),
            "missing sticky booking CTA"
        );
        assert!(
            template.html.contains("position: fixed") && template.html.contains("sticky-cta"),
            "sticky CTA missing fixed positioning"
        );
    }

    #[test]
    fn test_local_business_star_ratings_accessible() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("testimonial-card__stars"),
            "missing star rating class"
        );
        assert!(
            template.html.contains("aria-label") && template.html.contains("out of 5 stars"),
            "star ratings missing accessible aria-label"
        );
        assert!(
            template.html.contains("<svg") && template.html.contains("testimonial-card__stars"),
            "should use SVG for star ratings"
        );
    }

    #[test]
    fn test_local_business_contact_semantic() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("<address"),
            "missing <address> semantic element"
        );
        assert!(
            template.html.contains("<time"),
            "missing <time> semantic element for hours"
        );
    }

    #[test]
    fn test_local_business_hours_structured() {
        let template = get_template("local_business").unwrap();
        assert!(
            template.html.contains("hours-table") || template.html.contains("<dl"),
            "hours section should use table or dl for structured hours"
        );
        assert!(
            template.html.contains("tel:"),
            "phone number should be a clickable tel: link"
        );
    }

    // ── Phase 6.2 Session 6: docs_site template validation tests ──────

    #[test]
    fn test_docs_site_sections_match_slot_schema() {
        use crate::slot_schema;
        let template = get_template("docs_site").unwrap();
        let schema = slot_schema::get_template_schema("docs_site").unwrap();
        for section in &schema.sections {
            let attr = format!("data-nexus-section=\"{}\"", section.section_id);
            assert!(
                template.html.contains(&attr),
                "docs_site HTML missing section anchor for '{}'",
                section.section_id
            );
        }
        for section_id in template.sections {
            assert!(
                schema.sections.iter().any(|s| s.section_id == *section_id),
                "docs_site template section '{}' not in slot schema",
                section_id
            );
        }
    }

    #[test]
    fn test_docs_site_has_all_required_slot_placeholders() {
        use crate::slot_schema;
        let template = get_template("docs_site").unwrap();
        let schema = slot_schema::get_template_schema("docs_site").unwrap();
        for section in &schema.sections {
            for (slot_name, constraint) in &section.slots {
                if constraint.required {
                    let has_direct_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{slot_name}\""));
                    let direct_upper = format!("{{{{{}}}}}", slot_name.to_uppercase());
                    let has_direct_ph = template.html.contains(&direct_upper);
                    let prefixed = format!("{}_{}", section.section_id, slot_name);
                    let has_prefixed_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{prefixed}\""));
                    let prefixed_upper = format!("{{{{{}}}}}", prefixed.to_uppercase());
                    let has_prefixed_ph = template.html.contains(&prefixed_upper);
                    // footer_links special case
                    let footer_prefixed = format!("footer_{}", slot_name);
                    let has_footer_attr = template
                        .html
                        .contains(&format!("data-nexus-slot=\"{footer_prefixed}\""));
                    assert!(
                        has_direct_attr
                            || has_direct_ph
                            || has_prefixed_attr
                            || has_prefixed_ph
                            || has_footer_attr,
                        "docs_site missing required slot '{}' (section '{}')",
                        slot_name,
                        section.section_id
                    );
                }
            }
        }
    }

    #[test]
    fn test_docs_site_zero_hardcoded_colors_outside_root() {
        let template = get_template("docs_site").unwrap();
        let html = template.html;
        let style_start = html.find("<style>").unwrap_or(0);
        let style_end = html.find("</style>").unwrap_or(html.len());
        let style_content = &html[style_start..style_end];
        let root_start = style_content.find(":root {").unwrap_or(0);
        let mut brace_depth = 0;
        let mut root_end = root_start;
        for (i, ch) in style_content[root_start..].char_indices() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        root_end = root_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let outside_root = &style_content[root_end..];
        let found = regex_lite_hex_scan(outside_root);
        assert!(
            found.is_empty(),
            "docs_site CSS has hardcoded hex colors outside :root: {:?}",
            found
        );
    }

    #[test]
    fn test_docs_site_token_references_valid() {
        use crate::tokens::{FOUNDATION_TOKEN_NAMES, SEMANTIC_TOKEN_NAMES};
        let template = get_template("docs_site").unwrap();
        let html = template.html;
        let mut pos = 0;
        let compat_aliases = [
            "primary",
            "primary-light",
            "primary-dark",
            "accent",
            "bg",
            "text",
            "text-muted",
            "heading-font-family",
            "body-font-family",
            "code-font-family",
            // Docs-specific tokens
            "code-bg",
            "code-text",
            "code-header-bg",
            "color-warning",
            "color-success",
            "docs-sidebar-width",
            "docs-toc-width",
            "callout-color",
        ];
        while let Some(start) = html[pos..].find("var(--") {
            let abs_start = pos + start + 6;
            if let Some(end_paren) = html[abs_start..].find(')') {
                let token_name_raw = &html[abs_start..abs_start + end_paren];
                let token_name = token_name_raw
                    .split(',')
                    .next()
                    .unwrap_or(token_name_raw)
                    .trim();
                let is_foundation = FOUNDATION_TOKEN_NAMES.contains(&token_name);
                let is_semantic = SEMANTIC_TOKEN_NAMES.contains(&token_name);
                let is_compat = compat_aliases.contains(&token_name);
                assert!(
                    is_foundation || is_semantic || is_compat,
                    "docs_site references unknown token: var(--{})",
                    token_name
                );
                pos = abs_start + end_paren;
            } else {
                break;
            }
        }
    }

    #[test]
    fn test_docs_site_responsive_breakpoints() {
        let template = get_template("docs_site").unwrap();
        assert!(template.html.contains("768px"), "missing 768px breakpoint");
        assert!(
            template.html.contains("1024px") || template.html.contains("1279px"),
            "missing desktop breakpoint"
        );
    }

    #[test]
    fn test_docs_site_no_fixed_width_containers() {
        let template = get_template("docs_site").unwrap();
        let style_start = template.html.find("<style>").unwrap_or(0);
        let style_end = template
            .html
            .find("</style>")
            .unwrap_or(template.html.len());
        let style = &template.html[style_start..style_end];
        for line in style.lines() {
            let trimmed = line.trim();
            if let Some(w_pos) = trimmed.find("width:") {
                let after_width = &trimmed[w_pos + 6..].trim_start();
                if trimmed[..w_pos].ends_with("max-") || trimmed[..w_pos].ends_with("min-") {
                    continue;
                }
                if after_width.contains('%')
                    || after_width.contains("rem")
                    || after_width.contains("em")
                    || after_width.contains("vw")
                    || after_width.contains("var(")
                    || after_width.starts_with("0")
                    || after_width.starts_with("100%")
                    || after_width.starts_with("auto")
                    || after_width.starts_with("none")
                {
                    continue;
                }
                if let Some(px_pos) = after_width.find("px") {
                    let num_str = after_width[..px_pos].trim();
                    if let Ok(num) = num_str.parse::<f64>() {
                        assert!(num <= 320.0, "docs_site has fixed width: {}px", num);
                    }
                }
            }
        }
    }

    #[test]
    fn test_docs_site_dark_mode_support() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("prefers-color-scheme: dark"),
            "missing prefers-color-scheme"
        );
        assert!(
            template.html.contains("[data-theme=\"dark\"]"),
            "missing [data-theme='dark']"
        );
    }

    #[test]
    fn test_docs_site_reduced_motion() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("prefers-reduced-motion: reduce"),
            "missing prefers-reduced-motion"
        );
    }

    #[test]
    fn test_docs_site_accessibility() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("skip-link") || template.html.contains("Skip to"),
            "missing skip-to-content"
        );
        assert!(
            template.html.contains("aria-expanded"),
            "missing aria-expanded"
        );
        assert!(template.html.contains("<nav"), "missing <nav>");
        assert!(
            template.html.contains("<footer") || template.html.contains("docs-footer"),
            "missing footer"
        );
        assert!(
            template.html.contains("focus-visible"),
            "missing :focus-visible"
        );
    }

    #[test]
    fn test_docs_site_palette_presets_produce_valid_css() {
        use crate::variant::{palettes_for_template, MotionProfile, VariantSelection};
        use std::collections::HashMap;
        let palettes = palettes_for_template("docs_site");
        assert_eq!(palettes.len(), 4, "expected 4 palette presets");
        for palette in &palettes {
            let selection = VariantSelection {
                palette_id: palette.id.to_string(),
                typography_id: "tech".to_string(),
                layout: HashMap::new(),
                motion: MotionProfile::Subtle,
            };
            let token_set = selection.to_token_set();
            assert!(token_set.is_some(), "Palette '{}' failed", palette.id);
            let css = token_set.unwrap().to_css();
            assert!(
                css.contains("--color-primary:"),
                "Palette '{}' missing --color-primary",
                palette.id
            );
        }
    }

    #[test]
    fn test_docs_site_layout_variants_defined() {
        use crate::variant::layouts_for_section;
        let sections_with_variants = ["sidebar_nav", "content", "code_blocks"];
        for section_id in &sections_with_variants {
            let variants = layouts_for_section(section_id);
            assert!(
                variants.len() >= 2,
                "docs_site section '{}' should have >= 2 layout variants, got {}",
                section_id,
                variants.len()
            );
        }
    }

    #[test]
    fn test_docs_site_mobile_navigation() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("docs-sidebar__mobile-toggle"),
            "missing mobile toggle"
        );
        assert!(
            template.html.contains("sidebar-backdrop"),
            "missing sidebar backdrop"
        );
        assert!(
            template.html.contains("aria-controls"),
            "missing aria-controls"
        );
    }

    #[test]
    fn test_docs_site_token_driven_css() {
        let template = get_template("docs_site").unwrap();
        assert!(template.html.contains("--color-primary:"));
        assert!(template.html.contains("--color-bg:"));
        assert!(template.html.contains("--font-heading:"));
        assert!(template.html.contains("--text-4xl:"));
        assert!(template.html.contains("--space-section:"));
        assert!(template.html.contains("--radius-xl:"));
        assert!(template.html.contains("--shadow-lg:"));
        assert!(template.html.contains("--duration-fast:"));
        assert!(template.html.contains("--ease-default:"));
        assert!(template.html.contains("--btn-bg:"));
        assert!(template.html.contains("--card-bg:"));
        assert!(template.html.contains("--nav-bg:"));
        assert!(template.html.contains("--footer-bg:"));
        assert!(template.html.contains("--section-bg:"));
    }

    #[test]
    fn test_docs_site_three_column_layout() {
        let template = get_template("docs_site").unwrap();
        // 3-column at wide, 2-column at desktop, 1-column at mobile
        assert!(
            template.html.contains("docs-toc-width"),
            "missing 3rd column TOC width"
        );
        // Check responsive collapse
        assert!(
            template.html.contains("grid-template-columns")
                && template.html.contains("docs-sidebar-width"),
            "missing grid-template-columns with sidebar width"
        );
    }

    #[test]
    fn test_docs_site_content_width_constrained() {
        let template = get_template("docs_site").unwrap();
        // Content area should have max-width <= 50rem
        assert!(
            template.html.contains("max-width: 48rem")
                || template.html.contains("max-width: 50rem"),
            "docs content area should be width-constrained to ~48-50rem"
        );
    }

    #[test]
    fn test_docs_site_code_blocks_always_dark() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("--code-bg:") && template.html.contains("--code-text:"),
            "docs missing code block dark tokens"
        );
        assert!(
            template.html.contains("docs-code-block"),
            "docs missing code block class"
        );
    }

    #[test]
    fn test_docs_site_code_copy_button() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("docs-code-block__copy"),
            "docs missing code copy button"
        );
        assert!(
            template.html.contains("Copy code to clipboard"),
            "docs copy button missing aria-label"
        );
        assert!(
            template.html.contains("navigator.clipboard") || template.html.contains("execCommand"),
            "docs missing clipboard JS"
        );
    }

    #[test]
    fn test_docs_site_callout_boxes() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("docs-callout--note"),
            "missing note callout"
        );
        assert!(
            template.html.contains("docs-callout--warning"),
            "missing warning callout"
        );
        assert!(
            template.html.contains("border-left") && template.html.contains("callout-color"),
            "callouts should have border-left with callout-color"
        );
    }

    #[test]
    fn test_docs_site_heading_anchors() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("heading-anchor"),
            "docs missing heading anchor elements"
        );
        assert!(
            template.html.contains("Link to this section"),
            "heading anchors missing aria-label"
        );
    }

    #[test]
    fn test_docs_site_toc_structure() {
        let template = get_template("docs_site").unwrap();
        assert!(template.html.contains("docs-toc"), "missing right TOC");
        assert!(
            template.html.contains("On this page"),
            "missing 'On this page' TOC title"
        );
        assert!(
            template.html.contains("docs-toc__links"),
            "missing TOC links container"
        );
        assert!(
            template.html.contains("is-active"),
            "missing is-active class for TOC tracking"
        );
    }

    #[test]
    fn test_docs_site_breadcrumbs_accessible() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("aria-label=\"Breadcrumb\""),
            "missing breadcrumb aria-label"
        );
        assert!(
            template.html.contains("aria-current=\"page\""),
            "missing aria-current on current breadcrumb"
        );
    }

    #[test]
    fn test_docs_site_sidebar_collapsible() {
        let template = get_template("docs_site").unwrap();
        assert!(
            template.html.contains("<details") && template.html.contains("<summary"),
            "docs sidebar should use <details>/<summary> for collapsible sections"
        );
    }
}
