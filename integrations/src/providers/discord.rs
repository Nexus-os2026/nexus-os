//! Discord integration — Bot API notifications with rich embed formatting.

use crate::error::IntegrationError;
use crate::events::{Notification, Severity};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde::Serialize;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

pub struct DiscordIntegration {
    bot_token: String,
    default_channel_id: String,
    http: Client,
}

#[derive(Debug, Serialize)]
struct DiscordMessage {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeds: Option<Vec<DiscordEmbed>>,
}

#[derive(Debug, Clone, Serialize)]
struct DiscordEmbed {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fields: Vec<DiscordField>,
}

#[derive(Debug, Clone, Serialize)]
struct DiscordField {
    name: String,
    value: String,
    inline: bool,
}

impl DiscordIntegration {
    pub fn new(bot_token: String, default_channel_id: String) -> Result<Self, IntegrationError> {
        if bot_token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_DISCORD_BOT_TOKEN".into(),
            });
        }
        if default_channel_id.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_DISCORD_CHANNEL_ID".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "discord".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            bot_token,
            default_channel_id,
            http,
        })
    }

    /// Build from environment variables.
    pub fn from_env() -> Result<Self, IntegrationError> {
        let bot_token = std::env::var("NEXUS_DISCORD_BOT_TOKEN").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_DISCORD_BOT_TOKEN".into(),
            }
        })?;
        let channel_id = std::env::var("NEXUS_DISCORD_CHANNEL_ID").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_DISCORD_CHANNEL_ID".into(),
            }
        })?;
        Self::new(bot_token, channel_id)
    }

    fn severity_color(severity: &Severity) -> u32 {
        match severity {
            Severity::Critical => 0xFF4444, // red
            Severity::Warning => 0xFFB85C,  // orange
            Severity::Info => 0x4AF7D3,     // cyan
        }
    }

    fn severity_emoji(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => "🚨",
            Severity::Warning => "⚠️",
            Severity::Info => "ℹ️",
        }
    }

    fn build_embed(&self, msg: &Notification) -> DiscordEmbed {
        DiscordEmbed {
            title: Some(format!(
                "{} {}",
                Self::severity_emoji(&msg.severity),
                msg.title
            )),
            description: Some(msg.body.clone()),
            color: Some(Self::severity_color(&msg.severity)),
            fields: vec![
                DiscordField {
                    name: "Source".into(),
                    value: msg.source_event.clone(),
                    inline: true,
                },
                DiscordField {
                    name: "System".into(),
                    value: "Nexus OS".into(),
                    inline: true,
                },
            ],
        }
    }

    fn messages_url(&self, channel_id: &str) -> String {
        format!("{}/channels/{}/messages", DISCORD_API_BASE, channel_id)
    }

    fn send_to_channel(
        &self,
        channel_id: &str,
        message: &DiscordMessage,
    ) -> Result<(), IntegrationError> {
        let url = self.messages_url(channel_id);
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json")
            .json(message)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "discord".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "discord".into(),
                status,
                body,
            });
        }

        Ok(())
    }
}

impl Integration for DiscordIntegration {
    fn name(&self) -> &str {
        "Discord"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Discord
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let channel_id = message
            .channel
            .as_deref()
            .unwrap_or(&self.default_channel_id);

        let embed = self.build_embed(message);
        let discord_msg = DiscordMessage {
            content: String::new(),
            embeds: Some(vec![embed]),
        };

        self.send_to_channel(channel_id, &discord_msg)
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        // Validate token by calling GET /users/@me
        let url = format!("{}/users/@me", DISCORD_API_BASE);
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "discord".into(),
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(IntegrationError::AuthError {
                provider: "discord".into(),
                message: "Invalid bot token".into(),
            })
        }
    }

    fn send_webhook(&self, payload: &serde_json::Value) -> Result<(), IntegrationError> {
        let channel_id = payload
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_channel_id);
        let content = payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("Nexus OS notification");

        let msg = DiscordMessage {
            content: content.to_string(),
            embeds: None,
        };
        self.send_to_channel(channel_id, &msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_discord() -> DiscordIntegration {
        DiscordIntegration {
            bot_token: "test-token-12345".into(),
            default_channel_id: "123456789012345678".into(),
            http: Client::new(),
        }
    }

    #[test]
    fn discord_embed_format() {
        let discord = make_discord();
        let msg = Notification {
            title: "Agent Crash".into(),
            body: "nexus-coder OOM at tick 42".into(),
            severity: Severity::Critical,
            channel: None,
            source_event: "agent_error".into(),
        };
        let embed = discord.build_embed(&msg);
        assert_eq!(embed.color, Some(0xFF4444));
        assert!(embed.title.unwrap().contains("Agent Crash"));
        assert_eq!(embed.description, Some("nexus-coder OOM at tick 42".into()));
        assert_eq!(embed.fields.len(), 2);
        assert_eq!(embed.fields[0].name, "Source");
        assert_eq!(embed.fields[0].value, "agent_error");
    }

    #[test]
    fn discord_url_construction() {
        let discord = make_discord();
        let url = discord.messages_url("987654321");
        assert_eq!(
            url,
            "https://discord.com/api/v10/channels/987654321/messages"
        );
    }

    #[test]
    fn discord_message_serialization() {
        let msg = DiscordMessage {
            content: "Hello".into(),
            embeds: Some(vec![DiscordEmbed {
                title: Some("Test".into()),
                description: None,
                color: Some(0x00FF00),
                fields: vec![],
            }]),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["content"], "Hello");
        assert_eq!(json["embeds"][0]["title"], "Test");
        assert_eq!(json["embeds"][0]["color"], 0x00FF00);
    }

    #[test]
    fn discord_severity_colors() {
        assert_eq!(
            DiscordIntegration::severity_color(&Severity::Critical),
            0xFF4444
        );
        assert_eq!(
            DiscordIntegration::severity_color(&Severity::Warning),
            0xFFB85C
        );
        assert_eq!(
            DiscordIntegration::severity_color(&Severity::Info),
            0x4AF7D3
        );
    }

    #[test]
    fn discord_custom_channel() {
        let discord = make_discord();
        let msg = Notification {
            title: "Test".into(),
            body: "Body".into(),
            severity: Severity::Info,
            channel: Some("999888777".into()),
            source_event: "test".into(),
        };
        // The channel from the notification should override the default
        let channel = msg
            .channel
            .as_deref()
            .unwrap_or(&discord.default_channel_id);
        assert_eq!(channel, "999888777");
    }

    #[test]
    fn discord_missing_token_error() {
        let result = DiscordIntegration::new(String::new(), "12345".into());
        assert!(result.is_err());
    }

    #[test]
    fn discord_missing_channel_error() {
        let result = DiscordIntegration::new("token".into(), String::new());
        assert!(result.is_err());
    }
}
