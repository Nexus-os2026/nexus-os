//! P0-FINAL-GATE-CLOSURE guards for items J2 to J5 (standalone surfaces).
//!
//! The protocols server (`nexus-protocols-server`, J2) and its `nexus-os`
//! alias (J3), `nexus-cli` (J4) and the standalone `nx` terminal (J5) are
//! withdrawn, together with the alternate `coding-agent` and
//! `social-poster-agent` entry points, the `nx-*` computer-use harness
//! (coordinator decisions D1 and D2) and the six benchmarks that sent a
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
        Withdrawn,
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

/// A manifest, parsed as TOML (so spacing, quoting and table layout cannot
/// hide a key from these guards).
fn parse_manifest(path: &Path) -> toml::Table {
    read(path)
        .parse::<toml::Table>()
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The manifest of workspace member `member`.
fn member_manifest(member: &str) -> toml::Table {
    parse_manifest(&workspace_root().join(member).join("Cargo.toml"))
}

/// The `members` of the workspace manifest.
fn workspace_members() -> Vec<String> {
    let manifest = parse_manifest(&workspace_root().join("Cargo.toml"));
    let members: Vec<String> = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .expect("workspace members")
        .iter()
        .map(|member| member.as_str().expect("a member path").to_string())
        .collect();
    assert!(members.len() > 50, "workspace members not found");
    members
}

/// A manifest's `[package]` value for `key`, if it is set there.
fn package_value<'a>(manifest: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    manifest.get("package")?.as_table()?.get(key)
}

/// Whether the manifest switches a target auto-discovery key off.
fn autodiscovery_off(manifest: &toml::Table, key: &str) -> bool {
    package_value(manifest, key).and_then(toml::Value::as_bool) == Some(false)
}

