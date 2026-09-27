//! OpenAI Codex CLI adapter — closed in Phase Zero (P0-002C5A).
//!
//! This adapter used to run the local `codex` binary, located through PATH,
//! the user's home directory or `npm config get prefix` resolved in the
//! process working directory, as a coding agent session in that working
//! directory; its detection even started a real `codex exec` session to probe
//! authentication. An external CLI agent acts outside every Nexus authority
//! boundary, so Phase Zero neither detects, launches nor logs into it:
//! detection reports it unavailable without running anything, and every
//! query, stream and login fails closed. Reading the presence of the Codex
//! auth file and parsing Codex output remain available; neither runs anything.

use super::{LlmProvider, LlmResponse};
use crate::streaming::{StreamingLlmProvider, StreamingResponse};
use nexus_kernel::errors::AgentError;
use serde::{Deserialize, Serialize};

/// Why every agent entry point of this adapter is unavailable.
pub const CODEX_CLI_UNAVAILABLE: &str =
    "the Codex CLI provider is unavailable in Phase Zero: an external CLI agent runs outside Nexus authority";

/// Status of the locally installed Codex CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexCliStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
    pub binary_path: Option<String>,
    /// How the user authenticated: `"chatgpt"`, `"openai"`, `"apikey"`, or `None`.
    pub auth_mode: Option<String>,
}

/// Parsed auth info from `~/.codex/auth.json`.
#[derive(Debug, Clone)]
pub struct CodexAuthInfo {
    /// Whether `tokens.id_token` is present and non-empty.
    pub authenticated: bool,
    /// The `auth_mode` field from the JSON (e.g. `"chatgpt"`, `"openai"`, `"apikey"`).
    pub auth_mode: Option<String>,
}

/// Models available through the Codex CLI.
pub const CODEX_CLI_MODELS: &[(&str, &str)] = &[
    ("gpt-5-codex", "GPT-5 Codex"),
    ("gpt-5.4", "GPT-5.4"),
    ("gpt-5.3-codex", "GPT-5.3 Codex"),
];

/// Default model when none specified.
pub const CODEX_CLI_DEFAULT_MODEL: &str = "gpt-5.4";

/// Parse a `CodexAuthInfo` from the raw contents of an `auth.json` file.
///
/// File structure (confirmed):
/// ```json
/// {
///   "auth_mode": "chatgpt",
///   "tokens": { "id_token": "eyJ...", ... },
///   ...
/// }
/// ```
///
/// Auth is valid when `tokens.id_token` is a non-empty string.
pub fn parse_codex_auth_json(content: &str) -> CodexAuthInfo {
    let json: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => {
            return CodexAuthInfo {
                authenticated: false,
                auth_mode: None,
            };
        }
    };

    let authenticated = json["tokens"]["id_token"]
        .as_str()
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    let auth_mode = json["auth_mode"].as_str().map(|s| s.to_string());

    CodexAuthInfo {
        authenticated,
        auth_mode,
    }
}

/// Read and parse Codex CLI auth info from `~/.codex/auth.json`.
///
/// Returns full auth details including `auth_mode` for display.
/// Instant (<1 ms) — no CLI spawn needed.
pub fn read_codex_auth_info() -> CodexAuthInfo {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return CodexAuthInfo {
            authenticated: false,
            auth_mode: None,
        };
    }

    let path = format!("{home}/.codex/auth.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_codex_auth_json(&content),
        Err(_) => CodexAuthInfo {
            authenticated: false,
            auth_mode: None,
        },
    }
}

/// Quick boolean check: is Codex CLI authenticated via auth file?
///
/// Equivalent to `read_codex_auth_info().authenticated` but named for
/// call sites that only need a bool.
pub fn check_codex_auth_file() -> bool {
    read_codex_auth_info().authenticated
}

/// Reports the CLI as unavailable without running any process, so no router
/// or fallback ever selects it.
pub fn detect_codex_cli() -> CodexCliStatus {
    CodexCliStatus {
        installed: false,
        version: None,
        authenticated: false,
        binary_path: None,
        auth_mode: None,
    }
}

fn unavailable() -> AgentError {
    AgentError::SupervisorError(CODEX_CLI_UNAVAILABLE.to_string())
}

/// LLM provider for the Codex CLI. Every request fails closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexCliProvider;

impl CodexCliProvider {
    pub fn new() -> Self {
        Self
    }
}

impl LlmProvider for CodexCliProvider {
    fn query(
        &self,
        _prompt: &str,
        _max_tokens: u32,
        _model: &str,
    ) -> Result<LlmResponse, AgentError> {
        Err(unavailable())
    }

    fn name(&self) -> &str {
        "codex-cli"
    }

    fn cost_per_token(&self) -> f64 {
        // GPT-5 Codex estimated rates: ~$0.000012/token output
        0.000_012
    }

    fn endpoint_url(&self) -> String {
        "provider://codex-cli".to_string()
    }
}

