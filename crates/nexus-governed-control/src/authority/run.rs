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
//!
//! M2d-B1 (design v2 §11 H1′): each door's group also keeps a bounded table
//! of the Owner's yeses given at that door in this process (mission consent
//! and held-mission resume). An entry carries the lifetime it was asked in
//! (the stop epoch, the door's halt state and closure, read under this lock
//! before the Owner is asked) and is inserted only if that lifetime still
//! holds; a Stop or a halt empties the table, and a stale entry never
//! admits. A run's Files admission binds it once, for good, to one such yes
//! and the registered workspace, validated in the same hold of this lock as
//! the door's life, the run's membership and life, Stop and closure.
//! Nothing here reads evidence: consent is never reconstructed from it.

use super::evidence::{EvidencePhase, EvidenceRecord, Privacy};
use super::file_session::{HaltCell, RecoveryCheck};
use super::halt::{HaltReason, RecordBatch};
use super::ids::{AgentId, Digest, RunId};
use super::AuthorityError;
use crate::inert_page::ApprovedFacts;
use crate::inference::MAX_MODEL_ATTEMPTS;
use nexus_kernel::inference::ModelError;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

/// A run's cancellation flag, observed by actuators between bounded steps.
#[derive(Clone, Debug)]
pub struct CancelToken(Arc<AtomicBool>, Option<HaltCell>);

fn request_cell<'a>(
    original: Option<&'a HaltCell>,
    request: Option<&'a HaltCell>,
) -> Result<Option<&'a HaltCell>, AuthorityError> {
    match request {
        None => Ok(original),
        Some(request)
            if original.is_some_and(|original| original.host() && original.same(request))
                && request.host() =>
        {
            Ok(Some(request))
        }
        Some(_) => Err(AuthorityError::Halted),
    }
}

impl CancelToken {
    pub(crate) fn admit_host(&self) -> Result<(), AuthorityError> {
        match &self.1 {
            Some(cell) if cell.host() => {
                if self.0.load(Ordering::SeqCst) {
                    return Err(AuthorityError::RunCancelled);
                }
                cell.admit()
            }
            Some(cell) if cell.is_engaged() => Err(AuthorityError::Halted),
            _ => Ok(()),
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) || self.1.as_ref().is_some_and(HaltCell::is_engaged)
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

impl RunClass {
    /// The privacy class of a run of this class (P0, I6): `Owner` only for
    /// an owner run; every other run's evidence is Restricted.
    pub(crate) fn privacy(self) -> Privacy {
        match self {
            RunClass::Owner => Privacy::Owner,
            RunClass::Agent => Privacy::Restricted,
        }
    }
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
    /// P0: its privacy class, fixed from `class` at insertion and never
    /// changed (door membership, which can be reclaimed, plays no part).
    privacy: Privacy,
    cancelled: Arc<AtomicBool>,
    file_cell: Option<HaltCell>,
    /// M2c: the door groups the run belongs to (at most
    /// `MAX_GROUPS_PER_RUN` live ones).
    groups: Vec<GroupId>,
    /// Immutable original door, retained even when live membership is pruned.
    opened_by: Option<GroupId>,
    /// Unacknowledged File commitments. An ended run with any stays retained.
    retained: usize,
    /// M2d-B1: the run's Files admission, set once and never changed.
    files_admission: Option<FilesAdmission>,
    /// M2e-B: the run's inference admission, set once and never changed.
    inference_admission: Option<InferenceAdmission>,
    /// M2e-B: the model calls this run attempted (never refunded).
    model_attempts: u32,
}

/// M2d-B1: which yes of the Owner's a cached consent is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConsentKind {
    /// The Owner's consent to a mission (`confirm_mission`).
    Mission,
    /// The Owner's yes to resuming a held mission (`confirm_held_mission`).
    Resume,
}

impl ConsentKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ConsentKind::Mission => "mission consent",
            ConsentKind::Resume => "held-mission resume",
        }
    }
}

/// The lifetime a door's yes was asked in (H1′): the stop epoch (this
/// process's stop number), the door's halt state and closure, read under
/// the runs lock before the Owner was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Life {
    stop_epoch: u64,
    halted: bool,
    closed: bool,
}

/// One cached yes of the Owner's at a door (H1′).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Consent {
    contract_hash: String,
    request: String,
    evidence_seq: u64,
    kind: ConsentKind,
    at_wall_ms: u64,
    life: Life,
    /// S1a-D+B (M3 §6.1): the page facts the broker's own dialog showed
    /// with this yes; part of its identity, so they live and end with it.
    page_facts: Option<ApprovedFacts>,
}

/// The workspace a run is admitted to: its id, and its directory's identity
/// when the admission was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdmittedWorkspace {
    pub(crate) id: String,
    pub(crate) identity: Digest,
}

