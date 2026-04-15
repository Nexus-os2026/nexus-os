//! Scheduler, team orchestration, and content pipeline commands.

use crate::{agent_error, AppState, BridgeLlmQueryHandler};
use serde_json::json;
use std::sync::Arc;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Emitter;
use uuid::Uuid;

// ── Background Scheduler Commands ──

#[tauri::command]
pub async fn scheduler_create(
    state: tauri::State<'_, AppState>,
    entry: serde_json::Value,
) -> Result<String, String> {
    let parsed: nexus_kernel::scheduler::ScheduleEntry =
        serde_json::from_value(entry).map_err(|e| format!("invalid schedule entry: {e}"))?;
    let id = state
        .schedule_store
        .add(parsed)
        .map_err(|e| e.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub async fn scheduler_list(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let entries = state.schedule_store.list();
    serde_json::to_value(entries).map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn scheduler_enable(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| format!("invalid id: {e}"))?;
    state
        .schedule_store
        .enable(&uuid)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn scheduler_disable(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| format!("invalid id: {e}"))?;
    state
        .schedule_store
        .disable(&uuid)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn scheduler_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| format!("invalid id: {e}"))?;
    state
        .schedule_store
        .remove(&uuid)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn scheduler_history(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| format!("invalid id: {e}"))?;
    let entry = state
        .schedule_store
        .get(&uuid)
        .ok_or_else(|| format!("schedule {id} not found"))?;
    serde_json::to_value(serde_json::json!({
        "schedule_id": entry.id.to_string(),
        "name": entry.name,
        "run_count": entry.run_count,
        "last_run": entry.last_run,
        "next_run": entry.next_run,
        "enabled": entry.enabled,
    }))
    .map_err(|e| format!("serialize: {e}"))
}

#[tauri::command]
pub async fn scheduler_trigger_now(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| format!("invalid id: {e}"))?;
    let entry = state
        .schedule_store
        .get(&uuid)
        .ok_or_else(|| format!("schedule {id} not found"))?;
    let executor = nexus_kernel::scheduler::ScheduledExecutor::new(
        state.supervisor.clone(),
        state.adversarial_arena.clone(),
        state.audit.clone(),
    );
    let result = executor.execute(&entry, None).map_err(|e| e.to_string())?;
    // Best-effort: record schedule run for history; execution result is returned regardless
    let _ = state.schedule_store.record_run(&uuid, None);
    serde_json::to_value(result).map_err(|e| format!("serialize: {e}"))
}

/// Get the live status of all schedules in the background runner.
#[tauri::command]
pub async fn scheduler_runner_status(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let status = state.schedule_runner.status();
    serde_json::to_value(status).map_err(|e| format!("serialize: {e}"))
}

// ── Team Orchestration Commands ──────────────────────────────────────

/// Execute a team workflow: Director decomposes goal, assigns to workers, collects results.
#[tauri::command]
pub async fn execute_team_workflow(
    state: tauri::State<'_, AppState>,
    director_id: String,
    goal: String,
    member_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    let supervisor = state.supervisor.clone();

    // Build team config from registered agents
    let config = {
        let sup = supervisor.lock().unwrap_or_else(|p| p.into_inner());

        let director_uuid =
            Uuid::parse_str(&director_id).map_err(|e| format!("invalid director id: {e}"))?;
        let director_name = sup
            .get_agent(director_uuid)
            .map(|h| h.manifest.name.clone())
            .unwrap_or_else(|| "director".into());

        let mut members = Vec::new();
        for mid in &member_ids {
            let uuid = Uuid::parse_str(mid).map_err(|e| format!("invalid member id: {e}"))?;
            if let Some(handle) = sup.get_agent(uuid) {
                members.push(nexus_kernel::team_orchestrator::TeamMember {
                    agent_id: mid.clone(),
                    name: handle.manifest.name.clone(),
                    role: infer_team_role(&handle.manifest.name),
                    capabilities: handle.manifest.capabilities.clone(),
                    fuel_budget: handle.remaining_fuel,
                });
            }
        }

        nexus_kernel::team_orchestrator::TeamConfig {
            director_id: director_id.clone(),
            director_name,
            members,
        }
    };

    let llm_handler: Arc<dyn nexus_kernel::cognitive::loop_runtime::LlmQueryHandler> =
        Arc::new(BridgeLlmQueryHandler);

    let orchestrator = nexus_kernel::team_orchestrator::TeamOrchestrator::new(
        supervisor,
        state.audit.clone(),
        llm_handler,
    );

    let result = orchestrator
        .execute_team_workflow(&config, &goal)
        .map_err(agent_error)?;

    serde_json::to_value(result).map_err(|e| format!("serialize: {e}"))
}

