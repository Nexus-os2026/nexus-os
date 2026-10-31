//! GovernedShell actuator — allowlisted argv command execution (not an OS sandbox).

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
///
/// No entry may start a shell or an arbitrary program named in its arguments
/// or in files it reads from the working directory or HOME (M0-FIX2 F9).
/// Programs built for that (env, xargs, awk, sed, make, node, npx,
/// interpreters, compilers and build tools) and programs that run what their
/// config names (git, cargo, npm, docker) are absent; `find` and `sort` stay
/// only under the argument rules in `TRAMPOLINE_ARGUMENTS`.
const BASE_ALLOWLIST: &[&str] = &[
    // Basic shell utilities
    "ls", "cat", "head", "tail", "wc", "grep", "find", "echo", "date", "pwd", "mkdir", "cp", "mv",
    "sort", "cut", "tr", "tee", "which", "file", "diff", "touch",
    // System monitoring (read-only)
    "free", "df", "uptime", "top", "ps", "uname", "hostname", "whoami", "id", "nproc", "lsblk",
    "lscpu", "du", "stat", "printenv", "vmstat", "iostat", "lsof", "ss",
];

/// Commands with restricted subcommands: (command, &[allowed subcommands]).
///
/// git (hooks, config and attribute drivers), cargo (build scripts,
/// toolchain and cargo config), npm (package scripts, npmrc) and docker (CLI
/// plugin config) are absent: each runs programs named in files an allowed
/// writer can plant (M0-FIX2 F9).
const SUBCOMMAND_ALLOWLIST: &[(&str, &[&str])] = &[
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
    ("journalctl", &["--no-pager", "-u", "-n", "--since"]),
];

