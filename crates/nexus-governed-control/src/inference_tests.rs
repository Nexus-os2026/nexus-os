//! M2e-B pre-registered oracles (design v2 §8, §14 broker rows E-O20 to
//! E-O25; the M2e amendment's return time, E-O23b). From this commit on
//! the oracle bodies are frozen.
//!
//! Every runtime here is a fake Ollama on a loopback port of this process;
//! the real Ollama is never contacted. Tests synchronize on the fake's
//! request and close events and on named broker hooks, never on sleeps;
//! every wait is bounded and only detects a hang. Each refusal has a
//! positive control on the same fixture. Only private stop authorities are
//! engaged (a test front door claims its own).
//!
//! Deferred to M2e-A1b (the kernel's dispatch observer): `sent_mono_ms`,
//! the `Dispatch` notification and M3-O44/O45. Until it lands, E-O23b
//! requires the send time to be absent from every record, never 0.

use super::ModelIdentityView;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use crate::authority::clock::SystemClock;
use crate::authority::evidence::{
    EvidencePhase, EvidenceRecord, EvidenceSink, EvidenceUnavailable, MemoryEvidence,
};
use crate::authority::ids::{random16, AgentId, RunId};
use crate::authority::run::ConsentKind;
use crate::authority::AuthorityError;
use crate::broker::Vault;
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::PrimaryEvidence;
use crate::governed::{AgentDoor, GovernedControl, HaltReason};
use crate::harness_tests::{temp_root, TempRoot};
use crate::test_hooks::at;
use nexus_kernel::inference::{
    grammar, CallDeadline, CallMeter, Charged, MeterRefusal, ModelIdentity, ModelPinSpec,
    ModelReply, ModelRequest, RuntimeConfig, Think,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

const NAME: &str = "qwen3.6:35b-a3b";
const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
const VERSION: &str = "0.40.13";
const STATUS_DISABLED: &str = r#"{"cloud":{"disabled":true,"source":"env"}}"#;
const FINISH: &str = r#"{"action":"finish","summary":"done"}"#;
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
/// The return bound of a cancelled, halted or stopped call (§7.2).
const BOUND: Duration = Duration::from_secs(1);
/// How long a test waits for something that must happen promptly.
const PATIENCE: Duration = Duration::from_secs(10);
/// How long a lock probe may wait before it counts as blocked.
const PROBE: Duration = Duration::from_secs(2);
/// The keys a `model_call` record may hold (design v2 §8 and the M2e
/// amendment), each kept as written under the Restricted seal.
const CALL_KEYS: [&str; 12] = [
    "op",
    "call",
    "prompt_bytes",
    "bytes",
    "tokens_in",
    "tokens_out",
    "latency_ms",
    "model_digest",
    "identity",
    "outcome",
    "returned_mono_ms",
    "sent_mono_ms",
];

fn spec() -> ModelPinSpec {
    ModelPinSpec {
        name: NAME.into(),
        digest: DIGEST.into(),
        runtime_version: VERSION.into(),
        config: RuntimeConfig {
            num_ctx: 8192,
            temperature_milli: 0,
            seed: 7,
            think: Think::Off,
            truncate: false,
        },
    }
}

/// The identity digest of the pinned model, computed independently of the
/// broker.
fn identity() -> [u8; 32] {
    ModelIdentity::from_spec(&spec()).unwrap().digest()
}

fn label(n: u32) -> String {
    format!("msn-000001/1/{n}")
}

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

fn class<T>(result: Result<T, AuthorityError>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(error) => error.class(),
    }
}

/// The host's CLOCK_MONOTONIC in milliseconds, read by the test itself.
#[allow(unsafe_code)]
fn monotonic_ms() -> u64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes one timespec through a valid pointer.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) },
        0
    );
    u64::try_from(now.tv_sec).unwrap() * 1000 + u64::try_from(now.tv_nsec).unwrap() / 1_000_000
}

// ---------------------------------------------------------------- the fake runtime

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Request(String),
    Closed(String),
}

#[derive(Clone)]
enum Answer {
    Json(u16, String),
    /// Held open until the client closes the connection.
    Hold,
}

type Route = Arc<dyn Fn(&str) -> Answer + Send + Sync>;

/// A fake Ollama on 127.0.0.1: the healthy pinned runtime, but for the
/// paths a test's route answers.
struct Fake {
    addr: SocketAddr,
    events: Receiver<Event>,
    log: Arc<Mutex<Vec<String>>>,
    route: Arc<Mutex<Route>>,
}

fn chat_reply(content: &str) -> String {
    json!({
        "model": NAME,
        "created_at": "2026-11-03T06:10:00Z",
        "message": {"role": "assistant", "content": content},
        "done": true,
        "done_reason": "stop",
        "prompt_eval_count": 587,
        "eval_count": 378,
    })
    .to_string()
}

fn healthy(path: &str) -> Answer {
    match path {
        "/api/version" => Answer::Json(200, json!({ "version": VERSION }).to_string()),
        "/api/status" => Answer::Json(200, STATUS_DISABLED.into()),
        "/api/tags" => Answer::Json(
            200,
            json!({"models": [{"name": NAME, "model": NAME, "digest": DIGEST}]}).to_string(),
        ),
        "/api/show" => Answer::Json(
            200,
            json!({"capabilities": ["completion", "thinking"]}).to_string(),
        ),
        "/api/chat" => Answer::Json(200, chat_reply(FINISH)),
        _ => Answer::Json(404, "{}".into()),
    }
}

impl Fake {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, events) = channel();
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let route: Route = Arc::new(healthy);
        let route = Arc::new(Mutex::new(route));
        {
            let (log, route) = (log.clone(), route.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { return };
                    let (tx, log, route) = (tx.clone(), log.clone(), route.clone());
                    std::thread::spawn(move || serve(stream, &tx, &log, &route));
                }
            });
        }
        Self {
            addr,
            events,
            log,
            route,
        }
    }

    fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Answer the paths `over` answers; every other path as healthy.
    fn set(&self, over: impl Fn(&str) -> Option<Answer> + Send + Sync + 'static) {
        *self.route.lock().unwrap() =
            Arc::new(move |path: &str| over(path).unwrap_or_else(|| healthy(path)));
    }

    fn count(&self, path: &str) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == path)
            .count()
    }

    fn total(&self) -> usize {
        self.log.lock().unwrap().len()
    }

    #[track_caller]
    fn wait_for(&self, want: Event) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(event) if event == want => return,
                Ok(_) => {}
                Err(_) => panic!("the fake runtime never saw {want:?}"),
            }
        }
    }

    fn drain(&self) {
        while self.events.try_recv().is_ok() {}
    }
}

