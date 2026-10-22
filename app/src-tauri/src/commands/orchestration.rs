//! Scheduler, team orchestration, and content pipeline commands.

use crate::hm_dispatch::GatewayPlannerLlm;
use crate::{agent_error, AppState};
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
    run_team_workflow(&state, &director_id, &goal, &member_ids)
}

/// Runs a team workflow for registered agents (P3 D-2 R2). The director and
/// every member must be registered agents; one that is not refuses the
/// workflow (no participant is skipped, no name is assumed).
///
/// P3-HM-CONTRACT-02 §11.3: the director's context is minted at workflow
/// start ([`team_director_context`]); every member task runs under its own
/// derived context and every dispatch is admitted (§12.4).
pub(crate) fn run_team_workflow(
    state: &AppState,
    director_id: &str,
    goal: &str,
    member_ids: &[String],
) -> Result<serde_json::Value, String> {
    let supervisor = state.supervisor.clone();
    let unregistered =
        |id: &str| format!("team workflow refused: '{id}' is not a registered agent");

    // Build team config from registered agents
    let config = {
        let sup = supervisor.lock().unwrap_or_else(|p| p.into_inner());

        let director_uuid =
            Uuid::parse_str(director_id).map_err(|e| format!("invalid director id: {e}"))?;
        let director_name = sup
            .get_agent(director_uuid)
            .map(|h| h.manifest.name.clone())
            .ok_or_else(|| unregistered(director_id))?;

        let mut members = Vec::new();
        for mid in member_ids {
            let uuid = Uuid::parse_str(mid).map_err(|e| format!("invalid member id: {e}"))?;
            let handle = sup.get_agent(uuid).ok_or_else(|| unregistered(mid))?;
            members.push(nexus_kernel::team_orchestrator::TeamMember {
                agent_id: mid.clone(),
                name: handle.manifest.name.clone(),
                role: infer_team_role(&handle.manifest.name),
                capabilities: handle.manifest.capabilities.clone(),
                fuel_budget: handle.remaining_fuel,
            });
        }

        nexus_kernel::team_orchestrator::TeamConfig {
            director_id: director_id.to_string(),
            director_name,
            members,
        }
    };

    let issuer = state
        .hm_issuer()
        .map_err(|e| format!("team workflow refused: no dispatch issuer: {e}"))?;
    let mut director = team_director_context(state, director_id, member_ids)?;
    let llm_handler: Arc<dyn nexus_kernel::cognitive::loop_runtime::LlmQueryHandler> =
        Arc::new(GatewayPlannerLlm::new(state));

    let orchestrator = nexus_kernel::team_orchestrator::TeamOrchestrator::new(
        supervisor,
        state.audit.clone(),
        llm_handler,
    );

    let result = orchestrator
        .execute_team_workflow(issuer, &mut director, &config, goal)
        .map_err(agent_error)?;

    serde_json::to_value(result).map_err(|e| format!("serialize: {e}"))
}

/// The director's context of a team workflow (P3-HM-CONTRACT-02 §11.3),
/// minted at workflow start. Every participant's route policy is resolved
/// first (§4.2, O1): one that cannot be established refuses the workflow
/// before any model call. A local-only participant makes the whole workflow
/// local (P3 D-2 R2, kept: any model call can carry a participant's data):
/// the director's context is tightened, and every member's context derives
/// from it.
pub(crate) fn team_director_context(
    state: &AppState,
    director_id: &str,
    member_ids: &[String],
) -> Result<nexus_kernel::hm::DispatchContext, String> {
    let issuer = state
        .hm_issuer()
        .map_err(|e| format!("team workflow refused: no dispatch issuer: {e}"))?;
    let mut any_local = false;
    for id in std::iter::once(director_id).chain(member_ids.iter().map(String::as_str)) {
        let refused = |reason: String| {
            format!(
                "team workflow refused for '{id}': {}: {reason}",
                crate::commands::cognitive::unresolved_policy_error()
            )
        };
        let uuid = Uuid::parse_str(id).map_err(|e| refused(e.to_string()))?;
        let policy = issuer
            .resolve_agent_route_policy(uuid)
            .map_err(|e| refused(e.to_string()))?;
        any_local |= policy.local_only();
    }
    let director = Uuid::parse_str(director_id).map_err(|e| format!("invalid director id: {e}"))?;
    let mut ctx = issuer
        .begin_unit(
            nexus_kernel::hm::Principal::Agent(director),
            nexus_kernel::hm::Unit::TeamTask {
                workflow_id: Uuid::new_v4(),
                task_index: 0,
            },
        )
        .map_err(|e| {
            format!(
                "team workflow refused for '{director_id}': {}: {e}",
                crate::commands::cognitive::unresolved_policy_error()
            )
        })?;
    if any_local {
        ctx.tighten_local_only();
    }
    Ok(ctx)
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
        Arc::new(GatewayPlannerLlm::new(&state));
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

#[tauri::command]
pub fn run_content_pipeline() -> Result<serde_json::Value, String> {
    Err(crate::phase0_surface::closed(
        "run_content_pipeline",
        crate::phase0_surface::Closure::AgentExecution,
    ))
}
