use nexus_sdk::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewHandle {
    pub url: String,
    pub command: String,
    pub project_path: String,
}

pub fn start_preview(project_path: impl AsRef<Path>) -> Result<PreviewHandle, AgentError> {
    let project_path = project_path.as_ref();
    if !project_path.exists() {
        return Err(AgentError::SupervisorError(format!(
            "preview path '{}' does not exist",
            project_path.display()
        )));
    }

    let package_json = project_path.join("package.json");
    if !package_json.exists() {
        return Err(AgentError::SupervisorError(format!(
            "project '{}' is missing package.json",
            project_path.display()
        )));
    }

    Ok(PreviewHandle {
        url: "http://127.0.0.1:5173".to_string(),
        command: "npm run dev -- --host 127.0.0.1 --port 5173".to_string(),
        project_path: PathBuf::from(project_path).to_string_lossy().to_string(),
    })
}
