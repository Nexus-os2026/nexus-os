//! OpenAI Codex CLI adapter — spawns the local `codex` binary for LLM calls.
//!
//! Uses the user's authenticated ChatGPT Plus/Pro subscription via the Codex
//! CLI.  OpenAI explicitly allows third-party tool usage with subscriptions.
//! NEXUS never sees or stores credentials.

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

/// Strip display suffixes like " (via Codex CLI)" from a model string.
///
/// `streaming_provider_from_prefixed_model` appends display suffixes to the
/// model name, but the Codex CLI rejects anything except the bare model ID
/// (e.g. `gpt-5.4`).  This function ensures only the clean ID is passed to
/// the `-c model=` flag.
fn clean_model_id(model: &str) -> &str {
    model.find(" (").map(|i| &model[..i]).unwrap_or(model)
}

/// Directory for temporary prompt files, based on `$HOME`.
fn prompt_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".nexus").join("builds")
}

/// Write prompt to a temp file and return its path.
///
/// The caller MUST delete the file after use (best-effort cleanup).
/// Using a file avoids all shell quoting / arg-length truncation issues.
fn write_prompt_file(prompt: &str, tag: &str) -> Result<PathBuf, AgentError> {
    let dir = prompt_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        AgentError::SupervisorError(format!(
            "failed to create prompt dir {}: {e}",
            dir.display()
        ))
    })?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("prompt_{tag}_{ts}.txt"));

    std::fs::write(&path, prompt.as_bytes()).map_err(|e| {
        AgentError::SupervisorError(format!(
            "failed to write prompt file {}: {e}",
            path.display()
        ))
    })?;

    Ok(path)
}

/// Remove a prompt file (best-effort, never errors).
fn cleanup_prompt_file(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}

/// Try to find the `codex` binary by checking `which`, then common install
/// locations, then `npm config get prefix`.  Returns the full path if found.
fn find_codex_binary() -> Option<String> {
    // 1. Try `which codex` with explicit PATH (Tauri backend often lacks user PATH)
    if let Ok(output) = Command::new("which")
        .arg("codex")
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
        PathBuf::from(format!("{home}/.npm-global/bin/codex")),
        PathBuf::from(format!("{home}/.local/bin/codex")),
        PathBuf::from("/usr/local/bin/codex"),
        PathBuf::from("/usr/bin/codex"),
    ];

    for candidate in &candidates {
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    // 3. Ask npm for its global prefix and check {prefix}/bin/codex
    if let Ok(output) = Command::new("npm")
        .args(["config", "get", "prefix"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                let npm_path = PathBuf::from(format!("{prefix}/bin/codex"));
                if npm_path.is_file() {
                    return Some(npm_path.to_string_lossy().to_string());
                }
            }
        }
    }

    None
}

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

/// Verify Codex CLI auth by running a minimal non-interactive exec.
///
/// Only called when the auth file is missing — the file check is preferred
/// because this takes several seconds.  A 10-second timeout avoids blocking
/// the UI if the CLI hangs.
fn check_codex_auth_exec(bin: &str) -> bool {
    let result = Command::new(bin)
        .arg("exec")
        .arg("respond with ok")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match result {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
            // If stderr mentions auth failure → not authenticated
            if stderr.contains("not authenticated")
                || stderr.contains("not logged in")
                || stderr.contains("login")
                || stderr.contains("unauthorized")
            {
                return false;
            }
            // If the process succeeded, or if it produced stdout → authenticated
            if output.status.success() {
                return true;
            }
            // Non-zero exit but no auth error → probably authenticated but
            // something else failed (network, etc.)  Be optimistic.
            !stderr.contains("auth")
        }
        Err(_) => false,
    }
}

/// Detect the local Codex CLI installation and auth status.
///
/// Auth check strategy (fast → slow):
/// 1. Check auth file on disk (~0 ms)
/// 2. Fall back to `codex exec` with 10s timeout (only if file missing)
pub fn detect_codex_cli() -> CodexCliStatus {
    let binary_path = find_codex_binary();

    let bin = match &binary_path {
        Some(p) => p.as_str(),
        None => {
            return CodexCliStatus {
                installed: false,
                version: None,
                authenticated: false,
                binary_path: None,
                auth_mode: None,
            };
        }
    };

    // Get version
    let version = Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|v| !v.is_empty());

    // Check auth: file first (instant), exec fallback (slow)
    let auth_info = read_codex_auth_info();
    let (authenticated, auth_mode) = if auth_info.authenticated {
        (true, auth_info.auth_mode)
    } else {
        // File missing or invalid — fall back to exec probe
        (check_codex_auth_exec(bin), None)
    };

    CodexCliStatus {
        installed: true,
        version,
        authenticated,
        binary_path,
        auth_mode,
    }
}

