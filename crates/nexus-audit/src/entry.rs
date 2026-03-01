use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub kind: AuditEventKind,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventKind {
    AgentSpawned,
    AgentHalted,
    LlmRequest,
    LlmResponse,
    ToolCall,
    ToolResult,
    CapabilityDenied,
    FuelExhausted,
}

impl AuditEntry {
    pub fn new(agent_id: String, kind: AuditEventKind, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            timestamp: Utc::now(),
            kind,
            payload,
        }
    }
}
