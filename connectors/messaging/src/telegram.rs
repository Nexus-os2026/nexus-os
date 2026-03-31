use crate::messaging::{
    IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
    RichMessage,
};
use nexus_connectors_core::rate_limit::{RateLimitDecision, RateLimiter};
use nexus_kernel::config::load_config;
use nexus_kernel::errors::AgentError;
use nexus_kernel::firewall::{ContentOrigin, SemanticBoundary};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const TELEGRAM_LONG_POLL_TIMEOUT_SECS: u64 = 30;

pub struct TelegramAdapter {
    incoming: Vec<IncomingMessage>,
    limiter: RateLimiter,
    http_client: Client,
    bot_token: Option<String>,
    api_base: String,
    update_offset: i64,
    voice_download_dir: PathBuf,
}

impl TelegramAdapter {
    pub fn new() -> Self {
        Self::with_clock(None)
    }

    pub fn with_clock_and_no_token(clock: Option<Arc<dyn Fn() -> u64 + Send + Sync>>) -> Self {
        let mut adapter = Self::with_clock(clock);
        adapter.bot_token = None;
        adapter
    }

    pub fn with_clock(clock: Option<Arc<dyn Fn() -> u64 + Send + Sync>>) -> Self {
        let limiter = match clock {
            Some(clock_fn) => RateLimiter::with_clock(clock_fn),
            None => RateLimiter::new(),
        };
        limiter.configure("telegram", 1, 1);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(35))
            .build()
            .unwrap_or_else(|_| Client::new());

        // Optional: bot token may not be configured in environment
        let token_from_env = std::env::var("TELEGRAM_BOT_TOKEN").ok();
        let token_from_config = load_config()
            // Optional: config file may not exist or be malformed
            .ok()
            .map(|cfg| cfg.messaging.telegram_bot_token)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let token = token_from_env
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or(token_from_config);

