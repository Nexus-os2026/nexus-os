//! M2e-C1b test fixture: a fake Ollama on 127.0.0.1, composed with the real
//! ordinary broker (the broker's own fake is private to its test module).
//! It answers as the healthy pinned runtime, except for the paths a test
//! routes, and counts the paths it was asked.
use nexus_governed_control::inference::{ModelPinSpec, RuntimeConfig, Think};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub(crate) const NAME: &str = "qwen3.6:35b-a3b";
pub(crate) const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";
const VERSION: &str = "0.40.13";

pub(crate) fn spec() -> ModelPinSpec {
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

pub(crate) enum Answer {
    Json(u16, String),
}

type Route = Arc<dyn Fn(&str) -> Option<Answer> + Send + Sync>;

pub(crate) struct FakeOllama {
    addr: SocketAddr,
    log: Arc<Mutex<Vec<String>>>,
    route: Arc<Mutex<Route>>,
}

/// A successful chat envelope whose content is `content`.
pub(crate) fn chat(content: &str) -> Answer {
    Answer::Json(
        200,
        serde_json::json!({
            "model": NAME,
            "message": {"role": "assistant", "content": content},
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 587,
            "eval_count": 378,
        })
        .to_string(),
    )
}

fn healthy(path: &str) -> Answer {
    let json = |body: serde_json::Value| Answer::Json(200, body.to_string());
    match path {
        "/api/version" => json(serde_json::json!({ "version": VERSION })),
        "/api/status" => json(serde_json::json!({"cloud": {"disabled": true, "source": "env"}})),
        "/api/tags" => {
            json(serde_json::json!({"models": [{"name": NAME, "model": NAME, "digest": DIGEST}]}))
        }
        "/api/show" => json(serde_json::json!({"capabilities": ["completion", "thinking"]})),
        "/api/chat" => chat("{\"ok\":true}"),
        _ => Answer::Json(404, "{}".into()),
    }
}

impl FakeOllama {
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let route: Arc<Mutex<Route>> = Arc::new(Mutex::new(Arc::new(|_: &str| None)));
        let (l, r) = (log.clone(), route.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let (l, r) = (l.clone(), r.clone());
                std::thread::spawn(move || serve(stream, &l, &r));
            }
        });
        Self { addr, log, route }
    }

    pub(crate) fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Answer the paths `over` answers; every other path as healthy.
    pub(crate) fn route(&self, over: impl Fn(&str) -> Option<Answer> + Send + Sync + 'static) {
        *self.route.lock().unwrap() = Arc::new(over);
    }

    pub(crate) fn count(&self, path: &str) -> usize {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == path)
            .count()
    }
}

fn serve(mut stream: TcpStream, log: &Mutex<Vec<String>>, route: &Mutex<Route>) {
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
    let over = route.lock().unwrap().clone();
    let Answer::Json(status, body) = over(&path).unwrap_or_else(|| healthy(&path));
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
}
