//! P0-FINAL-GATE-CLOSURE, dossier items J2 and J3: the protocols server
//! binaries `nexus-protocols-server` and `nexus-os` are withdrawn.
//!
//! The behavioural tests run the executables built from this package, never
//! a program found on `PATH`. Before any run they check that the entry
//! source is exactly the withdrawal and that the executable carries its own
//! withdrawal message (and neither the sibling's nor the message of the
//! separately withdrawn `crates/nexus-server`, dossier item J1), so a build
//! that is not the withdrawal is never executed. `binary_target_identity.rs`
//! pins the two target names; this file does not repeat it.
//!
//! The source guards pin both entry points and the package's targets.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WITHDRAWN_STATUS: i32 = 69;
const RUN_DEADLINE: Duration = Duration::from_secs(60);
/// The withdrawal message of `crates/nexus-server` (J1). No protocols
/// executable may carry it, so J1's own identification can never select one.
const J1_MESSAGE: &str = "nexus-server: unavailable during Phase Zero; deployment withdrawn";

/// One withdrawn binary target of this package.
struct Withdrawn {
    name: &'static str,
    source: &'static str,
    message: &'static str,
    executable: &'static str,
}

const SERVER: Withdrawn = Withdrawn {
    name: "nexus-protocols-server",
    source: "nexus-server.rs",
    message: "nexus-protocols-server: unavailable during Phase Zero; deployment withdrawn",
    executable: env!("CARGO_BIN_EXE_nexus-protocols-server"),
};

const ALIAS: Withdrawn = Withdrawn {
    name: "nexus-os",
    source: "nexus-os.rs",
    message: "nexus-os: unavailable during Phase Zero; deployment withdrawn",
    executable: env!("CARGO_BIN_EXE_nexus-os"),
};

const BOTH: [&Withdrawn; 2] = [&SERVER, &ALIAS];

// ── Identification before any run ───────────────────────────────────────────

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Whether the file's bytes contain `needle`.
fn carries(path: &Path, needle: &str) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let needle = needle.as_bytes();
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

/// The executable Cargo built for `target`, after checking that its entry
/// source is exactly the withdrawal and that it carries only its own
/// withdrawal message. Panics (and runs nothing) otherwise.
fn identified(target: &Withdrawn) -> PathBuf {
    assert_entry_is_withdrawal(target);
    let path = PathBuf::from(target.executable);
    assert!(
        path.is_absolute(),
        "{}: Cargo must give an absolute path, never a PATH lookup",
        target.name
    );
    assert!(
        path.is_file(),
        "{}: {} was not built",
        target.name,
        path.display()
    );
    assert!(
        carries(&path, target.message),
        "{}: the executable must carry its withdrawal message",
        target.name
    );
    for other in BOTH.iter().filter(|other| other.name != target.name) {
        assert!(
            !carries(&path, other.message),
            "{}: the executable must not carry the {} message",
            target.name,
            other.name
        );
    }
    assert!(
        !carries(&path, J1_MESSAGE),
        "{}: the executable must never carry the J1 withdrawal message",
        target.name
    );
    path
}

// ── Isolated runs ───────────────────────────────────────────────────────────

/// A scratch directory owned by one test, inside Cargo's per-target test
/// directory. Every run gets the same fixture working directory and home.
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
            "p0-j2-j3-{name}-{}-{}-{}",
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
        for dir in [&cwd, &home, &root.join("out"), &root.join("frontend")] {
            fs::create_dir_all(dir).expect("create fixture directory");
        }
        // The server's frontend fallback served the working directory's
        // `app/dist` and the directory named by NEXUS_FRONTEND_DIST.
        fs::create_dir_all(cwd.join("app").join("dist")).expect("create app/dist");
        fs::write(
            cwd.join("app").join("dist").join("index.html"),
            b"p0j2 sentinel working-directory frontend",
        )
        .expect("seed app/dist");
        fs::write(
            root.join("frontend").join("index.html"),
            b"p0j2 sentinel frontend dist",
        )
        .expect("seed frontend");
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

