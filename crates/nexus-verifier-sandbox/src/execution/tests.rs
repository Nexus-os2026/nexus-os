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

/// The manager ends the requested unit by itself and collects it
/// (P2-V1-R3B-I4-R3: the backend never stops it), by the event that really
/// ends it (P2-V1-R3B-I4-Q3-R1): the cgroup-empty notification when its
/// cgroup ran empty after being populated, otherwise only its runtime
/// backstop (a cgroup never populated produces no empty notification).
fn unloaded(world: &World) {
    let mut state = world.lock();
    let unit = state.requested().unwrap();
    state.end_by_itself_and_collect(&unit);
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
        // later open succeeds).
        let world = World::new(|state| {
            let timeout = Property::Uncertain(Reply::Timeout);
            if runtime {
                state.runtime_max = timeout;
            } else {
                state.oom_policy = timeout;
            }
            state.opens_left = Some(1);
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
    // The manager has unloaded the unit by itself (as systemd refuses a start
    // that attached no process, its process id no longer existing, and
    // collects the failed unit). P2-V1-R3B-I4-R3: nothing stops it, and while
    // the manager keeps it nothing is confirmed (i4_06). A start whose
    // process is exiting but unreaped is not refused: its scope runs, never
    // populated, until its backstop (P2-V1-R3B-I4-Q3-R1, q3r1_01).
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.unit_loaded = false;
    });
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
    // Observed gone on the issuing connection (exactly NoSuchUnit) with the
    // helper (unreaped then) outside it; nothing opened or killed.
    assert!(state.calls.contains(&Call::GetUnit(unit.clone())));
    assert!(state
        .calls
        .iter()
        .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_))));
    drop(state);
    assert!(observed_unreaped(&world) && !is_child(pid));
}

#[test]
fn i4_06_a_unit_the_helper_never_entered_that_stays_loaded_retains_the_operation() {
    // P2-V1-R3B-I4-R3: nothing acts on the unit by its name, so while the
    // manager keeps it loaded nothing confirms the operation.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::NotPlaced))
    ));
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a unit still loaded was taken as proof that nothing remained"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.issued && observed.accepted && !observed.candidate && !observed.settled);
    assert!(boundary.holds_scope() && boundary.holds_helper());
    assert!(
        is_child(pid),
        "the helper of an unresolved operation was reaped"
    );
    // The manager answering every call changes nothing; once it has
    // unloaded the unit by itself, a retry confirms it gone.
    world.release();
    let boundary = boundary.retry().unwrap_err();
    unloaded(&world);
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_07_a_candidate_still_populated_after_its_kill_confirms_nothing() {
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a candidate still populated was taken as cleanup"
    );
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).candidate);
    {
        let state = world.lock();
        let created = state.created().unwrap();
        assert!(created.killed && created.populated());
    }
    assert!(is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_08_a_target_observed_gone_confirms() {
    // With the retained candidate: what the kill left ended, and the
    // manager unloaded the unit by itself, its cgroup removed.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let boundary = boundary_of(run_in(&world, None));
    world.release();
    unloaded(&world);
    boundary.retry().unwrap();
    assert!(world.lock().created().unwrap().removed);
    // Without one: the manager answers the unit gone and the helper is
    // outside it.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.unit_loaded = false;
    });
    let report = run_in(&world, None);
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
}

#[test]
fn i4_09_a_timed_out_observation_retains_the_operation() {
    // The manager has unloaded the unit, but GetUnit times out: a timeout is
    // evidence of uncertainty, never of absence.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.unit_loaded = false;
        state.get_unit = Script::always(Reply::Timeout);
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a timed-out GetUnit dropped the operation"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.accepted && !observed.candidate && !observed.settled);
    assert!(is_child(pid));
    let boundary = boundary.retry().unwrap_err();
    assert!(is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn i4_10_an_uncertain_query_after_the_candidate_keeps_its_descriptor() {
    // GetUnit times out once the candidate is retained; no cgroup can be
    // opened again.
    let world = World::new(|state| {
        state.get_unit = Script::first([Reply::Timeout], Reply::Delivered);
        state.opens_left = Some(1);
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
    // manager does not have loaded: nothing is proven or launched. Nothing
    // binds that cgroup to the invocation the start began
    // (P2-V1-R3B-I4-R3-R1), so it is retained only as observed (when it can
    // be opened) and never ended, and the manager's absence alone confirms
    // nothing while the helper is in it.
    for v2 in [true, false] {
        let world = World::new(|state| {
            state.unit_loaded = false;
            state.cgroup_v2 = v2;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        if v2 {
            assert!(
                matches!(
                    report.not_run,
                    Some(NotRun::Scope(ScopeError::Mismatch("unit not loaded")))
                ),
                "the manager's absence of the unit did not refuse the proof: {:?}",
                report.not_run
            );
        }
        assert_eq!(report.stdout.bytes, 0, "{v2}");
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{v2}: manager absence alone confirmed an operation whose helper is in a cgroup of its unit"
        );
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.candidate == v2 && !observed.owned,
            "{v2}: {observed:?}"
        );
        {
            let state = world.lock();
            assert!(
                state.cgroups[0].kills == 0 && !state.cgroups[0].killed,
                "{v2}: a cgroup not bound to the start's invocation was ended"
            );
        }
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
        assert!(!is_child(pid), "{v2}");
        assert_eq!(
            world.lock().count(|call| matches!(call, Call::Kill(_))),
            0,
            "{v2}"
        );
    }
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
    // The helper is in a cgroup that is not the unit's. No candidate, and
    // the manager keeps the unit: nothing acts on it by its name
    // (P2-V1-R3B-I4-R3), so the operation stays owned until the manager has
    // unloaded it by itself.
    for elsewhere in ["/elsewhere/../{unit}", "/elsewhere/{unit}-not"] {
        let world = World::new(|state| state.placement = Placement::Elsewhere(elsewhere));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::NotPlaced))),
            "{elsewhere}: {:?}",
            report.not_run
        );
        assert_eq!(opens(&world), 0, "{elsewhere}");
        let boundary = boundary_of(report);
        assert!(is_child(pid), "{elsewhere}");
        unloaded(&world);
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{elsewhere}");
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
    // Nor acted upon by it (P2-V1-R3B-I4-R3): that cgroup is never killed,
    // and the operation stays owned while the manager has the unit.
    let pid = requested_pid(&world);
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    assert_eq!(world.lock().created().unwrap().kills, 0);
    let boundary = boundary_of(report);
    world.release();
    unloaded(&world);
    boundary.retry().unwrap();
    assert!(!is_child(pid));
    assert_eq!(opens(&world), 0, "a cgroup was opened by the unit's name");
}

#[test]
fn i4_17_a_name_collision_fails_closed_and_never_touches_the_other_unit() {
    let world = World::new(|state| state.collide = true);
    let report = run_in(&world, None);
    assert!(
        world.lock().units.values().any(|unit| !unit.ours),
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
    world.lock().membership.remove(&pid);
    boundary.retry().unwrap();
    let state = world.lock();
    assert!(state.units.values().any(|unit| !unit.ours));
    assert!(state
        .cgroups
        .iter()
        .all(|cgroup| cgroup.kills == 0 && !cgroup.killed));
}

#[test]
fn i4_18_no_launch_message_reaches_the_helper_before_the_scope_is_proven() {
    let unproven: [fn(&mut crate::scope::tests::State); 6] = [
        |state| state.runtime_max = Property::Uncertain(Reply::Timeout),
        |state| state.oom_policy = Property::Uncertain(Reply::Disconnect),
        |state| state.get_unit = Script::first([Reply::Malformed], Reply::Delivered),
        |state| state.limit_mismatch = Some("cpu.max"),
        |state| state.listed = false,
        // Never placed, and the manager has unloaded the unit by itself
        // (P2-V1-R3B-I4-R3: while it keeps it, nothing is confirmed).
        |state| {
            state.placement = Placement::Never;
            state.unit_loaded = false;
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
    // The helper is in a cgroup of the unit's name that the manager does not
    // have loaded: never proven or launched, and (P2-V1-R3B-I4-R3-R1) never
    // bound, so never ended: retained (`i4_11`).
    let world = World::new(|state| state.unit_loaded = false);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(report.stdout.bytes, 0, "a launch reached an unbound cgroup");
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Mismatch("unit not loaded")))
    ));
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    abandon(boundary_of(report), pid);
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
    assert_eq!(state.count(|call| matches!(call, Call::GetUnit(_))), 1);
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
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    // The unit still loaded: the retry keeps it.
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && is_child(pid));
    unloaded(&world);
    boundary.retry().unwrap();
    let state = world.lock();
    assert!(!state.units.contains_key(&state.requested().unwrap()));
    drop(state);
    assert!(!is_child(pid));
}

