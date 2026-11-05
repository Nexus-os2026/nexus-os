//! M2e-A2 guards (design v2 §14; N1 spec E-O65): E-O65 in a non-test
//! build of the kernel, where no test seam exists, and the source guards
//! E-O12 (the worker's grammar is defined once), E-O15 (local-model predicates
//! occur only in kernel inference) and E-O65 (the qualification seam is
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

#[path = "hm_m3_guards/predicate_controls_tests.rs"]
mod predicate_controls_tests;

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
// The N1_QUALIFIED pin is the point: a constant until the Q-R-N1 commit flips it.
#[allow(clippy::assertions_on_constants)]
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

/// Exact production identifier inventory, measured through `production_files()`.
/// Doc comments and literal contents do not confer permission to name the trait.
const MODEL_TRANSPORT_ALLOWLIST: [(&str, usize); 4] = [
    ("kernel/src/inference/mod.rs", 1),
    ("kernel/src/inference/ollama.rs", 2),
    ("kernel/src/coding_run/local_model.rs", 1),
    ("crates/nexus-governed-control/src/inference.rs", 1),
];

/// Tokens from the masked code view. Keep punctuation so paths and generic
/// arguments can be read independently of whitespace; normalize raw identifiers.
fn route_tokens(code: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < code.len() {
        let ch = code[at..].chars().next().unwrap();
        if ch.is_whitespace() {
            at += ch.len_utf8();
            continue;
        }
        if code[at..].starts_with("r#")
            && code[at + 2..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        {
            at += 2;
        }
        let start = at;
        if code[at..].starts_with("::") || code[at..].starts_with("->") {
            at += 2;
        } else {
            let ch = code[at..].chars().next().unwrap();
            at += ch.len_utf8();
            if ch.is_alphanumeric() || ch == '_' {
                while let Some(ch) = code[at..].chars().next() {
                    if !(ch.is_alphanumeric() || ch == '_') {
                        break;
                    }
                    at += ch.len_utf8();
                }
            }
        }
        tokens.push(&code[start..at]);
    }
    tokens
}

/// Skip a balanced generic argument/parameter list or delimiter group.
/// Arrows are single tokens; comparisons inside const blocks cannot close `<`.
fn route_group_end(tokens: &[&str], start: usize) -> Option<usize> {
    let close = match *tokens.get(start)? {
        "<" => ">",
        "(" => ")",
        "[" => "]",
        "{" => "}",
        _ => return None,
    };
    let mut at = start + 1;
    while let Some(&token) = tokens.get(at) {
        if token == close {
            return Some(at + 1);
        }
        if matches!(token, "(" | "[" | "{") || (close == ">" && token == "<") {
            at = route_group_end(tokens, at)?;
        } else {
            at += 1;
        }
    }
    None
}

/// Read the trait path after `impl` and optional generics. Only the final
/// segment before `for` decides trait identity, never an exact header spelling.
fn model_transport_impl(tokens: &[&str], mut at: usize) -> bool {
    if tokens.get(at) == Some(&"<") {
        let Some(end) = route_group_end(tokens, at) else {
            return false;
        };
        at = end;
    }
    if tokens.get(at) == Some(&"!") {
        at += 1;
    }
    if tokens.get(at) == Some(&"::") {
        at += 1;
    }
    loop {
        let Some(&segment) = tokens.get(at) else {
            return false;
        };
        at += 1;
        // Type paths also allow `segment::<Args>`.
        if tokens.get(at..at + 2) == Some(&["::", "<"]) {
            at += 1;
        }
        if tokens.get(at) == Some(&"<") {
            let Some(end) = route_group_end(tokens, at) else {
                return false;
            };
            at = end;
        }
        match tokens.get(at) {
            Some(&"::") => at += 1,
            Some(&"for") => return segment == "ModelTransport",
            _ => return false,
        }
    }
}

/// Inside a macro definition, a caller-controlled trait position may introduce
/// another transport without spelling its name in the implementation header.
/// Skip impl generics, then inspect only the trait position before `for`.
fn macro_transport_impl(tokens: &[&str], mut at: usize) -> bool {
    if tokens.get(at) == Some(&"<") {
        let Some(end) = route_group_end(tokens, at) else {
            return false;
        };
        at = end;
    }
    let trait_start = at;
    let mut metavariable = false;
    while let Some(&token) = tokens.get(at) {
        match token {
            "for" => return metavariable || model_transport_impl(tokens, trait_start),
            "{" | ";" | "where" => return false,
            "<" | "(" | "[" => {
                let Some(end) = route_group_end(tokens, at) else {
                    return false;
                };
                metavariable |= tokens[at..end].contains(&"$");
                at = end;
            }
            _ => {
                metavariable |= token == "$";
                at += 1;
            }
        }
    }
    false
}

fn route_macro_findings(file: &ProdFile, tokens: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (at, &token) in tokens.iter().enumerate() {
        if token == "macro_rules" && tokens.get(at + 1) == Some(&"!") {
            let open = at + 3; // macro_rules ! NAME DELIMITER
            if let Some(end) = route_group_end(tokens, open) {
                for body_at in open + 1..end - 1 {
                    if tokens[body_at] == "impl" && macro_transport_impl(tokens, body_at + 1) {
                        out.push(format!(
                            "{}: macro_rules trait implementation can name ModelTransport",
                            file.rel
                        ));
                    }
                }
            } else {
                out.push(format!("{}: unclassified macro_rules body", file.rel));
            }
        }
        if token != "!" || at == 0 || tokens[at - 1] == "macro_rules" {
            continue;
        }
        let name = tokens[at - 1];
        let identifier = name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphabetic() || ch == '_');
        // A keyword followed by unary !(...) is not an invocation. Raw macro
        // names remain identifiers after route_tokens normalizes their r#.
        let offset = name.as_ptr() as usize - file.code.as_ptr() as usize;
        let raw = file.code[..offset].ends_with("r#");
        if !identifier || (!raw && matches!(name, "if" | "while" | "return" | "break" | "match")) {
            continue;
        }
        if route_group_end(tokens, at + 1)
            .is_some_and(|end| tokens[at + 2..end - 1].contains(&"ModelTransport"))
        {
            out.push(format!(
                "{}: macro invocation names ModelTransport",
                file.rel
            ));
        }
    }
    out
}

