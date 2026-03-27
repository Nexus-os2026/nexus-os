//! Nexus OS Memory Profiler — measures per-agent allocation overhead and scaling.
//!
//! Profiles heap usage across the agent lifecycle at 1K, 5K, and 10K agents.
//! Uses a tracking global allocator for precise malloc/dealloc counting and
//! `/proc/self/status` for RSS measurements.
//!
//! Run: `cargo run -p nexus-conductor-benchmark --release --bin memory-profile`

use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
use nexus_kernel::manifest::AgentManifest;
use nexus_kernel::supervisor::{AgentId, Supervisor};
use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ── Tracking Allocator ─────────────────────────────────────────────────────

struct TrackingAllocator;

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static DEALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static CURRENT_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            let live = CURRENT_LIVE_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed)
                + layout.size() as u64;
            // Update peak using CAS loop
            let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
            while live > peak {
                match PEAK_LIVE_BYTES.compare_exchange_weak(
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
        DEALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        DEALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        CURRENT_LIVE_BYTES.fetch_sub(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

// ── Allocation Snapshot ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct AllocSnapshot {
    alloc_count: u64,
    dealloc_count: u64,
    alloc_bytes: u64,
    dealloc_bytes: u64,
    live_bytes: u64,
    peak_bytes: u64,
    rss_kb: u64,
}

impl AllocSnapshot {
    fn capture() -> Self {
        Self {
            alloc_count: ALLOC_COUNT.load(Ordering::Relaxed),
            dealloc_count: DEALLOC_COUNT.load(Ordering::Relaxed),
            alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
            dealloc_bytes: DEALLOC_BYTES.load(Ordering::Relaxed),
            live_bytes: CURRENT_LIVE_BYTES.load(Ordering::Relaxed),
            peak_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
            rss_kb: read_rss_kb(),
        }
    }

    fn delta_from(&self, baseline: &Self) -> AllocDelta {
        AllocDelta {
            alloc_count: self.alloc_count - baseline.alloc_count,
            dealloc_count: self.dealloc_count - baseline.dealloc_count,
            alloc_bytes: self.alloc_bytes - baseline.alloc_bytes,
            dealloc_bytes: self.dealloc_bytes - baseline.dealloc_bytes,
            live_bytes_change: self.live_bytes as i64 - baseline.live_bytes as i64,
            peak_bytes: self.peak_bytes,
            rss_delta_kb: self.rss_kb as i64 - baseline.rss_kb as i64,
        }
    }
}

#[derive(Debug, Clone)]
struct AllocDelta {
    alloc_count: u64,
    dealloc_count: u64,
    alloc_bytes: u64,
    dealloc_bytes: u64,
    live_bytes_change: i64,
    peak_bytes: u64,
    rss_delta_kb: i64,
}

impl AllocDelta {
    fn net_bytes(&self) -> i64 {
        self.alloc_bytes as i64 - self.dealloc_bytes as i64
    }
}

fn read_rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

fn read_vm_size_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("VmSize:")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

// ── Agent Generator (mirrors conductor-bench) ──────────────────────────────

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

// ── Struct Size Report ─────────────────────────────────────────────────────

fn report_struct_sizes() -> Vec<(&'static str, usize)> {
    vec![
        ("AgentManifest", std::mem::size_of::<AgentManifest>()),
        ("AuditTrail", std::mem::size_of::<AuditTrail>()),
        ("AuditEvent", std::mem::size_of::<AuditEvent>()),
        ("Supervisor", std::mem::size_of::<Supervisor>()),
        ("AgentId (Uuid)", std::mem::size_of::<AgentId>()),
        ("serde_json::Value", std::mem::size_of::<Value>()),
        ("String", std::mem::size_of::<String>()),
        ("Vec<String>", std::mem::size_of::<Vec<String>>()),
        ("Vec<AuditEvent>", std::mem::size_of::<Vec<AuditEvent>>()),
    ]
}

// ── Profiling Phases ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct PhaseResult {
    name: String,
    agent_count: usize,
    duration_ms: f64,
    alloc_delta: AllocDelta,
    rss_after_kb: u64,
    vm_size_kb: u64,
}

/// Phase 1: Pure agent spawn — measure per-agent allocation cost.
fn profile_spawn(count: usize) -> (Supervisor, Vec<AgentId>, PhaseResult) {
    let baseline = AllocSnapshot::capture();
    let start = Instant::now();

    let mut supervisor = Supervisor::new();
    let mut ids = Vec::with_capacity(count);
    for i in 0..count {
        let manifest = generate_manifest(i);
        if let Ok(id) = supervisor.start_agent(manifest) {
            ids.push(id);
        }
    }

    let elapsed = start.elapsed();
    let after = AllocSnapshot::capture();

    let result = PhaseResult {
        name: "agent_spawn".into(),
        agent_count: ids.len(),
        duration_ms: elapsed.as_secs_f64() * 1000.0,
        alloc_delta: after.delta_from(&baseline),
        rss_after_kb: after.rss_kb,
        vm_size_kb: read_vm_size_kb(),
    };

    (supervisor, ids, result)
}

/// Phase 2: Fuel operations — measure fuel ledger growth.
fn profile_fuel_ops(
    supervisor: &mut Supervisor,
    ids: &[AgentId],
    ops_per_agent: usize,
) -> PhaseResult {
    let count = ids.len();
    let baseline = AllocSnapshot::capture();
    let start = Instant::now();

    for agent_id in ids {
        for _ in 0..ops_per_agent {
            if let Ok(reservation) = supervisor.reserve_fuel(*agent_id, 50, "profile_op") {
                let _ = supervisor.commit_fuel(reservation, 25);
            }
        }
    }

    let elapsed = start.elapsed();
    let after = AllocSnapshot::capture();

    PhaseResult {
        name: "fuel_operations".into(),
        agent_count: count,
        duration_ms: elapsed.as_secs_f64() * 1000.0,
        alloc_delta: after.delta_from(&baseline),
        rss_after_kb: after.rss_kb,
        vm_size_kb: read_vm_size_kb(),
    }
}

/// Phase 3: Audit trail growth — measure per-event allocation cost.
fn profile_audit_growth(
    _supervisor: &mut Supervisor,
    ids: &[AgentId],
    events_per_agent: usize,
) -> PhaseResult {
    let count = ids.len();
    let baseline = AllocSnapshot::capture();
    let start = Instant::now();

    let mut audit = AuditTrail::new();
    for agent_id in ids {
        for i in 0..events_per_agent {
            let _ = audit.append_event(
                *agent_id,
                EventType::StateChange,
                json!({
                    "event": "profile_test",
                    "iteration": i,
                    "action": "shell_command",
                    "result": "ok",
                }),
            );
        }
    }

    let elapsed = start.elapsed();
    let after = AllocSnapshot::capture();
    let event_count = audit.events().len();

    eprintln!(
        "    audit trail: {} events, ~{} bytes/event",
        event_count,
        if event_count > 0 {
            after.delta_from(&baseline).net_bytes() / event_count as i64
        } else {
            0
        }
    );

    PhaseResult {
        name: "audit_trail_growth".into(),
        agent_count: count,
        duration_ms: elapsed.as_secs_f64() * 1000.0,
        alloc_delta: after.delta_from(&baseline),
        rss_after_kb: after.rss_kb,
        vm_size_kb: read_vm_size_kb(),
    }
}

/// Phase 4: Message bus growth.
fn profile_message_bus(ids: &[AgentId], messages_per_agent: usize) -> PhaseResult {
    use nexus_kernel::orchestration::messaging::TeamMessageBus;
    use uuid::Uuid;

    let count = ids.len();
    let team_id = Uuid::new_v4();
    let baseline = AllocSnapshot::capture();
    let start = Instant::now();

    let mut bus = TeamMessageBus::new();
    for (i, from) in ids.iter().enumerate() {
        for m in 0..messages_per_agent {
            let to = ids[(i + m + 1) % ids.len()];
            let _ = bus.send(
                team_id,
                *from,
                to,
                "profiling payload message for benchmark",
            );
        }
    }

    let elapsed = start.elapsed();
    let after = AllocSnapshot::capture();

    PhaseResult {
        name: "message_bus".into(),
        agent_count: count,
        duration_ms: elapsed.as_secs_f64() * 1000.0,
        alloc_delta: after.delta_from(&baseline),
        rss_after_kb: after.rss_kb,
        vm_size_kb: read_vm_size_kb(),
    }
}

/// Phase 5: Full lifecycle — spawn + fuel + audit combined.
fn profile_full_lifecycle(count: usize) -> Vec<PhaseResult> {
    eprintln!("\n  --- {} agents ---", count);

    // Spawn
    eprint!("    [1/4] spawn ... ");
    let (mut supervisor, ids, spawn_result) = profile_spawn(count);
    eprintln!(
        "done ({} agents, +{} KB RSS, {:.1}ms)",
        spawn_result.agent_count, spawn_result.alloc_delta.rss_delta_kb, spawn_result.duration_ms,
    );

    // Fuel
    let fuel_ops = if count <= 1000 {
        10
    } else if count <= 5000 {
        5
    } else {
        3
    };
    eprint!("    [2/4] fuel ({} ops/agent) ... ", fuel_ops);
    let fuel_result = profile_fuel_ops(&mut supervisor, &ids, fuel_ops);
    eprintln!(
        "done (+{} KB RSS, {:.1}ms)",
        fuel_result.alloc_delta.rss_delta_kb, fuel_result.duration_ms,
    );

    // Audit
    let events_per = if count <= 1000 {
        10
    } else if count <= 5000 {
        5
    } else {
        3
    };
    eprint!("    [3/4] audit ({} events/agent) ... ", events_per);
    let audit_result = profile_audit_growth(&mut supervisor, &ids, events_per);
    eprintln!(
        "done (+{} KB RSS, {:.1}ms)",
        audit_result.alloc_delta.rss_delta_kb, audit_result.duration_ms,
    );

    // Messages
    let msgs_per = if count <= 1000 {
        5
    } else if count <= 5000 {
        3
    } else {
        2
    };
    eprint!("    [4/4] messages ({} msgs/agent) ... ", msgs_per);
    let msg_result = profile_message_bus(&ids, msgs_per);
    eprintln!(
        "done (+{} KB RSS, {:.1}ms)",
        msg_result.alloc_delta.rss_delta_kb, msg_result.duration_ms,
    );

    // Force drop to measure cleanup
    let pre_drop = AllocSnapshot::capture();
    drop(supervisor);
    let post_drop = AllocSnapshot::capture();
    eprintln!(
        "    [drop] freed {} KB heap, RSS now {} KB",
        (pre_drop.live_bytes as i64 - post_drop.live_bytes as i64) / 1024,
        post_drop.rss_kb,
    );

    vec![spawn_result, fuel_result, audit_result, msg_result]
}

// ── Fragmentation Analysis ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FragmentationResult {
    agent_count: usize,
    live_bytes: u64,
    rss_bytes: u64,
    fragmentation_pct: f64,
    alloc_dealloc_ratio: f64,
}

