use super::openai_compatible::{
    build_openai_chat_request, execute_openai_compatible_query, OpenAiCompatibleQuery,
};
use super::{LlmProvider, LlmResponse, ProviderRequest};
use nexus_kernel::errors::AgentError;
use std::env;

/// Popular OpenRouter models — access 200+ models from every major provider.
pub const OPENROUTER_MODELS: &[(&str, &str)] = &[
    (
        "qwen/qwen3.6-plus:free",
        "Qwen 3.6 Plus — Free, 1M ctx, best for agentic coding & web builds",
    ),
    (
        "meta-llama/llama-3.3-70b-instruct:free",
        "Llama 3.3 70B Free — Zero cost, rate-limited",
    ),
    (
        "meta-llama/llama-3.3-70b-instruct",
        "Llama 3.3 70B — Free tier, strong all-round",
    ),
    (
        "deepseek/deepseek-coder-v3",
        "DeepSeek Coder V3 — Best open-source code model",
    ),
    (
        "xiaomi/mimo-v2-flash",
        "MiMo V2 Flash — Ultra-fast reasoning",
    ),
    (
        "xiaomi/mimo-v2-pro",
        "MiMo V2 Pro — Advanced reasoning + math",
    ),
    ("openai/gpt-4.1-mini", "GPT-4.1 Mini — Strong mid-tier"),
    ("openai/gpt-4.1", "GPT-4.1 — Best coder"),
    ("openai/gpt-4o-mini", "GPT-4o Mini — Fast, affordable"),
    ("openai/gpt-4o", "GPT-4o — Capable multi-modal"),
    (
        "anthropic/claude-sonnet-4-6",
        "Claude Sonnet 4.6 — Latest, best value",
    ),
    (
        "anthropic/claude-sonnet-4-20250514",
        "Claude Sonnet 4 — Balanced intelligence",
    ),
    (
        "google/gemini-2.5-flash-preview",
        "Gemini 2.5 Flash — Google's fast model",
    ),
];

/// Resolve model aliases to OpenRouter model IDs.
pub fn resolve_alias(alias: &str) -> &str {
    match alias {
        "fast" => "xiaomi/mimo-v2-flash",
        "smart" => "xiaomi/mimo-v2-pro",
        "code" => "deepseek/deepseek-coder-v3",
        "free" => "qwen/qwen3.6-plus:free",
        "llama" => "meta-llama/llama-3.3-70b-instruct:free",
        other => other,
    }
}

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
        // Optional: API key may not be configured in environment
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
