use serde::Serialize;
use tauri::{command, AppHandle, Emitter, Manager, State};

use super::NxState;

// ─── Response Types ───

#[derive(Debug, Serialize)]
pub struct GovernanceStatus {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub fuel_remaining: u64,
    pub fuel_total: u64,
    pub fuel_consumed: u64,
    pub fuel_percentage: f64,
    pub audit_entries: usize,
    pub audit_chain_valid: bool,
    pub envelope_similarity: f64,
    pub envelope_status: String,
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub is_running: bool,
    pub memory_count: usize,
}

#[derive(Debug, Serialize)]
pub struct DiagnosticResult {
    pub has_any_provider: bool,
    pub configured_providers: Vec<String>,
    pub unconfigured_providers: Vec<UnconfiguredProvider>,
    pub has_git: bool,
    pub has_ripgrep: bool,
    pub has_nexuscode_md: bool,
    pub ready: bool,
}

#[derive(Debug, Serialize)]
pub struct UnconfiguredProvider {
    pub name: String,
    pub env_var: String,
}

// ─── Core Commands ───

/// Get comprehensive governance status.
#[command]
pub async fn nx_status(state: State<'_, NxState>) -> Result<GovernanceStatus, String> {
    let app = state.app.lock().await;
    let is_running = state.is_running.load(std::sync::atomic::Ordering::Relaxed);

    let fuel_remaining = app.governance.fuel.remaining();
    let fuel_total = app.governance.fuel.budget().total;
    let fuel_pct = if fuel_total > 0 {
        fuel_remaining as f64 / fuel_total as f64 * 100.0
    } else {
        0.0
    };

    Ok(GovernanceStatus {
        session_id: app.governance.identity.session_id()[..8].to_string(),
        provider: app.config.default_provider.clone(),
        model: app.config.default_model.clone(),
        fuel_remaining,
        fuel_total,
        fuel_consumed: app.governance.fuel.budget().consumed,
        fuel_percentage: fuel_pct,
        audit_entries: app.governance.audit.len(),
        audit_chain_valid: app.governance.audit.verify_chain().is_ok(),
        envelope_similarity: 100.0,
        envelope_status: "Normal".to_string(),
        tool_count: app.tool_registry.list().len(),
        tools: app
            .tool_registry
            .list()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        is_running,
        memory_count: app.memory.len(),
    })
}

/// Send a message to the nx agent. Starts the agent loop in a background task.
/// Results stream via Tauri events: nx:text-delta, nx:tool-start, etc.
#[command]
pub async fn nx_chat(
    message: String,
    app_handle: AppHandle,
    state: State<'_, NxState>,
) -> Result<(), String> {
    if state.is_running.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Agent is already running. Call nx_chat_cancel first.".to_string());
    }
    state
        .is_running
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let nx_app = state.app.clone();
    let pending_consents = state.pending_consents.clone();
    let is_running = state.is_running.clone();
    let cancel_token = tokio_util::sync::CancellationToken::new();

    {
        let mut ct = state.cancel_token.lock().await;
        *ct = Some(cancel_token.clone());
    }

    let handle = app_handle.clone();

    tokio::spawn(async move {
        let result = run_agent_with_events(
            &message,
            nx_app,
            pending_consents,
            handle.clone(),
            cancel_token,
        )
        .await;

        is_running.store(false, std::sync::atomic::Ordering::Relaxed);

        if let Err(e) = result {
            let _ = handle.emit(
                "nx:error",
                super::events::NxError {
                    message: format!("{}", e),
                },
            );
        }
    });

    Ok(())
}

