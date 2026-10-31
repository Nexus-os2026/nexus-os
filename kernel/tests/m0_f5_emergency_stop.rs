//! M0-FIX F5: the owner's emergency stop ends a coding run's in-flight
//! local-model call (the connection is closed, not merely ignored), refuses
//! new calls while it is in force, and lets them run again after the
//! owner's resume. This binary engages the process's own stop authority,
//! so its tests are serialized and nothing else runs in this process.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use nexus_kernel::coding_run::{
    loopback_endpoint, LocalModel, LocalOllama, ModelError, ModelMessage, ModelRole,
};
use nexus_kernel::emergency_stop::StopAuthority;
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

const TAGS: &str = r#"{"models":[{"name":"f5-model"}]}"#;

fn finish_reply() -> String {
    serde_json::json!({"message": {"content": "{\"action\":\"finish\",\"summary\":\"x\"}"}})
        .to_string()
}

fn messages() -> Vec<ModelMessage> {
    vec![ModelMessage {
        role: ModelRole::User,
        content: "task".to_string(),
    }]
}

#[test]
fn m0_f5_stop_ends_an_inflight_local_model_call() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let stop = StopAuthority::global();
    stop.lift();
    let server = Server::start(Arc::new(|path: &str| match path {
        "/api/tags" => Some((200, TAGS.to_string())),
        _ => None,
    }));
    let endpoint = loopback_endpoint(&server.base).unwrap();
    let model = LocalOllama::select(&endpoint, "f5-model").unwrap();
    assert_eq!(server.next(10), Event::Request("/api/tags".into()));

    let call = {
        let model = model.clone();
        std::thread::spawn(move || model.complete(&messages(), Duration::from_secs(120), 4096))
    };
    // Readiness: the request reached the model server and is held there.
    assert_eq!(server.next(30), Event::Request("/api/chat".into()));
    stop.engage();
    // The call ends promptly with the stop, and the connection is closed.
    assert_eq!(server.next(10), Event::Closed("/api/chat".into()));
    assert_eq!(call.join().unwrap(), Err(ModelError::Stopped));

    // While the stop is in force no new call is made.
    let before = server.connections();
    assert_eq!(
        model.complete(&messages(), Duration::from_secs(5), 4096),
        Err(ModelError::Stopped)
    );
    assert_eq!(
        server.connections(),
        before,
        "a call was made while stopped"
    );

    // After the owner's resume, calls run again.
    stop.lift();
    server.set_reply(Arc::new(|path: &str| match path {
        "/api/tags" => Some((200, TAGS.to_string())),
        _ => Some((200, finish_reply())),
    }));
    let answer = model
        .complete(&messages(), Duration::from_secs(30), 4096)
        .unwrap();
    assert!(answer.contains("finish"), "{answer}");
}
