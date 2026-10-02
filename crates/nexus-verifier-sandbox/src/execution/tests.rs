use super::*;
use crate::scope::expected_limit_files;

fn report() -> ExecutionReport {
    ExecutionReport {
        not_run: None,
        outcome: Some(Outcome::Finished(VerifierStatus::Exited(0))),
        ended_by: None,
        duration: Duration::from_millis(5),
        stdout: StreamRecord::default(),
        stderr: StreamRecord::default(),
        output_lost: false,
        events: Some(ScopeEvents::default()),
        cleanup: Cleanup::Confirmed,
    }
}

fn failed_cleanup() -> Cleanup {
    Cleanup::Failed(RetainedBoundary {
        scope: ScopeBoundary::None,
        helper: None,
    })
}

#[test]
fn p2d_only_a_clean_passing_exit_is_passed() {
    assert_eq!(report().classify(0), ExitClass::Passed);
    let mut failed = report();
    failed.outcome = Some(Outcome::Finished(VerifierStatus::Exited(101)));
    assert_eq!(failed.classify(0), ExitClass::Failed { exit_code: 101 });
    let mut signalled = report();
    signalled.outcome = Some(Outcome::Finished(VerifierStatus::Signalled(11)));
    assert_eq!(signalled.classify(0), ExitClass::Signalled { signal: 11 });
}

#[test]
fn p2d_a_lost_verifier_or_lost_counters_are_never_a_result() {
    let mut lost = report();
    lost.outcome = Some(Outcome::Lost);
    assert_eq!(lost.classify(0), ExitClass::SandboxFailed);
    let mut none = report();
    none.outcome = None;
    assert_eq!(none.classify(0), ExitClass::SandboxFailed);
    // A passing exit without the scope's counters cannot rule out a limit.
    let mut uncounted = report();
    uncounted.events = None;
    assert_eq!(uncounted.classify(0), ExitClass::SandboxFailed);
    let mut uncounted_failure = report();
    uncounted_failure.outcome = Some(Outcome::Finished(VerifierStatus::Exited(101)));
    uncounted_failure.events = None;
    assert_eq!(uncounted_failure.classify(0), ExitClass::SandboxFailed);
    // A lost init is explained by an out-of-memory kill when one happened.
    let mut lost_to_oom = report();
    lost_to_oom.outcome = Some(Outcome::Lost);
    lost_to_oom.events = Some(ScopeEvents {
        oom_kills: 1,
        pids_max: 0,
    });
    assert_eq!(lost_to_oom.classify(0), ExitClass::OomKilled);
    // A setup failure ran nothing: counters do not change that.
    let mut setup = report();
    setup.outcome = Some(Outcome::SetupFailed {
        stage: SetupStage::Exec,
        errno: libc::ENOENT,
    });
    setup.events = None;
    assert_eq!(setup.classify(0), ExitClass::SandboxSetupFailed);
}

#[test]
fn p2d_limits_and_backend_endings_are_never_passed() {
    let mut deadline = report();
    deadline.ended_by = Some(EndedBy::Deadline);
    assert_eq!(deadline.classify(0), ExitClass::TimedOut);
    let mut flood = report();
    flood.ended_by = Some(EndedBy::OutputLimit);
    assert_eq!(flood.classify(0), ExitClass::OutputLimitExceeded);
    let mut truncated = report();
    truncated.stderr.truncated = true;
    assert_eq!(truncated.classify(0), ExitClass::OutputLimitExceeded);
    let mut oom = report();
    oom.events = Some(ScopeEvents {
        oom_kills: 1,
        pids_max: 0,
    });
    assert_eq!(oom.classify(0), ExitClass::OomKilled);
    let mut pids = report();
    pids.events = Some(ScopeEvents {
        oom_kills: 0,
        pids_max: 3,
    });
    assert_eq!(pids.classify(0), ExitClass::ProcessLimit);
    // The memory limit outranks the process limit, and both outrank the exit.
    let mut both = report();
    both.events = Some(ScopeEvents {
        oom_kills: 1,
        pids_max: 1,
    });
    assert_eq!(both.classify(0), ExitClass::OomKilled);
}

#[test]
fn p2d_unconfirmed_cleanup_overrides_every_other_class() {
    let mut passed = report();
    passed.cleanup = failed_cleanup();
    assert_eq!(passed.classify(0), ExitClass::CleanupFailed);
    let mut deadline = report();
    deadline.ended_by = Some(EndedBy::Deadline);
    deadline.cleanup = failed_cleanup();
    assert_eq!(deadline.classify(0), ExitClass::CleanupFailed);
}

