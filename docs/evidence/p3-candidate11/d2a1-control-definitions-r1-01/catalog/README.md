# catalog/ — candidate catalog `d2a1-controls-v1-r1-candidate`

Status: CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING. Acceptance belongs to the Sole Architect after review; this file claims none.

- `catalog.json` — the machine-readable catalog: 36 entries in contract 7.5's order (35 kill-required controls and A4, the separate accepted comparison). Each control carries its edits (anchor, replacement, expected count 1, line at C), patch text and sha256, mutant blob and sha256, named property, test, configuration, ceiling, receipt type, restored-receipt link and restoration mapping, and its definition, review and execution status.
- `CATALOG.md` — one line per entry.
- `PATCH-IDENTITIES.md` — every patch and mutant identity.
- `patches/<id>.patch` — 35 documentary patches against Candidate C. 14 are the blocked catalog's patches, byte for byte; 21 encode the Architect's section-6 definitions. None has been applied to any worktree, built or run.

Every control: definition CANDIDATE_READY, Architect review pending, execution NOT_ATTEMPTED.
