# P2-V1-R3B-I4-R3-R1: test and model changes

The sandbox crate's library tests: 138 at the base, 153 in the candidate
(+15 new: ten behavioural `r3r1_01`–`r3r1_10` and `r3r1_x_a_start_whose_…`
in `execution::tests`; the decoding and capture rule `r3r1_05`, the surface
`r3r1_11`, the object path and the live-qualification pin `r3r1_x_*` in
`scope::tests`). Four tests are renamed because their old name stated the
behaviour R3-R1 forbids; none was deleted. The before-repair reproduction
module is evidence only, never part of the candidate.

## The model (`src/scope/tests.rs`)

- Every unit invocation has its own identity (`Unit::instance`) and every
  cgroup directory its own kernel ID (`Cgroup::id`, handed out by
  `State::add_cgroup`), never reused; `FakeDir::cgroup_id` reports it.
- An accepted start's identity is captured by the production rule
  (`captured_instance`) from the signals systemd would send, or from
  scripted others (`Capture`: exact, no removal within the bound, a failed
  job, malformed, null, two identities, after the removal, wrapped
  serials); `State::watch_fails` makes the start never sent (`NotIssued`).
- The manager answers `InvocationID` (`unit_instance`, a per-read script:
  exact, another valid identity, no identity, uncertain) and
  `ControlGroupId` (`control_group_id`: its record of the unit's directory,
  another, none, uncertain) and logs both (`Call::UnitInstance`,
  `Call::ControlGroupId`).
- `Interference`: once the identity is captured and before the proof,
  another client replaces the unit under the same name holding the helper
  (`Replace`), or replaces its directory at the same path under the
  manager's record (`Swap`, `State::swap_directory`).
- New panic points: `Op::Capture` (after the success reply, while the
  identity is captured), `Op::UnitInstance`, `Op::ControlGroupId`.

## Renamed tests (their old name stated what R3-R1 forbids)

| Old | New | What it asserts now |
|---|---|---|
| `scope::tests::i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven` | `scope::tests::i4_03_a_start_with_effect_whose_reply_is_lost_is_never_proven_or_ended` | a lost reply with effect (a timeout, a malformed reply, an error, a broken connection; the helper placed): never opened, proven or ended; owned with its helper unreaped through retries |
| `i4r1_14_an_uncertain_start_that_places_the_helper_is_acquired_and_may_be_proven` | `i4r1_14_an_uncertain_start_that_places_the_helper_is_never_acquired_proven_or_ended` | placed while the proof would wait, or later: never acquired, proven, launched into or ended; owned |
| `i4r2_03_an_uncertain_start_whose_helper_appears_later_is_ended_through_its_candidate` | `i4r2_03_an_uncertain_start_whose_helper_appears_later_is_never_ended_through_its_cgroup` | the helper placed later: the cgroup never opened or ended, by descriptor or by name; owned, the helper unreaped |
| `i4r2_04_an_uncertain_start_s_candidate_is_ended_by_descriptor_never_by_name` | `i4r2_04_an_uncertain_start_s_cgroup_is_never_ended_by_descriptor_or_by_name` | the helper placed while the proof would wait (populated or not): never opened, bound, launched into or ended; nothing stopped by name; retained |

## Tests whose assertions changed (names kept)

| Test | Change |
|---|---|
| `scope::tests::i4_01`, `i4r1_10`, `i4r1_18` | the exact call sequence of a proof that passes: the helper's membership read again after the open, then GetUnit, `InvocationID`, `Id`, `ControlGroup`, `ControlGroupId`, `InvocationID`, `RuntimeMaxUSec`, `OOMPolicy`, `InvocationID` (`proof_calls`); `i4_01` also reads the identity and the directory ID the proven scope holds |
| `i4_11` | the helper in a cgroup of the unit's name the manager does not have loaded: nothing binds it, so it is never ended (it was); retained until that cgroup goes, then confirmed by absence |
| `i4_18` | that same world moved out of the "confirmed" loop: never launched, refused, retained |
| `i4_25` | a panic after the start returned: the reply and its identity unrecorded, so nothing is opened or ended (the cgroup was ended); retained, also when placed |
| `i4r1_06` | a cgroup of the unit's name elsewhere: refused as before, and never ended (it was); confirmed once the unit is unloaded and that cgroup gone |
| `i4r1_07`, `i4r1_08`, `i4r1_09` | a refused or uncertain binding (control group, unit id): never ended (it was); once the manager answers exactly, the retained candidate is bound, ended and confirmed through its descriptor |
| `i4r1_17` | the same for every unresolved binding case; a panic inside the model's read is bound and ended by the finalizer at once |
| `i4r1_19` | the binding failure's candidate retained, never ended unbound; retried without the `ScopeManager` once the manager answers exactly, never reopened |
| `scope::tests::i4r3_10_*` (three) | the R3 structural guards follow R3-R1's code with the same invariants: the trait's eight methods and the nine requests (`Subscribe` added, StartTransientUnit through `call_reply`); settling's manager requests (the binding's reads added; `own` and `end_owned` asking nothing); the accepted arm `Started::Accepted(captured)` and `self.accepted && absent(...)` |

