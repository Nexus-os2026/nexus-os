# Stage 12 contract-to-checker coverage matrix — AD-C7-W (K2 blob cb3a208e, sha256 989490ec…3f3b)

Mission P3-C11-DR01-D2A1-C7-TOOL-REPAIR-RESUME-03, Part A2. Prepared by Claude Code (claude-opus-5-5) from the full text of K2,
read from the git object (1,419 lines; sha256 recomputed equal). No requirement is added here; each row cites the clause it comes
from. EXECUTOR ANALYSIS — subject to Architect review.

**Status codes.** **1** = checked by an existing EXEC-01/EXEC-02 checker (unchanged in RESUME-03). **2** = simple integration
added in RESUME-03 (final.py re-runs the existing checker in memory, or adds a cross-receipt/live check; or cmd/run-stage.sh
enforces it). **3** = missing contractual coverage. **A** = an Architect decision or obligation; not executor-checkable by
design (reported as ARCHITECT DECISION PENDING; never inferred). No row is status 3.

**Where results land.** Stage 1-2: `gates/AUTH-S01.txt`, `gates/AUTH-S02.txt` (gate.py). Each stage: `gates/<stage>.txt`
(stagegate.py) and `SEQUENCE.log`. Builds/listings: `receipts/10-*`, `11-*`, `20-*`, `21-*`; `12-pin/`; `25-stage3-gate/`;
`30-bwc/B-W-C.{txt,json}` (c7sup, c7bwc). Runs: `receipts/4x-*/CLASSIFICATION.{txt,json}` (c7class). Restoration:
`receipts/50-restoration/` (c7wt). Link: `receipts/60-link/LINK.{txt,json}` (c7link). Stage 12:
`receipts/70-final/FINAL-CLASSIFICATION.{txt,json}` (final.py; check ids below).

**Test-name keys.** `cls:` = c7class self-test case (resume-03/selftests/c7class_selftest.txt, 86/86). `lnk:` = c7link
(14/14). `wt:` = c7wt (31/31). `bwc:` = st_bwc_nocargo (17/17). `sup:` = c7sup supervision self-test (8/8). `fin:` =
final_selftest (64/64; `fin:POS` = the complete positive tree; `fin:N<k>` = negative case k).

## G.7 — the kill criterion (section 3)

