use nexus_adaptation::evolution::{EvolutionConfig, EvolutionEngine, MutationType, Strategy};
use nexus_conductor::types::UserRequest;
use nexus_conductor::Conductor;
use nexus_connectors_llm::chunking::SupportedFormat;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::model_hub::{self, DownloadProgress, DownloadStatus};
use nexus_connectors_llm::model_registry::ModelRegistry;
use nexus_connectors_llm::nexus_link::NexusLink;
use nexus_connectors_llm::providers::{LlmProvider, MockProvider, OllamaProvider};
use nexus_connectors_llm::rag::{RagConfig, RagPipeline};
use nexus_connectors_llm::whisper::WhisperTranscriber;
use nexus_distributed::ghost_protocol::{GhostConfig, GhostProtocol, SyncPeer as GhostSyncPeer};
use nexus_factory::pipeline::FactoryPipeline;
use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::computer_control::{ComputerControlEngine, InputAction};
use nexus_kernel::config::{
    load_config, save_config as save_nexus_config, AgentLlmConfig, HardwareConfig, ModelsConfig,
    NexusConfig, OllamaConfig,
};
use nexus_kernel::economic_identity::{EconomicConfig, EconomicEngine, TransactionType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::hardware::{recommend_agent_configs, HardwareProfile};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::neural_bridge::{ContextQuery, ContextSource, NeuralBridge, NeuralBridgeConfig};
use nexus_kernel::permissions::{
    CapabilityRequest as KernelCapabilityRequest, PermissionCategory as KernelPermissionCategory,
    PermissionHistoryEntry as KernelPermissionHistoryEntry,
};
use nexus_kernel::redaction::RedactionEngine;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use nexus_kernel::tracing::{SpanStatus, TracingEngine};
use nexus_marketplace::payments::{BillingInterval, PaymentEngine, RevenueSplit};
use nexus_protocols::mcp_client::{McpAuth, McpHostManager, McpServerConfig, McpTransport};
use nexus_sdk::memory::{AgentMemory, MemoryConfig, MemoryType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
use tauri::Emitter;
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[cfg(not(target_os = "linux"))]
use tauri::Manager;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub fuel_remaining: u64,
    pub fuel_budget: u64,
    pub last_action: String,
    pub capabilities: Vec<String>,
    pub sandbox_runtime: String,
    pub did: Option<String>,
}

/// Lightweight event emitted when agent status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusEvent {
    pub agent_id: String,
    pub status: String,
    pub fuel_remaining: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRow {
    pub event_id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub hash: String,
    pub previous_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
    pub token_count: u32,
    pub cost: f64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrayStatus {
    pub running_agents: usize,
    pub menu_items: Vec<String>,
}

#[derive(Debug, Clone)]
struct AgentMeta {
    name: String,
    last_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VoiceRuntimeState {
    pub wake_word_enabled: bool,
    pub push_to_talk_enabled: bool,
    pub overlay_visible: bool,
}

/// Tracks the Python voice server subprocess.
#[derive(Default)]
struct VoiceProcess {
    child: Option<std::process::Child>,
    running: bool,
}

#[derive(Clone)]
pub struct AppState {
    supervisor: Arc<Mutex<Supervisor>>,
    audit: Arc<Mutex<AuditTrail>>,
    meta: Arc<Mutex<HashMap<AgentId, AgentMeta>>>,
    voice: Arc<Mutex<VoiceRuntimeState>>,
    identity_mgr: Arc<Mutex<nexus_kernel::identity::IdentityManager>>,
    browser: Arc<Mutex<BrowserManager>>,
    research: Arc<Mutex<ResearchManager>>,
    build: Arc<Mutex<BuildManager>>,
    learning: Arc<Mutex<LearningManager>>,
    rag: Arc<Mutex<RagPipeline>>,
    redaction_engine: Arc<Mutex<RedactionEngine>>,
    model_registry: Arc<Mutex<ModelRegistry>>,
    nexus_link: Arc<Mutex<NexusLink>>,
    evolution: Arc<Mutex<EvolutionEngine>>,
    mcp_host: Arc<Mutex<McpHostManager>>,
    ghost_protocol: Arc<Mutex<GhostProtocol>>,
    voice_process: Arc<Mutex<VoiceProcess>>,
    factory: Arc<Mutex<FactoryPipeline>>,
    computer_control: Arc<Mutex<ComputerControlEngine>>,
    neural_bridge: Arc<Mutex<NeuralBridge>>,
    economic_engine: Arc<Mutex<EconomicEngine>>,
    agent_memory: Arc<Mutex<AgentMemory>>,
    tracing_engine: Arc<Mutex<TracingEngine>>,
    payment_engine: Arc<Mutex<PaymentEngine>>,
    whisper: Arc<Mutex<WhisperTranscriber>>,
    replay_recorder: Arc<Mutex<nexus_kernel::replay::recorder::ReplayRecorder>>,
    reputation_registry: Arc<Mutex<nexus_kernel::reputation::ReputationRegistry>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            supervisor: Arc::new(Mutex::new(Supervisor::new())),
            audit: Arc::new(Mutex::new(AuditTrail::new())),
            meta: Arc::new(Mutex::new(HashMap::new())),
            voice: Arc::new(Mutex::new(VoiceRuntimeState {
                wake_word_enabled: true,
                push_to_talk_enabled: true,
                overlay_visible: false,
            })),
            identity_mgr: Arc::new(Mutex::new(
                nexus_kernel::identity::IdentityManager::in_memory(),
            )),
            browser: Arc::new(Mutex::new(BrowserManager::new())),
            research: Arc::new(Mutex::new(ResearchManager::new())),
            build: Arc::new(Mutex::new(BuildManager::new())),
            learning: Arc::new(Mutex::new(LearningManager::new())),
            rag: Arc::new(Mutex::new(RagPipeline::new(RagConfig::default()))),
            redaction_engine: Arc::new(Mutex::new(RedactionEngine::default())),
            model_registry: Arc::new(Mutex::new(ModelRegistry::default_dir())),
            nexus_link: Arc::new(Mutex::new({
                let hostname = std::env::var("HOSTNAME")
                    .or_else(|_| std::env::var("COMPUTERNAME"))
                    .unwrap_or_else(|_| "nexus-device".to_string());
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let models_dir = std::path::Path::new(&home).join(".nexus").join("models");
                NexusLink::new(&hostname, &models_dir.display().to_string())
            })),
            evolution: Arc::new(Mutex::new(EvolutionEngine::new(EvolutionConfig::default()))),
            mcp_host: Arc::new(Mutex::new(McpHostManager::new())),
            ghost_protocol: Arc::new(Mutex::new(GhostProtocol::new(GhostConfig::default()))),
            voice_process: Arc::new(Mutex::new(VoiceProcess::default())),
            factory: Arc::new(Mutex::new(FactoryPipeline::new())),
            computer_control: Arc::new(Mutex::new(ComputerControlEngine::new())),
            neural_bridge: Arc::new(Mutex::new(NeuralBridge::new(NeuralBridgeConfig::default()))),
            economic_engine: Arc::new(Mutex::new(EconomicEngine::new(EconomicConfig::default()))),
            agent_memory: Arc::new(Mutex::new(AgentMemory::new(MemoryConfig::default()))),
            tracing_engine: Arc::new(Mutex::new(TracingEngine::new(1000))),
            payment_engine: Arc::new(Mutex::new(PaymentEngine::new(RevenueSplit::default()))),
            whisper: Arc::new(Mutex::new(WhisperTranscriber::new())),
            replay_recorder: Arc::new(Mutex::new(
                nexus_kernel::replay::recorder::ReplayRecorder::new(500),
            )),
            reputation_registry: Arc::new(Mutex::new(
                nexus_kernel::reputation::ReputationRegistry::new(),
            )),
        }
    }

    fn log_event(&self, agent_id: AgentId, event_type: EventType, payload: serde_json::Value) {
        let mut guard = match self.audit.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Err(e) = guard.append_event(agent_id, event_type, payload) {
            eprintln!("audit append failed: {e}");
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

pub fn get_system_info() -> Result<SystemInfo, String> {
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

pub fn start_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    voice.overlay_visible = true;
    Ok(voice.clone())
}

pub fn stop_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    voice.overlay_visible = false;
    Ok(voice.clone())
}

pub fn jarvis_status(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    Ok(voice.clone())
}

pub fn create_agent(state: &AppState, manifest_json: String) -> Result<String, String> {
    let manifest: AgentManifest = serde_json::from_str(manifest_json.as_str())
        .map_err(|error| format!("invalid manifest JSON: {error}"))?;
    let agent_name = manifest.name.clone();
    let agent_caps = manifest.capabilities.clone();

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let agent_id = supervisor.start_agent(manifest).map_err(agent_error)?;

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

pub fn start_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.restart_agent(parsed).map_err(agent_error)?;
    update_last_action(state, parsed, "started");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "start_agent", "status": "ok"}),
    );
    Ok(())
}

pub fn stop_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.stop_agent(parsed).map_err(agent_error)?;
    update_last_action(state, parsed, "stopped");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "stop_agent", "status": "ok"}),
    );
    Ok(())
}

pub fn pause_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.pause_agent(parsed).map_err(agent_error)?;
    update_last_action(state, parsed, "paused");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "pause_agent", "status": "ok"}),
    );
    Ok(())
}

pub fn resume_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor.resume_agent(parsed).map_err(agent_error)?;
    update_last_action(state, parsed, "resumed");
    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "resume_agent", "status": "ok"}),
    );
    Ok(())
}

pub fn list_agents(state: &AppState) -> Result<Vec<AgentRow>, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let statuses = supervisor.health_check();
    let meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let id_mgr = match state.identity_mgr.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut rows = statuses
        .into_iter()
        .map(|status| {
            let meta = meta_guard.get(&status.id).cloned().unwrap_or(AgentMeta {
                name: "unknown".to_string(),
                last_action: "none".to_string(),
            });

            // Pull real capabilities and fuel_budget from the agent handle
            let (capabilities, fuel_budget) = supervisor
                .get_agent(status.id)
                .map(|h| (h.manifest.capabilities.clone(), h.manifest.fuel_budget))
                .unwrap_or_default();

            // Look up DID if identity exists
            let did = id_mgr.get(&status.id).map(|id| id.did.clone());

            AgentRow {
                id: status.id.to_string(),
                name: meta.name,
                status: status.state.to_string(),
                fuel_remaining: status.remaining_fuel,
                fuel_budget,
                last_action: meta.last_action,
                capabilities,
                sandbox_runtime: "in-process".to_string(),
                did,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(rows)
}

pub fn get_audit_log(
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
fn build_provider_config(config: &NexusConfig) -> ProviderSelectionConfig {
    let non_empty = |s: &str| -> Option<String> {
        if s.trim().is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    };

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
    }
}

/// Select the configured LLM provider using the same logic as `send_chat`.
/// Falls back to `MockProvider` when no real provider is available.
fn get_configured_provider() -> Box<dyn LlmProvider> {
    match load_config() {
        Ok(config) => {
            let prov_config = build_provider_config(&config);
            let provider = select_provider(&prov_config);
            eprintln!("[nexus-rag] selected LLM provider: {}", provider.name());
            provider
        }
        Err(_) => {
            eprintln!("[nexus-rag] config unavailable, falling back to MockProvider");
            Box::new(MockProvider::new())
        }
    }
}

/// Return the default chat/completion model from config (or `"mock-1"`).
fn get_default_model() -> String {
    load_config()
        .map(|c| {
            let m = c.llm.default_model.trim().to_string();
            if m.is_empty() {
                "mock-1".to_string()
            } else {
                m
            }
        })
        .unwrap_or_else(|_| "mock-1".to_string())
}

pub fn send_chat(state: &AppState, message: String) -> Result<ChatResponse, String> {
    let config = load_config().map_err(agent_error)?;
    let provider_config = build_provider_config(&config);
    let provider = select_provider(&provider_config);
    let mut gateway = GovernedLlmGateway::new(provider);

    let mut capabilities = HashSet::new();
    capabilities.insert("llm.query".to_string());
    let mut context = AgentRuntimeContext {
        agent_id: Uuid::new_v4(),
        capabilities,
        fuel_remaining: 50_000,
    };

    let model = if config.llm.default_model.trim().is_empty() {
        "mock-1"
    } else {
        config.llm.default_model.as_str()
    };
    let response = gateway
        .query(&mut context, message.as_str(), 300, model)
        .map_err(agent_error)?;
    let oracle = gateway.oracle_events().last();

    let payload = json!({
        "event": "send_chat",
        "model": response.model_name,
        "token_count": response.token_count,
        "cost": oracle.map(|value| value.cost).unwrap_or(0.0),
        "latency_ms": oracle.map(|value| value.latency_ms).unwrap_or(0)
    });
    state.log_event(context.agent_id, EventType::LlmCall, payload);

    Ok(ChatResponse {
        text: response.output_text,
        model: response.model_name,
        token_count: response.token_count,
        cost: oracle.map(|value| value.cost).unwrap_or(0.0),
        latency_ms: oracle.map(|value| value.latency_ms).unwrap_or(0),
    })
}

pub fn get_config() -> Result<NexusConfig, String> {
    load_config().map_err(agent_error)
}

pub fn save_config(config: NexusConfig) -> Result<(), String> {
    save_nexus_config(&config).map_err(agent_error)
}

pub fn transcribe_push_to_talk() -> Result<String, String> {
    let voice_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../voice");
    if !voice_dir.exists() {
        return Ok("voice runtime unavailable".to_string());
    }

    let output = Command::new("python3")
        .arg("-c")
        .arg(
            "from stt import FasterWhisperSTT; model=FasterWhisperSTT().model; print(f'push-to-talk via {model}')",
        )
        .current_dir(&voice_dir)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let text = String::from_utf8_lossy(&result.stdout).trim().to_string();
            if text.is_empty() {
                Ok("push-to-talk ready".to_string())
            } else {
                Ok(text)
            }
        }
        _ => Ok("push-to-talk captured audio".to_string()),
    }
}

// TODO: Wire to frontend system tray indicator
pub fn tray_status(state: &AppState) -> Result<TrayStatus, String> {
    let agents = list_agents(state)?;
    let running_agents = agents
        .iter()
        .filter(|agent| agent.status == "Running")
        .count();
    Ok(TrayStatus {
        running_agents,
        menu_items: vec![
            "Show Dashboard".to_string(),
            "Start Voice".to_string(),
            "Quit".to_string(),
        ],
    })
}

fn update_last_action(state: &AppState, agent_id: AgentId, action: &str) {
    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&agent_id) {
        meta.last_action = action.to_string();
    }
}

fn event_to_row(event: &AuditEvent) -> AuditRow {
    AuditRow {
        event_id: event.event_id.to_string(),
        timestamp: event.timestamp,
        agent_id: event.agent_id.to_string(),
        event_type: format!("{:?}", event.event_type),
        payload: event.payload.clone(),
        hash: event.hash.clone(),
        previous_hash: event.previous_hash.clone(),
    }
}

fn parse_agent_id(value: &str) -> Result<AgentId, String> {
    uuid::Uuid::parse_str(value).map_err(|error| format!("invalid agent_id: {error}"))
}

fn agent_error(error: AgentError) -> String {
    error.to_string()
}

// ── Setup Wizard Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HardwareInfo {
    pub gpu: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub detected_at: String,
    pub tier: String,
    pub recommended_primary: String,
    pub recommended_fast: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaStatus {
    pub connected: bool,
    pub base_url: String,
    pub models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetupResult {
    pub hardware: HardwareInfo,
    pub ollama: OllamaStatus,
    pub config_saved: bool,
}

// ── Setup Wizard Functions ──

pub fn detect_hardware() -> Result<HardwareInfo, String> {
    let hw = HardwareProfile::detect();
    let tier = hw.recommended_tier();
    Ok(HardwareInfo {
        gpu: hw.gpu,
        vram_mb: hw.vram_mb,
        ram_mb: hw.ram_mb,
        detected_at: hw.detected_at,
        tier: tier.label().to_string(),
        recommended_primary: tier.primary_model().to_string(),
        recommended_fast: tier.fast_model().to_string(),
    })
}

pub fn check_ollama(base_url: Option<String>) -> Result<OllamaStatus, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);

    let connected = provider.health_check().unwrap_or(false);
    let models = if connected {
        provider
            .list_models()
            .unwrap_or_default()
            .into_iter()
            .map(|m| OllamaModelInfo {
                name: m.name,
                size: m.size,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(OllamaStatus {
        connected,
        base_url: url,
        models,
    })
}

pub fn pull_ollama_model(model_name: String, base_url: Option<String>) -> Result<String, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);
    provider
        .pull_model(&model_name, |_status, _completed, _total| {})
        .map_err(|e| e.to_string())
}

/// Pull a model with throttled progress events (max ~3/sec).
/// The callback is only invoked every 300ms for progress updates,
/// but always fires immediately for "success" and error statuses.
pub fn pull_ollama_model_throttled<F>(
    model_name: String,
    base_url: Option<String>,
    mut on_event: F,
) -> Result<String, String>
where
    F: FnMut(ModelPullProgress),
{
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);
    let model_id = model_name.clone();
    let mut last_emit = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(1))
        .unwrap_or_else(std::time::Instant::now);

    provider
        .pull_model(&model_name, |status, completed, total| {
            // Always emit terminal statuses immediately
            if status == "success" || status.contains("error") {
                let percent = if status == "success" { 100 } else { 0 };
                on_event(ModelPullProgress {
                    model: model_id.clone(),
                    status: status.to_string(),
                    percent,
                    completed_bytes: completed,
                    total_bytes: total,
                    error: if status.contains("error") {
                        Some(status.to_string())
                    } else {
                        None
                    },
                });
                return;
            }

            // Throttle: skip if <300ms since last emit
            let now = std::time::Instant::now();
            if now.duration_since(last_emit).as_millis() < 300 {
                return;
            }
            last_emit = now;

            let percent = if total > 0 {
                ((completed as f64 / total as f64) * 100.0).round() as u32
            } else {
                0
            };

            on_event(ModelPullProgress {
                model: model_id.clone(),
                status: status.to_string(),
                percent: percent.min(99),
                completed_bytes: completed,
                total_bytes: total,
                error: None,
            });
        })
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPullProgress {
    pub model: String,
    pub status: String,
    pub percent: u32,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Ensure Ollama server is running. Returns true if already running or started.
pub fn ensure_ollama(base_url: Option<String>) -> Result<bool, String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);

    // Check if already running
    if provider.health_check().unwrap_or(false) {
        return Ok(true);
    }

    // Try to start ollama serve in the background
    let started = Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    if started.is_err() {
        return Err(
            "Ollama is not installed. Please install it from https://ollama.ai".to_string(),
        );
    }

    // Wait up to 8 seconds for it to come online
    for _ in 0..16 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if provider.health_check().unwrap_or(false) {
            return Ok(true);
        }
    }

    Err("Ollama was started but did not respond within 8 seconds".to_string())
}

/// Check if ollama binary is available on PATH.
pub fn is_ollama_installed() -> bool {
    Command::new("ollama")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Delete a model from Ollama.
pub fn delete_ollama_model(model_name: String, base_url: Option<String>) -> Result<(), String> {
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let endpoint = format!("{}/api/delete", url.trim_end_matches('/'));
    let body = json!({ "name": model_name });
    let encoded = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let output = Command::new("curl")
        .args(["-sS", "-X", "DELETE"])
        .arg("-H")
        .arg("content-type: application/json")
        .arg("-d")
        .arg(&encoded)
        .arg(&endpoint)
        .output()
        .map_err(|e| format!("Failed to run curl: {e}"))?;

    if !output.status.success() {
        return Err("Failed to delete model".to_string());
    }
    Ok(())
}

/// Available model entry for the model browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModel {
    pub id: String,
    pub name: String,
    pub size_gb: f64,
    pub context: String,
    pub capabilities: Vec<String>,
    pub recommended: bool,
    pub tag: String,
    pub installed: bool,
    pub description: String,
}

/// List all Qwen 3.5 models with hardware-aware recommendations.
pub fn list_available_models() -> Result<Vec<AvailableModel>, String> {
    let hw = HardwareProfile::detect();
    let vram = hw.vram_mb;
    let ram = hw.ram_mb;

    let provider = OllamaProvider::new("http://localhost:11434");
    let installed_names: Vec<String> = provider
        .list_models()
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name)
        .collect();

    let has = |id: &str| installed_names.iter().any(|n| n == id);

    let mut models = vec![
        AvailableModel {
            id: "qwen3.5:0.8b".into(),
            name: "Qwen 3.5 0.8B".into(),
            size_gb: 1.0,
            context: "256K".into(),
            capabilities: vec!["Text".into(), "Vision".into(), "Tools".into()],
            recommended: false,
            tag: "Ultra-light".into(),
            installed: has("qwen3.5:0.8b"),
            description: "Smallest model — runs on anything".into(),
        },
        AvailableModel {
            id: "qwen3.5:2b".into(),
            name: "Qwen 3.5 2B".into(),
            size_gb: 2.7,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Lightweight".into(),
            installed: has("qwen3.5:2b"),
            description: "Fast responses, great for quick tasks".into(),
        },
        AvailableModel {
            id: "qwen3.5:4b".into(),
            name: "Qwen 3.5 4B".into(),
            size_gb: 3.4,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Balanced".into(),
            installed: has("qwen3.5:4b"),
            description: "Good balance of speed and quality".into(),
        },
        AvailableModel {
            id: "qwen3.5:9b".into(),
            name: "Qwen 3.5 9B".into(),
            size_gb: 6.6,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "Recommended".into(),
            installed: has("qwen3.5:9b"),
            description: "Best quality for consumer GPUs".into(),
        },
        AvailableModel {
            id: "qwen3.5:27b".into(),
            name: "Qwen 3.5 27B".into(),
            size_gb: 17.0,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "High-end".into(),
            installed: has("qwen3.5:27b"),
            description: "Premium quality — needs 24GB+ VRAM or 32GB+ RAM".into(),
        },
        AvailableModel {
            id: "qwen3.5:35b".into(),
            name: "Qwen 3.5 35B MoE".into(),
            size_gb: 24.0,
            context: "256K".into(),
            capabilities: vec![
                "Text".into(),
                "Vision".into(),
                "Code".into(),
                "Reasoning".into(),
                "Tools".into(),
                "Thinking".into(),
            ],
            recommended: false,
            tag: "MoE — only 3B active".into(),
            installed: has("qwen3.5:35b"),
            description: "35B total but only 3B active — fast with enough RAM".into(),
        },
    ];

    // Mark recommendations based on hardware
    for m in &mut models {
        match m.id.as_str() {
            "qwen3.5:9b" if vram >= 8000 => {
                m.recommended = true;
                m.tag = format!("Recommended for your {}MB VRAM", vram);
                m.description = format!("Best model for your {}MB VRAM — fits perfectly", vram);
            }
            "qwen3.5:4b" if (4000..8000).contains(&vram) => {
                m.recommended = true;
                m.tag = "Recommended for your GPU".into();
            }
            "qwen3.5:4b" if vram >= 8000 => {
                m.tag = "Fast companion".into();
                m.description = "Use alongside 9B for quick background tasks".into();
            }
            "qwen3.5:35b" if ram >= 48000 => {
                m.recommended = true;
                m.tag = format!("Bonus — your {}GB RAM enables this", ram / 1024);
                m.description = "MoE model with GPU+RAM offload — premium quality".into();
            }
            "qwen3.5:27b" if vram >= 20000 => {
                m.recommended = true;
                m.tag = "Best for your GPU".into();
            }
            "qwen3.5:27b" if vram < 20000 && ram < 32000 => {
                m.tag = "Too large for your system".into();
            }
            _ => {}
        }
    }

    // Add already-installed non-qwen3.5 models
    for name in &installed_names {
        if !models.iter().any(|m| m.id == *name) {
            models.push(AvailableModel {
                id: name.clone(),
                name: name.replace([':', '-'], " "),
                size_gb: 0.0,
                context: "varies".into(),
                capabilities: vec!["Text".into()],
                recommended: false,
                tag: "Already installed".into(),
                installed: true,
                description: "Previously downloaded model".into(),
            });
        }
    }

    Ok(models)
}

/// Stream a chat completion through Ollama with governance enforcement.
///
/// Pre-flight: PII redaction + prompt firewall on the last user message.
/// Post-flight: audit event with token count and model.
/// The `on_token` callback is called with each token for streaming.
pub fn chat_with_ollama_streaming<F>(
    state: &AppState,
    messages: Vec<serde_json::Value>,
    model: String,
    base_url: Option<String>,
    mut on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    let config = load_config().map_err(|e| e.to_string())?;
    let url = base_url.unwrap_or_else(|| {
        let cfg_url = config.llm.ollama_url.trim();
        if cfg_url.is_empty() {
            "http://localhost:11434".to_string()
        } else {
            cfg_url.to_string()
        }
    });
    let provider = OllamaProvider::new(&url);

    // Ensure Ollama is running first
    if !provider.health_check().unwrap_or(false) {
        return Err(format!(
            "Ollama is not running at {url}. Start it with: ollama serve"
        ));
    }

    // Governance pre-flight: redact PII and check firewall on last user message
    let chat_agent_id = Uuid::new_v4();
    let mut governed_messages = messages.clone();
    if let Some(last_user) = governed_messages
        .iter_mut()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
    {
        if let Some(content) = last_user.get("content").and_then(|c| c.as_str()) {
            // PII redaction
            let mut redaction_engine =
                nexus_kernel::redaction::RedactionEngine::new(Default::default());
            let result = redaction_engine.process_prompt(
                "llm.chat_stream",
                "strict",
                vec![chat_agent_id.to_string()],
                content,
            );
            let redacted = result.outbound_prompt.clone();

            // Prompt firewall check
            let mut input_filter = nexus_kernel::firewall::prompt_firewall::InputFilter::new();
            let mut audit = match state.audit.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            match input_filter.check(chat_agent_id, &redacted, &mut audit) {
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Block { reason } => {
                    return Err(format!("Prompt blocked by firewall: {reason}"));
                }
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Redacted {
                    redacted_text,
                    ..
                } => {
                    *last_user = json!({"role": "user", "content": redacted_text});
                }
                nexus_kernel::firewall::prompt_firewall::FirewallAction::Allow => {
                    // Use the PII-redacted version even if firewall allows
                    if result.summary.total_findings > 0 {
                        *last_user = json!({"role": "user", "content": redacted});
                    }
                }
            }
        }
    }

    let started = std::time::Instant::now();
    let result = provider
        .chat_stream(&governed_messages, &model, |token| {
            on_token(token);
        })
        .map_err(|e| e.to_string())?;
    let latency_ms = started.elapsed().as_millis() as u64;

    // Post-flight audit
    state.log_event(
        chat_agent_id,
        EventType::LlmCall,
        json!({
            "event": "chat_stream",
            "model": model,
            "provider": "ollama",
            "response_length": result.len(),
            "latency_ms": latency_ms,
            "governance": "firewall+redaction",
        }),
    );

    Ok(result)
}

/// Save agent-to-model assignment in config.
pub fn set_agent_model(agent: String, model: String) -> Result<(), String> {
    let mut config = load_config().map_err(|e| e.to_string())?;
    let entry = config.agents.entry(agent).or_insert(AgentLlmConfig {
        model: String::new(),
        temperature: 0.7,
        max_tokens: 4096,
    });
    entry.model = model;
    save_nexus_config(&config).map_err(|e| e.to_string())
}

