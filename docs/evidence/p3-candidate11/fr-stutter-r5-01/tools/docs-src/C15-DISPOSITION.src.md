# C15 — disposition: the mutant stays on the original explorer

**EXECUTOR RECORD — NOT ARCHITECT ACCEPTED.** Mission P3-C11-DR01-D2A1-FR-STUTTER-R5-01, section 16.

## The control, unchanged

| Field | Value | Source |
|---|---|---|
| Id | C15 | contract line 713 (sha256 `cac65351…3b3c`) |
| Change | a cell operation ignores the occupancy number | contract line 713 |
| Named property | E8 — "An operation given a stale token answers *stale* and leaves the core and the word exactly as they were." | contract 7.4 |
| Configuration | F-R (`full_fr`, original explorer) | contract line 713; AD-1-R4 E.3 row 15 |
| Ceiling | 30,000,000 states, original explorer | contract line 855; AD-1-R4 F (every configuration but F-1) |

C15 is not waived, not moved to another configuration, and not moved to another explorer.

## Its failing receipt

- The original explorer, run as `full_fr` selected alone with `--exact` and `--ignored` on the pinned C15 mutant
  executable (AD-1-R4 F.1 type (a); I items 1–6), under the existing 30,000,000-state F-R ceiling.
- It kills only if that run FAILs E8 within the cap: the printed discovered count is at most 30,000,000 (AD-1-R4 K.1).
- A FAIL found once the discovered count exceeds 30,000,000 is INCOMPLETE — BUDGET EXCEEDED, not admissible, and kills
  nothing. An INCOMPLETE run kills nothing. A PASS kills nothing. In each case 7.5's stop rule applies.
- No projection, quotient or refinement argument is ever C15's failing receipt, and C15 never runs under a quotient.

Why the mutant needs no refinement: the refinement concerns the unchanged core only. A mutant is a model change, which
voids the source facts the proof rests on (proof, section 11, condition 1); it is judged only by its own
original-explorer run.

## Its restored side

AD-1-R4 F requires, for every configuration but F-PT and F-B, "the original explorer's PASS on the exact final
commit". No such receipt exists for F-R (its exact-final run is INCOMPLETE). AD-1-R5 (draft) proposes, for C15 only:

- restoration to Candidate C verified independently after the mutant run (AD-1-R4 I item 7: every changed file's blob
  equal to C's, the worktree clean, nothing else differing);
- then the restored unchanged-core side may cite **the F-R refinement discharge record**: the accepted exact-final F-2
  original-explorer PASS, the accepted FR-1 to FR-17 proof, and the recording "F-R — UNCHANGED-CORE OBLIGATIONS
  DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF", all bound to the same exact frozen baseline;
- only when the exact frozen baseline matches, the source proof is accepted, the exact-final F-2 receipt is accepted,
  restoration is independently verified, and the Owner has adopted AD-1-R5.

The record is not a receipt of a run, has no outcome line, and is never PASS.

## Status now

| Item | Status |
|---|---|
| C15 mutant source, build, run | NOT MADE; NOT AUTHORISED by this mission |
| C15 kill | NOT ESTABLISHED |
| C15 restored side | under AD-1-R4: unsatisfiable while F-R's exact-final run is INCOMPLETE; under AD-1-R5 (draft, not adopted): the discharge record above, once every prerequisite holds |
