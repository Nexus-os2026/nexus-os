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

#[allow(dead_code)]
#[path = "retirement/coverage.rs"]
mod coverage;
#[path = "retirement/web_search_source.rs"]
mod web_search_source;

/// Exclusions are a CURRENT scanner proof, never baseline inventory membership.
/// Any in-memory mutation invalidates that proof: uncertain inputs are scanned.
#[derive(Clone, Default)]
struct Tree {
    files: BTreeMap<String, String>,
    test_only: BTreeSet<String>,
}
impl Tree {
    fn new() -> Self {
        Self::default()
    }
    fn prove_test_only_in(&mut self, workspace: &std::path::Path) {
        let production: BTreeSet<_> = source::production_files_in(workspace)
            .into_iter()
            .map(|f| f.rel)
            .collect();
        self.test_only = self
            .files
            .keys()
            .filter(|p| p.ends_with(".rs") && !production.contains(*p))
            .cloned()
            .collect();
    }
    fn write_fixture(&self) -> tempfile::TempDir {
        let fixture = tempfile::tempdir().unwrap();
        for (rel, raw) in &self.files {
            if !rel.ends_with(".rs") && !rel.ends_with("Cargo.toml") {
                continue;
            }
            let path = fixture.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, raw).unwrap();
        }
        fixture
    }
    fn prove_fixture(&mut self) {
        let fixture = self.write_fixture();
        self.prove_test_only_in(fixture.path());
    }
}
impl std::ops::Deref for Tree {
    type Target = BTreeMap<String, String>;
    fn deref(&self) -> &Self::Target {
        &self.files
    }
}
impl std::ops::DerefMut for Tree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.test_only.clear();
        &mut self.files
    }
}
#[path = "retirement/controls.rs"]
mod controls;

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
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct LogSink {
    path: String,
    expression: String,
    context_sha256: String,
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
const WEB_TEST_PATH: &str = "kernel/tests/kernel_retirement_web_search.rs";
// The predecessor's synthetic shape, independent of later source deletion.
// This is source-policy input, never a compiled/executed backend.
const WEB_SOURCE_FIXTURE: &str = r#"
pub trait WebSearchBackend {
    fn search(&self, q: &str) -> R;
    fn fetch_content(&self, url: &str) -> R;
}
impl WebSearchBackend for B {
    fn search(&self, _query: &str) -> R {
        Err(WEB_SEARCH_CLOSED.to_string())
    }
    fn fetch_content(&self, url: &str) -> R {
        curl_get(url)
    }
}
fn curl_get(url: &str) -> R {
    let command = Command::new("curl");
}
"#;
fn web_test_source(stage: u8) -> String {
    // A frozen text fixture survives the K12 retirement of the live binary.
    let raw = include_str!("retirement/web-search-guard.rs.txt");
    if stage >= 3 {
        raw.replace("use nexus_kernel::cognitive::types::PlannedAction;\n", "")
            .replace(
                "use nexus_kernel::actuators::{",
                "use nexus_kernel::action_types::PlannedAction;\nuse nexus_kernel::actuators::{",
            )
    } else {
        raw.to_owned()
    }
}

#[test]
fn k03_f3_import_relocation_preserves_all_other_bytes() {
    let raw = include_str!("retirement/web-search-guard.rs.txt");
    assert_eq!(web_test_source(2), raw);
    let staged = web_test_source(3);
    assert!(staged.contains(
        "use nexus_kernel::action_types::PlannedAction;\nuse nexus_kernel::actuators::{"
    ));
    assert_eq!(
        staged
            .replace("use nexus_kernel::action_types::PlannedAction;\n", "")
            .replace(
                "use nexus_kernel::test_support::HermeticSearchBackend;",
                "use nexus_kernel::cognitive::types::PlannedAction;\nuse nexus_kernel::test_support::HermeticSearchBackend;",
            ),
        raw,
        "K03 changes only the import path and its rustfmt position"
    );
    // A compiling import in the old position must still fail the exact oracle.
    assert_ne!(
        staged,
        raw.replace(
            "nexus_kernel::cognitive::types::PlannedAction",
            "nexus_kernel::action_types::PlannedAction"
        )
    );
}
fn required_test_policy() -> BTreeMap<String, coverage::Contract> {
    serde_json::from_str(include_str!("retirement/retained-test-policy.json")).unwrap()
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct TestEdge {
    source: String,
    module: String,
    target: String,
    contract: coverage::Contract,
}
fn test_edges() -> Vec<TestEdge> {
    serde_json::from_str(include_str!("retirement/retained-test-edges.json")).unwrap()
}
fn prod(raw: &str) -> (String, String) {
    type Cache = std::sync::Mutex<BTreeMap<String, (String, String)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Cache::default).lock().unwrap();
    if let Some(views) = cache.get(raw) {
        return views.clone();
    }
    if cache.len() >= 64 {
        cache.clear();
    }
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
    let views = (
        String::from_utf8(code).unwrap(),
        String::from_utf8(text).unwrap(),
    );
    cache.insert(raw.into(), views.clone());
    views
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
fn referenced_modules(code: &str, namespace: &str) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let add = |at: usize, modules: &mut BTreeSet<String>| {
        if let Some(name) = pattern(r"^(?:r#)?(\w+)").captures(&code[at..]) {
            modules.insert(name[1].into());
        }
    };
    for prefix in pattern(&format!(r"\b{}\s*::\s*", regex::escape(namespace))).find_iter(code) {
        let mut at = prefix.end();
        if code.as_bytes().get(at) != Some(&b'{') {
            add(at, &mut modules);
            continue;
        }
        let end = source::close_of(code, at);
        at += 1;
        while at < end - 1 {
            while at < end - 1 && code.as_bytes()[at].is_ascii_whitespace() {
                at += 1;
            }
            add(at, &mut modules);
            while at < end - 1 && code.as_bytes()[at] != b',' {
                if matches!(code.as_bytes()[at], b'{' | b'(' | b'[') {
                    at = source::close_of(code, at);
                } else {
                    at += 1;
                }
            }
            at += 1;
        }
    }
    modules
}
fn logging_sinks(path: &str, raw: &str) -> Vec<LogSink> {
    let (code, text) = prod(raw);
    let functions = coverage::declarations(&text);
    pattern(r"\b(?:(?:tracing|log)::(?:trace|debug|info|warn|error)|eprintln|println|eprint|print|dbg)!\s*[({\[]")
        .find_iter(&code).map(|m| {
            let open = m.end()-1;
            let context = functions.iter().filter(|d| d.kind == "fn" && d.span.0 <= m.start() && m.start() < d.span.1)
                .min_by_key(|d| d.span.1 - d.span.0);
            LogSink {
                path: path.into(),
                expression: text[m.start()..source::close_of(&code, open)].replace("\r\n", "\n"),
                // Freeze the entire containing function, including parameters,
                // bindings and branches. No interprocedural data-flow claim.
                context_sha256: context.map_or_else(|| "unbounded-context".into(), |d|
                    digest(text[d.span.0..d.span.1].replace("\r\n", "\n").as_bytes())),
            }
        }).collect()
}
fn classifier_bytes(raw: &str) -> Option<String> {
    // This named predecessor subject is the classifier function only. Unrelated
    // helpers before/after it may grow; its original body/refusal bytes may not.
    let code = source::mask(raw, false);
    let start = code.find("pub fn classify(")?;
    let open = start + code[start..].find('{')?;
    let end = source::close_of(&code, open);
    Some(format!("{}\n", raw.get(start..end)?.replace("\r\n", "\n")))
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

// The scanner supplies the production inventory. The metadata walk additionally
// rejects aliases/unreadable input and records retired non-Rust paths/empty dirs;
// it never decides which production files are loaded.
fn load_live_tree() -> Tree {
    load_tree_in(&root())
}
// Every historical F7 absent root remains absent, including empty directories.
const F7_ABSENT: [&str; 6] = [
    "kernel/src/free_api_registry",
    "kernel/src/orchestration",
    "kernel/src/delegation",
    "kernel/src/actuators/self_evolution",
    "kernel/src/actuators/governance_policy",
    "kernel/src/actuators/cognitive_param",
];
fn retired_spelling(rel: &str) -> bool {
    if F7_ABSENT
        .iter()
        .any(|stem| within(rel, stem) || rel == format!("{stem}.rs"))
    {
        return true;
    }
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
                || [".git", "node_modules", "dist"].contains(&name)
                || path == workspace.join("docs")
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
    let production: BTreeSet<_> = source::production_files_in(workspace)
        .into_iter()
        .map(|f| f.rel)
        .collect();
    let test_paths: BTreeSet<_> = required_tests()
        .into_iter()
        .map(|t| t.rsplit_once("::").unwrap().0.to_owned())
        .chain(std::iter::once(WEB_TEST_PATH.to_owned()))
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
        } else if known.contains(&rel) || retired_spelling(&rel) {
            tree.insert(rel, String::new());
        } // PROVED test-only file exists
    }
    // Load EVERY scanner-discovered file, independently of the metadata walk.
    for rel in &production {
        tree.insert(
            rel.clone(),
            std::fs::read_to_string(workspace.join(rel)).unwrap(),
        );
    }
    tree.test_only = tree
        .keys()
        .filter(|p| p.ends_with(".rs") && !production.contains(*p))
        .cloned()
        .collect();
    tree
}

