//! trust_security domain implementation.

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

// ── Reputation Registry ─────────────────────────────────────────────

pub(crate) fn reputation_register(
    state: &AppState,
    did: String,
    name: String,
) -> Result<String, String> {
    let mut reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let rep = reg.register_agent(&did, &name);

    drop(reg);
    state.log_event(
        uuid::Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "reputation",
            "action": "register",
            "agent_did": did,
        }),
    );

    serde_json::to_string(&rep).map_err(|e| e.to_string())
}

pub(crate) fn reputation_record_task(
    state: &AppState,
    did: String,
    success: bool,
) -> Result<String, String> {
    let mut reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    reg.record_task_completion(&did, success);
    reg.award_badges(&did);
    let rep = reg
        .get_reputation(&did)
        .ok_or_else(|| format!("agent '{did}' not found"))?;
    serde_json::to_string(rep).map_err(|e| e.to_string())
}

pub(crate) fn reputation_rate_agent(
    state: &AppState,
    did: String,
    rater_did: String,
    score: f64,
    comment: Option<String>,
) -> Result<String, String> {
    let mut reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    reg.add_peer_rating(&did, &rater_did, score, comment);
    reg.award_badges(&did);
    let rep = reg
        .get_reputation(&did)
        .ok_or_else(|| format!("agent '{did}' not found"))?;
    serde_json::to_string(rep).map_err(|e| e.to_string())
}

pub(crate) fn reputation_get(state: &AppState, did: String) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let rep = reg
        .get_reputation(&did)
        .ok_or_else(|| format!("agent '{did}' not found"))?;
    serde_json::to_string(rep).map_err(|e| e.to_string())
}

pub(crate) fn reputation_top(state: &AppState, limit: Option<usize>) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let top = reg.top_agents(limit.unwrap_or(10));
    serde_json::to_string(&top).map_err(|e| e.to_string())
}

pub(crate) fn reputation_export(state: &AppState, did: String) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    reg.export_reputation(&did)
}

pub(crate) fn reputation_import(state: &AppState, json: String) -> Result<String, String> {
    let mut reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let rep = reg.import_reputation(&json)?;

    drop(reg);
    state.log_event(
        uuid::Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "reputation",
            "action": "import",
            "agent_did": rep.agent_did,
        }),
    );

    serde_json::to_string(&rep).map_err(|e| e.to_string())
}

// ── Trust Overview ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustOverviewAgent {
    pub id: String,
    pub name: String,
    pub did: Option<String>,
    pub autonomy_level: u8,
    pub trust_score: f64,
    pub total_tasks: u64,
    pub success_rate: f64,
    pub violations: u64,
    pub fuel_remaining: u64,
    pub fuel_budget: u64,
    pub status: String,
    pub badges: Vec<String>,
    pub last_updated: u64,
}