/// Explicit `[[section]]` targets of a manifest as (name, optional path).
fn explicit_targets(manifest: &toml::Table, section: &str) -> Vec<(String, Option<String>)> {
    let Some(targets) = manifest.get(section) else {
        return Vec::new();
    };
    let targets = targets
        .as_array()
        .unwrap_or_else(|| panic!("[[{section}]] must be an array of tables"));
    targets
        .iter()
        .map(|target| {
            let field = |key: &str| target.get(key).and_then(toml::Value::as_str);
            (
                field("name").unwrap_or_default().to_string(),
                field("path").map(str::to_string),
            )
        })
        .collect()
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
    let manifest = member_manifest(member);
    let name = package_value(&manifest, "name")
        .and_then(toml::Value::as_str)
        .expect("package name")
        .to_string();
    let autodiscover = !autodiscovery_off(&manifest, "autobins");
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
    let manifest = member_manifest(member);
    let autodiscover = !autodiscovery_off(&manifest, "autoexamples");
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
    let manifest = member_manifest(member);
    let autodiscover = !autodiscovery_off(&manifest, "autobenches");
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
    assert_eq!(messages.len(), 18, "eighteen binaries are withdrawn");
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
/// entry point too (`cargo bench`), so a new one needs review. Each bench
/// file, comments aside, names no credential or environment read, process
/// spawn or network API from the list below (the criterion under which six
/// benchmark binaries were withdrawn).
///
/// Not a claim: this is a text check on the bench file only. It does not
/// follow the code a bench calls (its own package or its dependencies), so
/// it does not show that running a bench reads no environment variable,
/// starts no process or opens no network connection; a spelling not in the
/// list (a re-export, an alias, a macro) is not seen either. The inventory
/// makes any new or changed bench target a reviewed change.
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
        let manifest = member_manifest(member);
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
        || lower.ends_with(".dockerfile")
        || lower == "containerfile"
        || lower == "vagrantfile"
        || lower == "devcontainer.json"
        || lower == ".devcontainer.json"
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

/// A directory the root `.gitignore` anchors as ignored: a directory entry
/// (ending in `/`) with a `/` at its start or in its middle, as a
/// workspace-relative path, or as a path prefix when its last component ends
/// in a `*` glob. Negated, file, unanchored and other glob entries are not
/// used.
#[derive(Debug)]
struct IgnoredDirectory {
    path: String,
    prefix: bool,
}

impl IgnoredDirectory {
    fn matches(&self, relative: &str) -> bool {
        if self.prefix {
            relative.starts_with(&self.path) && !relative[self.path.len()..].contains('/')
        } else {
            relative == self.path
        }
    }
}

/// The anchored ignored directories the walk may skip, pinned: their number
/// and the SHA-256 of their sorted entries (`path`, or `path*` for a
/// prefix), joined by line feeds. A new or changed anchored entry in
/// `.gitignore` could hide a recipe or a pipeline file from the walk, so it
/// fails here until it is reviewed and the pin is updated.
const IGNORED_DIRECTORIES_PIN: (usize, &str) = (
    10,
    "6fa2a363ee77a556134b6c454da1eb3e830e243905cebc6fa315693d0e356ce5",
);

/// The anchored ignored directories of the root `.gitignore`: ignored build
/// output and local state, such as the Builder toolchain that the packaging
/// step assembles, cloned upstream sources and agent worktrees (other
/// checkouts of this repository, each guarded by its own copy of these
/// tests). Read at run time, so the walk never names them itself, and pinned
/// ([`IGNORED_DIRECTORIES_PIN`]).
fn ignored_anchored_directories() -> Vec<IgnoredDirectory> {
    let mut directories = Vec::new();
    for line in read(&workspace_root().join(".gitignore")).lines() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') || entry.starts_with('!') {
            continue;
        }
        let Some(body) = entry.strip_suffix('/') else {
            continue;
        };
        if !body.contains('/') {
            continue;
        }
        let body = body.trim_start_matches('/');
        let (path, prefix) = match body.strip_suffix('*') {
            Some(stem) => (stem, true),
            None => (body, false),
        };
        if path.is_empty() || path.contains(['*', '?', '[', '\\']) {
            continue;
        }
        directories.push(IgnoredDirectory {
            path: path.to_string(),
            prefix,
        });
    }
    let mut entries: Vec<String> = directories
        .iter()
        .map(|directory| {
            format!(
                "{}{}",
                directory.path,
                if directory.prefix { "*" } else { "" }
            )
        })
        .collect();
    entries.sort();
    let digest = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
        entries.join("\n").as_bytes(),
    ));
    assert_eq!(
        (entries.len(), digest.as_str()),
        IGNORED_DIRECTORIES_PIN,
        "the anchored ignored directories of .gitignore changed: check that none can hide a \
         recipe or a pipeline file, then update IGNORED_DIRECTORIES_PIN"
    );
    directories
}

/// The files git tracks in this checkout. `None` when there is no checkout
/// (`root` has no `.git`, as in a source archive) or git is not installed.
/// In a checkout with git installed, a git that fails (for example a
/// `safe.directory` refusal or a broken index) fails the guard: the walk
/// alone is not trusted there. The walk skips build output, dependencies and
/// ignored local state, so a recipe or pipeline file tracked there anyway
/// (for example force-added) is found through this list instead.
fn tracked_files(root: &Path) -> Option<Vec<String>> {
    if !root.join(".git").exists() {
        return None;
    }
    let output = match std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(std::process::Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => panic!("git could not be run in this checkout: {error}"),
    };
    assert!(
        output.status.success(),
        "`git ls-files` failed in this checkout ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Some(
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect(),
    )
}

/// Every file of the repository as a workspace-relative path, in sorted
/// order. Dot directories (`.github`, `.gitlab`, `.cargo`, `.claude` and any
/// new one) are walked like the others. The walk skips version-control
/// internals and build output and dependencies by name (`.git`, `target`,
/// `node_modules`), and the pinned anchored ignored directories of the root
/// `.gitignore` (see [`ignored_anchored_directories`]); every file git tracks
/// is added, wherever it is ([`tracked_files`]).
fn repository_files() -> Vec<String> {
    fn walk(root: &Path, dir: &Path, ignored: &[IgnoredDirectory], out: &mut Vec<String>) {
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
                let skipped = [".git", "target", "node_modules"].contains(&name.as_str())
                    || ignored.iter().any(|directory| directory.matches(&relative));
                if !skipped {
                    walk(root, &path, ignored, out);
                }
            } else {
                out.push(relative);
            }
        }
    }
    let root = workspace_root();
    let root = root.canonicalize().unwrap_or(root);
    let ignored = ignored_anchored_directories();
    let mut out = Vec::new();
    walk(&root, &root, &ignored, &mut out);
    assert!(out.len() > 1000, "repository files not found");
    let mut files: BTreeSet<String> = out.into_iter().collect();
    files.extend(tracked_files(&root).unwrap_or_default());
    files.into_iter().collect()
}

