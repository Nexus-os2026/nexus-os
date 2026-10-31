//! Stop from every state: no new dispatch, and the state is durable across
//! restart. Stop never waits for a lane or an effect in flight. The Owner's
//! emergency stop is Mother's stop.

use super::*;
use crate::journal::Event;
use crate::state::Trigger;

#[test]
fn stop_from_every_state_dispatches_nothing_more_and_survives_restart() {
    for state in MissionState::ALL {
        // Lanes that would ask for an effect in every working state.
        let w = world();
        let id = w.mission_in(state);
        let effects = w.port.effects().len();
        let lanes = w.keyed_calls();

        let after = w
            .mother
            .stop(id)
            .expect("stop never fails for a known mission");
        if state.is_terminal() {
            assert_eq!(
                after, state,
                "{state:?}: a terminal mission stays as it ended"
            );
        } else {
            assert_eq!(after, MissionState::Stopped, "{state:?}");
        }
        assert_eq!(w.mother.state(id), Some(after), "{state:?}");
        assert_eq!(w.mother.stop(id), Ok(after), "{state:?}: stop twice");

        // Nothing starts it again.
        let refuse_everything = |w: &World| {
            for _ in 0..6 {
                assert!(w.mother.advance(id).is_err(), "{state:?}");
            }
            assert!(w.mother.submit(id).is_err(), "{state:?}");
            assert!(w.mother.request_consent(id, &Yes).is_err(), "{state:?}");
            assert!(w.mother.revise(id, contract()).is_err(), "{state:?}");
        };
        refuse_everything(&w);

        // Durable: a restarted Mother finds it as it was left.
        let w = w.restart();
        assert_eq!(w.mother.state(id), Some(after), "{state:?} after restart");
        refuse_everything(&w);
        assert_eq!(w.mother.stop(id), Ok(after), "{state:?}");

        assert_eq!(
            w.port.effects().len(),
            effects,
            "{state:?}: dispatch after Stop"
        );
        assert_eq!(w.keyed_calls(), lanes, "{state:?}: a lane ran after Stop");
    }
}

#[test]
fn stop_is_journaled_before_it_returns() {
    let w = world();
    let id = w.mission_in(MissionState::Building);
    w.mother.stop(id).unwrap();
    let last = w
        .mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .last()
        .unwrap();
    assert!(
        matches!(
            last.event,
            Event::Transition {
                trigger: Trigger::Stop { .. }
            }
        ),
        "{last:?}"
    );
    // The lines are in the store, not only in memory.
    assert!(w.journal.lines().last().unwrap().contains("\"stop\""));
}

#[test]
fn stop_cancels_the_mission_at_the_port_once_it_may_have_dispatched() {
    let w = world();
    let id = w.mission_in(MissionState::Building);
    w.mother.stop(id).unwrap();
    assert!(w.port.is_cancelled(id));
    // Before consent there is nothing to cancel and the port is not reached.
    let draft = w.mission_in(MissionState::AwaitingConsent);
    w.mother.stop(draft).unwrap();
    assert!(!w.port.calls().contains(&PortCall::Cancel(draft)));
}

#[test]
fn stop_does_not_wait_for_a_lane_in_flight_and_its_answer_is_dropped() {
    let held = Arc::new(HeldLane::new(
        LaneRole::ResearcherA,
        vec![effect_of(EffectKind::Network)],
    ));
    let mut lanes: Vec<Arc<dyn Lane>> = vec![held.clone()];
    lanes.extend(
        keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::ResearcherA),
    );
    let w = world_with(lanes);
    let id = w.approved(contract());
    w.mother.advance(id).unwrap();

    let mother = w.mother.clone();
    let working = std::thread::spawn(move || mother.advance(id));
    assert!(held.wait_entered(), "the lane was called");
    // The lane is still inside its call: Stop must not wait for it.
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    held.release();
    assert_eq!(working.join().unwrap(), Err(CoordinatorError::Stopped));

    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(
        w.port.effects(),
        vec![],
        "the late answer's effect was dispatched"
    );
    let journal = w.mother.journal().unwrap();
    assert!(journal
        .iter()
        .any(|e| matches!(e.event, Event::LaneOvertaken { .. })));
    // No lane after the held one was called.
    let invoked: Vec<_> = journal
        .iter()
        .filter_map(|e| match &e.event {
            Event::LaneInvoked { identity, .. } => Some(identity.role),
            _ => None,
        })
        .collect();
    assert_eq!(invoked, vec![LaneRole::Sentinel, LaneRole::ResearcherA]);
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
}

