//! agents domain implementation.

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
    GroqProvider, LlmProvider, NvidiaProvider, OllamaProvider, OpenAiProvider,
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

pub(crate) fn restore_persisted_agents(state: &AppState) {
    let saved_agents = match state.db.list_agents() {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("persistence: failed to list persisted agents: {error}");
            return;
        }
    };

    for row in saved_agents {
        let Ok(manifest) = serde_json::from_str::<AgentManifest>(&row.manifest_json) else {
            continue;
        };
        let Ok(agent_id) = Uuid::parse_str(&row.id) else {
            eprintln!("persistence: invalid restored agent id {}", row.id);
            continue;
        };
        let manifest_name = manifest.name.clone();
        let interrupted_task = state
            .db
            .load_tasks_by_agent(&row.id, 5)
            .ok() // Optional: treat DB failure as no interrupted task
            .and_then(|tasks| {
                tasks.into_iter().find(|task| {
                    task.completed_at.is_none()
                        && matches!(task.status.as_str(), "running" | "queued" | "pending")
                })
            });

        let restored = {
            let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
            match supervisor.start_agent_with_id(agent_id, manifest) {
                Ok(restored_id) => {
                    if !row.was_running {
                        // Best-effort: agent was not running at shutdown, stop it after restore
                        let _ = supervisor.stop_agent(restored_id);
                    } else if interrupted_task.is_some() {
                        // Best-effort: pause agent so interrupted task can be resumed by user
                        let _ = supervisor.pause_agent(restored_id);
                    }

                    if let Ok(Some(ledger_row)) = state.db.load_fuel_ledger(&row.id) {
                        if let Some(report) = load_fuel_report_from_row(&ledger_row) {
                            let remaining_fuel =
                                report.cap_units.saturating_sub(report.spent_units);
                            // Best-effort: restore fuel ledger from persisted state
                            let _ =
                                supervisor.restore_fuel_report(restored_id, report, remaining_fuel);
                        }
                    }
                    Ok(restored_id)
                }
                Err(error) => Err(error),
            }
        };

        match restored {
            Ok(restored_id) => {
                // NOTE: schedule registration is deferred to
                // initialize_startup_schedules() which runs inside the Tauri
                // setup closure where the Tokio runtime is available.

                let last_action = interrupted_task
                    .as_ref()
                    .map(|task| format!("interrupted: {}", task.goal))
                    .unwrap_or_else(|| {
                        if row.was_running {
                            "restored (running)".to_string()
                        } else {
                            "restored (stopped)".to_string()
                        }
                    });

                let mut meta = state.meta.lock().unwrap_or_else(|p| p.into_inner());
                meta.insert(
                    restored_id,
                    AgentMeta {
                        name: manifest_name,
                        last_action,
                    },
                );
            }
            Err(error) => {
                eprintln!("persistence: failed to restore agent {}: {error}", row.id);
            }
        }
    }
}

pub(crate) fn persist_agent_fuel_ledger(state: &AppState, agent_id: &str) {
    let Ok(agent_uuid) = Uuid::parse_str(agent_id) else {
        return;
    };
    let report = {
        let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
        supervisor.fuel_audit_report(agent_uuid)
    };
    if let Some(report) = report {
        let row = fuel_ledger_row_from_report(agent_id, &report);
        if let Err(error) = state.db.save_fuel_ledger(agent_id, &row) {
            eprintln!("persistence: save_fuel_ledger failed for {agent_id}: {error}");
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_usage_percent: f32,
    pub ram_used_gb: f64,
    pub ram_total_gb: f64,
    pub cpu_name: String,
}

pub(crate) fn get_system_info() -> Result<SystemInfo, String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    // Brief sleep to let CPU usage settle, then re-read
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = if sys.cpus().is_empty() {
        0.0
    } else {
        sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32
    };

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(SystemInfo {
        cpu_usage_percent: (cpu_usage * 10.0).round() / 10.0,
        ram_used_gb: (sys.used_memory() as f64 / 1_073_741_824.0 * 10.0).round() / 10.0,
        ram_total_gb: (sys.total_memory() as f64 / 1_073_741_824.0 * 10.0).round() / 10.0,
        cpu_name,
    })
}

pub(crate) fn start_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    voice.overlay_visible = true;
    Ok(voice.clone())
}

pub(crate) fn stop_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    voice.overlay_visible = false;
    Ok(voice.clone())
}

pub(crate) fn jarvis_status(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    Ok(voice.clone())
}

