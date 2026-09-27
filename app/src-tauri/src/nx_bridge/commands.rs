//! Tauri commands for the Nexus Code (nx) bridge.

use std::sync::Arc;

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

/// Comprehensive governance status.
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

#[command]
pub fn nx_chat() -> Result<(), String> {
    Err(crate::phase0_surface::closed(
        "nx_chat",
        crate::phase0_surface::Closure::AgentExecution,
    ))
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

#[command]
pub fn nx_tool() -> Result<serde_json::Value, String> {
    Err(crate::phase0_surface::closed(
        "nx_tool",
        crate::phase0_surface::Closure::AgentExecution,
    ))
}

/// Run diagnostics (like `nx doctor`).
#[command]
pub async fn nx_doctor() -> Result<DiagnosticResult, String> {
    let status = nexus_code::setup::diagnose_without_cli_agents();
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
    let status = nexus_code::setup::diagnose_without_cli_agents();
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

/// Switch the active LLM provider at runtime.
#[command]
pub async fn nx_switch_provider(
    provider: String,
    state: State<'_, NxState>,
) -> Result<GovernanceStatus, String> {
    if state.is_running.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Cannot switch provider while agent is running".to_string());
    }

    // Validate the provider is available
    let status = nexus_code::setup::diagnose_without_cli_agents();
    if !status.configured_providers.iter().any(|p| p == &provider) {
        return Err(format!(
            "Provider '{}' is not configured. Available: {}",
            provider,
            status.configured_providers.join(", ")
        ));
    }

    let model = super::default_model_for_provider(&provider).to_string();

    let mut app = state.app.lock().await;
    app.config.default_provider = provider.clone();
    app.config.default_model = model.clone();
    app.router.set_slot(
        nexus_code::llm::router::ModelSlot::Execution,
        nexus_code::llm::router::SlotConfig {
            provider: provider.clone(),
            model: model.clone(),
        },
    );

    let fuel_remaining = app.governance.fuel.remaining();
    let fuel_total = app.governance.fuel.budget().total;
    let fuel_pct = if fuel_total > 0 {
        fuel_remaining as f64 / fuel_total as f64 * 100.0
    } else {
        0.0
    };

    Ok(GovernanceStatus {
        session_id: app.governance.identity.session_id()[..8].to_string(),
        provider,
        model,
        fuel_remaining,
        fuel_total,
        fuel_consumed: app.governance.fuel.budget().consumed,
        fuel_percentage: fuel_pct,
        audit_entries: app.governance.audit.len(),
        audit_chain_valid: app.governance.audit.verify_chain().is_ok(),
        tool_count: app.tool_registry.list().len(),
        tools: app
            .tool_registry
            .list()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        is_running: false,
        memory_count: app.memory.len(),
    })
}

// ─── Computer Use Response Types ───

#[derive(Debug, Serialize)]
pub struct NxScreenshot {
    pub base64: String,
    pub width: u32,
    pub height: u32,
    pub backend: String,
    pub file_size_bytes: usize,
    pub audit_hash: String,
}

#[derive(Debug, Serialize)]
pub struct ComputerUseStatus {
    pub display_server: Option<String>,
    pub capture_tool: Option<String>,
    pub input_tool: Option<String>,
    pub capture_ready: bool,
    pub input_ready: bool,
    pub safety_guard_active: bool,
}

#[derive(Debug, Serialize)]
pub struct AgentRunResult {
    pub task: String,
    pub completed: bool,
    pub summary: String,
    pub steps_executed: u32,
    pub fuel_consumed: u64,
    pub total_duration_ms: u64,
    pub audit_hash: String,
}

#[derive(Debug, Serialize)]
pub struct AppGrantInfo {
    pub id: String,
    pub app_wm_class: String,
    pub app_category: String,
    pub grant_level: String,
    pub permissions: Vec<String>,
    pub granted_at: String,
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
pub struct PatternInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub confidence: f32,
    pub last_used: String,
}

#[derive(Debug, Serialize)]
pub struct LearningStats {
    pub pattern_count: usize,
    pub memory_entries: usize,
    pub total_fuel_consumed: u64,
    pub avg_success_rate: f32,
}

// ─── Computer Use Commands ───

/// Take a screenshot via the governed computer-use pipeline.
#[command]
pub async fn nx_computer_use_screenshot() -> Result<NxScreenshot, String> {
    let opts = nexus_computer_use::capture::ScreenshotOptions::default();
    let shot = nexus_computer_use::capture::screenshot::take_screenshot(opts)
        .await
        .map_err(|e| format!("Screenshot failed: {}", e))?;

    Ok(NxScreenshot {
        base64: shot.base64,
        width: shot.width,
        height: shot.height,
        backend: shot.backend,
        file_size_bytes: shot.file_size_bytes,
        audit_hash: shot.audit_hash,
    })
}

/// Check computer-use system readiness: display server, capture, input.
#[command]
pub async fn nx_computer_use_status() -> Result<ComputerUseStatus, String> {
    let reqs = nexus_computer_use::capability::check_system_requirements();
    Ok(ComputerUseStatus {
        display_server: reqs.display_server,
        capture_tool: reqs.capture_tool,
        input_tool: reqs.input_tool,
        capture_ready: reqs.all_capture_ready,
        input_ready: reqs.all_input_ready,
        safety_guard_active: true, // always active when computer-use is loaded
    })
}

#[command]
pub fn nx_agent_run() -> Result<AgentRunResult, String> {
    Err(crate::phase0_surface::closed(
        "nx_agent_run",
        crate::phase0_surface::Closure::ProcessExecution,
    ))
}

/// Approve or deny a pending HITL consent request during an agent run.
#[command]
pub async fn nx_agent_approve(
    request_id: String,
    approved: bool,
    state: State<'_, NxState>,
) -> Result<(), String> {
    // Re-use the existing consent infrastructure
    let mut consents = state.pending_consents.lock().await;
    if let Some(pending) = consents.remove(&request_id) {
        pending
            .response_tx
            .send(approved)
            .map_err(|_| "Approval channel closed".to_string())?;
        Ok(())
    } else {
        Err(format!("No pending approval with ID: {}", request_id))
    }
}

/// List current app grants with categories.
#[command]
pub async fn nx_app_grants() -> Result<Vec<AppGrantInfo>, String> {
    let manager = nexus_computer_use::governance::AppGrantManager::new();
    let grants: Vec<AppGrantInfo> = manager
        .active_grants()
        .into_iter()
        .map(|g| AppGrantInfo {
            id: g.id.clone(),
            app_wm_class: g.app_wm_class.clone(),
            app_category: format!("{:?}", g.app_category),
            grant_level: format!("{}", g.grant_level),
            permissions: g.permissions.iter().map(|p| format!("{:?}", p)).collect(),
            granted_at: g.granted_at.to_rfc3339(),
            revoked: g.revoked,
        })
        .collect();
    Ok(grants)
}

/// List learned UI patterns.
#[command]
pub async fn nx_learned_patterns() -> Result<Vec<PatternInfo>, String> {
    let mut library = nexus_computer_use::learning::PatternLibrary::with_default_path();
    library
        .load()
        .map_err(|e| format!("Failed to load patterns: {}", e))?;

    let patterns: Vec<PatternInfo> = library
        .patterns()
        .iter()
        .map(|p| PatternInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            trigger: p.trigger.clone(),
            success_count: p.success_count,
            failure_count: p.failure_count,
            confidence: p.confidence,
            last_used: p.last_used.to_rfc3339(),
        })
        .collect();
    Ok(patterns)
}

/// Learning statistics: pattern count, memory entries, total fuel, success rate.
#[command]
pub async fn nx_learning_stats() -> Result<LearningStats, String> {
    let mut library = nexus_computer_use::learning::PatternLibrary::with_default_path();
    library.load().ok();

    let mut memory = nexus_computer_use::learning::ActionMemory::with_default_path();
    memory.load().ok();

    let entries = memory.entries();
    let total_fuel: u64 = entries.iter().map(|e| e.fuel_consumed).sum();
    let success_count = entries.iter().filter(|e| e.success).count();
    let avg_success_rate = if entries.is_empty() {
        0.0
    } else {
        success_count as f32 / entries.len() as f32
    };

    Ok(LearningStats {
        pattern_count: library.len(),
        memory_entries: memory.len(),
        total_fuel_consumed: total_fuel,
        avg_success_rate,
    })
}

// ─── Internal: Computer Use Agent Loop ───

// ─── Internal: Agent Loop with Event Emission ───
