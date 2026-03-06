use crate::nodes::Workflow;
use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const WORKFLOW_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowVersion {
    pub version_id: String,
    pub parent_version_id: Option<String>,
    pub workflow: Workflow,
    pub message: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowArchive {
    pub schema_version: String,
    pub current_version_id: String,
    pub versions: Vec<WorkflowVersion>,
}

impl WorkflowArchive {
    pub fn from_workflow(workflow: Workflow, message: impl Into<String>) -> Self {
        let version = WorkflowVersion {
            version_id: Uuid::new_v4().to_string(),
            parent_version_id: None,
            workflow,
            message: message.into(),
            created_at: now_secs(),
        };
        Self {
            schema_version: WORKFLOW_SCHEMA_VERSION.to_string(),
            current_version_id: version.version_id.clone(),
            versions: vec![version],
        }
    }

    pub fn current_workflow(&self) -> Option<&Workflow> {
        self.versions
            .iter()
            .find(|version| version.version_id == self.current_version_id)
            .map(|version| &version.workflow)
    }

    pub fn add_version(&mut self, workflow: Workflow, message: impl Into<String>) -> String {
        let version_id = Uuid::new_v4().to_string();
        let version = WorkflowVersion {
            version_id: version_id.clone(),
            parent_version_id: Some(self.current_version_id.clone()),
            workflow,
            message: message.into(),
            created_at: now_secs(),
        };
        self.current_version_id = version_id.clone();
        self.versions.push(version);
        version_id
    }
}

pub fn save_workflow(path: impl AsRef<Path>, archive: &WorkflowArchive) -> Result<(), AgentError> {
    let serialized = serde_json::to_string_pretty(archive).map_err(|error| {
        AgentError::SupervisorError(format!("failed to serialize workflow archive: {error}"))
    })?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed to create workflow directory '{}': {error}",
                parent.display()
            ))
        })?;
    }
    fs::write(path, serialized).map_err(|error| {
        AgentError::SupervisorError(format!(
            "failed to write workflow archive '{}': {error}",
            path.display()
        ))
    })?;
    Ok(())
}

pub fn load_workflow(path: impl AsRef<Path>) -> Result<WorkflowArchive, AgentError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|error| {
        AgentError::SupervisorError(format!(
            "failed to read workflow archive '{}': {error}",
            path.display()
        ))
    })?;
    import_workflow(content.as_str())
}

pub fn export_workflow(archive: &WorkflowArchive) -> Result<String, AgentError> {
    serde_json::to_string_pretty(archive).map_err(|error| {
        AgentError::SupervisorError(format!("failed to export workflow archive: {error}"))
    })
}

pub fn import_workflow(json: &str) -> Result<WorkflowArchive, AgentError> {
    let archive = serde_json::from_str::<WorkflowArchive>(json).map_err(|error| {
        AgentError::SupervisorError(format!("failed to parse workflow archive: {error}"))
    })?;
    validate_archive(&archive)?;
    Ok(archive)
}

fn validate_archive(archive: &WorkflowArchive) -> Result<(), AgentError> {
    if archive.versions.is_empty() {
        return Err(AgentError::ManifestError(
            "workflow archive must include at least one version".to_string(),
        ));
    }

    let exists = archive
        .versions
        .iter()
        .any(|version| version.version_id == archive.current_version_id);
    if !exists {
        return Err(AgentError::ManifestError(
            "current version id does not exist in archive".to_string(),
        ));
    }
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
