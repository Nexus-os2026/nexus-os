# Security Policy

## Reporting Vulnerabilities

**Do NOT open public issues for security vulnerabilities.**

Email: **security@nexus-os.dev** (or open a confidential issue on GitLab)

We will acknowledge receipt within 48 hours and provide an initial assessment within 7 days.

Include: description, reproduction steps, potential impact, and suggested fix if any.

### Scope
- Governance kernel bypass (capability escalation, HITL bypass, fuel metering evasion)
- WASM sandbox escape
- Audit trail tampering or integrity violation
- Agent identity spoofing (DID/Ed25519)
- PII redaction bypass, output firewall bypass
- Unauthorized cross-agent communication
- Privilege escalation across multi-tenancy boundaries
- Encryption weaknesses, authentication/authorization bypass

---

## Security Model — Seven Layers of Defense

```
Layer 7: Output Firewall ─── content filtering, exfiltration prevention
Layer 6: PII Redaction ───── automated detection and scrubbing
Layer 5: HITL Consent Gates ─ human approval for risky operations
Layer 4: Fuel Metering ───── resource consumption limits
Layer 3: Capability ACL ──── explicit permission tokens, no ambient authority
Layer 2: WASM Sandbox ────── wasmtime hardware-grade isolation
Layer 1: Agent Identity ──── DID/Ed25519 cryptographic signatures
Layer 0: Audit Trail ─────── hash-chained, tamper-evident, append-only
```

### Capability-Based Access Control
No agent has ambient authority. Every action requires an explicit, cryptographically signed capability token. Capabilities are scoped, non-escalatable, and require HITL approval for delegation above L3 autonomy.

### WASM Sandboxing (wasmtime)
Memory isolation, filesystem isolation, network isolation, CPU limits, no shared state. Inter-agent communication only through governed channels.

### Agent Identity (DID/Ed25519)
Each agent has a Decentralized Identifier with an Ed25519 keypair. All actions are cryptographically signed. Identity survives evolution — evolved agents inherit lineage from parent DID.

### Hash-Chained Audit Trail
Append-only log where each entry includes the SHA-256 hash of the previous entry. Tampering with any entry invalidates all subsequent hashes. Verifiable by any auditor at any time.

### Encryption

| Data State | Method |
|-----------|--------|
| In transit | TLS 1.3 via Rust-native TLS |
| At rest | AES-256-GCM |
| Agent keys | Ed25519 in OS keyring |
| Backups | AES-256-GCM with user-controlled keys |

### Supply Chain
- All dependencies pinned in `Cargo.lock`
- Rust borrow checker eliminates memory safety vulnerabilities
- No `unsafe` in governance-critical code paths
- CI runs `cargo clippy` and `cargo audit` on every commit

## Supported Versions

| Version | Status |
|---------|--------|
| 9.x.x | ✅ Active |
| 8.x.x | 🔄 Security fixes only |
| < 8.0 | ❌ End of life |
