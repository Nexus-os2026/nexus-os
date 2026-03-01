#![cfg(feature = "real-api-tests")]

#[cfg(feature = "real-claude")]
use nexus_connectors_llm::providers::ClaudeProvider;
use nexus_connectors_llm::providers::{DeepSeekProvider, LlmProvider, OllamaProvider};

#[test]
fn test_deepseek_real_api_optional() {
    if std::env::var("ENABLE_REAL_API").ok().as_deref() != Some("1") {
        return;
    }
    if std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .as_deref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        return;
    }

    let provider = DeepSeekProvider::from_env();
    let result = provider.query("Reply with one short sentence.", 32, "deepseek-chat");
    assert!(result.is_ok());
}

#[test]
fn test_ollama_real_api_optional() {
    if std::env::var("ENABLE_REAL_API").ok().as_deref() != Some("1") {
        return;
    }
    if std::env::var("OLLAMA_URL").is_err() {
        return;
    }

    let provider = OllamaProvider::from_env();
    let result = provider.query("Say hello.", 16, "llama3");
    assert!(result.is_ok());
}

#[cfg(feature = "real-claude")]
#[test]
fn test_claude_real_api_optional() {
    if std::env::var("ENABLE_REAL_API").ok().as_deref() != Some("1") {
        return;
    }
    if std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .as_deref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        return;
    }

    let provider = ClaudeProvider::from_env();
    let result = provider.query("Reply with one short sentence.", 32, "claude-sonnet-4-5");
    assert!(result.is_ok());
}