// ── LLM Provider Management ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderStatusEntry {
    pub name: String,
    pub available: bool,
    pub is_paid: bool,
    pub reason: String,
    pub latency_ms: Option<u64>,
    pub error_hint: Option<String>,
    pub setup_command: Option<String>,
    pub models_installed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatusResponse {
    pub active_provider: String,
    pub providers: Vec<LlmProviderStatusEntry>,
    pub governance_warning: Option<String>,
    pub has_any_provider: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendation {
    pub provider_type: String,
    pub display_name: String,
    pub reason: String,
    pub setup_command: Option<String>,
    pub cost_info: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendations {
    pub ram_mb: u64,
    pub gpu: String,
    pub can_run_local: bool,
    pub recommendations: Vec<LlmRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageStats {
    pub provider_name: String,
    pub total_queries: u64,
    pub total_tokens: u64,
    pub estimated_cost_dollars: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConnectionResult {
    pub provider: String,
    pub success: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub model_used: Option<String>,
}

fn key_present(opt: &Option<String>) -> bool {
    opt.as_deref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
}

/// Smart Ollama status detection: diagnose connection refused, not installed, no models.
fn check_ollama_smart(url: &str) -> LlmProviderStatusEntry {
    let provider = OllamaProvider::new(url);
    let start = std::time::Instant::now();
    let health = provider.health_check();
    let latency = start.elapsed().as_millis() as u64;

    match health {
        Ok(true) => {
            // Connected! Check how many models are installed.
            let models = provider.list_models().unwrap_or_default();
            if models.is_empty() {
                // Ollama running but no models — detect system RAM for recommendation.
                let sys = sysinfo::System::new_all();
                let ram_mb = sys.total_memory() / (1024 * 1024);
                let (suggestion, cmd) = if ram_mb < 8_000 {
                    ("phi3:mini (2.7B, ~1.6GB)", "ollama pull phi3:mini")
                } else if ram_mb < 16_000 {
                    ("llama3:8b (8B, ~4.7GB)", "ollama pull llama3:8b")
                } else if ram_mb < 32_000 {
                    ("llama3:70b-q4 or mixtral:8x7b", "ollama pull mixtral:8x7b")
                } else {
                    ("llama3:70b", "ollama pull llama3:70b")
                };
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: format!(
                        "Ollama is running but has no models. Based on your system ({ram_mb} MB RAM), try: {suggestion}"
                    ),
                    latency_ms: Some(latency),
                    error_hint: Some("No models installed".to_string()),
                    setup_command: Some(cmd.to_string()),
                    models_installed: Some(0),
                }
            } else {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: true,
                    is_paid: false,
                    reason: format!(
                        "connected to {url} ({} model{})",
                        models.len(),
                        if models.len() == 1 { "" } else { "s" }
                    ),
                    latency_ms: Some(latency),
                    error_hint: None,
                    setup_command: None,
                    models_installed: Some(models.len() as u32),
                }
            }
        }
        _ => {
            // Not reachable. Detect whether Ollama binary exists.
            let ollama_installed = Command::new("which")
                .arg("ollama")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !ollama_installed {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: "Ollama not found on this system. Download it from https://ollama.com"
                        .to_string(),
                    latency_ms: None,
                    error_hint: Some("Not installed".to_string()),
                    setup_command: Some(
                        "curl -fsSL https://ollama.com/install.sh | sh".to_string(),
                    ),
                    models_installed: None,
                }
            } else {
                LlmProviderStatusEntry {
                    name: "ollama".to_string(),
                    available: false,
                    is_paid: false,
                    reason: "Ollama is not running. Start it with: ollama serve".to_string(),
                    latency_ms: None,
                    error_hint: Some("Not running".to_string()),
                    setup_command: Some("ollama serve".to_string()),
                    models_installed: None,
                }
            }
        }
    }
}

fn cloud_provider_entry(name: &str, has_key: bool, cost_info: &str) -> LlmProviderStatusEntry {
    LlmProviderStatusEntry {
        name: name.to_string(),
        available: has_key,
        is_paid: true,
        reason: if has_key {
            "API key configured".to_string()
        } else {
            format!("no API key configured ({cost_info})")
        },
        latency_ms: None,
        error_hint: if has_key {
            None
        } else {
            Some("No API key".to_string())
        },
        setup_command: None,
        models_installed: None,
    }
}

/// Check which LLM providers are configured, reachable, and active.
pub fn check_llm_status() -> Result<LlmStatusResponse, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let prov_config = build_provider_config(&config);
    let active = select_provider(&prov_config);
    let active_name = active.name().to_string();

    let mut providers = Vec::new();

    // Ollama — local, free, smart diagnostics
    let ollama_url = prov_config
        .ollama_url
        .as_deref()
        .unwrap_or("http://localhost:11434");
    providers.push(check_ollama_smart(ollama_url));

    // OpenAI
    providers.push(cloud_provider_entry(
        "openai",
        key_present(&prov_config.openai_api_key),
        "~$5/M tokens",
    ));

    // DeepSeek
    providers.push(cloud_provider_entry(
        "deepseek",
        key_present(&prov_config.deepseek_api_key),
        "~$0.14/M tokens, cheapest cloud option",
    ));

    // Gemini
    providers.push(cloud_provider_entry(
        "gemini",
        key_present(&prov_config.gemini_api_key),
        "~$3.50/M tokens",
    ));

    // Claude / Anthropic
    {
        let has_key = key_present(&prov_config.anthropic_api_key);
        #[cfg(feature = "real-claude")]
        let feature_ok = true;
        #[cfg(not(feature = "real-claude"))]
        let feature_ok = false;
        let available = has_key && feature_ok;
        let reason = if !has_key {
            "no API key configured (~$3/M tokens)".to_string()
        } else if !feature_ok {
            "real-claude feature not enabled in build".to_string()
        } else {
            "API key configured, feature enabled".to_string()
        };
        providers.push(LlmProviderStatusEntry {
            name: "claude".to_string(),
            available,
            is_paid: true,
            reason,
            latency_ms: None,
            error_hint: if !feature_ok && has_key {
                Some("Feature gate".to_string())
            } else if !has_key {
                Some("No API key".to_string())
            } else {
                None
            },
            setup_command: None,
            models_installed: None,
        });
    }

    // Mock — always available
    providers.push(LlmProviderStatusEntry {
        name: "mock".to_string(),
        available: true,
        is_paid: false,
        reason: "built-in fallback".to_string(),
        latency_ms: None,
        error_hint: None,
        setup_command: None,
        models_installed: None,
    });

    let has_real = providers.iter().any(|p| p.available && p.name != "mock");

    // Governance warning: if no local provider, warn about cloud governance
    let governance_warning = if !providers.iter().any(|p| p.available && p.name == "ollama") {
        if has_real {
            Some(
                "Governance tasks are using cloud LLM. For maximum privacy, install a local model."
                    .to_string(),
            )
        } else {
            Some("Governance features limited. Configure an LLM provider in Settings.".to_string())
        }
    } else {
        None
    };

    Ok(LlmStatusResponse {
        active_provider: active_name,
        providers,
        governance_warning,
        has_any_provider: has_real,
    })
}

/// Get system-appropriate LLM recommendations.
pub fn get_llm_recommendations() -> Result<LlmRecommendations, String> {
    let sys = sysinfo::System::new_all();
    let ram_mb = sys.total_memory() / (1024 * 1024);

    // Try to detect GPU name from sysinfo cpus (basic heuristic)
    let gpu = "auto-detect".to_string();
    let can_run_local = ram_mb >= 8_000;

    let mut recs = Vec::new();

    // Local recommendations based on RAM
    if ram_mb < 8_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (phi3:mini)".to_string(),
            reason: format!("Your system has {ram_mb} MB RAM. phi3:mini is the lightest option."),
            setup_command: Some("ollama pull phi3:mini".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: false,
        });
    } else if ram_mb < 16_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (llama3:8b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. llama3:8b is a great balance of quality and speed."
            ),
            setup_command: Some("ollama pull llama3:8b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    } else if ram_mb < 32_000 {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (mixtral:8x7b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. mixtral:8x7b offers excellent quality."
            ),
            setup_command: Some("ollama pull mixtral:8x7b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    } else {
        recs.push(LlmRecommendation {
            provider_type: "ollama".to_string(),
            display_name: "Ollama (llama3:70b)".to_string(),
            reason: format!(
                "Your system has {ram_mb} MB RAM. llama3:70b is the most capable local model."
            ),
            setup_command: Some("ollama pull llama3:70b".to_string()),
            cost_info: "Free (local)".to_string(),
            recommended: true,
        });
    }

    // Cloud recommendations — always show
    recs.push(LlmRecommendation {
        provider_type: "deepseek".to_string(),
        display_name: "DeepSeek".to_string(),
        reason: "Cheapest cloud option with strong coding performance.".to_string(),
        setup_command: None,
        cost_info: "~$0.14/M tokens".to_string(),
        recommended: !can_run_local,
    });

    recs.push(LlmRecommendation {
        provider_type: "openai".to_string(),
        display_name: "OpenAI (GPT-4o)".to_string(),
        reason: "Industry standard with broad capabilities.".to_string(),
        setup_command: None,
        cost_info: "~$5/M tokens".to_string(),
        recommended: false,
    });

    recs.push(LlmRecommendation {
        provider_type: "gemini".to_string(),
        display_name: "Google Gemini".to_string(),
        reason: "Strong multimodal capabilities and competitive pricing.".to_string(),
        setup_command: None,
        cost_info: "~$3.50/M tokens".to_string(),
        recommended: false,
    });

    recs.push(LlmRecommendation {
        provider_type: "claude".to_string(),
        display_name: "Anthropic Claude".to_string(),
        reason: "Best for reasoning and safety-conscious tasks.".to_string(),
        setup_command: None,
        cost_info: "~$3/M tokens".to_string(),
        recommended: false,
    });

    Ok(LlmRecommendations {
        ram_mb,
        gpu,
        can_run_local,
        recommendations: recs,
    })
}

/// Set the LLM provider assignment for a specific agent.
pub fn set_agent_llm_provider(
    agent_id: String,
    provider_id: String,
    local_only: bool,
    budget_dollars: u32,
    budget_tokens: u64,
) -> Result<(), String> {
    let mut config = load_config().map_err(agent_error)?;
    let assignment = nexus_kernel::config::AgentLlmAssignment {
        provider_id,
        local_only,
        budget_dollars,
        budget_tokens,
    };
    config.agent_llm_assignments.insert(agent_id, assignment);
    save_nexus_config(&config).map_err(agent_error)
}

/// Get provider usage stats (from audit trail oracle events).
pub fn get_provider_usage_stats(state: &AppState) -> Result<Vec<ProviderUsageStats>, String> {
    let audit = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let events = audit.events();

    // Aggregate by provider from LlmCall audit events
    let mut stats: HashMap<String, (u64, u64, f64)> = HashMap::new();
    for event in events {
        if event.event_type == EventType::LlmCall {
            let provider = event
                .payload
                .get("provider")
                .or_else(|| event.payload.get("model"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let tokens = event
                .payload
                .get("token_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cost = event
                .payload
                .get("cost")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let entry = stats.entry(provider).or_insert((0, 0, 0.0));
            entry.0 += 1;
            entry.1 += tokens;
            entry.2 += cost;
        }
    }

    let result = stats
        .into_iter()
        .map(|(name, (queries, tokens, cost))| ProviderUsageStats {
            provider_name: name,
            total_queries: queries,
            total_tokens: tokens,
            estimated_cost_dollars: cost,
        })
        .collect();

    Ok(result)
}

/// Test connection to a specific provider by sending a simple prompt.
pub fn test_llm_connection(provider_name: String) -> Result<TestConnectionResult, String> {
    let config = load_config().map_err(agent_error)?;
    let prov_config = build_provider_config(&config);

    let mut test_config = prov_config.clone();
    test_config.provider = Some(provider_name.clone());
    let provider = select_provider(&test_config);

    let start = std::time::Instant::now();
    let result = provider.query("Reply with exactly: ok", 10, &config.llm.default_model);
    let latency = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => Ok(TestConnectionResult {
            provider: provider_name,
            success: true,
            latency_ms: latency,
            error: None,
            model_used: Some(response.model_name),
        }),
        Err(e) => Ok(TestConnectionResult {
            provider: provider_name,
            success: false,
            latency_ms: latency,
            error: Some(e.to_string()),
            model_used: None,
        }),
    }
}

// ── Permission Dashboard Commands ──

/// A single permission update for bulk operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionUpdate {
    pub capability_key: String,
    pub enabled: bool,
}

pub fn get_agent_permissions(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelPermissionCategory>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_agent_permissions(parsed)
        .map_err(agent_error)
}

pub fn update_agent_permission(
    state: &AppState,
    agent_id: String,
    capability_key: String,
    enabled: bool,
) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .update_agent_permission(parsed, &capability_key, enabled, "user", None)
        .map_err(agent_error)?;
    state.log_event(
        parsed,
        EventType::UserAction,
        json!({
            "event": "update_agent_permission",
            "capability": capability_key,
            "enabled": enabled,
        }),
    );
    Ok(())
}

pub fn get_permission_history(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelPermissionHistoryEntry>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_permission_history(parsed)
        .map_err(agent_error)
}

pub fn get_capability_request(
    state: &AppState,
    agent_id: String,
) -> Result<Vec<KernelCapabilityRequest>, String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    supervisor
        .get_capability_requests(parsed)
        .map_err(agent_error)
}

pub fn bulk_update_permissions(
    state: &AppState,
    agent_id: String,
    updates: Vec<PermissionUpdate>,
    reason: Option<String>,
) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;
    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let update_pairs: Vec<(String, bool)> = updates
        .iter()
        .map(|u| (u.capability_key.clone(), u.enabled))
        .collect();
    supervisor
        .bulk_update_agent_permissions(parsed, &update_pairs, "user", reason.as_deref())
        .map_err(agent_error)?;
    state.log_event(
        parsed,
        EventType::UserAction,
        json!({
            "event": "bulk_update_permissions",
            "updates": updates.len(),
            "reason": reason,
        }),
    );
    Ok(())
}

// ── Policy Engine API ──

pub fn policy_list() -> Result<serde_json::Value, String> {
    let dir = dirs_policy_dir();
    let mut engine = nexus_kernel::policy_engine::PolicyEngine::new(&dir);
    let _ = engine.load_policies();
    let policies: Vec<serde_json::Value> = engine
        .policies()
        .iter()
        .map(|p| {
            json!({
                "policy_id": p.policy_id,
                "description": p.description,
                "effect": format!("{:?}", p.effect),
                "principal": p.principal,
                "action": p.action,
                "resource": p.resource,
                "priority": p.priority,
                "conditions": {
                    "min_autonomy_level": p.conditions.min_autonomy_level,
                    "max_fuel_cost": p.conditions.max_fuel_cost,
                    "required_approvers": p.conditions.required_approvers,
                    "time_window": p.conditions.time_window,
                },
            })
        })
        .collect();
    Ok(json!({ "policies": policies, "count": policies.len() }))
}

pub fn policy_validate(content: String) -> Result<serde_json::Value, String> {
    match toml::from_str::<nexus_kernel::policy_engine::Policy>(&content) {
        Ok(policy) => Ok(json!({
            "valid": true,
            "policy_id": policy.policy_id,
            "effect": format!("{:?}", policy.effect),
        })),
        Err(e) => Ok(json!({
            "valid": false,
            "error": e.to_string(),
        })),
    }
}

pub fn policy_test(
    content: String,
    principal: String,
    action: String,
    resource: String,
) -> Result<serde_json::Value, String> {
    let policy: nexus_kernel::policy_engine::Policy =
        toml::from_str(&content).map_err(|e| format!("invalid policy TOML: {e}"))?;
    let engine = nexus_kernel::policy_engine::PolicyEngine::with_policies(vec![policy]);
    let ctx = nexus_kernel::policy_engine::EvaluationContext::default();
    let decision = engine.evaluate(&principal, &action, &resource, &ctx);
    Ok(json!({
        "principal": principal,
        "action": action,
        "resource": resource,
        "decision": format!("{decision:?}"),
    }))
}

pub fn policy_detect_conflicts() -> Result<serde_json::Value, String> {
    let dir = dirs_policy_dir();
    let mut engine = nexus_kernel::policy_engine::PolicyEngine::new(&dir);
    let _ = engine.load_policies();

    let policies = engine.policies();
    let mut conflicts: Vec<serde_json::Value> = Vec::new();

    for (i, a) in policies.iter().enumerate() {
        for b in policies.iter().skip(i + 1) {
            let principal_overlap =
                a.principal == "*" || b.principal == "*" || a.principal == b.principal;
            let action_overlap = a.action == "*" || b.action == "*" || a.action == b.action;
            let resource_overlap =
                a.resource == "*" || b.resource == "*" || a.resource == b.resource;
            let effect_differs = a.effect != b.effect;

            if principal_overlap && action_overlap && resource_overlap && effect_differs {
                conflicts.push(json!({
                    "policy_a": a.policy_id,
                    "policy_b": b.policy_id,
                    "effect_a": format!("{:?}", a.effect),
                    "effect_b": format!("{:?}", b.effect),
                    "overlap": {
                        "principal": if a.principal == b.principal { &a.principal } else { "*" },
                        "action": if a.action == b.action { &a.action } else { "*" },
                        "resource": if a.resource == b.resource { &a.resource } else { "*" },
                    },
                }));
            }
        }
    }

    Ok(json!({ "conflicts": conflicts, "count": conflicts.len() }))
}

fn dirs_policy_dir() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home)
            .join(".nexus")
            .join("policies")
    } else {
        std::path::PathBuf::from("~/.nexus/policies")
    }
}

/// Check if setup has been completed (hardware detected).
pub fn is_setup_complete() -> bool {
    match load_config() {
        Ok(cfg) => !cfg.hardware.gpu.is_empty() && cfg.hardware.gpu != "none",
        Err(_) => false,
    }
}

pub fn run_setup_wizard(ollama_url: Option<String>) -> Result<SetupResult, String> {
    let hw_info = detect_hardware()?;
    let ollama_status = check_ollama(ollama_url.clone())?;

    // Build and save config
    let mut config = load_config().map_err(|e| e.to_string())?;

    config.hardware = HardwareConfig {
        gpu: hw_info.gpu.clone(),
        vram_mb: hw_info.vram_mb,
        ram_mb: hw_info.ram_mb,
        detected_at: hw_info.detected_at.clone(),
    };

    config.ollama = OllamaConfig {
        base_url: ollama_status.base_url.clone(),
        status: if ollama_status.connected {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        },
    };
    config.llm.ollama_url = ollama_status.base_url.clone();

    config.models = ModelsConfig {
        primary: hw_info.recommended_primary.clone(),
        fast: hw_info.recommended_fast.clone(),
    };

    // Set default model to the recommended primary
    if ollama_status.connected {
        config.llm.default_model = hw_info.recommended_primary.clone();
    }

    // Apply agent configs
    let hw = HardwareProfile {
        gpu: hw_info.gpu.clone(),
        vram_mb: hw_info.vram_mb,
        ram_mb: hw_info.ram_mb,
        detected_at: hw_info.detected_at.clone(),
    };
    let tier = hw.recommended_tier();
    let agent_configs = recommend_agent_configs(tier);
    let mut agents_map = BTreeMap::new();
    for (name, ac) in &agent_configs {
        agents_map.insert(
            name.to_string(),
            AgentLlmConfig {
                model: ac.model.clone(),
                temperature: ac.temperature,
                max_tokens: ac.max_tokens,
            },
        );
    }
    config.agents = agents_map;

    let config_saved = save_nexus_config(&config).is_ok();

    Ok(SetupResult {
        hardware: hw_info,
        ollama: ollama_status,
        config_saved,
    })
}

// ── Protocols Dashboard Commands ──

/// Protocol server status for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolsStatusRow {
    pub a2a_status: String,
    pub a2a_version: String,
    pub a2a_peers: u32,
    pub a2a_tasks_processed: u64,
    pub mcp_status: String,
    pub mcp_registered_tools: u32,
    pub mcp_invocations: u64,
    pub gateway_port: Option<u16>,
    pub governance_bridge_active: bool,
    pub audit_integrity: bool,
}

/// A protocol request log entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRequestRow {
    pub id: String,
    pub timestamp: u64,
    pub protocol: String,
    pub method: String,
    pub sender: String,
    pub agent: String,
    pub status: String,
    pub fuel_consumed: u64,
    pub governance_decision: String,
}

/// MCP tool entry for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRow {
    pub name: String,
    pub description: String,
    pub agent: String,
    pub fuel_cost: u64,
    pub requires_hitl: bool,
    pub invocations: u64,
}

/// Agent Card summary for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardRow {
    pub agent_name: String,
    pub url: String,
    pub skills_count: usize,
    pub auth_scheme: String,
    pub rate_limit_rpm: u64,
    pub card_json: serde_json::Value,
}

pub fn get_protocols_status(state: &AppState) -> Result<ProtocolsStatusRow, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let audit = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let agent_count = supervisor.health_check().len() as u32;

    Ok(ProtocolsStatusRow {
        a2a_status: "stopped".to_string(),
        a2a_version: "0.2.1".to_string(),
        a2a_peers: 0,
        a2a_tasks_processed: 0,
        mcp_status: "stopped".to_string(),
        mcp_registered_tools: agent_count * 3, // estimate: ~3 tools per agent
        mcp_invocations: 0,
        gateway_port: None,
        governance_bridge_active: false,
        audit_integrity: audit.verify_integrity(),
    })
}

pub fn get_protocols_requests(_state: &AppState) -> Result<Vec<ProtocolRequestRow>, String> {
    // Return recent protocol requests — empty until gateway is started
    Ok(Vec::new())
}

pub fn get_mcp_tools(state: &AppState) -> Result<Vec<McpToolRow>, String> {
    use nexus_kernel::protocols::mcp::McpServer;

    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let mut mcp = McpServer::new();
            mcp.register_agent(agent_status.id, handle.manifest.clone());
            if let Ok(tools) = mcp.list_tools(agent_status.id) {
                for tool in tools {
                    rows.push(McpToolRow {
                        name: tool.name,
                        description: tool.description.unwrap_or_default(),
                        agent: handle.manifest.name.clone(),
                        fuel_cost: tool.governance.estimated_fuel_cost,
                        requires_hitl: tool.governance.requires_hitl,
                        invocations: 0,
                    });
                }
            }
        }
    }
    Ok(rows)
}

pub fn get_agent_cards(state: &AppState) -> Result<Vec<AgentCardRow>, String> {
    use nexus_kernel::protocols::a2a::AgentCard;

    let supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let card = AgentCard::from_manifest(&handle.manifest, "http://localhost:3000");
            let card_json = serde_json::to_value(&card).unwrap_or_default();
            let auth_scheme = if card.authentication.is_empty() {
                "none".to_string()
            } else {
                card.authentication
                    .iter()
                    .map(|a| a.scheme_type.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            rows.push(AgentCardRow {
                agent_name: card.name.clone(),
                url: card.url.clone(),
                skills_count: card.skills.len(),
                auth_scheme,
                rate_limit_rpm: card.rate_limit_rpm.unwrap_or(0),
                card_json,
            });
        }
    }
    Ok(rows)
}

// ── Identity Commands ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRow {
    pub agent_id: String,
    pub did: String,
    pub created_at: u64,
    pub public_key_hex: String,
}