#[test]
fn i4_23_a_retained_boundary_is_retried_without_the_scope_manager() {
    let world = World::new(|state| state.placement = Placement::Never);
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
    unloaded(&world);
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
    // The reply was delivered, but the panic came before it (and the
    // identity captured with it) was recorded, so for the backend the
    // request's outcome is uncertain (I4-R1). P2-V1-R3B-I4-R3-R1: the cgroup
    // the helper is in is never opened or ended; the operation stays owned
    // with its helper, through a retry once the manager answers again.
    let fault = Some(Fault::Panic(FaultPoint::AfterScopeStart));
    for placement in [Placement::Immediate, Placement::Never] {
        let world = World::new(|state| state.placement = placement);
        let report = run_in(&world, fault);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Interrupted)),
            "{placement:?}"
        );
        assert_eq!(report.stdout.bytes, 0, "{placement:?}");
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{placement:?}: a panic after StartTransientUnit lost the operation's owner"
        );
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.issued && !observed.accepted && !observed.instance && !observed.candidate,
            "{placement:?}: {observed:?}"
        );
        assert!(is_child(pid), "{placement:?}");
        world.release();
        let boundary = boundary.retry().unwrap_err();
        assert_eq!(
            world
                .lock()
                .count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
            0,
            "{placement:?}: a panicked start's cgroup was opened or ended"
        );
        abandon(boundary, pid);
    }
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
fn i4_27_a_panic_while_reconciling_retains_the_operation() {
    // Without a candidate (the unit loaded), and with one that stays
    // populated.
    let worlds: [fn(&mut crate::scope::tests::State); 2] = [
        |state| state.placement = Placement::Never,
        |state| {
            failing_proof(state);
            state.phantom = Phantom::Forever;
        },
    ];
    for (case, script) in worlds.into_iter().enumerate() {
        let world = World::new(script);
        let report = run_in(&world, Some(Fault::Panic(FaultPoint::ScopeReconcile)));
        let pid = requested_pid(&world);
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "case {case}");
        let boundary = boundary_of(report);
        assert!(pending_state(&boundary).issued, "case {case}");
        assert!(boundary.holds_helper() && is_child(pid), "case {case}");
        world.release();
        unloaded(&world);
        boundary.retry().unwrap();
        assert!(!is_child(pid), "case {case}");
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
    // placed) observed unloaded by the manager itself (P2-V1-R3B-I4-R3:
    // nothing stops it).
    for script in [
        (|state: &mut crate::scope::tests::State| state.limit_mismatch = Some("pids.max"))
            as fn(&mut crate::scope::tests::State),
        |state| {
            state.placement = Placement::Never;
            state.unit_loaded = false;
        },
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
    // That process ends and the manager unloads the unit by itself
    // (P2-V1-R3B-I4-R3: nothing stops it): only then is the operation
    // confirmed, the cgroup still never opened or killed by the backend.
    world.release();
    unloaded(&world);
    boundary.retry().unwrap();
    assert_eq!(opens(&world), 0);
    assert_eq!(world.lock().created().unwrap().kills, 0);
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
    for point in [FaultPoint::AfterScopeStart, FaultPoint::ScopeReconcile] {
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
    // of the operation and its helper. P2-V1-R3B-I4-R2, -R3: nothing is
    // acted upon by the unit's name: it stays loaded.
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
        !world.lock().units.is_empty(),
        "the unit was stopped by its name"
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
    let world = World::new(|state| uncertain_without_candidate(state, true, Reply::Timeout));
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
        let pid = requested_pid(&world);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
            ),
            "{beside}: a cgroup of the unit's name that is not the unit's own was proven: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{beside}");
        // P2-V1-R3B-I4-R3-R1: the cgroup the helper was found in is not the
        // unit's own, so nothing binds it to the invocation the start began:
        // it stays observed and is never ended, and the operation stays owned
        // while the helper is in it. The unit's own cgroup is never opened by
        // its path.
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{beside}");
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.candidate && !observed.owned,
            "{beside}: {observed:?}"
        );
        let found = {
            let state = world.lock();
            let unit = state.requested().unwrap();
            let found = expand(beside, &unit);
            assert!(
                state
                    .cgroups
                    .iter()
                    .all(|cgroup| cgroup.kills == 0 && !cgroup.killed),
                "{beside}: a cgroup not bound to the start's invocation was ended"
            );
            assert_eq!(
                state
                    .calls
                    .iter()
                    .filter(|call| matches!(call, Call::Open(_)))
                    .collect::<Vec<_>>(),
                [&Call::Open(found.clone())],
                "{beside}"
            );
            found
        };
        // The unit's cgroup was never populated, so only its runtime backstop
        // ends it (P2-V1-R3B-I4-Q3-R1); the manager collects it, and the
        // cgroup the helper is in goes: the manager's absence with the helper
        // outside confirms the accepted operation; the backend ended nothing.
        {
            let mut state = world.lock();
            let unit = state.requested().unwrap();
            assert!(state.expire_backstop(&unit) && state.collect(&unit));
            let index = state
                .cgroups
                .iter()
                .position(|cgroup| cgroup.path == found)
                .unwrap();
            state.cgroups[index].removed = true;
            state.membership.insert(pid, format!("{found} (deleted)"));
        }
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{beside}");
        assert_eq!(
            world.lock().count(|call| matches!(call, Call::Kill(_))),
            0,
            "{beside}"
        );
    }
    // Through the harness's direct owner: not proven or ended either.
    let world = World::new(|state| state.placement = Placement::Beside("/other.slice/{unit}"));
    let (placed, pid) = place_cat(&world);
    let failed =
        placed.expect_err("a cgroup of the unit's name that is not the unit's own was proven");
    assert!(matches!(
        failed.error,
        Some(ScopeError::Mismatch("unit control group"))
    ));
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("a cgroup not bound to the start's invocation confirmed the operation");
    };
    assert!(boundary.holds_helper() && is_child(pid));
    assert_eq!(world.lock().count(|call| matches!(call, Call::Kill(_))), 0);
    abandon(boundary, pid);
}

