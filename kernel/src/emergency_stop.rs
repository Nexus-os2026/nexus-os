//! The kernel's emergency-stop authority (M0-FIX F5).
//!
//! One backend-owned authority, process-wide in production
//! ([`StopAuthority::global`]), that the owner's emergency stop engages and
//! every long-running kernel path observes: the cognitive loop, coding runs
//! and model calls. The broker (`nexus-governed-control`) engages it with its
//! own emergency stop and lifts it only after the owner's native resume,
//! through the authority's [`StopController`], which it alone holds.
//!
//! Semantics:
//! - [`StopAuthority::admit`] refuses new work while a stop is in force and
//!   otherwise returns a [`StopWatch`] bound to the current stop number.
//! - A watch is *halted* once any stop is engaged after its admission, even
//!   if that stop has since been lifted: work that a stop ended never
//!   resumes by itself; only new work, admitted after the resume, runs.
//! - [`GuardedChild`] is an owned child process that a stop terminates
//!   (bounded, reaped by its owner, never killed by name or guessed PID).
//!
//! What a stop ended is recorded in a bounded halt log ([`HaltRecord`]):
//! the area, the stop number and the outcome, never any secret or content.
//! Each area also records the halt in its own audit (the cognitive loop's
//! audit trail, the coding-run ledger); a model call, which has none, is
//! written to the durable [`HaltAudit`] the owner's Stop path attaches (the
//! governed-control evidence of record).
//!
//! No lock is held across a termination, an audit append or a provider call.
//!
//! # Who may lift a stop (M0-FIX2 F10)
//!
//! Only the owner's Stop/Resume path. The authority itself cannot lift:
//!
//! ```compile_fail
//! nexus_kernel::emergency_stop::StopAuthority::global().lift();
//! ```
//!
//! ```compile_fail
//! nexus_kernel::emergency_stop::StopAuthority::global().lift_if(1);
//! ```
//!
//! and the controller that can is never copied:
//!
//! ```compile_fail
//! use nexus_kernel::emergency_stop::StopController;
//! fn copy(controller: &StopController) -> StopController {
//!     controller.clone()
//! }
//! ```
//!
//! A private authority's controller is claimed once, by its creator; the
//! process's authority never hands its controller out this way:
//!
//! ```
//! use nexus_kernel::emergency_stop::StopAuthority;
//! let stop = StopAuthority::new();
//! let controller = stop.claim_controller().unwrap();
//! assert!(stop.claim_controller().is_none());
//! let number = controller.engage();
//! assert!(stop.is_stopped());
//! assert!(controller.lift_if(number));
//! assert!(!stop.is_stopped());
//! assert!(StopAuthority::global().claim_controller().is_none());
//! ```

use std::collections::VecDeque;
use std::io;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use thiserror::Error;

/// The most halt records kept; the oldest are dropped first.
pub const HALT_LOG_CAPACITY: usize = 256;

/// How long a stopped child may take to be reaped after its kill.
pub const CHILD_REAP_BOUND: Duration = Duration::from_secs(5);

/// How often an ending child is polled for its exit.
const CHILD_POLL: Duration = Duration::from_millis(10);

/// The refusal of work by the owner's emergency stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("refused: the owner's emergency stop is in force or ended this work")]
pub struct EmergencyStopped;

/// Which kind of work a stop ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltArea {
    CognitiveLoop,
    /// A governed coding run.
    Coding,
    ModelCall,
}

impl HaltArea {
    /// A stable name (audit records).
    pub fn as_str(self) -> &'static str {
        match self {
            HaltArea::CognitiveLoop => "cognitive_loop",
            HaltArea::Coding => "coding",
            HaltArea::ModelCall => "model_call",
        }
    }

    /// Whether a halt of this area is written to the attached durable
    /// [`HaltAudit`]. Model calls have no audit of their own. The cognitive
    /// loop records its halts in its audit trail and coding runs in the
    /// coding-run ledger, from call sites that may hold a run lock, where
    /// no audit callback may run.
    fn durable(self) -> bool {
        matches!(self, HaltArea::ModelCall)
    }
}

