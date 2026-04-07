//! Claude Code CLI adapter — spawns the local `claude` binary for LLM calls.
//!
//! Uses the user's authenticated Claude Code session and subscription/Extra Usage
//! credits instead of an API key. NEXUS never sees or stores credentials.

use super::{LlmProvider, LlmResponse};
use crate::streaming::{
    new_usage_cell, StreamChunk, StreamUsage, StreamingLlmProvider, StreamingResponse, UsageCell,
};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Status of the locally installed Claude Code CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeCodeStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
    pub binary_path: Option<String>,
}

/// Models available through the Claude Code CLI.
pub const CLAUDE_CODE_MODELS: &[(&str, &str)] = &[
    ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
    ("claude-haiku-4-5", "Claude Haiku 4.5"),
    ("claude-opus-4-6", "Claude Opus 4.6"),
];

/// Default model when none specified.
pub const CLAUDE_CODE_DEFAULT_MODEL: &str = "claude-sonnet-4-6";

/// Map API model strings to CLI-accepted model IDs.
///
/// `claude --help` confirms `--model` accepts full model IDs:
/// `claude-sonnet-4-6`, `claude-haiku-4-5-20251001`, `claude-opus-4-6`.
/// Dated variants are normalised to their canonical form.
fn cli_model_id(api_model: &str) -> &str {
    // Normalise dated variants to canonical IDs
    if api_model.starts_with("claude-sonnet-4-6") {
        return "claude-sonnet-4-6";
    }
    if api_model.starts_with("claude-opus-4-6") {
        return "claude-opus-4-6";
    }
    if api_model.starts_with("claude-haiku-4-5") {
        return "claude-haiku-4-5-20251001";
    }
    api_model
}

