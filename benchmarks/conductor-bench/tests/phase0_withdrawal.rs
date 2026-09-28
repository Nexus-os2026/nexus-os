//! P0-FINAL-GATE-CLOSURE (standalone surfaces, coordinator decision after
//! internal review): the benchmark executables `nim-cloud-bench`,
//! `cloud-models-bench`, `inference-consistency-bench`,
//! `local-vs-cloud-battle` and `real-agent-validation` are withdrawn. Each
//! read a provider key from the environment and sent it as a bearer token on
//! curl's command line, where any local process could read it, and each sent
//! the `GROQ_API_KEY` value to NVIDIA NIM. The other benchmark binaries of
//! this package stay (developer/benchmark use, pending the Architect's
//! decision D3).
//!
//! The behavioural tests run the executables built from this package, never
//! a program found on `PATH`, and only after checking that each entry source
//! is exactly the withdrawal and that each executable carries its own
//! withdrawal message (and no sibling's, and not J1's). The environment holds
//! sentinel keys and settings where the benchmarks read them, and proxy
//! settings that point at a closed local port, so nothing could reach a
//! provider even if it tried. The working directory holds decoy reports and
//! manifests where the benchmarks wrote and read them; they must stay
//! unchanged.

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

/// One withdrawn binary target of this package, with the files it used to
/// write into (or read from) the working directory.
struct Withdrawn {
    name: &'static str,
    source: &'static str,
    message: &'static str,
    executable: &'static str,
    decoys: &'static [&'static str],
}

const NIM: Withdrawn = Withdrawn {
    name: "nim-cloud-bench",
    source: "nim_cloud_bench.rs",
    message: "nim-cloud-bench: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_nim-cloud-bench"),
    decoys: &["CLOUD_MODELS_ONLY_RESULTS.md"],
};

const CLOUD: Withdrawn = Withdrawn {
    name: "cloud-models-bench",
    source: "cloud_models_bench.rs",
    message: "cloud-models-bench: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_cloud-models-bench"),
    decoys: &["CLOUD_MODELS_COMPARISON_RESULTS.md"],
};

const CONSISTENCY: Withdrawn = Withdrawn {
    name: "inference-consistency-bench",
    source: "inference_consistency_bench.rs",
    message: "inference-consistency-bench: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_inference-consistency-bench"),
    decoys: &["INFERENCE_CONSISTENCY_RESULTS.md"],
};

const BATTLE: Withdrawn = Withdrawn {
    name: "local-vs-cloud-battle",
    source: "local_vs_cloud_battle.rs",
    message: "local-vs-cloud-battle: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_local-vs-cloud-battle"),
    decoys: &["LOCAL_vs_CLOUD_BATTLE_RESULTS.md"],
};

const AGENTS: Withdrawn = Withdrawn {
    name: "real-agent-validation",
    source: "real_agent_validation.rs",
    message: "real-agent-validation: unavailable during Phase Zero; standalone use withdrawn",
    executable: env!("CARGO_BIN_EXE_real-agent-validation"),
    decoys: &[
        "REAL_AGENT_VALIDATION_RESULTS.md",
        "agents/prebuilt/p0bench-decoy.json",
    ],
};

const ALL: [&Withdrawn; 5] = [&NIM, &CLOUD, &CONSISTENCY, &BATTLE, &AGENTS];

/// Invocations every executable gets. The benchmarks took no arguments;
/// their retired runs are the environment below.
const INVOCATIONS: &[&[&str]] = &[
    &[],
    &["--help"],
    &["-h"],
    &["--version"],
    &["help"],
    &["--release"],
    &["p0bench-sentinel-argument-value"],
    &["--"],
];

