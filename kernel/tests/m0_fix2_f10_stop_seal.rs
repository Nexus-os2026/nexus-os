//! M0-FIX2 F10 oracle (source half): only the owner's Stop/Resume path can
//! lift the process's emergency stop.
//!
//! The seal is a type-level one (see `kernel/src/emergency_stop.rs`):
//! lifting is a method of `StopController` only, a controller is not
//! `Clone`, `Copy` or `Default` and has no public constructor, and the
//! process's (global) authority mints its controller once, through
//! `StopAuthority::claim_global_controller`. This guard pins that shape and
//! pins who claims the global controller: in production source only the
//! governed-control front door (`crates/nexus-governed-control/src/
//! governed.rs`, the owner's `stop_for`/`resume` path); elsewhere only the
//! named test binaries, which are never part of a production build.
//!
//! It is a narrow text guard, not a Rust parser; the compile-fail doctests
//! in `emergency_stop.rs` are its type-level half.

use std::path::{Path, PathBuf};

const EMERGENCY_STOP_RS: &str = include_str!("../src/emergency_stop.rs");
const GOVERNED_RS: &str = include_str!("../../crates/nexus-governed-control/src/governed.rs");

/// The only files that may name `claim_global_controller(`: its
/// definition, the owner's Stop/Resume path, this guard, and test binaries
/// that drive the process's own authority (each its own process).
const MAY_CLAIM_THE_GLOBAL_CONTROLLER: &[&str] = &[
    "kernel/src/emergency_stop.rs",
    "crates/nexus-governed-control/src/governed.rs",
    "kernel/tests/m0_fix2_f10_stop_seal.rs",
    "kernel/tests/m0_f5_emergency_stop.rs",
    "connectors/llm/tests/m0_f5_stop.rs",
    "crates/nexus-governed-control/tests/m0_fix2_f10_owner_stop.rs",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root")
        .to_path_buf()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == "node_modules" || name == ".git" {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn normalized(source: &str) -> String {
    source.replace("\r\n", "\n")
}

/// The header (`impl X {`) of the top-level `impl` block that `offset`
/// sits inside: the nearest preceding one, if it is still open there.
fn enclosing_impl(source: &str, offset: usize) -> Option<&str> {
    let before = &source[..offset];
    let at = before.rfind("\nimpl")?;
    let line_end = before[at + 1..].find('\n').map(|i| at + 1 + i)?;
    if before[line_end..].contains("\n}\n") {
        // That block closed before `offset`: not inside any impl.
        return None;
    }
    Some(before[at + 1..line_end].trim())
}

#[test]
fn m0_fix2_f10_the_stop_authority_itself_cannot_lift() {
    let source = normalized(EMERGENCY_STOP_RS);
    assert!(
        !source.contains("pub fn lift("),
        "an unconditional public `lift` remains on the stop authority"
    );
    let mut lift_ifs = 0;
    let mut from = 0;
    while let Some(at) = source[from..].find("fn lift_if(") {
        let offset = from + at;
        from = offset + 1;
        lift_ifs += 1;
        let owner = enclosing_impl(&source, offset).unwrap_or("<none>");
        assert_eq!(
            owner, "impl StopController {",
            "`lift_if` is reachable from `{owner}`, not only from the controller"
        );
    }
    assert_eq!(lift_ifs, 1, "exactly one `lift_if`, on the controller");
}

#[test]
fn m0_fix2_f10_the_controller_cannot_be_copied_or_made() {
    let source = normalized(EMERGENCY_STOP_RS);
    let at = source
        .find("\npub struct StopController")
        .expect("a `StopController` type holds the right to lift");
    // The attributes directly above the struct.
    let head: Vec<&str> = source[..at]
        .lines()
        .rev()
        .take_while(|line| line.starts_with("#[") || line.starts_with("///"))
        .collect();
    for line in head {
        for banned in ["Clone", "Copy", "Default"] {
            assert!(
                !(line.starts_with("#[derive") && line.contains(banned)),
                "the controller derives {banned}: {line}"
            );
        }
    }
    for banned in [
        "impl Clone for StopController",
        "impl Default for StopController",
        "impl Copy for StopController",
    ] {
        assert!(!source.contains(banned), "{banned}");
    }
    // Its fields are private: no `pub` field inside the struct body.
    let body_start = at + source[at..].find('{').expect("a struct body");
    let body_end = body_start + source[body_start..].find("\n}").expect("its end");
    assert!(
        !source[body_start..body_end].contains("pub "),
        "the controller has a public field"
    );
}

#[test]
fn m0_fix2_f10_only_the_owner_path_claims_the_global_controller() {
    let root = workspace_root();
    let mut files = Vec::new();
    for dir in [
        "kernel",
        "connectors",
        "crates",
        "agents",
        "sdk",
        "persistence",
    ] {
        rust_sources(&root.join(dir), &mut files);
    }
    assert!(
        files.len() > 100,
        "the scan found too few sources: {}",
        files.len()
    );
    let mut claimants = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        if text.contains("claim_global_controller(") {
            let rel = file
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            claimants.push(rel);
        }
    }
    for claimant in &claimants {
        assert!(
            MAY_CLAIM_THE_GLOBAL_CONTROLLER.contains(&claimant.as_str()),
            "{claimant} claims the process's stop controller"
        );
    }
    // The owner's Stop/Resume path holds it (production construction).
    let governed = normalized(GOVERNED_RS);
    assert!(
        governed.contains("StopAuthority::claim_global_controller()"),
        "the governed-control front door does not claim the global controller"
    );
    assert!(
        claimants
            .iter()
            .any(|c| c == "crates/nexus-governed-control/src/governed.rs"),
        "{claimants:?}"
    );
}
