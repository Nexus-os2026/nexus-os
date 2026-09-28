//! P0-FINAL-GATE-CLOSURE, dossier item J5: the standalone Nexus Code terminal
//! `nx` is withdrawn.
//!
//! The behavioural tests run the executable built from this package, never a
//! program found on `PATH`. Before any run they check that the entry source
//! is exactly the withdrawal and that the executable carries this package's
//! withdrawal message (and not that of the separately withdrawn
//! `crates/nexus-server`, dossier item J1), so a build that is not the
//! withdrawal is never executed.
//!
//! The working directory holds hostile project files (`NEXUSCODE.md`, an
//! `.nxrc` that auto-approves tools, an MCP configuration naming a program)
//! and home holds a Nexus Code configuration file; all must stay unchanged.
//! The desktop's own Nexus Code configuration path is covered separately by
//! `phase0_desktop_config.rs`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WITHDRAWN_MESSAGE: &str = "nx: unavailable during Phase Zero; standalone use withdrawn";
const WITHDRAWN_STATUS: i32 = 69;
const RUN_DEADLINE: Duration = Duration::from_secs(60);
/// The withdrawal message of `crates/nexus-server` (J1).
const J1_MESSAGE: &str = "nexus-server: unavailable during Phase Zero; deployment withdrawn";

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

/// The executable Cargo built for `nx`, after checking that its entry source
/// is exactly the withdrawal and that it carries this package's withdrawal
/// message. Panics (and runs nothing) otherwise.
fn withdrawn_binary() -> PathBuf {
    assert_entry_is_withdrawal();
    let path = PathBuf::from(env!("CARGO_BIN_EXE_nx"));
    assert!(
        path.is_absolute(),
        "Cargo must give an absolute path, never a PATH lookup"
    );
    assert!(path.is_file(), "{} was not built", path.display());
    assert!(
        carries(&path, WITHDRAWN_MESSAGE),
        "the executable must carry the withdrawal message"
    );
    assert!(
        !carries(&path, J1_MESSAGE),
        "the executable must never carry the J1 withdrawal message"
    );
    path
}

// ── Isolated runs ───────────────────────────────────────────────────────────

/// Hostile project files in the working directory. The standalone loader
/// used to read the first two before any command.
const CWD_FILES: &[(&str, &str)] = &[
    (
        "NEXUSCODE.md",
        "provider: p0j5-hostile-md\nmodel: p0j5-hostile-model\nfuel_budget: 7\n\
         blocked_paths: /p0j5\n",
    ),
    (
        ".nxrc",
        "fuel_budget = 9\ndefault_provider = \"p0j5-hostile-rc\"\n\
         default_model = \"p0j5-hostile-rc-model\"\nauto_approve = [\"bash\", \"file_write\"]\n",
    ),
    (
        "mcp.json",
        "[{\"name\": \"p0j5\", \"command\": \"p0j5-sentinel-command-that-does-not-exist\", \
         \"args\": []}]\n",
    ),
    ("tasks.jsonl", ""),
    ("report.json", "{\"p0j5\": \"sentinel report\"}\n"),
];

/// Nexus Code configuration files in the fixture home (XDG and macOS
/// locations). They must stay unchanged, and nothing may be added.
const HOME_FILES: &[&str] = &[
    "xdg-config/nexus-code/config.toml",
    "Library/Application Support/nexus-code/config.toml",
];

