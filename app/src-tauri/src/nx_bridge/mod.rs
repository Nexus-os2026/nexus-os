//! Nexus Code (nx) bridge — exposes the governed coding agent as Tauri commands.

use std::sync::Arc;
use tokio::sync::Mutex;

pub mod commands;
pub mod events;

/// Shared Nexus Code state, managed by Tauri.
///
/// Single instance of the nx governance kernel shared by ALL React pages.
/// One kernel, one audit trail, one fuel budget.
pub struct NxState {
    /// The nx application instance (governance kernel + providers + tools).
    pub app: Arc<Mutex<nexus_code::app::App>>,
    /// Pending consent requests waiting for frontend response.
    pub pending_consents: Arc<Mutex<std::collections::HashMap<String, ConsentPending>>>,
    /// Whether an agent loop is currently running.
    pub is_running: Arc<std::sync::atomic::AtomicBool>,
    /// Cancellation token for the current agent loop.
    pub cancel_token: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
}

/// A consent request waiting for the user's decision.
pub struct ConsentPending {
    pub response_tx: tokio::sync::oneshot::Sender<bool>,
}

/// Default model for each provider.
pub fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "claude-sonnet-4-20250514",
        "openai" => "gpt-4o",
        "ollama" => "gemma4:e2b",
        "openrouter" => "qwen/qwen3.6-plus:free",
        "groq" => "llama-3.3-70b-versatile",
        "deepseek" => "deepseek-chat",
        "google" => "gemini-2.5-flash",
        _ => "gpt-4o",
    }
}

/// Auto-detect the best available provider using `diagnose()`.
///
/// Priority: anthropic > openai > ollama > google > openrouter > groq > deepseek
fn detect_best_provider() -> Option<(String, String)> {
    let status = nexus_code::setup::diagnose();
    let priority = [
        "anthropic",
        "openai",
        "ollama",
        "google",
        "openrouter",
        "groq",
        "deepseek",
    ];
    for name in priority {
        if status.configured_providers.iter().any(|p| p == name) {
            return Some((
                name.to_string(),
                default_model_for_provider(name).to_string(),
            ));
        }
    }
    None
}

/// Initialize the nx bridge. Called during Tauri app setup.
///
/// Loads config from NEXUSCODE.md / env / config files, then
/// auto-detects the best available provider if no explicit
/// provider is set (or if the configured provider isn't available).
pub fn init_nx_state() -> Result<NxState, String> {
    let mut config = nexus_code::config::NxConfig::load()
        .map_err(|e| format!("Failed to load NxConfig: {}", e))?;

    // Auto-detect provider if the configured one isn't actually available
    let status = nexus_code::setup::diagnose();
    let configured_ok = status
        .configured_providers
        .iter()
        .any(|p| p == &config.default_provider);

    if !configured_ok {
        if let Some((provider, model)) = detect_best_provider() {
            eprintln!("[nx-bridge] Auto-detected provider: {}/{}", provider, model);
            config.default_provider = provider;
            config.default_model = model;
        } else {
            eprintln!(
                "[nx-bridge] No LLM provider available — bridge will start but chat will fail"
            );
        }
    }

    let app = nexus_code::app::App::new(config)
        .map_err(|e| format!("Failed to initialize Nexus Code: {}", e))?;

    eprintln!(
        "[nx-bridge] Nexus Code bridge initialized ({} tools, provider: {}/{})",
        app.tool_registry.list().len(),
        app.config.default_provider,
        app.config.default_model,
    );

    Ok(NxState {
        app: Arc::new(Mutex::new(app)),
        pending_consents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        cancel_token: Arc::new(Mutex::new(None)),
    })
}
