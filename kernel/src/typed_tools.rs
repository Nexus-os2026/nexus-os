//! Typed tool interfaces — structured command execution without shell injection.
//!
//! Instead of `Command::new("sh").arg("-c").arg(user_string)`, callers build a
//! `TypedTool` enum value. Each variant maps to a specific binary with explicit
//! arguments — **no shell is ever spawned**.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

// ── Tool Enum ───────────────────────────────────────────────────────────

/// A typed tool invocation. Each variant maps to exactly one executable
/// with fully enumerated arguments — no shell interpolation possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypedTool {
    // Git operations
    GitCommit {
        message: String,
    },
    GitPush {
        remote: String,
        branch: String,
    },
    GitPull {
        remote: String,
        branch: String,
    },
    GitStatus,
    GitDiff {
        path: Option<String>,
    },
    GitCheckout {
        branch: String,
    },
    GitLog {
        count: usize,
    },

    // Cargo/Build operations
    CargoBuild {
        package: Option<String>,
        release: bool,
    },
    CargoTest {
        package: Option<String>,
        test_name: Option<String>,
    },
    CargoFmt {
        check: bool,
    },
    CargoClippy {
        deny_warnings: bool,
    },
    CargoRun {
        package: Option<String>,
        args: Vec<String>,
    },

    // Node/NPM operations
    NpmInstall,
    NpmBuild,
    NpmTest,
    NpmRun {
        script: String,
    },

    // Python operations
    PythonRun {
        script: String,
        args: Vec<String>,
    },
    PipInstall {
        packages: Vec<String>,
    },

    // File operations (already governed by fs permissions)
    FileList {
        path: String,
        recursive: bool,
    },
    FileCopy {
        from: String,
        to: String,
    },
    FileMove {
        from: String,
        to: String,
    },
    FileRemove {
        path: String,
    },
    MakeDirectory {
        path: String,
    },

    // System operations
    ProcessList,
    SystemInfo,
    DiskUsage {
        path: String,
    },

    // Custom (escape hatch with HITL approval)
    Custom {
        program: String,
        args: Vec<String>,
        requires_approval: bool,
    },
}

// ── Tool Output ─────────────────────────────────────────────────────────

/// Structured output from a typed tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub tool: String,
}

// ── Safe NPM scripts ───────────────────────────────────────────────────

const SAFE_NPM_SCRIPTS: &[&str] = &["build", "lint", "dev", "start", "test", "format", "check"];

// ── Dangerous characters ────────────────────────────────────────────────

/// Characters that should never appear in tool arguments — they indicate
/// shell injection attempts.
const DANGEROUS_CHARS: &[char] = &[';', '|', '`', '\0'];
const DANGEROUS_PATTERNS: &[&str] = &["$(", "${", "\n", "\r"];

// ── Implementation ──────────────────────────────────────────────────────

