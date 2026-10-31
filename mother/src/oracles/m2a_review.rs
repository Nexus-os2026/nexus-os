//! Oracles added after the M2a fresh-context review (MAJOR-1, MINOR-2,
//! MINOR-5), committed before the fixes they judge.

use super::*;
use crate::journal::{Entry, Event, JournalError, GENESIS};
use crate::state::Trigger;

/// MAJOR-1: a journal failure while settling an effect the broker left
/// waiting for approval withdraws it: the mission ends in memory and the
/// port is cancelled.
#[test]
fn m2a_review_a_journal_failure_settling_a_waiting_effect_withdraws_it() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    // Building: Sentinel (2 entries), Builder (2), dispatch; the settle
    // after that fails.
    w.journal.fail_after(5);
    assert!(matches!(
        w.mother.advance(id),
        Err(CoordinatorError::Journal(_))
    ));
    assert!(w.port.is_cancelled(id), "the waiting action is withdrawn");
    assert!(w.mother.state(id).unwrap().is_terminal());
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects().len(), 1);
    assert!(matches!(
        w.reopen(),
        Err(CoordinatorError::Journal(JournalError::Poisoned))
    ));
}

/// MAJOR-1, the same for an effect that ran: the settle failure ends the
/// mission and cancels at the port.
#[test]
fn m2a_review_a_journal_failure_settling_any_effect_cancels_at_the_port() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.journal.fail_after(5);
    assert!(w.mother.advance(id).is_err());
    assert!(w.port.is_cancelled(id));
    assert!(w.mother.state(id).unwrap().is_terminal());
}

/// MINOR-2: the broker's dialog carries every term of the contract, not
/// only the Owner's words.
#[test]
fn m2a_review_the_dialog_summary_carries_every_term() {
    let w = world();
    let mut c = contract();
    c.forbidden_effects.insert(EffectKind::Spend);
    c.forbidden_effects.insert(EffectKind::Process);
    let id = w.submitted(c.clone());
    let owner = Recording::default();
    w.mother.request_consent(id, &owner).unwrap();
    let summary = owner.0.lock().unwrap()[0].summary.clone();
    for line in [
        "│ A one-page site for Rosa's bakery",
        "│ ~/Sites/rosas-bakery",
        "Budget: 3600000 ms, 50 lane calls, money 0 (minor units)",
        "Never allowed: process, spend",
        "│ Which colours do you like?",
        "│ Blue and cream",
        "│ The page names the bakery",
    ] {
        assert!(
            summary.iter().any(|l| l == line),
            "missing {line:?} in {summary:#?}"
        );
    }
    let mut none = contract();
    none.requested_folder = None;
    let id = w.submitted(none);
    let owner = Recording::default();
    w.mother.request_consent(id, &owner).unwrap();
    let summary = owner.0.lock().unwrap()[0].summary.clone();
    assert!(summary.contains(&"Folder: none requested".to_string()));
    assert!(summary.contains(&"Never allowed: nothing".to_string()));

    // A folder that reads like the dialog's own text is shown quoted.
    let mut spoof = contract();
    spoof.forbidden_effects.insert(EffectKind::Spend);
    spoof.requested_folder = Some("Never allowed: nothing".into());
    let id = w.submitted(spoof);
    let owner = Recording::default();
    w.mother.request_consent(id, &owner).unwrap();
    let summary = owner.0.lock().unwrap()[0].summary.clone();
    assert!(
        !summary.contains(&"Never allowed: nothing".to_string()),
        "{summary:#?}"
    );
    assert!(summary.contains(&"│ Never allowed: nothing".to_string()));
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

/// A mission journaled into Investigating with effect 0 settled awaiting
/// approval.
fn settled_awaiting() -> Vec<Event> {
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
    ]
}

fn with_wait(rest: Vec<serde_json::Value>, roles_left: Vec<LaneRole>) -> Vec<Event> {
    let mut events = settled_awaiting();
    events.push(Event::Transition {
        trigger: Trigger::AwaitApproval {
            effect: 0,
            rest,
            roles_left,
        },
    });
    events
}

fn refused(w: &World, events: Vec<Event>) -> bool {
    w.journal.set_lines(chained(events));
    matches!(
        w.reopen(),
        Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
    )
}

/// MINOR-5: replay accepts a wait's payload only as the live coordinator
/// writes it.
#[test]
fn m2a_review_replay_checks_the_wait_s_payload() {
    let w = world();
    let effect = serde_json::to_value(effect_of(EffectKind::Network)).unwrap();
    // Positive control: the rest of a lane's answer and a suffix of the
    // Investigating lanes.
    w.journal.set_lines(chained(with_wait(
        vec![effect.clone()],
        vec![LaneRole::ResearcherB, LaneRole::Skeptic],
    )));
    assert_eq!(
        w.reopen().unwrap().state(MissionId(1)),
        Some(MissionState::AwaitingApproval)
    );
    // Lanes that are not a suffix of the paused phase's.
    for roles in [
        vec![LaneRole::Builder],
        vec![LaneRole::Skeptic, LaneRole::Skeptic],
        vec![LaneRole::Skeptic, LaneRole::ResearcherB],
        vec![
            LaneRole::Sentinel,
            LaneRole::ResearcherA,
            LaneRole::ResearcherB,
            LaneRole::Skeptic,
        ],
    ] {
        assert!(refused(&w, with_wait(vec![], roles.clone())), "{roles:?}");
    }
    // A rest that is not an effect request, or longer than one answer.
    assert!(refused(
        &w,
        with_wait(vec![serde_json::json!({"x": 1})], vec![])
    ));
    assert!(refused(
        &w,
        with_wait(
            vec![effect; crate::coordinator::MAX_EFFECTS_PER_CALL],
            vec![]
        )
    ));
    // A lane call between the settlement and the wait.
    let mut events = settled_awaiting();
    events.push(Event::LaneInvoked {
        invocation: crate::lane::InvocationId {
            mission: MissionId(1),
            seq: 6,
        },
        identity: crate::lane::LaneIdentity {
            role: LaneRole::Skeptic,
            name: "keyed-skeptic".into(),
            version: "m1-test".into(),
        },
        input_hash: "0".repeat(64),
    });
    events.push(Event::LaneOvertaken {
        invocation: crate::lane::InvocationId {
            mission: MissionId(1),
            seq: 6,
        },
    });
    events.push(Event::Transition {
        trigger: Trigger::AwaitApproval {
            effect: 0,
            rest: vec![],
            roles_left: vec![],
        },
    });
    assert!(refused(&w, events));
}
