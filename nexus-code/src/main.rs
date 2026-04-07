//! Nexus Code — The world's first governed terminal coding agent.

use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use nexus_code::app::App;
use nexus_code::chat::ChatRepl;
use nexus_code::config::NxConfig;

/// Nexus Code — The governed terminal coding agent.
#[derive(Parser)]
#[command(
    name = "nx",
    version,
    about = "Nexus Code — The governed terminal coding agent"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Override the LLM provider
    #[arg(long, short = 'p')]
    provider: Option<String>,

    /// Override the model
    #[arg(long, short = 'm')]
    model: Option<String>,

    /// Set fuel budget
    #[arg(long)]
    fuel: Option<u64>,

    /// Enable verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Auto-approve Tier2 tool operations in headless mode
    #[arg(long)]
    auto_approve: bool,

    /// Auto-approve ALL tool operations including destructive ones (DANGEROUS)
    #[arg(long)]
    dangerously_approve_all: bool,

    /// Use the classic rustyline REPL instead of the TUI
    #[arg(long)]
    no_tui: bool,

    /// MCP server configuration (JSON string or path to JSON file)
    #[arg(long)]
    mcp_config: Option<String>,

    /// Enable computer use capabilities (screen capture, interaction, vision)
    #[arg(long)]
    computer_use: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive chat (default if no subcommand)
    Chat {
        /// Initial prompt (if provided, runs non-interactively)
        #[arg(trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Initialize NEXUSCODE.md in the current directory
    Init,
    /// Show governance status (identity, fuel, capabilities, audit chain)
    Status,
    /// Show configured providers and models
    Providers,
    /// Show version and build info
    Info,
    /// Diagnose setup issues
    Doctor,
    /// Run benchmarks (SWE-bench evaluation)
    Bench {
        #[command(subcommand)]
        action: BenchAction,
    },
}

#[derive(Subcommand)]
enum BenchAction {
    /// Run SWE-bench tasks
    Run {
        /// Path to SWE-bench JSONL file
        #[arg(long)]
        tasks_file: String,
        /// Number of tasks to run (default: all)
        #[arg(long)]
        limit: Option<usize>,
        /// Fuel budget per task
        #[arg(long, default_value_t = 20000)]
        fuel: u64,
        /// Max turns per task
        #[arg(long, default_value_t = 15)]
        max_turns: u32,
        /// Workspace directory for cloned repos
        #[arg(long, default_value = "/tmp/nx-bench")]
        workspace: String,
    },
    /// Compare the same tasks across multiple providers
    Compare {
        /// Path to SWE-bench JSONL file
        #[arg(long)]
        tasks_file: String,
        /// Provider/model pairs (format: "provider/model")
        #[arg(long, num_args = 1..)]
        providers: Vec<String>,
        /// Number of tasks
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Fuel budget per task
        #[arg(long, default_value_t = 20000)]
        fuel: u64,
        /// Max turns per task
        #[arg(long, default_value_t = 15)]
        max_turns: u32,
        /// Workspace directory
        #[arg(long, default_value = "/tmp/nx-bench")]
        workspace: String,
    },
    /// Show results from a benchmark run
    Report {
        /// Path to report JSON file
        #[arg(long, default_value = "nx-bench-report.json")]
        file: String,
    },
    /// Generate paper-ready data package from benchmark results
    Paper {
        /// Comma-separated paths to benchmark report JSON files
        #[arg(long)]
        reports: String,
        /// Output path for data package
        #[arg(long, default_value = "paper-data.json")]
        output: String,
        /// Also print LaTeX tables
        #[arg(long)]
        latex: bool,
    },
}

