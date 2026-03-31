//! Universal messaging gateway — routes incoming messages from any platform to agents
//! and sends agent responses back through the originating platform.

use crate::messaging::{IncomingMessage, MessagingPlatform};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Status of a connected messaging platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformStatus {
    pub name: String,
    pub connected: bool,
    pub message_count: u64,
}

/// Tracks a routed conversation: which agent handles messages from a given chat.
#[derive(Debug, Clone)]
struct ConversationRoute {
    agent_id: String,
}

/// Central gateway that routes messages between messaging platforms and agents.
pub struct MessageGateway {
    platforms: HashMap<String, Box<dyn MessagingPlatform>>,
    /// Maps (platform, chat_id) -> agent_id for active conversations.
    conversation_routes: Mutex<HashMap<String, ConversationRoute>>,
    /// Maps user_id -> default agent_id.
    user_defaults: Mutex<HashMap<String, String>>,
    /// Per-platform message counters.
    message_counts: Mutex<HashMap<String, u64>>,
    audit_trail: AuditTrail,
}

impl MessageGateway {
    pub fn new() -> Self {
        Self {
            platforms: HashMap::new(),
            conversation_routes: Mutex::new(HashMap::new()),
            user_defaults: Mutex::new(HashMap::new()),
            message_counts: Mutex::new(HashMap::new()),
            audit_trail: AuditTrail::new(),
        }
    }

    /// Register a messaging platform with the gateway.
    pub fn register_platform(&mut self, platform: Box<dyn MessagingPlatform>) {
        let name = platform.platform_name().to_string();
        self.message_counts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(name.clone(), 0);
        self.platforms.insert(name, platform);
    }

    /// Set a user's default agent for new conversations.
    pub fn set_default_agent(&self, user_id: &str, agent_id: &str) {
        self.user_defaults
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(user_id.to_string(), agent_id.to_string());
    }

    /// Set a conversation route: messages in this chat go to this agent.
    pub fn set_conversation_route(&self, platform: &str, chat_id: &str, agent_id: &str) {
        let key = route_key(platform, chat_id);
        self.conversation_routes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                key,
                ConversationRoute {
                    agent_id: agent_id.to_string(),
                },
            );
    }

    /// Resolve which agent should handle a message from this chat.
    pub fn resolve_agent(&self, platform: &str, chat_id: &str, sender_id: &str) -> Option<String> {
        let key = route_key(platform, chat_id);
        // 1. Check conversation-level route
        if let Some(route) = self
            .conversation_routes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
        {
            return Some(route.agent_id.clone());
        }
        // 2. Check user default
        self.user_defaults
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(sender_id)
            .cloned()
    }

    /// Process all pending incoming messages from a specific platform.
    /// Returns a list of (chat_id, agent_id, message_text) tuples that were routed.
    pub fn poll_platform(&mut self, platform_name: &str) -> Result<Vec<RoutedMessage>, AgentError> {
        let platform = self.platforms.get_mut(platform_name).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown platform '{platform_name}'"))
        })?;

        let messages: Vec<IncomingMessage> = platform.receive_messages().collect();
        let mut routed = Vec::new();

        for msg in &messages {
            let agent_id = self
                .resolve_agent(platform_name, &msg.chat_id, &msg.sender_id)
                .unwrap_or_default();

            routed.push(RoutedMessage {
                platform: platform_name.to_string(),
                chat_id: msg.chat_id.clone(),
                sender_id: msg.sender_id.clone(),
                text: msg.text.clone(),
                agent_id: agent_id.clone(),
                message_id: Uuid::new_v4().to_string(),
            });

            // Best-effort: log routed message to audit trail for governance
            let _ = self.audit_trail.append_event(
                Uuid::nil(),
                EventType::UserAction,
                json!({
                    "event": "gateway_message_routed",
                    "platform": platform_name,
                    "chat_id": msg.chat_id,
                    "sender_id": msg.sender_id,
                    "agent_id": agent_id,
                }),
            );
        }

        // Update counter
        if !messages.is_empty() {
            let mut counts = self
                .message_counts
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let count = counts.entry(platform_name.to_string()).or_insert(0);
            *count += messages.len() as u64;
        }

        Ok(routed)
    }

    /// Send a response from an agent back to the originating platform/chat.
    pub fn send_response(
        &mut self,
        platform_name: &str,
        chat_id: &str,
        text: &str,
    ) -> Result<String, AgentError> {
        let platform = self.platforms.get_mut(platform_name).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown platform '{platform_name}'"))
        })?;

        // Split long messages per platform limits
        let max_len = match platform_name {
            "discord" => 2000,
            "telegram" => 4096,
            _ => 4096,
        };

        let mut last_id = String::new();
        for chunk in split_message(text, max_len) {
            last_id = platform.send_message(chat_id, chunk)?;
        }

        Ok(last_id)
    }

    /// Send a HITL consent prompt to the user on their platform.
    pub fn send_consent_prompt(
        &mut self,
        platform_name: &str,
        chat_id: &str,
        agent_name: &str,
        operation_summary: &str,
        consent_id: &str,
    ) -> Result<String, AgentError> {
        let text = format!(
            "{agent_name} wants to: {operation_summary}\n\
             Reply APPROVE {consent_id} or DENY {consent_id}"
        );
        self.send_response(platform_name, chat_id, &text)
    }

    /// Get status of all registered platforms.
    pub fn get_status(&self) -> Vec<PlatformStatus> {
        let counts = self
            .message_counts
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        self.platforms
            .keys()
            .map(|name| PlatformStatus {
                name: name.clone(),
                connected: true,
                message_count: counts.get(name).copied().unwrap_or(0),
            })
            .collect()
    }

    /// Check if an incoming message is an APPROVE/DENY consent reply.
    pub fn parse_consent_reply(text: &str) -> Option<ConsentReply> {
        let trimmed = text.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }
        match parts[0].to_uppercase().as_str() {
            "APPROVE" => Some(ConsentReply {
                approved: true,
                consent_id: parts[1].to_string(),
            }),
            "DENY" => Some(ConsentReply {
                approved: false,
                consent_id: parts[1].to_string(),
            }),
            _ => None,
        }
    }
}

