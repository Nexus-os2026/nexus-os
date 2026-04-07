//! Content Generator — calls gemma4:e4b to produce validated ContentPayload.
//!
//! Pipeline: build prompt → call LLM → parse JSON → validate → retry once on failure.

use crate::content_payload::{format_validation_errors, ContentError, ContentPayload};
use crate::content_prompt::build_content_prompt;
use crate::model_router::OLLAMA_LARGE;
use crate::slot_schema::TemplateSchema;
use crate::variant::VariantSelection;
use nexus_connectors_llm::providers::LlmProvider;

/// Generate personalized content for a template using gemma4:e4b.
///
/// 1. Builds the content prompt from brief + schema + variant
/// 2. Calls the LLM via the existing provider abstraction
/// 3. Parses and validates the response
/// 4. On validation failure, retries ONCE with a corrective prompt
/// 5. On second failure, returns the validation errors
pub fn generate_content(
    brief: &str,
    template_id: &str,
    schema: &TemplateSchema,
    variant: &VariantSelection,
    provider: &dyn LlmProvider,
) -> Result<ContentPayload, ContentError> {
    let prompt = build_content_prompt(brief, template_id, schema, variant);

    // First attempt
    let raw = call_llm(provider, &prompt)?;
    match ContentPayload::from_llm_response(&raw, template_id, schema, variant.clone()) {
        Ok(payload) => Ok(payload),
        Err(ContentError::InvalidJson { reason, raw: r }) => {
            // JSON parse failure — retry with hint
            let retry_prompt = format!(
                "{prompt}\n\nYour previous response was not valid JSON. Error: {reason}\n\
                 Please respond with ONLY a valid JSON object, no markdown fences."
            );
            let raw2 = call_llm(provider, &retry_prompt)?;
            ContentPayload::from_llm_response(&raw2, template_id, schema, variant.clone()).map_err(
                |_| ContentError::InvalidJson {
                    reason: format!("retry also failed: {reason}"),
                    raw: r,
                },
            )
        }
        Err(ContentError::ValidationFailed(errors)) => {
            // Validation failure — retry with specific error feedback
            let error_text = format_validation_errors(&errors);
            let retry_prompt = format!(
                "{prompt}\n\nYour previous response had validation errors:\n{error_text}\n\n\
                 Please fix these issues and respond with the corrected JSON."
            );
            let raw2 = call_llm(provider, &retry_prompt)?;
            ContentPayload::from_llm_response(&raw2, template_id, schema, variant.clone())
        }
        Err(e) => Err(e),
    }
}

