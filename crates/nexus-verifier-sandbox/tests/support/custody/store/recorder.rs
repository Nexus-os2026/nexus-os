//! The recorder worker (design sections 9.4 and 9.7) and the owner's start
//! through the real core (section 10.2 steps 9 and 10).
//!
//! The worker owns only the journal's I/O description (`O_RDWR`, on which no
//! lock is ever taken; the lock descriptions stay in the owner's
//! [`StoreGuard`]) and its own position. Its loop works on
//! `next = durable_through + 1`, one step at a time:
//! - W1, under the mutex: if nothing is latched and the slot is present,
//!   copy it and mark it pending;
//! - W2 to W5, with no mutex held: encode (the codec refuses a digest
//!   mismatch; 73 to 123 bytes), write the whole block (short counts looped,
//!   zero progress or `EINTR` retried at most three times), `fdatasync` on
//!   its own long-open description (`EINTR` retried at most three times, any
//!   other result final), then the identity recheck;
//! - W6, under the mutex: only if nothing is latched and the mutex is not
//!   poisoned, publish `durable_through = next`.
//!
//! A failure latches its cause and stops all writing. The claim and the seal
//! are written the same way. Steps are explicit so that a test can run the
//! worker, the owner's apply step, the admission gate and a latch in any
//! order without threads; the threaded runner calls the same steps. A
//! worker that unwinds latches `WorkerLost` through its drop guard.

use std::sync::Arc;

use super::super::codec;
use super::super::{
    Config, Control, Custody, DispositionReason, Generation, IncidentBinding, PriorOutcome,
    Resource, Tick,
};
use super::disposition::StoreValidator;
use super::exchange::{Cause, ClaimState, Exchange, Recorder, SealState, WorkerExit, WorkerHandle};
use super::format::{parse_header, Header, HeaderParse, BLOCK, BLOCK_U64};
use super::io::{sync_with_retries, write_fully, Stat, StoreIo, WriteFailure};
use super::open::{
    applied_reasons, open_owner, OpeningHooks, Platform, ProvisionPath, Refusal, Refused,
    StartupReport, StoreGuard,
};

/// Where the worker is about to act (a test's interleaving point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerPoint {
    /// W1, or picking up a claim or a seal request.
    Take,
    /// The claim's write, sync and recheck.
    ClaimIo,
    /// W2 to W5 for this record.
    Io(u64),
    /// W6 for this record.
    Publish(u64),
    /// The seal's write and sync.
    SealIo,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Progress,
    /// Nothing to do now.
    Idle,
    /// The worker ended normally.
    Exit(WorkerExit),
}

enum Phase {
    Idle,
    Claim(Box<[u8; BLOCK]>),
    Write(u64, Box<super::super::RecordIntent>),
    Publish(u64),
    Seal(Box<[u8; BLOCK]>),
    Done(WorkerExit),
}

/// The recorder worker of one claimed journal.
pub struct Worker<P: StoreIo> {
    io: P,
    file: P::File,
    journals: Arc<P::Dir>,
    name: String,
    identity: Stat,
    exchange: Arc<Exchange>,
    phase: Phase,
}

impl<P: StoreIo> Worker<P> {
    pub fn new(
        io: P,
        file: P::File,
        journals: Arc<P::Dir>,
        name: String,
        identity: Stat,
        exchange: Arc<Exchange>,
    ) -> Worker<P> {
        Worker {
            io,
            file,
            journals,
            name,
            identity,
            exchange,
            phase: Phase::Idle,
        }
    }

    /// The point the next step acts at.
    pub fn point(&self) -> WorkerPoint {
        match &self.phase {
            Phase::Idle | Phase::Done(_) => WorkerPoint::Take,
            Phase::Claim(_) => WorkerPoint::ClaimIo,
            Phase::Write(seq, _) => WorkerPoint::Io(*seq),
            Phase::Publish(seq) => WorkerPoint::Publish(*seq),
            Phase::Seal(_) => WorkerPoint::SealIo,
        }
    }

