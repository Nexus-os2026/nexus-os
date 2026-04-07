# NEXUS_UI_REPAIR v1.1 — Scout / Repair Split

**Amendment to:** `NEXUS_UI_REPAIR_v1.md`
**Drafted:** April 7, 2026
**Status:** Supersedes v1 §2 (driver loop), §3 (I-2, I-3), §4 (provider routing), §5 (Phase 1 scope), §8 (open questions 1 & 3).
**Author:** Suresh Karicheti + Claude (architecture)

---

## 0. Why this amendment exists

v1 designed `nexus-ui-repair` as a single autonomous loop: enumerate → act → observe → classify → **repair** → log. The repair step ran specialists that edited files via `nx`, with HITL gating only on >3-file or security-path changes.

After a near-miss on the night of April 6–7 (an autonomous agent with full permissions touched files it shouldn't have on a tired-developer session), and after Suresh proposed a cleaner split, v1.1 makes one structural change:

**The driver no longer repairs. It scouts.**

All repairs move to a separate, human-supervised phase using Claude Code interactively on the Max plan — exactly what that subscription is licensed for.

This is not a downgrade. It is a recognition that on a 335K-line Rust + 65K-line TypeScript codebase with 675+ Tauri commands, no current model fixes bugs autonomously at acceptable accuracy without supervision. The wrong fixes the autonomous version would silently make become bugs the human has to find and reverse later. The split eliminates that entire class of failure.

---

## 1. The two phases

### Phase A — Scout (autonomous, read-only)

**Provider:** Codex CLI / GPT-5.4 (free via ChatGPT Plus)
**Permissions:** `nexus-computer-use` for screen + input. **No filesystem write capability.**
**Output:** One markdown bug report per page, written to `~/.nexus/ui-repair/reports/<date>/<page>.md`
**Cost:** $0 default. Up to $1–2 of Anthropic API credit reserved for vision ambiguity escalation.
**Runtime:** Can run unattended overnight across all 87 pages.

The scout walks the UI like a human QA tester. It clicks every button, types into every field, navigates every flow, watches every screen and console, and writes down what's broken. It makes zero changes to source code. It cannot. The capability is not in its ACL.

### Phase B — Repair (you + Claude Code interactive)

**Provider:** Claude Code CLI (Opus 4.6) on the Max plan
**Permissions:** Full, because you are sitting at the keyboard
**Input:** The bug reports from Phase A, one section at a time
**Output:** Real fixes, reviewed by you, committed in logical chunks
**Cost:** Covered by the $200/month Max plan and the $150 Claude.ai credits — both used exactly as licensed.
**Runtime:** Spread over days/weeks at your pace.

You sit down with a bug report, paste a section into Claude Code, work the fix with Opus, review the diff, run the tests, commit. Move to the next bug. This is the same brain-and-body partnership we already use for everything else — just now driven by a structured worklist instead of you remembering bugs.

---

## 2. Updated driver loop (Phase A only)

```
┌─────────────────────────────────┐
│  ENUMERATE  (Ollama)            │
│  scrape DOM, fingerprint        │
│  elements, query Ledger         │
└────────────────┬────────────────┘
                 │
┌────────────────▼────────────────┐
│  PLAN  (Codex CLI)              │
│  pick next element, define      │
│  expected post-condition        │
└────────────────┬────────────────┘
                 │
┌────────────────▼────────────────┐
│  ACT  (nexus-computer-use)      │
│  click / type / navigate        │
└────────────────┬────────────────┘
                 │
┌────────────────▼────────────────┐
│  OBSERVE  (vision_judge)        │
│  did the screen change?         │
│  did logs/console emit errors?  │
└────────────────┬────────────────┘
                 │
┌────────────────▼────────────────┐
│  CLASSIFY                       │
│  PASS / DEAD / ERROR / HANG /   │
│  AMBIGUOUS                      │
└────────────────┬────────────────┘
                 │
┌────────────────▼────────────────┐
│  REPORT  (append to .md file)   │
│  + LOG (Ed25519 signed)         │
│  + ADVANCE                      │
└─────────────────────────────────┘
```

**No REPAIR state. No code_locator. No repair_proposer. No regression_runner.** Those move to Phase B and become you + Opus.

The driver is now a 5-state machine with one terminal action (REPORT). Every transition still hits the kernel invariant checks.

---

## 3. Revised invariants

I-1, I-4, I-5 from v1 stand unchanged. **I-2 and I-3 are simplified because the driver no longer mutates files.**

| # | Invariant | Enforcement |
|---|-----------|-------------|
| **I-1** | Kernel allowlist (unchanged from v1) | Path allowlist; the driver has no fs::write capability anyway, so this is now belt + suspenders |
| **I-2** | **Read-only-by-construction.** The scout has no capability to modify state outside its own report and audit directories. This holds across THREE layers: filesystem, desktop input, and OS process isolation. | See §3.1 below — the enforcement is multi-layer, not single-line. |
| **I-3** | **Every fix is HITL by definition.** The scout reports; the human repairs. There is no autonomous repair path, so no HITL gating logic is needed. | Structural — there is nothing to gate |
| **I-4** | Immutable provider routing (unchanged from v1, simplified) — the autonomous routing table contains only `codex_cli`, `ollama`, and a small Anthropic API allowance for vision ambiguity. `claude_cli` and `claude_ai_credits` are explicitly forbidden in the autonomous table. | Signed config at startup |
| **I-5** | Replayable sessions (unchanged from v1) | Append-only signed log; replay harness in CI. Every specialist call records `(inputs, output)` to the audit log, not just inputs — the output capture is what makes replay byte-identical despite non-deterministic LLM calls in `vision_judge`. |

**I-3 simplification is the biggest win of v1.1.** All the gating logic v1 needed (>3 files, security paths, public APIs, failed regression) disappears, because there are no autonomous edits to gate. The whole HITL escalation surface goes away.

---

## 3.1 I-2 enforcement (load-bearing detail)

> I-2 is the load-bearing safety property of `nexus-ui-repair`. v1.1's original framing — "`fs::write` returns `PermissionDenied` at the syscall layer" — was wrong on two counts. First, Rust does not enforce filesystem ACLs at the syscall layer; any `unsafe` block, new dependency, or `Command::new` call defeats it. Second, and more importantly, the scout's real attack surface is not the filesystem — it is `nexus-computer-use`, which gives the scout keyboard and mouse control over the entire desktop. A read-only Rust filesystem ACL is irrelevant when the agent can type into a terminal window or have its keystrokes redirected by a window-stealing modal.
>
> Real I-2 enforcement runs at three layers, all of which must be present in any environment that runs the scout against a real Nexus OS instance:
>
> **Layer 1 — Filesystem ACL (in-process, defense in depth).** The scout's `Acl` struct (`governance/acl.rs`) holds an allowlist of two roots: `~/.nexus/ui-repair/reports/` and `~/.nexus/ui-repair/sessions/`. Every code path that writes to disk must call `Acl::check_write(path)` before writing, and the call returns `Error::AclDenied` for any path outside the allowlist. This is enforced by code review and by `tests/acl.rs`, which fuzzes the boundary. It is *not* a security boundary against malicious code in the scout — it is a structural guarantee against accidental writes by correct code.
>
> **Layer 2 — Per-app input governance (the real boundary).** `nexus-computer-use` (crate #68) ships per-app governance. The scout's input ACL is configured to whitelist exactly one window: the Nexus OS main window, identified by window class plus PID at session start. Every input event is gated on the currently focused window matching the whitelist. Focus change to any non-whitelisted window pauses the scout, logs a `FocusLoss` event to the audit log, and surfaces a HITL flag. Two consecutive failed refocus attempts trigger an immediate HALT.
>
> **Layer 3 — OS-level isolation (defense against everything else).** The Nexus OS instance under test runs inside a dedicated Xvfb display (or sway headless compositor) that contains only Nexus OS. The scout's keyboard and mouse events route to that display. No other windows exist in that display to receive misdirected input. Phase 1.3 ships with Xvfb. Phase 4 (overnight mode) adds a separate Unix user `nexus-scout` with filesystem permissions only on `~/.nexus/ui-repair/`, providing real syscall-layer enforcement.
>
> **Phase gating:** Phase 1.1 (skeleton, no desktop reach) needs only Layer 1. Phase 1.3 (first `nexus-computer-use` integration) must land Layers 1, 2, and 3 in the same commit that imports `nexus-computer-use`. Phase 4 (overnight mode) adds the separate Unix user. **The commit that first imports `nexus-computer-use` into `nexus-ui-repair` is the same commit that lands per-app governance and Xvfb.** No exceptions.

---

## 4. Provider routing (revised)

| Specialist | Default | Escalation | Why |
|---|---|---|---|
| `enumerator` | Ollama `gemma4:e2b` | — | Pure DOM scrape |
| `vision_judge` | Codex CLI (GPT-5.4) | Anthropic API (Haiku 4.5 only, capped at $2 total) | Catches "is this broken or just slow" cases |
| `classifier` | Codex CLI (GPT-5.4) | — | Deterministic rules + GPT for edge cases |
| `report_writer` | Codex CLI (GPT-5.4) | — | Markdown formatting, no escalation needed |

**Forbidden in the autonomous routing table (I-4):**
- `claude_cli` — consumes Max plan, ToS violation
- `claude_ai_credits` — account ban risk
- `anthropic_api/sonnet` — not needed for scout work
- `anthropic_api/opus` — not needed for scout work

**The $10 Anthropic API credit becomes a backup you may never touch.** Realistic Phase A spend is $0–$2.

---

## 5. Bug report format

### 5.0 Bug report contract

> Every bug entry in a scout report has exactly two sections, separated by an explicit visual wall: **Observed (deterministic)** and **LLM analysis (Codex CLI guess)**. The wall exists because the scout produces both kinds of content, and conflating them in Phase B is the single biggest source of wasted Opus cycles.
>
> **Observed** contains only facts the scout measured directly: element identity, bounds, the action taken, screenshot file paths, vision diff similarity score, console output, IPC traffic, network traffic, DOM mutations, focus changes. Every line in this section is reproducible from the session audit log. Nothing in this section is generated by an LLM; the section is assembled by deterministic code that reads instrumentation output.
>
> **LLM analysis** contains the `report_writer` specialist's interpretation of the observed facts: likely cause, suggested files to check, reasoning, confidence level. Every line in this section is a guess by Codex CLI based on pattern matching against the page route and common bug shapes. The file paths in particular are *not* derived from a real code trace — code tracing is a Phase 2 specialist that doesn't exist yet. The section header includes the literal phrase "verify before trusting" to make this unambiguous in Phase B.
>
> **Phase B usage rule:** when you sit down with a report and Claude Code, paste the **Observed** section into Opus first and ask Opus to propose where the bug might be, *without* showing Opus the LLM analysis section. Then compare Opus's hypothesis to the scout's hypothesis. When they agree, confidence is high. When they disagree, the Observed section is the tiebreaker — it is the only ground truth in the report.
>
> The `report_writer` specialist is forbidden from putting any speculation in the Observed section. A unit test (`tests/report_format.rs`) parses generated reports and asserts that the Observed section contains only field names from a fixed allowlist; any other field name fails the test.

One markdown file per page, written to `~/.nexus/ui-repair/reports/<YYYY-MM-DD>/<page-slug>.md`. Append-only during the session, finalized at the end with a summary.

```markdown
# Builder / Teams page — QA scout report

**Session:** 2026-04-08T14:23:00Z
**Driver:** nexus-ui-repair v0.1.0 (scout mode)
**Provider:** codex_cli (gpt-5.4)
**Page route:** /builder/teams
**Session ID:** ses_8a3f...
**Audit log:** ~/.nexus/ui-repair/sessions/2026-04-08-teams/audit.jsonl

## Summary

- Elements enumerated: 23
- Verified working: 17
- Broken: 5
- Ambiguous (needs human review): 1

## Broken interactions

### BUG-001: "Edit team" button does nothing

#### Observed (deterministic — trust this)
- **Element:** `button#edit-team-btn`
- **Bounds:** x=240 y=180 w=80 h=32
- **Action:** click at (280, 196)
- **Screenshot before:** `bug-001-before.png`
- **Screenshot after:** `bug-001-after.png`
- **Vision diff similarity:** 0.99 (no meaningful change)
- **Console errors during action window:** none
- **Tauri commands emitted on IPC bridge:** none
- **Network requests during action window:** none
- **Focused element after click:** unchanged from before click
- **DOM mutations within 2s:** none

#### LLM analysis (Codex CLI guess — verify before trusting)
- **Likely cause:** onClick handler missing or not wired at the React level
- **Reasoning:** a click on a button with no console error, no Tauri call, no
  DOM mutation, and no focus change typically means the handler is not bound.
  Less likely: handler is bound but is a no-op stub.
- **Suggested files to check (UNVERIFIED, may be wrong):**
  - `app/src/components/builder/TeamsPanel.tsx` — likely missing onClick prop
    on the row action button
  - `app/src-tauri/src/commands/builder.rs` — secondary check, verify
    `update_team` command exists in case the handler IS wired but its target
    is missing
- **Confidence:** low — the file paths are pattern-matched against the page
  route, not derived from a real code trace.

#### Reproduction steps
1. Navigate to `/builder/teams`
2. Click "Edit team" on any team row
3. Observe: nothing happens

### BUG-002: Variant dropdown shows empty list

- **Element:** `select#variant-select`
- ... (same structure)

## Ambiguous interactions (need human review)

### AMB-001: "Generate report" button takes 45+ seconds

- **Element:** `button#generate-report-btn`
- **Action:** click
- **Observed:** spinner appears, no error, no result after 45s timeout
- **Could be:** legitimate slow operation OR hang
- **Recommendation:** human verifies whether 45s is expected for this action
```

This format is **immediately useful in Phase B**. You paste a `BUG-XXX` section into Claude Code, Opus has everything it needs to propose a fix, you review the diff, you commit. No context-rebuilding, no "wait what was the bug again."

---

## 6. Phase 1 acceptance criteria (revised)

Phase 1 is still "drive the Builder Teams page end-to-end," but the success criteria change because the goal is *finding* bugs, not fixing them.

1. **All interactive elements on Teams page enumerated** — driver reports a count and a per-element fingerprint table.
2. **Bug report markdown file produced** at the expected path with the expected structure.
3. **Suresh reads the report and confirms:**
   - Every real bug he knows about on Teams (variants not showing, edit not wiring, buttons not firing) is in the report
   - No more than 1 false positive (scout reports something as broken that actually works)
   - No more than 1 false negative (scout misses a bug Suresh knows is real)
4. **Zero filesystem writes outside `~/.nexus/ui-repair/`** — verified by audit log inspection. If the scout wrote anywhere else, the I-2 enforcement is broken and we stop.
5. **Cost ceiling: ≤$0.50** of Anthropic API credit, almost entirely on vision_judge ambiguity escalation. Realistic target: $0.00–$0.20.
6. **Full session replay** — `cargo test -p nexus-ui-repair --test replay` reconstructs every state transition byte-identically.
7. **Phase B repair pass** — Suresh sits down with the Teams report and Claude Code (Opus on Max plan), works through every BUG-XXX, fixes them, runs tests, commits. **The fixes ship as a separate v10.10.0-rc commit series.** The scout is the input; the human + Opus is the worker.

If criteria 3 fails (too many false positives or negatives), tune the vision_judge and classifier before Phase 2. If criteria 4 fails, stop everything — the I-2 enforcement is the load-bearing safety property of this entire design.

---

## 6.5 Destructive Action Policy

> Nexus OS contains buttons whose semantics are "destroy state the user cares about": Settings → Reset, Governance → Revoke Identity, Memory → Wipe Caches, Builder → Delete Project, and others. The scout's job description — "click every button like a human" — would, without policy, eventually click one of these. The first overnight run at Phase 4 scale will find one. This section defines the policy that prevents that.
>
> The policy has three layers, all of which must be present before Phase 1.3 ships.
>
> **Layer 1 — Destructive pattern denylist (enumeration time).** The `enumerator` specialist tags every element with a `kind`. The default `kind` taxonomy includes a `Destructive` variant. An element is tagged `Destructive` if its accessible label, accessible description, button text, or aria-label matches any of the following case-insensitive patterns:
>
> ```
> /\b(delete|remove|reset|wipe|revoke|destroy|purge|drop|clear all|factory reset|uninstall|forget|erase)\b/i
> ```
>
> Elements tagged `Destructive` are enumerated and recorded in the report (so you know they exist) but are **skipped from the ACT phase** by default. The driver records them as `Skipped: Destructive` in the report's "not exercised" section.
>
> **Layer 2 — Confirmation modal handling (action time).** When the scout's ACT phase produces a modal dialog, the modal is matched against a small set of known patterns: `LoginModal`, `ConfirmationModal`, `ErrorModal`, `InfoModal`. For any `ConfirmationModal`, the scout's only permitted action is to click the cancel/dismiss control, identified by accessible role plus label match against `/cancel|dismiss|close|no|back/i`. The scout never clicks "Yes", "Confirm", "Delete", "Continue", "OK", or any positive-affirmation control on a confirmation modal it did not explicitly expect. If a confirmation modal has no identifiable cancel control, the scout aborts the current page, writes a `confirmation_no_cancel.png` screenshot to the session directory, and surfaces a HITL flag. Three unrecognized modals in a single session triggers HALT and writes `panic.md` next to the report.
>
> **Layer 3 — Per-page descriptor opt-ins (rare, deliberate).** For the rare cases where you actually want to test a destructive action (e.g., the delete-project flow, against a fixture project), the page descriptor format gains an optional `destructive_opt_ins` field:
>
> ```
> PageDescriptor {
>     route: "/builder/projects",
>     destructive_opt_ins: Some(vec![
>         DestructiveOptIn {
>             element_id: "delete-project-btn",
>             fixture_required: true,
>             fixture_id: "throwaway_project_for_qa",
>         }
>     ]),
>     ...
> }
> ```
>
> An opt-in is honored only if the named fixture is present and is marked as throwaway. Without a fixture, the opt-in is ignored and the element stays skipped. Opt-ins never apply to elements outside Builder (no opt-ins for Settings → Reset, Governance → Revoke Identity, or Memory → Wipe — these are unconditionally skipped, ever).
>
> **Acceptance for Phase 1.3:** the unit test `tests/destructive_policy.rs` exercises:
>
> 1. The pattern denylist matches all of the example destructive labels above.
> 2. A confirmation modal with no cancel control triggers HITL, not a click.
> 3. A confirmation modal with both "Yes" and "Cancel" buttons results in a click on "Cancel", verified by the recorded action log.
> 4. An opt-in without a matching fixture is ignored.
> 5. An opt-in for a path matching `/settings|governance|memory/` is rejected at descriptor-load time.

---

## 7. File layout (revised)

The Phase 1 crate is smaller than v1's because half the specialists are deferred or removed.

```
crates/nexus-ui-repair/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── governance/
│   │   ├── identity.rs       # Ed25519 session identity
│   │   ├── invariants.rs     # I-1 through I-5, checked at transitions
│   │   ├── routing.rs        # signed provider routing table (I-4)
│   │   ├── acl.rs            # I-2 enforcement: report dirs only
│   │   ├── input_sandbox.rs   # I-2 Layer 2: per-app governance config + focus check
│   │   └── audit.rs          # hash-chained session log
│   ├── driver/
│   │   ├── state.rs          # 5-state DriverState enum + transitions
│   │   └── loop.rs           # the scout loop in §2
│   ├── specialists/
│   │   ├── enumerator.rs     # DOM scrape, fingerprinting
│   │   ├── vision_judge.rs   # before/after screen diff
│   │   ├── classifier.rs     # PASS/DEAD/ERROR/HANG/AMBIGUOUS
│   │   ├── destructive_policy.rs  # §6.5 enforcement: pattern denylist + modal handler
│   │   └── report_writer.rs  # append to .md
│   ├── ledger/
│   │   └── verification.rs   # VerificationLedger over nexus-memory
│   └── replay/
│       └── harness.rs        # I-5 replay test
├── tests/
│   ├── state_machine.rs      # property tests on transitions
│   ├── invariants.rs         # one test per invariant
│   ├── acl.rs                # I-2 fuzz test: can the scout escape its dirs?
│   ├── input_sandbox.rs       # focus-loss simulation, window allowlist enforcement
│   ├── destructive_policy.rs  # all five §6.5 acceptance cases
│   ├── report_format.rs       # §5.0 contract: Observed section is allowlist-only
│   ├── replay.rs             # full session replay
│   └── phase1_teams.rs       # the Phase 1 acceptance run
└── README.md
```

**Removed from v1:** `code_locator.rs`, `repair_proposer.rs`, `regression_runner.rs`, `driver/hitl.rs`, `ledger/taint.rs`. These were all repair-side. They are deferred to a possible v2 when Phase A has run for months and we have data on which fixes are mechanical enough to attempt automatically.

Added in this amendment: governance/input_sandbox.rs (lands in Phase 1.3 alongside
the first nexus-computer-use import).

---

## 8. Open questions (revised from v1 §8)

1. **HITL surface — RESOLVED.** Not needed in Phase A. Deferred.

2. **Taint granularity — DEFERRED.** Phase A doesn't need taint propagation because it doesn't repair. Phase 5 CI gate may revisit.

3. **Vision_judge ambiguity threshold — STILL OPEN.** Calibrate from Phase 1 data: log every vision_judge call's similarity score and the human-verified ground truth, build a small calibration set, set the threshold from data. Do not hard-code a number.

4. **Replay storage — STILL OPEN, low priority.** Soft cap at 100MB per session; gzip on next driver start; revisit at Phase 5.

5. **NEW: Overnight scout mode.** Once Phase 1 proves the loop works on Teams, should the scout run unattended overnight on all 87 pages, producing a directory of 87 reports Suresh wakes up to? **Recommendation: yes, with three safeties:**
   - Hard kill switch at `~/.nexus/HALT` checked every loop iteration
   - Per-page time budget (e.g., 15 min max) — page exceeded → log and skip
   - Daily cost ceiling on Anthropic API credit (e.g., $1/day) — exceeded → halt

6. **NEW: First-pass observe-only mode for Phase 1.** Before the scout writes any bug reports, do one full pass of Teams in pure observe mode — enumerate, click, classify, but **do not write the report file**. Suresh eyeballs the live console output. *Then* allow report writing. This catches "the scout thinks 90% of buttons are dead because it doesn't understand the page" before producing a misleading artifact.

7. **VerificationLedger integrity at scale.** The ledger lives in-process and trusts its own state. At one page this is fine. At 87 pages with overnight runs, a fingerprint collision or memory corruption silently causes the "skip what hasn't changed" optimization to skip something that's actually broken. Phase 4 should ship a ledger integrity check: on every load, verify the ledger's hash against an Ed25519-signed checkpoint written at the previous run's end. Mismatch → invalidate the ledger, re-test everything. Defer until Phase 4.

---

## 9. What this is not (still applies from v1)

- Not a fuzzer. Goal-directed QA scout.
- Not a replacement for `cargo test`. Unit tests stay.
- Not a swarm. One driver.
- **NEW:** Not a code editor. Phase A cannot modify source files. Structurally cannot.
- Not allowed near `claude_cli` or `claude.ai` credits in autonomous mode.

---

## 10. Why this is the right call

Three reasons, in plain language:

1. **It eliminates the failure mode that scared us on April 6.** An unsupervised loop with file-write permissions cannot damage the repo if it has no file-write permissions. The risk goes to zero, not "low."

2. **It uses every dollar Suresh is paying for, exactly as licensed.** The Max plan and the $150 credits flow through Claude Code interactive — which is what they're for. The $10 API credit becomes a barely-touched backup. The ChatGPT Plus subscription does the bulk autonomous work. Nothing is wasted, nothing is at ToS risk.

3. **The repair phase teaches Suresh the codebase deeper.** Every bug walked through with Opus interactively builds intuition that an autonomous fixer would hide. At the scale of a solo founder building infrastructure, that intuition compounds and is more valuable than the time saved.

The autonomous-repair version was faster on paper. The scout-and-repair-split version is faster in reality, because no fix is silently wrong, no rework cycles eat the gains, and no midnight panics destroy a week of work.

---

**v1.1 status:** Spec complete. Ready for implementation. No code shipped until Suresh signs off.