#[test]
fn i4r1_x_a_membership_path_not_in_normal_form_never_locates_a_candidate() {
    // The manager's unit really is at these paths and would report them as
    // its control group; the kernel names them for the helper. A path that
    // is not absolute and in normal form locates nothing.
    for path in ["{slice}/./{unit}", "{slice}//{unit}", "{unit}"] {
        let world = World::new(|state| state.placement = Placement::Elsewhere(path));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::NotPlaced))),
            "{path}: a membership path not in normal form located the scope: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{path}");
        assert_eq!(opens(&world), 0, "{path}");
        // No candidate, and the manager keeps the unit: owned until it has
        // unloaded the unit by itself (P2-V1-R3B-I4-R3).
        let boundary = boundary_of(report);
        unloaded(&world);
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{path}");
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
        let pid = requested_pid(&world);
        assert!(
            matches!(
                report.not_run,
                Some(NotRun::Scope(ScopeError::Mismatch("unit control group")))
            ),
            "{group:?}: a control group that is not exactly the kernel's was accepted: {:?}",
            report.not_run
        );
        assert!(!launched(&report), "{group:?}");
        // P2-V1-R3B-I4-R3-R1: not bound, so never ended; the operation stays
        // owned with its helper.
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{group:?}");
        assert_eq!(world.lock().created().unwrap().kills, 0, "{group:?}");
        let boundary = boundary_of(report);
        assert!(boundary.holds_helper() && is_child(pid), "{group:?}");
        // Once the manager reports exactly the kernel's cgroup, the retained
        // candidate is bound, ended and confirmed through its descriptor.
        world.lock().control_group = Text::Exact;
        boundary.retry().unwrap();
        assert!(world.lock().created().unwrap().killed, "{group:?}");
        assert!(!is_child(pid), "{group:?}");
        assert_eq!(opens(&world), 1, "{group:?}");
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
        let pid = requested_pid(&world);
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
        // P2-V1-R3B-I4-R3-R1: not bound, so never ended; once the manager
        // answers exactly, the candidate is bound, ended and confirmed.
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{group:?}");
        assert_eq!(world.lock().created().unwrap().kills, 0, "{group:?}");
        let boundary = boundary_of(report);
        world.release();
        world.lock().control_group = Text::Exact;
        boundary.retry().unwrap();
        assert!(world.lock().created().unwrap().killed, "{group:?}");
        assert!(!is_child(pid), "{group:?}");
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
        let pid = requested_pid(&world);
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
        // Refused before the control group or any property is asked for.
        assert_eq!(
            world.lock().count(|call| matches!(
                call,
                Call::ControlGroup(_)
                    | Call::ControlGroupId(_)
                    | Call::RuntimeMax(_)
                    | Call::OomPolicy(_)
            )),
            0,
            "{id:?}"
        );
        // P2-V1-R3B-I4-R3-R1: not bound, so never ended; once the manager
        // answers exactly, the candidate is bound, ended and confirmed.
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{id:?}");
        assert_eq!(world.lock().created().unwrap().kills, 0, "{id:?}");
        let boundary = boundary_of(report);
        world.lock().unit_id = Text::Exact;
        boundary.retry().unwrap();
        assert!(world.lock().created().unwrap().killed, "{id:?}");
        assert!(!is_child(pid), "{id:?}");
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
        assert_eq!(
            state.calls,
            proof_calls(&unit, pid),
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

/// The calls of a proof that passes (P2-V1-R3B-I4-R3-R1): the kernel's view
/// of the helper, the candidate retained, the helper inside it again, the
/// manager's unit bound by its invocation (read before and after its `Id`,
/// control group and the directory's ID), the policy, and the invocation
/// once more.
fn proof_calls(unit: &str, pid: u32) -> Vec<Call> {
    let object = format!("/unit/{unit}");
    vec![
        Call::Start(unit.to_string(), pid),
        Call::Membership(pid, true),
        Call::Open(format!("{SLICE}/{unit}")),
        Call::Membership(pid, true),
        Call::GetUnit(unit.to_string()),
        Call::UnitInstance(object.clone()),
        Call::UnitId(object.clone()),
        Call::ControlGroup(object.clone()),
        Call::ControlGroupId(object.clone()),
        Call::UnitInstance(object.clone()),
        Call::RuntimeMax(object.clone()),
        Call::OomPolicy(object.clone()),
        Call::UnitInstance(object),
    ]
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
fn i4r1_12_an_uncertain_start_without_a_candidate_is_not_released_once_its_unit_is_unloaded() {
    // The manager created the unit, the reply was lost, and the kernel never
    // reported the helper in it. Nothing acts on the unit by its name
    // (P2-V1-R3B-I4-R2, -R3): it stays loaded, and the operation stays owned
    // with its helper. Once the manager has unloaded it by itself
    // (NoSuchUnit, the helper outside), still nothing confirms an uncertain
    // start.
    let world = World::new(|state| uncertain_without_candidate(state, true, Reply::Timeout));
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(
        !world.lock().units.is_empty(),
        "the unit was stopped by its name"
    );
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an uncertain start without a candidate was released"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(!observed.accepted && !observed.settled && !observed.candidate);
    assert!(boundary.holds_helper() && is_child(pid));
    unloaded(&world);
    let Err(boundary) = boundary.retry() else {
        panic!("an uncertain start without a candidate was released once its unit was unloaded");
    };
    assert!(boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

#[test]
fn i4r1_13_an_uncertain_start_without_a_candidate_keeps_its_helper_unreaped() {
    for effect in [true, false] {
        let world = World::new(|state| uncertain_without_candidate(state, effect, Reply::Timeout));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{effect}: an uncertain start without a candidate was released"
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
fn i4r1_14_an_uncertain_start_that_places_the_helper_is_never_acquired_proven_or_ended() {
    // P2-V1-R3B-I4-R3-R1 (reversing I4-R1's acquisition): the reply was lost,
    // and the request's job places the helper in the unit's cgroup while the
    // proof would wait, or only later. Nothing identifies the invocation the
    // request may have started, so that cgroup is never opened, proven,
    // launched into or ended, and nothing confirms the operation: it stays
    // owned with its helper unreaped.
    for reply in [Reply::Timeout, Reply::Error, Reply::Malformed] {
        let case = format!("{reply:?}");
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = Placement::AfterReads(3);
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            !launched(&report),
            "{case}: an uncertain start was launched into"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{case}: {:?}",
            report.not_run
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        // The start job places the helper now.
        world.lock().place(pid, 0);
        world.release();
        let boundary = retained_through(boundary, 2, &case);
        let observed = pending_state(&boundary);
        assert!(
            !observed.accepted && !observed.instance && !observed.candidate && !observed.owned,
            "{case}: {observed:?}"
        );
        let state = world.lock();
        assert_eq!(
            state.count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
            0,
            "{case}: an uncertain start's cgroup was opened or ended"
        );
        assert!(state.cgroups[0].kills == 0 && !state.cgroups[0].killed);
        drop(state);
        assert!(boundary.holds_helper() && is_child(pid), "{case}");
        abandon(boundary, pid);
    }
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
            let Cleanup::Failed(boundary) = report.cleanup else {
                panic!("a collision was confirmed while the foreign unit holds the helper");
            };
            assert!(pending_state(&boundary).collided);
            let Err(boundary) = boundary.retry() else {
                panic!("a collision was confirmed while the foreign unit holds the helper");
            };
            world.lock().membership.remove(&pid);
            boundary.retry().unwrap();
        } else {
            assert!(report.cleanup.is_confirmed());
        }
        let state = world.lock();
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
        let pid = requested_pid(&world);
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
        // P2-V1-R3B-I4-R3-R1: the candidate is ended only once bound. While
        // the binding fails it stays observed, the operation owned; once the
        // manager answers exactly (after a panic inside the model's read, at
        // once) it is bound, ended and confirmed through its descriptor.
        match report.cleanup {
            Cleanup::Confirmed => assert_eq!(case, 7, "case {case}: confirmed unbound"),
            Cleanup::Failed(boundary) => {
                assert_ne!(case, 7, "case {case}");
                let observed = pending_state(&boundary);
                assert!(
                    observed.candidate && !observed.owned,
                    "case {case}: {observed:?}"
                );
                assert_eq!(
                    world.lock().created().unwrap().kills,
                    0,
                    "case {case}: a candidate was ended before it was bound"
                );
                world.release();
                {
                    let mut state = world.lock();
                    state.unit_id = Text::Exact;
                    state.control_group = Text::Exact;
                }
                boundary.retry().unwrap();
            }
        }
        assert!(world.lock().created().unwrap().killed, "case {case}");
        assert!(!is_child(pid), "case {case}");
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
    // The proof, the binding included; then finalization through the
    // retained descriptor only.
    let proof = proof_calls(&unit, pid);
    assert_eq!(state.calls[..proof.len()], proof);
    assert!(state.calls[proof.len()..]
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
    let observed = pending_state(&boundary);
    // P2-V1-R3B-I4-R3-R1: retained, observed only: never ended unbound.
    assert!(observed.candidate && !observed.owned, "{observed:?}");
    assert_eq!(world.lock().created().unwrap().kills, 0);
    drop(scopes);
    world.release();
    {
        let mut state = world.lock();
        state.opens_left = Some(0);
        state.control_group = Text::Exact;
    }
    // Bound on the issuing connection by the retained descriptor (never
    // opened again), then ended and confirmed through it.
    assert!(
        boundary.retry().is_ok(),
        "a retained boundary needs the ScopeManager that started it"
    );
    assert!(!is_child(pid));
    assert!(world.lock().created().unwrap().killed);
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
    assert_eq!(
        world.lock().count(|call| matches!(call, Call::GetUnit(_))),
        1
    );
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
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    unloaded(&world);
    world.lock().panic_at = Some(Op::GetUnit);
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
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

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R2: an uncertain scope operation (no recorded start reply)
// may as well have been refused as already loaded, that answer lost: it is
// ended only through its retained candidate, by descriptor, or stays owned
// with its helper unreaped. P2-V1-R3B-I4-R3 extends this to every
// operation: nothing is ever acted upon by a unit name (`i4r3_*`). Over the
// deterministic simulation only.

/// A process of a foreign unit (a model process id: the model never signals
/// a real process).
const FOREIGN: u32 = 4_000_001;

/// A foreign unit already loaded under the generated name, holding a
/// process of its own: the request is refused as exactly `UnitExists`,
/// without effect, and that reply reaches the backend as `reply`.
fn foreign_collision(state: &mut State, reply: Reply) {
    state.collide = true;
    state.collision_reply = reply;
    state.foreign_member = Some(FOREIGN);
}

/// The foreign unit is untouched: never stopped by its name, still loaded
/// and foreign, its cgroup never opened or killed, its process still in it.
fn foreign_untouched(world: &World, case: &str) {
    let state = world.lock();
    let unit = state.requested().unwrap();
    let foreign = state
        .units
        .get(&unit)
        .unwrap_or_else(|| panic!("{case}: the foreign unit was stopped by its name"));
    assert!(!foreign.ours, "{case}");
    let cgroup = &state.cgroups[foreign.cgroup.expect("the foreign unit's cgroup")];
    assert!(
        !cgroup.killed && cgroup.kills == 0 && !cgroup.removed && cgroup.members.contains(&FOREIGN),
        "{case}: the foreign unit was killed"
    );
    assert!(
        state.calls.iter().all(|call| !matches!(
            call,
            Call::Open(_) | Call::Kill(_) | Call::UnitId(_) | Call::ControlGroup(_)
        )),
        "{case}: the foreign unit was claimed"
    );
}

/// Retry `boundary` `attempts` times; each must keep it.
fn retained_through(
    mut boundary: RetainedBoundary,
    attempts: usize,
    case: &str,
) -> RetainedBoundary {
    for attempt in 1..=attempts {
        boundary = match boundary.retry() {
            Ok(()) => panic!(
                "{case}: an uncertain start was resolved by its unit name or presence (retry {attempt})"
            ),
            Err(boundary) => boundary,
        };
    }
    boundary
}

#[test]
fn i4r2_01_a_lost_collision_reply_never_stops_the_foreign_unit() {
    // A foreign unit already holds the generated name: the request is refused
    // as `UnitExists`, without effect, but that reply is lost, so the backend
    // sees an uncertain start, never a collision. The helper is never placed
    // (no candidate). Nothing grants a stop by the name: the foreign unit is
    // untouched, and the operation stays owned with its helper unreaped
    // (`CleanupFailed`), through retries once the manager answers again.
    for reply in [Reply::Timeout, Reply::Disconnect, Reply::Malformed] {
        let case = format!("{reply:?}");
        let world = World::new(|state| foreign_collision(state, reply));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        foreign_untouched(&world, &case);
        assert!(!launched(&report), "{case}");
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{case}: an uncertain start was resolved by its unit name or presence"
        );
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.issued
                && !observed.accepted
                && !observed.collided
                && !observed.candidate
                && !observed.settled,
            "{case}: {observed:?}"
        );
        assert!(
            boundary.holds_helper() && is_child(pid),
            "{case}: the helper of an unresolved uncertain start was reaped"
        );
        world.release();
        let boundary = retained_through(boundary, 2, &case);
        foreign_untouched(&world, &case);
        assert!(boundary.holds_helper() && is_child(pid) && observed_unreaped(&world));
        abandon(boundary, pid);
    }
    // Through the live harness's direct owner (the qualification's H6).
    let world = World::new(|state| foreign_collision(state, Reply::Timeout));
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("a lost collision was proven");
    foreign_untouched(&world, "place");
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("place: an uncertain start was resolved by its unit name or presence");
    };
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

#[test]
fn i4r2_02_an_uncertain_start_that_did_nothing_is_never_stopped_by_name() {
    // No effect, the reply lost (a timeout, a broken connection, an unexpected
    // error, a reply that does not decode): nothing can show what happened
    // and nothing grants a stop by the name. After bounded retries the
    // operation is still owned with its helper unreaped; neither the
    // manager's absence nor the helper outside confirms it.
    for reply in [
        Reply::Timeout,
        Reply::Disconnect,
        Reply::Error,
        Reply::Malformed,
    ] {
        let case = format!("{reply:?}");
        let world = World::new(|state| uncertain_without_candidate(state, false, reply));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert_eq!(
            report.classify(0),
            ExitClass::CleanupFailed,
            "{case}: an uncertain start was resolved by its unit name or presence"
        );
        world.release();
        let boundary = retained_through(boundary_of(report), 3, &case);
        let observed = pending_state(&boundary);
        assert!(
            observed.issued && !observed.accepted && !observed.settled,
            "{case}: {observed:?}"
        );
        assert!(
            boundary.holds_scope() && boundary.holds_helper() && is_child(pid),
            "{case}: the helper of an unresolved uncertain start was reaped"
        );
        assert!(
            observed_unreaped(&world),
            "{case}: the helper of an unresolved uncertain start was reaped"
        );
        abandon(boundary, pid);
    }
}

#[test]
fn i4r2_03_an_uncertain_start_whose_helper_appears_later_is_never_ended_through_its_cgroup() {
    // The request created the unit, its reply was lost, and the job has not
    // attached the helper yet; later the kernel reports the helper in the
    // unit's cgroup. P2-V1-R3B-I4-R3-R1 (reversing R2's acquisition by the
    // helper's membership): nothing identifies the invocation the request may
    // have started, so that cgroup is never opened or ended, by descriptor or
    // by the name, and the operation stays owned with its helper unreaped.
    let world = World::new(|state| uncertain_without_candidate(state, true, Reply::Timeout));
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(
        !world.lock().units.is_empty(),
        "an uncertain start was stopped by its name"
    );
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an uncertain start was resolved by its unit name or presence"
    );
    let boundary = boundary_of(report);
    assert!(!pending_state(&boundary).candidate);
    assert!(
        boundary.holds_helper() && is_child(pid),
        "the helper of an unresolved uncertain start was reaped"
    );
    // The start job attaches the helper now.
    {
        let mut state = world.lock();
        let unit = state.requested().unwrap();
        assert!(state.units.contains_key(&unit));
        state.place(pid, 0);
    }
    let boundary = retained_through(boundary, 2, "the helper placed later");
    assert!(boundary.holds_helper() && is_child(pid));
    assert!(!pending_state(&boundary).candidate);
    let state = world.lock();
    assert!(
        !state.units.is_empty(),
        "an uncertain start was stopped by its name"
    );
    assert_eq!(
        state.count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
        0,
        "an uncertain start's cgroup was opened or ended"
    );
    assert!(state.cgroups[0].kills == 0 && !state.cgroups[0].killed);
    drop(state);
    assert!(observed_unreaped(&world));
    abandon(boundary, pid);
}

#[test]
fn i4r2_04_an_uncertain_start_s_cgroup_is_never_ended_by_descriptor_or_by_name() {
    // The reply is lost, but the job attaches the helper while the proof
    // would wait. P2-V1-R3B-I4-R3-R1 (reversing R2's candidate cleanup):
    // nothing identifies the invocation the request may have started, so the
    // cgroup the helper is in is never opened, bound, launched into or ended
    // through a descriptor, and nothing is stopped by the name: the operation
    // stays owned, `CleanupFailed`, through retries once the manager answers
    // again, whether or not something else stays in that cgroup.
    for phantom in [Phantom::Forever, Phantom::None] {
        let case = format!("{phantom:?}");
        let world = World::new(|state| {
            state.start_reply = Reply::Timeout;
            state.phantom = phantom;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{case}: {:?}",
            report.not_run
        );
        assert!(
            !launched(&report),
            "{case}: a launch reached an unproven scope"
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            !observed.candidate && !observed.accepted && !observed.settled,
            "{case}: {observed:?}"
        );
        world.release();
        let boundary = retained_through(boundary, 2, &case);
        let state = world.lock();
        assert!(
            !state.units.is_empty(),
            "{case}: an uncertain start's unit was stopped by its name"
        );
        assert!(
            state
                .calls
                .iter()
                .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_))),
            "{case}: an uncertain start's cgroup was opened or ended"
        );
        let created = state.created().unwrap();
        assert!(
            created.kills == 0 && !created.killed && created.members.contains(&pid),
            "{case}"
        );
        drop(state);
        abandon(boundary, pid);
    }
}

#[test]
fn i4r2_05_a_recorded_start_reply_authorizes_no_stop_by_name() {
    // P2-V1-R3B-I4-R3: a delivered StartTransientUnit reply, recorded, shows
    // that the manager created the unit for this request; it never
    // authorizes acting on a unit by its name. The helper never entered it
    // (no candidate) and the manager keeps it loaded: with the reply
    // recorded or lost, the unit is untouched and the operation stays owned
    // with its helper unreaped, whatever the manager answers.
    for reply in [Reply::Delivered, Reply::Timeout] {
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = Placement::Never;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{reply:?}");
        let boundary = boundary_of(report);
        assert_eq!(
            pending_state(&boundary).accepted,
            reply == Reply::Delivered,
            "{reply:?}"
        );
        world.release();
        let Err(boundary) = boundary.retry() else {
            panic!("{reply:?}: a loaded unit was taken as gone");
        };
        assert!(boundary.holds_helper() && is_child(pid), "{reply:?}");
        let state = world.lock();
        let unit = state.requested().unwrap();
        assert!(
            state.units.get(&unit).is_some_and(|loaded| loaded.ours),
            "{reply:?}: the unit was stopped by its name"
        );
        assert!(
            state
                .calls
                .iter()
                .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_))),
            "{reply:?}: the unit was acted upon by its name"
        );
        drop(state);
        abandon(boundary, pid);
    }
}

