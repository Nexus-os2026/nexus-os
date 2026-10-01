//! Phase Two trust-surface pins: governed sandboxed verification.
//!
//! The only execution route the desktop has for project code is the
//! verifier sandbox, reached from one module, launched once, from backend
//! objects only; the sandbox crate spawns exactly one process (its trusted
//! helper) and takes nothing from the environment; the development toolchain
//! is never production; the profile and both policies are pinned by hash.
//! A failure here is a trust-surface change: review it, then update the pin.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Rust source without line comments (doc text may name what is forbidden).
fn code(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Non-test Rust files beneath `dir`.
fn production_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                production_files(&path, out);
            }
        } else if name.ends_with(".rs") && !name.ends_with("tests.rs") && name != "tests.rs" {
            out.push(path);
        }
    }
}

fn count_in(files: &[PathBuf], needle: &str) -> Vec<(String, usize)> {
    files
        .iter()
        .filter_map(|path| {
            let found = code(&std::fs::read_to_string(path).unwrap())
                .matches(needle)
                .count();
            (found > 0).then(|| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    found,
                )
            })
        })
        .collect()
}

#[test]
fn p2_g_01_the_desktop_reaches_project_code_only_through_the_verifier_sandbox() {
    let flow = code(include_str!("coding_flow/verification.rs"));
    for (needle, expected) in [
        ("execution::run(", 1),
        ("HelperProgram::installed()", 1),
        ("VerifiedVerifierToolchain::installed()", 1),
        ("launch_spec(", 1),
        ("HelperProgram::at", 0),
        ("development(", 0),
        ("Command", 0),
        ("std::process", 0),
        ("pre_exec", 0),
        ("std::env", 0),
        ("std::fs", 0),
    ] {
        assert_eq!(flow.matches(needle).count(), expected, "{needle}");
    }
    // The verification module is part of the governed coding flow: only
    // `coding_flow.rs` includes it.
    let flow_root = include_str!("coding_flow.rs");
    assert_eq!(
        flow_root
            .matches("#[path = \"coding_flow/verification.rs\"]")
            .count(),
        1
    );
    assert!(!include_str!("lib.rs").contains("verification.rs"));
    // No other desktop source reaches the sandbox crate.
    let mut files = Vec::new();
    production_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut users: Vec<String> = count_in(&files, "nexus_verifier_sandbox")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    users.sort();
    assert_eq!(users, ["coding_flow.rs", "verification.rs"]);
}

#[test]
fn p2_g_02_the_sandbox_crate_spawns_only_its_helper_and_reads_no_environment() {
    let mut files = Vec::new();
    production_files(
        &repo().join("crates/nexus-verifier-sandbox/src"),
        &mut files,
    );
    assert!(files.len() >= 15, "sandbox sources not found");
    let pins: [(&str, &[(&str, usize)]); 11] = [
        ("Command::new(", &[("launcher.rs", 1)]),
        ("pre_exec", &[]),
        ("CommandExt", &[]),
        ("sys::execveat_fd(", &[("helper.rs", 1)]),
        ("libc::fork(", &[("sys.rs", 1)]),
        ("sys::unshare(", &[("helper.rs", 1)]),
        ("libc::socket(", &[("sys.rs", 1)]),
        ("current_exe()", &[("launcher.rs", 1), ("toolchain.rs", 1)]),
        ("std::env::var", &[]),
        ("env::var_os", &[]),
        ("XDG_RUNTIME_DIR", &[]),
    ];
    for (needle, expected) in pins {
        let mut found = count_in(&files, needle);
        found.sort();
        let expected: Vec<(String, usize)> = expected
            .iter()
            .map(|(name, n)| (name.to_string(), *n))
            .collect();
        assert_eq!(found, expected, "{needle}");
    }
}

/// Every Cargo manifest of the repository (build output, dependencies and
/// assembled toolchains aside).
fn manifests() -> Vec<PathBuf> {
    let mut manifests = Vec::new();
    let mut pending = vec![repo()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !matches!(
                    name.as_str(),
                    "target" | "node_modules" | ".git" | ".claude" | "verifier-toolchain"
                ) && !name.starts_with('.')
                {
                    pending.push(path);
                }
            } else if name == "Cargo.toml" {
                manifests.push(path);
            }
        }
    }
    assert!(manifests.len() > 10, "workspace manifests not found");
    manifests
}

