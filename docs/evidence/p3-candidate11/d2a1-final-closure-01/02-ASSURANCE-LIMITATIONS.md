# Stage 2 — assurance limitations (retained after any acceptance)

1. **A-1 and A-2 are undischarged** (contract 4.1, lines 198–199; AD-1-R4 J). Every result — explorer or witness, PASS
   or FAIL, original or quotient — is the explored model's state-safety, D1 and L1 under those storage assumptions.
   Nothing here establishes real SQLite durability, a real execution, or contract 6.2/6.3.
2. **The model is not production.** The D2A-1 core is dormant and production-unactivated (contract 3.2: no other
   module can name it). A passing D2A-1 evidence gate cannot authorise real-world actuation.
3. **Finite bounds.** Each configuration is a reviewed bound (8.3). Faults, actors, admissions and acknowledgements
   beyond the bounds are not explored. L1 is a property of the explored graph, not of a real execution (6.3).
4. **Quotients.** F-3, F-PT and F-B explored the composed quotient only. Their safety, D1, L1 and branch claims hold
   for the original graph **only through B(1)** (the AQ and SQ equivalences, accepted as relayed in
   FINAL-EVIDENCE-STAGE1) and **AQ6** as a binding model-scope limitation. Their original state counts (119,948,772;
   173,414,754; 22,182,849) are DERIVED, not measured; original transitions are not stated. Per-member statements for
   F-B's batch are not established.
5. **F-R.** The raw receipt is INCOMPLETE (30,000,001 discovered, 26,539,923 expanded at the 30,000,000 cap). F-R's
   safety, D1 and L1 are DERIVED through the accepted stuttering-refinement proof from F-2's PASS (AD-1-R5 S5-6); they
   were not measured on F-R's graph. No F-R total state or transition count exists or is stated.
6. **Branches.** 105 of the 193 vocabulary rows were taken by no run and are disclosed UNOBSERVED; they are not covered.
   3 rows were seen only in F-R's incomplete prefix (existence on a reachable transition, not coverage).
7. **Traces.** A trace is a shortest breadth-first parent path to the checked state (S-19, O.4); no global
   counterexample minimality and no shortest cycle witness is claimed.
8. **Untested orders.** "Count first, then the ticket's state" has no control and can have none (contract 7.5, line
   746); properties P2, P5, P9, D1, E6, E10 and the handover-fault half of E9 have no mutation control (line 748).
9. **Re-offered gating records** are covered by T-GATE's table, not by exploration (contract 16, item 10).
10. **Reproducible builds are not established:** two builds of one source have differed in sha256 (AD-1-R4 I); every
    identity is the sha256 of the executable actually run.
11. **Decision records are executor-written.** The Architect's and the Owner's decisions cited here are recorded by
    the executor from mission text; none is signed. C4's and C7's acceptances, and FINAL-EVIDENCE-STAGE2's
    configuration acceptance, have no verbatim Git record.
12. **Validation deviations retained** (not cured by this mission): VALIDATION-02's Stage-0 heuristic FAIL line and its
    self-adjudication; its supervisor v1 failure and mid-mission R1 revision; `tools/final_state.py` added after its
    freeze; receipt 51's misstated sample counts; the Stage B memory.peak 16,384 B above memory.max; the generated
    `app/src-tauri/permissions/`. This mission's own deviations: self-test v1 FAILED (predicate) and the first Clippy
    launch NOT_STARTED (gate defect), both preserved (`04-RESOURCE-RECEIPTS.md`).