/// Synthetic values placed in the child's environment. None is a real key or
/// a routable destination: every address is loopback, and the listen address
/// is an ephemeral loopback port, so even a regression could not listen on
/// another interface.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("NEXUS_HTTP_ADDR", "127.0.0.1:0"),
    ("NEXUS_MODE", "hybrid"),
    ("NEXUS_CORS_ORIGINS", "*"),
    ("NEXUS_SHUTDOWN_TIMEOUT_SECS", "1"),
    ("JWT_SECRET", "p0j2-sentinel-jwt-secret"),
    ("HOSTNAME", "p0j2-sentinel-hostname"),
    ("DATABASE_URL", "postgres://p0j2-sentinel@127.0.0.1:9/p0j2"),
    ("OLLAMA_URL", "http://127.0.0.1:9/p0j2-sentinel-ollama"),
    ("GITHUB_TOKEN", "p0j2-sentinel-github-token"),
    ("NEXUS_CONFIG_KEY", "p0j2-sentinel-config-key"),
    ("NEXUS_ENCRYPTION_KEY", "p0j2-sentinel-encryption-key"),
    ("OPENAI_API_KEY", "p0j2-sentinel-openai-key"),
    ("ANTHROPIC_API_KEY", "p0j2-sentinel-anthropic-key"),
    ("RUST_LOG", "trace"),
];

