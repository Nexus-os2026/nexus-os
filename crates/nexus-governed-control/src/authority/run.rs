//! Governed runs and cancellation.
//!
//! A run is the scope of one command (or agent goal): every commitment
//! belongs to exactly one run and one agent. Cancelling or finishing a run
//! sets its token, which every executing effect observes between its bounded
//! steps (a contained process, a browser session and an input action end
//! their own resources when they see it), and ends its unconsumed
//! commitments with their credential leases. An emergency stop cancels every
//! run.
//!
//! Mother Nexus M2c: the registry also holds the agent doors' groups. A door
//! is its group: a run joins it when the door opens the run (at insertion)
//! or adopts it, and leaves it only when the registry drops the run or the
//! door is gone. Every membership change is made under the runs lock.

use super::ids::{AgentId, RunId};
use super::AuthorityError;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

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
    /// M2c: the door groups the run belongs to (at most
    /// `MAX_GROUPS_PER_RUN` live ones).
    groups: Vec<GroupId>,
}

/// A door's group (Mother Nexus M2c): a monotonic number, never reused.
pub(crate) type GroupId = u64;

/// What keeps a door's entry alive: the door's own core, held weakly. An
/// entry whose door is dropped is reclaimed at the next issue or adoption;
/// no `Drop` takes a lock or does I/O.
pub(crate) type DoorLife = Weak<dyn Any + Send + Sync>;

/// Most doors alive at once, reserved or issued (N5: a live limit).
pub(crate) const MAX_LIVE_DOORS: usize = 64;
/// Most live door groups one run may belong to.
pub(crate) const MAX_GROUPS_PER_RUN: usize = 8;

/// One door's group: reserved while the owner is asked, active once the
/// door is issued.
struct DoorEntry {
    life: DoorLife,
    agent: AgentId,
    active: bool,
    members: BTreeSet<RunId>,
}

impl DoorEntry {
    fn alive(&self) -> bool {
        self.life.strong_count() > 0
    }
}

/// What the runs lock holds: the runs, and the doors' groups.
#[derive(Default)]
struct Table {
    runs: HashMap<RunId, RunEntry>,
    doors: BTreeMap<GroupId, DoorEntry>,
}

impl Deref for Table {
    type Target = HashMap<RunId, RunEntry>;
    fn deref(&self) -> &Self::Target {
        &self.runs
    }
}

impl DerefMut for Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runs
    }
}

impl Table {
    /// Reclaim the groups of doors that are gone: each entry, and its id in
    /// its members' lists.
    fn prune_doors(&mut self) {
        let gone: Vec<GroupId> = self
            .doors
            .iter()
            .filter(|(_, door)| !door.alive())
            .map(|(group, _)| *group)
            .collect();
        for group in gone {
            if let Some(door) = self.doors.remove(&group) {
                for run in door.members {
                    if let Some(entry) = self.runs.get_mut(&run) {
                        entry.groups.retain(|g| *g != group);
                    }
                }
            }
        }
    }

    /// Drop every cancelled or finished run, and each from its groups.
    fn prune_ended(&mut self) {
        let ended: Vec<RunId> = self
            .runs
            .iter()
            .filter(|(_, e)| e.view.cancelled || e.view.finished)
            .map(|(id, _)| *id)
            .collect();
        for run in ended {
            if let Some(entry) = self.runs.remove(&run) {
                for group in entry.groups {
                    if let Some(door) = self.doors.get_mut(&group) {
                        door.members.remove(&run);
                    }
                }
            }
        }
    }
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
    runs: Mutex<Table>,
    /// M2c: the next door group's number. It never wraps in practice and
    /// is never reused.
    next_group: AtomicU64,
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
    /// Open a run in no door's group (tests).
    #[cfg(test)]
    pub(crate) fn open(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
        now_wall_ms: u64,
    ) -> Result<RunId, AuthorityError> {
        self.open_in(agent, origin, class, now_wall_ms, None)
    }

    /// Open a run; with `group` (M2c), it joins that door's group at
    /// insertion, in the same hold of the lock.
    pub(crate) fn open_in(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
        now_wall_ms: u64,
        group: Option<GroupId>,
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
            groups: group.into_iter().collect(),
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
        if let Some(group) = group {
            match runs.doors.get(&group) {
                Some(door) if door.active && door.alive() => {
                    if door.agent != entry.view.agent {
                        return Err(AuthorityError::WrongAgent);
                    }
                }
                _ => return Err(AuthorityError::NotMember),
            }
        }
        if runs.len() >= RUN_CAPACITY {
            runs.prune_ended();
            if runs.len() >= RUN_CAPACITY {
                return Err(AuthorityError::Capacity);
            }
        }
        runs.insert(id, entry);
        if let Some(door) = group.and_then(|group| runs.doors.get_mut(&group)) {
            door.members.insert(id);
        }
        Ok(id)
    }

