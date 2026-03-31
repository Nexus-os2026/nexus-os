use super::openai_compatible::{
    build_openai_chat_request, execute_openai_compatible_query, OpenAiCompatibleQuery,
};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use std::env;

/// Available Groq models — free tier with generous rate limits.
pub const GROQ_MODELS: &[(&str, &str)] = &[
    (
        "llama-3.3-70b-versatile",
        "Llama 3.3 70B Versatile — Best all-round, 128K ctx",
    ),
    (
        "llama-3.1-70b-versatile",
        "Llama 3.1 70B Versatile — Stable workhorse, 128K ctx",
    ),
    (
        "llama-3.1-8b-instant",
        "Llama 3.1 8B Instant — Ultra-fast small model, 128K ctx",
    ),
    (
        "gemma2-9b-it",
        "Gemma 2 9B IT — Google's efficient instruction-tuned",
    ),
    ("mistral-7b", "Mistral 7B — Fast general-purpose"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroqProvider {
    api_key: Option<String>,
    endpoint: String,
}

impl GroqProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            endpoint: "https://api.groq.com/openai/v1/chat/completions".to_string(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = env::var("GROQ_URL")
            .unwrap_or_else(|_| "https://api.groq.com/openai/v1/chat/completions".to_string());
        // Optional: API key may not be configured in environment
        let mut provider = Self::new(env::var("GROQ_API_KEY").ok());
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
            .or_else(|| env::var("GROQ_API_KEY").ok())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
    }
}

impl LlmProvider for GroqProvider {
    fn query(&self, prompt: &str, max_tokens: u32, model: &str) -> Result<LlmResponse, AgentError> {
        execute_openai_compatible_query(OpenAiCompatibleQuery {
            provider_name: "groq",
            missing_key_error: "GROQ_API_KEY is not set",
            api_key: self.api_key(),
            endpoint: &self.endpoint,
            prompt,
            max_tokens,
            model,
            extra_headers: &[],
        })
    }

    fn name(&self) -> &str {
        "groq"
    }

    fn cost_per_token(&self) -> f64 {
        0.000_000_6
    }

    fn endpoint_url(&self) -> String {
        "https://api.groq.com".to_string()
    }
}
