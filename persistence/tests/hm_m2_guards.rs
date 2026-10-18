//! P3-C11-HM-M2 N7: workspace source guards for the historical-memory
//! authority core (P3-HM-CONTRACT-02 §5.1, §8 item 4, §11.0, AD-4).
//!
//! Each guard scans the workspace sources and has planted negative controls
//! proving it detects what it forbids. Planted strings are assembled with
//! `concat!` so this file never contains a forbidden pattern itself.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Every workspace source file with one of `exts`, outside build output,
/// dependencies and VCS metadata.
fn source_files(exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![workspace_root()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !matches!(name.as_str(), "target" | "node_modules" | ".git" | "dist") {
                    stack.push(path);
                }
            } else if kind.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.contains(&e))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap()
        .display()
        .to_string()
}

/// Whitespace collapsed to single spaces, lower case.
fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A table token with quotes, escapes, brackets and a `main.` schema
/// removed (source text holds SQL inside string literals, so a quoted
/// identifier can appear as `\"hm_x\"`).
fn bare_table(token: &str) -> String {
    let quote = |c: char| matches!(c, '"' | '`' | '[' | ']' | '\'' | '\\');
    let t = token.trim_matches(quote);
    let t = t.strip_prefix("main.").unwrap_or(t);
    t.trim_matches(quote).to_string()
}

/// The first token after each occurrence of `phrase` in squashed text.
fn tokens_after(squashed: &str, phrase: &str) -> Vec<String> {
    squashed
        .match_indices(phrase)
        .map(|(i, m)| {
            let token: String = squashed[i + m.len()..]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '(')
                .collect();
            bare_table(&token)
        })
        .collect()
}

fn is_test_path(rel: &str) -> bool {
    rel.contains("/tests/")
        || rel.ends_with("_tests.rs")
        || rel.ends_with("/tests.rs")
        || rel.starts_with("tests/")
}

// ── Guard 1: no REPLACE write to any hm_* table (§5.1) ──────────────────────

/// The targets of every REPLACE write (`replace into`, `insert or replace
/// into`, `update or replace`) that name an hm_* table, or a table only
/// known at run time (`{…}`), or nothing.
fn replace_hits(squashed: &str) -> Vec<String> {
    let mut targets = tokens_after(squashed, "replace into ");
    targets.extend(tokens_after(squashed, "update or replace "));
    targets
        .into_iter()
        .filter(|t| t.starts_with("hm_") || t.starts_with('{') || t.is_empty())
        .collect()
}

#[test]
fn hm_m2_n7_no_replace_write_targets_an_hm_table() {
    // Planted negative controls.
    let planted = squash(concat!(
        "conn.execute(\"INSERT OR RE",
        "PLACE\n INTO hm_event VALUES (1)\")"
    ));
    assert_eq!(replace_hits(&planted), vec!["hm_event"]);
    let planted = squash(concat!(
        "\"RE",
        "PLACE INTO main.\\\"hm_grant\\\" (a) VALUES (1)\""
    ));
    assert_eq!(replace_hits(&planted), vec!["hm_grant"]);
    let planted = squash(concat!(
        "UPDATE OR RE",
        "PLACE hm_record_label SET origin = 'x'"
    ));
    assert_eq!(replace_hits(&planted), vec!["hm_record_label"]);
    let planted = squash(concat!("format!(\"RE", "PLACE INTO {table} VALUES (?1)\")"));
    assert_eq!(replace_hits(&planted), vec!["{table}"]);
    assert!(replace_hits(&squash("SELECT replace(key, 'a', 'b') FROM hm_event")).is_empty());

    let files = source_files(&["rs", "sql"]);
    assert!(files.len() > 100, "workspace scan found too few files");
    let mut hits = Vec::new();
    for f in &files {
        let rel = relative(f);
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let found = replace_hits(&squash(&text));
        if rel == "persistence/tests/hm_a1_storage.rs" {
            // A1's own REPLACE guard: its diagnostic message names a
            // run-time `{target}` and its negative control plants
            // `{table}`. Neither names an hm_* table; pinned exactly.
            assert_eq!(found, vec!["{target}\",", "{table}"], "{rel}");
            continue;
        }
        for t in found {
            hits.push(format!("{rel}: REPLACE {t}"));
        }
    }
    assert!(hits.is_empty(), "REPLACE write on an hm_* table: {hits:#?}");

    // The historical-memory modules use no REPLACE conflict resolution at
    // all, in DDL or DML.
    let mut hm_files = vec![
        workspace_root().join("persistence/src/hm.rs"),
        workspace_root().join("kernel/src/hm.rs"),
    ];
    for e in std::fs::read_dir(workspace_root().join("kernel/src/hm")).unwrap() {
        hm_files.push(e.unwrap().path());
    }
    for f in hm_files {
        let s = squash(&std::fs::read_to_string(&f).unwrap());
        for p in ["on conflict replace", "or replace", "replace into"] {
            assert!(!s.contains(p), "{}: {p}", relative(&f));
        }
    }
}