pub(crate) fn get_trust_overview(state: &AppState) -> Result<Vec<TrustOverviewAgent>, String> {
    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let meta_guard = state.meta.lock().unwrap_or_else(|p| p.into_inner());
    let id_mgr = state.identity_mgr.lock().unwrap_or_else(|p| p.into_inner());
    let mut rep_reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    let statuses = supervisor.health_check();
    let mut result = Vec::new();

    for status in &statuses {
        let meta = meta_guard.get(&status.id).cloned().unwrap_or(AgentMeta {
            name: "unknown".to_string(),
            last_action: "none".to_string(),
        });

        let handle = supervisor.get_agent(status.id);
        let autonomy_level = handle.map(|h| h.autonomy_level).unwrap_or(0);
        let fuel_budget = handle.map(|h| h.manifest.fuel_budget).unwrap_or(0);

        // Get or create DID
        let did = id_mgr.get(&status.id).map(|id| id.did.clone());
        let did_str = did
            .clone()
            .unwrap_or_else(|| format!("did:nexus:{}", status.id));

        // Auto-register in reputation if not already present
        if rep_reg.get_reputation(&did_str).is_none() {
            rep_reg.register_agent(&did_str, &meta.name);
        }

        let (trust_score, total_tasks, success_rate, violations, badges, last_updated) =
            match rep_reg.get_reputation(&did_str) {
                Some(rep) => (
                    rep.reputation_score,
                    rep.total_tasks_completed + rep.total_tasks_failed,
                    rep.success_rate,
                    rep.governance_violations,
                    rep.badges.iter().map(|b| format!("{b:?}")).collect(),
                    rep.last_updated,
                ),
                None => (0.5, 0, 0.0, 0, Vec::new(), 0),
            };

        result.push(TrustOverviewAgent {
            id: status.id.to_string(),
            name: meta.name,
            did,
            autonomy_level,
            trust_score,
            total_tasks,
            success_rate,
            violations,
            fuel_remaining: status.remaining_fuel,
            fuel_budget,
            status: status.state.to_string(),
            badges,
            last_updated,
        });
    }

    result.sort_by(|a, b| {
        b.trust_score
            .partial_cmp(&a.trust_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(result)
}

// ── Computer Control Engine ──────────────────────────────────────────

pub(crate) fn desktop_control_workspace() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".nexus")
        .join("desktop-backend")
        .join("computer-control")
}

pub(crate) fn show_desktop_notification(message: &str) {
    // Best-effort: desktop notification is informational; failure is non-fatal
    #[cfg(target_os = "linux")]
    let _ = Command::new("notify-send")
        .arg("Nexus OS")
        .arg(message)
        .output();

    // Best-effort: desktop notification is informational; failure is non-fatal
    #[cfg(target_os = "macos")]
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "display notification {:?} with title \"Nexus OS\"",
            message
        ))
        .output();

    // Best-effort: desktop notification is informational; failure is non-fatal
    #[cfg(target_os = "windows")]
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show({:?}, 'Nexus OS')",
                message
            ),
        ])
        .output();
}

