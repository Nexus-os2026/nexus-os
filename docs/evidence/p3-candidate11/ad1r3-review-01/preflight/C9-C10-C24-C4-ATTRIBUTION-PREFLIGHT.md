# P3-C11-DR01-D2A1-AD1R3-READINESS-01 — Attribution preflight: C9, C10, C24 and C4

**READ-ONLY. NOTHING WAS EXECUTED.** No mutation was applied, built or run. No model, test, probe or exploration
was run. `controls_defs.py` was read as text and parsed with `ast` only; it was never imported or executed. Every
ordering below is a **prediction from source**. None was measured. Nothing here is a mutation kill, changes a
contract row, or extends AD-1-R3's G beyond C1 and C25.

Prepared by Claude Code (Claude Opus 5.5) for the Sole Architect's review.

## 0. Inputs (authenticated in receipts/00)

| Input | Identity |
|---|---|
| Contract P3-C11-D2A1-CONTRACT-02-R1 | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`; rows 7.5 at lines 704 (C4), 709 (C9), 710 (C10), 720 (C24); 4.8 at lines 432–444 |
| Binding reviews | REVIEW-02 `6dfb1456…cdce`, REVIEW-03-01 `1f964d75…fb35`. Neither mentions control C4, C9, C10 or C24, or "P4, P8" (searched; REVIEW-02 line 243's "NC-C9-P1-HEADER-SCROLLS-AWAY" is an unrelated identifier). |
| Source | commit `42dd1bd1cf21ffdea5968b256d2983f6575fab5b`; `protocol.rs` blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (sha256 `a5454944…a296`); `model_tests.rs` blob `5ad26ddc727d8be30f4fb8d7478bc69e1c974933`. `model_tests.rs` lines 1–4814 and `protocol.rs` are identical at 244c5b23 and 243501f0, where F-1 PASSed. |
| Sealed definitions | CAMPAIGN-01 `tools/stage5-prepared-NOT-EXECUTED/controls_defs.py`, sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f`, listed in CAMPAIGN-01's sealed SHA256SUMS (93/93 OK). Its README: "NOT EXECUTED … Nothing here is evidence. Kept for transparency only." It defines C1, C2, C10, C24, C25 (the historical compound encoding), C12, C14, C19, C21, C22, C27, C28. **It defines neither C9 nor C4.** An inert byte copy travels as `inert/controls_defs.py.txt`. |
| Earlier flag | F3-CAP60-01 `receipts/stage5/02-c1-c25-attribution.txt`, section D: "Examples to check: C10 and C9 against E4, C24 against P8. They were not analysed." |
| Historical context only | The scratch model's REPORT-R4.txt (p3-c11-d2a-model-01; sha256 `10312fb4b679f32f87d020a3fbe49888228fed3bd4d9fc5cdb884a0d78a35a5f`, equal to the copy inside its sealed r4 tarball). A different model. By contract line 865 nothing depends on it. |

Line numbers below are of `protocol.rs` (P) or `model_tests.rs` (M) at 42dd1bd1 unless stated.

## A. How the original explorer chooses what it reports (PROVEN FROM SOURCE)

- A1. `explore` (M 3882–4110) is breadth-first. Keys are expanded in discovery order (M 3906, 3915). The trace is the
  breadth-first parent path (M 3896–3904).
- A2. Expanding a key runs `state_properties` and the KEY check first (M 3812–3815). The first state violation stops
  the run (M 3917–3920). Then D1 if no step is enabled (M 3921–3931). Then the successors, in actor order (M 3817)
  and, within an actor, in the oracle's choice order (M 3819–3842). The first violation of the first violating
  successor stops the run (M 3932–3940). Exactly one property is reported (M 4067–4072).
- A3. State properties are checked in this order (M 3403–3511): E1, E3, P2, P4, then for each fact P5, P6, E6, then
  for each gating record P7 then P8, then P9, P13, E11. A transition's own violations are those its step pushes
  (for a fact append, E5 before E4: M 2426–2444; for a gating append, E5 then E4 or E7: M 3038–3101; for an
  acknowledgement, E5 then E4: M 3203–3236; for a worker operation, E5 for cell appends, E4 only for labels ending
  NotWritten or Stale or containing Refused, E8 only for a stale token: M 2901–2959), then `transition_properties`
  (E2, E9, E11: M 3514–3545).
- A4. A successor's state properties are judged only when that successor is itself expanded, later in breadth-first
  order (A2).
