//! Local-first provider selection (M0-FIX F6).
//!
//! Policy: without an explicit provider choice (`ProviderSelectionConfig::provider`,
//! i.e. `LLM_PROVIDER`), Ollama is chosen first; a cloud API key that is merely
//! present is not an explicit choice of cloud and must never select a cloud
//! provider. These tests never depend on a real Ollama: auto-detection is
//! pointed at a refused loopback port, serialized because the environment is
//! process-wide.

use std::sync::Mutex;

use nexus_connectors_llm::gateway::{select_provider, ProviderSelectionConfig};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Runs `body` with Ollama auto-detection pointed at a port that refuses.
fn with_no_local_ollama<T>(body: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var_os("OLLAMA_URL");
    std::env::set_var("OLLAMA_URL", "http://127.0.0.1:1");
    let out = body();
    match previous {
        Some(v) => std::env::set_var("OLLAMA_URL", v),
        None => std::env::remove_var("OLLAMA_URL"),
    }
    out
}

fn name_of(config: &ProviderSelectionConfig) -> Result<String, String> {
    select_provider(config)
        .map(|p| p.name().to_string())
        .map_err(|e| e.to_string())
}

#[test]
fn cloud_key_without_explicit_provider_never_selects_cloud() {
    with_no_local_ollama(|| {
        let config = ProviderSelectionConfig {
            groq_api_key: Some("k".into()),
            nvidia_api_key: Some("k".into()),
            deepseek_api_key: Some("k".into()),
            openai_api_key: Some("k".into()),
            anthropic_api_key: Some("k".into()),
            gemini_api_key: Some("k".into()),
            ..Default::default()
        };
        if let Ok(name) = name_of(&config) {
            panic!("a key alone selected provider {name}");
        }
    });
}

#[test]
fn cloud_key_with_configured_ollama_url_selects_ollama() {
    with_no_local_ollama(|| {
        let config = ProviderSelectionConfig {
            ollama_url: Some("http://127.0.0.1:19999".into()),
            deepseek_api_key: Some("k".into()),
            anthropic_api_key: Some("k".into()),
            ..Default::default()
        };
        assert_eq!(name_of(&config).as_deref(), Ok("ollama"));
    });
}

#[test]
fn explicit_provider_with_key_selects_that_cloud_provider() {
    with_no_local_ollama(|| {
        let config = ProviderSelectionConfig {
            provider: Some("deepseek".into()),
            ollama_url: Some("http://127.0.0.1:19999".into()),
            deepseek_api_key: Some("k".into()),
            ..Default::default()
        };
        assert_eq!(name_of(&config).as_deref(), Ok("deepseek"));
    });
}

#[test]
fn cloud_key_only_and_no_local_model_is_an_honest_error() {
    with_no_local_ollama(|| {
        let config = ProviderSelectionConfig {
            deepseek_api_key: Some("k".into()),
            ..Default::default()
        };
        let err = name_of(&config).expect_err("must not fall back to cloud");
        assert!(err.contains("No local model"), "got: {err}");
        assert!(err.contains("LLM_PROVIDER"), "got: {err}");
    });
}
