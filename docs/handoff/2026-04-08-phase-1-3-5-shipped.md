# Nexus UI Repair (Crate #69) — Session Handoff (Day 1)
**Date:** April 8, 2026
**Last commit:** `d6ee360a` (Phase 1.3.5 — Xvfb structural sandbox active)
**Repo:** `~/NEXUS/nexus-os` on `main`, synced to GitLab + GitHub
**Roadmap of record:** `docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md` (with Holes A/B/C amendment applied)
**Next handoff:** `docs/handoff/2026-04-09-phase-1-4-and-c4-shipped.md` — read that next for Phase 1.4 + C4 context

This is the original session handoff covering Phases 1.1 through 1.3.5. The April 9 supplement covers Phase 1.4 and the C4 follow-up. Both should be read together for full context on crate #69.

Note: this file was regenerated from chat history on April 9 because the original April 8 save-to-disk step never actually happened — the doc lived only in conversation. The content below is reconstructed faithfully but the timestamp on the file is later than the work it describes.

---

## 1. Where main was at the end of the April 8 session

`main` was at `d6ee360a`, fully synced to both remotes. Working tree was clean except for two leftover files (`app/src-tauri/src/commands/cognitive.rs` and `app/src/pages/Agents.tsx`) that have been carried since before this session started. They are NOT part of any work from this session and should be left alone until you explicitly decide what to do with them.

**Crate #69 status as of April 8 end:** Phases 1.1, 1.2, 1.3, and 1.3.5 are shipped. Five of seven v1.1 amendment items are landed (the remaining two land in Phase 1.4).

**Test count in `nexus-ui-repair` as of April 8 end:** 52 passing + 3 ignored (`phase1_teams` stub from 1.1, two structural `xvfb_smoke` tests from 1.3.5).

**CI status:** the last fully-green pipeline on April 8 was `#2436007603` against `4f67f174` (the rustfmt fix one commit before 1.3.5). The pipeline against 1.3.5 (`d6ee360a`) was queued at the time of handoff finalization. (Per April 9 update: that pipeline ultimately tripped the dev_server flake twice in a row, but the underlying Phase 1.3.5 code was verified clean. The flake reverted to intermittent behavior on April 9, passing on the Phase 1.4 CI run.)

---

## 2. The 11 commits shipped during the April 8 session

In chronological order (oldest first):

```
d685a167  docs(qa): seal Builder Teams ground truth before nexus-ui-repair v0.1
a5a11fd2  feat(nexus-ui-repair): Phase 1.1 skeleton crate (#69)
58e31f66  feat(nexus-ui-repair): Phase 1.2 — real ACL, fixture enumerator, §5.0 reports
fe8bbee4  chore(security): unblock cargo-deny CI gate
3aea257e  feat(nexus-ui-repair): Phase 1.3 — structural sandbox + Hole B Layers 2/3
3ea5bb52  fix(web-builder-agent): silence 6 clippy 1.94 errors
320926b6  test(frontend): whitelist 5 staged Builder v3.2 collab/conversion components
7d49b2c5  test(frontend): fix stale assertions in Dashboard.test.tsx
dfeb4dfd  fix(nexus-kernel): remove duplicate #[test] attribute on parse_searxng_json
026cfb3e  fix(web-builder-agent): unique per-call tempdir in budget test helper
4f67f174  style(web-builder-agent): rustfmt budget.rs temp_tracker
d6ee360a  feat(nexus-ui-repair): Phase 1.3.5 — Xvfb structural sandbox active
```

Four substantive Phase 1 commits (1.1, 1.2, 1.3, 1.3.5) + one ground-truth seal + seven CI cleanup commits. The ratio is real and worth remembering when looking back at this session: most of the day was clearing pre-existing CI debt that had been rotting on `main` for at least 3 days before this session started, not new feature work. The new feature work is the four `feat(nexus-ui-repair)` commits.

---

## 3. Architectural state of crate #69 at end of April 8

### v1.1 amendment items — what was shipped, what was deferred to Phase 1.4

