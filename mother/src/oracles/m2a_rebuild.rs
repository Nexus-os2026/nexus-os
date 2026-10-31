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

/// Codex re-review MINOR (b), oracle first: claims are compared by parsed
/// identity, so a case-changed copy of the same action is a duplicate and
/// is handed back to neither mission.
#[test]
fn m2a_rebuild_a_case_changed_duplicate_is_handed_back_to_neither() {
    let w = world();
    let c = contract();
    let upper = PendingAction {
        run: "run-0123456789ABCDEF0123456789ABCDEF".into(),
        commitment: "cmt-0123456789ABCDEF0123456789ABCDEF".into(),
        deadline_mono_ms: 600_000,
    };
    let mut lines = Vec::new();
    let mut prev = GENESIS.to_string();
    let mut seq = 0u64;
    for (mission, action) in [(MissionId(1), action()), (MissionId(2), upper)] {
        let mut events = settled("awaiting_approval");
        events.push(Event::ApprovalPending {
            effect: 0,
            pending: action,
        });
        events.push(wait());
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
        mother.state(MissionId(2)),
        Some(MissionState::AwaitingApproval)
    );
    assert_eq!(w.port.adopted(), vec![]);
}

fn closed_entries(w: &World, id: MissionId) -> usize {
    w.mother
        .journal()
        .unwrap()
        .iter()
        .filter(|e| e.mission == id && matches!(e.event, Event::ApprovalClosed { .. }))
        .count()
}

/// Codex re-review MAJOR (oracle first): the closure is journaled only
/// after the port confirms the broker no longer holds the request open.
#[test]
fn m2a_close_a_stop_journals_the_closure_once_the_port_confirms_it() {
    let w = world();
    let id = waiting_with_action(&w);
    assert_eq!(closed_entries(&w, id), 0);
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert!(w.port.is_cancelled(id));
    assert_eq!(closed_entries(&w, id), 1);
    let last = w.mother.journal().unwrap().into_iter().last().unwrap();
    assert_eq!(last.event, Event::ApprovalClosed { effect: 0 });
}

#[test]
fn m2a_close_an_unconfirmed_withdrawal_is_retried_on_every_reopen() {
    let w = world();
    let id = waiting_with_action(&w);
    w.port.set_settled(false);
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(closed_entries(&w, id), 0, "not confirmed: not closed");
    // Reopened while the broker still holds it: handed back and cancelled
    // again, whatever the mission's state.
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    let adopted = w.port.adopted();
    assert_eq!(adopted.len(), 1);
    let cancels = w
        .port
        .calls()
        .iter()
        .filter(|c| **c == PortCall::Cancel(id))
        .count();
    assert_eq!(cancels, 2, "cancelled again on reopen");
    assert_eq!(closed_entries(&w, id), 0);
    // Once the broker confirms, the closure is journaled, once.
    w.port.set_settled(true);
    let w = w.restart();
    assert_eq!(closed_entries(&w, id), 1);
    assert_eq!(w.port.adopted().len(), 2);
    let w = w.restart();
    assert_eq!(closed_entries(&w, id), 1);
    assert_eq!(
        w.port.adopted().len(),
        2,
        "a closed request is not handed back"
    );
}

#[test]
fn m2a_close_each_end_of_a_wait_closes_the_request() {
    for (status, state) in [
        (ApprovalStatus::Denied, MissionState::Denied),
        (ApprovalStatus::Expired, MissionState::Blocked),
        (ApprovalStatus::Approved, MissionState::Verifying),
    ] {
        let w = world();
        let id = waiting_with_action(&w);
        w.port.approval_reply(status.clone());
        assert_eq!(w.mother.advance(id).unwrap().to, state, "{status:?}");
        assert_eq!(closed_entries(&w, id), 1, "{status:?}");
    }
}