/// Resolve the named import root, including kernel-local self/super paths.
/// A module merely named inference beneath some other root is not the kernel
/// inference module. Globs inside the inference authority stay permitted.
fn route_inference_import(rel: &str, path: &[&str]) -> bool {
    if path.starts_with(&["nexus_kernel", "inference"]) {
        return true;
    }
    let Some(local) = rel.strip_prefix("kernel/src/") else {
        return false;
    };
    if path.starts_with(&["crate", "inference"]) {
        return true;
    }
    let mut module: Vec<&str> = local.split('/').collect();
    let file = module.pop().unwrap_or("");
    if !matches!(file, "lib.rs" | "main.rs" | "mod.rs") {
        module.push(file.strip_suffix(".rs").unwrap_or(file));
    }
    let mut at = 0;
    if path.first() == Some(&"self") {
        at = 1;
    } else if path.first() == Some(&"super") {
        while path.get(at) == Some(&"super") {
            module.pop();
            at += 1;
        }
    } else {
        return path.first() == Some(&"inference");
    }
    module.extend_from_slice(&path[at..]);
    module.first() == Some(&"inference")
}

/// Read one use tree, preserving each group's inherited prefix. This catches
/// grouped forms such as nexus_kernel::{inference::{self, ollama::{*}}, Other}.
fn route_use_tree<'a>(tokens: &[&'a str], at: &mut usize, prefix: &[&'a str], rel: &str) -> bool {
    let mut path = prefix.to_vec();
    let mut found = false;
    if tokens.get(*at) == Some(&"::") {
        path.clear();
        *at += 1;
    }
    while let Some(&token) = tokens.get(*at) {
        match token {
            "*" => {
                *at += 1;
                return route_inference_import(rel, &path);
            }
            "{" => {
                *at += 1;
                while tokens.get(*at).is_some_and(|&token| token != "}") {
                    let before = *at;
                    found |= route_use_tree(tokens, at, &path, rel);
                    if tokens.get(*at) == Some(&",") {
                        *at += 1;
                    } else if *at == before {
                        // Unknown syntax cannot trap the scanner in a loop.
                        *at += 1;
                    }
                }
                if tokens.get(*at) == Some(&"}") {
                    *at += 1;
                }
                return found;
            }
            "as" => {
                *at += 2;
                return found;
            }
            "," | "}" | ";" => return found,
            "::" => *at += 1,
            _ => {
                path.push(token);
                *at += 1;
            }
        }
    }
    found
}

fn route_inference_glob_findings(file: &ProdFile, tokens: &[&str]) -> Vec<String> {
    if file.rel.starts_with("kernel/src/inference/") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (at, &token) in tokens.iter().enumerate() {
        if token == "use" && route_use_tree(tokens, &mut (at + 1), &[], &file.rel) {
            out.push(format!(
                "{}: inference glob import outside kernel inference",
                file.rel
            ));
        }
    }
    out
}