| # | Obligation (clause) | Raw input | Checker → predicate | Positive | Negative | Result | St |
|---|---|---|---|---|---|---|---|
| A-1 | (A) type (a): `full_fpt --exact --ignored`, one match, on E-W7 (G.7(A); 7.2; C7-4) | 41 descriptor, raw output; 12-pin | c7class ID-1..ID-6 (path, sha, argv, env, cwd); final RUN-A-1/3, EXE-1 | cls:(A) FAIL E11 admissible; fin:POS | cls:(A) on the E-W executable; fin:N18 | 41/CLASSIFICATION; 70-final RUN-A-*, EXE-1 | 1+2 |
| A-2 | original explorer: `EXPLORATION F-PT`, no quotient/symmetry, F-PT bounds with A4 true (G.7(A)) | raw output; frozen bounds/actors lines read from git (44-full-F-PT, 85-C7) | c7class EX-2, EX-2b, EX-3, STR | cls:(A) positive | cls:(A) header quotient; (A) F-3 bounds; (A) bounds differ | 41/CLASSIFICATION | 1 |
| A-3 | within 30,000,000 discovered states; admissible under K.2/K.2-C7 (G.7(A); K.1) | raw `states` line; supervisor record | c7class EX-4, CL1-1..CL1-8 | cls:positive | cls:(A) states beyond the ceiling; (A) timeout; (A) disk breach; fin:N10a | 41/CLASSIFICATION; RES-1 | 1+2 |
| A-4 | `running 1 test`, `348 filtered out` (G.7(A); 6A) | raw harness lines | c7class STR (RE_RESULT_FAIL), REG | cls:positive | cls:(A) with `347 filtered out` | 41/CLASSIFICATION | 1 |
| A-5 | FAIL E11 by a normal exit 101 (G.7(A); C7-1) | process record; raw | c7class C7-1a/b, CL1-1, F3-style two-instance layout | cls:positive | cls:(A) one instance; three instances; exit 0 with FAIL | 41/CLASSIFICATION; ATT-1 | 1+2 |
| A-6 | `outcome`, `FAIL: E11`, `states`, trace kept unedited (G.7(A)) | raw output, descriptor sha256 | c7class ID-7; final RUN-A-2 (raw sha now = descriptor) | fin:POS | fin:N2 (raw altered) | RUN-A-2 | 1+2 |
| A-7 | E11 never relabelled or counted as P6; P6 → C7-2 stop; other property/PASS/INCOMPLETE/beyond ceiling → C7-3 stop (G.7(A); 8.1; S-C7-18) | raw FAIL line | c7class C7-2, C7-3 verdicts; final ATT-1, RUN-A-3 | fin:POS | cls:(A) PASS/INCOMPLETE; fin:N6a (P6), N15 (INCOMPLETE), N16 (E4) | ATT-1, RUN-A-3 | 1+2 |
| B-1 | (B) witness test `--exact`, one match, on THE SAME E-W7 as (A), sha256 equal before/after (G.7(B); 7.4) | 42 descriptor; pins | c7class ID-1..ID-4; final EXE-1 (one path/sha for A, B, F-3) | fin:POS | fin:N18 | EXE-1 | 1+2 |
| B-2 | `1 failed`, `348 filtered out`, exit 101 normal (G.7(B)) | raw tail; process | c7class C7-17b, REG-4, CL1-1 | cls:(B) positive | cls (B) region cases | 42/CLASSIFICATION | 1 |
| B-3 | exactly two canonical records: E11 at step-13 transition, P6 after 24; no other (G.7(B); 4.7; CL-4/5; C7-5/C7-8) | REPLAY records | c7class C7-5, C7-6, C7-7, C7-8, C7-8b/c; final ATT-2 | cls:(B) positive; fin:POS | cls:(B) third record, duplicates, misplaced; fin:N6b | 42/CLASSIFICATION; ATT-2 | 1+2 |
| B-4 | panic summary is an exact copy, never counted (CL-6) | SUMMARY region | c7class C7-9a | cls:positive | cls:(B) summary altered | 42/CLASSIFICATION | 1 |
| B-5 | M1 n = 2; M2..M4 agree with records (CL-7; C7-9, C7-10) | MARKERS | c7class C7-9b, C7-10a..e, C7-5b | cls:positive | cls:(B) marker cases | 42/CLASSIFICATION | 1 |
| B-6 | G.9 holds (G.7(B)) | per-step lines | see G.9 rows | | | | 1+2 |
| C-1 | (C) same test on E-W; `ok`, `1 passed; 0 failed`, 348, exit 0 (G.7(C); 5; C7-12) | 40 descriptor/raw | c7class ID-*, C7-12, REG-3/4; final RUN-C-3, EXE-1 | cls:(C) positive; fin:POS | cls:(C) exit 101; fin:N14 | RUN-C-3 | 1+2 |
| C-2 | H1 exactly; no VIOLATION; no positive marker (G.7(C); 5; C7-13/14) | REPLAY | c7class RP-1, C7-13, M-C | cls:positive | cls:(C) without H1, H1 twice, violation, fixture failure | 40/CLASSIFICATION | 1 |
| C-3 | step 23 healthy end; kept predicate (G.7(C); 4.5; CL-8/9) | state lines | c7class CL8-C, CL9-C; final ATT-3, G9X-3 | fin:POS | cls:(C) changed end; table missing; fin:N14 | ATT-3 | 1+2 |
| D-1 | (D) link 44-full-F-PT at f023cc3d (type e1) under I-C7.7, never rerun (G.7(D)) | git objects at f023cc3d | c7link LK-1..LK-10; final LNK-1..LNK-5 (re-link in memory; frozen raw 0863794e, E-C cde9f9d2, 43-full-F-3 raw 6dfc796a) | lnk:all faithful; fin:POS | lnk:tamper cases; fin:N9a, N9b, N24 | 60-link; LNK-* | 1+2 |
| D-2 | (D) only when B-W/A, B-W/B and B-W/C all hold for the one W-SRC (G.7(D); 6.3) | K4 record (git); mission text; B-W-C.json | gate BWA, BWB; c7link LK-0a..LK-0d, LK-11; final LNK-2, BWC-1/2 | lnk:positive; fin:POS | lnk:B-W/C not satisfied, no B-W/B text, before restoration; fin:N20, N12c | LNK-2, BWC-* | 1+2 |
| E-1 | EC7 derived and matched (G.7(E); 2A; K3-X; CE-1; P-1..P-6) | K1, K2 git objects | gate K3-X1..3, K3, OVL-1, P-*, CE-1, 2A.4-5, EC7-fields; final AUTH-2/3/4/6 | gate dry-run 3 (221 PASS) | fin:N21 (gate with a FAIL) | AUTH-*.S01/S02 | 1+2 |
| E-2 | I-C7.1..I-C7.8 hold (G.7(E); 6.1) | git objects; worktrees | gate W-1..W-9, NI-*, PAT-*; c7wt verify; final AUTH-7, RST-5, OBJ-1..3, EXE-2 | fin:POS | wt:negatives; fin:N8d, N17 | AUTH-7, OBJ-*, EXE-* | 1+2 |
| E-3 | NI-1..NI-6, SR-1..SR-6 hold by B-W/B (G.7(E); 6.2) | W-COMMIT blobs | gate NI-1a/b, NI-2, NI-3a/b, NI-4, NI-5*, NI-6*, SR-1m..SR-6m (executor mechanical evidence only); final AUTH-2 requires each PASS | gate dry-run 3 | gate-dryrun1 (SR-3m FAIL) would fail AUTH-1/2 (fin: consistency case) | AUTH-2.* | 1+2 (+A: the SR approval itself is B-W/B, conveyed) |
| E-4 | NI-7, NI-8 and 6A hold by B-W/C (G.7(E); 6.3) | build/listing receipts | c7bwc check (NI-6a..e, NI-7a..d, NI-8.*, AGREE-1..3, ORD-1..3); final BWC-1..BWC-5 | fin:POS | bwc:parser/ORD/PROC cases; fin:N3, N4, N5a, N5b, N10b/c, N19 | BWC-* | 1+2 |
| E-5 | restoration EXACTLY_W, only as 7.3A allows after a stop (G.7(E); I-C7.6) | 50-restoration; worktrees | c7wt restore (R-1..R-8, one `git apply -R`, verify W); runner S10; final RST-1..RST-5, OBJ-1 | wt:restoration completes; fin:POS | wt:hand edit / R-3 / R-4 / staged; fin:N8a..N8d, N25, N26 | RST-*, OBJ-1 | 1+2 |
| E-6 | (A), (B) and F-3 used one E-W7 (G.7(E); 7.4; C7-16) | 41/42/43 descriptors; pin | final EXE-1; c7class ID-2 | fin:POS | fin:N18 | EXE-1 | 2 |
| E-7 | F-3 has a recorded outcome under section 10 (G.7(E)) | 43 raw | c7class F3; final F3 section and result labels | fin:F-3 PASS flagged | fin:F-3 signal / timeout | 70-final informational_f3 | 1+2 |
| E-8 | the Sole Architect's independent review passes (G.7(E)) | — | not executor-checkable | — | — | `architect_decision_pending` | A |
| F | recording 'C7: P6 (first failure E11; witness)' (G.7(F)) | — | never written by any tool | — | — | — | A |

