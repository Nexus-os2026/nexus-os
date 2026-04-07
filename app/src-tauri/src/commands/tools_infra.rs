//! tools_infra domain implementation.

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

// ── MCP Host Mode (external MCP tool consumption) ───────────────────────

pub(crate) fn mcp_host_list_servers(state: &AppState) -> Result<String, String> {
    let manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let servers: Vec<serde_json::Value> = manager
        .list_servers()
        .iter()
        .map(|s| {
            let connected = manager.is_server_connected(&s.id);
            json!({
                "id": s.id,
                "name": s.name,
                "url": s.url,
                "transport": s.transport,
                "enabled": s.enabled,
                "connected": connected,
                "tool_count": if connected {
                    manager.list_all_tools().iter().filter(|t| t.server_id == s.id).count()
                } else {
                    0
                },
            })
        })
        .collect();
    serde_json::to_string(&servers).map_err(|e| e.to_string())
}

pub(crate) fn mcp_host_add_server(
    state: &AppState,
    name: String,
    url: String,
    transport: String,
    auth_token: Option<String>,
) -> Result<String, String> {
    let transport_enum = match transport.as_str() {
        "http" | "Http" => McpTransport::Http,
        "sse" | "Sse" => McpTransport::Sse,
        "stdio" | "Stdio" => McpTransport::Stdio,
        _ => return Err(format!("Unknown transport: {transport}")),
    };
    let auth = auth_token.map(McpAuth::Bearer);
    let config = McpServerConfig {
        id: Uuid::new_v4().to_string(),
        name,
        url,
        transport: transport_enum,
        auth,
        enabled: true,
    };
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let result = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    manager.add_server(config)?;
    Ok(result)
}

pub(crate) fn mcp_host_remove_server(
    state: &AppState,
    server_id: String,
) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let removed = manager.remove_server(&server_id);
    Ok(json!({ "removed": removed }).to_string())
}

pub(crate) fn mcp_host_connect(state: &AppState, server_id: String) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let tools = manager.connect_server(&server_id)?;
    let result = json!({
        "server_id": server_id,
        "tools_discovered": tools.len(),
        "tools": tools,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn mcp_host_disconnect(state: &AppState, server_id: String) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    manager.disconnect_server(&server_id);
    Ok(json!({ "disconnected": true }).to_string())
}

pub(crate) fn mcp_host_list_tools(state: &AppState) -> Result<String, String> {
    let manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let tools = manager.list_all_tools();
    serde_json::to_string(&tools).map_err(|e| e.to_string())
}

pub(crate) fn mcp_host_call_tool(
    state: &AppState,
    tool_name: String,
    arguments: String,
) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&arguments).map_err(|e| format!("Invalid arguments JSON: {e}"))?;

    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let mut audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    // Governed call — enforces mcp.call capability and audit logging.
    // UI-initiated calls run as Uuid::nil with full capabilities.
    let result = manager.call_tool(&tool_name, args, SYSTEM_UUID, &["mcp.call"], &mut audit)?;
    drop(audit);
    drop(manager);

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

// ── Ghost Protocol commands ─────────────────────────────────────────────

