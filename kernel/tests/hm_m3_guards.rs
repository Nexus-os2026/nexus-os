//! P3-C11-HM-M3 N5: workspace source guards (P3-HM-CONTRACT-02 §18.2
//! A-S1, A-S2, A-S3, A-S4, A-S6; mission R7; AMENDMENT 2 A2.4 I4;
//! AMENDMENT 1 A1.10; AMENDMENT 3 A3.1 errata E-M3-1 and A3.2). Each guard
//! scans the workspace's production source (`source::production_files`)
//! and first proves, on planted text, that its scanner flags what it must.

#[path = "hm_m3_guards/source.rs"]
mod source;

use source::{close_of, enclosing_fn, mask, production_files, ProdFile};
use std::collections::BTreeMap;

// ── A-S4: every production model call is classified (§12.1, §12.2, 04) ────

/// The model-call shapes of A-S4: the call names and the known HTTP model
/// endpoints.
const CALLS: [&str; 7] = [
    "query",
    "query_with_origin",
    "query_with_image",
    "query_streaming",
    "chat_stream",
    "embed",
    "plan_query",
];

const ENDPOINTS: [&str; 14] = [
    "/api/generate",
    "/api/chat",
    "/api/embed",
    "/v1/chat/completions",
    "/chat/completions",
    "/v1/completions",
    "/v1/messages",
    "/v1/embeddings",
    "generativelanguage.googleapis.com",
    ":generateContent",
    "api.anthropic.com",
    "api.openai.com",
    "api.groq.com",
    "openrouter.ai/api",
];

/// The number of top-level arguments of the call whose `(` is at `open`.
fn arg_count(code: &str, open: usize) -> usize {
    let end = close_of(code, open);
    let inner = &code[open + 1..end - 1];
    if inner.trim().is_empty() {
        return 0;
    }
    let b = inner.as_bytes();
    let (mut depth, mut args) = (0i64, 1);
    for (k, &c) in b.iter().enumerate() {
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'<' if k > 0 && (b[k - 1].is_ascii_alphanumeric() || b[k - 1] == b':') => depth += 1,
            b'>' if depth > 0 && k > 0 && b[k - 1] != b'-' && b[k - 1] != b'=' => depth -= 1,
            b',' if depth == 0 => args += 1,
            _ => {}
        }
    }
    if inner.trim_end().ends_with(',') {
        args -= 1;
    }
    args
}

/// The model-call sites of one file, as `(enclosing fn, shape)`: a call of
/// one of [`CALLS`] (`query` with three or more arguments: a provider's or
/// the governed gateway's, not a database's or a URL's), or a string
/// literal naming one of [`ENDPOINTS`].
fn model_calls(file: &ProdFile) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let code = &file.code;
    let b = code.as_bytes();
    for call in CALLS {
        let needle = format!(".{call}");
        let mut from = 0;
        while let Some(p) = code[from..].find(&needle) {
            let at = from + p;
            from = at + needle.len();
            let mut k = at + needle.len();
            if k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'_') {
                continue;
            }
            if code[k..].starts_with("::<") {
                k = close_of(code, k + 2);
            }
            while k < b.len() && b[k].is_ascii_whitespace() {
                k += 1;
            }
            if b.get(k) != Some(&b'(') {
                continue;
            }
            if call == "query" && arg_count(code, k) < 3 {
                continue;
            }
            out.push((enclosing_fn(code, at), call.to_string()));
        }
    }
    let text = &file.text;
    for endpoint in ENDPOINTS {
        let mut from = 0;
        while let Some(p) = text[from..].find(endpoint) {
            let at = from + p;
            from = at + endpoint.len();
            // Inside a string literal: the code view is blank there.
            if !code[at..at + endpoint.len()].trim().is_empty() {
                continue;
            }
            // Slack's `chat.postMessage` is not a model endpoint.
            if endpoint == "/api/chat" && text[from..].starts_with('.') {
                continue;
            }
            out.push((enclosing_fn(code, at), format!("endpoint {endpoint}")));
        }
    }
    out
}

/// The workspace inventory: `(file, fn, shape) -> count`.
fn model_call_inventory(files: &[ProdFile]) -> BTreeMap<(String, String, String), usize> {
    let mut inventory = BTreeMap::new();
    for file in files {
        for (function, shape) in model_calls(file) {
            *inventory
                .entry((file.rel.clone(), function, shape))
                .or_insert(0) += 1;
        }
    }
    inventory
}

#[test]
#[ignore = "inventory print for the N5 allowlist"]
fn print_model_call_inventory() {
    let files = production_files();
    for ((file, function, shape), n) in model_call_inventory(&files) {
        let class = class_of(&file, &function).unwrap_or("UNCLASSIFIED");
        println!("{class} | {file} | {function} | {shape} | {n}");
    }
}

#[test]
fn hm_m3_n5_source_model_planted_controls() {
    let text = r##"
fn a() { let s = "x.query(p, 1, m)"; /* p.query(a, b, c) */ }
// q.query(a, b, c)
fn real() { provider.query(prompt, 64, &model); db.query(params); }
#[cfg(test)]
mod tests { fn t() { provider.query(a, b, c); } }
#[cfg(test)]
fn helper() { gateway.query(&mut c, p, 1, m); }
fn ep() { let u = format!("{base}/api/generate"); let s = "slack/api/chat.postMessage"; }
fn raw() { let r = r#"provider.query(a, b, c)"#; let c = '{'; }
fn arms(s: S) -> u8 { match s { S::A => 1,
    #[cfg(test)]
    S::B => { x.query(a, b, c); 2 },
    #[cfg(test)]
    S::C => y.query(a, b, c),
} }
fn after() { provider.query(p, 1, m); }
"##;
    let code = mask(text, false);
    let mut masked_text = mask(text, true).into_bytes();
    let mut code_bytes = code.clone().into_bytes();
    let (ranges, _) = source::cfg_test_items(&code, &mask(text, true));
    assert_eq!(ranges.len(), 4, "four cfg(test) items");
    for (a, z) in ranges {
        for k in a..z {
            if code_bytes[k] != b'\n' {
                code_bytes[k] = b' ';
                masked_text[k] = b' ';
            }
        }
    }
    let file = ProdFile {
        rel: "planted.rs".into(),
        code: String::from_utf8(code_bytes).unwrap(),
        text: String::from_utf8(masked_text).unwrap(),
    };
    let found = model_calls(&file);
    assert_eq!(
        found,
        vec![
            ("real".to_string(), "query".to_string()),
            ("after".to_string(), "query".to_string()),
            ("ep".to_string(), "endpoint /api/generate".to_string()),
        ]
    );
}

// ── shared scanning helpers ─────────────────────────────────────────────────

/// The production file `rel`.
fn file<'a>(files: &'a [ProdFile], rel: &str) -> &'a ProdFile {
    files
        .iter()
        .find(|f| f.rel == rel)
        .unwrap_or_else(|| panic!("{rel}: not a production file"))
}

/// Whole-word occurrences of `word` in `text` (identifier characters as
/// boundaries), as byte offsets.
fn words(text: &str, word: &str) -> Vec<usize> {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = text[from..].find(word) {
        let at = from + p;
        from = at + word.len();
        let before = text[..at].chars().next_back();
        let after = text[from..].chars().next();
        if !before.is_some_and(ident) && !after.is_some_and(ident) {
            out.push(at);
        }
    }
    out
}

/// Calls of `.name(` / `::name(` / ` name(` (not definitions) in `code`.
fn calls_of(code: &str, name: &str) -> Vec<usize> {
    words(code, name)
        .into_iter()
        .filter(|&at| {
            let after = code[at + name.len()..].trim_start();
            let before = code[..at].trim_end();
            (after.starts_with('(') || after.starts_with("::<")) && !before.ends_with("fn")
        })
        .collect()
}