| # | Item | Phase | Status as of April 8 |
|---|---|---|---|
| 1 | I-1 KernelAllowlist (real validator) | 1.2 | shipped |
| 2 | I-2 Filesystem ACL (real `check_write` with traversal rejection) | 1.2 | shipped |
| 3 | I-2 Per-app input governance (Hole A Layer 2 — structural) | 1.3 | shipped (structural) |
| 3b | I-2 Per-app input governance (active wiring) | 1.3.5 | shipped (`InputSandbox::validate_and_click`) |
| 4 | I-2 Xvfb display isolation (Hole A Layer 3) | 1.3.5 | shipped (`XvfbSession`) |
| 5 | I-3 HitlByDefinition (structural — no autonomous repair to gate) | 1.2 | trivially passes |
| 6 | I-4 ImmutableProviderRouting (real validator with neg case) | 1.2 | shipped (defended at one layer; Phase 1.4 added two more) |
| 7 | I-5 ReplayableSessions (audit log + specialist call recording) | 1.2 / 1.3 | shipped |
| 8 | Hole B Layer 1 (destructive pattern denylist) | 1.2 | shipped |
| 9 | Hole B Layer 2 (modal handler) | 1.3 | shipped |
| 10 | Hole B Layer 3 (PageDescriptor opt-in validation) | 1.3 | shipped |
| 11 | §4.1 replay determinism (`SpecialistCall` API + audit method) | 1.3 | shipped |
| 12 | §8 Q3 vision_judge calibration logging | 1.3 | recording API shipped; recompute logic deferred to Phase 1.4 |

### Module map of `crates/nexus-ui-repair/` at end of April 8

```
src/
├── lib.rs                          # error type, version, module decls
├── governance/
│   ├── mod.rs
│   ├── identity.rs                 # Ed25519 SessionIdentity (1.1)
│   ├── invariants.rs               # 5 invariant validators with neg cases (1.2)
│   ├── routing.rs                  # RoutingTable, forbidden providers (1.1+1.2)
│   ├── acl.rs                      # check_write with traversal rejection (1.2)
│   ├── audit.rs                    # AuditLog hash chain + record_specialist_call (1.1+1.3)
│   ├── input_sandbox.rs            # Hole A Layer 2: structural (1.3) + validate_and_click (1.3.5)
│   ├── calibration.rs              # CalibrationLog JSONL recording API (1.3)
│   └── xvfb_session.rs             # Hole A Layer 3: per-test Xvfb RAII (1.3.5)
├── driver/
│   ├── mod.rs
│   ├── state.rs                    # 5-state DriverState enum (1.1)
│   └── loop_.rs                    # STILL A STUB at end of April 8 — wired in Phase 1.4
├── specialists/
│   ├── mod.rs
│   ├── enumerator.rs               # scraper-based fixture parsing (1.2)
│   ├── vision_judge.rs             # STILL A STUB at end of April 8 — wired in Phase 1.4
│   ├── classifier.rs               # STILL A STUB at end of April 8 — wired in Phase 1.4
│   ├── report_writer.rs            # §5.0 contract serializer with allowlist (1.2)
│   ├── destructive_policy.rs       # Hole B Layer 1 pattern denylist (1.2)
│   ├── modal_handler.rs            # Hole B Layer 2: cancel-only + halt-on-3rd (1.3)
│   ├── specialist_call.rs          # §4.1 SpecialistCall struct (1.3)
│   └── eyes_and_hands.rs           # First real call to nexus-computer-use (1.3.5)
├── descriptors/
│   ├── mod.rs
│   └── page_descriptor.rs          # Hole B Layer 3: opt-in validation (1.3)
├── ledger/
│   ├── mod.rs
│   └── verification.rs             # in-process VerificationLedger (1.1)
└── replay/
    ├── mod.rs
    └── harness.rs                  # STILL A STUB — wired in a future phase

tests/
├── state_machine.rs                # state transition test (1.1)
├── invariants.rs                   # 10 invariant tests pos+neg (1.2)
├── acl.rs                          # 9-case ACL test (1.2)
├── enumerator.rs                   # 4 fixture parsing tests (1.2)
├── report_format.rs                # 5 §5.0 contract tests (1.2)
├── destructive_policy.rs           # 5 §6.5 acceptance tests (1.2 + 1.3)
├── modal_handler.rs                # 7 modal classification tests (1.3)
├── page_descriptor.rs              # 8 opt-in validation tests (1.3)
├── calibration.rs                  # 3 JSONL recording tests (1.3)
├── specialist_call.rs              # 2 chain integrity tests (1.3)
├── input_sandbox.rs                # 3 sandbox + 1 validate_and_click test (1.3 + 1.3.5)
├── xvfb_smoke.rs                   # 2 #[ignore]'d structural tests (1.3.5)
├── replay.rs                       # 1 stub test (1.1)
├── phase1_teams.rs                 # 1 #[ignore]'d Phase 1.5 acceptance stub
└── fixtures/
    ├── teams_page_snapshot.html
    ├── teams_with_delete_modal.html
    ├── teams_modal_no_cancel.html
    └── teams_unrecognized_modal.html
```

