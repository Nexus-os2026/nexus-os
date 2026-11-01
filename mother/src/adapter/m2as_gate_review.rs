//! M2a-S, oracles added after the gate review (they do not replace or edit
//! the earlier ones). Run from `real_broker.rs` (one broker per process).
//!
//! - MINOR 1: a panic inside a broker call that poisons the port's gate does
//!   not stop the fence: at the next break every run is still cancelled.
//! - MINOR 2: every request handed back for a mission counts: one that is
//!   final does not hide a second one still open.

use super::m2as_support::{rig, Disk, Rig};
use super::{GovernedPort, Home};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::governed::GovernedControl;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(
        r.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    id
}

fn a_poisoned_gate_still_fences(control: &Arc<GovernedControl>, origin: &str) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let waiting = approved(&r, &format!("POST-BUILDER:{origin}/gate-poison-wait"));
    for _ in 0..3 {
        r.mother.advance(waiting).unwrap();
    }
    assert_eq!(
        r.mother.state(waiting),
        Some(MissionState::AwaitingApproval)
    );
    let waiting_run = r.port.run_of(waiting).unwrap();

    // A panic inside the read gate (in run_for, before the broker opens a
    // run) poisons it.
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/gate-poison-b"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    r.port
        .before_open_run
        .arm(0, || panic!("a broker call panicked inside the gate"));
    let mother = r.mother.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| mother.advance(b))).is_err());

    // The break: the fence still runs, and cancels every run.
    disk.fail(true, true);
    let stopped = catch_unwind(AssertUnwindSafe(|| r.mother.stop(victim)));
    disk.fail(false, false);
    assert!(
        matches!(stopped, Ok(Err(_))),
        "the break did not return an error (it panicked: {})",
        stopped.is_err()
    );
    assert!(r.port.fenced());
    assert!(
        control
            .authority()
            .runs()
            .view(waiting_run)
            .unwrap()
            .cancelled,
        "a poisoned gate stopped the fence"
    );
}

fn every_handed_request_counts(control: &Arc<GovernedControl>, origin: &str) {
    let port = Arc::new(GovernedPort::new(control.clone()));
    let mother = Coordinator::open(
        MemoryJournal::new(),
        port.clone(),
        Lanes::new(keyed_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
    )
    .unwrap();
    let waiting = |path: &str| {
        let id = mother
            .draft(contract_with(&format!("POST-BUILDER:{origin}/{path}")))
            .unwrap();
        mother.submit(id).unwrap();
        assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
        id
    };
    let (n1, n2) = (waiting("gate-handed-1"), waiting("gate-handed-2"));
    let (c1, c2) = (port.pending(n1).unwrap(), port.pending(n2).unwrap());
    port.withdraw(n1).unwrap(); // C1 is final now; C2 stays open.

    // Mission M (holding nothing) is handed back the final C1 first, then
    // the open C2.
    let m = MissionId(9_201);
    port.hand(m, &c1);
    port.hand(m, &c2);
    assert!(
        port.withdraw(m).is_err(),
        "a final request handed back first hid an open one"
    );
    port.withdraw(n2).unwrap();
    assert!(port.withdraw(m).is_ok(), "both final: nothing to withdraw");
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str) {
    a_poisoned_gate_still_fences(&home.control, origin);
    every_handed_request_counts(&home.control, origin);
}
