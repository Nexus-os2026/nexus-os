# The model's unit lifecycle (`crates/nexus-verifier-sandbox/src/scope/tests.rs`)

Only enough fidelity to protect the invariant; not a systemd simulator.

## States and events

| Item | Meaning |
|---|---|
| `Cgroup::ever_populated` | a process was ever in it: set when the kernel places one (`State::place`) or when it is created with a process of its own (a phantom) |
| `Cgroup::stopped` | the manager stopped its unit (backstop or another client): what it held is ended; never the backend's doing (`kills`, `killed` stay the backend's `cgroup.kill`) |
| `Lifecycle::Running` | loaded and running; a scope stays so while nothing ends it |
| `Lifecycle::Ended(Ended::Emptied)` | `State::notify_empty`: the cgroup-empty notification, produced only by a cgroup that was populated and has run empty |
| `Lifecycle::Ended(Ended::Backstop)` | `State::expire_backstop`: `RuntimeMaxUSec` expired (only a unit created with a nonzero backstop); stopped, failed 'timeout' |
| `Lifecycle::Ended(Ended::StoppedByAnother)` | `State::stopped_by_another`: another client of the same manager stopped it |
| collected | `State::collect`: only an ended unit (`CollectMode=inactive-or-failed`): its cgroup removed, its members reported ` (deleted)`, the name free |
| `State::end_by_itself_and_collect` | the event that really ends the unit (the notification if its cgroup ran empty after being populated, else its backstop), then the collection |
| `State::unload` | now private: reachable only through `collect` (the compiler found every former caller) |

A start that attached no process at all (its process id gone) is refused and
its failed unit collected at once: the model's existing `unit_loaded = false`,
unchanged (`i4_05`).

## Every former `unload` (a never-populated cgroup can no longer just disappear)

| Call site | Scenario | Now |
|---|---|---|
| `execution/tests.rs` `unloaded()` (16 call sites) | the manager ends and collects the requested unit by itself | `end_by_itself_and_collect`: the notification when its cgroup ran empty after being populated (`i4_08`'s released phantom), otherwise its backstop (the `Placement::Never` cases) |
| `execution/tests.rs` `i4r1_06` | the helper placed beside, the unit's cgroup never populated | `expire_backstop` then `collect` |
| `execution/tests.rs` `reloaded_by_another()` | this request's unit ends by itself, then a foreign one loads the name | `end_by_itself_and_collect` |
| `execution/tests.rs` `r3r1_02` (after the execution) | the same, then the replacement takes the helper | `end_by_itself_and_collect` |
| `scope/tests.rs` `Interference::Replace` | mid-start, the unit running with the helper in it | `stopped_by_another` then `collect` (neither emptiness nor the backstop can end it at that moment) |
| `scope/tests.rs` `i4_a_pending_operation_is_bound_to_its_own_helper` | `Placement::Never`: never populated | `expire_backstop` then `collect` |

Every accepted test keeps its assertions; the 153 accepted unit tests pass
unchanged in outcome (`validation/lib-tests.txt`: 156 = 153 + 3).

## Observation left as it was

`r3r1_02`'s post-execution part has a foreign unit's cgroup take the killed,
unreaped helper. Per the kernel (`cgroup_migrate_add_task`) an exiting task
cannot be moved, so that world is stricter than the real one: a
conservative, authority-oriented model kept unchanged (its subject is that a
foreign cgroup holding the helper is never bound or ended).
