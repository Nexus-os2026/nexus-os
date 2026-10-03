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
use crate::scope::tests::{
    abandon, Call, Phantom, Placement, Property, Reply, Script, World, SLICE,
};

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
    let unproven: [fn(&mut crate::scope::tests::State); 7] = [
        |state| state.runtime_max = Property::Uncertain(Reply::Timeout),
        |state| state.oom_policy = Property::Uncertain(Reply::Disconnect),
        |state| state.get_unit = Script::first([Reply::Malformed], Reply::Delivered),
        |state| state.limit_mismatch = Some("cpu.max"),
        |state| state.listed = false,
        |state| state.unit_loaded = false,
        |state| state.placement = Placement::Never,
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
    // An uncertain start that created nothing (I4-R1): never confirmed gone,
    // so its output is not waited for; the attempt ended at the scope,
    // before any handshake, and its helper is retained unreaped.
    let world = World::new(|state| {
        state.start_effect = false;
        state.start_reply = Reply::Timeout;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(
        matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
        "{:?}",
        report.not_run
    );
    assert!(report.outcome.is_none() && report.ended_by.is_none());
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    abandon(boundary_of(report), pid);
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
    // I4-R1: without a candidate, nothing the manager answers later
    // confirms an uncertain start; a retry keeps it, with its helper.
    world.release();
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
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
    // Nothing can be confirmed: retained. The reply was delivered, but the
    // panic came before it was recorded, so for the backend the request's
    // outcome is uncertain (I4-R1): a retry keeps it, with its helper.
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
    let observed = pending_state(&boundary);
    assert!(observed.issued && !observed.accepted && is_child(pid));
    world.release();
    let boundary = boundary.retry().unwrap_err();
    abandon(boundary, pid);
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
    // No scope, and nothing it may have created remains: settled through
    // the retained candidate, or (a delivered start whose helper was never
    // placed) stopped and observed unloaded.
    for script in [
        (|state: &mut crate::scope::tests::State| state.limit_mismatch = Some("pids.max"))
            as fn(&mut crate::scope::tests::State),
        |state| state.placement = Placement::Never,
    ] {
        let report = run_in(&World::new(script), None);
        assert!(report.cleanup.is_confirmed());
        assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    }
    // I4-R1: an uncertain start without a candidate is never settled, even
    // when it created nothing: an unconfirmed cleanup, never an unavailable
    // sandbox.
    let world = World::new(|state| {
        state.start_effect = false;
        state.start_reply = Reply::Error;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(matches!(report.not_run, Some(NotRun::Scope(_))));
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    abandon(boundary_of(report), pid);
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

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R1: the owners of a scope operation, the binding of the
// manager's unit to the kernel's cgroup, uncertain starts without a
// candidate, and the helper's identity. Over the deterministic simulation
// only; no test here contacts a bus or creates a cgroup.

use std::sync::atomic::AtomicU64;

use crate::launcher::{allocate_serial, SPAWNED};
use crate::scope::tests::{expand, Op, State, Text};

const LIMITS: ResourcePolicy = ResourcePolicy::RUST_OFFLINE_V1;

/// A stand-in helper that only holds its control socket open (`cat`).
fn cat() -> Helper {
    Helper::spawn(&HelperProgram::at("/bin/cat")).unwrap().0
}

/// Place a `cat` stand-in through the live harness's direct owner.
fn place_cat(world: &World) -> (Result<ScopedHelper, PlacementFailed>, u32) {
    let helper = cat();
    let pid = helper.pid();
    (place(&world.scopes(), helper, &LIMITS), pid)
}

/// An uncertain StartTransientUnit whose helper is never placed, so that
/// no candidate can be retained; `effect`: whether the manager created the
/// unit.
fn uncertain_without_candidate(state: &mut State, effect: bool, reply: Reply) {
    state.start_effect = effect;
    state.start_reply = reply;
    state.placement = Placement::Never;
}

/// The stand-in's launch reached it: the attempt went past the handshake.
fn launched(report: &ExecutionReport) -> bool {
    report.stdout.bytes > 0
}

#[test]
fn i4r1_01_a_panic_after_an_uncertain_start_without_a_candidate_keeps_its_owner() {
    // The manager created nothing: asked, it would answer NoSuchUnit, and
    // the helper is outside. A panic after the request, before or after its
    // outcome was recorded, neither loses nor releases the operation.
    for point in [
        FaultPoint::AfterScopeStart,
        FaultPoint::ScopeReconcile,
        FaultPoint::BeforeScopeStop,
        FaultPoint::AfterScopeStop,
    ] {
        let world = World::new(|state| uncertain_without_candidate(state, false, Reply::Timeout));
        let report = run_in(&world, Some(Fault::Panic(point)));
        let pid = requested_pid(&world);
        assert!(!launched(&report), "{point:?}");
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{point:?}: a panic after an uncertain start lost or released its operation"
        );
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.issued && !observed.accepted && !observed.settled,
            "{point:?}: {observed:?}"
        );
        assert!(boundary.holds_helper() && is_child(pid), "{point:?}");
        world.release();
        let boundary = boundary.retry().unwrap_err();
        abandon(boundary, pid);
    }
    // A panic inside the manager's own StartTransientUnit, after its effect
    // and before any reply, through the harness's direct owner: one owner
    // of the operation and its helper (StopUnit even unloads the unit).
    let world = World::new(|state| {
        uncertain_without_candidate(state, true, Reply::Delivered);
        state.panic_at = Some(Op::Start);
    });
    let Ok((placed, pid)) = catch_unwind(AssertUnwindSafe(|| place_cat(&world))) else {
        panic!("a panic crossed the direct owner of an unresolved operation");
    };
    let failed = placed.expect_err("nothing was proven");
    assert!(failed.error.is_none(), "{:?}", failed.error);
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("a panic inside StartTransientUnit lost or released its operation");
    };
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    assert!(
        world.lock().units.is_empty(),
        "the unit was stopped and unloaded"
    );
    abandon(boundary, pid);
}

#[test]
fn i4r1_02_a_direct_placement_failure_returns_one_owner_of_the_operation_and_its_helper() {
    let world = World::new(|state| uncertain_without_candidate(state, true, Reply::Disconnect));
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("nothing was proven");
    assert!(
        matches!(failed.error, Some(ScopeError::Bus(_))),
        "{:?}",
        failed.error
    );
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("the direct owner lost the unresolved operation");
    };
    assert!(
        boundary.holds_scope(),
        "the direct owner lost the unresolved operation"
    );
    assert!(
        boundary.holds_helper() && is_child(pid),
        "the operation's failure was split from its helper"
    );
    let observed = pending_state(&boundary);
    assert!(observed.issued && !observed.accepted && !observed.candidate && !observed.settled);
    // The retry stays explicit, and keeps both together.
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

#[test]
fn i4r1_03_dropping_a_direct_owner_is_defense_only_and_never_a_confirmation() {
    // An unresolved failure, dropped: what it holds is ended without
    // waiting, without the manager, and its helper is never reaped.
    let world = World::new(|state| {
        uncertain_without_candidate(state, true, Reply::Timeout);
        state.stop = Script::always((false, Reply::Timeout));
    });
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("nothing was proven");
    assert!(!failed.cleanup.is_confirmed());
    let before = world.lock().calls.len();
    drop(failed);
    {
        let state = world.lock();
        assert!(
            state.calls[before..].is_empty(),
            "a dropped failure asked the manager or the kernel: {:?}",
            &state.calls[before..]
        );
        assert!(state.units.contains_key(&state.requested().unwrap()));
    }
    assert!(
        is_child(pid),
        "a dropped direct failure reaped the helper of its unresolved operation"
    );
    let start = Instant::now();
    while is_running(pid) && start.elapsed() < FINALIZE_TIMEOUT {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !is_running(pid),
        "the dropped failure did not end its helper"
    );
    // SAFETY: an unreaped child of this process; nothing else reaps it.
    assert_eq!(
        unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) },
        pid as libc::pid_t
    );
    // A proven direct owner, dropped: its scope is ended through the
    // retained descriptor, without the manager; only `end` confirms.
    let world = World::default();
    let (placed, _pid) = place_cat(&world);
    let placed = placed.unwrap();
    let before = world.lock().calls.len();
    drop(placed);
    let state = world.lock();
    assert!(state.calls[before..]
        .iter()
        .all(|call| matches!(call, Call::Kill(_))));
    assert!(state.created().unwrap().killed);
    assert_eq!(state.count(|call| matches!(call, Call::Stop(_))), 0);
}