#[test]
fn p2d_setup_failures_distinguish_an_unavailable_host() {
    let mut no_scope = report();
    no_scope.outcome = None;
    no_scope.not_run = Some(NotRun::Scope(crate::scope::ScopeError::NotPlaced));
    assert_eq!(no_scope.classify(0), ExitClass::SandboxUnavailable);
    for (stage, errno, class) in [
        (
            SetupStage::Unshare,
            libc::EPERM,
            ExitClass::SandboxUnavailable,
        ),
        (SetupStage::Landlock, -12, ExitClass::SandboxUnavailable),
        (SetupStage::Protocol, 0, ExitClass::SandboxSetupFailed),
        (
            SetupStage::Exec,
            libc::ENOENT,
            ExitClass::SandboxSetupFailed,
        ),
        (SetupStage::NetworkCheck, -1, ExitClass::SandboxSetupFailed),
    ] {
        let mut refused = report();
        refused.outcome = None;
        refused.not_run = Some(NotRun::Launch(LaunchError::SetupFailed { stage, errno }));
        assert_eq!(refused.classify(0), class, "{stage:?}");
    }
    let mut spawn = report();
    spawn.outcome = None;
    spawn.not_run = Some(NotRun::Spawn(LaunchError::Protocol));
    assert_eq!(spawn.classify(0), ExitClass::SandboxSetupFailed);
}

#[test]
fn p2d_scope_limit_files_are_the_exact_profile_limits() {
    let files = expected_limit_files(&ResourcePolicy::RUST_OFFLINE_V1);
    assert_eq!(
        files,
        [
            ("memory.max", "4294967296".to_string()),
            ("memory.swap.max", "0".to_string()),
            ("memory.oom.group", "0".to_string()),
            ("pids.max", "256".to_string()),
            ("cpu.max", "400000 100000".to_string()),
        ]
    );
}

#[test]
fn p2d_a_removed_directory_lists_as_empty_through_its_descriptor() {
    use std::os::fd::AsFd;
    let base = std::env::temp_dir().join(format!("nexus-p2d-listing-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let dir = std::fs::File::open(&base).unwrap();
    assert!(crate::sys::directory_is_empty(dir.as_fd()).unwrap());
    std::fs::write(base.join("entry"), b"x").unwrap();
    assert!(!crate::sys::directory_is_empty(dir.as_fd()).unwrap());
    std::fs::remove_file(base.join("entry")).unwrap();
    std::fs::remove_dir(&base).unwrap();
    assert!(crate::sys::directory_is_empty(dir.as_fd()).unwrap());
}

#[test]
fn p2r1_an_interrupted_execution_is_never_passed() {
    // A panic before the launch could reach the helper: nothing ran.
    let mut before = report();
    before.outcome = None;
    before.events = None;
    before.not_run = Some(NotRun::Interrupted);
    assert_eq!(before.classify(0), ExitClass::SandboxSetupFailed);
    // After it, whatever the verifier reported, the result is unknown.
    let mut after = report();
    after.ended_by = Some(EndedBy::Interrupted);
    assert_eq!(after.classify(0), ExitClass::SandboxFailed);
    // A lost output record is lost accounting, even with a passing exit.
    let mut lost = report();
    lost.output_lost = true;
    assert_eq!(lost.classify(0), ExitClass::SandboxFailed);
    // An unconfirmed cleanup still outranks each of them.
    for mut interrupted in [before, after, lost] {
        interrupted.cleanup = failed_cleanup();
        assert_eq!(interrupted.classify(0), ExitClass::CleanupFailed);
    }
    // What a panic leaves observed depends only on whether the launch could
    // have reached the helper.
    let mut owned = Owned::default();
    assert!(matches!(
        interrupted(&owned).not_run,
        Some(NotRun::Interrupted)
    ));
    owned.launched = true;
    let observed = interrupted(&owned);
    assert!(observed.not_run.is_none() && observed.outcome.is_none());
    assert_eq!(observed.ended_by, Some(EndedBy::Interrupted));
}

/// A stand-in helper for ownership tests without a scope: `cat` keeps its
/// control socket (stdin) open and stays alive until it is ended. Returns
/// its process id.
fn stand_in(owned: &mut Owned, fault: Option<Fault>) -> u32 {
    let (helper, output) = Helper::spawn(&HelperProgram::at("/bin/cat")).unwrap();
    let pid = helper.pid();
    let limits = ResourcePolicy::RUST_OFFLINE_V1;
    let flag = || Arc::new(AtomicBool::new(false));
    owned.helper = Some(helper);
    owned.stdout = Some(drain(output.stdout, &limits, flag(), flag(), fault).unwrap());
    owned.stderr = Some(drain(output.stderr, &limits, flag(), flag(), fault).unwrap());
    pid
}

/// Whether `pid` is still this process's child, running or unreaped.
fn is_child(pid: u32) -> bool {
    let parent = format!("PPid:\t{}", std::process::id());
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .is_ok_and(|status| status.lines().any(|line| line == parent))
}

/// Whether `pid` is still running (neither gone nor a zombie).
fn is_running(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        stat.rsplit_once(") ")
            .is_some_and(|(_, rest)| !rest.starts_with('Z'))
    })
}

