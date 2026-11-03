//! The front door: one object the desktop holds for all governed
//! real-world control.
//!
//! Everything arrives here as data (a typed [`Intent`], a [`GrantRequest`]
//! from the owner's interface, a command envelope) and leaves only through
//! the control pipeline: a domain prepares, the authority commits, the
//! owner's grants and native approvals authorize, the pipeline executes
//! once. There is no other entry to any actuator.

use crate::authority::approval::{
    AgentDoorConfirmation, ControlConfirmer, MissionApproval, MissionConfirmation,
};
use crate::authority::clock::Clock;
use crate::authority::commitment::CommitmentView;
use crate::authority::effect::EffectClass;
use crate::authority::evidence::{EndStatus, EvidencePhase, EvidenceSink};
use crate::authority::ids::{AgentId, CommitmentId, GrantId, RunId};
use crate::authority::policy::{Grant, GrantScope};
use crate::authority::run::{RunClass, RunOrigin};
use crate::authority::{Authority, AuthorityError};
use crate::broker::{CredentialBroker, SecretSource, Vault};
use crate::browser::{Browser, BrowserIntent};
use crate::connector::{ConnectorIntent, Connectors};
use crate::control::{Control, EffectOutput, Preparation};
use crate::display::{AgentDisplay, DisplayStatus, InputIntent, PerceptionIntent};
use crate::egress::{Egress, EgressIntent};
use crate::evidence_store::PrimaryEvidence;
use crate::files::{
    CleanupRetry, Exclusions, FileCreate, Files, RecoveryEntry, UndoReport, WorkingDetail,
    WorkspaceInfo,
};
use crate::runtime_root::RuntimeRoot;
use crate::synthetic::{self, SyntheticIntent};
use crate::tool::{ToolIntent, Tools};
use nexus_kernel::emergency_stop::{
    HaltAudit, HaltAudited, HaltOutcome, HaltRecord, StopAuthority, StopController,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod door;
pub use crate::authority::halt::{BatchOutcome, HaltReason, HaltRecords, RecordBatch};
use door::DoorCore;
pub use door::{AgentDoor, Lookup};

/// Every governed intent, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "domain",
    content = "intent",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Intent {
    Request(EgressIntent),
    Tool(ToolIntent),
    Connector(ConnectorIntent),
    Browse(BrowserIntent),
    Observe(PerceptionIntent),
    Input(InputIntent),
    /// P4-UAT05: the owner's synthetic approval probe (owner runs only).
    Synthetic(SyntheticIntent),
    /// M2d: create one new file in the registered workspace (runs admitted
    /// through a door only).
    File(FileCreate),
}

/// What the owner may grant, as data. Each becomes a canonical scope built
/// by its domain (pinning identities where there are executables) and is
/// granted only after the owner's native confirmation of that scope.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GrantRequest {
    Egress {
        origin: String,
        methods: Vec<String>,
        #[serde(default)]
        allow_private: bool,
    },
    Tool {
        tool: String,
    },
    Connector {
        connector: String,
        account: String,
        operations: Vec<String>,
    },
    Browser {
        origins: Vec<String>,
        #[serde(default)]
        downloads: bool,
    },
    Perception,
    Input {
        max_steps: u32,
        #[serde(default)]
        session_r1: bool,
    },
    /// P4-UAT05: synthetic test receipts (no external effect). A struct
    /// variant with no fields, so that a request carrying any field is
    /// refused (a unit variant here would ignore one).
    Synthetic {},
    /// M2d: create-only Files in the registered workspace, named by its id.
    Files {
        workspace: String,
    },
}

/// What the interface shows about the control.
#[derive(Clone, Debug, Serialize)]
pub struct ControlStatus {
    pub platform_supported: bool,
    pub emergency_stopped: bool,
    pub policy_generation: u64,
    pub display: Option<(u64, u32, u16, u16)>,
    pub tools: Vec<(&'static str, &'static str, &'static str)>,
    pub connector_operations: Vec<(&'static str, &'static str, &'static str, &'static str)>,
    /// M2c: agent doors issued and still alive.
    pub live_doors: usize,
    /// M2c: agent doors issued in this process (reported, never limited).
    pub doors_issued: u64,
    /// M2c PR-B: agent doors halted in this process.
    pub doors_halted: u64,
    /// M2c PR-B: every door halt's records, accounted for.
    pub halt_records: HaltRecords,
}

/// The longest grant the interface may ask for.
pub const MAX_GRANT: Duration = Duration::from_secs(24 * 3600);

/// The governed control.
pub struct GovernedControl {
    control: Arc<Control>,
    /// M2c PR-B: the credential broker the connectors use, kept so that a
    /// door halt ends its runs' leases.
    credentials: Arc<CredentialBroker>,
    /// M2c PR-B: agent doors halted in this process.
    doors_halted: AtomicU64,
    egress: Arc<Egress>,
    connectors: Connectors,
    tools: Tools,
    display: AgentDisplay,
    browser: Browser,
    /// M2d-B1: the Files domain (the one registered workspace).
    files: Files,
    /// M2e-B: the process's one local model, once the host registers it.
    models: crate::inference::LocalModels,
    /// Runs whose agent or command has ended while something of theirs
    /// still waits: each finishes once nothing in it can start any more.
    detached: Mutex<HashSet<RunId>>,
    /// Display starts under way, each counted until its end is recorded.
    display_starts: AtomicUsize,
    /// M2c: agent doors issued in this process.
    doors_issued: AtomicU64,
    /// The controller of the kernel's emergency-stop authority (M0-FIX F5,
    /// sealed by M0-FIX2 F10): the owner's emergency stop engages it and
    /// the owner's resume lifts it. In production it is the process's
    /// controller, minted once (first claim wins) and claimed here; only
    /// its holder can lift the kernel's stop.
    kernel_stop: StopController,
    /// Tests only: runs once inside the next stop, between its kernel
    /// engagement and its broker step (where a test interleaves a resume).
    #[cfg(test)]
    stop_gap: StopGap,
}

#[cfg(test)]
type StopGap = Mutex<Option<Box<dyn FnOnce(&GovernedControl) + Send>>>;

/// One display start, counted while it lasts.
struct Starting<'a>(&'a AtomicUsize);

