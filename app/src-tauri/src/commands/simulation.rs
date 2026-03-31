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

// ── Genesis Protocol (agent-writes-agent) ──────────────────────────────────

pub(crate) fn genesis_analyze_gap(
    _state: &AppState,
    user_request: String,
) -> Result<String, String> {
    use nexus_kernel::genesis::GenesisEngine;

    let agents = load_existing_agent_summaries();
    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);
    let prompt = engine.analyze_gap(&user_request, &agents);

    Ok(json!({
        "prompt": prompt,
        "agent_count": agents.len(),
    })
    .to_string())
}

pub(crate) fn genesis_preview_agent(
    _state: &AppState,
    user_request: String,
    llm_response: String,
) -> Result<String, String> {
    use nexus_kernel::genesis::GenesisEngine;

    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);
    let analysis = engine.parse_gap_analysis(&user_request, &llm_response)?;

    serde_json::to_string(&analysis).map_err(|e| format!("Serialize error: {e}"))
}

pub(crate) fn genesis_create_agent(
    _state: &AppState,
    spec_json: String,
    system_prompt: String,
) -> Result<String, String> {
    use nexus_kernel::genesis::generator::AgentSpec;
    use nexus_kernel::genesis::GenesisEngine;

    let mut spec: AgentSpec =
        serde_json::from_str(&spec_json).map_err(|e| format!("Invalid spec: {e}"))?;

    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);

    let manifest = engine.finalize_manifest(&mut spec, &system_prompt);
    let result = engine.deploy(&spec, &manifest)?;

    serde_json::to_string(&result).map_err(|e| format!("Serialize error: {e}"))
}

pub(crate) fn genesis_store_pattern(
    _state: &AppState,
    spec_json: String,
    missing_capabilities: Vec<String>,
    test_score: f64,
) -> Result<String, String> {
    use nexus_kernel::genesis::generator::AgentSpec;
    use nexus_kernel::genesis::GenesisEngine;

    let spec: AgentSpec =
        serde_json::from_str(&spec_json).map_err(|e| format!("Invalid spec: {e}"))?;

    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);
    engine.store_creation_pattern(&spec, &missing_capabilities, test_score)?;

    Ok(json!({"status": "stored"}).to_string())
}

pub(crate) fn genesis_list_generated(_state: &AppState) -> Result<String, String> {
    use nexus_kernel::genesis::GenesisEngine;

    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);
    let manifests = engine.list_generated()?;

    serde_json::to_string(&manifests).map_err(|e| format!("Serialize error: {e}"))
}

pub(crate) fn genesis_delete_agent(
    _state: &AppState,
    agent_name: String,
) -> Result<String, String> {
    use nexus_kernel::genesis::GenesisEngine;

    let base = project_base_dir();
    let engine = GenesisEngine::new(&base);
    engine.delete_generated(&agent_name)?;

    Ok(json!({"status": "deleted", "agent": agent_name}).to_string())
}

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
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    let llm = GatewayDreamLlm;
    let briefing = engine.generate_morning_briefing(&llm);
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
    let mut engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    let llm = GatewayDreamLlm;
    let results = engine.enter_dream_state(&llm);
    serde_json::to_string(&results).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_dream_history(state: &AppState, limit: u32) -> Result<String, String> {
    let engine = state.dream_engine.lock().unwrap_or_else(|p| p.into_inner());
    let recent = engine.scheduler.recent_dreams(limit);
    serde_json::to_string(&recent).map_err(|e| format!("serialize error: {e}"))
}

/// LLM adapter that uses the governed gateway for dream state queries.
pub(crate) struct GatewayDreamLlm;

impl nexus_kernel::dreams::engine::DreamLlm for GatewayDreamLlm {
    fn query(&self, system: &str, user: &str, max_tokens: u32) -> Result<(String, u64), String> {
        let config = load_config().map_err(agent_error)?;
        let provider_config = build_provider_config(&config);
        let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
        let model = if config.llm.default_model.trim().is_empty() {
            "mock-1".to_string()
        } else {
            config.llm.default_model.clone()
        };

        let mut gateway = GovernedLlmGateway::new(provider);
        gateway.set_skip_output_firewall(true);

        let mut caps = HashSet::new();
        caps.insert("llm.query".to_string());
        let mut ctx = AgentRuntimeContext {
            agent_id: Uuid::new_v4(),
            capabilities: caps,
            fuel_remaining: 50_000,
        };

        let prompt = format!("[System: {system}]\n\n{user}");
        let response = gateway
            .query(&mut ctx, &prompt, max_tokens, &model)
            .map_err(agent_error)?;
        Ok((response.output_text, response.token_count as u64))
    }
}

