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

const SLACK_API_BASE: &str = "https://slack.com/api";

pub struct SlackAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    http_client: Client,
    /// Bot token (xoxb-...) for sending messages.
    bot_token: Option<String>,
    /// App-level token (xapp-...) for Socket Mode connections.
    app_token: Option<String>,
    api_base: String,
}

impl SlackAdapter {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("slack", 3, 1);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        let bot_token = std::env::var("SLACK_BOT_TOKEN").ok();
        let app_token = std::env::var("SLACK_APP_TOKEN").ok();

        Self {
            incoming: Vec::new(),
            limiter,
            http_client: client,
            bot_token,
            app_token,
            api_base: SLACK_API_BASE.to_string(),
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("slack") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("slack rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }

    /// Build the API URL for a Slack method.
    pub fn api_url(&self, method: &str) -> String {
        format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            method.trim_start_matches('/')
        )
    }

    /// Build the chat.postMessage payload.
    pub fn build_post_message_payload(&self, channel: &str, text: &str) -> Value {
        json!({
            "channel": channel,
            "text": text
        })
    }

    /// Build the Socket Mode envelope acknowledgment.
    pub fn build_envelope_ack(envelope_id: &str) -> Value {
        json!({ "envelope_id": envelope_id })
    }

    /// Send a message via chat.postMessage.
    fn post_message(&self, channel: &str, text: &str) -> Result<MessageId, AgentError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("Slack bot token not configured".to_string())
        })?;

        let url = self.api_url("chat.postMessage");
        let payload = self.build_post_message_payload(channel, text);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&payload)
            .send()
            .map_err(|e| {
                AgentError::SupervisorError(format!("slack chat.postMessage failed: {e}"))
            })?;

        let body: Value = response.json().map_err(|e| {
            AgentError::SupervisorError(format!("slack response parse failed: {e}"))
        })?;

        let ok = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(AgentError::SupervisorError(format!(
                "slack chat.postMessage error: {error}"
            )));
        }

        let ts = body
            .get("ts")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("sl-{}", Uuid::new_v4()));
        Ok(ts)
    }

    /// Request a Socket Mode WebSocket URL via apps.connections.open.
    pub fn request_socket_url(&self) -> Result<String, AgentError> {
        let token = self.app_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("Slack app token not configured".to_string())
        })?;

        let url = self.api_url("apps.connections.open");
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send()
            .map_err(|e| {
                AgentError::SupervisorError(format!("slack apps.connections.open failed: {e}"))
            })?;

        let body: Value = response.json().map_err(|e| {
            AgentError::SupervisorError(format!("slack response parse failed: {e}"))
        })?;

        let ok = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        if !ok {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(AgentError::SupervisorError(format!(
                "slack apps.connections.open error: {error}"
            )));
        }

        body.get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                AgentError::SupervisorError("slack response missing url field".to_string())
            })
    }

    /// Parse a Socket Mode events_api message into an IncomingMessage.
    pub fn parse_socket_mode_event(envelope: &Value) -> Option<IncomingMessage> {
        let envelope_type = envelope.get("type")?.as_str()?;
        if envelope_type != "events_api" {
            return None;
        }

        let payload = envelope.get("payload")?;
        let event = payload.get("event")?;
        let event_type = event.get("type")?.as_str()?;
        if event_type != "message" {
            return None;
        }

        // Skip bot messages
        if event.get("bot_id").is_some() {
            return None;
        }

        let channel = event.get("channel")?.as_str()?;
        let text = event.get("text")?.as_str()?;
        let user = event
            .get("user")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Some(IncomingMessage {
            chat_id: channel.to_string(),
            sender_id: user.to_string(),
            text: text.to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: current_unix_timestamp(),
        })
    }

    /// Typing indicator is not natively supported in Slack. No-op.
    pub fn send_typing_indicator(&self, _channel: &str) -> Result<(), AgentError> {
        Ok(())
    }
}

impl Default for SlackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for SlackAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "slack message requires non-empty chat_id and text".to_string(),
            ));
        }

        if self.bot_token.is_none() {
            return Ok(format!("sl-{}", Uuid::new_v4()));
        }

        self.post_message(chat_id, text)
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
        "slack"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 3,
            window_seconds: 1,
            quality_tier: Some("workspace-standard".to_string()),
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

/// Slack Socket Mode envelope types.
#[derive(Debug, Deserialize)]
pub struct SocketModeEnvelope {
    pub envelope_id: Option<String>,
    #[serde(rename = "type")]
    pub envelope_type: Option<String>,
    pub payload: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_post_message_format() {
        let adapter = SlackAdapter::new();
        let payload = adapter.build_post_message_payload("C01234", "hello slack");
        assert_eq!(
            payload.get("channel").and_then(|v| v.as_str()),
            Some("C01234")
        );
        assert_eq!(
            payload.get("text").and_then(|v| v.as_str()),
            Some("hello slack")
        );
    }

    #[test]
    fn test_slack_envelope_ack() {
        let ack = SlackAdapter::build_envelope_ack("env-123");
        assert_eq!(
            ack.get("envelope_id").and_then(|v| v.as_str()),
            Some("env-123")
        );
    }

    #[test]
    fn test_slack_socket_mode_event_parsing() {
        let envelope = json!({
            "type": "events_api",
            "envelope_id": "e-1",
            "payload": {
                "event": {
                    "type": "message",
                    "channel": "C99",
                    "user": "U42",
                    "text": "hello from slack"
                }
            }
        });
        let msg = SlackAdapter::parse_socket_mode_event(&envelope).unwrap();
        assert_eq!(msg.chat_id, "C99");
        assert_eq!(msg.sender_id, "U42");
        assert_eq!(msg.text, "hello from slack");
    }

    #[test]
    fn test_slack_socket_mode_bot_filtering() {
        let envelope = json!({
            "type": "events_api",
            "envelope_id": "e-2",
            "payload": {
                "event": {
                    "type": "message",
                    "channel": "C99",
                    "bot_id": "B123",
                    "text": "bot message"
                }
            }
        });
        assert!(SlackAdapter::parse_socket_mode_event(&envelope).is_none());
    }

    #[test]
    fn test_slack_socket_mode_non_message_event() {
        let envelope = json!({
            "type": "events_api",
            "envelope_id": "e-3",
            "payload": {
                "event": {
                    "type": "reaction_added",
                    "channel": "C99"
                }
            }
        });
        assert!(SlackAdapter::parse_socket_mode_event(&envelope).is_none());
    }

    #[test]
    fn test_slack_api_url() {
        let adapter = SlackAdapter::new();
        let url = adapter.api_url("chat.postMessage");
        assert_eq!(url, "https://slack.com/api/chat.postMessage");
    }

    #[test]
    fn test_slack_typing_indicator_noop() {
        let adapter = SlackAdapter::new();
        let result = adapter.send_typing_indicator("C01");
        assert!(result.is_ok());
    }
}
