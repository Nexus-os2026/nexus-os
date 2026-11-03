//! M2d-C round 4 (settlement design v2, rule 3; pre-registered before the
//! API exists): a door's stop is engaged in memory first, with no lock that
//! can be poisoned: from then on none of its runs is admitted, and an
//! execution of any of them sees the cancellation at its final check, even
//! when the run registry cannot be read. Every door method on the Stop,
//! withdraw, reconcile, acknowledge, fence and recovery paths then returns
//! a typed error under poison, never a panic. A child of the B2 oracles.

use super::*;
use crate::governed::{HaltReason, Lookup};

fn no_panic<T>(what: &str, call: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call))
        .unwrap_or_else(|_| panic!("{what} panicked"))
}

/// An execution held before its final check; the run registry poisoned;
/// the door's stop engaged in memory: the held execution publishes nothing,
/// and the withdrawal that follows is a typed error, not a panic.
#[test]
fn m2d_c_r4_the_memory_stop_reaches_the_final_check_without_the_registry() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    let (gate, worker) = held(&f, &view, at::FILE_BEFORE_FINAL);
    f.gov.authority().runs().poison_for_test();
    no_panic("stop_now", || f.door.stop_now());
    let withdrawn = no_panic("cancel_run", || f.door.cancel_run(f.run).map(|_| ()));
    assert!(withdrawn.is_err(), "a typed error");
    gate.open();
    assert!(!worker.join().unwrap(), "refused at its final check");
    assert!(!f.ws.join(NAME).exists(), "nothing published");
}

/// Once stopped in memory, the door admits nothing: no run, no proposal,
/// no authorization of a commitment it holds; its stop is permanent and
/// idempotent.
#[test]
fn m2d_c_r4_a_door_stopped_in_memory_admits_nothing() {
    let f = Fx::new();
    let waiting = f.propose(NAME, CONTENT).unwrap();
    f.door.stop_now();
    f.door.stop_now();
    assert_eq!(class(f.door.open_run()), "halted");
    assert_eq!(class(f.propose("b.txt", "b\n")), "halted");
    assert!(f.authorize(&waiting).is_err());
    assert!(!f.ws.join(NAME).exists());
}

/// Every door method on those paths, with the run registry poisoned and
/// then the commitment registry poisoned: a typed error or an answer,
/// never a panic.
#[test]
fn m2d_c_r4_the_stop_paths_never_panic_under_poison() {
    for poison in ["runs", "commitments"] {
        let f = Fx::new();
        let view = f.ready(NAME, CONTENT);
        f.execute(&view).expect("published");
        match poison {
            "runs" => f.gov.authority().runs().poison_for_test(),
            _ => f.gov.authority().commitments().poison_for_test(),
        }
        no_panic("stop_now", || f.door.stop_now());
        let _ = no_panic("cancel_run", || f.door.cancel_run(f.run).map(|b| b.write()));
        let read = no_panic("try_commitment", || f.door.try_commitment(view.id));
        assert!(
            read.is_err() || matches!(read, Ok(Lookup::Mine(_))),
            "{poison}: {read:?}"
        );
        assert!(
            no_panic("ack", || f.door.ack_commitment(view.id)).is_err(),
            "{poison}"
        );
        assert!(
            no_panic("adopt", || f.door.adopt(f.run, view.id)).is_err(),
            "{poison}"
        );
        no_panic("halt", || f.door.halt(HaltReason::JournalFailed).write());
        no_panic("emergency_stopped", || f.door.emergency_stopped());
    }
}