// ── Guard 2: no hm_grant row is ever inserted (AD-4) ────────────────────────

/// Every insert or replace whose target is `hm_grant`.
fn grant_inserts(squashed: &str) -> usize {
    squashed
        .match_indices("into ")
        .filter(|(i, m)| {
            let token: String = squashed[i + m.len()..]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '(')
                .collect();
            let before = &squashed[i.saturating_sub(32)..*i];
            bare_table(&token) == "hm_grant"
                && (before.contains("insert") || before.contains("replace"))
        })
        .count()
}

#[test]
fn hm_m2_n7_no_insert_into_hm_grant() {
    // Planted negative controls.
    assert_eq!(
        grant_inserts(&squash(concat!("INSERT INTO hm", "_grant VALUES (1)"))),
        1
    );
    assert_eq!(
        grant_inserts(&squash(concat!(
            "insert or ignore\n into \"hm",
            "_grant\" (grant_id) values (?1)"
        ))),
        1
    );
    assert_eq!(
        grant_inserts(&squash(concat!("INSERT INTO main.hm", "_grant SELECT 1"))),
        1
    );
    assert_eq!(grant_inserts(&squash("SELECT COUNT(*) FROM hm_grant")), 0);
    assert_eq!(
        grant_inserts(&squash("INSERT INTO hm_grants_archive VALUES (1)")),
        0
    );

    let mut hits = Vec::new();
    for f in source_files(&["rs", "sql"]) {
        let rel = relative(&f);
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let n = grant_inserts(&squash(&text));
        if rel == "persistence/tests/hm_a1_storage.rs" {
            // A1's synthetic-database tests of the §5.1 hm_grant CHECK and
            // trigger rules (three inserts) and its table-name scanner
            // control (one string). Pinned: any further insert fails.
            assert_eq!(n, 4, "{rel}");
            continue;
        }
        if n > 0 {
            hits.push(format!("{rel}: {n}"));
        }
    }
    assert!(hits.is_empty(), "grant row inserts: {hits:#?}");
}

// ── Guard 3: HmStore / HmTx used only by the hm modules (§11.0, A-S3) ───────

fn hm_store_user_allowed(rel: &str) -> bool {
    rel == "persistence/src/hm.rs"
        || rel == "kernel/src/hm.rs"
        || rel.starts_with("kernel/src/hm/")
        || (rel.starts_with("kernel/tests/hm_") && rel.ends_with(".rs"))
        || (rel.starts_with("persistence/tests/hm_") && rel.ends_with(".rs"))
}

/// References to the store handle, its transaction or its module.
fn hm_store_refs(text: &str) -> usize {
    [".hm()", "HmStore", "HmTx", "persistence::hm"]
        .iter()
        .map(|p| text.matches(p).count())
        .sum()
}

