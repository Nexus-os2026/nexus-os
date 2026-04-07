//! Claude CLI provider — uses the `claude` binary (Claude Code) as an LLM backend.
//!
//! This enables using a Claude Max plan ($0 cost) instead of API credits.
//! The binary is invoked in print mode: `claude -p --output-format json "prompt"`

use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::error::NxError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{LlmRequest, LlmResponse, Role, StreamChunk, TokenUsage};

/// Provider that shells out to the `claude` CLI binary.
pub struct ClaudeCliProvider {
    /// Path to the claude binary (resolved at construction).
    binary_path: Option<String>,
    /// Working directory for claude to have codebase context.
    working_dir: PathBuf,
}

impl ClaudeCliProvider {
    /// Create a new Claude CLI provider.
    pub fn new() -> Self {
        let binary_path = find_claude_binary();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            binary_path,
            working_dir,
        }
    }
}

impl Default for ClaudeCliProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if `claude` exists on PATH and return its path.
fn find_claude_binary() -> Option<String> {
    std::process::Command::new("which")
        .arg("claude")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Build the prompt string from an LlmRequest's messages.
fn build_prompt(request: &LlmRequest) -> String {
    let mut parts = Vec::new();

    // Include system prompt if present
    if let Some(ref sys) = request.system {
        if !sys.is_empty() {
            parts.push(format!("[System]\n{}", sys));
        }
    }

    // Collect non-system messages; for a single user message just use it directly
    let user_messages: Vec<&crate::llm::types::Message> = request
        .messages
        .iter()
        .filter(|m| m.role != Role::System)
        .collect();

    if user_messages.len() == 1 && user_messages[0].role == Role::User {
        parts.push(user_messages[0].content.clone());
    } else {
        for msg in user_messages {
            let role_label = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => continue,
            };
            parts.push(format!("[{}]\n{}", role_label, msg.content));
        }
    }

    parts.join("\n\n")
}

/// Parse the JSON output from `claude -p --output-format json`.
/// Expected shape: {"type":"result","subtype":"success","result":"...","session_id":"...","cost_usd":0,...}
fn parse_claude_json(output: &str) -> Result<String, NxError> {
    // The output may contain multiple JSON lines; take the last result line
    let mut result_text = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            // Look for result type
            if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                if let Some(text) = json.get("result").and_then(|r| r.as_str()) {
                    result_text = Some(text.to_string());
                }
            }
        }
    }

    // If no structured result found, try parsing the entire output as one JSON object
    if result_text.is_none() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(text) = json.get("result").and_then(|r| r.as_str()) {
                result_text = Some(text.to_string());
            }
        }
    }

    result_text.ok_or_else(|| NxError::ProviderError {
        provider: "claude_cli".to_string(),
        message: format!(
            "Failed to parse claude output. Raw: {}",
            &output[..output.len().min(200)]
        ),
    })
}

#[async_trait]
impl LlmProvider for ClaudeCliProvider {
    fn name(&self) -> &str {
        "claude_cli"
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, NxError> {
        let binary = self
            .binary_path
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: "claude binary not found on PATH".to_string(),
            })?;

        let prompt = build_prompt(request);

        let output = tokio::process::Command::new(binary)
            .arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg(&prompt)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output();

