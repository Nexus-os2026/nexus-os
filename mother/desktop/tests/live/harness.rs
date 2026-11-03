//! The live GTK harness (design §7.1, Finding 7), shared by every live test.
//!
//! - **Isolation.** A live test re-runs itself alone as a child process.
//!   The parent starts its own `Xvfb` (`-displayfd`) and builds the child's
//!   environment from nothing: `GDK_BACKEND=x11`, `DISPLAY` = that display,
//!   `GSK_RENDERER=cairo`, a fresh `HOME` and `XDG_RUNTIME_DIR`. Nothing
//!   inherited reaches it: no `WAYLAND_*`, `XDG_SESSION_TYPE`, `GTK_DEBUG`
//!   (except where a test sets it) or `DBUS_SESSION_BUS_ADDRESS` (except
//!   G13, on its own private session bus: a `dbus-daemon` the harness
//!   starts and owns, as `dbus-run-session` does, so that services the bus
//!   activates write to the daemon's pipe, never into the child's report).
//! - **The child asserts its actual display** (`support::launch_isolated`).
//! - **Every subprocess is bounded.** The child runs in its own process
//!   group, which the parent kills on overrun. A separate watchdog fails a
//!   step that prints no progress within [`STEP`].
//! - **No skip.** A missing `Xvfb`, `xdotool` or `dbus-daemon` fails.
//! - **The report is read strictly:** one test, `ok`.
//! - **H1:** the parent itself may run with hostile display variables; a
//!   decoy display and a decoy Wayland socket record any client.

#![allow(dead_code)]

use crate::process_leader::{Leader, REAP as LEADER_REAP};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The child's role variable: the name of the test it runs.
pub const CHILD: &str = "NEXUS_DESKTOP_LIVE_CHILD";
/// The display the child must find itself on.
pub const EXPECTED_DISPLAY: &str = "NEXUS_DESKTOP_LIVE_DISPLAY";
/// H1's middle role: a parent started with hostile variables.
const HOSTILE: &str = "NEXUS_DESKTOP_LIVE_HOSTILE";

/// Longest silence between two progress lines of a child (the watchdog).
pub const STEP: Duration = Duration::from_secs(10);
/// Longest life of a whole child.
pub const WHOLE: Duration = Duration::from_secs(120);

/// How a live test runs.
#[derive(Clone, Default)]
pub struct Opts {
    /// Run the child on the harness's own Xvfb.
    pub display: bool,
    /// Run the child on its own private session bus (G13).
    pub dbus: bool,
    /// Extra variables for the child.
    pub env: Vec<(&'static str, String)>,
    /// H1: run the parent itself with hostile display variables.
    pub hostile: bool,
}

/// Run `body` as the live test `name` (the `#[test]` function's name).
pub fn live(name: &str, opts: Opts, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        progress("child started");
        body();
        progress("child done");
        return;
    }
    if opts.hostile && std::env::var(HOSTILE).as_deref() != Ok(name) {
        return hostile_parent(name, opts);
    }
    parent(name, &opts);
}

/// One progress line for the parent's watchdog. It goes to stderr, so it
/// never splits libtest's `test <name> ... ok` line on stdout.
pub fn progress(step: &str) {
    eprintln!("LIVE-STEP {step}");
}

/// A fresh private directory.
pub fn fresh_dir(label: &str) -> PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "nexus-desktop-live-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .unwrap();
    dir
}

/// A directory searched before the usual places (decision 4, LOW 3): the
/// live tests' own seam, so a plant can sit exactly where `tool` resolves.
pub const TOOL_DIR: &str = "NEXUS_DESKTOP_TOOL_DIR";

/// The tool at its usual place; a missing tool fails the test.
fn tool(name: &str) -> PathBuf {
    if let Some(dir) = std::env::var_os(TOOL_DIR) {
        let path = Path::new(&dir).join(name);
        if path.is_file() {
            return path;
        }
    }
    for dir in ["/usr/bin", "/bin", "/usr/local/bin"] {
        let path = Path::new(dir).join(name);
        if path.is_file() {
            return path;
        }
    }
    panic!("{name} is required for the live tests and was not found");
}

