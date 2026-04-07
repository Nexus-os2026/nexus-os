# NEXUS_UI_REPAIR v1.1 — Amendment Patch (Holes A, B, C)

**Amends:** `NEXUS_UI_REPAIR_v1.1_scout_repair_split.md`
**Drafted:** April 7, 2026
**Status:** Apply before Phase 1.2. Phase 1.1 (skeleton) is unaffected and may proceed first.
**Author:** Suresh Karicheti + Claude (architecture review)

---

## 0. Why this amendment exists

A close reading of v1.1 surfaced three architectural holes that would cause real failures at Phase 1.3 or later if not addressed:

- **Hole A** — I-2 enforcement narrative pointed at the wrong layer (Rust `fs::write`) and missed the real attack surface (desktop input control via `nexus-computer-use`).
- **Hole B** — No policy preventing the scout from clicking destructive buttons inside Nexus OS itself (Settings → Reset, Memory → Wipe, etc.).
- **Hole C** — Bug report format mixed deterministic observations with LLM speculation in a single flat list, creating wasted Phase B cycles when Opus chases hallucinated file paths.

Phase 1.1 (skeleton) has no desktop reach and is unaffected. This amendment must be applied before Phase 1.3 (first `nexus-computer-use` integration).

---

## 1. Hole A — I-2 enforcement at the right layer

### 1.1 Replace v1.1 §3, row I-2

**REMOVE** the existing I-2 row from the §3 invariants table:

```
| I-2 | Read-only filesystem. The scout has zero file write capability.
       It can write only to ~/.nexus/ui-repair/reports/** and
       ~/.nexus/ui-repair/sessions/** (its own report and audit
       directories). | ACL granted at session start; no nx_apply_patch
       tool registered; fs::write outside the allowed dirs returns
       PermissionDenied at the syscall layer |
```

**REPLACE WITH:**

```
| I-2 | Read-only-by-construction. The scout has no capability to
       modify state outside its own report and audit directories.
       This holds across THREE layers: filesystem, desktop input,
       and OS process isolation. | See §1.2 below — the enforcement
       is multi-layer, not single-line. |
```

### 1.2 Insert new section after §3

Insert this as **§3.1 — I-2 enforcement (load-bearing detail)**:

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

### 1.3 Update §7 file layout

Add the following file to the `governance/` directory listing:

```
│   │   ├── input_sandbox.rs   # I-2 Layer 2: per-app governance config + focus check
```

Add the following file to the `tests/` directory listing:

```
│   ├── input_sandbox.rs       # focus-loss simulation, window allowlist enforcement
```

Add the following entry to the deferred-from-v1 list at the end of §7:

```
Added in this amendment: governance/input_sandbox.rs (lands in Phase 1.3 alongside
the first nexus-computer-use import).
```

---

## 2. Hole B — Destructive Action Policy

### 2.1 Insert new section after v1.1 §6

Insert as **§6.5 — Destructive Action Policy**:

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

### 2.2 Update §7 file layout

Add to the `specialists/` directory listing:

```
│   │   ├── destructive_policy.rs  # §6.5 enforcement: pattern denylist + modal handler
```

Add to the `tests/` directory listing:

```
│   ├── destructive_policy.rs  # all five §6.5 acceptance cases
```

---

## 3. Hole C — Split bug report format (Observed vs LLM Analysis)

### 3.1 Replace v1.1 §5 example bug report

**REMOVE** the existing `### BUG-001: "Edit team" button does nothing` example block from §5.

**REPLACE WITH:**

```markdown
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
```

### 3.2 Insert new section before §5's existing content

Insert as **§5.0 — Bug report contract**:

