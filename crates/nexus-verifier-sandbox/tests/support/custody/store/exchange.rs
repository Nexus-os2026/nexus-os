//! The exchange between the owner thread and the recorder worker (design
//! section 9): bounded, idempotent, state-based (no message queue that could
//! fill, overflow or drop), with one fatal latch that records its first cause
//! and the durable position P at that instant. Submission ([`ExchangeSink`]),
//! the owner's apply step and failure delivery ([`Recorder::apply`]) and the
//! store admission gate ([`Recorder::admit`]) live here; the worker's steps
//! are in [`super::recorder`].
//!
//! Rules kept here:
//! - one mutex, held only for a copy or an assignment, never across I/O, a
//!   callback, a wait or a join; its one longer use is the admission gate,
//!   which holds it across one `Custody::admit` call, a call that performs no
//!   I/O, callback or wait (lock order: the exchange mutex, then the core's
//!   own locks);
//! - a poisoned mutex is never treated as usable: the first acquisition that
//!   finds it poisoned latches `Poisoned`, and what is read afterwards serves
//!   only to report;
//! - `durable_through` is set only by the worker's W6, and only while nothing
//!   is latched, so no sequence beyond P is ever published or acknowledged
//!   (INV-4);
//! - each sequence is acknowledged to the core exactly once, from the owner's
//!   own retained intent; any outcome but `Acknowledged` latches and stops
//!   application for good;
//! - the core is told of a failure only at its next unacknowledged record,
//!   only once that record is issued, and once;
//! - admission is refused at once from the latch on, without waiting for any
//!   record (INV-13);
//! - the seal is requested at most once, only from the actual closure of the
//!   custody the owner retains (see [`super::owner`]): no caller supplies the
//!   closure or the record count.
//!
//! Visibility (P2-V1-R3B-I3-I1-R1): the exchange, its state, its guard, its
//! sink, the recorder and the worker handle are internal to the store
//! (`pub(super)`). An external caller acts only through
//! [`super::owner::StoreOwner`] and [`super::owner::ClosedStore`], and reads
//! only copies ([`RecorderStatus`], [`Applied`], [`ClaimState`],
//! [`SealState`]). The narrow, fail-closed fault operations the tests use are
//! in [`super::faults`]. Nothing here defends against `unsafe` code in the
//! same process.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use super::super::{
    AckOutcome, AdmitRefused, Custody, FlushError, Generation, OpTicket, RecordAck, RecordId,
    RecordIntent, RecordSink, Reservation, Resource, Tick,
};
use super::classify::Grammar;
use super::format::{self, Header, PrefixFacts, BLOCK};
use super::io::Errno;

/// Why a submission was invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// Before the claim was published.
    BeforeClaim,
    /// Another generation's record.
    Foreign,
    /// Sequence zero, or beyond the capacity.
    OutOfRange,
    /// A sequence after the next one: a gap, never stored silently.
    Future,
}

/// A fatal recorder condition (design section 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The claim's header write failed.
    ClaimWrite,
    /// The claim's sync failed.
    ClaimSync(Errno),
    /// The claim's identity recheck failed.
    ClaimIdentity,
    Encode(u64),
    Write(u64),
    Sync(u64, Errno),
    Identity(u64),
    SealWrite,
    SealSync(Errno),
    /// The worker unwound (its drop guard).
    WorkerLost,
    /// The worker's thread finished without a recorded normal exit.
    WorkerVanished,
    InvalidSubmission(u64, Invalid),
    /// A submission for an already submitted sequence with another digest,
    /// acknowledged or not.
    Conflict(u64),
    /// The core answered an acknowledgement with this outcome.
    UnexpectedAck(u64, AckOutcome),
    /// A panic while the exchange mutex was held.
    Poisoned,
}

/// The claim's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimState {
    None,
    Requested(Box<[u8; BLOCK]>),
    Claimed,
    Failed,
}

/// The seal's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealState {
    None,
    Requested(Box<[u8; BLOCK]>),
    Sealed,
    Failed,
}