/// M2d-B1 (A3): a run's Files admission: the Owner's yes it rests on (the
/// contract hash, the request, the record's sequence number, its kind and
/// time) and the workspace. Installed once; never changed or taken from an
/// intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FilesAdmission {
    pub(crate) contract_hash: String,
    pub(crate) request: String,
    pub(crate) evidence_seq: u64,
    pub(crate) kind: ConsentKind,
    pub(crate) consented_wall_ms: u64,
    pub(crate) workspace: AdmittedWorkspace,
    /// S1a-D+B (S1a O8): the page facts the Owner's yes was given with,
    /// exactly as the broker's own dialog showed them; `None` for a yes
    /// given without facts. In memory only; part of the admission's
    /// identity.
    pub(crate) page_facts: Option<ApprovedFacts>,
}

/// M2e-B (design v2 §8): a run's inference admission: the Owner's yes it
/// rests on (as for Files) and the model identity it was admitted for.
/// Installed once; never changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InferenceAdmission {
    pub(crate) contract_hash: String,
    pub(crate) request: String,
    pub(crate) evidence_seq: u64,
    pub(crate) kind: ConsentKind,
    pub(crate) consented_wall_ms: u64,
    pub(crate) identity: [u8; 32],
}

/// M2e-B: what an admitted model call is ended by besides the kernel
/// stop, taken in the same hold of the runs lock that admitted it: the
/// run's token and its door's halt flag.
pub(crate) struct CallTicket {
    pub(crate) token: CancelToken,
    door_halted: Arc<AtomicBool>,
}

impl CallTicket {
    /// Whether the admitting door was halted since (set in the same hold of
    /// the runs lock that marks the door halted and cancels its runs).
    pub(crate) fn door_halted(&self) -> bool {
        self.door_halted.load(Ordering::SeqCst)
    }
}

/// Most yeses one door caches, reserved slots included (H1′): the next is
/// refused before the Owner is asked.
pub(crate) const MAX_CONSENTS: usize = 64;

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
    /// M2c PR-B: halted (final): its runs refuse every admission.
    halted: bool,
    file_cell: Option<HaltCell>,
    /// M2e-B: `halted`, readable without the lock by an admitted model
    /// call (set in the same hold that sets `halted`).
    halt_flag: Arc<AtomicBool>,
    members: BTreeSet<RunId>,
    /// M2d-B1 (H1′): the Owner's yeses cached at this door.
    consents: Vec<Consent>,
    /// M2d-B1: slots reserved for yeses whose dialog is open.
    pending_consents: usize,
}

impl DoorEntry {
    fn alive(&self) -> bool {
        self.life.strong_count() > 0
    }
}