#[test]
fn hm_m2_n7_hm_store_is_called_only_from_the_hm_modules() {
    // Planted negative controls.
    let planted = concat!("let s = db.h", "m().immediate(|tx| Ok(()));");
    assert_eq!(hm_store_refs(planted), 1);
    assert_eq!(
        hm_store_refs(concat!("use nexus_persistence::h", "m::{H", "mTx};")),
        2
    );
    assert!(!hm_store_user_allowed(
        "kernel/src/cognitive/loop_runtime.rs"
    ));
    assert!(!hm_store_user_allowed("app/src-tauri/src/lib.rs"));
    assert!(!hm_store_user_allowed("kernel/src/hmx.rs"));
    assert!(hm_store_user_allowed("kernel/src/hm/write.rs"));

    let mut hits = Vec::new();
    for f in source_files(&["rs"]) {
        let rel = relative(&f);
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let n = hm_store_refs(&text);
        if n == 0 || hm_store_user_allowed(&rel) {
            continue;
        }
        if rel == "persistence/src/lib.rs" {
            // Only the accessor that hands the handle out.
            assert_eq!(n, 2, "{rel}");
            assert!(text.contains(
                "pub fn hm(&self) -> hm::HmStore<'_> {\n        hm::HmStore::new(self)\n    }"
            ));
            continue;
        }
        hits.push(format!("{rel}: {n}"));
    }
    assert!(
        hits.is_empty(),
        "HmStore/HmTx used outside the hm modules: {hits:#?}"
    );
}

// ── Guard 4: every identity-home nexus.db connection sets foreign_keys=ON ───

/// The body of `fn <name>` in `src`, up to its `migrate()` call.
fn constructor_until_migrate<'a>(src: &'a str, signature: &str) -> Option<&'a str> {
    let start = src.find(signature)?;
    let end = src[start..].find("db.migrate()")?;
    Some(&src[start..start + end])
}

fn enables_foreign_keys(body: &str) -> bool {
    squash(body)
        .replace(' ', "")
        .contains("pragmaforeign_keys=on")
}

fn raw_opens(text: &str) -> usize {
    [
        "Connection::open(",
        "Connection::open_with_flags(",
        "Connection::open_in_memory(",
    ]
    .iter()
    .map(|p| text.matches(p).count())
    .sum()
}

/// Non-test files that open their own SQLite file directly, with the file
/// each one opens. None of them is the identity-home `nexus.db`.
const RAW_OPEN_FILES: [(&str, &str); 6] = [
    (
        "persistence/src/lib.rs",
        "NexusDatabase::open / in_memory (foreign keys on)",
    ),
    ("persistence/src/coding_run_ledger.rs", "coding-runs.db"),
    (
        "crates/nexus-memory/src/persistence.rs",
        "<data_dir>/memory.db",
    ),
    (
        "crates/nexus-memory/src/audit.rs",
        "<data_dir>/memory_audit.db",
    ),
    ("metering/src/store.rs", "metering.db"),
    ("marketplace/src/sqlite_registry.rs", "marketplace.db"),
];

/// Non-test files that resolve the identity-home `nexus.db` path.
const IDENTITY_DB_CALLERS: [&str; 7] = [
    "kernel/src/identity_home.rs",
    "kernel/src/cognitive/loop_runtime.rs",
    "kernel/src/actuators/knowledge_graph.rs",
    "agents/social-poster/src/lib.rs",
    "app/src-tauri/src/lib.rs",
    "app/src-tauri/src/commands/swarm.rs",
    "app/src-tauri/src/commands/chat_llm.rs",
];