pub(crate) fn enqueue_transcendent_review(
    state: &AppState,
    agent_id: &str,
    agent_name: &str,
    manifest_json: Option<&str>,
    mode: &str,
) -> Result<String, String> {
    let existing = state
        .db
        .load_pending_consent()
        .map_err(|e| format!("db error: {e}"))?
        .into_iter()
        .find(|row| {
            row.agent_id == agent_id
                && row.operation_type == "transcendent_creation"
                // Optional: skip rows with unparseable JSON in filter predicate
                && serde_json::from_str::<serde_json::Value>(&row.operation_json)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("mode")
                            .and_then(|mode_value| mode_value.as_str())
                            .map(str::to_string)
                    })
                    .as_deref()
                    == Some(mode)
        });
    if let Some(existing) = existing {
        return Ok(existing.id);
    }

    let consent_id = Uuid::new_v4().to_string();
    let summary = match mode {
        "activate_existing" => format!("Activate L6 Transcendent agent '{agent_name}'"),
        _ => format!("Create L6 Transcendent agent '{agent_name}'"),
    };
    let side_effects = vec![
        "Maximum-autonomy L6 activation".to_string(),
        "Mandatory 60-second review before approval".to_string(),
        "Triple-audited self-modification and hardcoded cooldown protections".to_string(),
    ];
    let operation_json = json!({
        "summary": summary,
        "side_effects": side_effects,
        "fuel_cost": 0.0,
        "min_review_seconds": 60,
        "mode": mode,
        "manifest_json": manifest_json,
        "source_surface": "agents",
    });
    let now = chrono::Utc::now().to_rfc3339();
    state
        .db
        .enqueue_consent(&nexus_persistence::ConsentRow {
            id: consent_id.clone(),
            agent_id: agent_id.to_string(),
            operation_type: "transcendent_creation".to_string(),
            operation_json: operation_json.to_string(),
            hitl_tier: "Tier3".to_string(),
            status: "pending".to_string(),
            created_at: now,
            resolved_at: None,
            resolved_by: None,
        })
        .map_err(|e| format!("db error: {e}"))?;

    Ok(consent_id)
}

pub(crate) fn create_agent_immediately(
    state: &AppState,
    manifest: AgentManifest,
    manifest_json: String,
) -> Result<String, String> {
    let agent_name = manifest.name.clone();
    let agent_caps = manifest.capabilities.clone();
    let manifest_schedule = manifest.schedule.clone();
    let manifest_default_goal = manifest.default_goal.clone();
    let manifest_autonomy_level = manifest.autonomy_level.unwrap_or(0);
    let manifest_description = extract_manifest_description(&manifest_json);

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let agent_id = supervisor.start_agent(manifest).map_err(agent_error)?;
    drop(supervisor);

    // Create cryptographic identity (DID) for this agent
    let did = {
        let mut id_mgr = match state.identity_mgr.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match id_mgr.get_or_create(agent_id) {
            Ok(identity) => Some(identity.did.clone()),
            Err(_) => None, // identity creation is best-effort; agent still works
        }
    };

    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    meta_guard.insert(
        agent_id,
        AgentMeta {
            name: agent_name,
            last_action: "created".to_string(),
        },
    );

    // Persist agent to database
    if let Err(e) = state.db.save_agent(
        &agent_id.to_string(),
        &manifest_json,
        "running",
        manifest_autonomy_level,
        "native",
    ) {
        eprintln!("persistence: save_agent failed: {e}");
    }
    persist_agent_fuel_ledger(state, &agent_id.to_string());

    register_manifest_schedule(
        state,
        &agent_id.to_string(),
        manifest_schedule.as_deref(),
        manifest_default_goal.as_deref(),
        manifest_description.as_deref(),
    );

    state.log_event(
        agent_id,
        EventType::UserAction,
        json!({
            "event": "create_agent",
            "status": "ok",
            "did": did,
            "capabilities": agent_caps,
        }),
    );
    Ok(agent_id.to_string())
}