/// Host work checks its original live door and the native consent binding,
/// independently of whichever accepted request owns this particular step.
fn host_prerequisites(
    table: &Table,
    entry: &RunEntry,
    stop_epoch: u64,
) -> Result<(), AuthorityError> {
    if !entry.file_cell.as_ref().is_some_and(HaltCell::host) {
        return Ok(());
    }
    let group = entry.opened_by.ok_or(AuthorityError::NotMember)?;
    let door = table.doors.get(&group).ok_or(AuthorityError::NotMember)?;
    if !door.active
        || !door.alive()
        || door.halted
        || door.agent != entry.view.agent
        || !door.members.contains(&entry.view.id)
        || !entry.groups.contains(&group)
    {
        return Err(AuthorityError::NotMember);
    }
    if let Some(admission) = &entry.files_admission {
        if !door.consents.iter().any(|consent| {
            consent.contract_hash == admission.contract_hash
                && consent.request == admission.request
                && consent.evidence_seq == admission.evidence_seq
                && consent.page_facts == admission.page_facts
                && consent.life.stop_epoch == stop_epoch
                && !consent.life.halted
                && !consent.life.closed
        }) {
            return Err(AuthorityError::NoConsent);
        }
    }
    Ok(())
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
    /// Whether `entry` lists a halted group.
    fn in_halted_group(&self, entry: &RunEntry) -> bool {
        entry
            .groups
            .iter()
            .any(|group| self.doors.get(group).is_some_and(|door| door.halted))
    }

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
            .filter(|(_, e)| {
                (e.view.cancelled || e.view.finished)
                    && e.retained == 0
                    // Keep an admitted session's provenance even before its
                    // first commitment pin. Bounded by RUN_CAPACITY; capacity
                    // pressure refuses new runs, never erases the cell.
                    && !(e.file_cell.is_some() && e.files_admission.is_some())
            })
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
    #[cfg(test)]
    pub(crate) review2_hooks: crate::test_hooks::Hooks,
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
    #[cfg(test)]
    capacity: AtomicUsize,
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
    /// A session ingress invalidated the Owner recovery request.
    Halted,
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
    fn capacity(&self) -> usize {
        #[cfg(test)]
        {
            let set = self.capacity.load(Ordering::SeqCst);
            if set > 0 {
                return set;
            }
        }
        RUN_CAPACITY
    }

    #[cfg(test)]
    pub(crate) fn set_capacity_for_test(&self, capacity: usize) {
        self.capacity.store(capacity, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn len_for_test(&self) -> usize {
        self.runs.lock().expect("runs").len()
    }

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
    #[cfg(test)]
    pub(crate) fn open_in(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
        now_wall_ms: u64,
        group: Option<GroupId>,
    ) -> Result<RunId, AuthorityError> {
        self.open_with_cell(agent, origin, class, now_wall_ms, group, None)
    }

    pub(crate) fn open_with_cell(
        &self,
        agent: AgentId,
        origin: RunOrigin,
        class: RunClass,
        now_wall_ms: u64,
        group: Option<GroupId>,
        file_cell: Option<HaltCell>,
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
            privacy: class.privacy(),
            cancelled: Arc::new(AtomicBool::new(false)),
            file_cell,
            groups: group.into_iter().collect(),
            opened_by: group,
            retained: 0,
            files_admission: None,
            inference_admission: None,
            model_attempts: 0,
        };
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        if entry.file_cell.as_ref().is_some_and(HaltCell::is_engaged) {
            return Err(AuthorityError::Halted);
        }
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
                Some(door)
                    if door.halted || door.file_cell.as_ref().is_some_and(HaltCell::is_engaged) =>
                {
                    return Err(AuthorityError::Halted)
                }
                Some(door) if door.active && door.alive() => {
                    match (&door.file_cell, &entry.file_cell) {
                        (None, None) => {}
                        (Some(a), Some(b)) if a.same(b) && !a.is_engaged() => {}
                        _ => return Err(AuthorityError::Halted),
                    }
                    if door.agent != entry.view.agent {
                        return Err(AuthorityError::WrongAgent);
                    }
                }
                _ => return Err(AuthorityError::NotMember),
            }
        }
        if runs.len() >= self.capacity() {
            runs.prune_ended();
            if runs.len() >= self.capacity() {
                return Err(AuthorityError::Capacity);
            }
        }
        #[cfg(test)]
        self.review2_hooks.fire("run.before_insert");
        // Final same-word comparison is the admission linearization point,
        // under the same hold as insertion, after all validation/pruning.
        super::file_session::check_halt(entry.file_cell.as_ref())?;
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
                halted: false,
                file_cell: None,
                halt_flag: Arc::new(AtomicBool::new(false)),
                members: BTreeSet::new(),
                consents: Vec::new(),
                pending_consents: 0,
            },
        );
        Ok(())
    }

    /// M2c, a door issue's registration (after the owner's recorded yes):
    /// the reserved group becomes the issued door's, empty.
    #[cfg(test)]
    pub(crate) fn activate_door(&self, group: GroupId) -> Result<(), AuthorityError> {
        self.activate_door_with_halt(group, None)
    }

    pub(crate) fn activate_door_with_halt(
        &self,
        group: GroupId,
        halt: Option<&HaltCell>,
    ) -> Result<(), AuthorityError> {
        let mut runs = self.runs.lock().expect("runs");
        match runs.doors.get_mut(&group) {
            Some(door) if door.alive() => {
                // Decision 8: activation and its final check share this hold.
                super::file_session::check_halt(halt)?;
                door.file_cell = halt.cloned();
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
    /// agent's run, and lists `group`; PR-B: and the group is not halted,
    /// unless `halted_too` (a cancellation only stops effects). Changes
    /// nothing.
    pub(crate) fn check_member(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        halted_too: bool,
    ) -> Result<(), AuthorityError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        if !halted_too && runs.doors.get(&group).is_some_and(|door| door.halted) {
            return Err(AuthorityError::Halted);
        }
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
        if !halted_too
            && runs
                .doors
                .get(&group)
                .and_then(|d| d.file_cell.as_ref())
                .is_some_and(|cell| {
                    !entry
                        .file_cell
                        .as_ref()
                        .is_some_and(|run_cell| run_cell.same(cell))
                })
        {
            return Err(AuthorityError::NotMember);
        }
        if !halted_too && entry.file_cell.as_ref().is_some_and(HaltCell::is_engaged) {
            return Err(AuthorityError::Halted);
        }
        Ok(())
    }

    pub(crate) fn attach_file_cell(
        &self,
        group: GroupId,
        cell: HaltCell,
    ) -> Result<(), AuthorityError> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let door = runs
            .doors
            .get_mut(&group)
            .ok_or(AuthorityError::NotMember)?;
        // A host may already have attached this exact deny-only session
        // cell at issuance. File selection never replaces another cell.
        if door
            .file_cell
            .as_ref()
            .is_some_and(|bound| !bound.same(&cell))
        {
            return Err(AuthorityError::Halted);
        }
        super::file_session::check_halt(Some(&cell))?;
        door.file_cell = Some(cell);
        Ok(())
    }

    pub(crate) fn try_door_halted(&self, group: GroupId) -> Result<bool, AuthorityError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let door = runs.doors.get(&group).ok_or(AuthorityError::NotMember)?;
        Ok(door.halted || door.file_cell.as_ref().is_some_and(HaltCell::is_engaged))
    }

    /// M2c PR-B, the halt's first step, in one hold of the runs lock, taken
    /// even if poisoned: mark `group` halted (its `door_halted` record goes
    /// to the call that set it); snapshot every member, cancelled ones too
    /// (§11 C1: the later steps use only this snapshot); cancel every
    /// active member, each record pushed as it is made. Returns the
    /// snapshot, and whether this call set the flag. No other lock, no I/O.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn halt_group(
        &self,
        group: GroupId,
        door: &str,
        agent: &AgentId,
        reason: HaltReason,
        wall_ms: u64,
        generation: u64,
        batch: &mut RecordBatch,
    ) -> (BTreeSet<RunId>, bool) {
        let mut guard = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        let table = &mut *guard;
        let Some(entry) = table.doors.get_mut(&group) else {
            return (BTreeSet::new(), false);
        };
        let newly = !entry.halted;
        // H1′: a halt invalidates every yes the door cached.
        entry.consents.clear();
        if newly {
            entry.halted = true;
            entry.halt_flag.store(true, Ordering::SeqCst);
            // A door's record: its agent, its opaque id and a reason code.
            let mut record = EvidenceRecord::new(
                EvidencePhase::DoorHalted,
                wall_ms,
                generation,
                Privacy::Restricted,
            );
            record.agent = Some(agent.to_string());
            record.cancelled = true;
            record.detail.push(("door".into(), door.to_string()));
            record
                .detail
                .push(("reason".into(), reason.as_str().into()));
            batch.push(record);
        }
        let members = entry.members.clone();
        for run in &members {
            if let Some(run_entry) = table.runs.get_mut(run) {
                cancel_one(run_entry, wall_ms, generation, batch);
            }
        }
        (members, newly)
    }

    /// M2c PR-B, a door's cancellation of one run, under the runs lock taken
    /// even if poisoned: its record goes into `batch` if this call made the
    /// transition.
    pub(crate) fn cancel_into(
        &self,
        run: RunId,
        wall_ms: u64,
        generation: u64,
        batch: &mut RecordBatch,
    ) {
        let mut runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = runs.get_mut(&run) {
            cancel_one(entry, wall_ms, generation, batch);
        }
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
        #[cfg(test)]
        self.review2_hooks.fire("run.before_join");
        match table.doors.get(&group) {
            Some(door)
                if door.halted || door.file_cell.as_ref().is_some_and(HaltCell::is_engaged) =>
            {
                return Err(AuthorityError::Halted)
            }
            Some(door) if door.active && door.alive() && &door.agent == agent => {}
            _ => return Err(AuthorityError::NotMember),
        }
        let session_cell = table
            .doors
            .get(&group)
            .and_then(|door| door.file_cell.clone());
        // A session constraint must exist at run/token issuance. Never retrofit
        // it through adoption: existing guards and preparation may hold None.
        // Check immutable origin AND cell identity before even pruning state.
        if let Some(cell) = &session_cell {
            let entry = table.runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
            if entry.opened_by != Some(group)
                || !entry
                    .file_cell
                    .as_ref()
                    .is_some_and(|bound| bound.same(cell))
            {
                return Err(AuthorityError::NotMember);
            }
        }
        table.prune_doors();
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
        if entry.file_cell.is_some() {
            return Err(AuthorityError::NotMember);
        }
        if entry.groups.len() >= MAX_GROUPS_PER_RUN {
            return Err(AuthorityError::Capacity);
        }
        super::file_session::check_halt(session_cell.as_ref())?;
        entry.groups.push(group);
        if let Some(door) = table.doors.get_mut(&group) {
            door.members.insert(run);
        }
        Ok(())
    }

    /// Membership read for File lifecycle evidence. Cancellation does not
    /// remove ownership; an unreadable registry must remain an error.
    pub(crate) fn try_is_member(&self, run: RunId, group: GroupId) -> Result<bool, AuthorityError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        Ok(runs
            .get(&run)
            .is_some_and(|entry| entry.groups.contains(&group)))
    }

    /// Copy only the optional deny-only cell, never authority. Used to
    /// carry the same session constraint through slow commitment preparation.
    pub(crate) fn session_cell(&self, run: RunId) -> Result<Option<HaltCell>, AuthorityError> {
        let runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        Ok(runs
            .get(&run)
            .ok_or(AuthorityError::Halted)?
            .file_cell
            .clone())
    }

    /// Missing, pruned and poisoned authority are halted, never cell-less.
    pub(crate) fn session_halted(&self, run: RunId) -> bool {
        self.session_cell(run)
            .map_or(true, |cell| cell.as_ref().is_some_and(HaltCell::is_engaged))
    }

    /// Final staging admission, under the same table hold as the lookup.
    /// Session runs retain provenance through the pre-commitment interval.
    pub(crate) fn admit_file_stage(&self, run: RunId) -> Result<(), AuthorityError> {
        self.admit_file_request(run, None)
    }

    pub(crate) fn admit_file_request(
        &self,
        run: RunId,
        request: Option<&HaltCell>,
    ) -> Result<(), AuthorityError> {
        let runs = self.runs.lock().map_err(|_| AuthorityError::Halted)?;
        let entry = runs.get(&run).ok_or(AuthorityError::Halted)?;
        host_prerequisites(&runs, entry, self.stop_number.load(Ordering::SeqCst))?;
        if entry.file_cell.is_some()
            && (entry.view.cancelled
                || entry.view.finished
                || entry.cancelled.load(Ordering::SeqCst)
                || runs.in_halted_group(entry)
                || self.stopped.load(Ordering::SeqCst)
                || self.closed.load(Ordering::SeqCst))
        {
            return Err(AuthorityError::Halted);
        }
        let cell = request_cell(entry.file_cell.as_ref(), request)?;
        super::file_session::check_halt(cell)
    }

    /// Memory-only native-lift handoff. No I/O or evidence callback is allowed
    /// in `lift`; the native kernel mutex is acquired only after this check.
    pub(crate) fn recovery_boundary(
        &self,
        recovery: &RecoveryCheck,
        lift: impl FnOnce(),
    ) -> Result<(), AuthorityError> {
        let _runs = self.runs.lock().map_err(|_| AuthorityError::Halted)?;
        recovery.check()?;
        lift();
        Ok(())
    }

    /// The run exists, belongs to `agent`, is not cancelled or finished, and
    /// no emergency stop is in force.
    pub(crate) fn check(&self, run: RunId, agent: &AgentId) -> Result<CancelToken, AuthorityError> {
        self.check_retaining(run, agent, false)
            .map(|(token, _)| token)
    }

    /// Check and retain in the SAME run-registry hold. Called under the
    /// commitment registry's lock: insertion cannot race a run's pruning.
    pub(crate) fn check_retaining(
        &self,
        run: RunId,
        agent: &AgentId,
        retain: bool,
    ) -> Result<(CancelToken, Option<GroupId>), AuthorityError> {
        self.check_request_retaining(run, agent, retain, None)
    }

    pub(crate) fn check_request(
        &self,
        run: RunId,
        agent: &AgentId,
        request: Option<&HaltCell>,
    ) -> Result<CancelToken, AuthorityError> {
        self.check_request_retaining(run, agent, false, request)
            .map(|(token, _)| token)
    }

    fn check_request_retaining(
        &self,
        run: RunId,
        agent: &AgentId,
        retain: bool,
        request: Option<&HaltCell>,
    ) -> Result<(CancelToken, Option<GroupId>), AuthorityError> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        let entry = runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        if &entry.view.agent != agent {
            return Err(AuthorityError::WrongAgent);
        }
        let cell = request_cell(entry.file_cell.as_ref(), request)?;
        host_prerequisites(&runs, entry, self.stop_number.load(Ordering::SeqCst))?;
        if cell.is_some_and(HaltCell::is_engaged) || runs.in_halted_group(entry) {
            return Err(AuthorityError::Halted);
        }
        if entry.view.finished {
            return Err(AuthorityError::RunNotActive);
        }
        if entry.view.cancelled || entry.cancelled.load(Ordering::SeqCst) {
            return Err(AuthorityError::RunCancelled);
        }
        let owner = entry.opened_by;
        let token = CancelToken(entry.cancelled.clone(), cell.cloned());
        if retain {
            owner.ok_or(AuthorityError::NotMember)?;
            let entry = runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
            let retained = entry
                .retained
                .checked_add(1)
                .ok_or(AuthorityError::Capacity)?;
            token.admit_host()?;
            entry.retained = retained;
        }
        Ok((token, owner))
    }

    /// Release precisely one retained obligation. Only the commitment
    /// registry, after validating its exact pinned entry, may call this.
    pub(crate) fn release(&self, run: RunId) -> Result<(), AuthorityError> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let entry = runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        entry.retained = entry
            .retained
            .checked_sub(1)
            .ok_or(AuthorityError::Unavailable(
                "the run has no retained obligation",
            ))?;
        Ok(())
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
        // H1′: a Stop invalidates every yes every door cached (the stop
        // number moved too, so no slot reserved before it fills).
        for door in runs.doors.values_mut() {
            door.consents.clear();
        }
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

    pub(crate) fn lift_with_recovery(
        &self,
        number: u64,
        recovery: &RecoveryCheck,
    ) -> Result<Lift, Refusal> {
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
        recovery.check().map_err(|_| Refusal::Halted)?;
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

    /// M2d-B1 (H1′), before the Owner is asked a mission or held-mission
    /// question at a door: under the runs lock, refuse under a Stop, while
    /// quitting, for a door that is not live and issued, halted or another
    /// agent's, or at `MAX_CONSENTS` (cached and reserved); otherwise
    /// reserve one slot and read the lifetime the yes is asked in.
    pub(crate) fn consent_slot(
        &self,
        group: GroupId,
        agent: &AgentId,
    ) -> Result<ConsentSlot<'_>, AuthorityError> {
        let mut runs = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        let door = live_door(&mut runs.doors, group, agent)?;
        if door.consents.len().saturating_add(door.pending_consents) >= MAX_CONSENTS {
            return Err(AuthorityError::Capacity);
        }
        door.pending_consents += 1;
        Ok(ConsentSlot {
            registry: self,
            group,
            life: Life {
                stop_epoch: self.stop_number.load(Ordering::SeqCst),
                halted: false,
                closed: false,
            },
            held: true,
        })
    }

    /// M2d-B1 (A3, H1′) and M2e-B, what a door's admission of `run`
    /// shares, under the runs lock the caller holds: no Stop, not quitting;
    /// the door live, issued, not halted and `agent`'s; the run `agent`'s
    /// agent run, listing the group, in no halted group, neither finished
    /// nor cancelled; a yes to exactly `contract_hash` and `request` cached
    /// at this door in the current lifetime, which is returned. Changes
    /// nothing.
    ///
    /// S1a-D+B (S1a O8, M3 §6.1): yeses to the same hash and request that
    /// disagree on the page facts admit nothing (`no_consent`): which facts
    /// the Owner meant is not established. The rule holds for every
    /// admission, Files and inference (fail closed); only a Files admission
    /// keeps the facts.
    #[allow(clippy::too_many_arguments)]
    fn admissible(
        &self,
        table: &mut Table,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        contract_hash: &str,
        request: &str,
    ) -> Result<Consent, AuthorityError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        let now = Life {
            stop_epoch: self.stop_number.load(Ordering::SeqCst),
            halted: false,
            closed: false,
        };
        let door = live_door(&mut table.doors, group, agent)?;
        let mut matching = door
            .consents
            .iter()
            .filter(|c| c.life == now && c.contract_hash == contract_hash && c.request == request);
        let consent = matching.next().cloned();
        let agreed = consent
            .as_ref()
            .is_some_and(|first| matching.all(|c| c.page_facts == first.page_facts));
        let entry = table.runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        live_member(table, entry, agent, group)?;
        consent.filter(|_| agreed).ok_or(AuthorityError::NoConsent)
    }

    /// M2d-B1 (A3, H1′), a door's admission of `run` for Files, in one hold
    /// of the runs lock: [`Self::admissible`]. Then the binding is
    /// installed, once: the same admission again is the same binding,
    /// another is refused. Any refusal changes nothing.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_files(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        contract_hash: &str,
        request: &str,
        workspace: AdmittedWorkspace,
    ) -> Result<FilesAdmission, AuthorityError> {
        let mut guard = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let table = &mut *guard;
        let consent = self.admissible(table, run, agent, group, contract_hash, request)?;
        #[cfg(test)]
        self.review2_hooks.fire("files.before_install");
        if table
            .runs
            .get(&run)
            .and_then(|entry| entry.file_cell.as_ref())
            .is_some_and(HaltCell::is_engaged)
        {
            return Err(AuthorityError::Halted);
        }
        let admission = FilesAdmission {
            contract_hash: consent.contract_hash,
            request: consent.request,
            evidence_seq: consent.evidence_seq,
            kind: consent.kind,
            consented_wall_ms: consent.at_wall_ms,
            workspace,
            page_facts: consent.page_facts,
        };
        let entry = table.runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        match &entry.files_admission {
            Some(bound)
                if bound.contract_hash == admission.contract_hash
                    && bound.request == admission.request
                    && bound.workspace == admission.workspace
                    && bound.page_facts == admission.page_facts =>
            {
                Ok(bound.clone())
            }
            Some(_) => Err(AuthorityError::AlreadyAdmitted),
            None => {
                super::file_session::admit_host(entry.file_cell.as_ref())?;
                entry.files_admission = Some(admission.clone());
                Ok(admission)
            }
        }
    }

    /// M2e-B (design v2 §8), a door's admission of `run` for inference, in
    /// one hold of the runs lock: [`Self::admissible`] (every H1′ rule,
    /// shared with Files). Then the binding to the yes and to `identity`
    /// is installed, once: the same admission again is the same binding,
    /// another is refused. Any refusal changes nothing.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_inference(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        contract_hash: &str,
        request: &str,
        identity: [u8; 32],
    ) -> Result<InferenceAdmission, AuthorityError> {
        let mut guard = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let table = &mut *guard;
        let consent = self.admissible(table, run, agent, group, contract_hash, request)?;
        #[cfg(test)]
        self.review2_hooks.fire("inference.before_install");
        if table
            .runs
            .get(&run)
            .and_then(|entry| entry.file_cell.as_ref())
            .is_some_and(HaltCell::is_engaged)
        {
            return Err(AuthorityError::Halted);
        }
        let admission = InferenceAdmission {
            contract_hash: consent.contract_hash,
            request: consent.request,
            evidence_seq: consent.evidence_seq,
            kind: consent.kind,
            consented_wall_ms: consent.at_wall_ms,
            identity,
        };
        let entry = table.runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        match &entry.inference_admission {
            Some(bound)
                if bound.contract_hash == admission.contract_hash
                    && bound.request == admission.request
                    && bound.identity == admission.identity =>
            {
                Ok(bound.clone())
            }
            Some(_) => Err(AuthorityError::AlreadyAdmitted),
            None => {
                entry.inference_admission = Some(admission.clone());
                Ok(admission)
            }
        }
    }

    /// M2e-B (design v2 §6 step 3), a door's model call on `run`, in one
    /// hold of the runs lock: no Stop, not quitting; the door live, issued,
    /// not halted and `agent`'s; the run `agent`'s agent run, listing the
    /// group, in no halted group, neither finished nor cancelled; its
    /// inference admission for `identity`; fewer than
    /// [`MAX_MODEL_ATTEMPTS`] attempts so far. The attempt is then counted
    /// (never refunded) and the run's token and the door's halt flag are
    /// returned. Any refusal changes nothing.
    pub(crate) fn admit_model_call(
        &self,
        run: RunId,
        agent: &AgentId,
        group: GroupId,
        identity: &[u8; 32],
    ) -> Result<CallTicket, AuthorityError> {
        let mut guard = self
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        let table = &mut *guard;
        if self.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        let door_halted = live_door(&mut table.doors, group, agent)?.halt_flag.clone();
        let entry = table.runs.get(&run).ok_or(AuthorityError::UnknownRun)?;
        live_member(table, entry, agent, group)?;
        let admission = entry
            .inference_admission
            .as_ref()
            .ok_or(AuthorityError::NotAdmitted)?;
        if &admission.identity != identity {
            return Err(AuthorityError::ModelIdentityMismatch);
        }
        if entry.model_attempts >= MAX_MODEL_ATTEMPTS {
            return Err(AuthorityError::ModelCall(ModelError::OverBudget));
        }
        let entry = table.runs.get_mut(&run).ok_or(AuthorityError::UnknownRun)?;
        #[cfg(test)]
        self.review2_hooks.fire("model.before_attempt");
        if entry.file_cell.as_ref().is_some_and(HaltCell::is_engaged) {
            return Err(AuthorityError::Halted);
        }
        entry.model_attempts += 1;
        Ok(CallTicket {
            token: CancelToken(entry.cancelled.clone(), entry.file_cell.clone()),
            door_halted,
        })
    }

    /// M2d-B1: `run`'s Files admission, if it has one.
    pub(crate) fn files_admission(&self, run: RunId) -> Option<FilesAdmission> {
        self.runs
            .lock()
            .ok()?
            .get(&run)
            .and_then(|entry| entry.files_admission.clone())
    }

    /// M2e-B: `run`'s inference admission, if it has one (tests: a call
    /// reads it under the lock it admits in).
    #[cfg(test)]
    pub(crate) fn inference_admission(&self, run: RunId) -> Option<InferenceAdmission> {
        self.runs
            .lock()
            .ok()?
            .get(&run)
            .and_then(|entry| entry.inference_admission.clone())
    }

    /// The yeses cached at `group` (tests).
    #[cfg(test)]
    pub(crate) fn consents_cached(&self, group: GroupId) -> usize {
        self.runs
            .lock()
            .expect("runs")
            .doors
            .get(&group)
            .map_or(0, |door| door.consents.len())
    }

    /// P0: the privacy class `run` was given at insertion. A run this
    /// registry does not know (never opened, or pruned), or a poisoned
    /// lock, is Restricted: missing state never selects the Owner class.
    pub(crate) fn privacy(&self, run: RunId) -> Privacy {
        self.runs
            .lock()
            .ok()
            .and_then(|runs| runs.get(&run).map(|entry| entry.privacy))
            .unwrap_or(Privacy::Restricted)
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

/// The door entry of `group` if it is live, issued, not halted and
/// `agent`'s. Under the runs lock.
fn live_door<'a>(
    doors: &'a mut BTreeMap<GroupId, DoorEntry>,
    group: GroupId,
    agent: &AgentId,
) -> Result<&'a mut DoorEntry, AuthorityError> {
    match doors.get_mut(&group) {
        Some(door) if door.halted || door.file_cell.as_ref().is_some_and(HaltCell::is_engaged) => {
            Err(AuthorityError::Halted)
        }
        Some(door) if door.active && door.alive() && &door.agent == agent => Ok(door),
        _ => Err(AuthorityError::NotMember),
    }
}

