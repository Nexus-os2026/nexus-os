//! First-run experience, diagnostics, and `nx doctor`.

use colored::Colorize;

/// Setup diagnostic status.
pub struct SetupStatus {
    pub has_any_provider: bool,
    pub configured_providers: Vec<String>,
    pub unconfigured_providers: Vec<(String, String)>,
    pub has_git: bool,
    pub has_ripgrep: bool,
    pub has_nexuscode_md: bool,
}

/// Why a host that runs no external CLI agent reports the Claude CLI
/// unavailable.
pub const CLI_AGENT_UNAVAILABLE: &str =
    "Unavailable in the Nexus OS desktop: an external CLI agent runs outside Nexus authority";

/// Run a full setup diagnostic.
pub fn diagnose() -> SetupStatus {
    diagnose_with(true, true)
}

/// Run the setup diagnostic without running any external CLI agent.
///
/// The Claude CLI is reported unavailable instead of being probed. The Nexus
/// OS desktop uses this: it never runs an external CLI agent (P0-002C5A).
pub fn diagnose_without_cli_agents() -> SetupStatus {
    diagnose_with(false, true)
}

/// The setup diagnostic for the Nexus OS desktop (P0-002C5C): no external CLI
/// agent is run, and the process working directory is not inspected, because
/// the desktop has no project `NEXUSCODE.md`. Final Gate item I: no program is
/// run to find one either; `ollama`, `git` and `rg` are looked up on `PATH` in
/// this process ([`program_on_path`]).
pub fn diagnose_for_desktop() -> SetupStatus {
    diagnose_with(false, false)
}

/// `cli_agents`: the Claude CLI may be probed. `project`: the standalone
/// terminal's project context: it reads the working directory's
/// `NEXUSCODE.md` and finds programs by running `which`, as it always has.
/// The desktop passes false for both.
fn diagnose_with(cli_agents: bool, project: bool) -> SetupStatus {
    let installed = |name: &str| program_installed(name, project);
    let provider_checks = [
        ("anthropic", "ANTHROPIC_API_KEY"),
        ("openai", "OPENAI_API_KEY"),
        ("google", "GOOGLE_API_KEY"),
        ("openrouter", "OPENROUTER_API_KEY"),
        ("groq", "GROQ_API_KEY"),
        ("deepseek", "DEEPSEEK_API_KEY"),
    ];

    let mut configured = Vec::new();
    let mut unconfigured = Vec::new();

    for (name, env_var) in &provider_checks {
        if std::env::var(env_var).is_ok() {
            configured.push(name.to_string());
        } else {
            unconfigured.push((name.to_string(), env_var.to_string()));
        }
    }

    // Claude CLI — uses Claude Code binary (Max plan = $0 cost)
    if !cli_agents {
        unconfigured.push(("claude_cli".to_string(), CLI_AGENT_UNAVAILABLE.to_string()));
    } else if check_claude_cli_available() {
        configured.push("claude_cli".to_string());
    } else {
        unconfigured.push((
            "claude_cli".to_string(),
            "Install Claude Code CLI (npm install -g @anthropic-ai/claude-code)".to_string(),
        ));
    }

    // Ollama is always available if installed
    if installed("ollama") {
        configured.push("ollama".to_string());
    } else {
        unconfigured.push((
            "ollama".to_string(),
            "Install from https://ollama.ai".to_string(),
        ));
    }

    SetupStatus {
        has_any_provider: !configured.is_empty(),
        configured_providers: configured,
        unconfigured_providers: unconfigured,
        has_git: installed("git"),
        has_ripgrep: installed("rg"),
        has_nexuscode_md: project && std::path::Path::new("NEXUSCODE.md").exists(),
    }
}

/// Check if the Claude CLI binary is available and is version 2.x+.
pub fn check_claude_cli_available() -> bool {
    if !check_command_exists("claude") {
        return false;
    }
    // Verify version is 2.x+ (Claude Code CLI)
    std::process::Command::new("claude")
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            if !o.status.success() {
                return None;
            }
            let version = String::from_utf8_lossy(&o.stdout).to_string();
            // Version string contains a major version number >= 2
            version
                .trim()
                .split('.')
                .next()
                .and_then(|major| {
                    major
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u32>()
                        .ok()
                })
                .filter(|&major| major >= 2)
        })
        .is_some()
}