#[test]
fn i4r1_06_a_cgroup_of_the_unit_s_name_in_another_place_is_never_proven() {
    for beside in [
        "/other.slice/{unit}",
        "{slice}/nested.slice/{unit}",
        "/{unit}",
    ] {
        let world = World::new(|state| state.placement = Placement::Beside(beside));
        let report = run_in(&world, None);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
            ),
            "{beside}: a cgroup of the unit's name that is not the unit's own was proven: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{beside}");
        // The cgroup the helper was found in stays cleanup ownership: it is
        // ended and confirmed empty through its retained descriptor; the
        // unit's own cgroup is never opened by its path.
        assert!(report.cleanup.is_confirmed(), "{beside}");
        let state = world.lock();
        let unit = state.requested().unwrap();
        let found = expand(beside, &unit);
        assert!(state
            .cgroups
            .iter()
            .any(|cgroup| cgroup.path == found && cgroup.killed));
        assert_eq!(
            state
                .calls
                .iter()
                .filter(|call| matches!(call, Call::Open(_)))
                .collect::<Vec<_>>(),
            [&Call::Open(found)],
            "{beside}"
        );
    }
    // Through the harness's direct owner: not proven either.
    let world = World::new(|state| state.placement = Placement::Beside("/other.slice/{unit}"));
    let (placed, pid) = place_cat(&world);
    let failed =
        placed.expect_err("a cgroup of the unit's name that is not the unit's own was proven");
    assert!(matches!(
        failed.error,
        Some(ScopeError::Mismatch("unit control group"))
    ));
    assert!(failed.cleanup.is_confirmed() && !is_child(pid));
}

