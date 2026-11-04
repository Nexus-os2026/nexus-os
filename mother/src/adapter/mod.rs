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
//! runs it from the interface; Mother never does).
//!
//! M2c: the port holds only an [`AgentDoor`] the host had issued (the Owner
//! said yes to it). The door fixes Mother's agent and the host's confirmer,
//! and acts only on the runs it opened or adopted. The Owner's consent to
//! a mission and resume of a held one are asked through it, by the
//! coordinator.
//!
//! [`FileJournal`] keeps the mission journal in a file in Mother's home
//! (Mother's own state, not an effect on the world). M2c: Mother builds no
//! broker; the host does (`nexus-mother-host`'s `Home`), issues the door
//! with the Owner's yes, and hands the port only that door.
//!
//! P0: [`FilePrivate`] keeps Mother's private store in her home. Every class
//! the port returns is one of the closed registry
//! ([`crate::evidence::BrokerClass`]).

use crate::effect::{
    Admission, ApprovalStatus, EffectOutcome, EffectPort, EffectRefusal, EffectRequest,
    FileSnapshot, Owed, PendingAction,
};
use crate::evidence::BrokerClass;
use crate::journal::{IoKind, JournalError, JournalStore, MissionId, StoreFailure};
use nexus_governed_control::authority::approval::{
    MissionApproval, MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
};
use nexus_governed_control::authority::commitment::PublicationClass;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::file_session::{
    FileAttachment, FilePort, FileSessionError, HaltCell, HaltTag,
};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use nexus_governed_control::authority::AuthorityError;
use nexus_governed_control::governed::{
    AgentDoor, AgentOutcome, HaltReason, Intent, Lookup, RecordBatch,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
mod lite_port;

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
mod m2as_sweep;
#[cfg(test)]
mod m2c_crash;
#[cfg(test)]
mod m2c_door;
#[cfg(test)]
mod m2c_halt;
#[cfg(test)]
pub(crate) mod m2c_support;
mod private_file;
pub use private_file::FilePrivate;

#[cfg(test)]
mod p0_api;
#[cfg(test)]
mod p0_chain;
#[cfg(test)]
mod p0_chain_io;
#[cfg(test)]
mod p0_dirsync;
#[cfg(test)]
mod p0_privacy;
#[cfg(test)]
mod p0_privdir;
#[cfg(test)]
pub(crate) mod p0_support;
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
    /// The exact File effect owning the current proposal/wait slot.
    file_effects: BTreeMap<MissionId, u64>,
    /// M2a-S: the coordinator broke. Every run registered then was
    /// cancelled at the broker; a run opened later is cancelled as it
    /// registers, and no broker call starts.
    fenced: bool,
    /// M2a-S: every journaled action handed back for each mission, taken or
    /// not, so that a withdrawal is an error while the broker still holds
    /// open any of them this port does not hold.
    handed: BTreeMap<MissionId, Vec<PendingAction>>,
    /// M2a-S: the runs the fence has still to cancel, last first. A sweep a
    /// panic cut short is completed when the poisoned mutex is taken back.
    unswept: Vec<RunId>,
    file_admissions: BTreeMap<MissionId, Admission>,
}

