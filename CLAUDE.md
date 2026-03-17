# CLAUDE.md - Nexus OS Development Guide

> Read automatically by Claude Code.

## Project Identity

- Name: Nexus OS
- Version: 9.0.0
- Tagline: Don't trust. Verify.
- Repo: https://gitlab.com/nexaiceo/nexus-os (primary — GitHub suspended)
- License: MIT
- Built by: Suresh Karicheti

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

## Push Commands

git push gitlab main --tags

## Roadmap

See .claude/roadmap/ for implementation plans:
- 01-v3x-hardening.md (DONE - benchmarks, replay evidence, installers, LLM hardening)
- 02-v4-distributed.md (DONE - cross-node replication, quorum, federated audit, marketplace)
- 03-v5-ecosystem.md (DONE - plugin SDK, enterprise RBAC/SSO, cloud scaffolding)
- 04-v6-intelligence.md (DONE - multi-agent collaboration, delegation, adaptive governance)
- 05-v5-production-ready.md (DONE - WASM sandbox, real networking, CLI, desktop UI, docs, E2E tests)
- 06-v6-next-era.md (DONE - real Wasm sandboxing, speculative execution, local SLM, distributed audit, permission dashboard)
- 07-v7-complete-os.md (IN PROGRESS - 15 built-in apps: code editor, design studio, terminal, file manager, database manager, API client, notes, email, project manager, media studio, system monitor, marketplace expansion, chat hub, deployment pipeline, learning center)

## Phase 7 Quick Reference

Phase 7 turns Nexus OS into a complete operating system. 15 built-in applications, each governed:

| # | Feature | File | Status |
|---|---------|------|--------|
| 7.1 | Code Editor (Monaco + agent coding) | app/src/pages/CodeEditor.tsx | IN PROGRESS |
| 7.2 | Design Studio (AI-powered Figma) | app/src/pages/DesignStudio.tsx | TODO |
| 7.3 | Terminal (governed shell) | app/src/pages/Terminal.tsx | TODO |
| 7.4 | File Manager (governed Finder) | app/src/pages/FileManager.tsx | TODO |
| 7.5 | Database Manager (visual SQL) | app/src/pages/DatabaseManager.tsx | TODO |
| 7.6 | API Client (governed Postman) | app/src/pages/ApiClient.tsx | TODO |
| 7.7 | Notes App (private Notion) | app/src/pages/NotesApp.tsx | TODO |
| 7.8 | Email Client (governed email) | app/src/pages/EmailClient.tsx | TODO |
| 7.9 | Project Manager (AI Jira) | app/src/pages/ProjectManager.tsx | TODO |
| 7.10 | Media Studio (image tools) | app/src/pages/MediaStudio.tsx | TODO |
| 7.11 | System Monitor (deep stats) | app/src/pages/SystemMonitor.tsx | TODO |
| 7.12 | App Store (expanded marketplace) | app/src/pages/Marketplace.tsx | EXPAND |
| 7.13 | AI Chat Hub (multi-model) | app/src/pages/Chat.tsx | EXPAND |
| 7.14 | Deployment Pipeline | app/src/pages/DeployPipeline.tsx | TODO |
| 7.15 | Learning Center | app/src/pages/LearningCenter.tsx | TODO |

## Workflow Rules

### Plan First
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan - don't keep pushing
- Write plans to tasks/todo.md with checkable items before implementing
- Verify plan before starting implementation

### Self-Improvement Loop
- After ANY correction from the user: update tasks/lessons.md with the pattern
- Write rules that prevent the same mistake from recurring
- Review lessons at session start for the relevant project

### Verification Before Done
- Never mark a task complete without proving it works
- Run tests, check logs, demonstrate correctness
- Ask yourself: Would a staff engineer approve this?
- Diff behavior between main and your changes when relevant

### Subagent Strategy
- Use subagents for research, exploration, and parallel analysis
- Keep the main context window clean and focused
- One task per subagent for focused execution

### Autonomous Bug Fixing
- When given a bug report: just fix it, don't ask for hand-holding
- Point at logs, errors, failing tests, then resolve them
- Go fix failing CI tests without being told how

### Core Principles
- Simplicity First: make every change as simple as possible, impact minimal code
- No Laziness: find root causes, no temporary fixes, senior developer standards
- Minimal Impact: changes should only touch what's necessary, avoid introducing bugs
