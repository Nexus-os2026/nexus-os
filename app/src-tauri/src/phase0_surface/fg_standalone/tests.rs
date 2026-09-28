//! P0-FINAL-GATE-CLOSURE guards for items J2 to J5 (standalone surfaces).
//!
//! The protocols server (`nexus-protocols-server`, J2) and its `nexus-os`
//! alias (J3), `nexus-cli` (J4) and the standalone `nx` terminal (J5) are
//! withdrawn, together with the alternate `coding-agent` and
//! `social-poster-agent` entry points, the `nx-*` computer-use harness
//! (coordinator decisions D1 and D2) and the five benchmarks that sent a
//! provider key on curl's command line (coordinator decision after internal
//! review), on the pattern of `crates/nexus-server` (J1). Each package's
//! `tests/phase0_withdrawal.rs` runs its withdrawn executables and pins their
//! sources and recipes. The guards here are workspace-wide:
//!
//! - every effective binary (and example) target of the workspace is
//!   inventoried with its disposition, so a new or changed entry point fails
//!   until it is reviewed, and every withdrawn one is a J1-pattern withdrawal;
//! - no production source outside the withdrawn surface's own library
//!   reaches its entry APIs (no alternate alias);
//! - the repository's deployment and packaging recipes are the inventoried,
//!   withdrawn ones, and no workflow builds, installs or publishes a withdrawn
//!   binary or a container image or chart.
//!
//! These guards read sources only; they run nothing. The libraries behind the
//! withdrawn surfaces stay (the desktop uses some of them) and are not
//! claimed governed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── Effective binary and example targets ────────────────────────────────────

/// What a binary target is, and why it may exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Disposition {
    /// A J1-pattern withdrawal: one fixed message, status 69, nothing else.
    Withdrawn,
    /// The desktop application itself.
    Desktop,
    /// A developer tool that no recipe ships (coordinator decision D3).
    Developer,
    /// A benchmark that no recipe ships (coordinator decision D3).
    Benchmark,
}

use Disposition::{Benchmark, Desktop, Developer, Withdrawn};

/// Every effective binary target of the workspace: (package directory,
/// binary name, source path within the package, disposition).
const BINARY_TARGETS: &[(&str, &str, &str, Disposition)] = &[
    (
        "agents/coding-agent",
        "coding-agent",
        "src/main.rs",
        Withdrawn,
    ),
    (
        "agents/social-poster",
        "social-poster-agent",
        "src/main.rs",
        Withdrawn,
    ),
    (
        "app/src-tauri",
        "nexus-desktop-backend",
        "src/main.rs",
        Desktop,
    ),
    (
        "benchmarks/conductor-bench",
        "audit-retention-bench",
        "src/audit_retention_bench.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "audit-throughput-bench",
        "src/audit_throughput_bench.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "cloud-models-bench",
        "src/cloud_models_bench.rs",
        Withdrawn,
    ),
    (
        "benchmarks/conductor-bench",
        "conductor-bench",
        "src/main.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "darwin-drift-bench",
        "src/darwin_drift_bench.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "genesis-protocol-bench",
        "src/genesis_protocol_bench.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "inference-consistency-bench",
        "src/inference_consistency_bench.rs",
        Withdrawn,
    ),
    (
        "benchmarks/conductor-bench",
        "local-vs-cloud-battle",
        "src/local_vs_cloud_battle.rs",
        Withdrawn,
    ),
    (
        "benchmarks/conductor-bench",
        "memory-profile",
        "src/memory_profile.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "multiagent-coordination-bench",
        "src/multiagent_coordination_bench.rs",
        Benchmark,
    ),
    (
        "benchmarks/conductor-bench",
        "nim-cloud-bench",
        "src/nim_cloud_bench.rs",
        Withdrawn,
    ),
    (
        "benchmarks/conductor-bench",
        "real-agent-validation",
        "src/real_agent_validation.rs",
        Withdrawn,
    ),
    (
        "benchmarks/conductor-bench",
        "real-battery-validation",
        "src/real_battery_validation.rs",
        Benchmark,
    ),
    ("cli", "nexus-cli", "src/main.rs", Withdrawn),
    (
        "crates/nexus-computer-use",
        "nx-agent",
        "src/bin/agent_test.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-computer-use",
        "nx-govern",
        "src/bin/governance_test.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-computer-use",
        "nx-input",
        "src/bin/input_test.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-computer-use",
        "nx-learn",
        "src/bin/learn_test.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-computer-use",
        "nx-screen",
        "src/bin/screen_test.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-server",
        "nexus-server",
        "src/main.rs",
        Withdrawn,
    ),
    (
        "crates/nexus-swarm",
        "nexus-swarm-healthcheck",
        "src/bin/healthcheck.rs",
        Developer,
    ),
    (
        "crates/nexus-ui-repair",
        "nexus-ui-repair",
        "src/bin/nexus_ui_repair.rs",
        Developer,
    ),
    (
        "crates/nexus-ui-repair",
        "scout",
        "src/bin/scout.rs",
        Developer,
    ),
    (
        "crates/nexus-ui-repair",
        "sg5_probe",
        "src/bin/sg5_probe.rs",
        Developer,
    ),
    ("nexus-code", "nx", "src/main.rs", Withdrawn),
    ("protocols", "nexus-os", "src/bin/nexus-os.rs", Withdrawn),
    (
        "protocols",
        "nexus-protocols-server",
        "src/bin/nexus-server.rs",
        Withdrawn,
    ),
];

