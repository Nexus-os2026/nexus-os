//! The store's owner (P2-V1-R3B-I3-I1-R1): the one object that retains,
//! together, the custody, its recorder, the claimed journal's header and
//! worker, and the exclusion guard; and the closed store that remains once
//! that custody closed. Design sections 9.7, 9.9 and 10.2 to 10.3.
//!
//! Ownership and authority:
//! - **One constructor.** [`start_owner`] is the only way to obtain a
//!   [`StoreOwner`]. It opens the store, applies only the dispositions that
//!   opening verified (through [`super::open::Opened::validator`]), claims a
//!   journal through the opening's own claim, and builds the recorder and
//!   the worker over that claim. Every field is internal to the store.
//! - **No substitution.** The custody, the recorder, the claimed header and
//!   journal and the guard live in one value and are never handed out:
//!   there is no `&mut Custody`, no field to replace and no setter. The
//!   custody's operations are forwarded one by one. `Custody::admit` is
//!   reachable only through the store admission gate. `apply_disposition`,
//!   `flush_records`, `acknowledge`, `record_failed` and `close` are not
//!   forwarded: the store performs them itself.
//! - **Closure authorizes the seal.** [`StoreOwner::close`] consumes the
//!   owner and calls the core's own `close` on the custody it retains. On
//!   refusal the same owner comes back, with that custody, every owner it
//!   holds and the guard. On `Ok`, and only then, it requests the seal of its
//!   own claimed journal with that result's record count. No caller supplies
//!   closure data, a record count, a header or a journal.
//! - **One-way.** The seal is requested at most once: [`ClosedStore`] has no
//!   close and no seal request, and the recorder refuses a second request
//!   ([`super::exchange::SealWithheld::AlreadyRequested`]).
//! - **Exclusion.** The guard is shared with the worker's thread when the
//!   worker runs on one. The store lock and the journal lock are released
//!   only once the owner (or the closed store) and the worker are both gone.
//!   Dropping either asks the worker to stop at its next idle point, never
//!   in the middle of a write or a sync.
//! - **Failure stays.** A successful close does not erase an earlier failure.
//!   The core's verdict and failures are in [`ClosedStore::closed`], and the
//!   records already written carry them.
//!
//! Limits: this is a safe-Rust API boundary. It does not defend against
//! `unsafe` code in the same process. It does not resolve G-AUTH: the
//! journal is still writable by the store uid. The core's own types stay
//! public: `Closed`, `ValidatedDisposition`, `Custody::new` and the
//! `DispositionValidator` trait. A caller may build and run its own custody,
//! and the store's guarantees concern only the custody it retains.

use std::sync::Arc;

use super::super::{
    CaseId, CaseOutcome, Cleanup, Closed, Closure, Completion, Config, Control, Custody, EntryId,
    Expectation, FlushError, Generation, IncidentBinding, LeaseCap, LendError, NativeOutcome,
    OpTicket, PriorOutcome, RecordIntent, RecordSink, Refusal as CoreRefusal, Rejected, Released,
    Request, RequestOutcome, Reservation, Resource, RunPhase, ShutdownDecision, SlotKind, Snapshot,
    Tick,
};
use super::exchange::{
    Applied, Cause, ClaimState, Delivery, Exchange, GateRefused, Recorder, RecorderStatus,
    SealState, SealWithheld,
};
use super::format::{parse_header, Header, HeaderParse};
use super::open::{
    open_owner, OpeningHooks, Platform, ProvisionPath, Refusal, Refused, StartupReport, StoreGuard,
};
use super::recorder::{spawn_worker, Step, Worker, WorkerHooks, WorkerPoint};

/// Where the owner's worker is: here, to be stepped by the owner (the
/// deterministic tests), or on its own thread.
pub(super) enum WorkerSlot<P: Platform> {
    Stepped(Box<Worker<P>>),
    Threaded,
}