/// An argument that would make an allowlisted program start another program.
enum TrampolineArgument {
    /// Refused when the argument equals this token.
    Exact(&'static str),
    /// Refused when the argument starts with this text.
    Prefix(&'static str),
}

/// Per-program arguments refused anywhere in the argument list (M0-FIX2 F9).
const TRAMPOLINE_ARGUMENTS: &[(&str, &[TrampolineArgument])] = &[
    (
        "find",
        &[
            // Run a program.
            TrampolineArgument::Exact("-exec"),
            TrampolineArgument::Exact("-execdir"),
            TrampolineArgument::Exact("-ok"),
            TrampolineArgument::Exact("-okdir"),
            // Write or delete files.
            TrampolineArgument::Exact("-delete"),
            TrampolineArgument::Exact("-fprint"),
            TrampolineArgument::Exact("-fprint0"),
            TrampolineArgument::Exact("-fprintf"),
            TrampolineArgument::Exact("-fls"),
        ],
    ),
    // `--compress-program`; `--co` is already its only unambiguous abbreviation.
    ("sort", &[TrampolineArgument::Prefix("--co")]),
];

impl TrampolineArgument {
    fn matches(&self, arg: &str) -> bool {
        match self {
            Self::Exact(token) => arg == *token,
            Self::Prefix(prefix) => arg.starts_with(prefix),
        }
    }
}

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

        // Check dangerous patterns in full command string
        let full_cmd = format!("{} {}", binary, effective_args.join(" "));
        for pattern in DANGEROUS_PATTERNS {
            if full_cmd.contains(pattern) {
                return Err(ActuatorError::CommandBlocked(format!(
                    "dangerous pattern detected: '{pattern}'"
                )));
            }
        }

        // Refuse arguments that would start another program, whatever their
        // position (option parsers permute and accept abbreviations).
        for (program, refused) in TRAMPOLINE_ARGUMENTS {
            if cmd_lower == *program {
                if let Some(arg) = effective_args
                    .iter()
                    .find(|arg| refused.iter().any(|rule| rule.matches(arg)))
                {
                    return Err(ActuatorError::CommandBlocked(format!(
                        "argument '{arg}' would let '{binary}' start another program"
                    )));
                }
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

        // Ensure working directory exists (it may not for newly created agents)
        #[cfg(unix)]
        if !context.working_dir.exists() {
            // Best-effort: create working directory for agent; command will fail separately if needed
            let _ = std::fs::create_dir_all(&context.working_dir);
        }

        #[cfg(windows)]
        if let Some(output) = super::execution_platform::basic_command(
            binary,
            &effective_args,
            &context.working_dir,
            MAX_OUTPUT_BYTES,
        )? {
            return Ok(ActionResult {
                success: true,
                output,
                fuel_cost: FUEL_COST_SHELL,
                side_effects: vec![SideEffect::CommandExecuted {
                    command: format!("{} {}", command, args.join(" ")),
                }],
            });
        }
        // Argv only: the validated binary and each argument reach the program
        // as separate entries and no shell interprets them. The fixed PATH
        // set on the command (not the parent's minimal Tauri PATH) resolves
        // the bare binary name.
        #[cfg(unix)]
        let output = {
            let mut cmd = std::process::Command::new(binary);
            cmd.args(&effective_args).env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            );
            cmd.current_dir(&context.working_dir);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            let child = cmd
                .spawn()
                .map_err(|e| ActuatorError::IoError(format!("spawn '{command}': {e}")))?;
            wait_with_timeout(child, COMMAND_TIMEOUT_SECS)?
        };
        #[cfg(windows)]
        let output = super::execution_platform::execute(
            binary,
            &effective_args
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
            &context.working_dir,
            false,
            std::time::Duration::from_secs(COMMAND_TIMEOUT_SECS),
            MAX_OUTPUT_BYTES,
        )?;

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
        #[cfg(unix)]
        if combined.len() > MAX_OUTPUT_BYTES {
            combined.truncate(MAX_OUTPUT_BYTES);
            combined.push_str("\n... [output truncated at 100KB]");
        }

        #[cfg(windows)]
        let combined = super::execution_platform::bounded(combined, MAX_OUTPUT_BYTES);

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
#[cfg(unix)]
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

    /// M0-FIX2 F9 review MAJOR-1: formerly `git_allowed_subcommand`, which
    /// asserted that `git status` is not blocked. git runs programs named in
    /// repository config, attributes and hooks, so it is refused.
    #[test]
    fn git_status_is_refused() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let action = PlannedAction::ShellCommand {
            command: "git".into(),
            args: vec!["status".into()],
        };
        let err = GovernedShell.execute(&action, &ctx).unwrap_err();
        assert!(
            matches!(err, ActuatorError::CommandBlocked(_)),
            "git status must be refused, got {err:?}"
        );
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

        // Validation alone should accept a packed "ls -la" with empty args.
        // (M0-FIX2 F9: formerly "git status"; git is now refused.)
        GovernedShell::validate_command("ls -la", &[])
            .expect("compound 'ls -la' command must validate");
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
        // Every argument must reach the program as one inert argv entry. A
        // shell would expand these: a POSIX `sh` `echo` builtin turns `\n`
        // into a newline, and the other entries would create directories.
        let tmp = TempDir::new().unwrap();
        let ctx = make_context(tmp.path());
        let args: Vec<String> = vec![
            r"a\nb".into(),
            "$(mkdir substitution)".into(),
            "`mkdir backtick`".into(),
            "; mkdir semicolon".into(),
            "&& mkdir and".into(),
            "| mkdir pipe".into(),
            "> redirect".into(),
            "$HOME".into(),
        ];
        let action = PlannedAction::ShellCommand {
            command: "echo".into(),
            args: args.clone(),
        };
        let result = GovernedShell.execute(&action, &ctx).unwrap();
        assert!(result.success);
        assert_eq!(result.output, format!("{}\n", args.join(" ")));
        let created: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(created.is_empty(), "shell side effects: {created:?}");
    }

    #[test]
    fn governed_shell_has_no_shell_route() {
        let source = include_str!("shell.rs").replace("\r\n", "\n");
        let end = source.find("\n#[cfg(test)]\nmod tests").unwrap();
        let production = source[..end]
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "\"/bin/sh\"",
            "\"sh\"",
            "\"bash\"",
            ".arg(\"-c\")",
            "\"-lc\"",
            "\"/C\"",
            "cmd.exe",
            "powershell",
            "shell_escape",
        ] {
            assert!(!production.contains(forbidden), "{forbidden} in shell.rs");
        }
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

    fn assert_refused(command: &str, args: &[&str]) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        match GovernedShell::validate_command(command, &args) {
            Err(ActuatorError::CommandBlocked(_)) => {}
            other => panic!("{command} {args:?} must be refused, got {other:?}"),
        }
    }

