//! Source scans that read the tree from disk (test code; it lives in the
//! adapter because only the adapter module touches the host).

use std::path::{Path, PathBuf};

/// G0 (M2b-A): the shared desktop tripwire.
#[path = "../../../kernel/tests/desktop_tripwire/mod.rs"]
mod desktop_tripwire;

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

/// M2b-A (G10): the desktop is host code. Its tests are test code; its
/// production sources (`desktop/src/`) are scanned like Mother's, with
/// exactly the exceptions below.
fn in_desktop_tests(rel: &str) -> bool {
    rel.starts_with("desktop/tests/")
}

/// M2d-D G4/G5: Cargo integration targets and these explicitly registered
/// unit-test files only. Production module declarations are checked below.
fn in_hands_tests(rel: &str) -> bool {
    rel.starts_with("host/tests/")
        || matches!(
            rel,
            "host/src/terminal_tests.rs" | "host/src/hands_tests.rs" | "host/src/stop_tests.rs"
        )
}

/// M2d-D G3: safe borrowed descriptor access in the terminal, and a typed
/// binary return. No process, filesystem, networking or FFI exemption.
fn without_hands_items(rel: &str, text: &str) -> String {
    let allowed: &[&str] = match rel {
        "host/src/terminal.rs" | "host/src/legacy_terminal.rs" => {
            &["use std::os::fd::{AsFd, AsRawFd};"]
        }
        "host/src/bin/mother-hands.rs" => &["use std::process::ExitCode;"],
        _ => &[],
    };
    let mut text = text.to_string();
    for item in allowed {
        text = text.replace(item, "");
    }
    text
}

#[test]
fn scan_m2dd_safe_items_are_exact() {
    let fd = "use std::os::fd::{AsFd, AsRawFd};";
    assert_eq!(without_hands_items("host/src/terminal.rs", fd), "");
    assert_eq!(without_hands_items("host/src/legacy_terminal.rs", fd), "");
    assert_eq!(without_hands_items("host/src/hands.rs", fd), fd);
    let exit = "use std::process::ExitCode;";
    assert_eq!(
        without_hands_items("host/src/bin/mother-hands.rs", exit),
        ""
    );
    assert_eq!(without_hands_items("host/src/terminal.rs", exit), exit);
    for denied in ["std::process::Command", "std::fs::File", "std::os::raw"] {
        assert_eq!(without_hands_items("host/src/terminal.rs", denied), denied);
    }
    assert!(in_hands_tests("host/tests/pty.rs"));
    assert!(!in_hands_tests("host/src/test_support.rs"));
    assert!(!in_hands_tests("host/src/bin/tests.rs"));
}

/// The only kernel items the desktop's production code may name: the
/// identity home (its state directory) and the kernel Stop's reads.
const DESKTOP_KERNEL_PATHS: [&str; 2] = [
    concat!("nexus_", "kernel::identity_home::nexus_state_path"),
    concat!("nexus_", "kernel::emergency_stop::StopAuthority"),
];

/// `text` with the desktop's allowed kernel paths removed; any other
/// kernel reference stays and is caught.
fn without_desktop_kernel_paths(text: &str) -> String {
    let mut text = text.to_string();
    for path in DESKTOP_KERNEL_PATHS {
        text = text.replace(path, "");
    }
    text
}

/// The only host item the desktop's production code may name: its
/// binary's exit code, in `desktop/src/main.rs`.
fn without_desktop_exit_code(rel: &str, text: &str) -> String {
    if rel == "desktop/src/main.rs" {
        text.replace(concat!("std::", "process::ExitCode"), "")
    } else {
        text.to_string()
    }
}

