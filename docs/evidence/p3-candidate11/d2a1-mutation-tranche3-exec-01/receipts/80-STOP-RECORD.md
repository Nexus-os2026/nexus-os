# Stop record — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-EXEC-01

**MUTATION_CONTROL_BLOCKED at C24** (the second control in the mission's order; the first unsuccessful one).

Written by the executor (Claude Code, Claude Opus 5.5, `claude-opus-5-5`, effort High) after the stop. A record of what happened; no ruling, no acceptance and no repair.

## What stopped

| Item | Value |
|---|---|
| Control | C24: resolve empties U for a present gating record without placing the correction (F-1, `ledger::protocol::model_tests::full_f1`, required property P8, ceiling 60,000,000) |
| Receipts | `receipts/67-C24/` (gate, worktree, apply, build, restore, RESULT; no run, no rules, no link) |
| Apply | `APPLY: EXACTLY_C_PLUS_PATCH`: the frozen patch (sha256 `39439c3c17a3b90d31ec3bec1f3b80e753ab3d042cc59ea9848c748872580e44`) gave exactly the catalog's mutant (`protocol.rs` blob `3e1bfbe746851b6c63811343cde9dd3b83ad0fa9`, sha256 `d4732d83f924d824752d62d59a28e12cc97a456669f7cbea0868d7e6e9742268`); only that file changed |
| Build | the exact command, once, in the fresh worktree-local target; exit 0, signal None, no OOM (0), no timeout (167.92 s of 900 s; scope peak 2,603,855,872 B; swap peak 0 B); cargo's `Finished` line present |
| Build classification | `BUILD: NOT OK`, 17 of 18 checks hold; the one failed check is the frozen classifier's `no error or warning diagnostic` (classify.py, byte-identical to the accepted tooling's) |
| Executable | cargo wrote one library-test executable (`/home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C24/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `8a81956b1105d4639a804cdbdc51555b7b3416bfcec86a1be223dec2c9fa7b1d`, size 8,987,680); it was not pinned and never run (an unsuccessful build kills nothing) |
| Run | not made: an unsuccessful build kills nothing and stops the mission (no rebuild) |
| Restoration | `RESTORE: EXACTLY_C` (`git apply -R` of the same patch; `protocol.rs` back to blob `a2df2841b99cad5d452008a6d4150e9369d07e66`; verify_tree.py EXACTLY_C over all 5,364 tracked blobs and modes; the path-aware status policy holds: ignored entries only beneath the worktree's own target) |
| Control result | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |

## The warning (receipts/67-C24/03-build/build.stderr, verbatim)

```
warning: variant `Corrected` is never constructed
   --> crates/nexus-governed-control/src/ledger/protocol.rs:739:5
    |
737 | pub(super) enum Resolve {
    |                 ------- variant in this enum
738 |     Absent,
739 |     Corrected(CellToken),
    |     ^^^^^^^^^
    |
    = note: `Resolve` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

```

cargo's summary: `warning: `nexus-governed-control` (lib test) generated 1 warning`. No error diagnostic.

## It follows from the frozen patch itself

At Candidate C, `Resolve::Corrected` is constructed in exactly one place: `protocol.rs` line(s) [1786] (git object `a2df2841b99cad5d452008a6d4150e9369d07e66`). The frozen C24 patch (catalog `patches/C24.patch`, carried forward unchanged; replacement `let _ = (row, exit); Resolve::Absent`) removes exactly that branch, so the variant is no longer constructed in the library-test build and rustc's default `dead_code` lint reports it. The catalog records no execution-dependent uncertainty for C24 (its definition is carried forward; `syntax_check`: "not required by the mission for carried-forward controls").

## Why this is a stop

Mission section 6: every new mutant build requires zero compiler errors and zero compiler warnings; "No warning exception from C5 or C28 transfers here. If a frozen patch itself causes a warning: STOP and preserve it for Architect review. No rebuild." Section 12 stops all later controls on a build error or warning. No retry, no second build, no alternate patch and no warning exception was applied.

## State at the stop

- Completed before the stop: C2 — `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING` (FAIL P4 at 361,534 discovered states of 60,000,000; restored; restored side linked).
- Not attempted (no worktree, no receipt): C3b, C3c, C6, C7 (and therefore C7's informational F-3 run), A1, A2, C20, A5.
- Preserved for Architect review: the worktrees `/home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C2` and `/home/nexus/NEXUS/nexus-os-p3-c11-mt3x-C24` (each exactly C, detached, locked with this mission's reason) and their targets, including C24's unpinned executable above.
- Not executed: C1, C4, C25 (reserved); the 22 accepted controls; A4; M01–M183; any unchanged-core configuration.
- The campaign count is unchanged: 22 of 35 Architect-accepted.
