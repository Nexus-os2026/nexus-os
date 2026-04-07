//! Build plan generation via Haiku 4.5 for the Nexus Builder.
//!
//! Generates a structured product brief and acceptance criteria from a user's
//! natural language prompt. This plan is reviewed/edited by the user before
//! committing to a full Sonnet generation.

use crate::budget::BudgetTracker;
use crate::build_stream::calculate_cost;
use nexus_connectors_llm::providers::LlmProvider;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Product brief describing what to build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductBrief {
    pub project_name: String,
    pub project_type: String,
    pub target_audience: String,
    pub sections: Vec<String>,
    pub design_direction: String,
    pub tone: String,
    pub template_suggestion: String,
    pub estimated_cost: String,
    pub estimated_time: String,
}

/// Acceptance criteria: what must/must-not be in the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriteria {
    pub must_have: Vec<String>,
    pub must_not_have: Vec<String>,
    pub constraints: Vec<String>,
}

/// Combined plan returned by the planning step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub product_brief: ProductBrief,
    pub acceptance_criteria: AcceptanceCriteria,
}

/// Result of a planning call, including cost metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResult {
    pub plan: BuildPlan,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cost_usd: f64,
    pub elapsed_seconds: f64,
}

// ─── Constants ───────────────────────────────────────────────────────────────

const HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

const PLAN_SYSTEM_PROMPT: &str = "\
You are a web project planner. Given a user's website request, generate a structured build plan. \
Return ONLY valid JSON with no markdown formatting, no backticks, no explanation. \
Return a JSON object with exactly two keys: product_brief and acceptance_criteria.\n\n\
product_brief must have these keys: project_name (String), project_type (String), \
target_audience (String), sections (array of strings), design_direction (String), \
tone (String), template_suggestion (String), estimated_cost (String like \"~$0.26\"), \
estimated_time (String like \"~60s\").\n\n\
acceptance_criteria must have these keys: must_have (array of strings), \
must_not_have (array of strings), constraints (array of strings).\n\n\
The constraints array must always include these four items:\n\
- \"Single-file HTML with embedded CSS/JS\"\n\
- \"No external dependencies beyond Google Fonts\"\n\
- \"Semantic HTML with ARIA labels\"\n\
- \"All images use placeholder URLs\"";

/// Default constraints injected into every plan.
const DEFAULT_CONSTRAINTS: &[&str] = &[
    "Single-file HTML with embedded CSS/JS",
    "No external dependencies beyond Google Fonts",
    "Semantic HTML with ARIA labels",
    "All images use placeholder URLs",
];

// ─── Plan Generation ─────────────────────────────────────────────────────────

/// Generate a build plan from a user prompt using the default Haiku model.
pub fn generate_plan(provider: &dyn LlmProvider, user_prompt: &str) -> Result<PlanResult, String> {
    generate_plan_with_model(provider, user_prompt, HAIKU_MODEL)
}

/// Generate a build plan using a specified model.
///
/// Prepends the system prompt to the user prompt (since `LlmProvider::query`
/// doesn't have a separate system prompt parameter) and calls the model.
/// Uses higher max_tokens for local models which are less token-efficient.
pub fn generate_plan_with_model(
    provider: &dyn LlmProvider,
    user_prompt: &str,
    model: &str,
) -> Result<PlanResult, String> {
    if user_prompt.trim().is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    let start = std::time::Instant::now();

    // Local models need more tokens — they're less efficient than API models
    let max_tokens: u32 = if provider.cost_per_token() == 0.0 {
        4096
    } else {
        2048
    };

    // Combine system prompt + user prompt (query() has no system_prompt param)
    let full_prompt = format!("{PLAN_SYSTEM_PROMPT}\n\nUser request: {user_prompt}");

    let response = provider
        .query(&full_prompt, max_tokens, model)
        .map_err(|e| format!("Planning call failed ({model}): {e}"))?;

    let elapsed = start.elapsed().as_secs_f64();

    // Parse the JSON response
    let text = response.output_text.trim();

    if text.is_empty() {
        return Err(format!("Model {model} returned empty response"));
    }

    // Strip markdown fences if the model wraps them despite instructions
    let json_text = strip_markdown_fences(text);

    // Try parsing directly, then try JSON repair for truncated responses
    let mut plan: BuildPlan = match serde_json::from_str(json_text) {
        Ok(p) => p,
        Err(e) => {
            // Attempt to repair truncated JSON (common with local models)
            let repaired = repair_truncated_json(json_text);
            serde_json::from_str(&repaired).map_err(|e2| {
                format!("Failed to parse plan JSON: {e}\nRepair also failed: {e2}\nRaw response: {text}")
            })?
        }
    };

    // Ensure default constraints are always present
    for &c in DEFAULT_CONSTRAINTS {
        if !plan.acceptance_criteria.constraints.iter().any(|x| x == c) {
            plan.acceptance_criteria.constraints.push(c.to_string());
        }
    }

    let input_tokens = response.input_tokens.unwrap_or(0) as usize;
    let output_tokens = response.token_count as usize;
    let cost = calculate_cost(model, input_tokens, output_tokens);

    Ok(PlanResult {
        plan,
        input_tokens,
        output_tokens,
        cost_usd: cost,
        elapsed_seconds: elapsed,
    })
}