### What was still a stub at end of April 8 (the Phase 1.4 surface)

- `driver/loop_.rs::Driver::run` — walks the state machine writing audit entries; doesn't actually call specialists
- `specialists/vision_judge.rs::VisionJudge::judge` — returns `VisionVerdict::Unchanged` constant; needs real Codex CLI integration
- `specialists/classifier.rs::Classifier::classify` — returns `Classification::Pass` constant; needs real classification rules
- `replay/harness.rs::ReplayHarness::replay` — returns `Ok(())` constant; needs real session log replay (still a stub after Phase 1.4 too)

---

## 4. Known issues parked at end of April 8

### `dev_server::tests::test_start_returns_url` flake (`agents/web-builder/`)
- **What:** Vite test in `agents/web-builder/src/dev_server.rs:575` intermittently fails with `ViteStartFailed("Vite exited before ready")`.
- **Why:** Race between vite startup and the test's wait-for-ready timeout.
- **Status as of April 8 end:** Failed twice in a row on the Phase 1.3.5 CI run, leading to the assessment "deterministically broken in CI's environment." This assessment was revised on April 9 when the flake passed first try on Phase 1.4's CI run — it's actually intermittent at ~30-50% trip rate.
- **When to fix:** anytime; small focused commit. Rough fix shape: extend wait-for-ready timeout or add retry loop around vite spawn (~5 lines).
- **Workaround:** retry the failing CI job once. ~60-70% chance it passes on retry.

### `cognitive.rs` and `Agents.tsx` uncommitted changes
- **What:** Two modified files in your working tree (`app/src-tauri/src/commands/cognitive.rs` and `app/src/pages/Agents.tsx`) that have been sitting there since before this session started.
- **Why:** Unknown — they're from some earlier session, unrelated to crate #69.
- **When to fix:** decide whether to commit, stash, or discard. They are NOT part of crate #69 work and should be left alone until you have context on what they were doing.
- **Diff size:** 156 insertions, 165 deletions across the two files.
- **Update from April 9 session:** these are still present and have now persisted across two coding days. **Resolve before Phase 1.5 starts.**

### Dashboard mock false-positive
- **What:** TODO documented in `app/src/pages/__tests__/Dashboard.test.tsx`. The `get_live_system_metrics` mock returns a JS object but production `getLiveSystemMetricsJson` calls `JSON.parse()` expecting a string. This causes the error banner to render under success conditions and means the "shows error state on backend failure" test passes for the wrong reason.
- **Why not fixed:** out of scope for the CI cleanup pass; would have been scope creep.
- **When to fix:** small focused commit anytime. Either wrap the mock value in `JSON.stringify()` or make `getLiveSystemMetricsJson` tolerate parsed objects.

### Bare Xvfb pixel-ground-truth limitation
- **What:** Documented in `crates/nexus-ui-repair/tests/xvfb_smoke.rs` file-level docs. Bare Xvfb without a window manager has four quirks: cursor not painted into framebuffer, xeyes does not redraw without WM events, xsetroot changes do not show up in scrot output, mousemove targets do not persist (getmouselocation returns screen center).
- **Why not fixed:** this is a property of bare Xvfb, not a bug in our code. Will disappear automatically in Phase 1.5.5 when we put real Nexus OS (a Tauri WebView) inside the Xvfb display.
- **When to fix:** Phase 1.5.5. The smoke tests are `#[ignore]`'d as structural verification only until then.

