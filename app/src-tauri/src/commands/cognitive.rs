//! cognitive domain implementation.

#![allow(unused_imports)]

use crate::*;
use base64::Engine;
use chrono::TimeZone;
use nexus_adaptation::evolution::{EvolutionConfig, EvolutionEngine, MutationType, Strategy};
use nexus_auth::SessionManager;
use nexus_conductor::types::UserRequest;
use nexus_connectors_llm::chunking::SupportedFormat;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::model_hub::{self, DownloadProgress, DownloadStatus};
use nexus_connectors_llm::model_registry::ModelRegistry;
use nexus_connectors_llm::nexus_link::NexusLink;
use nexus_connectors_llm::providers::{
    groq::GROQ_MODELS, nvidia::NVIDIA_MODELS, ClaudeProvider, DeepSeekProvider, GeminiProvider,
    GroqProvider, LlmProvider, NvidiaProvider, OpenAiProvider,
};
use nexus_connectors_llm::rag::{RagConfig, RagPipeline};
use nexus_connectors_llm::whisper::WhisperTranscriber;
use nexus_connectors_messaging::gateway::{MessageGateway, PlatformStatus};
use nexus_distributed::ghost_protocol::{GhostConfig, GhostProtocol, SyncPeer as GhostSyncPeer};
use nexus_factory::pipeline::FactoryPipeline;
use nexus_integrations::IntegrationRouter;
use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::cognitive::PlannedAction;
use nexus_kernel::computer_control::{
    activate_emergency_kill_switch, analyze_stored_screenshot, capture_and_analyze_screen,
    capture_and_store_screen, ComputerControlEngine, InputAction, InputControlStatus, ScreenRegion,
};
use nexus_kernel::config::{
    load_config, save_config as save_nexus_config, AgentLlmConfig, HardwareConfig, ModelsConfig,
    NexusConfig, OllamaConfig,
};
use nexus_kernel::economic_identity::{EconomicConfig, EconomicEngine, TransactionType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::experience::{
    ConversationalBuilder, LivePreviewEngine, MarketplacePublisher, ProblemSolver, RemixEngine,
    TeachMode,
};
use nexus_kernel::genome::{
    crossover, genome_from_manifest, mutate, set_offspring_prompt, AgentGenome,
    AutoEvolutionManager, EvolutionConfig as AutoEvolveConfig,
    JsonAgentManifest as GenomeJsonManifest,
};
use nexus_kernel::hardware::{recommend_agent_configs, HardwareProfile};
use nexus_kernel::lifecycle::AgentState;
use nexus_kernel::manifest::{parse_manifest, AgentManifest};
use nexus_kernel::neural_bridge::{ContextQuery, ContextSource, NeuralBridge, NeuralBridgeConfig};
use nexus_kernel::permissions::{
    CapabilityRequest as KernelCapabilityRequest, PermissionCategory as KernelPermissionCategory,
    PermissionHistoryEntry as KernelPermissionHistoryEntry,
};
use nexus_kernel::protocols::a2a_client::A2aClient;
use nexus_kernel::redaction::RedactionEngine;
use nexus_kernel::simulation::{
    compare_reports, estimate_simulation_fuel, generate_personas, parse_seed,
    run_parallel_simulations as kernel_run_parallel_simulations, PersistedSimulationState,
    PredictionReport, SimulatedWorld, SimulationControl, SimulationObserver, SimulationProgress,
    SimulationRuntime, SimulationStatus as KernelSimulationStatus, SimulationSummary, WorldStatus,
};
use nexus_kernel::supervisor::{AgentId, Supervisor};
use nexus_kernel::tracing::{SpanStatus, TracingEngine};
use nexus_marketplace::payments::{BillingInterval, PaymentEngine, RevenueSplit};
use nexus_persistence::{CheckpointRow, NexusDatabase, StateStore};
use nexus_protocols::mcp_client::{McpAuth, McpHostManager, McpServerConfig, McpTransport};
use nexus_sdk::memory::{AgentMemory, MemoryConfig, MemoryType};
use nexus_tenancy::WorkspaceManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Digest;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Emitter;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Manager;
use tokio::sync::Notify;
use uuid::Uuid;

// ── Cognitive Runtime Commands ──────────────────────────────────────────────

pub(crate) fn assign_agent_goal(
    state: &AppState,
    agent_id: String,
    goal_description: String,
    priority: u8,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;
    state.validate_input(&goal_description)?;
    let effective_goal_description = goal_with_manifest_context(
        &agent_id,
        &goal_description,
        find_manifest_description(state, &agent_id).as_deref(),
    );
    let goal = nexus_kernel::cognitive::AgentGoal::new(effective_goal_description, priority);
    let goal_id = goal.id.clone();
    state
        .cognitive_runtime
        .assign_goal(&agent_id, goal)
        .map_err(|e| e.to_string())?;
    state.log_event(
        Uuid::parse_str(&agent_id).unwrap_or_default(),
        EventType::UserAction,
        json!({"action": "assign_agent_goal", "agent_id": agent_id, "goal_id": goal_id}),
    );
    Ok(goal_id)
}

pub(crate) fn persist_task_start(state: &AppState, agent_id: &str, goal_id: &str) {
    let goal = state
        .cognitive_runtime
        .get_agent_status(agent_id)
        .and_then(|status| status.active_goal.map(|goal| goal.description))
        .unwrap_or_else(|| "unknown goal".to_string());
    let fuel_budget = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        agent_id.parse::<Uuid>().ok().and_then(|uuid| {
            // Optional: agent_id may not be a valid UUID
            supervisor
                .get_agent(uuid)
                .map(|handle| handle.remaining_fuel as f64)
        })
    };
    let task = nexus_persistence::TaskRow {
        id: goal_id.to_string(),
        agent_id: agent_id.to_string(),
        goal,
        status: "running".to_string(),
        steps_json: "[]".to_string(),
        result_json: None,
        fuel_consumed: 0.0,
        fuel_budget,
        estimated_time_secs: None,
        actual_time_secs: None,
        quality_score: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        success: false,
    };
    if let Err(error) = state.db.save_task(&task) {
        eprintln!("persistence: save_task start failed: {error}");
    }
}

pub fn persist_task_completion(
    state: &AppState,
    agent_id: &str,
    goal_id: &str,
    status: &str,
    result_summary: &str,
    success: bool,
    fallback_fuel_consumed: f64,
) {
    let fuel_consumed = state
        .db
        .load_tasks_by_agent(agent_id, 100)
        .ok() // Optional: DB failure treated as no tasks — non-fatal for status display
        .and_then(|tasks| {
            let initial_budget = tasks
                .into_iter()
                .find(|task| task.id == goal_id)
                .and_then(|task| task.fuel_budget)?;
            let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            let remaining = agent_id
                .parse::<Uuid>()
                .ok() // Optional: agent_id may not be a valid UUID
                .and_then(|uuid| {
                    supervisor
                        .get_agent(uuid)
                        .map(|handle| handle.remaining_fuel as f64)
                })
                .unwrap_or(initial_budget);
            Some((initial_budget - remaining).max(0.0))
        })
        .unwrap_or(fallback_fuel_consumed);
    let result_json = json!({ "summary": result_summary }).to_string();
    if let Err(error) =
        state
            .db
            .update_task_status(goal_id, status, Some(&result_json), fuel_consumed, success)
    {
        eprintln!("persistence: update_task_status failed: {error}");
    }
    state.log_event(
        Uuid::parse_str(agent_id).unwrap_or_default(),
        EventType::StateChange,
        json!({
            "action": "agent_goal_completed",
            "goal_id": goal_id,
            "status": status,
            "success": success,
            "result_summary": result_summary,
        }),
    );
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AgentCheckpointSnapshot {
    status: String,
    fuel_remaining: u64,
    memories: Vec<nexus_persistence::MemoryRow>,
}

pub(crate) fn capture_agent_snapshot(
    state: &AppState,
    agent_id: &str,
) -> Option<AgentCheckpointSnapshot> {
    let status = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        // Optional: returns None if agent_id is not a valid UUID
        let uuid = Uuid::parse_str(agent_id).ok()?;
        let handle = supervisor.get_agent(uuid)?;
        handle.state.to_string()
    };
    let fuel_remaining = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        // Optional: returns None if agent_id is not a valid UUID
        let uuid = Uuid::parse_str(agent_id).ok()?;
        supervisor
            .get_agent(uuid)
            .map(|handle| handle.remaining_fuel)?
    };
    // Optional: snapshot is incomplete without memories; return None on DB failure
    let memories = state.db.load_memories(agent_id, None, 250).ok()?;
    Some(AgentCheckpointSnapshot {
        status,
        fuel_remaining,
        memories,
    })
}

