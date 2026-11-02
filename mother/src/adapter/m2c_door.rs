//! M2c-A (PR-A) oracles on the real broker, pre-registered before the door
//! exists (design v3 §7.1, §11 C3): A4 consent through the door, A5 resume
//! asks the Owner (red on main: main resumes without asking), A6 resume is
//! bound (the consent pattern), A6-g (a fence outside a break) and A6-h
//! (added after Codex review 1: a stale take leaves the current request).
//!
//! They reach the port, consent and resume only through `m2c_support`, so
//! they compile against main's API; when the port goes over the door only
//! the helper bodies change. One broker per process: they run from
//! `real_broker_m1_oracles`, before its emergency stop.
//!
//! A held mission is made the way it happens in use: consented, advanced
//! once, then the coordinator is dropped without its orderly close (an
//! unclean exit), and a new coordinator over a new port opens the journal.

use super::m2as_support::{rig, Disk, Rig, WorldServer};
use super::m2c_support::{
    after_held_answer, consent, resume_reply, resume_request, resume_with, take_resume,
    unexpected_dialogs, HeldOwner, Home,
};
use crate::coordinator::CoordinatorError;
use crate::journal::{verify, Event, MemoryJournal, MissionId};
use crate::oracles::m2c_owner::OwnerLiftsTheStop;
use crate::oracles::{contract_with, No, Recording, Yes};
use crate::state::MissionState;
use nexus_governed_control::governed::GovernedControl;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// How many `Resumed` records the journal holds for `id` (read from its
/// lines: a poisoned journal refuses `load`).
fn resumed(disk: &Disk, id: MissionId) -> usize {
    verify(&disk.inner.lines())
        .unwrap()
        .iter()
        .filter(|e| e.mission == id && matches!(e.event, Event::Resumed))
        .count()
}

/// A mission held after an unclean exit, and a coordinator that opened the
/// journal afterwards (over a new port).
fn held(control: &Arc<GovernedControl>, origin: &str, label: &str) -> (Disk, MissionId, Rig) {
    let disk = Disk::over(MemoryJournal::new());
    let first = rig(control, disk.clone()).unwrap();
    let id = first
        .mother
        .draft(contract_with(&format!(
            "FETCH-BUILDER:{origin}/m2c-{label}"
        )))
        .unwrap();
    first.mother.submit(id).unwrap();
    assert_eq!(consent(&first.mother, id, &Yes), Ok(MissionState::Approved));
    assert_eq!(
        first.mother.advance(id).unwrap().to,
        MissionState::Investigating
    );
    drop(first);
    let reopened = rig(control, disk.clone()).unwrap();
    assert_eq!(
        reopened.mother.held(id).as_deref(),
        Some("unclean_shutdown"),
        "{label}: held after the unclean exit"
    );
    (disk, id, reopened)
}

/// A4 (Mother side): consent goes through the door's bound confirmer,
/// asked once with the exact request; a yes consents to that hash, a no
/// denies the mission.
fn m2c_a4_consent_asks_once_through_the_door(control: &Arc<GovernedControl>, origin: &str) {
    let r = rig(control, Disk::over(MemoryJournal::new())).unwrap();
    let id = r
        .mother
        .draft(contract_with(&format!("FETCH-BUILDER:{origin}/m2c-a4")))
        .unwrap();
    r.mother.submit(id).unwrap();
    let owner = Recording::default();
    assert_eq!(consent(&r.mother, id, &owner), Ok(MissionState::Approved));
    let shown = owner.0.lock().unwrap().clone();
    assert_eq!(shown.len(), 1, "asked once");
    let summary = r.mother.confirmation(id).unwrap();
    assert_eq!(shown[0].contract_hash, summary.hash.to_hex());
    assert_eq!(shown[0].summary, summary.lines());
    assert!(shown[0].request.starts_with(&format!("{id}/")));
    assert_eq!(r.mother.consented_hash(id), Some(summary.hash));

    let denied = r
        .mother
        .draft(contract_with(&format!("FETCH-BUILDER:{origin}/m2c-a4-no")))
        .unwrap();
    r.mother.submit(denied).unwrap();
    assert_eq!(
        consent(&r.mother, denied, &No),
        Err(CoordinatorError::Denied)
    );
    assert_eq!(r.mother.state(denied), Some(MissionState::Denied));
    assert_eq!(unexpected_dialogs(), 0);
}