### Phase 1.3.5.1 — un-ignore the smoke tests in CI
- **What:** The two `xvfb_smoke` tests are currently `#[ignore]`'d because GitLab Runner doesn't have Xvfb installed.
- **When to fix:** small follow-up commit (~10 minutes). Add `xvfb` and `x11-apps` to the runner setup in `.gitlab-ci.yml`, remove the `#[ignore]` attribute. Keep the static Mutex serialization.
- **Sequencing:** can ship before, during, or after Phase 1.4 — independent.

---

## 5. Phase 1.4 starting context (as it stood on April 8)

Phase 1.4 is the largest single phase since Phase 1.3 itself. It wires the driver loop end-to-end. **As of April 8 end, Phase 1.4 had not been started.** Per the April 9 supplement, it was actually written by Claude Code overnight in an autonomous session and shipped on April 9 morning after extensive code review.

### Phase 1.4 deliverables (as planned on April 8)

1. **Driver loop wiring.** `Driver::run` stops walking states blindly and actually calls specialists in sequence: `enumerator` → `vision_judge` → `classifier` → `report_writer`. Each call wrapped in `SpecialistCall` and recorded to the audit log per §4.1 replay determinism.

2. **Real `vision_judge` LLM integration.** First call to Codex CLI (GPT-5.4) from the scout. Provider routing through the `RoutingTable` we built in 1.2. Anthropic API ($10 credit) only on ambiguity escalation. Never `claude_cli` or `claude_ai_credits` (forbidden in the autonomous routing table per I-4).

3. **Real `Classifier` rules.** Given a `VisionVerdict` plus console output plus IPC traffic plus DOM mutation log, classify the interaction as `Pass | Dead | Error | Hang | Ambiguous`. Deterministic rules first, LLM adjudication for ambiguous cases.

4. **`--dry-run` flag.** When set: full loop runs (enumerate, plan, act, observe, classify), but `report_writer` prints structured bug entries to stdout instead of writing the .md file. Audit log still writes (you want the audit even on dry runs). VerificationLedger updates write to a scratch path, not the real ledger.

5. **Heartbeat file.** Driver writes to `~/.nexus/ui-repair/heartbeat` every interval with timestamp + current page + current state. External watchdog can monitor it. Defends against silent hangs.

6. **Real timestamps via chrono.** chrono is already in the dep tree transitively via `nexus-computer-use`. Phase 1.4 imports it directly and replaces hardcoded placeholder strings.

7. **Calibration recompute logic.** The `CalibrationLog` recording API exists from Phase 1.3. Phase 1.4 adds the recompute pass: read all calibration entries, find the threshold that maximizes F1 against ground truth, write back to disk.

### Phase 1.4 non-goals (deferred to later phases)

- Real Nexus OS as the test target — Phase 1.5.5
- Multi-window orchestration tests for the destructive policy modal handler — Phase 1.4 has the API but full integration tests wait for real Nexus OS
- The 4 currently-`#[ignore]`'d Phase 1.3 destructive policy tests stay as they are; they need real input gating which 1.4 provides but the multi-window test environment is 1.5.5

---

## 6. Process notes from the April 8 session — don't repeat these

These are honest mistakes that cost time during April 8. Writing them down so neither tomorrow-Suresh nor fresh-Claude steps on the same rakes.

1. **Every Rust prompt ends with `cargo fmt -p <crate> --check` AS WELL AS `cargo clippy` AND `cargo test`**, even for one-line changes. We dropped fmt from two prompts during this session and CI rejected both. The trio is non-negotiable.

2. **When something doesn't add up in git/CI state, the first command to run is `git log --oneline -3`**, not anything else. Twice during this session mental models drifted from reality and the cure was always "check the actual log first." Establish ground truth before reasoning.

3. **Empirical validation BEFORE architectural assumption.** When a diagnosis depends on an environment fact (like "the user is on Wayland"), verify the environment fact first (`echo $WAYLAND_DISPLAY`, `loginctl show-session`) before reading source. Source reading tells you what code does in a hypothetical environment; env reading tells you which hypothetical applies. We burned 90 minutes on a Wayland-precedence theory that was wrong because nobody verified Wayland was actually in use.

