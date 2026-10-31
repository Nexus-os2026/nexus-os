//! Codex review MAJOR (oracle first): the identity of the action a mission
//! waits on (its broker run and commitment, and the port's deadline for
//! it) is journaled, and a reopened coordinator hands it back to its port,
//! so a rebuilt port can still withdraw it.

use super::*;
use crate::journal::{Entry, Event, JournalError, GENESIS};
use crate::state::Trigger;

fn action() -> PendingAction {
    PendingAction {
        run: "run-0123456789abcdef0123456789abcdef".into(),
        commitment: "cmt-0123456789abcdef0123456789abcdef".into(),
        deadline_mono_ms: 600_000,
    }
}

fn waiting_with_action(w: &World) -> MissionId {
    w.port.set_pending(Some(action()));
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    id
}

#[test]
fn m2a_rebuild_the_pending_action_is_journaled_before_the_wait() {
    let w = world();
    let id = waiting_with_action(&w);
    let events: Vec<Event> = w
        .mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| e.event)
        .collect();
    let n = events.len();
    assert_eq!(
        events[n - 3],
        Event::EffectSettled {
            effect: 0,
            outcome: "awaiting_approval".into()
        }
    );
    assert_eq!(
        events[n - 2],
        Event::ApprovalPending {
            effect: 0,
            pending: action()
        }
    );
    assert!(matches!(
        &events[n - 1],
        Event::Transition {
            trigger: Trigger::AwaitApproval { effect: 0, .. }
        }
    ));
}

#[test]
fn m2a_rebuild_a_reopened_coordinator_hands_the_action_back_to_its_port() {
    let w = world();
    let id = waiting_with_action(&w);
    assert_eq!(w.port.adopted(), vec![]);
    let calls = w.port.calls();
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    let adopted = w.port.adopted();
    assert_eq!(adopted.len(), 1);
    assert_eq!((adopted[0].0, adopted[0].1.clone()), (id, action()));
    // Handing it back sends, asks and cancels nothing.
    assert_eq!(w.port.calls(), calls);
    // A Stop then cancels the mission at the port.
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert!(w.port.is_cancelled(id));
    // A stopped mission's action is not handed back again.
    let w = w.restart();
    assert_eq!(w.port.adopted().len(), 1);
}

#[test]
fn m2a_rebuild_no_action_is_journaled_when_the_port_reports_none() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    w.mother.advance(id).unwrap();
    assert!(!w
        .mother
        .journal()
        .unwrap()
        .iter()
        .any(|e| matches!(e.event, Event::ApprovalPending { .. })));
}

#[test]
fn m2a_rebuild_a_journal_failure_recording_the_action_withdraws_it() {
    let w = world();
    w.port.set_pending(Some(action()));
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    // Building: Sentinel (2 entries), Builder (2), dispatch, settle; the
    // action's record after that fails.
    w.journal.fail_after(6);
    assert!(matches!(
        w.mother.advance(id),
        Err(CoordinatorError::Journal(_))
    ));
    assert!(w.port.is_cancelled(id));
    assert!(w.mother.state(id).unwrap().is_terminal());
    assert!(matches!(
        w.reopen(),
        Err(CoordinatorError::Journal(JournalError::Poisoned))
    ));
}

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

fn settled(outcome: &str) -> Vec<Event> {
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
            outcome: outcome.into(),
        },
    ]
}

fn pending(effect: u64) -> Event {
    Event::ApprovalPending {
        effect,
        pending: action(),
    }
}

fn wait() -> Event {
    Event::Transition {
        trigger: Trigger::AwaitApproval {
            effect: 0,
            rest: vec![],
            roles_left: vec![],
        },
    }
}

#[test]
fn m2a_rebuild_an_interrupted_mission_s_action_is_handed_back_before_it_is_cancelled() {
    // The process died after the action was journaled, before the wait.
    let w = world();
    let mut events = settled("awaiting_approval");
    events.push(pending(0));
    w.journal.set_lines(chained(events));
    let mother = w.reopen().unwrap();
    assert_eq!(mother.state(MissionId(1)), Some(MissionState::Blocked));
    let adopted = w.port.adopted();
    assert_eq!(adopted.len(), 1);
    let cancel_at = w
        .port
        .calls()
        .iter()
        .position(|c| *c == PortCall::Cancel(MissionId(1)))
        .expect("cancelled");
    assert!(adopted[0].2 <= cancel_at, "handed back after the cancel");
}

#[test]
fn m2a_rebuild_replay_accepts_the_action_only_where_it_is_written() {
    let w = world();
    // Positive control.
    let mut good = settled("awaiting_approval");
    good.push(pending(0));
    good.push(wait());
    w.journal.set_lines(chained(good));
    assert_eq!(
        w.reopen().unwrap().state(MissionId(1)),
        Some(MissionState::AwaitingApproval)
    );
    let refused = |events: Vec<Event>| {
        w.journal.set_lines(chained(events));
        matches!(
            w.reopen(),
            Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
        )
    };
    // For an effect that was settled done.
    let mut done = settled("done");
    done.push(pending(0));
    assert!(refused(done));
    // For another effect.
    let mut other = settled("awaiting_approval");
    other.push(pending(1));
    assert!(refused(other));
    // Twice.
    let mut twice = settled("awaiting_approval");
    twice.push(pending(0));
    twice.push(pending(0));
    assert!(refused(twice));
    // After the wait.
    let mut late = settled("awaiting_approval");
    late.push(wait());
    late.push(pending(0));
    assert!(refused(late));
}

/// Second fresh-context review (oracle first): two missions whose journal
/// names the same action (a tampered journal) hand neither back.
#[test]
fn m2a_rebuild_an_action_claimed_by_two_missions_is_handed_back_to_neither() {
    let w = world();
    let c = contract();
    let mut lines = Vec::new();
    let mut prev = GENESIS.to_string();
    let mut seq = 0u64;
    for mission in [MissionId(1), MissionId(2)] {
        let mut events = settled("awaiting_approval");
        events.push(pending(0));
        events.push(wait());
        // Each mission drafts its own contract.
        events[0] = Event::Drafted {
            contract: c.clone(),
        };
        for event in events {
            let entry = Entry::chained(seq, &prev, mission, event);
            prev = entry.hash.clone();
            lines.push(entry.to_line());
            seq += 1;
        }
    }
    w.journal.set_lines(lines);
    let mother = w.reopen().unwrap();
    assert_eq!(
        mother.state(MissionId(1)),
        Some(MissionState::AwaitingApproval)
    );
    assert_eq!(
        mother.state(MissionId(2)),
        Some(MissionState::AwaitingApproval)
    );
    assert_eq!(w.port.adopted(), vec![]);
}
