/// Test learning and pattern system
///
/// Usage:
///   nx-learn patterns          — list learned patterns
///   nx-learn memory            — show action memory stats
///   nx-learn match "task"      — find matching patterns for a task
///   nx-learn optimize          — run optimizer on all patterns
///   nx-learn reset             — clear all patterns and memory
///   nx-learn stats             — show learning statistics
use nexus_computer_use::learning::memory::ActionMemory;
use nexus_computer_use::learning::optimizer::PatternOptimizer;
use nexus_computer_use::learning::pattern::PatternLibrary;

fn print_usage() {
    eprintln!("nx-learn — Nexus OS Self-Improving UI Pattern System");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  nx-learn patterns          List learned patterns");
    eprintln!("  nx-learn memory            Show action memory stats");
    eprintln!("  nx-learn match \"task\"       Find matching patterns for a task");
    eprintln!("  nx-learn optimize          Run optimizer on all patterns");
    eprintln!("  nx-learn reset             Clear all patterns and memory");
    eprintln!("  nx-learn stats             Show learning statistics");
}

fn cmd_patterns(library: &PatternLibrary) {
    let patterns = library.patterns();
    if patterns.is_empty() {
        println!("No patterns learned yet.");
        return;
    }
    println!("Learned UI Patterns ({} total):", patterns.len());
    println!("{:-<80}", "");
    for p in patterns {
        println!(
            "  [{id}] {name} (v{ver})",
            id = &p.id[..8],
            name = p.name,
            ver = p.version,
        );
        println!("    Trigger:    {}", p.trigger);
        println!("    App:        {}", p.app_context);
        println!(
            "    Confidence: {:.0}% ({}/{} uses)",
            p.confidence * 100.0,
            p.success_count,
            p.success_count + p.failure_count,
        );
        println!("    Actions:    {} steps", p.actions.len());
        println!("    Avg time:   {}ms", p.avg_duration_ms);
        println!();
    }
}

fn cmd_memory(memory: &ActionMemory) {
    println!("Action Memory Statistics:");
    println!("{:-<40}", "");
    println!("  Entries:       {}", memory.len());
    println!("  Total actions: {}", memory.total_actions());
    println!("  Total fuel:    {}", memory.total_fuel());
    if !memory.is_empty() {
        let entries = memory.entries();
        let successes = entries.iter().filter(|e| e.success).count();
        println!(
            "  Success rate:  {:.0}%",
            successes as f64 / entries.len() as f64 * 100.0
        );
        if let Some(last) = entries.last() {
            println!(
                "  Last run:      {} — \"{}\"",
                last.timestamp.format("%Y-%m-%d %H:%M"),
                last.task,
            );
        }
    }
}

fn cmd_match(library: &PatternLibrary, task: &str) {
    let matches = library.find_matching(task);
    if matches.is_empty() {
        println!("No matching patterns for: \"{task}\"");
        return;
    }
    println!("Matches for \"{task}\":");
    for m in &matches {
        println!(
            "  [{id}] {name} — score: {score:.0}%, confidence: {conf:.0}%",
            id = &m.pattern.id[..8],
            name = m.pattern.name,
            score = m.score * 100.0,
            conf = m.pattern.confidence * 100.0,
        );
    }
}

fn cmd_optimize(optimizer: &mut PatternOptimizer) {
    let pattern_ids: Vec<String> = optimizer
        .library()
        .patterns()
        .iter()
        .map(|p| p.id.clone())
        .collect();

    if pattern_ids.is_empty() {
        println!("No patterns to optimize.");
        return;
    }

    let mut applied = 0;
    for pid in &pattern_ids {
        if let Some(result) = optimizer.optimize_pattern(pid) {
            println!(
                "  Optimization: {} for pattern {}",
                result.optimization_type,
                &result.pattern_id[..8],
            );
            println!("    Before: {} actions", result.before.len());
            println!("    After:  {} actions", result.after.len());
            println!("    Improvement: {}", result.expected_improvement);

            match optimizer.apply_optimization(result) {
                Ok(()) => {
                    println!("    Status: APPLIED");
                    applied += 1;
                }
                Err(e) => {
                    println!("    Status: REJECTED — {e}");
                }
            }
        }
    }
    println!("\nOptimized {applied}/{} patterns.", pattern_ids.len());
}

fn cmd_reset(library: &mut PatternLibrary, memory: &mut ActionMemory) {
    // Clear in-memory state and save empty files
    *library = PatternLibrary::with_default_path();
    *memory = ActionMemory::with_default_path();
    match library.save() {
        Ok(()) => println!("Patterns cleared."),
        Err(e) => eprintln!("Failed to clear patterns: {e}"),
    }
    match memory.save() {
        Ok(()) => println!("Memory cleared."),
        Err(e) => eprintln!("Failed to clear memory: {e}"),
    }
}

fn cmd_stats(library: &PatternLibrary, memory: &ActionMemory) {
    println!("=== Nexus OS Learning Statistics ===");
    println!();
    println!("Patterns:       {}", library.len());
    println!("Memory entries: {}", memory.len());
    println!("Total actions:  {}", memory.total_actions());
    println!("Total fuel:     {}", memory.total_fuel());

    if !library.is_empty() {
        let patterns = library.patterns();
        let avg_conf: f32 =
            patterns.iter().map(|p| p.confidence).sum::<f32>() / patterns.len() as f32;
        let total_uses: u32 = patterns.iter().map(|p| p.total_uses()).sum();
        println!("Avg confidence: {:.0}%", avg_conf * 100.0);
        println!("Total pattern uses: {total_uses}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let mut library = PatternLibrary::with_default_path();
    if let Err(e) = library.load() {
        eprintln!("Warning: failed to load patterns: {e}");
    }

    let mut memory = ActionMemory::with_default_path();
    if let Err(e) = memory.load() {
        eprintln!("Warning: failed to load memory: {e}");
    }

    match args[1].as_str() {
        "patterns" => cmd_patterns(&library),
        "memory" => cmd_memory(&memory),
        "match" => {
            if args.len() < 3 {
                eprintln!("Usage: nx-learn match \"task description\"");
                std::process::exit(1);
            }
            cmd_match(&library, &args[2]);
        }
        "optimize" => {
            let mut optimizer = PatternOptimizer::new(memory, library);
            cmd_optimize(&mut optimizer);
            if let Err(e) = optimizer.library().save() {
                eprintln!("Failed to save patterns: {e}");
            }
            // memory and library moved into optimizer, nothing left to do
        }
        "reset" => cmd_reset(&mut library, &mut memory),
        "stats" => cmd_stats(&library, &memory),
        "--help" | "-h" | "help" => print_usage(),
        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
            std::process::exit(1);
        }
    }
}