fn file_name(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

/// No deployment, installation or packaging recipe exists beyond the
/// inventoried, withdrawn ones, so no new container, chart, service unit,
/// installer or package recipe can ship a standalone binary unreviewed. Dot
/// directories are searched too.
#[test]
fn p0_fg_standalone_every_recipe_is_inventoried() {
    let found: Vec<String> = repository_files()
        .into_iter()
        .filter(|relative| is_recipe(file_name(relative)))
        .collect();
    let mut expected: Vec<String> = RECIPE_FILES.iter().map(|path| path.to_string()).collect();
    expected.sort();
    assert_eq!(found, expected, "every recipe must be inventoried");
}

/// Whether a file configures a continuous-integration pipeline (which could
/// build, ship or publish a binary): any `workflows/*.yml` or `.yaml` under a
/// dot directory (GitHub, Gitea and Forgejo Actions, and any other service
/// that reads them there), GitHub actions (`action.yml`), GitLab CI
/// (including included files by the conventional names), sourcehut
/// (`.build.yml`, `.builds/`), TeamCity (`.teamcity/`), and the pipeline
/// files of other CI services.
fn is_ci_config(relative: &str) -> bool {
    let name = file_name(relative).to_ascii_lowercase();
    let yaml = name.ends_with(".yml") || name.ends_with(".yaml");
    let components: Vec<&str> = relative.split('/').collect();
    let workflows_under_dot_directory = yaml
        && components
            .windows(3)
            .any(|window| window[0].starts_with('.') && window[1] == "workflows");
    workflows_under_dot_directory
        || name == "action.yml"
        || name == "action.yaml"
        || name.ends_with("gitlab-ci.yml")
        || name.ends_with("gitlab-ci.yaml")
        || (relative.starts_with(".gitlab/") && yaml)
        || [
            ".circleci/",
            ".buildkite/",
            ".woodpecker/",
            ".tekton/",
            ".drone/",
            ".semaphore/",
            ".cirrus/",
            ".builds/",
            ".teamcity/",
        ]
        .iter()
        .any(|dir| relative.starts_with(dir))
        || [
            ".travis.yml",
            "jenkinsfile",
            "azure-pipelines.yml",
            "azure-pipelines.yaml",
            "bitbucket-pipelines.yml",
            ".drone.yml",
            ".woodpecker.yml",
            "appveyor.yml",
            ".appveyor.yml",
            "cloudbuild.yml",
            "cloudbuild.yaml",
            "buildspec.yml",
            "codemagic.yaml",
            ".cirrus.yml",
            "wercker.yml",
            ".build.yml",
        ]
        .contains(&name.as_str())
}

/// The `include` recognizer used by the workflow guard: every spelling a
/// YAML parser reads as an `include` key is refused, and ordinary lines that
/// only mention the word are not.
#[test]
fn p0_fg_standalone_gitlab_includes_are_recognized() {
    for yaml in [
        "include: ci/build.yml",
        "include : ci/build.yml",
        "  include:\n    - local: ci/build.yml",
        "\"include\": ci/build.yml",
        "'include': ci/build.yml",
        "'include' : ci/build.yml",
        "Include: ci/build.yml",
        "- include: ci/build.yml",
        "? include\n: ci/build.yml",
        "&base include: ci/build.yml",
        "!!str include: ci/build.yml",
        "{include: ci/build.yml}",
        "job: {script: build, include: ci/build.yml}",
        "jobs: [ {\"include\": ci/build.yml} ]",
        "\"inc\\x6cude\": ci/build.yml",
        "trigger:\n  include: ci/child.yml",
    ] {
        assert!(declares_include(yaml), "{yaml:?}");
    }
    for yaml in [
        "# include: ci/build.yml",
        "script:\n  - echo include the tests",
        "name: include",
        "includes_nothing: true",
        "stages: [build, test]",
    ] {
        assert!(!declares_include(yaml), "{yaml:?}");
    }
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
    "real-battery-validation",
];

/// The README's withdrawal section names exactly the withdrawn binaries (as
/// code spans), and that list is the inventory's: users are told which
/// binaries only deny, and no kept binary is named as withdrawn.
#[test]
fn p0_fg_standalone_readme_names_every_withdrawn_binary() {
    let inventoried: BTreeSet<&str> = BINARY_TARGETS
        .iter()
        .filter(|(_, _, _, disposition)| *disposition == Withdrawn)
        .map(|(_, name, _, _)| *name)
        .collect();
    let listed: BTreeSet<&str> = WITHDRAWN_BINARIES.iter().copied().collect();
    assert_eq!(
        listed, inventoried,
        "the withdrawn names are the inventory's"
    );

    let readme = read(&workspace_root().join("README.md")).replace("\r\n", "\n");
    let start = readme
        .find("### Server Deployment (withdrawn)")
        .expect("the README's withdrawal section");
    let section = &readme[start..];
    let section = &section[..section.find("\n## ").unwrap_or(section.len())];
    let named: BTreeSet<&str> = section.split('`').skip(1).step_by(2).collect();
    assert_eq!(
        named, inventoried,
        "the README's withdrawal section names exactly the withdrawn binaries"
    );
}

/// Whether YAML text declares an `include` key (GitLab CI's way of adding
/// jobs from other files, also inside `trigger:`), in any form a YAML parser
/// accepts on one line: a block mapping key, optionally after a list-item
/// dash, a complex-key `?`, an anchor or a tag, bare or quoted, with any
/// spacing before the colon; a complex key `? include` whose colon follows
/// on the next line; or any mention of `include` after a `{` or `[` on the
/// line (a flow mapping). A double-quoted key holding an escape counts too,
/// since an escape can spell `include`. Letter case is ignored. Lines that
/// hold only a comment are skipped; text inside block scalars is read like
/// any other line, which errs on the side of refusing.
fn declares_include(yaml: &str) -> bool {
    yaml.lines().any(|raw| {
        let mut line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        if let Some(flow) = line.find(['{', '[']) {
            if line[flow..].to_ascii_lowercase().contains("include") {
                return true;
            }
        }
        let mut complex_key = false;
        loop {
            if let Some(rest) = line
                .strip_prefix('-')
                .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
            {
                line = rest.trim_start();
            } else if let Some(rest) = line
                .strip_prefix('?')
                .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
            {
                complex_key = true;
                line = rest.trim_start();
            } else if line.starts_with('&') || line.starts_with('!') {
                line = line
                    .find(char::is_whitespace)
                    .map_or("", |end| line[end..].trim_start());
            } else {
                break;
            }
        }
        let (key, rest) = match line.chars().next() {
            Some(quote @ ('"' | '\'')) => match line[1..].find(quote) {
                Some(end) => (&line[1..=end], &line[end + 2..]),
                None => (&line[1..], ""),
            },
            _ => match line.find(':') {
                Some(colon) => (line[..colon].trim_end(), &line[colon..]),
                None => (line.trim_end(), ""),
            },
        };
        let escaped = line.starts_with('"') && key.contains('\\');
        let is_key = rest.trim_start().starts_with(':') || complex_key;
        is_key && (escaped || key.eq_ignore_ascii_case("include"))
    })
}

/// The `uses:` references CI configurations may make: the actions the
/// existing workflows use, by exact reference. Any other action (or a
/// reference in any other spelling) fails the workflow guard until reviewed.
const ALLOWED_ACTIONS: &[&str] = &[
    "actions/checkout@v4",
    "actions/download-artifact@v4",
    "actions/setup-node@v4",
    "actions/setup-python@v5",
    "actions/upload-artifact@v4",
    "dtolnay/rust-toolchain@1.94.0",
    "dtolnay/rust-toolchain@stable",
    "peaceiris/actions-gh-pages@v4",
    "softprops/action-gh-release@v2",
    "Swatinem/rust-cache@v2",
];

/// The build outputs under a `release` or `debug` profile directory that CI
/// configurations may name: the desktop bundle and its bundled toolchain.
const DESKTOP_BUILD_OUTPUTS: &[&str] = &["bundle", "toolchain"];

/// Cargo subcommands that compile or check code without producing a binary
/// to ship. A withdrawn package may be selected only by one of these.
const NON_SHIPPING_CARGO_SUBCOMMANDS: &[&str] = &[
    "audit", "bench", "c", "check", "clippy", "d", "deny", "doc", "fetch", "fmt", "llvm-cov",
    "metadata", "nextest", "t", "test", "tree",
];

/// Every `uses:` reference on a line (any quoting, block or flow style),
/// as written; an empty reference stands for one on a later line.
fn action_references(line: &str) -> Vec<&str> {
    let lower = line.to_ascii_lowercase();
    lower
        .match_indices("uses")
        .filter(|(at, _)| {
            !line[..*at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .filter_map(|(at, _)| {
            let rest = line[at + "uses".len()..].trim_start_matches(['"', '\'']);
            let rest = rest.trim_start().strip_prefix(':')?;
            let rest = rest.trim_start().trim_start_matches(['"', '\'']);
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '}' | ']'))
                .unwrap_or(rest.len());
            Some(&rest[..end])
        })
        .collect()
}

/// The path-like words of a line (backslashes read as slashes, case folded)
/// that name build output other than the desktop bundle: anything under a
/// `release` or `debug` profile directory but the allowlisted outputs, the
/// profile directory itself, the target directory itself, or a glob in the
/// target directory. Each word is checked as written and with its `.` and
/// `..` segments resolved, and any `..` segment in a word that names
/// `target/` fails it (`target/release/bundle/../nexus-server`).
fn undesktop_build_outputs(line: &str) -> Vec<String> {
    let normalized = line.to_ascii_lowercase().replace('\\', "/");
    normalized
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '"' | '\''
                        | '`'
                        | ','
                        | ';'
                        | '('
                        | ')'
                        | '='
                        | '|'
                        | '<'
                        | '>'
                        | '&'
                        | '{'
                        | '}'
                )
        })
        .filter(|word| {
            let offends = |components: &[&str]| {
                components.iter().enumerate().any(|(index, component)| {
                    let next = components.get(index + 1).copied();
                    let profile = index > 0 && matches!(*component, "release" | "debug");
                    let target = *component == "target" && word.contains('/');
                    (profile && !next.is_some_and(|next| DESKTOP_BUILD_OUTPUTS.contains(&next)))
                        || (target
                            && next.is_none_or(|next| {
                                next.is_empty() || next.contains(['*', '?', '['])
                            }))
                })
            };
            let written: Vec<&str> = word.split('/').collect();
            let mut resolved: Vec<&str> = Vec::new();
            for component in &written {
                match *component {
                    "." => {}
                    ".." => {
                        resolved.pop();
                    }
                    other => resolved.push(other),
                }
            }
            (word.contains("target/") && written.contains(&".."))
                || offends(&written)
                || offends(&resolved)
        })
        .map(str::to_string)
        .collect()
}