#[test]
fn scan_m2b_the_desktop_exceptions_are_exact() {
    let kernel = concat!("nexus_", "kernel");
    let planted = format!("use {kernel}::identity_home::nexus_state_path;\nuse {kernel}::emergency_stop::StopAuthority;\nuse {kernel}::emergency_stop::StopController;");
    let left = without_desktop_kernel_paths(&planted);
    assert_eq!(left.matches(kernel).count(), 1, "{left}");
    let exit = concat!("std::", "process::ExitCode");
    let other = concat!("std::", "process::exit");
    assert!(!without_desktop_exit_code("desktop/src/main.rs", exit).contains("std::"));
    assert!(without_desktop_exit_code("desktop/src/main.rs", other).contains("std::"));
    assert!(without_desktop_exit_code("desktop/src/app.rs", exit).contains("std::"));
    let files: Vec<String> = mother_sources().into_iter().map(|(rel, _)| rel).collect();
    for rel in [
        "desktop/src/lib.rs",
        "desktop/src/main.rs",
        "desktop/src/confirm.rs",
    ] {
        assert!(files.contains(&rel.to_string()), "{rel}: {files:?}");
    }
}

#[test]
fn scan_the_tree_is_what_the_scans_expect() {
    let files: Vec<String> = mother_sources().into_iter().map(|(rel, _)| rel).collect();
    assert!(
        files.contains(&"src/adapter/mod.rs".to_string()),
        "{files:?}"
    );
    // M2c-A (A8): the walk lives in the host crate (N2).
    assert!(
        files.contains(&"host/examples/walk.rs".to_string()),
        "{files:?}"
    );
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
        if in_adapter(&rel) || in_desktop_tests(&rel) || in_hands_tests(&rel) {
            continue;
        }
        checked += 1;
        let text = without_hands_items(&rel, &without_desktop_exit_code(&rel, &code(&text)));
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
        if in_desktop_tests(&rel) {
            continue;
        }
        let text = code(&text);
        let text = if rel.starts_with("desktop/src/") {
            without_desktop_kernel_paths(&text)
        } else {
            text
        };
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
    let test_only = |rel: &str| {
        rel.starts_with("src/oracles/")
            || rel == "src/adapter/real_broker.rs"
            || in_desktop_tests(rel)
            || in_hands_tests(rel)
    };
    let mut walk_impls = 0;
    let mut desktop_impls = 0;
    let mut terminal_impls = 0;
    let mut legacy_terminal_impls = 0;
    for (rel, text) in mother_sources() {
        let text = code(&text);
        assert!(
            !text.contains(gone.as_str()),
            "{rel}: Mother's own confirmer"
        );
        let impls = text.matches(needle.as_str()).count();
        if impls == 0 || test_only(&rel) {
            continue;
        }
        if rel == "host/src/legacy_terminal.rs" {
            assert!(
                walk_terminal_confirmer(&rel, &text),
                "walk terminal contract"
            );
            legacy_terminal_impls += impls;
            continue;
        }
        if rel == "host/src/terminal.rs" {
            assert!(
                hands_confirmer(&rel, &text),
                "{rel}: terminal confirmer contract"
            );
            terminal_impls += impls;
            continue;
        }
        // M2b-A (G10): the desktop's one confirmer answers no to all seven
        // questions: it has no dialogs yet.
        if rel.starts_with("desktop/") {
            assert_eq!(rel, "desktop/src/confirm.rs", "a confirmer in the desktop");
            assert_eq!(impls, 1, "{rel}");
            desktop_impls += impls;
            for dialog in [
                "confirm_action",
                "confirm_grant",
                "confirm_resume",
                "confirm_acknowledgement",
                "confirm_mission",
                "confirm_agent_door",
                "confirm_held_mission",
            ] {
                let bodies = bodies_of(&text, dialog);
                assert_eq!(bodies, vec!["false".to_string()], "desktop: {dialog}");
            }
            continue;
        }
        // M2c-A (A8): the walk lives in the host crate (N2).
        assert_eq!(
            rel, "host/examples/walk.rs",
            "a consent confirmer in Mother's production code"
        );
        walk_impls += impls;
        // M2d-D G2: exactly the three original permissions, now through
        // the shared, freshness-checked terminal implementation.
        for dialog in [
            "confirm_mission",
            "confirm_agent_door",
            "confirm_held_mission",
        ] {
            let bodies = bodies_of(&text, dialog);
            assert_eq!(
                bodies,
                vec![format!("self.0.{dialog}(request)")],
                "walk: {dialog}"
            );
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
    assert_eq!(desktop_impls, 1, "the desktop's confirmer, exactly once");
    assert_eq!(
        terminal_impls, 1,
        "M2d-D: exactly one session terminal confirmer"
    );
    assert_eq!(
        legacy_terminal_impls, 1,
        "exactly one isolated walk terminal confirmer"
    );
    // The eighth method stays denied by default outside Hands.
    for rel in ["desktop/src/confirm.rs", "host/examples/walk.rs"] {
        let text = code(&std::fs::read_to_string(crate_root().join(rel)).unwrap());
        assert!(
            bodies_of(&text, "confirm_undo")
                .iter()
                .all(|b| b == "false"),
            "{rel}: undo"
        );
    }

    // The rest of the workspace: outside test code, no confirmer answers a
    // literal yes to a mission; in the broker, every answer is no.
    let workspace = crate_root().parent().unwrap().to_path_buf();
    let broker = workspace.join("crates").join("nexus-governed-control");
    let mut files = Vec::new();
    rust_files(&workspace, &mut files);
    let mut broker_answers = 0;
    let mut dialog_defaults = 0;
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
        let text = code(&text);
        for body in confirm_bodies(&text) {
            assert_ne!(body, "true", "{rel}: an always-yes confirmer");
            assert!(!body.ends_with(";true"), "{rel}: an always-yes confirmer");
        }
        // Correction after the first run (see the commit): in the broker,
        // the confirmers are the `ControlConfirmer` trait (its default
        // answer) and its implementations; the broker's own entry points
        // named `confirm_mission` ask a confirmer and are not one.
        // M2c-A (A8): nor to a door or a held mission.
        for dialog in ["confirm_agent_door", "confirm_held_mission"] {
            for block in confirmer_blocks(&text) {
                for body in bodies_of(&block, dialog) {
                    assert_ne!(body, "true", "{rel}: an always-yes {dialog}");
                    assert!(!body.ends_with(";true"), "{rel}: an always-yes {dialog}");
                    if path.starts_with(&broker) {
                        assert_eq!(body, "false", "{rel}: a broker {dialog} that may say yes");
                        dialog_defaults += 1;
                    }
                }
            }
        }
        if path.starts_with(&broker) {
            for block in confirmer_blocks(&text) {
                for body in confirm_bodies(&block) {
                    assert_eq!(body, "false", "{rel}: a broker confirmer that may say yes");
                    broker_answers += 1;
                }
            }
        }
    }
    assert!(
        broker_answers >= 1,
        "the broker's ControlConfirmer::confirm_mission default was not found"
    );
    assert!(
        dialog_defaults >= 2,
        "the broker's door and held-mission defaults were not found"
    );
}

/// G1/G6: pin the implementation type, count, path and every delegation.
/// Behavior is independently exercised by the scripted and PTY oracles.
fn hands_confirmer(rel: &str, text: &str) -> bool {
    let implementation = ["impl", "ControlConfirmer", "for", "TerminalConfirmer"].join(" ");
    let needle = ["ControlConfirmer", "for"].join(" ");
    if rel != "host/src/terminal.rs"
        || text.matches(&needle).count() != 1
        || !text.contains(&implementation)
    {
        return false;
    }
    for (method, delegate) in [
        ("confirm_action", "action"),
        ("confirm_grant", "grant"),
        ("confirm_mission", "mission"),
        ("confirm_agent_door", "door"),
        ("confirm_undo", "undo"),
    ] {
        if bodies_of(text, method) != vec![format!("self.{delegate}(request)")] {
            return false;
        }
    }
    [
        "confirm_acknowledgement",
        "confirm_resume",
        "confirm_held_mission",
    ]
    .iter()
    .all(|method| bodies_of(text, method) == vec!["false".to_string()])
}

#[test]
fn scan_m2dd_terminal_confirmer_negative_controls() {
    let prefix = ["impl", "ControlConfirmer", "for", "TerminalConfirmer"].join(" ");
    let mut fixture = format!("{prefix} {{");
    for (method, delegate) in [
        ("action", "action"),
        ("grant", "grant"),
        ("mission", "mission"),
        ("agent_door", "door"),
        ("undo", "undo"),
    ] {
        fixture.push_str(&format!(
            "fn confirm_{method}(&self, request: &R) -> bool {{ self.{delegate}(request) }}"
        ));
    }
    fixture.push_str("fn confirm_acknowledgement(&self, _: &R) -> bool { false } fn confirm_resume(&self, _: &R) -> bool { false } fn confirm_held_mission(&self, _: &R) -> bool { false }}");
    assert!(hands_confirmer("host/src/terminal.rs", &fixture));
    assert!(!hands_confirmer("host/src/hands.rs", &fixture));
    assert!(!hands_confirmer(
        "host/src/terminal.rs",
        &format!("{fixture}{fixture}")
    ));
    assert!(!hands_confirmer(
        "host/src/terminal.rs",
        &fixture.replace("TerminalConfirmer", "Other")
    ));
    for delegate in ["action", "grant", "mission", "door", "undo"] {
        assert!(!hands_confirmer(
            "host/src/terminal.rs",
            &fixture.replace(&format!("self.{delegate}(request)"), "true")
        ));
    }
    assert!(!hands_confirmer(
        "host/src/terminal.rs",
        &fixture.replace("false", "true")
    ));
}

#[test]
fn scan_m2dd_test_helpers_cannot_be_production_modules() {
    for (rel, text) in mother_sources() {
        if !rel.starts_with("host/src/") || in_hands_tests(&rel) {
            continue;
        }
        for (name, test_only) in declared_modules(&code(&text)) {
            if matches!(
                name.as_str(),
                "terminal_tests" | "hands_tests" | "stop_tests"
            ) {
                assert!(test_only, "{rel}: {name} must be cfg(test)");
            }
        }
        assert!(
            !text.contains("#[path"),
            "host production modules use ordinary checked declarations: {rel}"
        );
        assert!(
            !text.contains("include!("),
            "no inclusion of test code: {rel}"
        );
    }
}

/// The text of each `trait ControlConfirmer` and `impl … ControlConfirmer
/// for …` block in `text`.
fn confirmer_blocks(text: &str) -> Vec<String> {
    let trait_of = format!("{} {}", "trait", "ControlConfirmer");
    let impl_of = format!("{}{}", "ControlConfirmer", " for ");
    let mut blocks = Vec::new();
    for needle in [trait_of, impl_of] {
        let mut rest = text;
        while let Some(at) = rest.find(needle.as_str()) {
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
            blocks.push(after[open..=end].to_string());
            rest = &after[end..];
        }
    }
    blocks
}

#[test]
fn scan_the_confirmer_blocks_are_the_trait_and_its_impls() {
    let t = ["trait", "ControlConfirmer"].join(" ");
    let c = "ControlConfirmer";
    let text = format!(
        "pub {t} {{\n fn confirm_mission(&self, _: &M) -> bool {{ false }}\n}}\nimpl Other {{ pub fn confirm_mission(&self, r: &M, c: &dyn X) -> R {{ ask(r, c) }} }}\nimpl {c} for Yes {{ fn confirm_mission(&self, _: &M) -> bool {{ true }} }}"
    );
    let bodies: Vec<String> = confirmer_blocks(&text)
        .iter()
        .flat_map(|b| confirm_bodies(b))
        .collect();
    assert_eq!(bodies, vec!["false".to_string(), "true".to_string()]);
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

// ---------------------------------------------------------------- M2c-A A8

/// The modules a file declares, each with whether its declaration is
/// under `#[cfg(test)]` (the line before it, comments and blanks aside).
fn declared_modules(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut previous = "";
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let declaration = ["mod ", "pub mod ", "pub(crate) mod ", "pub(super) mod "]
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix))
            .and_then(|rest| rest.strip_suffix(';'));
        if let Some(name) = declaration {
            out.push((name.trim().to_string(), previous == "#[cfg(test)]"));
        }
        previous = line;
    }
    out
}

