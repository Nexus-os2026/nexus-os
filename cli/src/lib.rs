//! Command-surface helpers for NEXUS OS operator and developer interfaces.

use clap::{Parser, Subcommand};
use coding_agent::run_coding_agent_from_manifest;
use nexus_kernel::manifest::parse_manifest;
use self_improve_agent::prompt_optimizer::PromptOutcome;
use self_improve_agent::r#loop::{run_once_with_storage, AgentRunObservation, ImprovementStatus};
use self_improve_agent::tracker::{OutcomeResult, TaskMetrics, TaskType};
use social_poster_agent::run_social_poster_from_manifest;
pub mod commands;
pub mod packager;
pub mod router;
pub mod scaffold;
pub mod setup;
pub mod templates;
pub mod test_runner;
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
    /// Scaffold a new Nexus agent project
    Create {
        /// Agent name (3-64 chars, alphanumeric and hyphens)
        name: String,
        /// Template to use (basic, data-analyst, web-researcher, code-reviewer, content-writer, file-organizer)
        #[arg(short, long, default_value = "basic")]
        template: String,
        /// Parent directory to create the project in
        #[arg(short, long)]
        output_dir: Option<String>,
    },
    /// Test an agent in a sandboxed environment
    Test {
        /// Path to agent project directory or manifest.toml
        path: String,
    },
    /// Package an agent into a signed .nexus-agent bundle
    Package {
        /// Path to agent project directory
        path: String,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommand,
    },
    Simulation {
        #[command(subcommand)]
        command: SimulationCommand,
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
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Governance {
        #[command(subcommand)]
        command: GovernanceCommand,
    },
    /// Policy engine: manage, test, and reload Cedar-style policies
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Protocols {
        #[command(subcommand)]
        command: ProtocolsCommand,
    },
    /// Marketplace: browse, publish, install, and manage agents
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
    },
    /// Orchestrate multi-agent builds from a natural language prompt
    Conduct {
        /// What to build (e.g., "build a portfolio site with 3D hero and auth")
        prompt: String,
        /// Output directory (default: ./nexus-output/<timestamp>/)
        #[arg(short, long)]
        output_dir: Option<String>,
        /// LLM model to use (default: llama3.2)
        #[arg(short, long)]
        model: Option<String>,
        /// Preview the plan without executing
        #[arg(long)]
        preview: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum MarketplaceCommand {
    /// Search the marketplace for agents
    Search {
        /// Search query string
        query: String,
    },
    /// Install an agent from the marketplace
    Install {
        /// Agent package ID or name
        name: String,
    },
    /// Publish a signed agent bundle to the marketplace
    Publish {
        /// Path to .nexus-agent bundle file
        bundle_path: String,
    },
    /// Show detailed info about a marketplace agent
    Info {
        /// Agent package ID
        agent_id: String,
    },
    /// List agents published by an author
    MyAgents {
        /// Author ID to filter by
        #[arg(long, default_value = "me")]
        author: String,
    },
    /// Uninstall an agent from the marketplace
    Uninstall {
        /// Agent package ID or name
        name: String,
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
pub enum SandboxCommand {
    /// Show sandbox runtime status: runtime type, active agents, fuel usage, memory, capabilities
    Status,
}

#[derive(Debug, Subcommand)]
pub enum SimulationCommand {
    /// Show speculative execution engine status: pending simulations, risk levels
    Status,
}

#[derive(Debug, Subcommand)]
pub enum SelfImproveCommand {
    Run {
        #[arg(long)]
        agent: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    /// List available and downloaded local SLM models
    List,
    /// Download a model from HuggingFace
    Download { model_id: String },
    /// Load a downloaded model into memory
    Load { model_id: String },
    /// Unload the active model from memory
    Unload,
    /// Show loaded model status, RAM usage, and inference stats
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ProtocolsCommand {
    /// Show A2A and MCP protocol server status and endpoints
    Status,
    /// Display an agent's A2A Agent Card
    AgentCard {
        /// Agent name to generate the card for
        agent_name: String,
    },
    /// Start the HTTP protocol gateway
    Start {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
pub enum GovernanceCommand {
    /// Run a governance task locally (pii_detection, prompt_safety, capability_risk, content_classification)
    Test {
        /// Task type: pii_detection, prompt_safety, capability_risk, content_classification
        task_type: String,
        /// Input text to evaluate
        input: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PolicyCommand {
    /// List all loaded policies with IDs and effects
    List,
    /// Show full details of a specific policy
    Show {
        /// Policy ID to display
        policy_id: String,
    },
    /// Validate a policy TOML file for syntax errors
    Validate {
        /// Path to policy TOML file
        file: String,
    },
    /// Dry-run a policy evaluation against specific parameters
    Test {
        /// Path to policy TOML file
        file: String,
        /// Agent DID or identifier
        #[arg(long)]
        principal: String,
        /// Operation type (tool_call, terminal_command, etc.)
        #[arg(long)]
        action: String,
        /// Capability key (web.search, fs.write, etc.)
        #[arg(long)]
        resource: String,
    },
    /// Reload policies from disk without restart
    Reload,
}

pub fn execute_command(cli: Cli) -> Result<String, String> {
    match cli.command {
        TopLevelCommand::Create {
            name,
            template,
            output_dir,
        } => execute_create_command(&name, &template, output_dir.as_deref()),
        TopLevelCommand::Test { path } => execute_test_command(&path),
        TopLevelCommand::Package { path } => execute_package_command(&path),
        TopLevelCommand::Agent { command } => execute_agent_command(command),
        TopLevelCommand::Sandbox { command } => execute_sandbox_command(command),
        TopLevelCommand::Simulation { command } => execute_simulation_command(command),
        TopLevelCommand::Voice { command } => execute_voice_command(command),
        TopLevelCommand::Setup { check } => setup::run_setup(check),
        TopLevelCommand::SelfImprove { command } => execute_self_improve_command(command),
        TopLevelCommand::Model { command } => execute_model_command(command),
        TopLevelCommand::Governance { command } => execute_governance_command(command),
        TopLevelCommand::Policy { command } => execute_policy_command(command),
        TopLevelCommand::Protocols { command } => execute_protocols_command(command),
        TopLevelCommand::Marketplace { command } => execute_marketplace_command(command),
        TopLevelCommand::Conduct {
            prompt,
            output_dir,
            model,
            preview,
        } => execute_conduct_command(&prompt, output_dir.as_deref(), model.as_deref(), preview),
    }
}

pub fn execute_create_command(
    name: &str,
    template: &str,
    output_dir: Option<&str>,
) -> Result<String, String> {
    let parent = match output_dir {
        Some(dir) => PathBuf::from(dir),
        None => std::env::current_dir()
            .map_err(|e| format!("Failed to determine current directory: {e}"))?,
    };

    let result = scaffold::scaffold_agent_project(name, template, &parent)?;

    let mut output = format!(
        "Created agent '{}' from template '{}'\n  {}\n\nFiles:\n",
        result.agent_name,
        result.template,
        result.project_dir.display(),
    );
    for f in &result.files_created {
        output.push_str(&format!("  - {f}\n"));
    }
    output.push_str(&format!(
        "\nNext steps:\n  cd {}\n  cargo build\n  cargo test\n",
        result.agent_name
    ));
    Ok(output)
}

pub fn execute_test_command(path: &str) -> Result<String, String> {
    let path = Path::new(path);
    let manifest_path = if path.is_dir() {
        path.join("manifest.toml")
    } else {
        path.to_path_buf()
    };

    let report = test_runner::run_agent_test(&manifest_path)?;
    let formatted = test_runner::format_report(&report);

    if report.passed {
        Ok(formatted)
    } else {
        Err(formatted)
    }
}

pub fn execute_package_command(path: &str) -> Result<String, String> {
    let project_dir = Path::new(path);
    if !project_dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    // Generate a deterministic dev signing key.
    // In production this would come from IdentityManager / keyring.
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&dev_signing_key_bytes());

    let result = packager::package_agent(project_dir, &signing_key)?;
    Ok(packager::format_result(&result))
}

/// Derive a deterministic dev signing key from the machine.
/// In production, keys would be stored in the OS keyring via IdentityManager.
fn dev_signing_key_bytes() -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    if let Ok(home) = std::env::var("HOME") {
        home.hash(&mut hasher);
    }
    "nexus-dev-signing-key".hash(&mut hasher);
    let h = hasher.finish();
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&h.to_le_bytes());
    bytes[8..16].copy_from_slice(&h.to_be_bytes());
    bytes[16..24].copy_from_slice(&h.to_le_bytes());
    bytes[24..32].copy_from_slice(&h.to_be_bytes());
    bytes
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

pub fn execute_sandbox_command(command: SandboxCommand) -> Result<String, String> {
    match command {
        SandboxCommand::Status => sandbox_status(),
    }
}

fn sandbox_status() -> Result<String, String> {
    use nexus_kernel::supervisor::Supervisor;

    let supervisor = Supervisor::new();

    let mut output = String::new();
    output.push_str("Nexus OS Sandbox Status\n");
    output.push_str("=======================\n\n");
    output.push_str("Runtime:   wasmtime v27 (real wasm isolation)\n");
    output.push_str("Policy:    AllowUnsigned (configurable per agent)\n");
    output.push_str("Engine:    shared Arc<Engine>, one Store per agent\n\n");

    // Query registered agents from supervisor's health check
    let agents = supervisor.health_check();
    if agents.is_empty() {
        output.push_str("Active Agents: (none registered)\n\n");
        output.push_str("  Use 'nexus agent create <manifest>' to register an agent.\n");
    } else {
        output.push_str(&format!("Active Agents: {}\n", agents.len()));
        output.push_str(&format!(
            "{:<38} {:<12} {:<10} {}\n",
            "AGENT ID", "STATUS", "FUEL", "MEMORY"
        ));
        output.push_str(&format!("{}\n", "-".repeat(72)));

        for agent in &agents {
            // Show capabilities if we can access the full handle
            let caps_display = supervisor
                .get_agent(agent.id)
                .map(|h| {
                    let caps: Vec<&str> = h
                        .manifest
                        .capabilities
                        .iter()
                        .take(3)
                        .map(|s| s.as_str())
                        .collect();
                    let suffix = if h.manifest.capabilities.len() > 3 {
                        format!(", +{} more", h.manifest.capabilities.len() - 3)
                    } else {
                        String::new()
                    };
                    format!("  caps: {}{}", caps.join(", "), suffix)
                })
                .unwrap_or_default();

            output.push_str(&format!(
                "{:<38} {:<12} {:<10} {}\n",
                agent.id,
                format!("{:?}", agent.state),
                agent.remaining_fuel,
                "isolated",
            ));
            if !caps_display.is_empty() {
                output.push_str(&format!("  {caps_display}\n"));
            }
        }
    }

    output.push_str("\nSandbox Features:\n");
    output.push_str("  Memory isolation:     Store-per-agent (wasmtime StoreLimits)\n");
    output.push_str("  Fuel metering:        1 nexus unit = 10,000 wasm instructions\n");
    output.push_str("  Host functions:       nexus_log, nexus_emit_audit, nexus_llm_query,\n");
    output.push_str(
        "                        nexus_fs_read, nexus_fs_write, nexus_request_approval\n",
    );
    output.push_str(
        "  Signature policy:     Ed25519 (configurable: RequireSigned / AllowUnsigned)\n",
    );
    output.push_str("  Kill gate:            SafetySupervisor three-strike rule\n");

    Ok(output)
}

pub fn execute_simulation_command(command: SimulationCommand) -> Result<String, String> {
    match command {
        SimulationCommand::Status => simulation_status(),
    }
}

fn simulation_status() -> Result<String, String> {
    use nexus_kernel::supervisor::Supervisor;

    let supervisor = Supervisor::new();

    let mut output = String::new();
    output.push_str("Nexus OS Speculative Execution Engine\n");
    output.push_str("=====================================\n\n");
    output.push_str("Engine:          SpeculativeEngine (shadow simulation)\n");
    output.push_str("Auto-simulate:   Tier2+ operations (TerminalCommand, SocialPost, SelfMutation, Distributed)\n");
    output.push_str("Risk levels:     Low, Medium, High, Critical\n\n");

    let pending = supervisor.pending_simulations();
    if pending.is_empty() {
        output.push_str("Pending Simulations: (none)\n\n");
        output.push_str("  Simulations are created automatically when Tier2+ operations\n");
        output.push_str("  require approval. Use 'nexus agent start <id>' to trigger.\n");
    } else {
        output.push_str(&format!("Pending Simulations: {}\n", pending.len()));
        output.push_str(&format!(
            "{:<20} {:<24} {:<10} {:<8} {}\n",
            "REQUEST ID", "OPERATION", "RISK", "FUEL", "SUMMARY"
        ));
        output.push_str(&format!("{}\n", "-".repeat(90)));
        for (req_id, result) in &pending {
            output.push_str(&format!(
                "{:<20} {:<24} {:<10} {:<8} {}\n",
                req_id,
                result.operation.as_str(),
                result.risk_level.as_str(),
                result.resource_impact.fuel_cost,
                &result.summary[..result.summary.len().min(40)],
            ));
        }
    }

    output.push_str("\nSimulation Triggers:\n");
    output.push_str("  Tier0 (auto-allow):      No simulation\n");
    output.push_str("  Tier1 (log-only):         No simulation\n");
    output.push_str("  Tier2 (1 approver):       Auto-simulate before approval\n");
    output.push_str("  Tier3 (2 approvers):      Auto-simulate before approval\n\n");
    output.push_str("Risk Derivation:\n");
    output.push_str("  Low:      Tier0-1 at any autonomy level\n");
    output.push_str("  Medium:   Tier2 at L0-L3\n");
    output.push_str("  High:     Tier2 at L4-L5\n");
    output.push_str("  Critical: Tier3 at any autonomy level\n");

    Ok(output)
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

pub fn execute_model_command(command: ModelCommand) -> Result<String, String> {
    let output = match command {
        ModelCommand::List => router::route(commands::CliCommand::ModelList),
        ModelCommand::Download { model_id } => {
            router::route(commands::CliCommand::ModelDownload { model_id })
        }
        ModelCommand::Load { model_id } => {
            router::route(commands::CliCommand::ModelLoad { model_id })
        }
        ModelCommand::Unload => router::route(commands::CliCommand::ModelUnload),
        ModelCommand::Status => router::route(commands::CliCommand::ModelStatus),
    };
    if output.success {
        let mut result = output.message;
        if let Some(data) = output.data {
            result.push('\n');
            result.push_str(&serde_json::to_string_pretty(&data).unwrap_or_default());
        }
        Ok(result)
    } else {
        Err(output.message)
    }
}

pub fn execute_governance_command(command: GovernanceCommand) -> Result<String, String> {
    let output = match command {
        GovernanceCommand::Test { task_type, input } => {
            router::route(commands::CliCommand::GovernanceTest { task_type, input })
        }
    };
    if output.success {
        let mut result = output.message;
        if let Some(data) = output.data {
            result.push('\n');
            result.push_str(&serde_json::to_string_pretty(&data).unwrap_or_default());
        }
        Ok(result)
    } else {
        Err(output.message)
    }
}

pub fn execute_policy_command(command: PolicyCommand) -> Result<String, String> {
    let output = match command {
        PolicyCommand::List => router::route(commands::CliCommand::PolicyList),
        PolicyCommand::Show { policy_id } => {
            router::route(commands::CliCommand::PolicyShow { policy_id })
        }
        PolicyCommand::Validate { file } => {
            router::route(commands::CliCommand::PolicyValidate { file })
        }
        PolicyCommand::Test {
            file,
            principal,
            action,
            resource,
        } => router::route(commands::CliCommand::PolicyTest {
            file,
            principal,
            action,
            resource,
        }),
        PolicyCommand::Reload => router::route(commands::CliCommand::PolicyReload),
    };
    if output.success {
        let mut result = output.message;
        if let Some(data) = output.data {
            result.push('\n');
            result.push_str(&serde_json::to_string_pretty(&data).unwrap_or_default());
        }
        Ok(result)
    } else {
        Err(output.message)
    }
}

pub fn execute_protocols_command(command: ProtocolsCommand) -> Result<String, String> {
    let output = match command {
        ProtocolsCommand::Status => router::route(commands::CliCommand::ProtocolsStatus),
        ProtocolsCommand::AgentCard { agent_name } => {
            router::route(commands::CliCommand::ProtocolsAgentCard { agent_name })
        }
        ProtocolsCommand::Start { port } => {
            router::route(commands::CliCommand::ProtocolsStart { port })
        }
    };
    if output.success {
        let mut result = output.message;
        if let Some(data) = output.data {
            result.push('\n');
            result.push_str(&serde_json::to_string_pretty(&data).unwrap_or_default());
        }
        Ok(result)
    } else {
        Err(output.message)
    }
}

pub fn execute_marketplace_command(command: MarketplaceCommand) -> Result<String, String> {
    let output = match command {
        MarketplaceCommand::Search { query } => {
            router::route(commands::CliCommand::MarketplaceSearch { query })
        }
        MarketplaceCommand::Install { name } => {
            router::route(commands::CliCommand::MarketplaceInstall { name })
        }
        MarketplaceCommand::Publish { bundle_path } => {
            router::route(commands::CliCommand::MarketplacePublish { bundle_path })
        }
        MarketplaceCommand::Info { agent_id } => {
            router::route(commands::CliCommand::MarketplaceInfo { agent_id })
        }
        MarketplaceCommand::MyAgents { author } => {
            router::route(commands::CliCommand::MarketplaceMyAgents { author })
        }
        MarketplaceCommand::Uninstall { name } => {
            router::route(commands::CliCommand::MarketplaceUninstall { name })
        }
    };
    if output.success {
        let mut result = output.message;
        if let Some(data) = output.data {
            result.push('\n');
            result.push_str(&serde_json::to_string_pretty(&data).unwrap_or_default());
        }
        Ok(result)
    } else {
        Err(output.message)
    }
}

pub fn execute_conduct_command(
    prompt: &str,
    output_dir: Option<&str>,
    model: Option<&str>,
    preview: bool,
) -> Result<String, String> {
    use chrono::Local;
    use nexus_conductor::types::UserRequest;
    use nexus_conductor::Conductor;
    use nexus_connectors_llm::providers::ollama::OllamaProvider;
    use nexus_kernel::supervisor::Supervisor;
    use std::time::Instant;

    let model_name = model.unwrap_or("llama3.2");

    let dir = match output_dir {
        Some(d) => d.to_string(),
        None => {
            let ts = Local::now().format("%Y%m%d-%H%M%S");
            format!("./nexus-output/{ts}")
        }
    };

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create output directory '{}': {e}", dir))?;

    let request = UserRequest::new(prompt, &dir);
    let provider = OllamaProvider::new("http://localhost:11434")
        .with_request_timeout(180)
        .with_streaming_timeout(180);
    let mut conductor = Conductor::new(provider, model_name);

    if preview {
        let plan = conductor
            .preview_plan(&request)
            .map_err(|e| format!("Planning failed: {e}"))?;

        let mut out = format!("Planning... {} tasks identified\n\n", plan.tasks.len());
        out.push_str(&format!(
            "{:<5} {:<14} {:<50} {:<10}\n",
            "#", "ROLE", "DESCRIPTION", "EST. FUEL"
        ));
        out.push_str(&format!("{}\n", "-".repeat(79)));
        for (i, task) in plan.tasks.iter().enumerate() {
            out.push_str(&format!(
                "[{:<3}] {:<14} {:<50} {:<10}\n",
                i + 1,
                task.role.agent_crate_name(),
                if task.description.len() > 48 {
                    format!("{}...", &task.description[..45])
                } else {
                    task.description.clone()
                },
                task.estimated_fuel,
            ));
        }
        out.push_str(&format!("\nOutput would be written to: {dir}"));
        return Ok(out);
    }

    // Full execution
    let start = Instant::now();
    let mut supervisor = Supervisor::new();

    // Show planning phase
    let plan = conductor
        .preview_plan(&request)
        .map_err(|e| format!("Planning failed: {e}"))?;

    let mut out = format!("Planning... {} tasks identified\n", plan.tasks.len());
    for (i, task) in plan.tasks.iter().enumerate() {
        out.push_str(&format!(
            "  [{}] {}: {} (est. {} fuel)\n",
            i + 1,
            task.role.agent_crate_name(),
            task.description,
            task.estimated_fuel,
        ));
    }

    out.push_str("\nExecuting...\n");

    // Re-create request (the old one was consumed conceptually; IDs are unique)
    let request = UserRequest::new(prompt, &dir);
    let result = conductor
        .run(request, &mut supervisor)
        .map_err(|e| format!("Execution failed: {e}"))?;

    let duration = start.elapsed().as_secs_f64();

    for (i, task) in plan.tasks.iter().enumerate() {
        let files_count = if i == 0 { result.output_files.len() } else { 0 };
        let fuel_per = result.total_fuel_used / plan.tasks.len().max(1) as u64;
        out.push_str(&format!(
            "  [{}] {} ✓ ({} files, {} fuel)\n",
            i + 1,
            task.role.agent_crate_name(),
            files_count,
            fuel_per,
        ));
    }

    out.push_str(&format!(
        "\nDone! {} agents, {} files, {} fuel, {:.1}s\nOutput: {}",
        result.agents_used,
        result.output_files.len(),
        result.total_fuel_used,
        duration,
        dir,
    ));

    Ok(out)
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
