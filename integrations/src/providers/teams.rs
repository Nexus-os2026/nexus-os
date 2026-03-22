//! Microsoft Teams integration — Adaptive Card webhook notifications.

use crate::error::IntegrationError;
use crate::events::{Notification, Severity};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct TeamsIntegration {
    webhook_url: String,
    http: Client,
}

impl TeamsIntegration {
    pub fn new(webhook_url: String) -> Result<Self, IntegrationError> {
        if webhook_url.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_TEAMS_WEBHOOK_URL".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams".into(),
                message: e.to_string(),
            })?;
        Ok(Self { webhook_url, http })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let url = std::env::var("NEXUS_TEAMS_WEBHOOK_URL").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_TEAMS_WEBHOOK_URL".into(),
            }
        })?;
        Self::new(url)
    }

    fn severity_color(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => "attention",
            Severity::Warning => "warning",
            Severity::Info => "good",
        }
    }

    fn build_adaptive_card(&self, msg: &Notification) -> serde_json::Value {
        json!({
            "type": "message",
            "attachments": [{
                "contentType": "application/vnd.microsoft.card.adaptive",
                "content": {
                    "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                    "type": "AdaptiveCard",
                    "version": "1.4",
                    "body": [
                        {
                            "type": "TextBlock",
                            "text": &msg.title,
                            "weight": "Bolder",
                            "size": "Large",
                            "color": Self::severity_color(&msg.severity)
                        },
                        {
                            "type": "TextBlock",
                            "text": &msg.body,
                            "wrap": true
                        },
                        {
                            "type": "FactSet",
                            "facts": [
                                { "title": "Source", "value": &msg.source_event },
                                { "title": "System", "value": "Nexus OS" }
                            ]
                        }
                    ]
                }
            }]
        })
    }
}

impl Integration for TeamsIntegration {
    fn name(&self) -> &str {
        "Microsoft Teams"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::MicrosoftTeams
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let payload = self.build_adaptive_card(message);

        let response = self
            .http
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "teams".into(),
                status,
                body,
            });
        }

        Ok(())
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        if self.webhook_url.is_empty() {
            return Err(IntegrationError::NotConfigured {
                provider: "teams".into(),
            });
        }
        Ok(())
    }
}

/// Microsoft Teams Graph API integration for interactive messaging.
///
/// While `TeamsIntegration` above uses incoming webhooks (notification-only),
/// this struct uses the Microsoft Graph API with an OAuth2 access token
/// to send channel messages, chat messages, and list channels.
pub struct TeamsGraphIntegration {
    access_token: String,
    http: Client,
}

impl TeamsGraphIntegration {
    pub fn new(access_token: String) -> Result<Self, IntegrationError> {
        if access_token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_TEAMS_ACCESS_TOKEN".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams-graph".into(),
                message: e.to_string(),
            })?;
        Ok(Self { access_token, http })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let token = std::env::var("NEXUS_TEAMS_ACCESS_TOKEN").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_TEAMS_ACCESS_TOKEN".into(),
            }
        })?;
        Self::new(token)
    }

    /// Send a message to a Teams channel via Graph API.
    pub fn send_channel_message(
        &self,
        team_id: &str,
        channel_id: &str,
        content: &str,
    ) -> Result<serde_json::Value, IntegrationError> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/teams/{team_id}/channels/{channel_id}/messages"
        );
        let payload = json!({
            "body": { "content": content, "contentType": "html" }
        });

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams-graph".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "teams-graph".into(),
                status,
                body,
            });
        }

        response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))
    }

    /// Send a direct chat message via Graph API.
    pub fn send_chat_message(
        &self,
        chat_id: &str,
        content: &str,
    ) -> Result<serde_json::Value, IntegrationError> {
        let url = format!("https://graph.microsoft.com/v1.0/chats/{chat_id}/messages");
        let payload = json!({
            "body": { "content": content, "contentType": "text" }
        });

        let response = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams-graph".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "teams-graph".into(),
                status,
                body,
            });
        }

        response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))
    }

    /// List channels in a team.
    pub fn list_channels(&self, team_id: &str) -> Result<Vec<serde_json::Value>, IntegrationError> {
        let url = format!("https://graph.microsoft.com/v1.0/teams/{team_id}/channels");

        let response = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams-graph".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "teams-graph".into(),
                status,
                body,
            });
        }

        let data: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;
        Ok(data["value"].as_array().cloned().unwrap_or_default())
    }

    /// Verify the access token is valid.
    pub fn health_check(&self) -> Result<(), IntegrationError> {
        let response = self
            .http
            .get("https://graph.microsoft.com/v1.0/me")
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "teams-graph".into(),
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(IntegrationError::AuthError {
                provider: "teams-graph".into(),
                message: format!("HTTP {}", response.status()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_adaptive_card_format() {
        let teams = TeamsIntegration {
            webhook_url: "https://outlook.office.com/webhook/test".into(),
            http: Client::new(),
        };

        let msg = Notification {
            title: "HITL Required".into(),
            body: "Agent nexus-deployer needs approval for production deploy".into(),
            severity: Severity::Warning,
            channel: None,
            source_event: "hitl_required".into(),
        };

        let card = teams.build_adaptive_card(&msg);
        let content = &card["attachments"][0]["content"];
        assert_eq!(content["type"], "AdaptiveCard");
        assert_eq!(content["body"][0]["text"], "HITL Required");
    }
}