/// Every Cargo example (`cargo run --example`) of the workspace: developer
/// entry points that no recipe ships.
const EXAMPLE_TARGETS: &[(&str, &str, &str)] = &[
    ("kernel", "dump_config", "examples/dump_config.rs"),
    ("kernel", "generate_genomes", "examples/generate_genomes.rs"),
];

/// Every Cargo bench target (`cargo bench`) of the workspace: the criterion
/// harnesses of `benchmarks` (`harness = false`, so each has its own
/// `main`). They are kept as benchmarks that no recipe ships (coordinator
/// decision D3), and they run in process (see the bench guard below).
const BENCH_TARGETS: &[(&str, &str, &str)] = &[
    ("benchmarks", "agent_bench", "benches/agent_bench.rs"),
    ("benchmarks", "gateway_bench", "benches/gateway_bench.rs"),
    ("benchmarks", "kernel_bench", "benches/kernel_bench.rs"),
    ("benchmarks", "phase67_bench", "benches/phase67_bench.rs"),
    ("benchmarks", "replay_bench", "benches/replay_bench.rs"),
];

/// The `members` of the workspace manifest.
fn workspace_members() -> Vec<String> {
    let manifest = read(&workspace_root().join("Cargo.toml"));
    let mut members = Vec::new();
    let mut inside = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with("members = [") {
            inside = true;
            continue;
        }
        if inside {
            if line.starts_with(']') {
                break;
            }
            let member = line.trim_end_matches(',').trim_matches('"');
            if !member.is_empty() {
                members.push(member.to_string());
            }
        }
    }
    assert!(members.len() > 50, "workspace members not found");
    members
}

/// A manifest's `[package]` value for `key`, if it is set there.
fn package_value(manifest: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(value) = line.strip_prefix(&format!("{key} = ")) {
                return Some(value.trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Explicit `[[section]]` targets of a manifest as (name, optional path).
fn explicit_targets(manifest: &str, section: &str) -> Vec<(String, Option<String>)> {
    let header = format!("[[{section}]]");
    let mut targets = Vec::new();
    let mut current: Option<(String, Option<String>)> = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            if let Some(target) = current.take() {
                targets.push(target);
            }
            if line == header {
                current = Some((String::new(), None));
            }
            continue;
        }
        if let Some((name, path)) = current.as_mut() {
            if let Some(value) = line.strip_prefix("name = ") {
                *name = value.trim_matches('"').to_string();
            } else if let Some(value) = line.strip_prefix("path = ") {
                *path = Some(value.trim_matches('"').to_string());
            }
        }
    }
    if let Some(target) = current.take() {
        targets.push(target);
    }
    targets
}

fn sorted_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = read.map(|entry| entry.expect("entry").path()).collect();
    paths.sort();
    paths
}

