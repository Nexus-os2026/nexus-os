//! P0-002C4B lifecycle tests. Real processes are created only here, by trusted
//! repository test code: the current test executable (absolute path, no PATH,
//! shell or network beyond loopback) re-run as ignored libtest fixtures.
use super::super::tests::{registered, Fixture};
use super::*;
use nexus_kernel::resource_limiter::{
    ResourceLimiter, ResourceOutput, ResourceProgram, ResourceReader, ResourceSpawnSpec,
    ResourceStdin,
};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

const WAIT: Duration = Duration::from_secs(20);
const FIXTURE_PREFIX: &str = "builder_workspace::process_lifecycle::tests::";
const CONTROL_MARKER: &str = "c4b-fixture-control";
const DESCENDANT_READY: &str = "c4b-descendant-ready";
// Finite fixtures: even broken cleanup never leaves a permanent process.
const ROOT_EXPIRY: Duration = Duration::from_secs(60);
const DESCENDANT_EXPIRY: Duration = Duration::from_secs(90);
const DELAYED_EXIT: Duration = Duration::from_millis(1000);

fn soon() -> Instant {
    Instant::now() + WAIT
}

fn wait_until(deadline: Instant, mut done: impl FnMut() -> bool) -> bool {
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn uuid(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap()
}

fn registry(f: &Fixture) -> LifecycleRegistry {
    LifecycleRegistry::new(Arc::clone(&f.authority.catalog))
}

fn target(f: &Fixture, id: &str) -> DevServerTarget {
    f.authority.dev_server_target(id, &f.audit()).unwrap()
}

fn execution_of(registry: &LifecycleRegistry, project: Uuid) -> Option<u64> {
    registry
        .shared
        .state()
        .slots
        .get(&project)
        .map(|slot| slot.execution().0)
}

fn quiet() -> Audit {
    Arc::new(|_| {})
}

fn lifecycle_events(f: &Fixture) -> Vec<Value> {
    f.events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| {
            event["operation"]
                .as_str()
                .is_some_and(|op| op.starts_with("builder.devserver.lifecycle."))
        })
        .cloned()
        .collect()
}

fn wait_event(f: &Fixture, operation: &str, reason: &str) -> bool {
    let operation = format!("builder.devserver.lifecycle.{operation}");
    wait_until(soon(), || {
        lifecycle_events(f)
            .iter()
            .any(|event| event["operation"] == operation.as_str() && event["reason"] == reason)
    })
}

// ── Fake owned tree for deterministic failure injection ───────────────────

#[derive(Clone, Copy)]
enum Fail {
    Termination,
    Deadline,
    Panic,
    /// Stand-in for lost Unix ownership (ECHILD): never recoverable.
    Lost,
}

#[derive(Default)]
struct FakeControl {
    exit: Mutex<Option<bool>>,
    poll_panics: AtomicBool,
    permanent: Mutex<Option<Fail>>,
    terminate_delay: Mutex<Duration>,
    polls: AtomicUsize,
    terminations: AtomicUsize,
    finalized: AtomicBool,
    dropped: AtomicBool,
    probe: Mutex<Option<(Weak<Shared>, Arc<ProjectCatalog>)>>,
    violations: AtomicUsize,
}

// True only if the lock stays unavailable for ~100 ms. Other threads hold these
// locks for microseconds; a lock held by the probing thread itself never frees.
fn stays_locked<T>(mutex: &Mutex<T>) -> bool {
    (0..50).all(|_| {
        let held = mutex.try_lock().is_err();
        if held {
            thread::sleep(Duration::from_millis(2));
        }
        held
    })
}

