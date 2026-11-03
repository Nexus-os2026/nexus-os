//! M2e-A2 guards (design v2 §14; N1 spec E-O65): E-O65 in a non-test
//! build of the kernel, where no test seam exists, and the source guards
//! E-O12 (the worker's grammar is defined once), E-O15 (the gateway's local
//! predicates are the kernel's) and E-O65 (the qualification seam is
//! test-only; the transport call boundary guards the route first; the
//! legacy chat route is gone).
//!
//! An integration test links the kernel as every production caller does,
//! without `cfg(test)`: what it observes is the shipped behaviour. The
//! runtime is a fake Ollama on a loopback port of this process. The source
//! guards read the N5 guards' production view (test code and comments
//! excluded); each scanner first proves on planted text that it flags what
//! it must.

#[allow(dead_code)]
#[path = "hm_m3_guards/source.rs"]
mod source;

use nexus_kernel::coding_run::{LocalOllama, ModelError};
use nexus_kernel::emergency_stop::StopAuthority;
use nexus_kernel::inference::{
    grammar, AbsoluteDeadline, CallCancel, CallContext, CallMeter, CancelCause, Charged,
    Conversation, LoopbackEndpoint, MeterRefusal, ModelPinSpec, ModelRequest, ModelTransport,
    RuntimeConfig, SystemClocks, Think, VerifiedOllama, N1_QUALIFIED,
};
use source::{production_files, ProdFile};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NAME: &str = "qwen3.6:35b-a3b";
const DIGEST: &str = "a7eb95c53bcf96b4bdd008d0fab4a5dac88047d9c1a7a9ab88ed453423fbd87c";

/// A healthy fake runtime: its address, its connection count and the paths
/// it was asked for.
fn fake_runtime() -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let connections = Arc::new(AtomicUsize::new(0));
    let paths = Arc::new(Mutex::new(Vec::new()));
    let (count, seen) = (connections.clone(), paths.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            count.fetch_add(1, Ordering::SeqCst);
            let seen = seen.clone();
            std::thread::spawn(move || {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut byte) {
                        Ok(1) => head.push(byte[0]),
                        _ => return,
                    }
                }
                let head = String::from_utf8_lossy(&head).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                if stream.read_exact(&mut body).is_err() {
                    return;
                }
                seen.lock().unwrap().push(path.clone());
                let reply = match path.as_str() {
                    "/api/version" => r#"{"version":"0.40.13"}"#.to_string(),
                    "/api/status" => r#"{"cloud":{"disabled":true,"source":"env"}}"#.to_string(),
                    "/api/tags" => {
                        format!(r#"{{"models":[{{"name":"{NAME}","digest":"{DIGEST}"}}]}}"#)
                    }
                    "/api/show" => {
                        r#"{"details":{},"capabilities":["completion","thinking"]}"#.to_string()
                    }
                    _ => format!(
                        r#"{{"model":"{NAME}","message":{{"role":"assistant","content":"{{\"action\":\"finish\",\"summary\":\"x\"}}"}},"done":true,"done_reason":"stop","prompt_eval_count":600,"eval_count":20}}"#
                    ),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            });
        }
    });
    (base, connections, paths)
}

#[derive(Default)]
struct Meter(Mutex<u32>);

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

// ── E-O65: the route is shut in the shipped build ───────────────────────────

#[test]
fn e_o65_in_a_non_test_build_bind_and_a_conversation_call_refuse_before_any_charge_or_connection() {
    assert!(!N1_QUALIFIED, "the route is shut until the Q-R-N1 commit");
    let (base, connections, paths) = fake_runtime();
    let stop = StopAuthority::new();
    let watch = stop.admit().unwrap();
    let deadline = AbsoluteDeadline::within(SystemClocks, Duration::from_secs(60));
    let transport = Arc::new(
        VerifiedOllama::verify(
            LoopbackEndpoint::parse(&base).unwrap(),
            &spec(),
            &Never,
            &deadline,
            &watch,
        )
        .unwrap(),
    );
    let before = connections.load(Ordering::SeqCst);
    let meter = Arc::new(Meter::default());

    let bound = LocalOllama::bind(
        transport.clone(),
        meter.clone(),
        Arc::new(Never),
        Arc::new(AbsoluteDeadline::within(
            SystemClocks,
            Duration::from_secs(60),
        )),
        watch.clone(),
    );
    assert_eq!(bound.err(), Some(ModelError::ConfigMismatch));

    let conversation = Conversation::new(
        grammar::WORKER_SYSTEM_INSTRUCTION.to_string(),
        "Owner task:\nx\n".to_string(),
    )
    .unwrap();
    let request = ModelRequest::conversation(&conversation, grammar::worker_schema()).unwrap();
    let ctx = CallContext {
        meter: &*meter,
        cancel: &Never,
        deadline: &deadline,
        stop: &watch,
        observer: None,
    };
    assert_eq!(
        transport.call(&request, &ctx).err(),
        Some(ModelError::ConfigMismatch)
    );
    assert_eq!(*meter.0.lock().unwrap(), 0, "no charge");
    assert_eq!(connections.load(Ordering::SeqCst), before, "no connection");

    // A1's qualified two-message kind is unaffected.
    let pair = ModelRequest::new(
        grammar::WORKER_SYSTEM_INSTRUCTION.to_string(),
        "Owner task:\nx\n".to_string(),
        grammar::worker_schema(),
    )
    .unwrap();
    assert!(transport.call(&pair, &ctx).is_ok());
    assert_eq!(*meter.0.lock().unwrap(), 1);
    assert_eq!(
        paths
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == "/api/chat")
            .count(),
        1
    );
}