pub(crate) fn ghost_protocol_status(state: &AppState) -> Result<String, String> {
    let gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = gp.get_stats();
    let result = json!({
        "enabled": gp.enabled(),
        "device_id": gp.device_id(),
        "device_name": gp.device_name(),
        "version": gp.current_version(),
        "peer_count": gp.list_peers().len(),
        "stats": {
            "total_syncs": stats.total_syncs,
            "total_conflicts": stats.total_conflicts,
            "total_changes_sent": stats.total_changes_sent,
            "total_changes_received": stats.total_changes_received,
            "last_sync_time": stats.last_sync_time,
            "connected_peers": stats.connected_peers,
        },
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn ghost_protocol_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    gp.set_enabled(enabled);

    drop(gp);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "ghost-protocol",
            "action": if enabled { "enabled" } else { "disabled" },
        }),
    );

    let result = json!({ "enabled": enabled });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn ghost_protocol_add_peer(
    state: &AppState,
    address: String,
    name: String,
) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    let peer = GhostSyncPeer {
        device_id: Uuid::new_v4().to_string(),
        device_name: name.clone(),
        address: address.clone(),
        last_synced_version: 0,
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        is_connected: true,
    };

    let peer_json = serde_json::to_value(&peer).map_err(|e| e.to_string())?;
    gp.add_peer(peer);

    drop(gp);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "ghost-protocol",
            "action": "add_peer",
            "address": address,
            "name": name,
        }),
    );

    serde_json::to_string(&peer_json).map_err(|e| e.to_string())
}