/// How the halted work ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltOutcome {
    /// The work ended (its child, if any, was terminated and reaped).
    Ended,
    /// The child could not be confirmed terminated and reaped.
    CleanupFailed(String),
}

impl HaltOutcome {
    /// A stable name (audit records).
    pub fn as_str(&self) -> &'static str {
        match self {
            HaltOutcome::Ended => "ended",
            HaltOutcome::CleanupFailed(_) => "cleanup_failed",
        }
    }
}

/// One halt, as recorded in the bounded halt log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaltRecord {
    pub area: HaltArea,
    pub stop_number: u64,
    pub outcome: HaltOutcome,
}

/// What became of a halt's durable record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltAudited {
    /// Its area keeps its own audit (see [`HaltArea`]): nothing written here.
    OwnAudit,
    /// No durable audit is attached to this authority: the halt is in the
    /// bounded halt log only.
    NoAudit,
    /// Durably written.
    Recorded,
    /// Kept by the audit, with its content, to be written before anything
    /// that audit admits later.
    Owed,
    /// The audit could not take it.
    Unrecorded,
}

/// The durable audit a halt is written to (M0-FIX2 F10). Production: the
/// governed-control evidence of record, attached by the owner's Stop path
/// through its [`StopController`]. It receives only the halt record (the
/// area, the stop number, the outcome): never a command, prompt, response
/// or credential. It is called with no lock of the authority held.
pub trait HaltAudit: Send + Sync {
    fn record(&self, halt: &HaltRecord) -> HaltAudited;
}

struct AttachedAudit(Arc<dyn HaltAudit>);

impl std::fmt::Debug for AttachedAudit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AttachedAudit")
    }
}

/// A durable audit was already attached; the first one stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a durable halt audit is already attached")]
pub struct HaltAuditAttached;

#[derive(Debug, Default)]
struct State {
    stopped: bool,
    /// Engagements so far; never decreases.
    number: u64,
    halts: VecDeque<HaltRecord>,
    audit: Option<AttachedAudit>,
}

/// The emergency-stop authority.
///
/// Anyone may hold it to admit work, observe the stop and engage a stop
/// (engaging only ever refuses and ends work: a fail-safe any kernel path
/// may take, which the owner's resume undoes). Lifting a stop is not a
/// method of the authority: it needs the authority's [`StopController`],
/// which is minted once. The process's authority mints it only through
/// [`StopAuthority::claim_global_controller`], which the owner's Stop/Resume
/// path (the governed-control front door) claims when it is built.
#[derive(Debug, Default)]
pub struct StopAuthority {
    state: Mutex<State>,
    changed: Condvar,
    notify: tokio::sync::Notify,
    /// The process's authority (its controller is claimed only through
    /// [`StopAuthority::claim_global_controller`]).
    global: bool,
    /// Whether its controller was minted.
    controlled: AtomicBool,
}

impl StopAuthority {
    /// A fresh authority, not stopped (tests and embedded hosts). Its
    /// creator may claim its controller with
    /// [`StopAuthority::claim_controller`].
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The process's authority: the one the broker's emergency stop engages
    /// and every production path observes.
    pub fn global() -> Arc<Self> {
        static GLOBAL: OnceLock<Arc<StopAuthority>> = OnceLock::new();
        GLOBAL
            .get_or_init(|| {
                Arc::new(Self {
                    global: true,
                    ..Self::default()
                })
            })
            .clone()
    }

    /// The controller of a fresh authority, once: `None` once claimed, and
    /// always `None` for the process's authority.
    pub fn claim_controller(self: &Arc<Self>) -> Option<StopController> {
        if self.global {
            return None;
        }
        self.mint_controller()
    }

    /// The controller of the process's authority, once per process. The
    /// owner's Stop/Resume path claims it when it is built (and refuses to
    /// be built without it); nothing else in production may name this
    /// function (guarded by `kernel/tests/m0_fix2_f10_stop_seal.rs`).
    pub fn claim_global_controller() -> Option<StopController> {
        Self::global().mint_controller()
    }

