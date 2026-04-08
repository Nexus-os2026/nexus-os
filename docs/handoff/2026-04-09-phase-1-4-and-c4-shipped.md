# Nexus UI Repair (Crate #69) — Session Handoff (Day 2)
**Date:** April 9, 2026
**Last commit:** `ac6e1f9e` (C4 — driver loop halts on fatal vision_judge errors)
**Repo:** `~/NEXUS/nexus-os` on `main`, synced to GitLab + GitHub
**Roadmap of record:** `docs/roadmaps/NEXUS_UI_REPAIR_v1.1_scout_repair_split.md` (with Holes A/B/C amendment applied)
**Previous handoff:** `docs/handoff/2026-04-08-phase-1-3-5-shipped.md` — read that first for context predating Phase 1.4

This is a supplement to the April 8 handoff, not a replacement. The April 8 doc covers Phases 1.1 through 1.3.5. This doc covers Phase 1.4 + the C4 follow-up shipped on April 9 after a 6-hour rest period.

---

## 1. Where you are right now

`main` is at `ac6e1f9e`, fully synced to both remotes. Working tree is clean except for the same two leftover files (`app/src-tauri/src/commands/cognitive.rs` and `app/src/pages/Agents.tsx`) that have been carried since before the April 8 session. **They are not Phase 1.4 work, they are not Phase 1.5 work, and they have now persisted across two coding days. Resolve them before Phase 1.5 starts** (recommendation: `git stash push -m "parked-pre-phase-1-4-cognitive-agents" app/src-tauri/src/commands/cognitive.rs app/src/pages/Agents.tsx`).

**Crate #69 status:** Phases 1.1, 1.2, 1.3, 1.3.5, and 1.4 are all shipped. C4 (the Phase 1.4 follow-up safety fix) is also shipped. All 7 v1.1 amendment items are landed structurally AND actively enforced where applicable.

**Test count in `nexus-ui-repair`:** 93 passing + 4 ignored.
- 93 = 52 from Phase 1.3.5 baseline + 39 new tests in Phase 1.4 + 1 new sync test (C3) + 1 new halt test (C4)
- 4 ignored = phase1_teams Phase 1.5 stub + 2 xvfb_smoke structural + 1 vision_judge integration_real_codex_call

**CI status:** the last fully-green pipeline was `#2436230151` against `e2d341f4` (Phase 1.4). The pipeline against `ac6e1f9e` (C4) was queued at the time of handoff finalization — verify it landed green before starting Phase 1.5.

---

## 2. The 2 commits shipped today (April 9)

In chronological order:

```
e2d341f4  feat(nexus-ui-repair): Phase 1.4 — driver loop + Codex CLI vision_judge + cost ceiling + dry-run + heartbeat
ac6e1f9e  fix(nexus-ui-repair): C4 — driver loop halts on fatal vision_judge errors
```

**That's it for today.** Two commits, both substantial. Phase 1.4 was the largest single commit in crate #69's history; C4 is its small focused safety follow-up.

### Phase 1.4 details (commit e2d341f4)

Phase 1.4 was originally written by Claude Code in an autonomous overnight session that ran past the "stop for the night" instruction from the previous evening. When Suresh returned in the morning, the working tree had ~35 dirty files representing potentially-complete Phase 1.4 work, none of it reviewed.

The morning's review:
- Verified `cargo check`, `cargo fmt --check`, `cargo clippy -D warnings`, and `cargo test` all clean before reading any code
- Read the highest-risk files line-by-line: `vision_judge.rs` (445 lines), `cost_ceiling.rs` (155 lines), `bin/nexus_ui_repair.rs` (128 lines), `governance/calibration.rs` (recompute logic), and structurally skimmed `driver/loop_.rs` (355 lines)
- Identified four issues, scoped C1+C2+C3 as "fix in this commit," scoped C4 as "follow-up commit"

The three review fixes baked into Phase 1.4:

