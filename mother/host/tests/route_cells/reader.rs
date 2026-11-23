//! SB-3E U18 (B4-B7; design 4.2 S6, S7 and DA-S7R, 4.3): the production
//! `Terminal::open` reader on a pty this child owns, over the composed
//! session, bound like production. Each test is one lane child. The harness
//! only arms and drains: a `host.read` acknowledgement is written by the
//! reader thread itself after it has processed the value, so a press faked
//! through the public ingress cannot satisfy `read_ack` (des:474).
//! Test-only parts: the pty ownership, the read shim and the fake peer.
//! SIGHUP and controlling-terminal behaviour are not modelled.
use super::{child_with, fake_ollama, spec, Fixture, CHILD, PATIENCE};
use nexus_governed_control::broker::route_seams as s1;
use nexus_governed_control::host_session::{HostSession, SessionIngress, SessionState};
use nexus_mother_host::route_seams::{self as s7, Branch};
use nexus_mother_host::terminal::Terminal;
use nix::poll::{poll, PollFd, PollFlags};
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// F-S0-6: every wait of a child ends by this, before child_with's 30 s kill.
const BUDGET: Duration = Duration::from_secs(25);
/// A request to the drain thread and its bounded acknowledgement.
type Request = (Op, SyncSender<()>);
enum Op {
    Write(Vec<u8>),
    Close,
}
type Transcript = Arc<(Mutex<Vec<u8>>, Condvar)>;

/// One child's clock: each wait gets at most PATIENCE and never past the
/// child's end; a spent budget panics naming the phase. Phases go to fd 2.
pub(super) struct Clock {
    start: Instant,
    phase: Mutex<&'static str>,
}
impl Clock {
    pub(super) fn new() -> Self {
        let phase = Mutex::new("start");
        Self {
            start: Instant::now(),
            phase,
        }
    }
    fn at(&self, phase: &'static str) {
        *self.phase.lock().unwrap() = phase;
        eprintln!("U18PHASE {phase} {}ms", self.start.elapsed().as_millis());
    }
    pub(super) fn left(&self) -> Duration {
        let left = (self.start + BUDGET).saturating_duration_since(Instant::now());
        let phase = *self.phase.lock().unwrap();
        assert!(!left.is_zero(), "reader: deadline in phase {phase}");
        left.min(PATIENCE)
    }
}

