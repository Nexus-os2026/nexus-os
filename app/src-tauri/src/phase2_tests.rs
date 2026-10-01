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

/// The top-level item of `source` starting at `head`, through its closing
/// brace (or the end of `source`).
fn item<'a>(source: &'a str, head: &str) -> &'a str {
    let start = source.find(head).unwrap_or_else(|| panic!("{head}"));
    let rest = &source[start..];
    &rest[..rest.find("\n}\n").map_or(rest.len(), |end| end + 2)]
}

/// The item at `head` inside an inline module: from `head` to the closing
/// brace at the module's own indentation.
fn indented_item<'a>(source: &'a str, head: &str) -> &'a str {
    let start = source.find(head).unwrap_or_else(|| panic!("{head}"));
    let rest = &source[start..];
    &rest[..rest.find("\n    }\n").map_or(rest.len(), |end| end + 6)]
}

#[test]
fn p2_g_09_cleanup_is_observed_only_through_the_checked_observations() {
    let read = |path: &str| std::fs::read_to_string(repo().join(path)).unwrap();
    // The live suite and its fixture controls share one checked observation,
    // made in their own process: it starts no process, reads no environment,
    // subscribes to nothing and has no authority to stop, kill, create or
    // remove anything.
    let support = code(&read(
        "crates/nexus-verifier-sandbox/tests/support/cleanup_observation.rs",
    ));
    let harness = code(&read(
        "crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs",
    ));
    let controls = code(&read(
        "crates/nexus-verifier-sandbox/tests/phase2_cleanup_observation.rs",
    ));
    for source in [&harness, &controls] {
        assert_eq!(
            source
                .matches("#[path = \"support/cleanup_observation.rs\"]")
                .count(),
            1
        );
    }
    for needle in [
        "Command",
        "process::Child",
        "process::Stdio",
        "spawn",
        "fork",
        "exec",
        "kill",
        "waitid",
        "waitpid",
        "pidfd",
        "systemctl",
        "busctl",
        "env::var",
        "var_os",
        "set_var",
        "Builder::address",
        "Builder::session",
        "Builder::system",
        "Connection::session",
        "Connection::system",
        "add_match",
        "AddMatch",
        "Subscribe",
        "StopUnit",
        "KillUnit",
        "ResetFailed",
        "remove_dir",
        "remove_file",
        "unlink",
        "rmdir",
        "set_permissions",
        "create_dir",
    ] {
        assert!(!support.contains(needle), "{needle}");
    }
    // One bounded call to one method of the user manager, on the checked bus
    // of this process's real uid, connected through the checked socket's own
    // descriptor and authenticated as this process's own uid; the
    // observation's runtime, and the connection with it, ends before the
    // reply is judged.
    for pin in [
        "pub const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(10);",
        "pub const MANAGER: &str = \"org.freedesktop.systemd1\";",
        "pub const MANAGER_PATH: &str = \"/org/freedesktop/systemd1\";",
        "pub const MANAGER_INTERFACE: &str = \"org.freedesktop.systemd1.Manager\";",
        "pub const LIST_UNITS: &str = \"ListUnitsByPatterns\";",
        "pub const STATES: [&str; 0] = [];",
        "pub const SCOPE_PATTERN: &str = \"nexus-verifier-*.scope\";",
        "pub const PATTERNS: [&str; 1] = [SCOPE_PATTERN];",
        "pub const REPLY_SIGNATURE: &str = \"a(ssssssouso)\";",
        "pub const MAX_REPLY_BODY: usize = 64 * 1024;",
        "pub const MAX_UNITS: usize = 256;",
    ] {
        assert_eq!(support.matches(pin).count(), 1, "{pin}");
    }
    assert!(indented_item(&support, "    impl Host<'static> {")
        .contains("uid: unsafe { libc::getuid() },"));
    let on = indented_item(&support, "    pub fn observe_scopes_on(");
    assert!(on.contains(
        "let deadline = deadline.min(Instant::now() + OBSERVATION_TIMEOUT);\n        \
         let bus = user_bus(host)?;\n"
    ));
    assert!(on.contains(
        "let scopes = list_scopes(move || tokio::net::UnixStream::connect(path), deadline);"
    ));
    let bus = indented_item(&support, "    pub fn user_bus(");
    assert!(bus.contains("let (runtime, _) = runtime_dir(host, &Fs::real())?;"));
    assert!(bus.contains("open_at(&runtime, \"bus\", libc::O_PATH)"));
    assert!(bus.contains("st.st_mode & libc::S_IFMT != libc::S_IFSOCK || st.st_uid != host.uid"));
    assert!(
        support.contains("PathBuf::from(format!(\"/proc/self/fd/{}\", self.socket.as_raw_fd()))")
    );
    assert!(indented_item(&support, "    fn open_at(")
        .contains("flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,"));
    let list = indented_item(&support, "    pub fn list_scopes<");
    for needle in [
        "zbus::connection::Builder::unix_stream(stream)\n                    \
         .auth_mechanism(zbus::AuthMechanism::External)\n",
        ".call_method(\n                        Some(MANAGER),\n                        \
         MANAGER_PATH,\n                        Some(MANAGER_INTERFACE),\n                        \
         LIST_UNITS,\n                        &(&STATES[..], &PATTERNS[..]),\n                    )",
        "tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), call).await",
        "Err(_elapsed) => Err(ObservationError::Timeout),",
        "Ok(Err(error)) => Err(error),",
    ] {
        assert!(list.contains(needle), "{needle}");
    }
    assert!(
        list.find("drop(runtime);").unwrap()
            < list
                .find("Ok(Ok(reply)) => decode_scopes(&reply),")
                .unwrap()
    );
    for (needle, count) in [
        (".call_method(", 1),
        ("timeout_at(", 1),
        ("Builder::unix_stream(", 1),
        (".auth_mechanism(", 1),
    ] {
        assert_eq!(support.matches(needle).count(), count, "{needle}");
    }
    // The reply: exactly the documented list, its body bounded before it is
    // decoded, the whole body consumed by decoding that one list (decoded
    // from the body's own data, keeping the bytes consumed) before anything
    // succeeds, every unit a distinct verifier scope with bounded fields.
    let decode = indented_item(&support, "    pub fn decode_scopes(");
    let bounded = decode
        .find("if body.len() > MAX_REPLY_BODY {")
        .expect("the body bound");
    let decoded = decode
        .find(
            "let (rows, consumed): (Vec<UnitRow>, usize) = body\n            .data()\n            \
             .deserialize_for_dynamic_signature(REPLY_SIGNATURE)\n",
        )
        .expect("the decoding, keeping the bytes it consumed");
    let complete = decode
        .find("if consumed != body.len() {\n            return Err(malformed(")
        .expect("a body not wholly consumed is refused");
    let answered = decode.find("Ok(scopes)").expect("the answer");
    assert!(bounded < decoded && decoded < complete && complete < answered);
    assert!(!decode[..complete].contains("Ok("));
    for needle in [".deserialize()", ".deserialize::<", "deserialize_unchecked"] {
        assert!(!support.contains(needle), "{needle}");
    }
    for needle in [
        "if !reply.data().fds().is_empty() {",
        "Some(signature) if signature.as_str() == REPLY_SIGNATURE => {}",
        "if rows.len() > MAX_UNITS {",
        "check_row(row)?;",
        "if !scopes.insert(row.0.clone()) {",
    ] {
        assert!(decode.contains(needle), "{needle}");
    }
    // A wait gives every observation its caller's one deadline, and a failed
    // observation ends it.
    let wait = indented_item(&support, "    pub fn wait_for(");
    assert!(wait.contains("mut observe: impl FnMut(Instant) -> Result<Scopes, ObservationError>,"));
    assert!(wait.contains(
        "let deadline = Instant::now() + within;\n        loop {\n            \
         if done(&observe(deadline)?) {"
    ));
    // Workspaces: reached beneath the checked runtime directory without
    // following a symlink, absent only as a missing final component of a
    // runtime directory still there, checked before they are listed.
    let workspaces = indented_item(&support, "    pub fn observe_workspaces_with(");
    for needle in [
        "let (runtime, runtime_st) = runtime_dir(host, fs)?;",
        "Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {",
        "if st.st_nlink == 0 {",
        "|| st.st_dev != runtime_st.st_dev",
    ] {
        assert!(workspaces.contains(needle), "{needle}");
    }
    assert_eq!(workspaces.matches("Ok(Workspaces::Absent)").count(), 1);
    // Both observations always run, a panic in one included; absence is
    // reported as such, never as "never created".
    let report = indented_item(&support, "    pub fn report_cleanup(");
    assert!(report.contains(
        "let scopes = contained(scopes, Err(ObservationError::Panicked));\n        \
         let workspaces = contained(workspaces, Err(ObservationError::Panicked));"
    ));
    assert!(report.contains("absent at observation time"));
    assert!(!support.contains("never created"));
    let mode = indented_item(&support, "    pub fn cleanup_observation_mode(");
    assert!(mode.contains("let host = Host::real();"));
    assert!(
        mode.contains("report_cleanup(observe_scopes, || observe_workspaces(&host), &mut |line| {")
    );

    // The entry: the arguments alone choose, before anything else happens;
    // only no argument at all enters the suite, exactly
    // `--cleanup-observation` the observation, and anything else is refused.
    let dispatch = item(&support, "pub fn dispatch<E: Entry>(");
    for needle in [
        "return entry.refuse(Refusal::NoProgram);",
        "return entry.refuse(Refusal::NotUtf8);",
        "let Some(first) = rest.first() else {\n        return entry.suite();\n    };",
        "if rest.len() != 1 {",
        "None => entry.refuse(Refusal::Unknown(first.clone())),",
    ] {
        assert!(dispatch.contains(needle), "{needle}");
    }
    assert_eq!(dispatch.matches("entry.suite()").count(), 1);
    assert_eq!(dispatch.matches("entry.cleanup_observation()").count(), 1);
    assert!(harness.contains(
        "fn main() -> ExitCode {\n    let args: Vec<OsString> = std::env::args_os().collect();\n    \
         cleanup_observation::dispatch(&args, true, &mut live::Live)\n}"
    ));
    assert!(harness.contains("cleanup_observation::dispatch(&args, false, &mut Unsupported)"));
    for needle in ["std::env::args()", "_ => suite()", "pub fn main()"] {
        assert!(!harness.contains(needle), "{needle}");
    }
    let live = indented_item(
        &harness,
        "    impl crate::cleanup_observation::Entry for Live {",
    );
    assert!(live.contains(
        "fn cleanup_observation(&mut self) -> Self::Exit {\n            \
         crate::cleanup_observation::cleanup_observation_mode()\n"
    ));
    assert_eq!(harness.matches("suite();").count(), 1);
    let unsupported = item(
        &harness,
        "impl cleanup_observation::Entry for Unsupported {",
    );
    assert!(!unsupported.contains("cleanup_observation_mode"));
    assert_eq!(unsupported.matches("ExitCode::SUCCESS").count(), 1);

    // The suite queries nothing itself and never reads a failed observation
    // as "no scopes": each use is checked before anything is owned, checked
    // by a wait (on its caller's one deadline) that ends at a failed
    // observation, or the retained boundary's evidence.
    for needle in [
        "systemctl",
        "reported(",
        "loaded_scopes() <",
        "loaded_scopes() >",
    ] {
        assert!(!harness.contains(needle), "{needle}");
    }
    assert!(harness.contains(
        "pub fn loaded_scopes() -> Result<Scopes, ObservationError> {\n            \
         cleanup_observation::observe_scopes()\n"
    ));
    assert!(harness.contains(
        "pub fn loaded_scopes_by(deadline: Instant) -> Result<Scopes, ObservationError> {\n            \
         cleanup_observation::observe_scopes_by(deadline)\n"
    ));
    assert_eq!(harness.matches("loaded_scopes()").count(), 4);
    assert_eq!(
        harness.matches(".unwrap_or_else(|error| panic!(").count(),
        2
    );
    assert_eq!(
        harness
            .matches("wait_for(Duration::from_secs(10), loaded_scopes_by, |now| {")
            .count(),
        2
    );
    assert_eq!(
        harness
            .matches("loaded_scopes().map_err(|error| error.to_string())")
            .count(),
        1
    );
    // A retry's result keeps its owner: the harness never turns a retry into
    // a boolean, and retries only through `settle` (directly or in
    // `judge_retained`), which keeps each failed attempt's owner.
    for needle in [
        ".retry().is_ok()",
        ".retry().is_err()",
        "boundary.retry()",
        "after_retry",
    ] {
        assert!(!harness.contains(needle), "{needle}");
    }
    assert_eq!(harness.matches("RetainedBoundary::retry").count(), 2);
    assert!(
        harness.contains("match settle(boundary, EXPLICIT_ATTEMPTS, RetainedBoundary::retry) {")
    );
    // Whatever the report owns leaves it before any check, and nothing
    // between the execution and that can fail.
    let ran = harness
        .find("let (report, most) = watched(&marker, || {")
        .expect("the execution");
    let taken = harness
        .find("let ExecutionReport {")
        .expect("the report's owner taken out");
    let judged = harness
        .find("match (expect, boundary) {")
        .expect("the judgement");
    assert!(ran < taken && taken < judged);
    for needle in ["assert", "panic!", ".unwrap()", ".expect("] {
        assert!(!harness[ran..taken].contains(needle), "{needle}");
    }
    assert!(harness.contains("(result, watcher.join().ok())"));
    // The retained boundary: evidence taken while it is held (a panic there
    // contained), then the bounded explicit cleanup, then every judgement;
    // an unconfirmed owner is released explicitly, once reported, before
    // the suite stops.
    let retained = item(&harness, "        fn retained(");
    let evidence = retained
        .find("let evidence = catch_unwind(AssertUnwindSafe(|| {")
        .expect("contained evidence");
    let settled = retained.find("judge_retained(").expect("the judgement");
    assert!(evidence < settled);
    assert!(retained.contains("EXPLICIT_ATTEMPTS,\n                RetainedBoundary::retry,"));
    assert!(retained.contains(
        "Verdict::Unconfirmed { owner, failures } => {\n                    \
         stop_unconfirmed(name, &failures, owner)"
    ));
    let stop = item(&harness, "        fn stop_unconfirmed(");
    assert!(
        stop.find("let report = release(owner, name, failures);")
            .unwrap()
            < stop.find("panic!(\"{report}\")").unwrap()
    );
    let settle = item(&support, "pub fn settle<B>(");
    assert!(settle.contains("Err(owner) => owner,"));
    let judge = item(&support, "pub fn judge_retained<B>(");
    assert!(
        judge.contains(") -> Verdict<B> {\n    let settled = settle(owner, attempts, retry);\n")
    );
    assert!(judge
        .contains("Settled::Unconfirmed(owner, _) => Verdict::Unconfirmed { owner, failures },"));
    let release = item(&support, "pub fn release<B>(");
    assert!(
        release.find("releasing the unconfirmed owner").unwrap()
            < release.find("drop(owner);").unwrap()
    );

    // The workflow observes only through the live harness's observation-only
    // mode of this checkout: after running the fixture controls, a gate
    // before the live suite (an observation that cannot answer, or anything
    // found, blocks it, and nothing is cleaned up), then the suite with every
    // result kept (its own status and its output capture's, each taken from
    // the pipeline at once, its exact passed count) and the observation after
    // it. The Python runtime checker is gone.
    let workflow = read(".github/workflows/ci-phase2-linux-sandbox.yml");
    for needle in [
        "systemctl",
        "ls -A",
        "grep -q .",
        "|| true",
        "|| live=",
        "continue-on-error",
        "always()",
        "scripts/ci/phase2_cleanup_check.py",
    ] {
        assert!(!workflow.contains(needle), "{needle}");
    }
    let live = &workflow[workflow
        .find("- name: Live Phase Two isolation")
        .expect("the live step")..];
    let live = &live[..live.find("\n      - name: ").expect("the next step")];
    assert!(!live.contains("2>/dev/null"), "{live}");
    assert!(!live.contains("python3"), "{live}");
    let mut at = 0;
    for line in [
        "- name: Cleanup observation controls (fixtures only)\n",
        "cargo test -p nexus-verifier-sandbox --locked --test phase2_cleanup_observation\n",
        "python3 scripts/ci/test_phase2_cleanup_check.py\n",
        "- name: Live Phase Two isolation, escape and cleanup suite (every layer required)\n",
        "NEXUS_PHASE2_REQUIRE_LIVE_SANDBOX: \"1\"\n",
        "observe_cleanup() {\n",
        "cargo test -p nexus-verifier-sandbox --locked --features development-toolchain \
         --test phase2_live_sandbox -- --cleanup-observation\n",
        "}\n",
        "preflight=0\n",
        "observe_cleanup || preflight=$?\n",
        "if (( preflight != 0 )); then\n",
        "the live suite was not run\"\n",
        "exit 1\n",
        "fi\n",
        "if cargo test -p nexus-verifier-sandbox --locked --features development-toolchain \
         --test phase2_live_sandbox 2>&1 | tee phase2-live.log; then\n",
        "statuses=(\"${PIPESTATUS[@]}\")\n",
        "else\n",
        "statuses=(\"${PIPESTATUS[@]}\")\n",
        "fi\n",
        "live=${statuses[0]:-255}\n",
        "captured=${statuses[1]:-255}\n",
        "passed=0\n",
        "grep -qx 'test result: ok. 31 live sandbox cases passed' phase2-live.log || passed=$?\n",
        "observed=0\n",
        "observe_cleanup || observed=$?\n",
        "if (( captured != 0 )); then\n",
        "if (( passed != 0 )); then\n",
        "if (( observed != 0 )); then\n",
        "if (( live != 0 )); then\n",
        "exit \"$live\"\n",
        "if (( captured != 0 || passed != 0 || observed != 0 )); then\n",
        "exit 1\n",
        "- name: Phase Two kernel and desktop controls\n",
    ] {
        let found = workflow[at..]
            .find(line)
            .unwrap_or_else(|| panic!("the workflow lacks, in order: {line}"));
        at += found + line.len();
    }
    for (needle, count) in [
        ("--cleanup-observation", 1),
        ("observe_cleanup", 3),
        ("scripts/ci/test_phase2_cleanup_check.py", 1),
    ] {
        assert_eq!(workflow.matches(needle).count(), count, "{needle}");
    }
    assert!(!repo().join("scripts/ci/phase2_cleanup_check.py").exists());
    // Its step's own script is what the Python controls run, with stand-ins:
    // no runtime observer is left in Python.
    let steps = read("scripts/ci/test_phase2_cleanup_check.py");
    assert!(steps.contains(
        "LIVE_STEP = \"      - name: Live Phase Two isolation, escape and cleanup suite \
         (every layer required)\""
    ));
    for needle in ["import phase2_cleanup_check", "os.waitid", "killpg", "dbus"] {
        assert!(!steps.contains(needle), "{needle}");
    }
}
