//! Two-stage template classifier for Nexus Builder Phase 2.
//!
//! Stage 1: Rule-based keyword matching (instant, free).
//! Stage 2: Haiku LLM disambiguation (~$0.002, ~2s) when confidence < 0.7.

use crate::plan::ProductBrief;
use crate::templates;
use nexus_connectors_llm::providers::LlmProvider;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Haiku model constant (same as plan.rs).
const HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

/// Confidence threshold — above this, rule-based classification is sufficient.
const CONFIDENCE_THRESHOLD: f64 = 0.7;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Result of template classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSelection {
    pub template_id: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
    pub modifiers: Vec<String>,
    /// Which classification stage produced this result.
    #[serde(default)]
    pub stage: String,
}

// ─── Stage 1: Rule-Based Classification ─────────────────────────────────────

/// Classify a user prompt + brief into a template using keyword matching.
/// Returns a `TemplateSelection` with confidence 0.0-1.0.
pub fn classify_rule_based(prompt: &str, brief: &ProductBrief) -> TemplateSelection {
    let lower_prompt = prompt.to_lowercase();
    let lower_type = brief.project_type.to_lowercase();
    let lower_suggestion = brief.template_suggestion.to_lowercase();

    // Combine all text sources for keyword matching
    let combined = format!("{lower_prompt} {lower_type} {lower_suggestion}");

    let mut best_id = String::new();
    let mut best_score: usize = 0;
    let mut best_reasons = Vec::new();

    for tmpl in templates::all_templates() {
        let mut score: usize = 0;
        let mut reasons = Vec::new();

        for &keyword in tmpl.keywords {
            if combined.contains(keyword) {
                score += 1;
                reasons.push(format!("matched keyword \"{keyword}\""));
            }
        }

        // Bonus for template_suggestion match (skip if suggestion is empty)
        if !lower_suggestion.is_empty()
            && (lower_suggestion.contains(tmpl.id) || tmpl.id.contains(lower_suggestion.as_str()))
        {
            score += 3;
            reasons.push(format!("Haiku suggested \"{}\"", brief.template_suggestion));
        }

        if score > best_score {
            best_score = score;
            best_id = tmpl.id.to_string();
            best_reasons = reasons;
        }
    }

    // Normalize: max possible score varies per template, but we cap at reasonable values
    // A score of 5+ keywords is very high confidence
    let confidence = if best_score == 0 {
        0.0
    } else {
        (best_score as f64 / 7.0).min(1.0)
    };

    // Infer modifiers from prompt keywords
    let modifiers = infer_modifiers(&combined);

    TemplateSelection {
        template_id: best_id,
        confidence,
        reasons: best_reasons,
        modifiers,
        stage: "rule_based".to_string(),
    }
}

/// Infer which modifiers to apply based on prompt keywords.
fn infer_modifiers(text: &str) -> Vec<String> {
    let mut mods = Vec::new();

    if text.contains("book") || text.contains("reserv") || text.contains("appointment") {
        mods.push("booking_form".to_string());
    }
    if text.contains("gallery") || text.contains("photos") || text.contains("images") {
        mods.push("photo_gallery".to_string());
    }
    if text.contains("blog") || text.contains("posts") || text.contains("articles") {
        mods.push("blog_feed".to_string());
    }
    if text.contains("map") || text.contains("location") || text.contains("directions") {
        mods.push("contact_map".to_string());
    }
    if text.contains("calculator") || text.contains("estimate") || text.contains("pricing tool") {
        mods.push("pricing_calculator".to_string());
    }
    if text.contains("sidebar") && text.contains("doc") {
        mods.push("docs_sidebar".to_string());
    }

    mods
}

// ─── Stage 2: LLM-Assisted Classification ──────────────────────────────────

/// Classify using LLM when rule-based confidence is below threshold.
///
/// Accepts a model name to allow routing to cheaper/local models.
/// Defaults to Haiku when called via the original path.
pub fn classify_with_llm(
    provider: &dyn LlmProvider,
    prompt: &str,
    brief: &ProductBrief,
) -> Result<TemplateSelection, String> {
    classify_with_llm_model(provider, prompt, brief, HAIKU_MODEL)
}