pub fn create_agent(state: &AppState, manifest_json: String) -> Result<String, String> {
    let manifest = parse_agent_manifest_json(manifest_json.as_str())?;
    if manifest.autonomy_level == Some(6) {
        let pending_agent_id = Uuid::new_v4().to_string();
        {
            let mut meta_guard = match state.meta.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            meta_guard.insert(
                Uuid::parse_str(&pending_agent_id).unwrap_or(SYSTEM_UUID),
                AgentMeta {
                    name: manifest.name.clone(),
                    last_action: "awaiting transcendent review".to_string(),
                },
            );
        }
        // Best-effort: persist pending agent for UI display; approval flow continues regardless
        let _ = state.db.save_agent(
            &pending_agent_id,
            &manifest_json,
            "pending_approval",
            6,
            "native",
        );
        let consent_id = enqueue_transcendent_review(
            state,
            &pending_agent_id,
            &manifest.name,
            Some(&manifest_json),
            "create_new",
        )?;
        state.log_event(
            SYSTEM_UUID,
            EventType::UserAction,
            json!({
                "event": "transcendent_creation_requested",
                "consent_id": consent_id,
                "agent_name": manifest.name,
            }),
        );
        return Ok(format!("approval-requested:{consent_id}"));
    }

    create_agent_immediately(state, manifest, manifest_json)
}

pub fn start_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let agent_id = parsed.to_string();
    if let Some(manifest) = find_manifest(state, &agent_id) {
        if manifest.autonomy_level == Some(6) {
            let consent_id = enqueue_transcendent_review(
                state,
                &agent_id,
                &manifest.name,
                None,
                "activate_existing",
            )?;
            update_last_action(state, parsed, "awaiting transcendent review");
            state.log_event(
                parsed,
                EventType::UserAction,
                json!({
                    "event": "transcendent_activation_requested",
                    "consent_id": consent_id,
                }),
            );
            return Ok(());
        }
    }
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.restart_agent(parsed).map_err(agent_error)?;
    drop(supervisor);
    // Best-effort: persist state to DB; in-memory supervisor is already updated
    let _ = state.db.update_agent_state(&agent_id, "running");
    persist_agent_fuel_ledger(state, &agent_id);
    let schedule_manifest = find_manifest(state, &agent_id);
    if let Some(manifest) = schedule_manifest {
        register_manifest_schedule(
            state,
            &agent_id,
            manifest.schedule.as_deref(),
            manifest.default_goal.as_deref(),
            find_manifest_description(state, &agent_id).as_deref(),
        );
    }
    update_last_action(state, parsed, "started");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "start_agent", "status": "ok"}),
    );
    Ok(())
}

pub(crate) fn stop_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    // Unregister from scheduler before stopping
    state.agent_scheduler.unregister_agent(&agent_id);
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.stop_agent(parsed).map_err(agent_error)?;
    drop(supervisor);
    // Best-effort: persist state to DB; in-memory supervisor is already updated
    let _ = state.db.update_agent_state(&agent_id, "stopped");
    persist_agent_fuel_ledger(state, &agent_id);
    update_last_action(state, parsed, "stopped");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "stop_agent", "status": "ok"}),
    );
    Ok(())
}

pub(crate) fn get_scheduled_agents(
    state: &AppState,
) -> Result<Vec<nexus_kernel::cognitive::ScheduledAgent>, String> {
    Ok(state.agent_scheduler.list())
}

pub(crate) fn clear_all_agents(state: &AppState) -> Result<usize, String> {
    // Clear in-memory supervisor state
    {
        let mut supervisor = match state.supervisor.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        supervisor.clear_all_agents();
    }
    // Clear in-memory meta
    {
        let mut meta = match state.meta.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        meta.clear();
    }
    // Clear persistence tables
    let count = state
        .db
        .clear_all_agents()
        .map_err(|e| format!("persistence error: {e}"))?;
    Ok(count)
}

pub(crate) fn pause_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.pause_agent(parsed).map_err(agent_error)?;
    drop(supervisor);
    // Best-effort: persist state to DB; in-memory supervisor is already updated
    let _ = state.db.update_agent_state(&agent_id, "paused");
    persist_agent_fuel_ledger(state, &agent_id);
    update_last_action(state, parsed, "paused");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "pause_agent", "status": "ok"}),
    );
    Ok(())
}

pub(crate) fn resume_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.resume_agent(parsed).map_err(agent_error)?;
    drop(supervisor);
    // Best-effort: persist state to DB; in-memory supervisor is already updated
    let _ = state.db.update_agent_state(&agent_id, "running");
    persist_agent_fuel_ledger(state, &agent_id);
    update_last_action(state, parsed, "resumed");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "resume_agent", "status": "ok"}),
    );
    Ok(())
}

