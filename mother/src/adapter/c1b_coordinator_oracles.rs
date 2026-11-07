//! Coordinator-owned C1b-2 oracles; N on a3dd4a11 (missing LaneContext/caller).
//! MP-E23 -> E-O30b; MP-E24 -> E-O33; MP-E28 -> E-O17-C1; clear cause -> C1-S6.
use crate::adapter::c1b_fake_ollama::{
    chat, spec, Answer, FakeOllama, Seen, DIGEST, NAME, PATIENCE,
};
use crate::adapter::c1b_port_oracles::{children as run_children, record_next, Fx as PortFx};
use crate::adapter::m2c_support::{port_for, Home};
use crate::adapter::GovernedPort;
use crate::contract::{ContractError, MissionContract, ModelChoice, Note};
use crate::coordinator::{
    inspect_history_lines, Coordinator, CoordinatorError, FileSession, Lanes,
};
use crate::effect::EffectPort;
use crate::journal::{verify, Event, JournalError, JournalStore, MemoryJournal, MissionId};
use crate::lane::{
    Lane, LaneContext, LaneIdentity, LaneInput, LaneOutput, LaneRole, ModelCallError, Verdict,
};
use crate::oracles::lite_support::file;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, keyed_lanes, Recording};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::evidence::EndStatus;
use nexus_governed_control::authority::file_session::{FileSessionError, HaltCell};
use nexus_governed_control::governed::HaltReason;
use nexus_governed_control::inference::{ModelIdentity, ModelReply, ModelRequest, ResponseSchema};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const CASE: &str = "NEXUS_M2E_C1B_CASE";
/// A cancelled, stopped or expired held request returns within this.
const BOUND: Duration = Duration::from_secs(2);
const NOTE: &str = "C1B_NOTE_CANARY";
const PROMPT: &str = "C1B_PROMPT_CANARY";
const REPLY: &str = "C1B_REPLY_CANARY";
const ERROR: &str = "C1B_ERROR_CANARY";

// ---------------------------------------------------------------- harness

fn children(test: &str, cases: &[&str]) {
    run_children("adapter::c1b_coordinator_oracles", CASE, test, cases, run);
}

fn run(case: &str) {
    let (name, arg) = case.split_once('/').unwrap_or((case, ""));
    match name {
        "deadline" => deadline(arg),
        "locks" => locks(),
        "refusals" => refusals(),
        "interrupt" => interrupt(arg),
        "late" => late(),
        "privacy" => privacy(arg),
        "identity" => identity(arg),
        "debt" => debt(arg),
        "close" => close_active(),
        "restart" => restart(),
        "first" => first_cause(arg),
        "race" => terminal_race(arg),
        "sealed" => sealed(),
        other => panic!("unknown case {other}"),
    }
}

type Script = Box<dyn Fn(&LaneContext<'_>) -> LaneOutput + Send + Sync>;

/// The Builder with a model cap; its behaviour is the case's script.
struct ModelLane {
    cap: u32,
    script: Script,
    invoked: Arc<AtomicUsize>,
}
impl Lane for ModelLane {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "c1b-model".into(),
            version: "1".into(),
        }
    }
    fn max_model_calls(&self) -> u32 {
        self.cap
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        panic!("a model lane is invoked with its context")
    }
    fn invoke_in(&self, _: &LaneInput, context: &LaneContext<'_>) -> LaneOutput {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        (self.script)(context)
    }
}

fn request(user: &str) -> ModelRequest {
    let schema = ResponseSchema::new(r#"{"type":"object"}"#).unwrap();
    ModelRequest::new("C1B system".into(), user.into(), schema).unwrap()
}
fn ask(context: &LaneContext<'_>) -> Result<ModelReply, ModelCallError> {
    context
        .model
        .expect("a model lane gets a caller")
        .call(&request(PROMPT))
}
fn outcome(result: &Result<ModelReply, ModelCallError>) -> String {
    match result {
        Ok(_) => "ok".into(),
        Err(ModelCallError::Invalidated) => "invalidated".into(),
        Err(ModelCallError::Refused(class)) => format!("refused:{class}"),
        Err(ModelCallError::Failed(class)) => format!("failed:{class}"),
        Err(ModelCallError::RecordUnavailable) => "record_unavailable".into(),
    }
}
fn draft(content: &str) -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "c1b model lane".into(),
        effects: vec![file("index.html", &format!("{content}\n"))],
        verdict: Verdict::Continue,
    }
}
fn quiet() -> LaneOutput {
    LaneOutput {
        verification: None,
        text: "c1b model lane".into(),
        effects: vec![],
        verdict: Verdict::Continue,
    }
}
pub(crate) fn within<T>(rx: &Receiver<T>) -> T {
    rx.recv_timeout(PATIENCE)
        .expect("the schedule step happens")
}
/// `body` completes on another thread within [`BOUND`]: it is not blocked
/// by a lock the calling schedule holds.
fn completes(what: &str, body: impl FnOnce() + Send + 'static) {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        body();
        let _ = tx.send(());
    });
    assert!(rx.recv_timeout(BOUND).is_ok(), "blocked: {what}");
}
fn identity_hex() -> String {
    hex::encode(ModelIdentity::from_spec(&spec()).unwrap().digest())
}
/// Runs a hook once, under the coordinator lock, just before the first
/// journal line that contains its needle is appended.
type ProbeHook = (&'static str, Box<dyn FnOnce() + Send>);
#[derive(Clone, Default)]
pub(crate) struct Probe(Arc<Mutex<Vec<ProbeHook>>>);
impl Probe {
    pub(crate) fn at(&self, needle: &'static str, hook: impl FnOnce() + Send + 'static) {
        self.0.lock().unwrap().push((needle, Box::new(hook)));
    }
}
pub(crate) struct ProbeJournal {
    pub(crate) journal: MemoryJournal,
    pub(crate) probe: Probe,
}
impl JournalStore for ProbeJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.journal.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let hook = {
            let mut hooks = self.probe.0.lock().unwrap();
            let at = hooks.iter().position(|(needle, _)| line.contains(needle));
            at.map(|at| hooks.remove(at).1)
        };
        if let Some(hook) = hook {
            hook();
        }
        self.journal.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.journal.poison()
    }
}