/// An owner started through the real core and the store. See the module
/// documentation for what it retains and what it never hands out.
pub struct StoreOwner<R: Resource, P: Platform> {
    pub(super) custody: Custody<R>,
    pub(super) control: Control,
    pub(super) recorder: Recorder,
    pub(super) worker: WorkerSlot<P>,
    pub(super) guard: Arc<StoreGuard<P>>,
    pub(super) header: Header,
    pub(super) claim: u64,
    pub(super) index: u32,
    pub(super) generation: Generation,
    pub(super) report: StartupReport,
    /// Fault injection ([`super::faults::panic_next_submission`]): the next
    /// flush's sink panics after storing its first record.
    pub(super) panic_next_submission: bool,
}

/// Why an owner did not start.
#[derive(Debug)]
pub enum StartRefused {
    /// The opening or the claim refused.
    Store(Refusal),
    /// The core refused the configuration or the priors.
    Core(CoreRefusal),
    /// After every verified disposition was applied, these incidents still
    /// block: the NotStarted custody was closed.
    PriorUnresolved(Vec<[u8; 32]>),
}

/// A sink that panics after storing its first record, once (fault
/// injection; the core contains the panic as a recorder fault).
struct PanicAfterStoring<S: RecordSink> {
    inner: S,
    armed: bool,
}

impl<S: RecordSink> RecordSink for PanicAfterStoring<S> {
    fn submit(&mut self, intent: &RecordIntent) {
        self.inner.submit(intent);
        if self.armed {
            self.armed = false;
            panic!("fault injection: the sink panics after storing");
        }
    }
}

fn step_slot<P: Platform>(slot: &mut WorkerSlot<P>) -> Option<Step> {
    match slot {
        WorkerSlot::Stepped(worker) => Some(worker.step()),
        WorkerSlot::Threaded => None,
    }
}

fn run_slot<P: Platform>(slot: &mut WorkerSlot<P>) -> Option<Step> {
    match slot {
        WorkerSlot::Stepped(worker) => Some(worker.run_until_idle()),
        WorkerSlot::Threaded => None,
    }
}

