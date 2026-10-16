# P3-C11-DR01-D2A1-C7-COMPLIANT-EXEC-02 — execution report (Part C of P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03)

Executor: Claude Code (claude-opus-5-5). Written 2026-10-16 after Stage 12, before any publication step.
Return to: ChatGPT GPT-6 — Sole Decision Architect, via the Nexus OS Owner.

**Executor status: KILL_EVIDENCE_CANDIDATE — ARCHITECT_REVIEW_PENDING.**
This is the executor maximum. It is not a C7 acceptance. It awards nothing under G.7(E) or G.7(F), changes no campaign count,
and does not mean 35/35, D2A-1 accepted, a section 11 label, Candidate 11 integrated or Phase Three complete.

Labels used below:
- **[GIT]** PROVEN FROM GIT OBJECT
- **[RAW]** MEASURED FROM RAW RECEIPT
- **[EXEC]** EXECUTOR ANALYSIS
- **[ARCH]** ARCHITECT DECISION PENDING

## 1. Authority chain (as received; strings are never authority)

- Mission P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03 (`resume-03/MISSION-RESUME-03.txt`, sha256 e9506bf5…d9a4). Part A ended at the
  Part B approval gate with TOOL REPAIR COMPLETE — EXECUTION APPROVAL REQUIRED (`resume-03/PART-A-STATUS.md`).
- The Owner then sent a fresh, explicit Part C authorisation for ONE compliant C7 execution, conditional on the frozen tool
  manifest dc75fcb2afdb931dc37dac819e3038ff8e66837ab0412dd14010573afd0fce22. It was received in session 929aa21f and transcribed at
  2026-10-16T14:04:51.854Z to `resume-03/OWNER-PART-C-AUTHORISATION.txt` (sha256 eb9d9c66…5f62d). It is unsigned. The executor
  records it; no tool reads it as authority.
- The governing mission text `MISSION.txt` (EXEC-02, sha256 17adfc35…b561) is unchanged. Both authorisation gates authenticated
  that sha256 (AUTH-5.S01, AUTH-5.S02).

## 2. Reauthentication before execution

- [RAW] All 26 entries of `tools/TOOLS.sha256` were OK (`sha256sum -c`). The manifest's own sha256 is dc75fcb2…ce22, equal to the
  Owner's value. Stage gate T-1 re-verified the tools before every stage, and Stage 12 TOOL-1 verified them again at the end. No
  tool file changed.
- [GIT] Re-read from the object store before S02:
  - Candidate C d5964ee5 → tree 51bac5fd.
  - W-COMMIT 6c424c3a → W-TREE 5cc7ae59.
  - W-BLOB c18a6235.
  - K1 1918374263 (sha256 d6c05a7c…1a83).
  - K2 cb3a208e (sha256 989490ec…3f3b).
  - K4 commit 2148c5a5; K4 blob cfcbf998 (sha256 4b74b90a…92e2).
  - Linked evidence commit f023cc3d.
  - The mutant blob 0d4ab992 was absent.
- [RAW] Both gates S01 and S02 each passed 221 checks with 0 failures, including:
  - AU-1..AU-6, B-W/A, B-W/B, K1..K5, NI-1..NI-6 and SR-1..SR-6;
  - processes, ownership and resources.
- [RAW] Both gates derived the same EC7: c3aa3d90…3109.

## 3. S02 preflight (actual enforcement, not requested limits)

- [RAW] `gates/AUTH-S02.txt`: GATE RESULT PASS, 221 PASS / 0 FAIL.
  - RES-1: MemAvailable 51,639,980 KiB, at least the required 18 GiB.
  - RES-2: free 26.74 GiB. The whole projected growth of 7.71 GB still leaves 21.0 GB of headroom above the 16 GiB floor.
