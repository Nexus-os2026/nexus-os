//! P0-FG1: `crates/nexus-server` (dossier item J1) is withdrawn.
//!
//! The behavioural tests run the executable built from this package, never a
//! `nexus-server` found on `PATH`. The source guards pin the production entry
//! point and the withdrawn deployment recipes under `deploy/`.
//!
//! `nexus-protocols` also builds a binary named `nexus-server`. When one Cargo
//! invocation builds both (for example `cargo test --workspace`), the shared
//! output path Cargo gives this package's tests
//! (`CARGO_BIN_EXE_nexus-server`) may hold the protocols server instead
//! (an output filename collision). The protocols server listens on every
//! interface when started, so these tests execute a file only after checking
//! that rustc's dep-info names this package's `src/main.rs` and that the file
//! carries this package's withdrawal message.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WITHDRAWN_MESSAGE: &str = "nexus-server: unavailable during Phase Zero; deployment withdrawn";
const WITHDRAWN_STATUS: i32 = 69;
const RUN_DEADLINE: Duration = Duration::from_secs(60);

// ── The executable under test ───────────────────────────────────────────────

/// The executable built from this package's `nexus-server` target.
fn withdrawn_binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(resolve_withdrawn_binary)
}

fn resolve_withdrawn_binary() -> PathBuf {
    let uplifted = PathBuf::from(env!("CARGO_BIN_EXE_nexus-server"));
    assert!(
        uplifted.is_absolute(),
        "Cargo must give an absolute path, never a PATH lookup"
    );
    // Cargo builds each target into `deps/nexus_server-<hash>` and then copies
    // it to the shared path above. The builds of this package's target are
    // the ones whose dep-info names this package's `src/main.rs`; the one to
    // run also carries the withdrawal message (the unit-test build does not).
    let deps = uplifted
        .parent()
        .expect("the executable has a parent directory")
        .join("deps");
    let mut candidates: Vec<(SystemTime, PathBuf)> = fs::read_dir(&deps)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", deps.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            is_target_artifact(path) && built_from_this_package(path) && carries_withdrawal(path)
        })
        .map(|path| {
            let modified = fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            (modified, path)
        })
        .collect();
    candidates.sort();
    let newest = candidates.pop().map(|(_, path)| path).unwrap_or_else(|| {
        panic!(
            "no executable built from crates/nexus-server was found in {}; \
             refusing to run any other file",
            deps.display()
        )
    });
    // Prefer Cargo's own path when it holds exactly that build.
    if same_contents(&uplifted, &newest) {
        uplifted
    } else {
        newest
    }
}

/// A `deps/` file named for this target that the platform can execute.
fn is_target_artifact(path: &Path) -> bool {
    let named = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("nexus_server-"));
    let executable = if cfg!(windows) {
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    } else {
        path.extension().is_none()
    };
    named && executable && path.is_file()
}

/// Whether rustc's dep-info for this artifact names this package's entry
/// point (the protocols server's names `protocols/src/bin/nexus-server.rs`).
fn built_from_this_package(artifact: &Path) -> bool {
    fs::read_to_string(artifact.with_extension("d"))
        .map(|info| {
            info.replace('\\', "/")
                .contains("crates/nexus-server/src/main.rs")
        })
        .unwrap_or(false)
}

fn same_contents(a: &Path, b: &Path) -> bool {
    let same_len = match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    };
    same_len && matches!((fs::read(a), fs::read(b)), (Ok(a), Ok(b)) if a == b)
}

/// Whether the file's bytes contain this package's withdrawal message.
fn carries_withdrawal(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let needle = WITHDRAWN_MESSAGE.as_bytes();
    let mut window: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let read = match file.read(&mut chunk) {
            Ok(0) | Err(_) => return false,
            Ok(n) => n,
        };
        window.extend_from_slice(&chunk[..read]);
        if window.windows(needle.len()).any(|w| w == needle) {
            return true;
        }
        let keep = needle.len() - 1;
        if window.len() > keep {
            window.drain(..window.len() - keep);
        }
    }
}

// ── Isolated runs ───────────────────────────────────────────────────────────

/// A scratch directory owned by one test, inside Cargo's per-target test
/// directory. Every run gets an empty working directory and home.
struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    runs: AtomicUsize,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = format!(
            "p0-fg1-{name}-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(unique);
        let cwd = root.join("cwd");
        let home = root.join("home");
        for dir in [&cwd, &home, &root.join("out")] {
            fs::create_dir_all(dir).expect("create fixture directory");
        }
        Fixture {
            root,
            cwd,
            home,
            runs: AtomicUsize::new(0),
        }
    }

    /// Removes the fixture. Only called after the test's assertions passed,
    /// so a failing test leaves its fixture for inspection.
    fn remove(self) {
        fs::remove_dir_all(&self.root).expect("remove fixture directory");
    }
}

