# AD1R3-DISPOSITION-01 — proposed dispositions supplementing the AD-1-R3 draft

**STATUS: DRAFT — NOT ADOPTED — NOT APPLIED. NOT IN FORCE.** Prepared by the executor (Claude Code, Claude Opus
5.5) under P3-C11-DR01-D2A1-C4-WITNESS-01 for the Sole Architect's review. It decides nothing. The Owner keeps
authority over every amendment.

**Supplements exactly:** AD-1-R3 draft, blob `7f2463f5421502cc6ee74c070fc3abc41d467770`, sha256
`485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e` (evidence commit `c2c69e3a`). That draft is not
changed or regenerated. No AD-1-R4 is made. "R3 n" below is that file's line n. ARCHITECT-RULED marks this
mission's rulings. Everything else is proposed.

## 1. R3-01: separate artifact identities for the two witness roles

**Defect.** The witness's executable rejection is "an executable other than the mutant executable of (a)" (R3 806).
G.3(c) applies it to the invocations of (b) and (d) (R3 798). But (d) must run on the pinned executable of the
exact final baseline (R3 809). Read literally, every healthy witness is rejected. I (R3 934) names only the mutant
executable's reuse.

**Disposition.**
- **Mutant witness (G.3(b)):** the same pinned mutant executable as its control's primary original-explorer
  invocation (G.3(a)), and that mutant's source identity (I items 1–3). Its sha256 is equal before and after.
- **Healthy witness (G.3(d)):** the pinned unchanged-baseline executable of I (the one fresh optimised build of the
  exact final baseline). Its sha256 is equal before and after.
- R3 806 is replaced by: "an executable other than the one its role requires: for (b), the mutant executable of
  (a); for (d), the pinned unchanged-baseline executable".
- **Common to both roles, and mandatory:** fixture-failure rejection; unrelated-panic rejection; exactly one named
  test with `--exact`; no compilation failure; source and executable identity; complete raw output; and K.2
  points 5–7 (exit or signal, launcher termination and resource records, completion).
- **Role-specific expectations:**

| Role | Executable | Expected harness result | Required markers |
|---|---|---|---|
| (b) mutant | as G.3(a) | the witness's own verdict panic, `1 failed`, exit 101 by a normal exit | the exact positive named-property line at its locus |
| (d) healthy | baseline | `ok`, `1 passed`, exit 0 by a normal exit | the exact healthy line; no positive marker |

  A positive marker in a (d) receipt, or a healthy line in a (b) receipt, is NOT ADMISSIBLE.
- **Alignment.**
  - F.1(c) (R3 708): (b) is on the mutant executable and (d) on the baseline executable, each as above.
  - I (R3 934): adds "the healthy witness runs on the pinned baseline executable".
  - K.2 case 3 (R3 1054) stays for (b). A new case is added for (d): the healthy line, exit 0, the baseline
    executable gives PASS. Either case on the other role's executable gives NOT ADMISSIBLE.

## 2. R3-02: separate PASS receipt types

**Defect.** F.1(e) (R3 725) covers every 8.2/8.3 run and every restored receipt of 7.5, and demands an explorer
report of all of them. Three kinds of receipt cannot produce one:
- the restored receipts of C14, C19, C21, C27 and C28, which are named unit tests;
- C22's restoration, which is a healthy build (R3 718, 948);
- G.3(d), which is a witness PASS.

K.2's exemption (R3 1032) names only (b) and (c). Case 1 (R3 1052) is an exploration row.

**Disposition.** F.1(e) is split into four types. Every original obligation stays: each restored receipt is still
required, and only its evidence type is named.

- **(e1) Unchanged-core exploration PASS.**
  - Covers every run of 8.2 and 8.3, and the restored receipt of every control that names a configuration.
  - Keeps all of F.1(e) as it stands: `outcome: PASS`, every line of Q, the counts of D, the branch list, a count at
    most C, `1 passed`, exit 0 by a normal exit, and K.2 points 5–7.
  - Only this type requires explorer reports, state counts, property lines and branch coverage.
- **(e2) Restored named-unit-test PASS.**
  - Covers C14 T-BIND, C19 T-CELL, C21 W6, C27 T-PREC and C28 T-RES.
  - Requires: the pinned baseline executable; exactly that test with `--exact`, one match;
    `test <name> ... ok` and `test result: ok. 1 passed; 0 failed`; exit 0 by a normal exit; K.2 points 5–6; and
    the complete raw output with its sha256.
  - The explorer report, counts, property lines and branches are NOT PRODUCED BY DESIGN.
- **(e3) Healthy-witness PASS (G.3(d)).**
  - Requires: the pinned baseline executable (§1); exactly that test; the exact healthy line with no violation;
    exit 0 by a normal exit; K.2 points 5–6; and the complete output.
  - No explorer report is required.
- **(e4) Healthy build success.**
  - Covers C22's restoration (F.1(d)'s "healthy build evidence") and any other healthy build.
  - Requires: the pinned build command of N.7 on the unmutated baseline; exit 0 by a normal exit; the build's
    finished line; the toolchain; the complete compiler output; the source identity; and K.2 points 5–6.
  - Every artifact the build produces is recorded by path, sha256 and size.
  - The C22 exemption (NOT PRODUCED BY DESIGN: executable, test selection, test run) covers only the mutant's
    intentionally failing build. It never extends to a healthy build's outputs.

**Alignment.**
- F.1 (R3 669): "every receipt shows its own type", now with types (a)–(d), (e1)–(e4) and (f).
- (f) (R3 735): "two admissible receipts of type (e1)".
- G.3(d) (R3 809): its receipt is of type (e3).
- I item 7 (R3 948): the restored PASS receipt is of its control's type: (e1), or (e2) for the named tests, or (e4)
  for C22.
