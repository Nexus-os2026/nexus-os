//! Phase 1.5 Group C — scout binary.
//!
//! Runs the live AT-SPI enumerator against a running Nexus OS instance
//! and emits a repair-ticket file (or a dry-run JSON dump) plus an
//! optional comparison against a ground-truth doc.
//!
//! Usage:
//!   scout --dry-run
//!   scout --output docs/qa/scout-runs/chat-phase-1-5-run-1.json
//!   scout --dry-run --compare docs/qa/chat_page_ground_truth_v1.md
//!
//! This binary is read-only by design — it never mutates Nexus OS
//! source. It is the Phase 1.5 entry point that replaces the
//! Phase 1.4 fixture-driven harness for live runs.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use nexus_ui_repair::comparison::harness::{compare, ComparisonReport, ScoutFinding};
use nexus_ui_repair::ground_truth::parser::parse_ground_truth_file;
use nexus_ui_repair::repair_ticket::schema::{
    ComparisonSummary, DomContext, FixCategory, RepairTicket, Severity, TicketFile,
};
use nexus_ui_repair::specialists::element::InteractiveElement;
use nexus_ui_repair::specialists::live_enumerator::{
    enumerate_live_with_options, LiveTargetConfig,
};
use nexus_ui_repair::VERSION as SCOUT_VERSION;

#[derive(Debug, Parser)]
#[command(
    name = "scout",
    about = "Nexus OS UI repair scout — live AT-SPI enumerator + ticket writer"
)]
struct Args {
    /// Print the ticket JSON to stdout instead of writing it to disk.
    #[arg(long)]
    dry_run: bool,

    /// Destination path for the ticket file. Required when --dry-run
    /// is not set.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Optional ground-truth markdown file to compare findings against.
    #[arg(long)]
    compare: Option<PathBuf>,

