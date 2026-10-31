//! The coordinator: turns a contract into a governed mission.
//!
//! Every change is journaled before it takes effect. Nothing reaches the
//! EffectPort before the Owner consented to the mission's exact contract
//! hash, and nothing reaches it after Stop. No coordinator lock is held
//! while the Owner is asked, while a lane runs or while an effect runs: a
//! call is reserved under the lock, made with the lock released, and its
//! result is kept only if the mission did not move meanwhile. Stop never
//! waits for a lane or an effect. (The journal append is the transition's
//! commit, a bounded local write, and is made under the lock so that the
//! journal's order is the state machine's.)

use crate::consent::{Consent, MissionConfirmation, MissionConfirmer};
use crate::contract::{sha256, ContractError, ContractHash, EffectKind, MissionContract};
use crate::effect::{EffectOutcome, EffectPort, EffectRequest};
use crate::journal::{verify, Entry, Event, JournalError, JournalStore, MissionId, GENESIS};
use crate::lane::{
    input_hash, output_hash, InvocationId, Lane, LaneIdentity, LaneInput, LaneRole, Verdict,
};
use crate::state::{transition, IllegalTransition, MissionState, Trigger};
use nexus_governed_control::authority::clock::Clock;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// The most missions one coordinator keeps.
pub const MAX_MISSIONS: usize = 1024;
/// The most effects one lane may ask for in one call.
pub const MAX_EFFECTS_PER_CALL: usize = 16;

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
                return Err(CoordinatorError::Lanes(format!(
                    "two lanes for {}",
                    role.as_str()
                )));
            }
        }
        if let Some(missing) = LaneRole::ALL.iter().find(|r| !by_role.contains_key(r)) {
            return Err(CoordinatorError::Lanes(format!(
                "no lane for {}",
                missing.as_str()
            )));
        }
        Ok(Self { by_role })
    }

    fn get(&self, role: LaneRole) -> &Arc<dyn Lane> {
        &self.by_role[&role]
    }
}

/// Why the coordinator refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatorError {
    UnknownMission,
    Contract(ContractError),
    Journal(JournalError),
    Illegal(IllegalTransition),
    /// The lanes are not exactly one per role.
    Lanes(String),
    Capacity,
    /// Work was asked of a mission without a consent covering its contract.
    NoConsent,
    /// The Owner's answer came for a contract or state that had moved on.
    ConsentStale,
    Denied,
    Stopped,
    /// Another call is already working on this mission.
    Busy,
}

/// One lane call, as reported.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// What one [`Coordinator::advance`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advance {
    pub from: MissionState,
    pub to: MissionState,
    pub lanes: Vec<LaneReport>,
    pub effects: Vec<EffectReport>,
    /// Set when the mission reaches Done: what Mother tells the Owner.
    pub explanation: Option<String>,
    /// Set when the mission ends Blocked or Failed.
    pub reason: Option<String>,
}

struct Mission {
    contract: MissionContract,
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
}

impl Mission {
    fn new(contract: MissionContract) -> Self {
        Self {
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
        }
    }

    /// Take a transition the table accepted.
    fn apply(&mut self, trigger: &Trigger, to: MissionState, mono_now: u64) {
        match trigger {
            Trigger::Revise { contract } => {
                self.contract = contract.clone();
                self.consent = None;
                self.approved_wall_ms = None;
            }
            Trigger::Consent { hash, at_wall_ms } => {
                self.consent = Some(Consent::minted(*hash));
                self.approved_wall_ms = Some(*at_wall_ms);
                self.mono_ref = mono_now;
                self.spent_at_ref = 0;
            }
            Trigger::VerifyFail { reason }
            | Trigger::Block { reason }
            | Trigger::Fail { reason } => {
                self.reason = Some(reason.clone());
            }
            _ => {}
        }
        self.state = to;
        self.generation += 1;
        self.dispatched_in_phase = false;
    }