- K.2 (R3 1032): the exemption reads "(b), (c), (e2) and (e3)". For (e4), points 1, 2, 4 and 7 read as for (d), but
  with the intended zero exit and the recorded artifacts.
- K.2 cases: case 1 (R3 1052) applies to (e1) only. New cases are added for (e2), (e3) and (e4).
- No reference to "every restored receipt" under F.1(e) remains operative.

The receipt classifier of this mission implements these types (`tools/classify.py`). It was tested on synthetic
descriptors only (`receipts/03-classifier-synthetic-cases.txt`).

## 3. C4: proposed attribution route (for C4 only; NOT ADOPTED; not extended to any other control)

**ARCHITECT-RULED.**
- Row 7.5 C4 ("acknowledgement test 1 removed | P4, P8 | F-8") keeps its comma. Both P4 and P8 are preserved.
- The experimental C4 definition is the complete removal of Test 1's five checks in `Publish::acknowledgement`:
  session open, latch, Healthy phase, C/X occupancy, and the U-occupied invariant latch.
- Test 0, `let core = &mut *self.core;`, and everything from `let set = snapshot.set;` are kept.
- Patch `C4-test1-removed.patch`, sha256 `24ac7add3716a030dd710539e6082e197fdc5bd624c26a595b378b3971e0838d`: 16
  lines removed, 0 added. Mutant `protocol.rs` blob `f3793c2f`.
- The original explorer's first failure is predicted to be P4. That prediction is not a receipt.

**Proposed kill criterion (C4 only).** C4 would be killed when all of (a) to (e) hold, and recorded as (f) states.

- **(a) Primary, preserved.**
  - `full_f8`, selected alone with `--exact --ignored`, on the pinned C4 mutant executable, under F-8's ceiling.
  - A type (a) receipt under K.2. Its raw report is kept unedited.
  - Its printed property is recorded as it is. It is never relabelled. If it names neither P4 nor P8, that is
    reported as it is, and the Architect rules.
- **(b) Both properties demonstrated on the actual mutant.**
  - The C4 witness, on the same pinned mutant executable as (a).
  - Two separately initialised scenarios from the unchanged `initial(F8)`, with at most 32 actor steps each.
  - The exact lines `NAMED PROPERTY P4: OBSERVED on the world after step 8 of scenario P4 (…)` and
    `NAMED PROPERTY P8: OBSERVED on the world after step 12 of scenario P8 (…)`.
  - Both scenarios are collected before the verdict. Only P4 or only P8 is not this receipt.
- **(c) Violation-free healthy replays.** The same witness on the pinned baseline executable PASSes (type (e3)).
  Both scenarios show no violation:
  - the P4 append is refused (`CellsBusy`), consuming 0 of its 6 offered choices;
  - the P8 append is refused (`NotHealthy`), consuming 0 of its 4, and G1's written attempt stays in U.
- **(d) The unchanged core's F-8 receipt.** The required full F-8 PASS (type (e1)) of the unchanged core.
- **(e) Identity and review.** Every receipt is on the exact final baseline or its C4 mutant (I), followed by
  independent review.
- **(f) Recording.** `C4: P4, P8 (first failure <as printed>; witness)`.

**Offered-versus-consumed Oracle rule (proposed).**
1. Each scenario step is offered a fixed choice stream. Healthy and mutant executions receive the same actor
   sequence and the same offered stream at every step.
2. Before the changed acknowledgement, both cores must show identical, exact consumption of the whole offered
   stream, as frozen: each consumed choice with its option count, the branch labels and the remaining fault
   budget.
3. At the changed acknowledgement, exactly two planned ends are allowed:
   - the healthy core refuses before storage and consumes zero choices, leaving the whole stream unused;
   - the mutant consumes its complete frozen storage-choice sequence, leaving no suffix unused.
4. Every consumed choice must equal the offered prefix.
5. Consumption beyond the supplied stream is rejected. Otherwise the Oracle's default-zero fallback would silently
   satisfy an underspecified schedule.
6. Any other unused suffix is rejected, unless it is justified and frozen before execution.
7. Each step's offered choices, consumed choices with option counts, unused suffix, branch and remaining faults are
   recorded and verified twice: by the witness and by the classifier from the raw output.
8. Oracle and the original replay helpers are not changed.

P4 is shown with the simultaneous occupants U (the acknowledgement) and X (F1's explanation). P8 is shown with G1's
facts: its insert was issued, its record is present, it was not told Committed, it is not in U, and it has no
correction in C or in the ledger. The Owner's confirmation in these schedules is a simulated model actor's choice.

G.3 stays for C1 and C25 only. This route does not become an exception for any other control.

## 4. Other controls (recorded, not applied, not run)

- **C9 (ARCHITECT-RULED: C9-b selected).** In `Publish::rolled`, `Rollback::Unknown` arm: keep `self.failed();` and
  replace only `Persistence::Uncertain` with `Persistence::NotWritten`. Named E4 on F-B2. Not applied, built or run.
- **C10, C24.** The authenticated sealed CAMPAIGN-01 definitions (`controls_defs.py`, sha256
  `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f`) are retained unchanged for later
  preparation. Not applied, built or run.
- **C1, C25.** The prior source and evidence rulings are kept. No further execution of either witness or mutant.

## 5. Open for the Architect

1. Review of §1 and §2 as replacement text for the cited R3 lines, and of whether any other line refers to F.1(e).
2. The C4 route of §3, its loci and its Oracle rule; and whether the prototype witness (source commit A of this
   mission) is carried to the final baseline by its own mission.
3. Authorisation, separately, of the `full_f8` primary run (none is made here).