fn serve(
    mut stream: TcpStream,
    tx: &Sender<Event>,
    log: &Mutex<Vec<String>>,
    route: &Mutex<Route>,
) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let path = head
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if stream.read_exact(&mut body).is_err() {
        return;
    }
    log.lock().unwrap().push(path.clone());
    let route = route.lock().unwrap().clone();
    let answer = route(&path);
    let _ = tx.send(Event::Request(path.clone()));
    match answer {
        Answer::Json(status, body) => {
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
        Answer::Hold => {
            let mut sink = [0u8; 1024];
            while let Ok(n) = stream.read(&mut sink) {
                if n == 0 {
                    break;
                }
            }
            let _ = tx.send(Event::Closed(path));
        }
    }
}

// ---------------------------------------------------------------- the caller's side

type During = Box<dyn FnOnce() + Send>;

/// The Owner: yes to every dialog; runs `during` once inside the next
/// mission or held-mission dialog.
#[derive(Default)]
struct Owner {
    missions: AtomicUsize,
    during: Mutex<Option<During>>,
}

impl Owner {
    fn during(&self, f: impl FnOnce() + Send + 'static) {
        *self.during.lock().unwrap() = Some(Box::new(f));
    }

    fn fire(&self) {
        let f = self.during.lock().unwrap().take();
        if let Some(f) = f {
            f();
        }
    }
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.missions.fetch_add(1, Ordering::SeqCst);
        self.fire();
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        self.fire();
        true
    }
}

type Hook = Box<dyn Fn() + Send + Sync>;

/// The caller's meter: counts and numbers its charges, records its
/// settlements, refuses when told to, and runs `during` inside `charge`.
#[derive(Default)]
struct Meter {
    charged: AtomicU32,
    settled: Mutex<Vec<u32>>,
    refuse: Option<MeterRefusal>,
    during: Mutex<Option<Hook>>,
}

impl Meter {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn refusing(refusal: MeterRefusal) -> Arc<Self> {
        Arc::new(Self {
            refuse: Some(refusal),
            ..Self::default()
        })
    }

    fn charged(&self) -> u32 {
        self.charged.load(Ordering::SeqCst)
    }

    fn settled(&self) -> Vec<u32> {
        self.settled.lock().unwrap().clone()
    }
}

impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        if let Some(during) = self.during.lock().unwrap().as_ref() {
            during();
        }
        if let Some(refusal) = self.refuse {
            return Err(refusal);
        }
        let call = self.charged.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Charged { call })
    }

    fn settle(&self, call: u32) {
        self.settled.lock().unwrap().push(call);
    }
}

/// The mission's deadline: expired once the test says so.
#[derive(Default)]
struct Deadline(AtomicBool);

impl CallDeadline for Deadline {
    fn expired(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

fn request(user: &str) -> ModelRequest {
    ModelRequest::new(
        "You draft one file.".into(),
        user.into(),
        grammar::worker_schema(),
    )
    .unwrap()
}

fn in_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx
}

#[track_caller]
fn within<T>(rx: &Receiver<T>, bound: Duration, what: &str) -> T {
    rx.recv_timeout(bound)
        .unwrap_or_else(|_| panic!("{what} did not end within {bound:?}"))
}

fn detail<'a>(record: &'a EvidenceRecord, key: &str) -> Option<&'a str> {
    record
        .detail
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

// ---------------------------------------------------------------- fixture

struct Fx {
    gov: Arc<GovernedControl>,
    history: Arc<MemoryEvidence>,
    owner: Arc<Owner>,
    door: Arc<AgentDoor>,
    fake: Fake,
    _root: TempRoot,
}

fn front_door(root: &Path, evidence: Arc<dyn EvidenceSink>) -> Arc<GovernedControl> {
    Arc::new(
        GovernedControl::with_secrets(
            root,
            Vault::Disabled.source(),
            evidence,
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    )
}

impl Fx {
    /// A front door over in-memory evidence, a fake runtime and the
    /// Owner's door for Mother; no model registered.
    fn bare() -> Self {
        let history = Arc::new(MemoryEvidence::new(100_000));
        Self::over(history.clone(), history)
    }

    fn over(sink: Arc<dyn EvidenceSink>, history: Arc<MemoryEvidence>) -> Self {
        let root = temp_root("m2e-b");
        let gov = front_door(root.0.path(), sink);
        let owner = Arc::new(Owner::default());
        let door = Arc::new(gov.issue_agent_door(mother(), owner.clone()).unwrap());
        Self {
            gov,
            history,
            owner,
            door,
            fake: Fake::start(),
            _root: root,
        }
    }

    /// [`Fx::bare`], with the pinned model registered.
    fn new() -> Self {
        let f = Self::bare();
        f.register().expect("the pinned model registers");
        f
    }

    fn register(&self) -> Result<ModelIdentityView, AuthorityError> {
        self.gov.register_local_model(&self.fake.endpoint(), spec())
    }

    fn door(&self) -> Arc<AgentDoor> {
        Arc::new(
            self.gov
                .issue_agent_door(mother(), self.owner.clone())
                .unwrap(),
        )
    }

    fn consent(&self, door: &AgentDoor, hash: &str, request: &str) -> u64 {
        door.confirm_mission(&mission(hash, request))
            .expect("the Owner consents")
            .evidence_seq()
    }

    /// A run of `door`, admitted for inference to (HASH, label(n)).
    fn admitted(&self, door: &AgentDoor, n: u32) -> RunId {
        self.consent(door, HASH, &label(n));
        let run = door.open_run().unwrap();
        door.admit_inference(run, HASH, &label(n), identity())
            .expect("admitted");
        run
    }

    fn admission(&self, run: RunId) -> Option<crate::authority::run::InferenceAdmission> {
        self.gov.authority().runs().inference_admission(run)
    }

    fn call(&self, run: RunId, meter: &Meter) -> Result<ModelReply, AuthorityError> {
        self.door
            .model_call(run, &request("Rosa's bakery."), meter, &Deadline::default())
    }

    /// One call of `door`'s `run` on another thread.
    fn calling(
        &self,
        door: &Arc<AgentDoor>,
        run: RunId,
        meter: &Arc<Meter>,
    ) -> Receiver<Result<ModelReply, AuthorityError>> {
        let (door, meter) = (door.clone(), meter.clone());
        in_thread(move || door.model_call(run, &request("x"), &*meter, &Deadline::default()))
    }

    fn records(&self, phase: EvidencePhase) -> Vec<EvidenceRecord> {
        self.history
            .records()
            .into_iter()
            .filter(|r| r.phase == phase)
            .collect()
    }

    fn calls_of(&self, run: RunId) -> Vec<EvidenceRecord> {
        let run = run.to_string();
        self.records(EvidencePhase::ModelCall)
            .into_iter()
            .filter(|r| r.run.as_deref() == Some(run.as_str()))
            .collect()
    }
}

fn mission(hash: &str, request: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: hash.into(),
        request: request.into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    }
}