pub(crate) fn run_backend_computer_action(
    state: &AppState,
    session_id: &str,
    description: &str,
    max_steps: u32,
    cancelled: &Arc<AtomicBool>,
) -> Result<String, String> {
    let workspace = desktop_control_workspace().join(session_id);
    std::fs::create_dir_all(&workspace)
        .map_err(|e| format!("failed to create {}: {e}", workspace.display()))?;

    {
        let mut engine = state
            .computer_control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if !engine.is_enabled() {
            engine.enable();
        }
    }

    let mut actions_taken = Vec::new();
    let max_steps = max_steps.max(1);
    for step in 0..max_steps {
        if cancelled.load(Ordering::SeqCst) {
            return Ok(format!(
                "Computer action '{session_id}' cancelled after {} steps",
                actions_taken.len()
            ));
        }

        let screenshot = capture_and_store_screen(
            &workspace,
            None,
            &format!("session-{session_id}-step-{step}"),
        )?;
        let analysis =
            analyze_stored_screenshot_for_backend(&screenshot, description, actions_taken.last())?;
        if analysis.action == "done" {
            return Ok(format!(
                "Computer action complete. Final screenshot: {}. Actions: {}",
                screenshot.display(),
                if actions_taken.is_empty() {
                    "none".to_string()
                } else {
                    actions_taken.join(", ")
                }
            ));
        }

        let action = backend_decision_to_input_action(&analysis)?;
        let label = action.label();
        {
            let mut engine = state
                .computer_control
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            engine.execute(action)?;
        }
        actions_taken.push(label);
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Best-effort: screenshot capture after each action step is supplementary
        let _ = capture_and_store_screen(
            &workspace,
            None,
            &format!("session-{session_id}-step-{step}-after"),
        );
    }

    Ok(format!(
        "Computer action stopped after reaching max_steps={max_steps}. Actions: {}",
        actions_taken.join(", ")
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackendComputerDecision {
    action: String,
    x: Option<u32>,
    y: Option<u32>,
    text: Option<String>,
    key: Option<String>,
    direction: Option<String>,
    amount: Option<u32>,
}

#[cfg_attr(test, allow(unused_variables))]
pub(crate) fn analyze_stored_screenshot_for_backend(
    screenshot: &std::path::Path,
    description: &str,
    previous_action: Option<&String>,
) -> Result<BackendComputerDecision, String> {
    #[cfg(test)]
    {
        if previous_action.is_none() && description.contains("click once") {
            return Ok(BackendComputerDecision {
                action: "click".to_string(),
                x: Some(10),
                y: Some(10),
                text: None,
                key: None,
                direction: None,
                amount: None,
            });
        }
        return Ok(BackendComputerDecision {
            action: "done".to_string(),
            x: None,
            y: None,
            text: None,
            key: None,
            direction: None,
            amount: None,
        });
    }

    #[allow(unreachable_code)]
    {
        let prompt = if let Some(previous_action) = previous_action {
            format!(
                "You are controlling a computer. Previous action: {previous_action}. New screen: [screenshot]. Goal: {description}. Is the goal complete? If not, what's the next action? Respond with JSON: {{\"action\":\"click|type|key|scroll|done\",\"x\":number,\"y\":number,\"text\":string,\"key\":string,\"direction\":string,\"amount\":number}}"
            )
        } else {
            format!(
                "You are controlling a computer. Current screen: [screenshot]. Goal: {description}. What is the next mouse/keyboard action to take? Respond with JSON: {{\"action\":\"click|type|key|scroll|done\",\"x\":number,\"y\":number,\"text\":string,\"key\":string,\"direction\":string,\"amount\":number}}"
            )
        };
        let provider = OllamaProvider::from_env();
        let bytes = std::fs::read(screenshot)
            .map_err(|e| format!("failed to read screenshot {}: {e}", screenshot.display()))?;
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let raw = provider
            .query_with_image(&prompt, &image_base64, "")
            .map_err(agent_error)?;
        let candidate = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        serde_json::from_str::<BackendComputerDecision>(candidate)
            .map_err(|e| format!("failed to parse computer-use decision: {e}"))
    }
}

pub(crate) fn backend_decision_to_input_action(
    decision: &BackendComputerDecision,
) -> Result<InputAction, String> {
    match decision.action.as_str() {
        "click" => Ok(InputAction::Click {
            x: decision.x.ok_or_else(|| "decision missing x".to_string())?,
            y: decision.y.ok_or_else(|| "decision missing y".to_string())?,
            button: nexus_kernel::computer_control::MouseButton::Left,
        }),
        "type" => Ok(InputAction::Type {
            text: decision
                .text
                .clone()
                .ok_or_else(|| "decision missing text".to_string())?,
        }),
        "key" => Ok(InputAction::KeyPress {
            key: decision
                .key
                .clone()
                .ok_or_else(|| "decision missing key".to_string())?,
            modifiers: vec![],
        }),
        "scroll" => Ok(InputAction::Scroll {
            direction: decision
                .direction
                .clone()
                .unwrap_or_else(|| "down".to_string()),
            amount: decision.amount.unwrap_or(1),
        }),
        other => Err(format!("unsupported computer-use action '{other}'")),
    }
}

pub(crate) fn computer_control_capture_screen(
    state: &AppState,
    region: Option<String>,
) -> Result<String, String> {
    let engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !engine.is_enabled() {
        return Err("Computer control engine is disabled".into());
    }
    let region_parsed: Option<nexus_kernel::computer_control::ScreenRegion> = match region {
        Some(r) => Some(serde_json::from_str(&r).map_err(|e| e.to_string())?),
        None => None,
    };
    let result = engine.capture_screen(region_parsed.as_ref());
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn computer_control_execute_action(
    state: &AppState,
    action_json: String,
) -> Result<String, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !engine.is_enabled() {
        return Err("Computer control engine is disabled".into());
    }
    let action: InputAction = serde_json::from_str(&action_json).map_err(|e| e.to_string())?;
    let result = engine.execute(action);
    state.log_event(
        Uuid::nil(),
        EventType::UserAction,
        json!({
            "source": "computer-control",
            "action": "execute",
            "success": result.is_ok(),
        }),
    );
    match result {
        Ok(record) => serde_json::to_string(&record).map_err(|e| e.to_string()),
        Err(e) => Err(e),
    }
}

pub(crate) fn computer_control_get_history(state: &AppState) -> Result<String, String> {
    let engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.action_history();
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

pub(crate) fn computer_control_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if enabled {
        engine.enable();
    } else {
        engine.disable();
    }
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "computer-control",
            "action": if enabled { "enable" } else { "disable" },
        }),
    );
    Ok(json!({ "enabled": engine.is_enabled() }).to_string())
}

