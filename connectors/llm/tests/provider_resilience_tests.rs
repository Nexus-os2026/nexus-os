//! Provider resilience tests — verify every LLM provider handles "not available" gracefully.
//!
//! These tests run WITHOUT any running LLM backends. They verify that:
//! - Ollama connection refused → clean Err, no panic
//! - Flash without loaded model → clean Err, no panic
//! - All cloud providers with invalid/empty API keys → clean Err, no panic
//! - Gateway `select_provider` with no config → clean Err, no panic
//! - Provider selection priority is correct

use nexus_connectors_llm::gateway::{select_provider, ProviderSelectionConfig};
use nexus_connectors_llm::providers::*;

// ── Group 1: Ollama resilience ──────────────────────────────────────────

#[test]
fn test_ollama_health_check_connection_refused() {
    // Point at a port where nothing is listening
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let result = provider.health_check();
    assert!(
        result.is_err(),
        "health_check should fail when nothing is listening"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not reachable") || err.contains("refused") || err.contains("Connection"),
        "error should mention unreachable: got {err}"
    );
}

#[test]
fn test_ollama_query_connection_refused_returns_error_not_panic() {
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let result = provider.query("test prompt", 100, "llama3.2");
    assert!(
        result.is_err(),
        "query should fail when Ollama is not running"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not running") || err.contains("not reachable"),
        "error should mention Ollama not running: got {err}"
    );
}

#[test]
fn test_ollama_list_models_connection_refused() {
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let result = provider.list_models();
    assert!(
        result.is_err(),
        "list_models should fail when Ollama is not running"
    );
}

#[test]
fn test_ollama_embed_connection_refused() {
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let result = provider.embed(&["hello"], "nomic-embed-text");
    assert!(
        result.is_err(),
        "embed should fail when Ollama is not running"
    );
}

#[test]
fn test_ollama_query_with_image_connection_refused() {
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let result = provider.query_with_image("what is this?", "ZmFrZQ==", "llava");
    assert!(
        result.is_err(),
        "vision query should fail when Ollama is not running"
    );
}

#[test]
fn test_ollama_from_env_defaults_to_localhost() {
    // Without OLLAMA_URL set, should default to localhost:11434
    let provider = OllamaProvider::from_env();
    assert!(
        provider.base_url().contains("localhost") || provider.base_url().contains("127.0.0.1"),
        "default should be localhost"
    );
}

// ── Group 2: Flash provider resilience ──────────────────────────────────

#[test]
fn test_flash_provider_query_without_feature_returns_error() {
    // Without flash-infer feature, query must return Err
    #[cfg(not(feature = "flash-infer"))]
    {
        let provider = FlashProvider::new("nonexistent-model.gguf".into());
        let result = provider.query("test", 100, "");
        assert!(
            result.is_err(),
            "Flash query without feature should return error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Flash") || err.contains("flash"),
            "error should mention Flash: got {err}"
        );
    }
}

#[test]
fn test_flash_provider_try_query_without_feature() {
    #[cfg(not(feature = "flash-infer"))]
    {
        let provider = FlashProvider::new("nonexistent.gguf".into());
        let result = provider.try_query("test", 100, "");
        assert!(result.is_err());
    }
}

#[test]
fn test_flash_provider_metadata() {
    #[cfg(not(feature = "flash-infer"))]
    let provider = FlashProvider::new("test.gguf".into());
    #[cfg(feature = "flash-infer")]
    let provider = FlashProvider::new(
        "test.gguf".into(),
        nexus_flash_infer::LoadConfig::default(),
        nexus_llama_bridge::GenerationConfig::fast(),
    );
    assert_eq!(provider.name(), "flash");
    assert_eq!(provider.cost_per_token(), 0.0);
    assert!(!provider.is_paid());
    assert_eq!(provider.endpoint_url(), "local://flash-infer");
}

// ── Group 3: Cloud provider resilience (invalid API keys) ───────────────