/// Targets Cargo infers from a directory: `<dir>/<name>.rs` and
/// `<dir>/<name>/main.rs`, as (name, path relative to the package).
fn inferred_in(package: &Path, relative_dir: &str) -> Vec<(String, String)> {
    let mut inferred = Vec::new();
    for path in sorted_dir(&package.join(relative_dir)) {
        let file_name = path
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        if path.is_file() {
            if let Some(stem) = file_name.strip_suffix(".rs") {
                inferred.push((stem.to_string(), format!("{relative_dir}/{file_name}")));
            }
        } else if path.join("main.rs").is_file() {
            inferred.push((
                file_name.clone(),
                format!("{relative_dir}/{file_name}/main.rs"),
            ));
        }
    }
    inferred
}

/// Explicit targets plus the inferred ones Cargo adds: an inferred target is
/// dropped when an explicit target has its name or its path, and none is
/// added when auto-discovery is switched off (edition 2018 and later).
fn merge_targets(
    explicit: Vec<(String, String)>,
    inferred: Vec<(String, String)>,
    autodiscover: bool,
) -> Vec<(String, String)> {
    let names: BTreeSet<String> = explicit.iter().map(|(name, _)| name.clone()).collect();
    let paths: BTreeSet<String> = explicit.iter().map(|(_, path)| path.clone()).collect();
    let mut targets = explicit;
    if autodiscover {
        targets.extend(
            inferred
                .into_iter()
                .filter(|(name, path)| !names.contains(name) && !paths.contains(path)),
        );
    }
    targets
}

/// The effective binary targets of one member, as (name, path).
fn effective_binaries(member: &str) -> Vec<(String, String)> {
    let package = workspace_root().join(member);
    let manifest = read(&package.join("Cargo.toml"));
    let name = package_value(&manifest, "name").expect("package name");
    let autodiscover = package_value(&manifest, "autobins").as_deref() != Some("false");
    let mut inferred = Vec::new();
    if package.join("src").join("main.rs").is_file() {
        inferred.push((name.clone(), "src/main.rs".to_string()));
    }
    inferred.extend(inferred_in(&package, "src/bin"));
    let explicit = explicit_targets(&manifest, "bin")
        .into_iter()
        .map(|(bin, path)| {
            let path = path.unwrap_or_else(|| {
                if bin == name && package.join("src").join("main.rs").is_file() {
                    "src/main.rs".to_string()
                } else if package.join("src/bin").join(&bin).join("main.rs").is_file() {
                    format!("src/bin/{bin}/main.rs")
                } else {
                    format!("src/bin/{bin}.rs")
                }
            });
            (bin, path)
        })
        .collect();
    merge_targets(explicit, inferred, autodiscover)
}

/// The effective example targets of one member, as (name, path).
fn effective_examples(member: &str) -> Vec<(String, String)> {
    let package = workspace_root().join(member);
    let manifest = read(&package.join("Cargo.toml"));
    let autodiscover = package_value(&manifest, "autoexamples").as_deref() != Some("false");
    let explicit = explicit_targets(&manifest, "example")
        .into_iter()
        .map(|(example, path)| {
            let path = path.unwrap_or_else(|| format!("examples/{example}.rs"));
            (example, path)
        })
        .collect();
    merge_targets(explicit, inferred_in(&package, "examples"), autodiscover)
}

