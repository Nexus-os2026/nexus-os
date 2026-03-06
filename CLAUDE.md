# CLAUDE.md - Nexus OS Development Guide

> Read automatically by Claude Code.

## Project Identity

- Name: Nexus OS
- Version: 3.0.0
- Tagline: Don't trust. Verify.
- Repo: https://github.com/nexai-lang/nexus-os
- License: MIT

## Architecture Invariants (NEVER VIOLATE)

1. Every agent action goes through kernel capability checks
2. Fuel budget checked before execution, not after
3. Audit trail is append-only with hash-chain integrity
4. PII redaction at LLM gateway boundary
5. HITL approval mandatory for Tier1+ operations
6. unsafe_code = forbid - zero unsafe Rust
7. All tests must pass before merging
8. Agents declare capabilities in TOML manifests

## Autonomy Levels

- L0: Inert
- L1: Suggest (human decides)
- L2: Act-with-approval (human approves)
- L3: Act-then-report (post-action review)
- L4: Autonomous-bounded (anomaly-triggered)
- L5: Full autonomy (kernel override only)

## Rust Conventions

- Edition 2021, no unsafe code
- Public types derive Debug, Clone, Serialize, Deserialize
- Errors use thiserror or custom enums
- UUID v4 for identifiers
- Audit events via AuditTrail::append_event()
- Capability checks before every action
- Fuel checks before every action

## Build Commands

- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --all-features -- -D warnings
- cargo test --workspace --all-features
- cd app && npm ci && npm run build
- cd voice && python3 -m pytest -v

## Roadmap

See .claude/roadmap/ for implementation plans:
- 01-v3x-hardening.md (CURRENT - benchmarks, replay evidence, installers, LLM hardening)
- 02-v4-distributed.md (cross-node replication, quorum, federated audit, marketplace)
- 03-v5-ecosystem.md (plugin SDK, enterprise RBAC/SSO, cloud scaffolding)
- 04-v6-intelligence.md (multi-agent collaboration, delegation, adaptive governance)
