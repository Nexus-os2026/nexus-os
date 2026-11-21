//! SB-3E U21a (SB3E-R196, R197, R199; Step 0 section 3): production
//! `hands::run`, File mode, in this lane child on a pty the child owns,
//! driven by Owner lines only, held at the S7 `close_report` points. The
//! harness only arms and drains: every `close_report` entry is written by
//! production on the close worker (`hands.rs` `Runtime::close_report`).
//! Readiness is the File-mode Normal line (hands.rs `run`, R199 Q-U21-2),
//! never the bare ready literal. Test-only parts: the pty ownership.
use super::{child_with, Fixture, CHILD, PATIENCE};
use nexus_governed_control::broker::route_seams as s1;
use nexus_mother_host::route_seams as s7;
use nix::poll::{poll, PollFd, PollFlags};
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

/// Every wait of a child ends by this, before the 30 s child kill.
const BUDGET: Duration = Duration::from_secs(25);
/// The close deadline is Quit + 3 s (close.rs:99-111): a hold is released
/// within this (s5.rs `SHUTDOWN_PAUSE`).
const RELEASE: Duration = Duration::from_secs(1);
/// After the hold, how long a live reader may take to echo the probe byte.
const PROBE: Duration = Duration::from_millis(300);
/// hands.rs `run`: emitted only when `state() == Normal` in File mode.
pub(super) const NORMAL: &str =
    "One File mission per fresh process. Emergency ends mission work.\n";

/// The Owner's lines after readiness, each then `quit`.
pub(super) enum Owner {
    /// `quit` alone: the clean close (SealedFinalizing at the hold).
    Quit,
    /// `run`, its three codes, `stop`, `quit`: the seal is refused
    /// (ClosingRestricted at the hold, stage `seal`).
    RunStopQuit,
}
/// What the S7 arming at the point does.
pub(super) enum Hold {
    /// Acknowledge and block until released (within `RELEASE`).
    Held,
    /// Acknowledge and begin an unwind on the close worker.
    Unwind,
}
/// What one run left, before any trace is drained.
pub(super) struct Reached {
    pub(super) ack: Option<s7::Entry>,
    pub(super) run: ThreadId,
    pub(super) test: ThreadId,
    /// The journal file and its length at the acknowledgement.
    pub(super) journal: (PathBuf, usize),
    pub(super) transcript: String,
    pub(super) returned: Option<bool>,
    pub(super) findings: Vec<String>,
}

type Request = (Op, SyncSender<()>);
enum Op {
    Write(Vec<u8>),
    Close,
}
type Transcript = Arc<(Mutex<Vec<u8>>, Condvar)>;