    fn mint_controller(self: &Arc<Self>) -> Option<StopController> {
        self.controlled
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| StopController {
                authority: Arc::clone(self),
            })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Engage a stop: refuse new work and halt every admitted watch. Returns
    /// the stop's number. Waiters are woken after the lock is released.
    pub fn engage(&self) -> u64 {
        let number = {
            let mut state = self.lock();
            state.stopped = true;
            state.number = state.number.saturating_add(1);
            state.number
        };
        self.changed.notify_all();
        self.notify.notify_waiters();
        number
    }

    pub fn is_stopped(&self) -> bool {
        self.lock().stopped
    }

    /// The number of the latest stop (0 before any).
    pub fn stop_number(&self) -> u64 {
        self.lock().number
    }

    /// Admit new work, or refuse it while a stop is in force.
    pub fn admit(self: &Arc<Self>) -> Result<StopWatch, EmergencyStopped> {
        let state = self.lock();
        if state.stopped {
            return Err(EmergencyStopped);
        }
        Ok(StopWatch {
            authority: Arc::clone(self),
            admitted_at: state.number,
        })
    }

    /// Record what a stop ended (bounded; no content): in the halt log and,
    /// for a model call, in the attached durable audit (written after the
    /// lock is released).
    pub(crate) fn record_halt(&self, area: HaltArea, outcome: HaltOutcome) -> HaltAudited {
        let (record, audit) = {
            let mut state = self.lock();
            let record = HaltRecord {
                area,
                stop_number: state.number,
                outcome,
            };
            if state.halts.len() == HALT_LOG_CAPACITY {
                state.halts.pop_front();
            }
            state.halts.push_back(record.clone());
            let audit = state.audit.as_ref().map(|audit| Arc::clone(&audit.0));
            (record, audit)
        };
        if !area.durable() {
            return HaltAudited::OwnAudit;
        }
        match audit {
            Some(audit) => audit.record(&record),
            None => HaltAudited::NoAudit,
        }
    }

    /// The halt log, oldest first.
    pub fn halts(&self) -> Vec<HaltRecord> {
        self.lock().halts.iter().cloned().collect()
    }

    /// Wake every waiter so it re-checks its own condition.
    fn wake(&self) {
        drop(self.lock());
        self.changed.notify_all();
    }
}

/// The right to lift one authority's stop: minted once per authority (see
/// [`StopAuthority::claim_global_controller`]), never cloned, copied or
/// built elsewhere. For the process's authority it is held by the owner's
/// Stop/Resume path alone.
#[must_use]
pub struct StopController {
    authority: Arc<StopAuthority>,
}

impl std::fmt::Debug for StopController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StopController").finish_non_exhaustive()
    }
}

impl StopController {
    /// The authority this controller lifts.
    pub fn authority(&self) -> &Arc<StopAuthority> {
        &self.authority
    }

    /// Engage a stop (see [`StopAuthority::engage`]).
    pub fn engage(&self) -> u64 {
        self.authority.engage()
    }

    /// Lift the stop only if it is still stop `number` (the one a resume
    /// confirmed): a stop engaged since stays in force. Returns whether the
    /// stop was lifted. Work the stop halted stays halted.
    pub fn lift_if(&self, number: u64) -> bool {
        let lifted = {
            let mut state = self.authority.lock();
            if state.number == number {
                state.stopped = false;
                true
            } else {
                false
            }
        };
        if lifted {
            self.authority.changed.notify_all();
        }
        lifted
    }

    /// Attach the durable audit model-call halts are written to, once.
    /// The model-call halts already in the halt log are written to it now,
    /// so a halt before the attachment is not left in memory only. Each
    /// halt is written once: the log is read and the audit attached in one
    /// critical section, and the writes happen after it.
    pub fn attach_halt_audit(&self, audit: Arc<dyn HaltAudit>) -> Result<(), HaltAuditAttached> {
        let backlog: Vec<HaltRecord> = {
            let mut state = self.authority.lock();
            if state.audit.is_some() {
                return Err(HaltAuditAttached);
            }
            state.audit = Some(AttachedAudit(Arc::clone(&audit)));
            state
                .halts
                .iter()
                .filter(|halt| halt.area.durable())
                .cloned()
                .collect()
        };
        for halt in &backlog {
            audit.record(halt);
        }
        Ok(())
    }
}