/// An Xvfb of our own: its display number, its child, its log.
pub struct Xvfb {
    pub display: String,
    leader: Leader,
    log: PathBuf,
}

impl Xvfb {
    /// Start one; `audit` records every client connection in its log.
    pub fn start(audit: bool) -> Self {
        let log = fresh_dir("xvfb").join("xvfb.log");
        let mut command = Command::new(tool("Xvfb"));
        command.args([
            "-displayfd",
            "1",
            "-nolisten",
            "tcp",
            "-noreset",
            "-screen",
            "0",
            "1280x800x24",
        ]);
        if audit {
            command.args(["-audit", "4"]);
        }
        let mut leader = Leader::spawn(
            command
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(std::fs::File::create(&log).unwrap()),
        )
        .expect("Xvfb starts");
        let stdout = leader.take_stdout().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line);
        });
        let display = match rx.recv_timeout(STEP) {
            Ok(line) if !line.trim().is_empty() => format!(":{}", line.trim()),
            _ => {
                let ended = leader.finish(LEADER_REAP);
                panic!("Xvfb printed no display within {STEP:?} (its cleanup: {ended:?})");
            }
        };
        Self {
            display,
            leader,
            log,
        }
    }

    /// The clients that connected, from the audit log.
    pub fn connections(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.contains(" connected"))
            .map(str::to_string)
            .collect()
    }
}

impl Xvfb {
    /// End it: the held leader's one final cleanup. The test asserts the
    /// result; dropping it unshut leaves the cleanup to the leader's
    /// backstop, which records any failure.
    pub fn shutdown(self) -> Result<(), String> {
        self.leader.finish(LEADER_REAP).map(|_| ())
    }
}

/// A private session bus of our own (G13): the `dbus-daemon` that
/// `dbus-run-session` would start, in its own process group, killed with
/// everything it activated when the test ends.
pub struct PrivateBus {
    pub address: String,
    leader: Leader,
}

impl PrivateBus {
    pub fn start() -> Self {
        let mut leader = Leader::spawn(
            Command::new(tool("dbus-daemon"))
                .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
                .env_clear()
                .env("PATH", "/usr/local/bin:/usr/bin:/bin")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
        )
        .expect("dbus-daemon starts");
        let stdout = leader.take_stdout().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut lines = BufReader::new(stdout).lines();
            if let Some(Ok(first)) = lines.next() {
                let _ = tx.send(first);
            }
            // Keep draining: activated services write here.
            for _ in lines {}
        });
        match rx.recv_timeout(STEP) {
            Ok(address) if !address.trim().is_empty() => Self {
                address: address.trim().to_string(),
                leader,
            },
            _ => {
                let ended = leader.finish(LEADER_REAP);
                panic!("dbus-daemon printed no address within {STEP:?} (its cleanup: {ended:?})");
            }
        }
    }
}

impl PrivateBus {
    /// End it, with everything it activated: the held leader's one final
    /// cleanup. The test asserts the result.
    pub fn shutdown(self) -> Result<(), String> {
        self.leader.finish(LEADER_REAP).map(|_| ())
    }
}

/// The variables every child gets; nothing else is inherited.
fn base_env(display: Option<&str>) -> Vec<(String, String)> {
    let home = fresh_dir("home");
    let runtime = fresh_dir("runtime");
    let mut env = vec![
        (
            "PATH".to_string(),
            "/usr/local/bin:/usr/bin:/bin".to_string(),
        ),
        ("HOME".to_string(), home.display().to_string()),
        ("XDG_RUNTIME_DIR".to_string(), runtime.display().to_string()),
        ("LANG".to_string(), "C.UTF-8".to_string()),
        ("GDK_BACKEND".to_string(), "x11".to_string()),
        ("GSK_RENDERER".to_string(), "cairo".to_string()),
        ("RUST_BACKTRACE".to_string(), "1".to_string()),
    ];
    if let Some(display) = display {
        env.push(("DISPLAY".to_string(), display.to_string()));
        env.push((EXPECTED_DISPLAY.to_string(), display.to_string()));
    }
    env
}

