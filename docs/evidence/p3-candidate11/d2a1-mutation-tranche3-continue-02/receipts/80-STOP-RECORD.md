# Stop record — P3-C11-DR01-D2A1-MUTATION-TRANCHE3-CONTINUE-02

**MUTATION_CONTROL_BLOCKED at C7** (the fifth control in the mission's order; the first unsuccessful one).

Written by the executor (Claude Code, Claude Opus 5.5, `claude-opus-5-5`, effort High) after the stop. A record of what happened; no ruling, no acceptance and no repair.

## What stopped

| Item | Value |
|---|---|
| Control | C7: *Explained* removes every table entry, **with** A4 (F-PT, `ledger::protocol::model_tests::full_fpt`, the original explorer, A4 ON, required property P6, ceiling 30,000,000) |
| Receipts | `receipts/85-C7/` (gate, worktree, apply, build, run, tranche-3 rules, restore, RESULT; no informational run, no link) |
| Apply | `APPLY: EXACTLY_C_PLUS_PATCH`: the frozen patch (sha256 `b5d8bc6c9254e0ca2853fcf2f9f220f24782ac6cfa77efe7032365d76f0efbc4`) gave exactly the catalog's mutant (`protocol.rs` blob `0d4ab9929c0e15d910735a52b72a5f3855bef58a`, sha256 `6d2f2d49842cdf5cc155dd1cb0984e121ada154cfc69a412219c7ac12dc846aa`); only that file changed |
| Build | the exact command, once, in the fresh worktree-local target; `BUILD: PINNED` (18 of 18; zero warnings, zero errors; 167.65 s of 900 s; scope peak 2,617,974,784 B; swap peak 0 B) |
| Executable | `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C7/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3`, sha256 `b320977a41d78ad031c74cb0f3fc9fe8c3f89051bbd599cc9b42deebb303a7e5`, size 8,984,592 (unchanged before, at selection and after the run) |
| Run | one supervised invocation: `full_fpt --exact --ignored --nocapture --test-threads=1`; header `EXPLORATION F-PT` (the original explorer; no quotient or symmetry line); printed bounds the frozen F-PT bounds with ` A4 true;`; exit 101 by the explorer's own verdict assertion; no signal, no OOM, swap peak 0, no timeout |
| Raw outcome | `outcome: FAIL`; `FAIL: E11: a table entry left in a step that did not see its explanation in the ledger`; shortest trace 13 steps; 1,071,165 discovered states (expanded 686,654) of the 30,000,000 ceiling; 4.7 s |
| Run classification | `CLASSIFICATION: NOT A KILL EVIDENCE CANDIDATE` — `FAIL of E11, not the named property P6: kills nothing (no attribution exception)`; 27 of 29 checks hold; the two failed checks are the property checks (the assertion's `failed: {"E11"}`; the printed property E11, not P6) |
| Tranche-3 rules | `TRANCHE-3 RULES: NOT SATISFIED` (the one failed rule: the printed first property is not the permitted P6; original explorer, A4 ON, ceiling and bounds all hold) |
| Informational F-3 run | not made: it follows only an admissible F-PT/P6 candidate (mission section 12) |
| Restoration | `RESTORE: EXACTLY_C` (`git apply -R` of the same patch; `protocol.rs` back to blob `a2df2841b99cad5d452008a6d4150e9369d07e66`; verify_tree.py EXACTLY_C; the path-aware status policy holds) |
| Link | not made: a restored side is linked only after a candidate |
| Control result | `CONTROL RESULT: MUTATION_CONTROL_BLOCKED` |

## The catalog's expectation for C7 (frozen R2 catalog, carried forward unchanged)

- `expected_primary_failure.printed_property_must_be`: `P6`; `special_attribution_rules`: "contract 7.5: 'with A4' — run on F-PT (A4 on), original explorer (AD-1-R4 F)" and "AD-1-R4 F (line 748): C7 without A4 on F-3 is an added run, not required to kill"; no attribution exception.
- `additional_comparison` (the run without A4 on F-3, not required to kill): "expected to fail E11".
- The kill run here, with A4 on F-PT, printed E11 first (at 1,071,165 discovered states), not P6.

## Why this is a stop

Mission section 18: stop on a wrong property; section 12: C7 requires P6 on F-PT with A4 ON on the original explorer; the quotient is forbidden for the mutant run. The run is a complete, admissible receipt of the explorer's verdict, and that verdict names E11. No retry, no rerun, no rebuild, no alternate patch, no cap change and no attribution exception was applied.

## State at the stop

- Completed before the stop, each `CONTROL RESULT: KILL_EVIDENCE_CANDIDATE — ARCHITECT REVIEW PENDING`:
  - C24: the preserved Tranche-3 build admitted under the narrow C24 ruling (not rebuilt); FAIL P8 at 185,114 discovered states of 60,000,000; reauthenticated; Stage-1 F-1 linked.
  - C3b: a clean build (no warning; AMENDMENT-01 path A); FAIL P13 at 67,871 of 30,000,000; restored; Stage-2 F-6 linked.
  - C3c: a clean build (no warning; AMENDMENT-01 path A); FAIL P12 at 541,827 of 30,000,000; restored; Stage-2 F-6 linked.
  - C6: zero-warning build; FAIL P6 on F-PT with A4 ON at 2,652,070 of 30,000,000; restored; Stage-1 F-PT composed-quotient PASS linked (restored side only).
- Not attempted (no worktree, no receipt): A1, A2, C20, A5.
- Preserved for Architect review: the worktrees `/home/nexus/NEXUS/nexus-os-p3-c11-mt3c-C3b`, `-C3c`, `-C6` and `-C7` (each exactly C, detached, locked with this mission's reason) and their targets, including C7's pinned executable above; C24's preserved Tranche-3 worktree and executable (read only here; still locked with the Tranche-3 reason).
- Not executed: C1, C4, C25 (reserved); the 23 accepted controls; A4; M01–M183; any unchanged-core configuration.
- The campaign count is unchanged: 23 of 35 Architect-accepted.
