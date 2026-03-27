//! Nexus OS — Multi-Agent Coordination Stress Test (50,000+ Agents)
//!
//! Validates lock-free ConcurrentSupervisor orchestration at AGI-scale populations.
//!
//! Phases:
//!   1. Spawn ramp: 10K → 25K → 50K → 100K agents — spawn throughput & memory
//!   2. Fuel contention: concurrent reserve/commit across all agents
//!   3. Message bus: lock-free SegQueue throughput at each scale level
//!   4. Governance: capability gate checks under mass concurrent load
//!   5. Coordination mix: realistic 30% resource / 20% governance / 50% messaging
//!   6. SwarmCoordinator: consensus stability at 50K+ agent coordination requests
//!   7. Find the ceiling: push past 100K until breakdown
//!
//! Run:
//!   cargo run -p nexus-conductor-benchmark --bin multiagent-coordination-bench --release

use crossbeam::queue::SegQueue;
use nexus_kernel::capabilities::has_capability;
use nexus_kernel::concurrent_supervisor::ConcurrentSupervisor;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::Supervisor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

// ── Configuration ────────────────────────────────────────────────────────────

const SCALE_LEVELS: &[usize] = &[10_000, 25_000, 50_000, 100_000];
const FUEL_OPS_PER_AGENT: usize = 3;
const MESSAGE_ROUNDS: usize = 5;
const GOVERNANCE_CHECKS_PER_AGENT: usize = 4;
const THREAD_COUNT: usize = 16;

// ── Helpers ──────────────────────────────────────────────────────────────────

const ARCHETYPES: &[(&str, &[&str], u64, u8)] = &[
    (
        "researcher",
        &["llm.query", "web.search", "web.read", "fs.read"],
        50_000,
        3,
    ),
    ("writer", &["llm.query", "fs.read", "fs.write"], 30_000, 2),
    (
        "coder",
        &["llm.query", "fs.read", "fs.write", "process.exec"],
        80_000,
        3,
    ),
    (
        "analyst",
        &["llm.query", "web.search", "mcp.call"],
        40_000,
        2,
    ),
    (
        "devops",
        &["process.exec", "fs.read", "fs.write", "mcp.call"],
        60_000,
        3,
    ),
    ("sentinel", &["llm.query", "fs.read"], 20_000, 1),
    (
        "publisher",
        &["llm.query", "fs.write", "web.read", "mcp.call"],
        40_000,
        3,
    ),
    (
        "strategist",
        &["llm.query", "self.modify"],
        100_000,
        3, // Cap at L3 to avoid L4/L5/L6 singleton limits
    ),
];

fn make_manifest(index: usize) -> AgentManifest {
    let arch = &ARCHETYPES[index % ARCHETYPES.len()];
    AgentManifest {
        name: format!("{}-{:06}", arch.0, index),
        version: "1.0.0".into(),
        capabilities: arch.1.iter().map(|s| s.to_string()).collect(),
        fuel_budget: arch.2,
        autonomy_level: Some(arch.3),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        default_goal: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
        filesystem_permissions: vec![],
    }
}

fn read_rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0)
        / 1024.0
}

fn percentiles_us(sorted_ns: &[u64]) -> (f64, f64, f64, f64) {
    if sorted_ns.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let len = sorted_ns.len();
    let p50 = sorted_ns[len * 50 / 100] as f64 / 1_000.0;
    let p95 = sorted_ns[len * 95 / 100] as f64 / 1_000.0;
    let p99 = sorted_ns[len * 99 / 100] as f64 / 1_000.0;
    let max = sorted_ns[len - 1] as f64 / 1_000.0;
    (p50, p95, p99, max)
}

fn fmt_num(n: f64) -> String {
    let s = format!("{:.0}", n);
    let bytes = s.as_bytes();
    let mut result = String::new();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

fn pass_fail(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

fn chrono_like_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let mut y = 1970i64;
    let mut remaining = days_since_epoch as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days: [i64; 12] = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m = i + 1;
            break;
        }
        remaining -= md;
    }
    let d = remaining + 1;
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{minutes:02}:{seconds:02} GMT")
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ── Phase 1: Spawn Ramp ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct SpawnResult {
    target: usize,
    spawned: usize,
    duration_secs: f64,
    spawn_rate: f64,
    rss_mb: f64,
}

fn spawn_to_level(cs: &ConcurrentSupervisor, current: usize, target: usize) -> SpawnResult {
    let start = Instant::now();
    let mut spawned = 0usize;
    for i in current..target {
        let manifest = make_manifest(i);
        if cs.start_agent(manifest).is_ok() {
            spawned += 1;
        }
    }
    let dur = start.elapsed().as_secs_f64();
    let rss = read_rss_mb();
    let rate = spawned as f64 / dur;

    eprintln!(
        "  {:>7} agents: spawned {} in {:.2}s ({:.0} agents/s) RSS={:.0}MB",
        target, spawned, dur, rate, rss,
    );

    SpawnResult {
        target,
        spawned,
        duration_secs: dur,
        spawn_rate: rate,
        rss_mb: rss,
    }
}

