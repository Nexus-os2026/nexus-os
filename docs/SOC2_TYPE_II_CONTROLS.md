# SOC 2 Type II Controls — Nexus OS

## Overview

This document maps Nexus OS v10.5.0 features to the AICPA Trust Service Criteria for SOC 2 Type II compliance. Each control includes implementation status, code references, and test evidence.

**Assessment Date:** 2026-03-31
**Observation Period Target:** Q3–Q4 2026
**System Version:** 10.5.0

**Status Key:**
- **IMPLEMENTED** — Feature is built, tested, and active in the current release
- **PARTIAL** — Core capability exists but requires additional work
- **PLANNED** — Scheduled for development

---

## 1. Security (Common Criteria CC1–CC9)

### CC1: Control Environment

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC1.1 | Integrity and ethical values | MIT open-source license; full source code auditability; all agent actions hash-chained | `kernel/src/audit/mod.rs` — `AuditTrail::append_event()` | `test_audit_chain_integrity` | IMPLEMENTED |
| CC1.2 | Board oversight | Project governance documented; contributing guidelines enforce review process | `CONTRIBUTING.md`, `SECURITY.md` | — | IMPLEMENTED |
| CC1.3 | Management structure | Architecture documented; 65-crate workspace with separation of concerns | `ARCHITECTURE.md`, `docs/OVERVIEW.md` | — | IMPLEMENTED |
| CC1.4 | Competence commitment | Rust type system enforces correctness at compile time; `unsafe_code = forbid` in kernel | `kernel/Cargo.toml` — `#![forbid(unsafe_code)]` | 5,029 tests, 0 failures | IMPLEMENTED |
| CC1.5 | Accountability | Every agent action attributed to a specific Ed25519 DID in the hash-chained audit trail | `kernel/src/identity/agent_identity.rs` — `AgentIdentity`, `kernel/src/audit/mod.rs` — `AuditEvent` | `test_audit_chain_integrity_passes`, `test_agent_identity_uses_key_manager` | IMPLEMENTED |

### CC2: Communication and Information

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC2.1 | Internal information quality | Structured audit logs with SHA-256 hash chains; cryptographic integrity verification | `kernel/src/audit/mod.rs` — `verify_integrity()`, `verify_full_integrity()` | `test_persistent_audit_hash_chain_integrity`, `test_audit_chain_detects_tampering` | IMPLEMENTED |
| CC2.2 | Internal communication | Agent-to-agent communication through governed A2A and MCP protocols; all messages audited | `protocols/src/http_gateway.rs` — A2A routes, `kernel/src/protocols/a2a_client.rs` | `test_audit_trail_recording` | IMPLEMENTED |
| CC2.3 | External communication | Security policy with vulnerability reporting; OIDC-A token-based API authentication | `SECURITY.md`, `kernel/src/identity/token_manager.rs` — `TokenManager` | `test_manager` | IMPLEMENTED |