- [RAW] Supervisor self-test `gates/SUP-S02/SELFTEST.txt`: ALL OK, 8/8.
  - 16 GiB and zero-swap limits read back from inside the scope (`memory.max` 17179869184, `memory.swap.max` 0).
  - A 64 MiB cap enforced against a 256 MiB writer.
  - The `timeout` kill enforced, and the RuntimeMaxSec backstop enforced.
  - A normal exit 101 and a SIGABRT each passed through the run chain.
  - The disk-floor guard killed its own scope and flagged the breach.
  - The preflight disk gate refused projected growth beyond the headroom.
- [RAW] Isolation: every build, listing and run executed in its own transient `systemd-run --user --scope`, with
  MemoryMax=16G, MemorySwapMax=0, a RuntimeMaxSec backstop and an `env -i` environment.

## 4. Executed order and timestamps (AD-C7-W 13.3) — [RAW] `SEQUENCE.log` (sha256 9d795ce7…9652)

Each stage passed `final.py precondition` (the exact 13.3 sequence so far) and `stagegate.py` before it ran. No stage ran twice and
nothing was retried.

| Stage | GATE (UTC) | END (UTC) | rc |
|---|---|---|---|
| S01 authorisation gate (Part A) | 13:47:46.526 | 13:47:48.520 | 0 |
| S02 preflight repeat | 14:04:59.419 | 14:05:08.593 | 0 |
| S03a create healthy worktree | 14:05:17.723 | 14:05:18.633 | 0 |
| S03b build E-W (once) | 14:05:24.063 | 14:08:13.734 | 0 |
| S03c pin E-W | 14:08:26.118 | 14:08:26.685 | 0 |
| S03d eight E-W listings | 14:08:35.090 | 14:08:42.143 | 0 |
| S03e Stage-3 hard order gate | 14:08:57.348 | 14:08:57.518 | 0 |
| S04a create mutant worktree | 14:09:01.926 | 14:09:02.935 | 0 |
| S04b apply C7 (once) | 14:09:08.467 | 14:09:09.545 | 0 |
| S04c build E-W7 (once) | 14:09:14.734 | 14:12:04.099 | 0 |
| S04d pin E-W7 | 14:12:12.076 | 14:12:12.491 | 0 |
| S04e eight E-W7 listings | 14:12:16.220 | 14:12:21.057 | 0 |
| S05 NI-7, NI-8, B-W/C | 14:12:31.957 | 14:12:32.133 | 0 |
| S06 (C) healthy witness, E-W | 14:12:40.032 | 14:12:41.304 | 0 |
| S07 (A) original F-PT, E-W7 | 14:12:56.812 | 14:13:03.792 | 0 |
| S08 (B) mutant witness, E-W7 | 14:13:20.113 | 14:13:21.096 | 0 |
| S09 informational F-3, E-W7 | 14:13:41.512 | 14:13:48.480 | 0 |
| S10 restoration EXACTLY_W | 14:13:59.021 | 14:14:00.432 | 0 |
| S11 Candidate C link | 14:14:15.048 | 14:14:15.241 | 0 |
| S12 Stage 12 final | 14:14:21.696 | 14:14:22.793 | 0 |

Note: a run's own rc is recorded in its receipt. The stage rc is the rc of the runner plus the classifier.

## 5. Sources and executables

- [RAW] Healthy worktree `/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-healthy`:
  - detached at 6c424c3a, tree 5cc7ae59;
  - all 5364 tracked paths equal W-TREE;
  - `protocol.rs` a2df2841 / a5454944…a296.
- [RAW] Mutant worktree `/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-mutant`: created fresh at the same commit. The frozen patch
  `receipts/00-worktrees/patch/C7.patch` (sha256 b5d8bc6c…fbc4, equal to the catalog) was applied once, worktree only.
  - `protocol.rs` became 0d4ab9929c0e15d910735a52b72a5f3855bef58a / 6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa.
  - It was the only differing path.
  - The mutant blob was never written to the object store.
- [RAW] E-W: `…c7w02-healthy/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256
  e8b4e986b36d31c313d307991c51b3a7f8b3cc169a39afc459ee49974313b843, 8,995,112 bytes.
- [RAW] E-W7: `…c7w02-mutant/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256
  156cf0cc27ac1ed50d31e3a2a2adc924211da2a2be77bc6f617f774f27e04a38, 8,995,176 bytes.
