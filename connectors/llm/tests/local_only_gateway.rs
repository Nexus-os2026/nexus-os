//! P3 D-2: the local-only checks of the gateway. `verify_local_ollama_model`
//! is driven against a fake Ollama runtime on a loopback port; nothing else
//! is contacted.

use nexus_connectors_llm::gateway::{local_only_ollama_base, verify_local_ollama_model};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

/// Serves one `/api/show` reply per connection, `replies` in order, and
/// reports each request (`METHOD path body`).
fn fake_runtime(replies: Vec<(u16, &'static str)>) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (log, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let mut replies = replies.into_iter();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            let mut reader = BufReader::new(&stream);
            let (mut head, mut length) = (String::new(), 0usize);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                if head.is_empty() {
                    head = line.trim_end().to_string();
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let mut parts = head.split(' ');
            let _ = log.send(format!(
                "{} {} {}",
                parts.next().unwrap_or_default(),
                parts.next().unwrap_or_default(),
                String::from_utf8_lossy(&body)
            ));
            let Some((status, reply)) = replies.next() else {
                return;
            };
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
        }
    });
    (base, seen)
}

#[test]
fn p3_d2_a_model_the_local_runtime_runs_is_verified() {
    let (base, seen) = fake_runtime(vec![(200, r#"{"modelfile":"FROM x","details":{}}"#)]);
    verify_local_ollama_model(&base, "qwen3:8b").unwrap();
    let request = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(request, r#"POST /api/show {"model":"qwen3:8b"}"#);
}

#[test]
fn p3_d2_a_model_the_runtime_relays_or_cannot_describe_is_refused() {
    for (status, reply) in [
        (
            200,
            r#"{"remote_host":"https://ollama.com:443","remote_model":"gpt-oss:120b"}"#,
        ),
        (200, r#"{"remote_model":"qwen3:8b"}"#),
        (404, r#"{"error":"model 'qwen3:8b' not found"}"#),
        (500, r#"{"error":"failed"}"#),
        (200, r#"["not","a","description"]"#),
        (200, ""),
    ] {
        let (base, seen) = fake_runtime(vec![(status, reply)]);
        let error = verify_local_ollama_model(&base, "qwen3:8b").unwrap_err();
        assert!(
            error.to_string().contains("local-only"),
            "{status} {reply}: {error}"
        );
        assert!(seen.recv_timeout(Duration::from_secs(5)).is_ok());
    }
}

#[test]
fn p3_d2_a_cloud_model_or_remote_address_is_refused_without_a_request() {
    let (base, seen) = fake_runtime(vec![(200, r#"{"details":{}}"#)]);
    for model in ["gpt-oss:120b-cloud", "glm-4.6:cloud", "", "  "] {
        assert!(
            verify_local_ollama_model(&base, model).is_err(),
            "{model:?}"
        );
    }
    for remote in [
        "http://10.0.0.5:11434",
        "http://ollama.example.com:11434",
        "http://localhost.example.com:11434",
        "http://0.0.0.0:11434",
    ] {
        assert!(
            verify_local_ollama_model(remote, "qwen3:8b").is_err(),
            "{remote}"
        );
        assert!(local_only_ollama_base(remote).is_err(), "{remote}");
    }
    assert!(
        seen.recv_timeout(Duration::from_millis(200)).is_err(),
        "nothing was asked of the runtime"
    );
}

#[test]
fn p3_d2_an_unreachable_local_runtime_refuses_the_model() {
    // A port bound and released: nothing listens there.
    let base = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    assert!(verify_local_ollama_model(&base, "qwen3:8b").is_err());
}

// ── P3 D-2 R2: a local-only request connects to the loopback runtime directly ──
//
// Each scenario runs in a child process of this test binary, so the proxy
// variables it sets never reach another test. The parent runs a fake Ollama
// runtime, a recording proxy and an "external" recording server, all on
// loopback ports; the prompts are synthetic and nothing else is contacted.

use nexus_connectors_llm::providers::{LlmProvider, OllamaProvider};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};

const PROMPT: &str = "SYNTHETIC-D2-R2-PROMPT";
const CHILD: &str = "NEXUS_P3_D2R2_CHILD";
const PROXY_KEYS: [&str; 8] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

/// How the fake runtime answers `/api/generate` and `/api/show`.
#[derive(Clone, Copy)]
enum Generate {
    Answers,
    Fails,
    /// A 307 to the "external" recording server.
    Redirects,
}

/// What a recording server received: one line per connection.
struct Seen(mpsc::Receiver<String>);

impl Seen {
    fn all(&self) -> Vec<String> {
        std::thread::sleep(Duration::from_millis(200));
        self.0.try_iter().collect()
    }
}

/// Reads one request head (and its body) or, for a connection that sends
/// no HTTP request line within the timeout, whatever arrived, as hex.
fn read_any(stream: &TcpStream) -> (String, String) {
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut reader = BufReader::new(stream);
    let mut first = Vec::new();
    let _ = reader.read_until(b'\n', &mut first);
    let line = String::from_utf8_lossy(&first).trim_end().to_string();
    if !line.contains("HTTP/1.") {
        let hex: String = first.iter().take(8).map(|b| format!("{b:02x}")).collect();
        return (format!("BINARY {hex}"), String::new());
    }
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    (line, String::from_utf8_lossy(&body).into_owned())
}

/// A server on a loopback port that logs every connection and answers it
/// with `answer(request line, body)`, or closes it when that is `None`.
fn recording_server(
    answer: impl Fn(&str, &str) -> Option<(u16, String, String)> + Send + 'static,
) -> (SocketAddr, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (log, seen) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let (line, body) = read_any(&stream);
            let _ = log.send(format!("{line} {body}").trim_end().to_string());
            if let Some((status, extra, reply)) = answer(&line, &body) {
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        }
    });
    (addr, Seen(seen))
}

/// A fake Ollama runtime serving `qwen3:8b`.
fn fake_ollama(generate: Generate, external: SocketAddr) -> (SocketAddr, Seen) {
    recording_server(move |line, _| {
        let path = line.split(' ').nth(1).unwrap_or_default();
        let redirect = format!("location: http://{external}/api/generate\r\n");
        Some(match (path, generate) {
            ("/api/tags", _) => (
                200,
                String::new(),
                r#"{"models":[{"name":"qwen3:8b"}]}"#.into(),
            ),
            ("/api/show", Generate::Redirects) => (307, redirect, "{}".into()),
            ("/api/show", _) => (200, String::new(), r#"{"details":{}}"#.into()),
            ("/api/generate", Generate::Answers) => (
                200,
                String::new(),
                r#"{"response":"local answer","eval_count":2}"#.into(),
            ),
            ("/api/generate", Generate::Fails) => (500, String::new(), r#"{"error":"x"}"#.into()),
            ("/api/generate", Generate::Redirects) => (307, redirect, "{}".into()),
            _ => (404, String::new(), "{}".into()),
        })
    })
}

/// A recording proxy: it logs each connection and answers nothing.
fn recording_proxy() -> (SocketAddr, Seen) {
    recording_server(|_, _| None)
}

/// The local-only operations, as the desktop performs them for a local-only
/// agent: verify the model, list the models, send the prompt.
fn local_only_operations(base: &str) -> Result<String, String> {
    let verified = verify_local_ollama_model(base, "qwen3:8b").map_err(|e| format!("verify: {e}"));
    let provider = OllamaProvider::local_only(base).map_err(|e| format!("provider: {e}"))?;
    let listed = provider
        .list_models()
        .map(|models| models.len())
        .map_err(|e| format!("list: {e}"));
    let answered = provider
        .with_request_timeout(5)
        .query(PROMPT, 16, "qwen3:8b")
        .map(|response| response.output_text)
        .map_err(|e| format!("query: {e}"));
    match (verified, listed, answered) {
        (Ok(()), Ok(1), Ok(text)) => Ok(text),
        other => Err(format!("{other:?}")),
    }
}

/// The unprotected control: the same prompt through an unrestricted
/// provider, which keeps the environment's proxy settings.
fn unrestricted_control(base: &str) -> Result<String, String> {
    let provider = OllamaProvider::new(base).with_request_timeout(5);
    let listed = provider.list_models().map(|m| m.len());
    let answered = provider
        .query(PROMPT, 16, "qwen3:8b")
        .map(|r| r.output_text);
    match (listed, answered) {
        (Ok(_), Ok(text)) => Ok(text),
        other => Err(format!("{other:?}")),
    }
}

/// A scenario a child process runs against the runtime address it is given.
type Scenario = fn(&str) -> Result<String, String>;

/// In a child process: runs the scenario named by the child variable and
/// prints its result line, and returns true. In the parent: returns false.
fn child_runs(scenarios: &[(&str, Scenario)]) -> bool {
    let Ok(key) = std::env::var(CHILD) else {
        return false;
    };
    let base = std::env::var("NEXUS_P3_D2R2_BASE").unwrap();
    let scenario = scenarios
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, scenario)| *scenario)
        .unwrap();
    let line = match scenario(&base) {
        Ok(text) => format!("OK {text}"),
        Err(error) => format!("ERR {error}"),
    };
    eprintln!("\nP3-D2R2-RESULT {line}");
    true
}

/// Runs the scenario `key` of the test `test` in a child process with
/// exactly the proxy variables `proxy_env` (every other proxy variable
/// removed, `NO_PROXY` and `no_proxy` empty unless given), and returns its
/// result line.
fn in_child(test: &str, key: &str, base: &str, proxy_env: &[(&str, String)]) -> String {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD, key)
        .env("NEXUS_P3_D2R2_BASE", base)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in PROXY_KEYS {
        child.env_remove(key);
    }
    child.env("NO_PROXY", "").env("no_proxy", "");
    for (key, value) in proxy_env {
        child.env(key, value);
    }
    let output = child.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{key}: {stderr}");
    let results: Vec<&str> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("P3-D2R2-RESULT "))
        .collect();
    assert_eq!(results.len(), 1, "{key}: {stderr}");
    results[0].to_string()
}

/// A labelled set of proxy variables, and whether an unprotected request to
/// an http:// runtime goes through them.
type ProxySetting = (&'static str, Vec<(&'static str, String)>, bool);

/// The proxy settings each scenario is run under, with whether curl applies
/// them to the runtime's scheme (an unprotected request then reaches the
/// proxy). curl reads `HTTP_PROXY` only in lower case, so the upper-case
/// spelling alone does not apply to an http:// address.
fn proxy_settings(proxy: SocketAddr) -> Vec<ProxySetting> {
    let http = format!("http://{proxy}");
    vec![
        (
            "lowercase http_proxy",
            vec![("http_proxy", http.clone())],
            true,
        ),
        (
            "uppercase HTTP_PROXY only",
            vec![("HTTP_PROXY", http.clone())],
            false,
        ),
        (
            "uppercase ALL_PROXY",
            vec![("ALL_PROXY", http.clone())],
            true,
        ),
        (
            "lowercase all_proxy",
            vec![("all_proxy", http.clone())],
            true,
        ),
        (
            "ALL_PROXY socks5",
            vec![("ALL_PROXY", format!("socks5://{proxy}"))],
            true,
        ),
        (
            "all_proxy socks5h",
            vec![("all_proxy", format!("socks5h://{proxy}"))],
            true,
        ),
        (
            "every variable at once",
            vec![
                ("http_proxy", http.clone()),
                ("HTTP_PROXY", http.clone()),
                ("https_proxy", http.clone()),
                ("HTTPS_PROXY", http.clone()),
                ("all_proxy", http.clone()),
                ("ALL_PROXY", http),
            ],
            true,
        ),
    ]
}

/// With a reachable proxy configured in each way and no proxy exemption,
/// the permitted local request still goes to the runtime directly: the
/// runtime is verified, listed and sent the prompt, and the proxy receives
/// nothing. The witness's own control: the same request from an
/// unrestricted provider does reach the proxy.
#[test]
fn p3_d2r2_local_only_requests_never_reach_a_configured_proxy() {
    let test = "p3_d2r2_local_only_requests_never_reach_a_configured_proxy";
    if child_runs(&[
        ("local", local_only_operations),
        ("control", unrestricted_control),
    ]) {
        return;
    }
    let (proxy, proxy_seen) = recording_proxy();
    let (external, _) = recording_proxy();
    for (label, env, applies) in proxy_settings(proxy) {
        let (runtime, runtime_seen) = fake_ollama(Generate::Answers, external);
        let base = format!("http://{runtime}");
        let result = in_child(test, "local", &base, &env);
        let (to_runtime, to_proxy) = (runtime_seen.all(), proxy_seen.all());
        eprintln!(
            "RECEIPT protected [{label}] result={result} runtime={to_runtime:?} proxy={to_proxy:?}"
        );
        assert_eq!(result, "OK local answer", "{label}");
        assert!(
            to_proxy.is_empty(),
            "{label}: the proxy received {to_proxy:?}"
        );
        assert!(
            to_runtime.iter().any(|r| r.starts_with("POST /api/show")),
            "{label}"
        );
        assert!(
            to_runtime.iter().any(|r| r.starts_with("GET /api/tags")),
            "{label}"
        );
        let prompts: Vec<_> = to_runtime
            .iter()
            .filter(|r| r.starts_with("POST /api/generate") && r.contains(PROMPT))
            .collect();
        assert_eq!(prompts.len(), 1, "{label}: {to_runtime:?}");

        let (runtime, runtime_seen) = fake_ollama(Generate::Answers, external);
        let base = format!("http://{runtime}");
        let result = in_child(test, "control", &base, &env);
        let (to_runtime, to_proxy) = (runtime_seen.all(), proxy_seen.all());
        eprintln!(
            "RECEIPT control [{label}] result={result} runtime={to_runtime:?} proxy={to_proxy:?}"
        );
        if applies {
            assert!(
                !to_proxy.is_empty(),
                "{label}: the control did not reach the proxy"
            );
            assert!(
                !to_runtime
                    .iter()
                    .any(|r| r.starts_with("POST /api/generate")),
                "{label}: {to_runtime:?}"
            );
        }
    }
}

/// HTTPS_PROXY / https_proxy apply to an https:// runtime address. The
/// local-only request connects to the runtime directly (the plain-text
/// fake answers no TLS, so it fails closed), and the proxy receives nothing;
/// the unprotected control is a CONNECT to the proxy.
#[test]
fn p3_d2r2_https_proxy_never_receives_a_local_only_request() {
    let test = "p3_d2r2_https_proxy_never_receives_a_local_only_request";
    if child_runs(&[
        ("local", local_only_operations),
        ("control", unrestricted_control),
    ]) {
        return;
    }
    let (proxy, proxy_seen) = recording_proxy();
    let (external, _) = recording_proxy();
    for key in ["HTTPS_PROXY", "https_proxy"] {
        let env = vec![(key, format!("http://{proxy}"))];
        let (runtime, runtime_seen) = fake_ollama(Generate::Answers, external);
        let base = format!("https://{runtime}");
        let result = in_child(test, "local", &base, &env);
        let (to_runtime, to_proxy) = (runtime_seen.all(), proxy_seen.all());
        eprintln!(
            "RECEIPT protected [{key}] result={result} runtime={to_runtime:?} proxy={to_proxy:?}"
        );
        assert!(result.starts_with("ERR "), "{key}: {result}");
        assert!(
            to_proxy.is_empty(),
            "{key}: the proxy received {to_proxy:?}"
        );
        assert!(
            !to_runtime.is_empty(),
            "{key}: the runtime was connected to directly"
        );

        let result = in_child(test, "control", &base, &env);
        let to_proxy = proxy_seen.all();
        eprintln!("RECEIPT control [{key}] result={result} proxy={to_proxy:?}");
        assert!(
            to_proxy.iter().any(|r| r.starts_with("CONNECT 127.0.0.1:")),
            "{key}: the control did not reach the proxy: {to_proxy:?}"
        );
    }
}

/// A runtime that redirects, at verification or at the prompt, to an
/// external address: no redirect is followed, so neither the external
/// server nor the proxy receives anything, and the request fails.
#[test]
fn p3_d2r2_a_redirect_to_an_external_address_is_not_followed() {
    let test = "p3_d2r2_a_redirect_to_an_external_address_is_not_followed";
    fn send_prompt(base: &str) -> Result<String, String> {
        OllamaProvider::local_only(base)
            .map_err(|e| e.to_string())?
            .with_request_timeout(5)
            .query(PROMPT, 16, "qwen3:8b")
            .map(|r| r.output_text)
            .map_err(|e| e.to_string())
    }
    if child_runs(&[("local", local_only_operations), ("prompt", send_prompt)]) {
        return;
    }
    let (proxy, proxy_seen) = recording_proxy();
    let (external, external_seen) = recording_proxy();
    let env = vec![
        ("http_proxy", format!("http://{proxy}")),
        ("ALL_PROXY", format!("http://{proxy}")),
    ];
    for scenario in ["local", "prompt"] {
        let (runtime, runtime_seen) = fake_ollama(Generate::Redirects, external);
        let result = in_child(test, scenario, &format!("http://{runtime}"), &env);
        let (to_runtime, to_proxy, to_external) =
            (runtime_seen.all(), proxy_seen.all(), external_seen.all());
        eprintln!("RECEIPT redirect [{scenario}] result={result} runtime={to_runtime:?} proxy={to_proxy:?} external={to_external:?}");
        assert!(
            result.starts_with("ERR ") && result.contains("307"),
            "{result}"
        );
        assert!(to_proxy.is_empty(), "the proxy received {to_proxy:?}");
        assert!(
            to_external.is_empty(),
            "the redirect was followed: {to_external:?}"
        );
        assert!(!to_runtime.is_empty());
    }
}

/// An unavailable local runtime: every local-only operation fails, and the
/// proxy receives nothing. The control (an unrestricted listing) shows the
/// proxy would have been asked instead.
#[test]
fn p3_d2r2_an_unavailable_runtime_fails_without_the_proxy() {
    let test = "p3_d2r2_an_unavailable_runtime_fails_without_the_proxy";
    fn list_unrestricted(base: &str) -> Result<String, String> {
        OllamaProvider::new(base)
            .list_models()
            .map(|m| m.len().to_string())
            .map_err(|e| e.to_string())
    }
    if child_runs(&[
        ("local", local_only_operations),
        ("control", list_unrestricted),
    ]) {
        return;
    }
    let (proxy, proxy_seen) = recording_proxy();
    let base = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let env = vec![
        ("http_proxy", format!("http://{proxy}")),
        ("ALL_PROXY", format!("http://{proxy}")),
    ];
    let result = in_child(test, "local", &base, &env);
    let to_proxy = proxy_seen.all();
    eprintln!("RECEIPT unavailable result={result} proxy={to_proxy:?}");
    assert!(result.starts_with("ERR "), "{result}");
    assert!(to_proxy.is_empty(), "the proxy received {to_proxy:?}");

    let result = in_child(test, "control", &base, &env);
    let to_proxy = proxy_seen.all();
    eprintln!("RECEIPT unavailable control result={result} proxy={to_proxy:?}");
    assert!(!to_proxy.is_empty(), "the control did not reach the proxy");
}

/// Verification succeeds, then the prompt fails at the runtime: the failure
/// is the result, the prompt was sent once, to the runtime, and the proxy
/// receives nothing.
#[test]
fn p3_d2r2_a_dispatch_failure_after_verification_does_not_reach_the_proxy() {
    let test = "p3_d2r2_a_dispatch_failure_after_verification_does_not_reach_the_proxy";
    if child_runs(&[("local", local_only_operations)]) {
        return;
    }
    let (proxy, proxy_seen) = recording_proxy();
    let (external, _) = recording_proxy();
    let (runtime, runtime_seen) = fake_ollama(Generate::Fails, external);
    let env = vec![
        ("http_proxy", format!("http://{proxy}")),
        ("ALL_PROXY", format!("http://{proxy}")),
    ];
    let result = in_child(test, "local", &format!("http://{runtime}"), &env);
    let (to_runtime, to_proxy) = (runtime_seen.all(), proxy_seen.all());
    eprintln!("RECEIPT dispatch-failure result={result} runtime={to_runtime:?} proxy={to_proxy:?}");
    assert!(result.starts_with("ERR (Ok(()), Ok(1), Err("), "{result}");
    assert!(result.contains("status 500"), "{result}");
    assert!(to_proxy.is_empty(), "the proxy received {to_proxy:?}");
    let prompts = to_runtime
        .iter()
        .filter(|r| r.starts_with("POST /api/generate"))
        .count();
    assert_eq!(prompts, 1, "{to_runtime:?}");
}

/// A local-only provider exists only for a loopback address, and its
/// requests are refused for any other host.
#[test]
fn p3_d2r2_a_local_only_provider_is_loopback_only() {
    for remote in [
        "http://10.0.0.5:11434",
        "http://ollama.example.com:11434",
        "http://localhost.example.com:11434",
        "http://0.0.0.0:11434",
        "socks5://127.0.0.1:1080",
    ] {
        assert!(OllamaProvider::local_only(remote).is_err(), "{remote}");
    }
    let provider = OllamaProvider::local_only("http://localhost:11434").unwrap();
    assert!(provider.is_local_only());
    assert_eq!(provider.base_url(), "http://127.0.0.1:11434");
    assert!(!OllamaProvider::new("http://127.0.0.1:11434").is_local_only());
}