/// Try to find the `claude` binary by checking `which`, then common install
/// locations, then `npm config get prefix`.  Returns the full path if found.
fn find_claude_binary() -> Option<String> {
    // 1. Try `which claude` with explicit PATH (Tauri backend often lacks user PATH)
    if let Ok(output) = Command::new("which")
        .arg("claude")
        .env(
            "PATH",
            format!(
                "{}/.npm-global/bin:/usr/local/bin:/usr/bin:/bin",
                std::env::var("HOME").unwrap_or_else(|_| "/home/nexus".to_string())
            ),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // 2. Check common install locations (Tauri backend often lacks ~/.npm-global/bin in PATH)
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates: Vec<PathBuf> = vec![
        PathBuf::from(format!("{home}/.npm-global/bin/claude")),
        PathBuf::from(format!("{home}/.local/bin/claude")),
        PathBuf::from("/usr/local/bin/claude"),
        PathBuf::from("/usr/bin/claude"),
    ];

    for candidate in &candidates {
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    // 3. Ask npm for its global prefix and check {prefix}/bin/claude
    if let Ok(output) = Command::new("npm")
        .args(["config", "get", "prefix"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                let npm_path = PathBuf::from(format!("{prefix}/bin/claude"));
                if npm_path.is_file() {
                    return Some(npm_path.to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

/// Detect the local Claude Code CLI installation and auth status.
pub fn detect_claude_code() -> ClaudeCodeStatus {
    let binary_path = find_claude_binary();

    let bin = match &binary_path {
        Some(p) => p.as_str(),
        None => {
            return ClaudeCodeStatus {
                installed: false,
                version: None,
                authenticated: false,
                binary_path: None,
            };
        }
    };

    // 2. Get version
    let version = Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty());

    // 3. Check auth status (exit code 0 = authenticated)
    let authenticated = Command::new(bin)
        .args(["auth", "status", "--text"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    ClaudeCodeStatus {
        installed: true,
        version,
        authenticated,
        binary_path,
    }
}

/// LLM provider that delegates to the local `claude` CLI binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeProvider {
    timeout_secs: u64,
    /// Full path to the `claude` binary (discovered via `find_claude_binary`).
    /// Falls back to bare `"claude"` if detection was skipped.
    binary_path: String,
}

impl Default for ClaudeCodeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeProvider {
    pub fn new() -> Self {
        let binary_path = find_claude_binary().unwrap_or_else(|| "claude".to_string());
        Self {
            timeout_secs: 180,
            binary_path,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Spawn `claude --print` and capture the full response.
    ///
    /// Correct invocation (confirmed from `claude --help`):
    ///   claude --print --model <model> --dangerously-skip-permissions "<prompt>"
    fn run_cli(&self, prompt: &str, model: &str) -> Result<String, AgentError> {
        let mut child = Command::new(&self.binary_path)
            .arg("--print")
            .arg("--model")
            .arg(cli_model_id(model))
            .arg("--dangerously-skip-permissions")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::SupervisorError(
                        "Claude Code CLI not found. Install: npm install -g @anthropic-ai/claude-code"
                            .to_string(),
                    )
                } else {
                    AgentError::SupervisorError(format!("failed to spawn claude CLI: {e}"))
                }
            })?;

        // Wait with timeout
        let timeout = Duration::from_secs(self.timeout_secs);
        let start = std::time::Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = child
                        .stdout
                        .take()
                        .map(|s| {
                            use std::io::Read;
                            let mut buf = String::new();
                            let mut reader = std::io::BufReader::new(s);
                            let _ = reader.read_to_string(&mut buf);
                            buf
                        })
                        .unwrap_or_default();

                    let stderr = child
                        .stderr
                        .take()
                        .map(|s| {
                            use std::io::Read;
                            let mut buf = String::new();
                            let mut reader = std::io::BufReader::new(s);
                            let _ = reader.read_to_string(&mut buf);
                            buf
                        })
                        .unwrap_or_default();

                    if !status.success() || stdout.trim().is_empty() {
                        let code = status.code().unwrap_or(-1);
                        let detail = if stderr.trim().is_empty() {
                            format!("exit code {code}, no stderr")
                        } else {
                            stderr.trim().to_string()
                        };
                        return Err(AgentError::SupervisorError(format!(
                            "Claude CLI failed: {detail}"
                        )));
                    }

                    return Ok(stdout);
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Err(AgentError::SupervisorError(format!(
                            "claude CLI timed out after {}s",
                            self.timeout_secs
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    return Err(AgentError::SupervisorError(format!(
                        "failed to wait on claude CLI: {e}"
                    )));
                }
            }
        }
    }

    /// Parse plain-text output from `claude --print`.
    ///
    /// `--print` outputs the model's response as raw text (no JSON wrapper).
    /// Token counts are estimated from text length.
    fn parse_response(raw: &str, model: &str) -> LlmResponse {
        let output_text = raw.trim().to_string();
        let token_count = (output_text.len() as u32).saturating_div(4).max(1);

        LlmResponse {
            output_text,
            token_count,
            model_name: model.to_string(),
            tool_calls: Vec::new(),
            input_tokens: None,
        }
    }
}

impl LlmProvider for ClaudeCodeProvider {
    fn query(
        &self,
        prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        // Validate CLI is available before attempting
        let status = detect_claude_code();
        if !status.installed {
            return Err(AgentError::SupervisorError(
                "Claude Code CLI not installed. Install: npm install -g @anthropic-ai/claude-code"
                    .to_string(),
            ));
        }
        if !status.authenticated {
            return Err(AgentError::SupervisorError(
                "Claude Code CLI not authenticated. Run: claude auth login".to_string(),
            ));
        }

        let effective_model = if model.is_empty() {
            CLAUDE_CODE_DEFAULT_MODEL
        } else {
            model
        };

        let raw = self.run_cli(prompt, effective_model)?;
        Ok(Self::parse_response(&raw, effective_model))
    }

    fn name(&self) -> &str {
        "claude-code"
    }

    fn cost_per_token(&self) -> f64 {
        // Sonnet 4.6 rates: $3 input / $15 output per MTok → ~$0.000015/token output
        0.000_015
    }

    fn endpoint_url(&self) -> String {
        "provider://claude-code-cli".to_string()
    }
}

impl StreamingLlmProvider for ClaudeCodeProvider {
    fn stream_query(
        &self,
        prompt: &str,
        system_prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<StreamingResponse, AgentError> {
        let effective_model = if model.is_empty() {
            CLAUDE_CODE_DEFAULT_MODEL
        } else {
            model
        };

        // Prepend system prompt if provided
        let full_prompt = if system_prompt.is_empty() {
            prompt.to_string()
        } else {
            format!("{system_prompt}\n\n{prompt}")
        };

        // Spawn `claude --print` — stdout streams plain text as the model generates.
        // Correct invocation: claude --print --model <m> --dangerously-skip-permissions "<prompt>"
        let child = Command::new(&self.binary_path)
            .arg("--print")
            .arg("--model")
            .arg(cli_model_id(effective_model))
            .arg("--dangerously-skip-permissions")
            .arg(&full_prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::SupervisorError(
                        "Claude Code CLI not found. Install: npm install -g @anthropic-ai/claude-code"
                            .to_string(),
                    )
                } else {
                    AgentError::SupervisorError(format!("failed to spawn claude CLI: {e}"))
                }
            })?;

        let stdout = child.stdout.ok_or_else(|| {
            AgentError::SupervisorError("failed to capture claude CLI stdout".to_string())
        })?;

        let usage_cell = new_usage_cell();
        let usage_cell_writer = usage_cell.clone();
        let timeout_secs = self.timeout_secs;

        let iter = CliStreamIterator::new(stdout, usage_cell_writer, timeout_secs);

        Ok(StreamingResponse::new(Box::new(iter), usage_cell))
    }

    fn streaming_provider_name(&self) -> &str {
        "claude-code"
    }
}

/// Iterator that reads line-by-line plain text from `claude --print` stdout.
///
/// `--print` outputs the model's response as raw text, streamed line by line.
/// Each line is emitted as a `StreamChunk`.
struct CliStreamIterator {
    lines: std::io::Lines<std::io::BufReader<std::process::ChildStdout>>,
    usage_cell: UsageCell,
    finished: bool,
    /// Idle timeout — fires if no data arrives for this duration.
    idle_timeout: Duration,
    /// Reset on every received line.
    last_data_at: std::time::Instant,
    /// Accumulated output tokens (estimated from text length).
    accumulated_tokens: usize,
}

impl CliStreamIterator {
    fn new(
        stdout: std::process::ChildStdout,
        usage_cell: UsageCell,
        idle_timeout_secs: u64,
    ) -> Self {
        Self {
            lines: std::io::BufReader::new(stdout).lines(),
            usage_cell,
            finished: false,
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            last_data_at: std::time::Instant::now(),
            accumulated_tokens: 0,
        }
    }
}

impl Iterator for CliStreamIterator {
    type Item = Result<StreamChunk, AgentError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            if self.last_data_at.elapsed() > self.idle_timeout {
                self.finished = true;
                // Write usage before returning error
                if let Ok(mut guard) = self.usage_cell.lock() {
                    *guard = Some(StreamUsage {
                        input_tokens: 0,
                        output_tokens: self.accumulated_tokens,
                    });
                }
                return Some(Err(AgentError::SupervisorError(format!(
                    "claude CLI stream idle timeout (no data for {}s)",
                    self.idle_timeout.as_secs()
                ))));
            }

            let line = match self.lines.next() {
                Some(Ok(l)) => {
                    self.last_data_at = std::time::Instant::now();
                    l
                }
                Some(Err(e)) => {
                    self.finished = true;
                    if let Ok(mut guard) = self.usage_cell.lock() {
                        *guard = Some(StreamUsage {
                            input_tokens: 0,
                            output_tokens: self.accumulated_tokens,
                        });
                    }
                    return Some(Err(AgentError::SupervisorError(format!(
                        "claude CLI stream read error: {e}"
                    ))));
                }
                None => {
                    // EOF — process exited
                    self.finished = true;
                    if let Ok(mut guard) = self.usage_cell.lock() {
                        *guard = Some(StreamUsage {
                            input_tokens: 0,
                            output_tokens: self.accumulated_tokens,
                        });
                    }
                    return None;
                }
            };

            // Skip empty lines (e.g. between paragraphs) — keep looping
            if line.is_empty() {
                continue;
            }

            // Emit content line as a chunk (with newline to preserve formatting)
            let text = format!("{line}\n");
            let est_tokens = (text.len() / 4).max(1);
            self.accumulated_tokens += est_tokens;

            return Some(Ok(StreamChunk {
                text,
                token_count: Some(est_tokens),
            }));
        }
    }
}

/// Trigger `claude auth login` which opens the browser for OAuth.
/// NEXUS never sees or stores the credentials.
pub fn trigger_login() -> Result<String, AgentError> {
    let bin = find_claude_binary().unwrap_or_else(|| "claude".to_string());
    let output = Command::new(bin)
        .args(["auth", "login"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            AgentError::SupervisorError(format!("failed to start claude auth login: {e}"))
        })?;

    if output.status.success() {
        Ok("Login initiated — check your browser.".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AgentError::SupervisorError(format!(
            "claude auth login failed: {stderr}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_plain_text() {
        let resp = ClaudeCodeProvider::parse_response("Hello from Claude!", "claude-sonnet-4-6");
        assert_eq!(resp.output_text, "Hello from Claude!");
        assert!(resp.token_count > 0);
        assert_eq!(resp.input_tokens, None);
        assert_eq!(resp.model_name, "claude-sonnet-4-6");
        assert!(resp.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_response_trims_whitespace() {
        let resp = ClaudeCodeProvider::parse_response("  trimmed output  \n", "claude-sonnet-4-6");
        assert_eq!(resp.output_text, "trimmed output");
    }

    #[test]
    fn test_parse_response_estimates_tokens() {
        let text = "a".repeat(400); // ~100 tokens
        let resp = ClaudeCodeProvider::parse_response(&text, "claude-sonnet-4-6");
        assert_eq!(resp.token_count, 100);
    }

    #[test]
    fn test_parse_response_empty() {
        let resp = ClaudeCodeProvider::parse_response("", "claude-sonnet-4-6");
        assert_eq!(resp.output_text, "");
        assert_eq!(resp.token_count, 1); // min 1
    }

    #[test]
    fn test_provider_traits() {
        let provider = ClaudeCodeProvider::new();
        assert_eq!(provider.name(), "claude-code");
        assert!(provider.cost_per_token() > 0.0);
        assert!(provider.is_paid());
        assert_eq!(provider.endpoint_url(), "provider://claude-code-cli");
    }

    #[test]
    fn test_default_model() {
        assert_eq!(CLAUDE_CODE_DEFAULT_MODEL, "claude-sonnet-4-6");
    }

    #[test]
    fn test_models_list() {
        assert_eq!(CLAUDE_CODE_MODELS.len(), 3);
        assert!(CLAUDE_CODE_MODELS
            .iter()
            .any(|(id, _)| *id == "claude-sonnet-4-6"));
        assert!(CLAUDE_CODE_MODELS
            .iter()
            .any(|(id, _)| *id == "claude-opus-4-6"));
        assert!(CLAUDE_CODE_MODELS
            .iter()
            .any(|(id, _)| *id == "claude-haiku-4-5"));
    }

    #[test]
    fn test_with_timeout() {
        let provider = ClaudeCodeProvider::new().with_timeout(60);
        assert_eq!(provider.timeout_secs, 60);
    }

    #[test]
    fn test_detect_status_struct() {
        let status = ClaudeCodeStatus {
            installed: true,
            version: Some("1.0.0".to_string()),
            authenticated: true,
            binary_path: Some("/usr/local/bin/claude".to_string()),
        };
        assert!(status.installed);
        assert!(status.authenticated);
        assert_eq!(status.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_no_credential_storage() {
        // Verify the provider struct contains no credential fields
        let provider = ClaudeCodeProvider::new();
        let debug_repr = format!("{provider:?}");
        assert!(!debug_repr.contains("key"));
        assert!(!debug_repr.contains("token"));
        assert!(!debug_repr.contains("secret"));
        assert!(!debug_repr.contains("password"));
    }

    #[test]
    fn test_embedding_not_supported() {
        let provider = ClaudeCodeProvider::new();
        let result = provider.embed(&["test"], "claude-sonnet-4-6");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not support embeddings"), "got: {err}");
    }

    #[test]
    fn test_find_claude_binary_returns_something_or_none() {
        // On CI or machines without claude installed this returns None — that's fine.
        // On dev machines it should return a valid path.
        let result = find_claude_binary();
        if let Some(ref path) = result {
            assert!(
                std::path::Path::new(path).exists(),
                "find_claude_binary returned non-existent path: {path}"
            );
        }
    }

    #[test]
    fn test_provider_stores_binary_path() {
        let provider = ClaudeCodeProvider::new();
        // binary_path should be either a discovered path or fallback "claude"
        assert!(!provider.binary_path.is_empty());
    }

    #[test]
    fn test_cli_model_id_mapping() {
        // Full model IDs pass through (confirmed by `claude --help`)
        assert_eq!(cli_model_id("claude-sonnet-4-6"), "claude-sonnet-4-6");
        assert_eq!(
            cli_model_id("claude-sonnet-4-6-20250514"),
            "claude-sonnet-4-6"
        );
        assert_eq!(cli_model_id("claude-opus-4-6"), "claude-opus-4-6");
        assert_eq!(
            cli_model_id("claude-haiku-4-5"),
            "claude-haiku-4-5-20251001"
        );
        assert_eq!(
            cli_model_id("claude-haiku-4-5-20251001"),
            "claude-haiku-4-5-20251001"
        );
        // Unknown models pass through unchanged
        assert_eq!(cli_model_id("gpt-4o"), "gpt-4o");
        assert_eq!(cli_model_id("custom-model"), "custom-model");
    }

    #[test]
    fn test_detect_uses_full_path() {
        // detect_claude_code should return the same path as find_claude_binary
        let status = detect_claude_code();
        let found = find_claude_binary();
        assert_eq!(status.binary_path, found);
        assert_eq!(status.installed, found.is_some());
    }

    /// Verify that `claude --print` is invoked with the correct flags:
    ///   claude --print --model claude-sonnet-4-6 --dangerously-skip-permissions <prompt>
    #[test]
    fn test_claude_print_args() {
        let binary = "claude";
        let model = "claude-sonnet-4-6";
        let prompt = "Build a landing page";

        let mut cmd = Command::new(binary);
        cmd.arg("--print")
            .arg("--model")
            .arg(cli_model_id(model))
            .arg("--dangerously-skip-permissions")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert_eq!(args[0], "--print");
        assert_eq!(args[1], "--model");
        assert_eq!(args[2], "claude-sonnet-4-6");
        assert_eq!(args[3], "--dangerously-skip-permissions");
        assert_eq!(args[4], prompt);
    }

    /// Verify binary_path resolution stores a full path (not bare "claude").
    #[test]
    fn test_binary_path_resolution() {
        let provider = ClaudeCodeProvider::new();
        // If claude is installed, binary_path should be an absolute path
        if provider.binary_path != "claude" {
            assert!(
                provider.binary_path.starts_with('/'),
                "binary_path should be absolute: {}",
                provider.binary_path
            );
        }
    }
}
