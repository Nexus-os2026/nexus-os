use crate::messaging::{
    IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
    RichMessage,
};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use uuid::Uuid;

const MATRIX_MAX_MESSAGE_LEN: usize = 4000;

/// Matrix messaging adapter.
///
/// Connects to any Matrix homeserver using the Client-Server API v3.
/// Ideal for privacy-focused and air-gapped deployments.
pub struct MatrixAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    http_client: Client,
    /// Homeserver base URL (e.g. "https://matrix.org").
    homeserver_url: String,
    /// Full user ID (e.g. "@nexus-bot:matrix.org").
    user_id: Option<String>,
    /// Access token for authentication.
    access_token: Option<String>,
    /// Sync token for incremental sync.
    sync_token: Option<String>,
}

impl MatrixAdapter {
    pub fn new() -> Self {
        let limiter = RateLimiter::new();
        limiter.configure("matrix", 10, 1);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        let homeserver_url = std::env::var("NEXUS_MATRIX_HOMESERVER")
            .unwrap_or_else(|_| "https://matrix.org".to_string());
        // Optional: Matrix credentials may not be configured in environment
        let user_id = std::env::var("NEXUS_MATRIX_USER_ID").ok();
        let access_token = std::env::var("NEXUS_MATRIX_ACCESS_TOKEN").ok();

        Self {
            incoming: Vec::new(),
            limiter,
            http_client: client,
            homeserver_url: homeserver_url.trim_end_matches('/').to_string(),
            user_id,
            access_token,
            sync_token: None,
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("matrix") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("matrix rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }

    /// Build the API URL for a Matrix Client-Server endpoint.
    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}/_matrix/client/v3/{}",
            self.homeserver_url,
            path.trim_start_matches('/')
        )
    }

    /// Build the JSON payload for sending a text message.
    pub fn build_send_message_payload(&self, text: &str) -> Value {
        json!({
            "msgtype": "m.text",
            "body": text
        })
    }

    /// Build the JSON payload for sending a formatted HTML message.
    pub fn build_rich_message_payload(&self, message: &RichMessage) -> Value {
        let mut parts = vec![message.text.clone()];
        for btn in &message.buttons {
            parts.push(format!("[{btn}]"));
        }
        let body = parts.join("\n");

        // Build HTML version
        let mut html = format!("<p>{}</p>", html_escape(&message.text));
        for btn in &message.buttons {
            html.push_str(&format!("<p><strong>[{}]</strong></p>", html_escape(btn)));
        }

        json!({
            "msgtype": "m.text",
            "body": body,
            "format": "org.matrix.custom.html",
            "formatted_body": html
        })
    }

    /// URL-encode a Matrix room ID for use in URL paths.
    pub fn encode_room_id(room_id: &str) -> String {
        room_id
            .replace('!', "%21")
            .replace(':', "%3A")
            .replace('#', "%23")
    }

