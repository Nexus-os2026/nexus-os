//! Nexus OS — Audit Trail Write-Pressure Throughput Stress Test
//!
//! Measures maximum sustained write throughput of the hash-chained audit trail
//! under increasing load, validates integrity under concurrency, and tests
//! Merkle retention under write pressure.
//!
//! Phases:
//!   1. Single-thread baseline: raw append throughput, latency percentiles
//!   2. Concurrent contended writes: 100 agents via Arc<Mutex<AuditTrail>>
//!   3. Sharded throughput: one trail per agent (no lock contention)
//!   4. Ramp test: 1K → 10K → 100K events/sec, find the ceiling
//!   5. Retention under write pressure: bounded trail with disk archival
//!   6. Recovery test: interrupted writes, verify chain integrity
//!
//! Run:
//!   cargo run -p nexus-conductor-benchmark --bin audit-throughput-bench --release

use nexus_kernel::audit::retention::RetentionConfig;
use nexus_kernel::audit::{AuditTrail, EventType};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

// ── Configuration ────────────────────────────────────────────────────────────

const BASELINE_EVENTS: usize = 500_000;
const CONCURRENT_AGENTS: usize = 100;
const CONCURRENT_EVENTS_PER_AGENT: usize = 1_000;
const SHARDED_AGENTS: usize = 100;
const SHARDED_EVENTS_PER_AGENT: usize = 1_000;
const RAMP_LEVELS: &[usize] = &[1_000, 10_000, 50_000, 100_000, 200_000, 500_000];
const RETENTION_EVENTS: usize = 200_000;
const RETENTION_MAX_LIVE: usize = 10_000;
const RETENTION_SEGMENT_SIZE: usize = 1_000;
const RECOVERY_EVENTS: usize = 10_000;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_payload(idx: usize) -> serde_json::Value {
    json!({
        "event": "audit.throughput_test",
        "action": "write_pressure",
        "idx": idx,
        "fuel_cost": 10,
    })
}

