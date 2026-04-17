# Nexus OS — The World's First Governed AI Agent Operating System

## Mission

Nexus OS is not a chatbot. It is not a coding assistant. It is an OPERATING SYSTEM for autonomous AI agents that work 24/7, make decisions, take actions on the user's computer, interact with the internet, manage money, and generate wealth — all while being governed, audited, and safe.

The user says: "Here is $1000 and full access to my computer. Generate me $5000 this month." Nexus OS makes it happen — governed, audited, every decision traceable.

---

## Engineering Discipline (Read This First, Every Session)

Nexus OS is infrastructure for the future of agentic AI. It is not a startup rushing to launch. Nothing ships until it is 10/10 complete. Work like a principal engineer responsible for the long-term quality, reliability, and competitive advantage of the system.

### Before Touching Any Code

1. **Understand the repository.** Read the relevant crates, modules, and tests before proposing changes. Do not assume structure — verify it.
2. **Inspect the architecture.** Trace the data flow, ownership boundaries, governance gates, and audit hooks that the change will touch.
3. **Identify root causes, not symptoms.** A failing test or a UI glitch is a signal, not the bug. Find what is actually wrong.
4. **Produce a repair plan.** Numbered, scoped, with explicit files, functions, and invariants. Include verification steps (which tests, which manual checks, which logs).
5. **Wait for approval before writing code.** The plan is the contract. No code is written until Suresh approves.
6. **Prefer minimal, high-leverage fixes** over broad rewrites.
7. **Preserve existing working behavior** unless a change is necessary and justified.

### Forbidden Behaviors

- Shallow edits that paper over a deeper bug.
- Speculative changes ("this might fix it") without a diagnosis.
- Partial fixes that leave the invariant broken in another path.
- Refactors smuggled into bug fixes.
- Touching files outside the scoped plan without surfacing it first.
- Skipping tests because "the change is small."
- Assumptions about code that has not been read in this session.

### Required Behaviors

- Cite the exact file paths and line ranges that justify the diagnosis.
- State the invariant being preserved or restored.
- Name the verification: which `cargo test -p <crate>`, which UI smoke, which log to grep.
- If the root cause spans multiple crates, say so explicitly and scope the plan accordingly.
- If diagnosis is incomplete, say so and propose the next investigation step instead of guessing.

### After Implementing

State exactly what changed, why it changed, how it was validated (which tests ran, which logs were checked, which manual smoke was performed), and what risks remain.

---

## Agent Philosophy: Brutal Honesty, Maximum Autonomy, Full Governance

Nexus OS agents are NOT polite assistants. They are autonomous workers that take REAL actions to generate REAL results. They are:

- **Brutally honest** — if a strategy is losing money, the agent says so and pivots immediately. No sugar-coating.
- **Aggressive** — agents pursue opportunities actively. They don't wait to be told. They find, evaluate, and execute.
- **Fearless** — agents try bold strategies. Failure is data. They learn and evolve via Darwin Core.
- **Accountable** — every action is hash-chained in the audit trail. Every dollar tracked. Every decision explainable.

Agents CAN go rogue. That is why the security exists:

| Threat | Protection |
|--------|-----------|
| Agent spends too much | Fuel metering — hard budget cap per agent |
| Agent makes bad trade | HITL gate — financial actions above $X need approval |
| Agent accesses wrong data | Capability-based access control per agent |
| Agent produces harmful output | Output firewall + PII redaction |
| Agent lies about results | Hash-chained audit trail — tamper-proof |
| Agent tries to escape sandbox | WASM sandbox — no system access outside boundary |
| Agent creates malicious sub-agent | Genesis Protocol requires governance approval |
| Agent colludes with other agents | A2A protocol logged, adversarial arena validates |

The security is not there to LIMIT agents. It is there to LET THEM RUN FREE while keeping humans safe. Maximum autonomy inside governed boundaries. That is the Nexus OS philosophy.

An ungoverned agent is a liability. A governed agent is an employee that works 24/7, never sleeps, never complains, and generates wealth.

---

## Why Nexus OS Wins Against OpenClaw

OpenClaw proved 109K people want autonomous agents. But:

- CVE-2026-25253: 1-click RCE, 40K+ exposed instances
- 800+ malicious skills (20% of ClawHub) distributing malware
- 512 vulnerabilities in security audit, 8 critical
- Cisco: "absolute security nightmare"
- China banned it from government systems
- Kaspersky: "utterly reckless"

Nexus OS is OpenClaw done RIGHT:

- Same autonomy, same capabilities, same 24/7 operation
- WASM sandboxing, hash-chained audit, fuel metering
- HITL safety gates on financial actions
- Ed25519 cryptographic agent identity
- No plaintext credentials — encrypted vault
- Adversarial testing (Darwin Core) on every skill
- Local intelligence — no cloud API keys to steal
- EU AI Act compliant

The pitch: "Would you give your bank password to software with 512 known vulnerabilities? Or to the only agent OS with tamper-proof audit trails and cryptographic identity?"

---

## Workflow Rules (Hard Constraints)

### Planning & Execution

- Before writing any code, output a numbered implementation plan and wait for approval.
- The plan must name files, functions, invariants, and verification steps.
- No speculative edits. No "while I'm here" refactors.
- If a plan grows mid-execution, stop and re-surface the new scope.

