# Stage 2 — section 8.3 / 8.4 closure record

This is the consolidated record that contract 8.4 requires for every default and full run, built from raw receipts by
the frozen `tools/s2_configs.py` (`receipts/20-23`; 372 of 372 checks) and rendered into `02-CONFIGURATIONS.tsv`,
`02-BRANCH-VOCABULARY.tsv` and `02-IGNORED-TEST-DISPOSITIONS.md`. Nothing was run to produce it.

## 1. The 20 configurations

| # | Config | Run | Raw outcome | States or keys (expanded) | Transitions (fault) | No-step | Property lines | Wall s / peak memory B | Discharge |
|---|---|---|---|---|---|---|---|---|---|
| 1 | X-1 | original, default | PASS | 59,104 (59,104) | 219,196 (15,026) | 34 | 24 HOLD, E10 n/a | 0.17 / 8,769,536 | 8.2 default exploration — MEASURED PASS |
| 2 | X-2 | original, default | PASS | 10,020 | 26,188 (922) | 100 | 24 HOLD, E10 n/a | see TSV | MEASURED PASS |
| 3 | X-3 | original, default | PASS | 25,617 | 81,573 (2,605) | 44 | 24 HOLD, E10 n/a | see TSV | MEASURED PASS |
| 4 | X-4 | original, default | PASS | 3,106 | 10,097 (816) | 14 | 24 HOLD, E10 n/a | see TSV | MEASURED PASS |
| 5 | X-5 | original, default | PASS | 3,311 | 7,757 (704) | 28 | 24 HOLD, E10 n/a | see TSV | MEASURED PASS |
| 6 | F-1 | original `full_f1`, cap 60,000,000 (AD-1-R4 A) | PASS | 46,524,336 | 209,297,440 (21,234,651) | 8,696 | 24 HOLD, E10 n/a | see TSV | REQUIRED_FINAL_RUN — PASS |
| 7 | F-2 | original `full_f2` | PASS | 12,602,670 | 54,214,081 (3,558,228) | 3,206 | 24 HOLD | | REQUIRED_FINAL_RUN — PASS |
| 8 | F-3 | composed quotient `gc_symmetry_f3` | PASS | 20,431,851 keys; Kf 880,778; DERIVED original 119,948,772 | 77,342,551 (6,034,170) quotient | 13,996 | 24 HOLD | | REPLACED_BY the quotient run (AD-1-R4 B–E, N) — the original graph was **not** explored |
| 9 | F-4 | original | PASS | 12,892,806 | 55,417,089 (3,639,522) | 3,478 | 24 HOLD | | PASS |
| 10 | F-5 | original | PASS | 5,553,180 | 27,818,472 (2,712,888) | 688 | 24 HOLD | | PASS |
| 11 | F-6 | original | PASS | 9,315,384 | 50,476,096 (5,547,330) | 862 | 24 HOLD | | PASS |
| 12 | F-7 | original | PASS | 2,420,576 | 12,707,156 (744,608) | 327 | 24 HOLD | | PASS |
| 13 | F-8 | original | PASS | 12,775,224 | 63,000,416 (4,562,913) | 1,594 | 24 HOLD | | PASS |
| 14 | F-PT | composed quotient `gc_symmetry_fpt` (A4 on) | PASS | 29,751,473 keys (99.17% of 30,000,000); Kf 1,698,028; DERIVED original 173,414,754 | 113,252,841 (7,396,731) quotient | 16,131 | 24 HOLD | | REPLACED_BY the quotient run; also A4's (f) comparison with F-3 |
| 15 | F-B | composed quotient `gc_symmetry_fb` | PASS | 3,775,373 keys; Kf 156,463; DERIVED original 22,182,849 | 11,404,590 (954,390) quotient | 10,821 | 24 HOLD | | REPLACED_BY the quotient run |
| 16 | F-B2 | original | PASS | 21,536 | 66,618 (7,890) | 70 | 24 HOLD | | PASS |
| 17 | F-G | original | PASS | 185,106 | 655,973 (62,304) | 303 | 24 HOLD | | PASS |
| 18 | F-W | original | PASS | 12,602,670 | 54,762,679 (4,106,826) | 3,206 | **25 HOLD (E10 HOLDS)** | | PASS — the only configuration that establishes E10 |
| 19 | F-D | original | PASS | 7,140,378 | 29,870,006 (1,767,636) | 5,436 | 23 HOLD, E10 n/a, **L1 not claimed** | | PASS |
| 20 | F-R | original `full_fr` | **INCOMPLETE** | 30,000,001 discovered (26,539,923 expanded) **at the cap — not a total** | 125,538,060 (9,877,119) **in the prefix — not a total** | 1,920 in the prefix | every line NOT ESTABLISHED (incomplete) | 165.06 / 3,973,214,208 | **F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF** (AD-1-R5 D). This is not an explorer PASS. |

Every run's bounds equal the contract's reviewed bounds (8.2/8.3), ticket limit 8 and refusal limit 4; every run used
E-C on exact C; every PASS completed (expanded = discovered, within its cap); no OOM, swap or timeout anywhere.
Per-run time, peak memory, exact argv and executable identity are in `02-CONFIGURATIONS.tsv`.

## 2. The 8.4 receipt fields, run by run