4. **Stop gates must be honored even when the code looks safe.** Phase 1.3.5's smoke test was almost shipped against an unverified test design. The structural failure surfaced only because Claude Code refused to iterate beyond the 2-attempt limit. The discipline of "stop and report instead of fixing forward" saved us from shipping a broken test as if it worked.

5. **Pre-existing CI debt is a real category of work.** When a Phase ships against a baseline that hasn't had a green CI in 3 days, expect to spend more than half your time clearing inherited issues, not building new things. Budget for it in advance instead of treating it as an unexpected detour.

6. **`web-builder-agent` has been undertested locally and surfaces issues only in CI.** Multiple pre-existing bugs landed in CI this session: clippy 1.94 violations, frontend test stale assertions, frontend orphan detection, budget tempdir cross-UID failure. Per-crate `cargo test -p` isn't enough discipline for that crate specifically. Worth a periodic full-workspace test run.

7. **The architecture is correct, the tests are correct, the friction is X11.** Phase 1.3.5's smoke test fought four different bare-Xvfb quirks before we accepted that bare Xvfb is not a real display environment. The lesson: when a test target doesn't behave like reality, the right move is to swap the test target, not to keep debugging it. xeyes/xsetroot/cursor-position were the wrong target. Phase 1.5.5 (real Nexus OS in Xvfb) is the right target.

---

## 7. Standing rules established during the April 8 session

These are workflow rules Suresh established and that should NOT be changed in future sessions:

- Plain text Claude Code prompts, never markdown lists in instructions
- Every Rust prompt ends with `cargo fmt -p <crate>` + `cargo clippy -p <crate> --all-targets -- -D warnings` + `cargo test -p <crate>`, scoped to the crate
- Never `--all-features` (triggers Candle ML crash on 62GB RAM machine)
- Never workspace-wide `cargo test` from inside Claude Code (Suresh runs those manually in his terminal)
- Never resume interrupted Claude Code sessions
- Never route Max plan or $150 Claude.ai credits through nx autonomously (ToS violation, account ban risk)
- Codex CLI / GPT-5.4 is the default scout brain ($0, ChatGPT Plus)
- Anthropic API ($10 credit) for vision_judge ambiguity escalation only
- Stop gates must be honored — refuse to iterate past failures, ask for input instead

Additional rules added during the April 9 session (preserved here for the next chat to reference):

- A 6-hour break does not change git's record of what's on `main`. When you come back, the FIRST command should always be `git log --oneline -5`.
- Long commit messages on the command line are a paste hazard. Never use `git commit -m "..."` for messages longer than ~200 characters. Use `git commit -F <file>` instead.
- Hard constraints in Claude Code prompts can force suboptimal architecture. When scoping a prompt, ask "would the cleanest implementation require touching one or two specific lines outside the listed files?" — and if so, allow it explicitly.

---

## 8. The arc of the April 8 session, briefly

The April 8 session started at roughly midnight on April 7-8 when Suresh and Claude began drafting the v1.1 amendment and the Phase 1.1 prompt. The session ran continuously for ~14 hours, shipping 11 commits. The work split roughly as:

- **Hours 1-3:** Phases 1.1, 1.2 (the foundational Phase 1 work)
- **Hours 4-6:** CI cleanup as the cargo-deny gate, clippy 1.94 errors, frontend test issues, and the kernel `#[test]` typo all surfaced sequentially
- **Hours 7-9:** Phase 1.3 (structural sandbox + Hole B Layers 2/3, the largest phase)
- **Hours 10-12:** Budget tempdir cross-UID bug debugging — diagnosed via Claude Code stop gates, fixed with the unique-per-call tempdir helper
- **Hours 13-14:** Phase 1.3.5 (Xvfb structural sandbox + EyesAndHands wiring + the four bare-Xvfb quirks discovered and worked around with structural-only smoke tests)

The session ended with a "stop for the night" instruction and a plan to start Phase 1.4 fresh the next morning. (Claude Code in a parallel session went ahead and wrote Phase 1.4 anyway during the night, which is what created the April 9 morning's "35 unknown dirty files" situation. The April 9 supplement covers what happened next.)

---

*End of regenerated April 8 handoff. The story continues in `2026-04-09-phase-1-4-and-c4-shipped.md`.*