impl TypedTool {
    /// Convert to (program, args) WITHOUT using shell.
    pub fn to_command(&self) -> (String, Vec<String>) {
        match self {
            TypedTool::GitCommit { message } => (
                "git".into(),
                vec!["commit".into(), "-m".into(), message.clone()],
            ),
            TypedTool::GitPush { remote, branch } => (
                "git".into(),
                vec!["push".into(), remote.clone(), branch.clone()],
            ),
            TypedTool::GitPull { remote, branch } => (
                "git".into(),
                vec!["pull".into(), remote.clone(), branch.clone()],
            ),
            TypedTool::GitStatus => ("git".into(), vec!["status".into()]),
            TypedTool::GitDiff { path } => {
                let mut args = vec!["diff".into()];
                if let Some(p) = path {
                    args.push("--".into());
                    args.push(p.clone());
                }
                ("git".into(), args)
            }
            TypedTool::GitCheckout { branch } => {
                ("git".into(), vec!["checkout".into(), branch.clone()])
            }
            TypedTool::GitLog { count } => (
                "git".into(),
                vec!["log".into(), format!("-{count}"), "--oneline".into()],
            ),
            TypedTool::CargoBuild { package, release } => {
                let mut args = vec!["build".into()];
                if let Some(pkg) = package {
                    args.push("--package".into());
                    args.push(pkg.clone());
                }
                if *release {
                    args.push("--release".into());
                }
                ("cargo".into(), args)
            }
            TypedTool::CargoTest { package, test_name } => {
                let mut args = vec!["test".into()];
                if let Some(pkg) = package {
                    args.push("-p".into());
                    args.push(pkg.clone());
                }
                if let Some(name) = test_name {
                    args.push(name.clone());
                }
                ("cargo".into(), args)
            }
            TypedTool::CargoFmt { check } => {
                let mut args = vec!["fmt".into(), "--all".into()];
                if *check {
                    args.push("--".into());
                    args.push("--check".into());
                }
                ("cargo".into(), args)
            }
            TypedTool::CargoClippy { deny_warnings } => {
                let mut args = vec!["clippy".into()];
                if *deny_warnings {
                    args.push("--".into());
                    args.push("-D".into());
                    args.push("warnings".into());
                }
                ("cargo".into(), args)
            }
            TypedTool::CargoRun {
                package,
                args: extra,
            } => {
                let mut args = vec!["run".into()];
                if let Some(pkg) = package {
                    args.push("--package".into());
                    args.push(pkg.clone());
                }
                if !extra.is_empty() {
                    args.push("--".into());
                    args.extend(extra.clone());
                }
                ("cargo".into(), args)
            }
            TypedTool::NpmInstall => ("npm".into(), vec!["install".into()]),
            TypedTool::NpmBuild => ("npm".into(), vec!["run".into(), "build".into()]),
            TypedTool::NpmTest => ("npm".into(), vec!["test".into()]),
            TypedTool::NpmRun { script } => ("npm".into(), vec!["run".into(), script.clone()]),
            TypedTool::PythonRun {
                script,
                args: extra,
            } => {
                let mut args = vec![script.clone()];
                args.extend(extra.clone());
                ("python3".into(), args)
            }
            TypedTool::PipInstall { packages } => {
                let mut args = vec!["install".into()];
                args.extend(packages.clone());
                ("pip3".into(), args)
            }
            TypedTool::FileList { path, recursive } => {
                if *recursive {
                    ("ls".into(), vec!["-laR".into(), path.clone()])
                } else {
                    ("ls".into(), vec!["-la".into(), path.clone()])
                }
            }
            TypedTool::FileCopy { from, to } => {
                ("cp".into(), vec!["-r".into(), from.clone(), to.clone()])
            }
            TypedTool::FileMove { from, to } => ("mv".into(), vec![from.clone(), to.clone()]),
            TypedTool::FileRemove { path } => ("rm".into(), vec!["-r".into(), path.clone()]),
            TypedTool::MakeDirectory { path } => ("mkdir".into(), vec!["-p".into(), path.clone()]),
            TypedTool::ProcessList => ("ps".into(), vec!["aux".into()]),
            TypedTool::SystemInfo => ("uname".into(), vec!["-a".into()]),
            TypedTool::DiskUsage { path } => ("du".into(), vec!["-sh".into(), path.clone()]),
            TypedTool::Custom { program, args, .. } => (program.clone(), args.clone()),
        }
    }

    /// Return the kernel capability required to execute this tool.
    pub fn capability_required(&self) -> &str {
        match self {
            TypedTool::FileList { .. } => "fs.read",
            TypedTool::FileCopy { .. }
            | TypedTool::FileMove { .. }
            | TypedTool::FileRemove { .. }
            | TypedTool::MakeDirectory { .. } => "fs.write",
            _ => "process.exec",
        }
    }