impl FakeControl {
    // Records a violation if the registry or catalog lock is held across the
    // owner's tree operation.
    fn probe_locks(&self) {
        let probe = self.probe.lock().unwrap();
        if let Some((shared, catalog)) = probe.as_ref() {
            if shared
                .upgrade()
                .is_some_and(|shared| stays_locked(&shared.state))
            {
                self.violations.fetch_add(1, Ordering::SeqCst);
            }
            if stays_locked(&catalog.projects) {
                self.violations.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

struct Fake(Arc<FakeControl>);

impl OwnedTree for Fake {
    fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        self.0.probe_locks();
        self.0.polls.fetch_add(1, Ordering::SeqCst);
        if self.0.poll_panics.load(Ordering::SeqCst) {
            panic!("injected poll_exit panic");
        }
        Ok(self.0.exit.lock().unwrap().map(status))
    }

    fn terminate_and_reap(&mut self, _deadline: Instant) -> Result<(), ResourceLimitError> {
        self.0.probe_locks();
        self.0.terminations.fetch_add(1, Ordering::SeqCst);
        if self.0.finalized.load(Ordering::SeqCst) {
            return Ok(());
        }
        let delay = *self.0.terminate_delay.lock().unwrap();
        thread::sleep(delay);
        let failure = *self.0.permanent.lock().unwrap();
        match failure {
            None => {
                self.0.finalized.store(true, Ordering::SeqCst);
                Ok(())
            }
            Some(Fail::Termination) => Err(ResourceLimitError::TerminationFailed(
                std::io::Error::from_raw_os_error(1),
            )),
            Some(Fail::Deadline) => Err(ResourceLimitError::CleanupDeadlineExceeded),
            Some(Fail::Lost) => Err(ResourceLimitError::TerminationFailed(
                std::io::Error::from_raw_os_error(10),
            )),
            Some(Fail::Panic) => panic!("injected terminate_and_reap panic"),
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::SeqCst);
    }
}

fn status(success: bool) -> ExitStatus {
    if success {
        return ExitStatus::default();
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(3 << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(3)
    }
}

type Launch = Result<Box<dyn OwnedTree>, ResourceLimitError>;

fn fake(control: &Arc<FakeControl>) -> impl FnOnce() -> Launch {
    let control = Arc::clone(control);
    move || Ok(Box::new(Fake(control)) as Box<dyn OwnedTree>)
}

fn counted(launched: &Arc<AtomicUsize>, control: &Arc<FakeControl>) -> impl FnOnce() -> Launch {
    let launched = Arc::clone(launched);
    let launch = fake(control);
    move || {
        launched.fetch_add(1, Ordering::SeqCst);
        launch()
    }
}

// ── Ownership traits ──────────────────────────────────────────────────────

#[test]
fn p0_002c4b_resource_limited_child_is_send_and_owned_tree_exposes_no_identity() {
    fn assert_send<T: Send>() {}
    assert_send::<ResourceLimitedChild>();
    assert_send::<Box<dyn OwnedTree>>();
    assert_send::<Owner>();
    let source = include_str!("../process_lifecycle.rs");
    let start = source.find("pub(super) trait OwnedTree: Send {").unwrap();
    let body = &source[start..start + source[start..].find("\n}").unwrap()];
    assert_eq!(body.matches("fn ").count(), 2, "{body}");
    assert!(body.contains("fn poll_exit(&mut self)"));
    assert!(body.contains("fn terminate_and_reap(&mut self, deadline: Instant)"));
    for forbidden in ["id(", "pid", "handle", "name", "Sync"] {
        assert!(!body.contains(forbidden), "{forbidden}");
    }
}

// ── Registry / generations ────────────────────────────────────────────────

#[test]
fn p0_002c4b_generations_are_monotonic_distinct_and_fail_closed_on_overflow() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Arc::new(FakeControl::default());
    let launched = Arc::new(AtomicUsize::new(0));
    r.start(target(&f, &id), f.audit(), counted(&launched, &control))
        .unwrap();
    let first = execution_of(&r, p).unwrap();
    assert_eq!(r.status(p), LifecycleStatus::Running);
    // Duplicate reservation is denied deterministically and never launches.
    assert_eq!(
        r.start(target(&f, &id), f.audit(), counted(&launched, &control)),
        Err(LifecycleError::Busy)
    );
    assert_eq!(launched.load(Ordering::SeqCst), 1);
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    assert!(control.finalized.load(Ordering::SeqCst));
    let second_control = Arc::new(FakeControl::default());
    r.start(target(&f, &id), f.audit(), fake(&second_control))
        .unwrap();
    let second = execution_of(&r, p).unwrap();
    assert!(second > first, "generations must be monotonic and distinct");
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    // The last representable generation is used; the next overflow fails closed.
    r.shared.state().next_execution = u64::MAX - 1;
    r.start(
        target(&f, &id),
        f.audit(),
        fake(&Arc::new(FakeControl::default())),
    )
    .unwrap();
    assert_eq!(execution_of(&r, p), Some(u64::MAX - 1));
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    assert_eq!(
        r.start(target(&f, &id), f.audit(), counted(&launched, &control)),
        Err(LifecycleError::GenerationExhausted)
    );
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert_eq!(r.shared.state().next_execution, u64::MAX);
    assert_eq!(launched.load(Ordering::SeqCst), 1);
    assert!(wait_event(&f, "reserve", "generation_exhausted"));
}

#[test]
fn p0_002c4b_concurrent_reservations_have_exactly_one_winner() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Arc::new(FakeControl::default());
    let launched = Arc::new(AtomicUsize::new(0));
    let targets: Vec<_> = (0..8).map(|_| target(&f, &id)).collect();
    let barrier = std::sync::Barrier::new(targets.len());
    let results: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = targets
            .into_iter()
            .map(|target| {
                let (r, barrier, audit) = (&r, &barrier, f.audit());
                let launch = counted(&launched, &control);
                scope.spawn(move || {
                    barrier.wait();
                    r.start(target, audit, launch)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(LifecycleError::Busy))
            .count(),
        7
    );
    assert_eq!(launched.load(Ordering::SeqCst), 1);
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
}

#[test]
fn p0_002c4b_stale_generations_cannot_publish_cancel_or_settle() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let (current, done) = r.reserve(p).unwrap();
    let stale = ServerExecutionId(current.0 - 1);
    let (control, _receiver) = sync_channel::<StopReason>(1);
    assert!(matches!(r.publish(p, stale, control), Published::Ended));
    r.cancel(p, stale);
    for confirmed in [true, false] {
        let tree = Arc::new(FakeControl::default());
        settle(
            &r.shared,
            p,
            stale,
            confirmed,
            Box::new(Fake(Arc::clone(&tree))),
        );
        // The stale caller never parks, removes or overwrites the newer slot.
        assert!(tree.dropped.load(Ordering::SeqCst));
    }
    assert_eq!(r.status(p), LifecycleStatus::Starting);
    assert_eq!(execution_of(&r, p), Some(current.0));
    // Nothing stale completed the current generation.
    assert_eq!(done.wait(Instant::now()), Err(LifecycleError::NotConfirmed));
    r.cancel(p, current);
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
}

// ── Cleanup failure / retry ───────────────────────────────────────────────

#[test]
fn p0_002c4b_cleanup_failure_is_retained_blocks_restart_and_can_be_retried() {
    for (failure, reason) in [
        (Fail::Termination, "termination_failed"),
        (Fail::Deadline, "deadline_exceeded"),
    ] {
        let f = Fixture::new();
        let (id, _) = registered(&f);
        let (r, p) = (registry(&f), uuid(&id));
        let control = Arc::new(FakeControl::default());
        *control.permanent.lock().unwrap() = Some(failure);
        r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
        assert_eq!(
            r.stop(p, soon(), &f.audit()),
            Err(LifecycleError::CleanupFailed)
        );
        assert_eq!(r.status(p), LifecycleStatus::CleanupFailed);
        assert_eq!(
            control.terminations.load(Ordering::SeqCst),
            CLEANUP_ATTEMPTS
        );
        assert!(
            !control.dropped.load(Ordering::SeqCst),
            "tree ownership lost"
        );
        assert!(wait_event(&f, "cleanup_failed", reason));
        // CleanupFailed blocks a new generation; no automatic overwrite.
        let launched = Arc::new(AtomicUsize::new(0));
        assert_eq!(
            r.start(target(&f, &id), f.audit(), counted(&launched, &control)),
            Err(LifecycleError::Busy)
        );
        assert_eq!(launched.load(Ordering::SeqCst), 0);
        // An explicit retry takes the retained tree and can succeed.
        *control.permanent.lock().unwrap() = None;
        assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
        assert!(control.finalized.load(Ordering::SeqCst));
        assert_eq!(r.status(p), LifecycleStatus::Stopped);
        assert!(wait_event(&f, "stopped", "cleanup_retried"));
        r.start(
            target(&f, &id),
            f.audit(),
            fake(&Arc::new(FakeControl::default())),
        )
        .unwrap();
        assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    }
}

#[test]
fn p0_002c4b_permanent_ownership_loss_never_becomes_success() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Arc::new(FakeControl::default());
    *control.permanent.lock().unwrap() = Some(Fail::Lost);
    r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
    for _ in 0..3 {
        assert_eq!(
            r.stop(p, soon(), &f.audit()),
            Err(LifecycleError::CleanupFailed)
        );
        assert_eq!(r.status(p), LifecycleStatus::CleanupFailed);
    }
    for _ in 0..2 {
        assert_eq!(
            r.shutdown_all(soon(), &f.audit()),
            Err(LifecycleError::NotConfirmed)
        );
        assert_eq!(r.status(p), LifecycleStatus::CleanupFailed);
    }
    assert!(!control.dropped.load(Ordering::SeqCst));
    assert!(!control.finalized.load(Ordering::SeqCst));
    // A later shutdown retries the retained tree and can then succeed.
    *control.permanent.lock().unwrap() = None;
    assert_eq!(r.shutdown_all(soon(), &f.audit()), Ok(()));
    assert!(control.finalized.load(Ordering::SeqCst));
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert!(wait_event(&f, "shutdown", "shutdown"));
}

#[test]
fn p0_002c4b_owned_tree_panics_do_not_lose_ownership() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    // A monitor panic still finalizes the retained tree explicitly.
    let control = Arc::new(FakeControl::default());
    control.poll_panics.store(true, Ordering::SeqCst);
    r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
    assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
    assert!(control.finalized.load(Ordering::SeqCst));
    assert!(wait_event(&f, "stopped", "monitor_failure"));
    // A finalization panic retains the tree for an explicit retry.
    let control = Arc::new(FakeControl::default());
    *control.permanent.lock().unwrap() = Some(Fail::Panic);
    r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
    assert_eq!(
        r.stop(p, soon(), &f.audit()),
        Err(LifecycleError::CleanupFailed)
    );
    assert_eq!(r.status(p), LifecycleStatus::CleanupFailed);
    assert!(
        !control.dropped.load(Ordering::SeqCst),
        "tree ownership lost"
    );
    assert!(wait_event(&f, "cleanup_failed", "panicked"));
    *control.permanent.lock().unwrap() = None;
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    assert!(control.finalized.load(Ordering::SeqCst));
}

// ── Stop semantics ────────────────────────────────────────────────────────

#[test]
fn p0_002c4b_waiter_deadline_is_never_success_and_duplicate_stop_shares_result() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Arc::new(FakeControl::default());
    *control.terminate_delay.lock().unwrap() = Duration::from_millis(1500);
    r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
    assert_eq!(
        r.stop(p, Instant::now() + Duration::from_millis(100), &f.audit()),
        Err(LifecycleError::NotConfirmed)
    );
    assert_eq!(r.status(p), LifecycleStatus::Stopping);
    // Duplicate callers wait on the same completion of the same generation.
    let results: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = (0..3)
            .map(|_| scope.spawn(|| r.stop(p, soon(), &quiet())))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(
        results.iter().all(|r| *r == Ok(Finalized::Stopped)),
        "{results:?}"
    );
    assert_eq!(control.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::NoOwnedServer));
}

