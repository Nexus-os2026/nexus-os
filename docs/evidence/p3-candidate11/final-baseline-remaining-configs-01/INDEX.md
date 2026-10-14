# Final-baseline remaining configurations 01 — index

Mission P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE2-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort
High. For the Sole Architect's review.

**Status: REMAINING_CONFIGURATION_BLOCKED.** F-R, the tenth and last remaining configuration, ended **INCOMPLETE**:
its original exploration reached the authorised 30,000,000-state ceiling. Nothing was retried, no ceiling was raised,
no other explorer was used. The other nine remaining configurations and all three standalone healthy witnesses PASS.
Because one remaining configuration did not PASS, the twenty-run aggregate matrix was **not produced** (mission 11).

**Evidence only.** No build, no source or manifest change, no mutation, no mutant witness, no run of F-1, F-2, F-3,
F-PT or F-B, no rerun of the nonignored suite, no quotient run, no hosted CI. D2A-1 is not accepted and not
integrated; nothing here completes Phase Three.

## Identities (`receipts/00-authentication.txt`, re-checked in `receipts/60-final-gate.txt`)

| Item | Value |
|---|---|
| Frozen C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| Blobs | protocol.rs `a2df2841…`, model_tests.rs `28085ce2…`, unit_tests.rs `55dc6640…`; ct-1 `5d31cb45…`; AD-1-R4 sha256 `00ced6d5…5192b693` |
| Worktree | `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01`, detached at C, byte-equal to C (5,364 tracked blobs) before and after every invocation; only `!! target/` |
| Pinned executable | `…/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`, 8,984,464 bytes — equal before, at selection and after every invocation; not rebuilt |
| Stage-1 evidence | `f023cc3d031a2a89e5ecbd4086e15f2e50869acd` (tree `646c4fe9…`, parent `0622a4d0…`) = GitHub `review/p3-c11-d2a1-final-baseline-known-runs-01`; its SHA256SUMS (179 entries) verified against the committed blobs |

## Branch vocabulary (mission 5; `vocabulary/`, `receipts/20`–`24`)

- **193 source-bound rows** (accounting unit: operation, exact label) from **11 `Obs::branch` sites** (`SOURCE-AUDIT.md`;
  `VOCABULARY.json` sha256 `db8734e0bd2743f22a27fb4001e8deb0f6381b393aecc33c781d710ee57fa932`; `VOCABULARY.tsv`).
  Per operation: append_facts 36, expire 1, append_gating 34, lookup 12, explain 11, snapshot 10,
  append_acknowledgement 16, barrier 5, resolve 15, append_cell C/X/U 14 each, empty 5, admission 2, DROP 1,
  malformed snapshot 3.
- The static `BRANCHES` constant lists 90 labels, every one a row. **103 source-emittable labels are missing from it**
  (`STATIC-OMISSIONS.txt`), including the three the mission names — `append_cell C: Refused(Latched)` (B148),
  `append_cell C: Refused(NotHealthy)` (B149), `append_cell X: Refused(NotHealthy)` (B163) — and
  `resolve: Refused(NotHealthy)` (B135), which Stage-1's F-1 report already prints as taken. No run of either stage
  printed any other omitted label as taken.
- Collisions: `admission: admitted` / `refused` come from S10 `step_adm` and S11 `admit_by`; proved the same
  operation and semantic branch (`SOURCE-AUDIT.md` §6), and S11 is unreachable from every required test. **No
  unresolved collision.** The 14 `append_cell U` rows are kept as type-domain rows (no reachability argument used).
- Three implementations agree exactly: the anchored-inventory **generator** (`20`, freeze check `21`: EQUAL),
  **check A** — sites, sinks, printers, reachability, derived Debug, over all 49 crate files (`22`: PASS) — and
  **check B** — an independent lexer/parser/evaluator deriving each site's labels from the control flow (`23`: PASS,
  every derived label listed). **Controls** (`24`): 19 of 19 behave as required — omitted site (2 forms), omitted enum
  variant (2), changed exact label (2), unresolved collision (2), raw label outside the vocabulary (2), row without
  source derivation (2), NOT APPLICABLE operation taken, unsorted extras, truncated list, 2 positive, 2 meta-controls.

## Stage-1 branch accounting (mission 6; `normalized/stage1/`, `receipts/30-stage1-normalization.txt`)

Read from the Stage-1 commit's git objects (not copied), after the freeze, with the frozen normalizer. Every raw label
resolved; no STOP; no Stage-1 verdict reinterpreted.