/// A5: a mission held after an unclean exit, and the Owner answers no to
/// the held-mission dialog: still held, `Resumed` not journaled, and the
/// dialog named that mission's contract hash. Main resumes without asking.
fn m2c_a5_resume_asks_the_owner(control: &Arc<GovernedControl>, origin: &str, world: &WorldServer) {
    let (disk, id, r) = held(control, origin, "a5");
    let before = world.count();
    let owner = HeldOwner::new(false);
    let answer = resume_with(&r.mother, id, &owner);
    assert!(
        r.mother.held(id).is_some(),
        "A5: the Owner said no, yet the mission was resumed: {answer:?}"
    );
    assert_eq!(resumed(&disk, id), 0, "A5: Resumed journaled without a yes");
    assert_eq!(format!("{answer:?}"), "Err(Declined)");
    let shown = owner.shown();
    assert_eq!(shown.len(), 1, "A5: the Owner was asked once");
    assert_eq!(
        shown[0].contract_hash,
        r.mother.contract(id).unwrap().hash().to_hex()
    );
    assert_eq!(shown[0].mission, id.to_string());
    assert_eq!(shown[0].held_reason, "unclean_shutdown");
    assert_eq!(r.mother.advance(id), Err(CoordinatorError::Held));
    assert_eq!(world.count(), before, "a held mission reaches nothing");

    // Positive control: yes resumes, once, and the mission goes on.
    let owner = HeldOwner::new(true);
    assert_eq!(
        resume_with(&r.mother, id, &owner),
        Ok(MissionState::Investigating)
    );
    assert_eq!(r.mother.held(id), None);
    assert_eq!(resumed(&disk, id), 1);
    assert_eq!(owner.shown().len(), 1);
    while !r.mother.state(id).unwrap().is_terminal() {
        r.mother.advance(id).unwrap();
    }
    assert_eq!(r.mother.state(id), Some(MissionState::Done));
    assert_eq!(world.count(), before + 1);
    assert_eq!(unexpected_dialogs(), 0);
}

/// A6-a (Codex R2-2's schedule): Q1 is in its dialog; Q2 is reserved and
/// answered no; then Q1 is answered yes. Q1 is stale: still held.
fn m2c_a6_a_a_superseded_yes_does_not_count(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-a");
    let q1 = HeldOwner::new(true);
    let q2 = Arc::new(HeldOwner::new(false));
    let q2_answer = Arc::new(Mutex::new(None));
    let (mother, q2_owner, seen) = (r.mother.clone(), q2.clone(), q2_answer.clone());
    q1.during.arm(0, move || {
        *seen.lock().unwrap() = Some(resume_with(&mother, id, &q2_owner));
    });
    let q1_answer = resume_with(&r.mother, id, &q1);
    assert_eq!(
        format!("{:?}", q2_answer.lock().unwrap()),
        "Some(Err(Declined))"
    );
    assert_eq!(format!("{q1_answer:?}"), "Err(ResumeStale)");
    assert!(r.mother.held(id).is_some(), "a superseded yes resumed it");
    assert_eq!(resumed(&disk, id), 0);
    let (shown_1, shown_2) = (q1.shown(), q2.shown());
    assert_eq!((shown_1.len(), shown_2.len()), (1, 1));
    assert_ne!(shown_1[0].request, shown_2[0].request);
    // Positive control: a fresh yes resumes.
    assert_eq!(
        resume_with(&r.mother, id, &HeldOwner::new(true)),
        Ok(MissionState::Investigating)
    );
    assert_eq!(resumed(&disk, id), 1);
}

/// A6-b: an answer is taken once. (The broker's approval cannot be copied,
/// so the same request is answered yes twice: the second take is stale.)
fn m2c_a6_b_an_answer_is_taken_once(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-b");
    let ask = resume_request(&r.mother, id).unwrap();
    let first = resume_reply(&r.port, &ask, true);
    let second = resume_reply(&r.port, &ask, true);
    assert_eq!(
        take_resume(&r.mother, id, &ask, first),
        Ok(MissionState::Investigating)
    );
    assert_eq!(r.mother.held(id), None);
    let again = take_resume(&r.mother, id, &ask, second);
    assert_eq!(format!("{again:?}"), "Err(ResumeStale)");
    assert_eq!(resumed(&disk, id), 1, "taken once");
}

/// A6-h (Codex review 1, finding 3): two requests are both answered yes and
/// both wait to be taken. Q1, superseded, is stale and leaves Q2's
/// reservation as it was; Q2's take then resumes the mission, once.
fn m2c_a6_h_a_stale_take_leaves_the_current_request(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-h");
    let q1 = resume_request(&r.mother, id).unwrap();
    let q1_yes = resume_reply(&r.port, &q1, true);
    let q2 = resume_request(&r.mother, id).unwrap();
    let q2_yes = resume_reply(&r.port, &q2, true);
    let stale = take_resume(&r.mother, id, &q1, q1_yes);
    assert_eq!(format!("{stale:?}"), "Err(ResumeStale)");
    assert!(r.mother.held(id).is_some(), "a stale take resumed it");
    assert_eq!(resumed(&disk, id), 0);
    assert_eq!(
        take_resume(&r.mother, id, &q2, q2_yes),
        Ok(MissionState::Investigating),
        "the stale take touched the current request"
    );
    assert_eq!(r.mother.held(id), None);
    assert_eq!(resumed(&disk, id), 1, "exactly one Resumed");
}