> Every bug entry in a scout report has exactly two sections, separated by an explicit visual wall: **Observed (deterministic)** and **LLM analysis (Codex CLI guess)**. The wall exists because the scout produces both kinds of content, and conflating them in Phase B is the single biggest source of wasted Opus cycles.
>
> **Observed** contains only facts the scout measured directly: element identity, bounds, the action taken, screenshot file paths, vision diff similarity score, console output, IPC traffic, network traffic, DOM mutations, focus changes. Every line in this section is reproducible from the session audit log. Nothing in this section is generated by an LLM; the section is assembled by deterministic code that reads instrumentation output.
>
> **LLM analysis** contains the `report_writer` specialist's interpretation of the observed facts: likely cause, suggested files to check, reasoning, confidence level. Every line in this section is a guess by Codex CLI based on pattern matching against the page route and common bug shapes. The file paths in particular are *not* derived from a real code trace — code tracing is a Phase 2 specialist that doesn't exist yet. The section header includes the literal phrase "verify before trusting" to make this unambiguous in Phase B.
>
> **Phase B usage rule:** when you sit down with a report and Claude Code, paste the **Observed** section into Opus first and ask Opus to propose where the bug might be, *without* showing Opus the LLM analysis section. Then compare Opus's hypothesis to the scout's hypothesis. When they agree, confidence is high. When they disagree, the Observed section is the tiebreaker — it is the only ground truth in the report.
>
> The `report_writer` specialist is forbidden from putting any speculation in the Observed section. A unit test (`tests/report_format.rs`) parses generated reports and asserts that the Observed section contains only field names from a fixed allowlist; any other field name fails the test.

### 3.3 Update §7 file layout

Add to the `tests/` directory listing:

```
│   ├── report_format.rs       # §5.0 contract: Observed section is allowlist-only
```

---

## 4. Other small fixes carried in this amendment

These are the two smaller flags from the architectural review. They're cheap and worth landing in the same patch.

### 4.1 Replay determinism (I-5 clarification)

Append the following sentence to v1.1 §3 row I-5, in the Enforcement column:

> Every specialist call records `(inputs, output)` to the audit log, not just inputs — the output capture is what makes replay byte-identical despite non-deterministic LLM calls in `vision_judge`.

### 4.2 VerificationLedger trust (Phase 4 TODO)

Append a new bullet to v1.1 §8 (open questions), as **question 7**:

> 7. **VerificationLedger integrity at scale.** The ledger lives in-process and trusts its own state. At one page this is fine. At 87 pages with overnight runs, a fingerprint collision or memory corruption silently causes the "skip what hasn't changed" optimization to skip something that's actually broken. Phase 4 should ship a ledger integrity check: on every load, verify the ledger's hash against an Ed25519-signed checkpoint written at the previous run's end. Mismatch → invalidate the ledger, re-test everything. Defer until Phase 4.

---

## 5. Phase gating summary

After this amendment is applied, the gating between Phase 1 sub-phases is:

| Phase | What lands | What must already exist |
|---|---|---|
| **1.1** | Skeleton crate, ACL stub, ACL test | Nothing (this is the foundation) |
| **1.2** | Real ACL enforcement in `report_writer`, fixture-based enumerator | Phase 1.1 |
| **1.3** | First `nexus-computer-use` integration | Phase 1.2 + **Hole A Layers 2 & 3** + **Hole B all three layers** + **Xvfb environment** — all in the same commit |
| **1.4** | `vision_judge`, replay determinism, calibration logging | Phase 1.3 |
| **1.5** | Drive Builder Teams page end-to-end against ground truth | Phase 1.4 + ground truth file sealed |

The hard rule from this amendment: **Phase 1.3 cannot ship without Holes A and B fully addressed.** Hole C lands at Phase 1.4 alongside the real `report_writer`, but the format contract is locked in this amendment and the tests are written in 1.3 against the stub writer.

---

## 6. What this amendment is not

- **Not a new architecture.** v1.1's two-phase scout/repair split stands. The driver loop in §2 is unchanged. The five invariants are unchanged in name and number; only I-2's enforcement narrative is corrected.
- **Not a delay to Phase 1.1.** Phase 1.1 has no desktop reach and may proceed immediately with the original v1.1 spec. This amendment must be applied before Phase 1.3.
- **Not exhaustive.** There are smaller issues (e.g., per-function vs per-file taint at Phase 5, replay storage rotation) that remain open in v1.1 §8 and are deferred.

---

*Amendment status: ready to apply. After application, v1.1 + this amendment is the spec of record. Phase 1.1 can begin immediately under v1.1; Phase 1.3 begins only after this amendment is applied.*
