//! GitLab integration — issue creation and pipeline status.

use crate::error::IntegrationError;
use crate::events::{Notification, TicketRequest, TicketResponse};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct GitLabIntegration {
    base_url: String,
    token: String,
    default_project_id: String,
    http: Client,
}

impl GitLabIntegration {
    pub fn new(
        base_url: String,
        token: String,
        default_project_id: String,
    ) -> Result<Self, IntegrationError> {
        if token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_GITLAB_TOKEN".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "gitlab".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            base_url,
            token,
            default_project_id,
            http,
        })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let base_url =
            std::env::var("NEXUS_GITLAB_BASE_URL").unwrap_or_else(|_| "https://gitlab.com".into());
        let token = std::env::var("NEXUS_GITLAB_TOKEN").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_GITLAB_TOKEN".into(),
            }
        })?;
        let project_id =
            std::env::var("NEXUS_GITLAB_PROJECT_ID").unwrap_or_else(|_| "nexaiceo/nexus-os".into());
        Self::new(base_url, token, project_id)
    }

    fn project_id<'a>(&'a self, project: &'a str) -> &'a str {
        if project.is_empty() {
            &self.default_project_id
        } else {
            project
        }
    }

    fn encode_project_id(id: &str) -> String {
        id.replace('/', "%2F")
    }
}

impl Integration for GitLabIntegration {
    fn name(&self) -> &str {
        "GitLab"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::GitLab
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let ticket = TicketRequest {
            title: message.title.clone(),
            description: message.body.clone(),
            project: String::new(),
            issue_type: "issue".into(),
            priority: String::new(),
            labels: vec!["nexus-os".into(), "automated".into()],
        };
        let _ = self.create_ticket(&ticket)?;
        Ok(())
    }

    fn create_ticket(&self, ticket: &TicketRequest) -> Result<TicketResponse, IntegrationError> {
        let proj = self.project_id(&ticket.project);
        let encoded = Self::encode_project_id(proj);
        let url = format!("{}/api/v4/projects/{encoded}/issues", self.base_url);

        let payload = json!({
            "title": &ticket.title,
            "description": &ticket.description,
            "labels": ticket.labels.join(","),
        });

        let response = self
            .http
            .post(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "gitlab".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "gitlab".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;

        Ok(TicketResponse {
            ticket_id: body["iid"].to_string(),
            url: body["web_url"].as_str().unwrap_or("").to_string(),
            status: "opened".into(),
        })
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        if self.token.is_empty() {
            return Err(IntegrationError::NotConfigured {
                provider: "gitlab".into(),
            });
        }
        Ok(())
    }
}