impl<R: Resource, P: Platform + 'static> StoreOwner<R, P> {
    // -- read-only views --------------------------------------------------

    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// The claimed pool file's index.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The claim number the header records.
    pub fn claim(&self) -> u64 {
        self.claim
    }

    /// The claimed journal's header, as the claim wrote it (read-only).
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The startup report (read-only).
    pub fn report(&self) -> &StartupReport {
        &self.report
    }

    /// The custody's control handle: status, cancellation, lease and
    /// requests, as the core defines them.
    pub fn control(&self) -> Control {
        self.control.clone()
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.custody.snapshot()
    }

    pub fn shutdown_decision(&self) -> ShutdownDecision {
        self.custody.shutdown_decision()
    }

    /// The recorder's status (a copy: it controls nothing).
    pub fn status(&self) -> RecorderStatus {
        self.recorder.status()
    }

    pub fn latched(&self) -> Option<(Cause, u64)> {
        self.recorder.latched()
    }

    pub fn claim_state(&self) -> ClaimState {
        self.recorder.claim_state()
    }

    pub fn seal_state(&self) -> SealState {
        self.recorder.seal_state()
    }

    /// The owner's copies of the intents it submitted (read-only).
    pub fn retained(&self) -> &[RecordIntent] {
        self.recorder.retained()
    }

    pub fn applied_through(&self) -> u64 {
        self.recorder.applied_through()
    }

    pub fn delivery(&self) -> Delivery {
        self.recorder.delivery()
    }

    /// Where the stepped worker acts next (`None` once it runs on a thread).
    pub fn worker_point(&self) -> Option<WorkerPoint> {
        match &self.worker {
            WorkerSlot::Stepped(worker) => Some(worker.point()),
            WorkerSlot::Threaded => None,
        }
    }

    // -- the retained custody's operations, forwarded --------------------

    pub fn start_run(&mut self, now: Tick) -> Result<LeaseCap, CoreRefusal> {
        self.custody.start_run(now)
    }

    pub fn begin_case(
        &mut self,
        case: CaseId,
        expectation: Expectation,
        now: Tick,
    ) -> Result<(), CoreRefusal> {
        self.custody.begin_case(case, expectation, now)
    }

    pub fn reserve(&mut self, kind: SlotKind, now: Tick) -> Result<Reservation, CoreRefusal> {
        self.custody.reserve(kind, now)
    }

    pub fn complete(
        &mut self,
        ticket: &OpTicket,
        outcome: NativeOutcome<R>,
        now: Tick,
    ) -> Result<Completion, Rejected<R>> {
        self.custody.complete(ticket, outcome, now)
    }

    pub fn lend<T>(
        &mut self,
        entry: EntryId,
        now: Tick,
        f: impl FnOnce(&mut R) -> T,
    ) -> Result<T, LendError> {
        self.custody.lend(entry, now, f)
    }

    pub fn record_assertion_failure(&mut self, detail: &str, now: Tick) -> Result<(), CoreRefusal> {
        self.custody.record_assertion_failure(detail, now)
    }

    pub fn end_entry<C: Cleanup<R>>(
        &mut self,
        entry: EntryId,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<bool, CoreRefusal> {
        self.custody.end_entry(entry, cleanup, now)
    }

    pub fn end_case<C: Cleanup<R>>(
        &mut self,
        cleanup: &mut C,
        now: Tick,
    ) -> Result<CaseOutcome, CoreRefusal> {
        self.custody.end_case(cleanup, now)
    }

    pub fn finish_run(&mut self, now: Tick) -> Result<RunPhase, CoreRefusal> {
        self.custody.finish_run(now)
    }

    pub fn observe_control(&mut self, now: Tick) -> Result<Option<Closure>, CoreRefusal> {
        self.custody.observe_control(now)
    }

    pub fn serve<C: Cleanup<R>>(
        &mut self,
        cleanup: &mut C,
        now: Tick,
    ) -> Option<(Request, RequestOutcome)> {
        self.custody.serve(cleanup, now)
    }

    pub fn take_released(&mut self) -> Vec<Released<R>> {
        self.custody.take_released()
    }

    // -- the recorder's transitions ---------------------------------------

    /// Submit the custody's issued records to the recorder, then the apply
    /// step (design sections 9.3 and 9.4).
    pub fn flush(&mut self, now: Tick) -> Result<Applied, FlushError> {
        let flushed = if std::mem::take(&mut self.panic_next_submission) {
            let mut sink = PanicAfterStoring {
                inner: self.recorder.sink(),
                armed: true,
            };
            self.custody.flush_records(&mut sink, now)
        } else {
            self.custody.flush_records(&mut self.recorder.sink(), now)
        };
        let applied = self.recorder.apply(&mut self.custody, now);
        flushed.map(|_| applied)
    }

    /// The owner's apply step and failure delivery (design sections 9.4 and
    /// 9.5).
    pub fn apply(&mut self, now: Tick) -> Applied {
        self.recorder.apply(&mut self.custody, now)
    }

    /// The store admission gate around `Custody::admit` (design section 9.8).
    pub fn admit(&mut self, reservation: Reservation, now: Tick) -> Result<OpTicket, GateRefused> {
        self.recorder.admit(&mut self.custody, reservation, now)
    }

    /// One step of the worker, on the owner's thread (`None` once the worker
    /// runs on its own thread).
    pub fn step_worker(&mut self) -> Option<Step> {
        step_slot(&mut self.worker)
    }

    /// The worker's steps until it is idle or ends, on the owner's thread.
    pub fn run_worker_until_idle(&mut self) -> Option<Step> {
        run_slot(&mut self.worker)
    }

    /// Run the worker on its own thread, which shares the guard (see the
    /// module documentation). `false` if it already runs on one.
    pub fn spawn_worker(&mut self) -> bool {
        self.spawn_with(None)
    }

    pub(super) fn spawn_with(&mut self, hooks: Option<Arc<dyn WorkerHooks>>) -> bool {
        let WorkerSlot::Stepped(worker) = std::mem::replace(&mut self.worker, WorkerSlot::Threaded)
        else {
            return false;
        };
        let handle = spawn_worker(*worker, hooks, Arc::clone(&self.guard));
        self.recorder.attach_worker(handle);
        true
    }

    /// Wait (bounded) for `durable_through` to reach `seq` or a latch. A
    /// timeout is never a failure and never an acknowledgement.
    pub fn wait_durable(&self, seq: u64, rounds: u32) -> u64 {
        self.recorder.wait_durable(seq, rounds)
    }

    /// Wait (bounded) until the claim is no longer requested.
    pub fn wait_claim(&self, rounds: u32) -> ClaimState {
        self.recorder.wait_claim(rounds)
    }

    /// Ask the worker to stop at its next idle point.
    pub fn stop_worker(&mut self) {
        self.recorder.stop_worker();
    }

    /// Join the worker's thread once it finished (never under a lock).
    pub fn join_worker(&mut self) {
        self.recorder.join_worker();
    }

    // -- closure ----------------------------------------------------------

    /// Close the custody this owner retains, with the core's own `close`. On
    /// refusal the same owner returns, with that custody, every owner it
    /// holds and the guard. On success, the seal of this owner's own claimed
    /// journal is requested, once, with that closure's record count; the
    /// result says whether it was requested or withheld, and why.
    pub fn close(self, now: Tick) -> Result<ClosedStore<R, P>, Box<StoreOwner<R, P>>> {
        let StoreOwner {
            custody,
            control,
            recorder,
            worker,
            guard,
            header,
            claim,
            index,
            generation,
            report,
            panic_next_submission,
        } = self;
        match custody.close(now) {
            Err(custody) => Err(Box::new(StoreOwner {
                custody: *custody,
                control,
                recorder,
                worker,
                guard,
                header,
                claim,
                index,
                generation,
                report,
                panic_next_submission,
            })),
            Ok(closed) => {
                let seal = recorder.request_seal(&header, closed.records, now.0);
                Ok(ClosedStore {
                    recorder,
                    worker,
                    guard,
                    header,
                    index,
                    generation,
                    closed,
                    seal,
                })
            }
        }
    }
}

