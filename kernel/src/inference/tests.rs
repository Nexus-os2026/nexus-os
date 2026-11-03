//! M2e-A1 pre-registered oracles (design v2 §14, kernel rows for A1, with
//! the Architect's binding note N2): E-O1–E-O11, E-O16, E-O17 and the
//! transport half of E-O18. E-O14 is the source guard
//! `kernel/tests/m2e_transport_guards.rs`.
//!
//! Every runtime here is a fake Ollama on a loopback port of this process;
//! the real Ollama is never contacted. Tests synchronize on the fake's
//! request and close events, never on sleeps, and stop only private
//! emergency-stop authorities.

use super::exchange::seams;
use super::grammar::{self, EditAction};
use super::*;
use crate::emergency_stop::{StopAuthority, StopController, StopWatch};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NAME: &str = "qwen3.6:35b-a3b";
const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
const OTHER_DIGEST: &str = "9ec8897f747e246e970bc5cfdda85d22f1123dc2e3d34978a010a75968716849";
const VERSION: &str = "0.40.13";
/// The return bound of a cancelled, stopped or expired request (§7.2).
const BOUND: Duration = Duration::from_secs(1);
/// How long a test waits for something that must happen promptly.
const PATIENCE: Duration = Duration::from_secs(10);
/// The qualified reply schema, compact, in the qualified key order
/// (`json.dumps(SCHEMA, separators=(",", ":"))` of the qualification
/// harness: 733 bytes, SHA-256 6dec685d…).
const QUALIFIED_SCHEMA: &str = r#"{"anyOf":[{"type":"object","properties":{"action":{"type":"string","enum":["read"]},"paths":{"type":"array","items":{"type":"string"}}},"required":["action","paths"],"additionalProperties":false},{"type":"object","properties":{"action":{"type":"string","enum":["edit"]},"edits":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string"},"op":{"type":"string","enum":["create","replace"]},"content":{"type":"string"}},"required":["path","op","content"],"additionalProperties":false}}},"required":["action","edits"],"additionalProperties":false},{"type":"object","properties":{"action":{"type":"string","enum":["finish"]},"summary":{"type":"string"}},"required":["action","summary"],"additionalProperties":false}]}"#;
const QUALIFIED_SCHEMA_SHA256: &str =
    "6dec685d0501e718c903c648a7bf9a7766a118731ef94b376a124b6aad5b0dac";
/// `/api/status` with cloud disabled. The key names are the pinned
/// runtime's `StatusResponse{Cloud{Disabled, Source}}` (`cloud.disabled`,
/// `cloud.source`); Q-R4's installed capture confirms the fixtures.
const STATUS_DISABLED: &str = r#"{"cloud":{"disabled":true,"source":"env"}}"#;
const FINISH: &str = r#"{"action":"finish","summary":"done"}"#;

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn qualified_config() -> RuntimeConfig {
    RuntimeConfig {
        num_ctx: 8192,
        temperature_milli: 0,
        seed: 7,
        think: Think::Off,
        truncate: false,
    }
}

fn spec() -> ModelPinSpec {
    ModelPinSpec {
        name: NAME.to_string(),
        digest: DIGEST.to_string(),
        runtime_version: VERSION.to_string(),
        config: qualified_config(),
    }
}

// ── the fake runtime ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Request(String),
    Closed(String),
}

#[derive(Clone)]
enum Reply {
    /// A JSON reply with this status.
    Json(u16, String),
    /// Held open until the client closes the connection.
    Hold,
    /// These exact response bytes.
    Raw(String),
    /// Closed without a response.
    Close,
}

type Route = Arc<dyn Fn(&str) -> Reply + Send + Sync>;
type Log = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

struct Fake {
    addr: SocketAddr,
    events: Receiver<Event>,
    log: Log,
    connections: Arc<AtomicUsize>,
    route: Arc<Mutex<Route>>,
}

impl Fake {
    fn start(route: Route) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, events) = mpsc::channel();
        let log: Log = Arc::default();
        let connections = Arc::new(AtomicUsize::new(0));
        let route = Arc::new(Mutex::new(route));
        {
            let (log, connections, route) = (log.clone(), connections.clone(), route.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { return };
                    connections.fetch_add(1, Ordering::SeqCst);
                    let (tx, log, route) = (tx.clone(), log.clone(), route.clone());
                    std::thread::spawn(move || serve(stream, &tx, &log, &route));
                }
            });
        }
        Self {
            addr,
            events,
            log,
            connections,
            route,
        }
    }

    fn healthy() -> Self {
        Self::start(route(|_| None))
    }

    fn endpoint(&self) -> LoopbackEndpoint {
        LoopbackEndpoint::parse(&format!("http://{}", self.addr)).unwrap()
    }

    fn set_route(&self, route: Route) {
        *self.route.lock().unwrap() = route;
    }

    fn paths(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    }

    fn count(&self, path: &str) -> usize {
        self.paths().iter().filter(|seen| *seen == path).count()
    }

    fn last_body(&self, path: &str) -> String {
        let log = self.log.lock().unwrap();
        let (_, body) = log
            .iter()
            .rev()
            .find(|(seen, _)| seen == path)
            .expect("a request to the path");
        String::from_utf8(body.clone()).unwrap()
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// Wait for `want`, skipping other events.
    #[track_caller]
    fn wait_for(&self, want: Event) {
        let deadline = std::time::Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.events.recv_timeout(left) {
                Ok(event) if event == want => return,
                Ok(_) => {}
                Err(_) => panic!("the fake runtime never saw {want:?}"),
            }
        }
    }

    /// Forget the events so far (the requests that produced them have been
    /// answered).
    fn drain(&self) {
        while self.events.try_recv().is_ok() {}
    }
}