fn held(request: &str) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: "msn-000001".into(),
        contract_hash: HASH.into(),
        request: request.into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    }
}

// ---------------------------------------------------------------- E-O20

/// E-O20: without a registered model a door has no model, no admission and
/// no call, and nothing reaches the runtime; the host registers one model
/// (recorded Restricted: its digests only), never a second; a registration
/// under Stop, or to a non-loopback address, makes no request.
#[test]
fn e_o20_registration_is_the_host_s_once_and_without_it_nothing_runs() {
    let f = Fx::bare();
    assert_eq!(f.door.model_identity(), None);
    f.consent(&f.door, HASH, &label(1));
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(run, HASH, &label(1), identity())),
        "model_not_registered"
    );
    assert_eq!(class(f.call(run, &Meter::new())), "model_not_registered");
    assert_eq!(f.fake.total(), 0, "nothing reached the runtime");

    assert_eq!(
        class(f.gov.register_local_model("http://192.0.2.1:11434", spec())),
        "model_not_local"
    );
    f.gov.emergency_stop();
    assert_eq!(class(f.register()), "emergency_stopped");
    assert_eq!(f.fake.total(), 0, "no request under Stop or off loopback");
    f.gov.resume(&*f.owner).unwrap();

    let view = f.register().expect("the host registers the pinned model");
    assert_eq!(view.name, NAME);
    assert_eq!(view.digest, DIGEST);
    assert_eq!(view.identity, hex::encode(identity()));
    assert_eq!(f.door.model_identity(), Some(view.clone()));
    assert_eq!(class(f.register()), "capacity", "one model per process");
    let registered = f.records(EvidencePhase::ModelRegistered);
    assert_eq!(registered.len(), 1);
    let detail_keys: Vec<&str> = registered[0]
        .detail
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(detail_keys, ["model_digest", "identity"]);
    assert_eq!(detail(&registered[0], "model_digest"), Some(DIGEST));
    assert_eq!(
        detail(&registered[0], "identity"),
        Some(view.identity.as_str())
    );
    assert_eq!(registered[0].agent, None);
}

/// E-O20: no door route to registration (with the `compile_fail` probes in
/// `lib.rs`): the door's source never names it.
#[test]
fn e_o20_no_door_route_to_registration() {
    let door = include_str!("governed/door.rs");
    for name in ["register_local_model", "models.register", ".register("] {
        assert!(!door.contains(name), "the door names `{name}`");
    }
}

/// E-O20: each refusal of admission and of a call, with its positive
/// control: another door's run, a cancelled or finished run, a halted door,
/// no admission, an identity mismatch. A refused call reaches neither the
/// runtime nor the meter and records nothing.
#[test]
fn e_o20_each_refusal_reaches_nothing_and_has_a_positive_control() {
    let f = Fx::new();
    let meter = Meter::new();
    let before = f.fake.total();

    // An identity mismatch admits nothing; the right identity admits.
    f.consent(&f.door, HASH, &label(1));
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(run, HASH, &label(1), [7u8; 32])),
        "model_identity_mismatch"
    );
    assert_eq!(f.admission(run), None);
    // No admission: no call.
    assert_eq!(class(f.call(run, &meter)), "not_admitted");

    // Another door's run.
    let other = f.door();
    let theirs = f.admitted(&other, 2);
    assert_eq!(class(f.call(theirs, &meter)), "not_member");
    assert_eq!(
        class(f.door.admit_inference(theirs, HASH, &label(1), identity())),
        "not_member"
    );

    // A cancelled run, a finished run.
    let cancelled = f.admitted(&f.door, 3);
    let _ = f.door.cancel_run(cancelled).unwrap().write();
    assert_eq!(class(f.call(cancelled, &meter)), "run_cancelled");
    let finished = f.admitted(&f.door, 4);
    f.door.finish_when_settled(finished).unwrap();
    assert_eq!(class(f.call(finished, &meter)), "run_not_active");

    // A halted door.
    let halted = f.door();
    let halted_run = f.admitted(&halted, 5);
    let _ = halted.halt(HaltReason::JournalFailed).write();
    let fresh = f.door.open_run().unwrap();
    assert_eq!(
        class(halted.model_call(halted_run, &request("x"), &*meter, &Deadline::default())),
        "halted"
    );
    assert_eq!(
        class(halted.admit_inference(fresh, HASH, &label(1), identity())),
        "halted"
    );

    assert_eq!(
        f.fake.total(),
        before,
        "no refused call reached the runtime"
    );
    assert_eq!(meter.charged(), 0, "no refused call reached the meter");
    assert!(f.records(EvidencePhase::ModelCall).is_empty());

    // Positive controls: the same door's admitted run calls; so does the
    // other door's own run.
    f.door
        .admit_inference(run, HASH, &label(1), identity())
        .unwrap();
    let admission = f.admission(run).expect("admitted");
    assert_eq!(admission.identity, identity());
    assert_eq!(admission.contract_hash, HASH);
    assert_eq!(f.call(run, &meter).unwrap().content(), FINISH);
    other
        .model_call(theirs, &request("x"), &*meter, &Deadline::default())
        .unwrap();
    assert_eq!(meter.charged(), 2);
    assert_eq!(f.fake.count("/api/chat"), 2);
    assert_eq!(f.calls_of(run).len(), 1);
    assert_eq!(f.calls_of(theirs).len(), 1);
}

