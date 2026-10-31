//! M2a: Mother waits for the broker's approval. When an effect needs the
//! Owner's native approval the mission waits (AwaitingApproval):
//!
//! - approve: the approved action is never sent again and the mission
//!   continues exactly once, from where it paused;
//! - deny: zero further effect, the mission is Denied;
//! - expiry: zero further effect, the mission is Blocked;
//! - Stop while waiting: the pending action is withdrawn at the port, zero
//!   effect, the mission is Stopped, durably;
//! - restart while waiting: the mission is still waiting; nothing is sent
//!   again and nothing is approved by itself.

use super::*;
use crate::journal::{Entry, Event, JournalError, GENESIS};
use crate::state::{transition, Trigger};

/// A mission of `markers` advanced until the Builder's first effect waits.
fn waiting(w: &World, markers: &str) -> MissionId {
    let id = w.approved(contract_with(markers));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    let built = w.mother.advance(id).unwrap();
    assert_eq!(built.from, MissionState::Building);
    assert_eq!(built.to, MissionState::AwaitingApproval);
    id
}

const BOTH: &str = "ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS";

fn network_sent(w: &World, id: MissionId) -> usize {
    w.port
        .effects()
        .iter()
        .filter(|e| **e == (id, EffectKind::Network))
        .count()
}

fn calls_of(w: &World, role: LaneRole) -> usize {
    w.keyed
        .iter()
        .find(|l| l.role == role)
        .unwrap()
        .calls
        .load(Ordering::SeqCst)
}

// ---------------------------------------------------------------- table

#[test]
fn m2a_the_waiting_transitions_are_exactly_these() {
    use MissionState::*;
    let paused = [Investigating, Building, Verifying];
    let triggers = |phase: MissionState| {
        vec![
            Trigger::AwaitApproval {
                effect: 0,
                rest: vec![],
                roles_left: vec![],
            },
            Trigger::ActionApproved { effect: 0, phase },
            Trigger::ActionDenied { effect: 0 },
            Trigger::ApprovalLapsed {
                effect: 0,
                reason: "approval_expired".into(),
            },
        ]
    };
    for from in MissionState::ALL {
        for phase in MissionState::ALL {
            for trigger in triggers(phase) {
                let expected = match (&trigger, from) {
                    (Trigger::AwaitApproval { .. }, s) if paused.contains(&s) => Some(AwaitingApproval),
                    (Trigger::ActionApproved { .. }, AwaitingApproval) if paused.contains(&phase) => {
                        Some(phase)
                    }
                    (Trigger::ActionDenied { .. }, AwaitingApproval) => Some(Denied),
                    (Trigger::ApprovalLapsed { .. }, AwaitingApproval) => Some(Blocked),
                    _ => None,
                };
                assert_eq!(
                    transition(from, &trigger).ok(),
                    expected,
                    "{from:?} --{}({phase:?})-->",
                    trigger.name()
                );
            }
        }
    }
    // Waiting is neither terminal nor working (no lane runs), and Stop
    // leaves it.
    assert!(!AwaitingApproval.is_terminal());
    assert!(!AwaitingApproval.is_working());
    assert_eq!(
        transition(AwaitingApproval, &Trigger::Stop { reason: "owner".into() }),
        Ok(Stopped)
    );
    assert_eq!(AwaitingApproval.as_str(), "awaiting_approval");
    assert!(MissionState::ALL.contains(&AwaitingApproval));
}

// ---------------------------------------------------------------- approve

#[test]
fn m2a_waiting_sends_nothing_more_and_runs_no_lane() {
    let w = world();
    let id = waiting(&w, BOTH);
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    let lanes = w.keyed_calls();
    for _ in 0..3 {
        let a = w.mother.advance(id).unwrap();
        assert_eq!(a.from, MissionState::AwaitingApproval);
        assert_eq!(a.to, MissionState::AwaitingApproval);
        assert!(a.effects.is_empty() && a.lanes.is_empty());
    }
    assert_eq!(w.port.approval_queries(), 3);
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    assert_eq!(w.keyed_calls(), lanes);
    assert!(!w.port.is_cancelled(id));
}