/// E-O65 (stricter oracle amendment): every impl header naming a trait path
/// ending in ModelTransport counts, including qualified/raw/generic spellings.
/// Exactly one implementation, in ollama.rs, is allowed. Trait renaming is
/// forbidden; every identifier occurrence also needs an exact file/count entry.
/// R-3 additionally rejects caller-supplied macro trait positions, named
/// ModelTransport macro trait paths/arguments, and inference globs outside its
/// module. Concrete macro trait paths with metavariables only in Self stay green.
fn model_transport_findings(files: &[ProdFile]) -> Vec<String> {
    let mut out = Vec::new();
    let mut impls = Vec::new();
    for file in files {
        let tokens = route_tokens(&file.code);
        out.extend(route_macro_findings(file, &tokens));
        out.extend(route_inference_glob_findings(file, &tokens));
        let count = tokens.iter().filter(|&&t| t == "ModelTransport").count();
        let expected = MODEL_TRANSPORT_ALLOWLIST
            .iter()
            .find(|&&(rel, _)| rel == file.rel)
            .map_or(0, |&(_, count)| count);
        if count != expected {
            out.push(format!(
                "{}: ModelTransport identifiers: {count}, exact allowlist count {expected}",
                file.rel
            ));
        }
        for (at, &token) in tokens.iter().enumerate() {
            if token == "impl" && model_transport_impl(&tokens, at + 1) {
                impls.push(file.rel.clone());
            }
            if token == "ModelTransport" && tokens.get(at + 1) == Some(&"as") {
                out.push(format!(
                    "{}: ModelTransport renaming is forbidden",
                    file.rel
                ));
            }
        }
    }
    if impls != ["kernel/src/inference/ollama.rs"] {
        out.push(format!("ModelTransport implementations: {impls:?}"));
    }
    out
}

