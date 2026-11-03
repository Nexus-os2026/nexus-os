//! The adapter: the one module of Mother that touches the host.
//!
//! [`GovernedPort`] is the EffectPort over the governed-control broker:
//! each mission acts as the agent `mother` in an agent run of its own, under
//! the Owner's standing grants (an R2 action waits for the Owner's native
//! approval). It never opens an owner run, authorizes, executes, grants,
//! revokes or resumes. The broker has no spend route, so that effect is
//! refused here.
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
//!
//! M2d-C: a File effect goes to the broker's Files route, only for a mission
//! the port admitted (I7: the door binds the run to the Owner's yes) and
//! only under that admission's generation; a perform of another generation
//! is refused (`admission_stale`) and cancels the run. The host gives the
//! port the workspace's display name ([`GovernedPort::with_workspace`]). A
//! publication the broker reports as interrupted after its final check is
//! [`ApprovalStatus::Overtaken`].

use crate::effect::{
    Admission, ApprovalStatus, EffectOutcome, EffectPort, EffectRefusal, EffectRequest, Owed,
    PendingAction,
};
use crate::evidence::BrokerClass;
use crate::journal::{IoKind, JournalError, JournalStore, MissionId, StoreFailure};
use nexus_governed_control::authority::approval::{
    MissionApproval, MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
};
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::effect::CapabilityKind;
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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

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
#[cfg(test)]
mod m2d_files;
#[cfg(test)]
mod m2d_files_r1;
#[cfg(test)]
mod m2d_files_r2;
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
    /// M2d (I7): the admission generation of each admitted mission.
    admitted: BTreeMap<MissionId, u64>,
}

/// The EffectPort over the governed-control broker, through one agent door.
pub struct GovernedPort {
    /// M2c: the port's only broker handle.
    door: AgentDoor,
    /// M2a-S: `fenced` and every run registration are under this mutex, so
    /// a run registers either before the fence (which cancels it) or after
    /// (and cancels itself); every broker call checks `fenced` first.
    runs: Mutex<Runs>,
    /// M2d: the workspace's display name, as the host gave it (data).
    workspace: Option<String>,
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
    /// M2c (tests): run once the door answered a held-mission request,
    /// before the coordinator takes the answer.
    #[cfg(test)]
    pub(crate) after_held_answer: crate::oracles::m2as_hooks::HookSlot,
    /// M2d-C (tests): run once the door admitted a run, before the port
    /// keeps the admission.
    #[cfg(test)]
    pub(crate) after_admit: crate::oracles::m2as_hooks::HookSlot,
}

impl GovernedPort {
    /// A port over `door`, which the host issued after the Owner's yes.
    pub fn new(door: AgentDoor) -> Self {
        Self {
            door,
            runs: Mutex::new(Runs::default()),
            workspace: None,
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
            #[cfg(test)]
            after_held_answer: Default::default(),
            #[cfg(test)]
            after_admit: Default::default(),
        }
    }