// ── Temporal Engine commands ─────────────────────────────────────────────

pub(crate) fn temporal_fork(
    state: &AppState,
    request: String,
    agent_id: String,
    fork_count: Option<u32>,
) -> Result<String, String> {
    let config = load_config().map_err(agent_error)?;
    let provider_config = build_provider_config(&config);
    let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
    let model = if config.llm.default_model.trim().is_empty() {
        "mock-1".to_string()
    } else {
        config.llm.default_model.clone()
    };

    let mut gateway = GovernedLlmGateway::new(provider);
    gateway.set_skip_output_firewall(true);

    let mut caps = HashSet::new();
    caps.insert("llm.query".to_string());
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

    // Get consciousness state
    let mut cons_engine = state
        .consciousness
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let consciousness = cons_engine.get_agent_state_snapshot(&agent_id);
    drop(cons_engine);

    // Optionally override fork count
    let mut engine = state
        .temporal_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(fc) = fork_count {
        let mut cfg = engine.config().clone();
        cfg.max_parallel_forks = fc;
        engine.update_config(cfg);
    }

    let model_clone = model.clone();
    let llm_query = |prompt: &str| -> Result<(String, u32), nexus_kernel::temporal::TemporalError> {
        let response = gateway
            .query(&mut ctx, prompt, 2048, &model_clone)
            .map_err(|e| nexus_kernel::temporal::TemporalError::LlmError(format!("{e}")))?;
        Ok((response.output_text, response.token_count))
    };

    let decision = engine
        .fork_and_evaluate(&request, &manifest, &consciousness, llm_query)
        .map_err(|e| format!("{e}"))?;

    serde_json::to_string(&decision).map_err(|e| format!("serialize error: {e}"))
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
    let config = load_config().map_err(agent_error)?;
    let provider_config = build_provider_config(&config);
    let provider = select_provider(&provider_config).map_err(|e| e.to_string())?;
    let model = if config.llm.default_model.trim().is_empty() {
        "mock-1".to_string()
    } else {
        config.llm.default_model.clone()
    };

    let mut gateway = GovernedLlmGateway::new(provider);
    gateway.set_skip_output_firewall(true);

    let mut caps = HashSet::new();
    caps.insert("llm.query".to_string());
    let mut ctx = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities: caps,
        fuel_remaining: 100_000,
    };

    let model_c = model.clone();
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
        let resp = gateway
            .query(&mut ctx, &prompt, 4096, &model_c)
            .map_err(|e| nexus_kernel::temporal::TemporalError::LlmError(format!("{e}")))?;
        Ok(resp.output_text)
    };

    let model_c2 = model.clone();
    let provider2 = select_provider(&provider_config).map_err(|e| e.to_string())?;
    let mut gateway2 = GovernedLlmGateway::new(provider2);
    gateway2.set_skip_output_firewall(true);
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
        let resp = gateway2
            .query(&mut ctx2, &prompt, 1024, &model_c2)
            .map_err(|e| nexus_kernel::temporal::TemporalError::LlmError(format!("{e}")))?;
        let val: serde_json::Value = serde_json::from_str(resp.output_text.trim())
            .unwrap_or_else(|_| json!({"score": 5.0, "feedback": resp.output_text}));
        let score = val["score"].as_f64().unwrap_or(5.0);
        let feedback = val["feedback"].as_str().unwrap_or("").to_string();
        Ok((score, feedback))
    };

    let dilator = state.time_dilator.lock().unwrap_or_else(|p| p.into_inner());
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

pub(crate) fn load_existing_agent_summaries(
) -> Vec<nexus_kernel::genesis::gap_analysis::ExistingAgentSummary> {
    use nexus_kernel::genesis::gap_analysis::ExistingAgentSummary;

    let mut summaries = Vec::new();
    for path in list_prebuilt_manifest_paths() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&raw) {
                summaries.push(ExistingAgentSummary {
                    name: manifest["name"].as_str().unwrap_or("unknown").to_string(),
                    description: manifest["description"].as_str().unwrap_or("").to_string(),
                    capabilities: manifest["capabilities"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    autonomy_level: manifest["autonomy_level"].as_u64().unwrap_or(0) as u32,
                });
            }
        }
    }
    summaries
}

