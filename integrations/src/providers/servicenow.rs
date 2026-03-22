//! ServiceNow integration — incident and change request creation.

use crate::error::IntegrationError;
use crate::events::{Notification, Severity, TicketRequest, TicketResponse};
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use serde_json::json;

pub struct ServiceNowIntegration {
    instance_url: String,
    username: String,
    password: String,
    http: Client,
}

impl ServiceNowIntegration {
    pub fn new(
        instance_url: String,
        username: String,
        password: String,
    ) -> Result<Self, IntegrationError> {
        if instance_url.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: "NEXUS_SNOW_INSTANCE_URL".into(),
            });
        }
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "servicenow".into(),
                message: e.to_string(),
            })?;
        Ok(Self {
            instance_url,
            username,
            password,
            http,
        })
    }

    pub fn from_env() -> Result<Self, IntegrationError> {
        let url = std::env::var("NEXUS_SNOW_INSTANCE_URL").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_SNOW_INSTANCE_URL".into(),
            }
        })?;
        let user = std::env::var("NEXUS_SNOW_USERNAME").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_SNOW_USERNAME".into(),
            }
        })?;
        let pass = std::env::var("NEXUS_SNOW_PASSWORD").map_err(|_| {
            IntegrationError::MissingCredential {
                env_var: "NEXUS_SNOW_PASSWORD".into(),
            }
        })?;
        Self::new(url, user, pass)
    }

    fn severity_to_impact(severity: &Severity) -> &'static str {
        match severity {
            Severity::Critical => "1",
            Severity::Warning => "2",
            Severity::Info => "3",
        }
    }
}

impl Integration for ServiceNowIntegration {
    fn name(&self) -> &str {
        "ServiceNow"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::ServiceNow
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let ticket = TicketRequest {
            title: message.title.clone(),
            description: message.body.clone(),
            project: String::new(),
            issue_type: "Incident".into(),
            priority: Self::severity_to_impact(&message.severity).to_string(),
            labels: vec!["nexus-os".into()],
        };
        let _ = self.create_ticket(&ticket)?;
        Ok(())
    }

    fn create_ticket(&self, ticket: &TicketRequest) -> Result<TicketResponse, IntegrationError> {
        let table = if ticket.issue_type == "Change" {
            "change_request"
        } else {
            "incident"
        };
        let url = format!("{}/api/now/table/{table}", self.instance_url);

        let payload = json!({
            "short_description": &ticket.title,
            "description": &ticket.description,
            "impact": &ticket.priority,
            "urgency": &ticket.priority,
            "category": "Nexus OS",
        });

        let response = self
            .http
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "servicenow".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "servicenow".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;

        let sys_id = body["result"]["sys_id"].as_str().unwrap_or("unknown");
        let number = body["result"]["number"].as_str().unwrap_or(sys_id);

        Ok(TicketResponse {
            ticket_id: number.to_string(),
            url: format!(
                "{}/nav_to.do?uri={table}.do?sys_id={sys_id}",
                self.instance_url
            ),
            status: "New".into(),
        })
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        let url = format!(
            "{}/api/now/table/sys_user?sysparm_limit=1",
            self.instance_url
        );
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "servicenow".into(),
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(IntegrationError::AuthError {
                provider: "servicenow".into(),
                message: format!("HTTP {}", response.status()),
            })
        }
    }
}

// ── Extended actions beyond the Integration trait ──

impl ServiceNowIntegration {
    /// Update an existing incident by sys_id.
    pub fn update_incident(
        &self,
        sys_id: &str,
        updates: serde_json::Value,
    ) -> Result<serde_json::Value, IntegrationError> {
        let url = format!("{}/api/now/table/incident/{sys_id}", self.instance_url);

        let response = self
            .http
            .patch(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&updates)
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "servicenow".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "servicenow".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;
        Ok(body["result"].clone())
    }

    /// Add a work note to an incident.
    pub fn add_work_note(&self, sys_id: &str, note: &str) -> Result<(), IntegrationError> {
        self.update_incident(sys_id, json!({ "work_notes": note }))?;
        Ok(())
    }

    /// Resolve an incident with a resolution note.
    pub fn resolve_incident(
        &self,
        sys_id: &str,
        close_notes: &str,
    ) -> Result<(), IntegrationError> {
        self.update_incident(
            sys_id,
            json!({
                "state": "6",
                "close_code": "Solved (Permanently)",
                "close_notes": close_notes,
            }),
        )?;
        Ok(())
    }

    /// Query incidents by filter.
    pub fn query_incidents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, IntegrationError> {
        let url = format!(
            "{}/api/now/table/incident?sysparm_query={}&sysparm_limit={limit}",
            self.instance_url, query
        );

        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Accept", "application/json")
            .send()
            .map_err(|e| IntegrationError::ConnectionError {
                provider: "servicenow".into(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(IntegrationError::HttpError {
                provider: "servicenow".into(),
                status,
                body,
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;
        Ok(body["result"].as_array().cloned().unwrap_or_default())
    }
}
