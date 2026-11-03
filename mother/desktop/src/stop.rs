//! The Stop route (M2b design §6.4 and §11 A1, A3).
//!
//! **The ingress** runs on the GTK thread at each press and is memory only:
//! it advances the Stop epoch (`Desk::cancel_all`, which in M2b-A cancels
//! nothing else: there are no confirmations yet) and reserves a worker slot,
//! under the desk's one small mutex. No I/O, no broker call, no other lock.
//!
//! **The blocking part**, the broker's `emergency_stop()` (kernel and broker
//! memory first, then its evidence records, which may stall on the store),
//! runs on a new worker thread for that press: never a shared queue, so one
//! stalled Stop never holds back the next press's ingress. At most
//! [`MAX_STOP_WORKERS`] workers live at once; a press at the cap still does
//! its ingress and is counted as coalesced.
//!
//! **Quit** joins every worker within a bound. A timed-out join is reported
//! as such, never as a clean shutdown, and Quit then exits non-zero.

use nexus_governed_control::governed::GovernedControl;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Live Stop workers allowed at once (A1).
pub const MAX_STOP_WORKERS: usize = 4;

/// One worker the route started.
struct Worker {
    id: u64,
    handle: Option<JoinHandle<()>>,
    done: bool,
}

#[derive(Default)]
struct DeskState {
    epoch: u64,
    receipts: Receipts,
    next_worker: u64,
    workers: Vec<Worker>,
}

/// The confirmation desk's Stop half. M2b-A has no confirmations, so the
/// desk holds only the epoch and the Stop route's receipts; M2b-B's desk
/// plugs in here. Its mutex is never held across I/O, a broker call, a GTK
/// call or a thread spawn.
pub struct Desk {
    state: Mutex<DeskState>,
    changed: Condvar,
}

impl Desk {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(DeskState::default()),
            changed: Condvar::new(),
        })
    }

    /// The state, even after a panic elsewhere poisoned the mutex: its
    /// fields are plain counters, valid at every point.
    fn lock(&self) -> MutexGuard<'_, DeskState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The current Stop epoch.
    pub fn epoch(&self) -> u64 {
        self.lock().epoch
    }

    /// Stop's step 0: memory only. Returns the new epoch.
    pub fn cancel_all(&self) -> u64 {
        let mut state = self.lock();
        Self::cancel_all_in(&mut state)
    }

    /// Step 0 under an already held desk mutex. M2b-A: no request exists
    /// to cancel; the epoch moves.
    fn cancel_all_in(state: &mut DeskState) -> u64 {
        state.epoch += 1;
        state.receipts.epoch = state.epoch;
        state.epoch
    }
}

/// What one press did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// The ingress happened, and a new worker runs `emergency_stop()`.
    Dispatched { epoch: u64, worker: u64 },
    /// The ingress happened; the cap was reached, so no new worker.
    Coalesced { epoch: u64 },
    /// The ingress happened; the OS refused a new thread.
    SpawnFailed { epoch: u64 },
}

impl Press {
    pub fn epoch(&self) -> u64 {
        match *self {
            Press::Dispatched { epoch, .. }
            | Press::Coalesced { epoch }
            | Press::SpawnFailed { epoch } => epoch,
        }
    }
}

/// The route's receipts, all in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Receipts {
    pub epoch: u64,
    pub presses: u64,
    pub dispatched: u64,
    pub coalesced: u64,
    pub spawn_failures: u64,
    /// Workers started and not yet finished.
    pub live: usize,
    pub finished: u64,
    /// Workers whose `emergency_stop()` panicked.
    pub panicked: u64,
}

/// How a bounded join ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joined {
    All { joined: usize },
    TimedOut { still_running: usize },
}

/// Test seams (compiled only into the crate's own unit tests): hold a
/// worker after it published `done`, or hold a press between the spawn and
/// registering the worker's handle.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Seams {
    pub after_done: Option<Box<dyn Fn() + Send + Sync>>,
    pub before_register: Option<Box<dyn Fn() + Send + Sync>>,
}

/// The Stop route: every Stop button and shortcut goes through it.
pub struct StopRoute {
    desk: Arc<Desk>,
    /// The blocking part of one Stop: the broker's `emergency_stop()`.
    body: Arc<dyn Fn() + Send + Sync>,
    #[cfg(test)]
    seams: Arc<Seams>,
}

impl StopRoute {
    pub fn new(desk: Arc<Desk>, owner: Arc<GovernedControl>) -> Arc<Self> {
        Arc::new(Self {
            desk,
            body: Arc::new(move || {
                owner.emergency_stop();
            }),
            #[cfg(test)]
            seams: Arc::new(Seams::default()),
        })
    }

    /// A route whose Stop body is `body`, with `seams` (unit tests only).
    #[cfg(test)]
    pub(crate) fn with_body(
        desk: Arc<Desk>,
        body: impl Fn() + Send + Sync + 'static,
        seams: Seams,
    ) -> Arc<Self> {
        Arc::new(Self {
            desk,
            body: Arc::new(body),
            seams: Arc::new(seams),
        })
    }

    pub fn desk(&self) -> &Arc<Desk> {
        &self.desk
    }

