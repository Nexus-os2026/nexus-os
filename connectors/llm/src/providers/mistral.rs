use super::openai_compatible::{
    build_openai_chat_request, execute_openai_compatible_query, OpenAiCompatibleQuery,
};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MistralProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl MistralProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.mistral.ai/v1/chat/completions".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("MISTRAL_URL")
            .unwrap_or_else(|_| "https://api.mistral.ai/v1/chat/completions".to_string());
        let mut provider = Self::new(env::var("MISTRAL_API_KEY").ok());
        provider.endpoint = endpoint;
        provider
    }

    pub fn build_request(&self, prompt: &str, max_tokens: u32, model: &str) -> ProviderRequest {
        build_openai_chat_request(
            &self.endpoint,
            self.api_key.clone().unwrap_or_default().as_str(),
            prompt,
            max_tokens,
            model,
        )
    }

    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| env::var("MISTRAL_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for MistralProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        execute_openai_compatible_query(OpenAiCompatibleQuery {
            provider_name: "mistral",
            missing_key_error: "MISTRAL_API_KEY is not set",
            api_key: self.api_key(),
            endpoint: &self.endpoint,
            prompt,
            max_tokens,
            model,
            extra_headers: &[],
        })
    }

    fn name(&self) -> &str {
        "mistral"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_002_5
    }

    fn endpoint_url(&self) -> String {
        "https://api.mistral.ai".to_string()
    }
}
