# Proposed twenty-configuration accounting — DRAFT

**PROPOSED — NOT ACCEPTED.** Mission P3-C11-DR01-D2A1-FR-STUTTER-R5-01, section 19. Executor: Claude Code, Claude
Opus 5.5. It records what the authenticated exact-final receipts printed, and F-R's *proposed* contract status under
AD-1-R5, which is a draft that is neither accepted nor adopted. AD-1-R4 governs.

**Basis.** Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd335999cc995c0ab9f582a30244973ef0`);
pinned executable sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`; Stage-1 evidence
`f023cc3d031a2a89e5ecbd4086e15f2e50869acd`; Stage-2 evidence `89326b859792a442c1853c3d35d30a48301c009e`. Every value
below is read from the receipts' git objects by `tools/accounting.py`; none is computed or estimated, except the
DERIVED original states of F-3, F-PT and F-B, which Stage 1 already reported as derived, conditional on B(1).

**Accepted by the Architect (FINAL-EVIDENCE-STAGE2 ruling):** the exact-final unchanged-core configurations F-1,
F-2, F-3, F-4, F-5, F-6, F-7, F-8, F-PT, F-B, F-B2, F-G, F-W and F-D. X-1 to X-5 are the default explorations of 8.2,
inside the passing Stage-1 suite.

## The twenty

<!-- ACCOUNTING TABLE -->

Nineteen required explorations or replacing quotient runs completed with PASS. F-3, F-PT and F-B are the composed
quotient runs that replace their `full_*` runs under AD-1-R4 E.1; their original graphs were not explored. F-R's
exact-final run is INCOMPLETE.

## F-R

| Item | Value |
|---|---|
| Raw verdict | **INCOMPLETE** — "the state cap of 30000000 was reached. No property is established by this run." Permanent. |
| Printed counts | 30,000,001 discovered, 26,539,923 expanded; 125,538,060 transitions (9,877,119 with a fault) and 1,920 no-step states **in the explored prefix only**. They are not F-R's totals. |
| Total state count | **not stated** (none is known; none is derived) |
| Total transition count | **not stated** |
| Original graph completed | **no** |
| Proposed contract status | **PROOF-DISCHARGED — pending R5 acceptance and adoption**: "F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF", only once every prerequisite of AD-1-R5 C holds (the accepted exact-final F-2 PASS, the accepted proof, the accepted occurrence closure, the complete branch accounting, the Architect's review and the Owner's adoption) |
| Safety, D1, L1 | DERIVED by the proof (FR-11, FR-12, FR-15) from the F-2 receipt; not measured on F-R's graph |
| Mutation control | C15, original explorer, 30,000,000 (`C15-DISPOSITION.md`) |

### F-R branch provenance (section 14)

From `FR-BRANCH-DISPOSITION.tsv` (193 rows; by proof: <!-- FR TAKEN --> TAKEN, <!-- FR NOT TAKEN --> NOT TAKEN,
<!-- FR NA --> NOT APPLICABLE; none NOT_ESTABLISHED):

| Rows | F-R status by proof | Provenance |
|---|---|---|
| the 54 labels the complete F-2 run took | TAKEN | MEASURED_IN_COMPLETE_F2 + DERIVED_BY_FR_PROOF (FR-8, FR-16); all 54 also printed taken in the F-R prefix (corroboration only) |
| `resolve: Stale` (B132), `append_cell C: Stale` (B146), `empty: Stale` (B186) | TAKEN | MEASURED_IN_FR_PREFIX_ONLY (existence on a reachable transition; not coverage) + MEASURED_IN_EXACT_FINAL_DIAGNOSTIC + DERIVED_BY_FR_PROOF (FR-16, FR-17, FR-RS) |
| `append_cell X: Stale` (B160) | NOT TAKEN | DERIVED_BY_FR_PROOF (FR-17.X) + MEASURED_IN_COMPLETE_F2 (not taken by ordinary transitions) + MEASURED_IN_EXACT_FINAL_DIAGNOSTIC (behaviour of a stale AppendX on a forced state; not reachability) |
| `append_cell U: Stale` (B174) | NOT TAKEN | DERIVED_BY_FR_PROOF (INV-AC, FR-16) + MEASURED_IN_COMPLETE_F2 |
| every other row F-2 did not take | NOT TAKEN | MEASURED_IN_COMPLETE_F2 + DERIVED_BY_FR_PROOF (FR-7, FR-16, FR-17) |
| the 30 rows of operations absent by configuration (DROP, snapshot, append_acknowledgement, malformed snapshot) | NOT APPLICABLE | MEASURED_IN_COMPLETE_F2 + DERIVED_BY_FR_PROOF (the configurations differ only in `replays`; REPLAY adds labels only to resolve, append_cell, empty) |

### Exact-final diagnostics used for the replay branches

| Diagnostic | Receipt | Result | What it shows |
|---|---|---|---|
| `ledger::protocol::model_tests::e8_holds_for_stale_worker_operations` | Stage-1 `final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` (sha256 `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3`), line `test ledger::protocol::model_tests::e8_holds_for_stale_worker_operations ... ok`; suite `313 passed; 0 failed` | ok | on configuration FR and a forced core, a stale Resolve, AppendC, AppendX and EmptyX each leave no violation and emit only labels ending in `Stale`; supporting evidence for FR-10, not a reachability result |

## Receipts

<!-- SOURCE TABLE -->
