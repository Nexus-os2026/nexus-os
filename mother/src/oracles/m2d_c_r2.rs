//! Oracles for Codex review 2 of PR #78 (M2d-C), with the Architect's one
//! design (AGENTS.md §12.2), pre-registered before it, against the
//! recording port; deterministic: every race is a forced order at a held
//! port call, never a sleep. Its real-broker twin is in
//! `adapter/m2d_files_r2.rs`.
//!
//! - A. A Stop (the Owner's or the emergency one) or the time budget moves
//!   a mission that waits on a File to a non-runnable settling state under
//!   the coordinator lock; nothing of it runs from then on, and its success
//!   is journaled only inside the terminal transition. A second Stop keeps
//!   the first cause.
//! - B. The broker's stop and withdrawal come first; the port keeps the
//!   waited File's identity and outcome until the coordinator acknowledges
//!   it after the terminal record.
//! - C. A settlement that cannot read the broker still ends the mission:
//!   Blocked `file_publication_uncertain`.

use super::{contract_with, world, World};
use crate::adapter::p0_support::action;
use crate::coordinator::CoordinatorError;
use crate::effect::Publication;
use crate::effect::{ApprovalStatus, EffectOutcome};
use crate::evidence::{SettlementEnd, SettlementOutcome};
use crate::journal::{Event, MissionId};
use crate::state::{MissionState, Trigger};
use std::sync::Arc;
/// Round 4 (compile only): the old status of a File that ran, as the
/// snapshot's read.
fn read(status: ApprovalStatus) -> Publication {
    match status {
        ApprovalStatus::RecoveryRequired => Publication::Published {
            recovery_required: true,
            interrupted: vec![],
        },
        ApprovalStatus::Overtaken { causes } => Publication::Published {
            recovery_required: false,
            interrupted: causes,
        },
        _ => Publication::Published {
            recovery_required: false,
            interrupted: vec![],
        },
    }
}

/// A mission whose Builder asked for two Files; the first waits.
fn waiting(w: &World) -> MissionId {
    let id = w.approved(contract_with("ASK-BUILDER-FILE ASK-BUILDER-FILE"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.set_pending(Some(action(1)));
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    id
}

fn events(w: &World, id: MissionId) -> Vec<Event> {
    w.mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| e.event)
        .collect()
}

/// The successes journaled (round 4: an ordinary approval, or a
/// settlement's published outcome).
fn approvals(w: &World, id: MissionId) -> usize {
    events(w, id)
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Transition {
                    trigger: Trigger::ActionApproved { .. }
                        | Trigger::Settled {
                            outcome: SettlementOutcome::Published { .. },
                            ..
                        }
                }
            )
        })
        .count()
}

/// Ended Blocked with `code`, the success journaled once just before the
/// end (with its closure), nothing more sent or run, and acknowledged.
fn settled(w: &World, id: MissionId, code: &str, calls: usize) {
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked), "{code}");
    assert_eq!(w.mother.reason(id).as_deref(), Some(code));
    let events = events(w, id);
    assert!(
        matches!(
            &events[events.len() - 2..],
            [
                Event::Transition {
                    trigger: Trigger::Settled {
                        outcome: SettlementOutcome::Published { .. },
                        end: SettlementEnd::Block(_),
                        ..
                    }
                },
                Event::ApprovalClosed { .. },
            ]
        ),
        "{code}: {events:?}"
    );
    assert_eq!(approvals(w, id), 1, "{code}");
    assert_eq!(w.port.effects().len(), 1, "{code}: the rest never sent");
    assert_eq!(w.keyed_calls(), calls, "{code}: no later lane");
    assert!(w.port.acks().contains(&id), "{code}: acknowledged");
}