#[test]
fn test_deepseek_invalid_key_returns_error() {
    let provider = DeepSeekProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "deepseek-chat");
    assert!(
        result.is_err(),
        "DeepSeek with invalid key should return error"
    );
}

#[test]
fn test_deepseek_empty_key_returns_error() {
    let provider = DeepSeekProvider::new(Some(String::new()));
    let result = provider.query("test", 100, "deepseek-chat");
    assert!(
        result.is_err(),
        "DeepSeek with empty key should return error"
    );
}

#[test]
fn test_openai_invalid_key_returns_error() {
    let provider = OpenAiProvider::new(Some("sk-invalid".into()));
    let result = provider.query("test", 100, "gpt-4");
    assert!(
        result.is_err(),
        "OpenAI with invalid key should return error"
    );
}

#[test]
fn test_gemini_invalid_key_returns_error() {
    let provider = GeminiProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "gemini-2.0-flash");
    assert!(
        result.is_err(),
        "Gemini with invalid key should return error"
    );
}

#[test]
fn test_groq_invalid_key_returns_error() {
    let provider = GroqProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "llama-3.3-70b-versatile");
    assert!(result.is_err(), "Groq with invalid key should return error");
}

#[test]
fn test_nvidia_invalid_key_returns_error() {
    let provider = NvidiaProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "nvidia/llama-3.1-nemotron-70b-instruct");
    assert!(
        result.is_err(),
        "NVIDIA NIM with invalid key should return error"
    );
}

#[test]
fn test_cohere_invalid_key_returns_error() {
    let provider = CohereProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "command-r-plus");
    assert!(
        result.is_err(),
        "Cohere with invalid key should return error"
    );
}

#[test]
fn test_mistral_invalid_key_returns_error() {
    let provider = MistralProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "mistral-large-latest");
    assert!(
        result.is_err(),
        "Mistral with invalid key should return error"
    );
}

#[test]
fn test_together_invalid_key_returns_error() {
    let provider = TogetherProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "meta-llama/Llama-3-70b-chat-hf");
    assert!(
        result.is_err(),
        "Together with invalid key should return error"
    );
}

#[test]
fn test_fireworks_invalid_key_returns_error() {
    let provider = FireworksProvider::new(Some("invalid-key".into()));
    let result = provider.query(
        "test",
        100,
        "accounts/fireworks/models/llama-v3p1-70b-instruct",
    );
    assert!(
        result.is_err(),
        "Fireworks with invalid key should return error"
    );
}

#[test]
fn test_perplexity_invalid_key_returns_error() {
    let provider = PerplexityProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "llama-3.1-sonar-large-128k-online");
    assert!(
        result.is_err(),
        "Perplexity with invalid key should return error"
    );
}

#[test]
fn test_openrouter_invalid_key_returns_error() {
    let provider = OpenRouterProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "openai/gpt-4");
    assert!(
        result.is_err(),
        "OpenRouter with invalid key should return error"
    );
}

#[test]
fn test_claude_invalid_key_returns_error() {
    let provider = ClaudeProvider::new(Some("invalid-key".into()));
    let result = provider.query("test", 100, "claude-sonnet-4-20250514");
    assert!(
        result.is_err(),
        "Claude with invalid key should return error"
    );
}

// ── Group 4: Mock provider always works ─────────────────────────────────

#[test]
fn test_mock_provider_always_succeeds() {
    let provider = MockProvider::new();
    let result = provider.query("test prompt", 100, "mock-1");
    assert!(result.is_ok(), "Mock provider should always succeed");
    let response = result.unwrap();
    assert!(!response.output_text.is_empty());
    assert_eq!(response.model_name, "mock-1");
}

#[test]
fn test_mock_provider_embed_succeeds() {
    let provider = MockProvider::new();
    let result = provider.embed(&["hello", "world"], "mock-embed");
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.embeddings.len(), 2);
    assert_eq!(response.model_name, "mock-embed");
}

