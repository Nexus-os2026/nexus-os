use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotAnalysis {
    pub layout: String,
    pub padding_px: u32,
    pub background_color: String,
    pub border_radius_px: u32,
    pub text_hierarchy: Vec<String>,
    pub shadow: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotCodeResult {
    pub analysis: ScreenshotAnalysis,
    pub react_component: String,
}

pub struct ScreenshotToCode {
    gateway: GovernedLlmGateway<Box<dyn LlmProvider>>,
    runtime: AgentRuntimeContext,
}

impl Default for ScreenshotToCode {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenshotToCode {
    pub fn new() -> Self {
        let config = ProviderSelectionConfig::from_env();
        let provider: Box<dyn LlmProvider> = select_provider(&config).unwrap_or_else(|_| {
            Box::new(nexus_connectors_llm::providers::OllamaProvider::from_env())
        });
        let gateway = GovernedLlmGateway::new(provider);
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        let runtime = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities,
            fuel_remaining: 3_000,
        };
        Self { gateway, runtime }
    }

    pub fn screenshot_to_code(
        &mut self,
        image_description: &str,
    ) -> Result<ScreenshotCodeResult, AgentError> {
        let prompt = format!(
            "Analyze screenshot and return UI structure, spacing, typography hierarchy, and color tokens: {image_description}"
        );
        let _ = self
            .gateway
            .query(&mut self.runtime, prompt.as_str(), 260, "mock-1")?;

        let analysis = infer_analysis(image_description);
        let react_component = render_component(&analysis);
        Ok(ScreenshotCodeResult {
            analysis,
            react_component,
        })
    }
}

pub fn screenshot_to_code(image_description: &str) -> Result<ScreenshotCodeResult, AgentError> {
    let mut engine = ScreenshotToCode::new();
    engine.screenshot_to_code(image_description)
}

fn infer_analysis(description: &str) -> ScreenshotAnalysis {
    let lower = description.to_ascii_lowercase();
    let layout = if lower.contains("card") {
        "card".to_string()
    } else if lower.contains("dashboard") {
        "dashboard".to_string()
    } else {
        "panel".to_string()
    };

    let padding_px = parse_px_after(lower.as_str(), "padding").unwrap_or(16);
    let border_radius_px = parse_px_after(lower.as_str(), "radius").unwrap_or(12);
    let background_color = if let Some(hex) = first_hex_color(lower.as_str()) {
        hex
    } else if lower.contains("white") {
        "#FFFFFF".to_string()
    } else if lower.contains("slate") || lower.contains("dark") {
        "#111827".to_string()
    } else {
        "#F8FAFC".to_string()
    };
    let shadow = if lower.contains("medium") || lower.contains("md") {
        "medium".to_string()
    } else if lower.contains("soft") || lower.contains("sm") {
        "soft".to_string()
    } else if lower.contains("strong") || lower.contains("lg") {
        "strong".to_string()
    } else {
        "none".to_string()
    };
    let mut text_hierarchy = Vec::new();
    if lower.contains("title") || lower.contains("heading") {
        text_hierarchy.push("title".to_string());
    }
    if lower.contains("subtitle") {
        text_hierarchy.push("subtitle".to_string());
    }
    if lower.contains("body") || lower.contains("description") {
        text_hierarchy.push("body".to_string());
    }
    if text_hierarchy.is_empty() {
        text_hierarchy = vec!["title".to_string(), "body".to_string()];
    }

    ScreenshotAnalysis {
        layout,
        padding_px,
        background_color,
        border_radius_px,
        text_hierarchy,
        shadow,
    }
}

fn parse_px_after(text: &str, keyword: &str) -> Option<u32> {
    let keyword_index = text.find(keyword)?;
    let tail = &text[(keyword_index + keyword.len())..];
    for token in tail.split_whitespace().take(4) {
        let clean = token
            .trim_matches(|ch: char| ch == ':' || ch == ',' || ch == ';' || ch == '.')
            .trim_end_matches("px");
        if let Ok(value) = clean.parse::<u32>() {
            return Some(value);
        }
    }
    None
}

fn first_hex_color(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b'#' && index + 7 <= bytes.len() {
            let candidate = &text[index..index + 7];
            if candidate.chars().skip(1).all(|ch| ch.is_ascii_hexdigit()) {
                return Some(candidate.to_ascii_uppercase());
            }
        }
    }
    None
}

fn render_component(analysis: &ScreenshotAnalysis) -> String {
    let shadow_class = match analysis.shadow.as_str() {
        "soft" => "shadow-sm",
        "medium" => "shadow-md",
        "strong" => "shadow-lg",
        _ => "shadow-none",
    };

    format!(
        "import React from \"react\";

export function ScreenshotReplica(): JSX.Element {{
  return (
    <article
      className=\"{shadow_class}\"
      style={{{{
        padding: \"{padding}px\",
        backgroundColor: \"{bg}\",
        borderRadius: \"{radius}px\"
      }}}}
    >
      <h3 className=\"text-lg font-semibold\">Title</h3>
      <p className=\"text-sm text-slate-600\">Body content</p>
    </article>
  );
}}",
        shadow_class = shadow_class,
        padding = analysis.padding_px,
        bg = analysis.background_color,
        radius = analysis.border_radius_px
    )
}
