use super::openai_compatible::{
    build_openai_chat_request, execute_openai_compatible_query, OpenAiCompatibleQuery,
};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerplexityProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl PerplexityProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.perplexity.ai/chat/completions".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("PERPLEXITY_URL")
            .unwrap_or_else(|_| "https://api.perplexity.ai/chat/completions".to_string());
        let mut provider = Self::new(env::var("PERPLEXITY_API_KEY").ok());
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
            .or_else(|| env::var("PERPLEXITY_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for PerplexityProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        execute_openai_compatible_query(OpenAiCompatibleQuery {
            provider_name: "perplexity",
            missing_key_error: "PERPLEXITY_API_KEY is not set",
            api_key: self.api_key(),
            endpoint: &self.endpoint,
            prompt,
            max_tokens,
            model,
            extra_headers: &[],
        })
    }

    fn name(&self) -> &str {
        "perplexity"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_003
    }

    fn endpoint_url(&self) -> String {
        "https://api.perplexity.ai".to_string()
    }
}
