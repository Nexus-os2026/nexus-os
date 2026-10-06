//! Governed runs and cancellation.
//!
//! A run is the scope of one command (or agent goal): every commitment
//! belongs to exactly one run and one agent. Cancelling or finishing a run
//! sets its token, which every executing effect observes between its bounded
//! steps (a contained process, a browser session and an input action end
//! their own resources when they see it), and ends its unconsumed
//! commitments with their credential leases. An emergency stop cancels every
//! run.

use super::ids::{AgentId, RunId};
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A run's cancellation flag, observed by actuators between bounded steps.
#[derive(Clone, Debug)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Where a run came from (display only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOrigin {
    /// The unified command front door, with the modalities it carried.
    Command { modalities: Vec<String> },
    /// An agent goal admitted through the front door.
    AgentGoal,
}

/// A run, for display.
#[derive(Clone, Debug)]
pub struct RunView {
    pub id: RunId,
    pub agent: AgentId,
    pub origin: RunOrigin,
    pub opened_wall_ms: u64,
    pub cancelled: bool,
    pub finished: bool,
}

struct RunEntry {
    view: RunView,
    cancelled: Arc<AtomicBool>,
}

/// Every run.
#[derive(Default)]
pub struct RunRegistry {
    runs: Mutex<HashMap<RunId, RunEntry>>,
    stopped: AtomicBool,
    cancelled_by_stop: AtomicUsize,
    /// The desktop is quitting: no run opens any more, and nothing resumes.
    closed: AtomicBool,
}

/// Most runs kept (ended ones are pruned first).
const RUN_CAPACITY: usize = 4096;

impl RunRegistry {
    pub(crate) fn open(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        now_wall_ms: u64,
    ) -> Result<RunId, AuthorityError> {
        let id = RunId::fresh();
        let entry = RunEntry {
            view: RunView {
                id,
                agent,
                origin,
                opened_wall_ms: now_wall_ms,
                cancelled: false,
                finished: false,
            },
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let mut runs = self.runs.lock().expect("runs");
        // Checked under the lock an emergency stop takes to cancel every
        // run: a run cannot slip in between the stop and its sweep.
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        if runs.len() >= RUN_CAPACITY {
            runs.retain(|_, e| !e.view.cancelled && !e.view.finished);
            if runs.len() >= RUN_CAPACITY {
                return Err(AuthorityError::Capacity);
            }
        }
        runs.insert(id, entry);
        Ok(id)
    }

    /// The run exists, belongs to `agent`, is not cancelled or finished, and
    /// no emergency stop is in force.
    pub(crate) fn check(&self, run: RunId, agent: &AgentId) -> Result<CancelToken, AuthorityError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        let runs = self.runs.lock().expect("runs");
        let entry = runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        if &entry.view.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.view.finished {
            return Err(AuthorityError::RunNotActive);
        }
        if entry.view.cancelled || entry.cancelled.load(Ordering::SeqCst) {
            return Err(AuthorityError::RunCancelled);
        }
        Ok(CancelToken(entry.cancelled.clone()))
    }

    /// Cancel one run: set its token. Returns whether the run was active.
    pub(crate) fn cancel(&self, run: RunId) -> bool {
        let mut runs = self.runs.lock().expect("runs");
        let Some(entry) = runs.get_mut(&run) else {
            return false;
        };
        let was_active = !entry.view.cancelled && !entry.view.finished;
        entry.view.cancelled = true;
        entry.cancelled.store(true, Ordering::SeqCst);
        was_active
    }

    /// Refuse every new run from now on (the desktop is quitting). Taken
    /// under the lock `open` checks under, so no run slips in after it.
    pub(crate) fn close(&self) {
        let _runs = self.runs.lock().expect("runs");
        self.closed.store(true, Ordering::SeqCst);
    }

    /// Cancel every run and refuse new work until `resume`.
    pub(crate) fn stop_all(&self) -> usize {
        self.stopped.store(true, Ordering::SeqCst);
        let ids: Vec<RunId> = self.runs.lock().expect("runs").keys().copied().collect();
        let cancelled = ids.into_iter().filter(|id| self.cancel(*id)).count();
        self.cancelled_by_stop
            .fetch_add(cancelled, Ordering::SeqCst);
        cancelled
    }

    /// Allow new runs again after an emergency stop (owner action). Returns
    /// how many runs the stop cancelled.
    pub(crate) fn resume(&self) -> usize {
        self.stopped.store(false, Ordering::SeqCst);
        self.cancelled_by_stop.swap(0, Ordering::SeqCst)
    }

    pub(crate) fn cancelled_by_stop(&self) -> usize {
        self.cancelled_by_stop.load(Ordering::SeqCst)
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// A run is done: it accepts nothing more, and whatever of it still
    /// runs sees the end through its token.
    pub(crate) fn finish(&self, run: RunId) {
        if let Some(entry) = self.runs.lock().expect("runs").get_mut(&run) {
            if !entry.view.finished && !entry.view.cancelled {
                entry.view.finished = true;
                entry.cancelled.store(true, Ordering::SeqCst);
            }
        }
    }

    pub fn view(&self, run: RunId) -> Option<RunView> {
        self.runs
            .lock()
            .expect("runs")
            .get(&run)
            .map(|e| e.view.clone())
    }

    pub fn views(&self) -> Vec<RunView> {
        let mut views: Vec<RunView> = self
            .runs
            .lock()
            .expect("runs")
            .values()
            .map(|e| e.view.clone())
            .collect();
        views.sort_by_key(|v| v.opened_wall_ms);
        views
    }
}