#[test]
fn p0_002c4b_stop_racing_natural_exit_finalizes_exactly_once() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    for _ in 0..5 {
        let control = Arc::new(FakeControl::default());
        r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
        *control.exit.lock().unwrap() = Some(true);
        let result = r.stop(p, soon(), &f.audit());
        assert!(
            matches!(result, Ok(Finalized::Stopped | Finalized::Exited)),
            "{result:?}"
        );
        assert_eq!(control.terminations.load(Ordering::SeqCst), 1);
        assert!(control.finalized.load(Ordering::SeqCst));
        assert_eq!(r.status(p), LifecycleStatus::Stopped);
    }
}

// ── Starting races ────────────────────────────────────────────────────────

#[test]
fn p0_002c4b_stop_or_shutdown_before_final_launch_check_prevents_launch() {
    for shutdown in [false, true] {
        let f = Fixture::new();
        let (id, _) = registered(&f);
        let p = uuid(&id);
        let r = Arc::new(registry(&f));
        let reentry = Arc::new(Mutex::new(None));
        let audit: Audit = {
            let (weak, reentry, events) = (
                Arc::downgrade(&r),
                Arc::clone(&reentry),
                Arc::clone(&f.events),
            );
            Arc::new(move |event: Value| {
                let reserved = event["operation"] == "builder.devserver.lifecycle.reserve"
                    && event["outcome"] == "succeeded";
                events.lock().unwrap().push(event);
                if reserved {
                    let r = weak.upgrade().unwrap();
                    let result = if shutdown {
                        r.shutdown_all(soon(), &quiet())
                            .map(|()| Finalized::Stopped)
                    } else {
                        r.stop(p, soon(), &quiet())
                    };
                    *reentry.lock().unwrap() = Some(result);
                }
            })
        };
        let launched = Arc::new(AtomicUsize::new(0));
        let control = Arc::new(FakeControl::default());
        assert_eq!(
            r.start(target(&f, &id), audit, counted(&launched, &control)),
            Err(LifecycleError::StopRequested)
        );
        assert_eq!(
            launched.load(Ordering::SeqCst),
            0,
            "launch was not prevented"
        );
        let expected = if shutdown {
            Err(LifecycleError::NotConfirmed)
        } else {
            Err(LifecycleError::StopPending)
        };
        assert_eq!(*reentry.lock().unwrap(), Some(expected));
        assert_eq!(r.status(p), LifecycleStatus::Stopped);
        assert!(wait_event(&f, "stopped", "launch_cancelled"));
        if shutdown {
            assert_eq!(
                r.start(target(&f, &id), f.audit(), counted(&launched, &control)),
                Err(LifecycleError::ShuttingDown)
            );
            assert_eq!(r.shutdown_all(soon(), &f.audit()), Ok(()));
            assert_eq!(launched.load(Ordering::SeqCst), 0);
        }
    }
}

