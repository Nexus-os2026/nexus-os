//! The adapter: the one module of Mother that touches the host.
//!
//! [`GovernedPort`] is the EffectPort over the governed-control broker:
//! each mission acts as the agent `mother` in an agent run of its own, under
//! the Owner's standing grants (an R2 action waits for the Owner's native
//! approval). It never opens an owner run, authorizes, executes, grants,
//! revokes or resumes. The broker has no file or spend route, so those
//! effects are refused here.
//!
//! M2a: an R2 action is left waiting at the broker; the port remembers it
//! and reports what the broker's record of it says (the Owner approves and
//! runs it from the interface; Mother never does). The Owner's mission
//! consent is asked at the broker's authority (`broker`), by the
//! coordinator.
//!
//! [`FileJournal`] keeps the mission journal in a file in Mother's home
//! (Mother's own state, not an effect on the world). [`Home`] builds both
//! from a directory the host chooses.

use crate::effect::{
    ApprovalStatus, EffectOutcome, EffectPort, EffectRefusal, EffectRequest, PendingAction,
};
use crate::journal::{JournalError, JournalStore, MissionId};
use nexus_governed_control::authority::clock::{Clock, SystemClock};
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use nexus_governed_control::authority::run::{RunClass, RunOrigin};
use nexus_governed_control::authority::Authority;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::{AgentOutcome, GovernedControl, Intent};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod expiry_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod m2as_adopt;
#[cfg(test)]
mod m2as_fence;
#[cfg(test)]
mod m2as_gate;
#[cfg(test)]
mod m2as_gate_review;
#[cfg(test)]
mod m2as_inflight;
#[cfg(test)]
mod m2as_model;
#[cfg(test)]
mod m2as_mutants;
#[cfg(test)]
mod m2as_nowait;
#[cfg(test)]
mod m2as_poison;
#[cfg(test)]
mod m2as_review2;
#[cfg(test)]
mod m2as_scan;
#[cfg(test)]
mod m2as_support;
#[cfg(test)]
mod real_broker;
#[cfg(test)]
mod scan;

/// The agent name Mother acts under in the broker.
pub const MOTHER_AGENT: &str = "mother";

/// Mother's broker identity.
pub fn mother_agent() -> AgentId {
    AgentId::new(MOTHER_AGENT).expect("a valid agent name")
}

#[derive(Default)]
struct Runs {
    open: BTreeMap<MissionId, RunId>,
    ended: BTreeSet<MissionId>,
    /// The action each mission waits on at the broker, and the port's
    /// monotonic deadline for it.
    waiting: BTreeMap<MissionId, (CommitmentId, u64)>,
    /// M2a-S: the coordinator broke. Every run registered then was
    /// cancelled at the broker; a run opened later is cancelled as it
    /// registers, and no broker call starts.
    fenced: bool,
    /// M2a-S: every journaled action handed back for each mission, taken or
    /// not, so that a withdrawal is an error while the broker still holds
    /// open any of them this port does not hold.
    handed: BTreeMap<MissionId, Vec<PendingAction>>,
}

/// The EffectPort over the governed-control broker.
pub struct GovernedPort {
    control: Arc<GovernedControl>,
    /// M2a-S: `fenced` and every run registration are under this mutex, so
    /// a run registers either before the fence (which cancels it) or after
    /// (and cancels itself); every broker call checks `fenced` first.
    runs: Mutex<Runs>,
    performed: AtomicUsize,
    cancel_failures: AtomicUsize,
    /// M2a-S (tests): run just before an effect is performed, and before
    /// the broker is asked about an approval (deterministic interleavings).
    #[cfg(test)]
    pub(crate) before_perform: crate::oracles::m2as_hooks::HookSlot,
    #[cfg(test)]
    pub(crate) before_approval: crate::oracles::m2as_hooks::HookSlot,
    /// M2a-S (tests): run once the effect's run is known, just before the
    /// broker is asked to act on it.
    #[cfg(test)]
    pub(crate) before_broker_action: crate::oracles::m2as_hooks::HookSlot,
    /// M2a-S (tests): run in run_for once the fence was checked, just before
    /// the broker is asked to open a run.
    #[cfg(test)]
    pub(crate) before_open_run: crate::oracles::m2as_hooks::HookSlot,
    /// M2a-S (tests): run in the fence after each run it cancels (under the
    /// coordinator lock and the port's mutex), and once it is done (under
    /// the coordinator lock only): a hook here must not call the coordinator
    /// or this port.
    #[cfg(test)]
    pub(crate) fence_between_cancels: crate::oracles::m2as_hooks::HookSlot,
    #[cfg(test)]
    pub(crate) after_fence: crate::oracles::m2as_hooks::HookSlot,
    /// M2a-S (tests): run in perform once the fence was checked, as the
    /// broker is asked to act (a stand-in for a broker call that stalls
    /// where no cancellation reaches it, such as a DNS lookup).
    #[cfg(test)]
    pub(crate) in_broker_action: crate::oracles::m2as_hooks::HookSlot,
    /// M2a-S (tests): run by the fence at its first cancellation, given
    /// whether the port was fenced at that moment.
    #[cfg(test)]
    pub(crate) first_fence_cancel: crate::oracles::m2as_hooks::SeenSlot,
}