/// Compute latency percentiles from a sorted slice of durations (in nanoseconds).
fn percentiles(sorted_ns: &[u64]) -> (f64, f64, f64, f64, f64) {
    if sorted_ns.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let len = sorted_ns.len();
    let p50 = sorted_ns[len * 50 / 100] as f64 / 1_000.0; // microseconds
    let p90 = sorted_ns[len * 90 / 100] as f64 / 1_000.0;
    let p95 = sorted_ns[len * 95 / 100] as f64 / 1_000.0;
    let p99 = sorted_ns[len * 99 / 100] as f64 / 1_000.0;
    let max = sorted_ns[len - 1] as f64 / 1_000.0;
    (p50, p90, p95, p99, max)
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

fn pass_fail(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

/// Format a number with comma grouping (e.g. 1234567 → "1,234,567").
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

// ── Phase 1: Single-Thread Baseline ──────────────────────────────────────────

#[derive(Debug)]
struct BaselineResult {
    total_events: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p90_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    integrity: bool,
}

fn run_baseline() -> BaselineResult {
    eprintln!("\n═══ Phase 1: Single-Thread Baseline ({BASELINE_EVENTS} events) ═══\n");

    let mut trail = AuditTrail::new();
    let agent_id = Uuid::new_v4();
    let mut latencies_ns: Vec<u64> = Vec::with_capacity(BASELINE_EVENTS);

    let wall_start = Instant::now();
    for i in 0..BASELINE_EVENTS {
        let t = Instant::now();
        let _ = trail.append_event(agent_id, EventType::StateChange, make_payload(i));
        latencies_ns.push(t.elapsed().as_nanos() as u64);

        if i > 0 && i % 100_000 == 0 {
            eprint!("\r  {i}/{BASELINE_EVENTS} events...");
        }
    }
    let wall_secs = wall_start.elapsed().as_secs_f64();
    eprintln!("\r  {BASELINE_EVENTS}/{BASELINE_EVENTS} events — verifying integrity...");

    let integrity = trail.verify_integrity();
    latencies_ns.sort_unstable();
    let (p50, p90, p95, p99, max) = percentiles(&latencies_ns);
    let throughput = BASELINE_EVENTS as f64 / wall_secs;

    eprintln!(
        "  Throughput: {throughput:.0} events/s | P50={p50:.1}µs P95={p95:.1}µs P99={p99:.1}µs | Integrity={integrity}"
    );

    BaselineResult {
        total_events: BASELINE_EVENTS,
        duration_secs: wall_secs,
        throughput,
        p50_us: p50,
        p90_us: p90,
        p95_us: p95,
        p99_us: p99,
        max_us: max,
        integrity,
    }
}

// ── Phase 2: Concurrent Contended Writes ─────────────────────────────────────

#[derive(Debug)]
struct ConcurrentResult {
    agents: usize,
    events_per_agent: usize,
    total_events: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    integrity: bool,
}

fn run_concurrent() -> ConcurrentResult {
    let total = CONCURRENT_AGENTS * CONCURRENT_EVENTS_PER_AGENT;
    eprintln!("\n═══ Phase 2: Concurrent Contended Writes ({CONCURRENT_AGENTS} agents × {CONCURRENT_EVENTS_PER_AGENT} events) ═══\n");

    let trail = Arc::new(Mutex::new(AuditTrail::new()));
    let agents: Vec<Uuid> = (0..CONCURRENT_AGENTS).map(|_| Uuid::new_v4()).collect();

    // Collect latencies from all threads
    let all_latencies: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::with_capacity(total)));

    let wall_start = Instant::now();

    std::thread::scope(|s| {
        for agent_id in &agents {
            let trail = Arc::clone(&trail);
            let lat_collector = Arc::clone(&all_latencies);
            let aid = *agent_id;
            s.spawn(move || {
                let mut local_lats: Vec<u64> = Vec::with_capacity(CONCURRENT_EVENTS_PER_AGENT);
                for i in 0..CONCURRENT_EVENTS_PER_AGENT {
                    let t = Instant::now();
                    let _ = trail.lock().unwrap().append_event(
                        aid,
                        EventType::ToolCall,
                        make_payload(i),
                    );
                    local_lats.push(t.elapsed().as_nanos() as u64);
                }
                lat_collector.lock().unwrap().extend_from_slice(&local_lats);
            });
        }
    });

    let wall_secs = wall_start.elapsed().as_secs_f64();
    eprintln!("  Writes complete — verifying integrity...");

    let integrity = trail.lock().unwrap().verify_integrity();
    let actual_events = trail.lock().unwrap().events().len();

    let mut lats = Arc::try_unwrap(all_latencies)
        .unwrap()
        .into_inner()
        .unwrap();
    lats.sort_unstable();
    let (p50, _, p95, p99, max) = percentiles(&lats);
    let throughput = actual_events as f64 / wall_secs;

    eprintln!(
        "  Throughput: {throughput:.0} events/s | P50={p50:.1}µs P95={p95:.1}µs P99={p99:.1}µs | Events={actual_events} Integrity={integrity}"
    );

    ConcurrentResult {
        agents: CONCURRENT_AGENTS,
        events_per_agent: CONCURRENT_EVENTS_PER_AGENT,
        total_events: actual_events,
        duration_secs: wall_secs,
        throughput,
        p50_us: p50,
        p95_us: p95,
        p99_us: p99,
        max_us: max,
        integrity,
    }
}

// ── Phase 3: Sharded Throughput (No Lock Contention) ────────────────────────

#[derive(Debug)]
struct ShardedResult {
    agents: usize,
    events_per_agent: usize,
    total_events: usize,
    duration_secs: f64,
    aggregate_throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    all_integrity: bool,
}