/// Classify using a specific model.
pub fn classify_with_llm_model(
    provider: &dyn LlmProvider,
    prompt: &str,
    brief: &ProductBrief,
    model: &str,
) -> Result<TemplateSelection, String> {
    let is_local = provider.cost_per_token() == 0.0;

    // Use a shorter prompt for local models to avoid overwhelming them
    let classification_prompt = if is_local {
        let template_ids: Vec<&str> = templates::all_templates().iter().map(|t| t.id).collect();
        format!(
            "Classify this website into one template. Options: {}.\n\
             Request: {} ({})\n\
             Return ONLY JSON: {{\"template_id\":\"...\",\"confidence\":0.8,\"reasons\":[\"...\"],\"modifiers\":[]}}",
            template_ids.join(", "),
            prompt,
            brief.project_type,
        )
    } else {
        let template_list: Vec<String> = templates::all_templates()
            .iter()
            .map(|t| {
                format!(
                    "- id: \"{}\", name: \"{}\", description: \"{}\"",
                    t.id, t.name, t.description
                )
            })
            .collect();
        format!(
            "You are a template classifier. Given a user's website request and brief, select the best template.\n\n\
             Available templates:\n{}\n\n\
             User prompt: {}\n\
             Project type: {}\n\
             Haiku suggestion: {}\n\n\
             Available modifiers: docs_sidebar, pricing_calculator, booking_form, photo_gallery, contact_map, blog_feed\n\n\
             Return ONLY valid JSON with no markdown formatting:\n\
             {{\"template_id\": \"...\", \"confidence\": 0.85, \"reasons\": [\"...\"], \"modifiers\": [\"...\"]}}",
            template_list.join("\n"),
            prompt,
            brief.project_type,
            brief.template_suggestion,
        )
    };

    // Local models get more tokens to avoid truncation
    let max_tokens: u32 = if is_local { 1024 } else { 512 };

    let start = std::time::Instant::now();
    let response = provider
        .query(&classification_prompt, max_tokens, model)
        .map_err(|e| format!("Classification LLM call failed ({model}): {e}"))?;
    let elapsed_ms = start.elapsed().as_millis();

    let text = response.output_text.trim();
    eprintln!(
        "[classifier] {model} responded in {elapsed_ms}ms, {} chars",
        text.len()
    );

    // Empty or too-short response — fail immediately for failover
    if text.len() < 10 {
        return Err(format!(
            "Model {model} returned empty/too-short response ({} chars)",
            text.len()
        ));
    }

    let json_text = strip_markdown_fences(text);

    // Try parsing, with JSON repair for local models
    let mut selection: TemplateSelection = match serde_json::from_str(json_text) {
        Ok(s) => s,
        Err(e) => {
            if is_local {
                let repaired = crate::plan::repair_truncated_json(json_text);
                serde_json::from_str(&repaired).map_err(|e2| {
                    format!("Failed to parse classification JSON: {e}\nRepair failed: {e2}\nRaw: {text}")
                })?
            } else {
                return Err(format!(
                    "Failed to parse classification JSON: {e}\nRaw: {text}"
                ));
            }
        }
    };
    selection.stage = "llm".to_string();
    Ok(selection)
}

/// Full classification pipeline: rule-based first, then LLM fallback.
pub fn classify(
    provider: &dyn LlmProvider,
    prompt: &str,
    brief: &ProductBrief,
) -> TemplateSelection {
    classify_with_model(provider, prompt, brief, HAIKU_MODEL)
}

/// Classify with a specific model (for multi-model routing).
pub fn classify_with_model(
    provider: &dyn LlmProvider,
    prompt: &str,
    brief: &ProductBrief,
    model: &str,
) -> TemplateSelection {
    let rule_result = classify_rule_based(prompt, brief);

    if rule_result.confidence >= CONFIDENCE_THRESHOLD && !rule_result.template_id.is_empty() {
        return rule_result;
    }

    // Fall back to LLM
    match classify_with_llm_model(provider, prompt, brief, model) {
        Ok(llm_result) => llm_result,
        Err(e) => {
            eprintln!("[classifier] LLM fallback failed ({model}): {e}, using rule-based result");
            rule_result
        }
    }
}

// ─── Artefact Persistence ───────────────────────────────────────────────────

/// Save a `TemplateSelection` as JSON to `{project_dir}/artefacts/template_selection.json`.
pub fn save_selection_artefact(
    project_dir: &Path,
    selection: &TemplateSelection,
) -> Result<(), String> {
    let artefacts_dir = project_dir.join("artefacts");
    std::fs::create_dir_all(&artefacts_dir)
        .map_err(|e| format!("failed to create artefacts dir: {e}"))?;

    let json = serde_json::to_string_pretty(selection)
        .map_err(|e| format!("serialize template_selection: {e}"))?;
    std::fs::write(artefacts_dir.join("template_selection.json"), json)
        .map_err(|e| format!("write template_selection.json: {e}"))?;

    Ok(())
}

