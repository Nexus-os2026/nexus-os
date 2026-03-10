//! Typed tool interfaces for WASM agents.
//!
//! Instead of passing raw shell strings to `sh -c`, agents construct typed
//! `ToolRequest` values. Each variant maps to a precise `Command::new()` call
//! with explicit arguments — no shell is ever invoked.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Command;
// ── Request ──────────────────────────────────────────────────────────

/// A typed tool invocation. Each variant maps to exactly one executable
/// with fully enumerated arguments — no shell interpolation possible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolRequest {
    GitCommit {
        message: String,
    },
    GitStatus,
    GitAdd {
        paths: Vec<String>,
    },
    GitPush {
        remote: String,
        branch: String,
    },
    CargoTest {
        package: Option<String>,
        test_name: Option<String>,
    },
    CargoBuild {
        release: bool,
    },
    CargoClippy,
    NpmInstall,
    NpmTest,
    NpmRunScript {
        script: String,
    },
    PythonRun {
        file_path: String,
    },
    PipInstall {
        packages: Vec<String>,
    },
}

// ── Result / Error ───────────────────────────────────────────────────

/// Outcome of a typed tool execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Error from typed tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    ExecutionFailed(String),
    NotAllowed(String),
    Timeout,
}

impl Display for ToolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::ExecutionFailed(reason) => write!(f, "execution failed: {reason}"),
            ToolError::NotAllowed(reason) => write!(f, "not allowed: {reason}"),
            ToolError::Timeout => write!(f, "command timed out"),
        }
    }
}

impl std::error::Error for ToolError {}

// ── Safe npm scripts ─────────────────────────────────────────────────

const SAFE_NPM_SCRIPTS: &[&str] = &["build", "lint", "dev", "start"];

// ── Execution ────────────────────────────────────────────────────────

/// Execute a typed tool request in `working_dir`.
///
/// Every variant builds a `Command` with explicit arguments — no shell is
/// spawned and no string interpolation occurs.
pub fn execute_typed_tool(
    request: &ToolRequest,
    working_dir: &Path,
) -> Result<ToolResult, ToolError> {
    let mut cmd = build_command(request)?;
    cmd.current_dir(working_dir);

    let output = cmd
        .output()
        .map_err(|e| ToolError::ExecutionFailed(format!("spawn failed: {e}")))?;

    let exit_code = output.status.code().unwrap_or(-1);
    Ok(ToolResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code,
    })
}