#[test]
fn p2_g_03_the_development_toolchain_is_never_production() {
    let toolchain = include_str!("../../../crates/nexus-verifier-sandbox/src/toolchain.rs");
    let at = toolchain
        .find("pub fn development(")
        .expect("development constructor");
    let before = &toolchain[..at];
    let attribute = before
        .rfind("#[cfg(feature = \"development-toolchain\")]")
        .unwrap();
    assert!(
        !before[attribute..].contains("pub fn "),
        "the development constructor is compiled only with its feature"
    );
    // Only the sandbox crate names the feature; nothing enables it.
    for manifest in manifests() {
        let text = std::fs::read_to_string(&manifest).unwrap();
        if text.contains("development-toolchain") {
            assert!(
                manifest.ends_with("crates/nexus-verifier-sandbox/Cargo.toml"),
                "{} names the development toolchain",
                manifest.display()
            );
        }
    }
    let app = include_str!("../Cargo.toml");
    let line = app
        .lines()
        .find(|line| line.starts_with("nexus-verifier-sandbox"))
        .expect("the desktop's sandbox dependency");
    assert!(!line.contains("features"), "{line}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn p2_g_04_the_profile_and_policies_are_pinned() {
    use nexus_verifier_sandbox::policy::{ResourcePolicy, SandboxPolicy};
    use nexus_verifier_sandbox::profile::VerifierProfileId;
    let names: Vec<&str> = VerifierProfileId::PRODUCTION
        .iter()
        .map(|id| id.name())
        .collect();
    assert_eq!(names, ["rust.cargo-test.offline.v1"]);
    let profile = VerifierProfileId::PRODUCTION[0].profile();
    assert_eq!(
        (profile.argv0, profile.args),
        (
            "cargo",
            &[
                "test",
                "--offline",
                "--locked",
                "--no-fail-fast",
                "--lib",
                "--tests"
            ][..]
        )
    );
    assert!(profile.env.iter().all(|var| var.key != "PATH"));
    for (what, actual, pinned) in [
        ("profile", hex::encode(profile.hash().bytes()), PROFILE_PIN),
        (
            "sandbox policy",
            hex::encode(SandboxPolicy::V1.hash().bytes()),
            SANDBOX_POLICY_PIN,
        ),
        (
            "resource policy",
            hex::encode(ResourcePolicy::RUST_OFFLINE_V1.hash().bytes()),
            RESOURCE_POLICY_PIN,
        ),
    ] {
        assert_eq!(
            actual, pinned,
            "the {what} changed: review it, then update the pin"
        );
    }
}

/// The attribute that compiles an item only for the sandbox crate's own
/// tests and its live sandbox harness.
const HARNESS_ONLY: &str = "#[cfg(any(test, feature = \"live-sandbox-harness\"))]";

/// Whether the public function `name` in `source` is compiled only under
/// `attribute` (the attribute directly precedes it, with only its
/// documentation between).
fn gated(source: &str, name: &str, attribute: &str) -> bool {
    let Some(at) = source.find(&format!("pub fn {name}(")) else {
        return false;
    };
    let before = &source[..at];
    before.rfind(attribute).is_some_and(|start| {
        before[start + attribute.len()..]
            .lines()
            .all(|line| line.trim().is_empty() || line.trim().starts_with("///"))
    })
}

#[test]
fn p2_g_06_production_cannot_construct_a_helper_from_an_arbitrary_path() {
    let sandbox = repo().join("crates/nexus-verifier-sandbox");
    let launcher = std::fs::read_to_string(sandbox.join("src/launcher.rs")).unwrap();
    let execution = std::fs::read_to_string(sandbox.join("src/execution.rs")).unwrap();
    // A helper is a backend-installed program (`installed`) or, only for the
    // sandbox crate's own tests and live harness, the build's own helper
    // (`at`). Nothing else constructs one.
    assert!(gated(&launcher, "at", HARNESS_ONLY), "HelperProgram::at");
    assert!(
        gated(&launcher, "in_extracted_package", HARNESS_ONLY),
        "HelperProgram::in_extracted_package"
    );
    assert!(!gated(&launcher, "installed", HARNESS_ONLY));
    assert_eq!(code(&launcher).matches("Self { path").count(), 2);
    for function in ["run_with_fault", "holds_scope", "holds_helper"] {
        assert!(gated(&execution, function, HARNESS_ONLY), "{function}");
    }
    let mut sources = Vec::new();
    production_files(&sandbox.join("src"), &mut sources);
    assert!(
        count_in(&sources, "HelperProgram { path").is_empty(),
        "only its own constructors build a helper"
    );
    // Only the sandbox crate names the harness feature, and only its own
    // dev-dependency enables it: no normal build (the desktop backend, the
    // release package) is compiled with it.
    let manifest = std::fs::read_to_string(sandbox.join("Cargo.toml")).unwrap();
    let section = |name: &str| {
        let header = format!("\n[{name}]\n");
        let start = manifest.find(&header).expect(name) + header.len();
        let rest = &manifest[start..];
        rest[..rest.find("\n[").unwrap_or(rest.len())].to_string()
    };
    let enabling: Vec<String> = manifest
        .lines()
        .filter(|line| line.contains("live-sandbox-harness") && !line.trim_start().starts_with('#'))
        .map(str::to_string)
        .collect();
    assert_eq!(enabling.len(), 2, "{enabling:?}");
    assert!(section("features").contains("live-sandbox-harness = []"));
    assert!(section("dev-dependencies").contains(
        "nexus-verifier-sandbox = { path = \".\", features = [\"live-sandbox-harness\"] }"
    ));
    let mut desktop = Vec::new();
    production_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut desktop,
    );
    for needle in [
        "HelperProgram::at",
        "in_extracted_package",
        "run_with_fault",
        "Fault::",
        "FaultPoint",
        "holds_scope",
        "holds_helper",
        "live-sandbox-harness",
        "live_sandbox_harness",
    ] {
        assert!(count_in(&desktop, needle).is_empty(), "{needle}");
    }
    for manifest in manifests() {
        if !manifest.ends_with("crates/nexus-verifier-sandbox/Cargo.toml") {
            let text = std::fs::read_to_string(&manifest).unwrap();
            assert!(
                !text.contains("live-sandbox-harness"),
                "{} names the live sandbox harness",
                manifest.display()
            );
        }
    }
}

