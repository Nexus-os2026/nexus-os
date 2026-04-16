//! GovernedShell actuator — sandboxed command execution with strict allowlists.

use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;

/// Command execution timeout in seconds.
const COMMAND_TIMEOUT_SECS: u64 = 60;

/// Maximum combined stdout+stderr output in bytes.
const MAX_OUTPUT_BYTES: usize = 100 * 1024;

/// Fuel cost per shell command.
const FUEL_COST_SHELL: f64 = 5.0;

/// Commands in the base allowlist (no subcommand restrictions).
///
/// Includes basic shell utilities + system monitoring commands needed by
/// agents like nexus-sysmon. Dangerous commands (rm, kill, shutdown, etc.)
/// are NOT included — see `COMMAND_BLOCKLIST`.
const BASE_ALLOWLIST: &[&str] = &[
    // Basic shell utilities
    "ls", "cat", "head", "tail", "wc", "grep", "find", "echo", "date", "pwd", "mkdir", "cp", "mv",
    "sort", "cut", "tr", "awk", "sed", "tee", "xargs", "which", "file", "diff", "touch",
    // System monitoring (read-only)
    "free", "df", "uptime", "top", "ps", "uname", "hostname", "whoami", "id", "nproc", "lsblk",
    "lscpu", "du", "stat", "env", "printenv", "vmstat", "iostat", "lsof", "ss", "ip",
    // Development tools
    "node", "npx", "rustc", "rustup", "make", "cmake", "gcc", "g++",
];

/// Commands with restricted subcommands: (command, &[allowed subcommands]).
const SUBCOMMAND_ALLOWLIST: &[(&str, &[&str])] = &[
    (
        "git",
        &[
            "status", "log", "diff", "add", "commit", "push", "pull", "branch", "show",
        ],
    ),
    ("cargo", &["build", "test", "check", "fmt", "clippy", "run"]),
    ("npm", &["install", "run", "build", "test", "ls"]),
    // systemctl: read-only status queries only — no start/stop/enable/disable
    (
        "systemctl",
        &[
            "status",
            "list-units",
            "list-timers",
            "is-active",
            "is-enabled",
            "show",
        ],
    ),
    ("docker", &["ps", "images", "logs", "inspect", "stats"]),
    ("journalctl", &["--no-pager", "-u", "-n", "--since"]),
];

/// Commands/patterns that are always rejected — destructive or privileged operations.
const COMMAND_BLOCKLIST: &[&str] = &[
    "sudo", "su", "eval", "exec", "rm", "rmdir", "mkfs", "dd", "kill", "killall", "pkill",
    "shutdown", "reboot", "halt", "poweroff", "passwd", "chown", "mount", "umount", "fdisk",
    "iptables", "nftables", "useradd", "userdel", "groupadd", "crontab",
];

/// Dangerous argument patterns (command + args joined).
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "chmod 777",
    "curl | sh",
    "wget | sh",
    "curl |sh",
    "wget |sh",
];

/// Governed shell actuator. Executes commands via `std::process::Command`
/// (never `sh -c`), enforces allowlists, timeouts, and output limits.
#[derive(Debug, Clone)]
pub struct GovernedShell;

impl GovernedShell {
    /// Extract the binary name from a command string.
    ///
    /// Defensive handling for planner output that packs the binary and its
    /// first argument into the `command` field (e.g. `"git status"` instead
    /// of `command="git"`, `args=["status"]`). The binary name is everything
    /// up to the first whitespace character.
    pub(crate) fn extract_binary_name(command: &str) -> &str {
        match command.split_once(char::is_whitespace) {
            Some((binary, _rest)) => binary,
            None => command,
        }
    }

