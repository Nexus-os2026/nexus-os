use web_builder_agent::codegen::{generate_website, FileChange};
use web_builder_agent::interpreter::{
    interpret, AnimationSpec, ComponentSpec, Framework, PageSpec, SectionKind, SectionSpec,
    ThemeSpec, ThreeDSpec, WebsiteSpec,
};
use web_builder_agent::styles::generate_theme;
use web_builder_agent::threejs::generate_3d_scene;

#[test]
fn test_interpret_description() {
    let spec = interpret("Landing page for coffee shop with a menu and contact form")
        .expect("description should be interpreted");

    assert!(!spec.pages.is_empty());
    let home = &spec.pages[0];

    assert!(
        home.sections
            .iter()
            .any(|section| section.kind == SectionKind::Hero),
        "expected hero section"
    );
    assert!(
        home.sections
            .iter()
            .any(|section| section.kind == SectionKind::Menu),
        "expected menu section"
    );
    assert!(
        home.sections
            .iter()
            .any(|section| section.kind == SectionKind::Contact),
        "expected contact section"
    );
    assert!(
        spec.theme.mood.contains("warm")
            || spec
                .theme
                .colors
                .iter()
                .any(|color| color == "#A06A42" || color == "#E39C5A"),
        "expected warm color direction"
    );
}

#[test]
fn test_threejs_generation() {
    let spec = ThreeDSpec {
        model: "rotating-cube".to_string(),
        animation: "spin".to_string(),
        position: "center".to_string(),
    };

    let component = generate_3d_scene(&spec);
    assert!(component.contains("@react-three/fiber"));
    assert!(component.contains("useFrame"));
    assert!(component.contains("rotation"));
}

#[test]
fn test_full_website_generation() {
    let spec = WebsiteSpec {
        pages: vec![
            page(
                "Home",
                vec![
                    SectionKind::Hero,
                    SectionKind::Features,
                    SectionKind::Contact,
                ],
            ),
            page(
                "Menu",
                vec![SectionKind::Header, SectionKind::Menu, SectionKind::Footer],
            ),
            page(
                "Contact",
                vec![
                    SectionKind::Header,
                    SectionKind::Contact,
                    SectionKind::Footer,
                ],
            ),
        ],
        theme: ThemeSpec {
            colors: vec!["#07010F".to_string(), "#00F5D4".to_string()],
            fonts: vec![
                "Audiowide".to_string(),
                "Space Grotesk".to_string(),
                "JetBrains Mono".to_string(),
            ],
            mood: "cyberpunk".to_string(),
        },
        components: vec![
            ComponentSpec {
                name: "Header".to_string(),
                props_schema: "{}".to_string(),
            },
            ComponentSpec {
                name: "Footer".to_string(),
                props_schema: "{}".to_string(),
            },
        ],
        three_d_elements: vec![ThreeDSpec {
            model: "coffee-cup".to_string(),
            animation: "slow-rotate".to_string(),
            position: "hero-right".to_string(),
        }],
        animations: vec![AnimationSpec {
            trigger: "on-load".to_string(),
            animation_type: "fade-in".to_string(),
            target: "hero".to_string(),
        }],
        responsive: true,
        framework: Framework::React,
    };

    let files = generate_website(&spec).expect("website generation should succeed");

    assert!(has_file(&files, "package.json"));
    assert!(has_file(&files, "src/App.tsx"));
    assert!(has_file(&files, "src/pages/HomePage.tsx"));
    assert!(has_file(&files, "src/pages/MenuPage.tsx"));
    assert!(has_file(&files, "src/pages/ContactPage.tsx"));
    assert!(has_file(&files, "tailwind.config.ts"));

    for (path, content) in created_files(&files) {
        if path.ends_with(".ts") || path.ends_with(".tsx") {
            assert!(
                !content.contains(": any") && !content.contains("<any>"),
                "expected generated TypeScript without any in {path}"
            );
        }
    }
}

#[test]
fn test_theme_from_mood() {
    let theme = generate_theme("cyberpunk", None);

    assert!(
        theme.tokens.background.starts_with("#07") || theme.tokens.background == "#07010F",
        "expected dark cyberpunk background"
    );
    assert!(
        theme.tokens.accent == "#00F5D4" || theme.tokens.accent == "#FF00A8",
        "expected neon accent"
    );
    assert!(theme.tokens.font_mono.contains("Mono"));
    assert!(theme.tailwind_config.contains("accent"));
}

fn page(name: &str, kinds: Vec<SectionKind>) -> PageSpec {
    PageSpec {
        name: name.to_string(),
        layout: "landing".to_string(),
        sections: kinds
            .into_iter()
            .map(|kind| SectionSpec {
                template_id: template_for(&kind).to_string(),
                kind,
                content: "Generated content".to_string(),
            })
            .collect(),
        content: format!("{name} page"),
    }
}

fn template_for(kind: &SectionKind) -> &'static str {
    match kind {
        SectionKind::Header => "nav-fixed",
        SectionKind::Hero => "hero-3d-product",
        SectionKind::Features => "features-card-grid",
        SectionKind::Testimonials => "testimonials-grid",
        SectionKind::Pricing => "pricing-tiered",
        SectionKind::Menu => "menu-two-column",
        SectionKind::Contact => "contact-split-form",
        SectionKind::Footer => "footer-newsletter",
        SectionKind::Custom(_) => "features-card-grid",
    }
}

fn has_file(changes: &[FileChange], path: &str) -> bool {
    changes.iter().any(|change| match change {
        FileChange::Create(candidate, _) => candidate == path,
        FileChange::Modify(candidate, _, _) => candidate == path,
        FileChange::Delete(candidate) => candidate == path,
    })
}

fn created_files(changes: &[FileChange]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for change in changes {
        if let FileChange::Create(path, content) = change {
            files.push((path.clone(), content.clone()));
        }
    }
    files
}