    fn finish(&mut self, exit: WorkerExit) -> Step {
        {
            let mut held = self.exchange.acquire();
            if held.state.worker_exit.is_none() {
                held.state.worker_exit = Some(exit);
            }
            held.state.pending = None;
        }
        self.exchange.wake_owner();
        self.phase = Phase::Done(exit);
        Step::Exit(exit)
    }

    fn fail(&mut self, cause: Cause) -> Step {
        self.exchange.latch(cause);
        self.finish(WorkerExit::Latched)
    }

    /// W5: the description still names a file with one link and its size,
    /// and the journal's name still names that inode.
    fn identity_ok(&self) -> bool {
        let Ok(now) = self.io.stat_file(&self.file) else {
            return false;
        };
        let Ok(entry) = self.io.stat_at(&self.journals, &self.name) else {
            return false;
        };
        now.nlink == 1
            && now.size == self.identity.size
            && now.same_inode(&self.identity)
            && entry.same_inode(&self.identity)
    }

    /// One step of the worker.
    pub fn step(&mut self) -> Step {
        match std::mem::replace(&mut self.phase, Phase::Idle) {
            Phase::Done(exit) => {
                self.phase = Phase::Done(exit);
                Step::Exit(exit)
            }
            Phase::Idle => self.take(),
            Phase::Claim(block) => self.claim(&block),
            Phase::Write(seq, intent) => self.write(seq, &intent),
            Phase::Publish(seq) => self.publish(seq),
            Phase::Seal(block) => self.seal(&block),
        }
    }

    /// W1, or a claim or seal request, under the mutex.
    fn take(&mut self) -> Step {
        let now = self.exchange.now();
        let mut held = self.exchange.acquire();
        if held.poisoned || held.state.fatal.is_some() {
            drop(held);
            return self.finish(WorkerExit::Latched);
        }
        match &held.state.claim {
            ClaimState::Requested(block) => {
                self.phase = Phase::Claim(block.clone());
                return Step::Progress;
            }
            ClaimState::Failed => {
                drop(held);
                return self.finish(WorkerExit::Latched);
            }
            ClaimState::None => {
                if held.state.stop {
                    drop(held);
                    return self.finish(WorkerExit::Stopped);
                }
                return Step::Idle;
            }
            ClaimState::Claimed => {}
        }
        let next = held.state.durable_through + 1;
        if let Some(intent) = held.state.slot(next).cloned() {
            held.state.pending = Some((next, now));
            self.phase = Phase::Write(next, Box::new(intent));
            return Step::Progress;
        }
        if let SealState::Requested(block) = &held.state.seal {
            self.phase = Phase::Seal(block.clone());
            return Step::Progress;
        }
        if held.state.stop {
            drop(held);
            return self.finish(WorkerExit::Stopped);
        }
        Step::Idle
    }

    /// The claim (design section 9.7): write the header, sync, recheck, then
    /// publish `Claimed` if nothing is latched.
    fn claim(&mut self, block: &[u8; BLOCK]) -> Step {
        if write_fully(&self.io, &self.file, 0, block).is_err() {
            return self.fail(Cause::ClaimWrite);
        }
        if let Err(error) = sync_with_retries(&self.io, &self.file, true) {
            return self.fail(Cause::ClaimSync(error.errno));
        }
        if !self.identity_ok() {
            return self.fail(Cause::ClaimIdentity);
        }
        let published = {
            let mut held = self.exchange.acquire();
            if !held.poisoned && held.state.fatal.is_none() {
                held.state.claim = ClaimState::Claimed;
                true
            } else {
                false
            }
        };
        self.exchange.wake_owner();
        if published {
            Step::Progress
        } else {
            self.finish(WorkerExit::Latched)
        }
    }

