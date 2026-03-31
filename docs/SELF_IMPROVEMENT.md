# Governed Self-Improvement — Nexus OS

## Overview

Nexus OS can improve itself — prompts, configs, and policies — while the governance kernel remains immutable. Every change passes through 10 hard invariants and requires Tier3 HITL approval before deployment. Applied changes enter a canary monitoring period with automatic rollback on anomaly.

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │        Simplex Guardian              │
                    │  (barrier certificate → baseline)    │
                    └──────────────┬──────────────────────┘
                                  │ PRE-CHECK
    ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Observer │──▶│ Analyzer │──▶│ Proposer │──▶│Validator │──▶│ Applier  │
    │ (signals)│   │(priority)│   │ (engine) │   │(10 inv.) │   │(canary)  │
    └──────────┘   └──────────┘   └──────────┘   └──────────┘   └──────────┘
                                                                     │
                                                          ┌──────────▼──────────┐
                                                          │ Behavioral Envelope │
                                                          │  (drift bounds)     │
                                                          └─────────────────────┘
```

### Five Stages

1. **Observer** — Monitors system metrics via EMA baselines. Emits `ImprovementSignal` when a metric deviates beyond the configurable sigma threshold.

2. **Analyzer** — Groups related signals, classifies opportunities (Security > Reliability > Performance > Quality > FeatureGap), estimates severity and blast radius.

3. **Proposer** — Generates concrete proposals using domain-specific engines:
   - **Prompt Optimizer**: DSPy/OPRO-style variant generation with safety keyword validation, cosine similarity scoring, and improvement threshold enforcement
   - **Config Optimizer**: Bounded parameter tuning with step-size constraints and min/max enforcement
   - **Policy Optimizer**: Cedar policy refinement (can only ADD or NARROW, never REMOVE or BROADEN)

4. **Validator** — Enforces all 10 hard invariants, runs test suite, performs simulation risk assessment, and requires Tier3 HITL consent (non-negotiable).

5. **Applier** — Creates checkpoint, applies change, runs post-apply tests (auto-rollback on failure), enters canary monitoring period.

## 10 Hard Invariants

These are enforced at the kernel level and CANNOT be overridden:

| # | Invariant | Protection |
|---|-----------|-----------|
| 1 | **Governance Kernel Immutable** | Cannot modify kernel governance code (permissions, consent, firewall, audit, identity) |
| 2 | **Audit Trail Integrity** | Hash chain must remain valid before any improvement |
| 3 | **HITL Gates Cannot Weaken** | Cannot lower consent tier requirements |
| 4 | **Capabilities Cannot Expand** | Cannot grant self new capabilities |
| 5 | **Fuel Limits Enforced** | Operates within allocated fuel budget |
| 6 | **Crypto Identity Immutable** | Ed25519 identity cannot be changed |
| 7 | **All Changes Reversible** | Every change requires a checkpoint + rollback plan |
| 8 | **Test Suite Green** | All tests must pass before and after |
| 9 | **HITL Approval Required** | Tier3 human-in-the-loop approval for all changes |
| 10 | **Self-Protected** | Cannot modify the self-improvement pipeline itself |

## Supported Domains

| Domain | Status | Description |
|--------|--------|-------------|
| Prompt Optimization | Active | DSPy/OPRO-style prompt variant generation and scoring |
| Config Tuning | Active | Bounded runtime parameter optimization |
| Governance Policy | Active | Cedar policy narrowing (adds constraints, never removes) |
| Scheduling Policy | Active | DRL-based scheduling weight adjustment |
| Routing Strategy | Active | Predictive router weight optimization |
| Code Patch | Locked | Requires `code-self-modify` feature flag (Phase 5) |

## Safety Guarantees

### Simplex Guardian
Maintains a verified-safe baseline configuration alongside the active (possibly improved) configuration. Uses the **Drift Bounds Theorem** (D\* = α/γ) to determine when behavioral drift approaches safety limits. If the barrier certificate threshold is exceeded, the guardian automatically switches to the verified baseline.

### Behavioral Envelope
Bounds agent behavior mathematically. Each agent has per-metric bounds (baseline ± tolerance) tracked with EMA. The `current_drift()` function calculates RMS deviation across all metrics. The `would_violate()` function predicts whether a proposed change would push an agent outside its envelope.

### Adaptive Scheduler
Learns WHEN to attempt improvements. Shortens intervals on success (more frequent optimization), backs off exponentially on failure. Respects min/max interval bounds.

## Usage

### Dashboard
Navigate to **Self-Improvement** in the sidebar. The dashboard shows:
- Pipeline status (signals, opportunities, pending proposals)
- 10 hard invariant status indicators
- Run Cycle button (triggers observe → analyze → propose)
- Pending proposals with Approve/Reject buttons
- Applied improvements with Rollback option
- Guardian status (drift, headroom, baseline hash)
- Improvement metrics (success rate, total applied/committed/rolled-back)

### Run a Cycle
1. Click "Run Improvement Cycle"
2. If signals are detected, the system analyzes and generates a proposal
3. Review the proposal (domain, description, fuel cost)
4. Click "Approve (Tier3)" to apply, or "Reject" to dismiss
5. Approved changes enter canary monitoring (default: 30 minutes)
6. Monitor for anomalies — automatic rollback on regression

### Configuration
- **Sigma Threshold**: Standard deviations from baseline to trigger a signal (default: 2.0)
- **Canary Duration**: Minutes to monitor after applying (default: 30)
- **Fuel Budget**: Maximum fuel for the pipeline (default: 5000)
- **Domain Toggles**: Enable/disable each improvement domain

## FAQ

**Can the self-improvement system modify the governance kernel?**
No. Invariant #1 protects 12 governance code paths. Any proposal touching these paths is rejected before it reaches HITL review.

**Can it disable HITL consent gates?**
No. Invariant #3 prevents weakening consent tier requirements. Invariant #9 requires Tier3 HITL approval for every change.

**Can it grant itself new capabilities?**
No. Invariant #4 blocks any config change targeting capability or permission keys.

**Can it modify its own pipeline?**
No. Invariant #10 protects `crates/nexus-self-improve/` from self-modification.

**What happens if the system drifts too far?**
The Simplex Guardian monitors behavioral drift using the Drift Bounds Theorem. If drift exceeds the barrier certificate threshold, the guardian automatically switches to the last verified-safe baseline configuration.

**Can I force a rollback?**
Yes. Click "Rollback" on any improvement in the Monitoring or Applied state. You can also click "Force Baseline" to restore the guardian's verified-safe configuration.

## Code References

| Component | Path |
|-----------|------|
| Pipeline | `crates/nexus-self-improve/src/pipeline.rs` |
| Observer | `crates/nexus-self-improve/src/observer.rs` |
| Analyzer | `crates/nexus-self-improve/src/analyzer.rs` |
| Proposer | `crates/nexus-self-improve/src/proposer.rs` |
| Validator | `crates/nexus-self-improve/src/validator.rs` |
| Applier | `crates/nexus-self-improve/src/applier.rs` |
| 10 Invariants | `crates/nexus-self-improve/src/invariants.rs` |
| Prompt Optimizer | `crates/nexus-self-improve/src/prompt_optimizer.rs` |
| Config Optimizer | `crates/nexus-self-improve/src/config_optimizer.rs` |
| Policy Optimizer | `crates/nexus-self-improve/src/policy_optimizer.rs` |
| Behavioral Envelope | `crates/nexus-self-improve/src/envelope.rs` |
| Simplex Guardian | `crates/nexus-self-improve/src/guardian.rs` |
| Adaptive Scheduler | `crates/nexus-self-improve/src/scheduler.rs` |
| Trajectory Tracking | `crates/nexus-self-improve/src/trajectory.rs` |
| Report Generator | `crates/nexus-self-improve/src/report.rs` |
| Tauri Commands | `app/src-tauri/src/commands/self_improvement.rs` |
| Frontend Dashboard | `app/src/pages/SelfImprovement.tsx` |
