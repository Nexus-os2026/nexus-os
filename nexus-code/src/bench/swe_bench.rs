//! SWE-bench task parsing and repository setup.

use serde::{Deserialize, Serialize};

/// A SWE-bench task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweBenchTask {
    pub instance_id: String,
    pub repo: String,
    pub base_commit: String,
    pub problem_statement: String,
    #[serde(default)]
    pub hints_text: Option<String>,
    #[serde(default)]
    pub test_patch: Option<String>,
    #[serde(default)]
    pub environment_setup_commit: Option<String>,
}

/// Load SWE-bench tasks from a JSONL file.
pub fn load_tasks(path: &std::path::Path) -> Result<Vec<SweBenchTask>, crate::error::NxError> {
    let content = std::fs::read_to_string(path)?;
    let mut tasks = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let task: SweBenchTask = serde_json::from_str(line).map_err(|e| {
            crate::error::NxError::ConfigError(format!("Failed to parse SWE-bench task: {}", e))
        })?;
        tasks.push(task);
    }
    Ok(tasks)
}

/// Set up a repository for a SWE-bench task.
pub async fn setup_repo(
    task: &SweBenchTask,
    workspace_dir: &std::path::Path,
) -> Result<std::path::PathBuf, crate::error::NxError> {
    let repo_dir = workspace_dir.join(&task.instance_id);

    if !repo_dir.exists() {
        let clone_url = format!("https://github.com/{}.git", task.repo);
        let output = tokio::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "100",
                &clone_url,
                &repo_dir.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| crate::error::NxError::ConfigError(format!("git clone failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::NxError::ConfigError(format!(
                "git clone failed: {}",
                stderr
            )));
        }
    }

    // Checkout base commit
    let checkout = tokio::process::Command::new("git")
        .args(["checkout", &task.base_commit])
        .current_dir(&repo_dir)
        .output()
        .await;

    if let Ok(output) = checkout {
        if !output.status.success() {
            // Try fetching first
            tokio::process::Command::new("git")
                .args(["fetch", "origin", &task.base_commit])
                .current_dir(&repo_dir)
                .output()
                .await
                .ok();

            tokio::process::Command::new("git")
                .args(["checkout", &task.base_commit])
                .current_dir(&repo_dir)
                .output()
                .await
                .ok();
        }
    }

    Ok(repo_dir)
}

/// Extract the patch (diff) produced by the agent.
pub async fn extract_patch(repo_dir: &std::path::Path) -> Result<String, crate::error::NxError> {
    let output = tokio::process::Command::new("git")
        .args(["diff"])
        .current_dir(repo_dir)
        .output()
        .await
        .map_err(|e| crate::error::NxError::ConfigError(format!("git diff failed: {}", e)))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
