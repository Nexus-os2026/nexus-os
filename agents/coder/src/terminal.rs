//! # Terminal Module — Shell Command Execution (Migration In Progress)
//!
//! This module handles governed shell command execution for the coder agent.
//!
//! ## Security Hardening (A.1 Complete)
//! - Shell operator injection blocked: ;, &&, ||, |, backticks, $, newlines, redirects, globs, braces
//! - Argument-level validation for dangerous commands: python -c, git -c, npm run, node -e, pip
//! - 25 tests covering all validator paths
//!
//! ## Migration Path
//! Raw shell execution via spawn_shell is deprecated. The replacement is
//! `sdk::typed_tools::execute_typed_tool()` which provides typed tool interfaces
//! (e.g., `GitCommit { message }`, `CargoTest { package }`) that map to direct
//! `Command::new()` calls with no shell involvement.
//!
//! WASM agents already use the new path via the `nexus_exec_tool` host function.

use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use nexus_sdk::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_sdk::consent::{
    ApprovalQueue, ApprovalRequest, ConsentError, ConsentPolicyEngine, ConsentRuntime,
    GovernedOperation,
};
use nexus_sdk::resource_limiter::ResourceLimiter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const TERMINAL_EXECUTE_CAPABILITY: &str = "terminal.execute";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const ALLOWLIST: &[&str] = &[
    "cargo", "npm", "pip", "pip3", "git", "python", "python3", "node", "npx",
];
const BLOCKED_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "sudo ",
    "curl|sh",
    "curl | sh",
    "wget|sh",
    "wget | sh",
    "mkfs",
    "dd if=",
    ":(){:|:&};:",
    "git reset --hard",
    "git clean -fd",
    "git clean -fdx",
    "shutdown",
    "reboot",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChunk {
    pub stream: OutputStream,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    CapabilityDenied(String),
    CommandBlocked(String),
    AutonomyDenied(String),
    ApprovalRequired(String),
    ConsentDenied(String),
    ExecutionFailed(String),
    Timeout(String),
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::CapabilityDenied(capability) => {
                write!(f, "capability denied: {capability}")
            }
            CommandError::CommandBlocked(reason) => write!(f, "command blocked: {reason}"),
            CommandError::AutonomyDenied(reason) => write!(f, "autonomy denied: {reason}"),
            CommandError::ApprovalRequired(request_id) => {
                write!(f, "approval required: request_id='{request_id}'")
            }
            CommandError::ConsentDenied(reason) => write!(f, "consent denied: {reason}"),
            CommandError::ExecutionFailed(reason) => write!(f, "execution failed: {reason}"),
            CommandError::Timeout(reason) => write!(f, "command timed out: {reason}"),
        }
    }
}

impl std::error::Error for CommandError {}

#[derive(Debug)]
pub struct TerminalExecutor {
    capabilities: HashSet<String>,
    audit_trail: AuditTrail,
    agent_id: Uuid,
    autonomy_guard: AutonomyGuard,
    consent_runtime: ConsentRuntime,
}

impl Default for TerminalExecutor {
    fn default() -> Self {
        Self::with_capabilities_and_autonomy(
            [TERMINAL_EXECUTE_CAPABILITY.to_string()]
                .into_iter()
                .collect(),
            AutonomyLevel::L0,
        )
    }
}

impl TerminalExecutor {
    pub fn with_capabilities(capabilities: HashSet<String>) -> Self {
        Self::with_capabilities_and_autonomy(capabilities, AutonomyLevel::L0)
    }

    pub fn with_capabilities_and_autonomy(
        capabilities: HashSet<String>,
        level: AutonomyLevel,
    ) -> Self {
        Self::with_capabilities_autonomy_and_consent(
            capabilities,
            level,
            ConsentRuntime::new(
                ConsentPolicyEngine::default(),
                ApprovalQueue::in_memory(),
                "terminal.executor".to_string(),
            ),
        )
    }