/// `entry` is `agent`'s agent run, lists `group`, is in no halted group
/// and is neither finished nor cancelled. Under the runs lock.
fn live_member(
    table: &Table,
    entry: &RunEntry,
    agent: &AgentId,
    group: GroupId,
) -> Result<(), AuthorityError> {
    if &entry.view.agent != agent {
        return Err(AuthorityError::WrongAgent);
    }
    if entry.class != RunClass::Agent {
        return Err(AuthorityError::WrongRun);
    }
    if !entry.groups.contains(&group) {
        return Err(AuthorityError::NotMember);
    }
    if let Some(door_cell) = table.doors.get(&group).and_then(|d| d.file_cell.as_ref()) {
        if !entry
            .file_cell
            .as_ref()
            .is_some_and(|cell| cell.same(door_cell))
        {
            return Err(AuthorityError::NotMember);
        }
    }
    if entry.file_cell.as_ref().is_some_and(HaltCell::is_engaged) || table.in_halted_group(entry) {
        return Err(AuthorityError::Halted);
    }
    if entry.view.finished {
        return Err(AuthorityError::RunNotActive);
    }
    if entry.view.cancelled || entry.cancelled.load(Ordering::SeqCst) {
        return Err(AuthorityError::RunCancelled);
    }
    Ok(())
}

