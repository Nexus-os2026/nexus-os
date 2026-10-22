# Test, harness and guard changes

## Added

| Test | Target | Proves |
|---|---|---|
| `q3r1_01_a_never_populated_scope_stays_loaded_and_unconfirmed_before_its_backstop` | lib | A, B, E: a killed, unreaped helper's placement is refused (`NotPlaced`), its accepted operation retained without a candidate; no cgroup-empty notification ends the never-populated scope and a running unit is never collected; three retries stay unconfirmed, the helper unreaped (`waitid WNOWAIT`), nothing opened or killed by the backend |
| `q3r1_02_only_the_backstop_and_the_collection_let_a_retry_confirm_it` | lib | C, D, E: the backstop ends it ('timeout'), still loaded, a retry still unconfirmed and the helper unreaped; once collected, a retry confirms and only then reaps the helper |
| `q3r1_03_a_populated_scope_that_runs_empty_still_ends_on_its_notification` | lib | F: a populated scope does not end while populated, ends on its notification once it ran empty, and is collected; the operation then confirms |
| `np_01`-`np_15` | `tests/phase2_never_populated.rs` (new) | case 18's procedure over a model of the real lifecycle in virtual time, and over each deviation it must report (a confirmation before the backstop; a collection before it; a proven placement; a cleanup confirmed at once; an early reap; a helper that never exits; a late exit, waited for by its report only; never collected; scopes left; unconfirmed after the collection; an unobservable manager); the failure report's fields; the case's own backstop and plan |
| `np_16`, `np_17` | same | on real child processes of the test: the exit observation never reaps (the child stays a waitable zombie until reaped here, then `ECHILD`) and never blocks on a running child |
| `np_18` | same | structural: live case 18 drives exactly this qualification through production's placement and retry, with its own backstop and production's bounds, and reaps, sleeps, waits, observes and retries nothing itself; the support has one exit observation (`WEXITED \| WNOHANG \| WNOWAIT`) and acts on no unit or process otherwise; every wait of the qualification is bounded |

## Changed

| File | Change |
|---|---|
| `src/scope/tests.rs` | the unit lifecycle (`lifecycle-model.md`); `Interference::Replace` and one test's unload through the lifecycle |
| `src/execution/tests.rs` | `unloaded()` and three direct unloads through the lifecycle; `i4_05`'s comment names the refused-start lifecycle it models; the three new tests |
| `tests/phase2_live_sandbox.rs` | case 18 (`case-18.md`); the `never_populated` support module included (x86_64 Linux only, as the others); `wait_for` no longer imported by `p2d` (only case 18 used it there) |
| `tests/support/never_populated.rs` | new: the qualification, its plan and the case's limits, the exit observation |
| `app/src-tauri/src/phase2_tests.rs` | `p2_g_09`'s case-18 pins only (below) |
| `docs/security/phase2-governed-verification.md` | the never-populated lifecycle (section 9, systemd), the test table, the residual on accepted operations without a candidate |

Unchanged: the live suite's count (39) and every case name, the workflow,
its step's fixture controls, the probe, the checked observation
(`support/cleanup_observation.rs`) and every production source.

## The desktop guard `p2_g_09`: what moved, and why

| Pin | Before | Now | Why |
|---|---|---|---|
| `loaded_scopes()` in the harness | 5 | 4 | case 18 no longer observes itself; its world observes through `loaded_scopes_by` |
| `.unwrap_or_else(\|error\| panic!(` | 5 | 4 | case 18's own baseline observation is gone (the qualification reports a failed observation as a failure, never as "no scopes") |
| `wait_for(Duration::from_secs(10), loaded_scopes_by, \|now\| {` | 3 | 2 | case 18's own wait is gone (the qualification's waits are bounded: `np_08`, `np_10`, `np_18`) |
| the exact `settle(boundary, EXPLICIT_ATTEMPTS, execution::RetainedBoundary::retry)` form | 2 | 1 | case 18 retries through the qualification instead |
| added | | | the harness drives `never_populated::qualify(&mut world, &plan)` once and retries through `execution::RetainedBoundary::retry(boundary)` once; it releases explicitly; the support keeps each failed retry's owner (`Err(boundary) => boundary,` twice, `world.retry(boundary)` twice), releases through `world.release(boundary, &failure.to_string())` and names no unit action, `waitpid` or `Command` |

`RetainedBoundary::retry` (4) and every other pin of `p2_g_01`-`p2_g_11` are
unchanged; the desktop's 11 guards pass.
