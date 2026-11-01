//! M2a-S oracles added after mutation testing (they do not replace or edit
//! the earlier ones): two checks survived every earlier oracle.
//!
//! - A fenced port opens no broker run at all (not one opened and then
//!   cancelled).
//! - A broken coordinator answers Err, never Ok, when the broker's answer
//!   about a waited action comes back after the break and still says
//!   waiting.
//!
//! Run from `real_broker.rs` (one broker per process).

use super::m2as_support::{rig, Disk, Rig, WorldServer};
use super::{GovernedPort, Home, MOTHER_AGENT};
use crate::coordinator::Coordinator;
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, Yes};
use crate::state::MissionState;
use nexus_governed_control::governed::GovernedControl;
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

fn breaker(
    disk: &Disk,
    mother: &Arc<Coordinator<GovernedPort>>,
    victim: MissionId,
) -> impl FnOnce() + Send + 'static {
    let (disk, mother): (Disk, Weak<Coordinator<GovernedPort>>) =
        (disk.clone(), Arc::downgrade(mother));
    move || {
        disk.fail(true, true);
        let mother = mother.upgrade().expect("the coordinator");
        assert!(mother.stop(victim).is_err());
        disk.fail(false, false);
    }
}

fn mothers_runs(control: &GovernedControl) -> usize {
    control
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .count()
}

/// B's first effect is reserved and the coordinator breaks before the port
/// performs it: no run of Mother's is opened at the broker.
fn a_fenced_port_opens_no_run(control: &Arc<GovernedControl>, origin: &str, world: &WorldServer) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/mutant-first"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    let runs = mothers_runs(control);
    r.port
        .before_perform
        .arm(0, breaker(&disk, &r.mother, victim));
    assert!(r.mother.advance(b).is_err());
    assert!(!r.port.before_perform.armed(), "the hook ran");
    assert_eq!(mothers_runs(control), runs, "a fenced port opened a run");
    assert!(!world.saw("/mutant-first"));
}

/// Mother asks the broker about C's waiting action while the coordinator
/// breaks; the broker still says waiting: the answer is Err, not Ok.
fn a_waiting_answer_after_the_break_is_an_error(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let c = approved(&r, &format!("POST-BUILDER:{origin}/mutant-waiting"));
    for _ in 0..3 {
        r.mother.advance(c).unwrap();
    }
    assert_eq!(r.mother.state(c), Some(MissionState::AwaitingApproval));
    r.port
        .before_approval
        .arm(0, breaker(&disk, &r.mother, victim));
    let answer = r.mother.advance(c);
    assert!(!r.port.before_approval.armed(), "the hook ran");
    assert!(
        answer.is_err(),
        "a broken coordinator answered {:?}",
        answer.map(|a| a.to)
    );
    assert!(!world.saw("/mutant-waiting"));
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    a_fenced_port_opens_no_run(&home.control, origin, &world);
    a_waiting_answer_after_the_break_is_an_error(&home.control, origin, &world);
    world.stop();
}
