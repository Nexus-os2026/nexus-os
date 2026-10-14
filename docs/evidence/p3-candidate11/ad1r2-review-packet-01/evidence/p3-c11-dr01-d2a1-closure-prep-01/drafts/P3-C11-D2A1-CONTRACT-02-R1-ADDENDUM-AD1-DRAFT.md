# P3-C11-D2A1-CONTRACT-02-R1 — ADDENDUM AD-1 — DRAFT

**STATUS: DRAFT. NOT ACCEPTED. NOT APPLIED. NOT IN FORCE.**
Prepared by the executor (Claude Code, Opus 5.5) under P3-C11-DR01-D2A1-CLOSURE-PREP-01 for the Sole Architect's
independent review. The Owner retains authority over every contract amendment. Until the Architect and the Owner
accept a text, contract P3-C11-D2A1-CONTRACT-02-R1 (sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`)
applies unchanged, with its binding reviews (`6dfb1456…cdce`, `1f964d75…fb35`).

Where this draft offers options, the executor's recommendation is marked; the choice is the Architect's.
Nothing here lowers a fault budget, drops an actor, changes a configuration's bounds, or counts an INCOMPLETE
run as PASS (contract 8.3 l.859, unchanged).

---

## 0. Basis

| Item | Identity | Status |
|---|---|---|
| Production protocol core | `protocol.rs` blob `a2df2841` (sha256 `a5454944…a296`), identical at 244c5b23, 83816e66, 77a146ca, 2d83b74b | unchanged since SOURCE-R2 |
| Contract explorer | `explore` (model_tests.rs l.3882) and everything it calls, byte-identical in that range of lines at all four commits | unchanged |
| Admission observer quotient (AQ) | ADM-QUOTIENT-01: Theorem 1, L-A, L-B, Q1–Q15, AQ1–AQ6 | executor PROVEN; not accepted |
| Gating-caller symmetry quotient (SQ) | GC-SYMMETRY-01: Lemma E, S1–S15, ASYM-1..6, AS1–AS4 | executor PROVEN for F-3; passed an independent source review for that bounded experiment only |
| SQ for F-PT and F-B | CLOSURE-PREP-01 Stage 2 proof; commit 2d83b74b (allow-list + fixtures) | executor PROVEN; review pending |
| F-3 composed (AQ ∘ SQ) result | GC-SYM-F3-01 at 77a146ca: 20,431,851 keys, 0 unexpanded, 880,778 self-mirrored, every applicable property, D1, L1 HOLD | OBSERVED, experimental |

---

## 1. Supersession table — the exact statements this draft would change

Line numbers are of `P3-C11-D2A1-CONTRACT-02-R1.md`. Each statement **stays in force** except as stated.

| # | Contract text (verbatim) | Line | Changed by | How |
|---|---|---|---|---|
| S-1 | "**Caps for each run:** 30,000,000 states, counted by the explorer; 45 minutes and 16 GiB, enforced by the command that starts the run." | 855 | A, C | F-1's state ceiling becomes 60,000,000 (A). For a quotient run admitted by E, "states counted by the explorer" means the quotient's keys (C). Time and memory unchanged. |
| S-2 | "`model_tests.rs` explores the real core. … It visits every interleaving of whole steps, breadth first, with a visited set and a parent table, so that a violation has a shortest trace." | 609 | B, D, E | For F-3, F-PT and F-B only, a complete composed-quotient run may stand for the full exploration; the quotient visits one representative per equivalence class, and its traces are unfolded (D). |
| S-3 | "Two states with equal keys are treated as one. Anything left out of the key would be a defect in the evidence; the receipts list the key's fields." | 620 | B | Supplemented, for E's configurations only: two keys in one AQ/SQ class are treated as one. The key itself and its fields are unchanged. |
| S-4 | "**Run explicitly, optimised, on the exact final commit of the mission, every one, before the mission reports.**" | 831 | I | Kept and made explicit about reuse (I-1, recommended), or relaxed under conditions (I-2). |
| S-5 | "Each control of 7.5 is run on its configuration, with a failing receipt, and a restored receipt showing the unchanged core passes that configuration." | 853 | F | The failing receipt stays on the original explorer. For F-PT and F-B the restored receipt may be the accepted composed-quotient PASS (F, option F-1). |
| S-6 | "Each control is one named change, run **on the configuration named**. It must make its property fail by assertion." | 693 | G | For C1 and C25 only, "its property fails" is shown by the preserved first failure plus a named-property replay witness. |
| S-7 | "\| C1 \| the count becomes a flag \| P1 \| F-1 \|" | 699 | G | Cell "P1" read as "P1 (first failure E3; witness)". |
| S-8 | "\| C25 \| a gating append answers Committed on `Commit::Unknown` \| P7 \| F-1 \|" | 721 | G | Cell "P7" read as "P7 (first failure E4; witness)". |
| S-9 | "For every run, default or full: the commit; … states and transitions; each property's verdict; for each branch of each operation, whether any run took it; …" | 863 | C, D | A quotient receipt reports its own counts, labelled, plus the DERIVED original counts with their formulas. |
| S-10 | "\| Manifest control M87 \| … TEXT UPDATE, SAME PROPERTY. \|" and "\| Manifest controls M32, M78, M79, M80, M89, M112, M113, M141, M143, M153, M177, M178, M179, M180 \| … TEXT UPDATE, SAME PROPERTY, **if** …" | 911, 912 | H | The "text update" is done by a separate identity supplement, never by editing the manifest; seven older stale entries are added to the record. |

Not superseded (stated so that no gate is weakened silently): 4.1 A-1/A-2 (l.198–199) and 6.3 (l.584); every
property of 7.3/7.4 and the claims table; the list of controls, each one's configuration and property (except
the reading in G); 8.1 "INCOMPLETE is never PASS" (l.813); 8.2; the bounds of every 8.3 row; "One full exploration
runs at a time" and the memory pre-check (l.857); "These bounds are reviewed bounds…" (l.859); section 9; the pins of
section 10 other than S-10; the label conditions of section 11; every stop condition of section 12, including
"A run of 8.2 or 8.3 is INCOMPLETE" (l.942).

---

## A. F-1's ceiling

**Proposed text.** "F-1 alone runs with a state ceiling of 60,000,000 states, counted by the original explorer.
Its time (45 minutes) and memory (16 GiB) ceilings, and every other cap, are unchanged. This is an investigation
ceiling, like the others; it is not a measured or accepted cost. F-D, though defined from F-1, keeps 30,000,000."

**Evidence.** F-1 PASS, 46,524,336 states, complete, 243 s, 5.41 GiB peak, at 244c5b23 (campaign-01) and at
243501f0 (F1-CAP60-01), under the Owner's one-run exception. The source already carries the override
(`full_f1`, model_tests.rs l.4169–4173) since 243501f0.

**Condition.** The Owner's acceptance of the ceiling as standing, not one-run, text. F-1 needs no quotient.

## B. F-3 by the composed quotient, and its equivalence basis

**Proposed text.** "F-3's full exploration may be established by one complete run of the composed quotient
(`explore_symmetric_kept(&F3, step_adm, Mode::Quotient, _)`, the admission observer quotient composed with the
gating-caller symmetry quotient), on the exact final commit, provided:
(i) Theorem 1 with Q1–Q15 and Lemma E with S1–S15 have passed independent review against the exact final source;
(ii) the run's runtime guards — the admission observer's re-encoding check at every expanded key, and the
symmetry guard (involution, expansion of each key's mirror held to the exchange, D1 judged alike) — raise no
refusal; any refusal ends the run with no verdict;
(iii) the run completes (no unexpanded key) and every applicable property, D1 and L1 hold.
The receipt states that the original graph was not explored."

**Exact equivalence basis, to be printed by the receipt.** With K the quotient's keys, Kf those that are their own
mirror, N = 2 admissions:
- admission-quotient states |R_Q| = 2K − Kf  (exact, DERIVED under S1–S15);
- original states |R_G| = (N+1)·|R_Q|  (exact, DERIVED under Theorem 1).
F-3 at 77a146ca: |R_Q| = 2 × 20,431,851 − 880,778 = 39,982,924; |R_G| = 3 × 39,982,924 = 119,948,772 (DERIVED).
Transition, fault-edge, no-step and admitted counts are exact only if the run prints the self-mirrored shares
(`Mirrors.fixed_*`); `gc_symmetry_f3` at 77a146ca (and `gc_symmetry_fpt`/`gc_symmetry_fb` at 2d83b74b) discard them,
so only bounds are available today. **Proposed requirement:** the final-commit run prints them, so that every
original count it cites is exact and DERIVED, never estimated. That needs a test change in a later, authorised
mission.

## C. What the caps mean for an equivalent-graph exploration

**Option C-1 (recommended).** "For a quotient run admitted by E, the state ceiling of 8.3 counts the quotient's
keys, as the explorer counts them; the receipt also reports the DERIVED original count. The time and memory
ceilings bound the real process, unchanged. Reaching any ceiling is INCOMPLETE, and an INCOMPLETE quotient run
supports no derived statement."
**Option C-2.** The 30,000,000 ceiling bounds the DERIVED original count. F-3 (119,948,772 derived) could then not
pass under any quotient; F-3, F-PT and F-B would stay without a verdict.
Either way the cap check's off-by-one (`visited.len() > cap` stops at cap + 1 or + 2; model_tests.rs at 2d83b74b
l.3907 `explore`, l.5068 `explore_experimental`, l.6609 `explore_symmetric_kept` — l.6607 at 77a146ca) is stated in
the receipt.

## D. D1, L1, safety, trace and coverage obligations under a quotient

**Proposed text.**
- **Safety.** A quotient PASS establishes the 7.4 safety claims for the original graph only through B(i); the
  receipt says so. The set of reachable violated property names is the original's (S8, Q12); the first-reported
  name may differ (ASYM-4, Q12), so a quotient FAIL names *a* violated property, not "the first".
- **D1.** Judged at every key with no enabled non-ADM step, with the admissions spent, at the key and at its mirror.
- **L1.** Kahn's algorithm over the quotient's non-fault edges; transfer to the original by S10 (both directions)
  and Q10. ADM steps are observations, never edges (a naive admission self-loop is not a cycle).
- **Traces.** Any FAIL trace is unfolded (S11) and replayed in the original semantics, with the callers' own names
  and the N admission steps appended for D1 (Q11), before it is cited.
- **Coverage.** The receipt lists, for each branch, whether the quotient took it; that set equals the original's
  (every original transition maps to a quotient transition with the same labels: S7, Corollary 1). Branches no run
  takes are listed as NOT TAKEN, as now.
- **E10** keeps its "not applicable" rule (worker verbs do not unwind).

## E. Which configurations may use a quotient

**Proposed text.** "Only these may use a quotient for 8.3, each only after the review of B(i) and of its own
configuration-specific audit against the exact final source:
- F-3: admission observer ∘ gating-caller symmetry (ADM-QUOTIENT-01; GC-SYMMETRY-01).
- F-PT: the same (CLOSURE-PREP-01 Stage 2, items 1–17).
- F-B: the same (CLOSURE-PREP-01 Stage 2, items 1–17).
F-1 and F-2 complete under the original explorer and use it. Every other configuration uses the original explorer
unless a later amendment admits it after its own audit. The gating-caller symmetry is never available to F-5,
F-6, F-7, F-8, F-B2 (callers or acknowledgements, ASYM-1/2) or F-G (ASYM-3)."
**Unchanged.** The source's allow-list (`SYMMETRIC`) and `assert_symmetric` enforce E mechanically: an unlisted
shape, a raised cap or another actor change is refused before exploring.

## F. Mutation controls stay on the original explorer

**Proposed text.** "Every 7.5 control, C6 and C7 (F-PT) and C8 (F-B) included, runs on the original explorer under
its configuration's original cap. A control is killed only by the original explorer's FAIL of its property (G for
C1 and C25). A control run that reaches a cap without a FAIL is not killed; 7.5's stop rule applies."
**The restored receipt for F-PT and F-B (S-5):**
- **Option F-1 (recommended).** The restored receipt is the accepted composed-quotient PASS of the unchanged core
  for that configuration on the exact final commit (B–E).
- **Option F-2.** The restored receipt must be an original-explorer PASS. Not shown achievable: the original F-3
  did not complete at 60,000,000 states, and F-PT/F-B are defined from it (their sizes are NOT ESTABLISHED). C6,
  C7 and C8 would then remain without a complete receipt.
**Reasons the controls cannot move** (ADM-QUOTIENT-01 Q15; GC-SYMMETRY-01 S15; CLOSURE-PREP-01 item 17): a control
is a model change, which voids the source audits (AS4, AQ1); the guard would end an asymmetric mutant's run
"symmetry inapplicable", which kills nothing; and the first-reported property may differ.

## G. C1 and C25 — Option A

**Proposed text.** "For C1 and C25 only, a kill is recorded when all of:
(a) **the first-failure receipt is preserved as primary**: the unchanged original explorer, on F-1, on the mutant,
FAILs; its first-reported property and shortest trace are kept as printed (predicted: E3 in 4 steps for C1, E4 in
1 step for C25; if it reports otherwise, that is reported, not reinterpreted);
(b) **an independent named-property replay witness on the same mutant build**: a fixed, deterministic schedule,
replayed in the original semantics, that reports every violation of every step (not only the first) and asserts P1
at its admission (C1) or P7 at its final state (C25);
(c) the same witness on the unmutated build of the exact final commit violates nothing;
(d) the rows are recorded as 'P1 (first failure E3; witness)' and 'P7 (first failure E4; witness)'.
E3 is not counted as P1 and E4 is not counted as P7."
**Where the witnesses live (proposed).** Two named, ignored tests in `model_tests.rs` at the final commit, so that
(c) runs them on the unmutated build and (b) selects them by name on the mutant build; adding them needs its own
authorised implementation mission. The C1 witness in ADM-QUOTIENT-01 (11 steps) was constructed by hand and never
replayed; its minimality is not claimed.

## H. The 22 stale mutation-manifest source hashes

**Proposed text.** "`docs/evidence/p3-candidate10/original-183/manifest.json` (sha256 `2c4adc84…2e14`) is preserved
byte for byte; no edit, no regeneration. A separate identity supplement, a new file with its own hash, records for
each of the 22 stale (control, file) pairs: the recorded hash, the file's blob and sha256 at the exact final
commit, the commit at which it first went stale, and a check that the control's anchor still matches exactly once.
No mutation result that depends on a stale entry is relied on until the supplement is reviewed (ruling B/Q-5,
unchanged)."
**The 22, as recorded by campaign-01 at 244c5b23** (unchanged through 2d83b74b, which touches none of these files):
- stale since D2A-1 (22ebf9a5), expected by contract 10: `phase3_tests.rs` M32 M78 M79 M80 M89 M112 M113 M141 M143
  M153 M177 M178 M179 M180; `gc/lib.rs` M87 (15);
- stale before D2A-1, not covered by contract 10: `broker.rs` M18 M19 M39 (at 862a11ce); `authority/approval.rs` M84,
  `authority/mod.rs` M05, `authority/run.rs` M45 M129 (at Candidate 11, b1d535f1) (7). These are recorded, not
  D2A-1's to resolve; their disposition belongs to Candidate 11's validation.

## I. Exact final-commit provenance, and reuse of earlier evidence

**Option I-1 (recommended, fail closed).** "Every 8.3 run, every 7.5 control (failing and restored receipts) and
every C1/C25 witness is made on the exact final commit, from a fresh optimised build whose test executable's sha256
is pinned and checked before and after each run. Earlier receipts — F-1 and F-2 PASS at 244c5b23, F-3 composed PASS
at 77a146ca — are informational and are not 8.3 evidence." Cost, as measured: F-1 about 4 min, F-2 about 1 min,
F-3 composed about 6 min.
**Option I-2 (not recommended).** Reuse when the protocol and test sources a run executes, Cargo.lock,
rust-toolchain.toml and the toolchain are identical, labelled "source-equivalent, not exact-commit". Reproducible
builds are NOT established (two builds of 77a146ca differ in sha256 at equal size), so binary identity cannot carry
the reuse.

## J. Storage assumptions

No change. A-1 and A-2 (4.1) stay stated and undischarged until the real storage/shell contract (6.3; 13). Every
result above — original or quotient, PASS or FAIL — is conditional on them and on AQ6: it is the explored model's
state-safety, D1 and L1, not a real execution, durability, or contract 6.2/6.3. Section 11's scope sentence is
unchanged.

---

## Open decisions for the Architect (none taken by the executor)

1. A: the 60M F-1 ceiling as standing text (Owner acceptance).
2. B/C: whether a composed-quotient PASS can stand for F-3, and C-1 or C-2.
3. E: whether F-PT and F-B join F-3, after review of the Stage 2 proof and of 2d83b74b.
4. F: F-1 or F-2 for the restored receipts of C6, C7, C8.
5. G: Option A for C1/C25, and where the witnesses live.
6. H: the identity supplement's form and owner.
7. I: I-1 or I-2.
8. The cap check's off-by-one and the R1 CAS wording (earlier open items) stay open; this draft does not settle them.