- **C1:** `cost_ceiling.rs::record_spend` was incrementing in-memory `spent_usd` BEFORE persisting to disk. If `save_to_disk` failed (disk full, permission error), the in-memory state would diverge from disk and the next session restart would "forget" the failed call, silently undercounting spend. Fixed: persist first, mutate only on successful save. ~5 lines.
- **C2:** Both `cost_ceiling.rs` and `calibration.rs::recompute_thresholds` bypassed `Acl::ensure_parent_dirs` and used raw `std::fs::create_dir_all` for parent directory creation. Fixed: route both through ACL helper. Note: the C2 fix constructs a fresh per-call `Acl` rooted at the parent directory, which satisfies the syntactic requirement but does not enforce the "writes constrained to a fixed root" property of a shared driver-scoped ACL. **See Issue T8 in the deferred list.**
- **C3:** The classifier rule duplication between `specialists/classifier.rs::Classifier::classify` and `governance/calibration.rs::predict_with_thresholds` (the rule table re-implementation needed by the F1 sweep) had no test verifying the two implementations stay in sync. Added `classifier_rules_match_sweep_predictor` in `tests/calibration_recompute.rs` covering 5 representative cases (one per classification branch) plus 2 boundary conditions. Test passed on first run, confirming no current drift.

What Phase 1.4 actually shipped (in `e2d341f4`):
- `driver/loop_.rs` (355 lines): Driver::run state machine wired end-to-end
- `driver/heartbeat.rs`: tokio task writing heartbeat file at configurable interval (default 1000ms — see Issue T3)
- `governance/cost_ceiling.rs` (155 lines): cross-session $10 ceiling persisted at `~/.nexus/ui-repair/spend.json`, atomic temp+rename writes, ACL-routed parent dir creation (per C2)
- `specialists/vision_judge.rs` (445 lines): real Codex CLI subprocess wrapper (built inline, not depending on `nexus-code` crate per the SG0 stop gate finding), real Anthropic Haiku 4.5 escalation through trait-based AnthropicClient, pre-call AND post-call cost ceiling checks, three-layer routing table immutability defense
- `specialists/vision_schema.rs`: VisionVerdict JSON schema with version constant
- `specialists/classifier.rs` (166 lines): real rule table for {Pass, Dead, Error, Hang, Ambiguous} with calibration-tunable thresholds
- `governance/calibration.rs` recompute_thresholds (added 265 lines): F1 sweep over 520 (min_window, hang_threshold) combinations, joins synthetic test entries against ground truth labels, writes RecomputeReport
- `bin/nexus_ui_repair.rs`: clap CLI with default `--dry-run = true` for safety (opt-out via `--no-dry-run`)
- 39 new tests covering all of the above plus 1 new ignored test (`integration_real_codex_call` for manual verification against real Codex CLI)
- New deps: chrono 0.4.44, async-trait 0.1, base64 0.22, reqwest 0.12 (rustls-tls only, no OpenSSL), tempfile 3, clap 4 with derive
- 4 mock_codex shell scripts as test fixtures (happy/fail/garbage/no-output paths)
- 2 calibration JSONL fixtures (synthetic entries + synthetic labels — NOT translated from `docs/qa/teams_page_ground_truth_v1.md`, real ground truth still deferred to Phase 1.5)

### C4 details (commit ac6e1f9e)

The Phase 1.4 driver loop swallowed all `vision_judge` errors with `tracing::warn!` and continued, including `CostCeilingExceeded`. This meant when the budget ran out, the driver would continue calling vision_judge on every subsequent page, hitting the same ceiling check, logging a warning, and producing N warning entries instead of one clean halt entry. Behavior was safe (no money spent past the ceiling) but the audit forensics were wrong, and the calibration telemetry from a real Phase 1.5 run would be partially poisoned.

C4 fixes this with policy-based error handling:

**Halt-worthy errors (driver returns Ok(outcome) with halt populated, exits run() early):**
- `CostCeilingExceeded` — the canonical case
- `CodexSpawnFailed` / `CodexExitedNonZero` / `OutputFileMissing` — Codex configuration / execution failures, persistent
- `MissingAnthropicApiKey` — configuration error
- `AuditLogFailed` — audit log integrity is essential for replay determinism
- `Io` — filesystem errors at this layer are usually broken invariants
- `Crate` — wrapped `crate::Error` variants are invariant violations by definition

**Continue-worthy errors (driver logs warning, proceeds to next iteration):**
- `OutputParseFailed` — could be a one-off bad LLM response
- `AnthropicHttp` — could be a one-off 503 / 429 / network blip