- [RAW] 0A inequalities hold: E-W ≠ E-W7 ≠ E-C (cde9f9d2…), and E-W7 ≠ the historical C7 executable (b320977a). This is EXE-2.

## 6. Builds (7.1) — [RAW] `receipts/10-build-E-W`, `receipts/11-build-E-W7`

Common to both builds:
- **Command:** `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`.
- **Environment:** exactly HOME, PATH, `CARGO_NET_OFFLINE=true` and the worktree-local `CARGO_TARGET_DIR`.
- **Toolchain:** rustc 1.94.0 and cargo 1.94.0, equal to E-C's build record (NI-6d/e).

| | E-W | E-W7 |
|---|---|---|
| Wall time | 167.44 s | 167.92 s |
| memory.peak | 2,616,926,208 B | 2,617,655,296 B |
| memory.swap.peak | 0 | 0 |
| oom / oom_kill | 0 / 0 | 0 / 0 |
| Warnings / errors | 0 / 0 | 0 / 0 |
| Minimum free disk | 27,802,275,840 B | 26,916,184,064 B |
| build.stderr sha256 | 463e2475…e40e | b32ac97b…4542 |

Exactly one build per source identity, each into its own fresh target. No rebuild.

## 7. Listings (6A, NI-8) — [RAW] `receipts/20-list-E-W/L1..L8`, `receipts/21-list-E-W7/L1..L8`

- L1 `--list` gave 349 tests (raw 4aec6077…).
- L2 `--list --ignored` gave 35 tests, byte-identical to E-C's `21-list-ignored` (4b13105f…ae1e).
- L3, L4 and L5 (`--exact --list` of the witness, `full_fpt` and `full_f3`) each resolved exactly one test.
- L6 (`--exact --ignored --list` of the witness) selected 0 tests: the witness is not ignored.
- L7 and L8 (`--exact --ignored --list` of `full_fpt` and `full_f3`) each resolved exactly one test.
- All 16 listings ran supervised. Each executable kept its pinned sha256 before and after every listing.
- The E-W7 listings are byte-identical to the E-W listings, all eight of them.
- [RAW] The Stage-3 hard order gate `receipts/25-stage3-gate/STAGE3-E-W.txt` judged E-W and its eight listings admissible before
  the mutant worktree existed (S3-0, S3-ORD).

## 8. B-W/C — [RAW] `receipts/30-bwc/B-W-C.txt` (297 PASS, 0 FAIL); JSON sha256 7fe82cbe…85c4

`B-W/C: MECHANICALLY SATISFIED (NI-6 build-time clause, NI-7, NI-8; four-way agreement holds). SUBJECT TO INDEPENDENT ARCHITECT AUDIT.`

The four-way agreement was {source 349/35, E-W 349/35, E-W7 349/35, expected 349/35}, with 348 filtered out. ORD-1..ORD-3 confirm
this strict order:
1. the E-W build;
2. the eight E-W listings;
3. the Stage-3 gate;
4. mutant creation;
5. the patch;
6. the E-W7 build;
7. the eight E-W7 listings.

No witness or explorer ran before B-W/C.

## 9. Runs

All runs used the 7.2 environment (`env -i PATH=/usr/bin:/bin LANG=C.UTF-8 RUST_BACKTRACE=0`), the crate cwd, the 2700 s ceiling,
16 GiB and zero swap, one invocation at a time.

**(C) healthy witness on E-W** — [RAW] `receipts/40-run-C-healthy-witness` (raw 9df95b75…64d6). Classifier: 35 PASS, 0 FAIL.
- Exit 0, `1 passed; 348 filtered out`.
- 24 enabled steps, no violation.
- Healthy step-23 end (`NotApplied NotWritten Conflict(MissingPrepared)`, consuming 3 of 4 offered choices); predicate kept.
- Verdict PASS_ADMISSIBLE. memory.peak 3,772,416 B, swap 0, oom 0.