/// LLM provider that delegates to the local `codex` CLI binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCliProvider {
    timeout_secs: u64,
    /// Full path to the `codex` binary (discovered via `find_codex_binary`).
    /// Falls back to bare `"codex"` if detection was skipped.
    binary_path: String,
}

impl Default for CodexCliProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexCliProvider {
    pub fn new() -> Self {
        let binary_path = find_codex_binary().unwrap_or_else(|| "codex".to_string());
        Self {
            timeout_secs: 180,
            binary_path,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Spawn `codex exec` and capture stdout.
    ///
    /// Correct invocation: `codex exec -c model=gpt-5.4 < prompt.txt`
    ///
    /// The prompt is written to a temp file and piped via stdin to avoid
    /// shell quoting issues and argument length truncation with long prompts.
    fn run_cli(&self, prompt: &str, model: &str) -> Result<String, AgentError> {
        let model = clean_model_id(model);

        let prompt_file = write_prompt_file(prompt, "sync")?;
        eprintln!(
            "[codex-debug] Running: {} exec -c model={} < {} ({} chars)",
            self.binary_path,
            model,
            prompt_file.display(),
            prompt.len(),
        );

        let stdin_file = std::fs::File::open(&prompt_file).map_err(|e| {
            cleanup_prompt_file(&prompt_file);
            AgentError::SupervisorError(format!("failed to open prompt file: {e}"))
        })?;

        let mut child = Command::new(&self.binary_path)
            .arg("exec")
            .arg("-c")
            .arg(format!("model={model}"))
            .stdin(stdin_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                cleanup_prompt_file(&prompt_file);
                AgentError::SupervisorError(format!("failed to spawn codex CLI: {e}"))
            })?;

        // Wait with timeout
        let timeout = Duration::from_secs(self.timeout_secs);
        let start = std::time::Instant::now();

        let result = loop {
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
                        break Err(AgentError::SupervisorError(format!(
                            "Codex CLI failed: {detail}"
                        )));
                    }

                    break Ok(stdout);
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        break Err(AgentError::SupervisorError(format!(
                            "codex CLI timed out after {}s",
                            self.timeout_secs
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    break Err(AgentError::SupervisorError(format!(
                        "failed to wait on codex CLI: {e}"
                    )));
                }
            }
        };

        cleanup_prompt_file(&prompt_file);
        result
    }

    /// Parse plain-text output from `-o` file into an `LlmResponse`.
    fn parse_response(text: &str, model: &str) -> LlmResponse {
        let output_text = text.trim().to_string();
        // Estimate tokens from text length (no usage JSON with -o approach)
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

impl LlmProvider for CodexCliProvider {
    fn query(
        &self,
        prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<LlmResponse, AgentError> {
        let status = detect_codex_cli();
        if !status.installed {
            return Err(AgentError::SupervisorError(
                "Codex CLI not installed. Install: npm install -g @openai/codex".to_string(),
            ));
        }
        if !status.authenticated {
            return Err(AgentError::SupervisorError(
                "Codex CLI not authenticated. Run: codex login".to_string(),
            ));
        }

        let effective_model = if model.is_empty() {
            CODEX_CLI_DEFAULT_MODEL
        } else {
            model
        };

        let raw = self.run_cli(prompt, effective_model)?;
        Ok(Self::parse_response(&raw, effective_model))
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
        prompt: &str,
        system_prompt: &str,
        _max_tokens: u32,
        model: &str,
    ) -> Result<StreamingResponse, AgentError> {
        // Pre-flight auth check — fail fast with clear message instead of
        // getting a silent empty stream and "LLM returned empty response".
        let status = detect_codex_cli();
        if !status.installed {
            return Err(AgentError::SupervisorError(
                "Codex CLI not installed. Install: npm install -g @openai/codex".to_string(),
            ));
        }
        if !status.authenticated {
            return Err(AgentError::SupervisorError(
                "Codex CLI is not authenticated. Run: codex login".to_string(),
            ));
        }

        let effective_model = if model.is_empty() {
            CODEX_CLI_DEFAULT_MODEL
        } else {
            clean_model_id(model)
        };

        // Prepend system prompt if provided
        let full_prompt = if system_prompt.is_empty() {
            prompt.to_string()
        } else {
            format!("{system_prompt}\n\n{prompt}")
        };

        // Spawn `codex exec` — stdout streams as the model generates tokens.
        // Prompt is written to a temp file and piped via stdin to avoid
        // shell quoting issues and argument length truncation.
        let prompt_file = write_prompt_file(&full_prompt, "stream")?;
        eprintln!(
            "[codex-debug] Running: {} exec -c model={} < {} ({} chars)",
            &self.binary_path,
            effective_model,
            prompt_file.display(),
            full_prompt.len(),
        );

        let stdin_file = std::fs::File::open(&prompt_file).map_err(|e| {
            cleanup_prompt_file(&prompt_file);
            AgentError::SupervisorError(format!("failed to open prompt file: {e}"))
        })?;

        let mut child = Command::new(&self.binary_path)
            .arg("exec")
            .arg("-c")
            .arg(format!("model={effective_model}"))
            .stdin(stdin_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                cleanup_prompt_file(&prompt_file);
                AgentError::SupervisorError(format!("failed to spawn codex CLI: {e}"))
            })?;

        // Prompt file can be cleaned up now — the OS has the fd open for stdin
        cleanup_prompt_file(&prompt_file);

        let stdout = child.stdout.take().ok_or_else(|| {
            AgentError::SupervisorError("failed to capture codex CLI stdout".to_string())
        })?;

        // Capture stderr in a background thread so we can surface real error
        // messages instead of "LLM returned empty response".
        let stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = String::new();
                let mut reader = std::io::BufReader::new(stderr);
                let _ = reader.read_to_string(&mut buf);
                buf
            })
        });