/// How long a killed child may take to be reaped.
const REAP: Duration = Duration::from_secs(10);

/// Kill `child` (not its group: a tool the tests run directly) and reap
/// it, the whole cleanup within `REAP`; its exit status, if it was reaped.
/// A kill that fails for a child that has not exited fails the test.
pub fn kill_and_reap(child: &mut Child) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + REAP;
    if let Err(e) = child.kill() {
        if !matches!(child.try_wait(), Ok(Some(_))) {
            panic!("killing child {} failed: {e}", child.id());
        }
    }
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => return None,
        }
    }
}

#[allow(unused_imports)]
pub use crate::process_leader::{checked_pgid, signal_outcome};

/// What supervising one owned child gave: its stdout and stderr lines, its
/// exit status once reaped, and why it was killed, if it was.
pub struct Supervised {
    pub out: Vec<String>,
    pub err: Vec<String>,
    pub status: Option<std::process::ExitStatus>,
    pub overrun: Option<String>,
    /// The held leader's final cleanup: its signal and reap.
    pub cleanup: Result<(), String>,
}

/// Supervise the held `leader` (stdout and stderr piped): read both
/// streams line by line, any line counting as progress, until it exits
/// (observed without reaping) or no line comes within `step` or the whole
/// run passes `whole`; then end it with its one final cleanup, its group's
/// signal and its reap.
pub fn supervise(mut leader: Leader, step: Duration, whole: Duration) -> Supervised {
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let stdout = leader.take_stdout().unwrap();
    let stderr = leader.take_stderr().unwrap();
    for (is_out, stream) in [
        (true, Box::new(stdout) as Box<dyn Read + Send>),
        (false, Box::new(stderr) as Box<dyn Read + Send>),
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if tx.send((is_out, line)).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);
    let started = Instant::now();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut last = Instant::now();
    let overrun = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((is_out, line)) => {
                last = Instant::now();
                if is_out {
                    out.push(line);
                } else {
                    err.push(line);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if last.elapsed() > step {
            break Some(format!("no progress within {step:?}"));
        }
        if started.elapsed() > whole {
            break Some(format!("overran {whole:?}"));
        }
    };
    // The streams closed (or a reader failed): the process itself is still
    // supervised, under the same deadline, observed without reaping it.
    let mut overrun = overrun;
    if overrun.is_none() {
        loop {
            match leader.exited() {
                Ok(true) => break,
                Ok(false) if started.elapsed() <= whole => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(false) => {
                    overrun = Some(format!("its output closed, but it ran past {whole:?}"));
                    break;
                }
                Err(e) => {
                    overrun = Some(format!("observing it failed: {e}"));
                    break;
                }
            }
        }
    }
    // The one final cleanup, always.
    let (status, cleanup) = match leader.finish(LEADER_REAP) {
        Ok(status) => (Some(status), Ok(())),
        Err(e) => (None, Err(e)),
    };
    Supervised {
        out,
        err,
        status,
        overrun,
        cleanup,
    }
}

/// Spawn `command` (stdout and stderr piped) in its own process group and
/// supervise it as `supervise` does.
pub fn supervise_command(command: &mut Command, step: Duration, whole: Duration) -> Supervised {
    let leader = Leader::spawn(command.stdout(Stdio::piped()).stderr(Stdio::piped()))
        .expect("the supervised child starts");
    supervise(leader, step, whole)
}

/// Run the child for `name` and read its report strictly.
fn parent(name: &str, opts: &Opts) {
    let xvfb = opts.display.then(|| Xvfb::start(false));
    let exe = std::env::current_exe().unwrap();
    let test_args = [name, "--exact", "--nocapture", "--test-threads=1"];
    let bus = opts.dbus.then(PrivateBus::start);
    let mut command = Command::new(&exe);
    command.args(test_args);
    command.env_clear();
    if let Some(bus) = &bus {
        command.env("DBUS_SESSION_BUS_ADDRESS", &bus.address);
    }
    for (key, value) in base_env(xvfb.as_ref().map(|x| x.display.as_str())) {
        command.env(key, value);
    }
    for (key, value) in &opts.env {
        command.env(key, value);
    }
    command.env(CHILD, name);
    let leader = Leader::spawn(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .expect("the live child starts");
    let supervised = supervise(leader, STEP, WHOLE);
    if let Err(e) = &supervised.cleanup {
        panic!("live child {name}: its cleanup failed: {e}");
    }
    let (out, err_lines, overrun) = (supervised.out, supervised.err, supervised.overrun);
    let status = supervised.status.expect("the live child's status");
    let err = err_lines.join("\n");
    let report = out.join("\n");
    let context = format!("--- stdout ---\n{report}\n--- stderr ---\n{err}");
    if let Some(why) = overrun {
        panic!("live child {name}: {why}\n{context}");
    }
    let ok_line = format!("test {name} ... ok");
    let ran: Vec<&String> = out
        .iter()
        .filter(|l| l.starts_with("test ") && !l.starts_with("test result:"))
        .collect();
    assert!(
        status.success()
            && ran.len() == 1
            && out.contains(&ok_line)
            && out
                .iter()
                .any(|l| l.starts_with("test result: ok. 1 passed;")),
        "live child {name} did not report exactly one ok test ({status})\n{context}"
    );
    if let Some(bus) = bus {
        bus.shutdown().expect("the private bus shuts down");
    }
    if let Some(xvfb) = xvfb {
        xvfb.shutdown().expect("Xvfb shuts down");
    }
    assert_eq!(
        crate::process_leader::backstop_failures(),
        Vec::<String>::new(),
        "a cleanup backstop failed"
    );
}

/// H1: run the normal parent for `name` with hostile display variables
/// inherited, a decoy display and a decoy Wayland socket that record any
/// client, then require that nothing reached them.
fn hostile_parent(name: &str, opts: Opts) {
    let decoy = Xvfb::start(true);
    let runtime = fresh_dir("decoy-runtime");
    let wayland = UnixListener::bind(runtime.join("wayland-0")).unwrap();
    wayland.set_nonblocking(true).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut middle = Leader::spawn(
        Command::new(&exe)
            .args([name, "--exact", "--nocapture", "--test-threads=1"])
            .env(HOSTILE, name)
            .env("WAYLAND_DISPLAY", "wayland-0")
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("DISPLAY", &decoy.display)
            .env("GDK_BACKEND", "wayland")
            .env("XDG_SESSION_TYPE", "wayland")
            .env("GTK_DEBUG", "interactive")
            .env_remove(CHILD)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )
    .expect("the hostile parent starts");
    let mut middle_out = middle.take_stdout().unwrap();
    let mut middle_err = middle.take_stderr().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = middle_out.read_to_string(&mut text);
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = middle_err.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + WHOLE + STEP;
    let overran = loop {
        if middle.exited().expect("observing the hostile parent") {
            break false;
        }
        if Instant::now() > deadline {
            break true;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // The one final cleanup of the held middle's group.
    let status = middle
        .finish(LEADER_REAP)
        .expect("the hostile parent's cleanup");
    assert!(!overran, "the hostile parent overran");
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    let ok_line = format!("test {name} ... ok");
    assert!(
        status.success() && out.lines().any(|l| l == ok_line),
        "the hostile parent failed ({status})\n{out}\n{err}"
    );
    assert_eq!(
        decoy.connections(),
        Vec::<String>::new(),
        "a client reached the inherited display"
    );
    let reached = wayland.accept().is_ok();
    assert!(!reached, "a client reached the inherited Wayland socket");
    decoy.shutdown().expect("the decoy Xvfb shuts down");
    assert_eq!(
        crate::process_leader::backstop_failures(),
        Vec::<String>::new(),
        "a cleanup backstop failed"
    );
    let _ = opts;
}