/// The effective bench targets of one member, as (name, path).
fn effective_benches(member: &str) -> Vec<(String, String)> {
    let package = workspace_root().join(member);
    let manifest = read(&package.join("Cargo.toml"));
    let autodiscover = package_value(&manifest, "autobenches").as_deref() != Some("false");
    let explicit = explicit_targets(&manifest, "bench")
        .into_iter()
        .map(|(bench, path)| {
            let path = path.unwrap_or_else(|| {
                if package
                    .join("benches")
                    .join(&bench)
                    .join("main.rs")
                    .is_file()
                {
                    format!("benches/{bench}/main.rs")
                } else {
                    format!("benches/{bench}.rs")
                }
            });
            (bench, path)
        })
        .collect();
    merge_targets(explicit, inferred_in(&package, "benches"), autodiscover)
}

/// Rust source with comments removed. String literals are kept verbatim.
fn strip_rust_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if c == '"' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                out.push(chars[i]);
                if chars[i] == '\\' && i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                i += 1;
                if chars[i - 1] == '"' {
                    break;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The withdrawal message of a J1-pattern entry point, or `None` if the
/// source is anything else.
fn withdrawal_message(source: &str) -> Option<String> {
    const PREFIX: &str = "#![forbid(unsafe_code)] #[cfg(not(test))] \
        fn main() -> std::process::ExitCode { use std::io::Write; \
        const WITHDRAWN_MESSAGE: &str = \"";
    const SUFFIX: &str = "\"; const WITHDRAWN_STATUS: u8 = 69; \
        let _ = writeln!(std::io::stderr(), \"{WITHDRAWN_MESSAGE}\"); \
        std::process::ExitCode::from(WITHDRAWN_STATUS) }";
    let code = normalize_whitespace(&strip_rust_comments(source));
    let message = code
        .strip_prefix(&normalize_whitespace(PREFIX))?
        .strip_suffix(&normalize_whitespace(SUFFIX))?;
    (!message.contains('"') && !message.contains('\\')).then(|| message.to_string())
}

/// Every effective binary target of every workspace member is inventoried
/// with its disposition; nothing is missing or extra. Every withdrawn target
/// is a J1-pattern withdrawal whose message names that binary and is unique,
/// and no other target is one.
#[test]
fn p0_fg_standalone_every_binary_target_is_inventoried() {
    let mut found: Vec<(String, String, String)> = Vec::new();
    for member in workspace_members() {
        for (name, path) in effective_binaries(&member) {
            found.push((member.clone(), name, path));
        }
    }
    found.sort();
    let mut expected: Vec<(String, String, String)> = BINARY_TARGETS
        .iter()
        .map(|(member, name, path, _)| (member.to_string(), name.to_string(), path.to_string()))
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "every binary target must be inventoried with a reviewed disposition"
    );
    assert_eq!(BINARY_TARGETS.len(), 30);

    let mut messages = BTreeMap::new();
    for (member, name, path, disposition) in BINARY_TARGETS {
        let source = read(&workspace_root().join(member).join(path));
        let message = withdrawal_message(&source);
        match disposition {
            Withdrawn => {
                let message = message.unwrap_or_else(|| {
                    panic!("{member}/{path}: `{name}` must be exactly a withdrawal entry point")
                });
                assert!(
                    message.starts_with(&format!("{name}: unavailable during Phase Zero; ")),
                    "{member}/{path}: the message must name `{name}`: {message}"
                );
                assert!(
                    messages.insert(message.clone(), *name).is_none(),
                    "{name}: withdrawal messages must be unique"
                );
            }
            Desktop | Developer | Benchmark => assert!(
                message.is_none(),
                "{member}/{path}: `{name}` is not inventoried as withdrawn"
            ),
        }
    }
    assert_eq!(messages.len(), 17, "seventeen binaries are withdrawn");
    for message in messages.keys() {
        for other in messages.keys().filter(|other| *other != message) {
            assert!(
                !other.contains(message.as_str()),
                "no withdrawal message may contain another ({message})"
            );
        }
    }
}

