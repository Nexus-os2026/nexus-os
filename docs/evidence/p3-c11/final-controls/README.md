# Candidate 11 — final control register (P3-C11-M7 N4)

The complete 596-record control register of Candidate 11 after the Architect's N3 acceptance. It is a record of
accepted outcomes; nothing here was re-run to produce it. Citation format and path prefixes (`EV/`, `AUD/`) are
explained in `docs/phase3/c11/CLOSURE-RECORD.md` §1; sources are listed at the end of this file.

## Files

| File | Content |
|---|---|
| `final-controls.tsv` | One summary row per record (596 rows plus a header), sorted by key |
| `final-controls.jsonl` | One JSON object per record, same order, with the full fields described below |

Columns of the TSV: `key`, `family`, `id`, `aliases`, `historical_class`, `pool_445` (in the 445-control campaign
pool), `accepted_outcome`, `effective_rule`, `ruling` (ruling id, see below), `evidence_files` (primary evidence,
`;`-separated, `EV/`-relative), `definition_source`.

JSON fields: the same, plus `meaning`, `kind`, `am1_class`, `mutation_text` (PRESENT / NOT FOUND / RECONSTRUCTED),
`verification`, `am2_rulings`, `credit`, `nonclaims`, `denominators`, `register_source` (record index and canonical
AM1 record digest in the AM2 register [S19]), `definition` and `outcome`. For every executed campaign control,
`definition` carries the exact approved edits (file, operation, original and replacement text, count), the judge,
the step argv and timeout, and the expected file digests before and after mutation, copied from the sealed runtime
plan that actually ran it, with that plan's path and sha256 [S60] [S61] [S62]. The tested source of every campaign
control is N1 `567cbc65a3998be6e4c3148520abf51314a6fd22` (tree `eb57c41deee9261fd364f2eccac41bf07ac063c7`) plus those
edits, applied temporarily and restored [S03 L33]. `outcome.evidence` lists every receipt, result and semantic-review
file with its sha256. For records outside the pool, `definition` points to the record in the AM2 register.

Values that would point into the closed session-transcript route are never copied into these files.

## Accounting

596 records: RUN 339, RUN-ADAPTED 88, RE-ANCHOR 18, CARRY 80, EXECUTED-AT-N1 7 lineage references representing
6 unique executions, RETIRE 2, HISTORICAL 62 [S07 L159-L161] [S06 L80]. The pool is RUN + RUN-ADAPTED + RE-ANCHOR =
445 [S07 L161]. Aliases and replacement attempts add no record [S03 L39].

| Family | Records | RUN | RUN-ADAPTED | RE-ANCHOR | CARRY | EXECUTED-AT-N1 | RETIRE | HISTORICAL | Pool KILLED | Pool AS SPECIFIED |
|---|---|---|---|---|---|---|---|---|---|---|
| ADHOC-NC | 6 | 2 | 0 | 0 | 0 | 0 | 0 | 4 | 2 | 0 |
| ARCHITECT-M3-REVIEW | 13 | 0 | 0 | 0 | 0 | 0 | 0 | 13 | 0 | 0 |
| ARCHITECT-M4-REVIEW | 6 | 0 | 0 | 0 | 0 | 0 | 0 | 6 | 0 | 0 |
| ARCHITECT-NO-TEXT | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 7 | 0 | 0 |
| C10-183 | 183 | 176 | 0 | 5 | 2 | 0 | 0 | 0 | 181 | 0 |
| C9-52 | 52 | 50 | 0 | 2 | 0 | 0 | 0 | 0 | 52 | 0 |
| D2-R2 | 2 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| D2A1-CORE | 36 | 0 | 0 | 0 | 35 | 0 | 0 | 1 | 0 | 0 |
| D2A1-SHELL | 35 | 0 | 0 | 0 | 35 | 0 | 0 | 0 | 0 | 0 |
| HIVEMIND-INFOFLOW | 5 | 1 | 0 | 3 | 0 | 0 | 0 | 1 | 4 | 0 |
| HIVEMIND-PRIVACY-CLOSURE | 2 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 |
| M1-HM-A1 | 10 | 9 | 0 | 0 | 0 | 0 | 0 | 1 | 9 | 0 |
| M2-HM-M2 | 13 | 10 | 0 | 2 | 0 | 0 | 0 | 1 | 12 | 0 |
| M3-HM-M3 | 34 | 27 | 0 | 3 | 0 | 0 | 0 | 4 | 30 | 0 |
| M4-HM-M4 | 26 | 26 | 0 | 0 | 0 | 0 | 0 | 0 | 26 | 0 |
| M5 | 20 | 14 | 0 | 3 | 2 | 0 | 0 | 1 | 17 | 0 |
| M6 | 29 | 23 | 0 | 0 | 5 | 1 | 0 | 0 | 23 | 0 |
| M7-N1 | 6 | 0 | 0 | 0 | 0 | 6 | 0 | 0 | 0 | 0 |
| RS01 | 111 | 0 | 88 | 0 | 0 | 0 | 0 | 23 | 66 | 22 |
| **Total** | **596** | **339** | **88** | **18** | **80** | **7** | **2** | **62** | **423** | **22** |