**(A) original F-PT on E-W7** — [RAW] `receipts/41-run-A-full_fpt` (raw 83407d51…9137). Classifier: 27 PASS, 0 FAIL.
- Explorer `EXPLORATION F-PT`, no quotient or symmetry line, A4 true, caps 30,000,000 states and 2700 s.
- 1,071,165 states (expanded 686,654), 3,864,850 transitions, 4.68 s.
- `FAIL: E11: a table entry left in a step that did not see its explanation in the ledger`; failed set {"E11"}.
- Normal exit 101 from the assertion panic (7.3: not an external kill). No incomplete reason.
- Verdict FAIL_E11_ADMISSIBLE (C7-1). memory.peak 136,519,680 B, swap 0, oom 0.

**(B) mutant deterministic witness on E-W7** — [RAW] `receipts/42-run-B-mutant-witness` (raw 4c30badd…6a16). Classifier: 48 PASS,
0 FAIL.
- Exactly two canonical records, in order:
  - `VIOLATION step 13 transition: E11: …`
  - `VIOLATION after 24: P6: F1 was refused as a conflict and is Paid`
- Steps 1–22 and 24 equal Table A in actor, offered and consumed (G9-1). This is the Oracle accounting.
- Step 23 is the planned changed end: `append_facts: Committed (inserted)`, consuming all 4 offered choices.
- The panic is the witness's own verdict panic at `model_tests.rs:9705:9` (C7-11).
- The summary and the M2, M3 and M4 messages each equal the text recomputed from the raw output (C7-9a, C7-10b..d). M4 is
  `["E11"]`; E11 is never counted as P6.
- Verdict ADMISSIBLE_TWO_CANONICAL.

**Informational F-3 on E-W7** — [RAW] `receipts/43-run-F3-informational` (raw 5a4a041a…1c9a). Classifier: 23 PASS, 0 FAIL.
- A4 false, 30M cap, 930,702 states, 4.25 s, FAIL E11, normal exit 101.
- Recorded as `INFORMATIONAL F-3: FAIL E11 (as expected)`: section 10 only, not a kill receipt and not a G.7 receipt.

## 10. Restoration — [RAW] `receipts/50-restoration`

- Evidence was preserved before the worktree changed (`pre-restore/`: HEAD, status `--ignored`, the worktree-list entry, the diff,
  `diff --cached` (empty), the mutant identity and ownership).
- R-1..R-8 all PASS. `git apply -R --check` returned rc 0, then exactly one `git apply -R` returned rc 0.
- Afterwards the mutant worktree is EXACTLY_W: `protocol.rs` a2df2841 / a5454944, no tracked difference, only `!! target/`, and
  the mutant blob is absent.
- E-W7's sha256 after restoration is 156cf0cc…, equal to its pin. The healthy worktree is EXACTLY_W.
- No reset, checkout, clean, stash, worktree removal or hand edit was used.

## 11. Candidate C link (D) — [RAW] `receipts/60-link/LINK.txt` (53 PASS, 0 FAIL); JSON sha256 f77193f0…bdde

`(D) LINK USABLE — C-BASE PASS on E-C; NOT W-SRC PASS`.

- The link uses the accepted evidence commit f023cc3d031a2a89e5ecbd4086e15f2e50869acd and E-C
  cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c.
- 44-full-F-PT raw: 0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8.
- The 43-full-F-3 comparison raw: 6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562.
- Candidate C F-PT was not rerun, and it is not claimed as a W-SRC measured PASS.

## 12. Stage 12 — [RAW] `receipts/70-final/FINAL-CLASSIFICATION.txt` (61 PASS, 0 FAIL); JSON sha256 05153f00…c7cc5

