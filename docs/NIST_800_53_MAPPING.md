# NIST 800-53 Rev 5 Control Mapping — Nexus OS

## Overview

This document maps Nexus OS v10.5.0 controls to NIST Special Publication 800-53 Revision 5 control families. Focus is on families most relevant to AI agent operating systems.

**Assessment Date:** 2026-03-31
**Framework:** NIST SP 800-53 Rev 5 (September 2020)
**System Version:** 10.5.0

**Status Key:**
- **IMPLEMENTED** — Feature is built, tested, and active
- **PARTIAL** — Core capability exists, additional work needed
- **PLANNED** — Scheduled for development

---

## AC — Access Control

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| AC-1 | Policy and Procedures | Cedar-inspired policy engine loads TOML governance rules; default-deny evaluation | `kernel/src/policy_engine/mod.rs` — `PolicyEngine` | IMPLEMENTED |
| AC-2 | Account Management | Agent lifecycle: registration, manifest-based provisioning, fuel allocation, suspension, decommissioning | `kernel/src/supervisor.rs` — `Supervisor` | IMPLEMENTED |
| AC-3 | Access Enforcement | Capability-based ACL — no ambient authority; agents can only invoke explicitly granted capabilities | `kernel/src/permissions.rs` — `PermissionManager` | IMPLEMENTED |
| AC-4 | Information Flow Enforcement | Egress governor: per-agent URL allowlisting + rate limiting; firewall blocks unauthorized data flows | `kernel/src/firewall/egress.rs` — `EgressGovernor` | IMPLEMENTED |
| AC-5 | Separation of Duties | HITL consent gates require human approval for Tier2+ actions; agent cannot approve its own escalations | `kernel/src/consent.rs` — `ConsentRuntime`, `HitlTier` | IMPLEMENTED |
| AC-6 | Least Privilege | Agents receive only capabilities declared in manifest.toml; delegation narrowing (OWASP #4) enforces subset on sub-agents | `kernel/src/owasp_defenses.rs` — `DelegationNarrowing` | IMPLEMENTED |
| AC-7 | Unsuccessful Logon Attempts | 3-strike halt: safety supervisor suspends agents after repeated governance violations | `kernel/src/safety_supervisor.rs` | IMPLEMENTED |
| AC-11 | Device Lock | Fuel exhaustion halts agent; circuit breaker opens after failure threshold | `kernel/src/supervisor.rs`, `kernel/src/owasp_defenses.rs` — `CircuitBreaker` | IMPLEMENTED |
| AC-17 | Remote Access | EdDSA JWT authentication on all API endpoints; OIDC-A claims with JWKS discovery | `kernel/src/identity/token_manager.rs` — `TokenManager` | IMPLEMENTED |
| AC-24 | Access Control Decisions | Multi-dimensional: AutonomyLevel (L0–L6) × HitlTier (0–3) × capability check × fuel availability | `kernel/src/autonomy.rs` — `AutonomyGuard`, `kernel/src/consent.rs` | IMPLEMENTED |

**Test Evidence:** `test_actuator_rejects_missing_capability`, `test_capability_check_required`, `test_no_capability_escalation`, `test_deny_overrides_allow_always`, `test_blocked_on_hitl_for_low_autonomy`, `test_3_strike_halt`, `test_fail_closed_fuel_exhaustion`

---

## AU — Audit and Accountability

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| AU-2 | Event Logging | Every agent action recorded: StateChange, ToolCall, LlmCall, Error, UserAction | `kernel/src/audit/mod.rs` — `AuditTrail`, `EventType` | IMPLEMENTED |
| AU-3 | Content of Audit Records | Each event: UUID, timestamp, agent_id, event_type, payload, previous_hash, hash | `kernel/src/audit/mod.rs` — `AuditEvent` | IMPLEMENTED |
| AU-4 | Audit Log Storage Capacity | Configurable retention with Merkle tree archival; distributed block batching | `kernel/src/audit/mod.rs` — `enable_retention()`, `kernel/src/audit/retention.rs` | IMPLEMENTED |
| AU-5 | Response to Audit Processing Failures | Audit system is embedded in kernel and cannot be disabled; errors propagated as `AuditError` | `kernel/src/audit/mod.rs` — `AuditError` | IMPLEMENTED |
| AU-8 | Time Stamps | Each `AuditEvent` includes Unix timestamp; events are append-only and chronologically ordered | `kernel/src/audit/mod.rs` — `AuditEvent.timestamp` | IMPLEMENTED |
| AU-9 | Protection of Audit Information | SHA-256 hash chain: each event links to predecessor; tampering detectable via `verify_integrity()` | `kernel/src/audit/mod.rs` — `verify_integrity()`, `verify_full_integrity()` | IMPLEMENTED |
| AU-10 | Non-Repudiation | Ed25519 signed audit blocks; distributed consensus via PBFT for multi-node tamper evidence | `distributed/src/immutable_audit.rs` — `AuditBlock`, `distributed/src/pbft.rs` | IMPLEMENTED |
| AU-11 | Audit Record Retention | Configurable retention; Merkle tree archival preserves integrity proofs for aged-out events | `kernel/src/audit/retention.rs` | IMPLEMENTED |
| AU-12 | Audit Record Generation | Automatic — every governance decision, tool call, LLM invocation, and consent action generates an audit event | `kernel/src/audit/mod.rs` — `append_event()` | IMPLEMENTED |

**Test Evidence:** `test_audit_chain_integrity_passes`, `test_audit_chain_detects_tampering`, `test_persistent_audit_hash_chain_integrity`, `test_persistent_audit_chain_detects_tamper`, `test_audit_log_chain_integrity`, `test_audit_log_tamper_detection`, `test_action_hash_chained_in_audit`, `test_action_sequence_integrity`, `test_tamper_detection`

---

## CA — Assessment, Authorization, and Monitoring

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| CA-2 | Control Assessments | Governance KPI engine: 4-category scoring (error rate, audit chain, fuel efficiency, budget adherence) | `kernel/src/governance_kpi.rs` | IMPLEMENTED |
| CA-5 | Plan of Action and Milestones | Capability measurement: 4-vector scoring at 5 difficulty levels with gaming detection | `kernel/src/immune/arena.rs`, `kernel/src/cognitive/` | IMPLEMENTED |
| CA-7 | Continuous Monitoring | Health endpoint, Prometheus metrics, anomaly monitor, burn anomaly detector | `protocols/src/http_gateway.rs`, `kernel/src/owasp_defenses.rs` — `AnomalyMonitor` | IMPLEMENTED |
| CA-8 | Penetration Testing | Adversarial arena: sandboxed testing of agents against adversarial inputs; Darwin Core evolves attacks | `kernel/src/immune/arena.rs`, `kernel/src/immune/detector.rs` | IMPLEMENTED |

**Test Evidence:** `test_compute_kpis_returns_all_categories`, `test_generate_scorecard`, `test_run_session`, `test_round_results_sequential`

---

## CM — Configuration Management

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| CM-2 | Baseline Configuration | Agent manifests (manifest.toml) define capabilities, fuel budget, autonomy level, schedule, model | Agent manifest schema in `DEPLOYMENT.md` | IMPLEMENTED |
| CM-3 | Configuration Change Control | Permission changes audited via `PermissionHistoryEntry`; policy engine enforces governance rules | `kernel/src/permissions.rs` — `PermissionHistoryEntry`, `PermissionAction` | IMPLEMENTED |
| CM-5 | Access Restrictions for Change | Governance policy modification requires L4+ autonomy and Tier3 HITL approval | `kernel/src/consent.rs` — `GovernedOperation::GovernancePolicyModify` | IMPLEMENTED |
| CM-7 | Least Functionality | Agents only receive declared capabilities; default-deny policy evaluation | `kernel/src/policy_engine/mod.rs`, `kernel/src/permissions.rs` | IMPLEMENTED |
| CM-8 | System Component Inventory | 65-crate workspace; agent registry with DID-based identity; marketplace package registry | `kernel/src/supervisor.rs`, `marketplace/src/sqlite_registry.rs` | IMPLEMENTED |
| CM-11 | User-Installed Software | Marketplace packages require Ed25519 signature and in-toto attestation; OWASP #7 runtime verification | `marketplace/src/package.rs` — `SignedPackageBundle`, `kernel/src/owasp_defenses.rs` — `RuntimePackageVerifier` | IMPLEMENTED |

**Test Evidence:** `test_capability_grant_and_revoke_history`, `test_capability_persistence_across_reopen`

---

## IA — Identification and Authentication

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| IA-2 | Identification and Authentication | Ed25519 keypair per agent; DID (`did:key:z6Mk...`) derived from public key; hardware-backed storage | `kernel/src/identity/agent_identity.rs` — `AgentIdentity`, `IdentityManager` | IMPLEMENTED |
| IA-3 | Device Identification | Device pairing protocol for distributed mesh; attestation reports | `distributed/src/device_pairing.rs`, `kernel/src/hardware_security/manager.rs` — `AttestationReport` | IMPLEMENTED |
| IA-4 | Identifier Management | DIDs are self-sovereign; multiple agents get independent keys; key rotation with audit | `kernel/src/identity/agent_identity.rs` — `PersistedIdentity` | IMPLEMENTED |
| IA-5 | Authenticator Management | Hardware key backends: TEE, TPM 2.0, Secure Enclave, software fallback; sealed at-rest storage | `kernel/src/hardware_security/manager.rs` — `KeyBackend`, `SealedKeyStore` | IMPLEMENTED |
| IA-8 | Identification and Authentication (Non-Org) | OIDC-A token format; EdDSA JWT with JWKS endpoint for federated verification | `kernel/src/identity/token_manager.rs`, `protocols/src/http_gateway.rs` — `/auth/jwks` | IMPLEMENTED |
| IA-9 | Service Identification | A2A agent cards provide machine-readable identity and capability discovery | `protocols/src/http_gateway.rs` — `/a2a/agent-card` | IMPLEMENTED |

**Test Evidence:** `test_software_backend_generate_sign_verify`, `test_agent_identity_uses_key_manager`, `test_agent_identity_persistence_no_plaintext`, `test_sealed_storage_roundtrip`, `test_sealed_storage_tamper_detection`, `test_attestation_report_valid`, `test_attestation_report_expired`, `test_multiple_agents_independent_keys`, `test_rotation_emits_audit_event`

---

## IR — Incident Response

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| IR-4 | Incident Handling | Circuit breaker (OWASP #8): Closed → Open → HalfOpen state machine; automatic recovery attempts | `kernel/src/owasp_defenses.rs` — `CircuitBreaker` | IMPLEMENTED |
| IR-5 | Incident Monitoring | Anomaly monitor (OWASP #10): spike detection triggers auto-suspension; burn anomaly detector for fuel | `kernel/src/owasp_defenses.rs` — `AnomalyMonitor`, `kernel/src/supervisor.rs` — `BurnAnomalyDetector` | IMPLEMENTED |
| IR-6 | Incident Reporting | Safety supervisor generates incident reports; audit trail provides forensic evidence | `kernel/src/safety_supervisor.rs`, `kernel/tests/safety_supervisor_phase6.rs` | IMPLEMENTED |
| IR-7 | Incident Response Assistance | Kill gates: manual override halts agent immediately; unfreeze requires Tier3 approval | `kernel/tests/kill_gates_phase7.rs` | IMPLEMENTED |

**Test Evidence:** `test_circuit_breaker_halts_agent`, `test_3_strike_halt`, `test_incident_report_structure`, `test_manual_override_halts_agent_immediately`, `test_screen_poster_freeze_from_kpi`, `test_unfreeze_requires_tier3`

---

## MA — Maintenance

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| MA-2 | Controlled Maintenance | Marketplace packages with Ed25519 signing and in-toto attestation; SLSA build provenance | `marketplace/src/package.rs` — `InTotoAttestation` | IMPLEMENTED |
| MA-3 | Maintenance Tools | Checkpoint-rollback system: full state snapshots with side-effect compensation actions | `kernel/src/checkpoint.rs` — `CheckpointManager`, `CompensationAction` | IMPLEMENTED |
| MA-6 | Timely Maintenance | Agent self-improvement via Darwin Core; fitness evaluation with generational evolution | `adaptation/src/evolution.rs`, `kernel/src/immune/arena.rs` | IMPLEMENTED |

**Test Evidence:** `test_create_checkpoint`, `test_undo_file_write`, `test_rollback`, `test_strategy_rollback`

---

## MP — Media Protection

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| MP-2 | Media Access | WASM sandboxing: untrusted agent code runs in isolated WebAssembly environment; no host filesystem access without capability | `sdk/src/wasmtime_sandbox.rs` | IMPLEMENTED |
| MP-4 | Media Storage | Hardware-backed sealed key store; encrypted at rest; no plaintext credential storage | `kernel/src/hardware_security/manager.rs` — `SealedKeyStore` | IMPLEMENTED |
| MP-5 | Media Transport | Ghost protocol: encrypted agent-to-agent communication; TLS for HTTP transport | `distributed/src/ghost_protocol.rs` | IMPLEMENTED |

**Test Evidence:** `test_wasm_sandbox_blocks_unauthorized_path`, `test_sealed_storage_roundtrip`, `test_sealed_keys_survive_restart`

---

## PE — Physical and Environmental Protection

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| PE-3 | Physical Access Control | Air-gapped deployment capability: fully operational without internet; local-first architecture | Deployment docs, `docs/DEPLOYMENT.md` — Air-Gapped section | IMPLEMENTED |
| PE-17 | Alternate Work Site | Docker + Helm deployment for any infrastructure; portable binary deployment | `Dockerfile`, `helm/nexus-os/`, `docker-compose.yml` | IMPLEMENTED |

---

## PL — Planning

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| PL-2 | Security and Privacy Plans | Governance kernel architecture formally defines seven-layer defense; threat model documented | `ARCHITECTURE.md`, `THREAT_MODEL.md`, `SECURITY.md` | IMPLEMENTED |
| PL-8 | Security and Privacy Architectures | Formal autonomy taxonomy (L0–L6); multi-dimensional risk model: HitlTier × AutonomyLevel × KPI | `kernel/src/autonomy.rs` — `AutonomyLevel`, `kernel/src/consent.rs` — `HitlTier` | IMPLEMENTED |

---

## PM — Program Management

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| PM-4 | Plan of Action and Milestones | OWASP Agentic Top 10: all 10 defenses implemented with 62 dedicated tests | `kernel/src/owasp_defenses.rs` | IMPLEMENTED |
| PM-9 | Risk Management Strategy | Multi-layer defense: identity → ACL → autonomy → HITL → fuel → sandbox → firewall | Entire governance kernel | IMPLEMENTED |

---

## RA — Risk Assessment

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| RA-3 | Risk Assessment | Speculative engine: shadow simulation before Tier2+ approval; predicts file changes, network calls, data modifications | `kernel/src/speculative.rs` — `SpeculativeEngine`, `SimulationResult` | IMPLEMENTED |
| RA-5 | Vulnerability Monitoring | Immune system: threat signature database, antibody response generation, hive-mind threat sharing | `kernel/src/immune/detector.rs`, `kernel/src/immune/antibody.rs`, `kernel/src/immune/hive.rs` | IMPLEMENTED |
| RA-7 | Risk Response | World simulation engine: dry-run execution in sandboxed environment before real deployment | `crates/nexus-world-simulation/src/` — `SimulationEngine` | IMPLEMENTED |

**Test Evidence:** `test_scan_injection`, `test_scan_exfiltration`, `test_spawn_injection_antibody`, `test_propagate_and_lookup`, `test_sandbox_file_write`, `test_sandbox_http_risk`

---

## SA — System and Services Acquisition

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| SA-4 | Acquisition Process | Marketplace with Ed25519 package signing; in-toto attestation with SLSA level; SBOM reference | `marketplace/src/package.rs` — `SignedPackageBundle`, `InTotoAttestation` | IMPLEMENTED |
| SA-10 | Developer Configuration Management | 65-crate workspace; Cargo.lock pins all dependencies; CI gates changes | Workspace `Cargo.toml` | IMPLEMENTED |
| SA-11 | Developer Testing | 5,029 tests; adversarial arena stress-tests agents; Darwin Core evolves for robustness | CI pipeline, `kernel/src/immune/arena.rs` | IMPLEMENTED |
| SA-12 | Supply Chain Protection | OWASP #7 RuntimePackageVerifier: load-time Ed25519 signature verification; malware scanner | `kernel/src/owasp_defenses.rs` — `RuntimePackageVerifier`, `marketplace/src/scanner.rs` | IMPLEMENTED |

---

## SC — System and Communications Protection

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| SC-2 | Separation of User Functionality | Agent code runs in WASM sandbox; governance kernel runs in host process; strict boundary | `sdk/src/wasmtime_sandbox.rs` | IMPLEMENTED |
| SC-4 | Information in Shared Resources | Agent isolation: each agent has independent identity, fuel ledger, capability set; no shared mutable state | `kernel/src/supervisor.rs` — `AgentHandle` | IMPLEMENTED |
| SC-7 | Boundary Protection | Prompt firewall: 20 injection patterns block malicious input; output firewall filters responses; egress governor rate-limits | `kernel/src/firewall/prompt_firewall.rs` — `PromptFirewall`, `InputFilter`, `OutputFilter` | IMPLEMENTED |
| SC-8 | Transmission Confidentiality | EdDSA JWT tokens for API auth; ghost protocol for encrypted A2A; HTTPS for all external calls | `kernel/src/identity/token_manager.rs`, `distributed/src/ghost_protocol.rs` | IMPLEMENTED |
| SC-13 | Cryptographic Protection | Ed25519 signing (current); X25519 key exchange; post-quantum roadmap: ML-DSA, ML-KEM, SLH-DSA | `crates/nexus-crypto/src/lib.rs` — `CryptoIdentity`, `SignatureAlgorithm` | IMPLEMENTED |
| SC-28 | Protection of Information at Rest | Sealed key store: encrypted at-rest key storage with hardware backend integration | `kernel/src/hardware_security/manager.rs` — `SealedKeyStore` | IMPLEMENTED |
| SC-39 | Process Isolation | WASM sandbox with epoch-based interruption; capability-gated host function calls | `sdk/src/wasmtime_sandbox.rs` | IMPLEMENTED |

**Test Evidence:** `test_scan_injection`, `test_scan_pii`, `test_injection_in_web_content_flagged`, `test_nested_encoding_attack`, `test_gateway_sends_redacted_payload_only`, `test_sanitize_injection_attempt`

---

## SI — System and Information Integrity

| NIST ID | Control Name | Nexus OS Implementation | Code Reference | Status |
|---------|-------------|------------------------|----------------|--------|
| SI-2 | Flaw Remediation | CI/CD: `cargo clippy -D warnings` enforces zero warnings; 5,029 tests gate all changes | CI pipeline | IMPLEMENTED |
| SI-3 | Malicious Code Protection | Tool poisoning guard (OWASP #2): output scanning + rate limiting; immune system threat detection | `kernel/src/owasp_defenses.rs` — `ToolPoisoningGuard`, `kernel/src/immune/detector.rs` | IMPLEMENTED |
| SI-4 | System Monitoring | Prometheus metrics; governance KPI scoring; anomaly monitor; burn anomaly detector | `protocols/src/http_gateway.rs` — `metrics_endpoint`, `kernel/src/governance_kpi.rs` | IMPLEMENTED |
| SI-5 | Security Alerts | Safety supervisor incident reports; circuit breaker state transitions; agent suspension notifications | `kernel/src/safety_supervisor.rs`, `kernel/src/owasp_defenses.rs` | IMPLEMENTED |
| SI-7 | Software and Information Integrity | Hash-chained audit trail; Ed25519 signed audit blocks; marketplace package verification | `kernel/src/audit/mod.rs`, `distributed/src/immutable_audit.rs` | IMPLEMENTED |
| SI-10 | Information Input Validation | Prompt firewall input filter: injection detection, PII scanning; memory write validator (OWASP #6) | `kernel/src/firewall/prompt_firewall.rs` — `InputFilter`, `kernel/src/owasp_defenses.rs` — `MemoryWriteValidator` | IMPLEMENTED |
| SI-15 | Information Output Filtering | Output firewall: schema validation, exfiltration detection, PII redaction before delivery | `kernel/src/firewall/prompt_firewall.rs` — `OutputFilter` | IMPLEMENTED |
| SI-16 | Memory Protection | Memory write validator (OWASP #6): sanitize + rate limit all agent memory writes | `kernel/src/owasp_defenses.rs` — `MemoryWriteValidator` | IMPLEMENTED |

**Test Evidence:** `test_scan_injection`, `test_scan_pii`, `test_scan_exfiltration`, `test_redaction_before_llm_call`, `test_output_validation_blocks_unauthorized`, `test_data_instruction_separation`

---

## Coverage Summary

| Control Family | Controls Mapped | Implemented | Partial | Planned |
|---------------|----------------|-------------|---------|---------|
| AC — Access Control | 10 | 10 | 0 | 0 |
| AU — Audit and Accountability | 9 | 9 | 0 | 0 |
| CA — Assessment | 4 | 4 | 0 | 0 |
| CM — Configuration Management | 6 | 6 | 0 | 0 |
| IA — Identification and Authentication | 6 | 6 | 0 | 0 |
| IR — Incident Response | 4 | 4 | 0 | 0 |
| MA — Maintenance | 3 | 3 | 0 | 0 |
| MP — Media Protection | 3 | 3 | 0 | 0 |
| PE — Physical and Environmental | 2 | 2 | 0 | 0 |
| PL — Planning | 2 | 2 | 0 | 0 |
| PM — Program Management | 2 | 2 | 0 | 0 |
| RA — Risk Assessment | 3 | 3 | 0 | 0 |
| SA — System and Services Acquisition | 4 | 4 | 0 | 0 |
| SC — System and Communications Protection | 7 | 7 | 0 | 0 |
| SI — System and Information Integrity | 8 | 8 | 0 | 0 |
| **Total** | **73** | **73** | **0** | **0** |
