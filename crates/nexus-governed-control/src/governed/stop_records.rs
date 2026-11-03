//! The Owner's Stop records, written off the Stop path (Codex review 1 of
//! PR #64, H1; Owner D2, Codex condition 5): a Stop takes effect in memory
//! and returns; its records go to one owned background writer through a
//! bounded queue. The writer holds no lock the Stop path needs while it
//! writes: the one lock here is held only to queue a batch or count an
//! outcome, never across a write. Nothing is swallowed: a Stop whose
//! records could not be queued, or that the store could not take, is
//! counted and surfaced ([`StopRecordStatus`]); records the store keeps
//! owed are written by it, first and in order, once storage recovers.

use crate::authority::evidence::{EndStatus, EvidencePhase};
use crate::authority::Stopped;
use crate::control::Control;
use serde::Serialize;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Most Stops whose records may wait to be written at once. A Stop beyond
/// it is not queued; that is counted as failed, never silent.
const QUEUE: usize = 256;
/// The most a Stop waits for its own records before it returns, so that a
/// healthy store has them written when it returns: its acknowledgement
/// never depends on storage for longer (well inside the 1 s bound).
pub(super) const ACKNOWLEDGE_WITHIN: Duration = Duration::from_millis(500);
/// The most dropping the front door waits for the writer to finish.
const SHUTDOWN_WITHIN: Duration = Duration::from_secs(5);

/// One Stop's records: the commitments it ended and the stop itself, and
/// the agent display it stopped, if one ran.
pub(super) struct StopBatch {
    pub(super) stopped: Stopped,
    pub(super) display: Option<u32>,
}

/// What became of the Owner's Stop records (the host shows it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct StopRecordStatus {
    /// Stops whose records are queued and not yet written.
    pub pending: u64,
    /// Stops whose records the store keeps owed (not committed): it writes
    /// them, in order, before anything later, once storage recovers.
    pub owed: u64,
    /// Stops whose records could not be queued, or that the store could not
    /// take at all (latched or failing): not written.
    pub failed: u64,
    /// Stops whose records were written.
    pub written: u64,
}

impl StopRecordStatus {
    /// `stop_record_failed`, `stop_record_pending` (queued or owed) or
    /// `stop_records_written`.
    pub fn state(&self) -> &'static str {
        if self.failed > 0 {
            "stop_record_failed"
        } else if self.pending + self.owed > 0 {
            "stop_record_pending"
        } else {
            "stop_records_written"
        }
    }
}

#[derive(Default)]
struct State {
    sender: Option<SyncSender<StopBatch>>,
    /// Batches queued, and batches written (in queue order).
    queued: u64,
    done: u64,
    status: StopRecordStatus,
    finished: bool,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    shared.0.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The writer of the Owner's Stop records: one thread, a bounded queue,
/// joined (bounded) when the front door is dropped.
pub(super) struct StopRecorder {
    shared: Shared,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl StopRecorder {
    /// Start the writer over `control`'s evidence. If the thread cannot be
    /// started, every Stop's records are counted as failed.
    pub(super) fn start(control: Arc<Control>) -> Self {
        let shared: Shared = Arc::default();
        let (sender, batches) = sync_channel::<StopBatch>(QUEUE);
        let theirs = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("nexus-stop-records".into())
            .spawn(move || {
                for batch in batches {
                    let end = write(&control, batch);
                    let mut state = lock(&theirs);
                    state.done += 1;
                    state.status.pending = state.status.pending.saturating_sub(1);
                    match end {
                        EndStatus::Recorded { .. } => state.status.written += 1,
                        EndStatus::Owed => state.status.owed += 1,
                        EndStatus::Uncertain => state.status.failed += 1,
                    }
                    drop(state);
                    theirs.1.notify_all();
                }
                lock(&theirs).finished = true;
                theirs.1.notify_all();
            })
            .ok();
        if thread.is_some() {
            lock(&shared).sender = Some(sender);
        }
        Self {
            shared,
            thread: Mutex::new(thread),
        }
    }

    /// Queue one Stop's records, never blocking; returns its place in the
    /// queue, or `None` when it could not be queued (counted as failed).
    pub(super) fn submit(&self, batch: StopBatch) -> Option<u64> {
        let mut state = lock(&self.shared);
        let sent = match &state.sender {
            Some(sender) => sender.try_send(batch).is_ok(),
            None => false,
        };
        if sent {
            state.queued += 1;
            state.status.pending += 1;
            Some(state.queued)
        } else {
            state.status.failed += 1;
            None
        }
    }

    /// Wait at most `bound` for the batch at `place` to be written.
    pub(super) fn acknowledge(&self, place: Option<u64>, bound: Duration) {
        let Some(place) = place else {
            return;
        };
        let state = lock(&self.shared);
        let _ = self
            .shared
            .1
            .wait_timeout_while(state, bound, |state| state.done < place);
    }

    pub(super) fn status(&self) -> StopRecordStatus {
        lock(&self.shared).status
    }

    /// Wait at most `bound` until nothing is queued (tests).
    #[cfg(test)]
    pub(super) fn idle(&self, bound: Duration) -> bool {
        let state = lock(&self.shared);
        let (state, _) = self
            .shared
            .1
            .wait_timeout_while(state, bound, |state| state.done < state.queued)
            .unwrap_or_else(PoisonError::into_inner);
        state.done >= state.queued
    }

    /// Close the queue and wait, at most [`SHUTDOWN_WITHIN`], for the
    /// writer to write what it holds and end; then join it. A writer still
    /// blocked by storage after that is left to finish on its own.
    fn shutdown(&self) {
        let deadline = Instant::now() + SHUTDOWN_WITHIN;
        let mut state = lock(&self.shared);
        state.sender = None;
        while !state.finished {
            let now = Instant::now();
            if now >= deadline {
                return;
            }
            state = self
                .shared
                .1
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        drop(state);
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Drop for StopRecorder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Write one Stop's records, as the Stop path did before: the commitments
/// it ended, the stop, then the display stop. Returns the worst outcome.
fn write(control: &Control, batch: StopBatch) -> EndStatus {
    let authority = control.authority();
    let mut end = authority.write_stop_records(batch.stopped);
    if let Some(number) = batch.display {
        let display = authority.record_display_end(
            EvidencePhase::DisplayStopped,
            vec![("display".into(), number.to_string())],
        );
        end = worse(end, display);
    }
    end
}

fn worse(a: EndStatus, b: EndStatus) -> EndStatus {
    match (a, b) {
        (EndStatus::Uncertain, _) | (_, EndStatus::Uncertain) => EndStatus::Uncertain,
        (EndStatus::Owed, _) | (_, EndStatus::Owed) => EndStatus::Owed,
        (recorded, _) => recorded,
    }
}
