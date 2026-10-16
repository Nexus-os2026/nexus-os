# H.6 attestation — status

**Is H.6 complete, or awaiting decisions? Awaiting decisions: CANDIDATE — PENDING_DECISIONS.**

- `05-H6-ATTESTATION-CANDIDATE.json` (sha256 `ac3a73a501c1478afd7dabd6ae531b0135a2d6c0553c81cad29e92cf42ca64a6`)
  binds A–K: the exact source; ct-1; the alignment; the 22 reconciliations; the 35 rulings with provenance; every
  configuration receipt and F-R's proof discharge; the branch accounting; Fast Local; the actual current-tree decisions
  with their sources; assumptions and limitations; the outstanding decisions.
- 140 inputs, each re-hashed at generation (`05-H6-INPUT-HASHES.tsv`, sha256 `c095257b…0bdb`), each labelled by nature
  (raw measurement, accepted evidence, derived proof, executor analysis, Architect or Owner decision record, pending).
- **Detached:** local evidence root only; not in Candidate C or any commit it attests; its Git blob is in no object store.
- It is a candidate because no current-tree review decision exists for any of the 183, M18's mapping is unruled, and
  the campaign scope (line 915) is unruled. It invents no approval, signature or measurement.

Details: `05-H6-ATTESTATION.md`, `05-H6-DECISION-PROVENANCE.md`, `05-H6-UNRESOLVED.md`.