/// Every Cargo example of the workspace is inventoried: an example is an
/// entry point too (`cargo run --example`), so a new one needs review.
#[test]
fn p0_fg_standalone_every_example_target_is_inventoried() {
    let mut found: Vec<(String, String, String)> = Vec::new();
    for member in workspace_members() {
        for (name, path) in effective_examples(&member) {
            found.push((member.clone(), name, path));
        }
    }
    found.sort();
    let mut expected: Vec<(String, String, String)> = EXAMPLE_TARGETS
        .iter()
        .map(|(member, name, path)| (member.to_string(), name.to_string(), path.to_string()))
        .collect();
    expected.sort();
    assert_eq!(found, expected, "every example target must be inventoried");
}

/// Every Cargo bench target of the workspace is inventoried: a bench is an
/// entry point too (`cargo bench`), so a new one needs review. Each runs in
/// process: it reads no credential or other environment variable, starts no
/// process and opens no network connection (none meets the criterion under
/// which five benchmark binaries were withdrawn).
#[test]
fn p0_fg_standalone_every_bench_target_is_inventoried() {
    let mut found: Vec<(String, String, String)> = Vec::new();
    for member in workspace_members() {
        for (name, path) in effective_benches(&member) {
            found.push((member.clone(), name, path));
        }
    }
    found.sort();
    let mut expected: Vec<(String, String, String)> = BENCH_TARGETS
        .iter()
        .map(|(member, name, path)| (member.to_string(), name.to_string(), path.to_string()))
        .collect();
    expected.sort();
    assert_eq!(found, expected, "every bench target must be inventoried");

    for (member, name, path) in BENCH_TARGETS {
        let code = strip_rust_comments(&read(&workspace_root().join(member).join(path)));
        for forbidden in [
            "_API_KEY",
            "_TOKEN",
            "env::var",
            "std::process",
            "Command::new",
            "curl",
            "reqwest",
            "TcpStream",
            "TcpListener",
            "UdpSocket",
        ] {
            assert!(
                !code.contains(forbidden),
                "{member}/{path}: bench `{name}` must not contain `{forbidden}`"
            );
        }
    }
}

// ── Build scripts of the withdrawn packages ─────────────────────────────────

/// The one build script of a package with a withdrawn entry point, comments
/// aside: `protocols/build.rs` asks Cargo to rebuild when the web interface
/// (`app/dist`) changes, and does nothing else.
const PROTOCOLS_BUILD_SCRIPT: &str = "use std::env; use std::path::PathBuf; \
    fn main() { let manifest_dir = PathBuf::from(match env::var(\"CARGO_MANIFEST_DIR\") { \
    Ok(d) => d, Err(e) => { eprintln!(\"CARGO_MANIFEST_DIR not set: {e}\"); \
    std::process::exit(1); } }); \
    let frontend_dist = manifest_dir.join(\"../app/dist\"); \
    println!(\"cargo:rerun-if-changed={}\", frontend_dist.display()); }";

/// The packages with a withdrawn entry point run no other build script:
/// `protocols/build.rs` is exactly the script above (its only Cargo directive
/// is `rerun-if-changed`), no other such package has a `build.rs`, and none
/// names a build script with a `build` key.
#[test]
fn p0_fg_standalone_withdrawn_packages_run_no_other_build_script() {
    let packages: BTreeSet<&str> = BINARY_TARGETS
        .iter()
        .filter(|(_, _, _, disposition)| *disposition == Withdrawn)
        .map(|(member, _, _, _)| *member)
        .collect();
    assert_eq!(
        packages.len(),
        8,
        "packages with a withdrawn entry point: {packages:?}"
    );
    for member in packages {
        let package = workspace_root().join(member);
        let manifest = read(&package.join("Cargo.toml"));
        assert_eq!(
            package_value(&manifest, "build"),
            None,
            "{member}: no `build` key may name a build script"
        );
        let script = package.join("build.rs");
        if member == "protocols" {
            let code = normalize_whitespace(&strip_rust_comments(&read(&script)));
            assert_eq!(
                code,
                normalize_whitespace(PROTOCOLS_BUILD_SCRIPT),
                "protocols/build.rs must stay exactly the rerun-if-changed script"
            );
            assert_eq!(code.matches("cargo:").count(), 1, "one Cargo directive");
            assert!(code.contains("\"cargo:rerun-if-changed={}\""));
        } else {
            assert!(!script.exists(), "{member}: no build script");
        }
    }
}