The `should_halt(&VisionJudgeError) -> bool` helper uses an exhaustive match (no wildcard) so future additions to the `VisionJudgeError` enum will force a compile-time decision. New `HaltReason` struct carries page, element, human-readable reason, and stable `error_kind` string. New `outcome.halt: Option<HaltReason>` field on `DriverOutcome`. CLI binary checks for halt before printing the success line and exits with code 2 on halt, code 0 on success.

**One architectural note worth knowing:** C4 introduced a new `VisionJudger` trait local to `driver/loop_.rs` instead of in `specialists/vision_judge.rs`. This was forced by the C4 prompt's constraint to not touch `specialists/vision_judge.rs`. The trait should eventually be moved to its rightful home — see Issue T9 in the deferred list.

C4's new test (`driver_halts_cleanly_when_vision_judge_returns_cost_ceiling_exceeded`) uses a `CostCeilingMockJudge` implementing `VisionJudger` directly to return `Err(CostCeilingExceeded)` on the first call. Asserts the driver returns Ok with halt populated, only 1 page visited (out of 3 in the work list), 0 successful vision_calls, and the audit log contains exactly one entry with kind "halt".

---

## 3. Updated module map of `crates/nexus-ui-repair/`

Changes from the April 8 handoff are marked **NEW** or **MODIFIED**.

```
src/
├── lib.rs
├── governance/
│   ├── mod.rs
│   ├── identity.rs
│   ├── invariants.rs
│   ├── routing.rs                  MODIFIED — new constants for AnthropicApi haiku-4.5 model name, new test helper Acl::with_roots etc
│   ├── acl.rs
│   ├── audit.rs                    MODIFIED — new "halt" kind support
│   ├── input_sandbox.rs
│   ├── calibration.rs              MODIFIED — recompute_thresholds + score_thresholds + predict_with_thresholds (rule duplication, see Issue T5/C3)
│   ├── cost_ceiling.rs             NEW — Phase 1.4
│   └── xvfb_session.rs
├── driver/
│   ├── mod.rs                      MODIFIED — re-exports HaltReason and VisionJudger trait
│   ├── state.rs
│   ├── loop_.rs                    MODIFIED — fully wired (was stub before Phase 1.4) + C4 halt handling
│   └── heartbeat.rs                NEW — Phase 1.4
├── specialists/
│   ├── mod.rs
│   ├── enumerator.rs
│   ├── vision_judge.rs             MODIFIED — fully wired with real Codex CLI + Anthropic Haiku (was stub before Phase 1.4)
│   ├── vision_schema.rs            NEW — Phase 1.4
│   ├── classifier.rs               MODIFIED — real rule table (was stub before Phase 1.4)
│   ├── report_writer.rs
│   ├── destructive_policy.rs
│   ├── modal_handler.rs
│   ├── specialist_call.rs
│   └── eyes_and_hands.rs
├── descriptors/
│   └── page_descriptor.rs
├── ledger/
│   └── verification.rs
├── replay/
│   └── harness.rs                  STILL A STUB — replay/harness.rs::ReplayHarness::replay returns Ok(()) constant
└── bin/
    └── nexus_ui_repair.rs          NEW — Phase 1.4

tests/
├── (all existing test files preserved)
├── calibration_recompute.rs        NEW — Phase 1.4 + C3 sync test
├── classifier.rs                   NEW — Phase 1.4
├── cost_ceiling.rs                 NEW — Phase 1.4
├── driver_loop.rs                  NEW — Phase 1.4 + C4 halt test
├── timestamps.rs                   NEW — Phase 1.4
├── vision_judge.rs                 NEW — Phase 1.4 (10 tests, 1 #[ignore]'d)
├── vision_schema.rs                NEW — Phase 1.4
└── fixtures/
    ├── (all existing fixtures preserved)
    ├── calibration_synthetic.jsonl       NEW — synthetic ClassifierInput entries
    ├── calibration_ground_truth.jsonl    NEW — synthetic labels (NOT real GT, see Phase 1.5 notes)
    ├── mock_codex.sh                     NEW — happy path Codex CLI mock
    ├── mock_codex_fail.sh                NEW — non-zero exit
    ├── mock_codex_garbage.sh             NEW — unparseable output
    └── mock_codex_no_output.sh           NEW — missing output file
```