    /// Dump every visited AT-SPI node to stderr in a one-line-per-node
    /// format. Diagnostic-only, used to trace BFS termination when the
    /// walker is yielding zero interactive elements.
    #[arg(long)]
    dump_walk: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("scout FAIL: failed to build tokio runtime: {e}");
            return ExitCode::from(1);
        }
    };

    let target = LiveTargetConfig::for_nexus_chat();
    eprintln!(
        "scout: enumerating live AT-SPI tree (app_name={:?}, page_route={:?})",
        target.app_name, target.page_route
    );

    let enumeration = match rt.block_on(enumerate_live_with_options(&target, args.dump_walk)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("scout FAIL: live enumeration failed: {e}");
            return ExitCode::from(2);
        }
    };

    eprintln!(
        "scout: walked {} AT-SPI nodes, collected {} interactive elements",
        enumeration.nodes_visited,
        enumeration.elements.len()
    );

    let elements = enumeration.elements;

    // Role histogram.
    let mut role_counts: BTreeMap<String, usize> = BTreeMap::new();
    for el in &elements {
        *role_counts.entry(el.role.clone()).or_insert(0) += 1;
    }

    // Key-element probes used by the Phase 1.5 Group C halt gate.
    let has_message_input = elements.iter().any(|el| {
        el.role == "embedded" && el.accessible_name.to_lowercase().starts_with("message ")
    });
    let chat_tab_labels = ["+ New", "⌁ Chat", "⇔ Compare", "History"];
    let present_chat_tabs: Vec<&str> = chat_tab_labels
        .iter()
        .copied()
        .filter(|label| {
            elements
                .iter()
                .any(|el| el.accessible_name.trim() == *label)
        })
        .collect();
    let has_start_jarvis = elements.iter().any(|el| {
        el.accessible_name
            .trim()
            .eq_ignore_ascii_case("START JARVIS")
    });
    let has_refresh = elements
        .iter()
        .any(|el| el.accessible_name.trim().eq_ignore_ascii_case("REFRESH"));

    // Build one unknown_new ScoutFinding per element. The scout has
    // no mapping from elements to GT-NNN ids yet — that mapping is
    // Group D work — so every finding goes into the unknown_new
    // bucket at this stage. The comparison harness reports the
    // ground-truth miss set accordingly.
    let findings: Vec<ScoutFinding> = elements
        .iter()
        .map(|el| ScoutFinding {
            matched_gt_id: None,
            label: el.element_id.clone(),
        })
        .collect();

    // Comparison (optional).
    let comparison_report: Option<ComparisonReport> = if let Some(gt_path) = args.compare.as_ref() {
        match parse_ground_truth_file(gt_path) {
            Ok(parsed) => {
                for w in &parsed.warnings {
                    eprintln!("scout: ground-truth warning: {w}");
                }
                Some(compare(&findings, &parsed.entries))
            }
            Err(e) => {
                eprintln!("scout FAIL: ground-truth parse error: {e}");
                return ExitCode::from(3);
            }
        }
    } else {
        None
    };

    // Build per-element repair tickets (unknown_new SCOUT-NNN entries).
    let tickets: Vec<RepairTicket> = elements
        .iter()
        .enumerate()
        .map(|(i, el)| element_to_ticket(i, el))
        .collect();

    let comparison_summary = match comparison_report.as_ref() {
        Some(report) => ComparisonSummary {
            confirmed_match_count: report.confirmed_match.len(),
            unknown_new_count: report.unknown_new.len(),
            confirmed_miss_count: report.confirmed_miss.len(),
            f1_score: report.f1_score,
            human_triage_required: report.unknown_new.len() + report.confirmed_miss.len(),
            is_partial: false,
        },
        None => ComparisonSummary {
            confirmed_match_count: 0,
            unknown_new_count: tickets.len(),
            confirmed_miss_count: 0,
            f1_score: None,
            human_triage_required: tickets.len(),
            is_partial: true,
        },
    };

    let ticket_file = TicketFile {
        schema_version: "1.0.0".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        scout_version: SCOUT_VERSION.to_string(),
        page: "chat".to_string(),
        tickets,
        halt: None,
        comparison_summary,
    };

    let json = match serde_json::to_string_pretty(&ticket_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("scout FAIL: serialize ticket file: {e}");
            return ExitCode::from(4);
        }
    };

    // Structured stdout summary (human-readable).
    println!("=== scout summary ===");
    println!("enumerated_elements: {}", elements.len());
    println!("nodes_visited: {}", enumeration.nodes_visited);
    println!("role_counts:");
    for (role, count) in &role_counts {
        println!("  {role}: {count}");
    }
    println!("probes:");
    println!("  chat_message_input_embedded: {has_message_input}");
    println!(
        "  chat_tabs_present: {}/{} ({:?})",
        present_chat_tabs.len(),
        chat_tab_labels.len(),
        present_chat_tabs
    );
    println!("  start_jarvis: {has_start_jarvis}");
    println!("  refresh: {has_refresh}");
    if let Some(report) = comparison_report.as_ref() {
        println!("comparison:");
        println!("  confirmed_match: {}", report.confirmed_match.len());
        println!("  unknown_new: {}", report.unknown_new.len());
        println!("  confirmed_miss: {}", report.confirmed_miss.len());
        println!(
            "  f1_score: {}",
            report
                .f1_score
                .map(|f| format!("{f:.4}"))
                .unwrap_or_else(|| "none".to_string())
        );
        println!("  missed_gt_ids: {:?}", report.confirmed_miss);
    }

    if args.dry_run {
        println!("=== ticket_file_json ===");
        println!("{json}");
        eprintln!("scout: dry run complete — no file written");
        return ExitCode::SUCCESS;
    }

    let output = match args.output.as_ref() {
        Some(p) => p.clone(),
        None => {
            eprintln!("scout FAIL: --output is required when --dry-run is not set");
            return ExitCode::from(5);
        }
    };

    if let Some(parent) = output.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("scout FAIL: create output dir {parent:?}: {e}");
            return ExitCode::from(6);
        }
    }
    if let Err(e) = std::fs::write(&output, &json) {
        eprintln!("scout FAIL: write output {output:?}: {e}");
        return ExitCode::from(7);
    }
    eprintln!("scout: wrote ticket file to {output:?}");
    ExitCode::SUCCESS
}

fn element_to_ticket(index: usize, el: &InteractiveElement) -> RepairTicket {
    RepairTicket {
        id: format!("SCOUT-{index:03}"),
        page: "chat".to_string(),
        sub_view: String::new(),
        severity: Severity::Low,
        dom_context: DomContext {
            selector: el.element_id.clone(),
            surrounding_markup: format!(
                "role={:?} name={:?} description={:?}",
                el.role, el.accessible_name, el.description
            ),
        },
        error_strings: Vec::new(),
        screenshot_path: None,
        suggested_fix_category: FixCategory::UxPolish,
        component_file_hint: "app/src/pages/Chat.tsx".to_string(),
        reproduction_steps: Vec::new(),
    }
}
