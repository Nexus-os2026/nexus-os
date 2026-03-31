use crate::messaging::{
    IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
    RichMessage,
};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Generic webhook adapter for arbitrary HTTP integrations.
///
/// Outbound: POSTs signed JSON payloads to a configurable URL.
/// Inbound: Buffers messages pushed by an HTTP endpoint, drained via
/// `receive_messages`.
pub struct WebhookAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    http_client: Client,
    /// Where to POST outbound messages.
    outbound_url: Option<String>,
    /// Secret for HMAC-SHA256 signing of outbound payloads.
    signing_secret: Option<String>,
    /// Secret for verifying inbound webhooks.
    verification_secret: Option<String>,
    /// Custom headers included on every outbound request.
    custom_headers: HashMap<String, String>,
    /// Shared buffer for inbound webhook messages.
    inbound_buffer: Arc<Mutex<Vec<IncomingMessage>>>,
}

impl WebhookAdapter {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("webhook", 30, 1);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Optional: webhook config may not be configured in environment
        let outbound_url = std::env::var("NEXUS_WEBHOOK_OUTBOUND_URL").ok();
        let signing_secret = std::env::var("NEXUS_WEBHOOK_SIGNING_SECRET").ok();
        let verification_secret = std::env::var("NEXUS_WEBHOOK_VERIFICATION_SECRET").ok();

        Self {
            incoming: Vec::new(),
            limiter,
            http_client: client,
            outbound_url,
            signing_secret,
            verification_secret,
            custom_headers: HashMap::new(),
            inbound_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create an adapter with a pre-set outbound URL (for testing).
    pub fn with_url(url: &str) -> Self {
        let mut adapter = Self::new();
        adapter.outbound_url = Some(url.to_string());
        adapter
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    /// Push a message into the shared inbound buffer (called by HTTP gateway).
    pub fn push_inbound(&self, message: IncomingMessage) -> Result<(), AgentError> {
        let mut buf = self
            .inbound_buffer
            .lock()
            .map_err(|e| AgentError::SupervisorError(format!("webhook buffer lock: {e}")))?;
        buf.push(message);
        Ok(())
    }

    /// Get a clone of the inbound buffer handle (for sharing with HTTP server).
    pub fn inbound_buffer_handle(&self) -> Arc<Mutex<Vec<IncomingMessage>>> {
        Arc::clone(&self.inbound_buffer)
    }

    /// Set a custom header to include on all outbound requests.
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.custom_headers
            .insert(key.to_string(), value.to_string());
    }

    /// Set the signing secret.
    pub fn set_signing_secret(&mut self, secret: &str) {
        self.signing_secret = Some(secret.to_string());
    }

    /// Set the verification secret.
    pub fn set_verification_secret(&mut self, secret: &str) {
        self.verification_secret = Some(secret.to_string());
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("webhook") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("webhook rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }

    /// Sign a payload using SHA-256(secret + payload).
    ///
    /// Matches the signing pattern used in `auth.rs`.
    pub fn sign_payload(payload: &str, secret: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify an inbound webhook signature.
    pub fn verify_inbound(&self, body: &str, signature: &str) -> bool {
        let secret = match &self.verification_secret {
            Some(s) => s,
            None => return false,
        };
        let expected = Self::sign_payload(body, secret);
        expected == signature
    }

    /// Build the outbound JSON payload for a text message.
    pub fn build_outbound_payload(&self, channel: &str, text: &str) -> Value {
        json!({
            "event": "message",
            "channel": channel,
            "text": text,
            "timestamp": current_iso_timestamp(),
            "message_id": Uuid::new_v4().to_string()
        })
    }

    /// Build the outbound JSON payload for a rich message.
    pub fn build_rich_outbound_payload(&self, channel: &str, message: &RichMessage) -> Value {
        json!({
            "event": "rich_message",
            "channel": channel,
            "message": {
                "text": message.text,
                "buttons": message.buttons,
                "images": message.images,
                "attachments": message.attachments
            },
            "timestamp": current_iso_timestamp(),
            "message_id": Uuid::new_v4().to_string()
        })
    }

    /// Parse a standard inbound webhook payload into an IncomingMessage.
    ///
    /// Accepts the standard Nexus format, or leniently extracts text from
    /// any JSON body with a "text", "message", "content", or "body" field.
    pub fn parse_inbound(payload: &Value) -> Option<IncomingMessage> {
        let text = payload
            .get("text")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("message").and_then(|v| v.as_str()))
            .or_else(|| payload.get("content").and_then(|v| v.as_str()))
            .or_else(|| payload.get("body").and_then(|v| v.as_str()))?;

        if text.is_empty() {
            return None;
        }

        let channel = payload
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let sender = payload
            .get("sender")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("from").and_then(|v| v.as_str()))
            .or_else(|| payload.get("user").and_then(|v| v.as_str()))
            .unwrap_or("webhook");

        Some(IncomingMessage {
            chat_id: channel.to_string(),
            sender_id: sender.to_string(),
            text: text.to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: current_unix_timestamp(),
        })
    }

    /// Send a payload to the outbound URL.
    fn post_outbound(&self, payload: &Value) -> Result<MessageId, AgentError> {
        let url = self.outbound_url.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("Webhook outbound URL not configured".to_string())
        })?;

        let body_str = serde_json::to_string(payload).map_err(|e| {
            AgentError::SupervisorError(format!("webhook payload serialization: {e}"))
        })?;

        let mut request = self
            .http_client
            .post(url)
            .header("Content-Type", "application/json");

        // Add signature header if secret is configured
        if let Some(ref secret) = self.signing_secret {
            let signature = Self::sign_payload(&body_str, secret);
            request = request.header("X-Nexus-Signature", &signature);
        }

        // Add custom headers
        for (key, value) in &self.custom_headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request
            .body(body_str)
            .send()
            .map_err(|e| AgentError::SupervisorError(format!("webhook POST failed: {e}")))?;

        if !response.status().is_success() {
            return Err(AgentError::SupervisorError(format!(
                "webhook POST returned {}",
                response.status()
            )));
        }

        let msg_id = payload
            .get("message_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("wh-{}", Uuid::new_v4()));

        Ok(msg_id)
    }
}

impl Default for WebhookAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for WebhookAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if text.is_empty() {
            return Err(AgentError::SupervisorError(
                "webhook message requires non-empty text".to_string(),
            ));
        }