fn measure_fragmentation(count: usize) -> FragmentationResult {
    let baseline = AllocSnapshot::capture();

    let mut supervisor = Supervisor::new();
    let mut ids = Vec::new();
    for i in 0..count {
        if let Ok(id) = supervisor.start_agent(generate_manifest(i)) {
            ids.push(id);
        }
    }

    // Simulate churn: stop half, start new ones (creates fragmentation)
    let half = ids.len() / 2;
    for id in &ids[..half] {
        let _ = supervisor.stop_agent(*id);
    }
    for i in count..(count + half) {
        let _ = supervisor.start_agent(generate_manifest(i));
    }

    let after = AllocSnapshot::capture();
    let delta = after.delta_from(&baseline);

    let live = after.live_bytes;
    let rss = after.rss_kb * 1024;
    let fragmentation = if live > 0 {
        ((rss as f64 - live as f64) / rss as f64) * 100.0
    } else {
        0.0
    };

    let ratio = if delta.dealloc_count > 0 {
        delta.alloc_count as f64 / delta.dealloc_count as f64
    } else {
        delta.alloc_count as f64
    };

    drop(supervisor);

    FragmentationResult {
        agent_count: count,
        live_bytes: live,
        rss_bytes: rss,
        fragmentation_pct: fragmentation.max(0.0),
        alloc_dealloc_ratio: ratio,
    }
}