pub(crate) struct Fx {
    pub(crate) home: Home,
    pub(crate) fake: Arc<FakeOllama>,
    pub(crate) port: Arc<GovernedPort>,
    pub(crate) journal: MemoryJournal,
    pub(crate) probe: Probe,
    pub(crate) clock: Arc<ManualClock>,
    pub(crate) folder: String,
    invoked: Arc<AtomicUsize>,
}
impl Fx {
    pub(crate) fn new() -> Self {
        Self::with(true)
    }
    pub(crate) fn with(register: bool) -> Self {
        let base = PortFx::with(register);
        let (home, fake, port, folder) = (base.home, Arc::new(base.fake), base.port, base.folder);
        Self {
            home,
            fake,
            port,
            journal: MemoryJournal::new(),
            probe: Probe::default(),
            clock: Arc::new(ManualClock::default()),
            folder,
            invoked: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn mother(
        &self,
        cap: u32,
        script: impl Fn(&LaneContext<'_>) -> LaneOutput + Send + Sync + 'static,
    ) -> Arc<FileSession<GovernedPort>> {
        let mut lanes: Vec<Arc<dyn Lane>> = keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::Builder)
            .collect();
        lanes.push(Arc::new(ModelLane {
            cap,
            script: Box::new(script),
            invoked: self.invoked.clone(),
        }));
        let store = ProbeJournal {
            journal: self.journal.clone(),
            probe: self.probe.clone(),
        };
        Arc::new(
            Coordinator::open(
                store,
                self.port.clone(),
                Lanes::new(lanes).unwrap(),
                self.clock.clone(),
                self.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        )
    }
    pub(crate) fn contract(&self) -> MissionContract {
        let mut c = contract_with("");
        c.requested_folder = Some(self.folder.clone());
        c.budget.max_duration_ms = 60_000;
        c.model = Some(ModelChoice {
            name: NAME.into(),
            digest: DIGEST.into(),
            identity: identity_hex(),
        });
        c.deliverable = Some("index.html".into());
        c
    }
    /// Consented and advanced to Building: the next advance invokes the
    /// model lane. Returns the native request the Owner was asked.
    pub(crate) fn building(
        &self,
        m: &FileSession<GovernedPort>,
        c: MissionContract,
    ) -> (
        MissionId,
        nexus_governed_control::authority::approval::MissionConfirmation,
    ) {
        let id = m.draft(c).unwrap();
        m.submit(id).unwrap();
        let owner = Recording::default();
        assert_eq!(
            answering(&owner, || m.request_consent(id)),
            Ok(MissionState::Approved)
        );
        m.advance(id).unwrap();
        m.advance(id).unwrap();
        assert_eq!(m.state(id).unwrap(), Some(MissionState::Building));
        let asked = owner.0.lock().unwrap()[0].clone();
        (id, asked)
    }
    pub(crate) fn events(&self) -> Vec<Event> {
        verify(&self.journal.lines())
            .unwrap()
            .into_iter()
            .map(|e| e.event)
            .collect()
    }
    pub(crate) fn charges(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| matches!(e, Event::ModelCallCharged { .. }))
            .count()
    }
    pub(crate) fn dispatched(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| matches!(e, Event::EffectDispatched { .. }))
            .count()
    }
    /// Answers journaled for the model lane's invocations.
    pub(crate) fn model_lane_returned(&self) -> usize {
        let events = self.events();
        let model: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::LaneInvoked {
                    invocation,
                    model_cap,
                    ..
                } if *model_cap > 0 => Some(*invocation),
                _ => None,
            })
            .collect();
        events
            .iter()
            .filter(|e| matches!(e, Event::LaneReturned { invocation, .. } if model.contains(invocation)))
            .count()
    }
    pub(crate) fn model_invocations(&self) -> usize {
        self.events()
            .iter()
            .filter(|e| matches!(e, Event::LaneInvoked { model_cap, .. } if *model_cap > 0))
            .count()
    }
    /// The terminal transition of `id`, as `kind:code`.
    pub(crate) fn end(&self, id: MissionId) -> String {
        let entries = verify(&self.journal.lines()).unwrap();
        let trigger = entries
            .iter()
            .filter(|e| e.mission == id)
            .filter_map(|e| match &e.event {
                Event::Transition { trigger } => Some(trigger.clone()),
                _ => None,
            })
            .next_back()
            .unwrap();
        match trigger {
            Trigger::Stop { reason } => format!("stop:{reason}"),
            Trigger::Block { reason } => format!("block:{reason}"),
            Trigger::Fail { reason } => format!("fail:{reason}"),
            other => other.name().to_string(),
        }
    }
    fn terminal_journaled(journal: &MemoryJournal, id: MissionId) -> bool {
        verify(&journal.lines()).unwrap().iter().any(|e| {
            e.mission == id
                && matches!(&e.event, Event::Transition { trigger } if matches!(
                    trigger,
                    Trigger::Stop { .. } | Trigger::Block { .. } | Trigger::Fail { .. }
                ))
        })
    }
    pub(crate) fn attempts(&self) -> usize {
        self.home.gate.count("model_call")
    }
    pub(crate) fn index_exists(&self) -> bool {
        self.home.dir.join("workspace/index.html").exists()
    }
    pub(crate) fn hold(&self, path: &'static str) {
        self.fake
            .route(move |at| (at == path).then_some(Answer::Hold));
        self.fake.drain();
    }
}
pub(crate) fn advance_in(
    m: &Arc<FileSession<GovernedPort>>,
    id: MissionId,
) -> std::thread::JoinHandle<Result<crate::coordinator::Advance, CoordinatorError>> {
    let m = m.clone();
    std::thread::spawn(move || m.advance(id))
}
// ---------------------------------------------------------------- E-O17-C1