// ── No alternate alias ──────────────────────────────────────────────────────

/// Directories that hold no production source, and the test-only guard
/// modules of `phase0_surface/` (which name the needles below as data).
const SKIPPED_DIRS: &[&str] = &["target", "node_modules", "dist", "tests", "fixtures"];
const SKIPPED_PATHS: &[&str] = &["app/src-tauri/src/phase0_surface"];

/// (workspace-relative path, source without comments) of every Rust file
/// under a `src`, `examples` or `benches` directory, except test files.
fn production_sources() -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        for path in sorted_dir(dir) {
            let name = path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            let relative = path
                .strip_prefix(root)
                .expect("inside the workspace")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if !name.starts_with('.')
                    && !SKIPPED_DIRS.contains(&name.as_str())
                    && !SKIPPED_PATHS.contains(&relative.as_str())
                {
                    walk(root, &path, out);
                }
                continue;
            }
            let in_code_dir = relative
                .split('/')
                .any(|component| matches!(component, "src" | "examples" | "benches"));
            if in_code_dir
                && name.ends_with(".rs")
                && name != "tests.rs"
                && !name.ends_with("_tests.rs")
            {
                out.push((relative, strip_rust_comments(&read(&path))));
            }
        }
    }
    let root = workspace_root();
    let root = root.canonicalize().unwrap_or(root);
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(out.len() > 500, "workspace sources not found");
    out
}

/// Entry APIs of the withdrawn surfaces and the only production files that
/// may name them (their definitions, their own libraries, and the withdrawn
/// CLI library that the withdrawn CLI binary used to call).
const ALIAS_NEEDLES: &[(&str, &[&str])] = &[
    // J2, J3: the protocols gateway runtime, and the gateway module and
    // router constructor it serves (public API of the `nexus-protocols`
    // library).
    ("server_runtime", &["protocols/src/lib.rs"]),
    ("run_from_args", &["protocols/src/server_runtime.rs"]),
    (
        "http_gateway",
        &["protocols/src/lib.rs", "protocols/src/server_runtime.rs"],
    ),
    (
        "build_router",
        &[
            "protocols/src/http_gateway.rs",
            "protocols/src/server_runtime.rs",
        ],
    ),
    // J4: the CLI library, and the agent flows its binary ran (D1).
    ("nexus_cli", &[]),
    (
        "run_coding_agent_from_manifest",
        &["agents/coding-agent/src/lib.rs", "cli/src/lib.rs"],
    ),
    (
        "run_social_poster_from_manifest",
        &["agents/social-poster/src/lib.rs", "cli/src/lib.rs"],
    ),
    // J5: the standalone Nexus Code entry points (the desktop uses only the
    // `_for_desktop` forms, which the desktop guards pin).
    ("NxConfig::load()", &[]),
    ("load_without_cli_agents(", &["nexus-code/src/config.rs"]),
    ("diagnose_without_cli_agents(", &["nexus-code/src/setup.rs"]),
    ("new_without_cli_agents(", &["nexus-code/src/app.rs"]),
    (
        "ChatRepl",
        &["nexus-code/src/chat/mod.rs", "nexus-code/src/chat/repl.rs"],
    ),
    ("run_tui(", &["nexus-code/src/tui/mod.rs"]),
];

/// No production source names a listed entry API of a withdrawn surface
/// outside the counted files above, so no other binary, example or library
/// calls one of them to become an alternate alias. The needles are names,
/// not a call graph: an entry API that is not listed here is not covered,
/// and a new one needs a row.
#[test]
fn p0_fg_standalone_no_alias_reaches_a_withdrawn_entry() {
    let sources = production_sources();
    for (needle, allowed) in ALIAS_NEEDLES {
        let found: BTreeSet<&str> = sources
            .iter()
            .filter(|(_, code)| code.contains(needle))
            .map(|(path, _)| path.as_str())
            .collect();
        let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
        assert_eq!(found, allowed, "production files that name `{needle}`");
    }
}