// ---------------------------------------------------------------- E-O21

/// E-O21: a run's ninth attempt is refused before the runtime's metadata
/// and before the meter; another run has its own eight.
#[test]
fn e_o21_the_ninth_attempt_is_refused_before_the_metadata_and_the_meter() {
    let f = Fx::new();
    let meter = Meter::new();
    let run = f.admitted(&f.door, 1);
    for n in 1..=8 {
        assert_eq!(class(f.call(run, &meter)), "ok", "attempt {n}");
    }
    let (requests, records) = (f.fake.total(), f.calls_of(run).len());
    assert_eq!(records, 8);
    assert_eq!(class(f.call(run, &meter)), "model_over_budget");
    assert_eq!(f.fake.total(), requests, "no metadata request for call 9");
    assert_eq!(meter.charged(), 8, "the meter was not asked for call 9");
    assert_eq!(f.calls_of(run).len(), 8);

    let other = f.admitted(&f.door, 2);
    assert_eq!(class(f.call(other, &meter)), "ok");
}

/// E-O21: a meter refusal gives no chat; the metadata was checked first,
/// and the attempt is recorded with no call index.
#[test]
fn e_o21_a_meter_refusal_sends_no_chat() {
    let f = Fx::new();
    let run = f.admitted(&f.door, 1);
    let (version, chats) = (f.fake.count("/api/version"), f.fake.count("/api/chat"));
    let meter = Meter::refusing(MeterRefusal::Exhausted);
    assert_eq!(class(f.call(run, &meter)), "model_over_budget");
    assert_eq!(f.fake.count("/api/chat"), chats, "no chat");
    assert_eq!(f.fake.count("/api/version"), version + 1);
    assert!(
        meter.settled().is_empty(),
        "nothing charged, nothing settled"
    );
    let records = f.calls_of(run);
    assert_eq!(records.len(), 1);
    assert_eq!(detail(&records[0], "outcome"), Some("over_budget"));
    assert_eq!(detail(&records[0], "call"), None);
}

// ---------------------------------------------------------------- E-O22

#[derive(Clone, Copy, Debug)]
enum Interrupt {
    Cancel,
    Halt,
    Stop,
}

/// One E-O22 schedule: a call of a run of a fresh door, held at `path`,
/// ended by `interrupt`.
fn held_call_ends(path: &'static str, interrupt: Interrupt) {
    let f = Fx::new();
    let door = f.door();
    let run = f.admitted(&door, 1);
    let meter = Meter::new();
    f.fake.drain();
    f.fake
        .set(move |seen| (seen == path).then_some(Answer::Hold));
    let rx = f.calling(&door, run, &meter);
    f.fake.wait_for(Event::Request(path.into()));
    match interrupt {
        Interrupt::Cancel => {
            let _ = door.cancel_run(run).unwrap().write();
        }
        Interrupt::Halt => {
            let _ = door.halt(HaltReason::JournalFailed).write();
        }
        Interrupt::Stop => {
            f.gov.emergency_stop();
        }
    }
    let result = within(&rx, BOUND, &format!("{interrupt:?} at {path}"));
    let want = match interrupt {
        Interrupt::Stop => "model_stopped",
        Interrupt::Cancel | Interrupt::Halt => "model_cancelled",
    };
    assert_eq!(class(result), want, "{interrupt:?} at {path}");
    f.fake.wait_for(Event::Closed(path.into()));
    let charged = u32::from(path == "/api/chat");
    assert_eq!(meter.charged(), charged, "{interrupt:?} at {path}");
    assert_eq!(meter.settled(), (1..=charged).collect::<Vec<_>>());
    if path != "/api/chat" {
        assert_eq!(f.fake.count("/api/chat"), 0);
    }
    let records = f.calls_of(run);
    assert_eq!(records.len(), 1);
    assert_eq!(
        detail(&records[0], "outcome"),
        Some(&want["model_".len()..])
    );
    if let Interrupt::Stop = interrupt {
        assert!(
            f.records(EvidencePhase::KernelHalt)
                .iter()
                .any(|r| detail(r, "area") == Some("model_call")),
            "the stop's kernel halt is recorded"
        );
    }
}

#[test]
fn e_o22_cancel_ends_a_held_chat() {
    held_call_ends("/api/chat", Interrupt::Cancel);
}

#[test]
fn e_o22_cancel_ends_held_tags() {
    held_call_ends("/api/tags", Interrupt::Cancel);
}

#[test]
fn e_o22_a_door_halt_ends_a_held_chat() {
    held_call_ends("/api/chat", Interrupt::Halt);
}

#[test]
fn e_o22_a_door_halt_ends_held_tags() {
    held_call_ends("/api/tags", Interrupt::Halt);
}

#[test]
fn e_o22_the_owner_s_stop_ends_a_held_chat() {
    held_call_ends("/api/chat", Interrupt::Stop);
}

#[test]
fn e_o22_the_owner_s_stop_ends_held_tags() {
    held_call_ends("/api/tags", Interrupt::Stop);
}

/// E-O22 control: the caller's (mission's) deadline ends a held chat.
#[test]
fn e_o22_the_mission_deadline_ends_a_held_chat() {
    let f = Fx::new();
    let run = f.admitted(&f.door, 1);
    f.fake.drain();
    f.fake
        .set(|seen| (seen == "/api/chat").then_some(Answer::Hold));
    let deadline = Arc::new(Deadline::default());
    let rx = {
        let (door, deadline) = (f.door.clone(), deadline.clone());
        in_thread(move || door.model_call(run, &request("x"), &*Meter::new(), &*deadline))
    };
    f.fake.wait_for(Event::Request("/api/chat".into()));
    deadline.0.store(true, Ordering::SeqCst);
    assert_eq!(class(within(&rx, BOUND, "the deadline")), "model_timeout");
    f.fake.wait_for(Event::Closed("/api/chat".into()));
}