/// The pty's slave is fd 0 and fd 1 (fd 2 is never touched); the drain
/// thread alone owns the one master fd (U18 review F3).
struct Tty {
    requests: SyncSender<Request>,
    transcript: Transcript,
    drain: Option<std::thread::JoinHandle<()>>,
    saved: Option<(OwnedFd, OwnedFd)>,
    start: Instant,
}
impl Tty {
    fn open() -> Self {
        std::io::stdout().flush().unwrap();
        let saved_in = std::io::stdin().as_fd().try_clone_to_owned().unwrap();
        let saved_out = std::io::stdout().as_fd().try_clone_to_owned().unwrap();
        let pty = nix::pty::openpty(None, None).unwrap();
        nix::unistd::dup2(pty.slave.as_raw_fd(), 0).unwrap();
        nix::unistd::dup2(pty.slave.as_raw_fd(), 1).unwrap();
        drop(pty.slave);
        let (requests, served) = sync_channel(4);
        let (master, transcript) = (pty.master, Transcript::default());
        let t = transcript.clone();
        let drain = std::thread::spawn(move || drain(master, served, t));
        Self {
            requests,
            transcript,
            drain: Some(drain),
            saved: Some((saved_in, saved_out)),
            start: Instant::now(),
        }
    }
    /// What is left of the child's budget, at most `PATIENCE`.
    fn left(&self) -> Duration {
        let left = (self.start + BUDGET).saturating_duration_since(Instant::now());
        left.min(PATIENCE)
    }
    fn request(&self, op: Op) -> bool {
        let (done, served) = sync_channel(1);
        let sent = self.requests.try_send((op, done)).is_ok();
        sent && served.recv_timeout(PATIENCE).is_ok()
    }
    fn write(&self, bytes: &[u8]) -> bool {
        self.request(Op::Write(bytes.to_vec()))
    }
    fn text(&self) -> String {
        let bytes = self.transcript.0.lock().unwrap();
        String::from_utf8_lossy(&bytes).replace("\r\n", "\n")
    }
    /// The offset just past `needle` in the text after `at`, waited for
    /// within `within` (bounded by the budget).
    fn wait_for(&self, at: usize, needle: &str, within: Duration) -> Option<usize> {
        let deadline = Instant::now() + within.min(self.left());
        let (lock, changed) = &*self.transcript;
        let mut seen = lock.lock().unwrap();
        loop {
            let text = String::from_utf8_lossy(&seen).replace("\r\n", "\n");
            if let Some(i) = text.get(at..).and_then(|t| t.find(needle)) {
                return Some(at + i + needle.len());
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            seen = changed.wait_timeout(seen, left).unwrap().0;
        }
    }
    /// After `hands::run` returned (its terminal finished): drained until
    /// one 100 ms quiet wait, bounded.
    fn settle(&self) {
        let deadline = Instant::now() + self.left();
        let (lock, changed) = &*self.transcript;
        let mut seen = lock.lock().unwrap();
        while Instant::now() < deadline {
            let before = seen.len();
            seen = changed
                .wait_timeout(seen, Duration::from_millis(100))
                .unwrap()
                .0;
            if seen.len() == before {
                return;
            }
        }
    }
}
fn drain(master: OwnedFd, served: Receiver<Request>, transcript: Transcript) {
    loop {
        while let Ok((op, done)) = served.try_recv() {
            let Op::Write(bytes) = op else {
                drop(master);
                let _ = done.send(());
                return;
            };
            let (mut written, deadline) = (0, Instant::now() + PATIENCE);
            while written < bytes.len() && Instant::now() < deadline {
                match nix::unistd::write(&master, &bytes[written..]) {
                    Ok(n) => written += n,
                    Err(_) => break,
                }
            }
            if written == bytes.len() {
                let _ = done.send(());
            }
        }
        let mut fds = [PollFd::new(master.as_fd(), PollFlags::POLLIN)];
        if !matches!(poll(&mut fds, 10u16), Ok(n) if n > 0) {
            continue;
        }
        let mut bytes = [0u8; 4096];
        match nix::unistd::read(master.as_raw_fd(), &mut bytes) {
            Ok(n) if n > 0 => {
                transcript.0.lock().unwrap().extend_from_slice(&bytes[..n]);
                transcript.1.notify_all();
            }
            _ => return,
        }
    }
}
/// fd 0 and fd 1 are restored before any libtest output, then the master is
/// dropped and the drain joined.
impl Drop for Tty {
    fn drop(&mut self) {
        let _ = std::io::stdout().flush();
        if let Some((input, output)) = self.saved.take() {
            let _ = nix::unistd::dup2(input.as_raw_fd(), 0);
            let _ = nix::unistd::dup2(output.as_raw_fd(), 1);
        }
        let _ = self.request(Op::Close);
        if let Some(drain) = self.drain.take() {
            let _ = drain.join();
        }
    }
}

/// A pause not released in time: recorded in the child's result, then abort.
fn timeout(line: &str) -> ! {
    let Some(root) = std::env::var_os(CHILD).map(PathBuf::from) else {
        std::process::abort()
    };
    let mut open = std::fs::OpenOptions::new();
    if let Ok(mut result) = open.create(true).append(true).open(root.join("result")) {
        let _ = writeln!(result, "{line}");
    }
    std::process::abort()
}

/// Answers one confirmation: its fresh code, then the code typed.
fn answer(tty: &Tty, at: usize, findings: &mut Vec<String>) -> usize {
    let Some(code_at) = tty.wait_for(at, "Answer code: ", PATIENCE) else {
        findings.push("owner: no answer code".into());
        return at;
    };
    let end = tty.wait_for(code_at, "\n", PATIENCE).unwrap_or(code_at);
    let code = tty
        .text()
        .get(code_at..end.saturating_sub(1))
        .unwrap_or("")
        .to_string();
    let ready = tty.wait_for(end, "INPUT READY\n", PATIENCE).unwrap_or(end);
    if code.len() != 8 || !tty.write(format!("{code}\r").as_bytes()) {
        findings.push(format!("owner: code {code:?} not typed"));
    }
    ready
}

/// `hands::run` in File mode on this child's pty: S1 traced and S7 armed at
/// `point` before it starts; readiness by the File Normal line; the Owner's
/// lines, then `quit`. At the acknowledgement the journal's length is taken,
/// one probe byte is typed (a live reader would read it and record
/// `host.read`) and the hold is released within `RELEASE`; then the run is
/// joined (bounded). Nothing is drained here.
pub(super) fn reached(f: &Fixture, owner: Owner, point: &'static str, hold: Hold) -> Reached {
    let mut findings = Vec::new();
    let workspace = f.0.join("workspace");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&workspace)
        .unwrap();
    let home = f.0.join("home");
    let journal = home.join("mother-journal.jsonl");
    let args: Vec<String> = vec![
        "--home".into(),
        home.to_string_lossy().into_owned(),
        "--workspace".into(),
        workspace.to_string_lossy().into_owned(),
        "--name".into(),
        "hello.txt".into(),
        "--text".into(),
        "U21a close report\n".into(),
    ];
    let tty = Tty::open();
    let traced = s1::arm("reg.reserve", s1::Action::Trace);
    drop((s1::drain_trace(), s7::drain_trace()));
    let held = match hold {
        Hold::Held => s7::arm(
            point,
            s7::Action::Pause {
                cell: "u21a",
                timeout,
            },
        ),
        Hold::Unwind => s7::arm(point, s7::Action::Panic),
    };
    let run = std::thread::Builder::new()
        .name("u21a-hands-run".into())
        .spawn(move || nexus_mother_host::hands::run(args.into_iter()))
        .unwrap();
    let ids = (run.thread().id(), std::thread::current().id());
    let mut at = match tty.wait_for(0, NORMAL, PATIENCE) {
        Some(at) => at,
        None => {
            findings.push("ready: no File Normal line".into());
            0
        }
    };
    if let Owner::RunStopQuit = owner {
        let typed = tty.write(b"run\r");
        for _ in 0..3 {
            at = answer(&tty, at, &mut findings);
        }
        at = tty.wait_for(at, "FILE PENDING\n", PATIENCE).unwrap_or(at);
        let stopped = tty.write(b"stop\r");
        let done = tty.wait_for(at, "STOP RECORDING COMPLETED", PATIENCE);
        if !typed || !stopped || done.is_none() {
            findings.push("owner: run and stop not completed".into());
        }
        at = done.unwrap_or(at);
    }
    let mark = at;
    if !tty.write(b"quit\r") {
        findings.push("owner: quit not typed".into());
    }
    let ack = held.acknowledged.recv_timeout(tty.left()).ok();
    let acked = Instant::now();
    let length = journal_length(&journal);
    if ack.is_some() && matches!(hold, Hold::Held) {
        // A live reader echoes the byte and records `host.read` (S7 armed).
        let echo = tty.text().len().max(mark);
        let _ = tty.write(b"x");
        let _ = tty.wait_for(echo, "x", PROBE.min(RELEASE / 2));
    }
    drop(held);
    if ack.is_some() && acked.elapsed() >= RELEASE {
        findings.push(format!(
            "report: released after {:?}, not within {RELEASE:?}",
            acked.elapsed()
        ));
    }
    let deadline = Instant::now() + tty.left();
    while !run.is_finished() && Instant::now() < deadline {
        let _ = tty.wait_for(usize::MAX, "\0", Duration::from_millis(5));
    }
    let returned = match run.is_finished() {
        true => run.join().ok(),
        false => {
            findings.push("run: hands::run did not return".into());
            None
        }
    };
    tty.settle();
    let transcript = tty.text();
    drop(tty);
    drop(traced);
    let (close, clean) = match owner {
        Owner::Quit => ("SESSION CLOSED recorded\n", true),
        Owner::RunStopQuit => ("CLOSE FAILED stage=seal\n", false),
    };
    let unwound = matches!(hold, Hold::Unwind);
    if !unwound && (!transcript.contains(close) || returned != Some(clean)) {
        findings.push(format!(
            "owner: no {close:?} (hands::run returned {returned:?})"
        ));
    }
    Reached {
        ack,
        run: ids.0,
        test: ids.1,
        journal: (journal, length),
        transcript,
        returned,
        findings,
    }
}
/// The journal's entries now (0 if it cannot be read).
fn journal_length(path: &Path) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().filter(|l| !l.is_empty()).count()
}