/// E-O65: the problems with the route guard in these production files.
/// Checks the ModelTransport impl/alias/file-count rules above, rejects
/// qualification_seam, and checks present files for a sole false-bodied
/// qualification_override, conversation_route_open before request.check,
/// check_runtime, charge or exchange in VerifiedOllama::call, and
/// conversation_route_open in LocalOllama::bind. Also flags the listed legacy
/// route tokens and API paths.
/// Per Ruling R-2, these source-text checks are regression tripwires for ordinary,
/// reviewed code. The reviewed macro, literal and cfg gaps have explicit
/// red/green controls; this is not an adversarial-completeness claim. Independent
/// PR review, Mother's sealed effect port and the broker's runtime authority
/// remain required, including for include!, build scripts and constructed code.
fn route_guard_findings(files: &[ProdFile]) -> Vec<String> {
    let mut out = model_transport_findings(files);
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

/// Exact production literal exemptions: (file, literal content, count, why).
const PREDICATE_LITERAL_ALLOWLIST: [(&str, &str, usize, &str); 1] = [(
    "kernel/src/actuators/tts.rs",
    "cloud",
    1,
    "Legacy actuator scheduled for removal in the kernel plan",
)];

const PREDICATE_LITERALS: [&str; 3] = ["cloud", "remote_host", "remote_model"];

/// Count complete string-literal contents, using the existing production views.
/// Only delimiter quotes survive in `code`: quotes inside ordinary/raw/byte
/// strings and char literals are masked. Pair those delimiters and read each
/// complete content from `text`, where comments and test code are excluded.
fn predicate_literal_counts(file: &ProdFile) -> [usize; 3] {
    let mut counts = [0; 3];
    let mut quotes = file.code.match_indices('"');
    while let (Some((open, _)), Some((close, _))) = (quotes.next(), quotes.next()) {
        let content = &file.text[open + 1..close];
        if let Some(i) = PREDICATE_LITERALS.iter().position(|&s| s == content) {
            counts[i] += 1;
        }
    }
    counts
}

/// E-O15: no second copy of the local-model predicates outside kernel inference.
/// Fix round 1 rejected ANY occurrence of the four protected identifiers,
/// including raw identifiers, macro arguments/bodies, calls and imports.
/// Fix round 2 is stricter: retain that fail-closed identifier rule AND reject
/// exact string-literal contents "cloud", "remote_host" and "remote_model"
/// outside kernel/src/inference/ (raw, byte and C strings included; comments and
/// test code excluded), except the exact file/literal/count allowlist above.
/// `git grep` plus the production inventory verified the sole exemption:
/// tts.rs's `"openai" | "cloud"`; identifier exemptions remain EMPTY.
///
/// Semantic clones using NEITHER the protected identifiers NOR these literals
/// are explicitly out of scope. Per Ruling R-2, the identifier and exact-literal
/// rules above are regression tripwires for ordinary, reviewed code, as are HM
/// model-call classification, transport provider classification and E-O65.
/// Reviewed macro, literal and cfg gaps have explicit red/green controls;
/// this is not an adversarial-completeness claim. Independent PR review,
/// Mother's sealed effect port and the broker's runtime authority remain
/// required, including for include!, build scripts and constructed code.
fn predicate_findings(files: &[ProdFile]) -> Vec<String> {
    let mut out = Vec::new();
    for file in files {
        if file.rel.starts_with("kernel/src/inference/") {
            continue;
        }
        // `#` is a separator so both ordinary and raw identifiers are caught,
        // including names next to macro/attribute punctuation.
        let tokens: Vec<&str> = file
            .code
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|token| !token.is_empty())
            .collect();
        for (i, &name) in tokens.iter().enumerate() {
            if matches!(
                name,
                "name_is_cloud"
                    | "show_names_remote"
                    | "ollama_model_name_is_cloud"
                    | "ollama_show_names_remote"
            ) {
                // Retain the old direct-definition diagnostic, also for r#name.
                let definition = i > 0 && tokens[i - 1] == "fn"
                    || i > 1 && tokens[i - 1] == "r" && tokens[i - 2] == "fn";
                let detail = if definition {
                    "defines its own"
                } else {
                    "contains protected predicate identifier"
                };
                out.push(format!("{} {detail} {name}", file.rel));
            }
        }
        for (literal, count) in PREDICATE_LITERALS
            .iter()
            .zip(predicate_literal_counts(file))
        {
            let expected = PREDICATE_LITERAL_ALLOWLIST
                .iter()
                .find(|&&(rel, content, _, _)| rel == file.rel && content == *literal)
                .map_or(0, |&(_, _, count, _)| count);
            if count != expected {
                out.push(format!(
                    "{}: predicate literal {literal:?}: {count} occurrence(s), exact allowlist count {expected}",
                    file.rel
                ));
            }
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
            &format!("use super::ModelTransport; impl ModelTransport for VerifiedOllama {{ fn call(&self) {{ {call} }} }}"),
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
    assert_eq!(route_guard_findings(&leaked).len(), 4);
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
}

#[test]
fn e_o65_model_transport_inventory_rejects_alternate_spellings_and_aliases() {
    let approved = || {
        planted(
            "kernel/src/inference/ollama.rs",
            "use super::ModelTransport; impl ModelTransport for VerifiedOllama {}",
        )
    };
    assert!(model_transport_findings(&[approved()]).is_empty());
    // Reviewer's bypass: qualification, metering and Stop are absent.
    let qualified = r#"
impl crate::inference::ModelTransport for Alternate {
    fn call(&self) { self.client.post(&self.endpoint).send(); }
}
"#;
    for header in [
        qualified,
        "impl<T> ModelTransport for Wrapper<T> {}",
        "impl ::nexus_kernel::inference::ModelTransport for X {}",
        "impl\n crate :: inference ::\n ModelTransport\n for X {}",
        "impl r#ModelTransport for X {}",
        "impl<T: Fn() -> Vec<U>, U> crate::inference::r#ModelTransport for Wrapper<T> {}",
        "impl<T> crate::ModelTransport<Vec<T>> for Wrapper<T> {}",
        "impl<T> crate::module::<T>::ModelTransport::<T> for Wrapper<T> {}",
        "impl<const N: bool = { 1 < 2 }> ModelTransport for X {}",
    ] {
        let found =
            model_transport_findings(&[approved(), planted("sdk/src/alternate.rs", header)]);
        assert!(
            found
                .iter()
                .any(|f| f.contains("ModelTransport implementations:")),
            "implementation must count: {header}: {found:?}"
        );
    }
    let renamed = "use crate::inference::ModelTransport as Mt; impl Mt for X {}";
    let found = model_transport_findings(&[approved(), planted("sdk/src/alternate.rs", renamed)]);
    assert!(found.iter().any(|f| f.contains("renaming is forbidden")));
    assert!(found
        .iter()
        .any(|f| f.contains("identifiers: 1, exact allowlist count 0")));
    // Renaming also fails at the exact allowed identifier count, in its owner.
    let found = model_transport_findings(&[planted(
        "kernel/src/inference/ollama.rs",
        "use super::ModelTransport as Mt; impl ModelTransport for X {}",
    )]);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("renaming is forbidden"));

    for src in [
        "use crate::inference::ModelTransport;",
        "type T = dyn r#ModelTransport;",
    ] {
        let found = model_transport_findings(&[approved(), planted("sdk/src/new.rs", src)]);
        assert_eq!(found.len(), 1, "merely naming the trait: {src}");
        assert!(found[0].contains("exact allowlist count 0"));
    }
    for src in [
        "/// ModelTransport\n/** r#ModelTransport */",
        "// impl ModelTransport for X {}",
        "const S: &str = \"impl ModelTransport for X {}\";",
        "const S: &str = r#\"ModelTransport as Mt\"#;",
        "impl OtherModelTransport for X {} impl ModelTransportSuffix for Y {}",
    ] {
        assert!(
            model_transport_findings(&[approved(), planted("sdk/src/new.rs", src)]).is_empty(),
            "must ignore: {src}"
        );
    }
    let found = model_transport_findings(&[
        approved(),
        planted(
            "sdk/src/new.rs",
            "impl crate::ModelTransport::Other for X {}",
        ),
    ]);
    assert_eq!(
        found.len(),
        1,
        "only the final path segment decides impl identity"
    );
    assert!(found[0].contains("exact allowlist count 0"));
    // Exactly one occurrence, even in the approved implementation file.
    let found = model_transport_findings(&[planted(
        "kernel/src/inference/ollama.rs",
        "impl ModelTransport for X {} impl ModelTransport for Y {}",
    )]);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("ModelTransport implementations:"));
}

