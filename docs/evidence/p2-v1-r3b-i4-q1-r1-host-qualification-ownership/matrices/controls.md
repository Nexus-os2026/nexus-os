# P2-V1-R3B-I4-Q1-R1: negative controls (mission sections 10 and 11)

The repaired H6 has two layers that can carry its ownership, and every
pattern of the finding is restored, in a scratch copy, in each layer where
it can occur:

- **the first start's owner**, `execution::place` (production code, which
  H6 now delegates to): exercised over the deterministic model by the new
  unit tests in `src/scope/tests.rs`;
- **the live harness** (`p2q`) and **its probe**: they run only on the
  supported host (absent here), so the desktop guard `p2_g_11` pins them.

## Targeted tests (all pass on the candidate)

| Control | Test | What it requires |
|---|---|---|
| Q1-R1-NC1 first-call foreign collision | `scope::tests::i4q1r1_nc1_a_first_start_refused_as_loaded_ends_only_the_harness_s_helper` | first start answered exactly `UnitExists` (the model's collision), the foreign cgroup holding the helper or not: no `StopUnit` at all, the foreign unit still loaded and foreign; no `cgroup.kill` of anything; nothing opened, read or proven for it (no claim); the harness's own helper ended, reaped only once outside; every observation of it while unreaped |
| Q1-R1-NC2 uncertain first start | `scope::tests::i4q1r1_nc2_an_uncertain_first_start_keeps_its_helper_unreaped_and_unconfirmed` | timeout, broken connection, unexpected error, undecodable reply, with and without an effect: never confirmed (not by a delivered, effective `StopUnit`, not by the manager's absence); the operation retained with its helper, killed but unreaped, across a retry |
| Q1-R1-NC3 panic after dispatch | `scope::tests::i4q1r1_nc3_a_panic_after_the_first_start_is_dispatched_keeps_one_owner` | a panic inside the start after its effect, while waiting for placement, or while retaining the candidate: never crosses `place` (a last owner always exists), never splits the operation from its helper, never reaps an unresolved helper; confirmation only with nothing left |
| Q1-R1-NC4 successful owned creation | `scope::tests::i4q1r1_nc4_a_proven_first_start_is_released_by_its_owner_and_observed` | the proven scope released as `released` does: `cgroup.kill` of exactly the created unit's cgroup through its descriptor, the helper reaped only then, emptiness observed, the owner's end confirmed; no `StopUnit`; nothing else touched; a scope that stays populated is never confirmed |
| Q1-R1-NC5 a name never authorizes a stop | `phase2_tests::p2_g_11_the_host_qualification_owns_no_native_effect_by_a_unit_name` | the probe calls only `GetUnit`, `Properties.Get` and the start without processes (no `StopUnit` or other acting method, no process, no property); the qualification cases have no stop path, no owner type, no raw helper kill or reap, one helper spawn, handed to `execution::place`; H6 sends its one request only for the proven unit, before its owner's release; a generated name is only ever read (H5); `released` ends the scope before it reaps the helper; nothing changes the environment |
| H1 (section 11) | `scope::tests::i4q1r1_connect_derives_the_bus_from_the_real_uid_alone`, `p2_g_11` | `connect()` derives its bus from `getuid` alone and connects by explicit address; the scope module reads no environment and names no ambient bus; the qualification cases change nothing process-wide |

## Mutation controls (`scripts/r1_controls.py`, `controls/r1/`)

Each applies its edits in an isolated clean checkout of the candidate,
compiles, runs its one intended test alone, which must fail at its marker,
restores every file byte for byte and checks the checkout's Git status.

| ID | Layer | File | Restores | Intended test | Marker |
|---|---|---|---|---|---|
| Q1-R1-NC1-FOREIGN-STOPPED | first-start owner | `src/scope/pending.rs` | the collided unit stopped by its name | `i4q1r1_nc1` | the foreign unit was stopped |
| Q1-R1-NC1-FOREIGN-KILLED | first-start owner | `src/scope/pending.rs` | the collided unit's cgroup taken and killed | `i4q1r1_nc1` | the foreign unit was killed |
| Q1-R1-NC1-H6-UNPROVEN-UNIT | live harness | `tests/phase2_live_sandbox.rs` | H6's request for a name it generated | `p2_g_11` | H6 acts on a unit it has not proven |
| Q1-R1-NC2-UNRESOLVED-REAPED | first-start owner | `src/execution.rs` | the helper reaped when its operation cannot be confirmed | `i4q1r1_nc2` | the helper was reaped while unresolved |
| Q1-R1-NC2-STOP-REPLY-PROOF | first-start owner | `src/scope/pending.rs` | a delivered `StopUnit` reply taken as confirmation | `i4q1r1_nc2` | confirmed without proof |
| Q1-R1-NC2-HARNESS-RAW-REAP | live harness | `tests/phase2_live_sandbox.rs` | a helper killed and reaped outside its owner | `p2_g_11` | the qualification ends a helper outside its owner |
| Q1-R1-NC3-PANIC-ESCAPES | first-start owner | `src/execution.rs` | a panic in the first start crossing its owner | `i4q1r1_nc3` | a panic crossed the owner of the first start |
| Q1-R1-NC3-PANIC-SPLITS | first-start owner | `src/execution.rs` | a panicked start's helper reaped and split from its operation | `i4q1r1_nc3` | the operation was split from its helper |
| Q1-R1-NC3-HARNESS-OWN-OWNER | live harness | `tests/phase2_live_sandbox.rs` | an owner type of its own (old `Existing`) | `p2_g_11` | the qualification owns a native effect beside the production owner |
| Q1-R1-NC4-CLEANUP-UNOBSERVED | first-start owner | `src/execution.rs` | a proven scope confirmed without observing it empty | `i4q1r1_nc4` | never observed empty |
| Q1-R1-NC4-HARNESS-REAP-BEFORE-KILL | live harness | `tests/phase2_live_sandbox.rs` | `released` reaping before the scope is ended | `p2_g_11` | released() reaps the helper before its scope is ended |
| Q1-R1-NC5-PROBE-STOPS-BY-NAME | live harness | `tests/support/host_qualification.rs` | a probe `StopUnit` by the unit's name | `p2_g_11` | the probe acts on a unit by its name |
| Q1-R1-NC5-PROBE-START-CARRIES-A-PROCESS | live harness | `tests/support/host_qualification.rs` | the start carrying `PIDs` | `p2_g_11` | the probe's start request carries a process or a property |
| Q1-R1-H1-CONNECT-ENV | production source | `src/scope.rs` | `connect()` following `DBUS_SESSION_BUS_ADDRESS` | `i4q1r1_connect` | reads the environment or names an ambient bus |
| Q1-R1-H1-ENV-MUTATION | live harness | `tests/phase2_live_sandbox.rs` | Q1's `set_var` in H1 | `p2_g_11` | the qualification changes the process environment |

Production files are mutated only in these scratch checkouts; the
candidate changes no production file.

## Kept and rerun unchanged

- The ten Q1 controls (`docs/evidence/p2-v1-r3b-i4-q1-host-live-qualification/scripts/q1_controls.py`,
  unchanged; `controls/q1/`).
- The accepted suites (Q1's `accepted_reruns.sh`, unchanged;
  `controls/accepted/`): the 38 I4-R1 behavioural controls, the 27
  harness-build and 9 normal-build API guards, the 14 source guards, the
  custody store's pinned hashes, R3's 179 store controls, the I2-R1 rerun
  and R3's API probes.
