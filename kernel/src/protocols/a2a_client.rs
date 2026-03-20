//! A2A Client — outbound agent-to-agent communication.
//!
//! Enables Nexus agents to discover, communicate with, and delegate tasks to
//! external A2A-compliant agents. Reuses the existing A2A types from the
//! `a2a` module for wire-level compatibility.

use super::a2a::{
    AgentCard, JsonRpcRequest, JsonRpcResponse, MessagePart, MessageRole, TaskMessage,
    TaskSendParams, TaskStatus, A2A_PROTOCOL_VERSION,
};
use crate::audit::{AuditTrail, EventType};
use crate::consent::{ConsentError, ConsentRuntime, GovernedOperation};
use crate::errors::AgentError;
use crate::fuel_hardening::FuelContext;
use crate::supervisor::max_fuel_cost;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Result of sending a task to a remote agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskResult {
    pub id: String,
    pub status: TaskStatus,
    pub result_text: Option<String>,
    pub artifacts: Vec<serde_json::Value>,
}

/// Error type for A2A client operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum A2aClientError {
    #[error("discovery failed for '{url}': {detail}")]
    DiscoveryFailed { url: String, detail: String },

    #[error("invalid agent card from '{url}': {detail}")]
    InvalidAgentCard { url: String, detail: String },

    #[error("send failed to '{url}': {detail}")]
    SendFailed { url: String, detail: String },

    #[error("remote agent error: {0}")]
    RemoteError(String),

    #[error("response parse error: {0}")]
    ResponseParse(String),

    #[error("agent '{name}' not found in known agents")]
    AgentNotFound { name: String },

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("HITL denied A2A delegation: {0}")]
    HitlDenied(String),

    #[error("fuel exhausted for A2A operation '{action}': {detail}")]
    FuelExhausted { action: String, detail: String },
}

impl From<A2aClientError> for AgentError {
    fn from(err: A2aClientError) -> Self {
        AgentError::SupervisorError(err.to_string())
    }
}

/// A2A Client for outbound agent-to-agent communication.
///
/// Discovers external agents via their Agent Card endpoints,
/// sends tasks via JSON-RPC 2.0, and tracks task status.
pub struct A2aClient {
    known_agents: HashMap<String, AgentCard>,
    request_counter: u64,
    audit: Mutex<AuditTrail>,
    consent: Option<Mutex<ConsentRuntime>>,
    agent_id: Uuid,
    fuel: Option<FuelContext>,
}

impl A2aClient {
    pub fn new() -> Self {
        Self {
            known_agents: HashMap::new(),
            request_counter: 0,
            audit: Mutex::new(AuditTrail::new()),
            consent: None,
            agent_id: Uuid::nil(),
            fuel: None,
        }
    }

    /// Create a client with HITL consent enforcement for A2A delegation.
    pub fn with_consent(consent_runtime: ConsentRuntime, agent_id: Uuid) -> Self {
        Self {
            known_agents: HashMap::new(),
            request_counter: 0,
            audit: Mutex::new(AuditTrail::new()),
            consent: Some(Mutex::new(consent_runtime)),
            agent_id,
            fuel: None,
        }
    }

    /// Attach a fuel context for metering A2A operations.
    pub fn set_fuel_context(&mut self, fuel: FuelContext) {
        self.fuel = Some(fuel);
    }

