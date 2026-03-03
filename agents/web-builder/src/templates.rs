use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
