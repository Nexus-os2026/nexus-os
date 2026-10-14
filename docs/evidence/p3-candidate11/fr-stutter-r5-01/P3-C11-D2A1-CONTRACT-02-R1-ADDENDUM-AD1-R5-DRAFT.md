# P3-C11-D2A1-CONTRACT-02-R1 — ADDENDUM AD-1-R5 — DRAFT

**STATUS: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. NOT IN FORCE.**

**AD-1-R4 remains governing until the Owner adopts R5.** AD-1-R4 is the Owner-adopted amendment of record
(sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693`, blob `35d5074c2daa2ce9602fee3cd23dc0ccd6aef5b1`,
adoption record `docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md`). This file does not edit it.

Prepared by the executor (Claude Code, Claude Opus 5.5) under P3-C11-DR01-D2A1-FR-STUTTER-R5-01, for the Sole
Architect's independent review. It is a narrow **supplement** to AD-1-R4, not a consolidation: AD-1-R4 and this file
are read together, and every clause of AD-1-R4 that this file does not name stays exactly as AD-1-R4 states it. The
Owner keeps authority over every contract amendment and its adoption. Until the Architect has reviewed this text and
the Owner has adopted it, the contract P3-C11-D2A1-CONTRACT-02-R1 (sha256
`cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`) applies as amended by AD-1-R4 alone.

**Its one purpose.** To permit an accepted F-R stuttering-refinement proof to discharge F-R's unchanged-core 8.3
obligation. It alters only the F-R-related obligations that this route needs, and C15's restored side.

Nothing here does any of the following:

- lowers a fault budget, drops an actor or a property, or changes any configuration's bounds;
- counts an INCOMPLETE run as PASS, or relabels the F-R receipt;
- moves any mutation control off the original explorer, or raises any ceiling;
- makes any configuration other than F-R proof-dischargeable;
- creates a general permission to replace an original run with a refinement argument;
- authorises any run, mutation, source change, manifest, adoption or integration.

---

## 0. Basis

| Item | Identity | Status |
|---|---|---|
| Contract | P3-C11-D2A1-CONTRACT-02-R1, sha256 `cac65351…3b3c`; F-R's row, line 851: "F-R \| as F-2 \| … \| REPLAY on, three times"; C15's row, line 713: "C15 \| a cell operation ignores the occupancy number \| E8 \| F-R" | PROVEN FROM SOURCE (authenticated copy) |
| AD-1-R4 | as above; E.3 row 15: "`full_fr` (4256) \| `explore`, F-R \| REQUIRED_FINAL_RUN \| C15" | Owner-adopted; governing |
| Candidate C | commit `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`; `protocol.rs` `a2df2841…`, `model_tests.rs` `28085ce2…`, `unit_tests.rs` `55dc6640…` | frozen |
| F-2 receipt | Stage-1 `f023cc3d…`, `receipts/41-full-F-2/raw-output.txt`, sha256 `985a3976…4ab5`: PASS, 12,602,670 states, every applicable property HOLDS, D1 HOLDS, L1 HOLDS | ARCHITECT-RULED: accepted exact-final unchanged-core configuration (FINAL-EVIDENCE-STAGE2 ruling) |
| F-R receipt | Stage-2 `89326b85…`, `receipts/69-full-F-R/raw-output.txt`, sha256 `78e44c33…8ccd`: INCOMPLETE, 30,000,001 discovered, 26,539,923 expanded, at the 30,000,000-state cap | ARCHITECT-RULED: INCOMPLETE — STATE CEILING REACHED; establishes no property |
| Exact-final diagnostic | `e8_holds_for_stale_worker_operations`, `... ok` in Stage-1's passing nonignored suite (`receipts/30-suite/raw-output.txt`, sha256 `d6f58742…a3b3`) | EXECUTOR-REPORTED; supporting evidence only |
| Proof | `FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md` (this packet): FR-1 to FR-17, FR-RS, FR-17.X, PROJ-1 to PROJ-3 | EXECUTOR ARGUMENT — NOT ARCHITECT ACCEPTED |
| Occurrence closure | `REPLAY-OCCURRENCE-MATRIX.tsv`, two independent checkers in agreement | EXECUTOR-REPORTED |
| Branch accounting | `FR-BRANCH-DISPOSITION.tsv`: 193 rows, each with status and provenance | EXECUTOR-REPORTED |

---

## 1. Supplementary supersession table

The effects have AD-1-R4's meanings (REPLACED, SUPPLEMENTED, CLARIFIED). Each row's scope is F-R, `full_fr` or C15
only. Every other clause of the contract and of AD-1-R4 stays in force exactly as AD-1-R4 states it.

| # | Clause | Effect, with its scope (F-R, `full_fr` or C15 only) | Section |
|---|---|---|---|
| S5-1 | AD-1-R4 E.3 row 15 (`full_fr`) | REPLACED, for the unchanged-core disposition of `full_fr` only: the row's new text is B's. The test, its configuration, its explorer and its mutant runs (C15) are unchanged. | B |
| S5-2 | AD-1-R4 E.2, row "831, every one" | SUPPLEMENTED for F-R only: F-R's one required unchanged-core run is still `full_fr` on the original explorer at the exact final commit, and its receipt is kept as it is (INCOMPLETE); F-R's unchanged-core 8.3 obligation is then discharged by C's prerequisites. "Missing evidence for any of them is a failed mission" stands; while any prerequisite of C is missing, F-R's evidence is missing. | A, B, C |
| S5-3 | Contract line 863 (AD-1-R4 S-9): "states and transitions"; "for each branch of each operation, whether any run took it" | SUPPLEMENTED for F-R only: F-R's states and transitions are the INCOMPLETE receipt's printed counts, labelled as the counts of an incomplete run; no total is stated. F-R's branch coverage is the proof-based disposition of D, with its provenance labels. `full_fr` is listed with its receipt and with this disposition. | A, D |
| S5-4 | Contract line 923 (AD-1-R4 S-18): "every run of 8.2 and 8.3 is PASS" | CLARIFIED (consequential) for F-R only: read per E as amended by S5-1; F-R's run is not PASS and is not counted as PASS; F-R's line is met only by D's recording, once C holds. Using the label remains the Owner's decision. | B, C, D |
| S5-5 | AD-1-R4 Q, "A configuration PASSes when every applicable line HOLDS" | SUPPLEMENTED for F-R only: F-R does not PASS. Its unchanged-core status is recorded only in D's words. | D |
| S5-6 | Contract 7.4, the claims table, for F-R (Safety: every configuration; L1: every configuration without DROP) | CLARIFIED for F-R only: F-R's safety, D1 and L1 claims are DERIVED through the accepted proof from the accepted exact-final F-2 PASS; they are not measured on F-R's graph. | C, D |
| S5-7 | Contract line 853 (AD-1-R4 S-5), AD-1-R4 F ("for every other configuration: the original explorer's PASS on the exact final commit") and AD-1-R4 I item 7 (the restored PASS receipt) | SUPPLEMENTED for C15 only: C15's restored side may cite the F-R refinement discharge record of F, after verified restoration. C15's failing receipt is unchanged. | F |

### Not superseded

Every other clause stays in force, including: every property of 7.3 and 7.4; every fault budget, actor and bound of
8.3, F-R's included; lines 855, 857, 859, 865 and 940–942 (stop conditions), as AD-1-R4 reads them; line 831 as
AD-1-R4 reads it, with S5-2's supplement for F-R only; AD-1-R4's
A to R other than the rows named above; E.1 (no mutation control under a quotient), E.3 rows 1–14 and 16–34; F's
mutant rule (every control that names a configuration runs the original explorer on the mutant, under that
configuration's ceiling); F.1 and F.2 (receipt types and witness identity); G; H; I (other than item 7 for C15);
J; K (ceilings and classification); M (historical receipts keep their verdicts); N; O; P; and R.

---

## A. The F-R receipt stays INCOMPLETE

**Text.** "The exact-final F-R receipt (`full_fr`, original explorer, Candidate C, Stage-2 `receipts/69-full-F-R`) keeps
its verdict: INCOMPLETE, 30,000,001 discovered, 26,539,923 expanded, the 30,000,000-state cap reached. It establishes
no property. It is not continued, rerun, relabelled or completed by this addendum, by the proof or by any later
equivalent argument (AD-1-R4 M applies to it). Its printed counts are the counts of an incomplete run and are never
reported as F-R's total."

Line 942 is unchanged. The F-R run stopped the mission that made it, which reported it (FINAL-EVIDENCE-STAGE2,
REMAINING_CONFIGURATION_BLOCKED); the Sole Architect then ruled on the route (AD-1-R4 E.4 rule 3). This addendum does
not reclassify that run. While Candidate C is the baseline, that receipt is F-R's required final run: this addendum
asks for no second run of `full_fr` on the unchanged core. Any run of `full_fr` on the unchanged core at another commit,
or again at C, is reported and stops for a ruling exactly as line 942 and E.4 rule 3 state.

## B. E.3 row 15 — the replacement

**Text.** Row 15 of AD-1-R4 E.3 reads:

| # | Test (line at 42dd1bd1) | Runs | Proposed disposition | Mutant runs (F) | Reason or condition |
|---|---|---|---|---|---|
| 15 | `full_fr` (4256) | `explore`, F-R | REQUIRED_FINAL_RUN, its receipt kept as printed; **its unchanged-core 8.3 obligation is DISCHARGED_BY_ACCEPTED_REFINEMENT_PROOF when every prerequisite of AD-1-R5 C holds** | C15 (original explorer, 30,000,000; F) | the exact-final run is INCOMPLETE (A); if any prerequisite of C fails, row 15 reads as AD-1-R4 states it, F-R's evidence is missing, and line 831's consequence applies |

"`full_fr` remains the original contract test. Its unchanged-core 8.3 obligation may be discharged by the accepted
FR-1 to FR-17 stuttering-refinement proof when every R5 prerequisite holds. The discharge is not a run, is never PASS,
and supplies no state or transition count."

## C. Prerequisites — all required

1. The exact frozen baseline: Candidate C `d5964ee5326c2733177307db6f507328023d361a` (tree `51bac5fd…`), with the
   cited blobs of `protocol.rs`, `model_tests.rs` and `unit_tests.rs`, and the 49 Rust inputs of the pinned executable
   unchanged. Any change to any of them voids the discharge until the proof is reviewed again for that change.
2. The accepted exact-final F-2 original-explorer PASS (`full_f2`, Stage-1 `receipts/41-full-F-2`, raw sha256
   `985a3976713bc77bdf358744ec770e6220310f2ce2ed72b3e9ae10b3fb314ab5`): PASS; every applicable property HOLDS; D1
   HOLDS; L1 HOLDS.
3. The exact-final diagnostic identified in the proof's FR-10: `e8_holds_for_stale_worker_operations` `... ok` in the
   passing exact-final nonignored suite (Stage-1 `receipts/30-suite`), as supporting evidence.
4. Accepted source-occurrence closure: `REPLAY-OCCURRENCE-MATRIX.tsv`, REPLAY_OCCURRENCE_CLOSURE_COMPLETE, by two
   independently implemented checkers in agreement.
5. The accepted FR-1 to FR-17 proof, with FR-RS, FR-17.X, PROJ-1 to PROJ-3 and the invariants on which they rest.
6. Complete proof-based branch accounting: every one of the 193 vocabulary rows with an F-R status and a provenance,
   none NOT_ESTABLISHED (`FR-BRANCH-DISPOSITION.tsv`).
7. Independent Sole Architect review of 1 to 6, recorded by the Architect.
8. Owner adoption of this addendum.

If any of them fails, nothing is discharged and AD-1-R4 governs F-R unchanged.

## D. Recording

**Text.** When C holds, F-R's unchanged-core 8.3 obligation is recorded in exactly these words:

> F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF

with, beside it: the F-R receipt's verdict (INCOMPLETE) and its printed counts as an incomplete run's; the F-2 receipt
on which the discharge rests; and the branch accounting of `FR-BRANCH-DISPOSITION.tsv` with each row's provenance
(MEASURED_IN_COMPLETE_F2, MEASURED_IN_FR_PREFIX_ONLY, MEASURED_IN_EXACT_FINAL_DIAGNOSTIC, DERIVED_BY_FR_PROOF). A
prefix-only observation is never recorded as a complete F-R measurement.

Never written, anywhere, about F-R:

```text prohibited-wording
F-R PASS
F-R ORIGINAL GRAPH PASSED
F-R ORIGINAL GRAPH EXPLORED
```

Nor may any report state a total state count, a total transition count, a completion ceiling or a bound for F-R's
original graph.

## E. Preserved

All of the following stay exactly as the contract and AD-1-R4 state them:

- every F-R fault budget (2), admission budget (2), gating caller (2), replay budget (3) and every other bound of F-R;
- every actor, F-R's REPLAY included;
- every property of 7.3 and 7.4, and E10's rule;
- D1 and L1 as properties of the explored graph;
- the trace requirements: S-19's reading of line 609 and D's replay requirement for any FAIL trace;
- the original-explorer status of every F-R mutant run (F);
- the 30,000,000-state ceiling of every mutation run on F-R;
- every other clause of AD-1-R4.

## F. C15

**Text.** "C15 — 'a cell operation ignores the occupancy number', named property E8, configuration F-R — is unchanged in
definition, property and configuration.

- Its failing receipt is a type (a) receipt of the ORIGINAL explorer: `full_fr` selected alone with `--exact` and
  `--ignored` on the pinned C15 mutant executable, under F-R's ceiling of 30,000,000 states.
- It kills only if that run FAILs E8 within the cap: its printed discovered count is at most 30,000,000 (AD-1-R4 K.1).
  A FAIL found during the overshoot is INCOMPLETE — BUDGET EXCEEDED and kills nothing. An INCOMPLETE run kills nothing.
  A PASS kills nothing. In each case 7.5's stop rule applies.
- No projection, quotient or refinement argument is ever its failing receipt, and C15 is never run under a quotient.
- Its restored side: after the mutant run, restoration to Candidate C is verified independently (every changed file's
  blob equal to C's, the worktree clean, nothing else differing; AD-1-R4 I item 7), and the restored unchanged-core
  side may then cite **the F-R refinement discharge record**: the F-2 receipt of C(2), the accepted proof of C(5), and
  the recording of D, bound to the same exact frozen baseline. This record is not a receipt of a run, has no outcome
  line, and is never PASS.
- That citation is permitted only when: the exact frozen baseline matches; the source proof is accepted; the
  exact-final F-2 receipt is accepted; restoration to Candidate C is independently verified; and the Owner has adopted
  this addendum."

C15 is not waived, not moved to another configuration and not moved to another explorer.

## G. No other configuration

**Text.** "Only F-R's unchanged-core 8.3 obligation can be discharged under this addendum. No other configuration —
X-1 to X-5, F-1, F-2, F-3, F-4, F-5, F-6, F-7, F-8, F-PT, F-B, F-B2, F-G, F-W or F-D — becomes proof-dischargeable
through it; each keeps the disposition AD-1-R4 gives it."

## H. No general permission

**Text.** "This addendum creates no general permission to replace an original run with a refinement, projection or
equivalence argument. Any other such use needs its own audit, its own amendment and the Owner's adoption."

---

## Open items (none decided by the executor)

1. The Sole Architect's independent review of the proof, the occurrence closure, the branch accounting and this text.
2. The `name` field: "F-R" and "F-2" differ in the configuration's report label (proof, section 2.1, decision D-1).
   Whether that satisfies "every other field mechanically equal" is the Architect's ruling.
3. The Owner's adoption of this addendum.
4. C15's own mission: preparing its mutant, running `full_fr` on it under 30,000,000, and verifying restoration. None
   is made or authorised here.

## Changes from AD-1-R4

| Section of AD-1-R4 | AD-1-R4 | AD-1-R5 (supplement) |
|---|---|---|
| E.3 row 15 | `full_fr`: REQUIRED_FINAL_RUN | REQUIRED_FINAL_RUN, receipt kept; unchanged-core 8.3 obligation discharged by the accepted proof when C holds (S5-1) |
| E.2, line 831 row | every configuration's run is the original explorer's (or E.1's quotient) | F-R: the run is kept as INCOMPLETE; its obligation is discharged under C (S5-2) |
| S-9 / line 863 | states, transitions, branch coverage per run | F-R: the incomplete run's counts as such; proof-based branch accounting with provenance (S5-3) |
| S-18 / line 923 | every run of 8.3 PASS, per E | F-R: met only by D's recording under C; never counted as PASS (S5-4) |
| Q | a configuration PASSes when every applicable line HOLDS | F-R does not PASS; D's recording (S5-5) |
| 7.4 claims for F-R | measured by F-R's own PASS | DERIVED through the accepted proof from F-2's PASS (S5-6) |
| F, I item 7, line 853 for C15 | restored receipt: the original explorer's PASS of F-R | C15 only: the F-R refinement discharge record after verified restoration; failing receipt unchanged (S5-7) |
| everything else | — | unchanged |
