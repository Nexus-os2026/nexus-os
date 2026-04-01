//! Bash tool — governed shell execution with output truncation,
//! environment sanitization, timeout, and dangerous command detection.

use super::{NxTool, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

/// Maximum output size in bytes. Prevents context blowup from
/// commands that dump enormous output (e.g., `cat /dev/urandom`).
const MAX_OUTPUT_BYTES: usize = 100 * 1024; // 100KB

/// Environment variables that are stripped before execution.
/// Prevents accidental leakage of API keys in command output.
const SENSITIVE_ENV_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GOOGLE_API_KEY",
    "OPENROUTER_API_KEY",
    "GROQ_API_KEY",
    "DEEPSEEK_API_KEY",
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
];

/// Commands that are considered dangerous and require Tier3 consent.
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "rm -r ",
    "sudo ",
    "chmod 777",
    "mkfs",
    "dd if=",
    "> /dev/",
    ":(){ :",
    "shutdown",
    "reboot",
    "kill -9",
    "pkill -9",
    "killall",
    "curl | sh",
    "curl | bash",
    "wget | sh",
    "wget | bash",
];

/// Execute a shell command with governance controls.
/// Features: timeout, output truncation, env sanitization, dangerous detection.
pub struct BashTool;

impl BashTool {
    /// Check if a command contains dangerous patterns.
    pub fn is_dangerous(command: &str) -> bool {
        let lower = command.to_lowercase();
        DANGEROUS_PATTERNS.iter().any(|p| lower.contains(p))
    }
}

#[async_trait]
impl NxTool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command in the session's working directory. Has a \
         configurable timeout (default 30s, max 300s). Output is truncated at \
         100KB. Dangerous commands (rm -rf, sudo, curl|sh, etc.) require Tier3 \
         consent. Sensitive environment variables (API keys) are stripped."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30, max: 300)"
                }
            },
            "required": ["command"]
        })
    }

    fn estimated_fuel(&self, _input: &serde_json::Value) -> u64 {
        20
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter: command"),
        };

        let timeout_secs = input
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30)
            .min(300);

        // Build command with env sanitization
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(&ctx.working_dir);

        // Strip sensitive environment variables
        for var in SENSITIVE_ENV_VARS {
            cmd.env_remove(var);
        }

        // Execute with timeout
        let result = tokio::time::timeout(Duration::from_secs(timeout_secs), cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut response = String::new();
                if !stdout.is_empty() {
                    response.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !response.is_empty() {
                        response.push('\n');
                    }
                    response.push_str("STDERR:\n");
                    response.push_str(&stderr);
                }
                if response.is_empty() {
                    response = format!("(no output, exit code {})", exit_code);
                }

                // Truncate output if too large
                let truncated = if response.len() > MAX_OUTPUT_BYTES {
                    let truncated_response = &response[..MAX_OUTPUT_BYTES];
                    format!(
                        "{}\n\n[OUTPUT TRUNCATED: {} bytes total, showing first {}]",
                        truncated_response,
                        response.len(),
                        MAX_OUTPUT_BYTES
                    )
                } else {
                    response
                };

                if output.status.success() {
                    ToolResult::success(truncated)
                } else {
                    ToolResult::error(format!("Exit code {}\n{}", exit_code, truncated))
                }
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to execute command: {}", e)),
            Err(_) => ToolResult::error(format!(
                "Command timed out after {} seconds. Consider increasing timeout_secs.",
                timeout_secs
            )),
        }
    }
}
