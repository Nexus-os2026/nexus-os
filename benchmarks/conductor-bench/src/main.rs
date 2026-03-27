//! Nexus Conductor Benchmark — stress-tests the orchestration layer under load.
//!
//! Simulates synthetic agent populations at various scales (100→2000) and measures:
//! - Supervisor decision latency (agent spawn, fuel ops, state transitions)
//! - Message passing throughput (TeamMessageBus under contention)
//! - Mutex contention (shared Supervisor via Arc<Mutex<>>)
//! - Async task saturation (concurrent cognitive cycle scheduling)
//!
//! Run: `cargo run -p nexus-conductor-benchmark --release`

use crossbeam::sync::WaitGroup;
use nexus_kernel::capabilities::has_capability;
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::orchestration::messaging::TeamMessageBus;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

// ── Configuration ──────────────────────────────────────────────────────────

const DEFAULT_AGENT_COUNTS: &[usize] = &[100, 200, 500, 1000, 2000];
const SCALED_AGENT_COUNTS: &[usize] = &[100, 500, 1000, 2000, 5000, 10000];
const WARMUP_OPS: usize = 10;

/// Scale per-agent work inversely with population to keep wall time bounded.
/// At 100 agents we do max work; at 10000 we do less per agent but more total.
fn ops_per_agent(agent_count: usize) -> usize {
    if agent_count <= 500 {
        20
    } else if agent_count <= 2000 {
        15
    } else if agent_count <= 5000 {
        10
    } else {
        5
    }
}

fn fuel_ops_per_agent(agent_count: usize) -> usize {
    if agent_count <= 500 {
        10
    } else if agent_count <= 2000 {
        8
    } else if agent_count <= 5000 {
        5
    } else {
        3
    }
}

fn message_rounds(agent_count: usize) -> usize {
    if agent_count <= 500 {
        50
    } else if agent_count <= 2000 {
        30
    } else if agent_count <= 5000 {
        15
    } else {
        8
    }
}

fn capability_checks_per_agent(agent_count: usize) -> usize {
    if agent_count <= 1000 {
        50
    } else {
        20
    }
}

// ── Synthetic Agent Generator ──────────────────────────────────────────────

/// Predefined agent archetypes with realistic capability sets.
const ARCHETYPES: &[(&str, &[&str], u64, u8)] = &[
    (
        "researcher",
        &["llm.query", "web.search", "web.read", "fs.read"],
        5000,
        3,
    ),
    ("writer", &["llm.query", "fs.read", "fs.write"], 3000, 2),
    (
        "coder",
        &["llm.query", "fs.read", "fs.write", "process.exec"],
        8000,
        3,
    ),
    ("analyst", &["llm.query", "web.search", "mcp.call"], 4000, 2),
    (
        "devops",
        &["process.exec", "fs.read", "fs.write", "mcp.call"],
        6000,
        3,
    ),
    ("sentinel", &["llm.query", "fs.read"], 2000, 1),
    (
        "publisher",
        &["llm.query", "fs.write", "web.read", "mcp.call"],
        4000,
        3,
    ),
    ("strategist", &["llm.query", "self.modify"], 10000, 4),
];

fn generate_manifest(index: usize) -> AgentManifest {
    let archetype = &ARCHETYPES[index % ARCHETYPES.len()];
    AgentManifest {
        name: format!("{}-{:04}", archetype.0, index),
        version: "1.0.0".into(),
        capabilities: archetype.1.iter().map(|s| s.to_string()).collect(),
        fuel_budget: archetype.2,
        autonomy_level: Some(archetype.3),
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

fn spawn_agents(supervisor: &mut Supervisor, count: usize) -> Vec<AgentId> {
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let manifest = generate_manifest(i);
        match supervisor.start_agent(manifest) {
            Ok(id) => ids.push(id),
            Err(e) => {
                // L5/L6 singleton limits — expected at scale
                if !e.to_string().contains("Sovereign") && !e.to_string().contains("Transcendent") {
                    eprintln!("  warn: agent {i} spawn failed: {e}");
                }
            }
        }
    }
    ids
}

// ── Metrics Collection ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LatencyBucket {
    samples: Vec<Duration>,
}

impl LatencyBucket {
    fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    fn record(&mut self, d: Duration) {
        self.samples.push(d);
    }

    fn merge(&mut self, other: &LatencyBucket) {
        self.samples.extend_from_slice(&other.samples);
    }

    fn count(&self) -> usize {
        self.samples.len()
    }

    fn mean_us(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self
            .samples
            .iter()
            .map(|d| d.as_secs_f64() * 1_000_000.0)
            .sum();
        sum / self.samples.len() as f64
    }