- A5. Choice order. `Oracle::next` enumerates choice sequences depth first, so lexicographically; option 0 is the
  fault-free one (M 546–580). `Io::pick` offers `[Ok, faults…, Unwound if the operation may unwind]` (M 652–666). A
  fact append's Io may unwind and may answer Constraint (M 2419). After `Commit::Unknown`, after `Rollback::Unknown`
  once an insert was issued, and after an unwind once an insert was issued, `undecided` explores **absent first**,
  then each holding as present (M 684–694). `Rollback::Undone` leaves nothing (A-1, M 746–752). A `Failed` insert
  stays marked as possibly written (M 704–728).
- A6. Actor order: the writers (one per fact, or one batch writer), SE, WK, the gating callers, the acknowledgement
  actors, ADM, then ENV and REPLAY where on (M 2332–2347). Under the healthy core F-1 has no violation anywhere in its
  graph (F-1 PASS, 46,524,336 states, at 244c5b23 and 243501f0: EXECUTOR-REPORTED; same source, A0). So on F-1 any
  violation of a mutant comes through a transition that the mutation changes.
- A7. The rollback answer is interpreted in one place: `Publish::rolled` (P 1289–1297). Its value sets a persistence
  only at P 1398 (fact append, step 5), P 1486 (gating), P 1687 (acknowledgement) and P 1913 (cell append).

## B. Per control

### B.1 C10 — "`Commit::Unknown` followed by `Rollback::Undone` gives Not written" — E4 on F-1

**Definition and origin.** Sealed in `controls_defs.py` (above): in `Publish::facts`, step 6, the arm
`Commit::Unknown => { self.failed(); self.rolled(io.rollback()); self.not_applied(Persistence::Uncertain, Reason::Storage) }`
(P 1404–1408; anchor occurs exactly once, `checks/03-defs-anchor-check.txt`) becomes
`let persistence = self.rolled(io.rollback()); self.not_applied(persistence, Reason::Storage)`, the one rollback still
issued. Target `full_f1`; property E4. The encoding was prepared by CAMPAIGN-01 and never approved or executed: it is
the only written definition, not an approved one.

**Configuration and property.** F-1 (M 155–161): two facts, none conflicting; faults 3; admissions 2; gating
callers 2; actors [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm]. E4 (contract 7.4): "If a result says Not written, the
simulated ledger is exactly as before the call." Checked inside `fact_append` on the transition (M 2434–2444).

**Operation and checks.** The mutation changes one path only: step 6 of a fact append that meets `Commit::Unknown`
and then `Rollback::Undone`. On that path the healthy core answers Uncertain; the mutant answers Not written. The
model has already chosen, at the commit, whether the record is present (A5). On the present branch the ledger changed
and the result says Not written: E4. The verb sequence is unchanged (commit, then one rollback), so E5, checked first
in the same step, is not affected. E2, E9 and E11 are not affected.

**Earlier violation?** None predicted. By A6 an earlier violation would need a changed transition, and the first
changed transition is the first fact append that meets this path. Fw(0) arms at depth 1; the first key expanded at
depth 1 is that one (actor 0). Its first successor is Fw(0)'s append. In choice order, the sequences before the
violating one are: commit Durable (Committed; F1 present); commit Unknown, absent, rollback Undone (Not written,
ledger unchanged: correct); commit Unknown, absent, rollback Unknown or unwound (Uncertain). None violates anything.
The next sequence violates E4.

**Candidate schedule (prediction).** 2 steps: `Fw(0)` arm; `Fw(0)` append of F1 with choices [no expiry, open Ok,
facts Ok, insert Ok, commit Unknown, present, rollback Undone] (one fault). Predicted report: `FAIL: E4: append_facts
said Not written but the ledger changed: …`, `shortest trace (2 steps):`. The discovered count would be tiny, far
below any ceiling (K.1).

**Facts, predictions, open points.**
- Established (source): the definition's text and its anchor; the changed path; the check order; the choice order.
- Predicted: the named property E4 is the first failure, at 2 steps. Not measured.
- Open: no definition of C10 has been approved; the run needs F-1 (`full_f1` has a source cap of 60,000,000, which
  is not an authorisation: AD-1-R3 A).

### B.2 C24 — "resolve empties U for a present gating record without placing the correction" — P8 on F-1

**Definition and origin.** Sealed in `controls_defs.py`: in `Publish::resolve_gating`, the arm
`Read::Got(Some(row)) if row.digest == key.digest => { … place the correction in C …; Resolve::Corrected(…) }`
(P 1774–1787; anchor occurs exactly once) becomes `{ let _ = (row, exit); Resolve::Absent }`. The function then
empties U and sets Healthy (P 1813–1815). Target `full_f1`; property P8. Prepared by CAMPAIGN-01; never approved or
executed.