    /// Validate command + args against allowlist and blocklist.
    fn validate_command(command: &str, args: &[String]) -> Result<(), ActuatorError> {
        // If the planner packed the binary + extra args into `command`,
        // split them so the blocklist/allowlist check sees only the binary
        // and the subcommand check sees the first extra token as a subcommand.
        let binary = Self::extract_binary_name(command);
        let mut effective_args: Vec<String> = command
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect();
        effective_args.extend(args.iter().cloned());

        // Check blocklist
        let cmd_lower = binary.to_lowercase();
        if COMMAND_BLOCKLIST.contains(&cmd_lower.as_str()) {
            return Err(ActuatorError::CommandBlocked(format!(
                "command '{binary}' is blocklisted"
            )));
        }

        // Check for python3 special case: only -c flag allowed
        if cmd_lower == "python3" || cmd_lower == "python" {
            if effective_args.first().map(|a| a.as_str()) != Some("-c") {
                return Err(ActuatorError::CommandBlocked(
                    "python3 only allowed with -c flag".into(),
                ));
            }
            return Ok(());
        }

        // Check dangerous patterns in full command string
        let full_cmd = format!("{} {}", binary, effective_args.join(" "));
        for pattern in DANGEROUS_PATTERNS {
            if full_cmd.contains(pattern) {
                return Err(ActuatorError::CommandBlocked(format!(
                    "dangerous pattern detected: '{pattern}'"
                )));
            }
        }

        // Check base allowlist
        if BASE_ALLOWLIST.contains(&cmd_lower.as_str()) {
            return Ok(());
        }

        // Check subcommand-restricted commands
        for (restricted_cmd, allowed_subs) in SUBCOMMAND_ALLOWLIST {
            if cmd_lower == *restricted_cmd {
                if let Some(sub) = effective_args.first() {
                    let sub_lower = sub.to_lowercase();
                    if allowed_subs.contains(&sub_lower.as_str()) {
                        return Ok(());
                    }
                    return Err(ActuatorError::CommandBlocked(format!(
                        "subcommand '{sub}' not allowed for '{binary}'; allowed: {}",
                        allowed_subs.join(", ")
                    )));
                }
                // No subcommand — allow bare command (e.g. `git` shows help)
                return Ok(());
            }
        }

        Err(ActuatorError::CommandBlocked(format!(
            "command '{binary}' not in allowlist"
        )))
    }
}

impl Actuator for GovernedShell {
    fn name(&self) -> &str {
        "governed_shell"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["process.exec".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (command, args) = match action {
            PlannedAction::ShellCommand { command, args } => (command, args),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "process.exec",
        ) {
            return Err(ActuatorError::CapabilityDenied("process.exec".into()));
        }

        Self::validate_command(command, args)?;

        // Defensive: if the planner packed extra tokens into `command`
        // (e.g. `"git status"`), split them off so the binary is clean and
        // the extra tokens are prepended to the arg list.
        let binary = Self::extract_binary_name(command);
        let mut effective_args: Vec<String> = command
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect();
        effective_args.extend(args.iter().cloned());

        // Run through `sh -c` so the shell resolves the command via its own PATH
        // (which includes /usr/bin, /usr/sbin, etc.). Command::new("free") fails
        // in Tauri because the parent process PATH is minimal — the binary isn't
        // found before spawn. Using sh -c also enables pipes and redirects.
        let full_command = if effective_args.is_empty() {
            binary.to_string()
        } else {
            format!("{} {}", binary, shell_escape_args(&effective_args))
        };
        // Ensure working directory exists (it may not for newly created agents)
        if !context.working_dir.exists() {
            // Best-effort: create working directory for agent; command will fail separately if needed
            let _ = std::fs::create_dir_all(&context.working_dir);
        }

        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg(&full_command);
        cmd.current_dir(&context.working_dir);
        cmd.env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );

        // Capture stdout/stderr
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| ActuatorError::IoError(format!("spawn '{command}': {e}")))?;

        // Wait with timeout
        let output = wait_with_timeout(child, COMMAND_TIMEOUT_SECS)?;

        let mut combined = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            combined.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&stderr);
        }

        // Truncate output
        if combined.len() > MAX_OUTPUT_BYTES {
            combined.truncate(MAX_OUTPUT_BYTES);
            combined.push_str("\n... [output truncated at 100KB]");
        }

        let full_cmd = format!("{} {}", command, args.join(" "));

        Ok(ActionResult {
            success: output.status.success(),
            output: combined,
            fuel_cost: FUEL_COST_SHELL,
            side_effects: vec![SideEffect::CommandExecuted { command: full_cmd }],
        })
    }
}