/// Admitted work's view of the authority.
#[derive(Clone)]
pub struct StopWatch {
    authority: Arc<StopAuthority>,
    admitted_at: u64,
}

impl std::fmt::Debug for StopWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StopWatch")
            .field("admitted_at", &self.admitted_at)
            .field("halted", &self.halted())
            .finish()
    }
}

impl StopWatch {
    /// Whether a stop was engaged since this work was admitted.
    pub fn halted(&self) -> bool {
        self.authority.lock().number != self.admitted_at
    }

    pub fn check(&self) -> Result<(), EmergencyStopped> {
        if self.halted() {
            Err(EmergencyStopped)
        } else {
            Ok(())
        }
    }

    pub fn authority(&self) -> &Arc<StopAuthority> {
        &self.authority
    }

    /// Block until this watch is halted (true) or `done` is set (false).
    /// Whoever sets `done` must call [`StopWatch::release`].
    fn wait_halted_or(&self, done: &AtomicBool) -> bool {
        let mut state = self.authority.lock();
        loop {
            if state.number != self.admitted_at {
                return true;
            }
            if done.load(Ordering::SeqCst) {
                return false;
            }
            state = self
                .authority
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Wait at most `timeout` for this watch to be halted.
    pub fn wait_halted_timeout(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.authority.lock();
        loop {
            if state.number != self.admitted_at {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = self
                .authority
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Wake waiters after setting a `done` flag they watch.
    fn release(&self) {
        self.authority.wake();
    }

    /// Resolves once this watch is halted (for async work).
    pub async fn halted_async(&self) {
        loop {
            let notified = self.authority.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.halted() {
                return;
            }
            notified.await;
        }
    }
}

/// Why a guarded child could not be started.
#[derive(Debug, Error)]
pub enum GuardError {
    #[error(transparent)]
    Stopped(#[from] EmergencyStopped),
    #[error("spawn failed: {0}")]
    Spawn(io::Error),
}

/// How a guarded child ended.
#[derive(Debug)]
pub enum ChildEnd {
    /// It exited by itself.
    Exited(ExitStatus),
    /// A stop terminated it: `Ok` once reaped, `Err` if its termination or
    /// reaping could not be confirmed within the bound.
    Halted(Result<ExitStatus, String>),
}

/// The pipes of a guarded child, taken before it is guarded.
#[derive(Debug, Default)]
pub struct ChildPipes {
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

#[derive(Debug)]
struct Slot {
    child: Option<Child>,
    killed: Option<io::Result<()>>,
}

/// An owned child process that the emergency stop terminates. The backend
/// owns its identity (the unreaped `Child`), so the kill reaches exactly
/// this process; it is reaped here, bounded, and a failure is reported.
pub struct GuardedChild {
    slot: Arc<Mutex<Slot>>,
    done: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>,
    watch: StopWatch,
    area: HaltArea,
    pid: u32,
}

impl std::fmt::Debug for GuardedChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardedChild")
            .field("pid", &self.pid)
            .field("area", &self.area)
            .finish_non_exhaustive()
    }
}

fn lock_slot(slot: &Mutex<Slot>) -> std::sync::MutexGuard<'_, Slot> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

impl GuardedChild {
    /// Spawn `command` as work admitted under `watch`. Refused, before
    /// anything starts, when the watch is already halted.
    pub fn spawn(
        command: &mut Command,
        watch: &StopWatch,
        area: HaltArea,
    ) -> Result<(Self, ChildPipes), GuardError> {
        watch.check()?;
        let mut child = command.spawn().map_err(GuardError::Spawn)?;
        let pipes = ChildPipes {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
        };
        let pid = child.id();
        let slot = Arc::new(Mutex::new(Slot {
            child: Some(child),
            killed: None,
        }));
        let done = Arc::new(AtomicBool::new(false));
        let watcher = {
            let slot = Arc::clone(&slot);
            let done = Arc::clone(&done);
            let watch = watch.clone();
            std::thread::Builder::new()
                .name("nexus-stop-guard".to_string())
                .spawn(move || {
                    if watch.wait_halted_or(&done) {
                        let mut slot = lock_slot(&slot);
                        if let Some(child) = slot.child.as_mut() {
                            slot.killed = Some(child.kill());
                        }
                    }
                })
        };
        let watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                // No guard could be started: the child is not left running.
                if let Some(mut child) = lock_slot(&slot).child.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(GuardError::Spawn(error));
            }
        };
        Ok((
            Self {
                slot,
                done,
                watcher: Some(watcher),
                watch: watch.clone(),
                area,
                pid,
            },
            pipes,
        ))
    }

    /// The child's process id (for diagnostics only; never an authority).
    pub fn id(&self) -> u32 {
        self.pid
    }

    /// Wait for the child to end: by itself, or terminated by a stop. A
    /// halted child is reaped within [`CHILD_REAP_BOUND`]; otherwise the
    /// failure is returned, never reported as success. The halt is recorded
    /// in the authority's halt log.
    pub fn finish(mut self) -> ChildEnd {
        let end = self.finish_inner();
        if let ChildEnd::Halted(result) = &end {
            let outcome = match result {
                Ok(_) => HaltOutcome::Ended,
                Err(error) => HaltOutcome::CleanupFailed(error.clone()),
            };
            self.watch.authority().record_halt(self.area, outcome);
        }
        end
    }

    fn finish_inner(&mut self) -> ChildEnd {
        // Wait for the exit while the watcher still guards the child.
        loop {
            let status = {
                let mut slot = lock_slot(&self.slot);
                let Some(child) = slot.child.as_mut() else {
                    return ChildEnd::Halted(Err("the child is no longer owned".to_string()));
                };
                child.try_wait()
            };
            match status {
                Ok(Some(status)) => {
                    self.stop_watcher();
                    let killed = lock_slot(&self.slot).killed.take();
                    lock_slot(&self.slot).child = None;
                    return match killed {
                        Some(Ok(())) => ChildEnd::Halted(Ok(status)),
                        Some(Err(error)) => {
                            // It exited anyway; the kill itself failed.
                            ChildEnd::Halted(Err(format!("kill failed: {error}")))
                        }
                        None if self.watch.halted() => ChildEnd::Halted(Ok(status)),
                        None => ChildEnd::Exited(status),
                    };
                }
                Ok(None) => {
                    if self.watch.halted() {
                        return self.reap_halted();
                    }
                    self.watch.wait_halted_timeout(CHILD_POLL);
                }
                Err(error) => {
                    self.stop_watcher();
                    if let Some(mut child) = lock_slot(&self.slot).child.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    return ChildEnd::Halted(Err(format!("wait failed: {error}")));
                }
            }
        }
    }

    fn stop_watcher(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        self.watch.release();
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }

    /// The watch is halted: make sure the child was killed, then reap it
    /// within the bound.
    fn reap_halted(&mut self) -> ChildEnd {
        self.stop_watcher();
        let mut slot = lock_slot(&self.slot);
        let killed = slot.killed.take();
        let Some(mut child) = slot.child.take() else {
            return ChildEnd::Halted(Err("the child is no longer owned".to_string()));
        };
        drop(slot);
        let kill = match killed {
            Some(result) => result,
            None => child.kill(),
        };
        let deadline = Instant::now() + CHILD_REAP_BOUND;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return match kill {
                        Ok(()) => ChildEnd::Halted(Ok(status)),
                        Err(error) => ChildEnd::Halted(Err(format!("kill failed: {error}"))),
                    };
                }
                Ok(None) if Instant::now() < deadline => std::thread::sleep(CHILD_POLL),
                Ok(None) => {
                    return ChildEnd::Halted(Err(format!(
                        "child {} was not reaped within {:?} of its kill",
                        self.pid, CHILD_REAP_BOUND
                    )));
                }
                Err(error) => return ChildEnd::Halted(Err(format!("reap failed: {error}"))),
            }
        }
    }
}