/// Cancel the currently running agent loop.
#[command]
pub async fn nx_chat_cancel(state: State<'_, NxState>) -> Result<(), String> {
    let ct = state.cancel_token.lock().await;
    if let Some(ref token) = *ct {
        token.cancel();
    }
    state
        .is_running
        .store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Respond to a consent request from the frontend.
#[command]
pub async fn nx_consent_respond(
    request_id: String,
    granted: bool,
    state: State<'_, NxState>,
) -> Result<(), String> {
    let mut consents = state.pending_consents.lock().await;
    if let Some(pending) = consents.remove(&request_id) {
        pending
            .response_tx
            .send(granted)
            .map_err(|_| "Consent channel closed".to_string())?;
        Ok(())
    } else {
        Err(format!("No pending consent with ID: {}", request_id))
    }
}

/// Invoke a tool directly through the governance pipeline.
#[command]
pub async fn nx_tool(
    tool_name: String,
    input: String,
    state: State<'_, NxState>,
) -> Result<serde_json::Value, String> {
    let input: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| format!("Invalid JSON input: {}", e))?;

    let mut app = state.app.lock().await;

    // Create tool independently to avoid borrow conflict with governance
    let tool = nexus_code::tools::create_tool(&tool_name).ok_or_else(|| {
        format!(
            "Unknown tool: {}. Available: {:?}",
            tool_name,
            app.tool_registry.list()
        )
    })?;

    let tool_ctx = nexus_code::tools::ToolContext {
        working_dir: app.config.project_dir().unwrap_or_default(),
        blocked_paths: app.config.blocked_paths.clone(),
        max_file_scope: app.config.max_file_scope.clone(),
        non_interactive: false,
    };

    match nexus_code::tools::execute_governed(tool.as_ref(), input, &tool_ctx, &mut app.governance)
        .await
    {
        Ok(result) => Ok(serde_json::json!({
            "success": result.success,
            "output": result.output,
            "duration_ms": result.duration_ms,
        })),
        Err(nexus_code::error::NxError::ConsentRequired { .. }) => {
            Err("Consent required. Use nx_chat for interactive consent flow.".to_string())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

/// Run diagnostics (like `nx doctor`).
#[command]
pub async fn nx_doctor() -> Result<DiagnosticResult, String> {
    let status = nexus_code::setup::diagnose();
    Ok(DiagnosticResult {
        has_any_provider: status.has_any_provider,
        configured_providers: status.configured_providers,
        unconfigured_providers: status
            .unconfigured_providers
            .iter()
            .map(|(name, env)| UnconfiguredProvider {
                name: name.clone(),
                env_var: env.clone(),
            })
            .collect(),
        has_git: status.has_git,
        has_ripgrep: status.has_ripgrep,
        has_nexuscode_md: status.has_nexuscode_md,
        ready: status.has_any_provider && status.has_git,
    })
}

/// List configured providers with status.
#[command]
pub async fn nx_providers() -> Result<Vec<serde_json::Value>, String> {
    let status = nexus_code::setup::diagnose();
    let mut providers = Vec::new();
    for name in &status.configured_providers {
        providers.push(serde_json::json!({ "name": name, "configured": true }));
    }
    for (name, env_var) in &status.unconfigured_providers {
        providers
            .push(serde_json::json!({ "name": name, "configured": false, "env_var": env_var }));
    }
    Ok(providers)
}

/// List available tools with descriptions.
#[command]
pub async fn nx_tools(state: State<'_, NxState>) -> Result<Vec<serde_json::Value>, String> {
    let app = state.app.lock().await;
    let tools: Vec<serde_json::Value> = app
        .tool_registry
        .all()
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
            })
        })
        .collect();
    Ok(tools)
}

/// Save the current session.
#[command]
pub async fn nx_session_save(name: String, state: State<'_, NxState>) -> Result<String, String> {
    let app = state.app.lock().await;
    let sessions_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nexus-code")
        .join("sessions");
    std::fs::create_dir_all(&sessions_dir).map_err(|e| format!("{}", e))?;

    let session_file = sessions_dir.join(format!("{}.json", name));
    let session_data = serde_json::json!({
        "name": name,
        "session_id": app.governance.identity.session_id(),
        "saved_at": chrono::Utc::now().to_rfc3339(),
        "fuel_remaining": app.governance.fuel.remaining(),
        "fuel_consumed": app.governance.fuel.budget().consumed,
        "audit_entries": app.governance.audit.len(),
        "provider": app.config.default_provider,
        "model": app.config.default_model,
    });

    std::fs::write(
        &session_file,
        serde_json::to_string_pretty(&session_data).unwrap_or_default(),
    )
    .map_err(|e| format!("{}", e))?;

    Ok(format!("Session '{}' saved", name))
}

/// List saved sessions.
#[command]
pub async fn nx_session_list() -> Result<Vec<serde_json::Value>, String> {
    let sessions_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("nexus-code")
        .join("sessions");

    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                            sessions.push(data);
                        }
                    }
                }
            }
        }
    }
    Ok(sessions)
}

// ─── Internal: Agent Loop with Event Emission ───

