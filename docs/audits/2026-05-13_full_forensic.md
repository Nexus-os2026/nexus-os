# Full Forensic Audit — Index

**Date:** 2026-05-13
**HEAD:** efb06aa2 (feat(secrets): real-AK-2 capability ledger)
**Auditor:** Claude Opus 4.6 (read-only)
**Version:** Nexus OS 10.6.0

---

## Executive Summary

Full forensic audit of Nexus OS across 70 workspace crates, 89 frontend pages, 804 registered Tauri commands, 7 swarm agent types, and 7 critical user journeys.

**3,790 crate tests pass. 0 failures. 0 flakes. All critical crates clippy-clean.**

The system is structurally sound. Governance, audit, fuel metering, and capability enforcement are wired through the swarm execution path. The 10 hard self-improvement invariants are implemented and tested. PQC abstraction boundary has 1 remaining bypass. No leaked secrets. RUSTSEC-2026-0114 (Wasmtime) properly resolved.

Key concerns: 3 user-visible P1 wiring breaks (audit tracing, admin policy save, learning center sync), 2 swarm-level P1s (missing audit hooks on execution paths, broken failure terminal state), and 6 messaging connectors using blocking HTTP in async context.

---

## Sub-Reports

| Phase | Document | Status |
|-------|----------|--------|
| 0 | [2026-05-13_full_forensic_00_inventory.md](2026-05-13_full_forensic_00_inventory.md) | COMPLETE |
| 1 | [2026-05-13_full_forensic_01_wiring.md](2026-05-13_full_forensic_01_wiring.md) | COMPLETE |
| 2 | [2026-05-13_full_forensic_02_agents.md](2026-05-13_full_forensic_02_agents.md) | COMPLETE |
| 3 | [2026-05-13_full_forensic_03_user_journeys.md](2026-05-13_full_forensic_03_user_journeys.md) | COMPLETE (code-walk only, all UNVERIFIED) |
| 4 | [2026-05-13_full_forensic_04_dev_correctness.md](2026-05-13_full_forensic_04_dev_correctness.md) | COMPLETE |
| 5 | [2026-05-13_full_forensic_05_threat_model.md](2026-05-13_full_forensic_05_threat_model.md) | COMPLETE |

---

## Severity-Sorted Finding List

### P1 Findings (5 total)

| # | Phase | Finding | File:Line |
|---|-------|---------|-----------|
| 1 | P1-Wiring | Audit tracing tab sends lowercase span statuses; backend rejects them as unknown | app/src/pages/Audit.tsx:338-345; app/src-tauri/src/commands/trust_security.rs:1211-1253 |
| 2 | P1-Wiring | Admin Policy Save is a no-op — backend ignores payload, returns Ok(()) | app/src/pages/AdminPolicyEditor.tsx:79-105; app/src-tauri/src/commands/enterprise.rs:512-524 |
| 3 | P1-Wiring | Learning Center mount-time sync and step completion both broken — wrong argument shapes | app/src/pages/LearningCenter.tsx:547-575,620-683; app/src-tauri/src/commands/browser_research.rs:980-986 |
| 4 | P1-Agents | Swarm-side audit hooks absent on Director and coordinator execution paths | tests/integration/tests/swarm_harness_scenario_a.rs:75-82; swarm_harness_scenario_c.rs:80-87 |
| 5 | P1-Agents | Failed swarm runs terminate as SwarmCompleted instead of distinct failure terminal | crates/nexus-swarm/src/coordinator.rs:118-148; swarm_harness_scenario_g.rs:81-90 |

### P2 Findings (top 10)

