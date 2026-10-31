//! M1's pre-registered oracles (`MOTHER.md` §3.1): committed before the
//! implementation they judge, and not edited after a result is seen.
//! `README.md` maps each oracle to the invariant it pins.

mod consent;
mod contract;
mod deny;
mod journal;
mod lanes;
mod m2a_consent;
mod m2a_rebuild;
mod m2a_review;
mod m2a_waiting;
mod mn01;
mod review;
mod state;
mod stop;
mod structure;

use crate::contract::{Budget, EffectKind, MissionContract, PassCriterion, Question};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{
    ApprovalStatus, EffectOutcome, EffectPort, EffectRefusal, EffectRequest, PendingAction,
};
use crate::journal::{MemoryJournal, MissionId};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole, Verdict};
use crate::state::MissionState;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    MissionConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::Authority;
use nexus_governed_control::connector::ConnectorIntent;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::tool::ToolIntent;
use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest a held call waits before giving up (a safety bound: tests
/// assert a hold ended by its event, never by this).
const HOLD_BOUND: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- port

/// A call Mother made to the port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortCall {
    Perform {
        mission: MissionId,
        kind: EffectKind,
    },
    Cancel(MissionId),
    Finish(MissionId),
    /// Mother asked what became of the action awaiting the Owner's
    /// approval.
    Approval(MissionId),
}

#[derive(Default)]
struct PortState {
    calls: Vec<PortCall>,
    replies: VecDeque<Result<EffectOutcome, EffectRefusal>>,
    emergency: bool,
    hold: bool,
    entered: bool,
    cancelled: BTreeSet<MissionId>,
    released_by_event: Vec<bool>,
    approvals: VecDeque<ApprovalStatus>,
    hold_approval: bool,
    approval_entered: bool,
    pending: Option<PendingAction>,
    /// Each adoption: the mission, the action, and how many calls had been
    /// made when it came.
    adopted: Vec<(MissionId, PendingAction, usize)>,
}

/// A recording EffectPort: performs nothing, records every call, and can
/// hold the next effect until its mission is cancelled. Mission consent is
/// asked of a real broker authority (with an in-memory evidence sink), so
/// a consent is only ever made from the broker's own answer.
pub struct RecordingPort {
    state: Mutex<PortState>,
    changed: Condvar,
    pub broker: Authority,
    pub evidence: Arc<MemoryEvidence>,
}

impl Default for RecordingPort {
    fn default() -> Self {
        let evidence = Arc::new(MemoryEvidence::new(4096));
        Self {
            state: Mutex::new(PortState::default()),
            changed: Condvar::new(),
            broker: Authority::new(evidence.clone(), Arc::new(ManualClock::default())),
            evidence,
        }
    }
}

impl RecordingPort {
    fn lock(&self) -> std::sync::MutexGuard<'_, PortState> {
        self.state.lock().expect("port")
    }

    pub fn calls(&self) -> Vec<PortCall> {
        self.lock().calls.clone()
    }

    /// Every effect handed to the port.
    pub fn effects(&self) -> Vec<(MissionId, EffectKind)> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                PortCall::Perform { mission, kind } => Some((mission, kind)),
                _ => None,
            })
            .collect()
    }

    pub fn reply(&self, reply: Result<EffectOutcome, EffectRefusal>) {
        self.lock().replies.push_back(reply);
    }

    pub fn set_emergency(&self, on: bool) {
        self.lock().emergency = on;
    }

    /// The next effect blocks until its mission is cancelled.
    pub fn hold(&self) {
        self.lock().hold = true;
    }

    pub fn wait_entered(&self) -> bool {
        let deadline = Instant::now() + HOLD_BOUND;
        let mut state = self.lock();
        while !state.entered {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self.changed.wait_timeout(state, left).expect("port").0;
        }
        true
    }

    pub fn released_by_event(&self) -> Vec<bool> {
        self.lock().released_by_event.clone()
    }

    pub fn is_cancelled(&self, id: MissionId) -> bool {
        self.lock().cancelled.contains(&id)
    }

    /// What the next approval queries answer, in order (then `Waiting`).
    pub fn approval_reply(&self, status: ApprovalStatus) {
        self.lock().approvals.push_back(status);
    }

    /// The next approval query blocks until its mission is cancelled, then
    /// answers what is queued (an approval that came just as Stop did).
    pub fn hold_approval(&self) {
        self.lock().hold_approval = true;
    }

    pub fn wait_approval_entered(&self) -> bool {
        let deadline = Instant::now() + HOLD_BOUND;
        let mut state = self.lock();
        while !state.approval_entered {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self.changed.wait_timeout(state, left).expect("port").0;
        }
        true
    }

    /// What the port reports as the pending action of a waiting effect.
    pub fn set_pending(&self, pending: Option<PendingAction>) {
        self.lock().pending = pending;
    }

    pub fn adopted(&self) -> Vec<(MissionId, PendingAction, usize)> {
        self.lock().adopted.clone()
    }

    /// How many times Mother asked about an awaited approval.
    pub fn approval_queries(&self) -> usize {
        self.calls()
            .iter()
            .filter(|c| matches!(c, PortCall::Approval(_)))
            .count()
    }
}

