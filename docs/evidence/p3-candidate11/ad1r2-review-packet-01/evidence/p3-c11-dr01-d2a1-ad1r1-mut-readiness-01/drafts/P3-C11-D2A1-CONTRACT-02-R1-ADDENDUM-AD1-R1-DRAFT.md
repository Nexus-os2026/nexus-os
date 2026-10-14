# P3-C11-D2A1-CONTRACT-02-R1 — ADDENDUM AD-1-R1 — DRAFT

**STATUS: DRAFT. NOT ACCEPTED. NOT ADOPTED. NOT APPLIED. NOT IN FORCE.**

Prepared by the executor (Claude Code, Claude Opus 5.5) under P3-C11-DR01-D2A1-AD1R1-MUT-READINESS-01, for the
Sole Architect's independent review. It encodes the Architect's rulings A to J of that mission. It decides nothing
the rulings left open. The Owner keeps authority over every contract amendment. Until the Architect has reviewed this
text and the Owner has approved it, contract P3-C11-D2A1-CONTRACT-02-R1 (sha256
`cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`) applies unchanged, with its binding reviews
(`6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`,
`1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35`).

**Relation to AD-1.** This file is a corrected copy of AD-1 (`P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md`,
sha256 `01778ac35cf98dfe97935dfc0ed413cbdfc18cb953b9d426822bef430061176e`). AD-1 is not changed and not overwritten.
A byte copy of it and the diff from it to this file are kept beside this file, each with its own hash.

**Verbatim rule.** Every block marked *verbatim* below is copied by program from the contract file named above, line by
line, without elision. A check run before hashing compared each block with those lines (`tools/fill-ad1r1.py`). Line
numbers are of the contract file.

Nothing here does any of the following:

- lowers a fault budget, drops an actor, or changes a configuration's bounds;
- counts an INCOMPLETE run as PASS;
- relabels a property;
- removes a property, stop condition or receipt obligation.

---

## 0. Basis

| Item | Identity | Status |
|---|---|---|
| Production protocol core | `protocol.rs` blob `a2df2841b99c` (sha256 `a5454944…a296`), the same at 244c5b23, 83816e66, 77a146ca, 2d83b74b and 42dd1bd1 | unchanged since SOURCE-R2 |
| Contract explorer | `explore` and everything it calls (`successors`, every `step_*`, `Io`, `Oracle`, `encode`/`decode`, the property functions, `terminal`): `model_tests.rs` lines 1–4814, byte-identical from 244c5b23 to 42dd1bd1 | unchanged |
| Admission observer quotient (AQ) | ADM-QUOTIENT-01: Theorem 1, L-A, L-B, Q1–Q15, AQ1–AQ6; `expand`/`observe_admission`/`explore_experimental` (83816e66) | executor PROVEN; not accepted |
| Gating-caller symmetry quotient (SQ) | GC-SYMMETRY-01: Lemma E, S1–S15, ASYM-1..6, AS1–AS4 (77a146ca); extension E-PT/E-B with the `SYMMETRIC` allow-list (2d83b74b, CLOSURE-PREP-01) | executor PROVEN; the source extension passed an independent source review for the bounded experiments; the proofs are not contractually accepted |
| F-3 composed (AQ ∘ SQ) | GC-SYM-F3-01 at 77a146ca: 20,431,851 keys, complete, PASS | experimental; not 8.3 evidence |
| F-PT, F-B composed | PT-FB-EXPERIMENT-01 at 2d83b74b: 29,751,473 and 3,775,373 keys, complete, PASS | experimental; not 8.3 evidence |
| F-1, F-2 original | CAMPAIGN-01 at 244c5b23: F-1 PASS 46,524,336 states (60M one-run exception), F-2 PASS 12,602,670 | informational; not final-commit evidence |
| F-3 original | CAMPAIGN-01 at 244c5b23: INCOMPLETE at 30,000,002; F3-CAP60-01 at 9e945ee7: INCOMPLETE at 60,000,001 | INCOMPLETE; establishes nothing |
| C1/C25 witnesses | experimental commit 42dd1bd1 (branch `experiment/p3-c11-d2a1-ad1r1-witness-01`, local, not pushed): `c1_witness_p1_at_a_successful_admission`, `c25_witness_p7_on_the_resulting_world`; one exploratory probe each | executor-only test results; not accepted |

## 0A. The rulings this draft encodes

The Architect's words, from the mission, then the section that writes each one as contract text.

| Ruling | Words | Section |
|---|---|---|
| A | "F-1's 60M original-state ceiling: proposed standing exception, subject to Owner approval." | A |
| B | "F-3's completed quotient: conditionally recognizable through exact admission and symmetry equivalence proofs." | B |
| C | "Select C-1: 30M quotient keys for configurations expressly admitted to equivalent-graph validation; time and memory unchanged." | C |
| D | "Preserve all property, D1, L1, branch coverage and trace obligations." | D |
| E | "Admit only F-3, F-PT and F-B to the composed quotient, subject to final source review, complete runs and Owner approval." | E |
| F | "Select F-1: mutation controls use the ORIGINAL explorer on the mutant. After restoration, an independently accepted quotient PASS may serve as the unchanged-core restored receipt for F-PT and F-B." | F |
| G | "Select C1/C25 Option A: preserve the first E3/E4 failure and separately establish P1/P7 by exact replay." | G |
| H | "Preserve the original historical mutation manifest unchanged. Prepare a SEPARATE versioned current-tree manifest covering all 22 stale source-hash entries and their anchors. Final Candidate 11 acceptance requires reviewed current-tree manifest alignment, not merely an explanatory supplement." | H |
| I | "Select exact-final-commit baseline evidence." Mutant build identity as listed. "Do NOT describe the mutant tree itself as identical to the unchanged baseline." | I |
| J | "Preserve storage assumptions A-1/A-2 as undischarged." | J |

