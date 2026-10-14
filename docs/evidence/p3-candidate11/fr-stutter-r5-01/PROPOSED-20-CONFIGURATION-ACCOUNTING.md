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

| # | Configuration | Explorer / test | Raw outcome (as printed) | States or keys (expanded) | Transitions (fault) | No-step | Property lines | L1 line | Outer class (receipt) | Contract status |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | X-1 | default, `x1_two_facts_two_writers_one_admission` | PASS | 59,104 states (59,104) | 219,196 (15,026) | 34 | 24 HOLD; E10 not applicable | HOLDS: 204170 non-fault edges, no cycle; 34 end states, each terminal (D1) | PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite) | MEASURED PASS (exact-final default exploration, 8.2) |
| 2 | X-2 | default, `x2_one_fact_two_gating_callers` | PASS | 10,020 states (10,020) | 26,188 (922) | 100 | 24 HOLD; E10 not applicable | HOLDS: 25266 non-fault edges, no cycle; 100 end states, each terminal (D1) | PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite) | MEASURED PASS (exact-final default exploration, 8.2) |
| 3 | X-3 | default, `x3_one_conflicting_fact_with_env` | PASS | 25,617 states (25,617) | 81,573 (2,605) | 44 | 24 HOLD; E10 not applicable | HOLDS: 78968 non-fault edges, no cycle; 44 end states, each terminal (D1) | PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite) | MEASURED PASS (exact-final default exploration, 8.2) |
| 4 | X-4 | default, `x4_one_conflicting_fact_one_acknowledgement` | PASS | 3,106 states (3,106) | 10,097 (816) | 14 | 24 HOLD; E10 not applicable | HOLDS: 9281 non-fault edges, no cycle; 14 end states, each terminal (D1) | PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite) | MEASURED PASS (exact-final default exploration, 8.2) |
| 5 | X-5 | default, `x5_one_clean_batch_of_two` | PASS | 3,311 states (3,311) | 7,757 (704) | 28 | 24 HOLD; E10 not applicable | HOLDS: 7053 non-fault edges, no cycle; 28 end states, each terminal (D1) | PASS (Stage-1 suite classification: PASS, 48 of 48; default exploration inside the suite) | MEASURED PASS (exact-final default exploration, 8.2) |
| 6 | F-1 | original, `full_f1` | PASS | 46,524,336 states (46,524,336) | 209,297,440 (21,234,651) | 8,696 | 24 HOLD; E10 not applicable | HOLDS: 188062789 non-fault edges, no cycle; 8696 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 7 | F-2 | original, `full_f2` | PASS | 12,602,670 states (12,602,670) | 54,214,081 (3,558,228) | 3,206 | 24 HOLD; E10 not applicable | HOLDS: 50655853 non-fault edges, no cycle; 3206 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 8 | F-3 | composed quotient (replacing run, AD-1-R4 E.1), `gc_symmetry_f3` | PASS | 20,431,851 keys (20,431,851); Kf 880,778; DERIVED original states 119,948,772 (conditional on B(1); original graph not explored) | 77,342,551 (6,034,170) | 13,996 | 24 HOLD; E10 not applicable | HOLDS: 71308381 non-fault edges, no cycle; 13996 end states, each terminal (D1) | PASS — admissible unchanged-core composed-quotient PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 9 | F-4 | original, `full_f4` | PASS | 12,892,806 states (12,892,806) | 55,417,089 (3,639,522) | 3,478 | 24 HOLD; E10 not applicable | HOLDS: 51777567 non-fault edges, no cycle; 3478 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 10 | F-5 | original, `full_f5` | PASS | 5,553,180 states (5,553,180) | 27,818,472 (2,712,888) | 688 | 24 HOLD; E10 not applicable | HOLDS: 25105584 non-fault edges, no cycle; 688 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 11 | F-6 | original, `full_f6` | PASS | 9,315,384 states (9,315,384) | 50,476,096 (5,547,330) | 862 | 24 HOLD; E10 not applicable | HOLDS: 44928766 non-fault edges, no cycle; 862 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 12 | F-7 | original, `full_f7` | PASS | 2,420,576 states (2,420,576) | 12,707,156 (744,608) | 327 | 24 HOLD; E10 not applicable | HOLDS: 11962548 non-fault edges, no cycle; 327 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 13 | F-8 | original, `full_f8` | PASS | 12,775,224 states (12,775,224) | 63,000,416 (4,562,913) | 1,594 | 24 HOLD; E10 not applicable | HOLDS: 58437503 non-fault edges, no cycle; 1594 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 14 | F-PT | composed quotient (replacing run, AD-1-R4 E.1), `gc_symmetry_fpt` | PASS | 29,751,473 keys (29,751,473); Kf 1,698,028; DERIVED original states 173,414,754 (conditional on B(1); original graph not explored) | 113,252,841 (7,396,731) | 16,131 | 24 HOLD; E10 not applicable | HOLDS: 105856110 non-fault edges, no cycle; 16131 end states, each terminal (D1) | PASS — admissible unchanged-core composed-quotient PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 15 | F-B | composed quotient (replacing run, AD-1-R4 E.1), `gc_symmetry_fb` | PASS | 3,775,373 keys (3,775,373); Kf 156,463; DERIVED original states 22,182,849 (conditional on B(1); original graph not explored) | 11,404,590 (954,390) | 10,821 | 24 HOLD; E10 not applicable | HOLDS: 10450200 non-fault edges, no cycle; 10821 end states, each terminal (D1) | PASS — admissible unchanged-core composed-quotient PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 16 | F-B2 | original, `full_fb2` | PASS | 21,536 states (21,536) | 66,618 (7,890) | 70 | 24 HOLD; E10 not applicable | HOLDS: 58728 non-fault edges, no cycle; 70 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 17 | F-G | original, `full_fg` | PASS | 185,106 states (185,106) | 655,973 (62,304) | 303 | 24 HOLD; E10 not applicable | HOLDS: 593669 non-fault edges, no cycle; 303 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 18 | F-W | original, `full_fw` | PASS | 12,602,670 states (12,602,670) | 54,762,679 (4,106,826) | 3,206 | 25 HOLD; E10 HOLDS | HOLDS: 50655853 non-fault edges, no cycle; 3206 end states, each terminal (D1) | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 19 | F-D | original, `full_fd` | PASS | 7,140,378 states (7,140,378) | 29,870,006 (1,767,636) | 5,436 | 23 HOLD; E10 not applicable; L1 not claimed | not claimed for this configuration | PASS — admissible unchanged-core original-explorer PASS | MEASURED PASS (exact-final; accepted by the Architect's ruling) |
| 20 | F-R | original, `full_fr` | INCOMPLETE | 30,000,001 discovered (26,539,923 expanded) at the cap — **not a total** | 125,538,060 (9,877,119) in the prefix — **not a total** | 1,920 in the prefix — **not a total** | every line NOT ESTABLISHED (incomplete) | NOT ESTABLISHED (incomplete) | INCOMPLETE (the state cap of 30000000 was reached. No property is established by this run.) | **PROOF-DISCHARGED — proposed, pending R5 acceptance and adoption**; raw verdict INCOMPLETE stays |

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

From `FR-BRANCH-DISPOSITION.tsv` (193 rows; by proof: 57 TAKEN, 106 NOT TAKEN,
30 NOT APPLICABLE; none NOT_ESTABLISHED):

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

| Configuration | Raw output (git path at the evidence commit) | Commit | sha256 |
|---|---|---|---|
| X-1 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` | `f023cc3d031a` | `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3` |
| X-2 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` | `f023cc3d031a` | `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3` |
| X-3 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` | `f023cc3d031a` | `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3` |
| X-4 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` | `f023cc3d031a` | `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3` |
| X-5 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/30-suite/raw-output.txt` | `f023cc3d031a` | `d6f58742ea483dbb95452b186dcf72f729538e672ae60741d978b0eddc6ca3b3` |
| F-1 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/45-full-F-1/raw-output.txt` | `f023cc3d031a` | `c1455a1ba3e55ae1ddd83feb97e13585882727613608eca01a669439e8f62c6c` |
| F-2 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/41-full-F-2/raw-output.txt` | `f023cc3d031a` | `985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5` |
| F-3 | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/43-full-F-3/raw-output.txt` | `f023cc3d031a` | `6dfc796a0b5144184e3e50c9b1b873c21bb31bf26405c572fe858fb694245562` |
| F-4 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/62-full-F-4/raw-output.txt` | `89326b859792` | `44b0023eb9106365e4bf88039f8cc914c1e542f8a8490f604dfa0274aacfb948` |
| F-5 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/65-full-F-5/raw-output.txt` | `89326b859792` | `a6d17e7691c80b07c79d1c8115eb40acfa149f8dab3574bce1e1564cecb47d8b` |
| F-6 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/66-full-F-6/raw-output.txt` | `89326b859792` | `3660a0b2b88146f787aed4cf7d6fefe16ebfa658bfa172c33d90e9df3df28452` |
| F-7 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/67-full-F-7/raw-output.txt` | `89326b859792` | `69e152df74318f497bfd1b8f2b6525de73c00317651f42934f4b22b69a3b417a` |
| F-8 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/64-full-F-8/raw-output.txt` | `89326b859792` | `2e0f15299b7ed66a30cd12cf8ab6414139480a52687c83796b9d5718d4d3e3ee` |
| F-PT | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/44-full-F-PT/raw-output.txt` | `f023cc3d031a` | `0863794ef7500c25382f55307213c65557f2d02b7bc4bb433511a71e082119a8` |
| F-B | `docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/42-full-F-B/raw-output.txt` | `f023cc3d031a` | `c8b612e9ee7e2b8478bf7d4a8bb1dc37d737da7a06828136156752772c8ccaa7` |
| F-B2 | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/61-full-F-B2/raw-output.txt` | `89326b859792` | `516adc39418a5cc0db8432e1ce7c1051029d05cb30dc133ac9b1147439633aa3` |
| F-G | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/60-full-F-G/raw-output.txt` | `89326b859792` | `fabcbf13cfccec2744d03d38042dec7e72e3f3699f941a9a2ce17869d55705d1` |
| F-W | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/63-full-F-W/raw-output.txt` | `89326b859792` | `d309de48cde0febf20ab897d3e01fad05339e27d2fbb7a42abe674670900ba71` |
| F-D | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/68-full-F-D/raw-output.txt` | `89326b859792` | `2f80e958de47a72a81f6d7ffb7f5ff00c95895a21f387d66eebb0b696304c6b3` |
| F-R | `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/69-full-F-R/raw-output.txt` | `89326b859792` | `78e44c335b6ce65f8df6aea18b71e488613403ba8c0471c5cf1c33057ab58ccd` |