impl EffectPort for RecordingPort {
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        let mut state = self.lock();
        state.calls.push(PortCall::Perform {
            mission,
            kind: effect.kind(),
        });
        if state.cancelled.contains(&mission) {
            return Err(refusal("mission_cancelled"));
        }
        if state.hold {
            state.hold = false;
            state.entered = true;
            self.changed.notify_all();
            let deadline = Instant::now() + HOLD_BOUND;
            while !state.cancelled.contains(&mission) {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                state = self.changed.wait_timeout(state, left).expect("port").0;
            }
            let by_event = state.cancelled.contains(&mission);
            state.released_by_event.push(by_event);
            return Err(refusal("mission_cancelled"));
        }
        state
            .replies
            .pop_front()
            .unwrap_or(Ok(EffectOutcome::Done { text: None }))
    }

    fn cancel(&self, mission: MissionId) {
        let mut state = self.lock();
        state.calls.push(PortCall::Cancel(mission));
        state.cancelled.insert(mission);
        self.changed.notify_all();
    }

    fn finish(&self, mission: MissionId) {
        self.lock().calls.push(PortCall::Finish(mission));
    }

    fn emergency_stopped(&self) -> bool {
        self.lock().emergency
    }

    fn broker(&self) -> &Authority {
        &self.broker
    }

    fn pending(&self, _: MissionId) -> Option<PendingAction> {
        self.lock().pending.clone()
    }

    fn adopt(&self, mission: MissionId, pending: &PendingAction) {
        let mut state = self.lock();
        let calls = state.calls.len();
        state.adopted.push((mission, pending.clone(), calls));
    }

    fn approval(&self, mission: MissionId) -> ApprovalStatus {
        let mut state = self.lock();
        state.calls.push(PortCall::Approval(mission));
        if state.hold_approval {
            state.hold_approval = false;
            state.approval_entered = true;
            self.changed.notify_all();
            let deadline = Instant::now() + HOLD_BOUND;
            while !state.cancelled.contains(&mission) {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                state = self.changed.wait_timeout(state, left).expect("port").0;
            }
            let by_event = state.cancelled.contains(&mission);
            state.released_by_event.push(by_event);
        }
        state
            .approvals
            .pop_front()
            .unwrap_or(ApprovalStatus::Waiting)
    }
}

pub fn refusal(class: &str) -> EffectRefusal {
    EffectRefusal {
        class: class.into(),
    }
}

// ---------------------------------------------------------------- lanes

/// One request of each kind.
pub fn effect_of(kind: EffectKind) -> EffectRequest {
    match kind {
        EffectKind::File => EffectRequest::File {
            relative_path: "index.html".into(),
            write: true,
        },
        EffectKind::Network => get("https://example.org/"),
        EffectKind::Process => EffectRequest::Process(ToolIntent {
            tool: "git".into(),
            input: serde_json::json!({}),
        }),
        EffectKind::ModelTool => EffectRequest::ModelTool(ConnectorIntent {
            operation: "llm.complete".into(),
            input: serde_json::json!({}),
        }),
        EffectKind::Spend => EffectRequest::Spend {
            amount_minor: 1,
            purpose: "domain".into(),
        },
    }
}

pub fn get(url: &str) -> EffectRequest {
    EffectRequest::Network(EgressIntent {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: None,
    })
}

/// A deterministic test lane. What it answers is decided by markers in the
/// mission's restatement, so one set of lanes serves every mission:
///
/// - `ASK-<ROLE>-<KIND>` (e.g. `ASK-BUILDER-NETWORK`): asks for that effect;
/// - `FETCH-<ROLE>:<url>`: asks for a GET of `<url>` (up to a space);
/// - `POST-<ROLE>:<url>`: asks for a POST of `<url>` (up to a space);
/// - `HALT-<ROLE>`: answers Halt;
/// - `VERIFY-FAIL`: the Verifier answers Fail;
/// - `TEXT-<ROLE>:<text>`: answers `<text>` (up to `|`).
///
/// It counts its calls.
pub struct KeyedLane {
    pub role: LaneRole,
    pub calls: AtomicUsize,
}

impl KeyedLane {
    pub fn new(role: LaneRole) -> Self {
        Self {
            role,
            calls: AtomicUsize::new(0),
        }
    }

    fn tag(&self) -> String {
        self.role.as_str().to_uppercase().replace('_', "")
    }
}

