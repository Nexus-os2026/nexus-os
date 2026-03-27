//! Frontend integration types.

use std::sync::RwLock;

use crate::actions::{BrowserAction, BrowserActionResult};
use crate::governance::BrowserPolicy;
use crate::session::BrowserSessionManager;

/// In-memory browser state held by the Tauri app.
pub struct BrowserState {
    pub manager: RwLock<BrowserSessionManager>,
}

impl BrowserState {
    pub fn new(python_path: String, script_path: String) -> Self {
        Self {
            manager: RwLock::new(BrowserSessionManager::new(
                python_path,
                script_path,
                BrowserPolicy::default(),
            )),
        }
    }
}

impl Default for BrowserState {
    fn default() -> Self {
        Self::new(
            "python3".into(),
            "crates/nexus-browser-agent/python/browser_bridge.py".into(),
        )
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

pub fn create_session(
    state: &BrowserState,
    agent_id: &str,
    autonomy_level: u8,
) -> Result<String, String> {
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .create_session(agent_id, autonomy_level)
}

pub fn execute_task(
    state: &BrowserState,
    session_id: &str,
    task: &str,
    max_steps: Option<u32>,
    model_id: &str,
) -> Result<BrowserActionResult, String> {
    let action = BrowserAction::ExecuteTask {
        task: task.into(),
        max_steps,
    };
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .execute_action(session_id, action, model_id)
}

pub fn navigate(
    state: &BrowserState,
    session_id: &str,
    url: &str,
) -> Result<BrowserActionResult, String> {
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .execute_action(session_id, BrowserAction::Navigate { url: url.into() }, "")
}

pub fn screenshot(
    state: &BrowserState,
    session_id: &str,
    output_path: Option<String>,
) -> Result<BrowserActionResult, String> {
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .execute_action(session_id, BrowserAction::Screenshot { output_path }, "")
}

pub fn get_content(state: &BrowserState, session_id: &str) -> Result<BrowserActionResult, String> {
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .execute_action(session_id, BrowserAction::GetContent, "")
}

pub fn close_session(state: &BrowserState, session_id: &str) -> Result<(), String> {
    state
        .manager
        .write()
        .map_err(|e| format!("lock: {e}"))?
        .close_session(session_id)
}

pub fn get_policy(state: &BrowserState) -> Result<BrowserPolicy, String> {
    Ok(state
        .manager
        .read()
        .map_err(|e| format!("lock: {e}"))?
        .policy()
        .clone())
}

pub fn session_count(state: &BrowserState) -> Result<usize, String> {
    Ok(state
        .manager
        .read()
        .map_err(|e| format!("lock: {e}"))?
        .active_session_count())
}