/// D1 stricter oracle amendment (R-3): old rule: "count named ModelTransport
/// implementations and exact identifier sites". New rule: "also reject macro
/// trait positions containing metavariables or ending in ModelTransport, macro
/// invocations naming ModelTransport, and inference globs outside inference".
/// Concrete macro trait paths without a metavariable remain permitted.
/// The same test-item filtering used by the workspace scan, for fixture text.
fn production_route_plant(rel: &str, src: &str) -> ProdFile {
    let mut file = planted(rel, src);
    let (ranges, _) = source::cfg_test_items(&file.code, &file.text);
    let mut code = file.code.into_bytes();
    let mut text = file.text.into_bytes();
    for (start, end) in ranges {
        for at in start..end {
            if code[at] != b'\n' {
                code[at] = b' ';
                text[at] = b' ';
            }
        }
    }
    file.code = String::from_utf8(code).unwrap();
    file.text = String::from_utf8(text).unwrap();
    file
}

#[test]
fn e_o65_macro_transport_reviewer_bypass_is_red_at_the_exact_identifier_count() {
    let real = include_str!("../src/inference/ollama.rs");
    let approved = production_route_plant("kernel/src/inference/ollama.rs", real);
    assert!(route_guard_findings(&[approved]).is_empty());
    let start = real.find("use super::{").unwrap();
    let end = start + real[start..].find("};").unwrap() + 2;
    assert!(real[start..end].contains("ModelTransport"));
    // Reviewer's exact-count bypass: glob import, parameterized trait impl,
    // and a qualified trait argument leave two ModelTransport identifiers.
    let bypass = format!(
        "{}use super::*;{}\n{}",
        &real[..start],
        &real[end..],
        r#"
struct Alternate {
    client: reqwest::blocking::Client,
    endpoint: String,
    identity: ModelIdentity,
}
macro_rules! alternate_transport {
    ($transport:path) => {
        impl $transport for Alternate {
            fn identity(&self) -> &ModelIdentity { &self.identity }
            fn call(&self, _: &ModelRequest, _: &CallContext<'_>) -> Result<ModelReply, ModelError> {
                let _ = self.client.post(&self.endpoint).send();
                Err(ModelError::ConfigMismatch)
            }
        }
    };
}
alternate_transport!(super::ModelTransport);
"#
    );
    let file = production_route_plant("kernel/src/inference/ollama.rs", &bypass);
    assert_eq!(
        route_tokens(&file.code)
            .iter()
            .filter(|&&token| token == "ModelTransport")
            .count(),
        2
    );
    let findings = route_guard_findings(&[file]);
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("macro_rules trait implementation")),
        "macro body must be rejected at the approved count: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("macro invocation names ModelTransport")),
        "macro argument must be rejected independently: {findings:?}"
    );
}

