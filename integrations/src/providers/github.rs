//! GitHub integration — issue creation and status checks.

use crate::error::IntegrationError;
use crate::events::{Notification, StatusUpdate, TicketRequest, TicketResponse};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct GitHubIntegration {
    token: String,
    default_owner: String,
    default_repo: String,
    http: Client,
}

impl GitHubIntegration {
    pub fn new(
        token: String,
        default_owner: String,
        default_repo: String,
    ) -> Result<Self, IntegrationError> {
        if token.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_GITHUB_TOKEN".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "github".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            token,
            default_owner,
            default_repo,
            http,
        })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let token = std::env::var("NEXUS_GITHUB_TOKEN").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_GITHUB_TOKEN".into(),
            }
        })?;
        let owner = std::env::var("NEXUS_GITHUB_OWNER").unwrap_or_else(|_| "nexaiceo".into());
        let repo = std::env::var("NEXUS_GITHUB_REPO").unwrap_or_else(|_| "nexus-os".into());
        Self::new(token, owner, repo)
    }

    fn parse_owner_repo<'a>(&'a self, project: &'a str) -> (&'a str, &'a str) {
        if let Some((owner, repo)) = project.split_once('/') {
            (owner, repo)
        } else {
            (&self.default_owner, &self.default_repo)
        }
    }
}

impl Integration for GitHubIntegration {
    fn name(&self) -> &str {
        "GitHub"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::GitHub
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
        let (owner, repo) = self.parse_owner_repo(&ticket.project);
        let url = format!("https://api.github.com/repos/{owner}/{repo}/issues");

        let payload = json!({
            "title": &ticket.title,
            "body": &ticket.description,
            "labels": &ticket.labels,
        });

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "nexus-os/9.0.0")
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "github".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "github".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;

        Ok(TicketResponse {
            ticket_id: body["number"].to_string(),
            url: body["html_url"].as_str().unwrap_or("").to_string(),
            status: "open".into(),
        })
    }

    fn update_status(&self, update: &StatusUpdate) -> Result<(), IntegrationError> {
        let (owner, repo) = self.parse_owner_repo("");
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/statuses/{}",
            update.resource_id
        );

        let payload = json!({
            "state": &update.status,
            "description": &update.message,
            "context": "nexus-os/integration",
        });

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "nexus-os/9.0.0")
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "github".into(),
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            Err(IntegrationError::HttpError {
                provider: "github".into(),
                status,
                body,
            })
        }
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        if self.token.is_empty() {
            return Err(IntegrationError::NotConfigured {
                provider: "github".into(),
            });
        }
        Ok(())
    }
}
