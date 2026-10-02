# P2-V1-R3B-I4 remote-operation state matrix

Every outcome of every remote operation the scope module issues, what the
operation may have done, which independent observation decides, and the
resulting ownership state. Line references are to the candidate's sources
(`crates/nexus-verifier-sandbox/src/`). "Settle" is
`PendingScope::reconcile` (`scope/pending.rs:153`); its two confirmation
rules are:

- **R2 (candidate held)**: the cgroup the kernel reported the bound helper
  in is retained by descriptor and reads Empty or Removed
  (`scope/pending.rs:220-228`, `scope.rs:270`). The helper was seen inside,
  so the start job has run; exactly the proven scope's rule.
- **R1 (no candidate)**: GetUnit on the issuing connection, after the
  request, answers `NoSuchUnit`, and then the kernel reports the bound,
  unreaped helper in a cgroup not of the unit's name
  (`scope/pending.rs:393-406`). Membership that cannot be read or parsed is
  never "outside".

A StopUnit outcome is recorded (`last_stop`, diagnostics) and never used as
evidence (`scope/pending.rs:176-182`).

## StartTransientUnit (`scope/pending.rs:241-283`)

| Outcome (`scope/manager.rs:182-219`) | What may have happened | Decided by | Resulting state | Reported | Tests |
|---|---|---|---|---|---|
| Delivered (a job path) | unit created; helper being attached | the helper's membership (placement), then the candidate's proofs | Proven when every proof passes; otherwise Pending with candidate, settled by R2 | launch, or `NotRun::Scope(..)` after settling | I4-01, I4-12..15, I4-19 |
| `UnitExists` | nothing: refused before any effect; the loaded unit is not this request's | the helper's membership only | Pending (collided): confirmed once the helper is in a cgroup not of that name; that unit is never stopped, opened or killed | `NotRun::Scope(Bus)` + confirmed, or `CleanupFailed` | I4-17 |
| Any other error reply | unknown | GetUnit (issuing connection) + membership | absence proven (R1) → nothing retained; otherwise discovered and proven, or settled | `Bus` with confirmed cleanup, launch, or `CleanupFailed` | I4-02, I4-03, I4-29 |
| Timeout | unknown | same | same | same | I4-02, I4-03 |
| Malformed reply | unknown | same | same | same | I4-02, I4-03, I4-18 |
| Disconnection | unknown; the issuing connection can answer nothing more | the candidate only (R2) | confirmed through a retained candidate; otherwise retained (no other connection answers for this one) | `CleanupFailed` when no candidate | I4-03, I4-21 |

## Placement and the candidate (`scope/pending.rs:290-337`, `411-430`)

| Observation | Resulting state | Tests |
|---|---|---|
| membership names the unit (last component, no `..`) | candidate opened by that path once and retained at once (`scope/pending.rs:300`), before any later check | I4-01, I4-04, I4-10, I4-26 |
| never placed within the bound | `NotPlaced` (after an uncertain start: the start's `Bus`); settled by R1 (StopUnit, then GetUnit + membership) | I4-05, I4-06, I4-16 |
| placed elsewhere, or a path with `..` | `NotPlaced`; nothing opened | I4-15 |
| the cgroup is not cgroup v2 / cannot be opened | `Mismatch` / `Io`; no candidate; R1 needed | I4-11 |
| not populated, helper not in `cgroup.procs`, a limit file differs | `Mismatch`; candidate retained; R2 | I4-12, I4-15 |
| membership reads `... (deleted)` (cgroup removed) | not the unit's: outside | I4-11 (retry) |

## Manager proofs (`scope/pending.rs:316-336`)

| Call | Outcome | Resulting state | Tests |
|---|---|---|---|
| GetUnit | Present (object path) | continue | I4-01 |
| GetUnit | `NoSuchUnit` while the kernel shows the helper in a cgroup of the name | conflicting observations: `Mismatch("unit not loaded")`; candidate retained; R2 | I4-11 |
| GetUnit | uncertain | `Bus`; candidate retained; R2 | I4-10, I4-18 |
| RuntimeMaxUSec / OOMPolicy | exact | continue / Proven | I4-01 |
| RuntimeMaxUSec / OOMPolicy | another value, or a value of another type | `Mismatch`; never launched | I4-13, I4-14 |
| RuntimeMaxUSec / OOMPolicy | uncertain | `Bus`; candidate retained; R2 | I4-04, I4-18 |

## StopUnit while settling (`scope/pending.rs:176-196`)

| Reply | Effect | Decided by | Resulting state | Tests |
|---|---|---|---|---|
| delivered | stopped and emptied | R2 / R1 observed again | confirmed | I4-05, I4-22 |
| delivered | cgroup still populated | R2 observed again | **not** confirmed: retained | I4-07 |
| error | none | R1 / R2 observed again | retained | I4-06 |
| timeout | stopped (cgroup removed) | R2 observed again | confirmed | I4-08 |
| timeout | stopped (unit gone, never placed) | R1 observed again | confirmed | I4-08 |
| timeout | none, target populated | R2 observed again | retained | I4-09 |
| any | any | a collided operation is never stopped | — | I4-17 |

## The base's counterexamples, at the candidate

`baseline/model-output.txt` is the source-derived model of the base. Each
of its scenarios, and what the candidate does:

| Base scenario | Base result | Candidate | Tests |
|---|---|---|---|
| start delivered, placed, proof ok | proven | proven | I4-01 |
| start had no effect, reply lost | `SandboxUnavailable`, confirmed (correct by luck: nothing observed) | absence proven by R1 (GetUnit on the issuing connection + membership); nothing retained | I4-02 |
| start took effect (placed), reply lost | ownerless `Bus`, confirmed (COUNTEREXAMPLE b) | discovered through membership and proven | I4-03 |
| start took effect (not placed), reply lost | ownerless `Bus`, unit survives, confirmed (COUNTEREXAMPLE a) | GetUnit Present → not absent; never placed → StopUnit, then R1; retained while the unit remains | I4-03, I4-06, I4-21 |
| not placed; stop took effect, reply lost | confirmed unobserved (COUNTEREXAMPLE b) | R1 after the stop: confirmed by observation | I4-05, I4-08 |
| not placed; stop had no effect, reply lost | unit survives, confirmed (COUNTEREXAMPLE a) | R1 fails: retained, `CleanupFailed` | I4-06, I4-09 |
| placed; property proof uncertain; stop took effect, reply lost | confirmed unobserved (COUNTEREXAMPLE b) | candidate retained; R2 | I4-04, I4-10 |
| placed; property proof uncertain; stop had no effect, reply lost | confirmed unobserved (COUNTEREXAMPLE b) | candidate retained; R2 decides; retained while populated | I4-04, I4-09 |