impl StreamingLlmProvider for CodexCliProvider {
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
        "codex-cli"
    }
}

/// Extract HTML content from raw codex exec output (non-streaming fallback).
///
/// Strips the header block and "user"/"codex" markers, returning just the
/// model's response.  Used when the streaming path needs to parse buffered output.
pub fn extract_html_from_codex_output(raw: &str) -> Result<String, String> {
    // Strategy 1: Find the "codex\n" marker that precedes the actual response
    if let Some(idx) = raw.find("\ncodex\n") {
        let html = raw[idx + 7..].trim().to_string();
        if html.is_empty() {
            return Err("Codex output contained marker but no content".to_string());
        }
        return Ok(html);
    }

    // Strategy 2: Find HTML document start
    if let Some(idx) = raw.find("<!DOCTYPE") {
        return Ok(raw[idx..].trim().to_string());
    }
    if let Some(idx) = raw.find("<!doctype") {
        return Ok(raw[idx..].trim().to_string());
    }
    if let Some(idx) = raw.find("<html") {
        return Ok(raw[idx..].trim().to_string());
    }

    // Strategy 3: Return everything after the last separator block
    if let Some(idx) = raw.rfind("--------\n") {
        let after = raw[idx + 9..].trim().to_string();
        if !after.is_empty() {
            return Ok(after);
        }
    }

    Err("No HTML content found in Codex CLI output".to_string())
}