## G.8 / G.9 — witness form and the offered-versus-consumed rule (section 4)

| # | Obligation | Checker → predicate | Positive | Negative | Result | St |
|---|---|---|---|---|---|---|
| G8-1..8, 11, 12 | witness source form (real `step`, no forced state, take_counts, unaltered predicates, SR-1 split, unchanged Oracle/Io, collect every violation, print divergence) | B-W/B (Architect source review, conveyed); gate SR-1m..SR-6m, NI-5* as executor evidence | gate dry-run 3 | gate-dryrun1 | AUTH-2 | 1 (+A) |
| G8-6 | every step asserted enabled; 24 steps | c7class RP-2, RP-6 (a non-enabled step is a fixture failure in the raw output) | cls:positive | cls:23 step blocks; fixture failure | RUN-* | 1 |
| G8-9/10 | required loci only; healthy no violation, healthy end, kept predicate | rows B-3, C-2, C-3 | | | | 1+2 |
| G9-1 | same actor sequence and offered stream on both cores | c7class G9-xx per receipt against Table A; final G9X-1 across (C) and (B) | fin:POS | cls:(C) offered changed; fin:N7a | G9X-1 | 1+2 |
| G9-2 | steps 1-22 frozen consumption, no unused suffix, frozen branches and fault budget, identical on both cores | c7class G9-1; final G9X-2 | fin:POS | cls:fault flag missing; fin:N7a | G9X-2 | 1+2 |
| G9-3 | step 23 exactly one of two planned ends; healthy unused [0] | c7class G9-23, CL8-*; final G9X-3 | fin:POS | fin:N7b | G9X-3 | 1+2 |
| G9-4 | step 24 consumes nothing and pairs with step 23 | c7class G9-24 | cls:positive | cls:(C) lost step 24 | RUN-* | 1 |
| G9-5 | consumed = offered prefix; no consumption beyond offered; no other unused suffix | c7class G9-5.xx | cls:positive | cls cases | RUN-* | 1 |
| G9-6 | verified twice: by the witness and by the classifier | witness output + c7class | | | | 1 |

