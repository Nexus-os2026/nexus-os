use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployProvider {
    Local,
    GitHubPages,
    Vercel,
    Netlify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub provider: DeployProvider,
    pub url: String,
    pub command: String,
}

pub struct Deployer {
    agent_id: Uuid,
    audit: AuditTrail,
}

impl Default for Deployer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deployer {
    pub fn new() -> Self {
        Self {
            agent_id: Uuid::new_v4(),
            audit: AuditTrail::new(),
        }
    }

    pub fn deploy_to(
        &mut self,
        provider: DeployProvider,
        project: impl AsRef<Path>,
    ) -> Result<DeploymentResult, AgentError> {
        let project = project.as_ref();
        if !project.exists() {
            return Err(AgentError::SupervisorError(format!(
                "deploy project path '{}' does not exist",
                project.display()
            )));
        }

        let project_name = project
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("site");

        let result = match provider {
            DeployProvider::Local => DeploymentResult {
                provider,
                url: "http://127.0.0.1:4173".to_string(),
                command: "npm run build && npm run preview -- --host 127.0.0.1 --port 4173"
                    .to_string(),
            },
            DeployProvider::GitHubPages => DeploymentResult {
                provider,
                url: format!("https://example.github.io/{project_name}/"),
                command: "git checkout -b gh-pages && npm run build && git add dist && git commit -m \"deploy\" && git push origin gh-pages"
                    .to_string(),
            },
            DeployProvider::Vercel => DeploymentResult {
                provider,
                url: format!("https://{project_name}.vercel.app"),
                command: "vercel --prod".to_string(),
            },
            DeployProvider::Netlify => DeploymentResult {
                provider,
                url: format!("https://{project_name}.netlify.app"),
                command: "netlify deploy --prod --dir=dist".to_string(),
            },
        };

        self.audit
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "step": "deploy",
                    "provider": format!("{:?}", provider),
                    "project": project.to_string_lossy().to_string(),
                    "url": result.url,
                }),
            )
            .expect("audit: fail-closed");

        Ok(result)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit.events()
    }
}

pub fn deploy_to(
    provider: DeployProvider,
    project: impl AsRef<Path>,
) -> Result<DeploymentResult, AgentError> {
    let mut deployer = Deployer::new();
    deployer.deploy_to(provider, project)
}
