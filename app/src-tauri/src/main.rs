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

#[derive(Clone, Default)]
pub struct AppState {
    supervisor: Arc<Mutex<Supervisor>>,
    audit: Arc<Mutex<AuditTrail>>,
    meta: Arc<Mutex<HashMap<AgentId, AgentMeta>>>,
    voice: Arc<Mutex<VoiceRuntimeState>>,
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
        }
    }

    fn log_event(&self, agent_id: AgentId, event_type: EventType, payload: serde_json::Value) {
        let mut guard = match self.audit.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _ = guard.append_event(agent_id, event_type, payload);
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