    /// M2c: a new door group's number (never reused).
    pub(crate) fn next_group(&self) -> GroupId {
        self.next_group.fetch_add(1, Ordering::SeqCst)
    }

    /// M2c, a door issue's reservation: reclaim the groups of doors that are
    /// gone, refuse at `MAX_LIVE_DOORS` live doors (reserved or issued),
    /// and otherwise reserve `group` for `agent`, kept while `life` lives.
    pub(crate) fn reserve_door(
        &self,
        group: GroupId,
        agent: &AgentId,
        life: DoorLife,
    ) -> Result<(), AuthorityError> {
        let mut runs = self.runs.lock().expect("runs");
        runs.prune_doors();
        if runs.doors.len() >= MAX_LIVE_DOORS {
            return Err(AuthorityError::Capacity);
        }
        runs.doors.insert(
            group,
            DoorEntry {
                life,
                agent: agent.clone(),
                active: false,
                members: BTreeSet::new(),
            },
        );
        Ok(())
    }

    /// M2c, a door issue's registration (after the owner's recorded yes):
    /// the reserved group becomes the issued door's, empty.
    pub(crate) fn activate_door(&self, group: GroupId) -> Result<(), AuthorityError> {
        let mut runs = self.runs.lock().expect("runs");
        match runs.doors.get_mut(&group) {
            Some(door) if door.alive() => {
                door.active = true;
                Ok(())
            }
            _ => Err(AuthorityError::Unavailable(
                "the door's reservation is gone",
            )),
        }
    }

    /// M2c: issued doors still alive.
    pub(crate) fn live_doors(&self) -> usize {
        self.runs
            .lock()
            .expect("runs")
            .doors
            .values()
            .filter(|door| door.active && door.alive())
            .count()
    }

    /// M2c, every door call on a run: the run exists, is `agent`'s and an
    /// agent's run, and lists `group`. Changes nothing.
    pub(crate) fn check_member(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
    ) -> Result<(), AuthorityError> {
        let runs = self.runs.lock().expect("runs");
        let entry = runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        if &entry.view.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.class != RunClass::Agent {
            return Err(AuthorityError::WrongRun);
        }
        if !entry.groups.contains(&group) {
            return Err(AuthorityError::NotMember);
        }
        Ok(())
    }

    /// M2c: whether `run` lists `group` (read-only).
    pub(crate) fn is_member(&self, run: RunId, group: GroupId) -> bool {
        self.runs
            .lock()
            .expect("runs")
            .get(&run)
            .is_some_and(|entry| entry.groups.contains(&group))
    }

    /// M2c, a door's adoption (called holding the commitments lock: the
    /// existing entries → runs order): reclaim gone groups; the run must be
    /// in this registry, `agent`'s, an agent's run, neither finished nor
    /// cancelled, and list fewer than `MAX_GROUPS_PER_RUN` live groups.
    /// Then it joins `group` (once). Any refusal changes nothing.
    pub(crate) fn join(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
    ) -> Result<(), AuthorityError> {
        let mut guard = self.runs.lock().expect("runs");
        let table = &mut *guard;
        table.prune_doors();
        if !table
            .doors
            .get(&group)
            .is_some_and(|door| door.active && door.alive() && &door.agent == agent)
        {
            return Err(AuthorityError::NotMember);
        }
        let entry = table.runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        if &entry.view.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        if entry.class != RunClass::Agent {
            return Err(AuthorityError::WrongRun);
        }
        if entry.view.finished {
            return Err(AuthorityError::RunNotActive);
        }
        if entry.view.cancelled || entry.cancelled.load(Ordering::SeqCst) {
            return Err(AuthorityError::RunCancelled);
        }
        if entry.groups.contains(&group) {
            return Ok(());
        }
        if entry.groups.len() >= MAX_GROUPS_PER_RUN {
            return Err(AuthorityError::Capacity);
        }
        entry.groups.push(group);
        if let Some(door) = table.doors.get_mut(&group) {
            door.members.insert(run);
        }
        Ok(())
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