fn marker_value<'a>(text: &'a str, marker: &str, end: char) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    Some(rest.split(end).next().unwrap_or(rest))
}

impl Lane for KeyedLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.role,
            name: format!("keyed-{}", self.role.as_str()),
            version: "m1-test".into(),
        }
    }

    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let r = &input.restatement;
        let tag = self.tag();
        let mut effects = Vec::new();
        for kind in EffectKind::ALL {
            let marker = format!(
                "ASK-{tag}-{}",
                kind.as_str().to_uppercase().replace('_', "")
            );
            effects.extend(std::iter::repeat_n(
                effect_of(kind),
                r.matches(&marker).count(),
            ));
        }
        if let Some(url) = marker_value(r, &format!("FETCH-{tag}:"), ' ') {
            effects.push(get(url));
        }
        if let Some(url) = marker_value(r, &format!("POST-{tag}:"), ' ') {
            effects.push(EffectRequest::Network(EgressIntent {
                method: "POST".into(),
                url: url.into(),
                headers: vec![],
                body: None,
            }));
        }
        let verdict = if r.contains(&format!("HALT-{tag}")) {
            Verdict::Halt {
                reason: "test halt".into(),
            }
        } else if self.role == LaneRole::Verifier {
            if r.contains("VERIFY-FAIL") {
                Verdict::Fail {
                    reason: "criterion p1 does not hold".into(),
                }
            } else {
                Verdict::Pass
            }
        } else {
            Verdict::Continue
        };
        let text = marker_value(r, &format!("TEXT-{tag}:"), '|')
            .map(str::to_string)
            .unwrap_or_else(|| format!("{} looked at the goal", self.role.as_str()));
        LaneOutput {
            text,
            effects,
            verdict,
        }
    }
}

/// A lane that blocks inside its call until released.
pub struct HeldLane {
    pub role: LaneRole,
    pub effects: Vec<EffectRequest>,
    gate: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl HeldLane {
    pub fn new(role: LaneRole, effects: Vec<EffectRequest>) -> Self {
        Self {
            role,
            effects,
            gate: Mutex::new((false, false)),
            changed: Condvar::new(),
        }
    }

    pub fn wait_entered(&self) -> bool {
        let deadline = Instant::now() + HOLD_BOUND;
        let mut gate = self.gate.lock().unwrap();
        while !gate.0 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            gate = self.changed.wait_timeout(gate, left).unwrap().0;
        }
        true
    }

    pub fn release(&self) {
        self.gate.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}

impl Lane for HeldLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.role,
            name: format!("held-{}", self.role.as_str()),
            version: "m1-test".into(),
        }
    }

    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        let mut gate = self.gate.lock().unwrap();
        gate.0 = true;
        self.changed.notify_all();
        let deadline = Instant::now() + HOLD_BOUND;
        while !gate.1 {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            gate = self.changed.wait_timeout(gate, left).unwrap().0;
        }
        LaneOutput {
            text: "held lane answered".into(),
            effects: self.effects.clone(),
            verdict: Verdict::Continue,
        }
    }
}

pub fn keyed_lanes() -> Vec<Arc<dyn Lane>> {
    LaneRole::ALL
        .iter()
        .map(|role| Arc::new(KeyedLane::new(*role)) as Arc<dyn Lane>)
        .collect()
}

// ---------------------------------------------------------------- contracts

/// A complete contract whose restatement carries `markers`.
pub fn contract_with(markers: &str) -> MissionContract {
    MissionContract {
        owner_words: "  Make me a small website for my bakery, \"Rosa's\".\n  Keep it simple!  "
            .into(),
        restatement: format!("A one-page site for Rosa's bakery {markers}")
            .trim_end()
            .to_string(),
        requested_folder: Some("~/Sites/rosas-bakery".into()),
        budget: Budget {
            max_duration_ms: 3_600_000,
            max_model_calls: 50,
            max_money_minor: 0,
        },
        forbidden_effects: BTreeSet::new(),
        questions: vec![Question {
            id: "q1".into(),
            text: "Which colours do you like?".into(),
            answer: Some("Blue and cream".into()),
        }],
        pass_criteria: vec![PassCriterion {
            id: "p1".into(),
            text: "The page names the bakery".into(),
        }],
    }
}

pub fn contract() -> MissionContract {
    contract_with("")
}

// ---------------------------------------------------------------- confirmers
//
// Test stand-ins for the Owner at the broker's native dialogs. Each answers
// only the mission dialog; every other dialog is declined.

macro_rules! mission_only {
    () => {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool {
            false
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            false
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            false
        }
        fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
            false
        }
    };
}

pub struct Yes;
impl ControlConfirmer for Yes {
    mission_only!();
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
}

pub struct No;
impl ControlConfirmer for No {
    mission_only!();
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        false
    }
}

