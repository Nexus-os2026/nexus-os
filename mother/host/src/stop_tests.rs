//! Registered before the Stop implementation. All waits use readiness.
//! Session-spec §5 amendment: the two Gate tests below check presentation
//! projections only. They establish no backend authority or sink coverage.
//! Session-sink schedules and real-route controls are separately required.
use crate::stop::{Gate, Route, StopRoute};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[test]
fn m2dd_presentation_gate_projects_each_route_and_reserved_callbacks() {
    let gate = Gate::default();
    for route in Route::ALL {
        assert!(gate.admit(route));
    }
    let reserved_epoch = gate.epoch();
    gate.emergency();
    assert!(gate.recovery());
    assert_ne!(reserved_epoch, gate.epoch());
    for route in Route::ALL {
        assert!(!gate.admit(route), "recovery route {route:?}");
    }
    gate.emergency();
    assert!(gate.recovery());
    for route in Route::ALL {
        assert!(!gate.admit(route), "repeated Stop route {route:?}");
    }
}

#[test]
fn m2dd_stop_cap_timeout_release_join_and_reaping() {
    let gate = Arc::new(Gate::default());
    let held = Arc::new((Mutex::new((0usize, false)), Condvar::new()));
    let body = held.clone();
    let route = StopRoute::with_body(
        gate.clone(),
        Arc::new(move || {
            let (lock, changed) = &*body;
            let mut state = lock.lock().unwrap();
            state.0 += 1;
            changed.notify_all();
            let (state, timeout) = changed
                .wait_timeout_while(state, Duration::from_secs(10), |s| !s.1)
                .unwrap();
            assert!(!timeout.timed_out() && state.1);
        }),
    );
    for _ in 0..5 {
        route.press();
    }
    let (lock, changed) = &*held;
    let (state, timeout) = changed
        .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(2), |s| s.0 < 4)
        .unwrap();
    assert!(!timeout.timed_out());
    assert_eq!(state.0, 4);
    drop(state);
    assert_eq!(gate.epoch(), 5);
    let receipt = route.receipt();
    assert_eq!(receipt.outstanding, 4);
    assert_eq!(receipt.coalesced, 1);
    assert!(!route.join_until(Instant::now()));
    lock.lock().unwrap().1 = true;
    changed.notify_all();
    assert!(route.join_until(Instant::now() + Duration::from_secs(2)));
    assert_eq!(route.receipt().outstanding, 0);
    // A9: the fifth event is deferred recording, not a discharged event.
    assert_eq!(route.receipt().finished, 5);
    assert_eq!(lock.lock().unwrap().0, 5);
    assert_eq!(route.retained_handles(), 0);
}

#[test]
fn m2dd_presentation_counters_have_positive_controls_for_all_labels() {
    let gate = Gate::default();
    for route in Route::ALL {
        assert!(gate.admit(route));
    }
    assert_eq!(gate.admitted(), [1; 7]);
    let reserved = || {
        for route in Route::ALL {
            assert!(!gate.admit(route), "reserved callback {route:?}");
        }
    };
    gate.emergency();
    reserved();
    gate.emergency();
    reserved();
    assert_eq!(gate.admitted(), [1; 7]);
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
        let home = crate::Home::open(std::path::Path::new(&home), Arc::new(NoDialogs)).unwrap();
        let route = StopRoute::new(Arc::default());
        assert!(route.bind(home.owner().clone()));
        std::fs::write(std::env::var_os("M2DD_ARM").unwrap(), b"armed").unwrap();
        route.press();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !home.owner().stop_snapshot().broker_stopped {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(home.owner().stop_snapshot().kernel_stopped);
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
        home.owner().shut_down();
        assert!(matches!(home.close_session(), EndStatus::Recorded { .. }));
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