#[test]
fn stop_cancels_an_effect_in_flight_without_waiting_and_dispatches_no_more() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.hold();
    let mother = w.mother.clone();
    let working = std::thread::spawn(move || mother.advance(id));
    assert!(w.port.wait_entered(), "the effect reached the port");
    // The effect holds until its mission is cancelled: Stop must not wait
    // for it, and must cancel it.
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(working.join().unwrap(), Err(CoordinatorError::Stopped));
    assert_eq!(w.port.released_by_event(), vec![true]);
    assert_eq!(
        w.port.effects(),
        vec![(id, EffectKind::Network)],
        "the second effect was dispatched after Stop"
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects().len(), 1);
}

#[test]
fn stop_all_stops_every_mission_that_has_not_ended() {
    let w = world();
    let draft = w.mission_in(MissionState::Draft);
    let approved = w.mission_in(MissionState::Approved);
    let building = w.mission_in(MissionState::Building);
    let done = w.mission_in(MissionState::Done);
    let effects = w.port.effects().len();
    assert_eq!(w.mother.stop_all(), 3);
    for id in [draft, approved, building] {
        assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    }
    assert_eq!(w.mother.state(done), Some(MissionState::Done));
    assert_eq!(w.mother.stop_all(), 0);
    let w = w.restart();
    for id in [draft, approved, building] {
        assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
        assert!(w.mother.advance(id).is_err());
    }
    assert_eq!(w.port.effects().len(), effects);
}

#[test]
fn stop_the_owner_s_emergency_stop_stops_the_mission_before_any_lane() {
    let w = world();
    let id = w.mission_in(MissionState::Investigating);
    let lanes = w.keyed_calls();
    w.port.set_emergency(true);
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(w.keyed_calls(), lanes);
    // Lifting the broker's stop does not restart Mother's mission.
    w.port.set_emergency(false);
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects(), vec![]);
}

/// A Builder that asks for an effect and, while it answers, the Owner's
/// emergency stop engages.
struct StopsWhileBuilding {
    port: std::sync::OnceLock<Arc<RecordingPort>>,
}

impl Lane for StopsWhileBuilding {
    fn identity(&self) -> crate::lane::LaneIdentity {
        crate::lane::LaneIdentity {
            role: LaneRole::Builder,
            name: "stops-while-building".into(),
            version: "m1-test".into(),
        }
    }

    fn invoke(&self, _: &crate::lane::LaneInput) -> crate::lane::LaneOutput {
        self.port.get().expect("port").set_emergency(true);
        crate::lane::LaneOutput {
            text: "built".into(),
            effects: vec![effect_of(EffectKind::Network)],
            verdict: Verdict::Continue,
        }
    }
}

#[test]
fn stop_the_owner_s_emergency_stop_is_checked_before_every_effect() {
    let builder = Arc::new(StopsWhileBuilding {
        port: std::sync::OnceLock::new(),
    });
    let mut lanes: Vec<Arc<dyn Lane>> = vec![builder.clone()];
    lanes.extend(
        keyed_lanes()
            .into_iter()
            .filter(|l| l.identity().role != LaneRole::Builder),
    );
    let w = world_with(lanes);
    builder.port.set(w.port.clone()).ok().unwrap();
    let id = w.approved(contract());
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    // The stop engages inside the Builder's call, after the coordinator's
    // own check at the start of the phase: the Builder's effect must not
    // reach the port.
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(w.port.effects(), vec![]);
}
