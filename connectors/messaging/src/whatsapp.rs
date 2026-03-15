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
use std::sync::Arc;
use uuid::Uuid;

const WHATSAPP_API_BASE: &str = "https://graph.facebook.com/v18.0";

/// WhatsApp adapter for specialized workflow actions, per Jan 2026 policy framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsAppQualityTier {
    Low,
    Medium,
    High,
}

pub struct WhatsAppAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    quality_tier: WhatsAppQualityTier,
    http_client: Client,
    access_token: Option<String>,
    phone_number_id: Option<String>,
    verify_token: Option<String>,
    api_base: String,
}

impl WhatsAppAdapter {
    pub fn new(quality_tier: WhatsAppQualityTier) -> Self {
        Self::with_clock(quality_tier, None)
    }

    pub fn with_clock(
        quality_tier: WhatsAppQualityTier,
        clock: Option<Arc<dyn Fn() -> u64 + Send + Sync>>,
    ) -> Self {
        let limiter = match clock {
            Some(clock_fn) => RateLimiter::with_clock(clock_fn),
            None => RateLimiter::new(),
        };

        let (max_messages, window_seconds) = match quality_tier {
            WhatsAppQualityTier::Low => (1, 2),
            WhatsAppQualityTier::Medium => (3, 2),
            WhatsAppQualityTier::High => (10, 2),
        };
        limiter.configure("whatsapp", max_messages, window_seconds);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        let access_token = std::env::var("WHATSAPP_ACCESS_TOKEN").ok();
        let phone_number_id = std::env::var("WHATSAPP_PHONE_NUMBER_ID").ok();
        let verify_token = std::env::var("WHATSAPP_VERIFY_TOKEN").ok();

        Self {
            incoming: Vec::new(),
            limiter,
            quality_tier,
            http_client: client,
            access_token,
            phone_number_id,
            verify_token,
            api_base: WHATSAPP_API_BASE.to_string(),
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("whatsapp") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!(
                    "whatsapp quality tier {:?} rate limit exceeded; retry after {retry_after_ms} ms",
                    self.quality_tier
                ),
            )),
        }
    }

    /// Build the API URL for WhatsApp Cloud API.
    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Build the send message payload for WhatsApp Cloud API.
    pub fn build_send_message_payload(&self, recipient: &str, text: &str) -> Value {
        json!({
            "messaging_product": "whatsapp",
            "to": recipient,
            "type": "text",
            "text": { "body": text }
        })
    }

    /// Build webhook verification challenge response.
    pub fn verify_webhook_challenge(
        &self,
        mode: &str,
        token: &str,
        challenge: &str,
    ) -> Result<String, AgentError> {
        if mode != "subscribe" {
            return Err(AgentError::SupervisorError(
                "whatsapp webhook verification: invalid mode".to_string(),
            ));
        }
        let expected = self.verify_token.as_deref().unwrap_or("");
        if token != expected {
            return Err(AgentError::SupervisorError(
                "whatsapp webhook verification: token mismatch".to_string(),
            ));
        }
        Ok(challenge.to_string())
    }

    /// Send a message via WhatsApp Cloud API.
    fn send_api_message(&self, recipient: &str, text: &str) -> Result<MessageId, AgentError> {
        let token = self.access_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("WhatsApp access token not configured".to_string())
        })?;
        let phone_id = self.phone_number_id.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("WhatsApp phone number ID not configured".to_string())
        })?;

        let url = self.api_url(&format!("{phone_id}/messages"));
        let payload = self.build_send_message_payload(recipient, text);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| {
                AgentError::SupervisorError(format!("whatsapp send message failed: {e}"))
            })?;

        let body: Value = response.json().map_err(|e| {
            AgentError::SupervisorError(format!("whatsapp response parse failed: {e}"))
        })?;

        let message_id = body
            .get("messages")
            .and_then(|m| m.as_array())
            .and_then(|arr| arr.first())
            .and_then(|msg| msg.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("wa-{}", Uuid::new_v4()));
        Ok(message_id)
    }

    /// Parse a WhatsApp webhook payload into IncomingMessages.
    pub fn parse_webhook_payload(payload: &Value) -> Vec<IncomingMessage> {
        let mut messages = Vec::new();

        let entries = match payload.get("entry").and_then(|e| e.as_array()) {
            Some(arr) => arr,
            None => return messages,
        };

        for entry in entries {
            let changes = match entry.get("changes").and_then(|c| c.as_array()) {
                Some(arr) => arr,
                None => continue,
            };

            for change in changes {
                let value = match change.get("value") {
                    Some(v) => v,
                    None => continue,
                };

                let wa_messages = match value.get("messages").and_then(|m| m.as_array()) {
                    Some(arr) => arr,
                    None => continue,
                };

                for msg in wa_messages {
                    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if msg_type != "text" {
                        continue;
                    }
                    let from = msg
                        .get("from")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let text = msg
                        .get("text")
                        .and_then(|t| t.get("body"))
                        .and_then(|b| b.as_str())
                        .unwrap_or("");

                    if !text.is_empty() {
                        messages.push(IncomingMessage {
                            chat_id: from.to_string(),
                            sender_id: from.to_string(),
                            text: text.to_string(),
                            sanitized_text: None,
                            voice_note_url: None,
                            timestamp: current_unix_timestamp(),
                        });
                    }
                }
            }
        }

        messages
    }

    /// Typing indicator is not supported in WhatsApp Cloud API. No-op.
    pub fn send_typing_indicator(&self, _chat_id: &str) -> Result<(), AgentError> {
        Ok(())
    }
}

