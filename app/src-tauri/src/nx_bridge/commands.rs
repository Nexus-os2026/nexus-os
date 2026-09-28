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

// P0-FINAL-GATE (item G): a caller's `granted` boolean is not human approval.
// The consent requests it answered came only from the closed nx agent loops.
#[command]
pub fn nx_consent_respond() -> Result<(), String> {
    Err(crate::phase0_surface::closed(
        "nx_consent_respond",
        crate::phase0_surface::Closure::ApprovalRequired,
    ))
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
    let status = nexus_code::setup::diagnose_for_desktop();
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
    let status = nexus_code::setup::diagnose_for_desktop();
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

/// The Nexus Code session directory under the validated identity home, with
/// no fallback to the working directory (P0-002C5B).
fn nx_sessions_dir() -> Result<std::path::PathBuf, String> {
    nexus_kernel::identity_home::nexus_state_path("nexus-code/sessions").map_err(|e| e.to_string())
}

/// The file of a saved session. A session name is a user label (a narrow
/// identifier grammar, but case-sensitive) and never a path. The file is its
/// storage stem, so two names that differ only by case never share a file on
/// a case-insensitive filesystem; the listing reads names from file contents.
fn nx_session_file(
    sessions_dir: &std::path::Path,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    nexus_kernel::governed_path::validate_identifier(name, 64)
        .map_err(|_| "nx_session_save: invalid session name".to_string())?;
    let file = format!("{}.json", nexus_kernel::governed_path::storage_stem(name));
    // P0-002C5C: a stored file differing only by letter case (a session saved
    // before C5B under its raw name) is refused, never selected.
    nexus_kernel::governed_path::case_exact_entry(sessions_dir, &file)
        .map_err(|_| "nx_session: a stored session differs only by letter case".to_string())?;
    Ok(sessions_dir.join(file))
}

/// Save the current session.
#[command]
pub async fn nx_session_save(name: String, state: State<'_, NxState>) -> Result<String, String> {
    // P0-002C5B: sessions live under the validated identity home.
    let sessions_dir = nx_sessions_dir()?;
    let session_file = nx_session_file(&sessions_dir, &name)?;
    let app = state.app.lock().await;
    std::fs::create_dir_all(&sessions_dir).map_err(|e| format!("{}", e))?;

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
    // P0-002C5B: sessions live under the validated identity home.
    let sessions_dir = nx_sessions_dir()?;

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
    let status = nexus_code::setup::diagnose_for_desktop();
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

/// P0-002C5C (Architect decision): screen capture is unavailable in Phase
/// Zero. This command used to capture the whole screen directly and return it
/// to the interface, bypassing the engine state and the emergency stop. It now
/// denies unconditionally. No capture options, capture backend, image
/// encoding, file access or model request precedes the denial, and readiness
/// (`nx_computer_use_status`) is not a condition for it.
#[command]
pub async fn nx_computer_use_screenshot() -> Result<NxScreenshot, String> {
    Err(crate::phase0_surface::closed(
        "nx_computer_use_screenshot",
        crate::phase0_surface::Closure::ScreenObservation,
    ))
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

// P0-FINAL-GATE (item G): a caller's `approved` boolean is not human approval.
// The approval requests it answered came only from the closed nx agent run.
#[command]
pub fn nx_agent_approve() -> Result<(), String> {
    Err(crate::phase0_surface::closed(
        "nx_agent_approve",
        crate::phase0_surface::Closure::ApprovalRequired,
    ))
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

/// The learned UI pattern library under the validated identity home, never a
/// shared `/tmp` fallback (P0-002C5B).
fn learned_patterns_library() -> Result<nexus_computer_use::learning::PatternLibrary, String> {
    let path = nexus_kernel::identity_home::nexus_state_path("ui_patterns.json")
        .map_err(|e| e.to_string())?;
    Ok(nexus_computer_use::learning::PatternLibrary::new(path))
}

/// List learned UI patterns.
#[command]
pub async fn nx_learned_patterns() -> Result<Vec<PatternInfo>, String> {
    let mut library = learned_patterns_library()?;
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
    let mut library = learned_patterns_library()?;
    library.load().ok();

    // P0-002C5B: under the validated identity home, never a shared `/tmp`.
    let mut memory = nexus_computer_use::learning::ActionMemory::new(
        nexus_kernel::identity_home::nexus_state_path("agent_memory.json")
            .map_err(|e| e.to_string())?,
        1000,
    );
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

#[cfg(test)]
mod tests {
    use super::nx_session_file;
    use std::path::Path;

    #[test]
    fn p0_002c5b_session_names_never_share_a_file_by_case() {
        let dir = Path::new("/nexus/sessions");
        let lower = nx_session_file(dir, "work").unwrap();
        assert_eq!(lower, dir.join("work.json"));
        let upper = nx_session_file(dir, "Work").unwrap();
        let shout = nx_session_file(dir, "WORK").unwrap();
        for (a, b) in [(&lower, &upper), (&lower, &shout), (&upper, &shout)] {
            let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
            assert!(!a.eq_ignore_ascii_case(&b), "{a} aliases {b}");
        }
        for hostile in ["", "../escape", "a/b", "CON", "a b", &"a".repeat(65)] {
            assert!(nx_session_file(dir, hostile).is_err(), "{hostile:?}");
        }
    }

    /// P0-002C5C: a session saved before C5B under its raw name keeps its
    /// file, which no spelling selects or overwrites; its name now maps to a
    /// digest-backed replacement that no other name reaches.
    #[test]
    fn p0_002c5c_legacy_session_files_are_never_selected_or_overwritten() {
        let dir = std::env::temp_dir().join(format!("nexus-c5c-sessions-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MySession.json"), r#"{"name":"MySession"}"#).unwrap();

        // A lowercase spelling would be the legacy file on a case-insensitive
        // filesystem: refused everywhere, with no search for a similar name.
        assert_eq!(
            nx_session_file(&dir, "mysession"),
            Err("nx_session: a stored session differs only by letter case".to_string())
        );
        // The legacy name maps to its digest-backed replacement.
        let replacement = nx_session_file(&dir, "MySession").unwrap();
        let stem = nexus_kernel::governed_path::storage_stem("MySession");
        assert!(stem.starts_with("h-"));
        assert_eq!(replacement, dir.join(format!("{stem}.json")));
        std::fs::write(&replacement, r#"{"name":"MySession","saved":2}"#).unwrap();
        // No other spelling, including the stem itself, reaches it.
        for other in [
            stem.clone(),
            stem.to_uppercase(),
            format!("H{}", &stem[1..]),
        ] {
            if let Ok(path) = nx_session_file(&dir, &other) {
                assert_ne!(path, replacement, "{other:?}");
                assert!(!path
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&replacement.to_string_lossy()));
            }
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("MySession.json")).unwrap(),
            r#"{"name":"MySession"}"#
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
