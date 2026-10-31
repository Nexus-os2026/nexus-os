//! Source scans that read the tree from disk (test code; it lives in the
//! adapter because only the adapter module touches the host).

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if matches!(
                name.as_str(),
                "target" | ".git" | "node_modules" | "dist" | ".cargo" | ".claude"
            ) {
                continue;
            }
            rust_files(&path, out);
        } else if kind.is_file() && name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// Every Rust file of the crate, as `(path relative to the crate, text)`.
fn mother_sources() -> Vec<(String, String)> {
    let root = crate_root();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            (rel, std::fs::read_to_string(&path).unwrap())
        })
        .collect()
}

fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn in_adapter(rel: &str) -> bool {
    rel.starts_with("src/adapter/")
}

#[test]
fn scan_the_tree_is_what_the_scans_expect() {
    let files: Vec<String> = mother_sources().into_iter().map(|(rel, _)| rel).collect();
    assert!(
        files.contains(&"src/adapter/mod.rs".to_string()),
        "{files:?}"
    );
    assert!(files.contains(&"examples/walk.rs".to_string()), "{files:?}");
    assert!(files.len() >= 20, "{files:?}");
}

#[test]
fn scan_no_host_access_outside_the_adapter() {
    // Built from parts so this file does not match itself.
    let needles: Vec<String> = [
        ("std::", "fs"),
        ("std::", "net"),
        ("std::", "process"),
        ("std::", "os::"),
        ("req", "west"),
        ("cu", "rl"),
        ("Command", "::new"),
        ("Tcp", "Stream"),
        ("Tcp", "Listener"),
        ("Udp", "Socket"),
        ("File", "::open"),
        ("File", "::create"),
        ("OpenOptions", ""),
        ("lib", "c::"),
        ("tok", "io"),
        ("include_", "bytes!"),
        ("unsafe ", "{"),
        ("unsafe ", "fn"),
        ("unsafe ", "impl"),
        ("extern ", "\"C\""),
    ]
    .iter()
    .map(|(a, b)| format!("{a}{b}"))
    .collect();
    let mut checked = 0;
    for (rel, text) in mother_sources() {
        if in_adapter(&rel) {
            continue;
        }
        checked += 1;
        let text = code(&text);
        for needle in &needles {
            assert!(!text.contains(needle.as_str()), "{rel}: {needle}");
        }
    }
    assert!(checked >= 15, "{checked}");
}

#[test]
fn scan_no_kernel_or_legacy_stack_dependency() {
    let needles: Vec<String> = [
        ("nexus_", "kernel"),
        ("nexus-", "kernel"),
        ("Code", "Execute"),
        ("Actu", "ator"),
        ("actu", "ator"),
        ("Super", "visor"),
        ("super", "visor"),
        ("nexus_", "connectors"),
        ("nexus_", "sdk"),
        ("nexus_", "persistence"),
        ("nexus_", "self_improve"),
        ("Shell", "Command"),
        ("governed_", "shell"),
    ]
    .iter()
    .map(|(a, b)| format!("{a}{b}"))
    .collect();
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml")).unwrap();
    for needle in &needles {
        assert!(!manifest.contains(needle.as_str()), "Cargo.toml: {needle}");
    }
    for (rel, text) in mother_sources() {
        let text = code(&text);
        for needle in &needles {
            assert!(!text.contains(needle.as_str()), "{rel}: {needle}");
        }
    }
}

/// Each `fn confirm_mission` body in `text`, without whitespace.
fn confirm_bodies(text: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("fn confirm_mission(") {
        let after = &rest[at..];
        let open = after.find('{').unwrap();
        let mut depth = 0;
        let mut end = open;
        for (i, c) in after[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        bodies.push(
            after[open + 1..end]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect(),
        );
        rest = &after[end..];
    }
    bodies
}

/// Owner condition (a) on M1: no production code implements the consent
/// confirmer to always answer yes, and none implements it inside `mother/`.
/// Only test code may provide a fake confirmer. The one Owner-approved
/// exception is the walk example, whose confirmer must ask the Owner at the
/// terminal (it reads stdin) and never answer a literal yes.
#[test]
fn scan_no_production_consent_confirmer() {
    let needle = format!("{}{}", "MissionConfirmer", " for");
    let test_only =
        |rel: &str| rel.starts_with("src/oracles/") || rel == "src/adapter/real_broker.rs";
    let mut walk_impls = 0;
    for (rel, text) in mother_sources() {
        let text = code(&text);
        let impls = text.matches(needle.as_str()).count();
        if impls == 0 || test_only(&rel) {
            continue;
        }
        assert_eq!(
            rel, "examples/walk.rs",
            "a consent confirmer in Mother's production code"
        );
        walk_impls += impls;
        for body in confirm_bodies(&text) {
            assert!(
                body.contains("stdin"),
                "the walk's confirmer must ask the Owner: {body}"
            );
            assert_ne!(body, "true", "an always-yes confirmer");
            assert!(!body.ends_with(";true"), "an always-yes confirmer: {body}");
        }
    }
    assert_eq!(
        walk_impls, 1,
        "the walk example's terminal confirmer, exactly once"
    );

    // The rest of the workspace: any implementation outside test code must
    // not answer a literal yes.
    let workspace = crate_root().parent().unwrap().to_path_buf();
    let mut files = Vec::new();
    rust_files(&workspace, &mut files);
    for path in files {
        if path.starts_with(crate_root()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(needle.as_str()) {
            continue;
        }
        let rel = path
            .strip_prefix(&workspace)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let test_code = rel.split('/').any(|part| part == "tests") || rel.ends_with("tests.rs");
        if test_code {
            continue;
        }
        for body in confirm_bodies(&code(&text)) {
            assert_ne!(body, "true", "{rel}: an always-yes confirmer");
            assert!(!body.ends_with(";true"), "{rel}: an always-yes confirmer");
        }
    }
}

#[test]
fn scan_the_confirmer_scan_recognises_an_always_yes_body() {
    // A negative control for the scan's parser.
    let fake = "impl X for Y { fn confirm_mission(&self, _: &C) -> bool {\n  true\n} }";
    assert_eq!(confirm_bodies(fake), vec!["true".to_string()]);
    let asks = "fn confirm_mission(&self, r: &C) -> bool { let mut l = String::new(); std::io::stdin().read_line(&mut l).is_ok() && l.trim() == r.hash.short() }";
    let bodies = confirm_bodies(asks);
    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].contains("stdin"));
}
