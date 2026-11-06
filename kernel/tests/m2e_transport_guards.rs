//! M2e-A1 E-O14 (design v2 §14, fact F17): the Ollama API paths are named
//! only by the kernel's local model transport and the classified
//! providers.
//!
//! A-S4 (`hm_m3_guards.rs`) matches `"/api/chat"` with its leading slash,
//! so the coding-run transport's `join("api/chat")` escaped it (F17). This
//! guard matches the paths `api/chat`, `api/generate`, `api/tags`,
//! `api/show`, `api/version` and `api/status`, with or without a leading
//! `/`, inside any string literal of the workspace's production source
//! (the N5 guards' view: test code and comments excluded). Each site must
//! be:
//!
//! - in `kernel/src/inference/ollama.rs`, which names each path exactly
//!   once (its `ApiPath`); or
//! - a classified provider (A-S4's class T transport, and the classified
//!   vision model of `computer_control.rs`).
//!
//! M2e-A2 replaced the coding-run transport's two sites with the kernel
//! transport and removed them, with their `PENDING_A2` entries: any path in
//! `kernel/src/coding_run/` is now unclassified.
//!
//! It is a text guard, not a Rust parser; its scanner first proves on
//! planted text that it flags what it must.

#[allow(dead_code)]
#[path = "hm_m3_guards/source.rs"]
mod source;

use source::{close_of, enclosing_fn, mask, production_files, ProdFile};
use std::collections::BTreeMap;

/// The Ollama API paths the guard looks for.
const PATHS: [&str; 6] = ["chat", "generate", "tags", "show", "version", "status"];

/// The kernel transport, which names each path of its `ApiPath` once.
const TRANSPORT: &str = "kernel/src/inference/ollama.rs";
const TRANSPORT_PATHS: [&str; 5] = [
    "api/chat",
    "api/show",
    "api/status",
    "api/tags",
    "api/version",
];

/// Classified providers: `(file or directory, fn or "*", why)`.
/// Mother Nexus cleanup (2026-11-05): the retired
/// (`connectors/llm/src/providers/`, `*`,
/// `A-S4 class T: the provider transport behind LlmProvider`) row was removed.
/// Two rows remain; every production API-path site retains the same scan.
const CLASSIFIED: [(&str, &str, &str); 2] = [
    (
        "kernel/src/computer_control.rs",
        "query_vision_model",
        "A-S4 04 K10: the classified vision model call",
    ),
    (
        "kernel/src/computer_control.rs",
        "list_ollama_models",
        "04 K10's model listing (names only, no prompt)",
    ),
];

/// The API path sites of one file, as `(enclosing fn, "api/<path>")`: an
/// occurrence inside a string literal, not followed by an identifier
/// character or a `.` (Slack's `api/chat.postMessage` is no model path).
fn api_paths(file: &ProdFile) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (text, code) = (&file.text, &file.code);
    let mut from = 0;
    while let Some(p) = text[from..].find("api/") {
        let at = from + p;
        from = at + 4;
        // Inside a string literal: the code view is blank there.
        if !code[at..at + 4].trim().is_empty() {
            continue;
        }
        for path in PATHS {
            let end = at + 4 + path.len();
            if !text[at + 4..].starts_with(path) {
                continue;
            }
            let next = text[end..].chars().next();
            if next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            out.push((enclosing_fn(code, at), format!("api/{path}")));
        }
    }
    out
}

/// The workspace inventory: `(file, fn, path) -> count`.
fn inventory(files: &[ProdFile]) -> BTreeMap<(String, String, String), usize> {
    let mut out = BTreeMap::new();
    for file in files {
        for (function, path) in api_paths(file) {
            *out.entry((file.rel.clone(), function, path)).or_insert(0) += 1;
        }
    }
    out
}

fn classified(file: &str, function: &str) -> bool {
    CLASSIFIED.iter().any(|(f, g, _)| {
        (*f == file || (f.ends_with('/') && file.starts_with(f))) && (*g == "*" || *g == function)
    })
}

fn planted(rel: &str, src: &str) -> ProdFile {
    ProdFile {
        rel: rel.into(),
        code: mask(src, false),
        text: mask(src, true),
    }
}

#[test]
#[ignore = "inventory print for the E-O14 allowlist"]
fn print_api_path_inventory() {
    for ((file, function, path), n) in inventory(&production_files()) {
        println!("{file} | {function} | {path} | {n}");
    }
}