#[test]
fn p2r1_finalization_ends_and_reaps_a_helper_without_a_scope() {
    let mut owned = Owned::default();
    let pid = stand_in(&mut owned, None);
    assert!(is_child(pid) && is_running(pid));
    let finalized = finalize(&mut owned, None);
    assert!(finalized.cleanup.is_confirmed());
    assert!(!finalized.output_lost);
    assert!(!is_child(pid), "the helper is killed and reaped");
    assert!(owned.helper.is_none() && matches!(owned.scope, ScopeBoundary::None));
    // Nothing owned: nothing to confirm, nothing retained.
    let mut empty = Owned::default();
    assert!(finalize(&mut empty, None).cleanup.is_confirmed());
    assert!(empty.retain().is_confirmed());
}

#[test]
fn p2r1_a_failed_or_panicking_finalization_retains_the_live_boundary() {
    for fault in [
        Fault::Fail(FaultPoint::Finalizing),
        Fault::Panic(FaultPoint::Finalizing),
    ] {
        let mut owned = Owned::default();
        let pid = stand_in(&mut owned, Some(fault));
        let finalized = finalize(&mut owned, Some(fault));
        let Cleanup::Failed(boundary) = finalized.cleanup else {
            panic!("{fault:?}: cleanup reported confirmed without proof");
        };
        // The unfinished boundary is retained, live and unreaped, never
        // dropped.
        assert!(boundary.holds_helper(), "{fault:?}");
        assert!(is_child(pid) && is_running(pid), "{fault:?}");
        assert!(owned.helper.is_none(), "{fault:?}: moved, not copied");
        // A retry ends and reaps it.
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{fault:?}");
    }
}

#[test]
fn p2r1_a_dropped_boundary_still_ends_its_helper() {
    // Defense in depth only: owners keep a retained boundary until a retry
    // succeeds, but one that is dropped does not leave its helper running.
    let mut owned = Owned::default();
    let pid = stand_in(&mut owned, None);
    let Cleanup::Failed(boundary) = owned.retain() else {
        panic!("a live helper is retained");
    };
    drop(boundary);
    let start = Instant::now();
    while is_running(pid) && start.elapsed() < FINALIZE_TIMEOUT {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!is_running(pid));
}

#[test]
fn p2r1_a_panicking_output_thread_loses_its_record() {
    use std::io::Write;
    let (read, write) = crate::sys::pipe().unwrap();
    let limits = ResourcePolicy::RUST_OFFLINE_V1;
    let flag = || Arc::new(AtomicBool::new(false));
    let thread = drain(
        read,
        &limits,
        flag(),
        flag(),
        Some(Fault::Panic(FaultPoint::DrainThread)),
    )
    .unwrap();
    std::fs::File::from(write).write_all(b"output").unwrap();
    let (record, lost) = join(Some(thread));
    assert!(lost);
    assert_eq!(record, StreamRecord::default());
    // An output thread that ended normally is never lost.
    let (read, write) = crate::sys::pipe().unwrap();
    let thread = drain(read, &limits, flag(), flag(), None).unwrap();
    std::fs::File::from(write).write_all(b"output").unwrap();
    let (record, lost) = join(Some(thread));
    assert!(!lost);
    assert_eq!(record.bytes, 6);
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4: scope operations whose remote effect is uncertain, through
// whole executions over the deterministic simulation of the user manager
// and the kernel (`crate::scope::tests`). No test here contacts a bus or
// creates a cgroup.

use crate::helper::enforced_policy_hash;
use crate::protocol::{FromHelper, PROTOCOL_VERSION};
use crate::scope::tests::{Call, Phantom, Placement, Property, Reply, Script, World, SLICE};

/// A stand-in helper that greets like the real one, then copies the first
/// message the backend sends it to its stdout and refuses it: a launch that
/// reaches it shows in the execution's stdout record.
struct StandIn {
    dir: std::path::PathBuf,
}

impl StandIn {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "nexus-i4-helper-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&dir).unwrap();
        let hello = FromHelper::Hello {
            version: PROTOCOL_VERSION,
            policy_hash: enforced_policy_hash(),
        };
        let refuse = FromHelper::SetupFailed {
            stage: SetupStage::Protocol,
            errno: 0,
        };
        std::fs::write(dir.join("hello"), hello.encode()).unwrap();
        std::fs::write(dir.join("refuse"), refuse.encode()).unwrap();
        let script = format!(
            "#!/bin/sh\n/bin/cat {dir}/hello >&0\n/bin/dd bs=65536 count=1 status=none\n\
             /bin/cat {dir}/refuse >&0\nexec /bin/cat >/dev/null\n",
            dir = dir.display()
        );
        std::fs::write(dir.join("helper"), script).unwrap();
        std::fs::set_permissions(dir.join("helper"), PermissionsExt::from_mode(0o755)).unwrap();
        let stand_in = Self { dir };
        // Executable once no process still holds it open for writing (a
        // process forked concurrently may, briefly).
        for attempt in 0.. {
            match Helper::spawn(&stand_in.program()) {
                Ok((mut helper, _)) => {
                    let _ = helper.kill();
                    helper.reap().unwrap();
                    break;
                }
                Err(LaunchError::Spawn(error))
                    if error.raw_os_error() == Some(libc::ETXTBSY) && attempt < 200 =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("the stand-in helper: {error:?}"),
            }
        }
        stand_in
    }

    fn program(&self) -> HelperProgram {
        HelperProgram::at(self.dir.join("helper"))
    }
}