/// Either mission clock expires while version or chat is held: Timeout
/// within the bound, the peer sees Closed, the mission ends budget_time.
/// The delay before the second call extends nothing (one absolute deadline).
#[test]
fn e_o17_c1_absolute_mission_deadline_ends_held_requests() {
    children(
        "e_o17_c1_absolute_mission_deadline_ends_held_requests",
        &[
            "deadline/mono-chat",
            "deadline/mono-version",
            "deadline/wall-chat",
            "deadline/wall-version",
        ],
    );
}
fn deadline(arg: &str) {
    let (clock, held) = arg.split_once('-').unwrap();
    let path: &'static str = if held == "chat" {
        "/api/chat"
    } else {
        "/api/version"
    };
    let f = Fx::new();
    let (tx, rx) = channel();
    let (go_tx, go_rx) = sync_channel::<()>(1);
    let go_rx = Mutex::new(go_rx);
    let m = f.mother(2, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        go_rx.lock().unwrap().recv_timeout(PATIENCE).unwrap();
        tx.send(outcome(&ask(ctx))).unwrap();
        draft("after the deadline")
    });
    let (id, _) = f.building(&m, f.contract());
    let adv = advance_in(&m, id);
    assert_eq!(within(&rx), "ok", "first call");
    // A delay between the calls, short of the deadline.
    f.clock.advance(Duration::from_millis(59_000));
    f.hold(path);
    go_tx.send(()).unwrap();
    assert!(f.fake.wait(Seen::Request(path.into())));
    let at = Instant::now();
    if clock == "mono" {
        f.clock.advance(Duration::from_millis(2_000));
    } else {
        f.clock.suspend(Duration::from_millis(2_000));
    }
    assert_eq!(within(&rx), "failed:model_timeout");
    assert!(at.elapsed() < BOUND, "the fixed deadline ends the request");
    assert!(f.fake.wait(Seen::Closed(path.into())));
    assert!(adv.join().unwrap().is_err());
    assert_eq!(f.end(id), "block:budget_time");
    let charged = if held == "chat" { 2 } else { 1 };
    assert_eq!(
        f.charges(),
        charged,
        "charged before the chat; never refunded"
    );
    assert!(!f.index_exists());
}

// ---------------------------------------------------------------- E-O24-C1

/// No coordinator, port or broker lock is held across the meter's callback,
/// the held chat or the broker's attempt record.
#[test]
fn e_o24_c1_no_lock_across_meter_transport_or_evidence() {
    children(
        "e_o24_c1_no_lock_across_meter_transport_or_evidence",
        &["locks"],
    );
}
fn locks() {
    let f = Fx::new();
    let (tx, rx) = channel();
    let m = f.mother(2, move |ctx| {
        tx.send(outcome(&ask(ctx))).unwrap();
        tx.send(outcome(&ask(ctx))).unwrap();
        quiet()
    });
    let (id, _) = f.building(&m, f.contract());
    let probes = |label: &'static str, f: &Fx, m: Option<&Arc<FileSession<GovernedPort>>>| {
        let port = f.port.clone();
        completes(label, move || {
            port.file_binding_count();
        });
        let control = f.home.control.clone();
        completes(label, move || {
            control.authority().runs().views();
        });
        if let Some(m) = m {
            let m = m.clone();
            completes(label, move || {
                m.state(id).unwrap();
            });
        }
    };
    // (a) Inside the meter: its journal append holds the coordinator lock,
    // inside the broker's call; the port and the broker's runs are free.
    let (seen_tx, seen_rx) = channel();
    {
        let (port, control) = (f.port.clone(), f.home.control.clone());
        f.probe.at("\"model_call_charged\"", move || {
            completes("port in the meter", move || {
                port.file_binding_count();
            });
            completes("broker in the meter", move || {
                control.authority().runs().views();
            });
            seen_tx.send(()).unwrap();
        });
    }
    let (entered_tx, entered_rx) = sync_channel(2);
    let (release_tx, release_rx) = sync_channel::<()>(2);
    let release_rx = Mutex::new(release_rx);
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            let release = release_rx.lock().unwrap();
            let _ = entered_tx.send(());
            let _ = release.recv_timeout(PATIENCE);
            chat("{}")
        })
    });
    let adv = advance_in(&m, id);
    within(&seen_rx);
    within(&entered_rx);
    // (b) The chat is held: nothing of Mother's or the broker's is locked.
    probes("during the held chat", &f, Some(&m));
    // (c) The broker's attempt record is held at its writer.
    let (held_tx, held_rx) = sync_channel(1);
    f.home.gate.arm_with("model_call", None, move || {
        let _ = held_tx.send(());
    });
    release_tx.send(()).unwrap();
    within(&held_rx);
    assert!(f.home.gate.wait_held());
    probes("during the attempt record", &f, Some(&m));
    f.home.gate.open();
    assert_eq!(within(&rx), "ok");
    release_tx.send(()).unwrap();
    assert_eq!(within(&rx), "ok");
    adv.join().unwrap().unwrap();
}

