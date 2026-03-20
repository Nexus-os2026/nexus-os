//! Jira integration — ticket creation and status sync.

use crate::error::IntegrationError;
use crate::events::{Notification, TicketRequest, TicketResponse};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct JiraIntegration {
    base_url: String,
    email: String,
    api_token: String,
    default_project: String,
    http: Client,
}

impl JiraIntegration {
    pub fn new(
        base_url: String,
        email: String,
        api_token: String,
        default_project: String,
    ) -> Result<Self, IntegrationError> {
        if base_url.is_empty() || api_token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_JIRA_TOKEN".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "jira".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            base_url,
            email,
            api_token,
            default_project,
            http,
        })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let base_url = std::env::var("NEXUS_JIRA_BASE_URL").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_JIRA_BASE_URL".into(),
            }
        })?;
        let email =
            std::env::var("NEXUS_JIRA_EMAIL").map_err(|_| IntegrationError::MissingCredential {
                env_var: "NEXUS_JIRA_EMAIL".into(),
            })?;
        let api_token =
            std::env::var("NEXUS_JIRA_TOKEN").map_err(|_| IntegrationError::MissingCredential {
                env_var: "NEXUS_JIRA_TOKEN".into(),
            })?;
        let project = std::env::var("NEXUS_JIRA_PROJECT").unwrap_or_else(|_| "NEXUS".to_string());
        Self::new(base_url, email, api_token, project)
    }

    fn auth_header(&self) -> String {
        use base64::Engine;
        let credentials = format!("{}:{}", self.email, self.api_token);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        format!("Basic {encoded}")
    }

    fn build_issue_payload(&self, ticket: &TicketRequest) -> serde_json::Value {
        let project = if ticket.project.is_empty() {
            &self.default_project
        } else {
            &ticket.project
        };

        let issue_type = if ticket.issue_type.is_empty() {
            "Task"
        } else {
            &ticket.issue_type
        };

        json!({
            "fields": {
                "project": { "key": project },
                "summary": &ticket.title,
                "description": {
                    "type": "doc",
                    "version": 1,
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": &ticket.description
                        }]
                    }]
                },
                "issuetype": { "name": issue_type },
                "priority": { "name": if ticket.priority.is_empty() { "Medium" } else { &ticket.priority } },
                "labels": &ticket.labels,
            }
        })
    }
}

impl Integration for JiraIntegration {
    fn name(&self) -> &str {
        "Jira"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Jira
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        // Jira notifications map to creating a ticket.
        let ticket = TicketRequest {
            title: message.title.clone(),
            description: message.body.clone(),
            project: String::new(),
            issue_type: "Task".into(),
            priority: match message.severity {
                crate::events::Severity::Critical => "Highest".into(),
                crate::events::Severity::Warning => "High".into(),
                crate::events::Severity::Info => "Medium".into(),
            },
            labels: vec!["nexus-os".into(), "automated".into()],
        };
        let _ = self.create_ticket(&ticket)?;
        Ok(())
    }

    fn create_ticket(&self, ticket: &TicketRequest) -> Result<TicketResponse, IntegrationError> {
        let url = format!("{}/rest/api/3/issue", self.base_url);
        let payload = self.build_issue_payload(ticket);

        let response = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "jira".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "jira".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;

        let key = body["key"].as_str().unwrap_or("UNKNOWN");
        Ok(TicketResponse {
            ticket_id: key.to_string(),
            url: format!("{}/browse/{key}", self.base_url),
            status: "Created".into(),
        })
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        let url = format!("{}/rest/api/3/myself", self.base_url);
        let response = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "jira".into(),
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(IntegrationError::AuthError {
                provider: "jira".into(),
                message: format!("HTTP {}", response.status()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jira_issue_payload_format() {
        let jira = JiraIntegration {
            base_url: "https://test.atlassian.net".into(),
            email: "user@test.com".into(),
            api_token: "test-token".into(),
            default_project: "NEXUS".into(),
            http: Client::new(),
        };

        let ticket = TicketRequest {
            title: "Agent OOM".into(),
            description: "nexus-coder ran out of memory".into(),
            project: String::new(),
            issue_type: "Bug".into(),
            priority: "Highest".into(),
            labels: vec!["agent-error".into()],
        };

        let payload = jira.build_issue_payload(&ticket);
        assert_eq!(payload["fields"]["project"]["key"], "NEXUS");
        assert_eq!(payload["fields"]["summary"], "Agent OOM");
        assert_eq!(payload["fields"]["issuetype"]["name"], "Bug");
        assert_eq!(payload["fields"]["priority"]["name"], "Highest");
    }

    #[test]
    fn jira_auth_header_format() {
        let jira = JiraIntegration {
            base_url: "https://test.atlassian.net".into(),
            email: "user@test.com".into(),
            api_token: "abc123".into(),
            default_project: "TEST".into(),
            http: Client::new(),
        };

        let header = jira.auth_header();
        assert!(header.starts_with("Basic "));
        // Decode and verify
        use base64::Engine;
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded_str = String::from_utf8(decoded).unwrap();
        assert_eq!(decoded_str, "user@test.com:abc123");
    }
}