/// Check if a command exists on PATH.
pub fn check_command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Whether a program named `name` is installed: found by running `which`
/// when `run_which` (the standalone terminal, as it always has), or else
/// looked up on `PATH` in this process with no program run
/// ([`program_on_path`]; the desktop).
pub fn program_installed(name: &str, run_which: bool) -> bool {
    if run_which {
        check_command_exists(name)
    } else {
        program_on_path(name)
    }
}

/// Whether a program named `name` is on the launch environment's `PATH`
/// (Final Gate item I). The lookup runs in this process: no program, such as
/// `which`, is started to find one. See [`program_on_path_in`].
pub fn program_on_path(name: &str) -> bool {
    program_on_path_in(name, std::env::var_os("PATH").as_deref())
}

/// [`program_on_path`] for a given `PATH` value. A relative entry, which would
/// name a directory under the working directory, is skipped, and so is a name
/// with a path separator. On Unix the file must be executable; on Windows the
/// name is tried with each `PATHEXT` extension (by default `.COM`, `.EXE`,
/// `.BAT` and `.CMD`).
pub fn program_on_path_in(name: &str, path: Option<&std::ffi::OsStr>) -> bool {
    if name.is_empty() || name.contains(['/', '\\']) {
        return false;
    }
    let Some(path) = path else {
        return false;
    };
    let names = program_file_names(name);
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .any(|dir| names.iter().any(|file| is_program(&dir.join(file))))
}

/// The file names a program named `name` may have.
fn program_file_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| format!("{name}{extension}"))
            .collect()
    } else {
        vec![name.to_string()]
    }
}

/// Whether `file` is a regular file (links followed) that may be run; on
/// Unix, one with an execute permission bit.
fn is_program(file: &std::path::Path) -> bool {
    std::fs::metadata(file).is_ok_and(|metadata| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            metadata.is_file()
        }
    })
}

/// Display the first-run welcome when no provider is configured.
pub fn print_first_run_guide(status: &SetupStatus) {
    println!();
    println!(
        "{}",
        "\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}".cyan()
    );
    println!(
        "{}",
        "\u{2502}  Welcome to Nexus Code (nx)                    \u{2502}".cyan()
    );
    println!(
        "{}",
        "\u{2502}  The governed terminal coding agent             \u{2502}".cyan()
    );
    println!(
        "{}",
        "\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}".cyan()
    );
    println!();

    if !status.has_any_provider {
        println!(
            "{}",
            "No LLM provider configured. Set up at least one:".yellow()
        );
        println!();
        println!("  {} Set ANTHROPIC_API_KEY for Claude", "Option 1:".bold());
        println!("    export ANTHROPIC_API_KEY=sk-ant-...");
        println!();
        println!("  {} Set OPENAI_API_KEY for GPT", "Option 2:".bold());
        println!("    export OPENAI_API_KEY=sk-...");
        println!();
        println!(
            "  {} Install Ollama for local models (free)",
            "Option 3:".bold()
        );
        println!("    curl -fsSL https://ollama.ai/install.sh | sh");
        println!("    ollama pull qwen3:8b");
        println!("    nx chat -p ollama -m qwen3:8b");
        println!();
        println!("  Run {} for full diagnostic.", "nx doctor".bold());
    } else {
        println!(
            "  Configured: {}",
            status.configured_providers.join(", ").green()
        );
        println!();
        println!("  Quick start:");
        println!("    {} \u{2014} interactive chat", "nx chat".bold());
        println!(
            "    {} \u{2014} headless mode",
            "nx chat \"fix the bug\"".bold()
        );
        println!("    {} \u{2014} create project config", "nx init".bold());
    }
    println!();
}

