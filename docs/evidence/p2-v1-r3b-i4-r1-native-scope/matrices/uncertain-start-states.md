# P2-V1-R3B-I4-R1: uncertain-start state matrix

StartTransientUnit's outcome, whether a native candidate is ever retained,
and what the manager answers later, against the result. "Delivered" means
a decoded job path that was recorded (`scope/pending.rs:270-273`);
anything else (a timeout, an error reply other than `UnitExists`, a reply
that does not decode, a broken transport, a panic before the reply was
recorded) is uncertain. Citations are to `crates/nexus-verifier-sandbox/src/`.

## The rule (`scope/pending.rs:231-242`)

| Candidate retained | Start | Confirmed gone when | Never confirmed by |
|---|---|---|---|
| yes | any | the retained descriptor reads Empty or Removed | a StopUnit reply; the helper's death; a unit name |
| no | delivered | GetUnit answers `NoSuchUnit` (sent after the reply arrived, so after the request was handled; systemd unloads no unit with a pending job or running processes, `systemd.unit(5)`), then the kernel reports the helper outside any cgroup of the name (`scope/pending.rs:438-443`) | the manager's absence alone (NC-I4-MANAGER-ABSENCE-ONLY); a StopUnit reply |
| no | uncertain | nothing: it stays Pending, its helper unreaped, `CleanupFailed`, possibly for the backend's lifetime (`scope/pending.rs:239`) | `NoSuchUnit` with the helper outside (A-R1-5); any StopUnit reply; a timeout |
| — | `UnitExists` | the kernel reports the helper outside any cgroup of the name (`scope/pending.rs:170-176`) | — (the foreign unit is never stopped, opened, killed or proven) |

## The section 11 cases, and every combination tested

| # | Start | Helper placed | Later | Result | Tests |
|---|---|---|---|---|---|
| 1 | delivered | yes | every proof, the binding included | Proven, then launched | `i4r1_10`, `i4_01`, `i4r1_18` |
| 2 | delivered | yes | a later proof fails or is uncertain (limits, Id, ControlGroup, properties) | no launch; the candidate ended and confirmed through its descriptor, or retained while it stays populated | `i4_04`, `i4_12`-`i4_14`, `i4r1_06`-`i4r1_09`, `i4r1_19`, `i4_07`, `i4_09` |
| 2' | delivered | no | StopUnit, then `NoSuchUnit` and the helper outside | confirmed (no candidate needed after a delivered reply) | `i4_05`, `i4_08`, `i4_22`, `i4_23` |
| 2'' | delivered | no | the unit stays (StopUnit fails, times out) | retained, retried later | `i4_06`, `i4_22` |
| 3 | uncertain | yes (at once or later while the proof waits) | every proof | Proven: the candidate is acquired and proven like an accepted one | `i4_03`, `i4r1_14` |
| 3' | uncertain | yes | a later proof fails (another control group; the connection lost) | no launch; confirmed through the candidate | `i4r1_14`, `i4_03` |
| 3'' | uncertain | only after the execution ended (a retry finds the helper in the unit's cgroup) | the candidate acquired, ended, Empty | confirmed by the retry | `i4r1_14` |
| 4 | uncertain | never | `NoSuchUnit`, helper outside | Pending, `CleanupFailed`, helper unreaped; a retry after the manager answers again still retains it | `i4r1_11`, `i4_02`, `i4_29`, `i4_18` |
| 4' | uncertain | never | StopUnit takes effect, its reply delivered | still Pending | `i4r1_12` |
| 4'' | uncertain | never | StopUnit times out (with or without effect) | still Pending; helper never reaped | `i4r1_13`, `i4_21` |
| 4''' | delivered, but a panic before the reply was recorded | never | anything | treated as uncertain: Pending | `i4r1_01`, `i4_25` |
| 5 | `UnitExists` | the helper outside, or in the foreign unit's cgroup | — | the request created nothing; the foreign unit is never stopped or claimed; confirmed once the helper is outside | `i4r1_15`, `i4r1_16`, `i4_17` |
| 6 | not attempted: preparation failed | — | — | an ordinary no-scope failure: nothing was issued (`execution.rs:633-636`); the helper reaped | by construction; the not-issued case `i4_28` (a panic before the request: nothing asked of the manager) |

Not relied on: any systemd-specific ordering of a later call after an
earlier, unanswered one. Messages between two peers keep their order, but
the recipient need not process or answer concurrently issued calls in that
order (primary sources: `baseline/primary/excerpts.md`). Whether systemd
offers a fence that would let an uncertain start without a candidate be
confirmed is left to G-HOST/G-LIVE.
