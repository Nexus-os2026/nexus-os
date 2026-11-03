//! Oracles for Codex review 1 of PR #78 (M2d-C), with the Architect's
//! decisions, pre-registered before the repairs, against the recording
//! port. Their real-broker twins are in `adapter/m2d_files_r1.rs`.
//!
//! - HIGH 1: a Stop (emergency or the Owner's) or the time budget ending a
//!   mission that waits on a File first reconciles it: a publication that
//!   ran is journaled as the success it is (`ActionApproved`, the action
//!   closed), then the mission ends with the interruption code
//!   (`file_publication_interrupted:<cause>`, the cause Mother ended it for);
//!   nothing more of it runs. One that did not run ends as before.
//! - HIGH 2: a publication that succeeded with recovery required is
//!   journaled the same way, then the mission ends
//!   `file_publication_recovery_required`; nothing more runs.

use super::{contract_with, world, World};
use crate::adapter::p0_support::action;
use crate::adapter::status_of;
use crate::effect::Publication;
use crate::effect::{ApprovalStatus, EffectOutcome};
use crate::evidence::BlockReason;
use crate::evidence::{SettlementEnd, SettlementOutcome};
use crate::journal::{Event, MissionId};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::commitment::{
    CommitmentState, CommitmentView, PublicationClass,
};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};
use std::time::Duration;
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

/// The journal from the mission's settlement on (round 4: its start, if
/// any, and its one terminal line).
fn tail(w: &World, id: MissionId) -> Vec<Event> {
    w.mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .map(|e| e.event)
        .skip_while(|e| {
            !matches!(
                e,
                Event::SettlementStarted { .. }
                    | Event::Transition {
                        trigger: Trigger::Settled { .. }
                    }
            )
        })
        .collect()
}

/// Round 4: whether the journal claims a publication.
fn claims_success(events: &[Event]) -> bool {
    events.iter().any(|e| {
        matches!(
            e,
            Event::Transition {
                trigger: Trigger::Settled {
                    outcome: SettlementOutcome::Published { .. },
                    ..
                }
            }
        )
    })
}

/// Round 4: the success and the end in one terminal line (after the
/// settlement's start, on a Stop), then the action closed: nothing after.
fn truthful_then_ended(w: &World, id: MissionId, code: &str, calls: usize) {
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked), "{code}");
    assert_eq!(w.mother.reason(id).as_deref(), Some(code));
    let tail = tail(w, id);
    let rest = match tail.as_slice() {
        [Event::SettlementStarted { .. }, rest @ ..] => rest,
        all => all,
    };
    assert!(
        matches!(
            rest,
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
        "{code}: {tail:?}"
    );
    assert_eq!(w.port.effects().len(), 1, "{code}: the rest never sent");
    assert_eq!(w.keyed_calls(), calls, "{code}: no later lane");
}

/// HIGH 1, the emergency Stop Mother finds on her next advance: the File
/// it waited on ran, so it is journaled first, whatever the broker named. Control: a File that did not run is stopped
/// as before.
#[test]
fn m2d_c_r1_an_emergency_stop_reconciles_a_publication_that_ran() {
    for (status, code) in [
        (
            ApprovalStatus::Approved,
            "file_publication_interrupted:emergency_stop",
        ),
        (
            ApprovalStatus::Overtaken {
                causes: vec!["emergency_stop"],
            },
            "file_publication_interrupted:emergency_stop",
        ),
    ] {
        let w = world();
        let id = waiting(&w);
        let calls = w.keyed_calls();
        w.port.set_publication(Some(read(status)));
        w.port.set_emergency(true);
        let step = w.mother.advance(id).unwrap();
        assert_eq!(step.to, MissionState::Blocked);
        truthful_then_ended(&w, id, code, calls);
    }
    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    w.port.set_emergency(true);
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert!(
        !claims_success(&tail(&w, id)),
        "nothing ran: no success journaled"
    );
}