#[test]
fn e_o65_macro_and_inference_glob_rules_have_red_and_green_controls() {
    let approved = || {
        planted(
            "kernel/src/inference/ollama.rs",
            "use super::ModelTransport; impl ModelTransport for VerifiedOllama {}",
        )
    };
    for body in [
        "impl $transport for Alternate {}",
        "impl<T> $transport for Wrapper<T> {}",
        "impl $($p)::+ for Alternate {}",
        "impl $module::Trait for $ty {}",
        "impl crate::$trait_name for $ty {}",
        "impl<T> crate::Trait<$transport> for Wrapper<T> {}",
        "impl crate::ModelTransport for Alternate {}",
        "impl ::nexus_kernel::inference::r#ModelTransport for Alternate {}",
    ] {
        let src = format!("macro_rules! make_impl {{ ($transport:path) => {{ {body} }}; }}");
        let findings =
            model_transport_findings(&[approved(), planted("kernel/src/ordinary.rs", &src)]);
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("macro_rules trait implementation")),
            "must fail closed on {src}: {findings:?}"
        );
    }
    for src in [
        "inspect!(super::ModelTransport);",
        "crate::inspect!{r#ModelTransport}",
        "inspect!([ModelTransport]);",
    ] {
        // Balance the inventory in its existing owner so this rule is tested
        // independently of the identifier-count rule.
        let src = format!("impl ModelTransport for VerifiedOllama {{}} {src}");
        let findings = model_transport_findings(&[planted("kernel/src/inference/ollama.rs", &src)]);
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("macro invocation names ModelTransport")),
            "must reject {src}: {findings:?}"
        );
    }
    for src in [
        "use nexus_kernel::inference::*;",
        "pub use crate::inference::ollama::*;",
        "use nexus_kernel::{inference::*, coding_run::LocalOllama};",
        "use crate::inference::{self, ollama::{*}};",
        "use ::nexus_kernel::r#inference::*;",
        "use super::inference::*;",
    ] {
        let findings =
            model_transport_findings(&[approved(), planted("kernel/src/ordinary.rs", src)]);
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("inference glob import")),
            "must reject {src}: {findings:?}"
        );
    }
    for src in [
        "macro_rules! ordinary { ($name:ident) => { fn $name() {} }; } ordinary!(hello);",
        "macro_rules! inherent { ($ty:ty) => { impl $ty { fn hello() {} } }; }",
        "macro_rules! concrete { ($name:ident) => { impl fmt::Debug for $name {} impl Serialize for $name {} impl<'de> Deserialize<'de> for $name {} }; }",
        "macro_rules! concrete_generic { ($name:ident) => { impl<T> crate::Trait<T> for $name {} }; }",
        "macro_rules! body { () => { const S: &str = \"impl Trait for X {} ModelTransport\"; }; }",
        "use crate::ordinary::*; use nexus_kernel::inference::VerifiedOllama;",
        "use crate::ordinary::inference::*; use other_crate::inference::*;",
        "#[cfg(test)] macro_rules! test_transport { ($transport:path) => { impl $transport for X {} }; }",
        "#[cfg(all(test, unix))] mod test_only { use nexus_kernel::inference::*; inspect!(ModelTransport); }",
    ] {
        let findings = model_transport_findings(&[
            approved(),
            production_route_plant("mother/src/ordinary.rs", src),
        ]);
        assert!(findings.is_empty(), "ordinary or genuinely test-only code: {src}: {findings:?}");
    }
    // Real production concrete-trait macro accepted by R-3, without exemptions.
    let real = include_str!("../../crates/nexus-typed-decision/src/slot.rs");
    let start = real.find("macro_rules! redacted_debug").unwrap();
    let open = start + real[start..].find('{').unwrap();
    let end = source::close_of(real, open);
    let findings = model_transport_findings(&[
        approved(),
        planted("crates/nexus-typed-decision/src/slot.rs", &real[start..end]),
    ]);
    assert!(
        findings.is_empty(),
        "real concrete Debug macro: {findings:?}"
    );
    // A glob within the inference authority does not import identifiers into
    // another production module and is allowed by the new glob rule.
    let findings = model_transport_findings(&[planted(
        "kernel/src/inference/ollama.rs",
        "use super::*; use super::ModelTransport; impl ModelTransport for VerifiedOllama {}",
    )]);
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn e_o65_inference_globs_respect_inline_module_scope() {
    let approved = || {
        planted(
            "kernel/src/inference/ollama.rs",
            "use super::ModelTransport; impl ModelTransport for VerifiedOllama {}",
        )
    };
    // The rule is unchanged: "inference globs outside inference are findings".
    // Resolve inline module scopes so ordinary modules named inference remain
    // green, while relative imports of the kernel authority remain red.
    for (rel, src) in [
        (
            "kernel/src/ordinary.rs",
            "mod inference { pub struct Ordinary; } mod child { use super::inference::*; }",
        ),
        (
            "kernel/src/lib.rs",
            "mod ordinary { mod inference {} mod child { use super::inference::*; } }",
        ),
        (
            "kernel/src/ordinary.rs",
            "mod wrapper { mod inference {} use self::inference::*; }",
        ),
    ] {
        let found = model_transport_findings(&[approved(), planted(rel, src)]);
        assert!(
            found.is_empty(),
            "ordinary inline module: {rel}: {src}: {found:?}"
        );
    }
    for (rel, src) in [
        (
            "kernel/src/lib.rs",
            "mod child { use super::inference::*; }",
        ),
        (
            "kernel/src/ordinary.rs",
            "mod child { use super::super::inference::*; }",
        ),
        (
            "kernel/src/ordinary.rs",
            "mod child { use crate::inference::*; }",
        ),
    ] {
        let found = model_transport_findings(&[approved(), planted(rel, src)]);
        assert!(
            found
                .iter()
                .any(|finding| finding.contains("inference glob import")),
            "kernel inference import: {rel}: {src}: {found:?}"
        );
    }
}

