//! One verifier execution, owned end to end by the backend (Linux).
//!
//! The helper is spawned, placed in a new, verified cgroup scope before any
//! launch message is sent, and launched. The backend then waits for the
//! verifier's report under the wall deadline and the output ceiling.
//! Finalization reads the scope's counters while the helper still holds the
//! scope, ends everything in it with `cgroup.kill`, reaps the helper and
//! confirms the scope is empty; only then is cleanup reported confirmed. An
//! unconfirmed cleanup keeps the scope and the helper as a retained boundary
//! that can be retried.
//!
//! Output is untrusted data: each stream is drained concurrently into a
//! bounded record (byte count, SHA-256 of the kept bytes, a bounded tail
//! excerpt, truncation), so a full pipe never stalls the verifier and an
//! output flood ends the execution.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::launcher::{Helper, HelperProgram, LaunchError, LaunchSpec, Outcome};
use crate::policy::ResourcePolicy;
use crate::protocol::{SetupStage, VerifierStatus};
use crate::scope::{Scope, ScopeError, ScopeEvents, ScopeManager};

/// Bound on confirming the scope is empty after `cgroup.kill`.
pub const FINALIZE_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the wait for the verifier's report checks the output ceiling.
const POLL_SLICE: Duration = Duration::from_millis(50);

/// A bounded record of one output stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamRecord {
    /// Every byte read, including any beyond the ceiling.
    pub bytes: u64,
    /// SHA-256 of the kept bytes (at most the ceiling).
    pub sha256: [u8; 32],
    /// More bytes arrived than the ceiling allows.
    pub truncated: bool,
    /// The last bytes kept, for a bounded display.
    pub excerpt: Vec<u8>,
}

fn drain(
    fd: OwnedFd,
    ceiling: u64,
    excerpt_bytes: usize,
    exceeded: Arc<AtomicBool>,
) -> JoinHandle<StreamRecord> {
    std::thread::spawn(move || {
        let mut file = std::fs::File::from(fd);
        let mut hasher = Sha256::new();
        let mut record = StreamRecord::default();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = match file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            let kept_before = record.bytes.min(ceiling);
            record.bytes += n as u64;
            let keep = (ceiling - kept_before).min(n as u64) as usize;
            if keep > 0 {
                hasher.update(&buf[..keep]);
                record.excerpt.extend_from_slice(&buf[..keep]);
                if record.excerpt.len() > excerpt_bytes {
                    let cut = record.excerpt.len() - excerpt_bytes;
                    record.excerpt.drain(..cut);
                }
            }
            if record.bytes > ceiling && !record.truncated {
                record.truncated = true;
                exceeded.store(true, Ordering::SeqCst);
            }
        }
        record.sha256 = hasher.finalize().into();
        record
    })
}

/// Why the backend ended the execution itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndedBy {
    Deadline,
    OutputLimit,
}

/// Where an execution stopped before the verifier ran.
#[derive(Debug)]
pub enum NotRun {
    /// The helper could not be started.
    Spawn(LaunchError),
    /// No verified scope could be created: a required layer is missing.
    Scope(ScopeError),
    /// The launch was refused before anything untrusted ran.
    Launch(LaunchError),
}

/// Whether the execution's boundary is gone.
#[derive(Debug)]
pub enum Cleanup {
    Confirmed,
    /// The scope could not be confirmed empty or the helper not reaped; the
    /// boundary is retained for a retry.
    Failed(RetainedBoundary),
}

