//! Runs all 54 agents through the full 4-vector × 5-level capability measurement
//! battery using real NVIDIA NIM API calls.
//!
//! Usage:
//!   GROQ_API_KEY=nvapi-xxx \
//!     cargo run -p nexus-conductor-benchmark --bin real-battery-validation --release

use nexus_capability_measurement::evaluation::validation_run::{
    discover_agents, execute_validation_run_real, save_validation_run, ValidationRunConfig,
};

fn main() {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║   NEXUS OS — Real Battery Validation (LLM-as-Judge)        ║");
    eprintln!("║   Groq Llama 3.1 8B (agent) + Llama 3.3 70B (judge)       ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝\n");

    // Check API key
    if std::env::var("GROQ_API_KEY").is_err() {
        eprintln!("ERROR: Set GROQ_API_KEY environment variable");
        std::process::exit(1);
    }

    // Load battery
    let battery = nexus_capability_measurement::battery::test_problem::load_battery(
        "crates/nexus-capability-measurement/data/battery_v1.json",
    )
    .expect("Failed to load battery");
    eprintln!("  Battery: {} locked problems", battery.len());

    // Discover agents
    let agents_dir = std::path::Path::new("agents/prebuilt");
    let agents = discover_agents(agents_dir);
    eprintln!("  Agents: {}", agents.len());

    // Probe agent model
    eprint!("  Probing agent model (Llama 3.1 8B)... ");
    let api_key = std::env::var("GROQ_API_KEY").unwrap();
    let nim = nexus_capability_measurement::evaluation::nim_client::NimClient::new(
        api_key.clone(),
        "llama-3.1-8b-instant".into(),
    );
    match nim.query("You are a test.", "Say OK.", 10) {
        Ok(r) => eprintln!("OK ({})", r.trim()),
        Err(e) => {
            eprintln!("FAILED: {e}");
            std::process::exit(1);
        }
    }

    // Probe judge model
    eprint!("  Probing judge model (Llama 3.3 70B)... ");
    let judge = nexus_capability_measurement::evaluation::nim_client::NimClient::new(
        api_key,
        "llama-3.3-70b-versatile".into(),
    );
    match judge.query("You are a test.", "Say OK.", 10) {
        Ok(r) => eprintln!("OK ({})", r.trim()),
        Err(e) => {
            eprintln!("FAILED: {e}");
            std::process::exit(1);
        }
    }

    // Run
    let config = ValidationRunConfig {
        run_label: "real-battery-llm-judge".into(),
        agent_ids: Vec::new(),
        enable_routing: true,
        staging_threshold: 0.95,
        agent_timeout_secs: 120,
    };

    eprintln!(
        "\n═══ Running: {} agents × {} problems × 2 runs ═══\n",
        agents.len(),
        battery.len()
    );

    let output =
        execute_validation_run_real(&battery, &config, agents_dir).expect("Validation run failed");

    // Print results
    let agg = &output.ab_result.aggregate;
    let baseline_count = output.ab_result.baseline.sessions.len();
    let routed_count = output.ab_result.routed.sessions.len();
    eprintln!("\n═══ Results (LLM-as-Judge) ═══\n");
    eprintln!("  Baseline sessions: {}", baseline_count);
    eprintln!("  Routed sessions: {}", routed_count);
    eprintln!("  Agents evaluated: {}", agg.agents_evaluated);
    eprintln!("  Avg routing delta: {:+.4}", agg.avg_composite_delta);
    eprintln!(
        "  Improved/Unchanged/Degraded: {}/{}/{}",
        agg.agents_improved, agg.agents_unchanged, agg.agents_degraded
    );

    let bc = &agg.baseline_census;
    eprintln!(
        "\n  Baseline census: bal={} theo={} proc={} rigid={} pat={} anom={}",
        bc.balanced,
        bc.theoretical_reasoner,
        bc.procedural_executor,
        bc.rigid_tool_user,
        bc.pattern_matching,
        bc.anomalous
    );

    for va in &agg.vector_aggregates {
        eprintln!(
            "  {:?}: base={:.3} route={:.3} delta={:+.4}",
            va.vector, va.avg_baseline, va.avg_routed, va.avg_delta
        );
    }

    let cal = &output.ab_result.baseline.calibration;
    eprintln!(
        "\n  Calibration: {} ({} inversions)",
        if cal.is_calibrated {
            "OK"
        } else {
            "INVERSIONS"
        },
        cal.inversions.len()
    );

    let gr = &output.ab_result.baseline.gaming_report;
    eprintln!(
        "  Gaming: {} flags (R{} O{} Y{}) in {} agents",
        gr.total_flags, gr.red_count, gr.orange_count, gr.yellow_count, gr.agents_with_flags
    );

    eprintln!("  API calls: {} total", output.api_calls.total_calls);
    for (model, count) in &output.api_calls.calls_by_model {
        eprintln!("    {model}: {count}");
    }
    eprintln!("  Duration: {}s", output.total_duration_secs);

    // Save
    let dir = std::path::Path::new("data/validation_runs");
    match save_validation_run(&output, dir) {
        Ok(path) => eprintln!("\n  Saved: {}", path.display()),
        Err(e) => eprintln!("\n  Save failed: {e}"),
    }

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  COMPLETE — {}s | {} agents | {} API calls{:>16}║",
        output.total_duration_secs, output.agents_evaluated, output.api_calls.total_calls, "",
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}
