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

/// Run a full setup diagnostic.
pub fn diagnose() -> SetupStatus {
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
    if check_claude_cli_available() {
        configured.push("claude_cli".to_string());
    } else {
        unconfigured.push((
            "claude_cli".to_string(),
            "Install Claude Code CLI (npm install -g @anthropic-ai/claude-code)".to_string(),
        ));
    }

    // Ollama is always available if installed
    if check_command_exists("ollama") {
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
        has_git: check_command_exists("git"),
        has_ripgrep: check_command_exists("rg"),
        has_nexuscode_md: std::path::Path::new("NEXUSCODE.md").exists(),
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