/// What remains once the retained custody closed: the core's closure result
/// (output data), the recorder and the worker that write the seal, and the
/// guard that keeps the store excluded until they are done. It has no close
/// and no seal request: the transition happened once, in
/// [`StoreOwner::close`].
pub struct ClosedStore<R, P: Platform> {
    pub(super) recorder: Recorder,
    pub(super) worker: WorkerSlot<P>,
    pub(super) guard: Arc<StoreGuard<P>>,
    pub(super) header: Header,
    pub(super) index: u32,
    pub(super) generation: Generation,
    pub(super) closed: Closed<R>,
    pub(super) seal: Result<(), SealWithheld>,
}

impl<R, P: Platform + 'static> ClosedStore<R, P> {
    /// The core's closure result (read-only; it authorizes nothing).
    pub fn closed(&self) -> &Closed<R> {
        &self.closed
    }

    /// The owners the core released at closure.
    pub fn take_released(&mut self) -> Vec<Released<R>> {
        std::mem::take(&mut self.closed.released)
    }

    /// Whether the seal was requested at closure, or why it was withheld.
    pub fn seal_request(&self) -> &Result<(), SealWithheld> {
        &self.seal
    }

    pub fn seal_state(&self) -> SealState {
        self.recorder.seal_state()
    }

    pub fn status(&self) -> RecorderStatus {
        self.recorder.status()
    }

    pub fn latched(&self) -> Option<(Cause, u64)> {
        self.recorder.latched()
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn step_worker(&mut self) -> Option<Step> {
        step_slot(&mut self.worker)
    }

    pub fn run_worker_until_idle(&mut self) -> Option<Step> {
        run_slot(&mut self.worker)
    }

    /// Wait (bounded) until the seal is no longer requested.
    pub fn wait_seal(&self, rounds: u32) -> SealState {
        self.recorder.wait_seal(rounds)
    }

    pub fn stop_worker(&mut self) {
        self.recorder.stop_worker();
    }

    pub fn join_worker(&mut self) {
        self.recorder.join_worker();
    }
}

