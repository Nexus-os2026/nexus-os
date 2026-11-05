//! Registered POLICY successors. K01 runs these ONLY on synthetic trees.
//! LIVE_STAGE remains 1. Activation requires the matching reviewed K deletion;
//! existing F7/HM/logging/web/acceptance guards are not bypassed here.
#[allow(dead_code)]
mod retirement;
#[allow(dead_code)]
#[path = "hm_m3_guards/source.rs"]
mod source;
use retirement::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

type Tree = BTreeMap<String, String>;
type Findings = BTreeSet<(&'static str, String)>;

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone)]
#[serde(deny_unknown_fields)]
struct HmRow {
    kind: String,
    retire: u8,
    path: String,
    site: Option<String>,
    route: Option<String>,
}
#[derive(Deserialize)]
struct LogSink {
    path: String,
    expression: String,
}
fn hm_rows(stage: u8) -> BTreeSet<HmRow> {
    include_str!("retirement/hm-rows.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<HmRow>(l).unwrap())
        .filter(|r| r.retire > stage)
        .collect()
}
fn logs() -> Vec<LogSink> {
    include_str!("retirement/logging.jsonl")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn required_tests() -> BTreeSet<String> {
    include_str!("retirement/retained-tests.txt")
        .lines()
        .map(str::to_owned)
        .collect()
}
fn prod(raw: &str) -> (String, String) {
    let mut code = source::mask(raw, false).into_bytes();
    let mut text = source::mask(raw, true).into_bytes();
    let (ranges, _) = source::cfg_test_items(
        std::str::from_utf8(&code).unwrap(),
        std::str::from_utf8(&text).unwrap(),
    );
    for (a, z) in ranges {
        for i in a..z.min(code.len()) {
            if code[i] != b'\n' {
                code[i] = b' ';
                text[i] = b' ';
            }
        }
    }
    (
        String::from_utf8(code).unwrap(),
        String::from_utf8(text).unwrap(),
    )
}
fn retiring(path: &str, stage: u8) -> bool {
    stages()
        .iter()
        .any(|r| r.stage > stage && r.roots.iter().any(|p| within(path, p)))
}
fn pattern(re: &str) -> std::sync::Arc<regex::Regex> {
    type Cache = std::sync::Mutex<BTreeMap<String, std::sync::Arc<regex::Regex>>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Cache::default).lock().unwrap();
    cache
        .entry(re.to_owned())
        .or_insert_with(|| std::sync::Arc::new(regex::Regex::new(re).unwrap()))
        .clone()
}
fn identifier(code: &str, word: &str) -> bool {
    pattern(&format!(r"\b{}\b", regex::escape(word))).is_match(code)
}
fn log_expressions(raw: &str) -> Vec<String> {
    let (code, text) = prod(raw);
    pattern(r"\b(?:(?:tracing|log)::(?:trace|debug|info|warn|error)|eprintln|println|eprint|print|dbg)!\s*[({\[]")
        .find_iter(&code).map(|m| {
            let open = m.end()-1;
            text[m.start()..source::close_of(&code, open)].replace("\r\n", "\n")
        }).collect()
}
fn classifier_bytes(raw: &str) -> Option<String> {
    // Freeze the entire body, including literal whitespace/refusal vocabulary.
    // The import block can move independently; only checkout line endings vary.
    Some(
        raw.get(raw.find("pub fn classify(")?..)?
            .replace("\r\n", "\n"),
    )
}
fn classifier_ok(raw: &str) -> bool {
    classifier_bytes(raw).is_some_and(|s| {
        digest(s.as_bytes()) == include_str!("retirement/broker-classifier.sha256").trim()
    })
}

fn module_names(code: &str) -> BTreeSet<String> {
    pattern(r"\bmod\s+(?:r#)?(\w+)")
        .captures_iter(code)
        .map(|c| c[1].to_owned())
        .collect()
}
fn module_edges(stage: u8) -> &'static BTreeMap<String, BTreeSet<String>> {
    type Edges = BTreeMap<String, BTreeSet<String>>;
    static EDGES: std::sync::OnceLock<Vec<Edges>> = std::sync::OnceLock::new();
    &EDGES.get_or_init(|| (0..=22).map(build_module_edges).collect())[stage as usize]
}
fn build_module_edges(stage: u8) -> BTreeMap<String, BTreeSet<String>> {
    let mut edges: BTreeMap<String, BTreeSet<String>> =
        serde_json::from_str(include_str!("retirement/module-edges.json")).unwrap();
    for row in stages().iter().filter(|r| r.stage <= stage) {
        for declaration in &row.declarations {
            let module = declaration
                .trim_start_matches("pub mod ")
                .trim_end_matches(';');
            edges.get_mut("kernel/src/lib.rs").unwrap().remove(module);
        }
        for extraction in &row.extract {
            if extraction
                .payload
                .iter()
                .any(|p| p == "string_or_vec::deserialize")
            {
                edges
                    .entry(extraction.destination.clone())
                    .or_default()
                    .insert("string_or_vec".into());
            }
            let destination = extraction
                .destination
                .trim_start_matches("kernel/src/")
                .trim_end_matches(".rs");
            if let Some((parent, child)) = destination.split_once('/') {
                edges
                    .entry(format!("kernel/src/{parent}.rs"))
                    .or_default()
                    .insert(child.into());
            } else {
                edges
                    .get_mut("kernel/src/lib.rs")
                    .unwrap()
                    .insert(destination.into());
            }
        }
    }
    edges
}