#[test]
fn i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate() {
    // The manager's unit really is at these paths and would report them as
    // its control group; the kernel names them for the helper. A path that
    // is not absolute and in normal form locates nothing.
    for path in ["{slice}/./{unit}", "{slice}//{unit}", "{unit}"] {
        let world = World::new(|state| state.placement = Placement::Elsewhere(path));
        let report = run_in(&world, None);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::NotPlaced))),
            "{path}: a membership path not in normal form located the scope: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{path}");
        assert_eq!(opens(&world), 0, "{path}");
        assert!(report.cleanup.is_confirmed(), "{path}");
    }
}

#[test]
fn i4r1_07_a_control_group_that_is_not_exactly_the_kernel_s_is_refused() {
    // The kernel reports the helper in the unit's own cgroup; the manager
    // reports another control group for the unit.
    for group in [
        "/other.slice/{unit}",
        "{slice}/x-{unit}",
        "{slice}/{unit}/child",
        "{slice}/{unit}/",
        "{slice}//{unit}",
        "{slice}/./{unit}",
        "{slice}/../app.slice/{unit}",
        "{unit}",
        "/",
        "",
    ] {
        let world = World::new(|state| state.control_group = Text::Is(group));
        let report = run_in(&world, None);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
            ),
            "{group:?}: a control group that is not exactly the kernel's was accepted: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{group:?}");
        assert!(report.cleanup.is_confirmed(), "{group:?}");
        assert!(world.lock().created().unwrap().killed, "{group:?}");
    }
}

#[test]
fn i4r1_08_an_unavailable_or_uncertain_control_group_is_refused() {
    for group in [
        Text::WrongType,
        Text::Uncertain(Reply::Timeout),
        Text::Uncertain(Reply::Error),
        Text::Uncertain(Reply::Malformed),
        Text::Uncertain(Reply::Disconnect),
    ] {
        let world = World::new(|state| state.control_group = group);
        let report = run_in(&world, None);
        let refused = match group {
            Text::WrongType => matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
            ),
            _ => matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
        };
        assert!(
            refused,
            "{group:?}: an unavailable or malformed control group was accepted: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{group:?}");
        assert!(report.cleanup.is_confirmed(), "{group:?}");
    }
}