### What's still a stub
- **`replay/harness.rs::ReplayHarness::replay`** — returns `Ok(())` constant. This is the natural home for replay determinism work in Phase 1.5 or later.

---

## 4. Known issues parked, not fixed

### Carried from the April 8 handoff (still present):
- **dev_server flake** in `agents/web-builder/src/dev_server.rs:575`. Did NOT trip on the Phase 1.4 CI run (`#2436230151`) — passed first try. Re-evaluate the "deterministically broken" assessment from last night: the flake is genuinely intermittent, ~30-50% trip rate. Defer the fix until it next trips on a real CI run, at which point we have fresh data to debug against.
- **`cognitive.rs` and `Agents.tsx` uncommitted changes** — STILL THERE. Two days running. **Resolve before Phase 1.5 starts** with `git stash push -m "..." app/...`.
- **Dashboard mock false-positive TODO** — unchanged.
- **Bare Xvfb pixel-ground-truth limitation** — unchanged. Phase 1.5.5 will not see this with real Tauri WebView as the test target.
- **Phase 1.3.5.1 (un-ignore xvfb_smoke in CI)** — unchanged. Still parked. Combine with `xvfb` apt-get install in `.gitlab-ci.yml`.

### New issues from Phase 1.4 + C4:
- **T1:** `tempfile = "3"` is now in both `[dependencies]` and `[dev-dependencies]` of `crates/nexus-ui-repair/Cargo.toml`. Cosmetic. Cargo dedupes. Cleanup later.
- **T2:** Verify whether `tempfile` is actually used in production code or only in tests; if only in tests, remove the runtime dep.
- **T3:** Heartbeat default interval is 1000ms (Phase 1.4 chose responsiveness over efficiency). The original spec said 30000ms in production, 100ms in tests. CLI override available via `--heartbeat-interval-ms`. Reconsider the default if `~/.nexus/ui-repair/heartbeat` write rate becomes a concern on long-running rollouts.
- **T4:** `bin/nexus_ui_repair.rs::tracing_subscriber_init` is empty. Tracing events from production code become no-ops in the binary. Future small commit: add `tracing-subscriber` dep + one line of init.
- **T5 (mitigated by C3):** Classifier rules are duplicated between `specialists/classifier.rs::Classifier::classify` and `governance/calibration.rs::predict_with_thresholds`. Documented as intentional with an inline comment. The C3 sync test catches drift between the two. Future refactor: lift the rules into a shared free function (see also T9).
- **T6:** `score_thresholds` silently skips samples that fail to deserialize `ClassifierInput`. Acceptable for calibration sweeps but worth knowing.
- **T7:** Driver loop error policy for `vision_judge` errors was made by C4 but deserves more design thought beyond the initial halt/continue split. Specifically: should `AnthropicHttp` halt after N consecutive failures? Should `OutputParseFailed` count toward a per-page retry budget? Defer to a future phase.
- **T8:** The C2 ACL fix constructs a fresh per-call `Acl` rooted at the parent directory passed to `cost_ceiling::load_from_disk` and `calibration::recompute_thresholds`. This satisfies the syntactic "all writes go through `Acl::ensure_parent_dirs`" requirement but does NOT enforce the "writes constrained to a fixed root" property of a shared driver-scoped ACL. Future refactor: lift a shared `Acl` to `Driver::new(config)` and inject it into `CostCeiling` and `CalibrationLog::recompute_thresholds`. ~50-80 lines across multiple files.
- **T9 (NEW from C4):** The `VisionJudger` trait was added to `driver/loop_.rs` because the C4 prompt forbade touching `specialists/vision_judge.rs`. Architecturally, the trait belongs in `specialists/vision_judge.rs` next to the concrete `VisionJudge` it abstracts. Future small refactor: move the trait definition to `specialists/vision_judge.rs`, leave the blanket impl there, update the `use` import in `driver/loop_.rs`. ~10 lines of code movement, no semantic change.

---

## 5. Deviations from the original spec that are intentional, defended