impl<'a> Starting<'a> {
    fn new(count: &'a AtomicUsize) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count)
    }
}

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The durable audit of kernel halts (M0-FIX2 F10): the evidence of
/// record, through the authority. It holds the control weakly, so the
/// process's stop authority (which lives as long as the process) never
/// keeps a front door or its evidence store alive; once the control is
/// gone a halt cannot be recorded, and says so.
struct KernelHaltEvidence(std::sync::Weak<Control>);

impl KernelHaltEvidence {
    fn of(control: &Arc<Control>) -> Arc<dyn HaltAudit> {
        Arc::new(Self(Arc::downgrade(control)))
    }
}

impl HaltAudit for KernelHaltEvidence {
    fn record(&self, halt: &HaltRecord) -> HaltAudited {
        let Some(control) = self.0.upgrade() else {
            return HaltAudited::Unrecorded;
        };
        let cleanup = match &halt.outcome {
            HaltOutcome::Ended => None,
            HaltOutcome::CleanupFailed(reason) => Some(reason.as_str()),
        };
        match control.authority().record_kernel_halt(
            halt.area.as_str(),
            halt.stop_number,
            halt.outcome.as_str(),
            cleanup,
        ) {
            EndStatus::Recorded { .. } => HaltAudited::Recorded,
            EndStatus::Owed => HaltAudited::Owed,
            EndStatus::Uncertain => HaltAudited::Unrecorded,
        }
    }
}

