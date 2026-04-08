//! Phase 1.4 Deliverable 8 — autonomous scout CLI.
//!
//! Thin clap-based entry point over `nexus_ui_repair::driver::Driver`.
//! The default mode is `--dry-run` to make accidental invocation safe:
//! walk the state machine and write an audit log, but do not call any
//! LLM provider or touch the cost ceiling.
//!
//! Real runs require `--no-dry-run` and an explicit `--pages` list.

use std::path::PathBuf;

use clap::Parser;
use nexus_ui_repair::driver::{Driver, DriverConfig, PageWorkItem};

#[derive(Debug, Parser)]
#[command(
    name = "nexus-ui-repair",
    version,
    about = "Nexus OS autonomous UI scout"
)]
struct Cli {
    /// Override the session audit log path.
    #[arg(long)]
    audit_path: Option<PathBuf>,

    /// Override the cost ceiling persistence file path.
    #[arg(long)]
    cost_path: Option<PathBuf>,

    /// Override the heartbeat file path.
    #[arg(long)]
    heartbeat_path: Option<PathBuf>,

    /// Override the calibration log path.
    #[arg(long)]
    calibration_path: Option<PathBuf>,

    /// Heartbeat tick interval in milliseconds.
    #[arg(long, default_value_t = 1000)]
    heartbeat_interval_ms: u64,

    /// Cost ceiling in USD.
    #[arg(long, default_value_t = nexus_ui_repair::governance::cost_ceiling::DEFAULT_CEILING_USD)]
    cost_ceiling_usd: f64,

    /// Dry-run: walk the state machine but call no LLM providers.
    /// Default is ON — pass `--no-dry-run` for a real run.
    #[arg(long, default_value_t = true, overrides_with = "no_dry_run")]
    dry_run: bool,

    /// Disable dry-run. Real LLM calls will be made.
    #[arg(long, default_value_t = false)]
    no_dry_run: bool,

    /// Pages to exercise. Repeatable. Format: `ROUTE:elem1,elem2,elem3`.
    #[arg(long = "page")]
    pages: Vec<String>,
}

fn parse_page(spec: &str) -> Result<PageWorkItem, String> {
    let (route, elems) = spec
        .split_once(':')
        .ok_or_else(|| format!("bad --page spec (missing ':'): {spec}"))?;
    let elements = elems
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    Ok(PageWorkItem {
        page: route.to_string(),
        elements,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber_init();

    let cli = Cli::parse();

    let mut config = DriverConfig::default_at_home();
    if let Some(p) = cli.audit_path {
        config.audit_path = p;
    }
    if let Some(p) = cli.cost_path {
        config.cost_ceiling_path = p;
    }
    if let Some(p) = cli.heartbeat_path {
        config.heartbeat_path = p;
    }
    if let Some(p) = cli.calibration_path {
        config.calibration_path = p;
    }
    config.heartbeat_interval_ms = cli.heartbeat_interval_ms;
    config.cost_ceiling_usd = cli.cost_ceiling_usd;
    config.dry_run = cli.dry_run && !cli.no_dry_run;

    let work: Vec<PageWorkItem> = cli
        .pages
        .iter()
        .map(|s| parse_page(s))
        .collect::<Result<_, _>>()?;

    if work.is_empty() {
        eprintln!("no --page work items provided; nothing to do");
        return Ok(());
    }

    let mut driver = Driver::new(config)?;
    driver.start_heartbeat()?;
    let outcome = driver.run(work).await?;
    driver.shutdown_heartbeat().await;

    if let Some(halt) = &outcome.halt {
        eprintln!(
            "HALTED at page {} element {}: {} ({})",
            halt.page, halt.element, halt.reason, halt.error_kind
        );
        eprintln!(
            "partial outcome: pages={} elements={} vision_calls={} classifications={}",
            outcome.pages_visited,
            outcome.elements_visited,
            outcome.vision_calls,
            outcome.classifications.len()
        );
        std::process::exit(2);
    }

    println!(
        "done: pages={} elements={} vision_calls={} classifications={}",
        outcome.pages_visited,
        outcome.elements_visited,
        outcome.vision_calls,
        outcome.classifications.len()
    );
    Ok(())
}

fn tracing_subscriber_init() {
    // Opt-in minimal init; avoid adding a new workspace dep just for
    // subscriber setup. If the default tracing subscriber is not
    // installed, tracing events become no-ops, which is fine.
}