/// U21a control: unarmed, no `close_report` point records anything.
#[test]
fn route_report_unarmed_records_nothing() {
    child_with(
        "report::route_report_unarmed_records_nothing",
        |_| Vec::new(),
        |f| {
            let workspace = f.0.join("workspace");
            let mut dir = std::fs::DirBuilder::new();
            dir.mode(0o700).create(&workspace).unwrap();
            let home = f.0.join("home");
            let args: Vec<String> = vec![
                "--home".into(),
                home.to_string_lossy().into_owned(),
                "--workspace".into(),
                workspace.to_string_lossy().into_owned(),
                "--name".into(),
                "hello.txt".into(),
                "--text".into(),
                "U21a unarmed\n".into(),
            ];
            let tty = Tty::open();
            let run = std::thread::spawn(move || nexus_mother_host::hands::run(args.into_iter()));
            let ready = tty.wait_for(0, NORMAL, PATIENCE);
            let typed = tty.write(b"quit\r");
            let deadline = Instant::now() + tty.left();
            while !run.is_finished() && Instant::now() < deadline {
                let _ = tty.wait_for(usize::MAX, "\0", Duration::from_millis(5));
            }
            let returned = run.is_finished().then(|| run.join().ok()).flatten();
            tty.settle();
            drop(tty);
            let trace = s7::drain_trace();
            assert!(ready.is_some() && typed, "unarmed: not ready");
            assert_eq!(returned, Some(true), "unarmed: no clean close");
            assert!(trace.is_empty(), "unarmed: {} entries", trace.len());
        },
    );
}