Every other existing test passes unchanged against the candidate.

## New tests (mission section 18)

| Mission | Test | Asserts |
|---|---|---|
| R3R1-01 | `r3r1_01_a_lost_collision_holding_the_helper_is_never_ended_proven_or_launched` | a lost `UnitExists`, the foreign cgroup holding the helper and a process of its own with matching limits and properties: no launch, nothing opened, read or ended, the foreign process untouched, `CleanupFailed`, the helper unreaped; through retries and the harness owner |
| R3R1-02 | `r3r1_02_a_same_name_replacement_holding_the_helper_is_never_ended_proven_or_launched` | the unit replaced while the proof waits, or after the execution, by a foreign unit holding the helper: `Mismatch("unit instance")`, never bound, ended, proven or launched into, nothing beyond its identity read; an owned candidate whose directory went with its unit confirmed through its descriptor, never reopened from its path |
| R3R1-03 | `r3r1_03_a_start_with_its_exact_identity_is_bound_owned_and_proven` | bound, owned, proven, the scope holding exactly that identity and directory ID; an execution launches and finalizes through the descriptor |
| R3R1-04 | `r3r1_04_an_identity_that_is_not_the_captured_one_is_never_bound` | another identity at every read, or only at the read closing the binding; another directory at the same path (`ControlGroupId`): never owned, ended, proven or launched into |
| R3R1-05 | `r3r1_05_an_unavailable_or_malformed_identity_binds_nothing`; `scope::tests::r3r1_05_the_identity_decoding_and_the_capture_rule_fail_closed` | every capture failure: nothing opened or ended, confirmed only once the unit is unloaded; every binding-read failure (manager error, timeout, broken connection, undecodable, no usable identity): never ended until answered exactly; the production decoding (wrong type, wrong length, all zero, a nested variant, null) and capture rule over exact values |
| R3R1-06 | `r3r1_06_an_owned_candidate_whose_policy_fails_is_ended_never_launched` | a limit, the runtime backstop, the out-of-memory policy, the identity after the policy: no launch, the owned candidate ended and confirmed, a foreign unit elsewhere untouched |
| R3R1-07 | `r3r1_07_an_uncertain_start_that_created_its_scope_is_never_ended_proven_or_launched` | four lost-reply kinds and the harness owner: never opened, ended, proven or launched into; owned even once the unit is unloaded |
| R3R1-08 | `r3r1_08_a_delivered_collision_is_unchanged` | a delivered `UnitExists`, holding the helper or not: never opened, ended, read, proven or launched into; confirmed once the helper is outside |
| R3R1-09 | `r3r1_09_no_panic_grants_cleanup_or_launch_authority_early` | fifteen panic points, through the execution and (inside the model) the harness owner: no owner escape, every kill after a complete binding run, no launch, the helper unreaped while unresolved; before the identity is recorded retained untouched, after it bound, ended and confirmed |
| R3R1-10 | `r3r1_10_no_authority_appears_when_a_retained_operation_is_retried_without_its_scope_manager` | uncertain, without an identity, another identity, an unanswered identity: retried without the `ScopeManager`, nothing ended; the unanswered one bound and ended only once answered |
| R3R1-11 | `scope::tests::r3r1_11_the_identity_and_the_ownership_have_no_surface` | no public identity minting, no serializable or defaulted identity or candidate, no caller-supplied identity, no frontend identity, the harness accessors gated and read-only, one start site; one `cgroup.kill` path per state; ownership set once after the binding; the promotion only of an owned candidate after its fault point |
| (the never-sent start) | `r3r1_x_a_start_whose_signals_cannot_be_watched_is_never_sent` | the request never sent: confirmed at once, nothing asked of the manager or the kernel |
| (the object path) | `scope::tests::r3r1_x_a_start_watches_its_unit_s_own_object_path` | systemd's `bus_label_escape` for the generated name, a leading digit, `_`, a non-ASCII byte and the empty name |
| (section 21) | `scope::tests::r3r1_x_the_host_qualification_reads_the_identity_production_records` | the live H3/H4 case reads `InvocationID` (`ay`, 16 bytes, not zero) twice and `ControlGroupId` (`t`), compares them with the scope's recorded identity and descriptor ID, re-reads the membership; the count stays 39 |
