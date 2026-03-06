use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::{LlmProvider, MockProvider};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    Page,
    Sidebar,
    Header,
    Main,
    CardGrid,
    ChartArea,
    Footer,
    Section,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutNode {
    pub id: String,
    pub kind: LayoutKind,
    pub children: Vec<LayoutNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignComponent {
    pub name: String,
    pub purpose: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypographySpec {
    pub display_font: String,
    pub body_font: String,
    pub mono_font: String,
    pub base_size_px: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacingSpec {
    pub base_unit_px: u8,
    pub scale: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignSpec {
    pub layout_tree: LayoutNode,
    pub components: Vec<DesignComponent>,
    pub colors: Vec<String>,
    pub typography: TypographySpec,
    pub spacing: SpacingSpec,
    pub svg_mockup: String,
    pub react_component: String,
}

pub struct DesignGenerator {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
}

impl Default for DesignGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignGenerator {
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

    pub fn generate_design(&mut self, description: &str) -> Result<DesignSpec, AgentError> {
        let prompt = format!(
            "Generate an accessible UI blueprint with layout and component guidance for: {description}"
        );
        let _ = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 280, "mock-1")?;
        Ok(infer_design(description))
    }
}

pub fn generate_design(description: &str) -> Result<DesignSpec, AgentError> {
    let mut generator = DesignGenerator::new();
    generator.generate_design(description)
}

fn infer_design(description: &str) -> DesignSpec {
    let lower = description.to_ascii_lowercase();
    if lower.contains("dashboard") || lower.contains("analytics") {
        return analytics_dashboard_spec();
    }

    DesignSpec {
        layout_tree: LayoutNode {
            id: "root".to_string(),
            kind: LayoutKind::Page,
            children: vec![
                LayoutNode {
                    id: "header".to_string(),
                    kind: LayoutKind::Header,
                    children: Vec::new(),
                },
                LayoutNode {
                    id: "main".to_string(),
                    kind: LayoutKind::Main,
                    children: vec![LayoutNode {
                        id: "section-primary".to_string(),
                        kind: LayoutKind::Section,
                        children: Vec::new(),
                    }],
                },
                LayoutNode {
                    id: "footer".to_string(),
                    kind: LayoutKind::Footer,
                    children: Vec::new(),
                },
            ],
        },
        components: vec![
            DesignComponent {
                name: "HeaderNav".to_string(),
                purpose: "Primary navigation and action buttons".to_string(),
            },
            DesignComponent {
                name: "HeroPanel".to_string(),
                purpose: "Main call-to-action area".to_string(),
            },
        ],
        colors: vec![
            "#0F172A".to_string(),
            "#2563EB".to_string(),
            "#F8FAFC".to_string(),
            "#22C55E".to_string(),
        ],
        typography: TypographySpec {
            display_font: "Space Grotesk".to_string(),
            body_font: "Inter".to_string(),
            mono_font: "JetBrains Mono".to_string(),
            base_size_px: 16,
        },
        spacing: SpacingSpec {
            base_unit_px: 4,
            scale: vec![4, 8, 12, 16, 24, 32],
        },
        svg_mockup: svg_mockup(false),
        react_component: react_component(false),
    }
}

fn analytics_dashboard_spec() -> DesignSpec {
    DesignSpec {
        layout_tree: LayoutNode {
            id: "root".to_string(),
            kind: LayoutKind::Page,
            children: vec![
                LayoutNode {
                    id: "sidebar".to_string(),
                    kind: LayoutKind::Sidebar,
                    children: Vec::new(),
                },
                LayoutNode {
                    id: "main".to_string(),
                    kind: LayoutKind::Main,
                    children: vec![
                        LayoutNode {
                            id: "header".to_string(),
                            kind: LayoutKind::Header,
                            children: Vec::new(),
                        },
                        LayoutNode {
                            id: "kpi-grid".to_string(),
                            kind: LayoutKind::CardGrid,
                            children: Vec::new(),
                        },
                        LayoutNode {
                            id: "charts".to_string(),
                            kind: LayoutKind::ChartArea,
                            children: Vec::new(),
                        },
                    ],
                },
            ],
        },
        components: vec![
            DesignComponent {
                name: "SidebarNav".to_string(),
                purpose: "Persistent analytics navigation".to_string(),
            },
            DesignComponent {
                name: "KpiCardGrid".to_string(),
                purpose: "Card grid with key metrics".to_string(),
            },
            DesignComponent {
                name: "LineChartPanel".to_string(),
                purpose: "Trend chart with time ranges".to_string(),
            },
            DesignComponent {
                name: "BarChartPanel".to_string(),
                purpose: "Comparative segmented chart".to_string(),
            },
        ],
        colors: vec![
            "#0B1220".to_string(),
            "#1E293B".to_string(),
            "#E2E8F0".to_string(),
            "#3B82F6".to_string(),
            "#22C55E".to_string(),
        ],
        typography: TypographySpec {
            display_font: "Sora".to_string(),
            body_font: "Inter".to_string(),
            mono_font: "JetBrains Mono".to_string(),
            base_size_px: 16,
        },
        spacing: SpacingSpec {
            base_unit_px: 4,
            scale: vec![4, 8, 12, 16, 20, 24, 32],
        },
        svg_mockup: svg_mockup(true),
        react_component: react_component(true),
    }
}

fn svg_mockup(with_sidebar: bool) -> String {
    if with_sidebar {
        return r##"<svg viewBox="0 0 1200 720" xmlns="http://www.w3.org/2000/svg">
  <rect x="0" y="0" width="1200" height="720" fill="#0B1220"/>
  <rect x="0" y="0" width="220" height="720" fill="#111827"/>
  <rect x="260" y="40" width="900" height="80" rx="12" fill="#1E293B"/>
  <rect x="260" y="150" width="280" height="140" rx="12" fill="#1E293B"/>
  <rect x="560" y="150" width="280" height="140" rx="12" fill="#1E293B"/>
  <rect x="860" y="150" width="280" height="140" rx="12" fill="#1E293B"/>
  <rect x="260" y="320" width="880" height="340" rx="12" fill="#1E293B"/>
</svg>"##
            .to_string();
    }

    r##"<svg viewBox="0 0 1200 720" xmlns="http://www.w3.org/2000/svg">
  <rect x="0" y="0" width="1200" height="720" fill="#0F172A"/>
  <rect x="40" y="40" width="1120" height="80" rx="12" fill="#1E293B"/>
  <rect x="40" y="150" width="1120" height="460" rx="16" fill="#1E293B"/>
  <rect x="40" y="640" width="1120" height="40" rx="8" fill="#334155"/>
</svg>"##
        .to_string()
}

fn react_component(with_sidebar: bool) -> String {
    if with_sidebar {
        return r#"import React from "react";

export function AnalyticsDashboard(): JSX.Element {
  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <div className="grid grid-cols-[240px_1fr]">
        <aside className="border-r border-slate-800 p-4">Sidebar</aside>
        <main className="p-6 space-y-6">
          <header className="rounded-xl bg-slate-800 p-4">Header</header>
          <section className="grid gap-4 md:grid-cols-3">
            <article className="rounded-xl bg-slate-800 p-4">KPI Card</article>
            <article className="rounded-xl bg-slate-800 p-4">KPI Card</article>
            <article className="rounded-xl bg-slate-800 p-4">KPI Card</article>
          </section>
          <section className="rounded-xl bg-slate-800 p-4">Chart Area</section>
        </main>
      </div>
    </div>
  );
}"#
        .to_string();
    }

    r#"import React from "react";

export function GeneratedPage(): JSX.Element {
  return (
    <div className="min-h-screen bg-slate-900 text-slate-100 p-6">
      <header className="rounded-xl bg-slate-800 p-4">Header</header>
      <main className="mt-6 rounded-2xl bg-slate-800 p-6">Main Section</main>
      <footer className="mt-6 rounded-lg bg-slate-700 p-3">Footer</footer>
    </div>
  );
}"#
    .to_string()
}