/// Strip markdown code fences from a string (```json ... ``` or ``` ... ```).
fn strip_markdown_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.starts_with("```") {
        // Find end of first line (skip ```json or ```)
        let after_start = if let Some(nl) = trimmed.find('\n') {
            &trimmed[nl + 1..]
        } else {
            return trimmed;
        };
        // Strip trailing ```
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

/// Repair truncated JSON by closing unmatched braces and brackets.
///
/// Local models sometimes run out of tokens mid-JSON. This appends the
/// missing `]` and `}` characters so the output can still be parsed.
pub fn repair_truncated_json(s: &str) -> String {
    let mut result = s.to_string();

    // Strip trailing comma if present (common truncation artifact)
    let trimmed = result.trim_end();
    if let Some(stripped) = trimmed.strip_suffix(',') {
        result = stripped.to_string();
    }

    // Count unmatched delimiters
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut in_string = false;
    let mut prev_char = ' ';

    for ch in result.chars() {
        if ch == '"' && prev_char != '\\' {
            in_string = !in_string;
        } else if !in_string {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                '[' => bracket_depth += 1,
                ']' => bracket_depth -= 1,
                _ => {}
            }
        }
        prev_char = ch;
    }

    // Close unclosed strings
    if in_string {
        result.push('"');
    }

    // Close brackets before braces (inner before outer)
    for _ in 0..bracket_depth {
        result.push(']');
    }
    for _ in 0..brace_depth {
        result.push('}');
    }

    result
}

// ─── Artefact Persistence ────────────────────────────────────────────────────

/// Save a plan's artefacts to `{project_dir}/artefacts/`.
pub fn save_plan_artefacts(project_dir: &Path, plan: &BuildPlan) -> Result<(), String> {
    let artefacts_dir = project_dir.join("artefacts");
    std::fs::create_dir_all(&artefacts_dir)
        .map_err(|e| format!("failed to create artefacts dir: {e}"))?;

    let brief_json = serde_json::to_string_pretty(&plan.product_brief)
        .map_err(|e| format!("serialize brief: {e}"))?;
    std::fs::write(artefacts_dir.join("product_brief.json"), brief_json)
        .map_err(|e| format!("write product_brief.json: {e}"))?;

    let criteria_json = serde_json::to_string_pretty(&plan.acceptance_criteria)
        .map_err(|e| format!("serialize criteria: {e}"))?;
    std::fs::write(
        artefacts_dir.join("acceptance_criteria.json"),
        criteria_json,
    )
    .map_err(|e| format!("write acceptance_criteria.json: {e}"))?;

    Ok(())
}

/// Load a previously saved plan from `{project_dir}/artefacts/`.
pub fn load_plan_artefacts(project_dir: &Path) -> Option<BuildPlan> {
    let artefacts_dir = project_dir.join("artefacts");
    let brief_path = artefacts_dir.join("product_brief.json");
    let criteria_path = artefacts_dir.join("acceptance_criteria.json");

    let brief_str = std::fs::read_to_string(&brief_path).ok()?;
    let criteria_str = std::fs::read_to_string(&criteria_path).ok()?;

    let product_brief: ProductBrief = serde_json::from_str(&brief_str).ok()?;
    let acceptance_criteria: AcceptanceCriteria = serde_json::from_str(&criteria_str).ok()?;

    Some(BuildPlan {
        product_brief,
        acceptance_criteria,
    })
}

