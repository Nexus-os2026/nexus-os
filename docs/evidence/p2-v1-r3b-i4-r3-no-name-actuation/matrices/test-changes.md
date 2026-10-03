# P2-V1-R3B-I4-R3: test and model changes

The sandbox crate's library tests: 131 at the base, 138 in the candidate
(+5 behavioural `i4r3_*`, +3 structural `i4r3_10_*`, −1 `i4r2_x`, which
they replace). No test was deleted without its invariant being carried by
a stronger one; every renamed test is listed below.

## The model (`src/scope/tests.rs`)

- Removed: `Phantom::UntilStop`, `Op::Stop`, `Call::Stop`, `State::stop`,
  `State::stopped`, `FakeManager::stop_unit`. The model's manager has
  nothing to stop or kill a unit with, as the production one.
- `World::release` no longer lets a stop act: it releases the processes the
  model kept alive and lets every call be answered; units stay loaded.
- Added: `State::unload` (the manager unloads a unit by itself, its cgroup
  removed) and `State::load_foreign` (another client loads a unit of its
  own under the free name, in a new cgroup of the same path, with the same
  limits and a process of its own).

## Renamed tests (their old name described the removed stop)

| Old | New | What it asserts now |
|---|---|---|
| `i4_06_a_failed_stop_of_a_unit_the_helper_never_entered_retains_the_operation` | `i4_06_a_unit_the_helper_never_entered_that_stays_loaded_retains_the_operation` | retained while the manager keeps the unit, whatever it answers; confirmed once it has unloaded it |
| `i4_07_a_delivered_stop_reply_with_the_scope_still_populated_confirms_nothing` | `i4_07_a_candidate_still_populated_after_its_kill_confirms_nothing` | the candidate killed but populated: retained; confirmed once empty |
| `i4_08_a_timed_out_stop_whose_target_is_gone_may_confirm` | `i4_08_a_target_observed_gone_confirms` | a removed candidate; without one, `NoSuchUnit` and the helper outside |
| `i4_09_a_timed_out_stop_whose_target_remains_retains_the_operation` | `i4_09_a_timed_out_observation_retains_the_operation` | a timed-out GetUnit is never absence |
| `i4_27_a_panic_while_stopping_or_reconciling_retains_the_operation` | `i4_27_a_panic_while_reconciling_retains_the_operation` | the stop fault points are gone; with and without a candidate |
| `i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_by_a_stop_reply` | `i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_once_its_unit_is_unloaded` | the unit stays loaded, untouched; even once the manager has unloaded it, nothing confirms an uncertain start |
| `i4r1_13_an_uncertain_start_without_a_candidate_is_not_released_by_a_timed_out_stop` | `i4r1_13_an_uncertain_start_without_a_candidate_keeps_its_helper_unreaped` | the same assertions, over the start's effect (its world scripted a stop the uncertain start never asked since R2) |
| `i4r2_05_only_a_recorded_start_reply_authorizes_a_stop_by_name` | `i4r2_05_a_recorded_start_reply_authorizes_no_stop_by_name` | recorded or lost, the reply authorizes nothing: the unit untouched, the operation owned |
| `scope::tests::i4r2_x_only_a_recorded_start_reply_authorizes_a_stop_by_name` | `scope::tests::i4r3_10_*` (three) | see below |

## Tests whose world or assertions changed (names kept)

