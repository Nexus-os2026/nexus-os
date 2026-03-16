# Nexus OS — The World's First Governed Autonomous Agent Operating System

45 AI agents that think, act, learn, and evolve — all governed by cryptographic consent.

## What is Nexus OS

Nexus OS is an operating system for autonomous AI agents, built around a governed Rust kernel and a multi-surface interface spanning desktop, voice, and messaging experiences. It is differentiated by making autonomy accountable: every action can be constrained, approved, audited, replayed, and measured through a layered governance model. It is designed for builders, researchers, and operators who want real agent capability without giving up control, safety, or traceability.

## Key Numbers

- 45 pre-built agents
- 7 autonomy levels (L0-L6)
- 35 Rust crates
- 2,500+ tests
- 4 real algorithm engines
- 16 security layers
- 11 persistence tables
- Zero clippy warnings

## Architecture

```text
User
  |
  v
Desktop / Telegram / Voice
  |
  v
Cognitive Loop
  Perceive -> Reason -> Plan -> Act -> Reflect -> Learn
  |
  v
Governed Actuators
  File / Shell / Web / API
  |
  v
Real World

Governance Stack
  Fuel Metering
  HITL Approval
  Audit Chain
  Speculative Execution
  Time Machine
```

## Autonomy Levels

- L0: Fully constrained execution with no independent decision-making.
- L1: Assisted operation with bounded actions and explicit user direction.
- L2: Task-level autonomy within predefined tools and policies.
- L3: Multi-step execution with planning, recovery, and governed delegation.
- L4: Self-improving agents that adapt behavior within hard system constraints.
- L5: Sovereign-class autonomy with elevated governance and approval controls.
- L6: Evolutionary autonomy with continuous reflection, learning, and strategic adaptation under cryptographic consent.

## Featured Agents

- Ascendant: High-autonomy agent for long-horizon execution and self-directed improvement.
- Sovereign: Governance-heavy executive agent for sensitive operations requiring durable oversight.
- Genesis Prime: Foundational orchestrator for bootstrapping complex multi-agent workflows.
- Legion: Parallel coordination agent designed for managing many agents as a unified system.
- Oracle Supreme: Deep analysis and foresight agent for strategic reasoning and scenario evaluation.
- Darwin: Adaptive experimentation agent focused on iteration, selection, and optimization.
- Architect: Systems design agent for structuring products, platforms, and technical plans.
- Strategist: Decision-support agent for roadmap planning, tradeoff analysis, and execution sequencing.
- Warden: Safety and enforcement agent responsible for guarding policy boundaries and risk controls.
- Nexus Prime: Flagship supervisory agent for cross-domain orchestration across the platform.

## Quick Start

### Prerequisites

- Rust
- Node.js
- Ollama

### Local Development

```bash
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os
cargo build --workspace
cd app && npm install && npm run tauri dev
```

## Docker Quick Start

```bash
docker-compose up
```

## Tech Stack

- Rust kernel
- Tauri 2.0 desktop
- React/TypeScript frontend
- Python voice pipeline
- SQLite persistence
- Wasmtime WASM sandbox

## Security

- WASM sandbox
- Ed25519 signing
- SHA-256 audit chain
- Prompt firewall
- Egress governor
- Speculative execution controls
- HITL consent
- EU AI Act compliance

## Credits

Created by Suresh Karicheti. Lead Architect: Claude (Anthropic).

## License

This project is licensed under the MIT License. See [LICENSE](/home/nexus/NEXUS/nexus-os/LICENSE).