/// Every binary target of this package, as (name, path), in manifest order:
/// the five withdrawn entries and the benchmarks that stay.
const PACKAGE_BINARIES: &[(&str, &str)] = &[
    ("conductor-bench", "src/main.rs"),
    ("memory-profile", "src/memory_profile.rs"),
    ("audit-retention-bench", "src/audit_retention_bench.rs"),
    (
        "inference-consistency-bench",
        "src/inference_consistency_bench.rs",
    ),
    ("cloud-models-bench", "src/cloud_models_bench.rs"),
    ("local-vs-cloud-battle", "src/local_vs_cloud_battle.rs"),
    ("nim-cloud-bench", "src/nim_cloud_bench.rs"),
    ("darwin-drift-bench", "src/darwin_drift_bench.rs"),
    ("audit-throughput-bench", "src/audit_throughput_bench.rs"),
    (
        "multiagent-coordination-bench",
        "src/multiagent_coordination_bench.rs",
    ),
    ("genesis-protocol-bench", "src/genesis_protocol_bench.rs"),
    ("real-battery-validation", "src/real_battery_validation.rs"),
    ("real-agent-validation", "src/real_agent_validation.rs"),
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
            "p0-bench-{name}-{}-{}-{}",
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
        for target in ALL {
            for decoy in target.decoys {
                let path = cwd.join(decoy);
                fs::create_dir_all(path.parent().expect("decoy parent"))
                    .expect("create decoy directory");
                fs::write(&path, "p0bench sentinel decoy: not a report or manifest\n")
                    .expect("write decoy");
            }
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

/// Synthetic values placed in the child's environment where the benchmarks
/// read their keys and settings; none is a real key. The proxy settings and
/// the Ollama address name a closed local port.
const ENV_SENTINELS: &[(&str, &str)] = &[
    ("GROQ_API_KEY", "p0bench-sentinel-groq-key"),
    ("NVIDIA_NIM_API_KEY", "nvapi-p0bench-sentinel"),
    ("DEEPSEEK_API_KEY", "p0bench-sentinel-deepseek-key"),
    ("MISTRAL_API_KEY", "p0bench-sentinel-mistral-key"),
    ("TOGETHER_API_KEY", "p0bench-sentinel-together-key"),
    ("FIREWORKS_API_KEY", "p0bench-sentinel-fireworks-key"),
    ("PERPLEXITY_API_KEY", "p0bench-sentinel-perplexity-key"),
    ("OPENROUTER_API_KEY", "p0bench-sentinel-openrouter-key"),
    ("OPENAI_API_KEY", "p0bench-sentinel-openai-key"),
    ("GEMINI_API_KEY", "p0bench-sentinel-gemini-key"),
    ("COHERE_API_KEY", "p0bench-sentinel-cohere-key"),
    ("OLLAMA_URL", "http://127.0.0.1:9/p0bench-sentinel-ollama"),
    ("OLLAMA_MODEL", "p0bench-sentinel-model"),
    ("NVIDIA_MODEL", "p0bench-sentinel-model"),
    ("NIM_MODELS", "1"),
    ("NIM_RATE_LIMIT", "1"),
    ("NIM_DETERMINISM_RUNS", "1"),
    ("NIM_CONCURRENCY", "1"),
    ("NEXUS_LONG_SESSION", "1"),
    ("NEXUS_SESSION_DURATION", "1"),
    ("HTTPS_PROXY", "http://127.0.0.1:9"),
    ("HTTP_PROXY", "http://127.0.0.1:9"),
    ("ALL_PROXY", "http://127.0.0.1:9"),
    ("NEXUS_CONFIG_KEY", "p0bench-sentinel-config-key"),
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
    assert!(!output.contains("p0bench"), "{context}: nothing is echoed");
    assert!(!output.contains("nvapi-"), "{context}: no key is echoed");
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

/// Every withdrawn benchmark, with no arguments, help and version words and
/// a stray argument, under an environment holding sentinel provider keys and
/// the settings its retired runs read, gets only its fixed denial. No report
/// is written, no decoy report or manifest changes, home stays empty, and no
/// key or other value is echoed.
#[test]
fn p0_bench_every_withdrawn_benchmark_invocation_is_withdrawn() {
    let fixture = Fixture::new("invocations");
    let cwd_before = snapshot(&fixture.cwd);
    for target in ALL {
        for args in INVOCATIONS {
            let args: Vec<OsString> = args.iter().map(OsString::from).collect();
            let outcome = run(&fixture, target, &args);
            assert_withdrawn(&outcome, target, &format!("{args:?}"));
        }
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

fn assert_entry_is_withdrawal(target: &Withdrawn) {
    let source = read(&manifest_dir().join("src").join(target.source));
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
        "nexus_kernel",
        "API_KEY",
        "Bearer",
        "authorization",
        "curl",
        "https://",
        "http://",
        "tokio",
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
        "src/{} must be exactly the withdrawal entry point (comments aside)",
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

/// Every withdrawn entry is exactly the withdrawal, and the package builds
/// exactly its listed binaries from `src/`: no library (which could host the
/// retired code for another target), no `src/bin`, examples, benches or
/// build script, and no `default-run`, auto-discovery switch, features or
/// `build` key. (Dependency lines are not examined.)
#[test]
fn p0_bench_withdrawn_entries_and_package_targets_are_pinned() {
    for target in ALL {
        assert_entry_is_withdrawal(target);
        assert!(!target.message.contains(J1_MESSAGE));
        assert!(
            PACKAGE_BINARIES.contains(&(target.name, &format!("src/{}", target.source))),
            "{} is a binary target of this package",
            target.name
        );
    }
    let dir = manifest_dir();
    let manifest = read(&dir.join("Cargo.toml"));
    let expected: Vec<(String, String)> = PACKAGE_BINARIES
        .iter()
        .map(|(name, path)| (name.to_string(), path.to_string()))
        .collect();
    assert_eq!(bin_targets(&manifest), expected);

    let mut sources: Vec<String> = PACKAGE_BINARIES
        .iter()
        .map(|(_, path)| path.trim_start_matches("src/").to_string())
        .collect();
    sources.sort();
    let mut listed: Vec<String> = fs::read_dir(dir.join("src"))
        .expect("list src")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    listed.sort();
    assert_eq!(listed, sources, "src holds exactly the binaries' sources");
    for extra in ["examples", "benches", "build.rs"] {
        assert!(!dir.join(extra).exists(), "{extra} must not exist");
    }
    let lines: Vec<&str> = manifest.lines().map(str::trim).collect();
    for forbidden in [
        "default-run",
        "autobins",
        "autoexamples",
        "autobenches",
        "build =",
        "[lib]",
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