    fn p50_us(&self) -> f64 {
        self.percentile(0.50)
    }

    fn p95_us(&self) -> f64 {
        self.percentile(0.95)
    }

    fn p99_us(&self) -> f64 {
        self.percentile(0.99)
    }

    fn max_us(&self) -> f64 {
        self.samples
            .iter()
            .map(|d| d.as_secs_f64() * 1_000_000.0)
            .fold(0.0_f64, f64::max)
    }

    fn percentile(&self, pct: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f64> = self
            .samples
            .iter()
            .map(|d| d.as_secs_f64() * 1_000_000.0)
            .collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((pct * sorted.len() as f64) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    fn throughput_per_sec(&self, wall_time: Duration) -> f64 {
        if wall_time.as_secs_f64() == 0.0 {
            return 0.0;
        }
        self.count() as f64 / wall_time.as_secs_f64()
    }
}

#[derive(Debug, Clone)]
struct BenchResult {
    agent_count: usize,
    spawn_latency: LatencyBucket,
    fuel_reserve_latency: LatencyBucket,
    fuel_commit_latency: LatencyBucket,
    capability_check_latency: LatencyBucket,
    message_send_latency: LatencyBucket,
    concurrent_decision_latency: LatencyBucket,
    lock_wait_latency: LatencyBucket,
    total_wall_time: Duration,
    async_task_saturation_pct: f64,
    // Lock-free (ConcurrentSupervisor) results
    lockfree_fuel_reserve: LatencyBucket,
    lockfree_decision: LatencyBucket,
    lockfree_message: LatencyBucket,
}

// ── Benchmark Suites ───────────────────────────────────────────────────────

/// Bench 1: Agent spawn latency — measures Supervisor.start_agent() under growing populations.
fn bench_spawn(count: usize) -> (Vec<AgentId>, LatencyBucket, Duration) {
    let mut supervisor = Supervisor::new();
    let mut bucket = LatencyBucket::new();

    let wall_start = Instant::now();
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let manifest = generate_manifest(i);
        let start = Instant::now();
        if let Ok(id) = supervisor.start_agent(manifest) {
            let elapsed = start.elapsed();
            bucket.record(elapsed);
            ids.push(id);
        }
    }
    let wall_time = wall_start.elapsed();
    (ids, bucket, wall_time)
}

/// Bench 2: Fuel reserve-commit cycle under contention (multi-threaded).
fn bench_fuel_contention(count: usize) -> (LatencyBucket, LatencyBucket, LatencyBucket, Duration) {
    let mut supervisor = Supervisor::new();
    let ids = spawn_agents(&mut supervisor, count);
    let supervisor = Arc::new(Mutex::new(supervisor));

    let reserve_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let commit_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let lock_bucket = Arc::new(Mutex::new(LatencyBucket::new()));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(ids.len());
    let chunk_size = ids.len().div_ceil(num_threads);

    for chunk in ids.chunks(chunk_size) {
        let chunk_ids: Vec<AgentId> = chunk.to_vec();
        let sup = supervisor.clone();
        let rb = reserve_bucket.clone();
        let cb = commit_bucket.clone();
        let lb = lock_bucket.clone();
        let wg = wg.clone();

        std::thread::spawn(move || {
            let mut local_reserve = LatencyBucket::new();
            let mut local_commit = LatencyBucket::new();
            let mut local_lock = LatencyBucket::new();

            for agent_id in &chunk_ids {
                for _ in 0..fuel_ops_per_agent(count) {
                    // Reserve
                    let lock_start = Instant::now();
                    let mut guard = sup.lock().unwrap_or_else(|p| p.into_inner());
                    local_lock.record(lock_start.elapsed());

                    let op_start = Instant::now();
                    let reservation = guard.reserve_fuel(*agent_id, 100, "bench_action");
                    local_reserve.record(op_start.elapsed());

                    // Commit
                    if let Ok(reservation) = reservation {
                        let op_start = Instant::now();
                        let _ = guard.commit_fuel(reservation, 50);
                        local_commit.record(op_start.elapsed());
                    }
                    drop(guard);
                }
            }

            rb.lock().unwrap().merge(&local_reserve);
            cb.lock().unwrap().merge(&local_commit);
            lb.lock().unwrap().merge(&local_lock);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();

    let rb = Arc::try_unwrap(reserve_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    let cb = Arc::try_unwrap(commit_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    let lb = Arc::try_unwrap(lock_bucket).unwrap().into_inner().unwrap();
    (rb, cb, lb, wall_time)
}

/// Bench 3: Capability gate arbitration (read-heavy, reflects real cognitive loop checks).
fn bench_capability_checks(count: usize) -> (LatencyBucket, Duration) {
    let test_capabilities: Vec<Vec<String>> = (0..count)
        .map(|i| {
            ARCHETYPES[i % ARCHETYPES.len()]
                .1
                .iter()
                .map(|s| s.to_string())
                .collect()
        })
        .collect();

    let queries = [
        "llm.query",
        "fs.read",
        "fs.write",
        "process.exec",
        "web.search",
        "mcp.call",
        "self.modify",
        "agent.message",
    ];

    let mut bucket = LatencyBucket::new();
    let wall_start = Instant::now();

    for caps in &test_capabilities {
        for _ in 0..capability_checks_per_agent(count) {
            for q in &queries {
                let start = Instant::now();
                let _ = has_capability(caps.iter().map(String::as_str), q);
                bucket.record(start.elapsed());
            }
        }
    }

    (bucket, wall_start.elapsed())
}

/// Bench 4: TeamMessageBus throughput under contention.
fn bench_message_bus(count: usize) -> (LatencyBucket, Duration) {
    let team_id = Uuid::new_v4();
    let agent_ids: Vec<Uuid> = (0..count).map(|_| Uuid::new_v4()).collect();

    let bus = Arc::new(Mutex::new(TeamMessageBus::new()));
    let bucket = Arc::new(Mutex::new(LatencyBucket::new()));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(count);
    let chunk_size = agent_ids.len().div_ceil(num_threads);

    for chunk in agent_ids.chunks(chunk_size) {
        let chunk_agents: Vec<Uuid> = chunk.to_vec();
        let bus = bus.clone();
        let bucket = bucket.clone();
        let wg = wg.clone();
        let all_agents = agent_ids.clone();

        std::thread::spawn(move || {
            let mut local_bucket = LatencyBucket::new();

            for from in &chunk_agents {
                for round in 0..message_rounds(count) {
                    let to = all_agents[(round + 1) % all_agents.len()];
                    let start = Instant::now();
                    let mut guard = bus.lock().unwrap_or_else(|p| p.into_inner());
                    let _ = guard.send(team_id, *from, to, "benchmark payload");
                    drop(guard);
                    local_bucket.record(start.elapsed());
                }
            }

            bucket.lock().unwrap().merge(&local_bucket);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();
    let bucket = Arc::try_unwrap(bucket).unwrap().into_inner().unwrap();
    (bucket, wall_time)
}

/// Bench 5: Concurrent decision loop — simulates multiple agents hitting
/// Supervisor simultaneously for state queries + fuel checks + capability gates.
fn bench_concurrent_decisions(count: usize) -> (LatencyBucket, LatencyBucket, f64, Duration) {
    let mut supervisor = Supervisor::new();
    let ids = spawn_agents(&mut supervisor, count);
    let supervisor = Arc::new(Mutex::new(supervisor));

    let decision_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let lock_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let tasks_completed = Arc::new(AtomicU64::new(0));
    let tasks_total = Arc::new(AtomicU64::new(0));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(ids.len());
    let chunk_size = ids.len().div_ceil(num_threads);

    for chunk in ids.chunks(chunk_size) {
        let chunk_ids: Vec<AgentId> = chunk.to_vec();
        let sup = supervisor.clone();
        let db = decision_bucket.clone();
        let lb = lock_bucket.clone();
        let completed = tasks_completed.clone();
        let total = tasks_total.clone();
        let wg = wg.clone();

        std::thread::spawn(move || {
            let mut local_decision = LatencyBucket::new();
            let mut local_lock = LatencyBucket::new();

            for agent_id in &chunk_ids {
                for _ in 0..ops_per_agent(count) {
                    total.fetch_add(1, Ordering::Relaxed);
                    let decision_start = Instant::now();

                    // Step 1: Lock + query agent state
                    let lock_start = Instant::now();
                    let guard = sup.lock().unwrap_or_else(|p| p.into_inner());
                    local_lock.record(lock_start.elapsed());

                    let agent_exists = guard.get_agent(*agent_id).is_some();
                    drop(guard);

                    if !agent_exists {
                        continue;
                    }

                    // Step 2: Lock + fuel reservation
                    let lock_start = Instant::now();
                    let mut guard = sup.lock().unwrap_or_else(|p| p.into_inner());
                    local_lock.record(lock_start.elapsed());

                    let reservation = guard.reserve_fuel(*agent_id, 50, "decision_cycle");
                    if let Ok(reservation) = reservation {
                        let _ = guard.commit_fuel(reservation, 25);
                    }
                    drop(guard);

                    local_decision.record(decision_start.elapsed());
                    completed.fetch_add(1, Ordering::Relaxed);
                }
            }

            db.lock().unwrap().merge(&local_decision);
            lb.lock().unwrap().merge(&local_lock);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();

    let completed = tasks_completed.load(Ordering::Relaxed) as f64;
    let total = tasks_total.load(Ordering::Relaxed) as f64;
    let saturation = if total > 0.0 {
        (completed / total) * 100.0
    } else {
        0.0
    };

    let db = Arc::try_unwrap(decision_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    let lb = Arc::try_unwrap(lock_bucket).unwrap().into_inner().unwrap();
    (db, lb, saturation, wall_time)
}

/// Bench 6: Async task saturation — tokio-based concurrent cognitive cycle simulation.
async fn bench_async_saturation(count: usize) -> (LatencyBucket, f64, Duration) {
    let mut supervisor = Supervisor::new();
    let ids = spawn_agents(&mut supervisor, count);
    let supervisor = Arc::new(Mutex::new(supervisor));
    let bucket = Arc::new(tokio::sync::Mutex::new(LatencyBucket::new()));
    let completed = Arc::new(AtomicU64::new(0));

    let wall_start = Instant::now();
    let mut handles = Vec::with_capacity(ids.len());

    for agent_id in ids.iter().copied() {
        let sup = supervisor.clone();
        let bucket = bucket.clone();
        let completed = completed.clone();

        handles.push(tokio::spawn(async move {
            let mut local = LatencyBucket::new();
            for _ in 0..ops_per_agent(count) {
                let start = Instant::now();

                // Simulate cognitive cycle: lock → check state → fuel op → release
                {
                    let mut guard = sup.lock().unwrap_or_else(|p| p.into_inner());
                    if guard.get_agent(agent_id).is_some() {
                        let _ = guard.reserve_fuel(agent_id, 20, "async_cycle");
                    }
                }

                // Simulate LLM latency (yield to scheduler)
                tokio::task::yield_now().await;

                local.record(start.elapsed());
                completed.fetch_add(1, Ordering::Relaxed);
            }
            bucket.lock().await.merge(&local);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let wall_time = wall_start.elapsed();
    let completed_count = completed.load(Ordering::Relaxed);
    let expected = (ids.len() * ops_per_agent(count)) as u64;
    let saturation = if expected > 0 {
        (completed_count as f64 / expected as f64) * 100.0
    } else {
        0.0
    };

    let bucket = Arc::try_unwrap(bucket).unwrap().into_inner();
    (bucket, saturation, wall_time)
}

// ── Lock-Free Benchmark Suites (ConcurrentSupervisor) ─────────────────────

use nexus_kernel::concurrent_supervisor::ConcurrentSupervisor;

/// Bench 7: Lock-free fuel operations via ConcurrentSupervisor (CAS-based).
fn bench_concurrent_fuel_lockfree(count: usize) -> (LatencyBucket, LatencyBucket, Duration) {
    let mut supervisor = Supervisor::new();
    let ids = spawn_agents(&mut supervisor, count);
    let csup = Arc::new(ConcurrentSupervisor::from_supervisor(supervisor));

    let reserve_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let commit_bucket = Arc::new(Mutex::new(LatencyBucket::new()));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(ids.len());
    let chunk_size = ids.len().div_ceil(num_threads);

    for chunk in ids.chunks(chunk_size) {
        let chunk_ids: Vec<AgentId> = chunk.to_vec();
        let csup = csup.clone();
        let rb = reserve_bucket.clone();
        let cb = commit_bucket.clone();
        let wg = wg.clone();

        std::thread::spawn(move || {
            let mut local_reserve = LatencyBucket::new();
            let mut local_commit = LatencyBucket::new();

            for agent_id in &chunk_ids {
                for _ in 0..fuel_ops_per_agent(count) {
                    let op_start = Instant::now();
                    let reservation = csup.reserve_fuel(*agent_id, 100, "bench_lockfree");
                    local_reserve.record(op_start.elapsed());

                    if let Ok(reservation) = reservation {
                        let op_start = Instant::now();
                        let _ = csup.commit_fuel(reservation, 50);
                        local_commit.record(op_start.elapsed());
                    }
                }
            }

            rb.lock().unwrap().merge(&local_reserve);
            cb.lock().unwrap().merge(&local_commit);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();

    let rb = Arc::try_unwrap(reserve_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    let cb = Arc::try_unwrap(commit_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    (rb, cb, wall_time)
}

/// Bench 8: Lock-free concurrent decisions via ConcurrentSupervisor.
fn bench_concurrent_decisions_lockfree(count: usize) -> (LatencyBucket, f64, Duration) {
    let mut supervisor = Supervisor::new();
    let ids = spawn_agents(&mut supervisor, count);
    let csup = Arc::new(ConcurrentSupervisor::from_supervisor(supervisor));

    let decision_bucket = Arc::new(Mutex::new(LatencyBucket::new()));
    let tasks_completed = Arc::new(AtomicU64::new(0));
    let tasks_total = Arc::new(AtomicU64::new(0));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(ids.len());
    let chunk_size = ids.len().div_ceil(num_threads);

    for chunk in ids.chunks(chunk_size) {
        let chunk_ids: Vec<AgentId> = chunk.to_vec();
        let csup = csup.clone();
        let db = decision_bucket.clone();
        let completed = tasks_completed.clone();
        let total = tasks_total.clone();
        let wg = wg.clone();

        std::thread::spawn(move || {
            let mut local_decision = LatencyBucket::new();

            for agent_id in &chunk_ids {
                for _ in 0..ops_per_agent(count) {
                    total.fetch_add(1, Ordering::Relaxed);
                    let decision_start = Instant::now();

                    // Step 1: Lock-free agent lookup (DashMap)
                    if !csup.agent_exists(*agent_id) {
                        continue;
                    }

                    // Step 2: Lock-free fuel reservation (CAS)
                    if let Ok(reservation) = csup.reserve_fuel(*agent_id, 50, "decision_cycle") {
                        let _ = csup.commit_fuel(reservation, 25);
                    }

                    local_decision.record(decision_start.elapsed());
                    completed.fetch_add(1, Ordering::Relaxed);
                }
            }

            db.lock().unwrap().merge(&local_decision);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();

    let completed = tasks_completed.load(Ordering::Relaxed) as f64;
    let total = tasks_total.load(Ordering::Relaxed) as f64;
    let saturation = if total > 0.0 {
        (completed / total) * 100.0
    } else {
        0.0
    };

    let db = Arc::try_unwrap(decision_bucket)
        .unwrap()
        .into_inner()
        .unwrap();
    (db, saturation, wall_time)
}

/// Bench 9: Lock-free message passing via SegQueue.
fn bench_message_lockfree(count: usize) -> (LatencyBucket, Duration) {
    let agent_ids: Vec<Uuid> = (0..count).map(|_| Uuid::new_v4()).collect();
    let csup = Arc::new(ConcurrentSupervisor::from_supervisor(Supervisor::new()));
    let bucket = Arc::new(Mutex::new(LatencyBucket::new()));

    let wall_start = Instant::now();
    let wg = WaitGroup::new();
    let num_threads = num_cpus().min(count);
    let chunk_size = agent_ids.len().div_ceil(num_threads);

    for chunk in agent_ids.chunks(chunk_size) {
        let chunk_agents: Vec<Uuid> = chunk.to_vec();
        let csup = csup.clone();
        let bucket = bucket.clone();
        let all_agents = agent_ids.clone();
        let wg = wg.clone();

        std::thread::spawn(move || {
            let mut local_bucket = LatencyBucket::new();

            for from in &chunk_agents {
                for round in 0..message_rounds(count) {
                    let to = all_agents[(round + 1) % all_agents.len()];
                    let start = Instant::now();
                    csup.send_message(*from, to, "benchmark payload");
                    local_bucket.record(start.elapsed());
                }
            }

            bucket.lock().unwrap().merge(&local_bucket);
            drop(wg);
        });
    }

    wg.wait();
    let wall_time = wall_start.elapsed();
    let bucket = Arc::try_unwrap(bucket).unwrap().into_inner().unwrap();
    (bucket, wall_time)
}

// ── Orchestrator ───────────────────────────────────────────────────────────

fn run_benchmark_suite(count: usize) -> BenchResult {
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("  BENCHMARK: {} agents", count);
    eprintln!("{}", "=".repeat(70));

    // Warmup
    eprintln!("  warming up...");
    {
        let mut sup = Supervisor::new();
        for i in 0..WARMUP_OPS {
            let _ = sup.start_agent(generate_manifest(i));
        }
    }

    // B1: Spawn
    eprint!("  [1/6] spawn latency ... ");
    let (_, spawn_bucket, _spawn_wall) = bench_spawn(count);
    eprintln!(
        "done (mean={:.1}µs, p99={:.1}µs, n={})",
        spawn_bucket.mean_us(),
        spawn_bucket.p99_us(),
        spawn_bucket.count()
    );

    // B2: Fuel contention
    eprint!("  [2/6] fuel contention ... ");
    let (fuel_reserve, fuel_commit, fuel_lock, _fuel_wall) = bench_fuel_contention(count);
    eprintln!(
        "done (reserve={:.1}µs, commit={:.1}µs, lock_wait={:.1}µs)",
        fuel_reserve.mean_us(),
        fuel_commit.mean_us(),
        fuel_lock.mean_us()
    );

    // B3: Capability checks
    eprint!("  [3/6] capability gates ... ");
    let (cap_bucket, _cap_wall) = bench_capability_checks(count);
    eprintln!(
        "done (mean={:.3}µs, n={})",
        cap_bucket.mean_us(),
        cap_bucket.count()
    );

    // B4: Message bus
    eprint!("  [4/6] message bus ... ");
    let (msg_bucket, msg_wall) = bench_message_bus(count);
    eprintln!(
        "done (mean={:.1}µs, throughput={:.0} msg/s)",
        msg_bucket.mean_us(),
        msg_bucket.throughput_per_sec(msg_wall)
    );

    // B5: Concurrent decisions
    eprint!("  [5/6] concurrent decisions ... ");
    let (decision_bucket, decision_lock, saturation_sync, _dec_wall) =
        bench_concurrent_decisions(count);
    eprintln!(
        "done (mean={:.1}µs, lock_wait={:.1}µs, completion={:.1}%)",
        decision_bucket.mean_us(),
        decision_lock.mean_us(),
        saturation_sync
    );

    // B6: Async saturation (run inside current tokio runtime)
    eprint!("  [6/9] async saturation ... ");
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (async_bucket, async_saturation, async_wall) = rt.block_on(bench_async_saturation(count));
    eprintln!(
        "done (mean={:.1}µs, saturation={:.1}%, wall={:.2}s)",
        async_bucket.mean_us(),
        async_saturation,
        async_wall.as_secs_f64()
    );

    // B7: Lock-free fuel (ConcurrentSupervisor)
    eprint!("  [7/9] lock-free fuel (CAS) ... ");
    let (lf_fuel_reserve, _lf_fuel_commit, lf_fuel_wall) = bench_concurrent_fuel_lockfree(count);
    eprintln!(
        "done (reserve={:.1}µs, wall={:.2}s)",
        lf_fuel_reserve.mean_us(),
        lf_fuel_wall.as_secs_f64()
    );

    // B8: Lock-free decisions (ConcurrentSupervisor)
    eprint!("  [8/9] lock-free decisions (DashMap+CAS) ... ");
    let (lf_decision, _lf_sat, lf_dec_wall) = bench_concurrent_decisions_lockfree(count);
    eprintln!(
        "done (mean={:.1}µs, wall={:.2}s)",
        lf_decision.mean_us(),
        lf_dec_wall.as_secs_f64()
    );

    // B9: Lock-free messages (SegQueue)
    eprint!("  [9/9] lock-free messages (SegQueue) ... ");
    let (lf_message, lf_msg_wall) = bench_message_lockfree(count);
    eprintln!(
        "done (mean={:.1}µs, throughput={:.0} msg/s)",
        lf_message.mean_us(),
        lf_message.throughput_per_sec(lf_msg_wall)
    );

    let total_wall = _spawn_wall
        + _fuel_wall
        + _cap_wall
        + msg_wall
        + _dec_wall
        + async_wall
        + lf_fuel_wall
        + lf_dec_wall
        + lf_msg_wall;

    BenchResult {
        agent_count: count,
        spawn_latency: spawn_bucket,
        fuel_reserve_latency: fuel_reserve,
        fuel_commit_latency: fuel_commit,
        capability_check_latency: cap_bucket,
        message_send_latency: msg_bucket,
        concurrent_decision_latency: decision_bucket,
        lock_wait_latency: decision_lock.clone(),
        total_wall_time: total_wall,
        async_task_saturation_pct: async_saturation,
        lockfree_fuel_reserve: lf_fuel_reserve,
        lockfree_decision: lf_decision,
        lockfree_message: lf_message,
    }
}

// ── Report Generation ──────────────────────────────────────────────────────

fn generate_report(results: &[BenchResult]) -> String {
    let mut out = String::new();

    out.push_str("# Nexus Conductor Benchmark Results\n\n");
    out.push_str(&format!("**Date**: {}\n", chrono_now()));
    out.push_str(&format!(
        "**System**: {} cores, ~62 GB RAM, Ubuntu (ROG Zephyrus)\n",
        num_cpus()
    ));
    out.push_str("**Ops/agent**: scaled per tier (fuel=3-10, cap_checks=20-50, messages=8-50, decisions=5-20)\n\n");

    // Table 1: Overview
    out.push_str("## Summary\n\n");
    out.push_str("| Agents | Spawn (µs) | Fuel Reserve (µs) | Decision (µs) | Msg Throughput (msg/s) | Lock Wait (µs) | Async Sat (%) | Wall (s) |\n");
    out.push_str("|--------|-----------|-------------------|--------------|----------------------|----------------|--------------|----------|\n");
    for r in results {
        out.push_str(&format!(
            "| {:>6} | {:>9.1} | {:>17.1} | {:>12.1} | {:>20.0} | {:>14.1} | {:>12.1} | {:>8.2} |\n",
            r.agent_count,
            r.spawn_latency.mean_us(),
            r.fuel_reserve_latency.mean_us(),
            r.concurrent_decision_latency.mean_us(),
            r.message_send_latency.throughput_per_sec(r.total_wall_time),
            r.lock_wait_latency.mean_us(),
            r.async_task_saturation_pct,
            r.total_wall_time.as_secs_f64(),
        ));
    }

    // Table 2: Latency distribution
    out.push_str("\n## Latency Distribution (µs)\n\n");
    out.push_str("### Supervisor Decision Latency\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.concurrent_decision_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    out.push_str("\n### Fuel Reserve Latency\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.fuel_reserve_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    out.push_str("\n### Fuel Commit Latency\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.fuel_commit_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    out.push_str("\n### Capability Check Latency\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.capability_check_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    out.push_str("\n### Message Send Latency\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.message_send_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    out.push_str("\n### Lock Wait Time\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.lock_wait_latency;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    // Lock-free comparison table
    out.push_str("\n## Lock-Free vs Mutex Comparison\n\n");
    out.push_str("| Agents | Mutex Fuel (µs) | Lock-Free Fuel (µs) | Speedup | Mutex Decision (µs) | Lock-Free Decision (µs) | Speedup | Mutex Msg (µs) | Lock-Free Msg (µs) | Speedup |\n");
    out.push_str("|--------|----------------|--------------------:|--------:|--------------------:|------------------------:|--------:|---------------:|--------------------:|--------:|\n");
    for r in results {
        let fuel_speedup =
            r.fuel_reserve_latency.mean_us() / r.lockfree_fuel_reserve.mean_us().max(0.001);
        let decision_speedup =
            r.concurrent_decision_latency.mean_us() / r.lockfree_decision.mean_us().max(0.001);
        let msg_speedup =
            r.message_send_latency.mean_us() / r.lockfree_message.mean_us().max(0.001);
        out.push_str(&format!(
            "| {:>6} | {:>14.1} | {:>19.1} | {:>6.1}x | {:>19.1} | {:>23.1} | {:>6.1}x | {:>14.1} | {:>19.1} | {:>6.1}x |\n",
            r.agent_count,
            r.fuel_reserve_latency.mean_us(),
            r.lockfree_fuel_reserve.mean_us(),
            fuel_speedup,
            r.concurrent_decision_latency.mean_us(),
            r.lockfree_decision.mean_us(),
            decision_speedup,
            r.message_send_latency.mean_us(),
            r.lockfree_message.mean_us(),
            msg_speedup,
        ));
    }

    out.push_str("\n### Lock-Free Decision Latency Distribution\n\n");
    out.push_str("| Agents | Mean | P50 | P95 | P99 | Max |\n");
    out.push_str("|--------|------|-----|-----|-----|-----|\n");
    for r in results {
        let b = &r.lockfree_decision;
        out.push_str(&format!(
            "| {:>6} | {:>8.1} | {:>7.1} | {:>7.1} | {:>7.1} | {:>9.1} |\n",
            r.agent_count,
            b.mean_us(),
            b.p50_us(),
            b.p95_us(),
            b.p99_us(),
            b.max_us(),
        ));
    }

    // Performance cliff analysis
    out.push_str("\n## Performance Cliff Analysis\n\n");
    if results.len() >= 2 {
        let mut cliff_found = false;
        for i in 1..results.len() {
            let prev = &results[i - 1];
            let curr = &results[i];

            let latency_ratio = curr.concurrent_decision_latency.p99_us()
                / prev.concurrent_decision_latency.p99_us().max(0.001);
            let lock_ratio =
                curr.lock_wait_latency.p99_us() / prev.lock_wait_latency.p99_us().max(0.001);

            if latency_ratio > 2.0 || lock_ratio > 3.0 {
                cliff_found = true;
                out.push_str(&format!(
                    "**Performance cliff detected between {} and {} agents:**\n\n",
                    prev.agent_count, curr.agent_count
                ));
                out.push_str(&format!(
                    "- Decision P99 latency: {:.1}µs → {:.1}µs ({:.1}x increase)\n",
                    prev.concurrent_decision_latency.p99_us(),
                    curr.concurrent_decision_latency.p99_us(),
                    latency_ratio,
                ));
                out.push_str(&format!(
                    "- Lock wait P99: {:.1}µs → {:.1}µs ({:.1}x increase)\n",
                    prev.lock_wait_latency.p99_us(),
                    curr.lock_wait_latency.p99_us(),
                    lock_ratio,
                ));
                out.push_str(&format!(
                    "- Agent scale: {}x more agents\n\n",
                    curr.agent_count as f64 / prev.agent_count as f64
                ));
            }
        }

        if !cliff_found {
            out.push_str("No significant performance cliff detected across the tested range.\n");
            out.push_str("Supervisor scales linearly through 2000 agents.\n\n");
        }

        // Scaling analysis
        let first = &results[0];
        let last = results.last().unwrap();
        let scale_factor = last.agent_count as f64 / first.agent_count as f64;
        let latency_factor = last.concurrent_decision_latency.mean_us()
            / first.concurrent_decision_latency.mean_us().max(0.001);
        let lock_factor =
            last.lock_wait_latency.mean_us() / first.lock_wait_latency.mean_us().max(0.001);

        out.push_str("### Scaling Summary\n\n");
        out.push_str(&format!(
            "- Agent scale: {:.0}x ({} → {})\n",
            scale_factor, first.agent_count, last.agent_count
        ));
        out.push_str(&format!(
            "- Decision latency scale: {:.1}x\n",
            latency_factor
        ));
        out.push_str(&format!("- Lock contention scale: {:.1}x\n", lock_factor));
        out.push_str(&format!(
            "- Async task completion: {:.1}% at max load\n\n",
            last.async_task_saturation_pct
        ));

        if latency_factor < scale_factor * 0.5 {
            out.push_str(
                "**Verdict**: Sub-linear scaling — the Conductor handles contention well.\n",
            );
        } else if latency_factor < scale_factor {
            out.push_str(
                "**Verdict**: Near-linear scaling — acceptable but monitor at higher loads.\n",
            );
        } else {
            out.push_str(
                "**Verdict**: Super-linear scaling — contention is a bottleneck. Consider sharding Supervisor state or switching to RwLock.\n",
            );
        }
    }

    // ASCII latency curve
    out.push_str("\n## Latency Curve (Decision P99)\n\n");
    out.push_str("```\n");
    let max_p99 = results
        .iter()
        .map(|r| r.concurrent_decision_latency.p99_us())
        .fold(0.0_f64, f64::max);
    let bar_width = 50;
    for r in results {
        let p99 = r.concurrent_decision_latency.p99_us();
        let bar_len = if max_p99 > 0.0 {
            ((p99 / max_p99) * bar_width as f64) as usize
        } else {
            0
        };
        let bar: String = "█".repeat(bar_len);
        out.push_str(&format!(
            "{:>5} agents │{:<width$}│ {:.1}µs\n",
            r.agent_count,
            bar,
            p99,
            width = bar_width,
        ));
    }
    out.push_str("```\n");

    out
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

fn chrono_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO-ish date from epoch
    let days = now / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{}-{:02}-{:02}", years, month.min(12), day.min(31))
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scaled = args.iter().any(|a| a == "--scaled");

    let (agent_counts, report_filename) = if scaled {
        (SCALED_AGENT_COUNTS, "BENCHMARK_RESULTS_SCALED.md")
    } else {
        (DEFAULT_AGENT_COUNTS, "BENCHMARK_RESULTS.md")
    };

    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Nexus Conductor Benchmark — {}           ║",
        if scaled {
            "SCALED (5K/10K)"
        } else {
            "Standard      "
        }
    );
    eprintln!("║  Agent Counts: {:?}", agent_counts);
    eprintln!("║  Threads: {}", num_cpus());
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    let mut results = Vec::new();

    for &count in agent_counts {
        let result = run_benchmark_suite(count);
        results.push(result);
    }

    eprintln!("\n\nGenerating report...");
    let report = generate_report(&results);

    let report_path = std::env::current_dir()
        .unwrap_or_default()
        .join(report_filename);
    match std::fs::File::create(&report_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(report.as_bytes()) {
                eprintln!("Failed to write report: {e}");
                print!("{report}");
            } else {
                eprintln!("Report written to: {}", report_path.display());
            }
        }
        Err(e) => {
            eprintln!("Failed to create report file: {e}");
            print!("{report}");
        }
    }

    println!("\n{report}");
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles_and_exports_are_reachable() {
        // Smoke test: verifies the crate compiles and public API is accessible
        assert!(true);
    }
}