The mission also asks this draft to resolve eight further items. They are sections K to Q, plus section 1:

- K: the cap check's off-by-one;
- L: the R1 CAS expected-value wording;
- M: historical INCOMPLETE receipts against new complete equivalent-graph results;
- N: the prerequisites of every quotient claim;
- O: first-failure attribution;
- P: test-only boundaries;
- Q: 24 HOLD and E10 not applicable;
- section 1: which clauses are replaced.

---

## 1. Supersession table

Each clause **stays in force** except as the "Effect" column states. The effects are:

- **REPLACED**: the text no longer governs for the stated scope.
- **SUPPLEMENTED**: the text governs, with the added rule.
- **CLARIFIED**: the text governs as written; the addendum states its one reading.

| # | Line(s) | Clause (short name) | Ruling | Effect | Section |
|---|---|---|---|---|---|
| S-1 | 855 | caps of a full run | A, C, K | SUPPLEMENTED (F-1 60M; quotient keys for E's configurations); "reaching" CLARIFIED | A, C, K |
| S-2 | 609 | the explorer visits every interleaving | B, E, D | SUPPLEMENTED for F-3, F-PT, F-B only | B, D, E, N |
| S-3 | 620 | equal keys are one state | B, E | SUPPLEMENTED for E's configurations | B, N |
| S-4 | 831 | run on the exact final commit | I | CLARIFIED and strengthened | I |
| S-5 | 853 | failing and restored receipts | F, I | SUPPLEMENTED: restored receipt for F-PT, F-B; mutant identity | F, I |
| S-6 | 693 | a control must make its property fail | G | SUPPLEMENTED for C1 and C25 only | G |
| S-7 | 699 | row C1 | G | SUPPLEMENTED: cell P1 read as "P1 (first failure E3; witness)" | G |
| S-8 | 721 | row C25 | G | SUPPLEMENTED: cell P7 read as "P7 (first failure E4; witness)" | G |
| S-9 | 863 | receipts of every run | C, D | SUPPLEMENTED for quotient runs and witnesses | C, D, G |
| S-10 | 911 | manifest control M87 | H | REPLACED as to *how* the text update is made | H |
| S-11 | 912 | manifest controls of `P3` | H | REPLACED as to *how* the text update is made; its condition kept | H |
| S-12 | 915 | D2A-1 does not edit the manifest; B/Q-5 | H | SUPPLEMENTED (not replaced): the revision it requires is specified | H |
| S-13 | 548 | rule R1 | L | CLARIFIED: the guess | L |
| S-14 | 813 | three outcomes | K, M | CLARIFIED: "a cap was reached"; INCOMPLETE is never PASS, kept | K, M |
| S-15 | 827 | caps of the default suite | K | CLARIFIED: "under its cap" | K |

### Verbatim text of each superseded clause

**S-1, line 855** (verbatim):

```text
**Caps for each run:** 30,000,000 states, counted by the explorer; 45 minutes and 16 GiB, enforced by the command that starts the run. **They are ceilings for an investigation, as the second review accepted them (ruling 6). They are not a measured cost and not an accepted one.** Reaching one, a failed allocation, a run killed from outside, and an unfinished search are each INCOMPLETE.
```

**S-2, line 609** (verbatim):

```text
`model_tests.rs` explores the real core. It simulates the ledger's content, implements every verb over that simulation, and plays every caller. It visits every interleaving of whole steps, breadth first, with a visited set and a parent table, so that a violation has a shortest trace.
```

**S-3, line 620** (verbatim):

```text
- Two states with equal keys are treated as one. Anything left out of the key would be a defect in the evidence; the receipts list the key's fields.
```

**S-4, line 831** (verbatim):

```text
Marked ignored, so that the ordinary test command stays short. **Run explicitly, optimised, on the exact final commit of the mission, every one, before the mission reports.** Missing evidence for any of them is a failed mission, whatever the ordinary test command says.
```

**S-5, line 853** (verbatim):

```text
Each control of 7.5 is run on its configuration, with a failing receipt, and a restored receipt showing the unchanged core passes that configuration.
```

**S-6, line 693** (verbatim):

```text
Each control is one named change, run **on the configuration named**. It must make its property fail by assertion. A build failure or a fixture failure kills nothing, with one stated exception (C22). **If a control is not killed on its configuration, the mission reports that and stops. It does not pick another configuration.**
```

**S-7, line 699** (verbatim; a row of the table headed at line 697, quoted with its header line):

```text
| Id | Change | Fails | On |
| C1 | the count becomes a flag | P1 | F-1 |
```

**S-8, line 721** (verbatim; the same table):

```text
| Id | Change | Fails | On |
| C25 | a gating append answers Committed on `Commit::Unknown` | P7 | F-1 |
```

**S-9, line 863** (verbatim):

```text
For every run, default or full: the commit; the compiler version; the profile; the exact command; the configuration and its bounds; states and transitions; each property's verdict; for each branch of each operation, whether any run took it; the time; the peak memory as the starting command reports it; and how the run ended. Every `#[ignore]` in the two test files is listed by name with the receipt that shows it ran.
```

**S-10, line 911** (verbatim; a row of the table headed at line 902):

```text
| Pin | Classification for D2A-1 |
| Manifest control M87 | its recorded hash of `gc/lib.rs` goes stale. TEXT UPDATE, SAME PROPERTY. |
```

**S-11, line 912** (verbatim; the same table):

```text
| Pin | Classification for D2A-1 |
| Manifest controls M32, M78, M79, M80, M89, M112, M113, M141, M143, M153, M177, M178, M179, M180 | their recorded hash of `P3` goes stale. TEXT UPDATE, SAME PROPERTY, **if** the new guard is added without touching any anchor. |
```

**S-12, line 915** (verbatim):

```text
**D2A-1 does not edit the manifest.** By ruling B/Q-5: the original is preserved; the mismatch is reported; no mutation result that depends on those two files is relied on until a reviewed mapping exists; failing, restored and final receipts are kept; and before final Candidate 11 validation the approved manifest revision and its campaign must agree with the exact final tree.
```

**S-13, line 548** (verbatim; a row of the table headed at line 546):

```text
| # | Rule |
| R1 | **The word is touched in exactly two private functions:** one that loads it, used only by `view`; one that runs a compare-and-exchange loop. Both use sequentially consistent ordering, for success and for failure. Nothing else in the file names the word. **The loop needs no load:** it starts from a guess — for a store operation, the value the core last published — and takes the present value from a failed exchange. |
```

**S-14, line 813** (verbatim):

```text
Every exploration ends as **PASS** (finished; every property holds), **FAIL** (a property failed; the trace is printed) or **INCOMPLETE** (a cap was reached, or the run was stopped). **INCOMPLETE is never PASS.** In the default suite it fails the test.
```

**S-15, line 827** (verbatim):

```text
Caps: 250,000 states each, and 180 seconds for these new tests together on the development machine. They are ceilings for an investigation, like those of 8.3, not measured costs. That is not a claim about how long the whole workspace's tests take. The five explorations take a test-only lock, so that they run one at a time even when the test runner uses several threads. **The mission measures. If a run does not finish under its cap, the mission stops and reports. It does not shrink a bound.**
```

### Not superseded

The following stay in force exactly as written. They are listed so that no gate is weakened silently.

```text
- **A-1.** A transaction whose COMMIT was never issued, and whose rollback was `Undone`, has persisted nothing.
- **A-2.** A transaction whose commit was `Durable` is durable.
```

```text
- The storage verbs, and assumptions A-1 and A-2.
```

```text
| Claim | Holds for |
|---|---|
| Safety: P1 to P15 as kept, E1 to E9, E11 | every configuration |
| L1, which is a property of the explored graph and not of a real execution (6.3) | every configuration without DROP |
| In the explored graph, an end state has every fact Paid or Retired and all cells empty | explored runs that end neither latched nor with handover fault |
| After a latch or a handover fault | **no progress is claimed.** Admission stays closed, the session ends without a clean close, and the remedy is a restart. |
```

```text
| E10 | With worker verbs unwinding, D1 and L1 hold. |
```

```text
| A4 | the drop paths record a blanket Uncertain | **nothing may fail** | F-PT |
```

```text
**Properties with no control:** P2, P5, P9, D1, E6, E10, and the handover-fault half of E9. C7 without A4 is expected to fail E11 on F-3; that run is added to the receipts but is not required to kill.
```

```text
Controls are applied by hand or by a script kept outside the repository. D2A-1 does not add them to the mutation-control manifest.
```

```text
**One full exploration runs at a time.** Before each, the starting command checks that the machine has the memory free; if not, the run does not start and is reported as not started. It never stops a local-model workload, or anything else, to make room. The full set is run at the mission's evidence gate, not after each edit.
```

```text
**These bounds are reviewed bounds.** The implementer may not lower a fault budget, drop an actor, or count an INCOMPLETE run as PASS.
```

```text
- A property fails on the unchanged core.
- A control is not killed on its named configuration or by its named test.
- A run of 8.2 or 8.3 is INCOMPLETE.
```

Also not superseded:

- every property of 7.3 and 7.4, and every row of 7.5 other than the readings of S-7 and S-8;
- each control's configuration;
- the bounds of every row of 8.3;
- section 9;
- the pins of section 10 other than S-10 and S-11;
- the label conditions of section 11;
- every stop condition of section 12.

---

## A. F-1's ceiling — a proposed standing exception

**Text.** "F-1 alone runs with a state ceiling of 60,000,000 states, counted by the original explorer. Its time
(45 minutes) and memory (16 GiB) ceilings, and every other cap, are unchanged. This is an investigation ceiling,
like the others. It is not a measured or accepted cost. It is a standing exception to 8.3's 30,000,000, in force
only once the Owner approves it. Until then, every F-1 run needs the Owner's one-run exception, as before. A control
run on F-1 uses F-1's ceiling. F-D, though defined from F-1, keeps 30,000,000."

**Source fact.** `full_f1` has run `Config { cap: 60_000_000, ..F1 }` since 243501f0. At 42dd1bd1 this is
`model_tests.rs` lines 4169–4174. The `F1` constant itself carries `FULL_CAP`, so F-D and every other user of `F1`
keep 30,000,000.

**Evidence (informational under I).** F-1 PASS: 46,524,336 states, complete, 4:03.66, at 244c5b23 (CAMPAIGN-01);
the same count at 243501f0 (F1-CAP60-01). F-1 needs no quotient.

## B. F-3's completed quotient — conditionally recognizable

**Text.** "F-3's full exploration may be established by one complete run of the composed quotient — the admission
observer quotient composed with the gating-caller symmetry quotient, `explore_symmetric(&F3, step_adm,
Mode::Quotient)` as `gc_symmetry_f3` runs it — on the exact final commit, if and only if:

1. the exact admission equivalence (Theorem 1 with Q1–Q15, AQ1–AQ6) and the exact symmetry equivalence (Lemma E
   with S1–S15) have each passed independent review against the exact final source, as equivalences of the
   original graph and not as bounds;
2. every prerequisite of N holds at that commit;
3. the run's runtime guards raise no refusal. These are:
   - the admission observer's checks at every expanded key;
   - the symmetry guard: involution; the expansion of each key's mirror held to the exchange; D1 judged alike.
   Any refusal ends the run with no verdict;
4. the run completes (no unexpanded key) within C's ceiling, and every applicable property, D1 and L1 hold (Q);
5. the Owner has approved this addendum.

The receipt states that the original graph was not explored. Until all five hold, a composed-quotient PASS of F-3 is
an experimental result and is not 8.3 evidence."

**Exact equivalence basis, printed by the receipt.** K is the quotient's keys; Kf is those that are their own
mirror; N is the number of admissions (2):

- admission-quotient states |R_Q| = 2K − Kf (exact, DERIVED under S1–S15);
- original states |R_G| = (N+1)·|R_Q| (exact, DERIVED under Theorem 1);
- admission evaluations = |R_Q|; original admission edges = N·|R_Q|.

For F-3 at 77a146ca: |R_Q| = 39,982,924 and |R_G| = 119,948,772, both DERIVED.

Transition, fault-edge, no-step and admitted counts are exact only if the run prints the self-mirrored shares
(`Mirrors.fixed_*`). `symmetric_full_exploration` discards them, so today only bounds exist. **Open (not decided
here):** whether the final-commit run must print them. That would be a test change in a later, authorised
mission. Until it is made, the receipt gives those counts as labelled bounds and never as values.

## C. C-1 — what a ceiling counts for an equivalent-graph run

**Text.** "For a configuration expressly admitted by E to equivalent-graph validation, the 30,000,000-state
ceiling of 8.3 counts the keys of its composed quotient, as that explorer counts them (K). The receipt also reports
the DERIVED original counts, with their formulas and their labels (B). The time (45 minutes) and memory (16 GiB)
ceilings bound the real process. They are enforced by the command that starts the run, and are unchanged. Reaching
any ceiling is INCOMPLETE (K), and an INCOMPLETE quotient run supports no derived statement. For every other
configuration, the ceiling counts the original explorer's states."

**Margins observed (informational).** F-3: 20,431,851 keys, 68.1% of the ceiling. F-PT: 29,751,473 keys, 99.17%; a
model change that adds 248,528 keys makes it INCOMPLETE. F-B: 3,775,373 keys, 12.6%.

## D. Obligations kept under a quotient

**Text.** "A quotient run carries every obligation of 7.3, 7.4, 8.1 and 8.4 that an original run carries. Where it
cannot discharge one exactly, it says so and does not substitute an estimate."

- **Safety.** A quotient PASS establishes the 7.4 safety claims for the original graph only through B(1). The
  receipt says so. The set of reachable violated property names is the original's (S8, Q12), but the first-reported
  name may differ (O).
- **D1.** Judged at every key with no enabled non-ADM step: with the admissions spent, at the key and at its mirror.
- **L1.** Kahn's algorithm over the quotient's non-fault edges. It transfers to the original by S10 (both
  directions) and Q10. ADM steps are observations, never edges. L1 stays "not claimed" where 7.4 does not claim it
  (Q).
- **Traces.** Any FAIL trace is unfolded (S11) and replayed in the original semantics before it is cited, with the
  callers' own names and the N admission steps appended for D1 (Q11).
- **Branch coverage.** The receipt lists, for each branch, whether the quotient took it. That set equals the
  original's (S7, Corollary 1). Branches no run takes are listed NOT TAKEN, as now. A per-member statement (for
  example, which batch member a branch took) is NOT ESTABLISHED by a quotient run unless the run prints it.
- **Counts.** The receipt reports its own counts as the quotient's, plus the DERIVED original counts of B, each
  labelled MEASURED, DERIVED or BOUND.
- **E10** keeps its rule (Q).

## E. Which configurations may use the composed quotient

**Text.** "Only F-3, F-PT and F-B may use the composed quotient for 8.3. Each may do so only when all of these
hold:

- the final source review of B(1) and of N;
- its own complete run at the exact final commit;
- the Owner's approval of this addendum.

F-PT and F-B use the same proofs, extended by E-PT and E-B (CLOSURE-PREP-01, Stage 2, items 1–17). F-1 and F-2
complete under the original explorer and use it. Every other configuration uses the original explorer, unless a
later amendment admits it after its own audit. The gating-caller symmetry is never available to:

- F-5, F-6, F-7, F-8 or F-B2 (acknowledgements or callers, ASYM-1/2);
- F-G (ASYM-3).

No mutation control is ever run under a quotient (F)."

**Enforcement.** `assert_symmetric` and the `SYMMETRIC` allow-list (lines 6079–6128 at 42dd1bd1) refuse, before
exploring:

- an unlisted shape;
- a raised cap or time cap;
- a seeded G2 conflict or acknowledgements;
- an initial state that is not its own mirror.

## F. Mutation controls — the original explorer on the mutant

**Text.** "Every control of 7.5 that names a configuration runs the ORIGINAL explorer (`explore`, through that
configuration's `full_*` or default test), on the mutant build, on its named configuration, under that
configuration's ceiling (F-1: 60,000,000 under A; every other: 30,000,000; default explorations: 250,000). A
control is killed only by the original explorer's FAIL of its named property, read under G for C1 and C25. A
control run that reaches a ceiling without a FAIL is not killed, and 7.5's stop rule applies.

After restoration (I), the restored receipt that shows the unchanged core passes the configuration is:

- for F-PT and F-B: either the original explorer's PASS, or an independently accepted composed-quotient PASS of the
  unchanged core on the exact final commit (B–E, N);
- for every other configuration: the original explorer's PASS on the exact final commit.

A quotient run is never a failing receipt. A control is never run under a quotient."

**What this means for the named rows.**

- **C6, C7 (F-PT) and C8 (F-B):** the failing receipt is the original explorer's, on the mutant. The restored
  receipt may be F-PT's or F-B's accepted quotient PASS.
- **A4 (F-PT, "nothing may fail"):** it is evidenced by F-PT's own 8.3 PASS. F-PT is F-3 with A4 on; its PASS is
  compared with F-3's PASS (both runs, their branch sets, and their counts).
- **Reasons the controls cannot move** (ADM-QUOTIENT-01 Q15; GC-SYMMETRY-01 S15; CLOSURE-PREP-01 item 17):
  - a control is a model change, which voids the source audits (AS4, AQ1);
  - the symmetry guard would end an asymmetric mutant's run "symmetry inapplicable", which kills nothing;
  - the first-reported property may differ (O).

## G. C1 and C25 — Option A

**Text.** "For C1 and C25 only, the control is killed when all of (a) to (d) hold. Every receipt is made on the
mutant build of I and on the unmutated build of the exact final commit.

(a) **The first failure is preserved as primary.** The unchanged original explorer, run as `full_f1` on the
mutant, FAILs. Its first-reported property, its detail and its shortest trace are kept as printed.
- Predicted: E3 in 4 steps for C1; E4 in 1 step for C25.
- If it reports otherwise, that is reported as it is and not reinterpreted. The control is then not recorded as
  killed under this clause until the Architect rules.
- E3 is never counted as P1, and E4 never as P7.

(b) **The named property is established by exact replay.** The control's witness test, selected alone with
`--exact` on the same mutant executable, FAILs with:
- C1: `NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)`;
- C25: `NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))`.
The receipt must also show:
- every step enabled and taking exactly its scheduled choices;
- no `WITNESS FIXTURE FAILURE`;
- E3 (C1) or E4 (C25) listed separately among the other properties.

A fixture failure kills nothing.

(c) **The healthy core violates nothing on the same schedule.** The same witness on the unmutated build of the
exact final commit PASSes. It shows no violation at any step or state, and the healthy branch at the changed step:
- C1: `admission: refused`;
- C25: `append_gating: Refused Uncertain Storage`.

(d) **The rows are recorded** as 'C1: P1 (first failure E3; witness)' and 'C25: P7 (first failure E4; witness)'."

**The witness's required form** (each is a property of the prototype at 42dd1bd1, shown in section 3 of the
report):

1. Real protocol operations and the original model transitions: `step`, with `step_adm` as admission.
2. No forced state: the replay starts from `initial(F1)`, and each step runs on a world decoded from the previous
   key.
3. Real hook counts: `hook::take_counts` around each step, passed to `transition_properties`.
4. Unaltered property predicates: `state_properties`, `transition_properties`, and the properties pushed inside the
   steps.
5. Deterministic replay of every scheduled actor step and every oracle choice.
6. Every step is asserted enabled.
7. Every step's choices are asserted to equal the schedule's. The fixed branch labels are asserted at every step
   except the one the control changes.
8. Every violation is collected:
   - of the initial state;
   - of every step: those pushed inside it, and its transition properties;
   - of every state after a step: its state properties and its key's round trip.
   None stops the replay.
9. The named property is checked at its locus. P1 must be in step 11's own violations, with `admission: admitted`.
   P7 must be in the after-state of step 1, with `append_gating: Committed (inserted)`.
10. On the healthy core the same schedule must show no violation and the healthy branch.
11. Differing behaviour is printed as evidence, step by step (branches, word, ledger), never absorbed into the
    fixture.

**Location.** Two ordinary (not ignored) tests at the end of `model_tests.rs`:
`c1_witness_p1_at_a_successful_admission` and `c25_witness_p7_on_the_resulting_world`. Each completes in under a
millisecond and touches no shared state but the thread-local hook, so it is safe in the default suite. On the
unmutated core it is a regression test of the behaviour each control removes. The prototype is commit 42dd1bd1 on
a local experimental branch. Carrying it to the final commit needs its own authorised mission.

**Open (not decided here): C25's exact mutant text.** Row C25 says only "a gating append answers Committed on
`Commit::Unknown`". The probe used CAMPAIGN-01's sealed encoding (`controls_defs.py`, sha256 `ef0420a1…108f`). It
returns `Committed(seq)` and also drops `self.keep = true`, so U is emptied, as it is on a Committed answer. The
narrower one-line form would keep U. The witness's P7 depends only on "told Committed" and on the absent row.
Under the narrower form P7 is therefore expected too, but that is ARGUED, not probed. The final campaign must fix
C25's text before its run. C1's encoding (`count_step` sets `unapplied` to 1 on a step up and 0 on a step down) has
no such alternative.

## H. The mutation manifest — original preserved, current-tree manifest separate

**Text.** "The historical manifest `docs/evidence/p3-candidate10/original-183/manifest.json` (sha256
`2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14`, version `1-r1`, 183 controls) is preserved byte
for byte. It is not edited, regenerated or overwritten.

A **separate, versioned current-tree manifest** is made: a new file, with its own version string and its own
sha256. It is not an explanatory supplement. Final Candidate 11 acceptance requires that this manifest be reviewed
and found aligned with the exact final tree. The 22 stale entries' 'TEXT UPDATE, SAME PROPERTY' (S-10, S-11) is
made only in it."

**Specification of the current-tree manifest.**

1. **Identity.** A new path, proposed `docs/evidence/p3-candidate11/current-tree-183/manifest.json`; the Architect
   chooses. Its own `version` (proposed `ct-1`) and `name`. A field `derived_from` holding the original's path,
   sha256 and version. A field `final_tree` holding the exact final commit and tree it was aligned with.
2. **Content.** All 183 controls, in the original's order. For each control, these are copied from the original
   and never changed:
   - `id`, `category`, the two meanings;
   - `edits`: every `file`, `anchor`, `replacement`, `count`, in order;
   - `command`, `cwd`, `expected_failure_tests`;
   - `review_status`, `lineage`, `review`.
   `source_sha256` holds the sha256 of each named file at the exact final tree.
3. **The 22 stale pairs.** Each gets its own reconciliation record:
   - control id; file;
   - the original's recorded hash;
   - the final tree's sha256 and git blob;
   - the commit at which it first went stale (below);
   - every anchor's occurrence count at the final tree against the manifest's `count`, with its line number;
   - a statement that the control's anchors, replacements, command and expected failing tests are unchanged;
   - the reviewer's finding that the control still introduces the same fault and the selected test still asserts
     the same property (SAME PROPERTY).
4. **Alignment check (mechanical).** Every one of these must hold, or the manifest is not aligned:
   - every recorded `source_sha256` equals the final tree's file;
   - every anchor occurs exactly `count` times at the final tree;
   - the original manifest's sha256 at the final tree is still `2c4adc84…2e14`;
   - the control set and every copied field equal the original's.
5. **What is not a text update.** A changed anchor, replacement, count, command or expected test is a redefinition
   of the control, not a TEXT UPDATE. It needs its own review, as line 912's condition requires.
6. **Restoration** after each control is verified by blob equality with the baseline, never by reversing a
   replacement. Several replacements already occur in their files (M45, M113, M129), and four are deletions (M32,
   M39, M79, M143).
7. **Ownership.** The 15 pairs that D2A-1 made stale are D2A-1's. The 7 that predate D2A-1 belong to Candidate 11's
   validation, and the Architect assigns them. All 22 must be aligned before final Candidate 11 acceptance.

**The 22, as recorded at 244c5b23 (CAMPAIGN-01) and re-derived at 42dd1bd1 by this mission's read-only dry run**
(`receipts/stage6/01-manifest-dry-run-42dd1bd1.txt`):

| File | Controls | Stale since |
|---|---|---|
| `app/src-tauri/src/phase3_tests.rs` | M32 M78 M79 M80 M89 M112 M113 M141 M143 M153 M177 M178 M179 M180 (14) | D2A-1 (22ebf9a5), expected by contract 10 |
| `crates/nexus-governed-control/src/lib.rs` | M87 (1) | D2A-1 (22ebf9a5), expected by contract 10 |
| `crates/nexus-governed-control/src/broker.rs` | M18 M19 M39 (3) | stale at 862a11ce (the D2A-1 contract base) |
| `crates/nexus-governed-control/src/authority/approval.rs` | M84 (1) | stale at Candidate 11 b1d535f1 |
| `crates/nexus-governed-control/src/authority/mod.rs` | M05 (1) | stale at Candidate 11 b1d535f1 |
| `crates/nexus-governed-control/src/authority/run.rs` | M45 M129 (2) | stale at Candidate 11 b1d535f1 |

At 42dd1bd1, every anchor of all 183 controls occurs exactly its `count` times, the 22 stale pairs included. This
is informational: 42dd1bd1 is not the final tree.

## I. Exact-final-commit baseline evidence, and what identifies a mutant build

**Text.** "Every 8.3 run, every 7.5 control (failing and restored receipts), every C1/C25 witness receipt and the
A4 comparison is made on the exact final commit of the mission. Each comes from a fresh optimised build, whose test
executable's sha256 is pinned and checked before and after each run. The following are informational and are not
8.3 or 7.5 evidence:

- F-1 and F-2 PASS at 244c5b23;
- the F-3, F-PT and F-B composed PASS at 77a146ca and 2d83b74b;
- the witness probes at 2d83b74b + witness source.

A mutant build is identified by all of:

1. the final unchanged baseline commit and tree;
2. the exact named control id and its mutation patch, with the patch's sha256;
3. the exact mutation source blob or blobs: git blob id and sha256 of every file the patch changes, as built;
4. the mutant test executable's sha256 and size, from a fresh optimised build of that source;
5. the exact configuration and invocation: test name selected with `--exact`, flags, cwd, environment and
   resource limits;
6. the raw failing receipt, unedited, with its sha256;
7. the restored baseline identity, and the restored PASS receipt. The identity is every changed file's blob equal
   to the baseline's, and nothing else differing from the baseline commit. The receipt is the restored build's run,
   or, for F-PT and F-B, the accepted quotient PASS of F.

A mutant tree is not the baseline tree, and no receipt describes it as identical to it. Its identity is the
baseline tree, plus the named patch, verified by blob. Reproducible builds are not established: two builds of one
source have differed in sha256. Every identity is the sha256 of the executable actually run."

## J. Storage assumptions

No change. A-1 and A-2 (4.1, lines 198–199) stay stated and undischarged until the real storage and shell contract
(6.3; 13). Every result above is conditional on them, and on AQ6 where a quotient is used. That covers original or
quotient runs, PASS or FAIL, and explorer or witness results. Each result is the explored model's state-safety, D1
and L1. It is not a real execution, durability, or contract 6.2/6.3. Section 11's scope sentence is unchanged.

## K. The cap check — what "reaching" a ceiling means

**Source fact.** All three explorers check `visited.len() as u64 > config.cap` before expanding each key. That is
`explore`, `explore_experimental` and `explore_symmetric_kept`: lines 3907, 5068 and 6609 at 2d83b74b and 42dd1bd1.
`visited` counts every distinct key discovered, the initial key included, expanded or not.

**Text.** "A state ceiling C of 8.2 or 8.3 bounds the number of distinct keys the run's explorer has discovered.
For an original run these are states; for a quotient run admitted by E, they are quotient keys.

- A run is within its ceiling if and only if it completes with at most C keys discovered.
- 'A cap was reached' (8.1) and 'reaching one' (8.3) mean this: at an expansion boundary, more than C keys have
  been discovered while some discovered key is still unexpanded. The run then stops INCOMPLETE.
- An INCOMPLETE receipt prints the discovered count as it is. That count lies between C + 1 and C plus the new keys
  of one expansion (observed: 30,000,002 at C = 30,000,000; 60,000,001 at C = 60,000,000).
- A run that completes with exactly C keys discovered is complete and within its ceiling.
- The time ceiling is the launching command's. The explorer's own elapsed-time check is an earlier stop, also
  INCOMPLETE, and does not extend the ceiling."

**Why this reading.**

- It is what the frozen code does. A stricter reading (C keys discovered is already INCOMPLETE) would need an
  explorer change or a receipt rule that overrides the explorer's verdict.
- It changes no recorded verdict. Every complete run so far ended well below its ceiling. The closest is F-PT, at
  29,751,473 against 30,000,000.
- Under the stricter reading the explorer would report PASS for a run that the contract calls INCOMPLETE. That
  would be a contradiction between the verdict and the contract, which this addendum must not create.

## L. Rule R1 — the exchange loop's first expected value

**Source fact.**

- Store operations: `Publish::new` sets `guess = core.flags()` (`protocol.rs` 1258–1259). These are the phase,
  session, latch and cell bits computed from the core when the guard is created, with zero counts and no handover
  fault.
- Lock-free ticket steps: `arm`, `confirm`, `expire` and drop start from 0 (`protocol.rs` 236, 353, 356, 369, 382,
  395, 398).
- A failed exchange takes the present value and recomputes the step (200–218).
- Appendix A's `Core` has no field holding a published value.

**Text (replaces only the third sentence of R1; the rest of R1 stays as written).** "**The loop needs no load:**
it starts from a guess, and takes the present value from a failed exchange. A store operation's guess is the phase,
session, latch and cell bits that the core's own state gives when its guard is created (by E1, the bits its last
publication wrote), with both counts zero and the handover-fault bit clear. A lock-free ticket step's guess is
zero. Any guess is safe, because a failed exchange supplies the present value and the step is recomputed from it.
A wrong guess costs one failed exchange, which the hook observes (T-PB2, T-PB4)."

**Effect.** This is wording only. It is safety-neutral. Appendix A and the code are unchanged. The second review's
reading of R1 is not otherwise affected.

## M. Historical INCOMPLETE receipts and new complete equivalent-graph results

**Text.** "A receipt keeps the verdict it was given. A later complete run of an equivalent graph never changes an
earlier receipt's verdict. The earlier receipts concerned are:

- F-3 INCOMPLETE at 30,000,002 (CAMPAIGN-01, 244c5b23);
- F-3 INCOMPLETE at 60,000,001 (F3-CAP60-01).

They stay INCOMPLETE, establish no property, and are listed with their verdicts wherever F-3's history is given. A
complete composed-quotient PASS of F-3 is a different run of a different (equivalent) graph. It is not a
continuation, completion or relabelling of the INCOMPLETE runs. It may stand for F-3 in 8.3 only under B–E and N,
at the exact final commit (I). The two kinds of result do not conflict: an INCOMPLETE run asserts nothing. No
report may say that the original F-3 graph was explored or passed. It may say only that it is DERIVED to satisfy
the properties, under B(1)."

## N. Prerequisites of every quotient claim

A quotient claim is any of:

- 'F-3, F-PT or F-B PASS by the composed quotient';
- any DERIVED original count;
- a quotient PASS used as a restored receipt.

Each needs all of the following at the exact final commit. If any one fails, the claim is not made.

**Source.**

1. `protocol.rs` is byte-identical to the blob the proofs were audited against (`a2df2841b99c`), or AQ1–AQ6 and
   S1–S15 have been re-audited against the new blob.
2. The model is unchanged: `initial`, `actors`, every `step_*`, `Io`, `Oracle`, `encode`, `decode`,
   `state_properties`, `transition_properties`, `admission_properties`, `terminal` and the configuration constants
   are byte-identical to their audited text. Otherwise the audits are void (AQ1, AS4) and must be redone. Lines
   added after them (for example, the witnesses) do not affect this, provided no earlier line changes.
3. The quotient code is byte-identical to the reviewed text at 2d83b74b, or has been re-reviewed. That covers
   `expand`, `observe_admission`, `explore_experimental`, `mirror`, `explore_symmetric`, `explore_symmetric_kept`,
   `assert_symmetric`, `shape` and `SYMMETRIC`.
4. `Cargo.lock` and `rust-toolchain.toml` (rustc 1.94.0) are as at the final commit, and the pinned executable is
   built from it.

**Configuration.**

5. The constants, field by field:
   - F3: facts 2; conflict [true, true]; env true; batch false; faults 2; admissions 2; gating 2; gating_conflict
     false; acks 0; ack_attempts 0; malformed false; cell_constraint false; worker_unwinds false; drop false;
     replays 0; blanket_uncertain false; l1 true; cap 30,000,000; time 45 min.
   - FPT: as F3 with blanket_uncertain true.
   - FB: as F3 with batch true.
6. The printed bounds and actors equal these:
   - F-3 and F-PT: [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env];
   - F-B: [Fw(0), Se, Wk, Gc(0), Gc(1), Adm, Env].

**Execution.**

7. Exactly one test is selected, with `--exact`: `gc_symmetry_f3`, `gc_symmetry_fpt` or `gc_symmetry_fb`.
8. The run completes with 0 unexpanded keys and verdict PASS, with no runtime-guard refusal ("symmetry
   inapplicable", "quotient inapplicable").
9. It runs under the launching command's 45-minute and 16 GiB limits, without swap.

**Review and approval.**

10. B(1) review passed, and the Owner has approved this addendum.

## O. First-failure attribution

1. The original explorer reports one property: the first violation in breadth-first order. A state's properties
   are judged when it is expanded, before its transitions. Transitions are judged in actor order, then oracle
   order. That property, its detail and its shortest trace are the primary record of a FAIL.
2. The composed quotient's first-reported property may differ from the original's for the same defect (ASYM-4,
   Q12). A quotient FAIL names *a* violated property, never "the first", and is never used as a control's failing
   receipt (F).
3. A control's attribution comes only from the original explorer on the mutant (F). For C1 and C25 it also comes
   from the witness (G). E3 is never counted as P1, and E4 never as P7.
4. A witness lists every violation by step and by kind (step, after-state). Its order is a listing order, not a
   priority. It does not claim to be the explorer's first failure. It claims nothing about any configuration
   other than its own schedule.

## P. Test-only boundaries

1. The witnesses, and any later change that prints more (fixed-orbit shares, per-member batch exercise), live
   only in `model_tests.rs`, a `#[cfg(test)]` module.