    /// Discover an agent by fetching its Agent Card from a URL.
    pub fn discover_agent(&mut self, base_url: &str) -> Result<AgentCard, A2aClientError> {
        // Fuel gate: reserve before network call
        let reservation = self.reserve_fuel("a2a_discover")?;

        let card_url = format!("{}/a2a/agent-card", base_url.trim_end_matches('/'));

        let output = std::process::Command::new("curl")
            .args(["-s", "-m", "10", &card_url])
            .output()
            .map_err(|e| A2aClientError::DiscoveryFailed {
                url: card_url.clone(),
                detail: e.to_string(),
            })?;

        if !output.status.success() {
            self.cancel_fuel(reservation);
            return Err(A2aClientError::DiscoveryFailed {
                url: card_url,
                detail: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let body = String::from_utf8_lossy(&output.stdout);
        let card: AgentCard = match serde_json::from_str(&body) {
            Ok(c) => c,
            Err(e) => {
                self.cancel_fuel(reservation);
                return Err(A2aClientError::InvalidAgentCard {
                    url: card_url,
                    detail: e.to_string(),
                });
            }
        };

        self.commit_fuel(reservation);
        self.audit_action("discover_agent", &card.name, base_url, true, "discovered");
        self.known_agents.insert(card.name.clone(), card.clone());
        Ok(card)
    }

    /// Register a known agent card directly (without discovery).
    pub fn register_agent(&mut self, card: AgentCard) {
        self.known_agents.insert(card.name.clone(), card);
    }

    /// Send a task to a remote agent via JSON-RPC 2.0.
    ///
    /// When a `ConsentRuntime` is configured (via `with_consent`), HITL approval
    /// is required before delegating work to an external agent. This prevents
    /// data from leaving the trust boundary without human authorization.
    pub fn send_task(
        &mut self,
        agent_url: &str,
        message: &str,
    ) -> Result<A2aTaskResult, A2aClientError> {
        // HITL gate: require human approval before sending data to external agent
        if let Some(consent_mutex) = &self.consent {
            let payload = format!(
                "a2a_delegate:{}:{}",
                agent_url,
                &message[..message.len().min(200)]
            );
            if let Ok(mut consent) = consent_mutex.lock() {
                if let Ok(mut audit) = self.audit.lock() {
                    let result = consent.enforce_operation(
                        GovernedOperation::A2aDelegation,
                        self.agent_id,
                        payload.as_bytes(),
                        &mut audit,
                    );
                    if let Err(e) = result {
                        let msg = match &e {
                            ConsentError::ApprovalRequired { request_id, .. } => {
                                format!(
                                    "HITL approval required for A2A delegation to {} (request: {})",
                                    agent_url, request_id
                                )
                            }
                            ConsentError::RequestDenied { request_id } => {
                                format!(
                                    "Human denied A2A delegation to {} (request: {})",
                                    agent_url, request_id
                                )
                            }
                            other => format!("A2A delegation blocked: {}", other),
                        };
                        return Err(A2aClientError::HitlDenied(msg));
                    }
                }
            }
        }

        // Fuel gate: reserve before delegation to external agent
        let reservation = self.reserve_fuel("a2a_delegate")?;

        let task_id = Uuid::new_v4().to_string();
        self.request_counter += 1;

        let params = TaskSendParams {
            id: Some(task_id.clone()),
            message: TaskMessage {
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    text: message.to_string(),
                }],
                metadata: None,
            },
            metadata: None,
        };

        let rpc_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::from(self.request_counter),
            method: "tasks/send".to_string(),
            params: Some(serde_json::to_value(&params).map_err(|e| {
                A2aClientError::SendFailed {
                    url: agent_url.to_string(),
                    detail: e.to_string(),
                }
            })?),
        };

        let response = match self.send_jsonrpc(agent_url, &rpc_request) {
            Ok(r) => r,
            Err(e) => {
                self.cancel_fuel(reservation);
                return Err(e);
            }
        };

        if let Some(error) = response.error {
            self.cancel_fuel(reservation);
            self.audit_action("send_task", "remote", agent_url, false, &error.message);
            return Err(A2aClientError::RemoteError(error.message));
        }

        let result = match response.result {
            Some(r) => r,
            None => {
                self.cancel_fuel(reservation);
                return Err(A2aClientError::ResponseParse("Missing result field".into()));
            }
        };

        let status_str = result
            .get("status")
            .and_then(|s| s.get("state"))
            .and_then(|s| s.as_str())
            .unwrap_or("submitted");

        let status = match status_str {
            "working" => TaskStatus::Working,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "canceled" => TaskStatus::Canceled,
            _ => TaskStatus::Submitted,
        };

        let result_text = result
            .get("artifacts")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|artifact| artifact.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .map(String::from);

        let artifacts = result
            .get("artifacts")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        self.commit_fuel(reservation);
        self.audit_action(
            "send_task",
            "remote",
            agent_url,
            true,
            &format!("task={task_id} status={status_str}"),
        );

        Ok(A2aTaskResult {
            id: task_id,
            status,
            result_text,
            artifacts,
        })
    }

    /// Get the status of a previously sent task.
    pub fn get_task_status(
        &mut self,
        agent_url: &str,
        task_id: &str,
    ) -> Result<A2aTaskResult, A2aClientError> {
        // Fuel gate: status checks are cheaper but still cost fuel
        let reservation = self.reserve_fuel("a2a_status_check")?;

        self.request_counter += 1;

        let rpc_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::from(self.request_counter),
            method: "tasks/get".to_string(),
            params: Some(json!({
                "id": task_id,
            })),
        };

        let response = match self.send_jsonrpc(agent_url, &rpc_request) {
            Ok(r) => r,
            Err(e) => {
                self.cancel_fuel(reservation);
                return Err(e);
            }
        };

        if let Some(error) = response.error {
            self.cancel_fuel(reservation);
            return Err(A2aClientError::RemoteError(error.message));
        }

        let result = match response.result {
            Some(r) => r,
            None => {
                self.cancel_fuel(reservation);
                return Err(A2aClientError::ResponseParse("Missing result field".into()));
            }
        };