fn strip_markdown_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        let after_start = if let Some(nl) = trimmed.find('\n') {
            &trimmed[nl + 1..]
        } else {
            return trimmed;
        };
        let stripped = after_start.trim_end();
        if let Some(without_suffix) = stripped.strip_suffix("```") {
            without_suffix.trim_end()
        } else {
            stripped
        }
    } else {
        trimmed
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_connectors_llm::providers::{EmbeddingResponse, LlmResponse};
    use nexus_sdk::errors::AgentError;

    fn test_brief(project_type: &str, suggestion: &str) -> ProductBrief {
        ProductBrief {
            project_name: "Test".into(),
            project_type: project_type.into(),
            target_audience: "Users".into(),
            sections: vec![],
            design_direction: "Modern".into(),
            tone: "Professional".into(),
            template_suggestion: suggestion.into(),
            estimated_cost: "~$0.26".into(),
            estimated_time: "~60s".into(),
        }
    }

    #[test]
    fn test_classify_saas_landing() {
        let brief = test_brief("SaaS Landing Page", "saas_landing");
        let result = classify_rule_based("Build a SaaS landing page with pricing", &brief);
        assert_eq!(result.template_id, "saas_landing");
        assert!(result.confidence >= CONFIDENCE_THRESHOLD);
    }

    #[test]
    fn test_classify_docs_site() {
        let brief = test_brief("Documentation", "docs");
        let result = classify_rule_based("Create a documentation site for my API", &brief);
        assert_eq!(result.template_id, "docs_site");
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_classify_portfolio() {
        let brief = test_brief("Portfolio", "portfolio");
        let result = classify_rule_based("Build my personal portfolio with projects", &brief);
        assert_eq!(result.template_id, "portfolio");
    }

    #[test]
    fn test_classify_local_business() {
        let brief = test_brief("Restaurant Website", "local business");
        let result = classify_rule_based("Create a website for my restaurant with a menu", &brief);
        assert_eq!(result.template_id, "local_business");
    }

    #[test]
    fn test_classify_ecommerce() {
        let brief = test_brief("E-Commerce Store", "ecommerce");
        let result = classify_rule_based("Build an e-commerce store to sell products", &brief);
        assert_eq!(result.template_id, "ecommerce");
    }

    #[test]
    fn test_classify_dashboard() {
        let brief = test_brief("Admin Dashboard", "dashboard");
        let result = classify_rule_based("Build an admin dashboard with analytics", &brief);
        assert_eq!(result.template_id, "dashboard");
    }

    #[test]
    fn test_modifier_inference_booking() {
        let mods = infer_modifiers("restaurant with online booking and reservations");
        assert!(mods.contains(&"booking_form".to_string()));
    }

    #[test]
    fn test_modifier_inference_blog() {
        let mods = infer_modifiers("portfolio with a blog section and posts");
        assert!(mods.contains(&"blog_feed".to_string()));
    }

    #[test]
    fn test_modifier_inference_gallery() {
        let mods = infer_modifiers("bakery website with a photo gallery");
        assert!(mods.contains(&"photo_gallery".to_string()));
    }

    #[test]
    fn test_empty_prompt_returns_zero_confidence() {
        let brief = test_brief("", "");
        let result = classify_rule_based("", &brief);
        assert_eq!(result.confidence, 0.0);
    }

    struct MockClassifierProvider {
        response: String,
    }

    impl LlmProvider for MockClassifierProvider {
        fn query(
            &self,
            _prompt: &str,
            _max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: self.response.clone(),
                token_count: 100,
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: Some(200),
            })
        }
        fn name(&self) -> &str {
            "mock-classifier"
        }
        fn cost_per_token(&self) -> f64 {
            0.0
        }
        fn embed(&self, _texts: &[&str], model: &str) -> Result<EmbeddingResponse, AgentError> {
            Ok(EmbeddingResponse {
                embeddings: vec![],
                model_name: model.to_string(),
                token_count: 0,
            })
        }
    }

    #[test]
    fn test_classify_with_llm_mock() {
        let provider = MockClassifierProvider {
            response: r#"{"template_id": "portfolio", "confidence": 0.9, "reasons": ["personal site"], "modifiers": ["blog_feed"]}"#.into(),
        };
        let brief = test_brief("Personal Site", "");
        let result = classify_with_llm(&provider, "Build my personal website", &brief).unwrap();
        assert_eq!(result.template_id, "portfolio");
        assert_eq!(result.confidence, 0.9);
        assert!(result.modifiers.contains(&"blog_feed".to_string()));
    }

    #[test]
    fn test_full_classify_high_confidence_skips_llm() {
        // This should match rule-based with high confidence, never hitting the LLM
        let provider = MockClassifierProvider {
            response: "this should not be called".into(),
        };
        let brief = test_brief("SaaS Landing Page", "saas_landing");
        let result = classify(
            &provider,
            "Build a SaaS landing page with pricing and features",
            &brief,
        );
        assert_eq!(result.template_id, "saas_landing");
        assert!(result.confidence >= CONFIDENCE_THRESHOLD);
    }
}