fn serve(mut stream: TcpStream, tx: &Sender<Event>, log: &Log, route: &Arc<Mutex<Route>>) {
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
    log.lock().unwrap().push((path.clone(), body));
    let reply = (route.lock().unwrap().clone())(&path);
    let _ = tx.send(Event::Request(path.clone()));
    match reply {
        Reply::Json(status, body) => {
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
        Reply::Raw(raw) => {
            let _ = stream.write_all(raw.as_bytes());
        }
        Reply::Close => {}
        Reply::Hold => {
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

fn tags(digest: &str) -> String {
    json!({"models": [
        {"name": "gemma4:26b", "model": "gemma4:26b", "digest": OTHER_DIGEST, "size": 1, "details": {}},
        {"name": NAME, "model": NAME, "digest": digest, "size": 1, "details": {}},
    ]})
    .to_string()
}

fn show() -> String {
    json!({"modelfile": "FROM x", "details": {}, "capabilities": ["completion", "vision", "tools", "thinking"]})
        .to_string()
}

fn chat_value() -> Value {
    json!({
        "model": NAME,
        "created_at": "2026-11-03T06:10:00Z",
        "message": {"role": "assistant", "content": FINISH},
        "done": true,
        "done_reason": "stop",
        "total_duration": 1,
        "load_duration": 1,
        "prompt_eval_count": 587,
        "prompt_eval_cached_count": 0,
        "eval_count": 378,
        "eval_duration": 1,
    })
}

fn healthy(path: &str) -> Reply {
    match path {
        "/api/version" => Reply::Json(200, json!({"version": VERSION}).to_string()),
        "/api/status" => Reply::Json(200, STATUS_DISABLED.to_string()),
        "/api/tags" => Reply::Json(200, tags(DIGEST)),
        "/api/show" => Reply::Json(200, show()),
        "/api/chat" => Reply::Json(200, chat_value().to_string()),
        _ => Reply::Json(404, r#"{"error":"not found"}"#.to_string()),
    }
}

/// The healthy runtime, but for the paths `over` answers.
fn route(over: impl Fn(&str) -> Option<Reply> + Send + Sync + 'static) -> Route {
    Arc::new(move |path: &str| over(path).unwrap_or_else(|| healthy(path)))
}

fn on(path: &'static str, reply: Reply) -> Route {
    route(move |seen| (seen == path).then(|| reply.clone()))
}

fn chat(value: Value) -> Route {
    on("/api/chat", Reply::Json(200, value.to_string()))
}

// ── the caller's side ───────────────────────────────────────────────────────

/// Two manual clocks (wall, monotonic), shared by clones.
#[derive(Clone, Default)]
struct Clocks(Arc<(AtomicU64, AtomicU64)>);

impl Clocks {
    fn advance_wall(&self, ms: u64) {
        self.0 .0.fetch_add(ms, Ordering::SeqCst);
    }

    fn advance_monotonic(&self, ms: u64) {
        self.0 .1.fetch_add(ms, Ordering::SeqCst);
    }
}

impl DeadlineClock for Clocks {
    fn wall_ms(&self) -> u64 {
        self.0 .0.load(Ordering::SeqCst)
    }

    fn monotonic_ms(&self) -> u64 {
        self.0 .1.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct Flag(AtomicBool);

impl CallCancel for Flag {
    fn cancelled(&self) -> Option<CancelCause> {
        self.0
            .load(Ordering::SeqCst)
            .then_some(CancelCause::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Metered {
    Charge(u32),
    Settle(u32),
}

/// No charge and no settlement.
const NOTHING: [Metered; 0] = [];

/// A meter that records its charges and settlements, and the runtime's
/// requests at the moment of each charge.
#[derive(Default)]
struct Meter {
    events: Mutex<Vec<Metered>>,
    refuse: AtomicBool,
    server: Mutex<Option<Log>>,
    at_charge: Mutex<Vec<Vec<String>>>,
}

impl Meter {
    fn events(&self) -> Vec<Metered> {
        self.events.lock().unwrap().clone()
    }
}

impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(MeterRefusal::Exhausted);
        }
        if let Some(log) = self.server.lock().unwrap().as_ref() {
            let paths = log.lock().unwrap().iter().map(|(p, _)| p.clone()).collect();
            self.at_charge.lock().unwrap().push(paths);
        }
        let mut events = self.events.lock().unwrap();
        let call = events
            .iter()
            .filter(|event| matches!(event, Metered::Charge(_)))
            .count() as u32
            + 1;
        events.push(Metered::Charge(call));
        Ok(Charged { call })
    }

    fn settle(&self, call: u32) {
        self.events.lock().unwrap().push(Metered::Settle(call));
    }
}

struct Ctx {
    meter: Meter,
    cancel: Flag,
    clocks: Clocks,
    deadline: AbsoluteDeadline<Clocks>,
    stop: Arc<StopAuthority>,
    controller: StopController,
    watch: StopWatch,
}

impl Ctx {
    /// A caller with an hour on both clocks, a private stop authority and
    /// a watch admitted now.
    fn new() -> Arc<Self> {
        Self::with_deadline(Clocks::default(), 3_600_000, 3_600_000)
    }

    fn with_deadline(clocks: Clocks, wall_at_ms: u64, monotonic_at_ms: u64) -> Arc<Self> {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        let watch = stop.admit().unwrap();
        Arc::new(Self {
            meter: Meter::default(),
            cancel: Flag::default(),
            deadline: AbsoluteDeadline::at(clocks.clone(), wall_at_ms, monotonic_at_ms),
            clocks,
            stop,
            controller,
            watch,
        })
    }

    fn observing(self: Arc<Self>, fake: &Fake) -> Arc<Self> {
        *self.meter.server.lock().unwrap() = Some(fake.log.clone());
        self
    }

    fn call_ctx<'a>(&'a self, watch: &'a StopWatch) -> CallContext<'a> {
        CallContext {
            meter: &self.meter,
            cancel: &self.cancel,
            deadline: &self.deadline,
            stop: watch,
            observer: None,
        }
    }

    fn charges(&self) -> usize {
        self.meter
            .events()
            .iter()
            .filter(|event| matches!(event, Metered::Charge(_)))
            .count()
    }
}

/// What ends a held request in a schedule.
#[derive(Debug, Clone, Copy)]
enum Cause {
    Cancel,
    Stop,
    /// The monotonic clock passes the deadline.
    Deadline,
    /// The wall clock alone passes the deadline (a suspend, N2).
    Suspend,
}

impl Cause {
    const ALL: [Cause; 4] = [Cause::Cancel, Cause::Stop, Cause::Deadline, Cause::Suspend];

    fn trigger(self, ctx: &Ctx) {
        match self {
            Cause::Cancel => ctx.cancel.0.store(true, Ordering::SeqCst),
            Cause::Stop => {
                ctx.controller.engage();
            }
            Cause::Deadline => ctx.clocks.advance_monotonic(3_600_000),
            Cause::Suspend => ctx.clocks.advance_wall(3_600_000),
        }
    }

    fn code(self) -> ModelError {
        match self {
            Cause::Cancel => ModelError::Cancelled,
            Cause::Stop => ModelError::Stopped,
            Cause::Deadline | Cause::Suspend => ModelError::Timeout,
        }
    }
}

/// A call's result, how many exchange threads it ran and how many were
/// still alive when it returned.
struct Outcome<T> {
    result: Result<T, ModelError>,
    exchanges: usize,
    live: usize,
}

impl<T> Outcome<T> {
    fn code(&self) -> Result<(), ModelError> {
        self.result.as_ref().map(|_| ()).map_err(|error| *error)
    }
}

/// Run `work` on its own thread, with the exchange probe (and the
/// after-reply hook, if any) set there.
fn in_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ModelError> + Send + 'static,
    after_reply: Option<Box<dyn Fn() + Send>>,
) -> Receiver<Outcome<T>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let probe = Arc::new(AtomicUsize::new(0));
        seams::set_probe(probe.clone());
        if let Some(hook) = after_reply {
            seams::set_after_reply(hook);
        }
        let result = work();
        seams::clear_probe();
        let _ = tx.send(Outcome {
            result,
            exchanges: probe.load(Ordering::SeqCst),
            live: Arc::strong_count(&probe) - 1,
        });
    });
    rx
}

#[track_caller]
fn done<T>(rx: &Receiver<Outcome<T>>) -> Outcome<T> {
    rx.recv_timeout(PATIENCE).expect("the call returned")
}

fn verify_in_thread(
    fake: &Fake,
    ctx: &Arc<Ctx>,
    spec: ModelPinSpec,
) -> Receiver<Outcome<VerifiedOllama>> {
    let (endpoint, ctx) = (fake.endpoint(), ctx.clone());
    in_thread(
        move || VerifiedOllama::verify(endpoint, &spec, &ctx.cancel, &ctx.deadline, &ctx.watch),
        None,
    )
}

fn verify(fake: &Fake, ctx: &Arc<Ctx>) -> Result<VerifiedOllama, ModelError> {
    done(&verify_in_thread(fake, ctx, spec())).result
}

/// A transport verified against `fake`, whose events are then forgotten.
fn verified(fake: &Fake, ctx: &Arc<Ctx>) -> Arc<VerifiedOllama> {
    let transport = Arc::new(verify(fake, ctx).expect("registration"));
    fake.drain();
    transport
}

fn call_with(
    transport: &Arc<VerifiedOllama>,
    ctx: &Arc<Ctx>,
    request: ModelRequest,
    watch: StopWatch,
    after_reply: Option<Box<dyn Fn() + Send>>,
) -> Receiver<Outcome<ModelReply>> {
    let (transport, ctx) = (transport.clone(), ctx.clone());
    in_thread(
        move || transport.call(&request, &ctx.call_ctx(&watch)),
        after_reply,
    )
}

fn call_in_thread(
    transport: &Arc<VerifiedOllama>,
    ctx: &Arc<Ctx>,
    request: ModelRequest,
) -> Receiver<Outcome<ModelReply>> {
    call_with(transport, ctx, request, ctx.watch.clone(), None)
}

fn call(transport: &Arc<VerifiedOllama>, ctx: &Arc<Ctx>) -> Outcome<ModelReply> {
    done(&call_in_thread(transport, ctx, small()))
}

fn request(system: &str, user: &str) -> ModelRequest {
    ModelRequest::new(
        system.to_string(),
        user.to_string(),
        grammar::worker_schema(),
    )
    .unwrap()
}

fn small() -> ModelRequest {
    request("system", "user")
}

/// Wait at most [`BOUND`] for a call ended at this moment.
#[track_caller]
fn within_bound<T>(rx: &Receiver<Outcome<T>>, what: &str) -> Outcome<T> {
    rx.recv_timeout(BOUND)
        .unwrap_or_else(|_| panic!("{what}: the call did not return within 1 s"))
}

// ── E-O1: the loopback address ──────────────────────────────────────────────

#[test]
fn e_o1_loopback_endpoint_refuses_every_other_address() {
    for raw in [
        "http://192.0.2.1:9/",
        "http://10.0.0.5:11434",
        "http://192.168.1.2:11434",
        "http://8.8.8.8:11434",
        "http://0.0.0.0:11434",
        "http://[::]:11434",
        "http://127.0.0.2:11434",
        "http://localhost.example.com:11434",
        "http://127.0.0.1.nip.io:11434",
        "http://ollama.example.com:11434",
        "http://[::ffff:10.0.0.1]:11434",
        "http://[::ffff:127.0.0.1]:11434",
    ] {
        assert_eq!(
            LoopbackEndpoint::parse(raw).err(),
            Some(ModelError::NotLocal),
            "{raw}"
        );
    }
    for raw in [
        "https://127.0.0.1:11434",
        "http://user@127.0.0.1:11434",
        "http://user:secret@127.0.0.1:11434",
        "http://127.0.0.1:11434/api",
        "http://127.0.0.1:11434/?q=1",
        "http://127.0.0.1:11434/#f",
        "http://127.0.0.1",
        "ftp://127.0.0.1:21",
        "127.0.0.1:11434",
        " http://127.0.0.1:11434",
        "http://127.0.0.1:11434\n",
        "",
    ] {
        assert_eq!(
            LoopbackEndpoint::parse(raw).err(),
            Some(ModelError::InvalidAddress),
            "{raw:?}"
        );
    }
    for (raw, addr) in [
        ("http://127.0.0.1:11434", "127.0.0.1:11434"),
        ("http://127.0.0.1:11434/", "127.0.0.1:11434"),
        ("http://localhost:11434", "127.0.0.1:11434"),
        ("http://LOCALHOST:8080/", "127.0.0.1:8080"),
        ("http://[::1]:11434", "[::1]:11434"),
    ] {
        let endpoint = LoopbackEndpoint::parse(raw).unwrap();
        assert_eq!(
            endpoint.addr(),
            addr.parse::<SocketAddr>().unwrap(),
            "{raw}"
        );
    }
}

// ── E-O2: registration ──────────────────────────────────────────────────────

#[test]
fn e_o2_registration_refuses_each_unverified_model_with_its_code() {
    let big = format!(r#"{{"models":[],"pad":"{}"}}"#, "x".repeat(1024 * 1024));
    let cases: Vec<(&str, Route, ModelError)> = vec![
        (
            "the name with another digest",
            on("/api/tags", Reply::Json(200, tags(OTHER_DIGEST))),
            ModelError::DigestMismatch,
        ),
        (
            "show names a remote host and model",
            on(
                "/api/show",
                Reply::Json(
                    200,
                    r#"{"remote_host":"https://ollama.com:443","remote_model":"gpt-oss:120b","capabilities":["thinking"]}"#.into(),
                ),
            ),
            ModelError::Remote,
        ),
        (
            "show names a remote model",
            on(
                "/api/show",
                Reply::Json(200, r#"{"remote_model":"qwen3:8b","capabilities":["thinking"]}"#.into()),
            ),
            ModelError::Remote,
        ),
        (
            "the tags entry names a remote",
            on(
                "/api/tags",
                Reply::Json(
                    200,
                    json!({"models": [{"name": NAME, "digest": DIGEST, "remote_model": "qwen3.6", "remote_host": "https://ollama.com:443"}]}).to_string(),
                ),
            ),
            ModelError::Remote,
        ),
        (
            "tags miss the name",
            on("/api/tags", Reply::Json(200, r#"{"models":[{"name":"gemma4:26b","digest":"00"}]}"#.into())),
            ModelError::NotInstalled,
        ),
        (
            "tags list another tag of the model",
            on(
                "/api/tags",
                Reply::Json(200, json!({"models": [{"name": "qwen3.6:latest", "digest": DIGEST}]}).to_string()),
            ),
            ModelError::NotInstalled,
        ),
        (
            "tags list the name twice",
            on(
                "/api/tags",
                Reply::Json(
                    200,
                    json!({"models": [{"name": NAME, "digest": DIGEST}, {"name": NAME, "digest": DIGEST}]}).to_string(),
                ),
            ),
            ModelError::Malformed,
        ),
        (
            "the tags entry has no digest",
            on("/api/tags", Reply::Json(200, json!({"models": [{"name": NAME}]}).to_string())),
            ModelError::DigestMismatch,
        ),
        (
            "tags are not a model list",
            on("/api/tags", Reply::Json(200, r#"{"models":"x"}"#.into())),
            ModelError::Malformed,
        ),
        (
            "show lacks the thinking capability",
            on(
                "/api/show",
                Reply::Json(200, r#"{"capabilities":["completion","tools"]}"#.into()),
            ),
            ModelError::ConfigMismatch,
        ),
        (
            "show lists no capabilities",
            on("/api/show", Reply::Json(200, r#"{"details":{}}"#.into())),
            ModelError::ConfigMismatch,
        ),
        (
            "show answers 404",
            on("/api/show", Reply::Json(404, r#"{"error":"model not found"}"#.into())),
            ModelError::Unavailable,
        ),
        (
            "show answers 500",
            on("/api/show", Reply::Json(500, r#"{"error":"failed"}"#.into())),
            ModelError::Unavailable,
        ),
        (
            "show is not a description",
            on("/api/show", Reply::Json(200, r#"["not","a","description"]"#.into())),
            ModelError::Malformed,
        ),
        (
            "show is empty",
            on("/api/show", Reply::Json(200, String::new())),
            ModelError::Malformed,
        ),
        (
            "tags answer 500",
            on("/api/tags", Reply::Json(500, r#"{"error":"failed"}"#.into())),
            ModelError::Unavailable,
        ),
        (
            "tags exceed the metadata cap",
            on("/api/tags", Reply::Json(200, big)),
            ModelError::ResponseTooLarge,
        ),
        (
            "another runtime version",
            on("/api/version", Reply::Json(200, r#"{"version":"0.40.12"}"#.into())),
            ModelError::RuntimeMismatch,
        ),
        (
            "version answers 500",
            on("/api/version", Reply::Json(500, "{}".into())),
            ModelError::Unavailable,
        ),
    ];
    for (what, route, code) in cases {
        let fake = Fake::start(route);
        let ctx = Ctx::new();
        assert_eq!(verify(&fake, &ctx).err(), Some(code), "{what}");
    }

    // Spelling refusals: no request is made.
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let refused = |change: &dyn Fn(&mut ModelPinSpec)| {
        let mut spec = spec();
        change(&mut spec);
        done(&verify_in_thread(&fake, &ctx, spec)).result.err()
    };
    for name in [
        "gpt-oss:120b-cloud",
        "glm-4.6:cloud",
        "qwen3:cloud",
        "Cloud/qwen3:8b",
    ] {
        assert_eq!(
            refused(&|spec| spec.name = name.to_string()),
            Some(ModelError::CloudName),
            "{name}"
        );
    }
    for name in ["", "  ", "-qwen", "qwen 3", "qwen3;rm"] {
        assert_eq!(
            refused(&|spec| spec.name = name.to_string()),
            Some(ModelError::InvalidModelName),
            "{name:?}"
        );
    }
    for digest in [
        format!("sha256:{DIGEST}"),
        DIGEST.to_ascii_uppercase(),
        DIGEST[1..].to_string(),
        String::new(),
    ] {
        assert_eq!(
            refused(&|spec| spec.digest = digest.clone()),
            Some(ModelError::DigestMismatch),
            "{digest}"
        );
    }
    assert_eq!(
        refused(&|spec| spec.runtime_version = "0.40.13 ".to_string()),
        Some(ModelError::RuntimeMismatch)
    );
    for change in [
        (|spec: &mut ModelPinSpec| spec.config.truncate = true) as fn(&mut ModelPinSpec),
        |spec| spec.config.num_ctx = 4096,
        |spec| spec.config.temperature_milli = 2001,
    ] {
        assert_eq!(refused(&change), Some(ModelError::ConfigMismatch));
    }
    assert_eq!(fake.connections(), 0, "a refused spelling makes no request");

    // Control: the qualified model verifies, with the pinned identity.
    let transport = verify(&fake, &ctx).expect("registration");
    let identity = transport.identity();
    assert_eq!(
        (
            identity.name(),
            identity.model_digest(),
            identity.runtime_version()
        ),
        (NAME, DIGEST, VERSION)
    );
    assert_eq!(*identity.config(), qualified_config());
    assert_eq!(
        fake.paths(),
        ["/api/version", "/api/status", "/api/tags", "/api/show"]
    );
}

// ── E-O2b: drift after registration ─────────────────────────────────────────

#[test]
fn e_o2b_drift_after_registration_is_refused_before_any_charge() {
    for (what, drift, code) in [
        (
            "another runtime version",
            on(
                "/api/version",
                Reply::Json(200, r#"{"version":"0.41.0"}"#.into()),
            ),
            ModelError::RuntimeMismatch,
        ),
        (
            "another digest under the same name",
            on("/api/tags", Reply::Json(200, tags(OTHER_DIGEST))),
            ModelError::DigestMismatch,
        ),
        (
            "the model was removed",
            on("/api/tags", Reply::Json(200, r#"{"models":[]}"#.into())),
            ModelError::NotInstalled,
        ),
    ] {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        fake.set_route(drift);
        let outcome = call(&transport, &ctx);
        assert_eq!(outcome.code(), Err(code), "{what}");
        assert_eq!(ctx.meter.events(), NOTHING, "{what}: nothing charged");
        assert_eq!(fake.count("/api/chat"), 0, "{what}: no chat");
        // Control: once the runtime is the pinned one again, a call runs.
        fake.set_route(route(|_| None));
        assert_eq!(call(&transport, &ctx).code(), Ok(()), "{what}");
    }
}

// ── E-O2c: the runtime's cloud status ───────────────────────────────────────

#[test]
fn e_o2c_cloud_status_must_report_disabled_at_registration_and_before_each_call() {
    let not_disabled = [
        Reply::Json(
            200,
            r#"{"cloud":{"disabled":false,"source":"default"}}"#.into(),
        ),
        Reply::Json(200, r#"{"cloud":{"source":"env"}}"#.into()),
        Reply::Json(
            200,
            r#"{"cloud":{"disabled":"true","source":"env"}}"#.into(),
        ),
        Reply::Json(200, r#"{"cloud":{}}"#.into()),
        Reply::Json(200, "{}".into()),
        Reply::Json(200, "not json".into()),
        Reply::Json(404, "404 page not found".into()),
        Reply::Json(500, STATUS_DISABLED.into()),
    ];
    for reply in not_disabled.clone() {
        let fake = Fake::start(on("/api/status", reply));
        let ctx = Ctx::new();
        assert_eq!(verify(&fake, &ctx).err(), Some(ModelError::CloudEnabled));
        assert_eq!(fake.count("/api/tags"), 0, "nothing after the status");
    }
    for reply in not_disabled {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        fake.set_route(on("/api/status", reply));
        assert_eq!(call(&transport, &ctx).code(), Err(ModelError::CloudEnabled));
        assert_eq!(ctx.meter.events(), NOTHING, "nothing charged");
        assert_eq!(fake.count("/api/chat"), 0, "no chat");
    }
}

// ── E-O3: the frozen request ────────────────────────────────────────────────

#[test]
fn e_o3_the_chat_body_is_the_frozen_qualified_body() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let outcome = done(&call_in_thread(
        &transport,
        &ctx,
        request("S \"q\"\n1", "U \u{e9}\t2"),
    ));
    assert_eq!(outcome.code(), Ok(()));
    let expected = [
        r#"{"model":"qwen3.6:35b-a3b","stream":false,"think":false,"truncate":false,"format":"#,
        QUALIFIED_SCHEMA,
        r#","options":{"temperature":0,"num_ctx":8192,"seed":7},"messages":["#,
        r#"{"role":"system","content":"S \"q\"\n1"},{"role":"user","content":"U é\t2"}]}"#,
    ]
    .concat();
    assert_eq!(fake.last_body("/api/chat"), expected);
}

// ── E-O4: the response schema ───────────────────────────────────────────────

#[test]
fn e_o4_response_schema_refuses_prose_keys_at_any_depth() {
    for key in [
        "description",
        "title",
        "examples",
        "default",
        "$comment",
        "Description",
        "TITLE",
    ] {
        for schema in [
            json!({"type": "object", key: "x"}),
            json!({"type": "object", "properties": {"a": {"type": "string", key: "x"}}}),
            json!({"anyOf": [{"type": "string"}, {"type": "array", "items": {"type": "string", key: ["x"]}}]}),
            json!({"type": "object", "properties": {key: {"type": "string"}}}),
        ] {
            assert_eq!(
                ResponseSchema::new(&schema.to_string()).err(),
                Some(ModelError::Malformed),
                "{schema}"
            );
        }
    }
    for schema in [
        r#"{"type":"object","type":"string"}"#.to_string(),
        r#"{"properties":{"a":{"type":"string","type":"number"}}}"#.to_string(),
        "[]".to_string(),
        "\"object\"".to_string(),
        "not json".to_string(),
    ] {
        assert_eq!(
            ResponseSchema::new(&schema).err(),
            Some(ModelError::Malformed),
            "{schema}"
        );
    }
    let oversize = json!({"enum": vec!["x".repeat(64); 300]}).to_string();
    assert_eq!(
        ResponseSchema::new(&oversize).err(),
        Some(ModelError::RequestTooLarge)
    );

    // The qualified schema passes and keeps its key order.
    let worker = grammar::worker_schema();
    assert_eq!(worker.as_json(), QUALIFIED_SCHEMA);
    assert_eq!(
        sha256_hex(worker.as_json().as_bytes()),
        QUALIFIED_SCHEMA_SHA256
    );
    let spaced = QUALIFIED_SCHEMA.replace(',', ", ").replace(':', ": ");
    assert_eq!(ResponseSchema::new(&spaced).unwrap(), worker);
}

// ── E-O5: order, charge and no refund ───────────────────────────────────────

#[test]
fn e_o5_order_is_version_status_tags_charge_chat_and_nothing_is_refunded() {
    let fake = Fake::healthy();
    let ctx = Ctx::new().observing(&fake);
    let transport = verified(&fake, &ctx);
    let from = fake.paths().len();

    let outcome = call(&transport, &ctx);
    assert_eq!(outcome.code(), Ok(()));
    assert_eq!(
        ctx.meter.events(),
        [Metered::Charge(1), Metered::Settle(1)],
        "one charge, settled"
    );
    let at_charge = ctx.meter.at_charge.lock().unwrap().clone();
    assert_eq!(
        at_charge
            .iter()
            .map(|paths| paths[from..].to_vec())
            .collect::<Vec<_>>(),
        [["/api/version", "/api/status", "/api/tags"]],
        "charged after the metadata and before the chat"
    );
    assert_eq!(
        fake.paths()[from..],
        ["/api/version", "/api/status", "/api/tags", "/api/chat"]
    );
    assert_eq!(
        outcome.exchanges, 4,
        "every request ran through the wrapper"
    );

    // A refused charge sends no chat and settles nothing.
    ctx.meter.refuse.store(true, Ordering::SeqCst);
    let from = fake.paths().len();
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::OverBudget));
    assert_eq!(
        fake.paths()[from..],
        ["/api/version", "/api/status", "/api/tags"]
    );
    assert_eq!(ctx.meter.events().len(), 2);
    ctx.meter.refuse.store(false, Ordering::SeqCst);

    // A failed validation makes no request and no charge.
    let from = fake.paths().len();
    let oversize =
        ModelRequest::unchecked("s".repeat(2000), "u".repeat(1937), grammar::worker_schema());
    let outcome = done(&call_in_thread(&transport, &ctx, oversize));
    assert_eq!(outcome.code(), Err(ModelError::RequestTooLarge));
    assert_eq!(fake.paths().len(), from);
    assert_eq!(ctx.meter.events().len(), 2);

    // Failed metadata makes no charge.
    fake.set_route(on("/api/tags", Reply::Json(500, "{}".into())));
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::Unavailable));
    assert_eq!(ctx.meter.events().len(), 2);
    assert_eq!(fake.count("/api/chat"), 1);

    // A refused reply stays charged: settled, never refunded or retried.
    let mut truncated = chat_value();
    truncated["done_reason"] = json!("length");
    fake.set_route(chat(truncated));
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::Truncated));
    assert_eq!(
        ctx.meter.events()[2..],
        [Metered::Charge(2), Metered::Settle(2)]
    );
    assert_eq!(fake.count("/api/chat"), 2);
}

// ── E-O6: a held chat ends on cancel, Stop or the deadline ──────────────────

fn held_chat_ends(cause: Cause) {
    let fake = Fake::start(on("/api/chat", Reply::Hold));
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let rx = call_in_thread(&transport, &ctx, small());
    fake.wait_for(Event::Request("/api/chat".into()));
    cause.trigger(&ctx);
    let outcome = within_bound(&rx, &format!("held chat, {cause:?}"));
    assert_eq!(outcome.code(), Err(cause.code()), "{cause:?}");
    assert_eq!(outcome.live, 0, "{cause:?}: the exchange thread was joined");
    fake.wait_for(Event::Closed("/api/chat".into()));
    assert_eq!(
        ctx.meter.events(),
        [Metered::Charge(1), Metered::Settle(1)],
        "{cause:?}: charged once, settled, not refunded"
    );
    if let Cause::Stop = cause {
        let halts = ctx.stop.halts();
        assert_eq!(halts.len(), 1, "the stop's halt is recorded");
        assert_eq!(halts[0].area, crate::emergency_stop::HaltArea::ModelCall);
    }
}

#[test]
fn e_o6_a_held_chat_ends_on_a_private_stop() {
    held_chat_ends(Cause::Stop);
}

#[test]
fn e_o6_a_held_chat_ends_on_cancel() {
    held_chat_ends(Cause::Cancel);
}

#[test]
fn e_o6_a_held_chat_ends_at_the_deadline() {
    held_chat_ends(Cause::Deadline);
}

// ── E-O6b: held metadata ends the same way, and nothing is charged ──────────

#[test]
fn e_o6b_held_metadata_at_registration_ends_within_the_bound() {
    for path in ["/api/tags", "/api/version", "/api/status", "/api/show"] {
        for cause in [Cause::Stop, Cause::Cancel, Cause::Deadline] {
            let fake = Fake::start(on(path, Reply::Hold));
            let ctx = Ctx::new();
            let rx = verify_in_thread(&fake, &ctx, spec());
            fake.wait_for(Event::Request(path.into()));
            cause.trigger(&ctx);
            let outcome = within_bound(&rx, &format!("{path} held at registration, {cause:?}"));
            assert_eq!(outcome.code(), Err(cause.code()), "{path} {cause:?}");
            assert_eq!(outcome.live, 0, "{path} {cause:?}: joined");
            assert_eq!(
                outcome.exchanges,
                fake.paths().len(),
                "{path} {cause:?}: every request ran through the wrapper"
            );
            fake.wait_for(Event::Closed(path.into()));
        }
    }
}

#[test]
fn e_o6b_held_metadata_before_a_charge_ends_within_the_bound_and_charges_nothing() {
    for path in ["/api/tags", "/api/version", "/api/status"] {
        for cause in [Cause::Stop, Cause::Cancel, Cause::Deadline] {
            let fake = Fake::healthy();
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            let from = fake.paths().len();
            fake.set_route(on(path, Reply::Hold));
            let rx = call_in_thread(&transport, &ctx, small());
            fake.wait_for(Event::Request(path.into()));
            cause.trigger(&ctx);
            let outcome = within_bound(&rx, &format!("{path} held before a charge, {cause:?}"));
            assert_eq!(outcome.code(), Err(cause.code()), "{path} {cause:?}");
            assert_eq!(outcome.live, 0, "{path} {cause:?}: joined");
            assert_eq!(outcome.exchanges, fake.paths().len() - from);
            fake.wait_for(Event::Closed(path.into()));
            assert_eq!(
                ctx.meter.events(),
                NOTHING,
                "{path} {cause:?}: nothing charged"
            );
            assert_eq!(fake.count("/api/chat"), 0);
        }
    }
}

// ── E-O7: a reply after the end is late ─────────────────────────────────────

#[test]
fn e_o7_a_reply_after_the_call_ended_is_late_and_unused() {
    for cause in Cause::ALL {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        let hook_ctx = ctx.clone();
        let rx = call_with(
            &transport,
            &ctx,
            small(),
            ctx.watch.clone(),
            Some(Box::new(move || cause.trigger(&hook_ctx))),
        );
        let outcome = done(&rx);
        assert_eq!(outcome.code(), Err(ModelError::Late), "{cause:?}");
        assert_eq!(
            fake.count("/api/chat"),
            1,
            "{cause:?}: the reply did arrive"
        );
        assert_eq!(
            ctx.meter.events(),
            [Metered::Charge(1), Metered::Settle(1)],
            "{cause:?}"
        );
        if let Cause::Stop = cause {
            assert_eq!(ctx.stop.halts().len(), 1, "the stop's halt is recorded");
        }
    }
}

// ── E-O8: the reply's refusals ──────────────────────────────────────────────

#[test]
fn e_o8_each_reply_refusal_has_its_code() {
    fn with(change: impl Fn(&mut Value)) -> Value {
        let mut value = chat_value();
        change(&mut value);
        value
    }
    let cases: Vec<(&str, Value, ModelError)> = vec![
        (
            "the qualification's truncated reply",
            with(|v| {
                v["prompt_eval_count"] = json!(4098);
                v["eval_count"] = json!(6);
                v["message"]["content"] = json!("/>663");
            }),
            ModelError::Truncated,
        ),
        (
            "tool calls",
            with(|v| {
                v["message"]["content"] = json!("");
                v["message"]["tool_calls"] =
                    json!([{"function": {"name": "assistant", "arguments": {"action": "edit"}}}]);
            }),
            ModelError::ToolCalls,
        ),
        (
            "thinking that was switched off",
            with(|v| v["message"]["thinking"] = json!("We need to create index.html.")),
            ModelError::ThinkingLeaked,
        ),
        (
            "empty content",
            with(|v| v["message"]["content"] = json!("")),
            ModelError::EmptyContent,
        ),
        (
            "a prompt of half the context",
            with(|v| v["prompt_eval_count"] = json!(4096)),
            ModelError::Truncated,
        ),
        (
            "prompt and answer fill the context",
            with(|v| {
                v["prompt_eval_count"] = json!(4095);
                v["eval_count"] = json!(4097);
            }),
            ModelError::Truncated,
        ),
        (
            "done for length",
            with(|v| v["done_reason"] = json!("length")),
            ModelError::Truncated,
        ),
        (
            "no done reason",
            with(|v| {
                v.as_object_mut().unwrap().remove("done_reason");
            }),
            ModelError::Truncated,
        ),
        (
            "not done",
            with(|v| v["done"] = json!(false)),
            ModelError::Truncated,
        ),
        (
            "no done flag",
            with(|v| {
                v.as_object_mut().unwrap().remove("done");
            }),
            ModelError::Truncated,
        ),
        (
            "empty tool calls",
            with(|v| v["message"]["tool_calls"] = json!([])),
            ModelError::ToolCalls,
        ),
        (
            "no prompt count",
            with(|v| {
                v.as_object_mut().unwrap().remove("prompt_eval_count");
            }),
            ModelError::Malformed,
        ),
        (
            "no answer count",
            with(|v| {
                v.as_object_mut().unwrap().remove("eval_count");
            }),
            ModelError::Malformed,
        ),
        (
            "a count that is not a number",
            with(|v| v["prompt_eval_count"] = json!("587")),
            ModelError::Malformed,
        ),
        (
            "another model",
            with(|v| v["model"] = json!("gpt-oss:20b")),
            ModelError::Malformed,
        ),
        (
            "not the assistant",
            with(|v| v["message"]["role"] = json!("user")),
            ModelError::Malformed,
        ),
        (
            "an error field",
            with(|v| v["error"] = json!("failed")),
            ModelError::Malformed,
        ),
        (
            "content that is not text",
            with(|v| v["message"]["content"] = json!({"action": "finish"})),
            ModelError::Malformed,
        ),
        (
            "thinking that is not text",
            with(|v| v["message"]["thinking"] = json!(["x"])),
            ModelError::Malformed,
        ),
        (
            "content over 32 KiB",
            with(|v| v["message"]["content"] = json!("x".repeat(32 * 1024 + 1))),
            ModelError::ResponseTooLarge,
        ),
    ];
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    for (what, value, code) in cases {
        fake.set_route(chat(value));
        assert_eq!(call(&transport, &ctx).code(), Err(code), "{what}");
    }
    for (what, reply, code) in [
        (
            "not JSON",
            Reply::Json(200, "not json".into()),
            ModelError::Malformed,
        ),
        (
            "an envelope over 256 KiB",
            Reply::Json(
                200,
                with(|v| v["pad"] = json!("x".repeat(256 * 1024))).to_string(),
            ),
            ModelError::ResponseTooLarge,
        ),
        (
            "an error status",
            Reply::Json(400, r#"{"error":"the input exceeds the context"}"#.into()),
            ModelError::Unavailable,
        ),
    ] {
        fake.set_route(on("/api/chat", reply));
        assert_eq!(call(&transport, &ctx).code(), Err(code), "{what}");
    }

    // Controls: no false refusal at the boundaries.
    for (what, value) in [
        ("the healthy reply", chat_value()),
        (
            "just under both bounds",
            with(|v| {
                v["prompt_eval_count"] = json!(4095);
                v["eval_count"] = json!(4096);
            }),
        ),
        (
            "empty thinking",
            with(|v| v["message"]["thinking"] = json!("")),
        ),
        (
            "null thinking",
            with(|v| v["message"]["thinking"] = Value::Null),
        ),
    ] {
        fake.set_route(chat(value));
        let outcome = call(&transport, &ctx);
        let reply = outcome
            .result
            .unwrap_or_else(|error| panic!("{what}: {error:?}"));
        assert_eq!(reply.content(), FINISH, "{what}");
    }
    let reply = call(&transport, &ctx).result.unwrap();
    assert_eq!((reply.prompt_tokens(), reply.output_tokens()), (587, 378));
}

// ── E-O9: no retry ──────────────────────────────────────────────────────────

#[test]
fn e_o9_a_failed_chat_is_never_retried() {
    for (what, reply, code) in [
        (
            "status 500",
            Reply::Json(500, r#"{"error":"x"}"#.into()),
            ModelError::Unavailable,
        ),
        (
            "closed without a reply",
            Reply::Close,
            ModelError::Unavailable,
        ),
    ] {
        let fake = Fake::start(on("/api/chat", reply));
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        assert_eq!(call(&transport, &ctx).code(), Err(code), "{what}");
        assert_eq!(fake.count("/api/chat"), 1, "{what}: exactly one chat");
    }
    // A timeout: the caller's deadline ends a held chat.
    let fake = Fake::start(on("/api/chat", Reply::Hold));
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let rx = call_in_thread(&transport, &ctx, small());
    fake.wait_for(Event::Request("/api/chat".into()));
    Cause::Deadline.trigger(&ctx);
    assert_eq!(
        within_bound(&rx, "timeout").code(),
        Err(ModelError::Timeout)
    );
    assert_eq!(fake.count("/api/chat"), 1, "timeout: exactly one chat");
}

// ── E-O10: no redirect, no proxy ────────────────────────────────────────────

#[test]
fn e_o10_a_redirect_is_never_followed() {
    let second = Fake::healthy();
    let location = format!("http://{}/api/chat", second.addr);
    let fake = Fake::start(on(
        "/api/chat",
        Reply::Raw(format!(
            "HTTP/1.1 307 X\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        )),
    ));
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::Unavailable));
    assert_eq!(
        second.connections(),
        0,
        "the redirect target was not contacted"
    );
}

const PROXY_CHILD: &str = "NEXUS_M2E_A1_PROXY_CHILD";
const PROXY_KEYS: [&str; 6] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
];

#[test]
fn e_o10_proxy_variables_are_never_used() {
    let fake = Fake::healthy();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let proxied = Arc::new(AtomicUsize::new(0));
    {
        let proxied = proxied.clone();
        std::thread::spawn(move || {
            for stream in proxy.incoming() {
                proxied.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "inference::tests::e_o10_proxy_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PROXY_CHILD, fake.addr.to_string())
        .env_remove("NO_PROXY")
        .env_remove("no_proxy");
    for key in PROXY_KEYS {
        child.env(key, &proxy_url);
    }
    let output = child.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("M2E-A1 proxy child: ok"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        proxied.load(Ordering::SeqCst),
        0,
        "the proxy was not contacted"
    );
    assert_eq!(
        fake.count("/api/chat"),
        1,
        "the runtime was reached directly"
    );
}

/// The child half of [`e_o10_proxy_variables_are_never_used`]: runs only in
/// the child process it starts, with the proxy variables set.
#[test]
fn e_o10_proxy_child() {
    let Some(addr) = std::env::var_os(PROXY_CHILD) else {
        return;
    };
    let endpoint = LoopbackEndpoint::parse(&format!("http://{}", addr.to_string_lossy())).unwrap();
    let ctx = Ctx::new();
    let transport =
        VerifiedOllama::verify(endpoint, &spec(), &ctx.cancel, &ctx.deadline, &ctx.watch).unwrap();
    transport.call(&small(), &ctx.call_ctx(&ctx.watch)).unwrap();
    println!("M2E-A1 proxy child: ok");
}

// ── E-O11: no content in Debug or Display ───────────────────────────────────

#[test]
fn e_o11_debug_and_display_carry_no_content() {
    const CANARY: &str = "M2E-A1-CANARY-c0ffee";
    let request = ModelRequest::new(
        format!("system {CANARY}"),
        format!("user {CANARY}"),
        grammar::worker_schema(),
    )
    .unwrap();
    assert!(!format!("{request:?}").contains(CANARY), "{request:?}");

    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let mut value = chat_value();
    value["message"]["content"] = json!(format!("answer {CANARY}"));
    fake.set_route(chat(value.clone()));
    let reply = call(&transport, &ctx).result.unwrap();
    assert!(reply.content().contains(CANARY));
    assert!(!format!("{reply:?}").contains(CANARY), "{reply:?}");
    // A refused reply carries nothing of it.
    value["done_reason"] = json!("length");
    fake.set_route(chat(value));
    let error = call(&transport, &ctx).result.unwrap_err();
    assert!(!format!("{error} {error:?}").contains(CANARY));

    const ALL: [ModelError; 23] = [
        ModelError::NotLocal,
        ModelError::InvalidAddress,
        ModelError::InvalidModelName,
        ModelError::CloudName,
        ModelError::CloudEnabled,
        ModelError::NotInstalled,
        ModelError::DigestMismatch,
        ModelError::RuntimeMismatch,
        ModelError::Remote,
        ModelError::ConfigMismatch,
        ModelError::Unavailable,
        ModelError::Timeout,
        ModelError::RequestTooLarge,
        ModelError::ResponseTooLarge,
        ModelError::Malformed,
        ModelError::Truncated,
        ModelError::ToolCalls,
        ModelError::ThinkingLeaked,
        ModelError::EmptyContent,
        ModelError::OverBudget,
        ModelError::Stopped,
        ModelError::Cancelled,
        ModelError::Late,
    ];
    assert_eq!(ALL.map(ModelError::code), ModelError::CODES);
    let mut codes = ModelError::CODES.to_vec();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), ALL.len(), "the codes are distinct");
    for error in ALL {
        let text = format!("{error} {error:?}");
        assert!(
            !text.contains(CANARY) && !text.contains(NAME) && !text.contains("qwen"),
            "{text}"
        );
    }

    let answer = format!(
        r#"{{"action":"edit","edits":[{{"path":"{CANARY}","op":"create","content":"{CANARY}"}}]}}"#
    );
    let action = grammar::parse(&answer).unwrap();
    assert!(!format!("{action:?}").contains(CANARY), "{action:?}");
}

// ── E-O16: the complete request is bounded ──────────────────────────────────

#[test]
fn e_o16_a_request_over_the_byte_bound_is_refused_before_any_charge() {
    assert_eq!(MAX_RENDERED_CONTENT_BYTES, 3_936);
    let schema = grammar::worker_schema;
    assert_eq!(
        ModelRequest::new(String::new(), "x".repeat(20 * 1024), schema()).err(),
        Some(ModelError::RequestTooLarge),
        "a 20 KiB prompt"
    );
    let system = "s".repeat(2000);
    assert_eq!(
        ModelRequest::new(system.clone(), "u".repeat(1937), schema()).err(),
        Some(ModelError::RequestTooLarge),
        "3,937 bytes"
    );
    let fits = ModelRequest::new(system.clone(), "u".repeat(1936), schema()).unwrap();
    assert_eq!(fits.rendered_bytes(), 3_936);
    // Bytes, not characters.
    assert!(ModelRequest::new(String::new(), "語".repeat(1312), schema()).is_ok());
    assert_eq!(
        ModelRequest::new(String::new(), format!("{}x", "語".repeat(1312)), schema()).err(),
        Some(ModelError::RequestTooLarge)
    );

    // The call checks the bound again: no request, no charge.
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let (from, connections) = (fake.paths().len(), fake.connections());
    let oversize = ModelRequest::unchecked(system, "u".repeat(1937), schema());
    let outcome = done(&call_in_thread(&transport, &ctx, oversize));
    assert_eq!(outcome.code(), Err(ModelError::RequestTooLarge));
    assert_eq!(ctx.meter.events(), NOTHING);
    assert_eq!(
        (fake.paths().len(), fake.connections()),
        (from, connections)
    );
}

// ── E-O17 (with N2): the absolute deadline, on both clocks ──────────────────

#[test]
fn e_o17_one_deadline_serves_every_call_and_a_delay_never_extends_it() {
    let fake = Fake::healthy();
    let clocks = Clocks::default();
    let ctx = Ctx::with_deadline(clocks.clone(), 10_000, 10_000);
    let transport = verified(&fake, &ctx);
    assert_eq!(call(&transport, &ctx).code(), Ok(()));
    // A delay between calls, short of the deadline: the next call runs.
    clocks.advance_wall(9_999);
    clocks.advance_monotonic(9_999);
    assert_eq!(call(&transport, &ctx).code(), Ok(()));
    // The deadline is where it was fixed: the next call is refused at once.
    clocks.advance_monotonic(1);
    let (from, connections) = (fake.paths().len(), fake.connections());
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::Timeout));
    assert_eq!(
        (fake.paths().len(), fake.connections()),
        (from, connections)
    );
    assert_eq!(ctx.charges(), 2);
}

#[test]
fn e_o17_a_held_request_ends_when_either_clock_passes_the_deadline() {
    for cause in [Cause::Deadline, Cause::Suspend] {
        // Held metadata: nothing charged.
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        fake.set_route(on("/api/version", Reply::Hold));
        let rx = call_in_thread(&transport, &ctx, small());
        fake.wait_for(Event::Request("/api/version".into()));
        cause.trigger(&ctx);
        let outcome = within_bound(&rx, &format!("version held, {cause:?}"));
        assert_eq!(outcome.code(), Err(ModelError::Timeout), "{cause:?}");
        fake.wait_for(Event::Closed("/api/version".into()));
        assert_eq!(ctx.meter.events(), NOTHING, "{cause:?}");

        // A held chat: charged once, settled; its content never arrives.
        let fake = Fake::start(on("/api/chat", Reply::Hold));
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        let rx = call_in_thread(&transport, &ctx, small());
        fake.wait_for(Event::Request("/api/chat".into()));
        cause.trigger(&ctx);
        let outcome = within_bound(&rx, &format!("chat held, {cause:?}"));
        assert_eq!(outcome.code(), Err(ModelError::Timeout), "{cause:?}");
        assert_eq!(outcome.live, 0);
        fake.wait_for(Event::Closed("/api/chat".into()));
        assert_eq!(ctx.meter.events(), [Metered::Charge(1), Metered::Settle(1)]);
    }
}

#[test]
fn e_o17_n2_a_suspend_before_reservation_leaves_only_the_wall_time() {
    // The owner's anchors: consent at wall 0 and monotonic 0, ten minutes.
    const BUDGET: u64 = 600_000;
    let fake = Fake::start(on("/api/chat", Reply::Hold));
    let clocks = Clocks::default();
    // A suspend before the reservation: the wall clock ran on, the
    // monotonic clock did not.
    clocks.advance_wall(BUDGET - 50);
    // The reservation fixes the deadline from the anchors, never as
    // now + time left.
    let ctx = Ctx::with_deadline(clocks.clone(), BUDGET, BUDGET);
    let transport = verified(&fake, &ctx);
    let rx = call_in_thread(&transport, &ctx, small());
    fake.wait_for(Event::Request("/api/chat".into()));
    // The monotonic clock alone still has nearly all of it: no expiry.
    clocks.advance_monotonic(100);
    assert!(
        rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "still held"
    );
    // The wall clock reaches the deadline: the call ends, bounded.
    clocks.advance_wall(50);
    let outcome = within_bound(&rx, "suspend before reservation");
    assert_eq!(outcome.code(), Err(ModelError::Timeout));
    fake.wait_for(Event::Closed("/api/chat".into()));

    // A suspend that used the whole wall budget: refused before any request.
    let fake = Fake::healthy();
    let clocks = Clocks::default();
    let ctx = Ctx::with_deadline(clocks.clone(), BUDGET, BUDGET);
    let transport = verified(&fake, &ctx);
    clocks.advance_wall(BUDGET);
    let from = fake.paths().len();
    assert_eq!(call(&transport, &ctx).code(), Err(ModelError::Timeout));
    assert_eq!(fake.paths().len(), from);
    assert_eq!(ctx.meter.events(), NOTHING);
}

// ── E-O18 (transport half): a stopped watch never resumes ───────────────────

#[test]
fn e_o18_a_watch_admitted_before_a_stop_refuses_after_the_resume() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let number = ctx.controller.engage();
    assert!(ctx.controller.lift_if(number), "the owner's resume");
    let connections = fake.connections();
    let outcome = done(&call_with(
        &transport,
        &ctx,
        small(),
        ctx.watch.clone(),
        None,
    ));
    assert_eq!(outcome.code(), Err(ModelError::Stopped));
    assert_eq!(
        done(&verify_in_thread(&fake, &ctx, spec())).code(),
        Err(ModelError::Stopped)
    );
    assert_eq!(fake.connections(), connections, "no request after the stop");
    assert_eq!(ctx.meter.events(), NOTHING);
    // Work admitted after the resume runs.
    let fresh = ctx.stop.admit().unwrap();
    let outcome = done(&call_with(&transport, &ctx, small(), fresh, None));
    assert_eq!(outcome.code(), Ok(()));
}

// ── supporting checks (frozen encodings and texts) ──────────────────────────

#[test]
fn a1_identity_digests_are_pinned() {
    let identity = ModelIdentity::from_spec(&spec()).unwrap();
    assert_eq!(identity.provider(), "ollama");
    assert_eq!(
        hex::encode(identity.digest()),
        "4d7211f035868b327c0e10f2bc23155a7d492d7beecc5b505edfa373db82f9a4"
    );
    assert_eq!(
        hex::encode(identity.config().digest()),
        "895ef990b81986e1c3ec92c5cc60388e2b48ae4e5137a3a04d9337444606675c"
    );
    for change in [
        (|spec: &mut ModelPinSpec| spec.digest = OTHER_DIGEST.to_string()) as fn(&mut ModelPinSpec),
        |spec| spec.runtime_version = "0.40.14".to_string(),
        |spec| spec.config.seed = 8,
        |spec| spec.name = "qwen3.8:27b".to_string(),
    ] {
        let mut other = spec();
        change(&mut other);
        let other = ModelIdentity::from_spec(&other).unwrap();
        assert_ne!(other.digest(), identity.digest());
    }
}

#[test]
fn a1_the_grammar_is_the_qualified_one() {
    let system = grammar::WORKER_SYSTEM_INSTRUCTION;
    assert_eq!(system.len(), 1024);
    assert_eq!(
        sha256_hex(system.as_bytes()),
        "e68e0a33bf0f37a6776df359e0eb3c1de8dcbde2f1f658b8f8b96757a5995be0"
    );
    assert!(matches!(
        grammar::parse(" {\"action\":\"read\",\"paths\":[\"a\"]}\n"),
        Ok(EditAction::Read { paths }) if paths == ["a"]
    ));
    assert!(matches!(
        grammar::parse(r#"{"action":"edit","edits":[{"path":"index.html","op":"create","content":"x"}]}"#),
        Ok(EditAction::Edit { edits }) if edits.len() == 1 && edits[0].path == "index.html"
    ));
    assert!(matches!(
        grammar::parse(FINISH),
        Ok(EditAction::Finish { summary }) if summary == "done"
    ));
    for refused in [
        r#"{"action":"delete","path":"index.html"}"#,
        r#"{"action":"finish","summary":"x","force":true}"#,
        r#"{"action":"edit","edits":[{"path":"a","op":"create"}]}"#,
        r#"{"action":"edit","edits":[{"path":"a","op":"create","content":"x","mode":"w"}]}"#,
        r#"{"action":"finish","summary":"x"}{"action":"finish","summary":"y"}"#,
        "Here is the page.",
        "",
    ] {
        assert_eq!(
            grammar::parse(refused).err(),
            Some(grammar::GrammarError),
            "{refused}"
        );
    }
}

// ── oracle-add after Codex review 1 of PR #60 ───────────────────────────────

/// The provisional scaffold floor (design §10.4 `SCAFFOLD_FLOOR`; the
/// Architect's provisional value until Q-R3 records the qualified one).
const PROVISIONAL_FLOOR: u64 = 256;

/// E-O8b: a reply whose prompt count is below the scaffold floor, 0
/// included, cannot have processed the fixed qualified texts: refused,
/// charged once and settled, never retried. At or above the floor a valid
/// reply is accepted.
#[test]
fn e_o8b_a_prompt_count_below_the_scaffold_floor_is_refused() {
    let fake = Fake::healthy();
    let ctx = Ctx::new();
    let transport = verified(&fake, &ctx);
    let reply = |count: u64| {
        let mut value = chat_value();
        value["prompt_eval_count"] = json!(count);
        fake.set_route(chat(value));
        call(&transport, &ctx)
    };
    for (what, count) in [
        ("zero", 0),
        ("one", 1),
        ("one below the floor", PROVISIONAL_FLOOR - 1),
    ] {
        assert_eq!(reply(count).code(), Err(ModelError::Truncated), "{what}");
    }
    for (what, count) in [
        ("at the floor", PROVISIONAL_FLOOR),
        ("the qualification's 587", 587),
    ] {
        let content = reply(count).result.map(ModelReply::into_content);
        assert_eq!(content, Ok(FINISH.to_string()), "{what}");
    }
    assert_eq!(fake.count("/api/chat"), 5, "one chat per call");
    assert_eq!(ctx.charges(), 5);
    assert_eq!(
        ctx.meter
            .events()
            .iter()
            .filter(|e| matches!(e, Metered::Settle(_)))
            .count(),
        5
    );
}

/// E-O1b: a port is still required, and an explicitly written default port
/// (`:80`) is a written port.
#[test]
fn e_o1b_an_explicitly_written_default_port_is_accepted() {
    for (raw, addr) in [
        ("http://127.0.0.1:80", "127.0.0.1:80"),
        ("http://127.0.0.1:80/", "127.0.0.1:80"),
        ("http://localhost:80", "127.0.0.1:80"),
        ("http://[::1]:80", "[::1]:80"),
    ] {
        assert_eq!(
            LoopbackEndpoint::parse(raw).map(|endpoint| endpoint.addr()),
            Ok(addr.parse::<SocketAddr>().unwrap()),
            "{raw}"
        );
    }
    for raw in [
        "http://127.0.0.1",
        "http://127.0.0.1/",
        "http://127.0.0.1:",
        "http://[::1]",
        "http://localhost",
        "http://@127.0.0.1:80",
    ] {
        assert_eq!(
            LoopbackEndpoint::parse(raw).err(),
            Some(ModelError::InvalidAddress),
            "{raw}"
        );
    }
}

// ── M2e-A1b oracle-add: the dispatch observer (M3 design §8.5; M3-O44 and
// M3-O45 at the transport) ──────────────────────────────────────────────────

/// The observer reads the host's CLOCK_MONOTONIC through nix, which the
/// kernel has on Linux and macOS only; elsewhere no dispatch is ever
/// notified (a send time is never estimated).
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod dispatch {
    use super::*;

    /// The host's CLOCK_MONOTONIC in ms, read by the test itself.
    fn host_monotonic_ms() -> u64 {
        let now = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC).unwrap();
        u64::try_from(now.tv_sec()).unwrap() * 1000
            + u64::try_from(now.tv_nsec()).unwrap() / 1_000_000
    }

    /// Records each notice with the fake runtime's requests at that moment.
    struct Recorder {
        log: Log,
        notices: Mutex<Vec<(u64, Vec<String>)>>,
    }

    impl Recorder {
        fn new(fake: &Fake) -> Arc<Self> {
            Arc::new(Self {
                log: fake.log.clone(),
                notices: Mutex::default(),
            })
        }

        fn notices(&self) -> Vec<(u64, Vec<String>)> {
            self.notices.lock().unwrap().clone()
        }
    }

    impl CallObserver for Recorder {
        fn dispatched(&self, sent_mono_ms: u64) {
            let seen = self
                .log
                .lock()
                .unwrap()
                .iter()
                .map(|(p, _)| p.clone())
                .collect();
            self.notices.lock().unwrap().push((sent_mono_ms, seen));
        }
    }

    /// A meter that runs `on_charge` right after each granted charge.
    struct HookMeter {
        ctx: Arc<Ctx>,
        on_charge: Cause,
    }

    impl CallMeter for HookMeter {
        fn charge(&self) -> Result<Charged, MeterRefusal> {
            let charged = self.ctx.meter.charge();
            self.on_charge.trigger(&self.ctx);
            charged
        }

        fn settle(&self, call: u32) {
            self.ctx.meter.settle(call);
        }
    }

    /// A call with `recorder` as its observer (and `meter`, if any, in
    /// place of the caller's own).
    fn observed(
        transport: &Arc<VerifiedOllama>,
        ctx: &Arc<Ctx>,
        request: ModelRequest,
        recorder: &Arc<Recorder>,
        meter: Option<HookMeter>,
    ) -> Receiver<Outcome<ModelReply>> {
        let (transport, ctx, recorder) = (transport.clone(), ctx.clone(), recorder.clone());
        in_thread(
            move || {
                let meter: &dyn CallMeter = match &meter {
                    Some(meter) => meter,
                    None => &ctx.meter,
                };
                let call = CallContext {
                    meter,
                    cancel: &ctx.cancel,
                    deadline: &ctx.deadline,
                    stop: &ctx.watch,
                    observer: Some(recorder.as_ref()),
                };
                transport.call(&request, &call)
            },
            None,
        )
    }

    /// M3-O44 (transport): one notice, before the runtime has received the
    /// chat request, valued on the host's CLOCK_MONOTONIC.
    #[test]
    fn m2e_a1b_dispatch_is_notified_once_before_the_chat_reaches_the_runtime() {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        let from = fake.paths().len();
        let recorder = Recorder::new(&fake);
        let before = host_monotonic_ms();
        let outcome = done(&observed(&transport, &ctx, small(), &recorder, None));
        let after = host_monotonic_ms();
        assert_eq!(outcome.code(), Ok(()));
        let notices = recorder.notices();
        assert_eq!(notices.len(), 1, "exactly one notice: {notices:?}");
        let (sent, seen) = &notices[0];
        assert_eq!(
            seen[from..],
            ["/api/version", "/api/status", "/api/tags"],
            "notified after the metadata and before the runtime received the chat"
        );
        assert!(
            before <= *sent && *sent <= after,
            "host CLOCK_MONOTONIC: {before} <= {sent} <= {after}"
        );
        assert_eq!(
            fake.paths()[from..],
            ["/api/version", "/api/status", "/api/tags", "/api/chat"]
        );
    }

    /// M3-O44 (transport): a sent chat is notified once, before the runtime
    /// sees it, however the call then ends.
    #[test]
    fn m2e_a1b_a_sent_chat_is_notified_once_however_it_ends() {
        for cause in [Cause::Stop, Cause::Cancel, Cause::Deadline] {
            let fake = Fake::start(on("/api/chat", Reply::Hold));
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            let recorder = Recorder::new(&fake);
            let rx = observed(&transport, &ctx, small(), &recorder, None);
            fake.wait_for(Event::Request("/api/chat".into()));
            assert_eq!(
                recorder.notices().len(),
                1,
                "{cause:?}: notified before the hold"
            );
            cause.trigger(&ctx);
            let outcome = within_bound(&rx, &format!("observed held chat, {cause:?}"));
            assert_eq!(outcome.code(), Err(cause.code()), "{cause:?}");
            assert_eq!(recorder.notices().len(), 1, "{cause:?}: once");
        }
        for (what, reply) in [
            ("status 500", Reply::Json(500, r#"{"error":"x"}"#.into())),
            ("closed without a reply", Reply::Close),
        ] {
            let fake = Fake::start(on("/api/chat", reply));
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            let recorder = Recorder::new(&fake);
            let outcome = done(&observed(&transport, &ctx, small(), &recorder, None));
            assert_eq!(outcome.code(), Err(ModelError::Unavailable), "{what}");
            assert_eq!(recorder.notices().len(), 1, "{what}: once");
        }
    }

    /// M3-O45 (transport): no notice for a metadata request, nor for a call
    /// refused before the chat is sent, including a Stop, cancel or deadline
    /// after the charge. (Registration has no observer at all.)
    #[test]
    fn m2e_a1b_no_notice_for_metadata_or_a_call_refused_before_the_send() {
        // Held metadata, ended.
        for (path, cause) in [
            ("/api/version", Cause::Cancel),
            ("/api/status", Cause::Deadline),
            ("/api/tags", Cause::Stop),
        ] {
            let fake = Fake::healthy();
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            fake.set_route(on(path, Reply::Hold));
            let recorder = Recorder::new(&fake);
            let rx = observed(&transport, &ctx, small(), &recorder, None);
            fake.wait_for(Event::Request(path.into()));
            cause.trigger(&ctx);
            let outcome = within_bound(&rx, &format!("observed {path} held, {cause:?}"));
            assert_eq!(outcome.code(), Err(cause.code()), "{path}");
            assert_eq!(recorder.notices(), [], "{path} {cause:?}: no notice");
        }
        // Refused by the identity checks, the byte bound or the meter.
        for (what, route, code) in [
            (
                "another runtime version",
                on(
                    "/api/version",
                    Reply::Json(200, r#"{"version":"0.41.0"}"#.into()),
                ),
                ModelError::RuntimeMismatch,
            ),
            (
                "cloud enabled",
                on(
                    "/api/status",
                    Reply::Json(200, r#"{"cloud":{"disabled":false}}"#.into()),
                ),
                ModelError::CloudEnabled,
            ),
            (
                "another digest",
                on("/api/tags", Reply::Json(200, tags(OTHER_DIGEST))),
                ModelError::DigestMismatch,
            ),
        ] {
            let fake = Fake::healthy();
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            fake.set_route(route);
            let recorder = Recorder::new(&fake);
            let outcome = done(&observed(&transport, &ctx, small(), &recorder, None));
            assert_eq!(outcome.code(), Err(code), "{what}");
            assert_eq!(recorder.notices(), [], "{what}: no notice");
        }
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        let recorder = Recorder::new(&fake);
        let oversize =
            ModelRequest::unchecked("s".repeat(2000), "u".repeat(1937), grammar::worker_schema());
        let outcome = done(&observed(&transport, &ctx, oversize, &recorder, None));
        assert_eq!(outcome.code(), Err(ModelError::RequestTooLarge));
        ctx.meter.refuse.store(true, Ordering::SeqCst);
        let outcome = done(&observed(&transport, &ctx, small(), &recorder, None));
        assert_eq!(outcome.code(), Err(ModelError::OverBudget));
        assert_eq!(recorder.notices(), [], "byte bound, meter: no notice");
        // Ended after the charge, before the send.
        for cause in [Cause::Stop, Cause::Cancel, Cause::Deadline] {
            let fake = Fake::healthy();
            let ctx = Ctx::new();
            let transport = verified(&fake, &ctx);
            let recorder = Recorder::new(&fake);
            let meter = HookMeter {
                ctx: ctx.clone(),
                on_charge: cause,
            };
            let outcome = done(&observed(&transport, &ctx, small(), &recorder, Some(meter)));
            assert_eq!(
                outcome.code(),
                Err(cause.code()),
                "{cause:?} after the charge"
            );
            assert_eq!(
                recorder.notices(),
                [],
                "{cause:?} after the charge: no notice"
            );
            assert_eq!(fake.count("/api/chat"), 0, "{cause:?}: nothing sent");
            assert_eq!(ctx.meter.events(), [Metered::Charge(1), Metered::Settle(1)]);
        }
    }
}

// ── M2e-A1b oracle-add after Codex review 1 of PR #69: a panicking observer
// cannot prevent (or fake) the send ──────────────────────────────────────────

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod dispatch_panic {
    use super::*;

    /// Records the notice, then panics.
    #[derive(Default)]
    struct RecordThenPanic {
        notices: Mutex<Vec<u64>>,
    }

    impl CallObserver for RecordThenPanic {
        fn dispatched(&self, sent_mono_ms: u64) {
            self.notices.lock().unwrap().push(sent_mono_ms);
            panic!("M2e-A1b planted observer panic (expected in this test)");
        }
    }

    /// A notice is a claim that the send was attempted: an observer that
    /// records it and then panics must not stop the request from being
    /// handed to the client. The runtime receives exactly one chat and the
    /// call ends with the runtime's own result.
    #[test]
    fn m2e_a1b_an_observer_that_panics_after_recording_cannot_prevent_the_send() {
        let fake = Fake::healthy();
        let ctx = Ctx::new();
        let transport = verified(&fake, &ctx);
        let observer = Arc::new(RecordThenPanic::default());
        let rx = {
            let (transport, ctx, observer) = (transport.clone(), ctx.clone(), observer.clone());
            in_thread(
                move || {
                    let call = CallContext {
                        meter: &ctx.meter,
                        cancel: &ctx.cancel,
                        deadline: &ctx.deadline,
                        stop: &ctx.watch,
                        observer: Some(observer.as_ref()),
                    };
                    transport.call(&small(), &call)
                },
                None,
            )
        };
        let outcome = done(&rx);
        assert_eq!(observer.notices.lock().unwrap().len(), 1, "one notice");
        assert_eq!(fake.count("/api/chat"), 1, "the notified chat was sent");
        let content = outcome.result.map(ModelReply::into_content);
        assert_eq!(content, Ok(FINISH.to_string()), "the runtime's own result");
        assert_eq!(outcome.live, 0, "the exchange thread was joined");
        assert_eq!(ctx.meter.events(), [Metered::Charge(1), Metered::Settle(1)]);
    }
}
