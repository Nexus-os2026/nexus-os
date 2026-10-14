C22 — MUTATION CONTROL KILL ACCEPTED

# Architect ruling record — C22 acceptance

Recorded by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort High) for mission
P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, from section 1 of that mission as issued by the Sole Nexus OS Architect
(ChatGPT GPT-6) via the Owner.

**This file is evidence of the Architect's ruling. A string is not authority.** The authority is the Sole Architect's
ruling itself. Claude Code did not accept C22 and makes no acceptance claim of its own: the executor's own
classifications of C22 are the historical ones below, and they stand unchanged.

## The ruling

> C22 — MUTATION CONTROL KILL ACCEPTED
>
> Receipt type: (d) intentional mutant compile failure.

The exact-command Candidate-C healthy build is accepted as C22's type-(e4) restored evidence.

Campaign status stated by the ruling: C22 accepted; 1 of 35 kill-required controls accepted; A4 separately
satisfied; 34 kill-required controls remain. Tranche 1: C22 accepted; 1 of 12 accepted; 11 controls remain.

Directions that bind this mission: do not rebuild, reapply or rerun C22; do not regenerate its diagnostic; do not
rewrite its original packet; do not change the frozen catalog or the C22 patch.

## The prior mission stopped correctly

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01 stopped at C22 (control 1 of 12) after its frozen
source/restoration verifier failed, recorded the cause (`receipts/80-STOP-RECORD.md`), repaired nothing, reran
nothing, and executed no later control. Evidence commit `19eda8cc7f559054933fed4a93bdb0a18cc725d4` (tree
`1b313ec5c23b5bf76db90b5fa89758d9cd880a75`, parent `9f22f890d8532df395f22cd8db8db2552bfdd14c`), review ref
`refs/heads/review/p3-c11-d2a1-mutation-tranche1-exec-01`, packet
`docs/evidence/p3-candidate11/d2a1-mutation-tranche1-exec-01/`.

## The executor's original classifications (immutable historical outputs of the defective frozen tool)

| Receipt in the previous packet | First line, unchanged |
|---|---|
| `receipts/31-C22/03-build/CLASSIFICATION.txt` | `CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE` |
| `receipts/31-C22/05-restore/RESTORE.txt` | `RESTORE: FAILED` |
| `receipts/31-C22/RESULT.txt` | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |

They are not edited, overwritten or re-derived here. This record supplies the Architect's acceptance decision
alongside them; it does not turn either classification into a success.

## The exact compiler evidence (as recorded in the previous packet)

- frozen patch sha256 `e017ced648c6d4329ab8c744441e8fc03e8207b47e9cb3387202cc29ce7e95c6`
  (`receipts/31-C22/02-apply/APPLY.json`, equal to the frozen catalog's `patches/C22.patch`)
- command: `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`, the exact command of the
  healthy build, with a fresh worktree-local target, `CARGO_NET_OFFLINE=true`, 16 GiB, zero swap, 900 s
- result: exit 101 by a normal exit; no signal, no OOM, no timeout; 124.73 s of 900 s; scope memory peak 2,615,095,296
  bytes; swap peak 0 (`receipts/31-C22/03-build/PARSED-RECEIPT.json`)
- diagnostics: exactly one coded error, `error[E0283]: type annotations needed`, and cargo's one summary
  `error: could not compile \`nexus-governed-control\` (lib test) due to 1 previous error`
- required location: `crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089:5`
- required assertion, as rendered at line 1089: `<Ticket as NotClone<_>>::check();`
  (`receipts/31-C22/03-build/build.stderr`)
- no compiler crash; no mutant executable produced (NOT PRODUCED BY DESIGN); no mutant test run (NOT PRODUCED BY
  DESIGN)

## Source identity before and after

- before the build (`receipts/31-C22/03-build/descriptor.json`, `source_before`): verdict `EXACTLY_C_PLUS_PATCH`;
  HEAD `d5964ee5326c2733177307db6f507328023d361a`, tree `51bac5fd335999cc995c0ab9f582a30244973ef0`, detached;
  nothing staged; no other tracked content, mode or missing-file difference; `protocol.rs` git blob
  `694b744de3b70a3a092c633b934262bd510d41aa`, sha256
  `0f13a8e5512439aa6385bbe65c20ceb6a77fe6c819216ef82564f7b8880425e5`
- after the build (`source_after`): the same HEAD, tree, detached state, empty index difference, no other tracked
  difference, and the same mutant blob and sha256. Its verdict reads `NOT_THE_MUTANT` only because
  `unexpected_status` holds 2,549 entries, every one an ignored file `!! target/<nested path>` under the worktree-local
  target directory.

## The false-negative cause

The frozen `tools/verify_mutant.py` and the restoration predicate of `tools/control.py` listed status with
`git status --porcelain --ignored --untracked-files=all`. With `--untracked-files=all`, git lists each ignored file
inside an ignored directory (`!! target/<nested path>`); both predicates allowed only the collapsed exact string
`!! target/`. The byte-level source checks themselves succeeded. Only the target-directory status representation
failed. The defect is repaired by this mission (sections 5-7) before any further control.

## The independently verified exact restoration

`receipts/31-C22/05-restore/RESTORE.json`: `git apply -R --check` rc 0 and `git apply -R` rc 0; `protocol.rs` went
from `694b744de3b70a3a092c633b934262bd510d41aa` / `0f13a8e5…25e5` to `a2df2841b99cad5d452008a6d4150e9369d07e66` /
`a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296` (Candidate C's); Stage 1's `verify_tree.py`
reported `EXACTLY_C`. The Architect independently verified the restoration to Candidate C. This mission re-checks the
preserved worktree `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C22` read-only with the repaired restored verifier (tool
control 5).

## The linked healthy type-(e4) build

`receipts/20-healthy-build/` of the previous packet: `CLASSIFICATION: PASS — admissible healthy-build type-(e4)
success (exact command, Candidate C)`; the identical command, environment template, toolchain, build inputs and
resource limits as the C22 mutant build; source `EXACTLY_C` before and after; executable
`nexus_governed_control-2faa0b9718d3e6a3` recorded and never run.

## The Architect decision

**C22 — MUTATION CONTROL KILL ACCEPTED** (receipt type (d); restored side: the exact-command healthy build, type
(e4)). Decision of the Sole Architect. Claude Code did not independently accept C22.