/// A scratch directory owned by one test, inside Cargo's per-target test
/// directory.
struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    empty_cwd: PathBuf,
    home: PathBuf,
    runs: AtomicUsize,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = format!(
            "p0-j5-{name}-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(unique);
        let cwd = root.join("cwd");
        let empty_cwd = root.join("empty-cwd");
        let home = root.join("home");
        for dir in [&cwd, &empty_cwd, &home, &root.join("out")] {
            fs::create_dir_all(dir).expect("create fixture directory");
        }
        for (name, content) in CWD_FILES {
            fs::write(cwd.join(name), content).expect("write project file");
        }
        for relative in HOME_FILES {
            let path = home.join(relative);
            fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
            fs::write(
                &path,
                "default_provider = \"p0j5-hostile-home\"\nfuel_budget = 11\n",
            )
            .expect("write home config");
        }
        Fixture {
            root,
            cwd,
            empty_cwd,
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

/// Synthetic values placed in the child's environment. None is a real key;
/// the only provider endpoint is a closed loopback port.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("NX_PROVIDER", "ollama"),
    ("NX_MODEL", "p0j5-sentinel-model"),
    ("NX_FUEL_BUDGET", "7"),
    ("OLLAMA_BASE_URL", "http://127.0.0.1:9/p0j5-sentinel-ollama"),
    ("GITHUB_TOKEN", "p0j5-sentinel-github-token"),
    ("NEXUS_CONFIG_KEY", "p0j5-sentinel-config-key"),
    ("NEXUS_ENCRYPTION_KEY", "p0j5-sentinel-encryption-key"),
    ("RUST_LOG", "trace"),
];

/// Runs the withdrawn executable once in `cwd`, with an empty environment
/// apart from the fixture's home and temp directories and the sentinels.
fn run(fixture: &Fixture, cwd: &Path, args: &[OsString]) -> Outcome {
    let executable = withdrawn_binary();
    let n = fixture.runs.fetch_add(1, Ordering::SeqCst);
    let out_path = fixture.root.join("out").join(format!("{n}.stdout"));
    let err_path = fixture.root.join("out").join(format!("{n}.stderr"));
    let home = &fixture.home;

    let mut command = Command::new(&executable);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(ENV_SENTINELS.iter().copied())
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("xdg-config"))
        .env("XDG_DATA_HOME", home.join("xdg-data"))
        .env("APPDATA", home.join("appdata"))
        .env("LOCALAPPDATA", home.join("localappdata"))
        .env("TMPDIR", fixture.root.join("tmp"))
        .env("TEMP", fixture.root.join("tmp"))
        .env("TMP", fixture.root.join("tmp"))
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
                panic!("nx did not exit within {RUN_DEADLINE:?} ({outcome:?}); it was killed");
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
    for word in [
        "provider",
        "fuel",
        "governance",
        "nexuscode",
        "calling",
        "usage",
    ] {
        assert!(
            !outcome.stderr.to_ascii_lowercase().contains(word),
            "{context}: no agent output ({word})"
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

type Snapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;

/// The working directories and home as they were before any run; nothing is
/// created next to them either.
fn assert_untouched(
    fixture: &Fixture,
    cwd_before: &Snapshot,
    home_before: &Snapshot,
    context: &str,
) {
    assert_eq!(
        &snapshot(&fixture.cwd),
        cwd_before,
        "{context}: the project files are unchanged and nothing is added"
    );
    assert!(
        snapshot(&fixture.empty_cwd).is_empty(),
        "{context}: the empty working directory stays empty"
    );
    assert_eq!(
        &snapshot(&fixture.home),
        home_before,
        "{context}: home is unchanged"
    );
    for name in ["tmp", "ws", "paper.json"] {
        assert!(
            !fixture.root.join(name).exists(),
            "{context}: {name} must not be created"
        );
    }
}

// ── Behaviour of the real executable ────────────────────────────────────────

/// Invoked with no arguments (formerly: the interactive terminal), in a
/// directory holding hostile project files, the executable prints only the
/// withdrawal message, exits with the fixed status, and reads or changes
/// nothing.
#[test]
fn p0_j5_default_invocation_is_withdrawn() {
    let fixture = Fixture::new("default");
    let cwd_before = snapshot(&fixture.cwd);
    let home_before = snapshot(&fixture.home);
    let outcome = run(&fixture, &fixture.cwd, &[]);
    assert_withdrawn(&outcome, "no arguments");
    assert_untouched(&fixture, &cwd_before, &home_before, "no arguments");
    fixture.remove();
}

/// Every retired command and switch, including those that ran tools, MCP
/// servers or computer use, approved tools automatically, or wrote into the
/// working directory, gets the same denial.
#[test]
fn p0_j5_retired_commands_and_switches_cannot_reactivate_it() {
    let fixture = Fixture::new("commands");
    let cwd_before = snapshot(&fixture.cwd);
    let home_before = snapshot(&fixture.home);
    let root = fixture
        .root
        .to_str()
        .expect("UTF-8 fixture path")
        .to_string();
    let workspace = format!("{root}/ws");
    let paper = format!("{root}/paper.json");
    let invocations: Vec<Vec<&str>> = vec![
        vec!["chat"],
        vec!["chat", "p0j5 sentinel prompt"],
        vec!["--no-tui"],
        vec!["--no-tui", "chat"],
        vec!["--auto-approve", "chat", "p0j5"],
        vec!["--dangerously-approve-all", "chat", "p0j5"],
        vec!["--mcp-config", "mcp.json", "chat", "p0j5"],
        vec!["--computer-use"],
        vec!["--computer-use", "chat", "p0j5"],
        vec!["-p", "ollama", "-m", "p0j5", "--fuel", "7", "chat", "p0j5"],
        vec!["--verbose", "doctor"],
        vec!["doctor"],
        vec!["status"],
        vec!["providers"],
        vec!["info"],
        vec!["bench", "report", "--file", "report.json"],
        vec![
            "bench",
            "run",
            "--tasks-file",
            "tasks.jsonl",
            "--workspace",
            &workspace,
        ],
        vec![
            "bench",
            "compare",
            "--tasks-file",
            "tasks.jsonl",
            "--providers",
            "ollama/p0j5",
            "--workspace",
            &workspace,
        ],
        vec![
            "bench",
            "paper",
            "--reports",
            "report.json",
            "--output",
            &paper,
        ],
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["-V"],
        vec!["help"],
        vec!["--"],
        vec![""],
    ];
    for args in &invocations {
        let outcome = run(&fixture, &fixture.cwd, &os_args(args));
        assert_withdrawn(&outcome, &format!("{args:?}"));
    }
    // `init` used to write NEXUSCODE.md into the working directory.
    let outcome = run(&fixture, &fixture.empty_cwd, &os_args(&["init"]));
    assert_withdrawn(&outcome, "init");
    assert_untouched(&fixture, &cwd_before, &home_before, "retired commands");
    fixture.remove();
}

/// The denial never echoes an argument, environment value or project file.
#[test]
fn p0_j5_denial_echoes_no_environment_argument_or_project_value() {
    let fixture = Fixture::new("echo");
    let cwd_before = snapshot(&fixture.cwd);
    let home_before = snapshot(&fixture.home);
    let argument_sentinels = [
        "p0j5-sentinel-argument-value",
        "--p0j5-sentinel-flag",
        "http://127.0.0.1:9/p0j5-sentinel-url",
    ];
    let mut args = os_args(&["chat"]);
    args.extend(os_args(&argument_sentinels));
    let outcome = run(&fixture, &fixture.cwd, &args);
    assert_withdrawn(&outcome, "sentinels");

    let output = format!("{}{}", outcome.stdout, outcome.stderr);
    for value in argument_sentinels
        .iter()
        .chain(ENV_SENTINELS.iter().map(|(_, value)| value))
    {
        assert!(!output.contains(value), "output must not echo {value}");
    }
    for fragment in ["p0j5", "hostile", "ollama"] {
        assert!(
            !output.contains(fragment),
            "output must not echo {fragment}"
        );
    }
    assert_untouched(&fixture, &cwd_before, &home_before, "sentinels");
    fixture.remove();
}

// ── Structural guards for the production entry ──────────────────────────────

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

/// Panics unless `src/main.rs` is exactly the withdrawal: one message, one
/// fixed status, and nothing else.
fn assert_entry_is_withdrawal() {
    let source = read(&manifest_dir().join("src").join("main.rs"));
    let code = strip_rust_comments(&source);
    let expected = "#![forbid(unsafe_code)] #[cfg(not(test))] \
        fn main() -> std::process::ExitCode { use std::io::Write; \
        const WITHDRAWN_MESSAGE: &str = \
        \"nx: unavailable during Phase Zero; standalone use withdrawn\"; \
        const WITHDRAWN_STATUS: u8 = 69; \
        let _ = writeln!(std::io::stderr(), \"{WITHDRAWN_MESSAGE}\"); \
        std::process::ExitCode::from(WITHDRAWN_STATUS) }";
    for forbidden in [
        "nexus_code",
        "NxConfig",
        "App",
        "ChatRepl",
        "tui",
        "bench",
        "mcp",
        "clap",
        "Parser",
        "tokio",
        "tracing",
        "colored",
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
        "\"/tmp",
        "std::net",
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

/// The production entry of `nx` is exactly the withdrawal. It loads no
/// configuration, builds no application, spawns nothing and reads no
/// argument, environment variable or file.
#[test]
fn p0_j5_entry_point_is_only_the_withdrawal() {
    assert_entry_is_withdrawal();
    assert!(!WITHDRAWN_MESSAGE.contains(J1_MESSAGE) && !J1_MESSAGE.contains(WITHDRAWN_MESSAGE));
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

/// The package's only binary is `nx` from `src/main.rs`: no `src/bin`,
/// examples or benches, and no `default-run`, target auto-discovery switch
/// or features that could add or select another entry point. (Dependency
/// lines are not examined.)
#[test]
fn p0_j5_package_has_no_other_entry_point() {
    let dir = manifest_dir();
    assert!(!dir.join("src").join("bin").exists());
    for extra in ["examples", "benches"] {
        assert!(
            !dir.join(extra).exists(),
            "nexus-code/{extra} must not exist"
        );
    }
    assert!(entries(&dir.join("tests")).contains(&"phase0_withdrawal.rs".to_string()));

    let manifest = read(&dir.join("Cargo.toml"));
    assert_eq!(
        bin_targets(&manifest),
        [("nx".to_string(), "src/main.rs".to_string())]
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
            "nexus-code/Cargo.toml must not declare `{forbidden}`"
        );
    }
}
