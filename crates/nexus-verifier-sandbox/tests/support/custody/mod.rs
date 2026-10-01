//! The live harness's custody core (P2-V1-R3B-I1): the reusable logic the
//! future custody owner keeps its native owners, operations, evidence and
//! failures in. It is not that owner, and not a service.
//!
//! What it is: ownership-preserving slots for one resource group (a native
//! process tree, the workspace and the fixture it uses) with dependency
//! ordering; action reservation, admission and cancellation ordering;
//! explicit pending and unknown native outcomes; expected injected conditions
//! kept apart from real failures; bounded cleanup-attempt accounting;
//! recovery-required state and explicit later recovery; immutable status
//! snapshots; evidence acknowledgements and record-capacity reservations; and
//! refusal of shutdown or new work while required state is unresolved.
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
//! Adapter contracts:
//!
//! - Native operations: an operation starts only with the [`OpTicket`]
//!   [`Custody::admit`] returned, and reports exactly one bound result per
//!   ticket (a later result for the same ticket is held as an unexpected owner
//!   or rejected as stale, never applied). A timeout is
//!   [`NativeOutcome::Unknown`]; only that operation's own later completion
//!   resolves it.
//! - Cleanup: bounded attempts on a borrowed owner, each fact reported
//!   separately (see [`Cleanup`], which also states the contract a wrapper for
//!   a consuming production cleanup call must meet).
//! - Recorder: acknowledges each issued record exactly, in order, only once it
//!   is durable, or reports the first record it could not make durable.
//! - Disposition validator: the records layer's verdict on one prior
//!   incident; nothing else can disposition one.
//!
//! Integration obligations (none is met here):
//!
//! - Destruction: this module does not preserve owners when its enclosing
//!   process (or the [`Custody`] value) is destroyed. The integration never
//!   drops a custody that holds anything, keeps the owner process alive while
//!   shutdown is refused, and leaves a destroyed owner's incident to the next
//!   generation's records and an external disposition.
//! - Durability: records are made durable (write-ahead) by the recorder, not
//!   here.
//! - Transport: peers are authenticated outside this module; a generation or
//!   sequence number is a freshness binding, never authentication.
//! - Threads: one execution-owner thread owns the [`Custody`]; other threads
//!   use [`Control`] only.
//! - Owner identity: an owner type's constructor is reachable only from its
//!   native adapter, so that no borrower or cleanup adapter can swap a
//!   replacement into the `&mut` it is lent (the core cannot detect a swap),
//!   and each generation is unique to one owner process.
//! - Authority loss: an operation that ended without the facts confirming its
//!   end leaves its run requiring recovery for good; only the destruction of
//!   the owner and an external disposition in a later generation end it.

// Shared fixture support: not every target that includes it uses every item.
#![allow(dead_code)]

mod core;
mod model;

pub use self::core::*;
pub use self::model::*;