// ---------------------------------------------------------------- E-O32

/// Refusals in the middle of a lane: failed metadata (no charge, no chat),
/// a post-charge failure (charged, not refunded), a success, and a refused
/// meter (no chat). The lane's cost is max(1, k) for k charged calls.
#[test]
fn e_o32_refusals_mid_lane_send_nothing_and_refund_nothing() {
    children(
        "e_o32_refusals_mid_lane_send_nothing_and_refund_nothing",
        &["refusals"],
    );
}
fn refusals() {
    let f = Fx::new();
    let (tx, rx) = channel();
    let fake = f.fake.clone();
    let m = f.mother(2, move |ctx| {
        for step in ["version", "chat500", "healthy", "exhausted"] {
            match step {
                "version" => fake.route(|p| {
                    (p == "/api/version")
                        .then(|| Answer::Json(200, r#"{"version":"0.40.12"}"#.into()))
                }),
                "chat500" => {
                    fake.route(|p| (p == "/api/chat").then(|| Answer::Json(500, "{}".into())))
                }
                _ => fake.route(|_| None),
            }
            tx.send((outcome(&ask(ctx)), fake.count("/api/chat")))
                .unwrap();
        }
        quiet()
    });
    let (id, _) = f.building(&m, f.contract());
    let adv = advance_in(&m, id);
    for (want, chats) in [
        ("failed:model_runtime_mismatch", 0),
        ("failed:model_unavailable", 1),
        ("ok", 2),
        ("failed:model_over_budget", 2),
    ] {
        assert_eq!(within(&rx), (want.to_string(), chats));
    }
    assert!(adv.join().unwrap().is_err());
    assert_eq!(f.end(id), "block:budget_model_calls");
    let charges: Vec<u32> = f
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::ModelCallCharged { call, .. } => Some(*call),
            _ => None,
        })
        .collect();
    assert_eq!(charges, vec![1, 2], "the failed chat stays charged");
    assert_eq!(f.model_lane_returned(), 0);
}

// ---------------------------------------------------------------- E-O34

/// A held chat or held metadata request, then the Owner's Stop, a door
/// halt, the emergency stop or a run cancellation: the call ends within
/// the bound, the peer sees Closed, no later chat or charge, no File, and
/// the first cause is the mission's end.
#[test]
fn e_o34_stop_halt_emergency_cancel_end_a_held_call() {
    children(
        "e_o34_stop_halt_emergency_cancel_end_a_held_call",
        &[
            "interrupt/stop-chat",
            "interrupt/stop-tags",
            "interrupt/halt-chat",
            "interrupt/halt-tags",
            "interrupt/emergency-chat",
            "interrupt/emergency-tags",
            "interrupt/cancel-chat",
            "interrupt/cancel-tags",
        ],
    );
}
fn interrupt(arg: &str) {
    let (how, held) = arg.split_once('-').unwrap();
    let path: &'static str = if held == "chat" {
        "/api/chat"
    } else {
        "/api/tags"
    };
    let f = Fx::new();
    let (tx, rx) = channel();
    let fake = f.fake.clone();
    let m = f.mother(2, move |ctx| {
        let first = outcome(&ask(ctx));
        let chats = fake.count("/api/chat");
        let second = outcome(&ask(ctx));
        tx.send((first, second, fake.count("/api/chat") - chats))
            .unwrap();
        draft("after the interruption")
    });
    let (id, _) = f.building(&m, f.contract());
    f.hold(path);
    let adv = advance_in(&m, id);
    assert!(f.fake.wait(Seen::Request(path.into())));
    let charged = f.charges();
    assert_eq!(charged, usize::from(held == "chat"));
    let at = Instant::now();
    match how {
        "stop" => assert_eq!(m.stop(id), Ok(MissionState::Stopped)),
        "halt" => crate::adapter::written(f.port.door.halt(HaltReason::JournalFailed).unwrap()),
        "emergency" => {
            f.home.control.emergency_stop();
        }
        "cancel" => f
            .home
            .control
            .cancel_run(f.port.run_of(id).unwrap())
            .unwrap(),
        other => panic!("{other}"),
    }
    let (first, second, more) = within(&rx);
    assert!(at.elapsed() < BOUND, "the held request ends promptly");
    let expected = if how == "emergency" {
        "failed:model_stopped"
    } else {
        "failed:model_cancelled"
    };
    assert_eq!(first, expected);
    assert!(second != "ok" && more == 0, "no later chat: {second}");
    assert!(f.fake.wait(Seen::Closed(path.into())));
    assert_eq!(f.charges(), charged, "no later charge");
    let _ = adv.join().unwrap();
    assert!(!f.index_exists(), "no File effect");
    assert!(m.state(id).unwrap().is_some_and(|s| s.is_terminal()));
    match how {
        "stop" => assert_eq!(f.end(id), "stop:owner"),
        "halt" => assert_eq!(f.end(id), "stop:door_halted"),
        "emergency" => assert_eq!(f.end(id), "stop:emergency_stop"),
        _ => assert!(
            !f.end(id).starts_with("stop:"),
            "a broker cancellation is not a Mother Stop"
        ),
    }
    if how != "cancel" {
        assert_eq!(f.dispatched(), 0, "no File request after invalidation");
        assert_eq!(f.model_lane_returned(), 0);
    }
}