**Configuration and property.** F-1, as B.1. P8 (contract 7.3), checked as a **state** property (M 3477–3492): a
gating attempt that issued an insert and was not told Committed, whose record is in the ledger, must be in U, or have
its correction in C or in the ledger.

**Operation and checks.** The mutation changes only `resolve` of a gating occupant whose record is present with the
same digest. Reaching it needs: a gating append that issues an insert, is not told Committed, writes its record and
keeps U (for example `Commit::Unknown`, present: P 1470–1477, U kept by the guard with its exit set: P 1984–1994,
phase barrier needed); the worker's barrier, which passes to *resolving* because U is occupied (P 1713–1729); and the
worker's resolve (P 1732–1763). Each worker operation takes three WK steps: read the agenda and choose, the [T] step,
the operation (M 2963–3016).

**Earlier violation?** None predicted. The resolve step itself pushes no violation: `worker_op` adds E4 only for
NotWritten, Stale or Refused labels, and E8 only for a stale token (M 2941–2959); "resolve: Absent" is neither, and
the token is present. E2, E9 and E11 are unaffected (one store operation, one exchange; no latch or table change).
The resulting state violates P8 and nothing earlier in A3's order: E1 holds (the guard publishes the core's flags: P
1995–2004), E3 and P2 are untouched, P4 holds (U is empty), P5 and P6 are untouched, and P7 holds (G1 was not told
Committed). By A6 no other changed transition exists. Every state discovered before it at its depth is a state of the
healthy graph or another state just after a mutant resolve, which also violates P8 first. So the first FAIL is P8, at
the expansion of the first such state.

**Candidate schedule (prediction).** 7 steps: `Gc(0)` append of G1 [open Ok, facts Ok, insert Ok, commit Unknown,
present, rollback Undone] (one fault; Refused Uncertain Storage; U kept); `Wk` agenda, Barrier; `Wk` [T]; `Wk`
barrier Passed; `Wk` agenda, Resolve; `Wk` [T]; `Wk` resolve: Absent (the mutant). Predicted report: `FAIL: P8: G1's
attempt is written, uncorrected and not in U`, `shortest trace (7 steps):`. F3-CAP60-01's concern (a transition
property before a state property) is answered from source: no transition property fires on that path.

**Facts, predictions, open points.**
- Established (source): the definition's text and its anchor; the changed path; that `worker_op` checks nothing that
  "resolve: Absent" triggers; the order of state properties.
- Predicted: P8 first, at 7 steps. Not measured. A different gating failure (insert Failed with rollback Unknown, or
  an unwind after the insert) gives the same depth.
- Open: no approved definition; F-1's authorisation, as B.1.

### B.3 C9 — "`Rollback::Unknown` is treated as `Undone`" — E4 on F-B2

**Definition and origin.** **NONE AUTHENTICATED.** No sealed or approved definition exists in the evidence root:
`controls_defs.py` lacks C9, and a search of the evidence root found no other definition. The only authenticated text
is the contract's row (line 709). Candidate encodings, for review only, **not definitions**:
- C9-a: in `Publish::rolled` (P 1292–1295), `Rollback::Unknown => { self.failed(); Persistence::Uncertain }` becomes
  `Rollback::Unknown => Persistence::NotWritten` (treated exactly as `Undone`, which also does not call `failed`);
- C9-b: as C9-a, but keeping `self.failed();` (only the persistence changes).

By A7 either changes every caller that uses `rolled`'s value: P 1398, 1486, 1687 and 1913. F-B2 has no gating
caller, no acknowledgement and no conflict (so no C or X occupant to append), so only P 1398 is reachable there.

**Configuration and property.** F-B2 (M 227–233): two facts, none conflicting, one writer offering both as a batch;
faults 3; admissions 1; no gating caller, no acknowledgement; actors [Fw(0), Se, Wk, Adm]. E4 as in B.1.

**Operation and checks.** Step 5 of the batch append (P 1384–1400): an insert fails (`Failed` or `Constraint`), the
core calls `failed()` (P 1397) and then `rolled(io.rollback())` (P 1398). The model then explores, after
`Rollback::Unknown`, both absent and present for every holding (A5): the rows whose inserts succeeded, and, if one
insert answered `Failed`, every issued row. The healthy core answers Uncertain. The mutant answers Not written. On a
present branch the ledger changed: E4 (M 2434–2444). The verb sequence is unchanged, so E5 is not affected.

