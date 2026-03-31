use crate::messaging::{
    IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
    RichMessage,
};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const DISCORD_MAX_MESSAGE_LEN: usize = 2000;

pub struct DiscordAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    http_client: Client,
    bot_token: Option<String>,
    api_base: String,
}

impl DiscordAdapter {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("discord", 5, 1);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Optional: bot token may not be configured in environment
        let token = std::env::var("DISCORD_BOT_TOKEN").ok();

        Self {
            incoming: Vec::new(),
            limiter,
            http_client: client,
            bot_token: token,
            api_base: DISCORD_API_BASE.to_string(),
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("discord") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("discord rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }

    /// Build the request URL for a Discord API endpoint.
    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Build the sendMessage payload for Discord.
    pub fn build_send_message_payload(&self, text: &str) -> Value {
        json!({ "content": text })
    }

    /// Build the Discord Gateway Identify payload (for documentation / test purposes).
    pub fn build_identify_payload(&self) -> Value {
        let token = self.bot_token.as_deref().unwrap_or("");
        json!({
            "op": 2,
            "d": {
                "token": token,
                "intents": 33281,
                "properties": {
                    "os": "linux",
                    "browser": "nexus-os",
                    "device": "nexus-os"
                }
            }
        })
    }

    /// Send a message to a Discord channel via REST API.
    fn send_api_message(&self, channel_id: &str, text: &str) -> Result<MessageId, AgentError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("Discord bot token not configured".to_string())
        })?;

        let url = self.api_url(&format!("channels/{channel_id}/messages"));
        let payload = self.build_send_message_payload(text);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bot {token}"))
            .json(&payload)
            .send()
            .map_err(|e| {
                AgentError::SupervisorError(format!("discord send message failed: {e}"))
            })?;

        let body: Value = response.json().map_err(|e| {
            AgentError::SupervisorError(format!("discord response parse failed: {e}"))
        })?;

        let message_id = body
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("dc-{}", Uuid::new_v4()));
        Ok(message_id)
    }

    /// Send a typing indicator to a Discord channel.
    pub fn send_typing_indicator(&self, channel_id: &str) -> Result<(), AgentError> {
        let Some(token) = &self.bot_token else {
            return Ok(());
        };
        let url = self.api_url(&format!("channels/{channel_id}/typing"));
        // Best-effort: typing indicator is cosmetic, don't fail the operation
        let _ = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bot {token}"))
            .send()
            .map_err(|e| {
                AgentError::SupervisorError(format!("discord typing indicator failed: {e}"))
            })?;
        Ok(())
    }

    /// Send a long message, splitting at Discord's 2000 char limit.
    pub fn send_long_message(
        &mut self,
        channel_id: &str,
        text: &str,
    ) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if channel_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "discord message requires non-empty chat_id and text".to_string(),
            ));
        }

        if self.bot_token.is_none() {
            return Ok(format!("dc-{}", Uuid::new_v4()));
        }

        let chunks = crate::gateway::split_message(text, DISCORD_MAX_MESSAGE_LEN);
        let mut last_id = String::new();
        for chunk in chunks {
            last_id = self.send_api_message(channel_id, chunk)?;
        }
        Ok(last_id)
    }

    /// Check if a Discord message is from a bot (for filtering).
    pub fn is_bot_message(message_json: &Value) -> bool {
        message_json
            .get("author")
            .and_then(|a| a.get("bot"))
            .and_then(|b| b.as_bool())
            .unwrap_or(false)
    }

    /// Parse a Discord MESSAGE_CREATE event into an IncomingMessage.
    pub fn parse_message_create(event_data: &Value) -> Option<IncomingMessage> {
        // Skip bot messages
        if Self::is_bot_message(event_data) {
            return None;
        }

        let channel_id = event_data.get("channel_id")?.as_str()?;
        let content = event_data.get("content")?.as_str()?;
        let author_id = event_data
            .get("author")
            .and_then(|a| a.get("id"))
            .and_then(|id| id.as_str())
            .unwrap_or("unknown");

        Some(IncomingMessage {
            chat_id: channel_id.to_string(),
            sender_id: author_id.to_string(),
            text: content.to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: current_unix_timestamp(),
        })
    }
}