pub(crate) fn ghost_protocol_remove_peer(
    state: &AppState,
    device_id: String,
) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let removed = gp.remove_peer(&device_id);

    drop(gp);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "ghost-protocol",
            "action": "remove_peer",
            "device_id": device_id,
            "removed": removed,
        }),
    );

    let result = json!({ "removed": removed });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn ghost_protocol_sync_now(state: &AppState) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    // In a real implementation this would contact peers over the network.
    // For now, prepare the delta as proof the engine works.
    let version = gp.current_version();
    let delta = gp.prepare_delta(version.saturating_sub(1));
    let changes_sent = match &delta {
        nexus_distributed::ghost_protocol::SyncMessage::StateDelta { changes, .. } => changes.len(),
        _ => 0,
    };

    drop(gp);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "ghost-protocol",
            "action": "sync_now",
            "changes_sent": changes_sent,
        }),
    );

    let result = json!({
        "changes_sent": changes_sent,
        "changes_received": 0,
        "conflicts": 0,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn ghost_protocol_get_state(state: &AppState) -> Result<String, String> {
    let gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let sync_state = gp.get_state();
    serde_json::to_string(sync_state).map_err(|e| e.to_string())
}

// ── Voice Assistant commands ────────────────────────────────────────────

pub(crate) fn voice_start_listening(state: &AppState) -> Result<String, String> {
    let mut vp = state
        .voice_process
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    // Spawn the Python voice server if not already running.
    if !vp.running {
        let script = std::path::Path::new("services/voice/nexus_voice/voice_server.py");

        match Command::new("python3")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                vp.child = Some(child);
                vp.running = true;
            }
            Err(_) => {
                // Python not available — voice works in stub mode.
                vp.running = false;
            }
        }
    }

    // Update the voice runtime state.
    let mut voice = state.voice.lock().unwrap_or_else(|p| p.into_inner());
    voice.wake_word_enabled = true;
    voice.overlay_visible = true;

    drop(voice);
    drop(vp);

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "voice-assistant",
            "action": "start_listening",
        }),
    );

    let result = json!({ "status": "listening" });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn voice_stop_listening(state: &AppState) -> Result<String, String> {
    let mut vp = state
        .voice_process
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    // Kill the Python process if running.
    if let Some(ref mut child) = vp.child {
        // Best-effort: send kill signal; process may have already exited
        let _ = child.kill();
        // Best-effort: reap zombie process; OS will clean up if this fails
        let _ = child.wait();
    }
    vp.child = None;
    vp.running = false;

    let mut voice = state.voice.lock().unwrap_or_else(|p| p.into_inner());
    voice.wake_word_enabled = false;
    voice.overlay_visible = false;

    drop(voice);
    drop(vp);

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "voice-assistant",
            "action": "stop_listening",
        }),
    );

    let result = json!({ "status": "stopped" });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn voice_get_status(state: &AppState) -> Result<String, String> {
    let voice = state.voice.lock().unwrap_or_else(|p| p.into_inner());
    let vp = state
        .voice_process
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let whisper = state.whisper.lock().unwrap_or_else(|p| p.into_inner());

    let engine = if whisper.is_loaded() {
        "candle-whisper"
    } else if vp.running {
        "python-server"
    } else {
        "stub"
    };

    let result = json!({
        "is_listening": voice.wake_word_enabled,
        "wake_word": "nexus",
        "python_server_running": vp.running,
        "whisper_loaded": whisper.is_loaded(),
        "whisper_model": whisper.model_info(),
        "transcription_engine": engine,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn voice_transcribe(state: &AppState, audio_base64: String) -> Result<String, String> {
    let start = std::time::Instant::now();

    // ── Fallback chain: Candle Whisper → Python server → error ──────

    // 1. Try Candle Whisper if model is loaded
    let whisper = state.whisper.lock().unwrap_or_else(|p| p.into_inner());
    if whisper.is_loaded() {
        // Decode base64 → raw bytes → interpret as 16-bit PCM → f32 samples
        let raw_bytes = base64_decode_audio(&audio_base64)?;
        let pcm: Vec<f32> = raw_bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        match whisper.transcribe(&pcm, 16000) {
            Ok(result) => {
                let json_result = json!({
                    "text": result.text,
                    "engine": result.engine,
                    "duration_ms": result.duration_ms,
                });
                return serde_json::to_string(&json_result).map_err(|e| e.to_string());
            }
            Err(e) => {
                eprintln!("[nexus-voice] candle whisper failed, falling back: {e}");
            }
        }
    }
    drop(whisper);

    // 2. Try Python voice server if running
    let vp = state
        .voice_process
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if vp.running {
        drop(vp);
        // Python server transcription would go here via WebSocket/HTTP
        // For now, fall through to stub since the Python bridge isn't wired yet
        eprintln!("[nexus-voice] python server running but bridge not wired, using stub");
    } else {
        drop(vp);
    }

    // 3. No transcription engine available
    let elapsed = start.elapsed();
    let result = json!({
        "text": "Voice transcription requires Whisper model - load via Model Hub",
        "engine": "none",
        "duration_ms": elapsed.as_millis() as u64,
        "error": true,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Load a Whisper model for on-device speech-to-text.
pub(crate) fn voice_load_whisper_model(
    state: &AppState,
    model_path: String,
) -> Result<String, String> {
    let transcriber = WhisperTranscriber::load_model(&model_path)?;
    let info = transcriber.model_info().unwrap_or_default();

    let mut whisper = state.whisper.lock().unwrap_or_else(|p| p.into_inner());
    *whisper = transcriber;
    drop(whisper);

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "voice-assistant",
            "action": "load_whisper_model",
            "model_path": model_path,
        }),
    );

    let result = json!({
        "status": "loaded",
        "engine": "candle-whisper",
        "model_path": info,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Decode base64-encoded audio data to raw bytes.
pub(crate) fn base64_decode_audio(encoded: &str) -> Result<Vec<u8>, String> {
    // Simple base64 decoder — handles standard base64 alphabet
    let table: Vec<u8> = (0..256u16)
        .map(|i| {
            let c = i as u8;
            match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => 255,
            }
        })
        .collect();

    let input: Vec<u8> = encoded
        .bytes()
        .filter(|&b| b != b'=' && b != b'\n' && b != b'\r')
        .collect();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        let mut buf = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            buf[i] = table[b as usize];
            if buf[i] == 255 {
                return Err(format!("invalid base64 character: {}", b as char));
            }
        }
        output.push((buf[0] << 2) | (buf[1] >> 4));
        if chunk.len() > 2 {
            output.push((buf[1] << 4) | (buf[2] >> 2));
        }
        if chunk.len() > 3 {
            output.push((buf[2] << 6) | buf[3]);
        }
    }

    Ok(output)
}

// ── Software Factory commands ───────────────────────────────────────────

pub(crate) fn factory_create_project(
    state: &AppState,
    name: String,
    language: String,
    source_dir: String,
) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let project = factory.create_project(&name, &language, &source_dir);

    drop(factory);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "software-factory",
            "action": "create_project",
            "project_id": project.id,
            "name": name,
            "language": language,
        }),
    );

    serde_json::to_string(&project).map_err(|e| e.to_string())
}