/// B4: the pty's slave is fd 0 and fd 1 (fd 2 is never touched). The drain
/// thread alone owns the one master fd and serves write and close requests
/// between its bounded polls (review F3: no caller waits behind a poll).
pub(super) struct Tty {
    requests: SyncSender<Request>,
    transcript: Transcript,
    drain: Option<std::thread::JoinHandle<()>>,
    saved: Option<(OwnedFd, OwnedFd)>,
}
impl Tty {
    pub(super) fn open() -> Self {
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
        }
    }
    /// One request, served by the drain thread; its acknowledgement bounded.
    fn request(&self, op: Op) -> bool {
        let (done, served) = sync_channel(1);
        let sent = self.requests.try_send((op, done)).is_ok();
        sent && served.recv_timeout(PATIENCE).is_ok()
    }
    fn write(&self, bytes: &[u8]) {
        assert!(self.request(Op::Write(bytes.to_vec())), "bounded pty write");
    }
    /// The drain thread drops the only master fd.
    fn close_master(&self) {
        assert!(self.request(Op::Close), "bounded master close");
    }
    pub(super) fn len(&self) -> usize {
        self.transcript.0.lock().unwrap().len()
    }
    pub(super) fn since(&self, at: usize) -> Vec<u8> {
        self.transcript.0.lock().unwrap()[at..].to_vec()
    }
    fn wait_for(&self, clock: &Clock, at: usize, needle: &[u8]) -> bool {
        let deadline = Instant::now() + clock.left();
        let (lock, changed) = &*self.transcript;
        let mut seen = lock.lock().unwrap();
        loop {
            if seen[at..].windows(needle.len()).any(|w| w == needle) {
                return true;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            seen = changed.wait_timeout(seen, left).unwrap().0;
        }
    }
    /// After `Terminal::finish` (every queued frame written): drained until
    /// one 100 ms quiet poll, bounded.
    pub(super) fn settle(&self, clock: &Clock) {
        let deadline = Instant::now() + clock.left();
        let (lock, changed) = &*self.transcript;
        let mut seen = lock.lock().unwrap();
        while Instant::now() < deadline {
            let before = seen.len();
            let quiet = Duration::from_millis(100);
            seen = changed.wait_timeout(seen, quiet).unwrap().0;
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
/// Runs after `Terminal::finish`: fd 0 and fd 1 are restored before any
/// libtest output, then the master is dropped and the drain joined.
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

/// B5: the fixture's session; production's terminal over its ingress, bound
/// by S6; then production's bootstrap order (hold, workspace, model,
/// readiness). Ready iff the state is Normal (N4: never a printed line).
/// `terminal` is declared before `tty`: the terminal and its reader drop
/// before the fds are restored (review N4).
struct Reader {
    terminal: Terminal,
    tty: Tty,
    session: Arc<HostSession>,
    ingress: SessionIngress,
    clock: Clock,
}
fn compose(f: &Fixture) -> Reader {
    let clock = Clock::new();
    clock.at("compose");
    let o = f.open();
    let tty = Tty::open();
    let terminal = Terminal::open(o.ingress.clone()).expect("Terminal::open on the pty");
    let session = o.session.clone();
    assert!(terminal.route_bind(session.clone()), "S6: binds once");
    let workspace = f.0.join("workspace");
    let mut dir = std::fs::DirBuilder::new();
    dir.mode(0o700).create(&workspace).unwrap();
    session.bootstrap_hold(Duration::ZERO).unwrap();
    session.bootstrap_workspace(&workspace, &[]).unwrap();
    let (endpoint, _) = fake_ollama();
    session.register_local_model(&endpoint, spec()).unwrap();
    session.finish_bootstrap().unwrap();
    assert_eq!(session.state(), SessionState::Normal, "ready");
    clock.at("ready");
    let ingress = o.ingress;
    Reader {
        tty,
        terminal,
        session,
        ingress,
        clock,
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

/// One chunk's barrier: S7 armed to pause before `apply`, its acknowledgement
/// received (bounded), then released.
fn acked(clock: &Clock, apply: impl FnOnce()) -> Option<s7::Entry> {
    let held = s7::arm(
        "host.read",
        s7::Action::Pause {
            cell: "u18",
            timeout,
        },
    );
    apply();
    let ack = held.acknowledged.recv_timeout(clock.left()).ok();
    drop(held);
    ack
}

/// B7: the value's chunks, each `(n, eot, branch)` as the reader must record.
fn chunks(value: &str) -> Vec<(usize, bool, Branch)> {
    match value {
        "0d" | "0a" => vec![(1, false, Branch::Data)],
        "0d0a" => vec![(2, false, Branch::Data)],
        // U19a (SB3E-R196.2): the typed `stop` line, ended by CR, LF or CRLF.
        "stop0d" | "stop0a" => vec![(5, false, Branch::Data)],
        "stop0d0a" => vec![(6, false, Branch::Data)],
        "04" => vec![(1, true, Branch::Data)],
        "partial+04" => vec![(4, false, Branch::Data), (1, true, Branch::Data)],
        "master_close" => vec![(0, false, Branch::Zero)],
        "read_failure" => vec![(0, false, Branch::Err)],
        _ => Vec::new(),
    }
}

/// B7: each chunk has exactly one S7 `host.read` acknowledgement after
/// `since`, equal to it. A press value's S1 `ingress.emergency` is the
/// reader's own: on the last acknowledgement's thread, not after it. A
/// terminator echoes exactly one CRLF and reaches no ingress; a `stop` line
/// (U19a, F-S0-2) echoes `stop` CRLF and is a press value. Nothing here
/// accepts a command. Findings are `read: ...`.
pub(super) fn read_ack(
    value: &str,
    acks: &[s7::Entry],
    entries: &[s1::Entry],
    echo: &[u8],
    since: Instant,
) -> Result<Vec<s7::Entry>, String> {
    let want = chunks(value);
    let acks: Vec<s7::Entry> = acks
        .iter()
        .filter(|e| e.point == "host.read" && e.instant >= since)
        .cloned()
        .collect();
    if want.is_empty() {
        return Err(format!("read: unknown value {value}"));
    }
    if acks.is_empty() {
        return Err("read: no host.read acknowledgement".into());
    }
    if acks.len() != want.len() {
        return Err(format!("read: {} acknowledgements", acks.len()));
    }
    for (a, (n, eot, branch)) in acks.iter().zip(&want) {
        if a.eot != *eot {
            return Err(format!("read: eot {}", a.eot));
        }
        // F2 (SB3E-R146): after the master closes, a slave read returns 0 or
        // EIO; both are the reader's end arms.
        let end = value == "master_close" && a.branch == Branch::Err;
        if a.branch != *branch && !end {
            return Err(format!("read: branch {:?}, not {branch:?}", a.branch));
        }
        if a.n != *n {
            return Err(format!("read: n {}, not {n}", a.n));
        }
    }
    let ingress: Vec<&s1::Entry> = entries
        .iter()
        .filter(|e| e.point.starts_with("ingress.") && e.instant >= since)
        .collect();
    if ingress.iter().any(|e| e.point == "ingress.accept") {
        return Err("read: an ingress.accept".into());
    }
    if matches!(value, "0d" | "0a" | "0d0a") {
        return match (ingress.is_empty(), echo == b"\r\n") {
            (true, true) => Ok(acks),
            (false, _) => Err(format!("read: {} ingress entries", ingress.len())),
            (true, false) => Err(format!("read: echo {:?}", String::from_utf8_lossy(echo))),
        };
    }
    let line = echo.windows(6).any(|w| w == b"stop\r\n");
    if value.starts_with("stop") && !line {
        return Err(format!("read: echo {:?}", String::from_utf8_lossy(echo)));
    }
    let last = &acks[acks.len() - 1];
    match ingress[..] {
        [] => Err("read: no ingress.emergency".into()),
        [e] if e.thread != last.thread => {
            Err("read: ingress.emergency not on the reader thread".into())
        }
        [e] if e.instant > last.instant => {
            Err("read: ingress.emergency after the acknowledgement".into())
        }
        [_] => Ok(acks),
        _ => Err(format!("read: {} ingress entries", ingress.len())),
    }
}

/// U19b-1 (SB3E-R214 F-S0-B, R216 F7; closes U19a's R202 C1 and C3): a byte
/// row's value admits exactly one reader value, the typed `stop` line for
/// CR, LF and CRLF (R196.2); a bare terminator or any other value is refused.
pub(super) fn tied(row: &str, reader: &str) -> Result<(), String> {
    let want = match row {
        "CR" => "stop0d",
        "LF" => "stop0a",
        "CRLF" => "stop0d0a",
        "EOF" => "04",
        "partial_Ctrl_D" => "partial+04",
        "master_close" => "master_close",
        "read_failure" => "read_failure",
        _ => return Err(format!("read: {row} is not a byte value")),
    };
    match reader == want {
        true => Ok(()),
        false => Err(format!("read: reader value {reader}, not {want}")),
    }
}

/// What one applied value left: the acknowledgements received, the drained
/// traces, the echo after the mark and the session state.
struct Applied {
    acks: Vec<Option<s7::Entry>>,
    trace: Vec<s7::Entry>,
    entries: Vec<s1::Entry>,
    echo: Vec<u8>,
    state: SessionState,
    since: Instant,
}
fn apply(f: &Fixture, drive: impl FnOnce(&Reader) -> Vec<Option<s7::Entry>>) -> Applied {
    let mut r = compose(f);
    let _s1 = s1::arm("ingress.emergency", s1::Action::Trace);
    drop((s1::drain_trace(), s7::drain_trace()));
    let (since, mark) = (Instant::now(), r.tty.len());
    r.clock.at("apply");
    let acks = drive(&r);
    let state = r.session.state();
    r.clock.at("finish");
    let deadline = Instant::now() + r.clock.left();
    let _finished = r.terminal.finish(deadline);
    r.tty.settle(&r.clock);
    r.clock.at("settled");
    let echo = r.tty.since(mark);
    let (trace, entries) = (s7::drain_trace(), s1::drain_trace());
    drop(r);
    Applied {
        acks,
        trace,
        entries,
        echo,
        state,
        since,
    }
}

/// B6: the seven reader appliers, one chunk barrier each.
fn applier(value: &'static str) -> impl FnOnce(&Reader) -> Vec<Option<s7::Entry>> {
    move |r| write_value(&r.tty, &r.clock, value)
}
/// U19a: the appliers' table, for s5's composition too, with the typed
/// `stop` lines (SB3E-R196.2).
pub(super) fn write_value(tty: &Tty, clock: &Clock, value: &str) -> Vec<Option<s7::Entry>> {
    {
        match value {
            "0d" => vec![acked(clock, || tty.write(b"\r"))],
            "0a" => vec![acked(clock, || tty.write(b"\n"))],
            "0d0a" => vec![acked(clock, || tty.write(b"\r\n"))],
            "stop0d" => vec![acked(clock, || tty.write(b"stop\r"))],
            "stop0a" => vec![acked(clock, || tty.write(b"stop\n"))],
            "stop0d0a" => vec![acked(clock, || tty.write(b"stop\r\n"))],
            "04" => vec![acked(clock, || tty.write(b"\x04"))],
            // Q4: a whole command word without its terminator, then EOF.
            "partial+04" => {
                let quit = acked(clock, || tty.write(b"quit"));
                vec![quit, acked(clock, || tty.write(b"\x04"))]
            }
            "master_close" => vec![acked(clock, || tty.close_master())],
            "read_failure" => {
                let arm = PathBuf::from(std::env::var_os(ARM).unwrap());
                vec![acked(clock, || {
                    std::fs::write(&arm, b"").unwrap();
                    tty.write(b"x");
                })]
            }
            _ => panic!("reader: no applier for {value}"),
        }
    }
}

/// A value's oracle: the reader's acknowledgements (B7) and the state after.
fn value(f: &Fixture, value: &'static str, state: SessionState) {
    let a = apply(f, applier(value));
    let got = read_ack(value, &a.trace, &a.entries, &a.echo, a.since);
    let acks = got.unwrap_or_else(|finding| panic!("{finding}"));
    let received: Vec<_> = a.acks.iter().flatten().cloned().collect();
    assert_eq!(received, acks, "read: the barriers are not the trace's");
    assert_eq!(a.state, state, "read: state after {value}");
    if value == "master_close" {
        println!("WITNESS master_close branch={:?}", acks[0].branch);
    }
    if value == "partial+04" {
        assert!(a.echo.starts_with(b"quit"), "read: echo {:?}", a.echo);
    }
}

const ARM: &str = "ROUTE_READ_ARM";
fn no_env(_: &Path) -> Vec<(&'static str, PathBuf)> {
    Vec::new()
}
/// Q7: the read shim, built per child as store.rs builds route_fsync.c, its
/// digest witnessed by child_with (store::shim_witness, shim_witnessed).
fn read_env(root: &Path) -> Vec<(&'static str, PathBuf)> {
    use sha2::Digest;
    let (source, library) = (root.join("route_read.c"), root.join("route_read.so"));
    std::fs::write(&source, include_str!("route_read.c")).unwrap();
    let built = std::process::Command::new("cc")
        .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&library)
        .arg(&source)
        .arg("-ldl")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(built.status.success(), "the read shim builds: {stderr}");
    let digest = sha2::Sha256::digest(std::fs::read(&library).unwrap());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let (key, arm) = (super::store::SHIM_SHA256, root.join("read.arm"));
    vec![
        ("LD_PRELOAD", library),
        (ARM, arm),
        (key, PathBuf::from(hex)),
    ]
}

#[test]
fn route_reader_cr() {
    child_with("reader::route_reader_cr", no_env, |f| {
        value(f, "0d", SessionState::Normal)
    });
}
#[test]
fn route_reader_lf() {
    child_with("reader::route_reader_lf", no_env, |f| {
        value(f, "0a", SessionState::Normal)
    });
}
#[test]
fn route_reader_crlf() {
    child_with("reader::route_reader_crlf", no_env, |f| {
        value(f, "0d0a", SessionState::Normal)
    });
}
#[test]
fn route_reader_eof() {
    child_with("reader::route_reader_eof", no_env, |f| {
        value(f, "04", SessionState::Failed)
    });
}
#[test]
fn route_reader_partial_ctrl_d() {
    child_with("reader::route_reader_partial_ctrl_d", no_env, |f| {
        value(f, "partial+04", SessionState::Failed)
    });
}
#[test]
fn route_reader_master_close() {
    child_with("reader::route_reader_master_close", no_env, |f| {
        value(f, "master_close", SessionState::Failed)
    });
}
#[test]
fn route_reader_read_failure() {
    child_with("reader::route_reader_read_failure", read_env, |f| {
        value(f, "read_failure", SessionState::Failed)
    });
}

/// Negative control (des:474): the press and fail a real EOF causes, made
/// through the public ingress by the harness, is never a reader EOF.
#[test]
fn route_reader_injected_eof_is_not_read() {
    child_with(
        "reader::route_reader_injected_eof_is_not_read",
        no_env,
        |f| {
            let a = apply(f, |r| {
                let _ = r.ingress.emergency();
                r.ingress.fail();
                Vec::new()
            });
            let got = read_ack("04", &a.trace, &a.entries, &a.echo, a.since);
            let pressed = a.entries.iter().any(|e| e.point == "ingress.emergency");
            assert!(pressed, "the fake pressed");
            let refused = got.as_ref().err().map(String::as_str);
            assert_eq!(refused, Some("read: no host.read acknowledgement"));
        },
    );
}

/// T4: unarmed, S7 records nothing and the reader still echoes.
#[test]
fn route_reader_unarmed_records_nothing() {
    child_with(
        "reader::route_reader_unarmed_records_nothing",
        no_env,
        |f| {
            let mut r = compose(f);
            let mark = r.tty.len();
            r.tty.write(b"\r");
            let echoed = r.tty.wait_for(&r.clock, mark, b"\r\n");
            // The echo precedes the reader's site (review F1): the trace is
            // read only after the reader has been joined.
            let deadline = Instant::now() + r.clock.left();
            let finished = r.terminal.finish(deadline);
            assert!(finished, "unarmed: the terminal did not finish");
            let recorded = s7::drain_trace().len();
            drop(r);
            assert_eq!(recorded, 0, "unarmed: {recorded} entries");
            assert!(echoed, "unarmed: no echo");
        },
    );
}

/// S6: `stop` CR presses on the reader thread; the bound reporter then
/// dispatches and the recording completes (stop.rs:283, 334).
#[test]
fn route_s6_binds_the_stop_route() {
    child_with("reader::route_s6_binds_the_stop_route", no_env, |f| {
        let mut completed = false;
        let a = apply(f, |r| {
            let mark = r.tty.len();
            let ack = acked(&r.clock, || r.tty.write(b"stop\r"));
            r.clock.at("recording");
            let done = b"STOP RECORDING COMPLETED count=1";
            completed = r.tty.wait_for(&r.clock, mark, done);
            vec![ack]
        });
        let ack = a.acks[0].clone().expect("S6: no host.read acknowledgement");
        assert_eq!((ack.n, ack.eot, ack.branch), (5, false, Branch::Data));
        let press: Vec<_> = a
            .entries
            .iter()
            .filter(|e| e.point == "ingress.emergency")
            .collect();
        assert!(
            press.len() == 1 && press[0].thread == ack.thread && press[0].instant <= ack.instant,
            "S6: the press is not the reader's"
        );
        assert!(completed, "S6: no stop recording");
        assert_eq!(a.state, SessionState::Emergency);
    });
}
