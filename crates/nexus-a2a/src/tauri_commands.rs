//! Tauri command handlers for the A2A crate.
//!
//! Provides 6 commands for the desktop frontend:
//! - `a2a_get_agent_card` — return this instance's composite AgentCard
//! - `a2a_list_skills` — list all agent skills in the registry
//! - `a2a_send_task` — send a task to an external A2A agent
//! - `a2a_get_task` — get task status from the bridge
//! - `a2a_discover_agent` — discover an external agent by URL
//! - `a2a_get_status` — server status summary

use crate::bridge::A2aBridge;
use crate::server::SkillRegistry;
use crate::types::{A2aClient, A2aServerStatus, AgentCard, SkillSummary, A2A_PROTOCOL_VERSION};
use std::sync::Mutex;

/// In-memory state held by the Tauri app for A2A.
pub struct A2aState {
    pub client: Mutex<A2aClient>,
    pub registry: Mutex<SkillRegistry>,
    pub bridge: Mutex<A2aBridge>,
}

impl A2aState {
    pub fn new(instance_name: &str, base_url: &str) -> Self {
        Self {
            client: Mutex::new(A2aClient::new()),
            registry: Mutex::new(SkillRegistry::new(instance_name, base_url)),
            bridge: Mutex::new(A2aBridge::new()),
        }
    }
}

impl Default for A2aState {
    fn default() -> Self {
        Self::new("nexus-os", "http://localhost:9090")
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// Return this Nexus OS instance's composite AgentCard.
pub fn a2a_crate_get_agent_card(state: &A2aState) -> Result<AgentCard, String> {
    let registry = state.registry.lock().map_err(|e| format!("lock: {e}"))?;
    Ok(registry.build_instance_card())
}

/// List all skills across all registered agents.
pub fn a2a_crate_list_skills(state: &A2aState) -> Result<Vec<SkillSummary>, String> {
    let registry = state.registry.lock().map_err(|e| format!("lock: {e}"))?;
    Ok(registry.all_skill_summaries())
}

/// Send a task to an external A2A-compatible agent.
pub fn a2a_crate_send_task(
    state: &A2aState,
    agent_url: &str,
    message: &str,
) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().map_err(|e| format!("lock: {e}"))?;
    let result = client
        .send_task(agent_url, message)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

/// Get task status — first checks the local bridge, then falls back to
/// querying a remote agent URL.
pub fn a2a_crate_get_task(
    state: &A2aState,
    task_id: &str,
    agent_url: Option<String>,
) -> Result<serde_json::Value, String> {
    // Check local bridge first
    {
        let bridge = state.bridge.lock().map_err(|e| format!("lock: {e}"))?;
        if let Some(routed) = bridge.get_task(task_id) {
            return serde_json::to_value(routed).map_err(|e| e.to_string());
        }
    }

    // Fall back to remote query
    if let Some(url) = agent_url {
        let mut client = state.client.lock().map_err(|e| format!("lock: {e}"))?;
        let result = client
            .get_task_status(&url, task_id)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&result).map_err(|e| e.to_string())
    } else {
        Err(format!(
            "Task '{task_id}' not found in local bridge and no agent_url provided"
        ))
    }
}

/// Discover an external A2A-compatible agent by URL.
pub fn a2a_crate_discover_agent(state: &A2aState, url: &str) -> Result<serde_json::Value, String> {
    let mut client = state.client.lock().map_err(|e| format!("lock: {e}"))?;
    let card = client.discover_agent(url).map_err(|e| e.to_string())?;
    serde_json::to_value(&card).map_err(|e| e.to_string())
}

/// Get the A2A server/bridge status summary.
pub fn a2a_crate_get_status(state: &A2aState) -> Result<A2aServerStatus, String> {
    let registry = state.registry.lock().map_err(|e| format!("lock: {e}"))?;
    let bridge = state.bridge.lock().map_err(|e| format!("lock: {e}"))?;
    let client = state.client.lock().map_err(|e| format!("lock: {e}"))?;

    Ok(A2aServerStatus {
        running: true,
        version: A2A_PROTOCOL_VERSION.to_string(),
        known_peers: client.known_agents().len(),
        tasks_processed: bridge.tasks_processed(),
        skills_count: registry.total_skills(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentSkill;

    fn make_state() -> A2aState {
        let state = A2aState::default();
        {
            let mut reg = state.registry.lock().unwrap();
            reg.register_skills(
                "test-agent",
                vec![AgentSkill {
                    id: "test-skill".to_string(),
                    name: "Test Skill".to_string(),
                    description: Some("A test skill".to_string()),
                    tags: vec!["test".to_string()],
                    input_modes: vec!["text/plain".to_string()],
                    output_modes: vec!["text/plain".to_string()],
                }],
            );
        }
        state
    }

    #[test]
    fn get_agent_card_returns_card() {
        let state = make_state();
        let card = a2a_crate_get_agent_card(&state).unwrap();
        assert_eq!(card.name, "nexus-os");
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.version, A2A_PROTOCOL_VERSION);
    }

    #[test]
    fn list_skills_returns_summaries() {
        let state = make_state();
        let skills = a2a_crate_list_skills(&state).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "test-skill");
    }

    #[test]
    fn get_status_returns_summary() {
        let state = make_state();
        let status = a2a_crate_get_status(&state).unwrap();
        assert!(status.running);
        assert_eq!(status.skills_count, 1);
        assert_eq!(status.known_peers, 0);
        assert_eq!(status.tasks_processed, 0);
    }

    #[test]
    fn get_task_not_found() {
        let state = make_state();
        let result = a2a_crate_get_task(&state, "nonexistent", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn discover_agent_fails_on_bad_url() {
        let state = make_state();
        let result = a2a_crate_discover_agent(&state, "http://127.0.0.1:1");
        assert!(result.is_err());
    }
}