    fn assert_validates(command: &str, args: &[&str]) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        GovernedShell::validate_command(command, &args)
            .unwrap_or_else(|e| panic!("{command} {args:?} must validate, got {e:?}"));
    }

    /// M0-FIX2 F9 oracle: no allowlisted program may start a shell or an
    /// arbitrary program named in its arguments. Validation only; nothing runs.
    #[test]
    fn m0_f9_program_trampolines_are_refused() {
        // Programs whose purpose or argument language starts other programs.
        assert_refused("env", &["sh"]);
        assert_refused("ENV", &["sh"]);
        assert_refused("xargs", &["sh"]);
        assert_refused("awk", &["BEGIN{system(\"sh\")}"]);
        assert_refused("sed", &["e"]);
        assert_refused("make", &[]);
        assert_refused("make", &["all"]);
        assert_refused("node", &["-e", "1"]);
        assert_refused("npx", &["anything"]);
        assert_refused("cmake", &["-P", "script.cmake"]);
        assert_refused("gcc", &["main.c"]);
        assert_refused("g++", &["main.cc"]);
        assert_refused("rustc", &["main.rs"]);
        assert_refused("rustup", &["run", "stable", "sh"]);
        assert_refused("ip", &["netns", "exec", "n", "sh"]);
        // Interpreters are not reachable through GovernedShell at all.
        assert_refused("python3", &["-c", "print(1)"]);
        assert_refused("python", &["-c", "print(1)"]);
        assert_refused("python3 -c", &["print(1)"]);
        assert_refused("python3", &["script.py"]);
        // find: the execution primaries are refused wherever they appear.
        for primary in ["-exec", "-execdir", "-ok", "-okdir"] {
            assert_refused("find", &[".", primary, "sh", ";"]);
            assert_refused("find", &[".", "-name", "x", primary, "sh", "{}", "+"]);
            assert_refused(&format!("find . {primary}"), &["sh", ";"]);
        }
        // sort: the compressor option names a program to run.
        assert_refused("sort", &["--compress-program=sh", "f"]);
        assert_refused("sort", &["--compress-program", "sh", "f"]);
        assert_refused("sort", &["--compress-prog=sh", "f"]);
        assert_refused("sort", &["--co=sh", "f"]);
        // npm: these subcommands run package scripts through a shell.
        for sub in ["run", "install", "test", "build"] {
            assert_refused("npm", &[sub]);
        }
        // cargo: run/test execute built programs; build/check/clippy run
        // build scripts and accept program-naming config on the command line.
        for sub in ["run", "test", "build", "check", "clippy"] {
            assert_refused("cargo", &[sub]);
        }
        assert_refused("cargo", &["fmt", "--config", "x=1"]);
        // git: transport options name the remote helper program to start.
        for arg in [
            "--upload-pack=sh",
            "--upload-pack",
            "--upload-p=sh",
            "--receive-pack=sh",
            "--receive-pack",
            "--rec=sh",
            "--exec=sh",
            "--exec",
            "--ex=sh",
        ] {
            assert_refused("git", &["pull", arg, "origin"]);
            assert_refused("git", &["push", arg, "origin"]);
        }
        // Global options before the subcommand stay refused.
        assert_refused("git", &["-c", "alias.x=!sh", "x"]);
        assert_refused("git", &["-c", "core.pager=sh", "log"]);
    }

    /// Positive controls: ordinary commands keep validating after F9.
    #[test]
    fn m0_f9_harmless_commands_still_validate() {
        assert_validates("ls", &[]);
        assert_validates("ls", &["-la"]);
        assert_validates("echo", &["hello"]);
        assert_validates("cat", &["file.txt"]);
        assert_validates("find", &[".", "-name", "*.rs", "-type", "f"]);
        assert_validates("sort", &["-n", "-r", "file.txt"]);
        assert_validates("sort", &["--check", "file.txt"]);
        // M0-FIX2 F9 review: git, cargo and npm entries removed from this
        // positive control; they are now refused (see the repair oracle).
        assert_validates("find", &[".", "-type", "d", "-print"]);
        assert_validates("systemctl", &["status"]);
        assert_validates("free", &["-m"]);
        assert_validates("df", &["-h"]);
    }

    /// M0-FIX2 F9 review oracle (MAJOR-1, MAJOR-2, MINOR-2): programs that
    /// start programs named in files they read from the working directory or
    /// HOME (repository config, hooks, attributes, toolchain and cargo config,
    /// npm and docker config) are refused outright, and `find` may not write
    /// or delete files. Validation only; nothing runs.
    #[test]
    fn m0_f9_config_driven_trampolines_are_refused() {
        assert_refused("git", &[]);
        for sub in [
            "status", "log", "diff", "add", "commit", "push", "pull", "branch", "show",
        ] {
            assert_refused("git", &[sub]);
            assert_refused(&format!("git {sub}"), &[]);
        }
        assert_refused("cargo", &[]);
        assert_refused("cargo", &["fmt"]);
        assert_refused("cargo", &["fmt", "--", "--check"]);
        assert_refused("cargo fmt", &[]);
        assert_refused("npm", &[]);
        assert_refused("npm", &["ls"]);
        assert_refused("docker", &[]);
        for sub in ["ps", "images", "logs", "inspect", "stats"] {
            assert_refused("docker", &[sub]);
        }
        for primary in ["-delete", "-fprint", "-fprint0", "-fprintf", "-fls"] {
            assert_refused("find", &[".", "-name", "x", primary, "out"]);
            assert_refused(&format!("find . {primary}"), &["out"]);
        }
    }
}