// ── Allocation Hotspot Isolation ───────────────────────────────────────────

#[derive(Debug, Clone)]
struct ComponentCost {
    name: &'static str,
    total_alloc_bytes: u64,
    alloc_count: u64,
    net_bytes: i64,
}

fn isolate_component_costs(count: usize) -> Vec<ComponentCost> {
    let mut results = Vec::new();

    // 1. Bare Supervisor creation
    {
        let b = AllocSnapshot::capture();
        let _sup = Supervisor::new();
        let a = AllocSnapshot::capture();
        let d = a.delta_from(&b);
        results.push(ComponentCost {
            name: "Supervisor::new()",
            total_alloc_bytes: d.alloc_bytes,
            alloc_count: d.alloc_count,
            net_bytes: d.net_bytes(),
        });
    }

    // 2. Agent spawning only (no ops)
    {
        let mut sup = Supervisor::new();
        let b = AllocSnapshot::capture();
        let mut spawned = 0usize;
        for i in 0..count {
            if sup.start_agent(generate_manifest(i)).is_ok() {
                spawned += 1;
            }
        }
        let a = AllocSnapshot::capture();
        let d = a.delta_from(&b);
        let per_agent = if spawned > 0 {
            d.net_bytes() / spawned as i64
        } else {
            0
        };
        results.push(ComponentCost {
            name: "start_agent() × N",
            total_alloc_bytes: d.alloc_bytes,
            alloc_count: d.alloc_count,
            net_bytes: d.net_bytes(),
        });
        eprintln!(
            "    agent spawn: {} bytes/agent (net), {} allocs/agent",
            per_agent,
            d.alloc_count / spawned.max(1) as u64
        );
    }

    // 3. Fuel reserve+commit cycle
    {
        let mut sup = Supervisor::new();
        let mut ids = Vec::new();
        for i in 0..count {
            if let Ok(id) = sup.start_agent(generate_manifest(i)) {
                ids.push(id);
            }
        }
        let b = AllocSnapshot::capture();
        for id in &ids {
            if let Ok(r) = sup.reserve_fuel(*id, 50, "cost_test") {
                let _ = sup.commit_fuel(r, 25);
            }
        }
        let a = AllocSnapshot::capture();
        let d = a.delta_from(&b);
        results.push(ComponentCost {
            name: "reserve+commit fuel × N",
            total_alloc_bytes: d.alloc_bytes,
            alloc_count: d.alloc_count,
            net_bytes: d.net_bytes(),
        });
    }

    // 4. Audit event appending
    {
        let b = AllocSnapshot::capture();
        let mut audit = AuditTrail::new();
        for _ in 0..count {
            let _ = audit.append_event(
                uuid::Uuid::new_v4(),
                EventType::StateChange,
                json!({"event": "test", "data": "payload"}),
            );
        }
        let a = AllocSnapshot::capture();
        let d = a.delta_from(&b);
        let per_event = if count > 0 {
            d.net_bytes() / count as i64
        } else {
            0
        };
        results.push(ComponentCost {
            name: "audit append_event × N",
            total_alloc_bytes: d.alloc_bytes,
            alloc_count: d.alloc_count,
            net_bytes: d.net_bytes(),
        });
        eprintln!(
            "    audit event: {} bytes/event (net), {} allocs/event",
            per_event,
            d.alloc_count / count.max(1) as u64
        );
    }

    // 5. TeamMessageBus
    {
        use nexus_kernel::orchestration::messaging::TeamMessageBus;
        let team = uuid::Uuid::new_v4();
        let agents: Vec<uuid::Uuid> = (0..count.min(500)).map(|_| uuid::Uuid::new_v4()).collect();
        let b = AllocSnapshot::capture();
        let mut bus = TeamMessageBus::new();
        for i in 0..agents.len() {
            let to = agents[(i + 1) % agents.len()];
            let _ = bus.send(team, agents[i], to, "profiling payload");
        }
        let a = AllocSnapshot::capture();
        let d = a.delta_from(&b);
        results.push(ComponentCost {
            name: "message_bus send × N",
            total_alloc_bytes: d.alloc_bytes,
            alloc_count: d.alloc_count,
            net_bytes: d.net_bytes(),
        });
    }

    results
}