pub(crate) fn computer_control_status(state: &AppState) -> Result<String, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(&engine.status()).map_err(|e| e.to_string())
}

pub(crate) fn capture_screen(
    state: &AppState,
    region: Option<ScreenRegion>,
) -> Result<String, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !engine.is_enabled() {
        engine.enable();
    }
    let workspace = desktop_control_workspace();
    let path = capture_and_store_screen(&workspace, region.as_ref(), "tauri-capture-screen")?;
    state.log_event(
        Uuid::nil(),
        EventType::ToolCall,
        json!({
            "source": "computer-control",
            "action": "capture_screen",
            "path": path,
        }),
    );
    Ok(path.display().to_string())
}

pub(crate) fn analyze_screen(state: &AppState, query: String) -> Result<String, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if !engine.is_enabled() {
        engine.enable();
    }
    let workspace = desktop_control_workspace();
    let analysis = capture_and_analyze_screen(&workspace, &query, None)?;
    state.log_event(
        Uuid::nil(),
        EventType::LlmCall,
        json!({
            "source": "computer-control",
            "action": "analyze_screen",
            "path": analysis.screenshot_path,
            "model": analysis.model,
        }),
    );
    Ok(analysis.output)
}

pub(crate) fn analyze_media_file(
    state: &AppState,
    path: String,
    query: String,
) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;
    let analysis = analyze_stored_screenshot(&canonical, &query, None)?;
    state.log_event(
        Uuid::nil(),
        EventType::LlmCall,
        json!({
            "source": "media-studio",
            "action": "analyze_media_file",
            "path": canonical,
            "query": query,
        }),
    );
    Ok(analysis)
}

pub(crate) fn start_computer_action(
    state: &AppState,
    description: String,
    max_steps: u32,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .computer_action_cancellations
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(session_id.clone(), cancelled.clone());
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "computer-control",
            "action": "start_computer_action",
            "session_id": session_id,
            "description": description,
            "max_steps": max_steps,
        }),
    );

    let state_clone = state.clone();
    let session_clone = session_id.clone();
    let description_clone = description.clone();
    std::thread::spawn(move || {
        let result = run_backend_computer_action(
            &state_clone,
            &session_clone,
            &description_clone,
            max_steps,
            &cancelled,
        );
        state_clone
            .computer_action_cancellations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&session_clone);
        state_clone.log_event(
            Uuid::nil(),
            EventType::StateChange,
            json!({
                "source": "computer-control",
                "action": "computer_action_complete",
                "session_id": session_clone,
                "success": result.is_ok(),
                "result": result.ok(),
            }),
        );
    });

    Ok(session_id)
}

pub(crate) fn stop_computer_action(state: &AppState, agent_id: String) -> Result<(), String> {
    if let Some(cancelled) = state
        .computer_action_cancellations
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&agent_id)
        .cloned()
    {
        cancelled.store(true, Ordering::SeqCst);
    }
    {
        let mut engine = state
            .computer_control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        engine.disable();
    }
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "computer-control",
            "action": "stop_computer_action",
            "session_id": agent_id,
        }),
    );
    Ok(())
}

pub(crate) fn get_input_control_status(state: &AppState) -> Result<InputControlStatus, String> {
    let mut engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    Ok(engine.status())
}

// ---------------------------------------------------------------------------
// Neural Bridge commands
// ---------------------------------------------------------------------------

pub(crate) fn neural_bridge_status(state: &AppState) -> Result<String, String> {
    let bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = bridge.get_stats();
    let config = bridge.config().clone();
    Ok(json!({
        "stats": stats,
        "config": config,
    })
    .to_string())
}

pub(crate) fn neural_bridge_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    bridge.set_enabled(enabled);
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "neural-bridge",
            "action": if enabled { "enable" } else { "disable" },
        }),
    );
    Ok(json!({ "enabled": enabled }).to_string())
}