/// The body of `fn name` in `source`, up to the next item at its depth.
fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = [format!("fn {name}("), format!("fn {name}<")]
        .iter()
        .find_map(|head| source.find(head.as_str()))
        .unwrap_or_else(|| panic!("fn {name}"));
    let rest = &source[start..];
    let end = rest[1..]
        .find("\n    fn ")
        .into_iter()
        .chain(rest[1..].find("\n    pub(crate) fn "))
        .chain(rest[1..].find("\nfn "))
        .min()
        .unwrap_or(rest.len() - 1);
    &rest[..end + 1]
}

#[test]
fn p2_g_07_a_retained_verification_cleanup_is_never_dropped() {
    let flow = code(include_str!("coding_flow/verification.rs"));
    // The verification thread holds what it owns outside the panicking
    // work, and a panic is settled from it.
    let execute = function(&flow, "execute");
    assert!(execute.contains("let mut owned = Owned::new(Some(workspace));"));
    assert!(execute.contains("guarded(&mut owned,"));
    let guarded = function(&flow, "guarded");
    assert!(guarded.contains("Err(_) => after_panic(owned)"));
    // The execution's unconfirmed boundary leaves its report at once.
    let launch = function(&flow, "run_launch");
    let run = launch.find("execution::run(").unwrap();
    let stash = launch.find("owned.boundary = Some(boundary);").unwrap();
    assert!(run < stash);
    assert!(!launch[run..stash].contains("with_run("));
    // A retry claims the run before it takes the retained cleanup, so a
    // refused claim cannot drop it.
    let retry = function(&flow, "retry_verification_cleanup");
    let claimed = retry.find("claim(&slot").unwrap();
    let taken = retry.find("take_retained(").unwrap();
    assert!(claimed < taken);
    // A run whose verification cleanup is unconfirmed cannot be discarded.
    let root = code(include_str!("coding_flow.rs"));
    let discard = function(&root, "discard");
    let refused = discard
        .find("verification_phase().blocks_apply()")
        .expect("discard refuses an unconfirmed verification cleanup");
    assert!(refused < discard.find("discard_run(&mut run)").unwrap());
}