### From Phase 1.4:
- **D1:** Cost ceiling persistence uses a single `spend.json` (running total only) instead of the JSONL append-only + summary-file pattern proposed in the planning chat. The per-call audit trail still exists in `audit.jsonl` via `SpecialistCall::record`, just in a different file. Defensible simplification — the audit log is the canonical per-call record, and `spend.json` only needs to know the running total to gate calls.
- **D2:** `vision_judge` built the Codex CLI subprocess wrapper inline in `specialists/vision_judge.rs` rather than depending on a wrapper in `crates/nexus-code`. The SG0 stop gate (the one that runs before SG1) found no existing Codex provider in `nexus-code`, so the inline wrapper was the only option. The wrapper is well-designed (schema-enforced JSON output, four error variants, no env mutation issues).

### From C4:
- **D3:** The `VisionJudger` trait was placed in `driver/loop_.rs` instead of `specialists/vision_judge.rs` due to the prompt's hard constraint. See Issue T9 above.

---

## 6. Phase 1.5 starting context for fresh-Claude

### What Phase 1.5 is supposed to do
Phase 1.5 is the first time the scout drives a real Nexus OS page (the Builder Teams page) instead of fixture HTML. It's the moment the scout transitions from "synthetic test data" to "real measured performance against reality." Specifically:

1. The driver runs against the actual Nexus OS Tauri app (probably inside Xvfb per the Phase 1.3.5 wiring)
2. `EyesAndHands::capture` captures the real WebView contents
3. `vision_judge` makes real Codex CLI calls (and possibly real Anthropic Haiku escalations)
4. The scout enumerates and clicks every interactive element on the Teams page
5. Findings are compared against `docs/qa/teams_page_ground_truth_v1.md` (the sealed tiebreaker)
6. Calibration data accumulates for the recompute pass

### What Phase 1.5 is NOT
- Not a CI-runnable phase. Real Codex CLI calls and a running Nexus OS instance can't happen in GitLab Runner. Phase 1.5's run is a manual local invocation.
- Not the place to wire the replay harness (`replay/harness.rs` is still a stub). Replay can be its own Phase 1.5.5 or later.
- Not multi-page. Phase 1.5 is *just* the Teams page. Other pages come in Phase 2.

### Critical blockers fresh-Claude must address before drafting the Phase 1.5 prompt

**Blocker 1: The ground truth doc at `docs/qa/teams_page_ground_truth_v1.md` was sealed incomplete on April 7.** Per the doc, only 2 confirmed bugs are filled in (GT-001 variant dropdown empty, GT-002 edit team button dead). GT-003+ are placeholder rows with explicit notes "fill in from memory — open Builder Teams in Nexus OS if it helps." The doc was sealed with the discipline of "don't edit after first scout run" but its contents were never finished.

**This is the load-bearing decision Phase 1.5 needs to make.** Two options:

- **Option A:** Spend 30-60 minutes manually clicking through Builder Teams in Nexus OS, cataloguing every bug, filling in GT-003 through whatever number, then re-sealing the doc with a new comment line `# Re-sealed YYYY-MM-DD with N total bugs before Phase 1.5 first run`. THEN run Phase 1.5.
- **Option B:** Accept the 2-entry ground truth as-is and design Phase 1.5's comparison harness to handle "ground truth incomplete" as a first-class state with three-way classification: `confirmed_match`, `unknown_new`, `confirmed_miss`. The `unknown_new` bucket gets manual triage after the first run, and bugs that turn out to be real get added to a new `teams_page_ground_truth_v2.md` (the v1 seal is preserved, never edited).

The fresh-Claude session in the parallel chat (paused) recommended Option A as a time-boxed 45-minute manual cataloguing session before Phase 1.5 starts. **Suresh has not yet decided.**

**Blocker 2: Builder dev server reachability.** Phase 1.5 needs to know how to reach the Teams page. Questions to answer:
- Is the Builder dev server running already, or does the scout need to start it?
- What URL serves Teams? (`http://localhost:5173/teams`? Something else?)
- Does Phase 1.5 spawn the dev server as part of its setup, or is that a manual prerequisite?