pub(crate) fn factory_build_project(
    state: &AppState,
    project_id: String,
) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.build_project(&project_id)?;

    drop(factory);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "software-factory",
            "action": "build",
            "project_id": project_id,
            "success": result.success,
            "duration_ms": result.duration_ms,
        }),
    );

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn factory_test_project(state: &AppState, project_id: String) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.test_project(&project_id)?;

    drop(factory);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "software-factory",
            "action": "test",
            "project_id": project_id,
            "success": result.success,
            "passed": result.passed,
            "failed": result.failed,
        }),
    );

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn factory_run_pipeline(state: &AppState, project_id: String) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.run_full_pipeline(&project_id)?;

    drop(factory);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "software-factory",
            "action": "full_pipeline",
            "project_id": project_id,
            "overall_success": result.overall_success,
            "total_duration_ms": result.total_duration_ms,
        }),
    );

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn factory_list_projects(state: &AppState) -> Result<String, String> {
    let factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let projects = factory.list_projects();
    serde_json::to_string(&projects).map_err(|e| e.to_string())
}

pub(crate) fn factory_get_build_history(
    state: &AppState,
    project_id: String,
) -> Result<String, String> {
    let factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let history = factory.get_build_history(&project_id);
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

// ── Conductor Build ─────────────────────────────────────────────────

#[allow(dead_code)]
pub(crate) fn conduct_build(
    state: &AppState,
    prompt: String,
    output_dir: Option<String>,
    model: Option<String>,
) -> Result<serde_json::Value, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let out_dir = output_dir.unwrap_or_else(|| format!("{home}/.nexus/builds/{timestamp}"));

    // Ensure output directory exists
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("failed to create output dir: {e}"))?;

    let full_model = model.unwrap_or_else(|| "openrouter/qwen/qwen3.6-plus:free".to_string());

    // Route to the correct provider based on prefix (e.g. "anthropic/claude-sonnet-4-20250514")
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);
    let (provider, model_name) = provider_from_prefixed_model(&full_model, &prov_config)?;

    // Create conductor
    let mut conductor = Conductor::new(provider, &model_name);

    // Create user request
    let request = UserRequest::new(&prompt, &out_dir);
    let request_id = request.id;

    // Get supervisor
    let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());

    // Preview plan first (for event emission by caller)
    eprintln!("[conductor] Planning with model={model_name}, provider={full_model}");
    let plan = conductor
        .preview_plan(&UserRequest::new(&prompt, &out_dir))
        .map_err(|e| {
            let msg = format!("planning failed (model={model_name}): {e}");
            eprintln!("[conductor] {msg}");
            msg
        })?;
    let plan_json = serde_json::to_value(&plan).unwrap_or_default();

    // Run full orchestration
    let start = std::time::Instant::now();
    eprintln!("[conductor] Running orchestration...");
    let mut result = conductor.run(request, &mut supervisor).map_err(|e| {
        let msg = format!("conductor failed: {e}");
        eprintln!("[conductor] {msg}");
        msg
    })?;
    eprintln!(
        "[conductor] Build finished: {:?}, {} files",
        result.status,
        result.output_files.len()
    );
    result.duration_secs = start.elapsed().as_secs_f64();

    drop(supervisor);

    // Log audit event
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "conductor",
            "action": "conduct_build",
            "request_id": request_id.to_string(),
            "status": format!("{:?}", result.status),
            "agents_used": result.agents_used,
            "total_fuel_used": result.total_fuel_used,
            "duration_secs": result.duration_secs,
        }),
    );

    let result_json = serde_json::to_value(&result).unwrap_or_default();
    Ok(json!({
        "plan": plan_json,
        "result": result_json,
    }))
}