// ---------------------------------------------------------------- E-O35

/// Return/overtake invalidates the invocation while the File cell/run stay live.
/// Release a successful broker reply afterward: no content, output hash or File.
#[test]
fn e_o35_a_reply_released_after_invalidation_is_never_used() {
    children(
        "e_o35_a_reply_released_after_invalidation_is_never_used",
        &["late"],
    );
}
fn late() {
    let f = Fx::new();
    let seen = Arc::new(Mutex::new(None));
    let (returned_tx, returned_rx) = sync_channel::<()>(1);
    let s = seen.clone();
    let m = f.mother(1, move |ctx| {
        let r = ask(ctx);
        let content = r
            .as_ref()
            .is_ok_and(|reply| reply.content().contains(REPLY));
        *s.lock().unwrap() = Some((outcome(&r), content));
        let _ = returned_tx.send(());
        draft(&r.map(ModelReply::into_content).unwrap_or_default())
    });
    let (id, _) = f.building(&m, f.contract());
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel::<()>(1);
    let release_rx = Mutex::new(release_rx);
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| {
            let release = release_rx.lock().unwrap();
            let _ = entered_tx.send(());
            let _ = release.recv_timeout(PATIENCE);
            chat(REPLY)
        })
    });
    let (replied_tx, replied_rx) = sync_channel(1);
    m.hooks_for_test().model_replied.arm(0, move || {
        replied_tx.send(true).unwrap();
    });
    let adv = advance_in(&m, id);
    within(&entered_rx);
    m.overtake_model_for_test(id).unwrap();
    assert!(
        !m.cell_for_test().is_engaged(),
        "reachable invalidation without File Stop"
    );
    release_tx.send(()).unwrap();
    within(&returned_rx);
    assert_eq!(
        *seen.lock().unwrap(),
        Some(("invalidated".to_string(), false)),
        "a successful reply released after invalidation never reaches the lane"
    );
    assert!(adv.join().unwrap().is_err());
    assert_eq!(f.attempts(), 1, "the broker's attempt record remains");
    assert_eq!(f.model_lane_returned(), 0);
    assert_eq!(f.dispatched(), 0);
    assert!(!f.index_exists());
    assert!(!f.journal.lines().concat().contains(REPLY));
    // Prove successful transport independently of Mother's rejected result.
    assert!(
        within(&replied_rx),
        "broker released a recorded successful reply"
    );
}

// ---------------------------------------------------------------- E-O40

/// Note, prompt, reply and error canaries: absent from the journal, the
/// broker's stores, Advance, errors and diagnostics, on a recorded success
/// and error, an owed record, and a failed charge append.
#[test]
fn e_o40_c1_canaries_stay_out_of_records_and_diagnostics() {
    children(
        "e_o40_c1_canaries_stay_out_of_records_and_diagnostics",
        &["privacy/replied", "privacy/owed", "privacy/journal"],
    );
}
fn privacy(arg: &str) {
    let f = Fx::new();
    let (tx, rx) = channel();
    let journal = f.journal.clone();
    let mode = arg.to_string();
    let m = f.mother(2, move |ctx| {
        if mode == "journal" {
            journal.fail_after(0);
        }
        let first = ask(ctx);
        let second = ask(ctx);
        let saw = first.as_ref().is_ok_and(|r| r.content().contains(REPLY));
        let errors = format!(
            "{:?} {:?} {first:?} {second:?}",
            first.as_ref().err(),
            second.as_ref().err()
        );
        tx.send((errors, saw)).unwrap();
        quiet()
    });
    let mut c = f.contract();
    c.notes = vec![Note {
        label: "notes".into(),
        text: format!("{NOTE} 面包"),
    }];
    let (id, _) = f.building(&m, c);
    let calls = AtomicUsize::new(0);
    f.fake.route(move |p| {
        (p == "/api/chat").then(|| match calls.fetch_add(1, Ordering::SeqCst) {
            0 => chat(REPLY),
            _ => Answer::Json(500, format!("{{\"error\":\"{ERROR}\"}}")),
        })
    });
    if arg == "owed" {
        record_next(&f.port, EndStatus::Owed);
    }
    let advance = advance_in(&m, id).join().unwrap();
    let (errors, saw) = within(&rx);
    assert_eq!(
        saw,
        arg == "replied",
        "only the lane, on a recorded attempt, sees the reply"
    );
    if arg == "journal" {
        assert_eq!(
            f.fake.count("/api/chat"),
            0,
            "an unrecorded charge sends nothing"
        );
    }
    let mut stores = String::new();
    let mut pending = vec![f.home.dir.join("broker")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                stores.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
            }
        }
    }
    let texts = [
        errors,
        format!("{advance:?}"),
        format!("{:?}", m.reason(id)),
        f.journal.lines().concat(),
        stores,
    ];
    for text in &texts {
        for canary in [NOTE, PROMPT, REPLY, ERROR, "面包"] {
            assert!(!text.contains(canary), "{canary} leaked");
            assert!(
                !text.contains(&hex::encode(canary)),
                "{canary} leaked (hex)"
            );
        }
    }
}

// ---------------------------------------------------------------- E-O42