// ---------------------------------------------------------------- E-O23

/// The P0 scanner's forms of one canary (raw, cases, hex, base64 variants,
/// percent, JSON `\u`, and four fragments of at least 8 characters).
fn forms(name: &str, canary: &str) -> Vec<(String, String)> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    use base64::Engine;
    let bytes = canary.as_bytes();
    let mut out = vec![
        ("raw", canary.to_string()),
        ("upper", canary.to_uppercase()),
        ("lower", canary.to_lowercase()),
        ("hex", hex::encode(bytes)),
        ("HEX", hex::encode_upper(bytes)),
        ("base64", STANDARD.encode(bytes)),
        ("base64 unpadded", STANDARD_NO_PAD.encode(bytes)),
        ("base64url", URL_SAFE.encode(bytes)),
        ("base64url unpadded", URL_SAFE_NO_PAD.encode(bytes)),
        (
            "percent",
            bytes.iter().map(|b| format!("%{b:02X}")).collect(),
        ),
        (
            "percent lower",
            bytes.iter().map(|b| format!("%{b:02x}")).collect(),
        ),
        (
            "json-u",
            canary
                .chars()
                .map(|c| format!("\\u{:04x}", c as u32))
                .collect(),
        ),
    ]
    .into_iter()
    .map(|(form, text)| (format!("{name} {form}"), text))
    .collect::<Vec<_>>();
    let chars: Vec<char> = canary.chars().collect();
    let n = chars.len();
    let spans = if n >= 40 {
        let q = n / 4;
        vec![(0, q), (q, 2 * q), (2 * q, 3 * q), (3 * q, n)]
    } else {
        vec![(0, 8), (4, 13), (8, 20), (n - 12, n)]
    };
    for (i, (a, b)) in spans.into_iter().enumerate() {
        let fragment: String = chars[a..b].iter().collect();
        out.push((format!("{name} fragment {i}"), fragment.clone()));
        out.push((
            format!("{name} fragment {i} upper"),
            fragment.to_uppercase(),
        ));
        out.push((
            format!("{name} fragment {i} lower"),
            fragment.to_lowercase(),
        ));
    }
    out
}

struct Scan {
    needles: Vec<(String, String)>,
    found: Vec<String>,
}

impl Scan {
    fn text(&mut self, source: &str, text: &str) {
        for (label, needle) in &self.needles {
            if text.contains(needle.as_str()) {
                self.found.push(format!("{label} in {source}"));
            }
        }
    }

    /// The store's main file, then its `-wal` and `-shm` files when they
    /// exist; returns the suffixes read.
    fn files(&mut self, label: &str, store: &Path) -> Vec<String> {
        let mut read = Vec::new();
        for suffix in ["", "-wal", "-shm"] {
            if let Ok(bytes) = std::fs::read(PathBuf::from(format!("{}{suffix}", store.display())))
            {
                self.text(
                    &format!("{label} store file{suffix}"),
                    &String::from_utf8_lossy(&bytes),
                );
                read.push(suffix.to_string());
            }
        }
        read
    }

    fn rows(&mut self, label: &str, primary: &PrimaryEvidence) {
        for row in primary.store().rows() {
            let text = format!(
                "{} {} {} {} {:?} {} {} {}",
                row.seq,
                row.session,
                row.stored_ms,
                row.phase,
                row.commitment,
                row.body,
                row.prev,
                row.tag
            );
            self.text(&format!("{label} row {}", row.seq), &text);
        }
    }
}

