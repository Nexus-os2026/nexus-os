//! One verifier execution, owned end to end by the backend (Linux).
//!
//! The helper is spawned, placed in a new, verified cgroup scope before any
//! launch message is sent, and launched. The backend then waits for the
//! verifier's report under the wall deadline and the output ceiling.
//! Finalization reads the scope's counters while the helper still holds the
//! scope, ends everything in it with `cgroup.kill`, kills and reaps the
//! helper and confirms the scope is empty; only then is cleanup reported
//! confirmed. An unconfirmed cleanup keeps the scope and the helper as a
//! retained boundary that can be retried.
//!
//! Panic safety: everything an execution creates (the helper, its scope and
//! the output threads) is owned in [`run`]'s own frame, never inside a
//! closure that can panic. The launch and the wait run behind
//! `catch_unwind`; whether they return or panic, the same finalizer then
//! runs over whatever exists, and a panic inside the finalizer leaves what
//! it could not confirm in a retained boundary. So a panic never unwinds
//! past the last owner of a live helper or scope, and [`run`] itself never
//! unwinds: a panicked execution is reported as interrupted (never passed),
//! with its cleanup either confirmed or retained.
//!
//! Output is untrusted data: each stream is drained concurrently into a
//! bounded record (byte count, SHA-256 of the kept bytes, a bounded tail
//! excerpt, truncation), so a full pipe never stalls the verifier and an
//! output flood ends the execution.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::fault;
#[cfg(not(any(test, feature = "live-sandbox-harness")))]
use crate::fault::{Fault, FaultPoint};
#[cfg(any(test, feature = "live-sandbox-harness"))]
pub use crate::fault::{Fault, FaultPoint, RUNNING_FAULT_AFTER};
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