    /// Return the fuel cost for this tool.
    pub fn fuel_cost(&self) -> u64 {
        match self {
            // Read operations: 2
            TypedTool::GitStatus
            | TypedTool::GitDiff { .. }
            | TypedTool::GitLog { .. }
            | TypedTool::FileList { .. }
            | TypedTool::ProcessList
            | TypedTool::SystemInfo
            | TypedTool::DiskUsage { .. } => 2,

            // Build/test: 15
            TypedTool::CargoBuild { .. }
            | TypedTool::CargoTest { .. }
            | TypedTool::CargoFmt { .. }
            | TypedTool::CargoClippy { .. }
            | TypedTool::CargoRun { .. }
            | TypedTool::NpmInstall
            | TypedTool::NpmBuild
            | TypedTool::NpmTest
            | TypedTool::NpmRun { .. }
            | TypedTool::PythonRun { .. }
            | TypedTool::PipInstall { .. }
            | TypedTool::GitCommit { .. }
            | TypedTool::GitCheckout { .. }
            | TypedTool::FileCopy { .. }
            | TypedTool::FileMove { .. }
            | TypedTool::MakeDirectory { .. } => 15,

            // Destructive/deploy: 20
            TypedTool::GitPush { .. }
            | TypedTool::GitPull { .. }
            | TypedTool::FileRemove { .. } => 20,

            // Custom: 25
            TypedTool::Custom { .. } => 25,
        }
    }

