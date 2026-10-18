//! simulation domain implementation.

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

// ── Genesis Protocol (agent-writes-agent) ──────────────────────────────────

// ── Consciousness commands ────────────────────────────────────────────

pub(crate) fn get_agent_consciousness(
    state: &AppState,
    agent_id: String,
) -> Result<String, String> {
    let mut engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let snapshot = engine.get_agent_state_snapshot(&agent_id);
    serde_json::to_string(&snapshot).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_user_behavior_state(state: &AppState) -> Result<String, String> {
    let engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let behavior = engine.get_user_behavior();
    serde_json::to_string(behavior).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn report_user_keystroke(
    state: &AppState,
    is_deletion: bool,
    timestamp: u64,
) -> Result<(), String> {
    let mut engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.report_user_event(&nexus_kernel::consciousness::UserInputEvent::Keystroke {
        timestamp,
        is_deletion,
    });
    Ok(())
}

pub(crate) fn get_consciousness_history(
    state: &AppState,
    agent_id: String,
    limit: u32,
) -> Result<String, String> {
    let mut engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.get_agent_history(&agent_id, limit);
    serde_json::to_string(&history).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn reset_agent_consciousness(state: &AppState, agent_id: String) -> Result<(), String> {
    let mut engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.reset_agent(&agent_id);
    Ok(())
}

// ── Dream Forge commands ──────────────────────────────────────────────

pub(crate) fn get_dream_status(state: &AppState) -> Result<String, String> {
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(&engine.scheduler).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_dream_queue(state: &AppState) -> Result<String, String> {
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(&engine.scheduler.priority_queue)
        .map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_morning_briefing(state: &AppState) -> Result<String, String> {
    let issuer = state.hm_issuer()?;
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    let llm = crate::hm_dispatch::GatewayPlannerLlm::new(state);
    let briefing = engine.generate_morning_briefing(issuer, &llm);
    serde_json::to_string(&briefing).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn set_dream_config(
    state: &AppState,
    enabled: bool,
    idle_trigger_minutes: u32,
    budget_tokens: u64,
    budget_calls: u32,
) -> Result<(), String> {
    let mut engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    engine
        .scheduler
        .configure(enabled, idle_trigger_minutes, budget_tokens, budget_calls);
    Ok(())
}

pub(crate) fn trigger_dream_now(state: &AppState) -> Result<String, String> {
    let issuer = state.hm_issuer()?;
    let mut engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    // P3-HM-CONTRACT-02 §11.3, §12.1: each entry is dreamed in a dream unit
    // started from its own label (agent-owned: AR; OwnerDirect: HR;
    // unlabelled: LOCAL_ONLY), through the issuer.
    let llm = crate::hm_dispatch::GatewayPlannerLlm::new(state);
    let results = engine.enter_dream_state(issuer, &llm);
    serde_json::to_string(&results).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_dream_history(state: &AppState, limit: u32) -> Result<String, String> {
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    let recent = engine.scheduler.recent_dreams(limit);
    serde_json::to_string(&recent).map_err(|e| format!("serialize error: {e}"))
}

// ── Temporal Engine commands ─────────────────────────────────────────────

pub(crate) fn temporal_fork(
    state: &AppState,
    request: String,
    agent_id: String,
    fork_count: Option<u32>,
) -> Result<String, String> {
    // Refused before the configuration is read, a provider is built or the
    // model is called.
    if let Some(forks) = fork_count {
        nexus_kernel::temporal::types::check_fork_count(forks).map_err(|e| e.to_string())?;
    }
    // P3-HM-CONTRACT-02 §11.3, §12.1 (HB): the request's unit and the
    // text-free projection of the caller's drives.
    let (mut unit, drive) = temporal_unit(state, &agent_id)?;

    let mut caps = HashSet::new();
    caps.insert("llm.query".to_string());
    // Gateway accounting only; never a principal.
    let mut ctx = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: caps,
        fuel_remaining: 100_000,
    };

    // Build manifest for this agent
    let manifest = nexus_kernel::manifest::AgentManifest {
        name: agent_id.clone(),
        version: "1.0.0".into(),
        capabilities: vec!["llm.query".into()],
        fuel_budget: 10_000,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    };

    // Optionally override fork count. The engine's limits are checked before
    // they are stored and before the first model call.
    let mut engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let mut cfg = engine.config().clone();
    if let Some(fc) = fork_count {
        cfg.max_parallel_forks = fc;
    }
    cfg.validate().map_err(|e| e.to_string())?;
    if fork_count.is_some() {
        engine.update_config(cfg);
    }

    let llm_query = |prompt: &str| -> Result<(String, u32), nexus_kernel::temporal::TemporalError> {
        human_request(state, &mut unit, &mut ctx, prompt, 2048)
            .map_err(nexus_kernel::temporal::TemporalError::LlmError)
    };

    let decision = engine
        .fork_and_evaluate(&request, &manifest, &drive, llm_query)
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&decision).map_err(|e| format!("serialize error: {e}"))
}

/// The unit of a temporal fork (P3-HM-CONTRACT-02 §11.3, §12.1 class HB)
/// and the projection of its caller's drives. The caller's id is resolved
/// through the agent registry: a registered agent is the principal (its
/// §4.2 policy applies; local-only → an attested local transport only), and
/// only its drives (urgency, confidence) are read; any other id is no
/// principal: an `OwnerDirect` request with the default projection.
fn temporal_unit(
    state: &AppState,
    caller: &str,
) -> Result<
    (
        nexus_kernel::hm::DispatchContext,
        nexus_kernel::temporal::DriveProjection,
    ),
    String,
> {
    use nexus_kernel::hm::{Principal, Unit, Unresolved};
    let issuer = state
        .hm_issuer()
        .map_err(|e| format!("no dispatch issuer: {e}"))?;
    if let Ok(agent) = Uuid::parse_str(caller) {
        let turn = Unit::ChatTurn {
            turn_id: Uuid::new_v4(),
        };
        match issuer.begin_unit(Principal::Agent(agent), turn) {
            Ok(unit) => {
                let drive = {
                    let mut engine = state
                        .consciousness
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    nexus_kernel::temporal::DriveProjection::of(
                        &engine.get_agent_state_snapshot(caller),
                    )
                };
                return Ok((unit, drive));
            }
            Err(Unresolved::UnregisteredAgent) => {}
            Err(e) => {
                return Err(format!(
                    "{}: {e}",
                    crate::commands::cognitive::unresolved_policy_error()
                ))
            }
        }
    }
    Ok((
        crate::hm_dispatch::request_unit(state, Principal::OwnerDirect)?,
        nexus_kernel::temporal::DriveProjection::default(),
    ))
}

/// One model call of a human request (HB, HD): `prompt` (the caller's
/// request and constant text) dispatched through the issuer under `unit`,
/// on the default model (a local-only unit only on the verified loopback
/// runtime), sent through the governed gateway. Returns the accepted reply
/// and its tokens.
fn human_request(
    state: &AppState,
    unit: &mut nexus_kernel::hm::DispatchContext,
    gateway_ctx: &mut AgentRuntimeContext,
    prompt: &str,
    max_tokens: u32,
) -> Result<(String, u32), String> {
    let tokens = std::cell::Cell::new(0u32);
    let reply = crate::hm_dispatch::dispatch_text(
        state,
        unit,
        prompt,
        &mut |c| {
            let (provider, model, attestation) = crate::hm_dispatch::human_provider(c, None)?;
            Ok(((provider, model), attestation))
        },
        &mut |(provider, model): (Box<dyn LlmProvider>, String), admitted: &str| {
            let mut gateway = GovernedLlmGateway::new(provider);
            gateway.set_skip_output_firewall(true);
            let response = gateway
                .query(gateway_ctx, admitted, max_tokens, &model)
                .map_err(|e| e.to_string())?;
            tokens.set(response.token_count);
            Ok(response.output_text)
        },
    )?;
    Ok((reply.text().to_string(), tokens.get()))
}

pub(crate) fn temporal_select_fork(
    state: &AppState,
    decision_id: String,
    fork_id: String,
) -> Result<(), String> {
    let mut engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine
        .manual_select_fork(&decision_id, &fork_id)
        .map_err(|e| format!("{e}"))
}

pub(crate) fn temporal_rollback(state: &AppState, decision_id: String) -> Result<String, String> {
    let engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let decision = engine
        .get_decision(&decision_id)
        .ok_or_else(|| format!("decision not found: {decision_id}"))?;

    // Find the checkpoint for this decision's selected fork
    let fork_id = decision
        .selected_fork
        .clone()
        .ok_or("no fork selected to rollback")?;

    let cp_mgr = state
        .temporal_checkpoints
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match cp_mgr.get_by_fork(&fork_id) {
        Some(cp) => serde_json::to_string(cp).map_err(|e| format!("serialize error: {e}")),
        None => Ok(json!({"status": "no checkpoint found", "fork_id": fork_id}).to_string()),
    }
}

pub(crate) fn run_dilated_session(
    state: &AppState,
    task: String,
    agent_ids: Vec<String>,
    max_iterations: u32,
) -> Result<String, String> {
    // Refused before the configuration is read or a provider is built; the
    // kernel applies the same bound again before either closure runs.
    let max_iterations = nexus_kernel::temporal::dilation::check_iteration_count(max_iterations)
        .map_err(|e| e.to_string())?;
    // P3-HM-CONTRACT-02 §12.1 (HD): the Owner's request; the caller's agent
    // ids are not principals.
    let mut create_unit =
        crate::hm_dispatch::request_unit(state, nexus_kernel::hm::Principal::OwnerDirect)?;
    let mut critique_unit =
        crate::hm_dispatch::request_unit(state, nexus_kernel::hm::Principal::OwnerDirect)?;

    let mut caps = HashSet::new();
    caps.insert("llm.query".to_string());
    let mut ctx = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: caps,
        fuel_remaining: 100_000,
    };

    let create_fn = |task: &str,
                     prev: &str,
                     feedback: &str|
     -> Result<String, nexus_kernel::temporal::TemporalError> {
        let prompt = if prev.is_empty() {
            format!("Create an artifact for: {task}\nReturn the content directly.")
        } else {
            format!(
                "Improve this artifact for: {task}\n\
                     Previous version:\n{prev}\n\
                     Feedback: {feedback}\n\
                     Return the improved content directly."
            )
        };
        human_request(state, &mut create_unit, &mut ctx, &prompt, 4096)
            .map(|(text, _)| text)
            .map_err(nexus_kernel::temporal::TemporalError::LlmError)
    };

    let mut caps2 = HashSet::new();
    caps2.insert("llm.query".to_string());
    let mut ctx2 = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: caps2,
        fuel_remaining: 100_000,
    };

    let critique_fn = |task: &str,
                       content: &str|
     -> Result<(f64, String), nexus_kernel::temporal::TemporalError> {
        let prompt = format!(
            "Score this artifact for task: {task}\n\
                 Artifact:\n{content}\n\n\
                 Return JSON: {{\"score\": N, \"feedback\": \"...\"}}\n\
                 Score 0-10. Return ONLY the JSON."
        );
        let (output_text, _) = human_request(state, &mut critique_unit, &mut ctx2, &prompt, 1024)
            .map_err(nexus_kernel::temporal::TemporalError::LlmError)?;
        let val: serde_json::Value = serde_json::from_str(output_text.trim())
            .unwrap_or_else(|_| json!({"score": 5.0, "feedback": output_text}));
        let score = val["score"].as_f64().unwrap_or(5.0);
        let feedback = val["feedback"].as_str().unwrap_or("").to_string();
        Ok((score, feedback))
    };

    // A snapshot of the dilator's settings: no lock is held across the
    // provider calls the session makes.
    let dilator = state
        .time_dilator
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let session = dilator
        .run_dilated_session(
            &task,
            agent_ids,
            Some(max_iterations),
            None,
            create_fn,
            critique_fn,
        )
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&session).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_temporal_history(state: &AppState, limit: u32) -> Result<String, String> {
    let engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.history();
    let recent: Vec<_> = history.iter().rev().take(limit as usize).cloned().collect();
    serde_json::to_string(&recent).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn set_temporal_config(
    state: &AppState,
    max_forks: u32,
    eval_strategy: String,
    budget_tokens: u64,
) -> Result<(), String> {
    let strategy = match eval_strategy.as_str() {
        "BestFinalScore" => nexus_kernel::temporal::EvalStrategy::BestFinalScore,
        "BestAverageScore" => nexus_kernel::temporal::EvalStrategy::BestAverageScore,
        "LowestRisk" => nexus_kernel::temporal::EvalStrategy::LowestRisk,
        "UserChoice" => nexus_kernel::temporal::EvalStrategy::UserChoice,
        _ => return Err(format!("unknown eval strategy: {eval_strategy}")),
    };
    // Refused before the engine is locked; the stored limits stay unchanged.
    nexus_kernel::temporal::types::check_fork_count(max_forks).map_err(|e| e.to_string())?;
    nexus_kernel::temporal::types::check_fork_budget(budget_tokens).map_err(|e| e.to_string())?;

    let mut engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let cfg = nexus_kernel::temporal::TemporalConfig {
        max_parallel_forks: max_forks,
        max_depth_per_fork: engine.config().max_depth_per_fork,
        fork_budget_tokens: budget_tokens,
        evaluation_strategy: strategy,
    };
    engine.update_config(cfg);
    Ok(())
}
