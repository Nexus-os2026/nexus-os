//! Shared test fixtures: a scripted confirmer, an authority with in-memory
//! evidence, and a tiny HTTP server on the loopback interface that records
//! what it receives.

use crate::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use crate::authority::clock::SystemClock;
use crate::authority::evidence::MemoryEvidence;
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::RunOrigin;
use crate::authority::Authority;
use crate::control::Control;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Answers every native confirmation with `self.0`, counting the asks.
pub struct Yes(pub bool, pub AtomicU32);

impl Yes {
    pub fn new(answer: bool) -> Self {
        Self(answer, AtomicU32::new(0))
    }

    pub fn asked(&self) -> u32 {
        self.1.load(Ordering::SeqCst)
    }
}

impl ControlConfirmer for Yes {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0
    }
}

/// A pipeline with in-memory evidence and the system clock, one agent and
/// one open run.
pub struct Harness {
    pub control: Arc<Control>,
    pub evidence: Arc<MemoryEvidence>,
    pub agent: AgentId,
    pub run: RunId,
}

pub fn harness() -> Harness {
    let evidence = Arc::new(MemoryEvidence::new(10_000));
    let control = Arc::new(Control::new(Authority::new(
        evidence.clone(),
        Arc::new(SystemClock::default()),
    )));
    let agent = AgentId::new("agent-test").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    Harness {
        control,
        evidence,
        agent,
        run,
    }
}

/// One request the test server received.
#[derive(Clone, Debug)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Received {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A canned response.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub delay: Duration,
}

impl Reply {
    pub fn ok(body: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".into(), "text/plain".into())],
            body: body.as_bytes().to_vec(),
            delay: Duration::ZERO,
        }
    }

    pub fn redirect(status: u16, location: &str) -> Self {
        Self {
            status,
            headers: vec![("Location".into(), location.into())],
            body: Vec::new(),
            delay: Duration::ZERO,
        }
    }
}

type Handler = dyn Fn(&Received) -> Reply + Send + Sync;

/// A loopback HTTP/1.1 server answering with `handler`, recording requests.
pub struct TestServer {
    pub address: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
    stop: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start(handler: impl Fn(&Received) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        {
            let received = received.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let received = received.clone();
                            let handler = handler.clone();
                            std::thread::spawn(move || serve(stream, &received, &*handler));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(5)),
                    }
                }
            });
        }
        Self {
            address,
            received,
            stop,
        }
    }

    pub fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn serve(stream: TcpStream, received: &Mutex<Vec<Received>>, handler: &Handler) {
    stream.set_nonblocking(false).ok();
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let request = Received {
        method,
        path,
        headers,
        body,
    };
    received.lock().unwrap().push(request.clone());
    let reply = handler(&request);
    std::thread::sleep(reply.delay);
    let mut out = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (name, value) in &reply.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let mut stream = stream;
    let _ = stream.write_all(out.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
}