/// Every module of `mother/src` that is compiled outside tests: from
/// `lib.rs`, each module not declared under `#[cfg(test)]`, recursively.
fn production_modules() -> Vec<(String, String)> {
    let src = crate_root().join("src");
    let mut out = Vec::new();
    let mut todo = vec![(src.join("lib.rs"), src.clone())];
    while let Some((file, dir)) = todo.pop() {
        let text = std::fs::read_to_string(&file).unwrap();
        for (name, test_only) in declared_modules(&code(&text)) {
            if test_only {
                continue;
            }
            let flat = dir.join(format!("{name}.rs"));
            let nested = dir.join(&name).join("mod.rs");
            let child = if flat.exists() { flat } else { nested };
            todo.push((child, dir.join(&name)));
        }
        let rel = file
            .strip_prefix(crate_root())
            .unwrap()
            .to_string_lossy()
            .to_string();
        out.push((rel, text));
    }
    out.sort();
    out
}

#[test]
fn scan_m2c_the_module_parser_reads_cfg_test_declarations() {
    let text = "pub mod adapter;\n#[cfg(test)]\nmod test_home;\n// mod commented;\n#[cfg(test)]\n\npub(crate) mod support;\nmod plain;";
    assert_eq!(
        declared_modules(text),
        vec![
            ("adapter".to_string(), false),
            ("test_home".to_string(), true),
            ("support".to_string(), true),
            ("plain".to_string(), false),
        ]
    );
}