/// Run in headless mode (non-interactive).
async fn run_headless(
    app: &mut App,
    prompt: &str,
    auto_approve: bool,
    dangerously_approve_all: bool,
) -> Result<(), nexus_code::error::NxError> {
    let mut messages = vec![nexus_code::llm::types::Message {
        role: nexus_code::llm::types::Role::User,
        content: prompt.to_string(),
    }];

    let config = nexus_code::agent::AgentConfig {
        max_turns: 10,
        system_prompt: "You are Nexus Code, a governed terminal coding agent. \
                        Be concise. Use tools to accomplish the task."
            .to_string(),
        model_slot: nexus_code::llm::router::ModelSlot::Execution,
        auto_approve_tier2: auto_approve || dangerously_approve_all,
        auto_approve_tier3: dangerously_approve_all,
        computer_use_active: app.is_computer_use_active(),
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    // Headless consent handler: approve/deny based on config
    let approve_t2 = config.auto_approve_tier2;
    let approve_t3 = config.auto_approve_tier3;
    let consent_handler: Arc<
        dyn Fn(&nexus_code::governance::ConsentRequest) -> bool + Send + Sync,
    > = Arc::new(move |request| match request.tier {
        nexus_code::governance::ConsentTier::Tier1 => true,
        nexus_code::governance::ConsentTier::Tier2 => approve_t2,
        nexus_code::governance::ConsentTier::Tier3 => approve_t3,
    });

    let cancel = tokio_util::sync::CancellationToken::new();

    let tool_ctx = nexus_code::tools::ToolContext {
        working_dir: app
            .config
            .project_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
        blocked_paths: app.config.blocked_paths.clone(),
        max_file_scope: app.config.max_file_scope.clone(),
        non_interactive: true,
    };

    // Spawn event display (minimal for headless)
    let display_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                nexus_code::agent::AgentEvent::TextDelta(text) => {
                    print!("{}", text);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                nexus_code::agent::AgentEvent::ToolCallStart { name, .. } => {
                    eprintln!("{}", format!("[Calling {}...]", name).dimmed());
                }
                nexus_code::agent::AgentEvent::ToolCallComplete {
                    name,
                    success,
                    duration_ms,
                    ..
                } => {
                    let status = if success { "✓" } else { "✗" };
                    eprintln!("{} {} ({}ms)", status, name, duration_ms);
                }
                nexus_code::agent::AgentEvent::ToolCallDenied { name, reason } => {
                    eprintln!("{} {} denied: {}", "✗".red(), name, reason);
                }
                nexus_code::agent::AgentEvent::Error(msg) => {
                    eprintln!("{}: {}", "Error".red(), msg);
                }
                _ => {}
            }
        }
    });

    // Run agent loop
    let result = nexus_code::agent::run_agent_loop(
        &mut messages,
        &app.router,
        &app.tool_registry,
        &tool_ctx,
        &mut app.governance,
        &config,
        event_tx,
        consent_handler,
        cancel,
    )
    .await;

    let _ = display_handle.await;
    println!();

    match result {
        Ok(_) => {
            app.governance.end_session("headless complete");
            Ok(())
        }
        Err(e) => {
            app.governance
                .end_session(&format!("headless error: {}", e));
            eprintln!("{}: {}", "Error".red(), e);
            std::process::exit(1);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let mut config = NxConfig::load()?;

    if let Some(provider) = cli.provider {
        config.default_provider = provider;
    }
    if let Some(model) = cli.model {
        config.default_model = model;
    }
    if let Some(fuel) = cli.fuel {
        config.fuel_budget = fuel;
    }

    let mut app = App::new(config)?;

    // Enable computer use if requested
    if cli.computer_use {
        app.enable_computer_use();
    }

    // Connect to MCP servers if configured
    if let Some(ref mcp_config_str) = cli.mcp_config {
        let configs: Vec<nexus_code::mcp::McpServerConfig> = if mcp_config_str.ends_with(".json") {
            let content = std::fs::read_to_string(mcp_config_str)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::from_str(mcp_config_str)?
        };
        app.mcp_manager.connect_all(&configs).await;
        app.mcp_manager.register_tools(&mut app.tool_registry);
    }

    // First-run check: show guide if no provider configured
    // (skip for commands that don't need a provider)
    if !matches!(
        cli.command,
        Some(Commands::Init)
            | Some(Commands::Doctor)
            | Some(Commands::Info)
            | Some(Commands::Status)
    ) {
        let setup_status = nexus_code::setup::diagnose();
        if !setup_status.has_any_provider {
            nexus_code::setup::print_first_run_guide(&setup_status);
            std::process::exit(0);
        }
    }

    match cli.command {
        Some(Commands::Chat { ref prompt }) if !prompt.is_empty() => {
            // Headless mode: prompt provided as positional args
            let prompt_str = prompt.join(" ");
            run_headless(
                &mut app,
                &prompt_str,
                cli.auto_approve,
                cli.dangerously_approve_all,
            )
            .await?;
        }
        Some(Commands::Init) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            nexus_code::setup::init_nexuscode_md(&cwd)?;
        }
        Some(Commands::Doctor) => {
            let status = nexus_code::setup::diagnose();
            nexus_code::setup::print_doctor(&status);
        }
        Some(Commands::Status) => {
            app.status();
        }
        Some(Commands::Providers) => {
            app.print_providers();
        }
        Some(Commands::Info) => {
            app.info();
        }
        Some(Commands::Bench { action }) => {
            handle_bench(action, &mut app).await?;
        }
        Some(Commands::Chat { .. }) | None => {
            let app = Arc::new(Mutex::new(app));
            if cli.no_tui {
                let mut repl = ChatRepl::new(app);
                repl.run().await?;
            } else {
                nexus_code::tui::run_tui(app).await?;
            }
        }
    }

    Ok(())
}