## Section 6A — test inventory

| # | Obligation | Checker → predicate | Positive | Negative | Result | St |
|---|---|---|---|---|---|---|
| 6A-1 | 349 total / 35 ignored in W-TREE source, E-W listing, E-W7 listing and the frozen expectation (four-way) | gate CNT; c7bwc NI-8.1..8.5, AGREE-2, S3-INV; final BWC-4 (stored four-way equals the frozen 349/35 expectation) | fin:POS | fin:N5a, N5b | BWC-1, BWC-4 | 1+2 |
| 6A-2 | listing = E-C's 348 lines + the witness line; ignored listing byte-identical to 21-list-ignored (4b13105f) | c7bwc NI-8.2, NI-8.3, NI-8.4 | fin:POS | fin:N5a | BWC-1 | 1 |
| 6A-3 | E-W7's listings equal E-W's | c7bwc AGREE-1.* | fin:POS | bwc (scratch-crate variant omitted: cargo) | BWC-1 | 1 |
| 6A-4 | each `--exact --list` one line; `--exact --ignored --list` only for full_fpt/full_f3 | c7bwc NI-8.6.*, NI-8.7.* | fin:POS | fin:N4 | BWC-1 | 1 |
| 6A-5 | all eight listings exist, run one at a time, executable unchanged around them | c7bwc NI-8.L*, LST-1..5, NI-8.0, S3-ORD; final IN-1, SEQ-3 | fin:POS | fin:N4 | IN-1, BWC-1, SEQ-3 | 1+2 |
| 6A-6 | single-test invocations print `running 1 test` and `348 filtered out` | c7class STR/REG; c7bwc AGREE-3 | cls:positive | cls:347 filtered | RUN-* | 1 |

## Section 7 — execution policy, limits, stop rules

