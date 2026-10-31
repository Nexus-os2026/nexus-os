//! M0-FIX F7 oracle: the cognitive loop is cut from the legacy supervisor
//! stack, and the modules nothing real used afterwards are gone.
//!
//! A narrow source guard over the cognitive loop (`kernel/src/cognitive/`)
//! and the actuators it reaches through the registry
//! (`kernel/src/actuators/`), plus the kernel's module list. It reads code
//! lines only (a `//` comment line is skipped); it is not a Rust parser or a
//! data-flow audit. The behaviour half of the oracle (the executor refuses
//! each removed action) is the unit test
//! `m0_f7_registry_executor_refuses_removed_legacy_actions` in
//! `kernel/src/cognitive/loop_runtime.rs`.
//!
//! Kept on purpose, and so not forbidden here:
//! - `policy_engine`: the Cedar policy check consulted by `consent.rs` and
//!   `autonomy.rs` (a guard).
//! - `time_machine`: the undo/rollback record written by `Supervisor` and by
//!   the computer-use actuator before it acts (a guard), and the grant-bound
//!   replay with its guard tests. Only `computer_use.rs` may name it here.
//! - `speculative`: the approval preview inside `Supervisor`'s consent API;
//!   removing it changes the approval path, which is an Owner decision.

use std::path::{Path, PathBuf};

fn kernel_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// The code lines of `text`: a line whose first non-blank characters are
/// `//` is a comment and is skipped.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim_start().starts_with("//"))
        .map(|(i, l)| (i + 1, l))
}

/// Every `(file, line, token)` in the cognitive loop and its actuators that
/// names a removed or cut module, except the one kept computer-use
/// checkpoint reference.
fn legacy_references(files: &[(String, String)]) -> Vec<(String, usize, &'static str)> {
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
    // The computer-use actuator persists an undo/rollback checkpoint before
    // it acts; that record is a kept guard.
    let allowed = |token: &str, rel: &str| {
        matches!(token, "time_machine::" | "TimeMachine")
            && rel == "kernel/src/actuators/computer_use.rs"
    };
    let mut found = Vec::new();
    for (rel, text) in files {
        for (line_no, line) in code_lines(text) {
            for token in FORBIDDEN {
                if line.contains(token) && !allowed(token, rel) {
                    found.push((rel.clone(), line_no, token));
                }
            }
        }
    }
    found
}

fn cognitive_loop_files() -> Vec<(String, String)> {
    let root = kernel_root();
    let mut paths = Vec::new();
    rust_files(&root.join("src/cognitive"), &mut paths);
    rust_files(&root.join("src/actuators"), &mut paths);
    assert!(
        paths.len() > 20,
        "the walk found the cognitive loop: {}",
        paths.len()
    );
    paths
        .into_iter()
        .map(|p| {
            let rel = format!(
                "kernel/{}",
                p.strip_prefix(&root).unwrap().to_string_lossy()
            );
            (rel, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

#[test]
fn m0_f7_cognitive_loop_names_no_legacy_supervisor_module() {
    let files = cognitive_loop_files();
    // Planted controls: each kind of reference is caught, in a code line,
    // in any file of the loop; a comment line is not a reference; the
    // computer-use exception covers only `time_machine` in that one file.
    let planted = |rel: &str, text: &str| legacy_references(&[(rel.into(), text.into())]);
    for text in [
        "    if crate::free_api_registry::is_known_free_api(url) {",
        "use crate::orchestration::orchestrator::Orchestrator;",
        "let e = crate::delegation::DelegationEngine::new();",
        "use crate::speculative::SpeculativeEngine;",
        "use crate::time_machine::{Checkpoint, TimeMachine};",
        "use crate::policy_engine::PolicyEngine;",
    ] {
        assert!(
            !planted("kernel/src/cognitive/planner.rs", text).is_empty(),
            "{text}"
        );
        assert!(
            !planted("kernel/src/actuators/shell.rs", text).is_empty(),
            "{text}"
        );
    }
    assert!(planted(
        "kernel/src/cognitive/planner.rs",
        "// crate::free_api_registry was removed (M0-FIX F7)"
    )
    .is_empty());
    assert!(planted(
        "kernel/src/actuators/computer_use.rs",
        "use crate::time_machine::{Checkpoint, TimeMachine};"
    )
    .is_empty());
    assert!(!planted(
        "kernel/src/actuators/computer_use.rs",
        "use crate::speculative::SpeculativeEngine;"
    )
    .is_empty());

    let found = legacy_references(&files);
    assert!(
        found.is_empty(),
        "the cognitive loop still names the legacy supervisor stack: {found:#?}"
    );
    // The actuators that existed only to drive the stack are gone.
    for rel in [
        "src/actuators/self_evolution.rs",
        "src/actuators/governance_policy.rs",
        "src/actuators/cognitive_param.rs",
    ] {
        assert!(!kernel_root().join(rel).exists(), "{rel} still exists");
    }
}

#[test]
fn m0_f7_unused_legacy_modules_are_deleted_and_guards_kept() {
    let root = kernel_root();
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let declared =
        |module: &str| code_lines(&lib).any(|(_, l)| l.trim() == format!("pub mod {module};"));
    for module in ["free_api_registry", "orchestration", "delegation"] {
        assert!(!declared(module), "lib.rs still declares {module}");
        assert!(
            !root.join(format!("src/{module}.rs")).exists()
                && !root.join(format!("src/{module}")).exists(),
            "src/{module} still exists"
        );
    }
    // The guards stay: the cut must not take them with it.
    for module in [
        "policy_engine",
        "time_machine",
        "speculative",
        "consent",
        "autonomy",
        "supervisor",
    ] {
        assert!(declared(module), "lib.rs no longer declares {module}");
    }
    let consent = std::fs::read_to_string(root.join("src/consent.rs")).unwrap();
    assert!(consent.contains("use crate::policy_engine::"));
    let autonomy = std::fs::read_to_string(root.join("src/autonomy.rs")).unwrap();
    assert!(autonomy.contains("use crate::policy_engine::"));
    let supervisor = std::fs::read_to_string(root.join("src/supervisor.rs")).unwrap();
    assert!(supervisor.contains("use crate::time_machine::TimeMachine;"));
}
