//! P0-002C4B: private lifecycle primitive for a trusted, internally created
//! owned process tree. P0-002C4C1: the only production registry is owned by
//! BuilderWorkspaceAuthority (sharing its ProjectCatalog) and production uses
//! only stop, owned_status and shutdown_all. No production source calls
//! `start` or creates a tree; production launch is a later checkpoint.
//!
//! Exactly one owner thread holds each tree. Executions are identified by a
//! private registry generation, never by an operating-system identifier. Stop,
//! natural or unexpected root exit, identity invalidation and `shutdown_all`
//! all end in explicit `terminate_and_reap`. On Windows success means the
//! private Job Object reported no active processes. On Linux/macOS it means
//! termination was delivered to the owned process group and the retained root
//! was reaped; descendant exit is asynchronous, and descendants that leave the
//! group (setsid/setpgid) are outside this primitive. Identity checks are
//! point-in-time polling, not filesystem or network containment.
//!
//! Lock rule: the registry lock is a leaf held only for map/flag updates, never
//! across a tree operation, catalog or identity validation, audit, the launcher
//! or a completion wait. Terminal order: cleanup, generation-checked registry
//! transition, unlock, completion, then audit.
#![cfg_attr(not(test), allow(dead_code))] // Staged: `start` has no production caller until launch is approved.

use super::{Audit, DevServerTarget, ProjectCatalog};
use nexus_kernel::resource_limiter::{ResourceLimitError, ResourceLimitedChild};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::ExitStatus;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MONITOR_INTERVAL: Duration = Duration::from_millis(250);
const CLEANUP_BUDGET: Duration = Duration::from_secs(5);
const CLEANUP_ATTEMPTS: usize = 3;
const RETRY_PAUSE: Duration = Duration::from_millis(20);

/// Lifecycle operations only. Deliberately no identifier, name or handle.
pub(super) trait OwnedTree: Send {
    fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError>;
    fn terminate_and_reap(&mut self, deadline: Instant) -> Result<(), ResourceLimitError>;
}

impl OwnedTree for ResourceLimitedChild {
    fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        ResourceLimitedChild::poll_exit(self)
    }
    fn terminate_and_reap(&mut self, deadline: Instant) -> Result<(), ResourceLimitError> {
        ResourceLimitedChild::terminate_and_reap(self, deadline).map(|_| ())
    }
}

/// Private registry generation. Never serialized, audited or OS-derived.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ServerExecutionId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Finalized {
    NoOwnedServer,
    /// Stop or shutdown won before the launcher ran.
    Cancelled,
    /// The launcher failed; nothing was owned.
    NotLaunched,
    Stopped,
    Exited,
    Crashed,
    Invalidated,
    MonitorFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LifecycleError {
    NotRegistered,
    Busy,
    ShuttingDown,
    GenerationExhausted,
    Unavailable,
    /// Stop or shutdown won; any created tree is finalized by its owner.
    StopRequested,
    /// Recorded from the reserving thread itself; it cancels before launch.
    StopPending,
    OwnerUnavailable,
    LaunchFailed,
    /// The retained registration or React identity no longer validated
    /// before or during launch. The launcher was not run, or its tree was
    /// finalized without ever being published as Running.
    IdentityDenied,
    CleanupFailed,
    /// The deadline passed before finalization was confirmed. Never success.
    NotConfirmed,
}

impl LifecycleError {
    fn reason(self) -> &'static str {
        match self {
            Self::NotRegistered => "not_registered",
            Self::Busy => "busy",
            Self::ShuttingDown => "shutting_down",
            Self::GenerationExhausted => "generation_exhausted",
            Self::Unavailable => "unavailable",
            Self::StopRequested | Self::StopPending => "stop_requested",
            Self::OwnerUnavailable => "owner_unavailable",
            Self::LaunchFailed => "launch_failed",
            Self::IdentityDenied => "identity_denied",
            Self::CleanupFailed => "cleanup_failed",
            Self::NotConfirmed => "not_confirmed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LifecycleStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    CleanupFailed,
}

impl LifecycleStatus {
    /// Bounded client label; never a generation, identifier or handle.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::CleanupFailed => "cleanup_failed",
        }
    }
}

type Terminal = Result<Finalized, LifecycleError>;

