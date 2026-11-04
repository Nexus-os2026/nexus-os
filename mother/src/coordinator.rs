//! The coordinator: turns a contract into a governed mission.
//!
//! Every change is journaled before it takes effect. Nothing reaches the
//! EffectPort before the Owner consented to the mission's exact contract
//! hash, and nothing reaches it after Stop. No coordinator lock is held
//! while the Owner is asked, while a lane runs or while an effect runs: a
//! call is reserved under the lock, made with the lock released, and its
//! result is kept only if the mission did not move meanwhile. Stop never
//! waits for a lane or an effect in flight. It still waits for its own
//! journal operations, for in-memory locks other calls hold, and for its
//! broker records, which it writes after the cancellation has taken effect
//! and while holding no lock. (The journal append is the transition's
//! commit, a bounded local write, and is made under the lock so that the
//! journal's order is the state machine's.)
//!
//! M2a: the Owner consents through the broker's native mission dialog, and
//! a live consent is made only from the broker's approval of the exact
//! hash and request asked about. An effect that waits for the Owner's
//! native approval puts the mission in AwaitingApproval: Mother asks the
//! broker what became of it and never approves it herself; once it ran,
//! the phase goes on from where it paused.
//!
//! M2a-S: every session journals its opening before anything else it
//! writes (at once if the journal's last session closed cleanly), and only
//! an orderly shutdown of a coordinator that is not broken
//! ([`Coordinator::close`]) journals a clean close. A journal whose last
//! session did not close cleanly may lack a transition whose append failed
//! together with its poisoning (a Stop, a denial): every unfinished mission
//! then comes back held, and only the Owner's resume (journaled) or Stop
//! moves it. The holds are journaled with the session's opening.
//!
//! M2c: consent and resume are asked through the port's agent door, whose
//! confirmer the host bound; the coordinator takes no confirmer. A resume
//! follows the consent pattern: the request is reserved under the lock
//! (replacing any earlier one, the generation moved), the Owner is asked
//! with no lock held, and the answer is taken under the lock only for the
//! request still current; it is consumed once. A resume takes effect only
//! when `Resumed` is journaled, under the lock; the broker's record of the
//! Owner's yes resumes nothing. What can land, and when:
//!
//! | What lands, and when | Outcome |
//! |---|---|
//! | Q2 reserved while Q1 is in its dialog | Q1's take fails (its request was replaced, the generation moved); Q2's answer is the only one that can count, once |
//! | A Stop of the mission before the take (in the dialog, the broker's record or before the journal write) | the Stop's transition moves the generation and clears the request and the hold: the take discards the answer and returns `Stopped` |
//! | A break (journal failure) before the take | the coordinator is broken: the take refuses |
//! | A fence of the port outside a break (the public port fence; in PR-B also poison recovery, which halts the door outside the lock), after the broker's post-record check and before the take | the take may journal `Resumed`: the Owner's recorded yes exists, and the fenced port (in PR-B, the halted door) refuses every effect of the mission |
//! | An emergency stop after the broker's post-record check | the take may journal `Resumed`; every effect of the mission is then refused by the broker. Resuming a held mission permits no effect by itself |
//! | Anything after the take | acts on a resumed mission, as on any other |
//!
//! P0 (privacy closure): the contract and a paused phase's rest are put in
//! the private store (with no lock held) before the entry naming them is
//! journaled; the journal keeps references and closed codes only. Replay
//! builds every mission from its references alone; recovery then loads the
//! content of each unfinished mission and verifies it (content id, size,
//! parse, contract hash). Content that does not verify stops its mission:
//! its journaled requests are withdrawn first, then the Stop
//! (`private_content_unavailable`) is journaled. A terminal mission's
//! contract is loaded only when asked for. Every error is a closed code;
//! the detail behind one (an unanswered question's id, a lane's reason, a
//! broker's text) is the Owner's [`WorkingDetail`], in memory only.

use crate::consent::{Consent, MissionSummary};
use crate::contract::{sha256, Budget, ContractError, ContractHash, EffectKind, MissionContract};
use crate::effect::{
    Admission, ApprovalStatus, EffectOutcome, EffectPort, EffectRequest, Owed, PendingAction,
    Publication,
};
use crate::evidence::{
    check, lane_identity_digest, BlockReason, BrokerClass, ConsentRefusedReason, FailReason,
    HeldReason, LapseReason, MotherRefusal, PublicationCause, SettleCause, SettledOutcome,
    SettlementOutcome, StopReason, VerifyFailReason, WorkingDetail, FILE_UNCERTAIN_NOTICE,
};
use crate::journal::{
    verify, ContractRef, Entry, Event, JournalError, JournalStore, MissionId, ReplayViolation,
    RestRef, StoreFailure, GENESIS, SESSION,
};
use crate::lane::{
    input_hash, output_hash, InvocationId, Lane, LaneIdentity, LaneInput, LaneRole, Verdict,
};
use crate::private::{ContentKind, PrivateError, PrivateStore};
use crate::state::{transition, IllegalTransition, MissionState, Trigger};
use nexus_governed_control::authority::approval::{
    MissionApproval, MissionConfirmation, MissionResumeApproval, MissionResumeConfirmation,
};
use nexus_governed_control::authority::clock::Clock;
use nexus_governed_control::authority::halt_cell::{HaltCause, HaltCells};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::authority::AuthorityError;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The most missions one coordinator keeps.
pub const MAX_MISSIONS: usize = 1024;
/// The most effects one lane may ask for in one call.
pub const MAX_EFFECTS_PER_CALL: usize = 16;
/// P0: the most missions whose working detail the coordinator keeps (the
/// oldest is dropped first).
pub const MAX_DETAILS: usize = 64;

const REQUEST_DOMAIN: &[u8] = b"nexus-mother-effect-request-v1\n";

/// One lane per role.
pub struct Lanes {
    by_role: BTreeMap<LaneRole, Arc<dyn Lane>>,
}

impl Lanes {
    /// Exactly one lane for every role, each naming its own role.
    pub fn new(lanes: Vec<Arc<dyn Lane>>) -> Result<Self, CoordinatorError> {
        let mut by_role = BTreeMap::new();
        for lane in lanes {
            let role = lane.identity().role;
            if by_role.insert(role, lane).is_some() {
                return Err(CoordinatorError::Lanes(LaneConfig::Duplicate(role)));
            }
        }
        if let Some(missing) = LaneRole::ALL.iter().find(|r| !by_role.contains_key(r)) {
            return Err(CoordinatorError::Lanes(LaneConfig::Missing(*missing)));
        }
        Ok(Self { by_role })
    }

    fn get(&self, role: LaneRole) -> &Arc<dyn Lane> {
        &self.by_role[&role]
    }
}

/// Why the lanes are not exactly one per role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneConfig {
    Duplicate(LaneRole),
    Missing(LaneRole),
}

/// Why the coordinator refused: closed codes only (P0). The detail, where
/// there is any, is the Owner's [`Coordinator::working_detail`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatorError {
    UnknownMission,
    /// M2d-C round 5 (D1): the coordinator's lock is poisoned (a Stop path
    /// says so, after its halt cells are engaged, instead of panicking).
    Poisoned,
    Contract(ContractError),
    Journal(JournalError),
    Illegal(IllegalTransition),
    /// The lanes are not exactly one per role.
    Lanes(LaneConfig),
    Capacity,
    /// Work was asked of a mission without a consent covering its contract.
    NoConsent,
    /// The Owner's answer came for a contract or state that had moved on.
    ConsentStale,
    /// The broker's approval was for another contract hash or request.
    ConsentMismatch,
    /// The broker refused to ask, or to record the answer: its class.
    Broker(BrokerClass),
    /// The port could not withdraw a mission's request: its class.
    Withdrawal(BrokerClass),
    /// P0: the private store could not keep or give back a mission's
    /// content.
    Private(PrivateError),
    Denied,
    Stopped,
    /// Another call is already working on this mission.
    Busy,
    /// M2c: the Owner said no to resuming a held mission (it stays held).
    Declined,
    /// M2c: the Owner's answer to a resume came for a request that is no
    /// longer current (replaced, or already taken): it is discarded.
    ResumeStale,
    /// M2a-S: the mission is held after an unclean shutdown; only the
    /// Owner's resume or Stop moves it.
    Held,
    /// M2a-S: the coordinator was closed.
    Closed,
    /// M2d-C round 6 (R6-2, X2″): the journal's identity, or the binding
    /// of its replayed references to surviving work, could not be
    /// established at the open.
    Identity(IdentityRefusal),
}

/// M2d-C round 6 (R6-2, X2″): why an open could not bind its journal to the
/// broker's halt cells (fail closed: the port is fenced where anything of
/// it was reached).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityRefusal {
    /// The store has no identity of its own: no coordinator opens over it
    /// (a test build takes a local one instead).
    NoIdentity,
    /// A replayed reference to a surviving run does not carry this
    /// journal's cell for its mission: that run is stopped, and nothing of
    /// the journal is exposed.
    SurvivingRunMismatch,
    /// A replayed settlement's first cause is not its cell's.
    CauseMismatch,
    /// The broker's run state could not be read.
    Unreadable,
}

/// R6-3: the cell's cause a settlement's first cause engages it with.
fn halt_cause_of(cause: SettleCause) -> HaltCause {
    match cause {
        SettleCause::Stop(StopReason::EmergencyStop) => HaltCause::Emergency,
        SettleCause::Stop(StopReason::DoorHalted) => HaltCause::Fence,
        SettleCause::TimeBudget | SettleCause::Ended(BlockReason::BudgetTime) => {
            HaltCause::TimeBudget
        }
        SettleCause::Stop(_) | SettleCause::Ended(_) => HaltCause::Stop,
    }
}

/// R6-3: the settlement cause an engaged cell's first cause stands for: the
/// terminal of a mission touched first after its cell was engaged elsewhere.
fn settle_cause_of(cause: HaltCause) -> SettleCause {
    match cause {
        HaltCause::Stop => SettleCause::Stop(StopReason::Owner),
        HaltCause::Emergency => SettleCause::Stop(StopReason::EmergencyStop),
        HaltCause::TimeBudget => SettleCause::TimeBudget,
        HaltCause::Fence => SettleCause::Stop(StopReason::DoorHalted),
    }
}

/// One lane call, as reported. Its `Debug` shows no lane text or reason
/// (P0): only the role, the verdict's kind, the sizes and the hashes.
#[derive(Clone, PartialEq, Eq)]
pub struct LaneReport {
    pub invocation: InvocationId,
    pub identity: LaneIdentity,
    pub text: String,
    pub verdict: Verdict,
    pub input_hash: String,
    pub output_hash: String,
}

/// One effect, as reported: dispatched to the port, or refused by Mother.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectReport {
    pub kind: EffectKind,
    pub dispatched: bool,
    pub outcome: String,
}

/// What one [`Coordinator::advance`] did. Its `Debug` shows no lane text,
/// lane reason or explanation (P0): they are for the Owner's display.
#[derive(Clone, PartialEq, Eq)]
pub struct Advance {
    pub from: MissionState,
    pub to: MissionState,
    pub lanes: Vec<LaneReport>,
    pub effects: Vec<EffectReport>,
    /// Set when the mission reaches Done: what Mother tells the Owner.
    pub explanation: Option<String>,
    /// Set when the mission ends Blocked or Failed: a closed code.
    pub reason: Option<String>,
}

fn verdict_kind(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Continue => "continue",
        Verdict::Pass => "pass",
        Verdict::Fail { .. } => "fail",
        Verdict::Halt { .. } => "halt",
    }
}

impl fmt::Debug for LaneReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneReport")
            .field("invocation", &self.invocation)
            .field("role", &self.identity.role)
            .field("text_bytes", &self.text.len())
            .field("verdict", &verdict_kind(&self.verdict))
            .field("input_hash", &self.input_hash)
            .field("output_hash", &self.output_hash)
            .finish()
    }
}

impl fmt::Debug for Advance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Advance")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("lanes", &self.lanes)
            .field("effects", &self.effects)
            .field(
                "explanation_bytes",
                &self.explanation.as_ref().map(String::len),
            )
            .field("reason", &self.reason)
            .finish()
    }
}

struct Mission {
    /// The journal's reference to the contract (always known).
    contract_ref: ContractRef,
    /// The contract itself, from the private store: set live, and by
    /// recovery for an unfinished mission (a terminal one's is loaded only
    /// when asked for).
    contract: Option<MissionContract>,
    state: MissionState,
    consent: Option<Consent>,
    /// Bumped by every transition: a reservation holds while it is equal.
    generation: u64,
    busy: bool,
    /// Wall time of the consent, from the journal.
    approved_wall_ms: Option<u64>,
    /// Monotonic reference for the time budget, and the time already
    /// spent when it was taken.
    mono_ref: u64,
    spent_at_ref: u64,
    model_calls: u32,
    next_effect: u64,
    unsettled: Option<u64>,
    /// An effect was dispatched since the last transition: re-running the
    /// phase after a restart could repeat it.
    dispatched_in_phase: bool,
    /// A lane call journaled as invoked and not yet answered (replay).
    pending_lane: Option<InvocationId>,
    prior_outputs: Vec<String>,
    reason: Option<String>,
    /// The broker request the Owner is being asked about (in memory).
    asking: Option<MissionConfirmation>,
    /// M2c: the held-mission request the Owner is being asked about (in
    /// memory), until its answer is taken or a transition clears it.
    resume_asking: Option<MissionResumeConfirmation>,
    /// What the mission waits on in AwaitingApproval.
    waiting: Option<Waiting>,
    /// The effect last settled as awaiting approval, with no transition
    /// since (replay: a wait follows it).
    awaiting_settled: Option<u64>,
    /// The journaled identity of an action at the broker (with its
    /// effect), from its record until its closure is journaled, whatever
    /// becomes of the mission.
    open_action: Option<(u64, PendingAction)>,
    /// M2a-S: why the mission is held (an unclean shutdown), until the
    /// Owner resumes or stops it.
    held: Option<HeldReason>,
    /// The hold is in the journal (otherwise it is written before the
    /// session's next record).
    held_journaled: bool,
    /// Replay: an unclean session opened while the mission was unfinished
    /// and not held, so the live coordinator holds it before its next
    /// record (recovery aside) and before the session's clean close.
    must_hold: bool,
    /// M2d (I7): the consent lifetime: the journal sequence of the consent
    /// or resume the mission works under. Every perform carries it.
    lifetime: u64,
    /// M2d: the request of the Owner's yes taken in this process (memory
    /// only: after a restart there is none, and Files cannot be admitted).
    consent_request: Option<String>,
    /// M2d: the lifetime the port admitted, in this process.
    admitted: Option<u64>,
    /// M2d-C round 4 (settlement design v2): the File effect dispatched
    /// and not yet resolved (a refusal, its approval, a denial or lapse
    /// confirmed not published, or its settlement's terminal line).
    unresolved_file: Option<u64>,
    /// Rule 4/6: the File whose settlement began, and its first cause. From
    /// then on the mission is never runnable again (on replay too); only
    /// the settlement's terminal line ends it.
    settlement: Option<(u64, SettleCause)>,
    /// Rule 2/6: the broker's identities of this mission's File actions
    /// whose conclusive outcome is journaled, acknowledged again on every
    /// reopen (idempotently; an uncertain one is never among them).
    acks: Vec<PendingAction>,
}

/// An effect that waits for the Owner's approval, and what its phase had
/// still to do.
#[derive(Clone)]
struct Waiting {
    phase: MissionState,
    effect: u64,
    /// The journal's reference to the rest (always known).
    rest_ref: RestRef,
    /// The rest itself, from the private store: set live, and by recovery.
    rest: Option<Vec<EffectRequest>>,
    roles_left: Vec<LaneRole>,
}

impl Mission {
    fn new(contract_ref: ContractRef, contract: Option<MissionContract>) -> Self {
        Self {
            contract_ref,
            contract,
            state: MissionState::Draft,
            consent: None,
            generation: 0,
            busy: false,
            approved_wall_ms: None,
            mono_ref: 0,
            spent_at_ref: 0,
            model_calls: 0,
            next_effect: 0,
            unsettled: None,
            dispatched_in_phase: false,
            pending_lane: None,
            prior_outputs: Vec::new(),
            reason: None,
            asking: None,
            resume_asking: None,
            waiting: None,
            awaiting_settled: None,
            open_action: None,
            held: None,
            held_journaled: false,
            must_hold: false,
            lifetime: 0,
            consent_request: None,
            admitted: None,
            unresolved_file: None,
            settlement: None,
            acks: Vec::new(),
        }
    }