    fn consented(&self) -> bool {
        self.consent
            .as_ref()
            .is_some_and(|consent| consent.covers(&self.contract))
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

    fn out_of_time(&self, clock: &dyn Clock) -> bool {
        self.spent_ms(clock) >= self.contract.budget.max_duration_ms
    }
}

struct Inner {
    store: Box<dyn JournalStore>,
    head: String,
    next_seq: u64,
    /// The store failed: nothing more is journaled or started.
    broken: bool,
    missions: BTreeMap<MissionId, Mission>,
    next_mission: u64,
}

impl Inner {
    fn append(&mut self, mission: MissionId, event: Event) -> Result<u64, CoordinatorError> {
        if self.broken {
            return Err(CoordinatorError::Journal(JournalError::Store(
                "the journal failed earlier".into(),
            )));
        }
        let entry = Entry::chained(self.next_seq, &self.head, mission, event);
        if let Err(e) = self.store.append(&entry.to_line()) {
            // Fail closed, durably: a journal that may have lost a
            // transition (a Stop) must not be reopened as if it had not.
            self.broken = true;
            let _ = self.store.poison();
            return Err(CoordinatorError::Journal(e));
        }
        self.head = entry.hash;
        self.next_seq += 1;
        Ok(entry.seq)
    }

    fn mission(&mut self, id: MissionId) -> Result<&mut Mission, CoordinatorError> {
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
        let from = self.mission(id)?.state;
        let to = transition(from, &trigger).map_err(|e| refusal(from, e))?;
        self.append(
            id,
            Event::Transition {
                trigger: trigger.clone(),
            },
        )?;
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

/// Mother's own refusal of a lane's effect, before any port call.
fn mother_refuses(contract: &MissionContract, effect: &EffectRequest) -> Option<String> {
    let kind = effect.kind();
    if contract.forbidden_effects.contains(&kind) {
        return Some(format!("forbidden_effect:{}", kind.as_str()));
    }
    match effect {
        EffectRequest::Spend { .. } if contract.budget.max_money_minor == 0 => {
            Some("budget_money_zero".into())
        }
        EffectRequest::Spend { .. } => Some("spend_not_supported".into()),
        EffectRequest::File { .. } if contract.requested_folder.is_none() => {
            Some("no_requested_folder".into())
        }
        EffectRequest::File { relative_path, .. } if !relative_request(relative_path) => {
            Some("file_request_not_relative".into())
        }
        _ => None,
    }
}

fn request_hash(effect: &EffectRequest) -> String {
    let json = serde_json::to_vec(effect).expect("an effect request serialises");
    hex::encode(sha256(REQUEST_DOMAIN, &[&json]))
}

/// The coordinator.
pub struct Coordinator<P: EffectPort> {
    port: Arc<P>,
    lanes: Lanes,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

/// Clears a mission's busy mark however its call ends.
struct Busy<'a, P: EffectPort> {
    coordinator: &'a Coordinator<P>,
    id: MissionId,
}

impl<P: EffectPort> Drop for Busy<'_, P> {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.coordinator.inner.lock() {
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
    pub fn open(
        store: impl JournalStore + 'static,
        port: Arc<P>,
        lanes: Lanes,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, CoordinatorError> {
        let lines = store.load().map_err(CoordinatorError::Journal)?;
        let entries = verify(&lines).map_err(CoordinatorError::Journal)?;
        let mut missions = BTreeMap::new();
        let mut next_mission = 1;
        for (index, entry) in entries.iter().enumerate() {
            replay(&mut missions, &mut next_mission, entry).map_err(|reason| {
                CoordinatorError::Journal(JournalError::Inconsistent {
                    line: index + 1,
                    reason,
                })
            })?;
        }
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
        let coordinator = Self {
            port,
            lanes,
            clock,
            inner: Mutex::new(Inner {
                store: Box::new(store),
                head: entries
                    .last()
                    .map_or(GENESIS.to_string(), |e| e.hash.clone()),
                next_seq: entries.len() as u64,
                broken: false,
                missions,
                next_mission,
            }),
        };
        // A lane call left unanswered by the restart is abandoned.
        {
            let mut inner = coordinator.lock();
            let pending: Vec<(MissionId, InvocationId)> = inner
                .missions
                .iter()
                .filter_map(|(id, m)| m.pending_lane.map(|inv| (*id, inv)))
                .collect();
            for (id, invocation) in pending {
                inner.append(id, Event::LaneOvertaken { invocation })?;
                inner.mission(id)?.pending_lane = None;
            }
        }
        // A mission interrupted after dispatching an effect in its current
        // phase is blocked: an unsettled effect may have happened, and
        // re-running the phase would repeat a settled one.
        let interrupted: Vec<(MissionId, &str)> = {
            let inner = coordinator.lock();
            inner
                .missions
                .iter()
                .filter(|(_, m)| !m.state.is_terminal())
                .filter_map(|(id, m)| {
                    if m.unsettled.is_some() {
                        Some((*id, "effect_outcome_unknown"))
                    } else if m.dispatched_in_phase {
                        Some((*id, "phase_interrupted"))
                    } else {
                        None
                    }
                })
                .collect()
        };
        for (id, reason) in interrupted {
            coordinator.end(id, reason)?;
        }
        Ok(coordinator)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("mother coordinator")
    }

    /// Draft a mission. Nothing reaches the port.
    pub fn draft(&self, contract: MissionContract) -> Result<MissionId, CoordinatorError> {
        contract.validate().map_err(CoordinatorError::Contract)?;
        let mut inner = self.lock();
        if inner.missions.len() >= MAX_MISSIONS {
            return Err(CoordinatorError::Capacity);
        }
        let id = MissionId(inner.next_mission);
        inner.append(
            id,
            Event::Drafted {
                contract: contract.clone(),
            },
        )?;
        inner.next_mission += 1;
        inner.missions.insert(id, Mission::new(contract));
        Ok(id)
    }

    /// Change a mission's contract before work starts: it returns to Draft
    /// and any consent is void.
    pub fn revise(
        &self,
        id: MissionId,
        contract: MissionContract,
    ) -> Result<MissionState, CoordinatorError> {
        let mut inner = self.lock();
        let state = inner.mission(id)?.state;
        transition(
            state,
            &Trigger::Revise {
                contract: contract.clone(),
            },
        )
        .map_err(|e| refusal(state, e))?;
        contract.validate().map_err(CoordinatorError::Contract)?;
        let mono = self.clock.monotonic_ms();
        inner.fire(id, Trigger::Revise { contract }, mono)
    }

    /// Put a complete contract (every question answered) to the Owner.
    pub fn submit(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        if mission.state != MissionState::Draft {
            return Err(illegal(mission.state, "submit"));
        }
        mission
            .contract
            .validate_for_consent()
            .map_err(CoordinatorError::Contract)?;
        let hash = mission.contract.hash();
        let mono = self.clock.monotonic_ms();
        inner.fire(id, Trigger::Submit { hash }, mono)
    }

    /// What the Owner would be shown.
    pub fn confirmation(&self, id: MissionId) -> Result<MissionConfirmation, CoordinatorError> {
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        Ok(MissionConfirmation::of(id, &mission.contract))
    }

    /// Ask the Owner. No lock is held while the Owner answers; a yes counts
    /// only if the mission is still awaiting consent to the same hash.
    pub fn request_consent(
        &self,
        id: MissionId,
        confirmer: &dyn MissionConfirmer,
    ) -> Result<MissionState, CoordinatorError> {
        let (request, generation) = {
            let mut inner = self.lock();
            let mission = inner.mission(id)?;
            if mission.state != MissionState::AwaitingConsent {
                return Err(illegal(mission.state, "consent"));
            }
            (
                MissionConfirmation::of(id, &mission.contract),
                mission.generation,
            )
        };
        let yes = confirmer.confirm_mission(&request);
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        if mission.generation != generation
            || mission.state != MissionState::AwaitingConsent
            || mission.contract.hash() != request.hash
        {
            let stopped = mission.state == MissionState::Stopped;
            inner.append(
                id,
                Event::ConsentRefused {
                    reason: "the mission changed while the Owner was asked".into(),
                },
            )?;
            return Err(if stopped {
                CoordinatorError::Stopped
            } else {
                CoordinatorError::ConsentStale
            });
        }
        let mono = self.clock.monotonic_ms();
        if yes {
            let at_wall_ms = self.clock.wall_ms();
            inner.fire(
                id,
                Trigger::Consent {
                    hash: request.hash,
                    at_wall_ms,
                },
                mono,
            )
        } else {
            inner.fire(id, Trigger::Deny { hash: request.hash }, mono)?;
            Err(CoordinatorError::Denied)
        }
    }

    /// Move a consented mission one state forward, running that state's
    /// lanes and dispatching the effects they ask for that pass every
    /// check.
    pub fn advance(&self, id: MissionId) -> Result<Advance, CoordinatorError> {
        let emergency = self.port.emergency_stopped();
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        if mission.busy {
            return Err(CoordinatorError::Busy);
        }
        let from = mission.state;
        if from != MissionState::Approved && !from.is_working() {
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
            drop(inner);
            self.stop_for(id, "emergency_stop")?;
            return Err(CoordinatorError::Stopped);
        }
        if mission.out_of_time(&*self.clock) {
            drop(inner);
            report.to = self.end(id, "budget_time")?;
            report.reason = Some("budget_time".into());
            return Ok(report);
        }
        let mono = self.clock.monotonic_ms();
        let roles: &[LaneRole] = match from {
            MissionState::Approved => {
                report.to = inner.fire(id, Trigger::Start, mono)?;
                return Ok(report);
            }
            MissionState::Explaining => {
                let explanation = self.explain(id, inner.mission(id)?);
                report.to = inner.fire(id, Trigger::PhaseDone, mono)?;
                report.explanation = Some(explanation);
                drop(inner);
                self.port.finish(id);
                return Ok(report);
            }
            MissionState::Investigating => &[
                LaneRole::Sentinel,
                LaneRole::ResearcherA,
                LaneRole::ResearcherB,
                LaneRole::Skeptic,
            ],
            MissionState::Building => &[LaneRole::Sentinel, LaneRole::Builder],
            _ => &[LaneRole::Sentinel, LaneRole::Verifier],
        };
        let mission = inner.mission(id)?;
        mission.busy = true;
        let generation = mission.generation;
        drop(inner);
        let _busy = Busy {
            coordinator: self,
            id,
        };

        match self.run_lanes(id, generation, roles, &mut report)? {
            Ended::Ended(state) => {
                report.to = state;
                report.reason = self.reason(id);
                return Ok(report);
            }
            Ended::Through => {}
        }
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        if mission.generation != generation {
            return Err(CoordinatorError::Stopped);
        }
        let trigger = if from == MissionState::Verifying {
            Trigger::VerifyPass
        } else {
            Trigger::PhaseDone
        };
        let mono = self.clock.monotonic_ms();
        report.to = inner.fire(id, trigger, mono)?;
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
        for role in roles {
            let lane = self.lanes.get(*role);
            let identity = lane.identity();
            if self.port.emergency_stopped() {
                self.stop_for(id, "emergency_stop")?;
                return Err(CoordinatorError::Stopped);
            }
            // Reserve the call.
            let input = {
                let mut inner = self.lock();
                let seq = inner.next_seq;
                let mission = inner.mission(id)?;
                if mission.generation != generation {
                    return Err(CoordinatorError::Stopped);
                }
                let reason = if identity.role != *role {
                    Some("lane_identity_changed")
                } else if mission.model_calls >= mission.contract.budget.max_model_calls {
                    Some("budget_model_calls")
                } else if mission.out_of_time(&*self.clock) {
                    Some("budget_time")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    drop(inner);
                    return Ok(Ended::Ended(self.end(id, reason)?));
                }
                let input = LaneInput {
                    invocation: InvocationId { mission: id, seq },
                    role: *role,
                    contract_hash: mission.contract.hash(),
                    owner_words: mission.contract.owner_words.clone(),
                    restatement: mission.contract.restatement.clone(),
                    pass_criteria: mission.contract.pass_criteria.clone(),
                    prior_outputs: mission.prior_outputs.clone(),
                };
                inner.append(
                    id,
                    Event::LaneInvoked {
                        invocation: input.invocation,
                        identity: identity.clone(),
                        input_hash: input_hash(&input),
                    },
                )?;
                inner.mission(id)?.model_calls += 1;
                input
            };

            let output = lane.invoke(&input);

            // Keep the answer only if the reservation still holds.
            let hash = output_hash(&output);
            let contract = {
                let mut inner = self.lock();
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
                mission.contract.clone()
            };
            report.lanes.push(LaneReport {
                invocation: input.invocation,
                identity,
                text: output.text.clone(),
                verdict: output.verdict.clone(),
                input_hash: input_hash(&input),
                output_hash: hash,
            });

            // The verdict.
            let verdict_end = match (&output.verdict, *role) {
                (Verdict::Halt { reason }, role) => Some(Trigger::Block {
                    reason: format!("{}_halt:{reason}", role.as_str()),
                }),
                (Verdict::Fail { reason }, LaneRole::Verifier) => Some(Trigger::VerifyFail {
                    reason: reason.clone(),
                }),
                (Verdict::Fail { reason }, _) => Some(Trigger::Fail {
                    reason: reason.clone(),
                }),
                (Verdict::Continue, LaneRole::Verifier) => Some(Trigger::VerifyFail {
                    reason: "verifier_gave_no_verdict".into(),
                }),
                _ => None,
            };
            if let Some(trigger) = verdict_end {
                return Ok(Ended::Ended(self.end_with(id, trigger)?));
            }

            // Mother's own checks of every effect, before any is sent.
            if output.effects.len() > MAX_EFFECTS_PER_CALL {
                return Ok(Ended::Ended(self.end(id, "too_many_effects")?));
            }
            for effect in &output.effects {
                if let Some(reason) = mother_refuses(&contract, effect) {
                    self.lock().append(
                        id,
                        Event::EffectRefused {
                            kind: effect.kind(),
                            reason: reason.clone(),
                        },
                    )?;
                    report.effects.push(EffectReport {
                        kind: effect.kind(),
                        dispatched: false,
                        outcome: reason.clone(),
                    });
                    return Ok(Ended::Ended(self.end(id, &reason)?));
                }
            }

            // Dispatch, one at a time.
            for effect in &output.effects {
                if self.port.emergency_stopped() {
                    self.stop_for(id, "emergency_stop")?;
                    return Err(CoordinatorError::Stopped);
                }
                let number = {
                    let mut inner = self.lock();
                    let mission = inner.mission(id)?;
                    if mission.generation != generation {
                        return Err(CoordinatorError::Stopped);
                    }
                    if mission.out_of_time(&*self.clock) {
                        drop(inner);
                        return Ok(Ended::Ended(self.end(id, "budget_time")?));
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
                    mission.dispatched_in_phase = true;
                    number
                };

                let outcome = self.port.perform(id, effect);

                let settled = match &outcome {
                    Ok(EffectOutcome::Done { .. }) => "done".to_string(),
                    Ok(EffectOutcome::AwaitingApproval) => "awaiting_approval".to_string(),
                    Err(refusal) => format!("refused:{}", refusal.class),
                };
                {
                    let mut inner = self.lock();
                    inner.append(
                        id,
                        Event::EffectSettled {
                            effect: number,
                            outcome: settled.clone(),
                        },
                    )?;
                    let mission = inner.mission(id)?;
                    mission.unsettled = None;
                    if mission.generation != generation {
                        return Err(CoordinatorError::Stopped);
                    }
                }
                report.effects.push(EffectReport {
                    kind: effect.kind(),
                    dispatched: true,
                    outcome: settled.trim_start_matches("refused:").to_string(),
                });
                match outcome {
                    Ok(EffectOutcome::Done { .. }) => {}
                    Ok(EffectOutcome::AwaitingApproval) => {
                        return Ok(Ended::Ended(self.end(id, "awaiting_owner_approval")?));
                    }
                    Err(refusal) => {
                        if self.port.emergency_stopped() {
                            self.stop_for(id, "emergency_stop")?;
                            return Err(CoordinatorError::Stopped);
                        }
                        let reason = format!("broker_refused:{}", refusal.class);
                        return Ok(Ended::Ended(self.end(id, &reason)?));
                    }
                }
            }
        }
        Ok(Ended::Through)
    }

    /// Block a mission for `reason` and cancel what it opened at the port.
    fn end(&self, id: MissionId, reason: &str) -> Result<MissionState, CoordinatorError> {
        self.end_with(
            id,
            Trigger::Block {
                reason: reason.into(),
            },
        )
    }

    /// End a mission with `trigger` (Block, Fail or VerifyFail) and cancel
    /// what it opened at the port.
    fn end_with(&self, id: MissionId, trigger: Trigger) -> Result<MissionState, CoordinatorError> {
        let mono = self.clock.monotonic_ms();
        let result = {
            let mut inner = self.lock();
            let from = inner.mission(id)?.state;
            let result = inner.fire(id, trigger.clone(), mono);
            if let (Err(CoordinatorError::Journal(_)), Ok(to)) =
                (&result, transition(from, &trigger))
            {
                // The journal failed: the mission ends in memory all the
                // same (and the poisoned journal will not reopen).
                inner.mission(id)?.apply(&trigger, to, mono);
            }
            result
        };
        // Whatever the journal said, nothing more of it may start.
        self.port.cancel(id);
        result
    }

    fn explain(&self, id: MissionId, mission: &Mission) -> String {
        let mut lines = vec![
            format!("Mission {id} is done."),
            format!("You asked: {}", mission.contract.owner_words.trim()),
            format!("I understood: {}", mission.contract.restatement),
            "The Verifier checked every pass criterion you set before work began:".into(),
        ];
        for criterion in &mission.contract.pass_criteria {
            lines.push(format!("  - {}: holds", criterion.text));
        }
        lines.push(format!(
            "Lane calls: {} of {}. Effects sent to the broker: {}. Money spent: 0.",
            mission.model_calls, mission.contract.budget.max_model_calls, mission.next_effect
        ));
        lines.join("\n")
    }

    /// Stop a mission from whatever state it is in; a terminal mission
    /// stays as it ended. Never waits for a lane or an effect.
    pub fn stop(&self, id: MissionId) -> Result<MissionState, CoordinatorError> {
        self.stop_for(id, "owner")
    }

    fn stop_for(&self, id: MissionId, reason: &str) -> Result<MissionState, CoordinatorError> {
        let mut inner = self.lock();
        let mission = inner.mission(id)?;
        if mission.state.is_terminal() {
            return Ok(mission.state);
        }
        let may_have_dispatched = mission.approved_wall_ms.is_some();
        let mono = self.clock.monotonic_ms();
        let journaled = inner.fire(
            id,
            Trigger::Stop {
                reason: reason.into(),
            },
            mono,
        );
        if journaled.is_err() {
            // The journal failed: the mission stops in memory all the same.
            let mission = inner.mission(id)?;
            mission.state = MissionState::Stopped;
            mission.generation += 1;
        }
        drop(inner);
        if may_have_dispatched {
            self.port.cancel(id);
        }
        journaled
    }

    /// Stop every mission that has not ended. Returns how many.
    pub fn stop_all(&self) -> usize {
        let live: Vec<MissionId> = self
            .lock()
            .missions
            .iter()
            .filter(|(_, m)| !m.state.is_terminal())
            .map(|(id, _)| *id)
            .collect();
        live.into_iter()
            .filter(|id| self.stop(*id) == Ok(MissionState::Stopped))
            .count()
    }

    pub fn state(&self, id: MissionId) -> Option<MissionState> {
        self.lock().missions.get(&id).map(|m| m.state)
    }

    pub fn contract(&self, id: MissionId) -> Option<MissionContract> {
        self.lock().missions.get(&id).map(|m| m.contract.clone())
    }

    /// The hash the mission's consent covers, if it holds one.
    pub fn consented_hash(&self, id: MissionId) -> Option<ContractHash> {
        self.lock()
            .missions
            .get(&id)
            .and_then(|m| m.consent.as_ref().map(Consent::hash))
    }

    /// Why a mission ended Blocked or Failed.
    pub fn reason(&self, id: MissionId) -> Option<String> {
        self.lock().missions.get(&id).and_then(|m| m.reason.clone())
    }

    pub fn missions(&self) -> Vec<MissionId> {
        self.lock().missions.keys().copied().collect()
    }

    /// The verified journal.
    pub fn journal(&self) -> Result<Vec<Entry>, CoordinatorError> {
        let lines = self
            .lock()
            .store
            .load()
            .map_err(CoordinatorError::Journal)?;
        verify(&lines).map_err(CoordinatorError::Journal)
    }
}

/// Replay one verified entry. Each trigger goes through the same table as
/// live, and records must come in the order the live coordinator writes
/// them; anything else found is refused.
fn replay(
    missions: &mut BTreeMap<MissionId, Mission>,
    next_mission: &mut u64,
    entry: &Entry,
) -> Result<(), String> {
    let id = entry.mission;
    if let Event::Drafted { contract } = &entry.event {
        if id.0 != *next_mission {
            return Err(format!("{id} drafted twice or out of order"));
        }
        if missions.len() >= MAX_MISSIONS {
            return Err(format!("{id}: more missions than a coordinator keeps"));
        }
        contract
            .validate()
            .map_err(|e| format!("{id}: drafted contract invalid: {e:?}"))?;
        missions.insert(id, Mission::new(contract.clone()));
        *next_mission = id.0 + 1;
        return Ok(());
    }
    let mission = missions
        .get_mut(&id)
        .ok_or_else(|| format!("{id} was never drafted"))?;
    match &entry.event {
        Event::Drafted { .. } => unreachable!(),
        Event::Transition { trigger } => {
            let to = transition(mission.state, trigger)
                .map_err(|e| format!("{id}: {} from {:?}", e.trigger, e.from))?;
            // While an effect is unsettled the live coordinator only stops,
            // or blocks the mission on restart.
            let while_unsettled = matches!(trigger, Trigger::Stop { .. })
                || matches!(trigger, Trigger::Block { reason } if reason == "effect_outcome_unknown");
            if mission.unsettled.is_some() && !while_unsettled {
                return Err(format!("{id}: a transition while an effect is unsettled"));
            }
            let hash = mission.contract.hash();
            match trigger {
                Trigger::Revise { contract } => contract
                    .validate()
                    .map_err(|e| format!("{id}: revised contract invalid: {e:?}"))?,
                Trigger::Submit { hash: h } => {
                    mission
                        .contract
                        .validate_for_consent()
                        .map_err(|e| format!("{id}: submitted incomplete: {e:?}"))?;
                    if *h != hash {
                        return Err(format!("{id}: submit names another hash"));
                    }
                }
                Trigger::Consent { hash: h, .. } | Trigger::Deny { hash: h } if *h != hash => {
                    return Err(format!("{id}: consent names another hash"));
                }
                Trigger::Start if !mission.consented() => {
                    return Err(format!("{id}: work without consent"));
                }
                _ => {}
            }
            mission.apply(trigger, to, 0);
        }
        Event::LaneInvoked { invocation, .. } => {
            if !mission.state.is_working()
                || mission.state == MissionState::Explaining
                || mission.pending_lane.is_some()
                || invocation.mission != id
                || invocation.seq != entry.seq
            {
                return Err(format!("{id}: a lane invocation out of place"));
            }
            mission.pending_lane = Some(*invocation);
            mission.model_calls = mission.model_calls.saturating_add(1);
        }
        Event::LaneReturned {
            invocation,
            output_hash,
        } => {
            if mission.pending_lane != Some(*invocation) {
                return Err(format!("{id}: a lane answer without its call"));
            }
            mission.pending_lane = None;
            mission.prior_outputs.push(output_hash.clone());
        }
        Event::LaneOvertaken { invocation } => {
            if mission.pending_lane != Some(*invocation) {
                return Err(format!("{id}: a lane answer without its call"));
            }
            mission.pending_lane = None;
        }
        Event::EffectDispatched { effect, .. } => {
            if !mission.state.is_working()
                || *effect != mission.next_effect
                || mission.unsettled.is_some()
            {
                return Err(format!("{id}: an effect dispatch out of place"));
            }
            mission.next_effect += 1;
            mission.unsettled = Some(*effect);
            mission.dispatched_in_phase = true;
        }
        Event::EffectSettled { effect, .. } => {
            if mission.unsettled != Some(*effect) {
                return Err(format!("{id}: an effect settled that was not dispatched"));
            }
            mission.unsettled = None;
        }
        Event::ConsentRefused { .. } | Event::EffectRefused { .. } => {}
    }
    Ok(())
}
