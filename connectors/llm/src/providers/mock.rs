use super::{LlmProvider, LlmResponse};
use nexus_kernel::errors::AgentError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        Self
    }
}

impl LlmProvider for MockProvider {
    fn query(
        &self,
        _prompt: &str,
        max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            output_text: "Mock provider response".to_string(),
            token_count: max_tokens.min(64),
            model_name: model.to_string(),
            tool_calls: Vec::new(),
        })
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn cost_per_token(&self) -> f64 {
        0.0
    }
}