#[test]
fn e_o65_model_transport_allowlist_matches_masked_production_counts() {
    let files = production_files();
    assert!(model_transport_findings(&files).is_empty());
    for (rel, expected) in MODEL_TRANSPORT_ALLOWLIST {
        let file = files.iter().find(|file| file.rel == rel).unwrap();
        let count = route_tokens(&file.code)
            .iter()
            .filter(|&&t| t == "ModelTransport")
            .count();
        assert_eq!(count, expected, "{rel}");
        println!("ModelTransport production inventory: {rel}: {count}");
        // Each real allowlisted file is green at its exact count and red when
        // that count decreases or increases. No doc-comment allowance is used.
        for code in [
            file.code.replace("ModelTransport", "OtherTransport"),
            format!("{} use crate::ModelTransport;", file.code),
        ] {
            let found = model_transport_findings(&[planted(rel, &code)]);
            assert!(found.iter().any(|f| f.contains("exact allowlist count")));
        }
    }
}

#[test]
fn e_o15_scanner_fails_closed_on_protected_identifiers() {
    let findings = |src: &str| predicate_findings(&[planted("sdk/src/local_model.rs", src)]);
    for predicate in [
        "name_is_cloud",
        "show_names_remote",
        "ollama_model_name_is_cloud",
        "ollama_show_names_remote",
    ] {
        for spelling in [predicate.to_string(), format!("r#{predicate}")] {
            let local = findings(&format!("pub fn\n {spelling} () -> bool {{ false }}"));
            assert_eq!(local.len(), 1, "definition of {spelling}");
            assert!(local[0].contains("defines its own"));
            for code in [
                format!("define_predicate!({spelling});"),
                format!("impl Local {{ fn {spelling}(&self) -> bool {{ false }} }}"),
                format!("fn caller() {{ nexus_kernel::inference::ollama::{spelling}(\"x\"); }}"),
                format!("use nexus_kernel::inference::ollama::{spelling};"),
            ] {
                assert_eq!(findings(&code).len(), 1, "must reject {code}");
            }
        }
    }
    // Reviewer's parameterized macro: the protected name is only an argument.
    let generated = r#"
macro_rules! define_predicate {
    ($name:ident) => { pub fn $name(_: &str) -> bool { false } };
}
define_predicate!(ollama_model_name_is_cloud);
"#;
    assert_eq!(findings(generated).len(), 1);
    assert_eq!(
        findings("macro_rules! define_predicate { () => { pub fn show_names_remote() {} }; }")
            .len(),
        1
    );
    // Imports/re-exports and calls are not exempt: today's allowlist is empty.
    assert_eq!(
        findings("pub use nexus_kernel::inference::ollama::{name_is_cloud as ollama_model_name_is_cloud, show_names_remote as ollama_show_names_remote};").len(),
        4
    );
    for ignored in [
        "// name_is_cloud\n/* r#show_names_remote */",
        "const TEXT: &str = \"ollama_model_name_is_cloud\";",
        "const TEXT: &str = r#\"ollama_show_names_remote\"#;",
        "const C: char = 'n';",
        "fn name_is_cloud_suffix() {} fn prefix_show_names_remote() {}",
    ] {
        assert!(findings(ignored).is_empty(), "must ignore {ignored}");
    }
    let owner = [planted("kernel/src/inference/ollama.rs", generated)];
    assert!(predicate_findings(&owner).is_empty());
}