impl Cleanup {
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

/// An execution whose cleanup is unconfirmed: the scope and, if not yet
/// reaped, the helper.
#[derive(Debug)]
pub struct RetainedBoundary {
    scope: Option<Scope>,
    helper: Option<Helper>,
}

impl RetainedBoundary {
    /// Kill again and re-check. `Ok(())` once the scope is empty and the
    /// helper reaped; otherwise the boundary is still retained.
    pub fn retry(mut self) -> Result<(), Self> {
        if let Some(scope) = &self.scope {
            let _ = scope.kill();
        }
        if let Some(helper) = self.helper.as_mut() {
            let _ = helper.kill();
            if reap_within(helper, FINALIZE_TIMEOUT) {
                self.helper = None;
            }
        }
        let empty = self.helper.is_none()
            && match &self.scope {
                Some(scope) => scope.wait_empty(FINALIZE_TIMEOUT).unwrap_or(false),
                None => true,
            };
        if empty {
            Ok(())
        } else {
            Err(self)
        }
    }
}

/// Everything the backend observed about one execution.
#[derive(Debug)]
pub struct ExecutionReport {
    pub not_run: Option<NotRun>,
    pub outcome: Option<Outcome>,
    pub ended_by: Option<EndedBy>,
    /// From the verifier's start to its end (or the backend ending it).
    pub duration: Duration,
    pub stdout: StreamRecord,
    pub stderr: StreamRecord,
    /// The scope's counters, if they could be read before it ended.
    pub events: Option<ScopeEvents>,
    pub cleanup: Cleanup,
}

/// Every class an execution can end in. The backend maps it one to one onto
/// the run-bound result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    Passed,
    Failed {
        exit_code: i32,
    },
    TimedOut,
    OutputLimitExceeded,
    OomKilled,
    ProcessLimit,
    Signalled {
        signal: i32,
    },
    SandboxUnavailable,
    SandboxSetupFailed,
    /// The verifier ran but the sandbox lost it or its accounting: the
    /// result is unknown.
    SandboxFailed,
    CleanupFailed,
}

fn unavailable_stage(stage: SetupStage, errno: i32) -> bool {
    matches!(
        (stage, errno),
        (
            SetupStage::Unshare,
            libc::EPERM | libc::ENOSYS | libc::EINVAL | libc::ENOSPC | libc::EUSERS
        )
    ) || stage == SetupStage::Landlock
}

impl ExecutionReport {
    /// Classify truthfully. Anything that is not a clean, unlimited exit
    /// with readable counters is never reported as passed; an unconfirmed
    /// cleanup overrides all else.
    pub fn classify(&self, passing_exit_code: i32) -> ExitClass {
        if !self.cleanup.is_confirmed() {
            return ExitClass::CleanupFailed;
        }
        match &self.not_run {
            Some(NotRun::Scope(_)) => return ExitClass::SandboxUnavailable,
            Some(NotRun::Launch(LaunchError::SetupFailed { stage, errno }))
                if unavailable_stage(*stage, *errno) =>
            {
                return ExitClass::SandboxUnavailable
            }
            Some(_) => return ExitClass::SandboxSetupFailed,
            None => {}
        }
        match self.ended_by {
            Some(EndedBy::Deadline) => return ExitClass::TimedOut,
            Some(EndedBy::OutputLimit) => return ExitClass::OutputLimitExceeded,
            None => {}
        }
        if self.stdout.truncated || self.stderr.truncated {
            return ExitClass::OutputLimitExceeded;
        }
        let status = match self.outcome {
            Some(Outcome::SetupFailed { stage, errno }) if unavailable_stage(stage, errno) => {
                return ExitClass::SandboxUnavailable
            }
            Some(Outcome::SetupFailed { .. }) => return ExitClass::SandboxSetupFailed,
            Some(Outcome::Finished(status)) => Some(status),
            Some(Outcome::Lost) | None => None,
        };
        // A limit the kernel enforced explains the end, whatever the
        // verifier reported; without counters nothing can be claimed.
        let Some(events) = self.events else {
            return ExitClass::SandboxFailed;
        };
        if events.oom_kills > 0 {
            return ExitClass::OomKilled;
        }
        if events.pids_max > 0 {
            return ExitClass::ProcessLimit;
        }
        match status {
            Some(VerifierStatus::Exited(code)) if code == passing_exit_code => ExitClass::Passed,
            Some(VerifierStatus::Exited(code)) => ExitClass::Failed { exit_code: code },
            Some(VerifierStatus::Signalled(signal)) => ExitClass::Signalled { signal },
            None => ExitClass::SandboxFailed,
        }
    }
}

