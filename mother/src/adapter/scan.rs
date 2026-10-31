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

/// Owner condition (a) on M1, kept by M2a: no production code implements a
/// consent confirmer that always answers yes. Since M2a the confirmer is the
/// broker's `ControlConfirmer` (`confirm_mission`), and Mother's own trait is
/// gone. Inside `mother/` only test code implements it, plus the one
/// Owner-approved stand-in in the walk example, whose mission dialog asks
/// the Owner at the terminal (it reads stdin), never answers a literal yes,
/// and declines every other dialog. In the broker's production code every
/// `confirm_mission` (the trait's own answer included) is a literal no.
#[test]
fn scan_no_production_consent_confirmer() {
    let needle = format!("{}{}", "ControlConfirmer", " for");
    let gone = format!("{}{}", "Mission", "Confirmer");
    let test_only =
        |rel: &str| rel.starts_with("src/oracles/") || rel == "src/adapter/real_broker.rs";
    let mut walk_impls = 0;
    for (rel, text) in mother_sources() {
        let text = code(&text);
        assert!(!text.contains(gone.as_str()), "{rel}: Mother's own confirmer");
        let impls = text.matches(needle.as_str()).count();
        if impls == 0 || test_only(&rel) {
            continue;
        }
        assert_eq!(
            rel, "examples/walk.rs",
            "a consent confirmer in Mother's production code"
        );
        walk_impls += impls;
        let bodies = confirm_bodies(&text);
        assert_eq!(bodies.len(), 1, "the walk asks about missions once");
        for body in bodies {
            assert!(
                body.contains("stdin"),
                "the walk's confirmer must ask the Owner: {body}"
            );
            assert_ne!(body, "true", "an always-yes confirmer");
            assert!(!body.ends_with(";true"), "an always-yes confirmer: {body}");
        }
        for other in [
            "confirm_action",
            "confirm_grant",
            "confirm_resume",
            "confirm_acknowledgement",
        ] {
            let bodies = bodies_of(&text, other);
            assert_eq!(bodies, vec!["false".to_string()], "walk: {other}");
        }
    }
    assert_eq!(
        walk_impls, 1,
        "the walk example's terminal confirmer, exactly once"
    );

    // The rest of the workspace: outside test code, no confirmer answers a
    // literal yes to a mission; in the broker, every answer is no.
    let workspace = crate_root().parent().unwrap().to_path_buf();
    let broker = workspace.join("crates").join("nexus-governed-control");
    let mut files = Vec::new();
    rust_files(&workspace, &mut files);
    let mut broker_answers = 0;
    for path in files {
        if path.starts_with(crate_root()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains("confirm_mission") {
            continue;
        }
        let rel = path
            .strip_prefix(&workspace)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let test_code = rel
            .split('/')
            .any(|part| part == "tests" || part.ends_with("_tests"))
            || rel.ends_with("tests.rs");
        if test_code {
            continue;
        }
        for body in confirm_bodies(&code(&text)) {
            assert_ne!(body, "true", "{rel}: an always-yes confirmer");
            assert!(!body.ends_with(";true"), "{rel}: an always-yes confirmer");
            if path.starts_with(&broker) {
                assert_eq!(body, "false", "{rel}: a broker confirmer that may say yes");
                broker_answers += 1;
            }
        }
    }
    assert!(
        broker_answers >= 1,
        "the broker's ControlConfirmer::confirm_mission default was not found"
    );
}

/// Each `fn <name>(` body in `text`, without whitespace.
fn bodies_of(text: &str, name: &str) -> Vec<String> {
    let marker = format!("fn {name}(");
    let mut bodies = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(marker.as_str()) {
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

#[test]
fn scan_the_body_scan_reads_each_dialog_separately() {
    let text = "impl Dialogs for W {\n fn confirm_action(&self, _: &A) -> bool { false }\n fn confirm_mission(&self, r: &M) -> bool { ask(std::io::stdin()) }\n fn confirm_grant(&self, _: &G) -> bool {\n true\n }\n}";
    assert_eq!(bodies_of(text, "confirm_action"), vec!["false".to_string()]);
    assert_eq!(bodies_of(text, "confirm_grant"), vec!["true".to_string()]);
    assert_eq!(bodies_of(text, "confirm_mission"), confirm_bodies(text));
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