struct Outcome {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

/// Synthetic values placed in the child's environment. None is a real key.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("GITHUB_TOKEN", "p0fg1-sentinel-github-token"),
    ("NEXUS_GITHUB_TOKEN", "p0fg1-sentinel-nexus-github-token"),
    ("NEXUS_CONFIG_KEY", "p0fg1-sentinel-config-key"),
    ("NEXUS_ENCRYPTION_KEY", "p0fg1-sentinel-encryption-key"),
    ("OPENAI_API_KEY", "p0fg1-sentinel-openai-key"),
    ("RUST_LOG", "trace"),
    ("OLLAMA_HOST", "http://127.0.0.1:9/p0fg1-sentinel-ollama"),
];

/// Runs the withdrawn executable once with an empty environment apart from
/// the fixture's home and temp directories and the synthetic sentinels.
fn run(fixture: &Fixture, args: &[OsString]) -> Outcome {
    let n = fixture.runs.fetch_add(1, Ordering::SeqCst);
    let out_path = fixture.root.join("out").join(format!("{n}.stdout"));
    let err_path = fixture.root.join("out").join(format!("{n}.stderr"));
    let home = &fixture.home;

    let mut command = Command::new(withdrawn_binary());
    command
        .args(args)
        .current_dir(&fixture.cwd)
        .env_clear()
        .envs(ENV_SENTINELS.iter().copied())
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg-config"))
        .env("XDG_DATA_HOME", home.join("xdg-data"))
        .env("APPDATA", home.join("appdata"))
        .env("LOCALAPPDATA", home.join("localappdata"))
        .env("TMPDIR", home.join("tmp"))
        .env("TEMP", home.join("tmp"))
        .env("TMP", home.join("tmp"))
        .env("NEXUS_DATA_DIR", fixture.root.join("env-data-dir"))
        .stdin(Stdio::null())
        .stdout(fs::File::create(&out_path).expect("create stdout capture"))
        .stderr(fs::File::create(&err_path).expect("create stderr capture"));
    // Windows loads system libraries relative to SystemRoot; it is not a
    // credential.
    if cfg!(windows) {
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }

    let mut child = command.spawn().expect("start the withdrawn executable");
    let deadline = Instant::now() + RUN_DEADLINE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            outcome => {
                // This test owns the child: stop it before failing.
                let _ = child.kill();
                let _ = child.wait();
                panic!("nexus-server did not exit within {RUN_DEADLINE:?} ({outcome:?}); it was killed");
            }
        }
    };
    Outcome {
        status,
        stdout: fs::read_to_string(&out_path).expect("read stdout capture"),
        stderr: fs::read_to_string(&err_path).expect("read stderr capture"),
    }
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// The whole observable result of every invocation.
fn assert_withdrawn(outcome: &Outcome, context: &str) {
    assert_eq!(
        outcome.status.code(),
        Some(WITHDRAWN_STATUS),
        "{context}: exit status must be the fixed withdrawal status"
    );
    assert!(!outcome.status.success(), "{context}: must not succeed");
    assert_eq!(
        outcome.stderr,
        format!("{WITHDRAWN_MESSAGE}\n"),
        "{context}: stderr must be exactly the withdrawal message"
    );
    assert_eq!(outcome.stdout, "", "{context}: stdout must be empty");
    for word in ["ready", "listening", "tools", "starting", "result"] {
        assert!(
            !outcome.stderr.to_ascii_lowercase().contains(word),
            "{context}: no service output ({word})"
        );
    }
}

/// Every file and directory under `root`, with file contents.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("list fixture directory") {
            let path = entry.expect("fixture entry").path();
            let relative = path.strip_prefix(root).expect("inside root").to_path_buf();
            if path.is_dir() {
                entries.insert(relative, None);
                pending.push(path);
            } else {
                entries.insert(relative, Some(fs::read(&path).expect("read fixture file")));
            }
        }
    }
    entries
}

fn assert_empty_dir(dir: &Path, context: &str) {
    assert!(
        snapshot(dir).is_empty(),
        "{context}: {} must stay empty",
        dir.display()
    );
}

// ── Behaviour of the real executable ────────────────────────────────────────

