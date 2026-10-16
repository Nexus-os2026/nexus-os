# The 20 configurations

**Are all configuration obligations discharged by actual adopted contracts? Yes.** 19 by admissible exact-final PASS
receipts and F-R by AD-1-R5's proof discharge, with its raw INCOMPLETE receipt kept.

| Group | Configurations | Evidence | Contractual discharge |
|---|---|---|---|
| Default suite (8.2) | X-1 59,104 · X-2 10,020 · X-3 25,617 · X-4 3,106 · X-5 3,311 states | optimised e1 receipts (VALIDATION-02 Stage C), the Stage-1 suite, and Stage B unoptimised (`... ok`) | MEASURED PASS |
| Original explorer (8.3) | F-1 (cap 60,000,000), F-2, F-4, F-5, F-6, F-7, F-8, F-B2, F-G, F-W, F-D | exact-final original-explorer PASS | REQUIRED_FINAL_RUN (AD-1-R4 E.3) |
| Composed quotient (8.3) | F-3, F-PT, F-B | exact-final composed-quotient PASS; **the original graph was not explored**; original states DERIVED only | REPLACED_BY (AD-1-R4 B–E, N; E.1) |
| Incomplete | F-R: **INCOMPLETE**, 30,000,001 discovered, 26,539,923 expanded | raw `78e44c33…8ccd` | **F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF** (AD-1-R5); not an explorer PASS |

Frozen recomputation `tools/s2_configs.py`: 372 of 372 checks — bounds equal the contract's, E-C equal before and
after, exact selection, no OOM/swap/timeout, every PASS complete within its cap. Every figure in
`02-SECTION84-CLOSURE.md` was re-verified against the derivation (0 mismatches). No omitted measurement became PASS.

Details: `02-CONFIGURATIONS.tsv`, `02-SECTION84-CLOSURE.md`, `02-RECEIPT-PROVENANCE.md`, `02-ASSURANCE-LIMITATIONS.md`.