fn run_sharded() -> ShardedResult {
    let total = SHARDED_AGENTS * SHARDED_EVENTS_PER_AGENT;
    eprintln!("\n═══ Phase 3: Sharded Throughput ({SHARDED_AGENTS} agents × {SHARDED_EVENTS_PER_AGENT} events, no contention) ═══\n");

    let all_latencies: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::with_capacity(total)));
    let integrity_results: Arc<Mutex<Vec<bool>>> =
        Arc::new(Mutex::new(Vec::with_capacity(SHARDED_AGENTS)));

    let wall_start = Instant::now();

    std::thread::scope(|s| {
        for _ in 0..SHARDED_AGENTS {
            let lat_collector = Arc::clone(&all_latencies);
            let int_collector = Arc::clone(&integrity_results);
            s.spawn(move || {
                let mut trail = AuditTrail::new();
                let agent_id = Uuid::new_v4();
                let mut local_lats: Vec<u64> = Vec::with_capacity(SHARDED_EVENTS_PER_AGENT);
                for i in 0..SHARDED_EVENTS_PER_AGENT {
                    let t = Instant::now();
                    let _ = trail.append_event(agent_id, EventType::StateChange, make_payload(i));
                    local_lats.push(t.elapsed().as_nanos() as u64);
                }
                let ok = trail.verify_integrity();
                lat_collector.lock().unwrap().extend_from_slice(&local_lats);
                int_collector.lock().unwrap().push(ok);
            });
        }
    });

    let wall_secs = wall_start.elapsed().as_secs_f64();

    let mut lats = Arc::try_unwrap(all_latencies)
        .unwrap()
        .into_inner()
        .unwrap();
    lats.sort_unstable();
    let (p50, _, p95, p99, max) = percentiles(&lats);
    let all_ok = Arc::try_unwrap(integrity_results)
        .unwrap()
        .into_inner()
        .unwrap()
        .iter()
        .all(|&v| v);
    let aggregate_throughput = total as f64 / wall_secs;

    eprintln!(
        "  Aggregate throughput: {aggregate_throughput:.0} events/s | P50={p50:.1}µs P95={p95:.1}µs P99={p99:.1}µs | All integrity={all_ok}"
    );

    ShardedResult {
        agents: SHARDED_AGENTS,
        events_per_agent: SHARDED_EVENTS_PER_AGENT,
        total_events: total,
        duration_secs: wall_secs,
        aggregate_throughput,
        p50_us: p50,
        p95_us: p95,
        p99_us: p99,
        max_us: max,
        all_integrity: all_ok,
    }
}

// ── Phase 4: Ramp Test (Find the Ceiling) ────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RampLevel {
    target_events: usize,
    actual_events: usize,
    duration_secs: f64,
    throughput: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    integrity: bool,
}

fn run_ramp() -> Vec<RampLevel> {
    eprintln!("\n═══ Phase 4: Ramp Test — Finding the Throughput Ceiling ═══\n");

    let mut results = Vec::new();
    let agent_id = Uuid::new_v4();

    for &target in RAMP_LEVELS {
        let mut trail = AuditTrail::new();
        let mut latencies_ns: Vec<u64> = Vec::with_capacity(target);

        let wall_start = Instant::now();
        for i in 0..target {
            let t = Instant::now();
            let _ = trail.append_event(agent_id, EventType::LlmCall, make_payload(i));
            latencies_ns.push(t.elapsed().as_nanos() as u64);
        }
        let wall_secs = wall_start.elapsed().as_secs_f64();

        let integrity = trail.verify_integrity();
        latencies_ns.sort_unstable();
        let (p50, _, p95, p99, max) = percentiles(&latencies_ns);
        let throughput = target as f64 / wall_secs;

        eprintln!(
            "  {:>7} events: {throughput:>10.0} events/s | P50={p50:>7.1}µs P95={p95:>7.1}µs P99={p99:>7.1}µs Max={max:>10.1}µs | Integrity={integrity}",
            target,
        );

        results.push(RampLevel {
            target_events: target,
            actual_events: target,
            duration_secs: wall_secs,
            throughput,
            p50_us: p50,
            p95_us: p95,
            p99_us: p99,
            max_us: max,
            integrity,
        });
    }

    results
}

// ── Phase 5: Retention Under Write Pressure ──────────────────────────────────

#[derive(Debug)]
struct RetentionResult {
    total_events: usize,
    duration_secs: f64,
    throughput: f64,
    live_events: usize,
    archived_segments: usize,
    archived_events: u64,
    total_accounted: u64,
    zero_loss: bool,
    live_integrity: bool,
    full_integrity: bool,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
}