pub(crate) fn list_agents(state: &AppState) -> Result<Vec<AgentRow>, String> {
    #[derive(Debug, Clone)]
    struct RuntimeAgentSnapshot {
        id: String,
        name: String,
        status: String,
        autonomy_level: Option<u8>,
        fuel_remaining: u64,
        fuel_budget: u64,
        capabilities: Vec<String>,
    }

    fn runtime_status_rank(status: &str) -> u8 {
        match status {
            "Running" => 6,
            "Starting" => 5,
            "Paused" => 4,
            "Created" => 3,
            "Stopping" => 2,
            "Stopped" => 1,
            "Destroyed" => 0,
            _ => 0,
        }
    }

    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut runtime_by_id = HashMap::new();
    let mut runtime_by_name = HashMap::new();
    for status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            let snapshot = RuntimeAgentSnapshot {
                id: status.id.to_string(),
                name: handle.manifest.name.clone(),
                status: status.state.to_string(),
                autonomy_level: handle.manifest.autonomy_level,
                fuel_remaining: status.remaining_fuel,
                fuel_budget: handle.manifest.fuel_budget,
                capabilities: handle.manifest.capabilities.clone(),
            };
            let should_replace = runtime_by_name
                .get(&snapshot.name)
                .map(|existing: &RuntimeAgentSnapshot| {
                    runtime_status_rank(&snapshot.status) >= runtime_status_rank(&existing.status)
                })
                .unwrap_or(true);
            if should_replace {
                runtime_by_name.insert(snapshot.name.clone(), snapshot.clone());
            }
            runtime_by_id.insert(snapshot.id.clone(), snapshot);
        }
    }
    drop(supervisor);

    let meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let id_mgr = match state.identity_mgr.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let persisted_rows = state
        .db
        .list_agents()
        .map_err(|e| format!("Failed to list agents: {e}"))?;

    let mut rows = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut seen_runtime_ids = HashSet::new();

    for row in persisted_rows {
        // Optional: manifest JSON may be corrupted or from an older schema version
        let manifest = serde_json::from_str::<JsonAgentManifest>(&row.manifest_json).ok();
        // Optional: agent ID may not be a valid UUID for legacy or pending agents
        let parsed_id = parse_agent_id(&row.id).ok();
        let runtime = runtime_by_id.get(&row.id).or_else(|| {
            manifest
                .as_ref()
                .and_then(|json| runtime_by_name.get(&json.manifest.name))
        });
        let meta = parsed_id
            .as_ref()
            .and_then(|agent_id| meta_guard.get(agent_id))
            .cloned();
        let did = parsed_id
            .as_ref()
            .and_then(|agent_id| id_mgr.get(agent_id))
            .map(|identity| identity.did.clone());

        let capabilities = runtime
            .map(|snapshot| snapshot.capabilities.clone())
            .or_else(|| {
                manifest
                    .as_ref()
                    .map(|json| json.manifest.capabilities.clone())
            })
            .unwrap_or_default();
        let autonomy_level = runtime
            .and_then(|snapshot| snapshot.autonomy_level)
            .or_else(|| {
                manifest
                    .as_ref()
                    .and_then(|json| json.manifest.autonomy_level)
            });
        let fuel_budget = runtime
            .map(|snapshot| snapshot.fuel_budget)
            .or_else(|| manifest.as_ref().map(|json| json.manifest.fuel_budget))
            .unwrap_or_default();
        let name = meta
            .as_ref()
            .map(|meta| meta.name.clone())
            .or_else(|| runtime.map(|snapshot| snapshot.name.clone()))
            .or_else(|| manifest.as_ref().map(|json| json.manifest.name.clone()))
            .unwrap_or_else(|| row.id.clone());
        let last_action = meta
            .map(|meta| meta.last_action)
            .unwrap_or_else(|| "persisted".to_string());

        if let Some(runtime) = runtime {
            seen_runtime_ids.insert(runtime.id.clone());
        }

        let description = manifest
            .as_ref()
            .and_then(|json| json.description.clone())
            .unwrap_or_default();

        rows.push(AgentRow {
            id: row.id.clone(),
            name,
            status: runtime
                .map(|snapshot| snapshot.status.clone())
                .unwrap_or_else(|| display_agent_state(&row.state)),
            autonomy_level,
            fuel_remaining: runtime
                .map(|snapshot| snapshot.fuel_remaining)
                .unwrap_or(fuel_budget),
            fuel_budget,
            last_action,
            capabilities,
            sandbox_runtime: "in-process".to_string(),
            did,
            description,
        });
        seen_ids.insert(row.id);
    }

    for (agent_id, runtime) in runtime_by_id {
        if seen_ids.contains(&agent_id) || seen_runtime_ids.contains(&agent_id) {
            continue;
        }

        if runtime.status.eq_ignore_ascii_case("stopped") {
            continue;
        }

        // Optional: agent ID may not be a valid UUID for legacy agents
        let parsed_id = parse_agent_id(&agent_id).ok();
        let meta = parsed_id
            .as_ref()
            .and_then(|id| meta_guard.get(id))
            .cloned()
            .unwrap_or(AgentMeta {
                name: runtime.name.clone(),
                last_action: "runtime".to_string(),
            });
        let did = parsed_id
            .as_ref()
            .and_then(|id| id_mgr.get(id))
            .map(|identity| identity.did.clone());

        rows.push(AgentRow {
            id: agent_id,
            name: meta.name,
            status: runtime.status,
            autonomy_level: runtime.autonomy_level,
            fuel_remaining: runtime.fuel_remaining,
            fuel_budget: runtime.fuel_budget,
            last_action: meta.last_action,
            capabilities: runtime.capabilities,
            sandbox_runtime: "in-process".to_string(),
            did,
            description: String::new(),
        });
    }

    rows.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(rows)
}

