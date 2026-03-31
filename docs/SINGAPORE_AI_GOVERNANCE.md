# Singapore Model AI Governance Framework — Nexus OS Mapping

## Overview

This document maps Nexus OS v10.5.0 to Singapore's Model AI Governance Framework (2nd Edition, 2020), published by the Infocomm Media Development Authority (IMDA) and the Personal Data Protection Commission (PDPC).

The framework is voluntary and principle-based, designed to guide organizations deploying AI systems in Singapore and the ASEAN region.

**Assessment Date:** 2026-03-31
**Framework:** Model AI Governance Framework, 2nd Edition (January 2020)
**System Version:** 10.5.0

---

## Principle 1: Internal Governance Structures and Measures

> Organizations using AI in decision-making should have governance structures to ensure accountability, oversight, and compliance.

| Requirement | Nexus OS Implementation | Code Reference |
|-------------|------------------------|----------------|
| Designate clear roles and responsibilities for AI governance | Governance kernel is the single authority for all agent decisions; no agent action bypasses the kernel | `kernel/src/supervisor.rs` — `Supervisor` |
| Establish policies and accountability mechanisms | Cedar-inspired policy engine with TOML-based rules; default-deny evaluation; priority-based ordering | `kernel/src/policy_engine/mod.rs` — `PolicyEngine` |
| Implement risk assessment processes | Multi-dimensional risk scoring: HitlTier (0–3) × AutonomyLevel (L0–L6) × KPI status; speculative engine previews actions | `kernel/src/speculative.rs` — `SpeculativeEngine`, `kernel/src/governance_kpi.rs` |
| Establish audit trails | Hash-chained audit trail: SHA-256 linked events, tamper-evident, append-only; `verify_integrity()` for cryptographic verification | `kernel/src/audit/mod.rs` — `AuditTrail` |
| Risk management throughout lifecycle | Agent lifecycle managed from creation through evolution and decommissioning; adversarial arena continuously stress-tests agents | `kernel/src/immune/arena.rs`, `kernel/src/safety_supervisor.rs` |

**Status:** IMPLEMENTED

---

## Principle 2: Determining AI Decision-Making

> Organizations should evaluate the level of human involvement in AI-augmented decision-making.

| Requirement | Nexus OS Implementation | Code Reference |
|-------------|------------------------|----------------|
| Assess risk and impact of AI decisions | World simulation engine runs dry-run execution before real deployment; speculative engine previews file changes, network calls, data modifications | `crates/nexus-world-simulation/src/` — `SimulationEngine`, `kernel/src/speculative.rs` |
| Determine appropriate level of human involvement | L0–L6 graduated autonomy with formal definitions: L0 (Manual) through L6 (Self-Evolving); deployers select level based on risk | `kernel/src/autonomy.rs` — `AutonomyLevel` |
| Implement human-in-the-loop for high-risk decisions | HITL consent gates: 4 tiers (Tier0–3) with configurable approval counts; Ed25519-signed approval tokens for non-repudiation | `kernel/src/consent.rs` — `ConsentRuntime`, `HitlTier` |
| Provide explanations for AI decisions | Audit trail records: agent DID, action performed, capability invoked, autonomy level, HITL approval status, fuel consumed | `kernel/src/audit/mod.rs` — `AuditEvent` |
| Allow human override | Kill gates: manual override halts agent immediately; unfreeze requires Tier3 approval; fuel exhaustion as deterministic stop | `kernel/tests/kill_gates_phase7.rs`, `kernel/src/supervisor.rs` |

**Autonomy Level Mapping to Singapore Framework:**

| Nexus Level | Human Involvement | Singapore Alignment |
|-------------|-------------------|-------------------|
| L0 — Manual | Every action requires approval | Human-in-the-loop |
| L1 — Supervised | All actions monitored in real time | Human-in-the-loop |
| L2 — Guided | Risk-flagged actions require approval | Human-over-the-loop |
| L3 — Semi-Autonomous | Only high-risk actions escalated | Human-over-the-loop |
| L4 — Autonomous | Periodic audit trail review | Human-out-of-the-loop (monitored) |
| L5 — Fully Autonomous | Governance kernel constraints only | Human-out-of-the-loop (governed) |
| L6 — Self-Evolving | Evolution within governed bounds | Human-out-of-the-loop (bounded) |

**Status:** IMPLEMENTED

---

## Principle 3: Operations Management

> Organizations should build systems that are robust, reliable, and secure.