/// Call the LLM provider and extract the response text.
fn call_llm(provider: &dyn LlmProvider, prompt: &str) -> Result<String, ContentError> {
    // Use generous token limit for local models
    let max_tokens: u32 = if provider.cost_per_token() == 0.0 {
        4096
    } else {
        2048
    };

    let response = provider
        .query(prompt, max_tokens, OLLAMA_LARGE)
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("connection") || msg.contains("refused") || msg.contains("timeout") {
                ContentError::ModelUnavailable(msg)
            } else {
                ContentError::ModelUnavailable(format!("LLM query failed: {msg}"))
            }
        })?;

    let text = response.output_text.trim().to_string();
    if text.is_empty() {
        return Err(ContentError::InvalidJson {
            reason: "model returned empty response".into(),
            raw: String::new(),
        });
    }

    Ok(text)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot_schema::get_template_schema;
    use crate::variant::MotionProfile;
    use nexus_connectors_llm::providers::{EmbeddingResponse, LlmResponse};
    use nexus_sdk::errors::AgentError;
    use std::collections::HashMap;

    /// Mock provider that returns a configurable response.
    struct MockContentProvider {
        response: String,
    }

    impl MockContentProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    impl LlmProvider for MockContentProvider {
        fn query(
            &self,
            _prompt: &str,
            _max_tokens: u32,
            _model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: self.response.clone(),
                token_count: 500,
                model_name: "gemma4:e4b".to_string(),
                tool_calls: Vec::new(),
                input_tokens: Some(200),
            })
        }

        fn name(&self) -> &str {
            "mock-content"
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

    /// Mock provider that simulates Ollama being down.
    struct FailingProvider;

    impl LlmProvider for FailingProvider {
        fn query(
            &self,
            _prompt: &str,
            _max_tokens: u32,
            _model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Err(AgentError::ManifestError("connection refused".into()))
        }

        fn name(&self) -> &str {
            "failing"
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

    fn default_variant() -> VariantSelection {
        VariantSelection {
            palette_id: "saas_midnight".into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn valid_saas_json() -> String {
        serde_json::json!({
            "sections": [
                {
                    "section_id": "hero",
                    "slots": {
                        "headline": "Build Faster with AI Power",
                        "subtitle": "The platform that helps developers ship 10x faster with intelligent automation.",
                        "cta_primary": "Start Free Trial"
                    }
                },
                {
                    "section_id": "features",
                    "slots": {
                        "heading": "Powerful Features",
                        "feature_1_icon": "rocket",
                        "feature_1_title": "Lightning Fast",
                        "feature_1_desc": "Deploy in seconds with our optimized pipeline.",
                        "feature_2_icon": "shield",
                        "feature_2_title": "Enterprise Security",
                        "feature_2_desc": "Bank-grade encryption protects your data at rest.",
                        "feature_3_icon": "chart",
                        "feature_3_title": "Real-time Analytics",
                        "feature_3_desc": "Monitor everything with live dashboards and alerts."
                    }
                },
                {
                    "section_id": "pricing",
                    "slots": {
                        "heading": "Simple Pricing",
                        "tier_1_name": "Starter",
                        "tier_1_price": "$9/mo",
                        "tier_1_features": "5 projects<br>1GB storage<br>Email support",
                        "tier_2_name": "Pro",
                        "tier_2_price": "$29/mo",
                        "tier_2_features": "Unlimited projects<br>10GB storage<br>Priority support",
                        "tier_3_name": "Enterprise",
                        "tier_3_price": "$99/mo",
                        "tier_3_features": "Everything in Pro<br>SSO<br>SLA<br>Dedicated support"
                    }
                },
                {
                    "section_id": "testimonials",
                    "slots": {
                        "heading": "What Our Users Say",
                        "testimonial_1_quote": "This tool saved us hundreds of hours.",
                        "testimonial_1_author": "Jane Smith",
                        "testimonial_1_role": "CTO at TechCorp",
                        "testimonial_2_quote": "The best developer tool we have ever used.",
                        "testimonial_2_author": "Alex Johnson",
                        "testimonial_2_role": "Lead Engineer at StartupCo",
                        "testimonial_3_quote": "Incredible speed and reliability every day.",
                        "testimonial_3_author": "Maria Garcia",
                        "testimonial_3_role": "VP Engineering at ScaleUp"
                    }
                },
                {
                    "section_id": "cta",
                    "slots": {
                        "headline": "Ready to Ship Faster?",
                        "body": "Join thousands of developers building better software.",
                        "cta_button": "Get Started Free"
                    }
                },
                {
                    "section_id": "footer",
                    "slots": {
                        "brand": "AcmeAI",
                        "copyright": "2026 AcmeAI Inc. All rights reserved."
                    }
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn test_generate_content_valid_response() {
        let schema = get_template_schema("saas_landing").unwrap();
        let provider = MockContentProvider::new(&valid_saas_json());
        let result = generate_content(
            "AI writing tool for marketers",
            "saas_landing",
            &schema,
            &default_variant(),
            &provider,
        );
        assert!(result.is_ok(), "generate_content failed: {result:?}");
        let payload = result.unwrap();
        assert_eq!(payload.template_id, "saas_landing");
        assert_eq!(payload.sections.len(), 6);
    }

    #[test]
    fn test_generate_content_model_unavailable() {
        let schema = get_template_schema("saas_landing").unwrap();
        let provider = FailingProvider;
        let result = generate_content(
            "test brief",
            "saas_landing",
            &schema,
            &default_variant(),
            &provider,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContentError::ModelUnavailable(_)
        ));
    }

    #[test]
    fn test_generate_content_empty_response() {
        let schema = get_template_schema("saas_landing").unwrap();
        let provider = MockContentProvider::new("");
        let result = generate_content(
            "test brief",
            "saas_landing",
            &schema,
            &default_variant(),
            &provider,
        );
        assert!(result.is_err());
    }
}