**Earlier violation?** None predicted on the changed transitions. In F-B2's first expanded key at depth 1 (after
`Fw(0)` arms), the batch append's choice sequences before the violating one are: both inserts Ok with commit Durable
(Committed, both present); both inserts Ok with commit Unknown, absent or present, with any rollback answer (step 6
ignores `rolled`'s value and answers Uncertain); both inserts Ok with an unwound commit (the guard counts Uncertain).
In C9-a and C9-b alike `failed()` has already run on these paths (P 1397, 1405), so the two encodings do not differ
here. The next sequence violates E4. **Limit:** unlike F-1, the unchanged core has never been run on F-B2 (AD1R1
stage6 matrix: "never run"). That no unchanged transition at depth 1 or 2 violates a property is argued from source
for the transitions listed here; it is not shown by a receipt.

**Candidate schedule (prediction).** 2 steps: `Fw(0)` arm; `Fw(0)` append of [F1, F2] with choices [no expiry, open
Ok, facts Ok, facts Ok, insert(F1) Ok, insert(F2) Constraint, rollback Unknown, F1 present] (two faults; the core
latches for the constraint). Predicted report: `FAIL: E4: append_facts said Not written but the ledger changed: …`,
`shortest trace (2 steps):`. Insert(F2) `Failed` in place of `Constraint` gives the same depth, later in choice order.

**Facts, predictions, open points.**
- Established (source): there is no authenticated C9 definition; `rolled` is the only interpretation of a rollback
  answer; the changed path; the check and choice orders.
- Predicted: E4 first, at 2 steps, under either candidate encoding. Not measured.
- Open: C9's definition must be written and approved before any run. F-B2's unchanged core has never been run.

### B.4 C4 — "acknowledgement test 1 removed" — "P4, P8" on F-8

**Definition and origin.** **NONE AUTHENTICATED.** `controls_defs.py` lacks C4; no other definition was found. The
contract defines the test itself (4.8, line 437): "open; not latched; Healthy; C and X empty. (U occupied while
Healthy: latched, as 4.5.)" In the code that is the block `// Test 1.` (P 1625–1642): session open, not latched,
Healthy, C and X empty, and U occupied latching for a cell invariant. Candidate encoding, for review only, **not a
definition**: C4-a, delete that block entirely, keeping `let core = &mut *self.core;`. A partial encoding (for
example one that keeps the U latch) would not be "test 1 removed"; the prediction below does not depend on it.

**Configuration and properties.** F-8 (M 207–216): two facts, F1 conflicting; faults 2; admissions 2; one gating
caller; one acknowledgement actor with one attempt; actors [Fw(0), Fw(1), Se, Wk, Gc(0), Ack(0), Adm]. Initial ledger:
Prepared(F1), Holder(F1), Prepared(F2) (M 1085–1100): no explanation row. P4 (7.3): never, at a step boundary, U
occupied together with C or X; a **state** property (M 3442–3444). P8 as in B.2.

**What the removal permits.** `Core::place` overwrites the cell whatever it holds (P 1139–1149), and the guard
empties U when the operation does not keep its occupant (P 1984–1987). Without test 1 an acknowledgement can:
- enter while C or X is occupied; if it keeps U (`Commit::Unknown`, or a failed insert with an unknown rollback, or an
  unwind after the insert: P 1680–1704, 1966–1983), U and C or X are both occupied at the step boundary: **P4**;
- enter while U holds a kept gating attempt (phase barrier needed or resolving); placing the acknowledgement in U
  removes the gating occupant; if the acknowledgement then ends without keeping U, U is emptied; with G1's record
  written and uncorrected: **P8**.
`step_ack` pushes only E5 and E4 checks (M 3203–3236), and neither fires on these paths.