// ── Typed Tools ─────────────────────────────────────────────────────

pub(crate) fn execute_tool(state: &AppState, tool_json: String) -> Result<String, String> {
    use nexus_kernel::typed_tools::{self, TypedTool};

    let tool: TypedTool =
        serde_json::from_str(&tool_json).map_err(|e| format!("invalid tool JSON: {e}"))?;

    // Validate arguments first
    tool.validate()?;

    // Check fuel cost
    let cost = tool.fuel_cost();

    // If destructive or custom-with-approval, flag for HITL
    let needs_hitl = tool.is_destructive()
        || matches!(
            &tool,
            TypedTool::Custom {
                requires_approval: true,
                ..
            }
        );

    // Execute
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let output = typed_tools::execute_typed_tool(&tool, &cwd)?;

    // Audit log
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "typed-tools",
            "tool": output.tool,
            "exit_code": output.exit_code,
            "duration_ms": output.duration_ms,
            "fuel_cost": cost,
            "capability": tool.capability_required(),
            "destructive": tool.is_destructive(),
            "hitl_required": needs_hitl,
        }),
    );

    serde_json::to_string(&output).map_err(|e| e.to_string())
}

pub(crate) fn list_tools() -> Result<String, String> {
    let tools = nexus_kernel::typed_tools::list_available_tools();
    serde_json::to_string(&tools).map_err(|e| e.to_string())
}

