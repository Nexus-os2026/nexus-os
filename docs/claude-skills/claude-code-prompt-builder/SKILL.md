---
name: claude-code-prompt-builder
description: Construct Claude Code prompts for Nexus OS. Use when the user asks for a Claude Code prompt, CC prompt, paste-ready prompt, or asks to draft a prompt for Claude Code execution.
---

# Claude Code Prompt Builder

Produces paste-ready Claude Code prompts that conform to the Nexus OS workflow. The prompt body MUST be plain text with no markdown — Claude Code parses plain text only.

## Output contract

The response MUST be exactly three message-parts in this order, with nothing else:

1. One line: `Paste into <new|same> session.`
2. One fenced code block containing the plain-text prompt for Claude Code.
3. One line: `Paste the report back.`

No preamble, no postamble, no commentary outside these three parts.

## Plain-text prompt structure

Inside the fenced block, the prompt uses bare uppercase section labels. No `#`, no `##`, no bold, no decorative bullets. Numbered lists are fine (they are plain text). Sections in this order:

SESSION SETUP — four sub-items, each answered explicitly:
- New session vs continue: state which.
- /compact with focus instructions: state whether to run and on what focus, or "skip".
- /rewind vs correct: state which approach if a prior step needs reversal.
- Subagents: state appropriate or not appropriate and why.

OBJECTIVE — one paragraph, what done looks like.

CONTEXT — crate(s) in scope, files to touch, files to NOT touch, related bug IDs.

CONSTRAINTS — hard rules that apply (see below).

STEPS — numbered, ordered actions for Claude Code to execute.

DELIVERABLES — exact files to produce/modify and report contents.

REPORT FORMAT — how Claude Code should structure the report back.

VERIFICATION — scoped commands for modified crates/packages only.

## Hard rules to embed in every prompt

These MUST appear in the CONSTRAINTS section, verbatim or as direct enforcement:

- NEVER use --all-features (Candle ML OOM on 62 GB).
- NEVER resume an interrupted Claude Code session — start fresh.
- cargo fmt, cargo clippy, cargo test run on modified crates only.
- Claude CLI is blocked for autonomous swarm nodes (interactive-only).
- Commit before running ./scripts/ci-local.sh (CI-LOCAL-01 rule).
- Audit artifacts to docs/audits/. Scratch/QA to docs/qa/. ADRs to docs/adr/.
- After CI-green, push to origin (mirrors to GitHub via configured pushurl).
- No production code in chat output unless explicitly requested.

## Verification commands

Every backend prompt MUST end with this line in the VERIFICATION section:

cargo fmt -p <crate> && cargo clippy -p <crate> -- -D warnings && cargo test -p <crate>

Frontend prompts MUST end with scoped pnpm lint and pnpm test on touched packages only.

NEVER include cargo test --workspace.

## Preflight / investigation variant

If the user asks for a "preflight prompt" or "investigation prompt", change the REPORT FORMAT section to require:
- The Claude Code report MUST be ONE single fenced code block.
- No nested fences inside that block.
- Surface blockers before implementation begins.
- Identify root causes, not symptoms.
- Propose remediation only after diagnosis.

## Delivery threshold

If the prompt body is roughly 2000 characters or longer, deliver as a downloadable .txt file in /mnt/user-data/outputs/ instead of inline. Shorter prompts go inline in the fenced code block.

## Example

User asks: "Draft a Claude Code prompt to add the DAG viewer to the Director console using @xyflow/react."

Response:

Paste into new session.

````
SESSION SETUP
Session: new.
/compact: skip (new session).
/rewind vs correct: correct in place; no prior state to rewind.
Subagents: not appropriate; single-crate frontend task.

OBJECTIVE
Implement the DAG viewer component in the Director console using @xyflow/react, wired to the swarm event bus and Zustand-equivalent store from Phase 2.

CONTEXT
In scope: frontend Agents page, Director console section.
Out of scope: nexus-swarm crate internals, GovernanceOracle.
Related: Phase 3a roadmap item; depends on Phase 2 store.

CONSTRAINTS
NEVER use --all-features.
NEVER resume an interrupted session.
cargo fmt / clippy / test on modified crates only.
Claude CLI blocked for autonomous swarm nodes.
Commit before running ./scripts/ci-local.sh.
Scratch artifacts to docs/qa/. ADR if architectural to docs/adr/.

STEPS
1. Add @xyflow/react to the frontend package.json. Pin version.
2. Create DagViewer.tsx under the Director console directory.
3. Subscribe to swarm events from the Phase 2 store.
4. Render nodes from plan DAG; edges from dependencies.
5. Add Vitest coverage for node/edge rendering and store subscription.
6. Run pnpm lint and pnpm test on the touched frontend package only.
7. Commit. Run ./scripts/ci-local.sh. If green, push.

DELIVERABLES
frontend/.../DagViewer.tsx
frontend/.../DagViewer.test.tsx
package.json updated
Report: files changed, test count delta, ci-local job results, any deviations.

REPORT FORMAT
Standard report. List files touched, test counts, CI results, blockers.

VERIFICATION
pnpm lint and pnpm test on the touched frontend package(s) only.
````

Paste the report back.