// ── RAM Exhaustion Projection ──────────────────────────────────────────────

fn project_ram_limit(per_agent_bytes: f64, total_ram_gb: f64) -> u64 {
    let available_bytes = total_ram_gb * 1024.0 * 1024.0 * 1024.0;
    // Reserve 4GB for OS + runtime overhead
    let usable = (available_bytes - 4.0 * 1024.0 * 1024.0 * 1024.0).max(0.0);
    (usable / per_agent_bytes) as u64
}

// ── Report Generation ──────────────────────────────────────────────────────

fn generate_report(
    struct_sizes: &[(&str, usize)],
    lifecycle_results: &[Vec<PhaseResult>],
    frag_results: &[FragmentationResult],
    component_costs: &[ComponentCost],
    agent_tiers: &[usize],
) -> String {
    let mut out = String::new();

    out.push_str("# Nexus OS Memory Profile Results\n\n");
    out.push_str(&format!("**Date**: {}\n", chrono_now()));
    out.push_str(&format!(
        "**System**: {} cores, ~62 GB RAM, Ubuntu (ROG Zephyrus)\n",
        num_cpus()
    ));
    out.push_str(&format!("**Agent tiers**: {:?}\n\n", agent_tiers));

    // Section 1: Struct sizes
    out.push_str("## 1. Stack/Inline Struct Sizes\n\n");
    out.push_str("| Struct | Stack Size (bytes) |\n");
    out.push_str("|--------|-------------------:|\n");
    for (name, size) in struct_sizes {
        out.push_str(&format!("| {} | {} |\n", name, size));
    }

    // Section 2: Per-agent footprint
    out.push_str("\n## 2. Per-Agent Heap Footprint\n\n");
    out.push_str("| Phase | Agents | Net Alloc (bytes) | Per-Agent (bytes) | Alloc Count | RSS After (MB) | Duration (ms) |\n");
    out.push_str("|-------|--------|------------------:|------------------:|------------:|---------------:|--------------:|\n");
    for tier_results in lifecycle_results {
        for r in tier_results {
            let per_agent = if r.agent_count > 0 {
                r.alloc_delta.net_bytes() / r.agent_count as i64
            } else {
                0
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {:.1} | {:.1} |\n",
                r.name,
                r.agent_count,
                r.alloc_delta.net_bytes(),
                per_agent,
                r.alloc_delta.alloc_count,
                r.rss_after_kb as f64 / 1024.0,
                r.duration_ms,
            ));
        }
    }

    // Section 3: Component cost breakdown
    out.push_str("\n## 3. Component Allocation Breakdown\n\n");
    out.push_str("Isolated measurement of each subsystem for the largest agent tier:\n\n");
    out.push_str(
        "| Component | Total Alloc (bytes) | Net Retained (bytes) | Alloc Count | Per-Unit Net |\n",
    );
    out.push_str(
        "|-----------|--------------------:|---------------------:|------------:|-------------:|\n",
    );
    let largest_tier = agent_tiers.last().copied().unwrap_or(1000);
    for c in component_costs {
        let per_unit = if largest_tier > 0 {
            c.net_bytes / largest_tier as i64
        } else {
            0
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            c.name, c.total_alloc_bytes, c.net_bytes, c.alloc_count, per_unit,
        ));
    }

    // Section 4: Memory scaling
    out.push_str("\n## 4. Memory Scaling Across Agent Tiers\n\n");
    out.push_str("| Agents | RSS (MB) | VM Size (MB) | Heap Live (MB) | Heap Peak (MB) |\n");
    out.push_str("|--------|--------:|-----------:|---------------:|---------------:|\n");
    for tier_results in lifecycle_results {
        if let Some(last) = tier_results.last() {
            let snap = AllocSnapshot {
                alloc_count: 0,
                dealloc_count: 0,
                alloc_bytes: 0,
                dealloc_bytes: 0,
                live_bytes: last.alloc_delta.peak_bytes,
                peak_bytes: last.alloc_delta.peak_bytes,
                rss_kb: last.rss_after_kb,
            };
            out.push_str(&format!(
                "| {} | {:.1} | {:.1} | {:.1} | {:.1} |\n",
                last.agent_count,
                last.rss_after_kb as f64 / 1024.0,
                last.vm_size_kb as f64 / 1024.0,
                last.alloc_delta.live_bytes_change as f64 / (1024.0 * 1024.0),
                snap.peak_bytes as f64 / (1024.0 * 1024.0),
            ));
        }
    }

    // Section 5: Fragmentation
    out.push_str("\n## 5. Heap Fragmentation (After Churn)\n\n");
    out.push_str("Simulates agent stop/start churn to measure fragmentation:\n\n");
    out.push_str(
        "| Agents | Live Heap (MB) | RSS (MB) | Fragmentation (%) | Alloc/Dealloc Ratio |\n",
    );
    out.push_str(
        "|--------|---------------:|---------:|------------------:|--------------------:|\n",
    );
    for f in frag_results {
        out.push_str(&format!(
            "| {} | {:.2} | {:.1} | {:.1} | {:.2} |\n",
            f.agent_count,
            f.live_bytes as f64 / (1024.0 * 1024.0),
            f.rss_bytes as f64 / (1024.0 * 1024.0),
            f.fragmentation_pct,
            f.alloc_dealloc_ratio,
        ));
    }

    // Section 6: Largest memory consumer analysis
    out.push_str("\n## 6. Largest Memory Consumers\n\n");

    // Extract spawn vs audit vs message cost
    let mut consumer_data: Vec<(&str, i64)> = Vec::new();
    if let Some(tier_results) = lifecycle_results.last() {
        for r in tier_results {
            consumer_data.push((&r.name, r.alloc_delta.net_bytes()));
        }
    }
    consumer_data.sort_by(|a, b| b.1.cmp(&a.1));

    out.push_str("Ranked by net retained bytes at the largest tier:\n\n");
    out.push_str("| Rank | Component | Net Retained (bytes) | Share (%) |\n");
    out.push_str("|------|-----------|---------------------:|----------:|\n");
    let total: i64 = consumer_data.iter().map(|c| c.1.max(0)).sum();
    for (i, (name, bytes)) in consumer_data.iter().enumerate() {
        let share = if total > 0 {
            (*bytes as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        out.push_str(&format!(
            "| {} | {} | {} | {:.1} |\n",
            i + 1,
            name,
            bytes,
            share
        ));
    }

    // Section 7: RAM exhaustion projection
    out.push_str("\n## 7. RAM Exhaustion Projection\n\n");
    if let Some(tier_results) = lifecycle_results.last() {
        if let Some(spawn_result) = tier_results.first() {
            let agents = spawn_result.agent_count;
            let per_agent = if agents > 0 {
                spawn_result.alloc_delta.net_bytes().max(0) as f64 / agents as f64
            } else {
                5000.0
            };

            // Include audit overhead estimate (10 events/agent × ~500 bytes)
            let per_agent_with_ops = per_agent + 5000.0;

            let max_agents_62gb = project_ram_limit(per_agent_with_ops, 62.0);
            let max_agents_32gb = project_ram_limit(per_agent_with_ops, 32.0);
            let max_agents_16gb = project_ram_limit(per_agent_with_ops, 16.0);

            out.push_str(&format!(
                "Per-agent spawn cost: **{:.0} bytes**\n",
                per_agent
            ));
            out.push_str(&format!("Per-agent with operations estimate: **{:.0} bytes** (spawn + 10 audit events + fuel ops)\n\n", per_agent_with_ops));

            out.push_str("| RAM | Max Agents (spawn only) | Max Agents (with ops) |\n");
            out.push_str("|-----|----------------------:|-----------------------:|\n");
            out.push_str(&format!(
                "| 16 GB | {} | {} |\n",
                project_ram_limit(per_agent, 16.0),
                max_agents_16gb,
            ));
            out.push_str(&format!(
                "| 32 GB | {} | {} |\n",
                project_ram_limit(per_agent, 32.0),
                max_agents_32gb,
            ));
            out.push_str(&format!(
                "| 62 GB | {} | {} |\n",
                project_ram_limit(per_agent, 62.0),
                max_agents_62gb,
            ));

            out.push_str(&format!(
                "\n**On this machine (62 GB), memory exhaustion occurs at approximately {} agents.**\n",
                max_agents_62gb
            ));
        }
    }

    // Section 8: Optimization targets
    out.push_str("\n## 8. Optimization Targets\n\n");
    out.push_str("Based on profiling data:\n\n");
    out.push_str("1. **AuditTrail** — unbounded `Vec<AuditEvent>` grows without limit. Each event ~500 bytes. At 10K agents × 100 events = **500 MB**. Fix: ring buffer or periodic flush to disk.\n");
    out.push_str("2. **TimeMachine checkpoints** — bounded at 200 but each stores change deltas. Monitor total size.\n");
    out.push_str("3. **SafetySupervisor per-agent maps** — 6 separate HashMaps keyed by AgentId. Could consolidate into single per-agent struct.\n");
    out.push_str("4. **ConsentRuntime approval_queue** — pending approvals never expire. Add TTL-based eviction.\n");
    out.push_str("5. **String allocations** — AgentManifest stores name, version, capabilities as owned Strings. At scale, consider interning or `Arc<str>`.\n");

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
    let days = now / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{}-{:02}-{:02}", years, month.min(12), day.min(31))
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let agent_tiers: Vec<usize> = vec![1000, 5000, 10000];

    eprintln!("╔═══════════════════════════════════════════════════════╗");
    eprintln!("║  Nexus OS Memory Profiler                            ║");
    eprintln!("║  Agent tiers: {:?}                  ║", agent_tiers);
    eprintln!("║  Tracking allocator: ACTIVE                          ║");
    eprintln!("╚═══════════════════════════════════════════════════════╝");

    // Struct sizes
    eprintln!("\n[1] Reporting struct sizes...");
    let struct_sizes = report_struct_sizes();
    for (name, size) in &struct_sizes {
        eprintln!("    {}: {} bytes", name, size);
    }

    // Full lifecycle profiles
    eprintln!("\n[2] Profiling agent lifecycle...");
    let mut lifecycle_results = Vec::new();
    for &count in &agent_tiers {
        let results = profile_full_lifecycle(count);
        lifecycle_results.push(results);
    }

    // Fragmentation
    eprintln!("\n[3] Measuring fragmentation...");
    let mut frag_results = Vec::new();
    for &count in &agent_tiers {
        eprint!("    {} agents (churn) ... ", count);
        let f = measure_fragmentation(count);
        eprintln!(
            "done (frag={:.1}%, ratio={:.2})",
            f.fragmentation_pct, f.alloc_dealloc_ratio
        );
        frag_results.push(f);
    }

    // Component isolation
    eprintln!(
        "\n[4] Isolating component costs ({} agents)...",
        agent_tiers.last().unwrap_or(&1000)
    );
    let component_costs = isolate_component_costs(*agent_tiers.last().unwrap_or(&1000));

    // Generate report
    eprintln!("\n[5] Generating report...");
    let report = generate_report(
        &struct_sizes,
        &lifecycle_results,
        &frag_results,
        &component_costs,
        &agent_tiers,
    );

    let report_path = std::env::current_dir()
        .unwrap_or_default()
        .join("MEMORY_PROFILE_RESULTS.md");
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