pub(crate) fn snapshot_state_hash(snapshot: &AgentCheckpointSnapshot) -> String {
    let serialized = serde_json::to_vec(snapshot).unwrap_or_default();
    format!("{:x}", sha2::Sha256::digest(serialized))
}

pub(crate) fn save_checkpoint_to_db(
    state: &AppState,
    checkpoint: &nexus_kernel::time_machine::Checkpoint,
) {
    let serialized = match serde_json::to_string(checkpoint) {
        Ok(serialized) => serialized,
        Err(error) => {
            eprintln!(
                "time-machine: failed to serialize checkpoint {}: {error}",
                checkpoint.id
            );
            return;
        }
    };
    let row = CheckpointRow {
        id: checkpoint.id.clone(),
        agent_id: checkpoint.agent_id.clone().unwrap_or_default(),
        state_json: serialized,
        description: Some(checkpoint.label.clone()),
        created_at: chrono::Utc
            .timestamp_millis_opt(checkpoint.timestamp as i64)
            .single()
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339(),
    };
    if let Err(error) = state.db.save_checkpoint(&row) {
        eprintln!(
            "time-machine: failed to persist checkpoint {}: {error}",
            checkpoint.id
        );
    }
}

pub(crate) fn commit_time_machine_checkpoint(
    state: &AppState,
    checkpoint: nexus_kernel::time_machine::Checkpoint,
) -> Result<String, String> {
    let checkpoint_id = {
        let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        let (id, _) = supervisor
            .time_machine_mut()
            .commit_checkpoint(checkpoint.clone())
            .map_err(|e| e.to_string())?;
        id
    };
    save_checkpoint_to_db(state, &checkpoint);
    Ok(checkpoint_id)
}

pub(crate) fn record_agent_execution_checkpoint(
    state: &AppState,
    agent_id: &str,
    label: &str,
    before: Option<&AgentCheckpointSnapshot>,
    after: Option<&AgentCheckpointSnapshot>,
    action: &str,
) {
    let Some(after_snapshot) = after.cloned() else {
        return;
    };
    let before_snapshot = before.cloned().unwrap_or_else(|| after_snapshot.clone());

    let before_memories =
        serde_json::to_value(&before_snapshot.memories).unwrap_or_else(|_| json!([]));
    let after_memories =
        serde_json::to_value(&after_snapshot.memories).unwrap_or_else(|_| json!([]));
    let mut builder = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        supervisor
            .time_machine()
            .begin_checkpoint(label, Some(agent_id.to_string()))
    };
    builder.record_agent_state(
        agent_id,
        "status",
        json!(before_snapshot.status),
        json!(after_snapshot.status),
    );
    builder.record_agent_state(
        agent_id,
        "fuel_remaining",
        json!(before_snapshot.fuel_remaining),
        json!(after_snapshot.fuel_remaining),
    );
    builder.record_agent_state(agent_id, "memories", before_memories, after_memories);
    builder.record_config_change(
        "state_hash",
        json!(snapshot_state_hash(&before_snapshot)),
        json!(snapshot_state_hash(&after_snapshot)),
    );
    builder.record_config_change("action", json!(label), json!(action));
    let checkpoint = builder.build();
    // Best-effort: time machine checkpoint is supplementary; failure does not block the action
    let _ = commit_time_machine_checkpoint(state, checkpoint);
}

pub(crate) fn parse_agent_state(value: &str) -> Option<AgentState> {
    match value {
        "Created" => Some(AgentState::Created),
        "Starting" => Some(AgentState::Starting),
        "Running" => Some(AgentState::Running),
        "Paused" => Some(AgentState::Paused),
        "Stopping" => Some(AgentState::Stopping),
        "Stopped" => Some(AgentState::Stopped),
        "Destroyed" => Some(AgentState::Destroyed),
        _ => None,
    }
}

pub(crate) fn restore_agent_memories(state: &AppState, agent_id: &str, value: &serde_json::Value) {
    let Ok(memories) = serde_json::from_value::<Vec<nexus_persistence::MemoryRow>>(value.clone())
    else {
        return;
    };
    // Best-effort: clear old memories before restoring; partial restore is acceptable
    let _ = state.db.delete_memories_by_agent(agent_id);
    for memory in memories {
        // Best-effort: skip individual memories that fail to persist
        let _ = state.db.save_memory(
            &memory.agent_id,
            &memory.memory_type,
            &memory.key,
            &memory.value_json,
        );
    }
}