    /// Return true if this tool modifies shared state or deletes data.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            TypedTool::GitPush { .. } | TypedTool::FileRemove { .. } | TypedTool::Custom { .. }
        )
    }

    /// Validate tool arguments for injection patterns.
    ///
    /// Checks all string arguments for:
    /// - Shell metacharacters (`;`, `|`, `` ` ``, `\0`)
    /// - Subshell patterns (`$(`, `${`)
    /// - Path traversal (`../`)
    /// - Null bytes
    /// - NPM script allowlist
    /// - Python/pip flag injection
    pub fn validate(&self) -> Result<(), String> {
        // Collect all string args to validate
        let args_to_check: Vec<&str> = match self {
            TypedTool::GitCommit { message } => vec![message.as_str()],
            TypedTool::GitPush { remote, branch } => vec![remote.as_str(), branch.as_str()],
            TypedTool::GitPull { remote, branch } => vec![remote.as_str(), branch.as_str()],
            TypedTool::GitStatus => vec![],
            TypedTool::GitDiff { path } => path.iter().map(|s| s.as_str()).collect(),
            TypedTool::GitCheckout { branch } => vec![branch.as_str()],
            TypedTool::GitLog { .. } => vec![],
            TypedTool::CargoBuild { package, .. } => package.iter().map(|s| s.as_str()).collect(),
            TypedTool::CargoTest { package, test_name } => {
                let mut v: Vec<&str> = package.iter().map(|s| s.as_str()).collect();
                v.extend(test_name.iter().map(|s| s.as_str()));
                v
            }
            TypedTool::CargoFmt { .. } | TypedTool::CargoClippy { .. } => vec![],
            TypedTool::CargoRun { package, args } => {
                let mut v: Vec<&str> = package.iter().map(|s| s.as_str()).collect();
                v.extend(args.iter().map(|s| s.as_str()));
                v
            }
            TypedTool::NpmInstall | TypedTool::NpmBuild | TypedTool::NpmTest => vec![],
            TypedTool::NpmRun { script } => {
                if !SAFE_NPM_SCRIPTS.contains(&script.as_str()) {
                    return Err(format!(
                        "npm script '{script}' not in safe list: {SAFE_NPM_SCRIPTS:?}"
                    ));
                }
                vec![script.as_str()]
            }
            TypedTool::PythonRun { script, args } => {
                if script.starts_with('-') {
                    return Err(format!(
                        "python script '{script}' looks like a flag, not a file"
                    ));
                }
                let mut v = vec![script.as_str()];
                v.extend(args.iter().map(|s| s.as_str()));
                v
            }
            TypedTool::PipInstall { packages } => {
                for p in packages {
                    if p.starts_with('-') {
                        return Err(format!("pip flag '{p}' not allowed in package list"));
                    }
                    if p.contains("://") {
                        return Err(format!("pip URL install '{p}' not allowed"));
                    }
                }
                packages.iter().map(|s| s.as_str()).collect()
            }
            TypedTool::FileList { path, .. } => vec![path.as_str()],
            TypedTool::FileCopy { from, to } => vec![from.as_str(), to.as_str()],
            TypedTool::FileMove { from, to } => vec![from.as_str(), to.as_str()],
            TypedTool::FileRemove { path } => vec![path.as_str()],
            TypedTool::MakeDirectory { path } => vec![path.as_str()],
            TypedTool::ProcessList | TypedTool::SystemInfo => vec![],
            TypedTool::DiskUsage { path } => vec![path.as_str()],
            TypedTool::Custom { program, args, .. } => {
                let mut v = vec![program.as_str()];
                v.extend(args.iter().map(|s| s.as_str()));
                v
            }
        };

        for arg in &args_to_check {
            // Check for dangerous characters
            for &ch in DANGEROUS_CHARS {
                if arg.contains(ch) {
                    return Err(format!(
                        "argument contains dangerous character '{ch}': {arg}"
                    ));
                }
            }
            // Check for dangerous patterns
            for &pat in DANGEROUS_PATTERNS {
                if arg.contains(pat) {
                    return Err(format!(
                        "argument contains dangerous pattern '{pat}': {arg}"
                    ));
                }
            }
            // Check for path traversal in file-related operations
            if matches!(
                self,
                TypedTool::FileList { .. }
                    | TypedTool::FileCopy { .. }
                    | TypedTool::FileMove { .. }
                    | TypedTool::FileRemove { .. }
                    | TypedTool::MakeDirectory { .. }
                    | TypedTool::PythonRun { .. }
            ) && arg.contains("../")
            {
                return Err(format!("path traversal '../' detected in argument: {arg}"));
            }
        }

        Ok(())
    }

    /// Human-readable name for audit logging.
    fn tool_name(&self) -> String {
        match self {
            TypedTool::GitCommit { .. } => "GitCommit".into(),
            TypedTool::GitPush { .. } => "GitPush".into(),
            TypedTool::GitPull { .. } => "GitPull".into(),
            TypedTool::GitStatus => "GitStatus".into(),
            TypedTool::GitDiff { .. } => "GitDiff".into(),
            TypedTool::GitCheckout { .. } => "GitCheckout".into(),
            TypedTool::GitLog { .. } => "GitLog".into(),
            TypedTool::CargoBuild { .. } => "CargoBuild".into(),
            TypedTool::CargoTest { .. } => "CargoTest".into(),
            TypedTool::CargoFmt { .. } => "CargoFmt".into(),
            TypedTool::CargoClippy { .. } => "CargoClippy".into(),
            TypedTool::CargoRun { .. } => "CargoRun".into(),
            TypedTool::NpmInstall => "NpmInstall".into(),
            TypedTool::NpmBuild => "NpmBuild".into(),
            TypedTool::NpmTest => "NpmTest".into(),
            TypedTool::NpmRun { .. } => "NpmRun".into(),
            TypedTool::PythonRun { .. } => "PythonRun".into(),
            TypedTool::PipInstall { .. } => "PipInstall".into(),
            TypedTool::FileList { .. } => "FileList".into(),
            TypedTool::FileCopy { .. } => "FileCopy".into(),
            TypedTool::FileMove { .. } => "FileMove".into(),
            TypedTool::FileRemove { .. } => "FileRemove".into(),
            TypedTool::MakeDirectory { .. } => "MakeDirectory".into(),
            TypedTool::ProcessList => "ProcessList".into(),
            TypedTool::SystemInfo => "SystemInfo".into(),
            TypedTool::DiskUsage { .. } => "DiskUsage".into(),
            TypedTool::Custom { program, .. } => format!("Custom({program})"),
        }
    }
}

// ── Execution ───────────────────────────────────────────────────────────

