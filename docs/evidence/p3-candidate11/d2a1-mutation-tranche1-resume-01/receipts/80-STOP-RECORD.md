# Stop record — control C28 (resume control 5 of 11): mutant build classified BUILD: NOT OK

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01. Status: **MUTATION_CONTROL_BLOCKED**. Written after the stop,
from the receipts, without changing any frozen tool, receipt or classification. Nothing was rebuilt, rerun, retried or
repaired; no alternate patch; no later control was executed.

## What the frozen receipts say

| Receipt | First line |
|---|---|
| `receipts/36-C28/00-GATE.txt` | `GATE: OPEN` (C27 a candidate; the freeze holds) |
| `receipts/36-C28/02-apply/APPLY.txt` | `APPLY: EXACTLY_C_PLUS_PATCH` (patch sha256 `519378de…3868`; mutant blob `446d0365e1472efc40321d259a6f1171d3fe716c`, sha256 `1b59477e…77d8`) |
| `receipts/36-C28/03-build/CLASSIFICATION.txt` | `BUILD: NOT OK` — outer class `BUILD NOT OK (no error or warning diagnostic): kills nothing; the mission stops`; 17 of 18 checks hold |
| `receipts/36-C28/05-restore/RESTORE.txt` | `RESTORE: EXACTLY_C` |
| `receipts/36-C28/RESULT.txt` | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |

These classifications stand. This record does not turn any of them into a success, and the executor proposes no
alternative classification.

## The raw facts (read from the same receipts)

- The exact command `cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2`, the exact
  environment, a fresh worktree-local target, 16 GiB, zero swap, 900 s: exit 0 by a normal exit, no signal, no OOM, no
  timeout; 166.4 s; scope memory peak 2,611,777,536 B; swap peak 0 (`03-build/PARSED-RECEIPT.json`).
- Source `EXACTLY_C_PLUS_PATCH` before and after the build (the repaired verifier; 2,553 ignored entries beneath the
  worktree-local real target after the build, nothing else).
- The only failed check: "no error or warning diagnostic". `03-build/build.stderr` lines 640-651:

  ```
  warning: unreachable pattern
      --> crates/nexus-governed-control/src/ledger/protocol.rs:1834:17
       |
  1826 |                 Read::Got(_) => {
       |                 ------------ matches all the relevant values
  ...
  1834 |                 Read::Got(_) => {
       |                 ^^^^^^^^^^^^ no value can reach this
       |
       = note: `#[warn(unreachable_patterns)]` (part of `#[warn(unused)]`) on by default

  warning: `nexus-governed-control` (lib test) generated 1 warning
  ```

  The warning is in the mutated file: the frozen C28 patch removes resolve's "present, frontier below N" check, which
  leaves a later `Read::Got(_)` arm unreachable.
- cargo named exactly one library-test executable, `nexus_governed_control-2faa0b9718d3e6a3` in the worktree's
  release/deps, sha256 `116a6584ebe58c1fea17e2067c6f2af3f3c365cb64f595648040f5c797a5de36`, 8,983,776 bytes; profile
  flags, cfg and fingerprint checks hold. It was **not pinned and never run** (no `PIN.json`, no `04-run/`).
- Restoration: `git apply -R` rc 0; protocol.rs back to `a2df2841…` / `a5454944…a296`; verify_tree.py EXACTLY_C; the
  path-aware status policy holds (2,553 ignored entries beneath the real target, nothing rejected).

## Why the mission stopped

The frozen classifier (`classify.classify_build_success`, kind `pin`, unchanged from EXEC-01 and not modifiable in
this mission) requires a mutant build with no error or warning diagnostic before its executable may be pinned. It
classified the C28 build `BUILD: NOT OK`, which kills nothing and stops the mission (resume sections 12 and 17: build
failure; no retry, no second build). The frozen gate then admits no later control.

Not attempted: C12, C9, C11, C17, C18a, C18b.

## State preserved for review

- `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C28`: exactly C now (restored), locked with this mission's reason, its
  target directory and the unpinned C28 executable kept.
- `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C14`, `-C19`, `-C21`, `-C27`: exactly C, locked, targets and pinned mutant
  executables kept.
- The C28 mutant blob (like every catalog mutant blob) is absent from the repository object store.

## Exact next blocker

An Architect ruling on the admissibility of a mutant build that exits 0 with a mutation-induced compiler warning
(here `unreachable_patterns` at the mutated file's line 1834), i.e. whether the frozen pin rule "no error or warning
diagnostic" is the intended policy for C28 or should be revised; and a new execution mission for C28 (which this
mission may not rebuild or rerun) and the six controls not attempted. The executor has repaired nothing.