/// Cargo options that take a value as the next word, so that word is not
/// the subcommand.
const CARGO_VALUE_OPTIONS: &[&str] = &[
    "--color",
    "--config",
    "--manifest-path",
    "--target-dir",
    "-C",
    "-Z",
];

/// The cargo invocations in a CI configuration that ship a withdrawn entry
/// point, as (what, invocation). The configuration is read as one word
/// stream (so continued and folded lines join): shell operators (`;`, `&`,
/// `|`, in any run) are separate words even when written against another
/// word, and continuation marks (`\`, a PowerShell backtick) are dropped.
/// An invocation runs from a `cargo` (or `cross`) word to the next shell
/// separator or the next invocation; its subcommand is its first word that
/// is neither an option nor the value of one in [`CARGO_VALUE_OPTIONS`].
/// Found are:
/// - a shipping subcommand (not in [`NON_SHIPPING_CARGO_SUBCOMMANDS`]) that
///   selects a withdrawn package by `-p NAME`, `-pNAME`, `-p=NAME`,
///   `--package NAME`, `--package=NAME` (a package id spec names its
///   package; a glob counts as selecting every package) or by
///   `--manifest-path` to a withdrawn package's manifest;
/// - any subcommand naming a withdrawn binary by `--bin NAME` or
///   `--bin=NAME`, quoted or not;
/// - `cargo install` from a path (`--path DIR`, `--path=DIR`).
///
/// Not a claim: this is a word-level recognizer, not a shell or YAML
/// parser. A build selected by working directory, by `cd`, or through a
/// variable, alias or script is not seen here (the upload and publish
/// checks bound those).
fn shipped_withdrawn_packages(
    text: &str,
    packages: &BTreeSet<String>,
    directories: &BTreeSet<String>,
    binaries: &[&str],
) -> Vec<(String, String)> {
    let spaced: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.chars().chain(['\n']))
        .flat_map(|c| {
            if matches!(c, ';' | '&' | '|') {
                vec![' ', c, ' ']
            } else {
                vec![c]
            }
        })
        .collect();
    let words: Vec<&str> = spaced
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | '`' | '(' | ')' | '{' | '}' | '[' | ']' | ','
                )
            })
        })
        .filter(|word| !word.is_empty() && *word != "\\")
        .collect();
    let is_cargo = |word: &str| {
        let word = word.to_ascii_lowercase();
        let word = word.trim_start_matches('$');
        let word = word.rsplit('/').next().unwrap_or(word);
        let word = word.strip_suffix(".exe").unwrap_or(word);
        word == "cargo" || word == "cross"
    };
    let is_separator = |word: &str| {
        word.chars().all(|c| matches!(c, ';' | '&' | '|')) || matches!(word, "then" | "do")
    };
    let unquoted = |value: &str| value.trim_matches(['"', '\'']).to_string();
    let selects_withdrawn = |spec: &str| {
        let spec = unquoted(spec);
        let spec = spec.rsplit('#').next().unwrap_or(&spec);
        let name = spec.split(['@', ':']).next().unwrap_or(spec);
        spec.contains(['*', '?', '[']) || packages.contains(name)
    };
    let manifest_is_withdrawn = |path: &str| {
        let path = unquoted(path).replace('\\', "/");
        directories.iter().any(|directory| {
            let manifest = format!("{directory}/Cargo.toml");
            path == manifest
                || path == format!("./{manifest}")
                || path.ends_with(&format!("/{manifest}"))
        })
    };
    let names_withdrawn_binary = |name: &str| binaries.contains(&unquoted(name).as_str());
    let mut found = Vec::new();
    let mut index = 0;
    while index < words.len() {
        if !is_cargo(words[index]) {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < words.len() && !is_cargo(words[end]) && !is_separator(words[end]) {
            end += 1;
        }
        let invocation = &words[index + 1..end];
        let mut subcommand = "";
        let mut at = 0;
        while let Some(word) = invocation.get(at) {
            if CARGO_VALUE_OPTIONS.contains(word) {
                at += 2;
            } else if word.starts_with('-') || word.starts_with('+') {
                at += 1;
            } else {
                subcommand = word;
                break;
            }
        }
        let shipping = !NON_SHIPPING_CARGO_SUBCOMMANDS.contains(&subcommand);
        let value_after = |at: usize| invocation.get(at + 1).copied().unwrap_or_default();
        for (at, word) in invocation.iter().enumerate() {
            let selected = if *word == "-p" || *word == "--package" {
                shipping && selects_withdrawn(value_after(at))
            } else if let Some(spec) = word.strip_prefix("--package=") {
                shipping && selects_withdrawn(spec)
            } else if let Some(spec) = word.strip_prefix("-p").filter(|_| !word.starts_with("--")) {
                shipping && selects_withdrawn(spec.strip_prefix('=').unwrap_or(spec))
            } else if *word == "--manifest-path" {
                shipping && manifest_is_withdrawn(value_after(at))
            } else if let Some(path) = word.strip_prefix("--manifest-path=") {
                shipping && manifest_is_withdrawn(path)
            } else if *word == "--bin" {
                names_withdrawn_binary(value_after(at))
            } else if let Some(name) = word.strip_prefix("--bin=") {
                names_withdrawn_binary(name)
            } else {
                subcommand == "install" && (*word == "--path" || word.starts_with("--path="))
            };
            if selected {
                found.push((subcommand.to_string(), invocation.join(" ")));
            }
        }
        index = end;
    }
    found
}

