//! M2a-S, oracles added after the gate review (they do not replace or edit
//! the earlier ones). Run from `real_broker.rs` (one broker per process).
//!
//! - MINOR 1: a gate poisoned by a panic in write mode (inside the fence;
//!   a panic under a read guard does not poison it) still refuses every
//!   broker call, and a second fence still runs, without panicking.
//! - MINOR 2: every request handed back for a mission counts: one that is
//!   final does not hide a second one still open.

use super::m2as_support::{rig, Disk, Rig};
use super::{GovernedPort, Home};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::{ApprovalStatus, EffectPort};
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, get, keyed_lanes, Yes};
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

fn a_poisoned_gate_still_refuses_and_fences(control: &Arc<GovernedControl>, origin: &str) {
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

    // Only a panic while the gate is held in write mode poisons it: the
    // broker panics at the end of the fence (inside write mode).
    r.port
        .after_fence
        .arm(0, || panic!("the broker panicked during the fence"));
    disk.fail(true, true);
    let stopped = catch_unwind(AssertUnwindSafe(|| r.mother.stop(victim)));
    disk.fail(false, false);
    assert!(stopped.is_err(), "the fence's panic was not raised");
    assert!(r.port.fenced());
    assert!(
        control
            .authority()
            .runs()
            .view(waiting_run)
            .unwrap()
            .cancelled,
        "the fence cancelled the runs before the panic"
    );

    // The gate is poisoned now: the port still refuses every broker call,
    // and a second fence still runs, without panicking.
    let port = r.port.clone();
    let approval = catch_unwind(AssertUnwindSafe(|| port.approval(waiting)));
    assert!(
        matches!(&approval, Ok(ApprovalStatus::Lapsed { class }) if class == "port_fenced"),
        "a poisoned gate: the approval query panicked or was not refused"
    );
    let perform = catch_unwind(AssertUnwindSafe(|| {
        port.perform(
            MissionId(9_301),
            &get(&format!("{origin}/gate-poison-after")),
        )
    }));
    assert!(
        matches!(&perform, Ok(Err(refusal)) if refusal.class == "port_fenced"),
        "a poisoned gate: the perform panicked or was not refused"
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| port.fence())).is_ok(),
        "a poisoned gate: a second fence panicked"
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
    a_poisoned_gate_still_refuses_and_fences(&home.control, origin);
    every_handed_request_counts(&home.control, origin);
}
