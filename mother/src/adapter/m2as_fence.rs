//! M2a-S Codex REJECT BLOCKER (oracle first): a call reserved before the
//! coordinator breaks must not reach the world after it. The fence is at
//! the authority: when the coordinator breaks, every broker run its port
//! holds is cancelled, so an effect already reserved is refused by the
//! broker; and a result that comes back after the break is discarded.
//!
//! Deterministic interleavings on the real broker: a hook in the port,
//! a lane or the Owner's dialog runs the other thread's work (a Stop of
//! another mission whose append and poisoning both fail) at exactly that
//! point of mission B's call. Run from `real_broker.rs` (one broker per
//! process).

use super::m2as_support::{
    executed, owner_runs, rig, Disk, HookConfirmer, HookSlot, Rig, WorldServer,
};
use super::{GovernedPort, Home};
use crate::coordinator::Coordinator;
use crate::journal::{Event, MemoryJournal, MissionId};
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::{Arc, Weak};

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(
        r.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    id
}

/// The other thread: a Stop of `victim` whose append and poisoning both
/// fail, so the coordinator breaks.
fn break_by_stopping(
    disk: &Disk,
    mother: &Arc<Coordinator<GovernedPort>>,
    victim: MissionId,
) -> impl FnOnce() + Send + 'static {
    let (disk, mother): (Disk, Weak<Coordinator<GovernedPort>>) =
        (disk.clone(), Arc::downgrade(mother));
    move || {
        disk.fail(true, true);
        let mother = mother.upgrade().expect("the coordinator");
        assert!(mother.stop(victim).is_err(), "the Stop is not durable");
        assert!(mother.broken());
        disk.fail(false, false);
    }
}

fn fresh(control: &Arc<GovernedControl>) -> (Rig, Disk, MissionId) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    (r, disk, victim)
}

fn waiting_view(control: &GovernedControl, run: RunId) -> CommitmentView {
    let views: Vec<CommitmentView> = control
        .authority()
        .commitments()
        .views_of_run(run)
        .into_iter()
        .filter(|v| v.state == CommitmentState::Prepared)
        .collect();
    assert_eq!(views.len(), 1, "{views:?}");
    views.into_iter().next().unwrap()
}

/// B has a run already (its Builder's effect ran): its Verifier's effect is
/// reserved (journaled dispatched) and the coordinator breaks just before
/// the port performs it.
fn a_reserved_effect_of_a_live_run_is_refused_by_the_broker(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (r, disk, victim) = fresh(control);
    let b = approved(
        &r,
        &format!("FETCH-BUILDER:{origin}/fence-perform-1 FETCH-VERIFIER:{origin}/fence-perform-2"),
    );
    for _ in 0..3 {
        r.mother.advance(b).unwrap();
    }
    assert!(world.saw("/fence-perform-1"));
    let run = r.port.run_of(b).expect("B's run");
    let runs = BTreeSet::from([run]);
    let before = executed(control, &runs);
    r.port
        .before_perform
        .arm(0, break_by_stopping(&disk, &r.mother, victim));
    assert!(r.mother.advance(b).is_err());
    assert!(!r.port.before_perform.armed(), "the hook ran");
    assert!(
        !world.saw("/fence-perform-2"),
        "an effect reserved before the break reached the world"
    );
    assert_eq!(executed(control, &runs), before, "the broker ran it");
    assert!(
        control.authority().runs().view(run).unwrap().cancelled,
        "B's run is cancelled at the broker"
    );
    assert!(r.port.fenced());
    for _ in 0..3 {
        assert!(r.mother.advance(b).is_err());
    }
    assert!(!world.saw("/fence-perform-2"));
}

/// B's first effect: no run yet when the coordinator breaks; the fenced
/// port opens none.
fn a_reserved_first_effect_opens_no_run(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (r, disk, victim) = fresh(control);
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/fence-first"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    assert_eq!(r.port.run_of(b), None);
    r.port
        .before_perform
        .arm(0, break_by_stopping(&disk, &r.mother, victim));
    assert!(r.mother.advance(b).is_err());
    assert!(!r.port.before_perform.armed(), "the hook ran");
    assert!(!world.saw("/fence-first"), "the effect reached the world");
    if let Some(run) = r.port.run_of(b) {
        assert_eq!(executed(control, &BTreeSet::from([run])), 0);
        assert!(control.authority().runs().view(run).unwrap().cancelled);
    }
}