**Blocker 3: Phase 1.5 scout behavior shape.** Two interpretations of "drives the Teams page":
- **(a) Exhaustive shallow:** enumerate every interactive element on Teams, click each one once, observe the result, classify. Closest to what the existing fixture-based tests do.
- **(b) Scripted deep:** follow scripted user journeys (create-team → edit-team → add-variant → delete-team) and observe each step. Requires the scout to have a notion of "user intent sequences" which doesn't exist in the crate yet.

Phase 1.5 should probably be (a) because the infrastructure is ready for it and (b) is a Phase 2 concern. **Suresh should confirm.**

**Blocker 4: The two leftover files (`cognitive.rs`, `Agents.tsx`) MUST be resolved before Phase 1.5 starts.** They've been carried for two coding days. The discipline rule is "working tree must be clean before stop gates run." Suggested fix: `git stash push -m "parked-pre-phase-1-5-cognitive-agents" app/src-tauri/src/commands/cognitive.rs app/src/pages/Agents.tsx`.

### Phase 1.5 expected size and risk
Comparable to Phase 1.4 in scope, but the risk profile is different. Phase 1.4 was "wire the loop and the integrations"; Phase 1.5 is "run it against reality and see what breaks." The biggest risk is finding architectural assumptions in Phase 1.4 that don't survive real-world contact. Allow for that — Phase 1.5 may need to ship multiple commits as findings accumulate.

### Standing rules from previous sessions (do NOT change)
- Plain text Claude Code prompts, never markdown lists in instructions
- Every Rust prompt ends with `cargo fmt -p <crate>` + `cargo clippy -p <crate> --all-targets -- -D warnings` + `cargo test -p <crate>`, scoped to the crate
- Never `--all-features`
- Never workspace-wide `cargo test` from inside Claude Code (Suresh runs those manually in his terminal)
- Never resume interrupted Claude Code sessions
- Never route Max plan or $150 Claude.ai credits through nx autonomously (ToS violation, account ban risk)
- Codex CLI / GPT-5.4 is the default scout brain ($0, ChatGPT Plus)
- Anthropic API ($10 credit, hard ceiling enforced by C1+C4) for vision_judge ambiguity escalation only
- Stop gates must be honored — refuse to iterate past failures, ask for input instead
- Long commit messages go to a file via `git commit -F <file>`, never `git commit -m "..."` for messages > ~500 characters (paste hazard learned the hard way)

---

## 7. Process notes from this session — don't repeat these

### From the morning's review:

1. **A 6-hour break does not change git's record of what's on `main`.** When you come back, the FIRST command should always be `git log --oneline -5` (not anything else). Memory after rest can compress "we discussed it" into "we shipped it." Verify with git, always.

2. **Long commit messages on the command line are a paste hazard.** A 6000-character `git commit -m "..."` will get tokenized line-by-line by bash if the paste hits the prompt incorrectly. The morning had two paste failures from this. The fix is `git commit -F <file>` where the message lives in a file and git reads it. **Never use `git commit -m "..."` for messages longer than 200 characters.**

3. **Hard constraints in Claude Code prompts can force suboptimal architecture.** The C4 prompt forbade touching `specialists/vision_judge.rs`, which forced Claude Code to put the `VisionJudger` trait in `driver/loop_.rs` instead of where it belonged. The right call when scoping a prompt is to ask "would the cleanest implementation require touching one or two specific lines outside the listed files?" — and if so, allow it explicitly. Issue T9 is the cost of this lesson.

4. **When Claude Code's autonomous output arrives unreviewed, the discipline is: verify-then-trust, not trust-then-verify.** Run `cargo check`, `cargo fmt --check`, `cargo clippy`, `cargo test` BEFORE reading any code. The verification cost is ~30 seconds and gives a strong baseline for trust. The morning's review went much faster because the test count + exit codes told us the bulk was correct, so we only had to read the highest-risk files carefully.