// Ready for later stage activation. A strict prewalk rejects unreadable inputs
// and directory symlinks BEFORE the existing HM production classifier is used.
// Whole-file test exclusions retain that scanner's intersection proof.
fn load_live_tree() -> Tree {
    load_tree_in(&root())
}
fn retired_spelling(rel: &str) -> bool {
    stages().iter().flat_map(|r| &r.roots).any(|p| {
        let stem = p.trim_end_matches(".rs");
        within(rel, stem) || rel == format!("{stem}.rs")
    })
}
fn load_tree_in(workspace: &std::path::Path) -> Tree {
    fn walk(
        workspace: &std::path::Path,
        dir: &std::path::Path,
        paths: &mut Vec<std::path::PathBuf>,
    ) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            if name.starts_with("target")
                || [".git", "node_modules", "dist", "docs"].contains(&name)
            {
                continue;
            }
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "unreviewed source alias: {}",
                path.display()
            );
            let rel = path
                .strip_prefix(workspace)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            if kind.is_dir() {
                if retired_spelling(&rel) {
                    paths.push(path.clone());
                }
                walk(workspace, &path, paths);
            } else {
                let input = path.extension().is_some_and(|e| e == "rs") || name == "Cargo.toml";
                if input {
                    std::fs::read_to_string(&path).expect("all scanner input must be readable");
                }
                if input || retired_spelling(&rel) {
                    paths.push(path);
                }
            }
        }
    }
    let mut paths = Vec::new();
    walk(workspace, workspace, &mut paths);
    let production: BTreeMap<_, _> = source::production_files_in(workspace)
        .into_iter()
        .map(|f| (f.rel, f.text))
        .collect();
    let test_paths: BTreeSet<_> = required_tests()
        .into_iter()
        .map(|t| t.rsplit_once("::").unwrap().0.to_owned())
        .collect();
    let mut known = production_paths();
    for row in stages() {
        known.extend(row.paths.iter().cloned());
    }
    let mut tree = Tree::new();
    for path in paths {
        let rel = path
            .strip_prefix(workspace)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        if path.is_dir() || (retired_spelling(&rel) && path.extension().is_none_or(|e| e != "rs")) {
            tree.insert(rel, String::new());
        } else if rel.ends_with("Cargo.toml") || test_paths.contains(&rel) {
            tree.insert(rel, std::fs::read_to_string(path).unwrap());
        } else if let Some(text) = production.get(&rel) {
            tree.insert(rel, text.clone());
        } else if known.contains(&rel) || retired_spelling(&rel) {
            tree.insert(rel, String::new());
        } // PROVED test-only file exists
    }
    tree
}