#[test]
fn synthetic_loader_keeps_empty_directories_non_rust_and_test_only_restorations() {
    let fixture = clean_tree(22).write_fixture();
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
    rejected(22, &tree, "f7-absence", "kernel/src/cognitive/note.txt");
}

#[test]
fn live_successors_activate_only_at_the_explicit_reviewed_stage() {
    if LIVE_STAGE >= 2 {
        dependencies::assert_live();
        assert_eq!(successor(LIVE_STAGE, &load_live_tree()), Findings::new());
    }
}

#[test]
fn registered_f3_split_pins_the_live_direct_guard_until_k12() {
    if LIVE_STAGE < 12 {
        assert_eq!(read(WEB_TEST_PATH), web_test_source(LIVE_STAGE));
    }
}

#[test]
fn registered_kernel_module_edges_name_todays_exact_declarations() {
    // This witnesses registration against the unchanged K01 baseline only.
    // Later stages are checked by the live successor against staged edges.
    if LIVE_STAGE != 1 {
        return;
    }
    let edges = module_edges(1);
    let mut mismatches = Vec::new();
    for path in production_paths()
        .into_iter()
        .filter(|p| within(p, "kernel"))
    {
        let code = source::mask(&read(&path), false);
        let actual = module_names(&code);
        if actual != edges.get(&path).cloned().unwrap_or_default() {
            mismatches.push(serde_json::json!({"path":path,"modules":actual}).to_string());
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn registered_kernel_logging_sinks_match_current_production_views() {
    if LIVE_STAGE != 1 {
        return;
    }
    let mut actual = BTreeMap::<String, Vec<LogSink>>::new();
    for file in source::production_files() {
        if !within(&file.rel, "kernel")
            || stages()
                .iter()
                .any(|r| r.roots.iter().any(|p| within(&file.rel, p)))
        {
            continue;
        }
        let sinks = logging_sinks(&file.rel, &file.text);
        if !sinks.is_empty() {
            actual.insert(file.rel, sinks);
        }
    }
    let mut expected = BTreeMap::<String, Vec<LogSink>>::new();
    for sink in logs() {
        expected.entry(sink.path.clone()).or_default().push(sink);
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

/// Kernel inventories and staged retirement absence rules under R-6. This is
/// a source oracle, not a semantic Rust/data-flow proof. Existing authority,
/// labelled-write, transport, broker, privacy and resolved-graph guards remain.
fn successor(stage: u8, tree: &Tree) -> Findings {
    assert!((2..=22).contains(&stage));
    let mut found = Findings::new();
    let expected = paths_at(stage);
    let manifests = cargo_manifests();
    let named_tests = required_tests();
    let test_files: BTreeSet<_> = named_tests
        .iter()
        .map(|t| t.rsplit_once("::").unwrap().0.to_owned())
        .collect();
    for path in expected
        .iter()
        .chain(manifests.iter())
        .chain(test_files.iter())
    {
        if !tree.contains_key(path) {
            found.insert(("omission", path.clone()));
        }
    }
    // K11 deletes the executor, not the web actuator. Preserve the independent
    // F3 source/default/injected tests until K12, including their bodies. The
    // only prescribed edit is K03's same-type PlannedAction import move.
    if stage < 12 && tree.get(WEB_TEST_PATH) != Some(&web_test_source(stage)) {
        found.insert(("f3-coverage", WEB_TEST_PATH.into()));
    }
    if stage < 12
        && tree
            .get("kernel/src/actuators/web.rs")
            .is_none_or(|raw| !web_search_source::search_process_violations(raw).is_empty())
    {
        found.insert(("f3-search", "kernel/src/actuators/web.rs".into()));
    }
    for stem in F7_ABSENT {
        for path in tree
            .keys()
            .filter(|p| within(p, stem) || *p == &format!("{stem}.rs"))
        {
            found.insert(("f7-absence", path.clone()));
        }
    }
    for row in stages().iter().filter(|r| r.stage <= stage) {
        for root in &row.roots {
            // Deny BOTH alternate Rust file/directory spellings, not just
            // the exact original file inventory.
            let stem = root.trim_end_matches(".rs");
            for path in tree
                .keys()
                .filter(|p| within(p, root) || within(p, stem) || *p == &format!("{stem}.rs"))
            {
                found.insert(("f7-absence", path.clone()));
            }
        }
    }
    // F7's retained guards and their security contacts survive until their
    // respective approved subject deletion, not just until the first cut.
    for (until, module) in [
        (19, "policy_engine"),
        (17, "time_machine"),
        (15, "speculative"),
        (19, "consent"),
        (19, "autonomy"),
        (14, "supervisor"),
    ] {
        if stage < until
            && tree
                .get("kernel/src/lib.rs")
                .and_then(|raw| coverage::root_module(raw, module))
                .is_none_or(|c| !c.attributes.is_empty() || c.head != format!("pubmod{module};"))
        {
            found.insert(("f7-presence", format!("kernel/src/lib.rs::{module}")));
        }
    }
    for (until, path, contact) in [
        (19, "kernel/src/consent.rs", "usecrate::policy_engine::"),
        (19, "kernel/src/autonomy.rs", "usecrate::policy_engine::"),
        (
            14,
            "kernel/src/supervisor.rs",
            "usecrate::time_machine::TimeMachine;",
        ),
    ] {
        if stage < until
            && tree
                .get(path)
                .is_none_or(|raw| !compact(&prod(raw).0).contains(contact))
        {
            found.insert(("f7-presence", path.into()));
        }
    }
    for (path, raw) in tree.iter() {
        // R-6: rustc owns cross-crate import/path resolution. Existing Mother
        // isolation guards protect live legacy modules before deletion.
        if !within(path, "kernel") || !path.ends_with(".rs") || tree.test_only.contains(path) {
            continue;
        }
        if within(path, "kernel") && !expected.contains(path) {
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
        let kernel_contacts = referenced_modules(&code, "nexus_kernel");
        let local_contacts = referenced_modules(&code, "crate");
        for row in stages().iter().filter(|r| r.stage <= stage) {
            for declaration in &row.declarations {
                let module = declaration
                    .trim_start_matches("pub mod ")
                    .trim_end_matches(';');
                if kernel_contacts.contains(module)
                    || (path.starts_with("kernel/") && local_contacts.contains(module))
                {
                    found.insert(("f7-dependency", path.clone()));
                }
                if path == "kernel/src/lib.rs"
                    && pattern(&format!(r"\bmod\s+{}\b", regex::escape(module))).is_match(&code)
                {
                    found.insert(("f7-absence", path.clone()));
                }
            }
        }
        // R-5: exact module inventories cover the retirement subject only.
        let allowed = module_edges(stage).get(path).cloned().unwrap_or_default();
        if within(path, "kernel") && !module_names(&code).is_subset(&allowed) {
            found.insert(("f7-dependency", path.clone()));
        }
        // The predecessor protects these subjects until each subject disappears.
        // Apply BEFORE the legacy exemption: K11 cannot retire the cognitive
        // walk while weakening the still-live actuators' prohibitions.
        if within(path, "kernel/src/cognitive") || within(path, "kernel/src/actuators") {
            const FORBIDDEN: [&str; 12] = [
                "free_api_registry",
                "orchestration::",
                "Orchestrator",
                "delegation::",
                "DelegationEngine",
                "DelegationConstraints",
                "speculative::",
                "SpeculativeEngine",
                "time_machine::",
                "TimeMachine",
                "policy_engine::",
                "PolicyEngine",
            ];
            // Retain the original code-line predicate as well as masked checks:
            // F7 also sees literal references on non-comment lines.
            for line in raw.lines().filter(|l| !l.trim_start().starts_with("//")) {
                if FORBIDDEN.iter().any(|token| {
                    line.contains(token)
                        && !(matches!(*token, "time_machine::" | "TimeMachine")
                            && path == "kernel/src/actuators/computer_use.rs")
                }) {
                    found.insert(("f7-dependency", path.clone()));
                }
            }
        }
        if path == "kernel/src/cognitive/algorithms/world_model.rs"
            && NO_HANDOFF
                .iter()
                .any(|word| identifier(&code, word) || identifier(&text, word))
        {
            found.insert(("hm-no-handoff", path.clone()));
        }
        if retiring(path, stage) {
            continue;
        } // Other legacy predicates remain under their unchanged current guards.
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
            || path.starts_with("kernel/src/resource_limiter/");
        if !process_boundary && pattern(r"\bCommand\s*::\s*new\s*\(").is_match(&code) {
            found.insert(("raw-process", path.clone()));
        }
        let network_boundary = path.starts_with("kernel/src/inference/");
        if !network_boundary
            && pattern(r"\b(?:reqwest|TcpStream|UdpSocket|TcpSocket)\s*::").is_match(&code)
        {
            found.insert(("raw-network", path.clone()));
        }
        let inference_boundary = path.starts_with("kernel/src/inference/");
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
        if identifier(&code, claim) && path != "kernel/src/emergency_stop.rs" {
            found.insert(("stop-seal", path.clone()));
        }
        if pattern(r"\bConnection\s*::\s*open(?:_with_flags|_in_memory)?\s*\(").is_match(&code) {
            found.insert(("hm-sqlite", path.clone()));
        }
        if squashed.contains("NexusDatabase::open(")
            && !hm_rows(stage)
                .iter()
                .any(|r| r.kind == "state-db-open" && r.path == *path)
        {
            found.insert(("hm-sqlite", path.clone()));
        }
        if stage >= 11 && !path.starts_with("kernel/src/hm") {
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
            if NO_HANDOFF
                .iter()
                .any(|word| identifier(&code, word) || identifier(&text, word))
            {
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
    // R-5: full retained kernel sink expressions/contexts remain frozen.
    let mut expected_logs: BTreeMap<String, Vec<LogSink>> = BTreeMap::new();
    for sink in logs() {
        expected_logs
            .entry(sink.path.clone())
            .or_default()
            .push(sink);
    }
    for (path, raw) in tree.iter().filter(|(p, _)| {
        within(p, "kernel")
            && p.ends_with(".rs")
            && !tree.test_only.contains(*p)
            && !retiring(p, stage)
    }) {
        let mut actual = logging_sinks(path, raw);
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
    let policies = required_test_policy();
    let mut actual_tests = BTreeMap::new();
    for path in &test_files {
        if let Some(raw) = tree.get(path) {
            for declaration in coverage::declarations(raw)
                .into_iter()
                .filter(|d| d.kind == "fn")
            {
                let name = format!("{path}::{}", declaration.name);
                if named_tests.contains(&name)
                    && actual_tests
                        .insert(name.clone(), declaration.contract)
                        .is_some()
                {
                    found.insert(("acceptance-inventory", name));
                }
            }
        }
    }
    for name in named_tests {
        if actual_tests.get(&name) != policies.get(&name) || !policies.contains_key(&name) {
            found.insert(("acceptance-inventory", name));
        }
    }
    for edge in test_edges() {
        if !tree.contains_key(&edge.target)
            || tree
                .get(&edge.source)
                .and_then(|raw| coverage::root_module(raw, &edge.module))
                .as_ref()
                != Some(&edge.contract)
        {
            found.insert(("acceptance-inventory", format!("{}::load", edge.target)));
        }
    }
    found
}

/// Effect-free staged slots plus representative RETAINED source. This is not a
/// simulated deletion build: actual stage PRs must still pass the live gate.
fn clean_tree(stage: u8) -> Tree {
    static CLEAN: std::sync::OnceLock<Vec<Tree>> = std::sync::OnceLock::new();
    CLEAN.get_or_init(|| (2..=22).map(build_clean_tree).collect())[usize::from(stage - 2)].clone()
}
/// R-8: live retained source copied into a historical synthetic stage loses
/// only the nested module edges registered by later, already-live extraction
/// rows, and their own new-home paths revert to the registered old home. Each
/// projected declaration must occur exactly once in its registered form;
/// anything else is left in place and judged at every stage.
fn project_live_source(stage: u8, path: &str, raw: String) -> String {
    let mut raw = raw;
    let mut projected = Vec::new();
    for row in stages()
        .iter()
        .filter(|r| r.stage > stage && r.stage <= LIVE_STAGE)
    {
        for extraction in &row.extract {
            let destination = extraction
                .destination
                .trim_start_matches("kernel/src/")
                .trim_end_matches(".rs");
            let Some((parent, child)) = destination.split_once('/') else {
                continue;
            };
            if path != format!("kernel/src/{parent}.rs") {
                continue;
            }
            let declaration = format!("pub mod {child};\n");
            if raw.contains(&declaration) {
                assert_eq!(
                    raw.matches(&declaration).count(),
                    1,
                    "K{}: {path} must declare {child} once",
                    row.stage
                );
                raw = raw.replacen(&format!("{declaration}\n"), "", 1);
                raw = raw.replacen(&declaration, "", 1);
            }
            let old = extraction
                .source
                .trim_start_matches("kernel/src/")
                .trim_end_matches(".rs")
                .replace('/', "::");
            for name in &extraction.payload {
                if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    raw = raw.replace(
                        &format!("crate::{parent}::{child}::{name}"),
                        &format!("crate::{old}::{name}"),
                    );
                }
            }
            projected.push((row.stage, parent, child));
        }
    }
    for (row, parent, child) in projected {
        assert!(
            !module_names(&raw).contains(child)
                && !raw.contains(&format!("crate::{parent}::{child}::")),
            "K{row}: {path} keeps an unprojected {child} edge"
        );
    }
    raw
}
fn build_clean_tree(stage: u8) -> Tree {
    let mut tree = Tree {
        files: paths_at(stage)
            .into_iter()
            .map(|p| (p, String::new()))
            .collect(),
        ..Tree::default()
    };
    // Cargo selects actual member manifests; required paths remain a minimum.
    for (path, raw) in dependencies::fixture_manifests() {
        tree.insert(path, raw);
    }
    for test in required_tests() {
        let (path, _) = test.rsplit_once("::").unwrap();
        tree.insert(path.into(), read(path));
    }
    if stage < 12 {
        tree.insert(WEB_TEST_PATH.into(), web_test_source(stage));
        tree.insert(
            "kernel/src/actuators/web.rs".into(),
            WEB_SOURCE_FIXTURE.into(),
        );
    }
    // Real loading declarations are needed to prove both test-only exclusions
    // and retained test reachability. Extraction imports use their same-type home.
    let mut lib = "pub mod audit; pub mod workspace_authority;".to_owned();
    for (until, module) in [
        (19, "policy_engine"),
        (17, "time_machine"),
        (15, "speculative"),
        (19, "consent"),
        (19, "autonomy"),
        (14, "supervisor"),
    ] {
        if stage < until {
            lib.push_str(&format!("pub mod {module};"));
        }
    }
    tree.insert("kernel/src/lib.rs".into(), lib);
    for (until, path, contact) in [
        (
            19,
            "kernel/src/consent.rs",
            "use crate::policy_engine::{EvaluationContext, PolicyDecision, PolicyEngine};",
        ),
        (
            19,
            "kernel/src/autonomy.rs",
            "use crate::policy_engine::{EvaluationContext, PolicyDecision, PolicyEngine};",
        ),
        (
            14,
            "kernel/src/supervisor.rs",
            "use crate::time_machine::TimeMachine;",
        ),
    ] {
        if stage < until {
            tree.insert(path.into(), contact.into());
        }
    }
    tree.insert(
        "kernel/src/workspace_authority.rs".into(),
        read("kernel/src/workspace_authority.rs")
            .replace("crate::manifest::", "crate::workspace_permissions::")
            .replace(
                "crate::supervisor::AgentId",
                "crate::agent_identity::AgentId",
            ),
    );
    for path in [
        "crates/nexus-verifier-sandbox/src/launcher.rs",
        "kernel/src/governed_path.rs",
        "kernel/src/coding_run/apply.rs",
        "crates/nexus-governed-control/src/files/mod.rs",
        "mother/src/coordinator/lite.rs",
    ] {
        tree.insert(
            path.into(),
            read(path).replace("crate::manifest::", "crate::workspace_permissions::"),
        );
    }
    for sink in logs() {
        let mut raw = project_live_source(stage, &sink.path, read(&sink.path));
        if stage >= 4 && sink.path == "kernel/src/config.rs" {
            for kind in [
                "BackupScheduleConfig",
                "RateLimitConfig",
                "ApiHardeningConfig",
            ] {
                let old = if kind == "BackupScheduleConfig" {
                    "backup"
                } else {
                    "rate_limit"
                };
                raw = raw.replace(
                    &format!("crate::{old}::{kind}"),
                    &format!("crate::config::schema_compat::{kind}"),
                );
            }
        }
        tree.insert(sink.path.clone(), raw);
    }
    tree.prove_fixture();
    tree
}
fn rejected(stage: u8, tree: &Tree, oracle: &'static str, subject: &str) {
    // Clean stage inputs are immutable (like clean_tree); cache their findings,
    // never a mutated tree's reachability proof or findings.
    static BASELINES: std::sync::OnceLock<Vec<Findings>> = std::sync::OnceLock::new();
    let baseline = &BASELINES
        .get_or_init(|| (2..=22).map(|s| successor(s, &clean_tree(s))).collect())
        [usize::from(stage - 2)];
    assert!(baseline.is_empty(), "K{stage}: baseline {baseline:?}");
    // All in-memory changes invalidate the old proof. Recompute from the
    // mutated fixture, including changed module/Cargo loading edges.
    let mut current = tree.clone();
    current.prove_fixture();
    let findings = successor(stage, &current);
    let added: Findings = findings.difference(baseline).cloned().collect();
    assert!(
        added.contains(&(oracle, subject.into())),
        "K{stage}: expected new {oracle} at {subject}, added {added:?}"
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
            rejected(
                row.stage,
                &tree,
                "f7-absence",
                &format!("{}/restored.rs", root.trim_end_matches(".rs")),
            );
        }
        for declaration in &row.declarations {
            let mut tree = clean_tree(row.stage);
            tree.entry("kernel/src/lib.rs".into())
                .or_default()
                .push_str(declaration);
            rejected(row.stage, &tree, "f7-absence", "kernel/src/lib.rs");
        }
        for export in &row.reexports {
            let mut tree = clean_tree(row.stage);
            tree.insert("kernel/src/cognitive/mod.rs".into(), export.clone());
            rejected(
                row.stage,
                &tree,
                "f7-absence",
                "kernel/src/cognitive/mod.rs",
            );
        }
    }
}

#[test]
fn renamed_runtime_file_and_inline_module_cannot_return() {
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
        ] {
            let mut tree = clean_tree(stage);
            tree.entry(path.into()).or_default().push_str(plant);
            rejected(stage, &tree, "f7-dependency", path);
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
            "stop-seal",
            "kernel/src/privacy.rs",
            concat!("fn control(){outside::claim_global_", "controller();}"),
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
        rejected(22, &tree, oracle, path);
    }
    for route in NO_HANDOFF {
        let mut tree = clean_tree(22);
        tree.entry("kernel/src/privacy.rs".into())
            .or_default()
            .push_str(&format!("fn handoff(){{outside::{route}(content);}}"));
        rejected(22, &tree, "hm-no-handoff", "kernel/src/privacy.rs");
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
    rejected(22, &Tree::new(), "omission", "kernel/src/lib.rs");
    for path in [
        "kernel/src/lib.rs",
        "kernel/src/hm.rs",
        "kernel/src/emergency_stop.rs",
        "mother/src/lib.rs",
    ] {
        let mut tree = clean_tree(22);
        assert!(tree.remove(path).is_some());
        rejected(22, &tree, "omission", path);
    }
    for name in required_tests() {
        let (path, function) = name.rsplit_once("::").unwrap();
        let mut tree = clean_tree(22);
        let raw = tree.get_mut(path).unwrap();
        *raw = raw.replace(&format!("fn {function}()"), "fn omitted()");
        rejected(22, &tree, "acceptance-inventory", &name);
    }
    let sink = logs().remove(0);
    let mut tree = clean_tree(22);
    tree.get_mut(&sink.path).unwrap().clear();
    rejected(22, &tree, "logging", &sink.path);
}

#[test]
fn cargo_manifest_omission_refuses_reachability_and_reports_the_subject() {
    let mut tree = clean_tree(22);
    let baseline = successor(22, &tree);
    assert!(baseline.is_empty());
    let subject = "kernel/Cargo.toml";
    tree.remove(subject).unwrap();
    let fixture = tree.write_fixture();
    // This plant makes the scanner refuse to establish reachability. Require
    // that exact error, keep every exclusion invalidated, and separately require
    // the successor's omission finding at the planted manifest.
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.prove_test_only_in(fixture.path());
    }))
    .unwrap_err();
    let message = error.downcast_ref::<String>().unwrap();
    assert_eq!(
        message,
        &format!(
            "source scan: workspace member lacks Cargo.toml: {}",
            fixture.path().join("kernel").display()
        )
    );
    assert!(tree.test_only.is_empty());
    let added: Findings = successor(22, &tree)
        .difference(&baseline)
        .cloned()
        .collect();
    assert!(added.contains(&("omission", subject.into())), "{added:?}");
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
        rejected(12, &tree, oracle, path);
    }
}