// ── Group 5: Provider name and cost correctness ─────────────────────────

#[test]
fn test_provider_names_are_correct() {
    assert_eq!(
        OllamaProvider::new("http://localhost:11434").name(),
        "ollama"
    );
    assert_eq!(DeepSeekProvider::new(None).name(), "deepseek");
    assert_eq!(OpenAiProvider::new(None).name(), "openai");
    assert_eq!(GeminiProvider::new(None).name(), "gemini");
    assert_eq!(GroqProvider::new(None).name(), "groq");
    assert_eq!(NvidiaProvider::new(None).name(), "nvidia");
    assert_eq!(CohereProvider::new(None).name(), "cohere");
    assert_eq!(MistralProvider::new(None).name(), "mistral");
    assert_eq!(TogetherProvider::new(None).name(), "together");
    assert_eq!(FireworksProvider::new(None).name(), "fireworks");
    assert_eq!(PerplexityProvider::new(None).name(), "perplexity");
    assert_eq!(OpenRouterProvider::new(None).name(), "openrouter");
    assert_eq!(ClaudeProvider::new(None).name(), "claude");
    assert_eq!(MockProvider::new().name(), "mock");
}

#[test]
fn test_ollama_is_free() {
    let provider = OllamaProvider::new("http://localhost:11434");
    assert_eq!(provider.cost_per_token(), 0.0);
    assert!(!provider.is_paid());
}

#[test]
fn test_paid_providers_have_nonzero_cost() {
    assert!(DeepSeekProvider::new(None).cost_per_token() > 0.0);
    assert!(OpenAiProvider::new(None).cost_per_token() > 0.0);
    assert!(ClaudeProvider::new(None).cost_per_token() > 0.0);
    assert!(GeminiProvider::new(None).cost_per_token() > 0.0);
}

// ── Group 6: Gateway provider selection ─────────────────────────────────

#[test]
fn test_select_provider_no_config_and_no_ollama_returns_error() {
    // With no API keys and no Ollama running, should return a clean error
    let config = ProviderSelectionConfig {
        provider: None,
        ollama_url: None,
        deepseek_api_key: None,
        anthropic_api_key: None,
        openai_api_key: None,
        gemini_api_key: None,
        groq_api_key: None,
        mistral_api_key: None,
        together_api_key: None,
        fireworks_api_key: None,
        perplexity_api_key: None,
        cohere_api_key: None,
        openrouter_api_key: None,
        nvidia_api_key: None,
        flash_model_path: None,
    };
    // This might succeed if Ollama is running on localhost, so we check both cases
    let result = select_provider(&config);
    // If Ollama isn't running, this should be an error
    // If Ollama IS running, it should succeed (auto-detect)
    // Either way: no panic
    match result {
        Ok(provider) => {
            assert_eq!(
                provider.name(),
                "ollama",
                "auto-detected provider should be Ollama"
            );
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("No LLM provider configured"),
                "error should guide user: got {msg}"
            );
        }
    }
}

