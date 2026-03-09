//! Governed inter-agent messaging channels with rate limiting and audit logging.

use nexus_sdk::audit::{AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::VecDeque;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub message_type: String,
    pub payload: Value,
    pub timestamp: u64,
    pub requires_ack: bool,
}

impl AgentMessage {
    pub fn new(
        from: Uuid,
        to: Uuid,
        message_type: &str,
        payload: Value,
        requires_ack: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            message_type: message_type.to_string(),
            payload,
            timestamp: unix_now(),
            requires_ack,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelError {
    WrongSender,
    MessageTypeNotAllowed(String),
    RateLimitExceeded,
    InsufficientFuel { required: u64 },
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSender => write!(f, "message sender does not match channel sender"),
            Self::MessageTypeNotAllowed(t) => write!(f, "message type not allowed: {t}"),
            Self::RateLimitExceeded => write!(f, "rate limit exceeded"),
            Self::InsufficientFuel { required } => {
                write!(f, "insufficient fuel: {required} required")
            }
        }
    }
}

#[derive(Debug)]
pub struct GovernedChannel {
    pub id: Uuid,
    pub sender: Uuid,
    pub receiver: Uuid,
    pub allowed_message_types: Vec<String>,
    pub max_messages_per_minute: u32,
    pub fuel_cost_per_message: u64,
    message_count_this_minute: u32,
    minute_started_at: u64,
    fuel_remaining: u64,
    audit_trail: AuditTrail,
    inbox: VecDeque<AgentMessage>,
}

impl GovernedChannel {
    pub fn new(
        sender: Uuid,
        receiver: Uuid,
        allowed_message_types: Vec<String>,
        max_messages_per_minute: u32,
        fuel_cost_per_message: u64,
        initial_fuel: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender,
            receiver,
            allowed_message_types,
            max_messages_per_minute,
            fuel_cost_per_message,
            message_count_this_minute: 0,
            minute_started_at: unix_now(),
            fuel_remaining: initial_fuel,
            audit_trail: AuditTrail::new(),
            inbox: VecDeque::new(),
        }
    }

    pub fn send(&mut self, msg: AgentMessage) -> Result<(), ChannelError> {
        // Validate sender
        if msg.from != self.sender {
            return Err(ChannelError::WrongSender);
        }

        // Validate message type
        if !self.allowed_message_types.contains(&msg.message_type) {
            return Err(ChannelError::MessageTypeNotAllowed(
                msg.message_type.clone(),
            ));
        }

        // Check rate limit — reset window if minute has elapsed
        let now = unix_now();
        if now - self.minute_started_at >= 60 {
            self.message_count_this_minute = 0;
            self.minute_started_at = now;
        }
        if self.message_count_this_minute >= self.max_messages_per_minute {
            return Err(ChannelError::RateLimitExceeded);
        }

        // Check fuel
        if self.fuel_remaining < self.fuel_cost_per_message {
            return Err(ChannelError::InsufficientFuel {
                required: self.fuel_cost_per_message,
            });
        }

        // Deduct fuel
        self.fuel_remaining -= self.fuel_cost_per_message;
        self.message_count_this_minute += 1;

        // Audit log
        self.audit_trail
            .append_event(
                self.sender,
                EventType::ToolCall,
                json!({
                    "action": "channel_send",
                    "channel_id": self.id.to_string(),
                    "message_id": msg.id.to_string(),
                    "message_type": msg.message_type,
                    "to": msg.to.to_string(),
                    "fuel_remaining": self.fuel_remaining,
                }),
            )
            .expect("audit: fail-closed");

        self.inbox.push_back(msg);
        Ok(())
    }

    pub fn recv(&mut self) -> Option<AgentMessage> {
        self.inbox.pop_front()
    }

    pub fn messages_sent(&self) -> usize {
        self.audit_trail.events().len()
    }

    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel(sender: Uuid, receiver: Uuid) -> GovernedChannel {
        GovernedChannel::new(
            sender,
            receiver,
            vec![
                "task_request".to_string(),
                "result".to_string(),
                "status".to_string(),
                "escalation".to_string(),
            ],
            10,  // max 10 per minute
            5,   // 5 fuel per message
            100, // 100 fuel budget
        )
    }

    #[test]
    fn send_and_recv_message() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver);

        let msg = AgentMessage::new(
            sender,
            receiver,
            "task_request",
            json!({"task": "build"}),
            false,
        );
        assert!(ch.send(msg).is_ok());

        let received = ch.recv().unwrap();
        assert_eq!(received.message_type, "task_request");
        assert_eq!(received.from, sender);
    }

    #[test]
    fn wrong_sender_rejected() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let imposter = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver);

        let msg = AgentMessage::new(imposter, receiver, "task_request", json!({}), false);
        assert_eq!(ch.send(msg), Err(ChannelError::WrongSender));
    }

    #[test]
    fn disallowed_message_type_rejected() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver);

        let msg = AgentMessage::new(sender, receiver, "forbidden_type", json!({}), false);
        assert_eq!(
            ch.send(msg),
            Err(ChannelError::MessageTypeNotAllowed(
                "forbidden_type".to_string()
            ))
        );
    }

    #[test]
    fn rate_limit_blocks_excess_messages() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        // max 3 per minute, plenty of fuel
        let mut ch = GovernedChannel::new(sender, receiver, vec!["status".to_string()], 3, 1, 1000);

        for _ in 0..3 {
            let msg = AgentMessage::new(sender, receiver, "status", json!({}), false);
            assert!(ch.send(msg).is_ok());
        }

        let msg = AgentMessage::new(sender, receiver, "status", json!({}), false);
        assert_eq!(ch.send(msg), Err(ChannelError::RateLimitExceeded));
    }

    #[test]
    fn fuel_deducted_per_message() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver); // 5 fuel per msg, 100 total

        assert_eq!(ch.fuel_remaining(), 100);

        let msg = AgentMessage::new(sender, receiver, "result", json!({}), false);
        ch.send(msg).unwrap();
        assert_eq!(ch.fuel_remaining(), 95);

        let msg = AgentMessage::new(sender, receiver, "result", json!({}), false);
        ch.send(msg).unwrap();
        assert_eq!(ch.fuel_remaining(), 90);
    }

    #[test]
    fn insufficient_fuel_rejected() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        // Only 4 fuel, cost is 5
        let mut ch = GovernedChannel::new(sender, receiver, vec!["status".to_string()], 100, 5, 4);

        let msg = AgentMessage::new(sender, receiver, "status", json!({}), false);
        assert_eq!(
            ch.send(msg),
            Err(ChannelError::InsufficientFuel { required: 5 })
        );
    }

    #[test]
    fn audit_event_per_send() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver);

        assert_eq!(ch.messages_sent(), 0);

        for i in 0..3 {
            let msg = AgentMessage::new(sender, receiver, "status", json!({"i": i}), false);
            ch.send(msg).unwrap();
        }

        assert_eq!(ch.messages_sent(), 3);

        let events = ch.audit_trail().events();
        assert_eq!(events.len(), 3);
        for event in events {
            assert_eq!(event.event_type, EventType::ToolCall);
            let action = event.payload.get("action").unwrap().as_str().unwrap();
            assert_eq!(action, "channel_send");
        }
    }

    #[test]
    fn recv_returns_none_when_empty() {
        let sender = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        let mut ch = make_channel(sender, receiver);
        assert!(ch.recv().is_none());
    }
}