impl Drop for StandIn {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn launch_spec() -> LaunchSpec {
    let root = || OwnedFd::from(std::fs::File::open("/").unwrap());
    LaunchSpec {
        generation: 1,
        executable: root(),
        working_directory: root(),
        argv: vec![b"verifier".to_vec()],
        env: Vec::new(),
        rules: Vec::new(),
    }
}

/// One whole execution over `world` with the stand-in helper; the
/// [`ScopeManager`] is gone when it returns.
fn run_in(world: &World, fault: Option<Fault>) -> ExecutionReport {
    let stand_in = StandIn::new();
    let scopes = world.scopes();
    execute(
        &scopes,
        &stand_in.program(),
        launch_spec(),
        &ResourcePolicy::RUST_OFFLINE_V1,
        fault,
    )
}

/// The helper's process id, as the scope request named it.
fn requested_pid(world: &World) -> u32 {
    world
        .lock()
        .calls
        .iter()
        .find_map(|call| match call {
            Call::Start(_, pid) => Some(*pid),
            _ => None,
        })
        .expect("a scope was requested")
}

fn boundary_of(report: ExecutionReport) -> RetainedBoundary {
    match report.cleanup {
        Cleanup::Failed(boundary) => boundary,
        Cleanup::Confirmed => panic!("cleanup reported confirmed without proof"),
    }
}

fn pending_state(boundary: &RetainedBoundary) -> crate::scope::PendingState {
    match &boundary.scope {
        ScopeBoundary::Pending(pending) => pending.state(),
        other => panic!("the unresolved operation is not retained: {other:?}"),
    }
}

fn opens(world: &World) -> usize {
    world.lock().count(|call| matches!(call, Call::Open(_)))
}

fn stops(world: &World) -> usize {
    world.lock().count(|call| matches!(call, Call::Stop(_)))
}

/// Every membership read happened while the helper was unreaped.
fn observed_unreaped(world: &World) -> bool {
    world
        .lock()
        .calls
        .iter()
        .all(|call| !matches!(call, Call::Membership(_, false)))
}

/// A scope whose proof fails on its retained cgroup (the out-of-memory
/// policy differs).
fn failing_proof(state: &mut crate::scope::tests::State) {
    state.oom_policy = Property::Wrong;
}

#[test]
fn i4_04_a_property_timeout_after_placement_never_launches_and_keeps_the_candidate() {
    for runtime in [true, false] {
        // Only the cgroup opened during the proof can ever be retained (no
        // later open succeeds), and StopUnit never acts.
        let world = World::new(|state| {
            let timeout = Property::Uncertain(Reply::Timeout);
            if runtime {
                state.runtime_max = timeout;
            } else {
                state.oom_policy = timeout;
            }
            state.opens_left = Some(1);
            state.stop = Script::always((false, Reply::Timeout));
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{:?}",
            report.not_run
        );
        assert_eq!(report.stdout.bytes, 0, "a launch reached the helper");
        assert!(
            report.cleanup.is_confirmed(),
            "cleanup not confirmed through the retained candidate"
        );
        assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
        assert_eq!(opens(&world), 1);
        assert!(world.lock().created().unwrap().killed);
        assert!(observed_unreaped(&world) && !is_child(pid));
    }
}

#[test]
fn i4_05_a_unit_the_helper_never_entered_is_confirmed_gone_before_the_failure() {
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::NotPlaced))
    ));
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    let state = world.lock();
    let unit = state.requested().unwrap();
    assert!(!state.units.contains_key(&unit));
    // Stopped, then observed gone on the issuing connection with the helper
    // (unreaped then) outside it.
    let stop = state
        .calls
        .iter()
        .position(|call| matches!(call, Call::Stop(_)))
        .expect("the unit was stopped");
    assert!(state.calls[stop..]
        .iter()
        .any(|call| matches!(call, Call::GetUnit(_))));
    drop(state);
    assert!(observed_unreaped(&world) && !is_child(pid));
}