#[test]
fn m2a_approve_continues_exactly_once_from_where_it_paused() {
    let w = world();
    let id = waiting(&w, BOTH);
    w.port.approval_reply(ApprovalStatus::Approved);
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.from, MissionState::AwaitingApproval);
    // The rest of the Builder's answer is sent, then the phase ends.
    assert_eq!(a.to, MissionState::Verifying);
    assert_eq!(
        w.port.effects(),
        vec![(id, EffectKind::Network), (id, EffectKind::Process)]
    );
    assert_eq!(network_sent(&w, id), 1, "the approved action is never re-sent");
    assert_eq!(w.port.approval_queries(), 1);
    // Exactly once: an approval answered again is never asked for or used.
    w.port.approval_reply(ApprovalStatus::Approved);
    let builder = calls_of(&w, LaneRole::Builder);
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::Done));
    assert_eq!(w.port.approval_queries(), 1);
    assert_eq!(calls_of(&w, LaneRole::Builder), builder);
    assert_eq!(network_sent(&w, id), 1);
    assert_eq!(w.port.effects().len(), 2);
    let triggers: Vec<&'static str> = w
        .mother
        .journal()
        .unwrap()
        .iter()
        .filter(|e| e.mission == id)
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
            "await_approval",
            "action_approved",
            "phase_done",
            "verify_pass",
            "phase_done"
        ]
    );
}

#[test]
fn m2a_approve_runs_the_phase_s_remaining_lanes_once() {
    let w = world();
    let id = w.approved(contract_with("ASK-RESEARCHERA-NETWORK"));
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    assert_eq!(calls_of(&w, LaneRole::ResearcherA), 1);
    assert_eq!(calls_of(&w, LaneRole::ResearcherB), 0);
    assert_eq!(calls_of(&w, LaneRole::Skeptic), 0);
    w.port.approval_reply(ApprovalStatus::Approved);
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Building);
    assert_eq!(calls_of(&w, LaneRole::Sentinel), 1);
    assert_eq!(calls_of(&w, LaneRole::ResearcherA), 1);
    assert_eq!(calls_of(&w, LaneRole::ResearcherB), 1);
    assert_eq!(calls_of(&w, LaneRole::Skeptic), 1);
    assert_eq!(network_sent(&w, id), 1);
}

// ---------------------------------------------------------------- deny, expiry

#[test]
fn m2a_deny_while_waiting_means_zero_effect_and_denied() {
    let w = world();
    let id = waiting(&w, BOTH);
    let lanes = w.keyed_calls();
    w.port.approval_reply(ApprovalStatus::Denied);
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Denied);
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert!(w.port.is_cancelled(id));
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    assert_eq!(w.keyed_calls(), lanes);
    assert!(w.mother.advance(id).is_err());
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects().len(), 1);
}

#[test]
fn m2a_expiry_while_waiting_means_zero_effect_and_blocked() {
    let w = world();
    let id = waiting(&w, BOTH);
    let lanes = w.keyed_calls();
    w.port.approval_reply(ApprovalStatus::Expired);
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("approval_expired"));
    assert!(w.port.is_cancelled(id));
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    assert_eq!(w.keyed_calls(), lanes);
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked));
    assert_eq!(w.mother.reason(id).as_deref(), Some("approval_expired"));
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects().len(), 1);
}

#[test]
fn m2a_an_action_that_lapsed_otherwise_blocks_with_zero_effect() {
    for class in ["revoked", "failed", "approval_outcome_unknown"] {
        let w = world();
        let id = waiting(&w, BOTH);
        w.port.approval_reply(ApprovalStatus::Lapsed {
            class: class.into(),
        });
        assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
        assert_eq!(
            w.mother.reason(id),
            Some(format!("approval_lapsed:{class}")),
            "{class}"
        );
        assert!(w.port.is_cancelled(id));
        assert_eq!(w.port.effects().len(), 1, "{class}");
    }
}