    /// Take a transition the table accepted. A consent's token is set by
    /// the caller: live from the broker's approval, on replay from the
    /// verified journal.
    fn apply(&mut self, trigger: &Trigger, to: MissionState, mono_now: u64) {
        let mut dispatched = false;
        // Round 4: the waited File, resolved by its approval, a denial or a
        // lapse (each confirmed not published, or published cleanly): its
        // journaled identity is owed an acknowledgement.
        let waited_file = self
            .waiting
            .as_ref()
            .map(|w| w.effect)
            .filter(|effect| self.unresolved_file == Some(*effect));
        if let (
            Some(effect),
            Trigger::ActionApproved { .. }
            | Trigger::ActionDenied { .. }
            | Trigger::ApprovalLapsed { .. },
        ) = (waited_file, trigger)
        {
            if let Some((_, pending)) = self.open_action.as_ref().filter(|(e, _)| *e == effect) {
                self.acks.push(pending.clone());
            }
            self.unresolved_file = None;
        }
        match trigger {
            Trigger::Revise { contract } => {
                // The caller sets the new contract itself (live); replay
                // has only the reference.
                self.contract_ref = contract.clone();
                self.contract = None;
                self.consent = None;
                self.approved_wall_ms = None;
                self.consent_request = None;
                self.admitted = None;
            }
            Trigger::Consent { at_wall_ms, .. } => {
                self.approved_wall_ms = Some(*at_wall_ms);
                self.mono_ref = mono_now;
                self.spent_at_ref = 0;
            }
            Trigger::VerifyFail { reason } => self.reason = Some(reason.to_string()),
            Trigger::Block { reason } => self.reason = Some(reason.to_string()),
            Trigger::Fail { reason } => self.reason = Some(reason.to_string()),
            Trigger::AwaitApproval {
                effect,
                rest,
                roles_left,
            } => {
                self.waiting = Some(Waiting {
                    phase: self.state,
                    effect: *effect,
                    rest_ref: rest.clone(),
                    rest: None,
                    roles_left: roles_left.clone(),
                });
            }
            Trigger::ActionApproved { .. } => {
                self.waiting = None;
                // An effect of this phase has run: re-running the phase
                // after a restart could ask for it again.
                dispatched = true;
            }
            Trigger::ApprovalLapsed { reason, .. } => {
                self.waiting = None;
                self.reason = Some(reason.to_string());
            }
            Trigger::ActionDenied { .. } => {
                self.waiting = None;
            }
            Trigger::Settled {
                action,
                outcome,
                end,
                ..
            } => {
                self.waiting = None;
                self.reason = Some(end.as_str());
                if let Some(pending) = action.as_ref().filter(|_| outcome.conclusive()) {
                    self.acks.push(pending.clone());
                }
                self.unresolved_file = None;
            }
            _ => {}
        }
        self.state = to;
        if to.is_terminal() {
            self.held = None;
            self.held_journaled = false;
            self.must_hold = false;
        }
        self.generation += 1;
        self.dispatched_in_phase = dispatched;
        self.asking = None;
        self.resume_asking = None;
        self.awaiting_settled = None;
    }

    /// M2a-S: a held mission does nothing until the Owner resumes it.
    fn not_held(&self) -> Result<(), CoordinatorError> {
        match self.held {
            Some(_) => Err(CoordinatorError::Held),
            None => Ok(()),
        }
    }

    fn consented(&self) -> bool {
        self.consent
            .as_ref()
            .is_some_and(|consent| consent.hash() == self.contract_ref.contract_hash)
    }

    /// The contract of a live mission (recovery loaded it, or stopped the
    /// mission).
    fn contract(&self) -> Result<&MissionContract, CoordinatorError> {
        self.contract
            .as_ref()
            .ok_or(CoordinatorError::Private(PrivateError::Missing))
    }

    fn budget(&self) -> Option<&Budget> {
        self.contract.as_ref().map(|c| &c.budget)
    }

    /// Time spent since consent: the larger of the wall and monotonic
    /// measures, so that neither clock alone can stretch the budget.
    fn spent_ms(&self, clock: &dyn Clock) -> u64 {
        let wall = self
            .approved_wall_ms
            .map_or(0, |at| clock.wall_ms().saturating_sub(at));
        let mono = self
            .spent_at_ref
            .saturating_add(clock.monotonic_ms().saturating_sub(self.mono_ref));
        wall.max(mono)
    }

    /// Without its contract a mission has no budget left (fail closed).
    fn out_of_time(&self, clock: &dyn Clock) -> bool {
        self.budget()
            .is_none_or(|budget| self.spent_ms(clock) >= budget.max_duration_ms)
    }
}

/// M2d-C round 5 (D1): what a mission's time budget is measured from, kept
/// outside the coordinator lock so a Stop path can check it first.
#[derive(Clone, Copy)]
struct BudgetRef {
    approved_wall_ms: u64,
    mono_ref: u64,
    spent_at_ref: u64,
    max_ms: u64,
}

impl BudgetRef {
    fn spent(&self, clock: &dyn Clock) -> bool {
        let wall = clock.wall_ms().saturating_sub(self.approved_wall_ms);
        let mono = self
            .spent_at_ref
            .saturating_add(clock.monotonic_ms().saturating_sub(self.mono_ref));
        wall.max(mono) >= self.max_ms
    }
}

/// D1's budget table: a dedicated mutex taken even if poisoned (it maps ids
/// to copies), held only to insert or read, never across anything else.
#[derive(Default)]
struct Budgets(Mutex<HashMap<MissionId, BudgetRef>>);

impl Budgets {
    fn lock(&self) -> MutexGuard<'_, HashMap<MissionId, BudgetRef>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// M2d-C round 5 (H2, C1): the coordinator's halt cells: the broker's
/// table for its agent and this journal's identity. Outside the
/// coordinator mutex; the table's own lock is dedicated (C3).
#[derive(Clone)]
struct Cells {
    table: HaltCells,
    journal: String,
}

impl Cells {
    fn engage(&self, id: MissionId) {
        if let Some(cell) = self.table.get(&self.journal, id.0) {
            cell.engage();
        }
    }

    /// C2: one snapshot of this journal's cells, engaged after the table's
    /// lock is released; the cohort's missions, in order.
    fn engage_cohort(&self) -> Vec<MissionId> {
        let cohort = self.table.cohort(&self.journal);
        for (_, cell) in &cohort {
            cell.engage();
        }
        cohort.into_iter().map(|(m, _)| MissionId(m)).collect()
    }
}

struct Inner {
    store: Box<dyn JournalStore>,
    head: String,
    next_seq: u64,
    /// The store failed: nothing more is journaled or started.
    broken: bool,
    /// M2a-S: the clean close is journaled: nothing more is journaled or
    /// acknowledged.
    closed: bool,
    /// M2a-S: this session's opening is journaled.
    session_journaled: bool,
    /// M2a-S: the port's fence, run under this lock at the moment the
    /// store fails: no broker call enters the port from then on, an action
    /// executing at the broker is interrupted (its run is cancelled), and
    /// every run the port holds is cancelled. It never waits for an effect,
    /// and (M2c) does no evidence I/O: it returns its records.
    fence: Box<dyn Fn() -> Owed + Send>,
    /// M2c PR-B: the records the fence left (in memory only), taken out by
    /// `with_lock` before it releases this lock and written by the same
    /// call once it is released.
    owed: Owed,
    /// M2c PR-B (tests): run in a journal failure after the fence, before
    /// the poison marker.
    #[cfg(test)]
    fence_to_marker: crate::oracles::m2as_hooks::HookSlot,
    missions: BTreeMap<MissionId, Mission>,
    next_mission: u64,
    /// Numbers the consent requests of this coordinator.
    next_request: u64,
    /// P0: the Owner's working detail, the last per mission (at most
    /// [`MAX_DETAILS`], the oldest dropped first), cleared by the mission's
    /// next call. Memory only.
    details: VecDeque<(MissionId, WorkingDetail)>,
    /// Round 5: the cells (shared with the coordinator) and the budgets.
    cells: Cells,
    budgets: Arc<Budgets>,
}

impl Inner {
    /// Journal one record, after this session's opening and any hold not
    /// journaled yet.
    fn append(&mut self, mission: MissionId, event: Event) -> Result<u64, CoordinatorError> {
        self.open_session()?;
        self.write(mission, event)
    }

    /// M2a-S: journal this session's opening, then every hold not journaled
    /// yet, once.
    fn open_session(&mut self) -> Result<(), CoordinatorError> {
        if !self.session_journaled {
            self.write(SESSION, Event::SessionOpened)?;
            self.session_journaled = true;
        }
        let holds: Vec<(MissionId, HeldReason)> = self
            .missions
            .iter()
            .filter(|(_, m)| !m.held_journaled)
            .filter_map(|(id, m)| m.held.map(|reason| (*id, reason)))
            .collect();
        for (id, reason) in holds {
            self.write(id, Event::Held { reason })?;
            self.mission(id)?.held_journaled = true;
        }
        Ok(())
    }

    /// M2a-S: on a journal whose last session closed cleanly, journal this
    /// session's opening at once. If that write fails nothing was lost (the
    /// journal still ends in the clean close, or a torn line it refuses), so
    /// the journal is not poisoned: opening fails and may be tried again
    /// (with a new port: the failure fenced this one).
    fn open_session_at_once(&mut self) -> Result<(), CoordinatorError> {
        self.write_record(SESSION, Event::SessionOpened, false)?;
        self.session_journaled = true;
        Ok(())
    }

    /// M2a-S: refused once the clean close is journaled, or once the store
    /// failed.
    fn usable(&self) -> Result<(), CoordinatorError> {
        if self.closed {
            return Err(CoordinatorError::Closed);
        }
        if self.broken {
            return Err(CoordinatorError::Journal(JournalError::Store(
                StoreFailure::FailedEarlier,
            )));
        }
        Ok(())
    }

    /// P0: the mission's next call clears its working detail.
    fn clear_detail(&mut self, id: MissionId) {
        self.details.retain(|(mission, _)| *mission != id);
    }

    /// P0: keep `detail` as the mission's working detail.
    fn set_detail(&mut self, id: MissionId, detail: WorkingDetail) {
        self.clear_detail(id);
        if self.details.len() >= MAX_DETAILS {
            self.details.pop_front();
        }
        self.details.push_back((id, detail));
    }

    fn write(&mut self, mission: MissionId, event: Event) -> Result<u64, CoordinatorError> {
        self.write_record(mission, event, true)
    }

    /// M2a-S: after an outbound call (a perform, a lane, the Owner's dialog,
    /// an approval query) its result is kept only if the coordinator is
    /// still usable and the mission not held; otherwise it is discarded and
    /// nothing is journaled for it.
    fn kept(&mut self, id: MissionId) -> Result<(), CoordinatorError> {
        self.usable()?;
        self.mission(id)?.not_held()
    }

    fn write_record(
        &mut self,
        mission: MissionId,
        event: Event,
        poison: bool,
    ) -> Result<u64, CoordinatorError> {
        self.usable()?;
        // P0: only evidence is journaled. A refused event writes nothing
        // (the journal is intact): the caller fails closed.
        if let Err(code) = check(&event) {
            return Err(CoordinatorError::Journal(JournalError::Inconsistent {
                line: self.next_seq as usize + 1,
                code,
            }));
        }
        let entry = Entry::chained(self.next_seq, &self.head, mission, event);
        if let Err(e) = self.store.append(&entry.to_line()) {
            // Fail closed, durably: a journal that may have lost a
            // transition (a Stop) must not be reopened as if it had not.
            // If poisoning fails too, the journal reopens, but its session
            // has no clean close: every unfinished mission comes back held.
            // Fenced at the authority first, under this same lock: nothing
            // reserved before the break can run at the broker after it.
            self.broken = true;
            let owed = (self.fence)();
            self.owed.extend(owed);
            #[cfg(test)]
            self.fence_to_marker.fire();
            if poison && self.store.poison().is_err() {
                // The next open holds every unfinished mission.
                return Err(CoordinatorError::Journal(JournalError::Store(
                    StoreFailure::PoisonFailed,
                )));
            }
            return Err(CoordinatorError::Journal(e));
        }
        self.head = entry.hash;
        self.next_seq += 1;
        Ok(entry.seq)
    }

    fn mission(&mut self, id: MissionId) -> Result<&mut Mission, CoordinatorError> {
        if self.closed {
            return Err(CoordinatorError::Closed);
        }
        self.missions
            .get_mut(&id)
            .ok_or(CoordinatorError::UnknownMission)
    }

    /// Journal a trigger, then take it. A mission whose File settlement
    /// began takes none (rule 6): only the settlement's terminal line,
    /// through [`Inner::fire_settling`].
    fn fire(
        &mut self,
        id: MissionId,
        trigger: Trigger,
        mono_now: u64,
    ) -> Result<MissionState, CoordinatorError> {
        if self.mission(id)?.settlement.is_some() {
            return Err(CoordinatorError::Stopped);
        }
        self.fire_settling(id, trigger, mono_now)
    }

    /// [`Inner::fire`] without the settling guard: the settlement's own
    /// terminal transition.
    fn fire_settling(
        &mut self,
        id: MissionId,
        trigger: Trigger,
        mono_now: u64,
    ) -> Result<MissionState, CoordinatorError> {
        let from = self.mission(id)?.state;
        let to = transition(from, &trigger).map_err(|e| refusal(from, e))?;
        self.append(
            id,
            Event::Transition {
                trigger: trigger.clone(),
            },
        )?;
        self.mission(id)?.apply(&trigger, to, mono_now);
        self.after_transition(id, &trigger, to);
        Ok(to)
    }

    /// Round 5: after a transition taken (journaled or in memory): a
    /// terminal mission's cell is retired (engaged, then dropped: C1); a
    /// consent starts the budget D1's precheck reads.
    fn after_transition(&mut self, id: MissionId, trigger: &Trigger, to: MissionState) {
        // A Done mission's cell is retired once its run is finished (the
        // finish is refused on an engaged cell).
        if to.is_terminal() && to != MissionState::Done {
            self.cells.table.retire(&self.cells.journal, id.0);
            self.budgets.lock().remove(&id);
        } else if let Trigger::Consent { .. } = trigger {
            self.note_budget(id);
        }
    }

    /// D1: the mission's budget reference, outside the lock (none without
    /// its contract or consent: the in-lock check alone then decides).
    fn note_budget(&mut self, id: MissionId) {
        let Some(mission) = self.missions.get(&id) else {
            return;
        };
        let (Some(at), Some(budget)) = (mission.approved_wall_ms, mission.budget()) else {
            return;
        };
        let reference = BudgetRef {
            approved_wall_ms: at,
            mono_ref: mission.mono_ref,
            spent_at_ref: mission.spent_at_ref,
            max_ms: budget.max_duration_ms,
        };
        self.budgets.lock().insert(id, reference);
    }
}

/// The error for a trigger a state does not accept.
fn refusal(from: MissionState, illegal: IllegalTransition) -> CoordinatorError {
    match from {
        MissionState::Stopped => CoordinatorError::Stopped,
        MissionState::Denied => CoordinatorError::Denied,
        _ => CoordinatorError::Illegal(illegal),
    }
}

fn illegal(from: MissionState, trigger: &'static str) -> CoordinatorError {
    refusal(from, IllegalTransition { from, trigger })
}

/// Whether a file request is a plain path inside the requested folder:
/// relative, without `..`, drive or root.
fn relative_request(path: &str) -> bool {
    !path.is_empty()
        && nexus_governed_control::authority::evidence::is_plain(path)
        && !path.starts_with(['/', '\\'])
        && !path.contains(':')
        && path.split(['/', '\\']).all(|part| part != "..")
}

/// M2d (§6.1 step 3): one top-level name, `[A-Za-z0-9_-][A-Za-z0-9._-]`,
/// at most 64 characters. (It cannot start with `.`, so never with the
/// kernel's staging prefix.)
fn file_name_ok(name: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= nexus_governed_control::files::MAX_NAME_CHARS
        && first(bytes[0])
        && bytes[1..].iter().all(|&b| first(b) || b == b'.')
}

/// M2d: Mother's own refusal of a File create (§6.1 steps 2-3): the
/// name, the content (text, size, lines) and the workspace.
fn file_refuses(
    contract: &MissionContract,
    create: &nexus_governed_control::files::FileCreate,
    workspace: Option<&str>,
) -> Option<MotherRefusal> {
    use nexus_governed_control::files::{MAX_FILE_BYTES, MAX_FILE_LINES};
    let content = &create.content;
    Some(if !relative_request(&create.name) {
        MotherRefusal::FileRequestNotRelative
    } else if !file_name_ok(&create.name) {
        MotherRefusal::FileNameInvalid
    } else if content.contains('\0') {
        MotherRefusal::FileNotText
    } else if content.len() > MAX_FILE_BYTES {
        MotherRefusal::FileTooLarge
    } else if content.split_inclusive('\n').count() > MAX_FILE_LINES {
        MotherRefusal::FileTooManyLines
    } else if workspace.is_none() || contract.requested_folder.as_deref() != workspace {
        MotherRefusal::FileFolderNotWorkspace
    } else {
        return None;
    })
}

/// Mother's own refusal of a lane's effect, before any port call.
fn mother_refuses(
    contract: &MissionContract,
    effect: &EffectRequest,
    workspace: Option<&str>,
) -> Option<MotherRefusal> {
    let kind = effect.kind();
    if contract.forbidden_effects.contains(&kind) {
        return Some(MotherRefusal::ForbiddenEffect(kind));
    }
    match effect {
        EffectRequest::Spend { .. } if contract.budget.max_money_minor == 0 => {
            Some(MotherRefusal::BudgetMoneyZero)
        }
        EffectRequest::Spend { .. } => Some(MotherRefusal::SpendNotSupported),
        EffectRequest::File { .. } if contract.requested_folder.is_none() => {
            Some(MotherRefusal::NoRequestedFolder)
        }
        EffectRequest::File(create) => file_refuses(contract, create, workspace),
        _ => None,
    }
}

/// The lanes a working phase runs, in order (none outside them).
fn phase_roles(phase: MissionState) -> &'static [LaneRole] {
    match phase {
        MissionState::Investigating => &[
            LaneRole::Sentinel,
            LaneRole::ResearcherA,
            LaneRole::ResearcherB,
            LaneRole::Skeptic,
        ],
        MissionState::Building => &[LaneRole::Sentinel, LaneRole::Builder],
        MissionState::Verifying => &[LaneRole::Sentinel, LaneRole::Verifier],
        _ => &[],
    }
}