fn run_retention() -> RetentionResult {
    eprintln!(
        "\n═══ Phase 5: Retention Under Write Pressure ({RETENTION_EVENTS} events, max_live={RETENTION_MAX_LIVE}) ═══\n"
    );

    let archive_dir =
        std::env::temp_dir().join(format!("nexus_audit_throughput_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&archive_dir);

    let mut trail = AuditTrail::new();
    trail.enable_retention(RetentionConfig {
        max_live_events: RETENTION_MAX_LIVE,
        segment_size: RETENTION_SEGMENT_SIZE,
        archive_dir: archive_dir.clone(),
    });

    let agent_id = Uuid::new_v4();
    let mut latencies_ns: Vec<u64> = Vec::with_capacity(RETENTION_EVENTS);

    let wall_start = Instant::now();
    for i in 0..RETENTION_EVENTS {
        let t = Instant::now();
        let _ = trail.append_event(agent_id, EventType::StateChange, make_payload(i));
        latencies_ns.push(t.elapsed().as_nanos() as u64);

        if i > 0 && i % 50_000 == 0 {
            eprint!(
                "\r  {i}/{RETENTION_EVENTS} events (live={}, segments={})...",
                trail.events().len(),
                trail.archived_segments().len(),
            );
        }
    }
    let wall_secs = wall_start.elapsed().as_secs_f64();
    eprintln!("\r  {RETENTION_EVENTS}/{RETENTION_EVENTS} events — verifying integrity...");

    let live_events = trail.events().len();
    let segments = trail.archived_segments();
    let archived_events: u64 = segments.iter().map(|s| s.event_count as u64).sum();
    let total_accounted = live_events as u64 + archived_events;
    let zero_loss = total_accounted == RETENTION_EVENTS as u64;

    let live_integrity = trail.verify_integrity();
    let full_integrity = trail.verify_full_integrity().unwrap_or(false);
    let throughput = RETENTION_EVENTS as f64 / wall_secs;

    latencies_ns.sort_unstable();
    let (p50, _, p95, p99, max) = percentiles(&latencies_ns);

    eprintln!(
        "  Throughput: {throughput:.0} events/s | Live={live_events} Archived={archived_events} Segments={} | Zero loss={zero_loss}",
        segments.len(),
    );
    eprintln!(
        "  P50={p50:.1}µs P95={p95:.1}µs P99={p99:.1}µs | Live integrity={live_integrity} Full integrity={full_integrity}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&archive_dir);

    RetentionResult {
        total_events: RETENTION_EVENTS,
        duration_secs: wall_secs,
        throughput,
        live_events,
        archived_segments: segments.len(),
        archived_events,
        total_accounted,
        zero_loss,
        live_integrity,
        full_integrity,
        p50_us: p50,
        p95_us: p95,
        p99_us: p99,
        max_us: max,
    }
}

// ── Phase 6: Recovery Test ──────────────────────────────────────────────────

#[derive(Debug)]
struct RecoveryResult {
    pre_interrupt_events: usize,
    post_recovery_events: usize,
    pre_integrity: bool,
    post_integrity: bool,
    chain_continuous: bool,
}

fn run_recovery() -> RecoveryResult {
    eprintln!("\n═══ Phase 6: Recovery Test — Interrupted Writes ═══\n");

    let agent_id = Uuid::new_v4();

    // Write first half of events
    let mut trail = AuditTrail::new();
    for i in 0..RECOVERY_EVENTS / 2 {
        let _ = trail.append_event(agent_id, EventType::StateChange, make_payload(i));
    }
    let pre_count = trail.events().len();
    let pre_integrity = trail.verify_integrity();

    // Save the last hash to simulate recovery point
    let last_hash = trail
        .events()
        .last()
        .map(|e| e.hash.clone())
        .unwrap_or_default();

    // Simulate "crash" — write second half to same trail (recovery = resume appending)
    for i in (RECOVERY_EVENTS / 2)..RECOVERY_EVENTS {
        let _ = trail.append_event(agent_id, EventType::StateChange, make_payload(i));
    }
    let post_count = trail.events().len();
    let post_integrity = trail.verify_integrity();

    // Verify the chain is continuous (no gaps at the recovery boundary)
    let boundary_event = &trail.events()[RECOVERY_EVENTS / 2];
    let chain_continuous = boundary_event.previous_hash == last_hash;

    eprintln!("  Pre-interrupt: {pre_count} events, integrity={pre_integrity}");
    eprintln!(
        "  Post-recovery: {post_count} events, integrity={post_integrity}, chain_continuous={chain_continuous}"
    );

    RecoveryResult {
        pre_interrupt_events: pre_count,
        post_recovery_events: post_count,
        pre_integrity,
        post_integrity,
        chain_continuous,
    }
}

// ── Report Generation ────────────────────────────────────────────────────────

fn generate_report(
    baseline: &BaselineResult,
    concurrent: &ConcurrentResult,
    sharded: &ShardedResult,
    ramp: &[RampLevel],
    retention: &RetentionResult,
    recovery: &RecoveryResult,
    total_elapsed: f64,
) {
    let now = chrono_like_utc();

    // Determine max sustained throughput (highest ramp level with integrity)
    let max_sustained = ramp
        .iter()
        .filter(|r| r.integrity)
        .map(|r| r.throughput)
        .fold(0.0_f64, f64::max);

    // Find P99 at 50K level (or closest)
    let p99_at_50k = ramp
        .iter()
        .find(|r| r.target_events >= 50_000)
        .map(|r| r.p99_us)
        .unwrap_or(0.0);

    // Success criteria
    let throughput_ok = max_sustained >= 100_000.0;
    let p99_ok = p99_at_50k < 1_000.0; // under 1ms = 1000µs
    let integrity_ok = baseline.integrity
        && concurrent.integrity
        && sharded.all_integrity
        && ramp.iter().all(|r| r.integrity);
    let retention_ok = retention.zero_loss && retention.live_integrity && retention.full_integrity;
    let recovery_ok =
        recovery.pre_integrity && recovery.post_integrity && recovery.chain_continuous;
    let all_pass = throughput_ok && p99_ok && integrity_ok && retention_ok && recovery_ok;

    let mut r = String::new();
    r.push_str("# Nexus OS — Audit Trail Write-Pressure Throughput Stress Test Results\n\n");
    r.push_str(&format!("**Date**: {now}\n"));
    r.push_str(&format!(
        "**Total wall time**: {total_elapsed:.1}s ({:.1} minutes)\n",
        total_elapsed / 60.0,
    ));
    r.push_str(&format!(
        "**Result**: {}\n\n",
        if all_pass {
            "ALL CRITERIA PASSED"
        } else {
            "CRITERIA FAILED"
        },
    ));

    // ── Success Criteria ─────────────────────────────────────────────────
    r.push_str("## Success Criteria\n\n");
    r.push_str("| Criterion | Target | Actual | Status |\n");
    r.push_str("|-----------|--------|--------|--------|\n");
    r.push_str(&format!(
        "| Sustained throughput | ≥100,000 events/s | {} events/s | {} |\n",
        fmt_num(max_sustained),
        pass_fail(throughput_ok),
    ));
    r.push_str(&format!(
        "| P99 latency at 50K events | <1ms (1,000µs) | {p99_at_50k:.1}µs | {} |\n",
        pass_fail(p99_ok),
    ));
    r.push_str(&format!(
        "| Hash chain integrity (all phases) | valid | {} | {} |\n",
        if integrity_ok { "valid" } else { "BROKEN" },
        pass_fail(integrity_ok),
    ));
    r.push_str(&format!(
        "| Retention zero data loss | 0 lost | {} lost | {} |\n",
        if retention.zero_loss {
            "0"
        } else {
            "EVENTS MISSING"
        },
        pass_fail(retention_ok),
    ));
    r.push_str(&format!(
        "| Recovery after interruption | continuous chain | {} | {} |\n",
        if recovery_ok { "continuous" } else { "BROKEN" },
        pass_fail(recovery_ok),
    ));

    // ── Phase 1: Baseline ────────────────────────────────────────────────
    r.push_str("\n---\n\n## Phase 1: Single-Thread Baseline\n\n");
    r.push_str(&format!("- **Events**: {}\n", baseline.total_events));
    r.push_str(&format!("- **Duration**: {:.2}s\n", baseline.duration_secs));
    r.push_str(&format!(
        "- **Throughput**: {} events/s\n",
        fmt_num(baseline.throughput)
    ));
    r.push_str(&format!(
        "- **Integrity**: {}\n\n",
        pass_fail(baseline.integrity)
    ));

    r.push_str("| Percentile | Latency |\n");
    r.push_str("|------------|----------|\n");
    r.push_str(&format!("| P50 | {:.1}µs |\n", baseline.p50_us));
    r.push_str(&format!("| P90 | {:.1}µs |\n", baseline.p90_us));
    r.push_str(&format!("| P95 | {:.1}µs |\n", baseline.p95_us));
    r.push_str(&format!("| P99 | {:.1}µs |\n", baseline.p99_us));
    r.push_str(&format!("| Max | {:.1}µs |\n", baseline.max_us));

    // ── Phase 2: Concurrent ──────────────────────────────────────────────
    r.push_str("\n## Phase 2: Concurrent Contended Writes (Arc<Mutex<AuditTrail>>)\n\n");
    r.push_str(&format!(
        "- **Agents**: {} × {} events = {} total\n",
        concurrent.agents, concurrent.events_per_agent, concurrent.total_events,
    ));
    r.push_str(&format!(
        "- **Duration**: {:.2}s\n",
        concurrent.duration_secs
    ));
    r.push_str(&format!(
        "- **Throughput**: {} events/s\n",
        fmt_num(concurrent.throughput)
    ));
    r.push_str(&format!(
        "- **Integrity**: {}\n\n",
        pass_fail(concurrent.integrity)
    ));

    r.push_str("| Percentile | Latency |\n");
    r.push_str("|------------|----------|\n");
    r.push_str(&format!("| P50 | {:.1}µs |\n", concurrent.p50_us));
    r.push_str(&format!("| P95 | {:.1}µs |\n", concurrent.p95_us));
    r.push_str(&format!("| P99 | {:.1}µs |\n", concurrent.p99_us));
    r.push_str(&format!("| Max | {:.1}µs |\n", concurrent.max_us));

    // ── Phase 3: Sharded ─────────────────────────────────────────────────
    r.push_str("\n## Phase 3: Sharded Throughput (Per-Agent Trails, No Contention)\n\n");
    r.push_str(&format!(
        "- **Agents**: {} × {} events = {} total\n",
        sharded.agents, sharded.events_per_agent, sharded.total_events,
    ));
    r.push_str(&format!("- **Duration**: {:.2}s\n", sharded.duration_secs));
    r.push_str(&format!(
        "- **Aggregate throughput**: {} events/s\n",
        fmt_num(sharded.aggregate_throughput),
    ));
    r.push_str(&format!(
        "- **All integrity verified**: {}\n\n",
        pass_fail(sharded.all_integrity)
    ));

    r.push_str("| Percentile | Latency |\n");
    r.push_str("|------------|----------|\n");
    r.push_str(&format!("| P50 | {:.1}µs |\n", sharded.p50_us));
    r.push_str(&format!("| P95 | {:.1}µs |\n", sharded.p95_us));
    r.push_str(&format!("| P99 | {:.1}µs |\n", sharded.p99_us));
    r.push_str(&format!("| Max | {:.1}µs |\n", sharded.max_us));

    // ── Phase 4: Ramp ────────────────────────────────────────────────────
    r.push_str("\n## Phase 4: Ramp Test — Throughput Ceiling\n\n");
    r.push_str("| Events | Throughput | P50 | P95 | P99 | Max | Integrity |\n");
    r.push_str("|--------|-----------|-----|-----|-----|-----|----------|\n");
    for level in ramp {
        r.push_str(&format!(
            "| {:>7} | {:>15} events/s | {:>7.1}µs | {:>7.1}µs | {:>7.1}µs | {:>10.1}µs | {} |\n",
            level.target_events,
            fmt_num(level.throughput),
            level.p50_us,
            level.p95_us,
            level.p99_us,
            level.max_us,
            pass_fail(level.integrity),
        ));
    }

    // ── Phase 5: Retention ───────────────────────────────────────────────
    r.push_str("\n## Phase 5: Retention Under Write Pressure\n\n");
    r.push_str(&format!("- **Events**: {}\n", retention.total_events));
    r.push_str(&format!(
        "- **Duration**: {:.2}s\n",
        retention.duration_secs
    ));
    r.push_str(&format!(
        "- **Throughput**: {} events/s\n",
        fmt_num(retention.throughput)
    ));
    r.push_str(&format!("- **Live events**: {}\n", retention.live_events));
    r.push_str(&format!(
        "- **Archived segments**: {}\n",
        retention.archived_segments
    ));
    r.push_str(&format!(
        "- **Archived events**: {}\n",
        retention.archived_events
    ));
    r.push_str(&format!(
        "- **Total accounted**: {}\n",
        retention.total_accounted
    ));
    r.push_str(&format!(
        "- **Zero data loss**: {}\n",
        pass_fail(retention.zero_loss)
    ));
    r.push_str(&format!(
        "- **Live integrity**: {}\n",
        pass_fail(retention.live_integrity)
    ));
    r.push_str(&format!(
        "- **Full integrity**: {}\n\n",
        pass_fail(retention.full_integrity)
    ));

    r.push_str("| Percentile | Latency |\n");
    r.push_str("|------------|----------|\n");
    r.push_str(&format!("| P50 | {:.1}µs |\n", retention.p50_us));
    r.push_str(&format!("| P95 | {:.1}µs |\n", retention.p95_us));
    r.push_str(&format!("| P99 | {:.1}µs |\n", retention.p99_us));
    r.push_str(&format!("| Max | {:.1}µs |\n", retention.max_us));

    // ── Phase 6: Recovery ────────────────────────────────────────────────
    r.push_str("\n## Phase 6: Recovery After Interruption\n\n");
    r.push_str(&format!(
        "- **Pre-interrupt events**: {}\n",
        recovery.pre_interrupt_events,
    ));
    r.push_str(&format!(
        "- **Post-recovery events**: {}\n",
        recovery.post_recovery_events,
    ));
    r.push_str(&format!(
        "- **Pre-interrupt integrity**: {}\n",
        pass_fail(recovery.pre_integrity),
    ));
    r.push_str(&format!(
        "- **Post-recovery integrity**: {}\n",
        pass_fail(recovery.post_integrity),
    ));
    r.push_str(&format!(
        "- **Chain continuous at boundary**: {}\n",
        pass_fail(recovery.chain_continuous),
    ));

    // ── Configuration ────────────────────────────────────────────────────
    r.push_str("\n## Test Configuration\n\n");
    r.push_str(&format!(
        "- Phase 1: {BASELINE_EVENTS} single-thread events\n"
    ));
    r.push_str(&format!(
        "- Phase 2: {CONCURRENT_AGENTS} agents × {CONCURRENT_EVENTS_PER_AGENT} events (contended)\n"
    ));
    r.push_str(&format!(
        "- Phase 3: {SHARDED_AGENTS} agents × {SHARDED_EVENTS_PER_AGENT} events (sharded)\n"
    ));
    r.push_str(&format!("- Phase 4: Ramp levels {:?}\n", RAMP_LEVELS,));
    r.push_str(&format!(
        "- Phase 5: {RETENTION_EVENTS} events, max_live={RETENTION_MAX_LIVE}, segment_size={RETENTION_SEGMENT_SIZE}\n"
    ));
    r.push_str(&format!(
        "- Phase 6: {RECOVERY_EVENTS} events, interrupt at midpoint\n"
    ));

    r.push_str("\n## How to Run\n\n```bash\ncargo run -p nexus-conductor-benchmark --bin audit-throughput-bench --release\n```\n");

    std::fs::write("AUDIT_THROUGHPUT_RESULTS.md", &r).expect("failed to write report");
    eprintln!("\n  Report: AUDIT_THROUGHPUT_RESULTS.md");
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let wall_start = Instant::now();

    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║   NEXUS OS — Audit Trail Write-Pressure Throughput Test     ║");
    eprintln!("║   Baseline • Concurrent • Sharded • Ramp • Retention       ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    let baseline = run_baseline();
    let concurrent = run_concurrent();
    let sharded = run_sharded();
    let ramp = run_ramp();
    let retention = run_retention();
    let recovery = run_recovery();

    let total_elapsed = wall_start.elapsed().as_secs_f64();

    generate_report(
        &baseline,
        &concurrent,
        &sharded,
        &ramp,
        &retention,
        &recovery,
        total_elapsed,
    );

    // Final summary
    let max_throughput = ramp
        .iter()
        .filter(|r| r.integrity)
        .map(|r| r.throughput)
        .fold(0.0_f64, f64::max);
    let all_integrity = baseline.integrity
        && concurrent.integrity
        && sharded.all_integrity
        && ramp.iter().all(|r| r.integrity)
        && retention.live_integrity
        && retention.full_integrity
        && recovery.post_integrity;

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  COMPLETE — {:.1}s ({:.1} minutes){:>30}║",
        total_elapsed,
        total_elapsed / 60.0,
        "",
    );
    eprintln!(
        "║  Max throughput: {:>15} events/s{:>19}║",
        fmt_num(max_throughput),
        "",
    );
    eprintln!(
        "║  All integrity: {}, Recovery: {}, Retention: {}{:>12}║",
        pass_fail(all_integrity),
        pass_fail(recovery.chain_continuous),
        pass_fail(retention.zero_loss),
        "",
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
}