    /// W2 to W5, with no mutex held.
    fn write(&mut self, seq: u64, intent: &super::super::RecordIntent) -> Step {
        let frame = match codec::encode_record(intent) {
            Ok(frame) if (73..=123).contains(&frame.len()) => frame,
            _ => return self.fail(Cause::Encode(seq)),
        };
        let mut block = vec![0u8; BLOCK];
        block[..frame.len()].copy_from_slice(&frame);
        let Some(offset) = seq.checked_add(1).and_then(|n| n.checked_mul(BLOCK_U64)) else {
            return self.fail(Cause::Encode(seq));
        };
        match write_fully(&self.io, &self.file, offset, &block) {
            Ok(()) => {}
            Err(WriteFailure::Error(_) | WriteFailure::NoProgress) => {
                return self.fail(Cause::Write(seq));
            }
        }
        if let Err(error) = sync_with_retries(&self.io, &self.file, true) {
            return self.fail(Cause::Sync(seq, error.errno));
        }
        if !self.identity_ok() {
            return self.fail(Cause::Identity(seq));
        }
        self.phase = Phase::Publish(seq);
        Step::Progress
    }

    /// W6: publish only while nothing is latched and the mutex is not
    /// poisoned; otherwise the block may be durable but is never
    /// acknowledged.
    fn publish(&mut self, seq: u64) -> Step {
        let published = {
            let mut held = self.exchange.acquire();
            if !held.poisoned && held.state.fatal.is_none() && held.state.durable_through + 1 == seq
            {
                held.state.durable_through = seq;
                held.state.pending = None;
                true
            } else {
                false
            }
        };
        self.exchange.wake_owner();
        if published {
            Step::Progress
        } else {
            self.finish(WorkerExit::Latched)
        }
    }

    /// The seal: write, sync, then `Sealed` if nothing is latched.
    fn seal(&mut self, block: &[u8; BLOCK]) -> Step {
        if write_fully(&self.io, &self.file, BLOCK_U64, block).is_err() {
            return self.fail(Cause::SealWrite);
        }
        if let Err(error) = sync_with_retries(&self.io, &self.file, true) {
            return self.fail(Cause::SealSync(error.errno));
        }
        {
            let mut held = self.exchange.acquire();
            if !held.poisoned && held.state.fatal.is_none() {
                held.state.seal = SealState::Sealed;
            } else {
                held.state.seal = SealState::Failed;
            }
        }
        self.finish(WorkerExit::Sealed)
    }

    /// Run steps until the worker is idle or ends (single-threaded use).
    pub fn run_until_idle(&mut self) -> Step {
        loop {
            match self.step() {
                Step::Progress => continue,
                other => return other,
            }
        }
    }
}

/// A test's view of the worker's thread: called before each step, outside
/// the mutex. It may block (a deterministic handshake) or panic (worker
/// loss); it decides nothing.
pub trait WorkerHooks: Send + Sync {
    fn reached(&self, point: WorkerPoint);
}

/// Latches `WorkerLost` if the worker's thread unwinds.
struct LossGuard {
    exchange: Arc<Exchange>,
    armed: bool,
}

impl Drop for LossGuard {
    fn drop(&mut self) {
        if self.armed {
            self.exchange.latch(Cause::WorkerLost);
        }
    }
}

/// Run the worker on its own thread.
pub fn spawn_worker<P>(mut worker: Worker<P>, hooks: Option<Arc<dyn WorkerHooks>>) -> WorkerHandle
where
    P: StoreIo + 'static,
{
    let exchange = Arc::clone(&worker.exchange);
    let fallback = Arc::clone(&worker.exchange);
    let join = std::thread::Builder::new()
        .name("custody-recorder".into())
        .spawn(move || {
            let mut guard = LossGuard {
                exchange: Arc::clone(&exchange),
                armed: true,
            };
            loop {
                if let Some(hooks) = &hooks {
                    hooks.reached(worker.point());
                }
                match worker.step() {
                    Step::Progress => {}
                    Step::Exit(_) => break,
                    Step::Idle => {
                        let held = exchange.acquire();
                        let wait = held.state.fatal.is_none()
                            && !held.state.stop
                            && !matches!(held.state.claim, ClaimState::Requested(_))
                            && !matches!(held.state.seal, SealState::Requested(_))
                            && held.state.slot(held.state.durable_through + 1).is_none();
                        if wait {
                            drop(exchange.wait_for_work(held));
                        }
                    }
                }
            }
            guard.armed = false;
        });
    match join {
        Ok(join) => WorkerHandle::new(join),
        Err(_) => {
            // No thread: the worker never ran. That is recorder loss.
            fallback.latch(Cause::WorkerLost);
            WorkerHandle::none()
        }
    }
}