#[test]
fn test_select_provider_explicit_ollama() {
    let config = ProviderSelectionConfig {
        provider: Some("ollama".into()),
        ollama_url: Some("http://127.0.0.1:19999".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    assert!(
        result.is_ok(),
        "explicit provider selection should succeed even if not reachable"
    );
    assert_eq!(result.unwrap().name(), "ollama");
}

#[test]
fn test_select_provider_explicit_deepseek() {
    let config = ProviderSelectionConfig {
        provider: Some("deepseek".into()),
        deepseek_api_key: Some("test-key".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name(), "deepseek");
}

#[test]
fn test_select_provider_explicit_unknown_returns_error() {
    let config = ProviderSelectionConfig {
        provider: Some("nonexistent-provider".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    match result {
        Ok(_) => panic!("should have returned error for unknown provider"),
        Err(e) => {
            let err = e.to_string();
            assert!(err.contains("Unknown LLM provider"), "got: {err}");
        }
    }
}

#[test]
fn test_select_provider_explicit_flash() {
    let config = ProviderSelectionConfig {
        provider: Some("flash".into()),
        flash_model_path: Some("test-model.gguf".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name(), "flash");
}

#[test]
fn test_select_provider_priority_deepseek_over_ollama() {
    // When both DeepSeek key and Ollama URL are set (without explicit provider),
    // the cloud provider with an API key takes priority — the user explicitly
    // configured a cloud key, so they want cloud inference, not local Ollama.
    let config = ProviderSelectionConfig {
        ollama_url: Some("http://127.0.0.1:19999".into()),
        deepseek_api_key: Some("test-key".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    assert!(result.is_ok());
    // Cloud provider with key takes priority over Ollama
    assert_eq!(result.unwrap().name(), "deepseek");
}

#[test]
fn test_select_provider_deepseek_without_ollama_url() {
    // Cloud-first routing: cloud providers with keys take priority.
    // DeepSeek key is set, so DeepSeek should be selected.
    // Ollama auto-detect only kicks in as last resort (no keys at all).
    let config = ProviderSelectionConfig {
        deepseek_api_key: Some("test-key".into()),
        ..Default::default()
    };
    let result = select_provider(&config);
    assert!(result.is_ok());
    let provider = result.unwrap();
    let name = provider.name();
    assert!(
        name == "deepseek" || name == "ollama",
        "expected deepseek or ollama, got {name}"
    );
}

// ── Group 7: Provider token estimation ──────────────────────────────────

#[test]
fn test_token_estimation_reasonable() {
    let provider = MockProvider::new();
    let short = provider.estimate_input_tokens("hello");
    let long = provider.estimate_input_tokens(&"word ".repeat(1000));
    assert!(short < long, "longer text should estimate more tokens");
    assert!(short >= 1, "minimum estimate should be 1");
}

#[test]
fn test_token_estimation_empty() {
    let provider = MockProvider::new();
    let tokens = provider.estimate_input_tokens("");
    assert_eq!(tokens, 1, "empty string should estimate 1 token (minimum)");
}

// ── Group 8: Ollama chat_stream connection refused ──────────────────────

#[test]
fn test_ollama_chat_stream_connection_refused() {
    let provider = OllamaProvider::new("http://127.0.0.1:19999");
    let messages = vec![serde_json::json!({"role": "user", "content": "test"})];
    let result = provider.chat_stream(&messages, "llama3.2", |_token| {});
    // curl will fail when nothing is listening — should return Err, not panic
    // Note: this may succeed with a curl error or timeout, either way no panic
    assert!(
        result.is_err() || result.unwrap().is_empty(),
        "chat_stream to dead server should fail or return empty"
    );
}

// ── Group 9: Provider embed unsupported ─────────────────────────────────

#[test]
fn test_embed_unsupported_returns_error() {
    // Most cloud providers don't implement embed
    let provider = DeepSeekProvider::new(Some("test".into()));
    let result = provider.embed(&["test"], "model");
    // Should return error, not panic
    assert!(result.is_err());
}

// ── Group 10: Provider request building ─────────────────────────────────

#[test]
fn test_ollama_build_request_structure() {
    let provider = OllamaProvider::new("http://localhost:11434");
    let req = provider.build_request("hello world", 256, "llama3.2");
    assert!(req.endpoint.contains("/api/generate"));
    assert_eq!(req.body["model"], "llama3.2");
    assert_eq!(req.body["prompt"], "hello world");
    assert_eq!(req.body["stream"], false);
    assert_eq!(req.body["options"]["num_predict"], 256);
}

#[test]
fn test_ollama_vision_request_structure() {
    let provider = OllamaProvider::new("http://localhost:11434");
    let body = provider.build_image_request_body("describe this", "base64data", "llava");
    assert_eq!(body["model"], "llava");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "describe this");
    assert_eq!(body["messages"][0]["images"][0], "base64data");
}
