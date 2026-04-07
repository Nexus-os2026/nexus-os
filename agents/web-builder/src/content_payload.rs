//! Content Payload — intermediate structure carrying LLM-generated content through the pipeline.
//!
//! `ContentPayload` is the validated bridge between the LLM's JSON response and the
//! assembler. Every slot value is validated against the Phase 6.1 slot schema before
//! it reaches HTML injection.

use crate::slot_schema::{validate_section_payload, TemplateSchema};
use crate::variant::VariantSelection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("invalid JSON from LLM: {reason}")]
    InvalidJson { reason: String, raw: String },
    #[error("validation failed: {0:?}")]
    ValidationFailed(Vec<String>),
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("unknown template: {0}")]
    UnknownTemplate(String),
}

// ─── Payload Types ──────────────────────────────────────────────────────────

/// The complete payload for assembling a personalized page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPayload {
    pub template_id: String,
    pub variant: VariantSelection,
    pub sections: Vec<SectionContent>,
}

/// Content for a single template section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionContent {
    pub section_id: String,
    pub slots: HashMap<String, String>,
}

// ─── LLM Response Shape ─────────────────────────────────────────────────────

/// The JSON shape we ask the LLM to produce (minimal wrapper for parsing).
#[derive(Debug, Deserialize)]
struct LlmSectionsResponse {
    sections: Vec<LlmSectionEntry>,
}

#[derive(Debug, Deserialize)]
struct LlmSectionEntry {
    section_id: String,
    slots: HashMap<String, String>,
}

// ─── Implementation ─────────────────────────────────────────────────────────

impl ContentPayload {
    /// Validate every section against the template schema.
    /// Returns ALL validation errors, not just the first.
    pub fn validate(&self, schema: &TemplateSchema) -> Result<(), Vec<String>> {
        let mut all_errors: Vec<String> = Vec::new();

        for sc in &self.sections {
            // Find the matching section schema
            let section_schema = schema
                .sections
                .iter()
                .find(|s| s.section_id == sc.section_id);
            match section_schema {
                Some(ss) => {
                    if let Err(slot_errors) = validate_section_payload(&sc.slots, ss) {
                        for e in slot_errors {
                            all_errors.push(format!("[{}] {}", sc.section_id, e));
                        }
                    }
                }
                None => {
                    all_errors.push(format!("unknown section: '{}'", sc.section_id));
                }
            }
        }

        // Check for required sections that have no content
        for ss in &schema.sections {
            let has_required = ss.slots.values().any(|c| c.required);
            if has_required
                && !self
                    .sections
                    .iter()
                    .any(|sc| sc.section_id == ss.section_id)
            {
                all_errors.push(format!(
                    "missing section '{}' which has required slots",
                    ss.section_id
                ));
            }
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }

    /// Parse an LLM JSON response into a validated ContentPayload.
    ///
    /// Handles malformed JSON gracefully — never panics.
    pub fn from_llm_response(
        raw: &str,
        template_id: &str,
        schema: &TemplateSchema,
        variant: VariantSelection,
    ) -> Result<Self, ContentError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ContentError::InvalidJson {
                reason: "empty response".into(),
                raw: raw.to_string(),
            });
        }

        // Strip markdown fences if present
        let json_text = strip_markdown_fences(trimmed);

        // Try to parse, then try repair for truncated JSON
        let parsed: LlmSectionsResponse = match serde_json::from_str(json_text) {
            Ok(p) => p,
            Err(e) => {
                let repaired = crate::plan::repair_truncated_json(json_text);
                serde_json::from_str(&repaired).map_err(|e2| ContentError::InvalidJson {
                    reason: format!("parse failed: {e}; repair also failed: {e2}"),
                    raw: raw.to_string(),
                })?
            }
        };

        let sections = parsed
            .sections
            .into_iter()
            .map(|entry| SectionContent {
                section_id: entry.section_id,
                slots: entry.slots,
            })
            .collect();

        let payload = ContentPayload {
            template_id: template_id.to_string(),
            variant,
            sections,
        };

        // Validate against schema
        if let Err(errors) = payload.validate(schema) {
            return Err(ContentError::ValidationFailed(errors));
        }

        Ok(payload)
    }
}