Where the 445 accepted outcomes come from [S03 L39]:

| Execution | Controls | Accepted | Ruling id |
|---|---|---|---|
| AM2 mandatory pilot: C9-52/NC-C9-P1-HEADER-SCROLLS-AWAY, C10-183/M146 | 2 | 2 KILLED | AM3-R1 [S06 L49-L58] |
| AM4 campaign | 441 | 419 KILLED + 22 AS SPECIFIED | AM5-S1 [S04 L32-L35] |
| AM5 replacements: C10-183/M42, C10-183/M172 | 2 | 2 KILLED | AM6-S2 [S03 L33-L37] |

Kept separate from the 445: 80 CARRY records (approved historical evidence, no N3 execution) [S07 L54-L67]; the six
unique N1 executions with credit 1 and the zero-credit M6 X1 lineage reference to the same execution [S07 L50]
[S07 L83] [S06 L85]; 64 no-credit records (62 HISTORICAL, 2 RETIRE) [S07 L151-L161].

## The two failed AM4 attempts and their superseding evidence

Both AM4 observations stay in the record as observed; neither was relabelled [S04 L36] [S03 L39]:

- **C10-183/M42.** AM4: COMPILE ERROR — the approved re-anchored mutation no longer compiled at N1 because `record()`
  returns `Result<u64, AuthorityError>` [S12 L129-L132]. AM5 re-anchored only the replacement tail
  (`self.record(&record).map(|_| ())`), keeping fault and judge [S04 L40-L56]; the mutant failed the unchanged state
  assertion at `authority/tests.rs:1225:5` and was accepted as KILLED [S03 L36]. In the data, `outcome.evidence` holds
  the AM5 receipts and review; `outcome.superseded_attempt` holds the AM4 receipts.
- **C10-183/M172.** AM4: INVALID(PROCESS LEFT BEHIND) — the stopped agent's planner reached the local model endpoint
  and a `curl` grandchild outlived the test [S12 L133-L137]. AM5 kept the mutation and test byte-identical and gave the
  test an owned loopback fixture instead of the real daemon [S04 L57-L66]; the mutant failed the unchanged result
  assertion at `scheduled_tests.rs:352:5` and was accepted as KILLED, with no model inference claimed [S03 L37].

## Effective rules

Each outcome was judged under its family's original rule, as preserved and corrected by the Architect:

| Rule id in the data | Meaning (summary) | Source |
|---|---|---|
| `C10` | Any intended test fails (compile errors, zero tests and timeouts never count), then a separate per-control semantic review with a verbatim diagnostic and a causal explanation; KILLED only after review | [S09 L101] [S07 L249] [S06 L126-L130] |
| `C9` | The named test fails in a one-test run (`0 passed; 1 failed`) with its marker, CI=1, no `tests.rs` edits | [S09 L100] [S07 L238] [S06 L122-L125] |
| `RS01-R4` | Each named test fails at its named assertion, with R4's oracle, via and inventory checks | [S09 L92-L99] [S07 L91-L97] [S06 L132] |
| `RS01-COV2` | RS-01 coverage part 2's own expected outcome and judge (reported AS SPECIFIED) | [S09 L92-L99] [S07 L91-L97] |
| `MISSION-M1`, `MISSION-M3+` | At least one designated killing run and no invalid or stale run (M3 onward with the doctest-capable form; N1-C1's compile-fail doctest mode) | [S06 L133] [S06 L118-L121] |
| `MISSION-M2` | The first run must kill and no run may be invalid or stale | [S06 L133] |
| `SERIAL` | The five recovered HiveMind/D2-R2 controls under serial `--nocapture` result handling with their specific named judges | [S06 L134] [S06 L137-L141] |
| `XA` | The named mutant test fails with exit 101, restoration verifies, and the restored test then passes with exit 0 | [S06 L135] |
| `M7 N1 mutant rule` | The six N1 mutants, each run once on the N1 commit, restored to HEAD | [S08 L98-L106] |

## Ruling ids used in the data

| Id | Ruling | Source |
|---|---|---|
| AM2-R1 | N1 accepted, six unique mutation executions stand | [S07 L49-L53] |
| AM2-R2 | 80 historical carries approved | [S07 L54-L67] |
| AM2-R3 | Re-anchors and the M6 X1 alias | [S07 L68-L90] |
| AM2-R5 | Recovered campaigns; M11L survivor; M10/M11 RETIRE | [S07 L98-L150] |
| AM2-R6 | Seven unavailable mutations, zero credit | [S07 L151-L158] |
| AM2-R7 | Resulting accounting (596; 62 HISTORICAL) | [S07 L159-L161] |
| AM3-R1 | Mandatory pilot accepted | [S06 L49-L70] |
| AM5-S1 | 441 AM4 controls accepted | [S04 L12-L37] |
| AM6-S2 | N3 accepted (P3-C11-M7-N3-ACCEPTANCE-01) | [S03 L32-L48] |

## Sources

| ID | Kind | Locator | Digest | Lines |
|---|---|---|---|---|
| S03 | FILE | EV/amendment-6/AMENDMENT-6.txt | f57679259c33171ed48517b224a46dca8440f007794e731764e51fb7195ab6e3 | L32-L48 |
| S04 | FILE | EV/amendment-5/AMENDMENT-5.txt | f91c53476e61ebe231666d1eab3417bb6b56d557038e3aca5f7dc2cb7e78b709 | L12-L66 |
| S06 | FILE | EV/amendment-3/AMENDMENT-3.txt | 186a92b5eda216ca783ea8eacf8e11fdcae374232472609c6cf24689f874a522 | L49-L141 |
| S07 | FILE | EV/amendment-2/AMENDMENT-2.txt | d681a462db614af06d045bf4cea1eb856f40b88f5a40504e7f6bad6a4dbff2e8 | L49-L249 |
| S08 | FILE | EV/REPORT.md | 031e583705045a67135862b138138ed89a899882d6f50fade4ecd718b69bb8b0 | L98-L106 |
| S09 | FILE | EV/amendment-1/REPORT.md | 0e45348846bb1b02a7e7c835c49fffb4e6d060d8fc4efda83d8b1f9107bf0139 | L92-L101 |
| S12 | FILE | EV/amendment-4/REPORT.md | 2871013bc3080359e2b3c80938b2e54d39322498844b75d9309c7add4e5567fb | L129-L137 |
| S19 | FILE | EV/amendment-2/manifest/am2-manifest.json | 133bbe60e4c675128ee8e6523a72abf259f482515c25d3a0c7412b4d96f2a2f4 | - |
| S60 | FILE | EV/amendment-2/contract/runtime-plan.json | 27f6a3c43f942beea527d34ae85cea064dbfc36086fc5409cd1f899900b431a1 | - |
| S61 | FILE | EV/amendment-4/contract/runtime-plan.json | 618352c7e2a641fe5245dace6f4b53da783a03938a583eeb57c33ade0d4b5015 | - |
| S62 | FILE | EV/amendment-5/contract/runtime-plan.json | 4813cc595724634ff2a89e4eb2c2571537a12e245973ee55a422ce93cd51dfdc | - |
