# P2-V1-R3B-I4-Q1-R1: the Architect's H6 finding, reproduced

The finding concerns the Q1 candidate `3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823`
(tree `d2bafa9b81edca6d5b07c3a23d4f2546404abede`): its collision case (H6)
built an owner, `Existing`, before it knew what its first
`StartTransientUnit` had done, and that owner's `Drop` killed and reaped the
helper and then stopped the unit by its name, whatever the outcome.

## The code (3d46b4d, `crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`)

```rust
 1734          /// A deliberately existing scope the collision case owns: its helper
 1735          /// and its unit are ended by the case; drop is defense only.
 1736          struct Existing {
 1737              probe: Probe,
 1738              helper: Option<Helper>,
 1739              unit: String,
 1740              done: bool,
 1741          }
 1742
 1743          impl Drop for Existing {
 1744              fn drop(&mut self) {
 1745                  if self.done {
 1746                      return;
 1747                  }
 1748                  if let Some(mut helper) = self.helper.take() {
 1749                      let _ = helper.kill();
 1750                      let _ = helper.reap();
 1751                  }
 1752                  let _ = self.probe.stop_unit(&self.unit);
 1753              }
 1754          }
 1755
 1756          /// H6: a transient scope this harness creates under a fresh unique
 1757          /// name, holding a helper it owns, is refused a second
 1758          /// `StartTransientUnit` with exactly
 1759          /// `org.freedesktop.systemd1.UnitExists`; the case then ends its own
 1760          /// helper and unit and observes both gone. No other unit is touched.
 1761          pub fn h6_unit_exists(_: &ScopeManager) {
 1762              let name = "h6";
 1763              let before = scopes_now(name);
 1764              let (helper, output) = Helper::spawn(&HelperProgram::at(HELPER)).unwrap();
 1765              std::mem::forget((read_all(output.stdout), read_all(output.stderr)));
 1766              let pid = helper.pid();
 1767              let mut existing = Existing {
 1768                  probe: probe(name),
 1769                  helper: Some(helper),
 1770                  unit: hq::fresh_unit_name("q1h6-"),
 1771                  done: false,
 1772              };
 1773              let unit = existing.unit.clone();
 1774              match existing.probe.start_transient_scope(&unit, pid) {
 1775                  Ok(Answer::Returned(job)) => evidence(
 1776                      "H6",
 1777                      &format!("created unit={unit} pid={pid} job={}", job.as_str()),
 1778                  ),
 1779                  other => panic!("{name}: StartTransientUnit of the fresh name: {other:?}"),
 1780              }
 1781              assert!(
 1782                  wait_until(Duration::from_secs(10), || {
 1783                      hq::membership(pid)
 1784                          .ok()
 1785                          .and_then(|membership| membership.unified)
 1786                          .is_some_and(|path| hq::last_component(&path) == unit)
 1787                  }),
 1788                  "{name}: the owned helper did not enter {unit}"
 1789              );
 1790              let (error, message) = match existing.probe.start_transient_scope(&unit, pid) {
 1791                  Ok(Answer::Refused { name, message }) => (name, message),
 1792                  other => panic!("{name}: a second StartTransientUnit of {unit}: {other:?}"),
 1793              };
 1794              assert_eq!(error, UNIT_EXISTS, "{name}: {message}");
 1795              evidence(
 1796                  "H6",
 1797                  &format!("second start error={error} message={message:?}"),
 1798              );
 1799              // Owned cleanup: the helper, then the unit; the reply is
 1800              // diagnostics, the manager's absence is observed.
 1801              let mut helper = existing.helper.take().unwrap();
 1802              helper.kill().unwrap();
 1803              helper.reap().unwrap();
 1804              let stopped = existing.probe.stop_unit(&unit);
 1805              evidence(
 1806                  "H6",
 1807                  &format!("StopUnit reply (diagnostics only): {stopped:?}"),
 1808              );
 1809              assert!(
 1810                  wait_until(Duration::from_secs(10), || existing
 1811                      .probe
 1812                      .is_no_such_unit(&unit)),
 1813                  "{name}: {unit} is still loaded"
 1814              );
 1815              existing.done = true;
 1816              drop(existing);
 1817              scopes_back(name, &before);
 1818          }
```

## How it fails

**Case A, a foreign collision.** The first start (line 1774) is answered
`org.freedesktop.systemd1.UnitExists`: a unit of the generated name is
already loaded, so it is foreign (a random name makes this improbable, never
impossible). The answer is `Ok(Answer::Refused { .. })`, which line 1779
turns into a panic while `existing` is alive and `done` is false. Unwinding
drops it: lines 1748-1751 kill and reap the harness's helper, and line 1752
calls `StopUnit` on the name, stopping the foreign unit with no authority
but the string.

**Case B, an uncertain first start.** The first start times out, the
connection breaks, the reply does not decode, the manager answers another
error, or the call panics after dispatch: the request may still take
effect, its start job may still attach the helper's process id. Line 1779
(or the panic itself) unwinds through the same `Drop`: the helper is killed
**and reaped** (lines 1749-1750) while the request is unresolved, so its
process id is free for reuse by any process, and `StopUnit` is then sent by
name (line 1752) for a unit that may be foreign (a lost `UnitExists`) or not
yet created. Nothing owns the operation afterwards.

