# Contributing to Nexus OS

Thanks for your interest in contributing to Nexus OS! This guide covers everything you need to get started.

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.94+ stable | Backend (kernel, agents, CLI, connectors) |
| Node.js | 22+ | Desktop frontend (React/TypeScript) |
| npm | 10+ | Frontend package management |
| Python | 3.11+ | Voice assistant (optional) |
| Ollama | Latest | Local LLM inference (optional) |

### Platform Dependencies

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev libssl-dev pkg-config
```

**macOS:**
```bash
xcode-select --install
```

**Windows:**
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload
- Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

### Clone and Build

```bash
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os

# Add the WASM target (required for sandbox tests)
rustup target add wasm32-wasip1

# Build everything
cargo build --workspace

# Build the frontend
cd app && npm install && npm run build
```

## Code Style

All code must pass these checks before merge. Run them locally before opening a PR:

```bash
# Format check (zero tolerance for unformatted code)
cargo fmt --all -- --check

# Lint check (zero warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# Full test suite (1,941 tests)
cargo test --workspace

# Frontend build
cd app && npm run build
```

### Rust Conventions

- **Edition 2021**, no unsafe code (`unsafe_code = "forbid"`)
- Public types derive `Debug`, `Clone`, `Serialize`, `Deserialize`
- Errors use `thiserror` or custom enums — no `unwrap()` in library code
- UUID v4 for all identifiers
- Capability checks before every agent action
- Fuel checks before every agent action
- Audit events via `AuditTrail::append_event()`

### TypeScript Conventions

- Strict mode enabled
- No `any` types — use proper interfaces
- Functional React components with hooks

## Architecture for New Contributors

Nexus OS is organized into 33 workspace crates:

```
kernel/          Core governance: capabilities, fuel, audit, autonomy
sdk/             Agent SDK: NexusAgent trait, AgentContext, ManifestBuilder
distributed/     Cross-node: TCP transport, replication, quorum, membership
enterprise/      RBAC, SOC 2 compliance reporting
marketplace/     Agent registry, trust scoring, signed bundles
cli/             24-command CLI
app/             Tauri desktop shell (React + TypeScript frontend)
connectors/      External connectors: LLM, web, social, messaging, control
agents/          9 built-in agents (coder, designer, web-builder, etc.)
```

**Key principle:** All agent actions flow through the kernel. The SDK wraps the kernel for agent-facing use. Agents depend on `nexus-sdk`, never on `nexus-kernel` directly.

**Governance pipeline** (every action follows this path):
```
Agent Request → Capability Check → Fuel Check → Autonomy Gate
    → HITL Approval (if needed) → Execute → Audit Event → Fuel Deduction
```

## Pull Request Process

1. **Branch from `main`** — use descriptive branch names (`fix/audit-hash-validation`, `feat/agent-checkpointing`)
2. **Keep changes focused** — one feature or fix per PR. Large PRs are harder to review.
3. **Include tests** when behavior changes. We have 1,941 tests and the count should only go up.
4. **Update docs** for public-facing changes (API, CLI, configuration)
5. **All CI checks must pass** — cargo fmt, clippy, tests, frontend build
6. **Write clear commit messages** — explain the "why", not just the "what"

### PR Title Format

```
feat: Add agent checkpointing to SDK
fix: Correct fuel deduction for delegated actions
docs: Update deployment guide for multi-node setup
refactor: Simplify audit hash-chain verification
```

## What to Contribute

- Bug fixes (check issues on GitLab)
- New agents (use `agents/coder/` as a template)
- Connector integrations (Slack, Discord, etc.)
- Documentation improvements
- Test coverage improvements
- Performance optimizations

## Code of Conduct

By participating, you agree to collaborate respectfully and constructively. We value clear communication, honest feedback, and inclusive behavior.

## Questions?

Open an issue on [GitLab](https://gitlab.com/nexaiceo/nexus-os/-/issues) or start a discussion. We're happy to help you get oriented.