/// Execute a typed tool in the given working directory.
///
/// 1. Validates arguments (rejects injection patterns)
/// 2. Converts to (program, args) via `to_command()`
/// 3. Uses `std::process::Command::new(program).args(args)` — NO shell
/// 4. Returns structured `ToolOutput`
pub fn execute_typed_tool(tool: &TypedTool, working_dir: &Path) -> Result<ToolOutput, String> {
    // Step 1: validate
    tool.validate()?;

    // Step 2: convert
    let (program, args) = tool.to_command();

    // Step 3: execute without shell
    let start = Instant::now();
    let output = std::process::Command::new(&program)
        .args(&args)
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("failed to spawn '{program}': {e}"))?;

    let duration_ms = start.elapsed().as_millis() as u64;

    // Step 4: structured output
    Ok(ToolOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms,
        tool: tool.tool_name(),
    })
}

/// Return a description of all available typed tools.
pub fn list_available_tools() -> Vec<ToolDescription> {
    vec![
        ToolDescription {
            name: "GitCommit",
            description: "Commit staged changes",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "GitPush",
            description: "Push commits to remote",
            capability: "process.exec",
            fuel_cost: 20,
            destructive: true,
        },
        ToolDescription {
            name: "GitPull",
            description: "Pull changes from remote",
            capability: "process.exec",
            fuel_cost: 20,
            destructive: false,
        },
        ToolDescription {
            name: "GitStatus",
            description: "Show working tree status",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "GitDiff",
            description: "Show file differences",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "GitCheckout",
            description: "Switch branches",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "GitLog",
            description: "Show commit history",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "CargoBuild",
            description: "Build Rust project",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "CargoTest",
            description: "Run Rust tests",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "CargoFmt",
            description: "Format Rust code",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "CargoClippy",
            description: "Lint Rust code",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "CargoRun",
            description: "Run Rust binary",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "NpmInstall",
            description: "Install Node packages",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "NpmBuild",
            description: "Build Node project",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "NpmTest",
            description: "Run Node tests",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "NpmRun",
            description: "Run npm script (safe list only)",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "PythonRun",
            description: "Run Python script",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "PipInstall",
            description: "Install Python packages",
            capability: "process.exec",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "FileList",
            description: "List directory contents",
            capability: "fs.read",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "FileCopy",
            description: "Copy file or directory",
            capability: "fs.write",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "FileMove",
            description: "Move/rename file",
            capability: "fs.write",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "FileRemove",
            description: "Delete file or directory",
            capability: "fs.write",
            fuel_cost: 20,
            destructive: true,
        },
        ToolDescription {
            name: "MakeDirectory",
            description: "Create directory",
            capability: "fs.write",
            fuel_cost: 15,
            destructive: false,
        },
        ToolDescription {
            name: "ProcessList",
            description: "List running processes",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "SystemInfo",
            description: "Show system information",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "DiskUsage",
            description: "Show disk usage",
            capability: "process.exec",
            fuel_cost: 2,
            destructive: false,
        },
        ToolDescription {
            name: "Custom",
            description: "Custom command (requires HITL approval)",
            capability: "process.exec",
            fuel_cost: 25,
            destructive: true,
        },
    ]
}