| Field required by line 863 | Where it is, for every run |
|---|---|
| the commit | `source_head`/`source_tree` columns: C `d5964ee5…`, `51bac5fd…`, EXACTLY_C before every run |
| the compiler version | rustc 1.94.0 (4a4ef493e 2026-03-02), the pinned build record (`final-baseline-known-runs-01/receipts/10-build`, `11-pin`) |
| the profile | release test profile (`--release --lib`, opt-level 3, thin LTO) for the 20 e1 receipts; the X runs also ran unoptimised in Stage B |
| the exact command | `argv` column |
| the configuration and its bounds | `bounds` and `actors` columns, checked against the contract |
| states and transitions | as section 1; quotient runs: MEASURED quotient counts plus DERIVED original states (AD-1-R4 D); F-R: prefix counts only, labelled (S5-3) |
| each property's verdict | `property_lines_hold`, `not_applicable`, `not_claimed`, `not_established`, `D1`, `L1` columns; the lines themselves are in the raw receipts |
| for each branch of each operation, whether any run took it | `02-BRANCH-VOCABULARY.tsv` (section 3) |
| the time | `elapsed_printed` and `wall_seconds` |
| the peak memory as the starting command reports it | `memory_peak_bytes` (the scope's cgroup `memory.peak`) |
| how the run ended | `exit_status`, `signal`, `raw_outcome`, `admissibility` |
| every `#[ignore]` listed with its receipt | `02-IGNORED-TEST-DISPOSITIONS.md` (34 of 34; S-9: "or with its approved disposition") |

## 3. Branch accounting (193 vocabulary rows)

Recomputed from the raw receipts against `final-baseline-remaining-configs-01/vocabulary/VOCABULARY.tsv` (193 rows,
unique ids and labels; every taken label of every run is a vocabulary row):

| Classification | Rows | Meaning |
|---|---|---|
| **MEASURED_COMPLETE** | **85** | taken by at least one completed run (the 19 PASS runs); quotient runs listed separately as MEASURED_COMPLETE_QUOTIENT (their taken set equals the original's only under B(1)) |
| **MEASURED_IN_INCOMPLETE_PREFIX** | **3** | B132 `resolve: Stale`, B146 `append_cell C: Stale`, B186 `empty: Stale`: printed taken only in F-R's incomplete prefix; each is also DERIVED_BY_ACCEPTED_PROOF TAKEN for F-R. Existence on a reachable transition; **not coverage** |
| **UNOBSERVED** | **105** | taken by no run; **not covered**, disclosed as such |

DERIVED_BY_ACCEPTED_PROOF is recorded per row as a separate source and never changes a row's classification: the F-R
disposition gives 57 TAKEN, 106 NOT TAKEN and 30 NOT APPLICABLE (none NOT_ESTABLISHED). Every proof-TAKEN row is
measured somewhere; the proof upgrades no unobserved row. The F-2 measured set and the F-R prefix set equal the
disposition file's columns exactly. No Stage B executable sha256 is recorded or invented: Stage B is an unoptimised
`cargo test` whose X reports were captured by libtest.

## 4. F-R, recorded exactly

- Raw receipt: **INCOMPLETE**, the state cap of 30,000,000 reached; 30,000,001 discovered, 26,539,923 expanded;
  `full_fr` on the original explorer at exact C; raw sha256 `78e44c33…8ccd`; exit 101 by the harness assertion.
- Recording, in AD-1-R5 D's words: **F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT
  PROOF**, beside the INCOMPLETE verdict, the F-2 receipt (`985a3976…4ab5`, PASS, D1 and L1 HOLD) and the branch
  accounting's provenance.
- AD-1-R5 C prerequisites: (1) exact C and its blobs, with the 49 Rust inputs of E-C's dep-info — holds; (2) the
  accepted F-2 PASS — holds (raw hash equal); (3) `e8_holds_for_stale_worker_operations ... ok` in the Stage-1 suite
  (line 428 of `30-suite/raw-output.txt`) — holds; (4) `REPLAY-OCCURRENCE-MATRIX` reports
  REPLAY_OCCURRENCE_CLOSURE_COMPLETE — present; (5) the proof `c97332a3…606b` — present; (6) 193 rows, none
  NOT_ESTABLISHED — holds; (7) the Architect's independent review — **conveyed only**: the Owner-adoption record calls the
  proof "accepted" and records the `Config.name` ruling, but no separate by-ID Architect review record of (4)–(6)
  exists in Git; (8) Owner adoption — recorded (`1492e185`).
- None of the three wordings that AD-1-R5 D prohibits for F-R is used anywhere in this packet, and no total, ceiling or bound of F-R's original graph is stated.

## 5. Verdict for 8.3 / 8.4

Every configuration obligation is discharged by the adopted contracts: 19 by admissible exact-final PASS receipts (3 of
them composed-quotient runs admitted by AD-1-R4 E.1), and F-R by AD-1-R5's proof discharge with its raw INCOMPLETE
receipt kept. **No omitted measurement became a PASS.** The 8.4 record now exists as a consolidated record; whether the
Architect accepts it as contract 8.4's record, and the X evidence form, are review decisions (`ARCHITECT-REVIEW-QUESTIONS.md`).
One contract-level tension is not resolved here: line 942 lists an INCOMPLETE 8.3 run as a stop condition and line 926
withholds the label for one (`06-SECTION11-DECISION-DOCKET.md`).
