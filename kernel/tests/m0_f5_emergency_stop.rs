//! M0-FIX F5: the owner's emergency stop ends a local-model call in flight
//! (the connection is closed, not merely ignored), and refuses new calls
//! while it is in force. After the owner's resume, work admitted before the
//! stop never resumes; only work admitted afterwards runs. This binary
//! engages the process's own stop authority, so its tests are serialized
//! and nothing else runs in this process.
//!
//! M2e-A2 (Architect decision A2-D2): the call is made through the kernel
//! transport (A1's `VerifiedOllama` and the qualified two-message
//! `ModelRequest`) under a watch from the process's authority, since the
//! coding run's `LocalOllama::bind` stays shut until Q-R-N1 (E-O65) and an
//! integration test cannot reach the test-only seam. The same schedule
//! through `LocalOllama::bind`, with a private authority, is E-O18 in
//! `kernel/src/coding_run/m2e_a2_tests.rs`.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use nexus_kernel::emergency_stop::{StopAuthority, StopWatch};
use nexus_kernel::inference::{
    grammar, AbsoluteDeadline, CallCancel, CallContext, CallMeter, CancelCause, Charged,
    LoopbackEndpoint, MeterRefusal, ModelError, ModelPinSpec, ModelRequest, ModelTransport,
    RuntimeConfig, SystemClocks, Think, VerifiedOllama,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SERIAL: Mutex<()> = Mutex::new(());

/// A hermetic loopback HTTP server for the stop tests: each request is
/// answered by `reply(path)`, or held open (`None`) until the client closes
/// the connection. Every request and every close of a held request is
/// reported, so tests synchronize on readiness, never on sleeps.
#[allow(dead_code)]
mod hermetic {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Event {
        Request(String),
        Closed(String),
    }

    pub type Reply = Arc<dyn Fn(&str) -> Option<(u16, String)> + Send + Sync>;

    pub struct Server {
        pub base: String,
        pub events: Receiver<Event>,
        pub connections: Arc<AtomicUsize>,
        reply: Arc<Mutex<Reply>>,
    }

    impl Server {
        pub fn start(reply: Reply) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let (tx, events) = mpsc::channel();
            let connections = Arc::new(AtomicUsize::new(0));
            let reply = Arc::new(Mutex::new(reply));
            {
                let connections = connections.clone();
                let reply = reply.clone();
                std::thread::spawn(move || {
                    for stream in listener.incoming() {
                        let Ok(stream) = stream else { return };
                        connections.fetch_add(1, Ordering::SeqCst);
                        let tx = tx.clone();
                        let reply = reply.lock().unwrap().clone();
                        std::thread::spawn(move || serve(stream, tx, reply));
                    }
                });
            }
            Self {
                base,
                events,
                connections,
                reply,
            }
        }

        pub fn set_reply(&self, reply: Reply) {
            *self.reply.lock().unwrap() = reply;
        }

        pub fn connections(&self) -> usize {
            self.connections.load(Ordering::SeqCst)
        }

        /// The next event, within `secs`.
        #[track_caller]
        pub fn next(&self, secs: u64) -> Event {
            self.events
                .recv_timeout(std::time::Duration::from_secs(secs))
                .expect("server event")
        }
    }

    fn serve(mut stream: TcpStream, tx: Sender<Event>, reply: Reply) {
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
        let _ = tx.send(Event::Request(path.clone()));
        match reply(&path) {
            Some((status, body)) => {
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
            None => {
                // Held open: report when the client closes it.
                let mut sink = [0u8; 1024];
                loop {
                    match stream.read(&mut sink) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                let _ = tx.send(Event::Closed(path));
            }
        }
    }
}

use hermetic::{Event, Server};

const NAME: &str = "qwen3.6:35b-a3b";
const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";

/// The runtime's metadata replies (the pinned identity); `chat` answers
/// `/api/chat` (`None`: held open).
fn route(chat: Option<String>) -> hermetic::Reply {
    Arc::new(move |path: &str| match path {
        "/api/version" => Some((200, r#"{"version":"0.40.13"}"#.to_string())),
        "/api/status" => Some((
            200,
            r#"{"cloud":{"disabled":true,"source":"env"}}"#.to_string(),
        )),
        "/api/tags" => Some((
            200,
            format!(r#"{{"models":[{{"name":"{NAME}","digest":"{DIGEST}"}}]}}"#),
        )),
        "/api/show" => Some((
            200,
            r#"{"details":{},"capabilities":["completion","thinking"]}"#.to_string(),
        )),
        _ => chat.clone().map(|body| (200, body)),
    })
}

fn finish_reply() -> String {
    serde_json::json!({
        "model": NAME,
        "message": {"role": "assistant", "content": "{\"action\":\"finish\",\"summary\":\"x\"}"},
        "done": true,
        "done_reason": "stop",
        "prompt_eval_count": 587,
        "eval_count": 20,
    })
    .to_string()
}

fn spec() -> ModelPinSpec {
    ModelPinSpec {
        name: NAME.to_string(),
        digest: DIGEST.to_string(),
        runtime_version: "0.40.13".to_string(),
        config: RuntimeConfig {
            num_ctx: 8192,
            temperature_milli: 0,
            seed: 7,
            think: Think::Off,
            truncate: false,
        },
    }
}

/// The qualified two-message request.
fn request() -> ModelRequest {
    ModelRequest::new(
        grammar::WORKER_SYSTEM_INSTRUCTION.to_string(),
        "Owner task:\ntask\n".to_string(),
        grammar::worker_schema(),
    )
    .unwrap()
}

#[derive(Default)]
struct Meter(Mutex<u32>);

impl Meter {
    fn charges(&self) -> u32 {
        *self.0.lock().unwrap()
    }
}

impl CallMeter for Meter {
    fn charge(&self) -> Result<Charged, MeterRefusal> {
        let mut calls = self.0.lock().unwrap();
        *calls += 1;
        Ok(Charged { call: *calls })
    }

    fn settle(&self, _call: u32) {}
}

struct Never;

impl CallCancel for Never {
    fn cancelled(&self) -> Option<CancelCause> {
        None
    }
}

/// One call through the transport, charged to `meter`, under `watch` (the
/// handle the work was admitted with).
fn call(
    transport: &VerifiedOllama,
    meter: &Meter,
    watch: &StopWatch,
    secs: u64,
) -> Result<String, ModelError> {
    let deadline = AbsoluteDeadline::within(SystemClocks, Duration::from_secs(secs));
    let ctx = CallContext {
        meter,
        cancel: &Never,
        deadline: &deadline,
        stop: watch,
        observer: None,
    };
    transport
        .call(&request(), &ctx)
        .map(|reply| reply.into_content())
}

#[test]
fn m0_f5_stop_ends_an_inflight_local_model_call() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // This binary stands in for the owner's Stop/Resume path (M0-FIX2
    // F10): it holds the process's stop controller.
    let controller =
        StopAuthority::claim_global_controller().expect("the process's stop controller");
    let stop = controller.authority().clone();
    controller.lift_if(stop.stop_number());
    let server = Server::start(route(None));
    let handle = stop.admit().expect("work admitted before the stop");
    let deadline = AbsoluteDeadline::within(SystemClocks, Duration::from_secs(30));
    let transport = Arc::new(
        VerifiedOllama::verify(
            LoopbackEndpoint::parse(&server.base).unwrap(),
            &spec(),
            &Never,
            &deadline,
            &handle,
        )
        .unwrap(),
    );
    for path in ["/api/version", "/api/status", "/api/tags", "/api/show"] {
        assert_eq!(server.next(10), Event::Request(path.into()));
    }

    // 1. An in-flight call is ended by the stop and its connection closed.
    let meter = Arc::new(Meter::default());
    let inflight = {
        let (transport, meter, handle) = (transport.clone(), meter.clone(), handle.clone());
        std::thread::spawn(move || call(&transport, &meter, &handle, 120))
    };
    for path in ["/api/version", "/api/status", "/api/tags"] {
        assert_eq!(server.next(10), Event::Request(path.into()));
    }
    // Readiness: the chat reached the model server and is held there.
    assert_eq!(server.next(30), Event::Request("/api/chat".into()));
    stop.engage();
    // The call ends promptly with the stop, and the connection is closed.
    assert_eq!(server.next(10), Event::Closed("/api/chat".into()));
    assert_eq!(inflight.join().unwrap(), Err(ModelError::Stopped));
    assert_eq!(meter.charges(), 1);

    // 2. While the stop is in force no new work is admitted and nothing is
    // sent, not even a metadata request.
    let before = server.connections();
    assert!(stop.admit().is_err(), "new work admitted while stopped");
    assert_eq!(
        call(&transport, &meter, &handle, 5),
        Err(ModelError::Stopped)
    );
    assert_eq!(
        server.connections(),
        before,
        "a call was made while stopped"
    );

    // 3. After the owner's resume, the handle made before the stop still
    // refuses, with no connection and no charge.
    assert!(controller.lift_if(stop.stop_number()));
    server.set_reply(route(Some(finish_reply())));
    assert_eq!(
        call(&transport, &meter, &handle, 5),
        Err(ModelError::Stopped)
    );
    assert_eq!(
        server.connections(),
        before,
        "work admitted before the stop resumed"
    );
    assert_eq!(meter.charges(), 1, "a charge after the stop");

    // 4. A fresh handle, admitted after the resume, runs.
    let fresh = stop.admit().expect("new work after the resume");
    let answer = call(&transport, &meter, &fresh, 30).unwrap();
    assert!(answer.contains("finish"), "{answer}");
    assert_eq!(meter.charges(), 2);
}