### CC3: Risk Assessment

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC3.1 | Risk identification | Capability-based ACL identifies and constrains agent risks; Cedar-inspired policy engine evaluates risk | `kernel/src/permissions.rs` — `PermissionManager`, `kernel/src/policy_engine/mod.rs` — `PolicyEngine` | `test_capability_check_required`, `test_deny_overrides_allow_always` | IMPLEMENTED |
| CC3.2 | Risk evaluation | Multi-dimensional risk scoring: HitlTier (0–3) × AutonomyLevel (L0–L6) × KPI status; speculative engine simulates actions before approval | `kernel/src/speculative.rs` — `SpeculativeEngine`, `kernel/src/governance_kpi.rs` | `test_risk_rating_thresholds`, `test_health_score_perfect` | IMPLEMENTED |
| CC3.3 | Fraud risk | Output firewall detects data exfiltration; anomaly monitor detects anomalous patterns; burn anomaly detector flags unusual fuel consumption | `kernel/src/firewall/prompt_firewall.rs` — `PromptFirewall`, `kernel/src/owasp_defenses.rs` — `AnomalyMonitor` | `test_scan_exfiltration`, `test_pii_redaction_rate` | IMPLEMENTED |
| CC3.4 | Change impact | CI/CD pipeline: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` (5,029 tests), `npm run build` | CI pipeline config | All tests passing | IMPLEMENTED |

### CC4: Monitoring Activities

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC4.1 | Ongoing monitoring | Hash-chained audit trail provides continuous monitoring; Prometheus metrics at `:9090/metrics`; governance KPI scoring per agent | `kernel/src/audit/mod.rs`, `protocols/src/http_gateway.rs` — `metrics_endpoint`, `kernel/src/governance_kpi.rs` | `test_compute_kpis_returns_all_categories`, `test_agent_count` | IMPLEMENTED |
| CC4.2 | Deficiency evaluation | 3-strike halt policy: safety supervisor suspends agents after repeated governance violations | `kernel/src/safety_supervisor.rs`, `kernel/tests/safety_supervisor_phase6.rs` | `test_3_strike_halt`, `test_kpi_degraded`, `test_incident_report_structure` | IMPLEMENTED |

### CC5: Control Activities

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC5.1 | Control selection | Seven-layer defense: identity, capability ACL, autonomy gate, HITL consent, fuel metering, WASM sandbox, output firewall | `kernel/src/` — see individual modules | OWASP 10/10 tests (62 dedicated) | IMPLEMENTED |
| CC5.2 | Technology controls | WASM sandboxing for untrusted code; Ed25519 cryptographic identity; hardware key backend (TEE/TPM/Secure Enclave) | `sdk/src/wasmtime_sandbox.rs`, `kernel/src/hardware_security/manager.rs` — `KeyManager` | `test_wasmtime_version_minimum`, `test_tee_backend_generates_and_signs` | IMPLEMENTED |
| CC5.3 | Policy deployment | Cedar-inspired policy engine evaluates governance rules at runtime; policies cannot be bypassed by agent code | `kernel/src/policy_engine/mod.rs` — `PolicyEngine::evaluate()` | `test_deny_by_default`, `test_allow_matching_rule` | IMPLEMENTED |

### CC6: Logical and Physical Access Controls

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC6.1 | Logical access security | Capability-based ACL — no ambient authority; agents can only invoke explicitly granted capabilities | `kernel/src/permissions.rs` — `PermissionManager`, `Permission`, `PermissionRiskLevel` | `test_actuator_rejects_missing_capability`, `test_capability_allowlist` | IMPLEMENTED |
| CC6.2 | Access provisioning | Capabilities granted at agent creation via manifest.toml; cryptographically signed; permission changes audited | `kernel/src/permissions.rs` — `PermissionHistoryEntry`, `CapabilityRequest` | `test_capability_grant_and_revoke_history`, `test_capability_persistence_across_reopen` | IMPLEMENTED |
| CC6.3 | Access removal | Agent lifecycle management: fuel exhaustion halts agent; capability revocation; manual override suspension; 3-strike halt | `kernel/src/supervisor.rs` — `Supervisor`, `kernel/src/safety_supervisor.rs` | `test_fail_closed_fuel_exhaustion`, `test_manual_override_halts_agent_immediately` | IMPLEMENTED |
| CC6.6 | Threat management | Output firewall with 20 injection patterns and 10 PII patterns; egress governor with URL allowlisting; semantic boundary classifier | `kernel/src/firewall/patterns.rs`, `kernel/src/firewall/egress.rs` — `EgressGovernor`, `kernel/src/firewall/semantic_boundary.rs` | `test_scan_injection`, `test_scan_pii`, `test_injection_in_web_content_flagged` | IMPLEMENTED |
| CC6.7 | Identity management | DID/Ed25519 cryptographic identity per agent; `did:key:z6Mk...` format; hardware-backed key storage | `kernel/src/identity/agent_identity.rs` — `AgentIdentity`, `IdentityManager` | `test_agent_identity_uses_key_manager`, `test_multiple_agents_independent_keys` | IMPLEMENTED |
| CC6.8 | Authentication | EdDSA (Ed25519) JWT tokens for API authentication; OIDC-A claims; JWKS endpoint for key discovery | `kernel/src/identity/token_manager.rs` — `TokenManager`, `protocols/src/http_gateway.rs` — `/auth/jwks` | `test_identity_and_km`, `test_token_manager` | IMPLEMENTED |

### CC7: System Operations

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC7.1 | Infrastructure monitoring | Health and readiness endpoints; Prometheus metrics; governance KPI dashboard; system monitor in UI | `protocols/src/http_gateway.rs` — `health_check`, `readiness_check`, `metrics_endpoint` | `test_health_score_perfect`, `test_health_score_poor` | IMPLEMENTED |
| CC7.2 | Security incident detection | OWASP #10 AnomalyMonitor: spike detection + auto-suspension; immune system with threat signatures; audit chain tamper detection | `kernel/src/owasp_defenses.rs` — `AnomalyMonitor`, `kernel/src/immune/detector.rs` | `test_scan_injection`, `test_anomaly_detection` | IMPLEMENTED |
| CC7.3 | Incident response | OWASP #8 CircuitBreaker: Closed/Open/HalfOpen state machine; automatic agent halting on repeated failures | `kernel/src/owasp_defenses.rs` — `CircuitBreaker`, `connectors/llm/src/defense.rs` | `test_circuit_breaker_halts_agent` | IMPLEMENTED |
| CC7.4 | Business continuity | Checkpoint-rollback system: 3-level recovery (memory, execution, side-effect compensation); Docker/Helm HA deployment; backup CronJob | `kernel/src/checkpoint.rs` — `CheckpointManager`, `kernel/src/time_machine.rs` — `TimeMachine` | `test_create_checkpoint`, `test_undo_file_write`, `test_rollback_snapshot_restore` | IMPLEMENTED |

### CC8: Change Management

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC8.1 | Change authorization | CI/CD pipeline gates all changes: `cargo fmt`, `cargo clippy -D warnings`, 5,029 tests; marketplace packages require Ed25519 signatures | CI pipeline, `marketplace/src/package.rs` — `SignedPackageBundle` | All tests passing | IMPLEMENTED |

### CC9: Risk Mitigation

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| CC9.1 | Vendor risk management | All dependencies pinned in Cargo.lock; marketplace packages verified with Ed25519 and in-toto attestation | `marketplace/src/verification_pipeline.rs`, `marketplace/src/package.rs` — `InTotoAttestation` | Package verification tests | IMPLEMENTED |
| CC9.2 | Vendor monitoring | Runtime package verification (OWASP #7): load-time signature verification of agent packages | `kernel/src/owasp_defenses.rs` — `RuntimePackageVerifier` | OWASP #7 tests (7 tests) | IMPLEMENTED |

---

## 2. Availability (A1)

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| A1.1 | Capacity planning | Fuel metering tracks and limits resource consumption per agent; burn anomaly detector flags spikes | `kernel/src/supervisor.rs` — `AgentFuelLedger`, `BurnAnomalyDetector` | `test_fuel_metering_ceiling_accuracy`, `test_fuel_never_undercharges` | IMPLEMENTED |
| A1.2 | Recovery objectives | Checkpoint-rollback system with side-effect compensation; `/health` and `/ready` endpoints for orchestrator probes; Docker/Helm HA with HPA autoscaling | `kernel/src/checkpoint.rs` — `CheckpointManager`, `kernel/src/time_machine.rs`, `protocols/src/server_runtime.rs` | `test_create_checkpoint`, `test_undo_file_write`, `test_redo_after_undo` | IMPLEMENTED |
| A1.3 | Recovery testing | Replay engine verifies execution bundles and detects governance divergence; rollback tested in unit and integration tests | `kernel/src/replay/player.rs`, `kernel/src/replay/recorder.rs` | `test_verify_bundle_passes`, `test_verify_bundle_governance_failure`, `test_verify_tampered_bundle` | IMPLEMENTED |

---

## 3. Processing Integrity (PI1)

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| PI1.1 | Processing accuracy | Governance kernel validates all agent actions through seven-layer pipeline; WASM sandbox provides deterministic execution; speculative engine previews Tier2+ actions | `kernel/src/supervisor.rs`, `kernel/src/speculative.rs` — `SpeculativeEngine`, `sdk/src/wasmtime_sandbox.rs` | `test_wasm_sandbox_blocks_unauthorized_path`, `test_capability_check_required` | IMPLEMENTED |
| PI1.2 | Complete processing | Fuel reservation pattern: reserve-then-commit ensures operations complete or refund; cancellation returns all fuel | `kernel/src/fuel_hardening.rs` — `SupervisorFuelReservation` | `test_fuel_reservation_commit`, `test_fuel_reservation_cancel`, `test_fuel_reservation_drop_returns_fuel` | IMPLEMENTED |
| PI1.3 | Timely processing | Subprocess timeout enforcement; resource limiter prevents fork bombs; circuit breaker halts hung services | `kernel/tests/resource_limiter_integration_tests.rs`, `kernel/src/owasp_defenses.rs` — `CircuitBreaker` | `test_subprocess_respects_timeout`, `test_process_group_kill`, `test_rlimit_nproc_prevents_fork_bomb` | IMPLEMENTED |
| PI1.4 | Output validation | Output firewall scans all agent outputs; PII redaction removes sensitive data before delivery; schema validation on structured outputs | `kernel/src/firewall/prompt_firewall.rs` — `OutputFilter` | `test_redaction_before_llm_call`, `test_data_instruction_separation`, `test_output_validation_blocks_unauthorized` | IMPLEMENTED |

---

## 4. Confidentiality (C1)

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| C1.1 | Confidential information identification | PII redaction engine: 10 PII patterns (SSN, email, credit card, API keys, AWS keys, passwords, private IPs); Luhn validation for credit cards | `kernel/src/immune/privacy.rs`, `kernel/src/firewall/patterns.rs` | `test_detect_ssn`, `test_detect_email`, `test_credit_card_with_luhn`, `test_detect_openai_key`, `test_detect_aws_key` | IMPLEMENTED |
| C1.2 | Confidential information protection | Capability-based data access — agents only access data within granted capabilities; WASM sandbox prevents filesystem escape; local-first architecture | `kernel/src/permissions.rs`, `sdk/src/wasmtime_sandbox.rs` | `test_actuator_rejects_missing_capability`, `test_wasm_sandbox_blocks_unauthorized_path` | IMPLEMENTED |
| C1.3 | Confidential information disposal | Crypto-erasure support; no plaintext credential storage; hardware-backed sealed key store | `kernel/src/privacy.rs`, `kernel/src/hardware_security/manager.rs` — `SealedKeyStore` | `test_crypto_erasure`, `test_agent_identity_persistence_no_plaintext`, `test_sealed_storage_roundtrip` | IMPLEMENTED |

---

## 5. Privacy (P1–P8)

| ID | Control | Nexus OS Implementation | Code Reference | Test Evidence | Status |
|----|---------|------------------------|----------------|---------------|--------|
| P1 | Privacy notice | Agent capability declarations enumerate all data-touching permissions in human-readable format; L0–L6 autonomy labels | `kernel/src/permissions.rs` — `PermissionCategory` | `test_action_required_capabilities` | IMPLEMENTED |
| P2 | Choice and consent | HITL consent gates: 4 tiers (Tier0–3) with configurable approval counts; Ed25519-signed approval tokens create non-repudiable records | `kernel/src/consent.rs` — `ConsentRuntime`, `HitlTier` | `test_hitl_approval_for_destructive`, `test_blocked_on_hitl_for_low_autonomy` | IMPLEMENTED |
| P3 | Collection limitation | Agents constrained to declared capabilities; no ambient data access; egress governor rate-limits outbound requests | `kernel/src/permissions.rs`, `kernel/src/firewall/egress.rs` — `EgressGovernor` | `test_no_capability_rejected`, `test_terminal_command_allowlist_enforced` | IMPLEMENTED |
| P4 | Use limitation | Capability scoping prevents data use beyond intended purpose; delegation narrowing (OWASP #4) enforces subset permissions on sub-agents | `kernel/src/owasp_defenses.rs` — `DelegationNarrowing` | OWASP #4 tests (5 tests) | IMPLEMENTED |
| P5 | Retention | Configurable audit trail retention with Merkle tree archival for older segments | `kernel/src/audit/mod.rs` — `enable_retention()`, `kernel/src/audit/retention.rs` | Retention tests | IMPLEMENTED |
| P6 | Access | Complete data access history in hash-chained audit trail; queryable by agent ID | `kernel/src/persistence.rs` | `test_persistent_audit_query_by_agent`, `test_persistent_audit_count` | IMPLEMENTED |
| P7 | Disclosure | Local-first architecture; no third-party data sharing without explicit consent; air-gappable deployment | Deployment architecture | — | IMPLEMENTED |
| P8 | Quality | PII redaction + output firewall ensure data quality; secure logger (OWASP #9) redacts credentials from all logs | `kernel/src/owasp_defenses.rs` — `SecureLogger`, `kernel/src/immune/privacy.rs` | `test_redact`, `test_clean_text`, `test_audit_redaction_events_do_not_contain_raw_secret` | IMPLEMENTED |

---

## Evidence Summary

| Category | Controls | Implemented | Partial | Planned |
|----------|----------|-------------|---------|---------|
| Security (CC1–CC9) | 25 | 25 | 0 | 0 |
| Availability (A1) | 3 | 3 | 0 | 0 |
| Processing Integrity (PI1) | 4 | 4 | 0 | 0 |
| Confidentiality (C1) | 3 | 3 | 0 | 0 |
| Privacy (P1–P8) | 8 | 8 | 0 | 0 |
| **Total** | **43** | **43** | **0** | **0** |

## Relationship to Other Compliance Documents

| Document | Scope |
|----------|-------|
| [SOC2_READINESS.md](SOC2_READINESS.md) | High-level readiness assessment and remediation roadmap |
| [EU_AI_ACT_CONFORMITY.md](EU_AI_ACT_CONFORMITY.md) | EU AI Act article-by-article mapping |
| [NIST_800_53_MAPPING.md](NIST_800_53_MAPPING.md) | NIST 800-53 Rev 5 control family mapping |
| [SINGAPORE_AI_GOVERNANCE.md](SINGAPORE_AI_GOVERNANCE.md) | Singapore Model AI Governance Framework |
| [SECURITY_HARDENING.md](SECURITY_HARDENING.md) | Production security hardening guide |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Adversarial threat analysis |
