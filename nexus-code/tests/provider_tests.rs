use nexus_code::llm::providers::anthropic::AnthropicProvider;
use nexus_code::llm::providers::claude_cli::ClaudeCliProvider;
use nexus_code::llm::providers::google::GoogleProvider;
use nexus_code::llm::{LlmProvider, ModelRouter, ModelSlot, ProviderRegistry, SlotConfig};
use std::sync::Arc;

#[test]
fn test_provider_registry_creation() {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ClaudeCliProvider::new()));
    registry.register(Box::new(AnthropicProvider::new()));
    registry.register(Box::new(
        nexus_code::llm::providers::create_openai_provider(),
    ));
    registry.register(Box::new(GoogleProvider::new()));
    registry.register(Box::new(
        nexus_code::llm::providers::create_ollama_provider(),
    ));
    registry.register(Box::new(
        nexus_code::llm::providers::create_openrouter_provider(),
    ));
    registry.register(Box::new(nexus_code::llm::providers::create_groq_provider()));
    registry.register(Box::new(
        nexus_code::llm::providers::create_deepseek_provider(),
    ));

    let list = registry.list();
    assert_eq!(list.len(), 8);
}

#[test]
fn test_claude_cli_provider() {
    let provider = ClaudeCliProvider::new();
    assert_eq!(provider.name(), "claude_cli");
    assert_eq!(provider.available_models(), vec!["claude-cli"]);
    // is_configured depends on whether `claude` binary is on PATH
}

#[test]
fn test_claude_cli_registered_and_resolvable() {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ClaudeCliProvider::new()));
    let registry = Arc::new(registry);

    let mut router = ModelRouter::new(registry);
    router.set_slot(
        ModelSlot::Execution,
        SlotConfig {
            provider: "claude_cli".to_string(),
            model: "claude-cli".to_string(),
        },
    );

    let result = router.resolve(ModelSlot::Execution);
    assert!(
        result.is_ok(),
        "claude_cli should be resolvable after registration: {:?}",
        result.err()
    );
    let (provider, model) = result.unwrap();
    assert_eq!(provider.name(), "claude_cli");
    assert_eq!(model, "claude-cli");
}

#[test]
fn test_anthropic_provider() {
    let provider = AnthropicProvider::new();
    assert_eq!(provider.name(), "anthropic");
    let expected = std::env::var("ANTHROPIC_API_KEY").is_ok();
    assert_eq!(provider.is_configured(), expected);
    assert!(provider
        .available_models()
        .contains(&"claude-sonnet-4-20250514"));
}

#[test]
fn test_openai_provider() {
    let provider = nexus_code::llm::providers::create_openai_provider();
    assert_eq!(provider.name(), "openai");
    let expected = std::env::var("OPENAI_API_KEY").is_ok();
    assert_eq!(provider.is_configured(), expected);
}

#[test]
fn test_ollama_always_configured() {
    let provider = nexus_code::llm::providers::create_ollama_provider();
    assert_eq!(provider.name(), "ollama");
    assert!(provider.is_configured()); // No API key required
}

#[test]
fn test_google_provider() {
    let provider = GoogleProvider::new();
    assert_eq!(provider.name(), "google");
    assert!(provider.available_models().contains(&"gemini-2.5-flash"));
}

#[test]
fn test_openrouter_provider() {
    let provider = nexus_code::llm::providers::create_openrouter_provider();
    assert_eq!(provider.name(), "openrouter");
    assert!(provider
        .available_models()
        .contains(&"anthropic/claude-sonnet-4"));
}

#[test]
fn test_groq_provider() {
    let provider = nexus_code::llm::providers::create_groq_provider();
    assert_eq!(provider.name(), "groq");
    assert!(provider
        .available_models()
        .contains(&"llama-3.3-70b-versatile"));
}

#[test]
fn test_deepseek_provider() {
    let provider = nexus_code::llm::providers::create_deepseek_provider();
    assert_eq!(provider.name(), "deepseek");
    assert!(provider.available_models().contains(&"deepseek-chat"));
}

#[test]
fn test_slot_configuration() {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(AnthropicProvider::new()));
    let registry = Arc::new(registry);

    let mut router = ModelRouter::new(registry);
    router.set_slot(
        ModelSlot::Execution,
        SlotConfig {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
        },
    );

    let result = router.resolve(ModelSlot::Execution);
    assert!(result.is_ok());
    let (provider, model) = result.unwrap();
    assert_eq!(provider.name(), "anthropic");
    assert_eq!(model, "claude-sonnet-4-20250514");
}

#[test]
fn test_slot_missing_provider() {
    let registry = Arc::new(ProviderRegistry::new());
    let mut router = ModelRouter::new(registry);
    router.set_slot(
        ModelSlot::Thinking,
        SlotConfig {
            provider: "nonexistent".to_string(),
            model: "model".to_string(),
        },
    );
    let result = router.resolve(ModelSlot::Thinking);
    assert!(result.is_err());
}

#[test]
fn test_slot_not_configured() {
    let registry = Arc::new(ProviderRegistry::new());
    let router = ModelRouter::new(registry);
    let result = router.resolve(ModelSlot::Vision);
    assert!(result.is_err());
}

#[test]
fn test_openai_compat_shared_base() {
    // All 5 OpenAI-compatible providers share the same struct type
    let openai = nexus_code::llm::providers::create_openai_provider();
    let ollama = nexus_code::llm::providers::create_ollama_provider();
    let groq = nexus_code::llm::providers::create_groq_provider();

    // They have different names but same type
    assert_ne!(openai.name(), ollama.name());
    assert_ne!(ollama.name(), groq.name());
}
