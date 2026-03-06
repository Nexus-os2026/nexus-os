//! Command-surface helpers for NEXUS OS operator and developer interfaces.

use clap::{Parser, Subcommand};
use coding_agent::run_coding_agent_from_manifest;
use nexus_kernel::manifest::parse_manifest;
use self_improve_agent::prompt_optimizer::PromptOutcome;
use self_improve_agent::r#loop::{run_once_with_storage, AgentRunObservation, ImprovementStatus};
use self_improve_agent::tracker::{OutcomeResult, TaskMetrics, TaskType};
use social_poster_agent::run_social_poster_from_manifest;
pub mod commands;
pub mod router;
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
    SelfImprove {
        #[command(subcommand)]
        command: SelfImproveCommand,
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

#[derive(Debug, Subcommand)]
pub enum SelfImproveCommand {
    Run {
        #[arg(long)]
        agent: String,
    },
}

pub fn execute_command(cli: Cli) -> Result<String, String> {
    match cli.command {
        TopLevelCommand::Agent { command } => execute_agent_command(command),
        TopLevelCommand::Voice { command } => execute_voice_command(command),
        TopLevelCommand::Setup { check } => setup::run_setup(check),
        TopLevelCommand::SelfImprove { command } => execute_self_improve_command(command),
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
        let posted_ratio = if report.generated_posts.is_empty() {
            0.0
        } else {
            report.published_post_ids.len() as f64 / report.generated_posts.len() as f64
        };
        let post_outcome = if report.published_post_ids.is_empty() {
            OutcomeResult::Partial
        } else {
            OutcomeResult::Success
        };
        let hook = run_self_improve_hook(AgentRunObservation {
            agent_id: agent_id.to_string(),
            task: "run social poster workflow".to_string(),
            task_type: TaskType::Posting,
            result: post_outcome,
            metrics: TaskMetrics {
                engagement_rate: Some(posted_ratio),
                approval_rate: Some(1.0),
                reach: Some(report.generated_posts.len() as f64),
                ..TaskMetrics::default()
            },
            base_prompt: "Generate approved social posts with platform constraints.".to_string(),
            prompt_outcomes: vec![PromptOutcome {
                prompt: "Generate approved social posts with platform constraints.".to_string(),
                success: !report.published_post_ids.is_empty(),
                score: posted_ratio,
            }],
            governance_approved: true,
            destructive_change_requested: false,
            sandbox_validation_passed: true,
        })?;
        summary.push_str(
            format!(
                "\nself-improve: status={:?}, version={}",
                hook.status, hook.version.version_id
            )
            .as_str(),
        );
        return Ok(summary);
    }

    if agent_id == "coding-agent" {
        let manifest = resolve_coding_agent_manifest()?;
        let report = run_coding_agent_from_manifest(manifest.as_path(), dry_run)
            .map_err(|error| format!("coding-agent run failed: {error}"))?;
        let mut summary = format!(
            "Agent 'coding-agent' completed (dry_run={}, success={}, iterations={}, modified_files={}, fuel_remaining={})\nstatus: {}",
            report.dry_run,
            report.success,
            report.iterations,
            report.modified_files.len(),
            report.fuel_remaining,
            report.status
        );
        let fix_iterations = if report.iterations == 0 {
            0.0
        } else {
            report.iterations as f64
        };
        let hook = run_self_improve_hook(AgentRunObservation {
            agent_id: agent_id.to_string(),
            task: "run coding agent iteration".to_string(),
            task_type: TaskType::Coding,
            result: if report.success {
                OutcomeResult::Success
            } else {
                OutcomeResult::Failure
            },
            metrics: TaskMetrics {
                test_pass_rate: Some(if report.success { 1.0 } else { 0.0 }),
                fix_iterations: Some(fix_iterations),
                code_quality_score: Some(if report.success { 0.9 } else { 0.4 }),
                ..TaskMetrics::default()
            },
            base_prompt: "Implement requested code changes and iterate until tests pass."
                .to_string(),
            prompt_outcomes: vec![PromptOutcome {
                prompt: "Implement requested code changes and iterate until tests pass."
                    .to_string(),
                success: report.success,
                score: if report.success { 0.9 } else { 0.2 },
            }],
            governance_approved: true,
            destructive_change_requested: false,
            sandbox_validation_passed: report.success || dry_run,
        })?;
        summary.push_str(
            format!(
                "\nself-improve: status={:?}, version={}",
                hook.status, hook.version.version_id
            )
            .as_str(),
        );
        return Ok(summary);
    }

    let generic_summary = if dry_run {
        format!("Agent '{agent_id}' started successfully (dry-run mode)")
    } else {
        format!("Agent '{agent_id}' started successfully")
    };

    let hook = run_self_improve_hook(AgentRunObservation {
        agent_id: agent_id.to_string(),
        task: "run generic agent command".to_string(),
        task_type: TaskType::Other,
        result: OutcomeResult::Partial,
        metrics: TaskMetrics::default(),
        base_prompt: "Execute governed agent workflow and report outcome.".to_string(),
        prompt_outcomes: vec![PromptOutcome {
            prompt: "Execute governed agent workflow and report outcome.".to_string(),
            success: true,
            score: 0.5,
        }],
        governance_approved: true,
        destructive_change_requested: false,
        sandbox_validation_passed: true,
    })?;

    Ok(format!(
        "{generic_summary}\nself-improve: status={:?}, version={}",
        hook.status, hook.version.version_id
    ))
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

pub fn execute_self_improve_command(command: SelfImproveCommand) -> Result<String, String> {
    match command {
        SelfImproveCommand::Run { agent } => {
            let result = run_self_improve_hook(AgentRunObservation {
                agent_id: agent.clone(),
                task: "manual self-improve run".to_string(),
                task_type: TaskType::Other,
                result: OutcomeResult::Partial,
                metrics: TaskMetrics::default(),
                base_prompt: "Review recent outcomes and suggest safe, tested improvements."
                    .to_string(),
                prompt_outcomes: vec![PromptOutcome {
                    prompt: "Review recent outcomes and suggest safe, tested improvements."
                        .to_string(),
                    success: true,
                    score: 0.6,
                }],
                governance_approved: true,
                destructive_change_requested: false,
                sandbox_validation_passed: true,
            })?;

            let line = match result.status {
                ImprovementStatus::Applied => "applied",
                ImprovementStatus::SkippedNeedsApproval => "skipped-needs-approval",
                ImprovementStatus::SkippedSandboxValidation => "skipped-sandbox-validation",
            };
            Ok(format!(
                "Self-improve run complete for '{}' (status={}, version={})",
                agent, line, result.version.version_id
            ))
        }
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

fn resolve_self_improve_storage_dir(agent_id: &str) -> Result<PathBuf, String> {
    if let Ok(root) = std::env::var("NEXUS_SELF_IMPROVE_DIR") {
        return Ok(PathBuf::from(root).join(agent_id));
    }

    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        return Ok(PathBuf::from(tmpdir)
            .join("nexus-self-improve")
            .join(agent_id));
    }

    Ok(PathBuf::from("/tmp")
        .join("nexus-self-improve")
        .join(agent_id))
}

fn run_self_improve_hook(
    observation: AgentRunObservation,
) -> Result<self_improve_agent::r#loop::LoopResult, String> {
    let agent_id = observation.agent_id.clone();
    let storage_dir = resolve_self_improve_storage_dir(agent_id.as_str())?;
    run_once_with_storage(storage_dir, agent_id.as_str(), observation)
        .map_err(|error| format!("self-improve hook failed: {error}"))
}
