//! Sandboxed code execution actuator — runs Python, Node.js, or Bash code
//! in an isolated subprocess with no network access and restricted filesystem.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use std::process::Command;
use std::time::Duration;

/// Maximum execution timeout in seconds.
const MAX_TIMEOUT_SECS: u32 = 30;
/// Default timeout if not specified.
const DEFAULT_TIMEOUT_SECS: u32 = 10;
/// Maximum code size: 256 KB.
const MAX_CODE_SIZE: usize = 256 * 1024;
/// Maximum output size: 100 KB.
const MAX_OUTPUT_SIZE: usize = 100 * 1024;

/// Fuel cost per code execution.
const FUEL_COST_CODE: f64 = 8.0;

/// Allowed language runtimes.
const ALLOWED_LANGUAGES: &[&str] = &["python3", "python", "node", "bash"];

/// Sandboxed code execution actuator. Runs code in a subprocess with:
/// - Working directory set to agent's sandbox
/// - Timeout enforcement
/// - Output size limits
/// - No network access (via environment neutering)
#[derive(Debug, Clone)]
pub struct CodeExecuteActuator;

impl CodeExecuteActuator {
    /// Validate the language runtime is allowed.
    fn validate_language(language: &str) -> Result<&'static str, ActuatorError> {
        let lower = language.to_lowercase();
        match lower.as_str() {
            "python3" | "python" => Ok("python3"),
            "node" | "nodejs" | "javascript" | "js" => Ok("node"),
            "bash" | "sh" => Ok("bash"),
            _ => Err(ActuatorError::CommandBlocked(format!(
                "language '{language}' not allowed; use one of: {}",
                ALLOWED_LANGUAGES.join(", ")
            ))),
        }
    }

    /// Check code size.
    fn check_code_size(code: &str) -> Result<(), ActuatorError> {
        if code.len() > MAX_CODE_SIZE {
            return Err(ActuatorError::BodyTooLarge {
                size: code.len() as u64,
                max: MAX_CODE_SIZE as u64,
            });
        }
        Ok(())
    }

    /// Scan code for dangerous patterns that should be blocked.
    fn check_dangerous_patterns(code: &str, language: &str) -> Result<(), ActuatorError> {
        let lower = code.to_lowercase();

        // Block network access attempts
        let network_patterns = [
            "import socket",
            "import urllib",
            "import requests",
            "import http",
            "require('http')",
            "require('https')",
            "require('net')",
            "require('dgram')",
            "fetch(",
            "xmlhttprequest",
            "curl ",
            "wget ",
            "nc ",
            "ncat ",
        ];
        for pattern in &network_patterns {
            if lower.contains(pattern) {
                return Err(ActuatorError::CommandBlocked(format!(
                    "network access not allowed in sandboxed code: found '{pattern}'"
                )));
            }
        }

        // Block shell escape attempts in Python/Node
        if language != "bash" {
            let escape_patterns = [
                "os.system(",
                "subprocess.",
                "os.popen(",
                "child_process",
                "execSync(",
                "spawnSync(",
            ];
            for pattern in &escape_patterns {
                if lower.contains(pattern) {
                    return Err(ActuatorError::CommandBlocked(format!(
                        "shell escape not allowed in sandboxed code: found '{pattern}'"
                    )));
                }
            }
        }

        // Block filesystem escape in bash
        if language == "bash" {
            let fs_escape = ["rm -rf /", "chmod 777", "dd if=", "mkfs.", "mount "];
            for pattern in &fs_escape {
                if lower.contains(pattern) {
                    return Err(ActuatorError::CommandBlocked(format!(
                        "dangerous command not allowed: found '{pattern}'"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Write code to a temporary file and execute it.
    fn execute_code(
        runtime: &str,
        code: &str,
        working_dir: &std::path::Path,
        timeout: Duration,
    ) -> Result<(bool, String), ActuatorError> {
        // Write code to temp file in agent workspace
        let extension = match runtime {
            "python3" => "py",
            "node" => "js",
            "bash" => "sh",
            _ => "txt",
        };
        let temp_path = working_dir.join(format!("_nexus_exec.{extension}"));

        // Ensure workspace exists
        if !working_dir.exists() {
            std::fs::create_dir_all(working_dir)
                .map_err(|e| ActuatorError::IoError(format!("create workspace: {e}")))?;
        }

        std::fs::write(&temp_path, code)
            .map_err(|e| ActuatorError::IoError(format!("write temp code: {e}")))?;

        // Build command with sandboxing environment
        // Resolve runtime to a full path. Tauri's process may have a minimal PATH
        // that doesn't include /usr/bin, so Command::new("bash") fails.
        let runtime_path = match runtime {
            "bash" => "/bin/bash",
            "sh" => "/bin/sh",
            "python3" => "/usr/bin/python3",
            "node" => "/usr/bin/node",
            other => other,
        };

        let mut cmd = Command::new(runtime_path);
        cmd.arg(&temp_path)
            .current_dir(working_dir)
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            // Neuter network access via environment
            .env("http_proxy", "http://0.0.0.0:0")
            .env("https_proxy", "http://0.0.0.0:0")
            .env("HTTP_PROXY", "http://0.0.0.0:0")
            .env("HTTPS_PROXY", "http://0.0.0.0:0")
            .env("no_proxy", "")
            .env("NO_PROXY", "")
            // Restrict HOME to agent workspace
            .env("HOME", working_dir)
            .env("TMPDIR", working_dir);

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ActuatorError::IoError(format!("spawn {runtime}: {e}")))?;

        // Wait with timeout
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Process exited
                    let stdout = child
                        .stdout
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            // Optional: partial stdout read still produces usable output; pipe may be broken after process exit
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();
                    let stderr = child
                        .stderr
                        .take()
                        .map(|mut s| {
                            let mut buf = Vec::new();
                            // Optional: partial stderr read still produces usable diagnostics; pipe may be broken after process exit
                            std::io::Read::read_to_end(&mut s, &mut buf).ok();
                            buf
                        })
                        .unwrap_or_default();

                    // Best-effort: temp file cleanup is housekeeping; execution result is already captured
                    let _ = std::fs::remove_file(&temp_path);

                    let mut output = String::from_utf8_lossy(&stdout).to_string();
                    let err_str = String::from_utf8_lossy(&stderr);
                    if !err_str.trim().is_empty() {
                        if !output.is_empty() {
                            output.push('\n');
                        }
                        output.push_str("[stderr] ");
                        output.push_str(&err_str);
                    }

                    // Truncate output
                    if output.len() > MAX_OUTPUT_SIZE {
                        output.truncate(MAX_OUTPUT_SIZE);
                        output.push_str("\n... [output truncated]");
                    }

                    return Ok((status.success(), output));
                }
                Ok(None) => {
                    // Still running — check timeout
                    if start.elapsed() > timeout {
                        // Best-effort: kill signal may fail if process already exited; timeout error is returned regardless
                        let _ = child.kill();
                        // Best-effort: reaping the child avoids zombies but timeout error takes precedence
                        let _ = child.wait();
                        // Best-effort: temp file cleanup after timeout; OS will reclaim on reboot if removal fails
                        let _ = std::fs::remove_file(&temp_path);
                        return Err(ActuatorError::CommandTimeout {
                            seconds: timeout.as_secs(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    // Best-effort: temp file cleanup on error; the wait error itself is returned
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(ActuatorError::IoError(format!("wait: {e}")));
                }
            }
        }
    }
}

impl Actuator for CodeExecuteActuator {
    fn name(&self) -> &str {
        "code_execute"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["process.exec".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (language, code, timeout_secs) = match action {
            PlannedAction::CodeExecute {
                language,
                code,
                timeout_secs,
            } => (language, code, timeout_secs),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "process.exec",
        ) {
            return Err(ActuatorError::CapabilityDenied("process.exec".into()));
        }

        // Validate language
        let runtime = Self::validate_language(language)?;

        // Check code size
        Self::check_code_size(code)?;

        // Check for dangerous patterns
        Self::check_dangerous_patterns(code, runtime)?;

        // Resolve timeout (capped at MAX_TIMEOUT_SECS)
        let timeout = Duration::from_secs(
            timeout_secs
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .min(MAX_TIMEOUT_SECS) as u64,
        );

        // Execute
        let (success, output) = Self::execute_code(runtime, code, &context.working_dir, timeout)?;

        Ok(ActionResult {
            success,
            output,
            fuel_cost: FUEL_COST_CODE,
            side_effects: vec![SideEffect::CommandExecuted {
                command: format!("{runtime} [inline code, {} bytes]", code.len()),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("process.exec".into());
        ActuatorContext {
            agent_id: "test-agent".into(),
            agent_name: "test-agent".into(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L3,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn validates_language() {
        assert!(CodeExecuteActuator::validate_language("python3").is_ok());
        assert!(CodeExecuteActuator::validate_language("Python").is_ok());
        assert!(CodeExecuteActuator::validate_language("node").is_ok());
        assert!(CodeExecuteActuator::validate_language("javascript").is_ok());
        assert!(CodeExecuteActuator::validate_language("bash").is_ok());
        assert!(CodeExecuteActuator::validate_language("ruby").is_err());
        assert!(CodeExecuteActuator::validate_language("perl").is_err());
    }

    #[test]
    fn blocks_network_access_patterns() {
        assert!(CodeExecuteActuator::check_dangerous_patterns("import socket", "python3").is_err());
        assert!(
            CodeExecuteActuator::check_dangerous_patterns("import requests", "python3").is_err()
        );
        assert!(CodeExecuteActuator::check_dangerous_patterns("require('http')", "node").is_err());
        assert!(CodeExecuteActuator::check_dangerous_patterns("fetch(url)", "node").is_err());
        assert!(CodeExecuteActuator::check_dangerous_patterns("print('hello')", "python3").is_ok());
        assert!(CodeExecuteActuator::check_dangerous_patterns("console.log('hi')", "node").is_ok());
    }

    #[test]
    fn blocks_shell_escape() {
        assert!(
            CodeExecuteActuator::check_dangerous_patterns("os.system('rm -rf /')", "python3")
                .is_err()
        );
        assert!(
            CodeExecuteActuator::check_dangerous_patterns("require('child_process')", "node")
                .is_err()
        );
    }

    #[test]
    fn blocks_oversized_code() {
        let big_code = "x".repeat(MAX_CODE_SIZE + 1);
        assert!(CodeExecuteActuator::check_code_size(&big_code).is_err());
        let ok_code = "x".repeat(MAX_CODE_SIZE);
        assert!(CodeExecuteActuator::check_code_size(&ok_code).is_ok());
    }

    #[test]
    fn capability_denied() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.capabilities.clear();
        let exec = CodeExecuteActuator;

        let action = PlannedAction::CodeExecute {
            language: "python3".into(),
            code: "print('hello')".into(),
            timeout_secs: None,
        };
        let err = exec.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }

    #[test]
    fn executes_python_code() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let exec = CodeExecuteActuator;

        let action = PlannedAction::CodeExecute {
            language: "python3".into(),
            code: "print('hello from sandbox')".into(),
            timeout_secs: Some(5),
        };
        let result = exec.execute(&action, &ctx);
        match result {
            Ok(r) => {
                assert!(r.success);
                assert!(r.output.contains("hello from sandbox"));
                assert_eq!(r.fuel_cost, FUEL_COST_CODE);
            }
            Err(ActuatorError::IoError(_)) => {
                // python3 not installed — acceptable in CI
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn executes_bash_code() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let exec = CodeExecuteActuator;

        let action = PlannedAction::CodeExecute {
            language: "bash".into(),
            code: "echo 'hello from bash'".into(),
            timeout_secs: Some(5),
        };
        let result = exec.execute(&action, &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.contains("hello from bash"));
    }

    #[test]
    fn invalid_language_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let exec = CodeExecuteActuator;

        let action = PlannedAction::CodeExecute {
            language: "ruby".into(),
            code: "puts 'hello'".into(),
            timeout_secs: None,
        };
        let err = exec.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }
}
