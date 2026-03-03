use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Framework {
    React,
    Vue,
    StaticHtml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSpec {
    pub name: String,
    pub layout: String,
    pub sections: Vec<SectionSpec>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSpec {
    pub kind: SectionKind,
    pub template_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionKind {
    Header,
    Hero,
    Features,
    Testimonials,
    Pricing,
    Menu,
    Contact,
    Footer,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSpec {
    pub colors: Vec<String>,
    pub fonts: Vec<String>,
    pub mood: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub name: String,
    pub props_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreeDSpec {
    pub model: String,
    pub animation: String,
    pub position: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationSpec {
    pub trigger: String,
    pub animation_type: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsiteSpec {
    pub pages: Vec<PageSpec>,
    pub theme: ThemeSpec,
    pub components: Vec<ComponentSpec>,
    pub three_d_elements: Vec<ThreeDSpec>,
    pub animations: Vec<AnimationSpec>,
    pub responsive: bool,
    pub framework: Framework,
}

pub struct DesignInterpreter {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
}

impl Default for DesignInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignInterpreter {
    pub fn new() -> Self {
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new());
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 4_000,
        };
        Self { gateway, runtime }
    }

    pub fn interpret(&mut self, description: &str) -> Result<WebsiteSpec, AgentError> {
        let prompt =
            format!("Convert this website request into a concise architecture plan: {description}");
        let _ = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 200, "mock-1")?;

        Ok(infer_spec(description))
    }
}

pub fn interpret(description: &str) -> Result<WebsiteSpec, AgentError> {
    let mut interpreter = DesignInterpreter::new();
    interpreter.interpret(description)
}

fn infer_spec(description: &str) -> WebsiteSpec {
    let lower = description.to_ascii_lowercase();

    let mut sections = vec![
        SectionSpec {
            kind: SectionKind::Header,
            template_id: "nav-fixed".to_string(),
            content: "Primary navigation with CTA".to_string(),
        },
        SectionSpec {
            kind: SectionKind::Hero,
            template_id: if lower.contains("3d") {
                "hero-3d-product".to_string()
            } else {
                "hero-centered".to_string()
            },
            content: description.to_string(),
        },
        SectionSpec {
            kind: SectionKind::Features,
            template_id: "features-card-grid".to_string(),
            content: "Core value propositions".to_string(),
        },
    ];

    if lower.contains("menu") || lower.contains("coffee") || lower.contains("restaurant") {
        sections.push(SectionSpec {
            kind: SectionKind::Menu,
            template_id: "menu-two-column".to_string(),
            content: "Menu categories and prices".to_string(),
        });
    }

    sections.push(SectionSpec {
        kind: SectionKind::Contact,
        template_id: "contact-split-form".to_string(),
        content: "Contact form, address, and hours".to_string(),
    });
    sections.push(SectionSpec {
        kind: SectionKind::Footer,
        template_id: "footer-newsletter".to_string(),
        content: "Footer links and social proof".to_string(),
    });

    let mood = if lower.contains("dark") && lower.contains("coffee") {
        "warm-dark".to_string()
    } else if lower.contains("coffee") {
        "warm-organic".to_string()
    } else if lower.contains("cyberpunk") {
        "cyberpunk".to_string()
    } else if lower.contains("luxury") {
        "luxury".to_string()
    } else {
        "tech".to_string()
    };

    let colors = if mood.contains("warm") {
        vec![
            "#2A1F1A".to_string(),
            "#A06A42".to_string(),
            "#F3E6D6".to_string(),
            "#E39C5A".to_string(),
        ]
    } else if mood == "cyberpunk" {
        vec![
            "#07010F".to_string(),
            "#00F5D4".to_string(),
            "#FF00A8".to_string(),
            "#B8FF00".to_string(),
        ]
    } else {
        vec![
            "#0B1020".to_string(),
            "#4CC9F0".to_string(),
            "#F8F9FA".to_string(),
            "#4361EE".to_string(),
        ]
    };

    let page = PageSpec {
        name: "Home".to_string(),
        layout: "landing".to_string(),
        sections,
        content: description.to_string(),
    };

    let mut three_d_elements = Vec::new();
    if lower.contains("3d") {
        let model = if lower.contains("coffee") {
            "coffee-cup".to_string()
        } else {
            "product-model".to_string()
        };
        three_d_elements.push(ThreeDSpec {
            model,
            animation: "slow-rotate".to_string(),
            position: "hero-right".to_string(),
        });
    }

    WebsiteSpec {
        pages: vec![page],
        theme: ThemeSpec {
            colors,
            fonts: vec![
                "Space Grotesk".to_string(),
                "Inter".to_string(),
                "JetBrains Mono".to_string(),
            ],
            mood,
        },
        components: vec![
            ComponentSpec {
                name: "Header".to_string(),
                props_schema: "{ links: NavItem[] }".to_string(),
            },
            ComponentSpec {
                name: "Hero".to_string(),
                props_schema: "{ title: string; subtitle: string }".to_string(),
            },
            ComponentSpec {
                name: "Footer".to_string(),
                props_schema: "{ year: number }".to_string(),
            },
        ],
        three_d_elements,
        animations: vec![AnimationSpec {
            trigger: "on-scroll".to_string(),
            animation_type: "fade-up".to_string(),
            target: "section".to_string(),
        }],
        responsive: true,
        framework: Framework::React,
    }
}
