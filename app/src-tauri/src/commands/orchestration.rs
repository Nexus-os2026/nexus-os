//! Scheduler, team orchestration, and content pipeline commands.

use crate::{
    agent_error, resolve_agent_llm_policy, with_agent_llm_policy, AgentLlmPolicy, AppState,
    BridgeLlmQueryHandler,
};
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
/// workflow (no participant is skipped, no name is assumed). Every model call
/// of the workflow runs under [`team_llm_policy`].
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

    let policy = team_llm_policy(state, director_id, member_ids)?;
    let llm_handler: Arc<dyn nexus_kernel::cognitive::loop_runtime::LlmQueryHandler> =
        Arc::new(TeamPolicyLlm { policy });

    let orchestrator = nexus_kernel::team_orchestrator::TeamOrchestrator::new(
        supervisor,
        state.audit.clone(),
        llm_handler,
    );

    let result = orchestrator
        .execute_team_workflow(&config, goal)
        .map_err(agent_error)?;

    serde_json::to_value(result).map_err(|e| format!("serialize: {e}"))
}

/// The routing policy of a team workflow (P3 D-2 R2). Any model call of the
/// workflow can carry any participant's data: the decomposition carries the
/// director's goal, a failed task is reassigned to another member, and the
/// merge carries every member's result. So the whole workflow runs under one
/// policy, local-only when the director or any member is. A participant
/// whose policy cannot be established refuses the workflow before any call.
pub(crate) fn team_llm_policy(
    state: &AppState,
    director_id: &str,
    member_ids: &[String],
) -> Result<AgentLlmPolicy, String> {
    let mut policy = AgentLlmPolicy::Unrestricted;
    for id in std::iter::once(director_id).chain(member_ids.iter().map(String::as_str)) {
        match resolve_agent_llm_policy(state, id) {
            AgentLlmPolicy::Unresolved => {
                return Err(format!(
                    "team workflow refused for '{id}': {}",
                    crate::commands::cognitive::unresolved_policy_error()
                ))
            }
            AgentLlmPolicy::LocalOnly => policy = AgentLlmPolicy::LocalOnly,
            AgentLlmPolicy::Unrestricted => {}
        }
    }
    Ok(policy)
}

/// A team workflow's model calls: the decomposition, each member's task and
/// the merge, each under the team's policy on the thread that makes it.
struct TeamPolicyLlm {
    policy: AgentLlmPolicy,
}

impl nexus_kernel::cognitive::loop_runtime::LlmQueryHandler for TeamPolicyLlm {
    fn query(&self, prompt: &str) -> Result<String, String> {
        with_agent_llm_policy(self.policy, || {
            nexus_kernel::cognitive::loop_runtime::LlmQueryHandler::query(
                &BridgeLlmQueryHandler,
                prompt,
            )
        })
    }
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

#[tauri::command]
pub fn run_content_pipeline() -> Result<serde_json::Value, String> {
    Err(crate::phase0_surface::closed(
        "run_content_pipeline",
        crate::phase0_surface::Closure::AgentExecution,
    ))
}
