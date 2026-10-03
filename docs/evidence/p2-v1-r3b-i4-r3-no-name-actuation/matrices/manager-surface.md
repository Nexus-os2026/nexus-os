# P2-V1-R3B-I4-R3: the manager surface

## `scope::manager::Manager` (crate-private trait)

| Method | D-Bus request (production `ZbusManager`) | Effect | Callers |
|---|---|---|---|
| `start_scope` | `Manager.StartTransientUnit(name, "fail", properties, [])` | creates and starts a transient scope for a fresh backend-random name | `PendingScope::issue_and_prove` only |
| `get_unit` | `Manager.GetUnit(name)` | none (read) | the proof (`prove`), and `absent` (settling, accepted without a candidate) |
| `unit_id` | `Properties.Get(Unit, "Id")` at the object path GetUnit returned | none (read) | the proof only |
| `control_group` | `Properties.Get(Scope, "ControlGroup")` | none (read) | the proof only |
| `runtime_max_usec` | `Properties.Get(Scope, "RuntimeMaxUSec")` | none (read) | the proof only |
| `oom_policy` | `Properties.Get(Scope, "OOMPolicy")` | none (read) | the proof only |
| ~~`stop_unit`~~ | ~~`Manager.StopUnit(name, "replace")`~~ | removed by R3 | — |

`StopUnit` is completely removed: the trait method, the production request
(`ZbusManager::stop_unit`), its only caller (`PendingScope::reconcile`),
the name authority that gated it (`stop_authorized`), its diagnostics
state (`stops`, `last_stop`, `PendingState::stops`) and its two fault
points (`FaultPoint::BeforeScopeStop`, `AfterScopeStop`). No replacement
was added: no `KillUnit`, no `Unit.Stop` or `Unit.Kill` on an object path,
no other request that acts.

The model's manager (`scope::tests::FakeManager`) implements the same six
methods; it has nothing to stop or kill a unit with. A unit leaves the
model only when the manager unloads it by itself
(`State::unload`), and a test may then load a foreign unit under the same
name (`State::load_foreign`).

## Pins

| Pin | Where |
|---|---|
| the trait declares exactly the six methods; every production request is one of StartTransientUnit, GetUnit and four property reads, through the one bounded call; no acting method name in the manager's code | `scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read`; source guards `SG-I4-DESTINATION` (adapted: six calls, the interface without `stop_unit`) and `SG-I4R3-NO-NAME-ACTUATION` |
| settling, observing, acquiring, the gone check and the drop backstop ask the manager nothing; `absent` asks exactly GetUnit; no production source names a stop or kill by name, the name authority, its state or its fault points | `scope::tests::i4r3_10_settling_never_acts_on_a_unit_by_its_name`; `SG-I4R3-NO-NAME-ACTUATION` |
| `accepted` is set only in the delivered reply's arm, after the post-dispatch fault point, and read only where the manager's absence is weighed (and by the test and debug views) | `scope::tests::i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms`; `SG-I4R1-UNCERTAIN` (accepted, unchanged) |
| the other native-facing code's own denylists: the cleanup observation (`StopUnit`, `KillUnit`, `ResetFailed`) and the live host qualification (`stop_unit`, `StopUnit`) | desktop `p2_g_09`, `p2_g_11`; `phase2_cleanup_observation` (unchanged) |

## Unchanged

The destination, object path, interfaces and error identities
(`NoSuchUnit`, `UnitExists`, each read in exactly one arm:
`i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names`),
the connection (`connect()` from the real uid alone), the request's
properties, and the normal-build surface (`BUS_CALL_TIMEOUT` the manager
module's only public item).
