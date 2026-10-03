# P2-V1-R3B-I4-R2: negative controls (mission section 16)

Every control applies its edits in an isolated clean checkout of the
candidate, compiles, runs its one intended test alone, which must fail at
its marker, restores every file byte for byte and checks the checkout's Git
status. Production files are mutated only in those scratch checkouts.

## R2 mutation controls (`scripts/r2_controls.py`, `controls/r2/`)

| ID | Mission item | File | Restores | Intended test | Marker |
|---|---|---|---|---|---|
| NC-R2-01-UNCONDITIONAL-STOP | 1 | `scope/pending.rs` | the name authority check removed: every operation not yet gone is stopped by name | `i4r2_01` | the foreign unit was stopped by its name |
| NC-R2-02-STOP-FOR-UNACCEPTED | 2 | `scope/pending.rs` | `stop_authorized()` true for any operation not collided | `i4r2_02` | an uncertain start was stopped by its name |
| NC-R2-03-LOST-COLLISION-CONVERTED | 3 | `scope/pending.rs` | an uncertain start without a candidate turned into a collision by GetUnit's presence of the name, then settled | `i4r2_01` | an uncertain start was resolved by its unit name or presence |
| NC-R2-04-REAPED-WHILE-UNCERTAIN | 4 | `execution.rs` | the helper reaped when its uncertain operation cannot be confirmed | `i4r2_02` | the helper of an unresolved uncertain start was reaped |
| NC-R2-05-NO-SUCH-UNIT-SETTLES | 5 | `scope/pending.rs` | `NoSuchUnit` with the helper outside settles an uncertain start without a candidate | `i4r2_02` | an uncertain start was resolved by its unit name or presence |
| NC-R2-06-STOP-REPLY-CONFIRMS | 6 | `scope/pending.rs` | a delivered StopUnit reply taken as confirmation | `i4r2_05` | a StopUnit reply confirmed the operation |
| NC-R2-07-CANDIDATE-NAME-AUTHORITY | 7 | `scope/pending.rs` | an uncertain operation with a candidate gains a stop by name | `i4r2_04` | an uncertain start's candidate was stopped by its name |
| NC-R2-08-ACCEPTED-BY-PRESENCE | 8 | `scope/pending.rs` | `accepted` forged from GetUnit's presence of the name | `i4r2_01` | the foreign unit was stopped by its name |
| NC-R2-X-STOP-OUTSIDE-SETTLING | additional | `scope/pending.rs` | a stop by name in the drop backstop (`end_now`) | `i4r2_x_…` (structural) | a stop by name outside settling |
| NC-R2-X-ACCEPTED-ELSEWHERE | additional | `scope/pending.rs` | `accepted` set where a candidate is retained | `i4r2_x_…` (structural) | a start is accepted other than in the delivered reply's arm |

## Accepted controls, rerun

- **I4 and I4-R1 behavioural controls (38)**: the accepted I4-R1 runner,
  unchanged (SHA-256 verified), through `scripts/accepted_scope_controls_r2.py`,
  which retargets exactly two controls, both because R2 removes every stop
  by name from an operation without a recorded start reply (each keeps its
  mutation and its anchor; `controls/accepted/`):
  - `NC-I4R1-STOP-REPLY-ABSENCE`: its accepted test `i4r1_12` concerned an
    uncertain start, which R2 never stops by name, so the mutated line is
    unreachable from it; it now runs `i4r2_05` (the only path that still
    stops by name) with that test's marker;
  - `NC-I4-X-COLLISION-CLAIMED` (`Started::Collision => None`): the mutated
    operation is neither collided nor accepted, so R2 no longer stops the
    colliding unit by name, which was its accepted test `i4_17`'s marker
    (`i4_17` still fails under the mutation, at its error classification:
    the first full rerun recorded it). The harm that remains is the claim
    of the foreign unit's cgroup as this request's scope; the control now
    runs the accepted I4-R1 test of exactly that, `i4r1_16`, with its
    marker.
- **Q1-R1 controls (15)**: the accepted Q1-R1 runner, unchanged (SHA-256
  verified), through `scripts/accepted_r1_controls_r2.py`, which retargets
  exactly one control for the same reason: `Q1-R1-NC2-STOP-REPLY-PROOF`
  keeps its mutation and anchor and runs `i4r2_05` instead of
  `i4q1r1_nc2`. `controls/q1r1/`.
- **Q1 controls (10)**: unchanged. `controls/q1/`.
- **API/type guards, normal-build probes, source guards, custody hashes,
  R3 store (179), I2-R1 (32 + 4), R3 API probes (28)**: unchanged runners
  (`scripts/accepted_reruns.sh`). `controls/accepted/`.

## Tests R2 changed (and why)

Three accepted tests asserted the defect: that an uncertain start without a
candidate is stopped by name. Each keeps its name and its markers and now
asserts the stronger rule:

| Test | Before | After |
|---|---|---|
| `execution::tests::i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_by_a_stop_reply` | a stop was attempted and took effect (the unit unloaded), and its reply released nothing | no stop is ever asked; the unit stays loaded; still not released, the helper still unreaped |
| `execution::tests::i4r1_01_a_panic_after_an_uncertain_start_without_a_candidate_keeps_its_owner` (second part) | after a panic inside StartTransientUnit, "StopUnit even unloads the unit" | the unit stays loaded, no stop by name; one owner, the helper unreaped (unchanged) |
| `scope::tests::i4q1r1_nc2_an_uncertain_first_start_keeps_its_helper_unreaped_and_unconfirmed` | at least one StopUnit was attempted | no StopUnit at all |