impl Default for DiscordAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for DiscordAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.send_long_message(chat_id, text)
    }

    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError> {
        self.send_message(chat_id, message.text.as_str())
    }

    fn receive_messages(&mut self) -> IncomingMessageStream {
        let mut drained = self.incoming.drain(..).collect::<Vec<_>>();

        let boundary = SemanticBoundary::new();
        for msg in &mut drained {
            msg.sanitized_text =
                Some(boundary.sanitize_data(msg.text.as_str(), ContentOrigin::MessageContent));
        }

        IncomingMessageStream::new(drained)
    }

    fn platform_name(&self) -> &str {
        "discord"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 5,
            window_seconds: 1,
            quality_tier: Some("bot-standard".to_string()),
        }
    }
}

fn current_unix_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    }
}

/// Discord Gateway opcodes for documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayOpcode {
    Dispatch = 0,
    Heartbeat = 1,
    Identify = 2,
    Hello = 10,
    HeartbeatAck = 11,
}

/// Parsed Discord gateway message.
#[derive(Debug, Deserialize)]
pub struct GatewayMessage {
    pub op: u8,
    pub d: Option<Value>,
    pub s: Option<u64>,
    pub t: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_send_message_payload() {
        let adapter = DiscordAdapter::new();
        let payload = adapter.build_send_message_payload("hello world");
        assert_eq!(
            payload.get("content").and_then(|v| v.as_str()),
            Some("hello world")
        );
    }

    #[test]
    fn test_discord_identify_payload_format() {
        let mut adapter = DiscordAdapter::new();
        adapter.bot_token = Some("test-token".to_string());
        let payload = adapter.build_identify_payload();
        assert_eq!(payload.get("op").and_then(|v| v.as_u64()), Some(2));
        let d = payload.get("d").unwrap();
        assert_eq!(d.get("token").and_then(|v| v.as_str()), Some("test-token"));
        // intents: GUILDS (1) + GUILD_MESSAGES (512) + MESSAGE_CONTENT (32768) = 33281
        assert_eq!(d.get("intents").and_then(|v| v.as_u64()), Some(33281));
    }

    #[test]
    fn test_discord_bot_message_filtering() {
        let bot_msg = json!({
            "author": { "id": "123", "bot": true },
            "channel_id": "ch-1",
            "content": "bot says hello"
        });
        assert!(DiscordAdapter::is_bot_message(&bot_msg));
        assert!(DiscordAdapter::parse_message_create(&bot_msg).is_none());

        let user_msg = json!({
            "author": { "id": "456", "bot": false },
            "channel_id": "ch-2",
            "content": "user says hello"
        });
        assert!(!DiscordAdapter::is_bot_message(&user_msg));
        let parsed = DiscordAdapter::parse_message_create(&user_msg).unwrap();
        assert_eq!(parsed.chat_id, "ch-2");
        assert_eq!(parsed.sender_id, "456");
        assert_eq!(parsed.text, "user says hello");
    }

    #[test]
    fn test_discord_message_splitting_at_2000() {
        let long_msg = "x".repeat(4500);
        let chunks = crate::gateway::split_message(&long_msg, DISCORD_MAX_MESSAGE_LEN);
        assert!(chunks.len() >= 3);
        for chunk in &chunks {
            assert!(chunk.len() <= DISCORD_MAX_MESSAGE_LEN);
        }
    }

    #[test]
    fn test_discord_api_url() {
        let adapter = DiscordAdapter::new();
        let url = adapter.api_url("channels/123/messages");
        assert_eq!(url, "https://discord.com/api/v10/channels/123/messages");
    }
}
