/// Run the computer use agent loop on a task
///
/// Usage:
///   nx-agent "click the terminal and type hello"
///   nx-agent --auto "take a screenshot and describe what you see"
///   nx-agent --max-steps 5 "find the file manager"
///   nx-agent --dry-run "click the browser" (show plan without executing)
use nexus_computer_use::agent::loop_controller::{run_agent_loop, AgentConfig};

fn print_usage() {
    eprintln!("Usage: nx-agent [OPTIONS] <TASK>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --auto           Run without user approval (headless mode)");
    eprintln!("  --max-steps N    Maximum steps (default: 20, max: 100)");
    eprintln!("  --dry-run        Show plans without executing actions");
    eprintln!("  --threshold N    Confidence threshold 0.0-1.0 (default: 0.3)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  nx-agent \"click the terminal and type hello\"");
    eprintln!("  nx-agent --auto \"take a screenshot and describe what you see\"");
    eprintln!("  nx-agent --max-steps 5 \"find the file manager\"");
    eprintln!("  nx-agent --dry-run \"click the browser\"");
}

fn parse_args() -> Result<AgentConfig, String> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Err("No task specified".into());
    }

    let mut config = AgentConfig::default();
    let mut i = 1;
    let mut task_parts: Vec<String> = Vec::new();

    while i < args.len() {
        match args[i].as_str() {
            "--auto" => {
                config.require_user_approval = false;
                i += 1;
            }
            "--dry-run" => {
                config.dry_run = true;
                i += 1;
            }
            "--max-steps" => {
                if i + 1 >= args.len() {
                    return Err("--max-steps requires a value".into());
                }
                config.max_steps = args[i + 1]
                    .parse::<u32>()
                    .map_err(|e| format!("Invalid --max-steps value: {e}"))?;
                if config.max_steps == 0 {
                    return Err("--max-steps must be > 0".into());
                }
                if config.max_steps > 100 {
                    eprintln!("Warning: --max-steps clamped to 100");
                    config.max_steps = 100;
                }
                i += 2;
            }
            "--threshold" => {
                if i + 1 >= args.len() {
                    return Err("--threshold requires a value".into());
                }
                config.confidence_threshold = args[i + 1]
                    .parse::<f64>()
                    .map_err(|e| format!("Invalid --threshold value: {e}"))?;
                if !(0.0..=1.0).contains(&config.confidence_threshold) {
                    return Err("--threshold must be between 0.0 and 1.0".into());
                }
                i += 2;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                if other.starts_with("--") {
                    return Err(format!("Unknown option: {other}"));
                }
                task_parts.push(other.to_string());
                i += 1;
            }
        }
    }

    if task_parts.is_empty() {
        print_usage();
        return Err("No task specified".into());
    }

    config.task = task_parts.join(" ");
    Ok(config)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("=== Nexus Computer Use Agent ===\n");

    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    println!("Task:      {}", config.task);
    println!("Max steps: {}", config.max_steps);
    println!(
        "Approval:  {}",
        if config.require_user_approval {
            "manual"
        } else {
            "auto"
        }
    );
    println!("Dry run:   {}", config.dry_run);
    println!();

    match run_agent_loop(config).await {
        Ok(result) => {
            println!("\n=== Agent Run Complete ===");
            println!("Completed:     {}", result.completed);
            println!("Summary:       {}", result.summary);
            println!("Steps:         {}", result.steps_executed);
            println!("Fuel consumed: {}", result.fuel_consumed);
            println!("Duration:      {}ms", result.total_duration_ms);
            println!("Audit hash:    {}", result.audit_hash);

            if !result.completed {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("\nAgent error: {e}");
            std::process::exit(1);
        }
    }
}
