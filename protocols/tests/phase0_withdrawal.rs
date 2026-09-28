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