impl GovernedPort {
    pub fn new(control: Arc<GovernedControl>) -> Self {
        Self {
            control,
            runs: Mutex::new(Runs::default()),
            performed: AtomicUsize::new(0),
            cancel_failures: AtomicUsize::new(0),
            #[cfg(test)]
            before_perform: Default::default(),
            #[cfg(test)]
            before_approval: Default::default(),
            #[cfg(test)]
            before_broker_action: Default::default(),
            #[cfg(test)]
            before_open_run: Default::default(),
            #[cfg(test)]
            fence_between_cancels: Default::default(),
            #[cfg(test)]
            after_fence: Default::default(),
            #[cfg(test)]
            in_broker_action: Default::default(),
            #[cfg(test)]
            first_fence_cancel: Default::default(),
        }
    }

    /// How many effects Mother has handed to this port.
    pub fn performed(&self) -> usize {
        self.performed.load(Ordering::SeqCst)
    }

    /// How many run cancellations the broker refused.
    pub fn cancel_failures(&self) -> usize {
        self.cancel_failures.load(Ordering::SeqCst)
    }

    /// The broker run a mission opened, if any.
    pub fn run_of(&self, mission: MissionId) -> Option<RunId> {
        self.lock().open.get(&mission).copied()
    }

    pub fn control(&self) -> &Arc<GovernedControl> {
        &self.control
    }

    /// M2a-S (tests): whether the port was fenced.
    #[cfg(test)]
    pub(crate) fn fenced(&self) -> bool {
        self.lock().fenced
    }