/// A8 (design v3 §3 E7, N2): no module of Mother compiled outside tests
/// constructs a broker or reaches the front door. The host crate holds the
/// only production construction; this scan is an extra layer beside the
/// type-level absence of every such path (A1).
#[test]
fn scan_m2c_no_broker_construction_in_mother_s_production_modules() {
    let modules = production_modules();
    let names: Vec<&str> = modules.iter().map(|(rel, _)| rel.as_str()).collect();
    for expected in [
        "src/lib.rs",
        "src/adapter/mod.rs",
        "src/coordinator.rs",
        "src/effect.rs",
    ] {
        assert!(names.contains(&expected), "{names:?}");
    }
    assert!(
        !names
            .iter()
            .any(|n| n.contains("oracles") || n.contains("test_home") || n.contains("m2c_")),
        "{names:?}"
    );
    let needles: Vec<String> = [
        ("GovernedControl", "::new"),
        ("Authority", "::new"),
        ("PrimaryEvidence", "::open"),
        ("Tools", "::production"),
        ("issue_agent", "_door"),
        (".authority", "()"),
        (".control", "()"),
    ]
    .iter()
    .map(|(a, b)| format!("{a}{b}"))
    .collect();
    for (rel, text) in &modules {
        let text = code(text);
        for needle in &needles {
            assert!(!text.contains(needle.as_str()), "{rel}: {needle}");
        }
    }
}