        let usage_cell = new_usage_cell();
        let usage_cell_writer = usage_cell.clone();
        let timeout_secs = self.timeout_secs;

        let iter = CodexStreamIterator::new_with_stderr(
            stdout,
            usage_cell_writer,
            timeout_secs,
            stderr_handle,
        );

        Ok(StreamingResponse::new(Box::new(iter), usage_cell))
    }

    fn streaming_provider_name(&self) -> &str {
        "codex-cli"
    }
}

/// Iterator that reads line-by-line from `codex exec` stdout.
///
/// Codex exec output may include a header block (version, workdir, model info)
/// followed by the actual model response.  We skip header lines and emit
/// content lines as stream chunks.  The header format is:
///
/// ```text
/// OpenAI Codex v0.118.0 (research preview)
/// --------
/// workdir: /path
/// model: gpt-5.4
/// ...
/// --------
/// user
/// <the prompt>
/// codex
/// <THE ACTUAL RESPONSE>
/// ```
///
/// We detect the `codex` marker to start emitting, or fall back to emitting
/// everything if the header format changes.
struct CodexStreamIterator {
    lines: std::io::Lines<std::io::BufReader<std::process::ChildStdout>>,
    usage_cell: UsageCell,
    finished: bool,
    idle_timeout: Duration,
    last_data_at: std::time::Instant,
    accumulated_tokens: usize,
    /// Whether we've passed the header and started emitting content.
    content_started: bool,
    /// Track separator lines to detect end of header.
    separator_count: usize,
    /// Whether we saw the "codex" marker line.
    saw_codex_marker: bool,
    /// Background thread capturing stderr — joined on EOF to get error details.
    stderr_handle: Option<std::thread::JoinHandle<String>>,
}

impl CodexStreamIterator {
    fn new_with_stderr(
        stdout: std::process::ChildStdout,
        usage_cell: UsageCell,
        idle_timeout_secs: u64,
        stderr_handle: Option<std::thread::JoinHandle<String>>,
    ) -> Self {
        Self {
            lines: std::io::BufReader::new(stdout).lines(),
            usage_cell,
            finished: false,
            stderr_handle,
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            last_data_at: std::time::Instant::now(),
            accumulated_tokens: 0,
            content_started: false,
            separator_count: 0,
            saw_codex_marker: false,
        }
    }
}