/// Parse a shell command string into a TypedTool and execute it.
///
/// Maps well-known commands to safe TypedTool variants.  Unknown commands
/// become `TypedTool::Custom` with `requires_approval: true`.
///
/// Returns JSON-serialised `TerminalResult`.
pub(crate) fn terminal_execute(
    state: &AppState,
    command: String,
    cwd: String,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;
    state.validate_input(&command)?;
    state.validate_path_input(&cwd)?;
    use nexus_kernel::typed_tools::{self, TypedTool};

    #[derive(serde::Serialize)]
    struct TerminalResult {
        stdout: String,
        stderr: String,
        exit_code: i32,
        duration_ms: u64,
        tool: String,
        needs_approval: bool,
        fuel_cost: u64,
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("empty command".into());
    }

    let working_dir = std::path::PathBuf::from(&cwd);
    if !working_dir.is_dir() {
        return Err(format!("directory does not exist: {cwd}"));
    }

    // Parse command string → TypedTool
    let tool: TypedTool = match parts[0] {
        "git" => match parts.get(1).copied() {
            Some("status") => TypedTool::GitStatus,
            Some("diff") => {
                let path = parts.get(2).map(|s| s.to_string());
                TypedTool::GitDiff { path }
            }
            Some("log") => {
                let count = parts
                    .iter()
                    .find_map(|p| p.strip_prefix('-').and_then(|n| n.parse::<usize>().ok()))
                    .unwrap_or(10);
                TypedTool::GitLog { count }
            }
            Some("commit") => {
                let msg = if let Some(pos) = parts.iter().position(|p| *p == "-m") {
                    parts[pos + 1..]
                        .join(" ")
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string()
                } else {
                    String::new()
                };
                TypedTool::GitCommit { message: msg }
            }
            Some("push") => {
                let remote = parts.get(2).unwrap_or(&"origin").to_string();
                let branch = parts.get(3).unwrap_or(&"main").to_string();
                TypedTool::GitPush { remote, branch }
            }
            Some("pull") => {
                let remote = parts.get(2).unwrap_or(&"origin").to_string();
                let branch = parts.get(3).unwrap_or(&"main").to_string();
                TypedTool::GitPull { remote, branch }
            }
            Some("checkout") => {
                let branch = parts.get(2).unwrap_or(&"main").to_string();
                TypedTool::GitCheckout { branch }
            }
            _ => TypedTool::Custom {
                program: "git".into(),
                args: parts[1..].iter().map(|s| s.to_string()).collect(),
                requires_approval: false,
            },
        },
        "cargo" => match parts.get(1).copied() {
            Some("build") | Some("b") => {
                let release = parts.contains(&"--release");
                let package = parts
                    .iter()
                    .position(|p| *p == "-p" || *p == "--package")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());
                TypedTool::CargoBuild { package, release }
            }
            Some("test") | Some("t") => {
                let package = parts
                    .iter()
                    .position(|p| *p == "-p" || *p == "--package")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());
                let test_name = parts.get(2).and_then(|s| {
                    if s.starts_with('-') {
                        None
                    } else {
                        Some(s.to_string())
                    }
                });
                TypedTool::CargoTest { package, test_name }
            }
            Some("fmt") => {
                let check = parts.contains(&"--check");
                TypedTool::CargoFmt { check }
            }
            Some("clippy") => {
                let deny_warnings = parts.contains(&"-D") || parts.contains(&"warnings");
                TypedTool::CargoClippy { deny_warnings }
            }
            Some("run") | Some("r") => {
                let package = parts
                    .iter()
                    .position(|p| *p == "-p" || *p == "--package")
                    .and_then(|i| parts.get(i + 1))
                    .map(|s| s.to_string());
                let extra_args: Vec<String> =
                    if let Some(pos) = parts.iter().position(|p| *p == "--") {
                        parts[pos + 1..].iter().map(|s| s.to_string()).collect()
                    } else {
                        vec![]
                    };
                TypedTool::CargoRun {
                    package,
                    args: extra_args,
                }
            }
            _ => TypedTool::Custom {
                program: "cargo".into(),
                args: parts[1..].iter().map(|s| s.to_string()).collect(),
                requires_approval: false,
            },
        },
        "npm" => match (parts.get(1).copied(), parts.get(2).copied()) {
            (Some("install") | Some("ci") | Some("i"), _) => TypedTool::NpmInstall,
            (Some("test"), _) => TypedTool::NpmTest,
            (Some("run"), Some("build")) => TypedTool::NpmBuild,
            (Some("run"), Some(script)) => TypedTool::NpmRun {
                script: script.to_string(),
            },
            _ => TypedTool::Custom {
                program: "npm".into(),
                args: parts[1..].iter().map(|s| s.to_string()).collect(),
                requires_approval: false,
            },
        },
        "ls" => {
            let recursive = parts.iter().any(|p| p.contains('R'));
            let path = parts
                .iter()
                .find(|p| !p.starts_with('-') && **p != "ls")
                .map(|s| s.to_string())
                .unwrap_or_else(|| ".".into());
            TypedTool::FileList { path, recursive }
        }
        "dir" => TypedTool::FileList {
            path: ".".into(),
            recursive: false,
        },
        "pwd" => TypedTool::Custom {
            program: "pwd".into(),
            args: vec![],
            requires_approval: false,
        },
        "cat" | "head" | "tail" => TypedTool::Custom {
            program: parts[0].to_string(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: false,
        },
        "echo" => TypedTool::Custom {
            program: "echo".into(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: false,
        },
        "whoami" | "date" | "uname" | "uptime" | "hostname" => TypedTool::Custom {
            program: parts[0].to_string(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: false,
        },
        "ps" => TypedTool::ProcessList,
        "df" => TypedTool::DiskUsage {
            path: parts.get(1).unwrap_or(&".").to_string(),
        },
        "free" => TypedTool::Custom {
            program: "free".into(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: false,
        },
        "mkdir" => {
            let path = parts
                .iter()
                .find(|p| !p.starts_with('-') && **p != "mkdir")
                .map(|s| s.to_string())
                .unwrap_or_default();
            if path.is_empty() {
                return Err("mkdir: missing operand".into());
            }
            TypedTool::MakeDirectory { path }
        }
        "cp" => {
            if parts.len() < 3 {
                return Err("cp: missing operand".into());
            }
            TypedTool::FileCopy {
                from: parts[parts.len() - 2].to_string(),
                to: parts[parts.len() - 1].to_string(),
            }
        }
        "mv" => {
            if parts.len() < 3 {
                return Err("mv: missing operand".into());
            }
            TypedTool::FileMove {
                from: parts[1].to_string(),
                to: parts[2].to_string(),
            }
        }
        "rm" => {
            let path = parts
                .iter()
                .find(|p| !p.starts_with('-') && **p != "rm")
                .map(|s| s.to_string())
                .unwrap_or_default();
            if path.is_empty() {
                return Err("rm: missing operand".into());
            }
            TypedTool::FileRemove { path }
        }
        "python3" | "python" => {
            let script = parts.get(1).unwrap_or(&"--version").to_string();
            let args: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
            TypedTool::PythonRun { script, args }
        }
        "pip3" | "pip" => {
            if parts.get(1).copied() == Some("install") {
                TypedTool::PipInstall {
                    packages: parts[2..].iter().map(|s| s.to_string()).collect(),
                }
            } else {
                TypedTool::Custom {
                    program: parts[0].to_string(),
                    args: parts[1..].iter().map(|s| s.to_string()).collect(),
                    requires_approval: true,
                }
            }
        }
        "grep" | "rg" | "find" | "wc" | "sort" | "uniq" | "tree" | "which" | "env" | "printenv"
        | "touch" => TypedTool::Custom {
            program: parts[0].to_string(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: false,
        },
        _ => TypedTool::Custom {
            program: parts[0].to_string(),
            args: parts[1..].iter().map(|s| s.to_string()).collect(),
            requires_approval: true,
        },
    };

    let needs_approval = tool.is_destructive()
        || matches!(
            &tool,
            TypedTool::Custom {
                requires_approval: true,
                ..
            }
        );

    // If it needs approval, return early — frontend handles HITL confirmation
    if needs_approval {
        let result = TerminalResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            duration_ms: 0,
            tool: tool.tool_name(),
            needs_approval: true,
            fuel_cost: tool.fuel_cost(),
        };
        return serde_json::to_string(&result).map_err(|e| e.to_string());
    }

    // Execute
    let output = typed_tools::execute_typed_tool(&tool, &working_dir)?;
    let fuel_cost = tool.fuel_cost();

    // Audit log
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "terminal",
            "command": command,
            "tool": output.tool,
            "exit_code": output.exit_code,
            "duration_ms": output.duration_ms,
            "fuel_cost": fuel_cost,
        }),
    );

    let result = TerminalResult {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
        duration_ms: output.duration_ms,
        tool: output.tool,
        needs_approval: false,
        fuel_cost,
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Force-execute a command that previously required HITL approval.
/// Called after the user clicks "Approve" in the terminal UI.
pub(crate) fn terminal_execute_approved(
    state: &AppState,
    command: String,
    cwd: String,
) -> Result<String, String> {
    state.check_rate(nexus_kernel::rate_limit::RateCategory::AgentExecute)?;
    state.validate_input(&command)?;
    state.validate_path_input(&cwd)?;
    use nexus_kernel::typed_tools::{self, TypedTool};

    #[derive(serde::Serialize)]
    struct TerminalResult {
        stdout: String,
        stderr: String,
        exit_code: i32,
        duration_ms: u64,
        tool: String,
        needs_approval: bool,
        fuel_cost: u64,
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("empty command".into());
    }

    let working_dir = std::path::PathBuf::from(&cwd);

    // For approved commands, build the tool the same way but force-execute
    let tool = TypedTool::Custom {
        program: parts[0].to_string(),
        args: parts[1..].iter().map(|s| s.to_string()).collect(),
        requires_approval: false, // Already approved by HITL
    };

    let output = typed_tools::execute_typed_tool(&tool, &working_dir)?;
    let fuel_cost = tool.fuel_cost();

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "terminal-hitl-approved",
            "command": command,
            "tool": output.tool,
            "exit_code": output.exit_code,
            "duration_ms": output.duration_ms,
            "fuel_cost": fuel_cost,
        }),
    );

    let result = TerminalResult {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
        duration_ms: output.duration_ms,
        tool: output.tool,
        needs_approval: false,
        fuel_cost,
    };
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