#[test]
fn m2a_mother_s_time_budget_ends_the_wait_with_zero_effect() {
    let w = world();
    let id = waiting(&w, BOTH);
    w.clock.advance(Duration::from_millis(3_600_000));
    w.port.approval_reply(ApprovalStatus::Approved);
    let a = w.mother.advance(id).unwrap();
    assert_eq!(a.to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_time"));
    assert!(w.port.is_cancelled(id));
    assert_eq!(w.port.effects().len(), 1);
}

// ---------------------------------------------------------------- stop

#[test]
fn m2a_stop_while_waiting_withdraws_the_action_durably() {
    let w = world();
    let id = waiting(&w, BOTH);
    let lanes = w.keyed_calls();
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert!(w.port.is_cancelled(id), "the pending action is withdrawn");
    // An approval that comes afterwards changes nothing.
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    assert_eq!(w.keyed_calls(), lanes);
    assert_eq!(w.port.approval_queries(), 0);
}

#[test]
fn m2a_stop_does_not_wait_for_an_approval_query_and_its_answer_is_dropped() {
    let w = world();
    let id = waiting(&w, BOTH);
    w.port.hold_approval();
    w.port.approval_reply(ApprovalStatus::Approved);
    let mother = w.mother.clone();
    let asking = std::thread::spawn(move || mother.advance(id));
    assert!(w.port.wait_approval_entered(), "the approval was queried");
    // One resolution at a time.
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Busy));
    // Stop while the query is in flight: it does not wait for it.
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(asking.join().unwrap(), Err(CoordinatorError::Stopped));
    assert_eq!(w.port.released_by_event(), vec![true]);
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(w.port.effects().len(), 1);
}

#[test]
fn m2a_the_owner_s_emergency_stop_while_waiting_stops_the_mission() {
    let w = world();
    let id = waiting(&w, BOTH);
    w.port.set_emergency(true);
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(w.port.is_cancelled(id));
    assert_eq!(w.port.effects().len(), 1);
}

// ---------------------------------------------------------------- restart

#[test]
fn m2a_restart_while_waiting_still_waits_and_never_resends_or_approves() {
    let w = world();
    let id = waiting(&w, BOTH);
    let calls = w.port.calls();
    let w = w.restart();
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    assert_eq!(w.mother.reason(id), None);
    assert_eq!(w.port.calls(), calls, "opening sent, asked or cancelled");
    // Still waiting until the broker says otherwise.
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    assert_eq!(network_sent(&w, id), 1);
    // Approved after the restart: the rest continues once; nothing is
    // sent again.
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Verifying);
    assert_eq!(
        w.port.effects(),
        vec![(id, EffectKind::Network), (id, EffectKind::Process)]
    );
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::Done));
    assert_eq!(network_sent(&w, id), 1);
}

#[test]
fn m2a_a_journal_failure_on_entering_the_wait_withdraws_the_action() {
    let w = world();
    let id = w.approved(contract_with(BOTH));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    // Building: Sentinel (2 entries), Builder (2), dispatch, settle; the
    // wait after that fails.
    w.journal.fail_after(6);
    assert!(matches!(
        w.mother.advance(id),
        Err(CoordinatorError::Journal(_))
    ));
    assert!(w.port.is_cancelled(id));
    assert!(w.mother.state(id).unwrap().is_terminal());
    assert_eq!(w.port.effects().len(), 1);
    assert!(matches!(
        w.reopen(),
        Err(CoordinatorError::Journal(JournalError::Poisoned))
    ));
}

// ---------------------------------------------------------------- replay