#[test]
fn i4_06_a_failed_stop_of_a_unit_the_helper_never_entered_retains_the_operation() {
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Error));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::NotPlaced))
    ));
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a StopUnit error was taken as proof that nothing remained"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.issued && !observed.settled && observed.stops >= 1);
    assert!(boundary.holds_scope() && boundary.holds_helper());
    assert!(
        is_child(pid),
        "the helper of an unresolved operation was reaped"
    );
    // Once the manager acts, a retry confirms it gone.
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_07_a_delivered_stop_reply_with_the_scope_still_populated_confirms_nothing() {
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
        state.stop = Script::always((true, Reply::Delivered));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a delivered StopUnit reply was taken as cleanup"
    );
    assert!(stops(&world) >= 1);
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).candidate);
    assert!(is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_08_a_timed_out_stop_whose_target_is_gone_may_confirm() {
    // With the retained candidate: the stop took effect, its reply was lost,
    // and the cgroup is removed.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::UntilStop;
        state.stop = Script::always((true, Reply::Timeout));
    });
    let report = run_in(&world, None);
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(stops(&world), 1);
    assert!(world.lock().created().unwrap().removed);
    // Without one: the manager answers the unit gone and the helper is
    // outside it.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((true, Reply::Timeout));
    });
    let report = run_in(&world, None);
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
}

#[test]
fn i4_09_a_timed_out_stop_whose_target_remains_retains_the_operation() {
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a timed-out StopUnit dropped the operation"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.candidate && !observed.settled);
    assert!(world.lock().created().unwrap().populated());
    assert!(is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_10_an_uncertain_query_after_the_candidate_keeps_its_descriptor() {
    // GetUnit times out once the candidate is retained; no cgroup can be
    // opened again and StopUnit never acts.
    let world = World::new(|state| {
        state.get_unit = Script::first([Reply::Timeout], Reply::Delivered);
        state.opens_left = Some(1);
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Bus(_)))
    ));
    assert!(
        report.cleanup.is_confirmed(),
        "the candidate retained before the uncertain query was not kept"
    );
    assert_eq!(opens(&world), 1);
    // While its cgroup stays populated, the retained boundary holds that
    // very descriptor, and a retry ends it through it.
    let world = World::new(|state| {
        state.get_unit = Script::first([Reply::Timeout], Reply::Delivered);
        state.opens_left = Some(1);
        state.phantom = Phantom::Forever;
    });
    let boundary = boundary_of(run_in(&world, None));
    assert!(pending_state(&boundary).candidate);
    world.release();
    world.lock().opens_left = Some(0);
    boundary.retry().unwrap();
    assert_eq!(opens(&world), 1);
}

#[test]
fn i4_11_manager_absence_with_the_helper_in_the_candidate_is_refused_and_retained() {
    // The kernel shows the helper in a cgroup of the unit's name that the
    // manager does not have loaded: nothing is proven or launched, and the
    // retained candidate is what cleanup is confirmed through.
    let world = World::new(|state| state.unit_loaded = false);
    let report = run_in(&world, None);
    assert!(
        matches!(
            report.not_run,
            Some(NotRun::Scope(ScopeError::Mismatch("unit not loaded")))
        ),
        "the manager's absence of the unit did not refuse the proof: {:?}",
        report.not_run
    );
    assert_eq!(report.stdout.bytes, 0);
    assert!(report.cleanup.is_confirmed());
    assert!(world.lock().created().unwrap().killed);
    // No candidate can be retained: the manager's absence alone confirms
    // nothing while the helper is in a cgroup of the unit's name.
    let world = World::new(|state| {
        state.unit_loaded = false;
        state.cgroup_v2 = false;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "manager absence alone confirmed an operation whose helper is in a cgroup of its unit"
    );
    let boundary = boundary_of(report);
    assert!(!pending_state(&boundary).candidate);
    // Once that cgroup is removed, the helper is outside it.
    {
        let mut state = world.lock();
        let unit = state.requested().unwrap();
        state.cgroups[0].removed = true;
        state
            .membership
            .insert(pid, format!("{SLICE}/{unit} (deleted)"));
    }
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_12_mismatched_limits_never_launch() {
    for file in [
        "memory.max",
        "memory.swap.max",
        "memory.oom.group",
        "pids.max",
        "cpu.max",
    ] {
        let world = World::new(|state| state.limit_mismatch = Some(file));
        let report = run_in(&world, None);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Mismatch(found))) if found == file),
            "{file}: {:?}",
            report.not_run
        );
        assert_eq!(report.stdout.bytes, 0, "{file}");
        assert!(report.cleanup.is_confirmed(), "{file}");
    }
}

#[test]
fn i4_13_a_runtime_backstop_mismatch_never_launches() {
    for property in [Property::Wrong, Property::WrongType] {
        let world = World::new(|state| state.runtime_max = property);
        let report = run_in(&world, None);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("runtime backstop")))
            ),
            "{property:?}: {:?}",
            report.not_run
        );
        assert_eq!(report.stdout.bytes, 0);
        assert!(report.cleanup.is_confirmed());
    }
}

