# Nexus OS — Darwin Evolution Drift Stress Test Results

**Date**: 2026-03-25 08:43:23 GMT
**Total wall time**: 19.2s (0.3 minutes)
**Result**: ALL CRITERIA PASSED

## Success Criteria

| Criterion | Target | Actual | Status |
|-----------|--------|--------|--------|
| Fitness regression | ≤5% | 0.0% | PASS |
| Governance violations | 0 | 0 | PASS |
| Genome diversity | ≥20% | 52.0% | PASS |
| Audit trail integrity | valid | valid | PASS |
| Threat detection (5/5) | all caught | 5/5 | PASS |

---

## Phase 1: Evolution Fitness Curves (50 Agents × 20 Generations)

- **Initial mean fitness**: 6.15
- **Final mean fitness**: 7.47
- **Peak fitness**: 7.63
- **Improvement**: +21.5%
- **Max single-gen regression**: 0.0%

| Gen | Pop | Mean Fit | Max Fit | Min Fit | Diversity | Violations | Time |
|-----|-----|----------|---------|---------|-----------|------------|------|
|  0 |  50 | 6.15 | 6.58 | 5.76 | 14.0% | 0 | 0ms |
|  1 |  50 | 6.41 | 6.86 | 6.15 | 52.0% | 0 | 0ms |
|  2 |  50 | 6.58 | 6.89 | 6.39 | 68.0% | 0 | 0ms |
|  3 |  50 | 6.68 | 6.96 | 6.52 | 82.0% | 0 | 0ms |
|  4 |  50 | 6.78 | 7.10 | 6.51 | 100.0% | 0 | 0ms |
|  5 |  50 | 6.87 | 7.19 | 6.67 | 98.0% | 0 | 0ms |
|  6 |  50 | 6.94 | 7.19 | 6.79 | 98.0% | 0 | 0ms |
|  7 |  50 | 7.00 | 7.19 | 6.76 | 100.0% | 0 | 0ms |
|  8 |  50 | 7.06 | 7.24 | 6.78 | 100.0% | 0 | 0ms |
|  9 |  50 | 7.10 | 7.24 | 6.89 | 100.0% | 0 | 0ms |
| 10 |  50 | 7.15 | 7.29 | 6.83 | 100.0% | 0 | 0ms |
| 11 |  50 | 7.20 | 7.36 | 7.02 | 98.0% | 0 | 0ms |
| 12 |  50 | 7.25 | 7.39 | 6.99 | 100.0% | 0 | 0ms |
| 13 |  50 | 7.29 | 7.45 | 7.14 | 100.0% | 0 | 0ms |
| 14 |  50 | 7.32 | 7.45 | 7.11 | 100.0% | 0 | 0ms |
| 15 |  50 | 7.36 | 7.56 | 7.15 | 100.0% | 0 | 0ms |
| 16 |  50 | 7.39 | 7.57 | 7.20 | 100.0% | 0 | 0ms |
| 17 |  50 | 7.42 | 7.58 | 7.22 | 100.0% | 0 | 0ms |
| 18 |  50 | 7.44 | 7.58 | 7.25 | 100.0% | 0 | 0ms |
| 19 |  50 | 7.47 | 7.63 | 7.28 | 98.0% | 0 | 0ms |

## Phase 2: Plan Evolution Engine Stress

- **Runs**: 50
- **Avg improvement**: 62.50
- **Avg generations**: 3.0
- **Avg defense rate**: 100.0%
- **Convergences**: 50
- **Adversarial rejections**: 0

## Phase 3: Adversarial Arena Results

### Cognitive Arena

- **Total challenges**: 525
- **Defense rate**: 99.0%

| Threat Category | Detected |
|-----------------|----------|
| Prompt Injection | PASS |
| Capability Escalation | PASS |
| Data Exfiltration | PASS |
| Resource Exhaustion | PASS |
| Governance Bypass | PASS |

### Immune Red-Team Arena

- **Sessions**: 20
- **Avg defender win rate**: 77.0%
- **Defense improvement trend**: STABLE/IMPROVING

## Phase 4: Scale Stress (100 Agents × 30 Generations)

- **Initial mean fitness**: 6.15
- **Final mean fitness**: 7.88
- **Improvement**: +28.0%
- **Total governance violations**: 0

| Gen | Pop | Mean Fit | Max Fit | Diversity | Violations | Time |
|-----|-----|----------|---------|-----------|------------|------|
|  0 | 100 | 6.15 | 6.58 | 7.0% | 0 | 0ms |
|  3 | 100 | 6.64 | 7.00 | 80.0% | 0 | 0ms |
|  6 | 100 | 6.94 | 7.21 | 100.0% | 0 | 0ms |
|  9 | 100 | 7.12 | 7.38 | 100.0% | 0 | 0ms |
| 12 | 100 | 7.28 | 7.41 | 98.0% | 0 | 0ms |
| 15 | 100 | 7.41 | 7.66 | 99.0% | 0 | 0ms |
| 18 | 100 | 7.54 | 7.76 | 99.0% | 0 | 0ms |
| 21 | 100 | 7.65 | 7.86 | 100.0% | 0 | 0ms |
| 24 | 100 | 7.75 | 7.93 | 100.0% | 0 | 0ms |
| 27 | 100 | 7.83 | 8.03 | 99.0% | 0 | 0ms |
| 29 | 100 | 7.88 | 8.03 | 99.0% | 0 | 0ms |

## Phase 5: Audit Trail Integrity

- **Total audit events**: 300
- **Hash chain valid**: true
- **Generations logged**: 10

## Governance Summary

| Metric | Phase 1 | Phase 4 | Total |
|--------|---------|---------|-------|
| Agents checked | 1000 | 3000 | 4000 |
| Autonomy violations | 0 | 0 | 0 |
| Capability escalations | 0 | 0 | 0 |
| Level mutations | 0 | 0 | 0 |

## Test Configuration

- Phase 1: 50 agents × 20 generations
- Phase 2: 50 PlanEvolutionEngine runs × 10 generations each
- Phase 3: 5 threat categories + 100 clean actions + 20 immune sessions × 50 rounds
- Phase 4: 100 agents × 30 generations (scale stress)
- Phase 5: 20 agents × 10 generations with full audit logging
- Max regression threshold: 5%
- Min diversity threshold: 20%

## How to Run

```bash
cargo run -p nexus-conductor-benchmark --bin darwin-drift-bench --release
```
