# The 35 D2A-1 mutation decisions

**Are the 35 mutation decisions complete and correctly linked? Yes — 35 of 35 Architect accepted, each linked to
admissible accepted evidence; 33 by by-ID Git records and C4 and C7 by conversational rulings, identified as such.**

- Frozen extractor `tools/s1_mutations.py`: **647 of 647 checks hold** (`01-CAMPAIGN-CHECKS.txt`).
- Exactly 35 distinct ids (the K1 catalog's, A4 excluded); none missing, none duplicated.
- Every mutant source was re-derived independently (Candidate C + the frozen catalog patch, in a scratch copy) and
  equals the recorded and catalog mutant blob.
- Every pinned mutant executable still on disk hashes to its pin (hashed, never run); C22's is NOT PRODUCED BY DESIGN.
- Every failing receipt's sha256 recomputed from Git; every restoration EXACTLY_C (C7: EXACTLY_W); every restored-side
  link verified.
- Attribution rules: C1 `P1 (first failure E3; witness)`, C25 `P7 (first failure E4; witness)`, C4
  `P4, P8 (first failure P4; witness)`, C7 `P6 (first failure E11; witness)`. Primary first failures are recorded as
  printed and are never counted as the named property where the rule forbids it. C7's original F-PT explorer FAILed
  E11; **no original F-PT P6 failure is claimed**. Its mutant witness: E11 at the step-13 transition, P6 after step 24;
  healthy witness PASS; restoration EXACTLY_W; baseline link Candidate C only, under CR-B.
- Three accepted runs (C28, C5, C24) used preserved, Architect-admitted earlier builds; four earlier BLOCKED records
  (C5, C24, C28, and the historical E-C-based C7) are kept as history and never relabelled.
- No mutation is a production change; no control was rerun.

Details: `01-MUTATION-DECISIONS.tsv`, `01-MUTATION-EVIDENCE-MATRIX.md`, `01-ACCEPTANCE-PROVENANCE.md`.