#[test]
fn i4r1_09_a_unit_id_that_is_not_exactly_the_generated_name_is_refused() {
    for id in [
        Text::Is("other.scope"),
        Text::Is("x{unit}"),
        Text::Is("{unit}.alias"),
        Text::Is(""),
        Text::WrongType,
        Text::Uncertain(Reply::Timeout),
        Text::Uncertain(Reply::Error),
    ] {
        let world = World::new(|state| state.unit_id = id);
        let report = run_in(&world, None);
        let refused = match id {
            Text::Uncertain(_) => {
                matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_))))
            }
            _ => matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit id")))
            ),
        };
        assert!(
            refused,
            "{id:?}: a unit id that is not the generated name was accepted: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{id:?}");
        assert!(report.cleanup.is_confirmed(), "{id:?}");
        // Refused before the control group or any property is asked for.
        assert_eq!(
            world.lock().count(|call| matches!(
                call,
                Call::ControlGroup(_) | Call::RuntimeMax(_) | Call::OomPolicy(_)
            )),
            0,
            "{id:?}"
        );
    }
}

#[test]
fn i4r1_10_the_exact_binding_of_the_manager_s_unit_to_the_kernel_s_cgroup_proves() {
    let world = World::default();
    let (placed, pid) = place_cat(&world);
    let placed = placed.unwrap();
    {
        let state = world.lock();
        let unit = state.requested().unwrap();
        let object = format!("/unit/{unit}");
        assert_eq!(
            state.calls,
            [
                Call::Start(unit.clone(), pid),
                Call::Membership(pid, true),
                Call::Open(format!("{SLICE}/{unit}")),
                Call::GetUnit(unit.clone()),
                Call::UnitId(object.clone()),
                Call::ControlGroup(object.clone()),
                Call::RuntimeMax(object.clone()),
                Call::OomPolicy(object),
            ],
            "the scope was proven without binding the manager's unit to the kernel's cgroup"
        );
    }
    assert_eq!(
        placed.scope().unwrap().occupancy().unwrap(),
        crate::scope::Occupancy::Populated
    );
    assert!(placed.end().is_confirmed());
    assert!(!is_child(pid));
    // An execution: the launch reaches the helper only after the binding.
    let world = World::default();
    let report = run_in(&world, None);
    assert!(launched(&report) && report.cleanup.is_confirmed());
}

#[test]
fn i4r1_11_an_uncertain_start_without_a_candidate_is_not_released_by_no_such_unit() {
    for reply in [
        Reply::Timeout,
        Reply::Error,
        Reply::Malformed,
        Reply::Disconnect,
    ] {
        // The manager created nothing: asked, it would answer NoSuchUnit,
        // and the kernel reports the helper outside any cgroup of the name.
        let world = World::new(|state| uncertain_without_candidate(state, false, reply));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{reply:?}: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{reply:?}");
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{reply:?}: an uncertain start without a candidate was released by the manager's absence"
        );
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.issued && !observed.accepted && !observed.candidate && !observed.settled,
            "{reply:?}: {observed:?}"
        );
        assert!(boundary.holds_helper() && is_child(pid), "{reply:?}");
        {
            let state = world.lock();
            assert!(state.units.is_empty() && !state.membership.contains_key(&pid));
        }
        // The manager answers again: a retry still confirms nothing.
        world.release();
        let boundary = boundary.retry().unwrap_err();
        abandon(boundary, pid);
    }
}