/// Runs one withdrawn executable once, with an empty environment apart from
/// the fixture's home, temp and configuration directories and the synthetic
/// sentinels.
fn run(fixture: &Fixture, target: &Withdrawn, args: &[OsString]) -> Outcome {
    let executable = identified(target);
    let n = fixture.runs.fetch_add(1, Ordering::SeqCst);
    let out_path = fixture.root.join("out").join(format!("{n}.stdout"));
    let err_path = fixture.root.join("out").join(format!("{n}.stderr"));
    let home = &fixture.home;

    let mut command = Command::new(&executable);
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
        .env("NEXUS_FRONTEND_DIST", fixture.root.join("frontend"))
        .env("NEXUS_CONFIG_PATH", fixture.root.join("config-path"))
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
                panic!(
                    "{} did not exit within {RUN_DEADLINE:?} ({outcome:?}); it was killed",
                    target.name
                );
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
fn assert_withdrawn(outcome: &Outcome, target: &Withdrawn, context: &str) {
    let context = format!("{} {context}", target.name);
    assert_eq!(
        outcome.status.code(),
        Some(WITHDRAWN_STATUS),
        "{context}: exit status must be the fixed withdrawal status"
    );
    assert!(!outcome.status.success(), "{context}: must not succeed");
    assert_eq!(
        outcome.stderr,
        format!("{}\n", target.message),
        "{context}: stderr must be exactly the withdrawal message"
    );
    assert_eq!(outcome.stdout, "", "{context}: stdout must be empty");
    for word in ["listening", "gateway", "ready", "started", "instance"] {
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

/// The fixture's working directory, home and frontend directory as they were
/// before any run, plus the locations the server used to create.
fn assert_fixture_untouched(
    fixture: &Fixture,
    cwd_before: &BTreeMap<PathBuf, Option<Vec<u8>>>,
    frontend_before: &BTreeMap<PathBuf, Option<Vec<u8>>>,
    context: &str,
) {
    assert_eq!(
        &snapshot(&fixture.cwd),
        cwd_before,
        "{context}: the working directory is unchanged"
    );
    assert_eq!(
        &snapshot(&fixture.root.join("frontend")),
        frontend_before,
        "{context}: the frontend directory is unchanged"
    );
    assert_empty_dir(&fixture.home, context);
    assert!(
        !fixture.root.join("config-path").exists(),
        "{context}: NEXUS_CONFIG_PATH must not be created"
    );
}

// ── Behaviour of the real executables ───────────────────────────────────────

/// Invoked with no arguments (formerly: start the gateway), each executable
/// prints only its withdrawal message, exits with the fixed status, and
/// changes nothing.
#[test]
fn p0_j2_j3_default_invocation_is_withdrawn() {
    let fixture = Fixture::new("default");
    let cwd_before = snapshot(&fixture.cwd);
    let frontend_before = snapshot(&fixture.root.join("frontend"));
    for target in BOTH {
        let outcome = run(&fixture, target, &[]);
        assert_withdrawn(&outcome, target, "no arguments");
    }
    assert_fixture_untouched(&fixture, &cwd_before, &frontend_before, "no arguments");
    fixture.remove();
}

/// The retired `start` command, help, version and subcommand-like words all
/// get the same denial from both executables. None starts a service.
#[test]
fn p0_j2_j3_server_commands_cannot_reactivate_it() {
    let fixture = Fixture::new("commands");
    let cwd_before = snapshot(&fixture.cwd);
    let frontend_before = snapshot(&fixture.root.join("frontend"));
    let invocations: Vec<Vec<&str>> = vec![
        vec!["start"],
        vec!["start", "--port", "0"],
        vec!["start", "--help"],
        vec!["help"],
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["-V"],
        vec!["serve"],
        vec!["server"],
        vec!["mcp"],
        vec!["a2a"],
        vec!["desktop"],
        vec!["--"],
        vec![""],
    ];
    for target in BOTH {
        for args in &invocations {
            let outcome = run(&fixture, target, &os_args(args));
            assert_withdrawn(&outcome, target, &format!("{args:?}"));
        }
    }
    assert_fixture_untouched(&fixture, &cwd_before, &frontend_before, "commands");
    fixture.remove();
}

/// The denial never echoes an argument or environment value: the output is
/// the fixed message whatever the caller supplies.
#[test]
fn p0_j2_j3_denial_echoes_no_environment_or_argument_value() {
    let fixture = Fixture::new("echo");
    let cwd_before = snapshot(&fixture.cwd);
    let frontend_before = snapshot(&fixture.root.join("frontend"));
    let argument_sentinels = [
        "p0j2-sentinel-argument-value",
        "--p0j2-sentinel-flag",
        "http://127.0.0.1:9/p0j2-sentinel-url",
    ];
    for target in BOTH {
        let mut args = os_args(&["start"]);
        args.extend(os_args(&argument_sentinels));
        let outcome = run(&fixture, target, &args);
        assert_withdrawn(&outcome, target, "sentinels");

        let output = format!("{}{}", outcome.stdout, outcome.stderr);
        for value in argument_sentinels
            .iter()
            .chain(ENV_SENTINELS.iter().map(|(_, value)| value))
        {
            assert!(!output.contains(value), "output must not echo {value}");
        }
        assert!(!output.contains("p0j2"), "no sentinel fragment is echoed");
        assert!(!output.contains("127.0.0.1"), "no address is echoed");
    }
    assert_fixture_untouched(&fixture, &cwd_before, &frontend_before, "sentinels");
    fixture.remove();
}

// ── Structural guards for the production entries ────────────────────────────

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

/// Code tokens a withdrawn entry point must not contain.
const FORBIDDEN_IN_ENTRY: &[&str] = &[
    "nexus_protocols",
    "server_runtime",
    "run_from_args",
    "http_gateway",
    "build_router",
    "GatewayState",
    "serve_frontend",
    "axum",
    "tokio",
    "tower",
    "Router",
    "serve(",
    "bind(",
    "TcpListener",
    "UdpSocket",
    "std::net",
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
    "current_dir",
    "create_dir",
    "tracing",
    "mod ",
    "include!",
    "include_str!",
    "#[path",
    "extern crate",
    "unsafe {",
    "ExitCode::SUCCESS",
];

/// Panics unless the entry source of `target` is exactly the withdrawal:
/// one message, one fixed status, and nothing else.
fn assert_entry_is_withdrawal(target: &Withdrawn) {
    let source = read(&manifest_dir().join("src").join("bin").join(target.source));
    let code = strip_rust_comments(&source);
    let expected = format!(
        "#![forbid(unsafe_code)] #[cfg(not(test))] \
         fn main() -> std::process::ExitCode {{ use std::io::Write; \
         const WITHDRAWN_MESSAGE: &str = \"{}\"; \
         const WITHDRAWN_STATUS: u8 = 69; \
         let _ = writeln!(std::io::stderr(), \"{{WITHDRAWN_MESSAGE}}\"); \
         std::process::ExitCode::from(WITHDRAWN_STATUS) }}",
        target.message
    );
    for forbidden in FORBIDDEN_IN_ENTRY {
        assert!(
            !code.contains(forbidden),
            "{}: the withdrawn entry point must not contain `{forbidden}`",
            target.name
        );
    }
    assert_eq!(
        normalize_whitespace(&code),
        normalize_whitespace(&expected),
        "src/bin/{} must be exactly the withdrawal entry point (comments aside)",
        target.source
    );
}

/// Both production entries are exactly the withdrawal. They build no router,
/// runtime or listener, spawn nothing, and read no argument, environment
/// variable or file.
#[test]
fn p0_j2_j3_entry_points_are_only_the_withdrawal() {
    for target in BOTH {
        assert_entry_is_withdrawal(target);
    }
    assert_ne!(SERVER.message, ALIAS.message);
    for target in BOTH {
        assert!(
            !target.message.contains(J1_MESSAGE) && !J1_MESSAGE.contains(target.message),
            "{}: the message must stay distinct from J1's",
            target.name
        );
    }
}

/// `[[bin]]` blocks of a manifest as (name, path), in order.
fn bin_targets(manifest: &str) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            if let Some(target) = current.take() {
                targets.push(target);
            }
            if line == "[[bin]]" {
                current = Some((String::new(), String::new()));
            }
            continue;
        }
        if let Some((name, path)) = current.as_mut() {
            if let Some(value) = line.strip_prefix("name = ") {
                *name = value.trim_matches('"').to_string();
            } else if let Some(value) = line.strip_prefix("path = ") {
                *path = value.trim_matches('"').to_string();
            }
        }
    }
    if let Some(target) = current.take() {
        targets.push(target);
    }
    targets
}