#[test]
fn p0_002c4b_stop_or_shutdown_racing_launcher_finalizes_immediately() {
    for shutdown in [false, true] {
        let f = Fixture::new();
        let (id, _) = registered(&f);
        let p = uuid(&id);
        let r = Arc::new(registry(&f));
        let control = Arc::new(FakeControl::default());
        let racer = Arc::new(Mutex::new(None));
        let launch = {
            let (r, racer, tree) = (Arc::clone(&r), Arc::clone(&racer), fake(&control));
            move || {
                // Runs after the final pre-launch check, with no lock held.
                let requester = Arc::clone(&r);
                *racer.lock().unwrap() = Some(thread::spawn(move || {
                    if shutdown {
                        requester
                            .shutdown_all(soon(), &quiet())
                            .map(|()| Finalized::Stopped)
                    } else {
                        requester.stop(p, soon(), &quiet())
                    }
                }));
                assert!(wait_until(soon(), || matches!(
                    r.shared.state().slots.get(&p),
                    Some(Slot::Starting {
                        stop_requested: true,
                        ..
                    })
                )));
                tree()
            }
        };
        assert_eq!(
            r.start(target(&f, &id), f.audit(), launch),
            Err(LifecycleError::StopRequested)
        );
        let raced = racer.lock().unwrap().take().unwrap().join().unwrap();
        assert_eq!(raced, Ok(Finalized::Stopped));
        // The just-created tree was handed over and finalized, never leaked.
        assert!(control.finalized.load(Ordering::SeqCst));
        assert_eq!(r.status(p), LifecycleStatus::Stopped);
        let reason = if shutdown {
            "shutdown"
        } else {
            "stop_requested"
        };
        assert!(wait_event(&f, "stopped", reason));
    }
}

#[test]
fn p0_002c4b_launch_failure_and_panic_clear_only_their_generation() {
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let failed = || -> Launch {
        Err(ResourceLimitError::SpawnFailed(std::io::Error::other(
            "injected launch failure",
        )))
    };
    assert_eq!(
        r.start(target(&f, &id), f.audit(), failed),
        Err(LifecycleError::LaunchFailed)
    );
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    let panicked = || -> Launch { panic!("injected launcher panic") };
    assert_eq!(
        r.start(target(&f, &id), f.audit(), panicked),
        Err(LifecycleError::LaunchFailed)
    );
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert!(wait_event(&f, "started", "launch_failed"));
    // The project remains usable: a new generation starts and stops.
    let control = Arc::new(FakeControl::default());
    r.start(target(&f, &id), f.audit(), fake(&control)).unwrap();
    assert_eq!(execution_of(&r, p), Some(3));
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    // A registry is bound to its own catalog's current registrations.
    let other = Fixture::new();
    let (other_id, _) = registered(&other);
    assert_eq!(
        r.start(target(&other, &other_id), f.audit(), fake(&control)),
        Err(LifecycleError::NotRegistered)
    );
}

// ── Audit re-entry / locking ──────────────────────────────────────────────

fn terminal(event: &Value) -> bool {
    ["stopped", "exited", "invalidated", "cleanup_failed"]
        .iter()
        .any(|op| event["operation"] == format!("builder.devserver.lifecycle.{op}").as_str())
}

// Runs a scenario on its own thread so a deadlock fails instead of hanging.
fn within<T: Send + 'static>(scenario: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(scenario());
    });
    receiver
        .recv_timeout(Duration::from_secs(90))
        .expect("lifecycle scenario deadlocked or panicked")
}