#[test]
fn i4_14_an_out_of_memory_policy_mismatch_never_launches() {
    for property in [Property::Wrong, Property::WrongType] {
        let world = World::new(|state| state.oom_policy = property);
        let report = run_in(&world, None);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("out-of-memory policy")))
            ),
            "{property:?}: {:?}",
            report.not_run
        );
        assert_eq!(report.stdout.bytes, 0);
        assert!(report.cleanup.is_confirmed());
    }
}

#[test]
fn i4_15_a_membership_mismatch_never_launches() {
    // The helper is in a cgroup that is not the unit's.
    for elsewhere in ["/elsewhere/../{unit}", "/elsewhere/{unit}-not"] {
        let world = World::new(|state| state.placement = Placement::Elsewhere(elsewhere));
        let report = run_in(&world, None);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::NotPlaced))),
            "{elsewhere}: {:?}",
            report.not_run
        );
        assert_eq!(opens(&world), 0, "{elsewhere}");
        assert!(report.cleanup.is_confirmed(), "{elsewhere}");
    }
    // The cgroup of the unit's name does not list the helper.
    let world = World::new(|state| state.listed = false);
    let report = run_in(&world, None);
    assert!(
        matches!(
            report.not_run,
            Some(NotRun::Scope(ScopeError::Mismatch(
                "helper not in the scope"
            )))
        ),
        "a cgroup that does not list the helper was proven: {:?}",
        report.not_run
    );
    assert_eq!(report.stdout.bytes, 0);
    assert!(report.cleanup.is_confirmed());
}

#[test]
fn i4_16_a_pending_operation_is_never_proven_by_its_unit_name() {
    // The manager has the unit, with exactly the requested properties, and
    // a cgroup of its name holds a process with exactly the requested
    // limits; but the kernel never reports the helper in it.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.phantom = Phantom::UntilKill;
    });
    let report = run_in(&world, None);
    assert!(
        matches!(report.not_run, Some(NotRun::Scope(ScopeError::NotPlaced))),
        "a scope was proven by its unit name: {:?}",
        report.not_run
    );
    assert_eq!(report.stdout.bytes, 0);
    assert_eq!(opens(&world), 0, "a cgroup was opened by the unit's name");
    assert!(report.cleanup.is_confirmed());
}

#[test]
fn i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit() {
    let world = World::new(|state| state.collide = true);
    let report = run_in(&world, None);
    assert_eq!(
        stops(&world),
        0,
        "the colliding unit, not this request's, was stopped"
    );
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Bus(_)))
    ));
    assert_eq!(report.stdout.bytes, 0);
    assert!(report.cleanup.is_confirmed());
    {
        let state = world.lock();
        let unit = state.requested().unwrap();
        assert!(!state.units[&unit].ours, "the other unit is still loaded");
        assert!(state
            .calls
            .iter()
            .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_) | Call::GetUnit(_))));
    }
    // The other unit's cgroup holds the helper: nothing is confirmed, and
    // still nothing of that unit is touched.
    let world = World::new(|state| {
        state.collide = true;
        state.collision_holds_helper = true;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).collided);
    assert_eq!(stops(&world), 0);
    world.lock().membership.remove(&pid);
    boundary.retry().unwrap();
    assert_eq!(stops(&world), 0);
}

#[test]
fn i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven() {
    let unproven: [fn(&mut crate::scope::tests::State); 8] = [
        |state| state.runtime_max = Property::Uncertain(Reply::Timeout),
        |state| state.oom_policy = Property::Uncertain(Reply::Disconnect),
        |state| state.get_unit = Script::first([Reply::Malformed], Reply::Delivered),
        |state| state.limit_mismatch = Some("cpu.max"),
        |state| state.listed = false,
        |state| state.unit_loaded = false,
        |state| state.placement = Placement::Never,
        |state| {
            state.start_effect = false;
            state.start_reply = Reply::Timeout;
        },
    ];
    for (case, script) in unproven.into_iter().enumerate() {
        let world = World::new(script);
        let report = run_in(&world, None);
        // Confirmed cleanup joins the output threads: the record is whole.
        assert!(report.cleanup.is_confirmed(), "case {case}");
        assert_eq!(
            report.stdout.bytes, 0,
            "case {case}: a launch message reached the helper before the proof"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(_))),
            "case {case}: {:?}",
            report.not_run
        );
    }
    // The stand-in shows a launch that does reach it: once proven.
    let world = World::default();
    let report = run_in(&world, None);
    assert!(
        matches!(
            report.not_run,
            Some(NotRun::Launch(LaunchError::SetupFailed {
                stage: SetupStage::Protocol,
                errno: 0
            }))
        ),
        "{:?}",
        report.not_run
    );
    assert!(report.stdout.bytes > 0, "the launch message is visible");
    assert!(report.cleanup.is_confirmed());
}

