# Nexus OS — Production Acceptance Criteria

> **Version:** 1.0
> **Date:** 2026-03-11
> **Authors:** Suresh Karicheti (Creator), Claude — Lead Architect
> **Applies to:** Nexus OS v7.0.0+

This document defines the mandatory acceptance criteria that must be satisfied before any Nexus OS release is promoted to production. Every criterion is measurable, has an explicit pass/fail threshold, and references the tool or endpoint used for verification.

---

## 1. Test Suite Requirements

| Criterion | Threshold | Current | Tool |
|-----------|-----------|---------|------|
| Minimum Rust tests passing | ≥ 1,300 | 1,376 | `cargo test --workspace --all-features` |
| Test failures | 0 allowed | 0 | `cargo test --workspace --all-features` |
| Feature flag coverage | All flags compiled and tested | Yes | `--all-features` flag |
| Python voice tests | 12/12 passing | 12/12 | `cd voice && python3 -m pytest tests/ -v` |
| Frontend smoke tests | All passing | 1/1 | `cd app && npm test` |
| Frontend type check | Zero type errors | 0 | `cd app && npx tsc --noEmit` |
| CI test duration | < 15 minutes wall clock | ~8 min | GitLab CI job timer |

**Rationale:** The 1,300 minimum is set 5% below the current count to allow for intentional test consolidation without triggering a false alarm. Any drop below 1,300 indicates accidental test deletion and must be investigated.

All feature flags (`hardware-tpm`, `hardware-secure-enclave`, `hardware-tee`, `real-claude`, `real-api-tests`, `local-slm`, `playwright-process`, `platform-linux`, `platform-macos`, `platform-windows`, `tauri-runtime`) are compiled and tested via `--all-features` in CI.

---

## 2. Security Requirements

| Criterion | Threshold | Tool |
|-----------|-----------|------|
| Critical RUSTSEC advisories | 0 unmitigated | `cargo audit` |
| License compliance | All checks pass | `cargo deny check` |
| Advisory compliance | All checks pass | `cargo deny check advisories` |
| Ban compliance | All checks pass | `cargo deny check bans` |
| Source compliance | All checks pass | `cargo deny check sources` |
| WASM agent bundle signing | `SignaturePolicy::RequireSigned` enforced | Kernel marketplace verification |
| Release binary signing | Cosign/Sigstore keyless signing | `cosign verify-blob` |
| SBOM generation | CycloneDX 1.5 JSON for every release | `cargo cyclonedx` + npm SBOM |
| Provenance attestation | SLSA Level 2 in-toto format | `sign-release` CI job |

**Advisory Management:** Known advisories without available fixes may be temporarily allowed in `deny.toml` with an accompanying comment explaining the risk assessment. The allow list must be reviewed on every release. Stale entries (advisories no longer matching any dependency) must be removed.

**Signing Policy:** Every WASM agent bundle distributed through the marketplace must carry an Ed25519 signature verified against the Nexus signing key. Unsigned bundles are rejected at install time when `SignaturePolicy::RequireSigned` is active.

---

## 3. Performance Thresholds

| Metric | Warn | Critical | Measured By |
|--------|------|----------|-------------|
| Governance overhead | ≥ 5% | ≥ 10% | `SafetySupervisor` KPI: `GovernanceOverhead` |
| LLM response latency | ≥ 5,000 ms | ≥ 15,000 ms | `SafetySupervisor` KPI: `LlmLatency` |
| Cold start time (kernel init) | — | ≥ 3,000 ms | Benchmark: `nexus benchmark run` |
| Fuel metering accuracy | — | > 1 unit error/call | Ceiling arithmetic in `fuel_hardening.rs` |
| Audit chain integrity | — | < 100% pass rate | `GET /health` → `audit_chain_valid` |

**Governance Overhead:** Measured as the percentage of wall-clock time spent in capability checks, fuel accounting, and audit logging relative to the total action execution time. The target is < 5%. Exceeding 10% triggers a safety action.