#[test]
fn p0_002c4b_audit_reentry_locks_and_completion_order() {
    within(|| {
        let f = Fixture::new();
        let (id, _) = registered(&f);
        let p = uuid(&id);
        let r = Arc::new(registry(&f));
        let done = Arc::new(Mutex::new(None::<Arc<Completion>>));
        let observations = Arc::new(Mutex::new(Vec::<(bool, LifecycleStatus, Terminal)>::new()));
        let violations = Arc::new(AtomicUsize::new(0));
        let reenter_shutdown = Arc::new(AtomicBool::new(false));
        let audit: Audit = {
            let weak = Arc::downgrade(&r);
            let catalog = Arc::clone(&f.authority.catalog);
            let (done, observations, violations, reenter_shutdown, events) = (
                Arc::clone(&done),
                Arc::clone(&observations),
                Arc::clone(&violations),
                Arc::clone(&reenter_shutdown),
                Arc::clone(&f.events),
            );
            Arc::new(move |event: Value| {
                let r = weak.upgrade().unwrap();
                // No audit is ever emitted with the registry or catalog locked.
                if stays_locked(&r.shared.state) || stays_locked(&catalog.projects) {
                    violations.fetch_add(1, Ordering::SeqCst);
                }
                let is_terminal = terminal(&event);
                events.lock().unwrap().push(event);
                if is_terminal {
                    let published = done
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|done| done.result.lock().unwrap().is_some());
                    // Re-entry into status-like queries and stop cannot wait on
                    // the owner whose completion is already published.
                    let status = r.status(p);
                    let stopped = r.stop(p, soon(), &quiet());
                    observations
                        .lock()
                        .unwrap()
                        .push((published, status, stopped));
                    if reenter_shutdown.load(Ordering::SeqCst) {
                        assert_eq!(r.shutdown_all(soon(), &quiet()), Ok(()));
                    }
                }
            })
        };
        let completion_of = |r: &LifecycleRegistry| match r.shared.state().slots.get(&p) {
            Some(Slot::Running { done, .. }) => Arc::clone(done),
            _ => panic!("expected a running generation"),
        };
        // Stop path, with lock probes inside every tree operation.
        let control = Arc::new(FakeControl::default());
        *control.probe.lock().unwrap() =
            Some((Arc::downgrade(&r.shared), Arc::clone(&f.authority.catalog)));
        r.start(target(&f, &id), Arc::clone(&audit), fake(&control))
            .unwrap();
        *done.lock().unwrap() = Some(completion_of(&r));
        assert!(wait_until(soon(), || control.polls.load(Ordering::SeqCst) >= 3));
        assert_eq!(r.stop(p, soon(), &audit), Ok(Finalized::Stopped));
        // Natural-exit path whose terminal audit re-enters shutdown_all.
        assert!(wait_until(soon(), || observations.lock().unwrap().len() == 1));
        let control2 = Arc::new(FakeControl::default());
        *control2.probe.lock().unwrap() =
            Some((Arc::downgrade(&r.shared), Arc::clone(&f.authority.catalog)));
        r.start(target(&f, &id), Arc::clone(&audit), fake(&control2))
            .unwrap();
        *done.lock().unwrap() = Some(completion_of(&r));
        reenter_shutdown.store(true, Ordering::SeqCst);
        *control2.exit.lock().unwrap() = Some(true);
        assert!(wait_until(soon(), || observations.lock().unwrap().len() == 2));
        for (published, status, stopped) in observations.lock().unwrap().iter() {
            assert!(
                published,
                "terminal audit ran before completion was published"
            );
            assert_eq!(*status, LifecycleStatus::Stopped);
            assert_eq!(*stopped, Ok(Finalized::NoOwnedServer));
        }
        assert_eq!(control.violations.load(Ordering::SeqCst), 0);
        assert_eq!(control2.violations.load(Ordering::SeqCst), 0);
        assert_eq!(violations.load(Ordering::SeqCst), 0);
        assert!(control2.finalized.load(Ordering::SeqCst));
        assert_eq!(
            r.start(target(&f, &id), audit, fake(&control2)),
            Err(LifecycleError::ShuttingDown)
        );
        // Re-entering stop from the started audit waits on another thread.
        let f2 = Fixture::new();
        let (id2, _) = registered(&f2);
        let p2 = uuid(&id2);
        let r2 = Arc::new(registry(&f2));
        let result = Arc::new(Mutex::new(None));
        let stopper: Audit = {
            let (weak, result) = (Arc::downgrade(&r2), Arc::clone(&result));
            Arc::new(move |event: Value| {
                if event["operation"] == "builder.devserver.lifecycle.started" {
                    let r2 = weak.upgrade().unwrap();
                    *result.lock().unwrap() = Some(r2.stop(p2, soon(), &quiet()));
                }
            })
        };
        let control3 = Arc::new(FakeControl::default());
        r2.start(target(&f2, &id2), stopper, fake(&control3))
            .unwrap();
        assert_eq!(*result.lock().unwrap(), Some(Ok(Finalized::Stopped)));
        assert!(control3.finalized.load(Ordering::SeqCst));
        assert_eq!(r2.status(p2), LifecycleStatus::Stopped);
    });
}

#[test]
fn p0_002c4b_audit_panics_cannot_break_ownership_or_cleanup() {
    within(|| {
        let f = Fixture::new();
        let (id, _) = registered(&f);
        let (r, p) = (registry(&f), uuid(&id));
        let panicking: Audit = Arc::new(|_| panic!("injected audit panic"));
        let control = Arc::new(FakeControl::default());
        r.start(target(&f, &id), Arc::clone(&panicking), fake(&control))
            .unwrap();
        assert_eq!(r.stop(p, soon(), &panicking), Ok(Finalized::Stopped));
        assert!(control.finalized.load(Ordering::SeqCst));
        let control = Arc::new(FakeControl::default());
        r.start(target(&f, &id), Arc::clone(&panicking), fake(&control))
            .unwrap();
        *control.exit.lock().unwrap() = Some(false);
        assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
        assert!(control.finalized.load(Ordering::SeqCst));
        assert_eq!(r.shutdown_all(soon(), &panicking), Ok(()));
    });
}

