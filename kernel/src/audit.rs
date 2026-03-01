use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    StateChange,
    ToolCall,
    LlmCall,
    Error,
    UserAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: u64,
    pub agent_id: Uuid,
    pub event_type: EventType,
    pub payload: Value,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct AuditTrail {
    events: Vec<AuditEvent>,
}

impl AuditTrail {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn append_event(&mut self, agent_id: Uuid, event_type: EventType, payload: Value) -> Uuid {
        let event_id = Uuid::new_v4();
        let timestamp = current_unix_timestamp();
        let previous_hash = self
            .events
            .last()
            .map(|event| event.hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        let hash = compute_hash(
            event_id,
            timestamp,
            agent_id,
            &event_type,
            &payload,
            &previous_hash,
        );

        let event = AuditEvent {
            event_id,
            timestamp,
            agent_id,
            event_type,
            payload,
            previous_hash,
            hash,
        };
        self.events.push(event);
        event_id
    }

    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    pub fn events_mut(&mut self) -> &mut [AuditEvent] {
        &mut self.events
    }

    pub fn verify_integrity(&self) -> bool {
        let mut expected_previous = GENESIS_HASH.to_string();

        for event in &self.events {
            if event.previous_hash != expected_previous {
                return false;
            }

            let expected_hash = compute_hash(
                event.event_id,
                event.timestamp,
                event.agent_id,
                &event.event_type,
                &event.payload,
                &event.previous_hash,
            );
            if event.hash != expected_hash {
                return false;
            }

            expected_previous = event.hash.clone();
        }

        true
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

fn compute_hash(
    event_id: Uuid,
    timestamp: u64,
    agent_id: Uuid,
    event_type: &EventType,
    payload: &Value,
    previous_hash: &str,
) -> String {
    #[derive(Serialize)]
    struct CanonicalEventData<'a> {
        event_id: &'a str,
        timestamp: u64,
        agent_id: &'a str,
        event_type: &'a EventType,
        payload: &'a Value,
    }

    let event_id_string = event_id.to_string();
    let agent_id_string = agent_id.to_string();
    let canonical = CanonicalEventData {
        event_id: &event_id_string,
        timestamp,
        agent_id: &agent_id_string,
        event_type,
        payload,
    };

    let serialized = serde_json::to_vec(&canonical).unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(serialized);
    let digest = hasher.finalize();
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::{AuditTrail, EventType};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_audit_chain_integrity() {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        for idx in 0..5 {
            let payload = json!({ "seq": idx, "status": "ok" });
            let _ = trail.append_event(agent_id, EventType::StateChange, payload);
        }

        assert!(trail.verify_integrity());

        let events = trail.events_mut();
        events[2].payload = json!({ "seq": 999, "status": "tampered" });

        assert!(!trail.verify_integrity());
    }
}
