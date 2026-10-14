# P3-C11-D2A1-CONTRACT-02-R1 — ADDENDUM AD-1-R3 — DRAFT

**STATUS: DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. NOT IN FORCE.**

Prepared by the executor (Claude Code, Claude Opus 5.5) under P3-C11-DR01-D2A1-AD1R3-READINESS-01, for the
Sole Architect's independent review. It repairs AD-1-R2 as that mission directs (0A.2: typed receipts, process-aware
classification, review roles, trace wording), carries the Architect's rulings (0A.1), and keeps AD-1-R2's dispositions
A to Q otherwise unchanged. It decides nothing those rulings left open. The Owner keeps
authority over every contract amendment. Until the Architect has reviewed this text and the Owner has approved it,
contract P3-C11-D2A1-CONTRACT-02-R1 (sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`)
applies unchanged, with its binding reviews (`6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`,
`1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35`).

**Relation to AD-1, AD-1-R1 and AD-1-R2.** This is a new file. AD-1 (`01778ac35cf98dfe97935dfc0ed413cbdfc18cb953b9d426822bef430061176e`),
AD-1-R1 (`589eb6cb24626585862749fed3fa619d2261cdbc8b08c56e7139c6d12bc04cda`) and AD-1-R2
(`53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc`) are history. None is changed or
overwritten. The diff from AD-1-R2 to this file is kept beside it.

**Verbatim rule.** Each fenced `text` block that follows a `VERBATIM` marker (an HTML comment naming contract line
numbers) is copied by program from the contract file above, line by line, without elision. A block that follows a
`VERBATIM REVIEW-02` or `VERBATIM REVIEW-03` marker is copied in the same way from the binding review of that hash. A
checker run against those bytes compares every such block, and every inline quotation listed in it, with those lines
(`tools/check-ad1r3.py`; its output is in the review packet). Line numbers are of the file named.

Nothing here does any of the following:

- lowers a fault budget, drops an actor, or changes a configuration's bounds;
- counts an INCOMPLETE run as PASS;
- relabels a property;
- removes a property, stop condition, diagnostic test or receipt obligation without saying so in section 1 or E;
- authorises any run, mutation, source change, manifest, adoption or integration.

---

## Provenance labels

| Label | Meaning |
|---|---|
| PROVEN FROM SOURCE | read by the executor from the exact source commit named; re-checkable at the cited lines |
| ARCHITECT-RULED | stated by the Sole Architect's ruling carried into the mission named |
| EXECUTOR-REPORTED | an earlier executor receipt or report; not independently re-run; the raw receipt is in the review packet |
| NOT ESTABLISHED | shown by nothing |

For counts, D adds MEASURED, DERIVED and BOUND.

---

## 0. Basis

| Item | Identity | Status |
|---|---|---|
| Production protocol core | `protocol.rs` blob `a2df2841b99c`, sha256 `a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296`; the same blob at 244c5b23, 83816e66, 77a146ca, 2d83b74b and 42dd1bd1 | PROVEN FROM SOURCE; unchanged since SOURCE-R2 |
| Contract explorer | `explore` and everything it calls: `model_tests.rs` lines 1–4814, byte-identical at 244c5b23, 83816e66, 77a146ca, 2d83b74b and 42dd1bd1 | PROVEN FROM SOURCE |
| Admission observer quotient (AQ) | ADM-QUOTIENT-01: Theorem 1, L-A, L-B, Q1–Q15, AQ1–AQ6 (83816e66) | executor proof; not accepted |
| Gating-caller symmetry quotient (SQ) | GC-SYMMETRY-01: Lemma E, S1–S15, ASYM-1..6, AS1–AS4 (77a146ca); extension E-PT/E-B and the `SYMMETRIC` allow-list (CLOSURE-PREP-01, 2d83b74b) | executor proof; not accepted. AD-1-R1 recorded that the extension's source passed an independent source review for the bounded experiments; that is not proof acceptance. No stored review record of these proofs is held in the evidence root. |
| F-3 composed (AQ ∘ SQ) | GC-SYM-F3-01 at 77a146ca: 20,431,851 keys, complete, PASS | EXECUTOR-REPORTED; experimental; not 8.3 evidence |
| F-PT, F-B composed | PT-FB-EXPERIMENT-01 at 2d83b74b: 29,751,473 and 3,775,373 keys, complete, PASS | EXECUTOR-REPORTED; experimental; not 8.3 evidence |
| F-1, F-2 original | CAMPAIGN-01 at 244c5b23: F-1 PASS 46,524,336 states (under the Owner's one-run 60M exception); F-2 PASS 12,602,670 | EXECUTOR-REPORTED; informational; not final-commit evidence |
| F-3 history | four INCOMPLETE runs (M) | INCOMPLETE; establish nothing |
| C1/C25 witness source | commit 42dd1bd1 (tree 7ac6bb21), published on `refs/heads/review/p3-c11-d2a1-ad1r1-witness-01` | PROVEN FROM SOURCE (ref, commit, tree and blobs read back); ARCHITECT-RULED: interim source review passed, no blocking source defect |
| C1/C25 probes | C1 probe and the historical compound C25 probe (AD1R1-MUT-READINESS-01); the narrow C25 probe (AD1R1-WITNESS-AUDIT-01) | EXECUTOR-REPORTED raw receipts. ARCHITECT-RULED (AD1R3-READINESS-01): the historical receipts support C1's intended P1 attribution; the retrieved patch and the authenticated historical receipts support C25's P7 with `self.keep = true` preserved. Historical, not final-baseline evidence; no final mutation kill (G) |

## 0A. What this draft encodes

### 0A.1 The Architect's rulings carried into this draft

**The ruling of P3-C11-DR01-D2A1-AD1R3-READINESS-01** (governs where it and the earlier ruling below differ):

| Subject | Ruling | Section |
|---|---|---|
| AD-1-R2 | REPAIR. | this file |
| C1 | The witness's source-review finding is maintained. The historical receipts support its intended P1 attribution. No final mutation kill. | G |
| C25 | The narrow one-line return-value definition remains selected. The retrieved patch and the authenticated historical receipts support P7 with `self.keep = true` preserved. No additional probe is justified. No final mutation kill. The old compound C25 definition is historical only. | G |
| Pending | Final AQ/SQ proof acceptance, the Owner's adoption of the amendment, final baseline selection, the full campaigns and integration. | B, I, N |

**The earlier ruling, on the witness audit (carried by AD-1-R2):**

| Subject | Ruling | Section |
|---|---|---|
| Witness delta 42dd1bd1 | No blocking source defect identified. Interim source review passed. Execution-evidence authentication remains pending. | G |
| C1 | Witness prototype provisionally valid for the specified count-to-flag mutation. No final mutation kill is established. | G |
| C25 | Select the narrow one-line return-value mutation. Preserve `self.failed()`, `self.rolled(io.rollback())` and `self.keep = true`. The old compound mutation is historical evidence only. No final mutation kill is established. | G |
| AD-1-R1 | REPAIR. Not ready for Owner adoption. | this file |

Neither ruling nor this draft accepts D2A-1, integrates it, or completes Phase Three.

### 0A.2 This mission's repairs

| Item | Subject | Section |
|---|---|---|
| R2-01 | typed receipt requirements; S-6 no longer applies G.3(c) to every failing receipt; the witness-only rules apply only to the C1/C25 witness invocation | 1 (S-6), F.1, G.3 |
| R2-02 | process-aware, fail-closed classification in place of "classification by text, never by exit status"; raw outcome kept apart from admissibility; INCOMPLETE — BUDGET EXCEEDED; classification table | 1 (S-1, S-14), F, G.3, K |
| AU | the contract's and the binding reviews' Lead Architect, First Architect and Codex review roles mapped to the Owner's current arrangement | 1 (S-20, S-21), R |
| TR | line 609's trace phrase replaced by one scoped, explicit statement; O.4 has one operative reading | 1 (S-2, S-19), O |

Every other disposition below (A to Q) is carried unchanged, except where these four items name its text.

### 0A.3 Dispositions of AD1R2-CONTRACT-EVIDENCE-01, carried

| Disposition | Subject | Section |
|---|---|---|
| A | F-1 ceiling: standing 60M proposal, not effective until Owner approval; no one-run authorisation inferred | A |
| B | F-3 equivalence: conditional recognition through exact, independently reviewed admission and symmetry proofs; no proof accepted here | B |
| C | 30M quotient keys only for expressly admitted F-3, F-PT and F-B; derived original counts do not consume it | C |
| D | properties, D1, L1, branch coverage, replay; exact measured quotient counts; derived original states; labelled bounds; no new instrumentation | D |
| E | 8.3 and 8.4 reconciled with quotient use; inventory of every `#[ignore]` with a proposed disposition | E |
| F | failing receipts on the original explorer or the named test; A4; C7 without A4 | F |
| G | C1/C25: narrow C25 definition; first failure, named-property witness, healthy replay; positive-line rule; provenance | G |
| H | historical manifest preserved; proposed current-tree manifest; self-reference resolved by a detached attestation | H |
| I | exact-final-baseline evidence; build identity; proposed receipt linking | I |
| J | A-1/A-2 undischarged | J |
| K | inclusive ceilings; expansion-boundary detection; precondition refusal is INCOMPLETE — VERIFICATION INVALID | K |
| L | R1: the sentence beginning "The loop needs no load" | L |
| M | historical INCOMPLETE verdicts kept; history inventory | M |
| N | proof and executable binding: source and build-input closure | N |
| O | attribution and traces: what path minimality is established | O |
| P | test-only boundary; this packet's additions only | P |
| Q | result-line counts by configuration | Q |

### 0A.4 Earlier rulings kept

AD-1-R1 encoded the rulings A to J of P3-C11-DR01-D2A1-AD1R1-MUT-READINESS-01 (F-1's ceiling as a proposed
standing exception; F-3's quotient conditionally recognizable; C-1; obligations kept; F-3, F-PT, F-B only; F-1 for
mutation controls; C1/C25 option A; historical manifest preserved and a separate current-tree manifest; exact
final-commit evidence; A-1/A-2 undischarged). This draft keeps each of them and narrows them where the dispositions
above direct. AD-1-R1 remains the record of their wording.

---

## 1. Supersession table

Each clause **stays in force** except as the "Effect" column states. The effects are:

- **REPLACED**: the text no longer governs for the stated scope; this draft's text governs there instead.
- **SUPPLEMENTED**: the text governs, with the added rule.
- **CLARIFIED**: the text governs as written; the addendum states its one reading and changes no obligation.
- **PROPOSED CLARIFICATION**: as CLARIFIED, but explicitly open: until approved, the present reading governs.

Numbers S-1 to S-15 are AD-1-R1's. S-16 to S-18 are AD-1-R2's. S-19 to S-21 are new. S-1, S-2, S-6 and S-14 are
amended by this draft (0A.2).

| # | Line(s) | Clause (short name) | Effect, with its scope | Section |
|---|---|---|---|---|
| S-1 | 855 | caps of a full run | REPLACED for F-1's state ceiling (A; only once the Owner approves). REPLACED for what the 30,000,000 counts in F-3, F-PT and F-B runs admitted by E (C). CLARIFIED: "Reaching one" (K.1). SUPPLEMENTED: a raw FAIL found once the discovered count exceeds the ceiling is kept as printed, and its outer gate is INCOMPLETE — BUDGET EXCEEDED (K.1); "a run killed from outside" includes a run killed after it printed PASS (K.2). Time, memory, every other configuration and every INCOMPLETE cause: unchanged. | A, C, K |
| S-2 | 609 | the explorer visits every interleaving | REPLACED for F-3, F-PT and F-B as the means of their unchanged-core 8.3 evidence: the admitted run visits one representative of each equivalence class (B, E). The sentence's last phrase is S-19's. Otherwise unchanged for every original run. | B, E |
| S-3 | 620 | equal keys are one state | REPLACED for E's quotient runs: the admission counter and the gating callers' orientation are left out of the quotient key by the equivalence, not by omission (B, N); the receipt names both. Unchanged for every original run. | B, D, N |
| S-4 | 831 | run on the exact final commit, every one | REPLACED for the unchanged-core runs of F-3, F-PT and F-B: "every one" is discharged by E's inventory. SUPPLEMENTED by I. "Missing evidence for any of them is a failed mission": unchanged. | E, I |
| S-5 | 853 | failing and restored receipts | REPLACED for the form of the restored receipt on F-PT and F-B, and for the unchanged-core side of the C7-without-A4 comparison (F). SUPPLEMENTED: failing receipts and build identity (F, I). PROPOSED CLARIFICATION: receipt linking (I). | F, I |
| S-6 | 693 | a control must make its property fail | REPLACED for C1 and C25 only: the kill criterion is G.3. SUPPLEMENTED for every control: each receipt meets the requirements of its own type (F.1) and is classified by K.2. G.3(c) applies only to the C1/C25 witness invocation, never to any other receipt. Named-property attribution, the rejection of fixture and unrelated failures, the C22 exception as contracted, and the stop rule: unchanged. | F, G, K |
| S-7 | 699 | row C1 | REPLACED: the cell "P1" is read as "P1 (first failure E3; witness)". CLARIFIED: the cell "the count becomes a flag" has the exact definition of G.2. | G |
| S-8 | 721 | row C25 | REPLACED: the cell "P7" is read as "P7 (first failure E4; witness)". CLARIFIED: the cell's change has the exact narrow definition of G.2. | G |
| S-9 | 863 | receipts of every run | REPLACED for E's quotient runs: "states and transitions" are the measured quotient counts, with the derived original states where N holds (D). REPLACED: "with the receipt that shows it ran" becomes "with the receipt that shows it ran, or with its approved disposition" (E). SUPPLEMENTED: count labels and replay (D). | D, E |
| S-10 | 911 | manifest control M87 | REPLACED as to *how* the text update is made: only in the current-tree manifest | H |
| S-11 | 912 | manifest controls of `P3` | REPLACED as to *how* the text update is made; its condition kept | H |
| S-12 | 915 | D2A-1 does not edit the manifest; B/Q-5 | SUPPLEMENTED (not replaced): the revision it requires, and its detached attestation, are specified | H |
| S-13 | 548 | rule R1 | REPLACED: only the sentence beginning "The loop needs no load". The other sentences of R1 stay as written. | L |
| S-14 | 813 | three outcomes | CLARIFIED: "a cap was reached" (K.1). SUPPLEMENTED: the printed outcome is the raw explorer outcome; whether the run is admissible evidence is decided separately (K.2). A quotient precondition refusal is INCOMPLETE — VERIFICATION INVALID, and a raw FAIL beyond the authorised cardinality is INCOMPLETE — BUDGET EXCEEDED (K.1). "INCOMPLETE is never PASS": unchanged. | K, M |
| S-15 | 827 | caps of the default suite | CLARIFIED: "under its cap" (K) | K |
| S-16 | 742 | row A4 | SUPPLEMENTED: its run is F-PT's 8.3 run, which may be F-PT's admitted quotient run, compared with F-3's (F). No other actor or core change may run under a quotient. | E, F |
| S-17 | 748 | C7 without A4 on F-3 | SUPPLEMENTED (second sentence only): the mutant runs on the original explorer; its unchanged-core side may cite an accepted exact-final-baseline F-3 quotient PASS (F). It is still added to the receipts and still not required to kill. The first sentence: unchanged. | F |
| S-18 | 923 | the label's first condition | CLARIFIED (consequential): "every run of 8.3" reads per E's inventory; "every control is killed" reads per G for C1 and C25 and per F for A4. Using the label remains the Owner's decision. | E, F, G |
| S-19 | 609 | the trace phrase "so that a violation has a shortest trace" | REPLACED, for every run (original or quotient, healthy or mutant), by O.4's statement: a shortest breadth-first parent path to the checked state, with the offending transition appended where applicable. No global counterexample minimality and no shortest cycle witness is claimed. The printed trace wording and the replay requirement (D) are unchanged. | O, D |
| S-20 | 923, 928, 976 | the evidence reviewer named Codex | REPLACED as to who reviews: the Sole Architect (ChatGPT) reviews Claude Code's evidence independently (R). Unchanged: the review must actually pass; a reviewer does not accept a repair it wrote; the Owner decides whether the label is used. | R |
| S-21 | 4, 5 | the Lead Architect and the First Architect | CLARIFIED: these role names, and their other occurrences listed in R, read under the Owner's current arrangement (R). Historical authorship, dates and acceptances are unchanged. | R |

### Verbatim text of each superseded clause

**S-1, line 855** (verbatim):

<!-- VERBATIM 855 -->
```text
{{L:855}}
```

**S-2, line 609** (verbatim):

<!-- VERBATIM 609 -->
```text
{{L:609}}
```

**S-3, line 620** (verbatim):

<!-- VERBATIM 620 -->
```text
{{L:620}}
```

**S-4, line 831** (verbatim):

<!-- VERBATIM 831 -->
```text
{{L:831}}
```

**S-5, line 853** (verbatim):

<!-- VERBATIM 853 -->
```text
{{L:853}}
```

**S-6, line 693** (verbatim):

<!-- VERBATIM 693 -->
```text
{{L:693}}
```

**S-7, line 699** (verbatim; a row of the table headed at line 697, quoted with its header line):

<!-- VERBATIM 697,699 -->
```text
{{L:697}}
{{L:699}}
```

**S-8, line 721** (verbatim; the same table):

<!-- VERBATIM 697,721 -->
```text
{{L:697}}
{{L:721}}
```

**S-9, line 863** (verbatim):

<!-- VERBATIM 863 -->
```text
{{L:863}}
```

**S-10, line 911** (verbatim; a row of the table headed at line 902):

<!-- VERBATIM 902,911 -->
```text
{{L:902}}
{{L:911}}
```

**S-11, line 912** (verbatim; the same table):

<!-- VERBATIM 902,912 -->
```text
{{L:902}}
{{L:912}}
```

**S-12, line 915** (verbatim):

<!-- VERBATIM 915 -->
```text
{{L:915}}
```

**S-13, line 548** (verbatim; a row of the table headed at line 546):

<!-- VERBATIM 546,548 -->
```text
{{L:546}}
{{L:548}}
```

**S-14, line 813** (verbatim):

<!-- VERBATIM 813 -->
```text
{{L:813}}
```

**S-15, line 827** (verbatim):

<!-- VERBATIM 827 -->
```text
{{L:827}}
```

**S-16, line 742** (verbatim; a row of the table headed at line 737):

<!-- VERBATIM 737,742 -->
```text
{{L:737}}
{{L:742}}
```

**S-17, line 748** (verbatim):

<!-- VERBATIM 748 -->
```text
{{L:748}}
```

**S-18, line 923** (verbatim):

<!-- VERBATIM 923 -->
```text
{{L:923}}
```

**S-19, line 609** (verbatim; the same line as S-2, whose last phrase S-19 replaces):

<!-- VERBATIM 609 -->
```text
{{L:609}}
```

**S-20, line 923, 928 and 976** (verbatim; line 976 is a row of the table headed at line 970):

<!-- VERBATIM 923,928,970,976 -->
```text
{{L:923}}
{{L:928}}
{{L:970}}
{{L:976}}
```

**S-21, line 4 and 5** (verbatim; the contract's header):

<!-- VERBATIM 4,5 -->
```text
{{L:4}}
{{L:5}}
```

### Not superseded

The following stay in force exactly as written. They are quoted so that no gate is weakened silently.

<!-- VERBATIM 198,199 -->
```text
{{L:198}}
{{L:199}}
```

<!-- VERBATIM 584 -->
```text
{{L:584}}
```

<!-- VERBATIM 684,685,686,687,688,689 -->
```text
{{L:684}}
{{L:685}}
{{L:686}}
{{L:687}}
{{L:688}}
{{L:689}}
```

<!-- VERBATIM 679 -->
```text
{{L:679}}
```

<!-- VERBATIM 750 -->
```text
{{L:750}}
```

<!-- VERBATIM 857 -->
```text
{{L:857}}
```

<!-- VERBATIM 859 -->
```text
{{L:859}}
```

<!-- VERBATIM 865 -->
```text
{{L:865}}
```

<!-- VERBATIM 940,941,942 -->
```text
{{L:940}}
{{L:941}}
{{L:942}}
```

Notes on three of them:

- **Line 750.** The patch files in this draft's review packet are inert documentary copies. They are not a control
  script, are never applied from the repository, and add nothing to the mutation-control manifest.
- **Line 865.** The receipts stay in directories outside the repository. The review packet holds byte copies made for
  transport, on a review ref. The originals stay authoritative.
- **Line 942.** It applies to every run of 8.2 or 8.3 made at the final commit, whether or not E's inventory requires
  that run (E.4). The historical INCOMPLETE runs of M predate the final commit. They are not runs of the final
  mission's evidence gate.

Also not superseded:

- every property of 7.3 and 7.4, and every row of 7.5 other than the readings of S-7, S-8 and S-16;
- each control's configuration;
- the bounds of every row of 8.3;
- 8.2 (the default suite), except the reading of "under its cap" (S-15);
- section 9;
- the pins of section 10 other than S-10 and S-11;
- the label conditions of section 11, read as S-18 and S-20 state;
- section 14's other preconditions, and section 15, read under S-21 (role names only);
- every stop condition of section 12.

---

## A. F-1's ceiling — a proposed standing exception

**Text.** "F-1 alone runs with a state ceiling of 60,000,000 states, counted by the original explorer. Its time
(45 minutes) and memory (16 GiB) ceilings, and every other cap of every configuration, are unchanged. This is an
investigation ceiling, like the others. It is not a measured or accepted cost. It is a standing exception to 8.3's
30,000,000, in force only once the Owner approves it. Once in force, it applies to every F-1 run, healthy or mutant.
F-D, though defined from F-1, keeps 30,000,000."

**Until the Owner approves it.** No F-1 run under 60,000,000 is authorised by this draft. Each earlier one-run
exception was spent on its own run (CAMPAIGN-01's F-1; F1-CAP60-01). No further one is inferred: not from those
exceptions, not from this draft, and not from the test source. A run of F-1 under 30,000,000 cannot complete (F-1
has 46,524,336 states, EXECUTOR-REPORTED), so G.3(a) waits for this approval or for an explicit Owner authorisation
that names its runs.

**Source fact (PROVEN FROM SOURCE).** `full_f1` has run `Config { cap: 60_000_000, ..F1 }` since 243501f0; at
42dd1bd1 this is `model_tests.rs` lines 4167–4174. The `F1` constant carries `FULL_CAP` (line 78, 30,000,000), so F-D
and every other user of `F1` keep 30,000,000. A cap in test source is not an approval.

**Evidence (informational under I).** F-1 PASS: 46,524,336 states, complete, at 244c5b23 (CAMPAIGN-01); the same
state count at 243501f0 (F1-CAP60-01). F-1 needs no quotient.

## B. F-3's completed quotient — conditionally recognizable

**Text.** "F-3's full exploration may be established by one complete run of the composed quotient — the admission
observer quotient composed with the gating-caller symmetry quotient, `explore_symmetric(&F3, step_adm,
Mode::Quotient)` as `gc_symmetry_f3` runs it — on the exact final commit, if and only if:

1. the exact admission equivalence (Theorem 1 with Q1–Q15, AQ1–AQ6) and the exact symmetry equivalence (Lemma E
   with S1–S15) have each passed independent review against the exact final source and its build-input closure
   (N), as equivalences of the original graph and not as bounds;
2. every prerequisite of N holds at that commit;
3. the run's runtime guards raise no refusal. These are:
   - the admission observer's checks at every expanded key;
   - the symmetry guard: involution; the expansion of each key's mirror held to the exchange; D1 judged alike.
   Any refusal ends the run with no verdict (K);
4. the run completes (no unexpanded key) within C's ceiling, and every applicable property line, D1 and L1 hold (Q);
5. the Owner has approved this addendum.

The receipt states that the original graph was not explored. Until all five hold, a composed-quotient PASS of F-3 is
an experimental result and is not 8.3 evidence."

No proof is accepted by this draft or by the mission that produced it. The same text, with the extension of E.1, is
what admits F-PT and F-B.

**Exact equivalence basis.** K is the number of quotient keys. Kf is the number of expanded keys that are their own
mirror; the run prints it ("<n> expanded keys are their own mirror"), and in a complete run every key is expanded.
N is the number of admissions (2). Under B(1):

- admission-quotient states |R_Q| = 2K − Kf;
- original states |R_G| = (N+1)·|R_Q|.

For the experimental runs these give (DERIVED, conditional on B(1); informational, not claims):

| Run | K | Kf | 2K − Kf | (N+1)·(2K − Kf) |
|---|---|---|---|---|
| F-3 at 77a146ca | 20,431,851 | 880,778 | 39,982,924 | 119,948,772 |
| F-PT at 2d83b74b | 29,751,473 | 1,698,028 | 57,804,918 | 173,414,754 |
| F-B at 2d83b74b | 3,775,373 | 156,463 | 7,394,283 | 22,182,849 |

The original graph's transitions, fault edges, no-step states and admitted evaluations are not derivable exactly from
what the run prints. D says how they are reported.

## C. C-1 — what a ceiling counts for an equivalent-graph run

**Text.** "For F-3, F-PT and F-B, and only when E admits them, the 30,000,000-state ceiling of 8.3 counts the keys of
the composed quotient as that explorer counts them (K). DERIVED original counts (B, D) are reported beside it and do
not consume that ceiling. The time (45 minutes) and memory (16 GiB) ceilings bound the real process. They are enforced
by the command that starts the run, and are unchanged. Section K's inclusive rule applies to that key count. Reaching
any ceiling is INCOMPLETE, and an INCOMPLETE quotient run supports no derived statement. For every other configuration, and for
every mutant run, the ceiling counts the original explorer's states."

**Margins observed (informational).** F-3: 20,431,851 keys, 68.1% of the ceiling. F-PT: 29,751,473 keys, 99.17%; a
model change that adds 248,528 or more keys makes it INCOMPLETE (K). F-B: 3,775,373 keys, 12.6%.

## D. Obligations kept under a quotient

**Text.** "A quotient run carries every obligation of 7.3, 7.4, 8.1 and 8.4 that an original run carries. Where it
cannot discharge one exactly, it says so, and does not substitute an estimate."

- **Safety.** A quotient PASS establishes the 7.4 safety claims for the original graph only through B(1). The
  receipt says so. The set of reachable violated property names is the original's (S8, Q12); the first-reported name
  may differ (O).
- **D1.** Judged at every key with no enabled non-ADM step, with the admissions spent, at the key and at its mirror.
- **L1.** Kahn's algorithm over the quotient's non-fault edges. It transfers to the original by S10 (both directions)
  and Q10. ADM steps are observations, never edges. L1 stays "not claimed" where 7.4 does not claim it (Q).
- **Branch coverage.** 8.4 requires, for each branch of each operation, whether any run took it. The receipt lists
  that per branch label. Under B(1) the quotient's taken set equals the original's (S7, Corollary 1). Branches no run
  takes are listed NOT TAKEN, as now. Per-member statements (for example, which member of F-B's batch took a branch)
  are NOT ESTABLISHED by aggregate labels and are not claimed. 8.4 does not require them.
- **Replay.** Any FAIL trace is unfolded (S11) and replayed in the original semantics before it is cited, with the
  callers' own names and, for D1, the N admission steps appended (Q11).
- **Counts.** Each count is labelled:
  - MEASURED: printed by the run itself. Mandatory and exact: keys (states), expanded keys, transitions, fault
    edges, no-step keys, non-fault edges (L1's line), admission evaluations, admitted, and Kf.
  - DERIVED: the original states |R_G| of B. Exact, and stated only where B(1) and N hold. Otherwise not stated.
  - BOUND: optional. Any bound on an original-graph count (for example, under B(1), original non-ADM transitions
    between (N+1)·T and 2(N+1)·T for T measured quotient transitions) is labelled BOUND. A bound never replaces a required
    MEASURED or DERIVED value and is never written as a value.
  - For 8.4's "states and transitions", a quotient receipt gives the MEASURED quotient states and transitions and,
    where N holds, the DERIVED original states. The original graph's transitions are not given as values (S-9).
- **No new instrumentation.** No fixed-share count (the self-mirrored share of transitions or edges) and no full-run
  per-member instrumentation is added. AD-1-R1's open item 1 is closed by this sentence.
- **E10** keeps its rule (Q).

**Diagnostic tests kept.** These ordinary tests of `model_tests.rs` are prerequisites of every quotient claim (N.8).
They must pass at the exact final commit on the pinned build. They are diagnostic evidence for the proofs, not
section 7.5 kill receipts (F). At 42dd1bd1 all 51 passed (EXECUTOR-REPORTED; AD1R1-MUT-READINESS-01 stage5, gates
31–37):

| Group | Tests |
|---|---|
| admission-quotient differentials (5) | `quotient_x1_matches_the_original` … `quotient_x5_matches_the_original` |
| admission-quotient negatives, observer, refusals (9) | `quotient_negative_e2_admission_views_twice`, `quotient_negative_e9_admission_ignores_the_latch`, `quotient_negative_p12_admission_ignores_recovery`, `quotient_negative_p1_admission_ignores_unapplied`, `quotient_observer_reports_the_admission_properties`, `quotient_refuses_an_admission_that_keeps_bits`, `quotient_refuses_an_admission_that_keeps_its_counter`, `quotient_refuses_an_admission_that_writes_the_table`, `an_admission_that_keeps_its_counter_fails_l1_in_the_original` |
| symmetry differentials (5) | `gc_symmetry_x2_matches_the_original`, `gc_symmetry_sym1_matches_the_original`, `gc_symmetry_sym_pt_matches_the_original`, `gc_symmetry_sym_b_matches_the_original`, `gc_symmetry_sym_bi_matches_the_original` |
| configuration exercise (3) | `gc_symmetry_sym_pt_takes_the_a4_steps`, `gc_symmetry_sym_b_takes_the_batch_steps`, `gc_symmetry_sym_bi_takes_the_batch_inserts` |
| negatives and fail-closed (24) | the 24 names of gate 36's command line, `gc_symmetry_negative_*`, `gc_symmetry_allow_list_is_exact` and `gc_symmetry_refuses_*` |
| symmetry supporting (5) | `gc_symmetry_finds_the_original_l1_cycle`, `gc_symmetry_initial_states`, `gc_symmetry_l1_on_finite_graphs`, `gc_symmetry_properties_follow_the_exchange`, `gc_symmetry_renames_consistently` |

## E. Which configurations may use the composed quotient, and the final test inventory

### E.1 Admission

**Text.** "Only F-3, F-PT and F-B may use the composed quotient for 8.3. Each may do so only when all of these hold:

- the final source review of B(1) and of N;
- its own complete run at the exact final commit;
- the Owner's approval of this addendum.

F-PT and F-B use the same proofs, extended by E-PT and E-B (CLOSURE-PREP-01, Stage 2, items 1–17). F-1 and F-2
complete under the original explorer and use it. Every other configuration uses the original explorer, unless a later
amendment admits it after its own audit. The gating-caller symmetry is never available to:

- F-5, F-6, F-7, F-8 or F-B2 (acknowledgements or callers, ASYM-1/2);
- F-G (ASYM-3).

No mutation control is ever run under a quotient (F)."

**Enforcement (PROVEN FROM SOURCE).** The `SYMMETRIC` allow-list (`model_tests.rs` line 6079 at 42dd1bd1) and
`assert_symmetric` (lines 6095–6128) refuse, before exploring: an unlisted shape; a raised cap or time cap; a seeded G2 conflict or
acknowledgements; an initial state that is not its own mirror.

### E.2 Sections 8.3 and 8.4 under quotient use

| Clause | Under this draft |
|---|---|
| 831, "every one" | Every configuration of 8.3 has exactly one required unchanged-core run at the exact final commit. For F-3, F-PT and F-B that run is the admitted composed-quotient run when E.1 holds; otherwise it is the original explorer's. For the other twelve it is the original explorer's. "Missing evidence for any of them is a failed mission" stands. |
| 853 | Failing receipts: F. Restored receipts: F and I. |
| 855 | Ceilings: A, C, K. |
| 857 | Unchanged. It binds quotient runs as it binds original runs: one at a time, the memory check first, no other workload stopped. |
| 859 | Unchanged. The quotient runs use the same reviewed bounds (N.5, N.6). |
| 863 | What a quotient receipt reports: D. The listing of every `#[ignore]`: E.3. |
| 865 | Unchanged (section 1's note). |

### E.3 Inventory of every `#[ignore]` in the two test files

Taken at 42dd1bd1 (PROVEN FROM SOURCE). `model_tests.rs` has 34 ignored tests; `unit_tests.rs` has none. The names are
the actual test names. Under the contract as written, line 831 requires the fifteen `full_*` tests, and line 863
requires every one of the 34 to be listed with a receipt that shows it ran. The proposed dispositions below change
that only if the Owner approves this addendum (E.4).

Dispositions:

- **REQUIRED_FINAL_RUN**: run on the unchanged core at the exact final commit, with its receipt.
- **REPLACED_BY X**: its unchanged-core 8.3 run is discharged by the named run X, approved as an equivalent-graph run
  under B to E and N. If X's conditions do not hold, the test is REQUIRED_FINAL_RUN.
- **EXPERIMENTAL_NOT_REQUIRED**: not run at the evidence gate unless an Architect mission names it, for the reason
  given.

"Mutant runs" are the original-explorer runs this test performs for 7.5 (F). A disposition never changes them.

| # | Test (line at 42dd1bd1) | Runs | Proposed disposition | Mutant runs (F) | Reason or condition |
|---|---|---|---|---|---|
| 1 | `full_f1` (4169) | `explore`, F-1, cap 60,000,000 | REQUIRED_FINAL_RUN | C1, C2, C10, C24, C25, A1, A2 | under A once approved; F-1 cannot complete under 30,000,000 |
| 2 | `full_f2` (4178) | `explore`, F-2 | REQUIRED_FINAL_RUN | C20, C23, A5 | |
| 3 | `full_f3` (4184) | `explore`, F-3 | REPLACED_BY `gc_symmetry_f3` | C7 without A4 (not required to kill) | if E.1 does not hold: REQUIRED_FINAL_RUN; the history (M) indicates it cannot complete under any approved ceiling, and an INCOMPLETE run stops the mission (942) |
| 4 | `full_f4` (4190) | `explore`, F-4 | REQUIRED_FINAL_RUN | C26 | |
| 5 | `full_f5` (4196) | `explore`, F-5 | REQUIRED_FINAL_RUN | C5, A3, A6 | |
| 6 | `full_f6` (4202) | `explore`, F-6 | REQUIRED_FINAL_RUN | C3b, C3c | |
| 7 | `full_f7` (4208) | `explore`, F-7 | REQUIRED_FINAL_RUN | C3a | |
| 8 | `full_f8` (4214) | `explore`, F-8 | REQUIRED_FINAL_RUN | C4 | |
| 9 | `full_fpt` (4220) | `explore`, F-PT | REPLACED_BY `gc_symmetry_fpt` | C6, C7 | if E.1 does not hold: REQUIRED_FINAL_RUN |
| 10 | `full_fb` (4226) | `explore`, F-B | REPLACED_BY `gc_symmetry_fb` | C8 | if E.1 does not hold: REQUIRED_FINAL_RUN |
| 11 | `full_fb2` (4232) | `explore`, F-B2 | REQUIRED_FINAL_RUN | C9, C11, C17 | |
| 12 | `full_fg` (4238) | `explore`, F-G | REQUIRED_FINAL_RUN | C18a, C18b | |
| 13 | `full_fw` (4244) | `explore`, F-W | REQUIRED_FINAL_RUN | none | the only configuration where E10 applies (Q) |
| 14 | `full_fd` (4250) | `explore`, F-D | REQUIRED_FINAL_RUN | C16 | L1 not claimed (Q) |
| 15 | `full_fr` (4256) | `explore`, F-R | REQUIRED_FINAL_RUN | C15 | |
| 16 | `quotient_full_f1` (5355) | `explore_experimental`, F-1 | EXPERIMENTAL_NOT_REQUIRED | none | admission observer quotient alone; E admits no configuration to it |
| 17 | `quotient_full_f2` (5361) | `explore_experimental`, F-2 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 18 | `quotient_full_f3` (5367) | `explore_experimental`, F-3 | EXPERIMENTAL_NOT_REQUIRED | none | as 16; INCOMPLETE at 30,000,001 keys (M) |
| 19 | `quotient_full_f4` (5373) | `explore_experimental`, F-4 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 20 | `quotient_full_f5` (5379) | `explore_experimental`, F-5 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 21 | `quotient_full_f6` (5385) | `explore_experimental`, F-6 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 22 | `quotient_full_f7` (5391) | `explore_experimental`, F-7 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 23 | `quotient_full_f8` (5397) | `explore_experimental`, F-8 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 24 | `quotient_full_fpt` (5403) | `explore_experimental`, F-PT | EXPERIMENTAL_NOT_REQUIRED | none | as 16; F-PT's admitted form is the composed quotient |
| 25 | `quotient_full_fb` (5409) | `explore_experimental`, F-B | EXPERIMENTAL_NOT_REQUIRED | none | as 16; F-B's admitted form is the composed quotient |
| 26 | `quotient_full_fb2` (5415) | `explore_experimental`, F-B2 | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 27 | `quotient_full_fg` (5421) | `explore_experimental`, F-G | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 28 | `quotient_full_fw` (5427) | `explore_experimental`, F-W | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 29 | `quotient_full_fd` (5433) | `explore_experimental`, F-D | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 30 | `quotient_full_fr` (5439) | `explore_experimental`, F-R | EXPERIMENTAL_NOT_REQUIRED | none | as 16 |
| 31 | `gc_symmetry_f3` (6933) | `explore_symmetric`, F-3 | REQUIRED_FINAL_RUN, if E.1 holds | none | the replacing run of 3; if E.1 does not hold: EXPERIMENTAL_NOT_REQUIRED |
| 32 | `gc_symmetry_prefix_f3` (6951) | `explore_symmetric`, F-3, cap 99,999 | EXPERIMENTAL_NOT_REQUIRED | none | INCOMPLETE by construction and asserts so; establishes nothing about F-3. Its check (each expanded key's mirror expanded and held to the exchange) is made on every expanded key by 31 (B(3)). |
| 33 | `gc_symmetry_fpt` (8234) | `explore_symmetric`, F-PT | REQUIRED_FINAL_RUN, if E.1 holds | none | the replacing run of 9; if E.1 does not hold: EXPERIMENTAL_NOT_REQUIRED |
| 34 | `gc_symmetry_fb` (8242) | `explore_symmetric`, F-B | REQUIRED_FINAL_RUN, if E.1 holds | none | the replacing run of 10; if E.1 does not hold: EXPERIMENTAL_NOT_REQUIRED |

Control C12 is on X-1, a default exploration (8.2), not an ignored test. A4 is F-PT's own run (F).

### E.4 Rules on the inventory

1. It is recomputed mechanically from both test files at the exact final commit. A name added, removed or renamed
   needs its own disposition before the evidence gate. Whether the experimental tests are kept at the final commit is
   not decided here.
2. A REPLACED_BY disposition takes effect only when its replacing run's conditions hold.
3. An EXPERIMENTAL_NOT_REQUIRED test that an Architect mission does run keeps its receipt and its verdict as printed.
   Any run at the final commit that ends INCOMPLETE or FAIL on the unchanged core is reported, and the mission stops
   for an Architect ruling (lines 940 and 942, unchanged).
4. This inventory is a proposed amendment disposition. It approves skipping no test the contract now requires. Until
   the Owner approves this addendum, lines 831 and 863 govern as written.

## F. Mutation controls — the original explorer on the mutant

**Text.** "Every control of 7.5 that names a configuration runs the ORIGINAL explorer (`explore`, through that
configuration's `full_*` or default test), on the mutant build, on its named configuration, under that
configuration's ceiling (F-1: 60,000,000 once A is in force; every other: 30,000,000; default explorations: 250,000).
Every control killed by a named test runs that test on the mutant build: C14 T-BIND, C19 T-CELL, C21 W6, C22 T-OWN,
C27 T-PREC, C28 T-RES. C22 alone is killed by a build failure, with the recorded diagnostic E0283 at the assertion's
line. A control is killed only by an admissible receipt of its own type (F.1, K.2): the original explorer's FAIL of
its named property, read under G for C1 and C25; its named test's failure; or, for C22 alone, that build failure. A
control run that reaches a ceiling without a FAIL, or whose FAIL is found beyond its ceiling (K.1), is not killed, and
7.5's stop rule applies.

After restoration (I), the restored receipt that shows the unchanged core passes the configuration is:

- for F-PT and F-B: either the original explorer's PASS, or an independently accepted composed-quotient PASS of the
  unchanged core on the exact final commit (B to E, N);
- for every other configuration: the original explorer's PASS on the exact final commit.

A quotient run is never a failing receipt. A control is never run under a quotient. An approved quotient diagnostic
test (D) is never a 7.5 kill receipt, whatever it shows on a mutant."

**A4.** A4 is not a change to the core. It is the actor program that defines F-PT (`blanket_uncertain`; its symmetry
was argued as E-PT in CLOSURE-PREP-01), so its run is F-PT's 8.3 run, original or, when E.1 holds, admitted composed-quotient. A4 is evidenced by that
PASS, compared with F-3's 8.3 PASS: both verdicts, both branch sets and both sets of measured counts. This is the
explicitly audited F-PT configuration comparison. It is not permission to run any source mutant, or any other actor
change, under a quotient.

**C7 without A4 (line 748).** The run is kept and its execution does not move. It is C7's mutant build, run by
`full_f3` (the original explorer, configuration F-3, A4 off), under 30,000,000. It is expected to fail E11. It is added
to the receipts and is not required to kill. Its comparison needs the unchanged core's F-3 result. That side may cite
the accepted exact-final-baseline F-3 composed-quotient PASS, under the same B to E and N prerequisites, because the
original F-3 did not complete at 30,000,000 or at 60,000,000 (M). Nothing waives the comparison. If the mutant run ends
INCOMPLETE, that is recorded as it is and reported (E.4 rule 3).

**Why the controls cannot move** (ADM-QUOTIENT-01 Q15; GC-SYMMETRY-01 S15; CLOSURE-PREP-01 item 17):

- a control is a model change, which voids the source audits (AS4, AQ1);
- the symmetry guard would end an asymmetric mutant's run "symmetry inapplicable", which kills nothing (K);
- the first-reported property may differ (O).

### F.1 Receipt types: what each must show

Every receipt is first classified under K.2. It must then show what its own type requires below, and nothing of
another type is required of it. A field that a type does not produce is recorded as NOT PRODUCED BY DESIGN. It is
never invented.

Common to every type, as the contract and AD-1-R2 already require:

- the exact baseline commit and tree, and for a mutant the identity of I (control id, patch and its sha256, the changed
  blobs);
- the actual test or build selected and the configuration it runs, read from the receipt itself;
- the raw output, complete and unedited, with its sha256;
- a fixture failure, an unrelated panic or failure, or a failure of anything other than the named property or the
  named test, kills nothing.

**(a) Original-explorer mutation receipt.** Every control of 7.5 that names a configuration (A1, A2, A3, A5 and A6
included; A4 is (f)), and the primary receipt of C1 and C25.

- Selection: exactly the configuration's test (`full_*` with `--exact` and `--ignored`, or the default test with
  `--exact`), one match, on the pinned mutant executable of I.
- Harness result: that test failed by the explorer's own verdict assertion (`full_exploration` or
  `default_exploration`, `model_tests.rs` lines 4130–4140 and 4115–4128 at 42dd1bd1), whose message carries
  `failed: {"<property>"}` and the report. The process exited 101, by a normal exit and not by a signal.
- Raw output: the report with `outcome: FAIL`, `FAIL: <property>: <detail>`, the line `states <n> (expanded <m>)`,
  and the trace under `shortest trace (<n> steps):`.
- Cardinality: one test ran, and the printed discovered count is at most the configuration's ceiling (K.1).
- Attribution: the printed property is the control's named property. For C3a, C3b and A6, either property the row
  names, as its "or" states. For C4, the reading of "P4, P8" is not settled here (open item 10). For C1 and C25 the
  printed property is recorded as it is under G.3(a). It is not their kill's attribution, which comes from G.3(b).
- Not required: any witness line, witness locus or witness verdict. A FAIL without a witness marker is complete under
  this type.

**(b) Named non-explorer unit-test receipt.** C14 T-BIND, C19 T-CELL, C21 W6, C27 T-PREC and C28 T-RES.

- Selection: exactly the named test, `ledger::protocol::unit_tests::<name>` with `--exact`, one match, on the pinned
  mutant executable of I.
- Harness result: that test failed at one of its own checks, which the receipt identifies by the panic's message and
  its `unit_tests.rs:<line>` location. The process exited 101, by a normal exit and not by a signal.
- Rejected: a failure in shared setup, or in a fixture before the test's own checks; a failure anywhere else; a pass.
- Not required: an explorer outcome line, or any witness line.

**(c) C1/C25 witness receipt.** G.3(b) on the mutant executable, and G.3(d) on the baseline executable. The exact
positive line, the header, the witness locus, the line of other properties and the rejections of G.3(c) apply to this
invocation and only to it. They do not apply to the primary receipt of C1 or C25 (type (a)), or to any other control.

**(d) C22's intentional compile-failure receipt.** The contract's one exception (line 731), exactly as contracted.

- Command: the pinned build command of N.7 on the mutant source of I. Its intended result is a nonzero exit of the
  compiler.
- Required: the diagnostic `error[E0283]` at the designated T-OWN assertion (`unit_tests.rs` line 1089 at 42dd1bd1,
  `<Ticket as NotClone<_>>::check();`; the line is derived again at the final baseline); the source and build identity
  of I; the toolchain; the complete compiler output; and healthy build evidence, the same command succeeding on the
  unmutated baseline.
- Rejected as a kill: a compiler crash (an internal compiler error); a resource failure (a signal, an out-of-memory
  kill, a timeout); any error diagnostic other than that E0283 at that line; E0283 at another line.
- NOT PRODUCED BY DESIGN: the mutant test executable, its sha256 and size, the test selection and the test run. No
  executable hash is invented, and no witness is required.

**(e) Unchanged-core PASS receipt.** Every required run of 8.2 and 8.3 (E), and every restored receipt of 7.5 (I).

- Selection: exactly the configuration's test with `--exact` (and `--ignored` for `full_*`), one match, on the pinned
  baseline executable; for an admitted quotient run, the test of N.11.
- Harness result: the test passed (`... ok`, then `test result: ok. 1 passed; 0 failed`). The process exited 0, by a
  normal exit and not by a signal. The launching command records no kill, timeout or out-of-memory event, and time
  and memory stayed within its limits.
- Raw output: `outcome: PASS`, every property line of Q, the counts of D and the branch list. The discovered count is
  at most the ceiling (K.1). For a quotient run, every prerequisite of N, and no refusal.

**(f) The A4 comparison.** Two admissible receipts of type (e), F-PT's and F-3's 8.3 runs, compared in a separate
record: both verdicts, both branch sets and both sets of measured counts. It is evidence for A4 only, and it is not a
kill receipt.

## G. C1 and C25 — first failure, named property, healthy replay

### G.1 Status, with provenance

| Item | Status |
|---|---|
| Witness source, commit 42dd1bd1 | PROVEN FROM SOURCE: published on `review/p3-c11-d2a1-ad1r1-witness-01`; one file, +277/−0, appended after the parent's end. ARCHITECT-RULED: interim source review passed; the source-review finding is maintained (AD1R3-READINESS-01). |
| C1 probe (AD1R1-MUT-READINESS-01) | EXECUTOR-REPORTED: P1 observed at step 11 with "admission: admitted"; E3 listed separately; healthy and restored runs PASS. ARCHITECT-RULED (AD1R3-READINESS-01): the historical receipts support the intended P1 attribution. Historical evidence, not a final-baseline receipt. No final kill. |
| C25 historical compound probe (AD1R1-MUT-READINESS-01, patch `a6de4ed9…656b`) | EXECUTOR-REPORTED. Historical evidence only. It also removed `self.keep = true`. It is not the control's definition. |
| C25 narrow probe (AD1R1-WITNESS-AUDIT-01, patch `29a2c1e5…d732`) | EXECUTOR-REPORTED: P7 observed on the world after step 1; E4 listed separately; U kept (word 0x25). ARCHITECT-RULED (AD1R3-READINESS-01): the retrieved patch and the authenticated historical receipts support P7 with `self.keep = true` preserved; no additional probe is justified. Historical evidence, not a final-baseline receipt. No final kill. |
| The probe helpers (`probe.py`, AD1R1-MUT-READINESS-01; `probe-c25-narrow.py`, AD1R1-WITNESS-AUDIT-01) | Historical evidence. They are not approved final campaign controllers. They are not modified or run again. |
| First failures under the original explorer (E3 for C1, E4 for C25) | NOT ESTABLISHED. Predictions (EXECUTOR-REPORTED static analysis). No `full_f1` run on either mutant has been made. |

### G.2 Proposed control definitions (future disposable mutations; NOT APPLIED here)

Each is applied only in a disposable worktree, by a script kept outside the repository (line 750), at the exact
final baseline, and never committed. Their documentary patches are in the review packet as inert copies.

- **C1, "the count becomes a flag".** At the head of `count_step` (`protocol.rs` line 175 at 42dd1bd1), for the
  `unapplied` field only, the step sets the field to 1 on a step up and 0 on a step down:
  `if shift == UNAPPLIED_SHIFT { return with_field(word, shift, u64::from(up)); }`. Documentary patch
  `C1-mutation.patch`, sha256 `5ecd716a8f123d61034b0d274238898e13a116a9cc464c7645a9736934970252` (the encoding the
  Architect ruled provisionally valid).
- **C25, "a gating append answers Committed on `Commit::Unknown`".** Within `Publish::gating` (`protocol.rs` line
  1413 at 42dd1bd1), in the arm `Insert::Inserted(seq)` → `Commit::Unknown`, preserve
  `self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`, and replace ONLY
  `return refused(Persistence::Uncertain, Reason::Storage);` (line 1476, its only occurrence in the file) with
  `return GatingOutcome::Committed(seq);`. Documentary patch `C25-narrow-mutation.patch`, sha256
  `29a2c1e571b4f7fd278d106fe3d40a8cf912119dbe44fbbbbd92855431ecd732`. The compound mutation that also drops
  `self.keep = true` is historical evidence only.

### G.3 The kill criterion, for C1 and C25 only

"For C1 and C25 only, the control is killed when (a), (b) and (d) all hold and none of (c) occurs; it is then
recorded as (e) states. Every receipt is made on the mutant build of I, or on the unmutated build of the exact final
baseline.

(a) **The first failure is preserved as primary.** The unchanged original explorer, run as `full_f1` selected alone
with `--exact` (and `--ignored`, since it is an ignored test) on the mutant executable, FAILs. Its raw report is kept unedited: the line `outcome: FAIL`, the line
`FAIL: <property>: <detail>`, and the trace printed under `shortest trace (<n> steps):`.
- Predicted: E3 in 4 steps for C1; E4 in 1 step for C25.
- If it reports otherwise, that is reported as it is and not reinterpreted. The control is then not recorded as
  killed under this clause until the Architect rules.
- E3 is never counted as P1, and E4 never as P7.
- An INCOMPLETE run is not a FAIL, and neither is a FAIL found beyond the ceiling (K.1). This receipt is of type (a)
  of F.1 and is classified under K.2: neither its text nor its exit status alone decides. No witness line is required
  of it.

(b) **The named property is established by exact replay.** The control's witness test, selected alone with
`--exact` on the same pinned mutant executable as (a), with a listing that shows exactly one match, FAILs, and its
raw output contains the exact positive line at the required locus:
- C1: `NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)`;
- C25: `NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))`.
The same output must also show:
- the header `C1 WITNESS F-1: 11 steps, each enabled and taking its choices; <n> violations`, or
  `C25 WITNESS F-1: 1 steps, each enabled and taking its choices; <n> violations`;
- E3 (C1) or E4 (C25) in the line `OTHER PROPERTIES VIOLATED, each reported separately and not counted as P1: [...]`
  (or `… as P7: [...]`).

(c) **Rejections of the witness receipt.** They apply only to the witness invocations of (b) and (d) (F.1 type (c)),
never to the primary receipt of (a) or to any other control's receipt. Any of these means the witness receipt
establishes nothing under this clause:
- a failing exit without the exact positive line of (b);
- `NOT OBSERVED` in the named-property line, or `OBSERVED` at another step or locus;
- any `WITNESS FIXTURE FAILURE`;
- a panic or failure whose message is not the witness's own verdict (an unrelated panic);
- a selection other than exactly the one named test: another test, zero or several matches, or no `--exact`;
- an executable other than the mutant executable of (a);
- a compilation failure. Only C22 is killed by a build failure, and it has no witness.

(d) **The healthy core violates nothing on the same schedule.** The same witness, selected alone with `--exact` on
the pinned executable of the exact final baseline, PASSes and prints:
- C1: `C1 WITNESS F-1: 11 steps, each enabled and taking its choices; no violation; step 11 shows the healthy core's 'admission: refused'`;
- C25: `C25 WITNESS F-1: 1 steps, each enabled and taking its choices; no violation; step 1 shows the healthy core's 'append_gating: Refused Uncertain Storage'`.

(e) **Recording.** Only then are the rows recorded as 'C1: P1 (first failure E3; witness)' and 'C25: P7 (first
failure E4; witness)'."

A failing test exit alone establishes none of (a), (b) or (d). Each of the three receipts must also be admissible
under K.2.

### G.4 The witness's required form

Each is a property of the prototype at 42dd1bd1 (`replay_schedule`, `run_witness`; PROVEN FROM SOURCE):

1. Real protocol operations and the original model transitions: `step`, with `step_adm` as admission.
2. No forced state: the replay starts from `initial(F1)`, and each step runs on a world decoded from the previous key.
3. Real hook counts: `hook::take_counts` around each step, passed to `transition_properties`.
4. Unaltered property predicates: `state_properties`, `transition_properties`, and the properties pushed inside the
   steps.
5. Deterministic replay of every scheduled actor step and every oracle choice.
6. Every step is asserted enabled.
7. Every step's choices are asserted to equal the schedule's. The fixed branch labels are asserted at every step
   except the one the control changes.
8. Every violation is collected: of the initial state; of every step (those pushed inside it, and its transition
   properties); of every state after a step (its state properties and its key's round trip). None stops the replay.
9. The named property is checked at its locus. P1 must be in step 11's own violations, with `admission: admitted`.
   P7 must be in the after-state of step 1, with `append_gating: Committed (inserted)`.
10. On the healthy core the same schedule must show no violation and the healthy branch.
11. Differing behaviour is printed as evidence, step by step (branches, word, ledger), never absorbed into the fixture.

**Location.** Two ordinary (not ignored) tests at the end of `model_tests.rs`:
`c1_witness_p1_at_a_successful_admission` and `c25_witness_p7_on_the_resulting_world`. Each completes in under a
millisecond and touches no shared state but the thread-local hook (EXECUTOR-REPORTED), so it is safe in the default
suite. On the unmutated core each is a regression test of the behaviour its control removes. Carrying them to the
final commit needs its own authorised mission. The schedules' minimality is not claimed (O).

## H. The mutation manifest — original preserved, current-tree manifest separate

### H.1 The historical manifest

`docs/evidence/p3-candidate10/original-183/manifest.json` (blob `65fa69dc`, sha256
`2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14`, version `1-r1`, 183 controls; the same at
42dd1bd1, PROVEN FROM SOURCE) is preserved byte for byte. It is not edited, regenerated or overwritten.

### H.2 The proposed current-tree manifest

A separate file, proposed (selected by the Architect as the proposal):

- path `docs/evidence/p3-candidate11/current-tree-183/manifest.json`;
- version `ct-1`.

It has its own sha256. It is not an explanatory supplement. Final Candidate 11 acceptance requires that it be
reviewed and found aligned with the exact final tree. The 22 stale entries' 'TEXT UPDATE, SAME PROPERTY' (S-10, S-11)
is made only in it. **This draft neither creates nor approves it.** Making it needs its own authorised mission; the
test-only allowance of P does not cover it.

### H.3 What the committed manifest holds

1. `name`, `version` (`ct-1`), and `derived_from`: the original's path, sha256 and version.
2. All 183 controls, in the original's order. For each, these definition fields are copied from the original and
   never changed: `id`, `category`, the two meanings; `edits` (every `file`, `anchor`, `replacement`, `count`, in
   order); `command`, `cwd`, `expected_failure_tests`.
3. For each control, the original's `review_status`, `lineage` and `review`, copied unchanged and marked historical.
   They are not current-tree approval.
4. For each control, a separate current review status. It starts `pending`. No string in the file confers acceptance.
   Acceptance exists only as the Architect's own recorded decision, bound by the attestation of H.6.
5. `source_sha256`: the sha256, at the tree being validated, of each file the control names. These are the hashes of
   the files the manifest actually validates.
6. **No self-reference.** The manifest records no commit or tree hash of the commit that contains it, and requires
   none. AD-1-R1's field `final_tree` is withdrawn.

### H.4 The 22 stale entries

The Architect owns the review coordination of all 22. They stay identified in two sets:

- **15 made stale by D2A-1** (22ebf9a5, expected by contract 10): M32 M78 M79 M80 M89 M112 M113 M141 M143 M153 M177
  M178 M179 M180 on `app/src-tauri/src/phase3_tests.rs` (14), and M87 on `crates/nexus-governed-control/src/lib.rs`.
- **7 earlier Candidate 11 entries**: M18 M19 M39 on `crates/nexus-governed-control/src/broker.rs` (stale at
  862a11ce, the D2A-1 contract base); M84 on `crates/nexus-governed-control/src/authority/approval.rs`, M05 on
  `crates/nexus-governed-control/src/authority/mod.rs`, M45 M129 on `crates/nexus-governed-control/src/authority/run.rs`
  (stale at Candidate 11 b1d535f1).

Each gets its own reconciliation record: control id; file; the original's recorded hash; the validated tree's sha256
and git blob; the commit at which it first went stale; every anchor's occurrence count against the manifest's
`count`, with its line number; a statement that the control's anchors, replacements, command and expected failing
tests are unchanged; and the reviewer's finding (SAME PROPERTY or not), recorded by the reviewer, not by the file.

At 42dd1bd1 a read-only dry run (EXECUTOR-REPORTED, AD1R1-MUT-READINESS-01 stage6/01) found the same 22 stale pairs,
and every anchor of all 183 controls at its `count`. 42dd1bd1 is not the final tree.

### H.5 Alignment check (mechanical)

Every one must hold, or the manifest is not aligned: every recorded `source_sha256` equals the validated tree's file;
every anchor occurs exactly `count` times there; the original manifest's sha256 there is still `2c4adc84…2e14`; the
control set and every copied field equal the original's. A changed anchor, replacement, count, command or expected
test is a redefinition of the control, not a TEXT UPDATE, and needs its own review (line 912's condition). Restoration
after each control is verified by blob equality with the baseline, never by reversing a replacement (M45, M113 and
M129's replacements already occur in their files; M32, M39, M79 and M143 are deletions).

### H.6 The detached attestation

Created after the source freeze, and not inside the source commit whose identity it records. It binds:

- the exact final commit and tree;
- the current-tree manifest's blob and sha256 at that tree;
- the alignment check's results;
- the campaign evidence (receipt hashes);
- the actual current review decisions, each with its source.

It may live in the final evidence directory, or in a later evidence-only commit whose parent is the final commit. It is
never placed in the commit it attests.

## I. Exact-final-baseline evidence, builds and restoration

**Text.** "Every required 8.3 run (E), every 7.5 failing and restored receipt, every C1/C25 witness receipt, the A4
comparison and the C7-without-A4 run is made against the exact final baseline commit, or a mutant of it. No historical
PASS substitutes for a final-baseline run. The following are informational and are not 8.3 or 7.5 evidence:

- F-1 and F-2 PASS at 244c5b23;
- the F-3, F-PT and F-B composed PASS at 77a146ca and 2d83b74b;
- the witness probes at 42dd1bd1 and its parent plus the witness source.

**Builds.** One fresh optimised build of the exact final baseline gives the pinned baseline executable. Its sha256 is
checked before and after each run. One fresh optimised build is made per distinct mutant source identity (control id,
patch sha256, resulting blob set). The same pinned mutant executable serves every invocation of that mutant: its
original-explorer run and, for C1 and C25, its witness run.

A mutant build is identified by all of:

1. the final unchanged baseline commit and tree;
2. the exact named control id and its mutation patch, with the patch's sha256;
3. the exact mutation source blob or blobs: git blob id and sha256 of every file the patch changes, as built;
4. the mutant test executable's sha256 and size, from a fresh optimised build of that source (for C22: NOT PRODUCED
   BY DESIGN, since its build is meant to fail, F.1 (d));
5. the exact configuration and invocation: test name selected with `--exact`, flags, cwd, environment and resource
   limits (for C22: the build command and its environment);
6. the raw failing receipt, unedited, with its sha256 (for C22: the compiler's complete output);
7. the restored baseline identity (every changed file's blob equal to the baseline's, the worktree clean, nothing
   else differing from the baseline commit) and the restored PASS receipt (for C22, it includes the healthy build
   evidence of F.1 (d)).

A mutant tree is not the baseline tree, and no receipt describes it as identical to it. Its identity is the baseline
tree, plus the named patch, verified by blob. Reproducible builds are not established: two builds of one source have
differed in sha256. Every identity is the sha256 of the executable actually run."

**Proposed receipt linking (PROPOSED CLARIFICATION, S-5).** One accepted full unchanged-core receipt per exact
baseline, configuration, build identity (the pinned baseline executable's sha256) and execution policy (command,
flags, limits, environment) may be linked by several control records as their restored receipt, provided each control
record has its own verified baseline restoration (blob equality and a clean worktree after that control) and its own
artifact-identity check (the linked receipt's executable sha256 equals the pinned baseline executable, checked at that
control's restoration). This is proposed for review. It is not permission to omit a restored run the contract now
requires. Until it is approved, the present reading of line 853 governs.

## J. Storage assumptions

No change. A-1 and A-2 (4.1, lines 198–199) stay stated and undischarged until the real storage and shell contract
(6.3; 13). Every result above is conditional on them, and on AQ6 where a quotient is used: original or quotient runs,
PASS or FAIL, explorer or witness. Each result is the explored model's state-safety, D1 and L1. No model result
establishes actual SQLite durability, a real execution, or contract 6.2/6.3. Section 11's scope sentence is unchanged.

## K. Ceilings, and runs that end without a verdict

**Source fact (PROVEN FROM SOURCE).** All three explorers check `visited.len() as u64 > config.cap` before expanding
each key: `explore`, `explore_experimental` and `explore_symmetric_kept`, at `model_tests.rs` lines 3907, 5068 and 6609
at 42dd1bd1. `visited` counts every distinct key discovered, the initial key included, expanded or not. A violation
found while expanding a key ends the run FAIL at once, before the next check. In `explore`, a state-property or D1
violation is found before that expansion adds any key, so its count is at most C. A transition's violation is found
after the earlier successors of the same expansion were added (lines 3932–3944). The printed count (`states <n>
(expanded <m>)`, lines 4048–4053) is taken after the search stops, and nothing is added after the violation, so for a
transition's violation it can exceed C.

### K.1 Ceilings

**Text.** "A state ceiling C of 8.2 or 8.3 bounds the number of distinct keys the run's explorer has discovered. For
an original run these are states; for a quotient run admitted by E, they are quotient keys.

- The cardinality is inclusive: a run is within its ceiling if and only if it completes with at most C keys
  discovered. A run that completes with exactly C is complete and within it.
- 'A cap was reached' (8.1) and 'reaching one' (8.3) mean: at an expansion boundary, more than C keys have been
  discovered. The run then stops INCOMPLETE. Because the check is made only at expansion boundaries, the discovered
  count can overshoot C by at most the new keys of one expansion (observed: 30,000,002 and 30,000,001 at
  C = 30,000,000; 60,000,001 at C = 60,000,000; 100,000 at C = 99,999). The overshoot is how the stop is detected. It
  is not extra budget: any run that discovers more than C keys does not complete.
- An INCOMPLETE receipt prints the discovered count as it is.
- For a FAIL, two facts are kept apart. The raw outcome: the explorer's FAIL, with its property, detail,
  counterexample trace and printed count, kept exactly as printed and never discarded. Its admissibility: the FAIL is
  within the authorised cardinality only if its printed discovered count is at most C. A FAIL whose printed count
  exceeds C was found during the overshoot. Unless a separate, explicit authorisation names that run, its outer gate
  is **INCOMPLETE — BUDGET EXCEEDED**: it is not a final mutation-kill receipt and not the verdict of a complete run,
  and the overshoot is still not budget. No PASS is possible beyond C.
- The time ceiling (45 minutes) and the memory ceiling (16 GiB) are the launching command's, and are not extended.
  The explorer's own elapsed-time check is an earlier stop, also INCOMPLETE.

A runtime quotient-precondition refusal — any panic whose message begins 'quotient inapplicable' or 'symmetry
inapplicable' — ends the run with no property verdict. Its outer receipt is unsuccessful and reads **INCOMPLETE —
VERIFICATION INVALID**, with the raw refusal text and the stop. It is never PASS, never FAIL of a property, and never a
mutation kill. The mission stops and reports it."

**Why this reading.** It is what the frozen code does. It changes no recorded verdict: every complete run so far ended
below its ceiling (the closest is F-PT, at 29,751,473 against 30,000,000). A stricter reading would have the explorer
report PASS for a run that the contract calls INCOMPLETE, a contradiction this addendum must not create. Raw outcomes
are kept as printed; K.1 and K.2 decide only what a receipt may be used as.

### K.2 Classification of a receipt — process-aware and fail-closed

AD-1-R2's rule "classification is by text, never by exit status" is withdrawn. A full test asserts PASS, so FAIL,
INCOMPLETE, a refusal and any other panic all exit 101 (EXECUTOR-REPORTED in the historical receipts); and a PASS
already printed when the process is stopped from outside is not a completed run. So neither alone decides.

**Text.** "Neither the printed output nor the exit status alone establishes any result. A receipt is admissible only
if all of these agree:

1. the authenticated source and executable identity (I, N), the executable checked before and after;
2. the intended test selection and configuration: exactly the one named test, with `--exact` (and `--ignored` where it
   is ignored), one match, and the configuration's printed bounds and actors;
3. the complete raw output, with the named-property evidence that its type requires (F.1);
4. the harness result expected for that invocation type (F.1);
5. the actual process exit status, or the signal that ended it;
6. the launching command's termination and resource-limit records: no kill, timeout or out-of-memory event, and time
   and memory within its limits;
7. completion and cardinality: the run finished, one test ran, and the discovered count is within its ceiling (K.1).

For a test that is not an exploration (F.1 (b) and (c)), point 2 needs no configuration bounds and point 7 no
discovered count. For C22's build-failure receipt (F.1 (d)), points 1, 2, 4 and 7 read as: the source and build
identity; the build command; the intended nonzero compiler result, with E0283 at the designated line; and a build that
finished. No executable, test selection or test run is required of it, since none is produced by design.

PASS text followed by a kill, a timeout, an out-of-memory kill, an incomplete termination, an identity mismatch or
missing completion evidence is not PASS. For an intended assertion failure, the receipt must show the expected,
completed failure of the named test, with the correct property evidence; a nonzero status alone shows nothing. Each
receipt records two facts separately: its raw outcome, unchanged, with any counterexample; and its admissibility under
the authorisation it ran under. A receipt that is not admissible is never PASS and never a kill. It is reported as it
is, with the reason, and nothing is run again without an Architect ruling."

Outer classes: PASS; FAIL (admissible, of a named property or test); INCOMPLETE (with its cause); INCOMPLETE — BUDGET
EXCEEDED (K.1); INCOMPLETE — VERIFICATION INVALID (K.1, a quotient refusal); NOT ADMISSIBLE (with its reason).

**Classification cases** (document and checker cases, not Nexus executions; the packet's checker applies these rules
to each):

| # | Case | Raw outcome kept | Outer classification | Kill evidence |
|---|---|---|---|---|
| 1 | Unchanged-core run: `outcome: PASS` and every line of Q; `test result: ok. 1 passed`; exit 0, no signal; no event of the launching command; identities match; count at most C | PASS | PASS | — |
| 2 | `outcome: PASS` printed, and then the launching command kills the process (a signal, the time limit, an out-of-memory kill), or the completion evidence is missing | PASS, as printed | INCOMPLETE: a run killed from outside (line 855); never PASS | no |
| 3 | C1/C25 witness on the mutant: the witness's own panic with the exact positive line at its locus, the header and the line of other properties; `1 failed`; exit 101 by a normal exit; the pinned mutant executable of G.3(a) | witness FAIL; named property OBSERVED | admissible witness receipt: G.3(b) holds | a part only: with G.3(a) and G.3(d) |
| 4 | A panic whose message is not the named test's own verdict (a fixture failure, `WITNESS FIXTURE FAILURE`, an `expect` in setup), or a nonzero exit with no completed failure of the named test | as printed | NOT ADMISSIBLE (unrelated failure). On an unchanged-core run it is not PASS: that run's evidence is missing (line 831); it is reported and the mission stops for a ruling | no |
| 5 | Original explorer on a mutant: `outcome: FAIL`, `FAIL: E4: …`, no witness line; `failed: {"E4"}`; exit 101, no signal; count at most C; identities match | FAIL E4 | FAIL, admissible (F.1 type (a)); no witness marker is required | yes where E4 is the control's named property (for example C10); for C25 only the primary receipt of G.3(a) |
| 6 | C22: the build of the mutant source exits nonzero with `error[E0283]` at the designated T-OWN line and no other error; the healthy build succeeds; no executable exists | build failure, E0283 at that line | admissible build-failure receipt (F.1 type (d)); executable and test-run fields NOT PRODUCED BY DESIGN | yes, for C22 only |
| 7 | Original explorer: `outcome: FAIL` with a printed count above C, found during the overshoot | FAIL, with its counterexample | INCOMPLETE — BUDGET EXCEEDED, unless a separate, explicit authorisation names that run | no |

## L. Rule R1 — the exchange loop's first expected value

**Target.** The sentence of row R1 (line 548) beginning "The loop needs no load". AD-1-R1 called it the third
sentence; it is the fourth, and it is identified here by its opening words, not by an ordinal.

**Kept unchanged** (the other sentences of R1): the word is touched in exactly two private functions, one that loads
it, used only by `view`, and one that runs a compare-and-exchange loop; both use sequentially consistent ordering, for
success and for failure; nothing else in the file names the word.

**Source fact (PROVEN FROM SOURCE, `protocol.rs` at 42dd1bd1).**

- `load` (lines 189–194) is the one load; `exchange` (from line 200) is the one loop. Both use `Ordering::SeqCst`
  (line 190; line 206 for success and failure).
- Store operations: `Publish::new` sets `guess = core.flags()` (lines 1258–1259): the phase, session, latch and cell
  bits from the core when the guard is created, with zero counts and no handover fault (`flags`, line 1068). The
  publication's step (lines 1995–2004) computes the value from the present value and from values fixed before the
  loop.
- Lock-free ticket steps start from 0 (lines 236, 353, 356, 369, 382, 395, 398). Each step computes the value from the
  present value only; `arm`'s step also sets a local flag, which every attempt overwrites.
- A failed exchange takes the present value and recomputes the step (lines 213–216).
- Appendix A's `Core` (contract lines 1429–1442) has no field holding a published value. So "the value the core last
  published" names nothing a store operation holds; the code's guess is `core.flags()`.

**Text (replaces only the sentence beginning "The loop needs no load"; the rest of R1 stays as written).** "**The loop
needs no load:** it starts from a guess and takes the present value from a failed exchange. A store operation's guess
is the phase, session, latch and cell bits that the core's own state gives when its guard is created, with both counts
zero and the handover-fault bit clear. A lock-free ticket step's guess is zero. Starting from a guess is safe only
because of how each attempt is made: the value to publish is recomputed from the value the attempt compares against,
by a step that depends on nothing but that value and inputs fixed before the loop; a failed attempt leaves behind
nothing that the next attempt does not overwrite; and the step cannot unwind (R8) and has no effect outside the attempt
(no storage verb, no other atomic, no publication but the exchange itself). A guess that differs from the present value
makes the comparison fail. The failed exchange returns the present value and the loop tries again from it. One failed
exchange then suffices only if no other thread changes the word before the retry; otherwise it fails again. No bound on
the number of failed exchanges, and no wait-freedom, is claimed (6.3)."

**Effect.** Wording only, and safety-neutral. Appendix A and the code are unchanged. The test-only hook reports each
failed exchange (line 215), which T-PB4 uses; the hook is not evidence for R8 (6.4). The second review's reading of R1 is not otherwise
affected.

## M. Historical INCOMPLETE receipts and later complete equivalent-graph results

**Text.** "A receipt keeps the verdict it was given. A later complete run of an equivalent graph never changes an
earlier receipt's verdict. Every INCOMPLETE run below stays INCOMPLETE, establishes no property, and is listed with its
verdict wherever F-3's history is given. A complete composed-quotient PASS of F-3 is a different run of a different
(equivalent) graph. It is not a continuation, completion or relabelling of any of them. The two kinds of result do
not conflict: an INCOMPLETE run asserts nothing. It may stand for F-3 in 8.3
only under B to E and N, at the exact final commit (I). No report may say that the original F-3 graph was explored or
passed. It may say only that it is DERIVED to satisfy the properties, under B(1)."

**History inventory** (from existing receipts; EXECUTOR-REPORTED; nothing rerun or relabelled):

| Configuration and explorer | Mission | Commit | Ceiling | Discovered (expanded) | Verdict |
|---|---|---|---|---|---|
| F-3, original `explore` | CAMPAIGN-01 | 244c5b23 | 30,000,000 | 30,000,002 (25,318,697) | INCOMPLETE |
| F-3, original `explore` | F3-CAP60-01 | 9e945ee7 | 60,000,000 | 60,000,001 (53,851,687) | INCOMPLETE |
| F-3, admission observer quotient alone (`quotient_full_f3`) | F3-QUOTIENT-01 | 83816e66 | 30,000,000 keys | 30,000,001 (28,284,667) | INCOMPLETE |
| F-3, composed quotient prefix (`gc_symmetry_prefix_f3`) | GC-SYMMETRY-01 | 77a146ca | 99,999 keys | 100,000 (61,565) | INCOMPLETE by construction |
| F-3, composed quotient (`gc_symmetry_f3`) | GC-SYM-F3-01 | 77a146ca | 30,000,000 keys | 20,431,851 (all) | PASS (experimental) |
| F-PT, composed quotient | PT-FB-EXPERIMENT-01 | 2d83b74b | 30,000,000 keys | 29,751,473 (all) | PASS (experimental) |
| F-B, composed quotient | PT-FB-EXPERIMENT-01 | 2d83b74b | 30,000,000 keys | 3,775,373 (all) | PASS (experimental) |
| F-1, original | CAMPAIGN-01 | 244c5b23 | 60,000,000 (one-run exception) | 46,524,336 (all) | PASS (informational) |
| F-1, original | F1-CAP60-01 | 243501f0 | 60,000,000 (one-run exception) | 46,524,336 (all) | PASS (informational) |
| F-2, original | CAMPAIGN-01 | 244c5b23 | 30,000,000 | 12,602,670 (all) | PASS (informational) |

The two quotient INCOMPLETE runs are kept for the same reason as the original ones. The prefix run was INCOMPLETE by
design and asserted to be so; it was a diagnostic of the exchange, not an attempt at F-3.

## N. Prerequisites of every quotient claim — proof and executable binding

A quotient claim is any of: 'F-3, F-PT or F-B PASS by the composed quotient'; any DERIVED original count; a quotient
PASS used as a restored receipt or as the unchanged-core side of a comparison. Each needs all of the following at the
exact final commit. If any one fails, the claim is not made.

**The rule.** Every input below is either identical to the identity the proofs were reviewed against, or has been
reviewed again for that change. "As at the final commit" is not evidence that an unreviewed change preserves a proof.
A change to a proof-relevant input inherits no approval. An addition after the audited text is such a change: it needs
review that it does not alter the meaning of the audited code (for example: no new trait implementation on a model
type, no item or import that changes name resolution, no attribute or `cfg` reaching earlier items, no global state
the explorers use, no change to the hook). The Architect's interim source review of 42dd1bd1 is that ruling's scope;
whether it also covers non-interference with the AQ and SQ proofs is for the Architect to state, and is not inferred
here.

**Source closure.**

1. `protocol.rs`, including its test-only hook (blob `a2df2841b99c` at 42dd1bd1).
2. The model in `model_tests.rs`: `initial`, `actors`, every `step_*`, `Io`, `Oracle`, `encode`, `decode`,
   `state_properties`, `transition_properties`, `admission_properties`, `terminal`, `PROPERTIES`, `BRANCHES` and the
   configuration constants (audits void otherwise: AQ1, AS4).
3. The quotient code: `expand`, `observe_admission`, `explore_experimental`, `mirror`, `explore_symmetric`,
   `explore_symmetric_kept`, `assert_symmetric`, `shape`, `SYMMETRIC` (reviewed text at 2d83b74b).
4. Everything else compiled into the same test executable that can change the meaning of 1 to 3:
   `unit_tests.rs` (blob `55dc6640` at 42dd1bd1), `ledger.rs` (`d872b908`), the crate root `lib.rs` (`b806ed57`),
   and the rest of `model_tests.rs` (blob `5ad26ddc` at 42dd1bd1).

**Build-input closure** (identities at 42dd1bd1, informational; the final commit's are what count):

5. `rust-toolchain.toml` (blob `726005b2`: channel 1.94.0) and the compiler that actually built the executable,
   recorded by `rustc --version`.
6. `Cargo.lock` (`1af53a60`); the crate's `Cargo.toml` (`2543c745`; no features declared); the workspace
   `Cargo.toml` (`2c558a44`; release profile `opt-level = 3`, `lto = "thin"`, `codegen-units = 16`,
   `strip = "symbols"`; the panic strategy is the default, unwind, on which `catch_unwind` in the explorer depends);
   `.cargo/config.toml` (`05a42b50`; `[build] jobs = 8`, `[env] CARGO_PROFILE_TEST_DEBUG = "1"`).
7. The build command and environment: `cargo test -p nexus-governed-control --locked --release --lib --no-run`; any
   `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_PROFILE_*`, `CARGO_BUILD_*`, `RUSTC_WRAPPER` or target setting; the
   enabled features and `cfg`. The effective profile is recorded from the build's own messages (at the narrow C25
   probe, EXECUTOR-REPORTED: `opt_level 3`, `debuginfo 0`, `debug_assertions false`, `overflow_checks false`,
   `test true`, features none).
8. The diagnostic tests of D pass on the pinned executable.

**Configuration.**

9. The constants, field by field:
   - F3: facts 2; conflict [true, true]; env true; batch false; faults 2; admissions 2; gating 2; gating_conflict
     false; acks 0; ack_attempts 0; malformed false; cell_constraint false; worker_unwinds false; drop false;
     replays 0; blanket_uncertain false; l1 true; cap 30,000,000; time 45 min.
   - FPT: as F3 with blanket_uncertain true.
   - FB: as F3 with batch true.
10. The printed bounds and actors equal these: F-3 and F-PT: [Fw(0), Fw(1), Se, Wk, Gc(0), Gc(1), Adm, Env]; F-B:
    [Fw(0), Se, Wk, Gc(0), Gc(1), Adm, Env].

**Execution.**

11. Exactly one test is selected, with `--exact`: `gc_symmetry_f3`, `gc_symmetry_fpt` or `gc_symmetry_fb`.
12. The run completes with 0 unexpanded keys and verdict PASS, with no runtime-guard refusal (K).
13. It runs under the launching command's 45-minute and 16 GiB limits, without swap, one exploration at a time
    (line 857).
14. The executable run is the pinned build of 5 to 7; its sha256 matches before and after.

**Review and approval.**

15. B(1) review passed against 1 to 8, and the Owner has approved this addendum.

## O. First-failure attribution, and what a trace establishes

1. **What the original explorer reports.** One property: the first violation in breadth-first order. A state's
   properties are judged when it is expanded, before its transitions; then D1 if it has no enabled step; then its
   transitions, in actor order, then oracle order. Only the first violation found is reported. That property, its
   detail and its trace are the primary record of a FAIL, and the raw report is kept with its text.
2. **What path minimality is established (PROVEN FROM SOURCE, `explore`; `explore_experimental` and
   `explore_symmetric_kept` build their traces the same way, lines 5057 and 6598).** The parent table is a
   breadth-first tree of the explored key graph. So the printed trace is a shortest breadth-first parent path, in the
   explored graph, from the initial state to the checked state, the state at which the reported violation was
   detected, followed, for a violation of a step, by that offending step. Every
   state of smaller depth was expanded before it, and none showed a violation. So every trace, in the key graph the
   explorer searches, that ends in a violation judged during expansion has at least d steps, d being the depth of that
   state, and the printed one has d or d + 1 steps.
3. **What is NOT ESTABLISHED.**
   - That the printed trace is the shortest trace exhibiting the reported property, or any violation: a state
     violation at depth d expanded later in the same layer gives d steps where a step violation reported first gives
     d + 1.
   - Anything about other violations at the same depth: only the first found is reported.
   - For an L1 FAIL: the trace is a shortest path to one state "on or behind a cycle", found after the search. It is
     not a cycle, and not a shortest witness of one.
   - Global counterexample minimality, from the breadth-first parent table alone or from quotient lifting. Q11's
     statement that lifted lengths equal original shortest lengths is an executor proof, not accepted, and holds only
     under B(1).
   - The minimality of the C1 and C25 witness schedules.
4. **Line 609's phrase "so that a violation has a shortest trace" is REPLACED (S-19).** The one operative text, for
   every run, original or quotient, healthy or mutant: "so that the trace printed for a reported violation is a
   shortest breadth-first parent path, in the key graph that run's explorer searched, from the initial state to the
   checked state, with the offending transition appended where the violation is of a transition." The claims NOT
   ESTABLISHED in 3 stay not established: no global counterexample minimality, and no shortest cycle witness. The
   explorer's printed wording (`shortest trace (<n> steps):`, and its unfolded form) is raw output: it is kept unedited
   in every receipt and read under this text. The replay requirement of D (unfold, then replay in the original
   semantics before a trace is cited) is unchanged. No gate relies on minimality: G and F rely on property names and
   verdicts.
5. **Quotient attribution.** The composed quotient's first-reported property may differ from the original's for the
   same defect (ASYM-4, Q12, S8). A quotient FAIL names *a* violated property, never "the first", and is never a
   control's failing receipt (F).
6. **Controls.** A control's attribution comes only from the original explorer on the mutant (F), and for C1 and C25
   also from the witness (G). E3 is never counted as P1, and E4 never as P7. A witness lists every violation by step
   and by kind. Its order is a listing order, not a priority. It does not claim to be the explorer's first failure,
   and claims nothing about any configuration beyond its own schedule.

## P. Test-only boundaries

1. All existing executable source is preserved. The witnesses live only in `model_tests.rs`, a `#[cfg(test)]` module,
   added after every existing line (42dd1bd1). No existing line of `model_tests.rs` is changed or removed.
2. `protocol.rs` production text is never changed by a witness or by a receipt change. The one test-only item in it,
   the hook, is used as it is.
3. None of the following is changed by this addendum, by the witnesses, or by its review packet:
   - `explore`, `successors`, `step_adm` or any `step_*`;
   - `explore_experimental` or `explore_symmetric_kept`;
   - any property function;
   - any F-1, F-2, F-3, F-PT, F-B (or other) constant, cap, actor or fault budget;
   - any guard of `P3`, any workflow, any `Cargo` or toolchain file;
   - any manifest, historical or current-tree.
4. The witness-only source allowance (1 and 2) authorises nothing else. In particular it does not authorise the
   current-tree manifest (H), any later printing change (D adds none), or carrying the witnesses to the final commit.
5. The review packet that carries this draft adds files only under
   `docs/evidence/p3-candidate11/ad1r3-review-01/`, in one documentation-only commit whose parent is the earlier
   evidence-transport commit 8693e3a7. Neither commit is an engineering baseline. The earlier packet,
   `docs/evidence/p3-candidate11/ad1r2-review-packet-01/`, is not changed.
6. Mutation patches are applied only in disposable worktrees, by a script kept outside the repository (line 750
   unchanged), and are never committed applied. Documentary patch copies are inert evidence.

## Q. Property lines by configuration

A PASS receipt prints one line for each of the 25 entries of `PROPERTIES` (`model_tests.rs` line 3694): P1, P2, P4,
P5, P6, P7, P8, P9, P12, P13, P15; D1, L1; E1 to E11; KEY. The same code prints them for original and quotient runs.
A FAIL receipt prints only its failing property and trace. An INCOMPLETE receipt prints every line as "NOT
ESTABLISHED (incomplete)".

**Text.** "A configuration PASSes when every applicable line HOLDS. A line that is not applicable, or not claimed, is
printed as such. It is never counted as HOLDS, and never as a failure. The individual lines are authoritative: a
summary count is a check on them, never a substitute."

Expected successful formats (PROVEN FROM SOURCE: `worker_unwinds` is true only for F-W and `l1` is false only for
F-D). These are formats, not claims that any run has completed:

| Configuration | HOLDS | Not applicable | Not claimed |
|---|---|---|---|
| X-1 to X-5 | 24 | E10: "not applicable here (worker verbs do not unwind)" | — |
| F-1, F-2, F-4, F-5, F-6, F-7, F-8, F-B2, F-G, F-R | 24 | E10 | — |
| F-3, F-PT, F-B (original or admitted quotient) | 24 | E10 | — |
| F-W | 25, when every applicable check succeeds | — | — |
| F-D | 23 | E10 | L1: "not claimed for this configuration" |

L1's line, where it holds, reads "HOLDS: <n> non-fault edges, no cycle; <m> end states, each terminal (D1)". It counts
as HOLDS. E10 is established only by F-W's PASS.

## R. Review roles under the Owner's current arrangement

**Basis (ARCHITECT-RULED, AD1R3-READINESS-01).** The Owner's current arrangement: ChatGPT is the Sole Architect and the
independent reviewer of Claude Code's work; Claude Code is the implementation executor. Neither Claude Chat nor Codex
has a role in it, and this draft requires neither.

**The binding reviews' role statements** (verbatim; neither review is edited):

<!-- VERBATIM REVIEW-02 238 -->
```text
{{R2:238}}
```

<!-- VERBATIM REVIEW-03 81,83 -->
```text
{{R3:81}}
{{R3:83}}
```

**Mapping** (proposed; no original is edited; the contract's own lines are quoted under S-20 and S-21):

| Where | Old operational role | Read under the current arrangement |
|---|---|---|
| contract line 4, and line 5 of each review: "Claude — Lead Architect" | wrote the contract; coordinates the work around it | Authorship stays historical. The coordinating functions belong to the Sole Architect. |
| contract lines 5, 8, 31, 972 and 982 ("First Architect"), and line 4 of each review ("ChatGPT — First Architect / Independent Auditor") | audits and accepts designs and contracts | The Sole Architect. The acceptances recorded there stay as dated. |
| contract line 923, "Codex's review of the evidence passes" | independent review of the implementation evidence | The Sole Architect's independent review of Claude Code's evidence passes. |
| contract line 928, "only after the evidence and Codex's review have actually passed" | the same | The same. The Owner's decision on the label is unchanged. |
| contract line 976, "Codex, under that arrangement" | reviewer of the implementation evidence | The Sole Architect. The line's second sentence is unchanged. |
| REVIEW-02 line 238 | Claude: Lead Architect; ChatGPT: audits designs and contracts; Codex: reviews implementation evidence | ChatGPT: the Sole Architect, auditing designs and contracts and independently reviewing Claude Code's implementation evidence. Claude Code: the implementation executor. Its last sentence is unchanged. |
| REVIEW-03 line 81 | Claude assembles the authorisation request and coordinates authorised work | The Sole Architect. |
| REVIEW-03 line 83, "Implementation review remains independent." | — | Unchanged: Claude Code's implementation is reviewed by the Sole Architect, independently of the executor. |

**Kept.** The obligation of independent review; the Owner's authority over adoption, scope, authorisation and the
label (line 928); the rule that a reviewer does not accept a repair it wrote (line 976; REVIEW-02 line 238); and that
Claude Code never approves its own work. No review is waived, and none is moved to the executor.

**Not resolved here.** If the Sole Architect is the author of a repair, those rules apply to it unchanged. How
independent review of such a repair is then obtained is the Owner's decision (open item 11).

**Line 929.** The mission that produced this draft names contract lines 923 and 929. In the contract file of sha256
`cac65351…3b3c`, line 929 is empty; the adjacent sentence that names Codex's review is line 928. This draft maps lines
923, 928 and 976, and infers nothing else (open item 11).

---

## Open items (none decided by the executor)

1. The Owner's approval of A, and of this addendum as a whole.
2. Independent review of every resolution in this draft, A to R, and of the supersession table.
3. Independent review of the AQ and SQ proofs, with E-PT and E-B, against the final source and build-input closure (B,
   N).
4. Closed as to the witness-audit evidence by the Architect's ruling of AD1R3-READINESS-01 (0A.1, G.1). That ruling
   establishes no final mutation kill.
5. Authorisation of the `full_f1` mutant runs for C1 and C25, which need A or an explicit Owner authorisation, and of
   carrying the witnesses to the final commit.
6. Acceptance of E's inventory dispositions and of the S-9 replacement of line 863's listing sentence; and whether the
   experimental tests are kept at the final commit.
7. The receipt-linking proposal (I).
8. The replacement of line 609's trace phrase (S-19, O.4).
9. The current-tree manifest's mission (H).
10. C9, C10, C24 and C4 were analysed read-only in AD1R3-READINESS-01's attribution preflight (in its review packet).
    Its findings are source-supported predictions, not measurements. Whether C4's "P4, P8" is a list or a
    conjunction, and so whether the original explorer, which reports one property, can show what the row requires,
    needs the Architect's decision. G applies to C1 and C25 only; nothing here extends it to any other control.
11. The review-role mapping (R, S-20, S-21), with the line-929 reference, and how independent review is obtained for a
    repair that the Sole Architect authored.
12. The typed receipt requirements (F.1) and the process-aware classification (K.2), including INCOMPLETE — BUDGET
    EXCEEDED and the separate authorisation that it names.

## Changes from AD-1-R2

| Section | AD-1-R2 | AD-1-R3 |
|---|---|---|
| status, 0, 0A | the witness-audit ruling; probes awaiting the Architect's authentication | the AD1R3 ruling added and governing (0A.1); this mission's four repairs (0A.2); AD-1-R2's dispositions carried (0A.3) |
| 1 | S-6 applied G.3(c)'s rejections to every failing receipt; S-2 CLARIFIED the trace phrase | S-6: each receipt meets its own type (F.1) and K.2, and G.3(c) applies only to the witness invocation; S-2 narrowed; S-19 replaces the trace phrase; S-20 and S-21 map the review roles; S-1 and S-14 supplemented for K.1 and K.2 |
| F | killed by the explorer's FAIL of the named property or the named test's failure; no receipt types | killed only by an admissible receipt of its own type; F.1 types (a) to (f); C22's executable and test-run fields NOT PRODUCED BY DESIGN |
| G | (a): "the report text, not the exit status, decides"; (c) unscoped; statuses awaiting authentication | (a): a type (a) receipt under K.2, with no witness line required; (c): only the witness invocations; statuses per the AD1R3 ruling; the probe helpers historical |
| K | "classification is by text, never by exit status"; a FAIL in the crossing expansion is a FAIL of the explored graph | K.1: the raw outcome is kept, and a FAIL beyond C is INCOMPLETE — BUDGET EXCEEDED; K.2: process-aware and fail-closed, the seven points that must agree, the outer classes and the case table |
| O | O.4: CLARIFIED, with an alternative replacement proposed | O.4: one operative replacement (S-19) |
| P | packet prefix `ad1r2-review-packet-01` | packet prefix `ad1r3-review-01`; the earlier packet unchanged |
| R | — | new: review roles under the Owner's current arrangement |
| open items | ten | twelve: 4, 8 and 10 updated; 11 and 12 added |

## Changes from AD-1-R1 (made by AD-1-R2; history)

Where this table and the one above differ, the one above describes this draft.

| Section | AD-1-R1 | AD-1-R2 |
|---|---|---|
| status | one ruling set (A–J of the readiness mission) | adds the Architect's witness-audit ruling (0A.1) and this mission's dispositions A–Q (0A.2); provenance labels defined |
| 0 | witness commit "local, not pushed" | published on the review ref; interim source review passed; probes EXECUTOR-REPORTED |
| 1 | S-1..S-15; several actual replacements labelled SUPPLEMENTED | effects relabelled: replacements are REPLACED with their scope; S-16 (A4), S-17 (C7 without A4), S-18 (label) added; lines 742 and 748 moved out of "not superseded"; line 865 added there |
| A | "every F-1 run needs the Owner's one-run exception, as before" | no one-run authorisation inferred; a cap in test source is not an approval |
| B | review "against the exact final source" | against the source and build-input closure (N); Kf is printed, so state derivation is exact under B(1); the open item on fixed shares closed by D |
| C | ceiling counts K | also: derived original counts do not consume it; "248,528 or more" |
| D | per-member NOT ESTABLISHED unless printed; counts labelled | MEASURED mandatory and exact; DERIVED original states only under N; BOUND optional, never in place of a required value; no fixed-share or per-member instrumentation; the 51 diagnostic tests kept as prerequisites |
| E | three configurations | adds E.2 (8.3 and 8.4 reconciled) and E.3/E.4 (all 34 ignored tests, with proposed dispositions) |
| F | restored receipt for F-PT, F-B | adds the named non-explorer tests and C22; quotient diagnostics are not kill receipts; A4 as the audited F-PT comparison; C7 without A4 resolved |
| G | option A; C25's text open; "ARGUED, not probed" | narrow C25 definition selected; status with provenance; positive-line rule and rejections; exit status alone insufficient |
| H | `final_tree` field in the manifest | no self-reference; detached attestation; historical review fields marked historical; current review starts pending; Architect coordinates all 22 |
| I | builds per run | one baseline build; one build per mutant identity serving both invocations; proposed receipt linking |
| K | "reaching" | inclusive cardinality; overshoot is detection, not budget; FAIL during the crossing expansion; precondition refusal is INCOMPLETE — VERIFICATION INVALID; classification by text |
| L | "replaces only the third sentence"; "a wrong guess costs one failed exchange" | targets the sentence by its opening words; retry-safety conditions; no one-failure bound, no wait-freedom |
| M | two INCOMPLETE runs | four INCOMPLETE runs, including the admission-only quotient and the prefix; complete runs listed |
| N | "as at the final commit"; later lines "do not affect this" | source and build-input closure; no inherited approval; additions need review |
| O | attribution | what path minimality is and is not established; line 609 disposition |
| P | later printing changes allowed in `model_tests.rs`; "any manifest" | the witness allowance authorises nothing else; this packet's prefix only |
| Q | 24 HOLD and E10 | counts for every configuration; FAIL and INCOMPLETE formats; lines authoritative |