        Self {
            incoming: Vec::new(),
            limiter,
            http_client: client,
            bot_token: token,
            api_base: TELEGRAM_API_BASE.to_string(),
            update_offset: 0,
            voice_download_dir: std::env::temp_dir().join("nexus-telegram-voice"),
        }
    }

    pub fn push_incoming(&mut self, message: IncomingMessage) {
        self.incoming.push(message);
    }

    pub fn build_send_message_payload(&self, chat_id: &str, text: &str) -> Value {
        json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        })
    }

    /// Send a typing indicator to the given chat.
    pub fn send_typing_indicator(&self, chat_id: &str) -> Result<(), AgentError> {
        if self.bot_token.is_none() {
            return Ok(());
        }
        let payload = json!({
            "chat_id": chat_id,
            "action": "typing"
        });
        // Best-effort: typing indicator is cosmetic, don't fail the operation
        let _ = self.send_payload("sendChatAction", &payload)?;
        Ok(())
    }

    /// Send a message, splitting into chunks if it exceeds 4096 characters.
    pub fn send_long_message(
        &mut self,
        chat_id: &str,
        text: &str,
    ) -> Result<MessageId, AgentError> {
        if text.len() <= 4096 {
            return self.send_message_impl(chat_id, text);
        }
        let chunks = crate::gateway::split_message(text, 4096);
        let mut last_id = String::new();
        for chunk in chunks {
            last_id = self.send_message_impl(chat_id, chunk)?;
        }
        Ok(last_id)
    }

    fn send_message_impl(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || text.is_empty() {
            return Err(AgentError::SupervisorError(
                "telegram message requires non-empty chat_id and text".to_string(),
            ));
        }
        if self.bot_token.is_none() {
            return Ok(format!("tg-mock-{}", Uuid::new_v4()));
        }
        let payload = self.build_send_message_payload(chat_id, text);
        let result = self.send_payload("sendMessage", &payload)?;
        if !result.ok {
            return Err(AgentError::SupervisorError(
                "telegram sendMessage returned ok=false".to_string(),
            ));
        }
        let message_id = result
            .result
            .and_then(|json| json.get("message_id").cloned())
            .and_then(|value| value.as_i64())
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("tg-{}", Uuid::new_v4()));
        Ok(message_id)
    }

    pub fn build_inline_keyboard_payload(
        &self,
        chat_id: &str,
        text: &str,
        buttons: &[String],
    ) -> Value {
        let keyboard = buttons
            .iter()
            .map(|button| {
                vec![json!({
                    "text": button,
                    "callback_data": button
                })]
            })
            .collect::<Vec<_>>();

        json!({
            "chat_id": chat_id,
            "text": text,
            "reply_markup": {
                "inline_keyboard": keyboard
            }
        })
    }

    fn check_rate_limit(&self) -> Result<(), AgentError> {
        match self.limiter.check("telegram") {
            RateLimitDecision::Allowed => Ok(()),
            RateLimitDecision::RateLimited { retry_after_ms } => Err(AgentError::SupervisorError(
                format!("telegram rate limit exceeded; retry after {retry_after_ms} ms"),
            )),
        }
    }

    fn api_url(&self, method: &str) -> Result<String, AgentError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError(
                "Telegram bot token missing. Configure messaging.telegram_bot_token".to_string(),
            )
        })?;
        Ok(format!(
            "{}/bot{}/{}",
            self.api_base.trim_end_matches('/'),
            token,
            method
        ))
    }

    fn file_url(&self, file_path: &str) -> Result<String, AgentError> {
        let token = self.bot_token.as_ref().ok_or_else(|| {
            AgentError::SupervisorError(
                "Telegram bot token missing. Configure messaging.telegram_bot_token".to_string(),
            )
        })?;
        Ok(format!(
            "{}/file/bot{}/{}",
            self.api_base.trim_end_matches('/'),
            token,
            file_path.trim_start_matches('/')
        ))
    }

    fn send_payload(
        &self,
        method: &str,
        payload: &Value,
    ) -> Result<TelegramApiResponse<Value>, AgentError> {
        let url = self.api_url(method)?;
        let response = self
            .http_client
            .post(url)
            .json(payload)
            .send()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram {method} failed: {error}"))
            })?;

        response
            .json::<TelegramApiResponse<Value>>()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram response parse failed: {error}"))
            })
    }

    fn poll_updates(&mut self) -> Result<Vec<TelegramUpdate>, AgentError> {
        let url = self.api_url("getUpdates")?;
        let payload = json!({
            "offset": self.update_offset,
            "timeout": TELEGRAM_LONG_POLL_TIMEOUT_SECS
        });
        let response = self
            .http_client
            .post(url)
            .json(&payload)
            .send()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram polling failed: {error}"))
            })?;
        let parsed = response
            .json::<TelegramApiResponse<Vec<TelegramUpdate>>>()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram polling parse failed: {error}"))
            })?;

        if !parsed.ok {
            return Err(AgentError::SupervisorError(
                "telegram polling returned ok=false".to_string(),
            ));
        }
        let updates = parsed.result.unwrap_or_default();
        if let Some(last) = updates.last() {
            self.update_offset = last.update_id.saturating_add(1);
        }
        Ok(updates)
    }

    fn download_voice_note(&self, file_id: &str) -> Result<Option<String>, AgentError> {
        let get_file = self.send_payload("getFile", &json!({ "file_id": file_id }))?;
        if !get_file.ok {
            return Ok(None);
        }
        let file_path = get_file
            .result
            .and_then(|result| result.get("file_path").cloned())
            .and_then(|value| value.as_str().map(ToString::to_string));

        let Some(file_path) = file_path else {
            return Ok(None);
        };

        let file_url = self.file_url(file_path.as_str())?;
        let bytes = self
            .http_client
            .get(file_url)
            .send()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram file download failed: {error}"))
            })?
            .bytes()
            .map_err(|error| {
                AgentError::SupervisorError(format!("telegram file read failed: {error}"))
            })?;

        fs::create_dir_all(&self.voice_download_dir).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to create voice download directory: {error}"
            ))
        })?;
        let output = self
            .voice_download_dir
            .join(format!("{}-{}.ogg", file_id, Uuid::new_v4()));
        fs::write(&output, bytes).map_err(|error| {
            AgentError::SupervisorError(format!("failed to store voice note: {error}"))
        })?;

        Ok(Some(output.to_string_lossy().to_string()))
    }

    fn parse_update(&self, update: TelegramUpdate) -> Result<Option<IncomingMessage>, AgentError> {
        let message = update.message.or_else(|| {
            update.callback_query.and_then(|callback| {
                callback.message.map(|msg| TelegramMessage {
                    text: callback.data,
                    ..msg
                })
            })
        });

        let Some(message) = message else {
            return Ok(None);
        };
        let chat_id = message.chat.id.to_string();
        let sender_id = message
            .from
            .map(|sender| sender.id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let text = message.text.unwrap_or_default();

        let voice_note_url = match message.voice {
            Some(voice) => self.download_voice_note(voice.file_id.as_str())?,
            None => None,
        };

        Ok(Some(IncomingMessage {
            chat_id,
            sender_id,
            text,
            sanitized_text: None,
            voice_note_url,
            timestamp: current_unix_timestamp(),
        }))
    }
}