/// A8: the port acts through its door.
#[test]
fn scan_m2c_the_port_acts_through_its_door() {
    let adapter = code(&std::fs::read_to_string(crate_root().join("src/adapter/mod.rs")).unwrap());
    for needle in ["door.agent_action(", "door.cancel_run(", "door.open_run("] {
        assert!(adapter.contains(needle), "adapter: {needle}");
    }
}

/// G0 (M2b-A PR-1): Mother's tree reaches no desktop or webview package
/// except through the registered `nexus-mother-desktop`, in the resolved
/// graph, with every feature configuration.
#[test]
fn scan_m2b_no_desktop_outside_the_registered_package() {
    desktop_tripwire::assert_no_desktop();
}

// The legacy walk keeps exactly its historical three dialogues. It cannot
// confirm Files, grants, native resume, acknowledgement or undo.
fn walk_terminal_confirmer(rel: &str, text: &str) -> bool {
    let implementation = ["impl", "ControlConfirmer", "for", "WalkConfirmer"].join(" ");
    let needle = ["ControlConfirmer", "for"].join(" ");
    rel == "host/src/legacy_terminal.rs"
        && text.matches(&needle).count() == 1
        && text.contains(&implementation)
        && [
            ("confirm_mission", "mission"),
            ("confirm_agent_door", "door"),
            ("confirm_held_mission", "held"),
        ]
        .iter()
        .all(|(method, delegate)| {
            bodies_of(text, method) == vec![format!("self.{delegate}(request)")]
        })
        && [
            "confirm_action",
            "confirm_grant",
            "confirm_resume",
            "confirm_acknowledgement",
            "confirm_undo",
        ]
        .iter()
        .all(|method| bodies_of(text, method) == vec!["false".to_string()])
}