pub(crate) fn neural_bridge_ingest(
    state: &AppState,
    source_type: String,
    content: String,
    metadata: serde_json::Value,
) -> Result<String, String> {
    let source = match source_type.as_str() {
        "Screen" => ContextSource::Screen {
            app_name: metadata
                .get("app_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            window_title: metadata
                .get("window_title")
                .and_then(|v| v.as_str())
                .unwrap_or("untitled")
                .to_string(),
        },
        "Audio" => ContextSource::Audio {
            duration_secs: metadata
                .get("duration_secs")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32,
        },
        "Clipboard" => ContextSource::Clipboard,
        "Document" => ContextSource::Document {
            path: metadata
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "UserInput" => ContextSource::UserInput {
            source: metadata
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
        },
        other => return Err(format!("unknown source type: {other}")),
    };

    let mut bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let entry = bridge.ingest(source, &content)?;
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

pub(crate) fn neural_bridge_search(
    state: &AppState,
    query: String,
    time_range: Option<(u64, u64)>,
    source_filter: Option<Vec<String>>,
    max_results: Option<usize>,
) -> Result<String, String> {
    let bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let results = bridge.search(&ContextQuery {
        query,
        time_range,
        source_filter,
        max_results: max_results.unwrap_or(20),
    });
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub(crate) fn neural_bridge_delete(state: &AppState, id: String) -> Result<String, String> {
    let mut bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let deleted = bridge.delete_entry(&id);
    Ok(json!({ "deleted": deleted }).to_string())
}

pub(crate) fn neural_bridge_clear_old(
    state: &AppState,
    before_timestamp: u64,
) -> Result<String, String> {
    let mut bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let cleared_count = bridge.clear_before(before_timestamp);
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "neural-bridge",
            "action": "clear_old",
            "cleared_count": cleared_count,
            "before_timestamp": before_timestamp,
        }),
    );
    Ok(json!({ "cleared_count": cleared_count }).to_string())
}

// ---------------------------------------------------------------------------
// Economic Identity commands
// ---------------------------------------------------------------------------

pub(crate) fn economy_create_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let wallet = engine.create_wallet(&agent_id);
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({ "source": "economy", "action": "create_wallet", "agent_id": agent_id }),
    );
    serde_json::to_string(&wallet).map_err(|e| e.to_string())
}

pub(crate) fn economy_get_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match engine.get_wallet(&agent_id) {
        Some(w) => serde_json::to_string(w).map_err(|e| e.to_string()),
        None => Err(format!("wallet not found: {agent_id}")),
    }
}

pub(crate) fn economy_spend(
    state: &AppState,
    agent_id: String,
    amount: f64,
    tx_type: String,
    description: String,
) -> Result<String, String> {
    let parsed_type = match tx_type.as_str() {
        "ApiCall" => TransactionType::ApiCall,
        "ServicePurchase" => TransactionType::ServicePurchase,
        "DataPurchase" => TransactionType::DataPurchase,
        "Refund" => TransactionType::Refund,
        other => return Err(format!("unknown transaction type: {other}")),
    };
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let tx = engine.spend(&agent_id, amount, parsed_type, &description, None)?;
    serde_json::to_string(&tx).map_err(|e| e.to_string())
}

pub(crate) fn economy_earn(
    state: &AppState,
    agent_id: String,
    amount: f64,
    description: String,
) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let tx = engine.earn(&agent_id, amount, &description)?;
    serde_json::to_string(&tx).map_err(|e| e.to_string())
}

pub(crate) fn economy_transfer(
    state: &AppState,
    from: String,
    to: String,
    amount: f64,
    description: String,
) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let (debit, credit) = engine.transfer(&from, &to, amount, &description)?;
    Ok(json!({ "debit": debit, "credit": credit }).to_string())
}

pub(crate) fn economy_freeze_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.freeze_wallet(&agent_id)?;
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({ "source": "economy", "action": "freeze", "agent_id": agent_id }),
    );
    Ok(json!({ "frozen": true }).to_string())
}