#[test]
fn i4_19_finalization_ends_a_proven_scope_as_before() {
    let world = World::default();
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(report.cleanup.is_confirmed());
    assert_eq!(report.events, Some(ScopeEvents::default()));
    assert_eq!(report.classify(0), ExitClass::SandboxSetupFailed);
    let state = world.lock();
    let cgroup = state.created().unwrap();
    assert!(cgroup.killed && cgroup.kills >= 1);
    // Through the retained descriptor only: nothing is stopped, nothing
    // asked of the manager after the proof.
    assert_eq!(state.count(|call| matches!(call, Call::Stop(_))), 0);
    assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 1);
    drop(state);
    assert!(!is_child(pid));
}

#[test]
fn i4_20_finalization_settles_a_pending_operation() {
    // Confirmed: the helper is killed but observed unreaped until the
    // operation is confirmed gone, and reaped after.
    let world = World::new(failing_proof);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(report.cleanup.is_confirmed());
    assert!(observed_unreaped(&world) && !is_child(pid));
    // Unconfirmed: the operation is retained with its helper.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    assert!(
        boundary.holds_scope(),
        "the unresolved operation was not retained"
    );
    assert!(boundary.holds_helper() && is_child(pid));
    world.release();
    boundary.retry().unwrap();
}

#[test]
fn i4_21_an_unconfirmed_operation_is_a_cleanup_failure_never_an_unavailable_sandbox() {
    // Created, its reply lost with the connection, the helper never placed:
    // nothing can be confirmed.
    let world = World::new(|state| {
        state.start_reply = Reply::Disconnect;
        state.placement = Placement::Never;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(matches!(report.not_run, Some(NotRun::Scope(_))));
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an unconfirmed scope operation was reported as an unavailable sandbox with confirmed cleanup"
    );
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).issued && is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_22_a_retry_resolves_a_pending_operation() {
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    // Still uncertain: the retry keeps it.
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && is_child(pid));
    world.release();
    boundary.retry().unwrap();
    let state = world.lock();
    assert!(!state.units.contains_key(&state.requested().unwrap()));
    drop(state);
    assert!(!is_child(pid));
}

#[test]
fn i4_23_a_retained_boundary_is_retried_without_the_scope_manager() {
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Error));
    });
    let stand_in = StandIn::new();
    let scopes = world.scopes();
    let report = execute(
        &scopes,
        &stand_in.program(),
        launch_spec(),
        &ResourcePolicy::RUST_OFFLINE_V1,
        None,
    );
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    drop(scopes);
    world.release();
    assert!(
        boundary.retry().is_ok(),
        "a retained boundary needs the ScopeManager that started it"
    );
    assert!(!is_child(pid));
}

#[test]
fn i4_24_dropping_an_unresolved_operation_is_best_effort_and_confirms_nothing() {
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    let before = world.lock().calls.len();
    drop(boundary);
    let state = world.lock();
    let after = &state.calls[before..];
    // Its candidate is ended, without waiting and without the manager.
    assert!(after.iter().any(|call| matches!(call, Call::Kill(_))));
    assert!(after.iter().all(|call| matches!(call, Call::Kill(_))));
    assert!(state.units.contains_key(&state.requested().unwrap()));
    drop(state);
    // The helper is killed but never reaped while its operation is
    // unresolved: reaped here, by this test, as this process's own child.
    assert!(is_child(pid), "a dropped boundary reaped the helper");
    let start = Instant::now();
    while is_running(pid) && start.elapsed() < FINALIZE_TIMEOUT {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!is_running(pid));
    // SAFETY: an unreaped child of this process; nothing else reaps it.
    assert_eq!(
        unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) },
        pid as libc::pid_t
    );
}

#[test]
fn i4_25_a_panic_after_the_start_request_keeps_its_owner() {
    let fault = Some(Fault::Panic(FaultPoint::AfterScopeStart));
    // Settled through the cgroup the helper is found in.
    let world = World::default();
    let report = run_in(&world, fault);
    assert!(
        world.lock().created().unwrap().killed,
        "a panic after StartTransientUnit lost the operation's owner"
    );
    assert!(matches!(report.not_run, Some(NotRun::Interrupted)));
    assert_eq!(report.stdout.bytes, 0);
    assert!(report.cleanup.is_confirmed());
    // Nothing can be confirmed: retained.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, fault);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a panic after StartTransientUnit lost the operation's owner"
    );
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).issued && is_child(pid));
    world.release();
    boundary.retry().unwrap();
}