// ── Native fixtures (ignored; selected only by exact name) ────────────────

fn fixture_args(name: &str) -> Vec<OsString> {
    vec![
        "--exact".into(),
        format!("{FIXTURE_PREFIX}{name}").into(),
        "--ignored".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ]
}

// Only a trusted per-test control directory (the fixture cwd) enables a
// fixture; an unrelated `--ignored` run returns immediately.
fn fixture_active() -> bool {
    Path::new(CONTROL_MARKER).is_file()
}

#[derive(Clone, Copy, PartialEq)]
enum RootMode {
    Hold,
    RootExit,
    Crash,
    DelayedExit,
}

#[allow(clippy::zombie_processes)] // Intentional: the descendant must outlive the root.
fn fixture_root(mode: RootMode) {
    if !fixture_active() {
        return;
    }
    let descendant = if mode == RootMode::DelayedExit {
        None
    } else {
        // A reused control directory would fake readiness: fail instead.
        if Path::new(DESCENDANT_READY).exists() {
            std::process::exit(91);
        }
        let descendant = std::process::Command::new(std::env::current_exe().unwrap())
            .args(fixture_args("c4b_fixture_descendant"))
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while !Path::new(DESCENDANT_READY).is_file() {
            if Instant::now() >= deadline {
                std::process::exit(90);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Some(descendant)
    };
    println!("C4B_ROOT_READY");
    std::io::stdout().flush().unwrap();
    match mode {
        RootMode::Hold => thread::sleep(ROOT_EXPIRY),
        RootMode::RootExit => {}
        RootMode::Crash => std::process::exit(3),
        RootMode::DelayedExit => thread::sleep(DELAYED_EXIT),
    }
    drop(descendant);
}

#[test]
#[ignore]
fn c4b_fixture_hold() {
    fixture_root(RootMode::Hold);
}

#[test]
#[ignore]
fn c4b_fixture_root_exit() {
    fixture_root(RootMode::RootExit);
}

#[test]
#[ignore]
fn c4b_fixture_crash() {
    fixture_root(RootMode::Crash);
}

#[test]
#[ignore]
fn c4b_fixture_delayed_exit() {
    fixture_root(RootMode::DelayedExit);
}

#[test]
#[ignore]
fn c4b_fixture_descendant() {
    if !fixture_active() {
        return;
    }
    // Exclusive loopback evidence held only by this descendant.
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    println!("C4B_DESCENDANT_READY port={port}");
    std::io::stdout().flush().unwrap();
    let pending = format!("{DESCENDANT_READY}.tmp");
    std::fs::write(&pending, b"ready").unwrap();
    std::fs::rename(&pending, DESCENDANT_READY).unwrap();
    thread::sleep(DESCENDANT_EXPIRY);
    drop(listener);
}

// Separate trusted control directory as fixture cwd (never React, so Windows
// cwd directory handles cannot interfere with identity replacement).
struct Control {
    path: PathBuf,
}

impl Control {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nexus-c4b-control-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join(CONTROL_MARKER), b"trusted C4B test fixture").unwrap();
        Self { path }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

enum Line {
    Text(String),
    End,
}

/// Test-owned reader of the fixture stdout pipe, inherited by root and
/// descendant. EOF proves every holder of the write end is gone.
struct Capture {
    lines: mpsc::Receiver<Line>,
}

impl Capture {
    fn new(reader: ResourceReader) -> Self {
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                if sender.send(Line::Text(line)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Line::End);
        });
        Self { lines }
    }

    fn ready(&self, deadline: Instant) -> Option<u16> {
        let mut port = None;
        loop {
            match self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(Line::Text(line)) => {
                    // libtest --nocapture may print "test <name> ... " first.
                    const MARKER: &str = "C4B_DESCENDANT_READY port=";
                    if let Some(index) = line.find(MARKER) {
                        let digits: String = line[index + MARKER.len()..]
                            .chars()
                            .take_while(char::is_ascii_digit)
                            .collect();
                        port = Some(digits.parse().expect("descendant port"));
                    }
                    if line.contains("C4B_ROOT_READY") {
                        return port;
                    }
                }
                Ok(Line::End) => panic!("fixture exited before readiness"),
                Err(_) => panic!("fixture readiness deadline"),
            }
        }
    }

    fn eof(&self, deadline: Instant) -> bool {
        loop {
            match self
                .lines
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(Line::Text(_)) => {}
                Ok(Line::End) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
                Err(mpsc::RecvTimeoutError::Timeout) => return false,
            }
        }
    }
}

// #[cfg(test)]-only real launcher: absolute current test executable, no
// PATH, shell or environment change; stdout is piped only as test evidence
// and taken before the tree is handed to the lifecycle owner.
fn fixture_launcher(
    control: &Control,
    mode: &'static str,
    capture: Arc<Mutex<Option<Capture>>>,
) -> impl FnOnce() -> Launch {
    let cwd = control.path.clone();
    move || {
        let program = std::env::current_exe().map_err(ResourceLimitError::SpawnFailed)?;
        let mut child = ResourceLimiter::default().spawn(&ResourceSpawnSpec {
            program: ResourceProgram::Executable {
                program: program.into_os_string(),
                args: fixture_args(mode),
            },
            current_dir: cwd,
            stdin: ResourceStdin::Null,
            stdout: ResourceOutput::Piped,
            stderr: ResourceOutput::Null,
        })?;
        let stdout = child.take_stdout().expect("fixture stdout pipe");
        *capture.lock().unwrap() = Some(Capture::new(stdout));
        Ok(Box::new(child))
    }
}

struct Run {
    capture: Capture,
    port: Option<u16>,
}