    /// The port's mutex. A panic while it was held poisons it; the only
    /// critical section that calls out is the fence's (it cancels runs at
    /// the broker), which sets `fenced` first. A poisoned mutex is taken
    /// back and its port treated as fenced, so every later call refuses
    /// instead of panicking.
    fn lock(&self) -> std::sync::MutexGuard<'_, Runs> {
        self.runs.lock().unwrap_or_else(|poisoned| {
            let mut runs = poisoned.into_inner();
            runs.fenced = true;
            runs
        })
    }

    /// The check before every broker call: refused once fenced. It waits
    /// only while the fence's critical section runs (the broker writing its
    /// cancellations), and the fence never waits for it. A call that passed
    /// it just before the break goes on unwaited: on a run the fence
    /// cancels (the broker refuses it, or an executing action sees its
    /// run's cancellation token), or on a run it opens afterwards, which
    /// its registration cancels at once (`run_for`).
    fn enter(&self) -> Result<(), EffectRefusal> {
        if self.lock().fenced {
            return Err(refused("port_fenced"));
        }
        Ok(())
    }

    /// The mission's agent run, opened on its first effect. Its registration
    /// checks `fenced` in the same critical section as the fence's: a run
    /// opened after the fence (its call checked just before the break) is
    /// cancelled at once and never given to the mission. A mission that
    /// ended meanwhile gets no run either (the one just opened is
    /// cancelled).
    fn run_for(&self, mission: MissionId) -> Result<RunId, EffectRefusal> {
        self.enter()?;
        {
            let runs = self.lock();
            if runs.ended.contains(&mission) {
                return Err(refused("mission_cancelled"));
            }
            if let Some(run) = runs.open.get(&mission) {
                return Ok(*run);
            }
        }
        #[cfg(test)]
        self.before_open_run.fire();
        let run = self
            .control
            .open_run(mother_agent(), RunOrigin::AgentGoal)
            .map_err(|e| refused(e.class()))?;
        let refusal = {
            let mut runs = self.lock();
            if runs.fenced {
                Some("port_fenced")
            } else if runs.ended.contains(&mission) || runs.open.contains_key(&mission) {
                Some("mission_cancelled")
            } else {
                runs.open.insert(mission, run);
                None
            }
        };
        if let Some(class) = refusal {
            self.cancel_run(run);
            return Err(refused(class));
        }
        Ok(run)
    }

    /// Cancel `run` at the broker (also once fenced: a cancellation only
    /// ever stops effects).
    fn cancel_run(&self, run: RunId) {
        if self.control.cancel_run(run).is_err() {
            self.cancel_failures.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Mark the mission ended; its run, if it opened one.
    fn end(&self, mission: MissionId) -> Option<RunId> {
        let mut runs = self.lock();
        runs.ended.insert(mission);
        runs.open.get(&mission).copied()
    }
}

fn refused(class: &str) -> EffectRefusal {
    EffectRefusal {
        class: class.into(),
    }
}

impl EffectPort for GovernedPort {
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        #[cfg(test)]
        self.before_perform.fire();
        self.performed.fetch_add(1, Ordering::SeqCst);
        let intent = match effect {
            EffectRequest::Network(intent) => Intent::Request(intent.clone()),
            EffectRequest::Process(intent) => Intent::Tool(intent.clone()),
            EffectRequest::ModelTool(intent) => Intent::Connector(intent.clone()),
            // The broker has no file or spend route.
            EffectRequest::File { .. } | EffectRequest::Spend { .. } => {
                return Err(refused("no_broker_route"))
            }
        };
        let run = self.run_for(mission)?;
        #[cfg(test)]
        self.before_broker_action.fire();
        self.enter()?;
        #[cfg(test)]
        self.in_broker_action.fire();
        // Before the broker is asked: the broker's deadline for an action
        // is its monotonic clock when it prepared it plus the action's
        // lifetime, so a deadline from this reading is never later.
        let asked_at = self.control.authority().clock().monotonic_ms();
        match self.control.agent_action(&mother_agent(), run, &intent) {
            Ok(AgentOutcome::Done(output)) => Ok(EffectOutcome::Done { text: output.text }),
            Ok(AgentOutcome::AwaitingApproval(view)) => {
                let lifetime = view.expires_wall_ms.saturating_sub(view.created_wall_ms);
                let deadline = asked_at.saturating_add(lifetime);
                self.lock().waiting.insert(mission, (view.id, deadline));
                Ok(EffectOutcome::AwaitingApproval)
            }
            Err(e) => Err(refused(e.class())),
        }
    }

    fn cancel(&self, mission: MissionId) {
        let entered = self.enter();
        let run = self.end(mission);
        // Fenced: every run this port held is cancelled already.
        if let (Ok(()), Some(run)) = (entered, run) {
            self.cancel_run(run);
        }
    }

    /// Cancel the mission's run, and say whether everything journaled for
    /// it is withdrawn: an error if the broker refused the cancellation, or
    /// if the broker still holds open a request handed back for the mission
    /// on any other run than the one just cancelled (asked of the broker for
    /// each one, never read from the record's run). Fenced: an error, unless
    /// there was nothing to withdraw.
    fn withdraw(&self, mission: MissionId) -> Result<(), EffectRefusal> {
        let entered = self.enter();
        let run = self.end(mission);
        let handed = self
            .lock()
            .handed
            .get(&mission)
            .cloned()
            .unwrap_or_default();
        // Fenced: the fence cancelled every run this port held (a
        // cancellation it could not make is counted, not retried here).
        if entered.is_err() {
            return match (run, handed.is_empty()) {
                (None, true) => Ok(()),
                _ => Err(refused("port_fenced")),
            };
        };
        if let Some(run) = run {
            self.control.cancel_run(run).map_err(|e| {
                self.cancel_failures.fetch_add(1, Ordering::SeqCst);
                refused(e.class())
            })?;
        }
        for pending in handed {
            if !self.withdrawn(&pending, run) {
                return Err(refused("action_not_held"));
            }
        }
        Ok(())
    }

    fn finish(&self, mission: MissionId) {
        let entered = self.enter();
        let run = self.end(mission);
        // Fenced: the run is cancelled already.
        if let (Ok(()), Some(run)) = (entered, run) {
            self.control.finish_when_settled(run);
        }
    }

    fn hand(&self, mission: MissionId, pending: &PendingAction) {
        self.record_handed(mission, pending);
    }

    /// The coordinator broke (called under the coordinator lock). Owner
    /// decision: the fence never waits. Under the port's mutex it sets
    /// `fenced` and cancels every run registered, then returns.
    /// - A broker call that checks `fenced` from now on is refused.
    /// - A call that checked it just before is not waited for: its run is
    ///   cancelled here, so the broker refuses it, or an action executing on
    ///   it sees its run's cancellation token (a call that stalls where no
    ///   cancellation reaches it, such as a DNS lookup, is refused by the
    ///   broker when it goes on).
    /// - A run registration's critical section precedes this one (the run
    ///   is cancelled here) or follows it (it sees `fenced` and cancels the
    ///   run itself).
    ///
    /// A cancellation the broker refuses is counted (`cancel_failures`). A
    /// fence is final: a coordinator opened again after a break needs a new
    /// port.
    fn fence(&self) {
        {
            let mut runs = self.lock();
            runs.fenced = true;
            let open: Vec<RunId> = runs.open.values().copied().collect();
            #[cfg(test)]
            if !open.is_empty() {
                self.first_fence_cancel.fire(runs.fenced);
            }
            for run in open {
                self.cancel_run(run);
                #[cfg(test)]
                self.fence_between_cancels.fire();
            }
        }
        #[cfg(test)]
        self.after_fence.fire();
    }

    fn emergency_stopped(&self) -> bool {
        self.control.authority().runs().is_stopped()
    }

    fn broker(&self) -> &Authority {
        self.control.authority()
    }

    /// The broker's record of the action `mission` waits on. A port that
    /// does not know it (another process's, or one built afresh that was
    /// not handed it back) says its outcome is unknown.
    fn approval(&self, mission: MissionId) -> ApprovalStatus {
        #[cfg(test)]
        self.before_approval.fire();
        let Ok(()) = self.enter() else {
            return ApprovalStatus::Lapsed {
                class: "port_fenced".into(),
            };
        };
        let unknown = || ApprovalStatus::Lapsed {
            class: "approval_outcome_unknown".into(),
        };
        let (run, commitment, deadline) = {
            let runs = self.lock();
            match (runs.open.get(&mission), runs.waiting.get(&mission)) {
                (Some(run), Some((commitment, deadline))) => (*run, *commitment, *deadline),
                _ => return unknown(),
            }
        };
        let authority = self.control.authority();
        let Some(view) = authority.commitments().view(commitment) else {
            return unknown();
        };
        if view.run != run || view.agent != mother_agent() {
            return unknown();
        }
        let clock = authority.clock();
        let status = status_of(&view, clock.wall_ms(), clock.monotonic_ms(), deadline);
        if status != ApprovalStatus::Waiting {
            self.lock().waiting.remove(&mission);
        }
        status
    }

    /// Final at the broker, or not held there at all. An action that is
    /// not Mother's, or that runs (approved, executing), is not settled.
    fn settled(&self, pending: &PendingAction) -> bool {
        // Fenced: not confirmed (the action stays open in the journal and is
        // withdrawn again on the next open).
        let Ok(()) = self.enter() else {
            return false;
        };
        self.settled_inside(pending)
    }

    fn pending(&self, mission: MissionId) -> Option<PendingAction> {
        let runs = self.lock();
        let run = runs.open.get(&mission)?;
        let (commitment, deadline) = runs.waiting.get(&mission)?;
        Some(PendingAction {
            run: run.to_string(),
            commitment: commitment.to_string(),
            deadline_mono_ms: *deadline,
        })
    }

    /// Take back a journaled action only if the broker's records agree: a
    /// live agent run of Mother's that no other mission of this port holds,
    /// and a commitment of that run of Mother's. The carried deadline is
    /// trusted no further than the action's remaining lifetime by its
    /// wall-clock expiry (`adopted_deadline`). A mission this port already
    /// knows, or has ended, is left as it is; a fenced port takes nothing.
    fn adopt(&self, mission: MissionId, pending: &PendingAction) {
        self.record_handed(mission, pending);
        let Ok(()) = self.enter() else {
            return;
        };
        self.adopt_inside(mission, pending);
    }
}