pub(crate) fn get_audit_log(
    state: &AppState,
    agent_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AuditRow>, String> {
    let parsed_agent = match agent_id {
        Some(value) if !value.trim().is_empty() => Some(parse_agent_id(value.as_str())?),
        _ => None,
    };
    let max_rows = limit.unwrap_or(200);

    let guard = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let filtered = guard
        .events()
        .iter()
        .filter(|event| {
            if let Some(required) = parsed_agent {
                return event.agent_id == required;
            }
            true
        })
        .map(event_to_row)
        .collect::<Vec<_>>();

    if filtered.len() <= max_rows {
        return Ok(filtered);
    }
    let offset = filtered.len().saturating_sub(max_rows);
    Ok(filtered[offset..].to_vec())
}

/// Build a [`ProviderSelectionConfig`] from the persisted config and environment.
/// Environment variables take precedence over config file values.
pub(crate) fn build_provider_config(config: &NexusConfig) -> ProviderSelectionConfig {
    let non_empty = |s: &str| -> Option<String> {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    };

    // Optional: env vars are optional provider configuration; missing vars yield None
    ProviderSelectionConfig {
        provider: std::env::var("LLM_PROVIDER").ok(),
        ollama_url: std::env::var("OLLAMA_URL")
            .ok()
            .or_else(|| non_empty(&config.llm.ollama_url)),
        deepseek_api_key: std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.deepseek_api_key)),
        anthropic_api_key: std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.anthropic_api_key)),
        openai_api_key: std::env::var("OPENAI_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.openai_api_key)),
        gemini_api_key: std::env::var("GEMINI_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.gemini_api_key)),
        groq_api_key: std::env::var("GROQ_API_KEY").ok(),
        mistral_api_key: std::env::var("MISTRAL_API_KEY").ok(),
        together_api_key: std::env::var("TOGETHER_API_KEY").ok(),
        fireworks_api_key: std::env::var("FIREWORKS_API_KEY").ok(),
        perplexity_api_key: std::env::var("PERPLEXITY_API_KEY").ok(),
        cohere_api_key: std::env::var("COHERE_API_KEY").ok(),
        openrouter_api_key: std::env::var("OPENROUTER_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.openrouter_api_key)),
        nvidia_api_key: std::env::var("NVIDIA_NIM_API_KEY")
            .ok()
            .or_else(|| non_empty(&config.llm.nvidia_api_key)),
        flash_model_path: std::env::var("FLASH_MODEL_PATH").ok(),
        claude_code_enabled: std::env::var("CLAUDE_CODE_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        codex_cli_enabled: std::env::var("CODEX_CLI_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    }
}

/// Select the configured LLM provider using the same logic as `send_chat`.
/// Falls back to the local Ollama provider when no real provider is configured.
pub(crate) fn get_configured_provider() -> Box<dyn LlmProvider> {
    match load_config() {
        Ok(config) => {
            let prov_config = build_provider_config(&config);
            let provider = select_provider(&prov_config).unwrap_or_else(|e| {
                eprintln!("[nexus-rag] select_provider failed: {e}, falling back to Ollama");
                Box::new(OllamaProvider::from_env())
            });
            eprintln!("[nexus-rag] selected LLM provider: {}", provider.name());
            provider
        }
        Err(_) => {
            eprintln!("[nexus-rag] config unavailable, falling back to Ollama");
            Box::new(OllamaProvider::from_env())
        }
    }
}