// ── Replay Evidence ─────────────────────────────────────────────────

pub(crate) fn replay_list_bundles(
    state: &AppState,
    agent_id: Option<String>,
    limit: Option<usize>,
) -> Result<String, String> {
    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let bundles = recorder.list_bundles(agent_id.as_deref(), limit.unwrap_or(50));
    serde_json::to_string(&bundles).map_err(|e| e.to_string())
}

pub(crate) fn replay_get_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let bundle = recorder
        .get_bundle(&bundle_id)
        .ok_or_else(|| format!("bundle '{bundle_id}' not found"))?;
    serde_json::to_string(bundle).map_err(|e| e.to_string())
}

pub(crate) fn replay_verify_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
    use nexus_kernel::replay::player::ReplayPlayer;

    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let bundle = recorder
        .get_bundle(&bundle_id)
        .ok_or_else(|| format!("bundle '{bundle_id}' not found"))?;
    let verdict = ReplayPlayer::verify_bundle(bundle);

    drop(recorder);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "replay-evidence",
            "action": "verify_bundle",
            "bundle_id": bundle_id,
            "verdict": serde_json::to_value(&verdict).unwrap_or_default(),
        }),
    );

    serde_json::to_string(&verdict).map_err(|e| e.to_string())
}

pub(crate) fn replay_export_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    recorder.export_bundle(&bundle_id)
}

