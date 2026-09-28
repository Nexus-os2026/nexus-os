//! P0-FINAL-GATE-CLOSURE (dossier item J4, alternate alias): the standalone
//! `social-poster-agent` executable is withdrawn with `nexus-cli`, whose
//! `agent start social-poster` command it duplicated.
//!
//! The behavioural tests run the executable built from this package, never a
//! program found on `PATH`, and only after checking that the entry source is
//! exactly the withdrawal and that the executable carries this package's
//! withdrawal message (and not J1's). The working directory holds a decoy
//! manifest where the retired binary looked for one; it must stay unchanged.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WITHDRAWN_MESSAGE: &str =
    "social-poster-agent: unavailable during Phase Zero; standalone use withdrawn";
const WITHDRAWN_STATUS: i32 = 69;
const RUN_DEADLINE: Duration = Duration::from_secs(60);
/// The withdrawal message of `crates/nexus-server` (J1).
const J1_MESSAGE: &str = "nexus-server: unavailable during Phase Zero; deployment withdrawn";
const DECOY_MANIFEST: &str = "agents/social-poster/manifest.toml";

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

/// The executable Cargo built for `social-poster-agent`, after the source and byte
/// identification. Panics (and runs nothing) otherwise.
fn withdrawn_binary() -> PathBuf {
    assert_entry_is_withdrawal();
    let path = PathBuf::from(env!("CARGO_BIN_EXE_social-poster-agent"));
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
            "p0-j4-social-poster-{name}-{}-{}-{}",
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
        let decoy = cwd.join(DECOY_MANIFEST);
        fs::create_dir_all(decoy.parent().expect("decoy parent")).expect("create decoy dir");
        fs::write(&decoy, "p0j4 sentinel decoy: not a manifest\n").expect("write decoy");
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

/// Synthetic values placed in the child's environment; none is a real key.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("X_API_KEY", "p0j4-sentinel-x-api-key"),
    ("X_ACCESS_TOKEN", "p0j4-sentinel-x-access-token"),
    ("BRAVE_API_KEY", "p0j4-sentinel-brave-key"),
    ("ANTHROPIC_API_KEY", "p0j4-sentinel-anthropic-key"),
    ("OLLAMA_URL", "http://127.0.0.1:9/p0j4-sentinel-ollama"),
    ("NEXUS_CONFIG_KEY", "p0j4-sentinel-config-key"),
    ("RUST_LOG", "trace"),
];

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
                    "social-poster-agent did not exit within {RUN_DEADLINE:?} ({outcome:?}); it was killed"
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

/// Every retired invocation, including an explicit manifest and a live
/// (publishing) run, gets the same denial; the decoy manifest, the working
/// directory and home stay unchanged; no argument or environment value is
/// echoed.
#[test]
fn p0_j4_social_poster_every_invocation_is_withdrawn() {
    let fixture = Fixture::new("invocations");
    let cwd_before = snapshot(&fixture.cwd);
    let decoy = fixture.cwd.join(DECOY_MANIFEST);
    let decoy = decoy.to_str().expect("UTF-8 fixture path");
    let invocations: Vec<Vec<&str>> = vec![
        vec![],
        vec!["--dry-run"],
        vec!["--manifest", decoy],
        vec!["--manifest", decoy, "--dry-run"],
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["p0j4-sentinel-argument-value"],
        vec!["--"],
    ];
    for args in &invocations {
        let outcome = run(&fixture, &os_args(args));
        assert_withdrawn(&outcome, &format!("{args:?}"));
        let output = format!("{}{}", outcome.stdout, outcome.stderr);
        assert!(!output.contains("p0j4"), "{args:?}: nothing is echoed");
    }
    assert_eq!(
        snapshot(&fixture.cwd),
        cwd_before,
        "working directory unchanged"
    );
    assert!(snapshot(&fixture.home).is_empty(), "home stays empty");
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

fn assert_entry_is_withdrawal() {
    let source = read(&manifest_dir().join("src").join("main.rs"));
    let code = strip_rust_comments(&source);
    let expected = format!(
        "#![forbid(unsafe_code)] #[cfg(not(test))] \
         fn main() -> std::process::ExitCode {{ use std::io::Write; \
         const WITHDRAWN_MESSAGE: &str = \"{WITHDRAWN_MESSAGE}\"; \
         const WITHDRAWN_STATUS: u8 = 69; \
         let _ = writeln!(std::io::stderr(), \"{{WITHDRAWN_MESSAGE}}\"); \
         std::process::ExitCode::from(WITHDRAWN_STATUS) }}"
    );
    for forbidden in [
        "social_poster_agent",
        "run_social_poster_from_manifest",
        "tokio",
        "spawn",
        "Command",
        "std::env",
        "env!",
        "args",
        "var(",
        "fs::",
        "PathBuf",
        "current_dir",
        "mod ",
        "include!",
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
        normalize_whitespace(&expected),
        "src/main.rs must be exactly the withdrawal entry point (comments aside)"
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

/// The entry is exactly the withdrawal, and the package's only binary is
/// `social-poster-agent` from `src/main.rs` (no `src/bin`, examples, benches,
/// `default-run`, auto-discovery switch or features; dependency lines are
/// not examined).
#[test]
fn p0_j4_social_poster_entry_and_package_are_only_the_withdrawal() {
    assert_entry_is_withdrawal();
    assert!(!WITHDRAWN_MESSAGE.contains(J1_MESSAGE));
    let dir = manifest_dir();
    assert!(!dir.join("src").join("bin").exists());
    for extra in ["examples", "benches"] {
        assert!(!dir.join(extra).exists(), "{extra} must not exist");
    }
    let manifest = read(&dir.join("Cargo.toml"));
    assert_eq!(
        bin_targets(&manifest),
        [("social-poster-agent".to_string(), "src/main.rs".to_string())]
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
            "Cargo.toml must not declare `{forbidden}`"
        );
    }
}