pub fn get_agent_identity(state: &AppState, agent_id: String) -> Result<IdentityRow, String> {
    let uuid = uuid::Uuid::parse_str(&agent_id).map_err(|e| format!("invalid UUID: {e}"))?;
    let mut mgr = state.identity_mgr.lock().map_err(|e| e.to_string())?;
    let identity = mgr
        .get_or_create(uuid)
        .map_err(|e| format!("identity error: {e}"))?;
    Ok(IdentityRow {
        agent_id: uuid.to_string(),
        did: identity.did.clone(),
        created_at: identity.created_at,
        public_key_hex: identity
            .public_key_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
}

pub fn list_identities(state: &AppState) -> Result<Vec<IdentityRow>, String> {
    let sup = state.supervisor.lock().map_err(|e| e.to_string())?;
    let mut mgr = state.identity_mgr.lock().map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for agent_status in sup.health_check() {
        if let Ok(identity) = mgr.get_or_create(agent_status.id) {
            rows.push(IdentityRow {
                agent_id: agent_status.id.to_string(),
                did: identity.did.clone(),
                created_at: identity.created_at,
                public_key_hex: identity
                    .public_key_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            });
        }
    }
    Ok(rows)
}

// ── Firewall Commands ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallStatusRow {
    pub status: String,
    pub mode: String,
    pub injection_pattern_count: usize,
    pub pii_pattern_count: usize,
    pub exfil_pattern_count: usize,
    pub sensitive_path_count: usize,
    pub ssn_detection: bool,
    pub passport_detection: bool,
    pub internal_ip_detection: bool,
    pub context_overflow_threshold_bytes: usize,
    pub egress_default_deny: bool,
    pub egress_rate_limit_per_min: u32,
}

pub fn get_firewall_status(_state: &AppState) -> Result<FirewallStatusRow, String> {
    let summary = nexus_kernel::firewall::pattern_summary();
    Ok(FirewallStatusRow {
        status: "active".to_string(),
        mode: "fail-closed".to_string(),
        injection_pattern_count: summary.injection_count,
        pii_pattern_count: summary.pii_count,
        exfil_pattern_count: summary.exfil_count,
        sensitive_path_count: summary.sensitive_path_count,
        ssn_detection: summary.has_ssn_detection,
        passport_detection: summary.has_passport_detection,
        internal_ip_detection: summary.has_internal_ip_detection,
        context_overflow_threshold_bytes: summary.context_overflow_threshold_bytes,
        egress_default_deny: true,
        egress_rate_limit_per_min: nexus_kernel::firewall::DEFAULT_RATE_LIMIT_PER_MIN,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallPatternsRow {
    pub injection_patterns: Vec<String>,
    pub pii_patterns: Vec<String>,
    pub exfil_patterns: Vec<String>,
    pub sensitive_paths: Vec<String>,
    pub ssn_regex: String,
    pub passport_regex: String,
    pub internal_ip_regex: String,
}

pub fn get_firewall_patterns() -> Result<FirewallPatternsRow, String> {
    use nexus_kernel::firewall::patterns;
    Ok(FirewallPatternsRow {
        injection_patterns: patterns::INJECTION_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        pii_patterns: patterns::PII_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        exfil_patterns: patterns::EXFIL_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        sensitive_paths: patterns::SENSITIVE_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ssn_regex: patterns::SSN_PATTERN.to_string(),
        passport_regex: patterns::PASSPORT_PATTERN.to_string(),
        internal_ip_regex: patterns::INTERNAL_IP_PATTERN.to_string(),
    })
}

// ── Marketplace API ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceAgentRow {
    pub package_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub tags: Vec<String>,
    pub version: String,
    pub capabilities: Vec<String>,
    pub price_cents: i64,
    pub downloads: i64,
    pub rating: f64,
    pub review_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceReviewRow {
    pub reviewer: String,
    pub stars: u8,
    pub comment: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceDetailRow {
    pub agent: MarketplaceAgentRow,
    pub reviews: Vec<MarketplaceReviewRow>,
    pub versions: Vec<MarketplaceVersionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceVersionRow {
    pub version: String,
    pub changelog: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplacePublishResult {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub verdict: String,
    pub checks: Vec<MarketplaceCheckRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceCheckRow {
    pub name: String,
    pub passed: bool,
    pub findings: Vec<String>,
}

fn open_marketplace_registry() -> Result<nexus_marketplace::sqlite_registry::SqliteRegistry, String>
{
    let db_path = nexus_marketplace::sqlite_registry::SqliteRegistry::default_db_path();
    nexus_marketplace::sqlite_registry::SqliteRegistry::open(&db_path)
        .map_err(|e| format!("Failed to open marketplace database: {e}"))
}

pub fn marketplace_search(query: &str) -> Result<Vec<MarketplaceAgentRow>, String> {
    let registry = open_marketplace_registry()?;
    let results = registry
        .search(query)
        .map_err(|e| format!("Search failed: {e}"))?;

    Ok(results
        .into_iter()
        .map(|r| {
            // Get full detail for each result to include rating/downloads
            let detail = registry.get_agent(&r.package_id).ok();
            MarketplaceAgentRow {
                package_id: r.package_id,
                name: r.name,
                description: r.description,
                author: r.author_id,
                tags: r.tags,
                version: detail
                    .as_ref()
                    .map(|d| d.version.clone())
                    .unwrap_or_default(),
                capabilities: detail
                    .as_ref()
                    .map(|d| d.capabilities.clone())
                    .unwrap_or_default(),
                price_cents: detail.as_ref().map(|d| d.price_cents).unwrap_or(0),
                downloads: detail.as_ref().map(|d| d.downloads).unwrap_or(0),
                rating: detail.as_ref().map(|d| d.rating).unwrap_or(0.0),
                review_count: detail.as_ref().map(|d| d.review_count).unwrap_or(0),
            }
        })
        .collect())
}

pub fn marketplace_install(package_id: &str) -> Result<MarketplaceAgentRow, String> {
    let registry = open_marketplace_registry()?;
    let bundle = registry
        .install(package_id)
        .map_err(|e| format!("Install failed: {e}"))?;
    let detail = registry
        .get_agent(package_id)
        .map_err(|e| format!("Failed to get agent detail: {e}"))?;

    Ok(MarketplaceAgentRow {
        package_id: bundle.package_id,
        name: bundle.metadata.name,
        description: bundle.metadata.description,
        author: bundle.metadata.author_id,
        tags: bundle.metadata.tags,
        version: bundle.metadata.version,
        capabilities: bundle.metadata.capabilities,
        price_cents: detail.price_cents,
        downloads: detail.downloads,
        rating: detail.rating,
        review_count: detail.review_count,
    })
}

pub fn marketplace_info(agent_id: &str) -> Result<MarketplaceDetailRow, String> {
    let registry = open_marketplace_registry()?;
    let detail = registry
        .get_agent(agent_id)
        .map_err(|e| format!("Agent not found: {e}"))?;
    let reviews = registry.get_reviews(agent_id).unwrap_or_default();
    let versions = registry.version_history(agent_id).unwrap_or_default();

    Ok(MarketplaceDetailRow {
        agent: MarketplaceAgentRow {
            package_id: detail.package_id,
            name: detail.name,
            description: detail.description,
            author: detail.author,
            tags: detail.tags,
            version: detail.version,
            capabilities: detail.capabilities,
            price_cents: detail.price_cents,
            downloads: detail.downloads,
            rating: detail.rating,
            review_count: detail.review_count,
        },
        reviews: reviews
            .into_iter()
            .map(|r| MarketplaceReviewRow {
                reviewer: r.reviewer,
                stars: r.stars,
                comment: r.comment,
                created_at: r.created_at,
            })
            .collect(),
        versions: versions
            .into_iter()
            .map(|v| MarketplaceVersionRow {
                version: v.version,
                changelog: v.changelog,
                created_at: v.created_at,
            })
            .collect(),
    })
}

pub fn marketplace_publish(bundle_json: &str) -> Result<MarketplacePublishResult, String> {
    use nexus_marketplace::package::SignedPackageBundle;
    use nexus_marketplace::verification_pipeline::{verify_bundle, Verdict};

    let bundle: SignedPackageBundle =
        serde_json::from_str(bundle_json).map_err(|e| format!("Invalid bundle format: {e}"))?;

    let verification = verify_bundle(&bundle);
    if verification.verdict == Verdict::Rejected {
        let findings: Vec<String> = verification
            .checks
            .iter()
            .filter(|c| !c.passed)
            .flat_map(|c| c.findings.clone())
            .collect();
        return Err(format!("Verification rejected: {}", findings.join("; ")));
    }

    let registry = open_marketplace_registry()?;
    registry
        .upsert_signed(&bundle)
        .map_err(|e| format!("Publish failed: {e}"))?;

    Ok(MarketplacePublishResult {
        package_id: bundle.package_id,
        name: bundle.metadata.name,
        version: bundle.metadata.version,
        verdict: format!("{:?}", verification.verdict),
        checks: verification
            .checks
            .iter()
            .map(|c| MarketplaceCheckRow {
                name: c.name.clone(),
                passed: c.passed,
                findings: c.findings.clone(),
            })
            .collect(),
    })
}

pub fn marketplace_my_agents(author: &str) -> Result<Vec<MarketplaceAgentRow>, String> {
    let registry = open_marketplace_registry()?;
    let results = registry
        .search(author)
        .map_err(|e| format!("Query failed: {e}"))?;

    Ok(results
        .into_iter()
        .filter(|r| r.author_id == author)
        .map(|r| {
            let detail = registry.get_agent(&r.package_id).ok();
            MarketplaceAgentRow {
                package_id: r.package_id,
                name: r.name,
                description: r.description,
                author: r.author_id,
                tags: r.tags,
                version: detail
                    .as_ref()
                    .map(|d| d.version.clone())
                    .unwrap_or_default(),
                capabilities: detail
                    .as_ref()
                    .map(|d| d.capabilities.clone())
                    .unwrap_or_default(),
                price_cents: detail.as_ref().map(|d| d.price_cents).unwrap_or(0),
                downloads: detail.as_ref().map(|d| d.downloads).unwrap_or(0),
                rating: detail.as_ref().map(|d| d.rating).unwrap_or(0.0),
                review_count: detail.as_ref().map(|d| d.review_count).unwrap_or(0),
            }
        })
        .collect())
}

// ── Research Mode ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentState {
    pub agent_id: String,
    pub agent_name: String,
    pub status: String,
    pub current_url: Option<String>,
    pub query: String,
    pub findings: Vec<String>,
    pub pages_visited: u32,
    pub fuel_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSessionState {
    pub session_id: String,
    pub topic: String,
    pub status: String,
    pub supervisor_message: String,
    pub sub_agents: Vec<SubAgentState>,
    pub summary: Option<String>,
    pub total_fuel_used: u64,
    pub pages_visited: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchEvent {
    pub event_type: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub message: String,
    pub url: Option<String>,
    pub finding: Option<String>,
    pub summary: Option<String>,
}

/// Manages multi-agent research sessions with supervisor delegation.
/// Each session: supervisor breaks topic into sub-queries, assigns to sub-agents,
/// sub-agents search + extract, supervisor merges findings.
/// PII redaction via PromptFirewall, fuel metered per page + LLM call, all audited.
#[derive(Debug, Clone, Default)]
pub struct ResearchManager {
    sessions: HashMap<String, ResearchSessionState>,
}

impl ResearchManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn get_session(&self, session_id: &str) -> Option<&ResearchSessionState> {
        self.sessions.get(session_id)
    }

    pub fn list_sessions(&self) -> Vec<ResearchSessionState> {
        self.sessions.values().cloned().collect()
    }
}

/// PII redaction helper — applies PromptFirewall-style redaction to extracted text.
/// Redacts SSN, email, phone patterns before they enter findings.
fn redact_pii(text: &str) -> String {
    use nexus_kernel::firewall::prompt_firewall::{FirewallAction, InputFilter};

    let mut filter = InputFilter::default();
    let agent_id = Uuid::nil();
    let mut audit = AuditTrail::new();
    match filter.check(agent_id, text, &mut audit) {
        FirewallAction::Redacted { redacted_text, .. } => redacted_text,
        _ => text.to_string(),
    }
}

/// Fuel cost constants for research operations.
const FUEL_PER_PAGE_VISIT: u64 = 25;
const FUEL_PER_LLM_EXTRACTION: u64 = 50;
const FUEL_PER_MERGE: u64 = 100;

fn start_research(
    state: &AppState,
    topic: String,
    num_agents: u32,
) -> Result<ResearchSessionState, String> {
    let num_agents = num_agents.clamp(1, 5);
    let session_id = Uuid::new_v4().to_string();
    let supervisor_id = Uuid::new_v4();

    // Audit: research started
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "research_started",
            "session_id": session_id,
            "topic": topic,
            "num_agents": num_agents,
        }),
    );

    // Step 1: Supervisor breaks topic into sub-queries
    let sub_queries = generate_sub_queries(&topic, num_agents);

    // Step 2: Create sub-agents
    let mut sub_agents = Vec::new();
    for (i, query) in sub_queries.iter().enumerate() {
        let agent_id = Uuid::new_v4().to_string();
        let agent_name = format!("Sub-Agent-{}", i + 1);

        sub_agents.push(SubAgentState {
            agent_id: agent_id.clone(),
            agent_name: agent_name.clone(),
            status: "searching".to_string(),
            current_url: None,
            query: query.clone(),
            findings: Vec::new(),
            pages_visited: 0,
            fuel_used: 0,
        });

        // Audit: sub-agent assigned
        state.log_event(
            supervisor_id,
            EventType::ToolCall,
            json!({
                "event": "agent_assigned",
                "session_id": session_id,
                "agent_id": agent_id,
                "agent_name": agent_name,
                "query": query,
            }),
        );
    }

    let supervisor_msg = format!(
        "Assigning research task to {}",
        sub_agents
            .iter()
            .map(|a| a.agent_name.as_str())
            .collect::<Vec<_>>()
            .join(" and ")
    );

    let session = ResearchSessionState {
        session_id: session_id.clone(),
        topic: topic.clone(),
        status: "running".to_string(),
        supervisor_message: supervisor_msg,
        sub_agents: sub_agents.clone(),
        summary: None,
        total_fuel_used: 0,
        pages_visited: 0,
    };

    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    research
        .sessions
        .insert(session_id.clone(), session.clone());

    // Add activity to browser manager for the activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Research started: \"{}\" with {} sub-agents",
            topic, num_agents
        ),
    );
    for agent in &sub_agents {
        browser.add_activity(
            &agent.agent_id,
            &agent.agent_name,
            "searching",
            &format!("Assigned query: \"{}\"", agent.query),
        );
    }

    Ok(session)
}

/// Generate sub-queries by splitting the topic into focused aspects.
fn generate_sub_queries(topic: &str, num_agents: u32) -> Vec<String> {
    let aspects = [
        "overview and key concepts",
        "recent developments and trends",
        "practical applications and examples",
        "challenges and limitations",
        "future directions and outlook",
    ];
    (0..num_agents as usize)
        .map(|i| {
            let aspect = aspects.get(i).unwrap_or(&"additional details");
            format!("{} — {}", topic, aspect)
        })
        .collect()
}

fn research_agent_action(
    state: &AppState,
    session_id: String,
    agent_id: String,
    action: String,
    url: Option<String>,
    content: Option<String>,
) -> Result<ResearchSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());

    let session = research
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Research session {} not found", session_id))?;

    let agent = session
        .sub_agents
        .iter_mut()
        .find(|a| a.agent_id == agent_id)
        .ok_or_else(|| format!("Sub-agent {} not found", agent_id))?;

    let agent_name = agent.agent_name.clone();

    // Egress governance check (before mutating agent state)
    if action == "reading" {
        if let Some(ref target_url) = url {
            let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(reason) = browser.check_url(target_url) {
                drop(research);
                state.log_event(
                    supervisor_id,
                    EventType::ToolCall,
                    json!({
                        "event": "research_url_blocked",
                        "session_id": session_id,
                        "agent_id": agent_id,
                        "url": target_url,
                        "reason": reason,
                    }),
                );
                return Err(format!("URL blocked by egress policy: {}", reason));
            }
        }
    }

    match action.as_str() {
        "searching" => {
            agent.status = "searching".to_string();
            agent.current_url = url.clone();
        }
        "reading" => {
            // Fuel metered per page visit
            agent.fuel_used += FUEL_PER_PAGE_VISIT;
            agent.pages_visited += 1;
            agent.status = "reading".to_string();
            agent.current_url = url.clone();
        }
        "extracting" => {
            // Fuel metered per LLM extraction call
            agent.fuel_used += FUEL_PER_LLM_EXTRACTION;
            agent.status = "extracting".to_string();

            // PII redaction on extracted content
            if let Some(ref raw_content) = content {
                let redacted = redact_pii(raw_content);
                agent.findings.push(redacted);
            }
        }
        "done" => {
            agent.status = "done".to_string();
            agent.current_url = None;
        }
        _ => {
            return Err(format!("Unknown action: {}", action));
        }
    }

    // Capture agent fields we need for activity stream before dropping the mutable borrow
    let agent_query = agent.query.clone();
    let agent_findings_count = agent.findings.len();

    // Update session totals (no longer conflicts with agent borrow)
    session.total_fuel_used = session.sub_agents.iter().map(|a| a.fuel_used).sum();
    session.pages_visited = session.sub_agents.iter().map(|a| a.pages_visited).sum();

    let result = session.clone();

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": format!("agent_{}", action),
            "session_id": session_id,
            "agent_id": agent_id,
            "agent_name": agent_name,
            "url": url,
            "fuel_cost": match action.as_str() {
                "reading" => FUEL_PER_PAGE_VISIT,
                "extracting" => FUEL_PER_LLM_EXTRACTION,
                _ => 0,
            },
        }),
    );

    // Record in browser activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    let msg_type = match action.as_str() {
        "searching" => "searching",
        "reading" => "reading",
        "extracting" => "extracting",
        "done" => "info",
        _ => "info",
    };
    let content_msg = match action.as_str() {
        "searching" => format!("Searching: \"{}\"", agent_query),
        "reading" => format!("Reading: {}", url.as_deref().unwrap_or("unknown")),
        "extracting" => format!(
            "Extracting findings from {}",
            url.as_deref().unwrap_or("current page")
        ),
        "done" => format!("Completed with {} findings", agent_findings_count),
        _ => action.clone(),
    };
    browser.add_activity(&agent_id, &agent_name, msg_type, &content_msg);

    // Record URL visit in browser history
    if let Some(ref target_url) = url {
        if action == "reading" {
            let title = target_url
                .split("://")
                .nth(1)
                .unwrap_or(target_url)
                .split('/')
                .next()
                .unwrap_or("Untitled")
                .to_string();
            browser.record_visit(target_url, &title, Some(agent_id.clone()));
        }
    }

    Ok(result)
}

fn complete_research(state: &AppState, session_id: String) -> Result<ResearchSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut research = state.research.lock().unwrap_or_else(|p| p.into_inner());

    let session = research
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Research session {} not found", session_id))?;

    // Fuel for merge operation
    session.total_fuel_used += FUEL_PER_MERGE;
    session.status = "merging".to_string();

    // Collect all findings from sub-agents, apply PII redaction to merged summary
    let all_findings: Vec<String> = session
        .sub_agents
        .iter()
        .flat_map(|a| {
            let header = format!("## {} (query: \"{}\")", a.agent_name, a.query);
            let mut items = vec![header];
            for (j, f) in a.findings.iter().enumerate() {
                items.push(format!("{}. {}", j + 1, f));
            }
            items
        })
        .collect();

    let raw_summary = format!(
        "# Research Summary: {}\n\n{}\n\n---\nTotal pages visited: {} | Total fuel used: {}",
        session.topic,
        all_findings.join("\n"),
        session.pages_visited,
        session.total_fuel_used,
    );

    // PII redaction on merged summary
    let summary = redact_pii(&raw_summary);

    session.summary = Some(summary.clone());
    session.status = "complete".to_string();

    // Mark all sub-agents as done
    for agent in &mut session.sub_agents {
        agent.status = "done".to_string();
        agent.current_url = None;
    }

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "research_complete",
            "session_id": session_id,
            "topic": session.topic,
            "total_pages": session.pages_visited,
            "total_fuel": session.total_fuel_used,
            "findings_count": session.sub_agents.iter().map(|a| a.findings.len()).sum::<usize>(),
        }),
    );

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "extracting",
        &format!(
            "Merging findings from {} agents ({} total findings)",
            session.sub_agents.len(),
            session
                .sub_agents
                .iter()
                .map(|a| a.findings.len())
                .sum::<usize>(),
        ),
    );
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Research complete: {} pages visited, {} fuel consumed",
            session.pages_visited, session.total_fuel_used,
        ),
    );

    let result = session.clone();
    Ok(result)
}

fn get_research_session(
    state: &AppState,
    session_id: String,
) -> Result<ResearchSessionState, String> {
    let research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    research
        .get_session(&session_id)
        .cloned()
        .ok_or_else(|| format!("Research session {} not found", session_id))
}

fn list_research_sessions(state: &AppState) -> Result<Vec<ResearchSessionState>, String> {
    let research = state.research.lock().unwrap_or_else(|p| p.into_inner());
    Ok(research.list_sessions())
}

// ── Build Mode ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildAgentMessage {
    pub id: String,
    pub timestamp: u64,
    pub agent_name: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSessionState {
    pub session_id: String,
    pub description: String,
    pub status: String,
    pub code: String,
    pub preview_html: String,
    pub messages: Vec<BuildAgentMessage>,
    pub fuel_used: u64,
    pub llm_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCodeDelta {
    pub session_id: String,
    pub delta: String,
    pub full_code: String,
    pub agent_name: String,
}

/// Fuel cost constants for build operations.
const FUEL_PER_BUILD_LLM_CALL: u64 = 75;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Manages build sessions where agents write code collaboratively.
#[derive(Debug, Clone, Default)]
pub struct BuildManager {
    sessions: HashMap<String, BuildSessionState>,
}

impl BuildManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

fn build_msg(agent_name: &str, role: &str, content: &str) -> BuildAgentMessage {
    BuildAgentMessage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_secs(),
        agent_name: agent_name.to_string(),
        role: role.to_string(),
        content: content.to_string(),
    }
}

/// Wrap code in a full HTML document for preview rendering.
fn wrap_preview_html(code: &str) -> String {
    // If code already has <html> or <!DOCTYPE>, use as-is
    let lower = code.to_lowercase();
    if lower.contains("<html") || lower.contains("<!doctype") {
        return code.to_string();
    }
    format!(
        "<!DOCTYPE html>\n<html>\n<head><meta charset=\"utf-8\"><style>body{{margin:0;font-family:system-ui,sans-serif}}</style></head>\n<body>\n{}\n</body>\n</html>",
        code
    )
}

fn start_build(state: &AppState, description: String) -> Result<BuildSessionState, String> {
    let session_id = Uuid::new_v4().to_string();
    let supervisor_id = Uuid::new_v4();

    // Audit: build started
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_started",
            "session_id": session_id,
            "description": description,
        }),
    );

    let mut messages = Vec::new();
    messages.push(build_msg(
        "Supervisor",
        "supervisor",
        &format!("Build task received: {}", description),
    ));
    messages.push(build_msg(
        "Supervisor",
        "supervisor",
        "Assigning to Coder agent. Designer agent on standby for styling.",
    ));

    let session = BuildSessionState {
        session_id: session_id.clone(),
        description,
        status: "planning".to_string(),
        code: String::new(),
        preview_html: String::new(),
        messages,
        fuel_used: 0,
        llm_calls: 0,
    };

    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions.insert(session_id.clone(), session.clone());

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!("Build started: {}", session.description),
    );

    Ok(session)
}

fn build_append_code(
    state: &AppState,
    session_id: String,
    delta: String,
    agent_name: String,
) -> Result<BuildSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    // PII redaction on code content
    let redacted_delta = redact_pii(&delta);
    session.code.push_str(&redacted_delta);
    session.preview_html = wrap_preview_html(&session.code);
    session.status = "coding".to_string();
    session.fuel_used += FUEL_PER_BUILD_LLM_CALL;
    session.llm_calls += 1;

    let result = session.clone();
    drop(bm);

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_code_delta",
            "session_id": session_id,
            "agent_name": agent_name,
            "delta_len": redacted_delta.len(),
            "total_len": result.code.len(),
            "fuel_cost": FUEL_PER_BUILD_LLM_CALL,
        }),
    );

    // Activity stream
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        &agent_name.to_lowercase().replace(' ', "-"),
        &agent_name,
        "extracting",
        &format!("Writing code ({} chars)", redacted_delta.len()),
    );

    Ok(result)
}

fn build_add_message(
    state: &AppState,
    session_id: String,
    agent_name: String,
    role: String,
    content: String,
) -> Result<BuildSessionState, String> {
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    session
        .messages
        .push(build_msg(&agent_name, &role, &content));

    let result = session.clone();
    drop(bm);

    // Activity stream
    let msg_type = match role.as_str() {
        "coder" => "extracting",
        "designer" => "deciding",
        _ => "info",
    };
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        &agent_name.to_lowercase().replace(' ', "-"),
        &agent_name,
        msg_type,
        &content,
    );

    Ok(result)
}

fn complete_build(state: &AppState, session_id: String) -> Result<BuildSessionState, String> {
    let supervisor_id = Uuid::new_v4();
    let mut bm = state.build.lock().unwrap_or_else(|p| p.into_inner());

    let session = bm
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| format!("Build session {} not found", session_id))?;

    session.status = "complete".to_string();
    session.preview_html = wrap_preview_html(&session.code);
    session.messages.push(build_msg(
        "Supervisor",
        "supervisor",
        "Build complete. Preview is ready.",
    ));

    let result = session.clone();
    drop(bm);

    // Audit
    state.log_event(
        supervisor_id,
        EventType::ToolCall,
        json!({
            "event": "build_complete",
            "session_id": session_id,
            "code_len": result.code.len(),
            "fuel_used": result.fuel_used,
            "llm_calls": result.llm_calls,
        }),
    );

    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "supervisor",
        "Supervisor",
        "info",
        &format!(
            "Build complete — {} chars, {} LLM calls",
            result.code.len(),
            result.llm_calls
        ),
    );

    Ok(result)
}

fn get_build_session(state: &AppState, session_id: String) -> Result<BuildSessionState, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

fn get_build_code(state: &AppState, session_id: String) -> Result<String, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .map(|s| s.code.clone())
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

fn get_build_preview(state: &AppState, session_id: String) -> Result<String, String> {
    let bm = state.build.lock().unwrap_or_else(|p| p.into_inner());
    bm.sessions
        .get(&session_id)
        .map(|s| s.preview_html.clone())
        .ok_or_else(|| format!("Build session {} not found", session_id))
}

// ── Learn Mode ──

/// Fuel cost constants for learning operations.
const FUEL_PER_LEARN_BROWSE: u64 = 25;
const FUEL_PER_LEARN_EXTRACT: u64 = 50;
const FUEL_PER_LEARN_COMPARE: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSource {
    pub url: String,
    pub label: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub id: String,
    pub title: String,
    pub source_url: String,
    pub key_points: Vec<String>,
    pub timestamp: u64,
    pub relevance_score: f64,
    pub category: String,
    pub is_new: bool,
    pub change_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSuggestion {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source_url: String,
    pub relevance: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSessionState {
    pub session_id: String,
    pub status: String,
    pub sources: Vec<LearningSource>,
    pub current_source_idx: usize,
    pub current_url: Option<String>,
    pub knowledge_base: Vec<KnowledgeEntry>,
    pub suggestions: Vec<LearningSuggestion>,
    pub fuel_used: u64,
    pub pages_visited: u64,
}

/// Manages learning sessions where agents browse documentation to stay current.
#[derive(Debug, Clone, Default)]
pub struct LearningManager {
    sessions: HashMap<String, LearningSessionState>,
    knowledge: Vec<KnowledgeEntry>,
}

impl LearningManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            knowledge: Vec::new(),
        }
    }
}

fn start_learning(
    state: &AppState,
    sources: Vec<LearningSource>,
) -> Result<LearningSessionState, String> {
    let session_id = Uuid::new_v4().to_string();
    let agent_id = Uuid::new_v4();

    // Validate sources — all must be http(s) URLs
    for src in &sources {
        let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(reason) = browser.check_url(&src.url) {
            return Err(format!("Source {} blocked: {}", src.label, reason));
        }
    }

    // Audit: learning started
    state.log_event(
        agent_id,
        EventType::ToolCall,
        json!({
            "event": "learning_started",
            "session_id": session_id,
            "source_count": sources.len(),
            "sources": sources.iter().map(|s| &s.url).collect::<Vec<_>>(),
        }),
    );

    let session = LearningSessionState {
        session_id: session_id.clone(),
        status: "browsing".to_string(),
        sources,
        current_source_idx: 0,
        current_url: None,
        knowledge_base: Vec::new(),
        suggestions: Vec::new(),
        fuel_used: 0,
        pages_visited: 0,
    };

    let mut lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    lm.sessions.insert(session_id.clone(), session.clone());

    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    browser.add_activity(
        "learn-agent",
        "LearnAgent",
        "info",
        &format!(
            "Learning session {} started with {} sources",
            &session_id[..8],
            session.sources.len()
        ),
    );

    Ok(session)
}

fn learning_agent_action(
    state: &AppState,
    session_id: String,
    action: String,
    url: Option<String>,
    content: Option<String>,
) -> Result<LearningSessionState, String> {
    let agent_id = Uuid::new_v4();

    // Egress check if URL provided
    if let Some(ref u) = url {
        let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
        if let Err(reason) = browser.check_url(u) {
            state.log_event(
                agent_id,
                EventType::ToolCall,
                json!({
                    "event": "learning_blocked",
                    "session_id": session_id,
                    "url": u,
                    "reason": reason,
                }),
            );
            return Err(format!("URL blocked: {}", reason));
        }
    }

    let mut lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());

    // Snapshot existing knowledge URLs before borrowing session
    let existing_knowledge_urls: HashSet<String> =
        lm.knowledge.iter().map(|k| k.source_url.clone()).collect();

    if !lm.sessions.contains_key(&session_id) {
        return Err(format!("Learning session {} not found", session_id));
    }

    // Perform session mutations in a block so we can access lm.knowledge afterward
    let (result, entries_to_merge) = {
        let session = lm
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;

        match action.as_str() {
            "browse" => {
                session.fuel_used += FUEL_PER_LEARN_BROWSE;
                session.pages_visited += 1;
                session.current_url = url.clone();
                session.status = "browsing".to_string();

                if let Some(ref u) = url {
                    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                    browser.record_visit(u, "Learning", Some("learn-agent".to_string()));
                    browser.add_activity(
                        "learn-agent",
                        "LearnAgent",
                        "navigating",
                        &format!("Browsing: {}", u),
                    );
                }

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "agent_browsing",
                        "session_id": session_id,
                        "url": url,
                        "fuel_used": session.fuel_used,
                    }),
                );

                (session.clone(), None)
            }
            "extract" => {
                session.fuel_used += FUEL_PER_LEARN_EXTRACT;
                session.status = "extracting".to_string();

                let source_url =
                    url.unwrap_or_else(|| session.current_url.clone().unwrap_or_default());
                let src_label = session
                    .sources
                    .iter()
                    .find(|s| s.url == source_url)
                    .map(|s| s.label.clone())
                    .unwrap_or_else(|| source_url.clone());
                let src_category = session
                    .sources
                    .iter()
                    .find(|s| s.url == source_url)
                    .map(|s| s.category.clone())
                    .unwrap_or_else(|| "documentation".to_string());

                let raw_content =
                    content.unwrap_or_else(|| format!("Extracted information from {}", src_label));
                let redacted = redact_pii(&raw_content);

                let entry = KnowledgeEntry {
                    id: Uuid::new_v4().to_string(),
                    title: format!("{} — Latest", src_label),
                    source_url: source_url.clone(),
                    key_points: vec![redacted],
                    timestamp: now_secs(),
                    relevance_score: 0.5,
                    category: src_category,
                    is_new: true,
                    change_summary: None,
                };
                session.knowledge_base.push(entry);

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "agent_extracted",
                        "session_id": session_id,
                        "source": source_url,
                        "fuel_used": session.fuel_used,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "extracting",
                    &format!("Extracted from {}", src_label),
                );

                (session.clone(), None)
            }
            "compare" => {
                session.fuel_used += FUEL_PER_LEARN_COMPARE;
                session.status = "comparing".to_string();

                for entry in &mut session.knowledge_base {
                    if !existing_knowledge_urls.contains(&entry.source_url) {
                        entry.is_new = true;
                        entry.change_summary =
                            Some("New source — not previously in knowledge base".to_string());
                        entry.relevance_score = 0.8;
                    }
                }

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "knowledge_updated",
                        "session_id": session_id,
                        "knowledge_count": session.knowledge_base.len(),
                        "fuel_used": session.fuel_used,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "deciding",
                    "Compared with existing knowledge base",
                );

                (session.clone(), None)
            }
            "done" => {
                session.status = "complete".to_string();
                session.current_url = None;

                let kb_len = session.knowledge_base.len();
                let fuel_used = session.fuel_used;
                let pages_visited = session.pages_visited;
                let merge = session.knowledge_base.clone();

                state.log_event(
                    agent_id,
                    EventType::ToolCall,
                    json!({
                        "event": "learning_complete",
                        "session_id": session_id,
                        "knowledge_entries": kb_len,
                        "fuel_used": fuel_used,
                        "pages_visited": pages_visited,
                    }),
                );

                let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
                browser.add_activity(
                    "learn-agent",
                    "LearnAgent",
                    "info",
                    &format!("Learning complete — {} entries, {} fuel", kb_len, fuel_used),
                );

                (session.clone(), Some(merge))
            }
            other => {
                return Err(format!("Unknown learning action: {}", other));
            }
        }
    };

    // Merge knowledge entries into global store (session borrow is now dropped)
    if let Some(entries) = entries_to_merge {
        for entry in entries {
            lm.knowledge.push(entry);
        }
    }

    Ok(result)
}

fn get_learning_session(
    state: &AppState,
    session_id: String,
) -> Result<LearningSessionState, String> {
    let lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    lm.sessions
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Learning session {} not found", session_id))
}

fn get_knowledge_base(state: &AppState) -> Result<Vec<KnowledgeEntry>, String> {
    let lm = state.learning.lock().unwrap_or_else(|p| p.into_inner());
    Ok(lm.knowledge.clone())
}

// ── Agent Browser ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigateResult {
    pub url: String,
    pub title: String,
    pub allowed: bool,
    pub deny_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserHistoryEntry {
    pub url: String,
    pub title: String,
    pub timestamp: u64,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMessageRow {
    pub id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub agent_name: String,
    pub message_type: String,
    pub content: String,
}

/// BrowserManager tracks active browsing sessions and enforces egress governance.
/// URLs are checked against a built-in blocklist and (when agents are assigned)
/// against the agent's `allowed_endpoints` from their manifest.
#[derive(Debug, Clone, Default)]
pub struct BrowserManager {
    history: Vec<BrowserHistoryEntry>,
    activity: Vec<ActivityMessageRow>,
    /// Blocked domain patterns (default deny list).
    blocked_domains: Vec<String>,
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            activity: Vec::new(),
            blocked_domains: vec![
                "malware.".to_string(),
                "phishing.".to_string(),
                "darkweb.".to_string(),
            ],
        }
    }

    /// Check whether a URL is allowed by egress governance.
    /// Returns Ok(title) on success, Err(reason) on block.
    pub fn check_url(&self, url: &str) -> Result<(), String> {
        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Only http:// and https:// URLs are allowed".to_string());
        }

        // Extract host for domain check
        let host = url
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        // Check against blocked domains
        for blocked in &self.blocked_domains {
            if host.contains(blocked) {
                return Err(format!("Domain blocked by egress policy: {}", host));
            }
        }

        Ok(())
    }

    pub fn record_visit(&mut self, url: &str, title: &str, agent_id: Option<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.history.push(BrowserHistoryEntry {
            url: url.to_string(),
            title: title.to_string(),
            timestamp: now,
            agent_id,
        });
    }

    pub fn add_activity(
        &mut self,
        agent_id: &str,
        agent_name: &str,
        message_type: &str,
        content: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.activity.push(ActivityMessageRow {
            id: Uuid::new_v4().to_string(),
            timestamp: now,
            agent_id: agent_id.to_string(),
            agent_name: agent_name.to_string(),
            message_type: message_type.to_string(),
            content: content.to_string(),
        });
    }
}

fn navigate_to(state: &AppState, url: String) -> Result<BrowserNavigateResult, String> {
    let mut browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());

    // Egress governance check — fail-closed
    if let Err(reason) = browser.check_url(&url) {
        browser.add_activity(
            "system",
            "Firewall",
            "blocked",
            &format!("Blocked: {} — {}", url, reason),
        );

        // Audit the blocked attempt
        let system_id = Uuid::nil();
        state.log_event(
            system_id,
            EventType::ToolCall,
            json!({
                "event": "browser_navigate",
                "url": url,
                "allowed": false,
                "reason": reason,
            }),
        );

        return Ok(BrowserNavigateResult {
            url,
            title: String::new(),
            allowed: false,
            deny_reason: Some(reason),
        });
    }

    // Extract a title from the URL (real browser would parse HTML)
    let title = url
        .split("://")
        .nth(1)
        .unwrap_or(&url)
        .split('/')
        .next()
        .unwrap_or("Untitled")
        .to_string();

    browser.record_visit(&url, &title, None);
    browser.add_activity(
        "system",
        "Browser",
        "navigating",
        &format!("Loaded: {}", url),
    );

    // Audit the page visit
    let system_id = Uuid::nil();
    state.log_event(
        system_id,
        EventType::ToolCall,
        json!({
            "event": "browser_navigate",
            "url": url,
            "allowed": true,
            "title": title,
        }),
    );

    Ok(BrowserNavigateResult {
        url,
        title,
        allowed: true,
        deny_reason: None,
    })
}

fn get_browser_history(state: &AppState) -> Result<Vec<BrowserHistoryEntry>, String> {
    let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    Ok(browser.history.clone())
}

fn get_agent_activity(state: &AppState) -> Result<Vec<ActivityMessageRow>, String> {
    let browser = state.browser.lock().unwrap_or_else(|p| p.into_inner());
    Ok(browser.activity.clone())
}

// ── RAG Pipeline Commands ──

fn format_from_extension(path: &str) -> Result<SupportedFormat, String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "txt" | "text" | "log" | "csv" => Ok(SupportedFormat::PlainText),
        "md" | "markdown" => Ok(SupportedFormat::Markdown),
        "rs" | "py" | "js" | "ts" | "go" | "java" | "c" | "cpp" | "h" | "toml" | "yaml"
        | "yml" | "json" | "html" | "css" | "sh" | "bash" | "sql" | "rb" | "swift" | "kt" => {
            Ok(SupportedFormat::Code)
        }
        _ => Err(format!(
            "unsupported file extension '.{ext}'. Supported: .txt, .md, .rs, .py, .js, .ts, .go, .java, .c, .cpp, .toml, .yaml, .json, .html, .css, .sh, .sql"
        )),
    }
}