    pub fn with_capabilities_autonomy_and_consent(
        capabilities: HashSet<String>,
        level: AutonomyLevel,
        consent_runtime: ConsentRuntime,
    ) -> Self {
        Self {
            capabilities,
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
            autonomy_guard: AutonomyGuard::new(level),
            consent_runtime,
        }
    }

    pub fn execute(
        &mut self,
        command: &str,
        working_dir: impl AsRef<Path>,
        timeout: Option<Duration>,
    ) -> Result<CommandResult, CommandError> {
        self.execute_with_stream(command, working_dir, timeout, |_| {})
    }

    pub fn execute_with_stream<F>(
        &mut self,
        command: &str,
        working_dir: impl AsRef<Path>,
        timeout: Option<Duration>,
        mut on_chunk: F,
    ) -> Result<CommandResult, CommandError>
    where
        F: FnMut(OutputChunk),
    {
        if let Err(error) = self
            .autonomy_guard
            .require_tool_call(self.agent_id, &mut self.audit_trail)
        {
            let denied = CommandError::AutonomyDenied(error.to_string());
            self.log_error(command, working_dir.as_ref(), &denied, "", "", 0);
            return Err(denied);
        }

        if let Err(error) = self.consent_runtime.enforce_operation(
            GovernedOperation::TerminalCommand,
            self.agent_id,
            command.as_bytes(),
            &mut self.audit_trail,
        ) {
            let denied = match error {
                ConsentError::ApprovalRequired { request_id, .. } => {
                    CommandError::ApprovalRequired(request_id)
                }
                other => CommandError::ConsentDenied(other.to_string()),
            };
            self.log_error(command, working_dir.as_ref(), &denied, "", "", 0);
            return Err(denied);
        }

        if !self.capabilities.contains(TERMINAL_EXECUTE_CAPABILITY) {
            let error = CommandError::CapabilityDenied(TERMINAL_EXECUTE_CAPABILITY.to_string());
            self.log_error(command, working_dir.as_ref(), &error, "", "", 0);
            return Err(error);
        }

        if let Err(error) = validate_command(command) {
            self.log_error(command, working_dir.as_ref(), &error, "", "", 0);
            return Err(error);
        }

        let cwd = working_dir.as_ref();
        let effective_timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);
        let start = Instant::now();

        let mut process = spawn_shell(command, cwd)?;
        let stdout = process.stdout.take().ok_or_else(|| {
            CommandError::ExecutionFailed("failed to capture stdout pipe".to_string())
        })?;
        let stderr = process.stderr.take().ok_or_else(|| {
            CommandError::ExecutionFailed("failed to capture stderr pipe".to_string())
        })?;

        let (sender, receiver) = mpsc::channel::<OutputChunk>();
        let stdout_handle = spawn_reader(stdout, OutputStream::Stdout, sender.clone());
        let stderr_handle = spawn_reader(stderr, OutputStream::Stderr, sender);

        let mut collected_stdout = String::new();
        let mut collected_stderr = String::new();
        let exit_status;
        let mut timed_out = false;

