//! M2a-S Codex REJECT MAJOR (oracle first): every journaled open request
//! is handed back to the port before any fallible session write, so an
//! open that fails still withdraws it; and the port's withdraw is an error,
//! not Ok, for a journaled action it was handed but does not hold (while
//! the broker still holds it open). Run from `real_broker.rs`.

use super::m2as_support::{owner_runs, Disk, WorldServer};
use super::{GovernedPort, Home};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::governed::GovernedControl;
use std::net::TcpListener;
use std::sync::Arc;

fn open(
    store: Disk,
    port: &Arc<GovernedPort>,
) -> Result<Coordinator<GovernedPort>, crate::coordinator::CoordinatorError> {
    Coordinator::open(
        store,
        port.clone(),
        Lanes::new(keyed_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
    )
}

/// A mission waiting for the Owner's approval of a POST; its request.
fn waiting(
    control: &GovernedControl,
    mother: &Coordinator<GovernedPort>,
    port: &GovernedPort,
    url: &str,
) -> (MissionId, CommitmentView) {
    let id = mother
        .draft(contract_with(&format!("POST-BUILDER:{url}")))
        .unwrap();
    mother.submit(id).unwrap();
    assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
    for _ in 0..3 {
        mother.advance(id).unwrap();
    }
    assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
    let run = port.run_of(id).unwrap();
    let view = control
        .authority()
        .commitments()
        .views_of_run(run)
        .into_iter()
        .find(|v| v.state == CommitmentState::Prepared)
        .expect("the waiting request");
    (id, view)
}

/// A clean close with a mission waiting, a rebuilt port, then an open whose
/// session record fails: the request is withdrawn, never left approvable.
fn a_failed_session_record_still_withdraws_every_journaled_request(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    for (n, poison_fails) in [false, true].into_iter().enumerate() {
        let disk = Disk::over(MemoryJournal::new());
        let port = Arc::new(GovernedPort::new(control.clone()));
        let mother = open(disk.clone(), &port).unwrap();
        let path = format!("/adopt-session-{n}");
        let (_, view) = waiting(control, &mother, &port, &format!("{origin}{path}"));
        mother.close().unwrap();

        let rebuilt = Arc::new(GovernedPort::new(control.clone()));
        disk.fail(true, poison_fails);
        assert!(
            open(disk.clone(), &rebuilt).is_err(),
            "the session record failed"
        );
        disk.fail(false, false);
        let now = control.authority().commitments().view(view.id).unwrap();
        assert!(
            !matches!(
                now.state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ),
            "{n}: the request is still open: {:?}",
            now.state
        );
        assert!(
            !owner_runs(control, &now),
            "{n}: the Owner could still run it"
        );
        assert!(!world.saw(&path), "{n}");
    }
}

/// The port's withdraw for a handed-back action: Err while the broker still
/// holds it open and the port does not; Ok once the broker holds it final;
/// Ok for a mission it was never handed anything for.
fn withdraw_is_an_error_for_a_journaled_action_not_held(
    control: &Arc<GovernedControl>,
    origin: &str,
) {
    let port = Arc::new(GovernedPort::new(control.clone()));
    let mother = open(Disk::over(MemoryJournal::new()), &port).unwrap();
    let (id, _) = waiting(control, &mother, &port, &format!("{origin}/adopt-held"));
    let pending = port.pending(id).expect("the pending action");

    // Another mission claims the same action: the port takes it back for
    // neither (its run is held already), so it cannot withdraw it.
    let other = MissionId(9_001);
    port.adopt(other, &pending);
    assert!(
        port.withdraw(other).is_err(),
        "a journaled action the port does not hold was 'withdrawn'"
    );
    assert!(
        port.withdraw(MissionId(9_002)).is_ok(),
        "nothing handed back"
    );

    // Once the request is final at the broker, there is nothing to withdraw.
    assert_eq!(mother.stop(id), Ok(MissionState::Stopped));
    let rebuilt = GovernedPort::new(control.clone());
    let late = MissionId(9_003);
    rebuilt.adopt(late, &pending);
    assert!(
        rebuilt.withdraw(late).is_ok(),
        "a final request is withdrawn"
    );
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    a_failed_session_record_still_withdraws_every_journaled_request(&home.control, origin, &world);
    withdraw_is_an_error_for_a_journaled_action_not_held(&home.control, origin);
    world.stop();
}
