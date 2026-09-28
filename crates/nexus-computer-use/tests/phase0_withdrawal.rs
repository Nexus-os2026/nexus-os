//! P0-FINAL-GATE-CLOSURE (dossier J items, decision D2): the computer-use
//! harness executables `nx-screen`, `nx-input`, `nx-agent`, `nx-govern` and
//! `nx-learn` are withdrawn. They captured the screen, sent OS input, ran an
//! autonomous capture-model-input loop and changed stored grants and patterns
//! outside the governed desktop.
//!
//! The behavioural tests run the executables built from this package, never
//! a program found on `PATH`, and only after checking that each entry source
//! is exactly the withdrawal and that each executable carries its own
//! withdrawal message (and no sibling's, and not J1's). A display sentinel is
//! set so that nothing could reach a real display even if it tried.

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
/// The withdrawal message of `crates/nexus-server` (J1).
const J1_MESSAGE: &str = "nexus-server: unavailable during Phase Zero; deployment withdrawn";

/// One withdrawn binary target of this package, with retired invocations.
struct Withdrawn {
    name: &'static str,
    source: &'static str,
    message: &'static str,
    executable: &'static str,
    retired: &'static [&'static [&'static str]],
}

const SCREEN: Withdrawn = Withdrawn {
    name: "nx-screen",
    source: "screen_test.rs",
    message: "nx-screen: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nx-screen"),
    retired: &[
        &["--output", "OUT/screen.png"],
        &["--region", "0,0,10,10", "--output", "OUT/region.png"],
    ],
};

const INPUT: Withdrawn = Withdrawn {
    name: "nx-input",
    source: "input_test.rs",
    message: "nx-input: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nx-input"),
    retired: &[
        &["click", "1", "1"],
        &["move", "1", "1"],
        &["type", "p0d2 sentinel text"],
        &["key", "Return"],
        &["combo", "ctrl+alt+t"],
        &["position"],
        &["status"],
    ],
};

const AGENT: Withdrawn = Withdrawn {
    name: "nx-agent",
    source: "agent_test.rs",
    message: "nx-agent: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nx-agent"),
    retired: &[
        &["p0d2 sentinel task"],
        &["--auto", "p0d2 sentinel task"],
        &["--dry-run", "p0d2 sentinel task"],
        &[
            "--max-steps",
            "1",
            "--threshold",
            "0.9",
            "p0d2 sentinel task",
        ],
    ],
};

const GOVERN: Withdrawn = Withdrawn {
    name: "nx-govern",
    source: "governance_test.rs",
    message: "nx-govern: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nx-govern"),
    retired: &[
        &["list"],
        &["focused"],
        &["grants"],
        &["grant", "p0d2-class", "full"],
        &["test-click"],
        &["test-type"],
        &["session"],
    ],
};

const LEARN: Withdrawn = Withdrawn {
    name: "nx-learn",
    source: "learn_test.rs",
    message: "nx-learn: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nx-learn"),
    retired: &[
        &["patterns"],
        &["memory"],
        &["match", "p0d2 sentinel task"],
        &["optimize"],
        &["reset"],
        &["stats"],
    ],
};

const ALL: [&Withdrawn; 5] = [&SCREEN, &INPUT, &AGENT, &GOVERN, &LEARN];

/// Invocations every executable gets besides its own retired commands.
const COMMON: &[&[&str]] = &[
    &[],
    &["--help"],
    &["-h"],
    &["--version"],
    &["help"],
    &["--"],
];

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

/// The executable Cargo built for `target`, after the source and byte
/// identification. Panics (and runs nothing) otherwise.
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
    for other in ALL.iter().filter(|other| other.name != target.name) {
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
            "p0-d2-{name}-{}-{}-{}",
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
        for dir in [&cwd, &home, &root.join("out"), &root.join("capture")] {
            fs::create_dir_all(dir).expect("create fixture directory");
        }
        Fixture {
            root,
            cwd,
            home,
            runs: AtomicUsize::new(0),
        }
    }

    fn remove(self) {
        fs::remove_dir_all(&self.root).expect("remove fixture directory");
    }
}