#[test]
fn i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_by_a_stop_reply() {
    // The manager created the unit; StopUnit stops and unloads it and its
    // reply is delivered; the kernel never reported the helper in it.
    let world = World::new(|state| {
        uncertain_without_candidate(state, true, Reply::Timeout);
        state.stop = Script::always((true, Reply::Delivered));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(stops(&world) >= 1, "StopUnit was attempted");
    assert!(world.lock().units.is_empty(), "the stop took effect");
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a StopUnit reply released an uncertain start without a candidate"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.stops >= 1 && !observed.settled && !observed.candidate);
    assert!(boundary.holds_helper() && is_child(pid));
    let boundary = boundary.retry().unwrap_err();
    abandon(boundary, pid);
}

#[test]
fn i4r1_13_an_uncertain_start_without_a_candidate_is_not_released_by_a_timed_out_stop() {
    for effect in [true, false] {
        let world = World::new(|state| {
            uncertain_without_candidate(state, true, Reply::Timeout);
            state.stop = Script::always((effect, Reply::Timeout));
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{effect}: a timed-out StopUnit released an uncertain start without a candidate"
        );
        let boundary = boundary_of(report);
        assert!(
            boundary.holds_scope() && boundary.holds_helper(),
            "{effect}"
        );
        assert!(
            is_child(pid),
            "{effect}: the helper of an unresolved uncertain start was reaped"
        );
        let boundary = boundary.retry().unwrap_err();
        assert!(
            is_child(pid),
            "{effect}: the helper of an unresolved uncertain start was reaped"
        );
        abandon(boundary, pid);
    }
}

#[test]
fn i4r1_14_an_uncertain_start_that_places_the_helper_is_acquired_and_may_be_proven() {
    // Placed while the proof waits: every proof passes, and only then does
    // the launch reach the helper.
    for reply in [Reply::Timeout, Reply::Error, Reply::Malformed] {
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = Placement::AfterReads(3);
        });
        let report = run_in(&world, None);
        assert!(
            launched(&report) && report.cleanup.is_confirmed(),
            "{reply:?}: {:?}",
            report.not_run
        );
        assert_eq!(stops(&world), 0, "{reply:?}");
    }
    // Placed, then a later proof fails: the candidate is retained, ended and
    // confirmed through its descriptor.
    let world = World::new(|state| {
        state.start_reply = Reply::Timeout;
        state.control_group = Text::Is("/other.slice/{unit}");
    });
    let report = run_in(&world, None);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
    ));
    assert!(!launched(&report) && report.cleanup.is_confirmed());
    assert!(world.lock().created().unwrap().killed);
    // Never placed while the execution ran (StopUnit never acts); a retry
    // then finds the helper in the unit's cgroup: acquired, ended and
    // confirmed through it.
    let world = World::new(|state| {
        uncertain_without_candidate(state, true, Reply::Timeout);
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    assert!(!pending_state(&boundary).candidate);
    world.lock().place(pid, 0);
    boundary.retry().unwrap();
    assert!(!is_child(pid));
    let state = world.lock();
    assert!(state.cgroups[0].killed);
    assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 1);
}

#[test]
fn i4r1_15_a_collision_never_stops_or_claims_the_foreign_unit() {
    for holds in [false, true] {
        let world = World::new(|state| {
            state.collide = true;
            state.collision_holds_helper = holds;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{holds}"
        );
        assert!(!launched(&report), "{holds}");
        if holds {
            // The helper is in the foreign unit's cgroup: nothing confirms
            // the operation, and retries still never touch that unit.
            let boundary = boundary_of(report);
            assert!(pending_state(&boundary).collided);
            let boundary = boundary.retry().unwrap_err();
            world.lock().membership.remove(&pid);
            boundary.retry().unwrap();
        } else {
            assert!(report.cleanup.is_confirmed());
        }
        let state = world.lock();
        assert_eq!(
            state.count(|call| matches!(call, Call::Stop(_))),
            0,
            "{holds}: the foreign unit was stopped"
        );
        assert!(
            state.calls.iter().all(|call| !matches!(
                call,
                Call::Open(_)
                    | Call::Kill(_)
                    | Call::GetUnit(_)
                    | Call::UnitId(_)
                    | Call::ControlGroup(_)
            )),
            "{holds}: the foreign unit was claimed"
        );
        assert!(!state.units[&state.requested().unwrap()].ours);
        drop(state);
        assert!(!is_child(pid));
    }
}

#[test]
fn i4r1_16_a_collision_is_never_proven_by_the_foreign_cgroup_s_name() {
    // The foreign unit's cgroup has the requested name, holds the helper
    // with exactly the requested limits, and the manager would bind it
    // exactly; but this request created nothing.
    let collide = |state: &mut State| {
        state.collide = true;
        state.collision_holds_helper = true;
    };
    let world = World::new(collide);
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("a collision was proven by the foreign cgroup's name");
    assert!(
        matches!(failed.error, Some(ScopeError::Bus(_))),
        "{:?}",
        failed.error
    );
    assert_eq!(
        world.lock().count(|call| matches!(
            call,
            Call::Open(_) | Call::GetUnit(_) | Call::UnitId(_) | Call::ControlGroup(_)
        )),
        0,
        "a collision was proven by the foreign cgroup's name"
    );
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("the helper is in the foreign cgroup: nothing is confirmed");
    };
    world.lock().membership.remove(&pid);
    boundary.retry().unwrap();
    // Through an execution: nothing reaches the helper.
    let world = World::new(collide);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(!launched(&report));
    let boundary = boundary_of(report);
    world.lock().membership.remove(&pid);
    boundary.retry().unwrap();
}