#[test]
fn m2a_close_replay_accepts_a_closure_only_of_the_open_request() {
    let w = world();
    let mut good = settled("awaiting_approval");
    good.push(pending(0));
    good.push(wait());
    good.push(Event::Transition {
        trigger: Trigger::Stop {
            reason: "owner".into(),
        },
    });
    good.push(Event::ApprovalClosed { effect: 0 });
    w.journal.set_lines(chained(good));
    assert_eq!(
        w.reopen().unwrap().state(MissionId(1)),
        Some(MissionState::Stopped)
    );
    let refused = |events: Vec<Event>| {
        w.journal.set_lines(chained(events));
        matches!(
            w.reopen(),
            Err(CoordinatorError::Journal(JournalError::Inconsistent { .. }))
        )
    };
    let mut none = settled("awaiting_approval");
    none.push(Event::ApprovalClosed { effect: 0 });
    assert!(refused(none), "a closure with no request");
    let mut other = settled("awaiting_approval");
    other.push(pending(0));
    other.push(Event::ApprovalClosed { effect: 1 });
    assert!(refused(other), "a closure of another effect");
    let mut twice = settled("awaiting_approval");
    twice.push(pending(0));
    twice.push(Event::ApprovalClosed { effect: 0 });
    twice.push(Event::ApprovalClosed { effect: 0 });
    assert!(refused(twice), "closed twice");
}

/// Third review MAJOR (oracle first): a journal that fails while recovery
/// closes one action still sees every other open action withdrawn, the
/// waiting one included (a journal that cannot open never resumes it).
#[test]
fn m2a_close_a_journal_failure_during_recovery_still_withdraws_every_action() {
    let w = world();
    let c = contract();
    let action_of = |n: u8| PendingAction {
        run: format!("run-{:032x}", n),
        commitment: format!("cmt-{:032x}", n),
        deadline_mono_ms: 600_000,
    };
    let mut lines = Vec::new();
    let mut prev = GENESIS.to_string();
    let mut seq = 0u64;
    for (n, stopped) in [(1u8, true), (2, true), (3, false)] {
        let mission = MissionId(n as u64);
        let mut events = settled("awaiting_approval");
        events[0] = Event::Drafted {
            contract: c.clone(),
        };
        events.push(Event::ApprovalPending {
            effect: 0,
            pending: action_of(n),
        });
        events.push(wait());
        if stopped {
            events.push(Event::Transition {
                trigger: Trigger::Stop {
                    reason: "owner".into(),
                },
            });
        }
        for event in events {
            let entry = Entry::chained(seq, &prev, mission, event);
            prev = entry.hash.clone();
            lines.push(entry.to_line());
            seq += 1;
        }
    }
    w.journal.set_lines(lines);
    // The first closure recovery journals fails.
    w.journal.fail_after(0);
    assert!(matches!(w.reopen(), Err(CoordinatorError::Journal(_))));
    for n in 1..=3u64 {
        assert!(
            w.port.is_cancelled(MissionId(n)),
            "mission {n} not withdrawn"
        );
    }
    assert!(w.journal.is_poisoned());
}

/// The journal of three missions that each left an action open: 1 stopped
/// (its withdrawal never ran), 2 waiting, 3 interrupted after the Owner
/// approved its action (journaled approved, not closed).
fn three_open() -> Vec<String> {
    let c = contract();
    let action_of = |n: u8| PendingAction {
        run: format!("run-{:032x}", n),
        commitment: format!("cmt-{:032x}", n),
        deadline_mono_ms: 600_000,
    };
    let mut lines = Vec::new();
    let mut prev = GENESIS.to_string();
    let mut seq = 0u64;
    for n in 1..=3u8 {
        let mission = MissionId(n as u64);
        let mut events = settled("awaiting_approval");
        events[0] = Event::Drafted {
            contract: c.clone(),
        };
        events.push(Event::ApprovalPending {
            effect: 0,
            pending: action_of(n),
        });
        events.push(wait());
        match n {
            1 => events.push(Event::Transition {
                trigger: Trigger::Stop {
                    reason: "owner".into(),
                },
            }),
            3 => events.push(Event::Transition {
                trigger: Trigger::ActionApproved {
                    effect: 0,
                    phase: MissionState::Investigating,
                },
            }),
            _ => {}
        }
        for event in events {
            let entry = Entry::chained(seq, &prev, mission, event);
            prev = entry.hash.clone();
            lines.push(entry.to_line());
            seq += 1;
        }
    }
    lines
}

