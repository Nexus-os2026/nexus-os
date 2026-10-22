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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

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

/// Whose run it is, as the backend opened it, retained with the run: only
/// the owner's own command front door opens an owner run
/// ([`super::Authority::open_owner_run`]); every other run is an agent's.
/// Never derived from the agent identity, the origin or any other text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunClass {
    Owner,
    Agent,
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
    class: RunClass,
    cancelled: Arc<AtomicBool>,
}

/// Every run.
///
/// `stopped`, `stop_number`, `exhausted`, `last_lifted`, `closed` and
/// `cancelled_by_stop` are written only while `runs` is held, and every
/// decision that must be atomic with another is made under it. While it is
/// held the registry takes no other lock, writes no record, raises no
/// dialog and calls nothing outside itself, and its arithmetic is checked
/// or saturating. A read of a flag without the lock is advisory: an early
/// refusal or a status.
#[derive(Default)]
pub struct RunRegistry {
    runs: Mutex<HashMap<RunId, RunEntry>>,
    stopped: AtomicBool,
    /// Runs cancelled by emergency stops since the last lift (it can span
    /// more than one stop).
    cancelled_by_stop: AtomicUsize,
    /// The desktop is quitting: no run opens any more, and nothing resumes.
    closed: AtomicBool,
    /// The emergency stops of this process, from 0. It never wraps.
    stop_number: AtomicU64,
    /// A stop came with `stop_number` at its maximum: stopped for the rest
    /// of the process, and no resume lifts it. Never cleared.
    exhausted: AtomicBool,
    /// The number of the last stop a resume lifted (0: none).
    last_lifted: AtomicU64,
}

/// Most runs kept (ended ones are pruned first).
const RUN_CAPACITY: usize = 4096;

/// What became of the stop before an emergency stop, as that stop found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviousStop {
    /// This is the process's first stop.
    First,
    Lifted,
    /// Not lifted, or the stop number is exhausted.
    NotLifted,
}

impl PreviousStop {
    /// As the stop's record says it: `none`, `yes` or `no`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PreviousStop::First => "none",
            PreviousStop::Lifted => "yes",
            PreviousStop::NotLifted => "no",
        }
    }
}

/// What an emergency stop did, in its one hold of the runs lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stop {
    /// The runs it cancelled (those that were active).
    pub(crate) cancelled: usize,
    /// Its stop number (once exhausted, the same as the stop's before it).
    pub(crate) number: u64,
    pub(crate) exhausted: bool,
    pub(crate) previous: PreviousStop,
}

/// The stop a resume asks the owner to lift, read in one hold of the runs
/// lock: its number, and the runs cancelled since the last lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StopToLift {
    pub(crate) number: u64,
    pub(crate) cancelled: usize,
}

/// A resume the registry lets through: the stop the owner confirmed is
/// lifted now, or was already lifted by another resume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lift {
    Lifted,
    AlreadyLifted,
}

/// Why the registry refuses a resume. Only a lift changes anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// A newer emergency stop came after the stop the owner confirmed.
    Superseded,
    /// The stop number is exhausted: the registry stays stopped.
    Exhausted,
    /// The desktop is quitting.
    TooLate,
    /// The runs lock is poisoned.
    Unavailable,
}