        let status_str = result
            .get("status")
            .and_then(|s| s.get("state"))
            .and_then(|s| s.as_str())
            .unwrap_or("submitted");

        let status = match status_str {
            "working" => TaskStatus::Working,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "canceled" => TaskStatus::Canceled,
            _ => TaskStatus::Submitted,
        };

        let result_text = result
            .get("artifacts")
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|artifact| artifact.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .map(String::from);

        self.commit_fuel(reservation);
        Ok(A2aTaskResult {
            id: task_id.to_string(),
            status,
            result_text,
            artifacts: vec![],
        })
    }

    /// Cancel a task on a remote agent.
    pub fn cancel_task(&mut self, agent_url: &str, task_id: &str) -> Result<(), A2aClientError> {
        self.request_counter += 1;

        let rpc_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::from(self.request_counter),
            method: "tasks/cancel".to_string(),
            params: Some(json!({ "id": task_id })),
        };

        let response = self.send_jsonrpc(agent_url, &rpc_request)?;

        if let Some(error) = response.error {
            return Err(A2aClientError::RemoteError(error.message));
        }

        self.audit_action(
            "cancel_task",
            "remote",
            agent_url,
            true,
            &format!("task={task_id}"),
        );

        Ok(())
    }

    /// List all discovered agents.
    pub fn known_agents(&self) -> Vec<&AgentCard> {
        self.known_agents.values().collect()
    }

    /// Get a known agent by name.
    pub fn get_agent(&self, name: &str) -> Option<&AgentCard> {
        self.known_agents.get(name)
    }

    /// Find agents that have a specific skill tag.
    pub fn find_agents_by_skill(&self, tag: &str) -> Vec<&AgentCard> {
        self.known_agents
            .values()
            .filter(|card| {
                card.skills
                    .iter()
                    .any(|skill| skill.tags.iter().any(|t| t == tag))
            })
            .collect()
    }

    /// Access the audit trail.
    pub fn audit_trail(&self) -> &Mutex<AuditTrail> {
        &self.audit
    }

    /// Protocol version this client speaks.
    pub fn protocol_version(&self) -> &str {
        A2A_PROTOCOL_VERSION
    }

    // ── private ─────────────────────────────────────────────────────────

    /// Reserve fuel for an A2A action. Returns `None` if no fuel context is set
    /// (backward-compatible: unfuelled clients still work).
    fn reserve_fuel(
        &self,
        action: &str,
    ) -> Result<Option<crate::fuel_hardening::FuelReservation>, A2aClientError> {
        match &self.fuel {
            Some(ctx) => {
                let cost = max_fuel_cost(action);
                ctx.reserve_fuel(cost)
                    .map(Some)
                    .map_err(|_| A2aClientError::FuelExhausted {
                        action: action.to_string(),
                        detail: format!("need {} fuel, have {}", cost, ctx.fuel_remaining()),
                    })
            }
            None => Ok(None),
        }
    }

    fn commit_fuel(&self, reservation: Option<crate::fuel_hardening::FuelReservation>) {
        if let Some(r) = reservation {
            r.commit();
        }
    }

    fn cancel_fuel(&self, reservation: Option<crate::fuel_hardening::FuelReservation>) {
        if let Some(r) = reservation {
            r.cancel();
        }
    }

    fn send_jsonrpc(
        &self,
        base_url: &str,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, A2aClientError> {
        let url = format!("{}/a2a", base_url.trim_end_matches('/'));
        let body = serde_json::to_string(request).map_err(|e| A2aClientError::SendFailed {
            url: url.clone(),
            detail: e.to_string(),
        })?;

        let output = std::process::Command::new("curl")
            .args([
                "-s",
                "-m",
                "30",
                "-X",
                "POST",
                &url,
                "-H",
                "Content-Type: application/json",
                "-d",
                &body,
            ])
            .output()
            .map_err(|e| A2aClientError::HttpError(e.to_string()))?;

        if !output.status.success() {
            return Err(A2aClientError::SendFailed {
                url,
                detail: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let response_text = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&response_text)
            .map_err(|e| A2aClientError::ResponseParse(format!("Failed to parse response: {e}")))
    }

    fn audit_action(&self, action: &str, agent_name: &str, url: &str, success: bool, detail: &str) {
        if let Ok(mut audit) = self.audit.lock() {
            let _ = audit.append_event(
                Uuid::nil(),
                EventType::ToolCall,
                json!({
                    "action": format!("a2a_client_{action}"),
                    "agent": agent_name,
                    "url": url,
                    "success": success,
                    "detail": detail,
                }),
            );
        }
    }
}

impl Default for A2aClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::a2a::{AgentCapabilities, AgentSkill, AuthScheme};

    fn make_test_card(name: &str, url: &str) -> AgentCard {
        AgentCard {
            name: name.to_string(),
            description: Some(format!("Test agent {name}")),
            url: url.to_string(),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities::default(),
            skills: vec![AgentSkill {
                id: "test-skill".to_string(),
                name: "Test Skill".to_string(),
                description: Some("A test skill".to_string()),
                tags: vec!["test".to_string(), "web".to_string()],
                input_modes: vec!["text/plain".to_string()],
                output_modes: vec!["text/plain".to_string()],
            }],
            authentication: vec![],
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            rate_limit_rpm: Some(60),
        }
    }

    #[test]
    fn test_a2a_client_new() {
        let client = A2aClient::new();
        assert!(client.known_agents().is_empty());
        assert_eq!(client.protocol_version(), A2A_PROTOCOL_VERSION);
    }

    #[test]
    fn test_register_and_lookup_agent() {
        let mut client = A2aClient::new();
        let card = make_test_card("agent-alpha", "http://localhost:9000");
        client.register_agent(card.clone());

        assert_eq!(client.known_agents().len(), 1);
        let found = client.get_agent("agent-alpha").unwrap();
        assert_eq!(found.name, "agent-alpha");
        assert_eq!(found.url, "http://localhost:9000");
    }

    #[test]
    fn test_find_agents_by_skill() {
        let mut client = A2aClient::new();
        client.register_agent(make_test_card("agent-a", "http://a:9000"));
        client.register_agent(AgentCard {
            name: "agent-b".to_string(),
            description: None,
            url: "http://b:9000".to_string(),
            version: A2A_PROTOCOL_VERSION.to_string(),
            capabilities: AgentCapabilities::default(),
            skills: vec![AgentSkill {
                id: "coding".to_string(),
                name: "Coding".to_string(),
                description: None,
                tags: vec!["code".to_string()],
                input_modes: vec![],
                output_modes: vec![],
            }],
            authentication: vec![],
            default_input_modes: vec![],
            default_output_modes: vec![],
            rate_limit_rpm: None,
        });

        let web_agents = client.find_agents_by_skill("web");
        assert_eq!(web_agents.len(), 1);
        assert_eq!(web_agents[0].name, "agent-a");

        let code_agents = client.find_agents_by_skill("code");
        assert_eq!(code_agents.len(), 1);
        assert_eq!(code_agents[0].name, "agent-b");

        let all = client.find_agents_by_skill("test");
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_multiple_agent_registration() {
        let mut client = A2aClient::new();
        for i in 0..5 {
            client.register_agent(make_test_card(
                &format!("agent-{i}"),
                &format!("http://agent-{i}:9000"),
            ));
        }
        assert_eq!(client.known_agents().len(), 5);
    }

    #[test]
    fn test_agent_not_found() {
        let client = A2aClient::new();
        assert!(client.get_agent("nonexistent").is_none());
    }

    #[test]
    fn test_audit_trail_recording() {
        let client = A2aClient::new();
        client.audit_action("test", "agent-1", "http://test", true, "ok");
        client.audit_action("test", "agent-1", "http://test", false, "fail");
        let audit = client.audit_trail().lock().unwrap();
        assert_eq!(audit.events().len(), 2);
    }

    #[test]
    fn test_task_result_serialization() {
        let result = A2aTaskResult {
            id: "task-123".to_string(),
            status: TaskStatus::Completed,
            result_text: Some("Done!".to_string()),
            artifacts: vec![json!({"type": "text", "text": "output"})],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: A2aTaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "task-123");
        assert_eq!(parsed.status, TaskStatus::Completed);
        assert_eq!(parsed.result_text, Some("Done!".to_string()));
    }

    #[test]
    fn test_discover_nonexistent_agent() {
        let mut client = A2aClient::new();
        let result = client.discover_agent("http://127.0.0.1:1");
        // Should fail because no server is running there
        assert!(result.is_err());
    }

    #[test]
    fn test_a2a_client_error_conversion() {
        let err = A2aClientError::AgentNotFound {
            name: "test".to_string(),
        };
        let agent_err: AgentError = err.into();
        let msg = agent_err.to_string();
        assert!(msg.contains("test"));
    }

    #[test]
    fn test_hitl_denied_error() {
        let err = A2aClientError::HitlDenied("delegation refused".to_string());
        assert!(err.to_string().contains("HITL denied"));
        assert!(err.to_string().contains("delegation refused"));
    }

    #[test]
    fn test_with_consent_constructor() {
        use crate::consent::ConsentRuntime;
        let consent = ConsentRuntime::default();
        let agent_id = Uuid::new_v4();
        let client = A2aClient::with_consent(consent, agent_id);
        assert!(client.known_agents().is_empty());
        assert!(client.consent.is_some());
    }
}