/// The release workflow job `name` (from its key to the next job's).
fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let start = workflow
        .find(&format!("\n  {name}:\n"))
        .unwrap_or_else(|| panic!("job {name}"))
        + 1;
    let rest = &workflow[start..];
    let end = rest
        .match_indices("\n  ")
        .find(|(at, _)| {
            let line = &rest[at + 3..];
            line.starts_with(|c: char| c.is_ascii_alphanumeric())
                && line
                    .split('\n')
                    .next()
                    .is_some_and(|key| key.ends_with(':'))
        })
        .map_or(rest.len(), |(at, _)| at + 1);
    &rest[..end]
}

#[test]
fn p2_g_08_the_linux_package_installs_the_verifier_runtime() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(manifest_dir.join(name)).unwrap()).unwrap()
    };
    // Development builds bundle nothing; the Linux release merges, after
    // the Builder toolchain, the verifier toolchain resource and the helper
    // sidecar, which the Debian package installs at
    // /usr/lib/NexusOS/verifier-toolchain and /usr/bin/nexus-verifier-sandbox.
    let base = json("tauri.conf.json");
    assert_eq!(base["productName"], "NexusOS");
    assert!(base["bundle"].get("externalBin").is_none());
    assert_eq!(
        json("tauri.verifier-runtime.conf.json"),
        serde_json::json!({
            "bundle": {
                "resources": { "verifier-toolchain": "verifier-toolchain" },
                "externalBin": ["binaries/nexus-verifier-sandbox"]
            }
        })
    );
    // Exactly the layout production derives, and the package inspection
    // checks.
    let read = |path: &str| std::fs::read_to_string(repo().join(path)).unwrap();
    let launcher = read("crates/nexus-verifier-sandbox/src/launcher.rs");
    assert!(launcher.contains("const INSTALLED_BIN_DIR: &str = \"/usr/bin\";"));
    assert!(launcher.contains("const INSTALLED_HELPER: &str = \"nexus-verifier-sandbox\";"));
    let toolchain = read("crates/nexus-verifier-sandbox/src/toolchain.rs");
    assert!(
        toolchain.contains("const INSTALLED_ROOT: &str = \"/usr/lib/NexusOS/verifier-toolchain\";")
    );
    let inspect = read("packaging/verifier-toolchain/scripts/inspect-deb.mjs");
    for constant in [
        "const HELPER = 'usr/bin/nexus-verifier-sandbox';",
        "const VERIFIER_ROOT = 'usr/lib/NexusOS/verifier-toolchain';",
        "const BUILDER_ROOT = 'usr/lib/NexusOS/toolchain';",
    ] {
        assert!(inspect.contains(constant), "{constant}");
    }
    let stage = read("packaging/verifier-toolchain/scripts/stage-helper.mjs");
    assert!(stage.contains("const TRIPLE = 'x86_64-unknown-linux-gnu';"));
    // The staged sidecar is build output, never committed.
    assert!(read(".gitignore")
        .lines()
        .any(|line| line == "/app/src-tauri/binaries/nexus-verifier-sandbox-*"));

    // The Linux release job builds, packages and proves the runtime, in
    // this order (the Builder toolchain's own steps are pinned by its
    // guard); Windows and macOS have no verifier sandbox.
    let workflow = read(".github/workflows/release.yml");
    let linux = job(&workflow, "build-linux");
    let mut at = 0;
    for step in [
        "NEXUS_BUILDER_TOOLCHAIN: packaged",
        "NEXUS_VERIFIER_TOOLCHAIN: packaged",
        "packaging/verifier-toolchain/scripts/assemble.mjs --out app/src-tauri/verifier-toolchain",
        "run: cargo build --release",
        "id: helper",
        "packaging/verifier-toolchain/scripts/stage-helper.mjs --out app/src-tauri/binaries >> \"$GITHUB_OUTPUT\"",
        "npm run tauri build -- --bundles deb --config src-tauri/tauri.",
        ".conf.json --config src-tauri/tauri.verifier-runtime.conf.json\n",
        "assemble.mjs --compare app/src-tauri/verifier-toolchain",
        "node --test packaging/verifier-toolchain/test/inspect-deb.test.mjs",
        "HELPER_SHA256: ${{ steps.helper.outputs.sha256 }}",
        "packaging/verifier-toolchain/scripts/inspect-deb.mjs",
        "--application nexus-desktop-backend",
        "--verifier-toolchain app/src-tauri/verifier-toolchain",
        "--helper app/src-tauri/binaries/nexus-verifier-sandbox-x86_64-unknown-linux-gnu",
        "--helper-sha256 \"$HELPER_SHA256\"",
        "cargo tree --locked -p nexus-desktop-backend -e features,normal,build -i nexus-verifier-sandbox",
        "grep -q -e development-toolchain -e live-sandbox-harness",
        "dpkg-deb -x \"$deb\" \"$root\"",
        "NEXUS_EXTRACTED_PACKAGE_ROOT=\"$root\" cargo test -p nexus-verifier-sandbox --locked --features development-toolchain --test phase2_package_layout",
        "name: NexusOS-Linux",
    ] {
        let found = linux[at..]
            .find(step)
            .unwrap_or_else(|| panic!("the Linux release job lacks, in order: {step}"));
        at += found + step.len();
    }
    assert_eq!(workflow.matches("NEXUS_VERIFIER_TOOLCHAIN").count(), 1);
    assert_eq!(
        workflow.matches("tauri.verifier-runtime.conf.json").count(),
        1
    );
    // The Debian bundle merges exactly two configurations, the verifier
    // runtime's last.
    let bundle = linux
        .lines()
        .find(|line| line.contains("--bundles deb"))
        .expect("the Debian bundle");
    assert_eq!(bundle.matches("--config ").count(), 2, "{bundle}");
    for name in ["build-windows", "build-macos", "create-release"] {
        let other = job(&workflow, name);
        for needle in ["verifier", "nexus-verifier-sandbox", "binaries/"] {
            assert!(!other.contains(needle), "{name}: {needle}");
        }
    }
}