| Report | Stage-1 raw (path under `final-baseline-known-runs-01/`; blob; sha256) | TAKEN / NOT TAKEN / N/A | Taken outside static BRANCHES |
|---|---|---|---|
| X-1…X-5 | `receipts/30-suite/raw-output.txt`; `6e2ec7f4…`; `d6f58742…6ca3b3` (each default test's segment) | 21/108/64, 26/137/30, 24/103/66, 36/119/38, 16/111/66 | X-3, X-4: `append_cell X: Refused(NotHealthy)` |
| F-1 | `receipts/45-full-F-1/raw-output.txt`; `c36226c8…`; `c1455a1b…2c6c` | 42/121/30 | `resolve: Refused(NotHealthy)`, `append_cell C: Refused(Latched)`, `append_cell C: Refused(NotHealthy)` |
| F-2 | `receipts/41-full-F-2/raw-output.txt`; `b32a5d64…`; `985a3976…4ab5` | 54/109/30 | the three named labels |
| F-3 | `receipts/43-full-F-3/raw-output.txt`; `872ef461…`; `6dfc796a…5562` | 56/107/30 | the three named labels |
| F-PT | `receipts/44-full-F-PT/raw-output.txt`; `a92cb8da…`; `0863794e…19a8` | 55/108/30 | the three named labels |
| F-B | `receipts/42-full-F-B/raw-output.txt`; `b5ba4be8…`; `c8b612e9…caa7` | 56/107/30 | the three named labels |

F-3, F-PT and F-B are Stage 1's composed-quotient reports (their original graphs were not explored).

## Standalone healthy witnesses (mission 7) — AD-1-R4 type (e3)

Each: the pinned executable, exactly one test with `--exact` (and shown not ignored), `--nocapture --test-threads=1`,
no `--ignored`; 16 GiB, zero swap, 180 s; exit 0, no signal, no OOM.

| Witness | Receipt | Result | Raw sha256 |
|---|---|---|---|
| C1 `c1_witness_p1_at_a_successful_admission` | `receipts/50-witness-C1` | **PASS — admissible healthy-witness type-(e3) PASS** (15/15): exact G.3(d) line, no marker | `26bc516e…8420e7` |
| C25 `c25_witness_p7_on_the_resulting_world` | `receipts/51-witness-C25` | **PASS** (15/15): exact G.3(d) line, no marker | `1ad1bc3e…75a6b0` |
| C4 `c4_witness_p4_and_p8_on_f8` | `receipts/52-witness-C4` | **PASS** (19/19): both scenarios, the three G.5(C) lines, G.6 for all 20 step records against the frozen schedule (a464a366; P4 step 8 and P8 step 12 consume nothing and leave their whole offered suffix) | `15ec6132…bc9918` |

## Remaining configurations (missions 8–10) — original explorer, one at a time

`--exact --ignored --nocapture --test-threads=1`; 16 GiB, zero swap, 45 min; bounds and actors equal to the
source-derived configuration (`receipts/26-configs.txt`, `27-EXPECTATIONS.json`); KEY and D1 HOLD in every PASS.

| Config | Outcome | States (expanded) | Transitions (fault) | Non-fault | No step | Explorer / wall | Scope peak | Properties | Branches T/N/NA | Taken outside static | Outer class |
|---|---|---|---|---|---|---|---|---|---|---|---|
| F-G | PASS | 185,106 (185,106) | 655,973 (62,304) | 593,669 | 303 | 0.7 / 0.66 s | 0.02 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 49/114/30 | C: Refused(Latched), C: Refused(NotHealthy), X: Refused(NotHealthy) | PASS — admissible unchanged-core original-explorer PASS |
| F-B2 | PASS | 21,536 (21,536) | 66,618 (7,890) | 58,728 | 70 | 0.1 / 0.05 s | <0.01 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 18/111/64 | none | PASS — admissible … |
| F-4 | PASS | 12,892,806 (12,892,806) | 55,417,089 (3,639,522) | 51,777,567 | 3,478 | 69.4 / 69.57 s | 1.63 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 54/109/30 | the three named | PASS — admissible … |
| F-W | PASS | 12,602,670 (12,602,670) | 54,762,679 (4,106,826) | 50,655,853 | 3,206 | 69.0 / 69.13 s | 1.60 GiB | **25 HOLD**, L1 HOLDS, **E10 HOLDS** | 58/105/30 | the three named | PASS — admissible … |
| F-8 | PASS | 12,775,224 (12,775,224) | 63,000,416 (4,562,913) | 58,437,503 | 1,594 | 75.2 / 75.36 s | 1.66 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 69/120/4 | the three named | PASS — admissible … |
| F-5 | PASS | 5,553,180 (5,553,180) | 27,818,472 (2,712,888) | 25,105,584 | 688 | 33.2 / 33.29 s | 0.77 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 41/114/38 | X: Refused(NotHealthy) | PASS — admissible … |
| F-6 | PASS | 9,315,384 (9,315,384) | 50,476,096 (5,547,330) | 44,928,766 | 862 | 59.3 / 59.42 s | 1.27 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 47/111/35 | X: Refused(NotHealthy) | PASS — admissible … |
| F-7 | PASS | 2,420,576 (2,420,576) | 12,707,156 (744,608) | 11,962,548 | 327 | 15.3 / 15.29 s | 0.34 GiB | 24 HOLD, L1 HOLDS, E10 n/a | 38/117/38 | X: Refused(NotHealthy) | PASS — admissible … |
| F-D | PASS | 7,140,378 (7,140,378) | 29,870,006 (1,767,636) | 28,102,370 | 5,436 | 32.6 / 32.65 s | 0.77 GiB | **23 HOLD**, **L1 not claimed**, E10 n/a | 42/122/29 | C: Refused(Latched), C: Refused(NotHealthy) | PASS — admissible … |
| **F-R** | **INCOMPLETE** | **30,000,001 (26,539,923)** | 125,538,060 (9,877,119) | 115,660,941 | 1,920 | 164.7 / 165.06 s | 3.70 GiB | NOT ESTABLISHED (incomplete) | not produced | — | **INCOMPLETE** (the state cap of 30000000 was reached) |

Each PASS: 34 of 34 checks (`receipts/6n-full-<CFG>/CHECKS.txt`); its normalized table is
`BRANCH-ACCOUNTING-<CFG>.{json,md}` in the same directory. "n/a" = E10 printed "not applicable here (worker verbs do not
unwind)". N/A rows are whole operations absent by configuration (no Ack actor, no Gc actor, DROP off, malformed off),
each with its source-anchored reason.