impl GovernedPort {
    /// Every journaled action handed back for `mission`, once each.
    fn record_handed(&self, mission: MissionId, pending: &PendingAction) {
        let mut runs = self.lock();
        let handed = runs.handed.entry(mission).or_default();
        if !handed.contains(pending) {
            handed.push(pending.clone());
        }
    }

    /// Whether the broker no longer holds `pending` open, or holds it on
    /// `cancelled` (the run this withdrawal just cancelled): the broker's own
    /// record of the commitment, never the record's run.
    fn withdrawn(&self, pending: &PendingAction, cancelled: Option<RunId>) -> bool {
        let Some(commitment) = CommitmentId::parse(&pending.commitment) else {
            return false;
        };
        match self.control.authority().commitments().view(commitment) {
            None => true,
            Some(view) => {
                cancelled == Some(view.run)
                    || !matches!(
                        view.state,
                        CommitmentState::Prepared
                            | CommitmentState::Authorized
                            | CommitmentState::Executing
                    )
            }
        }
    }

    /// [`EffectPort::settled`], once the fence was checked.
    fn settled_inside(&self, pending: &PendingAction) -> bool {
        let Some(commitment) = CommitmentId::parse(&pending.commitment) else {
            return false;
        };
        match self.control.authority().commitments().view(commitment) {
            None => true,
            Some(view) => {
                RunId::parse(&pending.run) == Some(view.run)
                    && view.agent == mother_agent()
                    && !matches!(
                        view.state,
                        CommitmentState::Prepared
                            | CommitmentState::Authorized
                            | CommitmentState::Executing
                    )
            }
        }
    }