fn request_hash(effect: &EffectRequest) -> String {
    let json = serde_json::to_vec(effect).expect("an effect request serialises");
    hex::encode(sha256(REQUEST_DOMAIN, &[&json]))
}

/// The canonical bytes of a contract (those its hash covers).
fn contract_bytes(contract: &MissionContract) -> Vec<u8> {
    serde_json::to_vec(contract).expect("a contract serialises")
}

/// The coordinator.
pub struct Coordinator<P: EffectPort> {
    port: Arc<P>,
    lanes: Lanes,
    clock: Arc<dyn Clock>,
    /// P0: where the contract and paused rests are kept. Called with no
    /// coordinator lock held.
    private: Arc<dyn PrivateStore>,
    inner: Mutex<Inner>,
    /// M2d-C round 5 (H2, C1): the halt cells and budget references, held
    /// outside `inner`: every Stop path engages them first.
    cells: Cells,
    budgets: Arc<Budgets>,
    /// M2c PR-B (tests): the call points of the crash and schedule oracles.
    #[cfg(test)]
    pub(crate) hooks: CoordinatorHooks,
}

/// M2c PR-B (tests): hooks at `with_lock`'s write (after the lock is
/// released, before and after the batches are written; they run also when
/// there are none) and in a journal failure.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct CoordinatorHooks {
    pub(crate) before_owed_write: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) after_owed_write: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) fence_to_marker: crate::oracles::m2as_hooks::HookSlot,
    /// Round 5 (tests): a Stop path's halt cells are engaged, before any
    /// coordinator lock.
    pub(crate) after_halt: crate::oracles::m2as_hooks::HookSlot,
    /// Round 5 (tests): `advance`'s budget precheck is done, before its
    /// first lock.
    pub(crate) after_precheck: crate::oracles::m2as_hooks::HookSlot,
}

/// What a step under the coordinator lock leaves to do once it is released.
enum Then<T> {
    /// Go on with the value.
    Go(T),
    /// End the mission (Blocked) for this reason, and report its state.
    End(BlockReason),
    /// End the mission for this reason, in memory if need be, then fail.
    EndFail(BlockReason, CoordinatorError),
}

/// What `advance`'s first step under the lock decided.
enum Start {
    /// An emergency stop holds: stop the mission.
    Emergency(Advance),
    /// The time budget is spent: end the mission.
    OutOfTime(Advance),
    /// Done in the step: report it.
    Done(Advance),
    /// Explained and done: finish the port, then report it.
    Finish(Advance),
    /// Run the phase's lanes for this reservation.
    Run(Advance, MissionState, &'static [LaneRole], u64),
}

/// Clears a mission's busy mark however its call ends.
struct Busy<'a, P: EffectPort> {
    coordinator: &'a Coordinator<P>,
    id: MissionId,
}

impl<P: EffectPort> Drop for Busy<'_, P> {
    /// Its own lock, directly: it never journals (so it makes no batch).
    fn drop(&mut self) {
        if let Ok(mut inner) = self.coordinator.inner.lock() {
            #[cfg(test)]
            debug_assert!(inner.owed.is_empty(), "a batch left in the coordinator");
            if let Some(mission) = inner.missions.get_mut(&self.id) {
                mission.busy = false;
            }
        }
    }
}

/// M2d-C round 4: what one mission's first Stop step did (rule 4).
enum Stepped {
    /// Terminal already, or its settlement under way (the first cause
    /// stands): left as it is.
    Done(MissionState),
    /// Its File's settlement began: withdrawn and reconciled after every
    /// first step.
    Settle,
    /// An ordinary Stop: whether it was journaled, and whether the mission
    /// may have dispatched (then it is withdrawn).
    Stopped(Result<MissionState, CoordinatorError>, bool),
    /// No File: the time budget (or another end) blocks it, as before.
    End(BlockReason),
}

/// Rule 1: only a confirmed not-published read says nothing was written;
/// pending (executing, or not answered) is uncertain once it is stopped.
fn settlement_outcome(read: Publication) -> SettlementOutcome {
    match read {
        Publication::Published {
            recovery_required,
            interrupted,
        } => SettlementOutcome::Published {
            recovery_required,
            interrupted: interrupted
                .iter()
                .map(|c| PublicationCause::of(c))
                .collect(),
        },
        Publication::ConfirmedNotPublished => SettlementOutcome::NotPublished,
        Publication::Pending | Publication::Uncertain => SettlementOutcome::Uncertain,
    }
}

/// What a phase's lanes came to.
enum Ended {
    /// Every lane ran and the phase may move on.
    Through,
    /// The mission ended (Blocked or Failed) during the phase.
    Ended(MissionState),
}