/// Codex login is not started in Phase Zero.
pub fn trigger_codex_login() -> Result<String, AgentError> {
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_unavailable(error: AgentError) -> bool {
        error.to_string().contains(CODEX_CLI_UNAVAILABLE)
    }

    #[test]
    fn test_provider_traits() {
        let provider = CodexCliProvider::new();
        assert_eq!(provider.name(), "codex-cli");
        assert!(provider.cost_per_token() > 0.0);
        assert!(provider.is_paid());
        assert_eq!(provider.endpoint_url(), "provider://codex-cli");
    }

    #[test]
    fn test_default_model() {
        assert_eq!(CODEX_CLI_DEFAULT_MODEL, "gpt-5.4");
    }

    #[test]
    fn test_models_list() {
        assert_eq!(CODEX_CLI_MODELS.len(), 3);
        assert!(CODEX_CLI_MODELS.iter().any(|(id, _)| *id == "gpt-5-codex"));
        assert!(CODEX_CLI_MODELS.iter().any(|(id, _)| *id == "gpt-5.4"));
        assert!(CODEX_CLI_MODELS
            .iter()
            .any(|(id, _)| *id == "gpt-5.3-codex"));
    }

    #[test]
    fn test_detect_status_struct() {
        let status = CodexCliStatus {
            installed: true,
            version: Some("1.0.0".to_string()),
            authenticated: true,
            binary_path: Some("/usr/local/bin/codex".to_string()),
            auth_mode: Some("chatgpt".to_string()),
        };
        assert!(status.installed);
        assert!(status.authenticated);
        assert_eq!(status.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_no_credential_storage() {
        let provider = CodexCliProvider::new();
        let debug_repr = format!("{provider:?}");
        assert!(!debug_repr.contains("key"));
        assert!(!debug_repr.contains("secret"));
        assert!(!debug_repr.contains("password"));
        assert!(!debug_repr.contains("cookie"));
    }

    #[test]
    fn test_embedding_not_supported() {
        let provider = CodexCliProvider::new();
        let result = provider.embed(&["test"], "gpt-5-codex");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not support embeddings"), "got: {err}");
    }

    // ── extract_html_from_codex_output tests ──

    #[test]
    fn test_extract_html_with_codex_marker() {
        let raw = "OpenAI Codex v0.118.0 (research preview)\n\
                    --------\n\
                    workdir: /tmp\n\
                    model: gpt-5.4\n\
                    --------\n\
                    user\n\
                    Build a landing page\n\
                    codex\n\
                    <!DOCTYPE html>\n<html><body><h1>Hello</h1></body></html>";
        let html = extract_html_from_codex_output(raw).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Hello</h1>"));
    }

    #[test]
    fn test_extract_html_no_marker_finds_doctype() {
        let raw = "Some preamble\n<!DOCTYPE html>\n<html><body>Test</body></html>";
        let html = extract_html_from_codex_output(raw).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn test_extract_html_finds_html_tag() {
        let raw = "Preamble text\n<html lang=\"en\"><body>Content</body></html>";
        let html = extract_html_from_codex_output(raw).unwrap();
        assert!(html.starts_with("<html"));
    }

    #[test]
    fn test_extract_html_empty_after_marker() {
        let raw = "--------\nuser\nBuild a site\ncodex\n   \n";
        let result = extract_html_from_codex_output(raw);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_html_no_html_at_all() {
        let raw = "just some random text with no html";
        let result = extract_html_from_codex_output(raw);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_html_separator_fallback() {
        let raw = "--------\nworkdir: /tmp\n--------\nHere is the generated content";
        let html = extract_html_from_codex_output(raw).unwrap();
        assert!(html.contains("generated content"));
    }

    // ── Streaming provider trait tests ──

    #[test]
    fn test_streaming_provider_name() {
        let provider = CodexCliProvider::new();
        assert_eq!(provider.streaming_provider_name(), "codex-cli");
    }

    // ── Auth file / JSON parsing tests ──

    /// Valid auth.json with tokens.id_token → authenticated, auth_mode extracted
    #[test]
    fn test_codex_auth_file_check_found() {
        let info = parse_codex_auth_json(
            r#"{"auth_mode": "chatgpt", "tokens": {"id_token": "eyJhbGciOiJSUzI1NiJ9.test"}}"#,
        );
        assert!(info.authenticated);
        assert_eq!(info.auth_mode.as_deref(), Some("chatgpt"));
    }

    /// auth.json with openai auth_mode
    #[test]
    fn test_codex_auth_file_openai_mode() {
        let info = parse_codex_auth_json(
            r#"{"auth_mode": "openai", "tokens": {"id_token": "sk-abc123"}}"#,
        );
        assert!(info.authenticated);
        assert_eq!(info.auth_mode.as_deref(), Some("openai"));
    }

    /// auth.json with apikey auth_mode
    #[test]
    fn test_codex_auth_file_apikey_mode() {
        let info = parse_codex_auth_json(
            r#"{"auth_mode": "apikey", "tokens": {"id_token": "sk-proj-test"}}"#,
        );
        assert!(info.authenticated);
        assert_eq!(info.auth_mode.as_deref(), Some("apikey"));
    }

    /// No auth file at all → check_codex_auth_file returns false, doesn't panic
    #[test]
    fn test_codex_auth_file_check_missing() {
        let _result = check_codex_auth_file();
        // No panic = pass
    }

    /// Empty file → not valid JSON → not authenticated
    #[test]
    fn test_codex_auth_file_check_empty() {
        let info = parse_codex_auth_json("");
        assert!(!info.authenticated);
        assert_eq!(info.auth_mode, None);
    }

    /// Valid JSON but no tokens.id_token → not authenticated
    #[test]
    fn test_codex_auth_file_check_no_id_token() {
        let info =
            parse_codex_auth_json(r#"{"auth_mode": "chatgpt", "tokens": {"access_token": "x"}}"#);
        assert!(!info.authenticated);
        assert_eq!(info.auth_mode.as_deref(), Some("chatgpt"));
    }

    /// tokens.id_token is empty string → not authenticated
    #[test]
    fn test_codex_auth_file_check_empty_id_token() {
        let info = parse_codex_auth_json(r#"{"auth_mode": "chatgpt", "tokens": {"id_token": ""}}"#);
        assert!(!info.authenticated);
    }

    /// tokens.id_token is null → not authenticated
    #[test]
    fn test_codex_auth_file_check_null_id_token() {
        let info =
            parse_codex_auth_json(r#"{"auth_mode": "chatgpt", "tokens": {"id_token": null}}"#);
        assert!(!info.authenticated);
    }

    /// No auth_mode field → authenticated but auth_mode is None
    #[test]
    fn test_codex_auth_file_no_auth_mode() {
        let info =
            parse_codex_auth_json(r#"{"tokens": {"id_token": "eyJhbGciOiJSUzI1NiJ9.test"}}"#);
        assert!(info.authenticated);
        assert_eq!(info.auth_mode, None);
    }

    /// Garbage / not JSON → not authenticated
    #[test]
    fn test_codex_auth_file_invalid_json() {
        let info = parse_codex_auth_json("this is not json at all {{{");
        assert!(!info.authenticated);
        assert_eq!(info.auth_mode, None);
    }

    // ── P0-002C5A: the external agent is unavailable ──

    #[test]
    fn p0_002c5a_detection_reports_the_cli_unavailable() {
        assert_eq!(
            detect_codex_cli(),
            CodexCliStatus {
                installed: false,
                version: None,
                authenticated: false,
                binary_path: None,
                auth_mode: None,
            }
        );
    }

    #[test]
    fn p0_002c5a_every_request_and_login_fails_closed() {
        let provider = CodexCliProvider::new();
        assert!(is_unavailable(
            provider.query("write code", 64, "").unwrap_err()
        ));
        assert!(is_unavailable(
            provider
                .query("write code", 64, CODEX_CLI_DEFAULT_MODEL)
                .unwrap_err()
        ));
        assert!(is_unavailable(
            provider
                .stream_query("write code", "system", 64, CODEX_CLI_DEFAULT_MODEL)
                .err()
                .unwrap()
        ));
        assert!(is_unavailable(trigger_codex_login().unwrap_err()));
    }

    #[test]
    fn p0_002c5a_adapter_runs_no_process() {
        let source = include_str!("codex_cli.rs");
        for forbidden in [concat!("Command", "::new"), concat!("std::", "process")] {
            assert!(!source.contains(forbidden), "{forbidden}");
        }
    }
}