| Test | Change |
|---|---|
| `i4_04`, `i4_10`, `i4_25`, `i4_26`, `i4r1_03`, `i4r1_14`, `i4r1_19` | the StopUnit script removed (it never acted, or the operation never asked a stop) |
| `i4_05` | the manager has already unloaded the unit (`unit_loaded = false`): confirmed gone before the failure by exactly `NoSuchUnit` and the helper outside, nothing opened or killed |
| `i4_15`, `i4r1_x_a_membership_path_not_in_normal_form_…` | accepted without a candidate: retained while the manager has the unit; confirmed once it has unloaded it |
| `i4_16` | retained (`CleanupFailed`) while the manager has the unit; the cgroup of its name (another process in it) never opened or killed; confirmed once that process ends and the unit is unloaded |
| `i4_17` | the stop counts replaced by: the foreign unit still loaded, its cgroup never killed |
| `i4_18` | the never-placed case: the manager has unloaded the unit (while it keeps it, nothing is confirmed: `i4_06`, `i4r3_02`) |
| `i4_19`, `i4r1_23` | the stop count replaced by: GetUnit asked once, by the proof (nothing asked of the manager after it) |
| `i4_22`, `i4_23` | the manager unloads the unit by itself, instead of a stop, before the confirming retry |
| `i4_29` | the never-placed case observes the unit unloaded by the manager |
| `i4_30_no_cgroup_…` | the other process ends and the manager unloads the unit; the cgroup never opened or killed by the backend |
| `i4r1_01` | the stop fault points removed; the unit still loaded, untouched |
| `i4r1_15` | explicit markers for a collision confirmed while the foreign unit holds the helper (NC-R3-10's target); the stop count removed |
| `i4r1_x_a_panic_at_any_ownership_point_…` | the stop fault points and `Op::Stop` removed; the retry's panic is now inside GetUnit |
| `i4r2_01`–`i4r2_04`, `i4r2_07`, `i4r2_08`, `foreign_untouched` | the stop counts replaced by: the unit still loaded and untouched; `i4r2_04` uses a process the kill does not end (`Phantom::Forever`); `i4r2_07` also shows that the manager's later absence confirms nothing |
| `scope::tests::i4_03_…_discovered_and_proven`, `i4q1r1_nc1`, `i4q1r1_nc4` | the stop count removed (the other assertions already pin what was touched) |
| `scope::tests::i4q1r1_nc2` | the stop count replaced by: the unit the effect created stays loaded |
| `scope::tests::i4_a_pending_operation_is_bound_to_its_own_helper` | the manager unloads the unit by itself (instead of a stop) before the other helper's settling, so only the binding decides |

## Added

| Test | Mission | Asserts |
|---|---|---|
| `i4r3_01_an_accepted_operation_never_acts_on_a_unit_loaded_under_its_name_again` | R3-01 | an accepted operation's unit replaced by a foreign one under the same name: only observed, never stopped, killed, opened or claimed; the operation retained, `CleanupFailed`, its helper unreaped, through three retries and through the harness owner |
| `i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded` | R3-02 | retained, each retry bounded, only Start, Membership and GetUnit ever asked, the unit untouched, the helper unreaped |
| `i4r3_03_an_accepted_operation_is_confirmed_once_the_manager_has_unloaded_its_unit` | R3-03 | exactly `NoSuchUnit`, then the helper outside (one membership read), confirm; the helper reaped only then; nothing opened or killed |
| `i4r3_04_an_accepted_operation_s_candidate_is_ended_by_descriptor_and_observed` | R3-04 | the candidate ended by `cgroup.kill` through its descriptor alone, confirmed empty or removed; with the name reused by a foreign unit, that unit's cgroup never opened or killed |
| `i4r3_09_a_retained_operation_retried_without_its_scope_manager_never_acts_by_name` | R3-09 | accepted and uncertain, without the `ScopeManager`: bounded, only observing, retained; the unit unloaded confirms the accepted one, never the uncertain one |
| `scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read` | R3-10 | the trait's six methods; the production requests exactly StartTransientUnit, GetUnit and four property reads; no acting method named |
| `scope::tests::i4r3_10_settling_never_acts_on_a_unit_by_its_name` | R3-10 | every manager request of a pending operation; settling and the drop backstop ask the manager nothing, `absent` exactly GetUnit; no stop or kill by name anywhere in production |
| `scope::tests::i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms` | R3-10 | `accepted` set only in the delivered reply's arm, after the fault point; read only by `gone` (and the test and debug views); a pending operation crate-private, uncopied, not derived |

R3-05 to R3-08 are R2's tests, preserved: `i4r2_03` and `i4r2_04`
(uncertain candidate), `i4r2_01` and `i4r2_02` (uncertain, no candidate),
`i4r2_06` and `i4r1_15` (definite collision), `i4r2_07` (a panic after the
reply arrived, before it was recorded).