impl GovernedControl {
    /// Build the production control: `root` is the owned runtime directory,
    /// `vault` where credentials come from, `evidence` the evidence of
    /// record (the Phase Three evidence store, with its projections).
    ///
    /// Host contract (M0-FIX2 F10): this claims the process's kernel
    /// stop controller, which is minted once and never returned. Build the
    /// front door once, at startup, before any other code runs that could
    /// claim the controller (if anything claimed it first, this refuses:
    /// `AuthorityError::Unavailable`), and keep it for the life of the
    /// process. Dropping it fails closed: a kernel stop then in force can
    /// never be lifted in this process, later front doors are refused, and
    /// later model-call halts are counted as unrecorded
    /// (`StopAuthority::unrecorded_halts`).
    pub fn new(
        root: &Path,
        vault: Vault,
        evidence: Arc<PrimaryEvidence>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AuthorityError> {
        Self::with_secrets(root, vault.source(), evidence, clock)
    }

    pub(crate) fn with_secrets(
        root: &Path,
        secrets: Arc<dyn SecretSource>,
        evidence: Arc<dyn EvidenceSink>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AuthorityError> {
        let root = RuntimeRoot::open(root)?;
        let files = Files::new(root.path().to_path_buf());
        // The owner's Stop/Resume path holds the kernel stop's controller
        // (M0-FIX2 F10): the process's, which is minted once (first claim
        // wins), so a second front door, or one built after anything else
        // claimed it, is refused.
        let kernel_stop = if cfg!(test) {
            StopAuthority::new().claim_controller()
        } else {
            StopAuthority::claim_global_controller()
        }
        .ok_or(AuthorityError::Unavailable(
            "the kernel's emergency stop is held by another owner path",
        ))?;
        let control = Arc::new(Control::new(Authority::new(evidence, clock)));
        // Model calls the kernel stop halts are recorded in the evidence of
        // record (M0-FIX2 F10).
        kernel_stop
            .attach_halt_audit(KernelHaltEvidence::of(&control))
            .map_err(|_| {
                AuthorityError::Unavailable("the kernel stop's halt audit is already attached")
            })?;
        let egress = Arc::new(Egress::system());
        let broker = CredentialBroker::new(control.authority(), secrets);
        let governed = Self {
            connectors: Connectors::production(egress.clone(), broker.clone()),
            credentials: broker,
            doors_halted: AtomicU64::new(0),
            tools: Tools::production(root.clone()),
            display: AgentDisplay::new(root.clone()),
            browser: Browser::new(root),
            files,
            models: crate::inference::LocalModels::default(),
            control,
            egress,
            detached: Mutex::new(HashSet::new()),
            display_starts: AtomicUsize::new(0),
            doors_issued: AtomicU64::new(0),
            kernel_stop,
            #[cfg(test)]
            stop_gap: Mutex::new(None),
        };
        // An action may have happened unrecorded (EL-1 §7): governed control
        // starts emergency-stopped, through the stop itself, and resumes only
        // after the owner's acknowledgement.
        if governed.authority().recovery_required() {
            governed.stop_for(Some("recovery_required"));
        }
        Ok(governed)
    }

    /// Engage and lift `stop` instead of the process's kernel authority
    /// (tests only: production always uses the process's authority, so the
    /// owner's Stop cannot be disconnected from the kernel).
    #[cfg(test)]
    pub(crate) fn in_next_stop_gap(&self, gap: impl FnOnce(&GovernedControl) + Send + 'static) {
        *self.stop_gap.lock().expect("stop gap") = Some(Box::new(gap));
    }

    #[cfg(test)]
    pub(crate) fn with_kernel_stop(mut self, stop: Arc<StopAuthority>) -> Self {
        let controller = stop
            .claim_controller()
            .expect("a test's own stop authority, its controller unclaimed");
        controller
            .attach_halt_audit(KernelHaltEvidence::of(&self.control))
            .expect("a fresh authority has no halt audit");
        if self.authority().runs().is_stopped() {
            controller.engage();
        }
        self.kernel_stop = controller;
        self
    }

    /// End whatever can no longer start (expired, stale, its run or a grant
    /// ended), recording each, and finish every detached run nothing of
    /// which can start any more.
    pub fn settle(&self) {
        self.control.prune();
        // M2d-B2: the staging of Files effects that ended is discarded here,
        // never on a Stop path.
        self.files.drain();
        let detached: Vec<RunId> = self
            .detached
            .lock()
            .expect("detached")
            .iter()
            .copied()
            .collect();
        for run in detached {
            let live = self
                .authority()
                .commitments()
                .views_of_run(run)
                .iter()
                .any(|view| !view.state.is_final());
            if !live {
                self.finish_run(run);
                self.detached.lock().expect("detached").remove(&run);
            }
        }
    }

    /// The agent or command of `run` has ended: finish it now if nothing of
    /// it waits, or as soon as what waits is approved, denied or expires.
    pub fn finish_when_settled(&self, run: RunId) {
        self.detached.lock().expect("detached").insert(run);
        self.settle();
    }

    pub fn authority(&self) -> &Authority {
        self.control.authority()
    }

    /// Open an agent's run (whatever the agent is called, it is an agent's).
    pub fn open_run(&self, agent: AgentId, origin: RunOrigin) -> Result<RunId, AuthorityError> {
        self.settle();
        self.authority().open_run(agent, origin)
    }

    /// Open the run of one of the owner's own commands, from the unified
    /// front door: the only way to an owner run. Returns its agent label
    /// and the run.
    pub fn open_owner_run(
        &self,
        modalities: Vec<String>,
    ) -> Result<(AgentId, RunId), AuthorityError> {
        self.settle();
        self.authority().open_owner_run(modalities)
    }

    /// Whether `run` is one of the owner's own command runs, as the backend
    /// opened it (never judged from a name).
    pub fn is_owner_run(&self, run: RunId) -> bool {
        self.authority().runs().class(run) == Some(RunClass::Owner)
    }

    fn prepare(&self, run: RunId, intent: &Intent) -> Result<Preparation, AuthorityError> {
        let authority = self.authority();
        let preparation = match intent {
            Intent::Request(intent) => self.egress.prepare(authority, intent),
            Intent::Tool(intent) => self.tools.prepare(authority, intent),
            Intent::Connector(_) => Err(AuthorityError::InvalidAction(
                "a connector operation is proposed through its connector",
            )),
            Intent::Browse(intent) => self.browser.prepare(authority, intent),
            Intent::Observe(intent) => self.display.prepare_observation(authority, run, intent),
            Intent::Input(intent) => self.display.prepare_input(authority, run, intent),
            Intent::Synthetic(intent) => synthetic::prepare(authority, run, intent),
            Intent::File(intent) => {
                self.files
                    .prepare(authority, run, intent, self.kernel_stop.authority())
            }
        }?;
        // P4-UAT05: a synthetic action is R2 or nothing, refused before any
        // commitment exists.
        synthetic::admit(&preparation.action)?;
        Ok(preparation)
    }

    /// Prepare and commit to an intent for `agent` in `run`. Nothing
    /// happens yet.
    pub fn propose(
        &self,
        agent: &AgentId,
        run: RunId,
        intent: &Intent,
    ) -> Result<CommitmentView, AuthorityError> {
        // A connector operation proposes itself: a post first has its
        // destination identified through governed reads of this run.
        if let Intent::Connector(intent) = intent {
            return self.connectors.propose(&self.control, agent, run, intent);
        }
        let proposed = self
            .prepare(run, intent)
            .and_then(|preparation| self.control.propose(agent, run, preparation));
        // M2d-B2: a refused Files proposal's staging is discarded now (its
        // effect only handed the run over when it was dropped).
        if matches!(intent, Intent::File(_)) {
            self.files.drain();
        }
        proposed
    }

    /// Authorize (R2: the owner's native approval of exactly this
    /// commitment).
    pub fn authorize(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<(), AuthorityError> {
        self.control.authorize(id, agent, run, confirmer)
    }

    pub fn execute(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<EffectOutput, AuthorityError> {
        self.control.execute(id, agent, run)
    }

    pub fn deny(
        &self,
        id: CommitmentId,
        agent: &AgentId,
        run: RunId,
    ) -> Result<(), AuthorityError> {
        let denied = self.control.deny(id, agent, run);
        self.settle();
        denied
    }

    /// An agent's action under the owner's standing grants: R0 and R1 run
    /// now; an R2 action is committed and left for the owner's native
    /// approval from the interface. An agent's path never raises the
    /// owner's dialog itself.
    pub fn agent_action(
        &self,
        agent: &AgentId,
        run: RunId,
        intent: &Intent,
    ) -> Result<AgentOutcome, AuthorityError> {
        // An agent acts only in an agent's run: an owner run is not reached
        // by carrying the owner's label.
        if self.authority().runs().class(run) != Some(RunClass::Agent) {
            return Err(AuthorityError::WrongRun);
        }
        let view = self.propose(agent, run, intent)?;
        if view.class == EffectClass::R2 {
            return Ok(AgentOutcome::AwaitingApproval(view));
        }
        self.authorize(view.id, agent, run, &NeverAsk)?;
        self.execute(view.id, agent, run).map(AgentOutcome::Done)
    }

    pub fn cancel_run(&self, run: RunId) -> Result<(), AuthorityError> {
        self.display.forget_run(run);
        self.control.cancel_run(run)
    }

    /// The desktop is quitting: no run opens any more, and every open run
    /// is cancelled (recorded). Returns how many were cancelled.
    pub fn shut_down(&self) -> usize {
        let runs = self.authority().runs();
        runs.close();
        runs.views()
            .into_iter()
            .filter(|run| !run.cancelled && !run.finished)
            .filter(|run| self.cancel_run(run.id).is_ok())
            .count()
    }

    pub fn finish_run(&self, run: RunId) {
        self.display.forget_run(run);
        self.control.finish_run(run);
    }

    /// The owner's emergency stop: every run cancelled, every unconsumed
    /// commitment ended, the policy generation moved, the agent display
    /// stopped; no new run until the owner resumes natively.
    pub fn emergency_stop(&self) -> usize {
        self.stop_for(None)
    }

    /// An emergency stop: every effect first, unconditionally (the display
    /// stopped too), then the records, which may only become owed (EL-1
    /// §6).
    ///
    /// The acknowledgement contract (Architect's H1 decision, Option B):
    /// Stop ingress, and confirmation that the halt is engaged, are
    /// independent of evidence and cleanup; this call returning means its
    /// synchronous recording phase finished, and hosts do not wait for that
    /// return to acknowledge Stop.
    ///
    /// The engagement comes first and is memory only: the kernel stop (twice,
    /// see below), the runs, the commitments, the lease ends, the generation,
    /// and the pending effects, dropped outside the pending table (a Files
    /// effect only hands its run to the deferred collection). No lock it
    /// takes is held by anyone across evidence, filesystem or cleanup work
    /// (the kernel stop's halt log releases its state before the audit), so
    /// the halt is engaged, and observable in memory, before any blocking
    /// dependency. Only after it does Stop tear the agent display down,
    /// which may stop the display's processes (cleanup that follows the
    /// engagement), and then write the records.
    fn stop_for(&self, reason: Option<&'static str>) -> usize {
        // The kernel's own work first (M0-FIX F5): the cognitive loop,
        // coding runs and model calls halt and refuse new work.
        self.kernel_stop.engage();
        // M2d-C round 6 (R6-3, X1): then every mission's halt cell, before
        // anything is recorded, with the emergency as its first cause; the
        // cells outlive the lift (a mission with no run included).
        self.authority().engage_every_cell();
        #[cfg(test)]
        if let Some(gap) = self.stop_gap.lock().expect("stop gap").take() {
            gap(self);
        }
        let stopped = self.control.stop_effects(reason);
        // Again once the broker is stopped: a resume that ran between the
        // first engagement and the broker step may have lifted the kernel
        // stop, and the broker must never be stopped with the kernel lifted.
        self.kernel_stop.engage();
        let cancelled = stopped.cancelled;
        let display = self.display.stop();
        self.authority().write_stop_records(stopped);
        // Recorded, as every display stop is.
        self.record_display_stop(display);
        cancelled
    }

    /// Lift the emergency stop after the owner's native confirmation. The
    /// kernel's stop is lifted only once the broker's stop is lifted too;
    /// work either stop ended stays ended.
    pub fn resume(&self, confirmer: &dyn ControlConfirmer) -> Result<(), AuthorityError> {
        // The kernel stop this resume may lift: the one in force before the
        // owner is asked. A Stop pressed meanwhile (from any door) engages
        // a newer kernel stop first, and that one is not lifted here.
        let confirmed = self.kernel_stop.authority().stop_number();
        self.authority().resume(confirmer)?;
        if !self.authority().runs().is_stopped() {
            self.kernel_stop.lift_if(confirmed);
        }
        Ok(())
    }

    /// Issue a door for `agent` (Mother Nexus M2c, S1): the owner is asked
    /// every time, through `confirmer`, which the door keeps for the
    /// mission and held-mission dialogs it raises. Nothing stored decides
    /// whether to ask.
    ///
    /// 1. Refused, with no dialog and no record, while the store is
    ///    latched, while recovery is required, under an emergency stop or
    ///    while quitting.
    /// 2. A reservation under the runs lock alone: at `MAX_LIVE_DOORS` live
    ///    doors it is refused (`Capacity`), with no dialog and no record;
    ///    the groups of dropped doors are reclaimed first.
    /// 3. The owner's dialog, with no lock held.
    /// 4. No: `agent_door_declined` (an end record), and no door.
    /// 5. Yes: step 1 again (a stop pressed meanwhile wins), then
    ///    `agent_door_opened` (evidence first: unrecorded, no door).
    /// 6. The group is registered under the runs lock alone, empty.
    pub fn issue_agent_door(
        self: &Arc<Self>,
        agent: AgentId,
        confirmer: Arc<dyn ControlConfirmer + Send + Sync>,
    ) -> Result<AgentDoor, AuthorityError> {
        let authority = self.authority();
        authority.door_admissible()?;
        let runs = authority.runs();
        let core = Arc::new(DoorCore {
            id: hex::encode(crate::authority::ids::random16()),
            agent: agent.clone(),
            group: runs.next_group(),
            halt: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        let life: Arc<dyn std::any::Any + Send + Sync> = core.clone();
        runs.reserve_door_with(core.group, &agent, Arc::downgrade(&life), core.halt.clone())?;
        drop(life);
        let mut advisory = Vec::new();
        if authority.unclean_sessions() > 0 {
            advisory.push("An earlier session of the broker did not close cleanly.".to_string());
        }
        let others = runs.live_doors();
        if others > 0 {
            advisory.push(format!("{others} other door(s) are open in this process."));
        }
        let request = AgentDoorConfirmation {
            agent: agent.to_string(),
            door: core.id.clone(),
            advisory,
        };
        let yes = confirmer.confirm_agent_door(&request);
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::ISSUE_ANSWERED);
        if !yes {
            // The decline has taken effect: the reservation dies with `core`.
            authority.record_door(false, &agent, &core.id)?;
            return Err(AuthorityError::Declined);
        }
        authority.door_admissible()?;
        authority.record_door(true, &agent, &core.id)?;
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::ISSUE_RECORDED);
        runs.activate_door(core.group)?;
        self.doors_issued.fetch_add(1, Ordering::SeqCst);
        Ok(AgentDoor::new(self.clone(), core, confirmer))
    }

    /// M2c PR-B: halt a door (§4.3). Every step is in memory: no evidence is
    /// written, no dialog raised, the store not called. The batch is made
    /// first, before any state changes, so a panic in any later step drops
    /// it and its records are counted lost.
    /// 1. The runs lock (even if poisoned): the group halted, every member
    ///    snapshotted (C1) and cancelled. From here every admission of the
    ///    group refuses.
    /// 2. The commitments lock alone (even if poisoned): the member runs'
    ///    unconsumed commitments revoked. It waits for any `complete()` in
    ///    progress, so once the halt returns no member commitment moves to
    ///    Executing.
    /// 3. The lease-end slot, then the lease table, each alone (even if
    ///    poisoned): the member runs' leases ended.
    /// 4. The pending table alone (even if poisoned): their effects taken
    ///    out, dropped once it is released.
    ///
    /// Every call does every step, whether or not it set the flag; a record
    /// belongs to the call that made its transition.
    pub(crate) fn halt_door(&self, core: &DoorCore, reason: HaltReason) -> RecordBatch {
        let authority = self.authority();
        let mut batch = authority.batch();
        let (wall_ms, generation) = (
            self.authority().clock().wall_ms(),
            authority.policy_generation(),
        );
        let (members, newly) = authority.runs().halt_group(
            core.group,
            &core.id,
            &core.agent,
            reason,
            wall_ms,
            generation,
            &mut batch,
        );
        if newly {
            self.doors_halted.fetch_add(1, Ordering::SeqCst);
        }
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::HALT_AFTER_RUNS);
        let leases =
            authority
                .commitments()
                .revoke_runs(&members, &AuthorityError::Halted, &mut batch);
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::HALT_AFTER_ENTRIES);
        authority.commitments().end_leases_even_if_poisoned(&leases);
        self.credentials.end_runs(&members);
        #[cfg(test)]
        self.hooks().fire(crate::test_hooks::at::HALT_AFTER_LEASES);
        self.control.drop_pending_of(&members);
        batch
    }

    /// M2c PR-B: a door's cancellation of one of its runs: the halt's steps
    /// for that run alone (no `door_halted`), its records in the batch.
    pub(crate) fn cancel_member(&self, run: RunId) -> RecordBatch {
        let authority = self.authority();
        let mut batch = authority.batch();
        let (wall_ms, generation) = (
            self.authority().clock().wall_ms(),
            authority.policy_generation(),
        );
        authority
            .runs()
            .cancel_into(run, wall_ms, generation, &mut batch);
        self.display.forget_run(run);
        let only = std::collections::BTreeSet::from([run]);
        let leases =
            authority
                .commitments()
                .revoke_runs(&only, &AuthorityError::RunCancelled, &mut batch);
        authority.commitments().end_leases_even_if_poisoned(&leases);
        self.credentials.end_runs(&only);
        self.control.drop_pending_of(&only);
        batch
    }

    /// The broker's test hooks (tests).
    #[cfg(test)]
    pub(crate) fn hooks(&self) -> &Arc<crate::test_hooks::Hooks> {
        self.authority().hooks()
    }

    /// Ask the owner, natively, to consent to an agent's mission (Mother
    /// Nexus M2a): see `Authority::confirm_mission`. It grants nothing.
    pub fn confirm_mission(
        &self,
        request: &MissionConfirmation,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<MissionApproval, AuthorityError> {
        self.authority().confirm_mission(request, confirmer)
    }

    /// The canonical scope for a grant request (identities pinned now).
    pub fn grant_scope(&self, request: &GrantRequest) -> Result<GrantScope, AuthorityError> {
        match request {
            GrantRequest::Egress {
                origin,
                methods,
                allow_private,
            } => Egress::grant_scope(origin, methods, *allow_private),
            GrantRequest::Tool { tool } => self.tools.grant_scope(tool),
            GrantRequest::Connector {
                connector,
                account,
                operations,
            } => self.connectors.grant_scope(connector, account, operations),
            GrantRequest::Browser { origins, downloads } => {
                self.browser.grant_scope(origins, *downloads)
            }
            GrantRequest::Perception => Ok(AgentDisplay::perception_scope()),
            GrantRequest::Input {
                max_steps,
                session_r1,
            } => AgentDisplay::input_scope(*max_steps, *session_r1),
            GrantRequest::Synthetic {} => Ok(synthetic::grant_scope()),
            GrantRequest::Files { workspace } => self.files.grant_scope(workspace),
        }
    }

    /// M2d (§6.2): register the one workspace of this process, the Owner's
    /// own selection (the host's `--workspace`), never a door's. Exactly
    /// `/`, `$HOME` or the identity home is refused; so is a folder inside
    /// or containing a `reserved` tree (the host names Mother's home, the
    /// staging parent and the Files ledger directory; the broker adds its
    /// runtime root and `<identity home>/.nexus`), one that is not its own
    /// canonical path, not the current user's or not empty. Nothing is
    /// persisted: after a restart the Owner selects it again.
    pub fn register_workspace(
        &self,
        path: &Path,
        reserved: &[&Path],
    ) -> Result<WorkspaceInfo, AuthorityError> {
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .filter(|home| home.is_absolute())
            .ok_or(AuthorityError::Unavailable("no home directory"))?;
        let identity_home = nexus_kernel::identity_home::identity_home()
            .map_err(|_| AuthorityError::Unavailable("no identity home"))?;
        let mut trees: Vec<std::path::PathBuf> = reserved.iter().map(|p| p.to_path_buf()).collect();
        trees.push(identity_home.join(".nexus"));
        #[cfg(target_os = "linux")]
        let uid = crate::launcher::effective_uid();
        #[cfg(not(target_os = "linux"))]
        let uid = u32::MAX;
        self.register_workspace_in(
            path,
            &Exclusions {
                home,
                identity_home,
                reserved: trees,
                uid,
            },
        )
    }

    /// [`Self::register_workspace`] with what may not be the workspace
    /// given (tests name their own homes).
    pub(crate) fn register_workspace_in(
        &self,
        path: &Path,
        exclusions: &Exclusions,
    ) -> Result<WorkspaceInfo, AuthorityError> {
        self.files.register(path, exclusions)
    }

    /// The registered workspace, if any (its id and display name).
    pub fn workspace(&self) -> Option<WorkspaceInfo> {
        self.files.info()
    }

    /// The Files domain (a door's admission reads the workspace).
    pub(crate) fn files(&self) -> &Files {
        &self.files
    }

    /// M2d (§6.6, H2′): undo the last file Files created in this process,
    /// once, after the Owner's own dialog (`ControlConfirmer::confirm_undo`)
    /// and under fresh authority: the kernel takes fresh stop watches when
    /// this undo starts, before its dialog, and checks those same watches
    /// at its final check (U1); a fresh write grant is issued for it. The
    /// Owner's only: no door route. Refused while a stop is in force, when
    /// there is nothing to undo (a restart offers nothing), and when the
    /// file is no longer exactly the one created (nothing is touched). An
    /// interrupted or refused undo stays offered. A completed undo reports
    /// a kernel write grant that lapsed after its final check (U1).
    pub fn files_undo(
        &self,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<UndoReport, AuthorityError> {
        self.files
            .undo(self.authority(), self.kernel_stop.authority(), confirmer)
    }

    /// M2d (§6.7): the startup recovery report: what the Files ledger says
    /// is uncertain or owed (a prepared apply or undo without an outcome,
    /// temps owed, recovery required, staging awaiting approval), with ids
    /// and digests only. Reporting only: nothing is swept, and a ledger
    /// that cannot be read is an error, never an empty report.
    pub fn files_recovery_report(&self) -> Result<Vec<RecoveryEntry>, AuthorityError> {
        self.files.recovery_report()
    }

    /// M2d (I4, D-O24): the Owner-only detail behind a Files execution's
    /// failure code, held in memory and taken once.
    pub fn files_working_detail(&self, id: CommitmentId) -> Option<WorkingDetail> {
        self.files.working_detail(id)
    }

    /// M2d (R3): the Owner's startup hold before every Files publication
    /// (the host's `--hold-before-publish`), at most 30 seconds and set
    /// before the workspace is registered. During it a cancellation, a stop
    /// or an expiry is seen at once; after it the ordinary final check
    /// still decides. It approves nothing.
    pub fn hold_files_before_publish(&self, hold: Duration) -> Result<(), AuthorityError> {
        self.files.set_hold(hold)
    }

    /// M2d (§6.6; Codex review 1 of PR #64, M1): the Owner's bounded retry
    /// of owed Files cleanup (temporary files the kernel could not remove,
    /// staging whose removal failed): one attempt per owed run; what is
    /// established is released, the rest stays owned and in the recovery
    /// report. The Owner's only: no door route.
    pub fn files_retry_cleanup(&self) -> Result<CleanupRetry, AuthorityError> {
        self.files.retry_cleanup()
    }

    /// M2e-B (design v2 §8): register the process's one local model, the
    /// host's own selection (no door route), at the loopback `endpoint`,
    /// pinned to `spec`. The kernel transport verifies it with no broker
    /// lock held, under a watch of this front door's kernel stop, the
    /// session's end and an absolute 30 s deadline: the runtime's version
    /// and cloud status, the model's name, digest, description and
    /// capabilities. Refused under Stop, while quitting, off loopback, for
    /// a second model, and on any mismatch (its `model_<code>` class). The
    /// registration is recorded first (Restricted: the manifest digest and
    /// the identity digest); unrecorded, there is no model. Nothing is
    /// persisted: after a restart the host registers it again.
    pub fn register_local_model(
        &self,
        endpoint: &str,
        spec: nexus_kernel::inference::ModelPinSpec,
    ) -> Result<crate::inference::ModelIdentityView, AuthorityError> {
        self.models.register(
            self.authority(),
            endpoint,
            &spec,
            self.kernel_stop.authority(),
        )
    }

    /// M2e-B: the registered local model, if any.
    pub(crate) fn registered_model(&self) -> Option<Arc<crate::inference::RegisteredModel>> {
        self.models.registered()
    }

    /// M2e-B: the stop authority a model call admits its watch from.
    pub(crate) fn kernel_stop_authority(&self) -> &Arc<StopAuthority> {
        self.kernel_stop.authority()
    }

    /// Ask the owner, natively, for a grant (a synthetic one lasts at most
    /// `synthetic::GRANT_TTL`).
    pub fn request_grant(
        &self,
        request: &GrantRequest,
        ttl: Duration,
        confirmer: &dyn ControlConfirmer,
    ) -> Result<GrantId, AuthorityError> {
        let scope = self.grant_scope(request)?;
        let ttl = synthetic::capped(&scope, ttl.min(MAX_GRANT));
        self.authority().grants().request(scope, ttl, confirmer)
    }

    pub fn revoke_grant(&self, id: GrantId) -> Result<(), AuthorityError> {
        let revoked = self.authority().grants().revoke(id);
        // What relied on it ends now, recorded, not when next touched.
        self.settle();
        revoked
    }

    pub fn grants(&self) -> Vec<Grant> {
        self.authority().grants().all()
    }

    /// Start the agent display: a process Phase Three owns, so it needs a
    /// live perception or input grant and no emergency stop, and its start
    /// and stop are recorded.
    pub fn start_display(&self) -> Result<DisplayStatus, AuthorityError> {
        // Under way until it returns, its end recorded: quitting waits.
        let _starting = Starting::new(&self.display_starts);
        if self.authority().runs().is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        if self.authority().runs().is_closed() {
            return Err(AuthorityError::Closed("the desktop is quitting"));
        }
        let grants = self.authority().grants();
        if grants
            .live_of(crate::authority::effect::CapabilityKind::Perception)
            .is_empty()
            && grants
                .live_of(crate::authority::effect::CapabilityKind::Input)
                .is_empty()
        {
            return Err(AuthorityError::NoCoveringGrant);
        }
        // Evidence first: an unrecorded display does not start. An
        // emergency stop, or quitting, that comes while it starts ends it
        // unused.
        let runs = self.authority().runs();
        let recorded = std::cell::Cell::new(false);
        let started = self.display.start(
            1280,
            800,
            || {
                let record = self.authority().record_display(
                    EvidencePhase::DisplayStarted,
                    vec![("size".into(), "1280x800".into())],
                );
                recorded.set(record.is_ok());
                record
            },
            || runs.is_stopped() || runs.is_closed(),
        );
        // Refused because the desktop is quitting, not by an emergency stop.
        let started = started.map_err(|error| {
            if error == AuthorityError::EmergencyStopped && !runs.is_stopped() && runs.is_closed() {
                AuthorityError::Closed("the desktop is quitting")
            } else {
                error
            }
        });
        // A start recorded but not completed is recorded as ended, so the
        // evidence never shows a display that is not there.
        if let Err(error) = &started {
            if recorded.get() {
                let outcome = match error {
                    AuthorityError::Closed(_) | AuthorityError::EmergencyStopped => {
                        "stopped while it started"
                    }
                    _ => "the start failed",
                };
                self.authority().record_display_end(
                    EvidencePhase::DisplayStopped,
                    vec![("outcome".into(), outcome.into())],
                );
            }
        }
        started
    }

    /// Take and release every lock of the control once (tests).
    #[cfg(test)]
    pub(crate) fn probe_locks(&self) {
        self.control.probe_locks();
        self.display.probe_locks();
        drop(self.detached.lock().expect("detached"));
    }

    /// Whether an agent display start is under way (quitting waits for it,
    /// so its end is recorded).
    pub fn is_starting_display(&self) -> bool {
        self.display_starts.load(Ordering::SeqCst) > 0
    }

    /// Stop the agent display (recorded if one ran). A stop always counts:
    /// a start in progress ends unused, even when no display runs yet.
    pub fn stop_display(&self) {
        let display = self.display.stop();
        self.record_display_stop(display);
    }

    /// The record of a display stop that has taken effect.
    fn record_display_stop(&self, stopped: Option<DisplayStatus>) {
        if let Some(status) = stopped {
            self.authority().record_display_end(
                EvidencePhase::DisplayStopped,
                vec![("display".into(), status.number.to_string())],
            );
        }
    }

    pub fn status(&self) -> ControlStatus {
        ControlStatus {
            platform_supported: cfg!(target_os = "linux"),
            emergency_stopped: self.authority().runs().is_stopped(),
            policy_generation: self.authority().policy_generation(),
            display: self
                .display
                .status()
                .map(|s| (s.generation, s.number, s.width, s.height)),
            tools: self
                .tools
                .tools()
                .into_iter()
                .map(|(key, class, executable)| (key, class.as_str(), executable))
                .collect(),
            connector_operations: self
                .connectors
                .operations()
                .into_iter()
                .map(|(connector, op, class, method)| (connector, op, class.as_str(), method))
                .collect(),
            live_doors: self.authority().runs().live_doors(),
            doors_issued: self.doors_issued.load(Ordering::SeqCst),
            doors_halted: self.doors_halted.load(Ordering::SeqCst),
            halt_records: self.authority().halt_records(),
        }
    }
}

/// Declines everything: R0 and R1 authorization never asks, and nothing on
/// an agent's path may ask the owner.
pub(crate) struct NeverAsk;

impl ControlConfirmer for NeverAsk {
    fn confirm_action(&self, _: &crate::authority::approval::ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &crate::authority::approval::GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &crate::authority::approval::ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(
        &self,
        _: &crate::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        false
    }
}

/// What an agent's governed action came to.
#[derive(Debug)]
pub enum AgentOutcome {
    Done(EffectOutput),
    /// Committed; the owner approves (or not) from the interface.
    AwaitingApproval(CommitmentView),
}

#[cfg(test)]
mod door_tests;
#[cfg(all(test, target_os = "linux"))]
mod files_exec_tests;
#[cfg(all(test, target_os = "linux"))]
mod files_tests;
#[cfg(test)]
mod halt_tests;
#[cfg(test)]
mod tests;
