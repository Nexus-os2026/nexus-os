# P2-V1-R3B-I4-R3: negative controls and historical control adaptations

Every mutation control applies its edits in an isolated clean checkout of
the candidate, compiles, runs its one intended test alone (`--exact`),
which must fail at its marker, restores every file byte for byte and checks
the checkout's Git status. Production files are mutated only in those
scratch checkouts; the tests and the model are never mutated.

## R3 mutation controls (`scripts/r3_controls.py`, `controls/r3/`)

| ID | Mission item | Files | Restores | Intended test | Marker |
|---|---|---|---|---|---|
| NC-R3-01-STOPUNIT-IN-RECONCILE | 1 | `scope/manager.rs`, `scope/pending.rs` | StopUnit by the unit's name in settling, for every operation not collided (the request restored as a trait default and in the production manager) | `scope::tests::i4r3_10_settling_never_acts_on_a_unit_by_its_name` (structural) | settling acts on a unit through the manager |
| NC-R3-02-MANAGER-STOP-RESTORED | 2 | `scope/manager.rs` | the manager's `stop_unit` and its StopUnit request, unused | `scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read` (structural) | the manager can act on a unit by its name |
| NC-R3-03-ACCEPTED-AUTHORIZES-STOP | 3 | `scope/manager.rs`, `scope/pending.rs` | `stop_authorized()` (= `accepted`) gating a stop by name: the R2 shape | `scope::tests::i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms` (structural) | the recorded start reply gates more than what the manager's absence can confirm |
| NC-R3-04-PRESENCE-AS-OWNERSHIP | 4 | `scope/pending.rs` | a unit the manager reports under the name taken as the accepted operation's: its control group retained as the candidate and ended | `execution::tests::i4r3_01_…` | an accepted operation was confirmed while a unit of its name is loaded |
| NC-R3-05-OBJECT-PATH-IDENTITY | 5 | `scope/pending.rs` | the object path GetUnit returns taken as the unit's identity, the unit at it ended through its control group | `execution::tests::i4r3_01_…` | the foreign unit loaded under the name was claimed |
| NC-R3-06-FOREIGN-PRESENCE-CONFIRMS | 6 | `scope/pending.rs` | an accepted operation without a candidate confirmed while a unit is loaded under its name (a loaded unit that does not hold the helper taken as another's) | `execution::tests::i4r3_01_…` | an accepted operation was confirmed while a unit of its name is loaded |
| NC-R3-07-REAPED-WHILE-PRESENT | 7 | `execution.rs` | the helper reaped while its operation is unresolved, here an accepted one whose unit is loaded | `execution::tests::i4r3_02_…` | the helper of an accepted operation was reaped while its unit is loaded |
| NC-R3-08-UNCERTAIN-WEAKENED | 8 | `scope/pending.rs` | an uncertain start recorded as accepted, so the manager's absence settles it | `execution::tests::i4r2_02_…` (R3-06) | an uncertain start was resolved by its unit name or presence |
| NC-R3-09-CANDIDATE-BY-NAME | 9 | `scope/pending.rs` | the candidate ended through the unit's path rebuilt from its name, not its descriptor | `execution::tests::i4r3_04_…` | the candidate was not ended through its descriptor alone |
| NC-R3-10-COLLISION-WEAKENED | 10 | `scope/pending.rs` | a definite collision confirmed at once, without the helper observed outside the foreign unit | `execution::tests::i4r1_15_…` (R3-07) | a collision was confirmed while the foreign unit holds the helper |

A control that restores a manager request acting by name gives the trait a
default method, so that the model's manager (never mutated) still
compiles; the model cannot observe such a request, so NC-R3-01 to NC-R3-03
are caught by the structural guards. NC-R3-04 to NC-R3-10 are caught by
behavioural tests over the model.

## The first compatibility run (`scripts/compat_run1.sh`, `controls/compat-run1/`)

Before any adaptation, the accepted runners R3 adapts were run exactly as
P2-V1-R3B-I4-R2 accepted them, unmodified, on a clean checkout of the
candidate: the I4 and I4-R1 controls through R2's
`accepted_scope_controls_r2.py`, R2's `r2_controls.py`, the Q1-R1 controls
through R2's `accepted_r1_controls_r2.py`, and the I4-R1
`source_guards.py`; each runner's anchor check, then its full run. They
stop where R3 removed what they anchor on (outputs kept as recorded there;
`REPORT.md` section 5 summarizes them).

## Historical controls: what R3 runs, retargets and supersedes

Every accepted runner is imported unchanged, its SHA-256 verified first;
the R3 adapters only select, retarget or re-anchor, and record why.

### I4 and I4-R1 (38; `scripts/accepted_scope_controls_r3.py`; runner `p2-v1-r3b-i4-r1-native-scope/scripts/scope_controls.py`, SHA-256 `6e84f051…f2`)

| Control | R3 | Reason | Carried by |
|---|---|---|---|
| NC-I4-STOP-OK-CONFIRMS | superseded | its anchor (the StopUnit call in `reconcile`) and the request it mutates no longer exist: no StopUnit reply exists to be taken as cleanup | NC-R3-01, NC-R3-02; `i4_07`, `i4r3_02` |
| NC-I4-STOP-ERR-NO-EFFECT | superseded | the same: no StopUnit error exists to be taken as nothing left | NC-R3-01, NC-R3-02; `i4_06`, `i4r3_02` |
| NC-I4-STOP-TIMEOUT-DROPS | superseded | the same: no StopUnit timeout exists to drop the operation | NC-R3-01, NC-R3-02; `i4_09`, `i4_07` |
| NC-I4R1-STOP-REPLY-ABSENCE | superseded | the same (R2 had retargeted it to `i4r2_05`, the one path that still stopped by name; R3 removes that path) | NC-R3-01, NC-R3-03; `i4r1_12`, `i4r3_02` |
| NC-I4R1-REAP-UNRESOLVED | retargeted | its test `i4r1_13` is renamed (`…_keeps_its_helper_unreaped`): same mutation, anchor, assertions and marker | — |
| NC-I4-X-COLLISION-CLAIMED | R2 adaptation kept | runs `i4r1_16` (the foreign cgroup's claim), as R2 recorded | — |
| the other 32 | unchanged | anchors, mutations, tests and markers exactly as accepted (two of them with I4-R1's own adaptations, unchanged) | — |

34 run, 4 superseded.

### R2 (10; `scripts/accepted_r2_controls_r3.py`; runner `p2-v1-r3b-i4-r2-uncertain-stop-authority/scripts/r2_controls.py`, SHA-256 `53d95af2…d036`)

| Control | R3 | Reason | Carried by |
|---|---|---|---|
| NC-R2-01-UNCONDITIONAL-STOP | superseded | its anchor (the name authority check) and the stop behind it no longer exist | NC-R3-01 |
| NC-R2-02-STOP-FOR-UNACCEPTED | superseded | `stop_authorized()` no longer exists: no operation has any authority to act by name | NC-R3-03, NC-R3-01 |
| NC-R2-06-STOP-REPLY-CONFIRMS | superseded | no StopUnit call or reply exists | NC-R3-01, NC-R3-02 |
| NC-R2-07-CANDIDATE-NAME-AUTHORITY | superseded | no stop by name exists for a candidate to grant | NC-R3-09, NC-R3-01 |
| NC-R2-X-STOP-OUTSIDE-SETTLING | superseded | its mutation calls the removed `stop_unit` (cannot compile) | NC-R3-02; the settling guard pins `end_now` |
| NC-R2-03-LOST-COLLISION-CONVERTED | re-anchored | the same mutation (an uncertain start turned into a collision by the name's presence, then settled), inserted where R3's settling reaches the observation | same test (`i4r2_01`), same marker |
| NC-R2-08-ACCEPTED-BY-PRESENCE | re-anchored, retargeted | the same mutation (`accepted` forged from the name's presence); its R2 harm (the foreign unit stopped) no longer exists, its remaining harm (an uncertain start confirmed by a later absence) is what `i4r3_09` marks | `i4r3_09`, marker "an uncertain start was resolved by its unit name or presence" |
| NC-R2-X-ACCEPTED-ELSEWHERE | retargeted | the R2 structural guard `i4r2_x` is replaced by the R3 guard of the recorded start reply, with the same check and marker | `i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms` |
| NC-R2-04, NC-R2-05 | unchanged | — | — |

5 run, 5 superseded.

### Q1-R1 (15; `scripts/accepted_r1_controls_r3.py`; runner `p2-v1-r3b-i4-q1-r1-host-qualification-ownership/scripts/r1_controls.py`, SHA-256 `eea344ea…a3c0`)

| Control | R3 | Reason | Carried by |
|---|---|---|---|
| Q1-R1-NC1-FOREIGN-STOPPED | superseded | its mutation calls the removed `stop_unit` on the collided unit (cannot compile) | NC-R3-02, NC-R3-01, NC-R3-10; Q1-R1-NC1-FOREIGN-KILLED (unchanged) |
| Q1-R1-NC2-STOP-REPLY-PROOF | superseded | its anchor (the StopUnit call in settling) no longer exists (R2 had retargeted it to `i4r2_05`) | NC-R3-01, NC-R3-02; Q1-R1-NC2-UNRESOLVED-REAPED (unchanged) |
| the other 13 | unchanged | — | — |

13 run, 2 superseded.

### Q1 (10; `p2-v1-r3b-i4-q1-host-live-qualification/scripts/q1_controls.py`, SHA-256 `088a2993…5efe92`)

Unchanged: every anchor applies; run as accepted.

### Source guards (14 guards with self-tests; `scripts/accepted_source_guards_r3.py`; script `p2-v1-r3b-i4-r1-native-scope/scripts/source_guards.py`, SHA-256 `b3c90101…0db84`)

| Guard or self-test | R3 | Reason |
|---|---|---|
| SG-I4-DESTINATION | adapted | the accepted function with exactly two pins changed: six internal calls (StopUnit's removed) and the manager interface without `stop_unit`; every other check as accepted |
| its self-test changing `stop_unit`'s signature | replaced | its anchor no longer exists; the same injection on `get_unit` (a destination parameter), plus one restoring `stop_unit` in the interface |
| SG-I4R3-NO-NAME-ACTUATION | added | no production source names a request that acts on a unit or what served the stop by name; the manager issues exactly StartTransientUnit, GetUnit and four property reads; settling and the drop backstop ask the manager nothing, `absent` exactly GetUnit; four self-tests |
| the other 13 guards and their self-tests | unchanged | — |

## Accepted suites, rerun unchanged (`scripts/accepted_reruns.sh`, `controls/accepted/`)

The I4-R1 harness-build API/type guards (27), the normal-build API guards
(9), the custody store's immutable hashes, the R3 store controls (179), the
I2-R1 rerun (32 + 4) and the R3 API probes (28), each with its accepted
runner and its own empty target directory.