    /// Send a message to a Matrix room.
    fn put_message(&self, room_id: &str, payload: &Value) -> Result<MessageId, AgentError> {
        let token = self.access_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError("Matrix access token not configured".to_string())
        })?;

        let txn_id = Uuid::new_v4().to_string();
        let encoded_room = Self::encode_room_id(room_id);
        let url = self.api_url(&format!(
            "rooms/{encoded_room}/send/m.room.message/{txn_id}"
        ));

        let response = self
            .http_client
            .put(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .map_err(|e| AgentError::SupervisorError(format!("matrix send failed: {e}")))?;

        let body: Value = response.json().map_err(|e| {
            AgentError::SupervisorError(format!("matrix response parse failed: {e}"))
        })?;

        if let Some(err) = body.get("errcode").and_then(|v| v.as_str()) {
            let msg = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(AgentError::SupervisorError(format!(
                "matrix send error: {err}: {msg}"
            )));
        }

        let event_id = body
            .get("event_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("mx-{}", Uuid::new_v4()));
        Ok(event_id)
    }

    /// Parse a Matrix sync response into incoming messages.
    pub fn parse_sync_response(
        &self,
        response: &Value,
        room_id: Option<&str>,
    ) -> Vec<IncomingMessage> {
        let mut messages = Vec::new();

        let rooms = match response.get("rooms").and_then(|r| r.get("join")) {
            Some(joined) => joined,
            None => return messages,
        };

        let rooms_obj = match rooms.as_object() {
            Some(obj) => obj,
            None => return messages,
        };

        let self_user = self.user_id.as_deref().unwrap_or("");

        for (rid, room_data) in rooms_obj {
            // If room_id filter is set, skip non-matching rooms
            if let Some(filter) = room_id {
                if rid != filter {
                    continue;
                }
            }

            let events = match room_data
                .get("timeline")
                .and_then(|t| t.get("events"))
                .and_then(|e| e.as_array())
            {
                Some(evts) => evts,
                None => continue,
            };

            for event in events {
                let etype = match event.get("type").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => continue,
                };
                if etype != "m.room.message" {
                    continue;
                }

                let sender = event
                    .get("sender")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Filter out bot's own messages
                if sender == self_user {
                    continue;
                }

                let content = match event.get("content") {
                    Some(c) => c,
                    None => continue,
                };

                let body = content.get("body").and_then(|v| v.as_str()).unwrap_or("");

                if body.is_empty() {
                    continue;
                }

                let ts = event
                    .get("origin_server_ts")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    / 1000; // Matrix uses milliseconds

                messages.push(IncomingMessage {
                    chat_id: rid.clone(),
                    sender_id: sender.to_string(),
                    text: body.to_string(),
                    sanitized_text: None,
                    voice_note_url: None,
                    timestamp: ts,
                });
            }
        }

        messages
    }

    /// Extract the next_batch token from a sync response.
    pub fn extract_sync_token(response: &Value) -> Option<String> {
        response
            .get("next_batch")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Set the sync token (used after restore or manual sync).
    pub fn set_sync_token(&mut self, token: String) {
        self.sync_token = Some(token);
    }
}

impl Default for MatrixAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for MatrixAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "matrix message requires non-empty room_id and text".to_string(),
            ));
        }

        if self.access_token.is_none() {
            return Ok(format!("mx-{}", Uuid::new_v4()));
        }

        // Split long messages
        if text.len() > MATRIX_MAX_MESSAGE_LEN {
            let mut last_id = String::new();
            let mut pos = 0;
            while pos < text.len() {
                let end = (pos + MATRIX_MAX_MESSAGE_LEN).min(text.len());
                let chunk = &text[pos..end];
                let payload = self.build_send_message_payload(chunk);
                last_id = self.put_message(chat_id, &payload)?;
                pos = end;
            }
            return Ok(last_id);
        }

        let payload = self.build_send_message_payload(text);
        self.put_message(chat_id, &payload)
    }

    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() {
            return Err(AgentError::SupervisorError(
                "matrix rich message requires non-empty room_id".to_string(),
            ));
        }

        if self.access_token.is_none() {
            return Ok(format!("mx-{}", Uuid::new_v4()));
        }

        let payload = self.build_rich_message_payload(&message);
        self.put_message(chat_id, &payload)
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
        "matrix"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 10,
            window_seconds: 1,
            quality_tier: Some("homeserver-standard".to_string()),
        }
    }
}