// Reviewed Phase Two identities (P2I).
const PROFILE_PIN: &str = "3b1726e81a65a3e3b2159fc500a98f77b688d639dfc2a5f2d2685f6b0aa24848";
const SANDBOX_POLICY_PIN: &str = "ce975e476e999e426253cfef71c50cd0e96ce1a0c7a4e88ab2fdd56a78342f7b";
const RESOURCE_POLICY_PIN: &str =
    "eeff12c6e175461ef424620187db7c1282605f2ae636e30fa304315db750d9fe";

#[test]
fn p2_g_05_verification_commands_are_registered_granted_and_nothing_else() {
    let lib = include_str!("lib.rs");
    let manifest = include_str!("webview_boundary/app_commands.rs");
    let capability = include_str!("../capabilities/app-commands.json");
    for command in [
        "coding_verification_profiles",
        "coding_start_verification",
        "coding_retry_verification_cleanup",
    ] {
        assert_eq!(
            lib.matches(&format!("fn {command}(")).count(),
            1,
            "{command}"
        );
        assert!(
            lib.contains(&format!("                {command},")),
            "{command}"
        );
        assert!(manifest.contains(&format!("\"{command}\"")), "{command}");
        let permission = format!("\"allow-{}\"", command.replace('_', "-"));
        assert!(capability.contains(&permission), "{permission}");
    }
    // No other command names verification or the sandbox.
    let registered: Vec<&str> = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"')?.strip_suffix("\","))
        .filter(|name| name.contains("verification") || name.contains("sandbox"))
        .collect();
    assert_eq!(
        registered,
        [
            "coding_retry_verification_cleanup",
            "coding_start_verification",
            "coding_verification_profiles"
        ]
    );
}
