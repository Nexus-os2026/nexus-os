//! Claude Code CLI adapter — closed in Phase Zero (P0-002C5A).
//!
//! This adapter used to run the local `claude` binary, located through PATH,
//! the user's home directory or `npm config get prefix` resolved in the
//! process working directory, as a complete coding agent in that working
//! directory with its permission checks bypassed. An external CLI agent acts
//! outside every Nexus authority boundary, so Phase Zero neither detects,
//! launches nor logs into it: detection reports it unavailable without
//! running anything, and every query, stream and login fails closed.

use super::{LlmProvider, LlmResponse};
use crate::streaming::{StreamingLlmProvider, StreamingResponse};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};

/// Why every entry point of this adapter is unavailable.
pub const CLAUDE_CODE_UNAVAILABLE: &str =
    "the Claude Code CLI provider is unavailable in Phase Zero: an external CLI agent runs outside Nexus authority";

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

/// Reports the CLI as unavailable without running any process, so no router
/// or fallback ever selects it.
pub fn detect_claude_code() -> ClaudeCodeStatus {
    ClaudeCodeStatus {
        installed: false,
        version: None,
        authenticated: false,
        binary_path: None,
    }
}

fn unavailable() -> AgentError {
    AgentError::SupervisorError(CLAUDE_CODE_UNAVAILABLE.to_string())
}

/// LLM provider for the Claude Code CLI. Every request fails closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeCodeProvider;

impl ClaudeCodeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl LlmProvider for ClaudeCodeProvider {
    fn query(
        &self,
        _prompt: &str,
        _max_tokens: u32,
        _model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Err(unavailable())
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
        _prompt: &str,
        _system_prompt: &str,
        _max_tokens: u32,
        _model: &str,
    ) -> Result<StreamingResponse, AgentError> {
        Err(unavailable())
    }

    fn streaming_provider_name(&self) -> &str {
        "claude-code"
    }
}

/// Claude Code login is not started in Phase Zero.
pub fn trigger_login() -> Result<String, AgentError> {
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_unavailable(error: AgentError) -> bool {
        error.to_string().contains(CLAUDE_CODE_UNAVAILABLE)
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
    fn p0_002c5a_detection_reports_the_cli_unavailable() {
        assert_eq!(
            detect_claude_code(),
            ClaudeCodeStatus {
                installed: false,
                version: None,
                authenticated: false,
                binary_path: None,
            }
        );
    }

    #[test]
    fn p0_002c5a_every_request_and_login_fails_closed() {
        let provider = ClaudeCodeProvider::new();
        assert!(is_unavailable(
            provider.query("write code", 64, "").unwrap_err()
        ));
        assert!(is_unavailable(
            provider
                .query("write code", 64, CLAUDE_CODE_DEFAULT_MODEL)
                .unwrap_err()
        ));
        assert!(is_unavailable(
            provider
                .stream_query("write code", "system", 64, CLAUDE_CODE_DEFAULT_MODEL)
                .err()
                .unwrap()
        ));
        assert!(is_unavailable(trigger_login().unwrap_err()));
    }

    #[test]
    fn p0_002c5a_adapter_runs_no_process_and_bypasses_no_permission() {
        let source = include_str!("claude_code.rs");
        for forbidden in [
            concat!("Command", "::new"),
            concat!("std::", "process"),
            concat!("dangerously", "-skip"),
        ] {
            assert!(!source.contains(forbidden), "{forbidden}");
        }
    }
}