/// Invoked with no arguments, the executable prints only the withdrawal
/// message, exits with the fixed non-zero status, and creates nothing: not
/// the old default `./nexus-data`, nothing under its home directory.
#[test]
fn p0_fg1_default_invocation_is_withdrawn() {
    let fixture = Fixture::new("default");
    let outcome = run(&fixture, &[]);
    assert_withdrawn(&outcome, "no arguments");
    assert_empty_dir(&fixture.cwd, "no arguments");
    assert_empty_dir(&fixture.home, "no arguments");
    assert!(!fixture.root.join("env-data-dir").exists());
    fixture.remove();
}

/// The retired server arguments, port 0, help and version flags, and
/// subcommand-like words all get the same denial. None starts a service.
#[test]
fn p0_fg1_old_server_arguments_cannot_reactivate_it() {
    let fixture = Fixture::new("arguments");
    let data_dir = fixture.root.join("old-data-dir");
    let data = data_dir.to_str().expect("UTF-8 fixture path");
    let invocations: Vec<Vec<&str>> = vec![
        vec![
            "--port",
            "3000",
            "--mcp-port",
            "3001",
            "--a2a-port",
            "3002",
            "--data-dir",
            data,
            "--log-level",
            "trace",
        ],
        vec!["--port", "0", "--mcp-port", "0", "--a2a-port", "0"],
        vec!["--port=3000", "--mcp-port=3001", "--a2a-port=3002"],
        vec!["--mcp-port", "3001"],
        vec!["--a2a-port", "3002"],
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["-V"],
        vec!["start"],
        vec!["serve"],
        vec!["mcp"],
        vec!["--"],
    ];
    for args in &invocations {
        let outcome = run(&fixture, &os_args(args));
        assert_withdrawn(&outcome, &format!("{args:?}"));
    }
    assert!(!data_dir.exists(), "--data-dir must not be created");
    assert_empty_dir(&fixture.cwd, "old arguments");
    assert_empty_dir(&fixture.home, "old arguments");
    fixture.remove();
}

/// A requested data directory is neither created nor modified, whether it is
/// missing, already exists with content, or is the old relative default.
#[test]
fn p0_fg1_requested_data_directories_are_not_created_or_modified() {
    let fixture = Fixture::new("data-dir");
    let missing = fixture.root.join("missing-data-dir");
    let existing = fixture.root.join("existing-data-dir");
    fs::create_dir_all(existing.join("nested")).expect("create existing data dir");
    fs::write(existing.join("audit.log"), b"p0fg1 sentinel content").expect("seed file");
    fs::write(existing.join("nested").join("genome.json"), b"{}").expect("seed file");
    let before = snapshot(&existing);

    for target in [&missing, &existing] {
        let args = vec![
            OsString::from("--data-dir"),
            target.clone().into_os_string(),
        ];
        assert_withdrawn(&run(&fixture, &args), "--data-dir");
        let mut joined = OsString::from("--data-dir=");
        joined.push(target.as_os_str());
        assert_withdrawn(&run(&fixture, &[joined]), "--data-dir=");
    }
    assert_withdrawn(
        &run(&fixture, &os_args(&["--data-dir", "./nexus-data"])),
        "relative --data-dir",
    );

    assert!(!missing.exists(), "a missing data directory stays missing");
    assert_eq!(
        snapshot(&existing),
        before,
        "an existing data directory is unchanged"
    );
    assert_empty_dir(&fixture.cwd, "relative data directory");
    assert_empty_dir(&fixture.home, "data directory");
    fixture.remove();
}

/// The denial never echoes an argument or environment value: the output is
/// the fixed message whatever the caller supplies.
#[test]
fn p0_fg1_denial_echoes_no_environment_or_argument_value() {
    let fixture = Fixture::new("echo");
    let argument_sentinels = [
        "p0fg1-sentinel-argument-value",
        "--p0fg1-sentinel-flag",
        "http://127.0.0.1:9/p0fg1-sentinel-url",
    ];
    let mut args = os_args(&["--log-level"]);
    args.extend(os_args(&argument_sentinels));
    let outcome = run(&fixture, &args);
    assert_withdrawn(&outcome, "sentinels");

    let output = format!("{}{}", outcome.stdout, outcome.stderr);
    for value in argument_sentinels
        .iter()
        .chain(ENV_SENTINELS.iter().map(|(_, value)| value))
    {
        assert!(!output.contains(value), "output must not echo {value}");
    }
    assert!(!output.contains("p0fg1"), "no sentinel fragment is echoed");
    assert_empty_dir(&fixture.cwd, "sentinels");
    assert_empty_dir(&fixture.home, "sentinels");
    fixture.remove();
}