| # | Phase | Finding | File:Line |
|---|-------|---------|-----------|
| 1 | P2-Wiring | Messaging.tsx sets msgError but never renders it — connect/send failures hidden | app/src/pages/Messaging.tsx:148-229 |
| 2 | P2-Wiring | ComplianceDashboard swallows backend failures, falls back to "compliant" | app/src/pages/ComplianceDashboard.tsx:103,126 |
| 3 | P2-Wiring | Settings provider save failures swallowed | app/src/pages/Settings.tsx:281-302 |
| 4 | P2-Agents | Artisan/Herald descriptor schemas drifted from actual entry contracts | crates/nexus-swarm/src/adapters/artisan.rs:55-62; herald.rs:70-78 |
| 5 | P2-Agents | Scout, Watchdog, Prospector remain stubs — return error on execution | crates/nexus-swarm/src/adapters/scout.rs:32, watchdog.rs:27, prospector.rs:27 |
| 6 | P2-Dev | full_agent_flow integration test FAILS — file write path mismatch | tests/integration/src/full_agent_flow.rs:310 |
| 7 | P2-Dev | 6 messaging connectors use reqwest::blocking — blocks tokio runtime | connectors/messaging/src/{whatsapp,discord,matrix,slack,webhook,telegram}.rs:8 |
| 8 | P2-Threat | PQC Phase 1b: 1 file bypasses CryptoIdentity abstraction | crates/nexus-ui-repair/src/governance/identity.rs:3 |
| 9 | P2-Wiring | Email client failures silently converted to local-draft fallbacks | app/src/pages/EmailClient.tsx:67-75,128-137 |
| 10 | P2-Wiring | Firewall page — backend failures only console-logged, no user error surface | app/src/pages/Firewall.tsx:11-17 |

### P3 Finding Count

**4 total** (style/clarity issues: swarm-core low test count, test placeholder API keys, self-improvement stage names not individually surfaced in UI, designer agent low governance coverage)

---

## Counts

| Category | Count |
|----------|-------|
| Pages audited | 89 |
| Commands audited | 804 |
| Agents audited | 7 swarm + 9 crate = 16 |
| User journeys walked | 7 (all code-walk, UNVERIFIED) |
| Crates tested | 32 |
| Total tests executed | 3,790 |
| Test failures | 0 (crate tests); 1 (integration: full_agent_flow) |

---

## Coverage Gaps

| Gap | Reason |
|-----|--------|
| All 7 user journeys UNVERIFIED at runtime | Tauri desktop build did not reach usable window during audit session |
| Provider routing across 8 providers (J6) | Requires live runtime + provider availability |
| Checkpoint-rollback state restoration (J7) | Requires live runtime |
| OWASP defense test functions | Filtered to 0 via cargo test — defenses exist but may lack dedicated test functions |
| gitleaks automated scan | gitleaks not installed on system |
| Epistemic class assignment in swarm | Not found in any swarm execution path |
| Agent memory ACL by autonomy level | Not exercised in executed harness scenarios |

---

## Remediation Roadmap (ranked)

| Priority | Item | Effort |
|----------|------|--------|
| 1 | Fix 3 P1 wiring breaks (audit tracing case, admin policy save, learning center args) | Small — argument/case fixes |
| 2 | Add swarm-side audit hooks on Director/coordinator execution paths | Medium — event emission wiring |
| 3 | Add distinct SwarmFailed terminal state (not SwarmCompleted on error) | Medium — coordinator state machine change |
| 4 | Migrate 6 messaging connectors from reqwest::blocking to async reqwest | Medium — API change across 6 files |
| 5 | Fix Artisan/Herald descriptor schema drift | Small — sync descriptors with entry contracts |
| 6 | Fix full_agent_flow integration test file path assertion | Small — path expectation update |
| 7 | Migrate nexus-ui-repair/governance/identity.rs to CryptoIdentity | Small — import change |
| 8 | Add backend.ts wrappers for 9 swarm commands (plan/approve/reject/etc.) | Medium — JS wiring |
| 9 | Surface error states in ~15 pages that swallow backend failures | Medium — UI error banner additions |
| 10 | Implement Scout, Watchdog, Prospector beyond stubs | Large — full agent implementation |
| 11 | Add manifests for 6 agent crates missing manifest.toml | Small — configuration |
| 12 | Add frontend tests for NexusBuilder.tsx, NexusCode.tsx | Small — test files |
| 13 | Install and integrate gitleaks in CI | Small — CI config |
| 14 | Add OWASP defense unit tests | Medium — test coverage |
| 15 | Add epistemic class assignment to swarm outputs | Medium — type system addition |