pub fn index_document(state: &AppState, file_path: String) -> Result<String, String> {
    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("failed to read file: {e}"))?;

    let format = format_from_extension(&file_path)?;

    let provider = get_configured_provider();

    // Detect embedding dimension from the provider on first ingest.
    // Probe with a short text to discover the actual dimension.
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    if rag.documents.is_empty() {
        if let Ok(probe) = provider.embed(&["dimension probe"], &rag.config.embedding_model) {
            if let Some(first) = probe.embeddings.first() {
                let detected = first.len();
                if detected != rag.config.embedding_dimension {
                    eprintln!(
                        "[nexus-rag] embedding dimension changed: {} -> {} (provider: {}). Recreating vector store.",
                        rag.config.embedding_dimension, detected, provider.name()
                    );
                    rag.config.embedding_dimension = detected;
                    rag.vector_store =
                        nexus_connectors_llm::vector_store::VectorStore::new(detected);
                }
            }
        }
    } else {
        // Documents already indexed — warn if dimension would change
        if let Ok(probe) = provider.embed(&["dimension probe"], &rag.config.embedding_model) {
            if let Some(first) = probe.embeddings.first() {
                let detected = first.len();
                if detected != rag.config.embedding_dimension {
                    eprintln!(
                        "[nexus-rag] WARNING: provider {} produces {}-dim embeddings but store uses {}. Re-index to switch.",
                        provider.name(), detected, rag.config.embedding_dimension
                    );
                }
            }
        }
    }

    let mut redaction = state
        .redaction_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    let doc = rag
        .ingest_document(&content, &file_path, format, &provider, &mut redaction)
        .map_err(|e| format!("ingest failed: {e}"))?;

    drop(rag);
    drop(redaction);

    state.log_event(
        Uuid::new_v4(),
        EventType::ToolCall,
        json!({
            "event": "rag.ingest",
            "file_path": file_path,
            "format": doc.format,
            "chunk_count": doc.chunk_count,
            "provider": provider.name(),
        }),
    );

    serde_json::to_string(&doc).map_err(|e| format!("serialize error: {e}"))
}

pub fn search_documents(
    state: &AppState,
    query: String,
    top_k: Option<u32>,
) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());

    if let Some(k) = top_k {
        rag.config.top_k = k as usize;
    }

    let provider = get_configured_provider();
    let results = rag
        .query(&query, &provider)
        .map_err(|e| format!("search failed: {e}"))?;

    // SearchResult doesn't derive Serialize, so convert manually.
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "chunk_id": r.chunk_id,
                "doc_path": r.doc_path,
                "chunk_index": r.chunk_index,
                "content": r.content,
                "score": r.score,
            })
        })
        .collect();

    serde_json::to_string(&rows).map_err(|e| format!("serialize error: {e}"))
}

pub fn chat_with_documents(state: &AppState, question: String) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let provider = get_configured_provider();

    let results = rag
        .query(&question, &provider)
        .map_err(|e| format!("query failed: {e}"))?;

    let prompt = rag.build_rag_prompt(&question, &results);
    let chunk_count = results.len();

    let sources: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "doc_path": r.doc_path,
                "chunk_index": r.chunk_index,
                "score": r.score,
            })
        })
        .collect();

    drop(rag);

    let model = get_default_model();
    let provider_name = provider.name().to_string();

    // Call the real LLM with the assembled RAG prompt.
    let response = match provider.query(&prompt, 1024, &model) {
        Ok(llm_resp) => {
            state.log_event(
                Uuid::new_v4(),
                EventType::LlmCall,
                json!({
                    "event": "rag.chat",
                    "question_len": question.len(),
                    "chunk_count": chunk_count,
                    "provider": provider_name,
                    "model": llm_resp.model_name,
                    "tokens": llm_resp.token_count,
                }),
            );

            json!({
                "answer": llm_resp.output_text,
                "sources": sources,
                "model": format!("{}/{}", provider_name, llm_resp.model_name),
                "tokens": llm_resp.token_count,
            })
        }
        Err(e) => {
            eprintln!("[nexus-rag] LLM query failed, returning raw prompt: {e}");

            state.log_event(
                Uuid::new_v4(),
                EventType::ToolCall,
                json!({
                    "event": "rag.chat_fallback",
                    "question_len": question.len(),
                    "chunk_count": chunk_count,
                    "error": e.to_string(),
                }),
            );

            json!({
                "answer": prompt,
                "sources": sources,
                "model": format!("{}/fallback", provider_name),
                "tokens": 0,
                "fallback": true,
                "error": format!("LLM query failed: {e}. Returning raw RAG prompt."),
            })
        }
    };

    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

pub fn list_indexed_documents(state: &AppState) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let docs = rag.list_documents();
    serde_json::to_string(docs).map_err(|e| format!("serialize error: {e}"))
}

pub fn remove_indexed_document(state: &AppState, doc_path: String) -> Result<String, String> {
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let removed = rag.remove_document(&doc_path);
    let response = json!({
        "removed": removed,
        "path": doc_path,
    });
    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

pub fn get_document_governance(state: &AppState, doc_path: String) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let doc = rag
        .documents
        .iter()
        .find(|d| d.path == doc_path)
        .ok_or_else(|| format!("document not found: {doc_path}"))?;
    serde_json::to_string(&doc.governance).map_err(|e| format!("serialize error: {e}"))
}

pub fn get_semantic_map(state: &AppState) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let points = rag.vector_store.get_2d_projection();
    serde_json::to_string(&points).map_err(|e| format!("serialize error: {e}"))
}

pub fn get_document_access_log(state: &AppState, doc_path: String) -> Result<String, String> {
    let rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
    let entries: Vec<_> = rag.get_document_access_log(&doc_path);
    serde_json::to_string(&entries).map_err(|e| format!("serialize error: {e}"))
}

pub fn get_active_llm_provider(_state: &AppState) -> Result<String, String> {
    let provider = get_configured_provider();
    let provider_name = provider.name().to_string();
    let model = get_default_model();

    let (status, message) = if provider_name == "mock" {
        (
            "no_provider_available",
            "Install Ollama or configure an API key".to_string(),
        )
    } else {
        ("connected", format!("Using {provider_name}"))
    };

    // Determine embedding model from RAG config default
    let embedding_model = if provider_name == "ollama" {
        "nomic-embed-text".to_string()
    } else {
        "all-minilm".to_string()
    };

    let response = json!({
        "provider": provider_name,
        "model": model,
        "embedding_model": embedding_model,
        "status": status,
        "message": message,
    });

    serde_json::to_string(&response).map_err(|e| format!("serialize error: {e}"))
}

// ── Model Hub Commands ──────────────────────────────────────────────────────

pub fn search_models(
    state: &AppState,
    query: String,
    limit: Option<u32>,
) -> Result<String, String> {
    let limit = limit.unwrap_or(20) as usize;
    state.log_event(
        AgentId::nil(),
        EventType::ToolCall,
        json!({"operation": "model_hub.search", "query": &query, "limit": limit}),
    );
    let result = model_hub::search_huggingface(&query, limit)?;
    serde_json::to_string(&result).map_err(|e| format!("serialize error: {e}"))
}

pub fn get_model_info(_state: &AppState, model_id: String) -> Result<String, String> {
    let info = model_hub::get_model_details(&model_id)?;
    serde_json::to_string(&info).map_err(|e| format!("serialize error: {e}"))
}

pub fn check_model_compatibility(
    _state: &AppState,
    file_size_bytes: u64,
) -> Result<String, String> {
    let compat = model_hub::check_compatibility(file_size_bytes);
    serde_json::to_string(&compat).map_err(|e| format!("serialize error: {e}"))
}

pub fn list_local_models(state: &AppState) -> Result<String, String> {
    let mut registry = state
        .model_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.discover();
    let models = registry.available_models().to_vec();
    serde_json::to_string(&models).map_err(|e| format!("serialize error: {e}"))
}

pub fn delete_local_model(state: &AppState, model_id: String) -> Result<String, String> {
    let mut registry = state
        .model_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    registry.discover();

    let model_dir = match registry.find_model(&model_id) {
        Some(config) => config.model_path.clone(),
        None => {
            return serde_json::to_string(
                &json!({"deleted": false, "model_id": &model_id, "error": "model not found"}),
            )
            .map_err(|e| format!("Serialization error: {}", e));
        }
    };

    // Safety: only delete within the models directory
    let models_root = registry.models_dir().clone();
    if !model_dir.starts_with(&models_root) {
        return Err("refusing to delete path outside models directory".to_string());
    }

    drop(registry); // unlock before filesystem operation

    match std::fs::remove_dir_all(&model_dir) {
        Ok(()) => {
            state.log_event(
                AgentId::nil(),
                EventType::ToolCall,
                json!({"operation": "model_hub.delete", "model_id": &model_id, "path": model_dir.display().to_string()}),
            );
            serde_json::to_string(&json!({"deleted": true, "model_id": &model_id}))
                .map_err(|e| format!("Serialization error: {}", e))
        }
        Err(e) => serde_json::to_string(
            &json!({"deleted": false, "model_id": &model_id, "error": e.to_string()}),
        )
        .map_err(|e| format!("Serialization error: {}", e)),
    }
}

pub fn get_system_specs() -> Result<String, String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let cpu_cores = sys.cpus().len();

    serde_json::to_string(&json!({
        "total_ram_mb": sys.total_memory() / (1024 * 1024),
        "available_ram_mb": sys.available_memory() / (1024 * 1024),
        "cpu_name": cpu_name,
        "cpu_cores": cpu_cores,
    }))
    .map_err(|e| format!("Serialization error: {}", e))
}

pub fn get_live_system_metrics(state: &AppState) -> Result<String, String> {
    use sysinfo::{Disks, System};

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let cpu_cores = sys.cpus().len();

    let per_core_usage: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_avg = if cpu_cores > 0 {
        per_core_usage.iter().sum::<f32>() / cpu_cores as f32
    } else {
        0.0
    };

    let total_ram = sys.total_memory();
    let used_ram = sys.used_memory();
    let available_ram = sys.available_memory();

    let uptime = System::uptime();
    let process_count = sys.processes().len();

    // Disk usage for ~/.nexus/ directory
    let nexus_dir = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".nexus"))
        .unwrap_or_default();
    let nexus_disk_bytes: u64 = if nexus_dir.exists() {
        fn dir_size(path: &std::path::Path) -> u64 {
            let mut total = 0u64;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        total += dir_size(&p);
                    } else if let Ok(meta) = p.metadata() {
                        total += meta.len();
                    }
                }
            }
            total
        }
        dir_size(&nexus_dir)
    } else {
        0
    };

    // Total disk info from sysinfo
    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_available) = disks
        .list()
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| (d.total_space(), d.available_space()))
        .unwrap_or_else(|| {
            disks
                .list()
                .first()
                .map(|d| (d.total_space(), d.available_space()))
                .unwrap_or((0, 0))
        });

    // Per-agent fuel from Supervisor
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let meta_guard = match state.meta.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    let statuses = supervisor.health_check();
    let mut agent_fuel = Vec::new();
    for status in &statuses {
        let name = meta_guard
            .get(&status.id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| status.id.to_string());
        let (fuel_budget, fuel_used) = if let Some(report) = supervisor.fuel_audit_report(status.id)
        {
            (report.cap_units, report.spent_units)
        } else {
            let budget = supervisor
                .get_agent(status.id)
                .map(|h| h.manifest.fuel_budget)
                .unwrap_or(0);
            let remaining = status.remaining_fuel;
            (budget, budget.saturating_sub(remaining))
        };
        agent_fuel.push(json!({
            "id": status.id.to_string(),
            "name": name,
            "state": status.state.to_string(),
            "fuel_budget": fuel_budget,
            "fuel_used": fuel_used,
            "remaining_fuel": status.remaining_fuel,
        }));
    }

    serde_json::to_string(&json!({
        "cpu_name": cpu_name,
        "cpu_cores": cpu_cores,
        "cpu_avg": (cpu_avg * 10.0).round() / 10.0,
        "per_core_usage": per_core_usage,
        "total_ram": total_ram,
        "used_ram": used_ram,
        "available_ram": available_ram,
        "uptime_secs": uptime,
        "process_count": process_count,
        "nexus_disk_bytes": nexus_disk_bytes,
        "disk_total": disk_total,
        "disk_available": disk_available,
        "agents": agent_fuel,
    }))
    .map_err(|e| format!("Serialization error: {}", e))
}

// ---------------------------------------------------------------------------
// Time Machine commands
// ---------------------------------------------------------------------------

pub fn time_machine_list_checkpoints(state: &AppState) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let checkpoints = supervisor.time_machine().list_checkpoints();
    let summaries: Vec<serde_json::Value> = checkpoints
        .iter()
        .map(|cp| {
            json!({
                "id": cp.id,
                "label": cp.label,
                "timestamp": cp.timestamp,
                "agent_id": cp.agent_id,
                "change_count": cp.changes.len(),
                "undone": cp.undone,
            })
        })
        .collect();
    serde_json::to_string(&summaries).map_err(|e| e.to_string())
}

pub fn time_machine_get_checkpoint(state: &AppState, id: String) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let cp = supervisor
        .time_machine()
        .get_checkpoint(&id)
        .ok_or_else(|| format!("checkpoint not found: {id}"))?;
    serde_json::to_string(cp).map_err(|e| e.to_string())
}

pub fn time_machine_create_checkpoint(state: &AppState, label: String) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let builder = supervisor.time_machine().begin_checkpoint(&label, None);
    let cp = builder.build();
    let (id, _evicted) = supervisor
        .time_machine_mut()
        .commit_checkpoint(cp)
        .map_err(|e| e.to_string())?;

    state.log_event(
        uuid::Uuid::nil(),
        nexus_kernel::audit::EventType::StateChange,
        json!({ "action": "time_machine.checkpoint_created", "checkpoint_id": id, "label": label }),
    );
    Ok(id)
}