/// Only the desktop and the `nexus-code` package itself name the
/// `nexus_code` library, so no other member can rebuild the standalone
/// terminal from it.
#[test]
fn p0_fg_standalone_only_the_desktop_embeds_nexus_code() {
    for (path, code) in production_sources() {
        if code.contains("nexus_code") {
            assert!(
                path.starts_with("app/src-tauri/src/") || path.starts_with("nexus-code/src/"),
                "{path}: only the desktop and nexus-code name the nexus_code library"
            );
        }
    }
}

/// The computer-use agent loop, OS input controllers and screen capture
/// (the withdrawn `nx-*` harness, D2) are reached outside their own crate
/// only by the counted, developer-only library file below (its binaries do
/// not reach it; decision D3 is pending with the Architect).
#[test]
fn p0_fg_standalone_computer_use_is_not_re_exposed() {
    let sources = production_sources();
    // `run_agent_loop` is not re-exported: reaching it means naming
    // `loop_controller`.
    for (needle, allowed) in [
        ("loop_controller", &[][..]),
        (
            "MouseController",
            &["crates/nexus-ui-repair/src/specialists/eyes_and_hands.rs"][..],
        ),
        ("KeyboardController", &[][..]),
        (
            "take_screenshot",
            &["crates/nexus-ui-repair/src/specialists/eyes_and_hands.rs"][..],
        ),
    ] {
        let found: BTreeSet<&str> = sources
            .iter()
            .filter(|(path, code)| {
                !path.starts_with("crates/nexus-computer-use/src/") && code.contains(needle)
            })
            .map(|(path, _)| path.as_str())
            .collect();
        let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
        assert_eq!(
            found, allowed,
            "files outside nexus-computer-use that name `{needle}`"
        );
    }
}

// ── Recipes and workflows ───────────────────────────────────────────────────

/// Every deployment, installation and packaging recipe in the repository.
/// All are withdrawn: J1's under `deploy/`, and J2 to J5's (pinned exactly by
/// the `phase0_withdrawal.rs` tests of `nexus-protocols`, `nexus-cli` and
/// `nexus-code`).
const RECIPE_FILES: &[&str] = &[
    "Dockerfile",
    "deploy/Dockerfile",
    "deploy/docker-compose.cpu.yml",
    "deploy/docker-compose.yml",
    "deploy/helm/nexus-os/Chart.yaml",
    "docker-compose.yml",
    "helm/nexus-os/Chart.yaml",
    "install.sh",
    "nexus-code/Dockerfile",
    "nexus-code/install.sh",
    "packaging/linux/nexus-os.service",
    "packaging/macos/com.nexusos.agent.plist",
    "packaging/macos/homebrew/nexus-os.rb",
    "packaging/windows/nexus-os.wxs",
];

