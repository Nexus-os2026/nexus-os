# Stop record — control C22 (control 1 of 12): frozen-tool failure

Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-EXEC-01. Status: **MUTATION_CONTROL_BLOCKED**. Written after the stop,
from the receipts, without changing any frozen tool, receipt or classification. No later control was executed; nothing
was rebuilt, rerun or repaired.

## What the frozen receipts say

| Receipt | First line | Note |
|---|---|---|
| `receipts/31-C22/02-apply/APPLY.txt` | `APPLY: EXACTLY_C_PLUS_PATCH` | frozen patch sha256 `e017ced6…95c6`; mutant blob `694b744de3b70a3a092c633b934262bd510d41aa`, sha256 `0f13a8e5…25e5` |
| `receipts/31-C22/03-build/CLASSIFICATION.txt` | `CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE` | outer class: NOT ADMISSIBLE (source EXACTLY_C_PLUS_PATCH before and after the build); 15 of 16 checks hold |
| `receipts/31-C22/05-restore/RESTORE.txt` | `RESTORE: FAILED` | |
| `receipts/31-C22/RESULT.txt` | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` | |

These classifications stand. This record does not turn either into a success.

## The raw facts behind the two failed checks (read from the same receipts)

1. **Source after the C22 build** (`03-build/descriptor.json`, `source_after`, produced by the frozen
   `tools/verify_mutant.py`): HEAD `d5964ee5…` and tree `51bac5fd…`, detached; nothing staged; no tracked file other than
   `protocol.rs` differs from C (0 content, mode or missing mismatches over 5,364 tracked blobs); `protocol.rs` is
   exactly the catalog mutant (git blob `694b744d…`, sha256 `0f13a8e5…`). The verdict is `NOT_THE_MUTANT` only because
   `unexpected_status` holds 2,549 entries, every one of them an ignored file under the worktree-local `target/`
   (for example `!! target/.rustc_info.json`).
2. **Restoration** (`05-restore/RESTORE.json`): `git apply -R --check` rc 0 and `git apply -R` rc 0; `protocol.rs` went
   from `694b744d…` / `0f13a8e5…` to `a2df2841b99cad5d452008a6d4150e9369d07e66` / `a5454944…a296` (C's); the frozen
   `tools/verify_tree.py` (Stage 1's, unchanged) reports `EXACTLY_C` with status `['!! target/']`. The restore predicate
   failed only on its second status listing, which again lists the 2,549 ignored target files one by one.

## Cause (a defect in this mission's frozen tooling, not in Nexus)

`tools/verify_mutant.py` and `tools/control.py` (`restore`) list status with `git status --porcelain --ignored
--untracked-files=all`. With `--untracked-files=all`, git lists every ignored file inside an ignored directory; without
it (as `verify_tree.py` does), the directory is collapsed to the single line `!! target/`. Both predicates allow only
the collapsed form. Before the freeze, the predicates were exercised only in worktrees without a target directory (the
synthetic patch rehearsal built nothing; the synthetic tool controls supplied source verdicts directly), so the defect
was not observed. It appears in the first worktree that both carries a mutant and has been built.

The same defect would recur in every later control: each builds into its own worktree-local target before the
post-build source check.

## The C22 compiler evidence, as recorded (not a classification)

`03-build/CHECKS.txt`: the build exited 101 by a normal exit (no signal, no OOM, no timeout; 124.7 s of 900 s; scope
peak 2.44 GiB; swap peak 0); exactly one coded error diagnostic, `error[E0283]`, at
`crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs:1089:5`, rendered line
`<Ticket as NotClone<_>>::check();`; the only other error line is cargo's summary for `nexus-governed-control` (lib
test) due to 1 previous error; rustc's own status `exit status: 1`; no compiler crash; no mutant test executable
(NOT PRODUCED BY DESIGN); 2,549 target files recorded as found. Whether this receipt can be admitted despite the
source-check defect is for the Sole Architect; the executor makes no such claim.

## State preserved for review

- `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-C22`: exactly C now (verify_tree.py EXACTLY_C), locked with this mission's
  reason, its target directory kept.
- `/home/nexus/NEXUS/nexus-os-p3-c11-mt1x-healthy`: exactly C, its target and the healthy executable kept (never run).
- The pinned baseline executable is unchanged. The C22 mutant blob is absent from the repository object store.
- Not attempted: C14, C19, C21, C27, C28, C12, C9, C11, C17, C18a, C18b.

## Exact next blocker

An Architect ruling on (a) the C22 receipt's admissibility given the source-check defect, and (b) an authorized
repair of the two status predicates (for example: accept ignored entries only under the worktree's own `target/`, or
list status without `--untracked-files=all` as verify_tree.py does) followed by a new execution mission. The executor
has repaired nothing.