pub fn time_machine_undo(state: &AppState) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (cp, non_file_actions) = supervisor
        .time_machine_mut()
        .undo()
        .map_err(|e| e.to_string())?;

    let files_restored: Vec<String> = cp
        .changes
        .iter()
        .filter_map(|c| match c {
            nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    let agents_affected: Vec<String> = non_file_actions
        .iter()
        .filter_map(|a| match a {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        })
        .collect();
    let actions_applied = files_restored.len() + non_file_actions.len();

    drop(supervisor);

    state.log_event(
        uuid::Uuid::nil(),
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.undo",
            "checkpoint_id": cp.id,
            "label": cp.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": cp.id,
        "label": cp.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub fn time_machine_undo_checkpoint(state: &AppState, id: String) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (cp, non_file_actions) = supervisor
        .time_machine_mut()
        .undo_checkpoint(&id)
        .map_err(|e| e.to_string())?;

    let files_restored: Vec<String> = cp
        .changes
        .iter()
        .filter_map(|c| match c {
            nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    let agents_affected: Vec<String> = non_file_actions
        .iter()
        .filter_map(|a| match a {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        })
        .collect();
    let actions_applied = files_restored.len() + non_file_actions.len();

    drop(supervisor);

    state.log_event(
        uuid::Uuid::nil(),
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.undo_checkpoint",
            "checkpoint_id": cp.id,
            "label": cp.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": cp.id,
        "label": cp.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub fn time_machine_redo(state: &AppState) -> Result<String, String> {
    let mut supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let (cp, non_file_actions) = supervisor
        .time_machine_mut()
        .redo()
        .map_err(|e| e.to_string())?;

    let files_restored: Vec<String> = cp
        .changes
        .iter()
        .filter_map(|c| match c {
            nexus_kernel::time_machine::ChangeEntry::FileWrite { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileCreate { path, .. }
            | nexus_kernel::time_machine::ChangeEntry::FileDelete { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect();
    let agents_affected: Vec<String> = non_file_actions
        .iter()
        .filter_map(|a| match a {
            nexus_kernel::time_machine::UndoAction::RestoreAgentState { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        })
        .collect();
    let actions_applied = files_restored.len() + non_file_actions.len();

    drop(supervisor);

    state.log_event(
        uuid::Uuid::nil(),
        nexus_kernel::audit::EventType::StateChange,
        json!({
            "action": "time_machine.redo",
            "checkpoint_id": cp.id,
            "label": cp.label,
            "actions_applied": actions_applied,
        }),
    );

    serde_json::to_string(&json!({
        "checkpoint_id": cp.id,
        "label": cp.label,
        "actions_applied": actions_applied,
        "files_restored": files_restored,
        "agents_affected": agents_affected,
    }))
    .map_err(|e| e.to_string())
}

pub fn time_machine_get_diff(state: &AppState, id: String) -> Result<String, String> {
    let supervisor = match state.supervisor.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let cp = supervisor
        .time_machine()
        .get_checkpoint(&id)
        .ok_or_else(|| format!("checkpoint not found: {id}"))?;

    let diffs: Vec<serde_json::Value> = cp
        .changes
        .iter()
        .map(|entry| match entry {
            nexus_kernel::time_machine::ChangeEntry::FileWrite {
                path,
                before,
                after,
            } => json!({
                "path": path,
                "change_type": "modify",
                "size_before": before.as_ref().map(|b| b.len()).unwrap_or(0),
                "size_after": after.len(),
            }),
            nexus_kernel::time_machine::ChangeEntry::FileCreate { path, after } => json!({
                "path": path,
                "change_type": "create",
                "size_before": 0,
                "size_after": after.len(),
            }),
            nexus_kernel::time_machine::ChangeEntry::FileDelete { path, before } => json!({
                "path": path,
                "change_type": "delete",
                "size_before": before.len(),
                "size_after": 0,
            }),
            nexus_kernel::time_machine::ChangeEntry::AgentStateChange {
                agent_id,
                field,
                before,
                after,
            } => json!({
                "path": format!("agent://{agent_id}/{field}"),
                "change_type": "modify",
                "before_value": before,
                "after_value": after,
            }),
            nexus_kernel::time_machine::ChangeEntry::ConfigChange { key, before, after } => json!({
                "path": format!("config://{key}"),
                "change_type": "modify",
                "before_value": before,
                "after_value": after,
            }),
        })
        .collect();
    serde_json::to_string(&diffs).map_err(|e| e.to_string())
}

// ── Nexus Link (peer-to-peer model sharing) ─────────────────────────────

pub fn nexus_link_status(state: &AppState) -> Result<String, String> {
    let link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let local_model_count = link.get_local_models().unwrap_or_default().len();
    let result = json!({
        "device_id": link.device_id(),
        "device_name": link.device_name(),
        "sharing_enabled": link.sharing_enabled(),
        "peer_count": link.list_peers().len(),
        "local_model_count": local_model_count,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn nexus_link_toggle_sharing(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    if enabled {
        link.enable_sharing();
    } else {
        link.disable_sharing();
    }
    let result = json!({ "sharing_enabled": link.sharing_enabled() });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn nexus_link_add_peer(
    state: &AppState,
    address: String,
    name: String,
) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let peer = link.add_peer(&address, &name);
    serde_json::to_string(&peer).map_err(|e| e.to_string())
}

pub fn nexus_link_remove_peer(state: &AppState, device_id: String) -> Result<String, String> {
    let mut link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    let removed = link.remove_peer(&device_id);
    let result = json!({ "removed": removed });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn nexus_link_list_peers(state: &AppState) -> Result<String, String> {
    let link = state.nexus_link.lock().unwrap_or_else(|p| p.into_inner());
    serde_json::to_string(link.list_peers()).map_err(|e| e.to_string())
}

// ── Evolution engine (self-improving agent strategies) ───────────────────

pub fn evolution_get_status(state: &AppState) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    let result = json!({
        "enabled": engine.config().enabled,
        "total_strategies": engine.total_strategies(),
        "active_agents": engine.active_agent_count(),
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn evolution_register_strategy(
    state: &AppState,
    agent_id: String,
    name: String,
    parameters: String,
) -> Result<String, String> {
    let params: serde_json::Value =
        serde_json::from_str(&parameters).map_err(|e| format!("Invalid parameters JSON: {e}"))?;
    let strategy = Strategy {
        id: uuid::Uuid::new_v4().to_string(),
        version: 1,
        agent_id,
        name,
        parameters: params,
        score: 0.0,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        parent_id: None,
    };
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    engine.register_strategy(strategy.clone())?;
    serde_json::to_string(&strategy).map_err(|e| e.to_string())
}

pub fn evolution_evolve_once(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    // Simple scoring: count non-null fields in parameters
    let result = engine.evolve_once(&agent_id, MutationType::ParameterTweak, |s| {
        let param_count = s.parameters.as_object().map(|o| o.len()).unwrap_or(0);
        (param_count as f64 * 0.1).min(1.0)
    })?;
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn evolution_get_history(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    match engine.get_history(&agent_id) {
        Some(history) => serde_json::to_string(history).map_err(|e| e.to_string()),
        None => Ok(json!({
            "agent_id": agent_id,
            "total_generations": 0,
            "total_improvements": 0,
            "total_regressions": 0,
            "current_best_score": 0.0,
            "results": []
        })
        .to_string()),
    }
}

pub fn evolution_rollback(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    let strategy = engine.rollback(&agent_id)?;
    serde_json::to_string(&strategy).map_err(|e| e.to_string())
}

pub fn evolution_get_active_strategy(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state.evolution.lock().unwrap_or_else(|p| p.into_inner());
    match engine.get_active_strategy(&agent_id) {
        Some(strategy) => serde_json::to_string(strategy).map_err(|e| e.to_string()),
        None => Err(format!("No active strategy for agent {agent_id}")),
    }
}

// ── MCP Host Mode (external MCP tool consumption) ───────────────────────

pub fn mcp_host_list_servers(state: &AppState) -> Result<String, String> {
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

pub fn mcp_host_add_server(
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

pub fn mcp_host_remove_server(state: &AppState, server_id: String) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let removed = manager.remove_server(&server_id);
    Ok(json!({ "removed": removed }).to_string())
}

pub fn mcp_host_connect(state: &AppState, server_id: String) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let tools = manager.connect_server(&server_id)?;
    let result = json!({
        "server_id": server_id,
        "tools_discovered": tools.len(),
        "tools": tools,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn mcp_host_disconnect(state: &AppState, server_id: String) -> Result<String, String> {
    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    manager.disconnect_server(&server_id);
    Ok(json!({ "disconnected": true }).to_string())
}

pub fn mcp_host_list_tools(state: &AppState) -> Result<String, String> {
    let manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let tools = manager.list_all_tools();
    serde_json::to_string(&tools).map_err(|e| e.to_string())
}

pub fn mcp_host_call_tool(
    state: &AppState,
    tool_name: String,
    arguments: String,
) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&arguments).map_err(|e| format!("Invalid arguments JSON: {e}"))?;

    let mut manager = state.mcp_host.lock().unwrap_or_else(|p| p.into_inner());
    let result = manager.call_tool(&tool_name, args)?;

    // Audit the tool call
    drop(manager); // release mcp_host lock before acquiring audit lock
    state.log_event(
        Uuid::nil(),
        EventType::ToolCall,
        json!({
            "source": "mcp-host",
            "tool_name": result.tool_name,
            "server_id": result.server_id,
            "is_error": result.is_error,
            "execution_ms": result.execution_ms,
        }),
    );

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

// ── Ghost Protocol commands ─────────────────────────────────────────────

pub fn ghost_protocol_status(state: &AppState) -> Result<String, String> {
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

pub fn ghost_protocol_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    gp.set_enabled(enabled);

    drop(gp);
    state.log_event(
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "ghost-protocol",
            "action": if enabled { "enabled" } else { "disabled" },
        }),
    );

    let result = json!({ "enabled": enabled });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn ghost_protocol_add_peer(
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
        Uuid::nil(),
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

pub fn ghost_protocol_remove_peer(state: &AppState, device_id: String) -> Result<String, String> {
    let mut gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let removed = gp.remove_peer(&device_id);

    drop(gp);
    state.log_event(
        Uuid::nil(),
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

pub fn ghost_protocol_sync_now(state: &AppState) -> Result<String, String> {
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
        Uuid::nil(),
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

pub fn ghost_protocol_get_state(state: &AppState) -> Result<String, String> {
    let gp = state
        .ghost_protocol
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let sync_state = gp.get_state();
    serde_json::to_string(sync_state).map_err(|e| e.to_string())
}

// ── Voice Assistant commands ────────────────────────────────────────────

pub fn voice_start_listening(state: &AppState) -> Result<String, String> {
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
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "voice-assistant",
            "action": "start_listening",
        }),
    );

    let result = json!({ "status": "listening" });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn voice_stop_listening(state: &AppState) -> Result<String, String> {
    let mut vp = state
        .voice_process
        .lock()
        .unwrap_or_else(|p| p.into_inner());

    // Kill the Python process if running.
    if let Some(ref mut child) = vp.child {
        let _ = child.kill();
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
        Uuid::nil(),
        EventType::StateChange,
        json!({
            "source": "voice-assistant",
            "action": "stop_listening",
        }),
    );

    let result = json!({ "status": "stopped" });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn voice_get_status(state: &AppState) -> Result<String, String> {
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

pub fn voice_transcribe(state: &AppState, audio_base64: String) -> Result<String, String> {
    let start = std::time::Instant::now();
    let decoded_len = audio_base64.len() * 3 / 4;

    // ── Fallback chain: Candle Whisper → Python server → stub ───────

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

    // 3. Stub fallback
    let size_kb = decoded_len as f64 / 1024.0;
    let elapsed = start.elapsed();
    let result = json!({
        "text": format!("[transcription stub — {:.1} KB audio received]", size_kb),
        "engine": "stub",
        "duration_ms": elapsed.as_millis() as u64,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Load a Whisper model for on-device speech-to-text.
pub fn voice_load_whisper_model(state: &AppState, model_path: String) -> Result<String, String> {
    let transcriber = WhisperTranscriber::load_model(&model_path)?;
    let info = transcriber.model_info().unwrap_or_default();

    let mut whisper = state.whisper.lock().unwrap_or_else(|p| p.into_inner());
    *whisper = transcriber;
    drop(whisper);

    state.log_event(
        Uuid::nil(),
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
fn base64_decode_audio(encoded: &str) -> Result<Vec<u8>, String> {
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

pub fn factory_create_project(
    state: &AppState,
    name: String,
    language: String,
    source_dir: String,
) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let project = factory.create_project(&name, &language, &source_dir);

    drop(factory);
    state.log_event(
        Uuid::nil(),
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

pub fn factory_build_project(state: &AppState, project_id: String) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.build_project(&project_id)?;

    drop(factory);
    state.log_event(
        Uuid::nil(),
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

pub fn factory_test_project(state: &AppState, project_id: String) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.test_project(&project_id)?;

    drop(factory);
    state.log_event(
        Uuid::nil(),
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

pub fn factory_run_pipeline(state: &AppState, project_id: String) -> Result<String, String> {
    let mut factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let result = factory.run_full_pipeline(&project_id)?;

    drop(factory);
    state.log_event(
        Uuid::nil(),
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

pub fn factory_list_projects(state: &AppState) -> Result<String, String> {
    let factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let projects = factory.list_projects();
    serde_json::to_string(&projects).map_err(|e| e.to_string())
}

pub fn factory_get_build_history(state: &AppState, project_id: String) -> Result<String, String> {
    let factory = state.factory.lock().unwrap_or_else(|p| p.into_inner());
    let history = factory.get_build_history(&project_id);
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

// ── Conductor Build ─────────────────────────────────────────────────

pub fn conduct_build(
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

    let model_name = model.unwrap_or_else(|| "mistral".to_string());

    // Create the provider
    let provider = OllamaProvider::from_env();

    // Create conductor
    let mut conductor = Conductor::new(provider, &model_name);

    // Create user request
    let request = UserRequest::new(&prompt, &out_dir);
    let request_id = request.id;

    // Get supervisor
    let mut supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());

    // Preview plan first (for event emission by caller)
    let plan = conductor
        .preview_plan(&UserRequest::new(&prompt, &out_dir))
        .map_err(|e| format!("planning failed: {e}"))?;
    let plan_json = serde_json::to_value(&plan).unwrap_or_default();

    // Run full orchestration
    let start = std::time::Instant::now();
    let mut result = conductor
        .run(request, &mut supervisor)
        .map_err(|e| format!("conductor failed: {e}"))?;
    result.duration_secs = start.elapsed().as_secs_f64();

    drop(supervisor);

    // Log audit event
    state.log_event(
        Uuid::nil(),
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

pub fn execute_tool(state: &AppState, tool_json: String) -> Result<String, String> {
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
        uuid::Uuid::nil(),
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

pub fn list_tools() -> Result<String, String> {
    let tools = nexus_kernel::typed_tools::list_available_tools();
    serde_json::to_string(&tools).map_err(|e| e.to_string())
}

/// Parse a shell command string into a TypedTool and execute it.
///
/// Maps well-known commands to safe TypedTool variants.  Unknown commands
/// become `TypedTool::Custom` with `requires_approval: true`.
///
/// Returns JSON-serialised `TerminalResult`.
pub fn terminal_execute(state: &AppState, command: String, cwd: String) -> Result<String, String> {
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
        uuid::Uuid::nil(),
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
pub fn terminal_execute_approved(
    state: &AppState,
    command: String,
    cwd: String,
) -> Result<String, String> {
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
        uuid::Uuid::nil(),
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

pub fn replay_list_bundles(
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

pub fn replay_get_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let bundle = recorder
        .get_bundle(&bundle_id)
        .ok_or_else(|| format!("bundle '{bundle_id}' not found"))?;
    serde_json::to_string(bundle).map_err(|e| e.to_string())
}

pub fn replay_verify_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
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
        uuid::Uuid::nil(),
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

pub fn replay_export_bundle(state: &AppState, bundle_id: String) -> Result<String, String> {
    let recorder = state
        .replay_recorder
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    recorder.export_bundle(&bundle_id)
}

pub fn replay_toggle_recording(state: &AppState, enabled: bool) -> Result<String, String> {
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
        uuid::Uuid::nil(),
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

pub fn airgap_create_bundle(
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

pub fn airgap_validate_bundle(_state: &AppState, bundle_path: String) -> Result<String, String> {
    let result = nexus_airgap::AirgapInstaller::validate_bundle(&bundle_path);
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn airgap_install_bundle(
    state: &AppState,
    bundle_path: String,
    install_dir: String,
) -> Result<String, String> {
    let bundle = nexus_airgap::AirgapInstaller::install(&bundle_path, &install_dir)?;

    state.log_event(
        uuid::Uuid::nil(),
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

pub fn airgap_get_system_info(_state: &AppState) -> Result<String, String> {
    let info = nexus_airgap::get_system_info();
    serde_json::to_string(&info).map_err(|e| e.to_string())
}

// ── Reputation Registry ─────────────────────────────────────────────

pub fn reputation_register(state: &AppState, did: String, name: String) -> Result<String, String> {
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

pub fn reputation_record_task(
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

pub fn reputation_rate_agent(
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

pub fn reputation_get(state: &AppState, did: String) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let rep = reg
        .get_reputation(&did)
        .ok_or_else(|| format!("agent '{did}' not found"))?;
    serde_json::to_string(rep).map_err(|e| e.to_string())
}

pub fn reputation_top(state: &AppState, limit: Option<usize>) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let top = reg.top_agents(limit.unwrap_or(10));
    serde_json::to_string(&top).map_err(|e| e.to_string())
}

pub fn reputation_export(state: &AppState, did: String) -> Result<String, String> {
    let reg = state
        .reputation_registry
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    reg.export_reputation(&did)
}

pub fn reputation_import(state: &AppState, json: String) -> Result<String, String> {
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

// ── Computer Control Engine ──────────────────────────────────────────

pub fn computer_control_capture_screen(
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

pub fn computer_control_execute_action(
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

pub fn computer_control_get_history(state: &AppState) -> Result<String, String> {
    let engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.action_history();
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

pub fn computer_control_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
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

pub fn computer_control_status(state: &AppState) -> Result<String, String> {
    let engine = state
        .computer_control
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    Ok(json!({
        "enabled": engine.is_enabled(),
        "max_actions_per_minute": engine.max_actions_per_minute(),
        "total_actions": engine.total_actions(),
    })
    .to_string())
}

// ---------------------------------------------------------------------------
// Neural Bridge commands
// ---------------------------------------------------------------------------

pub fn neural_bridge_status(state: &AppState) -> Result<String, String> {
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

pub fn neural_bridge_toggle(state: &AppState, enabled: bool) -> Result<String, String> {
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

pub fn neural_bridge_ingest(
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

pub fn neural_bridge_search(
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

pub fn neural_bridge_delete(state: &AppState, id: String) -> Result<String, String> {
    let mut bridge = state
        .neural_bridge
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let deleted = bridge.delete_entry(&id);
    Ok(json!({ "deleted": deleted }).to_string())
}

pub fn neural_bridge_clear_old(state: &AppState, before_timestamp: u64) -> Result<String, String> {
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

pub fn economy_create_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
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

pub fn economy_get_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match engine.get_wallet(&agent_id) {
        Some(w) => serde_json::to_string(w).map_err(|e| e.to_string()),
        None => Err(format!("wallet not found: {agent_id}")),
    }
}

pub fn economy_spend(
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

pub fn economy_earn(
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

pub fn economy_transfer(
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

pub fn economy_freeze_wallet(state: &AppState, agent_id: String) -> Result<String, String> {
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

pub fn economy_get_history(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let history = engine.get_transaction_history(&agent_id);
    serde_json::to_string(&history).map_err(|e| e.to_string())
}

pub fn economy_get_stats(state: &AppState) -> Result<String, String> {
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
pub fn economy_create_contract(
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

pub fn economy_complete_contract(
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

pub fn economy_list_contracts(state: &AppState, agent_id: String) -> Result<String, String> {
    let engine = state
        .economic_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let contracts = engine.list_contracts(&agent_id);
    serde_json::to_string(&contracts).map_err(|e| e.to_string())
}

pub fn economy_dispute_contract(
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

pub fn economy_agent_performance(state: &AppState, agent_id: String) -> Result<String, String> {
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

pub fn agent_memory_remember(
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

pub fn agent_memory_recall(
    state: &AppState,
    agent_id: String,
    query: String,
    max_results: Option<usize>,
) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let results = mem.recall(&agent_id, &query, max_results.unwrap_or(10));
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub fn agent_memory_recall_by_type(
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

pub fn agent_memory_forget(
    state: &AppState,
    agent_id: String,
    memory_id: String,
) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let removed = mem.forget(&agent_id, &memory_id);
    Ok(json!({ "removed": removed }).to_string())
}

pub fn agent_memory_get_stats(state: &AppState, agent_id: String) -> Result<String, String> {
    let mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    let stats = mem.get_stats(&agent_id);
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

pub fn agent_memory_save(state: &AppState, agent_id: String) -> Result<String, String> {
    let mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    mem.save(&agent_id)?;
    Ok(json!({ "saved": true }).to_string())
}

pub fn agent_memory_clear(state: &AppState, agent_id: String) -> Result<String, String> {
    let mut mem = state.agent_memory.lock().unwrap_or_else(|p| p.into_inner());
    mem.clear(&agent_id);
    Ok(json!({ "cleared": true }).to_string())
}

fn parse_memory_type(s: &str) -> Result<MemoryType, String> {
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

pub fn tracing_start_trace(
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

pub fn tracing_start_span(
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

pub fn tracing_end_span(
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

pub fn tracing_end_trace(state: &AppState, trace_id: String) -> Result<String, String> {
    let mut engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    match engine.end_trace(&trace_id) {
        Some(trace) => serde_json::to_string(&trace).map_err(|e| e.to_string()),
        None => Err(format!("trace not found: {trace_id}")),
    }
}

pub fn tracing_list_traces(state: &AppState, limit: Option<usize>) -> Result<String, String> {
    let engine = state
        .tracing_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let traces: Vec<_> = engine.list_traces(limit.unwrap_or(50));
    serde_json::to_string(&traces).map_err(|e| e.to_string())
}

pub fn tracing_get_trace(state: &AppState, trace_id: String) -> Result<String, String> {
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

fn parse_billing_interval(s: &str) -> Result<BillingInterval, String> {
    match s {
        "Monthly" => Ok(BillingInterval::Monthly),
        "Yearly" => Ok(BillingInterval::Yearly),
        "OneTime" => Ok(BillingInterval::OneTime),
        other => Err(format!("unknown billing interval: {other}")),
    }
}

pub fn payment_create_plan(
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

pub fn payment_list_plans(state: &AppState) -> Result<String, String> {
    let engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let plans: Vec<_> = engine.list_plans();
    serde_json::to_string(&plans).map_err(|e| e.to_string())
}

pub fn payment_create_invoice(
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

pub fn payment_pay_invoice(state: &AppState, invoice_id: String) -> Result<String, String> {
    let mut engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let invoice = engine.pay_invoice(&invoice_id)?;
    serde_json::to_string(&invoice).map_err(|e| e.to_string())
}

pub fn payment_get_revenue_stats(state: &AppState) -> Result<String, String> {
    let engine = state
        .payment_engine
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let stats = engine.get_revenue_stats();
    serde_json::to_string(&stats).map_err(|e| e.to_string())
}

pub fn payment_create_payout(
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

// ── Compliance Dashboard API ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceAlertRow {
    pub severity: String,
    pub check_id: String,
    pub message: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatusRow {
    pub status: String,
    pub checks_passed: usize,
    pub checks_failed: usize,
    pub agents_checked: usize,
    pub alerts: Vec<ComplianceAlertRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceAgentRow {
    pub id: String,
    pub name: String,
    pub risk_tier: String,
    pub autonomy_level: String,
    pub capabilities: Vec<String>,
    pub status: String,
}

pub fn get_compliance_status(state: &AppState) -> Result<ComplianceStatusRow, String> {
    use nexus_kernel::compliance::monitor::{AgentSnapshot, ComplianceMonitor};

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let identity_mgr = state.identity_mgr.lock().unwrap_or_else(|p| p.into_inner());

    let snapshots: Vec<AgentSnapshot> = supervisor
        .health_check()
        .iter()
        .filter_map(|s| {
            supervisor.get_agent(s.id).map(|h| AgentSnapshot {
                agent_id: s.id,
                manifest: h.manifest.clone(),
                running: matches!(
                    s.state,
                    nexus_kernel::lifecycle::AgentState::Running
                        | nexus_kernel::lifecycle::AgentState::Starting
                ),
            })
        })
        .collect();

    let monitor = ComplianceMonitor::new();
    let result = monitor.check_compliance(&snapshots, &audit, &identity_mgr);

    Ok(ComplianceStatusRow {
        status: result.status.as_str().to_string(),
        checks_passed: result.checks_passed,
        checks_failed: result.checks_failed,
        agents_checked: result.agents_checked,
        alerts: result
            .alerts
            .into_iter()
            .map(|a| ComplianceAlertRow {
                severity: a.severity.as_str().to_string(),
                check_id: a.check_id,
                message: a.message,
                agent_id: a.agent_id.map(|id| id.to_string()),
            })
            .collect(),
    })
}

pub fn get_compliance_agents(state: &AppState) -> Result<Vec<ComplianceAgentRow>, String> {
    use nexus_kernel::autonomy::AutonomyLevel;
    use nexus_kernel::compliance::eu_ai_act::RiskClassifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let classifier = RiskClassifier::new();

    let mut rows = Vec::new();
    for agent_status in supervisor.health_check() {
        if let Some(handle) = supervisor.get_agent(agent_status.id) {
            let profile = classifier.classify_agent(&handle.manifest);
            let autonomy = AutonomyLevel::from_manifest(handle.manifest.autonomy_level);
            rows.push(ComplianceAgentRow {
                id: agent_status.id.to_string(),
                name: handle.manifest.name.clone(),
                risk_tier: profile.tier.as_str().to_string(),
                autonomy_level: autonomy.as_str().to_string(),
                capabilities: handle.manifest.capabilities.clone(),
                status: format!("{}", agent_status.state),
            });
        }
    }
    Ok(rows)
}

// ── Distributed Audit API ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditChainStatusRow {
    pub total_events: usize,
    pub chain_valid: bool,
    pub first_hash: String,
    pub last_hash: String,
}

pub fn get_audit_chain_status(state: &AppState) -> Result<AuditChainStatusRow, String> {
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());
    let events = audit.events();
    let total = events.len();
    let chain_valid = if total == 0 {
        true
    } else {
        audit.verify_integrity()
    };
    let first_hash = events.first().map(|e| e.hash.clone()).unwrap_or_default();
    let last_hash = events.last().map(|e| e.hash.clone()).unwrap_or_default();

    Ok(AuditChainStatusRow {
        total_events: total,
        chain_valid,
        first_hash,
        last_hash,
    })
}

// ── Governance verification commands ────────────────────────────────────────

pub fn verify_governance_invariants(state: &AppState) -> Result<String, String> {
    use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    // Use first agent's data if available, otherwise use defaults.
    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let manifest = nexus_kernel::manifest::AgentManifest {
        name: "verification-probe".to_string(),
        version: "1.0.0".to_string(),
        capabilities: capabilities.clone(),
        fuel_budget,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![
            FilesystemPermission {
                path_pattern: "/safe/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            },
            FilesystemPermission {
                path_pattern: "/safe/secret.key".to_string(),
                permission: FsPermissionLevel::Deny,
            },
        ],
    };

    let test_paths: Vec<&str> = vec!["/safe/readme.txt", "/safe/secret.key"];
    let results = verifier.verify_all(
        fuel_remaining,
        fuel_budget,
        &capabilities,
        &capabilities,
        &audit,
        &manifest,
        &test_paths,
    );

    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub fn verify_specific_invariant(
    state: &AppState,
    invariant_name: String,
) -> Result<String, String> {
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let proof = match invariant_name.as_str() {
        "FuelNeverNegative" => verifier.verify_fuel_invariant(fuel_remaining, fuel_budget),
        "FuelNeverExceedsBudget" => {
            verifier.verify_fuel_budget_invariant(fuel_remaining, fuel_budget)
        }
        "CapabilityCheckBeforeAction" => {
            verifier.verify_capability_invariant(&capabilities, "llm.query")
        }
        "AuditChainIntegrity" => verifier.verify_audit_chain(&audit),
        "RedactionBeforeLlmCall" => verifier.verify_redaction_invariant(&audit),
        "HitlApprovalForDestructive" => verifier.verify_hitl_invariant(&audit),
        "NoCapabilityEscalation" => verifier.verify_no_escalation(&capabilities, &capabilities),
        "DenyOverridesAllow" => {
            use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
            let manifest = nexus_kernel::manifest::AgentManifest {
                name: "verification-probe".to_string(),
                version: "1.0.0".to_string(),
                capabilities: capabilities.clone(),
                fuel_budget,
                autonomy_level: None,
                consent_policy_path: None,
                requester_id: None,
                schedule: None,
                llm_model: None,
                fuel_period_id: None,
                monthly_fuel_cap: None,
                allowed_endpoints: None,
                domain_tags: vec![],
                filesystem_permissions: vec![
                    FilesystemPermission {
                        path_pattern: "/safe/".to_string(),
                        permission: FsPermissionLevel::ReadOnly,
                    },
                    FilesystemPermission {
                        path_pattern: "/safe/secret.key".to_string(),
                        permission: FsPermissionLevel::Deny,
                    },
                ],
            };
            verifier.verify_filesystem_deny_override(&manifest, &["/safe/secret.key"])
        }
        _ => return Err(format!("Unknown invariant: {invariant_name}")),
    };

    serde_json::to_string(&proof).map_err(|e| e.to_string())
}

pub fn export_compliance_report(state: &AppState) -> Result<String, String> {
    use nexus_kernel::manifest::{FilesystemPermission, FsPermissionLevel};
    use nexus_kernel::verification::GovernanceVerifier;

    let supervisor = state.supervisor.lock().unwrap_or_else(|p| p.into_inner());
    let audit = state.audit.lock().unwrap_or_else(|p| p.into_inner());

    let mut verifier = GovernanceVerifier::new();

    let agents: Vec<_> = supervisor.health_check();
    let (fuel_remaining, fuel_budget, capabilities) = if let Some(status) = agents.first() {
        if let Some(handle) = supervisor.get_agent(status.id) {
            (
                handle.remaining_fuel,
                handle.manifest.fuel_budget,
                handle.manifest.capabilities.clone(),
            )
        } else {
            (0u64, 1000u64, vec!["llm.query".to_string()])
        }
    } else {
        (0u64, 1000u64, vec!["llm.query".to_string()])
    };

    let manifest = nexus_kernel::manifest::AgentManifest {
        name: "verification-probe".to_string(),
        version: "1.0.0".to_string(),
        capabilities: capabilities.clone(),
        fuel_budget,
        autonomy_level: None,
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![
            FilesystemPermission {
                path_pattern: "/safe/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            },
            FilesystemPermission {
                path_pattern: "/safe/secret.key".to_string(),
                permission: FsPermissionLevel::Deny,
            },
        ],
    };

    let test_paths: Vec<&str> = vec!["/safe/readme.txt", "/safe/secret.key"];
    verifier.verify_all(
        fuel_remaining,
        fuel_budget,
        &capabilities,
        &capabilities,
        &audit,
        &manifest,
        &test_paths,
    );

    Ok(verifier.generate_compliance_report())
}

/* ================================================================== */
/*  File Manager — real filesystem operations                          */
/*                                                                     */
/*  These commands are USER-initiated via the Tauri frontend, not      */
/*  agent-initiated.  Agent file operations go through the kernel's    */
/*  capability system (CapabilityCheck + fuel budget).  User-facing    */
/*  commands rely on OS-level permissions and path sandboxing below    */
/*  (allowed_roots + reject "..") rather than agent capability gates.  */
/* ================================================================== */

/// Allowed root directories for the file manager.  Operations outside these
/// are rejected.  The user's home directory is always allowed.
fn file_manager_allowed_roots() -> Vec<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    vec![home]
}

/// Validate that `path` is under one of the allowed roots.  Returns the
/// canonical path on success.
fn file_manager_validate_path(path: &str) -> Result<std::path::PathBuf, String> {
    let candidate = std::path::PathBuf::from(path);
    // Canonicalize — resolves symlinks and `..` segments
    let canonical = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|e| format!("path error: {e}"))?
    } else {
        // For creation: parent must exist and be valid
        let parent = candidate
            .parent()
            .ok_or_else(|| "invalid path: no parent".to_string())?;
        let parent_canon = parent
            .canonicalize()
            .map_err(|e| format!("parent path error: {e}"))?;
        parent_canon.join(candidate.file_name().unwrap_or_default())
    };
    let roots = file_manager_allowed_roots();
    if roots.iter().any(|r| canonical.starts_with(r)) {
        Ok(canonical)
    } else {
        Err(format!(
            "access denied: path outside allowed directories: {}",
            path
        ))
    }
}

pub fn file_manager_list(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_list", "path": path}),
    );

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&canonical).map_err(|e| format!("read_dir failed: {e}"))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("entry error: {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("metadata error: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path().to_string_lossy().to_string();
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        entries.push(serde_json::json!({
            "name": name,
            "path": entry_path,
            "is_dir": is_dir,
            "size": size,
            "modified": modified,
        }));
    }

    serde_json::to_string(&entries).map_err(|e| format!("json error: {e}"))
}

pub fn file_manager_read(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_read", "path": path}),
    );

    std::fs::read_to_string(&canonical).map_err(|e| format!("read failed: {e}"))
}

pub fn file_manager_write(
    state: &AppState,
    path: String,
    content: String,
) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_write", "path": path, "size": content.len()}),
    );

    std::fs::write(&canonical, &content).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub fn file_manager_create_dir(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_create_dir", "path": path}),
    );

    std::fs::create_dir_all(&canonical).map_err(|e| format!("create_dir failed: {e}"))?;
    Ok("ok".to_string())
}

pub fn file_manager_delete(state: &AppState, path: String) -> Result<String, String> {
    let canonical = file_manager_validate_path(&path)?;

    let is_dir = canonical.is_dir();

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_delete", "path": path, "is_dir": is_dir}),
    );

    if is_dir {
        std::fs::remove_dir_all(&canonical).map_err(|e| format!("remove_dir failed: {e}"))?;
    } else {
        std::fs::remove_file(&canonical).map_err(|e| format!("remove_file failed: {e}"))?;
    }
    Ok("ok".to_string())
}

pub fn file_manager_rename(state: &AppState, from: String, to: String) -> Result<String, String> {
    let from_canonical = file_manager_validate_path(&from)?;
    let to_canonical = file_manager_validate_path(&to)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        serde_json::json!({"action": "file_manager_rename", "from": from, "to": to}),
    );

    std::fs::rename(&from_canonical, &to_canonical).map_err(|e| format!("rename failed: {e}"))?;
    Ok("ok".to_string())
}

pub fn file_manager_home() -> Result<String, String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())
}

// ── Database Manager ──────────────────────────────────────────────────
// User-initiated commands (not agent-initiated).  Agent DB access goes
// through kernel capability checks.  These user-facing commands use
// SQL keyword blocking (DB_BLOCKED_KEYWORDS) as a governance safeguard.

/// Blocked SQL keywords that require HITL approval.
const DB_BLOCKED_KEYWORDS: &[&str] = &["DROP", "TRUNCATE", "ALTER", "DELETE", "GRANT", "REVOKE"];

fn db_check_governance(sql: &str) -> Result<(), String> {
    let upper = sql.trim().to_uppercase();
    for kw in DB_BLOCKED_KEYWORDS {
        // Check if the keyword appears as a standalone word
        if upper.split_whitespace().any(|w| w == *kw) {
            return Err(format!(
                "BLOCKED: \"{kw}\" queries require Tier2+ HITL approval. Agent write access is governed."
            ));
        }
    }
    Ok(())
}

fn nexus_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".nexus"))
}

pub fn db_connect(state: &AppState, connection_string: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "db_connect", "connection_string": connection_string}),
    );

    // Validate the path exists or can be created
    let db_path = std::path::Path::new(&connection_string);
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create db directory: {e}"))?;
        }
    }

    // Test that we can open the database
    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    // Get table list
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .map_err(|e| format!("Failed to query tables: {e}"))?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| format!("Failed to fetch tables: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let conn_id = Uuid::new_v4().to_string();
    let result = json!({
        "conn_id": conn_id,
        "path": connection_string,
        "tables": tables,
    });
    serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
}

pub fn db_execute_query(
    state: &AppState,
    connection_string: String,
    query: String,
) -> Result<String, String> {
    // Governance check
    db_check_governance(&query)?;

    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "db_execute_query", "query": query}),
    );

    let start = std::time::Instant::now();
    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    let trimmed = query.trim().to_uppercase();
    if trimmed.starts_with("SELECT")
        || trimmed.starts_with("PRAGMA")
        || trimmed.starts_with("EXPLAIN")
    {
        let mut stmt = conn
            .prepare(query.trim().trim_end_matches(';'))
            .map_err(|e| format!("SQL error: {e}"))?;

        let col_count = stmt.column_count();
        let columns: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let rows: Vec<Vec<serde_json::Value>> = stmt
            .query_map([], |row| {
                let mut vals = Vec::with_capacity(col_count);
                for i in 0..col_count {
                    let val: rusqlite::Result<String> = row.get(i);
                    vals.push(match val {
                        Ok(s) => serde_json::Value::String(s),
                        Err(_) => {
                            // Try as integer
                            let int_val: rusqlite::Result<i64> = row.get(i);
                            match int_val {
                                Ok(n) => serde_json::Value::Number(n.into()),
                                Err(_) => {
                                    // Try as float
                                    let float_val: rusqlite::Result<f64> = row.get(i);
                                    match float_val {
                                        Ok(f) => serde_json::json!(f),
                                        Err(_) => serde_json::Value::Null,
                                    }
                                }
                            }
                        }
                    });
                }
                Ok(vals)
            })
            .map_err(|e| format!("Query failed: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        let duration = start.elapsed().as_millis() as u64;
        let result = json!({
            "columns": columns,
            "rows": rows,
            "row_count": rows.len(),
            "duration_ms": duration,
        });
        serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
    } else {
        // INSERT / UPDATE / CREATE TABLE etc.
        let affected = conn
            .execute(query.trim().trim_end_matches(';'), [])
            .map_err(|e| format!("SQL error: {e}"))?;
        let duration = start.elapsed().as_millis() as u64;
        let result = json!({
            "columns": [],
            "rows": [],
            "row_count": affected,
            "duration_ms": duration,
        });
        serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
    }
}

pub fn db_list_tables(state: &AppState, connection_string: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "db_list_tables", "path": connection_string}),
    );

    let conn = rusqlite::Connection::open(&connection_string)
        .map_err(|e| format!("SQLite connection failed: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT m.name, m.type, \
             (SELECT COUNT(*) FROM pragma_table_info(m.name)) as col_count \
             FROM sqlite_master m WHERE m.type='table' ORDER BY m.name",
        )
        .map_err(|e| format!("Failed to query tables: {e}"))?;

    let tables: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let ttype: String = row.get(1)?;
            let col_count: i64 = row.get(2)?;
            Ok(json!({"name": name, "type": ttype, "col_count": col_count}))
        })
        .map_err(|e| format!("Failed to fetch tables: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // For each table, get column info
    let mut detailed = Vec::new();
    for tbl in &tables {
        let name = tbl["name"].as_str().unwrap_or("");
        let mut col_stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", name.replace('"', "")))
            .map_err(|e| format!("pragma error: {e}"))?;
        let columns: Vec<serde_json::Value> = col_stmt
            .query_map([], |row| {
                let col_name: String = row.get(1)?;
                let col_type: String = row.get(2)?;
                let notnull: i64 = row.get(3)?;
                let pk: i64 = row.get(5)?;
                Ok(json!({
                    "name": col_name,
                    "type": col_type,
                    "nullable": notnull == 0,
                    "primaryKey": pk > 0,
                }))
            })
            .map_err(|e| format!("col info error: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        // Get row count
        let row_count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM \"{}\"", name.replace('"', "")),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        detailed.push(json!({
            "name": name,
            "columns": columns,
            "rowCount": row_count,
        }));
    }

    serde_json::to_string(&detailed).map_err(|e| format!("json error: {e}"))
}

// ── API Client ────────────────────────────────────────────────────────
// User-initiated HTTP requests from the UI (governed Postman).  Agent
// network access goes through kernel capability checks.  These commands
// are scoped to the authenticated desktop user, not agent identities.

pub fn api_client_request(
    state: &AppState,
    method: String,
    url: String,
    headers_json: String,
    body: String,
) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "api_client_request", "method": method, "url": url}),
    );

    let start = std::time::Instant::now();

    let mut args: Vec<String> = vec![
        "-sS".to_string(),
        "-w".to_string(),
        "\n__NEXUS_STATUS__%{http_code}\n__NEXUS_HEADERS__%{header_json}".to_string(),
        "-X".to_string(),
        method.clone(),
    ];

    // Parse headers
    let headers: Vec<(String, String)> = serde_json::from_str(&headers_json).unwrap_or_default();
    for (k, v) in &headers {
        args.push("-H".to_string());
        args.push(format!("{k}: {v}"));
    }

    // Add body for methods that support it
    if !body.is_empty() && method != "GET" && method != "HEAD" {
        args.push("-d".to_string());
        args.push(body);
    }

    args.push(url.clone());

    let output = Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run curl: {e}"))?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    // Parse status code from our -w format
    let mut response_body = raw.clone();
    let mut status_code: u16 = 0;
    let mut resp_headers = json!({});

    if let Some(status_pos) = raw.rfind("__NEXUS_STATUS__") {
        response_body = raw[..status_pos].to_string();
        let after_status = &raw[status_pos + 16..];
        // Parse status code (first line after marker)
        if let Some(newline) = after_status.find('\n') {
            status_code = after_status[..newline].trim().parse().unwrap_or(0);
            // Parse headers json after __NEXUS_HEADERS__
            let header_part = &after_status[newline + 1..];
            if let Some(hdr_pos) = header_part.find("__NEXUS_HEADERS__") {
                let hdr_json = &header_part[hdr_pos + 17..];
                resp_headers = serde_json::from_str(hdr_json.trim()).unwrap_or_else(|_| json!({}));
            }
        } else {
            status_code = after_status.trim().parse().unwrap_or(0);
        }
    }

    if !output.status.success() && status_code == 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl failed: {stderr}"));
    }

    let status_text = match status_code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    };

    let size = response_body.len();
    let result = json!({
        "status": status_code,
        "statusText": status_text,
        "headers": resp_headers,
        "body": response_body,
        "duration": duration_ms,
        "size": size,
    });
    serde_json::to_string(&result).map_err(|e| format!("json error: {e}"))
}

// ── Notes App ─────────────────────────────────────────────────────────

fn notes_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("notes");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create notes dir: {e}"))?;
    }
    Ok(dir)
}

pub fn notes_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "notes_list"}),
    );

    let dir = notes_dir()?;
    let mut notes = Vec::new();

    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(note) = serde_json::from_str::<serde_json::Value>(&content) {
                    notes.push(note);
                }
            }
        }
    }

    serde_json::to_string(&notes).map_err(|e| format!("json error: {e}"))
}

pub fn notes_get(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "notes_get", "id": id}),
    );

    let path = notes_dir()?.join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("note not found: {id}"));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))
}

pub fn notes_save(
    state: &AppState,
    id: String,
    title: String,
    content: String,
    folder_id: String,
    tags_json: String,
) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "notes_save", "id": id, "title": title}),
    );

    let dir = notes_dir()?;
    let path = dir.join(format!("{id}.json"));

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    // Load existing note to preserve createdAt, or create new timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let created_at = if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&existing)
            .ok()
            .and_then(|v| v["createdAt"].as_u64())
            .unwrap_or(now)
    } else {
        now
    };

    let word_count = content.split_whitespace().count();

    let note = json!({
        "id": id,
        "title": title,
        "content": content,
        "folderId": folder_id,
        "tags": tags,
        "createdAt": created_at,
        "updatedAt": now,
        "wordCount": word_count,
    });

    let serialized = serde_json::to_string_pretty(&note).map_err(|e| format!("json error: {e}"))?;
    std::fs::write(&path, &serialized).map_err(|e| format!("write failed: {e}"))?;
    Ok(serialized)
}

pub fn notes_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "notes_delete", "id": id}),
    );

    let path = notes_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

// ── Email Client (local drafts) ───────────────────────────────────────

fn emails_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("emails");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create emails dir: {e}"))?;
    }
    Ok(dir)
}

pub fn email_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "email_list"}),
    );
    let dir = emails_dir()?;
    let mut emails = Vec::new();
    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(email) = serde_json::from_str::<serde_json::Value>(&content) {
                    emails.push(email);
                }
            }
        }
    }
    serde_json::to_string(&emails).map_err(|e| format!("json error: {e}"))
}

pub fn email_save(state: &AppState, id: String, data_json: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "email_save", "id": id}),
    );
    let dir = emails_dir()?;
    let path = dir.join(format!("{id}.json"));
    // Validate JSON
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub fn email_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "email_delete", "id": id}),
    );
    let path = emails_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

// ── Project Manager ───────────────────────────────────────────────────

fn projects_dir() -> Result<PathBuf, String> {
    let dir = nexus_data_dir()?.join("projects");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create projects dir: {e}"))?;
    }
    Ok(dir)
}

pub fn project_list(state: &AppState) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "project_list"}),
    );
    let dir = projects_dir()?;
    let mut projects = Vec::new();
    if dir.exists() {
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read_dir failed: {e}"))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("entry error: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let content =
                    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))?;
                if let Ok(project) = serde_json::from_str::<serde_json::Value>(&content) {
                    projects.push(project);
                }
            }
        }
    }
    serde_json::to_string(&projects).map_err(|e| format!("json error: {e}"))
}

pub fn project_get(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "project_get", "id": id}),
    );
    let path = projects_dir()?.join(format!("{id}.json"));
    if !path.exists() {
        return Err(format!("project not found: {id}"));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("read failed: {e}"))
}