/// The shipping recognizers used by the workflow guard catch every
/// spelling of a package selector, a profile-directory or glob upload, and
/// an action reference, and pass what the workflows legitimately use.
#[test]
fn p0_fg_standalone_shipping_recognizers_catch_probes() {
    let packages: BTreeSet<String> = ["nexus-cli".to_string()].into();
    let directories: BTreeSet<String> = ["cli".to_string()].into();
    let binaries = ["fg-withdrawn-bin"];
    let ships = |text: &str| {
        !shipped_withdrawn_packages(text, &packages, &directories, &binaries).is_empty()
    };
    for text in [
        "cargo build --release -p nexus-cli",
        "cargo build --release -pnexus-cli",
        "cargo build --release -p=nexus-cli",
        "cargo build --package nexus-cli",
        "cargo build --package=nexus-cli",
        "run: \"cargo build --package 'nexus-cli'\"",
        "cargo +stable build --locked -p nexus-cli@10.6.0",
        "cargo install -p nexus-c*",
        "cargo build --release \\\n  -p nexus-cli",
        "run: >\n  cargo rustc\n  --package nexus-cli",
        "cargo run --manifest-path cli/Cargo.toml",
        "$CARGO build --manifest-path=./cli/Cargo.toml",
        "cross build -p nexus-cli",
        "cargo --config x build -p nexus-cli",
        "cargo build --release -p nexus-cli;echo done",
        "cargo build -p nexus-cli&&echo done",
        "cargo --color always build -p nexus-cli",
        "cargo -Z unstable-options build -p nexus-cli",
        "cargo `\n  build -p nexus-cli",
        "cargo build --release --bin 'fg-withdrawn-bin'",
        "cargo build --release --bin=\"fg-withdrawn-bin\"",
        "cargo test --bin fg-withdrawn-bin",
        "cargo install --locked --path cli",
        "cargo install --path=cli",
    ] {
        assert!(ships(text), "not recognized as shipping: {text}");
    }
    for text in [
        "cargo test -p nexus-kernel -p nexus-sdk -p nexus-cli",
        "cargo clippy -p nexus-cli -- -D warnings",
        "cargo build --release && mkdir -p nexus-cli",
        "cargo build -p nexus-desktop-backend",
        "# cargo build -p nexus-cli",
        "cargo --color always test -p nexus-cli",
        "cargo --config net.offline=true check -p nexus-cli",
        "cargo \\\n  test -p nexus-cli",
        "cargo `\n  clippy -p nexus-cli",
        "cargo test -p nexus-cli | tee log",
        "cargo build --release --bin fg-withdrawn-bin-2",
        "cargo install ripgrep --version 14.1.1 --locked --root \"$root\"",
    ] {
        assert!(!ships(text), "wrongly recognized as shipping: {text}");
    }
    for line in [
        "path: target/release/nexus-server",
        "path: target/release",
        "path: target/release/",
        "path: target/release/*",
        "path: target/debug/**",
        "path: 'target/*/nexus-server'",
        "path: target/",
        "path: app/src-tauri/target",
        "Copy-Item target\\release\\nexus-server.exe out",
        "path: Target/Release/nexus-server.exe",
        "path: target/x86_64-unknown-linux-gnu/release/nexus-server",
        "path: ${{ env.CARGO_TARGET_DIR }}/release/nexus-server",
        "path: target/release/{bundle,nexus-server}",
        "path: target/release/bundle/../nexus-server",
        "path: target/release/bundle/../../release/nexus-server",
        "path: target/release/./nexus-server",
        "path: target/release/bundle/..",
        "Copy-Item target\\release\\bundle\\..\\nexus-server.exe out",
    ] {
        assert!(
            !undesktop_build_outputs(line).is_empty(),
            "not recognized as a build-output upload: {line}"
        );
    }
    for line in [
        "bundled=target/release/toolchain",
        "Get-ChildItem -Recurse target\\release\\bundle -ErrorAction SilentlyContinue",
        "ls -R target/release/bundle || true",
        "find target/release/bundle/deb -mindepth 2",
        "ls ./target/release/bundle",
        "cargo build --release --target x86_64-pc-windows-msvc",
    ] {
        assert!(
            undesktop_build_outputs(line).is_empty(),
            "wrongly recognized as a build-output upload: {line}"
        );
    }
    assert_eq!(
        action_references("- uses: actions/checkout@v4"),
        ["actions/checkout@v4"]
    );
    assert_eq!(
        action_references("  \"uses\": 'evil/x@v1' # c"),
        ["evil/x@v1"]
    );
    assert_eq!(
        action_references("- {name: a, uses: evil/y@main}"),
        ["evil/y@main"]
    );
    assert_eq!(action_references("uses:"), [""]);
    assert!(action_references("run: echo causes: x").is_empty());
}

