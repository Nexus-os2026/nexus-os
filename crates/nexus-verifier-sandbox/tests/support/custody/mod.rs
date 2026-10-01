//! The live harness's custody core (P2-V1-R3B-I1, corrected by -R1 and
//! -R2): the reusable logic the future custody owner keeps its native owners,
//! operations, evidence and failures in. It is not that owner, and not a
//! service.
//!
//! What it is: ownership-preserving slots for one resource group (a native
//! process tree, the workspace and the fixture it uses) with dependency
//! ordering; action reservation, admission and cancellation ordering;
//! fail-stop admission (the first actual failure closes execution admission
//! for good, including an expected condition the moment it can no longer be
//! met); explicit pending and unknown native outcomes; expected injected
//! conditions kept apart from actual failures; output loss kept apart from
//! native completion; bounded cleanup-attempt accounting; recovery-required
//! state and explicit later recovery; a finalization boundary (completion
//! candidate, terminal commitment with its terminal record issued, terminal
//! record acknowledged) whose commitment point is ordered against every
//! cancellation in the gate's critical section, after which the run's
//! outcome is fixed, a cancellation is late and changes nothing, and late
//! owners are incidents of their own; immutable status snapshots; evidence
//! acknowledgements and record-capacity reservations; and refusal of closure
//! while anything native or any required evidence is unresolved or failed.
//!
//! What it is not: it opens no socket, makes no systemd or bus call, spawns
//! nothing, stores no record, stages no artifact and initializes no host. Its
//! adapters ([`Cleanup`], [`RecordSink`], [`DispositionValidator`] and the
//! native operations whose results [`Custody::complete`] takes) are
//! deliberately unimplemented here; the fixture controls use in-process
//! stand-ins at those boundaries. Nothing here is durable storage: a record is
//! durable only when the recorder adapter acknowledges it, and this module
//! takes that acknowledgement as the adapter's claim.
//!
//! Public API by phase. Phases: NotStarted, Running (with or without a case),
//! RecoveryRequired, Candidate (a completion candidate: not yet committed),
//! Finalizing (terminal commitment made, terminal record issued), Finalized
//! (terminal record acknowledged), Closed. "Terminal" below means the
//! commitment is made and the terminal record issued (Finalizing or
//! Finalized). Every refusal changes nothing (beyond publishing a snapshot)
//! and returns any owner or reservation it was given.
//!
//! | Method | Valid phases | Mutation | Ownership | Evidence required | Terminal |
//! |---|---|---|---|---|---|
//! | `Control::status` | any | none | none | none | read-only |
//! | `Control::cancel` | any | before the commitment: closes admission, first closure wins (`Accepted`, else `AlreadyClosed`); the run cannot pass | none | none | `Late`: closes nothing, changes nothing |
//! | `Control::renew_lease` | any, with the run's `LeaseCap` | renews an unexpired lease; an expired one is lost and cancels as `check_lease` does | none | none | same |
//! | `Control::check_lease` | any | an expired lease is lost and, before the commitment, closes admission (a cancellation) | none | none | the lease is lost; nothing else changes |
//! | `Control::submit` | any | queues one request | none | none | same |
//! | `apply_disposition` | NotStarted | one prior incident dispositioned | none | none | refused |
//! | `start_run` | NotStarted, priors resolved, admission open | Running | none | room for the run's records | refused |
//! | `begin_case` | Running, no case, not halted | a case | none | every earlier record acknowledged; room | refused |
//! | `reserve` | Running with a case, admission open | an action reserved | none | room for its 3 records | refused |
//! | `admit` | Running, its own reservation | the gate decides | reservation returned or settled | start record acknowledged | refused |
//! | `complete` | any but Closed | resolves its own open operation only | accepts owners; refused owners returned | room for a late incident | late owner: incident and fault, verdict unchanged |
//! | `lend` | Running, RecoveryRequired | none; a panic is a failure | lends `&mut` | none | refused |
//! | `record_assertion_failure` | Running, RecoveryRequired, Candidate | a failure (fail-stop) | none | none | refused |
//! | `end_entry` | Running with a case | cleanup attempts; an expected retained boundary's unconfirmed first attempt fails the case at once (fail-stop) | releases confirmed owners | none | refused |
//! | `end_case` | Running with a case | cleanup (the same first-attempt rule), judgement, may stop the run | releases confirmed owners | reserved at `begin_case` | refused |
//! | `finish_run` | Running, no case | stops the run; may commit | none | none | refused |
//! | `observe_control` | any | records a closure; outside a case and before the commitment a cancellation becomes the run's failure; stops an idle run | none | none | records only |
//! | `serve` (retry) | RecoveryRequired; or terminal with late incidents | one attempt per held owner; may commit | releases confirmed owners | attempt records reserved at start | incident recovery only |
//! | `serve` (shutdown) | any | closes admission (a cancellation) only while Running; otherwise decides only, never commits; first refusal recorded | none | none | decision only |
//! | `take_released` | any | none | returns released owners | none | same |
//! | `shutdown_decision` | any | none | none | reports what is unresolved | same |
//! | `close` | Finalized (or NotStarted, untouched), nothing unresolved | Closed | returns released owners | every record acknowledged, none failed | the only way to close |
//! | `flush_records` | any | submits records; a panic is a failure | none | none | a panic is a custody fault |
//! | `acknowledge` | any | exact, in-order acknowledgement; the last earlier record's commits (the commitment point, terminal record issued), the terminal record's finalizes | none | exact binding | acknowledges later records |
//! | `record_failed` | any | evidence fails for good (fail-stop) | none | none | a custody fault; closure refused for good |
//!
//! Adapter contracts:
//!
//! - Native operations: an operation starts only with the [`OpTicket`]
//!   [`Custody::admit`] returned, and reports one bound result per ticket. A
//!   later result for the same ticket, after the operation retired, is held
//!   as a late incident (or returned) and never applied. A timeout is
//!   [`NativeOutcome::Unknown`]; only that operation's own later completion
//!   resolves it.
//! - Cleanup: bounded attempts on a lent owner, each fact reported
//!   separately, output completeness apart from output detachment (see
//!   [`Cleanup`], which also states the contract a wrapper for a consuming
//!   production cleanup call must meet).
//! - Recorder: acknowledges each issued record exactly, in order, only once it
//!   is durable, or reports the first record it could not make durable.
//! - Disposition validator: the records layer's verdict on one prior
//!   incident; nothing else can disposition one.
//!
//! Limits of this pure core:
//!
//! - Panics: a callback's unwinding panic (and a panic in dropping its
//!   payload) is contained at the callback boundary and recorded as a
//!   failure. An abort, a panic while panicking, a payload whose drop panics
//!   again, stack overflow, allocation failure and process death are not
//!   contained.
//! - Mutable borrows: owners are lent by `&mut`. That does not stop a cleanup
//!   adapter or a borrower from replacing or altering an owner's contents,
//!   and custody cannot detect it.
//!
//! Integration obligations (none is met here):
//!
//! - Destruction: this module does not preserve owners when its enclosing
//!   process (or the [`Custody`] value) is destroyed. The integration never
//!   drops a custody that holds anything, and keeps the owner process alive
//!   while closure is refused, including after a recording failure, which
//!   refuses closure for good.
//! - Durability: records are made durable (write-ahead) by the recorder, not
//!   here.
//! - Transport: peers are authenticated outside this module; a generation or
//!   sequence number is a freshness binding, never authentication.
//! - Threads: one execution-owner thread owns the [`Custody`]; other threads
//!   use [`Control`] only.
//! - Lease: the core reads no clock. An expired lease cancels the run only
//!   once it is observed (by `Control::check_lease`, or a refused
//!   `Control::renew_lease`), and only before the terminal commitment; the
//!   integration observes it in time.
//! - Owner identity: an owner type's constructor is reachable only from its
//!   native adapter, and its adapters and borrowers never replace an owner
//!   they are lent; each generation is unique to one owner process.
//! - Returned owners: an owner refused by [`Custody::complete`] (another
//!   generation's ticket, an earlier instant, or no room for another late
//!   incident) is held by no record here (a late one is still counted as a
//!   failure); the integration must keep and dispose of it.
//! - Unresolvable states: lost authority, an exhausted recovery budget, an
//!   unknown outcome that never completes and failed evidence leave the
//!   custody unable to close. This core implements no disposition writer,
//!   administrative bypass or forced destruction for them.

// Shared fixture support: not every target that includes it uses every item.
#![allow(dead_code)]

mod core;
mod model;

pub use self::core::*;
pub use self::model::*;