    /// M2d: the display name of the workspace the Owner registered at the
    /// broker, as the host read it (Mother checks a File's requested folder
    /// against it; the broker binds the workspace itself).
    pub fn with_workspace(mut self, name: impl Into<String>) -> Self {
        self.workspace = Some(name.into());
        self
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
        let run = self.door.open_run().map_err(|e| refused_by(e.class()))?;
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

impl EffectPort for GovernedPort {
    fn perform(
        &self,
        mission: MissionId,
        generation: u64,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        #[cfg(test)]
        self.before_perform.fire();
        self.performed.fetch_add(1, Ordering::SeqCst);
        let intent = match effect {
            EffectRequest::Network(intent) => Intent::Request(intent.clone()),
            EffectRequest::Process(intent) => Intent::Tool(intent.clone()),
            EffectRequest::ModelTool(intent) => Intent::Connector(intent.clone()),
            EffectRequest::File(create) => Intent::File(create.clone()),
            // The broker has no spend route.
            EffectRequest::Spend { .. } => return Err(refused(BrokerClass::NoBrokerRoute)),
        };
        // I7: a File only once admitted; once admitted, every effect only
        // under that generation (another cancels the run).
        match (
            self.with_runs(|runs| runs.admitted.get(&mission).copied()),
            effect,
        ) {
            (Some(admitted), _) if admitted != generation => {
                self.cancel(mission);
                return Err(refused(BrokerClass::AdmissionStale));
            }
            (None, EffectRequest::File(_)) => return Err(refused(BrokerClass::NotAdmitted)),
            _ => {}
        }
        let run = self.run_for(mission)?;
        #[cfg(test)]
        self.before_broker_action.fire();
        self.enter()?;
        #[cfg(test)]
        self.in_broker_action.fire();
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

    /// I7: open or reuse the mission's run, have the door bind it to the
    /// Owner's yes to exactly this hash and request (the broker checks the
    /// yes, the run and the workspace), then keep the generation, only if
    /// the mission did not end and the port was not fenced meanwhile (the
    /// same critical section as theirs).
    fn admit_files(&self, mission: MissionId, admission: &Admission) -> Result<(), EffectRefusal> {
        let run = self.run_for(mission)?;
        self.door
            .admit_files(run, &admission.contract_hash, &admission.consent_request)
            .map_err(|e| refused_by(e.class()))?;
        #[cfg(test)]
        self.after_admit.fire();
        let refusal = self.with_runs(|runs| {
            if runs.fenced {
                Some(BrokerClass::PortFenced)
            } else if runs.ended.contains(&mission) || runs.open.get(&mission) != Some(&run) {
                Some(BrokerClass::MissionCancelled)
            } else {
                runs.admitted.insert(mission, admission.generation);
                None
            }
        });
        refusal.map_or(Ok(()), |class| Err(refused(class)))
    }

    fn workspace(&self) -> Option<String> {
        self.workspace.clone()
    }

    /// HIGH 1: the broker's record of the action `mission` waits on, only
    /// if it is a File create of this door's run that succeeded. Nothing
    /// is changed: the wait stays the port's until the coordinator
    /// acknowledges it. Decision C: a read the broker cannot answer (its
    /// registry poisoned: `try_commitment`), or a fenced port, is an error,
    /// never a panic.
    fn publication(&self, mission: MissionId) -> Result<Option<ApprovalStatus>, EffectRefusal> {
        self.enter()?;
        let Some((run, commitment, deadline)) = self.with_runs(|runs| {
            Some((
                *runs.open.get(&mission)?,
                runs.waiting.get(&mission)?.0,
                runs.waiting.get(&mission)?.1,
            ))
        }) else {
            return Ok(None);
        };
        let lookup = self
            .door
            .try_commitment(commitment)
            .map_err(|e| refused_by(e.class()))?;
        let Lookup::Mine(view) = lookup else {
            return Ok(None);
        };
        if view.run != run
            || view.agent != mother_agent()
            || view.kind != CapabilityKind::Files
            || view.state != CommitmentState::Succeeded
        {
            return Ok(None);
        }
        let clock = self.door.clock();
        Ok(Some(status_of(
            &view,
            clock.wall_ms(),
            clock.monotonic_ms(),
            deadline,
        )))
    }

    /// Decision B: the waited action is dropped only once the coordinator
    /// journaled what became of it.
    fn ack_publication(&self, mission: MissionId) {
        self.with_runs(|runs| runs.waiting.remove(&mission));
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
        self.door.confirm_mission(request)
    }

    /// Asked through the door, as [`EffectPort::confirm_mission`].
    fn confirm_held_mission(
        &self,
        request: &MissionResumeConfirmation,
    ) -> Result<MissionResumeApproval, AuthorityError> {
        let answer = self.door.confirm_held_mission(request);
        #[cfg(test)]
        self.after_held_answer.fire();
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
        // Kept until the coordinator acknowledges it (decision B): a Stop
        // after this answer still sees the File.
        status_of(&view, clock.wall_ms(), clock.monotonic_ms(), deadline)
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
        self.adopt_inside(mission, pending);
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
        // M2d (§11 M4′): published, and its authority ended after the
        // broker's final check.
        // M2d (Codex review 1 of PR #78, HIGH 2): published, recovery
        // required.
        CommitmentState::Succeeded if view.recovery_required => ApprovalStatus::RecoveryRequired,
        CommitmentState::Succeeded if !view.interrupted.is_empty() => ApprovalStatus::Overtaken {
            causes: view.interrupted.clone(),
        },
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
