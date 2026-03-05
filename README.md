[![CI](https://github.com/nexai-lang/nexus-os/actions/workflows/ci.yml/badge.svg)](https://github.com/nexai-lang/nexus-os/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-3.0.0-blue.svg)](CHANGELOG.md)

# NexusOS
Governed deterministic agent OS with audit + replay.

> Don't trust. Verify.

## Status
| Area | Summary |
| --- | --- |
| What works today | Hash-chained audit trail and governed runtime checks in the Rust kernel |
| What works today | Human-in-the-loop approval tiers and queue-backed consent flow |
| What works today | Redaction-first LLM gateway with fuel/budget accounting hooks |
| Experimental | Hardware security backend abstractions (TPM / enclave / TEE stubs) |
| Experimental | Multi-provider LLM routing/fallback paths still being hardened |
| Experimental | Distributed interfaces are local-only scaffolding (no networking/consensus yet) |
| Planned | Cross-node replication and quorum-backed distributed execution |
| Planned | Production installer/signing pipeline for broad end-user distribution |
| Planned | Published benchmark suite and replay evidence bundles for every release |

## Problem and Thesis
Autonomous systems are useful only when their behavior can be constrained, replayed, and audited. NexusOS treats governance as a runtime requirement: deterministic decision paths, explicit approvals for sensitive operations, and tamper-evident evidence trails. The design goal is not maximum autonomy; it is controllable autonomy with reproducible outcomes.

## Research Areas
- Deterministic Replay: same inputs and policy state should reproduce the same governed decisions.
- Capability Security: agents operate inside explicit permission and policy boundaries.
- Human-in-the-loop Approvals: risky transitions require verifiable operator consent.
- Evidence Bundles: audit-chain events and policy decisions remain compliance-ready.

## Architecture
```mermaid
graph TD
  K[Kernel (Rust)] --> P[Policy/Governance Layer]
  P --> A[Audit Chain + Replay]
  K --> G[Governed LLM Gateway]
  K --> S[Supervisor + Safety Controls]
  D[Desktop Shell (Tauri + TypeScript)] --> K
  V[Voice Module (local)] --> K
```

## Quickstart
### Prerequisites
- Rust stable toolchain
- Node.js 20+ and npm
- Platform dependencies required by Tauri (see workflow files for Linux packages)

### Local build and verification
```bash
git clone https://github.com/nexai-lang/nexus-os.git
cd nexus-os

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd app
npm ci
npm run build
```

### Run desktop shell (development)
```bash
cd app
npm run tauri dev
```

### Planned distribution
Installer availability depends on release workflow outputs for tagged versions. If a platform installer is missing for a tag, build from source using the steps above.

## Proof Artifacts
- [THREAT_MODEL.md](THREAT_MODEL.md)
- [PRIVACY_DESIGN.md](PRIVACY_DESIGN.md)
- [COMPLIANCE.md](COMPLIANCE.md)
- [SECURITY.md](SECURITY.md)
- [CHANGELOG.md](CHANGELOG.md)

## Reproducibility
Use the exact commands below from repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd app && npm ci && npm run build
cd ../voice && python3 -m pytest -v
```

## Screenshots
Screenshots coming soon.

Checklist (expected paths):
- [ ] `docs/screenshots/command-center.png`
- [ ] `docs/screenshots/governance-audit-timeline.png`
- [ ] `docs/screenshots/hitl-approval-queue.png`
- [ ] `docs/screenshots/redaction-and-fuel-dashboard.png`

## Contributing and Security
- [CONTRIBUTING.md](CONTRIBUTING.md)
- [SECURITY.md](SECURITY.md)

 ci/fix-windows-release-artifact-discovery
Rust (kernel) + TypeScript/React (desktop app via Tauri) + Python (voice/ML)  
200+ tests | CI/CD on GitHub Actions | 28 milestone versions

## Documentation

- [User Guide](docs/USER_GUIDE.md)
- [Developer Guide](docs/DEVELOPER_GUIDE.md)
- [Threat Model](docs/THREAT_MODEL.md)
- [Privacy Design](PRIVACY_DESIGN.md)
- [Changelog](CHANGELOG.md)

## Built By

Created by Devil — a self-taught developer who built an entire governed agent operating system from scratch.
=======
## Maintainers
Maintained by the NEXAI Lab community.
 main

## License
This project is licensed under the MIT License. See [LICENSE](LICENSE).