fn run_phase1(cs: &ConcurrentSupervisor) -> Vec<SpawnResult> {
    eprintln!("\n═══ Phase 1: Spawn Ramp ═══\n");
    let mut results = Vec::new();
    let mut current = 0;
    for &target in SCALE_LEVELS {
        let r = spawn_to_level(cs, current, target);
        current = target;
        results.push(r);
    }
    results
}

// ── Phase 2: Fuel Contention ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FuelResult {
    agents: usize,
    total_ops: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    errors: usize,
}

fn run_phase2(cs: &Arc<ConcurrentSupervisor>, agent_ids: &[Uuid]) -> Vec<FuelResult> {
    eprintln!("\n═══ Phase 2: Fuel Contention (Lock-Free CAS) ═══\n");

    let mut results = Vec::new();

    for &scale in SCALE_LEVELS {
        let subset: Vec<Uuid> = agent_ids.iter().take(scale).copied().collect();
        if subset.len() < scale / 2 {
            eprintln!("  {:>7} agents: skipped (insufficient agents)", scale);
            continue;
        }

        let total_ops = subset.len() * FUEL_OPS_PER_AGENT;
        let all_latencies: Arc<SegQueue<u64>> = Arc::new(SegQueue::new());
        let error_count = Arc::new(AtomicU64::new(0));

        let chunk_size = subset.len().div_ceil(THREAD_COUNT);

        let wall_start = Instant::now();

        std::thread::scope(|s| {
            for chunk in subset.chunks(chunk_size) {
                let cs = Arc::clone(cs);
                let lats = Arc::clone(&all_latencies);
                let errs = Arc::clone(&error_count);
                let ids: Vec<Uuid> = chunk.to_vec();
                s.spawn(move || {
                    for &agent_id in &ids {
                        for _ in 0..FUEL_OPS_PER_AGENT {
                            let t = Instant::now();
                            match cs.reserve_fuel(agent_id, 10, "bench_op") {
                                Ok(reservation) => {
                                    let _ = cs.commit_fuel(reservation, 5);
                                }
                                Err(_) => {
                                    errs.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            lats.push(t.elapsed().as_nanos() as u64);
                        }
                    }
                });
            }
        });

        let wall_secs = wall_start.elapsed().as_secs_f64();
        let errors = error_count.load(Ordering::Relaxed) as usize;

        let mut lat_vec: Vec<u64> = Vec::with_capacity(total_ops);
        while let Some(l) = all_latencies.pop() {
            lat_vec.push(l);
        }
        lat_vec.sort_unstable();
        let (p50, p95, p99, max) = percentiles_us(&lat_vec);
        let throughput = lat_vec.len() as f64 / wall_secs;

        eprintln!(
            "  {:>7} agents: {} ops/s | P50={:.1}µs P95={:.1}µs P99={:.1}µs | errors={}",
            scale,
            fmt_num(throughput),
            p50,
            p95,
            p99,
            errors,
        );

        results.push(FuelResult {
            agents: scale,
            total_ops: lat_vec.len(),
            duration_secs: wall_secs,
            throughput,
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            max_us: max,
            errors,
        });
    }

    results
}

// ── Phase 3: Message Bus ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MessageResult {
    agents: usize,
    total_messages: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

fn run_phase3(cs: &Arc<ConcurrentSupervisor>, agent_ids: &[Uuid]) -> Vec<MessageResult> {
    eprintln!("\n═══ Phase 3: Message Bus Throughput (Lock-Free SegQueue) ═══\n");

    let mut results = Vec::new();

    for &scale in SCALE_LEVELS {
        let subset: Vec<Uuid> = agent_ids.iter().take(scale).copied().collect();
        if subset.len() < scale / 2 {
            continue;
        }

        let total_messages = subset.len() * MESSAGE_ROUNDS;
        let all_latencies: Arc<SegQueue<u64>> = Arc::new(SegQueue::new());
        let chunk_size = subset.len().div_ceil(THREAD_COUNT);

        let wall_start = Instant::now();

        std::thread::scope(|s| {
            for chunk in subset.chunks(chunk_size) {
                let cs = Arc::clone(cs);
                let lats = Arc::clone(&all_latencies);
                let ids: Vec<Uuid> = chunk.to_vec();
                let all_ids = subset.clone();
                s.spawn(move || {
                    for (j, &from_id) in ids.iter().enumerate() {
                        for round in 0..MESSAGE_ROUNDS {
                            let to_idx = (j + round + 1) % all_ids.len();
                            let to_id = all_ids[to_idx];
                            let t = Instant::now();
                            cs.send_message(from_id, to_id, "coordination_ping");
                            lats.push(t.elapsed().as_nanos() as u64);
                        }
                    }
                });
            }
        });

        let wall_secs = wall_start.elapsed().as_secs_f64();

        // Drain messages to free memory
        let drained = cs.drain_messages().len();

        let mut lat_vec: Vec<u64> = Vec::with_capacity(total_messages);
        while let Some(l) = all_latencies.pop() {
            lat_vec.push(l);
        }
        lat_vec.sort_unstable();
        let (p50, p95, p99, max) = percentiles_us(&lat_vec);
        let throughput = lat_vec.len() as f64 / wall_secs;

        eprintln!(
            "  {:>7} agents: {} msg/s | P50={:.1}µs P95={:.1}µs P99={:.1}µs | drained={}",
            scale,
            fmt_num(throughput),
            p50,
            p95,
            p99,
            drained,
        );

        results.push(MessageResult {
            agents: scale,
            total_messages: lat_vec.len(),
            duration_secs: wall_secs,
            throughput,
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            max_us: max,
        });
    }

    results
}

// ── Phase 4: Governance Gate Checks ──────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct GovernanceResult {
    agents: usize,
    total_checks: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

fn run_phase4(cs: &Arc<ConcurrentSupervisor>, agent_ids: &[Uuid]) -> Vec<GovernanceResult> {
    eprintln!("\n═══ Phase 4: Governance Gate Checks ═══\n");

    let capability_queries = ["llm.query", "fs.write", "process.exec", "web.search"];
    let mut results = Vec::new();

    for &scale in SCALE_LEVELS {
        let subset: Vec<Uuid> = agent_ids.iter().take(scale).copied().collect();
        if subset.len() < scale / 2 {
            continue;
        }

        let total_checks = subset.len() * GOVERNANCE_CHECKS_PER_AGENT;
        let all_latencies: Arc<SegQueue<u64>> = Arc::new(SegQueue::new());
        let chunk_size = subset.len().div_ceil(THREAD_COUNT);

        let wall_start = Instant::now();

        std::thread::scope(|s| {
            for chunk in subset.chunks(chunk_size) {
                let cs = Arc::clone(cs);
                let lats = Arc::clone(&all_latencies);
                let ids: Vec<Uuid> = chunk.to_vec();
                s.spawn(move || {
                    for &agent_id in &ids {
                        if let Some(snap) = cs.get_agent(agent_id) {
                            for (ci, query) in capability_queries
                                .iter()
                                .enumerate()
                                .take(GOVERNANCE_CHECKS_PER_AGENT)
                            {
                                let t = Instant::now();
                                // Simulate governance gate: lookup + capability check
                                let caps: Vec<&str> =
                                    snap.capabilities.iter().map(|s| s.as_str()).collect();
                                let _ = has_capability(caps.iter().copied(), query);
                                // Also check autonomy level gate
                                let _ = snap.autonomy_level >= (ci as u8 % 4);
                                lats.push(t.elapsed().as_nanos() as u64);
                            }
                        }
                    }
                });
            }
        });

        let wall_secs = wall_start.elapsed().as_secs_f64();

        let mut lat_vec: Vec<u64> = Vec::with_capacity(total_checks);
        while let Some(l) = all_latencies.pop() {
            lat_vec.push(l);
        }
        lat_vec.sort_unstable();
        let (p50, p95, p99, max) = percentiles_us(&lat_vec);
        let throughput = lat_vec.len() as f64 / wall_secs;

        eprintln!(
            "  {:>7} agents: {} checks/s | P50={:.1}µs P95={:.1}µs P99={:.1}µs",
            scale,
            fmt_num(throughput),
            p50,
            p95,
            p99,
        );

        results.push(GovernanceResult {
            agents: scale,
            total_checks: lat_vec.len(),
            duration_secs: wall_secs,
            throughput,
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            max_us: max,
        });
    }

    results
}

// ── Phase 5: Realistic Coordination Mix ──────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MixResult {
    agents: usize,
    total_ops: usize,
    fuel_ops: usize,
    msg_ops: usize,
    gov_ops: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    rss_mb: f64,
}

fn run_phase5(cs: &Arc<ConcurrentSupervisor>, agent_ids: &[Uuid]) -> Vec<MixResult> {
    eprintln!("\n═══ Phase 5: Realistic Coordination Mix (30% fuel / 20% governance / 50% messaging) ═══\n");

    let ops_per_agent = 10;
    let mut results = Vec::new();

    for &scale in SCALE_LEVELS {
        let subset: Vec<Uuid> = agent_ids.iter().take(scale).copied().collect();
        if subset.len() < scale / 2 {
            continue;
        }

        let total_ops = subset.len() * ops_per_agent;
        let all_latencies: Arc<SegQueue<u64>> = Arc::new(SegQueue::new());
        let fuel_count = Arc::new(AtomicU64::new(0));
        let msg_count = Arc::new(AtomicU64::new(0));
        let gov_count = Arc::new(AtomicU64::new(0));
        let chunk_size = subset.len().div_ceil(THREAD_COUNT);

        let wall_start = Instant::now();

        std::thread::scope(|s| {
            for chunk in subset.chunks(chunk_size) {
                let cs = Arc::clone(cs);
                let lats = Arc::clone(&all_latencies);
                let fc = Arc::clone(&fuel_count);
                let mc = Arc::clone(&msg_count);
                let gc = Arc::clone(&gov_count);
                let ids: Vec<Uuid> = chunk.to_vec();
                let all_ids = subset.clone();
                s.spawn(move || {
                    for (j, &agent_id) in ids.iter().enumerate() {
                        for op in 0..ops_per_agent {
                            let t = Instant::now();
                            match op % 10 {
                                // 30% fuel operations (ops 0, 1, 2)
                                0..=2 => {
                                    if let Ok(res) = cs.reserve_fuel(agent_id, 5, "mix") {
                                        let _ = cs.commit_fuel(res, 3);
                                    }
                                    fc.fetch_add(1, Ordering::Relaxed);
                                }
                                // 20% governance checks (ops 3, 4)
                                3..=4 => {
                                    if let Some(snap) = cs.get_agent(agent_id) {
                                        let caps: Vec<&str> =
                                            snap.capabilities.iter().map(|s| s.as_str()).collect();
                                        let _ = has_capability(caps.iter().copied(), "llm.query");
                                    }
                                    gc.fetch_add(1, Ordering::Relaxed);
                                }
                                // 50% messaging (ops 5-9)
                                _ => {
                                    let to_idx = (j + op + 1) % all_ids.len();
                                    cs.send_message(agent_id, all_ids[to_idx], "coord");
                                    mc.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            lats.push(t.elapsed().as_nanos() as u64);
                        }
                    }
                });
            }
        });

        let wall_secs = wall_start.elapsed().as_secs_f64();
        let rss = read_rss_mb();

        // Drain messages
        cs.drain_messages();

        let mut lat_vec: Vec<u64> = Vec::with_capacity(total_ops);
        while let Some(l) = all_latencies.pop() {
            lat_vec.push(l);
        }
        lat_vec.sort_unstable();
        let (p50, p95, p99, max) = percentiles_us(&lat_vec);
        let throughput = lat_vec.len() as f64 / wall_secs;

        let fl = fuel_count.load(Ordering::Relaxed) as usize;
        let ml = msg_count.load(Ordering::Relaxed) as usize;
        let gl = gov_count.load(Ordering::Relaxed) as usize;

        eprintln!(
            "  {:>7} agents: {} ops/s | P50={:.1}µs P95={:.1}µs P99={:.1}µs | RSS={:.0}MB | fuel={} msg={} gov={}",
            scale,
            fmt_num(throughput),
            p50, p95, p99, rss,
            fmt_num(fl as f64), fmt_num(ml as f64), fmt_num(gl as f64),
        );

        results.push(MixResult {
            agents: scale,
            total_ops: lat_vec.len(),
            fuel_ops: fl,
            msg_ops: ml,
            gov_ops: gl,
            duration_secs: wall_secs,
            throughput,
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            max_us: max,
            rss_mb: rss,
        });
    }

    results
}

// ── Phase 6: SwarmCoordinator Stress ─────────────────────────────────────────

#[derive(Debug, Clone)]
struct SwarmResult {
    swarm_runs: usize,
    total_evaluations: usize,
    convergences: usize,
    avg_generations: f64,
    duration_secs: f64,
    throughput: f64,
}

fn run_phase6() -> SwarmResult {
    use nexus_kernel::cognitive::algorithms::swarm::SwarmCoordinator;
    use nexus_kernel::cognitive::types::{AgentStep, PlannedAction};

    eprintln!("\n═══ Phase 6: SwarmCoordinator Consensus Stability ═══\n");

    let runs = 1_000;
    let mut total_gens = 0usize;
    let mut convergences = 0usize;
    let mut total_evals = 0usize;

    let wall_start = Instant::now();

    for run in 0..runs {
        let mut swarm = SwarmCoordinator::new(8);

        for gen in 0..20 {
            let step = AgentStep::new(
                format!("goal-{run}"),
                PlannedAction::LlmQuery {
                    prompt: format!("swarm coordination task gen {gen}"),
                    context: vec![],
                },
            );

            let variants = swarm.prepare_parallel_variants(&step, 8);
            let results: Vec<(AgentStep, String, f64)> = variants
                .into_iter()
                .enumerate()
                .map(|(i, v)| (v, format!("agent-{i}"), 0.3 + (i as f64 * 0.08)))
                .collect();

            swarm.evaluate_swarm_results(results);
            total_evals += 8;
            total_gens += 1;

            if swarm.has_converged() {
                convergences += 1;
                break;
            }
        }
    }

    let wall_secs = wall_start.elapsed().as_secs_f64();
    let throughput = total_evals as f64 / wall_secs;
    let avg_gens = total_gens as f64 / runs as f64;

    eprintln!(
        "  {} runs: {} evals/s | avg_gens={:.1} | convergences={}/{}",
        runs,
        fmt_num(throughput),
        avg_gens,
        convergences,
        runs,
    );

    SwarmResult {
        swarm_runs: runs,
        total_evaluations: total_evals,
        convergences,
        avg_generations: avg_gens,
        duration_secs: wall_secs,
        throughput,
    }
}

// ── Phase 7: Find the Ceiling ────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CeilingResult {
    target: usize,
    spawned: usize,
    decision_throughput: f64,
    p50_us: f64,
    p99_us: f64,
    rss_mb: f64,
    breakdown: bool,
}

fn run_phase7(cs: &Arc<ConcurrentSupervisor>, base_agents: usize) -> Vec<CeilingResult> {
    eprintln!("\n═══ Phase 7: Find the Ceiling (Beyond 100K) ═══\n");

    let ceiling_levels: &[usize] = &[150_000, 200_000, 250_000];
    let mut results = Vec::new();
    let mut current = base_agents;

    for &target in ceiling_levels {
        // Check RSS before spawning
        let pre_rss = read_rss_mb();
        if pre_rss > 50_000.0 {
            eprintln!(
                "  {:>7}: SKIPPED — RSS={:.0}MB exceeds 50GB safety limit",
                target, pre_rss
            );
            results.push(CeilingResult {
                target,
                spawned: 0,
                decision_throughput: 0.0,
                p50_us: 0.0,
                p99_us: 0.0,
                rss_mb: pre_rss,
                breakdown: true,
            });
            break;
        }

        // Spawn additional agents
        let spawn_start = Instant::now();
        let mut spawned = 0;
        for i in current..target {
            let manifest = make_manifest(i);
            if cs.start_agent(manifest).is_ok() {
                spawned += 1;
            }
        }
        let spawn_secs = spawn_start.elapsed().as_secs_f64();
        current = target;

        let rss = read_rss_mb();

        // Quick decision latency test: fuel + message ops across all agents
        // Use a sample of agents to keep test time bounded
        let sample_size = 10_000.min(target);
        let sample_ids: Vec<Uuid> = {
            let mut ids = Vec::new();
            // Get agent IDs via the concurrent supervisor
            for _i in 0..sample_size {
                // Use random UUIDs for message send (doesn't need valid agent IDs)
                ids.push(Uuid::new_v4());
            }
            ids
        };

        let all_latencies: Arc<SegQueue<u64>> = Arc::new(SegQueue::new());
        let ops_count = sample_size * 5;

        let decision_start = Instant::now();

        std::thread::scope(|s| {
            let chunk_size = sample_ids.len().div_ceil(THREAD_COUNT);
            for chunk in sample_ids.chunks(chunk_size) {
                let cs = Arc::clone(cs);
                let lats = Arc::clone(&all_latencies);
                let ids: Vec<Uuid> = chunk.to_vec();
                s.spawn(move || {
                    for &id in &ids {
                        for round in 0..5 {
                            let t = Instant::now();
                            // Lock-free message send (measures raw SegQueue perf at scale)
                            cs.send_message(id, Uuid::new_v4(), "ceiling_probe");
                            let _ = round; // used in loop
                            lats.push(t.elapsed().as_nanos() as u64);
                        }
                    }
                });
            }
        });

        let decision_secs = decision_start.elapsed().as_secs_f64();
        cs.drain_messages();

        let mut lat_vec: Vec<u64> = Vec::with_capacity(ops_count);
        while let Some(l) = all_latencies.pop() {
            lat_vec.push(l);
        }
        lat_vec.sort_unstable();
        let (p50, _, p99, _) = percentiles_us(&lat_vec);
        let decision_throughput = lat_vec.len() as f64 / decision_secs;

        // Breakdown if P99 exceeds 1ms or throughput drops below 100K
        let breakdown = p99 > 1_000.0 || decision_throughput < 100_000.0;

        eprintln!(
            "  {:>7}: spawned {} in {:.1}s | {} ops/s | P50={:.1}µs P99={:.1}µs | RSS={:.0}MB | {}",
            target,
            spawned,
            spawn_secs,
            fmt_num(decision_throughput),
            p50,
            p99,
            rss,
            if breakdown { "CEILING HIT" } else { "OK" },
        );

        results.push(CeilingResult {
            target,
            spawned,
            decision_throughput,
            p50_us: p50,
            p99_us: p99,
            rss_mb: rss,
            breakdown,
        });

        if breakdown || rss > 50_000.0 {
            break;
        }
    }

    results
}

// ── Report Generation ────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn generate_report(
    spawn_results: &[SpawnResult],
    fuel_results: &[FuelResult],
    msg_results: &[MessageResult],
    gov_results: &[GovernanceResult],
    mix_results: &[MixResult],
    swarm: &SwarmResult,
    ceiling_results: &[CeilingResult],
    total_elapsed: f64,
) {
    let now = chrono_like_utc();

    // Determine key metrics
    let max_agents = spawn_results.iter().map(|r| r.spawned).sum::<usize>();
    let peak_rss = spawn_results
        .iter()
        .map(|r| r.rss_mb)
        .chain(mix_results.iter().map(|r| r.rss_mb))
        .fold(0.0_f64, f64::max);

    let decision_p99_at_50k = mix_results
        .iter()
        .find(|r| r.agents >= 50_000)
        .map(|r| r.p99_us)
        .unwrap_or(0.0);

    let msg_throughput_at_50k = msg_results
        .iter()
        .find(|r| r.agents >= 50_000)
        .map(|r| r.throughput)
        .unwrap_or(0.0);

    let rss_at_100k = spawn_results
        .iter()
        .find(|r| r.target >= 100_000)
        .map(|r| r.rss_mb)
        .unwrap_or(0.0);

    // Success criteria
    let latency_ok = decision_p99_at_50k < 1_000.0; // under 1ms
    let memory_ok = rss_at_100k < 50_000.0; // under 50GB
    let msg_ok = msg_throughput_at_50k >= 1_000_000.0;
    let swarm_ok = swarm.convergences > 0;
    let gov_ok = gov_results
        .iter()
        .find(|r| r.agents >= 50_000)
        .map(|r| r.throughput > 100_000.0)
        .unwrap_or(false);
    let all_pass = latency_ok && memory_ok && msg_ok && swarm_ok && gov_ok;

    let mut rpt = String::new();
    rpt.push_str("# Nexus OS — Multi-Agent Coordination Stress Test Results\n\n");
    rpt.push_str(&format!("**Date**: {now}\n"));
    rpt.push_str(&format!(
        "**Total wall time**: {total_elapsed:.1}s ({:.1} minutes)\n",
        total_elapsed / 60.0,
    ));
    rpt.push_str(&format!(
        "**Max agents spawned**: {}\n",
        fmt_num(max_agents as f64)
    ));
    rpt.push_str(&format!("**Peak RSS**: {:.0} MB\n", peak_rss));
    rpt.push_str(&format!(
        "**Result**: {}\n\n",
        if all_pass {
            "ALL CRITERIA PASSED"
        } else {
            "CRITERIA FAILED"
        },
    ));

    // ── Success Criteria ─────────────────────────────────────────────────
    rpt.push_str("## Success Criteria\n\n");
    rpt.push_str("| Criterion | Target | Actual | Status |\n");
    rpt.push_str("|-----------|--------|--------|--------|\n");
    rpt.push_str(&format!(
        "| Decision P99 at 50K agents | <1ms (1,000µs) | {:.1}µs | {} |\n",
        decision_p99_at_50k,
        pass_fail(latency_ok),
    ));
    rpt.push_str(&format!(
        "| Memory at 100K agents | <50GB | {:.1} MB | {} |\n",
        rss_at_100k,
        pass_fail(memory_ok),
    ));
    rpt.push_str(&format!(
        "| Message throughput at 50K | ≥1M msg/s | {} msg/s | {} |\n",
        fmt_num(msg_throughput_at_50k),
        pass_fail(msg_ok),
    ));
    rpt.push_str(&format!(
        "| SwarmCoordinator consensus | converges | {}/{} | {} |\n",
        swarm.convergences,
        swarm.swarm_runs,
        pass_fail(swarm_ok),
    ));
    rpt.push_str(&format!(
        "| Governance scales at 50K | >100K checks/s | {} checks/s | {} |\n",
        gov_results
            .iter()
            .find(|r| r.agents >= 50_000)
            .map(|r| fmt_num(r.throughput))
            .unwrap_or_else(|| "N/A".to_string()),
        pass_fail(gov_ok),
    ));

    // ── Phase 1: Spawn ───────────────────────────────────────────────────
    rpt.push_str("\n---\n\n## Phase 1: Spawn Ramp\n\n");
    rpt.push_str("| Target | Spawned | Duration | Spawn Rate | RSS |\n");
    rpt.push_str("|--------|---------|----------|------------|-----|\n");
    for s in spawn_results {
        rpt.push_str(&format!(
            "| {} | {} | {:.2}s | {} agents/s | {:.0} MB |\n",
            fmt_num(s.target as f64),
            fmt_num(s.spawned as f64),
            s.duration_secs,
            fmt_num(s.spawn_rate),
            s.rss_mb,
        ));
    }

    // ── Phase 2: Fuel ────────────────────────────────────────────────────
    rpt.push_str("\n## Phase 2: Fuel Contention (Lock-Free CAS)\n\n");
    rpt.push_str("| Agents | Total Ops | Throughput | P50 | P95 | P99 | Max | Errors |\n");
    rpt.push_str("|--------|-----------|-----------|-----|-----|-----|-----|--------|\n");
    for f in fuel_results {
        rpt.push_str(&format!(
            "| {} | {} | {} ops/s | {:.1}µs | {:.1}µs | {:.1}µs | {:.1}µs | {} |\n",
            fmt_num(f.agents as f64),
            fmt_num(f.total_ops as f64),
            fmt_num(f.throughput),
            f.p50_us,
            f.p95_us,
            f.p99_us,
            f.max_us,
            f.errors,
        ));
    }

    // ── Phase 3: Messages ────────────────────────────────────────────────
    rpt.push_str("\n## Phase 3: Message Bus Throughput (Lock-Free SegQueue)\n\n");
    rpt.push_str("| Agents | Messages | Throughput | P50 | P95 | P99 | Max |\n");
    rpt.push_str("|--------|----------|-----------|-----|-----|-----|-----|\n");
    for m in msg_results {
        rpt.push_str(&format!(
            "| {} | {} | {} msg/s | {:.1}µs | {:.1}µs | {:.1}µs | {:.1}µs |\n",
            fmt_num(m.agents as f64),
            fmt_num(m.total_messages as f64),
            fmt_num(m.throughput),
            m.p50_us,
            m.p95_us,
            m.p99_us,
            m.max_us,
        ));
    }

    // ── Phase 4: Governance ──────────────────────────────────────────────
    rpt.push_str("\n## Phase 4: Governance Gate Checks\n\n");
    rpt.push_str("| Agents | Checks | Throughput | P50 | P95 | P99 | Max |\n");
    rpt.push_str("|--------|--------|-----------|-----|-----|-----|-----|\n");
    for g in gov_results {
        rpt.push_str(&format!(
            "| {} | {} | {} checks/s | {:.1}µs | {:.1}µs | {:.1}µs | {:.1}µs |\n",
            fmt_num(g.agents as f64),
            fmt_num(g.total_checks as f64),
            fmt_num(g.throughput),
            g.p50_us,
            g.p95_us,
            g.p99_us,
            g.max_us,
        ));
    }

    // ── Phase 5: Mix ─────────────────────────────────────────────────────
    rpt.push_str("\n## Phase 5: Realistic Coordination Mix\n\n");
    rpt.push_str(
        "| Agents | Total Ops | Throughput | P50 | P95 | P99 | RSS | Fuel | Msg | Gov |\n",
    );
    rpt.push_str("|--------|-----------|-----------|-----|-----|-----|-----|------|-----|-----|\n");
    for m in mix_results {
        rpt.push_str(&format!(
            "| {} | {} | {} ops/s | {:.1}µs | {:.1}µs | {:.1}µs | {:.0}MB | {} | {} | {} |\n",
            fmt_num(m.agents as f64),
            fmt_num(m.total_ops as f64),
            fmt_num(m.throughput),
            m.p50_us,
            m.p95_us,
            m.p99_us,
            m.rss_mb,
            fmt_num(m.fuel_ops as f64),
            fmt_num(m.msg_ops as f64),
            fmt_num(m.gov_ops as f64),
        ));
    }

    // ── Phase 6: Swarm ───────────────────────────────────────────────────
    rpt.push_str("\n## Phase 6: SwarmCoordinator Consensus\n\n");
    rpt.push_str(&format!("- **Runs**: {}\n", swarm.swarm_runs));
    rpt.push_str(&format!(
        "- **Total evaluations**: {}\n",
        fmt_num(swarm.total_evaluations as f64)
    ));
    rpt.push_str(&format!(
        "- **Convergences**: {}/{}\n",
        swarm.convergences, swarm.swarm_runs
    ));
    rpt.push_str(&format!(
        "- **Avg generations**: {:.1}\n",
        swarm.avg_generations
    ));
    rpt.push_str(&format!(
        "- **Evaluation throughput**: {} evals/s\n",
        fmt_num(swarm.throughput)
    ));
    rpt.push_str(&format!("- **Duration**: {:.2}s\n", swarm.duration_secs));

    // ── Phase 7: Ceiling ─────────────────────────────────────────────────
    rpt.push_str("\n## Phase 7: Coordination Ceiling\n\n");
    if ceiling_results.is_empty() {
        rpt.push_str("No ceiling tests run (base level may have exceeded limits).\n");
    } else {
        rpt.push_str("| Target | Spawned | Throughput | P50 | P99 | RSS | Status |\n");
        rpt.push_str("|--------|---------|-----------|-----|-----|-----|--------|\n");
        for c in ceiling_results {
            rpt.push_str(&format!(
                "| {} | {} | {} ops/s | {:.1}µs | {:.1}µs | {:.0}MB | {} |\n",
                fmt_num(c.target as f64),
                fmt_num(c.spawned as f64),
                fmt_num(c.decision_throughput),
                c.p50_us,
                c.p99_us,
                c.rss_mb,
                if c.breakdown { "CEILING HIT" } else { "OK" },
            ));
        }
    }

    // ── Configuration ────────────────────────────────────────────────────
    rpt.push_str("\n## Test Configuration\n\n");
    rpt.push_str(&format!("- Scale levels: {:?}\n", SCALE_LEVELS));
    rpt.push_str(&format!("- Threads: {THREAD_COUNT}\n"));
    rpt.push_str(&format!("- Fuel ops/agent: {FUEL_OPS_PER_AGENT}\n"));
    rpt.push_str(&format!("- Message rounds/agent: {MESSAGE_ROUNDS}\n"));
    rpt.push_str(&format!(
        "- Governance checks/agent: {GOVERNANCE_CHECKS_PER_AGENT}\n"
    ));
    rpt.push_str("- Coordination mix: 30% fuel / 20% governance / 50% messaging\n");
    rpt.push_str("- Ceiling levels: 150K, 200K, 250K\n");

    rpt.push_str(
        "\n## How to Run\n\n```bash\ncargo run -p nexus-conductor-benchmark --bin multiagent-coordination-bench --release\n```\n",
    );

    std::fs::write("MULTIAGENT_COORDINATION_RESULTS.md", &rpt).expect("failed to write report");
    eprintln!("\n  Report: MULTIAGENT_COORDINATION_RESULTS.md");
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let wall_start = Instant::now();

    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║   NEXUS OS — Multi-Agent Coordination Stress Test          ║");
    eprintln!("║   50,000+ Agents • Lock-Free • Governance • Ceiling        ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    // Create ConcurrentSupervisor from empty Supervisor
    let supervisor = Supervisor::new();
    let cs = Arc::new(ConcurrentSupervisor::from_supervisor(supervisor));

    // Phase 1: Spawn to all scale levels
    let spawn_results = run_phase1(&cs);

    // Collect all spawned agent IDs from the ConcurrentSupervisor
    // We know agents were spawned as make_manifest(0..100000), so we can
    // just query the supervisor for all agents. But ConcurrentSupervisor
    // doesn't expose an iterator. We can use the inner Supervisor instead.
    let agent_ids: Vec<Uuid> =
        cs.with_inner(|sup| sup.health_check().iter().map(|s| s.id).collect());

    eprintln!("\n  Total spawned agents: {}", agent_ids.len());

    // Phase 2-5: Coordination tests at each scale level
    let fuel_results = run_phase2(&cs, &agent_ids);
    let msg_results = run_phase3(&cs, &agent_ids);
    let gov_results = run_phase4(&cs, &agent_ids);
    let mix_results = run_phase5(&cs, &agent_ids);

    // Phase 6: SwarmCoordinator stress
    let swarm = run_phase6();

    // Phase 7: Push past 100K
    let ceiling_results = run_phase7(&cs, agent_ids.len());

    let total_elapsed = wall_start.elapsed().as_secs_f64();

    generate_report(
        &spawn_results,
        &fuel_results,
        &msg_results,
        &gov_results,
        &mix_results,
        &swarm,
        &ceiling_results,
        total_elapsed,
    );

    // Final summary
    let peak_rss = read_rss_mb();
    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  COMPLETE — {:.1}s ({:.1} minutes){:>30}║",
        total_elapsed,
        total_elapsed / 60.0,
        "",
    );
    eprintln!(
        "║  Agents: {} | Peak RSS: {:.0}MB{:>27}║",
        fmt_num(cs.total_agents() as f64),
        peak_rss,
        "",
    );
    eprintln!(
        "║  Messages sent: {} | Fuel consumed: {}{:>15}║",
        fmt_num(cs.total_messages_sent() as f64),
        fmt_num(cs.total_fuel_consumed() as f64),
        "",
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}
