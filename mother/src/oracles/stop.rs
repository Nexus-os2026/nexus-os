//! I4 Stop is honoured from every state; I5 the Owner's emergency stop is
//! Mother's stop.

use super::fake::{commitment_id, Call, Released, Reply};
use super::*;
use crate::coordinator::{CoordinatorError, JournalEvent, MissionState};
use std::time::Duration;

/// Every state a mission can be left in without a call in flight, each
/// reached the way it is reached in use.
fn mission_in(s: &Setup, name: &str) -> MissionId {
    match name {
        "drafted" => s.mother.draft(two_steps()).unwrap(),
        "approved" => s.approved(two_steps()),
        "approved after a step" => {
            let id = s.approved(two_steps());
            s.mother.step(id).unwrap();
            id
        }
        "awaiting approval" => {
            let id = s.approved(two_steps());
            s.broker.reply(Reply::Await(commitment_id(1)));
            s.mother.step(id).unwrap();
            id
        }
        "completed" => {
            let id = s.approved(one_step());
            s.mother.step(id).unwrap();
            id
        }
        "failed" => {
            let id = s.approved(two_steps());
            s.broker.reply(Reply::Refuse("no_covering_grant"));
            let _ = s.mother.step(id);
            id
        }
        "declined" => {
            let id = s.mother.draft(two_steps()).unwrap();
            let _ = s.mother.request_consent(id, &No);
            id
        }
        "expired" => {
            let id = s.approved(two_steps());
            s.clock.advance(Duration::from_millis(TTL_MS));
            let _ = s.mother.step(id);
            id
        }
        "stopped" => {
            let id = s.approved(two_steps());
            s.mother.stop(id).unwrap();
            id
        }
        other => panic!("{other}"),
    }
}

const STATES: &[&str] = &[
    "drafted",
    "approved",
    "approved after a step",
    "awaiting approval",
    "completed",
    "failed",
    "declined",
    "expired",
    "stopped",
];

#[test]
fn m1_i4_stop_is_honoured_from_every_state() {
    for name in STATES {
        let s = setup();
        let id = mission_in(&s, name);
        let before = s.mother.state(id).unwrap();
        let effects = s.broker.effect_calls().len();
        let run = s.mother.run_of(id);

        let after = s.mother.stop(id).expect(name);
        if before.is_terminal() {
            assert_eq!(after, before, "{name}: a terminal mission stays as it ended");
        } else {
            assert_eq!(after, MissionState::Stopped, "{name}");
            if let Some(run) = run {
                assert!(s.broker.is_cancelled(run), "{name}: its run is cancelled");
            }
        }
        assert_eq!(s.mother.state(id), Some(after), "{name}");

        // Nothing starts it again, and no effect follows.
        assert!(s.mother.step(id).is_err(), "{name}");
        assert!(s.mother.request_consent(id, &Yes).is_err(), "{name}");
        let _ = s.mother.refresh(id);
        assert_eq!(s.mother.stop(id), Ok(after), "{name}: stop twice");
        assert_eq!(s.mother.state(id), Some(after), "{name}");
        assert_eq!(
            s.broker.effect_calls().len(),
            effects,
            "{name}: an effect started after Stop"
        );
        if before.is_terminal() {
            assert!(after.is_terminal());
        }
    }
}

#[test]
fn m1_i4_the_state_table_reaches_every_state_but_running() {
    // The table above covers every variant: a variant added later must be
    // added there (Running is driven by the threaded oracles below).
    let s = setup();
    let mut seen = Vec::new();
    for name in STATES {
        let state = s.mother.state(mission_in(&s, name)).unwrap();
        seen.push(std::mem::discriminant(&state));
        let named = match state {
            MissionState::Drafted
            | MissionState::Approved { .. }
            | MissionState::Running { .. }
            | MissionState::AwaitingApproval { .. }
            | MissionState::Completed
            | MissionState::Failed { .. }
            | MissionState::Declined
            | MissionState::Expired
            | MissionState::Stopped => true,
        };
        assert!(named);
    }
    seen.dedup();
    assert_eq!(seen.len(), 8, "every state but Running is reached");
}

/// Stops the mission while it is being asked, then says yes.
struct StopsThenYes<'a> {
    s: &'a Setup,
    id: std::sync::Mutex<Option<MissionId>>,
}

impl OwnerConsent for StopsThenYes<'_> {
    fn confirm_mission(&self, _: &crate::coordinator::ConsentRequest) -> bool {
        let id = self.id.lock().unwrap().expect("id");
        // No coordinator lock is held while the Owner is asked.
        assert_eq!(self.s.mother.stop(id), Ok(MissionState::Stopped));
        true
    }
}

#[test]
fn m1_i4_a_stop_while_the_owner_is_asked_wins_over_a_yes() {
    let s = setup();
    let id = s.mother.draft(two_steps()).unwrap();
    let owner = StopsThenYes {
        s: &s,
        id: std::sync::Mutex::new(Some(id)),
    };
    assert_eq!(
        s.mother.request_consent(id, &owner),
        Err(CoordinatorError::Stopped)
    );
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    assert!(s.mother.step(id).is_err());
    assert_eq!(s.broker.calls(), vec![]);
    assert!(s
        .mother
        .journal()
        .iter()
        .any(|entry| entry.mission == id && entry.event == JournalEvent::ConsentOvertaken));
}