| Requirement | Nexus OS Implementation | Code Reference |
|-------------|------------------------|----------------|
| Ensure AI system robustness | Seven-layer defense: identity, capability ACL, autonomy gate, HITL consent, fuel metering, WASM sandbox, output firewall | `kernel/src/` — governance modules |
| Monitor system performance | Prometheus metrics at `:9090/metrics`; `/health` and `/ready` endpoints; governance KPI scoring per agent | `protocols/src/http_gateway.rs` — `health_check`, `metrics_endpoint` |
| Implement incident response | Circuit breaker (OWASP #8): automatic state management; anomaly monitor (OWASP #10): spike detection + auto-suspension; 3-strike halt | `kernel/src/owasp_defenses.rs` — `CircuitBreaker`, `AnomalyMonitor` |
| Cybersecurity measures | OWASP Agentic Top 10: all 10 defenses implemented (62 tests); Ed25519 identity; WASM sandbox; prompt firewall with 20 injection patterns | `kernel/src/owasp_defenses.rs`, `kernel/src/firewall/` |
| Ensure data quality and integrity | PII redaction (10 patterns); output firewall filters responses; hash-chained audit ensures data integrity | `kernel/src/immune/privacy.rs`, `kernel/src/firewall/prompt_firewall.rs` |
| Recovery and continuity | Checkpoint-rollback: 3-level recovery; Docker/Helm HA deployment with HPA autoscaling; backup CronJob | `kernel/src/checkpoint.rs`, `kernel/src/time_machine.rs` |
| Supply chain security | Marketplace packages: Ed25519 signing + in-toto attestation; OWASP #7 runtime package verification | `marketplace/src/package.rs`, `kernel/src/owasp_defenses.rs` — `RuntimePackageVerifier` |

**Status:** IMPLEMENTED

---

## Principle 4: Stakeholder Interaction and Communication

> Organizations should foster open communication with stakeholders about their use of AI.

| Requirement | Nexus OS Implementation | Code Reference |
|-------------|------------------------|----------------|
| Transparency about AI use | Agent capability declarations enumerate all permitted actions in human-readable format; L0–L6 labels clearly communicate autonomy degree | `kernel/src/permissions.rs` — `PermissionCategory` |
| Explain AI decision-making | Full audit trail with decision provenance: every action links to agent identity, capability, autonomy level, and HITL approval status | `kernel/src/audit/mod.rs` — `AuditEvent` |
| Allow feedback and recourse | HITL consent gates: humans can approve, reject, or modify proposed actions before execution; undo/redo via time machine | `kernel/src/consent.rs`, `kernel/src/time_machine.rs` — `TimeMachine` |
| Publish governance documentation | Compliance documents: EU AI Act conformity, SOC 2 controls, NIST 800-53 mapping, threat model, security policy | `docs/` directory |
| Protect personal data (PDPA alignment) | PII scanning and auto-redaction; crypto-erasure support; local-first architecture — no data leaves the machine by default | `kernel/src/immune/privacy.rs`, `kernel/src/privacy.rs` |

**Status:** IMPLEMENTED

---

## PDPA (Personal Data Protection Act) Alignment

Singapore's PDPA complements the AI Governance Framework. Nexus OS provides technical controls for PDPA obligations:

| PDPA Obligation | Nexus OS Control | Code Reference |
|----------------|-----------------|----------------|
| Consent | HITL consent gates for data-touching operations | `kernel/src/consent.rs` |
| Purpose Limitation | Capability scoping: agents only access data within granted capabilities | `kernel/src/permissions.rs` |
| Notification | Agent capability declarations enumerate data access | `kernel/src/permissions.rs` |
| Access and Correction | Audit trail provides complete data access history | `kernel/src/audit/mod.rs` |
| Accuracy | Output firewall validates data quality | `kernel/src/firewall/prompt_firewall.rs` |
| Protection | WASM sandbox, encryption, sealed key storage | `sdk/src/wasmtime_sandbox.rs`, `kernel/src/hardware_security/manager.rs` |
| Retention Limitation | Configurable audit retention with Merkle archival | `kernel/src/audit/retention.rs` |
| Transfer Limitation | Local-first; air-gappable; egress governor rate-limits outbound | `kernel/src/firewall/egress.rs` |
| Data Breach Notification | Audit trail provides forensic evidence for breach investigation | `kernel/src/audit/mod.rs` |

---

## Summary

Nexus OS provides comprehensive technical controls aligned with all four principles of Singapore's Model AI Governance Framework. The governance-first architecture — where controls are built into the kernel rather than added as plugins — provides structural assurance that cannot be bypassed.

For organizations deploying AI agents in Singapore and ASEAN markets, Nexus OS's combination of formal autonomy levels (L0–L6), HITL consent gates, hash-chained audit trails, and OWASP Agentic Top 10 defenses provides the most complete governance infrastructure available in any open-source AI agent platform.