    /// One press: the ingress, then a worker (or the coalescing receipt).
    pub fn press(&self) -> Press {
        // The ingress: memory only, under the desk mutex alone.
        let (epoch, worker) = {
            let mut state = self.desk.lock();
            let epoch = Desk::cancel_all_in(&mut state);
            state.receipts.presses += 1;
            if state.receipts.live >= MAX_STOP_WORKERS {
                state.receipts.coalesced += 1;
                return Press::Coalesced { epoch };
            }
            state.receipts.live += 1;
            state.receipts.dispatched += 1;
            state.next_worker += 1;
            let id = state.next_worker;
            state.workers.push(Worker {
                id,
                handle: None,
                done: false,
            });
            (epoch, id)
        };
        // The blocking part, on its own new thread, after the unlock.
        let desk = self.desk.clone();
        let body = self.body.clone();
        #[cfg(test)]
        let seams = self.seams.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("nexus-stop-{worker}"))
            .spawn(move || {
                let panicked = catch_unwind(AssertUnwindSafe(|| body())).is_err();
                let mut state = desk.lock();
                if let Some(entry) = state.workers.iter_mut().find(|w| w.id == worker) {
                    entry.done = true;
                }
                state.receipts.live -= 1;
                state.receipts.finished += 1;
                if panicked {
                    state.receipts.panicked += 1;
                }
                desk.changed.notify_all();
                drop(state);
                #[cfg(test)]
                if let Some(seam) = &seams.after_done {
                    seam();
                }
            });
        #[cfg(test)]
        if let Some(seam) = &self.seams.before_register {
            seam();
        }
        let mut state = self.desk.lock();
        match spawned {
            Ok(handle) => {
                if let Some(entry) = state.workers.iter_mut().find(|w| w.id == worker) {
                    entry.handle = Some(handle);
                }
                Press::Dispatched { epoch, worker }
            }
            Err(_) => {
                state.workers.retain(|w| w.id != worker);
                state.receipts.live -= 1;
                state.receipts.dispatched -= 1;
                state.receipts.spawn_failures += 1;
                Press::SpawnFailed { epoch }
            }
        }
    }

    pub fn receipts(&self) -> Receipts {
        self.desk.lock().receipts.clone()
    }

    /// Join every worker started so far, within `within`. A worker still
    /// running at the bound is reported, never joined by waiting longer.
    pub fn join_within(&self, within: Duration) -> Joined {
        let deadline = Instant::now() + within;
        let mut state = self.desk.lock();
        loop {
            let running = state.workers.iter().filter(|w| !w.done).count();
            if running == 0 {
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Joined::TimedOut {
                    still_running: running,
                };
            }
            state = self
                .desk
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        let handles: Vec<JoinHandle<()>> = state
            .workers
            .drain(..)
            .filter_map(|mut w| w.handle.take())
            .collect();
        drop(state);
        // Every one of these has marked itself done: it only returns now.
        let joined = handles.len();
        for handle in handles {
            let _ = handle.join();
        }
        Joined::All { joined }
    }
}

/// How Quit ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuitOutcome {
    /// Every Stop worker joined, and the broker shut down.
    Clean {
        joined: usize,
        runs_cancelled: usize,
    },
    /// Something was still running when the bound passed.
    Unclean {
        stop_workers_running: usize,
        shut_down: bool,
    },
}

impl QuitOutcome {
    pub fn is_clean(&self) -> bool {
        matches!(self, QuitOutcome::Clean { .. })
    }

    /// 0 only for a clean Quit.
    pub fn exit_code(&self) -> i32 {
        if self.is_clean() {
            0
        } else {
            3
        }
    }

    /// What the app says on Quit.
    pub fn report(&self) -> String {
        match self {
            QuitOutcome::Clean {
                joined,
                runs_cancelled,
            } => format!(
                "Mother quit: {joined} Stop worker(s) joined; the broker shut down \
                 ({runs_cancelled} open run(s) cancelled)."
            ),
            QuitOutcome::Unclean {
                stop_workers_running: 0,
                shut_down: false,
            } => "Mother quit before the broker's shutdown finished: it is still running \
                  (its evidence store did not answer in time)."
                .to_string(),
            QuitOutcome::Unclean {
                stop_workers_running,
                shut_down,
            } => format!(
                "Mother quit with {stop_workers_running} Stop worker(s) still running \
                 (their evidence writes did not finish in time); the broker was {}shut down. \
                 The Stop itself is in force: its memory step happened before its records.",
                if *shut_down { "" } else { "not " }
            ),
        }
    }
}

/// Quit (design §6.6, A3): join the Stop workers, then shut the broker
/// down, everything within `within`. A wait that times out is reported as
/// such; nothing after it is attempted that could block on the store.
pub fn quit(route: &StopRoute, owner: &Arc<GovernedControl>, within: Duration) -> QuitOutcome {
    let deadline = Instant::now() + within;
    let joined = match route.join_within(within) {
        Joined::All { joined } => joined,
        Joined::TimedOut { still_running } => {
            return QuitOutcome::Unclean {
                stop_workers_running: still_running,
                shut_down: false,
            }
        }
    };
    // The shutdown writes records too: on its own thread, within the bound.
    let (tx, rx) = mpsc::channel();
    let broker = owner.clone();
    let spawned = std::thread::Builder::new()
        .name("nexus-quit".into())
        .spawn(move || {
            let _ = tx.send(broker.shut_down());
        });
    if spawned.is_err() {
        return QuitOutcome::Unclean {
            stop_workers_running: 0,
            shut_down: false,
        };
    }
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(runs_cancelled) => QuitOutcome::Clean {
            joined,
            runs_cancelled,
        },
        Err(_) => QuitOutcome::Unclean {
            stop_workers_running: 0,
            shut_down: false,
        },
    }
}

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