#[test]
fn scan_isolated_walk_confirmer_keeps_only_legacy_permissions() {
    let prefix = ["impl", "ControlConfirmer", "for", "WalkConfirmer"].join(" ");
    let mut fixture = format!("{prefix} {{");
    for (method, delegate) in [
        ("mission", "mission"),
        ("agent_door", "door"),
        ("held_mission", "held"),
    ] {
        fixture.push_str(&format!(
            "fn confirm_{method}(&self, request: &R) -> bool {{ self.{delegate}(request) }}"
        ));
    }
    for method in ["action", "grant", "resume", "acknowledgement", "undo"] {
        fixture.push_str(&format!(
            "fn confirm_{method}(&self, _: &R) -> bool {{ false }}"
        ));
    }
    fixture.push('}');
    assert!(walk_terminal_confirmer(
        "host/src/legacy_terminal.rs",
        &fixture
    ));
    for path in [
        "host/src/terminal.rs",
        "host/src/hands.rs",
        "host/src/other.rs",
    ] {
        assert!(!walk_terminal_confirmer(path, &fixture));
    }
    for (old, new) in [
        ("WalkConfirmer", "Other"),
        ("false", "true"),
        ("self.mission(request)", "true"),
        ("self.door(request)", "true"),
        ("self.held(request)", "true"),
    ] {
        assert!(!walk_terminal_confirmer(
            "host/src/legacy_terminal.rs",
            &fixture.replace(old, new)
        ));
    }
    assert!(!walk_terminal_confirmer(
        "host/src/legacy_terminal.rs",
        &format!("{fixture}{fixture}")
    ));
}

#[test]
fn scan_hands_services_cannot_import_the_walk_or_raw_authority() {
    for rel in [
        "host/src/hands.rs",
        "host/src/terminal.rs",
        "host/src/stop.rs",
        "host/src/session_home.rs",
    ] {
        let text = code(&std::fs::read_to_string(crate_root().join(rel)).unwrap());
        for forbidden in [
            "legacy_terminal",
            "legacy_stop",
            "GovernedControl",
            "GovernedPort",
            ".owner()",
            "Home::",
        ] {
            assert!(!text.contains(forbidden), "{rel}: {forbidden}");
        }
    }
}

// G/F10: typed Hands answers reach the same fixed terminal implementation;
// adding typed callbacks does not create an unguarded authority route.
fn hands_typed_answers(text: &str) -> bool {
    [
        ("confirm_grant_answer", "grant"),
        ("confirm_agent_door_answer", "door"),
        ("confirm_mission_answer", "mission"),
    ]
    .iter()
    .all(|(method, delegate)| {
        bodies_of(text, method)
            == vec![format!(
                "self.{delegate}(request);self.confirmation_answer()"
            )]
    })
}

#[test]
fn scan_m2dd_typed_confirmation_exact_delegates_and_negative_controls() {
    let mut fixture = String::new();
    for (method, delegate) in [
        ("grant", "grant"),
        ("agent_door", "door"),
        ("mission", "mission"),
    ] {
        fixture.push_str(&format!("fn confirm_{method}_answer(&self, request: &R) -> ConfirmationAnswer {{ self.{delegate}(request); self.confirmation_answer() }}"));
    }
    assert!(hands_typed_answers(&fixture));
    for delegate in ["grant", "door", "mission"] {
        assert!(!hands_typed_answers(&fixture.replace(
            &format!("self.{delegate}(request); self.confirmation_answer()"),
            "ConfirmationAnswer::Affirmative",
        )));
        assert!(!hands_typed_answers(&fixture.replace(
            &format!("self.{delegate}(request); self.confirmation_answer()"),
            "ConfirmationAnswer::Declined",
        )));
    }
    assert!(!hands_typed_answers(&format!("{fixture}{fixture}")));
    let text = code(&std::fs::read_to_string(crate_root().join("host/src/terminal.rs")).unwrap());
    assert!(hands_confirmer("host/src/terminal.rs", &text));
    assert!(
        hands_typed_answers(&text),
        "typed answers must keep the real terminal's exact result"
    );
}
