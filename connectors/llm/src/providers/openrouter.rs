use super::openai_compatible::{
    build_openai_chat_request, execute_openai_compatible_query, OpenAiCompatibleQuery,
};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRouterProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://openrouter.ai/api/v1/chat/completions".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("OPENROUTER_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1/chat/completions".to_string());
        let mut provider = Self::new(env::var("OPENROUTER_API_KEY").ok());
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
            .or_else(|| env::var("OPENROUTER_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for OpenRouterProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        let referer = env::var("OPENROUTER_HTTP_REFERER")
            .unwrap_or_else(|_| "https://nexus-os.dev".to_string());
        let title = env::var("OPENROUTER_X_TITLE").unwrap_or_else(|_| "NEXUS OS".to_string());

        execute_openai_compatible_query(OpenAiCompatibleQuery {
            provider_name: "openrouter",
            missing_key_error: "OPENROUTER_API_KEY is not set",
            api_key: self.api_key(),
            endpoint: &self.endpoint,
            prompt,
            max_tokens,
            model,
            extra_headers: &[("http-referer", referer), ("x-title", title)],
        })
    }

    fn name(&self) -> &str {
        "openrouter"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_002
    }

    fn endpoint_url(&self) -> String {
        "https://openrouter.ai".to_string()
    }
}
