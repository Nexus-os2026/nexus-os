# Nexus OS — Genesis Protocol Stress Test Results

**Date**: 2026-03-25 15:00:58 GMT
**Total wall time**: 0.2s (0.0 minutes)
**Total agents created**: 14,023
**Result**: ALL CRITERIA PASSED

## Success Criteria

| Criterion | Target | Actual | Status |
|-----------|--------|--------|--------|
| Dynamic agents created | ≥5,000 | 8,296 | PASS |
| Lineage integrity | valid, 0 orphans | valid=true, orphans=0 | PASS |
| Zero capability escalation | all caught | all caught | PASS |
| Fitness regression | ≤5% | 0.0% | PASS |
| Adversarial attempts caught | all caught | all caught | PASS |
| Memory under 10GB | <10GB | 14.2MB / 49.8MB | PASS |
| Genesis throughput | ≥1,000 agents/s | 311,461 agents/s | PASS |

---

## Phase 1: Seed Population

- **Agents**: 10
- **Duration**: 0.000s

## Phase 2: Multi-Generational Genesis

| Gen | Created | Mean Fit | Max Fit | Min Fit | Violations | Escalations Caught | Time |
|-----|---------|----------|---------|---------|------------|-------------------|------|
| 1 | 50 | 5.56 | 5.74 | 5.34 | 0 | 0 | 0ms |
| 2 | 150 | 5.68 | 5.74 | 5.64 | 0 | 0 | 0ms |
| 3 | 225 | 5.72 | 5.80 | 5.70 | 0 | 0 | 1ms |
| 4 | 170 | 5.77 | 5.83 | 5.74 | 0 | 0 | 0ms |
| 5 | 130 | 5.82 | 5.87 | 5.80 | 0 | 0 | 0ms |

## Phase 3: Lineage Integrity

- **Total records**: 735
- **Chain valid**: true
- **Orphans**: 0
- **Max depth**: 5
- **Unique ancestors**: 10

## Phase 4: Governance Enforcement

- **L0-L3 rejections**: 4/4
- **L4-L6 approvals**: 3/3
- **Escalation blocked**: 3/3
- **All enforced**: PASS

## Phase 5: Scale Genesis

- **Total agents**: 8,296
- **Generations**: 5
- **Duration**: 0.03s
- **Throughput**: 311,461 agents/s
- **Lineage valid**: true
- **RSS**: 14MB

| Gen | Created | Mean Fit | Max Fit | Time |
|-----|---------|----------|---------|------|
| 1 | 1,200 | 5.24 | 5.79 | 3ms |
| 2 | 1,956 | 5.73 | 5.82 | 5ms |
| 3 | 1,596 | 5.81 | 5.86 | 4ms |
| 4 | 1,308 | 5.86 | 5.91 | 2ms |
| 5 | 2,136 | 5.91 | 5.95 | 6ms |

## Phase 6: Adversarial Genesis

| Attack Type | Attempted | Rejected | Status |
|-------------|-----------|----------|--------|
| Malformed specs | 3 | 3 | PASS |
| Privilege escalation | 10 | 10 | PASS |
| Self-replication bomb | 20 | 20 | PASS |

## Phase 7: System Stability

- **Agents spawned**: 4,992
- **Spawn throughput**: 46,983 agents/s
- **RSS**: 50MB
- **Audit events**: 14,976
- **Audit integrity**: PASS

## Test Configuration

- Seed agents: 10 (L4-L6)
- Small test: 5 generations × 5 children/parent
- Scale test: 100 parents × 12 children × 5 generations
- Stability test: 5,000 agents via ConcurrentSupervisor (16 threads)
- Adversarial: 3 malformed, 10 escalation, 20 replication bomb

## How to Run

```bash
cargo run -p nexus-conductor-benchmark --bin genesis-protocol-bench --release
```