    /// [`EffectPort::adopt`], once the fence was checked; its registration
    /// checks `fenced` again, in the same critical section.
    fn adopt_inside(&self, mission: MissionId, pending: &PendingAction) {
        let (Some(run), Some(commitment)) = (
            RunId::parse(&pending.run),
            CommitmentId::parse(&pending.commitment),
        ) else {
            return;
        };
        let authority = self.control.authority();
        let live_run =
            authority.runs().view(run).is_some_and(|view| {
                view.agent == mother_agent() && !view.cancelled && !view.finished
            }) && authority.runs().class(run) == Some(RunClass::Agent);
        let Some(view) = authority
            .commitments()
            .view(commitment)
            .filter(|view| live_run && view.run == run && view.agent == mother_agent())
        else {
            return;
        };
        let clock = authority.clock();
        let deadline = adopted_deadline(
            pending.deadline_mono_ms,
            clock.monotonic_ms(),
            clock.wall_ms(),
            view.expires_wall_ms,
        );
        let mut runs = self.lock();
        if runs.fenced
            || runs.ended.contains(&mission)
            || runs.open.contains_key(&mission)
            || runs.open.values().any(|held| *held == run)
        {
            return;
        }
        runs.open.insert(mission, run);
        runs.waiting.insert(mission, (commitment, deadline));
    }
}

/// The deadline a port takes for a handed-back action: what was journaled,
/// but no later than the action's remaining lifetime by the broker's
/// wall-clock expiry. The broker's own monotonic deadline is not in its
/// public view, so this bound relies on the wall clock: set back since the
/// action was created, it is looser (the journaled value, written by the
/// port that asked, still never exceeds the broker's).
pub(crate) fn adopted_deadline(
    journaled: u64,
    now_mono_ms: u64,
    now_wall_ms: u64,
    expires_wall_ms: u64,
) -> u64 {
    journaled.min(now_mono_ms.saturating_add(expires_wall_ms.saturating_sub(now_wall_ms)))
}

