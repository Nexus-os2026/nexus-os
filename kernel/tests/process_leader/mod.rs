//! The leader-held rule (M2b-A decision 4), shared by the G0 tripwire's
//! oracles and the desktop's live-test harness (`#[path]`-included).
//!
//! On Linux a process-group id cannot be reused while any process of that
//! group exists, a zombie leader included. So a test that owns a process
//! group holds its leader's `Child`, never reaps it before the group's final
//! signal, and ends it with exactly one cleanup: `killpg` of the held
//! leader's group (only success or ESRCH passes), then the leader's reap,
//! within one deadline. Nothing is ever signalled on inference.
//!
//! Recording seam (tests of this rule only): with recording on for a
//! thread, every group signal sent from that thread is recorded, with
//! whether it went through a held leader and whether the group's leader
//! process was present at that moment; a signal not sent through a held
//! leader, or to a group whose leader is gone, is recorded and
//! **suppressed**, so a planted mistake can never reach a reused group.

#![allow(dead_code)]

use std::cell::RefCell;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a final cleanup may take when its caller names no bound.
pub const REAP: Duration = Duration::from_secs(10);

/// One recorded group signal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signalled {
    pub pgid: i32,
    /// Sent through a held leader's final cleanup.
    pub held: bool,
    /// A process whose pid is `pgid` was in group `pgid` when it was sent.
    pub leader_present: bool,
}

thread_local! {
    static RECORDING: RefCell<Option<Vec<Signalled>>> = const { RefCell::new(None) };
    /// Under recording only: every held signal is sent, then reported as
    /// failed (to prove that a failed final signal fails its cleanup, without
    /// leaving the process running).
    static FAIL_SIGNALS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Failures of `Drop` backstops, for the tests to check at their end.
static BACKSTOP_FAILURES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Run `f` with recording on for this thread; what it signalled.
pub fn recording<R>(f: impl FnOnce() -> R) -> (R, Vec<Signalled>) {
    RECORDING.with(|r| *r.borrow_mut() = Some(Vec::new()));
    let result = f();
    let signalled = RECORDING
        .with(|r| r.borrow_mut().take())
        .unwrap_or_default();
    (result, signalled)
}

/// As `recording`, with every held signal reported as failed after it was
/// sent.
pub fn recording_with_failed_signals<R>(f: impl FnOnce() -> R) -> (R, Vec<Signalled>) {
    FAIL_SIGNALS.with(|fail| fail.set(true));
    let outcome = recording(f);
    FAIL_SIGNALS.with(|fail| fail.set(false));
    outcome
}

/// Take the failures `Drop` backstops recorded so far.
pub fn backstop_failures() -> Vec<String> {
    std::mem::take(&mut *BACKSTOP_FAILURES.lock().unwrap_or_else(|p| p.into_inner()))
}

/// The process group a cleanup may signal, from the pid of a child spawned
/// as its own group's leader. Never 0 or 1 (which would signal every
/// process this user owns), never out of range, never the caller's own
/// group.
pub fn checked_pgid(pid: u32) -> Result<i32, String> {
    let pgid =
        i32::try_from(pid).map_err(|_| format!("refusing process group {pid}: out of range"))?;
    if pgid <= 1 {
        return Err(format!(
            "refusing process group {pgid}: never a broadcast or init's group"
        ));
    }
    if pgid == nix::unistd::getpgrp().as_raw() {
        return Err(format!(
            "refusing process group {pgid}: it is the caller's own"
        ));
    }
    Ok(pgid)
}

/// What a group signal's result means: delivered, or the group no longer
/// exists (ESRCH: gone). Any other error is a failure.
pub fn signal_outcome(result: Result<(), nix::errno::Errno>, pgid: i32) -> Result<(), String> {
    match result {
        Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
        Err(error) => Err(format!("signalling process group {pgid} failed: {error}")),
    }
}

/// Whether a process with pid `pgid` exists in group `pgid` (a zombie
/// counts): `/proc/<pgid>/stat`'s process-group field.
pub fn leader_present(pgid: i32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pgid}/stat")) else {
        return false;
    };
    // The command name may hold spaces: the fields after its closing ')'.
    stat.rsplit_once(')')
        .map(|(_, rest)| rest.split_whitespace().nth(2) == Some(&pgid.to_string()))
        .unwrap_or(false)
}

/// SIGKILL to group `pgid`. `held`: through a held leader's final cleanup.
/// Under recording, a signal that is not held is recorded and suppressed.
pub fn send_group_kill(pgid: i32, held: bool) -> Result<(), String> {
    let present = leader_present(pgid);
    let recorded = RECORDING.with(|r| {
        r.borrow_mut().as_mut().map(|signalled| {
            signalled.push(Signalled {
                pgid,
                held,
                leader_present: present,
            })
        })
    });
    if recorded.is_some() && (!held || !present) {
        return Ok(());
    }
    let outcome = signal_outcome(
        nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pgid),
            nix::sys::signal::Signal::SIGKILL,
        ),
        pgid,
    );
    if recorded.is_some() && FAIL_SIGNALS.with(|fail| fail.get()) {
        return Err(format!("signalling process group {pgid} failed: injected"));
    }
    outcome
}

/// A process-group leader the test owns: the only thing it may signal.
pub struct Leader {
    child: Child,
    pgid: i32,
    finished: bool,
}

impl Leader {
    /// Spawn `command` as the leader of its own new process group.
    pub fn spawn(command: &mut Command) -> Result<Self, String> {
        use std::os::unix::process::CommandExt;
        let child = command
            .process_group(0)
            .spawn()
            .map_err(|e| format!("the process did not start: {e}"))?;
        let pgid = match checked_pgid(child.id()) {
            Ok(pgid) => pgid,
            Err(e) => {
                // Not a group this test may signal: end the child alone,
                // through its own handle, within a bound.
                let mut child = child;
                let _ = child.kill();
                let until = Instant::now() + REAP;
                while matches!(child.try_wait(), Ok(None)) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(10));
                }
                return Err(e);
            }
        };
        Ok(Self {
            child,
            pgid,
            finished: false,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Whether the leader has exited, observed without reaping it
    /// (`waitid` with `WNOWAIT`): an exited leader stays a zombie that holds
    /// its process group until the final cleanup.
    pub fn exited(&mut self) -> Result<bool, String> {
        use nix::sys::wait::{waitid, Id, WaitPidFlag, WaitStatus};
        match waitid(
            Id::Pid(nix::unistd::Pid::from_raw(self.pgid)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        ) {
            Ok(WaitStatus::StillAlive) => Ok(false),
            Ok(_) => Ok(true),
            Err(e) => Err(format!("observing leader {}: {e}", self.pgid)),
        }
    }

    /// The final cleanup: SIGKILL to the held leader's group (only success
    /// or ESRCH passes), then the leader's reap, all within `within`.
    pub fn finish(mut self, within: Duration) -> Result<ExitStatus, String> {
        self.final_cleanup(within)
    }

    fn final_cleanup(&mut self, within: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + within;
        self.finished = true;
        send_group_kill(self.pgid, true)?;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    return Err(format!(
                        "leader {} was not reaped within {within:?}",
                        self.pgid
                    ))
                }
                Err(e) => return Err(format!("reaping leader {}: {e}", self.pgid)),
            }
        }
    }
}

impl Drop for Leader {
    /// A backstop only: a leader its test did not finish is finished here,
    /// and a failure is recorded for the test to find.
    fn drop(&mut self) {
        if !self.finished {
            if let Err(e) = self.final_cleanup(REAP) {
                BACKSTOP_FAILURES
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(e);
            }
        }
    }
}
