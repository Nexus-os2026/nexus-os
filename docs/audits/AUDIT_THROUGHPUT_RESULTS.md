# Nexus OS — Audit Trail Write-Pressure Throughput Stress Test Results

**Date**: 2026-03-25 08:53:06 GMT
**Total wall time**: 4.9s (0.1 minutes)
**Result**: ALL CRITERIA PASSED

## Success Criteria

| Criterion | Target | Actual | Status |
|-----------|--------|--------|--------|
| Sustained throughput | ≥100,000 events/s | 763,698 events/s | PASS |
| P99 latency at 50K events | <1ms (1,000µs) | 4.1µs | PASS |
| Hash chain integrity (all phases) | valid | valid | PASS |
| Retention zero data loss | 0 lost | 0 lost | PASS |
| Recovery after interruption | continuous chain | continuous | PASS |

---

## Phase 1: Single-Thread Baseline

- **Events**: 500000
- **Duration**: 0.86s
- **Throughput**: 579,035 events/s
- **Integrity**: PASS

| Percentile | Latency |
|------------|----------|
| P50 | 1.2µs |
| P90 | 2.8µs |
| P95 | 2.9µs |
| P99 | 5.2µs |
| Max | 322.3µs |

## Phase 2: Concurrent Contended Writes (Arc<Mutex<AuditTrail>>)

- **Agents**: 100 × 1000 events = 100000 total
- **Duration**: 0.28s
- **Throughput**: 355,310 events/s
- **Integrity**: PASS

| Percentile | Latency |
|------------|----------|
| P50 | 1.4µs |
| P95 | 1704.6µs |
| P99 | 2934.8µs |
| Max | 11397.1µs |

## Phase 3: Sharded Throughput (Per-Agent Trails, No Contention)

- **Agents**: 100 × 1000 events = 100000 total
- **Duration**: 0.03s
- **Aggregate throughput**: 3,139,248 events/s
- **All integrity verified**: PASS

| Percentile | Latency |
|------------|----------|
| P50 | 1.9µs |
| P95 | 3.0µs |
| P99 | 14.4µs |
| Max | 18777.3µs |

## Phase 4: Ramp Test — Throughput Ceiling

| Events | Throughput | P50 | P95 | P99 | Max | Integrity |
|--------|-----------|-----|-----|-----|-----|----------|
|    1000 |         763,698 events/s |     1.2µs |     1.4µs |     1.9µs |       29.5µs | PASS |
|   10000 |         561,332 events/s |     1.2µs |     2.9µs |     5.6µs |      422.7µs | PASS |
|   50000 |         615,884 events/s |     1.2µs |     2.8µs |     4.1µs |     1702.3µs | PASS |
|  100000 |         664,829 events/s |     1.2µs |     2.7µs |     3.9µs |     3530.2µs | PASS |
|  200000 |         654,360 events/s |     1.2µs |     2.8µs |     3.9µs |     8027.6µs | PASS |
|  500000 |         644,719 events/s |     1.2µs |     2.8µs |     3.8µs |    16023.1µs | PASS |

## Phase 5: Retention Under Write Pressure

- **Events**: 200000
- **Duration**: 0.59s
- **Throughput**: 336,941 events/s
- **Live events**: 10000
- **Archived segments**: 190
- **Archived events**: 190000
- **Total accounted**: 200000
- **Zero data loss**: PASS
- **Live integrity**: PASS
- **Full integrity**: PASS

| Percentile | Latency |
|------------|----------|
| P50 | 1.2µs |
| P95 | 1.3µs |
| P99 | 1.5µs |
| Max | 21777.9µs |

## Phase 6: Recovery After Interruption

- **Pre-interrupt events**: 5000
- **Post-recovery events**: 10000
- **Pre-interrupt integrity**: PASS
- **Post-recovery integrity**: PASS
- **Chain continuous at boundary**: PASS

## Test Configuration

- Phase 1: 500000 single-thread events
- Phase 2: 100 agents × 1000 events (contended)
- Phase 3: 100 agents × 1000 events (sharded)
- Phase 4: Ramp levels [1000, 10000, 50000, 100000, 200000, 500000]
- Phase 5: 200000 events, max_live=10000, segment_size=1000
- Phase 6: 10000 events, interrupt at midpoint

## How to Run

```bash
cargo run -p nexus-conductor-benchmark --bin audit-throughput-bench --release
```