/// How the worker ended, when it ended normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerExit {
    /// After its seal was published or failed.
    Sealed,
    /// After a latch: it stopped all writing.
    Latched,
    /// The owner asked it to stop while idle (the process is ending).
    Stopped,
}

/// The exchange's state, under its one mutex (design section 9.2). Internal
/// to the store: only the designated transitions here and in the worker
/// change it.
#[derive(Debug)]
pub(super) struct ExchangeState {
    pub(super) generation: Generation,
    pub(super) capacity: u64,
    slots: Vec<Option<RecordIntent>>,
    pub(super) submitted_through: u64,
    pub(super) claim: ClaimState,
    pub(super) seal: SealState,
    pub(super) durable_through: u64,
    pub(super) fatal: Option<(Cause, u64)>,
    pub(super) later_causes: u64,
    pub(super) pending: Option<(u64, Tick)>,
    pub(super) worker_exit: Option<WorkerExit>,
    pub(super) stop: bool,
    poison_noted: bool,
}

impl ExchangeState {
    /// Latching is one critical section: the first cause records itself and
    /// P = `durable_through`, and a requested claim or seal fails; any later
    /// cause is only counted. Never cleared.
    pub(super) fn latch(&mut self, cause: Cause) {
        if self.fatal.is_none() {
            self.fatal = Some((cause, self.durable_through));
            if matches!(self.claim, ClaimState::Requested(_)) {
                self.claim = ClaimState::Failed;
            }
            if matches!(self.seal, SealState::Requested(_)) {
                self.seal = SealState::Failed;
            }
        } else {
            self.later_causes = self.later_causes.saturating_add(1);
        }
    }

    /// The first time the mutex is found poisoned, `Poisoned` is latched.
    fn note_poison(&mut self) {
        if !self.poison_noted {
            self.poison_noted = true;
            self.latch(Cause::Poisoned);
        }
    }

    /// The intent submitted for `seq`, if any.
    pub(super) fn slot(&self, seq: u64) -> Option<&RecordIntent> {
        if seq == 0 {
            return None;
        }
        self.slots.get((seq - 1) as usize).and_then(Option::as_ref)
    }
}

/// The exchange: its state, the worker's wake-up hint and the owner's.
/// Internal to the store.
pub(super) struct Exchange {
    state: Mutex<ExchangeState>,
    work: Condvar,
    owner: Condvar,
    clock: Arc<dyn Fn() -> Tick + Send + Sync>,
}

/// One acquisition of the exchange mutex. `poisoned` is true when the mutex
/// was found poisoned (`Poisoned` is then latched): the state may be read
/// only to report. Internal to the store.
pub(super) struct Held<'a> {
    pub(super) state: MutexGuard<'a, ExchangeState>,
    pub(super) poisoned: bool,
}

impl Exchange {
    /// The exchange of one claimed generation: `capacity` slots, allocated
    /// once and never grown.
    pub(super) fn new(
        generation: Generation,
        capacity: u32,
        clock: Arc<dyn Fn() -> Tick + Send + Sync>,
    ) -> Arc<Exchange> {
        Arc::new(Exchange {
            state: Mutex::new(ExchangeState {
                generation,
                capacity: u64::from(capacity),
                slots: vec![None; capacity as usize],
                submitted_through: 0,
                claim: ClaimState::None,
                seal: SealState::None,
                durable_through: 0,
                fatal: None,
                later_causes: 0,
                pending: None,
                worker_exit: None,
                stop: false,
                poison_noted: false,
            }),
            work: Condvar::new(),
            owner: Condvar::new(),
            clock,
        })
    }

