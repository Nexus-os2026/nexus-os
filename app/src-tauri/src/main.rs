use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub fuel_budget: u64,
    pub llm_model: Option<String>,
}

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

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn start_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    voice.overlay_visible = true;
    Ok(voice.clone())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn stop_jarvis_mode(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let mut voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    voice.overlay_visible = false;
    Ok(voice.clone())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn jarvis_status(state: &AppState) -> Result<VoiceRuntimeState, String> {
    let voice = match state.voice.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    Ok(voice.clone())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn create_agent(state: &AppState, request: CreateAgentRequest) -> Result<String, String> {
    let manifest = AgentManifest {
        name: request.name.clone(),
        version: "0.13.0".to_string(),
        capabilities: vec![
            "web.search".to_string(),
            "llm.query".to_string(),
            "fs.read".to_string(),
        ],
        fuel_budget: request.fuel_budget,
        schedule: None,
        llm_model: request.llm_model,
    };

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
            name: request.name,
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

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn start_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    supervisor.restart_agent(parsed).map_err(agent_error)?;

    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&parsed) {
        meta.last_action = "started".to_string();
    }

    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "start_agent", "status": "ok"}),
    );

    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn stop_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    supervisor.stop_agent(parsed).map_err(agent_error)?;

    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&parsed) {
        meta.last_action = "stopped".to_string();
    }

    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "stop_agent", "status": "ok"}),
    );

    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn pause_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    supervisor.pause_agent(parsed).map_err(agent_error)?;

    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&parsed) {
        meta.last_action = "paused".to_string();
    }

    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "pause_agent", "status": "ok"}),
    );

    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn resume_agent(state: &AppState, agent_id: String) -> Result<(), String> {
    let parsed = parse_agent_id(agent_id.as_str())?;

    let mut supervisor = match state.supervisor.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    supervisor.resume_agent(parsed).map_err(agent_error)?;

    let mut meta_guard = match state.meta.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(meta) = meta_guard.get_mut(&parsed) {
        meta.last_action = "resumed".to_string();
    }

    state.log_event(
        parsed,
        EventType::StateChange,
        json!({"event": "resume_agent", "status": "ok"}),
    );

    Ok(())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
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

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn get_audit_log(state: &AppState) -> Result<Vec<AuditRow>, String> {
    let guard = match state.audit.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    Ok(guard.events().iter().map(event_to_row).collect::<Vec<_>>())
}

#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub fn tray_status(state: &AppState) -> Result<TrayStatus, String> {
    let agents = list_agents(state)?;
    let running_agents = agents
        .iter()
        .filter(|agent| agent.status == "Running")
        .count();

    Ok(TrayStatus {
        running_agents,
        menu_items: vec!["Dashboard".to_string(), "Quit".to_string()],
    })
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

fn main() {
    println!("NEXUS OS desktop backend (tauri-runtime disabled in this build)");
}

#[cfg(test)]
mod tests {
    use super::{
        create_agent, list_agents, pause_agent, resume_agent, AppState, CreateAgentRequest,
    };

    fn build_request(name: &str) -> CreateAgentRequest {
        CreateAgentRequest {
            name: name.to_string(),
            fuel_budget: 10_000,
            llm_model: Some("claude-sonnet-4-5".to_string()),
        }
    }

    #[test]
    fn test_tauri_create_agent_command() {
        let state = AppState::new();
        let created = create_agent(&state, build_request("my-social-poster"));
        assert!(created.is_ok());

        if let Ok(agent_id) = created {
            let parsed = uuid::Uuid::parse_str(agent_id.as_str());
            assert!(parsed.is_ok());
        }
    }

    #[test]
    fn test_tauri_list_agents() {
        let state = AppState::new();

        let a = create_agent(&state, build_request("a"));
        assert!(a.is_ok());
        let b = create_agent(&state, build_request("b"));
        assert!(b.is_ok());
        let c = create_agent(&state, build_request("c"));
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
        let created = create_agent(&state, build_request("voice-agent"));
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
