# P2-V1-R3B-I4-R2: root cause

## The defect

The accepted I4-R1 settling of a pending scope operation,
`PendingScope::reconcile` (`crates/nexus-verifier-sandbox/src/scope/pending.rs`
at `c3a12b2b`, unchanged since `ea475eac`), ends in a `StopUnit` by the
unit's name whenever the operation is not yet confirmed gone, whatever the
start's outcome was:

```rust
  159      /// Settle this operation: end whatever it may have created and confirm
  160      /// it gone. `true` once nothing it may have created can still hold or
  161      /// receive a process; `false` keeps it owned, for another attempt. Only
  162      /// the execution's finalizer settles, after killing `helper`, the bound
  163      /// helper its owner keeps unreaped (another helper is ignored).
  164      pub(crate) fn reconcile(&mut self, helper: Option<&Helper>, fault: Option<Fault>) -> bool {
  165          if !self.unresolved() {
  166              return true;
  167          }
  168          let controller = Arc::clone(&self.controller);
  169          let helper = helper.filter(|helper| self.binds(helper));
  170          if self.collided {
  171              // Refused before any effect: the unit of that name is not this
  172              // request's, so it is never stopped or claimed.
  173              fault::at(fault, FaultPoint::ScopeReconcile);
  174              self.settled = outside(&controller, helper, &self.unit);
  175              return self.settled;
  176          }
  177          // Cleanup ownership: everything in the candidate is ended.
  178          if let Some(candidate) = &self.candidate {
  179              let _ = candidate.kill();
  180          }
  181          self.acquire(&controller, helper);
  182          fault::at(fault, FaultPoint::ScopeReconcile);
  183          if self.gone(&controller, helper) {
  184              self.settled = true;
  185              return true;
  186          }
  187          fault::at(fault, FaultPoint::BeforeScopeStop);
  188          self.stops = self.stops.saturating_add(1);
  189          // Neither reply is a confirmation, nor a failure proof that nothing
  190          // was stopped: it is kept for diagnostics only, and what remains is
  191          // observed again.
  192          self.last_stop = Some(controller.manager.stop_unit(&self.unit));
  193          fault::at(fault, FaultPoint::AfterScopeStop);
  194          let start = Instant::now();
  195          loop {
  196              self.acquire(&controller, helper);
  197              fault::at(fault, FaultPoint::ScopeReconcile);
  198              if self.gone(&controller, helper) {
  199                  self.settled = true;
  200                  return true;
  201              }
  202              if start.elapsed() >= controller.timing.settle {
  203                  return false;
  204              }
  205              std::thread::sleep(controller.timing.poll);
  206          }
  207      }
```

`gone()` (lines 231-242) never confirms an operation that is not accepted
and has no candidate, so for such an operation lines 187-193 always run:

```rust
  229      /// Whether nothing this operation may have created can still hold or
  230      /// receive a process (see the module documentation).
  231      fn gone(&self, controller: &Controller, helper: Option<&Helper>) -> bool {
  232          match &self.candidate {
  233              Some(candidate) => matches!(
  234                  occupancy(candidate.as_ref()),
  235                  Ok(Occupancy::Empty | Occupancy::Removed)
  236              ),
  237              // Without a delivered reply the manager's absence proves
  238              // nothing.
  239              None if !self.accepted => false,
  240              None => absent(controller, &self.unit, helper),
  241          }
  242      }
```

## Why that is a trust-boundary defect

`Started::Uncertain` (no reply decoded: a timeout, a broken transport, an
unexpected error, a reply that does not decode; or a panic before the reply
was recorded) leaves the remote outcome unknown. Three outcomes are
indistinguishable to the backend:

| Outcome | What happened | What the name then designates |
|---|---|---|
| A | the request created the unit; its reply was lost | this request's unit |
| B | the request had no effect | nothing (or anything loaded later) |
| C | a foreign unit already held the name; systemd refused the request with exactly `UnitExists` before reading any property (no effect; R1's `primary/excerpts.md`, `dbus-manager.c` 1016-1018) and that error reply was lost | **a foreign unit** |

In outcome C the operation is `accepted == false`, `collided == false`, and
no candidate can ever appear (the request never moved the helper), so the
I4-R1 settling reaches line 192 and stops the foreign unit by its name: the
generated string was the only authority used. The accepted rule (a
collision is foreign and never stopped) held only when the `UnitExists`
reply was delivered. Randomness made outcome C improbable, never
impossible, and probability is not authority.

## Reproduction

- P2-V1-R3B-I4-Q1-R1, reproduction E
  (`docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/reproduction/run.out`):
  "RESIDUAL (E): StopUnit attempts for the generated name: 1; the foreign
  unit is still loaded: false".
- As production regression tests (`reproduction/before-repair.out`): the
  candidate's own tests (`execution::tests::i4r2_*` and the three tests R2
  tightened) run against the unrepaired production code (`c3a12b2b` with
  only the candidate's two test files). `i4r2_01` (a lost `UnitExists`
  reply) fails with "the foreign unit was stopped by its name"; every test
  that requires no stop by name without a recorded start reply fails the
  same way; `i4r2_06` (a delivered `UnitExists`) passes, as it did.

## The repair

`reconcile` asks the manager to stop the unit by name only past
`stop_authorized()`, which is exactly the recorded start reply
(`accepted`); every other operation goes straight to `observe()`: the
candidate retained (through the bound helper's membership), ended through
its descriptor, and seen empty or removed; or the operation stays owned.
`matrices/state-machine.md` states every path.