/// An optional host restriction shared with the actual coordinator port.
/// Closing is memory-only and irreversible. This can only deny authority;
/// it neither replaces nor lifts any broker or File-session halt.
#[derive(Default)]
pub struct AdmissionGate {
    closed: AtomicBool,
}
impl AdmissionGate {
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

/// The EffectPort over the governed-control broker, through one agent door.
pub struct GovernedPort {
    #[cfg(test)]
    pub(crate) file_duplicate_claim: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    pub(crate) file_engagement: crate::oracles::lite_engagement_hooks::SiteHook,
    /// M2c: the port's only broker handle.
    door: AgentDoor,
    file_port: Result<FilePort, FileSessionError>,
    file_session: OnceLock<(Arc<FileAttachment>, HaltCell)>,
    workspace: Option<String>,
    admission_gate: Option<Arc<AdmissionGate>>,
    /// M2a-S: `fenced` and every run registration are under this mutex, so
    /// a run registers either before the fence (which cancels it) or after
    /// (and cancels itself); every broker call checks `fenced` first.
    runs: Mutex<Runs>,
    performed: AtomicUsize,
    cancel_failures: AtomicUsize,
    #[cfg(test)]
    file_acks_tried: AtomicUsize,
    #[cfg(test)]
    fail_file_ack_once: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    pub(crate) before_file_ack: crate::oracles::m2as_hooks::HookSlot,
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
    /// M2c (tests): run once the door answered a held-mission request,
    /// before the coordinator takes the answer.
    #[cfg(test)]
    pub(crate) after_held_answer: crate::oracles::m2as_hooks::HookSlot,
}

impl GovernedPort {
    /// A port over `door`, which the host issued after the Owner's yes.
    pub fn new(door: AgentDoor) -> Self {
        let file_port = door.file_port();
        Self {
            file_port,
            #[cfg(test)]
            file_duplicate_claim: Default::default(),
            #[cfg(test)]
            file_engagement: Default::default(),
            file_session: OnceLock::new(),
            workspace: None,
            admission_gate: None,
            door,
            runs: Mutex::new(Runs::default()),
            performed: AtomicUsize::new(0),
            cancel_failures: AtomicUsize::new(0),
            #[cfg(test)]
            file_acks_tried: AtomicUsize::new(0),
            #[cfg(test)]
            fail_file_ack_once: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            before_file_ack: Default::default(),
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
            #[cfg(test)]
            after_held_answer: Default::default(),
        }
    }

    /// Install a narrowing gate before sharing this port with a coordinator.
    /// There is no setter on a shared port and no way to reopen the gate.
    /// A host-gated port never adopts replayed work from a surviving broker.
    pub fn with_admission_gate(mut self, gate: Arc<AdmissionGate>) -> Self {
        self.admission_gate = Some(gate);
        self
    }

    fn admit_host(&self) -> Result<(), EffectRefusal> {
        if self
            .admission_gate
            .as_ref()
            .is_some_and(|gate| gate.is_closed())
        {
            return Err(refused(BrokerClass::Halted));
        }
        Ok(())
    }

    /// A requested registered workspace label, never workspace authority.
    pub fn with_workspace(mut self, name: String) -> Self {
        self.workspace = Some(name);
        self
    }

    #[cfg(test)]
    pub(crate) fn acks_tried(&self) -> usize {
        self.file_acks_tried.load(Ordering::SeqCst)
    }

    /// How many effects Mother has handed to this port.
    pub fn performed(&self) -> usize {
        self.performed.load(Ordering::SeqCst)
    }

    /// Successful File bindings retained by this port, for host diagnostics.
    /// Observation only: this exposes no admission or broker authority.
    pub fn file_binding_count(&self) -> usize {
        self.with_runs(|runs| runs.file_admissions.len())
    }

    /// How many run cancellations the broker refused.
    pub fn cancel_failures(&self) -> usize {
        self.cancel_failures.load(Ordering::SeqCst)
    }

    /// The broker run a mission opened, if any.
    pub fn run_of(&self, mission: MissionId) -> Option<RunId> {
        self.with_runs(|runs| runs.open.get(&mission).copied())
    }

    /// M2a-S (tests): whether the port was fenced.
    #[cfg(test)]
    pub(crate) fn fenced(&self) -> bool {
        self.with_runs(|runs| runs.fenced)
    }

    /// The port's mutex. A panic while it was held poisons it; the only
    /// critical section that calls out is the fence's (it halts the door
    /// and cancels runs at the broker, in memory), which sets `fenced`
    /// first. A poisoned mutex is taken back and its port treated as
    /// fenced, so every later call refuses instead of panicking; the door
    /// is halted and a sweep the panic cut short is completed (its panic
    /// was raised already). M2c PR-B: their records come back with the
    /// guard, to be written once the mutex is released.
    fn locked(&self) -> (std::sync::MutexGuard<'_, Runs>, Owed) {
        match self.runs.lock() {
            Ok(runs) => (runs, Owed::default()),
            Err(poisoned) => {
                let mut runs = poisoned.into_inner();
                runs.fenced = true;
                let mut owed = Owed::default();
                if let Ok(batch) = catch_unwind(AssertUnwindSafe(|| {
                    self.door.halt(HaltReason::JournalFailed)
                })) {
                    owed.push(batch);
                }
                let _ = self.sweep(&mut runs, &mut owed);
                (runs, owed)
            }
        }
    }