#[test]
fn i4r2_06_a_definite_collision_is_never_stopped_or_claimed() {
    // Exactly `UnitExists`, delivered (I4-R1, unchanged): the foreign unit,
    // holding its own process, is never stopped, killed, opened or claimed;
    // the helper, never placed, is outside it, so the collided operation is
    // confirmed and the helper reaped. The same through the direct owner.
    let world = World::new(|state| foreign_collision(state, Reply::Delivered));
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    foreign_untouched(&world, "execution");
    assert!(matches!(
        report.not_run,
        Some(NotRun::Scope(ScopeError::Bus(_)))
    ));
    assert!(report.cleanup.is_confirmed() && !is_child(pid));
    assert_eq!(
        world.lock().count(|call| matches!(call, Call::GetUnit(_))),
        0,
        "the foreign unit was claimed"
    );
    let world = World::new(|state| foreign_collision(state, Reply::Delivered));
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("a collision was proven");
    assert!(failed.cleanup.is_confirmed() && !is_child(pid));
    foreign_untouched(&world, "place");
}

#[test]
fn i4r2_07_a_panic_before_the_start_reply_is_recorded_grants_no_name_authority() {
    // The success reply arrives, but a panic comes before it is recorded: for
    // the backend the outcome is uncertain, so it never stops the unit by
    // name; the operation and its helper stay owned together, unreaped, and
    // no panic crosses the owner.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, Some(Fault::Panic(FaultPoint::AfterScopeStart)));
    let pid = requested_pid(&world);
    assert!(
        matches!(report.not_run, Some(NotRun::Interrupted)),
        "{:?}",
        report.not_run
    );
    assert!(
        !world.lock().units.is_empty(),
        "a stop by name was asked without a recorded start reply"
    );
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an uncertain start was resolved by its unit name or presence"
    );
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(observed.issued && !observed.accepted);
    assert!(boundary.holds_helper() && is_child(pid));
    // P2-V1-R3B-I4-R3: the outcome stays uncertain, so even the manager's
    // later absence of the unit (unloaded by itself, the helper outside)
    // confirms nothing.
    world.release();
    unloaded(&world);
    let boundary = retained_through(boundary, 1, "a panic before the reply was recorded");
    abandon(boundary, pid);
    // A panic inside StartTransientUnit itself, after its effect and before
    // any reply, through the harness's direct owner: no panic crosses it; the
    // unit is never stopped by name; the helper stays owned, unreaped.
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
        !world.lock().units.is_empty(),
        "the unit was stopped by its name"
    );
    abandon(boundary, pid);
}

#[test]
fn i4r2_08_a_retained_uncertain_operation_never_gains_name_authority_without_its_scope_manager() {
    // The unit created, its reply lost, the helper never placed; the
    // ScopeManager that started it is gone. Retried on the issuing
    // connection, with the manager answering again, the retained operation
    // still never stops its unit by name and stays owned with its helper.
    let world = World::new(|state| uncertain_without_candidate(state, true, Reply::Timeout));
    let stand_in = StandIn::new();
    let scopes = world.scopes();
    let report = execute(&scopes, &stand_in.program(), launch_spec(), &LIMITS, None);
    let pid = requested_pid(&world);
    drop(scopes);
    world.release();
    let boundary = retained_through(boundary_of(report), 3, "without its ScopeManager");
    {
        let state = world.lock();
        assert!(state.units.contains_key(&state.requested().unwrap()));
    }
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3: no scope operation acts on a unit by its name. A unit name
// is a locator and evidence, never a retained identity: once this request's
// unit is unloaded, another client can load a unit of its own under the same
// name. What an operation may have created is ended only through its
// retained candidate, by descriptor; without one, an accepted operation is
// confirmed only once the manager answers the unit gone with the helper
// outside, and stays owned (`CleanupFailed`, its helper unreaped) while a
// unit of the name is loaded. Over the deterministic simulation only.

/// A process of the foreign unit loaded under a reused name (a model process
/// id: the model never signals a real process).
const REUSER: u32 = 4_000_002;

/// This request's unit ends by itself and is collected (see [`unloaded`]),
/// and another client loads a unit of its own under the same name, holding
/// `REUSER`: the foreign cgroup's index.
fn reloaded_by_another(world: &World) -> usize {
    let mut state = world.lock();
    let unit = state.requested().unwrap();
    assert!(state.end_by_itself_and_collect(&unit));
    state.load_foreign(&unit, REUSER)
}

/// The foreign unit loaded under the reused name (its cgroup at `index`) is
/// untouched by the calls since `since`: nothing of it opened or read, its
/// cgroup never killed, the unit still loaded with its process in it.
fn reuser_untouched(world: &World, index: usize, since: usize, case: &str) {
    let state = world.lock();
    let unit = state.requested().unwrap();
    assert!(
        state.calls[since..].iter().all(|call| !matches!(
            call,
            Call::Open(_)
                | Call::UnitId(_)
                | Call::ControlGroup(_)
                | Call::RuntimeMax(_)
                | Call::OomPolicy(_)
        )),
        "{case}: the foreign unit loaded under the name was claimed: {:?}",
        &state.calls[since..]
    );
    let cgroup = &state.cgroups[index];
    assert!(
        !cgroup.killed && cgroup.kills == 0 && !cgroup.removed && cgroup.members.contains(&REUSER),
        "{case}: the foreign unit loaded under the name was killed"
    );
    assert!(
        state
            .units
            .get(&unit)
            .is_some_and(|loaded| !loaded.ours && loaded.cgroup == Some(index)),
        "{case}: the foreign unit loaded under the name was stopped"
    );
}

#[test]
fn i4r3_01_an_accepted_operation_never_acts_on_a_unit_loaded_under_its_name_again() {
    // The start's reply was delivered and recorded: the manager created the
    // unit for this request. The helper never entered it (no candidate).
    // Then that unit is unloaded and another client loads a unit of its own
    // under the same name, with a process of its own. Nothing is acted upon
    // by the name: the foreign unit is only observed (GetUnit), never
    // stopped, killed, opened or claimed, and the operation stays owned,
    // `CleanupFailed`, its helper unreaped, through every retry.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an accepted operation was confirmed while a unit of its name is loaded"
    );
    let mut boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(
        observed.accepted && !observed.candidate && !observed.settled,
        "{observed:?}"
    );
    let since = world.lock().calls.len();
    let foreign = reloaded_by_another(&world);
    for retry in 1..=3 {
        boundary = match boundary.retry() {
            Ok(()) => panic!(
                "an accepted operation was confirmed while a unit of its name is loaded (retry {retry})"
            ),
            Err(boundary) => boundary,
        };
        reuser_untouched(&world, foreign, since, &format!("retry {retry}"));
        assert!(
            boundary.holds_scope() && boundary.holds_helper() && is_child(pid),
            "retry {retry}: the helper of an unresolved operation was reaped"
        );
    }
    {
        // Only observed: the manager's view of the name, and the helper's
        // position.
        let state = world.lock();
        assert!(
            state.calls[since..]
                .iter()
                .all(|call| matches!(call, Call::GetUnit(_) | Call::Membership(..))),
            "the foreign unit loaded under the name was acted upon: {:?}",
            &state.calls[since..]
        );
        assert!(state.calls[since..].contains(&Call::GetUnit(state.requested().unwrap())));
    }
    assert!(observed_unreaped(&world));
    abandon(boundary, pid);
    // Through the live harness's direct owner: the same.
    let world = World::new(|state| state.placement = Placement::Never);
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("a unit the helper never entered was proven");
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("an accepted operation was confirmed while a unit of its name is loaded");
    };
    let since = world.lock().calls.len();
    let foreign = reloaded_by_another(&world);
    let Err(boundary) = boundary.retry() else {
        panic!("an accepted operation was confirmed while a unit of its name is loaded");
    };
    reuser_untouched(&world, foreign, since, "place");
    assert!(boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

#[test]
fn i4r3_02_an_accepted_operation_whose_unit_stays_loaded_stays_owned_and_bounded() {
    // Accepted, the helper never placed, and the manager keeps the unit
    // loaded: no candidate and no absence, so nothing confirms the
    // operation. Each settling attempt ends within its bound and only
    // observes: the unit is never stopped, killed or opened, and the helper
    // stays unreaped, whatever the manager answers.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "an accepted operation was confirmed while its unit is loaded"
    );
    let mut boundary = boundary_of(report);
    world.release();
    for retry in 1..=3 {
        let started = Instant::now();
        boundary = match boundary.retry() {
            Ok(()) => panic!(
                "an accepted operation was confirmed while its unit is loaded (retry {retry})"
            ),
            Err(boundary) => boundary,
        };
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "retry {retry}: settling was not bounded"
        );
        assert!(
            boundary.holds_scope() && boundary.holds_helper() && is_child(pid),
            "retry {retry}: the helper of an accepted operation was reaped while its unit is loaded"
        );
    }
    let observed = pending_state(&boundary);
    assert!(
        observed.accepted && !observed.candidate && !observed.settled,
        "{observed:?}"
    );
    {
        let state = world.lock();
        let unit = state.requested().unwrap();
        assert!(
            state.units.get(&unit).is_some_and(|loaded| loaded.ours),
            "the unit was stopped by its name"
        );
        assert!(
            state.calls.iter().all(|call| matches!(
                call,
                Call::Start(..) | Call::Membership(..) | Call::GetUnit(_)
            )),
            "the unit was acted upon by its name: {:?}",
            state.calls
        );
        assert!(state
            .cgroups
            .iter()
            .all(|cgroup| cgroup.kills == 0 && !cgroup.killed));
    }
    assert!(
        observed_unreaped(&world),
        "the helper of an accepted operation was reaped while its unit is loaded"
    );
    abandon(boundary, pid);
}