The accepted production owner (I4-R1) closed exactly these two patterns: a
collision is never stopped, killed or claimed, and an unresolved operation's
helper stays unreaped (`scope/pending.rs`, `execution.rs`).

## Reproduction on the deterministic model

`scripts/reproduce_old_h6.sh` takes a scratch snapshot of 3d46b4d, appends
`reproduction/old_h6_on_the_model.rs` to the crate's `src/scope/tests.rs`
(the model of the user manager and the kernel the accepted unit tests use:
no bus, no cgroup, no live sandbox), runs only those tests, and writes the
file back byte for byte (status clean). The transliteration keeps old H6
statement for statement, with the model's manager in place of the probe
(`start_transient_scope` is `start_scope` for the same unit and helper,
`stop_unit` is `stop_unit`). Output: `reproduction/run.out`.

| Case | World | What old H6 did (observed) |
|---|---|---|
| A | a foreign unit is loaded under the generated name, holding a foreign process | panicked on `Collision` (`UnitExists`); then `calls: [Start(name, pid), Stop(name)]`: the foreign unit stopped by name, its cgroup killed and removed, the unit unloaded, its process's membership now `… (deleted)` |
| B | the start creates the unit, its reply times out, its job attaches the helper only later; StopUnit times out too | panicked on `Uncertain("StartTransientUnit timed out")`; the helper reaped (`unreaped: false`) while the unit is loaded and its start job still targets that process id (`Some((pid, 0, 2))`) |
| C | as B, the call panicking after its effect | the same: reaped while its start job still targets the process id |

The same worlds through the repaired first start (`execution::place`, test D
of the reproduction): (A) the foreign unit is never stopped, killed or
claimed (`calls: [Start, Membership(pid, true)]`), only the harness's own
helper is ended; (B) the unit is discovered when the job attaches the
helper, proven, then ended by its owner; (C) the operation is settled
through its candidate (killed, observed empty) before the helper is reaped.
Every membership read was of the unreaped helper.

Residual (test E, a scratch-only model extension, never in a candidate): a
collision whose `UnitExists` reply is lost is, to the backend, an uncertain
start; the accepted settling then attempts `StopUnit` for its own generated
name, which here a foreign unit holds. This is accepted production
semantics (I4-R1), not the harness's; the repaired H6 inherits it and adds
no stop path of its own (`matrices/ownership-model.md`, "Residual").

## The run (excerpt of `reproduction/run.out`)

```text
reproduction of the P2-V1-R3B-I4-Q1 H6 finding, 2026-10-03T03:29:36Z
base: 3d46b4dbe6b920dfcc2a1fa0c2d3b055f86c1823 (snapshot tree d2bafa9b81edca6d5b07c3a23d4f2546404abede)
reproduction source sha256: c4407848a5b239c568030682b506fbe69cb75f42045b1fc978a4dbcd9f7d28c3
scope/tests.rs sha256 before: 56f6ed239e78d336d3e961eedf6feb3626d94356421a081f3790132add10cd54
scratch model extension applied (reproduction E)
    old H6 panicked: h6: StartTransientUnit of the fresh name: Collision
    calls: [Start("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope", 1895731), Stop("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope")]
    foreign unit loaded after old H6: false; its cgroup killed: true, removed: true; its process now in: Some("/user.slice/user-1000.slice/user@1000.service/app.slice/nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope (deleted)")
REPRODUCED (A): a first start answered UnitExists made old H6 stop the foreign unit by name
    old H6 panicked: h6: StartTransientUnit of the fresh name: Uncertain("StartTransientUnit timed out")
    calls: [Start("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope", 1895733), Stop("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope")]
    after old H6: helper 1895733 unreaped: false; unit nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope loaded: true; its start job still to attach: Some((1895733, 0, 2))
REPRODUCED (B, the reply lost): old H6 reaped the helper while its first start could still act
    old H6 panicked: a panic inside the model's Start
    calls: [Start("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope", 1895735), Stop("nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope")]
    after old H6: helper 1895735 unreaped: false; unit nexus-verifier-q1h6-0123456789abcdef0123456789abcdef.scope loaded: true; its start job still to attach: Some((1895735, 0, 2))
REPRODUCED (C, a panic after dispatch): old H6 reaped the helper while its first start could still act
REPAIRED (A): the foreign unit was never stopped, killed or claimed; the harness's own helper was ended
REPAIRED (B): the helper was reaped only after the operation was proven or settled
REPAIRED (C): the helper was reaped only after the operation was proven or settled
RESIDUAL (E): StopUnit attempts for the generated name: 1; the foreign unit is still loaded: false; the operation stays retained with its helper unreaped
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 117 filtered out; finished in 0.32s
cargo test exit: 0
scope/tests.rs sha256 after restore: 56f6ed239e78d336d3e961eedf6feb3626d94356421a081f3790132add10cd54 (identical: yes)
checkout status entries after restore: 0
```