impl<P: EffectPort> Coordinator<P> {
    /// Rebuild a coordinator from its journal (empty for a new home). A
    /// journal that does not verify opens nothing. A mission whose last
    /// effect's outcome was not recorded is blocked: it may have happened.
    /// M2a-S: if the journal's last session did not close cleanly, every
    /// unfinished mission is held until the Owner resumes or stops it.
    ///
    /// P0: `private` keeps the contracts and paused rests the journal names.
    /// Every entry is checked as evidence before it is replayed.
    pub fn open(
        store: impl JournalStore + 'static,
        port: Arc<P>,
        lanes: Lanes,
        clock: Arc<dyn Clock>,
        private: impl PrivateStore + 'static,
    ) -> Result<Self, CoordinatorError>
    where
        P: 'static,
    {
        let lines = store.load().map_err(CoordinatorError::Journal)?;
        let entries = verify(&lines).map_err(CoordinatorError::Journal)?;
        let mut missions = BTreeMap::new();
        let mut next_mission = 1;
        let mut session = Session::Never;
        for (index, entry) in entries.iter().enumerate() {
            check(&entry.event)
                .and_then(|()| replay(&mut missions, &mut next_mission, &mut session, entry))
                .map_err(|code| {
                    CoordinatorError::Journal(JournalError::Inconsistent {
                        line: index + 1,
                        code,
                    })
                })?;
        }
        // Clean only on positive evidence: the last session's orderly close
        // (an empty journal holds nothing). A journal from before sessions
        // never closed cleanly.
        let closed_cleanly = session == Session::Closed;
        let clean = entries.is_empty() || closed_cleanly;
        let (mono_now, wall_now) = (clock.monotonic_ms(), clock.wall_ms());
        for mission in missions.values_mut() {
            if let Some(at) = mission.approved_wall_ms {
                mission.mono_ref = mono_now;
                // A wall clock now earlier than the consent cannot be
                // trusted to measure the budget: count it as spent.
                mission.spent_at_ref = if wall_now < at {
                    u64::MAX
                } else {
                    wall_now - at
                };
            }
        }
        let fencer = port.clone();
        // Round 5 (C1) and round 6 (R6-2, X2″): this journal's identity (the
        // store's own: a file's device and inode, a memory journal's
        // instance) and the agent's halt table. A store with no identity
        // opens nothing in production; a test build gives its test
        // wrappers (`Disk`, `Gated`) an identity local to this coordinator.
        let journal = match store.identity() {
            Some(identity) => identity,
            None => {
                #[cfg(not(test))]
                {
                    return Err(CoordinatorError::Identity(IdentityRefusal::NoIdentity));
                }
                #[cfg(test)]
                {
                    crate::journal::fresh_identity("anonymous")
                }
            }
        };
        // X2: the anchor the broker keeps while any cell of this journal
        // lives: the identity cannot be reused meanwhile.
        let anchor = store.anchor();
        let cells = Cells {
            table: port.halt_cells(),
            journal,
        };
        let budgets = Arc::new(Budgets::default());
        #[cfg(test)]
        let hooks = CoordinatorHooks::default();
        let coordinator = Self {
            port,
            lanes,
            clock,
            private: Arc::new(private),
            inner: Mutex::new(Inner {
                store: Box::new(store),
                head: entries
                    .last()
                    .map_or(GENESIS.to_string(), |e| e.hash.clone()),
                next_seq: entries.len() as u64,
                broken: false,
                closed: false,
                session_journaled: false,
                fence: Box::new(move || fencer.fence()),
                owed: Owed::default(),
                #[cfg(test)]
                fence_to_marker: hooks.fence_to_marker.clone(),
                missions,
                next_mission,
                next_request: 1,
                details: VecDeque::new(),
                cells: cells.clone(),
                budgets: budgets.clone(),
            }),
            cells,
            budgets,
            #[cfg(test)]
            hooks,
        };
        // Round 5 (C1): every replayed unfinished mission's cell, before
        // anything of it is exposed (recovery included); a cell that cannot
        // be had fails closed: the port's fence (its door's memory stop),
        // then a typed error.
        let unfinished: Vec<MissionId> = coordinator.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .map(|(id, _)| *id)
                .collect()
        });
        for id in &unfinished {
            match coordinator
                .cells
                .table
                .cell(&coordinator.cells.journal, id.0)
            {
                Ok(cell) => coordinator.port.bind_halt(*id, cell),
                Err(_) => {
                    coordinator.port.fence().write();
                    return Err(CoordinatorError::Capacity);
                }
            }
        }
        if let Some(anchor) = anchor {
            coordinator
                .cells
                .table
                .anchor(&coordinator.cells.journal, anchor);
        }
        // X3: a replayed settlement's first cause is its cell's: a fresh
        // cell is engaged with it, an engaged one must agree.
        let settling: Vec<(MissionId, SettleCause)> = coordinator.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .filter_map(|(id, m)| m.settlement.map(|(_, cause)| (*id, cause)))
                .collect()
        });
        for (id, cause) in settling {
            let Some(cell) = coordinator.cells.table.get(&coordinator.cells.journal, id.0) else {
                continue;
            };
            cell.engage_with(halt_cause_of(cause));
            if cell.cause() != Some(halt_cause_of(cause)) {
                coordinator.port.fence().write();
                return Err(CoordinatorError::Identity(IdentityRefusal::CauseMismatch));
            }
        }
        // If opening fails from here on (the journal is then poisoned and
        // never reopens, or, if poisoning failed too, reopens with every
        // unfinished mission held), every journaled open action is
        // withdrawn first, a waiting mission's too.
        if let Err(e) = coordinator.recover(closed_cleanly, clean) {
            coordinator.withdraw_open_actions();
            return Err(e);
        }
        // R6-2: a cell of this identity that belongs to no replayed
        // unfinished mission (a replaced file whose identity was reused,
        // unknown surviving work, a terminal mission's leftover) is engaged
        // and retired: it can never run. After recovery, which adopts and
        // withdraws the journaled actions of terminal missions first (an
        // engaged cell admits no adoption); before anything is exposed.
        for (mission, cell) in coordinator.cells.table.cohort(&coordinator.cells.journal) {
            if !unfinished.contains(&MissionId(mission)) {
                cell.engage_with(HaltCause::Fence);
                coordinator
                    .cells
                    .table
                    .retire(&coordinator.cells.journal, mission);
            }
        }
        // D1: the budgets of the missions recovery kept (their contracts are
        // loaded now).
        coordinator.with_lock(|inner| {
            let ids: Vec<MissionId> = inner.missions.keys().copied().collect();
            for id in ids {
                inner.note_budget(id);
            }
        });
        Ok(coordinator)
    }

    /// M2c PR-B: run `body` under the coordinator lock. A record batch it
    /// left (a break's fence) is taken out still under the lock and written
    /// once the lock is released, by this call, on every way out (a `?` in
    /// `body` returns from `body` only). No call can see another's batch.
    /// Round 5 (D1): [`Self::with_lock`] for the Stop paths: a poisoned
    /// coordinator lock is `Poisoned`, never a panic.
    fn try_with_lock<T>(
        &self,
        body: impl FnOnce(&mut Inner) -> Result<T, CoordinatorError>,
    ) -> Result<T, CoordinatorError> {
        let (result, owed) = {
            let mut inner = self.inner.lock().map_err(|_| CoordinatorError::Poisoned)?;
            #[cfg(test)]
            let _held = crate::effect::held::Held::coordinator();
            let result = body(&mut inner);
            (result, std::mem::take(&mut inner.owed))
        };
        #[cfg(test)]
        self.hooks.before_owed_write.fire();
        owed.write();
        #[cfg(test)]
        self.hooks.after_owed_write.fire();
        result
    }

    fn with_lock<T>(&self, body: impl FnOnce(&mut Inner) -> T) -> T {
        let (result, owed) = {
            let mut inner = self.inner.lock().expect("mother coordinator");
            #[cfg(test)]
            let _held = crate::effect::held::Held::coordinator();
            let result = body(&mut inner);
            (result, std::mem::take(&mut inner.owed))
        };
        #[cfg(test)]
        self.hooks.before_owed_write.fire();
        owed.write();
        #[cfg(test)]
        self.hooks.after_owed_write.fire();
        result
    }

    /// Read-only, under the coordinator lock: it cannot journal, so it
    /// makes no batch.
    fn peek<T>(&self, body: impl FnOnce(&Inner) -> T) -> T {
        let inner = self.inner.lock().expect("mother coordinator");
        body(&inner)
    }

    /// After a locked step, with the lock released: go on with its value,
    /// or do what it left (end the mission; or end it and fail).
    fn then<T>(
        &self,
        id: MissionId,
        step: Then<T>,
    ) -> Result<Result<T, MissionState>, CoordinatorError> {
        match step {
            Then::Go(value) => Ok(Ok(value)),
            Then::End(reason) => Ok(Err(self.end(id, reason)?)),
            Then::EndFail(reason, error) => {
                let _ = self.end(id, reason);
                Err(error)
            }
        }
    }

    /// P0: keep a contract in the private store (no lock held): its
    /// reference.
    fn put_contract(&self, contract: &MissionContract) -> Result<ContractRef, CoordinatorError> {
        let bytes = contract_bytes(contract);
        let content = self
            .private
            .put(ContentKind::Contract, &bytes)
            .map_err(CoordinatorError::Private)?;
        Ok(ContractRef {
            contract_hash: contract.hash(),
            content,
            bytes: bytes.len() as u64,
        })
    }

    /// P0: a contract from the private store, only if it verifies against
    /// its reference: the content id (the store checks it), the size, the
    /// parse, the contract hash and the contract's bounds.
    fn fetch_contract(&self, reference: &ContractRef) -> Result<MissionContract, PrivateError> {
        let bytes = self
            .private
            .get(ContentKind::Contract, &reference.content)?;
        if bytes.len() as u64 != reference.bytes {
            return Err(PrivateError::Mismatch);
        }
        let contract: MissionContract =
            serde_json::from_slice(&bytes).map_err(|_| PrivateError::Mismatch)?;
        if contract.hash() != reference.contract_hash || contract.validate().is_err() {
            return Err(PrivateError::Mismatch);
        }
        Ok(contract)
    }

    /// P0: a paused rest from the private store, only if it verifies
    /// against its reference (content id, size, parse, count).
    fn fetch_rest(&self, reference: &RestRef) -> Result<Vec<EffectRequest>, PrivateError> {
        let bytes = self
            .private
            .get(ContentKind::PausedPhase, &reference.content)?;
        if bytes.len() as u64 != reference.bytes {
            return Err(PrivateError::Mismatch);
        }
        let rest: Vec<EffectRequest> =
            serde_json::from_slice(&bytes).map_err(|_| PrivateError::Mismatch)?;
        if rest.len() as u64 != reference.count {
            return Err(PrivateError::Mismatch);
        }
        Ok(rest)
    }

    /// A mission's contract: in memory, or (a terminal mission after a
    /// reopen) fetched and verified now.
    fn contract_of(&self, id: MissionId) -> Result<MissionContract, CoordinatorError> {
        let (loaded, reference) = self.peek(|inner| {
            inner
                .missions
                .get(&id)
                .map(|m| (m.contract.clone(), m.contract_ref.clone()))
                .ok_or(CoordinatorError::UnknownMission)
        })?;
        match loaded {
            Some(contract) => Ok(contract),
            None => self
                .fetch_contract(&reference)
                .map_err(CoordinatorError::Private),
        }
    }

    /// Draft a mission. Nothing reaches the port. P0: the contract is put
    /// in the private store first; the journal names it.
    pub fn draft(&self, contract: MissionContract) -> Result<MissionId, CoordinatorError> {
        contract.validate().map_err(CoordinatorError::Contract)?;
        self.with_lock(|inner| {
            if inner.missions.len() >= MAX_MISSIONS {
                return Err(CoordinatorError::Capacity);
            }
            inner.usable()
        })?;
        let reference = self.put_contract(&contract)?;
        self.with_lock(|inner| {
            if inner.missions.len() >= MAX_MISSIONS {
                return Err(CoordinatorError::Capacity);
            }
            let id = MissionId(inner.next_mission);
            // Round 5 (C1): its cell first, before it is journaled or exposed.
            let cell = inner
                .cells
                .table
                .cell(&inner.cells.journal, id.0)
                .map_err(|_| CoordinatorError::Capacity)?;
            inner.append(
                id,
                Event::Drafted {
                    contract: reference.clone(),
                },
            )?;
            inner.next_mission += 1;
            inner
                .missions
                .insert(id, Mission::new(reference, Some(contract)));
            self.port.bind_halt(id, cell);
            Ok(id)
        })
    }

    /// Change a mission's contract before work starts: it returns to Draft
    /// and any consent is void. P0: the new contract is put in the private
    /// store first; the journal names it.
    pub fn revise(
        &self,
        id: MissionId,
        contract: MissionContract,
    ) -> Result<MissionState, CoordinatorError> {
        let may_revise = |inner: &mut Inner| {
            let mission = inner.mission(id)?;
            mission.not_held()?;
            let state = mission.state;
            if !matches!(
                state,
                MissionState::Draft | MissionState::AwaitingConsent | MissionState::Approved
            ) {
                return Err(illegal(state, "revise"));
            }
            Ok(())
        };
        self.with_lock(|inner| {
            inner.clear_detail(id);
            may_revise(inner)
        })?;
        contract.validate().map_err(CoordinatorError::Contract)?;
        let reference = self.put_contract(&contract)?;
        self.with_lock(|inner| {
            may_revise(inner)?;
            let mono = self.clock.monotonic_ms();
            let state = inner.fire(
                id,
                Trigger::Revise {
                    contract: reference,
                },
                mono,
            )?;
            inner.mission(id)?.contract = Some(contract);
            Ok(state)
        })
    }

    /// Put a complete contract (every question answered) to the Owner. P0:
    /// an unanswered question is named by its index; its id is the Owner's
    /// working detail.
    pub fn submit(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.with_lock(|inner| {
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            mission.not_held()?;
            if mission.state != MissionState::Draft {
                return Err(illegal(mission.state, "submit"));
            }
            let contract = mission.contract()?;
            if let Err(e) = contract.validate_for_consent() {
                if let ContractError::UnansweredQuestion { index } = e {
                    let question = contract.questions[index].id.clone();
                    inner.set_detail(id, WorkingDetail::new("unanswered question", &question));
                }
                return Err(CoordinatorError::Contract(e));
            }
            let hash = mission.contract_ref.contract_hash;
            let mono = self.clock.monotonic_ms();
            inner.fire(id, Trigger::Submit { hash }, mono)
        })
    }

    /// What the Owner would be shown.
    pub fn confirmation(&self, id: MissionId) -> Result<MissionSummary, CoordinatorError> {
        let contract = self.contract_of(id)?;
        Ok(MissionSummary::of(id, &contract))
    }

    /// P0: the Owner's working detail for `id` (the last call's: an
    /// unanswered question's id, a lane's reason, a broker's refusal), if
    /// any. Taken once; memory only.
    pub fn working_detail(&self, id: MissionId) -> Option<WorkingDetail> {
        self.with_lock(|inner| {
            let at = inner.details.iter().position(|(m, _)| *m == id)?;
            inner.details.remove(at).map(|(_, detail)| detail)
        })
    }

    /// Ask the Owner, through the broker's native mission dialog, by the
    /// port's door (M2c: its confirmer is the host's). No lock is held
    /// while the Owner answers. A consent is made only from the broker's
    /// recorded yes to the exact hash and request asked about, while the
    /// mission still awaits consent to that hash; the broker's no denies
    /// the mission.
    pub fn request_consent(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let (request, generation) = self.consent_request(id)?;
        let answer = self.port.confirm_mission(&request);
        self.take_answer(id, generation, answer)
    }

    /// Reserve a consent request: the broker's confirmation for the
    /// mission's current contract under a request label of its own, which
    /// replaces any earlier request of the mission, and the reservation.
    pub(crate) fn consent_request(
        &self,
        id: MissionId,
    ) -> Result<(MissionConfirmation, u64), CoordinatorError> {
        self.with_lock(|inner| {
            // A broken coordinator asks the Owner nothing.
            inner.usable()?;
            inner.clear_detail(id);
            let (seq, number) = (inner.next_seq, inner.next_request);
            inner.next_request += 1;
            let mission = inner.mission(id)?;
            mission.not_held()?;
            if mission.state != MissionState::AwaitingConsent {
                return Err(illegal(mission.state, "consent"));
            }
            let request = MissionSummary::of(id, mission.contract()?)
                .confirmation(format!("{id}/{seq}/{number}"));
            mission.asking = Some(request.clone());
            Ok((request, mission.generation))
        })
    }

    /// Take the broker's answer to the mission's current request.
    pub(crate) fn take_answer(
        &self,
        id: MissionId,
        generation: u64,
        answer: Result<MissionApproval, AuthorityError>,
    ) -> Result<MissionState, CoordinatorError> {
        self.with_lock(|inner| {
            // M2a-S: an answer that comes after the break is discarded.
            inner.kept(id)?;
            let mission = inner.mission(id)?;
            let hash = mission.contract_ref.contract_hash;
            let asked = match &mission.asking {
                Some(asked)
                    if mission.generation == generation
                        && mission.state == MissionState::AwaitingConsent
                        && asked.contract_hash == hash.to_hex() =>
                {
                    asked.clone()
                }
                _ => {
                    let stopped = mission.state == MissionState::Stopped;
                    inner.append(
                        id,
                        Event::ConsentRefused {
                            reason: ConsentRefusedReason::MissionChanged,
                        },
                    )?;
                    return Err(if stopped {
                        CoordinatorError::Stopped
                    } else {
                        CoordinatorError::ConsentStale
                    });
                }
            };
            let mono = self.clock.monotonic_ms();
            match answer {
                Ok(approval) => {
                    let Some(consent) = Consent::from_broker(approval, &asked) else {
                        inner.append(
                            id,
                            Event::ConsentRefused {
                                reason: ConsentRefusedReason::AnotherRequest,
                            },
                        )?;
                        return Err(CoordinatorError::ConsentMismatch);
                    };
                    let at_wall_ms = self.clock.wall_ms();
                    let state = inner.fire(
                        id,
                        Trigger::Consent {
                            hash: consent.hash(),
                            at_wall_ms,
                        },
                        mono,
                    )?;
                    let lifetime = inner.next_seq - 1;
                    let mission = inner.mission(id)?;
                    mission.consent = Some(consent);
                    mission.lifetime = lifetime;
                    mission.consent_request = Some(asked.request);
                    mission.admitted = None;
                    Ok(state)
                }
                Err(AuthorityError::Declined) => {
                    inner.fire(id, Trigger::Deny { hash }, mono)?;
                    Err(CoordinatorError::Denied)
                }
                Err(refused) => {
                    let class = BrokerClass::of(refused.class());
                    inner.set_detail(id, WorkingDetail::new("broker", &refused.to_string()));
                    inner.append(
                        id,
                        Event::ConsentRefused {
                            reason: ConsentRefusedReason::BrokerRefused(class),
                        },
                    )?;
                    Err(CoordinatorError::Broker(class))
                }
            }
        })
    }

    /// Move a consented mission one state forward, running that state's
    /// lanes and dispatching the effects they ask for that pass every
    /// check. A mission that waits for the Owner's approval asks the broker
    /// what became of the action, and goes on only once it ran.
    pub fn advance(&self, id: MissionId) -> Result<Advance, CoordinatorError> {
        let emergency = self.port.emergency_stopped();
        // D1 / H3: before any lock: the emergency's cells and door stop, or
        // the mission's cell when its budget is spent by the precheck.
        if emergency {
            self.cells.engage_cohort();
            self.port.emergency_halt();
            #[cfg(test)]
            self.hooks.after_halt.fire();
        } else if self.budget_spent(id) {
            self.cells.engage(id);
            #[cfg(test)]
            self.hooks.after_halt.fire();
        }
        #[cfg(test)]
        self.hooks.after_precheck.fire();
        // A poisoned lock is a typed error here (the cells are engaged).
        let start = self.try_with_lock(|inner| -> Result<Start, CoordinatorError> {
            // A broken coordinator starts nothing and asks the broker nothing.
            inner.usable()?;
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            mission.not_held()?;
            // Rule 6: a mission whose File settlement began never runs again.
            if mission.settlement.is_some() {
                return Err(CoordinatorError::Stopped);
            }
            if mission.busy {
                return Err(CoordinatorError::Busy);
            }
            let from = mission.state;
            if from != MissionState::Approved
                && from != MissionState::AwaitingApproval
                && !from.is_working()
            {
                return Err(illegal(from, "advance"));
            }
            if !mission.consented() {
                return Err(CoordinatorError::NoConsent);
            }
            let mut report = Advance {
                from,
                to: from,
                lanes: vec![],
                effects: vec![],
                explanation: None,
                reason: None,
            };
            if emergency {
                return Ok(Start::Emergency(report));
            }
            if mission.out_of_time(&*self.clock) {
                // D1: found inside the lock: engaged before any write.
                self.cells.engage(id);
                return Ok(Start::OutOfTime(report));
            }
            let mono = self.clock.monotonic_ms();
            let roles: &'static [LaneRole] = match from {
                MissionState::Approved => {
                    report.to = inner.fire(id, Trigger::Start, mono)?;
                    return Ok(Start::Done(report));
                }
                MissionState::Explaining => {
                    let explanation = self.explain(id, inner.mission(id)?)?;
                    report.to = inner.fire(id, Trigger::PhaseDone, mono)?;
                    report.explanation = Some(explanation);
                    return Ok(Start::Finish(report));
                }
                other => phase_roles(other),
            };
            let mission = inner.mission(id)?;
            mission.busy = true;
            Ok(Start::Run(report, from, roles, mission.generation))
        })?;
        let (mut report, from, roles, generation) = match start {
            Start::Emergency(mut report) => {
                // A File that ran is reconciled first (HIGH 1): the mission
                // then ends Blocked, reported; otherwise it is stopped.
                let state = self.emergency(id)?;
                if state == MissionState::Stopped {
                    return Err(CoordinatorError::Stopped);
                }
                report.to = state;
                report.reason = self.reason(id);
                return Ok(report);
            }
            Start::OutOfTime(mut report) => {
                // Rule 4: through the barrier (a File settles; any other
                // mission is blocked `budget_time`, as before).
                report.to = self
                    .barrier(vec![(id, SettleCause::TimeBudget)])
                    .into_iter()
                    .find(|(mission, _)| *mission == id)
                    .map_or(Err(CoordinatorError::UnknownMission), |(_, state)| state)?;
                report.reason = self.reason(id);
                return Ok(report);
            }
            Start::Done(report) => return Ok(report),
            Start::Finish(report) => {
                self.port.finish(id);
                // Round 5 (C1): retired once its run is finished.
                self.cells.table.retire(&self.cells.journal, id.0);
                self.budgets.lock().remove(&id);
                return Ok(report);
            }
            Start::Run(report, from, roles, generation) => (report, from, roles, generation),
        };
        let _busy = Busy {
            coordinator: self,
            id,
        };

        if from == MissionState::AwaitingApproval {
            return self.resolve_wait(id, generation, report);
        }
        match self.run_lanes(id, generation, roles, &mut report)? {
            Ended::Ended(state) => {
                report.to = state;
                report.reason = self.reason(id);
                return Ok(report);
            }
            Ended::Through => {}
        }
        report.to = self.phase_done(id, generation, from)?;
        Ok(report)
    }

    /// The phase `phase` has run every lane of a reservation: on to the
    /// next state.
    fn phase_done(
        &self,
        id: MissionId,
        generation: u64,
        phase: MissionState,
    ) -> Result<MissionState, CoordinatorError> {
        self.with_lock(|inner| {
            let mission = inner.mission(id)?;
            if mission.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let trigger = if phase == MissionState::Verifying {
                Trigger::VerifyPass
            } else {
                Trigger::PhaseDone
            };
            let mono = self.clock.monotonic_ms();
            inner.fire(id, trigger, mono)
        })
    }

    /// Ask the broker what became of the action a reserved mission waits
    /// on (no lock held), and act on it if the reservation still holds:
    /// approved and run, the phase goes on from where it paused; denied,
    /// the mission is Denied; expired or otherwise lapsed, it is Blocked.
    /// The port is cancelled whenever the wait ends without the action.
    fn resolve_wait(
        &self,
        id: MissionId,
        generation: u64,
        mut report: Advance,
    ) -> Result<Advance, CoordinatorError> {
        let status = self.port.approval(id);
        let step = self.with_lock(|inner| {
            inner.kept(id)?;
            let mission = inner.mission(id)?;
            if mission.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let Some(waiting) = mission.waiting.clone() else {
                return Ok(Then::End(BlockReason::WaitUnknown));
            };
            // Round 4: the wait is the mission's unresolved File, and its
            // journaled identity (owed an acknowledgement once conclusive).
            let file = mission.unresolved_file == Some(waiting.effect);
            let open = mission
                .open_action
                .as_ref()
                .filter(|(e, _)| file && *e == waiting.effect)
                .map(|(_, pending)| pending.clone());
            Ok(Then::Go((waiting, mission.contract()?.clone(), file, open)))
        })?;
        let (waiting, contract, file, open) = match self.then(id, step)? {
            Ok(found) => found,
            Err(state) => {
                report.to = state;
                report.reason = self.reason(id);
                return Ok(report);
            }
        };
        let lapsed = |reason: LapseReason| Trigger::ApprovalLapsed {
            effect: waiting.effect,
            reason,
        };
        // Addendum 3, the ordinary path's dispositions for a File: a clean
        // publication goes on; one published with an interruption or with
        // recovery required, and one the broker cannot confirm either way,
        // end in one terminal line (`Trigger::Settled`); a denial, expiry or
        // lapse is taken only as the broker's confirmation that nothing was
        // published (the port reports nothing else so); a still valid wait
        // keeps waiting.
        let settled =
            |outcome: SettlementOutcome| self.settle_terminal(id, None, outcome, Some(generation));
        let end = match status {
            ApprovalStatus::Waiting => return Ok(report),
            ApprovalStatus::Approved => None,
            ApprovalStatus::Overtaken { causes } if file => {
                report.to = settled(SettlementOutcome::Published {
                    recovery_required: false,
                    interrupted: causes.iter().map(|c| PublicationCause::of(c)).collect(),
                })?;
                report.reason = self.reason(id);
                return Ok(report);
            }
            // D6: both recovery and the interruption codes are recorded.
            ApprovalStatus::RecoveryRequired { causes } if file => {
                report.to = settled(SettlementOutcome::Published {
                    recovery_required: true,
                    interrupted: causes.iter().map(|c| PublicationCause::of(c)).collect(),
                })?;
                report.reason = self.reason(id);
                return Ok(report);
            }
            ApprovalStatus::Uncertain if file => {
                report.to = settled(SettlementOutcome::Uncertain)?;
                report.reason = self.reason(id);
                return Ok(report);
            }
            ApprovalStatus::Lapsed { class }
                if file
                    && matches!(
                        BrokerClass::of(&class),
                        BrokerClass::ApprovalOutcomeUnknown
                            | BrokerClass::PortFenced
                            | BrokerClass::Unknown
                    ) =>
            {
                report.to = settled(SettlementOutcome::Uncertain)?;
                report.reason = self.reason(id);
                return Ok(report);
            }
            // Only a File is overtaken, recovery-required or uncertain.
            ApprovalStatus::Overtaken { .. }
            | ApprovalStatus::RecoveryRequired { .. }
            | ApprovalStatus::Uncertain => {
                Some(lapsed(LapseReason::ApprovalLapsed(BrokerClass::Unknown)))
            }
            ApprovalStatus::Denied => Some(Trigger::ActionDenied {
                effect: waiting.effect,
            }),
            ApprovalStatus::Expired => Some(lapsed(LapseReason::ApprovalExpired)),
            // A class outside the registry is `unknown`.
            ApprovalStatus::Lapsed { class } => {
                Some(lapsed(LapseReason::ApprovalLapsed(BrokerClass::of(&class))))
            }
        };
        if let Some(trigger) = end {
            // D3: a File's denial, expiry or lapse (Mother's own deadline
            // among them) is withdrawn first, then read once: only a File
            // confirmed not published takes the ordinary end.
            if file {
                let _ = self.port.withdraw(id);
                let read = self.port.publication(id);
                report.to = match read {
                    Ok(Some((_, Publication::ConfirmedNotPublished))) => {
                        self.end_plain(id, trigger, open)?
                    }
                    Ok(Some((pending, read))) => self.settle_terminal(
                        id,
                        Some(pending),
                        settlement_outcome(read),
                        Some(generation),
                    )?,
                    Ok(None) | Err(_) => self.settle_terminal(
                        id,
                        None,
                        SettlementOutcome::Uncertain,
                        Some(generation),
                    )?,
                };
                report.reason = self.reason(id);
                return Ok(report);
            }
            report.to = self.end_plain(id, trigger, open)?;
            report.reason = self.reason(id);
            return Ok(report);
        }

        // Approved, and it ran at the broker: the phase goes on.
        let generation = self.with_lock(|inner| {
            if inner.mission(id)?.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let mono = self.clock.monotonic_ms();
            report.to = inner.fire(
                id,
                Trigger::ActionApproved {
                    effect: waiting.effect,
                    phase: waiting.phase,
                },
                mono,
            )?;
            Ok::<_, CoordinatorError>(inner.mission(id)?.generation)
        })?;
        // Journaled: the broker may release the File's pin (rule 2).
        if let Some(pending) = &open {
            let _ = self.port.ack(id, pending);
        }
        // The approved action is final at the broker: its closure is
        // journaled before anything more is sent.
        if !self.close_action(id)? {
            report.to = self.end(id, BlockReason::ApprovalNotClosed)?;
            report.reason = self.reason(id);
            return Ok(report);
        }
        // The rest, from the private store (recovery loaded and verified it).
        let Some(rest) = waiting.rest.clone() else {
            report.to = self.end(id, BlockReason::WaitedRestUnreadable)?;
            report.reason = self.reason(id);
            return Ok(report);
        };
        let ended = match self.checked(id, &contract, &rest, &mut report)? {
            Ended::Through => {
                match self.dispatch(id, generation, &rest, &waiting.roles_left, &mut report)? {
                    Ended::Through => {
                        self.run_lanes(id, generation, &waiting.roles_left, &mut report)?
                    }
                    ended => ended,
                }
            }
            ended => ended,
        };
        if let Ended::Ended(state) = ended {
            report.to = state;
            report.reason = self.reason(id);
            return Ok(report);
        }
        report.to = self.phase_done(id, generation, waiting.phase)?;
        Ok(report)
    }

    /// Run `roles` in order for a reserved mission, and the effects they ask
    /// for. `generation` is the reservation: if the mission moved (a Stop),
    /// nothing more is called or dispatched.
    fn run_lanes(
        &self,
        id: MissionId,
        generation: u64,
        roles: &[LaneRole],
        report: &mut Advance,
    ) -> Result<Ended, CoordinatorError> {
        for (index, role) in roles.iter().enumerate() {
            let lane = self.lanes.get(*role);
            let identity = lane.identity();
            if self.port.emergency_stopped() {
                self.emergency(id)?;
                return Err(CoordinatorError::Stopped);
            }
            // Reserve the call.
            let step = self.with_lock(|inner| {
                let seq = inner.next_seq;
                let mission = inner.mission(id)?;
                if mission.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                let contract = mission.contract()?;
                let reason = if identity.role != *role {
                    Some(BlockReason::LaneIdentityChanged)
                } else if mission.model_calls >= contract.budget.max_model_calls {
                    Some(BlockReason::BudgetModelCalls)
                } else if mission.out_of_time(&*self.clock) {
                    // D1: found inside the lock: engaged before any write.
                    self.cells.engage(id);
                    Some(BlockReason::BudgetTime)
                } else {
                    None
                };
                if let Some(reason) = reason {
                    return Ok(Then::End(reason));
                }
                let input = LaneInput {
                    invocation: InvocationId { mission: id, seq },
                    role: *role,
                    contract_hash: mission.contract_ref.contract_hash,
                    owner_words: contract.owner_words.clone(),
                    restatement: contract.restatement.clone(),
                    pass_criteria: contract.pass_criteria.clone(),
                    prior_outputs: mission.prior_outputs.clone(),
                };
                inner.append(
                    id,
                    Event::LaneInvoked {
                        invocation: input.invocation,
                        role: identity.role,
                        identity_sha256: lane_identity_digest(&identity),
                        input_hash: input_hash(&input),
                    },
                )?;
                inner.mission(id)?.model_calls += 1;
                Ok(Then::Go(input))
            })?;
            let input = match self.then(id, step)? {
                Ok(input) => input,
                Err(state) => return Ok(Ended::Ended(state)),
            };

            let output = lane.invoke(&input);

            // Keep the answer only if the reservation still holds.
            let hash = output_hash(&output);
            let contract = self.with_lock(|inner| {
                inner.kept(id)?;
                if inner.mission(id)?.generation != generation {
                    inner.append(
                        id,
                        Event::LaneOvertaken {
                            invocation: input.invocation,
                        },
                    )?;
                    return Err(CoordinatorError::Stopped);
                }
                inner.append(
                    id,
                    Event::LaneReturned {
                        invocation: input.invocation,
                        output_hash: hash.clone(),
                    },
                )?;
                let mission = inner.mission(id)?;
                mission.prior_outputs.push(hash.clone());
                Ok(mission.contract()?.clone())
            })?;
            report.lanes.push(LaneReport {
                invocation: input.invocation,
                identity,
                text: output.text.clone(),
                verdict: output.verdict.clone(),
                input_hash: input_hash(&input),
                output_hash: hash,
            });

            // The verdict: a code; the lane's own reason is the Owner's
            // working detail (memory only).
            let verdict_end = match (&output.verdict, *role) {
                (Verdict::Halt { reason }, role) => Some((
                    Trigger::Block {
                        reason: BlockReason::LaneHalt(role),
                    },
                    Some(reason),
                )),
                (Verdict::Fail { reason }, LaneRole::Verifier) => Some((
                    Trigger::VerifyFail {
                        reason: VerifyFailReason::VerifierFailed,
                    },
                    Some(reason),
                )),
                (Verdict::Fail { reason }, role) => Some((
                    Trigger::Fail {
                        reason: FailReason::LaneFail(role),
                    },
                    Some(reason),
                )),
                (Verdict::Continue, LaneRole::Verifier) => Some((
                    Trigger::VerifyFail {
                        reason: VerifyFailReason::VerifierGaveNoVerdict,
                    },
                    None,
                )),
                _ => None,
            };
            if let Some((trigger, reason)) = verdict_end {
                if let Some(reason) = reason {
                    let detail =
                        WorkingDetail::new("lane", role.as_str()).and("lane reason", reason);
                    self.with_lock(|inner| inner.set_detail(id, detail));
                }
                return Ok(Ended::Ended(self.end_with(id, trigger)?));
            }

            // Mother's own checks of every effect, before any is sent.
            if output.effects.len() > MAX_EFFECTS_PER_CALL {
                return Ok(Ended::Ended(self.end(id, BlockReason::TooManyEffects)?));
            }
            if let Ended::Ended(state) = self.checked(id, &contract, &output.effects, report)? {
                return Ok(Ended::Ended(state));
            }
            if let Ended::Ended(state) =
                self.dispatch(id, generation, &output.effects, &roles[index + 1..], report)?
            {
                return Ok(Ended::Ended(state));
            }
        }
        Ok(Ended::Through)
    }

    /// Mother's own checks of `effects`, before any is sent: the first one
    /// she refuses blocks the mission.
    fn checked(
        &self,
        id: MissionId,
        contract: &MissionContract,
        effects: &[EffectRequest],
        report: &mut Advance,
    ) -> Result<Ended, CoordinatorError> {
        let workspace = self.port.workspace();
        for effect in effects {
            if let Some(reason) = mother_refuses(contract, effect, workspace.as_deref()) {
                self.with_lock(|inner| {
                    inner.append(
                        id,
                        Event::EffectRefused {
                            kind: effect.kind(),
                            reason,
                        },
                    )
                })?;
                report.effects.push(EffectReport {
                    kind: effect.kind(),
                    dispatched: false,
                    outcome: reason.to_string(),
                });
                return Ok(Ended::Ended(self.end(id, BlockReason::Refused(reason))?));
            }
        }
        Ok(Ended::Through)
    }

    /// Dispatch checked `effects`, one at a time. An effect that waits for
    /// the Owner's approval pauses the mission, keeping the rest of
    /// `effects` and the lanes `roles_left` for when it has run.
    fn dispatch(
        &self,
        id: MissionId,
        generation: u64,
        effects: &[EffectRequest],
        roles_left: &[LaneRole],
        report: &mut Advance,
    ) -> Result<Ended, CoordinatorError> {
        for (index, effect) in effects.iter().enumerate() {
            if self.port.emergency_stopped() {
                self.emergency(id)?;
                return Err(CoordinatorError::Stopped);
            }
            if effect.kind() == EffectKind::File {
                if let Ended::Ended(state) = self.admit_files(id, generation)? {
                    return Ok(Ended::Ended(state));
                }
            }
            let step = self.with_lock(|inner| {
                let mission = inner.mission(id)?;
                if mission.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                if mission.out_of_time(&*self.clock) {
                    // D1: found inside the lock: engaged before any write.
                    self.cells.engage(id);
                    return Ok(Then::End(BlockReason::BudgetTime));
                }
                let (number, lifetime) = (mission.next_effect, mission.lifetime);
                inner.append(
                    id,
                    Event::EffectDispatched {
                        effect: number,
                        kind: effect.kind(),
                        request_hash: request_hash(effect),
                    },
                )?;
                let mission = inner.mission(id)?;
                mission.next_effect += 1;
                mission.unsettled = Some(number);
                mission.dispatched_in_phase = true;
                if effect.kind() == EffectKind::File {
                    mission.unresolved_file = Some(number);
                }
                Ok(Then::Go((number, lifetime)))
            })?;
            let (number, lifetime) = match self.then(id, step)? {
                Ok(reserved) => reserved,
                Err(state) => return Ok(Ended::Ended(state)),
            };

            let outcome = self.port.perform(id, lifetime, effect);

            let settled = match &outcome {
                Ok(EffectOutcome::Done { .. }) => SettledOutcome::Done,
                Ok(EffectOutcome::AwaitingApproval) => SettledOutcome::AwaitingApproval,
                Err(refusal) => SettledOutcome::Refused(refusal.class),
            };
            let step = self.with_lock(|inner| {
                // M2a-S: after the break the outcome is discarded, not
                // journaled; the mission ends in memory.
                if let Err(e) = inner.kept(id) {
                    return Ok(Then::EndFail(BlockReason::JournalFailed, e));
                }
                let journaled = inner.append(
                    id,
                    Event::EffectSettled {
                        effect: number,
                        outcome: settled,
                    },
                );
                if let Err(e) = journaled {
                    // The journal failed: the mission ends in memory (the
                    // journal is poisoned) and the port is cancelled, which
                    // withdraws an action left waiting for approval.
                    return Ok(Then::EndFail(BlockReason::JournalFailed, e));
                }
                let mission = inner.mission(id)?;
                mission.unsettled = None;
                // A refused File was never committed at the broker.
                if !matches!(settled, SettledOutcome::AwaitingApproval)
                    && mission.unresolved_file == Some(number)
                {
                    mission.unresolved_file = None;
                }
                if mission.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                Ok(Then::Go(()))
            })?;
            if let Err(state) = self.then(id, step)? {
                return Ok(Ended::Ended(state));
            }
            report.effects.push(EffectReport {
                kind: effect.kind(),
                dispatched: true,
                outcome: match settled {
                    SettledOutcome::Refused(class) => class.to_string(),
                    other => other.to_string(),
                },
            });
            match outcome {
                Ok(EffectOutcome::Done { .. }) => {}
                Ok(EffectOutcome::AwaitingApproval) => {
                    let rest = &effects[index + 1..];
                    return Ok(Ended::Ended(
                        self.wait(id, generation, number, rest, roles_left)?,
                    ));
                }
                Err(refusal) => {
                    if self.port.emergency_stopped() {
                        self.emergency(id)?;
                        return Err(CoordinatorError::Stopped);
                    }
                    let reason = BlockReason::BrokerRefused(refusal.class);
                    return Ok(Ended::Ended(self.end(id, reason)?));
                }
            }
        }
        Ok(Ended::Through)
    }

    /// M2d (I7, §11 M3′): before a mission's first File effect in its
    /// consent lifetime, the port admits its run to the Owner's yes taken
    /// in this process (none after a restart: `files_consent_unverifiable`).
    /// Reserved under the lock, asked with none held, and kept only if the
    /// reservation and the lifetime still hold: a late answer revives
    /// nothing. Once admitted, later File effects of the lifetime (after a
    /// wait, in a later phase) are not admitted again.
    fn admit_files(&self, id: MissionId, generation: u64) -> Result<Ended, CoordinatorError> {
        let step = self.with_lock(|inner| {
            let mission = inner.mission(id)?;
            if mission.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            if mission.admitted == Some(mission.lifetime) {
                return Ok(Then::Go(None));
            }
            let Some(request) = mission.consent_request.clone() else {
                return Ok(Then::End(BlockReason::FilesConsentUnverifiable));
            };
            Ok(Then::Go(Some(Admission {
                contract_hash: mission.contract_ref.contract_hash.to_hex(),
                consent_request: request,
                generation: mission.lifetime,
            })))
        })?;
        let admission = match self.then(id, step)? {
            Ok(Some(admission)) => admission,
            Ok(None) => return Ok(Ended::Through),
            Err(state) => return Ok(Ended::Ended(state)),
        };
        let admitted = self.port.admit_files(id, &admission);
        let step = self.with_lock(|inner| {
            inner.kept(id)?;
            let mission = inner.mission(id)?;
            if mission.generation != generation || mission.lifetime != admission.generation {
                return Err(CoordinatorError::Stopped);
            }
            Ok(match admitted {
                Ok(()) => {
                    mission.admitted = Some(admission.generation);
                    Then::Go(())
                }
                Err(refusal) if refusal.class == BrokerClass::NoConsent => {
                    Then::End(BlockReason::FilesConsentUnverifiable)
                }
                Err(refusal) => Then::End(BlockReason::BrokerRefused(refusal.class)),
            })
        })?;
        Ok(match self.then(id, step)? {
            Ok(()) => Ended::Through,
            Err(state) => Ended::Ended(state),
        })
    }

    /// Effect `effect` waits for the Owner's approval: the mission waits,
    /// keeping what its phase had still to do. If the journal cannot
    /// record the wait, the mission ends (in memory, the journal poisoned)
    /// and the port is cancelled, which withdraws the pending action.
    /// P0: the rest is put in the private store first (no lock held); the
    /// journal names it. If it cannot be kept, the mission ends the same way.
    fn wait(
        &self,
        id: MissionId,
        generation: u64,
        effect: u64,
        rest: &[EffectRequest],
        roles_left: &[LaneRole],
    ) -> Result<MissionState, CoordinatorError> {
        let bytes = serde_json::to_vec(rest).expect("effect requests serialise");
        let content = match self.private.put(ContentKind::PausedPhase, &bytes) {
            Ok(content) => content,
            Err(e) => {
                let _ = self.end(id, BlockReason::AwaitingOwnerApproval);
                return Err(CoordinatorError::Private(e));
            }
        };
        let trigger = Trigger::AwaitApproval {
            effect,
            rest: RestRef {
                content,
                count: rest.len() as u64,
                bytes: bytes.len() as u64,
            },
            roles_left: roles_left.to_vec(),
        };
        // The action's identity first, so that a rebuilt port can still
        // withdraw it.
        if let Some(pending) = self.port.pending(id) {
            let step = self.with_lock(|inner| {
                if let Err(e) = inner.kept(id) {
                    return Ok(Then::EndFail(BlockReason::AwaitingOwnerApproval, e));
                }
                if inner.mission(id)?.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                let journaled = inner.append(
                    id,
                    Event::ApprovalPending {
                        effect,
                        pending: pending.clone(),
                    },
                );
                if let Err(e) = journaled {
                    return Ok(Then::EndFail(BlockReason::AwaitingOwnerApproval, e));
                }
                inner.mission(id)?.open_action = Some((effect, pending));
                Ok(Then::Go(()))
            })?;
            if let Err(state) = self.then(id, step)? {
                return Ok(state);
            }
        }
        let mono = self.clock.monotonic_ms();
        let waited = self.with_lock(|inner| {
            if inner.mission(id)?.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let state = inner.fire(id, trigger, mono)?;
            if let Some(waiting) = inner.mission(id)?.waiting.as_mut() {
                waiting.rest = Some(rest.to_vec());
            }
            Ok(state)
        });
        if let Err(CoordinatorError::Journal(e)) = waited {
            let _ = self.end(id, BlockReason::AwaitingOwnerApproval);
            return Err(CoordinatorError::Journal(e));
        }
        waited
    }

    /// End a mission for `reason` (Blocked) and cancel what it opened at
    /// the port; a waiting mission's wait lapses (which withdraws the
    /// pending action). Round 4 (rule 7, addendum 2): a mission whose File
    /// is unresolved ends only through its settlement.
    fn end(&self, id: MissionId, reason: BlockReason) -> Result<MissionState, CoordinatorError> {
        if self.file_unresolved(id) {
            return self.settle_one(id, SettleCause::Ended(reason));
        }
        self.end_plain_block(id, reason)
    }

    /// [`Self::end`] of a mission with no unresolved File.
    fn end_plain_block(
        &self,
        id: MissionId,
        reason: BlockReason,
    ) -> Result<MissionState, CoordinatorError> {
        let trigger = self.with_lock(|inner| {
            let mission = inner.mission(id)?;
            Ok::<_, CoordinatorError>(match (&mission.waiting, mission.state) {
                (Some(waiting), MissionState::AwaitingApproval) => Trigger::ApprovalLapsed {
                    effect: waiting.effect,
                    reason: LapseReason::Ended(reason),
                },
                _ => Trigger::Block { reason },
            })
        })?;
        self.end_plain(id, trigger, None)
    }

    /// End a mission with `trigger` (Block, Fail, VerifyFail, or the end of
    /// a wait) and cancel what it opened at the port. Round 4: a mission
    /// whose File is unresolved ends only through its settlement.
    fn end_with(&self, id: MissionId, trigger: Trigger) -> Result<MissionState, CoordinatorError> {
        if self.file_unresolved(id) {
            let cause = match &trigger {
                Trigger::Stop { reason } => SettleCause::Stop(*reason),
                Trigger::Block { reason }
                | Trigger::ApprovalLapsed {
                    reason: LapseReason::Ended(reason),
                    ..
                } => SettleCause::Ended(*reason),
                _ => SettleCause::Ended(BlockReason::PhaseInterrupted),
            };
            return self.settle_one(id, cause);
        }
        self.end_plain(id, trigger, None)
    }

    /// Whether the mission, unfinished, has a File not yet resolved.
    fn file_unresolved(&self, id: MissionId) -> bool {
        self.peek(|inner| {
            inner
                .missions
                .get(&id)
                .is_some_and(|m| !m.state.is_terminal() && m.unresolved_file.is_some())
        })
    }

    /// One mission's settlement for an end that is not a Stop (no door
    /// halt: [`Self::barrier`] with `halt` false).
    fn settle_one(
        &self,
        id: MissionId,
        cause: SettleCause,
    ) -> Result<MissionState, CoordinatorError> {
        self.barrier(vec![(id, cause)])
            .into_iter()
            .find(|(mission, _)| *mission == id)
            .map_or(Err(CoordinatorError::UnknownMission), |(_, state)| state)
    }

    /// The end itself: journal `trigger` (in memory if the journal fails),
    /// then withdraw and close. `ack`: the File action whose conclusive
    /// end this is (a denial or lapse confirmed not published), acknowledged
    /// once the end is journaled.
    fn end_plain(
        &self,
        id: MissionId,
        trigger: Trigger,
        ack: Option<PendingAction>,
    ) -> Result<MissionState, CoordinatorError> {
        let mono = self.clock.monotonic_ms();
        let result = self.with_lock(|inner| {
            let from = inner.mission(id)?.state;
            let result = inner.fire(id, trigger.clone(), mono);
            if let (Err(CoordinatorError::Journal(_)), Ok(to)) =
                (&result, transition(from, &trigger))
            {
                // The journal failed: the mission ends in memory all the
                // same (and the journal, poisoned, will not reopen; if
                // poisoning failed too, it reopens with the mission held).
                inner.mission(id)?.apply(&trigger, to, mono);
            }
            Ok::<_, CoordinatorError>(result)
        })?;
        // Whatever the journal said, nothing more of it may start; a
        // withdrawal or closure that failed is an error too (an action not
        // confirmed closed stays open in the journal and is withdrawn again
        // on every reopen).
        let withdrawn = self.port.withdraw(id);
        if let Some(pending) = ack.filter(|_| result.is_ok()) {
            let _ = self.port.ack(id, &pending);
        }
        let closed = self.close_action(id);
        let state = result?;
        withdrawn.map_err(|r| CoordinatorError::Withdrawal(r.class))?;
        closed?;
        Ok(state)
    }

    /// Journal the mission's open action closed, once the port confirms
    /// the broker no longer holds it open. `Ok(false)`: not confirmed.
    fn close_action(&self, id: MissionId) -> Result<bool, CoordinatorError> {
        let open = self.with_lock(|inner| inner.mission(id).map(|m| m.open_action.clone()))?;
        let Some((effect, pending)) = open else {
            return Ok(true);
        };
        if !self.port.settled(&pending) {
            return Ok(false);
        }
        self.with_lock(|inner| {
            if inner.mission(id)?.open_action.as_ref().map(|(e, _)| *e) == Some(effect) {
                inner.append(id, Event::ApprovalClosed { effect })?;
                inner.mission(id)?.open_action = None;
            }
            Ok(true)
        })
    }

    /// Every journaled action not journaled closed, with its mission and
    /// the mission's state.
    fn open_actions(&self) -> Vec<(MissionId, MissionState, PendingAction)> {
        self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter_map(|(id, m)| {
                    m.open_action
                        .as_ref()
                        .map(|(_, p)| (*id, m.state, p.clone()))
                })
                .collect()
        })
    }

    /// On open, before anything is journaled (M2a-S: so that an open that
    /// fails at its first write can still withdraw them): every journaled
    /// action not journaled closed, whatever its mission's state, is handed
    /// back to the port. Claims are compared by parsed identity: an action
    /// two missions name (a tampered journal) is handed back to neither.
    fn adopt_actions(&self) {
        // D4(c): an action whose conclusive outcome is journaled (an
        // ordinary approval or a settlement, its acknowledgement or closure
        // left undone) is never adopted: it is only handed back, and the
        // journal's acknowledgement (ack-only ownership) settles it.
        let conclusive: Vec<PendingAction> = self.peek(|inner| {
            inner
                .missions
                .values()
                .flat_map(|m| m.acks.iter().cloned())
                .collect()
        });
        let open: Vec<(MissionId, MissionState, PendingAction)> = self
            .open_actions()
            .into_iter()
            .filter(|(id, _, pending)| {
                if conclusive.contains(pending) {
                    self.port.hand(*id, pending);
                    return false;
                }
                true
            })
            .collect();
        let key = |p: &PendingAction| (RunId::parse(&p.run), CommitmentId::parse(&p.commitment));
        for (id, _, pending) in &open {
            let (run, commitment) = key(pending);
            let claims = open
                .iter()
                .filter(|(_, _, other)| {
                    let (r, c) = key(other);
                    (run.is_some() && r == run) || (commitment.is_some() && c == commitment)
                })
                .count();
            if claims == 1 {
                self.port.adopt(*id, pending);
            } else {
                // Given back to neither, but recorded: withdrawing it is an
                // error while the broker still holds it open.
                self.port.hand(*id, pending);
            }
        }
    }

    /// M2d-C round 6 (X2″), right after the adoptions: every replayed
    /// reference to a run that survives at the broker must carry this
    /// journal's cell for its mission (the adoption attached it, or the run
    /// was opened with it); numeric mission ids prove nothing. One that
    /// does not (its adoption refused by D4(a): a surviving execution of
    /// this journal's mission under a replacement's fresh cell, review 5
    /// HIGH 1) is stopped at the broker (its own cells engaged `Fence`, the
    /// run cancelled), and the open fails closed: the port's fence, then a
    /// typed error, adopting nothing more. An unreadable broker fails the
    /// same way.
    fn bind_surviving(&self) -> Result<(), CoordinatorError> {
        let references: Vec<(MissionId, RunId)> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .filter_map(|(id, m)| {
                    let (_, pending) = m.open_action.as_ref()?;
                    RunId::parse(&pending.run).map(|run| (*id, run))
                })
                .collect()
        });
        for (id, run) in references {
            let Some(cell) = self.cells.table.get(&self.cells.journal, id.0) else {
                continue;
            };
            let bound = match self.port.run_survives(run) {
                Ok(false) => continue,
                Ok(true) => self.port.run_carries(run, &cell),
                Err(e) => Err(e),
            };
            match bound {
                Ok(Some(true)) | Ok(None) => {}
                Ok(Some(false)) => {
                    let _ = self.port.stop_surviving(run);
                    self.port.fence().write();
                    return Err(CoordinatorError::Identity(
                        IdentityRefusal::SurvivingRunMismatch,
                    ));
                }
                Err(_) => {
                    self.port.fence().write();
                    return Err(CoordinatorError::Identity(IdentityRefusal::Unreadable));
                }
            }
        }
        Ok(())
    }

    /// On open: every journaled action not journaled closed (handed back
    /// already) whose mission does not wait is withdrawn (the mission's run
    /// is cancelled) and, once the port confirms it, journaled closed.
    /// Every one is withdrawn first; only then is any closure journaled, so
    /// that a journal failure cannot leave a later action unwithdrawn.
    fn recover_actions(&self) -> Result<(), CoordinatorError> {
        let open = self.open_actions();
        let mut first_error = None;
        for (id, state, _) in &open {
            if *state != MissionState::AwaitingApproval {
                if let Err(refusal) = self.port.withdraw(*id) {
                    first_error.get_or_insert(CoordinatorError::Withdrawal(refusal.class));
                }
            }
        }
        for (id, state, _) in &open {
            if *state != MissionState::AwaitingApproval {
                if let Err(e) = self.close_action(*id) {
                    first_error.get_or_insert(e);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Recovery, the one place: everything opening does after the journal
    /// was replayed. Fail-closed: any error in it, and any journal append
    /// that failed in it even if a step did not report it, is returned, and
    /// `open` then withdraws every open request (waiting ones included).
    ///
    /// M2a-S: on a journal whose last session closed cleanly, this
    /// session's opening is journaled first, so that a record lost later
    /// cannot leave the journal ending in that clean close. After a session
    /// that did not close cleanly, every mission still unfinished once the
    /// rest of recovery is done is held (journaled with the session's
    /// opening, before its next record or its clean close). A held waiting
    /// mission's request is handed back as for any waiting mission: Mother
    /// never approves it, and the Owner's Stop withdraws it.
    fn recover(&self, closed_cleanly: bool, clean: bool) -> Result<(), CoordinatorError> {
        self.adopt_actions();
        self.bind_surviving()?;
        if closed_cleanly {
            self.with_lock(|inner| inner.open_session_at_once())?;
        }
        self.recover_and_settle()?;
        self.load_content()?;
        if !clean {
            self.hold_unfinished();
        }
        if self.peek(|inner| inner.broken) {
            return Err(CoordinatorError::Journal(JournalError::Store(
                StoreFailure::FailedDuringRecovery,
            )));
        }
        Ok(())
    }

    /// P0: the content of every mission still unfinished (held ones too):
    /// its contract (verified: content id, size, parse, hash, bounds, and
    /// every question answered once submitted) and a waiting mission's rest
    /// (content id, size, parse, count). Content that does not verify stops
    /// its mission: its journaled requests are withdrawn first, then the
    /// Stop is journaled, then its open action is closed if the port
    /// confirms it. Nothing in the content restores a consent, an approval
    /// or a resume: those were replayed from the journal.
    fn load_content(&self) -> Result<(), CoordinatorError> {
        type Wanted = (MissionId, MissionState, ContractRef, Option<RestRef>);
        let wanted: Vec<Wanted> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .map(|(id, m)| {
                    (
                        *id,
                        m.state,
                        m.contract_ref.clone(),
                        m.waiting.as_ref().map(|w| w.rest_ref.clone()),
                    )
                })
                .collect()
        });
        let mut first_error = None;
        for (id, state, contract_ref, rest_ref) in wanted {
            let loaded = self.fetch_contract(&contract_ref).and_then(|contract| {
                if state != MissionState::Draft && contract.validate_for_consent().is_err() {
                    return Err(PrivateError::Mismatch);
                }
                let rest = rest_ref.map(|r| self.fetch_rest(&r)).transpose()?;
                Ok((contract, rest))
            });
            match loaded {
                Ok((contract, rest)) => {
                    self.with_lock(|inner| {
                        let mission = inner.mission(id)?;
                        mission.contract = Some(contract);
                        if let Some(waiting) = mission.waiting.as_mut() {
                            waiting.rest = rest;
                        }
                        Ok::<_, CoordinatorError>(())
                    })?;
                }
                Err(_) if self.file_unresolved(id) => {
                    // Round 4: through its settlement, never an ordinary Stop.
                    let stop = SettleCause::Stop(StopReason::PrivateContentUnavailable);
                    if let Err(e) = self.settle_one(id, stop) {
                        first_error.get_or_insert(e);
                    }
                }
                Err(_) => {
                    // Withdraw first: if the Stop cannot be journaled, nothing
                    // of the mission is left approvable at the broker.
                    if let Err(refusal) = self.port.withdraw(id) {
                        first_error.get_or_insert(CoordinatorError::Withdrawal(refusal.class));
                    }
                    let mono = self.clock.monotonic_ms();
                    let stopped = self.with_lock(|inner| {
                        inner.fire(
                            id,
                            Trigger::Stop {
                                reason: StopReason::PrivateContentUnavailable,
                            },
                            mono,
                        )
                    });
                    if let Err(e) = stopped {
                        first_error.get_or_insert(e);
                    }
                    if let Err(e) = self.close_action(id) {
                        first_error.get_or_insert(e);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// The rest of opening, after the journal was replayed: recover the
    /// open actions, abandon unanswered lane calls, block interrupted
    /// missions.
    fn recover_and_settle(&self) -> Result<(), CoordinatorError> {
        self.recover_actions()?;
        // A lane call left unanswered by the restart is abandoned.
        self.with_lock(|inner| {
            let pending: Vec<(MissionId, InvocationId)> = inner
                .missions
                .iter()
                .filter_map(|(id, m)| m.pending_lane.map(|inv| (*id, inv)))
                .collect();
            for (id, invocation) in pending {
                inner.append(id, Event::LaneOvertaken { invocation })?;
                inner.mission(id)?.pending_lane = None;
            }
            Ok::<_, CoordinatorError>(())
        })?;
        // Round 4 (rule 6): a settlement begun before the restart cannot read
        // the broker as it was: it ends uncertain (never "not published",
        // unless Mother's own journal shows the port refused the File).
        let begun: Vec<MissionId> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal() && m.settlement.is_some())
                .map(|(id, _)| *id)
                .collect()
        });
        for id in begun {
            let _ = self.port.withdraw(id);
            self.settle_terminal(id, None, SettlementOutcome::Uncertain, None)?;
        }
        // Addendum 2: every conclusive File outcome journaled is acknowledged
        // again (idempotent at the broker); an uncertain one never is.
        let owed: Vec<(MissionId, PendingAction)> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .flat_map(|(id, m)| m.acks.iter().map(|p| (*id, p.clone())))
                .collect()
        });
        for (id, pending) in owed {
            let _ = self.port.ack(id, &pending);
        }
        // A mission interrupted after dispatching an effect in its current
        // phase is blocked: an unsettled effect may have happened, and
        // re-running the phase would repeat a settled one.
        let interrupted: Vec<(MissionId, BlockReason)> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .filter_map(|(id, m)| {
                    if m.unsettled.is_some() {
                        Some((*id, BlockReason::EffectOutcomeUnknown))
                    } else if m.dispatched_in_phase {
                        Some((*id, BlockReason::PhaseInterrupted))
                    } else {
                        None
                    }
                })
                .collect()
        });
        for (id, reason) in interrupted {
            self.end(id, reason)?;
        }
        Ok(())
    }

    /// M2a-S: hold every unfinished mission not held already. A Stop or
    /// denial the journal lost would have ended it; only the Owner decides.
    /// The holds are journaled with the session's opening.
    fn hold_unfinished(&self) {
        self.with_lock(|inner| {
            for mission in inner.missions.values_mut() {
                if !mission.state.is_terminal() && mission.held.is_none() {
                    // A new hold: journaled with the session's opening, even if
                    // an earlier hold (since resumed) was journaled.
                    mission.held = Some(HeldReason::UncleanShutdown);
                    mission.held_journaled = false;
                }
            }
        })
    }

    /// Opening failed: withdraw every mission's journaled open action
    /// (handed back to the port already), whatever its state. Every one is
    /// attempted; their errors are not returned, because opening already
    /// returns the first error (and the journal stays as it is: a failed
    /// append poisoned it, so it opens nothing until the Owner reconciles
    /// it; if poisoning failed too, it reopens with every unfinished
    /// mission held).
    fn withdraw_open_actions(&self) {
        let open: Vec<MissionId> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| m.open_action.is_some())
                .map(|(id, _)| *id)
                .collect()
        });
        for id in open {
            let _ = self.port.withdraw(id);
        }
    }

    fn explain(&self, id: MissionId, mission: &Mission) -> Result<String, CoordinatorError> {
        let contract = mission.contract()?;
        let mut lines = vec![
            format!("Mission {id} is done."),
            format!("You asked: {}", contract.owner_words.trim()),
            format!("I understood: {}", contract.restatement),
            "The Verifier checked every pass criterion you set before work began:".into(),
        ];
        for criterion in &contract.pass_criteria {
            lines.push(format!("  - {}: holds", criterion.text));
        }
        lines.push(format!(
            "Lane calls: {} of {}. Effects sent to the broker: {}. Money spent: 0.",
            mission.model_calls, contract.budget.max_model_calls, mission.next_effect
        ));
        Ok(lines.join("\n"))
    }

    /// Stop a mission from whatever state it is in; a terminal mission
    /// stays as it ended. Never waits for a lane or an effect.
    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.stop_for(id, StopReason::Owner)
    }

    /// Round 5 (tests): this journal's identity (its cells' key).
    #[cfg(test)]
    pub(crate) fn journal_identity(&self) -> &str {
        &self.cells.journal
    }

    /// D1's precheck: whether the mission's budget is spent, read outside
    /// the coordinator lock (no reference: not known here, and the in-lock
    /// check decides).
    fn budget_spent(&self, id: MissionId) -> bool {
        let reference = self.budgets.lock().get(&id).copied();
        reference.is_some_and(|r| r.spent(&*self.clock))
    }

    /// Tests: poison the coordinator's lock (a panic while it was held).
    #[cfg(test)]
    pub(crate) fn poison_for_test(&self) {
        std::thread::scope(|scope| {
            let _ = scope
                .spawn(|| {
                    let _held = self.inner.lock();
                    panic!("poisoning the coordinator (a test)");
                })
                .join();
        });
    }

    fn stop_for(
        &self,
        id: MissionId,
        reason: StopReason,
    ) -> Result<MissionState, CoordinatorError> {
        // D1 / H3: the mission's cell first, before any lock or write.
        self.cells.engage(id);
        #[cfg(test)]
        self.hooks.after_halt.fire();
        self.barrier(vec![(id, SettleCause::Stop(reason))])
            .into_iter()
            .find(|(mission, _)| *mission == id)
            .map_or(Err(CoordinatorError::UnknownMission), |(_, state)| state)
    }

    /// M2d-C round 5 (settlement design v2, rules 4-5; D1, H3, C2): the
    /// stopping of `targets`, whose halt cells their callers engaged first
    /// (before any lock or write; engaged again here, idempotently):
    /// 1. each mission's first step under the lock: terminal already, or
    ///    its settlement already begun (the first cause stands), it is left
    ///    as it is; one with an unresolved File journals `SettlementStarted`
    ///    (the journal's own failure machinery: a failed append still leaves
    ///    it settling, in memory) and is never runnable again; any other is
    ///    stopped (or, for the time budget, blocked) as before;
    /// 2. only then, every settling mission is withdrawn, and then read
    ///    once and ended (`reconcile`). Nothing waits.
    ///
    /// Only `targets` are stopped (C4): other missions stay runnable. A
    /// poisoned coordinator lock is a typed error for each, never a panic.
    fn barrier(
        &self,
        targets: Vec<(MissionId, SettleCause)>,
    ) -> Vec<(MissionId, Result<MissionState, CoordinatorError>)> {
        for (id, _) in &targets {
            self.cells.engage(*id);
        }
        let all = targets;
        let mut results = Vec::with_capacity(all.len());
        let mut settling = Vec::new();
        for (id, cause) in all {
            match self.stop_step(id, cause) {
                Ok(Stepped::Settle) => settling.push(id),
                Ok(Stepped::Done(state)) => results.push((id, Ok(state))),
                Ok(Stepped::Stopped(journaled, may_have_dispatched)) => {
                    results.push((id, self.after_stop(id, journaled, may_have_dispatched)))
                }
                Ok(Stepped::End(reason)) => results.push((id, self.end_plain_block(id, reason))),
                Err(e) => results.push((id, Err(e))),
            }
        }
        let withdrawn: Vec<(MissionId, Result<(), CoordinatorError>)> = settling
            .iter()
            .map(|id| {
                let withdrawn = self.port.withdraw(*id);
                (
                    *id,
                    withdrawn.map_err(|r| CoordinatorError::Withdrawal(r.class)),
                )
            })
            .collect();
        for (id, withdrawn) in withdrawn {
            let state = self
                .reconcile(id)
                .and_then(|state| withdrawn.map(|()| state));
            results.push((id, state));
        }
        results
    }

    /// Step 2 of [`Self::barrier`], in one hold of the lock.
    fn stop_step(&self, id: MissionId, cause: SettleCause) -> Result<Stepped, CoordinatorError> {
        self.try_with_lock(|inner| {
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            if mission.state.is_terminal() {
                // M2a-S: on a broken coordinator a terminal state may be in
                // memory only (a Stop or denial the journal lost): never Ok.
                inner.usable()?;
                return Ok(Stepped::Done(inner.mission(id)?.state));
            }
            if mission.settlement.is_some() {
                return Ok(Stepped::Done(mission.state));
            }
            if let Some(effect) = mission.unresolved_file {
                // The existing machinery: a failed append fences the port and
                // poisons the journal; the mission settles in memory anyway.
                let _ = inner.append(id, Event::SettlementStarted { effect, cause });
                let mission = inner.mission(id)?;
                mission.settlement = Some((effect, cause));
                mission.generation += 1;
                return Ok(Stepped::Settle);
            }
            let reason = match cause {
                SettleCause::Stop(reason) => reason,
                SettleCause::TimeBudget => return Ok(Stepped::End(BlockReason::BudgetTime)),
                SettleCause::Ended(reason) => return Ok(Stepped::End(reason)),
            };
            let may_have_dispatched = mission.approved_wall_ms.is_some();
            let mono = self.clock.monotonic_ms();
            let journaled = inner.fire(id, Trigger::Stop { reason }, mono);
            if journaled.is_err() {
                // The journal failed: the mission stops in memory all the same.
                let mission = inner.mission(id)?;
                mission.state = MissionState::Stopped;
                mission.generation += 1;
                inner.after_transition(id, &Trigger::Stop { reason }, MissionState::Stopped);
            }
            Ok(Stepped::Stopped(journaled, may_have_dispatched))
        })
    }

    /// An ordinary Stop's withdrawal and closure, as before.
    fn after_stop(
        &self,
        id: MissionId,
        journaled: Result<MissionState, CoordinatorError>,
        may_have_dispatched: bool,
    ) -> Result<MissionState, CoordinatorError> {
        if may_have_dispatched {
            let withdrawn = self.port.withdraw(id);
            let closed = self.close_action(id);
            let state = journaled?;
            withdrawn.map_err(|r| CoordinatorError::Withdrawal(r.class))?;
            closed?;
            return Ok(state);
        }
        journaled
    }

    /// Rule 5: one snapshot of the settling mission's File, with no lock
    /// held, then its terminal line. Published: the truthful success and
    /// the cause's block; confirmed not published (by the broker, or by
    /// Mother's own journal: the port refused it): the cause's end; pending,
    /// uncertain, unknown or unreadable: `file_publication_uncertain`, the
    /// pin kept.
    fn reconcile(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let (action, outcome) = match self.port.publication(id) {
            Ok(Some((pending, read))) => (Some(pending), settlement_outcome(read)),
            Ok(None) | Err(_) => (None, SettlementOutcome::Uncertain),
        };
        self.settle_terminal(id, action, outcome, None)
    }

    /// Rules 5-6, addendum 2: the File's terminal line (`Trigger::Settled`:
    /// its outcome and the mission's end, one journal line), in one hold of
    /// the lock. `reserved`: the ordinary path's wait reservation (the
    /// broker's own answer; the end follows from the outcome); `None`: a
    /// settlement (the end follows from its first cause). A journal that
    /// fails ends the mission in memory and acknowledges nothing; otherwise
    /// a conclusive outcome is acknowledged. Then the closure, as for any
    /// end.
    fn settle_terminal(
        &self,
        id: MissionId,
        action: Option<PendingAction>,
        outcome: SettlementOutcome,
        reserved: Option<u64>,
    ) -> Result<MissionState, CoordinatorError> {
        let mono = self.clock.monotonic_ms();
        let (result, owed) = self.with_lock(|inner| {
            let mission = inner.mission(id)?;
            if mission.state.is_terminal() {
                return Err(CoordinatorError::Stopped);
            }
            let (effect, outcome, end) = match (mission.settlement, reserved) {
                (Some((effect, cause)), None) => {
                    // Mother's own journal proves the port refused it.
                    let outcome = if mission.unresolved_file != Some(effect) && action.is_none() {
                        SettlementOutcome::NotPublished
                    } else {
                        outcome
                    };
                    let end = cause.end(&outcome);
                    (effect, outcome, end)
                }
                (None, Some(generation)) => {
                    if mission.generation != generation {
                        return Err(CoordinatorError::Stopped);
                    }
                    let effect = mission
                        .waiting
                        .as_ref()
                        .map(|w| w.effect)
                        .filter(|e| mission.unresolved_file == Some(*e))
                        .ok_or(CoordinatorError::Stopped)?;
                    // D3: a clean publication found after a deadline ends
                    // `approval_deadline`.
                    let end = outcome
                        .ordinary_end()
                        .or_else(|| outcome.deadline_end())
                        .ok_or(CoordinatorError::Stopped)?;
                    (effect, outcome, end)
                }
                _ => return Err(CoordinatorError::Stopped),
            };
            // The journaled identity, when there is one.
            let action = match mission.open_action.as_ref() {
                Some((e, pending)) if *e == effect => Some(pending.clone()),
                _ => action,
            };
            let trigger = Trigger::Settled {
                effect,
                action: action.clone(),
                outcome: outcome.clone(),
                end,
            };
            let from = mission.state;
            let result = inner.fire_settling(id, trigger.clone(), mono);
            if let (Err(CoordinatorError::Journal(_)), Ok(to)) =
                (&result, transition(from, &trigger))
            {
                inner.mission(id)?.apply(&trigger, to, mono);
            }
            let owed = action.filter(|_| result.is_ok() && outcome.conclusive());
            Ok((result, owed))
        })?;
        let withdrawn = match reserved {
            Some(_) => self.port.withdraw(id),
            None => Ok(()),
        };
        if let Some(pending) = owed {
            // A failed acknowledgement is kept by the port and tried again;
            // the journal's record is tried again on every reopen.
            let _ = self.port.ack(id, &pending);
        }
        let closed = self.close_action(id);
        let state = result?;
        withdrawn.map_err(|r| CoordinatorError::Withdrawal(r.class))?;
        closed?;
        Ok(state)
    }

    /// Stop every mission that has not ended, through one barrier. Returns
    /// how many it stopped or (a File's settlement) blocked.
    pub fn stop_all(&self) -> usize {
        self.stop_everything().unwrap_or(0)
    }

    /// Round 5 (C2, D1): [`Self::stop_all`] with its typed error. ONE
    /// snapshot of this journal's cells (the cohort) is taken under the halt
    /// table's lock, engaged once it is released, and exactly that cohort
    /// is then stopped: a mission registered after the snapshot is ordered
    /// after this call and is not stopped. A poisoned coordinator lock is
    /// `Poisoned` once the cells are engaged, never a panic.
    pub fn stop_everything(&self) -> Result<usize, CoordinatorError> {
        let cohort = self.cells.engage_cohort();
        #[cfg(test)]
        self.hooks.after_halt.fire();
        let targets = cohort
            .into_iter()
            .map(|id| (id, SettleCause::Stop(StopReason::Owner)))
            .collect();
        let results = self.barrier(targets);
        if results
            .iter()
            .any(|(_, r)| matches!(r, Err(CoordinatorError::Poisoned)))
        {
            return Err(CoordinatorError::Poisoned);
        }
        Ok(results
            .into_iter()
            .filter(|(_, state)| matches!(state, Ok(MissionState::Stopped | MissionState::Blocked)))
            .count())
    }

    /// Round 5 (C2, C4): the Owner's emergency stop, found by `id`'s call:
    /// this journal's cohort's cells and the door's memory stop first, then
    /// exactly that cohort is stopped; `id`'s result.
    fn emergency(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let cohort = self.cells.engage_cohort();
        self.port.emergency_halt();
        #[cfg(test)]
        self.hooks.after_halt.fire();
        let mut targets: Vec<(MissionId, SettleCause)> = cohort
            .into_iter()
            .map(|m| (m, SettleCause::Stop(StopReason::EmergencyStop)))
            .collect();
        if !targets.iter().any(|(m, _)| *m == id) {
            targets.push((id, SettleCause::Stop(StopReason::EmergencyStop)));
        }
        self.barrier(targets)
            .into_iter()
            .find(|(mission, _)| *mission == id)
            .map_or(Err(CoordinatorError::UnknownMission), |(_, state)| state)
    }

    /// M2d-C round 4 (addendum 5): what the Owner is told of a mission whose
    /// File's outcome is uncertain, exactly; `None` otherwise.
    pub fn notice(&self, id: MissionId) -> Option<&'static str> {
        let uncertain = BlockReason::FilePublicationUncertain.as_str();
        (self.reason(id).as_deref() == Some(uncertain.as_str())).then_some(FILE_UNCERTAIN_NOTICE)
    }

    /// M2a-S: shut down in order, journaling the clean close that lets the
    /// next open resume every mission. Refused (and the next open holds
    /// every unfinished mission) if the journal failed in this session,
    /// while a call is in flight, or if the close cannot be journaled.
    pub fn close(self) -> Result<(), CoordinatorError> {
        self.close_ref()
    }

    /// [`Coordinator::close`] through a shared handle: once it succeeds,
    /// the coordinator journals and acknowledges nothing more (every call
    /// that would is refused with [`CoordinatorError::Closed`]), so nothing
    /// can be lost behind the clean close.
    pub(crate) fn close_ref(&self) -> Result<(), CoordinatorError> {
        self.with_lock(|inner| {
            if inner.closed {
                return Err(CoordinatorError::Closed);
            }
            if inner.missions.values().any(|m| m.busy) {
                return Err(CoordinatorError::Busy);
            }
            // A clean close vouches for the journal this coordinator wrote: it
            // is refused if the journal changed under it (another writer).
            let lines = inner.store.load().map_err(CoordinatorError::Journal)?;
            let head = match lines.last() {
                None => GENESIS.to_string(),
                Some(line) => serde_json::from_str::<Entry>(line)
                    .map(|e| e.hash)
                    .map_err(|_| {
                        CoordinatorError::Journal(JournalError::Malformed { line: lines.len() })
                    })?,
            };
            if head != inner.head || lines.len() as u64 != inner.next_seq {
                return Err(CoordinatorError::Journal(JournalError::Store(
                    StoreFailure::ChangedUnderCoordinator,
                )));
            }
            inner.append(SESSION, Event::SessionClosed)?;
            inner.closed = true;
            Ok(())
        })
    }

    /// M2a-S: the Owner resumes a mission held after an unclean shutdown.
    /// The resume is journaled before it takes effect; the mission then
    /// goes on from its state as it would have after a clean restart.
    /// Mother never calls this herself: it is the Owner's decision, like
    /// Stop.
    ///
    /// M2c (S3): the Owner is asked natively, through the port's door,
    /// naming the mission's contract hash and this request; no lock is held
    /// meanwhile. Only a recorded yes to the request still current resumes
    /// it; a no ([`CoordinatorError::Declined`]), a stale answer
    /// ([`CoordinatorError::ResumeStale`]), a Stop meanwhile or a broker
    /// refusal journal nothing, and the mission stays held (or stopped).
    pub fn resume(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let (request, generation) = self.resume_request(id)?;
        let answer = self.port.confirm_held_mission(&request);
        self.take_resume(id, generation, answer)
    }

    /// M2c, a resume's reservation: the held mission's request, under a
    /// label of its own, replacing any earlier request; the generation
    /// moves, so a superseded request is stale. Returns the request and the
    /// reservation.
    pub(crate) fn resume_request(
        &self,
        id: MissionId,
    ) -> Result<(MissionResumeConfirmation, u64), CoordinatorError> {
        self.with_lock(|inner| {
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            if mission.held.is_none() {
                return Err(illegal(mission.state, "resume"));
            }
            // Rule 6: a mission whose File settlement began never resumes.
            if mission.settlement.is_some() {
                return Err(CoordinatorError::Stopped);
            }
            // A broken or closed coordinator asks the Owner nothing.
            inner.usable()?;
            let (seq, number) = (inner.next_seq, inner.next_request);
            inner.next_request += 1;
            let mission = inner.mission(id)?;
            let summary = MissionSummary::of(id, mission.contract()?);
            let request = MissionResumeConfirmation {
                mission: id.to_string(),
                contract_hash: summary.hash.to_hex(),
                request: format!("{id}/{seq}/{number}"),
                held_reason: mission.held.map(|r| r.to_string()).unwrap_or_default(),
                summary: summary.lines(),
            };
            mission.resume_asking = Some(request.clone());
            mission.generation += 1;
            Ok((request, mission.generation))
        })
    }

    /// M2c, a resume's take: only for the request still current (the
    /// coordinator usable, the mission still held, the generation the one
    /// reserved), which is then consumed and the generation moved, whatever
    /// the answer. Only the broker's recorded yes to that exact hash and
    /// request journals `Resumed` and clears the hold.
    pub(crate) fn take_resume(
        &self,
        id: MissionId,
        generation: u64,
        answer: Result<MissionResumeApproval, AuthorityError>,
    ) -> Result<MissionState, CoordinatorError> {
        self.with_lock(|inner| {
            // An answer that comes after the break is discarded.
            inner.usable()?;
            let mission = inner.mission(id)?;
            let current = mission.generation == generation
                && mission.held.is_some()
                && mission.resume_asking.is_some();
            if !current {
                return Err(if mission.state == MissionState::Stopped {
                    CoordinatorError::Stopped
                } else {
                    CoordinatorError::ResumeStale
                });
            }
            let Some(asked) = mission.resume_asking.take() else {
                return Err(CoordinatorError::ResumeStale);
            };
            mission.generation += 1;
            let state = mission.state;
            let hash = mission.contract_ref.contract_hash.to_hex();
            match answer {
                Ok(approval)
                    if approval.contract_hash() == asked.contract_hash
                        && approval.request() == asked.request
                        && asked.contract_hash == hash =>
                {
                    let lifetime = inner.append(id, Event::Resumed)?;
                    let mission = inner.mission(id)?;
                    mission.held = None;
                    mission.held_journaled = false;
                    mission.lifetime = lifetime;
                    mission.consent_request = Some(asked.request);
                    mission.admitted = None;
                    Ok(state)
                }
                Ok(_) => Err(CoordinatorError::ResumeStale),
                Err(AuthorityError::Declined) => Err(CoordinatorError::Declined),
                Err(refused) => {
                    inner.set_detail(id, WorkingDetail::new("broker", &refused.to_string()));
                    Err(CoordinatorError::Broker(BrokerClass::of(refused.class())))
                }
            }
        })
    }

    /// M2a-S (tests): whether the journal failed in this coordinator.
    #[cfg(test)]
    pub(crate) fn broken(&self) -> bool {
        self.peek(|inner| inner.broken)
    }

    /// M2a-S: why a mission is held, if it is (a closed code).
    pub fn held(&self, id: MissionId) -> Option<String> {
        self.peek(|inner| {
            inner
                .missions
                .get(&id)
                .and_then(|m| m.held.map(|r| r.to_string()))
        })
    }

    pub fn state(&self, id: MissionId) -> Option<MissionState> {
        self.peek(|inner| inner.missions.get(&id).map(|m| m.state))
    }

    /// The mission's contract (P0: for a terminal mission after a reopen,
    /// fetched from the private store and verified now; `None` if it does
    /// not verify).
    pub fn contract(&self, id: MissionId) -> Option<MissionContract> {
        self.contract_of(id).ok()
    }

    /// The hash the mission's consent covers, if it holds one.
    pub fn consented_hash(&self, id: MissionId) -> Option<ContractHash> {
        self.peek(|inner| {
            inner
                .missions
                .get(&id)
                .and_then(|m| m.consent.as_ref().map(Consent::hash))
        })
    }

    /// Why a mission ended Blocked or Failed.
    pub fn reason(&self, id: MissionId) -> Option<String> {
        self.peek(|inner| inner.missions.get(&id).and_then(|m| m.reason.clone()))
    }

    pub fn missions(&self) -> Vec<MissionId> {
        self.peek(|inner| inner.missions.keys().copied().collect())
    }

    /// The verified journal.
    pub fn journal(&self) -> Result<Vec<Entry>, CoordinatorError> {
        let lines = self
            .peek(|inner| inner.store.load())
            .map_err(CoordinatorError::Journal)?;
        verify(&lines).map_err(CoordinatorError::Journal)
    }
}