        loop {
            drain_output(
                &receiver,
                &mut collected_stdout,
                &mut collected_stderr,
                &mut on_chunk,
            );

            if let Some(status) = process.try_wait().map_err(|error| {
                CommandError::ExecutionFailed(format!("failed waiting for command: {error}"))
            })? {
                exit_status = status;
                break;
            }

            if start.elapsed() > effective_timeout {
                timed_out = true;
                // Kill the entire process group (child + all descendants) to
                // prevent orphaned grandchildren from surviving the timeout.
                // Best-effort: kill timed-out process tree to prevent orphan descendants
                let _ = ResourceLimiter::kill_process_tree(process.id());
                exit_status = process.wait().map_err(|error| {
                    CommandError::ExecutionFailed(format!("failed waiting after kill: {error}"))
                })?;
                break;
            }

            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(chunk) => consume_chunk(
                    chunk,
                    &mut collected_stdout,
                    &mut collected_stderr,
                    &mut on_chunk,
                ),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {}
            }
        }

        // Best-effort: join I/O reader threads, ignore panics
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();
        drain_output(
            &receiver,
            &mut collected_stdout,
            &mut collected_stderr,
            &mut on_chunk,
        );

        let duration_ms = start.elapsed().as_millis();
        if timed_out {
            let error = CommandError::Timeout(format!(
                "command '{command}' exceeded {} ms",
                effective_timeout.as_millis()
            ));
            self.log_error(
                command,
                cwd,
                &error,
                collected_stdout.as_str(),
                collected_stderr.as_str(),
                duration_ms,
            );
            return Err(error);
        }

        let result = CommandResult {
            exit_code: exit_status.code().unwrap_or(-1),
            stdout: collected_stdout,
            stderr: collected_stderr,
            duration_ms,
        };
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::ToolCall,
            json!({
                "tool": "terminal.execute",
                "command": command,
                "cwd": cwd.to_string_lossy().to_string(),
                "exit_code": result.exit_code,
                "duration_ms": result.duration_ms,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
        Ok(result)
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }

    pub fn pending_approvals(&self) -> Vec<ApprovalRequest> {
        self.consent_runtime.pending_requests()
    }

    pub fn approve_request(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), CommandError> {
        self.consent_runtime
            .approve(request_id, approver_id, &mut self.audit_trail)
            .map_err(|error| CommandError::ConsentDenied(error.to_string()))
    }

    pub fn deny_request(
        &mut self,
        request_id: &str,
        approver_id: &str,
    ) -> Result<(), CommandError> {
        self.consent_runtime
            .deny(request_id, approver_id, &mut self.audit_trail)
            .map_err(|error| CommandError::ConsentDenied(error.to_string()))
    }

    fn log_error(
        &mut self,
        command: &str,
        cwd: &Path,
        error: &CommandError,
        stdout: &str,
        stderr: &str,
        duration_ms: u128,
    ) {
        if let Err(e) = self.audit_trail.append_event(
            self.agent_id,
            EventType::Error,
            json!({
                "tool": "terminal.execute",
                "command": command,
                "cwd": cwd.to_string_lossy().to_string(),
                "error": error.to_string(),
                "duration_ms": duration_ms,
                "stdout": stdout,
                "stderr": stderr,
            }),
        ) {
            tracing::error!("Audit append failed: {e}");
        }
    }
}

pub fn execute(
    command: &str,
    working_dir: impl AsRef<Path>,
    timeout: Option<Duration>,
) -> Result<CommandResult, CommandError> {
    let mut executor = TerminalExecutor::default();
    executor.execute(command, working_dir, timeout)
}

/// DEPRECATED: This function passes raw command strings to `sh -lc` which is inherently unsafe.
/// New agent code should use `sdk::typed_tools::execute_typed_tool()` which provides typed,
/// shell-free command execution. This function will be removed in a future release.
/// See: `sdk/src/typed_tools.rs`
///
/// Resource limits (memory, CPU, process count, file size) are enforced via
/// rlimits set in the child process.  On timeout, the entire process group is
/// killed to prevent orphaned grandchildren.
fn spawn_shell(command: &str, cwd: &Path) -> Result<std::process::Child, CommandError> {
    eprintln!("DEPRECATED: spawn_shell called with raw command string. Migrate to sdk::typed_tools::execute_typed_tool for safe typed tool execution.");
    let mut shell = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };

    // Apply OS-level resource limits (RLIMIT_AS, RLIMIT_CPU, RLIMIT_NPROC,
    // RLIMIT_FSIZE) and put the child in its own process group via pre_exec.
    let limiter = ResourceLimiter::default();
    limiter.apply_to_command(&mut shell);

    shell
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            CommandError::ExecutionFailed(format!(
                "failed to spawn command '{command}' in '{}': {error}",
                cwd.display()
            ))
        })
}