#[test]
fn synthetic_loader_keeps_empty_directories_non_rust_and_test_only_restorations() {
    let fixture = tempfile::tempdir().unwrap();
    for dir in ["kernel/src/cognitive", "kernel/src/supervisor"] {
        std::fs::create_dir_all(fixture.path().join(dir)).unwrap();
    }
    std::fs::write(
        fixture.path().join("kernel/src/cognitive/note.txt"),
        "plant",
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("kernel/src/cognitive/tests.rs"),
        "#[test] fn plant() {} ",
    )
    .unwrap();
    let tree = load_tree_in(fixture.path());
    for path in [
        "kernel/src/cognitive",
        "kernel/src/supervisor",
        "kernel/src/cognitive/note.txt",
        "kernel/src/cognitive/tests.rs",
    ] {
        assert!(tree.contains_key(path), "{path}");
    }
    rejected(22, &tree, "f7-absence");
}

#[test]
fn live_successors_activate_only_at_the_explicit_reviewed_stage() {
    if LIVE_STAGE >= 2 {
        assert_eq!(successor(LIVE_STAGE, &load_live_tree()), Findings::new());
    }
}

#[test]
fn registered_module_edges_name_todays_exact_declarations() {
    // This witnesses registration against the unchanged K01 baseline only.
    // Later stages are checked by the live successor against staged edges.
    if LIVE_STAGE != 1 {
        return;
    }
    let edges = module_edges(1);
    let mut mismatches = Vec::new();
    for path in production_paths() {
        let code = source::mask(&read(&path), false);
        let actual = module_names(&code);
        if actual != edges.get(&path).cloned().unwrap_or_default() {
            mismatches.push(serde_json::json!({"path":path,"modules":actual}).to_string());
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn registered_logging_sinks_match_current_production_views() {
    if LIVE_STAGE != 1 {
        return;
    }
    let mut actual = BTreeMap::<String, Vec<String>>::new();
    for file in source::production_files() {
        if stages()
            .iter()
            .any(|r| r.roots.iter().any(|p| within(&file.rel, p)))
        {
            continue;
        }
        let sinks = log_expressions(&file.text);
        if !sinks.is_empty() {
            actual.insert(file.rel, sinks);
        }
    }
    let mut expected = BTreeMap::<String, Vec<String>>::new();
    for sink in logs() {
        expected.entry(sink.path).or_default().push(sink.expression);
    }
    assert_eq!(actual, expected, "production logging registration");
}

// Keep the full 26-route no-handoff vocabulary. No world-model exception after K11.
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

/// An exhaustive closed input inventory plus narrow source predicates. This is
/// a source oracle, not a semantic Rust/data-flow proof. Existing authority,
/// labelled-write, transport, broker, privacy and resolved-graph guards remain.
fn successor(stage: u8, tree: &Tree) -> Findings {
    assert!((2..=22).contains(&stage));
    let mut found = Findings::new();
    let expected = paths_at(stage);
    let production_seeds = production_paths();
    let manifests = cargo_manifests();
    let named_tests = required_tests();
    let test_files: BTreeSet<_> = named_tests
        .iter()
        .map(|t| t.rsplit_once("::").unwrap().0.to_owned())
        .collect();
    for path in expected
        .iter()
        .chain(manifests.keys())
        .chain(test_files.iter())
    {
        if !tree.contains_key(path) {
            found.insert(("omission", path.clone()));
        }
    }
    for (path, expected_hash) in manifests {
        if tree
            .get(&path)
            .is_none_or(|raw| digest(raw.as_bytes()) != expected_hash)
        {
            found.insert(("f7-dependency", path));
        }
    }
    for row in stages().iter().filter(|r| r.stage <= stage) {
        for root in &row.roots {
            if tree.keys().any(|p| within(p, root)) {
                found.insert(("f7-absence", root.clone()));
            }
            // Deny BOTH alternate Rust file/directory spellings, not just
            // the exact original file inventory.
            let stem = root.trim_end_matches(".rs");
            if tree
                .keys()
                .any(|p| within(p, stem) || p == &format!("{stem}.rs"))
            {
                found.insert(("f7-absence", root.clone()));
            }
        }
    }
    for (path, raw) in tree {
        if !path.ends_with(".rs") || (test_files.contains(path) && !production_seeds.contains(path))
        {
            continue;
        }
        if !expected.contains(path) {
            found.insert(("f7-dependency", path.clone()));
        }
        if raw.is_empty() {
            continue;
        }
        let (code, text) = prod(raw);
        let squashed = compact(&code);
        // Preserve F7's already retired free-api/orchestration/delegation bans.
        for name in [
            "free_api_registry",
            "orchestration",
            "delegation",
            "Orchestrator",
            "DelegationEngine",
            "DelegationConstraints",
        ] {
            if identifier(&code, name) {
                found.insert(("f7-dependency", path.clone()));
            }
        }
        for row in stages().iter().filter(|r| r.stage <= stage) {
            for declaration in &row.declarations {
                let module = declaration
                    .trim_start_matches("pub mod ")
                    .trim_end_matches(';');
                for prefix in ["crate::", "nexus_kernel::", "nexus_kernel::{"] {
                    if squashed.contains(&format!("{prefix}{module}"))
                        && (prefix != "crate::" || path.starts_with("kernel/"))
                    {
                        found.insert(("f7-dependency", path.clone()));
                    }
                }
                if path == "kernel/src/lib.rs"
                    && pattern(&format!(r"\bmod\s+{}\b", regex::escape(module))).is_match(&code)
                {
                    found.insert(("f7-absence", path.clone()));
                }
            }
        }
        // Exact preregistered module edges catch renamed inline runtimes too.
        let allowed = module_edges(stage).get(path).cloned().unwrap_or_default();
        if !module_names(&code).is_subset(&allowed) {
            found.insert(("f7-dependency", path.clone()));
        }
        if retiring(path, stage) {
            continue;
        } // legacy subjects still under their CURRENT guards
        for (n, token) in [
            (11, "CognitiveRuntime"),
            (11, "RegistryExecutor"),
            (12, "ActuatorRegistry"),
            (12, "HermeticSearchBackend"),
            (14, "Supervisor"),
            (15, "SpeculativeEngine"),
            (17, "TimeMachine"),
            (19, "ConsentRuntime"),
            (19, "PolicyEngine"),
        ] {
            if stage >= n && identifier(&code, token) {
                found.insert(("f7-dependency", path.clone()));
            }
        }
        let process_boundary = path == "kernel/src/resource_limiter.rs"
            || path.starts_with("kernel/src/resource_limiter/")
            || path == "crates/nexus-governed-control/src/launcher.rs"
            || path.starts_with("crates/nexus-governed-control/src/launcher/")
            || path.starts_with("crates/nexus-governed-control/src/tool/");
        if !process_boundary && pattern(r"\bCommand\s*::\s*new\s*\(").is_match(&code) {
            found.insert(("raw-process", path.clone()));
        }
        let network_boundary = path.starts_with("kernel/src/inference/")
            || path.starts_with("crates/nexus-governed-control/src/egress")
            || path.starts_with("crates/nexus-governed-control/src/browser/");
        if !network_boundary
            && pattern(r"\b(?:reqwest|TcpStream|UdpSocket|TcpSocket)\s*::").is_match(&code)
        {
            found.insert(("raw-network", path.clone()));
        }
        let inference_boundary = path.starts_with("kernel/src/inference/")
            || path == "crates/nexus-governed-control/src/inference.rs";
        if !inference_boundary
            && [
                "/api/generate",
                "/api/chat",
                "/api/embed",
                "/v1/chat/completions",
                "/v1/completions",
                "/v1/messages",
                "/v1/embeddings",
                "api.openai.com",
                "api.anthropic.com",
                "generativelanguage.googleapis.com",
            ]
            .iter()
            .any(|s| text.contains(s))
        {
            found.insert(("raw-inference", path.clone()));
        }
        // Split the identifier so the unchanged Stop scanner still rejects
        // new literal claimants even in a test fixture file.
        let claim = concat!("claim_global_", "controller");
        if identifier(&code, claim)
            && ![
                "kernel/src/emergency_stop.rs",
                "crates/nexus-governed-control/src/governed.rs",
            ]
            .contains(&path.as_str())
        {
            found.insert(("stop-seal", path.clone()));
        }
        if path.starts_with("mother/desktop/src/")
            && pattern(r"\b(?:StopController|claim_global_|resume|lift_if)\b").is_match(&code)
        {
            found.insert(("desktop-controller", path.clone()));
        }
        if ![
            "persistence/src/lib.rs",
            "persistence/src/coding_run_ledger.rs",
            "crates/nexus-governed-control/src/evidence_store.rs",
        ]
        .contains(&path.as_str())
            && pattern(r"\bConnection\s*::\s*open(?:_with_flags|_in_memory)?\s*\(").is_match(&code)
        {
            found.insert(("hm-sqlite", path.clone()));
        }
        if squashed.contains("NexusDatabase::open(")
            && !hm_rows(stage)
                .iter()
                .any(|r| r.kind == "state-db-open" && r.path == *path)
        {
            found.insert(("hm-sqlite", path.clone()));
        }
        if stage >= 11
            && !path.starts_with("persistence/src/")
            && !path.starts_with("kernel/src/hm")
        {
            for name in [
                "save_memory",
                "save_task",
                "update_task_status",
                "upsert_strategy_score",
                "replace_world_model_state",
            ] {
                if pattern(&format!(r"\.\s*{name}\s*\(")).is_match(&code) {
                    found.insert(("hm-writer", path.clone()));
                }
            }
            let sql = text.to_ascii_lowercase();
            if ["insert", "update", "replace"]
                .iter()
                .any(|s| sql.contains(s))
                && [
                    "agent_memory",
                    "task_history",
                    "strategy_scores",
                    "world_model_entities",
                    "world_model_relationships",
                ]
                .iter()
                .any(|s| sql.contains(s))
            {
                found.insert(("hm-writer", path.clone()));
            }
            if NO_HANDOFF.iter().any(|word| identifier(&code, word)) {
                found.insert(("hm-no-handoff", path.clone()));
            }
            if identifier(&code, "PromptBuilder")
                && ["value_json", "result_json", "steps_json", "goal"]
                    .iter()
                    .any(|s| text.contains(s))
            {
                found.insert(("hm-prompt", path.clone()));
            }
        }
    }
    // Full retained sink expressions are frozen. Unknown or omitted sinks fail.
    let mut expected_logs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for sink in logs() {
        expected_logs
            .entry(sink.path)
            .or_default()
            .push(sink.expression.replace("\r\n", "\n"));
    }
    for (path, raw) in tree
        .iter()
        .filter(|(p, _)| expected.contains(*p) && !retiring(p, stage))
    {
        let mut actual = log_expressions(raw);
        let mut wanted = expected_logs.remove(path).unwrap_or_default();
        actual.sort();
        wanted.sort();
        if actual != wanted {
            found.insert(("logging", path.clone()));
        }
    }
    for path in expected_logs.keys() {
        found.insert(("logging", path.clone()));
    }
    for name in named_tests {
        let (path, function) = name.rsplit_once("::").unwrap();
        if tree.get(path).is_none_or(|raw| {
            !pattern(&format!(
                r"#\[(?:tokio::)?test(?:\([^\]]*\))?\](?:\s*#\[[^\]]*\])*\s*(?:async\s+)?fn\s+{}\b",
                regex::escape(function)
            ))
            .is_match(raw)
        }) {
            found.insert(("acceptance-inventory", name));
        }
    }
    found
}

/// Explicitly synthetic, deliberately no subject code is read from disk here.
/// Empty source slots represent unchanged, effect-free fixture modules.
fn clean_tree(stage: u8) -> Tree {
    let mut tree: Tree = paths_at(stage)
        .into_iter()
        .map(|p| (p, String::new()))
        .collect();
    for (path, _) in cargo_manifests() {
        tree.insert(path.clone(), read(&path));
    }
    for test in required_tests() {
        let (path, function) = test.rsplit_once("::").unwrap();
        tree.entry(path.into())
            .or_default()
            .push_str(&format!("#[test]\nfn {function}() {{}}\n"));
    }
    for sink in logs() {
        tree.entry(sink.path)
            .or_default()
            .push_str(&format!("{};\n", sink.expression));
    }
    tree
}
fn rejected(stage: u8, tree: &Tree, oracle: &'static str) {
    let findings = successor(stage, tree);
    assert!(
        findings.iter().any(|(o, _)| *o == oracle),
        "expected {oracle}, got {findings:?}"
    );
}

#[test]
fn successors_are_registered_and_green_only_on_clean_synthetic_stages() {
    assert!((1..=22).contains(&LIVE_STAGE));
    for stage in 2..=22 {
        assert_eq!(
            successor(stage, &clean_tree(stage)),
            Findings::new(),
            "K{stage:02}"
        );
    }
}

#[test]
fn every_retired_path_alternate_spelling_declaration_and_reexport_is_a_red_plant() {
    for row in stages().iter().filter(|r| !r.roots.is_empty()) {
        for root in &row.roots {
            let mut tree = clean_tree(row.stage);
            tree.insert(
                format!("{}/restored.rs", root.trim_end_matches(".rs")),
                String::new(),
            );
            rejected(row.stage, &tree, "f7-absence");
        }
        for declaration in &row.declarations {
            let mut tree = clean_tree(row.stage);
            tree.entry("kernel/src/lib.rs".into())
                .or_default()
                .push_str(declaration);
            rejected(row.stage, &tree, "f7-absence");
        }
        for export in &row.reexports {
            let mut tree = clean_tree(row.stage);
            tree.insert("kernel/src/cognitive/mod.rs".into(), export.clone());
            rejected(row.stage, &tree, "f7-absence");
        }
    }
}

#[test]
fn renamed_runtime_alias_dependency_and_inline_module_cannot_return() {
    for stage in [11, 12, 14, 19, 22] {
        for (path, plant) in [
            (
                "kernel/src/new_brain.rs",
                "pub struct FreshRunner; impl FreshRunner { fn delegate(&self) { worker.run(); } }",
            ),
            (
                "kernel/src/privacy.rs",
                "mod renamed_engine { pub fn run() { planner.next(); } }",
            ),
            (
                "kernel/src/lib.rs",
                "pub use alternate_brain::FreshRunner as Coordinator;",
            ),
        ] {
            let mut tree = clean_tree(stage);
            tree.entry(path.into()).or_default().push_str(plant);
            if plant.contains("alternate_brain") {
                tree.get_mut("kernel/Cargo.toml").unwrap().push_str("\n[dependencies.alternate_brain]\npackage = \"runtime-wrapper\"\nversion = \"1\"\n");
            }
            rejected(stage, &tree, "f7-dependency");
        }
    }
}

#[test]
fn boundary_stop_hm_desktop_and_logging_mutations_have_specific_red_oracles() {
    for (oracle, path, plant) in [
        (
            "raw-process",
            "kernel/src/privacy.rs",
            r#"fn run(){std::process::Command::new("sh");}"#,
        ),
        (
            "raw-network",
            "kernel/src/audit/mod.rs",
            r#"fn run(){reqwest::Client::new();}"#,
        ),
        (
            "raw-inference",
            "kernel/src/privacy.rs",
            r#"fn run(){client.post("http://localhost/api/generate");}"#,
        ),
        (
            "stop-seal",
            "kernel/src/privacy.rs",
            concat!("fn run(){StopAuthority::claim_global_", "controller();}"),
        ),
        (
            "hm-writer",
            "kernel/src/privacy.rs",
            r#"fn run(){db.save_memory(a,b,c,d);}"#,
        ),
        (
            "hm-writer",
            "kernel/src/privacy.rs",
            r#"fn run(){db.execute("INSERT INTO agent_memory VALUES (?)", content);}"#,
        ),
        (
            "hm-sqlite",
            "kernel/src/privacy.rs",
            r#"fn run(){rusqlite::Connection::open(path);}"#,
        ),
        (
            "hm-sqlite",
            "kernel/src/privacy.rs",
            r#"fn run(){NexusDatabase::open(path);}"#,
        ),
        (
            "hm-prompt",
            "kernel/src/privacy.rs",
            r#"fn run(){PromptBuilder::new(c).text(row.field("value_json"));}"#,
        ),
        (
            "desktop-controller",
            "mother/desktop/src/status.rs",
            "pub fn control(c: StopController) { c.lift_if(ok); }",
        ),
        (
            "logging",
            "kernel/src/privacy.rs",
            r#"fn log(){let preview = raw_content; tracing::info!("{preview}");}"#,
        ),
        (
            "logging",
            "kernel/src/privacy.rs",
            r#"fn log(){eprintln!("{}",error);}"#,
        ),
    ] {
        let mut tree = clean_tree(22);
        tree.entry(path.into()).or_default().push_str(plant);
        rejected(22, &tree, oracle);
    }
    for route in NO_HANDOFF {
        let mut tree = clean_tree(22);
        tree.entry("kernel/src/privacy.rs".into())
            .or_default()
            .push_str(&format!("fn handoff(){{outside::{route}(content);}}"));
        rejected(22, &tree, "hm-no-handoff");
    }
}

#[test]
fn hm_narrowing_is_exact_and_never_creates_an_exception() {
    let baseline = hm_rows(1);
    assert_eq!(baseline.len(), 23);
    for stage in 2..=22 {
        let rows = hm_rows(stage);
        assert!(rows.is_subset(&baseline));
        for row in &baseline {
            assert_eq!(rows.contains(row), row.retire > stage);
        }
        if stage >= 11 {
            assert!(!rows
                .iter()
                .any(|r| ["prompt-read", "unlabelled-write", "separation"]
                    .contains(&r.kind.as_str())));
        }
        if stage >= 13 {
            assert!(rows
                .iter()
                .all(|r| r.kind == "identity-db" && r.path == "kernel/src/identity_home.rs"));
        }
    }
    // An extra allowance or an omitted retained row fails exact inventory equality.
    let expected = hm_rows(22);
    let mut changed = expected.clone();
    changed.insert(HmRow {
        kind: "unlabelled-write".into(),
        retire: 255,
        path: "kernel/src/privacy.rs".into(),
        site: None,
        route: None,
    });
    assert_ne!(expected, changed);
    changed.clear();
    assert_ne!(expected, changed);
}

#[test]
fn omission_controls_reject_empty_missing_seed_sink_and_named_security_test() {
    rejected(22, &Tree::new(), "omission");
    for path in [
        "kernel/src/lib.rs",
        "kernel/src/hm.rs",
        "kernel/src/emergency_stop.rs",
        "mother/src/lib.rs",
        "kernel/Cargo.toml",
    ] {
        let mut tree = clean_tree(22);
        assert!(tree.remove(path).is_some());
        rejected(22, &tree, "omission");
    }
    for name in required_tests() {
        let (path, function) = name.rsplit_once("::").unwrap();
        let mut tree = clean_tree(22);
        let raw = tree.get_mut(path).unwrap();
        *raw = raw.replace(&format!("fn {function}()"), "fn omitted()");
        rejected(22, &tree, "acceptance-inventory");
    }
    let sink = logs().remove(0);
    let mut tree = clean_tree(22);
    tree.get_mut(&sink.path).unwrap().clear();
    rejected(22, &tree, "logging");
}

#[test]
fn web_search_successor_has_no_curl_exception_and_classifier_mutation_is_red() {
    // Existing broker behavior remains frozen independently of kernel type imports.
    let raw = read("crates/nexus-governed-control/src/planned.rs");
    assert!(classifier_ok(&raw));
    let mutant = raw.replace(
        "PlannedAction::WebSearch { .. }",
        "PlannedAction::WebFetch { .. }",
    );
    assert_ne!(raw, mutant);
    assert!(!classifier_ok(&mutant));
    let mutant = raw.replace(
        "agent file reads stay closed",
        "agent file reads now allowed",
    );
    assert_ne!(raw, mutant);
    assert!(!classifier_ok(&mutant));
    let mutant = raw.replace("agent file reads stay closed", "agentfilereadsstayclosed");
    assert_ne!(raw, mutant);
    assert!(!classifier_ok(&mutant));
    for (path, plant, oracle) in [
        (
            "kernel/src/actuators/web.rs",
            "pub struct GovernedWeb;",
            "f7-absence",
        ),
        (
            "kernel/src/privacy.rs",
            "fn search(){Command::new(\"curl\");}",
            "raw-process",
        ),
        (
            "kernel/src/privacy.rs",
            "fn search(){let backend = HermeticSearchBackend;}",
            "f7-dependency",
        ),
    ] {
        let mut tree = clean_tree(12);
        tree.entry(path.into()).or_default().push_str(plant);
        rejected(12, &tree, oracle);
    }
}
