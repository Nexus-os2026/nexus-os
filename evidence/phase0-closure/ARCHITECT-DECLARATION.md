# Architect declaration: Phase Zero

| Item | Value |
|---|---|
| Date | 2026-09-29 |
| Declared by | the ChatGPT Architect |
| Declaration | **PHASE ZERO COMPLETE — LINUX SUPPORT PROFILE** |
| Source checkpoint commit | `f727f5c39fab8d5c729a55eb28ad576d3d56ce47` |
| Source checkpoint tree | `8106ec232a494a3e9b857c7986f85eee3a7888b9` |

This file records the declaration. The evidence it was based on is the
pre-declaration package `PHASE-ZERO-COMPLETION-RECORD.md` (evidence commit
`6b41c8a1551e6a65ed00c848e8b70c5d038f2d34`), which is kept unchanged. This
file is a record of the declaration, not approval authority in itself.

## Basis

The Architect issued the declaration only after independently reviewing:

- the R2C technical closure (`repair/p0-linux-final-closure-r2` at
  `cae7bb52`);
- the documentation truth repair (P0-DOC-TRUTH-02R1, `f727f5c3`);
- Fast Local #14 (run `36622280925`) on `f727f5c3`;
- Hosted Final Gate #111 (run `36624858376`, `ci.yml`, `workflow_dispatch`,
  attempt 1, success) on `f727f5c3`;
- the authoritative fast-forward of `rebuild/phase0-trust-boundary`,
  `71c47acb` → `f727f5c3`;
- the public `main` fast-forward, `80640bba` → `f727f5c3`;
- main CI #112 (run `36636434205`, `push` on `main`, attempt 1, success);
- Security Audit #4 (run `36636434242`, `push` on `main`, attempt 1,
  success);
- the public metadata and release truth alignment (description, topics and
  the three "[HISTORICAL]" releases);
- the completion record at evidence commit `6b41c8a1…`.

## Scope and non-claims

- Linux only.
- Windows and macOS portability validation is deferred.
- Not a production-readiness declaration.
- Not an external certification or attestation.
- No universal OS or WebAssembly sandbox claim.
- Accepted dependency exceptions remain (the reviewed set in `deny.toml`,
  including the RUSTSEC-2026-0316 Wasmtime non-use exception). A green
  dependency gate does not mean zero dependency debt.
- No Phase One capability is implied.
- Standalone server deployment remains withdrawn.
- The limitations and non-claims recorded in `PHASE-ZERO-COMPLETION-RECORD.md`
  (section M) and in the Final-Gate dossier continue to apply.

## Frozen checkpoint

`rebuild/phase0-trust-boundary` remains the frozen, exact Phase Zero
engineering checkpoint at `f727f5c39fab8d5c729a55eb28ad576d3d56ce47`.
Later documentation or status commits (for example the post-completion
status update on `repair/p0-post-complete-status`) do not redefine those
validated bytes. No version, tag or release was created for this
declaration.