/// Open the store, then (design section 10.2 steps 9 and 10): draw the
/// generation; `Custody::new` with every current incident as a prior; apply
/// the dispositions this opening verified, through its own validator;
/// refuse if anything still blocks; check that the dispositions the custody
/// accepted are exactly the ones the opening derived; then the opening's own
/// claim handoff. No record exists before the claim, so a refusal leaves an
/// untouched NotStarted custody that closes.
pub fn start_owner<R: Resource, P: Platform + Clone + 'static>(
    io: &P,
    path: &ProvisionPath,
    config: &Config,
    now: Tick,
    clock: Arc<dyn Fn() -> Tick + Send + Sync>,
    hooks: &mut dyn OpeningHooks,
) -> Result<StoreOwner<R, P>, StartRefused> {
    let opened = open_owner(io, path, config, hooks).map_err(StartRefused::Store)?;
    let generation = opened.draw_generation(io).map_err(StartRefused::Store)?;
    let current = opened.current();
    let (mut custody, control) =
        Custody::<R>::new(*config, generation, opened.priors(), now).map_err(StartRefused::Core)?;
    {
        let validator = opened.validator();
        for (binding, _) in &current {
            let _ = custody.apply_disposition(&IncidentBinding::new(*binding), &validator, now);
        }
    }
    let snapshot = custody.snapshot();
    let blocking: Vec<[u8; 32]> = current
        .iter()
        .filter(|(binding, _)| {
            snapshot.prior.iter().any(|prior| {
                prior.binding == IncidentBinding::new(*binding)
                    && prior.outcome != PriorOutcome::Resolved
                    && prior.disposition.is_none()
            })
        })
        .map(|(binding, _)| *binding)
        .collect();
    if !blocking.is_empty() {
        let _ = custody.close(now);
        return Err(StartRefused::PriorUnresolved(blocking));
    }
    let revision = Some(opened.selection().revision());
    let refuse = |custody: Custody<R>, why: &str| {
        let _ = custody.close(now);
        StartRefused::Store(Refusal {
            refused: Refused::ClaimFailed(why.into()),
            revision,
        })
    };
    let applied = opened.applied();
    let accepted = snapshot
        .prior
        .iter()
        .filter(|prior| prior.disposition.is_some())
        .count();
    let same = applied.len() == accepted
        && applied.iter().all(|(binding, reason)| {
            snapshot.prior.iter().any(|prior| {
                prior.binding == IncidentBinding::new(*binding)
                    && prior.disposition == Some(*reason)
            })
        });
    if !same {
        return Err(refuse(custody, "dispositions"));
    }
    let report = opened.report();
    let claim = match opened.claim(io, config, generation) {
        Ok(claim) => claim,
        Err(refusal) => {
            let _ = custody.close(now);
            return Err(StartRefused::Store(refusal));
        }
    };
    let header = match parse_header(
        &claim.header_block[..],
        &claim.header.root_id,
        Some(claim.header.c_pool),
        Some(claim.index),
    ) {
        HeaderParse::Valid(header) => header,
        _ => return Err(refuse(custody, "header")),
    };
    let exchange = Exchange::new(generation, config.record_capacity, clock);
    let recorder = Recorder::new(Arc::clone(&exchange));
    let journals = Arc::clone(claim.guard.journals());
    let worker = Worker::new(
        io.clone(),
        claim.io_file,
        journals,
        claim.journal_name,
        claim.journal_stat,
        exchange,
    );
    recorder.request_claim(claim.header_block);
    drop(claim.selection);
    Ok(StoreOwner {
        custody,
        control,
        recorder,
        worker: WorkerSlot::Stepped(Box::new(worker)),
        guard: Arc::new(claim.guard),
        header,
        claim: claim.claim,
        index: claim.index,
        generation,
        report,
        panic_next_submission: false,
    })
}
