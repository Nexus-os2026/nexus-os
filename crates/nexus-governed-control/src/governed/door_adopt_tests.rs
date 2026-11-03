//! M2d-C round 5 (decision D4(a) with C1; pre-registered before the
//! repair): a door becomes a member of a run (an adoption) only while no
//! commitment of that run is authorized, handed off or executing and no
//! Files obligation of it is unacknowledged, decided atomically against
//! authorization; otherwise the refusal changes nothing. An adoption
//! attaches the adopting mission's halt cell in the same hold, so every
//! later execution token carries it (the cell-less, pre-mission adoption).
//! Such runs are reachable through ack-only ownership, which may cancel
//! them (only narrowing). A child of the M2c door oracles.

use super::*;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, GrantConfirmation, ResumeConfirmation,
};

/// A confirmer whose action dialog runs `during` first, then says yes.
struct During(Mutex<Option<Box<dyn FnOnce() + Send>>>);

impl ControlConfirmer for During {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        if let Some(during) = self.0.lock().unwrap().take() {
            during();
        }
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
}

/// The parent's waiting POST (named apart from the bindings below).
fn waiting_on(f: &Fixture, door: &crate::governed::AgentDoor, run: RunId, path: &str) -> CommitmentId {
    waiting(f, door, run, path)
}

fn adoptable(f: &Fixture) -> (crate::governed::AgentDoor, crate::governed::AgentDoor, RunId, CommitmentId) {
    let owner = Owner::new(true, true, true);
    let first = f.door_with(&owner);
    let run = first.open_run().unwrap();
    let waiting = waiting(f, &first, run, "/r5-adopt");
    let second = f.door_with(&owner);
    (first, second, run, waiting)
}

/// D4(a): another commitment of the run authorized: the adoption is
/// refused and changes nothing (no membership, no view). Control: a run
/// with only a waiting, unauthorized action is adopted as before.
#[test]
fn m2d_c_r5_no_adoption_over_an_authorized_commitment() {
    let f = Fixture::new();
    let (first, second, run, waiting) = adoptable(&f);
    let other = waiting_on(&f, &first, run, "/r5-other");
    f.control
        .authorize(other, &mother(), run, &Yes::new(true))
        .unwrap();
    let cell = second.halt_cells().cell("journal", 1).unwrap();
    assert!(second.adopt_with_halt(run, waiting, &cell).is_err());
    assert!(matches!(second.commitment(waiting), Lookup::NotMine { .. }));

    let f = Fixture::new();
    let (_first, second, run, waiting) = adoptable(&f);
    let cell = second.halt_cells().cell("journal", 1).unwrap();
    second
        .adopt_with_halt(run, waiting, &cell)
        .expect("a waiting, unauthorized action is adopted as before");
    assert!(matches!(second.commitment(waiting), Lookup::Mine(_)));
}

/// D4(a) and C1 (the cell-less, pre-mission adoption): a run opened
/// without a cell, adopted with the mission's cell: once the cell is
/// engaged, nothing of the run is authorized.
#[test]
fn m2d_c_r5_an_adoption_attaches_the_mission_s_cell() {
    let f = Fixture::new();
    let (_first, second, run, waiting) = adoptable(&f);
    let cell = second.halt_cells().cell("journal", 2).unwrap();
    second.adopt_with_halt(run, waiting, &cell).unwrap();
    cell.engage();
    assert!(f
        .control
        .authorize(waiting, &mother(), run, &Yes::new(true))
        .is_err());
    assert_eq!(f.reached("/r5-adopt"), 0);
}

/// D4(a), concurrently: an adoption during the Owner's approval dialog is
/// decided against the authorization atomically: if it is taken, the
/// cell it attached reaches the execution that follows.
#[test]
fn m2d_c_r5_adoption_and_authorization_are_ordered() {
    let f = Fixture::new();
    let (_first, second, run, waiting) = adoptable(&f);
    let cell = second.halt_cells().cell("journal", 3).unwrap();
    let adopted: Arc<Mutex<Option<bool>>> = Arc::default();
    let (seen, door, attach) = (adopted.clone(), Arc::new(second), cell.clone());
    let adopting = door.clone();
    let owner = During(Mutex::new(Some(Box::new(move || {
        *seen.lock().unwrap() = Some(adopting.adopt_with_halt(run, waiting, &attach).is_ok());
    }))));
    f.control
        .authorize(waiting, &mother(), run, &owner)
        .unwrap();
    assert_eq!(*adopted.lock().unwrap(), Some(true), "taken before it");
    cell.engage();
    assert!(f.control.execute(waiting, &mother(), run).is_err());
    assert_eq!(f.reached("/r5-adopt"), 0, "the cell reached the execution");
}

/// D4(a): ack-only ownership cancels: once the run's door is gone, a
/// door of the same agent may cancel it (its waiting action is revoked)
/// and nothing more; while its door lives, and for another agent, no.
#[test]
fn m2d_c_r5_ack_only_ownership_may_cancel() {
    let f = Fixture::new();
    let (first, second, run, waiting) = adoptable(&f);
    assert!(second.cancel_orphan(run).is_err(), "its door lives");
    let foreign = f
        .control
        .issue_agent_door(AgentId::new("other").unwrap(), Owner::new(true, true, true))
        .unwrap();
    drop(first);
    assert!(foreign.cancel_orphan(run).is_err(), "another agent's");
    second.cancel_orphan(run).unwrap().write();
    assert!(!f.live(run));
    assert!(matches!(
        second.commitment(waiting),
        Lookup::NotMine { settled: true }
    ));
}