// ── the scanners ────────────────────────────────────────────────────────────

/// Whitespace removed (for comparing code shapes).
fn squeeze(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

fn file<'a>(files: &'a [ProdFile], rel: &str) -> &'a ProdFile {
    files
        .iter()
        .find(|f| f.rel == rel)
        .unwrap_or_else(|| panic!("no production file {rel}"))
}

/// The body (between its braces) of the first `fn NAME(` after `after` in
/// `code`, or `None`.
fn fn_body<'a>(code: &'a str, after: &str, name: &str) -> Option<&'a str> {
    let from = code.find(after)?;
    let at = from + code[from..].find(&format!("fn {name}("))?;
    let open = at + code[at..].find('{')?;
    // `close_of` is the exclusive end, one past the closing brace.
    let close = source::close_of(code, open);
    Some(&code[open + 1..close - 1])
}

/// Production files whose string literals hold `needle`, with counts.
fn literal_sites(files: &[ProdFile], needle: &str) -> Vec<(String, usize)> {
    files
        .iter()
        .filter_map(|f| {
            let n = f.text.matches(needle).count();
            (n > 0).then(|| (f.rel.clone(), n))
        })
        .collect()
}

/// E-O65: the problems with the route guard in these production files.
fn route_guard_findings(files: &[ProdFile]) -> Vec<String> {
    let mut out = Vec::new();
    let impls: Vec<String> = files
        .iter()
        .flat_map(|f| {
            f.code
                .match_indices("impl ModelTransport for")
                .map(move |_| f.rel.clone())
        })
        .collect();
    if impls != ["kernel/src/inference/ollama.rs"] {
        out.push(format!("ModelTransport implementations: {impls:?}"));
    }
    for f in files {
        if f.code.contains("qualification_seam") {
            out.push(format!("{}: the test seam in production code", f.rel));
        }
    }
    if let Some(f) = files
        .iter()
        .find(|f| f.rel == "kernel/src/inference/mod.rs")
    {
        let defs = f.code.matches("fn qualification_override(").count();
        let body = fn_body(&f.code, "", "qualification_override").map(squeeze);
        if defs != 1 || body.as_deref() != Some("false") {
            out.push(format!(
                "the production override: {defs} definition(s), body {body:?}"
            ));
        }
    }
    if let Some(f) = files
        .iter()
        .find(|f| f.rel == "kernel/src/inference/ollama.rs")
    {
        match fn_body(&f.code, "impl ModelTransport for VerifiedOllama", "call") {
            Some(body) => {
                let guard = body.find("conversation_route_open");
                let first = ["request.check", "check_runtime", "charge", "exchange"]
                    .iter()
                    .filter_map(|step| body.find(step))
                    .min();
                if guard.is_none() || first.is_some_and(|first| guard > Some(first)) {
                    out.push("VerifiedOllama::call does not guard the route first".to_string());
                }
            }
            None => out.push("no VerifiedOllama::call".to_string()),
        }
    }
    if let Some(f) = files
        .iter()
        .find(|f| f.rel == "kernel/src/coding_run/local_model.rs")
    {
        if !fn_body(&f.code, "impl LocalOllama", "bind")
            .is_some_and(|body| body.contains("conversation_route_open"))
        {
            out.push("LocalOllama::bind does not guard the route".to_string());
        }
        for legacy in [
            "reqwest",
            "StopAuthority::global",
            "on_request_thread",
            "fn select(",
        ] {
            if f.code.contains(legacy) {
                out.push(format!("local_model.rs keeps the legacy route: {legacy}"));
            }
        }
        if f.text.contains("\"api/") || f.text.contains("/api/") {
            out.push("local_model.rs names an Ollama path".to_string());
        }
    }
    out
}

