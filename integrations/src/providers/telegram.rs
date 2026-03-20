//! Telegram integration — Bot API notifications with Markdown formatting
//! and inline keyboard support.

use crate::error::IntegrationError;
use crate::events::{Notification, Severity};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";

pub struct TelegramIntegration {
    bot_token: String,
    default_chat_id: String,
    http: Client,
}

impl TelegramIntegration {
    pub fn new(bot_token: String, default_chat_id: String) -> Result<Self, IntegrationError> {
        if bot_token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_TELEGRAM_BOT_TOKEN".into(),
            });
        }
        if default_chat_id.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_TELEGRAM_CHAT_ID".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "telegram".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            bot_token,
            default_chat_id,
            http,
        })
    }

    /// Build from environment variables.
    pub fn from_env() -> Result<Self, IntegrationError> {
        let bot_token = std::env::var("NEXUS_TELEGRAM_BOT_TOKEN").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_TELEGRAM_BOT_TOKEN".into(),
            }
        })?;
        let chat_id = std::env::var("NEXUS_TELEGRAM_CHAT_ID").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_TELEGRAM_CHAT_ID".into(),
            }
        })?;
        Self::new(bot_token, chat_id)
    }

    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", TELEGRAM_API_BASE, self.bot_token, method)
    }

    fn severity_emoji(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => "🚨",
            Severity::Warning => "⚠️",
            Severity::Info => "ℹ️",
        }
    }

    fn build_message_text(&self, msg: &Notification) -> String {
        format!(
            "{emoji} *{title}*\n\n{body}\n\n_Source: {source} | Nexus OS_",
            emoji = Self::severity_emoji(&msg.severity),
            title = escape_markdown(&msg.title),
            body = escape_markdown(&msg.body),
            source = escape_markdown(&msg.source_event),
        )
    }

    /// Send a plain text message to a chat.
    fn send_message_to_chat(
        &self,
        chat_id: &str,
        text: &str,
        reply_markup: Option<serde_json::Value>,
    ) -> Result<(), IntegrationError> {
        let url = self.api_url("sendMessage");
        let mut body = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "MarkdownV2"
        });

        if let Some(markup) = reply_markup {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("reply_markup".to_string(), markup);
            }
        }

        let response = self.http.post(&url).json(&body).send().map_err(|e| {
            IntegrationError::ConnectionError {
                provider: "telegram".into(),
                message: e.to_string(),
            }
        })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "telegram".into(),
                status,
                body,
            });
        }

        // Telegram returns {"ok": true/false} in the body
        Ok(())
    }

    /// Send a notification with inline keyboard buttons for HITL actions.
    pub fn send_with_buttons(
        &self,
        chat_id: Option<&str>,
        text: &str,
        buttons: &[(&str, &str)],
    ) -> Result<(), IntegrationError> {
        let target = chat_id.unwrap_or(&self.default_chat_id);
        let keyboard: Vec<Vec<serde_json::Value>> = buttons
            .iter()
            .map(|(label, callback)| {
                vec![json!({
                    "text": label,
                    "callback_data": callback
                })]
            })
            .collect();

        let markup = json!({
            "inline_keyboard": keyboard
        });

        self.send_message_to_chat(target, text, Some(markup))
    }
}

impl Integration for TelegramIntegration {
    fn name(&self) -> &str {
        "Telegram"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Telegram
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let chat_id = message.channel.as_deref().unwrap_or(&self.default_chat_id);
        let text = self.build_message_text(message);
        self.send_message_to_chat(chat_id, &text, None)
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        let url = self.api_url("getMe");
        let response =
            self.http
                .get(&url)
                .send()
                .map_err(|e| IntegrationError::ConnectionError {
                    provider: "telegram".into(),
                    message: e.to_string(),
                })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(IntegrationError::AuthError {
                provider: "telegram".into(),
                message: "Invalid bot token".into(),
            })
        }
    }

    fn send_webhook(&self, payload: &serde_json::Value) -> Result<(), IntegrationError> {
        let chat_id = payload
            .get("chat_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_chat_id);
        let text = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("Nexus OS notification");
        self.send_message_to_chat(chat_id, text, None)
    }
}

/// Escape special characters for Telegram MarkdownV2 format.
fn escape_markdown(text: &str) -> String {
    let special = [
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];
    let mut escaped = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        if special.contains(&ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_telegram() -> TelegramIntegration {
        TelegramIntegration {
            bot_token: "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".into(),
            default_chat_id: "-1001234567890".into(),
            http: Client::new(),
        }
    }

    #[test]
    fn telegram_message_format() {
        let tg = make_telegram();
        let msg = Notification {
            title: "Agent Error".into(),
            body: "nexus-coder crashed".into(),
            severity: Severity::Critical,
            channel: None,
            source_event: "agent_error".into(),
        };
        let text = tg.build_message_text(&msg);
        assert!(text.contains("Agent Error"));
        assert!(text.contains("nexus\\-coder crashed"));
        assert!(text.contains("Nexus OS"));
    }

    #[test]
    fn telegram_api_url_construction() {
        let tg = make_telegram();
        let url = tg.api_url("sendMessage");
        assert!(url.starts_with("https://api.telegram.org/bot"));
        assert!(url.ends_with("/sendMessage"));
        assert!(url.contains(&tg.bot_token));
    }

    #[test]
    fn telegram_markdown_escaping() {
        let escaped = escape_markdown("Hello [world] (test)");
        assert_eq!(escaped, "Hello \\[world\\] \\(test\\)");
    }

    #[test]
    fn telegram_severity_emojis() {
        assert_eq!(
            TelegramIntegration::severity_emoji(&Severity::Critical),
            "🚨"
        );
        assert_eq!(
            TelegramIntegration::severity_emoji(&Severity::Warning),
            "⚠️"
        );
        assert_eq!(TelegramIntegration::severity_emoji(&Severity::Info), "ℹ️");
    }

    #[test]
    fn telegram_custom_chat_id() {
        let tg = make_telegram();
        let msg = Notification {
            title: "Test".into(),
            body: "Body".into(),
            severity: Severity::Info,
            channel: Some("-999".into()),
            source_event: "test".into(),
        };
        let chat_id = msg.channel.as_deref().unwrap_or(&tg.default_chat_id);
        assert_eq!(chat_id, "-999");
    }

    #[test]
    fn telegram_missing_token_error() {
        let result = TelegramIntegration::new(String::new(), "-1001234".into());
        assert!(result.is_err());
    }

    #[test]
    fn telegram_missing_chat_id_error() {
        let result = TelegramIntegration::new("token".into(), String::new());
        assert!(result.is_err());
    }
}
