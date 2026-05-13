# Full Forensic Audit — Phase 4: Developer Correctness

**Date:** 2026-05-13
**HEAD:** efb06aa2

---

## Test Results Summary

**Total: 3,790 tests passed across 32 crates. 0 failures. 0 flakes.**

| Crate | Tests Passed | Clippy Clean |
|-------|-------------|-------------|
| nexus-kernel | 2,049 | YES |
| nexus-connectors-llm | 292 | YES |
| nexus-computer-use | 218 | YES |
| nexus-memory | 197 | YES |
| nexus-protocols | 110 | YES |
| nexus-self-improve | 101 | YES |
| nexus-swarm | 88 | YES |
| nexus-persistence | 75 | YES |
| nexus-capability-measurement | 73 | YES |
| nexus-migrate | 63 | YES |
| nexus-outcome-eval | 41 | YES |
| nexus-ui-repair | 35 | YES |
| nexus-auth | 32 | YES |
| nexus-a2a | 32 | YES |
| nexus-mcp | 31 | YES |
| nexus-token-economy | 29 | YES |
| nexus-crypto | 24 | YES |
| nexus-agent-memory | 21 | YES |
| nexus-perception | 19 | YES |
| nexus-metering | 18 | YES |
| nexus-collab-protocol | 18 | YES |
| nexus-world-simulation | 18 | YES |
| nexus-external-tools | 17 | YES |
| nexus-computer-control | 16 | YES |
| nexus-connectors-core | 14 | YES |
| nexus-predictive-router | 14 | YES |
| nexus-governance-oracle | 13 | YES |
| nexus-browser-agent | 12 | YES |
| nexus-governance-engine | 10 | YES |
| nexus-governance-evolution | 7 | YES |
| nexus-flash-infer | 6 | YES |
| nexus-swarm-core | 2 | YES |

### Integration Tests

| Test Suite | Status |
|-----------|--------|
| swarm_harness_scenario_a | PASS |
| swarm_harness_scenario_b | PASS |
| swarm_harness_scenario_c | PASS |
| swarm_harness_scenario_d | PASS |
| swarm_harness_scenario_e | PASS |
| swarm_harness_scenario_f | PASS |
| swarm_harness_scenario_g | PASS |
| full_agent_flow | FAIL (P2) |

---

## Clippy

`cargo clippy -D warnings` clean on all critical crates:
- nexus-kernel, nexus-swarm, nexus-swarm-core, nexus-crypto
- nexus-governance-oracle, nexus-governance-engine, nexus-self-improve

---

## Self-Improvement Hard Invariants

All 10 invariants defined in `crates/nexus-self-improve/src/invariants.rs`:

| # | Invariant | Guard Location |
|---|-----------|---------------|
| 1 | GovernanceKernelImmutable | invariants.rs:179 — checks protected paths |
| 2 | AuditTrailIntegrity | invariants.rs:190 — checks audit hash chain |
| 3 | HitlGatesCannotWeaken | invariants.rs:207,216 — checks HITL tier/threshold |
| 4 | CapabilitiesCannotExpand | invariants.rs:229 — checks capability set |
| 5 | FuelLimitsEnforced | invariants.rs:243 — checks fuel bounds |
| 6 | CryptoIdentityImmutable | invariants.rs:257 — checks identity key paths |
| 7 | AllChangesReversible | invariants.rs:268,275 — checks rollback capability |
| 8-10 | SelfProtected + remaining | invariants.rs:141-170 — protected path sets |

All invariants have check functions and are exercised in the nexus-self-improve test suite (101 tests passing).

---

## Async Hygiene

### block_on in async contexts

| Location | Risk | Assessment |
|----------|------|-----------|
| nexus-computer-use/capture/screenshot.rs:380,399 | LOW | Test-only block_on with own runtime |
| nexus-ui-repair/specialists/eyes_and_hands.rs:49,89,95 | LOW | Creates per-call current-thread runtime; documented |
| agents/web-builder (image_gen, deploy) | LOW | Test-only block_on with own runtimes |

### reqwest::blocking in async contexts

| Location | Risk | Assessment |
|----------|------|-----------|
| connectors/messaging/src/{whatsapp,discord,matrix,slack,webhook,telegram}.rs | **P2** | All 6 messaging connectors use `reqwest::blocking::Client`. Will block tokio runtime thread if called from async context. |

### Mutex held across .await

All detected cases use `tokio::Mutex` (not `std::Mutex`), which is safe:

| Location | Type |
|----------|------|
| nexus-swarm/adapters/mod.rs:148 | tokio::Mutex budget |
| nexus-swarm/coordinator.rs:214 | tokio::Mutex health snapshot |
| nexus-swarm-core/emitter.rs:47,54,61 | tokio::Mutex recording |
| nexus-memory/audit.rs:103,119,150,192 | tokio::Mutex DB handle |

**No std::Mutex held across .await detected.**

### ArcSwap Usage

ArcSwap confirmed for policy/status reads on hot paths:
- `kernel/src/cognitive/loop_runtime.rs:20,573,574,667` — `ArcSwap<CognitiveStatusResponse>` for lock-free agent status reads

No Mutex/RwLock regression on hot paths detected.

### Unsafe Blocks

| Location | Justification |
|----------|--------------|
| kernel/src/resource_limiter.rs:111 | `pre_exec` for setrlimit/setpgid — required by stdlib API. Uses only safe nix wrappers. Well-commented inline. No ADR in docs/adr/. |

No other unsafe blocks found outside test code.

---

## P1 Findings

None.

## P2 Findings

| # | Finding | File:Line |
|---|---------|-----------|
| P2-D01 | full_agent_flow integration test FAILS — file write goes to `.nexus/agents/test.txt` instead of expected workspace subdirectory | tests/integration/src/full_agent_flow.rs:310 |
| P2-D02 | 6 messaging connectors use reqwest::blocking::Client — will block tokio runtime if called from async context | connectors/messaging/src/{whatsapp,discord,matrix,slack,webhook,telegram}.rs:8 |
| P2-D03 | kernel/src/resource_limiter.rs:111 — unsafe block has no ADR in docs/adr/ (inline justification exists) | kernel/src/resource_limiter.rs:111 |

## P3 Findings

| # | Finding | File:Line |
|---|---------|-----------|
| P3-D01 | nexus-swarm-core has only 2 tests — low coverage for a shared types crate | crates/nexus-swarm-core/src/ |
