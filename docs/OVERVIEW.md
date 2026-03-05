# NexusOS Lab Overview

NexusOS explores how autonomous software can remain governable under real operational pressure. The project prioritizes deterministic control surfaces, policy-backed approvals, and replayable evidence over unconstrained autonomy.

## Scope
- Governed agent runtime with explicit policy and capability checks
- Hash-chained audit trail and reproducibility-oriented execution model
- Human-in-the-loop approvals for sensitive operations
- Desktop control surface (Tauri) over Rust kernel orchestration

## Non-Goals
- Unbounded autonomous decision-making without operator control
- Hidden background actions that bypass policy/audit surfaces
- Undocumented provider calls or opaque governance behavior

## Roadmap Snapshot
- Stabilize multiprovider LLM governance and fallback approvals
- Expand replay tooling and evidence bundle export
- Move distributed interfaces from local-only scaffolding to validated protocols
- Improve release quality for cross-platform installer delivery
