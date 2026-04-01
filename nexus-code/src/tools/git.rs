//! GitTool — native git operations with governance.
//! GitRead (status, log, diff, branch, show) → Tier1 auto-approved.
//! GitWrite (add, commit, checkout, push) → Tier2 consent required.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "push --force",
    "push -f",
    "reset --hard",
    "clean -fd",
    "clean -fx",
];

/// Native git operations with governance.
pub struct GitTool;

impl GitTool {
    /// Determine the required capability based on the git subcommand.
    pub fn capability_for_subcommand(subcommand: &str) -> crate::governance::Capability {
        match subcommand {
            "status" | "log" | "diff" | "show" | "remote" | "branch" => {
                crate::governance::Capability::GitRead
            }
            _ => crate::governance::Capability::GitWrite,
        }
    }
}

#[async_trait]
impl NxTool for GitTool {
    fn name(&self) -> &str {
        "git"
    }

    fn description(&self) -> &str {
        "Execute git operations. Read operations (status, log, diff, branch, show) are \
         auto-approved. Write operations (add, commit, checkout, push) require consent. \
         Forbidden: push --force, reset --hard, clean -f."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "subcommand": {
                    "type": "string",
                    "description": "Git subcommand: status, log, diff, add, commit, checkout, branch, show, stash, push, pull, merge, remote"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional arguments for the git command"
                },
                "message": {
                    "type": "string",
                    "description": "Commit message (for 'commit' subcommand)"
                }
            },
            "required": ["subcommand"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        10
    }

    fn required_capability(
        &self,
        input: &serde_json::Value,
    ) -> Option<crate::governance::Capability> {
        let subcmd = input
            .get("subcommand")
            .and_then(|v| v.as_str())
            .unwrap_or("status");
        Some(Self::capability_for_subcommand(subcmd))
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let subcommand = match input.get("subcommand").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("Missing required parameter: subcommand"),
        };

        let extra_args: Vec<String> = input
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Check forbidden patterns
        let full_cmd = format!("{} {}", subcommand, extra_args.join(" "));
        for pattern in FORBIDDEN_PATTERNS {
            if full_cmd.to_lowercase().contains(pattern) {
                return ToolResult::error(format!(
                    "Forbidden git operation: '{}'. Blocked for safety.",
                    pattern
                ));
            }
        }

        // Build git command
        let mut cmd = tokio::process::Command::new("git");
        cmd.current_dir(&ctx.working_dir);

        match subcommand {
            "commit" => {
                let message = input
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("nx: auto-commit");
                cmd.args(["commit", "-m", message]);
                for arg in &extra_args {
                    cmd.arg(arg);
                }
            }
            "log" => {
                cmd.arg("log");
                if extra_args.is_empty() {
                    cmd.args(["--oneline", "-n", "10"]);
                } else {
                    for arg in &extra_args {
                        cmd.arg(arg);
                    }
                }
            }
            "diff" => {
                cmd.arg("diff");
                if extra_args.is_empty() {
                    cmd.arg("--stat");
                } else {
                    for arg in &extra_args {
                        cmd.arg(arg);
                    }
                }
            }
            "stash" => {
                cmd.arg("stash");
                if let Some(sub) = extra_args.first() {
                    cmd.arg(sub);
                    for arg in extra_args.iter().skip(1) {
                        cmd.arg(arg);
                    }
                } else {
                    cmd.arg("list");
                }
            }
            _ => {
                cmd.arg(subcommand);
                for arg in &extra_args {
                    cmd.arg(arg);
                }
            }
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut response = String::new();
                if !stdout.is_empty() {
                    response.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !response.is_empty() {
                        response.push('\n');
                    }
                    response.push_str(&stderr);
                }
                if response.is_empty() {
                    response = format!(
                        "(no output, exit code {})",
                        output.status.code().unwrap_or(-1)
                    );
                }

                if response.len() > 50_000 {
                    response = format!("{}...\n[OUTPUT TRUNCATED]", &response[..50_000]);
                }

                if output.status.success() {
                    ToolResult::success(response)
                } else {
                    ToolResult::error(format!(
                        "git {} failed (exit {}):\n{}",
                        subcommand,
                        output.status.code().unwrap_or(-1),
                        response
                    ))
                }
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to execute git: {}", e)),
            Err(_) => ToolResult::error("Git command timed out after 30 seconds"),
        }
    }
}