**Fuel Metering Accuracy:** The token-to-fuel conversion uses ceiling arithmetic (`(tokens * cost_per_1k + 999) / 1000`) with saturating math to prevent overflow. This guarantees the system never undercharges — the maximum rounding error is < 1 fuel unit per API call. This is by design: overcharging by a fractional unit is the safe default.

**Audit Chain Integrity:** Every call to `GET /health` recomputes the SHA-256 hash chain from genesis through all recorded events. The `audit_chain_valid` field must always return `true`. A single `false` indicates tampering or corruption and is a critical incident.

---

## 4. Safety & Governance KPIs

| KPI | Warn | Critical | Source |
|-----|------|----------|--------|
| Agent error rate | ≥ 10% | ≥ 25% | `SafetySupervisor` KPI: `AgentErrorRate` |
| Budget compliance | ≥ 90% consumed | ≥ 100% consumed | `SafetySupervisor` KPI: `BudgetCompliance` |
| Fuel burn rate | ≥ 90% of allocation | ≥ 100% | `SafetySupervisor` KPI: `FuelBurnRate` |
| Ban rate | ≥ 2/period | ≥ 5/period | `SafetySupervisor` KPI: `BanRate` |
| Replay mismatch | Any occurrence | Any occurrence | `SafetySupervisor` KPI: `ReplayMismatch` |
| Divergence | Any occurrence | Any occurrence | `SafetySupervisor` KPI: `Divergence` |
| Quorum invariant | Any violation | Any violation | `SafetySupervisor` KPI: `QuorumInvariant` |

**Three-Strike Safety Model:**

| Strike | Action | Effect |
|--------|--------|--------|
| 1st violation | `SafetyAction::Continue` | Warning logged, agent continues |
| 2nd violation | `SafetyAction::Degraded` | Agent capabilities restricted |
| 3rd+ violation | `SafetyAction::Halted` | Agent stopped, incident report generated |

**Circuit Breaker (LLM Providers):**
- Failure threshold: 5 consecutive failures → circuit opens
- Reset timeout: 30 seconds → circuit transitions to half-open
- Half-open: single test request allowed; success closes circuit, failure reopens

**Trust Score Thresholds:**
- Promotion (autonomy level increase): trust score ≥ 0.85
- Demotion (autonomy level decrease): trust score ≤ 0.30
- Violation cooldown: 86,400 seconds (24 hours) before trust recovery begins
- Each policy violation reduces trust by 20% (`violation_penalty = 1.0 - violations × 0.2`)

---

## 5. Reliability Requirements

| Requirement | Implementation | Verification |
|-------------|----------------|--------------|
| Audit chain hash integrity | SHA-256 chain with genesis block, verified on every `/health` call | `GET /health` → `audit_chain_valid: true` |
| Fail-closed design | Internal errors block operation; no silent pass-through | `FuelExhausted`, `CapabilityDenied` errors halt execution |
| Atomic fuel reservation | `reserve_fuel()` returns `FuelReservation`; auto-refund on `Drop` if not committed | Unit tests in `fuel_hardening.rs` |
| Subprocess resource limits | `rlimit` enforcement on Linux for CPU time, memory, file descriptors | `nexus-control` crate with platform-specific guards |
| Process group killing | `kill(-pgid, SIGKILL)` on timeout — no orphaned grandchildren | E2E tests: `e2e_system_workflows.rs` |
| Mutex poisoning fail-closed | Poisoned fuel lock panics with "fuel lock poisoned" | Fail-closed by design |

**Concurrency Guarantees:** The fuel context uses `Mutex`-protected state. Concurrent `deduct_fuel()` and `reserve_fuel()` calls are serialized. Tests verify no overdraw under concurrent reservation scenarios.

---

## 6. Supply Chain Requirements

