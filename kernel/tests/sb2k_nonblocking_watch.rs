//! SB-2k public Stop-watch observations; K2 is a private-lock unit oracle.
use nexus_kernel::emergency_stop::{EmergencyStopped, StopAuthority};

#[test]
fn sb2k_check_now_agrees_with_check() {
    let stop = StopAuthority::new();
    let controller = stop.claim_controller().unwrap();
    let mut watches = Vec::new();
    for _ in 0..3 {
        let fresh = stop.admit().unwrap();
        assert_eq!(fresh.check(), Ok(()));
        assert_eq!(fresh.check_now(), Ok(()));
        assert!(!fresh.halted_now());
        watches.push(fresh);
        let number = stop.engage();
        for watch in &watches {
            assert_eq!(watch.check(), Err(EmergencyStopped));
            assert_eq!(watch.check_now(), watch.check(), "K1: after engage");
            assert!(watch.halted_now());
        }
        assert!(controller.lift_if(number));
        for watch in &watches {
            assert_eq!(watch.check(), Err(EmergencyStopped));
            assert_eq!(watch.check_now(), watch.check(), "K1: after resume");
            assert!(watch.halted_now());
        }
    }
}

#[test]
fn sb2k_engage_publishes_before_returning() {
    for iteration in 0..1_000 {
        let stop = StopAuthority::new();
        let watch = stop.admit().unwrap();
        stop.engage();
        let halted = std::thread::spawn(move || watch.halted_now())
            .join()
            .unwrap();
        assert!(halted, "K3: unpublished stop at iteration {iteration}");
    }
}
