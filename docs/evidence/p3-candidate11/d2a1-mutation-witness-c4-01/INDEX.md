# P3-C11-DR01-D2A1-MUTATION-WITNESS-C4-01 — evidence packet

Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High. Evidence only: no source, catalog, patch, contract or manifest is changed; no mutant source, executable, target directory or source copy is committed; the previous packets are referenced, not modified or duplicated.

Derived status: **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**. This mission attempted 1 of 1 controls; new executor candidates 1 (C4), each **KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING**; stopped at none; not attempted none. Architect-accepted before this mission: 33 of 35 (unchanged here; a C4 candidate is pending Architect review, never accepted here). C7: MUTATION_CONTROL_BLOCKED (Architect ruling): not executed.

## Identities

- Candidate C `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, parent `71264aa81a40b86189783facc8437e0427887c16`; protocol.rs `a2df2841b99cad5d452008a6d4150e9369d07e66`, model_tests.rs `28085ce232827301104b5d757af042ec933976f8`, unit_tests.rs `55dc66409eaccdaa93e8f0a56f2ccb14be81e114`
- frozen catalog `docs/evidence/p3-candidate11/d2a1-control-definitions-r2-01/catalog/catalog.json` blob `1918374263740d2f98640da00fc466737945975e` sha256 `d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83` at `9f22f890d8532df395f22cd8db8db2552bfdd14c`
- accepted unchanged-core evidence: Stage 1 `f023cc3d031a2a89e5ecbd4086e15f2e50869acd`, Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`
- accepted Tranche-1 evidence: commit `ebe42009ba1bd262cf31ae2f401e02cec101afa7`, tree `7d8ab59e4b3e61196d9c4cee3cb3a046fa8c4400`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche1-continue-02`
- Tranche-2 stopped evidence: commit `2e06860f4b404d1a79a0d290116239b5545b931f`, tree `4c929aa877816cc0cb5c42e32ddbdd75b380bacd`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-exec-01`
- accepted Tranche-2 continuation evidence: commit `b97205b2b9ea058ee864b6649dc5002d28b1edeb`, tree `6db2f923278f9f6ea7bacd97bd29444d4e8d6816`, parent `2e06860f4b404d1a79a0d290116239b5545b931f`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche2-continue-02`
- Tranche-3 stopped evidence (C2 Architect-accepted): commit `774f457a4fc27f6b26e132a0caf71b683b9af8cd`, tree `f74660b98288c53e285bcb4d3b98fb9f55962b57`, parent `b97205b2b9ea058ee864b6649dc5002d28b1edeb`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-exec-01`
- Tranche-3 continuation evidence (C24 C3b C3c C6 Architect-accepted; C7 MUTATION_CONTROL_BLOCKED): commit `c10681b3c65dd377b5a13d08140427e8b34441d0`, tree `7071c0ce01c9ed9a01f28ff7792d648516466a48`, parent `774f457a4fc27f6b26e132a0caf71b683b9af8cd`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-continue-02`
- Tranche-3 tail evidence (A1 A2 C20 A5 Architect-accepted): commit `e41c84b1c41dded6cc9816f11f6603f5d80587a9`, tree `10b408a1432edeed5720024bd6b8807852ecb464`, parent `c10681b3c65dd377b5a13d08140427e8b34441d0`, ref `refs/heads/review/p3-c11-d2a1-mutation-tranche3-tail-01`
- C1/C25 witness evidence (C1 C25 Architect-accepted): commit `74b3f485c78d0d964b85ff958c09c69d85058ad5`, tree `a1201237e741336bae0f4bcd3c1dcd2b892a2935`, parent `e41c84b1c41dded6cc9816f11f6603f5d80587a9`, ref `refs/heads/review/p3-c11-d2a1-mutation-witness-c1-c25-01`
- accepted healthy type-(e3) witness receipt (Stage 2 `89326b859792a442c1853c3d35d30a48301c009e`): `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/52-witness-C4`; accepted F-8 type-(e1) receipt: `docs/evidence/p3-candidate11/final-baseline-remaining-configs-01/receipts/64-full-F-8`
- pinned unchanged-baseline executable (the linked receipts') `/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3` sha256 `cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c`
- this evidence commit's parent: `74b3f485c78d0d964b85ff958c09c69d85058ad5`; review ref `refs/heads/review/p3-c11-d2a1-mutation-witness-c4-01`

## Contents

| Path | What |
|---|---|
| `INDEX.md` | this file |
| `TRANCHE-SUMMARY.md` | C4: identities, build, executable, the type-(a) primary (first property as printed, raw state counts), the type-(c) mutant witness (both exact positive lines, header, the two lines of other properties; the same pinned executable; G.6 at each changed append), resources, restoration and its authentication, the linked type-(e3) and type-(e1) receipts, the G.5 record; the campaign status before Architect review |
| `CARRIED-FORWARD.md` | every immutable file referenced, linked, gated on or run unchanged (commit, path, blob, sha256; not republished), including the accepted controls' and C7's blocked receipts and the healthy witness receipts |
| `documents/ARCHITECT-RULING-C1-C25-WITNESS-ACCEPTANCE.md` | record of the Architect's ruling: C1 and C25 accepted (33 of 35); C7 MUTATION_CONTROL_BLOCKED; this mission's authorization (C4, under G.5 and G.6) |
| `documents/CAMPAIGN-STATUS-BEFORE-WITNESS-C4.md` | the campaign status before this mission (33 of 35) |
| `documents/DUPLICATE-AND-ZERO-LENGTH-MAP.tsv` (+ `.md`) | every packet file byte-equal to a previous-packet file and every empty file, each with its exact path, class, role, descriptor and prior occurrences |
| `receipts/00-authentication.txt` | provenance of every input, including every previous packet |
| `receipts/01-supervision-selftest/` | the unchanged enforcement chain on non-Nexus processes |
| `receipts/02-EXPECTATIONS.json` (+ console) | every expectation, derived from the frozen catalog |
| `receipts/03-t3-controls/` | tooling controls on synthetic, historical and authenticated evidence (every required positive and negative control of mission section 11, G.5 and G.6 included) |
| `receipts/04-tool-diff/` | the old-to-new tool diff, its authorization map and mechanical check |
| `receipts/04b-rehearsals/` | render.py and build_packet.py on synthetic evidence roots, and the stopped-first-control path |
| `receipts/05-FREEZE.json`, `05-FREEZE-RECORD.txt` | the freeze |
| `receipts/06-PRE-EXECUTION-GATE.txt` (+ `06-gate-authentication.txt`) | the static pre-execution gate |
| `receipts/81-C4/` | C4: gate, worktree, apply, build, 04-run (primary), 04c-witness (mutant witness), 05-restore, 05b-restore-auth, 06a-healthy-witness (e3), 06b-link (e1), 07-g5, RESULT |
| `receipts/80-STOP-RECORD.md` | present only when the mission stopped |
| `receipts/90-final-gate.txt` (+ `90-gate-authentication.txt`) | the final gate |
| `tools/` | the tools this mission changed (unchanged ones referenced in `CARRIED-FORWARD.md`) |
| `development/` | the development log, every failing development output, every revision and its diff |
| `publication/10-build-packet.txt` | the packet build's console |
| `SHA256SUMS` | every packet file but itself |

## Freeze

Frozen files: 80; recorded 2026-10-15T12:59:25+0100; 05-FREEZE.json sha256 `6a8109d78b9a9606439bf6f71cdb9b5e44ecf1f0643e65f60c49d5a47179814e`. Every supervised step re-checks the freeze.

## Limitations

- Every classification here is an executor result for Architect review; no kill is accepted by the executor.
- Results are the explored model's results on Candidate C's mutants, under AD-1-R4 J's storage assumptions; nothing here is a filesystem, network or OS-containment claim.
- Reproducible builds are not established (AD-1-R4 I); every identity is the sha256 of the executable actually built and run.
- The restored sides are linked, never rerun: the accepted exact-final Stage-2 F-8 receipt (type (e1)) and the accepted Stage-2 C4 healthy witness (type (e3)). The primary is the original explorer; no quotient, symmetry reduction, changed predicate, changed Oracle, changed schedule or instrumentation is used on the mutant.
- AD-1-R4 G.5 and G.6 apply to C4 only: the primary's first property is recorded as printed and never relabelled; P4 and P8 are established only by the witness on the same pinned executable, both at their frozen loci; G.5(E)'s Architect review is pending; the (F) recording is proposed for Architect review, not made.
- C7 (MUTATION_CONTROL_BLOCKED) and the 33 accepted controls were not executed; C7's receipts are cited as they stand.