async fn handle_bench(
    action: BenchAction,
    app: &mut App,
) -> Result<(), nexus_code::error::NxError> {
    match action {
        BenchAction::Run {
            tasks_file,
            limit,
            fuel,
            max_turns,
            workspace,
        } => {
            let tasks =
                nexus_code::bench::swe_bench::load_tasks(std::path::Path::new(&tasks_file))?;
            let workspace_dir = std::path::Path::new(&workspace);
            std::fs::create_dir_all(workspace_dir)?;

            let tasks_to_run: Vec<_> = {
                let lim = limit.unwrap_or(tasks.len());
                tasks.into_iter().take(lim).collect()
            };

            println!(
                "Running {} SWE-bench tasks with {}/{}...\n",
                tasks_to_run.len(),
                app.config.default_provider,
                app.config.default_model
            );

            let mut results = Vec::new();
            for (i, task_def) in tasks_to_run.iter().enumerate() {
                println!(
                    "[{}/{}] {} ...",
                    i + 1,
                    tasks_to_run.len(),
                    task_def.instance_id
                );

                match nexus_code::bench::swe_bench::setup_repo(task_def, workspace_dir).await {
                    Ok(repo_dir) => {
                        let result = nexus_code::bench::harness::run_task(
                            task_def,
                            &repo_dir,
                            &app.config.default_provider,
                            &app.config.default_model,
                            fuel,
                            max_turns,
                        )
                        .await;
                        let status = if result.success {
                            "\u{2713}"
                        } else {
                            "\u{2717}"
                        };
                        println!(
                            "  {} ({:.1}s, {}fu)",
                            status, result.time_secs, result.fuel_consumed
                        );
                        results.push(result);
                    }
                    Err(e) => {
                        println!("  \u{2717} Setup failed: {}", e);
                        results.push(nexus_code::bench::TaskResult {
                            task_id: task_def.instance_id.clone(),
                            success: false,
                            patch: String::new(),
                            turns: 0,
                            fuel_consumed: 0,
                            time_secs: 0.0,
                            tools_used: Vec::new(),
                            audit_entries: 0,
                            error: Some(format!("{}", e)),
                        });
                    }
                }
            }

            let report = nexus_code::bench::report::generate_report(
                &results,
                &app.config.default_provider,
                &app.config.default_model,
            );
            println!("{}", nexus_code::bench::report::format_report(&report));
            nexus_code::bench::report::save_report(
                &report,
                std::path::Path::new("nx-bench-report.json"),
            )?;
            println!("Report saved to nx-bench-report.json");
        }
        BenchAction::Compare {
            tasks_file,
            providers,
            limit,
            fuel,
            max_turns,
            workspace,
        } => {
            let tasks =
                nexus_code::bench::swe_bench::load_tasks(std::path::Path::new(&tasks_file))?;
            let tasks: Vec<_> = tasks.into_iter().take(limit).collect();
            let workspace_dir = std::path::Path::new(&workspace);
            std::fs::create_dir_all(workspace_dir)?;

            let mut reports = Vec::new();
            for provider_spec in &providers {
                let parts: Vec<&str> = provider_spec.splitn(2, '/').collect();
                let prov = parts[0];
                let model = parts.get(1).unwrap_or(&"default");

                println!("\n=== Running with {}/{} ===", prov, model);

                let mut results = Vec::new();
                for (i, task_def) in tasks.iter().enumerate() {
                    println!("[{}/{}] {} ...", i + 1, tasks.len(), task_def.instance_id);
                    match nexus_code::bench::swe_bench::setup_repo(task_def, workspace_dir).await {
                        Ok(repo_dir) => {
                            let result = nexus_code::bench::harness::run_task(
                                task_def, &repo_dir, prov, model, fuel, max_turns,
                            )
                            .await;
                            let status = if result.success {
                                "\u{2713}"
                            } else {
                                "\u{2717}"
                            };
                            println!(
                                "  {} ({:.1}s, {}fu)",
                                status, result.time_secs, result.fuel_consumed
                            );
                            results.push(result);
                        }
                        Err(e) => {
                            println!("  \u{2717} Setup failed: {}", e);
                            results.push(nexus_code::bench::TaskResult {
                                task_id: task_def.instance_id.clone(),
                                success: false,
                                patch: String::new(),
                                turns: 0,
                                fuel_consumed: 0,
                                time_secs: 0.0,
                                tools_used: Vec::new(),
                                audit_entries: 0,
                                error: Some(format!("{}", e)),
                            });
                        }
                    }
                }
                reports.push(nexus_code::bench::report::generate_report(
                    &results, prov, model,
                ));
            }

            println!("{}", nexus_code::bench::report::format_comparison(&reports));
        }
        BenchAction::Report { file } => {
            let content = std::fs::read_to_string(&file)?;
            let report: nexus_code::bench::BenchmarkReport = serde_json::from_str(&content)
                .map_err(|e| {
                    nexus_code::error::NxError::ConfigError(format!("Parse report: {}", e))
                })?;
            println!("{}", nexus_code::bench::report::format_report(&report));
        }
        BenchAction::Paper {
            reports,
            output,
            latex,
        } => {
            let mut benchmark_reports = Vec::new();
            for path_str in reports.split(',') {
                let path_str = path_str.trim();
                if path_str.is_empty() {
                    continue;
                }
                let content = std::fs::read_to_string(path_str)?;
                let report: nexus_code::bench::BenchmarkReport = serde_json::from_str(&content)
                    .map_err(|e| {
                        nexus_code::error::NxError::ConfigError(format!(
                            "Parse {}: {}",
                            path_str, e
                        ))
                    })?;
                benchmark_reports.push(report);
            }

            let package = nexus_code::bench::data_pipeline::PaperDataPackage::from_reports(
                &benchmark_reports,
            );
            package.save(std::path::Path::new(&output))?;
            println!("Paper data saved to {}", output);

            if latex {
                println!("\n{}", package.to_latex());
            }
        }
    }
    Ok(())
}