#[test]
fn i4r1_17_no_launch_while_the_unit_binding_is_unresolved() {
    let unresolved: [fn(&mut State); 8] = [
        |state| state.unit_id = Text::Uncertain(Reply::Timeout),
        |state| state.unit_id = Text::Uncertain(Reply::Disconnect),
        |state| state.unit_id = Text::WrongType,
        |state| state.control_group = Text::Uncertain(Reply::Timeout),
        |state| state.control_group = Text::Uncertain(Reply::Malformed),
        |state| state.control_group = Text::WrongType,
        |state| state.control_group = Text::Is("/other.slice/{unit}"),
        |state| state.panic_at = Some(Op::ControlGroup),
    ];
    for (case, script) in unresolved.into_iter().enumerate() {
        let world = World::new(script);
        let report = run_in(&world, None);
        assert!(report.cleanup.is_confirmed(), "case {case}");
        assert!(
            !launched(&report),
            "case {case}: a launch message reached the helper before the binding was proven"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(_) | NotRun::Interrupted)),
            "case {case}: {:?}",
            report.not_run
        );
        assert_eq!(
            world
                .lock()
                .count(|call| matches!(call, Call::RuntimeMax(_) | Call::OomPolicy(_))),
            0,
            "case {case}: a property was proven before the binding"
        );
    }
    // A panic injected where the binding is checked.
    let world = World::default();
    let report = run_in(&world, Some(Fault::Panic(FaultPoint::ScopeBinding)));
    assert!(matches!(report.not_run, Some(NotRun::Interrupted)));
    assert!(!launched(&report) && report.cleanup.is_confirmed());
    assert!(world.lock().created().unwrap().killed);
}

#[test]
fn i4r1_18_a_proven_execution_s_lifecycle_is_unchanged() {
    let world = World::default();
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(launched(&report), "the launch reached the helper");
    assert!(report.cleanup.is_confirmed());
    assert_eq!(report.events, Some(ScopeEvents::default()));
    let state = world.lock();
    let unit = state.requested().unwrap();
    let object = format!("/unit/{unit}");
    // The proof, the binding included; then finalization through the
    // retained descriptor only.
    assert_eq!(
        state.calls[..8],
        [
            Call::Start(unit.clone(), pid),
            Call::Membership(pid, true),
            Call::Open(format!("{SLICE}/{unit}")),
            Call::GetUnit(unit.clone()),
            Call::UnitId(object.clone()),
            Call::ControlGroup(object.clone()),
            Call::RuntimeMax(object.clone()),
            Call::OomPolicy(object),
        ]
    );
    assert!(state.calls[8..]
        .iter()
        .all(|call| matches!(call, Call::Kill(_))));
    assert!(state.created().unwrap().kills >= 1);
    drop(state);
    assert!(!is_child(pid));
}

#[test]
fn i4r1_19_a_binding_failure_s_candidate_is_retried_without_the_scope_manager() {
    let world = World::new(|state| {
        state.control_group = Text::Is("/other.slice/{unit}");
        state.phantom = Phantom::Forever;
        state.opens_left = Some(1);
        state.stop = Script::always((false, Reply::Timeout));
    });
    let stand_in = StandIn::new();
    let scopes = world.scopes();
    let report = execute(&scopes, &stand_in.program(), launch_spec(), &LIMITS, None);
    let pid = requested_pid(&world);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
    ));
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).candidate);
    drop(scopes);
    world.release();
    world.lock().opens_left = Some(0);
    assert!(
        boundary.retry().is_ok(),
        "a retained boundary needs the ScopeManager that started it"
    );
    assert!(!is_child(pid));
    assert_eq!(opens(&world), 1);
}