// ---------------------------------------------------------------------------
// The owner's start (design section 10.2 steps 9 and 10, section 10.3)
// ---------------------------------------------------------------------------

/// An owner started through the real core and the store: the custody
/// (NotStarted, every prior dispositioned), its control handle, the recorder
/// (its claim requested), the guard that holds the store's and the journal's
/// locks, the worker (to be run by a thread or stepped by a test), and the
/// header the claim writes.
pub struct OwnerStart<R: Resource, P: Platform> {
    pub custody: Custody<R>,
    pub control: Control,
    pub recorder: Recorder,
    pub guard: StoreGuard<P>,
    pub worker: Worker<P>,
    pub header: Header,
    pub claim: u64,
    pub index: u32,
    pub generation: Generation,
    pub report: StartupReport,
}

/// Why an owner did not start.
#[derive(Debug)]
pub enum StartRefused {
    /// The opening or the claim refused.
    Store(Refusal),
    /// The core refused the configuration or the priors.
    Core(super::super::Refusal),
    /// After every disposition was applied, these incidents still block: the
    /// NotStarted custody was closed.
    PriorUnresolved(Vec<[u8; 32]>),
}

/// Open the store, then (design section 10.2 steps 9 and 10): draw the
/// generation; `Custody::new` with every current incident as a prior; apply
/// the dispositions through the store's validator; refuse if anything still
/// blocks; then the claim handoff, with the header listing every current
/// incident and its disposition's reason. No record exists before the claim,
/// so a refusal leaves an untouched NotStarted custody that closes.
pub fn start_owner<R: Resource, P: Platform + Clone + 'static>(
    io: &P,
    path: &ProvisionPath,
    config: &Config,
    now: Tick,
    clock: Arc<dyn Fn() -> Tick + Send + Sync>,
    hooks: &mut dyn OpeningHooks,
) -> Result<OwnerStart<R, P>, StartRefused> {
    let opened = open_owner(io, path, config, hooks).map_err(StartRefused::Store)?;
    let generation = opened.draw_generation(io).map_err(StartRefused::Store)?;
    let priors = opened.priors();
    let current = opened.current();
    let (mut custody, control) =
        Custody::<R>::new(*config, generation, priors, now).map_err(StartRefused::Core)?;
    let validator = StoreValidator::new(&opened.selection.provision.root_id, &opened.scan);
    for (binding, _) in &current {
        let _ = custody.apply_disposition(&IncidentBinding::new(*binding), &validator, now);
    }
    let blocking: Vec<[u8; 32]> = current
        .iter()
        .filter(|(binding, _)| {
            custody.snapshot().prior.iter().any(|prior| {
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
    let report = opened.report();
    let applied: Vec<([u8; 32], DispositionReason)> =
        applied_reasons(&opened.decision, &opened.scan.dispositions);
    let claim = match opened.claim(io, config, generation, &applied) {
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
        _ => {
            let _ = custody.close(now);
            return Err(StartRefused::Store(Refusal {
                refused: Refused::ClaimFailed("header".into()),
                revision: Some(claim.selection.revision()),
            }));
        }
    };
    let exchange = Exchange::new(generation, config.record_capacity, clock);
    let recorder = Recorder::new(Arc::clone(&exchange));
    let worker = Worker::new(
        io.clone(),
        claim.io_file,
        Arc::clone(claim.guard.journals()),
        claim.journal_name,
        claim.journal_stat,
        exchange,
    );
    recorder.request_claim(claim.header_block);
    Ok(OwnerStart {
        custody,
        control,
        recorder,
        guard: claim.guard,
        worker,
        header,
        claim: claim.claim,
        index: claim.index,
        generation,
        report,
    })
}
