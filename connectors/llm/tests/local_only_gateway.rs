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
