use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_connectors_llm::providers::OllamaProvider;
use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::config::{
    load_config, save_config as save_nexus_config, AgentLlmConfig, HardwareConfig, ModelsConfig,
    NexusConfig, OllamaConfig,
};
use nexus_kernel::errors::AgentError;
use nexus_kernel::hardware::{recommend_agent_configs, HardwareProfile};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::permissions::{
    CapabilityRequest as KernelCapabilityRequest, PermissionCategory as KernelPermissionCategory,
    PermissionHistoryEntry as KernelPermissionHistoryEntry,
};
use nexus_kernel::supervisor::{AgentId, Supervisor};
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
    pub last_action: String,
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

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let agent_id = supervisor.start_agent(manifest).map_err(agent_error)?;

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
        json!({"event": "create_agent", "status": "ok"}),
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

    let mut rows = statuses
        .into_iter()
        .map(|status| {
            let meta = meta_guard.get(&status.id).cloned().unwrap_or(AgentMeta {
                name: "unknown".to_string(),
                last_action: "none".to_string(),
            });
            AgentRow {
                id: status.id.to_string(),
                name: meta.name,
                status: status.state.to_string(),
                fuel_remaining: status.remaining_fuel,
                last_action: meta.last_action,
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

pub fn send_chat(state: &AppState, message: String) -> Result<ChatResponse, String> {
    let config = load_config().map_err(agent_error)?;
    let provider_config = ProviderSelectionConfig {
        provider: std::env::var("LLM_PROVIDER").ok(),
        ollama_url: if config.llm.ollama_url.trim().is_empty() {
            None
        } else {
            Some(config.llm.ollama_url.clone())
        },
        deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
        anthropic_api_key: if config.llm.anthropic_api_key.trim().is_empty() {
            None
        } else {
            Some(config.llm.anthropic_api_key.clone())
        },
    };
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

/// Stream a chat completion through Ollama. Returns the full response text.
/// The `on_token` callback is called with each token for streaming to the frontend.
pub fn chat_with_ollama_streaming<F>(
    messages: Vec<serde_json::Value>,
    model: String,
    base_url: Option<String>,
    mut on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let provider = OllamaProvider::new(&url);

    // Ensure Ollama is running first
    if !provider.health_check().unwrap_or(false) {
        return Err("Ollama is not running. Start it with: ollama serve".into());
    }

    provider
        .chat_stream(&messages, &model, |token| {
            on_token(token);
        })
        .map_err(|e| e.to_string())
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
        state: tauri::State<'_, AppState>,
        manifest_json: String,
    ) -> Result<String, String> {
        super::create_agent(state.inner(), manifest_json)
    }

    #[tauri::command]
    fn start_agent(state: tauri::State<'_, AppState>, agent_id: String) -> Result<(), String> {
        super::start_agent(state.inner(), agent_id)
    }

    #[tauri::command]
    fn stop_agent(state: tauri::State<'_, AppState>, agent_id: String) -> Result<(), String> {
        super::stop_agent(state.inner(), agent_id)
    }

    #[tauri::command]
    fn pause_agent(state: tauri::State<'_, AppState>, agent_id: String) -> Result<(), String> {
        super::pause_agent(state.inner(), agent_id)
    }

    #[tauri::command]
    fn resume_agent(state: tauri::State<'_, AppState>, agent_id: String) -> Result<(), String> {
        super::resume_agent(state.inner(), agent_id)
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
        messages: Vec<serde_json::Value>,
        model: String,
        base_url: Option<String>,
    ) -> Result<String, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let mut last_emit = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now);
            let mut full = String::new();

            let result = super::chat_with_ollama_streaming(messages, model, base_url, |token| {
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
    use super::{create_agent, list_agents, pause_agent, resume_agent, AppState};
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
}