fn launch(
    r: &LifecycleRegistry,
    f: &Fixture,
    id: &str,
    control: &Control,
    mode: &'static str,
) -> Run {
    let slot = Arc::new(Mutex::new(None));
    r.start(
        target(f, id),
        f.audit(),
        fixture_launcher(control, mode, Arc::clone(&slot)),
    )
    .unwrap();
    let capture = slot.lock().unwrap().take().expect("fixture stdout");
    let port = capture.ready(soon());
    // Every mode except delayed exit must prove a live descendant.
    assert_eq!(
        port.is_some(),
        mode != "c4b_fixture_delayed_exit",
        "{mode}: descendant readiness"
    );
    Run { capture, port }
}

fn port_held(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

fn port_released(port: u16) -> bool {
    wait_until(soon(), || TcpListener::bind(("127.0.0.1", port)).is_ok())
}

// Bounded, independent evidence that root and descendant are gone. On Unix
// terminate_and_reap confirms only group signalling plus root reap.
fn assert_tree_gone(run: &Run, context: &str) {
    assert!(
        run.capture.eof(soon()),
        "{context}: fixture pipe still held"
    );
    if let Some(port) = run.port {
        assert!(
            port_released(port),
            "{context}: descendant still holds its port"
        );
    }
}

fn assert_no_private_material(f: &Fixture, control: &Control, root: &Path, id: &str) {
    let events = lifecycle_events(f);
    assert!(!events.is_empty());
    for event in &events {
        let object = event.as_object().unwrap();
        assert_eq!(object.len(), 4, "{event}");
        assert_eq!(event["project_id"], id);
    }
    let text = serde_json::to_string(&events).unwrap();
    for sensitive in [
        f.path.to_string_lossy().into_owned(),
        control.path.to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
        root.join("react").to_string_lossy().into_owned(),
        "pid".into(),
        "port".into(),
        "exit_code".into(),
        "generation".into(),
        "execution".into(),
        "fixture".into(),
        "--exact".into(),
    ] {
        assert!(!text.contains(&sensitive), "sensitive: {sensitive}");
    }
}

// ── Real process ownership ────────────────────────────────────────────────

#[test]
fn p0_002c4b_real_tree_stop_terminates_root_and_descendant() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Control::new();
    let run = launch(&r, &f, &id, &control, "c4b_fixture_hold");
    let port = run.port.expect("descendant port");
    assert!(port_held(port), "descendant is not alive");
    assert_eq!(r.status(p), LifecycleStatus::Running);
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert_tree_gone(&run, "stop");
    assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::NoOwnedServer));
    assert!(wait_event(&f, "stopped", "stop_requested"));
    assert_no_private_material(&f, &control, &root, &id);
}

#[test]
fn p0_002c4b_stop_of_one_execution_leaves_another_and_shutdown_drains_all() {
    let f = Fixture::new();
    let r = registry(&f);
    let (a, _) = registered(&f);
    let (b, _) = registered(&f);
    let (control_a, control_b) = (Control::new(), Control::new());
    let run_a = launch(&r, &f, &a, &control_a, "c4b_fixture_hold");
    let run_b = launch(&r, &f, &b, &control_b, "c4b_fixture_hold");
    assert_eq!(r.stop(uuid(&a), soon(), &f.audit()), Ok(Finalized::Stopped));
    assert_tree_gone(&run_a, "stop A");
    assert!(port_held(run_b.port.unwrap()), "stopping A affected B");
    assert_eq!(r.status(uuid(&b)), LifecycleStatus::Running);
    let control_a = Control::new();
    let run_a = launch(&r, &f, &a, &control_a, "c4b_fixture_hold");
    assert_eq!(r.shutdown_all(soon(), &f.audit()), Ok(()));
    assert!(!r.shared.state().accepting);
    for (run, id) in [(&run_a, &a), (&run_b, &b)] {
        assert_tree_gone(run, "shutdown");
        assert_eq!(r.status(uuid(id)), LifecycleStatus::Stopped);
    }
    assert!(wait_event(&f, "stopped", "shutdown"));
    assert_eq!(r.shutdown_all(soon(), &f.audit()), Ok(()));
    assert!(matches!(
        r.start(
            target(&f, &a),
            f.audit(),
            fake(&Arc::new(FakeControl::default()))
        ),
        Err(LifecycleError::ShuttingDown)
    ));
}

#[test]
fn p0_002c4b_natural_root_exit_still_finalizes_surviving_descendant() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Control::new();
    let run = launch(&r, &f, &id, &control, "c4b_fixture_root_exit");
    assert!(
        run.port.is_some(),
        "descendant did not start before root exit"
    );
    // The descendant outlives the root (60s+) unless the owner finalizes it.
    assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
    assert_tree_gone(&run, "natural root exit");
    assert!(wait_event(&f, "exited", "exited"));
    assert!(!lifecycle_events(&f)
        .iter()
        .any(|event| event["operation"] == "builder.devserver.lifecycle.stop"));
    assert_no_private_material(&f, &control, &root, &id);
}

#[test]
fn p0_002c4b_unexpected_root_exit_is_finalized_and_audited_without_exit_code() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Control::new();
    let run = launch(&r, &f, &id, &control, "c4b_fixture_crash");
    assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
    assert_tree_gone(&run, "unexpected exit");
    assert!(wait_event(&f, "exited", "crashed"));
    // Four bounded fields only: no exit code, signal or status is audited.
    assert_no_private_material(&f, &control, &root, &id);
}