    /// Acquire the mutex, checking for poison: the first acquisition that
    /// finds it poisoned latches `Poisoned`.
    pub(super) fn acquire(&self) -> Held<'_> {
        match self.state.lock() {
            Ok(state) => Held {
                state,
                poisoned: false,
            },
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.note_poison();
                Held {
                    state,
                    poisoned: true,
                }
            }
        }
    }

    pub(super) fn now(&self) -> Tick {
        (self.clock)()
    }

    /// Latch `cause` in one critical section.
    pub(super) fn latch(&self, cause: Cause) {
        self.acquire().state.latch(cause);
        self.work.notify_all();
        self.owner.notify_all();
    }

    /// Wake the worker (a hint: a missed one is harmless).
    pub(super) fn wake_worker(&self) {
        self.work.notify_all();
    }

    /// Wake the owner (a hint).
    pub(super) fn wake_owner(&self) {
        self.owner.notify_all();
    }

    /// The worker waits for work: the mutex is released while it waits, and
    /// the wait is bounded so it rechecks the state; a timeout is never a
    /// result.
    pub(super) fn wait_for_work<'a>(&'a self, held: Held<'a>) -> Held<'a> {
        let Held { state, poisoned } = held;
        match self.work.wait_timeout(state, Duration::from_millis(20)) {
            Ok((state, _)) => Held { state, poisoned },
            Err(error) => {
                let (mut state, _) = error.into_inner();
                state.note_poison();
                Held {
                    state,
                    poisoned: true,
                }
            }
        }
    }

    /// The owner waits for a change of the claim or the seal state, bounded
    /// by `rounds` short waits. Used only before the claim (the owner holds
    /// no custody then) and at the end, after `close()`.
    pub(super) fn wait_owner<'a>(&'a self, held: Held<'a>) -> Held<'a> {
        let Held { state, poisoned } = held;
        match self.owner.wait_timeout(state, Duration::from_millis(20)) {
            Ok((state, _)) => Held { state, poisoned },
            Err(error) => {
                let (mut state, _) = error.into_inner();
                state.note_poison();
                Held {
                    state,
                    poisoned: true,
                }
            }
        }
    }

    /// Fault injection ([`super::faults::poison`] only): poison the mutex, as
    /// a panic in a critical section would. The store's code never panics
    /// while it holds the mutex. This can only make the recorder fail.
    pub(super) fn poison(self: &Arc<Exchange>) {
        let exchange = Arc::clone(self);
        let _ = std::thread::spawn(move || {
            let _held = exchange.state.lock();
            panic!("fixture: a panic while the exchange mutex is held");
        })
        .join();
    }
}

// ---------------------------------------------------------------------------
// Submission (design section 9.3)
// ---------------------------------------------------------------------------

/// The core's record sink: an idempotent, non-blocking insertion into the
/// exchange, apart from one short critical section. It also keeps the
/// owner's own copy of every stored intent, from which acknowledgements are
/// built. Internal to the store: only the retained custody's own flush
/// submits through it (and the fault interface, which can only submit what
/// the sink refuses or already holds).
pub(super) struct ExchangeSink<'a> {
    recorder: &'a mut Recorder,
}

impl RecordSink for ExchangeSink<'_> {
    fn submit(&mut self, intent: &RecordIntent) {
        let exchange = Arc::clone(&self.recorder.exchange);
        let stored = {
            let mut held = exchange.acquire();
            submit_locked(&mut held, intent)
        };
        if stored {
            let seq = intent.id.seq();
            if self.recorder.retained.len() as u64 == seq - 1 {
                self.recorder.retained.push(intent.clone());
            }
            exchange.wake_worker();
        }
    }
}

/// Whether a submission of `intent` now would be stored as the next record
/// (rather than refused, latched or recognized as a duplicate). The fault
/// interface uses it to refuse injecting anything but an invalid,
/// conflicting or duplicate submission.
pub(super) fn stores_as_new(held: &Held<'_>, intent: &RecordIntent) -> bool {
    let state = &held.state;
    let seq = intent.id.seq();
    !held.poisoned
        && state.fatal.is_none()
        && state.claim == ClaimState::Claimed
        && intent.id.generation() == state.generation
        && seq != 0
        && seq <= state.capacity
        && seq == state.submitted_through + 1
}