fn validate_command(command: &str) -> Result<(), CommandError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(CommandError::CommandBlocked(
            "empty command is not allowed".to_string(),
        ));
    }

    if contains_shell_operator(trimmed) {
        return Err(CommandError::CommandBlocked(
            "shell control operators are not allowed".to_string(),
        ));
    }

    let lowered = trimmed.to_ascii_lowercase();
    for pattern in BLOCKED_PATTERNS {
        if lowered.contains(pattern) {
            return Err(CommandError::CommandBlocked(format!(
                "dangerous pattern '{pattern}' detected"
            )));
        }
    }

    let base = extract_command_base(trimmed).ok_or_else(|| {
        CommandError::CommandBlocked("unable to detect command executable".to_string())
    })?;
    if !ALLOWLIST.iter().any(|allowed| *allowed == base) {
        return Err(CommandError::CommandBlocked(format!(
            "command '{base}' is not in the safe allowlist"
        )));
    }
    validate_command_args(trimmed, &base)
}

/// Per-command argument restrictions for allowlisted executables.
///
/// Even though a command passes the allowlist, certain argument patterns let
/// the caller break out of the intended sandbox:
/// - `python3 -c "os.system(...)"` — arbitrary code execution
/// - `git -c core.sshCommand=...` — config injection
/// - `npm run <arbitrary>` — runs anything in package.json scripts
/// - `node -e "..."` — eval arbitrary JS
/// - `pip install <malicious-pkg>` — only safe subcommands allowed
fn validate_command_args(command: &str, base: &str) -> Result<(), CommandError> {
    let args: Vec<&str> = command.split_whitespace().skip(1).collect();
    let args_lower: Vec<String> = args.iter().map(|a| a.to_ascii_lowercase()).collect();

    match base {
        "python" | "python3" => {
            if args_lower.iter().any(|a| a == "-c") {
                return Err(CommandError::CommandBlocked(
                    "python -c is blocked: arbitrary code execution".to_string(),
                ));
            }
        }
        "git" => {
            // Block `git -c key=value` config injection
            for (i, arg) in args_lower.iter().enumerate() {
                if arg == "-c" {
                    if let Some(val) = args.get(i + 1) {
                        let val_lower = val.to_ascii_lowercase();
                        if val_lower.contains('=')
                            || val_lower.contains("sshcommand")
                            || val_lower.contains("core.editor")
                        {
                            return Err(CommandError::CommandBlocked(
                                "git -c config injection is blocked".to_string(),
                            ));
                        }
                    }
                }
            }
        }
        "npm" => {
            const SAFE_NPM: &[&[&str]] = &[
                &["install"],
                &["ci"],
                &["test"],
                &["run", "build"],
                &["run", "lint"],
                &["run", "dev"],
                &["run", "start"],
            ];
            if args.is_empty() {
                // bare `npm` is fine (prints help)
            } else {
                let matches_safe = SAFE_NPM.iter().any(|pattern| {
                    pattern.len() <= args_lower.len()
                        && pattern
                            .iter()
                            .zip(args_lower.iter())
                            .all(|(p, a)| *p == a.as_str())
                });
                if !matches_safe {
                    return Err(CommandError::CommandBlocked(format!(
                        "npm subcommand '{}' is not in the safe npm allowlist",
                        args_lower.first().unwrap_or(&String::new()),
                    )));
                }
            }
        }
        "npx" => {
            // npx can run arbitrary packages; allow through for now
            // (npx is already on the allowlist and is needed for tooling)
        }
        "node" => {
            if args_lower.iter().any(|a| a == "-e" || a == "--eval") {
                return Err(CommandError::CommandBlocked(
                    "node -e/--eval is blocked: arbitrary code execution".to_string(),
                ));
            }
        }
        "pip" | "pip3" => {
            const SAFE_PIP: &[&str] = &["install", "list", "show", "freeze"];
            if let Some(sub) = args_lower.first() {
                if !SAFE_PIP.contains(&sub.as_str()) {
                    return Err(CommandError::CommandBlocked(format!(
                        "pip subcommand '{sub}' is not in the safe pip allowlist",
                    )));
                }
            }
        }
        "cargo" => {
            // cargo is generally safe within the project context
        }
        _ => {}
    }
    Ok(())
}