// ── Structural guard for the production entry ───────────────────────────────

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("list {}: {e}", dir.display()))
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
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

/// The production entry of `crates/nexus-server` is exactly the withdrawal:
/// one message, one fixed status, and nothing else. It builds no router, MCP
/// server, runtime or listener, spawns nothing, and reads no argument,
/// environment variable or file.
#[test]
fn p0_fg1_entry_point_is_only_the_withdrawal() {
    let source = read(&manifest_dir().join("src").join("main.rs"));
    let code = strip_rust_comments(&source);
    let expected = "#![forbid(unsafe_code)] #[cfg(not(test))] \
        fn main() -> std::process::ExitCode { use std::io::Write; \
        const WITHDRAWN_MESSAGE: &str = \
        \"nexus-server: unavailable during Phase Zero; deployment withdrawn\"; \
        const WITHDRAWN_STATUS: u8 = 69; \
        let _ = writeln!(std::io::stderr(), \"{WITHDRAWN_MESSAGE}\"); \
        std::process::ExitCode::from(WITHDRAWN_STATUS) }";
    for forbidden in [
        "axum",
        "tokio",
        "tower",
        "Router",
        "route(",
        "serve(",
        "bind(",
        "TcpListener",
        "UdpSocket",
        "std::net",
        "nexus_mcp",
        "McpServer",
        "nexus_kernel",
        "spawn",
        "Command",
        "thread",
        "std::env",
        "env!",
        "args",
        "var(",
        "fs::",
        "File",
        "create_dir",
        "tracing",
        "clap",
        "Parser",
        "mod ",
        "include!",
        "include_str!",
        "#[path",
        "extern crate",
        "unsafe {",
        "ExitCode::SUCCESS",
    ] {
        assert!(
            !code.contains(forbidden),
            "the withdrawn entry point must not contain `{forbidden}`"
        );
    }
    assert_eq!(
        normalize_whitespace(&code),
        normalize_whitespace(expected),
        "src/main.rs must be exactly the withdrawal entry point (comments aside)"
    );
}

/// The package has one target, `src/main.rs`, and nothing that could add
/// another entry point or module: no library, build script, auto-discovered
/// `src/bin`, examples or benches.
#[test]
fn p0_fg1_package_has_no_other_entry_or_module() {
    let dir = manifest_dir();
    assert_eq!(entries(&dir), ["Cargo.toml", "src", "tests"]);
    assert_eq!(entries(&dir.join("src")), ["main.rs"]);
    assert_eq!(entries(&dir.join("tests")), ["phase0_withdrawal.rs"]);

    let manifest = read(&dir.join("Cargo.toml"));
    let lines: Vec<&str> = manifest.lines().map(str::trim).collect();
    let bin = lines
        .iter()
        .position(|line| *line == "[[bin]]")
        .expect("one [[bin]] target");
    assert_eq!(
        &lines[bin + 1..bin + 3],
        ["name = \"nexus-server\"", "path = \"src/main.rs\""]
    );
    assert_eq!(
        lines.iter().filter(|line| line.starts_with("[[")).count(),
        1
    );
    for forbidden in [
        "[lib]",
        "build =",
        "[[example]]",
        "[[bench]]",
        "[[test]]",
        "autobins",
    ] {
        assert!(
            !lines.iter().any(|line| line.starts_with(forbidden)),
            "Cargo.toml must not declare `{forbidden}`"
        );
    }
}

// ── Withdrawn deployment recipes under deploy/ ──────────────────────────────

fn deploy_dir() -> PathBuf {
    manifest_dir().join("..").join("..").join("deploy")
}

