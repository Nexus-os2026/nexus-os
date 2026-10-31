//! P3-C11-HM-M3: transport attestation (P3-HM-CONTRACT-02 §2.4, §11.0).
//!
//! `attest_route` is driven with concrete provider objects. (L1) needs the
//! verification proof of a fake Ollama runtime on a loopback port; nothing
//! else is contacted. A-F2 is a build guard over `llama-bridge`. The Flash
//! (L2) path conditions run in the desktop crate, which builds this crate
//! with `flash-infer`; here, without it, no Flash provider is local (A-F4).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

#[cfg(not(feature = "flash-infer"))]
use nexus_connectors_llm::attest::attest_route_in;
use nexus_connectors_llm::attest::{attest_route, flash_model_dir_for_home, verify_local_ollama};
#[cfg(not(feature = "flash-infer"))]
use nexus_connectors_llm::providers::FlashProvider;
use nexus_connectors_llm::providers::{LlmProvider, LlmResponse, OllamaProvider};
use nexus_kernel::errors::AgentError;
use nexus_kernel::hm::RouteClass;

/// Serves one reply per connection, `replies` in order, and reports each
/// request (`METHOD path body`).
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
            if head.is_empty() {
                // A connect-only probe (the provider's health check).
                continue;
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

const SHOW_LOCAL: &str = r#"{"modelfile":"FROM x","details":{}}"#;
const GENERATED: &str = r#"{"model":"qwen3:8b","response":"SYNTH","done":true,"eval_count":1}"#;

/// A provider of another crate that claims to be a loopback Ollama runtime
/// through every trait method it has.
struct ClaimsLocal;

impl LlmProvider for ClaimsLocal {
    fn query(&self, _: &str, _: u32, _: &str) -> Result<LlmResponse, AgentError> {
        Err(AgentError::SupervisorError("never".into()))
    }
    fn name(&self) -> &str {
        "ollama"
    }
    fn cost_per_token(&self) -> f64 {
        0.0
    }
    fn endpoint_url(&self) -> String {
        "http://127.0.0.1:11434".into()
    }
}

/// (L1): only the verification proof of a direct-loopback runtime is local.
#[test]
fn hm_m3_n2_l1_only_a_verified_direct_loopback_ollama_is_local() {
    let (base, seen) = fake_runtime(vec![(200, SHOW_LOCAL), (200, GENERATED)]);
    let verified = verify_local_ollama(&base, "qwen3:8b").unwrap();
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(5)).unwrap(),
        r#"POST /api/show {"model":"qwen3:8b"}"#
    );
    assert_eq!(attest_route(&verified).class(), RouteClass::Local);
    assert_eq!(verified.model(), "qwen3:8b");
    // The proof sends to the verified model, whatever the caller names.
    verified.query("SYNTH-prompt", 16, "other:model").unwrap();
    let sent = seen.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(sent.starts_with("POST /api/"), "{sent}");
    assert!(sent.contains(r#""model":"qwen3:8b""#), "{sent}");
    assert!(!sent.contains("other:model"), "{sent}");

    // Not a proof: non-local, whatever the address.
    for unverified in [
        OllamaProvider::new("https://ollama.example.com"),
        OllamaProvider::new("http://127.0.0.1:11434"),
        OllamaProvider::new(base.as_str()),
        OllamaProvider::local_only(&base).unwrap(),
    ] {
        assert_eq!(
            attest_route(&unverified).class(),
            RouteClass::NonLocal,
            "{unverified:?}"
        );
    }
    // Another crate's type claiming the same name and endpoint, a mock, a
    // boxed proof (the concrete object is required) and arbitrary values.
    assert_eq!(attest_route(&ClaimsLocal).class(), RouteClass::NonLocal);
    let mock = nexus_connectors_llm::providers::MockProvider::new();
    assert_eq!(attest_route(&mock).class(), RouteClass::NonLocal);
    let boxed: Box<dyn LlmProvider> = Box::new(verified);
    assert_eq!(attest_route(&boxed).class(), RouteClass::NonLocal);
    assert_eq!(attest_route(&"ollama").class(), RouteClass::NonLocal);
    assert_eq!(attest_route(&()).class(), RouteClass::NonLocal);
}

/// The verification refuses, and so yields no proof, for a cloud model, a
/// relayed or unknown model, an unreachable runtime and a non-loopback
/// address.
#[test]
fn hm_m3_n2_l1_no_proof_without_the_d2_r2_verification() {
    let (base, _seen) = fake_runtime(vec![
        (200, r#"{"remote_host":"https://ollama.com:443"}"#),
        (404, r#"{"error":"not found"}"#),
    ]);
    assert!(verify_local_ollama(&base, "qwen3:8b").is_err(), "relayed");
    assert!(verify_local_ollama(&base, "qwen3:8b").is_err(), "unknown");
    assert!(verify_local_ollama(&base, "gpt-oss:120b-cloud").is_err());
    assert!(verify_local_ollama("https://ollama.example.com", "qwen3:8b").is_err());
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = closed.local_addr().unwrap();
    drop(closed);
    assert!(verify_local_ollama(&format!("http://{addr}"), "qwen3:8b").is_err());
}

/// A-F4 (feature off): this crate is built here without `flash-infer`, so
/// a Flash provider runs no in-process inference and is non-local, even
/// with its model a regular file inside the model directory.
///
/// Built only without `flash-infer`: a workspace build unifies the feature
/// on (the desktop enables it), and the feature-on conditions are tested in
/// the desktop crate (`hm_wiring` A-F4 and A1.5 (b)).
#[cfg(not(feature = "flash-infer"))]
#[test]
fn hm_m3_n2_a_f4_flash_without_in_process_inference_is_non_local() {
    const { assert!(!cfg!(feature = "flash-infer")) };
    let dir = std::env::temp_dir().join(format!(
        "nexus-hm-m3-attest-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("synthetic.gguf");
    std::fs::write(&model, b"SYNTH").unwrap();
    let flash = FlashProvider::new(model.to_string_lossy().into_owned());
    assert_eq!(
        attest_route_in(&flash, Some(&dir)).class(),
        RouteClass::NonLocal
    );
    let shared = std::sync::Arc::new(flash);
    assert_eq!(
        attest_route_in(&shared, Some(&dir)).class(),
        RouteClass::NonLocal
    );
    assert_eq!(attest_route(&shared).class(), RouteClass::NonLocal);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The backend-owned Flash model directory, as the Flash storage defines it,
/// for a synthetic home; none for a relative or empty home.
#[test]
fn hm_m3_n2_flash_model_dir_follows_the_storage_rule() {
    let home = Path::new(if cfg!(windows) {
        r"C:\synthetic-home"
    } else {
        "/synthetic-home"
    });
    let expected: PathBuf = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/nexus-os/models")
    } else if cfg!(target_os = "windows") {
        home.join(r"AppData\Roaming\nexus-os\models")
    } else {
        home.join(".local/share/nexus-os/models")
    };
    assert_eq!(flash_model_dir_for_home(home), Some(expected));
    assert_eq!(flash_model_dir_for_home(Path::new("relative")), None);
    assert_eq!(flash_model_dir_for_home(Path::new("")), None);
}

// ── A-F2: the bridge links no remote inference backend ─────────────────────

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The `cargo:rustc-link-lib=` values a build script prints.
fn link_libs(build_rs: &str) -> Vec<String> {
    build_rs
        .match_indices("cargo:rustc-link-lib=")
        .map(|(i, m)| {
            build_rs[i + m.len()..]
                .chars()
                .take_while(|c| !matches!(c, '"' | '\'' | ')' | '\n' | '{'))
                .collect::<String>()
        })
        .collect()
}

/// The libraries the bridge may link: llama.cpp's CPU core, the local GPU
/// offload libraries, and system runtime libraries. No remote backend
/// (`ggml-rpc`) and no network library.
const ALLOWED_LIBS: [&str; 21] = [
    "static=llama",
    "static=ggml",
    "static=ggml-base",
    "static=ggml-cpu",
    "static=ggml-cuda",
    "cuda",
    "cublas",
    "cublasLt",
    "cudart",
    "framework=Accelerate",
    "framework=Metal",
    "framework=MetalKit",
    "framework=Foundation",
    "stdc++",
    "c++",
    "m",
    "pthread",
    "gomp",
    "user32",
    "kernel32",
    "advapi32",
];

const REMOTE_MARKERS: [&str; 9] = [
    "rpc", "curl", "http", "ssl", "socket", "grpc", "ws2_32", "remote", "server",
];

fn link_violations(build_rs: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let libs = link_libs(build_rs);
    if libs.is_empty() {
        bad.push("no link directive found".to_string());
    }
    for lib in libs {
        if !ALLOWED_LIBS.contains(&lib.as_str()) {
            bad.push(format!("unexpected link library {lib}"));
        }
    }
    let lower = build_rs.to_ascii_lowercase();
    for marker in ["ggml-rpc", "ggml_rpc", "llama_rpc", "-dggml_rpc"] {
        if lower.contains(marker) {
            bad.push(format!("remote backend marker {marker}"));
        }
    }
    bad
}

/// The field names of `pub struct <name> { ... }` in `src`.
fn struct_fields(src: &str, name: &str) -> Vec<String> {
    let start = src
        .find(&format!("pub struct {name} {{"))
        .unwrap_or_else(|| panic!("struct {name} not found"));
    let body = &src[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with('#') && l.contains(':'))
        .map(|l| {
            l.trim_start_matches("pub ")
                .split(':')
                .next()
                .unwrap()
                .trim()
                .to_string()
        })
        .collect()
}

fn remote_fields(fields: &[String]) -> Vec<String> {
    fields
        .iter()
        .filter(|f| {
            let f = f.to_ascii_lowercase();
            REMOTE_MARKERS
                .iter()
                .chain(["url", "host", "endpoint", "address", "port"].iter())
                .any(|m| f.contains(m))
        })
        .cloned()
        .collect()
}

/// Mother Nexus cleanup (2026-10-31): `llama-bridge` (with
/// `nexus-flash-infer` and the `flash-infer` feature that linked them) was
/// removed (see `archive/nexus-before-mother`). This tripwire fails as soon
/// as the bridge returns, so the A-F2 checks on its `build.rs` link
/// directives and its `ModelLoadConfig` fields must be restored and
/// re-pinned before the guard can pass.
fn assert_llama_bridge_removed() {
    let root = workspace_root();
    assert!(
        !root.join("llama-bridge").exists(),
        "llama-bridge returned; restore and re-pin its A-F2 checks"
    );
    for manifest in ["Cargo.toml", "connectors/llm/Cargo.toml"] {
        let toml = std::fs::read_to_string(root.join(manifest)).unwrap();
        // TOML comments (the manifest's notes on the removal) are no use.
        for line in toml.lines().map(|l| l.split('#').next().unwrap_or("")) {
            assert!(
                !line.contains("llama-bridge") && !line.contains("llama_bridge"),
                "{manifest} names llama-bridge; restore and re-pin its A-F2 checks: {line}"
            );
        }
    }
}

/// A-F2 guarded `llama-bridge/build.rs` (only the allowed local link
/// libraries, no remote backend) and `llama-bridge/src/types.rs` (a
/// `ModelLoadConfig` with no remote field). The bridge was deleted: the
/// tripwire stands in for those checks, and the scanners' planted controls
/// run on a synthetic bridge of the same shape.
#[test]
fn hm_m3_n2_a_f2_the_bridge_links_no_remote_backend_and_loads_no_server() {
    assert_llama_bridge_removed();
    let build_rs = concat!(
        "fn main() {\n",
        "    println!(\"cargo:rustc-link-lib=static=llama\");\n",
        "    println!(\"cargo:rustc-link-lib=static=ggml\");\n",
        "    println!(\"cargo:rustc-link-lib=static=ggml-base\");\n",
        "    println!(\"cargo:rustc-link-lib=static=ggml-cpu\");\n",
        "    println!(\"cargo:rustc-link-lib=stdc++\");\n",
        "}\n"
    );
    assert_eq!(link_violations(build_rs), Vec::<String>::new());
    let libs = link_libs(build_rs);
    for core in [
        "static=llama",
        "static=ggml",
        "static=ggml-base",
        "static=ggml-cpu",
    ] {
        assert!(libs.iter().any(|l| l == core), "{core} in {libs:?}");
    }
    for lib in &libs {
        let lower = lib.to_ascii_lowercase();
        for marker in REMOTE_MARKERS {
            assert!(!lower.contains(marker), "{lib}");
        }
    }
    let types = concat!(
        "pub struct ModelLoadConfig {\n",
        "    /// Filesystem path to the GGUF file.\n",
        "    pub model_path: String,\n",
        "    pub n_gpu_layers: i32,\n",
        "    pub use_mmap: bool,\n",
        "    pub n_threads: Option<u32>,\n",
        "}\n"
    );
    let fields = struct_fields(types, "ModelLoadConfig");
    assert!(fields.contains(&"model_path".to_string()), "{fields:?}");
    assert_eq!(remote_fields(&fields), Vec::<String>::new(), "{fields:?}");

    // Planted negative controls: each must be detected.
    let planted_rpc = concat!("println!(\"cargo:rustc-link-lib=static=ggml-", "rpc\");\n");
    assert!(!link_violations(&format!("{build_rs}{planted_rpc}")).is_empty());
    let planted_curl = concat!("println!(\"cargo:rustc-link-lib=cu", "rl\");\n");
    assert!(!link_violations(&format!("{build_rs}{planted_curl}")).is_empty());
    let planted_flag = concat!("cmake.define(\"GGML_", "RPC\", \"ON\");\n");
    assert!(!link_violations(&format!("{build_rs}{planted_flag}")).is_empty());
    assert!(!link_violations("fn main() {}").is_empty(), "no directive");
    let planted_field = types.replacen(
        "    pub model_path: String,",
        concat!(
            "    pub model_path: String,\n    pub rpc_",
            "servers: String,"
        ),
        1,
    );
    assert!(!remote_fields(&struct_fields(&planted_field, "ModelLoadConfig")).is_empty());
    let planted_url = types.replacen(
        "    pub model_path: String,",
        concat!(
            "    pub model_path: String,\n    pub server_",
            "url: String,"
        ),
        1,
    );
    assert!(!remote_fields(&struct_fields(&planted_url, "ModelLoadConfig")).is_empty());
}