#[test]
fn i4_26_a_panic_after_the_candidate_is_retained_keeps_it() {
    for (point, phantom) in [
        (FaultPoint::ScopeCandidate, Phantom::None),
        (FaultPoint::ScopeProperties, Phantom::None),
        (FaultPoint::ScopeCandidate, Phantom::Forever),
    ] {
        let world = World::new(|state| {
            state.opens_left = Some(1);
            state.phantom = phantom;
            state.stop = Script::always((false, Reply::Timeout));
        });
        let report = run_in(&world, Some(Fault::Panic(point)));
        assert!(
            matches!(report.not_run, Some(NotRun::Interrupted)),
            "{point:?}"
        );
        assert_eq!(report.stdout.bytes, 0, "{point:?}");
        if phantom == Phantom::None {
            assert!(
                report.cleanup.is_confirmed(),
                "{point:?}: the candidate was lost"
            );
        } else {
            let boundary = boundary_of(report);
            assert!(pending_state(&boundary).candidate, "{point:?}");
            world.release();
            world.lock().opens_left = Some(0);
            boundary.retry().unwrap();
        }
        assert_eq!(opens(&world), 1, "{point:?}");
    }
    // Before the candidate is opened: it is found again through the
    // helper's membership.
    let world = World::default();
    let report = run_in(&world, Some(Fault::Panic(FaultPoint::ScopeProof)));
    assert!(matches!(report.not_run, Some(NotRun::Interrupted)));
    assert!(report.cleanup.is_confirmed());
    assert!(world.lock().created().unwrap().killed);
}

#[test]
fn i4_27_a_panic_while_stopping_or_reconciling_retains_the_operation() {
    for point in [
        FaultPoint::BeforeScopeStop,
        FaultPoint::AfterScopeStop,
        FaultPoint::ScopeReconcile,
    ] {
        let world = World::new(|state| state.placement = Placement::Never);
        let report = run_in(&world, Some(Fault::Panic(point)));
        let pid = requested_pid(&world);
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{point:?}");
        let boundary = boundary_of(report);
        assert!(pending_state(&boundary).issued, "{point:?}");
        assert!(boundary.holds_helper() && is_child(pid), "{point:?}");
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{point:?}");
    }
}

#[test]
fn i4_28_the_helper_and_the_proven_scope_precede_any_launch() {
    // Owned before any scope request: a panic just before the request
    // leaves only the helper, which finalization ends.
    let world = World::default();
    let report = run_in(&world, Some(Fault::Panic(FaultPoint::BeforeScopeStart)));
    assert!(matches!(report.not_run, Some(NotRun::Interrupted)));
    assert!(report.cleanup.is_confirmed());
    assert!(world.lock().calls.is_empty(), "nothing was requested");
    // The proven scope holds the helper before the launch: a panic between
    // them ends the scope as before, and nothing launched.
    let world = World::default();
    let report = run_in(&world, Some(Fault::Panic(FaultPoint::AfterScope)));
    assert!(matches!(report.not_run, Some(NotRun::Interrupted)));
    assert_eq!(report.stdout.bytes, 0);
    assert!(report.cleanup.is_confirmed());
    assert!(world.lock().created().unwrap().killed);
}

#[test]
fn i4_29_the_classification_of_settled_executions_is_unchanged() {
    // A proven scope and a refused launch.
    let report = run_in(&World::default(), None);
    assert_eq!(report.classify(0), ExitClass::SandboxSetupFailed);
    // No scope, and nothing it may have created remains.
    for script in [
        (|state: &mut crate::scope::tests::State| state.limit_mismatch = Some("pids.max"))
            as fn(&mut crate::scope::tests::State),
        |state| {
            state.start_effect = false;
            state.start_reply = Reply::Error;
        },
    ] {
        let report = run_in(&World::new(script), None);
        assert!(report.cleanup.is_confirmed());
        assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    }
}

#[test]
fn i4_30_the_helper_s_death_is_never_the_operation_s_cleanup() {
    // The helper is killed, but the cgroup it was found in stays populated.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "the helper's death was taken as the operation's cleanup"
    );
    let boundary = boundary_of(report);
    let start = Instant::now();
    while is_running(pid) && start.elapsed() < FINALIZE_TIMEOUT {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!is_running(pid) && is_child(pid), "killed, never reaped");
    world.release();
    boundary.retry().unwrap();
}

#[test]
fn i4_30_no_cgroup_of_the_unit_s_name_is_ever_taken_by_its_path() {
    // A cgroup of the unit's name holds someone else's process, and the
    // kernel never reports the helper in it: it is neither opened nor
    // ended, and the operation stays owned while the manager has the unit.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.phantom = Phantom::UntilKill;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    assert_eq!(
        opens(&world),
        0,
        "a cgroup was opened by a path the kernel never reported for the helper"
    );
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    {
        let state = world.lock();
        let cgroup = state.created().unwrap();
        assert!(cgroup.populated() && cgroup.kills == 0);
    }
    let boundary = boundary_of(report);
    world.lock().stop = Script::always((true, Reply::Delivered));
    boundary.retry().unwrap();
    assert_eq!(opens(&world), 0);
}
