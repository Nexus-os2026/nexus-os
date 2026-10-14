# catalog/ — candidate catalog `d2a1-controls-v1-r2-candidate`

Status: CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING. Acceptance belongs to the Sole Architect after review; this file claims none.

Supersedes, as a candidate, `d2a1-controls-v1-r1-candidate` (sha256 `a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479`, evidence commit `f81159046bf0b62666851bc1185992318302d31f`), which stays immutable review history. Changed from it: A6 only.

- `catalog.json` — the machine-readable catalog: 36 entries in contract 7.5's order (35 kill-required controls and A4, the separate accepted comparison). Each control carries its edits (anchor, replacement, expected count 1, line at C), patch text and sha256, mutant blob and sha256, named property, test, configuration, ceiling, receipt type, restored-receipt link and restoration mapping, and its definition, review and execution status. `r2` lists the changed and unchanged controls; A6 carries `r2_correction`.
- `CATALOG.md` — one line per entry.
- `PATCH-IDENTITIES.md` — every patch and mutant identity.
- `patches/<id>.patch` — 35 documentary patches against Candidate C. 34 are R1's patches, byte for byte (14 of them first carried from the blocked catalog); A6's is R1's with one KEY_FIELDS receipt-metadata hunk added. None has been applied to any worktree, built or run.

Every control: definition CANDIDATE_READY, Architect review pending, execution NOT_ATTEMPTED.