/// Lines that are neither blank nor `#` comments.
fn directive_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The J1 Dockerfile fails at its first step with the withdrawal message. It
/// has one stage, no parser directive that could swap the frontend, and no
/// package installation, source copy, build, entry point, port or health
/// check.
#[test]
fn p0_fg1_docker_recipe_fails_before_any_build_step() {
    let dockerfile = read(&deploy_dir().join("Dockerfile"));
    for line in dockerfile.lines() {
        let lower = line.trim().to_ascii_lowercase();
        let directive = lower.trim_start_matches('#').trim_start();
        assert!(
            !(lower.starts_with('#')
                && ["syntax", "escape", "check"]
                    .iter()
                    .any(|key| directive.starts_with(key) && directive.contains('='))),
            "no Dockerfile parser directive is allowed: {line}"
        );
    }
    assert_eq!(
        directive_lines(&dockerfile),
        [
            "FROM debian:bookworm-slim",
            "RUN echo \"nexus-server: deploy/Dockerfile is withdrawn during Phase Zero; \
             no image is built\" >&2; exit 1",
        ],
        "deploy/Dockerfile must fail at its first step and build nothing"
    );
    let upper = directive_lines(&dockerfile).join("\n").to_ascii_uppercase();
    for forbidden in [
        "COPY",
        "ADD ",
        "CARGO",
        "APT",
        "ENTRYPOINT",
        "CMD",
        "EXPOSE",
        "HEALTHCHECK",
        "VOLUME",
        "USER",
        "WORKDIR",
        "ONBUILD",
        " AS ",
    ] {
        assert!(
            !upper.contains(forbidden),
            "deploy/Dockerfile must not use {forbidden}"
        );
    }
}

/// Both J1 Compose recipes are non-operational stubs: no service, port,
/// build, image, volume, credential or restart policy, including the
/// companion Ollama service they used to publish.
#[test]
fn p0_fg1_compose_recipes_define_no_services() {
    for name in ["docker-compose.yml", "docker-compose.cpu.yml"] {
        let compose = read(&deploy_dir().join(name));
        for forbidden in [
            "ports:",
            "image:",
            "build:",
            "environment:",
            "env_file:",
            "restart:",
            "volumes:",
            "command:",
            "entrypoint:",
            "secrets:",
            "configs:",
            "extends:",
            "include:",
            "profiles:",
            "network_mode:",
            "ollama",
            "11434",
            "3000",
        ] {
            assert!(
                !directive_lines(&compose)
                    .iter()
                    .any(|line| line.to_ascii_lowercase().contains(forbidden)),
                "deploy/{name} must not define `{forbidden}`"
            );
        }
        assert_eq!(
            directive_lines(&compose),
            [
                "version: \"3.9\"",
                "x-nexus-withdrawn: \"nexus-server: this Compose recipe is withdrawn during \
                 Phase Zero; it defines no services\"",
                "services: {}",
            ],
            "deploy/{name} must be the withdrawal stub"
        );
    }
}

/// The J1 Helm chart renders nothing: its only template is an unconditional
/// `fail`, so no values override can produce a Deployment, Service, volume
/// claim or hook. Because rendering fails, an upgrade of an existing release
/// changes nothing (and deletes nothing).
#[test]
fn p0_fg1_helm_chart_fails_for_every_values_override() {
    let chart = deploy_dir().join("helm").join("nexus-os");
    assert_eq!(entries(&chart), ["Chart.yaml", "templates", "values.yaml"]);
    assert_eq!(entries(&chart.join("templates")), ["withdrawn.yaml"]);

    let template = read(&chart.join("templates").join("withdrawn.yaml"));
    assert_eq!(
        directive_lines(&template),
        ["{{- fail \"nexus-os chart: withdrawn during Phase Zero; it renders no resources\" -}}"],
        "the only template must fail unconditionally"
    );

    let values = read(&chart.join("values.yaml"));
    assert!(
        directive_lines(&values).is_empty(),
        "values.yaml must define no values"
    );

    let manifest = read(&chart.join("Chart.yaml"));
    let manifest_lines = directive_lines(&manifest);
    assert!(manifest_lines.contains(&"deprecated: true"));
    assert!(manifest_lines.contains(&"type: application"));
    for forbidden in ["dependencies:", "kubeVersion:"] {
        assert!(
            !manifest_lines
                .iter()
                .any(|line| line.starts_with(forbidden)),
            "Chart.yaml must not declare `{forbidden}`"
        );
    }
}

/// The deployment README says the recipes are withdrawn, says that existing
/// deployments are not stopped by this, and gives no command that installs,
/// builds, starts or reaches the server.
#[test]
fn p0_fg1_deploy_readme_withdraws_without_claiming_a_stop() {
    let readme = read(&deploy_dir().join("README.md"));
    assert!(readme.contains("These recipes are withdrawn"));
    assert!(readme.contains("An existing deployment is not stopped"));
    for forbidden in [
        "docker-compose up",
        "docker compose up",
        "docker build",
        "docker run",
        "helm install",
        "helm upgrade",
        "port-forward",
        "cargo build",
        "cargo run",
        "./target/",
        "curl ",
    ] {
        assert!(
            !readme.contains(forbidden),
            "deploy/README.md must not instruct `{forbidden}`"
        );
    }
}