#[test]
fn m1_i4_stop_cancels_an_effect_in_flight_without_waiting_for_it() {
    let s = setup();
    let id = s.approved(two_steps());
    s.broker.hold_act();
    let mother = s.mother.clone();
    let stepping = std::thread::spawn(move || mother.step(id));
    assert!(s.broker.wait_entered_act(), "the step reached the broker");
    assert_eq!(s.mother.state(id), Some(MissionState::Running { step: 0 }));

    // The action holds until its run is cancelled: Stop must not wait for
    // it, and must cancel the run.
    assert_eq!(s.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(stepping.join().unwrap(), Err(CoordinatorError::Stopped));
    assert_eq!(
        s.broker.released(),
        vec![Released::ByEvent],
        "the effect ended because Stop cancelled its run"
    );
    let run = s.mother.run_of(id).unwrap();
    assert!(s.broker.is_cancelled(run));
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    assert!(s
        .mother
        .journal()
        .iter()
        .any(|entry| entry.event == JournalEvent::StepOvertaken { step: 0 }));
    assert!(s.mother.step(id).is_err());
    assert_eq!(s.broker.acts(), 1);
}

#[test]
fn m1_i4_a_stop_while_the_run_opens_cancels_the_new_run() {
    let s = setup();
    let id = s.approved(two_steps());
    s.broker.hold_open();
    let mother = s.mother.clone();
    let stepping = std::thread::spawn(move || mother.step(id));
    assert!(s.broker.wait_entered_open());
    assert_eq!(s.mother.stop(id), Ok(MissionState::Stopped));
    s.broker.release_open();
    assert_eq!(stepping.join().unwrap(), Err(CoordinatorError::Stopped));
    assert_eq!(s.broker.released(), vec![Released::ByEvent]);
    let calls = s.broker.calls();
    let opened = super::fake::run_id(1);
    assert!(calls.contains(&Call::Cancel(opened)), "{calls:?}");
    assert_eq!(s.broker.acts(), 0);
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
}

#[test]
fn m1_i4_stop_all_stops_every_mission_that_has_not_ended() {
    let s = setup();
    let drafted = mission_in(&s, "drafted");
    let approved = mission_in(&s, "approved after a step");
    let waiting = mission_in(&s, "awaiting approval");
    let completed = mission_in(&s, "completed");
    let effects = s.broker.effect_calls().len();
    assert_eq!(s.mother.stop_all(), 3);
    for id in [drafted, approved, waiting] {
        assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    }
    assert_eq!(s.mother.state(completed), Some(MissionState::Completed));
    for id in [approved, waiting] {
        assert!(s.broker.is_cancelled(s.mother.run_of(id).unwrap()));
    }
    assert_eq!(s.mother.stop_all(), 0);
    assert_eq!(s.broker.effect_calls().len(), effects);
}

#[test]
fn m1_i5_the_owner_s_emergency_stop_stops_a_mission_before_its_first_step() {
    let s = setup();
    let id = s.approved(two_steps());
    s.broker.set_emergency(true);
    assert_eq!(s.mother.step(id), Err(CoordinatorError::Stopped));
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(s.broker.effect_calls(), vec![]);
    // Lifting the broker's stop does not restart Mother's mission.
    s.broker.set_emergency(false);
    assert!(s.mother.step(id).is_err());
    assert_eq!(s.broker.effect_calls(), vec![]);
}

#[test]
fn m1_i5_the_owner_s_emergency_stop_stops_a_mission_with_an_open_run() {
    let s = setup();
    let id = mission_in(&s, "approved after a step");
    let run = s.mother.run_of(id).unwrap();
    s.broker.set_emergency(true);
    assert_eq!(s.mother.step(id), Err(CoordinatorError::Stopped));
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    assert!(s.broker.is_cancelled(run));
    assert_eq!(s.broker.acts(), 1);
}

#[test]
fn m1_i5_an_emergency_stop_refusal_stops_rather_than_fails() {
    let s = setup();
    let id = s.approved(two_steps());
    s.broker.reply(Reply::Refuse("emergency_stopped"));
    assert_eq!(s.mother.step(id), Err(CoordinatorError::Stopped));
    assert_eq!(s.mother.state(id), Some(MissionState::Stopped));
    assert!(s.broker.is_cancelled(s.mother.run_of(id).unwrap()));
}

#[test]
fn m1_i5_an_emergency_stop_while_awaiting_approval_stops_on_refresh() {
    let s = setup();
    let id = mission_in(&s, "awaiting approval");
    s.broker.set_emergency(true);
    assert_eq!(s.mother.refresh(id), Ok(MissionState::Stopped));
    assert!(s.broker.is_cancelled(s.mother.run_of(id).unwrap()));
}
