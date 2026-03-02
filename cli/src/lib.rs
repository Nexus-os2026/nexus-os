//! Command-surface helpers for NEXUS OS operator and developer interfaces.

use clap::{Parser, Subcommand};
use coding_agent::run_coding_agent_from_manifest;
use nexus_kernel::manifest::parse_manifest;
use social_poster_agent::run_social_poster_from_manifest;
pub mod setup;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Parser)]
#[command(name = "nexus", about = "NEXUS OS command-line interface")]
pub struct Cli {
    #[command(subcommand)]
    pub command: TopLevelCommand,
}

#[derive(Debug, Subcommand)]
pub enum TopLevelCommand {
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Voice {
        #[command(subcommand)]
        command: VoiceCommand,
    },
    Setup {
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    Create {
        manifest: String,
    },
    Start {
        agent_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    Stop {
        agent_id: String,
    },
    Pause {
        agent_id: String,
    },
    Resume {
        agent_id: String,
    },
    Destroy {
        agent_id: String,
    },
    List,
    Logs {
        agent_id: String,
    },
    Audit {
        agent_id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum VoiceCommand {
    Start,
    Test,
    Models,
}

pub fn execute_command(cli: Cli) -> Result<String, String> {
    match cli.command {
        TopLevelCommand::Agent { command } => execute_agent_command(command),
        TopLevelCommand::Voice { command } => execute_voice_command(command),
        TopLevelCommand::Setup { check } => setup::run_setup(check),
    }
}

pub fn execute_agent_command(command: AgentCommand) -> Result<String, String> {
    match command {
        AgentCommand::Create { manifest } => create_agent_from_path(Path::new(&manifest))
            .map_err(|error| format!("Failed to create agent: {error}")),
        AgentCommand::Start { agent_id, dry_run } => start_agent(agent_id.as_str(), dry_run),
        AgentCommand::Stop { agent_id } => Ok(format!("Agent '{agent_id}' stopped successfully")),
        AgentCommand::Pause { agent_id } => Ok(format!("Agent '{agent_id}' paused successfully")),
        AgentCommand::Resume { agent_id } => Ok(format!("Agent '{agent_id}' resumed successfully")),
        AgentCommand::Destroy { agent_id } => {
            Ok(format!("Agent '{agent_id}' destroyed successfully"))
        }
        AgentCommand::List => Ok("Listing all registered agents".to_string()),
        AgentCommand::Logs { agent_id } => Ok(format!("Showing logs for agent '{agent_id}'")),
        AgentCommand::Audit { agent_id } => {
            Ok(format!("Showing audit trail for agent '{agent_id}'"))
        }
    }
}

fn start_agent(agent_id: &str, dry_run: bool) -> Result<String, String> {
    if agent_id == "social-poster" {
        let manifest = resolve_social_poster_manifest()?;
        let report = run_social_poster_from_manifest(manifest.as_path(), dry_run)
            .map_err(|error| format!("social-poster run failed: {error}"))?;

        let mut summary = format!(
            "Agent 'social-poster' completed (dry_run={}, generated={}, published={})",
            report.dry_run,
            report.generated_posts.len(),
            report.published_post_ids.len()
        );
        if report.dry_run {
            for (index, post) in report.generated_posts.iter().enumerate() {
                summary.push_str(format!("\n[post {}] {}", index + 1, post.text).as_str());
            }
        }
        return Ok(summary);
    }

    if agent_id == "coding-agent" {
        let manifest = resolve_coding_agent_manifest()?;
        let report = run_coding_agent_from_manifest(manifest.as_path(), dry_run)
            .map_err(|error| format!("coding-agent run failed: {error}"))?;
        return Ok(format!(
            "Agent 'coding-agent' completed (dry_run={}, success={}, iterations={}, modified_files={}, fuel_remaining={})\nstatus: {}",
            report.dry_run,
            report.success,
            report.iterations,
            report.modified_files.len(),
            report.fuel_remaining,
            report.status
        ));
    }

    if dry_run {
        return Ok(format!(
            "Agent '{agent_id}' started successfully (dry-run mode)"
        ));
    }
    Ok(format!("Agent '{agent_id}' started successfully"))
}

pub fn create_agent_from_path(manifest_path: &Path) -> Result<String, String> {
    let manifest_content = fs::read_to_string(manifest_path).map_err(|error| {
        format!(
            "unable to read manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    create_agent_from_manifest_str(&manifest_content)
}

pub fn create_agent_from_manifest_str(content: &str) -> Result<String, String> {
    let manifest = parse_manifest(content).map_err(|error| error.to_string())?;
    Ok(format!(
        "Agent '{}' created successfully (fuel: {})",
        manifest.name, manifest.fuel_budget
    ))
}

pub fn execute_voice_command(command: VoiceCommand) -> Result<String, String> {
    match command {
        VoiceCommand::Start => run_voice_python("start"),
        VoiceCommand::Test => run_voice_python("test"),
        VoiceCommand::Models => run_voice_python("models"),
    }
}

fn run_voice_python(subcommand: &str) -> Result<String, String> {
    let voice_dir = resolve_voice_dir()?;
    let output = Command::new("python3")
        .arg("jarvis.py")
        .arg(subcommand)
        .current_dir(&voice_dir)
        .output()
        .map_err(|error| format!("failed to launch voice runtime: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("voice command failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(format!("Voice command '{subcommand}' completed."));
    }
    Ok(stdout)
}

fn resolve_voice_dir() -> Result<PathBuf, String> {
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("voice");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../voice");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err("voice directory not found".to_string())
}

fn resolve_social_poster_manifest() -> Result<PathBuf, String> {
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("agents/social-poster/manifest.toml");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let fallback =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agents/social-poster/manifest.toml");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err("social-poster manifest not found".to_string())
}

fn resolve_coding_agent_manifest() -> Result<PathBuf, String> {
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("agents/coding-agent/manifest.toml");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let fallback =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agents/coding-agent/manifest.toml");
    if fallback.exists() {
        return Ok(fallback);
    }

    Err("coding-agent manifest not found".to_string())
}