async fn run_agent_with_events(
    message: &str,
    nx_app: std::sync::Arc<tokio::sync::Mutex<nexus_code::app::App>>,
    pending_consents: std::sync::Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, super::ConsentPending>>,
    >,
    app_handle: AppHandle,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<(), nexus_code::error::NxError> {
    let mut app = nx_app.lock().await;

    let agent_config = nexus_code::agent::AgentConfig {
        max_turns: 10,
        system_prompt: nexus_code::agent::build_system_prompt(
            "You are Nexus Code, a governed coding agent within Nexus OS. \
             You have access to the project's files, can run tests, and can make changes \
             through the governed execution pipeline. Be concise and precise.",
            &app.tool_registry,
        ),
        model_slot: nexus_code::llm::ModelSlot::Execution,
        auto_approve_tier2: false,
        auto_approve_tier3: false,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    // Consent handler: emits Tauri event, waits for frontend response
    let consents_for_handler = pending_consents.clone();
    let handle_for_consent = app_handle.clone();
    let consent_handler: std::sync::Arc<
        dyn Fn(&nexus_code::governance::ConsentRequest) -> bool + Send + Sync,
    > = std::sync::Arc::new(move |request| {
        let request_id = uuid::Uuid::new_v4().to_string();

        let _ = handle_for_consent.emit(
            "nx:consent-required",
            super::events::NxConsentRequired {
                request_id: request_id.clone(),
                tool_name: request.action.clone(),
                tier: format!("{:?}", request.tier),
                details: request.details.clone(),
                capability: String::new(),
                fuel_cost: 0,
            },
        );

        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let consents = consents_for_handler.clone();
            let rid = request_id;
            tokio::runtime::Handle::current().block_on(async {
                let mut map = consents.lock().await;
                map.insert(
                    rid,
                    super::ConsentPending {
                        request: request.clone(),
                        response_tx: tx,
                    },
                );
            });
        }

        rx.blocking_recv().unwrap_or(false)
    });

    let mut messages = vec![nexus_code::llm::types::Message {
        role: nexus_code::llm::types::Role::User,
        content: message.to_string(),
    }];

    let tool_ctx = nexus_code::tools::ToolContext {
        working_dir: app.config.project_dir().unwrap_or_default(),
        blocked_paths: app.config.blocked_paths.clone(),
        max_file_scope: app.config.max_file_scope.clone(),
        non_interactive: false,
    };

    // Forward agent events to Tauri events
    let handle_for_events = app_handle.clone();
    let nx_app_for_update = nx_app.clone();
    let event_forwarder = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                nexus_code::agent::AgentEvent::TextDelta(text) => {
                    let _ = handle_for_events
                        .emit("nx:text-delta", super::events::NxTextDelta { text });
                }
                nexus_code::agent::AgentEvent::ToolCallStart { name, id } => {
                    let _ = handle_for_events
                        .emit("nx:tool-start", super::events::NxToolStart { name, id });
                }
                nexus_code::agent::AgentEvent::ToolCallComplete {
                    name,
                    success,
                    duration_ms,
                    summary,
                } => {
                    let _ = handle_for_events.emit(
                        "nx:tool-complete",
                        super::events::NxToolComplete {
                            name,
                            success,
                            duration_ms,
                            summary,
                        },
                    );

                    // Emit governance update after each tool completion
                    if let Ok(app_lock) = nx_app_for_update.try_lock() {
                        let _ = handle_for_events.emit(
                            "nx:governance-update",
                            super::events::NxGovernanceUpdate {
                                fuel_remaining: app_lock.governance.fuel.remaining(),
                                fuel_consumed: app_lock.governance.fuel.budget().consumed,
                                audit_entries: app_lock.governance.audit.len(),
                                envelope_similarity: 100.0,
                            },
                        );
                    }
                }
                nexus_code::agent::AgentEvent::ToolCallDenied { name, reason } => {
                    let _ = handle_for_events.emit(
                        "nx:tool-denied",
                        super::events::NxToolDenied { name, reason },
                    );
                }
                nexus_code::agent::AgentEvent::Done {
                    reason,
                    total_turns,
                } => {
                    let _ = handle_for_events.emit(
                        "nx:done",
                        super::events::NxDone {
                            reason,
                            total_turns,
                        },
                    );
                }
                nexus_code::agent::AgentEvent::Error(msg) => {
                    let _ =
                        handle_for_events.emit("nx:error", super::events::NxError { message: msg });
                }
                _ => {}
            }
        }
    });

    // Destructure to allow simultaneous immutable and mutable borrows of different fields
    let nexus_code::app::App {
        ref router,
        ref tool_registry,
        ref mut governance,
        ref config,
        ..
    } = *app;
    let _ = config; // suppress unused warning

    let result = nexus_code::agent::run_agent_loop(
        &mut messages,
        router,
        tool_registry,
        &tool_ctx,
        governance,
        &agent_config,
        event_tx,
        consent_handler,
        cancel,
    )
    .await;

    event_forwarder.await.ok();
    result.map(|_| ())
}