pub(crate) fn apply_non_file_undo_actions(
    state: &AppState,
    actions: &[nexus_kernel::time_machine::UndoAction],
) {
    for action in actions {
        match action {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState {
                agent_id,
                field,
                value,
            } => {
                if let Ok(agent_uuid) = Uuid::parse_str(agent_id) {
                    let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
                    if let Some(handle) = supervisor.get_agent_mut(agent_uuid) {
                        match field.as_str() {
                            "status" => {
                                if let Some(status) = value.as_str().and_then(parse_agent_state) {
                                    handle.state = status;
                                }
                            }
                            "fuel_remaining" => {
                                if let Some(fuel) = value.as_u64() {
                                    handle.remaining_fuel = fuel;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if field == "memories" {
                    restore_agent_memories(state, agent_id, value);
                }
            }
            nexus_kernel::time_machine::UndoAction::RestoreConfig { key, value } => {
                if key == "state_hash" || key == "action" {
                    continue;
                }
                let Ok(mut config) = load_config() else {
                    continue;
                };
                if key == "governance.enable_warden_review" {
                    if let Some(enabled) = value.as_bool() {
                        config.governance.enable_warden_review = enabled;
                        // Best-effort: persist config change during undo; config may be read-only
                        let _ = save_nexus_config(&config);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn task_timing_for_goal(
    state: &AppState,
    agent_id: &str,
    goal_id: &str,
) -> Option<(f64, f64)> {
    // Optional: returns None if DB query fails — timing data is supplementary
    let tasks = state.db.load_tasks_by_agent(agent_id, 200).ok()?;
    let task = tasks.into_iter().find(|task| task.id == goal_id)?;
    // Optional: returns None if timestamp is not valid RFC3339
    let started = chrono::DateTime::parse_from_rfc3339(&task.started_at).ok()?;
    let completed = task
        .completed_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .unwrap_or_else(|| chrono::Utc::now().into());
    Some((
        ((completed - started).num_milliseconds().max(0) as f64) / 1000.0,
        task.fuel_budget.unwrap_or(0.0),
    ))
}

pub(crate) fn recent_task_outcomes(
    state: &AppState,
    agent_id: &str,
    limit: usize,
) -> Vec<(bool, f64, f64)> {
    state
        .db
        .load_tasks_by_agent(agent_id, limit)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|task| {
            // Optional: skip tasks with unparseable start timestamps
            let started = chrono::DateTime::parse_from_rfc3339(&task.started_at).ok()?;
            let completed = task
                .completed_at
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .unwrap_or_else(|| chrono::Utc::now().into());
            Some((
                task.success,
                task.fuel_consumed,
                ((completed - started).num_milliseconds().max(0) as f64) / 1000.0,
            ))
        })
        .collect()
}

pub(crate) fn run_post_goal_evolution(
    bridge: &BackendEventBridge,
    state: &AppState,
    agent_id: &str,
    goal_id: &str,
    success: bool,
    fallback_fuel_consumed: f64,
) {
    let (autonomy_level, agent_name) = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        // Optional: returns early if agent_id is not a valid UUID or agent not found
        let Some(handle) = Uuid::parse_str(agent_id)
            .ok()
            .and_then(|uuid| supervisor.get_agent(uuid))
        else {
            return;
        };
        (handle.autonomy_level, handle.manifest.name.clone())
    };

    if autonomy_level < 4 {
        return;
    }

    let mem_store = DbMemoryStore {
        db: state.db.clone(),
    };
    let memory_mgr = nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(mem_store));
    let (duration_secs, fuel_budget) =
        task_timing_for_goal(state, agent_id, goal_id).unwrap_or((0.0, fallback_fuel_consumed));
    let fuel_consumed = state
        .db
        .load_tasks_by_agent(agent_id, 200)
        .ok()
        .and_then(|tasks| tasks.into_iter().find(|task| task.id == goal_id))
        .map(|task| task.fuel_consumed)
        .unwrap_or(fallback_fuel_consumed);
    let goal_type = state
        .db
        .load_tasks_by_agent(agent_id, 200)
        .ok()
        .and_then(|tasks| tasks.into_iter().find(|task| task.id == goal_id))
        .map(|task| task.goal)
        .unwrap_or_else(|| "scheduled_goal".to_string())
        .to_lowercase();
    let strategy_hash = nexus_kernel::cognitive::hash_strategy(&goal_type);

    // Best-effort: evolution tracking is supplementary; failure does not affect task completion
    let _ = state.evolution_tracker.record_task_result(
        agent_id,
        goal_id,
        &strategy_hash,
        &goal_type,
        success,
        fuel_consumed,
        duration_secs,
        fuel_budget,
        60.0,
        &memory_mgr,
    );

    let completed_count = state
        .db
        .load_tasks_by_agent(agent_id, 500)
        .unwrap_or_default()
        .into_iter()
        .filter(|task| task.completed_at.is_some())
        .count();

    if completed_count > 0 && completed_count % 5 == 0 {
        if let Ok(Some(best_strategy)) = state
            .evolution_tracker
            .select_best_strategy(agent_id, &goal_type)
        {
            // Best-effort: inject best strategy into agent memory for future planning
            let _ = memory_mgr.store_procedural(
                agent_id,
                &format!("planner_strategy_injection:{best_strategy}"),
                1.0,
            );
            if let Ok(strategies) = state.evolution_tracker.get_agent_strategies(agent_id) {
                if let Some(strategy) = strategies
                    .into_iter()
                    .find(|entry| entry.strategy_hash == best_strategy)
                {
                    let score = strategy.composite_score;
                    let generation = (completed_count / 5) as u64;
                    state.log_event(
                        Uuid::parse_str(agent_id).unwrap_or_default(),
                        EventType::StateChange,
                        json!({
                            "action": "agent_evolved_strategy",
                            "message": format!("Agent {} evolved strategy. New composite score: {:.3}", agent_name, score),
                            "new_score": score,
                            "generation": generation,
                            "strategy_hash": best_strategy,
                        }),
                    );
                    bridge.emit(
                        "agent-evolved",
                        json!({
                            "agent_id": agent_id,
                            "new_score": score,
                            "generation": generation,
                            "strategy_hash": best_strategy,
                        }),
                    );
                }
            }
        }
    }

    if completed_count > 0 && completed_count % 10 == 0 {
        let outcomes = recent_task_outcomes(state, agent_id, 10);
        let current_prompt = format!(
            "Plan safe, governed work for agent {} while respecting its capabilities and audit trail.",
            agent_name
        );
        let llm = GatewayPlannerLlm;
        // Best-effort: prompt optimization is a background improvement; failure is non-fatal
        let _ = state.evolution_tracker.optimize_planning_prompt(
            agent_id,
            &current_prompt,
            &outcomes,
            &llm,
            &memory_mgr,
        );
    }
}

/// Bridges the configured LLM provider to the cognitive planner's `PlannerLlm` trait.
pub(crate) struct GatewayPlannerLlm;

/// Bridges the cognitive loop's LlmQuery actions to the configured LLM provider.
///
/// When an agent's plan includes a step like "analyze these file contents" or
/// "summarize this data", the RegistryExecutor delegates to this handler which
/// routes through the same LLM provider infrastructure as the planner.
pub(crate) struct BridgeLlmQueryHandler;

impl nexus_kernel::cognitive::LlmQueryHandler for BridgeLlmQueryHandler {
    fn query(&self, prompt: &str) -> Result<String, String> {
        nexus_kernel::cognitive::PlannerLlm::plan_query(&GatewayPlannerLlm, prompt)
            .map_err(|e| e.to_string())
    }
}

impl nexus_kernel::cognitive::PlannerLlm for GatewayPlannerLlm {
    fn plan_query(&self, prompt: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        // If a Flash Inference provider is loaded, use it. The agent WAITS for Flash
        // (blocking lock) rather than falling back to Ollama, because Ollama may be dead
        // and Flash is the primary local provider. llama.cpp is single-threaded, so queries
        // are serialized — the agent simply waits its turn behind the UI.
        let flash = ACTIVE_FLASH_PROVIDER.with(|slot| slot.borrow().clone());
        if let Some(flash_provider) = flash {
            let prompt_chars = prompt.len();
            eprintln!("[planner] using Flash Inference, prompt len={prompt_chars} chars");

            // Run Flash query in a dedicated OS thread. This isolates the main
            // app from llama.cpp crashes (segfaults in FFI). If the thread dies,
            // we get an error instead of the whole app crashing.
            let prompt_owned = prompt.to_string();
            let handle = std::thread::spawn(move || {
                // Allow up to 2048 tokens for planner — multi-step plans from small
                // models can exceed 1024 tokens, causing truncated JSON parse failures.
                flash_provider.query(&prompt_owned, 2048, "flash")
            });

            match handle.join() {
                Ok(Ok(response)) => {
                    eprintln!(
                        "[planner] Flash Inference query complete ({} chars)",
                        response.output_text.len()
                    );
                    return Ok(strip_think_tags(&response.output_text));
                }
                Ok(Err(e)) => {
                    eprintln!("[planner] Flash Inference error: {e}");
                    return Err(e);
                }
                Err(_) => {
                    eprintln!(
                        "[planner] Flash Inference thread crashed (segfault or panic in llama.cpp)"
                    );
                    return Err(nexus_kernel::errors::AgentError::SupervisorError(
                        "Flash Inference crashed during query — the model may be corrupted or out of memory. \
                         Try reloading the model from the Flash Inference page."
                            .to_string(),
                    ));
                }
            }
        }

        let config = nexus_kernel::config::load_config().unwrap_or_default();
        let prov_config = build_provider_config(&config);
        let route_model = ACTIVE_AGENT_LLM_ROUTE
            .with(|slot| slot.borrow().as_ref().map(|route| route.model.clone()));
        let (provider, model) = if let Some(route_model) = route_model {
            // Skip flash routes — already handled by ACTIVE_FLASH_PROVIDER above.
            // If we reach here, Flash was requested but couldn't be resolved.
            if route_model.starts_with("flash:")
                || route_model.starts_with("flash/")
                || route_model == "flash"
            {
                // Flash provider wasn't available — return clear error instead of
                // silently falling back to Ollama (which causes confusing 404 errors).
                return Err(nexus_kernel::errors::AgentError::SupervisorError(
                    "Flash Inference is selected but no model is loaded. \
                     Go to the Agents page and click 'Load Model' to start a Flash session."
                        .to_string(),
                ));
            } else {
                provider_from_prefixed_model(&route_model, &prov_config).map_err(|e| {
                    nexus_kernel::errors::AgentError::SupervisorError(format!(
                        "Cannot resolve model '{}': {}. Please select a different model.",
                        route_model, e
                    ))
                })?
            }
        } else {
            // Use smart default model detection (checks API keys in priority order)
            let default_model = crate::commands::chat_llm::get_default_model();
            if default_model.contains('/') {
                // Prefixed model (e.g. "anthropic/claude-sonnet-4-6") — resolve provider from it
                provider_from_prefixed_model(&default_model, &prov_config).map_err(|e| {
                    nexus_kernel::errors::AgentError::SupervisorError(format!(
                        "Cannot resolve default model '{}': {}. Add an API key in Settings.",
                        default_model, e
                    ))
                })?
            } else if default_model == "mock-1" {
                // No provider configured at all — fail with clear message
                return Err(nexus_kernel::errors::AgentError::SupervisorError(
                    "No LLM provider configured. Add an API key in Settings (Anthropic, OpenAI, etc.) \
                     or install Ollama for local models."
                        .to_string(),
                ));
            } else {
                let provider = select_provider(&prov_config)?;
                (provider, default_model)
            }
        };
        // Wrap the LLM query in catch_unwind as a last resort — if the provider
        // crashes (e.g., llama.cpp segfault caught by signal handler, or Ollama timeout),
        // we return an error instead of killing the app.
        let query_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            provider.query(prompt, 4096, &model)
        }));
        match query_result {
            Ok(Ok(response)) => Ok(strip_think_tags(&response.output_text)),
            Ok(Err(e)) => Err(e),
            Err(_panic) => Err(nexus_kernel::errors::AgentError::SupervisorError(
                "LLM provider panicked during query — model may be corrupted or out of memory"
                    .to_string(),
            )),
        }
    }
}

/// Strip `<think>...</think>` reasoning blocks from LLM output.
/// Qwen3 and similar models emit these blocks for chain-of-thought reasoning;
/// they must be removed before the output reaches the JSON parser or the user.
pub(crate) fn strip_think_tags(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result[start..].find("</think>") {
            result = format!("{}{}", &result[..start], &result[start + end + 8..]);
        } else {
            // Unclosed <think> — remove to end
            result.truncate(start);
            break;
        }
    }
    result
}

impl nexus_kernel::cognitive::EvolutionLlm for GatewayPlannerLlm {
    fn optimize_prompt(&self, prompt: &str) -> Result<String, String> {
        nexus_kernel::cognitive::PlannerLlm::plan_query(self, prompt)
            .map_err(|error| error.to_string())
    }
}

impl nexus_kernel::genome::AutoEvolveLlm for GatewayPlannerLlm {
    fn score_response(&self, user_message: &str, agent_response: &str) -> Result<f64, String> {
        let prompt = format!(
            "Rate this AI agent response on a scale of 1-10.\n\
             User asked: {user_message}\n\
             Agent responded: {agent_response}\n\n\
             Score based on: relevance, accuracy, helpfulness, conciseness.\n\
             Return ONLY a number 1-10, nothing else."
        );
        let text = nexus_kernel::cognitive::PlannerLlm::plan_query(self, &prompt)
            .map_err(|e| e.to_string())?;
        // Parse the first number found in the response
        let score = text
            .trim()
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .find(|s| !s.is_empty())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(7.0);
        Ok(score.clamp(1.0, 10.0))
    }

    fn mutate_prompt(
        &self,
        current_prompt: &str,
        weak_responses: &[(String, String, f64)],
    ) -> Result<String, String> {
        let mut weak_desc = String::new();
        for (i, (user_msg, agent_resp, score)) in weak_responses.iter().enumerate() {
            weak_desc.push_str(&format!(
                "Task {}: User asked: {user_msg}\n  Agent said: {agent_resp}\n  Score: {score}/10\n\n",
                i + 1
            ));
        }
        let prompt = format!(
            "Here is an AI agent's system prompt:\n{current_prompt}\n\n\
             It performed poorly on these tasks:\n{weak_desc}\n\
             Analyze WHY the responses were weak and rewrite the system prompt \
             to address these specific weaknesses. Keep the core personality \
             and capabilities, but add targeted instructions to improve.\n\n\
             Return ONLY the improved system prompt."
        );
        nexus_kernel::cognitive::PlannerLlm::plan_query(self, &prompt).map_err(|e| e.to_string())
    }

    fn generate_with_prompt(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<String, String> {
        let prompt =
            format!("[System prompt: {system_prompt}]\n\nUser: {user_message}\n\nAssistant:");
        nexus_kernel::cognitive::PlannerLlm::plan_query(self, &prompt).map_err(|e| e.to_string())
    }
}

#[cfg(not(test))]
pub(crate) struct SimulationPlannerLlm;

#[cfg(not(test))]
impl nexus_kernel::cognitive::PlannerLlm for SimulationPlannerLlm {
    fn plan_query(&self, prompt: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        let gateway = GatewayPlannerLlm;
        match gateway.plan_query(prompt) {
            Ok(response) if simulation_response_is_usable(prompt, &response) => Ok(response),
            Ok(response) => {
                Err(nexus_kernel::errors::AgentError::SupervisorError(format!(
                    "LLM response unusable for simulation ({} chars). Configure a capable LLM provider.",
                    response.len()
                )))
            }
            Err(e) => {
                Err(nexus_kernel::errors::AgentError::SupervisorError(format!(
                    "World Simulation requires a running LLM. Error: {e}"
                )))
            }
        }
    }
}

#[cfg(test)]
pub(crate) struct TestSimulationPlannerLlm;

#[cfg(test)]
impl nexus_kernel::cognitive::PlannerLlm for TestSimulationPlannerLlm {
    fn plan_query(&self, prompt: &str) -> Result<String, nexus_kernel::errors::AgentError> {
        Ok(simulation_mock_response(prompt))
    }
}

#[cfg(not(test))]
pub(crate) fn simulation_response_is_usable(prompt: &str, response: &str) -> bool {
    if response.trim().is_empty() || response.contains("[Mock Response") {
        return false;
    }
    if prompt.contains("structured JSON")
        || prompt.contains("Return as JSON")
        || prompt.contains("Return as JSON array")
        || prompt.contains("Extract all entities")
        || prompt.contains("What do you do next?")
    {
        return nexus_kernel::simulation::extract_json_value(response).is_ok();
    }
    true
}

#[cfg(test)]
pub(crate) fn simulation_mock_response(prompt: &str) -> String {
    if prompt.contains("Analyze this text and extract") {
        return json!({
            "scenario": "A simulated governance scenario",
            "entities": [
                {"name": "Nexus Council", "entity_type": "organization"},
                {"name": "Policy X", "entity_type": "policy"}
            ],
            "relationships": [
                {"from": "Nexus Council", "to": "Policy X", "relation_type": "debates"}
            ],
            "variables": [
                {"key": "policy_x_passed", "description": "Whether Policy X passes"}
            ],
            "suggested_personas": ["analyst", "executive", "citizen"]
        })
        .to_string();
    }
    if prompt.contains("Extract all entities") {
        return json!({
            "entities": [
                {"entity_name": "Nexus Council", "entity_type": "organization", "properties": {"domain": "governance"}},
                {"entity_name": "Policy X", "entity_type": "policy", "properties": {"status": "proposed"}}
            ],
            "relationships": [
                {"from": "Nexus Council", "to": "Policy X", "relation_type": "debates", "strength": 0.75}
            ]
        })
        .to_string();
    }
    if prompt.contains("Generate") && prompt.contains("diverse personas") {
        let count = prompt
            .split("Generate ")
            .nth(1)
            .and_then(|rest| rest.split(" diverse personas").next())
            .and_then(|digits| digits.parse::<usize>().ok())
            .unwrap_or(6);
        return serde_json::to_string(
            &(0..count)
                .map(|index| {
                    json!({
                        "id": format!("mock-persona-{index}"),
                        "name": format!("Mock Persona {index}"),
                        "role": match index % 4 {
                            0 => "policy analyst",
                            1 => "tech ceo",
                            2 => "voter",
                            _ => "journalist",
                        },
                        "personality": {
                            "openness": 0.55,
                            "conscientiousness": 0.52,
                            "extraversion": 0.48,
                            "agreeableness": 0.58,
                            "neuroticism": 0.32
                        },
                        "beliefs": {
                            "policy_x": if index % 2 == 0 { 0.35 } else { -0.15 },
                            "market_confidence": if index % 3 == 0 { 0.25 } else { -0.05 }
                        },
                        "goals": ["shape the outcome", "protect long-term interests"],
                        "memories": [],
                        "relationships": {},
                        "behavior_rules": ["react to new information", "protect allies"],
                        "last_action": null,
                        "influence_score": 0.42 + (index as f64 * 0.01)
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".to_string());
    }
    if prompt.contains("Return as JSON array of persona decisions") {
        let count = prompt.matches("\"id\":\"").count().max(1);
        let batch = (0..count)
            .map(|index| {
                json!({
                    "id": format!("mock-persona-{index}"),
                    "action": if index % 4 == 0 { "speak" } else if index % 4 == 1 { "whisper" } else if index % 4 == 2 { "act" } else { "observe" },
                    "target": if index % 4 == 1 { Some("mock-persona-0") } else { None::<&str> },
                    "content": if index % 4 == 0 {
                        Some("We should stabilize support")
                    } else if index % 4 == 1 {
                        Some("Coordinate lobbying before the vote")
                    } else if index % 4 == 2 {
                        Some("publish a position memo supporting Policy X")
                    } else {
                        None::<&str>
                    },
                    "reasoning": "Mock simulation batch decision"
                })
            })
            .collect::<Vec<_>>();
        return serde_json::to_string(&batch).unwrap_or_else(|_| "[]".to_string());
    }
    if prompt.contains("What do you do next?") {
        let action = if prompt.contains("journalist") {
            json!({"action":"speak","target":null,"content":"I support transparent reporting on Policy X","reasoning":"Public information shifts the world."})
        } else if prompt.contains("tech ceo") {
            json!({"action":"whisper","target":"mock-persona-0","content":"Coordinate lobbying before the vote","reasoning":"Private coordination can amplify influence."})
        } else if prompt.contains("voter") {
            json!({"action":"observe","target":null,"content":null,"reasoning":"Waiting for more evidence."})
        } else {
            json!({"action":"act","target":null,"content":"publish a position memo supporting Policy X","reasoning":"A concrete action moves the coalition."})
        };
        return action.to_string();
    }
    if prompt.contains("Analyze this simulation summary") {
        return "The governed simulation converged toward a stable coalition around Policy X."
            .to_string();
    }
    if prompt.contains("Respond in character") {
        return "I still see this through the lens of Policy X and my accumulated memories."
            .to_string();
    }
    "{}".to_string()
}

#[derive(Clone, Default)]
pub(crate) struct BackendEventBridge {
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    app: Option<tauri::AppHandle<tauri::Wry>>,
}

impl BackendEventBridge {
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    fn from_app(app: tauri::AppHandle<tauri::Wry>) -> Self {
        Self { app: Some(app) }
    }

    #[cfg(not(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    )))]
    fn from_app(_: ()) -> Self {
        Self::default()
    }

    /// Maximum payload size for IPC events (64KB). Larger payloads are truncated
    /// to prevent Tauri/webview serialization crashes.
    const MAX_EMIT_PAYLOAD: usize = 64 * 1024;

    fn emit(&self, _event: &str, _payload: serde_json::Value) {
        #[cfg(all(
            feature = "tauri-runtime",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        {
            let Some(app) = &self.app else {
                eprintln!(
                    "[agent-ipc] BUG: emit called but app is None for event={}",
                    _event
                );
                return;
            };

            // Pre-serialize to check size and catch serialization errors safely
            let payload_json = match serde_json::to_string(&_payload) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!(
                        "[agent-ipc] serialization FAILED for event '{}': {}",
                        _event, e
                    );
                    // Emit a safe error payload instead of crashing
                    let fallback = json!({"error": format!("serialization failed: {e}")});
                    if let Err(emit_err) = app.emit(_event, fallback) {
                        eprintln!("[agent-ipc] fallback emit also failed: {emit_err}");
                    }
                    return;
                }
            };

            // Truncate oversized payloads
            if payload_json.len() > Self::MAX_EMIT_PAYLOAD {
                eprintln!(
                    "[agent-ipc] TRUNCATING event '{}': {} bytes > {} max",
                    _event,
                    payload_json.len(),
                    Self::MAX_EMIT_PAYLOAD,
                );
                let truncated = json!({
                    "truncated": true,
                    "original_size": payload_json.len(),
                    "partial": &payload_json[..Self::MAX_EMIT_PAYLOAD.min(payload_json.len())],
                });
                if let Err(e) = app.emit(_event, truncated) {
                    eprintln!("[agent-ipc] truncated emit failed: {e}");
                }
                return;
            }

            // Use app.emit() (not window.emit()) so the global listen() in
            // the frontend receives the event. window.emit() only targets the
            // webview-level emitter which global listen() does not subscribe to.
            match app.emit(_event, &_payload) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!(
                        "[agent-ipc] emit FAILED for '{}' ({} bytes): {}",
                        _event,
                        payload_json.len(),
                        e,
                    );
                }
            }
        }
    }
}

pub(crate) struct ScheduledGoalExecutor {
    pub(crate) state: AppState,
}

impl nexus_kernel::cognitive::ScheduledGoalExecutor for ScheduledGoalExecutor {
    fn execute(&self, agent_id: &str, default_goal: &str) -> Result<(), String> {
        let agent_uuid = Uuid::parse_str(agent_id).map_err(|e| format!("invalid agent id: {e}"))?;
        let agent_name = {
            let supervisor = self
                .state
                .supervisor
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let handle = supervisor
                .get_agent(agent_uuid)
                .ok_or_else(|| format!("agent '{agent_id}' not found"))?;
            handle.manifest.name.clone()
        };

        {
            let mut supervisor = self
                .state
                .supervisor
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if let Some(handle) = supervisor.get_agent(agent_uuid) {
                if handle.state == AgentState::Stopped {
                    supervisor.restart_agent(agent_uuid).map_err(agent_error)?;
                }
            }
        }

        let goal_id = execute_agent_goal(
            &self.state,
            agent_id.to_string(),
            default_goal.to_string(),
            5,
        )?;
        self.state.log_event(
            agent_uuid,
            EventType::StateChange,
            json!({
                "action": "scheduled_execution_triggered",
                "message": format!("Scheduled execution triggered for {agent_name}"),
                "agent_name": agent_name,
                "goal_id": goal_id,
            }),
        );

        #[cfg(all(
            feature = "tauri-runtime",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        {
            if let Some(app) = self.state.app_handle() {
                spawn_cognitive_loop_with_bridge(
                    BackendEventBridge::from_app(app),
                    self.state.clone(),
                    agent_id.to_string(),
                    goal_id,
                );
            }
        }

        Ok(())
    }
}

/// Bridges the ScheduleRunner to the Tauri cognitive loop for run_agent tasks.
pub(crate) struct RunnerGoalCallback {
    pub(crate) state: AppState,
}

impl nexus_kernel::scheduler::ScheduleGoalCallback for RunnerGoalCallback {
    fn execute_goal(&self, agent_id: &str, goal: &str) -> Result<String, String> {
        execute_agent_goal(&self.state, agent_id.to_string(), goal.to_string(), 5)
    }
}

pub(crate) struct WardenReviewEngine {
    state: AppState,
}

impl nexus_kernel::actuators::ActionReviewEngine for WardenReviewEngine {
    fn review(
        &self,
        actor_agent_id: &str,
        actor_name: &str,
        action: &PlannedAction,
    ) -> Result<nexus_kernel::actuators::ActionReviewDecision, String> {
        let config = load_config().map_err(agent_error)?;
        if !config.governance.enable_warden_review {
            return Ok(nexus_kernel::actuators::ActionReviewDecision::Allow {
                reason: "Warden governance review disabled".to_string(),
            });
        }

        let (warden_id, warden_model, warden_name) = {
            let supervisor = self
                .state
                .supervisor
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let Some((id, model, name)) =
                supervisor.health_check().into_iter().find_map(|status| {
                    supervisor.get_agent(status.id).and_then(|handle| {
                        if handle.manifest.name.eq_ignore_ascii_case("nexus-warden")
                            && matches!(
                                status.state,
                                AgentState::Running | AgentState::Starting | AgentState::Paused
                            )
                        {
                            Some((
                                status.id,
                                handle
                                    .manifest
                                    .llm_model
                                    .clone()
                                    .unwrap_or_else(get_default_model),
                                handle.manifest.name.clone(),
                            ))
                        } else {
                            None
                        }
                    })
                })
            else {
                return Ok(nexus_kernel::actuators::ActionReviewDecision::Allow {
                    reason: "Warden inactive".to_string(),
                });
            };
            (id, model, name)
        };

        let prompt = format!(
            "Agent {actor_name} wants to execute {}. Is this safe? Respond YES or NO with reason.",
            format_hitl_action_summary(action)
        );
        let provider =
            select_provider(&build_provider_config(&config)).map_err(|e| e.to_string())?;
        let response = provider
            .query(&prompt, 256, &warden_model)
            .map_err(agent_error)?
            .output_text;
        let trimmed = response.trim();
        self.state.log_event(
            warden_id,
            EventType::LlmCall,
            json!({
                "action": "warden_review",
                "actor_agent_id": actor_agent_id,
                "actor_name": actor_name,
                "warden_name": warden_name,
                "review_prompt": prompt,
                "review_response": trimmed,
            }),
        );

        if trimmed.to_ascii_uppercase().starts_with("NO") {
            let reason = trimmed
                .split_once(' ')
                .map(|(_, rest)| rest.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "Warden denied the action".to_string());
            create_warden_consent_request(
                &self.state,
                actor_agent_id,
                actor_name,
                action,
                &reason,
            )?;
            return Ok(nexus_kernel::actuators::ActionReviewDecision::Deny { reason });
        }

        let reason = trimmed
            .split_once(' ')
            .map(|(_, rest)| rest.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Warden approved the action".to_string());
        Ok(nexus_kernel::actuators::ActionReviewDecision::Allow { reason })
    }
}

/// Execute an agent goal end-to-end: assign goal, run cognitive cycles in a background
/// thread, emit Tauri events for each step/phase/completion, and handle HITL consent
/// by creating consent requests in the database and emitting notifications.
pub fn execute_agent_goal(
    state: &AppState,
    agent_id: String,
    goal_description: String,
    priority: u8,
) -> Result<String, String> {
    let before_snapshot = capture_agent_snapshot(state, &agent_id);
    // Assign the goal to the cognitive runtime
    let goal_id = assign_agent_goal(state, agent_id.clone(), goal_description, priority)?;
    persist_task_start(state, &agent_id, &goal_id);
    let after_snapshot = capture_agent_snapshot(state, &agent_id);
    record_agent_execution_checkpoint(
        state,
        &agent_id,
        "before_goal_execution",
        before_snapshot.as_ref(),
        after_snapshot.as_ref(),
        "Goal assigned",
    );
    // Return the goal_id immediately; the loop is spawned by the Tauri command
    Ok(goal_id)
}

pub(crate) fn format_hitl_action_summary(action: &PlannedAction) -> String {
    match action {
        PlannedAction::ShellCommand { command, args } => {
            if args.is_empty() {
                format!("ShellCommand: {command}")
            } else {
                format!("ShellCommand: {} {}", command, args.join(" "))
            }
        }
        PlannedAction::FileWrite { path, .. } => format!("FileWrite: {path}"),
        PlannedAction::FileRead { path } => format!("FileRead: {path}"),
        PlannedAction::DockerCommand { subcommand, args } => {
            if args.is_empty() {
                format!("DockerCommand: {subcommand}")
            } else {
                format!("DockerCommand: {} {}", subcommand, args.join(" "))
            }
        }
        PlannedAction::ApiCall { method, url, .. } => format!("ApiCall: {} {}", method, url),
        PlannedAction::WebFetch { url } => format!("WebFetch: {url}"),
        PlannedAction::BrowserAutomate { start_url, .. } => {
            format!("BrowserAutomate: {start_url}")
        }
        PlannedAction::CaptureScreen { .. } => "CaptureScreen".to_string(),
        PlannedAction::CaptureWindow { window_title } => {
            format!("CaptureWindow: {window_title}")
        }
        PlannedAction::AnalyzeScreen { query } => format!("AnalyzeScreen: {query}"),
        PlannedAction::MouseMove { x, y } => format!("MouseMove: {x}, {y}"),
        PlannedAction::MouseClick { x, y, button } => {
            format!("MouseClick: {button} @ {x}, {y}")
        }
        PlannedAction::MouseDoubleClick { x, y } => format!("MouseDoubleClick: {x}, {y}"),
        PlannedAction::MouseDrag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => format!("MouseDrag: {from_x},{from_y} -> {to_x},{to_y}"),
        PlannedAction::KeyboardType { text } => format!("KeyboardType: {} chars", text.len()),
        PlannedAction::KeyboardPress { key } => format!("KeyboardPress: {key}"),
        PlannedAction::KeyboardShortcut { keys } => {
            format!("KeyboardShortcut: {}", keys.join("+"))
        }
        PlannedAction::ScrollWheel { direction, amount } => {
            format!("ScrollWheel: {direction} x{amount}")
        }
        PlannedAction::ComputerAction { description, .. } => {
            format!("ComputerAction: {description}")
        }
        PlannedAction::HitlRequest { question, .. } => format!("HitlRequest: {question}"),
        other => other.action_type().to_string(),
    }
}

pub(crate) fn format_hitl_batch_message(agent_name: &str, actions: &[String]) -> String {
    let numbered = actions
        .iter()
        .enumerate()
        .map(|(index, action)| format!("{}. {}", index + 1, action))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Awaiting your approval — {agent_name} wants to execute {} actions:\n{}\nReview in Approval Center: Approve All, Review Each, or Deny All.",
        actions.len(),
        numbered
    )
}

pub(crate) fn consent_goal_id(operation_json: &Value) -> Option<String> {
    operation_json
        .get("goal_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub(crate) fn consent_rows_for_goal(
    state: &AppState,
    goal_id: &str,
) -> Result<Vec<nexus_persistence::ConsentRow>, String> {
    let pending = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?;

    Ok(pending
        .into_iter()
        .filter(|row| {
            serde_json::from_str::<Value>(&row.operation_json)
                .ok() // Optional: skip rows with malformed JSON rather than failing the filter
                .and_then(|value| consent_goal_id(&value))
                .as_deref()
                == Some(goal_id)
        })
        .collect())
}

/// Background driver for the cognitive loop. Spawned by the Tauri async command.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_cognitive_loop(
    window: tauri::Window,
    state: AppState,
    agent_id: String,
    goal_id: String,
) {
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    let bridge = BackendEventBridge::from_app(window.app_handle().clone());
    #[cfg(not(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    )))]
    let bridge = {
        // suppress unused window — only used when tauri-runtime feature is enabled
        let _ = window;
        BackendEventBridge::default()
    };

    spawn_cognitive_loop_with_bridge(bridge, state, agent_id, goal_id);
}

pub(crate) fn spawn_cognitive_loop_with_bridge(
    bridge: BackendEventBridge,
    state: AppState,
    agent_id: String,
    goal_id: String,
) {
    tauri::async_runtime::spawn(async move {
        // NOTE: Do NOT install a custom panic hook here. Calling prev_hook(info)
        // inside a hook can cause a double-panic (which aborts the entire process).
        // The catch_unwind below is sufficient for recovery.

        let planner = nexus_kernel::cognitive::CognitivePlanner::new(Box::new(GatewayPlannerLlm));
        let mem_store = DbMemoryStore {
            db: state.db.clone(),
        };
        let memory_mgr = nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(mem_store));

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let workspace_base = std::path::PathBuf::from(&home)
            .join(".nexus")
            .join("agents");
        let memory_mgr = Arc::new(memory_mgr);
        let executor = nexus_kernel::cognitive::RegistryExecutor::new(
            workspace_base,
            state.audit.clone(),
            state.supervisor.clone(),
            Some(Arc::new(WardenReviewEngine {
                state: state.clone(),
            })),
        )
        .with_llm_handler(Arc::new(BridgeLlmQueryHandler))
        .with_memory_manager(memory_mgr.clone());

        let max_cycles = 500u32;
        'cycle_loop: for _cycle in 0..max_cycles {
            let before_snapshot = capture_agent_snapshot(&state, &agent_id);

            // Run the cognitive cycle inside catch_unwind
            let cycle_result_or_panic =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut audit_guard = state.audit.lock().unwrap_or_else(|p| p.into_inner());
                    with_agent_llm_route(&state, &agent_id, || {
                        state.cognitive_runtime.run_cycle_with_evolution(
                            &agent_id,
                            &planner,
                            &memory_mgr,
                            &executor,
                            &mut audit_guard,
                            Some(&state.evolution_tracker),
                        )
                    })
                }));

            let result = match cycle_result_or_panic {
                Ok(r) => r,
                Err(panic_info) => {
                    let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    eprintln!(
                        "[agent-loop] PANIC caught for agent={}: {msg}",
                        &agent_id[..agent_id.len().min(8)]
                    );
                    bridge.emit(
                        "agent-goal-completed",
                        json!({
                            "agent_id": &agent_id, "goal_id": &goal_id,
                            "success": false, "reason": format!("agent panic: {msg}"),
                        }),
                    );
                    break 'cycle_loop;
                }
            };

            match result {
                Ok(cycle_result) => {
                    let after_snapshot = capture_agent_snapshot(&state, &agent_id);
                    if cycle_result.steps_executed > 0 {
                        persist_agent_fuel_ledger(&state, &agent_id);
                        record_agent_execution_checkpoint(
                            &state,
                            &agent_id,
                            "cognitive_loop_step",
                            before_snapshot.as_ref(),
                            after_snapshot.as_ref(),
                            &format!("Phase {}", cycle_result.phase),
                        );
                    }
                    // Collect recent step details from audit trail
                    let step_details: Vec<serde_json::Value> = {
                        let audit_guard = state.audit.lock().unwrap_or_else(|p| p.into_inner());
                        let agent_uuid = uuid::Uuid::parse_str(&agent_id).unwrap_or_default();
                        audit_guard.events()
                            .iter()
                            .rev()
                            .filter(|e| e.agent_id == agent_uuid)
                            .filter(|e| {
                                e.payload.get("event")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.starts_with("cognitive."))
                                    .unwrap_or(false)
                            })
                            .take(10)
                            .map(|e| {
                                json!({
                                    "action": e.payload.get("action").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                    "status": e.payload.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                                    "result": e.payload.get("result_preview").and_then(|v| v.as_str()).unwrap_or(""),
                                    "fuel_cost": e.payload.get("fuel_cost").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                })
                            })
                            .collect()
                    };

                    // Emit phase/step events to the frontend
                    bridge.emit(
                        "agent-cognitive-cycle",
                        json!({
                            "agent_id": &agent_id,
                            "goal_id": &goal_id,
                            "phase": format!("{}", cycle_result.phase),
                            "steps_executed": cycle_result.steps_executed,
                            "fuel_consumed": cycle_result.fuel_consumed,
                            "should_continue": cycle_result.should_continue,
                            "blocked_reason": cycle_result.blocked_reason,
                            "steps": step_details,
                        }),
                    );

                    // If blocked (HITL required), create a consent request
                    if cycle_result.phase == nexus_kernel::cognitive::CognitivePhase::Blocked {
                        // Get agent name for the notification
                        let agent_name = {
                            let agent_uuid = Uuid::parse_str(&agent_id).unwrap_or_default();
                            let m = state.meta.lock().unwrap_or_else(|p| p.into_inner());
                            m.get(&agent_uuid)
                                .map(|am| am.name.clone())
                                .unwrap_or_else(|| agent_id.clone())
                        };
                        let action_desc = cycle_result
                            .blocked_reason
                            .clone()
                            .unwrap_or_else(|| "perform a governed action".to_string());
                        let pending_hitl_steps = state
                            .cognitive_runtime
                            .pending_hitl_steps(&agent_id)
                            .unwrap_or_default();
                        let review_each_mode = state
                            .cognitive_runtime
                            .review_each_mode(&agent_id)
                            .unwrap_or(false);
                        let batch_actions: Vec<String> = pending_hitl_steps
                            .iter()
                            .map(|step| format_hitl_action_summary(&step.action))
                            .collect();
                        let use_batch = batch_actions.len() > 1 && !review_each_mode;

                        let approval_msg = if use_batch {
                            format_hitl_batch_message(&agent_name, &batch_actions)
                        } else {
                            format!(
                                "Awaiting your approval — {} wants to {}. Go to Approval Center to review.",
                                agent_name, action_desc
                            )
                        };
                        let action_label = batch_actions
                            .first()
                            .cloned()
                            .unwrap_or_else(|| action_desc.clone());
                        bridge.emit(
                            "agent-blocked",
                            json!({
                                "agent_id": &agent_id,
                                "goal_id": &goal_id,
                                "message": &approval_msg,
                                "action": if use_batch {
                                    format!("{} actions pending", batch_actions.len())
                                } else {
                                    action_label.clone()
                                },
                                "agent_name": &agent_name,
                            }),
                        );

                        let status = state.cognitive_runtime.get_agent_status(&agent_id);
                        let step_info = if use_batch {
                            json!({
                                "summary": format!("Execute {} governed actions", batch_actions.len()),
                                "goal": status.as_ref().and_then(|s| s.active_goal.as_ref().map(|g| g.description.clone())),
                                "goal_id": &goal_id,
                                "phase": status.as_ref().map(|s| format!("{}", s.phase)).unwrap_or_else(|| "blocked".to_string()),
                                "fuel_cost": batch_actions.len() as f64 * 5.0,
                                "side_effects": batch_actions.clone(),
                                "batch_action_count": batch_actions.len(),
                                "batch_actions": batch_actions.clone(),
                                "review_each_available": true,
                            })
                        } else {
                            let single_action = pending_hitl_steps
                                .first()
                                .map(|step| format_hitl_action_summary(&step.action))
                                .unwrap_or_else(|| action_desc.clone());
                            json!({
                                "summary": single_action.clone(),
                                "goal": status.as_ref().and_then(|s| s.active_goal.as_ref().map(|g| g.description.clone())),
                                "goal_id": &goal_id,
                                "phase": status.as_ref().map(|s| format!("{}", s.phase)).unwrap_or_else(|| "blocked".to_string()),
                                "fuel_cost": 5.0,
                                "side_effects": [single_action],
                            })
                        };

                        let consent_id = Uuid::new_v4().to_string();
                        let notify = state.register_blocked_consent_wait(&agent_id, &consent_id);
                        let now = {
                            use chrono::Utc;
                            Utc::now().to_rfc3339()
                        };

                        // Persist consent request
                        let consent_row = nexus_persistence::ConsentRow {
                            id: consent_id.clone(),
                            agent_id: agent_id.clone(),
                            operation_type: if use_batch {
                                "cognitive.hitl_batch".to_string()
                            } else {
                                "cognitive.hitl_approval".to_string()
                            },
                            operation_json: serde_json::to_string(&step_info).unwrap_or_default(),
                            hitl_tier: "Tier1".to_string(),
                            status: "pending".to_string(),
                            created_at: now.clone(),
                            resolved_at: None,
                            resolved_by: None,
                        };
                        if let Err(e) = state.db.enqueue_consent(&consent_row) {
                            eprintln!(
                                "[agent-loop] CRITICAL: consent DB write failed for agent={} action={}: {e}",
                                &agent_id[..agent_id.len().min(8)],
                                &action_desc
                            );
                            // Emit failure to frontend so user sees the error
                            bridge.emit(
                                "agent-goal-completed",
                                json!({
                                    "agent_id": &agent_id,
                                    "goal_id": &goal_id,
                                    "success": false,
                                    "reason": format!("Consent request failed to save: {e}"),
                                }),
                            );
                            break 'cycle_loop;
                        }
                        record_agent_execution_checkpoint(
                            &state,
                            &agent_id,
                            "awaiting_approval",
                            before_snapshot.as_ref(),
                            after_snapshot.as_ref(),
                            consent_row.operation_type.as_str(),
                        );

                        // Emit consent notification to frontend
                        let notification = consent_row_to_notification(&consent_row, &agent_name);
                        bridge.emit("consent-request-pending", json!(notification));

                        // Sleep with zero CPU until approve/deny/stop wakes this agent.
                        notify.notified().await;
                        state.clear_blocked_consent_wait(&agent_id, &consent_id);

                        if !state.cognitive_runtime.has_active_loop(&agent_id) {
                            return;
                        }

                        let resolution_status = state
                            .db
                            .load_consent_by_agent(&agent_id)
                            .ok() // Optional: treat DB failure as unresolved consent
                            .and_then(|rows| {
                                rows.into_iter()
                                    .find(|row| row.id == consent_id)
                                    .map(|row| row.status)
                            })
                            .unwrap_or_else(|| "unknown".to_string());

                        bridge.emit(
                            "consent-resolved",
                            json!({"consent_id": consent_id, "status": &resolution_status}),
                        );

                        if resolution_status == "approved" {
                            let approved_before = capture_agent_snapshot(&state, &agent_id);
                            let approved_after = capture_agent_snapshot(&state, &agent_id);
                            record_agent_execution_checkpoint(
                                &state,
                                &agent_id,
                                "approval_granted",
                                approved_before.as_ref(),
                                approved_after.as_ref(),
                                "Approval granted",
                            );
                            bridge.emit(
                                "agent-resumed",
                                json!({
                                    "agent_id": &agent_id,
                                    "goal_id": &goal_id,
                                    "message": if use_batch {
                                        format!("Approval granted — executing {} approved actions...", batch_actions.len())
                                    } else {
                                        format!("Approval granted — executing {}...", action_desc)
                                    },
                                }),
                            );
                        }
                        continue;
                    }

                    // If the cognitive loop signals it should stop, we're done
                    if !cycle_result.should_continue {
                        let success =
                            cycle_result.phase == nexus_kernel::cognitive::CognitivePhase::Learn;

                        // Build a result summary from the cognitive status
                        let result_summary = if success {
                            let status = state.cognitive_runtime.get_agent_status(&agent_id);
                            status
                                .as_ref()
                                .and_then(|s| s.active_goal.as_ref())
                                .map(|g| {
                                    format!(
                                        "Completed: {} ({} steps, {:.1} fuel used)",
                                        g.description,
                                        cycle_result.steps_executed,
                                        cycle_result.fuel_consumed
                                    )
                                })
                                .unwrap_or_else(|| "Goal completed successfully.".to_string())
                        } else {
                            cycle_result
                                .blocked_reason
                                .clone()
                                .map(|r| format!("Goal failed: {}", r))
                                .unwrap_or_else(|| {
                                    format!("Goal stopped in {} phase.", cycle_result.phase)
                                })
                        };

                        bridge.emit(
                            "agent-goal-completed",
                            json!({
                                "agent_id": &agent_id,
                                "goal_id": &goal_id,
                                "success": success,
                                "phase": format!("{}", cycle_result.phase),
                                "result_summary": result_summary,
                            }),
                        );
                        persist_task_completion(
                            &state,
                            &agent_id,
                            &goal_id,
                            if success { "completed" } else { "failed" },
                            &result_summary,
                            success,
                            cycle_result.fuel_consumed,
                        );
                        run_post_goal_evolution(
                            &bridge,
                            &state,
                            &agent_id,
                            &goal_id,
                            success,
                            cycle_result.fuel_consumed,
                        );
                        return;
                    }

                    // Brief delay between cycles
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                Err(e) => {
                    bridge.emit(
                        "agent-goal-completed",
                        json!({
                            "agent_id": &agent_id,
                            "goal_id": &goal_id,
                            "success": false,
                            "reason": format!("cognitive cycle error: {e}"),
                        }),
                    );
                    let result_summary = format!("Goal failed: cognitive cycle error: {e}");
                    persist_task_completion(
                        &state,
                        &agent_id,
                        &goal_id,
                        "failed",
                        &result_summary,
                        false,
                        0.0,
                    );
                    return;
                }
            }
        }

        // If we exhausted max_cycles
        bridge.emit(
            "agent-goal-completed",
            json!({
                "agent_id": &agent_id,
                "goal_id": &goal_id,
                "success": false,
                "reason": "max cognitive cycles reached",
            }),
        );
        persist_task_completion(
            &state,
            &agent_id,
            &goal_id,
            "failed",
            "Goal failed: max cognitive cycles reached",
            false,
            0.0,
        );

        eprintln!(
            "[agent-loop] cognitive loop finished for agent={}",
            &agent_id[..agent_id.len().min(8)]
        );
    });
}

pub(crate) fn stop_agent_goal(state: &AppState, agent_id: String) -> Result<(), String> {
    state
        .cognitive_runtime
        .stop_agent_loop(&agent_id)
        .map_err(|e| e.to_string())?;
    state.wake_and_clear_blocked_consent_wait(&agent_id);
    state.log_event(
        Uuid::parse_str(&agent_id).unwrap_or_default(),
        EventType::UserAction,
        json!({"action": "stop_agent_goal", "agent_id": agent_id}),
    );
    Ok(())
}

pub(crate) fn execute_hivemind_subtask(
    state: &AppState,
    agent_id: &str,
    description: &str,
) -> Result<String, String> {
    let goal_id = execute_agent_goal(state, agent_id.to_string(), description.to_string(), 5)?;
    spawn_cognitive_loop_with_bridge(
        BackendEventBridge::default(),
        state.clone(),
        agent_id.to_string(),
        goal_id.clone(),
    );

    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(300);
    loop {
        if started.elapsed() >= timeout {
            // Best-effort: attempt to stop the timed-out goal before returning error
            let _ = stop_agent_goal(state, agent_id.to_string());
            return Err(format!("sub-task timed out after {}s", timeout.as_secs()));
        }

        if let Ok(tasks) = state.db.load_tasks_by_agent(agent_id, 100) {
            if let Some(task) = tasks.into_iter().find(|task| task.id == goal_id) {
                if let Some(completed_at) = task.completed_at {
                    let summary = task
                        .result_json
                        .as_deref()
                        // Optional: result JSON may be absent or malformed; fall back to default summary
                        .and_then(|json| serde_json::from_str::<Value>(json).ok())
                        .and_then(|json| {
                            json.get("summary")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        })
                        .unwrap_or_else(|| {
                            format!("Sub-task '{}' completed at {}", description, completed_at)
                        });
                    return if task.success {
                        Ok(summary)
                    } else {
                        Err(summary)
                    };
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

pub(crate) fn get_agent_cognitive_status(
    state: &AppState,
    agent_id: String,
) -> Result<serde_json::Value, String> {
    match state.cognitive_runtime.get_agent_status(&agent_id) {
        Some(status) => serde_json::to_value(&status).map_err(|e| format!("serialize error: {e}")),
        None => Ok(json!({
            "phase": "Idle",
            "active_goal": null,
            "steps_completed": 0,
            "steps_total": 0,
            "fuel_remaining": 0.0,
            "cycle_count": 0
        })),
    }
}

pub(crate) fn get_agent_task_history(
    state: &AppState,
    agent_id: String,
    limit: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let tasks = state
        .db
        .load_tasks_by_agent(&agent_id, limit as usize)
        .map_err(|e| format!("load tasks error: {e}"))?;
    tasks
        .into_iter()
        .map(|t| serde_json::to_value(&t).map_err(|e| format!("serialize error: {e}")))
        .collect()
}

pub(crate) fn get_agent_memories(
    state: &AppState,
    agent_id: String,
    memory_type: Option<String>,
    limit: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let memories = state
        .db
        .load_memories(&agent_id, memory_type.as_deref(), limit as usize)
        .map_err(|e| format!("load memories error: {e}"))?;
    memories
        .into_iter()
        .map(|m| serde_json::to_value(&m).map_err(|e| format!("serialize error: {e}")))
        .collect()
}

// ── Self-Evolution Commands ──

pub(crate) fn get_self_evolution_metrics(
    state: &AppState,
    agent_id: String,
) -> Result<serde_json::Value, String> {
    // Build a temporary memory manager backed by the DB
    let mem_store = DbMemoryStore {
        db: state.db.clone(),
    };
    let memory_mgr = nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(mem_store));
    let metrics = state
        .evolution_tracker
        .get_evolution_metrics(&agent_id, &memory_mgr)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&metrics).map_err(|e| e.to_string())
}

pub(crate) fn get_self_evolution_strategies(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let strategies = state
        .evolution_tracker
        .get_agent_strategies(&agent_id)
        .map_err(|e| e.to_string())?;
    strategies
        .into_iter()
        .map(|s| serde_json::to_value(&s).map_err(|e| e.to_string()))
        .collect()
}

pub(crate) fn trigger_cross_agent_learning(state: &AppState) -> Result<u32, String> {
    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let agent_ids: Vec<String> = supervisor
        .health_check()
        .iter()
        .map(|s| s.id.to_string())
        .collect();
    drop(supervisor);

    let agent_id_refs: Vec<&str> = agent_ids.iter().map(|s: &String| s.as_str()).collect();
    let shareable = state
        .evolution_tracker
        .discover_shareable_strategies(&agent_id_refs, 0.8)
        .map_err(|e| e.to_string())?;

    let mem_store = DbMemoryStore {
        db: state.db.clone(),
    };
    let memory_mgr = nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(mem_store));

    let mut count: u32 = 0;
    for (from_agent, strategy, score) in &shareable {
        for target_id in &agent_ids {
            if target_id != from_agent {
                // Best-effort: cross-pollination of strategies is supplementary; skip failures
                let _ = state.evolution_tracker.share_learning(
                    from_agent,
                    target_id,
                    strategy,
                    *score,
                    &memory_mgr,
                );
                count += 1;
            }
        }
    }

    Ok(count)
}

// ── Hivemind Orchestration Commands ──

pub(crate) fn start_hivemind(
    state: &AppState,
    goal: String,
    agent_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    // Build AgentInfo from supervisor state
    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let agents: Vec<nexus_kernel::cognitive::AgentInfo> = agent_ids
        .iter()
        .filter_map(|id| {
            // Optional: skip agent IDs that are not valid UUIDs
            let uuid = Uuid::parse_str(id).ok()?;
            supervisor
                .get_agent(uuid)
                .map(|handle| nexus_kernel::cognitive::AgentInfo {
                    id: id.clone(),
                    capabilities: handle.manifest.capabilities.clone(),
                    available_fuel: handle.remaining_fuel as f64,
                })
        })
        .collect();
    drop(supervisor);

    let session = state
        .hivemind
        .execute_with_executor(&goal, agents, |_task_id, assigned_agent_id, task_desc| {
            execute_hivemind_subtask(state, assigned_agent_id, task_desc)
        })
        .map_err(|e| e.to_string())?;

    // Persist session
    let row = nexus_persistence::HivemindSessionRow {
        id: session.id.clone(),
        goal: session.master_goal.clone(),
        status: format!("{:?}", session.status),
        sub_tasks_json: serde_json::to_string(&session.sub_tasks)
            .unwrap_or_else(|_| "[]".to_string()),
        assignments_json: serde_json::to_string(&session.assignments)
            .unwrap_or_else(|_| "{}".to_string()),
        results_json: serde_json::to_string(&session.results).unwrap_or_else(|_| "{}".to_string()),
        fuel_consumed: session.total_fuel_consumed,
        started_at: session.started_at.clone(),
        completed_at: session.completed_at.clone(),
    };
    // Best-effort: persist hivemind session for history; in-memory state is authoritative
    let _ = state.db.save_hivemind_session(&row);

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({"action": "start_hivemind", "session_id": session.id, "goal": goal}),
    );

    serde_json::to_value(&session).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_hivemind_status(
    state: &AppState,
    session_id: String,
) -> Result<serde_json::Value, String> {
    // Try in-memory first
    if let Some(session) = state.hivemind.get_session(&session_id) {
        return serde_json::to_value(&session).map_err(|e| format!("serialize error: {e}"));
    }

    // Fall back to database
    match state.db.load_hivemind_session(&session_id) {
        Ok(Some(row)) => serde_json::to_value(&row).map_err(|e| format!("serialize error: {e}")),
        Ok(None) => Err(format!("hivemind session {session_id} not found")),
        Err(e) => Err(format!("load error: {e}")),
    }
}

pub(crate) fn cancel_hivemind(state: &AppState, session_id: String) -> Result<(), String> {
    state
        .hivemind
        .cancel_session(&session_id)
        .map_err(|e| e.to_string())?;

    // Best-effort: update persisted session status; cancellation already succeeded in-memory
    let _ = state
        .db
        .update_hivemind_session_status(&nexus_persistence::HivemindSessionRow {
            id: session_id.clone(),
            goal: String::new(),
            status: "Cancelled".to_string(),
            sub_tasks_json: "[]".to_string(),
            assignments_json: "{}".to_string(),
            results_json: "{}".to_string(),
            fuel_consumed: 0.0,
            started_at: String::new(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
        });

    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "cancel_hivemind", "session_id": session_id}),
    );

    Ok(())
}

// ── Messaging Gateway Commands ──

pub(crate) fn get_messaging_status(state: &AppState) -> Result<Vec<PlatformStatus>, String> {
    let gw = state
        .message_gateway
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    Ok(gw.get_status())
}

pub(crate) fn set_default_agent(
    state: &AppState,
    user_id: String,
    agent_id: String,
) -> Result<(), String> {
    let gw = state
        .message_gateway
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    gw.set_default_agent(&user_id, &agent_id);
    state.log_event(
        SYSTEM_UUID,
        EventType::UserAction,
        json!({"action": "set_messaging_default_agent", "user_id": user_id, "agent_id": agent_id}),
    );
    Ok(())
}