/// `(file, enclosing fn)` of every call of `name` in `files`.
fn call_sites(files: &[ProdFile], name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in files {
        for at in calls_of(&f.code, name) {
            out.push((f.rel.clone(), enclosing_fn(&f.code, at)));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `(file, enclosing fn)` of every call of `name` with `arity` arguments.
fn call_sites_with_arity(files: &[ProdFile], name: &str, arity: usize) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in files {
        for at in calls_of(&f.code, name) {
            let mut k = at + name.len();
            if f.code[k..].starts_with("::<") {
                k = close_of(&f.code, k + 2);
            }
            let open = k + f.code[k..].find('(').unwrap();
            if arg_count(&f.code, open) == arity {
                out.push((f.rel.clone(), enclosing_fn(&f.code, at)));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `text` lower-cased with whitespace runs collapsed to one space.
fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

const HM_TABLES: [&str; 10] = [
    "hm_migration",
    "hm_t0_member",
    "hm_t0_tombstone",
    "hm_agent_policy",
    "hm_policy_change",
    "hm_record_label",
    "hm_derivation",
    "hm_grant",
    "hm_dispatch_admission",
    "hm_event",
];

const IN_SCOPE_TABLES: [&str; 5] = [
    "agent_memory",
    "task_history",
    "strategy_scores",
    "world_model_entities",
    "world_model_relationships",
];

fn is_hm_module(rel: &str) -> bool {
    rel == "kernel/src/hm.rs" || rel.starts_with("kernel/src/hm/") || rel == "persistence/src/hm.rs"
}

// ── A-S1: no IPC writes hm_*; no REPLACE; foreign keys on ────────────────────

/// The desktop's production references to the historical-memory tables
/// and store handle: `(file, what)`.
fn desktop_hm_store_refs(files: &[ProdFile]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in files
        .iter()
        .filter(|f| f.rel.starts_with("app/src-tauri/src/"))
    {
        for t in HM_TABLES {
            if !words(&f.text, t).is_empty() {
                out.push((f.rel.clone(), t.to_string()));
            }
        }
        for handle in [".hm()", "HmStore", "HmTx", "persistence::hm"] {
            if f.code.contains(handle) {
                out.push((f.rel.clone(), handle.to_string()));
            }
        }
    }
    out
}

/// REPLACE writes (`REPLACE`, `INSERT OR REPLACE` and `UPDATE OR REPLACE`
/// statements) to an hm_* or in-scope table: `(file, table)`.
fn replace_writes(files: &[ProdFile]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in files {
        let s = squash(&f.text);
        for t in HM_TABLES.iter().chain(IN_SCOPE_TABLES.iter()) {
            // Split literals: the persistence REPLACE guards scan this file.
            for prefix in [
                concat!("re", "place into "),
                concat!("update or re", "place "),
            ] {
                for qualifier in ["", "main."] {
                    if !words(&s, &format!("{prefix}{qualifier}{t}")).is_empty()
                        || s.contains(&format!("{prefix}{qualifier}{t} "))
                        || s.contains(&format!("{prefix}{qualifier}{t}("))
                    {
                        out.push((f.rel.clone(), t.to_string()));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn hm_m3_n5_a_s1_planted_controls() {
    let planted = |rel: &str, src: &str| ProdFile {
        rel: rel.into(),
        code: mask(src, false),
        text: mask(src, true),
    };
    let ipc = [planted(
        "app/src-tauri/src/x.rs",
        "fn c(db: &D) { db.hm().immediate(|tx| Ok(())); let q = \"DELETE FROM hm_grant\"; }",
    )];
    assert_eq!(
        desktop_hm_store_refs(&ipc),
        vec![
            ("app/src-tauri/src/x.rs".to_string(), "hm_grant".to_string()),
            ("app/src-tauri/src/x.rs".to_string(), ".hm()".to_string()),
        ]
    );
    let replace = [planted(
        "persistence/src/x.rs",
        concat!(
            "fn w() { let a = \"INSERT OR RE",
            "PLACE INTO task_history (id) VALUES (1)\"; let b = \"re",
            "place into main.hm_event(x) values(1)\"; }"
        ),
    )];
    assert_eq!(
        replace_writes(&replace),
        vec![
            ("persistence/src/x.rs".to_string(), "hm_event".to_string()),
            (
                "persistence/src/x.rs".to_string(),
                "task_history".to_string()
            ),
        ]
    );
    let other = [planted(
        "persistence/src/x.rs",
        concat!(
            "fn w() { let a = \"INSERT OR RE",
            "PLACE INTO idempotency_cache (k) VALUES (1)\"; }"
        ),
    )];
    assert!(replace_writes(&other).is_empty());
}

#[test]
fn hm_m3_n5_a_s1_no_ipc_writes_hm_no_replace_and_foreign_keys_on() {
    let files = production_files();
    // (a) The desktop names no hm_* table and never reaches the store; its
    // historical-memory state changes only through the kernel's
    // `nexus_kernel::hm` (labelled writes, §4.2 resolution); the start-up
    // migration and back-fill run only in `start_historical_memory`.
    assert_eq!(
        desktop_hm_store_refs(&files),
        Vec::<(String, String)>::new()
    );
    for authority_writer in [
        "run_t0_option_a",
        "run_t0_option_a_probed",
        "run_t0_option_a_at_start",
        "backfill_authority_events",
    ] {
        let outside: Vec<(String, String)> = call_sites(&files, authority_writer)
            .into_iter()
            .filter(|(rel, _)| !is_hm_module(rel))
            .collect();
        let allowed: Vec<(String, String)> = outside
            .iter()
            .filter(|(rel, f)| {
                rel == "app/src-tauri/src/hm_wiring.rs" && f == "start_historical_memory"
            })
            .cloned()
            .collect();
        assert_eq!(outside, allowed, "{authority_writer}");
    }
    // A-G0 (AD-4 disabled): nothing in production inserts a grant row, and
    // the store has no grant-insert method.
    for f in &files {
        let s = squash(&f.text);
        assert!(!s.contains("into hm_grant"), "{}: grant insert", f.rel);
        assert!(!s.contains("into main.hm_grant"), "{}: grant insert", f.rel);
    }
    let hm_store = file(&files, "persistence/src/hm.rs");
    assert!(words(&hm_store.code, "insert_grant").is_empty());
    // (b) No REPLACE write to an hm_* or in-scope table (F-A1-1, §5.1).
    assert_eq!(replace_writes(&files), Vec::<(String, String)>::new());
    // (c) Both `NexusDatabase` constructors enable foreign keys before they
    // migrate, and they are the only raw SQLite opens of the persistence
    // crate's lib.rs.
    let lib = &file(&files, "persistence/src/lib.rs").text;
    for sig in [
        "pub fn open(path: &Path) -> Result<Self> {",
        "pub fn in_memory() -> Result<Self> {",
    ] {
        let start = lib.find(sig).unwrap_or_else(|| panic!("{sig}"));
        let end = start + lib[start..].find("db.migrate()").expect("migrate");
        let body = squash(&lib[start..end]).replace(' ', "");
        assert!(body.contains("pragmaforeign_keys=on"), "{sig}");
    }
    let opens = [
        "Connection::open(",
        "Connection::open_in_memory(",
        "Connection::open_with_flags(",
    ]
    .iter()
    .map(|p| lib.matches(p).count())
    .sum::<usize>();
    assert_eq!(opens, 2, "lib.rs opens only in its two constructors");
}

// ── A-S2: prompt-building modules read no record content directly ───────────

/// The direct reads of in-scope record content: the stores' unfiltered
/// loaders and the content columns.
const CONTENT_READS: [&str; 10] = [
    "load_memories",
    "recall_relevant",
    "recall_by_type",
    "load_tasks_by_agent",
    "load_top_strategies",
    "load_strategy_history",
    "load_world_model_entities",
    "load_world_model_relationships",
    "scan_table",
    "current_row",
];
const CONTENT_COLUMNS: [&str; 4] = ["value_json", "result_json", "steps_json", "goal"];

/// A prompt-building module: production code outside `nexus_kernel::hm`
/// that builds a `PromptBuilder` envelope.
fn is_prompt_module(f: &ProdFile) -> bool {
    !is_hm_module(&f.rel) && f.code.contains("PromptBuilder::new(")
}

/// The direct content reads of one file: `(fn, what)`.
fn content_reads(f: &ProdFile) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for name in CONTENT_READS {
        for at in calls_of(&f.code, name) {
            out.push((enclosing_fn(&f.code, at), name.to_string()));
        }
    }
    for column in CONTENT_COLUMNS {
        // A field access `.value_json` (not a method of that name), or a
        // column read `.field("value_json")`.
        for at in words(&f.code, column) {
            let before = f.code[..at].trim_end();
            let after = f.code[at + column.len()..].trim_start();
            if column != "goal" && before.ends_with('.') && !after.starts_with('(') {
                out.push((enclosing_fn(&f.code, at), format!(".{column}")));
            }
        }
        let quoted = format!("\"{column}\"");
        let mut from = 0;
        while let Some(p) = f.text[from..].find(&quoted) {
            let at = from + p;
            from = at + quoted.len();
            if f.text[..at].trim_end().ends_with("field(") {
                out.push((enclosing_fn(&f.code, at), format!("field({column})")));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The reviewed direct reads inside prompt-building modules, each with
/// why it never feeds a prompt or is the route-first control read.
const PROMPT_MODULE_READS: [(&str, &str, &str, &str); 7] = [
    (
        "kernel/src/cognitive/loop_runtime.rs",
        "CognitiveRuntime::latest_control_json",
        "field(value_json)",
        "a ControlRecord of `control_records` (route-first, §12.3 item 4): cognitive parameters, model mapping",
    ),
    (
        "kernel/src/cognitive/evolution.rs",
        "EvolutionTracker::get_evolution_metrics",
        "recall_relevant",
        "a count for the metrics display; no content leaves (§6.6 role count by source_kind)",
    ),
    (
        "kernel/src/cognitive/evolution.rs",
        "EvolutionTracker::get_evolution_metrics",
        "load_strategy_history",
        "aggregate counters for the metrics display",
    ),
    (
        "kernel/src/cognitive/evolution.rs",
        "EvolutionTracker::get_agent_strategies",
        "load_strategy_history",
        "the strategy list for the display; no prompt",
    ),
    (
        "kernel/src/cognitive/evolution.rs",
        "EvolutionTracker::record_outcome",
        "load_top_strategies",
        "the counters of the aggregate this unit just upserted, for its composite score; the procedural write that follows has that aggregate as its parent",
    ),
    (
        "kernel/src/cognitive/evolution.rs",
        "EvolutionTracker::discover_shareable_strategies",
        "load_strategy_history",
        "strategy hashes and scores; each share is a labelled §14 rule 3 write (share_learning)",
    ),
    (
        "kernel/src/team_orchestrator.rs",
        "TeamOrchestrator::read_team_memories",
        "recall_relevant",
        "latent (§12.3 item 4): no production caller (asserted below)",
    ),
];

#[test]
fn hm_m3_n5_a_s2_planted_controls() {
    let src = "fn plan(db: &D) { let b = PromptBuilder::new(c); let m = db.load_memories(a, None, 5); b.text(&m[0].value_json); let v = row.field(\"result_json\"); }\nfn ok() { let s = \"value_json\"; }";
    let f = ProdFile {
        rel: "kernel/src/x.rs".into(),
        code: mask(src, false),
        text: mask(src, true),
    };
    assert!(is_prompt_module(&f));
    assert_eq!(
        content_reads(&f),
        vec![
            ("plan".to_string(), ".value_json".to_string()),
            ("plan".to_string(), "field(result_json)".to_string()),
            ("plan".to_string(), "load_memories".to_string()),
        ]
    );
}

#[test]
fn hm_m3_n5_a_s2_prompt_modules_read_content_only_through_the_hm_api() {
    let files = production_files();
    let mut found = Vec::new();
    for f in files.iter().filter(|f| is_prompt_module(f)) {
        for (function, what) in content_reads(f) {
            found.push((f.rel.clone(), function, what));
        }
    }
    let mut reviewed: Vec<(String, String, String)> = PROMPT_MODULE_READS
        .iter()
        .map(|(f, g, w, _)| (f.to_string(), g.to_string(), w.to_string()))
        .collect();
    reviewed.sort();
    found.sort();
    assert_eq!(
        found, reviewed,
        "direct content reads in prompt-building modules"
    );
    // The latent reader has no production caller.
    assert!(call_sites(&files, "read_team_memories").is_empty());
}

// ── A-S3: the authority's single sources ────────────────────────────────────

/// `pub fn`s whose parameters name a label, a route class or transport
/// evidence (M2 ruling 1): `(file, fn)`.
fn label_taking_pub_fns(f: &ProdFile) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for at in words(&f.code, "fn") {
        let head = f.code[..at].trim_end();
        if !head.ends_with("pub") {
            continue;
        }
        let rest = &f.code[at + 2..];
        let name: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let Some(open) = rest.find('(') else { continue };
        let open = at + 2 + open;
        let params = &f.code[open..close_of(&f.code, open)];
        if [
            "OriginLabel",
            "EvaluatedLabel",
            "RouteClass",
            "TransportEvidence",
            "RouteAttestation",
        ]
        .iter()
        .any(|t| !words(params, t).is_empty())
        {
            out.push((f.rel.clone(), name));
        }
    }
    out
}

#[test]
fn hm_m3_n5_a_s3_planted_controls() {
    let src = "impl X { pub fn admit_as(&self, l: OriginLabel) {} pub(crate) fn ok(r: RouteClass) {} }\npub fn f(e: TransportEvidence) {}\npub fn g() -> RouteClass { x }";
    let f = ProdFile {
        rel: "kernel/src/x.rs".into(),
        code: mask(src, false),
        text: mask(src, true),
    };
    assert_eq!(
        label_taking_pub_fns(&f),
        vec![
            ("kernel/src/x.rs".to_string(), "admit_as".to_string()),
            ("kernel/src/x.rs".to_string(), "f".to_string()),
        ]
    );
}

#[test]
fn hm_m3_n5_a_s3_single_issuer_attestation_admission_and_store() {
    let files = production_files();
    // One issuer: `DispatchIssuer::new` once in production, in the
    // desktop's `build_dispatch_issuer`, which both `AppState`
    // constructors call (the single issuer of the process).
    let issuer_new: Vec<(String, String)> = files
        .iter()
        .filter(|f| !is_hm_module(&f.rel))
        .flat_map(|f| {
            let mut v = Vec::new();
            let mut from = 0;
            while let Some(p) = f.code[from..].find("DispatchIssuer::new(") {
                let at = from + p;
                from = at + 1;
                v.push((f.rel.clone(), enclosing_fn(&f.code, at)));
            }
            v
        })
        .collect();
    assert_eq!(
        issuer_new,
        vec![(
            "app/src-tauri/src/hm_wiring.rs".to_string(),
            "build_dispatch_issuer".to_string()
        )]
    );
    assert_eq!(
        call_sites(&files, "build_dispatch_issuer"),
        vec![
            (
                "app/src-tauri/src/lib.rs".to_string(),
                "AppState::new".to_string()
            ),
            (
                "app/src-tauri/src/lib.rs".to_string(),
                "AppState::new_in_memory".to_string()
            ),
        ]
    );
    // Transport evidence only from `attest_route` (connectors), besides its
    // definition in `nexus_kernel::hm`.
    for f in files.iter().filter(|f| !is_hm_module(&f.rel)) {
        let evidence = f.code.matches("TransportEvidence::").count()
            + f.code.matches("from_transport_evidence(").count();
        if f.rel == "connectors/llm/src/attest.rs" {
            assert!(evidence > 0);
        } else {
            assert_eq!(evidence, 0, "{}: transport evidence", f.rel);
        }
    }
    // Admission and reply acceptance only in the dispatch module (calls of
    // the issuer's `admit(ctx, build)` and `accept_reply(ctx, admission,
    // reply)`; same-named methods of other types take other arguments).
    for (name, arity) in [("admit", 2), ("admit_non_agent", 2), ("accept_reply", 3)] {
        let sites: Vec<(String, String)> = call_sites_with_arity(&files, name, arity)
            .into_iter()
            .filter(|(rel, _)| rel != "kernel/src/hm/dispatch.rs")
            .collect();
        assert!(sites.is_empty(), "{name}: {sites:?}");
    }
    // The store handle only in the hm modules (and the persistence
    // accessor that hands it out).
    for f in &files {
        let n = [".hm()", "HmStore", "HmTx", "persistence::hm"]
            .iter()
            .map(|p| f.code.matches(p).count())
            .sum::<usize>();
        let allowed = is_hm_module(&f.rel)
            || f.rel == "persistence/src/lib.rs"
            || f.rel == "kernel/src/hm.rs";
        if !allowed {
            assert_eq!(n, 0, "{}: store handle", f.rel);
        }
    }
    // The label-input guard (M2 ruling 1), extended to every production
    // crate: only the pure algebra and the attestation's own constructor
    // take a label, route class or transport evidence.
    let mut taking = Vec::new();
    for f in &files {
        taking.extend(label_taking_pub_fns(f));
    }
    taking.sort();
    let allowed: Vec<(String, String)> = [
        ("kernel/src/hm.rs", "join"),
        ("kernel/src/hm/context.rs", "unit_taint_step"),
        ("kernel/src/hm/dispatch.rs", "from_transport_evidence"),
        ("kernel/src/hm/dispatch.rs", "dispatch"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    let unexpected: Vec<&(String, String)> =
        taking.iter().filter(|x| !allowed.contains(x)).collect();
    assert!(
        unexpected.is_empty(),
        "label-taking pub fns: {unexpected:?}"
    );
    // Both `AppState` constructors open the database, then run the start-up
    // sequence (T₀ → verify → back-fill), then build the issuer
    // (AMENDMENT 1 A1.10).
    let lib = &file(&files, "app/src-tauri/src/lib.rs").code;
    for (ctor, open) in [
        ("pub fn new(", "NexusDatabase::open("),
        ("pub fn new_in_memory(", "NexusDatabase::in_memory("),
    ] {
        let at = lib.find(ctor).unwrap_or_else(|| panic!("{ctor}"));
        let body_open = at + lib[at..].find('{').unwrap();
        let body = &lib[body_open..close_of(lib, body_open)];
        let opened = body.find(open).unwrap_or_else(|| panic!("{ctor}: {open}"));
        let started = body
            .find("start_historical_memory(")
            .unwrap_or_else(|| panic!("{ctor}: start"));
        let issued = body
            .find("build_dispatch_issuer(")
            .unwrap_or_else(|| panic!("{ctor}: issuer"));
        assert!(opened < started && started < issued, "{ctor}");
        assert_eq!(
            body.matches("start_historical_memory(").count(),
            1,
            "{ctor}"
        );
    }
}

// ── A-S6: the §12.8 no-handoff guard ────────────────────────────────────────

/// What an excluded (XS), human-direct (HD) or human-backend (HB)
/// subsystem may never reach: HM-store reads, the dream queue, the
/// auto-evolution log, the text of `ConsciousnessState`.
const NO_HANDOFF: [&str; 26] = [
    "load_memories",
    "recall_relevant",
    "recall_by_type",
    "recall_for_dispatch",
    "recall_spec",
    "control_records",
    "load_tasks_by_agent",
    "load_top_strategies",
    "load_strategy_history",
    "load_world_model_entities",
    "load_world_model_relationships",
    "agent_memory",
    "task_history",
    "strategy_scores",
    "world_model_entities",
    "world_model_relationships",
    "dream_engine",
    "DreamScheduler",
    "priority_queue",
    "queue_dreams_from_interaction",
    "auto_evolution",
    "get_evolution_log",
    "weak_tasks",
    "current_task",
    "emotional_history",
    "task_context",
];

/// The subsystems of §12.8, by whole file (`dir/` prefix or file) or by
/// function (`file::fn`).
const SEPARATED_FILES: [&str; 12] = [
    "kernel/src/simulation/",
    "kernel/src/temporal/",
    "connectors/llm/src/rag.rs",
    "crates/nexus-swarm/",
    "agents/conductor/",
    "agents/coder/",
    "agents/web-builder/",
    "app/src-tauri/src/builder_workspace.rs",
    "app/src-tauri/src/commands/swarm.rs",
    "app/src-tauri/src/commands/flash.rs",
    "app/src-tauri/src/commands/model_hub.rs",
    "kernel/src/cognitive/algorithms/world_model.rs",
];
const SEPARATED_FNS: [(&str, &str); 14] = [
    ("app/src-tauri/src/commands/chat_llm.rs", "chat_dispatch"),
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        "chat_with_ollama_streaming",
    ),
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        "test_llm_connection",
    ),
    ("app/src-tauri/src/commands/simulation.rs", "temporal_fork"),
    ("app/src-tauri/src/commands/simulation.rs", "temporal_unit"),
    ("app/src-tauri/src/commands/simulation.rs", "human_request"),
    (
        "app/src-tauri/src/commands/simulation.rs",
        "run_dilated_session",
    ),
    (
        "app/src-tauri/src/commands/consent.rs",
        "build_simulation_llm",
    ),
    (
        "app/src-tauri/src/commands/consent.rs",
        "run_parallel_simulation_reports_with",
    ),
    ("app/src-tauri/src/commands/cognitive.rs", "simulation_llm"),
    ("app/src-tauri/src/hm_dispatch.rs", "human_provider"),
    ("app/src-tauri/src/hm_dispatch.rs", "dispatch_text"),
    ("app/src-tauri/src/hm_dispatch.rs", "IssuedProvider::issued"),
    ("app/src-tauri/src/hm_dispatch.rs", "IssuedProvider::query"),
];

/// The forbidden names reached in `span` of `f`: `(fn, name)`.
fn handoffs(f: &ProdFile, span: (usize, usize)) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (code, text) = (&f.code[span.0..span.1], &f.text[span.0..span.1]);
    for name in NO_HANDOFF {
        for at in words(code, name).into_iter().chain(words(text, name)) {
            out.push((enclosing_fn(&f.code, span.0 + at), name.to_string()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The byte span of the body of `function` (`Type::name` or `name`) in `f`.
fn fn_span(f: &ProdFile, function: &str) -> (usize, usize) {
    let name = function.rsplit("::").next().unwrap();
    for at in words(&f.code, name) {
        if !f.code[..at].trim_end().ends_with("fn") {
            continue;
        }
        if enclosing_fn(&f.code, at + 1) != function && !function.contains("::") {
            // The body check below decides.
        }
        let open = at + f.code[at..].find('{').unwrap();
        let end = close_of(&f.code, open);
        if enclosing_fn(&f.code, open + 1) == function {
            return (open, end);
        }
    }
    panic!("{}: no fn {function}", f.rel)
}

#[test]
fn hm_m3_n5_a_s6_planted_controls() {
    let src = "fn plan() { let c = engine.get_agent_state_snapshot(id); let t = c.task_context.current_task; d.dream_engine.lock(); }";
    let f = ProdFile {
        rel: "kernel/src/temporal/x.rs".into(),
        code: mask(src, false),
        text: mask(src, true),
    };
    assert_eq!(
        handoffs(&f, (0, f.code.len())),
        vec![
            ("plan".to_string(), "current_task".to_string()),
            ("plan".to_string(), "dream_engine".to_string()),
            ("plan".to_string(), "task_context".to_string()),
        ]
    );
}

#[test]
fn hm_m3_n5_a_s6_separated_subsystems_receive_no_hm_content() {
    let files = production_files();
    let mut found = Vec::new();
    for f in &files {
        if SEPARATED_FILES
            .iter()
            .any(|p| f.rel == *p || (p.ends_with('/') && f.rel.starts_with(p)))
        {
            for (function, name) in handoffs(f, (0, f.code.len())) {
                found.push((f.rel.clone(), function, name));
            }
        }
    }
    for (rel, function) in SEPARATED_FNS {
        let f = file(&files, rel);
        for (inner, name) in handoffs(f, fn_span(f, function)) {
            found.push((f.rel.clone(), inner, name));
        }
    }
    assert_eq!(found, Vec::<(String, String, String)>::new());
    // No actuator or cognitive module hands data to them.
    for f in files.iter().filter(|f| {
        (f.rel.starts_with("kernel/src/actuators/") || f.rel.starts_with("kernel/src/cognitive/"))
            && f.rel != "kernel/src/cognitive/algorithms/world_model.rs"
    }) {
        for path in [
            "simulation::",
            "temporal::",
            "rag::",
            "nexus_swarm",
            "web_builder",
            "nexus_conductor",
        ] {
            assert!(!f.code.contains(path), "{}: {path}", f.rel);
        }
    }
}

// ── R7: no unlabelled in-scope content write ────────────────────────────────

/// The context-free content writers of the persistence crate, with their
/// arities (the labelled writers of `nexus_kernel::hm` that share a name
/// take the unit's context and other arguments).
const UNLABELLED_WRITERS: [(&str, usize); 5] = [
    ("save_memory", 4),
    ("save_task", 1),
    ("update_task_status", 5),
    ("upsert_strategy_score", 6),
    ("replace_world_model_state", 3),
];

#[test]
fn hm_m3_n5_r7_no_unlabelled_in_scope_content_write() {
    let files = production_files();
    // Calls of the context-free writers outside the persistence crate: only
    // the legacy `AgentMemoryManager` methods, through the `MemoryStore`
    // trait, whose production implementation refuses.
    let mut calls = Vec::new();
    for (name, arity) in UNLABELLED_WRITERS {
        calls.extend(
            call_sites_with_arity(&files, name, arity)
                .into_iter()
                .filter(|(rel, _)| !rel.starts_with("persistence/src/"))
                .map(|(rel, f)| (rel, f, name.to_string())),
        );
    }
    calls.sort();
    let reviewed: Vec<(String, String, String)> = [
        ("AgentMemoryManager::store_episodic", "save_memory"),
        ("AgentMemoryManager::store_procedural", "save_memory"),
        ("AgentMemoryManager::store_semantic", "save_memory"),
    ]
    .iter()
    .map(|(f, n)| {
        (
            "kernel/src/cognitive/memory_manager.rs".to_string(),
            f.to_string(),
            n.to_string(),
        )
    })
    .collect();
    assert_eq!(calls, reviewed);
    let lib = file(&files, "app/src-tauri/src/lib.rs");
    for function in [
        "DbMemoryStore::save_memory",
        "DbStrategyStore::upsert_strategy_score",
    ] {
        let (a, z) = fn_span(lib, function);
        let body = &lib.code[a..z];
        assert!(body.contains("Err(UNLABELLED_WRITE_REFUSED"), "{function}");
        assert!(!body.contains("self.db"), "{function}");
    }
    // No raw SQL content write to an in-scope table outside the
    // persistence crate.
    for f in files
        .iter()
        .filter(|f| !f.rel.starts_with("persistence/src/"))
    {
        let s = squash(&f.text);
        for t in IN_SCOPE_TABLES {
            for write in [format!("insert into {t}"), format!("update {t} ")] {
                assert!(!s.contains(&write), "{}: {write}", f.rel);
            }
        }
    }
}

// ── A2.4 I4: no production path writes checkpoint memories back ─────────────

#[test]
fn hm_m3_n5_i4_no_checkpoint_memory_is_written_back() {
    let files = production_files();
    // The desktop never replays a time-machine checkpoint: no undo or redo
    // call, and its undo/redo commands are closed.
    for f in files
        .iter()
        .filter(|f| f.rel.starts_with("app/src-tauri/src/"))
    {
        for name in [
            "undo",
            "redo",
            "undo_checkpoint",
            "undo_checkpoint_with",
            "redo_checkpoint",
        ] {
            assert!(calls_of(&f.code, name).is_empty(), "{}: {name}", f.rel);
        }
    }
    let lib = file(&files, "app/src-tauri/src/lib.rs");
    for command in [
        "time_machine_undo",
        "time_machine_undo_checkpoint",
        "time_machine_redo",
    ] {
        let (a, z) = fn_span(lib, command);
        assert!(
            lib.text[a..z].contains("Closure::CheckpointReplay"),
            "{command} is not closed"
        );
    }
    // The checkpoint's memories are only recorded (serialised into the
    // checkpoint), never handed to a writer.
    let cognitive = file(&files, "app/src-tauri/src/commands/cognitive.rs");
    let (a, z) = fn_span(cognitive, "record_agent_execution_checkpoint");
    let body = &cognitive.code[a..z];
    for writer in [
        "save_memory",
        "record_episodic",
        "record_semantic",
        "record_procedural",
        "save_task",
    ] {
        assert!(
            !body.contains(writer),
            "record_agent_execution_checkpoint: {writer}"
        );
    }
}

/// The classified model-call sites (P3-HM-CONTRACT-02 §12.2 and
/// `04-DISPATCH-INVENTORY.md` §K): `(file, fn or "*", class, reference)`.
/// A file entry with `"*"` covers every site of that file (the contract
/// names the file or its directory).
const CLASSIFIED: [(&str, &str, &str, &str); 36] = [
    // Desktop (§12.2), at their post-M3 locations.
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        "chat_dispatch",
        "AC/HD",
        "§12.2 send_chat, agent-routed and no agent",
    ),
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        "chat_with_ollama_streaming",
        "HD",
        "§12.2 Ollama streaming",
    ),
    (
        "app/src-tauri/src/commands/chat_llm.rs",
        "test_llm_connection",
        "SY",
        "§12.2 test_llm_connection",
    ),
    (
        "app/src-tauri/src/commands/flash.rs",
        "flash_generate",
        "HD",
        "§12.2 Flash flash_generate",
    ),
    (
        "app/src-tauri/src/commands/model_hub.rs",
        "chat_with_documents",
        "XS",
        "§12.2 RAG",
    ),
    (
        "app/src-tauri/src/commands/simulation.rs",
        "human_request",
        "HB/HD",
        "§12.2 temporal_fork, run_dilated_session",
    ),
    (
        "app/src-tauri/src/hm_dispatch.rs",
        "AgentTransport::send",
        "AR",
        "§12.2 cognitive planner / Flash as an agent transport",
    ),
    (
        "app/src-tauri/src/hm_dispatch.rs",
        "guarded_query",
        "AR",
        "§12.2 cognitive planner, LlmQuery, evolution, auto-evolution, HiveMind, team, dreams",
    ),
    (
        "app/src-tauri/src/hm_dispatch.rs",
        "IssuedProvider::query",
        "XS",
        "§12.2 RAG, builder (A2.4 I1)",
    ),
    (
        "app/src-tauri/src/hm_dispatch.rs",
        "IssuedProvider::embed",
        "XS",
        "§12.2 RAG",
    ),
    // Kernel (04 §K).
    ("kernel/src/cognitive/planner.rs", "*", "AR", "04 K1"),
    ("kernel/src/dreams/engine.rs", "*", "AR/HR", "04 K7"),
    ("kernel/src/dreams/report.rs", "*", "AR/HR", "04 K7"),
    ("kernel/src/simulation/mod.rs", "*", "XS", "04 K8"),
    (
        "kernel/src/cognitive/algorithms/world_model.rs",
        "*",
        "XS",
        "04 K8 (A2.3)",
    ),
    (
        "kernel/src/computer_control.rs",
        "query_vision_model",
        "AR, latent",
        "04 K10",
    ),
    // Connectors (04 §K).
    (
        "connectors/llm/src/gateway.rs",
        "*",
        "common sink",
        "04 K11",
    ),
    ("connectors/llm/src/rag.rs", "*", "XS", "04 K17"),
    (
        "connectors/llm/src/governance_slm.rs",
        "*",
        "latent",
        "04 K18",
    ),
    (
        "connectors/llm/src/inference_queue.rs",
        "*",
        "latent",
        "04 K18",
    ),
    ("connectors/llm/src/routing.rs", "*", "latent", "04 K18"),
    // Builder agents (04 K12, §12.2 "Builder agents").
    ("agents/conductor/src/lib.rs", "*", "XS", "04 K12"),
    ("agents/conductor/src/planner.rs", "*", "XS", "04 K12"),
    ("agents/coder/src/llm_codegen.rs", "*", "XS", "04 K12"),
    ("agents/web-builder/src/llm_codegen.rs", "*", "XS", "04 K12"),
    ("agents/web-builder/src/plan.rs", "*", "XS", "04 K12"),
    ("agents/web-builder/src/classifier.rs", "*", "XS", "04 K12"),
    (
        "agents/web-builder/src/model_router.rs",
        "*",
        "XS",
        "04 K12",
    ),
    ("agents/web-builder/src/content_gen.rs", "*", "XS", "04 K12"),
    (
        "agents/web-builder/src/backend/rls_gen.rs",
        "*",
        "XS",
        "04 K12",
    ),
    (
        "agents/web-builder/src/backend/schema_gen.rs",
        "*",
        "XS",
        "04 K12",
    ),
    ("agents/web-builder/src/interpreter.rs", "*", "XS", "04 K12"),
    ("agents/self-improve/src/learner.rs", "*", "SY", "04 K13"),
    // Crates (04 §K; §12.2 "Outside the desktop crate").
    (
        "crates/nexus-swarm/src/providers/",
        "*",
        "HD",
        "04 K14; §12.2 swarm providers/*",
    ),
    (
        "crates/nexus-capability-measurement/src/",
        "*",
        "SY",
        "04 K15",
    ),
    ("crates/nexus-memory/src/", "*", "latent", "04 K16"),
];
const CLASSIFIED_UI_REPAIR: (&str, &str, &str, &str) = (
    "crates/nexus-ui-repair/src/specialists/vision_judge.rs",
    "*",
    "XS",
    "04 K19",
);

// ── A-S4 errata E-M3-1 (AMENDMENT 3 A3.1): the sites §12.2 and 04 do not name

/// Class T, "transport implementation": the endpoint implementations behind
/// `LlmProvider::query`/`embed` and their `Box`/`Arc` forwarding. Not
/// dispatch sites: every production call of `query`/`embed` outside these
/// files is itself a classified site (asserted in A-S4).
const TRANSPORT: [(&str, &str); 3] = [
    ("connectors/llm/src/providers/", "*"),
    ("connectors/llm/src/attest.rs", "VerifiedLocalOllama::query"),
    ("nexus-code/src/llm/providers/", "*"),
];

/// The E-M3-1 classes XS, SY and AR CLOSED: `(file, fn or "*", class,
/// rule)`. XS: a production caller, and the crate reaches no HM content
/// (`hm_m3_n5_a_s6_xs_crates_reach_no_hm_content`). SY: a fixed probe, no
/// content (pinned in A-S4). AR CLOSED: the kernel executor never reaches
/// the actuator (pinned in A-S4; runtime test and mutant 18).
const ERRATA_E_M3_1: [(&str, &str, &str, &str); 15] = [
    (
        "protocols/src/http_gateway.rs",
        "build_router",
        "XS",
        "caller server_runtime::run_server (run_from_args, its only caller, has no production caller; both binaries withdrawn)",
    ),
    (
        "protocols/src/http_gateway.rs",
        "anthropic_messages",
        "XS",
        "routed by build_router",
    ),
    (
        "protocols/src/http_gateway.rs",
        "openai_chat_completions",
        "XS",
        "routed by build_router",
    ),
    (
        "protocols/src/http_gateway.rs",
        "openai_embeddings",
        "XS",
        "routed by build_router",
    ),
    (
        "content/src/generator.rs",
        "ContentGenerator::generate_post",
        "XS",
        "caller agents/social-poster (CLI run_social_poster_from_manifest)",
    ),
    (
        "factory/src/intent.rs",
        "IntentParser::parse",
        "XS",
        "caller factory remote.rs RemoteFactoryInterface (referenced nowhere outside factory)",
    ),
    (
        "crates/nexus-perception/src/vision.rs",
        "ApiVisionProvider::new_groq",
        "XS",
        "caller tauri_commands::init_provider (the desktop's perception_init is closed and never calls it)",
    ),
    (
        "crates/nexus-perception/src/vision.rs",
        "ApiVisionProvider::new_nim",
        "XS",
        "caller tauri_commands::init_provider (as new_groq)",
    ),
    (
        "agents/coder/src/fix_loop.rs",
        "LlmErrorFixer::propose_fixes",
        "XS",
        "caller conductor execute_fix_project through fix_until_pass",
    ),
    (
        "agents/coder/src/writer.rs",
        "CodeWriter::write_code",
        "XS",
        "caller conductor through writer::write_code",
    ),
    (
        "agents/web-builder/src/image_gen/api.rs",
        "generate_api",
        "XS",
        "caller image_gen::generate_image (generate_image and generate_all_images are referenced nowhere outside web-builder)",
    ),
    (
        "nexus-code/src/",
        "*",
        "XS",
        "reached through app/src-tauri nx_bridge; coding content",
    ),
    (
        "connectors/core/src/validation.rs",
        "validate_anthropic_key",
        "SY",
        "a fixed key probe (GET /v1/models, headers only), no content",
    ),
    (
        "kernel/src/actuators/image_gen.rs",
        "*",
        "AR CLOSED",
        "the kernel RegistryExecutor refuses ImageGenerate for every executor",
    ),
    (
        "kernel/src/actuators/tts.rs",
        "*",
        "AR CLOSED",
        "the kernel RegistryExecutor refuses TextToSpeech for every executor",
    ),
];

/// Class LATENT: `(file, site, closed set)`. The closed set is the site's
/// item and every item of the same file that reaches it (a type stands for
/// its definition and impl blocks; a lower-case name is a free function; a
/// `Type::method` is that method alone).
/// The proof (`latent_references`): no production reference to any item of
/// the set outside the set's own items, so none outside the defining file;
/// a reference appearing later fails A-S4.
const LATENT: [(&str, &str, &[&str]); 11] = [
    (
        "adaptation/src/adapter.rs",
        "LlmStrategyAdvisor::recommend_focus",
        &["LlmStrategyAdvisor"],
    ),
    (
        "research/src/strategy.rs",
        "StrategySynthesizer::synthesize",
        &["StrategySynthesizer"],
    ),
    (
        "control/src/vision/loop.rs",
        "ProviderVisionModel::infer",
        &["ProviderVisionModel"],
    ),
    (
        "self-update/src/analyzer.rs",
        "ErrorAnalyzer::analyze_crash",
        &["ErrorAnalyzer"],
    ),
    (
        "agents/coder/src/analyzer.rs",
        "query_llm_summary",
        &["query_llm_summary", "analyze"],
    ),
    (
        "agents/coder/src/init.rs",
        "build_readme",
        &[
            "build_readme",
            "ProjectInitializer",
            "init_project",
            "init_project_in",
        ],
    ),
    (
        "agents/designer/src/generator.rs",
        "DesignGenerator::generate_design",
        &["DesignGenerator", "generate_design"],
    ),
    (
        "agents/designer/src/screenshot_to_code.rs",
        "ScreenshotToCode::screenshot_to_code",
        &["ScreenshotToCode", "screenshot_to_code"],
    ),
    (
        "agents/screen-poster/src/comments.rs",
        "CommentInteractor::generate_reply",
        &["CommentInteractor", "generate_reply", "scan_comments"],
    ),
    (
        "agents/screen-poster/src/composer.rs",
        "ContentComposer::compose",
        &["ContentComposer", "compose"],
    ),
    // Mission P3-C11-HM-M4 C-4: no production caller; it returns a genome
    // whose system prompt is an unlabelled copy of a model reply (its model
    // call is the `AutoEvolveLlm::mutate_prompt` trait call, §11.4).
    (
        "kernel/src/genome/auto_evolve.rs",
        "AutoEvolutionManager::apply_evolution",
        &["AutoEvolutionManager::apply_evolution"],
    ),
];

/// The §11.4 model-trait calls through which a LATENT method that is no
/// model-call site of [`CALLS`] reaches a model (mission P3-C11-HM-M4 C-4).
const MODEL_TRAIT_CALLS: [&str; 3] = [
    ".mutate_prompt(",
    ".score_response(",
    ".generate_with_prompt(",
];

/// Exact-site scanner exceptions: `(file, fn, shape, reason)`. Each matches
/// exactly one inventory row, and its premise is pinned in A-S4.
const NOT_MODEL_CALLS: [(&str, &str, &str, &str); 1] = [(
    "agents/social-poster/src/lib.rs",
    "RealSearchStep::search",
    "query",
    "WebSearchConnector::query(ctx, topic, max_results): a web-search connector, not a model call",
)];

/// The class of the site `(file, fn)`: T first, then §12.2 / 04, then
/// E-M3-1 (XS, SY, AR CLOSED), then LATENT.
fn class_of(file: &str, function: &str) -> Option<&'static str> {
    let matches = |f: &str, g: &str| {
        (f == file || (f.ends_with('/') && file.starts_with(f))) && (g == "*" || g == function)
    };
    if TRANSPORT.iter().any(|e| matches(e.0, e.1)) {
        return Some("T");
    }
    CLASSIFIED
        .iter()
        .chain(std::iter::once(&CLASSIFIED_UI_REPAIR))
        .chain(ERRATA_E_M3_1.iter())
        .find(|e| matches(e.0, e.1))
        .map(|e| e.2)
        .or_else(|| LATENT.iter().any(|e| matches(e.0, e.1)).then_some("LATENT"))
}

/// The module name of `rel` (its file stem, or its directory for `mod.rs`).
fn module_name(rel: &str) -> &str {
    let file = rel.rsplit('/').next().unwrap();
    if file == "mod.rs" {
        rel.rsplit('/').nth(1).unwrap()
    } else {
        file.trim_end_matches(".rs")
    }
}

/// The spans of the items named `name` in `code`: a free `fn name` (not a
/// method of another type's impl), or a type's `struct`/`enum` definition
/// and its `impl` blocks.
fn item_spans(code: &str, name: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let b = code.as_bytes();
    let block_end = |from: usize| -> usize {
        // The item's `{ .. }` (or its `;`) at bracket depth 0 of `( [ <`.
        let mut depth = 0i64;
        let mut j = from;
        while j < b.len() {
            match b[j] {
                b'(' | b'[' | b'<' => depth += 1,
                b')' | b']' | b'>' if j > 0 && b[j - 1] != b'-' && b[j - 1] != b'=' => depth -= 1,
                b';' if depth <= 0 => return j + 1,
                b'{' if depth <= 0 => return close_of(code, j),
                _ => {}
            }
            j += 1;
        }
        b.len()
    };
    for keyword in ["fn", "struct", "enum"] {
        for at in words(code, keyword) {
            let rest = code[at + keyword.len()..].trim_start();
            if rest.starts_with(name)
                && !rest[name.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                out.push((at, block_end(at + keyword.len())));
            }
        }
    }
    for at in words(code, "impl") {
        let Some(open) = code[at..].find('{').map(|o| at + o) else {
            continue;
        };
        let head: String = code[at..open]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let ty = head
            .rsplit(" for ")
            .next()
            .unwrap_or(&head)
            .split_whitespace()
            .last()
            .unwrap_or("")
            .split('<')
            .next()
            .unwrap_or("");
        if ty == name {
            out.push((at, close_of(code, open)));
        }
    }
    out
}

/// The crate directory of `rel` (the part before `/src/`).
fn crate_dir(rel: &str) -> &str {
    rel.find("/src/").map_or(rel, |p| &rel[..p])
}

/// The library name of the crate in `dir` (its `[lib]` name, else its
/// package name with `-` as `_`; a directory without a manifest, as in a
/// planted control, by its own name).
fn lib_name(dir: &str) -> String {
    let Ok(toml) = std::fs::read_to_string(source::workspace_root().join(dir).join("Cargo.toml"))
    else {
        return dir.rsplit('/').next().unwrap().replace('-', "_");
    };
    let name_in = |section: &str| {
        let s = toml.find(section)?;
        let body = &toml[s + section.len()..];
        let body = &body[..body.find("\n[").unwrap_or(body.len())];
        let line = body.lines().find(|l| l.trim_start().starts_with("name"))?;
        Some(line.split('"').nth(1)?.to_string())
    };
    name_in("[lib]")
        .or_else(|| name_in("[package]"))
        .unwrap()
        .replace('-', "_")
}

/// The path segments written before byte `at` of `code` (`a::b::` gives
/// `[a, b]`).
fn path_before(code: &str, at: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut end = at;
    loop {
        let before = code[..end].trim_end();
        let Some(rest) = before.strip_suffix("::") else {
            break;
        };
        let rest = rest.trim_end();
        let start = rest
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(0, |p| p + 1);
        if start == rest.len() {
            break;
        }
        out.insert(0, rest[start..].to_string());
        end = start;
    }
    out
}

/// The production references to the closed set `items` of `file` that lie
/// outside the set's own items: `(file, line, item)`. A type is referenced
/// by its name anywhere. A free function is referenced, in its file, by a
/// call or path that is not a method call (`.name`) or a definition
/// (`fn name`); in another file, by a path `module::name` that resolves to
/// the defining crate (its library name, or `crate`/`self`/`super` or a bare
/// module path inside that crate, or a bare module path in a file that
/// names the library), by such a `use`, or by a bare call after such a glob
/// import of the module. A method `Type::method` is referenced by any
/// method call `.method` or path `::method` outside its own body, in any
/// file (a method call is not resolved to its receiver's type, so a method
/// of the same name anywhere counts too).
fn latent_references(
    files: &[ProdFile],
    file: &str,
    items: &[&str],
) -> Vec<(String, usize, String)> {
    let module = module_name(file);
    let home = crate_dir(file);
    let lib = lib_name(home);
    let defining = files
        .iter()
        .find(|f| f.rel == file)
        .unwrap_or_else(|| panic!("{file}: not a production file"));
    let spans: Vec<(usize, usize)> = items
        .iter()
        .flat_map(|n| item_spans(&defining.code, n))
        .collect();
    let mut out = Vec::new();
    for f in files {
        let same_crate = crate_dir(&f.rel) == home;
        let names_lib = !words(&f.code, &lib).is_empty();
        let resolves = |path: &[String]| -> bool {
            match path.first().map(String::as_str) {
                Some("crate" | "self" | "super") => same_crate,
                Some(first) if path.len() > 1 => first == lib,
                _ => same_crate || names_lib,
            }
        };
        let glob = words(&f.code, module).into_iter().any(|at| {
            f.code[at + module.len()..].trim_start().starts_with("::*")
                && resolves(&{
                    let mut p = path_before(&f.code, at);
                    p.push(module.to_string());
                    p
                })
        });
        let use_spans: Vec<(usize, usize)> = words(&f.code, "use")
            .into_iter()
            .map(|u| (u, f.code[u..].find(';').map_or(f.code.len(), |e| u + e)))
            .collect();
        for name in items {
            if let Some((_, method)) = name.split_once("::") {
                let own = (f.rel == file).then(|| fn_span(defining, name));
                for at in words(&f.code, method) {
                    let before = f.code[..at].trim_end();
                    if (before.ends_with('.') || before.ends_with("::"))
                        && !own.is_some_and(|(a, z)| a <= at && at < z)
                    {
                        let line = f.code[..at].matches('\n').count() + 1;
                        out.push((f.rel.clone(), line, name.to_string()));
                    }
                }
                continue;
            }
            let is_type = name.starts_with(|c: char| c.is_ascii_uppercase());
            for at in words(&f.code, name) {
                let before = f.code[..at].trim_end();
                let line = f.code[..at].matches('\n').count() + 1;
                if !is_type && (before.ends_with('.') || before.ends_with("fn")) {
                    continue;
                }
                let referenced = if f.rel == file {
                    !spans.iter().any(|&(a, z)| a <= at && at < z)
                } else if is_type {
                    true
                } else {
                    let path = path_before(&f.code, at);
                    let qualified =
                        path.last().map(String::as_str) == Some(module) && resolves(&path);
                    let imported = use_spans
                        .iter()
                        .find(|&&(u, e)| u < at && at < e)
                        .is_some_and(|&(u, _)| {
                            let body = &f.code[u + 3..at];
                            let segs: Vec<String> = body
                                .split("::")
                                .map(|s| s.trim().trim_start_matches('{').trim().to_string())
                                .collect();
                            !words(body, module).is_empty() && resolves(&segs)
                        });
                    qualified || imported || (glob && path.is_empty())
                };
                if referenced {
                    out.push((f.rel.clone(), line, name.to_string()));
                }
            }
        }
    }
    out
}

/// A-S4: every production model call of every workspace crate is in the
/// allowlist with its class (§12.1, §12.2, 04 and errata E-M3-1); any other
/// call fails.
#[test]
fn hm_m3_n5_a_s4_every_production_model_call_is_classified() {
    // Planted: an unknown site is unclassified; each class resolves.
    assert_eq!(class_of("kernel/src/x.rs", "f"), None);
    assert_eq!(
        class_of("kernel/src/dreams/engine.rs", "dream_query"),
        Some("AR/HR")
    );
    assert_eq!(
        class_of("crates/nexus-swarm/src/providers/anthropic.rs", "invoke"),
        Some("HD")
    );
    assert_eq!(
        class_of(
            "connectors/llm/src/providers/ollama.rs",
            "OllamaProvider::embed"
        ),
        Some("T")
    );
    assert_eq!(
        class_of(
            "nexus-code/src/llm/providers/google.rs",
            "GoogleProvider::new"
        ),
        Some("T")
    );
    assert_eq!(class_of("nexus-code/src/agent/mod.rs", "run"), Some("XS"));
    assert_eq!(
        class_of("connectors/llm/src/attest.rs", "verify_local_ollama"),
        None
    );
    assert_eq!(
        class_of(
            "research/src/strategy.rs",
            "StrategySynthesizer::synthesize"
        ),
        Some("LATENT")
    );
    assert_eq!(class_of("research/src/strategy.rs", "other"), None);
    assert_eq!(
        class_of("kernel/src/actuators/tts.rs", "TtsActuator::run_openai"),
        Some("AR CLOSED")
    );

    let files = production_files();
    let mut inventory = model_call_inventory(&files);
    // Exact-site scanner exceptions, each matching exactly one row.
    for (file, function, shape, reason) in NOT_MODEL_CALLS {
        let key = (file.to_string(), function.to_string(), shape.to_string());
        assert_eq!(
            inventory.remove(&key),
            Some(1),
            "{file} {function}: {reason}"
        );
    }
    // Its premise: the call is on the step's web-search connector.
    let poster = file(&files, "agents/social-poster/src/lib.rs");
    let def = poster.code.find("struct RealSearchStep").unwrap();
    let body =
        &poster.code[def..close_of(&poster.code, def + poster.code[def..].find('{').unwrap())];
    assert!(
        squash(body).contains("connector: websearchconnector,"),
        "{body}"
    );
    let (a, z) = fn_span(poster, "RealSearchStep::search");
    assert!(poster.code[a..z].contains("self.connector.query("));

    let unclassified: Vec<String> = inventory
        .iter()
        .filter(|((file, function, _), _)| class_of(file, function).is_none())
        .map(|((file, function, shape), n)| format!("{file} | {function} | {shape} | {n}"))
        .collect();
    assert!(
        unclassified.is_empty(),
        "{} unclassified production model-call sites:\n{}",
        unclassified.len(),
        unclassified.join("\n")
    );
    // T: a transport file makes no dispatch of its own. Its only calls of
    // query/embed are the Box/Arc forwarding and the L1 proof's forwarding
    // to the verified provider; every other call of query/embed is a
    // classified non-T site (checked above).
    let transport_calls: Vec<(String, String)> = inventory
        .keys()
        .filter(|(f, g, shape)| !shape.starts_with("endpoint") && class_of(f, g) == Some("T"))
        .map(|(f, g, _)| (f.clone(), g.clone()))
        .collect();
    let forwarding: Vec<(String, String)> = [
        ("connectors/llm/src/attest.rs", "VerifiedLocalOllama::query"),
        ("connectors/llm/src/providers/mod.rs", "Arc::embed"),
        ("connectors/llm/src/providers/mod.rs", "Arc::query"),
        ("connectors/llm/src/providers/mod.rs", "Box::embed"),
        ("connectors/llm/src/providers/mod.rs", "Box::query"),
    ]
    .iter()
    .map(|(f, g)| (f.to_string(), g.to_string()))
    .collect();
    assert_eq!(
        transport_calls, forwarding,
        "dispatch inside a transport file"
    );
    // nexus-code's provider trait and implementations are reached only
    // inside nexus-code (class XS); the desktop names only its router's
    // slot configuration.
    for f in files
        .iter()
        .filter(|f| !f.rel.starts_with("nexus-code/src/"))
    {
        for path in ["nexus_code::llm::provider", "nexus_code::llm::streaming"] {
            assert!(!f.code.contains(path), "{}: {path}", f.rel);
        }
        let providers = f.code.matches("nexus_code::llm::providers").count();
        assert_eq!(providers, 0, "{}: nexus-code providers", f.rel);
    }
    // LATENT: each closed set has no production reference from outside.
    for (file, site, items) in LATENT {
        let found = latent_references(&files, file, items);
        assert!(
            found.is_empty(),
            "LATENT {file} {site}: referenced at {found:?}"
        );
        // A model-call site, or a method reaching a model through a §11.4
        // model trait.
        let reaches_model = site.contains("::") && items.len() == 1 && items[0] == site && {
            let def = self::file(&files, file);
            let (a, z) = fn_span(def, site);
            MODEL_TRAIT_CALLS
                .iter()
                .any(|call| def.code[a..z].contains(call))
        };
        assert!(
            inventory.keys().any(|(f, g, _)| f == file && g == site) || reaches_model,
            "LATENT {file} {site}: no such model-call site"
        );
    }
    // XS / SY / AR CLOSED entries name existing sites (no stale entry).
    for (file, function, class, _) in ERRATA_E_M3_1 {
        if function != "*" {
            assert!(
                inventory.keys().any(|(f, g, _)| f == file && g == function),
                "{class} {file} {function}: no such model-call site"
            );
        }
    }
    // SY: the key probe sends headers to a fixed models URL, and no body.
    let validation = file(&files, "connectors/core/src/validation.rs");
    let (a, z) = fn_span(validation, "validate_anthropic_key");
    let probe = &validation.text[a..z];
    assert!(probe.contains("https://api.anthropic.com/v1/models"));
    assert!(validation.code[a..z].contains("http_status_with_headers(url.as_str(), &headers)"));
    for body in ["-d", "--data", "messages", "prompt", "json!"] {
        assert!(!probe.contains(body), "validate_anthropic_key: {body}");
    }
    // AR CLOSED (G4): the kernel executor refuses both actions before any
    // routing, in `execute` and in `execute_in`; no other production
    // registry user names them.
    let runtime = file(&files, "kernel/src/cognitive/loop_runtime.rs");
    let (a, z) = fn_span(runtime, "is_closed_model_actuator");
    assert_eq!(
        squash(&runtime.code[a..z]),
        "{ matches!( action, plannedaction::imagegenerate { .. } | plannedaction::texttospeech { .. } ) }"
    );
    for method in ["RegistryExecutor::execute", "RegistryExecutor::execute_in"] {
        let (a, z) = fn_span(runtime, method);
        let body = squash(&runtime.text[a..z]);
        let first_arm = body.find("match action {").map(|m| &body[m..]).unwrap();
        assert!(
            first_arm.starts_with(
                "match action { _ if is_closed_model_actuator(action) => { err(format!("
            ),
            "{method}: {first_arm:.200}"
        );
        let arm = &first_arm[..first_arm.find("})").unwrap()];
        assert!(arm.contains("model_actuator_closed"), "{method}: {arm}");
    }
    let registries: Vec<&str> = files
        .iter()
        .filter(|f| f.code.contains("ActuatorRegistry::with_defaults("))
        .map(|f| f.rel.as_str())
        .collect();
    assert_eq!(
        registries,
        [
            "kernel/src/cognitive/loop_runtime.rs",
            "kernel/src/content_pipeline.rs",
            "kernel/src/protocols/mcp.rs",
        ],
        "production users of the default actuator registry"
    );
    for rel in &registries[1..] {
        let code = &file(&files, rel).code;
        for action in ["ImageGenerate", "TextToSpeech"] {
            assert!(!code.contains(action), "{rel}: {action}");
        }
    }
}

/// Mission P3-C11-HM-M4 C-4 planted controls: a production caller of the
/// latent `AutoEvolutionManager::apply_evolution`, by a method call (also
/// split across lines) or by a path, is a reference that fails A-S4; the
/// method's own definition, another type's definition of a method of that
/// name, and a mention in a comment or a string are none.
#[test]
fn hm_m4_n1_c4_latent_apply_evolution_planted_controls() {
    let (file_rel, site, items) = LATENT
        .iter()
        .find(|e| e.1 == "AutoEvolutionManager::apply_evolution")
        .copied()
        .expect("C-4: apply_evolution is in the LATENT set");
    let mut files = production_files();
    assert!(
        latent_references(&files, file_rel, items).is_empty(),
        "precondition: no production caller in the workspace"
    );
    let planted = |text: &str| ProdFile {
        rel: "app/src-tauri/src/planted_c4.rs".into(),
        code: mask(text, false),
        text: mask(text, true),
    };
    let callers = [
        "fn planted(m: &AutoEvolutionManager) { let _ = m.apply_evolution(&mut c, &g, &l); }",
        "fn planted(m: &AutoEvolutionManager) { let _ = m\n    .apply_evolution(&mut c, &g, &l); }",
        "fn planted(m: &AutoEvolutionManager) { let _ = AutoEvolutionManager::apply_evolution(m, &mut c, &g, &l); }",
        "fn planted(m: &Arc<AutoEvolutionManager>) { m.as_ref().apply_evolution(&mut c, &g, &l); }",
    ];
    for caller in callers {
        files.push(planted(caller));
        let found = latent_references(&files, file_rel, items);
        files.pop();
        assert_eq!(
            found
                .iter()
                .map(|(f, _, i)| (f.as_str(), i.as_str()))
                .collect::<Vec<_>>(),
            vec![("app/src-tauri/src/planted_c4.rs", site)],
            "C-4: a planted production caller was not found: {caller}"
        );
    }
    let not_callers = [
        "impl Other { fn apply_evolution(&self) {} }",
        "// m.apply_evolution(&mut c, &g, &l);\nfn x() {}",
        "fn x() { let s = \"m.apply_evolution(a, b, c)\"; }",
        "fn apply_evolution_report() { record_apply_evolution(); }",
    ];
    for text in not_callers {
        files.push(planted(text));
        let found = latent_references(&files, file_rel, items);
        files.pop();
        assert!(
            found.is_empty(),
            "C-4: not a caller, but found: {text}: {found:?}"
        );
    }
}

// ── A-S6 extended (E-M3-1): XS crates reach no HM content ───────────────────

/// The XS crates of E-M3-1, and the modules they hand data to, scanned
/// whole: no read of the in-scope tables, the dream queue, the
/// auto-evolution log or `ConsciousnessState` text.
const XS_CRATES: [&str; 10] = [
    "protocols/src/",
    "content/src/",
    "agents/social-poster/src/",
    "connectors/social/src/",
    "factory/src/",
    "crates/nexus-perception/src/",
    "nexus-code/src/",
    "agents/coder/src/",
    "agents/conductor/",
    "agents/web-builder/",
];

/// What an XS crate may never name: [`NO_HANDOFF`] and the types that hold
/// or produce HM content.
const XS_NO_HANDOFF: [&str; 8] = [
    "ConsciousnessState",
    "AgentMemoryManager",
    "EvolutionTracker",
    "AutoEvolutionManager",
    "DreamEngine",
    "CognitiveRuntime",
    "DispatchIssuer",
    "RecalledRecord",
];

/// XS entry points referenced nowhere outside their crate: `(crate dir,
/// identifiers)`.
const XS_CLOSED_ENTRIES: [(&str, &[&str]); 3] = [
    (
        "protocols/src/",
        &[
            "build_router",
            "GatewayState",
            "run_from_args",
            "server_runtime",
            "http_gateway",
        ],
    ),
    ("factory/src/", &["IntentParser", "RemoteFactoryInterface"]),
    (
        "agents/web-builder/",
        &["generate_image", "generate_all_images", "generate_api"],
    ),
];

/// XS crates reached from outside the scanned set, by their library names.
const XS_LIBS: [(&str, &str); 5] = [
    ("crates/nexus-perception/src/", "nexus_perception"),
    ("nexus-code/src/", "nexus_code"),
    ("agents/social-poster/src/", "social_poster_agent"),
    ("content/src/", "nexus_content"),
    ("agents/coder/src/", "coder_agent"),
];

/// The reviewed functions outside the scanned set that hand data to an XS
/// crate: `(file, fn, lib, what is handed)`.
const XS_HANDOFF_FNS: [(&str, &str, &str, &str); 11] = [
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_analyze_chart",
        "nexus_perception",
        "the frontend's image and format",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_describe",
        "nexus_perception",
        "the frontend's image and format",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_extract_data",
        "nexus_perception",
        "the frontend's image, format and schema",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_extract_text",
        "nexus_perception",
        "the frontend's image and format",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_find_ui_elements",
        "nexus_perception",
        "the frontend's image",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_get_policy",
        "nexus_perception",
        "the perception state only (reads its policy)",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_question",
        "nexus_perception",
        "the frontend's image, format and question",
    ),
    (
        "app/src-tauri/src/commands/crate_bridges.rs",
        "perception_read_error",
        "nexus_perception",
        "the frontend's image",
    ),
    (
        "app/src-tauri/src/nx_bridge/commands.rs",
        "nx_switch_provider",
        "nexus_code",
        "the frontend's provider name and its default model",
    ),
    (
        "app/src-tauri/src/nx_bridge/mod.rs",
        "init_nx_state",
        "nexus_code",
        "the desktop's nexus-code config file and memory paths under the identity home",
    ),
    (
        "cli/src/lib.rs",
        "start_agent",
        "social_poster_agent",
        "the social-poster manifest path and the dry-run flag",
    ),
];

/// The forbidden names reached in `span` of `f`, with [`XS_NO_HANDOFF`]:
/// `(fn, name)`.
fn xs_handoffs(f: &ProdFile, span: (usize, usize)) -> Vec<(String, String)> {
    let mut out = handoffs(f, span);
    let code = &f.code[span.0..span.1];
    for name in XS_NO_HANDOFF {
        for at in words(code, name) {
            out.push((enclosing_fn(&f.code, span.0 + at), name.to_string()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The leaves of the use tree `tree` (after `prev::`): each imported name
/// or alias.
fn use_leaves(tree: &str, prev: &str, out: &mut Vec<String>) {
    let tree = tree.trim();
    if let Some(inner) = tree.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        let (mut depth, mut start) = (0i64, 0);
        for (i, c) in inner.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                ',' if depth == 0 => {
                    use_leaves(&inner[start..i], prev, out);
                    start = i + 1;
                }
                _ => {}
            }
        }
        use_leaves(&inner[start..], prev, out);
    } else if let Some(p) = tree.find("::") {
        use_leaves(&tree[p + 2..], tree[..p].trim(), out);
    } else if let Some((_, alias)) = tree.split_once(" as ") {
        out.push(alias.trim().to_string());
    } else if tree == "self" {
        out.push(prev.to_string());
    } else if !tree.is_empty() && tree != "*" {
        out.push(tree.to_string());
    }
}

/// The local names a `use` of `lib` brings into scope (leaves and aliases).
fn imported_names(stmt: &str, lib: &str) -> Vec<String> {
    let s = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(p) = s.find(&format!("{lib}::")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    use_leaves(s[p + lib.len() + 2..].trim_end_matches(';'), lib, &mut out);
    out
}

/// Whether the occurrence of a handle at `at` in `code` carries data: the
/// path it starts ends in a call with arguments or a struct literal (not a
/// type in a signature, `impl` header or field).
fn carries_data(code: &str, at: usize, handle: &str) -> bool {
    let b = code.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let skip_ws = |mut k: usize| {
        while k < b.len() && b[k].is_ascii_whitespace() {
            k += 1;
        }
        k
    };
    let mut k = at + handle.len();
    loop {
        k = skip_ws(k);
        if !code[k..].starts_with("::") {
            break;
        }
        k = skip_ws(k + 2);
        let s = k;
        while k < b.len() && ident(b[k]) {
            k += 1;
        }
        if k == s {
            return false;
        }
    }
    match b.get(k) {
        Some(b'(') => arg_count(code, k) > 0,
        Some(b'{') => {
            let before = code[..at].trim_end();
            let in_type = before.ends_with("->")
                || words(before, "for")
                    .last()
                    .is_some_and(|&p| p + 3 == before.len())
                || words(before, "impl")
                    .last()
                    .is_some_and(|&p| p + 4 == before.len());
            !in_type && !code[k + 1..close_of(code, k) - 1].trim().is_empty()
        }
        _ => false,
    }
}

/// The functions outside the scanned set that hand data to an XS crate:
/// `(file, fn, lib)`.
fn xs_handoff_fns(files: &[ProdFile]) -> Vec<(String, String, String)> {
    let scanned = |rel: &str| {
        XS_CRATES
            .iter()
            .chain(SEPARATED_FILES.iter())
            .any(|p| rel == *p || (p.ends_with('/') && rel.starts_with(p)))
    };
    let crate_of = |rel: &str| -> String {
        rel.find("src/")
            .map_or_else(|| rel.to_string(), |p| rel[..p + 4].to_string())
    };
    let mut out = Vec::new();
    for (_, lib) in XS_LIBS {
        // The handles of each crate: the library name and every name a
        // `use` of it brings into scope in any file of that crate.
        let mut handles: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in files.iter().filter(|f| !scanned(&f.rel)) {
            for at in words(&f.code, "use") {
                let end = f.code[at..].find(';').map_or(f.code.len(), |e| at + e);
                let stmt = &f.code[at..end];
                if !words(stmt, lib).is_empty() {
                    handles
                        .entry(crate_of(&f.rel))
                        .or_default()
                        .extend(imported_names(stmt, lib));
                }
            }
        }
        for f in files.iter().filter(|f| !scanned(&f.rel)) {
            let mut hs = vec![lib.to_string()];
            hs.extend(handles.get(&crate_of(&f.rel)).cloned().unwrap_or_default());
            for h in &hs {
                for at in words(&f.code, h) {
                    let stmt_start = f.code[..at].rfind([';', '}', '{']).map_or(0, |p| p + 1);
                    let stmt = f.code[stmt_start..at].trim_start();
                    if stmt.starts_with("use ") || stmt.starts_with("pub use ") {
                        continue;
                    }
                    if carries_data(&f.code, at, h) {
                        out.push((f.rel.clone(), enclosing_fn(&f.code, at), lib.to_string()));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn hm_m3_n5_a_s6_xs_planted_controls() {
    let src = "use nexus_x::tauri as x_cmds;\nuse nexus_x::{a::{self, B}, c as d};\nfn h(s: &S) -> nexus_x::R { x_cmds::describe(&s.p, &img) }\nfn ty(p: x_cmds::State) { let _ = x_cmds::State::default(); }\nfn lit() { let c = nexus_x::Cfg { p: 1 }; }\nfn sneaky(db: &D) { let m = db.load_memories(a, None, 5); x_cmds::ask(&m); }";
    let f = ProdFile {
        rel: "app/src-tauri/src/x.rs".into(),
        code: mask(src, false),
        text: mask(src, true),
    };
    assert_eq!(
        imported_names("use nexus_x::{a::{self, B}, c as d}", "nexus_x"),
        vec!["a", "B", "d"]
    );
    assert_eq!(
        imported_names("use nexus_x::tauri as x_cmds", "nexus_x"),
        vec!["x_cmds"]
    );
    let at = f.code.find("x_cmds::describe").unwrap();
    assert!(carries_data(&f.code, at, "x_cmds"));
    let at = f.code.find("x_cmds::State::default").unwrap();
    assert!(!carries_data(&f.code, at, "x_cmds"));
    let at = f.code.find("x_cmds::State)").unwrap();
    assert!(!carries_data(&f.code, at, "x_cmds"));
    let at = f.code.find("nexus_x::Cfg").unwrap();
    assert!(carries_data(&f.code, at, "nexus_x"));
    let at = f.code.find("nexus_x::R").unwrap();
    assert!(!carries_data(&f.code, at, "nexus_x"));
    assert_eq!(
        xs_handoffs(&f, fn_span(&f, "sneaky")),
        vec![("sneaky".to_string(), "load_memories".to_string())]
    );
    // LATENT proof: a planted outside reference is found; the set's own
    // items are not.
    let def = "pub struct Adv<P> { p: P }\nimpl<P> Adv<P> { pub fn go(&self) {} }\nfn wrap() -> u8 { 1 }\npub fn entry() { Adv { p: 1 }.go(); wrap(); }";
    let user = "fn main2() { let a = m::entry(); let b = crate::m::Adv { p: 2 }; x.wrap(); }";
    let planted = [
        ProdFile {
            rel: "k/src/m.rs".into(),
            code: mask(def, false),
            text: mask(def, true),
        },
        ProdFile {
            rel: "k/src/u.rs".into(),
            code: mask(user, false),
            text: mask(user, true),
        },
    ];
    let lines = |v: Vec<(String, usize, String)>| -> Vec<(String, String)> {
        v.into_iter().map(|(f, _, n)| (f, n)).collect()
    };
    assert_eq!(
        lines(latent_references(&planted, "k/src/m.rs", &["Adv"])),
        vec![
            ("k/src/m.rs".to_string(), "Adv".to_string()),
            ("k/src/u.rs".to_string(), "Adv".to_string()),
        ]
    );
    assert_eq!(
        lines(latent_references(&planted, "k/src/m.rs", &["Adv", "entry"])),
        vec![
            ("k/src/u.rs".to_string(), "Adv".to_string()),
            ("k/src/u.rs".to_string(), "entry".to_string()),
        ]
    );
    assert_eq!(
        lines(latent_references(
            &planted[..1],
            "k/src/m.rs",
            &["Adv", "entry", "wrap"]
        )),
        Vec::<(String, String)>::new()
    );
}

#[test]
fn hm_m3_n5_a_s6_xs_crates_reach_no_hm_content() {
    let files = production_files();
    // (a) Each XS crate, and each module it hands data to, names no HM
    // content.
    let mut found = Vec::new();
    for f in files
        .iter()
        .filter(|f| XS_CRATES.iter().any(|p| f.rel.starts_with(p)))
    {
        for (function, name) in xs_handoffs(f, (0, f.code.len())) {
            found.push((f.rel.clone(), function, name));
        }
    }
    assert_eq!(found, Vec::<(String, String, String)>::new());
    // (b) Entry points referenced nowhere outside their crate.
    for (dir, entries) in XS_CLOSED_ENTRIES {
        for f in files.iter().filter(|f| !f.rel.starts_with(dir)) {
            for name in entries {
                assert!(words(&f.code, name).is_empty(), "{}: {name}", f.rel);
            }
        }
    }
    // (c) Every function outside the scanned set that hands data to an XS
    // crate is reviewed, and reads no HM content.
    let computed = xs_handoff_fns(&files);
    let mut reviewed: Vec<(String, String, String)> = XS_HANDOFF_FNS
        .iter()
        .map(|(f, g, l, _)| (f.to_string(), g.to_string(), l.to_string()))
        .collect();
    reviewed.sort();
    assert_eq!(computed, reviewed, "XS handoff functions");
    let mut found = Vec::new();
    for (rel, function, _) in &computed {
        let f = file(&files, rel);
        for (inner, name) in xs_handoffs(f, fn_span(f, function)) {
            found.push((rel.clone(), inner, name));
        }
    }
    assert_eq!(found, Vec::<(String, String, String)>::new());
}

// ── A3.2 (C2): the one test-only desktop module that uses the store ─────────

#[test]
fn hm_m3_n5_a3_2_the_desktop_store_fixture_module_is_test_only() {
    let root = source::workspace_root();
    let lib = std::fs::read_to_string(root.join("app/src-tauri/src/lib.rs")).unwrap();
    let code = mask(&lib, false);
    let decls: Vec<usize> = words(&code, "hm_m3_tests")
        .into_iter()
        .filter(|&at| code[..at].trim_end().ends_with("mod"))
        .collect();
    assert_eq!(decls.len(), 1, "one declaration of hm_m3_tests");
    let before = code[..decls[0]]
        .trim_end()
        .trim_end_matches("mod")
        .trim_end();
    assert!(
        before.ends_with("#[cfg(test)]"),
        "hm_m3_tests is not #[cfg(test)]"
    );
    let files = production_files();
    assert!(files
        .iter()
        .all(|f| f.rel != "app/src-tauri/src/hm_m3_tests.rs"));
    // Every desktop source file (test code included) that reaches the store
    // is that module.
    let mut users = Vec::new();
    let mut stack = vec![root.join("app/src-tauri/src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                let code = mask(&std::fs::read_to_string(&p).unwrap(), false);
                if [".hm()", "HmStore", "HmTx", "persistence::hm"]
                    .iter()
                    .any(|h| code.contains(h))
                {
                    users.push(source::relative(&p));
                }
            }
        }
    }
    assert_eq!(users, vec!["app/src-tauri/src/hm_m3_tests.rs".to_string()]);
}
