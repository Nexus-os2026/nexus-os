//! The recorder worker (design sections 9.4 and 9.7). The owner's start
//! through the real core and the owner that retains the custody are in
//! [`super::owner`].
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
//!
//! The worker, its constructor and its runner are internal to the store
//! (`pub(super)`): only [`super::owner::start_owner`] constructs one for a
//! store, over the journal its own claim opened and the exchange of the same
//! owner. No interface points an owner's worker at another file or exchange.
//! The one other construction is [`native_wiring`] (Linux only): a check of
//! the write and sync path on a native file of a test's own, on an exchange
//! of its own that never leaves it. It is not a store and has no seal.

use std::sync::Arc;

use super::super::codec;
use super::exchange::{Cause, ClaimState, Exchange, SealState, WorkerExit, WorkerHandle};
use super::format::{BLOCK, BLOCK_U64};
use super::io::{sync_with_retries, write_fully, Stat, StoreIo, WriteFailure};

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

/// The recorder worker of one claimed journal. Internal to the store.
pub(super) struct Worker<P: StoreIo> {
    io: P,
    file: P::File,
    journals: Arc<P::Dir>,
    name: String,
    identity: Stat,
    exchange: Arc<Exchange>,
    phase: Phase,
}

impl<P: StoreIo> Worker<P> {
    pub(super) fn new(
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
    pub(super) fn point(&self) -> WorkerPoint {
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
    pub(super) fn step(&mut self) -> Step {
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
    pub(super) fn run_until_idle(&mut self) -> Step {
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
/// loss); it decides nothing and can change no state. Installed only through
/// [`super::faults::spawn_worker_with_hooks`].
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

/// Run the worker on its own thread. `keep` (the owner's exclusion guard) is
/// held by the thread until the worker ends, so the store's exclusion lasts
/// as long as any of its I/O may be in flight, whatever happens to the owner.
pub(super) fn spawn_worker<P, K>(
    mut worker: Worker<P>,
    hooks: Option<Arc<dyn WorkerHooks>>,
    keep: K,
) -> WorkerHandle
where
    P: StoreIo + 'static,
    K: Send + 'static,
{
    let exchange = Arc::clone(&worker.exchange);
    let fallback = Arc::clone(&worker.exchange);
    let join = std::thread::Builder::new()
        .name("custody-recorder".into())
        .spawn(move || {
            let _keep = keep;
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

/// A native file of a test's own, beneath its `CARGO_TARGET_TMPDIR`, for
/// [`native_wiring`]: the I/O, the file's read-write description, its
/// directory, its name there and its identity.
#[cfg(target_os = "linux")]
pub struct NativeJournal {
    pub io: super::io::Faulty<super::io::linux::LinuxIo>,
    pub file: super::io::linux::LinuxFile,
    pub dir: Arc<super::io::linux::LinuxDir>,
    pub name: String,
    pub identity: Stat,
}

/// The worker's write, sync and publication wiring on a native file (design
/// section 16.5), through the Linux primitives.
///
/// It is not a store and confers nothing on one: there is no `PROVISION`,
/// activation, lock, opening, claim decision or owner, and the file is no
/// pool file. The exchange, recorder and worker it builds are its own and
/// never leave it: `drive` reaches them only through the borrowed
/// [`NativeWiring`]. That offers the claim request, a flush and an apply
/// step of the caller's own custody, the worker's run until idle, and the
/// claim state. It offers no seal, latch, exchange fault, state write or
/// thread (planned I/O faults of `Faulty` act on the caller's own file).
#[cfg(target_os = "linux")]
pub fn native_wiring<T>(
    journal: NativeJournal,
    generation: super::super::Generation,
    capacity: u32,
    drive: impl FnOnce(&mut NativeWiring) -> T,
) -> T {
    let exchange = Exchange::new(generation, capacity, Arc::new(|| super::super::Tick(0)));
    let recorder = super::exchange::Recorder::new(Arc::clone(&exchange));
    let worker = Worker::new(
        journal.io,
        journal.file,
        journal.dir,
        journal.name,
        journal.identity,
        exchange,
    );
    drive(&mut NativeWiring { recorder, worker })
}

/// [`native_wiring`]'s exchange, recorder and worker, borrowed by its
/// `drive` only.
#[cfg(target_os = "linux")]
pub struct NativeWiring {
    recorder: super::exchange::Recorder,
    worker: Worker<super::io::Faulty<super::io::linux::LinuxIo>>,
}

#[cfg(target_os = "linux")]
impl NativeWiring {
    /// Request the claim: the worker writes and syncs `header` first.
    pub fn request_claim(&mut self, header: Box<[u8; BLOCK]>) {
        self.recorder.request_claim(header);
    }

    pub fn claim_state(&self) -> ClaimState {
        self.recorder.claim_state()
    }

    /// The caller's custody's records through the recorder's sink, then the
    /// apply step.
    pub fn flush<R: super::super::Resource>(
        &mut self,
        custody: &mut super::super::Custody<R>,
        now: super::super::Tick,
    ) -> Result<super::exchange::Applied, super::super::FlushError> {
        self.recorder.flush(custody, now)
    }

    /// The apply step: acknowledgements of durable records to the caller's
    /// custody.
    pub fn apply<R: super::super::Resource>(
        &mut self,
        custody: &mut super::super::Custody<R>,
        now: super::super::Tick,
    ) -> super::exchange::Applied {
        self.recorder.apply(custody, now)
    }

    /// Run the worker until it has nothing to do.
    pub fn run_worker_until_idle(&mut self) -> Step {
        self.worker.run_until_idle()
    }
}