fn is_recipe(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("dockerfile")
        || lower == "containerfile"
        || (lower.contains("compose") && (lower.ends_with(".yml") || lower.ends_with(".yaml")))
        || lower == "chart.yaml"
        || lower == "pkgbuild"
        || lower == "snapcraft.yaml"
        || lower == "procfile"
        || (lower.starts_with("install") && (lower.ends_with(".sh") || lower.ends_with(".ps1")))
        || [
            ".service", ".socket", ".timer", ".plist", ".wxs", ".nsi", ".iss", ".rb", ".nuspec",
            ".spec",
        ]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// No deployment, installation or packaging recipe exists beyond the
/// inventoried, withdrawn ones, so no new container, chart, service unit,
/// installer or package recipe can ship a standalone binary unreviewed.
#[test]
fn p0_fg_standalone_every_recipe_is_inventoried() {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for path in sorted_dir(dir) {
            let name = path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            let relative = path
                .strip_prefix(root)
                .expect("inside the workspace")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                // Build output, dependencies, and the Builder toolchain that
                // the packaging step assembles (ignored by Git).
                let generated = ["target", "node_modules", "dist"].contains(&name.as_str())
                    || relative == "app/src-tauri/builder-toolchain"
                    || relative.starts_with("app/src-tauri/builder-toolchain.assembly-");
                if !name.starts_with('.') && !generated {
                    walk(root, &path, out);
                }
            } else if is_recipe(&name) {
                out.push(relative);
            }
        }
    }
    let root = workspace_root();
    let root = root.canonicalize().unwrap_or(root);
    let mut found = Vec::new();
    walk(&root, &root, &mut found);
    found.sort();
    let mut expected: Vec<String> = RECIPE_FILES.iter().map(|path| path.to_string()).collect();
    expected.sort();
    assert_eq!(found, expected, "every recipe must be inventoried");
}

/// Names of the withdrawn binaries, as a workflow would name them.
const WITHDRAWN_BINARIES: &[&str] = &[
    "nexus-server",
    "nexus-protocols-server",
    "nexus-os",
    "nexus-cli",
    "nx",
    "coding-agent",
    "social-poster-agent",
    "nx-screen",
    "nx-input",
    "nx-agent",
    "nx-govern",
    "nx-learn",
    "nim-cloud-bench",
    "cloud-models-bench",
    "inference-consistency-bench",
    "local-vs-cloud-battle",
    "real-agent-validation",
];

/// No workflow builds, installs, uploads or publishes a withdrawn binary, a
/// container image or a chart, and the release publishes only the desktop
/// installers. (Workflows still compile and test the withdrawn packages.)
#[test]
fn p0_fg_standalone_no_workflow_ships_a_standalone_binary() {
    let root = workspace_root();
    let mut workflows: Vec<PathBuf> = sorted_dir(&root.join(".github").join("workflows"))
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yml" || extension == "yaml")
        })
        .collect();
    assert!(workflows.len() >= 5, "workflows not found");
    workflows.push(root.join(".gitlab-ci.yml"));
    for workflow in &workflows {
        let text = read(workflow);
        let name = workflow.display();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with('#'))
        {
            for forbidden in [
                "docker build",
                "docker push",
                "docker/build-push-action",
                "buildx",
                "helm package",
                "helm push",
                "helm install",
                "cargo install --path",
            ] {
                assert!(!line.contains(forbidden), "{name}: `{forbidden}` in {line}");
            }
            for binary in WITHDRAWN_BINARIES {
                for needle in [
                    format!("--bin {binary}"),
                    format!("--bin={binary}"),
                    format!("target/release/{binary}"),
                    format!("target/debug/{binary}"),
                ] {
                    let hit = line.match_indices(&needle).any(|(at, _)| {
                        !line[at + needle.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    });
                    assert!(
                        !hit,
                        "{name}: withdrawn binary `{binary}` shipped by {line}"
                    );
                }
            }
            assert!(
                !(line.contains("cargo build") && line.contains("-p nexus-cli")),
                "{name}: nexus-cli is built for shipping by {line}"
            );
        }
    }

    // The release publishes the desktop installers and nothing else.
    let release = read(&root.join(".github").join("workflows").join("release.yml"));
    let lines: Vec<&str> = release.lines().collect();
    let files = lines
        .iter()
        .position(|line| line.trim() == "files: |")
        .expect("the release's published file list");
    let published: Vec<&str> = lines[files + 1..]
        .iter()
        .map(|line| line.trim())
        .take_while(|line| !line.is_empty() && !line.contains(':'))
        .collect();
    assert_eq!(
        published,
        [
            "NexusOS-Windows/*.msi",
            "NexusOS-Windows/*.exe",
            "NexusOS-Linux/*.deb",
            "NexusOS-macOS/*.dmg",
        ],
        "the release publishes only the desktop installers"
    );
}