struct Outcome {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

/// Synthetic values placed in the child's environment. The display values
/// name no real display; none is a real key.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("DISPLAY", ":98765"),
    ("WAYLAND_DISPLAY", "p0d2-sentinel-wayland"),
    ("ANTHROPIC_API_KEY", "p0d2-sentinel-anthropic-key"),
    ("NEXUS_CONFIG_KEY", "p0d2-sentinel-config-key"),
    ("RUST_LOG", "trace"),
];

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
        .env("XDG_RUNTIME_DIR", home.join("xdg-runtime"))
        .env("APPDATA", home.join("appdata"))
        .env("LOCALAPPDATA", home.join("localappdata"))
        .env("TMPDIR", home.join("tmp"))
        .env("TEMP", home.join("tmp"))
        .env("TMP", home.join("tmp"))
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
    let output = format!("{}{}", outcome.stdout, outcome.stderr);
    assert!(!output.contains("p0d2"), "{context}: nothing is echoed");
}

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

/// Every executable, with no arguments, help and version words and each of
/// its retired commands (a capture to a file, OS input, an autonomous run
/// without approval, a grant, a reset), gets only its fixed denial. Nothing
/// is captured, written or created, and no value is echoed.
#[test]
fn p0_d2_every_harness_invocation_is_withdrawn() {
    let fixture = Fixture::new("invocations");
    let capture = fixture.root.join("capture");
    let capture = capture.to_str().expect("UTF-8 fixture path").to_string();
    for target in ALL {
        for args in COMMON.iter().chain(target.retired.iter()) {
            let args: Vec<OsString> = args
                .iter()
                .map(|arg| OsString::from(arg.replace("OUT", &capture)))
                .collect();
            let outcome = run(&fixture, target, &args);
            assert_withdrawn(&outcome, target, &format!("{args:?}"));
        }
    }
    assert!(
        snapshot(&fixture.cwd).is_empty(),
        "working directory stays empty"
    );
    assert!(snapshot(&fixture.home).is_empty(), "home stays empty");
    assert!(
        snapshot(&fixture.root.join("capture")).is_empty(),
        "no capture is written"
    );
    fixture.remove();
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
    for forbidden in [
        "nexus_computer_use",
        "capture",
        "screenshot",
        "Mouse",
        "Keyboard",
        "run_agent_loop",
        "AppRegistry",
        "PatternLibrary",
        "tokio",
        "tracing",
        "spawn",
        "Command",
        "std::env",
        "env!",
        "args",
        "var(",
        "fs::",
        "PathBuf",
        "mod ",
        "include!",
        "#[path",
        "extern crate",
        "unsafe {",
        "ExitCode::SUCCESS",
    ] {
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

/// Every entry is exactly the withdrawal, and the package builds exactly the
/// five withdrawn binaries from their five sources: no other `src/bin` file,
/// no `src/main.rs`, examples or benches, and no `default-run`,
/// auto-discovery switch or features. (Dependency lines are not examined.)
#[test]
fn p0_d2_entries_and_package_are_only_the_withdrawal() {
    for target in ALL {
        assert_entry_is_withdrawal(target);
        assert!(!target.message.contains(J1_MESSAGE));
    }
    let dir = manifest_dir();
    let mut sources: Vec<String> = ALL.iter().map(|t| t.source.to_string()).collect();
    sources.sort();
    let mut listed: Vec<String> = fs::read_dir(dir.join("src").join("bin"))
        .expect("list src/bin")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    listed.sort();
    assert_eq!(
        listed, sources,
        "src/bin holds exactly the withdrawn entries"
    );
    assert!(!dir.join("src").join("main.rs").exists());
    for extra in ["examples", "benches"] {
        assert!(!dir.join(extra).exists(), "{extra} must not exist");
    }
    let manifest = read(&dir.join("Cargo.toml"));
    let expected: Vec<(String, String)> = ALL
        .iter()
        .map(|t| (t.name.to_string(), format!("src/bin/{}", t.source)))
        .collect();
    assert_eq!(bin_targets(&manifest), expected);
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
            "Cargo.toml must not declare `{forbidden}`"
        );
    }
}