        let output = tokio::time::timeout(std::time::Duration::from_secs(300), output)
            .await
            .map_err(|_| NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: "claude CLI timed out after 300 seconds".to_string(),
            })?
            .map_err(|e| NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: format!("Failed to spawn claude: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: format!("claude exited with {}: {}", output.status, stderr),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let content = parse_claude_json(&stdout)?;

        Ok(LlmResponse {
            content,
            model: "claude-cli".to_string(),
            usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: Some("end_turn".to_string()),
            content_blocks: None,
            tool_calls: None,
            stop_reason: Some("end_turn".to_string()),
        })
    }

    async fn stream(
        &self,
        request: &LlmRequest,
        tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<(), NxError> {
        let binary = self
            .binary_path
            .as_ref()
            .ok_or_else(|| NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: "claude binary not found on PATH".to_string(),
            })?;

        let prompt = build_prompt(request);

        let mut child = tokio::process::Command::new(binary)
            .arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg(&prompt)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| NxError::ProviderError {
                provider: "claude_cli".to_string(),
                message: format!("Failed to spawn claude: {}", e),
            })?;

        let stdout = child.stdout.take().ok_or_else(|| NxError::ProviderError {
            provider: "claude_cli".to_string(),
            message: "Failed to capture claude stdout".to_string(),
        })?;

        let mut reader = BufReader::new(stdout).lines();

        // Read streamed JSON lines from claude
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                match json.get("type").and_then(|t| t.as_str()) {
                    Some("assistant") => {
                        // Content block with text
                        if let Some(text) = json.get("content").and_then(|c| c.as_str()) {
                            let _ = tx.send(StreamChunk::Delta(text.to_string()));
                        }
                    }
                    Some("result") => {
                        // Final result — extract any remaining text
                        if let Some(text) = json.get("result").and_then(|r| r.as_str()) {
                            let _ = tx.send(StreamChunk::Delta(text.to_string()));
                        }
                    }
                    _ => {
                        // Other event types (content_block_delta, etc.) — try to extract text
                        if let Some(text) = json.get("content").and_then(|c| c.as_str()) {
                            let _ = tx.send(StreamChunk::Delta(text.to_string()));
                        }
                    }
                }
            }
        }

        // Wait for process to finish
        let _ = child.wait().await;

        let _ = tx.send(StreamChunk::Done);
        Ok(())
    }

    fn available_models(&self) -> Vec<&str> {
        vec!["claude-cli"]
    }

    fn is_configured(&self) -> bool {
        self.binary_path.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_single_message() {
        let request = LlmRequest {
            messages: vec![crate::llm::types::Message {
                role: Role::User,
                content: "Hello".to_string(),
            }],
            model: String::new(),
            max_tokens: 4096,
            temperature: None,
            stream: false,
            system: None,
            tools: None,
        };
        assert_eq!(build_prompt(&request), "Hello");
    }

    #[test]
    fn test_build_prompt_with_system() {
        let request = LlmRequest {
            messages: vec![crate::llm::types::Message {
                role: Role::User,
                content: "Hello".to_string(),
            }],
            model: String::new(),
            max_tokens: 4096,
            temperature: None,
            stream: false,
            system: Some("You are helpful.".to_string()),
            tools: None,
        };
        let prompt = build_prompt(&request);
        assert!(prompt.contains("[System]"));
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("Hello"));
    }

    #[test]
    fn test_build_prompt_multi_turn() {
        let request = LlmRequest {
            messages: vec![
                crate::llm::types::Message {
                    role: Role::User,
                    content: "Hi".to_string(),
                },
                crate::llm::types::Message {
                    role: Role::Assistant,
                    content: "Hello!".to_string(),
                },
                crate::llm::types::Message {
                    role: Role::User,
                    content: "How are you?".to_string(),
                },
            ],
            model: String::new(),
            max_tokens: 4096,
            temperature: None,
            stream: false,
            system: None,
            tools: None,
        };
        let prompt = build_prompt(&request);
        assert!(prompt.contains("[User]\nHi"));
        assert!(prompt.contains("[Assistant]\nHello!"));
        assert!(prompt.contains("[User]\nHow are you?"));
    }

    #[test]
    fn test_parse_claude_json_success() {
        let output = r#"{"type":"result","subtype":"success","result":"Hello, world!","session_id":"abc123","cost_usd":0}"#;
        let result = parse_claude_json(output).unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_parse_claude_json_multiline() {
        let output = r#"{"type":"system","message":"starting"}
{"type":"result","subtype":"success","result":"The answer is 42.","session_id":"abc","cost_usd":0}"#;
        let result = parse_claude_json(output).unwrap();
        assert_eq!(result, "The answer is 42.");
    }

    #[test]
    fn test_parse_claude_json_failure() {
        let output = "not json at all";
        assert!(parse_claude_json(output).is_err());
    }

    #[test]
    fn test_is_configured_without_binary() {
        let provider = ClaudeCliProvider {
            binary_path: None,
            working_dir: PathBuf::from("."),
        };
        assert!(!provider.is_configured());
    }

    #[test]
    fn test_is_configured_with_binary() {
        let provider = ClaudeCliProvider {
            binary_path: Some("/usr/bin/claude".to_string()),
            working_dir: PathBuf::from("."),
        };
        assert!(provider.is_configured());
    }

    #[test]
    fn test_provider_name() {
        let provider = ClaudeCliProvider {
            binary_path: None,
            working_dir: PathBuf::from("."),
        };
        assert_eq!(provider.name(), "claude_cli");
    }

    #[test]
    fn test_available_models() {
        let provider = ClaudeCliProvider {
            binary_path: None,
            working_dir: PathBuf::from("."),
        };
        assert_eq!(provider.available_models(), vec!["claude-cli"]);
    }
}
