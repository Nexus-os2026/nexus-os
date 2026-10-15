# P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02 — evidence packet

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, patch, contract, amendment or manifest is changed; no mutant source, executable, target directory or source copy is committed; the previous packets are referenced, not modified or duplicated; C5 is not rebuilt.

Derived status: **READY_FOR_MUTATION_TRANCHE2_FINAL_ARCHITECT_REVIEW**. This mission attempted 5 of 5 controls; new executor candidates 5 (C5, A3, A6, C8, C15), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**; stopped at none; not attempted none. Architect-accepted before this mission: 17 of 35 (unchanged here).

## Identities

- Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16`; protocol.rs `a2df2841b99cad5d452008a6d4150e9369d07e66`, model_tests.rs `28085ce232827301104b5d757af042ec933976f8`, unit_tests.rs `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`
- frozen catalog `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` blob `1918374263740d2f98640da00fc466737945975e` sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` at `9f22f890d8532df395f22cd8db8db2552bfdd14c`
- accepted unchanged-core evidence: Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`; AD-1-R5 `4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473` (Owner adoption `9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4`); accepted F-R proof `c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b`
- accepted Tranche-1 evidence: commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7`, tree `7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`
- Tranche-2 stopped evidence: commit `2e06860f4b404d1a79a0d290116239b5545b931f`, tree `4c929aa877816cc0cb5c42e32ddbdd75b380bacd`, parent `ebe42009ba1bd262cf31ae2f401e02cec101afa7`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01`
- C5's preserved build `docs/evidence/p3-candidate11/d2a1-mutation-tranche2-exec-01/receipts/56-C5/03-build` at `2e06860f4b404d1a79a0d290116239b5545b931f`; preserved executable `/home/nexus/NEXUS/nexus-os-p3-c11-mt2x-C5/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `90a608834ad0227632ba26f095de5e16914942da86d2f05fdaab5a0d745c15f9` size 8,986,720
- pinned unchanged-baseline executable (the linked receipts') `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`
- this evidence commit's parent: `2e06860f4b404d1a79a0d290116239b5545b931f`; review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02`

## Contents

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `TRANCHE-SUMMARY.md` | every attempted control: identities, build (C5: the admitted preserved build), executable, selection, raw and outer outcome, printed property, discovered count, resources, restoration or reauthentication, restored-side link; the campaign status before Architect review |
| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, sha256; not republished) |
| `documents/ARCHITECT-RULING-TRANCHE2-PARTIAL-ACCEPTANCE.md` | record of the Architect's Tranche-2 partial acceptance (17 of 35; C5 kill pending) with the exact evidence identities |
| `documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md` | record of the Architect's narrow C5-specific build-admissibility ruling |
| `documents/CAMPAIGN-STATUS-BEFORE-CONTINUE-02.md` | the campaign status before this mission (17 of 35) |
| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |
| `receipts/00-authentication.txt` | provenance of every input, including the previous packet |
| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |
| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |
| `receipts/03-c2-controls/` | tooling controls on synthetic and historical evidence (the C5 admission and pin negative controls; the unchanged general classifier on the historical C5 build) |
| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |
| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots |
| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |
| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |
| `receipts/61-C5/` | C5: gate, build admission, preserved worktree, executable pin, run, reauthentication, link, RESULT |
| `receipts/62-A3/` … `65-C15/` | one directory per later attempted control: gate, worktree, apply, build, run, restore, link, RESULT |
| `receipts/80-STOP-RECORD.md` | present only when the mission stopped |
| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |
| `tools/` | the tools this mission changed or added (unchanged ones referenced in `CARRIED-FORWARD.md`) |
| `development/` | the development log, every failing development output, every revision and its diff |
| `publication/10-build-packet.txt` | the packet build's console |
| `SHA256SUMS` | every packet file but itself |

## Freeze

Frozen files: 118; recorded 2026-10-15T02:11:13+0100; 05-FREEZE.json sha256 `4b776f02332a9d3049f68fdb8bec1c60c45ae32637c678015adb4b0791754fb6`. Every supervised step re-checks the freeze.

## Limitations

- Every classification here is an executor candidate for Architect review; no kill is accepted by the executor.
- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.
- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually built and run. C5's executable is the one its Tranche-2 build wrote, pinned where cargo wrote it.
- The restored sides are linked, never rerun: the accepted exact-final Stage-1 and Stage-2 receipts; for C8 the Stage-1 composed-quotient F-B PASS (restored side only, AD-1-R4 F); for C15 the AD-1-R5 F refinement discharge record, which is not a run and never PASS (the F-R receipt stays INCOMPLETE).
