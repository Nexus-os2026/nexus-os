//! Browser session management — one bridge per session, governance-controlled.

use std::collections::HashMap;

use crate::actions::{BrowserAction, BrowserActionResult};
use crate::bridge::BrowserBridge;
use crate::governance::{self, BrowserPolicy};

/// Manages browser sessions for all agents.
pub struct BrowserSessionManager {
    sessions: HashMap<String, BrowserSession>,
    policy: BrowserPolicy,
    python_path: String,
    script_path: String,
}

/// A single browser session.
pub struct BrowserSession {
    pub session_id: String,
    pub agent_id: String,
    pub bridge: BrowserBridge,
    pub created_at: u64,
    pub actions_taken: u64,
    pub total_tokens_burned: u64,
}

impl BrowserSessionManager {
    pub fn new(python_path: String, script_path: String, policy: BrowserPolicy) -> Self {
        Self {
            sessions: HashMap::new(),
            policy,
            python_path,
            script_path,
        }
    }

    /// Create a new browser session for an agent.
    pub fn create_session(&mut self, agent_id: &str, autonomy_level: u8) -> Result<String, String> {
        governance::check_authorization(agent_id, autonomy_level, &self.policy)
            .map_err(|e| e.to_string())?;

        let agent_sessions = self
            .sessions
            .values()
            .filter(|s| s.agent_id == agent_id)
            .count();
        if agent_sessions as u32 >= self.policy.max_sessions_per_agent {
            return Err(format!(
                "Agent {agent_id} already has {agent_sessions} sessions (max {})",
                self.policy.max_sessions_per_agent
            ));
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let bridge = BrowserBridge::new(self.python_path.clone(), self.script_path.clone());

        self.sessions.insert(
            session_id.clone(),
            BrowserSession {
                session_id: session_id.clone(),
                agent_id: agent_id.to_string(),
                bridge,
                created_at: epoch_now(),
                actions_taken: 0,
                total_tokens_burned: 0,
            },
        );

        Ok(session_id)
    }

    /// Execute a browser action in a session.
    pub fn execute_action(
        &mut self,
        session_id: &str,
        action: BrowserAction,
        model_id: &str,
    ) -> Result<BrowserActionResult, String> {
        if let BrowserAction::Navigate { ref url } = action {
            governance::check_url(url, &self.policy).map_err(|e| e.to_string())?;
        }

        if let BrowserAction::ExecuteTask { max_steps, .. } = &action {
            governance::check_steps(max_steps.unwrap_or(20), &self.policy)
                .map_err(|e| e.to_string())?;
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session {session_id} not found"))?;

        let command = action.to_command(model_id);
        let response = session
            .bridge
            .send_command(command)
            .map_err(|e| e.to_string())?;

        session.actions_taken += 1;
        let tokens = action.estimated_tokens();
        session.total_tokens_burned += tokens;

        Ok(BrowserActionResult {
            success: response.status == "ok",
            action: format!("{action:?}"),
            result: response.result,
            url: response.url,
            title: response.title,
            steps_taken: response.steps_taken,
            error: if response.status != "ok" {
                response.message
            } else {
                None
            },
            estimated_tokens: tokens,
        })
    }

    /// Close a session.
    pub fn close_session(&mut self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.remove(session_id) {
            session.bridge.shutdown().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Close all sessions for an agent.
    pub fn close_agent_sessions(&mut self, agent_id: &str) {
        let ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.agent_id == agent_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let _ = self.close_session(&id);
        }
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn policy(&self) -> &BrowserPolicy {
        &self.policy
    }
}

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> BrowserSessionManager {
        BrowserSessionManager::new(
            "python3".into(),
            "crates/nexus-browser-agent/python/browser_bridge.py".into(),
            BrowserPolicy::default(),
        )
    }

    #[test]
    fn test_session_limit_per_agent() {
        let mut mgr = BrowserSessionManager::new(
            "python3".into(),
            "test.py".into(),
            BrowserPolicy {
                max_sessions_per_agent: 1,
                ..BrowserPolicy::default()
            },
        );
        // First session should succeed
        let _id1 = mgr.create_session("agent-l3", 3).unwrap();
        assert_eq!(mgr.active_session_count(), 1);

        // Second session should fail (limit = 1)
        let result = mgr.create_session("agent-l3", 3);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already has 1 sessions"));

        // Different agent should succeed
        let _id2 = mgr.create_session("agent-l4", 4).unwrap();
        assert_eq!(mgr.active_session_count(), 2);
    }

    #[test]
    fn test_session_governance_enforcement() {
        let mut mgr = test_manager();
        // L2 agent should be denied (min is L3)
        let result = mgr.create_session("agent-l2", 2);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("L3+"));
    }
}