/// (1) A: a Stop held in its settlement (at the broker read) leaves the
/// mission non-runnable: an advance meanwhile runs and sends nothing; the
/// Stop then ends it with the publication reason.
#[test]
fn m2d_c_r2_an_advance_during_settlement_runs_nothing() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.set_publication(Some(read(ApprovalStatus::Approved)));
    w.port.hold_publication();
    let mother = Arc::clone(&w.mother);
    let stopping = std::thread::spawn(move || mother.stop(id));
    assert!(w.port.wait_publication_entered());
    assert!(w.mother.advance(id).is_err(), "settling is not runnable");
    assert_eq!(w.port.effects().len(), 1);
    assert_eq!(w.keyed_calls(), calls);
    assert_eq!(approvals(&w, id), 0, "nothing journaled before the end");
    w.port.release_publication();
    assert_eq!(stopping.join().unwrap(), Ok(MissionState::Blocked));
    settled(&w, id, "file_publication_interrupted:owner_stop", calls);
}

/// (2) A: a second Stop during the settlement is a no-op that keeps the
/// first cause (the emergency Stop found by an advance), and the mission
/// still ends with it.
#[test]
fn m2d_c_r2_a_second_stop_keeps_the_first_cause() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.set_publication(Some(read(ApprovalStatus::Approved)));
    w.port.hold_publication();
    w.port.set_emergency(true);
    let mother = Arc::clone(&w.mother);
    let advancing = std::thread::spawn(move || mother.advance(id).map(|a| a.to));
    assert!(w.port.wait_publication_entered());
    let second = w.mother.stop(id);
    assert_ne!(second, Ok(MissionState::Stopped), "{second:?}");
    assert_eq!(w.mother.state(id), Some(MissionState::AwaitingApproval));
    w.port.release_publication();
    assert_eq!(advancing.join().unwrap(), Ok(MissionState::Blocked));
    settled(&w, id, "file_publication_interrupted:emergency_stop", calls);
}

/// (3) B: the Owner's Stop while an advance holds the broker's answer (the
/// File ran; the advance has not journaled it): the Stop's settlement still
/// sees the File, journals it once and ends `owner_stop`; the advance runs
/// nothing more.
#[test]
fn m2d_c_r2_a_stop_between_the_answer_and_its_journal_sees_the_file() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.approval_reply(ApprovalStatus::Approved);
    w.port.set_publication(Some(read(ApprovalStatus::Approved)));
    w.port.hold_approval();
    let mother = Arc::clone(&w.mother);
    let advancing = std::thread::spawn(move || mother.advance(id).map(|a| a.to));
    assert!(w.port.wait_approval_entered());
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    let advanced = advancing.join().unwrap();
    assert!(
        matches!(
            advanced,
            Err(CoordinatorError::Stopped) | Ok(MissionState::Blocked)
        ),
        "{advanced:?}"
    );
    settled(&w, id, "file_publication_interrupted:owner_stop", calls);
}

/// (4) C: a settlement whose broker read fails still ends the mission,
/// Blocked `file_publication_uncertain`, with no success claimed; Stop
/// everything completes and counts it.
#[test]
fn m2d_c_r2_an_unreadable_broker_ends_the_mission_uncertain() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.fail_publication();
    assert_eq!(w.mother.stop_all(), 1);
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked));
    assert_eq!(
        w.mother.reason(id).as_deref(),
        Some("file_publication_uncertain")
    );
    assert_eq!(approvals(&w, id), 0, "no success claimed");
    assert_eq!(w.port.effects().len(), 1);
    assert_eq!(w.keyed_calls(), calls);
    assert!(w.port.is_cancelled(id), "withdrawn first");
}

/// (5) Controls: a File that did not run is stopped as before (the
/// withdrawal first); an ordinary approval goes on as before and is
/// acknowledged.
#[test]
fn m2d_c_r2_controls_the_ordinary_paths_are_unchanged() {
    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert_eq!(approvals(&w, id), 0);
    assert!(w.port.is_cancelled(id));

    let w = world();
    let id = waiting(&w);
    w.port.approval_reply(ApprovalStatus::Approved);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Verifying);
    assert_eq!(approvals(&w, id), 1);
    assert_eq!(w.port.effects().len(), 2, "File B goes on");
    assert!(w.port.acks().contains(&id));
}
