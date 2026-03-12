use nexus_adaptation::evolution::{EvolutionConfig, EvolutionEngine, MutationType, Strategy};
use nexus_connectors_llm::chunking::SupportedFormat;
use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::model_hub::{self, DownloadProgress, DownloadStatus};
use nexus_connectors_llm::model_registry::ModelRegistry;
use nexus_connectors_llm::nexus_link::NexusLink;
use nexus_connectors_llm::providers::{MockProvider, OllamaProvider};
use nexus_connectors_llm::rag::{RagConfig, RagPipeline};
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
        }
    }

    fn log_event(&self, agent_id: AgentId, event_type: EventType, payload: serde_json::Value) {
        let mut guard = match self.audit.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .append_event(agent_id, event_type, payload)
            .expect("audit: fail-closed — no unrecorded operations allowed");
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
        let session = lm.sessions.get_mut(&session_id).unwrap();

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

    let provider = MockProvider::new();
    let mut rag = state.rag.lock().unwrap_or_else(|p| p.into_inner());
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

    let provider = MockProvider::new();
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
    let provider = MockProvider::new();

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

    state.log_event(
        Uuid::new_v4(),
        EventType::ToolCall,
        json!({
            "event": "rag.query",
            "question_len": question.len(),
            "chunk_count": chunk_count,
        }),
    );

    let response = json!({
        "prompt": prompt,
        "sources": sources,
        "chunk_count": chunk_count,
    });

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
            return Ok(serde_json::to_string(
                &json!({"deleted": false, "model_id": &model_id, "error": "model not found"}),
            )
            .unwrap());
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
            Ok(serde_json::to_string(&json!({"deleted": true, "model_id": &model_id})).unwrap())
        }
        Err(e) => Ok(serde_json::to_string(
            &json!({"deleted": false, "model_id": &model_id, "error": e.to_string()}),
        )
        .unwrap()),
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

    Ok(serde_json::to_string(&json!({
        "total_ram_mb": sys.total_memory() / (1024 * 1024),
        "available_ram_mb": sys.available_memory() / (1024 * 1024),
        "cpu_name": cpu_name,
        "cpu_cores": cpu_cores,
    }))
    .unwrap())
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
    let id = supervisor
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

    let result = json!({
        "is_listening": voice.wake_word_enabled,
        "wake_word": "nexus",
        "python_server_running": vp.running,
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

pub fn voice_transcribe(_state: &AppState, audio_base64: String) -> Result<String, String> {
    // Stub transcription — returns a placeholder.
    // Real implementation will forward to the Python voice server via WebSocket
    // or route through the LLM gateway for cloud/local STT.
    let decoded_len = audio_base64.len() * 3 / 4;
    let size_kb = decoded_len as f64 / 1024.0;

    let result = json!({
        "text": format!("[transcription placeholder — {:.1} KB audio received]", size_kb),
    });
    serde_json::to_string(&result).map_err(|e| e.to_string())
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
                search_models,
                get_model_info,
                check_model_compatibility,
                download_model,
                list_local_models,
                delete_local_model,
                get_system_specs,
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
                factory_create_project,
                factory_build_project,
                factory_test_project,
                factory_run_pipeline,
                factory_list_projects,
                factory_get_build_history,
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
        complete_build, complete_research, create_agent, get_agent_activity, get_browser_history,
        get_knowledge_base, learning_agent_action, list_agents, navigate_to, pause_agent,
        resume_agent, start_build, start_learning, start_research, AppState, LearningSource,
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
}