#[test]
fn i4r1_20_helper_identities_are_refused_instead_of_wrapping() {
    let next = AtomicU64::new(u64::MAX - 2);
    assert_eq!(allocate_serial(&next), Some(u64::MAX - 2));
    assert_eq!(allocate_serial(&next), Some(u64::MAX - 1));
    for _ in 0..3 {
        assert_eq!(allocate_serial(&next), None, "the helper identity wrapped");
    }
    assert_eq!(next.load(Ordering::SeqCst), u64::MAX);
    // Nonzero and increasing; zero is never issued.
    let next = AtomicU64::new(1);
    let issued: Vec<_> = (0..4).map(|_| allocate_serial(&next)).collect();
    assert_eq!(issued, [Some(1), Some(2), Some(3), Some(4)]);
    assert_eq!(
        allocate_serial(&AtomicU64::new(0)),
        None,
        "the helper identity wrapped"
    );
}

#[test]
fn i4r1_21_no_helper_is_spawned_once_identities_are_exhausted() {
    let program = HelperProgram::at("/bin/cat");
    let spawned = || SPAWNED.with(std::cell::Cell::get);
    let before = spawned();
    let exhausted = AtomicU64::new(u64::MAX);
    assert!(matches!(
        Helper::spawn_with(&program, &exhausted),
        Err(LaunchError::IdentitiesExhausted)
    ));
    assert_eq!(
        spawned(),
        before,
        "a helper was spawned without an identity"
    );
    // With the last identity: one child, bound to it; then none.
    let next = AtomicU64::new(u64::MAX - 1);
    let (helper, _output) = Helper::spawn_with(&program, &next).unwrap();
    assert_eq!(helper.serial(), u64::MAX - 1);
    assert_eq!(spawned(), before + 1);
    assert!(matches!(
        Helper::spawn_with(&program, &next),
        Err(LaunchError::IdentitiesExhausted)
    ));
    assert_eq!(
        spawned(),
        before + 1,
        "a helper was spawned without an identity"
    );
    let mut helper = helper;
    let _ = helper.kill();
    helper.reap().unwrap();
}

#[test]
fn i4r1_23_the_harness_owner_lets_the_helper_hold_its_scope_until_released() {
    // The live scope-hold test's sequence over the model: placed and owned
    // with its scope, launched only then; after its final report the
    // helper still holds the scope and its counters; the scope is ended
    // through its descriptor, the helper reaped, and the owner confirms.
    let world = World::default();
    let stand_in = StandIn::new();
    let (helper, _output) = Helper::spawn(&stand_in.program()).unwrap();
    let pid = helper.pid();
    let mut placed = place(&world.scopes(), helper, &LIMITS).unwrap();
    {
        let helper = placed.helper().unwrap();
        helper.handshake().unwrap();
        // The stand-in refuses the launch: its final report.
        assert!(matches!(
            helper.launch(launch_spec()),
            Err(LaunchError::SetupFailed { .. })
        ));
    }
    let scope = placed.scope().unwrap();
    assert_eq!(
        scope.occupancy().unwrap(),
        crate::scope::Occupancy::Populated
    );
    assert_eq!(scope.events().unwrap(), ScopeEvents::default());
    scope.kill().unwrap();
    // The model's `cgroup.kill` ends no real process: this test ends it.
    // SAFETY: the stand-in is this process's unreaped child.
    assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
    // Never while an operation is unresolved; here the scope is proven.
    placed.reap_helper().unwrap();
    assert!(!is_child(pid));
    assert!(placed
        .scope()
        .unwrap()
        .wait_empty(Duration::from_secs(1))
        .unwrap());
    assert!(placed.end().is_confirmed());
    assert_eq!(world.lock().count(|call| matches!(call, Call::Stop(_))), 0);
}

#[test]
fn i4r1_24_unit_tests_never_construct_the_real_manager_or_kernel() {
    // Every unit test reaches the scope module through the model; only the
    // production constructors name the real ones (the needles are split so
    // that this test does not name them).
    let needles = [
        concat!("ScopeManager::", "connect"),
        concat!("Zbus", "Manager"),
        concat!("Box::new(", "Kernel)"),
        concat!("Timing::", "PRODUCTION"),
        concat!("/run/", "user/"),
    ];
    for (name, source) in [
        ("scope/tests.rs", include_str!("../scope/tests.rs")),
        ("execution/tests.rs", include_str!("tests.rs")),
    ] {
        // Code only: a comment may name what is never called.
        let code: String = source
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in needles {
            assert!(!code.contains(needle), "{name} names {needle}");
        }
    }
}

