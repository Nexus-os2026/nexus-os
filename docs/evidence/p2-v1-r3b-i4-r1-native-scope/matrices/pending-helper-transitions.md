# P2-V1-R3B-I4-R1: Pending / Helper ownership transitions

One execution's (or one direct placement's) state is a pair: the scope
boundary (None, Pending, Proven) and the helper (unreaped child, or reaped).
Both live in one owner at every moment: the execution's `Owned`
(`execution.rs:479`), the harness's `ScopedHelper` (`execution.rs:235-238`),
or the `RetainedBoundary` either hands them to (`execution.rs:176-179`).
No transition takes a unit name, a process id or a path. Citations are to
`crates/nexus-verifier-sandbox/src/`.

| # | From | To | Where | Condition | Helper | Tests |
|---|---|---|---|---|---|---|
| T0 | — | helper owned | `execution.rs:599-615` (`owned.helper.insert`); `execution.rs:262-265` (consumed by `place`) | spawned with an identity allocated first (`launcher.rs:220`) | owned before any scope exists | `i4_28`, `i4r1_21` |
| T1 | None | Pending (not issued) | `execution.rs:633-636`; `execution.rs:290` | `prepare` gave a fresh nonce (`scope.rs:177-189`); stored in the owner before the request | owned beside it | `i4_28`, `i4_a_pending_operation_is_bound_to_its_own_helper` |
| T2 | Pending | Pending (issued) | `scope/pending.rs:262` | set before StartTransientUnit is sent: from here the request may have an effect | bound by pid and identity (`scope/pending.rs:150-152`) | `i4_25`, `i4r1_01` |
| T3 | Pending (issued) | Pending (accepted) | `scope/pending.rs:270-273` | only in the arm of a delivered, decoded job path; a panic before it leaves the outcome uncertain | — | `i4r1_01`, NC-I4R1-X-ACCEPTED-EARLY |
| T3' | Pending (issued) | Pending (collided) | `scope/pending.rs:274-279` | `UnitExists`: the request had no effect; the unit is not this request's | — | `i4r1_15`, `i4r1_16` |
| T4 | Pending | Pending + candidate | `scope/pending.rs:311` (proof), `scope/pending.rs:212-227` (settling) | the kernel reports the bound helper in a cgroup whose path is absolute, in normal form and named for the unit (`scope/pending.rs:420-425`); retained by descriptor at once | unreaped | `i4_10`, `i4r1_14`, `i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate` |
| T5 | Pending + candidate | Proven | `scope/pending.rs:485-502` | every proof passed, the binding included (`scope/pending.rs:301-363`); in place, so the owner never lets go | unreaped; now launchable | `i4r1_10`, `i4_01` |
| T6 | Pending | gone (settled) | `scope/pending.rs:164-207`, called only by `end` (`execution.rs:731-761`) | collided and the helper outside; or the candidate Empty/Removed; or, after an accepted start only, `NoSuchUnit` then the helper outside (`scope/pending.rs:231-242`) | killed first (`execution.rs:737-739`); reaped only after (`execution.rs:747-753`) | `i4_05`, `i4_08`, `i4_20`, `i4r1_14` |
| T7 | Pending, or Proven not confirmed | retained | `execution.rs:503-509` (`retain`) | settling or the proven scope's wait not confirmed, or a panic in the finalizer (`execution.rs:694-697`) | moved with the scope into one `RetainedBoundary`; never reaped while the operation is unresolved | `i4_06`, `i4_21`, `i4r1_02`, `i4r1_11`-`i4r1_13` |
| T8 | retained | gone | `execution.rs:186-196` (`retry`) | the same steps (`end`) confirm it, after the `ScopeManager` is gone too | reaped only then | `i4_22`, `i4_23`, `i4r1_19` |
| T9 | retained (uncertain, no candidate) | retained | `scope/pending.rs:239` | no answer of the manager confirms it: not `NoSuchUnit`, not any StopUnit reply | killed, unreaped (a zombie) for the backend's lifetime unless a candidate appears | `i4r1_11`-`i4r1_13`, `i4_21`, `i4_29` |
| T10 | Proven | gone | `execution.rs:731-761` | `cgroup.kill`, the helper killed and reaped, then Empty/Removed through the descriptor | reaped | `i4_19`, `i4r1_18` |
| T11 | Proven (harness) | Proven, helper reaped | `execution.rs:308-316` | only while the scope is proven (the scope-hold sequence) | reaped by its owner | `i4r1_23` |
| T12 | any | dropped | `execution.rs:766-774` (`end_now`), `scope/pending.rs:246-252` | defense in depth only: what the candidate or proven scope holds is killed, nothing waited for, the manager not asked | killed; reaped only if no operation is unresolved (`execution.rs:770-772`) | `i4_24`, `i4r1_03` |

What cannot happen (each closed by a test and a control):

- a pending operation held anywhere but beside its helper: there is no
  public type, constructor or failure that carries one
  (`direct-public-api-ownership.md`; NC-I4R1-SPLIT-OWNER,
  NC-I4R1-HARNESS-WEAK-OWNER);
- a panic leaving a pending operation in a local that is dropped
  (NC-I4-PANIC-OWNER-LOSS, NC-I4R1-PANIC-DROPS-PENDING);
- Pending to gone by the helper's death (NC-I4-PID-CLEANUP), a reply
  (NC-I4-STOP-OK-CONFIRMS, NC-I4R1-STOP-REPLY-ABSENCE) or, after an
  uncertain start, the manager's absence (NC-I4R1-ABSENCE-GETUNIT);
- the helper reaped while its operation is unresolved
  (NC-I4R1-REAP-UNRESOLVED, NC-I4-X-DROP-REAPS).