/// B's Builder is called (journaled invoked) and the coordinator breaks
/// before it answers: its answer is discarded, nothing is performed.
fn a_lane_answer_after_the_break_is_discarded(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (r, disk, victim) = fresh(control);
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/fence-lane"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    let performed = r.port.performed();
    // Building runs the Sentinel, then the Builder.
    r.lane_hook
        .arm(1, break_by_stopping(&disk, &r.mother, victim));
    assert!(r.mother.advance(b).is_err());
    assert!(!r.lane_hook.armed(), "the hook ran");
    assert_eq!(r.port.performed(), performed, "an effect was performed");
    assert!(!world.saw("/fence-lane"));
    assert!(
        !r.mother
            .journal()
            .unwrap()
            .into_iter()
            .any(|e| e.mission == b && matches!(e.event, Event::EffectDispatched { .. })),
        "an effect of B was dispatched"
    );
}

/// The Owner says yes to B while the coordinator breaks: no consent is
/// taken, nothing runs.
fn a_consent_after_the_break_is_discarded(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (r, disk, victim) = fresh(control);
    let b = r
        .mother
        .draft(contract_with(&format!(
            "FETCH-BUILDER:{origin}/fence-consent"
        )))
        .unwrap();
    r.mother.submit(b).unwrap();
    let hook = HookSlot::default();
    hook.arm(0, break_by_stopping(&disk, &r.mother, victim));
    let owner = HookConfirmer {
        hook: hook.clone(),
        yes: true,
    };
    assert!(r.mother.request_consent(b, &owner).is_err());
    assert!(!hook.armed(), "the hook ran");
    assert_ne!(r.mother.state(b), Some(MissionState::Approved));
    assert_eq!(r.mother.consented_hash(b), None);
    for _ in 0..3 {
        assert!(r.mother.advance(b).is_err());
    }
    assert!(!world.saw("/fence-consent"));
    assert!(!r
        .mother
        .journal()
        .unwrap()
        .into_iter()
        .any(|e| e.mission == b
            && matches!(
                e.event,
                Event::Transition {
                    trigger: Trigger::Consent { .. }
                }
            )));
}

/// Mother asks the broker about C's approved action while the coordinator
/// breaks: C does not go on, and another waiting request (D's) can no
/// longer be run by anyone.
fn an_approval_answer_after_the_break_is_discarded(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (r, disk, victim) = fresh(control);
    let c = approved(
        &r,
        &format!("POST-BUILDER:{origin}/fence-approval-post FETCH-VERIFIER:{origin}/fence-approval-after"),
    );
    for _ in 0..3 {
        r.mother.advance(c).unwrap();
    }
    assert_eq!(r.mother.state(c), Some(MissionState::AwaitingApproval));
    let d = approved(&r, &format!("POST-BUILDER:{origin}/fence-approval-other"));
    for _ in 0..3 {
        r.mother.advance(d).unwrap();
    }
    assert_eq!(r.mother.state(d), Some(MissionState::AwaitingApproval));
    let (c_run, d_run) = (r.port.run_of(c).unwrap(), r.port.run_of(d).unwrap());
    // The Owner approved and ran C's action before the break.
    assert!(owner_runs(control, &waiting_view(control, c_run)));
    assert!(world.saw("/fence-approval-post"));
    let d_view = waiting_view(control, d_run);

    r.port
        .before_approval
        .arm(0, break_by_stopping(&disk, &r.mother, victim));
    assert!(r.mother.advance(c).is_err());
    assert!(!r.port.before_approval.armed(), "the hook ran");
    for _ in 0..3 {
        assert!(r.mother.advance(c).is_err());
        assert!(r.mother.advance(d).is_err());
    }
    assert!(
        !world.saw("/fence-approval-after"),
        "C went on after the break"
    );
    // D's request: fenced at the broker, the Owner can no longer run it.
    assert!(
        !owner_runs(control, &d_view),
        "a request outlived the break"
    );
    assert!(!world.saw("/fence-approval-other"));
    assert_eq!(executed(control, &BTreeSet::from([d_run])), 0);

    // A restart holds every unfinished mission, and nothing is open.
    let again = rig(control, Disk::over(disk.inner.clone())).unwrap();
    for id in [victim, c, d] {
        assert_eq!(again.mother.held(id).as_deref(), Some("unclean_shutdown"));
    }
    assert!(
        super::m2as_support::open_requests(control, &BTreeSet::from([c_run, d_run])).is_empty()
    );
}

/// Every ordering, each on a coordinator of its own over the shared broker.
pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    let control = &home.control;
    a_reserved_effect_of_a_live_run_is_refused_by_the_broker(control, origin, &world);
    a_reserved_first_effect_opens_no_run(control, origin, &world);
    a_lane_answer_after_the_break_is_discarded(control, origin, &world);
    a_consent_after_the_break_is_discarded(control, origin, &world);
    an_approval_answer_after_the_break_is_discarded(control, origin, &world);
    world.stop();
}