#[test]
fn i4r3_03_an_accepted_operation_is_confirmed_once_the_manager_has_unloaded_its_unit() {
    // Owned while the manager keeps the unit. Once it has unloaded the unit
    // by itself, GetUnit answers exactly NoSuchUnit on the issuing
    // connection and the kernel then reports the helper outside any cgroup
    // of the name: the operation is confirmed, and only then is the helper
    // reaped. Nothing was stopped, killed or opened.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    let boundary = boundary_of(report);
    let since = world.lock().calls.len();
    unloaded(&world);
    assert!(
        boundary.retry().is_ok(),
        "an accepted operation was not confirmed once its unit was unloaded"
    );
    assert!(!is_child(pid));
    assert!(
        observed_unreaped(&world),
        "the helper was reaped before the operation was confirmed"
    );
    {
        let state = world.lock();
        let unit = state.requested().unwrap();
        let after = &state.calls[since..];
        let asked = after
            .iter()
            .rposition(|call| *call == Call::GetUnit(unit.clone()))
            .expect("the manager's absence was asked for");
        assert_eq!(after[asked + 1..], [Call::Membership(pid, true)]);
        assert!(state
            .calls
            .iter()
            .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_))));
    }
    // Already unloaded when the first attempt settles: confirmed at once.
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.unit_loaded = false;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    assert!(observed_unreaped(&world) && !is_child(pid));
}

#[test]
fn i4r3_04_an_accepted_operation_s_candidate_is_ended_by_descriptor_and_observed() {
    // Accepted, the helper placed (its cgroup retained by descriptor as the
    // candidate), then a proof fails. The candidate is ended with
    // `cgroup.kill` through its descriptor and confirmed once observed empty;
    // nothing is asked of the manager after the proof, and the unit, still
    // loaded, is never acted upon by its name.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::UntilKill;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(report.cleanup.is_confirmed(), "{:?}", report.cleanup);
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    {
        let state = world.lock();
        let unit = state.requested().unwrap();
        let proof = state
            .calls
            .iter()
            .rposition(|call| matches!(call, Call::OomPolicy(_)))
            .expect("the proof");
        assert_eq!(
            state.calls[proof + 1..],
            [Call::Kill(format!("{SLICE}/{unit}"))],
            "the candidate was not ended through its descriptor alone"
        );
        assert!(state.created().unwrap().killed);
        assert!(
            state.units.get(&unit).is_some_and(|loaded| loaded.ours),
            "the unit was stopped by its name"
        );
    }
    assert!(observed_unreaped(&world) && !is_child(pid));
    // Still populated after the kill (a process the kill does not end):
    // never confirmed meanwhile; once empty, confirmed through the
    // descriptor.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert_eq!(
        report.classify(0),
        ExitClass::CleanupFailed,
        "a candidate still populated was confirmed"
    );
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).candidate && is_child(pid));
    world.release();
    boundary.retry().unwrap();
    assert!(!is_child(pid));
    assert_eq!(opens(&world), 1);
    // Removed, and the name reused: this request's unit unloaded with its
    // cgroup, and another client's unit loaded under the same name in a new
    // cgroup of the same path. The candidate is the removed cgroup, by
    // descriptor: confirmed (removed), and the foreign cgroup of that path
    // is never opened or killed.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    world.release();
    let since = world.lock().calls.len();
    let foreign = reloaded_by_another(&world);
    assert!(
        boundary.retry().is_ok(),
        "a removed candidate was not confirmed through its descriptor"
    );
    assert!(!is_child(pid));
    reuser_untouched(&world, foreign, since, "a removed candidate");
    assert_eq!(opens(&world), 1);
}

