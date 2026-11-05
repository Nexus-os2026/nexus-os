//! A9 host worker ownership through the real original session. The obsolete
//! presentation Gate controls are replaced by immutable ingress-envelope checks.
use crate::stop::StopRoute;
use nexus_governed_control::host_session::{SessionSeed, CommandKind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[test]
fn m2dd_original_ingress_invalidates_queued_work_and_repeated_commands() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let run = ingress.accept(CommandKind::Run, 1).unwrap();
    assert_eq!(run.generation(), 0);
    ingress.emergency().unwrap();
    for (i, kind) in [CommandKind::Run, CommandKind::Approve, CommandKind::Undo, CommandKind::Cleanup, CommandKind::Detail, CommandKind::Lift].into_iter().enumerate() {
        assert!(ingress.accept(kind, i as u64 + 2).is_err());
    }
    assert_eq!(run.generation(), 0, "retained envelope is never refreshed");
    ingress.emergency().unwrap();
    assert_eq!(ingress.generation(), 2);
    assert!(ingress.accept(CommandKind::Status, 8).is_ok());
    assert!(ingress.accept(CommandKind::Evidence, 9).is_ok());
}

#[test]
fn m2dd_stop_cap_timeout_release_join_and_reaping() {
    use nexus_governed_control::authority::approval::*;
    use nexus_governed_control::authority::evidence::{EvidenceProjection, EvidenceRecord, EvidencePhase};
    struct No;
    impl ControlConfirmer for No {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool { false }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool { false }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool { false }
        fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool { false }
    }
    struct Held(Arc<(Mutex<(usize,bool)>, Condvar)>);
    impl EvidenceProjection for Held {
        fn project(&self, record: &EvidenceRecord) {
            if record.phase != EvidencePhase::EmergencyStop { return; }
            let (lock, changed) = &*self.0;
            let mut state = lock.lock().unwrap(); state.0 += 1; changed.notify_all();
            let (state, timeout) = changed.wait_timeout_while(state, Duration::from_secs(10), |s| !s.1).unwrap();
            assert!(!timeout.timed_out() && state.1);
        }
    }
    if std::env::var_os("M2DD_HOST_STOP_CAP").is_none() {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "stop_tests::m2dd_stop_cap_timeout_release_join_and_reaping", "--nocapture"]).env("M2DD_HOST_STOP_CAP", "1");
        let mut child = support::Pty::start(command);
        assert!(child.finish().success(), "{}", child.output);
        return;
    }
    let f = support::Fixture::new();
    let seed = SessionSeed::new().unwrap();
    let route = StopRoute::new(seed.ingress());
    let held = Arc::new((Mutex::new((0usize, false)), Condvar::new()));
    let home = crate::session_home::SessionHome::open_projected(seed, &f.home, Arc::new(No), vec![Arc::new(Held(held.clone()))]).unwrap();
    assert!(route.bind(home.session.clone()));
    for _ in 0..5 { route.press(); }
    route.dispatch_pending();
    let (lock, changed) = &*held;
    let (state, timeout) = changed.wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(3), |s| s.0 < 4).unwrap();
    assert!(!timeout.timed_out()); assert_eq!(state.0, 4); drop(state);
    assert_eq!(route.ingress.generation(), 5);
    assert_eq!(route.retained_handles(), 4);
    assert_eq!(route.receipt().outstanding, 5, "four active plus the uncovered fifth event");
    assert_eq!(route.receipt().coalesced, 1);
    assert!(!route.join_until(Instant::now()));
    lock.lock().unwrap().1 = true; changed.notify_all();
    assert!(route.join_until(Instant::now() + Duration::from_secs(3)));
    assert_eq!(route.receipt().finished, 5);
    assert_eq!(lock.lock().unwrap().0, 5);
    assert_eq!(route.receipt().outstanding, 0);
    assert_eq!(route.retained_handles(), 0);
    assert_eq!(home.recent_evidence().iter().filter(|r| r.phase == EvidencePhase::EmergencyStop).count(), 5);
    assert_eq!(home.session.stop_obligations().unwrap().recorded, 5);
}

#[test]
fn m2dd_command_sequence_and_original_close_deadline_are_conserved() {
    let seed = SessionSeed::new().unwrap();
    let ingress = seed.ingress();
    let run = ingress.accept(CommandKind::Run, 1).unwrap();
    assert!(ingress.accept(CommandKind::Run, 1).is_err());
    let quit = ingress.accept(CommandKind::Quit, 2).unwrap();
    let again = ingress.accept(CommandKind::Quit, 3).unwrap();
    assert_eq!(quit.deadline(), again.deadline());
    assert_eq!(quit.close_identity(), again.close_identity());
    assert_eq!(run.generation(), 0);
    assert!(ingress.accept(CommandKind::Run, 4).is_err());
}

#[path = "../tests/support/mod.rs"]
pub(super) mod support;

#[test]
fn m2dd_real_store_timeout_then_release_joins_and_records() {
    use nexus_governed_control::authority::approval::*;
    use nexus_governed_control::authority::evidence::{EndStatus, EvidencePhase};
    struct NoDialogs;
    impl ControlConfirmer for NoDialogs {
        fn confirm_action(&self, _: &ActionConfirmation) -> bool {
            false
        }
        fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
            false
        }
        fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
            false
        }
        fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
            false
        }
    }
    if let Some(home) = std::env::var_os("M2DD_STOP_CHILD_HOME") {
        let seed = SessionSeed::new().unwrap();
        let route = StopRoute::new(seed.ingress());
        let home = crate::session_home::SessionHome::open(seed, std::path::Path::new(&home), Arc::new(NoDialogs)).unwrap();
        assert!(route.bind(home.session.clone()));
        std::fs::write(std::env::var_os("M2DD_ARM").unwrap(), b"armed").unwrap();
        route.press();
        route.dispatch_pending();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !home.session.stop_snapshot().broker_stopped {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(home.session.stop_snapshot().kernel_stopped);
        assert!(!route.join_until(Instant::now()));
        assert_eq!(route.receipt().finished, 0);
        println!("TIMED OUT WITH REAL STORAGE HELD");
        assert!(route.join_until(Instant::now() + Duration::from_secs(5)));
        assert_eq!(route.receipt().finished, 1);
        assert_eq!(route.retained_handles(), 0);
        assert!(home
            .recent_evidence()
            .iter()
            .any(|r| r.phase == EvidencePhase::EmergencyStop));
        assert_eq!(home.evidence_health().owed, 0);
        assert!(matches!(home.session.stop_events().unwrap()[0].phase, nexus_governed_control::host_session::stop::StopPhase::Finished(receipt) if matches!(receipt.record, EndStatus::Recorded { .. })));
        return;
    }
    let f = support::Fixture::new();
    let fault = support::faults::Fault::new(&f, "evidence");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "stop_tests::m2dd_real_store_timeout_then_release_joins_and_records",
            "--nocapture",
        ])
        .env("M2DD_STOP_CHILD_HOME", &f.home);
    fault.attach(&f, &mut command);
    let mut child = support::Pty::start(command);
    let receipt = fault.reached();
    assert!(receipt.path.contains("/p3-evidence/evidence.sqlite3"));
    child.expect("TIMED OUT WITH REAL STORAGE HELD");
    receipt.release();
    assert!(child.finish().success());
}