fn chained(events: Vec<Event>) -> Vec<String> {
    let mut prev = GENESIS.to_string();
    events
        .into_iter()
        .enumerate()
        .map(|(seq, event)| {
            let entry = Entry::chained(seq as u64, &prev, MissionId(1), event);
            prev = entry.hash.clone();
            entry.to_line()
        })
        .collect()
}

/// A mission journaled up to an Investigating effect that awaits approval.
fn waited() -> Vec<Event> {
    let c = contract();
    vec![
        Event::Drafted {
            contract: c.clone(),
        },
        Event::Transition {
            trigger: Trigger::Submit { hash: c.hash() },
        },
        Event::Transition {
            trigger: Trigger::Consent {
                hash: c.hash(),
                at_wall_ms: 0,
            },
        },
        Event::Transition {
            trigger: Trigger::Start,
        },
        Event::EffectDispatched {
            effect: 0,
            kind: EffectKind::Network,
            request_hash: "0".repeat(64),
        },
        Event::EffectSettled {
            effect: 0,
            outcome: "awaiting_approval".into(),
        },
        Event::Transition {
            trigger: Trigger::AwaitApproval {
                effect: 0,
                rest: vec![],
                roles_left: vec![],
            },
        },
    ]
}

fn opens(w: &World, events: Vec<Event>) -> Result<Coordinator<RecordingPort>, CoordinatorError> {
    w.journal.set_lines(chained(events));
    w.reopen()
}

#[test]
fn m2a_replay_accepts_a_wait_only_where_the_live_coordinator_writes_one() {
    let w = world();
    // The positive control: a journaled wait reopens waiting.
    let mother = opens(&w, waited()).unwrap();
    assert_eq!(
        mother.state(MissionId(1)),
        Some(MissionState::AwaitingApproval)
    );
    drop(mother);

    // A wait with no effect settled as awaiting approval.
    let mut no_settle = waited();
    no_settle.remove(5);
    no_settle.remove(4);
    assert!(matches!(
        opens(&w, no_settle),
        Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
    ));

    // A wait for an effect that was settled done.
    let mut done = waited();
    done[5] = Event::EffectSettled {
        effect: 0,
        outcome: "done".into(),
    };
    assert!(matches!(
        opens(&w, done),
        Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
    ));

    // A wait naming another effect.
    let mut other = waited();
    other[6] = Event::Transition {
        trigger: Trigger::AwaitApproval {
            effect: 1,
            rest: vec![],
            roles_left: vec![],
        },
    };
    assert!(matches!(
        opens(&w, other),
        Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
    ));

    // An approval that names another phase or effect than the wait's.
    for trigger in [
        Trigger::ActionApproved {
            effect: 0,
            phase: MissionState::Building,
        },
        Trigger::ActionApproved {
            effect: 1,
            phase: MissionState::Investigating,
        },
        Trigger::ActionDenied { effect: 1 },
        Trigger::ApprovalLapsed {
            effect: 1,
            reason: "approval_expired".into(),
        },
    ] {
        let mut events = waited();
        events.push(Event::Transition {
            trigger: trigger.clone(),
        });
        assert!(
            matches!(
                opens(&w, events),
                Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
            ),
            "{trigger:?}"
        );
    }
}

#[test]
fn m2a_a_restart_after_an_approved_action_mid_phase_blocks_the_mission() {
    // Re-running the phase could ask for the approved action again: the
    // mission is blocked instead (fail closed).
    let w = world();
    let mut events = waited();
    events.push(Event::Transition {
        trigger: Trigger::ActionApproved {
            effect: 0,
            phase: MissionState::Investigating,
        },
    });
    let mother = opens(&w, events).unwrap();
    assert_eq!(mother.state(MissionId(1)), Some(MissionState::Blocked));
    assert_eq!(
        mother.reason(MissionId(1)).as_deref(),
        Some("phase_interrupted")
    );
    assert_eq!(w.port.effects(), vec![]);
}