/// No workflow builds, installs, uploads or publishes a withdrawn binary, a
/// container image or a chart, and the release publishes only the desktop
/// installers. (Workflows still compile and test the withdrawn packages.)
/// A withdrawn package is selected only by a non-shipping cargo subcommand,
/// no path names build output under a profile directory other than the
/// desktop bundle (nor the profile or target directory itself, nor a glob
/// there), and every action referenced is in [`ALLOWED_ACTIONS`].
/// Every CI configuration in the repository, dot directories included, is
/// one this guard reads: the GitHub workflows and `.gitlab-ci.yml`, which
/// includes no other file. A pipeline file of any other kind or place fails
/// until it is reviewed.
#[test]
fn p0_fg_standalone_no_workflow_ships_a_standalone_binary() {
    let root = workspace_root();
    let configs: Vec<String> = repository_files()
        .into_iter()
        .filter(|relative| is_ci_config(relative))
        .collect();
    let mut expected: Vec<String> = sorted_dir(&root.join(".github").join("workflows"))
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yml" || extension == "yaml")
        })
        .map(|path| {
            format!(
                ".github/workflows/{}",
                path.file_name().expect("file name").to_string_lossy()
            )
        })
        .collect();
    assert!(expected.len() >= 5, "workflows not found");
    expected.push(".gitlab-ci.yml".to_string());
    expected.sort();
    assert_eq!(
        configs, expected,
        "every CI configuration must be one this guard reads"
    );
    let gitlab = read(&root.join(".gitlab-ci.yml"));
    assert!(
        !declares_include(&gitlab),
        ".gitlab-ci.yml must include no other file"
    );
    let withdrawn_directories: BTreeSet<String> = BINARY_TARGETS
        .iter()
        .filter(|(_, _, _, disposition)| *disposition == Withdrawn)
        .map(|(member, _, _, _)| member.to_string())
        .collect();
    let withdrawn_packages: BTreeSet<String> = withdrawn_directories
        .iter()
        .map(|member| {
            package_value(&member_manifest(member), "name")
                .and_then(toml::Value::as_str)
                .expect("a package name")
                .to_string()
        })
        .collect();
    assert_eq!(withdrawn_packages.len(), 8, "{withdrawn_packages:?}");
    for config in &configs {
        let workflow = root.join(config);
        let text = read(&workflow);
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
            let outputs = undesktop_build_outputs(line);
            assert!(
                outputs.is_empty(),
                "{name}: build output other than the desktop bundle {outputs:?} in {line}"
            );
            for action in action_references(line) {
                assert!(
                    ALLOWED_ACTIONS.contains(&action),
                    "{name}: action `{action}` is not allowlisted in {line}"
                );
            }
        }
        let shipped = shipped_withdrawn_packages(
            &text,
            &withdrawn_packages,
            &withdrawn_directories,
            WITHDRAWN_BINARIES,
        );
        assert!(
            shipped.is_empty(),
            "{name}: a withdrawn package is built for shipping by {shipped:?}"
        );
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