pub(crate) fn economy_get_history(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.get_transaction_history(&agent_id);
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

pub(crate) fn economy_get_stats(state: &AppState) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = engine.total_economy_stats();
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Outcome contract commands
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) fn economy_create_contract(
    state: &AppState,
    agent_id: String,
    client_id: String,
    description: String,
    criteria_json: String,
    reward: f64,
    penalty: f64,
    deadline: Option<u64>,
) -> Result<String, String> {
    let criteria: nexus_kernel::economic_identity::SuccessCriteria =
        serde_json::from_str(&criteria_json).map_err(|e| format!("invalid criteria JSON: {e}"))?;
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let contract = engine.create_contract(
        &agent_id,
        &client_id,
        &description,
        criteria,
        reward,
        penalty,
        deadline,
    )?;
    serde_json::to_string(&contract).map_err(|e| e.to_string())
}

pub(crate) fn economy_complete_contract(
    state: &AppState,
    contract_id: String,
    success: bool,
    evidence: Option<String>,
) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let tx = engine.complete_contract(&contract_id, success, evidence)?;
    serde_json::to_string(&tx).map_err(|e| e.to_string())
}

pub(crate) fn economy_list_contracts(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let contracts = engine.list_contracts(&agent_id);
    serde_json::to_string(&contracts).map_err(|e| e.to_string())
}

pub(crate) fn economy_dispute_contract(
    state: &AppState,
    contract_id: String,
    reason: String,
) -> Result<String, String> {
    let mut engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.dispute_contract(&contract_id, &reason)?;
    Ok("disputed".to_string())
}

pub(crate) fn economy_agent_performance(
    state: &AppState,
    agent_id: String,
) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let revenue = engine.revenue_by_outcome(&agent_id);
    serde_json::to_string(&revenue).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Agent Memory commands
// ---------------------------------------------------------------------------