/// Display `nx doctor` diagnostic output.
pub fn print_doctor(status: &SetupStatus) {
    println!();
    println!("{}", "Nexus Code \u{2014} System Diagnostic".bold());
    println!();

    println!("  {}", "LLM Providers:".bold());
    for name in &status.configured_providers {
        if name == "claude_cli" {
            println!("    {} {} (Claude Code Max plan)", "\u{2713}".green(), name);
        } else {
            println!("    {} {}", "\u{2713}".green(), name);
        }
    }
    for (name, env_var) in &status.unconfigured_providers {
        println!("    {} {} (set {})", "\u{2717}".red(), name, env_var);
    }
    println!();

    println!("  {}", "System Tools:".bold());
    println!(
        "    {} git{}",
        if status.has_git {
            "\u{2713}".green()
        } else {
            "\u{2717}".red()
        },
        if !status.has_git {
            " (required \u{2014} install git)"
        } else {
            ""
        }
    );
    println!(
        "    {} ripgrep (rg){}",
        if status.has_ripgrep {
            "\u{2713}".green()
        } else {
            "\u{25cb}".yellow()
        },
        if !status.has_ripgrep {
            " (optional \u{2014} faster search)"
        } else {
            ""
        }
    );
    println!();

    println!("  {}", "Project:".bold());
    println!(
        "    {} NEXUSCODE.md{}",
        if status.has_nexuscode_md {
            "\u{2713}".green()
        } else {
            "\u{25cb}".yellow()
        },
        if !status.has_nexuscode_md {
            " (run 'nx init' to create)"
        } else {
            ""
        }
    );
    println!();

    if status.has_any_provider && status.has_git {
        println!("  {} Ready to use!", "Status:".bold());
    } else if !status.has_any_provider {
        println!(
            "  {} Configure at least one LLM provider.",
            "Status:".bold()
        );
    } else if !status.has_git {
        println!(
            "  {} Install git for version control features.",
            "Status:".bold()
        );
    }
    println!();
}

