# P2-V1-R3B-I4-R1: panic / fault matrix (mission section 14)

The invariant: a panic produces either confirmed cleanup, or one retained
owner containing every still-unconfirmed native boundary (the operation, if
unresolved, with its unreaped helper); never a panic and a dropped pending
operation. No production source resumes a panic (SG-I4R1-NO-RESUME-UNWIND).

Injection: a deterministic fault at a production fault point
(`fault.rs:72`), or a panic inside the simulated manager or kernel layer
(`scope::tests`, `Op`), once. `i4r1_x_a_panic_at_any_ownership_point_leaves_confirmation_or_one_owner`
runs every point and every simulated panic below for an accepted and an
uncertain start, each with and without a candidate, through an execution
and through the harness's direct owner, and judges each result by that
invariant (confirmed: no simulated cgroup populated and the helper reaped;
otherwise: the retained boundary holds the unreaped helper and, if
unresolved, the operation). Citations are to
`crates/nexus-verifier-sandbox/src/`.

| Boundary | Injection | Execution (`run`) | Harness owner (`place`) | Tests |
|---|---|---|---|---|
| Start request dispatch | `BeforeScopeStart`; a panic inside StartTransientUnit after its effect (`Op::Start`) | before: nothing issued, the helper ended, confirmed; inside: the issued, unaccepted operation stays in `Owned` and the finalizer settles or retains it | `place` catches (`execution.rs:266`), `end` settles or retains both (`execution.rs:320-330`) | `i4_28`, `i4r1_01`, matrix |
| Immediately after dispatch | `AfterScopeStart` | the reply is not recorded, so the start is uncertain (`scope/pending.rs:270-273`); a candidate is found or the operation is retained | as for `run` | `i4_25`, `i4r1_01`, NC-I4R1-X-ACCEPTED-EARLY, matrix |
| Native membership | `Op::Membership` | the operation stays owned; settling observes again | as for `run` | matrix |
| Candidate open | `ScopeProof` (before), `Op::Open` (inside), `ScopeCandidate` (after) | before or inside: the candidate is found again through the membership; after: it is retained already (`scope/pending.rs:311`) | as for `run` | `i4_26`, matrix |
| Identity / ControlGroup proof | `ScopeBinding`; `Op::GetUnit`, `Op::UnitId`, `Op::ControlGroup` | the candidate is retained; no launch; confirmed through it | as for `run` | `i4r1_17`, matrix |
| Resource / property proof | `ScopeProperties`; `Op::RuntimeMax` | as above | as above | `i4_26`, matrix |
| StopUnit | `BeforeScopeStop`, `AfterScopeStop`; `Op::Stop` | the finalizer's panic is caught (`execution.rs:694-697`); everything retained | `end` catches; retained | `i4_27`, `i4r1_01`, matrix |
| Pending reconciliation | `ScopeReconcile` | as above | as above | `i4_27`, `i4r1_01`, matrix |
| Direct / harness owner path | every simulated panic above through `place` | — | never unwinds; `PlacementFailed { error: None, cleanup }`: confirmed or one boundary of both | `i4r1_01`, `i4r1_02`, NC-I4R1-PANIC-DROPS-PENDING, NC-I4R1-HARNESS-WEAK-OWNER, matrix |
| Execution finalization | `Finalizing` | retained with the proven scope and the helper | — | matrix, `p2r1_a_failed_or_panicking_finalization_retains_the_live_boundary` |
| RetainedBoundary retry | `Op::Stop` during a retry | — | `retry` catches (`execution.rs:186-196`): `Err(boundary)` still holds both; a later retry confirms | matrix |

The accepted P2-R1/I4 panic controls are unchanged and still run:
NC-I4-PANIC-OWNER-LOSS (`i4_25`), and the live harness's `p2r1` cases
(built, not run here).
