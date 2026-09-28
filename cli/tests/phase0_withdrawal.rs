//! P0-FINAL-GATE-CLOSURE, dossier item J4: `nexus-cli` (the `nexus` command)
//! is withdrawn, whole.
//!
//! The behavioural tests run the executable built from this package, never a
//! program found on `PATH`. Before any run they check that the entry source
//! is exactly the withdrawal and that the executable carries this package's
//! withdrawal message (and not that of the separately withdrawn
//! `crates/nexus-server`, dossier item J1), so a build that is not the
//! withdrawal is never executed. The working directory holds decoys for every
//! file the retired commands looked for there; they must stay unchanged.
//!
//! The source guards pin the entry point and the package's single binary.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WITHDRAWN_MESSAGE: &str =
    "nexus-cli: unavailable during Phase Zero; standalone use withdrawn";
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

/// The executable Cargo built for `nexus-cli`, after checking that its entry
/// source is exactly the withdrawal and that it carries this package's
/// withdrawal message. Panics (and runs nothing) otherwise.
fn withdrawn_binary() -> PathBuf {
    assert_entry_is_withdrawal();
    let path = PathBuf::from(env!("CARGO_BIN_EXE_nexus-cli"));
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

/// A scratch directory owned by one test, inside Cargo's per-target test
/// directory. The working directory holds decoys for the files the retired
/// commands read or ran from there; home starts empty.
struct Fixture {
    root: PathBuf,
    cwd: PathBuf,
    home: PathBuf,
    runs: AtomicUsize,
}

/// Files the retired commands looked for in the working directory, as
/// decoys: none is valid, and each must stay byte-identical.
const CWD_DECOYS: &[(&str, &str)] = &[
    ("voice/jarvis.py", "# p0j4 sentinel decoy: never executed\n"),
    (
        "agents/coding-agent/manifest.toml",
        "p0j4 sentinel decoy: not a manifest\n",
    ),
    (
        "agents/social-poster/manifest.toml",
        "p0j4 sentinel decoy: not a manifest\n",
    ),
    ("policy.toml", "p0j4 sentinel decoy: not a policy\n"),
    (
        "project/manifest.toml",
        "p0j4 sentinel decoy: not a manifest\n",
    ),
];

impl Fixture {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = format!(
            "p0-j4-{name}-{}-{}-{}",
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
        for (relative, content) in CWD_DECOYS {
            let path = cwd.join(relative);
            fs::create_dir_all(path.parent().expect("decoy parent")).expect("create decoy dir");
            fs::write(&path, content).expect("write decoy");
        }
        Fixture {
            root,
            cwd,
            home,
            runs: AtomicUsize::new(0),
        }
    }

    /// Locations outside the working directory and home that the retired
    /// commands would have created. None may appear.
    fn created_elsewhere(&self) -> Vec<PathBuf> {
        ["create", "conduct", "self-improve", "db", "config", "tmp"]
            .iter()
            .map(|name| self.root.join(name))
            .filter(|path| path.exists())
            .collect()
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
/// every endpoint is a closed loopback port.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("OLLAMA_URL", "http://127.0.0.1:9/p0j4-sentinel-ollama"),
    (
        "OLLAMA_HOST",
        "http://127.0.0.1:9/p0j4-sentinel-ollama-host",
    ),
    ("ANTHROPIC_API_KEY", "p0j4-sentinel-anthropic-key"),
    ("OPENAI_API_KEY", "p0j4-sentinel-openai-key"),
    ("GROQ_API_KEY", "p0j4-sentinel-groq-key"),
    ("BRAVE_API_KEY", "p0j4-sentinel-brave-key"),
    ("X_API_KEY", "p0j4-sentinel-x-key"),
    ("TELEGRAM_BOT_TOKEN", "p0j4-sentinel-telegram-token"),
    ("GITHUB_TOKEN", "p0j4-sentinel-github-token"),
    ("NEXUS_CONFIG_KEY", "p0j4-sentinel-config-key"),
    ("NEXUS_ENCRYPTION_KEY", "p0j4-sentinel-encryption-key"),
    ("RUST_LOG", "trace"),
];

/// Runs the withdrawn executable once, with an empty environment apart from
/// the fixture's home, temp and state locations and the synthetic sentinels.
fn run(fixture: &Fixture, args: &[OsString]) -> Outcome {
    let executable = withdrawn_binary();
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
        .env("TMPDIR", fixture.root.join("tmp"))
        .env("TEMP", fixture.root.join("tmp"))
        .env("TMP", fixture.root.join("tmp"))
        .env("NEXUS_SELF_IMPROVE_DIR", fixture.root.join("self-improve"))
        .env("NEXUS_DB_PATH", fixture.root.join("db").join("nexus.db"))
        .env(
            "NEXUS_CONFIG_PATH",
            fixture.root.join("config").join("config.toml"),
        )
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
                    "nexus-cli did not exit within {RUN_DEADLINE:?} ({outcome:?}); it was killed"
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
    for word in ["usage", "created", "started", "completed", "agent", "voice"] {
        assert!(
            !outcome.stderr.to_ascii_lowercase().contains(word),
            "{context}: no command output ({word})"
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

fn assert_untouched(
    fixture: &Fixture,
    cwd_before: &BTreeMap<PathBuf, Option<Vec<u8>>>,
    context: &str,
) {
    assert_eq!(
        &snapshot(&fixture.cwd),
        cwd_before,
        "{context}: the working directory and its decoys are unchanged"
    );
    assert!(
        snapshot(&fixture.home).is_empty(),
        "{context}: home must stay empty"
    );
    assert_eq!(
        fixture.created_elsewhere(),
        Vec::<PathBuf>::new(),
        "{context}: no state location is created"
    );
}

// ── Behaviour of the real executable ────────────────────────────────────────

/// Invoked with no arguments, the executable prints only the withdrawal
/// message, exits with the fixed status, and changes nothing.
#[test]
fn p0_j4_default_invocation_is_withdrawn() {
    let fixture = Fixture::new("default");
    let cwd_before = snapshot(&fixture.cwd);
    let outcome = run(&fixture, &[]);
    assert_withdrawn(&outcome, "no arguments");
    assert_untouched(&fixture, &cwd_before, "no arguments");
    fixture.remove();
}

/// Every retired command, including those that ran processes, published,
/// wrote into the working directory or rewrote configuration, gets the same
/// denial. Nothing is created, run or changed.
#[test]
fn p0_j4_retired_commands_cannot_reactivate_it() {
    let fixture = Fixture::new("commands");
    let cwd_before = snapshot(&fixture.cwd);
    let root = fixture
        .root
        .to_str()
        .expect("UTF-8 fixture path")
        .to_string();
    let cwd = fixture
        .cwd
        .to_str()
        .expect("UTF-8 fixture path")
        .to_string();
    let create_dir = format!("{root}/create");
    let conduct_dir = format!("{root}/conduct");
    let project_dir = format!("{cwd}/project");
    let policy = format!("{cwd}/policy.toml");
    let coding_manifest = format!("{cwd}/agents/coding-agent/manifest.toml");
    let invocations: Vec<Vec<&str>> = vec![
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["-V"],
        vec!["help"],
        vec!["setup"],
        vec!["setup", "--check"],
        vec!["voice", "start"],
        vec!["voice", "test"],
        vec!["voice", "models"],
        vec!["agent", "start", "coding-agent", "--dry-run"],
        vec!["agent", "start", "social-poster", "--dry-run"],
        vec!["agent", "create", &coding_manifest],
        vec!["agent", "list"],
        vec![
            "conduct",
            "p0j4 sentinel",
            "--preview",
            "--output-dir",
            &conduct_dir,
        ],
        vec!["create", "p0j4-agent", "--output-dir", &create_dir],
        vec!["test", &coding_manifest],
        vec!["package", &project_dir],
        vec!["self-improve", "run", "--agent", "p0j4"],
        vec!["marketplace", "search", "p0j4"],
        vec!["marketplace", "install", "p0j4"],
        vec!["policy", "validate", &policy],
        vec!["policy", "reload"],
        vec!["protocols", "start", "--port", "0"],
        vec!["protocols", "status"],
        vec!["model", "list"],
        vec!["governance", "test", "pii_detection", "p0j4"],
        vec!["sandbox", "status"],
        vec!["simulation", "status"],
        vec!["--"],
        vec![""],
    ];
    for args in &invocations {
        let outcome = run(&fixture, &os_args(args));
        assert_withdrawn(&outcome, &format!("{args:?}"));
    }
    assert!(!fixture.cwd.join("nexus-output").exists());
    assert_untouched(&fixture, &cwd_before, "retired commands");
    fixture.remove();
}

/// The denial never echoes an argument or environment value.
#[test]
fn p0_j4_denial_echoes_no_environment_or_argument_value() {
    let fixture = Fixture::new("echo");
    let cwd_before = snapshot(&fixture.cwd);
    let argument_sentinels = [
        "p0j4-sentinel-argument-value",
        "--p0j4-sentinel-flag",
        "http://127.0.0.1:9/p0j4-sentinel-url",
    ];
    let mut args = os_args(&["conduct"]);
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
    assert!(!output.contains("p0j4"), "no sentinel fragment is echoed");
    assert_untouched(&fixture, &cwd_before, "sentinels");
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
        \"nexus-cli: unavailable during Phase Zero; standalone use withdrawn\"; \
        const WITHDRAWN_STATUS: u8 = 69; \
        let _ = writeln!(std::io::stderr(), \"{WITHDRAWN_MESSAGE}\"); \
        std::process::ExitCode::from(WITHDRAWN_STATUS) }";
    for forbidden in [
        "nexus_cli",
        "execute_command",
        "Cli",
        "clap",
        "Parser",
        "coding_agent",
        "social_poster",
        "self_improve",
        "python",
        "tokio",
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

/// The production entry of `nexus-cli` is exactly the withdrawal. It parses
/// no command, calls no `nexus_cli` function, spawns nothing and reads no
/// argument, environment variable or file.
#[test]
fn p0_j4_entry_point_is_only_the_withdrawal() {
    assert_entry_is_withdrawal();
    assert!(!WITHDRAWN_MESSAGE.contains(J1_MESSAGE) && !J1_MESSAGE.contains(WITHDRAWN_MESSAGE));
}

/// The package's only binary is the one Cargo infers from `src/main.rs`:
/// no `[[bin]]`, `src/bin`, examples or benches, and no `default-run`,
/// target auto-discovery switch or features that could add or select
/// another entry point. (Dependency lines are not examined.)
#[test]
fn p0_j4_package_has_no_other_entry_point() {
    let dir = manifest_dir();
    assert!(dir.join("src").join("main.rs").is_file());
    assert!(!dir.join("src").join("bin").exists());
    for extra in ["examples", "benches"] {
        assert!(!dir.join(extra).exists(), "cli/{extra} must not exist");
    }
    assert!(entries(&dir.join("tests")).contains(&"phase0_withdrawal.rs".to_string()));

    let manifest = read(&dir.join("Cargo.toml"));
    let lines: Vec<&str> = manifest.lines().map(str::trim).collect();
    assert!(lines.contains(&"name = \"nexus-cli\""));
    for forbidden in [
        "[[bin]]",
        "default-run",
        "autobins",
        "autoexamples",
        "[[example]]",
        "[[bench]]",
        "[features]",
    ] {
        assert!(
            !lines.iter().any(|line| line.starts_with(forbidden)),
            "cli/Cargo.toml must not declare `{forbidden}`"
        );
    }
}

// ── Withdrawn packaging and release recipes ─────────────────────────────────

fn workspace_root() -> PathBuf {
    manifest_dir().join("..")
}

/// Lines that are neither blank nor comments (`#`, and `;` for systemd; a
/// shebang counts as a comment).
fn directive_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
        .collect()
}

/// XML text with `<!-- ... -->` comments removed.
fn strip_xml_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => panic!("unterminated XML comment"),
        }
    }
    out.push_str(rest);
    out
}

/// The systemd unit and the launchd job that ran `nexus-cli` start nothing:
/// the unit has no `[Service]` (systemd refuses a service without
/// `ExecStart=`) and no `[Install]` (it cannot be enabled); the launchd job
/// names no program and is disabled.
#[test]
fn p0_j4_service_units_start_nothing() {
    let unit = read(&workspace_root().join("packaging/linux/nexus-os.service"));
    assert_eq!(
        directive_lines(&unit),
        [
            "[Unit]",
            "Description=WITHDRAWN during Phase Zero: this unit starts nothing"
        ],
        "the systemd unit must be the withdrawal stub"
    );
    for forbidden in [
        "[Service]",
        "[Install]",
        "ExecStart",
        "ExecStop",
        "WantedBy",
        "Restart",
        "/usr/bin/nexus-cli",
    ] {
        assert!(
            !directive_lines(&unit).join("\n").contains(forbidden),
            "the systemd unit must not contain `{forbidden}`"
        );
    }

    let plist = strip_xml_comments(&read(
        &workspace_root().join("packaging/macos/com.nexusos.agent.plist"),
    ));
    let plist = normalize_whitespace(&plist);
    assert!(
        plist.contains("<key>Disabled</key> <true/>"),
        "the launchd job must be disabled"
    );
    for forbidden in [
        "<key>Program</key>",
        "ProgramArguments",
        "RunAtLoad",
        "KeepAlive",
        "StartInterval",
        "StartCalendarInterval",
        "StartOnMount",
        "WatchPaths",
        "QueueDirectories",
        "Sockets",
        "WorkingDirectory",
        "StandardOutPath",
        "StandardErrorPath",
        "nexus-cli",
    ] {
        assert!(
            !plist.contains(forbidden),
            "the launchd job must not contain `{forbidden}`"
        );
    }
}

/// The Homebrew formula raises as soon as it is loaded (it defines no
/// formula, so nothing is fetched, built, installed or kept running), and the
/// WiX source stops at a preprocessor error before compilation (no installer,
/// no file, no component).
#[test]
fn p0_j4_homebrew_formula_and_msi_source_build_nothing() {
    let formula = read(&workspace_root().join("packaging/macos/homebrew/nexus-os.rb"));
    assert_eq!(
        directive_lines(&formula),
        ["raise \"nexus-os formula: withdrawn during Phase Zero; nothing is built or installed\""],
        "the formula must only raise"
    );
    let code = directive_lines(&formula).join("\n");
    for forbidden in [
        "class ",
        "Formula",
        "url ",
        "sha256",
        "cargo",
        "bin.install",
        "service",
        "system ",
    ] {
        assert!(
            !code.contains(forbidden),
            "the formula must not contain `{forbidden}`"
        );
    }

    let wxs = strip_xml_comments(&read(
        &workspace_root().join("packaging/windows/nexus-os.wxs"),
    ));
    assert_eq!(
        normalize_whitespace(&wxs),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?> \
         <?error nexus-os.wxs: withdrawn during Phase Zero; no installer is built ?> \
         <Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\" />",
        "the WiX source must stop at the preprocessor error and define nothing"
    );
    for forbidden in [
        "<Package",
        "<File",
        "<Component",
        "<Directory",
        "<Feature",
        "Source=",
        "nexus-cli.exe",
    ] {
        assert!(
            !wxs.contains(forbidden),
            "the WiX source must not contain `{forbidden}`"
        );
    }
}

/// The packaging scripts only print the withdrawal and fail: no build, no
/// copy, no package tool, no argument or variable.
#[test]
fn p0_j4_packaging_scripts_build_nothing() {
    for (script, shebang, expected) in [
        (
            "scripts/build_linux_deb.sh",
            Some("#!/usr/bin/env bash"),
            [
                "echo \"build_linux_deb.sh: withdrawn during Phase Zero; nothing is built or \
                 packaged\" >&2",
                "exit 1",
            ],
        ),
        (
            "scripts/build_macos_release.sh",
            Some("#!/usr/bin/env bash"),
            [
                "echo \"build_macos_release.sh: withdrawn during Phase Zero; nothing is built or \
                 packaged\" >&2",
                "exit 1",
            ],
        ),
        (
            "scripts/build_windows_msi.ps1",
            None,
            [
                "[Console]::Error.WriteLine(\"build_windows_msi.ps1: withdrawn during Phase \
                 Zero; nothing is built or packaged\")",
                "exit 1",
            ],
        ),
    ] {
        let text = read(&workspace_root().join(script));
        if let Some(shebang) = shebang {
            // `lines()` drops a `\r`, so a CRLF checkout (Windows) reads the same.
            assert_eq!(text.lines().next(), Some(shebang), "{script}: shebang");
        }
        assert_eq!(
            directive_lines(&text),
            expected,
            "{script} must only print the withdrawal and fail"
        );
        let code = directive_lines(&text).join("\n");
        for forbidden in [
            "$", "cargo", "target/", "dpkg", "wix ", "hdiutil", "tar ", "cp ", "param",
        ] {
            assert!(
                !code.contains(forbidden),
                "{script} must not use `{forbidden}`"
            );
        }
    }
}

/// The packaging directories hold only the withdrawn recipes, so no other
/// unit, job, formula or installer source for `nexus-cli` sits beside them.
#[test]
fn p0_j4_packaging_directories_hold_only_withdrawn_recipes() {
    let packaging = workspace_root().join("packaging");
    assert_eq!(entries(&packaging.join("linux")), ["nexus-os.service"]);
    assert_eq!(
        entries(&packaging.join("macos")),
        ["com.nexusos.agent.plist", "homebrew"]
    );
    assert_eq!(entries(&packaging.join("macos/homebrew")), ["nexus-os.rb"]);
    assert_eq!(entries(&packaging.join("windows")), ["nexus-os.wxs"]);
}

/// The GitLab `release-build` job builds and publishes nothing: it keeps its
/// manual trigger on version tags but its script only prints the withdrawal
/// and fails, and it declares no artifact. No job builds or exports the
/// `nexus-cli` binary. The core test job still tests the `nexus-cli`
/// package (its library and these withdrawal tests).
#[test]
fn p0_j4_gitlab_release_build_job_publishes_nothing() {
    let ci = read(&workspace_root().join(".gitlab-ci.yml"));
    let lines: Vec<&str> = ci.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == "release-build:")
        .expect("the release-build job stays, withdrawn");
    let block: Vec<&str> = std::iter::once(lines[start])
        .chain(
            lines[start + 1..]
                .iter()
                .take_while(|line| line.is_empty() || line.starts_with(' '))
                .copied(),
        )
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        block,
        [
            "release-build:",
            "stage: deploy",
            "allow_failure: true",
            "script:",
            "- echo \"release-build is withdrawn during Phase Zero; nexus-cli is not built or \
             published\" >&2",
            "- exit 1",
            "rules:",
            "- if: $CI_COMMIT_TAG =~ /^v/",
            "when: manual",
        ],
        "release-build must only print the withdrawal and fail"
    );
    for line in lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.starts_with('#'))
    {
        assert!(
            !(line.contains("cargo build") && line.contains("nexus-cli")),
            "no job may build nexus-cli: {line}"
        );
        assert!(
            !line.contains("target/release/nexus-cli"),
            "no job may export the nexus-cli binary: {line}"
        );
    }
    assert!(
        lines
            .iter()
            .any(|line| line.trim() == "- cargo test -p nexus-kernel -p nexus-sdk -p nexus-cli"),
        "the core test job still tests the nexus-cli package"
    );
}

