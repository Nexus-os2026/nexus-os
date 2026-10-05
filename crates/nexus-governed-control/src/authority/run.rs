//! Governed runs and cancellation.
//!
//! A run is the scope of one command (or agent goal): every commitment
//! belongs to exactly one run and one agent. Cancelling a run sets its token,
//! which every actuator observes, and fires the hooks of the resources it
//! owns (a contained process, a browser session, a credential lease), so
//! nothing it started keeps acting. An emergency stop cancels every run.

use super::ids::{AgentId, RunId};
use super::AuthorityError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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

type Hook = Box<dyn FnOnce() + Send>;

struct RunEntry {
    view: RunView,
    cancelled: Arc<AtomicBool>,
    hooks: Vec<(u64, Hook)>,
}

/// Every run, and the hooks of what each owns.
#[derive(Default)]
pub struct RunRegistry {
    runs: Mutex<HashMap<RunId, RunEntry>>,
    next_hook: AtomicU64,
    stopped: AtomicBool,
    cancelled_by_stop: AtomicUsize,
}

/// Deregisters a cancellation hook when the resource it would release has
/// been released normally.
pub struct HookRegistration {
    run: RunId,
    hook: u64,
    registry: Arc<RunRegistry>,
}

impl Drop for HookRegistration {
    fn drop(&mut self) {
        if let Some(entry) = self.registry.runs.lock().expect("runs").get_mut(&self.run) {
            entry.hooks.retain(|(id, _)| *id != self.hook);
        }
    }
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
            hooks: Vec::new(),
        };
        let mut runs = self.runs.lock().expect("runs");
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
    pub fn check(&self, run: RunId, agent: &AgentId) -> Result<CancelToken, AuthorityError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        let runs = self.runs.lock().expect("runs");
        let entry = runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        if &entry.view.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.view.cancelled || entry.cancelled.load(Ordering::SeqCst) {
            return Err(AuthorityError::RunCancelled);
        }
        if entry.view.finished {
            return Err(AuthorityError::RunNotActive);
        }
        Ok(CancelToken(entry.cancelled.clone()))
    }

    /// Register `hook` to run if `run` is cancelled. Dropping the returned
    /// registration (after a normal release) removes it.
    pub fn on_cancel(
        self: &Arc<Self>,
        run: RunId,
        hook: Box<dyn FnOnce() + Send>,
    ) -> Result<HookRegistration, AuthorityError> {
        let id = self.next_hook.fetch_add(1, Ordering::SeqCst);
        let mut runs = self.runs.lock().expect("runs");
        let entry = runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        if entry.view.cancelled {
            // Already cancelled: release now.
            drop(runs);
            hook();
            return Err(AuthorityError::RunCancelled);
        }
        entry.hooks.push((id, hook));
        Ok(HookRegistration {
            run,
            hook: id,
            registry: self.clone(),
        })
    }

    /// Cancel one run: set its token and fire its hooks (outside the lock).
    /// Returns whether the run was active.
    pub(crate) fn cancel(&self, run: RunId) -> bool {
        let hooks = {
            let mut runs = self.runs.lock().expect("runs");
            let Some(entry) = runs.get_mut(&run) else {
                return false;
            };
            let was_active = !entry.view.cancelled && !entry.view.finished;
            entry.view.cancelled = true;
            entry.cancelled.store(true, Ordering::SeqCst);
            if !was_active {
                return false;
            }
            std::mem::take(&mut entry.hooks)
        };
        for (_, hook) in hooks {
            hook();
        }
        true
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

    /// A run is done: it accepts nothing more, and whatever it still owns is
    /// released (the hooks fire outside the lock).
    pub(crate) fn finish(&self, run: RunId) {
        let hooks = match self.runs.lock().expect("runs").get_mut(&run) {
            Some(entry) if !entry.view.finished && !entry.view.cancelled => {
                entry.view.finished = true;
                std::mem::take(&mut entry.hooks)
            }
            _ => return,
        };
        for (_, hook) in hooks {
            hook();
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
