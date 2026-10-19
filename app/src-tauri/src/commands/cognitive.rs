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
    capture_and_store_screen, ComputerControlEngine, InputControlStatus, ScreenRegion,
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
    model_override: Option<String>,
) -> Result<String, String> {
    assign_agent_goal_in(
        state,
        agent_id,
        goal_description,
        priority,
        model_override,
        None,
    )
}

/// [`assign_agent_goal`], with the goal's unit given (a HiveMind sub-task's
/// context, derived from its session's) or, with `None`, a goal unit of its
/// own started by the runtime (P3-HM-CONTRACT-02 §4.6 O1).
pub(crate) fn assign_agent_goal_in(
    state: &AppState,
    agent_id: String,
    goal_description: String,
    priority: u8,
    model_override: Option<String>,
    unit: Option<nexus_kernel::hm::DispatchContext>,
) -> Result<String, String> {
    // P0-FINAL-GATE (item G): an L6 (transcendent) agent needs a human
    // approval the backend cannot verify, so a goal for one is refused before
    // the rate limit, the input check, the assignment or the audit event.
    // `execute_agent_goal`, and every caller of it, reaches this check first.
    if is_transcendent_agent(state, &agent_id) {
        return Err(crate::phase0_surface::closed(
            "assign_agent_goal",
            crate::phase0_surface::Closure::ApprovalRequired,
        ));
    }
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;
    state.validate_input(&goal_description)?;
    let effective_goal_description = goal_with_manifest_context(
        &agent_id,
        &goal_description,
        find_manifest_description(state, &agent_id).as_deref(),
    );
    let mut goal = nexus_kernel::cognitive::AgentGoal::new(effective_goal_description, priority);
    goal.user_goal = goal_description.clone();
    goal.model_override = normalize_model_override(model_override);
    let goal_id = goal.id.clone();
    match unit {
        Some(ctx) => state.cognitive_runtime.assign_goal_in(&agent_id, goal, ctx),
        None => state.cognitive_runtime.assign_goal(&agent_id, goal),
    }
    .map_err(|e| e.to_string())?;
    state.log_event(
        Uuid::parse_str(&agent_id).unwrap_or_default(),
        EventType::UserAction,
        json!({"action": "assign_agent_goal", "agent_id": agent_id, "goal_id": goal_id}),
    );
    Ok(goal_id)
}

/// Start an autonomous agent loop: register the agent with the scheduler to
/// run its default goal (or `goal_override`) every `interval_seconds`
/// (default 60).
///
/// P0-FINAL-GATE (item G): an L6 (transcendent) agent is refused before
/// anything is read from its manifest or registered with the scheduler.
/// The refusal for an autonomous-loop interval outside 60..=3599 seconds.
pub(crate) const AUTONOMOUS_LOOP_INTERVAL: &str =
    "interval_seconds must be from 60 to 3599: an agent schedule fires at most once per minute";

pub(crate) fn start_autonomous_loop(
    state: &AppState,
    agent_id: String,
    interval_seconds: Option<u64>,
    goal_override: Option<String>,
) -> Result<(), String> {
    if is_transcendent_agent(state, &agent_id) {
        return Err(crate::phase0_surface::closed(
            "start_autonomous_loop",
            crate::phase0_surface::Closure::ApprovalRequired,
        ));
    }
    let interval = interval_seconds.unwrap_or(60);
    // P0-FINAL-GATE (item K): agent schedules fire at most once per minute,
    // and the minute step of "0 */N * * * *" allows 1 to 59. Any other
    // interval is refused here with a bounded reason, rather than as a cron
    // expression the caller never wrote (under 60 s it was refused by the
    // scheduler, from 3600 s it was unparsable).
    if !(60..3600).contains(&interval) {
        return Err(AUTONOMOUS_LOOP_INTERVAL.to_string());
    }
    let cron_expr = format!("0 */{} * * * *", interval / 60); // every N minutes

    let manifest = find_manifest(state, &agent_id);
    let goal = goal_override
        .or_else(|| manifest.as_ref().and_then(|m| m.default_goal.clone()))
        .unwrap_or_else(|| "Execute autonomous task".to_string());
    let description = find_manifest_description(state, &agent_id);

    let full_goal = goal_with_manifest_context(&agent_id, &goal, description.as_deref());

    state
        .agent_scheduler
        .register_agent(&agent_id, &cron_expr, &full_goal)
        .map_err(agent_error)?;

    Ok(())
}

/// Normalize a raw model override value into `Option<String>`.
///
/// Sentinels `""`, `"auto"`, `"mock"` all collapse to `None` so existing
/// manifest/auto-resolver fallback applies. Any other non-empty string is
/// preserved as an explicit override.
pub(crate) fn normalize_model_override(raw: Option<String>) -> Option<String> {
    let trimmed = raw.map(|value| value.trim().to_string())?;
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower == "auto" || lower == "mock" {
        return None;
    }
    Some(trimmed)
}