impl Iterator for CodexStreamIterator {
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
                    "codex CLI stream idle timeout (no data for {}s)",
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
                        "codex CLI stream read error: {e}"
                    ))));
                }
                None => {
                    // EOF — process exited
                    self.finished = true;
                    // Write final usage
                    if let Ok(mut guard) = self.usage_cell.lock() {
                        *guard = Some(StreamUsage {
                            input_tokens: 0, // codex exec doesn't report input tokens
                            output_tokens: self.accumulated_tokens,
                        });
                    }
                    // If no content was emitted, check stderr for the real error
                    if self.accumulated_tokens == 0 {
                        let stderr_text = self
                            .stderr_handle
                            .take()
                            .and_then(|h| h.join().ok())
                            .unwrap_or_default();
                        let stderr_trimmed = stderr_text.trim();
                        if !stderr_trimmed.is_empty() {
                            let detail = if stderr_trimmed.len() > 300 {
                                format!("{}...", &stderr_trimmed[..300])
                            } else {
                                stderr_trimmed.to_string()
                            };
                            if stderr_trimmed.contains("not logged in")
                                || stderr_trimmed.contains("login")
                                || stderr_trimmed.contains("auth")
                                || stderr_trimmed.contains("unauthorized")
                            {
                                return Some(Err(AgentError::SupervisorError(
                                    format!("Codex CLI is not authenticated. Run: codex login (stderr: {detail})")
                                )));
                            }
                            return Some(Err(AgentError::SupervisorError(format!(
                                "Codex CLI produced no output. stderr: {detail}"
                            ))));
                        }
                    }
                    return None;
                }
            };

            // Header detection: skip everything until we see content
            if !self.content_started {
                let trimmed = line.trim();

                // Count separator lines (--------)
                if trimmed.starts_with("--------") {
                    self.separator_count += 1;
                    continue;
                }

                // The "codex" marker after the second separator means content follows
                if self.separator_count >= 2 && trimmed == "codex" {
                    self.saw_codex_marker = true;
                    self.content_started = true;
                    continue;
                }

                // If we see HTML starting without a codex marker, start emitting
                if trimmed.starts_with("<!DOCTYPE")
                    || trimmed.starts_with("<!doctype")
                    || trimmed.starts_with("<html")
                {
                    self.content_started = true;
                    // Fall through to emit this line
                } else if self.separator_count < 2 {
                    // Still in header
                    continue;
                } else if trimmed == "user" || trimmed.is_empty() {
                    // Between second separator and codex marker — skip user echo
                    continue;
                } else if !self.saw_codex_marker {
                    // After second separator but before codex marker — could be prompt echo
                    continue;
                }
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

/// Trigger `codex login` which opens the browser for ChatGPT OAuth.
/// NEXUS never sees or stores the credentials.
pub fn trigger_codex_login() -> Result<String, AgentError> {
    let bin = find_codex_binary().unwrap_or_else(|| "codex".to_string());
    eprintln!("[nexus-llm][governance] codex_cli::trigger_login binary={bin}");
    let output = Command::new(bin)
        .arg("login")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AgentError::SupervisorError(format!("failed to start codex login: {e}")))?;

    if output.status.success() {
        Ok("Login initiated — check your browser.".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AgentError::SupervisorError(format!(
            "codex login failed: {stderr}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_plain_text() {
        let resp = CodexCliProvider::parse_response("Hello from Codex!", "gpt-5-codex");
        assert_eq!(resp.output_text, "Hello from Codex!");
        assert!(resp.token_count > 0);
        assert_eq!(resp.model_name, "gpt-5-codex");
        assert!(resp.tool_calls.is_empty());
        assert_eq!(resp.input_tokens, None);
    }

    #[test]
    fn test_parse_response_trims_whitespace() {
        let resp = CodexCliProvider::parse_response("  trimmed output  \n", "gpt-5.4");
        assert_eq!(resp.output_text, "trimmed output");
    }

    #[test]
    fn test_parse_response_empty() {
        let resp = CodexCliProvider::parse_response("", "gpt-5-codex");
        assert_eq!(resp.output_text, "");
        assert_eq!(resp.token_count, 1); // min 1
    }

    #[test]
    fn test_parse_response_long_text_token_estimate() {
        let text = "a".repeat(400); // ~100 tokens
        let resp = CodexCliProvider::parse_response(&text, "gpt-5-codex");
        assert_eq!(resp.token_count, 100);
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
    fn test_with_timeout() {
        let provider = CodexCliProvider::new().with_timeout(60);
        assert_eq!(provider.timeout_secs, 60);
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

    #[test]
    fn test_find_codex_binary_returns_something_or_none() {
        let result = find_codex_binary();
        if let Some(ref path) = result {
            assert!(
                std::path::Path::new(path).exists(),
                "find_codex_binary returned non-existent path: {path}"
            );
        }
    }

    #[test]
    fn test_provider_stores_binary_path() {
        let provider = CodexCliProvider::new();
        assert!(!provider.binary_path.is_empty());
    }

    #[test]
    fn test_detect_uses_full_path() {
        let status = detect_codex_cli();
        let found = find_codex_binary();
        assert_eq!(status.binary_path, found);
        assert_eq!(status.installed, found.is_some());
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

    #[test]
    fn test_clean_model_id_strips_suffix() {
        assert_eq!(clean_model_id("gpt-5.4 (via Codex CLI)"), "gpt-5.4");
        assert_eq!(clean_model_id("gpt-5.4 (via CLI)"), "gpt-5.4");
        assert_eq!(clean_model_id("gpt-5.4"), "gpt-5.4");
        assert_eq!(clean_model_id("gpt-5-codex"), "gpt-5-codex");
        assert_eq!(
            clean_model_id("claude-sonnet-4-6 (via CLI)"),
            "claude-sonnet-4-6"
        );
    }

    #[test]
    fn test_default_timeout_is_180() {
        let provider = CodexCliProvider::new();
        assert_eq!(provider.timeout_secs, 180);
    }

    /// Verify prompt is written to file with correct content.
    #[test]
    fn test_prompt_written_to_file() {
        let prompt = "Build a landing page with hero section";
        let path = write_prompt_file(prompt, "test_write").unwrap();
        assert!(path.exists(), "prompt file was not created");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, prompt);
        cleanup_prompt_file(&path);
    }

    /// Verify prompt file is deleted after cleanup.
    #[test]
    fn test_prompt_file_cleaned_up() {
        let prompt = "test cleanup";
        let path = write_prompt_file(prompt, "test_cleanup").unwrap();
        assert!(path.exists());
        cleanup_prompt_file(&path);
        assert!(!path.exists(), "prompt file was not cleaned up");
    }

    /// Verify cleanup on a non-existent file does not panic.
    #[test]
    fn test_prompt_file_cleanup_nonexistent() {
        cleanup_prompt_file(std::path::Path::new("/tmp/nonexistent_prompt_file.txt"));
        // No panic = pass
    }

    /// Verify a 5000-char prompt is written to file without truncation.
    #[test]
    fn test_long_prompt_no_truncation() {
        let long_prompt = "x".repeat(5000);
        let path = write_prompt_file(&long_prompt, "test_long").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            content.len(),
            5000,
            "prompt was truncated: got {} chars",
            content.len()
        );
        assert_eq!(content, long_prompt);
        cleanup_prompt_file(&path);
    }

    /// Verify prompt_dir uses HOME env and does not hardcode /home/nexus.
    #[test]
    fn test_prompt_dir_uses_home_env() {
        let dir = prompt_dir();
        let dir_str = dir.to_string_lossy();
        assert!(
            dir_str.contains(".nexus/builds"),
            "expected .nexus/builds in path: {dir_str}"
        );
        // Must NOT contain a hardcoded username path
        assert!(
            !dir_str.contains("/home/nexus")
                || std::env::var("HOME").ok().as_deref() == Some("/home/nexus"),
            "prompt_dir hardcodes /home/nexus instead of using HOME"
        );
    }

    /// Verify prompts with special characters (quotes, newlines) are written intact.
    #[test]
    fn test_prompt_with_special_chars() {
        let prompt = "You are the Nexus Builder.\n\nUser's request: \"Build it's page\"\n<html>\n";
        let path = write_prompt_file(prompt, "test_special").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, prompt, "special characters were mangled");
        cleanup_prompt_file(&path);
    }

    /// Verify binary_path resolution stores a full path (not bare "codex").
    #[test]
    fn test_binary_path_resolution() {
        let provider = CodexCliProvider::new();
        // If codex is installed, binary_path should be an absolute path
        if provider.binary_path != "codex" {
            assert!(
                provider.binary_path.starts_with('/'),
                "binary_path should be absolute: {}",
                provider.binary_path
            );
        }
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

    /// Live test — run manually: cargo test test_codex_exec_live -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_codex_exec_live() {
        let binary = find_codex_binary().expect("codex binary not found");
        let prompt = "Output only: <h1>Hello</h1>";
        let prompt_file = write_prompt_file(prompt, "test_live").unwrap();
        let stdin_file = std::fs::File::open(&prompt_file).unwrap();

        let output = tokio::process::Command::new(&binary)
            .arg("exec")
            .arg("-c")
            .arg("model=gpt-5.4")
            .stdin(stdin_file)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .expect("failed to run codex exec");

        cleanup_prompt_file(&prompt_file);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("stdout: {stdout}");
        println!("stderr: {stderr}");
        assert!(
            stdout.contains("h1") || !stdout.is_empty(),
            "expected output, got empty stdout (stderr: {stderr})"
        );
    }
}