2. They are added after every existing line. No existing line of `model_tests.rs` is changed or removed.
3. `protocol.rs` production text is never changed by a witness or by a receipt change. The one test-only item in
   it, the hook, is used as it is.
4. None of the following is changed by this addendum or by the witnesses:
   - `explore`, `successors`, `step_adm` or any `step_*`;
   - `explore_experimental` or `explore_symmetric_kept`;
   - any property function;
   - any F-1, F-2, F-3, F-PT, F-B (or other) constant, cap, actor or fault budget;
   - any guard of `P3`;
   - any manifest.
5. Mutation patches are applied only in disposable worktrees, by a script kept outside the repository (line 750
   unchanged), and are never committed.

## Q. Property lines: 24 HOLD and E10 not applicable

A PASS receipt prints one line for each of the 25 entries of `PROPERTIES`:

- P1, P2, P4, P5, P6, P7, P8, P9, P12, P13, P15;
- D1, L1;
- E1 to E11;
- KEY.

**Text.** "A configuration PASSes when every applicable line HOLDS. A line that is not applicable, or not claimed,
is printed as such. It is never counted as HOLDS, and never as a failure.

- **E10** ('With worker verbs unwinding, D1 and L1 hold') applies only where worker verbs unwind (F-W). Everywhere
  else it prints 'not applicable here (worker verbs do not unwind)'. A PASS there is 24 HOLD (L1's line included)
  and E10 not applicable. E10 is established only by F-W's PASS.
- **L1** is not claimed for F-D (DROP; 7.4's claims table). F-D's PASS prints 'not claimed for this
  configuration'.
- The same rule applies to quotient receipts, which print the same lines by the same code."

---

## Open items (none decided by the executor)

1. Whether the final-commit quotient runs must print the fixed-orbit shares and per-member batch exercise (B, D).
   That would be a later test change.
2. C25's exact mutant text: keep or drop `self.keep = true` (G).
3. The current-tree manifest's path, version string and owner, and who assigns the 7 pre-D2A-1 entries (H).
4. Acceptance of the C1/C25 witness prototype, and its carriage to the final commit (G).
5. Independent review of every resolution of K to Q.
6. The Owner's approval of A, and of this addendum as a whole.

## Changes from AD-1

| AD-1 | AD-1-R1 |
|---|---|
| Supersession table quoted clauses with "…" elisions | Every clause quoted whole, by program, in a fenced block. Five clauses added: 548 (R1), 813, 827, 915, and the C1/C25 table header |
| A: "Condition: the Owner's acceptance" | A: a standing exception, not in force until the Owner approves; one-run exceptions until then; controls on F-1 use 60M |
| B: three conditions | B: five conditions, adding N's prerequisites and Owner approval; "conditionally recognizable" |
| C: C-1 recommended, C-2 offered | C-1 selected; margins stated; C-2 dropped |
| D: obligations | D: obligations kept in full; per-member statements NOT ESTABLISHED unless printed; counts labelled |
| E: three configurations, "after the review" | E: only those three, subject to final source review, complete runs and Owner approval |
| F: F-1 recommended, F-2 offered | F-1 selected; F-2 dropped; A4 row and C6/C7/C8 stated |
| G: Option A, witnesses "proposed" | Option A selected; the witness's required form; the prototype's names and kill strings; C25's encoding left open |
| H: an "identity supplement" | A separate versioned current-tree manifest required for final Candidate 11 acceptance; its specification |
| I: I-1 recommended, I-2 offered | I-1 selected; I-2 dropped; mutant build identity in 7 parts; mutant tree not described as baseline |
| J: unchanged | unchanged |
| "cap check and R1 CAS wording stay open" | resolved as proposed text in K and L |
| — | M, N, O, P, Q added |