Every section passed:
- IN: 174 required files present.
- SEQ-1..4.
- AUTH-1..7: both gates; K1..K5; EC7; the mission sha; git objects re-read.
- BWC-1..5: re-judged in memory.
- RUN-*: the four runs re-classified from raw output, equal to the stored verdicts, raw sha256 intact.
- EXE-1..3.
- G9X-0..3: G.9 rules 1–4 compared across (C) and (B).
- ATT-1..3: E11 stays E11; P6 comes only from (B).
- RES-1: all 22 supervised processes.
- RST-1..5.
- LNK-1..5.
- OBJ-1..5: live state.
- TOOL-1.

`STAGE 12 RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`.

## 13. Resources (all 22 supervised processes) — [RAW]

- 16 GiB cap and zero swap held throughout: memory.swap.peak 0 everywhere, oom 0 and oom_kill 0.
- No timeout and no external kill.
- The disk floor was never breached. The minimum sampled free space was 26.92 GB, during the E-W7 build.
- Peak memory was 2.62 GB, during the builds.

## 14. Part A references (unchanged by Part C)

- Tool repair diff and lineage: `tools/TOOLS-PROVENANCE.txt` and `resume-03/provenance/`.
  - `final.py`: incomplete EXEC-02 snapshot 492aa1c5… → frozen 2289a164….
  - New files: `final_selftest.py` d42871a9… and `cmd/run-stage.sh` 2a3e8cd7….
  - No other tool changed.
- Coverage matrix: `resume-03/STAGE12-COVERAGE-MATRIX.md` (sha256 0ca17de8…).
- Verifier tests: `resume-03/selftests/`.
  - c7class 86/86, c7link 14/14, c7wt 31/31, c7bwc-no-cargo 17/17, c7sup 8/8, final+stagegate 64/64.
  - Omitted (they need cargo builds): `selftest_sup.py` and the c7bwc cargo self-test. They are recorded as omitted, never as
    passed.
- Part A manifest: `resume-03/MANIFEST-PART-A.sha256` (sha256 a9e843d3…).

## 15. Deviations, uncertainties, observations — [EXEC]

1. **Stale process (pre-existing, not touched).** PID 2306315 has run since 2026-10-13. It is a bash wait loop from an earlier
   Claude session that polls with `pgrep` for a `full_f3` process of mission f3-cap60-01. It matches its own argv, so it never
   exits. It is bash plus sleep, not a cargo, rustc, test or runner executable, and PROC-1/P-1 judged it non-conflicting at every
   gate. Killing it was outside scope.
2. **Two cargo-dependent self-tests were not run**, as Part A already recorded. Supervision was instead demonstrated by c7sup 8/8
   in S02, and by the 22 real supervised receipts.
3. **The CARGO_TERM_VERBOSE limitation** recorded in B-W/C: E-C's build environment carried CARGO_TERM_VERBOSE=true (output only).
   Section 7.1 does not list it, so this mission omitted it.
4. **Runtime bytecode not in the packet.** `tools/c7tools/__pycache__/` holds bytecode written during EXEC-02 before the freeze.
   It is not a frozen tool and is excluded from the publication packet and its manifest. The run-stage runner disables bytecode
   writing.
5. **Files added after Part A.** The Owner Part C authorisation transcript, `FINAL-STATE.txt`, this report and `MANIFEST.sha256`
   were added to the evidence root after Part A. Each was created new. No existing receipt, gate record or tool was edited.

## 16. Architect decisions pending — [ARCH]

- G.7(E): the independent Architect review.
- G.7(F): any recording.
- Acceptance or rejection of C7, and any campaign count change (13.5).
- The open items listed in AD-C7-W 13.3/13.5 and Annex B.

## 17. Manifest and publication

- `MANIFEST.sha256` is self-excluding and covers every file of this packet.
- Publication is limited to one evidence-only commit on parent 2148c5a524b6979dde2eb63b6d1442e39dcfb960 and one push to
  `refs/heads/review/p3-c11-d2a1-c7-compliant-exec-02`. It requires the Owner's genuine native Manual permission approval. The
  push receipt and remote read-back are recorded locally beside this packet, not inside it.
- No GitHub CI dispatch, PR, merge, integration or protected-ref change.
