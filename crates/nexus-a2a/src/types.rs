//! A2A protocol types — re-exports from kernel plus crate-local extensions.
//!
//! The canonical A2A types live in `nexus_kernel::protocols::a2a`.  This module
//! re-exports them so downstream consumers (Tauri commands, bridge, server) can
//! import everything from one place.

// ── Re-exports from kernel ──────────────────────────────────────────────────
pub use nexus_kernel::protocols::a2a::{
    A2ATask, AgentCapabilities, AgentCard, AgentSkill, Artifact, AuthScheme, FileContent,
    GovernanceContext, JsonRpcError, JsonRpcRequest, JsonRpcResponse, MessagePart, MessageRole,
    TaskCancelParams, TaskGetParams, TaskMessage, TaskPayload, TaskSendParams, TaskStatus,
    A2A_PROTOCOL_VERSION,
};

pub use nexus_kernel::protocols::a2a_client::{A2aClient, A2aClientError, A2aTaskResult};

use serde::{Deserialize, Serialize};

// ── Crate-local extensions ──────────────────────────────────────────────────

/// Summary of the A2A server's runtime status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aServerStatus {
    /// Whether the A2A endpoint is active.
    pub running: bool,
    /// Protocol version advertised.
    pub version: String,
    /// Number of discovered/known peer agents.
    pub known_peers: usize,
    /// Total tasks processed since start.
    pub tasks_processed: u64,
    /// Number of skills advertised in the local AgentCard.
    pub skills_count: usize,
}

/// A lightweight skill descriptor returned by `a2a_list_skills`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub input_modes: Vec<String>,
    pub output_modes: Vec<String>,
}

impl From<&AgentSkill> for SkillSummary {
    fn from(s: &AgentSkill) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            description: s.description.clone(),
            tags: s.tags.clone(),
            input_modes: s.input_modes.clone(),
            output_modes: s.output_modes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_summary_from_agent_skill() {
        let skill = AgentSkill {
            id: "web-search".to_string(),
            name: "Web Search".to_string(),
            description: Some("Search the web".to_string()),
            tags: vec!["web".to_string()],
            input_modes: vec!["text/plain".to_string()],
            output_modes: vec!["application/json".to_string()],
        };
        let summary = SkillSummary::from(&skill);
        assert_eq!(summary.id, "web-search");
        assert_eq!(summary.name, "Web Search");
        assert_eq!(summary.tags, vec!["web"]);
    }

    #[test]
    fn server_status_serialization() {
        let status = A2aServerStatus {
            running: true,
            version: A2A_PROTOCOL_VERSION.to_string(),
            known_peers: 3,
            tasks_processed: 42,
            skills_count: 11,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: A2aServerStatus = serde_json::from_str(&json).unwrap();
        assert!(parsed.running);
        assert_eq!(parsed.known_peers, 3);
        assert_eq!(parsed.tasks_processed, 42);
    }

    #[test]
    fn protocol_version_constant() {
        assert!(!A2A_PROTOCOL_VERSION.is_empty());
        // Should be semver-ish
        assert!(A2A_PROTOCOL_VERSION.contains('.'));
    }
}