/// Whether a panic left confirmed cleanup or one owner of everything still
/// unconfirmed (the operation, if unresolved, with its unreaped helper);
/// the boundary, if any, is given up afterwards.
fn judge_panic(world: &World, case: &str, cleanup: Cleanup, pid: u32) {
    match cleanup {
        Cleanup::Confirmed => {
            // Confirmed only with nothing left: no cgroup of the model
            // still holds a process, and the helper is reaped.
            let state = world.lock();
            assert!(
                state.cgroups.iter().all(|cgroup| !cgroup.populated()),
                "{case}: confirmed with a populated cgroup"
            );
            drop(state);
            assert!(!is_child(pid), "{case}: confirmed with its helper unreaped");
        }
        Cleanup::Failed(boundary) => {
            assert!(
                boundary.holds_helper() && is_child(pid),
                "{case}: the retained boundary lost its helper"
            );
            if let Some(observed) = boundary.pending() {
                assert!(observed.issued && !observed.settled, "{case}: {observed:?}");
            }
            world.release();
            match boundary.retry() {
                Ok(()) => assert!(!is_child(pid), "{case}"),
                Err(boundary) => abandon(boundary, pid),
            }
        }
    }
}

#[test]
fn i4r1_x_a_panic_at_any_ownership_point_leaves_confirmation_or_one_owner() {
    // Each panic boundary, for an accepted and an uncertain start whose
    // helper is placed (a candidate) or never placed (none).
    let starts: [(Reply, Placement); 4] = [
        (Reply::Delivered, Placement::Immediate),
        (Reply::Delivered, Placement::Never),
        (Reply::Timeout, Placement::Immediate),
        (Reply::Timeout, Placement::Never),
    ];
    let points = [
        FaultPoint::BeforeScopeStart,
        FaultPoint::AfterScopeStart,
        FaultPoint::ScopeProof,
        FaultPoint::ScopeCandidate,
        FaultPoint::ScopeBinding,
        FaultPoint::ScopeProperties,
        FaultPoint::ScopeReconcile,
        FaultPoint::BeforeScopeStop,
        FaultPoint::AfterScopeStop,
        FaultPoint::AfterScope,
        FaultPoint::Finalizing,
    ];
    let ops = [
        Op::Start,
        Op::Membership,
        Op::Open,
        Op::GetUnit,
        Op::UnitId,
        Op::ControlGroup,
        Op::RuntimeMax,
        Op::Stop,
    ];
    for (reply, placement) in starts {
        let script = move |state: &mut State| {
            state.start_reply = reply;
            state.placement = placement;
        };
        // Execution: an injected fault, and a panic inside the model.
        for point in points {
            let world = World::new(script);
            let report = run_in(&world, Some(Fault::Panic(point)));
            let pid = requested_pid_or_spawned(&world, &report);
            if let Some(pid) = pid {
                judge_panic(
                    &world,
                    &format!("{reply:?} {placement:?} {point:?}"),
                    report.cleanup,
                    pid,
                );
            } else {
                assert!(report.cleanup.is_confirmed());
            }
        }
        for op in ops {
            let world = World::new(|state| {
                script(state);
                state.panic_at = Some(op);
            });
            let report = run_in(&world, None);
            let pid = requested_pid(&world);
            judge_panic(
                &world,
                &format!("{reply:?} {placement:?} run {op:?}"),
                report.cleanup,
                pid,
            );
            // The harness's direct owner.
            let world = World::new(|state| {
                script(state);
                state.panic_at = Some(op);
            });
            let (placed, pid) = place_cat(&world);
            let cleanup = match placed {
                Ok(placed) => placed.end(),
                Err(failed) => failed.cleanup,
            };
            judge_panic(
                &world,
                &format!("{reply:?} {placement:?} place {op:?}"),
                cleanup,
                pid,
            );
        }
    }
    // A panic while retrying a retained boundary keeps it.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    world.lock().panic_at = Some(Op::Stop);
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

/// The helper's process id if the scope was requested; `None` when the
/// execution never reached the request (its helper then ended with it).
fn requested_pid_or_spawned(world: &World, report: &ExecutionReport) -> Option<u32> {
    let pid = world.lock().calls.iter().find_map(|call| match call {
        Call::Start(_, pid) => Some(*pid),
        _ => None,
    });
    if pid.is_none() {
        assert!(report.cleanup.is_confirmed());
    }
    pid
}