/// A port scripted for the Codex case: mission 3's approved action is
/// still executing when recovery first asks, and final when it asks again.
fn scripted(w: &World) {
    w.port
        .script_settled(&format!("cmt-{:032x}", 3), vec![false, true]);
}

/// Codex re-review #2 MAJOR (oracle first): recovery is fail-closed. For
/// every N, a journal that fails the Nth append recovery makes (the
/// closures, the interrupted mission's block and its late closure in
/// end_with) makes open() fail, and every open request is withdrawn, the
/// waiting one too. With no failure, open() succeeds and only the waiting
/// request stays open.
#[test]
fn m2a_recovery_fails_closed_at_every_append() {
    let clean = world();
    clean.journal.set_lines(three_open());
    scripted(&clean);
    let mother = clean.reopen().unwrap();
    let appends = clean.journal.lines().len() - three_open().len();
    assert!(appends >= 3, "{appends}");
    assert_eq!(
        mother.state(MissionId(2)),
        Some(MissionState::AwaitingApproval)
    );
    assert!(clean.port.is_cancelled(MissionId(1)));
    assert!(clean.port.is_cancelled(MissionId(3)));
    assert!(
        !clean.port.is_cancelled(MissionId(2)),
        "a waiting request stays"
    );
    for n in 0..appends {
        let w = world();
        w.journal.set_lines(three_open());
        scripted(&w);
        w.journal.fail_after(n);
        assert!(w.reopen().is_err(), "append {n} failed, yet open succeeded");
        for m in 1..=3u64 {
            assert!(
                w.port.is_cancelled(MissionId(m)),
                "append {n} failed: mission {m} not withdrawn"
            );
        }
    }
}

/// Codex re-review #2: a withdrawal the port fails during recovery makes
/// open() fail, and every open request is withdrawn.
#[test]
fn m2a_recovery_fails_closed_when_a_withdrawal_fails() {
    let w = world();
    w.journal.set_lines(three_open());
    w.port.fail_withdrawal(MissionId(1));
    assert!(matches!(
        w.reopen(),
        Err(CoordinatorError::Withdrawal(class)) if class == "withdrawal_failed"
    ));
    for m in 1..=3u64 {
        assert!(
            w.port.is_cancelled(MissionId(m)),
            "mission {m} not withdrawn"
        );
    }
}

/// Added after mutation testing (the propagation of these two errors
/// survived the recovery oracles above): a withdrawal the broker refuses
/// while recovery blocks an interrupted mission fails open(); a live Stop
/// whose closure cannot be journaled returns the error, with the request
/// withdrawn.
#[test]
fn m2a_recovery_fails_closed_when_an_interrupted_mission_s_withdrawal_fails() {
    let w = world();
    // Interrupted after an effect that ran (no open request).
    let mut events = settled("done");
    events.truncate(6);
    w.journal.set_lines(chained(events));
    w.port.fail_withdrawal(MissionId(1));
    assert!(matches!(
        w.reopen(),
        Err(CoordinatorError::Withdrawal(class)) if class == "withdrawal_failed"
    ));
}

#[test]
fn m2a_close_a_stop_whose_closure_cannot_be_journaled_is_an_error() {
    let w = world();
    let id = waiting_with_action(&w);
    // The Stop is journaled; its closure is not.
    w.journal.fail_after(1);
    assert!(matches!(
        w.mother.stop(id),
        Err(CoordinatorError::Journal(_))
    ));
    assert!(w.port.is_cancelled(id));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(w.journal.is_poisoned());
}

/// Fourth review MINOR: a live Stop whose withdrawal the broker refuses
/// returns the error, with the Stop journaled and the withdrawal attempted.
#[test]
fn m2a_close_a_stop_whose_withdrawal_fails_is_an_error() {
    let w = world();
    let id = waiting_with_action(&w);
    w.port.fail_withdrawal(id);
    assert!(matches!(
        w.mother.stop(id),
        Err(CoordinatorError::Withdrawal(class)) if class == "withdrawal_failed"
    ));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(w.port.is_cancelled(id), "the withdrawal was attempted");
    assert!(w.mother.journal().unwrap().iter().any(|e| e.mission == id
        && matches!(
            e.event,
            Event::Transition {
                trigger: Trigger::Stop { .. }
            }
        )));
}