| Requirement | Threshold | Tool |
|-------------|-----------|------|
| Rust dependency SBOM | ≥ 800 components listed | `cargo cyclonedx --format json` |
| npm dependency SBOM | ≥ 240 components listed | `cd app && npm sbom --sbom-format cyclonedx` |
| SBOM format | CycloneDX 1.5 JSON | Schema validation in `verify-sbom` CI job |
| Provenance attestation | In-toto format with real timestamps | `sign-release` CI job |
| Signing method | Cosign keyless (Sigstore) | `cosign sign-blob --yes` |
| Signing manifest | SHA-256 hash per artifact, non-empty signature per artifact | `ReleaseVerifier::verify_manifest()` |
| Artifact hash length | Exactly 64 hex characters (SHA-256) | `verify_artifact_hash()` in `release_signing.rs` |

**Verification Pipeline:** The `ReleaseVerifier::verify_manifest()` checks every artifact for: non-empty hash, non-empty signature, valid hex format (64 chars). Results are classified as `AllValid`, `PartialFailure`, or `AllFailed`. Only `AllValid` passes the release gate.

---

## 7. CI Pipeline Requirements

| Stage | Jobs | Must Pass | Blocks Release |
|-------|------|-----------|----------------|
| **security** | `cargo-audit`, `cargo-deny` | Yes | Yes |
| **test** | `rust-tests`, `voice-tests`, `frontend-tests`, `code-coverage` | Yes (coverage advisory) | Yes (except coverage) |
| **release** | `build-release`, `build-agent-bundles` | Yes | Yes |
| **sign** | `sign-release`, `verify-signatures`, `verify-sbom` | Yes | Yes |

**Pipeline Rules:**
- Security jobs run on every push and must not have `allow_failure: true` (except `code-coverage`)
- `code-coverage` generates Cobertura XML via `cargo-tarpaulin`; currently advisory (`allow_failure: true`)
- Coverage threshold will be enforced once a baseline is established (target: TBD after 3 release cycles)
- All feature flags are tested in CI via `--all-features`
- Tag pushes trigger the full pipeline: security → test → release → sign

**Feature Matrix:**

All 11 feature flags across 4 crates are compiled and tested:

| Crate | Flags |
|-------|-------|
| `nexus-kernel` | `hardware-tpm`, `hardware-secure-enclave`, `hardware-tee` |
| `nexus-connectors-llm` | `real-claude`, `real-api-tests`, `local-slm` |
| `nexus-control` | `playwright-process`, `platform-linux`, `platform-macos`, `platform-windows` |
| `nexus-desktop-backend` | `tauri-runtime` |

---

## 8. Compliance Requirements