/// Description of a typed tool for UI listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescription {
    pub name: &'static str,
    pub description: &'static str,
    pub capability: &'static str,
    pub fuel_cost: u64,
    pub destructive: bool,
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_commit_to_command() {
        let tool = TypedTool::GitCommit {
            message: "fix: resolve bug".into(),
        };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["commit", "-m", "fix: resolve bug"]);
    }

    #[test]
    fn test_cargo_build_to_command() {
        let tool = TypedTool::CargoBuild {
            package: Some("nexus-kernel".into()),
            release: true,
        };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "cargo");
        assert_eq!(
            args,
            vec!["build", "--package", "nexus-kernel", "--release"]
        );
    }

    #[test]
    fn test_npm_run_to_command() {
        let tool = TypedTool::NpmRun {
            script: "build".into(),
        };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "npm");
        assert_eq!(args, vec!["run", "build"]);
    }

    #[test]
    fn test_validate_clean() {
        let tool = TypedTool::GitCommit {
            message: "fix: normal commit message".into(),
        };
        assert!(tool.validate().is_ok());

        let tool2 = TypedTool::CargoBuild {
            package: Some("nexus-kernel".into()),
            release: false,
        };
        assert!(tool2.validate().is_ok());

        let tool3 = TypedTool::FileList {
            path: "/home/user/project".into(),
            recursive: true,
        };
        assert!(tool3.validate().is_ok());
    }

    #[test]
    fn test_validate_semicolon_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg; rm -rf /".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous character"));
        assert!(err.contains(";"));
    }

    #[test]
    fn test_validate_pipe_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg | cat /etc/passwd".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous character"));
        assert!(err.contains("|"));
    }

    #[test]
    fn test_validate_backtick_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg `whoami`".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous character"));
        assert!(err.contains("`"));
    }

    #[test]
    fn test_validate_dollar_paren_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg $(rm -rf /)".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous pattern"));
        assert!(err.contains("$("));
    }

    #[test]
    fn test_validate_path_traversal() {
        let tool = TypedTool::FileRemove {
            path: "../../../etc/passwd".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("path traversal"));
    }

    #[test]
    fn test_validate_null_byte() {
        let tool = TypedTool::GitCommit {
            message: "msg\0cmd".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous character"));
    }

    #[test]
    fn test_capability_required() {
        assert_eq!(TypedTool::GitStatus.capability_required(), "process.exec");
        assert_eq!(
            TypedTool::CargoBuild {
                package: None,
                release: false
            }
            .capability_required(),
            "process.exec"
        );
        assert_eq!(
            TypedTool::FileList {
                path: ".".into(),
                recursive: false
            }
            .capability_required(),
            "fs.read"
        );
        assert_eq!(
            TypedTool::FileRemove { path: "x".into() }.capability_required(),
            "fs.write"
        );
        assert_eq!(
            TypedTool::FileCopy {
                from: "a".into(),
                to: "b".into()
            }
            .capability_required(),
            "fs.write"
        );
        assert_eq!(
            TypedTool::Custom {
                program: "x".into(),
                args: vec![],
                requires_approval: true
            }
            .capability_required(),
            "process.exec"
        );
    }

    #[test]
    fn test_fuel_cost() {
        // Read ops = 2
        assert_eq!(TypedTool::GitStatus.fuel_cost(), 2);
        assert_eq!(TypedTool::GitLog { count: 10 }.fuel_cost(), 2);
        assert_eq!(TypedTool::ProcessList.fuel_cost(), 2);
        assert_eq!(TypedTool::SystemInfo.fuel_cost(), 2);

        // Build/test = 15
        assert_eq!(
            TypedTool::CargoBuild {
                package: None,
                release: false
            }
            .fuel_cost(),
            15
        );
        assert_eq!(TypedTool::NpmBuild.fuel_cost(), 15);
        assert_eq!(
            TypedTool::GitCommit {
                message: "x".into()
            }
            .fuel_cost(),
            15
        );

        // Destructive = 20
        assert_eq!(
            TypedTool::GitPush {
                remote: "o".into(),
                branch: "m".into()
            }
            .fuel_cost(),
            20
        );
        assert_eq!(TypedTool::FileRemove { path: "x".into() }.fuel_cost(), 20);

        // Custom = 25
        assert_eq!(
            TypedTool::Custom {
                program: "x".into(),
                args: vec![],
                requires_approval: true
            }
            .fuel_cost(),
            25
        );
    }

    #[test]
    fn test_is_destructive() {
        assert!(TypedTool::GitPush {
            remote: "origin".into(),
            branch: "main".into()
        }
        .is_destructive());
        assert!(TypedTool::FileRemove {
            path: "/tmp/x".into()
        }
        .is_destructive());
        assert!(TypedTool::Custom {
            program: "x".into(),
            args: vec![],
            requires_approval: true
        }
        .is_destructive());

        assert!(!TypedTool::GitStatus.is_destructive());
        assert!(!TypedTool::CargoBuild {
            package: None,
            release: false
        }
        .is_destructive());
        assert!(!TypedTool::NpmInstall.is_destructive());
    }

    #[test]
    fn test_custom_requires_approval() {
        let tool = TypedTool::Custom {
            program: "custom-bin".into(),
            args: vec!["--flag".into()],
            requires_approval: true,
        };
        assert!(tool.is_destructive());
        assert_eq!(tool.fuel_cost(), 25);
        assert_eq!(tool.capability_required(), "process.exec");

        let (prog, args) = tool.to_command();
        assert_eq!(prog, "custom-bin");
        assert_eq!(args, vec!["--flag"]);
    }

    #[test]
    fn test_tool_output_serde() {
        let output = ToolOutput {
            stdout: "hello".into(),
            stderr: "".into(),
            exit_code: 0,
            duration_ms: 42,
            tool: "GitStatus".into(),
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: ToolOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stdout, "hello");
        assert_eq!(deserialized.exit_code, 0);
        assert_eq!(deserialized.duration_ms, 42);
        assert_eq!(deserialized.tool, "GitStatus");
    }

    #[test]
    fn test_git_diff_to_command() {
        let tool = TypedTool::GitDiff {
            path: Some("src/main.rs".into()),
        };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["diff", "--", "src/main.rs"]);

        let tool2 = TypedTool::GitDiff { path: None };
        let (_, args2) = tool2.to_command();
        assert_eq!(args2, vec!["diff"]);
    }

    #[test]
    fn test_cargo_clippy_deny_warnings() {
        let tool = TypedTool::CargoClippy {
            deny_warnings: true,
        };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["clippy", "--", "-D", "warnings"]);
    }

    #[test]
    fn test_cargo_fmt_check() {
        let tool = TypedTool::CargoFmt { check: true };
        let (prog, args) = tool.to_command();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["fmt", "--all", "--", "--check"]);
    }

    #[test]
    fn test_validate_npm_unsafe_script() {
        let tool = TypedTool::NpmRun {
            script: "postinstall".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("not in safe list"));
    }

    #[test]
    fn test_validate_python_flag_injection() {
        let tool = TypedTool::PythonRun {
            script: "-c".into(),
            args: vec![],
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("flag"));
    }

    #[test]
    fn test_validate_pip_url_rejected() {
        let tool = TypedTool::PipInstall {
            packages: vec!["git+https://evil.com/pkg".into()],
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("URL install"));
    }

    #[test]
    fn test_validate_pip_flag_rejected() {
        let tool = TypedTool::PipInstall {
            packages: vec!["--pre".into()],
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("flag"));
    }

    #[test]
    fn test_typed_tool_serde_roundtrip() {
        let tool = TypedTool::CargoTest {
            package: Some("nexus-kernel".into()),
            test_name: Some("test_fuel".into()),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: TypedTool = serde_json::from_str(&json).unwrap();
        let (prog, args) = deserialized.to_command();
        assert_eq!(prog, "cargo");
        assert_eq!(args, vec!["test", "-p", "nexus-kernel", "test_fuel"]);
    }

    #[test]
    fn test_list_available_tools_count() {
        let tools = list_available_tools();
        assert!(tools.len() >= 27);
        // Verify at least one destructive and one non-destructive
        assert!(tools.iter().any(|t| t.destructive));
        assert!(tools.iter().any(|t| !t.destructive));
    }

    #[test]
    fn test_execute_typed_tool_git_status() {
        // This actually runs `git status` in a temp dir — it should succeed
        // even in a non-git directory (exit code 128 is fine, just not a spawn error)
        let dir = std::env::temp_dir();
        let result = execute_typed_tool(&TypedTool::GitStatus, &dir);
        // Should not return an Err (spawn failure) — even if git says "not a repo"
        assert!(result.is_ok());
    }

    #[test]
    fn test_dollar_brace_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg ${HOME}".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous pattern"));
    }

    #[test]
    fn test_newline_injection() {
        let tool = TypedTool::GitCommit {
            message: "msg\ncmd".into(),
        };
        let err = tool.validate().unwrap_err();
        assert!(err.contains("dangerous pattern"));
    }
}