/// A slot reserved at a door for a yes whose dialog is open (H1′). Filled,
/// the yes is cached if its lifetime still holds; dropped unfilled (refused,
/// declined, panicked), the slot is given back.
pub(crate) struct ConsentSlot<'a> {
    registry: &'a RunRegistry,
    group: GroupId,
    life: Life,
    held: bool,
}

impl ConsentSlot<'_> {
    /// Cache the Owner's recorded yes, in one hold of the runs lock: the
    /// slot is given back first; then a Stop, a stop since the question was
    /// asked (even one already lifted), quitting, or a door that is gone,
    /// halted or not issued refuses it, and nothing is cached. S1a-D+B:
    /// `page_facts` are the facts the broker's dialog showed with this yes,
    /// cached with it.
    pub(crate) fn fill(
        mut self,
        contract_hash: &str,
        request: &str,
        evidence_seq: u64,
        kind: ConsentKind,
        at_wall_ms: u64,
        page_facts: Option<ApprovedFacts>,
    ) -> Result<(), AuthorityError> {
        let registry = self.registry;
        let mut runs = registry
            .runs
            .lock()
            .map_err(|_| AuthorityError::Unavailable("the broker cannot read its runs"))?;
        self.held = false;
        let door = runs.doors.get_mut(&self.group);
        if let Some(door) = door {
            door.pending_consents = door.pending_consents.saturating_sub(1);
        }
        if registry.stopped.load(Ordering::SeqCst) {
            return Err(AuthorityError::EmergencyStopped);
        }
        if registry.closed.load(Ordering::SeqCst) {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        let door = match runs.doors.get_mut(&self.group) {
            Some(door)
                if door.halted || door.file_cell.as_ref().is_some_and(HaltCell::is_engaged) =>
            {
                return Err(AuthorityError::Halted)
            }
            Some(door) if door.active && door.alive() => door,
            _ => return Err(AuthorityError::NotMember),
        };
        if registry.stop_number.load(Ordering::SeqCst) != self.life.stop_epoch {
            return Err(AuthorityError::EmergencyStopped);
        }
        if door.consents.len() >= MAX_CONSENTS {
            return Err(AuthorityError::Capacity);
        }
        let consent = Consent {
            contract_hash: contract_hash.to_string(),
            request: request.to_string(),
            evidence_seq,
            kind,
            at_wall_ms,
            life: self.life,
            page_facts,
        };
        super::file_session::admit_host(door.file_cell.as_ref())?;
        door.consents.push(consent);
        Ok(())
    }
}