impl MessagingPlatform for WhatsAppAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "whatsapp message requires non-empty chat_id and text".to_string(),
            ));
        }

        if self.access_token.is_none() {
            return Ok(format!("wa-{}", Uuid::new_v4()));
        }

        self.send_api_message(chat_id, text)
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
        "whatsapp"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        let (max_messages, window_seconds, tier) = match self.quality_tier {
            WhatsAppQualityTier::Low => (1, 2, "low"),
            WhatsAppQualityTier::Medium => (3, 2, "medium"),
            WhatsAppQualityTier::High => (10, 2, "high"),
        };

        RateLimitConfig {
            max_messages,
            window_seconds,
            quality_tier: Some(tier.to_string()),
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

/// WhatsApp webhook verification query parameters.
#[derive(Debug, Deserialize)]
pub struct WebhookVerifyQuery {
    #[serde(rename = "hub.mode")]
    pub mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    pub verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    pub challenge: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatsapp_send_message_format() {
        let adapter = WhatsAppAdapter::new(WhatsAppQualityTier::Medium);
        let payload = adapter.build_send_message_payload("+1234567890", "hello");
        assert_eq!(
            payload.get("messaging_product").and_then(|v| v.as_str()),
            Some("whatsapp")
        );
        assert_eq!(
            payload.get("to").and_then(|v| v.as_str()),
            Some("+1234567890")
        );
        assert_eq!(payload.get("type").and_then(|v| v.as_str()), Some("text"));
        assert_eq!(
            payload
                .get("text")
                .and_then(|t| t.get("body"))
                .and_then(|b| b.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn test_whatsapp_webhook_verification() {
        let mut adapter = WhatsAppAdapter::new(WhatsAppQualityTier::High);
        adapter.verify_token = Some("my-secret".to_string());

        let result = adapter.verify_webhook_challenge("subscribe", "my-secret", "challenge-123");
        assert_eq!(result.unwrap(), "challenge-123");

        let result = adapter.verify_webhook_challenge("subscribe", "wrong-token", "challenge-123");
        assert!(result.is_err());

        let result = adapter.verify_webhook_challenge("bad-mode", "my-secret", "challenge-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_whatsapp_webhook_payload_parsing() {
        let payload = json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "from": "+15551234567",
                            "type": "text",
                            "text": { "body": "Hello from WhatsApp" }
                        }]
                    }
                }]
            }]
        });
        let messages = WhatsAppAdapter::parse_webhook_payload(&payload);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chat_id, "+15551234567");
        assert_eq!(messages[0].sender_id, "+15551234567");
        assert_eq!(messages[0].text, "Hello from WhatsApp");
    }

    #[test]
    fn test_whatsapp_webhook_non_text_skipped() {
        let payload = json!({
            "entry": [{
                "changes": [{
                    "value": {
                        "messages": [{
                            "from": "+15551234567",
                            "type": "image",
                            "image": { "id": "img-1" }
                        }]
                    }
                }]
            }]
        });
        let messages = WhatsAppAdapter::parse_webhook_payload(&payload);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_whatsapp_api_url() {
        let adapter = WhatsAppAdapter::new(WhatsAppQualityTier::Low);
        let url = adapter.api_url("12345/messages");
        assert_eq!(url, "https://graph.facebook.com/v18.0/12345/messages");
    }

    #[test]
    fn test_whatsapp_typing_indicator_noop() {
        let adapter = WhatsAppAdapter::new(WhatsAppQualityTier::Low);
        let result = adapter.send_typing_indicator("+1234");
        assert!(result.is_ok());
    }
}
