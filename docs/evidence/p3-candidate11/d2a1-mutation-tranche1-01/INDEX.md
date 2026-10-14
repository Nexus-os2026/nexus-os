# D2A-1 mutation tranche 1 — index (blocker packet)

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort
High. For the Sole Architect's review.

**Status: CONTROL_CATALOG_BLOCKED.** Of the 35 kill-required controls of contract 7.5, 21 have no exact definition in
any authorised source — four of them (C11, C17, C18a, C18b) in tranche 1. The mission forbids inventing one
(section 7), and no Rust build may start before the static catalog gate passes (section 8). So, per section 19:
**no baseline test and no mutant was executed.** No cargo or rustc invocation, no mutant worktree, no mutant build,
no test run of any kind. The pinned baseline executable was hashed and sized only. Tranche kill count: 0.

**Evidence only.** No source, contract, AD-1-R4, AD-1-R5 or manifest changed. D2A-1 remains unaccepted and
unintegrated; nothing here discharges A-1 or A-2 or completes Phase Three.

## The Owner's AD-1-R5 adoption (mission 5)

Recorded at `docs/evidence/p3-candidate11/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md`, sha256
`9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4`, blob `1492e185cb475bd32d95079a39b49078c0e23ddc`.
This file is audit evidence of the Owner's decision, not authority.

## Identities (`receipts/00-authentication.txt`: ALL CHECKS OK; re-run in `receipts/60-gate-authentication.txt`)

| Item | Value |
|---|---|
| Candidate C | `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, single parent `71264aa81a40b86189783facc8437e0427887c16` |
| Blobs | protocol.rs `a2df2841…`, model_tests.rs `28085ce2…`, unit_tests.rs `55dc6640…`; manifest ct-1 `5d31cb45…` (sha256 `20bcc6fe…1a44279`) |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` |
| AD-1-R4 | sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693` |
| AD-1-R5 | sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473`, blob `ae3cabdb…`, evidence commit `c1e46fc4…` |
| Accepted F-R proof | sha256 `c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b` |
| Sealed controls_defs.py | sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` |
| Documentary patches | C1 `5ecd716a…`, C25 narrow `29a2c1e5…`, C4 `24ac7add…` |
| Pinned baseline executable | sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`, 8,984,464 bytes; inode and mtime unchanged since Stage 2 (final gate G6); never run |
| Evidence commit parent | `c1e46fc404a74ed76b848b709a7a246b59f3347f` |

## Results

| Step | Result | Record |
|---|---|---|
| Input authentication | ALL CHECKS OK | `receipts/00-authentication.txt` |
| Catalog derivation | `catalog/catalog.json`, version `d2a1-controls-v1`, sha256 `a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747`: 35 kill-required + A4; READY 14, BLOCKED 21; **not frozen** | `catalog/`, `CONTROL-CATALOG-BLOCKER.md` |
| Static catalog gate | **FAIL — CONTROL_CATALOG_BLOCKED**: 314 checks hold; exactly the 21 definition-completeness checks fail | `receipts/20-static-gate.txt` |
| Gate controls | 14 seeded defects each detected; positive control fails only the 21 definition checks | `receipts/21-gate-controls.txt` |
| Baseline exact receipts (mission 10) | **not run** (section 19) | — |
| Tranche 1 (12 controls) | **none attempted**; 0 killed | `TRANCHE-SUMMARY.md` |
| Final gate | see its receipt | `receipts/60-final-gate.txt` |

READY (exact definition from an authorised source; every static check holds): C1, C2, C4, C9, C10, C12, C14, C19,
C21, C22, C24, C25, C27, C28. BLOCKED (contract prose only): C3a, C3b, C3c, C5, C6, C7, C8, C11, C15, C16, C17, C18a,
C18b, C20, C23, C26, A1, A2, A3, A5, A6. A4: SATISFIED — ARCHITECT ACCEPTED (Stage-1 `receipts/50-A4`), not rerun.

## Exact next blocker

Exact, Architect-ruled definitions (an anchor and its replacement, or a patch, at Candidate C) for the 21 BLOCKED
controls. `CONTROL-CATALOG-BLOCKER.md` lists, for each, where the change would land at C and the questions a
definition must settle. Until they exist, the catalog cannot be frozen and no mutant can be built.

## Not produced, and why

- `docs/evidence/p3-candidate11/d2a1-control-catalog-v1/`: not created, because the catalog is not frozen
  (`catalog/README.md`).
- All 35 frozen patches; a catalog freeze record: the gate did not pass. `receipts/50-TOOLS-RECORD.json` records the
  tools and documents used for this packet instead; it is not a catalog freeze.
- Baseline exact receipts; per-control directories (no control was attempted); restoration receipts (no mutant
  worktree was created).

## Files

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `CONTROL-CATALOG-BLOCKER.md` | why the catalog is blocked; the 21 blocked controls; the 14 READY definitions; observations |
| `TRANCHE-SUMMARY.md` | the 12 tranche-1 controls: nothing attempted |
| `catalog/` | `catalog.json`, `CATALOG.md`, `README.md`, `patches/` (14 documentary patches, none applied) |
| `receipts/` | authentication, static gate, gate controls, tools record, final gate |
| `development/` | `DEVELOPMENT-LOG.txt` (D0 to D5), failed runs as found, diffs of the intermediate catalogs |
| `tools/` | `authenticate.py`, `derive_catalog.py`, `blocked_notes.py`, `static_gate.py`, `gate_controls.py`, `render_docs.py`, `record_tools.py`, `final_gate.py`, `build_packet.py`, `publish_checks.py` |
| `SHA256SUMS` | every file of this directory except itself |

## How to verify

From this directory: `sha256sum -c SHA256SUMS`. The tools re-run read-only given a root directory holding the
authenticated inputs: `authenticate.py ROOT` writes them to `ROOT/inputs/`; `derive_catalog.py ROOT` then
reproduces `catalog/` byte for byte; `static_gate.py ROOT` and `gate_controls.py ROOT` reproduce `receipts/20` and
`21`. They read git objects of `/home/nexus/NEXUS/nexus-os` and build nothing.