5. **The post-shipment second wind is the most dangerous part of a long session.** After Phase 1.4 went green on CI, the temptation was to skip C4 and jump to Phase 1.5. The right call was to ship C4 first because the failure mode it prevents (poisoned calibration data on Phase 1.5's first real run) costs hours to debug compared to the 30 minutes C4 took to ship.

### Carried from the April 8 handoff (still apply):

6. Every Rust prompt ends with the cargo trio. No exceptions, even for one-line changes.
7. When git/CI state is confusing, FIRST run `git log --oneline -3` to establish ground truth before reasoning.
8. Empirical validation BEFORE architectural assumption. Verify environment facts with bash before reading source code.
9. Stop gates must be honored. The discipline of "stop and report instead of fixing forward" saved this morning's review from shipping a broken Phase 1.4.

---

## 8. The exact opening message for the next chat (Phase 1.5 planning)

When opening a fresh Claude chat for Phase 1.5 planning, **attach BOTH handoff files** (`docs/handoff/2026-04-08-phase-1-3-5-shipped.md` AND this one, `docs/handoff/2026-04-09-phase-1-4-and-c4-shipped.md`), then paste this:

```
Hey Claude — fresh session. We're picking up Nexus UI Repair (crate
#69) at Phase 1.5. Phases 1.1, 1.2, 1.3, 1.3.5, 1.4, and the C4
follow-up are all shipped to main (commit ac6e1f9e). All 7 v1.1
amendment items are landed structurally and actively where applicable.
The driver loop halts cleanly on cost ceiling exhaustion (C4) so a
real Phase 1.5 run cannot poison calibration telemetry.

Attaching TWO handoff docs from previous sessions — please read both
end-to-end before responding:
- 2026-04-08-phase-1-3-5-shipped.md (covers Phases 1.1-1.3.5)
- 2026-04-09-phase-1-4-and-c4-shipped.md (covers Phase 1.4 + C4)

After you've read them, your job is to walk me through the shape of
Phase 1.5 in 5-10 bullet points so I can sanity-check the scope before
you write any Claude Code prompt. Phase 1.5 drives the real Builder
Teams page in real Nexus OS for the first time — the transition from
synthetic fixtures to real measured performance against reality.

CRITICAL BLOCKERS to address before drafting the prompt:
1. The ground truth doc at docs/qa/teams_page_ground_truth_v1.md was
   sealed incomplete on April 7 with only 2 of N expected entries
   filled in. Decide Option A (manually catalogue all bugs and re-seal
   before first run) vs Option B (accept incomplete, design comparison
   harness around three-way classification). I lean Option A.
2. Builder dev server reachability — how does the scout reach Teams?
3. Scout behavior shape — exhaustive shallow (a) vs scripted deep (b).
4. The two leftover cognitive.rs/Agents.tsx files MUST be stashed
   before Phase 1.5 starts (working tree clean before stop gates).
5. Phase 1.5 is NOT the place to wire replay/harness.rs (still a
   stub). Replay can be 1.5.5.

Standing rules from previous sessions (do NOT change):
- Plain text Claude Code prompts
- Every Rust prompt ends with cargo fmt + clippy + test scoped to the
  crate
- Never --all-features, never workspace-wide cargo test inside Claude
  Code
- Never resume interrupted Claude Code sessions
- Never route Max plan or $150 credits autonomously
- Codex CLI / GPT-5.4 is the default brain
- Anthropic API ($10 ceiling, enforced by cost_ceiling.rs + C4 halt)
  for ambiguity escalation only
- Stop gates must be honored
- Long commit messages go to a file via git commit -F (paste hazard
  learned the hard way April 9)

Before you draft anything, confirm you've read both handoffs and
flag anything you want me to clarify. Then walk me through the shape
of Phase 1.5 in 5-10 bullets so I can sanity-check the scope before
you write the actual Claude Code prompt.
```

---

## 9. What to do right now (immediately after shipping C4)

1. **Verify CI is queued against `ac6e1f9e`** by glancing at the GitLab pipelines page once. Don't watch the pipeline run.
2. **Save this doc** to `docs/handoff/2026-04-09-phase-1-4-and-c4-shipped.md` in the repo.
3. **Commit the doc** as `docs(handoff): session 2026-04-09 — Phase 1.4 + C4 shipped` and push to both remotes.
4. **Resolve the cognitive.rs/Agents.tsx leftover files** with a `git stash` before any further work.
5. **Take a break.** Phase 1.5 is the next big phase and it deserves a fresh start.

---

*End of supplementary handoff. Two commits shipped today — Phase 1.4 and C4 — both genuine wins. Crate #69 has its strongest baseline ever. Phase 1.5 is the next milestone, but it's not for tonight.*