    /// M2c PR-B: run `body` under the port's mutex; records its poison
    /// recovery made are written once the mutex is released.
    fn with_runs<T>(&self, body: impl FnOnce(&mut Runs) -> T) -> T {
        let (result, owed) = {
            let (mut runs, owed) = self.locked();
            #[cfg(test)]
            let _held = crate::effect::held::Held::port();
            (body(&mut runs), owed)
        };
        owed.write();
        result
    }

    /// Cancel every run the fence has still to cancel, each on its own,
    /// their records into `owed`: a panic while cancelling one is counted
    /// as a failed cancellation and does not skip the others. Returns the
    /// first panic, once all are done.
    fn sweep(&self, runs: &mut Runs, owed: &mut Owed) -> Option<Box<dyn std::any::Any + Send>> {
        let mut first = None;
        while let Some(run) = runs.unswept.pop() {
            let cancelled = catch_unwind(AssertUnwindSafe(|| {
                match self.door.cancel_run(run) {
                    Ok(batch) => owed.push(batch),
                    Err(_) => {
                        self.cancel_failures.fetch_add(1, Ordering::SeqCst);
                    }
                }
                #[cfg(test)]
                self.fence_between_cancels.fire();
            }));
            if let Err(panic) = cancelled {
                self.cancel_failures.fetch_add(1, Ordering::SeqCst);
                first.get_or_insert(panic);
            }
        }
        first
    }