/// The package builds exactly the two withdrawn binaries from their two
/// sources, and nothing that could add another entry point: no `src/main.rs`,
/// no other `src/bin` file, no examples or benches, no `default-run` and no
/// features. (Dependency lines are not examined.)
#[test]
fn p0_j2_j3_package_has_no_other_entry_point() {
    let dir = manifest_dir();
    assert_eq!(
        entries(&dir.join("src").join("bin")),
        ["nexus-os.rs", "nexus-server.rs"]
    );
    assert!(!dir.join("src").join("main.rs").exists());
    for extra in ["examples", "benches"] {
        assert!(
            !dir.join(extra).exists(),
            "protocols/{extra} must not exist"
        );
    }

    let manifest = read(&dir.join("Cargo.toml"));
    assert_eq!(
        bin_targets(&manifest),
        [
            (
                "nexus-protocols-server".to_string(),
                "src/bin/nexus-server.rs".to_string()
            ),
            ("nexus-os".to_string(), "src/bin/nexus-os.rs".to_string()),
        ]
    );
    let lines: Vec<&str> = manifest.lines().map(str::trim).collect();
    for forbidden in [
        "default-run",
        "autobins",
        "autoexamples",
        "[[example]]",
        "[[bench]]",
        "[features]",
    ] {
        assert!(
            !lines.iter().any(|line| line.starts_with(forbidden)),
            "protocols/Cargo.toml must not declare `{forbidden}`"
        );
    }
}

// ── Withdrawn deployment recipes at the repository root ─────────────────────

fn workspace_root() -> PathBuf {
    manifest_dir().join("..")
}

/// Lines that are neither blank nor `#` comments (a shebang counts as a
/// comment).
fn directive_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The root Dockerfile (the protocols server image) fails at its first step
/// with the withdrawal message. It has one stage, no parser directive that
/// could swap the frontend, and no package installation, source copy, build,
/// entry point, port, health check, volume or environment.
#[test]
fn p0_j2_j3_docker_recipe_fails_before_any_build_step() {
    let dockerfile = read(&workspace_root().join("Dockerfile"));
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
            "RUN echo \"nexus-protocols-server: Dockerfile is withdrawn during Phase Zero; \
             no image is built\" >&2; exit 1",
        ],
        "Dockerfile must fail at its first step and build nothing"
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
        "ENV ",
        "ARG ",
        "ONBUILD",
        " AS ",
    ] {
        assert!(
            !upper.contains(forbidden),
            "Dockerfile must not use {forbidden}"
        );
    }
}

