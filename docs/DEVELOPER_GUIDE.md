# NEXUS OS Developer Guide

## Architecture Overview

NEXUS OS is a governed agent operating system composed of Rust crates with strict boundaries:

1. `kernel/`: lifecycle, supervisor, manifest validation, audit, privacy, orchestration.
2. `connectors/`: external capability adapters (LLM, web, social, messaging, control).
3. `workflows/` and `research/`: higher-order agent execution and synthesis pipelines.
4. `content/`, `analytics/`, `adaptation/`: creation, measurement, continuous improvement.
5. `factory/` and `marketplace/`: NL-to-agent generation and signed package distribution.
6. `self-update/`: TUF/in-toto verified update pipeline.
7. `app/` and `cli/`: desktop and command interfaces.
8. `agents/`: runnable product agents (for example `agents/social-poster`).

Core enforcement patterns:
- Explicit capabilities
- Fuel budgeting
- Audit chain integrity
- Approval gates for authority-sensitive actions

## Creating Custom Connectors

### 1. Pick connector domain

Use `connectors/core` patterns for:
- health checks
- idempotency
- rate limiting
- secret handling

### 2. Define governed interface

A connector should:
1. Check required capability before action.
2. Consume fuel proportionally.
3. Enforce provider rate limits.
4. Emit audit events for every meaningful action.

### 3. Add tests

Required test classes:
- request-format tests
- capability-denied tests
- rate-limit tests
- timeout/error-path tests

### 4. Wire into higher layers

Expose connector through workflow/agent pipelines rather than ad hoc command calls.

## Creating Custom Agents

Recommended pattern:
1. Create new crate under `agents/<agent-name>/`.
2. Add manifest with least-privilege capabilities.
3. Build pipeline with explicit stages (research, generate, review, publish, log).
4. Implement dry-run path that executes full logic but avoids external mutation.
5. Add integration test proving dry-run behavior and audit coverage.

Reference implementation:
- `agents/social-poster/`

## Writing Marketplace Packages

Marketplace package requirements:
1. Package metadata and signed manifest.
2. Capability declaration.
3. Provenance metadata (in-toto attestations).
4. Scanner/lint pass for policy violations.

Validation model:
- signature verification
- trust policy checks
- install-time capability risk evaluation

## API Documentation Workflow

Generate API docs locally:

1. `cargo doc --no-deps --document-private-items`
2. Open `target/doc/index.html`

Documentation quality standard:
- Public API surface must include meaningful doc comments.
- Describe capability requirements, failure modes, and security assumptions.

## Contributing Guide

### Development setup

1. Install Rust stable.
2. Clone repository.
3. Run checks:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace`

### Contribution rules

1. Keep changes scoped.
2. Add tests for new behavior.
3. Preserve governance invariants:
   - no hidden capability expansion
   - no bypass of audit/event logging
   - no removal of approval boundaries without replacement controls
4. Update docs and changelog with shipped behavior.

### Pull request checklist

1. CI green on Linux/macOS/Windows jobs.
2. Security/audit workflow passes.
3. Docs updated for user-facing features.
4. No secrets in code, tests, or logs.

## Suggested Extension Order

For new contributors, this sequence minimizes risk:
1. Add a mock-only connector with governance tests.
2. Add real provider integration behind explicit opt-in gates.
3. Add a dry-run agent using the connector.
4. Add desktop/CLI surfaces last.