#[test]
fn i4r3_09_a_retained_operation_retried_without_its_scope_manager_never_acts_by_name() {
    // Accepted and uncertain, the helper never placed, the unit loaded; the
    // ScopeManager that started it is gone. Retried on the issuing
    // connection: each attempt is bounded and only observes, nothing is
    // acted upon by the name, and the operation stays owned with its
    // helper. Then the manager unloads the unit by itself: that confirms the
    // accepted operation, never the uncertain one.
    for reply in [Reply::Delivered, Reply::Timeout] {
        let case = format!("{reply:?}");
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = Placement::Never;
        });
        let stand_in = StandIn::new();
        let scopes = world.scopes();
        let report = execute(&scopes, &stand_in.program(), launch_spec(), &LIMITS, None);
        let pid = requested_pid(&world);
        drop(scopes);
        world.release();
        let mut boundary = boundary_of(report);
        for retry in 1..=3 {
            let started = Instant::now();
            boundary = match boundary.retry() {
                Ok(()) => panic!(
                    "{case}: an operation was confirmed while its unit is loaded (retry {retry})"
                ),
                Err(boundary) => boundary,
            };
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{case}: retry {retry} was not bounded"
            );
        }
        {
            let state = world.lock();
            let unit = state.requested().unwrap();
            assert!(
                state.units.get(&unit).is_some_and(|loaded| loaded.ours),
                "{case}: the unit was stopped by its name"
            );
            assert!(
                state.calls.iter().all(|call| matches!(
                    call,
                    Call::Start(..) | Call::Membership(..) | Call::GetUnit(_)
                )),
                "{case}: the unit was acted upon by its name: {:?}",
                state.calls
            );
        }
        assert!(
            boundary.holds_scope() && boundary.holds_helper() && is_child(pid),
            "{case}"
        );
        unloaded(&world);
        match (reply, boundary.retry()) {
            (Reply::Delivered, Ok(())) => assert!(!is_child(pid), "{case}"),
            (Reply::Delivered, Err(_)) => {
                panic!("{case}: an accepted operation was not confirmed once its unit was unloaded")
            }
            (_, Ok(())) => {
                panic!("{case}: an uncertain start was resolved by its unit name or presence")
            }
            (_, Err(boundary)) => {
                let boundary = retained_through(boundary, 1, &case);
                abandon(boundary, pid);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3-R1: candidate cleanup ownership. Observing the helper in a
// cgroup is never by itself authority over that cgroup: a retained
// candidate is only observed until it is bound to the unit invocation this
// request's start began (the identity captured from the start's own job);
// only an owned candidate is ever ended, and only a proven scope is
// launched into. Over the deterministic simulation only.

use crate::scope::tests::{Capture, Interference};

/// A process of a foreign unit whose cgroup also holds the helper (a model
/// process id: the model never signals a real process).
const HOLDER: u32 = 4_000_003;

/// What a case arranges in the model.
type Arrange = fn(&mut State);

/// Every `cgroup.kill` in the log follows a complete binding of the unit's
/// object path: GetUnit, the identity, `Id`, `ControlGroup`,
/// `ControlGroupId` and the identity again, in one run (a call that panics
/// breaks the run).
fn kills_follow_a_binding(world: &World) -> bool {
    let state = world.lock();
    let Some(unit) = state.requested() else {
        return true;
    };
    let object = format!("/unit/{unit}");
    let binding = [
        Call::GetUnit(unit.clone()),
        Call::UnitInstance(object.clone()),
        Call::UnitId(object.clone()),
        Call::ControlGroup(object.clone()),
        Call::ControlGroupId(object.clone()),
        Call::UnitInstance(object),
    ];
    state
        .calls
        .iter()
        .enumerate()
        .filter(|(_, call)| matches!(call, Call::Kill(_)))
        .all(|(at, _)| {
            state.calls[..at]
                .windows(binding.len())
                .any(|window| window == binding)
        })
}

/// The foreign unit of a lost collision holds the helper and is untouched:
/// never opened, ended, read or proven; its process and the helper still
/// in its cgroup.
fn foreign_holder_untouched(world: &World, pid: u32, case: &str) {
    foreign_untouched(world, case);
    let state = world.lock();
    let unit = state.requested().unwrap();
    let cgroup = &state.cgroups[state.units[&unit].cgroup.unwrap()];
    assert!(
        cgroup.members.contains(&pid) && state.membership.get(&pid) == Some(&cgroup.path),
        "{case}: the helper left the foreign cgroup"
    );
    assert_eq!(
        state.count(|call| matches!(
            call,
            Call::GetUnit(_)
                | Call::UnitInstance(_)
                | Call::ControlGroupId(_)
                | Call::RuntimeMax(_)
                | Call::OomPolicy(_)
        )),
        0,
        "{case}: the foreign unit was claimed"
    );
}

/// The foreign unit loaded under the name (holding `HOLDER` and, moved
/// there, the helper) is untouched: never ended or proven, and nothing of
/// it read beyond its identity; still loaded with its process.
fn replacement_untouched(world: &World, case: &str) {
    let state = world.lock();
    let unit = state.requested().unwrap();
    let replacement = state
        .units
        .get(&unit)
        .unwrap_or_else(|| panic!("{case}: the replacement was stopped by its name"));
    assert!(!replacement.ours, "{case}");
    let cgroup = &state.cgroups[replacement.cgroup.unwrap()];
    assert!(
        !cgroup.killed && cgroup.kills == 0 && !cgroup.removed && cgroup.members.contains(&HOLDER),
        "{case}: a cgroup not bound to the start's invocation was ended"
    );
    assert_eq!(
        state.count(|call| matches!(
            call,
            Call::UnitId(_)
                | Call::ControlGroup(_)
                | Call::ControlGroupId(_)
                | Call::RuntimeMax(_)
                | Call::OomPolicy(_)
        )),
        0,
        "{case}: the replacement was claimed"
    );
}

#[test]
fn r3r1_01_a_lost_collision_holding_the_helper_is_never_ended_proven_or_launched() {
    // A foreign unit already holds the generated name. Its cgroup holds a
    // process of its own and the helper, with exactly the requested limits,
    // and the manager would answer its `Id`, `ControlGroup`, runtime
    // backstop and out-of-memory policy exactly. The request is refused as
    // exactly `UnitExists`, without effect, but that answer is lost: the
    // backend sees an uncertain start. Nothing identifies an invocation of
    // this request, so that cgroup is never opened, ended, proven or
    // launched into; the operation stays owned with its helper unreaped.
    for reply in [
        Reply::Timeout,
        Reply::Disconnect,
        Reply::Malformed,
        Reply::Error,
    ] {
        let case = format!("{reply:?}");
        let world = World::new(|state| {
            foreign_collision(state, reply);
            state.collision_holds_helper = true;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            !launched(&report),
            "{case}: the foreign cgroup was launched into"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{case}: {:?}",
            report.not_run
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let mut boundary = boundary_of(report);
        world.release();
        for retry in 1..=2 {
            let retried = boundary.retry();
            foreign_holder_untouched(&world, pid, &format!("{case}, retry {retry}"));
            boundary = match retried {
                Ok(()) => panic!(
                    "{case}: confirmed while the foreign unit holds the helper (retry {retry})"
                ),
                Err(boundary) => boundary,
            };
        }
        let observed = pending_state(&boundary);
        assert!(
            observed.issued
                && !observed.accepted
                && !observed.collided
                && !observed.instance
                && !observed.candidate
                && !observed.owned
                && !observed.settled,
            "{case}: {observed:?}"
        );
        assert!(
            boundary.holds_helper() && is_child(pid) && observed_unreaped(&world),
            "{case}: the helper of an unresolved operation was reaped"
        );
        abandon(boundary, pid);
    }
    // Through the live harness's direct owner.
    let world = World::new(|state| {
        foreign_collision(state, Reply::Timeout);
        state.collision_holds_helper = true;
    });
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("the foreign cgroup was proven");
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("an uncertain start was confirmed while a foreign cgroup holds its helper");
    };
    foreign_holder_untouched(&world, pid, "place");
    assert!(boundary.holds_scope() && boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
}

#[test]
fn r3r1_02_a_same_name_replacement_holding_the_helper_is_never_ended_proven_or_launched() {
    // While the proof waits: once the start's identity is captured, the
    // manager unloads this request's unit and another client loads a unit
    // of its own under the same name, its cgroup at the same path holding a
    // process of its own and the helper. The candidate the kernel locates
    // is the replacement's cgroup: its identity is not the captured one, so
    // it is never bound, ended, proven or launched into, and nothing of it
    // is read beyond its identity.
    let world = World::new(|state| state.interference = Interference::Replace(HOLDER));
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    assert!(!launched(&report), "the replacement was launched into");
    assert!(
        matches!(
            report.not_run,
            Some(NotRun::Scope(ScopeError::Mismatch("unit instance")))
        ),
        "{:?}",
        report.not_run
    );
    assert_eq!(report.classify(0), ExitClass::CleanupFailed);
    let boundary = boundary_of(report);
    let observed = pending_state(&boundary);
    assert!(
        observed.accepted && observed.instance && observed.candidate && !observed.owned,
        "{observed:?}"
    );
    world.release();
    let boundary = retained_through(boundary, 2, "replaced while the proof waits");
    replacement_untouched(&world, "replaced while the proof waits");
    assert!(boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
    // After the execution: the helper was never placed; this request's unit
    // is unloaded and the replacement takes the helper. Retries observe the
    // replacement's cgroup and never bind or end it.
    let world = World::new(|state| state.placement = Placement::Never);
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    {
        let mut state = world.lock();
        let unit = state.requested().unwrap();
        assert!(state.end_by_itself_and_collect(&unit));
        let index = state.load_foreign(&unit, HOLDER);
        state.place(pid, index);
    }
    let boundary = match boundary.retry() {
        Ok(()) => {
            panic!("replaced after the execution: confirmed while the replacement holds the helper")
        }
        Err(boundary) => boundary,
    };
    let observed = pending_state(&boundary);
    assert!(
        observed.candidate && !observed.owned,
        "replaced after the execution: the replacement's cgroup was owned through the helper's membership: {observed:?}"
    );
    let boundary = retained_through(boundary, 1, "replaced after the execution");
    replacement_untouched(&world, "replaced after the execution");
    assert!(boundary.holds_helper() && is_child(pid));
    abandon(boundary, pid);
    // An owned candidate whose directory goes with its unit, a replacement
    // then loaded at the same path: confirmed through the retained
    // descriptor (removed); never re-opened from its path, so the
    // replacement's cgroup is never opened or ended.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let report = run_in(&world, None);
    let pid = requested_pid(&world);
    let boundary = boundary_of(report);
    assert!(pending_state(&boundary).owned);
    world.release();
    let since = world.lock().calls.len();
    let foreign = reloaded_by_another(&world);
    assert!(
        boundary.retry().is_ok(),
        "a removed owned candidate was not confirmed through its descriptor"
    );
    assert_eq!(
        opens(&world),
        1,
        "a candidate was reconstructed from its path"
    );
    reuser_untouched(&world, foreign, since, "a removed owned candidate");
    assert!(!is_child(pid));
}

#[test]
fn r3r1_03_a_start_with_its_exact_identity_is_bound_owned_and_proven() {
    // The start's identity is captured exactly and the helper is placed in
    // the unit's own cgroup: bound (cleanup ownership), every policy proven,
    // the scope proven and bound to that invocation and that directory.
    let world = World::default();
    let (placed, pid) = place_cat(&world);
    let placed = placed.expect("a start with its exact identity was not proven");
    let (unit, instance, id) = {
        let state = world.lock();
        let unit = state.requested().unwrap();
        let instance = state.units[&unit].instance;
        (unit, instance, state.created().unwrap().id)
    };
    let scope = placed.scope().expect("the scope is proven");
    assert_eq!(scope.invocation_id(), instance.bytes());
    assert_eq!(scope.cgroup_id().unwrap(), id);
    assert_eq!(world.lock().calls, proof_calls(&unit, pid));
    assert!(placed.end().is_confirmed());
    assert!(!is_child(pid));
    // The normal execution path stays usable: proven, launched, finalized
    // through the descriptor.
    let world = World::default();
    let report = run_in(&world, None);
    assert!(launched(&report) && report.cleanup.is_confirmed());
    assert_eq!(report.classify(0), ExitClass::SandboxSetupFailed);
    assert!(world.lock().created().unwrap().killed);
    assert!(kills_follow_a_binding(&world));
}

#[test]
fn r3r1_04_an_identity_that_is_not_the_captured_one_is_never_bound() {
    // Everything else matches exactly; the manager's unit reports another
    // identity (at every read, or only at the read that closes the
    // binding), or its directory was replaced at the same path under the
    // manager's record (another kernel cgroup ID). The candidate is never
    // owned, ended, proven or launched into.
    let cases: [(&str, Arrange, &str); 3] = [
        (
            "another identity",
            |state| state.unit_instance = Script::always(Property::Wrong),
            "unit instance",
        ),
        (
            "another identity when the binding closes",
            |state| state.unit_instance = Script::first([Property::Exact], Property::Wrong),
            "unit instance",
        ),
        (
            "another directory at the path",
            |state| state.interference = Interference::Swap(HOLDER),
            "unit control group id",
        ),
    ];
    for (case, script, error) in cases {
        let world = World::new(script);
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            !launched(&report),
            "{case}: launched into an unbound cgroup"
        );
        assert!(
            world
                .lock()
                .cgroups
                .iter()
                .all(|cgroup| cgroup.kills == 0 && !cgroup.killed),
            "{case}: a cgroup not bound to the captured identity was ended"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Mismatch(found))) if found == error),
            "{case}: {:?}",
            report.not_run
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.candidate && !observed.owned,
            "{case}: {observed:?}"
        );
        world.release();
        let boundary = retained_through(boundary, 1, case);
        {
            let state = world.lock();
            assert!(
                state
                    .cgroups
                    .iter()
                    .all(|cgroup| cgroup.kills == 0 && !cgroup.killed),
                "{case}: a cgroup not bound to the captured identity was ended"
            );
            assert_eq!(
                state.count(|call| matches!(call, Call::RuntimeMax(_) | Call::OomPolicy(_))),
                0,
                "{case}: a policy was proven unbound"
            );
        }
        abandon(boundary, pid);
    }
}

#[test]
fn r3r1_05_an_unavailable_or_malformed_identity_binds_nothing() {
    // At the capture: the start job's removal never arrives, the job fails,
    // the identity is malformed, null, ambiguous or sent after the removal,
    // or the sender's serials wrapped. The accepted start has no identity:
    // nothing it created is ever opened, ended, proven or launched into, and
    // it is confirmed only once the manager has unloaded the unit by itself
    // (NoSuchUnit, the helper outside).
    for capture in [
        Capture::Timeout,
        Capture::JobFailed,
        Capture::Malformed,
        Capture::Null,
        Capture::Two,
        Capture::Late,
        Capture::Wrapped,
    ] {
        let case = format!("{capture:?}");
        let world = World::new(|state| state.capture = capture);
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(!launched(&report), "{case}");
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
            "{case}: {:?}",
            report.not_run
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.accepted && !observed.instance && !observed.candidate && !observed.owned,
            "{case}: {observed:?}"
        );
        let boundary = retained_through(boundary, 1, &case);
        unloaded(&world);
        boundary.retry().unwrap();
        assert!(!is_child(pid), "{case}");
        assert_eq!(
            world
                .lock()
                .count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
            0,
            "{case}: what a start without an identity created was opened or ended"
        );
    }
    // At the binding: the manager's answer is an error, a timeout, a broken
    // connection, a reply that does not decode, or no usable identity
    // (another type, another length, all zero: no identity). Never bound or
    // ended; once the manager answers exactly, bound, ended and confirmed
    // through the descriptor.
    for answer in [
        Property::Uncertain(Reply::Error),
        Property::Uncertain(Reply::Timeout),
        Property::Uncertain(Reply::Disconnect),
        Property::Uncertain(Reply::Malformed),
        Property::WrongType,
    ] {
        let case = format!("{answer:?}");
        let world = World::new(|state| state.unit_instance = Script::always(answer));
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(!launched(&report), "{case}");
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        let observed = pending_state(&boundary);
        assert!(
            observed.candidate && !observed.owned,
            "{case}: {observed:?}"
        );
        assert_eq!(
            world.lock().created().unwrap().kills,
            0,
            "{case}: a candidate was ended unbound"
        );
        world.release();
        world.lock().unit_instance = Script::always(Property::Exact);
        boundary.retry().unwrap();
        assert!(world.lock().created().unwrap().killed, "{case}");
        assert!(!is_child(pid), "{case}");
        assert!(kills_follow_a_binding(&world), "{case}");
    }
}

