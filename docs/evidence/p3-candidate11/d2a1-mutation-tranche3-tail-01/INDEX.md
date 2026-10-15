# P3-C11-DR01-D2A1-MUTATION-TRANCHE3-TAIL-01 — evidence packet

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, patch, contract or manifest is changed; no mutant source, executable, target directory or source copy is committed; the previous packets are referenced, not modified or duplicated.

Derived status: **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**. This mission attempted 4 of 4 controls; new executor candidates 4 (A1, A2, C20, A5), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**; stopped at none; not attempted none. Architect-accepted before this mission: 27 of 35 (unchanged here). C7: MUTATION_CONTROL_BLOCKED (Architect ruling), not executed.

## Identities

- Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16`; protocol.rs `a2df2841b99cad5d452008a6d4150e9369d07e66`, model_tests.rs `28085ce232827301104b5d757af042ec933976f8`, unit_tests.rs `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`
- frozen catalog `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` blob `1918374263740d2f98640da00fc466737945975e` sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` at `9f22f890d8532df395f22cd8db8db2552bfdd14c`
- accepted unchanged-core evidence: Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`
- accepted Tranche-1 evidence: commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7`, tree `7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`
- Tranche-2 stopped evidence: commit `2e06860f4b404d1a79a0d290116239b5545b931f`, tree `4c929aa877816cc0cb5c42e32ddbdd75b380bacd`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01`
- accepted Tranche-2 continuation evidence: commit `b97205b2b9ea058ee864b6649dc5002d28b1edeb`, tree `6db2f923278f9f6ea7bacd97bd29444d4e8d6816`, parent `2e06860f4b404d1a79a0d290116239b5545b931f`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02`
- Tranche-3 stopped evidence (C2 Architect-accepted): commit `774f457a4fc27f6b26e132a0caf71b683b9af8cd`, tree `f74660b98288c53e285bcb4d3b98fb9f55962b57`, parent `b97205b2b9ea058ee864b6649dc5002d28b1edeb`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-exec-01`
- Tranche-3 continuation evidence (C24 C3b C3c C6 Architect-accepted; C7 MUTATION_CONTROL_BLOCKED): commit `c10681b3c65dd377b5a13d08140427e8b34441d0`, tree `7071c0ce01c9ed9a01f28ff7792d648516466a48`, parent `774f457a4fc27f6b26e132a0caf71b683b9af8cd`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-continue-02`
- pinned unchanged-baseline executable (the linked receipts') `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`
- this evidence commit's parent: `c10681b3c65dd377b5a13d08140427e8b34441d0`; review ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-tail-01`

## Contents

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `TRANCHE-SUMMARY.md` | every attempted control: identities, build, executable, selection, raw and outer outcome, printed property, raw state counts, the tranche-3 rules, L1's complete-search evidence (C20, A5), resources, restoration, restored-side link; the campaign status before Architect review |
| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, sha256; not republished), including the accepted controls' and C7's blocked receipts |
| `documents/ARCHITECT-RULING-TRANCHE3-CONTINUATION-ACCEPTANCE.md` | record of the Architect's ruling: C24, C3b, C3c, C6 accepted (27 of 35); C7 MUTATION_CONTROL_BLOCKED; this mission's authorization (A1 A2 C20 A5) |
| `documents/CAMPAIGN-STATUS-BEFORE-TRANCHE3-TAIL.md` | the campaign status before this mission (27 of 35) |
| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |
| `receipts/00-authentication.txt` | provenance of every input, including every previous packet |
| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |
| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |
| `receipts/03-t3-controls/` | tooling controls on synthetic and historical evidence (every required negative control) |
| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |
| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots, and the stopped-first-control path |
| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |
| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |
| `receipts/81-A1/` … `receipts/84-A5/` | one directory per attempted control: gate, worktree, apply, build, run, tranche-3 rules, restore, link, RESULT |
| `receipts/80-STOP-RECORD.md` | present only when the mission stopped |
| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |
| `tools/` | the tools this mission changed (unchanged ones referenced in `CARRIED-FORWARD.md`) |
| `development/` | the development log, every failing development output, every revision and its diff |
| `publication/10-build-packet.txt` | the packet build's console |
| `SHA256SUMS` | every packet file but itself |

## Freeze

Frozen files: 82; recorded 2026-10-15T08:32:41+0100; 05-FREEZE.json sha256 `7538fcc1c53600be299d0d289a19b7608cb32ca841f9607001b498f07019fe3d`. Every supervised step re-checks the freeze.

## Limitations

- Every classification here is an executor result for Architect review; no kill is accepted by the executor.
- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.
- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually built and run.
- The restored sides are linked, never rerun: the accepted exact-final Stage-1 F-1 and F-2 receipts. Every mutant run is the original explorer; no quotient or symmetry reduction is used on a mutant.
- L1 is classified only after a complete search (every discovered state expanded, no INCOMPLETE line, within the ceiling); an INCOMPLETE run kills nothing.
- C7 is MUTATION_CONTROL_BLOCKED by the Architect and was not executed; its receipts are cited as they stand.
