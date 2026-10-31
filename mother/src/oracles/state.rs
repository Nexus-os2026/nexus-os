//! The state machine: one deterministic table, written out here
//! independently of the implementation.

use super::*;
use crate::contract::ContractHash;
use crate::journal::Event;
use crate::state::{transition, Trigger};
use MissionState::*;

fn triggers() -> Vec<Trigger> {
    let hash = ContractHash::from_hex(&"ab".repeat(32)).unwrap();
    vec![
        Trigger::Revise {
            contract: contract(),
        },
        Trigger::Submit { hash },
        Trigger::Consent {
            hash,
            at_wall_ms: 0,
        },
        Trigger::Deny { hash },
        Trigger::Start,
        Trigger::PhaseDone,
        Trigger::VerifyPass,
        Trigger::VerifyFail { reason: "r".into() },
        Trigger::Block { reason: "r".into() },
        Trigger::Fail { reason: "r".into() },
        Trigger::Stop { reason: "r".into() },
    ]
}

/// The table, as the mission states it.
fn expected(from: MissionState, trigger: &str) -> Option<MissionState> {
    let working = [Investigating, Building, Verifying, Explaining];
    match (trigger, from) {
        ("revise", Draft | AwaitingConsent | Approved) => Some(Draft),
        ("submit", Draft) => Some(AwaitingConsent),
        ("consent", AwaitingConsent) => Some(Approved),
        ("deny", AwaitingConsent) => Some(Denied),
        ("start", Approved) => Some(Investigating),
        ("phase_done", Investigating) => Some(Building),
        ("phase_done", Building) => Some(Verifying),
        ("phase_done", Explaining) => Some(Done),
        ("verify_pass", Verifying) => Some(Explaining),
        ("verify_fail", Verifying) => Some(Failed),
        ("block", s) if s == Approved || working.contains(&s) => Some(Blocked),
        ("fail", s) if working.contains(&s) => Some(Failed),
        ("stop", s) if !matches!(s, Done | Stopped | Denied | Failed | Blocked) => Some(Stopped),
        _ => None,
    }
}

#[test]
fn state_the_transition_table_is_exactly_the_mission_s() {
    for from in MissionState::ALL {
        for trigger in triggers() {
            let got = transition(from, &trigger).ok();
            assert_eq!(
                got,
                expected(from, trigger.name()),
                "{from:?} --{}-->",
                trigger.name()
            );
            // Deterministic.
            assert_eq!(transition(from, &trigger).ok(), got);
            if let Err(e) = transition(from, &trigger) {
                assert_eq!(e.from, from);
                assert_eq!(e.trigger, trigger.name());
            }
        }
    }
}

#[test]
fn state_terminal_states_accept_nothing() {
    for from in MissionState::ALL {
        let terminal = matches!(from, Done | Stopped | Denied | Failed | Blocked);
        assert_eq!(from.is_terminal(), terminal, "{from:?}");
        assert_eq!(
            from.is_working(),
            matches!(from, Investigating | Building | Verifying | Explaining),
            "{from:?}"
        );
        if terminal {
            for trigger in triggers() {
                assert!(transition(from, &trigger).is_err(), "{from:?}");
            }
        }
    }
}

#[test]
fn state_a_mission_walks_every_state_in_order() {
    let w = world();
    let id = w.mother.draft(contract()).unwrap();
    let mut seen = vec![w.mother.state(id).unwrap()];
    w.mother.submit(id).unwrap();
    seen.push(w.mother.state(id).unwrap());
    w.mother.request_consent(id, &Yes).unwrap();
    seen.push(w.mother.state(id).unwrap());
    let mut explanation = None;
    for _ in 0..5 {
        let a = w.mother.advance(id).unwrap();
        assert_eq!(a.from, *seen.last().unwrap());
        seen.push(a.to);
        explanation = a.explanation.or(explanation);
    }
    assert_eq!(
        seen,
        vec![
            Draft,
            AwaitingConsent,
            Approved,
            Investigating,
            Building,
            Verifying,
            Explaining,
            Done
        ]
    );
    let explanation = explanation.expect("Mother explains the result");
    assert!(explanation.contains("Rosa's"), "{explanation}");
    assert!(
        explanation.contains("The page names the bakery"),
        "{explanation}"
    );
    // Every transition is journaled, in order.
    let triggers: Vec<&'static str> = w
        .mother
        .journal()
        .unwrap()
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transition { trigger } => Some(trigger.name()),
            _ => None,
        })
        .collect();
    assert_eq!(
        triggers,
        vec![
            "submit",
            "consent",
            "start",
            "phase_done",
            "phase_done",
            "verify_pass",
            "phase_done"
        ]
    );
    assert_eq!(w.port.calls(), vec![PortCall::Finish(id)]);
}

#[test]
fn state_a_restart_resumes_every_state_where_it_was() {
    for state in MissionState::ALL {
        let w = world();
        let id = w.mission_in(state);
        let w2 = w.restart();
        assert_eq!(w2.mother.state(id), Some(state), "{state:?}");
        assert_eq!(w2.mother.contract(id), w.mother.contract(id), "{state:?}");
        assert_eq!(
            w2.mother.consented_hash(id),
            w.mother.consented_hash(id),
            "{state:?}"
        );
        assert_eq!(w2.mother.reason(id), w.mother.reason(id), "{state:?}");
    }
}

#[test]
fn state_a_restarted_mission_carries_on_to_done() {
    let w = world();
    let id = w.mission_in(Building);
    let w = w.restart();
    for _ in 0..3 {
        w.mother.advance(id).unwrap();
    }
    assert_eq!(w.mother.state(id), Some(Done));
    // New missions after a restart get fresh ids.
    let next = w.mother.draft(contract()).unwrap();
    assert!(next > id);
}

#[test]
fn state_a_mission_is_worked_on_by_one_call_at_a_time() {
    let held = Arc::new(HeldLane::new(LaneRole::ResearcherA, vec![]));
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
    assert!(held.wait_entered());
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Busy));
    held.release();
    assert_eq!(working.join().unwrap().unwrap().to, Building);
}