| Requirement | Status | Implementation |
|-------------|--------|----------------|
| EU AI Act risk classification | Active | Risk tier mapping in governance layer |
| Transparency reports | Generated | Audit trail exports with agent decision rationale |
| Cryptographic erasure | Available | AES-256 key destruction for data removal |
| Data provenance tracking | Active | Hash-chained audit trail with agent attribution |
| Semantic boundary filter | Active | PII redaction at LLM gateway boundary (Architecture Invariant #4) |
| Platform posting limits | Enforced | X: 300/3hr, Instagram: 25/day, Facebook: 50/day |
| Autonomy level gates | Enforced | L0–L5 with HITL approval mandatory for Tier1+ |

**Privacy by Design:** All agent actions are recorded in an append-only, hash-chained audit trail. PII is redacted before reaching LLM providers. Secrets are protected with AES-256 encryption at rest. Cryptographic erasure is available for complete data removal.

---

## 9. Measurement & Verification Matrix

Every criterion above maps to a concrete verification method:

| # | Criterion | What Is Measured | How | Pass/Fail | Evidence Location |
|---|-----------|-----------------|-----|-----------|-------------------|
| 1 | Test count | Number of passing tests | `cargo test --workspace --all-features 2>&1 \| grep "test result"` | ≥ 1,300 passed, 0 failed | CI job log: `rust-tests` |
| 2 | Voice tests | Python test pass rate | `cd voice && python3 -m pytest tests/ -v` | 12/12 pass | CI job log: `voice-tests` |
| 3 | Frontend build | TypeScript compilation | `cd app && npm run build && npx tsc --noEmit` | Zero errors | CI job log: `frontend-tests` |
| 4 | Security advisories | Known vulnerabilities | `cargo audit` | 0 critical unmitigated | CI job log: `cargo-audit` |
| 5 | License compliance | Dependency licenses | `cargo deny check` | All 4 checks pass | CI job log: `cargo-deny` |
| 6 | Governance overhead | Wall-clock % in governance | `SafetySupervisor` KPI sampling | < 10% (critical) | Runtime KPI log |
| 7 | LLM latency | End-to-end LLM call time | `SafetySupervisor` KPI: `LlmLatency` | < 15,000 ms (critical) | Runtime KPI log |
| 8 | Audit integrity | Hash chain validity | `GET /health` → `audit_chain_valid` | `true` on every check | Health endpoint JSON |
| 9 | Fuel accuracy | Rounding error per call | Ceiling arithmetic proof | < 1 unit/call | Code review: `fuel_hardening.rs` |
| 10 | SBOM completeness | Component count | `verify-sbom` CI job | ≥ 100 Rust + ≥ 10 npm | CI artifact: `sbom/` |
| 11 | Binary signing | Cosign signature presence | `cosign verify-blob` | Valid signature | CI artifact: `signatures/` |
| 12 | Provenance | In-toto attestation | `sign-release` CI job | Valid JSON with timestamps | CI artifact: `provenance.json` |
| 13 | Signing manifest | Artifact hash verification | `ReleaseVerifier::verify_manifest()` | `AllValid` result | CI job log: `verify-signatures` |
| 14 | Three-strike model | Violation escalation | Unit tests in `safety_supervisor.rs` | Correct escalation sequence | Test suite |
| 15 | Circuit breaker | Failure isolation | Unit tests in `circuit_breaker.rs` | Opens at 5 failures, resets at 30s | Test suite |
| 16 | Trust thresholds | Promotion/demotion | Unit tests in `adaptive_policy.rs` | Promote ≥ 0.85, demote ≤ 0.30 | Test suite |

---

## 10. Release Gate Checklist

The following checklist must be completed — in order — before any release tag is pushed:

- [ ] `cargo fmt --all -- --check` passes with no formatting issues
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes with zero warnings
- [ ] `cargo test --workspace --all-features` passes: ≥ 1,300 tests, 0 failures
- [ ] `cd voice && python3 -m pytest tests/ -v` passes: 12/12 tests
- [ ] `cd app && npm run build` succeeds with zero errors
- [ ] `cd app && npx tsc --noEmit` passes with zero type errors
- [ ] `cargo audit` reports zero critical unmitigated advisories
- [ ] `cargo deny check` passes all 4 checks (advisories, bans, licenses, sources)
- [ ] Stale `deny.toml` allow entries removed
- [ ] SBOM generated: `cargo cyclonedx` + `npm sbom`
- [ ] Provenance attestation generated with real timestamps
- [ ] Release binaries signed via Cosign/Sigstore
- [ ] Signing manifest verified: `ReleaseVerifier::verify_manifest()` returns `AllValid`
- [ ] `GET /health` returns `audit_chain_valid: true`
- [ ] This acceptance criteria document reviewed and version-appropriate

---

## Appendix A: Threshold Quick Reference

| Metric | Green | Yellow (Warn) | Red (Critical) |
|--------|-------|---------------|----------------|
| Governance overhead | < 5% | 5–10% | ≥ 10% |
| LLM latency | < 5s | 5–15s | ≥ 15s |
| Agent error rate | < 10% | 10–25% | ≥ 25% |
| Fuel burn rate | < 90% | 90–100% | ≥ 100% |
| Budget compliance | < 90% | 90–100% | ≥ 100% |
| Ban rate | < 2 | 2–5 | ≥ 5 |
| Trust score | ≥ 0.85 (promote) | 0.30–0.85 (hold) | ≤ 0.30 (demote) |
| Audit chain | valid | — | invalid |
| Test count | ≥ 1,300 | — | < 1,300 |
| Critical advisories | 0 | — | ≥ 1 |

## Appendix B: Document History

| Version | Date | Change |
|---------|------|--------|
| 1.0 | 2026-03-11 | Initial acceptance criteria based on v7.0.0 baseline |
