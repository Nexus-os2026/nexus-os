//! M0-FIX F5: the owner's emergency stop ends an in-flight provider model
//! call: the provider's own curl child is terminated and reaped (the
//! connection is closed and no child process remains), new calls are
//! refused while the stop is in force, and they run again after the
//! owner's resume. This binary engages the process's own stop authority,
//! so its tests are serialized and nothing else runs in this process.
#![cfg(target_os = "linux")]

use nexus_connectors_llm::providers::ollama::OllamaProvider;
use nexus_connectors_llm::providers::LlmProvider;
use nexus_kernel::emergency_stop::{HaltArea, HaltOutcome, StopAuthority};
use std::sync::{Arc, Mutex};

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

/// Child processes of this process (all threads), from /proc.
fn children() -> Vec<String> {
    let mut out = Vec::new();
    for task in std::fs::read_dir("/proc/self/task").unwrap().flatten() {
        if let Ok(text) = std::fs::read_to_string(task.path().join("children")) {
            out.extend(text.split_whitespace().map(str::to_string));
        }
    }
    out
}

fn model_call_halts(stop: &StopAuthority) -> usize {
    stop.halts()
        .iter()
        .filter(|h| h.area == HaltArea::ModelCall)
        .count()
}

fn stopped_in_flight(
    path: &'static str,
    call: impl FnOnce(OllamaProvider) -> Result<String, String> + Send + 'static,
) {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let stop = StopAuthority::global();
    stop.lift();
    let server = Server::start(Arc::new(|_: &str| None));
    let provider = OllamaProvider::local_only(&server.base)
        .unwrap()
        .with_request_timeout(120)
        .with_streaming_timeout(120);
    let halts_before = model_call_halts(&stop);
    let worker = {
        let provider = provider.clone();
        std::thread::spawn(move || call(provider))
    };
    // Readiness: the provider's curl child reached the server and is held.
    assert_eq!(server.next(30), Event::Request(path.into()));
    assert!(!children().is_empty(), "no provider child is running");
    stop.engage();
    // The child is terminated: its connection closes and the call ends.
    assert_eq!(server.next(10), Event::Closed(path.into()));
    let result = worker.join().unwrap();
    let error = result.unwrap_err();
    assert!(error.contains("emergency stop"), "{error}");
    assert!(
        children().is_empty(),
        "a provider child survived the stop: {:?}",
        children()
    );
    let halts = stop.halts();
    assert_eq!(model_call_halts(&stop), halts_before + 1);
    assert_eq!(halts.last().unwrap().outcome, HaltOutcome::Ended);

    // While the stop is in force no model call starts.
    let refused = provider.query("again", 8, "m").unwrap_err().to_string();
    assert!(refused.contains("emergency stop"), "{refused}");
    assert!(
        server.events.try_recv().is_err(),
        "a request was sent while stopped"
    );
    assert_eq!(model_call_halts(&stop), halts_before + 1);

    // After the owner's resume, calls run again.
    stop.lift();
    server.set_reply(Arc::new(|_: &str| {
        Some((200, r#"{"response":"resumed","done":true}"#.to_string()))
    }));
    let answer = provider.query("after", 8, "m").unwrap();
    assert!(answer.output_text.contains("resumed"), "{answer:?}");
}

#[test]
fn m0_f5_stop_ends_an_inflight_query() {
    stopped_in_flight("/api/generate", |provider| {
        provider
            .query("hello", 8, "m")
            .map(|r| r.output_text)
            .map_err(|e| e.to_string())
    });
}

#[test]
fn m0_f5_stop_ends_an_inflight_chat_stream() {
    stopped_in_flight("/v1/chat/completions", |provider| {
        provider
            .chat_stream(
                &[serde_json::json!({"role": "user", "content": "hi"})],
                "m",
                |_| {},
            )
            .map_err(|e| e.to_string())
    });
}