/// The contract's identity differs from the port's, is unpinned, or the
/// port has none: a stable refusal before inference, with no other model or
/// run. The broker independently refuses a mismatched identity.
#[test]
fn e_o42_identity_mismatch_refuses_before_inference() {
    children(
        "e_o42_identity_mismatch_refuses_before_inference",
        &["identity/changed", "identity/unpinned", "identity/absent"],
    );
}
fn identity(arg: &str) {
    let f = Fx::with(arg != "absent");
    let m = f.mother(2, |ctx| {
        let _ = ask(ctx);
        draft("x")
    });
    let mut c = f.contract();
    match arg {
        "changed" => c.model.as_mut().unwrap().identity = "cd".repeat(32),
        "unpinned" => c.deliverable = None,
        _ => {}
    }
    if arg == "absent" {
        let id = m.draft(c).unwrap();
        assert_eq!(
            m.submit(id),
            Err(CoordinatorError::Contract(
                ContractError::ModelLaneUnavailable
            ))
        );
        assert!(f.port.model_identity().is_none());
    } else {
        let (id, _) = f.building(&m, c);
        let _ = m.advance(id);
        let code = if arg == "changed" {
            "model_identity_changed"
        } else {
            "model_not_pinned"
        };
        assert_eq!(f.end(id), format!("block:{code}"));
        assert_eq!(f.model_invocations(), 0, "no model invocation is reserved");
    }
    assert_eq!(f.invoked.load(Ordering::SeqCst), 0);
    assert_eq!(f.fake.count("/api/chat"), 0);
    assert_eq!(f.attempts(), 0);
}

// ---------------------------------------------------------------- C1-S3

/// Recorded is the only status whose reply reaches a lane. Owed or
/// Uncertain discards the content, invalidates the invocation, keeps the
/// charge, ends the mission with the closed evidence failure, refuses later
/// work and a clean close. A pre-attempt refusal is distinct.
#[test]
fn c1_s3_evidence_debt_never_yields_a_reply() {
    children(
        "c1_s3_evidence_debt_never_yields_a_reply",
        &[
            "debt/recorded",
            "debt/owed",
            "debt/uncertain",
            "debt/pre-attempt",
        ],
    );
}
fn debt(arg: &str) {
    let f = Fx::new();
    let (tx, rx) = channel();
    let mode = arg.to_string();
    let (control, port) = (f.home.control.clone(), f.port.clone());
    let m = f.mother(2, move |ctx| {
        if mode == "pre-attempt" {
            control
                .cancel_run(port.run_of(MissionId(1)).unwrap())
                .unwrap();
        }
        let first = ask(ctx);
        let second = outcome(&ask(ctx));
        tx.send((outcome(&first), second)).unwrap();
        draft(&first.map(ModelReply::into_content).unwrap_or_default())
    });
    let (id, _) = f.building(&m, f.contract());
    assert!(
        f.port.model_identity().is_some(),
        "the ordinary route's positive control"
    );
    match arg {
        "owed" => record_next(&f.port, EndStatus::Owed),
        "uncertain" => record_next(&f.port, EndStatus::Uncertain),
        _ => {}
    }
    let advanced = m.advance(id);
    let (first, second) = within(&rx);
    match arg {
        "recorded" => {
            assert_eq!((first.as_str(), second.as_str()), ("ok", "ok"));
            advanced.unwrap();
            assert_eq!(f.model_lane_returned(), 1);
            assert_eq!(f.charges(), 2);
        }
        "pre-attempt" => {
            assert!(first.starts_with("refused:") && second.starts_with("refused:"));
            assert_ne!(advanced.err(), Some(CoordinatorError::Recording));
            assert_eq!(f.charges(), 0);
            assert_eq!(f.attempts(), 0);
        }
        _ => {
            assert_eq!(
                (first.as_str(), second.as_str()),
                ("record_unavailable", "invalidated")
            );
            assert_eq!(advanced.err(), Some(CoordinatorError::Recording));
            assert_eq!(f.end(id), "block:broker_refused:evidence_unavailable");
            assert_eq!(f.model_lane_returned(), 0);
            assert_eq!(f.dispatched(), 0);
            assert_eq!(f.charges(), 1, "the charge is retained");
            assert_eq!(f.attempts(), 1, "the broker's own record is untouched");
            assert!(m.draft(f.contract()).is_err(), "later work is refused");
            assert_eq!(m.close(), Err(CoordinatorError::Recording));
            assert!(!f.events().iter().any(|e| matches!(e, Event::SessionClosed)));
        }
    }
}

// ---------------------------------------------------------------- C1-S5

/// No clean close while a call is active; after it is joined and recorded,
/// the clean close is the positive control.
#[test]
fn c1_s5_no_clean_close_with_an_active_call() {
    children("c1_s5_no_clean_close_with_an_active_call", &["close"]);
}
fn close_active() {
    let f = Fx::new();
    let m = f.mother(1, |ctx| {
        let _ = ask(ctx);
        quiet()
    });
    let (id, _) = f.building(&m, f.contract());
    f.hold("/api/chat");
    let adv = advance_in(&m, id);
    assert!(f.fake.wait(Seen::Request("/api/chat".into())));
    assert!(m.close().is_err(), "an active call");
    f.home.gate.arm("model_call", None);
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    assert!(f.home.gate.wait_held());
    assert!(
        m.close().is_err(),
        "terminal but transport/record return outstanding"
    );
    f.home.gate.open();
    assert!(adv.join().unwrap().is_err());
    assert_eq!(f.attempts(), 1);
    assert_eq!(m.close(), Ok(()), "joined and recorded: the clean close");
    assert!(f.events().iter().any(|e| matches!(e, Event::SessionClosed)));
}