pub(crate) fn project_base_dir() -> std::path::PathBuf {
    // Try CARGO_MANIFEST_DIR first (dev), then walk from current dir
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = std::path::PathBuf::from(manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        if let Some(b) = base {
            if b.join("agents").exists() {
                return b;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("agents").exists() {
            return cwd;
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("agents").exists() {
                return parent.to_path_buf();
            }
        }
    }
    std::path::PathBuf::from(".")
}

/// Simple prompt breeding without LLM — merges the two prompts structurally.
/// When an LLM is available, this can be upgraded to use LLM-based merging.
pub(crate) fn breed_system_prompts_via_llm(prompt_a: &str, prompt_b: &str) -> String {
    // Extract the first sentence (identity) from each parent
    let identity_a = prompt_a
        .split('.')
        .next()
        .unwrap_or("You are a versatile agent");
    // Combine: take identity from parent A, add skills from both
    format!(
        "{identity_a}, with hybrid capabilities. \
         You combine the strengths of two parent agents. \
         From your first parent: {} \
         From your second parent: {}",
        truncate_prompt(prompt_a, 500),
        truncate_prompt(prompt_b, 500),
    )
}

pub(crate) fn truncate_prompt(prompt: &str, max_chars: usize) -> &str {
    if prompt.len() <= max_chars {
        prompt
    } else {
        let end = prompt
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(prompt.len());
        &prompt[..end]
    }
}

pub(crate) fn evolve_population(
    _state: &AppState,
    agent_ids: Vec<String>,
    _task: String,
    generations: u32,
) -> Result<String, String> {
    use nexus_kernel::genome::tournament_select;

    if agent_ids.len() < 2 {
        return Err("Need at least 2 agents for evolution".to_string());
    }

    // Load initial population
    let mut population: Vec<AgentGenome> = agent_ids
        .iter()
        .map(|id| load_genome(id))
        .collect::<Result<Vec<_>, _>>()?;

    let mut generation_reports: Vec<serde_json::Value> = Vec::new();

    for gen in 0..generations {
        // Assign synthetic fitness scores based on genome diversity and balance
        for genome in &mut population {
            let domain_count = genome.genes.capabilities.domains.len() as f64;
            let avg_weight: f64 = if genome.genes.capabilities.domain_weights.is_empty() {
                0.5
            } else {
                genome
                    .genes
                    .capabilities
                    .domain_weights
                    .values()
                    .sum::<f64>()
                    / genome.genes.capabilities.domain_weights.len() as f64
            };
            let fitness = (domain_count * 0.3 + avg_weight * 0.7).min(1.0);
            genome.record_fitness(fitness);
        }

        // Selection: keep top 50%
        let survivors = tournament_select(&population);

        // Breed survivors to replenish population
        let mut next_gen = survivors.clone();
        let mut breed_idx = 0;
        while next_gen.len() < population.len() {
            let a = &survivors[breed_idx % survivors.len()];
            let b = &survivors[(breed_idx + 1) % survivors.len()];
            if a.agent_id != b.agent_id {
                let offspring = crossover(a, b);
                next_gen.push(offspring);
            }
            breed_idx += 1;
            if breed_idx > population.len() * 2 {
                break; // safety valve
            }
        }

        // Mutate all non-survivor offspring
        for item in next_gen.iter_mut().skip(survivors.len()) {
            *item = mutate(item);
        }

        let gen_summary = json!({
            "generation": gen,
            "population_size": next_gen.len(),
            "avg_fitness": next_gen.iter().map(|g| g.average_fitness()).sum::<f64>() / next_gen.len() as f64,
            "best_agent": next_gen.iter().max_by(|a, b| a.average_fitness().partial_cmp(&b.average_fitness()).unwrap_or(std::cmp::Ordering::Equal)).map(|g| g.agent_id.clone()).unwrap_or_default(),
        });
        generation_reports.push(gen_summary);

        population = next_gen;
    }

    // Save all final-generation genomes
    for genome in &population {
        // Best-effort: persist evolved genome; simulation results are returned regardless
        let _ = save_genome(genome);
    }

    let report = json!({
        "generations_run": generations,
        "final_population_size": population.len(),
        "final_agents": population.iter().map(|g| json!({
            "agent_id": g.agent_id,
            "generation": g.generation,
            "fitness": g.average_fitness(),
            "domains": g.genes.capabilities.domains,
        })).collect::<Vec<_>>(),
        "generation_reports": generation_reports,
    });

    Ok(report.to_string())
}
