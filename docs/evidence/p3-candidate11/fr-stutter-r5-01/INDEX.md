# F-R stuttering refinement, R5 — index

Mission P3-C11-DR01-D2A1-FR-STUTTER-R5-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High.
For the Sole Architect's independent review.

**Status: READY_FOR_INDEPENDENT_FR_STUTTER_PROOF_AND_R5_REVIEW.**

**Read-only proof and documentation work.** Nothing was built, tested or run: no Cargo command, no test, no model
configuration, no quotient, no mutation, no witness. No source, Candidate C, executable or AD-1-R4 was changed. The
F-R receipt stays **INCOMPLETE** (30,000,001 discovered, 26,539,923 expanded at the 30,000,000-state cap). No
document here is accepted or adopted; AD-1-R4 governs. D2A-1 is not accepted and not integrated; Phase Three remains
active.

## Identities (`receipts/00-authentication.txt`, re-checked by the final gate)

| Item | Value |
|---|---|
| Candidate C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| Blobs | `protocol.rs` `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` `28085ce232827301104b5d757af042ec933976f8`; `unit_tests.rs` `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`; all 49 Rust inputs of the pinned executable equal C's blobs |
| Pinned executable (reference only) | sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c` |
| Stage-1 / Stage-2 evidence | `f023cc3d031a2a89e5ecbd4086e15f2e50869acd` (SHA256SUMS: 179 entries, every committed blob matches) / `89326b859792a442c1853c3d35d30a48301c009e` (268 entries, every blob matches; = GitHub `review/p3-c11-d2a1-final-baseline-remaining-configs-01`) |
| F-2 receipt | PASS, 12,602,670 states, 24 HOLD + E10 n/a, D1 HOLDS, L1 HOLDS; raw sha256 `985a3976…4ab5` |
| F-R receipt | INCOMPLETE; raw sha256 `78e44c33…8ccd` |
| Diagnostic | `e8_holds_for_stale_worker_operations ... ok` in the exact-final suite (313 passed, 0 failed) |
| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` (unchanged) |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`; C15 at line 713; F-R at line 851 |

## The documents

| File | Git blob | sha256 | Bytes |
|---|---|---|---|
| `FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md` — EXECUTOR ARGUMENT, NOT ARCHITECT ACCEPTED | `c631a061dd7206b9bc62822711be0eca2a21c223` | `c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b` | 57,318 |
| `P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md` — DRAFT, NOT ACCEPTED, NOT ADOPTED, NOT APPLIED | `ae3cabdb45627f9b3179a098c50d27e50c55101f` | `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` | 16,067 |
| `PROPOSED-20-CONFIGURATION-ACCOUNTING.md` — PROPOSED | `5ac925afc3998fc0f4e87751d29e3e118847ba85` | `3a3faf747a5e561946040d2981e3571f346a76d4967cb682c806f55bdfe79e68` | 16,424 |
| `C15-DISPOSITION.md` | `d54f477777a4a209207e4a2ae18aaddae091555f` | `cd889022c1196e5a69afa35c355d775f8e671b4f7e3d5bc569a54b2292da90b1` | 3,151 |
| `REPLAY-OCCURRENCE-MATRIX.tsv` (and `.md` summary) | `1f676ef6b21bb442cd63f0bb3566565b6cd4a2cb` | `a341dbf0d7a992c08c95363656f764f0a8b2df9c1fd7491bae6e119fe205e08f` | 94,764 |
| `SOURCE-ITEM-IDENTITY-MATRIX.tsv` (61 cited items) | `d86d91afd33c4b092b54d71712ccab41b01ed1e2` | `5386c33fe06157f5c3a13063f2b84b777c0ce0ead5077914a08cd7b8ea40e9c3` | 10,467 |
| `FR-BRANCH-DISPOSITION.tsv` (and its summary JSON) | `18ffb9a9c184dfe766fa905cac01cbdce128269a` | `23c3dad1dce7bdc935706de85449ca2f8b8be96731dbdf924283e3a45561f072` | 53,514 |

## Results

**Occurrence closure: REPLAY_OCCURRENCE_CLOSURE_COMPLETE.** 233 target occurrences (every `replays`, `Replay`,
`step_replay`, `last`, `.name`, `Action::<variant>`, and every word containing "replay") in 15 of the 49 Rust inputs,
each classified by exactly one of 179 rules; 21 semantic consumers. No property, terminality or branch-labelling
function and no ordinary step reads a replay value; `step_wk` writes `Wk.last` only when `config.replays > 0`.
Checker A (token lexer, item extractor, call closure) and Checker B (masking lexer, regex items, control-flow and shape
checks, 62 checks) were implemented separately and agree exactly (`receipts/10`–`13`).

**The proof (executor argument).** π erases `Wk.last` and the replay counter; on keys it replaces the `wk.last`
segment and the replay byte by single zero bytes. FR-1 to FR-17 are each argued and marked ESTABLISHED (executor
argument), with FR-RS (a REPLAY step is a stutter, given E1 and KEY at its source) and FR-17.X (REPLAY never runs an
AppendX). Safety, D1 and L1 of F-R's reachable graph are derived from the accepted exact-final F-2 PASS; no F-R
cardinality is claimed. 88 source citations, each resolved independently by both checkers.

**Branch accounting (complete, by proof).** 193 rows: 57 TAKEN, 106 NOT TAKEN, 30 NOT APPLICABLE, none NOT_ESTABLISHED.
F-2's 54 taken labels lift into F-R; REPLAY adds `resolve: Stale`, `append_cell C: Stale` and `empty: Stale` (taken in
the F-R prefix; never complete coverage); `append_cell X: Stale` is never taken (FR-17.X). The diagnostic exercised all
four stale operation classes on a forced state (supporting only).

**C15.** Unchanged: E8, F-R, the original explorer, 30,000,000; a kill needs an original-explorer FAIL of E8 within
the cap. Only its restored side may, under AD-1-R5, cite the refinement discharge after verified restoration.

**AD-1-R5 (draft).** Seven supersession rows, each scoped to F-R, `full_fr` or C15 only; the F-R receipt stays
INCOMPLETE; E.3 row 15's unchanged-core disposition replaced; eight prerequisites; recording language fixed; no other
configuration and no general permission.

**Twenty-configuration accounting (proposed).** Nineteen measured PASS (X-1 to X-5; F-1, F-2, F-4 to F-8, F-B2, F-G,
F-W, F-D original; F-3, F-PT, F-B composed-quotient replacing runs); F-R INCOMPLETE, proposed PROOF-DISCHARGED pending
R5 acceptance and adoption, with no total state or transition count.

## Freeze and final gate

- Frozen: 54 files (`FREEZE-RECORD.txt`; `receipts/40-FREEZE.json`, sha256
  `f3990fbc376fa58c5542e100b13266849b9a3ec69131e1fefa07bb0574be66a7`).
- Final gate, run once: **ALL CHECKS HOLD** (`receipts/60-final-gate.txt`, sha256
  `1e3557f8309ad81b9194e597447538178faeb5847d864717711b87f5d495375a`): frozen files unchanged; every tool re-ran from
  the frozen inputs and every output equals its frozen copy byte for byte; provenance; occurrence closure; both
  analyses agree; every citation resolves; FR-1 to FR-17 represented; no forbidden wording; the F-R receipt kept
  INCOMPLETE; C15 on the original explorer at 30,000,000; AD-1-R5 scoped; 25 of 25 controls (13 source defects seeded
  into both checkers, 10 document defects, 2 positive); AD-1-R4 unchanged; no tracked source changed; packet confined
  and clean.
- Development (before the freeze): ten repairs, all of executor tools or controls, none in Nexus
  (`development/DEVELOPMENT-LOG.txt`, with the failed outputs kept).

## Decisions for the Architect and the Owner

1. **D-1 — `name`.** FR and F2 differ in `replays` (permitted) and in the label `name` ("F-R", "F-2"), which the
   explorer reads only in its report header (Checker B B86). Whether that satisfies FR-1's "every other field
   mechanically equal" is the Architect's ruling; if not, the argument is blocked at FR-1.
2. **D-2** — independent review and acceptance of the proof, the occurrence closure and the branch accounting.
3. **D-3** — the Owner's adoption of AD-1-R5.
4. C15's own mission (mutant, original-explorer run under 30,000,000, verified restoration) needs its own
   authorisation.

## Limitations

- Every result is of the explored model at C, conditional on A-1 and A-2 (AD-1-R4 J), and on the compiled program
  implementing its source.
- The checkers' checks are syntactic (occurrences, shapes, citations, agreement); they support the semantic argument,
  which needs review. Self-delimitation of the key codec is argued from the encoder/decoder mirror; only the replay
  segments' positions and the action and token codecs are checked mechanically.
- The diagnostic runs on a forced state, and its label assertion is vacuous on an empty label list (non-emptiness
  follows from the source).

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. With the repository at hand, the frozen tools re-run read-only:
`python3 tools/prepare.py <new-dir>` reproduces every receipt and document byte for byte (it reads git objects only and
runs nothing of Nexus); `tools/checker_a.py`, `tools/checker_b.py`, `tools/agree.py`, `tools/doc_check.py` and
`tools/controls.py` run individually.