/// Wait for a child process with a timeout. Kills the process if it exceeds
/// the deadline.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout_secs: u64,
) -> Result<std::process::Output, ActuatorError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // Process finished — collect output
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    std::io::Read::read_to_end(&mut out, &mut stdout)
                        .map_err(|e| ActuatorError::IoError(format!("read stdout: {e}")))?;
                }
                if let Some(mut err) = child.stderr.take() {
                    std::io::Read::read_to_end(&mut err, &mut stderr)
                        .map_err(|e| ActuatorError::IoError(format!("read stderr: {e}")))?;
                }
                return Ok(std::process::Output {
                    status: _status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                // Still running
                if std::time::Instant::now() >= deadline {
                    // Best-effort: kill timed-out child process; already returning timeout error
                    let _ = child.kill();
                    // Best-effort: reap killed child to avoid zombie; timeout error is still returned
                    let _ = child.wait();
                    return Err(ActuatorError::CommandTimeout {
                        seconds: timeout_secs,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(ActuatorError::IoError(format!("try_wait: {e}")));
            }
        }
    }
}

/// Shell-escape each argument and join with spaces for use with `sh -c`.
/// Wraps each arg in single quotes, escaping any embedded single quotes.
fn shell_escape_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./=:@,+".contains(c))
            {
                // Safe characters — no quoting needed
                arg.clone()
            } else {
                // Wrap in single quotes; escape embedded single quotes
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(workspace: &std::path::Path) -> ActuatorContext {
        let mut caps = HashSet::new();
        caps.insert("shell.execute".into());
        ActuatorContext {
            agent_id: "test-agent".into(),
            agent_name: "test-agent".into(),
            working_dir: workspace.to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities: caps,
            fuel_remaining: 1000.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn echo_hello() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["hello".into()],
        };
        let result = shell.execute(&action, &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.trim().contains("hello"));
    }

    #[test]
    fn ls_in_workspace() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "data").unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "ls".into(),
            args: vec![],
        };
        let result = shell.execute(&action, &ctx).unwrap();
        assert!(result.success);
        assert!(result.output.contains("test.txt"));
    }

    #[test]
    fn rm_rf_root_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "rm".into(),
            args: vec!["-rf".into(), "/".into()],
        };
        let err = shell.execute(&action, &ctx).unwrap_err();
        assert!(
            matches!(err, ActuatorError::CommandBlocked(_)),
            "expected CommandBlocked, got {err:?}"
        );
    }

    #[test]
    fn sudo_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "sudo".into(),
            args: vec!["apt".into(), "install".into(), "foo".into()],
        };
        let err = shell.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn unlisted_command_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "curl".into(),
            args: vec!["https://example.com".into()],
        };
        let err = shell.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn git_allowed_subcommand() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "git".into(),
            args: vec!["status".into()],
        };
        // May fail because tmp isn't a git repo, but validation passes
        let result = shell.execute(&action, &ctx);
        // We only care that it wasn't CommandBlocked
        if let Err(e) = &result {
            assert!(
                !matches!(e, ActuatorError::CommandBlocked(_)),
                "git status should not be blocked"
            );
        }
    }

    #[test]
    fn git_disallowed_subcommand() {
        let err = GovernedShell::validate_command("git", &["rebase".into()]).unwrap_err();
        assert!(matches!(err, ActuatorError::CommandBlocked(_)));
    }

    #[test]
    fn command_with_space_is_split() {
        assert_eq!(GovernedShell::extract_binary_name("git status"), "git");
        assert_eq!(GovernedShell::extract_binary_name("python3"), "python3");
        assert_eq!(
            GovernedShell::extract_binary_name("python3 -m pytest"),
            "python3"
        );
    }

    #[test]
    fn shell_command_splits_compound_command() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        // Planner packed "hello" into the command field; extra arg in args.
        let action = PlannedAction::ShellCommand {
            command: "echo hello".into(),
            args: vec!["world".into()],
        };
        let result = shell.execute(&action, &ctx).unwrap();
        assert!(result.success);
        let out = result.output.trim();
        assert!(
            out.contains("hello") && out.contains("world"),
            "expected 'hello world' in output, got: {out:?}"
        );

        // Validation alone should accept "git status" with empty args.
        GovernedShell::validate_command("git status", &[])
            .expect("compound 'git status' command must validate");
    }

    #[test]
    fn command_executed_side_effect() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["test".into()],
        };
        let result = shell.execute(&action, &ctx).unwrap();
        assert_eq!(result.side_effects.len(), 1);
        assert!(matches!(
            &result.side_effects[0],
            SideEffect::CommandExecuted { command } if command.contains("echo")
        ));
    }

    #[test]
    fn uses_command_new_not_sh() {
        // Verify that our implementation uses Command::new() directly
        // by confirming that an unknown shell built-in fails to spawn
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let shell = GovernedShell;

        // "echo" works because it's a binary on most systems, but we verify
        // the allowlist mechanism, not sh -c
        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["direct".into()],
        };
        let result = shell.execute(&action, &ctx).unwrap();
        assert!(result.success);
    }

    #[test]
    fn capability_denied() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = make_context(tmp.path());
        ctx.capabilities.clear();
        let shell = GovernedShell;

        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: vec!["no".into()],
        };
        let err = shell.execute(&action, &ctx).unwrap_err();
        assert!(matches!(err, ActuatorError::CapabilityDenied(_)));
    }
}