// ── Withdrawn command-line documentation ────────────────────────────────────

fn assert_doc(path: &str, required: &[&str], forbidden: &[&str]) {
    // Line breaks in the Markdown source do not matter.
    let text = normalize_whitespace(&read(&workspace_root().join(path)));
    for phrase in required {
        assert!(text.contains(phrase), "{path} must say `{phrase}`");
    }
    for phrase in forbidden {
        assert!(
            !text.contains(phrase),
            "{path} must not instruct the withdrawn CLI (`{phrase}`)"
        );
    }
}

/// The user guide and the social poster's README say that `nexus-cli` is
/// withdrawn, that existing installations are not removed or stopped, and
/// give no command that builds, installs or runs it; the desktop installers
/// stay.
#[test]
fn p0_j4_user_docs_withdraw_the_cli_without_claiming_a_stop() {
    assert_doc(
        "docs/USER_GUIDE.md",
        &[
            "Install the NexusOS desktop app with the installer for your platform",
            "## Command-line interface (withdrawn)",
            "`nexus-cli` (the `nexus` command) is withdrawn during Phase Zero",
            "The repository provides no supported installation of it",
            "is not removed or stopped by this change",
        ],
        &[
            "cargo build --release -p nexus-cli",
            "target/release/nexus-cli",
            "nexus-cli.exe",
            "`nexus setup",
            "`nexus agent create",
            "`nexus agent start",
            "`nexus agent logs",
            "`nexus agent audit",
            "`nexus voice",
            "nexus --help",
            "Install binary to your PATH",
            "desktop app or CLI",
            "CLI:",
        ],
    );
    assert_doc(
        "agents/social-poster/README.md",
        &[
            "## Running it (withdrawn)",
            "The repository provides no supported way to run this agent from the command line \
             at this point",
        ],
        &[
            "`nexus setup`",
            "`nexus agent create",
            "`nexus agent start",
            "`nexus agent logs",
            "--dry-run`",
        ],
    );
}