// ─── Budget Recording ────────────────────────────────────────────────────────

/// Record the plan generation cost in the budget tracker.
pub fn record_plan_cost(result: &PlanResult, project_name: &str) {
    let tracker = BudgetTracker::new();
    let record = crate::budget::BuildRecord {
        project_name: format!("Plan: {project_name}"),
        model_name: HAIKU_MODEL.to_string(),
        provider: "anthropic".to_string(),
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
        cost_usd: result.cost_usd,
        elapsed_seconds: result.elapsed_seconds,
        lines_generated: 0,
        checkpoint_id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = tracker.record_build(record);
}

// ─── Prompt Augmentation ─────────────────────────────────────────────────────

/// Compact quality directives for the Sonnet generation prompt.
///
/// Condensed from ~1100 chars to ~600 chars without losing any requirements.
const GENERATION_QUALITY_DIRECTIVES: &str = "\
Expert web developer building a production-quality single-page website.\n\
RULES: modern CSS (custom properties, Grid/Flexbox, gradients, transitions 0.3s ease). \
Google Fonts with distinct heading/body fonts, proper weight variation. \
Responsive: breakpoints 768px/1024px, hamburger nav on mobile, min 44px tap targets. \
Hero: full-viewport, strong typography. Spacing: 4rem section padding. \
ARIA labels, h1>h2>h3 hierarchy, sufficient contrast. Semantic HTML5, inline critical CSS.\n\
DO NOT: invent fake stats/testimonials/metrics, use lorem ipsum or placeholder text. \
Write real content — describe actual capabilities or category features honestly.";

/// Final instruction appended after the acceptance criteria.
const GENERATION_QUALITY_CLOSING: &str = "\
Generate a COMPLETE, production-ready website — fully styled, real content, smooth interactions, \
professional typography. Not a wireframe or skeleton.";

/// Build an augmented prompt that prepends the approved plan to the user's
/// original prompt for the Sonnet generation step.
pub fn build_planned_prompt(
    user_prompt: &str,
    brief: &ProductBrief,
    criteria: &AcceptanceCriteria,
) -> String {
    build_planned_prompt_with_template(user_prompt, brief, criteria, None)
}

/// Build an augmented prompt with an optional template skeleton.
///
/// When `template_html` is provided, we include a compact section spec instead
/// of the full 30-40KB HTML scaffold. The spec lists section IDs and required
/// attributes — the LLM generates structure from the plan, not by copying HTML.
///
/// **Prompt budget breakdown:**
/// - Quality directives: ~150 tokens
/// - Plan (compact JSON): ~150 tokens
/// - Acceptance criteria (compact): ~80 tokens
/// - Template spec (if any): ~60 tokens
/// - Closing + user prompt: ~50 tokens
/// - Total: ~490 tokens (well under 5,000 target)
pub fn build_planned_prompt_with_template(
    user_prompt: &str,
    brief: &ProductBrief,
    criteria: &AcceptanceCriteria,
    template_html: Option<&str>,
) -> String {
    // Use compact (non-pretty) JSON to save ~40% on whitespace
    let brief_json = serde_json::to_string(brief).unwrap_or_default();
    let criteria_json = serde_json::to_string(criteria).unwrap_or_default();

    // If we have template HTML, extract the template ID and use compact_spec
    // instead of embedding the full 30-40KB HTML scaffold.
    let template_section = match template_html {
        Some(html) => {
            // Try to find the matching template by checking if the html matches
            // any known template (exact or modified). Fall back to a generic hint.
            let spec = find_template_spec_from_html(html);
            format!("\n\n{spec}")
        }
        None => String::new(),
    };

    format!(
        "{GENERATION_QUALITY_DIRECTIVES}\n\n\
         Build a website according to this plan:\n\
         {brief_json}\n\n\
         Acceptance criteria: {criteria_json}{template_section}\n\n\
         {GENERATION_QUALITY_CLOSING}\n\n\
         User prompt: {user_prompt}"
    )
}

/// Try to match template HTML to a known template and return its compact spec.
/// Falls back to a generic section-attribute reminder.
fn find_template_spec_from_html(html: &str) -> String {
    // Check each template — the HTML may have been modified, so check for
    // data-nexus-section attributes that are unique to each template.
    let all = crate::templates::all_templates();
    for tmpl in all {
        // Check if any of the template's section IDs appear as data-nexus-section
        // in the HTML (robust against modifier changes)
        let matches = tmpl
            .sections
            .iter()
            .filter(|s| {
                html.contains(&format!("data-nexus-section=\"{s}\""))
                    || html.contains(&format!("data-nexus-section='{s}'"))
            })
            .count();
        // If at least half the sections match, it's this template
        if matches > 0 && matches >= tmpl.sections.len() / 2 {
            return tmpl.compact_spec();
        }
    }

    // Fallback: generic instruction (no specific template identified)
    "Structure: add data-nexus-section=\"<id>\" and data-nexus-editable=\"true\" on each <section>."
        .to_string()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_connectors_llm::providers::{EmbeddingResponse, LlmResponse};
    use nexus_sdk::errors::AgentError;
    use std::fs;

    /// A test-only mock provider that returns a configurable response string.
    struct PlanMockProvider {
        response_text: String,
    }

    impl PlanMockProvider {
        fn new(response_text: &str) -> Self {
            Self {
                response_text: response_text.to_string(),
            }
        }
    }

    impl LlmProvider for PlanMockProvider {
        fn query(
            &self,
            _prompt: &str,
            _max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: self.response_text.clone(),
                token_count: 500,
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: Some(200),
            })
        }

        fn name(&self) -> &str {
            "plan-mock"
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
    fn test_strip_markdown_fences_plain() {
        let input = r#"{"product_brief": {}}"#;
        assert_eq!(strip_markdown_fences(input), input);
    }

    #[test]
    fn test_strip_markdown_fences_with_json_tag() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_markdown_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_strip_markdown_fences_with_bare_backticks() {
        let input = "```\n{\"key\": \"value\"}\n```";
        assert_eq!(strip_markdown_fences(input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_default_constraints_injected() {
        let plan_json = r#"{
            "product_brief": {
                "project_name": "Test Site",
                "project_type": "Landing page",
                "target_audience": "Developers",
                "sections": ["Hero", "Features"],
                "design_direction": "Dark, modern",
                "tone": "Professional",
                "template_suggestion": "SaaS landing",
                "estimated_cost": "~$0.26",
                "estimated_time": "~60s"
            },
            "acceptance_criteria": {
                "must_have": ["Responsive design"],
                "must_not_have": ["Lorem ipsum"],
                "constraints": []
            }
        }"#;

        let mut plan: BuildPlan = serde_json::from_str(plan_json).unwrap();

        // Simulate what generate_plan does
        for &c in DEFAULT_CONSTRAINTS {
            if !plan.acceptance_criteria.constraints.iter().any(|x| x == c) {
                plan.acceptance_criteria.constraints.push(c.to_string());
            }
        }

        assert_eq!(plan.acceptance_criteria.constraints.len(), 4);
        assert!(plan
            .acceptance_criteria
            .constraints
            .contains(&"Single-file HTML with embedded CSS/JS".to_string()));
    }

    #[test]
    fn test_save_and_load_plan_artefacts() {
        let dir = std::env::temp_dir().join(format!("nexus-plan-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();

        let plan = BuildPlan {
            product_brief: ProductBrief {
                project_name: "Test Site".into(),
                project_type: "Landing page".into(),
                target_audience: "Devs".into(),
                sections: vec!["Hero".into(), "Features".into()],
                design_direction: "Dark".into(),
                tone: "Professional".into(),
                template_suggestion: "SaaS".into(),
                estimated_cost: "~$0.26".into(),
                estimated_time: "~60s".into(),
            },
            acceptance_criteria: AcceptanceCriteria {
                must_have: vec!["Responsive".into()],
                must_not_have: vec!["Lorem ipsum".into()],
                constraints: vec!["Single-file HTML with embedded CSS/JS".into()],
            },
        };

        save_plan_artefacts(&dir, &plan).unwrap();

        // Verify artefacts are saved in the artefacts/ subdirectory
        assert!(dir.join("artefacts").join("product_brief.json").exists());
        assert!(dir
            .join("artefacts")
            .join("acceptance_criteria.json")
            .exists());

        let loaded = load_plan_artefacts(&dir).unwrap();
        assert_eq!(loaded.product_brief.project_name, "Test Site");
        assert_eq!(loaded.acceptance_criteria.must_have.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_plan_missing_dir() {
        let dir = std::env::temp_dir().join("nexus-plan-nonexistent-dir-xyz");
        assert!(load_plan_artefacts(&dir).is_none());
    }

    #[test]
    fn test_build_planned_prompt() {
        let brief = ProductBrief {
            project_name: "My Site".into(),
            project_type: "Portfolio".into(),
            target_audience: "Employers".into(),
            sections: vec!["Hero".into()],
            design_direction: "Minimal".into(),
            tone: "Clean".into(),
            template_suggestion: "Portfolio".into(),
            estimated_cost: "~$0.26".into(),
            estimated_time: "~60s".into(),
        };
        let criteria = AcceptanceCriteria {
            must_have: vec!["Dark theme".into()],
            must_not_have: vec![],
            constraints: vec!["Single-file HTML with embedded CSS/JS".into()],
        };

        let prompt = build_planned_prompt("Build a portfolio site", &brief, &criteria);
        // Quality directives present at the top
        assert!(prompt.contains("Expert web developer"));
        assert!(prompt.contains("Google Fonts"));
        assert!(prompt.contains("DO NOT"));
        assert!(prompt.contains("lorem ipsum"));
        // Plan JSON injected
        assert!(prompt.contains("Build a website according to this plan:"));
        assert!(prompt.contains("My Site"));
        assert!(prompt.contains("Acceptance criteria:"));
        assert!(prompt.contains("Dark theme"));
        // Quality closing directive
        assert!(prompt.contains("COMPLETE, production-ready"));
        // Original prompt preserved
        assert!(prompt.contains("User prompt: Build a portfolio site"));
    }

    #[test]
    fn test_generate_plan_empty_prompt() {
        let provider = PlanMockProvider::new("not called");
        let result = generate_plan(&provider, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_generate_plan_valid_response() {
        let mock_response = r#"{
            "product_brief": {
                "project_name": "Acme Corp",
                "project_type": "SaaS Landing Page",
                "target_audience": "B2B SaaS buyers",
                "sections": ["Hero", "Features", "Pricing", "Testimonials", "CTA"],
                "design_direction": "Dark, modern, glassmorphism accents",
                "tone": "Professional yet approachable",
                "template_suggestion": "SaaS landing page",
                "estimated_cost": "~$0.26",
                "estimated_time": "~60s"
            },
            "acceptance_criteria": {
                "must_have": ["Responsive design", "Dark theme", "Pricing table"],
                "must_not_have": ["Lorem ipsum", "Placeholder images without alt text"],
                "constraints": ["Single-file HTML with embedded CSS/JS"]
            }
        }"#;

        let provider = PlanMockProvider::new(mock_response);
        let result = generate_plan(&provider, "Build a SaaS landing page for Acme Corp");
        assert!(result.is_ok(), "generate_plan failed: {:?}", result.err());

        let plan_result = result.unwrap();
        assert_eq!(plan_result.plan.product_brief.project_name, "Acme Corp");
        assert_eq!(plan_result.plan.acceptance_criteria.must_have.len(), 3);
        // Default constraints should be injected (3 new + 1 already present = 4)
        assert!(
            plan_result.plan.acceptance_criteria.constraints.len() >= 4,
            "Expected at least 4 constraints, got {}",
            plan_result.plan.acceptance_criteria.constraints.len()
        );
    }

    #[test]
    fn test_generate_plan_markdown_wrapped_response() {
        let mock_response = "```json\n{\"product_brief\":{\"project_name\":\"Test\",\"project_type\":\"Blog\",\"target_audience\":\"Readers\",\"sections\":[\"Posts\"],\"design_direction\":\"Minimal\",\"tone\":\"Casual\",\"template_suggestion\":\"Blog\",\"estimated_cost\":\"~$0.20\",\"estimated_time\":\"~45s\"},\"acceptance_criteria\":{\"must_have\":[\"Posts list\"],\"must_not_have\":[],\"constraints\":[]}}\n```";

        let provider = PlanMockProvider::new(mock_response);
        let result = generate_plan(&provider, "Build a blog");
        assert!(
            result.is_ok(),
            "Should handle markdown-wrapped JSON: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap().plan.product_brief.project_name, "Test");
    }

    #[test]
    fn test_plan_approved_fields_in_sonnet_prompt() {
        let brief = ProductBrief {
            project_name: "Acme Corp".into(),
            project_type: "SaaS Landing".into(),
            target_audience: "B2B".into(),
            sections: vec!["Hero".into(), "Pricing".into()],
            design_direction: "Dark".into(),
            tone: "Professional".into(),
            template_suggestion: "SaaS".into(),
            estimated_cost: "~$0.26".into(),
            estimated_time: "~60s".into(),
        };
        let criteria = AcceptanceCriteria {
            must_have: vec!["Responsive".into(), "Pricing table".into()],
            must_not_have: vec!["Lorem ipsum".into()],
            constraints: vec!["Single-file HTML with embedded CSS/JS".into()],
        };

        let prompt = build_planned_prompt("Build a SaaS landing page", &brief, &criteria);

        // Verify all plan fields appear in the generation prompt
        assert!(prompt.contains("Acme Corp"));
        assert!(prompt.contains("SaaS Landing"));
        assert!(prompt.contains("Hero"));
        assert!(prompt.contains("Pricing"));
        assert!(prompt.contains("Responsive"));
        assert!(prompt.contains("Pricing table"));
        assert!(prompt.contains("Lorem ipsum"));
        assert!(prompt.contains("Single-file HTML with embedded CSS/JS"));
        assert!(prompt.contains("Build a SaaS landing page"));
    }

    #[test]
    fn test_repair_truncated_json_missing_closing_brace() {
        let input =
            r#"{"product_brief":{"project_name":"Test"},"acceptance_criteria":{"must_have":["a"]}"#;
        let repaired = repair_truncated_json(input);
        assert!(repaired.ends_with('}'));
        // Should be valid JSON now
        let parsed: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        assert_eq!(parsed["product_brief"]["project_name"], "Test");
    }

    #[test]
    fn test_repair_truncated_json_missing_bracket_and_brace() {
        let input = r#"{"items":["a","b"#;
        let repaired = repair_truncated_json(input);
        let parsed: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        assert_eq!(parsed["items"][0], "a");
    }

    #[test]
    fn test_repair_truncated_json_trailing_comma() {
        // Real truncation: JSON cut off after a comma
        let input = r#"{"a":1,"b":2,"#;
        let repaired = repair_truncated_json(input);
        // Should strip trailing comma and add closing brace
        assert!(repaired.ends_with('}'));
        assert!(!repaired.ends_with(",}"));
        let parsed: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        assert_eq!(parsed["a"], 1);
    }

    #[test]
    fn test_repair_truncated_json_valid_input_unchanged() {
        let input = r#"{"key":"value"}"#;
        let repaired = repair_truncated_json(input);
        assert_eq!(repaired, input);
    }

    #[test]
    fn test_repair_real_plan_truncation() {
        // Simulate the exact failure: plan JSON missing final }
        let input = r#"{"product_brief":{"project_name":"Pizza Palace","project_type":"Restaurant","target_audience":"Local diners","sections":["Hero","Menu","Reservations"],"design_direction":"Warm dark","tone":"Friendly","template_suggestion":"restaurant","estimated_cost":"~$0.20","estimated_time":"~60s"},"acceptance_criteria":{"must_have":["Menu section","Reservation form"],"must_not_have":["Lorem ipsum"],"constraints":["Single-file HTML with embedded CSS/JS"]}"#;
        let repaired = repair_truncated_json(input);
        let parsed: Result<BuildPlan, _> = serde_json::from_str(&repaired);
        assert!(
            parsed.is_ok(),
            "Repaired JSON should parse as BuildPlan: {:?}",
            parsed.err()
        );
        assert_eq!(parsed.unwrap().product_brief.project_name, "Pizza Palace");
    }

    #[test]
    fn test_planned_prompt_under_5000_tokens_without_template() {
        let brief = ProductBrief {
            project_name: "Acme Corp SaaS Platform".into(),
            project_type: "SaaS Landing Page with Dashboard".into(),
            target_audience: "B2B SaaS buyers and enterprise teams".into(),
            sections: vec![
                "Hero".into(),
                "Features".into(),
                "Pricing".into(),
                "Testimonials".into(),
                "CTA".into(),
                "Footer".into(),
            ],
            design_direction: "Dark, modern with glassmorphism accents".into(),
            tone: "Professional yet approachable".into(),
            template_suggestion: "SaaS landing page".into(),
            estimated_cost: "~$0.26".into(),
            estimated_time: "~60s".into(),
        };
        let criteria = AcceptanceCriteria {
            must_have: vec![
                "Responsive design".into(),
                "Dark theme".into(),
                "Pricing table with 3 tiers".into(),
                "Testimonial carousel".into(),
                "Animated hero section".into(),
            ],
            must_not_have: vec![
                "Lorem ipsum".into(),
                "Placeholder images without alt text".into(),
            ],
            constraints: vec![
                "Single-file HTML with embedded CSS/JS".into(),
                "No external dependencies beyond Google Fonts".into(),
                "Semantic HTML with ARIA labels".into(),
                "All images use placeholder URLs".into(),
            ],
        };

        let prompt = build_planned_prompt(
            "Build a modern SaaS landing page for Acme Corp with pricing, features, and testimonials",
            &brief,
            &criteria,
        );

        let chars = prompt.len();
        let est_tokens = chars / 4;
        eprintln!(
            "[prompt-size-test] Planned prompt WITHOUT template: ~{est_tokens} tokens ({chars} chars)"
        );
        assert!(
            est_tokens < 5000,
            "Planned prompt without template is ~{est_tokens} tokens ({chars} chars) — must be < 5,000"
        );
    }

    #[test]
    fn test_planned_prompt_under_5000_tokens_with_template() {
        let brief = ProductBrief {
            project_name: "Acme Corp SaaS Platform".into(),
            project_type: "SaaS Landing Page".into(),
            target_audience: "B2B SaaS buyers".into(),
            sections: vec![
                "Hero".into(),
                "Features".into(),
                "Pricing".into(),
                "Testimonials".into(),
                "CTA".into(),
                "Footer".into(),
            ],
            design_direction: "Dark, modern with glassmorphism".into(),
            tone: "Professional".into(),
            template_suggestion: "saas_landing".into(),
            estimated_cost: "~$0.26".into(),
            estimated_time: "~60s".into(),
        };
        let criteria = AcceptanceCriteria {
            must_have: vec![
                "Responsive".into(),
                "Dark theme".into(),
                "Pricing table".into(),
            ],
            must_not_have: vec!["Lorem ipsum".into()],
            constraints: vec!["Single-file HTML with embedded CSS/JS".into()],
        };

        // Simulate what the conductor does: get full template HTML
        let template = crate::templates::get_template("saas_landing").unwrap();

        let prompt = build_planned_prompt_with_template(
            "Build a SaaS landing page for Acme Corp",
            &brief,
            &criteria,
            Some(template.html),
        );

        let chars = prompt.len();
        let est_tokens = chars / 4;
        eprintln!(
            "[prompt-size-test] Planned prompt WITH template: ~{est_tokens} tokens ({chars} chars)"
        );
        assert!(
            est_tokens < 5000,
            "Planned prompt with template is ~{est_tokens} tokens ({chars} chars) — must be < 5,000. \
             OLD baseline was ~10,888 tokens (43,552 chars)"
        );
    }

    #[test]
    fn test_compact_spec_used_instead_of_full_html() {
        let template = crate::templates::get_template("saas_landing").unwrap();
        // Full HTML is 30-40KB
        assert!(
            template.html.len() > 30_000,
            "Template HTML should be >30KB, got {} bytes",
            template.html.len()
        );
        // Compact spec should be <500 chars
        let spec = template.compact_spec();
        assert!(
            spec.len() < 500,
            "Compact spec should be <500 chars, got {} chars",
            spec.len()
        );
        // Spec should contain the template ID and section list
        assert!(spec.contains("saas_landing"));
        assert!(spec.contains("hero"));
        assert!(spec.contains("pricing"));
        assert!(spec.contains("data-nexus-section"));
    }
}