/// Strip markdown code fences (`\`\`\`json ... \`\`\`` or `\`\`\` ... \`\`\``).
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

/// Format slot errors into human-readable strings for corrective prompts.
pub fn format_validation_errors(errors: &[String]) -> String {
    errors
        .iter()
        .map(|e| format!("- {e}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot_schema::get_template_schema;
    use crate::variant::MotionProfile;

    fn default_variant() -> VariantSelection {
        VariantSelection {
            palette_id: "saas_midnight".into(),
            typography_id: "modern".into(),
            layout: HashMap::new(),
            motion: MotionProfile::Subtle,
        }
    }

    fn saas_schema() -> TemplateSchema {
        get_template_schema("saas_landing").unwrap()
    }

    fn valid_saas_sections() -> Vec<SectionContent> {
        vec![
            SectionContent {
                section_id: "hero".into(),
                slots: HashMap::from([
                    ("headline".into(), "Build Faster with AI Power".into()),
                    ("subtitle".into(), "The platform that helps developers ship 10x faster with intelligent automation.".into()),
                    ("cta_primary".into(), "Start Free Trial".into()),
                ]),
            },
            SectionContent {
                section_id: "features".into(),
                slots: HashMap::from([
                    ("heading".into(), "Powerful Features".into()),
                    ("feature_1_icon".into(), "rocket".into()),
                    ("feature_1_title".into(), "Lightning Fast".into()),
                    ("feature_1_desc".into(), "Deploy in seconds with our optimized pipeline.".into()),
                    ("feature_2_icon".into(), "shield".into()),
                    ("feature_2_title".into(), "Enterprise Security".into()),
                    ("feature_2_desc".into(), "Bank-grade encryption protects your data at rest.".into()),
                    ("feature_3_icon".into(), "chart".into()),
                    ("feature_3_title".into(), "Real-time Analytics".into()),
                    ("feature_3_desc".into(), "Monitor everything with live dashboards and alerts.".into()),
                ]),
            },
            SectionContent {
                section_id: "pricing".into(),
                slots: HashMap::from([
                    ("heading".into(), "Simple Pricing".into()),
                    ("tier_1_name".into(), "Starter".into()),
                    ("tier_1_price".into(), "$9/mo".into()),
                    ("tier_1_features".into(), "5 projects<br>1GB storage<br>Email support".into()),
                    ("tier_2_name".into(), "Pro".into()),
                    ("tier_2_price".into(), "$29/mo".into()),
                    ("tier_2_features".into(), "Unlimited projects<br>10GB storage<br>Priority support".into()),
                    ("tier_3_name".into(), "Enterprise".into()),
                    ("tier_3_price".into(), "$99/mo".into()),
                    ("tier_3_features".into(), "Everything in Pro<br>SSO<br>SLA<br>Dedicated support".into()),
                ]),
            },
            SectionContent {
                section_id: "testimonials".into(),
                slots: HashMap::from([
                    ("heading".into(), "What Our Users Say".into()),
                    ("testimonial_1_quote".into(), "This tool saved us hundreds of hours.".into()),
                    ("testimonial_1_author".into(), "Jane Smith".into()),
                    ("testimonial_1_role".into(), "CTO at TechCorp".into()),
                    ("testimonial_2_quote".into(), "The best developer tool we have ever used.".into()),
                    ("testimonial_2_author".into(), "Alex Johnson".into()),
                    ("testimonial_2_role".into(), "Lead Engineer at StartupCo".into()),
                    ("testimonial_3_quote".into(), "Incredible speed and reliability every day.".into()),
                    ("testimonial_3_author".into(), "Maria Garcia".into()),
                    ("testimonial_3_role".into(), "VP Engineering at ScaleUp".into()),
                ]),
            },
            SectionContent {
                section_id: "cta".into(),
                slots: HashMap::from([
                    ("headline".into(), "Ready to Ship Faster?".into()),
                    ("body".into(), "Join thousands of developers building better software.".into()),
                    ("cta_button".into(), "Get Started Free".into()),
                ]),
            },
            SectionContent {
                section_id: "footer".into(),
                slots: HashMap::from([
                    ("brand".into(), "AcmeAI".into()),
                    ("copyright".into(), "2026 AcmeAI Inc. All rights reserved.".into()),
                ]),
            },
        ]
    }

    #[test]
    fn test_validate_payload_all_required_slots_present() {
        let schema = saas_schema();
        let payload = ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant(),
            sections: valid_saas_sections(),
        };
        assert!(payload.validate(&schema).is_ok());
    }

    #[test]
    fn test_validate_payload_missing_required_slot() {
        let schema = saas_schema();
        let payload = ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant(),
            sections: vec![SectionContent {
                section_id: "hero".into(),
                slots: HashMap::from([
                    // Missing "headline" (required)
                    ("subtitle".into(), "A valid subtitle for testing.".into()),
                    ("cta_primary".into(), "Start Free Trial".into()),
                ]),
            }],
        };
        let err = payload.validate(&schema).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("headline")),
            "should mention missing headline: {err:?}"
        );
    }

    #[test]
    fn test_validate_payload_slot_exceeds_max_chars() {
        let schema = saas_schema();
        let mut sections = valid_saas_sections();
        // hero headline max is 80 chars
        sections[0].slots.insert("headline".into(), "x".repeat(100));
        let payload = ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant(),
            sections,
        };
        let err = payload.validate(&schema).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("headline") && e.contains("80")),
            "should mention headline max: {err:?}"
        );
    }

    #[test]
    fn test_validate_payload_unknown_section() {
        let schema = saas_schema();
        let payload = ContentPayload {
            template_id: "saas_landing".into(),
            variant: default_variant(),
            sections: vec![SectionContent {
                section_id: "nonexistent_section".into(),
                slots: HashMap::new(),
            }],
        };
        let err = payload.validate(&schema).unwrap_err();
        assert!(err.iter().any(|e| e.contains("nonexistent_section")));
    }

    #[test]
    fn test_from_llm_response_valid_json() {
        let schema = saas_schema();
        let json = serde_json::json!({
            "sections": valid_saas_sections().iter().map(|s| {
                serde_json::json!({
                    "section_id": s.section_id,
                    "slots": s.slots,
                })
            }).collect::<Vec<_>>()
        });
        let raw = serde_json::to_string(&json).unwrap();
        let result =
            ContentPayload::from_llm_response(&raw, "saas_landing", &schema, default_variant());
        assert!(result.is_ok(), "should parse valid JSON: {result:?}");
        let payload = result.unwrap();
        assert_eq!(payload.sections.len(), 6);
    }

    #[test]
    fn test_from_llm_response_malformed_json() {
        let schema = saas_schema();
        let raw = "{ this is not valid json at all !!!";
        let result =
            ContentPayload::from_llm_response(raw, "saas_landing", &schema, default_variant());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContentError::InvalidJson { .. }
        ));
    }

    #[test]
    fn test_from_llm_response_empty_string() {
        let schema = saas_schema();
        let result =
            ContentPayload::from_llm_response("", "saas_landing", &schema, default_variant());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ContentError::InvalidJson { .. }
        ));
    }

    #[test]
    fn test_from_llm_response_json_with_extra_fields() {
        let schema = saas_schema();
        let sections: Vec<serde_json::Value> = valid_saas_sections()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "section_id": s.section_id,
                    "slots": s.slots,
                    "extra_field": "should be ignored",
                })
            })
            .collect();
        let json = serde_json::json!({
            "sections": sections,
            "bonus_key": 42,
        });
        let raw = serde_json::to_string(&json).unwrap();
        let result =
            ContentPayload::from_llm_response(&raw, "saas_landing", &schema, default_variant());
        assert!(result.is_ok(), "should ignore extra fields: {result:?}");
    }
}