/// Drain one output stream into a bounded record, on a thread of its own.
fn drain(
    fd: OwnedFd,
    limits: &ResourcePolicy,
    exceeded: Arc<AtomicBool>,
    drained: Arc<AtomicBool>,
    fault: Option<Fault>,
) -> std::io::Result<JoinHandle<StreamRecord>> {
    let ceiling = limits.output_ceiling_bytes;
    let excerpt_bytes = limits.excerpt_bytes as usize;
    std::thread::Builder::new()
        .name("nexus-verifier-output".to_string())
        .spawn(move || {
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
                drained.store(true, Ordering::SeqCst);
                fault::at(fault, FaultPoint::DrainThread);
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
    /// A panic in the backend interrupted the execution after its launch
    /// could reach the helper: the result is unknown.
    Interrupted,
}

/// Where an execution stopped before the verifier ran.
#[derive(Debug)]
pub enum NotRun {
    /// The helper (or its output threads) could not be started.
    Spawn(LaunchError),
    /// No verified scope could be created: a required layer is missing.
    Scope(ScopeError),
    /// The launch was refused before anything untrusted ran.
    Launch(LaunchError),
    /// A panic in the backend interrupted the execution before its launch
    /// could reach the helper: nothing untrusted ran.
    Interrupted,
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

/// An execution whose cleanup is unconfirmed: its scope and, if not yet
/// reaped, its helper. Retaining it is the only way to finish the cleanup:
/// its owner keeps it until [`Self::retry`] succeeds.
#[derive(Debug)]
pub struct RetainedBoundary {
    scope: Option<Scope>,
    helper: Option<Helper>,
}

impl RetainedBoundary {
    /// End everything again with the finalizer's own steps and re-check.
    /// `Ok(())` once the helper is reaped and the scope empty or removed;
    /// otherwise, a panic while retrying included, the boundary is still
    /// retained.
    pub fn retry(mut self) -> Result<(), Self> {
        let ended = catch_unwind(AssertUnwindSafe(|| {
            end(self.scope.as_ref(), &mut self.helper)
        }))
        .unwrap_or(false);
        if ended {
            self.scope = None;
            Ok(())
        } else {
            Err(self)
        }
    }

    /// Whether the scope is still retained (live panic controls).
    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub fn holds_scope(&self) -> bool {
        self.scope.is_some()
    }

    /// Whether the helper is still unreaped (live panic controls).
    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub fn holds_helper(&self) -> bool {
        self.helper.is_some()
    }
}

impl Drop for RetainedBoundary {
    /// Defense in depth for a boundary dropped without a confirmed retry:
    /// end what it holds without waiting. Never a confirmation; owners keep
    /// a retained boundary until [`Self::retry`] succeeds.
    fn drop(&mut self) {
        end_now(self.scope.as_ref(), &mut self.helper);
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
    /// An output thread panicked: its stream's record is lost.
    pub output_lost: bool,
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
    /// Classify truthfully. Anything that is not a clean, unlimited,
    /// uninterrupted exit with readable counters and output records is
    /// never reported as passed; an unconfirmed cleanup overrides all else.
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
            Some(EndedBy::Interrupted) => return ExitClass::SandboxFailed,
            None => {}
        }
        // Lost output is lost accounting: nothing can be claimed.
        if self.output_lost {
            return ExitClass::SandboxFailed;
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
/// connected [`ScopeManager`] no execution can be started at all. Never
/// unwinds (see the module documentation).
pub fn run(
    scopes: &ScopeManager,
    program: &HelperProgram,
    spec: LaunchSpec,
    limits: &ResourcePolicy,
) -> ExecutionReport {
    execute(scopes, program, spec, limits, None)
}

/// [`run`] with one deterministic fault injected: the live panic controls
/// only.
#[cfg(any(test, feature = "live-sandbox-harness"))]
pub fn run_with_fault(
    scopes: &ScopeManager,
    program: &HelperProgram,
    spec: LaunchSpec,
    limits: &ResourcePolicy,
    fault: Fault,
) -> ExecutionReport {
    execute(scopes, program, spec, limits, Some(fault))
}

/// Everything one execution owns. It lives in [`execute`]'s own frame and
/// is only ever borrowed by the code that can panic.
#[derive(Default)]
struct Owned {
    helper: Option<Helper>,
    scope: Option<Scope>,
    stdout: Option<JoinHandle<StreamRecord>>,
    stderr: Option<JoinHandle<StreamRecord>>,
    /// The launch may have reached the helper: something untrusted may
    /// have run. Only ever set once the scope exists.
    launched: bool,
    /// When the helper reported the verifier running.
    started: Option<Instant>,
}

impl Owned {
    /// Hand whatever remains to a retained boundary; with nothing left, the
    /// cleanup is confirmed.
    fn retain(&mut self) -> Cleanup {
        match (self.scope.take(), self.helper.take()) {
            (None, None) => Cleanup::Confirmed,
            (scope, helper) => Cleanup::Failed(RetainedBoundary { scope, helper }),
        }
    }
}

impl Drop for Owned {
    /// Defense in depth only: [`execute`] never lets a live boundary reach
    /// this point (it is confirmed ended or moved into a retained boundary).
    fn drop(&mut self) {
        end_now(self.scope.as_ref(), &mut self.helper);
    }
}

/// What the launch and the wait observed.
#[derive(Default)]
struct Observed {
    not_run: Option<NotRun>,
    outcome: Option<Outcome>,
    ended_by: Option<EndedBy>,
    duration: Duration,
}

/// What finalization established.
struct Finalized {
    cleanup: Cleanup,
    stdout: StreamRecord,
    stderr: StreamRecord,
    output_lost: bool,
    events: Option<ScopeEvents>,
}

impl Finalized {
    /// Finalization that confirmed nothing more: the output is not waited
    /// for, since a surviving process may still hold it.
    fn without_output(cleanup: Cleanup, events: Option<ScopeEvents>) -> Self {
        Self {
            cleanup,
            stdout: StreamRecord::default(),
            stderr: StreamRecord::default(),
            output_lost: false,
            events,
        }
    }
}

fn execute(
    scopes: &ScopeManager,
    program: &HelperProgram,
    spec: LaunchSpec,
    limits: &ResourcePolicy,
    fault: Option<Fault>,
) -> ExecutionReport {
    let mut owned = Owned::default();
    let observed = catch_unwind(AssertUnwindSafe(|| {
        attempt(&mut owned, scopes, program, spec, limits, fault)
    }))
    .unwrap_or_else(|_| interrupted(&owned));
    let finalized = finalize(&mut owned, fault);
    ExecutionReport {
        not_run: observed.not_run,
        outcome: observed.outcome,
        ended_by: observed.ended_by,
        duration: observed.duration,
        stdout: finalized.stdout,
        stderr: finalized.stderr,
        output_lost: finalized.output_lost,
        events: finalized.events,
        cleanup: finalized.cleanup,
    }
}

/// What a panic leaves observed: nothing it saw is trusted. Before the
/// launch could reach the helper nothing untrusted ran; after that the
/// result is unknown.
fn interrupted(owned: &Owned) -> Observed {
    if owned.launched {
        Observed {
            ended_by: Some(EndedBy::Interrupted),
            duration: owned
                .started
                .map_or(Duration::ZERO, |started| started.elapsed()),
            ..Observed::default()
        }
    } else {
        Observed {
            not_run: Some(NotRun::Interrupted),
            ..Observed::default()
        }
    }
}

/// Spawn, place, launch and wait. Everything created is stored in `owned`
/// at once, before the next step that can fail or panic.
fn attempt(
    owned: &mut Owned,
    scopes: &ScopeManager,
    program: &HelperProgram,
    spec: LaunchSpec,
    limits: &ResourcePolicy,
    fault: Option<Fault>,
) -> Observed {
    let not_run = |reason: NotRun| Observed {
        not_run: Some(reason),
        ..Observed::default()
    };
    let (helper, output) = match Helper::spawn(program) {
        Ok(spawned) => spawned,
        Err(error) => return not_run(NotRun::Spawn(error)),
    };
    let helper = owned.helper.insert(helper);
    let exceeded = Arc::new(AtomicBool::new(false));
    let drained = Arc::new(AtomicBool::new(false));
    let start = |fd| drain(fd, limits, exceeded.clone(), drained.clone(), fault);
    match start(output.stdout) {
        Ok(thread) => owned.stdout = Some(thread),
        Err(error) => return not_run(NotRun::Spawn(LaunchError::Spawn(error))),
    }
    match start(output.stderr) {
        Ok(thread) => owned.stderr = Some(thread),
        Err(error) => return not_run(NotRun::Spawn(LaunchError::Spawn(error))),
    }
    fault::at(fault, FaultPoint::AfterSpawn);

    // The helper waits for its launch; it is placed in its scope first.
    match scopes.start_with_fault(helper, limits, fault) {
        Ok(scope) => owned.scope = Some(scope),
        Err(error) => return not_run(NotRun::Scope(error)),
    }
    fault::at(fault, FaultPoint::AfterScope);
    if let Err(error) = helper.handshake() {
        return not_run(NotRun::Launch(error));
    }
    owned.launched = true;
    if let Err(error) = helper.launch(spec) {
        return not_run(NotRun::Launch(error));
    }

    let started = *owned.started.insert(Instant::now());
    fault::at(fault, FaultPoint::AfterLaunch);
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
            Ok(None) => {
                if started.elapsed() >= fault::RUNNING_FAULT_AFTER {
                    fault::at(fault, FaultPoint::Running);
                }
                if drained.load(Ordering::SeqCst) {
                    fault::at(fault, FaultPoint::Draining);
                }
            }
            Err(_) => break Some(Outcome::Lost),
        }
    };
    let duration = started.elapsed();
    fault::at(fault, FaultPoint::BeforeFinalize);
    Observed {
        not_run: None,
        outcome,
        ended_by,
        duration,
    }
}

/// The one finalizer, whether the attempt returned or panicked. A panic
/// inside it leaves what it could not confirm retained.
fn finalize(owned: &mut Owned, fault: Option<Fault>) -> Finalized {
    catch_unwind(AssertUnwindSafe(|| finalize_steps(owned, fault)))
        .unwrap_or_else(|_| Finalized::without_output(owned.retain(), None))
}

fn finalize_steps(owned: &mut Owned, fault: Option<Fault>) -> Finalized {
    // After a final report the helper still holds the scope, so its
    // counters are read before anything in it is ended.
    let events = owned.scope.as_ref().and_then(|scope| scope.events().ok());
    if fault::at(fault, FaultPoint::Finalizing) || !end(owned.scope.as_ref(), &mut owned.helper) {
        // Do not wait on output that a surviving process may still hold.
        return Finalized::without_output(owned.retain(), events);
    }
    // Nothing of this execution remains, so every writer of the output
    // pipes is gone.
    owned.scope = None;
    let (stdout, stdout_lost) = join(owned.stdout.take());
    let (stderr, stderr_lost) = join(owned.stderr.take());
    Finalized {
        cleanup: Cleanup::Confirmed,
        stdout,
        stderr,
        output_lost: stdout_lost || stderr_lost,
        events,
    }
}

/// End everything an execution may have left, and confirm it: `cgroup.kill`
/// on the scope, a kill of the helper (whose death-signal chain also ends
/// its namespace init), a bounded reap of the helper, then a bounded wait
/// for the scope to be empty or removed. A helper without a scope never
/// received a launch, so reaping it ends everything it started. The helper
/// is released once reaped; the scope stays with the caller. `true` only
/// once nothing can remain.
fn end(scope: Option<&Scope>, helper: &mut Option<Helper>) -> bool {
    if let Some(scope) = scope {
        let _ = scope.kill();
    }
    if let Some(child) = helper.as_mut() {
        let _ = child.kill();
        if !reap_within(child, FINALIZE_TIMEOUT) {
            return false;
        }
        *helper = None;
    }
    match scope {
        Some(scope) => scope.wait_empty(FINALIZE_TIMEOUT).unwrap_or(false),
        None => true,
    }
}

/// Best effort and without waiting: end whatever may remain. Defense in
/// depth only, never a confirmation.
fn end_now(scope: Option<&Scope>, helper: &mut Option<Helper>) {
    if let Some(scope) = scope {
        let _ = scope.kill();
    }
    if let Some(helper) = helper.as_mut() {
        let _ = helper.kill();
        let _ = helper.try_reap();
    }
}

/// A drained stream's record, and whether it was lost to a panicked output
/// thread.
fn join(thread: Option<JoinHandle<StreamRecord>>) -> (StreamRecord, bool) {
    match thread.map(JoinHandle::join) {
        Some(Ok(record)) => (record, false),
        Some(Err(_)) => (StreamRecord::default(), true),
        None => (StreamRecord::default(), false),
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

#[cfg(test)]
mod tests;