/// The conditions of design section 9.3, in order, under the mutex. Returns
/// whether the intent was stored.
fn submit_locked(held: &mut Held<'_>, intent: &RecordIntent) -> bool {
    let state = &mut held.state;
    if held.poisoned || state.fatal.is_some() {
        return false;
    }
    let seq = intent.id.seq();
    if state.claim != ClaimState::Claimed {
        state.latch(Cause::InvalidSubmission(seq, Invalid::BeforeClaim));
        return false;
    }
    if intent.id.generation() != state.generation {
        state.latch(Cause::InvalidSubmission(seq, Invalid::Foreign));
        return false;
    }
    if seq == 0 || seq > state.capacity {
        state.latch(Cause::InvalidSubmission(seq, Invalid::OutOfRange));
        return false;
    }
    if seq <= state.submitted_through {
        let same = state
            .slot(seq)
            .is_some_and(|stored| stored.digest == intent.digest);
        if !same {
            state.latch(Cause::Conflict(seq));
        }
        return false;
    }
    if seq != state.submitted_through + 1 {
        state.latch(Cause::InvalidSubmission(seq, Invalid::Future));
        return false;
    }
    state.slots[(seq - 1) as usize] = Some(intent.clone());
    state.submitted_through = seq;
    true
}

// ---------------------------------------------------------------------------
// The owner's side: apply, failure delivery, the admission gate (9.4-9.8)
// ---------------------------------------------------------------------------

/// Where failure delivery stands (design section 9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Nothing is latched.
    NotNeeded,
    /// A cause is latched, and the core's next unacknowledged record is not
    /// issued yet: nothing could truthfully be failed now.
    Waiting,
    /// `record_failed` at this record returned `FailureRecorded`.
    Delivered { target: u64 },
    /// The core's ledger had already failed: nothing more is reported.
    AlreadyFailed,
    /// `record_failed` returned another outcome: an integration fault, never
    /// retried and never counted as delivery.
    IntegrationFault { target: u64, outcome: AckOutcome },
}

/// What one apply step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// The owner's copy of `durable_through` (at most P after a latch).
    pub delivered: u64,
    /// Sequences the core has accepted acknowledgements for.
    pub applied_through: u64,
    pub fatal: Option<(Cause, u64)>,
    pub delivery: Delivery,
}

/// Why the store admission gate refused.
#[derive(Debug)]
pub enum GateRefused {
    /// A fatal cause is latched: refused at once, without calling the core's
    /// admission. The reservation is returned; it stays Reserved in the core
    /// until the case ends (`NotAdmitted`).
    RecorderFatal {
        reservation: Reservation,
        cause: Cause,
    },
    /// The core refused.
    Core(AdmitRefused),
}

/// A copy of the recorder's status for the control side (design section
/// 14.1): a stall (a pending record, nothing latched) and a fatal condition
/// (latched, with P) are kept apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderStatus {
    pub submitted_through: u64,
    pub durable_through: u64,
    pub pending: Option<(u64, Tick)>,
    pub fatal: Option<(Cause, u64)>,
    pub later_causes: u64,
    pub claimed: bool,
    pub claim_failed: bool,
    pub sealed: bool,
    pub seal_failed: bool,
    pub worker_exit: Option<WorkerExit>,
}

impl RecorderStatus {
    /// A stall: a record pending, nothing latched. Never a failure.
    pub fn stalled(&self) -> bool {
        self.fatal.is_none() && self.pending.is_some()
    }
}

/// A thread running a worker, and how the owner checks it. Internal to the
/// store.
pub(super) struct WorkerHandle {
    join: Option<std::thread::JoinHandle<()>>,
}

impl WorkerHandle {
    pub(super) fn new(join: std::thread::JoinHandle<()>) -> WorkerHandle {
        WorkerHandle { join: Some(join) }
    }

    /// No thread was started.
    pub(super) fn none() -> WorkerHandle {
        WorkerHandle { join: None }
    }

    pub(super) fn is_finished(&self) -> bool {
        self.join
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
    }