/// Minimal HTML escaping for formatted Matrix messages.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_send_message_payload() {
        let adapter = MatrixAdapter::new();
        let payload = adapter.build_send_message_payload("hello matrix");
        assert_eq!(
            payload.get("msgtype").and_then(|v| v.as_str()),
            Some("m.text")
        );
        assert_eq!(
            payload.get("body").and_then(|v| v.as_str()),
            Some("hello matrix")
        );
    }

    #[test]
    fn test_matrix_api_url() {
        let adapter = MatrixAdapter::new();
        let url = adapter.api_url("rooms/%21abc/send/m.room.message/txn1");
        assert!(url.contains("/_matrix/client/v3/rooms/%21abc/send/m.room.message/txn1"));
    }

    #[test]
    fn test_matrix_room_id_encoding() {
        let encoded = MatrixAdapter::encode_room_id("!abc123:matrix.org");
        assert_eq!(encoded, "%21abc123%3Amatrix.org");
    }

    #[test]
    fn test_matrix_rich_message_payload() {
        let adapter = MatrixAdapter::new();
        let msg = RichMessage {
            text: "Choose an option".into(),
            buttons: vec!["Accept".into(), "Reject".into()],
            images: vec![],
            attachments: vec![],
        };
        let payload = adapter.build_rich_message_payload(&msg);
        assert_eq!(
            payload.get("format").and_then(|v| v.as_str()),
            Some("org.matrix.custom.html")
        );
        assert!(payload
            .get("formatted_body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("Accept"));
    }

    #[test]
    fn test_matrix_sync_response_parsing() {
        let adapter = MatrixAdapter::new();
        let sync_resp = json!({
            "next_batch": "s123_456",
            "rooms": {
                "join": {
                    "!room1:example.com": {
                        "timeline": {
                            "events": [
                                {
                                    "type": "m.room.message",
                                    "sender": "@alice:example.com",
                                    "content": {
                                        "msgtype": "m.text",
                                        "body": "hello from alice"
                                    },
                                    "origin_server_ts": 1711580400000u64
                                },
                                {
                                    "type": "m.room.member",
                                    "sender": "@bob:example.com",
                                    "content": { "membership": "join" }
                                }
                            ]
                        }
                    }
                }
            }
        });

        let messages = adapter.parse_sync_response(&sync_resp, None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chat_id, "!room1:example.com");
        assert_eq!(messages[0].sender_id, "@alice:example.com");
        assert_eq!(messages[0].text, "hello from alice");
    }

    #[test]
    fn test_matrix_bot_message_filtering() {
        let mut adapter = MatrixAdapter::new();
        // Set user_id via env — for test, manually set it
        adapter.user_id = Some("@nexus-bot:example.com".to_string());

        let sync_resp = json!({
            "next_batch": "s999",
            "rooms": {
                "join": {
                    "!room1:example.com": {
                        "timeline": {
                            "events": [
                                {
                                    "type": "m.room.message",
                                    "sender": "@nexus-bot:example.com",
                                    "content": { "msgtype": "m.text", "body": "bot echo" },
                                    "origin_server_ts": 1000000u64
                                },
                                {
                                    "type": "m.room.message",
                                    "sender": "@human:example.com",
                                    "content": { "msgtype": "m.text", "body": "human msg" },
                                    "origin_server_ts": 1000001u64
                                }
                            ]
                        }
                    }
                }
            }
        });

        let messages = adapter.parse_sync_response(&sync_resp, None);
        assert_eq!(messages.len(), 1, "bot message should be filtered");
        assert_eq!(messages[0].sender_id, "@human:example.com");
    }

    #[test]
    fn test_matrix_missing_token_returns_mock_id() {
        let mut adapter = MatrixAdapter::new();
        adapter.access_token = None;
        let result = adapter.send_message("!room:matrix.org", "test");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with("mx-"));
    }

    #[test]
    fn test_matrix_sync_token_extraction() {
        let resp = json!({ "next_batch": "s72595_4483_1934" });
        let token = MatrixAdapter::extract_sync_token(&resp);
        assert_eq!(token, Some("s72595_4483_1934".to_string()));
    }

    #[test]
    fn test_matrix_platform_name() {
        let adapter = MatrixAdapter::new();
        assert_eq!(adapter.platform_name(), "matrix");
    }

    #[test]
    fn test_matrix_rate_limit_config() {
        let adapter = MatrixAdapter::new();
        let config = adapter.rate_limit();
        assert_eq!(config.max_messages, 10);
        assert_eq!(config.window_seconds, 1);
    }
}
