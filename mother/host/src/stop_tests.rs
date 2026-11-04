//! Registered before the Stop implementation. All waits use readiness.
use crate::stop::{Gate, Route, StopRoute};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[test]
fn m2dd_recovery_gate_covers_each_route_and_reserved_callbacks() {
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
    assert_eq!(route.receipt().finished, 4);
    assert_eq!(route.retained_handles(), 0);
}
