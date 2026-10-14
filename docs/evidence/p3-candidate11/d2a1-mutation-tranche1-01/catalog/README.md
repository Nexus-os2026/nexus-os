# catalog/ — the D2A-1 control catalog, BLOCKED

`catalog.json` is the deterministic output of `tools/derive_catalog.py` over the authenticated inputs: all 35
kill-required controls of contract 7.5, and A4 as the separate, already-satisfied comparison.

**Status: CONTROL_CATALOG_BLOCKED. NOT FROZEN.** 14 definitions are READY and 21 are BLOCKED, because no authorised
source gives them an exact change (`../CONTROL-CATALOG-BLOCKER.md`). The static catalog gate therefore fails
(`../receipts/20-static-gate.txt`), and under mission sections 8 and 19 nothing was built or run.

- `catalog.json` — every control's id, contract row and line, named property or test, configuration and test, ceiling,
  receipt types, expected primary failure, restored-receipt source and its verified link, witness and special rules,
  citations, executor status, Architect review status (pending for every control) and execution status (NOT
  ATTEMPTED). READY entries add the anchor, the replacement, the counts at C, the patch and its sha256, and the mutant
  blob. BLOCKED entries carry no anchor, replacement or patch; they carry the executor's observations instead.
- `CATALOG.md` — a one-table rendering of `catalog.json`.
- `patches/<id>.patch` — documentary copies of the 14 READY patches. C1, C25 and C4 are the authenticated documentary
  patches AD-1-R4 G.2 cites, byte for byte; the other eleven were generated from the authorised anchor and replacement.
  **None was applied to any worktree.** None is frozen.

The version label `d2a1-controls-v1` names this derivation. Because it is not frozen, it was not published at
`docs/evidence/p3-candidate11/d2a1-control-catalog-v1/`; that path was left unused.