/// Interrupted accounting stays unresolved and report-only across repeated
/// starts: no new invocation, charge, chat or attempt is made by them.
#[test]
fn c1_s5_interrupted_accounting_restores_no_authority() {
    children(
        "c1_s5_interrupted_accounting_restores_no_authority",
        &["restart"],
    );
}
fn restart() {
    let f = Fx::new();
    let m = f.mother(2, |ctx| {
        let _ = ask(ctx);
        let _ = ask(ctx);
        quiet()
    });
    let (id, _) = f.building(&m, f.contract());
    f.hold("/api/chat");
    let adv = advance_in(&m, id);
    assert!(f.fake.wait(Seen::Request("/api/chat".into())));
    let mut lines = f.journal.lines();
    let counted = |lines: &[String]| {
        verify(lines)
            .unwrap()
            .iter()
            .filter(|e| {
                matches!(
                    e.event,
                    Event::LaneInvoked { .. } | Event::ModelCallCharged { .. }
                )
            })
            .count()
    };
    let before = counted(&lines);
    for _start in 0..2 {
        let report = inspect_history_lines(&lines).unwrap();
        assert!(
            report.unresolved.contains(&id),
            "the interrupted call stays unresolved"
        );
        assert!(!report.session_end_recorded);
        let copy = MemoryJournal::new();
        for line in &lines {
            copy.clone().append(line).unwrap();
        }
        let port = Arc::new(port_for(&f.home.control, true).with_workspace(f.folder.clone()));
        // A File session can never be reopened: the repeated start is refused
        // and restores no coordinator, so nothing can adopt the interrupted run.
        let reopened = Coordinator::open(
            copy.clone(),
            port,
            Lanes::new(keyed_lanes()).unwrap(),
            f.clock.clone(),
            f.home.private(),
        );
        assert!(
            matches!(
                reopened,
                Err(CoordinatorError::FileSession(FileSessionError::Reopened))
            ),
            "the repeated start is refused as a reopened File session"
        );
        lines = copy.lines();
        assert_eq!(counted(&lines), before, "no invocation or charge restored");
    }
    assert!(inspect_history_lines(&lines)
        .unwrap()
        .unresolved
        .contains(&id));
    assert_eq!(f.fake.count("/api/chat"), 1);
    // A refused File reopening halts the live door (file_session.rs `attach`/
    // `port`), so the held call's own record may land at any moment; it is
    // not asserted absent here. The restarts add none: exactly one after join.
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    let _ = adv.join().unwrap();
    assert_eq!(
        f.attempts(),
        1,
        "only the held call's own record, no restart's"
    );
    assert_eq!(f.fake.count("/api/chat"), 1);
}

// ---------------------------------------------------------------- C1-S6

/// The first disposition is the mission's end; the recording fact is kept
/// separately. Denial is engaged before the generation moves.
#[test]
fn c1_s6_first_cause_and_separate_recording_facts() {
    children(
        "c1_s6_first_cause_and_separate_recording_facts",
        &[
            "first/debt-then-stop",
            "first/stop-then-debt",
            "first/budget-then-stop",
        ],
    );
}
fn first_cause(arg: &str) {
    let f = Fx::new();
    let (tx, rx) = channel();
    let (go_tx, go_rx) = sync_channel::<()>(1);
    let go_rx = Mutex::new(go_rx);
    let cell: Arc<OnceLock<HaltCell>> = Arc::default();
    let (c, journal, waits) = (cell.clone(), f.journal.clone(), arg != "stop-then-debt");
    let m = f.mother(1, move |ctx| {
        let first = outcome(&ask(ctx));
        let engaged = c.get().unwrap().is_engaged();
        let terminal = Fx::terminal_journaled(&journal, MissionId(1));
        tx.send((first, engaged, terminal)).unwrap();
        if waits {
            go_rx.lock().unwrap().recv_timeout(PATIENCE).unwrap();
        }
        draft("x")
    });
    cell.set(m.cell_for_test()).unwrap();
    let (id, _) = f.building(&m, f.contract());
    match arg {
        "debt-then-stop" => {
            record_next(&f.port, EndStatus::Owed);
            let adv = advance_in(&m, id);
            let (first, engaged, terminal) = within(&rx);
            assert_eq!(first, "record_unavailable");
            assert!(engaged && !terminal, "denial before the generation moves");
            assert_eq!(m.stop(id), Ok(MissionState::Blocked), "Stop arrives second");
            go_tx.send(()).unwrap();
            assert_eq!(adv.join().unwrap().err(), Some(CoordinatorError::Recording));
            assert_eq!(f.end(id), "block:broker_refused:evidence_unavailable");
        }
        "stop-then-debt" => {
            record_next(&f.port, EndStatus::Owed);
            let (entered_tx, entered_rx) = sync_channel(1);
            let (release_tx, release_rx) = sync_channel::<()>(1);
            let release_rx = Mutex::new(release_rx);
            f.fake.route(move |p| {
                (p == "/api/chat").then(|| {
                    let release = release_rx.lock().unwrap();
                    let _ = entered_tx.send(());
                    let _ = release.recv_timeout(PATIENCE);
                    chat("{}")
                })
            });
            let adv = advance_in(&m, id);
            within(&entered_rx);
            // The Owner's Stop engages first, then pauses before it records.
            let (paused_tx, paused_rx) = sync_channel(1);
            let (resume_tx, resume_rx) = sync_channel::<()>(1);
            m.hooks_for_test().file_after_halt.arm(0, move || {
                paused_tx.send(()).unwrap();
                let _ = resume_rx.recv_timeout(PATIENCE);
            });
            let stopper = {
                let m = m.clone();
                std::thread::spawn(move || m.stop(id))
            };
            within(&paused_rx);
            release_tx.send(()).unwrap();
            let (first, engaged, _) = within(&rx);
            assert_eq!(first, "record_unavailable");
            assert!(engaged);
            assert_eq!(adv.join().unwrap().err(), Some(CoordinatorError::Recording));
            resume_tx.send(()).unwrap();
            assert_eq!(stopper.join().unwrap(), Ok(MissionState::Stopped));
            assert_eq!(f.end(id), "stop:owner", "the first cause is kept");
        }
        _ => {
            f.hold("/api/chat");
            let adv = advance_in(&m, id);
            assert!(f.fake.wait(Seen::Request("/api/chat".into())));
            f.clock.advance(Duration::from_millis(61_000));
            let (first, engaged, terminal) = within(&rx);
            assert_eq!(first, "failed:model_timeout");
            assert!(engaged && !terminal, "denial before the generation moves");
            assert_eq!(m.stop(id), Ok(MissionState::Blocked), "Stop arrives second");
            go_tx.send(()).unwrap();
            assert!(adv.join().unwrap().is_err());
            assert_eq!(f.end(id), "block:budget_time");
        }
    }
    let recording = arg != "budget-then-stop";
    assert_eq!(
        m.close().err() == Some(CoordinatorError::Recording),
        recording,
        "the recording fact is separate"
    );
}

