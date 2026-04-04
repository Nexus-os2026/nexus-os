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
}