    /// Join a finished thread (never called under any lock).
    pub(super) fn join(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Why a seal was not requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealWithheld {
    /// A cause is latched: the seal is withheld (design section 9.7).
    Latched(Cause),
    /// The custody's records are not all durable and acknowledged.
    NotAllDurable { durable: u64, records: u64 },
    /// The claim was not published.
    NotClaimed,
    /// The recorded prefix does not satisfy the grammar (never expected).
    Grammar,
    /// A seal was already requested, published or failed: the transition is
    /// one-way and happens once.
    AlreadyRequested,
}

/// The owner's recorder handle: its retained intents, its applied position,
/// failure delivery and the admission gate. It lives on the owner thread
/// beside the custody, inside [`super::owner::StoreOwner`]; it is internal to
/// the store.
pub(super) struct Recorder {
    exchange: Arc<Exchange>,
    retained: Vec<RecordIntent>,
    applied_through: u64,
    stopped: bool,
    delivery: Delivery,
    worker: Option<WorkerHandle>,
}

impl Recorder {
    pub(super) fn new(exchange: Arc<Exchange>) -> Recorder {
        Recorder {
            exchange,
            retained: Vec::new(),
            applied_through: 0,
            stopped: false,
            delivery: Delivery::NotNeeded,
            worker: None,
        }
    }

    pub(super) fn exchange(&self) -> &Arc<Exchange> {
        &self.exchange
    }

    /// The thread running the worker (the owner's join-handle check).
    pub(super) fn attach_worker(&mut self, handle: WorkerHandle) {
        self.worker = Some(handle);
    }

    pub(super) fn sink(&mut self) -> ExchangeSink<'_> {
        ExchangeSink { recorder: self }
    }

    /// The owner's retained copies of the intents it submitted.
    pub(super) fn retained(&self) -> &[RecordIntent] {
        &self.retained
    }

    pub(super) fn applied_through(&self) -> u64 {
        self.applied_through
    }

    pub(super) fn delivery(&self) -> Delivery {
        self.delivery
    }

    /// Request the claim (design section 9.7), after activation. A cause
    /// latched before the request fails it at once.
    pub(super) fn request_claim(&self, header: Box<[u8; BLOCK]>) {
        {
            let mut held = self.exchange.acquire();
            if held.poisoned || held.state.fatal.is_some() {
                held.state.claim = ClaimState::Failed;
            } else if held.state.claim == ClaimState::None {
                held.state.claim = ClaimState::Requested(header);
            }
        }
        self.exchange.wake_worker();
    }

    pub(super) fn claim_state(&self) -> ClaimState {
        self.exchange.acquire().state.claim.clone()
    }

    pub(super) fn seal_state(&self) -> SealState {
        self.exchange.acquire().state.seal.clone()
    }

    /// The owner waits (bounded, `rounds` short waits) until the claim is no
    /// longer requested. Only before the claim, when the owner holds no
    /// custody and serves no request.
    pub(super) fn wait_claim(&self, rounds: u32) -> ClaimState {
        let mut held = self.exchange.acquire();
        for _ in 0..rounds {
            if !matches!(held.state.claim, ClaimState::Requested(_)) {
                break;
            }
            held = self.exchange.wait_owner(held);
        }
        held.state.claim.clone()
    }

    /// Wait (bounded) until the seal is no longer requested.
    pub(super) fn wait_seal(&self, rounds: u32) -> SealState {
        let mut held = self.exchange.acquire();
        for _ in 0..rounds {
            if !matches!(held.state.seal, SealState::Requested(_)) {
                break;
            }
            held = self.exchange.wait_owner(held);
        }
        held.state.seal.clone()
    }

    /// Wait (bounded) until `durable_through` reaches `seq` or a cause is
    /// latched. The owner never needs this to stay responsive; tests use it.
    pub(super) fn wait_durable(&self, seq: u64, rounds: u32) -> u64 {
        let mut held = self.exchange.acquire();
        for _ in 0..rounds {
            if held.state.durable_through >= seq || held.state.fatal.is_some() {
                break;
            }
            held = self.exchange.wait_owner(held);
        }
        held.state.durable_through
    }

    /// `flush_records` through this recorder's sink, then the apply step.
    pub(super) fn flush<R: Resource>(
        &mut self,
        custody: &mut Custody<R>,
        now: Tick,
    ) -> Result<Applied, FlushError> {
        let flushed = custody.flush_records(&mut self.sink(), now);
        let applied = self.apply(custody, now);
        flushed.map(|_| applied)
    }

    /// The owner's apply step (design section 9.4), at a safe point; it
    /// never blocks. Then failure delivery while a cause is latched.
    pub(super) fn apply<R: Resource>(&mut self, custody: &mut Custody<R>, now: Tick) -> Applied {
        let vanished = {
            let finished = self.worker.as_ref().is_some_and(WorkerHandle::is_finished);
            finished && self.exchange.acquire().state.worker_exit.is_none()
        };
        if vanished {
            self.exchange.latch(Cause::WorkerVanished);
        }
        let (delivered, mut fatal) = {
            let held = self.exchange.acquire();
            (held.state.durable_through, held.state.fatal)
        };
        if !self.stopped {
            while self.applied_through < delivered {
                let seq = self.applied_through + 1;
                let Some(intent) = self.retained.get((seq - 1) as usize) else {
                    self.exchange
                        .latch(Cause::UnexpectedAck(seq, AckOutcome::NotIssued));
                    self.stopped = true;
                    break;
                };
                let outcome = custody.acknowledge(&RecordAck::of(intent), now);
                if outcome == AckOutcome::Acknowledged {
                    self.applied_through = seq;
                } else {
                    self.exchange.latch(Cause::UnexpectedAck(seq, outcome));
                    self.stopped = true;
                    break;
                }
            }
        }
        if fatal.is_none() {
            fatal = self.exchange.acquire().state.fatal;
        }
        if fatal.is_some() {
            self.deliver_failure(custody, now);
        }
        Applied {
            delivered,
            applied_through: self.applied_through,
            fatal,
            delivery: self.delivery,
        }
    }

    /// Failure delivery (design section 9.5): only at the core's next
    /// unacknowledged record, only once that record is issued, and once.
    fn deliver_failure<R: Resource>(&mut self, custody: &mut Custody<R>, now: Tick) {
        if matches!(
            self.delivery,
            Delivery::Delivered { .. }
                | Delivery::AlreadyFailed
                | Delivery::IntegrationFault { .. }
        ) {
            return;
        }
        let evidence = custody.snapshot().evidence.clone();
        if evidence.failed.is_some() {
            self.delivery = Delivery::AlreadyFailed;
            return;
        }
        let target = evidence.acknowledged + 1;
        if target > evidence.issued {
            self.delivery = Delivery::Waiting;
            return;
        }
        // A record the core really issued: its own generation, the sequence
        // the snapshot shows as its next unacknowledged one.
        let id = RecordId {
            generation: custody.generation(),
            seq: target,
        };
        let outcome = custody.record_failed(id, now);
        self.delivery = if outcome == AckOutcome::FailureRecorded {
            Delivery::Delivered { target }
        } else {
            Delivery::IntegrationFault { target, outcome }
        };
    }

    /// The store admission gate (design section 9.8): the health check and
    /// `Custody::admit` in one critical section of the exchange mutex, the
    /// mutex every latch takes. An admission completes before a latch, or
    /// starts after it and is refused, at once.
    pub(super) fn admit<R: Resource>(
        &mut self,
        custody: &mut Custody<R>,
        reservation: Reservation,
        now: Tick,
    ) -> Result<OpTicket, GateRefused> {
        let held = self.exchange.acquire();
        if let Some((cause, _)) = held.state.fatal {
            return Err(GateRefused::RecorderFatal { reservation, cause });
        }
        if held.poisoned {
            return Err(GateRefused::RecorderFatal {
                reservation,
                cause: Cause::Poisoned,
            });
        }
        let admitted = custody.admit(reservation, now);
        drop(held);
        admitted.map_err(GateRefused::Core)
    }

    /// The recorder's status, copied (a short critical section).
    pub(super) fn status(&self) -> RecorderStatus {
        status_of(&self.exchange)
    }

    /// The latched cause and P, if any.
    pub(super) fn latched(&self) -> Option<(Cause, u64)> {
        self.exchange.acquire().state.fatal
    }

    /// Request the seal (design section 9.7). Called only by
    /// [`super::owner::StoreOwner::close`], right after `close()` on the
    /// custody that owner retains returned `Ok`, with that result's record
    /// count; never with a caller's data. Only with every record durable and
    /// acknowledged, only while nothing is latched, and only once: otherwise
    /// the seal is withheld.
    pub(super) fn request_seal(
        &self,
        header: &Header,
        records: u64,
        closed_ms: u64,
    ) -> Result<(), SealWithheld> {
        let facts = {
            let held = self.exchange.acquire();
            if held.state.seal != SealState::None {
                return Err(SealWithheld::AlreadyRequested);
            }
            if let Some((cause, _)) = held.state.fatal {
                return Err(SealWithheld::Latched(cause));
            }
            if held.state.claim != ClaimState::Claimed {
                return Err(SealWithheld::NotClaimed);
            }
            let durable = held.state.durable_through;
            if durable != records || self.applied_through != records {
                return Err(SealWithheld::NotAllDurable { durable, records });
            }
            durable
        };
        let prefix = prefix_facts(&self.retained[..facts as usize], header)?;
        let seal = format::encode_seal(&format::seal_for(header, &prefix, closed_ms));
        {
            let mut held = self.exchange.acquire();
            if held.state.seal != SealState::None {
                return Err(SealWithheld::AlreadyRequested);
            }
            if let Some((cause, _)) = held.state.fatal {
                return Err(SealWithheld::Latched(cause));
            }
            held.state.seal = SealState::Requested(seal);
        }
        self.exchange.wake_worker();
        Ok(())
    }

    /// The process is ending: the worker stops at its next idle point (never
    /// in the middle of a write or a sync).
    pub(super) fn stop_worker(&mut self) {
        self.exchange.acquire().state.stop = true;
        self.exchange.wake_worker();
    }

    /// Join the worker's thread once it finished (never under a lock).
    pub(super) fn join_worker(&mut self) {
        if let Some(handle) = self.worker.as_mut() {
            if handle.is_finished() {
                handle.join();
            }
        }
    }
}

/// A recorder going away (its owner or closed store dropped, or the owner
/// process ending) asks a threaded worker to stop at its next idle point:
/// never in the middle of a write or a sync, and after any requested seal.
/// The worker's thread holds the guard until then.
impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

/// The status copy.
pub(super) fn status_of(exchange: &Exchange) -> RecorderStatus {
    let held = exchange.acquire();
    let state = &held.state;
    RecorderStatus {
        submitted_through: state.submitted_through,
        durable_through: state.durable_through,
        pending: state.pending,
        fatal: state.fatal,
        later_causes: state.later_causes,
        claimed: state.claim == ClaimState::Claimed,
        claim_failed: state.claim == ClaimState::Failed,
        sealed: state.seal == SealState::Sealed,
        seal_failed: state.seal == SealState::Failed,
        worker_exit: state.worker_exit,
    }
}

/// The prefix facts of the records the owner retained, by the grammar.
fn prefix_facts(records: &[RecordIntent], header: &Header) -> Result<PrefixFacts, SealWithheld> {
    let mut grammar = Grammar::new(
        Generation::new(header.generation),
        header.applied.len() as u32,
    );
    for intent in records {
        let evidence = super::super::codec::RecordEvidence {
            id: intent.id,
            at: intent.at,
            kind: intent.kind,
            digest: intent.digest,
        };
        grammar.step(&evidence).map_err(|_| SealWithheld::Grammar)?;
    }
    let summary = grammar.summary();
    Ok(PrefixFacts {
        records: records.len() as u64,
        last_digest: records
            .last()
            .map(|intent| intent.digest)
            .unwrap_or([0; 32]),
        run_ended: summary.run_ended,
        run_started: summary.run_started,
        unsettled: summary.unsettled(),
    })
}