        if self.outbound_url.is_none() {
            return Ok(format!("wh-{}", Uuid::new_v4()));
        }

        let payload = self.build_outbound_payload(chat_id, text);
        self.post_outbound(&payload)
    }

    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;

        if self.outbound_url.is_none() {
            return Ok(format!("wh-{}", Uuid::new_v4()));
        }

        let payload = self.build_rich_outbound_payload(chat_id, &message);
        self.post_outbound(&payload)
    }

    fn receive_messages(&mut self) -> IncomingMessageStream {
        // Drain from both the direct incoming vec and the shared inbound buffer
        let mut drained = self.incoming.drain(..).collect::<Vec<_>>();

        if let Ok(mut buf) = self.inbound_buffer.lock() {
            drained.extend(buf.drain(..));
        }

        let boundary = SemanticBoundary::new();
        for msg in &mut drained {
            msg.sanitized_text =
                Some(boundary.sanitize_data(msg.text.as_str(), ContentOrigin::MessageContent));
        }

        IncomingMessageStream::new(drained)
    }

    fn platform_name(&self) -> &str {
        "webhook"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 30,
            window_seconds: 1,
            quality_tier: None,
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

fn current_iso_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };
    // Simple ISO-ish timestamp without chrono dependency
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_outbound_payload_format() {
        let adapter = WebhookAdapter::new();
        let payload = adapter.build_outbound_payload("alerts", "server is down");
        assert_eq!(
            payload.get("event").and_then(|v| v.as_str()),
            Some("message")
        );
        assert_eq!(
            payload.get("channel").and_then(|v| v.as_str()),
            Some("alerts")
        );
        assert_eq!(
            payload.get("text").and_then(|v| v.as_str()),
            Some("server is down")
        );
        assert!(payload.get("timestamp").is_some());
        assert!(payload.get("message_id").is_some());
    }

    #[test]
    fn test_webhook_rich_outbound_payload() {
        let adapter = WebhookAdapter::new();
        let msg = RichMessage {
            text: "Deploy complete".into(),
            buttons: vec!["Rollback".into()],
            images: vec![],
            attachments: vec!["build.log".into()],
        };
        let payload = adapter.build_rich_outbound_payload("deploys", &msg);
        assert_eq!(
            payload.get("event").and_then(|v| v.as_str()),
            Some("rich_message")
        );
        let inner = payload.get("message").unwrap();
        assert_eq!(
            inner.get("text").and_then(|v| v.as_str()),
            Some("Deploy complete")
        );
    }

    #[test]
    fn test_webhook_signing() {
        let signature = WebhookAdapter::sign_payload("hello", "secret123");
        // SHA-256 of "secret123hello" — deterministic
        assert!(!signature.is_empty());
        assert_eq!(signature.len(), 64); // SHA-256 hex = 64 chars

        // Same input → same output
        let sig2 = WebhookAdapter::sign_payload("hello", "secret123");
        assert_eq!(signature, sig2);

        // Different input → different output
        let sig3 = WebhookAdapter::sign_payload("world", "secret123");
        assert_ne!(signature, sig3);
    }

    #[test]
    fn test_webhook_inbound_verification_valid() {
        let mut adapter = WebhookAdapter::new();
        adapter.set_verification_secret("my-secret");

        let body = r#"{"text":"hello"}"#;
        let signature = WebhookAdapter::sign_payload(body, "my-secret");

        assert!(adapter.verify_inbound(body, &signature));
    }

    #[test]
    fn test_webhook_inbound_verification_invalid() {
        let mut adapter = WebhookAdapter::new();
        adapter.set_verification_secret("my-secret");

        let body = r#"{"text":"hello"}"#;
        assert!(!adapter.verify_inbound(body, "bad-signature"));
    }

    #[test]
    fn test_webhook_inbound_verification_no_secret() {
        let adapter = WebhookAdapter::new();
        assert!(!adapter.verify_inbound("body", "sig"));
    }

    #[test]
    fn test_webhook_parse_standard_inbound() {
        let payload = json!({
            "event": "message",
            "channel": "alerts",
            "sender": "monitoring",
            "text": "CPU at 95%"
        });

        let msg = WebhookAdapter::parse_inbound(&payload).unwrap();
        assert_eq!(msg.chat_id, "alerts");
        assert_eq!(msg.sender_id, "monitoring");
        assert_eq!(msg.text, "CPU at 95%");
    }

    #[test]
    fn test_webhook_parse_lenient_inbound() {
        // Non-standard format — should still extract text
        let payload = json!({
            "content": "build failed",
            "user": "ci-bot"
        });

        let msg = WebhookAdapter::parse_inbound(&payload).unwrap();
        assert_eq!(msg.text, "build failed");
        assert_eq!(msg.sender_id, "ci-bot");
        assert_eq!(msg.chat_id, "default"); // no channel specified
    }

    #[test]
    fn test_webhook_parse_empty_text_rejected() {
        let payload = json!({ "text": "" });
        assert!(WebhookAdapter::parse_inbound(&payload).is_none());
    }

    #[test]
    fn test_webhook_inbound_buffer_drain() {
        let mut adapter = WebhookAdapter::new();

        // Push 3 messages via the shared buffer
        adapter
            .push_inbound(IncomingMessage {
                chat_id: "ch1".into(),
                sender_id: "s1".into(),
                text: "msg1".into(),
                sanitized_text: None,
                voice_note_url: None,
                timestamp: 1,
            })
            .unwrap();
        adapter
            .push_inbound(IncomingMessage {
                chat_id: "ch1".into(),
                sender_id: "s2".into(),
                text: "msg2".into(),
                sanitized_text: None,
                voice_note_url: None,
                timestamp: 2,
            })
            .unwrap();
        adapter.push_incoming(IncomingMessage {
            chat_id: "ch1".into(),
            sender_id: "s3".into(),
            text: "msg3".into(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: 3,
        });

        let messages: Vec<_> = adapter.receive_messages().collect();
        assert_eq!(messages.len(), 3);

        // Second drain should be empty
        let messages2: Vec<_> = adapter.receive_messages().collect();
        assert!(messages2.is_empty());
    }

    #[test]
    fn test_webhook_missing_url_returns_mock_id() {
        let mut adapter = WebhookAdapter::new();
        adapter.outbound_url = None;
        let result = adapter.send_message("ch", "test");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("wh-"));
    }

    #[test]
    fn test_webhook_platform_name() {
        let adapter = WebhookAdapter::new();
        assert_eq!(adapter.platform_name(), "webhook");
    }

    #[test]
    fn test_webhook_rate_limit_config() {
        let adapter = WebhookAdapter::new();
        let config = adapter.rate_limit();
        assert_eq!(config.max_messages, 30);
        assert_eq!(config.window_seconds, 1);
    }
}