pub(crate) fn replay_toggle_recording(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if enabled {
        recorder.start_recording();
    } else {
        recorder.stop_recording();
    }

    drop(recorder);
    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "replay-evidence",
            "action": "toggle_recording",
            "enabled": enabled,
        }),
    );

    serde_json::to_string(&serde_json::json!({"recording": enabled})).map_err(|e| e.to_string())
}

// ── Air-Gap Deployment ──────────────────────────────────────────────

pub(crate) fn airgap_create_bundle(
    _state: &AppState,
    target_os: String,
    target_arch: String,
    output_path: String,
    components: Option<String>,
) -> Result<String, String> {
    let mut builder = nexus_airgap::AirgapBuilder::new(&target_os, &target_arch);

    // If components JSON array provided, add each
    if let Some(comp_json) = components {
        let comps: Vec<nexus_airgap::BundleComponent> =
            serde_json::from_str(&comp_json).map_err(|e| format!("invalid components: {e}"))?;
        for comp in comps {
            builder.add_component(comp);
        }
    }

    let bundle = builder.build(&output_path)?;
    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

pub(crate) fn airgap_validate_bundle(
    _state: &AppState,
    bundle_path: String,
) -> Result<String, String> {
    let result = nexus_airgap::AirgapInstaller::validate_bundle(&bundle_path);
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub(crate) fn airgap_install_bundle(
    state: &AppState,
    bundle_path: String,
    install_dir: String,
) -> Result<String, String> {
    let bundle = nexus_airgap::AirgapInstaller::install(&bundle_path, &install_dir)?;

    state.log_event(
        SYSTEM_UUID,
        EventType::StateChange,
        json!({
            "source": "airgap",
            "action": "install_bundle",
            "bundle_id": bundle.id,
            "install_dir": install_dir,
        }),
    );

    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

pub(crate) fn airgap_get_system_info(_state: &AppState) -> Result<String, String> {
    let info = nexus_airgap::get_system_info();
    serde_json::to_string(&info).map_err(|e| e.to_string())
}