impl Default for TelegramAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingPlatform for TelegramAdapter {
    fn send_message(&mut self, chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
        self.send_long_message(chat_id, text)
    }

    fn send_rich_message(
        &mut self,
        chat_id: &str,
        message: RichMessage,
    ) -> Result<MessageId, AgentError> {
        self.check_rate_limit()?;
        if chat_id.is_empty() || message.text.trim().is_empty() {
            return Err(AgentError::SupervisorError(
                "telegram rich message requires non-empty chat_id and text".to_string(),
            ));
        }

        if self.bot_token.is_none() {
            return Ok(format!("tg-rich-mock-{}", Uuid::new_v4()));
        }

        let payload =
            self.build_inline_keyboard_payload(chat_id, message.text.as_str(), &message.buttons);
        let result = self.send_payload("sendMessage", &payload)?;
        if !result.ok {
            return Err(AgentError::SupervisorError(
                "telegram sendMessage (rich) returned ok=false".to_string(),
            ));
        }
        let message_id = result
            .result
            .and_then(|json| json.get("message_id").cloned())
            .and_then(|value| value.as_i64())
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("tg-{}", Uuid::new_v4()));
        Ok(message_id)
    }

    fn receive_messages(&mut self) -> IncomingMessageStream {
        let mut drained = self.incoming.drain(..).collect::<Vec<_>>();

        if self.bot_token.is_some() {
            if let Ok(updates) = self.poll_updates() {
                for update in updates {
                    if let Ok(Some(incoming)) = self.parse_update(update) {
                        drained.push(incoming);
                    }
                }
            }
        }

        let boundary = SemanticBoundary::new();
        for msg in &mut drained {
            msg.sanitized_text =
                Some(boundary.sanitize_data(msg.text.as_str(), ContentOrigin::MessageContent));
        }

        IncomingMessageStream::new(drained)
    }

    fn platform_name(&self) -> &str {
        "telegram"
    }

    fn rate_limit(&self) -> RateLimitConfig {
        RateLimitConfig {
            max_messages: 1,
            window_seconds: 1,
            quality_tier: Some("bot-standard".to_string()),
        }
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[derive(Debug, Deserialize)]
struct TelegramApiResponse<T> {
    ok: bool,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
    callback_query: Option<TelegramCallbackQuery>,
}

#[derive(Debug, Deserialize)]
struct TelegramCallbackQuery {
    data: Option<String>,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    chat: TelegramChat,
    from: Option<TelegramUser>,
    text: Option<String>,
    voice: Option<TelegramVoice>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramVoice {
    file_id: String,
}

#[cfg(test)]
mod tests {
    use super::TelegramAdapter;

    #[test]
    fn test_telegram_message_format() {
        let adapter = TelegramAdapter::new();
        let payload = adapter.build_send_message_payload("12345", "status");

        assert_eq!(
            payload.get("chat_id").and_then(|v| v.as_str()),
            Some("12345")
        );
        assert_eq!(payload.get("text").and_then(|v| v.as_str()), Some("status"));
        assert_eq!(
            payload.get("parse_mode").and_then(|v| v.as_str()),
            Some("Markdown")
        );
    }

    #[test]
    fn test_telegram_api_url_construction() {
        let mut adapter = TelegramAdapter::new();
        adapter.bot_token = Some("test-token-123".to_string());
        let url = adapter.api_url("getUpdates").unwrap();
        assert_eq!(url, "https://api.telegram.org/bottest-token-123/getUpdates");
    }

    #[test]
    fn test_telegram_api_url_missing_token() {
        let adapter = TelegramAdapter::with_clock_and_no_token(None);
        let result = adapter.api_url("sendMessage");
        assert!(result.is_err());
    }

    #[test]
    fn test_telegram_long_polling_offset_tracking() {
        let adapter = TelegramAdapter::new();
        assert_eq!(adapter.update_offset, 0);
    }

    #[test]
    fn test_telegram_typing_indicator_no_token() {
        let adapter = TelegramAdapter::with_clock_and_no_token(None);
        let result = adapter.send_typing_indicator("chat-1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_telegram_inline_keyboard_payload() {
        let adapter = TelegramAdapter::new();
        let payload = adapter.build_inline_keyboard_payload(
            "chat-1",
            "Choose an option",
            &["Option A".to_string(), "Option B".to_string()],
        );
        assert!(payload.get("reply_markup").is_some());
        let keyboard = payload["reply_markup"]["inline_keyboard"]
            .as_array()
            .unwrap();
        assert_eq!(keyboard.len(), 2);
    }
}