/// E-O23: one Restricted `model_call` record per attempt, holding only the
/// registered counts, digests and codes; canaries in the prompt, the reply,
/// an error reply and a metadata reply appear in no record, no store file
/// (main, WAL, SHM, open or reopened), no row and no error.
#[test]
fn e_o23_one_restricted_record_per_attempt_and_no_canary_anywhere() {
    let c = format!("NXCANARY-{}", hex::encode(random16()));
    let cl = format!("nxsecret_{}", &hex::encode(random16())[..16]);
    let scratch = Scratch::new();
    let history = Arc::new(MemoryEvidence::new(20_000));
    let primary = PrimaryEvidence::open(&scratch.store(), vec![history.clone()]).unwrap();
    let f = Fx::over(primary.clone(), history.clone());
    f.register().unwrap();
    let run = f.admitted(&f.door, 1);
    let meter = Meter::new();
    let prompt = format!("Notes: {c} {cl}");
    let call = |f: &Fx| {
        f.door
            .model_call(run, &request(&prompt), &*meter, &Deadline::default())
    };
    let mut errors = Vec::new();

    // ok, with the canaries in the reply.
    let reply_content = format!(r#"{{"action":"finish","summary":"{c} {cl}"}}"#);
    let content = reply_content.clone();
    f.fake
        .set(move |seen| (seen == "/api/chat").then(|| Answer::Json(200, chat_reply(&content))));
    assert_eq!(call(&f).unwrap().content(), reply_content);
    // An error reply carrying them.
    let body = format!(r#"{{"error":"{c} {cl}"}}"#);
    f.fake
        .set(move |seen| (seen == "/api/chat").then(|| Answer::Json(500, body.clone())));
    errors.push(call(&f).unwrap_err());
    // A malformed reply carrying them.
    let body = json!({"model": NAME, "message": {"role": "user", "content": format!("{c} {cl}")},
        "done": true, "done_reason": "stop", "prompt_eval_count": 587, "eval_count": 3})
    .to_string();
    f.fake
        .set(move |seen| (seen == "/api/chat").then(|| Answer::Json(200, body.clone())));
    errors.push(call(&f).unwrap_err());
    // A metadata reply carrying them (refused before any charge).
    let body = json!({ "version": format!("{c} {cl}") }).to_string();
    f.fake
        .set(move |seen| (seen == "/api/version").then(|| Answer::Json(200, body.clone())));
    errors.push(call(&f).unwrap_err());
    // Tool calls carrying them.
    let body = json!({"model": NAME, "message": {"role": "assistant", "content": "",
        "tool_calls": [{"function": {"name": c, "arguments": {"x": cl}}}]},
        "done": true, "done_reason": "stop", "prompt_eval_count": 587, "eval_count": 3})
    .to_string();
    f.fake
        .set(move |seen| (seen == "/api/chat").then(|| Answer::Json(200, body.clone())));
    errors.push(call(&f).unwrap_err());

    let records = f.calls_of(run);
    let outcomes: Vec<Option<&str>> = records.iter().map(|r| detail(r, "outcome")).collect();
    assert_eq!(
        outcomes,
        [
            Some("ok"),
            Some("unavailable"),
            Some("malformed"),
            Some("runtime_mismatch"),
            Some("tool_calls")
        ],
        "one record per attempt"
    );
    let calls: Vec<Option<&str>> = records.iter().map(|r| detail(r, "call")).collect();
    assert_eq!(calls, [Some("1"), Some("2"), Some("3"), None, Some("4")]);
    let prompt_bytes = request(&prompt).rendered_bytes().to_string();
    for record in &records {
        assert_eq!(record.target, None);
        assert_eq!(record.agent.as_deref(), Some("mother"));
        for (key, value) in &record.detail {
            assert!(
                CALL_KEYS.contains(&key.as_str()),
                "a model_call record keeps `{key}`"
            );
            assert!(!value.is_empty(), "{key}");
        }
        assert_eq!(detail(record, "op"), Some("model_call"));
        assert_eq!(detail(record, "model_digest"), Some(DIGEST));
        assert_eq!(
            detail(record, "identity"),
            Some(hex::encode(identity()).as_str())
        );
        assert_eq!(detail(record, "prompt_bytes"), Some(prompt_bytes.as_str()));
    }
    assert_eq!(
        detail(&records[0], "bytes"),
        Some(reply_content.len().to_string().as_str())
    );
    assert_eq!(detail(&records[0], "tokens_in"), Some("587"));
    assert_eq!(detail(&records[0], "tokens_out"), Some("378"));
    assert!(detail(&records[0], "latency_ms").is_some());
    for record in &records[1..] {
        for key in ["bytes", "tokens_in", "tokens_out", "latency_ms"] {
            assert_eq!(detail(record, key), None, "a refused call keeps no {key}");
        }
    }

    let mut scan = Scan {
        needles: [forms("C", &c), forms("Cl", &cl)].concat(),
        found: Vec::new(),
    };
    for error in &errors {
        scan.text("an error", &format!("{error} {error:?}"));
    }
    for record in history.records() {
        scan.text(
            &format!("history {}", record.phase.as_str()),
            &format!("{} {:?}", record.to_json(), record),
        );
    }
    scan.rows("open store", &primary);
    let store = scratch.store();
    let read = scan.files("open", &store);
    assert!(
        read.iter().any(|s| s == "-wal"),
        "the open store has its WAL"
    );
    drop(f);
    assert_eq!(
        Arc::strong_count(&primary),
        1,
        "nothing else holds the store"
    );
    drop(primary);
    let reopened = PrimaryEvidence::open(&store, vec![]).expect("the chain verifies");
    scan.rows("reopened store", &reopened);
    drop(reopened);
    scan.files("closed", &store);
    assert!(scan.found.is_empty(), "canaries found: {:?}", scan.found);
}

/// E-O23b (the M2e amendment, M3 §8.5): every attempt's record holds the
/// host CLOCK_MONOTONIC time it returned at (between the test's own reads
/// before and after), on every path: ok, an error reply, a metadata
/// mismatch and a meter refusal before dispatch, a stopped and a cancelled
/// chat. Until the kernel's dispatch observer lands (M2e-A1b), no record
/// holds a send time: the key is omitted, never 0.
#[test]
fn e_o23b_the_return_time_is_on_every_path_and_the_send_time_is_omitted() {
    let f = Fx::new();
    let meter = Meter::new();
    let mut seen = Vec::new();
    let mut timed = |f: &Fx, run: RunId, meter: &Meter, over: Option<Route>| {
        if let Some(over) = over {
            f.fake.set(move |path| Some(over(path)));
        } else {
            f.fake.set(|_| None);
        }
        let before = monotonic_ms();
        let outcome = class(f.call(run, meter));
        let after = monotonic_ms();
        seen.push((outcome, run, before, after));
    };
    let run = f.admitted(&f.door, 1);
    timed(&f, run, &meter, None);
    timed(
        &f,
        run,
        &meter,
        Some(Arc::new(|path: &str| match path {
            "/api/chat" => Answer::Json(500, "{}".into()),
            other => healthy(other),
        })),
    );
    timed(
        &f,
        run,
        &meter,
        Some(Arc::new(|path: &str| match path {
            "/api/version" => Answer::Json(200, r#"{"version":"0.0.1"}"#.into()),
            other => healthy(other),
        })),
    );
    timed(&f, run, &Meter::refusing(MeterRefusal::OutOfTime), None);
    let outcomes: Vec<&str> = seen.iter().map(|(o, ..)| *o).collect();
    assert_eq!(
        outcomes,
        [
            "ok",
            "model_unavailable",
            "model_runtime_mismatch",
            "model_over_budget"
        ]
    );

    // A stopped chat and a cancelled chat (each held, then ended).
    for interrupt in [Interrupt::Stop, Interrupt::Cancel] {
        let g = Fx::new();
        let door = g.door();
        let run = g.admitted(&door, 1);
        g.fake
            .set(|seen| (seen == "/api/chat").then_some(Answer::Hold));
        let before = monotonic_ms();
        let rx = g.calling(&door, run, &Meter::new());
        g.fake.wait_for(Event::Request("/api/chat".into()));
        match interrupt {
            Interrupt::Stop => {
                g.gov.emergency_stop();
            }
            _ => {
                let _ = door.cancel_run(run).unwrap().write();
            }
        }
        let outcome = class(within(&rx, BOUND, "the held chat"));
        let after = monotonic_ms();
        let records = g.calls_of(run);
        assert_eq!(records.len(), 1, "{interrupt:?}");
        check_times(&records[0], outcome, before, after);
    }

    let records = f.calls_of(run);
    assert_eq!(records.len(), seen.len());
    for (record, (outcome, _, before, after)) in records.iter().zip(&seen) {
        check_times(record, outcome, *before, *after);
    }
}

#[track_caller]
fn check_times(record: &EvidenceRecord, outcome: &str, before: u64, after: u64) {
    let returned: u64 = detail(record, "returned_mono_ms")
        .unwrap_or_else(|| panic!("{outcome}: no returned_mono_ms"))
        .parse()
        .unwrap();
    assert!(
        (before..=after).contains(&returned),
        "{outcome}: returned_mono_ms {returned} outside [{before}, {after}]"
    );
    assert_eq!(
        detail(record, "sent_mono_ms"),
        None,
        "{outcome}: a send time before the dispatch observer exists"
    );
}

// ---------------------------------------------------------------- E-O24

/// Whether every broker lock a door's reader takes can be taken now, from
/// another thread, within `PROBE` (a held lock blocks it).
fn locks_free(gov: &Weak<GovernedControl>, door: &Weak<AgentDoor>) -> bool {
    let (Some(gov), Some(door)) = (gov.upgrade(), door.upgrade()) else {
        return false;
    };
    let rx = in_thread(move || {
        let _ = door.model_identity();
        let _ = door.emergency_stopped();
        gov.probe_locks();
        let _ = gov.authority().runs().views();
    });
    rx.recv_timeout(PROBE).is_ok()
}

/// A sink that probes the broker's locks as a `model_call` record arrives.
struct Probing {
    inner: Arc<MemoryEvidence>,
    probe: OnceLock<Box<dyn Fn() -> bool + Send + Sync>>,
    results: Mutex<Vec<bool>>,
}

impl EvidenceSink for Probing {
    fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
        if record.phase == EvidencePhase::ModelCall {
            if let Some(probe) = self.probe.get() {
                self.results.lock().unwrap().push(probe());
            }
        }
        self.inner.record(record)
    }
}

/// E-O24: no broker lock is held while the meter charges, while the chat
/// is in flight, or while the call's record is written: a door's reader,
/// and every broker lock, is taken from another thread at each of the
/// three places within a bounded wait.
#[test]
fn e_o24_no_broker_lock_is_held_across_the_meter_the_exchange_or_the_record() {
    let history = Arc::new(MemoryEvidence::new(10_000));
    let sink = Arc::new(Probing {
        inner: history.clone(),
        probe: OnceLock::new(),
        results: Mutex::new(Vec::new()),
    });
    let f = Fx::over(sink.clone(), history);
    f.register().unwrap();
    let run = f.admitted(&f.door, 1);
    let (gov, door) = (Arc::downgrade(&f.gov), Arc::downgrade(&f.door));
    {
        let (gov, door) = (gov.clone(), door.clone());
        let _ = sink.probe.set(Box::new(move || locks_free(&gov, &door)));
    }
    let in_exchange = Arc::new(Mutex::new(Vec::new()));
    {
        let (gov, door, seen) = (gov.clone(), door.clone(), in_exchange.clone());
        f.fake.set(move |path| {
            (path == "/api/chat").then(|| {
                seen.lock().unwrap().push(locks_free(&gov, &door));
                healthy(path)
            })
        });
    }
    let meter = Meter::new();
    let in_charge = Arc::new(Mutex::new(Vec::new()));
    {
        let seen = in_charge.clone();
        *meter.during.lock().unwrap() = Some(Box::new(move || {
            seen.lock().unwrap().push(locks_free(&gov, &door))
        }));
    }
    let rx = {
        let (door, meter) = (f.door.clone(), meter.clone());
        in_thread(move || door.model_call(run, &request("x"), &*meter, &Deadline::default()))
    };
    assert_eq!(class(within(&rx, PATIENCE, "the probed call")), "ok");
    assert_eq!(*in_charge.lock().unwrap(), [true], "in charge");
    assert_eq!(*in_exchange.lock().unwrap(), [true], "in the exchange");
    assert_eq!(*sink.results.lock().unwrap(), [true], "in the record write");
}

// ---------------------------------------------------------------- E-O25

/// E-O25 (a) a halt during the Owner's confirmation, (b) during the table
/// insertion and (c) during admission each leave no admission; the door's
/// own fresh yes admits (control).
#[test]
fn e_o25_a_b_c_halts_during_confirmation_insertion_and_admission() {
    let f = Fx::new();
    // (a) during the confirmation.
    let door = f.door();
    let halting = door.clone();
    f.owner.during(move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(
        class(door.confirm_mission(&mission(HASH, &label(1)))),
        "halted"
    );
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(door.admit_inference(run, HASH, &label(1), identity())),
        "halted"
    );
    // (b) during the insertion.
    let door = f.door();
    let halting = door.clone();
    f.gov.hooks().arm(at::CONSENT_BEFORE_FILL, move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(
        class(door.confirm_mission(&mission(HASH, &label(2)))),
        "halted"
    );
    assert_eq!(door.consents_cached(), 0);
    // (c) during admission.
    let door = f.door();
    f.consent(&door, HASH, &label(3));
    let theirs = door.open_run().unwrap();
    let halting = door.clone();
    f.gov.hooks().arm(at::INFERENCE_BEFORE_BIND, move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(
        class(door.admit_inference(theirs, HASH, &label(3), identity())),
        "halted"
    );
    assert_eq!(f.admission(theirs), None);
    // A Stop during admission refuses too.
    f.consent(&f.door, HASH, &label(4));
    let gov = f.gov.clone();
    f.gov.hooks().arm(at::INFERENCE_BEFORE_BIND, move || {
        gov.emergency_stop();
    });
    assert_eq!(
        class(f.door.admit_inference(run, HASH, &label(4), identity())),
        "emergency_stopped"
    );
    assert_eq!(f.admission(run), None);
    f.gov.resume(&*f.owner).unwrap();

    // Control.
    let seq = f.consent(&f.door, HASH, &label(5));
    let run = f.door.open_run().unwrap();
    f.door
        .admit_inference(run, HASH, &label(5), identity())
        .unwrap();
    let admission = f.admission(run).expect("admitted");
    assert_eq!(admission.evidence_seq, seq);
    assert_eq!(admission.kind, ConsentKind::Mission);
    assert_eq!(admission.request, label(5));
}

/// E-O25 (d) a yes cached before a Stop admits nothing after the resume;
/// (e) a late confirmation whose dialog outlived a Stop and its resume
/// cannot repopulate the table. A fresh yes admits (control).
#[test]
fn e_o25_d_e_a_pre_stop_yes_and_a_late_confirmation_admit_nothing() {
    let f = Fx::new();
    f.consent(&f.door, HASH, &label(1));
    let before = f.door.open_run().unwrap();
    f.gov.emergency_stop();
    f.gov.resume(&*f.owner).unwrap();
    let after = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(after, HASH, &label(1), identity())),
        "no_consent"
    );
    assert_eq!(
        class(f.door.admit_inference(before, HASH, &label(1), identity())),
        "run_cancelled"
    );
    assert_eq!(f.admission(after), None);

    let gov = f.gov.clone();
    let owner = f.owner.clone();
    f.owner.during(move || {
        gov.emergency_stop();
        gov.resume(&*owner).unwrap();
    });
    assert!(f.door.confirm_mission(&mission(HASH, &label(2))).is_err());
    let late = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(late, HASH, &label(2), identity())),
        "no_consent"
    );
    assert_eq!(f.admission(late), None);

    // `after` was cancelled by (e)'s own Stop: it stays refused (H1′).
    f.consent(&f.door, HASH, &label(1));
    assert_eq!(
        class(f.door.admit_inference(after, HASH, &label(1), identity())),
        "run_cancelled"
    );
    assert_eq!(f.admission(after), None);
    // Control: the run opened after the last resume admits a fresh yes.
    f.door
        .admit_inference(late, HASH, &label(1), identity())
        .unwrap();
    assert!(f.admission(late).is_some());
}

