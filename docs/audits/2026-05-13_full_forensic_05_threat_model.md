# Full Forensic Audit — Phase 5: Threat Model Regression

**Date:** 2026-05-13
**HEAD:** efb06aa2

---

## 1. OWASP Agentic Top 10

OWASP defenses module present at `kernel/src/owasp_defenses.rs`. Contains:

- **GoalIntegrityGuard** (line 57) — Prevents goal hijacking / prompt injection on agent tasks
- **DelegationNarrowing** (line 191) — Ensures delegated capabilities cannot expand
- **MemoryWriteValidator** (line 329) — Validates memory writes for poisoning attacks

### Test Coverage

| OWASP Risk | Defense | Test Status |
|-----------|---------|-------------|
| Agent goal hijacking | GoalIntegrityGuard | Code present; kernel tests cover goal verification |
| Capability escalation via delegation | DelegationNarrowing | Code present; max_depth enforcement tested |
| Memory poisoning | MemoryWriteValidator | Code present |
| Prompt injection (semantic boundary) | SemanticBoundaryDefense | 6 integration tests PASS (semantic_boundary_integration_tests) |
| Resource exhaustion | Budget/fuel metering | Budget tests PASS in nexus-swarm; fuel tests in kernel |
| Governance bypass via self-improvement | 10 hard invariants | 101 tests PASS in nexus-self-improve |
| Synthetic attacks / threat evolution | nexus-governance-evolution | 7 tests PASS including attack cycle and child budget check |
| Shell injection | ShellExecutor sandboxing | 10 tests PASS (shell_executor_tests) including blocked sudo/rm |

---

## 2. Egress Allowlist

| Component | Status |
|-----------|--------|
| EgressGovernor in kernel/src/protocols/mcp.rs:339 | ACTIVE — registers per-agent allowlists, checks egress, logs to audit trail |
| WebConduct engine in kernel/src/web_conduct.rs:80 | ACTIVE — blocked_domains list with add/remove/check API |
| Browser agent egress | AgentBrowser.tsx surfaces deny_reason from egress policy |

**No new crates with unrestricted egress detected since last audit.**

---

## 3. Tauri Command Capability Coverage (Bug P Regression)

The three commands that previously lost coverage:

| Command | File:Line | Coverage Status |
|---------|-----------|----------------|
| `get_agent_permissions` | app/src-tauri/src/lib.rs:2606 | PRESENT — delegates to `super::get_agent_permissions` |
| `update_agent_permission` | app/src-tauri/src/lib.rs:2614 | PRESENT — delegates to `super::update_agent_permission` |
| `get_firewall_status` | app/src-tauri/src/lib.rs:2739 | PRESENT — registered and callable |

**Bug P regression: NO REGRESSION detected.**

---

## 4. PQC Phase 1 Abstraction Boundary

CryptoIdentity in `crates/nexus-crypto/` is the canonical signature abstraction.

### Direct Ed25519 bypasses outside nexus-crypto

| File | Status |
|------|--------|
| crates/nexus-ui-repair/src/governance/identity.rs:3 | **BYPASS** — `use ed25519_dalek::SigningKey` directly |

**Phase 1b remaining files: 1** (`nexus-ui-repair/governance/identity.rs`)

### Ed25519 references via CryptoIdentity (correct path)

- `crates/nexus-token-economy/src/gating.rs:74` — via `SignatureAlgorithm::Ed25519`
- `crates/nexus-token-economy/src/wallet.rs:216` — via `SignatureAlgorithm::Ed25519`
- `crates/nexus-governance-oracle/src/sealed_token.rs:106` — via `SignatureAlgorithm::Ed25519`
- `crates/nexus-governance-oracle/src/oracle.rs:88,188` — via `SignatureAlgorithm::Ed25519`

---

## 5. Fuel Metering

Fuel metering active on agent execution paths:

| Path | Evidence |
|------|---------|
| Swarm coordinator | coordinator.rs:88 accepts Budget; line 199 checks consumed percentage |
| Swarm adapters | mod.rs:148 locks budget, line 152 emits budget update |
| Kernel speculative execution | speculative.rs:106-421 — fuel cost calculated per operation type |
| Agent manifest governance | manifest.toml fuel_budget field (e.g., conductor: 50000) |
| Governance evolution | evolution.rs tests child_cannot_exceed_parent_budget |

---

## 6. Leaked Secret Scan

gitleaks not installed. Manual scan performed.

| Pattern | Findings |
|---------|----------|
| `AKIA` (AWS keys) | 0 |
| `sk-` (API keys) | 2 — test placeholders in benchmarks (`sk-abcdefghijklmnopqrstuvwxyz`) |
| `ghp_` / `glpat-` / `xoxb-` | 0 each |
| `Bearer` in source | 10 — all constructing auth headers from variables, no hardcoded tokens |

**No leaked secrets detected.**

---

## 7. cargo-deny Advisory Status

| Advisory | Status | Justification |
|----------|--------|---------------|
| RUSTSEC-2026-0044 | Ignored | aws-lc-sys X.509 — not used by Nexus |
| RUSTSEC-2026-0048 | Ignored | aws-lc-sys CRL — not used by Nexus |
| RUSTSEC-2023-0071 | Ignored | RSA Marvin attack — Nexus uses Ed25519 |
| RUSTSEC-2026-0067/0068 | Ignored | tar — trusted package extraction only |
| RUSTSEC-2024-0411/0412/0413 | Ignored | GTK3 — Tauri transitive dep |
| RUSTSEC-2026-0114 | **REMOVED** | Wasmtime — resolved (AK-13 closure) |

---

## P1 Findings

None from direct investigation. (Note: linter observation 4898 flags broader PQC bypass — see P2-T01.)

## P2 Findings

| # | Finding | File:Line |
|---|---------|-----------|
| P2-T01 | PQC Phase 1b: 1 file bypasses CryptoIdentity with direct ed25519_dalek import | crates/nexus-ui-repair/src/governance/identity.rs:3 |
| P2-T02 | gitleaks not installed — automated secret scan could not run | (system config) |
| P2-T03 | OWASP defense tests filtered to 0 via `cargo test -p nexus-kernel owasp` — defenses exist but may lack dedicated test functions | kernel/src/owasp_defenses.rs |

## P3 Findings

| # | Finding | File:Line |
|---|---------|-----------|
| P3-T01 | 2 test placeholder API keys in benchmark code | benchmarks/benches/kernel_bench.rs:121, phase67_bench.rs:204 |
