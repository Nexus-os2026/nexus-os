//! Slack integration — webhook notifications with Block Kit formatting.

use crate::error::IntegrationError;
use crate::events::{Notification, Severity};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct SlackIntegration {
    webhook_url: String,
    bot_token: Option<String>,
    default_channel: String,
    http: Client,
}

impl SlackIntegration {
    pub fn new(
        webhook_url: String,
        bot_token: Option<String>,
        default_channel: String,
    ) -> Result<Self, IntegrationError> {
        if webhook_url.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_SLACK_WEBHOOK_URL".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "slack".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            webhook_url,
            bot_token,
            default_channel,
            http,
        })
    }

    /// Build from environment variables.
    pub fn from_env() -> Result<Self, IntegrationError> {
        let webhook_url = std::env::var("NEXUS_SLACK_WEBHOOK_URL").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_SLACK_WEBHOOK_URL".into(),
            }
        })?;
        let bot_token = std::env::var("NEXUS_SLACK_BOT_TOKEN").ok();
        let channel =
            std::env::var("NEXUS_SLACK_CHANNEL").unwrap_or_else(|_| "#nexus-agents".into());
        Self::new(webhook_url, bot_token, channel)
    }

    fn severity_emoji(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => ":rotating_light:",
            Severity::Warning => ":warning:",
            Severity::Info => ":information_source:",
        }
    }

    fn severity_color(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => "#ff4444",
            Severity::Warning => "#ffb85c",
            Severity::Info => "#4af7d3",
        }
    }

    fn build_payload(&self, msg: &Notification) -> serde_json::Value {
        let channel = msg
            .channel
            .as_deref()
            .unwrap_or(self.default_channel.as_str());
        json!({
            "channel": channel,
            "attachments": [{
                "color": Self::severity_color(&msg.severity),
                "blocks": [
                    {
                        "type": "header",
                        "text": {
                            "type": "plain_text",
                            "text": format!("{} {}", Self::severity_emoji(&msg.severity), msg.title)
                        }
                    },
                    {
                        "type": "section",
                        "text": {
                            "type": "mrkdwn",
                            "text": &msg.body
                        }
                    },
                    {
                        "type": "context",
                        "elements": [{
                            "type": "mrkdwn",
                            "text": format!("Source: {} | Nexus OS", msg.source_event)
                        }]
                    }
                ]
            }]
        })
    }
}

impl Integration for SlackIntegration {
    fn name(&self) -> &str {
        "Slack"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Slack
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let payload = self.build_payload(message);

        let mut request = self.http.post(&self.webhook_url).json(&payload);
        if let Some(token) = &self.bot_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "slack".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "slack".into(),
                status,
                body,
            });
        }

        Ok(())
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        if self.webhook_url.is_empty() {
            return Err(IntegrationError::NotConfigured {
                provider: "slack".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slack_payload_format() {
        let slack = SlackIntegration {
            webhook_url: "https://hooks.slack.com/test".into(),
            bot_token: None,
            default_channel: "#nexus".into(),
            http: Client::new(),
        };

        let msg = Notification {
            title: "Agent Error".into(),
            body: "nexus-coder crashed with OOM".into(),
            severity: Severity::Critical,
            channel: None,
            source_event: "agent_error".into(),
        };

        let payload = slack.build_payload(&msg);
        assert_eq!(payload["channel"], "#nexus");
        assert_eq!(payload["attachments"][0]["color"], "#ff4444");
        let header = &payload["attachments"][0]["blocks"][0];
        assert!(header["text"]["text"]
            .as_str()
            .unwrap()
            .contains("Agent Error"));
    }

    #[test]
    fn slack_custom_channel() {
        let slack = SlackIntegration {
            webhook_url: "https://hooks.slack.com/test".into(),
            bot_token: None,
            default_channel: "#nexus".into(),
            http: Client::new(),
        };

        let msg = Notification {
            title: "Test".into(),
            body: "Body".into(),
            severity: Severity::Info,
            channel: Some("#alerts".into()),
            source_event: "system_alert".into(),
        };

        let payload = slack.build_payload(&msg);
        assert_eq!(payload["channel"], "#alerts");
    }
}