/// The root Compose recipe is a non-operational stub: no service, port,
/// build, image, volume, credential or restart policy, including the Ollama,
/// PostgreSQL and high-availability services it used to define.
#[test]
fn p0_j2_j3_compose_recipe_defines_no_services() {
    let compose = read(&workspace_root().join("docker-compose.yml"));
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
        "healthcheck:",
        "depends_on:",
        "deploy:",
        "ollama",
        "postgres",
        "8080",
        "9090",
        "11434",
        "5432",
    ] {
        assert!(
            !directive_lines(&compose)
                .iter()
                .any(|line| line.to_ascii_lowercase().contains(forbidden)),
            "docker-compose.yml must not define `{forbidden}`"
        );
    }
    assert_eq!(
        directive_lines(&compose),
        [
            "x-nexus-withdrawn: \"nexus-protocols-server: this Compose recipe is withdrawn \
             during Phase Zero; it defines no services\"",
            "services: {}",
        ],
        "docker-compose.yml must be the withdrawal stub"
    );
}

/// The root Helm chart renders nothing: its only template is an
/// unconditional `fail`, so no values override can produce a Deployment,
/// Service, volume claim, CronJob or hook. Because rendering fails, an
/// upgrade of an existing release changes nothing (and deletes nothing).
#[test]
fn p0_j2_j3_helm_chart_fails_for_every_values_override() {
    let chart = workspace_root().join("helm").join("nexus-os");
    assert_eq!(
        entries(&workspace_root().join("helm")),
        ["nexus-os"],
        "helm/ holds only the withdrawn chart"
    );
    assert_eq!(entries(&chart), ["Chart.yaml", "templates", "values.yaml"]);
    assert_eq!(entries(&chart.join("templates")), ["withdrawn.yaml"]);

    let template = read(&chart.join("templates").join("withdrawn.yaml"));
    assert_eq!(
        directive_lines(&template),
        [
            "{{- fail \"helm/nexus-os chart: withdrawn during Phase Zero; it renders no \
             resources\" -}}"
        ],
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
    assert!(manifest_lines
        .iter()
        .any(|line| line.starts_with("description: WITHDRAWN during Phase Zero")));
    for forbidden in ["dependencies:", "kubeVersion:"] {
        assert!(
            !manifest_lines
                .iter()
                .any(|line| line.starts_with(forbidden)),
            "Chart.yaml must not declare `{forbidden}`"
        );
    }
}

/// The `nexus-os` Makefile target builds, copies and runs nothing: it has no
/// prerequisite and its one recipe line fails with the withdrawal message.
/// No other target builds a protocols binary.
#[test]
fn p0_j3_make_target_builds_nothing() {
    let makefile = read(&workspace_root().join("Makefile"));
    let lines: Vec<&str> = makefile.lines().collect();
    let rule = lines
        .iter()
        .position(|line| line.starts_with("nexus-os:"))
        .expect("the nexus-os rule stays, withdrawn");
    assert_eq!(lines[rule], "nexus-os:", "the rule has no prerequisite");
    let recipe: Vec<&str> = lines[rule + 1..]
        .iter()
        .take_while(|line| line.starts_with('\t'))
        .copied()
        .collect();
    assert_eq!(
        recipe,
        ["\t@echo \"make nexus-os: withdrawn during Phase Zero; nothing is built\" >&2; exit 1"],
        "the nexus-os recipe must only fail"
    );
    for forbidden in [
        "cargo",
        "nexus-protocols",
        "--bin",
        "target/release",
        "server_runtime",
    ] {
        assert!(
            !makefile.contains(forbidden),
            "Makefile must not contain `{forbidden}`"
        );
    }
}

/// The root installer downloads and installs nothing: after its comments it
/// only prints the withdrawal and fails. It reads no environment variable
/// (so no repository or release-API override), uses no network tool, no
/// sudo and no install, archive or disk-image tool.
#[test]
fn p0_j3_install_script_downloads_and_installs_nothing() {
    let script = read(&workspace_root().join("install.sh"));
    // `lines()` drops a `\r`, so a CRLF checkout (Windows) reads the same.
    assert_eq!(script.lines().next(), Some("#!/usr/bin/env bash"));
    assert_eq!(
        directive_lines(&script),
        [
            "echo \"install.sh: withdrawn during Phase Zero; nothing is downloaded or \
             installed\" >&2",
            "exit 1",
        ],
        "install.sh must only print the withdrawal and fail"
    );
    let code = directive_lines(&script).join("\n");
    for forbidden in [
        "$", "curl", "wget", "sudo", "install ", "tar ", "dpkg", "hdiutil", "mktemp", "chmod",
        "mv ", "cp ",
    ] {
        assert!(
            !code.contains(forbidden),
            "install.sh must not use `{forbidden}`"
        );
    }
}

// ── Withdrawn deployment documentation ──────────────────────────────────────

fn assert_doc(path: &str, required: &[&str], forbidden: &[&str]) {
    // Line breaks and blockquote markers in the Markdown source do not
    // matter.
    let text = read(&workspace_root().join(path))
        .lines()
        .map(|line| line.trim_start().trim_start_matches('>'))
        .collect::<Vec<_>>()
        .join("\n");
    let text = normalize_whitespace(&text);
    for phrase in required {
        assert!(text.contains(phrase), "{path} must say `{phrase}`");
    }
    for phrase in forbidden {
        assert!(
            !text.contains(phrase),
            "{path} must not instruct the withdrawn server (`{phrase}`)"
        );
    }
}

/// The deployment guide, the README, the J1 deployment notes and the
/// SWE-bench harness say that the protocols server is withdrawn, say that
/// existing deployments are not stopped, and give no command that builds,
/// installs, deploys, starts or reaches it. The README also says that the
/// withdrawn binaries only deny, that developer and benchmark binaries remain
/// pending the Architect's decision (D3), and that the deployment guide and
/// the Docker/Helm roadmap items are withdrawn. (J1's own guard checks its
/// part of the deployment guide; the desktop's standalone guard checks that
/// the README names every withdrawn binary.)
#[test]
fn p0_j2_j3_deployment_docs_withdraw_without_claiming_a_stop() {
    assert_doc(
        "docs/DEPLOYMENT.md",
        &[
            "### Protocols server (`nexus-protocols-server`, `nexus-os`) (withdrawn)",
            "are withdrawn during Phase Zero",
            "The repository provides no supported deployment of the protocols server at this \
             point",
            "### Other standalone binaries (withdrawn)",
            "is not stopped automatically by this source change, and nothing is deleted",
        ],
        &[
            "docker compose up",
            "docker-compose up",
            "docker compose --profile",
            "docker pull",
            "docker save",
            "docker load",
            "docker run",
            "docker build",
            "helm install ",
            "helm upgrade ",
            "helm package",
            "port-forward",
            "kubectl",
            "cargo build",
            "cargo run",
            "./target/",
            "--bin ",
            "nexus-os start",
            "nexus-protocols-server start",
            "curl ",
            "JWT_SECRET",
            "NEXUS_HTTP_ADDR",
            "existingSecret",
            "/mcp/tools/invoke",
        ],
    );
    assert_doc(
        "README.md",
        &[
            "### Server Deployment (withdrawn)",
            "are withdrawn during Phase Zero",
            "Each now prints a fixed withdrawal message and exits with status 69.",
            "Developer and benchmark binaries remain in the repository, pending the \
             Architect's decision on them (D3); no recipe ships them.",
            "The repository provides no supported server deployment at this point",
            "An existing deployment is not stopped automatically",
            "| [Deployment Guide](docs/DEPLOYMENT.md) | Server deployment (Docker, \
             Kubernetes/Helm, air-gapped): withdrawn during Phase Zero |",
            "Docker/Helm deployment (withdrawn during Phase Zero)",
            "Docker + Helm chart for server/K8s deployment (withdrawn during Phase Zero)",
        ],
        &[
            "docker compose up",
            "docker-compose up",
            "docker compose --profile",
            "curl http://localhost:8080",
            "helm install",
            "for Kubernetes/Helm, air-gapped, and HA deployment",
            "and the standalone command-line binaries are withdrawn",
            "| Docker, Kubernetes/Helm, air-gapped installation |",
        ],
    );
    assert_doc(
        "deploy/README.md",
        &["It is withdrawn separately, in the same way; see `docs/DEPLOYMENT.md`."],
        &["is not changed here"],
    );
    assert_doc(
        "eval/swebench/README.md",
        &[
            "**Withdrawn during Phase Zero.**",
            "the repository provides no Nexus OS endpoint for this harness at this point",
        ],
        &[
            "cargo run -p nexus-protocols",
            "--port 3000",
            "OR: launch the desktop app",
        ],
    );
}
