use nexus_connectors_llm::gateway::{
    select_provider, AgentRuntimeContext, GovernedLlmGateway, ProviderSelectionConfig,
};
use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::config::{load_config, save_config as save_nexus_config, NexusConfig};
use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
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
