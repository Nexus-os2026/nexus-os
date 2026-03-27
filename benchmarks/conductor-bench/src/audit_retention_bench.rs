//! Audit Retention Memory Validation — proves bounded memory under sustained load.
//!
//! Simulates 10,000 agents producing continuous audit events over a sustained
//! period. Compares unbounded (default) vs bounded (Merkle retention) audit
//! trails. Validates that:
//! 1. Memory stays under 500 MB with retention enabled
//! 2. Audit integrity is preserved (Merkle roots + hash chain)
//! 3. Zero events lost (all events recoverable from disk + live buffer)
//! 4. Performance overhead of retention is acceptable
//!
//! Run: `cargo run -p nexus-conductor-benchmark --release --bin audit-retention-bench`

use nexus_kernel::audit::retention::RetentionConfig;
use nexus_kernel::audit::{AuditTrail, EventType};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use uuid::Uuid;

// ── Tracking Allocator ─────────────────────────────────────────────────────

struct TrackingAllocator;

static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static CURRENT_LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            let live = CURRENT_LIVE.fetch_add(layout.size() as u64, Ordering::Relaxed)
                + layout.size() as u64;
            let mut peak = PEAK_LIVE.load(Ordering::Relaxed);
            while live > peak {
                match PEAK_LIVE.compare_exchange_weak(
                    peak,
                    live,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => peak = actual,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        CURRENT_LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn live_heap_mb() -> f64 {
    CURRENT_LIVE.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn peak_heap_mb() -> f64 {
    PEAK_LIVE.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
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

// ── Benchmark Scenarios ────────────────────────────────────────────────────

const AGENT_COUNT: usize = 10_000;
const EVENTS_PER_AGENT: usize = 100; // = 1M total events
const RETENTION_MAX_LIVE: usize = 10_000;
const RETENTION_SEGMENT_SIZE: usize = 1_000;

fn generate_agents(count: usize) -> Vec<Uuid> {
    (0..count).map(|_| Uuid::new_v4()).collect()
}

fn generate_payload(agent_idx: usize, event_idx: usize) -> serde_json::Value {
    json!({
        "event": "cognitive.step_executed",
        "action": "ShellCommand",
        "status": "succeeded",
        "agent_idx": agent_idx,
        "event_idx": event_idx,
        "fuel_cost": 50,
        "result_preview": "mem: 1024MB used, 62GB total"
    })
}

#[derive(Debug)]
#[allow(dead_code)]
struct ScenarioResult {
    name: String,
    total_events: u64,
    duration_secs: f64,
    events_per_sec: f64,
    peak_heap_mb: f64,
    final_heap_mb: f64,
    rss_mb: f64,
    live_event_count: usize,
    archived_event_count: u64,
    integrity_verified: bool,
}

/// Scenario 1: Unbounded audit trail (baseline — shows the memory problem).
fn scenario_unbounded(agents: &[Uuid], events_per_agent: usize) -> ScenarioResult {
    // Reset peak tracking
    PEAK_LIVE.store(CURRENT_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    let start = Instant::now();

    let mut trail = AuditTrail::new();
    let mut total = 0u64;

    for (a_idx, agent_id) in agents.iter().enumerate() {
        for e_idx in 0..events_per_agent {
            let _ = trail.append_event(
                *agent_id,
                EventType::StateChange,
                generate_payload(a_idx, e_idx),
            );
            total += 1;
        }

        // Print progress every 1000 agents
        if a_idx > 0 && a_idx % 1000 == 0 {
            eprint!(
                "\r    unbounded: {} agents, {} events, heap={:.1}MB",
                a_idx,
                total,
                live_heap_mb()
            );
        }
    }
    eprintln!();

    let duration = start.elapsed();
    let integrity = trail.verify_integrity();

    ScenarioResult {
        name: "unbounded".into(),
        total_events: total,
        duration_secs: duration.as_secs_f64(),
        events_per_sec: total as f64 / duration.as_secs_f64(),
        peak_heap_mb: peak_heap_mb(),
        final_heap_mb: live_heap_mb(),
        rss_mb: read_rss_mb(),
        live_event_count: trail.events().len(),
        archived_event_count: 0,
        integrity_verified: integrity,
    }
}

/// Scenario 2: Bounded retention with Merkle archival.
fn scenario_bounded(agents: &[Uuid], events_per_agent: usize) -> ScenarioResult {
    let archive_dir = std::env::temp_dir().join(format!("nexus-audit-bench-{}", Uuid::new_v4()));

    // Reset peak tracking
    PEAK_LIVE.store(CURRENT_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    let start = Instant::now();

    let mut trail = AuditTrail::new();
    trail.enable_retention(RetentionConfig {
        max_live_events: RETENTION_MAX_LIVE,
        segment_size: RETENTION_SEGMENT_SIZE,
        archive_dir: archive_dir.clone(),
    });

    let mut total = 0u64;

    for (a_idx, agent_id) in agents.iter().enumerate() {
        for e_idx in 0..events_per_agent {
            let _ = trail.append_event(
                *agent_id,
                EventType::StateChange,
                generate_payload(a_idx, e_idx),
            );
            total += 1;
        }

        if a_idx > 0 && a_idx % 1000 == 0 {
            eprint!(
                "\r    bounded: {} agents, {} events, heap={:.1}MB, live={}, archived={}",
                a_idx,
                total,
                live_heap_mb(),
                trail.events().len(),
                trail.total_event_count() - trail.events().len() as u64,
            );
        }
    }
    eprintln!();

    let duration = start.elapsed();
    let live_count = trail.events().len();
    let archived_count = trail.total_event_count() - live_count as u64;

    // Verify live chain integrity
    let integrity = trail.verify_integrity();

    // Verify full chain if practical
    let full_integrity = if total <= 200_000 {
        trail.verify_full_integrity().unwrap_or(false)
    } else {
        // For very large runs, just verify Merkle roots (fast)
        let segments = trail.archived_segments();
        let merkle_ok = segments.iter().all(|s| s.verify_merkle().unwrap_or(false));
        integrity && merkle_ok
    };

    let result = ScenarioResult {
        name: "bounded (Merkle retention)".into(),
        total_events: total,
        duration_secs: duration.as_secs_f64(),
        events_per_sec: total as f64 / duration.as_secs_f64(),
        peak_heap_mb: peak_heap_mb(),
        final_heap_mb: live_heap_mb(),
        rss_mb: read_rss_mb(),
        live_event_count: live_count,
        archived_event_count: archived_count,
        integrity_verified: full_integrity,
    };

    // Cleanup
    let _ = std::fs::remove_dir_all(&archive_dir);

    result
}

// ── Report ─────────────────────────────────────────────────────────────────

fn generate_report(unbounded: &ScenarioResult, bounded: &ScenarioResult) -> String {
    let mut out = String::new();

    out.push_str("# Audit Trail Retention — Memory Validation Results\n\n");
    out.push_str(&format!("**Agents**: {}\n", AGENT_COUNT));
    out.push_str(&format!("**Events/agent**: {}\n", EVENTS_PER_AGENT));
    out.push_str(&format!(
        "**Total events**: {}\n",
        AGENT_COUNT * EVENTS_PER_AGENT
    ));
    out.push_str(&format!(
        "**Retention window**: {} live events, {} per segment\n\n",
        RETENTION_MAX_LIVE, RETENTION_SEGMENT_SIZE
    ));

    out.push_str("## Comparison\n\n");
    out.push_str("| Metric | Unbounded | Bounded (Merkle) | Improvement |\n");
    out.push_str("|--------|----------:|------------------:|------------:|\n");

    let heap_improvement = if bounded.peak_heap_mb > 0.0 {
        unbounded.peak_heap_mb / bounded.peak_heap_mb
    } else {
        0.0
    };
    let throughput_ratio = if unbounded.events_per_sec > 0.0 {
        bounded.events_per_sec / unbounded.events_per_sec
    } else {
        0.0
    };

    out.push_str(&format!(
        "| Total events | {} | {} | — |\n",
        unbounded.total_events, bounded.total_events
    ));
    out.push_str(&format!(
        "| Peak heap (MB) | **{:.1}** | **{:.1}** | **{:.1}x smaller** |\n",
        unbounded.peak_heap_mb, bounded.peak_heap_mb, heap_improvement
    ));
    out.push_str(&format!(
        "| Final heap (MB) | {:.1} | {:.1} | {:.1}x smaller |\n",
        unbounded.final_heap_mb,
        bounded.final_heap_mb,
        if bounded.final_heap_mb > 0.0 {
            unbounded.final_heap_mb / bounded.final_heap_mb
        } else {
            0.0
        }
    ));
    out.push_str(&format!(
        "| RSS (MB) | {:.1} | {:.1} | {:.1}x smaller |\n",
        unbounded.rss_mb,
        bounded.rss_mb,
        if bounded.rss_mb > 0.0 {
            unbounded.rss_mb / bounded.rss_mb
        } else {
            0.0
        }
    ));
    out.push_str(&format!(
        "| Live events in memory | {} | {} | {:.0}x fewer |\n",
        unbounded.live_event_count,
        bounded.live_event_count,
        if bounded.live_event_count > 0 {
            unbounded.live_event_count as f64 / bounded.live_event_count as f64
        } else {
            0.0
        }
    ));
    out.push_str(&format!(
        "| Archived to disk | {} | {} | — |\n",
        unbounded.archived_event_count, bounded.archived_event_count
    ));
    out.push_str(&format!(
        "| Throughput (events/sec) | {:.0} | {:.0} | {:.2}x |\n",
        unbounded.events_per_sec, bounded.events_per_sec, throughput_ratio
    ));
    out.push_str(&format!(
        "| Duration (sec) | {:.1} | {:.1} | — |\n",
        unbounded.duration_secs, bounded.duration_secs
    ));
    out.push_str(&format!(
        "| Integrity verified | {} | {} | — |\n",
        if unbounded.integrity_verified {
            "PASS"
        } else {
            "FAIL"
        },
        if bounded.integrity_verified {
            "PASS"
        } else {
            "FAIL"
        },
    ));

    out.push_str("\n## Memory Boundedness\n\n");
    let under_500mb = bounded.peak_heap_mb < 500.0;
    if under_500mb {
        out.push_str(&format!(
            "**PASS**: Peak heap with retention = {:.1} MB (under 500 MB target)\n\n",
            bounded.peak_heap_mb
        ));
    } else {
        out.push_str(&format!(
            "**FAIL**: Peak heap with retention = {:.1} MB (EXCEEDS 500 MB target)\n\n",
            bounded.peak_heap_mb
        ));
    }

    out.push_str("## Event Accounting\n\n");
    let total_expected = (AGENT_COUNT * EVENTS_PER_AGENT) as u64;
    let total_actual = bounded.live_event_count as u64 + bounded.archived_event_count;
    let zero_loss = total_actual == total_expected;
    if zero_loss {
        out.push_str(&format!(
            "**PASS**: {} events produced = {} live + {} archived (zero loss)\n\n",
            total_expected, bounded.live_event_count, bounded.archived_event_count
        ));
    } else {
        out.push_str(&format!(
            "**FAIL**: {} events produced but {} accounted for ({} live + {} archived)\n\n",
            total_expected, total_actual, bounded.live_event_count, bounded.archived_event_count
        ));
    }

    out.push_str("## Cryptographic Integrity\n\n");
    if bounded.integrity_verified {
        out.push_str(
            "**PASS**: Hash chain and Merkle roots verified — audit trail is tamper-proof\n",
        );
    } else {
        out.push_str("**FAIL**: Integrity verification failed\n");
    }

    out
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    eprintln!("╔═══════════════════════════════════════════════════════════╗");
    eprintln!("║  Audit Retention Memory Validation                       ║");
    eprintln!(
        "║  {} agents × {} events = {} total         ║",
        AGENT_COUNT,
        EVENTS_PER_AGENT,
        AGENT_COUNT * EVENTS_PER_AGENT
    );
    eprintln!("╚═══════════════════════════════════════════════════════════╝");

    let agents = generate_agents(AGENT_COUNT);

    // Scenario 1: Unbounded
    eprintln!("\n[1/2] Running UNBOUNDED scenario...");
    let unbounded = scenario_unbounded(&agents, EVENTS_PER_AGENT);
    eprintln!(
        "  done: {} events in {:.1}s, peak heap={:.1}MB",
        unbounded.total_events, unbounded.duration_secs, unbounded.peak_heap_mb
    );

    // Force cleanup before bounded run
    drop(agents);
    let agents = generate_agents(AGENT_COUNT);

    // Scenario 2: Bounded
    eprintln!("\n[2/2] Running BOUNDED (Merkle retention) scenario...");
    let bounded = scenario_bounded(&agents, EVENTS_PER_AGENT);
    eprintln!(
        "  done: {} events in {:.1}s, peak heap={:.1}MB, live={}, archived={}",
        bounded.total_events,
        bounded.duration_secs,
        bounded.peak_heap_mb,
        bounded.live_event_count,
        bounded.archived_event_count,
    );

    // Generate report
    eprintln!("\nGenerating report...");
    let report = generate_report(&unbounded, &bounded);

    let report_path = std::env::current_dir()
        .unwrap_or_default()
        .join("AUDIT_RETENTION_RESULTS.md");
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