#[test]
fn r3r1_06_an_owned_candidate_whose_policy_fails_is_ended_never_launched() {
    // The candidate is bound to the captured identity (cleanup ownership);
    // then a limit, the runtime backstop, the out-of-memory policy, or the
    // identity read after them is not as required: no launch, and the owned
    // candidate is ended through its descriptor and confirmed. A foreign
    // unit elsewhere is never touched.
    let cases: [(&str, Arrange, &str); 4] = [
        (
            "a limit",
            |state| state.limit_mismatch = Some("memory.max"),
            "memory.max",
        ),
        (
            "the runtime backstop",
            |state| state.runtime_max = Property::Wrong,
            "runtime backstop",
        ),
        (
            "the out-of-memory policy",
            |state| state.oom_policy = Property::Wrong,
            "out-of-memory policy",
        ),
        (
            "the identity after the policy",
            |state| {
                state.unit_instance =
                    Script::first([Property::Exact, Property::Exact], Property::Wrong)
            },
            "unit instance",
        ),
    ];
    for (case, script, error) in cases {
        let world = World::new(|state| {
            script(state);
            state.load_foreign("foreign.scope", HOLDER);
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(!launched(&report), "{case}: launched despite the policy");
        assert!(
            report.cleanup.is_confirmed(),
            "{case}: the owned candidate was not ended through its descriptor"
        );
        assert!(
            matches!(report.not_run, Some(NotRun::Scope(ScopeError::Mismatch(found))) if found == error),
            "{case}: {:?}",
            report.not_run
        );
        assert!(
            world.lock().created().unwrap().killed,
            "{case}: the owned candidate was not ended through its descriptor"
        );
        assert!(kills_follow_a_binding(&world), "{case}");
        let state = world.lock();
        let foreign = &state.cgroups[state.units["foreign.scope"].cgroup.unwrap()];
        assert!(
            !foreign.killed && foreign.kills == 0 && foreign.members.contains(&HOLDER),
            "{case}: a foreign unit was touched"
        );
        drop(state);
        assert!(!is_child(pid), "{case}");
    }
}

#[test]
fn r3r1_07_an_uncertain_start_that_created_its_scope_is_never_ended_proven_or_launched() {
    // The start created the unit and placed the helper, but the success
    // reply was lost: no identity exists independent of that reply, so the
    // cgroup is never opened, ended, proven or launched into, and the
    // operation stays owned with its helper unreaped, even once the manager
    // has unloaded the unit by itself (the accepted availability cost).
    for reply in [
        Reply::Timeout,
        Reply::Disconnect,
        Reply::Error,
        Reply::Malformed,
    ] {
        let case = format!("{reply:?}");
        let world = World::new(|state| state.start_reply = reply);
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(
            !launched(&report),
            "{case}: an uncertain start was launched into"
        );
        assert_eq!(report.classify(0), ExitClass::CleanupFailed, "{case}");
        let boundary = boundary_of(report);
        world.release();
        let boundary = retained_through(boundary, 1, &case);
        {
            let state = world.lock();
            assert_eq!(
                state.count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
                0,
                "{case}: an uncertain start's scope was opened or ended"
            );
            let created = state.created().unwrap();
            assert!(
                created.members.contains(&pid) && created.kills == 0 && !created.killed,
                "{case}"
            );
        }
        unloaded(&world);
        let boundary = retained_through(boundary, 1, &case);
        assert!(boundary.holds_helper() && is_child(pid), "{case}");
        abandon(boundary, pid);
    }
    // Through the live harness's direct owner.
    let world = World::new(|state| state.start_reply = Reply::Timeout);
    let (placed, pid) = place_cat(&world);
    let failed = placed.expect_err("an uncertain start was proven");
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("an uncertain start was confirmed");
    };
    assert_eq!(
        world
            .lock()
            .count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
        0,
        "an uncertain start's scope was opened or ended"
    );
    abandon(boundary, pid);
}

#[test]
fn r3r1_08_a_delivered_collision_is_unchanged() {
    // Exactly `UnitExists`, delivered (I4-R1; i4r1_15, i4r1_16): the foreign
    // unit, holding a process of its own and the helper or not, is never
    // opened, ended, read, proven or launched into; the operation is
    // confirmed once the helper is outside it.
    for holds in [false, true] {
        let case = format!("holds the helper: {holds}");
        let world = World::new(|state| {
            foreign_collision(state, Reply::Delivered);
            state.collision_holds_helper = holds;
        });
        let report = run_in(&world, None);
        let pid = requested_pid(&world);
        assert!(!launched(&report), "{case}");
        assert!(
            matches!(&report.not_run, Some(NotRun::Scope(ScopeError::Bus(reason))) if reason.contains("the unit exists")),
            "{case}: {:?}",
            report.not_run
        );
        if holds {
            let boundary = boundary_of(report);
            let observed = pending_state(&boundary);
            assert!(observed.collided && !observed.instance && !observed.candidate);
            world.lock().membership.remove(&pid);
            boundary.retry().unwrap();
        } else {
            assert!(report.cleanup.is_confirmed(), "{case}");
        }
        foreign_untouched(&world, &case);
        assert_eq!(
            world.lock().count(|call| matches!(
                call,
                Call::GetUnit(_)
                    | Call::UnitInstance(_)
                    | Call::ControlGroupId(_)
                    | Call::RuntimeMax(_)
                    | Call::OomPolicy(_)
            )),
            0,
            "{case}: the foreign unit was claimed"
        );
        assert!(!is_child(pid), "{case}");
    }
}

#[test]
fn r3r1_09_no_panic_grants_cleanup_or_launch_authority_early() {
    // A panic at every identity and ownership boundary: inside the manager
    // after the start is dispatched, once it returned, after its success
    // reply while the identity is captured; once the candidate is retained;
    // inside the binding; once it is owned; during the policy proof; at the
    // promotion to the proven scope. No panic crosses the owner, nothing is
    // ended before a complete binding of it, nothing is launched, and the
    // helper is never reaped while the operation is unresolved. Before the
    // identity is recorded the operation stays owned unconfirmed; after it,
    // the candidate is bound (again, by the finalizer, if the panic came
    // first), ended and confirmed.
    #[derive(Debug, Clone, Copy)]
    enum At {
        Fault(FaultPoint),
        Model(Op),
    }
    let points = [
        (At::Model(Op::Start), false),
        (At::Fault(FaultPoint::AfterScopeStart), false),
        (At::Model(Op::Capture), false),
        (At::Fault(FaultPoint::ScopeProof), true),
        (At::Fault(FaultPoint::ScopeCandidate), true),
        (At::Model(Op::GetUnit), true),
        (At::Fault(FaultPoint::ScopeBinding), true),
        (At::Model(Op::UnitInstance), true),
        (At::Model(Op::UnitId), true),
        (At::Model(Op::ControlGroup), true),
        (At::Model(Op::ControlGroupId), true),
        (At::Fault(FaultPoint::ScopeOwned), true),
        (At::Fault(FaultPoint::ScopeProperties), true),
        (At::Model(Op::RuntimeMax), true),
        (At::Fault(FaultPoint::ScopePromotion), true),
    ];
    for (at, identified) in points {
        let mut runs = vec![false];
        if matches!(at, At::Model(_)) {
            runs.push(true);
        }
        for direct in runs {
            let case = format!("{at:?}, direct owner: {direct}");
            let world = World::new(|state| {
                if let At::Model(op) = at {
                    state.panic_at = Some(op);
                }
            });
            let fault = match at {
                At::Fault(point) => Some(Fault::Panic(point)),
                At::Model(_) => None,
            };
            let Ok((cleanup, launched, pid)) = catch_unwind(AssertUnwindSafe(|| {
                if direct {
                    let (placed, pid) = place_cat(&world);
                    let cleanup = match placed {
                        Ok(placed) => panic!("a panicked placement was proven: {placed:?}"),
                        Err(failed) => failed.cleanup,
                    };
                    (cleanup, false, pid)
                } else {
                    let report = run_in(&world, fault);
                    let launched = launched(&report);
                    (report.cleanup, launched, requested_pid(&world))
                }
            })) else {
                panic!("{case}: a panic crossed the owner");
            };
            assert!(!launched, "{case}: launched before the scope was proven");
            assert!(
                kills_follow_a_binding(&world),
                "{case}: a cgroup was ended before it was bound"
            );
            assert!(
                observed_unreaped(&world),
                "{case}: the helper was reaped while unresolved"
            );
            match cleanup {
                Cleanup::Confirmed => {
                    assert!(identified, "{case}: confirmed without an identity");
                    assert!(world.lock().created().unwrap().killed, "{case}");
                    assert!(!is_child(pid), "{case}");
                }
                Cleanup::Failed(boundary) => {
                    assert!(
                        !identified,
                        "{case}: an identified operation was not settled"
                    );
                    let observed = pending_state(&boundary);
                    assert!(
                        observed.issued && !observed.instance && !observed.candidate,
                        "{case}: {observed:?}"
                    );
                    assert!(boundary.holds_helper() && is_child(pid), "{case}");
                    assert_eq!(
                        world
                            .lock()
                            .count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
                        0,
                        "{case}"
                    );
                    abandon(boundary, pid);
                }
            }
        }
    }
}

#[test]
fn r3r1_10_no_authority_appears_when_a_retained_operation_is_retried_without_its_scope_manager() {
    // Retained operations retried on the issuing connection after the
    // ScopeManager that started them is gone: an uncertain start whose
    // helper is in the unit's cgroup, an accepted start without an
    // identity, one whose candidate reports another identity, and one whose
    // identity read went unanswered. Nothing is ended that was not bound; the
    // unanswered one is bound, ended and confirmed once the manager answers.
    let cases: [(&str, Arrange); 4] = [
        ("uncertain", |state| state.start_reply = Reply::Timeout),
        ("no identity", |state| state.capture = Capture::Timeout),
        ("another identity", |state| {
            state.unit_instance = Script::always(Property::Wrong)
        }),
        ("unanswered", |state| {
            state.unit_instance = Script::always(Property::Uncertain(Reply::Timeout))
        }),
    ];
    for (case, script) in cases {
        let world = World::new(script);
        let stand_in = StandIn::new();
        let scopes = world.scopes();
        let report = execute(&scopes, &stand_in.program(), launch_spec(), &LIMITS, None);
        let pid = requested_pid(&world);
        drop(scopes);
        assert!(!launched(&report), "{case}");
        let boundary = boundary_of(report);
        world.release();
        let boundary = retained_through(boundary, 2, case);
        assert_eq!(
            world.lock().count(|call| matches!(call, Call::Kill(_))),
            0,
            "{case}: authority appeared without the ScopeManager"
        );
        if case == "unanswered" {
            world.lock().unit_instance = Script::always(Property::Exact);
            boundary.retry().unwrap();
            assert!(world.lock().created().unwrap().killed, "{case}");
            assert!(kills_follow_a_binding(&world), "{case}");
            assert!(!is_child(pid), "{case}");
        } else {
            assert!(boundary.holds_helper() && is_child(pid), "{case}");
            abandon(boundary, pid);
        }
    }
}

#[test]
fn r3r1_x_a_start_whose_signals_cannot_be_watched_is_never_sent() {
    // The start's own signals are matched before its request exists; when
    // they cannot be (Subscribe or a match refused or timed out), the
    // request is never sent: nothing can exist, so the operation is
    // confirmed at once, nothing is asked of the manager or the kernel
    // beyond it, and the helper is reaped.
    let world = World::new(|state| state.watch_fails = true);
    let stand_in = StandIn::new();
    let scopes = world.scopes();
    let report = execute(&scopes, &stand_in.program(), launch_spec(), &LIMITS, None);
    assert!(!launched(&report));
    assert!(
        matches!(report.not_run, Some(NotRun::Scope(ScopeError::Bus(_)))),
        "{:?}",
        report.not_run
    );
    assert!(
        report.cleanup.is_confirmed(),
        "a start never sent was not confirmed"
    );
    assert_eq!(report.classify(0), ExitClass::SandboxUnavailable);
    let state = world.lock();
    assert!(
        state.calls.is_empty(),
        "a start never sent asked the manager or the kernel: {:?}",
        state.calls
    );
    assert!(state.units.is_empty() && state.cgroups.is_empty());
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-Q3-R1: the never-populated scope lifecycle (live run
// 37163435032, case 18). A start whose helper is killed but unreaped is not
// refused: systemd 255 still resolves the exiting process, the kernel's
// migration skips it, and the start succeeds. The scope runs never populated:
// no cgroup-empty notification ends it, so it stays loaded until its runtime
// backstop expires and the manager collects it. Until then an accepted
// operation without a candidate stays unconfirmed (its helper unreaped);
// only once the manager has collected the unit does a retry confirm it gone,
// and only then is the helper reaped. A scope that was populated and ran
// empty still ends on its notification. Over the deterministic simulation
// only.

use crate::scope::tests::{unreaped, Ended, Lifecycle};

/// A `cat` stand-in killed and observed exited but not reaped, as live case
/// 18 makes its helper before the placement: its process id stays reserved.
fn killed_unreaped() -> (Helper, u32) {
    let mut helper = cat();
    let pid = helper.pid();
    helper.kill().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while is_running(pid) {
        assert!(
            Instant::now() < deadline,
            "the killed stand-in did not exit"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        is_child(pid) && unreaped(pid),
        "the killed stand-in was reaped before its placement"
    );
    (helper, pid)
}

/// Place a killed, unreaped helper over a model whose kernel never moves it:
/// no scope is proven, and the accepted operation is retained without a
/// candidate (the retained boundary, the helper's process id, the unit).
fn never_populated(world: &World) -> (RetainedBoundary, u32, String) {
    let (helper, pid) = killed_unreaped();
    let failed = match place(&world.scopes(), helper, &LIMITS) {
        Ok(placed) => panic!("a scope was proven for an unmovable process: {placed:?}"),
        Err(failed) => failed,
    };
    assert!(
        matches!(failed.error, Some(ScopeError::NotPlaced)),
        "{:?}",
        failed.error
    );
    let Cleanup::Failed(boundary) = failed.cleanup else {
        panic!("confirmed while the never-populated scope is loaded")
    };
    let observed = pending_state(&boundary);
    assert!(
        observed.issued
            && observed.accepted
            && observed.instance
            && !observed.candidate
            && !observed.owned
            && !observed.settled,
        "{observed:?}"
    );
    let unit = world.lock().requested().unwrap();
    (boundary, pid, unit)
}

/// Nothing of the operation was opened or killed by the backend.
fn nothing_opened_or_killed(world: &World) {
    let state = world.lock();
    assert!(
        state
            .calls
            .iter()
            .all(|call| !matches!(call, Call::Open(_) | Call::Kill(_))),
        "{:?}",
        state.calls
    );
}

#[test]
fn q3r1_01_a_never_populated_scope_stays_loaded_and_unconfirmed_before_its_backstop() {
    let world = World::new(|state| state.placement = Placement::Never);
    let (mut boundary, pid, unit) = never_populated(&world);
    // A: its cgroup was never populated, so no cgroup-empty notification
    // ends it, and a running unit is never collected.
    {
        let mut state = world.lock();
        let index = state.units[&unit].cgroup.unwrap();
        assert!(!state.cgroups[index].ever_populated && !state.cgroups[index].populated());
        assert!(
            !state.notify_empty(&unit),
            "a never-populated scope ended on a cgroup-empty notification"
        );
        assert!(!state.collect(&unit), "a running scope was collected");
        assert_eq!(state.units[&unit].lifecycle, Lifecycle::Running);
    }
    // B, E: every retry before the backstop leaves the operation unconfirmed
    // and its helper unreaped; that is expected, never a failure.
    for attempt in 1..=3 {
        boundary = match boundary.retry() {
            Ok(()) => panic!("confirmed before the runtime backstop (retry {attempt})"),
            Err(boundary) => boundary,
        };
        assert!(boundary.holds_scope() && boundary.holds_helper());
        assert!(
            is_child(pid) && unreaped(pid),
            "the helper was reaped before the operation was confirmed (retry {attempt})"
        );
        let observed = pending_state(&boundary);
        assert!(!observed.candidate && !observed.settled, "{observed:?}");
    }
    assert!(world.lock().units.contains_key(&unit));
    nothing_opened_or_killed(&world);
    // Its backstop then ends it and the manager collects it (q3r1_02).
    {
        let mut state = world.lock();
        assert!(state.expire_backstop(&unit) && state.collect(&unit));
    }
    boundary.retry().unwrap();
    assert!(!is_child(pid));
}

#[test]
fn q3r1_02_only_the_backstop_and_the_collection_let_a_retry_confirm_it() {
    let world = World::new(|state| state.placement = Placement::Never);
    let (boundary, pid, unit) = never_populated(&world);
    // C: the backstop ends it (stopped, failed 'timeout'); until the manager
    // collects it, it is still loaded and nothing is confirmed.
    {
        let mut state = world.lock();
        assert!(state.expire_backstop(&unit), "the backstop did not end it");
        assert_eq!(
            state.units[&unit].lifecycle,
            Lifecycle::Ended(Ended::Backstop)
        );
        assert!(state.units.contains_key(&unit));
    }
    let boundary = match boundary.retry() {
        Ok(()) => panic!("confirmed before the manager collected the ended scope"),
        Err(boundary) => boundary,
    };
    assert!(
        is_child(pid) && unreaped(pid),
        "the helper was reaped before the operation was confirmed"
    );
    // D: once collected, the manager answers it gone with the helper
    // outside: a retry confirms the operation, and (E) only then is the
    // helper reaped.
    assert!(
        world.lock().collect(&unit),
        "the ended scope was not collected"
    );
    assert!(!world.lock().units.contains_key(&unit));
    if let Err(boundary) = boundary.retry() {
        panic!("the collected scope's operation was not confirmed: {boundary:?}");
    }
    assert!(!is_child(pid), "the helper was not reaped once confirmed");
    nothing_opened_or_killed(&world);
}

#[test]
fn q3r1_03_a_populated_scope_that_runs_empty_still_ends_on_its_notification() {
    // F: the retained candidate's cgroup holds a process of its own until
    // released. While populated nothing ends it; once it has run empty, the
    // notification ends it and the manager collects it, as before.
    let world = World::new(|state| {
        failing_proof(state);
        state.phantom = Phantom::Forever;
    });
    let boundary = boundary_of(run_in(&world, None));
    let unit = world.lock().requested().unwrap();
    {
        let mut state = world.lock();
        let index = state.units[&unit].cgroup.unwrap();
        assert!(state.cgroups[index].ever_populated && state.cgroups[index].populated());
        assert!(
            !state.notify_empty(&unit),
            "a populated scope ended on a notification"
        );
    }
    world.release();
    {
        let mut state = world.lock();
        assert!(
            state.notify_empty(&unit),
            "a populated scope that ran empty did not end on its notification"
        );
        assert_eq!(
            state.units[&unit].lifecycle,
            Lifecycle::Ended(Ended::Emptied)
        );
        assert!(state.collect(&unit));
    }
    boundary.retry().unwrap();
    assert!(world.lock().created().unwrap().removed);
}
