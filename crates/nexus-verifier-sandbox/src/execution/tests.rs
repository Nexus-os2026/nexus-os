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
        scope: None,
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
    assert!(owned.helper.is_none() && owned.scope.is_none());
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
