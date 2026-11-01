//! M2a-S, oracle added after the fresh review (MINOR 1; it does not replace
//! or edit the earlier ones). Run from `real_broker.rs` (one broker per
//! process).
//!
//! A panic while the fence holds the port's mutex poisons it. Every later
//! port call still refuses cleanly (no panic), and nothing reaches the
//! broker or the world.

use super::m2as_support::{executed, rig, Disk, Rig, WorldServer};
use super::{Home, MOTHER_AGENT};
use crate::effect::{ApprovalStatus, EffectPort};
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, get, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
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

fn mothers_runs(control: &GovernedControl) -> BTreeSet<RunId> {
    control
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .map(|run| run.id)
        .collect()
}

fn a_panic_inside_the_fence_leaves_the_port_refusing(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let waiting = approved(&r, &format!("POST-BUILDER:{origin}/poison-wait"));
    for _ in 0..3 {
        r.mother.advance(waiting).unwrap();
    }
    assert_eq!(
        r.mother.state(waiting),
        Some(MissionState::AwaitingApproval)
    );
    let held = r.port.pending(waiting).expect("the waiting action");

    // The broker panics as the fence cancels a run: inside the port's mutex.
    r.port
        .fence_between_cancels
        .arm(0, || panic!("the broker panicked inside the fence"));
    disk.fail(true, true);
    let stopped = catch_unwind(AssertUnwindSafe(|| r.mother.stop(victim)));
    disk.fail(false, false);
    assert!(stopped.is_err(), "the fence's panic was not raised");
    assert!(
        !r.port.fence_between_cancels.armed(),
        "the fence cancelled nothing"
    );
    assert!(
        r.port.runs.is_poisoned(),
        "the port's mutex is not poisoned"
    );

    let runs = mothers_runs(control);
    let executed_before = executed(control, &runs);
    let port = r.port.clone();
    let calm = |what: &str, call: &dyn Fn()| {
        assert!(
            catch_unwind(AssertUnwindSafe(call)).is_ok(),
            "a poisoned port: {what} panicked"
        );
    };
    calm("fenced", &|| assert!(port.fenced()));
    calm("approval", &|| {
        assert!(matches!(
            port.approval(waiting),
            ApprovalStatus::Lapsed { class } if class == "port_fenced"
        ))
    });
    for mission in [waiting, MissionId(9_401)] {
        calm("perform", &|| {
            let refusal = port
                .perform(mission, &get(&format!("{origin}/poison-after")))
                .expect_err("a perform after the fence");
            assert_eq!(refusal.class, "port_fenced");
        });
    }
    calm("settled", &|| assert!(!port.settled(&held)));
    calm("adopt", &|| {
        port.adopt(MissionId(9_402), &held);
        assert_eq!(port.run_of(MissionId(9_402)), None);
    });
    calm("withdraw", &|| {
        assert_eq!(port.withdraw(waiting).unwrap_err().class, "port_fenced")
    });
    calm("pending", &|| {
        let _ = port.pending(waiting);
    });
    calm("hand", &|| port.hand(MissionId(9_403), &held));
    calm("cancel", &|| port.cancel(MissionId(9_404)));
    calm("finish", &|| port.finish(MissionId(9_405)));
    calm("fence", &|| port.fence());

    assert_eq!(mothers_runs(control), runs, "a run opened after the fence");
    assert_eq!(
        executed(control, &runs),
        executed_before,
        "an action ran after the fence"
    );
    assert!(!world.saw("/poison-after"), "an effect reached the world");
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    a_panic_inside_the_fence_leaves_the_port_refusing(&home.control, origin, &world);
    world.stop();
}