/// Run one launch to completion. A cgroup scope is mandatory: without a
/// connected [`ScopeManager`] no execution can be started at all.
pub fn run(
    scopes: &ScopeManager,
    program: &HelperProgram,
    spec: LaunchSpec,
    limits: &ResourcePolicy,
) -> ExecutionReport {
    let not_run = |reason: NotRun, cleanup: Cleanup| ExecutionReport {
        not_run: Some(reason),
        outcome: None,
        ended_by: None,
        duration: Duration::ZERO,
        stdout: StreamRecord::default(),
        stderr: StreamRecord::default(),
        events: None,
        cleanup,
    };
    let (helper, output) = match Helper::spawn(program) {
        Ok(spawned) => spawned,
        Err(error) => return not_run(NotRun::Spawn(error), Cleanup::Confirmed),
    };
    let exceeded = Arc::new(AtomicBool::new(false));
    let excerpt = limits.excerpt_bytes as usize;
    let stdout = drain(
        output.stdout,
        limits.output_ceiling_bytes,
        excerpt,
        exceeded.clone(),
    );
    let stderr = drain(
        output.stderr,
        limits.output_ceiling_bytes,
        excerpt,
        exceeded.clone(),
    );

    // The helper waits for its launch; it is placed in its scope first.
    let scope = match scopes.start(&helper, limits) {
        Ok(scope) => scope,
        Err(error) => {
            let cleanup = finalize_unscoped(helper, stdout, stderr);
            return not_run(NotRun::Scope(error), cleanup);
        }
    };
    if let Err(error) = helper.handshake().and_then(|()| helper.launch(spec)) {
        let (cleanup, records, events) = finalize(scope, helper, stdout, stderr);
        let mut report = not_run(NotRun::Launch(error), cleanup);
        report.stdout = records.0;
        report.stderr = records.1;
        report.events = events;
        return report;
    }

    let started = Instant::now();
    let deadline = Duration::from_secs(limits.wall_timeout_secs);
    let mut ended_by = None;
    // Finalization, which follows at once, ends the execution with
    // `cgroup.kill` when the backend ends it.
    let outcome = loop {
        if exceeded.load(Ordering::SeqCst) {
            ended_by = Some(EndedBy::OutputLimit);
            break None;
        }
        let elapsed = started.elapsed();
        if elapsed >= deadline {
            ended_by = Some(EndedBy::Deadline);
            break None;
        }
        match helper.wait_report((deadline - elapsed).min(POLL_SLICE)) {
            Ok(Some(outcome)) => break Some(outcome),
            Ok(None) => continue,
            Err(_) => break Some(Outcome::Lost),
        }
    };
    let duration = started.elapsed();
    let (cleanup, (stdout, stderr), events) = finalize(scope, helper, stdout, stderr);
    ExecutionReport {
        not_run: None,
        outcome,
        ended_by,
        duration,
        stdout,
        stderr,
        events,
        cleanup,
    }
}

/// A helper that never reached a scope: it is still blocked before any
/// launch, so killing and reaping it ends everything it started.
fn finalize_unscoped(
    mut helper: Helper,
    stdout: JoinHandle<StreamRecord>,
    stderr: JoinHandle<StreamRecord>,
) -> Cleanup {
    let _ = helper.kill();
    if reap_within(&mut helper, FINALIZE_TIMEOUT) {
        let _ = (stdout.join(), stderr.join());
        Cleanup::Confirmed
    } else {
        Cleanup::Failed(RetainedBoundary {
            scope: None,
            helper: Some(helper),
        })
    }
}

/// Reap the helper (this backend's own child), waiting at most `timeout`.
fn reap_within(helper: &mut Helper, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match helper.try_reap() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return false,
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn finalize(
    scope: Scope,
    mut helper: Helper,
    stdout: JoinHandle<StreamRecord>,
    stderr: JoinHandle<StreamRecord>,
) -> (Cleanup, (StreamRecord, StreamRecord), Option<ScopeEvents>) {
    // After a final report the helper still holds the scope, so its
    // counters are read before anything in it is ended.
    let events = scope.events().ok();
    let _ = scope.kill();
    let reaped = reap_within(&mut helper, FINALIZE_TIMEOUT);
    let empty = reaped && scope.wait_empty(FINALIZE_TIMEOUT).unwrap_or(false);
    if !empty {
        // Do not wait on output that a surviving process may still hold.
        return (
            Cleanup::Failed(RetainedBoundary {
                scope: Some(scope),
                helper: (!reaped).then_some(helper),
            }),
            (StreamRecord::default(), StreamRecord::default()),
            events,
        );
    }
    // Every writer of the output pipes was in the scope, which is empty.
    let records = (
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    );
    (Cleanup::Confirmed, records, events)
}

#[cfg(test)]
mod tests;