### F-R (the stop) — `receipts/69-full-F-R/`

- Raw outcome: `outcome: INCOMPLETE`, `INCOMPLETE: the state cap of 30000000 was reached. No property is established by
  this run.`; the harness `test result: FAILED. 0 passed; 1 failed`; exit 101 by a normal exit (the panic is
  `full_exploration`'s own verdict assertion, model_tests.rs 4133); no signal, no OOM (oom_kill 0), swap peak 0, wall
  165 s of 2,700 s. Outer class: **INCOMPLETE** (not PASS). Not BUDGET EXCEEDED: no FAIL was printed.
- Branch accounting: **not produced**. The frozen normalizer refused (STOP: "2 EXPLORATION headers"), because the
  failing assertion prints the report a second time. It was not repaired or bypassed. Read-only executor observations
  (`receipts/69-full-F-R.executor-observations.txt`, not a classification): the two copies are identical; all 93
  printed branch lines are frozen-vocabulary labels (57 taken; outside the static list: the three named labels). An
  INCOMPLETE run's branch set covers only the expanded part; it is not a coverage receipt.

## Aggregate (mission 11)

Not produced: F-R is not PASS (`receipts/70-aggregate.txt`, the frozen tool's own refusal). The cross-links to
diagnostics are part of the aggregate and were therefore not produced either.

## Freeze, development and tools

- Frozen before the first witness: 26 files — every tool, the vocabulary and its renderings, and the expectations
  (`FREEZE-RECORD.txt`, `receipts/40-FREEZE.json`). Every invocation re-verified them first; nothing frozen changed.
- Before the freeze: supervision self-test 6/6 (`receipts/05-supervision-selftest`); classifier controls 38/38
  (`receipts/25`). Executor tool defects found and repaired before the freeze, none in Nexus, defective versions kept
  (`development/DEVELOPMENT-LOG.txt`) — among them a vacuous control harness (every control passed regardless) caught
  from its own output, and a vacuous literal check in check A.
- Carried byte-identical from Stage 1: `tools/verify_tree.py`, `tools/scope-wrap.sh`, `tools/report.py`.

## Limitations and unresolved items

1. **F-R did not complete within 30,000,000 states** (26,539,923 expanded when stopped). How F-R is to be closed
   (ceiling, an admitted quotient, or otherwise) is an Architect decision; nothing was attempted here.
2. No branch accounting exists for F-R, and no twenty-run matrix or diagnostic cross-links exist.
3. Original graphs of F-3, F-PT and F-B were never explored (Stage 1); their tables are of the composed quotient.
4. The vocabulary is conservative by type domain (e.g. `append_cell U`, every Persistence × Reason pair); many rows are
   not expected to be reachable, and nothing requires them to be taken.
5. Informational: F-W discovered exactly F-2's 12,602,670 states with 548,598 more transitions. Recorded, not
   interpreted.
6. The selection listings run the pinned executable with `--list` outside the scope, as Stage 1 did.
7. Every result is of the explored model and stays conditional on A-1, A-2 and AQ6 (AD-1-R4 J).

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. With the mission's evidence root and the Stage-1 commit available, the
frozen tools re-run read-only: `tools/vocab_generate.py --compare vocabulary/VOCABULARY.json`,
`tools/vocab_check_sites.py …`, `tools/vocab_check_eval.py …`, `tools/check_witness.py`, `tools/check_full.py`
(on a copy of a receipt directory), `tools/final_gate.py`.
