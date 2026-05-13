---
name: claude-code-prompt-builder
description: Construct Claude Code prompts for Nexus OS. Use when the user asks for a Claude Code prompt, CC prompt, paste-ready prompt, or asks to draft a prompt for Claude Code execution.
---

# Claude Code Prompt Builder

Produces paste-ready Claude Code prompts that conform to the Nexus OS workflow.

## Output contract

The response MUST be exactly three message-parts in this order, with nothing else:

1. One line: `Paste into <new|same> session.`
2. One fenced code block. This block is the ENTIRE prompt for Claude Code. No other fenced blocks may appear anywhere else in the message.
3. One line: `Paste the report back.`

No preamble, no postamble, no commentary outside these three parts.

## Prompt body template

Every prompt inside the fenced block must contain these sections, in order:

1. **Session Setup** — four sub-items, each answered explicitly:
   - New session vs continue: state which.
   - `/compact` with focus instructions: state whether to run and on what focus, or "skip".
   - `/rewind` vs correct: state which approach if a prior step needs reversal.
   - Subagents: state appropriate / not appropriate and why.
2. **Objective** — one paragraph, what done looks like.
3. **Context** — crate(s) in scope, files to touch, files to NOT touch, related bug IDs.
4. **Constraints** — hard rules that apply (see below).
5. **Steps** — numbered, ordered actions for Claude Code to execute.
6. **Deliverables** — exact files to produce/modify and report contents.
7. **Report format** — how Claude Code should structure the report back.

## Hard rules to embed in every prompt

These MUST appear in the Constraints section, verbatim or as direct enforcement:

- NEVER use `--all-features` (Candle ML OOM on 62 GB).
- NEVER resume an interrupted Claude Code session — start fresh.
- `cargo fmt`, `cargo clippy`, `cargo test` run on modified crates only.
- Claude CLI is blocked for autonomous swarm nodes (interactive-only).
- Commit before running `./scripts/ci-local.sh` (CI-LOCAL-01 rule).
- Audit artifacts → `docs/audits/`. Scratch/QA → `docs/qa/`. ADRs → `docs/adr/`.
- After CI-green, push to `origin` (mirrors to GitHub via configured pushurl).
- No production code in chat output unless explicitly requested.

## Preflight / investigation variant

If the user asks for a "preflight prompt" or "investigation prompt", change the Report format section to require:

- The Claude Code report MUST be ONE single fenced code block.
- No nested fences inside that block.
- Surface blockers before implementation begins.
- Identify root causes, not symptoms.
- Propose remediation only after diagnosis.

## Example

User asks: "Draft a Claude Code prompt to add the DAG viewer to the Director console using @xyflow/react."

Response:

Paste into new session.

```
## Session Setup
- Session: new.
- /compact: skip (new session).
- /rewind vs correct: correct in place; no prior state to rewind.
- Subagents: not appropriate; single-crate frontend task.

## Objective
Implement the DAG viewer component in the Director console using @xyflow/react, wired to the swarm event bus and Zustand-equivalent store from Phase 2.

## Context
- In scope: frontend Agents page (Director console section).
- Out of scope: nexus-swarm crate internals, GovernanceOracle.
- Related: Phase 3a roadmap item; depends on Phase 2 store.

## Constraints
- NEVER use --all-features.
- NEVER resume an interrupted session.
- cargo fmt / clippy / test on modified crates only.
- Claude CLI blocked for autonomous swarm nodes.
- Commit before running ./scripts/ci-local.sh.
- Scratch artifacts → docs/qa/. ADR if architectural → docs/adr/.

## Steps
1. Add @xyflow/react to the frontend package.json. Pin version.
2. Create DagViewer.tsx under the Director console directory.
3. Subscribe to swarm events from the Phase 2 store.
4. Render nodes from plan DAG; edges from dependencies.
5. Add Vitest coverage for node/edge rendering and store subscription.
6. Run frontend tests on modified files only.
7. Commit. Run ./scripts/ci-local.sh. If green, push.

## Deliverables
- frontend/.../DagViewer.tsx
- frontend/.../DagViewer.test.tsx
- package.json updated
- Report: files changed, test count delta, ci-local job results, any deviations.

## Report format
Standard report. Free-form structure. List files touched, test counts, CI results, blockers.
```

Paste the report back.
