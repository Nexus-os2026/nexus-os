//! Integration provider plugins.
//!
//! Each provider implements the [`Integration`] trait and handles
//! communication with a specific external service.

pub mod discord;
pub mod github;
pub mod gitlab;
pub mod jira;
pub mod servicenow;
pub mod slack;
pub mod teams;
pub mod telegram;
pub mod webhook;

use crate::error::IntegrationError;
use crate::events::{Notification, StatusUpdate, TicketRequest, TicketResponse};
use serde::{Deserialize, Serialize};

/// Provider type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    Slack,
    MicrosoftTeams,
    Discord,
    Telegram,
    Jira,
    ServiceNow,
    GitHub,
    GitLab,
    CustomWebhook,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Slack => write!(f, "slack"),
            Self::MicrosoftTeams => write!(f, "teams"),
            Self::Discord => write!(f, "discord"),
            Self::Telegram => write!(f, "telegram"),
            Self::Jira => write!(f, "jira"),
            Self::ServiceNow => write!(f, "servicenow"),
            Self::GitHub => write!(f, "github"),
            Self::GitLab => write!(f, "gitlab"),
            Self::CustomWebhook => write!(f, "webhook"),
        }
    }
}

/// The core integration trait. Every provider implements this.
pub trait Integration: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &str;

    /// Provider type for routing.
    fn provider_type(&self) -> ProviderType;

    /// Required agent capability (e.g. `integration.slack`).
    fn required_capability(&self) -> String {
        format!("integration.{}", self.provider_type())
    }

    /// Send a notification message.
    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError>;

    /// Create a ticket/issue.
    fn create_ticket(&self, _ticket: &TicketRequest) -> Result<TicketResponse, IntegrationError> {
        Err(IntegrationError::NotConfigured {
            provider: self.name().to_string(),
        })
    }

    /// Push a status update.
    fn update_status(&self, _update: &StatusUpdate) -> Result<(), IntegrationError> {
        Err(IntegrationError::NotConfigured {
            provider: self.name().to_string(),
        })
    }

    /// Send a raw webhook payload.
    fn send_webhook(&self, _payload: &serde_json::Value) -> Result<(), IntegrationError> {
        Err(IntegrationError::NotConfigured {
            provider: self.name().to_string(),
        })
    }

    /// Health check — returns Ok(()) if the provider is reachable.
    fn health_check(&self) -> Result<(), IntegrationError>;
}
