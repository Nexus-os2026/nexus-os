# P3-C11-DR01-D2A1-MUTATION-TRANCHE1-CONTINUE-02 — evidence packet: publication recovery index

Published by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04 (publication recovery only). Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), effort High; the mission named Claude Sonnet 5.5 (`claude-sonnet-5-5`), a deviation recorded here. No mutation control, baseline, build, test, explorer, witness or executable was run by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03 or by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04: only Python publication tools, read-only git and the GitHub CLI.

This file indexes how the already-produced CONTINUE-02 execution evidence came to be published. `INDEX.md` beside it is CONTINUE-02's own index, rendered by its frozen `render.py` from its receipts.

## Status

- C28, C12, C9, C11, C17, C18a, C18b: executor candidates (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`), pending independent Architect review. Nothing here accepts any of them.
- Architect-accepted before this packet (`ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md`, sha256 `cf9bf865e01c8f8b359916eae75f29147132ff67465f93b1c08473c0a8b0d776`): C22, C14, C19, C21, C27. C28's build: Architect-admissible under a narrow ruling (`ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md`, sha256 `870972bbc991b759d00ecaa379dd8f96fca17ff0ad7b3ff11425fc254cdd1d7b`).

## Layers

| Path | Origin | What |
|---|---|---|
| `INDEX.md`, `TRANCHE-SUMMARY.md`, `CARRIED-FORWARD.md`, the two `ARCHITECT-RULING-*.md`, `receipts/`, `tools/`, `development/`, `SHA256SUMS` | CONTINUE-02 root, staged by the R2 builder | the complete CONTINUE-02 execution evidence, byte for byte; the same files the frozen CONTINUE-02 builder staged, plus its `receipts/80-STOP-RECORD.md` (`SHA256SUMS` rewritten over the whole packet) |
| `publication/10-build-packet.txt` | CONTINUE-02 root | the original packet failure (the frozen builder's `PACKET: FAILED`, seven link receipts) |
| `publication-recovery/publish-03/` | PUBLISH-03 root (without `scratch/`) | ruling R1 (seven link receipts), its map, the first repaired builder and its diff, authentication, repair controls with the failed P3 result, the 14-duplicate and zero-length inventory, the source-evidence manifests, its stop record, development history and root SHA256SUMS |
| `publication-recovery/publish-04/` | this mission's root (without `scratch/`) | ruling R2, the 21-row and 8-row maps, the R2 builder and its three diffs, repair controls, manifests of both read-only roots, worktree and preflight receipts, freeze, final gate, development history |
| `INDEX-PUBLICATION-RECOVERY.md` | this mission | this file |

## Key records

| Record | Path |
|---|---|
| Architect ruling R1 — duplicate link receipts (sha256 `e826ff824c00660c…`) | `publication-recovery/publish-03/documents/ARCHITECT-RULING-DUPLICATE-LINK-RECEIPTS.md`, map `publication-recovery/publish-03/documents/DUPLICATE-CONTENT-MAP.tsv` |
| Architect ruling R2 — duplicate policy (sha256 `a8c96b07b55eeedb…`) | `publication-recovery/publish-04/documents/ARCHITECT-RULING-DUPLICATE-POLICY-R2.md` |
| Nonempty duplicate map, exactly 21 rows (sha256 `7882f99f253c5519…`) | `publication-recovery/publish-04/documents/NONEMPTY-DUPLICATE-MAP.tsv` and `.md` |
| Zero-length artifact map, exactly 8 rows (sha256 `ff0beb691bb83b50…`) | `publication-recovery/publish-04/documents/ZERO-LENGTH-ARTIFACT-MAP.tsv` and `.md` |
| The frozen failed builder (CONTINUE-02) | `tools/build_packet.py` |
| The first repaired builder (PUBLISH-03) and its diff | `publication-recovery/publish-03/tools/build_packet.py`, `publication-recovery/publish-03/receipts/03-BUILDER-DIFF/BUILDER.diff` |
| The R2 builder (PUBLISH-04) and its three diffs (frozen → PUBLISH-03, PUBLISH-03 → PUBLISH-04, frozen → PUBLISH-04), with the mechanical only-the-policy check | `publication-recovery/publish-04/tools/build_packet.py`, `publication-recovery/publish-04/receipts/03-BUILDER-DIFF/`, `publication-recovery/publish-04/receipts/03-BUILDER-DIFF.console.txt` |
| Repair controls R2 (P1–P4, N1–N26) | `publication-recovery/publish-04/receipts/04-REPAIR-CONTROLS/CONTROLS.json`, `publication-recovery/publish-04/receipts/04-REPAIR-CONTROLS.console.txt` |
| Repair controls R1 (29 of 30; P3 failed) | `publication-recovery/publish-03/receipts/04-REPAIR-CONTROLS/`, `publication-recovery/publish-03/receipts/04-REPAIR-CONTROLS.console.txt` |
| Manifests of both read-only roots, before (and after: post-gate) | `publication-recovery/publish-04/receipts/00-ROOT-MANIFESTS-BEFORE/`, `publication-recovery/publish-04/receipts/21-ROOT-MANIFESTS-AFTER/`; PUBLISH-03's `publication-recovery/publish-03/receipts/00-SOURCE-MANIFEST-BEFORE.tsv`, `05-SOURCE-MANIFEST-AFTER.tsv` |
| Authentication of every input | `publication-recovery/publish-04/receipts/01-AUTHENTICATION.txt` |
| Worktree and publication preflight | `publication-recovery/publish-04/receipts/05-WORKTREE.txt`, `publication-recovery/publish-04/receipts/06-PUBLICATION-PREFLIGHT.txt` |
| Packet assembly, freeze, final publication gate (post-gate records) | `publication-recovery/publish-04/receipts/07-ASSEMBLE.console.txt`, `10-FREEZE.json`, `10-FREEZE-RECORD.txt`, `20-FINAL-GATE.txt` |

## Preserved failures

- CONTINUE-02: the first final-gate run's crash (`receipts/90-final-gate-attempt-1-CRASHED.txt`) and the successful unchanged re-run (`receipts/90-final-gate.txt`); the frozen builder (`tools/build_packet.py`) and its duplicate failure (`publication/10-build-packet.txt`); `receipts/80-STOP-RECORD.md`; `development/DEVELOPMENT-LOG.txt`.
- PUBLISH-03: control P3 failed (`publication-recovery/publish-03/receipts/04-REPAIR-CONTROLS/P3-builder-run.txt`), the exact 14-duplicate and eight-zero-length inventory (`publication-recovery/publish-03/receipts/07-DUPLICATE-INVENTORY.tsv`), its stop record (`publication-recovery/publish-03/receipts/80-STOP-RECORD.md`), its development history and revisions, a superseded receipt pair (`publication-recovery/publish-03/development/superseded-1-rc-misrecorded/`).
- PUBLISH-04: every development failure and revision (`publication-recovery/publish-04/development/`).

## Exclusions (nothing else is left out)

- `scratch/` of the PUBLISH-03 root and of this mission's root (control stagings; not evidence).
- `publication-recovery/publish-03/development/make-map-dev1-FAILED-sha-column-key.txt`: zero-length (the empty sha256). Ruling R2 permits exactly eight zero-length packet artifacts, so it is not staged; it remains in the PUBLISH-03 root, listed in `publication-recovery/publish-03/receipts/90-ROOT-SHA256SUMS.txt` and in `publication-recovery/publish-04/receipts/00-ROOT-MANIFESTS-BEFORE/publish-03.tsv`. PUBLISH-03's development log, entry 6, describes the failure it recorded.
- Previous packets (EXEC-01, RESUME-01) are referenced by commit, path, blob and sha256 (`CARRIED-FORWARD.md`, the maps), never copied.

## Limitations

- Every classification here is an executor candidate for Architect review, never an acceptance.
- The duplicate and zero-length policy is a publication rule over files and their recorded provenance; it is not a claim about the mutants, the tests or any OS containment.
- Publication receipts written after the commit (push, remote readback, Actions readback) cannot be inside the commit they describe; they are kept in this mission's root and reported to the Architect.
