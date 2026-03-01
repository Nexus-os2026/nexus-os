use crate::connector::{Connector, HealthStatus, RetryPolicy};
use crate::http_connector::HttpConnector;
use nexus_kernel::errors::AgentError;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub struct GitHubConnector {
    id: String,
    name: String,
    retry_policy: RetryPolicy,
    degrade_gracefully: bool,
    http: HttpConnector,
}

impl GitHubConnector {
    pub fn new(agent_id: Uuid) -> Self {
        Self {
            id: "github".to_string(),
            name: "GitHub Connector".to_string(),
            retry_policy: RetryPolicy {
                max_retries: 3,
                backoff_ms: 250,
                backoff_multiplier: 2.0,
            },
            degrade_gracefully: true,
            http: HttpConnector::new("http.github", "HTTP for GitHub", agent_id),
        }
    }

    pub fn list_repos(&mut self, owner: &str) -> Result<String, AgentError> {
        let url = format!("https://api.github.com/users/{owner}/repos");
        let response = self.http.get(url.as_str(), HashMap::new())?;
        Ok(response.body)
    }

    pub fn create_issue(
        &mut self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
    ) -> Result<String, AgentError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/issues");
        let payload = json!({
            "title": title,
            "body": body
        })
        .to_string();

        let response = self.http.post(url.as_str(), payload.as_str(), HashMap::new())?;
        Ok(response.body)
    }

    pub fn get_file(&mut self, owner: &str, repo: &str, path: &str) -> Result<String, AgentError> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}");
        let response = self.http.get(url.as_str(), HashMap::new())?;
        Ok(response.body)
    }
}

impl Connector for GitHubConnector {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["net.outbound".to_string(), "github.api".to_string()]
    }

    fn health_check(&self) -> Result<HealthStatus, AgentError> {
        Ok(HealthStatus::Healthy)
    }

    fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy.clone()
    }

    fn degrade_gracefully(&self) -> bool {
        self.degrade_gracefully
    }
}

#[cfg(test)]
mod tests {
    use super::GitHubConnector;
    use crate::connector::{Connector, HealthStatus};
    use uuid::Uuid;

    #[test]
    fn test_github_connector_health() {
        let connector = GitHubConnector::new(Uuid::new_v4());
        let health = connector.health_check();
        assert_eq!(health, Ok(HealthStatus::Healthy));
    }
}