/// Detect project language and create NEXUSCODE.md with appropriate settings.
pub fn init_nexuscode_md(working_dir: &std::path::Path) -> Result<(), crate::error::NxError> {
    let path = working_dir.join("NEXUSCODE.md");
    if path.exists() {
        return Err(crate::error::NxError::ConfigError(
            "NEXUSCODE.md already exists. Delete it first to reinitialize.".to_string(),
        ));
    }

    let language = if working_dir.join("Cargo.toml").exists() {
        "rust"
    } else if working_dir.join("package.json").exists() {
        "javascript"
    } else if working_dir.join("pyproject.toml").exists() {
        "python"
    } else if working_dir.join("go.mod").exists() {
        "go"
    } else {
        "unknown"
    };

    let (build_cmd, test_cmd, lint_cmd) = match language {
        "rust" => ("cargo build", "cargo test", "cargo clippy -- -D warnings"),
        "javascript" => ("npm run build", "npm test", "npx eslint ."),
        "python" => (
            "python -m build",
            "python -m pytest",
            "python -m ruff check .",
        ),
        "go" => ("go build ./...", "go test ./...", "golangci-lint run"),
        _ => (
            "echo 'no build configured'",
            "echo 'no tests configured'",
            "echo 'no lint configured'",
        ),
    };

    let project_name = working_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-project".to_string());

    let content = format!(
        "# NEXUSCODE.md\n\n\
         ## Project\n\
         name: {}\n\
         language: {}\n\
         build: {}\n\
         test: {}\n\
         lint: {}\n\n\
         ## Governance\n\
         fuel_budget: 50000\n\
         blocked_paths: .env, .env.local\n\n\
         ## Models\n\
         execution: anthropic/claude-sonnet-4-20250514\n\n\
         ## Style\n\
         prefer_short_responses: true\n\
         auto_run_tests_after_edit: true\n",
        project_name, language, build_cmd, test_cmd, lint_cmd
    );

    std::fs::write(&path, content)?;
    println!(
        "{} Created NEXUSCODE.md for {} project",
        "\u{2713}".green(),
        language
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// The file a program named `name` has in `dir` on this platform.
    fn program_file(dir: &Path, name: &str) -> PathBuf {
        dir.join(if cfg!(windows) {
            format!("{name}.EXE")
        } else {
            name.to_string()
        })
    }

    /// Write a program file (a shell script on Unix) that records each run
    /// by writing `<file>.ran` next to itself, with no program of its own.
    fn write_program(dir: &Path, name: &str) -> PathBuf {
        let file = program_file(dir, name);
        std::fs::write(&file, "#!/bin/sh\necho ran > \"$0.ran\"\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        file
    }

    /// Final Gate item I: a program is found on `PATH` in process. Absent
    /// names, directories, non-executable files (Unix), names with a
    /// separator, a missing `PATH` and relative entries find nothing.
    #[test]
    fn p0_fg_programs_are_looked_up_on_path_in_process() {
        let dir = tempfile::tempdir().unwrap();
        let program = write_program(dir.path(), "fg-tool");
        std::fs::create_dir(program_file(dir.path(), "fg-dir")).unwrap();
        let path =
            std::env::join_paths([PathBuf::from("fg-relative-entry"), dir.path().to_path_buf()])
                .unwrap();
        assert!(program_on_path_in("fg-tool", Some(&path)));
        for absent in ["fg-absent", "fg-dir", "", "sub/fg-tool", "sub\\fg-tool"] {
            assert!(!program_on_path_in(absent, Some(&path)), "{absent:?}");
        }
        assert!(!program_on_path_in("fg-tool", None));
        let ran = PathBuf::from(format!("{}.ran", program.display()));
        assert!(!ran.exists(), "the lookup ran the program");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let plain = dir.path().join("fg-plain");
            std::fs::write(&plain, "data").unwrap();
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(!program_on_path_in("fg-plain", Some(&path)));

            // A relative entry that names the program's directory from the
            // working directory is skipped.
            let cwd = std::env::current_dir().unwrap();
            let up = cwd
                .components()
                .filter(|component| matches!(component, std::path::Component::Normal(_)))
                .count();
            let relative = std::iter::repeat_n("..", up)
                .collect::<PathBuf>()
                .join(dir.path().strip_prefix("/").unwrap());
            assert!(relative.is_relative() && relative.join("fg-tool").exists());
            let only_relative = std::env::join_paths([relative]).unwrap();
            assert!(!program_on_path_in("fg-tool", Some(&only_relative)));
        }
    }

    /// Set only in the environment of the stand-in below.
    const STAND_IN_ENV: &str = "NEXUS_FG_DESKTOP_DIAGNOSTIC";

    /// Not a check. Run by the test below in a child process whose `PATH` is
    /// a scratch directory, it prints what the desktop's diagnostic and
    /// configuration found. Run as a normal test, it returns at once.
    #[test]
    fn fg_desktop_diagnostic_stand_in() {
        if std::env::var_os(STAND_IN_ENV).is_some() {
            let status = diagnose_for_desktop();
            let config = crate::config::NxConfig::load_for_desktop(None).unwrap();
            println!(
                "FG-DESKTOP git={} rg={} ollama={} provider={}",
                status.has_git,
                status.has_ripgrep,
                status.configured_providers.iter().any(|p| p == "ollama"),
                config.default_provider
            );
        }
    }

    /// Final Gate item I: the desktop's diagnostic and configuration run no
    /// program to find one. In a child process whose `PATH` holds only
    /// stand-in `which`, `git`, `rg` and `ollama` programs that record every
    /// run, both find the three programs and none of the four runs.
    #[cfg(unix)]
    #[test]
    fn p0_fg_the_desktop_diagnostic_runs_no_program_to_find_one() {
        let dir = tempfile::tempdir().unwrap();
        let programs: Vec<PathBuf> = ["which", "git", "rg", "ollama"]
            .iter()
            .map(|name| write_program(dir.path(), name))
            .collect();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "setup::tests::fg_desktop_diagnostic_stand_in",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("PATH", dir.path())
            .env(STAND_IN_ENV, "1");
        for key in [
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "NX_PROVIDER",
            "NX_MODEL",
            "NX_FUEL_BUDGET",
        ] {
            child.env_remove(key);
        }
        let output = child.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{stdout}");
        assert!(
            stdout.contains("FG-DESKTOP git=true rg=true ollama=true provider=ollama"),
            "{stdout}"
        );
        for program in programs {
            let ran = PathBuf::from(format!("{}.ran", program.display()));
            assert!(!ran.exists(), "{} was run", program.display());
        }
    }
}