/// E-O25 (f): at 64 cached yeses the 65th is refused before the Owner is
/// asked; the cached ones still admit.
#[test]
fn e_o25_f_the_65th_entry_is_refused() {
    let f = Fx::new();
    for n in 1..=64 {
        f.consent(&f.door, HASH, &label(n));
    }
    let asked = f.owner.missions.load(Ordering::SeqCst);
    assert_eq!(
        class(f.door.confirm_mission(&mission(HASH, &label(65)))),
        "capacity"
    );
    assert_eq!(f.owner.missions.load(Ordering::SeqCst), asked);
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(run, HASH, &label(65), identity())),
        "no_consent"
    );
    f.door
        .admit_inference(run, HASH, &label(64), identity())
        .unwrap();
}

/// E-O25 (g) a cancelled or finished run, (h) a second, different
/// admission (sticky; the same one again is the same binding) and (i)
/// another door's confirmation; a held-mission resume yes admits.
#[test]
fn e_o25_g_h_i_run_life_stickiness_and_another_door() {
    let f = Fx::new();
    f.consent(&f.door, HASH, &label(1));
    f.consent(&f.door, HASH2, &label(2));
    let cancelled = f.door.open_run().unwrap();
    let _ = f.door.cancel_run(cancelled).unwrap().write();
    assert_eq!(
        class(
            f.door
                .admit_inference(cancelled, HASH, &label(1), identity())
        ),
        "run_cancelled"
    );
    let finished = f.door.open_run().unwrap();
    f.door.finish_when_settled(finished).unwrap();
    assert_eq!(
        class(
            f.door
                .admit_inference(finished, HASH, &label(1), identity())
        ),
        "run_not_active"
    );
    let (_, owner_run) = f.gov.open_owner_run(vec![]).unwrap();
    assert!(f
        .door
        .admit_inference(owner_run, HASH, &label(1), identity())
        .is_err());
    assert_eq!(f.admission(cancelled), None);
    assert_eq!(f.admission(finished), None);
    assert_eq!(f.admission(owner_run), None);

    // (h) sticky.
    let run = f.door.open_run().unwrap();
    f.door
        .admit_inference(run, HASH, &label(1), identity())
        .unwrap();
    let first = f.admission(run).unwrap();
    assert_eq!(
        class(f.door.admit_inference(run, HASH2, &label(2), identity())),
        "already_admitted"
    );
    assert_eq!(f.admission(run), Some(first.clone()));
    f.door
        .admit_inference(run, HASH, &label(1), identity())
        .unwrap();
    assert_eq!(f.admission(run), Some(first));

    // (i) another door's yes admits nothing here.
    let other = f.door();
    f.consent(&other, HASH, &label(3));
    let mine = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_inference(mine, HASH, &label(3), identity())),
        "no_consent"
    );
    assert_eq!(
        class(other.admit_inference(mine, HASH, &label(3), identity())),
        "not_member"
    );
    assert_eq!(f.admission(mine), None);

    // A held-mission resume yes admits, as its kind.
    let approval = f.door.confirm_held_mission(&held(&label(4))).unwrap();
    f.door
        .admit_inference(mine, HASH, &label(4), identity())
        .unwrap();
    let admission = f.admission(mine).unwrap();
    assert_eq!(admission.kind, ConsentKind::Resume);
    assert_eq!(admission.evidence_seq, approval.evidence_seq());
}

/// The model classes are `model_` and the kernel's codes, in order, and
/// each is a registered reason.
#[test]
fn every_model_class_is_its_kernel_code_and_a_registered_reason() {
    use nexus_kernel::inference::ModelError;
    for (class, code) in super::MODEL_CLASSES.iter().zip(ModelError::CODES) {
        assert_eq!(*class, format!("model_{code}"));
        assert!(crate::authority::REASONS.contains(class), "{class}");
    }
    for extra in ["model_not_registered", "model_identity_mismatch"] {
        assert!(crate::authority::REASONS.contains(&extra), "{extra}");
    }
}