/// Check for shell operators and injection vectors.
///
/// Blocked: `;` `&&` `||` `|` `` ` `` `$` (covers `$()`, `$VAR`, `${IFS}`)
/// `\n` `\r` (newline injection) `>` `<` (redirection)
/// `*` `?` (glob expansion) `{` `}` (brace expansion)
fn contains_shell_operator(command: &str) -> bool {
    command.contains(';')
        || command.contains("&&")
        || command.contains("||")
        || command.contains('|')
        || command.contains('`')
        || command.contains('$')
        || command.contains('\n')
        || command.contains('\r')
        || command.contains('>')
        || command.contains('<')
        || command.contains('*')
        || command.contains('?')
        || command.contains('{')
        || command.contains('}')
}

fn extract_command_base(command: &str) -> Option<String> {
    let token = command.split_whitespace().next()?;
    let executable = PathBuf::from(token);
    let base = executable
        .file_name()?
        .to_string_lossy()
        .to_ascii_lowercase();
    Some(base)
}

fn spawn_reader<R: Read + Send + 'static>(
    mut reader: R,
    stream: OutputStream,
    sender: Sender<OutputChunk>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 2048];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_bytes) => {
                    let text = String::from_utf8_lossy(&buffer[..read_bytes]).to_string();
                    if sender.send(OutputChunk { stream, text }).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn drain_output<F>(
    receiver: &Receiver<OutputChunk>,
    stdout: &mut String,
    stderr: &mut String,
    on_chunk: &mut F,
) where
    F: FnMut(OutputChunk),
{
    while let Ok(chunk) = receiver.try_recv() {
        consume_chunk(chunk, stdout, stderr, on_chunk);
    }
}

fn consume_chunk<F>(chunk: OutputChunk, stdout: &mut String, stderr: &mut String, on_chunk: &mut F)
where
    F: FnMut(OutputChunk),
{
    match chunk.stream {
        OutputStream::Stdout => stdout.push_str(chunk.text.as_str()),
        OutputStream::Stderr => stderr.push_str(chunk.text.as_str()),
    }
    on_chunk(chunk);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── contains_shell_operator ──────────────────────────────────────

    #[test]
    fn test_contains_shell_operator_blocks_semicolon() {
        assert!(contains_shell_operator("cargo test; rm -rf /"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_pipes() {
        assert!(contains_shell_operator("cargo test | grep ok"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_and_or() {
        assert!(contains_shell_operator("true && false"));
        assert!(contains_shell_operator("true || false"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_backticks() {
        assert!(contains_shell_operator("cargo `whoami`"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_dollar_paren() {
        assert!(contains_shell_operator("cargo $(whoami)"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_newline() {
        assert!(contains_shell_operator("cargo test\nrm -rf /"));
        assert!(contains_shell_operator("cargo test\rrm -rf /"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_redirects() {
        assert!(contains_shell_operator("cargo test > out.txt"));
        assert!(contains_shell_operator("cargo test >> out.txt"));
        assert!(contains_shell_operator("python3 < input.py"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_env_vars() {
        assert!(contains_shell_operator("cargo $HOME"));
        assert!(contains_shell_operator("cargo ${IFS}"));
        assert!(contains_shell_operator("$PATH"));
    }

    #[test]
    fn test_contains_shell_operator_blocks_globs() {
        assert!(contains_shell_operator("python3 /tmp/*.py"));
        assert!(contains_shell_operator("node ?.js"));
    }

    #[test]
    fn test_contains_shell_operator_allows_clean_command() {
        assert!(!contains_shell_operator("cargo test --release"));
        assert!(!contains_shell_operator("git status"));
        assert!(!contains_shell_operator("npm install"));
    }

    // ── validate_command (end-to-end) ────────────────────────────────

    #[test]
    fn test_validate_command_rejects_empty() {
        assert!(matches!(
            validate_command(""),
            Err(CommandError::CommandBlocked(_))
        ));
        assert!(matches!(
            validate_command("   "),
            Err(CommandError::CommandBlocked(_))
        ));
    }

    #[test]
    fn test_validate_command_rejects_unknown_command() {
        let err = validate_command("curl http://example.com").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("not in the safe allowlist")),
            "expected allowlist rejection, got: {err}"
        );
    }

    #[test]
    fn test_validate_command_allows_cargo_test() {
        assert!(validate_command("cargo test").is_ok());
        assert!(validate_command("cargo test --release").is_ok());
        assert!(validate_command("cargo fmt --all").is_ok());
    }

    #[test]
    fn test_validate_command_allows_git_status() {
        assert!(validate_command("git status").is_ok());
        assert!(validate_command("git log --oneline").is_ok());
    }

    #[test]
    fn test_validate_command_rejects_python_dash_c() {
        let err = validate_command("python3 -c import os").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("python -c")),
            "expected python -c rejection, got: {err}"
        );
        // Also block `python -c`
        assert!(validate_command("python -c print('hi')").is_err());
    }

    #[test]
    fn test_validate_command_rejects_git_config_injection() {
        let err = validate_command("git -c core.sshCommand=evil fetch").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("config injection")),
            "expected git config injection rejection, got: {err}"
        );
    }

    #[test]
    fn test_validate_command_rejects_npm_arbitrary_script() {
        let err = validate_command("npm run malicious").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("npm")),
            "expected npm rejection, got: {err}"
        );
        assert!(validate_command("npm exec something").is_err());
    }

    #[test]
    fn test_validate_command_allows_npm_test() {
        assert!(validate_command("npm test").is_ok());
        assert!(validate_command("npm install").is_ok());
        assert!(validate_command("npm run build").is_ok());
        assert!(validate_command("npm run lint").is_ok());
        assert!(validate_command("npm run dev").is_ok());
        assert!(validate_command("npm run start").is_ok());
        assert!(validate_command("npm ci").is_ok());
    }

    #[test]
    fn test_validate_command_rejects_node_eval() {
        let err = validate_command("node -e process.exit").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("node -e")),
            "expected node eval rejection, got: {err}"
        );
        assert!(validate_command("node --eval process.exit").is_err());
    }

    #[test]
    fn test_validate_command_rejects_newline_injection() {
        let err = validate_command("cargo test\nrm -rf /").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("shell control operators")),
            "expected shell operator rejection, got: {err}"
        );
    }

    #[test]
    fn test_validate_command_rejects_redirect() {
        let err = validate_command("cargo test > /etc/crontab").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("shell control operators")),
            "expected shell operator rejection, got: {err}"
        );
    }

    #[test]
    fn test_blocklist_rejects_rm_rf() {
        // rm is not on the allowlist, but also check blocklist fires
        let err = validate_command("rm -rf /").unwrap_err();
        assert!(matches!(err, CommandError::CommandBlocked(_)));
    }

    #[test]
    fn test_blocklist_rejects_sudo() {
        let err = validate_command("sudo anything").unwrap_err();
        assert!(
            matches!(err, CommandError::CommandBlocked(ref msg) if msg.contains("dangerous pattern")),
            "expected blocklist rejection, got: {err}"
        );
    }

    // ── validate_command_args (unit) ─────────────────────────────────

    #[test]
    fn test_pip_only_safe_subcommands() {
        assert!(validate_command_args("pip install requests", "pip").is_ok());
        assert!(validate_command_args("pip3 list", "pip3").is_ok());
        assert!(validate_command_args("pip3 show flask", "pip3").is_ok());
        assert!(validate_command_args("pip freeze", "pip").is_ok());
        assert!(validate_command_args("pip download evil", "pip").is_err());
    }

    #[test]
    fn test_extract_command_base_strips_path() {
        assert_eq!(
            extract_command_base("/usr/bin/cargo test"),
            Some("cargo".to_string())
        );
        assert_eq!(
            extract_command_base("python3 script.py"),
            Some("python3".to_string())
        );
    }
}
