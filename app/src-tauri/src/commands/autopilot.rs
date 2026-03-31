//! autopilot domain implementation.

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

// ── Screenshot Clone ──

pub(crate) fn screenshot_analyze(state: &AppState, image_path: String) -> Result<String, String> {
    let cloner = state
        .screenshot_cloner
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let (system, user) = cloner.build_analysis_prompt(&image_path);
    serde_json::to_string(&serde_json::json!({
        "system_prompt": system,
        "user_prompt": user,
        "min_visual_match": cloner.min_visual_match,
    }))
    .map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn screenshot_generate_spec(
    state: &AppState,
    analysis_json: String,
    project_name: String,
) -> Result<String, String> {
    let cloner = state
        .screenshot_cloner
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let analysis: nexus_kernel::autopilot::screenshot_clone::ScreenshotAnalysis =
        serde_json::from_str(&analysis_json).map_err(|e| format!("parse error: {e}"))?;
    let spec = cloner.generate_project_spec(&analysis, &project_name);
    serde_json::to_string(&spec).map_err(|e| format!("serialize error: {e}"))
}

// ── Voice Project ──

pub(crate) fn voice_project_start(state: &AppState) -> Result<(), String> {
    let mut builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    builder.start_listening();
    Ok(())
}

pub(crate) fn voice_project_stop(state: &AppState) -> Result<String, String> {
    let mut builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let intent = builder.stop_listening();
    serde_json::to_string(&intent).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn voice_project_add_chunk(
    state: &AppState,
    text: String,
    timestamp: u64,
) -> Result<(), String> {
    let mut builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    builder.add_chunk(text, timestamp);
    Ok(())
}

pub(crate) fn voice_project_get_status(state: &AppState) -> Result<String, String> {
    let builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let summary = builder.get_status_summary();
    serde_json::to_string(&summary).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn voice_project_get_prompt(state: &AppState) -> Result<String, String> {
    let builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let (system, user) = builder.build_intent_prompt();
    serde_json::to_string(&serde_json::json!({
        "system_prompt": system,
        "user_prompt": user,
    }))
    .map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn voice_project_update_intent(
    state: &AppState,
    response: String,
    timestamp: u64,
) -> Result<String, String> {
    let mut builder = state
        .voice_project
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let triggered = builder
        .update_intent(&response, timestamp)
        .map_err(|e| format!("intent error: {e}"))?;
    serde_json::to_string(&serde_json::json!({
        "autopilot_triggered": triggered,
        "confidence": builder.intent.confidence,
    }))
    .map_err(|e| format!("serialize error: {e}"))
}

// ── Stress Test ──

pub(crate) fn stress_generate_personas(state: &AppState, count: u32) -> Result<String, String> {
    let sim = state
        .stress_simulator
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let personas = sim.generate_default_personas(count);
    serde_json::to_string(&personas).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn stress_generate_actions(
    state: &AppState,
    persona_json: String,
) -> Result<String, String> {
    let sim = state
        .stress_simulator
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let persona: nexus_kernel::autopilot::stress_test::UserPersona =
        serde_json::from_str(&persona_json).map_err(|e| format!("parse error: {e}"))?;
    let actions = sim.generate_actions_for_persona(&persona);
    serde_json::to_string(&actions).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn stress_evaluate_report(
    state: &AppState,
    report_json: String,
) -> Result<String, String> {
    let sim = state
        .stress_simulator
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let report: nexus_kernel::autopilot::stress_test::StressReport =
        serde_json::from_str(&report_json).map_err(|e| format!("parse error: {e}"))?;
    let passed = sim.evaluate_report(&report);
    serde_json::to_string(&serde_json::json!({ "passed": passed }))
        .map_err(|e| format!("serialize error: {e}"))
}

// ── Deploy ──

pub(crate) fn deploy_generate_dockerfile(
    state: &AppState,
    config_json: String,
) -> Result<String, String> {
    let deployer = state
        .live_deployer
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let config: nexus_kernel::autopilot::deploy::DeployConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("parse error: {e}"))?;
    let docker = deployer.generate_dockerfile(&config);
    serde_json::to_string(&docker).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn deploy_validate_config(
    state: &AppState,
    config_json: String,
) -> Result<String, String> {
    let deployer = state
        .live_deployer
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let config: nexus_kernel::autopilot::deploy::DeployConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("parse error: {e}"))?;
    match deployer.validate_config(&config) {
        Ok(()) => Ok("{\"valid\":true}".into()),
        Err(errors) => serde_json::to_string(&serde_json::json!({
            "valid": false,
            "errors": errors,
        }))
        .map_err(|e| format!("serialize error: {e}")),
    }
}

pub(crate) fn deploy_get_commands(state: &AppState, config_json: String) -> Result<String, String> {
    let deployer = state
        .live_deployer
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let config: nexus_kernel::autopilot::deploy::DeployConfig =
        serde_json::from_str(&config_json).map_err(|e| format!("parse error: {e}"))?;
    let commands = deployer
        .deploy_command(&config)
        .map_err(|e| format!("deploy error: {e}"))?;
    serde_json::to_string(&commands).map_err(|e| format!("serialize error: {e}"))
}

// ── Live Evolution ──

pub(crate) fn evolver_register_app(state: &AppState, app_json: String) -> Result<(), String> {
    let mut evolver = state.live_evolver.lock().unwrap_or_else(|p| p.into_inner());
    let app: nexus_kernel::autopilot::live_evolution::DeployedApp =
        serde_json::from_str(&app_json).map_err(|e| format!("parse error: {e}"))?;
    evolver.register_app(app);
    Ok(())
}

pub(crate) fn evolver_unregister_app(state: &AppState, project_id: String) -> Result<bool, String> {
    let mut evolver = state.live_evolver.lock().unwrap_or_else(|p| p.into_inner());
    Ok(evolver.unregister_app(&project_id))
}

pub(crate) fn evolver_list_apps(state: &AppState) -> Result<String, String> {
    let evolver = state.live_evolver.lock().unwrap_or_else(|p| p.into_inner());
    let apps = evolver.list_apps();
    serde_json::to_string(&apps).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn evolver_detect_issues(
    state: &AppState,
    metrics_json: String,
) -> Result<String, String> {
    let evolver = state.live_evolver.lock().unwrap_or_else(|p| p.into_inner());
    let metrics: nexus_kernel::autopilot::live_evolution::AppMetrics =
        serde_json::from_str(&metrics_json).map_err(|e| format!("parse error: {e}"))?;
    let config = nexus_kernel::autopilot::live_evolution::MonitoringConfig::default();
    let issues = evolver.detect_issues(&metrics, &config);
    serde_json::to_string(&issues).map_err(|e| format!("serialize error: {e}"))
}

// ── Freelance Engine ──

pub(crate) fn freelance_get_status(state: &AppState) -> Result<String, String> {
    let engine = state
        .freelance_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let status = engine.get_status();
    serde_json::to_string(&status).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn freelance_start_scanning(state: &AppState) -> Result<(), String> {
    let mut engine = state
        .freelance_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.start_scanning();
    Ok(())
}

pub(crate) fn freelance_stop_scanning(state: &AppState) -> Result<(), String> {
    let mut engine = state
        .freelance_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.stop_scanning();
    Ok(())
}

pub(crate) fn freelance_evaluate_job(state: &AppState, job_json: String) -> Result<String, String> {
    let engine = state
        .freelance_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let job: nexus_kernel::economy::freelancer::JobOpportunity =
        serde_json::from_str(&job_json).map_err(|e| format!("parse error: {e}"))?;
    let eval = engine.evaluate_opportunity(&job);
    serde_json::to_string(&eval).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn freelance_get_revenue(state: &AppState) -> Result<String, String> {
    let engine = state
        .freelance_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(&engine.revenue).map_err(|e| format!("serialize error: {e}"))
}

// ---------------------------------------------------------------------------
// Experience Layer — conversational builder, remix, preview, teach, etc.
// ---------------------------------------------------------------------------

pub(crate) fn start_conversational_build(
    state: &AppState,
    message: String,
) -> Result<String, String> {
    let mut builder = state
        .conversational_builder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    *builder = ConversationalBuilder::new();
    // First message: LLM not yet called, produce a stub response to kick off.
    let resp = builder.process_message(
        &message,
        "Great idea! A few quick questions:\n1. Who is your target audience?\n2. What's your budget?\n  • $0 (free tools only)\n  • Under $50/month\n  • Under $200/month",
    );
    serde_json::to_string(&resp).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn builder_respond(state: &AppState, message: String) -> Result<String, String> {
    let mut builder = state
        .conversational_builder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let resp = builder.process_message(
        &message, &message, // In production, this would be the LLM response
    );
    serde_json::to_string(&resp).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn get_live_preview(state: &AppState, project_id: String) -> Result<String, String> {
    let previews = state
        .live_previews
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if let Some(engine) = previews.get(&project_id) {
        if let Some(frame) = engine.latest() {
            return serde_json::to_string(frame).map_err(|e| format!("serialize error: {e}"));
        }
    }
    Ok("null".to_string())
}

pub(crate) fn remix_project(
    state: &AppState,
    _project_id: String,
    change: String,
) -> Result<String, String> {
    let engine = state.remix_engine.lock().unwrap_or_else(|p| p.into_inner());
    let result = engine.apply_remix(&change, "", &change);
    serde_json::to_string(&result).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn analyze_problem(state: &AppState, problem: String) -> Result<String, String> {
    let solver = state
        .problem_solver
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let analysis = solver.analyze(
        &problem,
        &serde_json::json!({
            "problem_summary": problem,
            "root_causes": ["Manual process identified"],
            "current_cost": "time-consuming",
            "solution_title": "Custom Automation",
            "solution_features": ["Automated workflow", "Smart notifications", "Analytics dashboard"],
            "build_time_minutes": 20,
            "monthly_cost": "$0",
            "expected_savings": "significant time savings",
            "buildable": true
        })
        .to_string(),
    );
    serde_json::to_string(&analysis).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn publish_to_marketplace(
    state: &AppState,
    project_id: String,
    pricing: String,
) -> Result<String, String> {
    let mut publisher = state
        .marketplace_publisher
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let pricing_enum = match pricing.to_lowercase().as_str() {
        "free" => nexus_kernel::experience::Pricing::Free,
        _ => {
            if let Ok(cents) = pricing.parse::<u64>() {
                nexus_kernel::experience::Pricing::OneTime(cents)
            } else {
                nexus_kernel::experience::Pricing::Free
            }
        }
    };
    let listing = publisher.publish(
        &project_id,
        "My Project",
        "Published from Nexus OS",
        pricing_enum,
        vec![],
    );
    serde_json::to_string(&listing).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn install_from_marketplace(
    state: &AppState,
    listing_id: String,
) -> Result<String, String> {
    let mut publisher = state
        .marketplace_publisher
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match publisher.install(&listing_id) {
        Some(listing) => {
            serde_json::to_string(&listing).map_err(|e| format!("serialize error: {e}"))
        }
        None => Err("Listing not found".to_string()),
    }
}

pub(crate) fn start_teach_mode(state: &AppState, project_id: String) -> Result<String, String> {
    let mut modes = state.teach_modes.lock().unwrap_or_else(|p| p.into_inner());
    let mut tm = TeachMode::new(&project_id, 10);
    let step = tm.next_step(
        "Welcome",
        "Let's build this together! I'll explain each step so you understand what's happening.",
        Some("Think of building an app like building a house — we start with the foundation."),
        None,
        vec!["Project structure".into()],
    );
    modes.insert(project_id, tm);
    serde_json::to_string(&step).map_err(|e| format!("serialize error: {e}"))
}

pub(crate) fn teach_mode_respond(
    state: &AppState,
    project_id: String,
    response: String,
) -> Result<String, String> {
    let mut modes = state.teach_modes.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(tm) = modes.get_mut(&project_id) {
        let action = tm.respond(&response);
        let step = match action {
            nexus_kernel::experience::teach_mode::TeachModeAction::Skip => {
                tm.skip_step("Next step", None)
            }
            nexus_kernel::experience::teach_mode::TeachModeAction::ExplainMore => {
                nexus_kernel::experience::TeachStep {
                    step_number: tm.current_step,
                    total_steps: tm.total_steps,
                    title: "More detail".into(),
                    explanation: "Let me break this down further...".into(),
                    analogy: None,
                    implementation_preview: None,
                    concepts: vec![],
                    skipped: false,
                }
            }
            nexus_kernel::experience::teach_mode::TeachModeAction::Next => tm.next_step(
                "Next step",
                "Moving on to the next part of your project.",
                None,
                None,
                vec!["Building blocks".into()],
            ),
        };
        serde_json::to_string(&step).map_err(|e| format!("serialize error: {e}"))
    } else {
        Err("No teach mode session found for this project".to_string())
    }
}
