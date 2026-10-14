# P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01 — evidence packet

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, patch, contract, amendment or manifest is changed; no mutant source, executable or target directory is committed.

Derived tranche status: **MUTATION_CONTROL_BLOCKED**. Candidate kill evidence: 0 of 12 (none), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**.

## Identities

- Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16`; protocol.rs `a2df2841b99cad5d452008a6d4150e9369d07e66`, model_tests.rs `28085ce232827301104b5d757af042ec933976f8`, unit_tests.rs `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`
- frozen catalog `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` blob `1918374263740d2f98640da00fc466737945975e` sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` at `9f22f890d8532df395f22cd8db8db2552bfdd14c` (refs/heads/review/p3-c11-d2a1-control-definitions-r2-01); definitions record blob `c79b75b65b85e1fad53592e92ab2d8507d6c8799` sha256 `1da1ae92f313496dade2bfea5c508af1e17bf2e39b9bd11cca3e448c35f0cb26`
- base contract sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c`; AD-1-R4 sha256 `00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693`; AD-1-R5 sha256 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` (R5 evidence commit `c1e46fc404a74ed76b848b709a7a246b59f3347f`)
- accepted unchanged-core evidence: Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`
- pinned unchanged-baseline executable `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c` size 8,984,464
- evidence commit parent: `9f22f890d8532df395f22cd8db8db2552bfdd14c`; review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-exec-01`

## Contents

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `TRANCHE-SUMMARY.md` | every baseline, the healthy build and every attempted control, with identities, outcomes, classifications, resources, restoration and links |
| `ARCHITECT-RULING-CATALOG-FREEZE.md` | the catalog-freeze Architect ruling, as received, and the frozen catalog's identity |
| `receipts/00-authentication.txt` | full provenance: every input authenticated before any worktree or command |
| `receipts/01-supervision-selftest/` | the enforcement chain proven on non-Nexus processes |
| `receipts/02-EXPECTATIONS.json`, `02-expectations.txt` | every selection and expectation derived from the catalog and C's sources, cross-checked |
| `receipts/03-tool-controls/` | classifiers and supervisor exercised on synthetic and historical evidence (positive and negative controls) |
| `receipts/04-rehearsal/` | worktree, apply, verify, restore and link steps rehearsed with a synthetic patch in a development worktree (nothing built or run) |
| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze: sha256 of every frozen tool, input and expectation, before the first baseline invocation |
| `receipts/10-baseline-X-1/` … `15-baseline-T-RES/` | the standalone baseline receipts on the pinned executable: X-1 (e1), T-BIND, T-CELL, W6, T-PREC, T-RES (e2) |
| `receipts/19-healthy-worktree/`, `20-healthy-build/` | the exact-command healthy build (e4) with its complete compiler output and target manifest |
| `receipts/31-C22/` … `42-C18b/` | one directory per attempted control: gate, worktree, apply, build, run, restore, link, RESULT |
| `receipts/90-final-gate.txt` | the final gate |
| `tools/` | the minimal reproducible tooling (Stage-2's configs.py, normalize.py, report.py, verify_tree.py and scope-wrap.sh byte for byte; the frozen vocabulary is read from the Stage-2 evidence commit) |
| `development/` | the development log, every defective tool version and its repair diff, failing development outputs |
| `SHA256SUMS` | every packet file but itself |

## Freeze

Frozen files: 64; recorded 2026-10-14T20:02:06+0100; freeze record sha256 `ad79d893774dc934d25309737fae0a15ee288ae3af86ae85c2ef131b0e572224`. No frozen file was changed after the freeze; every supervised step re-checks it.

## Limitations

- Classifications are executor candidates for Architect review; no kill is accepted here.
- Results are the explored model's and the named tests' results on Candidate C and its mutants, under AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.
- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually built and run.
- The F-B2 and F-G restored-side receipts are the accepted Stage-2 receipts, linked (not rerun) under AD-1-R4 I's proposed receipt-linking clause S-5 as the mission's section 15 directs; each link is checked at its control's own verified restoration.