pub(crate) fn agent_memory_remember(
    state: &AppState,
    agent_id: String,
    content: String,
    memory_type: String,
    importance: f64,
    tags: Vec<String>,
) -> Result<String, String> {
    let mt = parse_memory_type(&memory_type)?;
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let entry = mem.remember(&agent_id, &content, mt, importance, tags);
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

pub(crate) fn agent_memory_recall(
    state: &AppState,
    agent_id: String,
    query: String,
    max_results: Option<usize>,
) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let results = mem.recall(&agent_id, &query, max_results.unwrap_or(10));
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub(crate) fn agent_memory_recall_by_type(
    state: &AppState,
    agent_id: String,
    memory_type: String,
    max_results: Option<usize>,
) -> Result<String, String> {
    let mt = parse_memory_type(&memory_type)?;
    let mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let results = mem.recall_by_type(&agent_id, &mt, max_results.unwrap_or(10));
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub(crate) fn agent_memory_forget(
    state: &AppState,
    agent_id: String,
    memory_id: String,
) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let removed = mem.forget(&agent_id, &memory_id);
    Ok(json!({ "removed": removed }).to_string())
}

pub(crate) fn agent_memory_get_stats(state: &AppState, agent_id: String) -> Result<String, String> {
    let mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let stats = mem.get_stats(&agent_id);
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

pub(crate) fn agent_memory_save(state: &AppState, agent_id: String) -> Result<String, String> {
    let mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    mem.save(&agent_id)?;
    Ok(json!({ "saved": true }).to_string())
}

pub(crate) fn agent_memory_clear(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    mem.clear(&agent_id);
    Ok(json!({ "cleared": true }).to_string())
}

pub(crate) fn parse_memory_type(s: &str) -> Result<MemoryType, String> {
    match s {
        "Fact" => Ok(MemoryType::Fact),
        "Preference" => Ok(MemoryType::Preference),
        "Conversation" => Ok(MemoryType::Conversation),
        "Task" => Ok(MemoryType::Task),
        "Error" => Ok(MemoryType::Error),
        "Strategy" => Ok(MemoryType::Strategy),
        other => Err(format!("unknown memory type: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Distributed Tracing commands
// ---------------------------------------------------------------------------

pub(crate) fn tracing_start_trace(
    state: &AppState,
    operation_name: String,
    agent_id: Option<String>,
) -> Result<String, String> {
    let mut engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let (trace_id, span_id) = engine.start_trace(&operation_name, agent_id.as_deref());
    Ok(json!({ "trace_id": trace_id, "span_id": span_id }).to_string())
}

pub(crate) fn tracing_start_span(
    state: &AppState,
    trace_id: String,
    parent_span_id: String,
    operation_name: String,
    agent_id: Option<String>,
) -> Result<String, String> {
    let mut engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let span_id = engine.start_span(
        &trace_id,
        &parent_span_id,
        &operation_name,
        agent_id.as_deref(),
    );
    Ok(json!({ "span_id": span_id }).to_string())
}

pub(crate) fn tracing_end_span(
    state: &AppState,
    span_id: String,
    status: String,
    error_message: Option<String>,
) -> Result<String, String> {
    let span_status = match status.as_str() {
        "Ok" => SpanStatus::Ok,
        "Error" => SpanStatus::Error(error_message.unwrap_or_default()),
        _ => return Err(format!("unknown span status: {status}")),
    };
    let mut engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    engine.end_span(&span_id, span_status);
    Ok(json!({ "ended": true }).to_string())
}

pub(crate) fn tracing_end_trace(state: &AppState, trace_id: String) -> Result<String, String> {
    let mut engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match engine.end_trace(&trace_id) {
        Some(trace) => serde_json::to_string(&trace).map_err(|e| e.to_string()),
        None => Err(format!("trace not found: {trace_id}")),
    }
}

pub(crate) fn tracing_list_traces(
    state: &AppState,
    limit: Option<usize>,
) -> Result<String, String> {
    let engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let traces: Vec<_> = engine.list_traces(limit.unwrap_or(50));
    serde_json::to_string(&traces).map_err(|e| e.to_string())
}

pub(crate) fn tracing_get_trace(state: &AppState, trace_id: String) -> Result<String, String> {
    let engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match engine.get_trace(&trace_id) {
        Some(trace) => serde_json::to_string(trace).map_err(|e| e.to_string()),
        None => Err(format!("trace not found: {trace_id}")),
    }
}

// ---------------------------------------------------------------------------
// Payment commands
// ---------------------------------------------------------------------------

pub(crate) fn parse_billing_interval(s: &str) -> Result<BillingInterval, String> {
    match s {
        "Monthly" => Ok(BillingInterval::Monthly),
        "Yearly" => Ok(BillingInterval::Yearly),
        "OneTime" => Ok(BillingInterval::OneTime),
        other => Err(format!("unknown billing interval: {other}")),
    }
}

pub(crate) fn payment_create_plan(
    state: &AppState,
    name: String,
    price_cents: u64,
    interval: String,
    features: Vec<String>,
) -> Result<String, String> {
    let bi = parse_billing_interval(&interval)?;
    let mut engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let plan = engine.create_plan(&name, price_cents, bi, features);
    serde_json::to_string(&plan).map_err(|e| e.to_string())
}

pub(crate) fn payment_list_plans(state: &AppState) -> Result<String, String> {
    let engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let plans: Vec<_> = engine.list_plans();
    serde_json::to_string(&plans).map_err(|e| e.to_string())
}

pub(crate) fn payment_create_invoice(
    state: &AppState,
    plan_id: String,
    buyer_id: String,
) -> Result<String, String> {
    let mut engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let invoice = engine.create_invoice(&plan_id, &buyer_id)?;
    serde_json::to_string(&invoice).map_err(|e| e.to_string())
}

pub(crate) fn payment_pay_invoice(state: &AppState, invoice_id: String) -> Result<String, String> {
    let mut engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let invoice = engine.pay_invoice(&invoice_id)?;
    serde_json::to_string(&invoice).map_err(|e| e.to_string())
}

pub(crate) fn payment_get_revenue_stats(state: &AppState) -> Result<String, String> {
    let engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = engine.get_revenue_stats();
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

pub(crate) fn payment_create_payout(
    state: &AppState,
    developer_id: String,
    agent_id: String,
    amount_cents: u64,
    period: String,
) -> Result<String, String> {
    let mut engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let payout = engine.create_payout(&developer_id, &agent_id, amount_cents, &period);
    serde_json::to_string(&payout).map_err(|e| e.to_string())
}
