# P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01 — evidence packet

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, patch, contract, amendment or manifest is changed; no mutant source, executable, target directory or source copy is committed; the previous packets are referenced, not modified or duplicated.

Derived status: **MUTATION_CONTROL_BLOCKED**. Tranche 2 attempted 6 of 10 controls; new executor candidates 5 (C23, C10, C16, C26, C3a), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**; stopped at C5; not attempted A3, A6, C8, C15. Architect-accepted before this tranche: 12 of 35 (unchanged here).

## Identities

- Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16`; protocol.rs `a2df2841b99cad5d452008a6d4150e9369d07e66`, model_tests.rs `28085ce232827301104b5d757af042ec933976f8`, unit_tests.rs `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`
- frozen catalog `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` blob `1918374263740d2f98640da00fc466737945975e` sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` at `9f22f890d8532df395f22cd8db8db2552bfdd14c`
- accepted unchanged-core evidence: Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`; AD-1-R5 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` (Owner adoption `9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4`); accepted F-R proof `c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b`
- accepted Tranche-1 evidence: commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7`, tree `7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400`, parent `3695a817a6190781dc66769a72a487b442d3e6da`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`
- pinned unchanged-baseline executable (the linked receipts') `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`
- this evidence commit's parent: `ebe42009ba1bd262cf31ae2f401e02cec101afa7`; review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01`

## Contents

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `TRANCHE-SUMMARY.md` | every attempted control: identities, build, executable, selection, raw and outer outcome, printed property, discovered count, resources, restoration, restored-side link; the campaign status |
| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, sha256; not republished) |
| `documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md` | record of the Architect's Tranche-1 acceptance (12 of 35; A4 satisfied) |
| `documents/CAMPAIGN-STATUS-BEFORE-TRANCHE2.md` | the campaign status before this tranche (12 of 35) |
| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |
| `receipts/00-authentication.txt` | provenance of every input |
| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |
| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |
| `receipts/03-t2-controls/` | tooling controls on synthetic and historical evidence |
| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |
| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots |
| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |
| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |
| `receipts/51-C23/` … `60-C15/` | one directory per attempted control: gate, worktree, apply, build, run, restore, link, RESULT |
| `receipts/80-STOP-RECORD.md` | present only when the tranche stopped |
| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |
| `tools/` | the tools this mission changed or added (unchanged ones referenced in `CARRIED-FORWARD.md`) |
| `development/` | the development log, every failing development output, every revision and its diff |
| `SHA256SUMS` | every packet file but itself |

## Freeze

Frozen files: 151; recorded 2026-10-15T01:00:02+0100; 05-FREEZE.json sha256 `389f6ce6b342c9dd17a2d32a38fd8b1eba8ba6f9dce92453de61aa805dba3955`. Every supervised step re-checks the freeze.

## Limitations

- Every classification here is an executor candidate for Architect review; no kill is accepted by the executor.
- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.
- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually built and run.
- The restored sides are linked, never rerun: the accepted exact-final Stage-1 and Stage-2 receipts; for C8 the Stage-1 composed-quotient F-B PASS (restored side only, AD-1-R4 F); for C15 the AD-1-R5 F refinement discharge record, which is not a run and never PASS (the F-R receipt stays INCOMPLETE).
