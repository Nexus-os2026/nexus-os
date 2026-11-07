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
    ApprovalStatus, EffectOutcome, EffectPort, EffectRequest, Owed, PendingAction,
};
use crate::evidence::{
    check, lane_identity_digest, BlockReason, BrokerClass, ConsentRefusedReason, FailReason,
    HeldReason, LapseReason, MotherRefusal, SettleCause, SettledOutcome, SettlementOutcome,
    StopReason, VerifyFailReason, WorkingDetail, FILE_UNCERTAIN_NOTICE,
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
use nexus_governed_control::authority::file_session::{FileAttachment, FileSessionError};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::authority::AuthorityError;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex};
mod model_calls;
use model_calls::{CallAccounting, LaneCalls};
mod lite;
mod lite_intent;
#[path = "oracles/m2e_budget.rs"]
#[cfg(test)]
mod m2e_budget;
mod session_close;
pub use session_close::PreparedSessionEnd;
#[cfg(test)]
mod lite_oracles;
mod lite_settle;
mod page_proof;
pub use lite::FileSession;
mod session_result;
pub use session_result::{TerminalFacts, TerminalIntent};

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
    FileExclusive,
    FileSession(FileSessionError),
    Poisoned,
    Recording,
    FileUnfinished,
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
    /// Kind of the last journaled dispatch, kept through approval retirement.
    /// No later effect is dispatched while its open action remains.
    dispatched_kind: Option<EffectKind>,
    /// An effect was dispatched since the last transition: re-running the
    /// phase after a restart could repeat it.
    dispatched_in_phase: bool,
    /// A lane call journaled as invoked and not yet answered (replay).
    pending_lane: Option<InvocationId>,
    call_accounting: Option<CallAccounting>,
    lane_calls: Option<LaneCalls>,
    declared_model_budgets: Vec<u32>,
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
    file_mode: bool,
    consent_request: Option<String>,
    unresolved_file: Option<u64>,
    settlement: Option<(u64, SettleCause)>,
    acks: Vec<PendingAction>,
    terminal_uncertain: bool,
    // Exact terminal append result, independent of later store failures and
    // readable-but-unacknowledged bytes. Never inferred from an older prefix.
    terminal_record: Option<(Trigger, Option<u64>)>,
    /// M3-Db: the retained page and its receipt, by reference only.
    proof: page_proof::Proof,
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
            dispatched_kind: None,
            dispatched_in_phase: false,
            pending_lane: None,
            call_accounting: None,
            lane_calls: None,
            declared_model_budgets: Vec::new(),
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
            file_mode: false,
            consent_request: None,
            unresolved_file: None,
            settlement: None,
            acks: Vec::new(),
            terminal_uncertain: false,
            terminal_record: None,
            proof: page_proof::Proof::default(),
        }
    }

    /// End the invocation without erasing its pending settlement evidence,
    /// charged total or declared budgets. Dropping live state revokes its
    /// retained atomic observers before this owning boundary is released.
    fn invalidate_calls(&mut self) {
        if let Some(live) = self.lane_calls.take() {
            live.owner.revoke(); // O1: before journal I/O, still owning the lock.
        }
        self.call_accounting = None;
    }

    /// Take a transition the table accepted. A consent's token is set by
    /// the caller: live from the broker's approval, on replay from the
    /// verified journal.
    fn apply(&mut self, trigger: &Trigger, to: MissionState, mono_now: u64) {
        self.invalidate_calls();
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
                self.unsettled = None;
                self.pending_lane = None;
                self.terminal_uncertain = !outcome.conclusive();
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

struct Inner {
    session_context: Option<nexus_governed_control::host_session::SessionContext>,
    file_cell: Option<nexus_governed_control::authority::file_session::HaltCell>,
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
}

#[path = "adapter/session_journal_oracles_tests.rs"]
#[cfg(target_os = "linux")]
#[cfg(test)]
mod session_journal_oracles;

impl Inner {
    fn admit_session(
        &self,
        step: nexus_governed_control::host_session::MotherStep,
    ) -> Result<(), CoordinatorError> {
        if let Some(context) = &self.session_context {
            context.admit(step).map_err(|_| CoordinatorError::Stopped)?;
        }
        Ok(())
    }
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

    fn invalidate_calls(&mut self) {
        for mission in self.missions.values_mut() {
            mission.invalidate_calls();
        }
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
        let work = journal_work(&entry.event);
        let line = entry.to_line();
        #[cfg(all(test, target_os = "linux"))]
        session_journal_oracles::at("before", &entry);
        #[cfg(all(test, target_os = "linux"))]
        lite::session_terminal_fix2::before(&entry);
        if work {
            self.admit_session(nexus_governed_control::host_session::MotherStep::Transition)?;
        }
        // Deny memory-only observers before journal I/O/terminal intent,
        // while this boundary still owns the coordinator lock. A failed
        // append cannot restore the old invocation's authority.
        match &entry.event {
            Event::SessionOpened | Event::SessionClosed => self.invalidate_calls(),
            Event::LaneReturned { .. }
            | Event::LaneOvertaken { .. }
            | Event::Transition { .. }
            | Event::SettlementStarted { .. }
            | Event::Held { .. } => {
                if let Some(mission) = self.missions.get_mut(&mission) {
                    mission.invalidate_calls();
                }
            }
            _ => {}
        }
        // Serialization and refused work admission are not terminal intent.
        // Retain an attempt only once this exact append has been admitted;
        // an entered append which fails must still report recording unknown.
        if let Event::Transition { trigger } = &entry.event {
            if self
                .missions
                .get(&mission)
                .is_some_and(|m| transition(m.state, trigger).is_ok_and(|to| to.is_terminal()))
            {
                self.mission(mission)?
                    .terminal_record
                    .get_or_insert((trigger.clone(), None));
            }
        }
        if let Err(e) = self.store.append(&line) {
            // Fail closed, durably: a journal that may have lost a
            // transition (a Stop) must not be reopened as if it had not.
            // If poisoning fails too, the journal reopens, but its session
            // has no clean close: every unfinished mission comes back held.
            // Fenced at the authority first, under this same lock: nothing
            // reserved before the break can run at the broker after it.
            self.broken = true;
            self.invalidate_calls();
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
        #[cfg(all(test, target_os = "linux"))]
        session_journal_oracles::at("after", &entry);
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

    /// Journal a trigger, then take it.
    fn fire(
        &mut self,
        id: MissionId,
        trigger: Trigger,
        mono_now: u64,
    ) -> Result<MissionState, CoordinatorError> {
        let m = self.mission(id)?;
        if m.settlement.is_some() {
            return Err(CoordinatorError::Stopped);
        }
        if m.file_mode
            && self
                .file_cell
                .as_ref()
                .is_some_and(|cell| cell.is_engaged())
            && !matches!(
                trigger,
                Trigger::Stop { .. }
                    | Trigger::Block { .. }
                    | Trigger::Fail { .. }
                    | Trigger::VerifyFail { .. }
                    | Trigger::ActionDenied { .. }
                    | Trigger::ApprovalLapsed { .. }
            )
        {
            return Err(CoordinatorError::Stopped);
        }
        self.fire_settling(id, trigger, mono_now)
    }

    fn fire_settling(
        &mut self,
        id: MissionId,
        trigger: Trigger,
        mono_now: u64,
    ) -> Result<MissionState, CoordinatorError> {
        let from = self.mission(id)?.state;
        let to = transition(from, &trigger).map_err(|e| refusal(from, e))?;
        let sequence = self.append(
            id,
            Event::Transition {
                trigger: trigger.clone(),
            },
        )?;
        if to.is_terminal() {
            self.mission(id)?.terminal_record = Some((trigger.clone(), Some(sequence)));
        }
        self.mission(id)?.apply(&trigger, to, mono_now);
        Ok(to)
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
        EffectRequest::File { .. } | EffectRequest::CreateFile(_)
            if contract.requested_folder.is_none() =>
        {
            Some(MotherRefusal::NoRequestedFolder)
        }
        EffectRequest::File { relative_path, .. } if !relative_request(relative_path) => {
            Some(MotherRefusal::FileRequestNotRelative)
        }
        EffectRequest::CreateFile(create) => file_refuses(contract, create, workspace),
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
    file_attachment: Option<Arc<FileAttachment>>,
    file: Option<lite::FileLife>,
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
    pub(crate) file_missing_cause: std::sync::atomic::AtomicBool,
    pub(crate) file_selection_poison: std::sync::atomic::AtomicBool,
    pub(crate) file_engagement: crate::oracles::lite_engagement_hooks::SiteHook,
    pub(crate) file_after_halt: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) file_invocations: std::sync::atomic::AtomicUsize,
    pub(crate) file_before_ack: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) model_replied: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) file_before_lane: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) file_after_perform_check: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) before_owed_write: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) after_owed_write: crate::oracles::m2as_hooks::HookSlot,
    pub(crate) fence_to_marker: crate::oracles::m2as_hooks::HookSlot,
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
    Emergency,
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
        let file_attachment = port.file_attach().map_err(CoordinatorError::FileSession)?;
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
        #[cfg(test)]
        let hooks = CoordinatorHooks::default();
        let coordinator = Self {
            file_attachment,
            file: None,
            port: port.clone(),
            lanes,
            clock,
            private: Arc::new(private),
            inner: Mutex::new(Inner {
                session_context: port.session_context(),
                file_cell: None,
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
            }),
            #[cfg(test)]
            hooks,
        };
        // If opening fails from here on (the journal is then poisoned and
        // never reopens, or, if poisoning failed too, reopens with every
        // unfinished mission held), every journaled open action is
        // withdrawn first, a waiting mission's too.
        if let Err(e) = coordinator.recover(closed_cleanly, clean) {
            coordinator.withdraw_open_actions();
            return Err(e);
        }
        Ok(coordinator)
    }

    /// M2c PR-B: run `body` under the coordinator lock. A record batch it
    /// left (a break's fence) is taken out still under the lock and written
    /// once the lock is released, by this call, on every way out (a `?` in
    /// `body` returns from `body` only). No call can see another's batch.
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
        self.step(|inner| {
            if inner.missions.len() >= MAX_MISSIONS {
                return Err(CoordinatorError::Capacity);
            }
            inner.usable()
        })?;
        let reference = self.put_contract(&contract)?;
        self.step(|inner| {
            if inner.missions.len() >= MAX_MISSIONS {
                return Err(CoordinatorError::Capacity);
            }
            let id = MissionId(inner.next_mission);
            if let Some(file) = &self.file {
                file.check_budget(&*self.clock);
                if file.cell.is_engaged() {
                    return Err(CoordinatorError::Stopped);
                }
                if file.mission.load(std::sync::atomic::Ordering::SeqCst) != 0 {
                    return Err(CoordinatorError::FileExclusive);
                }
                file.mission
                    .store(id.0, std::sync::atomic::Ordering::SeqCst);
            }
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
            if self.file.is_some() {
                inner.append(id, Event::FileModeSelected)?;
                inner.mission(id)?.file_mode = true;
            }
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
        self.step(|inner| {
            inner.clear_detail(id);
            may_revise(inner)
        })?;
        contract.validate().map_err(CoordinatorError::Contract)?;
        let reference = self.put_contract(&contract)?;
        self.step(|inner| {
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
        if self
            .lanes
            .by_role
            .values()
            .any(|lane| lane.max_model_calls() > 0)
            && (self.file.is_none() || self.port.model_identity().is_none())
        {
            return Err(CoordinatorError::Contract(
                ContractError::ModelLaneUnavailable,
            ));
        }
        self.step(|inner| {
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
        self.file_boundary(id)?;
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
        self.step(|inner| {
            // A broken coordinator asks the Owner nothing.
            inner.usable()?;
            inner.clear_detail(id);
            let (seq, number) = (inner.next_seq, inner.next_request);
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("before", "consent_reserve");
            inner.admit_session(nexus_governed_control::host_session::MotherStep::ConsentReservation)?;
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("entered", "consent_reserve");
            inner.next_request += 1;
            let mission = inner.mission(id)?;
            mission.not_held()?;
            if mission.state != MissionState::AwaitingConsent {
                return Err(illegal(mission.state, "consent"));
            }
            let mut request = MissionSummary::of(id, mission.contract()?)
                .confirmation(format!("{id}/{seq}/{number}"));
            if mission.file_mode {
                request.summary.extend([
                    "File mode: this is the only active mission in this process session.".into(),
                    "Only this original session can run it; a replacement session cannot continue it.".into(),
                    "Stop or emergency ends this File session permanently. Starting again requires a fresh process; restart does not authorize retrying an uncertain write.".into(),
                ]);
            }
            mission.asking = Some(request.clone());
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("after", "consent_reserve");
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
        self.step(|inner| {
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
                    let mission = inner.mission(id)?;
                    mission.consent = Some(consent);
                    if mission.file_mode {
                        mission.consent_request = Some(asked.request.clone());
                        if let Some(file) = &self.file {
                            let duration = mission.contract()?.budget.max_duration_ms;
                            file.consent_at(mono, at_wall_ms, duration);
                            if let Some(context) = self.port.session_context() {
                                context
                                    .narrow_mission_deadlines(
                                        self.clock.clone(),
                                        mono.saturating_add(duration),
                                        at_wall_ms.saturating_add(duration),
                                    )
                                    .map_err(|_| CoordinatorError::Stopped)?;
                            }
                        }
                    }
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
        self.file_boundary(id)?;
        let emergency = self.port.emergency_stopped();
        let start = self.step(|inner| -> Result<Start, CoordinatorError> {
            // A broken coordinator starts nothing and asks the broker nothing.
            inner.usable()?;
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            mission.not_held()?;
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
                return Ok(Start::Emergency);
            }
            if mission.out_of_time(&*self.clock) {
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
            let context = inner.session_context.clone();
            let mission = inner.mission(id)?;
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("before", "phase_reserve");
            if let Some(context) = context {
                context
                    .admit(nexus_governed_control::host_session::MotherStep::PhaseReservation)
                    .map_err(|_| CoordinatorError::Stopped)?;
            }
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("entered", "phase_reserve");
            mission.busy = true;
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("after", "phase_reserve");
            Ok(Start::Run(report, from, roles, mission.generation))
        })?;
        let (mut report, from, roles, generation) = match start {
            Start::Emergency => {
                self.stop_for(id, StopReason::EmergencyStop)?;
                return Err(CoordinatorError::Stopped);
            }
            Start::OutOfTime(mut report) => {
                report.to = self.end(id, BlockReason::BudgetTime)?;
                report.reason = Some(BlockReason::BudgetTime.to_string());
                return Ok(report);
            }
            Start::Done(report) => return Ok(report),
            Start::Finish(report) => {
                self.port.finish(id);
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
        self.step(|inner| {
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
        self.file_boundary(id)?;
        let file_approval = if self.file.is_some() {
            self.step(|inner| {
                let m = inner.mission(id)?;
                if m.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                Ok(m.dispatched_kind.ok_or(CoordinatorError::Stopped)? == EffectKind::File)
            })?
        } else {
            false
        };
        let (status, receipt) = if file_approval {
            match self.file_approval(id, generation)? {
                lite_settle::FileDecision::Status(status, receipt) => (status, receipt),
                lite_settle::FileDecision::Ended(state) => {
                    report.to = state;
                    report.reason = self.reason(id);
                    report.explanation = self.file_notice(id)?.map(str::to_owned);
                    return Ok(report);
                }
            }
        } else {
            (self.port.approval(id), None)
        };
        let step = self.step(|inner| {
            inner.kept(id)?;
            let mission = inner.mission(id)?;
            if mission.generation != generation {
                return Err(CoordinatorError::Stopped);
            }
            let Some(waiting) = mission.waiting.clone() else {
                return Ok(Then::End(BlockReason::WaitUnknown));
            };
            Ok(Then::Go((waiting, mission.contract()?.clone())))
        })?;
        let (waiting, contract) = match self.then(id, step)? {
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
        let end = match status {
            ApprovalStatus::Waiting => return Ok(report),
            ApprovalStatus::Approved => None,
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
            report.to = if file_approval {
                self.file_nonpublication_end(id, generation, trigger)?
            } else {
                self.end_non_file_with(id, trigger)?
            };
            report.reason = self.reason(id);
            return Ok(report);
        }

        self.file_boundary(id)?;
        // M3-Db: a page File's proof is read first, with no lock held.
        let page = file_approval && contract.page_facts.is_some();
        let proof = if page {
            Some(self.page_receipt(id, waiting.effect, receipt)?)
        } else {
            None
        };
        // Approved, and it ran at the broker: the phase goes on. A page's
        // receipt is appended under the same lock, before closure and ack.
        let generation = self.step(|inner| {
            let mono = self.clock.monotonic_ms();
            let approved = Trigger::ActionApproved {
                effect: waiting.effect,
                phase: waiting.phase,
            };
            let recorded = inner.fire(id, approved, mono).and_then(|to| {
                report.to = to;
                if let Some(Ok(event)) = &proof {
                    inner.append(id, event.clone())?;
                    inner.mission(id)?.proof.receipt = Some(waiting.effect);
                }
                Ok(())
            });
            if page && matches!(recorded, Err(CoordinatorError::Journal(_))) {
                page_proof::unrecorded(inner.mission(id)?, &mut report);
                return Ok(None);
            }
            recorded?;
            Ok::<_, CoordinatorError>(Some(inner.mission(id)?.generation))
        })?;
        let Some(generation) = generation else {
            return Ok(report);
        };
        if let Some(Err(end)) = proof {
            report.to = match end {
                page_proof::PageEnd::Fail(reason) => self.end_with(id, Trigger::Fail { reason })?,
                page_proof::PageEnd::Unavailable => {
                    self.stop_for(id, StopReason::PrivateContentUnavailable)?
                }
            };
            report.reason = self.reason(id);
            report.explanation = Some(page_proof::WRITTEN.into());
            return Ok(report);
        }
        // The approved action is final at the broker: its closure is
        // journaled before anything more is sent.
        if !self.close_action(id)? {
            report.to = self.end(id, BlockReason::ApprovalNotClosed)?;
            report.reason = self.reason(id);
            return Ok(report);
        }
        if self.file.is_some() {
            // The durable obligation is independent of the waiting slot. Ack
            // failure does not repeat the already published effect or lose its rest.
            let _acknowledgement = self.file_ack_all(id);
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
            self.file_boundary(id)?;
            let lane = self.lanes.get(*role);
            let identity = lane.identity();
            if self.port.emergency_stopped() {
                self.stop_for(id, StopReason::EmergencyStop)?;
                return Err(CoordinatorError::Stopped);
            }
            // Identity observation is outside the coordinator hold; native admission
            // independently rechecks it against the same original run and consent.
            let cap = lane.max_model_calls();
            let model = (cap > 0).then(|| self.port.model_identity()).flatten();
            let (step, caller, lane_contract) = self.step(|inner| {
                if identity.role != *role {
                    return Ok((Then::End(BlockReason::LaneIdentityChanged), None, None));
                }
                if cap > 0 {
                    let c = inner.mission(id)?.contract()?;
                    let reason = match (&c.model, &model) {
                        (Some(pin), Some(view))
                            if self.file.is_some()
                                && c.deliverable.as_deref() == Some("index.html") =>
                        {
                            (pin.name != view.name
                                || pin.digest != view.digest
                                || pin.identity != view.identity)
                                .then_some(BlockReason::ModelIdentityChanged)
                        }
                        _ => Some(BlockReason::ModelNotPinned),
                    };
                    if let Some(reason) = reason {
                        return Ok((Then::End(reason), None, None));
                    }
                }
                let step =
                    model_calls::reserve_lane(inner, id, generation, &identity, cap, &*self.clock)?;
                let caller = match &step {
                    Then::Go(input) if cap > 0 => Some(model_calls::Caller::reserved(
                        self,
                        inner,
                        input.invocation,
                        generation,
                    )?),
                    _ => None,
                };
                let lane_contract =
                    if matches!(&step, Then::Go(_)) && *role == LaneRole::Builder && cap > 0 {
                        Some(inner.mission(id)?.contract()?.clone())
                    } else {
                        None
                    };
                Ok((step, caller, lane_contract))
            })?;
            let input = match self.then(id, step)? {
                Ok(input) => input,
                Err(state) => return Ok(Ended::Ended(state)),
            };

            if let Some(caller) = &caller {
                if caller.admit().is_err() {
                    return Ok(Ended::Ended(
                        self.end(id, BlockReason::ModelAdmissionRefused)?,
                    ));
                }
            }
            #[cfg(test)]
            if self.file.is_some() {
                self.hooks.file_before_lane.fire();
            }
            self.file_boundary(id)?;
            #[cfg(test)]
            if self.file.is_some() {
                self.hooks
                    .file_invocations
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("before", "lane_invoke");
            #[cfg(all(test, target_os = "linux"))]
            lite::session_budget_fix1::before_admit();
            if let Some(context) = self.port.session_context() {
                context
                    .admit(nexus_governed_control::host_session::MotherStep::LaneInvocation)
                    .map_err(|_| CoordinatorError::Stopped)?;
            }
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("entered", "lane_invoke");
            let context = crate::lane::LaneContext {
                contract: lane_contract.as_ref(),
                model: caller.as_ref().map(|c| c as &dyn crate::lane::ModelCaller),
            };
            let output = lane.invoke_in(&input, &context);
            drop(caller);
            if cap > 0 {
                let terminal = match output.verdict {
                    Verdict::Fail { .. } if *role == LaneRole::Verifier => {
                        Some(Trigger::VerifyFail {
                            reason: VerifyFailReason::VerifierFailed,
                        })
                    }
                    Verdict::Fail { .. } => Some(Trigger::Fail {
                        reason: FailReason::LaneFail(*role),
                    }),
                    Verdict::Halt { .. } => Some(Trigger::Block {
                        reason: BlockReason::LaneHalt(*role),
                    }),
                    _ => None,
                };
                if let Some(trigger) = terminal {
                    let ended = self.end_with(id, trigger);
                    self.model_obligations()?;
                    return Ok(Ended::Ended(ended?));
                }
            }
            #[cfg(all(test, target_os = "linux"))]
            session_journal_oracles::call("after", "lane_invoke");
            let boundary = self.file_boundary(id);
            self.model_obligations()?;
            boundary?;

            // Keep the answer only if the reservation still holds.
            let hash = output_hash(&output);
            let contract = self.step(|inner| {
                inner.kept(id)?;
                let m = inner.mission(id)?;
                if m.generation != generation || m.pending_lane != Some(input.invocation) {
                    if m.pending_lane != Some(input.invocation) {
                        return Err(CoordinatorError::Stopped);
                    }
                    inner.append(
                        id,
                        Event::LaneOvertaken {
                            invocation: input.invocation,
                        },
                    )?;
                    let m = inner.mission(id)?;
                    m.pending_lane = None;
                    m.invalidate_calls();
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
                mission.pending_lane = None;
                mission.invalidate_calls();
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
                    self.step(|inner| {
                        inner.set_detail(id, detail);
                        Ok(())
                    })?;
                }
                return Ok(Ended::Ended(self.end_with(id, trigger)?));
            }

            // Db-R1 L5: a page Builder's answer shape before any generic check.
            let page = *role == LaneRole::Builder && contract.page_facts.is_some();
            if page && page_proof::deliverable(&output.effects).is_err() {
                let reason = FailReason::DeliverableInvalid;
                return Ok(Ended::Ended(self.end_with(id, Trigger::Fail { reason })?));
            }
            // Mother's own checks of every effect, before any is sent.
            if output.effects.len() > MAX_EFFECTS_PER_CALL {
                return Ok(Ended::Ended(self.end(id, BlockReason::TooManyEffects)?));
            }
            if let Ended::Ended(state) = self.checked(id, &contract, &output.effects, report)? {
                return Ok(Ended::Ended(state));
            }
            if *role == LaneRole::Builder && contract.page_facts.is_some() {
                let retained = self.retain(id, generation, input.invocation, &output.effects)?;
                if let Ended::Ended(state) = retained {
                    return Ok(Ended::Ended(state));
                }
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
        for effect in effects {
            if let Some(reason) = mother_refuses(contract, effect, self.port.file_workspace()) {
                self.step(|inner| {
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
            self.file_boundary(id)?;
            if self.port.emergency_stopped() {
                self.stop_for(id, StopReason::EmergencyStop)?;
                return Err(CoordinatorError::Stopped);
            }
            let step = self.step(|inner| {
                let mission = inner.mission(id)?;
                if mission.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                if mission.out_of_time(&*self.clock) {
                    return Ok(Then::End(BlockReason::BudgetTime));
                }
                let number = mission.next_effect;
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
                mission.dispatched_kind = Some(effect.kind());
                if mission.file_mode && effect.kind() == EffectKind::File {
                    mission.unresolved_file = Some(number);
                }
                mission.dispatched_in_phase = true;
                Ok(Then::Go(number))
            })?;
            let number = match self.then(id, step)? {
                Ok(number) => number,
                Err(state) => return Ok(Ended::Ended(state)),
            };

            self.file_boundary(id)?;
            let outcome = if self.file.is_some() && effect.kind() == EffectKind::File {
                self.port.perform_file(id, number, effect)
            } else {
                self.port.perform(id, effect)
            };
            self.file_boundary(id)?;
            #[cfg(test)]
            self.hooks.file_after_perform_check.fire();

            let settled = match &outcome {
                Ok(EffectOutcome::Done { .. }) => SettledOutcome::Done,
                Ok(EffectOutcome::AwaitingApproval) => SettledOutcome::AwaitingApproval,
                Err(refusal) => SettledOutcome::Refused(refusal.class),
            };
            let step = self.step(|inner| {
                // M2a-S: after the break the outcome is discarded, not
                // journaled; the mission ends in memory.
                if let Err(e) = inner.kept(id) {
                    return Ok(Then::EndFail(BlockReason::JournalFailed, e));
                }
                // Stop can win after the out-of-lock halt check. Validate the
                // exact reservation under this lock before writing any outcome.
                let mission = inner.mission(id)?;
                if mission.generation != generation
                    || mission.unsettled != Some(number)
                    || mission.settlement.is_some()
                    || mission.state.is_terminal()
                {
                    return Err(CoordinatorError::Stopped);
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
                if matches!(settled, SettledOutcome::Refused(_)) && mission.settlement.is_none() {
                    mission.unresolved_file = None;
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
                        self.stop_for(id, StopReason::EmergencyStop)?;
                        return Err(CoordinatorError::Stopped);
                    }
                    let reason = BlockReason::BrokerRefused(refusal.class);
                    return Ok(Ended::Ended(self.end(id, reason)?));
                }
            }
        }
        Ok(Ended::Through)
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
        #[cfg(all(test, target_os = "linux"))]
        session_journal_oracles::call("before", "paused_store");
        if let Some(context) = self.port.session_context() {
            context
                .admit(nexus_governed_control::host_session::MotherStep::PrivateWrite)
                .map_err(|_| CoordinatorError::Stopped)?;
        }
        #[cfg(all(test, target_os = "linux"))]
        session_journal_oracles::call("entered", "paused_store");
        let stored = self.private.put(ContentKind::PausedPhase, &bytes);
        #[cfg(all(test, target_os = "linux"))]
        session_journal_oracles::call("after", "paused_store");
        let content = match stored {
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
            let step = self.step(|inner| {
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
        let waited = self.step(|inner| {
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

    fn publish_file_wait(&self, id: MissionId) -> Result<(), CoordinatorError> {
        let target = self.peek(|inner| {
            let m = inner
                .missions
                .get(&id)
                .ok_or(CoordinatorError::UnknownMission)?;
            if m.state != MissionState::AwaitingApproval {
                return Ok(None);
            }
            if m.busy {
                return Err(CoordinatorError::Stopped);
            }
            Ok(m.open_action
                .clone()
                .map(|(effect, pending)| (effect, pending, m.generation)))
        })?;
        let Some((effect, pending, generation)) = target else {
            return Ok(());
        };
        let Some(prepared) = self
            .port
            .prepare_file_wait(id, effect, &pending)
            .map_err(|e| CoordinatorError::Broker(e.class))?
        else {
            return Ok(());
        };
        self.step(|inner| {
            let m = inner.mission(id)?;
            if m.state != MissionState::AwaitingApproval
                || m.busy
                || m.generation != generation
                || m.waiting.as_ref().is_none_or(|wait| wait.effect != effect)
                || m.open_action.as_ref() != Some(&(effect, pending))
            {
                return Err(CoordinatorError::Stopped);
            }
            self.port
                .publish_file_wait(prepared, generation)
                .map_err(|e| CoordinatorError::Broker(e.class))
        })
    }

    /// End a mission for `reason` (Blocked) and cancel what it opened at
    /// the port; a waiting mission's wait lapses (which withdraws the
    /// pending action).
    fn end(&self, id: MissionId, reason: BlockReason) -> Result<MissionState, CoordinatorError> {
        if self.file.is_some() {
            // This is also an ingress: a caller may have just discovered its
            // budget expired. Engage before the first lock, and keep that
            // cause rather than converting the wait to a generic lapse.
            let cause = if reason == BlockReason::BudgetTime {
                SettleCause::TimeBudget
            } else {
                SettleCause::Ended(reason)
            };
            return self.file_stop(id, cause);
        }
        let trigger = self.step(|inner| {
            let mission = inner.mission(id)?;
            Ok::<_, CoordinatorError>(match (&mission.waiting, mission.state) {
                (Some(waiting), MissionState::AwaitingApproval) => Trigger::ApprovalLapsed {
                    effect: waiting.effect,
                    reason: LapseReason::Ended(reason),
                },
                _ => Trigger::Block { reason },
            })
        })?;
        self.end_with(id, trigger)
    }

    /// End a mission with `trigger` (Block, Fail, VerifyFail, or the end of
    /// a wait) and cancel what it opened at the port.
    fn end_with(&self, id: MissionId, trigger: Trigger) -> Result<MissionState, CoordinatorError> {
        if self.file.is_some() {
            if matches!(trigger, Trigger::Fail { .. } | Trigger::VerifyFail { .. }) {
                // Keep the exact failure before exposing the halt. A competing
                // touch completes this same intent; any unresolved File still
                // takes settlement precedence over the ordinary terminal.
                self.file_latch_terminal(lite_intent::FileTerminal::Failure { id, trigger })?;
                return self.file_stop(id, SettleCause::Ended(BlockReason::PhaseInterrupted));
            }
            let reason = match trigger {
                Trigger::Block { reason } => reason,
                _ => BlockReason::PhaseInterrupted,
            };
            return self.end(id, reason);
        }
        self.end_non_file_with(id, trigger)
    }

    /// The existing non-File terminal path also serves non-File approvals in
    /// a File session. Keep the denial/lapse trigger and any older File acks.
    fn end_non_file_with(
        &self,
        id: MissionId,
        trigger: Trigger,
    ) -> Result<MissionState, CoordinatorError> {
        if self.file.is_some() {
            self.file_latch_terminal(lite_intent::FileTerminal::NonFile { id, trigger })?;
            #[cfg(test)]
            self.hooks.file_engagement.fire("non-file-terminal");
            return self.file_stop(id, SettleCause::Stop(StopReason::Owner));
        }
        self.record_non_file_end(id, trigger)
    }

    fn record_non_file_end(
        &self,
        id: MissionId,
        trigger: Trigger,
    ) -> Result<MissionState, CoordinatorError> {
        let mono = self.clock.monotonic_ms();
        let result = self.step(|inner| {
            let from = inner.mission(id)?.state;
            if self.file.is_some() && from.is_terminal() {
                return Ok(Ok(from));
            }
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
        let closed = self.close_action(id);
        let state = result?;
        withdrawn.map_err(|r| CoordinatorError::Withdrawal(r.class))?;
        closed?;
        if self.file.is_some() {
            self.file_ack_all(id)?;
        }
        Ok(state)
    }

    /// Journal the mission's open action closed, once the port confirms
    /// the broker no longer holds it open. `Ok(false)`: not confirmed.
    fn close_action(&self, id: MissionId) -> Result<bool, CoordinatorError> {
        let (open, file_action, unrecorded) = self.step(|inner| {
            let m = inner.mission(id)?;
            Ok((
                m.open_action.clone(),
                m.file_mode && m.dispatched_kind == Some(EffectKind::File),
                m.proof.unrecorded,
            ))
        })?;
        // Db2-R1 L2: an unrecorded proof's action is never journaled closed.
        if unrecorded {
            return Ok(false);
        }
        if file_action {
            return self.file_close_action(id);
        }
        let Some((effect, pending)) = open else {
            return Ok(true);
        };
        if !self.port.settled(&pending) {
            return Ok(false);
        }
        self.step(|inner| {
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
                .filter(|(_, m)| !m.file_mode)
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
        let open = self.open_actions();
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
        if closed_cleanly {
            self.step(|inner| inner.open_session_at_once())?;
        }
        self.recover_files()?;
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
                    self.step(|inner| {
                        let line = inner.next_seq as usize + 1;
                        let mission = inner.mission(id)?;
                        if mission
                            .declared_model_budgets
                            .iter()
                            .any(|b| *b != contract.budget.max_model_calls)
                            || mission.model_calls > contract.budget.max_model_calls
                        {
                            return Err(CoordinatorError::Journal(JournalError::Inconsistent {
                                line,
                                code: ReplayViolation::ModelBudgetMismatch,
                            }));
                        }
                        mission.contract = Some(contract);
                        if let Some(waiting) = mission.waiting.as_mut() {
                            waiting.rest = rest;
                        }
                        Ok::<_, CoordinatorError>(())
                    })?;
                }
                Err(_) => {
                    // Withdraw first: if the Stop cannot be journaled, nothing
                    // of the mission is left approvable at the broker.
                    if let Err(refusal) = self.port.withdraw(id) {
                        first_error.get_or_insert(CoordinatorError::Withdrawal(refusal.class));
                    }
                    let mono = self.clock.monotonic_ms();
                    let stopped = self.step(|inner| {
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
        self.step(|inner| {
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
                    mission.invalidate_calls();
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

    fn stop_for(
        &self,
        id: MissionId,
        reason: StopReason,
    ) -> Result<MissionState, CoordinatorError> {
        if self.file.is_some() {
            return self.file_stop(id, SettleCause::Stop(reason));
        }
        type Stopping = (Result<MissionState, CoordinatorError>, bool);
        let step = self.step(
            |inner| -> Result<Result<Stopping, MissionState>, CoordinatorError> {
                inner.clear_detail(id);
                let mission = inner.mission(id)?;
                if mission.state.is_terminal() {
                    // M2a-S: on a broken coordinator a terminal state may be in
                    // memory only (a Stop or denial the journal lost): never Ok.
                    inner.usable()?;
                    return Ok(Err(inner.mission(id)?.state));
                }
                let may_have_dispatched = mission.approved_wall_ms.is_some();
                let mono = self.clock.monotonic_ms();
                let journaled = inner.fire(id, Trigger::Stop { reason }, mono);
                if journaled.is_err() {
                    // The journal failed: the mission stops in memory all the same.
                    let mission = inner.mission(id)?;
                    mission.state = MissionState::Stopped;
                    mission.generation += 1;
                }
                Ok(Ok((journaled, may_have_dispatched)))
            },
        )?;
        let (journaled, may_have_dispatched) = match step {
            Ok(stopping) => stopping,
            Err(state) => return Ok(state),
        };
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

    /// Stop every mission that has not ended. Returns how many.
    pub fn stop_all(&self) -> usize {
        let live: Vec<MissionId> = self.peek(|inner| {
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .map(|(id, _)| *id)
                .collect()
        });
        live.into_iter()
            .filter(|id| self.stop(*id) == Ok(MissionState::Stopped))
            .count()
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
        self.step(|inner| {
            self.model_obligations()?;
            if inner.closed {
                return Err(CoordinatorError::Closed);
            }
            if inner.missions.values().any(|m| {
                m.file_mode
                    && (!m.state.is_terminal() || m.terminal_uncertain || m.proof.unrecorded)
            }) {
                return Err(CoordinatorError::FileUnfinished);
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
        self.step(|inner| {
            inner.clear_detail(id);
            let mission = inner.mission(id)?;
            if mission.held.is_none() {
                return Err(illegal(mission.state, "resume"));
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
        self.step(|inner| {
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
                    inner.append(id, Event::Resumed)?;
                    let mission = inner.mission(id)?;
                    mission.held = None;
                    mission.held_journaled = false;
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
/// A read-only account of verified Mother history. No port is attached, no
/// run is adopted, no consent is installed and no transition is appended.
#[derive(Clone, Debug)]
pub struct HistoryInspection {
    pub records: usize,
    pub session_end_recorded: bool,
    pub missions: Vec<(MissionId, MissionState)>,
    pub unresolved: Vec<MissionId>,
    /// Terminal records survive; lost process-local acknowledgement does not.
    pub prior_acknowledgement_unavailable: usize,
    pub details: Vec<HistoryMission>,
}

/// Verified identifiers only; no reconstructed operational authority.
#[derive(Clone, Debug)]
pub struct HistoryMission {
    pub mission: MissionId,
    pub state: MissionState,
    pub terminal: Option<(Trigger, u64)>,
    pub actions: Vec<(u64, PendingAction)>,
}

pub fn inspect_history(store: &dyn JournalStore) -> Result<HistoryInspection, JournalError> {
    inspect_history_lines(&store.load()?)
}

/// Canonical report-only inspection of an already captured source snapshot.
pub fn inspect_history_lines(lines: &[String]) -> Result<HistoryInspection, JournalError> {
    let entries = verify(lines)?;
    let mut missions = BTreeMap::new();
    let mut next_mission = 1;
    let mut session = Session::Never;
    for (index, entry) in entries.iter().enumerate() {
        check(&entry.event)
            .and_then(|()| replay(&mut missions, &mut next_mission, &mut session, entry))
            .map_err(|code| JournalError::Inconsistent {
                line: index + 1,
                code,
            })?;
    }
    Ok(HistoryInspection {
        records: entries.len(),
        session_end_recorded: session == Session::Closed,
        missions: missions.iter().map(|(id, m)| (*id, m.state)).collect(),
        unresolved: missions
            .iter()
            .filter_map(|(id, m)| {
                // Settlement retains its first cause after the terminal line.
                // Canonical replay validates that line; incomplete settlement
                // is nonterminal, and uncertain outcomes retain their own debt.
                (!m.state.is_terminal()
                    || m.unresolved_file.is_some()
                    || m.terminal_uncertain
                    || m.unsettled.is_some()
                    || m.open_action.is_some()
                    || m.pending_lane.is_some())
                .then_some(*id)
            })
            .collect(),
        prior_acknowledgement_unavailable: missions.values().map(|m| m.acks.len()).sum(),
        details: missions
            .iter()
            .map(|(id, mission)| {
                let mut actions = BTreeMap::new();
                for entry in entries.iter().filter(|entry| entry.mission == *id) {
                    match &entry.event {
                        Event::ApprovalPending { effect, pending } => {
                            actions.insert(*effect, pending.clone());
                        }
                        Event::Transition {
                            trigger:
                                Trigger::Settled {
                                    effect,
                                    action: Some(action),
                                    ..
                                },
                        } => {
                            actions.insert(*effect, action.clone());
                        }
                        _ => {}
                    }
                }
                HistoryMission {
                    mission: *id,
                    state: mission.state,
                    terminal: mission
                        .terminal_record
                        .as_ref()
                        .and_then(|(trigger, seq)| seq.map(|seq| (trigger.clone(), seq))),
                    actions: actions.into_iter().collect(),
                }
            })
            .collect(),
    })
}

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
            // A session boundary ends the invocation's charging lifetime.
            // Keep totals, declarations and pending-lane settlement evidence.
            for mission in missions.values_mut() {
                mission.invalidate_calls();
            }
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
        Event::SessionClosed
            if missions.values().any(|m| {
                m.must_hold
                    || (m.file_mode
                        && (!m.state.is_terminal()
                            || m.unresolved_file.is_some()
                            || m.terminal_uncertain))
            }) =>
        {
            return Err(V::CloseWithUnheldMission);
        }
        Event::SessionClosed => {
            // A session boundary ends the invocation's charging lifetime.
            // Keep totals, declarations and pending-lane settlement evidence.
            for mission in missions.values_mut() {
                mission.invalidate_calls();
            }
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
    if matches!(entry.event, Event::FileModeSelected) {
        if mission.file_mode || mission.state != MissionState::Draft || mission.generation != 0 {
            return Err(V::SettlementConflict);
        }
        mission.file_mode = true;
        return Ok(());
    }

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
        Event::Drafted { .. } | Event::FileModeSelected => unreachable!(),
        Event::Transition { trigger } => {
            let to = transition(mission.state, trigger).map_err(|_| V::IllegalTransition)?;
            if mission.unresolved_file.is_some()
                && matches!(
                    trigger,
                    Trigger::Stop { .. }
                        | Trigger::Block { .. }
                        | Trigger::Fail { .. }
                        | Trigger::VerifyFail { .. }
                )
            {
                return Err(V::SettlementConflict);
            }
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
                    let published = matches!(outcome, SettlementOutcome::Published { .. });
                    let expected =
                        page_proof::settled_end(mission, *effect, published && action.is_some())
                            .or(expected);
                    if expected != Some(*end) {
                        return Err(V::SettlementConflict);
                    }
                    if let Some((open, pending)) = &mission.open_action {
                        if open == effect && action.as_ref() != Some(pending) {
                            return Err(V::SettlementConflict);
                        }
                    }
                    if page_proof::early_conflict(mission, *effect, action.as_ref()) {
                        return Err(V::ProofConflict);
                    }
                }
                _ => {}
            }
            mission.apply(trigger, to, 0);
            if to.is_terminal() {
                mission.terminal_record = Some((trigger.clone(), Some(entry.seq)));
            }
            if let Trigger::Consent { hash, .. } = trigger {
                // The verified journal's record of the broker's approval.
                mission.consent = Some(Consent::minted(*hash));
            }
        }
        Event::ModelCallCharged { invocation, call } => {
            if mission.pending_lane != Some(*invocation)
                || !mission.state.is_working()
                || mission.settlement.is_some()
            {
                return Err(V::ModelChargeOutOfPlace);
            }
            mission
                .call_accounting
                .as_mut()
                .ok_or(V::ModelChargeOutOfPlace)?
                .charge(*invocation, *call, &mut mission.model_calls)?;
        }
        Event::LaneInvoked {
            invocation,
            model_cap,
            model_budget,
            ..
        } => {
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
            mission.call_accounting =
                CallAccounting::reserved(mission, *invocation, *model_cap, *model_budget)?;
            // Evidence only. Replay never constructs a live LaneCalls or meter.
            if *model_cap > 0 {
                mission.declared_model_budgets.push(*model_budget);
            }
            mission.pending_lane = Some(*invocation);
            mission.proof.returned = None;
            mission.model_calls = mission
                .model_calls
                .checked_add(1)
                .ok_or(V::ModelChargeOutOfPlace)?;
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
            mission.call_accounting = None;
            mission.prior_outputs.push(output_hash.clone());
            mission.proof.returned = Some(*invocation);
        }
        Event::LaneOvertaken { invocation } => {
            if mission.pending_lane != Some(*invocation) {
                return Err(V::LaneAnswerWithoutCall);
            }
            mission.pending_lane = None;
            mission.call_accounting = None;
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
            mission.dispatched_kind = Some(*kind);
            if mission.file_mode && *kind == EffectKind::File {
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
            if mission.file_mode
                && (mission.unresolved_file.is_some() || mission.terminal_uncertain)
            {
                return Err(V::SettlementConflict);
            }
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
            if mission.file_mode || mission.settlement.is_some() {
                return Err(V::SettlementConflict);
            }
            mission.held = None;
            mission.held_journaled = false;
        }
        Event::DeliverableRetained { .. } | Event::PublicationReceipt { .. } => {
            page_proof::replay(mission, entry)?;
        }
    }
    Ok(())
}

// A charge authorizes future work; it is never a settlement exemption.
fn journal_work(event: &Event) -> bool {
    match event {
        Event::Drafted { .. }
        | Event::FileModeSelected
        | Event::ModelCallCharged { .. }
        | Event::LaneInvoked { .. }
        | Event::LaneReturned { .. }
        | Event::EffectDispatched { .. }
        | Event::EffectSettled { .. }
        | Event::ApprovalPending { .. }
        | Event::Resumed => true,
        Event::Transition { trigger } => matches!(
            trigger,
            Trigger::Revise { .. }
                | Trigger::Submit { .. }
                | Trigger::Consent { .. }
                | Trigger::Start
                | Trigger::PhaseDone
                | Trigger::VerifyPass
                | Trigger::AwaitApproval { .. }
                | Trigger::ActionApproved { .. }
        ),
        Event::ConsentRefused { .. }
        | Event::LaneOvertaken { .. }
        | Event::EffectRefused { .. }
        | Event::ApprovalClosed { .. }
        | Event::SessionOpened
        | Event::SessionClosed
        | Event::Held { .. }
        | Event::SettlementStarted { .. }
        | Event::DeliverableRetained { .. }
        | Event::PublicationReceipt { .. } => false,
    }
}
