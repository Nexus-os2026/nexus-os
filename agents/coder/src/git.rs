use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::errors::AgentError;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

#[derive(Debug)]
pub struct GitIntegration {
    project_root: std::path::PathBuf,
    audit_trail: AuditTrail,
    agent_id: Uuid,
}

impl GitIntegration {
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
        }
    }

    pub fn git_status(&mut self) -> Result<Vec<String>, AgentError> {
        let changed = git_status(self.project_root.as_path())?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "git.status",
                "changed_files": changed.len(),
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(changed)
    }

    pub fn git_diff(&mut self) -> Result<String, AgentError> {
        let diff = git_diff(self.project_root.as_path())?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "git.diff",
                "bytes": diff.len(),
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(diff)
    }

    pub fn git_branch(&mut self, name: &str) -> Result<(), AgentError> {
        git_branch(self.project_root.as_path(), name)?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "git.branch",
                "name": name,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(())
    }

    pub fn git_commit(&mut self, message: &str) -> Result<String, AgentError> {
        let hash = git_commit(self.project_root.as_path(), message)?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "git.commit",
                "message": message,
                "hash": hash,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(hash)
    }

    pub fn auto_commit(&mut self, description: &str) -> Result<String, AgentError> {
        let hash = auto_commit(self.project_root.as_path(), description)?;
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "git.auto_commit",
                "description": description,
                "hash": hash,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(hash)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }
}

pub fn git_status(project: impl AsRef<Path>) -> Result<Vec<String>, AgentError> {
    let stdout = run_git(project.as_ref(), ["status", "--porcelain"])?;
    let mut changed = Vec::new();
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let payload = &line[3..];
        let file = if let Some((_, right)) = payload.split_once("->") {
            right.trim()
        } else {
            payload.trim()
        };
        if !file.is_empty() {
            changed.push(file.to_string());
        }
    }
    changed.sort();
    changed.dedup();
    Ok(changed)
}

pub fn git_diff(project: impl AsRef<Path>) -> Result<String, AgentError> {
    run_git(project.as_ref(), ["diff", "--"])
}

pub fn git_commit(project: impl AsRef<Path>, message: &str) -> Result<String, AgentError> {
    let commit_output = run_git(project.as_ref(), ["commit", "-m", message])?;
    if commit_output.is_empty() {
        return Err(AgentError::SupervisorError(
            "git commit returned no output".to_string(),
        ));
    }
    run_git(project.as_ref(), ["rev-parse", "HEAD"]).map(|hash| hash.trim().to_string())
}

pub fn git_branch(project: impl AsRef<Path>, name: &str) -> Result<(), AgentError> {
    if name.trim().is_empty() {
        return Err(AgentError::ManifestError(
            "git branch name cannot be empty".to_string(),
        ));
    }
    let _ = run_git(project.as_ref(), ["checkout", "-b", name])?;
    Ok(())
}

pub fn auto_commit(project: impl AsRef<Path>, description: &str) -> Result<String, AgentError> {
    let message = format!("feat: {}", description.trim());
    let _ = run_git(project.as_ref(), ["add", "."])?;
    git_commit(project, message.as_str())
}

fn run_git<I, S>(project: &Path, args: I) -> Result<String, AgentError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<Vec<_>>();
    let output = Command::new("git")
        .current_dir(project)
        .args(args_vec.iter().map(|value| value.as_str()))
        .output()
        .map_err(|error| {
            AgentError::SupervisorError(format!(
                "failed running git {}: {error}",
                args_vec.join(" ")
            ))
        })?;

    if !output.status.success() {
        return Err(AgentError::SupervisorError(format!(
            "git {} failed: {}",
            args_vec.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
