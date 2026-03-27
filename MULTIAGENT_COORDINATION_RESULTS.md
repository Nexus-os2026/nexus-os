# Nexus OS — Multi-Agent Coordination Stress Test Results

**Date**: 2026-03-25 14:27:12 GMT
**Total wall time**: 3.9s (0.1 minutes)
**Max agents spawned**: 100,000
**Peak RSS**: 784 MB
**Result**: ALL CRITERIA PASSED

## Success Criteria

| Criterion | Target | Actual | Status |
|-----------|--------|--------|--------|
| Decision P99 at 50K agents | <1ms (1,000µs) | 1.1µs | PASS |
| Memory at 100K agents | <50GB | 692.7 MB | PASS |
| Message throughput at 50K | ≥1M msg/s | 11,681,452 msg/s | PASS |
| SwarmCoordinator consensus | converges | 1000/1000 | PASS |
| Governance scales at 50K | >100K checks/s | 10,961,815 checks/s | PASS |

---

## Phase 1: Spawn Ramp

| Target | Spawned | Duration | Spawn Rate | RSS |
|--------|---------|----------|------------|-----|
| 10,000 | 10,000 | 0.12s | 83,737 agents/s | 75 MB |
| 25,000 | 15,000 | 0.18s | 83,600 agents/s | 176 MB |
| 50,000 | 25,000 | 0.31s | 81,374 agents/s | 348 MB |
| 100,000 | 50,000 | 0.63s | 79,861 agents/s | 693 MB |

## Phase 2: Fuel Contention (Lock-Free CAS)

| Agents | Total Ops | Throughput | P50 | P95 | P99 | Max | Errors |
|--------|-----------|-----------|-----|-----|-----|-----|--------|
| 10,000 | 30,000 | 6,328,727 ops/s | 0.6µs | 1.4µs | 1.8µs | 17.7µs | 0 |
| 25,000 | 75,000 | 8,844,835 ops/s | 0.6µs | 0.9µs | 1.1µs | 71.9µs | 0 |
| 50,000 | 150,000 | 9,670,474 ops/s | 0.5µs | 0.8µs | 0.9µs | 57.8µs | 0 |
| 100,000 | 300,000 | 9,683,809 ops/s | 0.6µs | 0.8µs | 1.1µs | 385.8µs | 0 |

## Phase 3: Message Bus Throughput (Lock-Free SegQueue)

| Agents | Messages | Throughput | P50 | P95 | P99 | Max |
|--------|----------|-----------|-----|-----|-----|-----|
| 10,000 | 50,000 | 10,944,904 msg/s | 0.1µs | 0.2µs | 0.5µs | 852.5µs |
| 25,000 | 125,000 | 11,290,262 msg/s | 0.1µs | 0.3µs | 4.2µs | 1290.8µs |
| 50,000 | 250,000 | 11,681,452 msg/s | 0.1µs | 0.3µs | 3.7µs | 1878.9µs |
| 100,000 | 500,000 | 11,258,772 msg/s | 0.1µs | 0.3µs | 3.8µs | 3159.7µs |

## Phase 4: Governance Gate Checks

| Agents | Checks | Throughput | P50 | P95 | P99 | Max |
|--------|--------|-----------|-----|-----|-----|-----|
| 10,000 | 40,000 | 8,887,765 checks/s | 0.0µs | 0.1µs | 0.1µs | 5.7µs |
| 25,000 | 100,000 | 10,740,076 checks/s | 0.0µs | 0.0µs | 0.1µs | 8.5µs |
| 50,000 | 200,000 | 10,961,815 checks/s | 0.0µs | 0.0µs | 0.1µs | 69.1µs |
| 100,000 | 400,000 | 11,026,138 checks/s | 0.0µs | 0.0µs | 0.1µs | 16.2µs |

## Phase 5: Realistic Coordination Mix

| Agents | Total Ops | Throughput | P50 | P95 | P99 | RSS | Fuel | Msg | Gov |
|--------|-----------|-----------|-----|-----|-----|-----|------|-----|-----|
| 10,000 | 100,000 | 8,866,665 ops/s | 0.2µs | 1.0µs | 1.3µs | 776MB | 30,000 | 50,000 | 20,000 |
| 25,000 | 250,000 | 10,022,860 ops/s | 0.2µs | 0.9µs | 1.2µs | 776MB | 75,000 | 125,000 | 50,000 |
| 50,000 | 500,000 | 10,455,894 ops/s | 0.2µs | 0.9µs | 1.1µs | 776MB | 150,000 | 250,000 | 100,000 |
| 100,000 | 1,000,000 | 10,645,169 ops/s | 0.2µs | 0.9µs | 1.2µs | 784MB | 300,000 | 500,000 | 200,000 |

## Phase 6: SwarmCoordinator Consensus

- **Runs**: 1000
- **Total evaluations**: 24,000
- **Convergences**: 1000/1000
- **Avg generations**: 3.0
- **Evaluation throughput**: 3,290,854 evals/s
- **Duration**: 0.01s

## Phase 7: Coordination Ceiling

| Target | Spawned | Throughput | P50 | P99 | RSS | Status |
|--------|---------|-----------|-----|-----|-----|--------|
| 150,000 | 50,000 | 6,218,931 ops/s | 0.5µs | 1.2µs | 1157MB | OK |
| 200,000 | 50,000 | 6,527,380 ops/s | 0.5µs | 1.2µs | 1445MB | OK |
| 250,000 | 50,000 | 6,741,404 ops/s | 0.5µs | 1.3µs | 1955MB | OK |

## Test Configuration

- Scale levels: [10000, 25000, 50000, 100000]
- Threads: 16
- Fuel ops/agent: 3
- Message rounds/agent: 5
- Governance checks/agent: 4
- Coordination mix: 30% fuel / 20% governance / 50% messaging
- Ceiling levels: 150K, 200K, 250K

## How to Run

```bash
cargo run -p nexus-conductor-benchmark --bin multiagent-coordination-bench --release
```