/// What the broker's record of a waiting action says. The broker ends an
/// expired action only when it next touches it; past either deadline (its
/// wall-clock expiry, or the port's monotonic deadline, which a wall clock
/// set back does not delay) it can no longer be approved or started, so it
/// is expired. An approved action that is running is waited for.
pub(crate) fn status_of(
    view: &CommitmentView,
    now_wall_ms: u64,
    now_mono_ms: u64,
    deadline_mono_ms: u64,
) -> ApprovalStatus {
    let expired = now_wall_ms >= view.expires_wall_ms || now_mono_ms >= deadline_mono_ms;
    match view.state {
        CommitmentState::Prepared | CommitmentState::Authorized if expired => {
            ApprovalStatus::Expired
        }
        CommitmentState::Prepared | CommitmentState::Authorized | CommitmentState::Executing => {
            ApprovalStatus::Waiting
        }
        CommitmentState::Succeeded => ApprovalStatus::Approved,
        CommitmentState::Denied => ApprovalStatus::Denied,
        CommitmentState::Expired => ApprovalStatus::Expired,
        CommitmentState::Failed | CommitmentState::Cancelled | CommitmentState::Revoked => {
            ApprovalStatus::Lapsed {
                class: view.state.as_str().into(),
            }
        }
    }
}

/// The journal in a file: one entry per line, appended and synced.
pub struct FileJournal {
    path: PathBuf,
}

fn store_error(e: std::io::Error) -> JournalError {
    JournalError::Store(e.kind().to_string())
}

impl FileJournal {
    /// The journal at `path` (created empty if absent). `path` must be a
    /// regular file, not a link.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.file_type().is_file() => {
                return Err(JournalError::Store(
                    "the journal is not a regular file".into(),
                ))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                OpenOptions::new()
                    .create_new(true)
                    .append(true)
                    .open(path)
                    .map_err(store_error)?;
            }
            Err(e) => return Err(store_error(e)),
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The marker that poisons the journal: beside it, `<name>.poisoned`.
    fn marker(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".poisoned");
        self.path.with_file_name(name)
    }
}

impl JournalStore for FileJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        match std::fs::symlink_metadata(self.marker()) {
            Ok(_) => return Err(JournalError::Poisoned),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(store_error(e)),
        }
        let text = std::fs::read_to_string(&self.path).map_err(store_error)?;
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        // Every line ends in a newline: the last piece is empty, or torn.
        if !text.ends_with('\n') {
            return Err(JournalError::Malformed { line: lines.len() });
        }
        Ok(lines[..lines.len() - 1].to_vec())
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if line.contains('\n') {
            return Err(JournalError::Store("a journal line holds a newline".into()));
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(store_error)?;
        file.write_all(format!("{line}\n").as_bytes())
            .map_err(store_error)?;
        file.sync_data().map_err(store_error)
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.marker())
            .map_err(store_error)?;
        file.sync_all().map_err(store_error)?;
        // The marker's directory entry must be durable too.
        #[cfg(unix)]
        if let Some(dir) = self.path.parent() {
            std::fs::File::open(dir)
                .and_then(|d| d.sync_all())
                .map_err(store_error)?;
        }
        Ok(())
    }
}

/// Create `dir` and any missing directory above it private to this user
/// (0700): the broker refuses a root below a directory others can change.
#[cfg(unix)]
fn create_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Mother's home: the broker built as the desktop builds it, and the
/// journal file. One per process: the broker claims the process's
/// emergency-stop controller.
pub struct Home {
    pub dir: PathBuf,
    pub control: Arc<GovernedControl>,
}

/// Why a home cannot be opened.
#[derive(Debug)]
pub struct HomeError(pub String);

impl Home {
    /// Open (or create) the home at `dir`, an absolute path the host
    /// chose.
    pub fn open(dir: &Path) -> Result<Self, HomeError> {
        if !dir.is_absolute() {
            return Err(HomeError("the home must be an absolute path".into()));
        }
        create_private(dir).map_err(|e| HomeError(e.to_string()))?;
        let broker = dir.join("broker");
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256))],
        )
        .map_err(|e| HomeError(format!("evidence store: {}", e.as_str())))?;
        let control = GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence,
            Arc::new(SystemClock::default()),
        )
        .map_err(|e| HomeError(format!("broker: {e:?}")))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            control: Arc::new(control),
        })
    }

    /// A new home under the system's temporary directory (examples and
    /// tests): refused if it already exists.
    pub fn fresh_temp(label: &str) -> Result<Self, HomeError> {
        let wall = SystemClock::default().wall_ms();
        let dir = std::env::temp_dir().join(format!(
            "nexus-mother-{label}-{}-{wall}",
            std::process::id()
        ));
        if dir.exists() {
            return Err(HomeError("the home already exists".into()));
        }
        Self::open(&dir)
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }

    pub fn journal(&self) -> Result<FileJournal, JournalError> {
        FileJournal::open(&self.journal_path())
    }
}
