# P2-V1-R3B-I4-R3-R1: the manager surface

`scope/manager.rs`. R3's rule stands: StartTransientUnit is the only request
that acts on a unit; no request acts on a unit by its name or its object
path (no StopUnit, KillUnit, `Unit.Stop` or other action, nothing
verify-then-act).

## Requests (all through the one bounded call, to the fixed destination)

| Request | Object path, interface | Acts | Added by |
|---|---|---|---|
| `Subscribe` | `/org/freedesktop/systemd1`, `Manager` | on no unit: the manager sends this connection its signals (once per connection; `AlreadySubscribed` tolerated) | R3-R1 |
| `StartTransientUnit(name, "fail", properties, [])` | `/org/freedesktop/systemd1`, `Manager` | the one request that acts on a unit, a fresh backend-random name; its reply message kept (sender, serial, job path) | (I4) |
| `GetUnit(name)` | `/org/freedesktop/systemd1`, `Manager` | reads | (I4) |
| `Get(Unit, "Id")` | GetUnit's path, `Properties` | reads | (I4-R1) |
| `Get(Scope, "ControlGroup")` | GetUnit's path, `Properties` | reads | (I4-R1) |
| `Get(Scope, "RuntimeMaxUSec")`, `Get(Scope, "OOMPolicy")` | GetUnit's path, `Properties` | read | (I4) |
| `Get(Unit, "InvocationID")` | GetUnit's path, `Properties` | reads | R3-R1 |
| `Get(Scope, "ControlGroupId")` | GetUnit's path, `Properties` | reads | R3-R1 |

Besides these, each start registers two match rules with the bus
(`AddMatch`, through zbus) before its request is sent, and removes them
when its watch is dropped: signals only, from `org.freedesktop.systemd1`,
`JobRemoved` with arg2 the unit's name, and `PropertiesChanged` on the
unit's object path with arg0 `org.freedesktop.systemd1.Unit`. They are
bus-daemon requests, not manager requests, and act on nothing.

## The trait

`start_scope`, `get_unit`, `runtime_max_usec`, `oom_policy`, `unit_id`,
`control_group`, and (R3-R1) `unit_instance`, `control_group_id`. Crate-
private; the deterministic model implements it in test builds only.

## What `start_scope` returns

| Outcome | `Started` |
|---|---|
| the start's signals could not be watched (Subscribe or a match failed or timed out): the request never sent | `NotIssued(reason)` |
| a decoded success reply (its job path) | `Accepted(Ok(identity))`, or `Accepted(Err(reason))` when the capture failed |
| exactly `org.freedesktop.systemd1.UnitExists` | `Collision` (the arm unchanged) |
| any other error reply, a timeout, a transport failure, a reply that does not decode | `Uncertain(reason)` (the arms unchanged) |

## Pins

- `scope::tests::i4r3_10_the_manager_can_only_start_a_scope_and_read`: the
  trait's eight methods, one `.call_method(`, the forwarding of `call` to
  `call_reply`, ten `self.call` occurrences, and the exact nine requests
  above (object path, interface, method, body); no acting method name.
- `scope::tests::r3r1_11_the_identity_and_the_ownership_have_no_surface`:
  `UnitInstance` crate-private, made only in `invocation_of` from the
  manager's value, never serialized or defaulted; `Started::Accepted(`
  constructed once; a request names only the unit, the helper and the
  limits.
- The source guards (`accepted_source_guards_r3r1.py`): SG-I4-DESTINATION
  (nine internal calls, eight trait signatures), SG-I4R3-NO-NAME-ACTUATION
  (the request list), SG-R3R1-CANDIDATE-OWNERSHIP (the capture rule, the
  identity's one constructor).
- `scope::tests::i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names`
  (unchanged): `UnitExists` and `NoSuchUnit`, each in exactly one arm.