/// E-O12: the problems with the worker grammar's single definition.
fn grammar_findings(files: &[ProdFile]) -> Vec<String> {
    let mut out = Vec::new();
    let sites = literal_sites(files, "You are the Nexus OS coding worker");
    if sites != [("kernel/src/inference/grammar.rs".to_string(), 1)] {
        out.push(format!("the system instruction is defined at {sites:?}"));
    }
    if let Some(worker) = files
        .iter()
        .find(|f| f.rel == "kernel/src/coding_run/worker.rs")
    {
        for second in [
            "deny_unknown_fields",
            "enum ModelAction",
            "struct RawProposal",
        ] {
            if worker.code.contains(second) {
                out.push(format!("worker.rs defines its own grammar: {second}"));
            }
        }
        for used in ["grammar::WORKER_SYSTEM_INSTRUCTION", "grammar::parse"] {
            if !worker.code.contains(used) {
                out.push(format!("worker.rs does not use {used}"));
            }
        }
    }
    out
}

/// E-O15: the problems with the gateway's re-export of the predicates.
fn gateway_findings(gateway: &ProdFile) -> Vec<String> {
    let mut out = Vec::new();
    for local in [
        "fn ollama_model_name_is_cloud",
        "fn ollama_show_names_remote",
    ] {
        if gateway.code.contains(local) {
            out.push(format!("the gateway defines {local}"));
        }
    }
    let code = squeeze(&gateway.code);
    for export in [
        "name_is_cloudasollama_model_name_is_cloud",
        "show_names_remoteasollama_show_names_remote",
    ] {
        let re_exported = code
            .split("pubusenexus_kernel::inference::ollama::{")
            .skip(1)
            .any(|rest| {
                rest.split('}')
                    .next()
                    .is_some_and(|list| list.contains(export))
            });
        if !re_exported {
            out.push(format!("the gateway does not re-export {export}"));
        }
    }
    out
}

fn planted(rel: &str, src: &str) -> ProdFile {
    ProdFile {
        rel: rel.into(),
        code: source::mask(src, false),
        text: source::mask(src, true),
    }
}

#[test]
fn m2e_a2_scanners_flag_planted_violations() {
    let ollama = |call: &str| {
        planted(
            "kernel/src/inference/ollama.rs",
            &format!("impl ModelTransport for VerifiedOllama {{ fn call(&self) {{ {call} }} }}"),
        )
    };
    let unguarded = [ollama(
        "request.check()?; check_runtime(); if !conversation_route_open() {}",
    )];
    assert!(route_guard_findings(&unguarded)
        .iter()
        .any(|f| f.contains("guard the route first")));
    let guarded = [ollama(
        "if !conversation_route_open() {} request.check()?; charge();",
    )];
    assert!(route_guard_findings(&guarded).is_empty());
    let leaked = [planted(
        "kernel/src/inference/mod.rs",
        "fn qualification_override() -> bool { qualification_seam::is_open() }",
    )];
    assert_eq!(route_guard_findings(&leaked).len(), 3);
    let legacy = [planted(
        "kernel/src/coding_run/local_model.rs",
        "impl LocalOllama { fn bind() { conversation_route_open() } fn complete() { let u = \"api/chat\"; } }",
    )];
    assert!(route_guard_findings(&legacy)
        .iter()
        .any(|f| f.contains("names an Ollama path")));

    let twice = [
        planted(
            "kernel/src/inference/grammar.rs",
            "const S: &str = \"You are the Nexus OS coding worker.\";",
        ),
        planted(
            "kernel/src/coding_run/worker.rs",
            "const S: &str = \"You are the Nexus OS coding worker.\"; #[serde(deny_unknown_fields)] enum ModelAction {}",
        ),
    ];
    assert_eq!(grammar_findings(&twice).len(), 5);

    let local = planted(
        "connectors/llm/src/gateway.rs",
        "pub fn ollama_model_name_is_cloud(m: &str) -> bool { false }",
    );
    assert_eq!(gateway_findings(&local).len(), 3);
    let exported = planted(
        "connectors/llm/src/gateway.rs",
        "pub use nexus_kernel::inference::ollama::{\n    name_is_cloud as ollama_model_name_is_cloud,\n    show_names_remote as ollama_show_names_remote,\n};",
    );
    assert!(gateway_findings(&exported).is_empty());
}

// ── E-O65, E-O12, E-O15 on the workspace ────────────────────────────────────

#[test]
fn e_o65_the_seam_is_test_only_and_the_transport_call_guards_the_route_first() {
    let files = production_files();
    let findings = route_guard_findings(&files);
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn e_o12_the_worker_grammar_is_defined_once() {
    let files = production_files();
    let findings = grammar_findings(&files);
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn e_o15_the_gateway_predicates_are_the_kernels() {
    let files = production_files();
    let findings = gateway_findings(file(&files, "connectors/llm/src/gateway.rs"));
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}