/// U21a (SB3E-R140 Q3; x:763 not credited, R199 Q-U21-3): S7 `Panic` at
/// `host.close_report` unwinds the close worker inside `close_report`: the
/// seal drops unfinished (close.rs:249-262), the fallback report is
/// delivered (hands.rs `close_fallback_report`), `finish_delivery` never
/// runs (no S1 `close.delivery`) and `hands::run` returns false.
#[test]
fn route_report_panic_unwinds_the_close_worker() {
    child_with(
        "report::route_report_panic_unwinds_the_close_worker",
        |_| Vec::new(),
        |f| {
            let r = reached(f, Owner::Quit, "host.close_report", Hold::Unwind);
            let (broker, host) = (s1::drain_trace(), s7::drain_trace());
            let ack = r
                .ack
                .clone()
                .expect("panic: no host.close_report acknowledgement");
            let after = |p: &str| host.iter().any(|e| e.point == p && e.tick > ack.tick);
            let delivered = broker.iter().any(|e| e.point == "close.delivery");
            let fallback =
                "CLOSE OBLIGATIONS resource_snapshot=unknown in_flight_or_unknown=true\n";
            println!("WITNESS panic: stage {:?} state {:?}", ack.stage, ack.state);
            assert!(
                ack.thread != r.run && ack.thread != r.test,
                "panic: off the close worker"
            );
            assert_eq!(r.returned, Some(false), "panic: hands::run returned");
            assert!(
                !after("host.close_report.resources"),
                "panic: the report went on"
            );
            assert!(!delivered, "panic: finish_delivery ran");
            assert!(r.transcript.contains(fallback), "panic: no fallback report");
            assert!(
                r.transcript.contains("MOTHER END Recorded\n"),
                "panic: mother end"
            );
            assert_eq!(
                ack.stage.as_deref(),
                Some("delivery_pending"),
                "panic: stage"
            );
        },
    );
}

/// U21a: an unreleased S7 pause at `host.close_report` is the harness's
/// irrevocable failure: `TIMEOUT u21a host.close_report` in the child's
/// result, then the child aborts (as s5.rs's S5 pause test).
#[test]
fn route_report_unreleased_hold_times_out_and_aborts() {
    let name = "report::route_report_unreleased_hold_times_out_and_aborts";
    if let Some(root) = std::env::var_os(CHILD) {
        let f = Fixture(PathBuf::from(root));
        let tty = Tty::open();
        let _traced = s1::arm("reg.reserve", s1::Action::Trace);
        let held = s7::arm(
            "host.close_report",
            s7::Action::Pause {
                cell: "u21a",
                timeout,
            },
        );
        let workspace = f.0.join("workspace");
        let mut dir = std::fs::DirBuilder::new();
        dir.mode(0o700).create(&workspace).unwrap();
        let args: Vec<String> = vec![
            "--home".into(),
            f.0.join("home").to_string_lossy().into_owned(),
            "--workspace".into(),
            workspace.to_string_lossy().into_owned(),
            "--name".into(),
            "hello.txt".into(),
            "--text".into(),
            "U21a unreleased\n".into(),
        ];
        let _run = std::thread::spawn(move || nexus_mother_host::hands::run(args.into_iter()));
        let _ = tty.wait_for(0, NORMAL, PATIENCE);
        let _ = tty.write(b"quit\r");
        let acked = held.acknowledged.recv_timeout(PATIENCE).is_ok();
        // Never released: the S7 patience (10 s) ends this process first.
        let (_keep, never) = sync_channel::<()>(1);
        let _ = never.recv_timeout(PATIENCE + PATIENCE);
        drop((held, tty));
        panic!("unreleased: not aborted (acknowledged {acked})");
    }
    let root = std::env::temp_dir().join(format!(
        "sb3e-route-{}-report-unreleased",
        std::process::id()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let root = Fixture(root);
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, &root.0)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            process.kill().unwrap();
        }
        std::thread::sleep(Duration::from_millis(5)); // Polls an exit only.
    }
    let out = process.wait_with_output().unwrap();
    let result = std::fs::read_to_string(root.0.join("result")).unwrap_or_default();
    use std::os::unix::process::ExitStatusExt;
    assert_eq!(
        out.status.signal(),
        Some(6),
        "unreleased: not aborted: {out:?}"
    );
    assert_eq!(
        result, "TIMEOUT u21a host.close_report\n",
        "unreleased: result"
    );
}