#[test]
fn m2e_e_o14_scanner_flags_planted_paths_with_or_without_a_slash() {
    let src = r##"
fn joined(base: &Url) { let url = base.join("api/chat"); }
fn slashed(base: &str) { let url = format!("{base}/api/tags"); }
fn raw() { let r = r#"api/version"#; }
fn slack() { let s = "https://slack.com/api/chat.postMessage"; }
fn comment() { /* "api/show" */ let t = 1; }
// "api/status"
fn longer() { let s = "api/statuses"; let g = "api/generate?x=1"; }
fn not_literal() { api/status; }
"##;
    let found = api_paths(&planted("planted.rs", src));
    assert_eq!(
        found,
        [
            ("joined", "api/chat"),
            ("slashed", "api/tags"),
            ("raw", "api/version"),
            ("longer", "api/generate"),
        ]
        .map(|(f, p)| (f.to_string(), p.to_string()))
    );
    // A planted production file outside the allowlist is flagged.
    let files = [planted(
        "kernel/src/planted.rs",
        "fn sneak(base: &Url) { base.join(\"api/show\"); }",
    )];
    let inventory = inventory(&files);
    assert!(inventory
        .keys()
        .all(|(file, function, _)| !classified(file, function) && file != TRANSPORT));
    assert_eq!(inventory.len(), 1);
}

#[test]
fn m2e_e_o14_ollama_api_paths_appear_only_in_the_transport_and_classified_providers() {
    let mut inventory = inventory(&production_files());

    // The kernel transport names each of its paths exactly once.
    let transport: Vec<(String, usize)> = inventory
        .iter()
        .filter(|((file, _, _), _)| file == TRANSPORT)
        .map(|((_, _, path), n)| (path.clone(), *n))
        .collect();
    assert_eq!(
        transport,
        TRANSPORT_PATHS.map(|path| (path.to_string(), 1)),
        "{TRANSPORT}"
    );
    inventory.retain(|(file, _, _), _| file != TRANSPORT);

    let unclassified: Vec<String> = inventory
        .iter()
        .filter(|((file, function, _), _)| !classified(file, function))
        .map(|((file, function, path), n)| format!("{file} | {function} | {path} | {n}"))
        .collect();
    assert!(
        unclassified.is_empty(),
        "Ollama API paths outside the transport and the classified providers:\n{}",
        unclassified.join("\n")
    );
}

// ── SB1-S1: the admission hook sits between construction and send ───────────

/// The kernel transport's production source.
const INFERENCE: &str = "kernel/src/inference/";
/// The file that maps the request kind and calls the hook.
const EXCHANGE: &str = "kernel/src/inference/exchange.rs";

/// SB1-S1 (supplementary to the behavioural oracles): the production
/// transport has exactly one `admit_request(` call, inside `async fn
/// request`, after the client's `.build()` and before both `dispatched(` and
/// `.send()`; the `RequestKind` mapping has no `_` arm. Empty = conforms.
fn sb1_hook_findings(files: &[ProdFile]) -> Vec<String> {
    let mut findings = Vec::new();
    let mut calls = Vec::new();
    for file in files.iter().filter(|file| file.rel.starts_with(INFERENCE)) {
        let code = &file.code;
        let mut from = 0;
        while let Some(p) = code[from..].find("admit_request(") {
            let at = from + p;
            from = at + 1;
            let before = code[..at].chars().next_back();
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            // A definition, not a call.
            if code[..at].trim_end().ends_with("fn") {
                continue;
            }
            calls.push((file, at));
        }
    }
    if calls.len() != 1 {
        findings.push(format!(
            "{} admit_request( calls, want exactly 1",
            calls.len()
        ));
    }
    for (file, at) in &calls {
        let code = &file.code;
        let Some(start) = code.find("async fn request(") else {
            findings.push(format!("{}: no async fn request", file.rel));
            continue;
        };
        let open = start + code[start..].find('{').expect("a body");
        let end = close_of(code, open);
        if !(open..end).contains(at) || enclosing_fn(code, *at) != "request" {
            findings.push(format!(
                "{}: the call is outside async fn request",
                file.rel
            ));
            continue;
        }
        let body = &code[open..end];
        let at = at - open;
        let order = |needle: &str, before_call: bool| match body.find(needle) {
            Some(p) if (p < at) == before_call => None,
            Some(_) if before_call => Some(format!("{needle} is not before the hook")),
            Some(_) => Some(format!("{needle} is not after the hook")),
            None => Some(format!("no {needle} in async fn request")),
        };
        findings.extend(order(".build()", true));
        findings.extend(order("dispatched(", false));
        findings.extend(order(".send()", false));
    }
    match files.iter().find(|file| file.rel == EXCHANGE) {
        None => findings.push(format!("{EXCHANGE} is not scanned")),
        Some(file) => findings.extend(kind_mapping_findings(&file.code)),
    }
    findings
}

/// The `match` that yields `RequestKind::Chat` names every API path and has
/// no `_` arm.
fn kind_mapping_findings(code: &str) -> Vec<String> {
    let Some(chat) = code.find("RequestKind::Chat") else {
        return vec!["no RequestKind mapping".to_string()];
    };
    let Some(start) = code[..chat].rfind("match ") else {
        return vec!["the RequestKind mapping is not a match".to_string()];
    };
    let open = start + code[start..].find('{').expect("a match body");
    let block = &code[open..close_of(code, open)];
    let mut findings = Vec::new();
    let bytes = block.as_bytes();
    for (k, _) in block.match_indices('_') {
        let ident = |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
        if !ident(k.checked_sub(1).and_then(|j| bytes.get(j)))
            && !ident(bytes.get(k + 1))
            && block[k + 1..].trim_start().starts_with("=>")
        {
            findings.push("the RequestKind mapping has a _ arm".to_string());
        }
    }
    for path in ["Version", "Status", "Tags", "Show", "Chat"] {
        if !block.contains(&format!("ApiPath::{path}")) {
            findings.push(format!(
                "the RequestKind mapping does not name ApiPath::{path}"
            ));
        }
    }
    findings
}

const SB1_TRAIT: &str = r#"
pub trait CallCancel: Send + Sync {
    fn admit_request(&self, _kind: RequestKind) -> Result<(), ModelError> { Ok(()) }
}
"#;

const SB1_GREEN: &str = r#"
fn exchange(path: ApiPath) {
    let kind = match path {
        ApiPath::Chat => RequestKind::Chat,
        ApiPath::Version | ApiPath::Status | ApiPath::Tags | ApiPath::Show => RequestKind::Metadata,
    };
}
async fn request(admit: &dyn CallCancel, kind: RequestKind) -> Result<(), ModelError> {
    let client = builder().build()?;
    let request = client.get(url);
    admit.admit_request(kind)?;
    observer.dispatched(ms);
    let response = request.send().await?;
    Ok(())
}
"#;

const SB1_HOOK: &str = "    admit.admit_request(kind)?;\n";

fn sb1_planted(exchange: &str) -> Vec<ProdFile> {
    vec![
        planted("kernel/src/inference/mod.rs", SB1_TRAIT),
        planted(EXCHANGE, exchange),
    ]
}

#[test]
fn sb1_hook_is_called_once_between_construction_and_send() {
    // Green control.
    assert_eq!(
        sb1_hook_findings(&sb1_planted(SB1_GREEN)),
        Vec::<String>::new()
    );
    // Planted red controls: missing, before build, after send, wildcard.
    let missing = SB1_GREEN.replace(SB1_HOOK, "");
    let before_build = missing.replace(
        "    let client = builder()",
        &format!("{SB1_HOOK}    let client = builder()"),
    );
    let after_send = missing.replace("    Ok(())\n}", &format!("{SB1_HOOK}    Ok(())\n}}"));
    let wildcard = SB1_GREEN.replace(
        "ApiPath::Version | ApiPath::Status | ApiPath::Tags | ApiPath::Show =>",
        "ApiPath::Version | ApiPath::Status | ApiPath::Tags | ApiPath::Show | _ =>",
    );
    for (name, planted) in [
        ("missing", missing),
        ("before build", before_build),
        ("after send", after_send),
        ("wildcard", wildcard),
    ] {
        assert_ne!(planted, SB1_GREEN, "{name}: the plant changed the source");
        assert!(
            !sb1_hook_findings(&sb1_planted(&planted)).is_empty(),
            "{name}: the planted red control was not flagged"
        );
    }
    // The production transport.
    let findings = sb1_hook_findings(&production_files());
    assert!(findings.is_empty(), "SB1-S1:\n{}", findings.join("\n"));
}