#[test]
fn p0_002c4b_delayed_root_exit_without_descendant_finalizes_and_races_stop() {
    // No descendant: after root exit only the unreaped root remains in the
    // group, exercising the zombie-only finalization path (Darwin natively).
    let f = Fixture::new();
    let (id, _) = registered(&f);
    let (r, p) = (registry(&f), uuid(&id));
    let control = Control::new();
    let run = launch(&r, &f, &id, &control, "c4b_fixture_delayed_exit");
    assert!(run.port.is_none());
    assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
    assert_tree_gone(&run, "delayed exit");
    assert!(wait_event(&f, "exited", "exited"));
    // Stop issued around the natural exit finalizes exactly one generation.
    let control = Control::new();
    let run = launch(&r, &f, &id, &control, "c4b_fixture_delayed_exit");
    thread::sleep(DELAYED_EXIT.saturating_sub(Duration::from_millis(50)));
    let result = r.stop(p, soon(), &f.audit());
    assert!(
        matches!(
            result,
            Ok(Finalized::Stopped | Finalized::Exited | Finalized::NoOwnedServer)
        ),
        "{result:?}"
    );
    assert_eq!(r.status(p), LifecycleStatus::Stopped);
    assert_tree_gone(&run, "stop racing delayed exit");
}

// ── Identity invalidation (real process) ──────────────────────────────────

#[test]
fn p0_002c4b_identity_invalidation_terminates_real_tree() {
    for case in [
        "storage-removed",
        "storage-replaced",
        "project-removed",
        "project-replaced",
        "react-removed",
        "react-replaced",
    ] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let (r, p) = (registry(&f), uuid(&id));
        let control = Control::new();
        let run = launch(&r, &f, &id, &control, "c4b_fixture_hold");
        let first = execution_of(&r, p).unwrap();
        let react = root.join("react");
        let detached = f.path.with_extension("project");
        let old_storage = f.path.with_extension("old");
        match case {
            "project-removed" => std::fs::remove_dir_all(&root).unwrap(),
            "project-replaced" => {
                std::fs::rename(&root, root.with_extension("original")).unwrap();
                std::fs::create_dir_all(&react).unwrap();
            }
            "react-removed" => std::fs::remove_dir(&react).unwrap(),
            "react-replaced" => {
                std::fs::rename(&react, react.with_extension("old")).unwrap();
                std::fs::create_dir(&react).unwrap();
            }
            _ => {
                // Move the observed project first (Windows share-delete rule).
                std::fs::rename(&root, &detached).unwrap();
                std::fs::rename(&f.path, &old_storage).unwrap();
                if case == "storage-replaced" {
                    std::fs::create_dir(&f.path).unwrap();
                    std::fs::rename(&detached, &root).unwrap();
                }
            }
        }
        assert!(
            wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped),
            "{case}"
        );
        assert_tree_gone(&run, case);
        let react_case = case.starts_with("react");
        let reason = if react_case { "react" } else { "registration" };
        assert!(wait_event(&f, "invalidated", reason), "{case}");
        // Storage/project changes keep C3 tombstone semantics; React does not.
        assert_eq!(f.authority.catalog.lookup(p).is_ok(), react_case, "{case}");
        if react_case {
            if case == "react-removed" {
                std::fs::create_dir(&react).unwrap();
            }
            let control = Control::new();
            let run = launch(&r, &f, &id, &control, "c4b_fixture_hold");
            assert!(execution_of(&r, p).unwrap() > first);
            assert_eq!(r.stop(p, soon(), &f.audit()), Ok(Finalized::Stopped));
            assert_tree_gone(&run, case);
        } else {
            // A tombstoned registration cannot yield a target for any new
            // generation of this project.
            assert!(f.authority.dev_server_target(&id, &f.audit()).is_err());
            assert_eq!(r.status(p), LifecycleStatus::Stopped);
        }
        let _ = std::fs::remove_dir_all(&detached);
        let _ = std::fs::remove_dir_all(&old_storage);
    }
}

#[cfg(unix)]
#[test]
fn p0_002c4b_unix_symlink_redirects_terminate_real_tree() {
    use std::os::unix::fs::symlink;
    for project_redirect in [false, true] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let (r, p) = (registry(&f), uuid(&id));
        let control = Control::new();
        let run = launch(&r, &f, &id, &control, "c4b_fixture_hold");
        let redirected = if project_redirect {
            root.clone()
        } else {
            root.join("react")
        };
        let old = redirected.with_extension("old");
        std::fs::rename(&redirected, &old).unwrap();
        symlink(&old, &redirected).unwrap();
        assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
        assert_tree_gone(&run, "unix symlink redirect");
        assert!(wait_event(
            &f,
            "invalidated",
            if project_redirect {
                "registration"
            } else {
                "react"
            }
        ));
        assert_eq!(f.authority.catalog.lookup(p).is_err(), project_redirect);
    }
}

#[cfg(windows)]
#[test]
fn p0_002c4b_windows_reparse_redirects_terminate_real_tree() {
    use std::os::windows::fs::symlink_dir;
    for project_redirect in [false, true] {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let (r, p) = (registry(&f), uuid(&id));
        let control = Control::new();
        let run = launch(&r, &f, &id, &control, "c4b_fixture_hold");
        let redirected = if project_redirect {
            root.clone()
        } else {
            root.join("react")
        };
        let old = redirected.with_extension("old");
        std::fs::rename(&redirected, &old).unwrap();
        symlink_dir(&old, &redirected)
            .expect("native Windows test requires symlink creation privilege");
        assert!(wait_until(soon(), || r.status(p) == LifecycleStatus::Stopped));
        assert_tree_gone(&run, "windows reparse redirect");
        assert!(wait_event(
            &f,
            "invalidated",
            if project_redirect {
                "registration"
            } else {
                "react"
            }
        ));
        assert_eq!(f.authority.catalog.lookup(p).is_err(), project_redirect);
    }
}