/// Complete the model-lane cap/failure/denial versus Stop ordering matrix.
#[test]
fn c1_s6_cap_failure_denial_racing_stop() {
    children(
        "c1_s6_cap_failure_denial_racing_stop",
        &[
            "race/cap-first",
            "race/stop-cap",
            "race/fail-first",
            "race/stop-fail",
            "race/deny-first",
            "race/stop-deny",
        ],
    );
}
fn terminal_race(arg: &str) {
    let f = Fx::new();
    let stop_first = arg.starts_with("stop-");
    let kind = if stop_first {
        &arg[5..]
    } else {
        arg.strip_suffix("-first").unwrap()
    }
    .to_string();
    let (ready_tx, ready_rx) = sync_channel(1);
    let (go_tx, go_rx) = sync_channel(1);
    let go_rx = Mutex::new(go_rx);
    let m = f.mother(1, move |ctx| {
        if stop_first {
            ready_tx.send(()).unwrap();
            go_rx.lock().unwrap().recv_timeout(PATIENCE).unwrap();
        }
        let mut out = quiet();
        match kind.as_str() {
            "cap" => {
                let _ = ask(ctx);
                let _ = ask(ctx);
            }
            "fail" => {
                out.verdict = Verdict::Fail {
                    reason: ERROR.into(),
                }
            }
            "deny" => {
                out.verdict = Verdict::Halt {
                    reason: ERROR.into(),
                }
            }
            _ => unreachable!(),
        }
        out
    });
    let (id, _) = f.building(&m, f.contract());
    let (paused_tx, paused_rx) = sync_channel(1);
    let (resume_tx, resume_rx) = sync_channel(1);
    if !stop_first {
        m.hooks_for_test().file_after_halt.arm(0, move || {
            paused_tx.send(()).unwrap();
            resume_rx.recv_timeout(PATIENCE).unwrap();
        });
    }
    let adv = advance_in(&m, id);
    if stop_first {
        within(&ready_rx);
    } else {
        within(&paused_rx);
        assert!(m.cell_for_test().is_engaged());
        assert!(!Fx::terminal_journaled(&f.journal, id));
    }
    let state = m.stop(id).unwrap();
    if stop_first {
        assert_eq!(state, MissionState::Stopped);
        go_tx.send(()).unwrap();
    } else {
        assert_eq!(
            state,
            if arg == "fail-first" {
                MissionState::Failed
            } else {
                MissionState::Blocked
            }
        );
        resume_tx.send(()).unwrap();
    }
    let _ = adv.join().unwrap();
    let want = match arg {
        "cap-first" => "block:budget_model_calls",
        "fail-first" => "fail:lane_fail:builder",
        "deny-first" => "block:lane_halt:builder",
        _ => "stop:owner",
    };
    assert_eq!(f.end(id), want);
    assert_eq!(f.dispatched(), 0);
    assert!(!f.journal.lines().concat().contains(ERROR));
    assert_eq!(m.close(), Ok(()));
}

#[test]
fn c1_s1_model_lane_submit_refuses_sealed_session() {
    children(
        "c1_s1_model_lane_submit_refuses_sealed_session",
        &["sealed"],
    );
}
fn sealed() {
    let home = crate::adapter::session_support::Fixture::new("c1b2-sealed");
    let invoked = Arc::new(AtomicUsize::new(0));
    let mut lanes = keyed_lanes();
    lanes.retain(|l| l.identity().role != LaneRole::Builder);
    lanes.push(Arc::new(ModelLane {
        cap: 1,
        script: Box::new(|_| quiet()),
        invoked: invoked.clone(),
    }));
    let m = Coordinator::open(
        home.journal(),
        home.port.clone(),
        Lanes::new(lanes).unwrap(),
        home.clock.clone(),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut c = contract_with("");
    c.requested_folder = Some(home.session.workspace().unwrap().name);
    c.model = Some(ModelChoice {
        name: NAME.into(),
        digest: DIGEST.into(),
        identity: identity_hex(),
    });
    c.deliverable = Some("index.html".into());
    let id = m.draft(c).unwrap();
    assert_eq!(
        m.submit(id),
        Err(CoordinatorError::Contract(
            ContractError::ModelLaneUnavailable
        ))
    );
    assert_eq!(invoked.load(Ordering::SeqCst), 0);
}