/// HIGH 1, the time budget spent while a File waited: one that ran is
/// journaled, then `file_publication_interrupted:time_budget`. Control: one
/// that did not run lapses `budget_time`, as before.
#[test]
fn m2d_c_r1_the_time_budget_reconciles_a_publication_that_ran() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.set_publication(Some(read(ApprovalStatus::Approved)));
    w.clock.advance(Duration::from_millis(3_600_001));
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    truthful_then_ended(&w, id, "file_publication_interrupted:time_budget", calls);

    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    w.clock.advance(Duration::from_millis(3_600_001));
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.reason(id).as_deref(), Some("budget_time"));
    assert!(!claims_success(&tail(&w, id)));
}

/// HIGH 1, the Owner's Stop of a mission whose File ran: journaled, then
/// `file_publication_interrupted:owner_stop`, whatever the broker named.
/// Control: a File that did not run is stopped as before.
#[test]
fn m2d_c_r1_the_owner_s_stop_reconciles_a_publication_that_ran() {
    for (status, code) in [
        (
            ApprovalStatus::Approved,
            "file_publication_interrupted:owner_stop",
        ),
        (
            ApprovalStatus::Overtaken {
                causes: vec!["revoked"],
            },
            "file_publication_interrupted:owner_stop",
        ),
    ] {
        let w = world();
        let id = waiting(&w);
        let calls = w.keyed_calls();
        w.port.set_publication(Some(read(status)));
        assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
        truthful_then_ended(&w, id, code, calls);
    }
    let w = world();
    let id = waiting(&w);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert!(!claims_success(&tail(&w, id)));
}

/// HIGH 2: a publication that succeeded with recovery required is
/// journaled as the success it is, then the mission ends
/// `file_publication_recovery_required`, on the ordinary wait and on a
/// Stop alike.
#[test]
fn m2d_c_r1_a_recovery_required_publication_ends_the_mission() {
    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port.approval_reply(ApprovalStatus::RecoveryRequired);
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    truthful_then_ended(&w, id, "file_publication_recovery_required", calls);

    let w = world();
    let id = waiting(&w);
    let calls = w.keyed_calls();
    w.port
        .set_publication(Some(read(ApprovalStatus::RecoveryRequired)));
    assert_eq!(w.mother.stop(id), Ok(MissionState::Blocked));
    truthful_then_ended(&w, id, "file_publication_recovery_required", calls);
}

/// HIGH 2 at the port, and the new codes: a Succeeded commitment whose view
/// says recovery is required reads RecoveryRequired, before any
/// interruption; the codes round-trip.
#[test]
fn m2d_c_r1_the_port_reads_recovery_and_the_codes_round_trip() {
    let view = |recovery_required: bool, interrupted: Vec<&'static str>| CommitmentView {
        id: CommitmentId::parse("cmt-00000000000000000000000000000001").unwrap(),
        agent: AgentId::new("mother").unwrap(),
        run: RunId::parse("run-00000000000000000000000000000001").unwrap(),
        kind: CapabilityKind::Files,
        class: EffectClass::R2,
        operation: "create",
        target: "index.html in workspace wsp-0".into(),
        summary: vec![],
        state: CommitmentState::Succeeded,
        requires_approval: true,
        created_wall_ms: 1_000,
        expires_wall_ms: 601_000,
        binding_short: "0000".into(),
        interrupted: interrupted.clone(),
        recovery_required,
        publication: PublicationClass::Published {
            recovery_required,
            interrupted,
        },
    };
    for interrupted in [vec![], vec!["revoked"]] {
        assert_eq!(
            status_of(&view(true, interrupted), 2_000, 2_000, 600_000),
            ApprovalStatus::RecoveryRequired
        );
    }
    for code in [
        "file_publication_recovery_required",
        "file_publication_interrupted:time_budget",
        "file_publication_interrupted:owner_stop",
        "file_publication_interrupted:emergency_stop",
    ] {
        let reason = BlockReason::parse(code).expect(code);
        assert_eq!(reason.to_string(), code);
        assert!(BlockReason::every().contains(&reason), "{code}");
    }
}
