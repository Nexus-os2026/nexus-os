# Nexus OS Memory Profile Results

**Date**: 2026-04-08
**System**: 16 cores, ~62 GB RAM, Ubuntu (ROG Zephyrus)
**Agent tiers**: [1000, 5000, 10000]

## 1. Stack/Inline Struct Sizes

| Struct | Stack Size (bytes) |
|--------|-------------------:|
| AgentManifest | 320 |
| AuditTrail | 32 |
| AuditEvent | 128 |
| Supervisor | 1304 |
| AgentId (Uuid) | 16 |
| serde_json::Value | 32 |
| String | 24 |
| Vec<String> | 24 |
| Vec<AuditEvent> | 24 |

## 2. Per-Agent Heap Footprint

| Phase | Agents | Net Alloc (bytes) | Per-Agent (bytes) | Alloc Count | RSS After (MB) | Duration (ms) |
|-------|--------|------------------:|------------------:|------------:|---------------:|--------------:|
| agent_spawn | 1000 | 6702504 | 6702 | 125334 | 9.8 | 11.8 |
| fuel_operations | 1000 | 10802964 | 10802 | 481009 | 22.8 | 38.5 |
| audit_trail_growth | 1000 | 10227152 | 10227 | 240020 | 33.8 | 17.2 |
| message_bus | 1000 | 6725840 | 6725 | 265033 | 29.6 | 16.1 |
| agent_spawn | 5000 | 30754856 | 6150 | 626340 | 36.7 | 45.8 |
| fuel_operations | 5000 | 29449556 | 5889 | 1205009 | 69.1 | 98.6 |
| audit_trail_growth | 5000 | 24519304 | 4903 | 600021 | 96.5 | 42.0 |
| message_bus | 5000 | 18276384 | 3655 | 795035 | 89.3 | 46.4 |
| agent_spawn | 10000 | 61361992 | 6136 | 1252593 | 80.3 | 82.5 |
| fuel_operations | 10000 | 31792404 | 3179 | 1440008 | 109.1 | 112.5 |
| audit_trail_growth | 10000 | 28584304 | 2858 | 720021 | 142.1 | 50.7 |
| message_bus | 10000 | 26902472 | 2690 | 1060037 | 136.1 | 61.4 |

## 3. Component Allocation Breakdown

Isolated measurement of each subsystem for the largest agent tier:

| Component | Total Alloc (bytes) | Net Retained (bytes) | Alloc Count | Per-Unit Net |
|-----------|--------------------:|---------------------:|------------:|-------------:|
| Supervisor::new() | 6288 | 1916 | 45 | 0 |
| start_agent() × N | 142262450 | 61200076 | 1252554 | 6120 |
| reserve+commit fuel × N | 43182672 | 13392404 | 480008 | 1339 |
| audit append_event × N | 24357856 | 9897152 | 180020 | 989 |
| message_bus send × N | 1861500 | 590580 | 25525 | 59 |

## 4. Memory Scaling Across Agent Tiers

| Agents | RSS (MB) | VM Size (MB) | Heap Live (MB) | Heap Peak (MB) |
|--------|--------:|-----------:|---------------:|---------------:|
| 1000 | 29.6 | 32.0 | 6.4 | 26.5 |
| 5000 | 89.3 | 94.1 | 17.4 | 80.8 |
| 10000 | 136.1 | 140.9 | 25.7 | 116.1 |

## 5. Heap Fragmentation (After Churn)

Simulates agent stop/start churn to measure fragmentation:

| Agents | Live Heap (MB) | RSS (MB) | Fragmentation (%) | Alloc/Dealloc Ratio |
|--------|---------------:|---------:|------------------:|--------------------:|
| 1000 | 9.42 | 136.1 | 93.1 | 1.66 |
| 5000 | 50.26 | 110.1 | 54.4 | 1.65 |
| 10000 | 100.39 | 128.5 | 21.9 | 1.64 |

## 6. Largest Memory Consumers

Ranked by net retained bytes at the largest tier:

| Rank | Component | Net Retained (bytes) | Share (%) |
|------|-----------|---------------------:|----------:|
| 1 | agent_spawn | 61361992 | 41.3 |
| 2 | fuel_operations | 31792404 | 21.4 |
| 3 | audit_trail_growth | 28584304 | 19.2 |
| 4 | message_bus | 26902472 | 18.1 |

## 7. RAM Exhaustion Projection

Per-agent spawn cost: **6136 bytes**
Per-agent with operations estimate: **11136 bytes** (spawn + 10 audit events + fuel ops)

| RAM | Max Agents (spawn only) | Max Agents (with ops) |
|-----|----------------------:|-----------------------:|
| 16 GB | 2099818 | 1157028 |
| 32 GB | 4899575 | 2699733 |
| 62 GB | 10149120 | 5592305 |

**On this machine (62 GB), memory exhaustion occurs at approximately 5592305 agents.**

## 8. Optimization Targets

Based on profiling data:

1. **AuditTrail** — unbounded `Vec<AuditEvent>` grows without limit. Each event ~500 bytes. At 10K agents × 100 events = **500 MB**. Fix: ring buffer or periodic flush to disk.
2. **TimeMachine checkpoints** — bounded at 200 but each stores change deltas. Monitor total size.
3. **SafetySupervisor per-agent maps** — 6 separate HashMaps keyed by AgentId. Could consolidate into single per-agent struct.
4. **ConsentRuntime approval_queue** — pending approvals never expire. Add TTL-based eviction.
5. **String allocations** — AgentManifest stores name, version, capabilities as owned Strings. At scale, consider interning or `Arc<str>`.