#[test]
fn e_o15_scanner_rejects_renamed_predicate_copies_and_exact_literals() {
    let findings = |src: &str| predicate_findings(&[planted("sdk/src/local_model.rs", src)]);
    // Reviewer's renamed function, closure and trait-default copies.
    let renamed = r#"
pub fn rejects_cloud_model(model: &str) -> bool {
    model.to_ascii_lowercase().split([':', '-', '/', '.', '_']).any(|part| part == "cloud")
}
"#;
    let closure = r#"
fn caller(model: &str) -> bool {
    let rejects = |model: &str| model.to_ascii_lowercase().split([':', '-', '/', '.', '_']).any(|part| part == "cloud");
    rejects(model)
}
"#;
    let trait_default = r#"
trait LocalModel {
    fn rejects_cloud_model(&self, model: &str) -> bool {
        model.to_ascii_lowercase().split([':', '-', '/', '.', '_']).any(|part| part == "cloud")
    }
}
"#;
    for src in [renamed, closure, trait_default] {
        let found = findings(src);
        assert_eq!(found.len(), 1, "must reject {src}");
        assert!(found[0].contains("predicate literal \"cloud\""));
    }
    let remote_copy = r#"
fn description_is_remote(show: &serde_json::Value) -> bool {
    ["remote_host", "remote_model"].iter().any(|field| {
        show.get(field).is_some_and(|value| {
            !value.is_null() && value.as_str().is_none_or(|text| !text.trim().is_empty())
        })
    })
}
"#;
    let remote = findings(remote_copy);
    assert_eq!(remote.len(), 2);
    for literal in ["remote_host", "remote_model"] {
        assert!(remote.iter().any(|finding| finding.contains(literal)));
    }
    // Exact contents in ordinary, raw, byte and raw-byte strings all count.
    for literal in PREDICATE_LITERALS {
        for spelling in [
            format!("\"{literal}\""),
            format!("r#\"{literal}\"#"),
            format!("b\"{literal}\""),
            format!("br##\"{literal}\"##"),
        ] {
            assert_eq!(findings(&format!("const S = {spelling};")).len(), 1);
        }
    }
    for src in [
        r#"// "cloud"
/* "remote_host" /* "remote_model" */ */"#,
        r#"const S: &str = "cloudy"; const T: &str = "remote_hosts";"#,
        r###"const S: &str = r##"contains "cloud" and "remote_host""##;"###,
        r#"const S: &str = "contains \"cloud\""; const C: char = '"';"#,
    ] {
        assert!(findings(src).is_empty(), "must ignore {src}");
    }
    for src in [renamed, closure, trait_default, remote_copy] {
        assert!(predicate_findings(&[planted("kernel/src/inference/ollama.rs", src)]).is_empty());
    }

    let tts = r#"match provider { "openai" | "cloud" => run_openai(), _ => refuse() }"#;
    let allowlisted =
        |src: &str| predicate_findings(&[planted("kernel/src/actuators/tts.rs", src)]);
    assert!(allowlisted(tts).is_empty());
    assert_eq!(findings(tts).len(), 1, "exemption is exact-file only");
    assert_eq!(allowlisted("").len(), 1, "exemption requires exact count");
    assert_eq!(allowlisted(&format!("{tts} {tts}")).len(), 1);
    assert_eq!(
        allowlisted(&format!("{tts} const S = \"remote_model\";")).len(),
        1
    );
    assert_eq!(allowlisted(&format!("{tts} name_is_cloud();")).len(), 1);
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
fn e_o15_local_model_predicates_are_defined_only_in_kernel_inference() {
    let files = production_files();
    let findings = predicate_findings(&files);
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

/// Mother Nexus cleanup (2026-11-05): two absence guards keep the retired
/// `connectors/llm` provider/gateway stack and its workspace membership gone.
#[test]
fn m2e_a2_retired_llm_connectors_directory_is_absent() {
    let path = source::workspace_root().join("connectors/llm");
    assert!(
        matches!(
            std::fs::symlink_metadata(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ),
        "the retired connectors/llm path must remain absent"
    );
}

#[test]
fn m2e_a2_retired_llm_connectors_are_not_a_workspace_member() {
    let manifest = std::fs::read_to_string(source::workspace_root().join("Cargo.toml")).unwrap();
    let manifest: toml::Value = toml::from_str(&manifest).unwrap();
    let members = manifest["workspace"]["members"]
        .as_array()
        .expect("workspace members array");
    assert!(
        members.iter().all(|member| {
            std::path::Path::new(member.as_str().expect("workspace member path"))
                != std::path::Path::new("connectors/llm")
        }),
        "the retired connectors/llm must not be a workspace member"
    );
}
