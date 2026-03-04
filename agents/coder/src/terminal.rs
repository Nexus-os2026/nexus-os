use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::autonomy::{AutonomyGuard, AutonomyLevel};
use nexus_kernel::consent::{
    ApprovalQueue, ApprovalRequest, ConsentError, ConsentPolicyEngine, ConsentRuntime,
    GovernedOperation,
};
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
                let _ = process.kill();
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
        self.audit_trail.append_event(
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
        );
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
        self.audit_trail.append_event(
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
        );
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

fn spawn_shell(command: &str, cwd: &Path) -> Result<std::process::Child, CommandError> {
    let mut shell = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };

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
    Ok(())
}

fn contains_shell_operator(command: &str) -> bool {
    command.contains(';')
        || command.contains("&&")
        || command.contains("||")
        || command.contains('|')
        || command.contains('`')
        || command.contains("$(")
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