/// Says yes and keeps what it was shown.
#[derive(Default)]
pub struct Recording(pub Mutex<Vec<MissionConfirmation>>);
impl ControlConfirmer for Recording {
    mission_only!();
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        self.0.lock().unwrap().push(request.clone());
        true
    }
}

/// Runs `during` while the Owner is being asked, then says yes.
pub struct During<F: Fn(&MissionConfirmation)>(pub F);
impl<F: Fn(&MissionConfirmation)> ControlConfirmer for During<F> {
    mission_only!();
    fn confirm_mission(&self, request: &MissionConfirmation) -> bool {
        (self.0)(request);
        true
    }
}

// ---------------------------------------------------------------- world

pub struct World {
    pub port: Arc<RecordingPort>,
    pub journal: MemoryJournal,
    pub clock: Arc<ManualClock>,
    pub lanes: Vec<Arc<dyn Lane>>,
    /// The keyed lanes, when the world was built with them.
    pub keyed: Vec<Arc<KeyedLane>>,
    pub mother: Arc<Coordinator<RecordingPort>>,
}

pub fn world() -> World {
    let keyed: Vec<Arc<KeyedLane>> = LaneRole::ALL
        .iter()
        .map(|role| Arc::new(KeyedLane::new(*role)))
        .collect();
    let mut w = world_with(keyed.iter().map(|l| l.clone() as Arc<dyn Lane>).collect());
    w.keyed = keyed;
    w
}

pub fn world_with(lanes: Vec<Arc<dyn Lane>>) -> World {
    let port = Arc::new(RecordingPort::default());
    let journal = MemoryJournal::new();
    let clock = Arc::new(ManualClock::default());
    let mother = Arc::new(
        Coordinator::open(
            journal.clone(),
            port.clone(),
            Lanes::new(lanes.clone()).unwrap(),
            clock.clone(),
        )
        .unwrap(),
    );
    World {
        port,
        journal,
        clock,
        lanes,
        keyed: vec![],
        mother,
    }
}

impl World {
    /// Mother restarts: a new coordinator from the same journal (the port
    /// and the clock carry on).
    pub fn reopen(&self) -> Result<Coordinator<RecordingPort>, CoordinatorError> {
        Coordinator::open(
            self.journal.clone(),
            self.port.clone(),
            Lanes::new(self.lanes.clone()).unwrap(),
            self.clock.clone(),
        )
    }

    pub fn restart(&self) -> World {
        World {
            port: self.port.clone(),
            journal: self.journal.clone(),
            clock: self.clock.clone(),
            lanes: self.lanes.clone(),
            keyed: self.keyed.clone(),
            mother: Arc::new(self.reopen().unwrap()),
        }
    }

    /// Calls made to the keyed lanes.
    pub fn keyed_calls(&self) -> usize {
        self.keyed
            .iter()
            .map(|l| l.calls.load(Ordering::SeqCst))
            .sum()
    }

    pub fn submitted(&self, contract: MissionContract) -> MissionId {
        let id = self.mother.draft(contract).unwrap();
        self.mother.submit(id).unwrap();
        id
    }

    pub fn approved(&self, contract: MissionContract) -> MissionId {
        let id = self.submitted(contract);
        assert_eq!(
            self.mother.request_consent(id, &Yes),
            Ok(MissionState::Approved)
        );
        id
    }

    /// A mission in `state`, reached the way it is reached in use.
    pub fn mission_in(&self, state: MissionState) -> MissionId {
        let advanced = |markers: &str, steps: usize| {
            let id = self.approved(contract_with(markers));
            for _ in 0..steps {
                self.mother.advance(id).unwrap();
            }
            id
        };
        let id = match state {
            MissionState::Draft => self.mother.draft(contract()).unwrap(),
            MissionState::AwaitingConsent => self.submitted(contract()),
            MissionState::Approved => self.approved(contract()),
            MissionState::Investigating => advanced("", 1),
            MissionState::Building => advanced("", 2),
            MissionState::Verifying => advanced("", 3),
            MissionState::Explaining => advanced("", 4),
            MissionState::Done => advanced("", 5),
            MissionState::Failed => advanced("VERIFY-FAIL", 4),
            MissionState::Blocked => advanced("HALT-SENTINEL", 2),
            MissionState::Denied => {
                let id = self.submitted(contract());
                assert_eq!(
                    self.mother.request_consent(id, &No),
                    Err(CoordinatorError::Denied)
                );
                id
            }
            MissionState::Stopped => {
                let id = self.approved(contract());
                self.mother.stop(id).unwrap();
                id
            }
            MissionState::AwaitingApproval => {
                let id = advanced("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS", 2);
                self.port.reply(Ok(EffectOutcome::AwaitingApproval));
                self.mother.advance(id).unwrap();
                id
            }
        };
        assert_eq!(self.mother.state(id), Some(state), "reaching {state:?}");
        id
    }
}
