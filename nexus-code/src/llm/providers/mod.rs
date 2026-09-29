//! LLM provider implementations.

pub mod anthropic;
pub mod claude_cli;
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
            "o3-mini".to_string(),
        ],
        true,
    )
}

/// Create the Ollama provider (no API key required) at `OLLAMA_BASE_URL`,
/// the standalone terminal's setting, or the local default.
pub fn create_ollama_provider() -> OpenAiCompatibleProvider {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    create_ollama_provider_at(&base_url)
}

/// The Ollama API base URL a Nexus Code application uses, or `None` when
/// Ollama is unavailable. The standalone terminal (`cli_agents`) keeps
/// `OLLAMA_BASE_URL` (`ollama_base_url`) or the local default. The desktop
/// uses the desktop's own Ollama authority ([`desktop_ollama_api_base`]).
pub fn ollama_api_base(
    cli_agents: bool,
    ollama_url: Option<std::ffi::OsString>,
    ollama_base_url: Option<String>,
) -> Option<String> {
    if cli_agents {
        Some(ollama_base_url.unwrap_or_else(|| "http://localhost:11434/v1".to_string()))
    } else {
        desktop_ollama_api_base(ollama_url)
    }
}

/// Final Gate item B: the Ollama address of the desktop's Nexus Code comes
/// from the same authority as the rest of the desktop: the operator's
/// `OLLAMA_URL` (`ollama_url`), an http(s) base URL with a host and no user
/// information, query or fragment, or else the fixed
/// `http://localhost:11434`. A set but unusable value leaves Ollama
/// unavailable (`None`); nothing falls back to the default, and
/// `OLLAMA_BASE_URL` is not read. The OpenAI-compatible API is under `/v1`.
pub fn desktop_ollama_api_base(ollama_url: Option<std::ffi::OsString>) -> Option<String> {
    let base = match ollama_url {
        None => "http://localhost:11434".to_string(),
        Some(value) => {
            let url = nexus_kernel::governed_http::http_url(value.to_str()?).ok()?;
            if url.query().is_some() || url.fragment().is_some() {
                return None;
            }
            url.as_str().trim_end_matches('/').to_string()
        }
    };
    Some(format!("{base}/v1"))
}

/// Create the Ollama provider (no API key required) at `base_url`, its
/// OpenAI-compatible API base.
pub fn create_ollama_provider_at(base_url: &str) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        "ollama",
        base_url,
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