impl Drop for GuardedChild {
    /// Defense in depth: a guard dropped without `finish` (an early return
    /// or a panic) still ends and reaps its child, bounded.
    fn drop(&mut self) {
        self.stop_watcher();
        if let Some(mut child) = lock_slot(&self.slot).child.take() {
            let _ = child.kill();
            let deadline = Instant::now() + CHILD_REAP_BOUND;
            while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                std::thread::sleep(CHILD_POLL);
            }
        }
    }
}

/// A guarded child's collected output, or how a stop ended it.
#[derive(Debug, Error)]
pub enum GuardedOutputError {
    #[error(transparent)]
    Guard(#[from] GuardError),
    #[error("refused: the owner's emergency stop ended this work ({cleanup})")]
    Halted { cleanup: String },
    #[error("i/o error: {0}")]
    Io(io::Error),
}

/// Run `command` to completion as admitted work, writing `stdin` (if any),
/// collecting stdout and stderr; a stop terminates it.
pub fn guarded_output(
    command: &mut Command,
    stdin: Option<&[u8]>,
    watch: &StopWatch,
    area: HaltArea,
) -> Result<Output, GuardedOutputError> {
    use std::io::{Read, Write};
    let (child, mut pipes) = GuardedChild::spawn(command, watch, area)?;
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let stdout = drain(
        pipes
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        pipes
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let mut write_error = None;
    if let Some(mut pipe) = pipes.stdin.take() {
        if let Some(bytes) = stdin {
            if let Err(error) = pipe.write_all(bytes) {
                write_error = Some(error);
            }
        }
    }
    let end = child.finish();
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    match end {
        ChildEnd::Exited(status) => match write_error {
            Some(error) => Err(GuardedOutputError::Io(error)),
            None => Ok(Output {
                status,
                stdout,
                stderr,
            }),
        },
        ChildEnd::Halted(Ok(_)) => Err(GuardedOutputError::Halted {
            cleanup: "terminated and reaped".to_string(),
        }),
        ChildEnd::Halted(Err(error)) => Err(GuardedOutputError::Halted {
            cleanup: format!("cleanup failed: {error}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_halt_and_lift() {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        let before = stop.admit().unwrap();
        assert!(!before.halted());
        assert_eq!(stop.engage(), 1);
        assert!(before.halted());
        assert_eq!(stop.admit().unwrap_err(), EmergencyStopped);
        // A resume of stop 1 does not lift a newer stop 2.
        assert_eq!(stop.engage(), 2);
        assert!(!controller.lift_if(1));
        assert!(stop.is_stopped());
        assert!(controller.lift_if(2));
        // Work the stop ended stays ended; new work is admitted.
        assert!(before.halted());
        let after = stop.admit().unwrap();
        assert!(!after.halted());
        assert!(!after.wait_halted_timeout(Duration::from_millis(1)));
    }

    #[test]
    fn halt_log_is_bounded() {
        let stop = StopAuthority::new();
        stop.engage();
        for _ in 0..HALT_LOG_CAPACITY + 3 {
            stop.record_halt(HaltArea::ModelCall, HaltOutcome::Ended);
        }
        assert_eq!(stop.halts().len(), HALT_LOG_CAPACITY);
    }

    #[test]
    fn m0_fix2_f10_a_controller_is_minted_once() {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().expect("the creator's claim");
        assert!(stop.claim_controller().is_none(), "minted twice");
        assert!(Arc::ptr_eq(controller.authority(), &stop));
        // The process's authority: never through the generic claim (this
        // unit-test binary never claims the global controller, so the
        // refusal is the generic claim's own).
        assert!(StopAuthority::global().claim_controller().is_none());
    }

    /// A durable audit that records what it is given, and checks it is
    /// called with no lock of the authority held.
    struct Recording {
        authority: Arc<StopAuthority>,
        records: Mutex<Vec<HaltRecord>>,
    }

    impl HaltAudit for Recording {
        fn record(&self, halt: &HaltRecord) -> HaltAudited {
            // Would deadlock if the authority's lock were held.
            let _ = self.authority.halts();
            self.records.lock().unwrap().push(halt.clone());
            HaltAudited::Recorded
        }
    }

    #[test]
    fn m0_fix2_f10_model_call_halts_reach_the_durable_audit_once() {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        stop.engage();
        // Before an audit is attached: in the halt log only.
        assert_eq!(
            stop.record_halt(HaltArea::ModelCall, HaltOutcome::Ended),
            HaltAudited::NoAudit
        );
        let audit = Arc::new(Recording {
            authority: stop.clone(),
            records: Mutex::new(Vec::new()),
        });
        controller.attach_halt_audit(audit.clone()).unwrap();
        // The earlier model-call halt was written on attachment.
        assert_eq!(audit.records.lock().unwrap().len(), 1);
        assert_eq!(
            controller.attach_halt_audit(audit.clone()),
            Err(HaltAuditAttached),
            "the attached audit was replaced"
        );
        stop.engage();
        assert_eq!(
            stop.record_halt(
                HaltArea::ModelCall,
                HaltOutcome::CleanupFailed("kill failed".into())
            ),
            HaltAudited::Recorded
        );
        // Areas with their own audit are not written here.
        assert_eq!(
            stop.record_halt(HaltArea::Coding, HaltOutcome::Ended),
            HaltAudited::OwnAudit
        );
        assert_eq!(
            stop.record_halt(HaltArea::CognitiveLoop, HaltOutcome::Ended),
            HaltAudited::OwnAudit
        );
        assert_eq!(
            *audit.records.lock().unwrap(),
            vec![
                HaltRecord {
                    area: HaltArea::ModelCall,
                    stop_number: 1,
                    outcome: HaltOutcome::Ended,
                },
                HaltRecord {
                    area: HaltArea::ModelCall,
                    stop_number: 2,
                    outcome: HaltOutcome::CleanupFailed("kill failed".into()),
                },
            ]
        );
        assert_eq!(stop.halts().len(), 4);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn current_exe_helper() -> Command {
        // The test binary itself: an ignored helper that blocks on stdin.
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "emergency_stop::tests::helper_blocks_on_stdin",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        command
    }

    #[test]
    #[ignore]
    fn helper_blocks_on_stdin() {
        use std::io::{Read, Write};
        println!("HELPER_READY");
        std::io::stdout().flush().unwrap();
        let mut sink = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut sink);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn a_stop_terminates_and_reaps_a_guarded_child() {
        use std::io::{BufRead, BufReader};
        let stop = StopAuthority::new();
        let watch = stop.admit().unwrap();
        let (child, mut pipes) =
            GuardedChild::spawn(&mut current_exe_helper(), &watch, HaltArea::ModelCall).unwrap();
        let pid = child.id();
        // Readiness: the helper is running and blocked on its stdin.
        let mut out = BufReader::new(pipes.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("HELPER_READY") {
            line.clear();
            assert!(out.read_line(&mut line).unwrap() > 0, "helper ended early");
        }
        let _stdin = pipes.stdin.take();
        stop.engage();
        match child.finish() {
            ChildEnd::Halted(Ok(status)) => assert!(!status.success()),
            other => panic!("{other:?}"),
        }
        // Reaped: the process id no longer names our child.
        let alive = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None);
        assert_eq!(alive, Err(nix::errno::Errno::ESRCH));
        assert_eq!(
            stop.halts(),
            vec![HaltRecord {
                area: HaltArea::ModelCall,
                stop_number: 1,
                outcome: HaltOutcome::Ended,
            }]
        );
        // New work is refused before anything starts, until the lift.
        assert!(matches!(
            GuardedChild::spawn(&mut current_exe_helper(), &watch, HaltArea::ModelCall),
            Err(GuardError::Stopped(_))
        ));
    }

    /// A durable audit that never records anything.
    struct Failing;

    impl HaltAudit for Failing {
        fn record(&self, _: &HaltRecord) -> HaltAudited {
            HaltAudited::Unrecorded
        }
    }

    /// The helper, running and blocked on its stdin, under `watch`.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn blocked_helper(watch: &StopWatch) -> (GuardedChild, ChildPipes) {
        use std::io::{BufRead, BufReader};
        let (child, mut pipes) =
            GuardedChild::spawn(&mut current_exe_helper(), watch, HaltArea::ModelCall).unwrap();
        let mut out = BufReader::new(pipes.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("HELPER_READY") {
            line.clear();
            assert!(out.read_line(&mut line).unwrap() > 0, "helper ended early");
        }
        (child, pipes)
    }

    /// M0-FIX2 F10 review (MINOR-1) oracle: an audit that cannot record a
    /// halt never undoes it (the model call's child is still terminated and
    /// reaped, the stop stays in force), and the failure is observable.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn m0_fix2_f10_a_failing_halt_audit_never_undoes_a_halt_and_is_counted() {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        controller.attach_halt_audit(Arc::new(Failing)).unwrap();
        assert_eq!(stop.unrecorded_halts(), 0);
        let watch = stop.admit().unwrap();
        let (child, mut pipes) = blocked_helper(&watch);
        let pid = child.id();
        let _stdin = pipes.stdin.take();
        stop.engage();
        match child.finish() {
            ChildEnd::Halted(Ok(status)) => assert!(!status.success()),
            other => panic!("{other:?}"),
        }
        let alive = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None);
        assert_eq!(alive, Err(nix::errno::Errno::ESRCH));
        assert!(stop.is_stopped());
        assert!(stop.admit().is_err());
        assert_eq!(
            stop.unrecorded_halts(),
            1,
            "the unrecorded halt is invisible"
        );
    }

    /// M0-FIX2 F10 review (MINOR-1, NIT) oracle: a model-call halt that
    /// never reaches a durable audit is counted, whether it fell out of the
    /// bounded log before any audit was attached or the audit refused it
    /// when the backlog was written.
    #[test]
    fn m0_fix2_f10_halts_that_never_reach_the_audit_are_counted() {
        let stop = StopAuthority::new();
        let controller = stop.claim_controller().unwrap();
        stop.engage();
        for _ in 0..HALT_LOG_CAPACITY + 2 {
            stop.record_halt(HaltArea::ModelCall, HaltOutcome::Ended);
        }
        // Areas with their own audit are never counted here.
        stop.record_halt(HaltArea::Coding, HaltOutcome::Ended);
        assert_eq!(stop.unrecorded_halts(), 3, "halts lost from the log");
        controller.attach_halt_audit(Arc::new(Failing)).unwrap();
        assert_eq!(
            stop.unrecorded_halts(),
            3 + HALT_LOG_CAPACITY as u64 - 1,
            "backlog halts the audit refused"
        );
    }

    /// M0-FIX2 F10 review (NIT) oracle: a halt is recorded under the stop
    /// that ended the work, not under a later stop engaged before the halt
    /// was recorded.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn m0_fix2_f10_a_halt_is_attributed_to_the_stop_that_ended_it() {
        let stop = StopAuthority::new();
        let watch = stop.admit().unwrap();
        let (child, mut pipes) = blocked_helper(&watch);
        let _stdin = pipes.stdin.take();
        assert_eq!(stop.engage(), 1);
        assert_eq!(stop.engage(), 2);
        assert!(matches!(child.finish(), ChildEnd::Halted(Ok(_))));
        assert_eq!(stop.halts().last().unwrap().stop_number, 1);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn an_unstopped_guarded_child_exits_normally() {
        let stop = StopAuthority::new();
        let watch = stop.admit().unwrap();
        let output = guarded_output(
            &mut current_exe_helper(),
            Some(b"bytes"),
            &watch,
            HaltArea::ModelCall,
        )
        .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("HELPER_READY"));
        assert!(stop.halts().is_empty());
    }
}