    /// The check before every broker call: refused once fenced. It waits
    /// only while the fence's critical section runs (M2c: the door's halt
    /// and the sweep, in memory, with no evidence write), and the fence
    /// never waits for it. A call that passed
    /// it just before the break goes on unwaited: on a run the fence
    /// cancels (the broker refuses it, or an executing action sees its
    /// run's cancellation token), or on a run it opens afterwards, which
    /// its registration cancels at once (`run_for`).
    fn enter(&self) -> Result<(), EffectRefusal> {
        if self.file_session.get().is_some() {
            self.file_enter()?;
        }
        if self.with_runs(|runs| runs.fenced) {
            return Err(refused(BrokerClass::PortFenced));
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
        self.admit_host()?;
        self.enter()?;
        let known = self.with_runs(|runs| {
            if runs.ended.contains(&mission) {
                return Some(Err(refused(BrokerClass::MissionCancelled)));
            }
            runs.open.get(&mission).map(|run| Ok(*run))
        });
        if let Some(known) = known {
            return known;
        }
        #[cfg(test)]
        self.before_open_run.fire();
        self.admit_host()?;
        let run = match self.file_session.get() {
            Some((attachment, _)) => self.door.open_file_run(attachment),
            None => self.door.open_run(),
        }
        .map_err(|e| refused_by(e.class()))?;
        let refusal = self.with_runs(|runs| {
            if runs.fenced {
                Some(BrokerClass::PortFenced)
            } else if runs.ended.contains(&mission) || runs.open.contains_key(&mission) {
                Some(BrokerClass::MissionCancelled)
            } else {
                runs.open.insert(mission, run);
                None
            }
        });
        if let Some(class) = refusal {
            self.cancel_run(run);
            return Err(refused(class));
        }
        Ok(run)
    }

    /// Cancel `run` at the broker (also once fenced: a cancellation only
    /// ever stops effects); its records are written here, with no lock held.
    fn cancel_run(&self, run: RunId) {
        match self.door.cancel_run(run) {
            Ok(batch) => written(batch),
            Err(_) => {
                self.cancel_failures.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    /// Mark the mission ended; its run, if it opened one.
    fn end(&self, mission: MissionId) -> Option<RunId> {
        self.with_runs(|runs| {
            runs.ended.insert(mission);
            runs.open.get(&mission).copied()
        })
    }
}

/// Write a batch the port's door returned, with no lock held (tests check
/// it on every write).
fn written(batch: RecordBatch) {
    let mut owed = Owed::default();
    owed.push(batch);
    owed.write();
}

fn refused(class: BrokerClass) -> EffectRefusal {
    EffectRefusal { class }
}

/// The broker's class, in the closed registry (anything else is `unknown`).
fn refused_by(class: &str) -> EffectRefusal {
    refused(BrokerClass::of(class))
}

impl GovernedPort {
    fn perform_inner(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        self.admit_host()?;
        if self.file_session.get().is_some() {
            self.file_enter()?;
            let runs = self.file_runs()?;
            if !runs.file_admissions.contains_key(&mission) {
                return Err(refused(BrokerClass::NotAdmitted));
            }
        }
        self.performed.fetch_add(1, Ordering::SeqCst);
        let intent = match effect {
            EffectRequest::CreateFile(file) if self.file_session.get().is_some() => {
                Intent::File(file.clone())
            }
            EffectRequest::CreateFile(_) => return Err(refused(BrokerClass::NoBrokerRoute)),
            EffectRequest::Network(intent) => Intent::Request(intent.clone()),
            EffectRequest::Process(intent) => Intent::Tool(intent.clone()),
            EffectRequest::ModelTool(intent) => Intent::Connector(intent.clone()),
            // The broker has no file or spend route.
            EffectRequest::File { .. } | EffectRequest::Spend { .. } => {
                return Err(refused(BrokerClass::NoBrokerRoute))
            }
        };
        let run = self.run_for(mission)?;
        #[cfg(test)]
        self.before_broker_action.fire();
        self.enter()?;
        #[cfg(test)]
        self.in_broker_action.fire();
        self.admit_host()?;
        // Before the broker is asked: the broker's deadline for an action
        // is its monotonic clock when it prepared it plus the action's
        // lifetime, so a deadline from this reading is never later.
        let asked_at = self.door.clock().monotonic_ms();
        match self.door.agent_action(run, &intent) {
            Ok(AgentOutcome::Done(output)) => Ok(EffectOutcome::Done { text: output.text }),
            Ok(AgentOutcome::AwaitingApproval(view)) => {
                let lifetime = view.expires_wall_ms.saturating_sub(view.created_wall_ms);
                let deadline = asked_at.saturating_add(lifetime);
                self.with_runs(|runs| runs.waiting.insert(mission, (view.id, deadline)));
                Ok(EffectOutcome::AwaitingApproval)
            }
            Err(e) => Err(refused_by(e.class())),
        }
    }
}

impl EffectPort for GovernedPort {
    fn file_workspace(&self) -> Option<&str> {
        self.workspace.as_deref()
    }
    fn file_attach(&self) -> Result<Option<Arc<FileAttachment>>, FileSessionError> {
        self.admit_host()
            .map_err(|_| FileSessionError::EmergencyHistory)?;
        Ok(Some(Arc::new(
            self.file_port.as_ref().map_err(|e| *e)?.attach()?,
        )))
    }
    fn file_claim(&self, attachment: Arc<FileAttachment>) -> Result<HaltCell, FileSessionError> {
        self.admit_host()
            .map_err(|_| FileSessionError::EmergencyHistory)?;
        let cell = self.door.claim_file_session(&attachment)?;
        #[cfg(test)]
        if self.file_duplicate_claim.load(Ordering::SeqCst) {
            assert!(self
                .file_session
                .set((attachment.clone(), cell.clone()))
                .is_ok());
        }
        if self.file_session.set((attachment, cell.clone())).is_err() {
            cell.engage_tagged(HaltTag::DoorHalted);
            #[cfg(test)]
            self.file_engagement.fire("port-claim");
            return Err(FileSessionError::AlreadySelected);
        }
        Ok(cell)
    }
    fn file_admit(&self, mission: MissionId, admission: &Admission) -> Result<(), EffectRefusal> {
        self.admit_lite(mission, admission)
    }
    fn file_publication(&self, mission: MissionId) -> Result<Option<FileSnapshot>, EffectRefusal> {
        let effect = self.file_runs()?.file_effects.get(&mission).copied();
        match effect {
            Some(effect) => self.publication_lite(mission, effect),
            None => Ok(None),
        }
    }
    fn file_publication_for(
        &self,
        mission: MissionId,
        effect: u64,
    ) -> Result<Option<FileSnapshot>, EffectRefusal> {
        self.publication_lite(mission, effect)
    }
    fn perform_file(
        &self,
        mission: MissionId,
        number: u64,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        #[cfg(test)]
        self.before_perform.fire();
        if !matches!(effect, EffectRequest::CreateFile(_)) {
            return Err(refused(BrokerClass::NotAdmitted));
        }
        self.file_enter()?;
        {
            let mut runs = self.file_runs()?;
            let (_, cell) = self
                .file_session
                .get()
                .ok_or_else(|| refused(BrokerClass::NotAdmitted))?;
            if cell.is_engaged() || runs.ended.contains(&mission) {
                return Err(refused(BrokerClass::Halted));
            }
            if runs
                .file_effects
                .get(&mission)
                .is_some_and(|prior| *prior >= number)
            {
                return Err(refused(BrokerClass::ActionNotHeld));
            }
            // Reserve before entering the broker. A read during proposal is
            // uncertain, never proof that the effect was not published.
            runs.file_effects.insert(mission, number);
            runs.waiting.remove(&mission);
        }
        self.perform_inner(mission, effect)
    }
    fn file_deadline_elapsed(&self, pending: &PendingAction) -> bool {
        self.door.clock().monotonic_ms() >= pending.deadline_mono_ms
    }
    fn file_ack(&self, pending: &PendingAction) -> Result<(), EffectRefusal> {
        #[cfg(test)]
        self.file_acks_tried.fetch_add(1, Ordering::SeqCst);
        #[cfg(test)]
        self.before_file_ack.fire();
        #[cfg(test)]
        if self.fail_file_ack_once.swap(false, Ordering::SeqCst) {
            return Err(refused(BrokerClass::ActionNotHeld));
        }
        let id = CommitmentId::parse(&pending.commitment)
            .ok_or_else(|| refused(BrokerClass::ActionNotHeld))?;
        self.door
            .ack_commitment(id)
            .map_err(|e| refused_by(e.class()))?;
        let mut runs = self.file_runs()?;
        let mission = runs.open.iter().find_map(|(mission, run)| {
            (run.to_string() == pending.run
                && runs.waiting.get(mission) == Some(&(id, pending.deadline_mono_ms)))
            .then_some(*mission)
        });
        if let Some(mission) = mission {
            runs.waiting.remove(&mission);
        }
        Ok(())
    }

    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        #[cfg(test)]
        self.before_perform.fire();
        if self.file_session.get().is_some() && effect.kind() == crate::contract::EffectKind::File {
            return Err(refused(BrokerClass::NotAdmitted));
        }
        self.perform_inner(mission, effect)
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
        if self.file_session.get().is_some() {
            return self.withdraw_lite(mission);
        }
        let entered = self.enter();
        let run = self.end(mission);
        let handed = self.with_runs(|runs| runs.handed.get(&mission).cloned().unwrap_or_default());
        // Fenced: the fence cancelled every run this port held (a
        // cancellation it could not make is counted, not retried here).
        if entered.is_err() {
            return match (run, handed.is_empty()) {
                (None, true) => Ok(()),
                _ => Err(refused(BrokerClass::PortFenced)),
            };
        };
        if let Some(run) = run {
            let batch = self.door.cancel_run(run).map_err(|e| {
                self.cancel_failures.fetch_add(1, Ordering::SeqCst);
                refused_by(e.class())
            })?;
            written(batch);
        }
        for pending in handed {
            if !self.withdrawn(&pending, run) {
                return Err(refused(BrokerClass::ActionNotHeld));
            }
        }
        Ok(())
    }

    fn finish(&self, mission: MissionId) {
        let entered = self.enter();
        let run = self.end(mission);
        // Fenced: the run is cancelled already.
        if let (Ok(()), Some(run)) = (entered, run) {
            // The run is this door's: only a run it opened is finished.
            let _ = self.door.finish_when_settled(run);
        }
    }

    fn hand(&self, mission: MissionId, pending: &PendingAction) {
        self.record_handed(mission, pending);
    }

    /// The coordinator broke (called under the coordinator lock). Owner
    /// decision: the fence does not wait for in-flight actions; the broker's
    /// cancellation evidence writes can still block (known limit, closed by
    /// M2c). Under the port's mutex it sets `fenced` and cancels every run
    /// registered, then returns.
    /// - A broker call that checks `fenced` from now on is refused.
    /// - A call that checked it just before is not waited for: it is
    ///   interrupted where the broker can (its run is cancelled here, so the
    ///   broker refuses a step it has not begun, and an action executing on
    ///   it sees its run's cancellation token). That is the in-flight limit
    ///   of an ordinary Stop (README, known limits).
    /// - A run registration's critical section precedes this one (the run
    ///   is cancelled here) or follows it (it sees `fenced` and cancels the
    ///   run itself).
    ///
    /// A cancellation the broker refuses is counted (`cancel_failures`). A
    /// panic while cancelling one run is counted too and does not skip the
    /// others: the whole sweep is done, then the panic is resumed (under the
    /// mutex, which it poisons; the port stays fenced). A fence is final: a
    /// coordinator opened again after a break needs a new port.
    fn fence(&self) -> Owed {
        if let Some((_, cell)) = self.file_session.get() {
            cell.engage_tagged(HaltTag::DoorHalted);
            #[cfg(test)]
            self.file_engagement.fire("port-fence");
        }
        let mut owed = Owed::default();
        {
            let (mut runs, recovered) = self.locked();
            owed.extend(recovered);
            #[cfg(test)]
            let _held = crate::effect::held::Held::port();
            runs.fenced = true;
            // M2c PR-B: the door's halt first. From its first step nothing
            // of this door is admitted; once it returns none of its
            // commitments moves to Executing. A panic in it is counted as a
            // cancel failure and resumed at the end.
            let halted = catch_unwind(AssertUnwindSafe(|| {
                self.door.halt(HaltReason::JournalFailed)
            }));
            let first = match halted {
                Ok(batch) => {
                    owed.push(batch);
                    None
                }
                Err(panic) => {
                    self.cancel_failures.fetch_add(1, Ordering::SeqCst);
                    Some(panic)
                }
            };
            runs.unswept = runs.open.values().rev().copied().collect();
            #[cfg(test)]
            if !runs.unswept.is_empty() {
                self.first_fence_cancel.fire(runs.fenced);
            }
            // A backstop now: the halt cancelled every run of the door's
            // group; normally these batches are empty.
            let swept = self.sweep(&mut runs, &mut owed);
            if let Some(panic) = first.or(swept) {
                resume_unwind(panic);
            }
        }
        #[cfg(test)]
        self.after_fence.fire();
        owed
    }

    fn emergency_stopped(&self) -> bool {
        self.door.emergency_stopped()
    }

    /// Asked through the door, whose confirmer the host bound. Not fenced:
    /// it has no effect, and an answer that comes after a break is
    /// discarded by the coordinator.
    fn confirm_mission(
        &self,
        request: &MissionConfirmation,
    ) -> Result<MissionApproval, AuthorityError> {
        self.admit_host().map_err(|_| AuthorityError::Halted)?;
        let answer = self.door.confirm_mission(request)?;
        self.admit_host().map_err(|_| AuthorityError::Halted)?;
        Ok(answer)
    }

    /// Asked through the door, as [`EffectPort::confirm_mission`].
    fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        self.admit_host().map_err(|_| AuthorityError::Halted)?;
        let answer = self.door.confirm_held_mission(request);
        #[cfg(test)]
        self.after_held_answer.fire();
        self.admit_host().map_err(|_| AuthorityError::Halted)?;
        answer
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
        let found =
            self.with_runs(
                |runs| match (runs.open.get(&mission), runs.waiting.get(&mission)) {
                    (Some(run), Some((commitment, deadline))) => {
                        Some((*run, *commitment, *deadline))
                    }
                    _ => None,
                },
            );
        let Some((run, commitment, deadline)) = found else {
            return unknown();
        };
        // Only a commitment of one of this door's runs is known here.
        let Lookup::Mine(view) = self.door.commitment(commitment) else {
            return unknown();
        };
        if view.run != run || view.agent != mother_agent() {
            return unknown();
        }
        let clock = self.door.clock();
        let status = status_of(&view, clock.wall_ms(), clock.monotonic_ms(), deadline);
        if status != ApprovalStatus::Waiting {
            self.with_runs(|runs| runs.waiting.remove(&mission));
        }
        status
    }

    /// Final at the broker, or not held there at all. An action that is
    /// not Mother's, or that runs (approved, executing), is not settled.
    fn settled(&self, pending: &PendingAction) -> bool {
        if self.file_session.get().is_some() {
            return self.non_file_settled_lite(pending);
        }
        // Fenced: not confirmed (the action stays open in the journal and is
        // withdrawn again on the next open).
        let Ok(()) = self.enter() else {
            return false;
        };
        self.settled_inside(pending)
    }

    fn pending(&self, mission: MissionId) -> Option<PendingAction> {
        self.with_runs(|runs| {
            let run = runs.open.get(&mission)?;
            let (commitment, deadline) = runs.waiting.get(&mission)?;
            Some(PendingAction {
                run: run.to_string(),
                commitment: commitment.to_string(),
                deadline_mono_ms: *deadline,
            })
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
        // Hands has no surviving-broker replacement or replay adoption. Keep
        // the handed-back obligation above so withdrawal never fakes success;
        // only legacy ports may recover membership through this route.
        if self.admission_gate.is_none() {
            self.adopt_inside(mission, pending);
        }
    }
}

impl GovernedPort {
    /// Every journaled action handed back for `mission`, once each.
    fn record_handed(&self, mission: MissionId, pending: &PendingAction) {
        self.with_runs(|runs| {
            let handed = runs.handed.entry(mission).or_default();
            if !handed.contains(pending) {
                handed.push(pending.clone());
            }
        });
    }

    /// Whether the broker no longer holds `pending` open, or holds it on
    /// `cancelled` (the run this withdrawal just cancelled): the broker's own
    /// record of the commitment, never the record's run. Of a commitment
    /// that is not this door's, the door says only whether it is settled
    /// (final, and Mother's).
    fn withdrawn(&self, pending: &PendingAction, cancelled: Option<RunId>) -> bool {
        let Some(commitment) = CommitmentId::parse(&pending.commitment) else {
            return false;
        };
        match self.door.commitment(commitment) {
            Lookup::Unknown => true,
            Lookup::NotMine { settled } => settled,
            Lookup::Mine(view) => {
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

    /// [`EffectPort::settled`], once the fence was checked. Of a commitment
    /// that is not this door's, the door says only whether it is settled
    /// (final, and Mother's).
    fn settled_inside(&self, pending: &PendingAction) -> bool {
        let Some(commitment) = CommitmentId::parse(&pending.commitment) else {
            return false;
        };
        match self.door.commitment(commitment) {
            Lookup::Unknown => true,
            Lookup::NotMine { settled } => settled,
            Lookup::Mine(view) => {
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
    /// checks `fenced` again, in the same critical section. M2c: the broker
    /// validates the action (Mother's agent, a live agent run of this
    /// registry, its commitment) and the run joins this door's group first;
    /// only then does the port register it (over-covering, never under).
    fn adopt_inside(&self, mission: MissionId, pending: &PendingAction) {
        let (Some(run), Some(commitment)) = (
            RunId::parse(&pending.run),
            CommitmentId::parse(&pending.commitment),
        ) else {
            return;
        };
        let Ok(view) = self.door.adopt(run, commitment) else {
            return;
        };
        let clock = self.door.clock();
        let deadline = adopted_deadline(
            pending.deadline_mono_ms,
            clock.monotonic_ms(),
            clock.wall_ms(),
            view.expires_wall_ms,
        );
        self.with_runs(|runs| {
            if runs.fenced
                || runs.ended.contains(&mission)
                || runs.open.contains_key(&mission)
                || runs.open.values().any(|held| *held == run)
            {
                return;
            }
            runs.open.insert(mission, run);
            runs.waiting.insert(mission, (commitment, deadline));
        });
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
    JournalError::Store(StoreFailure::Io(IoKind::of(e.kind())))
}

impl FileJournal {
    /// The journal at `path` (created empty if absent). `path` must be a
    /// regular file, not a link.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.file_type().is_file() => {
                return Err(JournalError::Store(StoreFailure::NotRegularFile))
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
        // M2d-D V8: another handle may have created this entry without
        // syncing it. Establish both the file and its directory entry on
        // every open, not only when this handle was the creator. No record
        // can be acknowledged through this handle before both succeed.
        OpenOptions::new()
            .append(true)
            .open(path)
            .and_then(|file| file.sync_all())
            .map_err(store_error)?;
        #[cfg(unix)]
        {
            let parent =
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .ok_or(JournalError::Store(StoreFailure::Io(IoKind::of(
                        std::io::ErrorKind::InvalidInput,
                    ))))?;
            std::fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(store_error)?;
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
            return Err(JournalError::Store(StoreFailure::LineHasNewline));
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
/// (The host builds the broker's home with it, and so do Mother's tests.)
#[cfg(unix)]
pub fn create_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
pub fn create_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod lite;

#[cfg(target_os = "linux")]
#[cfg(test)]
pub(crate) mod lite_clock;
#[cfg(target_os = "linux")]
#[cfg(test)]
mod lite_crash;

#[cfg(target_os = "linux")]
#[cfg(test)]
mod lite_review5_port;

#[cfg(test)]
mod host_decision9_tests;