### Build & Test Gates

- **Never** use `--all-features` (Candle ML OOM crash on 62GB RAM).
- **Never** run `cargo test --workspace` inside Claude Code. Workspace runs happen in the local terminal only.
- After every prompt: `cargo fmt && cargo clippy && cargo test -p <modified_crate>` — modified crates only.
- Never resume an interrupted Claude Code session. Start fresh.

### Prompt Delivery

- Plain text prompts only into Claude Code — no markdown.
- Prompts longer than 200 chars are delivered as downloadable `.txt` files in `/mnt/user-data/outputs/`.
- Local terminal is reserved for: dev server, UI smoke tests, git push, manual verification.

### Session Hygiene

- No assumptions about time of day from session length. Suresh works long sessions starting at any hour.
- Breaks are framed as breaks (eat real food, step away) — never as "end of day."
- Phase timing and "come back later" calls are Suresh's, not Claude's.

### Shipping Discipline

- Nothing ships until it is 10/10 complete.
- No pressure to launch early. No cutting corners.
- When Nexus OS is ready, its own agents will handle the launch.

---

## Truthfulness Rule

Never invent repository facts, test results, benchmarks, or file contents. Never claim something is fixed unless you can explain exactly what changed and how it was verified. When uncertain, say what you know, what you infer, and what still needs inspection.

## Opus 4.7 Execution Policy

This section governs how Claude Code (Opus 4.7) executes work in this repo.
It overrides Opus 4.7's defaults where our workflow requires different
behavior. Do not remove without explicit instruction from Suresh.

### Subagent fan-out

Opus 4.7 spawns fewer subagents by default. For this repo, spawn parallel
subagents in the SAME turn when:
- Fanning out across independent GT tickets in docs/qa/*.md
- Reading or probing multiple crates (e.g., nexus-kernel + nexus-protocols + nexus-ui-repair)
- Running cargo fmt / clippy / test on >1 modified crate
- Inspecting >1 independent file for a review or audit task

Do NOT spawn subagents for:
- Single-function edits you can complete directly in one response
- Sequential work where step N depends on step N-1's output
- Any edit to the governance kernel, nexus-crypto, or files listed under
  "Hard Invariants" elsewhere in this document

### Tool-use frequency

Opus 4.7 calls tools less often and reasons more. For this repo, explicitly
prefer tool use when:
- Diagnosing CI failures — always read the failing test file AND the
  implementation under test before hypothesizing
- nexus-ui-repair scout work — aggressively read AT-SPI output, UI files,
  and related crate sources
- QA remediation against docs/qa/*_ground_truth_*.md — always re-read the
  ground truth file at the start of the task

Prefer reasoning over tool use when:
- The fix is a one-line clippy or fmt correction
- The task is a pure refactor with no behavioral change

### Adaptive thinking

Opus 4.7 uses adaptive thinking — no fixed budgets. Steer explicitly:
- For complex design, architectural, or cross-crate tasks: "Think carefully
  and step-by-step; this is harder than it looks."
- For mechanical fixes (fmt, clippy, rename, single-test failures):
  "Respond directly; prioritize speed over depth."

Never set CLAUDE_CODE_DISABLE_ADAPTIVE_THINKING=1 in any settings file —
it defeats Opus 4.7's core reasoning mechanism.

### Response length

Opus 4.7 calibrates length to task complexity (less verbose than 4.6).
When a specific length or format is required, state it positively:
- "Output only the unified diff, no prose."
- "Output the exact failing test names, one per line."
- "Respond with a numbered list of file paths to modify."
Avoid negative instructions ("don't be verbose") — they work worse than
positive examples of the desired output.

### Session hygiene (unchanged from existing rules)

- Never resume an interrupted session.
- Never use cargo --all-features (Candle OOM on 62GB RAM).
- Never run cargo test --workspace inside Claude Code.
- After every prompt: cargo fmt && cargo clippy && cargo test -p <crate>
  on MODIFIED crates only.

### Audit and scratch artifact location

Never write audit reports, codebase analyses, deep-read documents,
ground-truth drafts, or any file intended for future reference to /tmp.
/tmp is cleared on reboot; the artifact is lost.

Write persistent artifacts to these paths, relative to repo root:

- docs/audits/   — codebase audits, deep reads, architecture reviews,
                   security or governance audits
- docs/qa/       — ground truth docs and GT ticket backlogs
                   (e.g., chat_page_ground_truth_v1.md, group_d_backlog.md)
- docs/adr/      — architecture decision records
- /tmp/          — ONLY for truly ephemeral files (test output parsed
                   and discarded in the same session, intermediate diffs,
                   throwaway scratch)

Filename convention for audits: <YYYY-MM-DD>_<short-kebab-topic>.md
Examples:
  docs/audits/2026-04-17_nexus_os_deep_audit.md
  docs/audits/2026-04-20_nexus-ui-repair_scout_review.md
  docs/audits/2026-05-01_governance_kernel_security_audit.md

When asked to produce an audit or deep read, always:
1. Write it directly to docs/audits/ with the naming convention above.
2. Include the full relative path in the session recap.
3. Do NOT commit the audit — Suresh reviews and commits manually.

This rule also applies when Suresh asks for a "ground truth" doc
(write to docs/qa/) or an "ADR" / "design decision" (write to docs/adr/).