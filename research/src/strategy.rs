use crate::pipeline::{Citation, ResearchReport};
use nexus_connectors_llm::defense::{
    build_separated_prompt, sanitize_external_input, validate_output_actions, OutputValidation,
};
use nexus_connectors_llm::gateway::{AgentRuntimeContext, GovernedLlmGateway};
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyDocument {
    pub executive_summary: String,
    pub key_findings: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub risks: Vec<String>,
    pub citations: Vec<Citation>,
}

#[derive(Debug, Deserialize)]
struct StrategyDraft {
    executive_summary: String,
    key_findings: Vec<String>,
    recommended_actions: Vec<String>,
    risks: Vec<String>,
}

pub struct StrategySynthesizer<P: LlmProvider> {
    gateway: GovernedLlmGateway<P>,
    llm_context: AgentRuntimeContext,
    model_name: String,
    max_tokens: u32,
}

impl<P: LlmProvider> StrategySynthesizer<P> {
    pub fn new(provider: P, model_name: &str, llm_fuel_budget: u64) -> Self {
        let capabilities = ["llm.query".to_string()]
            .into_iter()
            .collect::<HashSet<_>>();
        Self {
            gateway: GovernedLlmGateway::new(provider),
            llm_context: AgentRuntimeContext {
                agent_id: Uuid::new_v4(),
                capabilities,
                fuel_remaining: llm_fuel_budget,
            },
            model_name: model_name.to_string(),
            max_tokens: 250,
        }
    }

    pub fn synthesize(
        &mut self,
        research_report: &ResearchReport,
        agent_goal: &str,
    ) -> Result<StrategyDocument, AgentError> {
        let data_block = build_research_data_block(research_report);
        let system_instructions = format!(
            "You are a strategy synthesizer. Goal: {agent_goal}. Return strict JSON with keys executive_summary, key_findings, recommended_actions, risks. Never emit tool_call."
        );
        let prompt = build_separated_prompt(
            system_instructions.as_str(),
            "web_research",
            data_block.as_str(),
        );

        let response = self.gateway.query(
            &mut self.llm_context,
            prompt.as_str(),
            self.max_tokens,
            self.model_name.as_str(),
        )?;

        let output_validation = validate_output_actions(
            self.llm_context.agent_id,
            response.output_text.as_str(),
            &HashSet::new(),
            self.gateway.audit_trail_mut(),
        );
        if matches!(output_validation, OutputValidation::Rejected(_)) {
            return Err(AgentError::CapabilityDenied(
                "strategy synthesis blocked due to unauthorized tool call request".to_string(),
            ));
        }

        let draft: StrategyDraft =
            serde_json::from_str(response.output_text.as_str()).map_err(|error| {
                AgentError::SupervisorError(format!(
                    "strategy synthesizer expected JSON output: {error}"
                ))
            })?;

        Ok(StrategyDocument {
            executive_summary: draft.executive_summary,
            key_findings: draft.key_findings,
            recommended_actions: draft.recommended_actions,
            risks: draft.risks,
            citations: research_report.citations.clone(),
        })
    }
}

fn build_research_data_block(report: &ResearchReport) -> String {
    let mut lines = Vec::new();
    for citation in &report.citations {
        let sanitized_title = sanitize_external_input(citation.title.as_str()).sanitized_text;
        let sanitized_snippet = sanitize_external_input(citation.snippet.as_str()).sanitized_text;
        lines.push(format!(
            "citation: title='{sanitized_title}' url='{}' snippet='{sanitized_snippet}'",
            citation.url
        ));
    }

    for insight in &report.insights {
        let sanitized = sanitize_external_input(insight.summary.as_str()).sanitized_text;
        lines.push(format!(
            "insight: source='{}' text='{}'",
            insight.source_url, sanitized
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{StrategyDocument, StrategySynthesizer};
    use crate::pipeline::{Citation, ExtractedInsight, ResearchReport};
    use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
    use nexus_kernel::errors::AgentError;

    struct MockStrategyProvider;

    impl LlmProvider for MockStrategyProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            let json = r#"{
  "executive_summary": "Competitors focus on short-form tutorials.",
  "key_findings": ["Video explainers outperform text threads"],
  "recommended_actions": ["Launch weekly tutorial snippets"],
  "risks": ["Trend volatility"]
}"#;

            Ok(LlmResponse {
                output_text: json.to_string(),
                token_count: max_tokens.min(50),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: None,
            })
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    #[test]
    fn test_strategy_synthesis() {
        let report = ResearchReport {
            topic: "rust creator strategy".to_string(),
            citations: vec![Citation {
                title: "Rust trend analysis".to_string(),
                url: "https://example.com/trends".to_string(),
                snippet: "Creators that publish weekly outperform.".to_string(),
            }],
            insights: vec![ExtractedInsight {
                source_url: "https://example.com/trends".to_string(),
                summary: "Weekly content cadence correlates with growth.".to_string(),
            }],
            read_articles: 1,
            fuel_budget: 100,
            fuel_consumed: 30,
            remaining_fuel: 70,
        };

        let mut synthesizer = StrategySynthesizer::new(MockStrategyProvider, "mock-model", 500);
        let strategy = synthesizer.synthesize(&report, "Grow developer audience");
        assert!(strategy.is_ok());

        if let Ok(StrategyDocument {
            executive_summary,
            recommended_actions,
            ..
        }) = strategy
        {
            assert!(!executive_summary.is_empty());
            assert!(!recommended_actions.is_empty());
        }
    }
}