/// A6-c: a Stop lands after the broker's record and before the take: the
/// answer is discarded and the mission is Stopped; `Resumed` never written.
fn m2c_a6_c_a_stop_before_the_take_wins(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-c");
    let mother = r.mother.clone();
    after_held_answer(&r.port, move || {
        assert_eq!(mother.stop(id), Ok(MissionState::Stopped));
    });
    let owner = HeldOwner::new(true);
    let answer = resume_with(&r.mother, id, &owner);
    assert_eq!(format!("{answer:?}"), "Err(Stopped)");
    assert_eq!(r.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(resumed(&disk, id), 0);
    assert_eq!(owner.shown().len(), 1);
}

/// A6-d: the journal breaks before the take: refused, still held.
fn m2c_a6_d_a_break_before_the_take_refuses(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-d");
    let (mother, breaking) = (r.mother.clone(), disk.clone());
    after_held_answer(&r.port, move || {
        breaking.fail(true, false);
        assert!(mother.draft(contract_with("")).is_err());
    });
    let owner = HeldOwner::new(true);
    let answer = resume_with(&r.mother, id, &owner);
    assert!(
        format!("{answer:?}").starts_with("Err(Journal("),
        "{answer:?}"
    );
    assert!(r.mother.held(id).is_some());
    disk.fail(false, false);
    assert_eq!(resumed(&disk, id), 0);
    assert_eq!(owner.shown().len(), 1);
}

/// A6-f: under the Owner's emergency stop the held-mission dialog is not
/// raised and the mission stays held. The Owner then lifts the stop.
fn m2c_a6_f_no_resume_under_an_emergency_stop(control: &Arc<GovernedControl>, origin: &str) {
    let (disk, id, r) = held(control, origin, "a6-f");
    control.emergency_stop();
    let owner = HeldOwner::new(true);
    let answer = resume_with(&r.mother, id, &owner);
    assert_eq!(format!("{answer:?}"), "Err(Broker(\"emergency_stopped\"))");
    assert!(owner.shown().is_empty(), "refused before the dialog");
    assert!(r.mother.held(id).is_some());
    assert_eq!(resumed(&disk, id), 0);
    control.resume(&OwnerLiftsTheStop).unwrap();
    assert!(!control.authority().runs().is_stopped());
}

/// A6-g (§11 C3): resume passes the broker's post-record check; another
/// thread fences the port, with no journal failure; the take journals
/// `Resumed` (the Owner's recorded yes). Afterwards no effect runs through
/// that port and the mission makes no effect progress.
fn m2c_a6_g_a_fence_outside_a_break_still_refuses_every_effect(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let (disk, id, r) = held(control, origin, "a6-g");
    let port = r.port.clone();
    after_held_answer(&r.port, move || {
        let fencing = std::thread::spawn(move || {
            use crate::effect::EffectPort;
            port.fence();
        });
        fencing.join().unwrap();
    });
    let before = world.count();
    let owner = HeldOwner::new(true);
    assert_eq!(
        resume_with(&r.mother, id, &owner),
        Ok(MissionState::Investigating)
    );
    assert_eq!(resumed(&disk, id), 1);
    assert_eq!(r.mother.held(id), None);
    assert!(!r.mother.broken(), "no journal failure");
    assert!(r.port.fenced());
    for _ in 0..4 {
        let _ = r.mother.advance(id);
    }
    assert_eq!(world.count(), before, "an effect ran through a fenced port");
    assert!(!world.saw("/m2c-a6-g"));
    assert_eq!(
        r.port.run_of(id),
        None,
        "no run opened through a fenced port"
    );
    assert_ne!(r.mother.state(id), Some(MissionState::Done));
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    let control = &home.control;
    m2c_a4_consent_asks_once_through_the_door(control, origin);
    m2c_a5_resume_asks_the_owner(control, origin, &world);
    m2c_a6_a_a_superseded_yes_does_not_count(control, origin);
    m2c_a6_b_an_answer_is_taken_once(control, origin);
    m2c_a6_h_a_stale_take_leaves_the_current_request(control, origin);
    m2c_a6_c_a_stop_before_the_take_wins(control, origin);
    m2c_a6_d_a_break_before_the_take_refuses(control, origin);
    m2c_a6_g_a_fence_outside_a_break_still_refuses_every_effect(control, origin, &world);
    m2c_a6_f_no_resume_under_an_emergency_stop(control, origin);
    world.stop();
}