/// The connections to the state database: contract §8 item 4, plus
/// `knowledge_graph.rs:38` and `agents/social-poster/src/lib.rs:503`,
/// which A1 found missing from that list.
const STATE_DB_OPENERS: [&str; 9] = [
    "app/src-tauri/src/lib.rs",
    "kernel/src/persistence.rs",
    "kernel/src/actuators/cognitive_param.rs",
    "kernel/src/actuators/self_evolution.rs",
    "kernel/src/actuators/agent_lifecycle.rs",
    "app/src-tauri/src/commands/swarm.rs",
    "kernel/src/cognitive/loop_runtime.rs",
    "kernel/src/actuators/knowledge_graph.rs",
    "agents/social-poster/src/lib.rs",
];

#[test]
fn hm_m2_n7_every_identity_home_connection_enables_foreign_keys() {
    // Planted negative controls.
    let without = concat!(
        "pub fn open(path: &Path) -> Result<Self> {\n let conn = Connection::open(path)?;\n",
        " conn.execute_batch(\"PRAGMA journal_mode=WAL;\")?;\n db.migrate()?;"
    );
    assert!(!enables_foreign_keys(
        constructor_until_migrate(without, "pub fn open(").unwrap()
    ));
    let with = concat!(
        "pub fn open(path: &Path) -> Result<Self> {\n conn.execute_batch(\"PRAGMA foreign_keys",
        " = ON;\")?;\n db.migrate()?;"
    );
    assert!(enables_foreign_keys(
        constructor_until_migrate(with, "pub fn open(").unwrap()
    ));
    assert_eq!(
        raw_opens(concat!(
            "let c = rusqlite::Connection::op",
            "en(nexus_db_path()?)?;"
        )),
        1
    );

    let root = workspace_root();
    let lib = std::fs::read_to_string(root.join("persistence/src/lib.rs")).unwrap();
    // FK-1: both NexusDatabase constructors enable foreign keys before
    // migrating, and they are lib.rs's only raw opens.
    for sig in [
        "pub fn open(path: &Path) -> Result<Self> {",
        "pub fn in_memory() -> Result<Self> {",
    ] {
        let body = constructor_until_migrate(&lib, sig).unwrap_or_else(|| panic!("{sig}"));
        assert!(enables_foreign_keys(body), "{sig}");
        assert_eq!(raw_opens(body), 1, "{sig}");
    }
    assert_eq!(
        raw_opens(&lib),
        2,
        "lib.rs opens only in its two constructors"
    );

    // FK-2: every non-test raw open is classified, and none of those files
    // names the identity-home database.
    let mut unclassified = Vec::new();
    let mut identity_callers = Vec::new();
    for f in source_files(&["rs"]) {
        let rel = relative(&f);
        if is_test_path(&rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let names_identity_db = text.contains("nexus_db_path");
        if names_identity_db {
            identity_callers.push(rel.clone());
        }
        if raw_opens(&text) > 0 {
            if !RAW_OPEN_FILES.iter().any(|(p, _)| *p == rel) {
                unclassified.push(rel.clone());
            }
            if rel != "persistence/src/lib.rs" {
                assert!(
                    !names_identity_db,
                    "{rel}: raw open in a file naming nexus.db"
                );
                assert!(!text.contains("\"nexus.db\""), "{rel}");
            }
        }
    }
    assert!(
        unclassified.is_empty(),
        "unclassified raw SQLite opens: {unclassified:#?}"
    );

    // FK-3: the identity-home path is resolved only where it is opened
    // through NexusDatabase (or, in chat_llm, only removed as a legacy
    // file); a new caller fails until classified.
    identity_callers.sort();
    let mut expected: Vec<String> = IDENTITY_DB_CALLERS.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(identity_callers, expected);

    // FK-4: every state-database connection site, including the two A1
    // found, opens through NexusDatabase and so enables foreign keys.
    for rel in STATE_DB_OPENERS {
        let text = std::fs::read_to_string(root.join(rel)).unwrap();
        assert!(text.contains("NexusDatabase::open("), "{rel}");
        assert_eq!(raw_opens(&text), 0, "{rel}");
    }
}