/// M2a-S: where replay is in the journal's sessions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Session {
    /// No session record yet (a journal from before sessions).
    Never,
    Open,
    /// Closed cleanly: only the next session's opening may follow.
    Closed,
}

/// Replay one verified entry. Each trigger goes through the same table as
/// live, and records must come in the order the live coordinator writes
/// them; anything else found is refused, with a closed code. P0: a mission
/// is rebuilt from its references alone; recovery loads its content.
fn replay(
    missions: &mut BTreeMap<MissionId, Mission>,
    next_mission: &mut u64,
    session: &mut Session,
    entry: &Entry,
) -> Result<(), ReplayViolation> {
    use ReplayViolation as V;
    let id = entry.mission;
    match &entry.event {
        Event::SessionOpened | Event::SessionClosed if id != SESSION => {
            return Err(V::SessionRecordNamesMission);
        }
        Event::SessionOpened => {
            if *session != Session::Closed {
                // After a session with no clean close, every unfinished
                // mission not held must be held.
                for mission in missions.values_mut() {
                    if !mission.state.is_terminal() && mission.held.is_none() {
                        mission.must_hold = true;
                    }
                }
            }
            *session = Session::Open;
            return Ok(());
        }
        Event::SessionClosed if *session != Session::Open => {
            return Err(V::CloseOfNoSession);
        }
        Event::SessionClosed if missions.values().any(|m| m.must_hold) => {
            return Err(V::CloseWithUnheldMission);
        }
        Event::SessionClosed => {
            *session = Session::Closed;
            return Ok(());
        }
        _ if *session == Session::Closed => {
            return Err(V::RecordAfterClose);
        }
        _ => {}
    }
    if let Event::Drafted { contract } = &entry.event {
        if id.0 != *next_mission {
            return Err(V::DraftedOutOfOrder);
        }
        if missions.len() >= MAX_MISSIONS {
            return Err(V::TooManyMissions);
        }
        missions.insert(id, Mission::new(contract.clone(), None));
        *next_mission = id.0 + 1;
        return Ok(());
    }
    let mission = missions.get_mut(&id).ok_or(V::NeverDrafted)?;
    // While held the live coordinator only stops the mission, journals an
    // action closed that the broker no longer holds, or takes the Owner's
    // resume.
    let while_held = matches!(
        &entry.event,
        Event::Transition {
            trigger: Trigger::Stop { .. } | Trigger::Settled { .. }
        } | Event::ApprovalClosed { .. }
            | Event::Resumed
            | Event::SettlementStarted { .. }
    );
    if mission.held.is_some() && !while_held {
        return Err(V::RecordWhileHeld);
    }
    // Before its hold, only what recovery writes: an abandoned lane call, a
    // closed action, or an end (a block, a lapse, a Stop).
    let before_hold = match &entry.event {
        Event::Held { .. }
        | Event::LaneOvertaken { .. }
        | Event::ApprovalClosed { .. }
        | Event::SettlementStarted { .. } => true,
        Event::Transition { trigger } => {
            transition(mission.state, trigger).is_ok_and(MissionState::is_terminal)
        }
        _ => false,
    };
    if mission.must_hold && !before_hold {
        return Err(V::RecordBeforeHold);
    }
    match &entry.event {
        Event::Drafted { .. } => unreachable!(),
        Event::Transition { trigger } => {
            let to = transition(mission.state, trigger).map_err(|_| V::IllegalTransition)?;
            // Rule 6: once a File's settlement began, only its terminal line.
            if mission.settlement.is_some() && !matches!(trigger, Trigger::Settled { .. }) {
                return Err(V::SettlementConflict);
            }
            // While an effect is unsettled the live coordinator only stops,
            // or blocks the mission on restart (or settles its File).
            let while_unsettled = matches!(trigger, Trigger::Stop { .. } | Trigger::Settled { .. })
                || matches!(
                    trigger,
                    Trigger::Block {
                        reason: BlockReason::EffectOutcomeUnknown
                    }
                );
            if mission.unsettled.is_some() && !while_unsettled {
                return Err(V::TransitionWhileUnsettled);
            }
            let hash = mission.contract_ref.contract_hash;
            match trigger {
                Trigger::Submit { hash: h } if *h != hash => {
                    return Err(V::SubmitOtherHash);
                }
                Trigger::Consent { hash: h, .. } | Trigger::Deny { hash: h } if *h != hash => {
                    return Err(V::ConsentOtherHash);
                }
                Trigger::Start if !mission.consented() => {
                    return Err(V::WorkWithoutConsent);
                }
                // A wait follows the settlement of the effect it waits on.
                Trigger::AwaitApproval { effect, .. }
                    if mission.awaiting_settled != Some(*effect) =>
                {
                    return Err(V::WaitNotAwaiting);
                }
                // What the live coordinator keeps: the rest of one answer
                // (effect requests) and the lanes after the asking one.
                Trigger::AwaitApproval {
                    rest, roles_left, ..
                } if rest.count >= MAX_EFFECTS_PER_CALL as u64
                    || !phase_roles(mission.state).ends_with(roles_left)
                    || roles_left.len() >= phase_roles(mission.state).len() =>
                {
                    return Err(V::WaitNotWritten);
                }
                Trigger::ActionApproved { effect, phase }
                    if mission
                        .waiting
                        .as_ref()
                        .is_none_or(|w| w.effect != *effect || w.phase != *phase) =>
                {
                    return Err(V::ApprovalOfOtherWait);
                }
                Trigger::ActionDenied { effect } | Trigger::ApprovalLapsed { effect, .. }
                    if mission.waiting.as_ref().is_none_or(|w| w.effect != *effect) =>
                {
                    return Err(V::EndOfOtherWait);
                }
                Trigger::Settled {
                    effect,
                    action,
                    outcome,
                    end,
                } => {
                    // Addendum 2: the exact File, its first cause's end, and
                    // the journaled identity.
                    let expected = match mission.settlement {
                        Some((started, cause)) if started == *effect => Some(cause.end(outcome)),
                        Some(_) => return Err(V::SettlementConflict),
                        None if mission.unresolved_file == Some(*effect)
                            && mission
                                .waiting
                                .as_ref()
                                .is_some_and(|w| w.effect == *effect) =>
                        {
                            outcome.ordinary_end().or_else(|| outcome.deadline_end())
                        }
                        None => return Err(V::SettlementOutOfPlace),
                    };
                    if expected != Some(*end) {
                        return Err(V::SettlementConflict);
                    }
                    if let Some((open, pending)) = &mission.open_action {
                        if open == effect && action.as_ref() != Some(pending) {
                            return Err(V::SettlementConflict);
                        }
                    }
                }
                _ => {}
            }
            mission.apply(trigger, to, 0);
            if let Trigger::Consent { hash, .. } = trigger {
                // The verified journal's record of the broker's approval.
                mission.consent = Some(Consent::minted(*hash));
                mission.lifetime = entry.seq;
            }
        }
        Event::LaneInvoked { invocation, .. } => {
            if mission.settlement.is_some() {
                return Err(V::SettlementConflict);
            }
            if !mission.state.is_working()
                || mission.state == MissionState::Explaining
                || mission.pending_lane.is_some()
                || mission.open_action.is_some()
                || invocation.mission != id
                || invocation.seq != entry.seq
            {
                return Err(V::LaneOutOfPlace);
            }
            mission.pending_lane = Some(*invocation);
            mission.model_calls = mission.model_calls.saturating_add(1);
            mission.awaiting_settled = None;
        }
        Event::LaneReturned {
            invocation,
            output_hash,
        } => {
            if mission.pending_lane != Some(*invocation) {
                return Err(V::LaneAnswerWithoutCall);
            }
            mission.pending_lane = None;
            mission.prior_outputs.push(output_hash.clone());
        }
        Event::LaneOvertaken { invocation } => {
            if mission.pending_lane != Some(*invocation) {
                return Err(V::LaneAnswerWithoutCall);
            }
            mission.pending_lane = None;
        }
        Event::EffectDispatched { effect, kind, .. } => {
            if !mission.state.is_working()
                || *effect != mission.next_effect
                || mission.unsettled.is_some()
                || mission.open_action.is_some()
                || mission.settlement.is_some()
            {
                return Err(V::DispatchOutOfPlace);
            }
            mission.next_effect += 1;
            mission.unsettled = Some(*effect);
            if *kind == EffectKind::File {
                mission.unresolved_file = Some(*effect);
            }
            mission.dispatched_in_phase = true;
            mission.awaiting_settled = None;
        }
        Event::EffectSettled { effect, outcome } => {
            if mission.unsettled != Some(*effect) {
                return Err(V::SettledNotDispatched);
            }
            mission.unsettled = None;
            if *outcome == SettledOutcome::AwaitingApproval {
                mission.awaiting_settled = Some(*effect);
            } else if mission.unresolved_file == Some(*effect) {
                mission.unresolved_file = None;
            }
        }
        Event::ApprovalPending { effect, pending } => {
            if !mission.state.may_wait()
                || mission.awaiting_settled != Some(*effect)
                || mission.open_action.is_some()
                || mission.settlement.is_some()
            {
                return Err(V::PendingOutOfPlace);
            }
            mission.open_action = Some((*effect, pending.clone()));
        }
        Event::ApprovalClosed { effect } => {
            if mission.open_action.as_ref().map(|(e, _)| *e) != Some(*effect) {
                return Err(V::ClosureOfNoAction);
            }
            mission.open_action = None;
        }
        Event::ConsentRefused { .. } | Event::EffectRefused { .. } => {}
        Event::SessionOpened | Event::SessionClosed => unreachable!(),
        // Rule 4/6: the File's settlement began; its first cause stands.
        Event::SettlementStarted { effect, cause } => {
            if mission.state.is_terminal() || mission.settlement.is_some() {
                return Err(V::SettlementConflict);
            }
            if mission.unresolved_file != Some(*effect) {
                return Err(V::SettlementOutOfPlace);
            }
            mission.settlement = Some((*effect, *cause));
            mission.generation += 1;
        }
        Event::Held { reason } => {
            if mission.state.is_terminal()
                || mission.pending_lane.is_some()
                || mission.unsettled.is_some()
                || mission.dispatched_in_phase
                || mission.settlement.is_some()
            {
                return Err(V::HoldOutOfPlace);
            }
            mission.held = Some(*reason);
            mission.held_journaled = true;
            mission.must_hold = false;
        }
        Event::Resumed => {
            if mission.held.is_none() {
                return Err(V::ResumeNotHeld);
            }
            if mission.settlement.is_some() {
                return Err(V::SettlementConflict);
            }
            mission.held = None;
            mission.held_journaled = false;
            mission.lifetime = entry.seq;
        }
    }
    Ok(())
}