/// Map a `ToolRequest` to a concrete `Command` with no shell involvement.
pub fn build_command(request: &ToolRequest) -> Result<Command, ToolError> {
    match request {
        ToolRequest::GitCommit { message } => {
            let mut cmd = Command::new("git");
            cmd.args(["commit", "-m"]).arg(message);
            Ok(cmd)
        }
        ToolRequest::GitStatus => {
            let mut cmd = Command::new("git");
            cmd.arg("status");
            Ok(cmd)
        }
        ToolRequest::GitAdd { paths } => {
            if paths.is_empty() {
                return Err(ToolError::NotAllowed(
                    "git add requires at least one path".to_string(),
                ));
            }
            let mut cmd = Command::new("git");
            cmd.arg("add");
            for p in paths {
                cmd.arg(p);
            }
            Ok(cmd)
        }
        ToolRequest::GitPush { remote, branch } => {
            let mut cmd = Command::new("git");
            cmd.args(["push"]).arg(remote).arg(branch);
            Ok(cmd)
        }
        ToolRequest::CargoTest { package, test_name } => {
            let mut cmd = Command::new("cargo");
            cmd.arg("test");
            if let Some(pkg) = package {
                cmd.args(["-p"]).arg(pkg);
            }
            if let Some(name) = test_name {
                cmd.arg(name);
            }
            Ok(cmd)
        }
        ToolRequest::CargoBuild { release } => {
            let mut cmd = Command::new("cargo");
            cmd.arg("build");
            if *release {
                cmd.arg("--release");
            }
            Ok(cmd)
        }
        ToolRequest::CargoClippy => {
            let mut cmd = Command::new("cargo");
            cmd.arg("clippy");
            Ok(cmd)
        }
        ToolRequest::NpmInstall => {
            let mut cmd = Command::new("npm");
            cmd.arg("install");
            Ok(cmd)
        }
        ToolRequest::NpmTest => {
            let mut cmd = Command::new("npm");
            cmd.arg("test");
            Ok(cmd)
        }
        ToolRequest::NpmRunScript { script } => {
            if !SAFE_NPM_SCRIPTS.contains(&script.as_str()) {
                return Err(ToolError::NotAllowed(format!(
                    "npm script '{script}' is not in the safe scripts list"
                )));
            }
            let mut cmd = Command::new("npm");
            cmd.args(["run"]).arg(script);
            Ok(cmd)
        }
        ToolRequest::PythonRun { file_path } => {
            // Reject flag injection: file_path starting with `-` could pass
            // flags like `-c` or `--import` to python3.
            if file_path.starts_with('-') {
                return Err(ToolError::NotAllowed(format!(
                    "python file_path '{file_path}' looks like a flag, not a file"
                )));
            }
            let mut cmd = Command::new("python3");
            cmd.arg(file_path);
            Ok(cmd)
        }
        ToolRequest::PipInstall { packages } => {
            if packages.is_empty() {
                return Err(ToolError::NotAllowed(
                    "pip install requires at least one package".to_string(),
                ));
            }
            // Reject URL-based installs (git+https://, https://, http://)
            for p in packages {
                if p.contains("://") {
                    return Err(ToolError::NotAllowed(format!(
                        "pip URL install '{p}' is not allowed; use package names only"
                    )));
                }
                if p.starts_with('-') {
                    return Err(ToolError::NotAllowed(format!(
                        "pip flag '{p}' is not allowed in package list"
                    )));
                }
            }
            let mut cmd = Command::new("pip3");
            cmd.arg("install");
            for p in packages {
                cmd.arg(p);
            }
            Ok(cmd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a command and collect the program + args as strings.
    fn command_parts(request: &ToolRequest) -> Result<(String, Vec<String>), ToolError> {
        let cmd = build_command(request)?;
        let program = cmd.get_program().to_string_lossy().into_owned();
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        Ok((program, args))
    }

    #[test]
    fn git_commit_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::GitCommit {
            message: "fix: resolve bug".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["commit", "-m", "fix: resolve bug"]);
    }

    #[test]
    fn git_status_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::GitStatus).unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["status"]);
    }

    #[test]
    fn git_add_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::GitAdd {
            paths: vec!["src/main.rs".to_string(), "Cargo.toml".to_string()],
        })
        .unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["add", "src/main.rs", "Cargo.toml"]);
    }

    #[test]
    fn git_add_empty_paths_rejected() {
        let err = build_command(&ToolRequest::GitAdd { paths: vec![] }).unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));
    }

    #[test]
    fn git_push_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::GitPush {
            remote: "origin".to_string(),
            branch: "main".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["push", "origin", "main"]);
    }

    #[test]
    fn cargo_test_basic() {
        let (prog, args) = command_parts(&ToolRequest::CargoTest {
            package: None,
            test_name: None,
        })
        .unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["test"]);
    }

    #[test]
    fn cargo_test_with_package_and_name() {
        let (prog, args) = command_parts(&ToolRequest::CargoTest {
            package: Some("nexus-kernel".to_string()),
            test_name: Some("test_fuel".to_string()),
        })
        .unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["test", "-p", "nexus-kernel", "test_fuel"]);
    }

    #[test]
    fn cargo_build_debug() {
        let (prog, args) = command_parts(&ToolRequest::CargoBuild { release: false }).unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["build"]);
    }

    #[test]
    fn cargo_build_release() {
        let (prog, args) = command_parts(&ToolRequest::CargoBuild { release: true }).unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["build", "--release"]);
    }

    #[test]
    fn cargo_clippy_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::CargoClippy).unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["clippy"]);
    }

    #[test]
    fn npm_install_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::NpmInstall).unwrap();
        assert_eq!(prog, "npm");
        assert_eq!(args, vec!["install"]);
    }

    #[test]
    fn npm_test_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::NpmTest).unwrap();
        assert_eq!(prog, "npm");
        assert_eq!(args, vec!["test"]);
    }

    #[test]
    fn npm_run_script_safe_allowed() {
        let (prog, args) = command_parts(&ToolRequest::NpmRunScript {
            script: "build".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "npm");
        assert_eq!(args, vec!["run", "build"]);
    }

    #[test]
    fn npm_run_script_unsafe_rejected() {
        let err = build_command(&ToolRequest::NpmRunScript {
            script: "malicious".to_string(),
        })
        .unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));
    }

    #[test]
    fn python_run_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::PythonRun {
            file_path: "script.py".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "python3");
        assert_eq!(args, vec!["script.py"]);
    }

    #[test]
    fn pip_install_builds_correct_command() {
        let (prog, args) = command_parts(&ToolRequest::PipInstall {
            packages: vec!["requests".to_string(), "flask".to_string()],
        })
        .unwrap();
        assert_eq!(prog, "pip3");
        assert_eq!(args, vec!["install", "requests", "flask"]);
    }

    #[test]
    fn pip_install_empty_packages_rejected() {
        let err = build_command(&ToolRequest::PipInstall { packages: vec![] }).unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));
    }

    #[test]
    fn shell_injection_in_message_is_harmless() {
        // Even if the message contains shell metacharacters, Command::new
        // passes it as a single argument — no shell interpretation.
        let (prog, args) = command_parts(&ToolRequest::GitCommit {
            message: "$(rm -rf /); echo pwned".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["commit", "-m", "$(rm -rf /); echo pwned"]);
    }

    #[test]
    fn tool_request_serializes_round_trip() {
        let request = ToolRequest::CargoTest {
            package: Some("nexus-sdk".to_string()),
            test_name: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ToolRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn tool_error_display() {
        assert_eq!(
            ToolError::ExecutionFailed("boom".to_string()).to_string(),
            "execution failed: boom"
        );
        assert_eq!(
            ToolError::NotAllowed("nope".to_string()).to_string(),
            "not allowed: nope"
        );
        assert_eq!(ToolError::Timeout.to_string(), "command timed out");
    }

    // ── Step 6 tests ─────────────────────────────────────────────────

    #[test]
    fn test_git_commit_builds_correct_args() {
        let (prog, args) = command_parts(&ToolRequest::GitCommit {
            message: "fix: resolve login bug".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["commit", "-m", "fix: resolve login bug"]);
    }

    #[test]
    fn test_cargo_test_with_package() {
        let (prog, args) = command_parts(&ToolRequest::CargoTest {
            package: Some("nexus-kernel".to_string()),
            test_name: None,
        })
        .unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["test", "-p", "nexus-kernel"]);
    }

    #[test]
    fn test_cargo_test_with_name() {
        let (prog, args) = command_parts(&ToolRequest::CargoTest {
            package: None,
            test_name: Some("test_fuel".to_string()),
        })
        .unwrap();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["test", "test_fuel"]);
    }

    #[test]
    fn test_cargo_build_release() {
        let (_, args) = command_parts(&ToolRequest::CargoBuild { release: true }).unwrap();
        assert!(args.contains(&"--release".to_string()));
    }

    #[test]
    fn test_npm_run_build_allowed() {
        assert!(build_command(&ToolRequest::NpmRunScript {
            script: "build".to_string(),
        })
        .is_ok());
    }

    #[test]
    fn test_npm_run_malicious_blocked() {
        let err = build_command(&ToolRequest::NpmRunScript {
            script: "postinstall".to_string(),
        })
        .unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));
    }

    #[test]
    fn test_python_run_rejects_flag_injection() {
        let err = build_command(&ToolRequest::PythonRun {
            file_path: "-c".to_string(),
        })
        .unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));

        let err2 = build_command(&ToolRequest::PythonRun {
            file_path: "--import".to_string(),
        })
        .unwrap_err();
        assert!(matches!(err2, ToolError::NotAllowed(_)));
    }

    #[test]
    fn test_python_run_allows_normal_file() {
        let (prog, args) = command_parts(&ToolRequest::PythonRun {
            file_path: "main.py".to_string(),
        })
        .unwrap();
        assert_eq!(prog, "python3");
        assert_eq!(args, vec!["main.py"]);
    }

    #[test]
    fn test_pip_install_rejects_url() {
        let err = build_command(&ToolRequest::PipInstall {
            packages: vec!["git+https://evil.com/pkg".to_string()],
        })
        .unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));

        let err2 = build_command(&ToolRequest::PipInstall {
            packages: vec!["https://evil.com/package.tar.gz".to_string()],
        })
        .unwrap_err();
        assert!(matches!(err2, ToolError::NotAllowed(_)));
    }

    #[test]
    fn test_pip_install_allows_normal() {
        let (prog, args) = command_parts(&ToolRequest::PipInstall {
            packages: vec!["requests".to_string(), "flask".to_string()],
        })
        .unwrap();
        assert_eq!(prog, "pip3");
        assert_eq!(args, vec!["install", "requests", "flask"]);
    }

    #[test]
    fn test_tool_request_json_roundtrip() {
        let request = ToolRequest::GitCommit {
            message: "initial commit".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ToolRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_git_add_rejects_empty_paths() {
        let err = build_command(&ToolRequest::GitAdd { paths: vec![] }).unwrap_err();
        assert!(matches!(err, ToolError::NotAllowed(_)));
    }
}
