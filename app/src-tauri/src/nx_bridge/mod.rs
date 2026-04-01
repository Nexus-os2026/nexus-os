use std::sync::Arc;
use tokio::sync::Mutex;

pub mod commands;
pub mod events;

/// Shared Nexus Code state, managed by Tauri.
///
/// This is the single instance of the nx governance kernel that
/// ALL React pages share. One kernel, one audit trail, one fuel budget.
pub struct NxState {
    /// The nx application instance (governance kernel + providers + tools)
    pub app: Arc<Mutex<nexus_code::app::App>>,
    /// Pending consent requests waiting for frontend response
    pub pending_consents: Arc<Mutex<std::collections::HashMap<String, ConsentPending>>>,
    /// Whether an agent loop is currently running
    pub is_running: Arc<std::sync::atomic::AtomicBool>,
    /// Cancellation token for the current agent loop
    pub cancel_token: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
}

/// A consent request waiting for the user's decision.
pub struct ConsentPending {
    pub request: nexus_code::governance::ConsentRequest,
    pub response_tx: tokio::sync::oneshot::Sender<bool>,
}

/// Initialize the nx bridge. Called during Tauri app setup.
pub fn init_nx_state() -> Result<NxState, String> {
    let nexuscode = nexus_code::context::NexusCodeMd::load(std::path::Path::new("NEXUSCODE.md"));

    let mut config = nexus_code::config::NxConfig::default();
    if let Some(budget) = nexuscode.fuel_budget {
        config.fuel_budget = budget;
    }
    config.blocked_paths = nexuscode.blocked_paths.clone();
    config.max_file_scope = nexuscode.max_file_scope.clone();

    let app = nexus_code::app::App::new(config)
        .map_err(|e| format!("Failed to initialize Nexus Code: {}", e))?;

    tracing::info!(
        tools = app.tool_registry.list().len(),
        "Nexus Code bridge initialized"
    );

    Ok(NxState {
        app: Arc::new(Mutex::new(app)),
        pending_consents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        cancel_token: Arc::new(Mutex::new(None)),
    })
}