impl Drop for ConsentSlot<'_> {
    /// Never under the runs lock of this thread (a slot is held across the
    /// Owner's dialog, with no lock); a poisoned lock keeps the slot.
    fn drop(&mut self) {
        if self.held {
            if let Ok(mut runs) = self.registry.runs.lock() {
                if let Some(door) = runs.doors.get_mut(&self.group) {
                    door.pending_consents = door.pending_consents.saturating_sub(1);
                }
            }
        }
    }
}

/// Cancel one run that is still active, as `cancel` does, and push its
/// `run_cancelled` record (M2c PR-B: the record of the call that made the
/// transition). Under the runs lock.
fn cancel_one(entry: &mut RunEntry, wall_ms: u64, generation: u64, batch: &mut RecordBatch) {
    let was_active = !entry.view.cancelled && !entry.view.finished;
    entry.view.cancelled = true;
    entry.cancelled.store(true, Ordering::SeqCst);
    if was_active {
        let mut record = EvidenceRecord::new(
            EvidencePhase::RunCancelled,
            wall_ms,
            generation,
            entry.privacy,
        );
        record.run = Some(entry.view.id.to_string());
        record.agent = Some(entry.view.agent.to_string());
        record.cancelled = true;
        batch.push(record);
    }
}

// Decision 6: observation of real backend state, never an adapter map.
#[cfg(test)]
impl RunRegistry {
    pub(crate) fn review2_counts(&self) -> [usize; 4] {
        let table = self.runs.lock().unwrap();
        [
            table.runs.len(),
            table
                .runs
                .values()
                .filter(|r| r.files_admission.is_some())
                .count(),
            table
                .runs
                .values()
                .filter(|r| r.inference_admission.is_some())
                .count(),
            table.runs.values().map(|r| r.model_attempts as usize).sum(),
        ]
    }
}