impl RunRegistry {
    pub(crate) fn open(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
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
            class,
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

    /// Whether the desktop is quitting (see `close`).
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// The emergency stop: cancel every run and refuse new work until a
    /// resume lifts this stop, in one hold of the runs lock. In order: what
    /// became of the stop before; the next stop number, or `exhausted` at
    /// the maximum; the flag; every run cancelled as `cancel` cancels one;
    /// the count. The flag is set only once the lock is held. A poisoned
    /// lock does not prevent the stop: it goes on with what the lock holds,
    /// treats nothing as repaired, and leaves the poison.
    pub(crate) fn stop_all(&self) -> Stop {
        let mut runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        let current = self.stop_number.load(Ordering::SeqCst);
        let previous = if current == 0 {
            PreviousStop::First
        } else if self.exhausted.load(Ordering::SeqCst)
            || self.last_lifted.load(Ordering::SeqCst) != current
        {
            PreviousStop::NotLifted
        } else {
            PreviousStop::Lifted
        };
        let (number, exhausted) = match current.checked_add(1) {
            Some(next) => {
                self.stop_number.store(next, Ordering::SeqCst);
                (next, false)
            }
            None => {
                self.exhausted.store(true, Ordering::SeqCst);
                (current, true)
            }
        };
        self.stopped.store(true, Ordering::SeqCst);
        let mut cancelled = 0usize;
        for entry in runs.values_mut() {
            let was_active = !entry.view.cancelled && !entry.view.finished;
            entry.view.cancelled = true;
            entry.cancelled.store(true, Ordering::SeqCst);
            if was_active {
                cancelled = cancelled.saturating_add(1);
            }
        }
        let since_the_last_lift = self
            .cancelled_by_stop
            .load(Ordering::SeqCst)
            .saturating_add(cancelled);
        self.cancelled_by_stop
            .store(since_the_last_lift, Ordering::SeqCst);
        Stop {
            cancelled,
            number,
            exhausted,
            previous,
        }
    }

    /// A resume's first step, in one hold of the runs lock: the stop it
    /// will ask the owner to lift. Poison first; then quitting refuses,
    /// nothing stopped is nothing to lift (`None`), and an exhausted stop
    /// number refuses.
    pub(crate) fn stop_to_lift(&self) -> Result<Option<StopToLift>, Refusal> {
        let Ok(_runs) = self.runs.lock() else {
            return Err(Refusal::Unavailable);
        };
        if self.closed.load(Ordering::SeqCst) {
            return Err(Refusal::TooLate);
        }
        if !self.stopped.load(Ordering::SeqCst) {
            return Ok(None);
        }
        if self.exhausted.load(Ordering::SeqCst) {
            return Err(Refusal::Exhausted);
        }
        Ok(Some(StopToLift {
            number: self.stop_number.load(Ordering::SeqCst),
            cancelled: self.cancelled_by_stop(),
        }))
    }

    /// A resume's early check, once the owner has confirmed lifting stop
    /// `number`: what `lift` would answer now, read under the runs lock,
    /// with the same tests in the same order, changing nothing.
    pub(crate) fn check_lift(&self, number: u64) -> Result<Lift, Refusal> {
        let Ok(_runs) = self.runs.lock() else {
            return Err(Refusal::Unavailable);
        };
        if self.closed.load(Ordering::SeqCst) {
            return Err(Refusal::TooLate);
        }
        if self.exhausted.load(Ordering::SeqCst) {
            return Err(Refusal::Exhausted);
        }
        if self.stop_number.load(Ordering::SeqCst) != number {
            return Err(Refusal::Superseded);
        }
        if !self.stopped.load(Ordering::SeqCst) {
            return Ok(Lift::AlreadyLifted);
        }
        Ok(Lift::Lifted)
    }

    /// A resume's compare-and-clear, in one hold of the runs lock: lift
    /// stop `number` only if it is still the stop in force. Poison first;
    /// then quitting, exhaustion and a newer stop refuse, and a stop already
    /// lifted is left as it is. Only a lift changes anything: the flag, the
    /// last lifted stop, the count.
    pub(crate) fn lift(&self, number: u64) -> Result<Lift, Refusal> {
        let Ok(_runs) = self.runs.lock() else {
            return Err(Refusal::Unavailable);
        };
        if self.closed.load(Ordering::SeqCst) {
            return Err(Refusal::TooLate);
        }
        if self.exhausted.load(Ordering::SeqCst) {
            return Err(Refusal::Exhausted);
        }
        if self.stop_number.load(Ordering::SeqCst) != number {
            return Err(Refusal::Superseded);
        }
        if !self.stopped.load(Ordering::SeqCst) {
            return Ok(Lift::AlreadyLifted);
        }
        self.stopped.store(false, Ordering::SeqCst);
        self.last_lifted.store(number, Ordering::SeqCst);
        self.cancelled_by_stop.store(0, Ordering::SeqCst);
        Ok(Lift::Lifted)
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

    /// Take and release the runs' lock once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        drop(self.runs.lock().expect("runs"));
    }

    /// Put the registry in the state "stop `number` was lifted": the stop
    /// number and the last lifted stop, set together (tests near the stop
    /// number's maximum).
    #[cfg(test)]
    pub(crate) fn set_lifted_stop(&self, number: u64) {
        let _runs = self.runs.lock().expect("runs");
        self.stop_number.store(number, Ordering::SeqCst);
        self.last_lifted.store(number, Ordering::SeqCst);
    }

    /// Take the runs lock and panic while holding it, which poisons it
    /// (tests, on a thread the test joins; not a state production reaches).
    #[cfg(test)]
    pub(crate) fn panic_holding_the_runs_lock(&self) {
        let _runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        panic!("the runs lock is poisoned on purpose (test)");
    }

    /// Whose run it is (`None`: no such run).
    pub fn class(&self, run: RunId) -> Option<RunClass> {
        self.runs.lock().expect("runs").get(&run).map(|e| e.class)
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