impl Default for MessageGateway {
    fn default() -> Self {
        Self::new()
    }
}

/// A message that has been routed to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedMessage {
    pub platform: String,
    pub chat_id: String,
    pub sender_id: String,
    pub text: String,
    pub agent_id: String,
    pub message_id: String,
}

/// Parsed APPROVE/DENY reply from a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentReply {
    pub approved: bool,
    pub consent_id: String,
}

/// Build a composite key for conversation routing.
fn route_key(platform: &str, chat_id: &str) -> String {
    format!("{platform}:{chat_id}")
}

/// Split a message into chunks no longer than `max_len` characters.
pub fn split_message(text: &str, max_len: usize) -> Vec<&str> {
    if max_len == 0 {
        return vec![text];
    }
    if text.len() <= max_len {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + max_len).min(text.len());
        // Try to split at a newline or space boundary
        let actual_end = if end < text.len() {
            text[start..end]
                .rfind('\n')
                .or_else(|| text[start..end].rfind(' '))
                .map(|pos| start + pos + 1)
                .unwrap_or(end)
        } else {
            end
        };
        chunks.push(&text[start..actual_end]);
        start = actual_end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::{
        IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig, RichMessage,
    };

    struct MockPlatform {
        name: String,
        incoming: Vec<IncomingMessage>,
        sent: Vec<(String, String)>,
    }

    impl MockPlatform {
        fn new(name: &str, messages: Vec<IncomingMessage>) -> Self {
            Self {
                name: name.to_string(),
                incoming: messages,
                sent: Vec::new(),
            }
        }
    }

    impl MessagingPlatform for MockPlatform {
        fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
            self.sent.push((chat_id.to_string(), text.to_string()));
            Ok(format!("mock-{}", Uuid::new_v4()))
        }

        fn send_rich_message(
            &mut self,
            chat_id: &str,
            message: RichMessage,
        ) -> Result<MessageId, AgentError> {
            self.send_message(chat_id, &message.text)
        }

        fn receive_messages(&mut self) -> IncomingMessageStream {
            let drained = self.incoming.drain(..).collect();
            IncomingMessageStream::new(drained)
        }

        fn platform_name(&self) -> &str {
            &self.name
        }

        fn rate_limit(&self) -> RateLimitConfig {
            RateLimitConfig {
                max_messages: 10,
                window_seconds: 1,
                quality_tier: None,
            }
        }
    }

    fn make_incoming(chat_id: &str, sender_id: &str, text: &str) -> IncomingMessage {
        IncomingMessage {
            chat_id: chat_id.to_string(),
            sender_id: sender_id.to_string(),
            text: text.to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: 1000,
        }
    }

    #[test]
    fn test_gateway_route_to_correct_agent() {
        let mut gw = MessageGateway::new();
        let platform = MockPlatform::new(
            "test",
            vec![make_incoming("chat-1", "user-1", "hello agent")],
        );
        gw.register_platform(Box::new(platform));
        gw.set_conversation_route("test", "chat-1", "agent-alpha");

        let routed = gw.poll_platform("test").unwrap();
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].agent_id, "agent-alpha");
        assert_eq!(routed[0].text, "hello agent");
    }

    #[test]
    fn test_gateway_default_agent_when_no_mapping() {
        let mut gw = MessageGateway::new();
        let platform = MockPlatform::new("test", vec![make_incoming("chat-2", "user-2", "hello")]);
        gw.register_platform(Box::new(platform));

        let routed = gw.poll_platform("test").unwrap();
        assert_eq!(routed.len(), 1);
        // No route set, agent_id should be empty (caller creates default agent)
        assert_eq!(routed[0].agent_id, "");
    }

    #[test]
    fn test_gateway_user_default_agent() {
        let mut gw = MessageGateway::new();
        let platform = MockPlatform::new("test", vec![make_incoming("chat-3", "user-3", "hi")]);
        gw.register_platform(Box::new(platform));
        gw.set_default_agent("user-3", "agent-beta");

        let routed = gw.poll_platform("test").unwrap();
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].agent_id, "agent-beta");
    }

    #[test]
    fn test_gateway_consent_reply_approve() {
        let reply = MessageGateway::parse_consent_reply("APPROVE consent-123");
        assert_eq!(
            reply,
            Some(ConsentReply {
                approved: true,
                consent_id: "consent-123".to_string(),
            })
        );
    }

    #[test]
    fn test_gateway_consent_reply_deny() {
        let reply = MessageGateway::parse_consent_reply("deny xyz-789");
        assert_eq!(
            reply,
            Some(ConsentReply {
                approved: false,
                consent_id: "xyz-789".to_string(),
            })
        );
    }

    #[test]
    fn test_gateway_consent_reply_invalid() {
        assert!(MessageGateway::parse_consent_reply("hello world").is_none());
        assert!(MessageGateway::parse_consent_reply("APPROVE").is_none());
        assert!(MessageGateway::parse_consent_reply("").is_none());
    }

    #[test]
    fn test_split_message_short() {
        let chunks = split_message("hello", 4096);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_message_at_limit() {
        let msg = "a".repeat(5000);
        let chunks = split_message(&msg, 2000);
        assert!(chunks.len() >= 3);
        for chunk in &chunks {
            assert!(chunk.len() <= 2000);
        }
    }

    #[test]
    fn test_split_message_telegram_limit() {
        let msg = "word ".repeat(1000); // ~5000 chars
        let chunks = split_message(&msg, 4096);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 4096);
        }
    }

    #[test]
    fn test_gateway_platform_status() {
        let mut gw = MessageGateway::new();
        gw.register_platform(Box::new(MockPlatform::new("telegram", vec![])));
        gw.register_platform(Box::new(MockPlatform::new("discord", vec![])));

        let status = gw.get_status();
        assert_eq!(status.len(), 2);
        assert!(status.iter().all(|s| s.connected));
    }

    #[test]
    fn test_gateway_message_count_tracking() {
        let mut gw = MessageGateway::new();
        let platform = MockPlatform::new(
            "test",
            vec![
                make_incoming("c1", "u1", "msg1"),
                make_incoming("c1", "u1", "msg2"),
            ],
        );
        gw.register_platform(Box::new(platform));

        let _ = gw.poll_platform("test").unwrap();
        let status = gw.get_status();
        let test_status = status.iter().find(|s| s.name == "test").unwrap();
        assert_eq!(test_status.message_count, 2);
    }

    #[test]
    fn test_gateway_conversation_route_overrides_user_default() {
        let mut gw = MessageGateway::new();
        let platform = MockPlatform::new("test", vec![make_incoming("chat-5", "user-5", "hi")]);
        gw.register_platform(Box::new(platform));
        gw.set_default_agent("user-5", "agent-default");
        gw.set_conversation_route("test", "chat-5", "agent-specific");

        let routed = gw.poll_platform("test").unwrap();
        assert_eq!(routed[0].agent_id, "agent-specific");
    }

    #[test]
    fn test_gateway_unknown_platform_error() {
        let mut gw = MessageGateway::new();
        let result = gw.poll_platform("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_gateway_send_consent_prompt() {
        let mut gw = MessageGateway::new();
        gw.register_platform(Box::new(MockPlatform::new("test", vec![])));
        let result =
            gw.send_consent_prompt("test", "chat-1", "writer-agent", "delete file.txt", "c-001");
        assert!(result.is_ok());
    }
}
