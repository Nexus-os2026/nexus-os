# Stop record — P3-C11-DR01-D2A1-MUTATION-TRANCHE2-EXEC-01

**MUTATION_CONTROL_BLOCKED at C5** (the sixth control in the mission's order; the first unsuccessful one).

Written by the executor (Claude Code, Claude Opus 5.5, `claude-opus-5-5`, effort High) after the stop. A record of what
happened; no ruling, no acceptance and no repair.

## What stopped

| Item | Value |
|---|---|
| Control | C5: resolve, for an acknowledgement in U, raises the frontier with neither the presence read nor the frontier check |
| Receipts | `receipts/56-C5/` (gate, worktree, apply, build, restore, RESULT; no run, no link) |
| Apply | `APPLY: EXACTLY_C_PLUS_PATCH`: the frozen patch (sha256 `a4776885b669bd912fd8710d5a3283f4c1863ab9213e8474d26707b5a6e5f3d8`) gave exactly the catalog's mutant (`protocol.rs` blob `e1a49f5ba952bc81cdac9f8f8ace9eeb2aa93495`) |
| Build | the exact command, once; exit 0, no signal, no OOM, no timeout (168.31 s of 900 s; scope peak 2,615,914,496 B; zero swap); cargo's `Finished` line present |
| Build classification | `BUILD: NOT OK`, 17 of 18 checks hold; the one failed check is the frozen classifier's `no error or warning diagnostic` |
| Run | not made: an unsuccessful build kills nothing and stops the mission (no rebuild) |
| Restoration | `RESTORE: EXACTLY_C` (`git apply -R` of the same patch; `protocol.rs` back to blob `a2df2841b99cad5d452008a6d4150e9369d07e66`; verify_tree.py EXACTLY_C; the path-aware status policy holds) |
| Control result | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |

## The five warnings (receipts/56-C5/03-build/build.stderr, verbatim first lines)

- `warning: unused variable: `nonce`` at `crates/nexus-governed-control/src/ledger/protocol.rs:1818:47`
- `warning: unused variable: `io`` at `crates/nexus-governed-control/src/ledger/protocol.rs:1818:65`
- `warning: methods `acknowledgement` and `frontier` are never used`
- `warning: variant `Inconsistent` is never constructed`
- `warning: variants `Acknowledgement` and `Frontier` are never constructed`
- cargo's summary: `warning: `nexus-governed-control` (lib test) generated 5 warnings`

Every one follows from the frozen C5 patch itself: the patch replaces `resolve_ack`'s body and keeps its signature byte
for byte (catalog preserved_behaviour: "`nonce` and `io` stay, unused"), so the reads it removed leave parameters,
methods and variants unused. The frozen catalog states this as an execution-dependent uncertainty of C5: "`nonce` and
`io` become unused parameters (rustc unused_variables warnings, not errors under C's lint configuration);
`Resolve::Inconsistent` and `LatchCause::Inconsistent` may lose their only production constructions (dead_code
warnings)."

## Why this is a stop

Mission section 6: "No C28-style warning exception applies. Every Tranche-2 mutant build requires zero error diagnostics
and zero warning diagnostics." Section 9 requires "no warning diagnostic"; section 14 stops all later controls on a
"build error or warning", with no retry, no alternate patch, no second build and no continuation after a blocked
control. The frozen build classifier (classify_build_success, unchanged from Tranche 1) applied exactly that rule.

## State after the stop

- Executor candidates of this tranche (`KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`): C23, C10, C16, C26, C3a.
- Blocked: C5 (build warnings; kills nothing).
- Not attempted (no worktree, no receipt): A3, A6, C8, C15.
- Cargo wrote a mutant library-test executable into the C5 worktree's own target directory as part of the build; it
  was not pinned and never run. The worktree and its target are kept for review; the worktree is exactly C.
- Architect-accepted kills: 12 of 35, unchanged. Nothing here is accepted by the executor.

## The exact next blocker

Whether C5's warning-emitting build may be admitted, or C5's frozen patch revised, is an Architect decision. The
executor changed no patch, classifier, rule or ceiling and did not rebuild; A3, A6, C8 and C15 wait for an explicit
mission.