**Which is first (prediction).** An acknowledgement needs a snapshot, and a snapshot needs an explanation row above
the frontier (P 1599–1610; M 449–464). The only source of one in F-8 is F1's conflict: Fw(0)'s append is refused as a
DuplicateRole conflict (P 858–864), which places the explanation in X (P 1221–1224); the worker appends X (three WK
steps). So the earliest acknowledgement append is at depth 8, and at that depth X is still occupied (emptying X takes
three more WK steps). Shortest P4 schedule: `Fw(0)` arm; `Fw(0)` append F1 (Refused, Conflict; X built); `Wk` agenda,
AppendX; `Wk` [T]; `Wk` append X Committed; `Ack(0)` snapshot Taken; `Ack(0)` the Owner confirms; `Ack(0)` append
[open Ok, recheck Ok, insert Ok, commit Unknown] (one fault; U kept while X occupied). The P4 state is at depth 8.
The shortest P8 schedule needs X emptied (three more WK steps) before the gating append can enter, then the gating
failure, then the acknowledgement: depth 12 or more. Every violating state at depth 8 violates P4 (and nothing
earlier in A3's order). By A2 the run stops at the first. **So the original explorer is predicted to report P4 and
never P8.** With a partial encoding that keeps the U latch, the P4 schedule is the same (U is empty there).
Historical context only: the scratch model's NC18 ("k_append test 1 removed entirely. Expected P9, P4 or P8. RESULT:
P4, P8 and P9 VIOLATED.") saw P4 at its step 5 and P8 at its step 8. That model collects every violation; D2A-1's
explorer reports one.

**The wording "P4, P8".** Other rows with two properties say "or": C3a "P13 or P15", C3b "P13 or P12", A6 "P12 or
P15". C4 alone uses a comma. Neither binding review mentions C4. Two readings, neither adopted here:
- **Conjunctive** ("P4 and P8 must both fail"). One original-explorer receipt shows one property (A2). If P4 is
  first, as predicted, no admissible type (a) receipt can show P8. The row could then be met only through an
  additional named-property route for P8: an attribution exception like the C1/C25 witness of G.3. That would need its
  own design, source mission and review. **It is not designed or implemented here.** Absent it, a conjunctive reading
  makes C4 unkillable on its configuration, and line 941 would stop the mission.
- **List or disjunctive** ("P4 or P8"). The predicted P4 FAIL would suffice under F.1 type (a). That reading changes
  the row's comma into "or", so it needs an explicit Architect ruling and a supersession row. It is not made here.

Neither P4 nor P8 is waived. The C1/C25 witness exception is not extended to C4.

**Facts, predictions, open points.**
- Established (source): no authenticated C4 definition; test 1's contract text and code block; `place` overwrites;
  the guard empties U; the snapshot precondition; F-8's initial ledger; the explorer reports one property.
- Predicted: P4 first, at depth 8; P8 not reported by the explorer. Not measured. The unchanged core has never been
  run on F-8 ("never run"): absence of earlier unchanged violations at depth 8 or less is argued, not shown.
- Open: C4's definition; the reading of "P4, P8" (Architect decision); if conjunctive, whether and how a P8
  attribution route is designed.

## C. Attribution matrix

| Control | Definition | Configuration | Named | Predicted first failure | Depth (steps) | Named attribution by explorer? | Exception needed? |
|---|---|---|---|---|---|---|---|
| C10 | sealed (CAMPAIGN-01), not approved; anchor P 1404–1408 ×1 | F-1 | E4 | E4 (transition, M 2436) | 2 | yes, if the prediction holds | no |
| C24 | sealed (CAMPAIGN-01), not approved; anchor P 1774–1787 ×1 | F-1 | P8 | P8 (state, M 3486) | 7 | yes, if the prediction holds | no |
| C9 | **none**; candidates C9-a, C9-b | F-B2 | E4 | E4 (transition, M 2436) under either candidate | 2 | yes, if the prediction holds | no; a definition must be approved |
| C4 | **none**; candidate C4-a | F-8 | "P4, P8" | P4 (state, M 3443); P8 not reported | 8 | P4 only | **for Architect review**: none if the row is read "P4 or P8" (by explicit ruling); a P8 route if read "P4 and P8" |

For C1 and C25 (for comparison; G.3 governs): predicted first failures E3 and E4, which are not the named P1 and P7;
the witnesses supply the named property.

## D. Additional attribution exception for Architect review

Only C4, and only under the conjunctive reading of "P4, P8". For C9, C10 and C24 the named property is predicted to
be the explorer's first failure, so no exception is predicted to be needed. If a run shows otherwise, F.1 type (a)
and line 941 apply as written: the run is reported as it is and the mission stops. No exception arises
automatically, and G is not broadened.

## E. Limits

- Every ordering, depth and report line above is a prediction from source. No receipt supports it.
- C9 and C4 have no authenticated definitions. C10's and C24's are sealed but unapproved.
- F-1's absence of unchanged violations rests on the EXECUTOR-REPORTED PASS receipts. F-B2 and F-8 have never been
  run unchanged.
- The final baseline is not chosen. The analysis is of 42dd1bd1 and must be redone if any input of A changes.
- Detail strings are given only as far as the source fixes them. Choice labels in traces are paraphrased.