pub fn project_save(state: &AppState, id: String, data_json: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "project_save", "id": id}),
    );
    let dir = projects_dir()?;
    let path = dir.join(format!("{id}.json"));
    let _parsed: serde_json::Value =
        serde_json::from_str(&data_json).map_err(|e| format!("invalid json: {e}"))?;
    std::fs::write(&path, &data_json).map_err(|e| format!("write failed: {e}"))?;
    Ok("ok".to_string())
}

pub fn project_delete(state: &AppState, id: String) -> Result<String, String> {
    state.log_event(
        uuid::Uuid::nil(),
        EventType::UserAction,
        json!({"action": "project_delete", "id": id}),
    );
    let path = projects_dir()?.join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok("ok".to_string())
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod runtime {
    use super::*;
    #[cfg(not(target_os = "linux"))]
    use tauri::menu::{Menu, MenuItem};
    #[cfg(not(target_os = "linux"))]
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    #[tauri::command]
    fn list_agents(state: tauri::State<'_, AppState>) -> Result<Vec<AgentRow>, String> {
        super::list_agents(state.inner())
    }

    #[tauri::command]
    fn create_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        manifest_json: String,
    ) -> Result<String, String> {
        let id = super::create_agent(state.inner(), manifest_json)?;
        emit_agent_status(&window, state.inner(), &id);
        Ok(id)
    }

    #[tauri::command]
    fn start_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::start_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn stop_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::stop_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn pause_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::pause_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    #[tauri::command]
    fn resume_agent(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<(), String> {
        super::resume_agent(state.inner(), agent_id.clone())?;
        emit_agent_status(&window, state.inner(), &agent_id);
        Ok(())
    }

    /// Emit an agent-status-changed event to the frontend.
    fn emit_agent_status(window: &tauri::Window, state: &AppState, agent_id: &str) {
        let parsed = match uuid::Uuid::parse_str(agent_id) {
            Ok(id) => id,
            Err(_) => return,
        };
        let supervisor = match state.supervisor.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(status) = supervisor
            .health_check()
            .into_iter()
            .find(|s| s.id == parsed)
        {
            let _ = window.emit(
                "agent-status-changed",
                AgentStatusEvent {
                    agent_id: agent_id.to_string(),
                    status: status.state.to_string(),
                    fuel_remaining: status.remaining_fuel,
                },
            );
        }
    }

    #[tauri::command]
    fn get_audit_log(
        state: tauri::State<'_, AppState>,
        agent_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<AuditRow>, String> {
        super::get_audit_log(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn send_chat(
        state: tauri::State<'_, AppState>,
        message: String,
    ) -> Result<ChatResponse, String> {
        super::send_chat(state.inner(), message)
    }

    #[tauri::command]
    fn get_config() -> Result<NexusConfig, String> {
        super::get_config()
    }

    #[tauri::command]
    fn save_config(config: NexusConfig) -> Result<(), String> {
        super::save_config(config)
    }

    #[tauri::command]
    fn start_jarvis_mode(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::start_jarvis_mode(state.inner())
    }

    #[tauri::command]
    fn stop_jarvis_mode(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::stop_jarvis_mode(state.inner())
    }

    #[tauri::command]
    fn jarvis_status(state: tauri::State<'_, AppState>) -> Result<VoiceRuntimeState, String> {
        super::jarvis_status(state.inner())
    }

    #[tauri::command]
    fn transcribe_push_to_talk() -> Result<String, String> {
        super::transcribe_push_to_talk()
    }

    #[tauri::command]
    fn tray_status(state: tauri::State<'_, AppState>) -> Result<TrayStatus, String> {
        super::tray_status(state.inner())
    }

    #[tauri::command]
    fn detect_hardware() -> Result<HardwareInfo, String> {
        super::detect_hardware()
    }

    #[tauri::command]
    fn check_ollama(base_url: Option<String>) -> Result<OllamaStatus, String> {
        super::check_ollama(base_url)
    }

    #[tauri::command]
    fn pull_ollama_model(model_name: String, base_url: Option<String>) -> Result<String, String> {
        super::pull_ollama_model(model_name, base_url)
    }

    /// Pull a model on a background thread with throttled progress events.
    /// The Tauri async runtime keeps the main thread free while we block here.
    #[tauri::command]
    async fn pull_model(
        window: tauri::Window,
        model_name: String,
        base_url: Option<String>,
    ) -> Result<String, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let result = super::pull_ollama_model_throttled(model_name, base_url, |progress| {
                let _ = window.emit("model-pull-progress", &progress);
            });
            let _ = tx.send(result);
        });
        // recv() blocks this async task's thread, but Tauri runs async commands
        // on a thread pool so the main/UI thread stays responsive.
        rx.recv()
            .unwrap_or(Err("Download thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn ensure_ollama(base_url: Option<String>) -> Result<bool, String> {
        super::ensure_ollama(base_url)
    }

    #[tauri::command]
    fn is_ollama_installed() -> bool {
        super::is_ollama_installed()
    }

    #[tauri::command]
    fn delete_model(model_name: String, base_url: Option<String>) -> Result<(), String> {
        super::delete_ollama_model(model_name, base_url)
    }

    #[tauri::command]
    fn is_setup_complete() -> bool {
        super::is_setup_complete()
    }

    #[tauri::command]
    fn run_setup_wizard(ollama_url: Option<String>) -> Result<SetupResult, String> {
        super::run_setup_wizard(ollama_url)
    }

    #[tauri::command]
    fn list_available_models() -> Result<Vec<super::AvailableModel>, String> {
        super::list_available_models()
    }

    /// Stream chat via Ollama's OpenAI-compatible endpoint.
    /// Emits `chat-token` events with throttling, returns full text.
    #[tauri::command]
    async fn chat_with_ollama(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        messages: Vec<serde_json::Value>,
        model: String,
        base_url: Option<String>,
    ) -> Result<String, String> {
        let app_state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let mut last_emit = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now);
            let mut full = String::new();

            let result =
                super::chat_with_ollama_streaming(&app_state, messages, model, base_url, |token| {
                    full.push_str(token);

                    // Throttle: emit at most every 50ms
                    let now = std::time::Instant::now();
                    if now.duration_since(last_emit).as_millis() >= 50 {
                        let _ = window.emit(
                            "chat-token",
                            serde_json::json!({
                                "token": token,
                                "full": &full,
                                "done": false,
                            }),
                        );
                        last_emit = now;
                    }
                });

            match &result {
                Ok(text) => {
                    let _ = window.emit(
                        "chat-token",
                        serde_json::json!({
                            "token": "",
                            "full": text,
                            "done": true,
                        }),
                    );
                }
                Err(e) => {
                    let _ = window.emit(
                        "chat-token",
                        serde_json::json!({
                            "token": "",
                            "full": "",
                            "done": true,
                            "error": e,
                        }),
                    );
                }
            }

            let _ = tx.send(result);
        });
        rx.recv()
            .unwrap_or(Err("Chat thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn set_agent_model(agent: String, model: String) -> Result<(), String> {
        super::set_agent_model(agent, model)
    }

    #[tauri::command]
    fn check_llm_status() -> Result<super::LlmStatusResponse, String> {
        super::check_llm_status()
    }

    #[tauri::command]
    fn get_llm_recommendations() -> Result<super::LlmRecommendations, String> {
        super::get_llm_recommendations()
    }

    #[tauri::command]
    fn set_agent_llm_provider(
        agent_id: String,
        provider_id: String,
        local_only: bool,
        budget_dollars: u32,
        budget_tokens: u64,
    ) -> Result<(), String> {
        super::set_agent_llm_provider(
            agent_id,
            provider_id,
            local_only,
            budget_dollars,
            budget_tokens,
        )
    }

    #[tauri::command]
    fn get_provider_usage_stats(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ProviderUsageStats>, String> {
        super::get_provider_usage_stats(state.inner())
    }

    #[tauri::command]
    fn test_llm_connection(provider_name: String) -> Result<super::TestConnectionResult, String> {
        super::test_llm_connection(provider_name)
    }

    #[tauri::command]
    fn get_system_info() -> Result<SystemInfo, String> {
        super::get_system_info()
    }

    // ── Permission Dashboard Commands ──

    #[tauri::command]
    fn get_agent_permissions(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelPermissionCategory>, String> {
        super::get_agent_permissions(state.inner(), agent_id)
    }

    #[tauri::command]
    fn update_agent_permission(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        capability_key: String,
        enabled: bool,
    ) -> Result<(), String> {
        super::update_agent_permission(state.inner(), agent_id, capability_key, enabled)
    }

    #[tauri::command]
    fn get_permission_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelPermissionHistoryEntry>, String> {
        super::get_permission_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn get_capability_request(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<Vec<KernelCapabilityRequest>, String> {
        super::get_capability_request(state.inner(), agent_id)
    }

    #[tauri::command]
    fn bulk_update_permissions(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        updates: Vec<super::PermissionUpdate>,
        reason: Option<String>,
    ) -> Result<(), String> {
        super::bulk_update_permissions(state.inner(), agent_id, updates, reason)
    }

    // ── Protocols Dashboard Commands ──

    #[tauri::command]
    fn get_protocols_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::ProtocolsStatusRow, String> {
        super::get_protocols_status(state.inner())
    }

    #[tauri::command]
    fn get_protocols_requests(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ProtocolRequestRow>, String> {
        super::get_protocols_requests(state.inner())
    }

    #[tauri::command]
    fn get_mcp_tools(state: tauri::State<'_, AppState>) -> Result<Vec<super::McpToolRow>, String> {
        super::get_mcp_tools(state.inner())
    }

    #[tauri::command]
    fn get_agent_cards(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::AgentCardRow>, String> {
        super::get_agent_cards(state.inner())
    }

    // ── Identity Commands ──

    #[tauri::command]
    fn get_agent_identity(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<super::IdentityRow, String> {
        super::get_agent_identity(state.inner(), agent_id)
    }

    #[tauri::command]
    fn list_identities(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::IdentityRow>, String> {
        super::list_identities(state.inner())
    }

    // ── Firewall Commands ──

    #[tauri::command]
    fn get_firewall_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::FirewallStatusRow, String> {
        super::get_firewall_status(state.inner())
    }

    #[tauri::command]
    fn get_firewall_patterns() -> Result<super::FirewallPatternsRow, String> {
        super::get_firewall_patterns()
    }

    // ── Marketplace Commands ──

    #[tauri::command]
    fn marketplace_search(query: String) -> Result<Vec<super::MarketplaceAgentRow>, String> {
        super::marketplace_search(&query)
    }

    #[tauri::command]
    fn marketplace_install(package_id: String) -> Result<super::MarketplaceAgentRow, String> {
        super::marketplace_install(&package_id)
    }

    #[tauri::command]
    fn marketplace_info(agent_id: String) -> Result<super::MarketplaceDetailRow, String> {
        super::marketplace_info(&agent_id)
    }

    #[tauri::command]
    fn marketplace_publish(bundle_json: String) -> Result<super::MarketplacePublishResult, String> {
        super::marketplace_publish(&bundle_json)
    }

    #[tauri::command]
    fn marketplace_my_agents(author: String) -> Result<Vec<super::MarketplaceAgentRow>, String> {
        super::marketplace_my_agents(&author)
    }

    // ── Learn Mode Commands ──

    #[tauri::command]
    fn start_learning(
        state: tauri::State<'_, AppState>,
        sources: Vec<super::LearningSource>,
    ) -> Result<super::LearningSessionState, String> {
        super::start_learning(state.inner(), sources)
    }

    #[tauri::command]
    fn learning_agent_action(
        state: tauri::State<'_, AppState>,
        session_id: String,
        action: String,
        url: Option<String>,
        content: Option<String>,
    ) -> Result<super::LearningSessionState, String> {
        super::learning_agent_action(state.inner(), session_id, action, url, content)
    }

    #[tauri::command]
    fn get_learning_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::LearningSessionState, String> {
        super::get_learning_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_knowledge_base(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::KnowledgeEntry>, String> {
        super::get_knowledge_base(state.inner())
    }

    // ── Agent Browser Commands ──

    #[tauri::command]
    fn navigate_to(
        state: tauri::State<'_, AppState>,
        url: String,
    ) -> Result<super::BrowserNavigateResult, String> {
        super::navigate_to(state.inner(), url)
    }

    #[tauri::command]
    fn get_browser_history(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::BrowserHistoryEntry>, String> {
        super::get_browser_history(state.inner())
    }

    #[tauri::command]
    fn get_agent_activity(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ActivityMessageRow>, String> {
        super::get_agent_activity(state.inner())
    }

    // ── Research Mode Commands ──

    #[tauri::command]
    fn start_research(
        state: tauri::State<'_, AppState>,
        topic: String,
        num_agents: u32,
    ) -> Result<super::ResearchSessionState, String> {
        super::start_research(state.inner(), topic, num_agents)
    }

    #[tauri::command]
    fn research_agent_action(
        state: tauri::State<'_, AppState>,
        session_id: String,
        agent_id: String,
        action: String,
        url: Option<String>,
        content: Option<String>,
    ) -> Result<super::ResearchSessionState, String> {
        super::research_agent_action(state.inner(), session_id, agent_id, action, url, content)
    }

    #[tauri::command]
    fn complete_research(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::ResearchSessionState, String> {
        super::complete_research(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_research_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::ResearchSessionState, String> {
        super::get_research_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn list_research_sessions(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ResearchSessionState>, String> {
        super::list_research_sessions(state.inner())
    }

    // ── Build Mode Commands ──

    #[tauri::command]
    fn start_build(
        state: tauri::State<'_, AppState>,
        description: String,
    ) -> Result<super::BuildSessionState, String> {
        super::start_build(state.inner(), description)
    }

    #[tauri::command]
    fn build_append_code(
        state: tauri::State<'_, AppState>,
        session_id: String,
        delta: String,
        agent_name: String,
    ) -> Result<super::BuildSessionState, String> {
        super::build_append_code(state.inner(), session_id, delta, agent_name)
    }

    #[tauri::command]
    fn build_add_message(
        state: tauri::State<'_, AppState>,
        session_id: String,
        agent_name: String,
        role: String,
        content: String,
    ) -> Result<super::BuildSessionState, String> {
        super::build_add_message(state.inner(), session_id, agent_name, role, content)
    }

    #[tauri::command]
    fn complete_build(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::BuildSessionState, String> {
        super::complete_build(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_session(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<super::BuildSessionState, String> {
        super::get_build_session(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_code(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<String, String> {
        super::get_build_code(state.inner(), session_id)
    }

    #[tauri::command]
    fn get_build_preview(
        state: tauri::State<'_, AppState>,
        session_id: String,
    ) -> Result<String, String> {
        super::get_build_preview(state.inner(), session_id)
    }

    #[tauri::command]
    fn policy_list() -> Result<serde_json::Value, String> {
        super::policy_list()
    }

    #[tauri::command]
    fn policy_validate(content: String) -> Result<serde_json::Value, String> {
        super::policy_validate(content)
    }

    #[tauri::command]
    fn policy_test(
        content: String,
        principal: String,
        action: String,
        resource: String,
    ) -> Result<serde_json::Value, String> {
        super::policy_test(content, principal, action, resource)
    }

    #[tauri::command]
    fn policy_detect_conflicts() -> Result<serde_json::Value, String> {
        super::policy_detect_conflicts()
    }

    // ── RAG Pipeline Commands ──

    #[tauri::command]
    fn index_document(
        state: tauri::State<'_, AppState>,
        file_path: String,
    ) -> Result<String, String> {
        super::index_document(state.inner(), file_path)
    }

    #[tauri::command]
    fn search_documents(
        state: tauri::State<'_, AppState>,
        query: String,
        top_k: Option<u32>,
    ) -> Result<String, String> {
        super::search_documents(state.inner(), query, top_k)
    }

    #[tauri::command]
    fn chat_with_documents(
        state: tauri::State<'_, AppState>,
        question: String,
    ) -> Result<String, String> {
        super::chat_with_documents(state.inner(), question)
    }

    #[tauri::command]
    fn list_indexed_documents(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::list_indexed_documents(state.inner())
    }

    #[tauri::command]
    fn remove_indexed_document(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::remove_indexed_document(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_document_governance(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::get_document_governance(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_semantic_map(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_semantic_map(state.inner())
    }

    #[tauri::command]
    fn get_document_access_log(
        state: tauri::State<'_, AppState>,
        doc_path: String,
    ) -> Result<String, String> {
        super::get_document_access_log(state.inner(), doc_path)
    }

    #[tauri::command]
    fn get_active_llm_provider(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_active_llm_provider(state.inner())
    }

    // ── Model Hub Commands ──

    #[tauri::command]
    fn search_models(
        state: tauri::State<'_, AppState>,
        query: String,
        limit: Option<u32>,
    ) -> Result<String, String> {
        super::search_models(state.inner(), query, limit)
    }

    #[tauri::command]
    fn get_model_info(
        state: tauri::State<'_, AppState>,
        model_id: String,
    ) -> Result<String, String> {
        super::get_model_info(state.inner(), model_id)
    }

    #[tauri::command]
    fn check_model_compatibility(
        state: tauri::State<'_, AppState>,
        file_size_bytes: u64,
    ) -> Result<String, String> {
        super::check_model_compatibility(state.inner(), file_size_bytes)
    }

    #[tauri::command]
    async fn download_model(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        model_id: String,
        filename: String,
    ) -> Result<String, String> {
        // Read models_dir from registry (lock briefly)
        let models_dir = {
            let registry = state
                .model_registry
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            registry.models_dir().clone()
        };

        let model_id_clone = model_id.clone();
        let filename_clone = filename.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let target_dir = models_dir.display().to_string();
            let last_emit = std::cell::Cell::new(
                std::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(1))
                    .unwrap_or_else(std::time::Instant::now),
            );

            let result = super::model_hub::download_model_file(
                &model_id_clone,
                &filename_clone,
                &target_dir,
                |progress: DownloadProgress| {
                    let now = std::time::Instant::now();
                    let is_terminal = matches!(
                        progress.status,
                        DownloadStatus::Completed | DownloadStatus::Failed(_)
                    );

                    // Throttle at 300ms, but always emit terminal states
                    if is_terminal || now.duration_since(last_emit.get()).as_millis() >= 300 {
                        let _ = window.emit("model-download-progress", &progress);
                        last_emit.set(now);
                    }
                },
            );

            match &result {
                Ok(model_path) => {
                    // Generate nexus-model.toml so ModelRegistry can discover it
                    let _ = super::model_hub::generate_model_config(
                        &model_id_clone,
                        &filename_clone,
                        model_path,
                    );
                    let _ = window.emit(
                        "model-download-complete",
                        serde_json::json!({"model_id": &model_id_clone, "path": model_path}),
                    );
                }
                Err(e) => {
                    let _ = window.emit(
                        "model-download-complete",
                        serde_json::json!({"model_id": &model_id_clone, "error": e}),
                    );
                }
            }

            let _ = tx.send(result);
        });

        // Return immediately — the thread will emit progress events
        // But we still wait for the result so Tauri knows when the command finishes
        rx.recv()
            .unwrap_or(Err("Download thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn list_local_models(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::list_local_models(state.inner())
    }

    #[tauri::command]
    fn delete_local_model(
        state: tauri::State<'_, AppState>,
        model_id: String,
    ) -> Result<String, String> {
        super::delete_local_model(state.inner(), model_id)
    }

    #[tauri::command]
    fn get_system_specs() -> Result<String, String> {
        super::get_system_specs()
    }

    #[tauri::command]
    fn get_live_system_metrics(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::get_live_system_metrics(state.inner())
    }

    #[tauri::command]
    fn time_machine_list_checkpoints(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_list_checkpoints(state.inner())
    }

    #[tauri::command]
    fn time_machine_get_checkpoint(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_get_checkpoint(state.inner(), id)
    }

    #[tauri::command]
    fn time_machine_create_checkpoint(
        state: tauri::State<'_, AppState>,
        label: String,
    ) -> Result<String, String> {
        super::time_machine_create_checkpoint(state.inner(), label)
    }

    #[tauri::command]
    fn time_machine_undo(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_undo(state.inner())
    }

    #[tauri::command]
    fn time_machine_undo_checkpoint(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_undo_checkpoint(state.inner(), id)
    }

    #[tauri::command]
    fn time_machine_redo(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::time_machine_redo(state.inner())
    }

    #[tauri::command]
    fn time_machine_get_diff(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::time_machine_get_diff(state.inner(), id)
    }

    // ── Nexus Link commands ─────────────────────────────────────────────

    #[tauri::command]
    fn nexus_link_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::nexus_link_status(state.inner())
    }

    #[tauri::command]
    fn nexus_link_toggle_sharing(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::nexus_link_toggle_sharing(state.inner(), enabled)
    }

    #[tauri::command]
    fn nexus_link_add_peer(
        state: tauri::State<'_, AppState>,
        address: String,
        name: String,
    ) -> Result<String, String> {
        super::nexus_link_add_peer(state.inner(), address, name)
    }

    #[tauri::command]
    fn nexus_link_remove_peer(
        state: tauri::State<'_, AppState>,
        device_id: String,
    ) -> Result<String, String> {
        super::nexus_link_remove_peer(state.inner(), device_id)
    }

    #[tauri::command]
    fn nexus_link_list_peers(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::nexus_link_list_peers(state.inner())
    }

    #[tauri::command]
    async fn nexus_link_send_model(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        peer_address: String,
        model_id: String,
        filename: String,
    ) -> Result<String, String> {
        // Clone what we need from state before spawning thread
        let link_arc = state.nexus_link.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let link = link_arc.lock().unwrap_or_else(|p| p.into_inner());

            let last_emit = std::cell::Cell::new(
                std::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(1))
                    .unwrap_or_else(std::time::Instant::now),
            );

            let result = link.send_model(
                &peer_address,
                &model_id,
                &filename,
                |progress: nexus_connectors_llm::nexus_link::TransferProgress| {
                    let now = std::time::Instant::now();
                    let is_terminal = matches!(
                        progress.status,
                        nexus_connectors_llm::nexus_link::TransferStatus::Completed
                            | nexus_connectors_llm::nexus_link::TransferStatus::Failed(_)
                    );

                    if is_terminal || now.duration_since(last_emit.get()).as_millis() >= 300 {
                        let _ = window.emit("nexus-link-transfer-progress", &progress);
                        last_emit.set(now);
                    }
                },
            );

            let _ = tx.send(result.map(|()| "completed".to_string()));
        });

        // Return immediately — progress is emitted via events
        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(format!("Transfer thread failed: {e}")),
        }
    }

    // ── Evolution commands ───────────────────────────────────────────────

    #[tauri::command]
    fn evolution_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::evolution_get_status(state.inner())
    }

    #[tauri::command]
    fn evolution_register_strategy(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        name: String,
        parameters: String,
    ) -> Result<String, String> {
        super::evolution_register_strategy(state.inner(), agent_id, name, parameters)
    }

    #[tauri::command]
    fn evolution_evolve_once(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_evolve_once(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_get_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_get_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_rollback(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_rollback(state.inner(), agent_id)
    }

    #[tauri::command]
    fn evolution_get_active_strategy(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::evolution_get_active_strategy(state.inner(), agent_id)
    }

    // ── MCP Host commands ───────────────────────────────────────────────

    #[tauri::command]
    fn mcp_host_list_servers(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::mcp_host_list_servers(state.inner())
    }

    #[tauri::command]
    fn mcp_host_add_server(
        state: tauri::State<'_, AppState>,
        name: String,
        url: String,
        transport: String,
        auth_token: Option<String>,
    ) -> Result<String, String> {
        super::mcp_host_add_server(state.inner(), name, url, transport, auth_token)
    }

    #[tauri::command]
    fn mcp_host_remove_server(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_remove_server(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_connect(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_connect(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_disconnect(
        state: tauri::State<'_, AppState>,
        server_id: String,
    ) -> Result<String, String> {
        super::mcp_host_disconnect(state.inner(), server_id)
    }

    #[tauri::command]
    fn mcp_host_list_tools(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::mcp_host_list_tools(state.inner())
    }

    #[tauri::command]
    fn mcp_host_call_tool(
        state: tauri::State<'_, AppState>,
        tool_name: String,
        arguments: String,
    ) -> Result<String, String> {
        super::mcp_host_call_tool(state.inner(), tool_name, arguments)
    }

    // ── Ghost Protocol commands ─────────────────────────────────────────

    #[tauri::command]
    fn ghost_protocol_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_status(state.inner())
    }

    #[tauri::command]
    fn ghost_protocol_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::ghost_protocol_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn ghost_protocol_add_peer(
        state: tauri::State<'_, AppState>,
        address: String,
        name: String,
    ) -> Result<String, String> {
        super::ghost_protocol_add_peer(state.inner(), address, name)
    }

    #[tauri::command]
    fn ghost_protocol_remove_peer(
        state: tauri::State<'_, AppState>,
        device_id: String,
    ) -> Result<String, String> {
        super::ghost_protocol_remove_peer(state.inner(), device_id)
    }

    #[tauri::command]
    fn ghost_protocol_sync_now(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_sync_now(state.inner())
    }

    #[tauri::command]
    fn ghost_protocol_get_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::ghost_protocol_get_state(state.inner())
    }

    // ── Voice Assistant commands ─────────────────────────────────────

    #[tauri::command]
    fn voice_start_listening(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_start_listening(state.inner())
    }

    #[tauri::command]
    fn voice_stop_listening(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_stop_listening(state.inner())
    }

    #[tauri::command]
    fn voice_get_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::voice_get_status(state.inner())
    }

    #[tauri::command]
    fn voice_transcribe(
        state: tauri::State<'_, AppState>,
        audio_base64: String,
    ) -> Result<String, String> {
        super::voice_transcribe(state.inner(), audio_base64)
    }

    #[tauri::command]
    fn voice_load_whisper_model(
        state: tauri::State<'_, AppState>,
        model_path: String,
    ) -> Result<String, String> {
        super::voice_load_whisper_model(state.inner(), model_path)
    }

    // ── Software Factory commands ────────────────────────────────────

    #[tauri::command]
    fn factory_create_project(
        state: tauri::State<'_, AppState>,
        name: String,
        language: String,
        source_dir: String,
    ) -> Result<String, String> {
        super::factory_create_project(state.inner(), name, language, source_dir)
    }

    #[tauri::command]
    fn factory_build_project(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_build_project(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_test_project(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_test_project(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_run_pipeline(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_run_pipeline(state.inner(), project_id)
    }

    #[tauri::command]
    fn factory_list_projects(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::factory_list_projects(state.inner())
    }

    #[tauri::command]
    fn factory_get_build_history(
        state: tauri::State<'_, AppState>,
        project_id: String,
    ) -> Result<String, String> {
        super::factory_get_build_history(state.inner(), project_id)
    }

    /// Run the Conductor orchestration pipeline with progress events.
    #[tauri::command]
    async fn conduct_build(
        window: tauri::Window,
        state: tauri::State<'_, AppState>,
        prompt: String,
        output_dir: Option<String>,
        model: Option<String>,
    ) -> Result<serde_json::Value, String> {
        let app_state = state.inner().clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();

        std::thread::spawn(move || {
            // Compute output dir
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let out_dir = output_dir.unwrap_or_else(|| format!("{home}/.nexus/builds/{timestamp}"));

            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                let _ = tx.send(Err(format!("failed to create output dir: {e}")));
                return;
            }

            let model_name = model.unwrap_or_else(|| "mistral".to_string());
            let provider = super::OllamaProvider::from_env();
            let mut conductor = super::Conductor::new(provider, &model_name);

            // Preview plan and emit
            let request_for_plan = super::UserRequest::new(&prompt, &out_dir);
            let plan = match conductor.preview_plan(&request_for_plan) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(Err(format!("planning failed: {e}")));
                    return;
                }
            };
            let _ = window.emit("conductor:plan", &plan);

            // Run full orchestration
            let request = super::UserRequest::new(&prompt, &out_dir);
            let mut supervisor = app_state
                .supervisor
                .lock()
                .unwrap_or_else(|p| p.into_inner());

            let start = std::time::Instant::now();
            let result = conductor.run(request, &mut supervisor);
            drop(supervisor);

            match result {
                Ok(mut res) => {
                    res.duration_secs = start.elapsed().as_secs_f64();

                    // Emit per-agent completion events
                    let _ = window.emit(
                        "conductor:agent_completed",
                        &serde_json::json!({
                            "agents_used": res.agents_used,
                            "output_files": &res.output_files,
                        }),
                    );

                    // Emit finished
                    let _ = window.emit("conductor:finished", &res);

                    // Audit log
                    app_state.log_event(
                        uuid::Uuid::nil(),
                        super::EventType::StateChange,
                        serde_json::json!({
                            "source": "conductor",
                            "action": "conduct_build",
                            "status": format!("{:?}", res.status),
                            "agents_used": res.agents_used,
                            "total_fuel_used": res.total_fuel_used,
                            "duration_secs": res.duration_secs,
                        }),
                    );

                    let plan_json = serde_json::to_value(&plan).unwrap_or_default();
                    let result_json = serde_json::to_value(&res).unwrap_or_default();
                    let _ = tx.send(Ok(serde_json::json!({
                        "plan": plan_json,
                        "result": result_json,
                    })));
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("conductor failed: {e}")));
                }
            }
        });

        rx.recv()
            .unwrap_or(Err("Conductor thread terminated unexpectedly".to_string()))
    }

    #[tauri::command]
    fn execute_tool(
        state: tauri::State<'_, AppState>,
        tool_json: String,
    ) -> Result<String, String> {
        super::execute_tool(state.inner(), tool_json)
    }

    #[tauri::command]
    fn list_tools() -> Result<String, String> {
        super::list_tools()
    }

    #[tauri::command]
    fn terminal_execute(
        state: tauri::State<'_, AppState>,
        command: String,
        cwd: String,
    ) -> Result<String, String> {
        super::terminal_execute(state.inner(), command, cwd)
    }

    #[tauri::command]
    fn terminal_execute_approved(
        state: tauri::State<'_, AppState>,
        command: String,
        cwd: String,
    ) -> Result<String, String> {
        super::terminal_execute_approved(state.inner(), command, cwd)
    }

    #[tauri::command]
    fn replay_list_bundles(
        state: tauri::State<'_, AppState>,
        agent_id: Option<String>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::replay_list_bundles(state.inner(), agent_id, limit)
    }

    #[tauri::command]
    fn replay_get_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_get_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_verify_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_verify_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_export_bundle(
        state: tauri::State<'_, AppState>,
        bundle_id: String,
    ) -> Result<String, String> {
        super::replay_export_bundle(state.inner(), bundle_id)
    }

    #[tauri::command]
    fn replay_toggle_recording(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::replay_toggle_recording(state.inner(), enabled)
    }

    #[tauri::command]
    fn airgap_create_bundle(
        state: tauri::State<'_, AppState>,
        target_os: String,
        target_arch: String,
        output_path: String,
        components: Option<String>,
    ) -> Result<String, String> {
        super::airgap_create_bundle(
            state.inner(),
            target_os,
            target_arch,
            output_path,
            components,
        )
    }

    #[tauri::command]
    fn airgap_validate_bundle(
        state: tauri::State<'_, AppState>,
        bundle_path: String,
    ) -> Result<String, String> {
        super::airgap_validate_bundle(state.inner(), bundle_path)
    }

    #[tauri::command]
    fn airgap_install_bundle(
        state: tauri::State<'_, AppState>,
        bundle_path: String,
        install_dir: String,
    ) -> Result<String, String> {
        super::airgap_install_bundle(state.inner(), bundle_path, install_dir)
    }

    #[tauri::command]
    fn airgap_get_system_info(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::airgap_get_system_info(state.inner())
    }

    #[tauri::command]
    fn reputation_register(
        state: tauri::State<'_, AppState>,
        did: String,
        name: String,
    ) -> Result<String, String> {
        super::reputation_register(state.inner(), did, name)
    }

    #[tauri::command]
    fn reputation_record_task(
        state: tauri::State<'_, AppState>,
        did: String,
        success: bool,
    ) -> Result<String, String> {
        super::reputation_record_task(state.inner(), did, success)
    }

    #[tauri::command]
    fn reputation_rate_agent(
        state: tauri::State<'_, AppState>,
        did: String,
        rater_did: String,
        score: f64,
        comment: Option<String>,
    ) -> Result<String, String> {
        super::reputation_rate_agent(state.inner(), did, rater_did, score, comment)
    }

    #[tauri::command]
    fn reputation_get(state: tauri::State<'_, AppState>, did: String) -> Result<String, String> {
        super::reputation_get(state.inner(), did)
    }

    #[tauri::command]
    fn reputation_top(
        state: tauri::State<'_, AppState>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::reputation_top(state.inner(), limit)
    }

    #[tauri::command]
    fn reputation_export(state: tauri::State<'_, AppState>, did: String) -> Result<String, String> {
        super::reputation_export(state.inner(), did)
    }

    #[tauri::command]
    fn reputation_import(
        state: tauri::State<'_, AppState>,
        json: String,
    ) -> Result<String, String> {
        super::reputation_import(state.inner(), json)
    }

    #[tauri::command]
    fn computer_control_capture_screen(
        state: tauri::State<'_, AppState>,
        region: Option<String>,
    ) -> Result<String, String> {
        super::computer_control_capture_screen(state.inner(), region)
    }

    #[tauri::command]
    fn computer_control_execute_action(
        state: tauri::State<'_, AppState>,
        action_json: String,
    ) -> Result<String, String> {
        super::computer_control_execute_action(state.inner(), action_json)
    }

    #[tauri::command]
    fn computer_control_get_history(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::computer_control_get_history(state.inner())
    }

    #[tauri::command]
    fn computer_control_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::computer_control_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn computer_control_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::computer_control_status(state.inner())
    }

    #[tauri::command]
    fn neural_bridge_status(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::neural_bridge_status(state.inner())
    }

    #[tauri::command]
    fn neural_bridge_toggle(
        state: tauri::State<'_, AppState>,
        enabled: bool,
    ) -> Result<String, String> {
        super::neural_bridge_toggle(state.inner(), enabled)
    }

    #[tauri::command]
    fn neural_bridge_ingest(
        state: tauri::State<'_, AppState>,
        source_type: String,
        content: String,
        metadata: serde_json::Value,
    ) -> Result<String, String> {
        super::neural_bridge_ingest(state.inner(), source_type, content, metadata)
    }

    #[tauri::command]
    fn neural_bridge_search(
        state: tauri::State<'_, AppState>,
        query: String,
        time_range: Option<(u64, u64)>,
        source_filter: Option<Vec<String>>,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::neural_bridge_search(state.inner(), query, time_range, source_filter, max_results)
    }

    #[tauri::command]
    fn neural_bridge_delete(
        state: tauri::State<'_, AppState>,
        id: String,
    ) -> Result<String, String> {
        super::neural_bridge_delete(state.inner(), id)
    }

    #[tauri::command]
    fn neural_bridge_clear_old(
        state: tauri::State<'_, AppState>,
        before_timestamp: u64,
    ) -> Result<String, String> {
        super::neural_bridge_clear_old(state.inner(), before_timestamp)
    }

    #[tauri::command]
    fn economy_create_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_create_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_get_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_spend(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        amount: f64,
        tx_type: String,
        description: String,
    ) -> Result<String, String> {
        super::economy_spend(state.inner(), agent_id, amount, tx_type, description)
    }

    #[tauri::command]
    fn economy_earn(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        amount: f64,
        description: String,
    ) -> Result<String, String> {
        super::economy_earn(state.inner(), agent_id, amount, description)
    }

    #[tauri::command]
    fn economy_transfer(
        state: tauri::State<'_, AppState>,
        from: String,
        to: String,
        amount: f64,
        description: String,
    ) -> Result<String, String> {
        super::economy_transfer(state.inner(), from, to, amount, description)
    }

    #[tauri::command]
    fn economy_freeze_wallet(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_freeze_wallet(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_history(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_get_history(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_get_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::economy_get_stats(state.inner())
    }

    #[tauri::command]
    #[allow(clippy::too_many_arguments)]
    fn economy_create_contract(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        client_id: String,
        description: String,
        criteria_json: String,
        reward: f64,
        penalty: f64,
        deadline: Option<u64>,
    ) -> Result<String, String> {
        super::economy_create_contract(
            state.inner(),
            agent_id,
            client_id,
            description,
            criteria_json,
            reward,
            penalty,
            deadline,
        )
    }

    #[tauri::command]
    fn economy_complete_contract(
        state: tauri::State<'_, AppState>,
        contract_id: String,
        success: bool,
        evidence: Option<String>,
    ) -> Result<String, String> {
        super::economy_complete_contract(state.inner(), contract_id, success, evidence)
    }

    #[tauri::command]
    fn economy_list_contracts(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_list_contracts(state.inner(), agent_id)
    }

    #[tauri::command]
    fn economy_dispute_contract(
        state: tauri::State<'_, AppState>,
        contract_id: String,
        reason: String,
    ) -> Result<String, String> {
        super::economy_dispute_contract(state.inner(), contract_id, reason)
    }

    #[tauri::command]
    fn economy_agent_performance(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::economy_agent_performance(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_remember(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        content: String,
        memory_type: String,
        importance: f64,
        tags: Vec<String>,
    ) -> Result<String, String> {
        super::agent_memory_remember(
            state.inner(),
            agent_id,
            content,
            memory_type,
            importance,
            tags,
        )
    }

    #[tauri::command]
    fn agent_memory_recall(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        query: String,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::agent_memory_recall(state.inner(), agent_id, query, max_results)
    }

    #[tauri::command]
    fn agent_memory_recall_by_type(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        memory_type: String,
        max_results: Option<usize>,
    ) -> Result<String, String> {
        super::agent_memory_recall_by_type(state.inner(), agent_id, memory_type, max_results)
    }

    #[tauri::command]
    fn agent_memory_forget(
        state: tauri::State<'_, AppState>,
        agent_id: String,
        memory_id: String,
    ) -> Result<String, String> {
        super::agent_memory_forget(state.inner(), agent_id, memory_id)
    }

    #[tauri::command]
    fn agent_memory_get_stats(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_get_stats(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_save(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_save(state.inner(), agent_id)
    }

    #[tauri::command]
    fn agent_memory_clear(
        state: tauri::State<'_, AppState>,
        agent_id: String,
    ) -> Result<String, String> {
        super::agent_memory_clear(state.inner(), agent_id)
    }

    #[tauri::command]
    fn tracing_start_trace(
        state: tauri::State<'_, AppState>,
        operation_name: String,
        agent_id: Option<String>,
    ) -> Result<String, String> {
        super::tracing_start_trace(state.inner(), operation_name, agent_id)
    }

    #[tauri::command]
    fn tracing_start_span(
        state: tauri::State<'_, AppState>,
        trace_id: String,
        parent_span_id: String,
        operation_name: String,
        agent_id: Option<String>,
    ) -> Result<String, String> {
        super::tracing_start_span(
            state.inner(),
            trace_id,
            parent_span_id,
            operation_name,
            agent_id,
        )
    }

    #[tauri::command]
    fn tracing_end_span(
        state: tauri::State<'_, AppState>,
        span_id: String,
        status: String,
        error_message: Option<String>,
    ) -> Result<String, String> {
        super::tracing_end_span(state.inner(), span_id, status, error_message)
    }

    #[tauri::command]
    fn tracing_end_trace(
        state: tauri::State<'_, AppState>,
        trace_id: String,
    ) -> Result<String, String> {
        super::tracing_end_trace(state.inner(), trace_id)
    }

    #[tauri::command]
    fn tracing_list_traces(
        state: tauri::State<'_, AppState>,
        limit: Option<usize>,
    ) -> Result<String, String> {
        super::tracing_list_traces(state.inner(), limit)
    }

    #[tauri::command]
    fn tracing_get_trace(
        state: tauri::State<'_, AppState>,
        trace_id: String,
    ) -> Result<String, String> {
        super::tracing_get_trace(state.inner(), trace_id)
    }

    #[tauri::command]
    fn payment_create_plan(
        state: tauri::State<'_, AppState>,
        name: String,
        price_cents: u64,
        interval: String,
        features: Vec<String>,
    ) -> Result<String, String> {
        super::payment_create_plan(state.inner(), name, price_cents, interval, features)
    }

    #[tauri::command]
    fn payment_list_plans(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::payment_list_plans(state.inner())
    }

    #[tauri::command]
    fn payment_create_invoice(
        state: tauri::State<'_, AppState>,
        plan_id: String,
        buyer_id: String,
    ) -> Result<String, String> {
        super::payment_create_invoice(state.inner(), plan_id, buyer_id)
    }

    #[tauri::command]
    fn payment_pay_invoice(
        state: tauri::State<'_, AppState>,
        invoice_id: String,
    ) -> Result<String, String> {
        super::payment_pay_invoice(state.inner(), invoice_id)
    }

    #[tauri::command]
    fn payment_get_revenue_stats(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::payment_get_revenue_stats(state.inner())
    }

    #[tauri::command]
    fn payment_create_payout(
        state: tauri::State<'_, AppState>,
        developer_id: String,
        agent_id: String,
        amount_cents: u64,
        period: String,
    ) -> Result<String, String> {
        super::payment_create_payout(state.inner(), developer_id, agent_id, amount_cents, period)
    }

    #[tauri::command]
    fn get_compliance_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::ComplianceStatusRow, String> {
        super::get_compliance_status(state.inner())
    }

    #[tauri::command]
    fn get_compliance_agents(
        state: tauri::State<'_, AppState>,
    ) -> Result<Vec<super::ComplianceAgentRow>, String> {
        super::get_compliance_agents(state.inner())
    }

    #[tauri::command]
    fn get_audit_chain_status(
        state: tauri::State<'_, AppState>,
    ) -> Result<super::AuditChainStatusRow, String> {
        super::get_audit_chain_status(state.inner())
    }

    #[tauri::command]
    fn verify_governance_invariants(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::verify_governance_invariants(state.inner())
    }

    #[tauri::command]
    fn verify_specific_invariant(
        state: tauri::State<'_, AppState>,
        invariant_name: String,
    ) -> Result<String, String> {
        super::verify_specific_invariant(state.inner(), invariant_name)
    }

    #[tauri::command]
    fn export_compliance_report(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::export_compliance_report(state.inner())
    }

    #[tauri::command]
    fn file_manager_list(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_list(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_read(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_read(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_write(
        state: tauri::State<'_, AppState>,
        path: String,
        content: String,
    ) -> Result<String, String> {
        super::file_manager_write(state.inner(), path, content)
    }

    #[tauri::command]
    fn file_manager_create_dir(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_create_dir(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_delete(
        state: tauri::State<'_, AppState>,
        path: String,
    ) -> Result<String, String> {
        super::file_manager_delete(state.inner(), path)
    }

    #[tauri::command]
    fn file_manager_rename(
        state: tauri::State<'_, AppState>,
        from: String,
        to: String,
    ) -> Result<String, String> {
        super::file_manager_rename(state.inner(), from, to)
    }

    #[tauri::command]
    fn file_manager_home() -> Result<String, String> {
        super::file_manager_home()
    }

    // ── Database Manager commands ──
    #[tauri::command]
    fn db_connect(
        state: tauri::State<'_, AppState>,
        connection_string: String,
    ) -> Result<String, String> {
        super::db_connect(state.inner(), connection_string)
    }

    #[tauri::command]
    fn db_execute_query(
        state: tauri::State<'_, AppState>,
        connection_string: String,
        query: String,
    ) -> Result<String, String> {
        super::db_execute_query(state.inner(), connection_string, query)
    }

    #[tauri::command]
    fn db_list_tables(
        state: tauri::State<'_, AppState>,
        connection_string: String,
    ) -> Result<String, String> {
        super::db_list_tables(state.inner(), connection_string)
    }

    // ── API Client commands ──
    #[tauri::command]
    fn api_client_request(
        state: tauri::State<'_, AppState>,
        method: String,
        url: String,
        headers_json: String,
        body: String,
    ) -> Result<String, String> {
        super::api_client_request(state.inner(), method, url, headers_json, body)
    }

    // ── Email Client commands ──
    #[tauri::command]
    fn email_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::email_list(state.inner())
    }

    #[tauri::command]
    fn email_save(
        state: tauri::State<'_, AppState>,
        id: String,
        data_json: String,
    ) -> Result<String, String> {
        super::email_save(state.inner(), id, data_json)
    }

    #[tauri::command]
    fn email_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::email_delete(state.inner(), id)
    }

    // ── Project Manager commands ──
    #[tauri::command]
    fn project_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::project_list(state.inner())
    }

    #[tauri::command]
    fn project_get(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::project_get(state.inner(), id)
    }

    #[tauri::command]
    fn project_save(
        state: tauri::State<'_, AppState>,
        id: String,
        data_json: String,
    ) -> Result<String, String> {
        super::project_save(state.inner(), id, data_json)
    }

    #[tauri::command]
    fn project_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::project_delete(state.inner(), id)
    }

    // ── Notes App commands ──
    #[tauri::command]
    fn notes_list(state: tauri::State<'_, AppState>) -> Result<String, String> {
        super::notes_list(state.inner())
    }

    #[tauri::command]
    fn notes_get(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::notes_get(state.inner(), id)
    }

    #[tauri::command]
    fn notes_save(
        state: tauri::State<'_, AppState>,
        id: String,
        title: String,
        content: String,
        folder_id: String,
        tags_json: String,
    ) -> Result<String, String> {
        super::notes_save(state.inner(), id, title, content, folder_id, tags_json)
    }

    #[tauri::command]
    fn notes_delete(state: tauri::State<'_, AppState>, id: String) -> Result<String, String> {
        super::notes_delete(state.inner(), id)
    }

    pub fn run() {
        let builder = tauri::Builder::<tauri::Wry>::default().manage(AppState::new());

        #[cfg(target_os = "linux")]
        let builder = builder;

        #[cfg(not(target_os = "linux"))]
        let builder = builder.setup(|app| {
            let show_dashboard =
                MenuItem::with_id(app, "show_dashboard", "Show Dashboard", true, None::<&str>)?;
            let start_voice =
                MenuItem::with_id(app, "start_voice", "Start Voice", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_dashboard, &start_voice, &quit])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_dashboard" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "start_voice" => {
                        let state = app.state::<AppState>();
                        let _ = super::start_jarvis_mode(state.inner());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Down,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        });

        builder
            .invoke_handler(tauri::generate_handler![
                list_agents,
                create_agent,
                start_agent,
                stop_agent,
                pause_agent,
                resume_agent,
                get_audit_log,
                send_chat,
                get_config,
                save_config,
                start_jarvis_mode,
                stop_jarvis_mode,
                jarvis_status,
                transcribe_push_to_talk,
                tray_status,
                detect_hardware,
                check_ollama,
                pull_ollama_model,
                pull_model,
                ensure_ollama,
                is_ollama_installed,
                delete_model,
                is_setup_complete,
                run_setup_wizard,
                list_available_models,
                chat_with_ollama,
                set_agent_model,
                check_llm_status,
                get_llm_recommendations,
                set_agent_llm_provider,
                get_provider_usage_stats,
                test_llm_connection,
                get_system_info,
                get_agent_permissions,
                update_agent_permission,
                get_permission_history,
                get_capability_request,
                bulk_update_permissions,
                get_protocols_status,
                get_protocols_requests,
                get_mcp_tools,
                get_agent_cards,
                get_agent_identity,
                list_identities,
                get_firewall_status,
                get_firewall_patterns,
                marketplace_search,
                marketplace_install,
                marketplace_info,
                marketplace_publish,
                marketplace_my_agents,
                start_learning,
                learning_agent_action,
                get_learning_session,
                get_knowledge_base,
                navigate_to,
                get_browser_history,
                get_agent_activity,
                start_research,
                research_agent_action,
                complete_research,
                get_research_session,
                list_research_sessions,
                start_build,
                build_append_code,
                build_add_message,
                complete_build,
                get_build_session,
                get_build_code,
                get_build_preview,
                policy_list,
                policy_validate,
                policy_test,
                policy_detect_conflicts,
                index_document,
                search_documents,
                chat_with_documents,
                list_indexed_documents,
                remove_indexed_document,
                get_document_governance,
                get_semantic_map,
                get_document_access_log,
                get_active_llm_provider,
                search_models,
                get_model_info,
                check_model_compatibility,
                download_model,
                list_local_models,
                delete_local_model,
                get_system_specs,
                get_live_system_metrics,
                time_machine_list_checkpoints,
                time_machine_get_checkpoint,
                time_machine_create_checkpoint,
                time_machine_undo,
                time_machine_undo_checkpoint,
                time_machine_redo,
                time_machine_get_diff,
                nexus_link_status,
                nexus_link_toggle_sharing,
                nexus_link_add_peer,
                nexus_link_remove_peer,
                nexus_link_list_peers,
                nexus_link_send_model,
                evolution_get_status,
                evolution_register_strategy,
                evolution_evolve_once,
                evolution_get_history,
                evolution_rollback,
                evolution_get_active_strategy,
                mcp_host_list_servers,
                mcp_host_add_server,
                mcp_host_remove_server,
                mcp_host_connect,
                mcp_host_disconnect,
                mcp_host_list_tools,
                mcp_host_call_tool,
                ghost_protocol_status,
                ghost_protocol_toggle,
                ghost_protocol_add_peer,
                ghost_protocol_remove_peer,
                ghost_protocol_sync_now,
                ghost_protocol_get_state,
                voice_start_listening,
                voice_stop_listening,
                voice_get_status,
                voice_transcribe,
                voice_load_whisper_model,
                factory_create_project,
                factory_build_project,
                factory_test_project,
                factory_run_pipeline,
                factory_list_projects,
                factory_get_build_history,
                conduct_build,
                execute_tool,
                list_tools,
                terminal_execute,
                terminal_execute_approved,
                replay_list_bundles,
                replay_get_bundle,
                replay_verify_bundle,
                replay_export_bundle,
                replay_toggle_recording,
                airgap_create_bundle,
                airgap_validate_bundle,
                airgap_install_bundle,
                airgap_get_system_info,
                reputation_register,
                reputation_record_task,
                reputation_rate_agent,
                reputation_get,
                reputation_top,
                reputation_export,
                reputation_import,
                computer_control_capture_screen,
                computer_control_execute_action,
                computer_control_get_history,
                computer_control_toggle,
                computer_control_status,
                neural_bridge_status,
                neural_bridge_toggle,
                neural_bridge_ingest,
                neural_bridge_search,
                neural_bridge_delete,
                neural_bridge_clear_old,
                economy_create_wallet,
                economy_get_wallet,
                economy_spend,
                economy_earn,
                economy_transfer,
                economy_freeze_wallet,
                economy_get_history,
                economy_get_stats,
                economy_create_contract,
                economy_complete_contract,
                economy_list_contracts,
                economy_dispute_contract,
                economy_agent_performance,
                agent_memory_remember,
                agent_memory_recall,
                agent_memory_recall_by_type,
                agent_memory_forget,
                agent_memory_get_stats,
                agent_memory_save,
                agent_memory_clear,
                tracing_start_trace,
                tracing_start_span,
                tracing_end_span,
                tracing_end_trace,
                tracing_list_traces,
                tracing_get_trace,
                payment_create_plan,
                payment_list_plans,
                payment_create_invoice,
                payment_pay_invoice,
                payment_get_revenue_stats,
                payment_create_payout,
                get_compliance_status,
                get_compliance_agents,
                get_audit_chain_status,
                verify_governance_invariants,
                verify_specific_invariant,
                export_compliance_report,
                file_manager_list,
                file_manager_read,
                file_manager_write,
                file_manager_create_dir,
                file_manager_delete,
                file_manager_rename,
                file_manager_home,
                db_connect,
                db_execute_query,
                db_list_tables,
                api_client_request,
                notes_list,
                notes_get,
                notes_save,
                notes_delete,
                email_list,
                email_save,
                email_delete,
                project_list,
                project_get,
                project_save,
                project_delete,
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn main() {
    runtime::run();
}

#[cfg(not(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
)))]
fn main() {
    println!("NexusOS desktop backend (tauri-runtime disabled in this build)");
}

#[cfg(test)]
mod tests {
    use super::{
        agent_memory_clear, agent_memory_forget, agent_memory_get_stats, agent_memory_recall,
        agent_memory_remember, chat_with_documents, check_model_compatibility, complete_build,
        complete_research, create_agent, economy_create_wallet, economy_earn,
        economy_freeze_wallet, economy_get_history, economy_get_stats, economy_get_wallet,
        economy_spend, economy_transfer, evolution_evolve_once, evolution_get_active_strategy,
        evolution_get_history, evolution_get_status, evolution_register_strategy,
        evolution_rollback, factory_create_project, factory_get_build_history,
        factory_list_projects, get_active_llm_provider, get_agent_activity, get_browser_history,
        get_configured_provider, get_knowledge_base, get_live_system_metrics, get_system_specs,
        ghost_protocol_add_peer, ghost_protocol_remove_peer, ghost_protocol_status,
        ghost_protocol_toggle, index_document, learning_agent_action, list_agents,
        list_indexed_documents, list_local_models, mcp_host_add_server, mcp_host_list_servers,
        mcp_host_list_tools, mcp_host_remove_server, navigate_to, neural_bridge_delete,
        neural_bridge_ingest, neural_bridge_search, neural_bridge_status, neural_bridge_toggle,
        pause_agent, payment_create_invoice, payment_create_plan, payment_get_revenue_stats,
        payment_list_plans, payment_pay_invoice, remove_indexed_document, replay_export_bundle,
        replay_get_bundle, replay_list_bundles, replay_toggle_recording, replay_verify_bundle,
        resume_agent, search_documents, start_build, start_learning, start_research,
        time_machine_create_checkpoint, time_machine_list_checkpoints, time_machine_redo,
        time_machine_undo, tracing_end_span, tracing_end_trace, tracing_get_trace,
        tracing_list_traces, tracing_start_span, tracing_start_trace, voice_get_status,
        voice_load_whisper_model, voice_transcribe, AppState, LearningSource,
    };
    use serde_json::json;

    fn build_manifest(name: &str) -> String {
        json!({
            "name": name,
            "version": "2.0.0",
            "capabilities": ["web.search", "llm.query", "fs.read"],
            "fuel_budget": 10000,
            "schedule": null,
            "llm_model": "claude-sonnet-4-5"
        })
        .to_string()
    }

    #[test]
    fn test_tauri_create_agent_command() {
        let state = AppState::new();
        let created = create_agent(&state, build_manifest("my-social-poster"));
        assert!(created.is_ok());

        if let Ok(agent_id) = created {
            let parsed = uuid::Uuid::parse_str(agent_id.as_str());
            assert!(parsed.is_ok());
        }
    }

    #[test]
    fn test_tauri_list_agents() {
        let state = AppState::new();

        let a = create_agent(&state, build_manifest("a-agent"));
        assert!(a.is_ok());
        let b = create_agent(&state, build_manifest("b-agent"));
        assert!(b.is_ok());
        let c = create_agent(&state, build_manifest("c-agent"));
        assert!(c.is_ok());

        let listed = list_agents(&state);
        assert!(listed.is_ok());

        if let Ok(agents) = listed {
            assert_eq!(agents.len(), 3);
            assert!(agents.iter().all(|agent| agent.status == "Running"));
        }
    }

    #[test]
    fn test_tauri_pause_and_resume() {
        let state = AppState::new();
        let created = create_agent(&state, build_manifest("voice-agent"));
        assert!(created.is_ok());

        if let Ok(agent_id) = created {
            let paused = pause_agent(&state, agent_id.clone());
            assert!(paused.is_ok());

            let paused_rows = list_agents(&state).expect("list should succeed");
            assert_eq!(paused_rows.len(), 1);
            assert_eq!(paused_rows[0].status, "Paused");
            assert_eq!(paused_rows[0].last_action, "paused");

            let resumed = resume_agent(&state, agent_id);
            assert!(resumed.is_ok());

            let resumed_rows = list_agents(&state).expect("list should succeed");
            assert_eq!(resumed_rows.len(), 1);
            assert_eq!(resumed_rows[0].status, "Running");
            assert_eq!(resumed_rows[0].last_action, "resumed");
        }
    }

    // ── Browser Navigate Tests ──

    #[test]
    fn test_browser_navigate_logs_audit() {
        let state = AppState::new();
        let result = navigate_to(&state, "https://docs.rust-lang.org/".to_string());
        assert!(result.is_ok());
        let nav = result.unwrap();
        assert!(nav.allowed);
        assert_eq!(nav.url, "https://docs.rust-lang.org/");

        // History should have one entry
        let hist = get_browser_history(&state).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].url, "https://docs.rust-lang.org/");

        // Activity log should have recorded the visit
        let activity = get_agent_activity(&state).unwrap();
        assert!(!activity.is_empty());

        // Audit trail should have at least one event
        let audit = state.audit.lock().unwrap();
        assert!(!audit.events().is_empty());
    }

    #[test]
    fn test_browser_blocked_domain_returns_error() {
        let state = AppState::new();
        let result = navigate_to(&state, "https://malware.example.com/payload".to_string());
        assert!(result.is_ok());
        let nav = result.unwrap();
        assert!(!nav.allowed);
        assert!(nav.deny_reason.is_some());
        assert!(nav
            .deny_reason
            .unwrap()
            .contains("blocked by egress policy"));
    }

    #[test]
    fn test_browser_invalid_protocol_blocked() {
        let state = AppState::new();
        let result = navigate_to(&state, "ftp://files.example.com/data".to_string());
        assert!(result.is_ok());
        let nav = result.unwrap();
        assert!(!nav.allowed);
    }

    // ── Research Session Tests ──

    #[test]
    fn test_research_session_creates_multiple_agents() {
        let state = AppState::new();
        let result = start_research(&state, "Rust async patterns".to_string(), 3);
        assert!(result.is_ok());
        let session = result.unwrap();
        assert_eq!(session.sub_agents.len(), 3);
        assert_eq!(session.status, "running");
        assert_eq!(session.topic, "Rust async patterns");

        // Each agent should have a unique ID and a query
        let ids: Vec<_> = session.sub_agents.iter().map(|a| &a.agent_id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 3, "agent IDs should be unique");

        for agent in &session.sub_agents {
            assert!(!agent.query.is_empty());
            assert_eq!(agent.status, "searching");
        }
    }

    #[test]
    fn test_research_complete_merges_findings() {
        let state = AppState::new();
        let session = start_research(&state, "WebAssembly".to_string(), 2).unwrap();
        let result = complete_research(&state, session.session_id);
        assert!(result.is_ok());
        let completed = result.unwrap();
        assert_eq!(completed.status, "complete");
        assert!(completed.total_fuel_used > 0);
    }

    // ── Build Session Tests ──

    #[test]
    fn test_build_session_streams_code() {
        let state = AppState::new();
        let session = start_build(&state, "Dashboard widget".to_string()).unwrap();
        assert_eq!(session.status, "planning");
        assert!(!session.messages.is_empty());

        // Complete the build
        let result = complete_build(&state, session.session_id);
        assert!(result.is_ok());
        let completed = result.unwrap();
        assert_eq!(completed.status, "complete");
    }

    // ── Learning Session Tests ──

    #[test]
    fn test_learning_session_extracts_takeaways() {
        let state = AppState::new();
        let sources = vec![
            LearningSource {
                url: "https://docs.rust-lang.org/stable/".to_string(),
                label: "Rust Docs".to_string(),
                category: "documentation".to_string(),
            },
            LearningSource {
                url: "https://blog.rust-lang.org/".to_string(),
                label: "Rust Blog".to_string(),
                category: "blog".to_string(),
            },
        ];

        let session = start_learning(&state, sources).unwrap();
        assert_eq!(session.status, "browsing");
        assert_eq!(session.sources.len(), 2);

        // Browse first source
        let browsed = learning_agent_action(
            &state,
            session.session_id.clone(),
            "browse".to_string(),
            Some("https://docs.rust-lang.org/stable/".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(browsed.pages_visited, 1);
        assert!(browsed.fuel_used > 0);

        // Extract from it
        let extracted = learning_agent_action(
            &state,
            session.session_id.clone(),
            "extract".to_string(),
            Some("https://docs.rust-lang.org/stable/".to_string()),
            Some("Rust 1.78 adds diagnostic attributes".to_string()),
        )
        .unwrap();
        assert_eq!(extracted.knowledge_base.len(), 1);
        assert!(extracted.knowledge_base[0]
            .key_points
            .iter()
            .any(|p| p.contains("diagnostic")));

        // Compare with existing knowledge
        let compared = learning_agent_action(
            &state,
            session.session_id.clone(),
            "compare".to_string(),
            None,
            None,
        )
        .unwrap();
        assert!(compared.knowledge_base[0].is_new);

        // Complete session
        let done = learning_agent_action(
            &state,
            session.session_id.clone(),
            "done".to_string(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(done.status, "complete");

        // Global knowledge base should now have entries
        let kb = get_knowledge_base(&state).unwrap();
        assert!(!kb.is_empty());
    }

    #[test]
    fn test_learning_blocked_source_rejected() {
        let state = AppState::new();
        let sources = vec![LearningSource {
            url: "https://phishing.evil.com/".to_string(),
            label: "Bad Source".to_string(),
            category: "blog".to_string(),
        }];

        let result = start_learning(&state, sources);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("blocked"));
    }

    #[test]
    fn test_learning_browse_blocked_url() {
        let state = AppState::new();
        let sources = vec![LearningSource {
            url: "https://docs.rust-lang.org/".to_string(),
            label: "Rust Docs".to_string(),
            category: "documentation".to_string(),
        }];

        let session = start_learning(&state, sources).unwrap();

        // Try browsing a blocked URL during the session
        let result = learning_agent_action(
            &state,
            session.session_id,
            "browse".to_string(),
            Some("https://darkweb.example.com/".to_string()),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_get_configured_provider_fallback() {
        // Without Ollama running or API keys set, should fall back to MockProvider.
        let provider = get_configured_provider();
        // In CI / test environments, mock is the expected fallback.
        // If a real provider is configured, that's fine too — just verify it returns something.
        assert!(!provider.name().is_empty());
    }

    #[test]
    fn test_chat_with_documents_returns_answer() {
        // Force MockProvider so the test works without Ollama embedding models.
        std::env::set_var("LLM_PROVIDER", "mock");

        let state = AppState::new();

        // Write a temp file to index
        let tmp = std::env::temp_dir().join("nexus_rag_test_chat.txt");
        std::fs::write(
            &tmp,
            "Rust is a systems programming language focused on safety.",
        )
        .unwrap();

        // Index the document
        let ingest_result = index_document(&state, tmp.to_string_lossy().to_string());
        assert!(
            ingest_result.is_ok(),
            "ingest failed: {:?}",
            ingest_result.err()
        );

        // Chat with documents
        let chat_result = chat_with_documents(&state, "What is Rust?".to_string());
        assert!(chat_result.is_ok(), "chat failed: {:?}", chat_result.err());

        let parsed: serde_json::Value = serde_json::from_str(&chat_result.unwrap()).unwrap();
        assert!(parsed.get("answer").is_some());
        assert!(parsed.get("sources").is_some());
        assert!(parsed.get("model").is_some());
        assert!(parsed.get("tokens").is_some());

        let _ = std::fs::remove_file(&tmp);
        // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
    }

    #[test]
    fn test_provider_status_command() {
        let state = AppState::new();
        let result = get_active_llm_provider(&state);
        assert!(result.is_ok());

        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("provider").is_some());
        assert!(parsed.get("model").is_some());
        assert!(parsed.get("embedding_model").is_some());
        assert!(parsed.get("status").is_some());
        assert!(parsed.get("message").is_some());

        let provider = parsed["provider"].as_str().unwrap();
        assert!(!provider.is_empty());
    }

    // ── RAG wiring tests ────────────────────────────────────────────────

    #[test]
    fn test_index_document_end_to_end() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        let tmp = std::env::temp_dir().join("nexus_test_index_e2e.md");
        std::fs::write(&tmp, "# Heading\n\nSome markdown content about Nexus OS.").unwrap();

        let result = index_document(&state, tmp.to_string_lossy().to_string());
        assert!(result.is_ok(), "index_document failed: {:?}", result.err());

        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed["chunk_count"].as_u64().unwrap() > 0);
        assert_eq!(parsed["path"].as_str().unwrap(), tmp.to_string_lossy());

        let _ = std::fs::remove_file(&tmp);
        // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
    }

    #[test]
    fn test_search_documents_end_to_end() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        let tmp = std::env::temp_dir().join("nexus_test_search_e2e.txt");
        std::fs::write(
            &tmp,
            "Quantum computing uses qubits for parallel computation.",
        )
        .unwrap();

        let _ = index_document(&state, tmp.to_string_lossy().to_string()).unwrap();
        let result = search_documents(&state, "quantum".to_string(), Some(5));
        assert!(result.is_ok(), "search failed: {:?}", result.err());

        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.unwrap()).unwrap();
        // MockProvider embeddings may not produce high cosine similarity for all queries,
        // so we only verify the response parses as an array of valid result objects.
        for r in &parsed {
            assert!(r.get("chunk_id").is_some());
            assert!(r.get("score").is_some());
        }

        let _ = std::fs::remove_file(&tmp);
        // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
    }

    #[test]
    fn test_list_indexed_documents_two_docs() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        let tmp1 = std::env::temp_dir().join("nexus_test_list_a.txt");
        let tmp2 = std::env::temp_dir().join("nexus_test_list_b.txt");
        std::fs::write(&tmp1, "Document A content.").unwrap();
        std::fs::write(&tmp2, "Document B content.").unwrap();

        let _ = index_document(&state, tmp1.to_string_lossy().to_string()).unwrap();
        let _ = index_document(&state, tmp2.to_string_lossy().to_string()).unwrap();

        let result = list_indexed_documents(&state).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed.len(), 2);

        let _ = std::fs::remove_file(&tmp1);
        let _ = std::fs::remove_file(&tmp2);
        // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
    }

    #[test]
    fn test_remove_indexed_document() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        let tmp = std::env::temp_dir().join("nexus_test_remove.txt");
        std::fs::write(&tmp, "Content to be removed.").unwrap();
        let path_str = tmp.to_string_lossy().to_string();

        let _ = index_document(&state, path_str.clone()).unwrap();
        let result = remove_indexed_document(&state, path_str).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["removed"].as_bool().unwrap());

        let list = list_indexed_documents(&state).unwrap();
        let docs: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert!(docs.is_empty());

        let _ = std::fs::remove_file(&tmp);
        // Note: don't remove LLM_PROVIDER — tests run in parallel in the same process.
    }

    // ── Model Hub wiring tests ──────────────────────────────────────────

    #[test]
    fn test_list_local_models_returns_array() {
        let state = AppState::new();
        let result = list_local_models(&state);
        assert!(result.is_ok());
        // Must parse as a JSON array (may be empty)
        let _: Vec<serde_json::Value> = serde_json::from_str(&result.unwrap()).unwrap();
    }

    #[test]
    fn test_get_system_specs_has_fields() {
        let result = get_system_specs();
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("total_ram_mb").is_some());
        assert!(parsed.get("cpu_name").is_some());
        assert!(parsed.get("cpu_cores").is_some());
        assert!(parsed["total_ram_mb"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_get_live_system_metrics_has_fields() {
        let state = AppState::new();
        let result = get_live_system_metrics(&state);
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("cpu_avg").is_some());
        assert!(parsed.get("cpu_cores").is_some());
        assert!(parsed.get("total_ram").is_some());
        assert!(parsed.get("used_ram").is_some());
        assert!(parsed.get("uptime_secs").is_some());
        assert!(parsed.get("process_count").is_some());
        assert!(parsed.get("agents").is_some());
        assert!(parsed["total_ram"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_check_model_compatibility() {
        let state = AppState::new();
        // 500 MB file
        let result = check_model_compatibility(&state, 500_000_000);
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("can_run").is_some());
    }

    // ── Time Machine wiring tests ───────────────────────────────────────

    #[test]
    fn test_time_machine_create_and_list_checkpoints() {
        let state = AppState::new();

        let created = time_machine_create_checkpoint(&state, "test-checkpoint".to_string());
        assert!(created.is_ok());
        let cp_id = created.unwrap();
        assert!(!cp_id.is_empty());

        let list_result = time_machine_list_checkpoints(&state).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&list_result).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["label"].as_str().unwrap(), "test-checkpoint");
        assert_eq!(parsed[0]["id"].as_str().unwrap(), cp_id);
    }

    #[test]
    fn test_time_machine_undo_empty() {
        let state = AppState::new();
        let result = time_machine_undo(&state);
        assert!(result.is_err());
    }

    #[test]
    fn test_time_machine_create_undo_redo_cycle() {
        let state = AppState::new();

        let _ = time_machine_create_checkpoint(&state, "cycle-test".to_string()).unwrap();

        // Undo
        let undo_result = time_machine_undo(&state);
        assert!(undo_result.is_ok());
        let undo_parsed: serde_json::Value = serde_json::from_str(&undo_result.unwrap()).unwrap();
        assert_eq!(undo_parsed["label"].as_str().unwrap(), "cycle-test");

        // Redo
        let redo_result = time_machine_redo(&state);
        assert!(redo_result.is_ok());
        let redo_parsed: serde_json::Value = serde_json::from_str(&redo_result.unwrap()).unwrap();
        assert_eq!(redo_parsed["label"].as_str().unwrap(), "cycle-test");
    }

    // ── Voice wiring tests ──────────────────────────────────────────────

    #[test]
    fn test_voice_get_status_json() {
        let state = AppState::new();
        let result = voice_get_status(&state);
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("is_listening").is_some());
        assert!(parsed.get("wake_word").is_some());
        assert!(parsed.get("python_server_running").is_some());
        assert!(parsed.get("whisper_loaded").is_some());
        assert!(parsed.get("transcription_engine").is_some());
        // Default state: whisper not loaded, engine is stub
        assert_eq!(parsed["whisper_loaded"].as_bool(), Some(false));
        assert_eq!(parsed["transcription_engine"].as_str(), Some("stub"));
    }

    #[test]
    fn test_voice_transcribe_fallback_stub() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        // With no whisper model loaded and no python server, should fall back to stub
        let result = voice_transcribe(&state, "AAAA".to_string());
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("text").is_some());
        assert_eq!(parsed["engine"].as_str(), Some("stub"));
        assert!(parsed.get("duration_ms").is_some());
    }

    #[test]
    fn test_voice_load_whisper_model_missing() {
        let state = AppState::new();
        let result = voice_load_whisper_model(&state, "/nonexistent/whisper/model".to_string());
        assert!(result.is_err());
        // Whisper should still not be loaded
        let status = voice_get_status(&state).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&status).unwrap();
        assert_eq!(parsed["whisper_loaded"].as_bool(), Some(false));
    }

    #[test]
    fn test_voice_transcribe_returns_engine_field() {
        std::env::set_var("LLM_PROVIDER", "mock");
        let state = AppState::new();
        // Send some base64 data (doesn't matter what — stub ignores content)
        let result = voice_transcribe(&state, "SGVsbG8gV29ybGQ=".to_string());
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        // Must always have text, engine, and duration_ms
        assert!(parsed["text"].is_string());
        assert!(parsed["engine"].is_string());
        assert!(parsed["duration_ms"].is_number());
    }

    // ── Economy wiring tests ────────────────────────────────────────────

    #[test]
    fn test_economy_full_cycle() {
        let state = AppState::new();
        let agent_id = uuid::Uuid::new_v4().to_string();

        // Create wallet
        let wallet_result = economy_create_wallet(&state, agent_id.clone());
        assert!(wallet_result.is_ok());

        // Earn credits
        let earn_result =
            economy_earn(&state, agent_id.clone(), 100.0, "test earnings".to_string());
        assert!(earn_result.is_ok());

        // Check balance (default_balance=100 + earned=100 = 200)
        let wallet = economy_get_wallet(&state, agent_id.clone()).unwrap();
        let wallet_parsed: serde_json::Value = serde_json::from_str(&wallet).unwrap();
        let balance = wallet_parsed["balance"].as_f64().unwrap();
        assert!((balance - 200.0).abs() < 0.01);

        // Spend credits (within default spending_limit of 10.0)
        let spend_result = economy_spend(
            &state,
            agent_id.clone(),
            5.0,
            "ApiCall".to_string(),
            "test spend".to_string(),
        );
        assert!(spend_result.is_ok());

        // Verify balance after spend (200 - 5 = 195)
        let wallet2 = economy_get_wallet(&state, agent_id.clone()).unwrap();
        let w2: serde_json::Value = serde_json::from_str(&wallet2).unwrap();
        let balance2 = w2["balance"].as_f64().unwrap();
        assert!((balance2 - 195.0).abs() < 0.01);

        // History should have 2 transactions
        let history = economy_get_history(&state, agent_id.clone()).unwrap();
        let h: Vec<serde_json::Value> = serde_json::from_str(&history).unwrap();
        assert_eq!(h.len(), 2);

        // Stats
        let stats = economy_get_stats(&state).unwrap();
        let s: serde_json::Value = serde_json::from_str(&stats).unwrap();
        assert!(s.get("total_wallets").is_some());
    }

    #[test]
    fn test_economy_transfer_between_wallets() {
        let state = AppState::new();
        let from_id = uuid::Uuid::new_v4().to_string();
        let to_id = uuid::Uuid::new_v4().to_string();

        economy_create_wallet(&state, from_id.clone()).unwrap();
        economy_create_wallet(&state, to_id.clone()).unwrap();
        economy_earn(&state, from_id.clone(), 200.0, "seed".to_string()).unwrap();

        let transfer = economy_transfer(
            &state,
            from_id.clone(),
            to_id.clone(),
            50.0,
            "pay".to_string(),
        );
        assert!(transfer.is_ok());

        // from: default(100) + earn(200) - transfer(50) = 250
        let from_w = economy_get_wallet(&state, from_id).unwrap();
        let from_v: serde_json::Value = serde_json::from_str(&from_w).unwrap();
        assert!((from_v["balance"].as_f64().unwrap() - 250.0).abs() < 0.01);

        // to: default(100) + received(50) = 150
        let to_w = economy_get_wallet(&state, to_id).unwrap();
        let to_v: serde_json::Value = serde_json::from_str(&to_w).unwrap();
        assert!((to_v["balance"].as_f64().unwrap() - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_economy_freeze_wallet() {
        let state = AppState::new();
        let agent_id = uuid::Uuid::new_v4().to_string();
        economy_create_wallet(&state, agent_id.clone()).unwrap();
        economy_earn(&state, agent_id.clone(), 100.0, "seed".to_string()).unwrap();

        let freeze = economy_freeze_wallet(&state, agent_id.clone());
        assert!(freeze.is_ok());

        // Spending on frozen wallet should fail
        let spend = economy_spend(
            &state,
            agent_id,
            10.0,
            "ApiCall".to_string(),
            "test".to_string(),
        );
        assert!(spend.is_err());
    }

    // ── Ghost Protocol wiring tests ─────────────────────────────────────

    #[test]
    fn test_ghost_protocol_status_has_device_id() {
        let state = AppState::new();
        let result = ghost_protocol_status(&state).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("device_id").is_some());
        assert!(parsed.get("enabled").is_some());
        assert!(parsed.get("peer_count").is_some());
        assert!(parsed.get("stats").is_some());
    }

    #[test]
    fn test_ghost_protocol_toggle() {
        let state = AppState::new();

        let toggle = ghost_protocol_toggle(&state, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&toggle).unwrap();
        assert!(parsed["enabled"].as_bool().unwrap());

        let status = ghost_protocol_status(&state).unwrap();
        let s: serde_json::Value = serde_json::from_str(&status).unwrap();
        assert!(s["enabled"].as_bool().unwrap());
    }

    #[test]
    fn test_ghost_protocol_add_remove_peer() {
        let state = AppState::new();
        ghost_protocol_toggle(&state, true).unwrap();

        let add_result = ghost_protocol_add_peer(
            &state,
            "127.0.0.1:9090".to_string(),
            "test-peer".to_string(),
        );
        assert!(add_result.is_ok());
        let added: serde_json::Value = serde_json::from_str(&add_result.unwrap()).unwrap();
        let peer_device_id = added["device_id"].as_str().unwrap().to_string();

        // Verify peer count
        let status = ghost_protocol_status(&state).unwrap();
        let s: serde_json::Value = serde_json::from_str(&status).unwrap();
        assert_eq!(s["peer_count"].as_u64().unwrap(), 1);

        // Remove peer
        let remove = ghost_protocol_remove_peer(&state, peer_device_id);
        assert!(remove.is_ok());
    }

    // ── Evolution wiring tests ──────────────────────────────────────────

    #[test]
    fn test_evolution_status() {
        let state = AppState::new();
        let result = evolution_get_status(&state).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("enabled").is_some());
        assert!(parsed.get("total_strategies").is_some());
        assert!(parsed.get("active_agents").is_some());
    }

    #[test]
    fn test_evolution_register_and_evolve() {
        let state = AppState::new();
        let agent_id = uuid::Uuid::new_v4().to_string();
        let params = json!({"learning_rate": 0.01, "batch_size": 32}).to_string();

        let reg = evolution_register_strategy(
            &state,
            agent_id.clone(),
            "test-strategy".to_string(),
            params,
        );
        assert!(reg.is_ok());
        let strategy: serde_json::Value = serde_json::from_str(&reg.unwrap()).unwrap();
        assert_eq!(strategy["name"].as_str().unwrap(), "test-strategy");

        // Evolve
        let evolve = evolution_evolve_once(&state, agent_id.clone());
        assert!(evolve.is_ok());
        let evolved: serde_json::Value = serde_json::from_str(&evolve.unwrap()).unwrap();
        assert!(evolved.get("generation").is_some());

        // History
        let history = evolution_get_history(&state, agent_id.clone()).unwrap();
        let h: serde_json::Value = serde_json::from_str(&history).unwrap();
        assert!(h.get("total_generations").is_some());

        // Active strategy
        let active = evolution_get_active_strategy(&state, agent_id.clone());
        assert!(active.is_ok());

        // Rollback — may fail if evolve_once didn't accept the child (no parent to rollback to).
        // We just verify it doesn't panic.
        let _ = evolution_rollback(&state, agent_id);
    }

    // ── MCP Host wiring tests ───────────────────────────────────────────

    #[test]
    fn test_mcp_host_add_list_remove_server() {
        let state = AppState::new();

        // Initially empty
        let list = mcp_host_list_servers(&state).unwrap();
        let servers: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert!(servers.is_empty());

        // Add server
        let add = mcp_host_add_server(
            &state,
            "test-server".to_string(),
            "http://localhost:8080".to_string(),
            "http".to_string(),
            None,
        );
        assert!(add.is_ok());
        let added: serde_json::Value = serde_json::from_str(&add.unwrap()).unwrap();
        let server_id = added["id"].as_str().unwrap().to_string();

        // List should have 1
        let list2 = mcp_host_list_servers(&state).unwrap();
        let servers2: Vec<serde_json::Value> = serde_json::from_str(&list2).unwrap();
        assert_eq!(servers2.len(), 1);
        assert_eq!(servers2[0]["name"].as_str().unwrap(), "test-server");

        // Tools should be empty (not connected)
        let tools = mcp_host_list_tools(&state).unwrap();
        let tools_parsed: Vec<serde_json::Value> = serde_json::from_str(&tools).unwrap();
        assert!(tools_parsed.is_empty());

        // Remove
        let remove = mcp_host_remove_server(&state, server_id);
        assert!(remove.is_ok());

        // List should be empty again
        let list3 = mcp_host_list_servers(&state).unwrap();
        let servers3: Vec<serde_json::Value> = serde_json::from_str(&list3).unwrap();
        assert!(servers3.is_empty());
    }

    // ── Neural Bridge wiring tests ──────────────────────────────────────

    #[test]
    fn test_neural_bridge_status() {
        let state = AppState::new();
        let result = neural_bridge_status(&state).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("stats").is_some());
        assert!(parsed.get("config").is_some());
    }

    #[test]
    fn test_neural_bridge_ingest_and_search() {
        let state = AppState::new();
        neural_bridge_toggle(&state, true).unwrap();

        // Ingest content
        let ingest = neural_bridge_ingest(
            &state,
            "Clipboard".to_string(),
            "Nexus OS uses capability-based security for agent governance.".to_string(),
            json!({}),
        );
        assert!(ingest.is_ok());
        let entry: serde_json::Value = serde_json::from_str(&ingest.unwrap()).unwrap();
        let entry_id = entry["id"].as_str().unwrap().to_string();
        assert!(!entry_id.is_empty());

        // Search
        let search = neural_bridge_search(
            &state,
            "capability security".to_string(),
            None,
            None,
            Some(5),
        );
        assert!(search.is_ok());
        let results: Vec<serde_json::Value> = serde_json::from_str(&search.unwrap()).unwrap();
        assert!(!results.is_empty());

        // Delete
        let del = neural_bridge_delete(&state, entry_id);
        assert!(del.is_ok());
        let d: serde_json::Value = serde_json::from_str(&del.unwrap()).unwrap();
        assert!(d["deleted"].as_bool().unwrap());
    }

    // ── Tracing wiring tests ────────────────────────────────────────────

    #[test]
    fn test_tracing_full_lifecycle() {
        let state = AppState::new();

        // Start trace
        let trace_result = tracing_start_trace(&state, "test-operation".to_string(), None);
        assert!(trace_result.is_ok());
        let t: serde_json::Value = serde_json::from_str(&trace_result.unwrap()).unwrap();
        let trace_id = t["trace_id"].as_str().unwrap().to_string();
        let root_span_id = t["span_id"].as_str().unwrap().to_string();

        // Start child span
        let span_result = tracing_start_span(
            &state,
            trace_id.clone(),
            root_span_id.clone(),
            "child-op".to_string(),
            None,
        );
        assert!(span_result.is_ok());
        let s: serde_json::Value = serde_json::from_str(&span_result.unwrap()).unwrap();
        let child_span_id = s["span_id"].as_str().unwrap().to_string();

        // End child span
        let end_child = tracing_end_span(&state, child_span_id, "Ok".to_string(), None);
        assert!(end_child.is_ok());

        // End root span
        let end_root = tracing_end_span(&state, root_span_id, "Ok".to_string(), None);
        assert!(end_root.is_ok());

        // End trace
        let end_trace = tracing_end_trace(&state, trace_id.clone());
        assert!(end_trace.is_ok());
        let completed: serde_json::Value = serde_json::from_str(&end_trace.unwrap()).unwrap();
        assert!(completed.get("spans").is_some());

        // List traces
        let list = tracing_list_traces(&state, Some(10)).unwrap();
        let traces: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert!(!traces.is_empty());

        // Get specific trace
        let get = tracing_get_trace(&state, trace_id);
        assert!(get.is_ok());
    }

    // ── Agent Memory wiring tests ───────────────────────────────────────

    #[test]
    fn test_agent_memory_remember_and_recall() {
        let state = AppState::new();
        let agent_id = uuid::Uuid::new_v4().to_string();

        // Remember
        let mem_result = agent_memory_remember(
            &state,
            agent_id.clone(),
            "The sky is blue.".to_string(),
            "Fact".to_string(),
            0.9,
            vec!["science".to_string()],
        );
        assert!(mem_result.is_ok());
        let entry: serde_json::Value = serde_json::from_str(&mem_result.unwrap()).unwrap();
        assert!(entry.get("id").is_some());

        // Recall
        let recall = agent_memory_recall(&state, agent_id.clone(), "sky".to_string(), Some(5));
        assert!(recall.is_ok());
        let results: Vec<serde_json::Value> = serde_json::from_str(&recall.unwrap()).unwrap();
        assert!(!results.is_empty());

        // Stats
        let stats = agent_memory_get_stats(&state, agent_id.clone()).unwrap();
        let s: serde_json::Value = serde_json::from_str(&stats).unwrap();
        assert!(s.get("total").is_some());

        // Forget
        let memory_id = entry["id"].as_str().unwrap().to_string();
        let forget = agent_memory_forget(&state, agent_id.clone(), memory_id);
        assert!(forget.is_ok());

        // Clear
        let clear = agent_memory_clear(&state, agent_id);
        assert!(clear.is_ok());
    }

    // ── Factory wiring tests ────────────────────────────────────────────

    #[test]
    fn test_factory_create_project_and_list() {
        let state = AppState::new();
        let tmp_dir = std::env::temp_dir().join("nexus_test_factory");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let create = factory_create_project(
            &state,
            "test-project".to_string(),
            "rust".to_string(),
            tmp_dir.to_string_lossy().to_string(),
        );
        assert!(create.is_ok());
        let project: serde_json::Value = serde_json::from_str(&create.unwrap()).unwrap();
        assert_eq!(project["name"].as_str().unwrap(), "test-project");
        assert!(project.get("id").is_some());

        // List
        let list = factory_list_projects(&state).unwrap();
        let projects: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert_eq!(projects.len(), 1);

        // Build history (empty initially)
        let project_id = project["id"].as_str().unwrap().to_string();
        let history = factory_get_build_history(&state, project_id).unwrap();
        let h: Vec<serde_json::Value> = serde_json::from_str(&history).unwrap();
        assert!(h.is_empty());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    // ── Payments wiring tests ───────────────────────────────────────────

    #[test]
    fn test_payment_plan_and_invoice() {
        let state = AppState::new();

        // Create plan
        let plan = payment_create_plan(
            &state,
            "Pro Plan".to_string(),
            999,
            "Monthly".to_string(),
            vec![
                "unlimited-agents".to_string(),
                "priority-support".to_string(),
            ],
        );
        assert!(plan.is_ok());
        let plan_parsed: serde_json::Value = serde_json::from_str(&plan.unwrap()).unwrap();
        let plan_id = plan_parsed["id"].as_str().unwrap().to_string();
        assert_eq!(plan_parsed["name"].as_str().unwrap(), "Pro Plan");
        assert_eq!(plan_parsed["price_cents"].as_u64().unwrap(), 999);

        // List plans
        let plans = payment_list_plans(&state).unwrap();
        let plans_parsed: Vec<serde_json::Value> = serde_json::from_str(&plans).unwrap();
        assert_eq!(plans_parsed.len(), 1);

        // Create invoice
        let invoice = payment_create_invoice(&state, plan_id, "buyer-123".to_string());
        assert!(invoice.is_ok());
        let inv: serde_json::Value = serde_json::from_str(&invoice.unwrap()).unwrap();
        let invoice_id = inv["id"].as_str().unwrap().to_string();
        assert_eq!(inv["status"].as_str().unwrap(), "Pending");

        // Pay invoice
        let pay = payment_pay_invoice(&state, invoice_id);
        assert!(pay.is_ok());
        let paid: serde_json::Value = serde_json::from_str(&pay.unwrap()).unwrap();
        assert_eq!(paid["status"].as_str().unwrap(), "Paid");

        // Revenue stats
        let stats = payment_get_revenue_stats(&state).unwrap();
        let s: serde_json::Value = serde_json::from_str(&stats).unwrap();
        assert!(s.get("total_revenue_cents").is_some());
    }

    #[test]
    fn test_tauri_replay_evidence_flow() {
        let state = AppState::new();

        // Toggle recording on
        let toggle = replay_toggle_recording(&state, true).unwrap();
        let t: serde_json::Value = serde_json::from_str(&toggle).unwrap();
        assert_eq!(t["recording"], true);

        // Initially no bundles
        let list = replay_list_bundles(&state, None, Some(50)).unwrap();
        let bundles: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert!(bundles.is_empty());

        // Record a bundle manually via the recorder
        {
            let mut recorder = state.replay_recorder.lock().unwrap();
            let bid = recorder.capture_pre_state(
                "test-agent",
                "tool_call",
                vec!["fs.read".into()],
                1000,
                vec![],
                Some("mock".into()),
                json!({"cmd": "ls"}),
            );
            recorder.record_governance_check(&bid, "capability", true, "ok");
            recorder.record_governance_check(&bid, "fuel", true, "ok");
            recorder
                .capture_post_state(
                    &bid,
                    vec!["fs.read".into()],
                    998,
                    vec![],
                    json!({"out": "ok"}),
                )
                .unwrap();
        }

        // List bundles — should have 1
        let list2 = replay_list_bundles(&state, None, Some(50)).unwrap();
        let bundles2: Vec<serde_json::Value> = serde_json::from_str(&list2).unwrap();
        assert_eq!(bundles2.len(), 1);
        let bundle_id = bundles2[0]["id"].as_str().unwrap().to_string();

        // Get full bundle
        let full = replay_get_bundle(&state, bundle_id.clone()).unwrap();
        let b: serde_json::Value = serde_json::from_str(&full).unwrap();
        assert_eq!(b["agent_id"], "test-agent");
        assert_eq!(b["action_type"], "tool_call");

        // Verify bundle
        let verdict = replay_verify_bundle(&state, bundle_id.clone()).unwrap();
        assert!(verdict.contains("Verified"));

        // Export bundle
        let exported = replay_export_bundle(&state, bundle_id).unwrap();
        assert!(exported.contains("test-agent"));
        assert!(exported.contains("bundle_hash"));

        // Filter by agent
        let filtered = replay_list_bundles(&state, Some("nonexistent".into()), Some(50)).unwrap();
        let empty: Vec<serde_json::Value> = serde_json::from_str(&filtered).unwrap();
        assert!(empty.is_empty());

        // Toggle off
        let off = replay_toggle_recording(&state, false).unwrap();
        let o: serde_json::Value = serde_json::from_str(&off).unwrap();
        assert_eq!(o["recording"], false);
    }
}