/// Transfer fuel from one agent to another (Director privilege).
#[tauri::command]
pub async fn transfer_agent_fuel(
    state: tauri::State<'_, AppState>,
    from_agent_id: String,
    to_agent_id: String,
    amount: u64,
) -> Result<(), String> {
    let llm_handler: Arc<dyn nexus_kernel::cognitive::loop_runtime::LlmQueryHandler> =
        Arc::new(BridgeLlmQueryHandler);
    let orchestrator = nexus_kernel::team_orchestrator::TeamOrchestrator::new(
        state.supervisor.clone(),
        state.audit.clone(),
        llm_handler,
    );
    orchestrator
        .transfer_fuel(&from_agent_id, &to_agent_id, amount)
        .map_err(agent_error)
}

pub fn infer_team_role(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("research") || lower.contains("oracle") {
        "researcher".into()
    } else if lower.contains("writ") || lower.contains("content") {
        "writer".into()
    } else if lower.contains("publish") {
        "publisher".into()
    } else if lower.contains("director") || lower.contains("conductor") {
        "director".into()
    } else {
        "worker".into()
    }
}

// ── Content Pipeline Commands ────────────────────────────────────────

/// Run the full content pipeline: scan trends → research → write → publish → analytics.
/// Returns a PipelineResult with the article path, word count, and all phase details.
#[tauri::command]
pub async fn run_content_pipeline(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> Result<serde_json::Value, String> {
    let agent_uuid = Uuid::parse_str(&agent_id).map_err(|e| format!("invalid agent id: {e}"))?;

    // Build the pipeline context from the agent's manifest
    let (agent_name, capabilities, fuel_remaining, autonomy_level, egress_allowlist) = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        let handle = supervisor.get_agent(agent_uuid).ok_or("agent not found")?;
        (
            handle.manifest.name.clone(),
            handle.manifest.capabilities.clone(),
            handle.remaining_fuel as f64,
            nexus_kernel::autonomy::AutonomyLevel::from_numeric(handle.autonomy_level)
                .unwrap_or_default(),
            handle
                .manifest
                .allowed_endpoints
                .clone()
                .unwrap_or_default(),
        )
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let workspace = std::path::PathBuf::from(&home).join("agent-output");
    if !workspace.exists() {
        std::fs::create_dir_all(&workspace).map_err(|e| format!("create workspace: {e}"))?;
    }

    let context = nexus_kernel::actuators::ActuatorContext {
        agent_id: agent_id.clone(),
        agent_name,
        working_dir: workspace,
        autonomy_level,
        capabilities: capabilities.into_iter().collect(),
        fuel_remaining,
        egress_allowlist,
        action_review_engine: None,
        hitl_approved: false,
    };

    let llm_handler: Arc<dyn nexus_kernel::cognitive::loop_runtime::LlmQueryHandler> =
        Arc::new(BridgeLlmQueryHandler);

    let pipeline = nexus_kernel::content_pipeline::ContentPipeline::new(llm_handler);
    let mut audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let result = pipeline.run(&context, &mut audit);

    // Send notification if article was created
    if result.success {
        drop(audit); // release the lock before emitting
        #[cfg(all(
            feature = "tauri-runtime",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        {
            if let Some(app) = state.app_handle() {
                // Best-effort: notify frontend of new content; missed events are non-fatal
                let _ = app.emit(
                    "agent-notification",
                    json!({
                        "agent_id": agent_id,
                        "title": format!("New article: {}", result.article_title),
                        "body": format!(
                            "Published {} words on '{}'. Saved to {}",
                            result.word_count, result.topic, result.article_path
                        ),
                        "level": "success",
                    }),
                );
            }
        }
    }

    serde_json::to_value(result).map_err(|e| format!("serialize: {e}"))
}
