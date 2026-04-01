//! LLM provider implementations.

pub mod anthropic;
pub mod google;
pub mod openai_compat;

use openai_compat::OpenAiCompatibleProvider;

/// Create the OpenAI provider.
pub fn create_openai_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "openai",
        "https://api.openai.com/v1",
        "OPENAI_API_KEY",
        "gpt-4o",
        vec![],
        vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "o3".to_string(),
            "o4-mini".to_string(),
        ],
        true,
    )
}

/// Create the Ollama provider (no API key required).
pub fn create_ollama_provider() -> OpenAiCompatibleProvider {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    OpenAiCompatibleProvider::new(
        "ollama",
        &base_url,
        "OLLAMA_API_KEY",
        "qwen3:8b",
        vec![],
        vec![
            "qwen3:8b".to_string(),
            "llama3.1:8b".to_string(),
            "codellama:13b".to_string(),
            "deepseek-coder-v2:16b".to_string(),
        ],
        false,
    )
}

/// Create the OpenRouter provider.
pub fn create_openrouter_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "openrouter",
        "https://openrouter.ai/api/v1",
        "OPENROUTER_API_KEY",
        "anthropic/claude-sonnet-4",
        vec![
            (
                "HTTP-Referer".to_string(),
                "https://nexus-os.dev".to_string(),
            ),
            ("X-Title".to_string(), "Nexus Code".to_string()),
        ],
        vec![
            "anthropic/claude-sonnet-4".to_string(),
            "openai/gpt-4o".to_string(),
            "google/gemini-2.5-flash".to_string(),
            "deepseek/deepseek-r1".to_string(),
            "meta-llama/llama-3.1-70b-instruct".to_string(),
        ],
        true,
    )
}

/// Create the Groq provider.
pub fn create_groq_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "groq",
        "https://api.groq.com/openai/v1",
        "GROQ_API_KEY",
        "llama-3.3-70b-versatile",
        vec![],
        vec![
            "llama-3.3-70b-versatile".to_string(),
            "llama-3.1-8b-instant".to_string(),
            "mixtral-8x7b-32768".to_string(),
            "gemma2-9b-it".to_string(),
        ],
        true,
    )
}

/// Create the DeepSeek provider.
pub fn create_deepseek_provider() -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "deepseek",
        "https://api.deepseek.com",
        "DEEPSEEK_API_KEY",
        "deepseek-chat",
        vec![],
        vec!["deepseek-chat".to_string(), "deepseek-reasoner".to_string()],
        true,
    )
}