| # | Obligation | Checker → predicate | Positive | Negative | Result | St |
|---|---|---|---|---|---|---|
| 7.1 | build command/env/offline/target, -j 2, 16 GiB, zero swap, 900 s, zero warnings/errors, one build per identity | c7sup build; c7bwc NI-6a..e, NI-7a..d, BLD-1..4, PROC-1..8; runner (one S03b and one S04c, exclusive gate records); final RES-1, SEQ-1 | sup:1,5; fin:POS | sup:2,3,4,7,8; fin:N10b | BWC-1, RES-1 | 1+2 |
| 7.2 | run environment `env -i PATH LANG RUST_BACKTRACE=0`, 16 GiB, zero swap, 2700 s, one invocation at a time, exact commands | c7sup invoke; c7class ID-4, ID-5, CL1-*; c7bwc LST-1; runner sequence | cls:positive | cls:timeout/oom/signal | RUN-*, RES-1 | 1+2 |
| 7.3 | STOP on kill/signal/OOM, swap, timeout, warning, mismatch, (C) not PASS, (A) not E11, (B) inadmissible, restoration not exact; no retry | runner: a stage with rc ≠ 0 or a failed gate blocks every later stage except S10 on the stop path (final.py precondition); stagegate exclusivity; c7class verdicts | fin:precondition allows | fin:precondition stop cases | console/, SEQUENCE.log | 2 |
| 7.3A | preserve first; R-1..R-8; one inverse patch within 60 s; preservation outcome | c7wt preserve/restore/auth_r; runner S10 (stop path allowed only after apply-C7.json exists); final RST-1..RST-2 | wt:R-1..R-8 hold; restoration completes | wt:hand edit, no creation record, staged, second path; fin:N8c | 50-restoration; RST-* | 1+2 |
| 7.4 | one E-W7: sha at pin, around listings, before/after (A), (B), F-3, after restoration; no cargo after pin | c7bwc PIN-1, LST-2, NI-8.0; c7class ID-2; runner S10 re-hash; final EXE-1, EXE-3 (sha, size, inode, mtime unchanged since the pin), RST-3 | fin:POS | fin:N17, N26 | EXE-*, RST-3 | 1+2 |
| 7.5 | the 34 accepted controls and A4 untouched | by construction: the runner addresses only the two c7w02 worktrees and this evidence root; gate HIST-1..HIST-5 at Stage 1-2; final OBJ-5 (historical C7 worktree locked at C-BASE; EXEC-01 evidence verifies) | gate dry-run 3; fin:POS | — | AUTH-2, OBJ-5 | 1+2 (process obligation; the 34 controls' locked worktrees are not enumerated one by one at Stage 12 — no clause asks for it) |

## Section 8 — K.2-C7

| # | Obligation | Checker | St |
|---|---|---|---|
| C7-1..C7-18, C7-21 | case table 8.1 | c7class (verdicts STOP_C7-2_P6, STOP_C7-3, NOT_ADMISSIBLE, etc.; C7-21 via ID-3 against b320977a); final RUN-*, ATT-* re-run in memory | 1+2 |
| C7-19 | EC7/overlay/AU-1..AU-6/B-W/B/B-W/A → STOPPED BEFORE EXECUTION | gate.py (Stage 1-2, twice: S01 and S02); runner (no stage after a failed S01/S02); final AUTH-1..AUTH-7 | 1+2 |
| C7-20 | F-3 is never a kill or G.7 receipt | c7class F3 verdicts `COMPLETED: …`; final never counts F-3 toward G.7 (result label rules) | 1+2 |
| CL-1..CL-12 | independent classifier checks; never from exit status alone | c7class (reads raw and supervision records only); final re-classifies in memory and requires equality with the stored classification (RUN-x-1) | 1+2 |

## Section 10 — informational F-3

| # | Obligation | Checker | Positive | Negative | St |
|---|---|---|---|---|---|
| 10-1 | runs once, after (C) PASS, (A) admissible E11, (B) admissible, before restoration | runner precondition (S09 only after S08 rc 0); final SEQ-1, SEQ-4 | fin:precondition | fin:N13, N27 | 2 |
| 10-2 | same E-W7, sha equal before/after and equal to (A)'s and (B)'s | c7class ID-2; final EXE-1 | fin:POS | fin:N18 | 1+2 |
| 10-3 | own receipt directory with descriptor, raw, supervision, CHECKS, CLASSIFICATION, and the INFORMATIONAL label | c7sup invoke + c7class classify (label); final IN-1 | fin:POS | fin:N1 analog | 1+2 |
| 10-4 | comparison side links 43-full-F-3 under the same B-W condition | c7link (43-full-F-3); final LNK-3 | lnk:positive | fin:N9b | 1+2 |
| 10-5 | outcome recording: FAIL E11 as expected; other FAIL/PASS/INCOMPLETE recorded and flagged; NOT COMPLETED → MUTATION_CONTROL_BLOCKED — INFORMATIONAL F-3 NOT COMPLETED | c7class F3-1..F3-4 texts; final flags and labels | cls:F-3 cases; fin:F-3 PASS flagged | cls:F3-4 cases; fin:F-3 signal/timeout; F-3 + other defect | 1+2 |

## Section 13.3 — order (the C7 order, not EXEC-01's)

| # | Obligation | Checker | Positive | Negative | St |
|---|---|---|---|---|---|
| 13.3-1/2 | authorisation gate and W-SRC authentication before any worktree/build | gate.py at S01 and again at S02 (DEST/DEST-EV: receipts empty, worktrees absent); final AUTH-*, SEQ-3 (gate times inside S01/S02) | gate dry-run 3; fin:POS | fin:N21 | 1+2 |
| 13.3-3 | healthy build, pin, eight listings, Stage-3 hard order gate, all before the mutant worktree exists | c7bwc stage3 (S3-0, S3-0b, S3-ORD) and ORD-1..3; runner (S04a only after S03e rc 0); final BWC-5, SEQ-1, SEQ-3 | bwc:compliant order; fin:POS | bwc:EXEC-01 order, gate late; fin:N3, N19 | 1+2 |
| 13.3-4 | mutant worktree, one patch application, W7-SRC by blob, one build, pin, eight listings | c7wt create/apply (EXACTLY_W7); c7bwc; final RST-5, SEQ-* | wt:apply; fin:POS | wt:negatives | 1+2 |
| 13.3-5/6 | NI-7/NI-8 then B-W/C record (O-6) before any run | c7bwc check (O-6 text); runner (S06 only after S05 rc 0); final SEQ-4 (B-W/C END < (C)) | fin:POS | fin:N13 | 1+2 |
| 13.3-7..10 | (C), then (A), then (B), then F-3 | runner preconditions; final SEQ-3, SEQ-4 | fin:POS | fin:N13, N27 | 2 |
| 13.3-11 | restore EXACTLY_W | row E-5 | | | 1+2 |
| 13.3-12 | (D) link citing B-W/C, after restoration | c7link LK-11, LK-0a; runner (S11 only on the clean sequence); final LNK-*, SEQ-4 | lnk:positive | lnk:before restoration; fin:precondition (S11 refused after a stop) | 1+2 |
| 13.3-13 | independent classification (8.2) | final.py (this matrix) | fin:POS | fin:N1..N27 | 2 |

## NI-1..NI-8, SR-1..SR-6, CR-B (section 6.2, 6.3)

| # | Checker | Stage-12 check | St |
|---|---|---|---|
| NI-1 | gate NI-1a, NI-1b (byte prefix); c7link LK-8d | AUTH-2, LNK-2 | 1+2 |
| NI-2 | gate NI-2, W-7 | AUTH-2 | 1+2 |
| NI-3 | gate NI-3a, NI-3b | AUTH-2 | 1+2 |
| NI-4 | gate NI-4 | AUTH-2 | 1+2 |
| NI-5 (a)..(h) | gate NI-5a1..a3, b, b2, c, c2, d, f, f2, g1..g4, h, -uses2, S-counts | AUTH-2 | 1+2 |
| NI-6 (blobs) | gate NI-6, NI-6p, NI-6u, NI-6t2 | AUTH-2 | 1+2 |
| NI-6 (build-time clause) | c7bwc NI-6a..e from the build receipts; gate ENV-2/ENV-3 | BWC-1 | 1+2 |
| NI-7 | c7bwc NI-7a..d | BWC-1 | 1+2 |
| NI-8 | c7bwc NI-8.*, AGREE-* | BWC-1, BWC-4 | 1+2 |
| SR-1..SR-6 | B-W/B (Architect source approval for this exact W-SRC, conveyed by the EXEC-02 mission); gate SR-1m..SR-6m executor mechanical evidence | AUTH-2 | 1 (+A) |
| CR-B / B-W/A | K4 record (git) and Owner decision: gate AU-3b, BWA; c7link LK-0c | AUTH-2, LNK-2 | 1+2 |
| B-W/B | gate BWB, W-9; c7link LK-0d | AUTH-2, LNK-2 | 1+2 |
| B-W/C (O-1..O-6) | c7bwc check (writes the O-6 record); c7link LK-0a | BWC-1..BWC-5, LNK-2 | 1+2 |

## Not Stage-12 inputs

- **Publication provenance** (mission section "PUBLICATION"): publication happens only after a successful Stage 12 and only with
  native Owner permission. It is not an input to the classification, so it is not a Stage-12 predicate. If publication is reached,
  it is verified at that point (destination absent, exact parent, evidence-only diff, workflow triggers, one push, read-back).
- **Section 1.4 (scoped first-failure masking)**, A-1..A-3, AS-1/AS-2: arguments and assumptions for the Architect; nothing
  executable (A).

## Equivalence statement (A3)

final.py does not replace any checker. It re-runs c7class (four runs), c7bwc.check (B-W/C, including its own 13.3 ORD checks),
c7link.link (the (D) link) and c7wt.verify (live worktrees) in memory and requires agreement with the stored receipts. It adds only
cross-receipt and live checks that no single checker can make (sequence windows, G.9 across cores, one E-W7, executable inode/mtime
since the pin, gate-receipt identities, frozen linked identities, tool manifest). Stage 12 was not removed.