/// The `task_history` row of a goal at its start (`goal_record`,
/// P3-HM-CONTRACT-02 §6.5): a labelled write in the goal's unit, then its
/// fuel budget as bookkeeping. A goal without a unit writes nothing.
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
    let Ok(task_id) = Uuid::parse_str(goal_id) else {
        eprintln!("persistence: save_task start refused: the goal id is not a UUID");
        return;
    };
    let written = state
        .cognitive_runtime
        .with_goal_context(agent_id, goal_id, |issuer, ctx| {
            issuer
                .save_task(
                    ctx,
                    &nexus_kernel::hm::TaskWrite {
                        task_id,
                        goal: &goal,
                        status: "running",
                        steps_json: "[]",
                        result_json: None,
                        completed: false,
                        success: false,
                    },
                    &[],
                )
                .map(|_| issuer)
                .map_err(|e| e.to_string())
                .and_then(|issuer| {
                    issuer
                        .task_bookkeeping(
                            goal_id,
                            &nexus_kernel::hm::TaskBookkeeping {
                                fuel_consumed: Some(0.0),
                                fuel_budget,
                                ..Default::default()
                            },
                        )
                        .map_err(|e| e.to_string())
                })
        });
    match written {
        Some(Ok(())) => {}
        Some(Err(error)) => eprintln!("persistence: save_task start failed: {error}"),
        None => eprintln!("persistence: save_task start refused: the goal has no unit"),
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
    // P3-HM-CONTRACT-02 §6.7: the completion is a content update labelled
    // in the goal's unit; the fuel is bookkeeping. A goal whose unit has
    // ended (its loop was removed) records only its bookkeeping columns.
    let completed = Uuid::parse_str(goal_id).ok().and_then(|task_id| {
        state
            .cognitive_runtime
            .with_goal_context(agent_id, goal_id, |issuer, ctx| {
                issuer
                    .complete_task(ctx, task_id, status, Some(&result_json), success, &[])
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
    });
    let now = chrono::Utc::now().to_rfc3339();
    let bookkeeping = nexus_kernel::hm::TaskBookkeeping {
        status: Some(status),
        fuel_consumed: Some(fuel_consumed),
        completed_at: completed.is_none().then_some(now.as_str()),
        success: Some(success),
        ..Default::default()
    };
    if let Some(Err(error)) = &completed {
        eprintln!("persistence: complete_task failed: {error}");
    }
    match state.hm_issuer() {
        Err(error) => eprintln!("persistence: task bookkeeping refused: {error}"),
        Ok(issuer) => {
            if let Err(error) = issuer.task_bookkeeping(goal_id, &bookkeeping) {
                eprintln!("persistence: task bookkeeping failed: {error}");
            }
        }
    }
    // P3-HM-CONTRACT-02 §15.2 D3: the goal id, its status and its success,
    // never the result summary.
    state.log_event(
        Uuid::parse_str(agent_id).unwrap_or_default(),
        EventType::StateChange,
        json!({
            "action": "agent_goal_completed",
            "goal_id": goal_id,
            "status": status,
            "success": success,
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

/// Evolution after a goal (P3-HM-CONTRACT-02 §11.3): it runs in the goal's
/// post-goal context (the goal's own policy snapshot, taint and working
/// set; never a freshly resolved policy), and every row it writes is
/// labelled in that context (§6.5).
pub(crate) fn run_post_goal_evolution(
    bridge: &BackendEventBridge,
    state: &AppState,
    agent_id: &str,
    goal_id: &str,
    success: bool,
    fallback_fuel_consumed: f64,
    hive: Option<Arc<HivemindAdmission>>,
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
    let Ok(issuer) = state.hm_issuer() else {
        return;
    };
    // The goal's unit, derived for evolution; without one nothing runs.
    let Ok(mut post_goal) = state.cognitive_runtime.derive_post_goal(agent_id, goal_id) else {
        return;
    };

    let (duration_secs, fuel_budget) =
        task_timing_for_goal(state, agent_id, goal_id).unwrap_or((0.0, fallback_fuel_consumed));
    let fuel_consumed = state
        .db
        .load_tasks_by_agent(agent_id, 200)
        .ok()
        .and_then(|tasks| tasks.into_iter().find(|task| task.id == goal_id))
        .map(|task| task.fuel_consumed)
        .unwrap_or(fallback_fuel_consumed);
    // The goal type is the goal's own text, from its unit (never re-read
    // from a stored row).
    let goal_type = state
        .cognitive_runtime
        .get_agent_status_fast(agent_id)
        .and_then(|status| status.active_goal)
        .filter(|goal| goal.id == goal_id)
        .map(|goal| goal.description)
        .unwrap_or_else(|| "scheduled_goal".to_string())
        .to_lowercase();
    let strategy_hash = nexus_kernel::cognitive::hash_strategy(&goal_type);

    // Best-effort: evolution tracking is supplementary; failure does not affect task completion
    let _ = state.evolution_tracker.record_task_result(
        issuer,
        &post_goal,
        goal_id,
        &strategy_hash,
        &goal_type,
        success,
        fuel_consumed,
        duration_secs,
        fuel_budget,
        60.0,
    );

    let completed_count = state
        .db
        .load_tasks_by_agent(agent_id, 500)
        .unwrap_or_default()
        .into_iter()
        .filter(|task| task.completed_at.is_some())
        .count();

    if completed_count > 0 && completed_count % 5 == 0 {
        if let Ok(Some(best)) = state
            .evolution_tracker
            .select_best_strategy(issuer, &post_goal, &goal_type)
        {
            let best_strategy = best.field("strategy_hash").unwrap_or_default().to_string();
            // Best-effort: inject best strategy into agent memory for future
            // planning; derived from the strategy record (its parent).
            let _ = nexus_kernel::cognitive::AgentMemoryManager::record_procedural(
                issuer,
                &post_goal,
                nexus_kernel::hm::SourceKind::PlannerStrategyInjection,
                &format!("planner_strategy_injection:{best_strategy}"),
                1.0,
                &[nexus_kernel::hm::Parent::Record(best.record_ref())],
            );
            if let Ok(strategies) = state.evolution_tracker.get_agent_strategies(agent_id) {
                if let Some(strategy) = strategies
                    .into_iter()
                    .find(|entry| entry.strategy_hash == best_strategy)
                {
                    let score = strategy.composite_score;
                    let generation = (completed_count / 5) as u64;
                    // P3-HM-CONTRACT-02 §15.2 D4: no strategy hash in the audit
                    // payload (the frontend emit below is local UI delivery).
                    state.log_event(
                        Uuid::parse_str(agent_id).unwrap_or_default(),
                        EventType::StateChange,
                        json!({
                            "action": "agent_evolved_strategy",
                            "message": format!("Agent {} evolved strategy. New composite score: {:.3}", agent_name, score),
                            "new_score": score,
                            "generation": generation,
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
        let llm = GatewayPlannerLlm::for_session(state, hive);
        // P3-HM-CONTRACT-02 §11.3: the goal's context, not a freshly
        // resolved current policy.
        // Best-effort: prompt optimization is a background improvement; failure is non-fatal
        let _ = state.evolution_tracker.optimize_planning_prompt(
            issuer,
            &mut post_goal,
            &current_prompt,
            &outcomes,
            &llm,
        );
    }
}

/// The desktop's model of agent units (P3-HM-CONTRACT-02 §11.0, §12.4).
pub(crate) use crate::hm_dispatch::GatewayPlannerLlm;

/// A provider dispatch: sends a prompt to a provider for a model.
#[cfg(test)]
pub(crate) type PlannerDispatch<'a> =
    &'a dyn Fn(
        &dyn LlmProvider,
        &str,
        &str,
    ) -> Result<nexus_connectors_llm::providers::LlmResponse, AgentError>;

/// Chooses the planner's provider under `policy` and sends `prompt` to it
/// through `dispatch`, once. A failed dispatch is the result: no other
/// provider is tried. (Tests: production dispatch goes through
/// `hm_dispatch::select_transport`, which uses the same `planner_provider`
/// and `local_only_target`.)
#[cfg(test)]
pub(crate) fn planner_dispatch(
    policy: AgentLlmPolicy,
    route_model: Option<String>,
    prov_config: &ProviderSelectionConfig,
    ollama_base: Result<String, String>,
    runtime: &dyn LocalOllamaRuntime,
    prompt: &str,
    dispatch: PlannerDispatch<'_>,
) -> Result<String, AgentError> {
    let (provider, model) =
        planner_provider(policy, route_model, prov_config, ollama_base, runtime)?;
    let response = dispatch(provider.as_ref(), prompt, &model)?;
    let pre_strip_len = response.output_text.len();
    let starts_with_think = response.output_text.trim_start().starts_with("<think>");
    let has_closing_think = response.output_text.contains("</think>");
    let stripped = strip_think_tags(&response.output_text);
    let post_strip_len = stripped.len();
    eprintln!(
        "[planner] prompt={} chars -> response={} chars (stripped={} chars)",
        prompt.len(),
        pre_strip_len,
        post_strip_len
    );
    if pre_strip_len > 0 && post_strip_len == 0 {
        eprintln!(
            "[planner] WARNING: response stripped to empty. pre_strip={} starts_with_think={} has_closing={}",
            pre_strip_len, starts_with_think, has_closing_think
        );
    }
    Ok(stripped)
}

/// The planner's provider and model for `route_model` under `policy`. A
/// local-only policy is decided by [`local_only_provider`] and never reaches
/// the route resolution or default model below. `ollama_base` is the
/// authorized Ollama address.
pub(crate) fn planner_provider(
    policy: AgentLlmPolicy,
    route_model: Option<String>,
    prov_config: &ProviderSelectionConfig,
    ollama_base: Result<String, String>,
    runtime: &dyn LocalOllamaRuntime,
) -> Result<(Box<dyn LlmProvider>, String), AgentError> {
    // Skip flash routes — already handled by ACTIVE_FLASH_PROVIDER above.
    // If we reach here, Flash was requested but couldn't be resolved.
    if let Some(route_model) = route_model.as_deref() {
        if route_model.starts_with("flash:")
            || route_model.starts_with("flash/")
            || route_model == "flash"
        {
            // Flash provider wasn't available — return clear error instead of
            // silently falling back to Ollama (which causes confusing 404 errors).
            return Err(AgentError::SupervisorError(
                "Flash Inference is selected but no model is loaded. \
                 Go to the Agents page and click 'Load Model' to start a Flash session."
                    .to_string(),
            ));
        }
    }
    if let Some(local) = local_only_provider(policy, route_model.as_deref(), ollama_base, runtime)
        .map_err(AgentError::SupervisorError)?
    {
        return Ok(local);
    }
    let (provider, model) = if let Some(route_model) = route_model {
        provider_from_prefixed_model(&route_model, prov_config).map_err(|e| {
            AgentError::SupervisorError(format!(
                "Cannot resolve model '{}': {}. Please select a different model.",
                route_model, e
            ))
        })?
    } else {
        // Use smart default model detection (checks API keys in priority order)
        let default_model = crate::commands::chat_llm::get_default_model();
        if default_model.contains('/') {
            // Prefixed model (e.g. "anthropic/claude-sonnet-4-6") — resolve provider from it
            provider_from_prefixed_model(&default_model, prov_config).map_err(|e| {
                AgentError::SupervisorError(format!(
                    "Cannot resolve default model '{}': {}. Add an API key in Settings.",
                    default_model, e
                ))
            })?
        } else if default_model == "mock-1" {
            // No provider configured at all — fail with clear message
            return Err(AgentError::SupervisorError(
                "No LLM provider configured. Add an API key in Settings (Anthropic, OpenAI, etc.) \
                 or install Ollama for local models."
                    .to_string(),
            ));
        } else {
            let provider = select_provider(prov_config)?;
            (provider, default_model)
        }
    };
    Ok((provider, model))
}

// ── Local-only routing (P3 D-2) ──────────────────────────────────────────

/// The refusal for an agent's work whose routing policy is unknown.
pub(crate) fn unresolved_policy_error() -> AgentError {
    AgentError::SupervisorError(
        "local-only: the agent's routing policy could not be established, so no model is used"
            .to_string(),
    )
}

/// A local Ollama runtime, as a local-only request consults it before any
/// prompt is sent.
pub(crate) trait LocalOllamaRuntime {
    /// The names of the models installed at `base`.
    fn installed_models(&self, base: &str) -> Result<Vec<String>, String>;
    /// Whether `model` is installed at `base` and runs there.
    fn verify_local_model(&self, base: &str, model: &str) -> Result<(), String>;
}

/// The Ollama runtime at the authorized loopback address.
pub(crate) struct OllamaLocalRuntime;

impl LocalOllamaRuntime for OllamaLocalRuntime {
    fn installed_models(&self, base: &str) -> Result<Vec<String>, String> {
        // P3 D-2 R2: listed over the direct loopback connection, as the
        // prompt is sent.
        nexus_connectors_llm::providers::OllamaProvider::local_only(base)
            .map_err(|e| e.to_string())?
            .list_models()
            .map(|models| models.into_iter().map(|model| model.name).collect())
            .map_err(|e| e.to_string())
    }

    fn verify_local_model(&self, base: &str, model: &str) -> Result<(), String> {
        nexus_connectors_llm::gateway::verify_local_ollama_model(base, model)
            .map_err(|e| e.to_string())
    }
}

/// Where a local-only agent's prompt goes: a model of the Ollama runtime at
/// a loopback address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalOnlyTarget {
    pub(crate) base: String,
    pub(crate) model: String,
}

/// The verified local target for a local-only agent's `route` (P3 D-2).
///
/// The address is the authorized Ollama address, and only when it is a
/// loopback address. The model is the one the route names (`ollama/<model>`
/// or an Ollama `name:tag`), or, with no model named (`auto`, `ollama`, no
/// route), the runtime's preferred installed model. Any other route, a cloud
/// provider, a CLI provider, a bare provider id, is refused, never
/// substituted. A cloud model of the runtime is refused by its name, and
/// the runtime must then confirm the model runs locally.
pub(crate) fn local_only_target(
    route: Option<&str>,
    authorized_base: Result<String, String>,
    runtime: &dyn LocalOllamaRuntime,
) -> Result<LocalOnlyTarget, String> {
    let base = authorized_base.map_err(|reason| format!("local-only: {reason}"))?;
    let base =
        nexus_connectors_llm::gateway::local_only_ollama_base(&base).map_err(|e| e.to_string())?;
    let route = route.map(str::trim).unwrap_or_default();
    let model = if route.is_empty() || route == "auto" || route == "ollama" {
        let mut installed: Vec<String> = runtime
            .installed_models(&base)
            .map_err(|reason| format!("local-only: no local model is available: {reason}"))?
            .into_iter()
            .filter(|model| !nexus_connectors_llm::gateway::ollama_model_name_is_cloud(model))
            .collect();
        installed.sort_by_key(|model| std::cmp::Reverse(ollama_model_preference(model)));
        installed
            .into_iter()
            .next()
            .ok_or_else(|| "local-only: no local model is installed".to_string())?
    } else if let Some(model) = route.strip_prefix("ollama/") {
        model.to_string()
    } else if !route.contains('/') && route.contains(':') {
        route.to_string()
    } else {
        return Err(format!(
            "local-only: the route '{route}' is not a local model, and the agent may use only local models"
        ));
    };
    if nexus_connectors_llm::gateway::ollama_model_name_is_cloud(&model) {
        return Err(format!(
            "local-only: '{model}' is a cloud model of the Ollama runtime, which relays it to a remote service"
        ));
    }
    runtime.verify_local_model(&base, &model)?;
    Ok(LocalOnlyTarget { base, model })
}

/// A provider and the model it is asked for.
pub(crate) type ChosenProvider = (Box<dyn LlmProvider>, String);

/// The provider a prompt goes to under `policy`, when the policy decides it
/// (P3 D-2): `None` for an unrestricted policy, the verified local Ollama
/// model for a local-only one, and a refusal when the policy is unknown.
/// `ollama_base` is the authorized Ollama address.
pub(crate) fn local_only_provider(
    policy: AgentLlmPolicy,
    route: Option<&str>,
    ollama_base: Result<String, String>,
    runtime: &dyn LocalOllamaRuntime,
) -> Result<Option<ChosenProvider>, String> {
    match policy {
        AgentLlmPolicy::Unrestricted => Ok(None),
        AgentLlmPolicy::Unresolved => Err(unresolved_policy_error().to_string()),
        AgentLlmPolicy::LocalOnly => {
            let target = local_only_target(route, ollama_base, runtime)?;
            // P3 D-2 R2: the prompt goes to the verified runtime directly,
            // over the same route the verification used: no proxy setting
            // applies and no redirect is followed.
            let provider =
                nexus_connectors_llm::providers::OllamaProvider::local_only(&target.base)
                    .map_err(|e| e.to_string())?;
            Ok(Some((Box::new(provider), target.model)))
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

/// The simulation's model (class XS, P3-HM-CONTRACT-02 §12.2): every call
/// runs under the explicit `System` context the simulation mints for it
/// (`nexus_kernel::simulation::SimulationLlm`), through the issuer; a reply
/// unusable for the simulation is an error.
#[cfg(not(test))]
pub(crate) struct SimulationPlannerLlm {
    pub(crate) state: AppState,
}

#[cfg(not(test))]
impl nexus_kernel::cognitive::PlannerLlm for SimulationPlannerLlm {
    fn plan_query(
        &self,
        ctx: &mut nexus_kernel::hm::DispatchContext,
        prompt: &mut nexus_kernel::hm::PromptSource<'_>,
    ) -> Result<nexus_kernel::hm::DerivedItem, nexus_kernel::errors::AgentError> {
        crate::hm_dispatch::dispatch_unit_with(
            &self.state,
            ctx,
            prompt,
            crate::hm_dispatch::DispatchOptions {
                hive: None,
                model: crate::hm_dispatch::ModelChoice::Default,
                max_tokens: 2048,
                text: crate::hm_dispatch::ReplyText::StripThink,
            },
            &|admitted: &str, response: String| {
                if simulation_response_is_usable(admitted, &response) {
                    Ok(response)
                } else {
                    Err(format!(
                        "LLM response unusable for simulation ({} chars). Configure a capable LLM provider.",
                        response.len()
                    ))
                }
            },
        )
        .map(|(reply, _)| reply)
        .map_err(|e| {
            nexus_kernel::errors::AgentError::SupervisorError(format!(
                "World Simulation requires a running LLM. Error: {e}"
            ))
        })
    }
}

/// The simulation's test model: dispatched through the issuer like the
/// production adapter, on a test transport attested non-local, answering
/// [`simulation_mock_response`].
#[cfg(test)]
pub(crate) struct TestSimulationPlannerLlm {
    pub(crate) issuer: Arc<nexus_kernel::hm::DispatchIssuer>,
}

#[cfg(test)]
impl nexus_kernel::cognitive::PlannerLlm for TestSimulationPlannerLlm {
    fn plan_query(
        &self,
        ctx: &mut nexus_kernel::hm::DispatchContext,
        prompt: &mut nexus_kernel::hm::PromptSource<'_>,
    ) -> Result<nexus_kernel::hm::DerivedItem, nexus_kernel::errors::AgentError> {
        // `attest_route` of a value that is no provider: non-local.
        let attestation = nexus_connectors_llm::attest::attest_route(&());
        self.issuer
            .dispatch(
                ctx,
                &mut |c: &mut nexus_kernel::hm::DispatchContext| {
                    prompt(c).map(|envelope| (envelope, (), attestation))
                },
                &mut |(), admitted: &str| Ok(simulation_mock_response(admitted)),
            )
            .map_err(|e| nexus_kernel::errors::AgentError::SupervisorError(e.to_string()))
    }
}

/// A simulation model that dispatches nothing (no issuer).
#[cfg(test)]
pub(crate) struct SimulationRefused;

#[cfg(test)]
impl nexus_kernel::cognitive::PlannerLlm for SimulationRefused {
    fn plan_query(
        &self,
        _ctx: &mut nexus_kernel::hm::DispatchContext,
        _prompt: &mut nexus_kernel::hm::PromptSource<'_>,
    ) -> Result<nexus_kernel::hm::DerivedItem, nexus_kernel::errors::AgentError> {
        Err(nexus_kernel::errors::AgentError::SupervisorError(
            "simulation: refused: no dispatch issuer".to_string(),
        ))
    }
}

/// The simulation's model with its context source: every call mints an
/// explicit `System("simulation")` request context from the process's
/// issuer (class XS); with no issuer, nothing is dispatched.
pub(crate) fn simulation_llm(
    state: &AppState,
    planner: Arc<dyn nexus_kernel::cognitive::PlannerLlm>,
) -> nexus_kernel::simulation::SimulationLlm {
    let issuer = state.hm_issuer().ok().cloned();
    nexus_kernel::simulation::SimulationLlm::new(
        planner,
        Arc::new(move || {
            let issuer = issuer.as_ref().ok_or_else(|| {
                AgentError::SupervisorError("simulation: refused: no dispatch issuer".to_string())
            })?;
            issuer
                .begin_unit(
                    nexus_kernel::hm::Principal::System("simulation"),
                    nexus_kernel::hm::Unit::Request {
                        request_id: Uuid::new_v4(),
                    },
                )
                .map_err(|e| AgentError::SupervisorError(format!("simulation: {e}")))
        }),
    )
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
        /// Returned (and audited by the scheduler) for a skipped tick.
        const LOOP_ACTIVE: &str =
            "scheduled run skipped: the agent's cognitive loop is still running";
        /// A tick already under way when the owner stopped the agent (an
        /// owner's stop removes the schedule first).
        const UNSCHEDULED: &str = "scheduled run skipped: the agent's schedule was removed";
        /// A tick whose agent was stopped while its goal was assigned.
        const STOPPED: &str = "scheduled run skipped: the agent was stopped as it started";

        // Whether this tick's schedule exists as it begins, before anything
        // else: an owner's stop removes it first, so a tick that loses it
        // meanwhile was overtaken by a stop (checked again before any
        // restart, under the supervisor's lock).
        let scheduled = || {
            self.state
                .agent_scheduler
                .list()
                .iter()
                .any(|scheduled| scheduled.agent_id == agent_id)
        };
        let was_scheduled = scheduled();

        // P0-FINAL-GATE (item G): a scheduled tick for a transcendent (L6)
        // agent is refused before anything is audited, restarted or assigned,
        // with the refusal `assign_agent_goal` would give the goal.
        if is_transcendent_agent(&self.state, agent_id) {
            return Err(crate::phase0_surface::closed(
                "assign_agent_goal",
                crate::phase0_surface::Closure::ApprovalRequired,
            ));
        }
        let agent_uuid = Uuid::parse_str(agent_id).map_err(|e| format!("invalid agent id: {e}"))?;
        // P0-FG resource bound: a scheduled tick never starts a second loop
        // beside the agent's running one. A desktop loop holds its
        // cancellation entry from spawn until it exits on any path. Nothing
        // is assigned, restarted or persisted for a skipped tick.
        let loop_running = self
            .state
            .cognitive_cancellations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .keys()
            .any(|key| key == agent_id || Uuid::parse_str(key).ok() == Some(agent_uuid));
        if loop_running {
            self.state.log_event(
                agent_uuid,
                EventType::StateChange,
                json!({
                    "action": "scheduled_execution_skipped",
                    "reason": "agent_loop_active",
                }),
            );
            return Err(LOOP_ACTIVE.to_string());
        }
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
                    // Checked under the lock the owner's stop takes after it
                    // removed the schedule: an agent the owner stopped (Phase
                    // Three was told), or one whose schedule this tick saw
                    // vanish, is not brought back by a tick.
                    let owner_stopped = self
                        .state
                        .real_world()
                        .is_ok_and(|world| world.was_stopped(agent_id));
                    if !scheduled() && (was_scheduled || owner_stopped) {
                        return Err(UNSCHEDULED.to_string());
                    }
                    supervisor.restart_agent(agent_uuid).map_err(agent_error)?;
                }
            }
        }

        let goal_id = execute_agent_goal(
            &self.state,
            agent_id.to_string(),
            default_goal.to_string(),
            5,
            None,
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

        // A stop that came while this tick assigned the goal: no loop starts
        // for it. Checked, and the loop registered, under the supervisor's
        // lock, which the owner's stop takes (a loop that starts before the
        // stop ends at its next cycle, once the agent is stopped). A paused
        // agent's tick runs, as pausing keeps schedules.
        let supervisor = self
            .state
            .supervisor
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let stopped = supervisor.get_agent(agent_uuid).is_none_or(|handle| {
            matches!(
                handle.state,
                AgentState::Stopping | AgentState::Stopped | AgentState::Destroyed
            )
        });
        let refusal = if stopped {
            Some(STOPPED)
        } else if was_scheduled && !scheduled() {
            Some(UNSCHEDULED)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            drop(supervisor);
            // Only this tick's goal ends (not one the owner gave since), as
            // the scheduler's decision, recorded as skipped.
            end_goal_loop(&self.state, agent_id, &goal_id);
            persist_task_completion(
                &self.state,
                agent_id,
                &goal_id,
                "failed",
                refusal,
                false,
                0.0,
            );
            return Err(refusal.to_string());
        }

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
        drop(supervisor);

        Ok(())
    }
}

/// Bridges the ScheduleRunner to the Tauri cognitive loop for run_agent tasks.
pub(crate) struct RunnerGoalCallback {
    pub(crate) state: AppState,
}

impl nexus_kernel::scheduler::ScheduleGoalCallback for RunnerGoalCallback {
    fn execute_goal(&self, agent_id: &str, goal: &str) -> Result<String, String> {
        execute_agent_goal(&self.state, agent_id.to_string(), goal.to_string(), 5, None)
    }
}

/// P0-FINAL-GATE (item G): the bounded reason an enabled Warden review gives.
/// In Phase Zero no agent can be verified as the Warden: the prebuilt Warden
/// is L6 and never registered, and an agent that is merely named
/// "nexus-warden" (any caller can create one) is not the Warden.
pub(crate) const WARDEN_REVIEW_UNAVAILABLE: &str =
    "Warden review is unavailable in Phase Zero: no agent can be verified as the Warden";

pub(crate) struct WardenReviewEngine {
    // Kept so the executor's construction is unchanged; the Phase Zero
    // decision below reads no state.
    #[allow(dead_code)]
    pub(crate) state: AppState,
}

impl nexus_kernel::actuators::ActionReviewEngine for WardenReviewEngine {
    fn review(
        &self,
        _actor_agent_id: &str,
        _actor_name: &str,
        _action: &PlannedAction,
    ) -> Result<nexus_kernel::actuators::ActionReviewDecision, String> {
        let config = load_config().map_err(agent_error)?;
        Ok(self.review_with(config.governance.enable_warden_review))
    }
}

impl WardenReviewEngine {
    /// The Phase Zero review decision.
    ///
    /// P0-FINAL-GATE (item G): a disabled review allows, as before. An enabled
    /// review is denied with the bounded `WARDEN_REVIEW_UNAVAILABLE` reason
    /// and nothing else happens: no Warden is looked up, by name or
    /// otherwise; no model is resolved or queried; and the engine writes no
    /// audit event or consent request of its own. It used to take any running
    /// agent named "nexus-warden" as the Warden, whose model's YES allowed the
    /// action and was audited as a Warden review.
    pub(crate) fn review_with(
        &self,
        enabled: bool,
    ) -> nexus_kernel::actuators::ActionReviewDecision {
        if !enabled {
            return nexus_kernel::actuators::ActionReviewDecision::Allow {
                reason: "Warden governance review disabled".to_string(),
            };
        }
        nexus_kernel::actuators::ActionReviewDecision::Deny {
            reason: WARDEN_REVIEW_UNAVAILABLE.to_string(),
        }
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
    model_override: Option<String>,
) -> Result<String, String> {
    execute_agent_goal_in(
        state,
        agent_id,
        goal_description,
        priority,
        model_override,
        None,
    )
}

/// [`execute_agent_goal`] with the goal's unit given (a HiveMind sub-task's).
pub(crate) fn execute_agent_goal_in(
    state: &AppState,
    agent_id: String,
    goal_description: String,
    priority: u8,
    model_override: Option<String>,
    unit: Option<nexus_kernel::hm::DispatchContext>,
) -> Result<String, String> {
    let before_snapshot = capture_agent_snapshot(state, &agent_id);
    // Assign the goal to the cognitive runtime
    let goal_id = assign_agent_goal_in(
        state,
        agent_id.clone(),
        goal_description,
        priority,
        model_override,
        unit,
    )?;
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

// Shared synchronous entry used by the desktop driver and lock regressions.
pub(crate) fn run_cognitive_cycle(
    state: &AppState,
    agent_id: &str,
    planner: &nexus_kernel::cognitive::CognitivePlanner,
    memory_mgr: &nexus_kernel::cognitive::AgentMemoryManager,
    executor: &dyn nexus_kernel::cognitive::loop_runtime::ActionExecutor,
) -> Result<nexus_kernel::cognitive::CycleResult, AgentError> {
    // Pass the shared writer, never a guard, through routing and execution.
    // The route of each dispatch comes from the goal's unit (R4).
    let mut audit = state.audit.clone();
    state.cognitive_runtime.run_cycle_with_evolution(
        agent_id,
        planner,
        memory_mgr,
        executor,
        &mut audit,
        Some(&state.evolution_tracker),
    )
}

/// Why `Phase0AgentExecutor` refuses an agent action, or `None` when it runs.
///
/// P0-002C5A: only actions that need no filesystem, process or OS-input
/// authority run. Everything else, including any action variant added later,
/// is refused (`Closure::AgentExecution`). A non-HTTP fetch URL (`file:` or
/// any other scheme) would be a local path under another name.
///
/// Final Gate item B (Architect decision D4): an http(s) web fetch is refused
/// too (`Closure::NetworkDestination`). Its URL is the model's choice, and
/// the only thing that admitted it was the agent's `allowed_endpoints`, which
/// comes from the interface at creation or from a stored record. Neither is
/// an egress grant, and Phase Zero has no backend-issued agent allowlist.
/// Web search stays: it reaches fixed or operator-configured hosts.
pub(crate) fn phase0_agent_action_closure(
    action: &nexus_kernel::cognitive::PlannedAction,
) -> Option<crate::phase0_surface::Closure> {
    use crate::phase0_surface::Closure;
    use nexus_kernel::cognitive::PlannedAction;
    match action {
        PlannedAction::WebFetch { url } => {
            let url = url.trim_start().to_ascii_lowercase();
            if url.starts_with("https://") || url.starts_with("http://") {
                Some(Closure::NetworkDestination)
            } else {
                Some(Closure::AgentExecution)
            }
        }
        PlannedAction::LlmQuery { .. }
        | PlannedAction::Noop
        | PlannedAction::MemoryStore { .. }
        | PlannedAction::MemoryRecall { .. }
        | PlannedAction::SendNotification { .. }
        | PlannedAction::AgentMessage { .. }
        | PlannedAction::HitlRequest { .. }
        | PlannedAction::WebSearch { .. }
        | PlannedAction::KnowledgeGraphUpdate { .. }
        | PlannedAction::KnowledgeGraphQuery { .. } => None,
        _ => Some(Closure::AgentExecution),
    }
}

/// P0-002C5A, Phase Three: the production agent executor.
///
/// Every action is classified by the Phase Three classification first.
/// Inert actions keep their Phase Zero route: the closure above, then the
/// kernel registry, which is given no workspace root at all. Governed
/// actions (network, browser, observation and input on the agent display,
/// governed tools) go only to the Phase Three pipeline, never to the
/// registry: they are committed, authorized by the owner's standing grants
/// (R2 waits for the owner's native approval) and executed once. Everything
/// else stays closed. A kernel approval allowance is not authority here.
pub(crate) struct Phase0AgentExecutor<E> {
    inner: E,
    governed: Option<crate::governed_real_world::AgentBridge>,
}

impl<E: nexus_kernel::cognitive::loop_runtime::ActionExecutor>
    nexus_kernel::cognitive::loop_runtime::ActionExecutor for Phase0AgentExecutor<E>
{
    fn execute(
        &self,
        agent_id: &str,
        action: &nexus_kernel::cognitive::PlannedAction,
        audit: &mut dyn nexus_kernel::audit::AuditWriter,
        hitl_approved: bool,
    ) -> Result<String, String> {
        match executor_route(self.governed.as_ref(), agent_id, action) {
            ExecutorRoute::Inner => self.inner.execute(agent_id, action, audit, hitl_approved),
            ExecutorRoute::Decided(result) => result,
        }
    }

    /// The same decision for a step of a unit (P3-HM-CONTRACT-02 §11.3):
    /// what the inner executor runs, it runs in the unit; what is decided
    /// here (a governed action's outcome) is a product of the unit.
    fn execute_in(
        &self,
        scope: &mut nexus_kernel::cognitive::StepScope<'_>,
        agent_id: &str,
        action: &nexus_kernel::cognitive::PlannedAction,
        audit: &mut dyn nexus_kernel::audit::AuditWriter,
        hitl_approved: bool,
    ) -> Result<nexus_kernel::cognitive::StepOutput, String> {
        match executor_route(self.governed.as_ref(), agent_id, action) {
            ExecutorRoute::Inner => {
                self.inner
                    .execute_in(scope, agent_id, action, audit, hitl_approved)
            }
            ExecutorRoute::Decided(result) => result.map(|text| {
                let item =
                    nexus_kernel::cognitive::StepItem::Derived(scope.ctx.produce(text.clone()));
                nexus_kernel::cognitive::StepOutput { text, item }
            }),
        }
    }
}

/// Where the production executor sends an action.
enum ExecutorRoute {
    /// To the inner (Phase Zero) executor.
    Inner,
    /// Decided here: a refusal, or the Phase Three outcome.
    Decided(Result<String, String>),
}

/// The production executor's one decision, for both of its entry points.
/// Without Phase Three it is the Phase Zero executor (closure first, then
/// the inner executor). With it, the Phase Three classification decides:
/// only inert actions reach the closure and then the inner executor,
/// governed actions reach only the Phase Three bridge, and closed actions
/// keep their Phase Zero refusal.
fn executor_route(
    governed: Option<&crate::governed_real_world::AgentBridge>,
    agent_id: &str,
    action: &nexus_kernel::cognitive::PlannedAction,
) -> ExecutorRoute {
    use nexus_governed_control::planned::{classify, Disposition};
    let Some(bridge) = governed else {
        // Without Phase Three, the Phase Zero closure stands.
        if let Some(closure) = phase0_agent_action_closure(action) {
            return ExecutorRoute::Decided(Err(crate::phase0_surface::closed(
                action.action_type(),
                closure,
            )));
        }
        return ExecutorRoute::Inner;
    };
    match classify(action) {
        Disposition::Inert => {
            if let Some(closure) = phase0_agent_action_closure(action) {
                return ExecutorRoute::Decided(Err(crate::phase0_surface::closed(
                    action.action_type(),
                    closure,
                )));
            }
            ExecutorRoute::Inner
        }
        Disposition::Governed(intent) => {
            ExecutorRoute::Decided(bridge.act(agent_id, &intent, warden_reviews(action)))
        }
        Disposition::Orchestrated { max_steps } => {
            ExecutorRoute::Decided(Ok(orchestration_guidance(max_steps)))
        }
        Disposition::Closed(_) => ExecutorRoute::Decided(Err(crate::phase0_surface::closed(
            action.action_type(),
            phase0_agent_action_closure(action)
                .unwrap_or(crate::phase0_surface::Closure::AgentExecution),
        ))),
    }
}

/// Whether the Warden review covers `action`: every action but the reads
/// the kernel registry exempts (`should_apply_governance_review`), of which
/// only `WebFetch` is governed.
fn warden_reviews(action: &nexus_kernel::cognitive::PlannedAction) -> bool {
    !matches!(
        action,
        nexus_kernel::cognitive::PlannedAction::FileRead { .. }
            | nexus_kernel::cognitive::PlannedAction::WebSearch { .. }
            | nexus_kernel::cognitive::PlannedAction::WebFetch { .. }
            | nexus_kernel::cognitive::PlannedAction::MemoryRecall { .. }
            | nexus_kernel::cognitive::PlannedAction::KnowledgeGraphQuery { .. }
            | nexus_kernel::cognitive::PlannedAction::Noop
    )
}

/// Tests: the production executor with an isolated Phase Three control and
/// a fixed Warden setting.
#[cfg(test)]
#[cfg(target_os = "linux")]
impl<E> Phase0AgentExecutor<E> {
    pub(crate) fn with_real_world(
        mut self,
        world: Arc<crate::governed_real_world::RealWorld>,
    ) -> Self {
        self.governed = Some(crate::governed_real_world::AgentBridge::with_warden(
            world,
            || false,
        ));
        self
    }
}

/// What a `ComputerAction` returns: the agent takes it one governed step at
/// a time.
fn orchestration_guidance(max_steps: u32) -> String {
    format!(
        "computer actions are taken one governed step at a time on the agent display: \
         observe it, then issue at most {max_steps} single input actions, each \
         committed and authorized on its own"
    )
}

/// The executor every production cognitive loop runs with. Its LlmQuery
/// steps dispatch through `llm` (the loop's adapter, which carries a
/// HiveMind session's admission when the goal is a sub-task).
pub(crate) fn phase0_agent_executor(
    state: &AppState,
    memory: Arc<nexus_kernel::cognitive::AgentMemoryManager>,
    llm: GatewayPlannerLlm,
) -> Phase0AgentExecutor<nexus_kernel::cognitive::RegistryExecutor> {
    Phase0AgentExecutor {
        inner: nexus_kernel::cognitive::RegistryExecutor::new(
            // No workspace root: an actuator that needed one would fail closed.
            std::path::PathBuf::new(),
            state.audit.clone(),
            state.supervisor.clone(),
            Some(Arc::new(WardenReviewEngine {
                state: state.clone(),
            })),
        )
        .with_llm_handler(Arc::new(llm))
        .with_memory_manager(memory),
        governed: state.real_world().ok().map(|world| {
            // A paused or stopped agent acts in the real world no more,
            // whichever way it was paused or stopped.
            let supervisor = state.supervisor.clone();
            crate::governed_real_world::AgentBridge::new(
                world,
                Arc::new(move |agent: &str| {
                    Uuid::parse_str(agent).ok().is_some_and(|id| {
                        supervisor
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .get_agent(id)
                            .is_some_and(|handle| handle.state == AgentState::Running)
                    })
                }),
            )
        }),
    }
}

/// Owns one cognitive loop's entry in `AppState::cognitive_cancellations`.
///
/// The entry is the loop's cancellation flag: the Chat Stop button sets it,
/// and a scheduled tick is skipped while it exists. A newer loop for the same
/// agent replaces the entry, so when a loop ends only its own flag is removed
/// (`Arc::ptr_eq`). An older loop's exit never erases a newer loop's entry.
pub(crate) struct CognitiveCancelGuard {
    state: AppState,
    agent_id: String,
    flag: Arc<AtomicBool>,
}

impl CognitiveCancelGuard {
    /// Register a new loop's flag for `agent_id`, replacing any earlier
    /// loop's entry, and return the flag with the guard that removes it.
    pub(crate) fn register(state: &AppState, agent_id: &str) -> (Arc<AtomicBool>, Self) {
        let flag = Arc::new(AtomicBool::new(false));
        state
            .cognitive_cancellations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(agent_id.to_string(), flag.clone());
        let guard = Self {
            state: state.clone(),
            agent_id: agent_id.to_string(),
            flag: flag.clone(),
        };
        (flag, guard)
    }
}

impl Drop for CognitiveCancelGuard {
    fn drop(&mut self) {
        let mut map = self
            .state
            .cognitive_cancellations
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if map
            .get(&self.agent_id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.flag))
        {
            map.remove(&self.agent_id);
        }
    }
}

pub(crate) fn spawn_cognitive_loop_with_bridge(
    bridge: BackendEventBridge,
    state: AppState,
    agent_id: String,
    goal_id: String,
) {
    spawn_cognitive_loop_in(bridge, state, agent_id, goal_id, None);
}

/// [`spawn_cognitive_loop_with_bridge`] for a HiveMind session's sub-task:
/// its goal's unit was derived from the session's (P3-HM-CONTRACT-02
/// §11.3), and each of its model calls first requalifies the session's
/// admission (P3 HiveMind INFOFLOW).
pub(crate) fn spawn_cognitive_loop_in(
    bridge: BackendEventBridge,
    state: AppState,
    agent_id: String,
    goal_id: String,
    hivemind_admission: Option<Arc<HivemindAdmission>>,
) {
    // G1b: register a cancellation flag so the Chat Stop button can break the
    // loop cleanly. Flag handle is stored in AppState keyed by agent_id so the
    // Tauri `stop_agent` command can look it up and set it.
    let (cancel_flag, cancel_guard) = CognitiveCancelGuard::register(&state, &agent_id);

    tauri::async_runtime::spawn(async move {
        // The task owns the guard: it removes this loop's own entry on any
        // exit path (normal, break, return, panic), and also if the task is
        // dropped before it runs.
        let _cancel_guard = cancel_guard;

        // NOTE: Do NOT install a custom panic hook here. Calling prev_hook(info)
        // inside a hook can cause a double-panic (which aborts the entire process).
        // The catch_unwind below is sufficient for recovery.

        let llm = GatewayPlannerLlm::for_loop(&state, hivemind_admission.clone());
        let planner = nexus_kernel::cognitive::CognitivePlanner::new(Box::new(llm.clone()));
        let mem_store = DbMemoryStore {
            db: state.db.clone(),
        };
        let memory_mgr = nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(mem_store));

        // P0-002C5A: the process working directory is not an agent workspace
        // ("Option B" is withdrawn). See `phase0_agent_executor`.
        let memory_mgr = Arc::new(memory_mgr);
        let executor = phase0_agent_executor(&state, memory_mgr.clone(), llm);

        let max_cycles = 500u32;
        'cycle_loop: for _cycle in 0..max_cycles {
            // A stopped agent runs no cycle: an owner's stop reaches a loop
            // whose cancel flag it could not set (its start was under way).
            if agent_stopped(&state, &agent_id) {
                bridge.emit(
                    "agent-goal-completed",
                    json!({
                        "agent_id": &agent_id,
                        "goal_id": &goal_id,
                        "success": false,
                        "reason": "the agent is stopped",
                        "result_summary": "Goal ended: the agent is stopped.",
                    }),
                );
                persist_task_completion(
                    &state,
                    &agent_id,
                    &goal_id,
                    "failed",
                    "Goal ended: the agent is stopped.",
                    false,
                    0.0,
                );
                return;
            }
            // G1b: check cancellation flag before starting a new cycle. If the
            // user hit Stop while we were sleeping between cycles, exit cleanly
            // and emit a Failed goal-completed event.
            if cancel_flag.load(Ordering::Relaxed) {
                bridge.emit(
                    "agent-goal-completed",
                    json!({
                        "agent_id": &agent_id,
                        "goal_id": &goal_id,
                        "success": false,
                        "reason": "cancelled by user",
                        "result_summary": "Goal cancelled by user.",
                    }),
                );
                persist_task_completion(
                    &state,
                    &agent_id,
                    &goal_id,
                    "failed",
                    "Goal cancelled by user.",
                    false,
                    0.0,
                );
                break 'cycle_loop;
            }

            let before_snapshot = capture_agent_snapshot(&state, &agent_id);

            // Run the cognitive cycle inside catch_unwind
            let cycle_result_or_panic =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_cognitive_cycle(&state, &agent_id, &planner, &memory_mgr, &executor)
                }));

            // G1b: poll cancellation again immediately after the potentially
            // long-running cycle returns, so we exit before the inter-cycle
            // sleep (or the next iteration's setup) wastes time.
            if cancel_flag.load(Ordering::Relaxed) {
                bridge.emit(
                    "agent-goal-completed",
                    json!({
                        "agent_id": &agent_id,
                        "goal_id": &goal_id,
                        "success": false,
                        "reason": "cancelled by user",
                        "result_summary": "Goal cancelled by user.",
                    }),
                );
                persist_task_completion(
                    &state,
                    &agent_id,
                    &goal_id,
                    "failed",
                    "Goal cancelled by user.",
                    false,
                    0.0,
                );
                break 'cycle_loop;
            }

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
                                "source_surface": "agents",
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
                                "source_surface": "agents",
                            })
                        };

                        let consent_id = Uuid::new_v4().to_string();
                        let notify =
                            state.register_blocked_consent_wait(&agent_id, &goal_id, &consent_id);
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
                            json!({
                                "consent_id": consent_id,
                                "status": &resolution_status,
                                "agent_id": &agent_id,
                                "source_surface": "agents",
                            }),
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
                        // G1a: respect the kernel's explicit success flag. Reaching
                        // the Learn phase is necessary but not sufficient — cycles
                        // can finish in Learn yet produce no real output (empty
                        // LLM responses, planner fallback that yielded nothing),
                        // and the kernel flags those via cycle_result.success.
                        let success = cycle_result.success
                            && cycle_result.phase == nexus_kernel::cognitive::CognitivePhase::Learn;

                        // Hoist status — single call, used by both result_summary and emit
                        let status = state.cognitive_runtime.get_agent_status(&agent_id);

                        // Build a result summary from the cognitive status
                        let result_summary = if success {
                            // Prefer actual LLM output over formatted status string
                            let last_output = status
                                .as_ref()
                                .and_then(|s| s.last_step_result.clone())
                                .filter(|t| !t.is_empty());
                            last_output.unwrap_or_else(|| {
                                status
                                    .as_ref()
                                    .and_then(|s| s.active_goal.as_ref())
                                    .map(|g| {
                                        format!(
                                            "Completed: {} ({} steps, {:.1} fuel used)",
                                            g.user_goal,
                                            cycle_result.steps_executed,
                                            cycle_result.fuel_consumed
                                        )
                                    })
                                    .unwrap_or_else(|| "Goal completed successfully.".to_string())
                            })
                        } else {
                            // G1a: prefer the explicit failure_reason over the
                            // ambiguous blocked_reason, so silent-failure cycles
                            // surface "plan executed but produced no output" etc.
                            cycle_result
                                .failure_reason
                                .clone()
                                .or_else(|| cycle_result.blocked_reason.clone())
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
                                "final_output": status.as_ref().and_then(|s| s.last_step_result.clone()),
                                "user_goal": status.as_ref()
                                    .and_then(|s| s.active_goal.as_ref())
                                    .map(|g| g.user_goal.clone()),
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
                            hivemind_admission.clone(),
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

/// Whether the agent has no authority to run: the supervisor records it as
/// stopped (or destroyed), or holds no record of it at all (an id that is
/// not one, or an agent cleared or never registered). Its loop runs no
/// further cycle and a HiveMind session gives it no sub-task. A missing
/// agent counts as stopped, as at the scheduler's tick and Phase Three's
/// bridge (which also refuses a paused one).
pub(crate) fn agent_stopped(state: &AppState, agent_id: &str) -> bool {
    Uuid::parse_str(agent_id).ok().is_none_or(|id| {
        state
            .supervisor
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_agent(id)
            .is_none_or(|handle| {
                matches!(
                    handle.state,
                    AgentState::Stopping | AgentState::Stopped | AgentState::Destroyed
                )
            })
    })
}

/// End the agent's loop only while it still drives `goal_id` (a goal given
/// since is left alone: compared and removed under the loops' one guard),
/// and wake only that goal's consent wait, for the scheduler's or a
/// session's own reasons: not an owner's action, so none is recorded.
/// Waits while any agent's cycle holds the loop lock.
fn end_goal_loop(state: &AppState, agent_id: &str, goal_id: &str) {
    if state
        .cognitive_runtime
        .stop_agent_loop_if(agent_id, goal_id)
    {
        state.wake_and_clear_goal_consent_wait(agent_id, goal_id);
    }
}

/// Remove the agent's loop (this waits while any agent's cycle holds the
/// loop lock), wake a consent wait it sleeps in, and record it.
pub(crate) fn end_agent_loop(state: &AppState, agent_id: &str) -> Result<(), String> {
    state
        .cognitive_runtime
        .stop_agent_loop(agent_id)
        .map_err(|e| e.to_string())?;
    state.wake_and_clear_blocked_consent_wait(agent_id);
    state.log_event(
        Uuid::parse_str(agent_id).unwrap_or_default(),
        EventType::UserAction,
        json!({"action": "stop_agent_goal", "agent_id": agent_id}),
    );
    Ok(())
}

/// The longest a HiveMind sub-task's goal may run.
const SUBTASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Run one sub-task of `session` on `agent_id`: assign its goal, start its
/// loop, and wait for the goal to end. A cancelled session assigns no
/// further sub-task, and a stopped agent takes none.
pub(crate) fn execute_hivemind_subtask(
    state: &AppState,
    session: &hive::HiveSession,
    admission: &Arc<HivemindAdmission>,
    session_ctx: &nexus_kernel::hm::DispatchContext,
    subtask_id: u32,
    agent_id: &str,
    description: &str,
) -> nexus_kernel::cognitive::SubtaskRun {
    use nexus_kernel::cognitive::SubtaskRun;
    if session.cancelled() {
        return SubtaskRun::refused(hive::SESSION_CANCELLED);
    }
    // A stopped agent takes no sub-task: an owner's stop ends its part in a
    // session (which reassigns the sub-task, or fails it).
    if agent_stopped(state, agent_id) {
        return SubtaskRun::refused(format!("sub-task refused: agent '{agent_id}' is stopped"));
    }
    // P3 HiveMind INFOFLOW: a sub-task is started (and stored) only while
    // its session still qualifies as admitted.
    if let Err(reason) = admission.requalify() {
        return SubtaskRun::refused(format!("sub-task refused: {reason}"));
    }
    // P3-HM-CONTRACT-02 §11.3: the sub-task's unit derives from the
    // session's: the participant's own policy ⊔ the session floor, the
    // session's taint inherited.
    let sub_ctx = match (state.hm_issuer(), Uuid::parse_str(agent_id)) {
        (Ok(issuer), Ok(participant)) => {
            session_ctx.derive_subtask(issuer, participant, subtask_id)
        }
        (Err(e), _) => return SubtaskRun::refused(format!("sub-task refused: {e}")),
        (_, Err(e)) => return SubtaskRun::refused(format!("sub-task refused: {e}")),
    };
    let sub_ctx = match sub_ctx {
        Ok(ctx) => ctx,
        Err(e) => {
            return SubtaskRun::refused(format!(
                "sub-task refused: {}: {e}",
                unresolved_policy_error()
            ))
        }
    };
    let goal_id = match execute_agent_goal_in(
        state,
        agent_id.to_string(),
        description.to_string(),
        5,
        None,
        Some(sub_ctx),
    ) {
        Ok(goal_id) => goal_id,
        Err(e) => return SubtaskRun::refused(e),
    };
    // A cancellation that came while the goal was assigned: it ends before
    // its loop starts.
    if session.cancelled() {
        end_goal_loop(state, agent_id, &goal_id);
        return SubtaskRun::refused(hive::SESSION_CANCELLED);
    }
    spawn_cognitive_loop_in(
        BackendEventBridge::default(),
        state.clone(),
        agent_id.to_string(),
        goal_id.clone(),
        Some(admission.clone()),
    );
    let finished = await_subtask(
        state,
        session,
        agent_id,
        &goal_id,
        description,
        SUBTASK_TIMEOUT,
    );
    // The sub-task's unit and its final output come from its loop, never
    // from the stored row (§11.3): the session joins the unit before the
    // merge carries the output.
    let (context, output) = match state.cognitive_runtime.goal_outcome(agent_id, &goal_id) {
        Some((ctx, output)) => (Some(ctx), output),
        None => (None, None),
    };
    let outcome = match finished {
        Ok(()) => output.ok_or_else(|| format!("Sub-task '{description}' produced no output")),
        Err(reason) => Err(reason),
    };
    SubtaskRun { outcome, context }
}

/// Wait (polling, within `timeout`) for a sub-task's goal to end. The
/// session's cancellation or its time limit ends only this goal; the
/// owner's stop of the agent has ended its loop already. Only the goal's
/// bookkeeping is read (completion and success, never its content).
fn await_subtask(
    state: &AppState,
    session: &hive::HiveSession,
    agent_id: &str,
    goal_id: &str,
    description: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        if session.cancelled() {
            end_goal_loop(state, agent_id, goal_id);
            return Err(hive::SESSION_CANCELLED.to_string());
        }
        if started.elapsed() >= timeout {
            // The session's own limit, not an owner's stop: only this
            // sub-task's goal ends.
            end_goal_loop(state, agent_id, goal_id);
            return Err(format!("sub-task timed out after {}s", timeout.as_secs()));
        }
        // The owner stopped the agent: its loop has ended, and the sub-task
        // with it.
        if agent_stopped(state, agent_id) {
            return Err(format!("sub-task ended: agent '{agent_id}' was stopped"));
        }

        if let Ok(tasks) = state.db.load_tasks_by_agent(agent_id, 100) {
            if let Some(task) = tasks.into_iter().find(|task| task.id == goal_id) {
                if let Some(completed_at) = task.completed_at {
                    return if task.success {
                        Ok(())
                    } else {
                        Err(format!(
                            "Sub-task '{}' failed at {}",
                            description, completed_at
                        ))
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
    match state.cognitive_runtime.get_agent_status_fast(&agent_id) {
        Some(status) => serde_json::to_value(&status).map_err(|e| format!("serialize error: {e}")),
        None => Ok(json!({
            "phase": "Idle",
            "active_goal": null,
            "steps_completed": 0,
            "steps_total": 0,
            "fuel_remaining": 0.0,
            "cycle_count": 0,
            "started_at_secs": 0
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
        .get_evolution_metrics(&agent_id, &memory_mgr, &state.db)
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

    // P3-HM-CONTRACT-02 §14 rule 3: each copy is written in a unit of its
    // target (a goal-kind unit started for the learning, O1), labelled with
    // the source's label ⊔ the target's, and refused when restricted for a
    // target that is not local-only. A target whose policy cannot be
    // established receives nothing.
    let issuer = state.hm_issuer()?;
    let mut count: u32 = 0;
    for (from_agent, strategy, score) in &shareable {
        for target_id in &agent_ids {
            if target_id != from_agent {
                let Ok(target) = Uuid::parse_str(target_id) else {
                    continue;
                };
                let Ok(target_ctx) = issuer.begin_unit(
                    nexus_kernel::hm::Principal::Agent(target),
                    nexus_kernel::hm::Unit::Goal {
                        goal_id: Uuid::new_v4(),
                    },
                ) else {
                    continue;
                };
                // Best-effort: cross-pollination of strategies is supplementary; skip failures
                let _ = state.evolution_tracker.share_learning(
                    issuer,
                    &target_ctx,
                    from_agent,
                    strategy,
                    *score,
                );
                count += 1;
            }
        }
    }

    Ok(count)
}

// ── Hivemind Orchestration Commands ──

/// Admit a HiveMind session before any thread is started for it: within
/// the cap on sessions at once, then the agent-execution rate limit (a
/// refusal there gives the place back). The owner cancels it by the
/// identity recorded here.
pub(crate) fn admit_hivemind(state: &AppState) -> Result<hive::HiveSession, String> {
    let session = state.hive_sessions.admit()?;
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({"action": "hivemind_session_admitted", "session": session.id()}),
    );
    Ok(session)
}

pub(crate) fn start_hivemind(
    state: &AppState,
    session: &hive::HiveSession,
    goal: String,
    agent_ids: Vec<String>,
) -> Result<serde_json::Value, String> {
    start_hivemind_with(state, session, goal, agent_ids, |admission| {
        Box::new(GatewayPlannerLlm::for_session(state, Some(admission)))
    })
}

/// [`start_hivemind`] whose decomposition and merge model is made by
/// `planner` from the session's admission (production: the session's
/// adapter, which requalifies the admission before each dispatch).
pub(crate) fn start_hivemind_with(
    state: &AppState,
    session: &hive::HiveSession,
    goal: String,
    agent_ids: Vec<String>,
    planner: impl FnOnce(Arc<HivemindAdmission>) -> Box<dyn nexus_kernel::cognitive::HivemindLlm>,
) -> Result<serde_json::Value, String> {
    let (agents, policy) = hivemind_participants(state, &agent_ids)?;
    // P3 HiveMind INFOFLOW: the session's admission, requalified before each
    // sub-task it starts and each model call of its work.
    let admission = Arc::new(HivemindAdmission {
        state: state.clone(),
        participants: agents.iter().map(|agent| agent.id.clone()).collect(),
        policy,
    });
    let mut session_ctx = hivemind_session_context(state, &agents, policy)?;
    let llm = planner(admission.clone());
    let mut next_subtask = 0u32;

    // P3-HM-CONTRACT-02 §11.3: the decomposition and the merge run under the
    // session's context; every sub-task's context derives from it and joins
    // it before the merge.
    let outcome = state
        .hivemind
        .execute_with_executor_via(
            llm.as_ref(),
            &mut session_ctx,
            &goal,
            agents,
            |session_ctx, _task_id, assigned_agent_id, task_desc| {
                next_subtask += 1;
                execute_hivemind_subtask(
                    state,
                    session,
                    &admission,
                    session_ctx,
                    next_subtask,
                    assigned_agent_id,
                    task_desc,
                )
            },
        )
        .map_err(|e| e.to_string())?;

    // Persist session
    let row = nexus_persistence::HivemindSessionRow {
        id: outcome.id.clone(),
        goal: outcome.master_goal.clone(),
        status: format!("{:?}", outcome.status),
        sub_tasks_json: serde_json::to_string(&outcome.sub_tasks)
            .unwrap_or_else(|_| "[]".to_string()),
        assignments_json: serde_json::to_string(&outcome.assignments)
            .unwrap_or_else(|_| "{}".to_string()),
        results_json: serde_json::to_string(&outcome.results).unwrap_or_else(|_| "{}".to_string()),
        fuel_consumed: outcome.total_fuel_consumed,
        started_at: outcome.started_at.clone(),
        completed_at: outcome.completed_at.clone(),
    };
    // Best-effort: persist hivemind session for history; in-memory state is authoritative
    let _ = state.db.save_hivemind_session(&row);

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        // P3-HM-CONTRACT-02 §15.2 D6: the session id, never the goal.
        json!({
            "action": "start_hivemind",
            "session_id": outcome.id,
            "session": session.id(),
        }),
    );

    serde_json::to_value(&outcome).map_err(|e| format!("serialize error: {e}"))
}

/// The participants of a HiveMind session and the session's routing policy
/// (P3 HiveMind), established before any model call. Every requested id must
/// be an agent the supervisor registers, named once; an invalid, unknown or
/// repeated id, or no participant, refuses the session (none is skipped).
/// The session's policy is its participants' common policy
/// ([`hivemind_session_policy`]): a mix of local-only and unrestricted
/// participants, or a participant whose policy cannot be established,
/// refuses it.
pub(crate) fn hivemind_participants(
    state: &AppState,
    agent_ids: &[String],
) -> Result<(Vec<nexus_kernel::cognitive::AgentInfo>, AgentLlmPolicy), String> {
    let refused = |reason: String| format!("hivemind session refused: {reason}");
    if agent_ids.is_empty() {
        return Err(refused("no participant was named".to_string()));
    }
    let mut agents: Vec<nexus_kernel::cognitive::AgentInfo> = Vec::new();
    {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        for id in agent_ids {
            let uuid =
                Uuid::parse_str(id).map_err(|_| refused(format!("'{id}' is not an agent id")))?;
            let canonical = uuid.to_string();
            if agents.iter().any(|agent| agent.id == canonical) {
                return Err(refused(format!("'{id}' is named more than once")));
            }
            let handle = supervisor
                .get_agent(uuid)
                .ok_or_else(|| refused(format!("'{id}' is not a registered agent")))?;
            agents.push(nexus_kernel::cognitive::AgentInfo {
                id: canonical,
                capabilities: handle.manifest.capabilities.clone(),
                available_fuel: handle.remaining_fuel as f64,
            });
        }
    }
    let ids: Vec<String> = agents.iter().map(|agent| agent.id.clone()).collect();
    let policy = hivemind_session_policy(state, &ids).map_err(refused)?;
    Ok((agents, policy))
}

/// The context of a HiveMind session (P3-HM-CONTRACT-02 §11.3): a unit of
/// its first participant; a local-only session (all its participants
/// local-only) is local throughout. Every sub-task's context derives from
/// it.
fn hivemind_session_context(
    state: &AppState,
    agents: &[nexus_kernel::cognitive::AgentInfo],
    policy: AgentLlmPolicy,
) -> Result<nexus_kernel::hm::DispatchContext, String> {
    let refused = |reason: String| format!("hivemind session refused: {reason}");
    let issuer = state
        .hm_issuer()
        .map_err(|e| refused(format!("no dispatch issuer: {e}")))?;
    let lead = agents
        .first()
        .and_then(|agent| Uuid::parse_str(&agent.id).ok())
        .ok_or_else(|| refused("no participant was named".to_string()))?;
    let mut ctx = issuer
        .begin_unit(
            nexus_kernel::hm::Principal::Agent(lead),
            nexus_kernel::hm::Unit::HiveSubtask {
                session_id: Uuid::new_v4(),
                subtask_id: 0,
            },
        )
        .map_err(|e| refused(format!("{}: {e}", unresolved_policy_error())))?;
    if policy == AgentLlmPolicy::LocalOnly {
        ctx.tighten_local_only();
    }
    Ok(ctx)
}

/// A HiveMind session's admission and context for `agent_ids`, made as
/// [`start_hivemind_with`] makes them (tests that drive a sub-task
/// directly).
#[cfg(test)]
pub(crate) fn hivemind_test_session(
    state: &AppState,
    agent_ids: &[String],
) -> (Arc<HivemindAdmission>, nexus_kernel::hm::DispatchContext) {
    let (agents, policy) = hivemind_participants(state, agent_ids).unwrap();
    let admission = Arc::new(HivemindAdmission {
        state: state.clone(),
        participants: agents.iter().map(|agent| agent.id.clone()).collect(),
        policy,
    });
    let ctx = hivemind_session_context(state, &agents, policy).unwrap();
    (admission, ctx)
}

/// The routing policy of a HiveMind session of the registered agents `ids`
/// (P3 HiveMind INFOFLOW): every participant's own route policy as the
/// backend store holds it (P3-HM-CONTRACT-02 §4.2), which must be the same
/// for all. A sub-task's loop stores what it did under the participant that
/// ran it, and an unrestricted participant's later work is bound by no
/// session, so a session with a local-only participant admits only
/// local-only ones. A mix, or a participant whose policy cannot be
/// established, is refused.
fn hivemind_session_policy(state: &AppState, ids: &[String]) -> Result<AgentLlmPolicy, String> {
    let issuer = state
        .hm_issuer()
        .map_err(|e| format!("no dispatch issuer: {e}"))?;
    let (mut local_only, mut unrestricted) = (false, false);
    for id in ids {
        let policy = Uuid::parse_str(id)
            .map_err(|e| e.to_string())
            .and_then(|uuid| {
                issuer
                    .resolve_agent_route_policy(uuid)
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| format!("'{id}': {}: {e}", unresolved_policy_error()))?;
        if policy.local_only() {
            local_only = true;
        } else {
            unrestricted = true;
        }
    }
    if local_only && unrestricted {
        return Err(
            "the session mixes local-only and unrestricted participants; \
                    with a local-only participant every participant must be local-only"
                .to_string(),
        );
    }
    Ok(if local_only {
        AgentLlmPolicy::LocalOnly
    } else {
        AgentLlmPolicy::Unrestricted
    })
}

/// A HiveMind session's admission, kept for the session's work (P3 HiveMind
/// INFOFLOW): its participants and the policy it was admitted under. Each
/// sub-task the session starts and each model call of its work first
/// requalifies it; the admission is handed to that work explicitly.
pub(crate) struct HivemindAdmission {
    state: AppState,
    participants: Vec<String>,
    policy: AgentLlmPolicy,
}

impl HivemindAdmission {
    /// The session still qualifies as it was admitted: its participants'
    /// current policies give the same session policy. A participant's
    /// policy changed either way, a mix, or a policy that can no longer be
    /// established ends the session's work.
    pub(crate) fn requalify(&self) -> Result<(), String> {
        let changed =
            "hivemind session ended: a participant's routing policy changed during the session";
        match hivemind_session_policy(&self.state, &self.participants) {
            Ok(policy) if policy == self.policy => Ok(()),
            Ok(_) => Err(changed.to_string()),
            Err(reason) => Err(format!("{changed}: {reason}")),
        }
    }
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
    // A session under way (by the identity its admission recorded) is told
    // to stop: it assigns no further sub-task, and the one it waits on ends.
    if state.hive_sessions.cancel(&session_id) {
        state.log_event(
            SYSTEM_UUID,
            EventType::UserAction,
            json!({"action": "cancel_hivemind", "session": session_id}),
        );
        return Ok(());
    }
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

pub(crate) mod hive;

#[cfg(test)]
mod hive_tests;
#[cfg(test)]
mod local_only_tests;
#[cfg(test)]
mod lock_tests;
#[cfg(test)]
mod scheduled_tests;
#[cfg(test)]
mod stop_tests;