#[derive(Clone, Copy)]
enum StopReason {
    Stop,
    Shutdown,
    /// Every control sender is gone (for example the registry was dropped).
    Released,
}

/// One-shot terminal result. Every wait is bounded by a caller deadline.
#[derive(Default)]
struct Completion {
    result: Mutex<Option<Terminal>>,
    ready: Condvar,
}

impl Completion {
    // Plain Copy data only, so a poisoned guard cannot expose a partial update.
    fn publish(&self, terminal: Terminal) {
        let mut result = self.result.lock().unwrap_or_else(PoisonError::into_inner);
        if result.is_none() {
            *result = Some(terminal);
        }
        drop(result);
        self.ready.notify_all();
    }

    fn wait(&self, deadline: Instant) -> Terminal {
        let mut result = self.result.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(terminal) = *result {
                return terminal;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(LifecycleError::NotConfirmed);
            }
            result = self
                .ready
                .wait_timeout(result, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

enum Slot {
    /// Reserved; the tree may not exist yet.
    Starting {
        execution: ServerExecutionId,
        starter: ThreadId,
        stop_requested: bool,
        done: Arc<Completion>,
    },
    /// Exactly one owner thread holds the tree.
    Running {
        execution: ServerExecutionId,
        control: SyncSender<StopReason>,
        done: Arc<Completion>,
    },
    /// Termination/finalization is underway by the owner or a retrying caller.
    Stopping {
        execution: ServerExecutionId,
        done: Arc<Completion>,
    },
    /// Finalization was not confirmed. The retained tree blocks a new
    /// generation until an explicit retry succeeds.
    CleanupFailed {
        execution: ServerExecutionId,
        tree: Box<dyn OwnedTree>,
    },
}

impl Slot {
    fn execution(&self) -> ServerExecutionId {
        match self {
            Self::Starting { execution, .. }
            | Self::Running { execution, .. }
            | Self::Stopping { execution, .. }
            | Self::CleanupFailed { execution, .. } => *execution,
        }
    }

    fn status(&self) -> LifecycleStatus {
        match self {
            Self::Starting { .. } => LifecycleStatus::Starting,
            Self::Running { .. } => LifecycleStatus::Running,
            Self::Stopping { .. } => LifecycleStatus::Stopping,
            Self::CleanupFailed { .. } => LifecycleStatus::CleanupFailed,
        }
    }
}

struct RegistryInner {
    accepting: bool,
    next_execution: u64,
    slots: HashMap<Uuid, Slot>,
}

struct Shared {
    state: Mutex<RegistryInner>,
    catalog: Arc<ProjectCatalog>,
}

impl Shared {
    // Only plain map/flag updates run under this lock, so a poisoned guard still
    // holds a consistent map. Recover it so a retained tree is never abandoned;
    // reservation separately refuses a poisoned registry.
    fn state(&self) -> MutexGuard<'_, RegistryInner> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Private per-authority registry keyed by registered project. Owner threads
/// hold only a Weak reference, so dropping the registry releases every control
/// channel and each owner finalizes its tree (Drop is defense in depth only).
pub(super) struct LifecycleRegistry {
    shared: Arc<Shared>,
}

enum Action {
    Vacant,
    Pending,
    Wait(Arc<Completion>),
    Signal(SyncSender<StopReason>, Arc<Completion>),
    Retry(ServerExecutionId, Box<dyn OwnedTree>, Arc<Completion>),
}

enum Abandon {
    /// Stop or shutdown won before the launcher ran.
    Cancelled,
    Failed(LifecycleError),
    /// Pre-launch identity revalidation failed (bounded reason).
    Identity(&'static str),
}

enum Published {
    Running,
    Stop(SyncSender<StopReason>, StopReason),
    Ended,
}

impl LifecycleRegistry {
    pub(super) fn new(catalog: Arc<ProjectCatalog>) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(RegistryInner {
                    accepting: true,
                    next_execution: 1,
                    slots: HashMap::new(),
                }),
                catalog,
            }),
        }
    }

    pub(super) fn status(&self, project: Uuid) -> LifecycleStatus {
        self.owned_status(project)
            .unwrap_or(LifecycleStatus::Stopped)
    }

    /// `Some` only while this registry owns an execution (any slot state) for
    /// the project key; `None` means no backend-owned execution exists.
    pub(super) fn owned_status(&self, project: Uuid) -> Option<LifecycleStatus> {
        self.shared.state().slots.get(&project).map(Slot::status)
    }

    /// Reserve a generation, run the launcher with no lock held, and hand the
    /// created tree to its single owner thread. `Ok` means ownership was
    /// transferred; later outcomes are reported through stop, status and audit.
    pub(super) fn start<L>(
        &self,
        target: DevServerTarget,
        audit: Audit,
        launch: L,
    ) -> Result<(), LifecycleError>
    where
        L: FnOnce() -> Result<Box<dyn OwnedTree>, ResourceLimitError>,
    {
        let project = target.project.project_id;
        let reserved = self
            .shared
            .catalog
            .active(&target.project)
            .map_err(|_| LifecycleError::NotRegistered)
            .and_then(|()| self.reserve(project));
        let (execution, done) = match reserved {
            Ok(reserved) => reserved,
            Err(error) => {
                lifecycle_event(&audit, Some(project), "reserve", "denied", error.reason());
                return Err(error);
            }
        };
        lifecycle_event(&audit, Some(project), "reserve", "succeeded", "reserved");
        // Reservation callbacks may have requested stop or shutdown.
        if !self.confirm_launch(project, execution) {
            return self.abandon(project, execution, &done, &audit, Abandon::Cancelled);
        }
        // Revalidate the retained target before any process can exist. Catalog
        // invalidation audits (and may re-enter) with no registry lock held.
        if let Err(reason) = validate_target(&self.shared.catalog, &target, &audit) {
            return self.abandon(project, execution, &done, &audit, Abandon::Identity(reason));
        }
        let (adopt, adoption) = sync_channel::<(Box<dyn OwnedTree>, DevServerTarget)>(1);
        let (control, control_receiver) = sync_channel::<StopReason>(1);
        let owner = Owner {
            registry: Arc::downgrade(&self.shared),
            catalog: Arc::clone(&self.shared.catalog),
            project,
            execution,
            control: control_receiver,
            done: Arc::clone(&done),
            audit: Arc::clone(&audit),
        };
        // Created before the launcher so a failure here launches nothing.
        let spawned = thread::Builder::new()
            .name("nexus-builder-lifecycle".into())
            .spawn(move || owner.run(adoption));
        if spawned.is_err() {
            let error = Abandon::Failed(LifecycleError::OwnerUnavailable);
            return self.abandon(project, execution, &done, &audit, error);
        }
        // Last lifecycle check: validation callbacks may have re-entered stop
        // or shutdown. Nothing runs between this check and the launcher.
        if !self.confirm_launch(project, execution) {
            drop(adopt);
            return self.abandon(project, execution, &done, &audit, Abandon::Cancelled);
        }
        // No lock is held. A panicking launcher is a launch failure.
        let tree = match catch_unwind(AssertUnwindSafe(launch)) {
            Ok(Ok(tree)) => tree,
            _ => {
                drop(adopt);
                let error = Abandon::Failed(LifecycleError::LaunchFailed);
                return self.abandon(project, execution, &done, &audit, error);
            }
        };
        // Close identity changes during process creation. This thread is still
        // the tree's only owner: it finalizes the tree, never publishes Running.
        if let Err(reason) = validate_target(&self.shared.catalog, &target, &audit) {
            drop(adopt);
            let ending = Ending::Invalidated(reason);
            let result = self.conclude(project, execution, tree, ending, &done, &audit);
            return Err(result.err().unwrap_or(LifecycleError::IdentityDenied));
        }
        if let Err(returned) = adopt.send((tree, target)) {
            // The owner vanished before adoption: finalize here, never drop.
            let (tree, _) = returned.0;
            let ending = Ending::Stop(StopReason::Released);
            let result = self.conclude(project, execution, tree, ending, &done, &audit);
            return Err(result.err().unwrap_or(LifecycleError::OwnerUnavailable));
        }
        match self.publish(project, execution, control) {
            Published::Running => {
                lifecycle_event(&audit, Some(project), "started", "succeeded", "running");
                Ok(())
            }
            Published::Stop(control, reason) => {
                // Stop or shutdown raced the launcher: the owner finalizes now.
                let _ = control.try_send(reason);
                Err(LifecycleError::StopRequested)
            }
            // The owner already finalized this generation (early root exit).
            Published::Ended => Ok(()),
        }
    }

    fn reserve(
        &self,
        project: Uuid,
    ) -> Result<(ServerExecutionId, Arc<Completion>), LifecycleError> {
        if self.shared.state.is_poisoned() {
            return Err(LifecycleError::Unavailable);
        }
        let mut state = self.shared.state();
        if !state.accepting {
            return Err(LifecycleError::ShuttingDown);
        }
        if state.slots.contains_key(&project) {
            return Err(LifecycleError::Busy);
        }
        let execution = ServerExecutionId(state.next_execution);
        state.next_execution = state
            .next_execution
            .checked_add(1)
            .ok_or(LifecycleError::GenerationExhausted)?;
        let done = Arc::new(Completion::default());
        state.slots.insert(
            project,
            Slot::Starting {
                execution,
                starter: thread::current().id(),
                stop_requested: false,
                done: Arc::clone(&done),
            },
        );
        Ok((execution, done))
    }

    /// Final pre-launch check, after every reservation callback has returned.
    fn confirm_launch(&self, project: Uuid, execution: ServerExecutionId) -> bool {
        let state = self.shared.state();
        state.accepting
            && matches!(state.slots.get(&project),
                Some(Slot::Starting { execution: current, stop_requested: false, .. })
                    if *current == execution)
    }

    /// Remove this generation's reservation only; nothing was launched.
    fn cancel(&self, project: Uuid, execution: ServerExecutionId) {
        let removed = {
            let mut state = self.shared.state();
            match state.slots.get(&project) {
                Some(Slot::Starting {
                    execution: current, ..
                }) if *current == execution => state.slots.remove(&project),
                _ => None,
            }
        };
        drop(removed);
    }

    fn abandon(
        &self,
        project: Uuid,
        execution: ServerExecutionId,
        done: &Completion,
        audit: &Audit,
        outcome: Abandon,
    ) -> Result<(), LifecycleError> {
        // Nothing was launched for this generation.
        let (finalized, event, error) = match outcome {
            Abandon::Cancelled => (
                Finalized::Cancelled,
                ("stopped", "succeeded", "launch_cancelled"),
                LifecycleError::StopRequested,
            ),
            Abandon::Failed(error) => (
                Finalized::NotLaunched,
                ("started", "failed", error.reason()),
                error,
            ),
            Abandon::Identity(reason) => (
                Finalized::NotLaunched,
                ("invalidated", "denied", reason),
                LifecycleError::IdentityDenied,
            ),
        };
        self.cancel(project, execution);
        done.publish(Ok(finalized));
        lifecycle_event(audit, Some(project), event.0, event.1, event.2);
        Err(error)
    }

    /// Finalize a tree the calling thread still solely owns (never adopted by
    /// an owner thread): bounded cleanup, generation-checked transition,
    /// completion, then audit. A failure parks the tree as CleanupFailed.
    fn conclude(
        &self,
        project: Uuid,
        execution: ServerExecutionId,
        mut tree: Box<dyn OwnedTree>,
        ending: Ending,
        done: &Completion,
        audit: &Audit,
    ) -> Terminal {
        let cleanup = finalize(tree.as_mut(), Instant::now() + CLEANUP_BUDGET);
        settle(&self.shared, project, execution, cleanup.is_ok(), tree);
        let (result, event) = conclusion(ending, cleanup);
        done.publish(result);
        lifecycle_event(audit, Some(project), event.0, event.1, event.2);
        result
    }

    fn publish(
        &self,
        project: Uuid,
        execution: ServerExecutionId,
        control: SyncSender<StopReason>,
    ) -> Published {
        let mut state = self.shared.state();
        let accepting = state.accepting;
        let (stop_requested, done) = match state.slots.get(&project) {
            Some(Slot::Starting {
                execution: current,
                stop_requested,
                done,
                ..
            }) if *current == execution => (*stop_requested, Arc::clone(done)),
            // Another generation or already finalized: never mutate it.
            _ => return Published::Ended,
        };
        if stop_requested || !accepting {
            state
                .slots
                .insert(project, Slot::Stopping { execution, done });
            let reason = if accepting {
                StopReason::Stop
            } else {
                StopReason::Shutdown
            };
            Published::Stop(control, reason)
        } else {
            state.slots.insert(
                project,
                Slot::Running {
                    execution,
                    control,
                    done,
                },
            );
            Published::Running
        }
    }

    /// Returns success only after explicit native finalization, or when no
    /// owned tree exists. A passed deadline is `NotConfirmed`, never success.
    pub(super) fn stop(&self, project: Uuid, deadline: Instant, audit: &Audit) -> Terminal {
        let action = request(&mut self.shared.state().slots, project);
        match action {
            Action::Vacant => {
                lifecycle_event(audit, Some(project), "stop", "succeeded", "no_owned_server");
                Ok(Finalized::NoOwnedServer)
            }
            Action::Pending => {
                lifecycle_event(audit, Some(project), "stop", "requested", "stop_requested");
                Err(LifecycleError::StopPending)
            }
            Action::Wait(done) => {
                lifecycle_event(audit, Some(project), "stop", "requested", "stop_requested");
                done.wait(deadline)
            }
            Action::Signal(control, done) => {
                let _ = control.try_send(StopReason::Stop);
                drop(control);
                lifecycle_event(audit, Some(project), "stop", "requested", "stop_requested");
                done.wait(deadline)
            }
            Action::Retry(execution, tree, done) => {
                lifecycle_event(audit, Some(project), "stop", "requested", "cleanup_retry");
                self.retry(project, execution, tree, &done, deadline, audit)
            }
        }
    }

    /// Permanently stops accepting, requests every owner to stop, retries
    /// retained trees and waits against one overall deadline. Repeatable.
    pub(super) fn shutdown_all(
        &self,
        deadline: Instant,
        audit: &Audit,
    ) -> Result<(), LifecycleError> {
        let actions: Vec<(Uuid, Action)> = {
            let mut state = self.shared.state();
            state.accepting = false;
            let projects: Vec<Uuid> = state.slots.keys().copied().collect();
            projects
                .into_iter()
                .map(|project| (project, request(&mut state.slots, project)))
                .collect()
        };
        lifecycle_event(audit, None, "shutdown", "requested", "shutdown");
        let mut confirmed = true;
        let mut waits = Vec::new();
        let mut retries = Vec::new();
        for (project, action) in actions {
            match action {
                Action::Vacant => {}
                // The reserving thread re-entered; it cancels before launch,
                // but that is not yet confirmed here.
                Action::Pending => confirmed = false,
                Action::Wait(done) => waits.push(done),
                Action::Signal(control, done) => {
                    let _ = control.try_send(StopReason::Shutdown);
                    waits.push(done);
                }
                Action::Retry(execution, tree, done) => {
                    retries.push((project, execution, tree, done))
                }
            }
        }
        for (project, execution, tree, done) in retries {
            confirmed &= self
                .retry(project, execution, tree, &done, deadline, audit)
                .is_ok();
        }
        for done in waits {
            confirmed &= done.wait(deadline).is_ok();
        }
        let outcome = if confirmed { "succeeded" } else { "failed" };
        lifecycle_event(audit, None, "shutdown", outcome, "shutdown");
        if confirmed {
            Ok(())
        } else {
            Err(LifecycleError::NotConfirmed)
        }
    }

    /// Explicit retry by the single caller that took the retained tree.
    fn retry(
        &self,
        project: Uuid,
        execution: ServerExecutionId,
        mut tree: Box<dyn OwnedTree>,
        done: &Completion,
        deadline: Instant,
        audit: &Audit,
    ) -> Terminal {
        let cleanup = finalize(tree.as_mut(), deadline.min(Instant::now() + CLEANUP_BUDGET));
        settle(&self.shared, project, execution, cleanup.is_ok(), tree);
        let (result, event) = conclusion(Ending::Stop(StopReason::Stop), cleanup);
        done.publish(result);
        let reason = if cleanup.is_ok() {
            "cleanup_retried"
        } else {
            event.2
        };
        lifecycle_event(audit, Some(project), event.0, event.1, reason);
        result
    }
}

/// Stop request under the registry lock (map updates only).
fn request(slots: &mut HashMap<Uuid, Slot>, project: Uuid) -> Action {
    let Some(slot) = slots.remove(&project) else {
        return Action::Vacant;
    };
    let (slot, action) = match slot {
        Slot::Starting {
            execution,
            starter,
            done,
            ..
        } => {
            let action = if starter == thread::current().id() {
                Action::Pending
            } else {
                Action::Wait(Arc::clone(&done))
            };
            let slot = Slot::Starting {
                execution,
                starter,
                stop_requested: true,
                done,
            };
            (slot, action)
        }
        Slot::Running {
            execution,
            control,
            done,
        } => {
            let slot = Slot::Stopping {
                execution,
                done: Arc::clone(&done),
            };
            (slot, Action::Signal(control, done))
        }
        Slot::Stopping { execution, done } => {
            let action = Action::Wait(Arc::clone(&done));
            (Slot::Stopping { execution, done }, action)
        }
        Slot::CleanupFailed { execution, tree } => {
            let done = Arc::new(Completion::default());
            let slot = Slot::Stopping {
                execution,
                done: Arc::clone(&done),
            };
            (slot, Action::Retry(execution, tree, done))
        }
    };
    slots.insert(project, slot);
    action
}

/// Generation-checked terminal transition: confirmed cleanup removes this
/// execution's slot; failure parks the retained tree for an explicit retry.
/// Another generation's slot is never touched (and cannot exist while this
/// generation's owner or retrier holds the tree).
fn settle(
    shared: &Shared,
    project: Uuid,
    execution: ServerExecutionId,
    confirmed: bool,
    tree: Box<dyn OwnedTree>,
) {
    let released = {
        let mut state = shared.state();
        match state.slots.get(&project) {
            Some(slot) if slot.execution() == execution => {
                if confirmed {
                    (state.slots.remove(&project), Some(tree))
                } else {
                    let parked = Slot::CleanupFailed { execution, tree };
                    (state.slots.insert(project, parked), None)
                }
            }
            _ => (None, Some(tree)),
        }
    };
    drop(released); // Slots and finalized trees are released outside the lock.
}

#[derive(Clone, Copy)]
struct CleanupFailure(&'static str);

/// Bounded explicit finalization. Each attempt runs outside any lock; a panic
/// is contained so the caller still owns the tree.
fn finalize(tree: &mut dyn OwnedTree, deadline: Instant) -> Result<(), CleanupFailure> {
    let mut failure = CleanupFailure("deadline_exceeded");
    for attempt in 0..CLEANUP_ATTEMPTS {
        if attempt > 0 {
            thread::sleep(RETRY_PAUSE.min(deadline.saturating_duration_since(Instant::now())));
        }
        match catch_unwind(AssertUnwindSafe(|| tree.terminate_and_reap(deadline))) {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => {
                failure = CleanupFailure(match error {
                    ResourceLimitError::TerminationFailed(_) => "termination_failed",
                    ResourceLimitError::CleanupDeadlineExceeded => "deadline_exceeded",
                    ResourceLimitError::ObservationFailed(_) => "observation_failed",
                    _ => "cleanup_error",
                })
            }
            Err(_) => failure = CleanupFailure("panicked"),
        }
    }
    Err(failure)
}

enum Ending {
    Stop(StopReason),
    Exited { success: bool },
    Invalidated(&'static str),
    MonitorFailure,
}

type Event = (&'static str, &'static str, &'static str);

fn conclusion(ending: Ending, cleanup: Result<(), CleanupFailure>) -> (Terminal, Event) {
    if let Err(CleanupFailure(reason)) = cleanup {
        return (
            Err(LifecycleError::CleanupFailed),
            ("cleanup_failed", "failed", reason),
        );
    }
    let (finalized, event) = match ending {
        Ending::Stop(StopReason::Stop) => (Finalized::Stopped, ("stopped", "stop_requested")),
        Ending::Stop(StopReason::Shutdown) => (Finalized::Stopped, ("stopped", "shutdown")),
        Ending::Stop(StopReason::Released) => (Finalized::Stopped, ("stopped", "released")),
        Ending::Exited { success: true } => (Finalized::Exited, ("exited", "exited")),
        Ending::Exited { success: false } => (Finalized::Crashed, ("exited", "crashed")),
        Ending::Invalidated(reason) => (Finalized::Invalidated, ("invalidated", reason)),
        Ending::MonitorFailure => (Finalized::MonitorFailure, ("stopped", "monitor_failure")),
    };
    (Ok(finalized), (event.0, "succeeded", event.1))
}

struct Owner {
    registry: Weak<Shared>,
    catalog: Arc<ProjectCatalog>,
    project: Uuid,
    execution: ServerExecutionId,
    control: Receiver<StopReason>,
    done: Arc<Completion>,
    audit: Audit,
}

impl Owner {
    fn run(self, adoption: Receiver<(Box<dyn OwnedTree>, DevServerTarget)>) {
        // No tree: the launcher failed or never ran, or start finalized the
        // tree after failed revalidation; start published the outcome.
        let Ok((mut tree, target)) = adoption.recv() else {
            return;
        };
        // Catalog events raised while monitoring are deferred until after the
        // terminal completion, so a re-entrant callback never waits on us.
        let deferred = Arc::new(Mutex::new(Vec::<Value>::new()));
        let buffer: Audit = {
            let deferred = Arc::clone(&deferred);
            Arc::new(move |event| {
                deferred
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(event)
            })
        };
        // The tree stays outside the unwind boundary: a panic cannot lose it.
        let ending = catch_unwind(AssertUnwindSafe(|| {
            self.monitor(tree.as_mut(), &target, &buffer)
        }))
        .unwrap_or(Ending::MonitorFailure);
        // Natural exit is not cleanup: always terminate and finalize explicitly.
        let cleanup = finalize(tree.as_mut(), Instant::now() + CLEANUP_BUDGET);
        let (result, event) = conclusion(ending, cleanup);
        match self.registry.upgrade() {
            Some(shared) => settle(&shared, self.project, self.execution, cleanup.is_ok(), tree),
            // Registry dropped: nothing can retain the tree; its Drop is
            // defense in depth only and is not reported as success.
            None => drop(tree),
        }
        self.done.publish(result);
        let events = std::mem::take(&mut *deferred.lock().unwrap_or_else(PoisonError::into_inner));
        for payload in events {
            let _ = catch_unwind(AssertUnwindSafe(|| (self.audit)(payload)));
        }
        lifecycle_event(&self.audit, Some(self.project), event.0, event.1, event.2);
    }

    fn monitor(&self, tree: &mut dyn OwnedTree, target: &DevServerTarget, audit: &Audit) -> Ending {
        loop {
            if let Some(reason) = self.pending() {
                return Ending::Stop(reason);
            }
            match tree.poll_exit() {
                Ok(Some(status)) => {
                    return Ending::Exited {
                        success: status.success(),
                    }
                }
                Ok(None) => {}
                Err(_) => return Ending::MonitorFailure,
            }
            // A pending stop is honored before another identity cycle.
            if let Some(reason) = self.pending() {
                return Ending::Stop(reason);
            }
            if let Err(reason) = validate_target(&self.catalog, target, audit) {
                return Ending::Invalidated(reason);
            }
            match self.control.recv_timeout(MONITOR_INTERVAL) {
                Ok(reason) => return Ending::Stop(reason),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ending::Stop(StopReason::Released),
            }
        }
    }

    fn pending(&self) -> Option<StopReason> {
        match self.control.try_recv() {
            Ok(reason) => Some(reason),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(StopReason::Released),
        }
    }
}

/// Point-in-time revalidation of the retained target: the current registration
/// with storage/project identity (C3 tombstones on Changed), then the retained
/// React identity (never tombstones). Pathname identity, not containment.
/// A panicking invalidation audit cannot unwind out and strand a reservation.
fn validate_target(
    catalog: &ProjectCatalog,
    target: &DevServerTarget,
    audit: &Audit,
) -> Result<(), &'static str> {
    let guarded: Audit = {
        let audit = Arc::clone(audit);
        Arc::new(move |event| {
            let _ = catch_unwind(AssertUnwindSafe(|| audit(event)));
        })
    };
    catalog
        .validate(&target.project, &guarded)
        .map_err(|_| "registration")?;
    target.validate_react().map_err(|_| "react")
}

// Bounded categories only: never generations, identifiers, paths, handles,
// arguments, environment or exit codes. An audit panic never interrupts
// ownership, cleanup or state publication.
fn lifecycle_event(
    audit: &Audit,
    project: Option<Uuid>,
    operation: &str,
    outcome: &str,
    reason: &str,
) {
    let payload = json!({"operation": format!("builder.devserver.lifecycle.{operation}"),
        "project_id": project.map(|id| id.to_string()), "outcome": outcome, "reason": reason});
    let _ = catch_unwind(AssertUnwindSafe(|| audit(payload)));
}

#[cfg(test)]
mod tests;
