//! M2d-C round 5, decision D6 (pre-registered before the repair): a
//! publication that requires recovery and was also interrupted after its
//! final check keeps both in its `Settled` line (never `interrupted: []`);
//! the Block is `file_publication_recovery_required`.

use super::{contract_with, world};
use crate::adapter::p0_support::action;
use crate::adapter::status_of;
use crate::effect::{ApprovalStatus, EffectOutcome};
use crate::evidence::{BlockReason, PublicationCause, SettlementEnd, SettlementOutcome};
use crate::journal::Event;
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::commitment::{
    CommitmentState, CommitmentView, PublicationClass,
};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};

/// The port reads both from the broker's class.
#[test]
fn m2d_c_r5_d6_the_port_keeps_recovery_and_its_codes() {
    let view = CommitmentView {
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
        interrupted: vec!["cancelled"],
        recovery_required: true,
        publication: PublicationClass::Published {
            recovery_required: true,
            interrupted: vec!["cancelled"],
        },
    };
    assert_eq!(
        status_of(&view, 2_000, 2_000, 600_000),
        ApprovalStatus::RecoveryRequired {
            causes: vec!["cancelled"]
        }
    );
}

/// The ordinary path journals both, then blocks recovery required.
#[test]
fn m2d_c_r5_d6_the_settled_line_keeps_both() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-FILE ASK-BUILDER-FILE"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.set_pending(Some(action(1)));
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    w.mother.advance(id).unwrap();
    w.port.approval_reply(ApprovalStatus::RecoveryRequired {
        causes: vec!["cancelled"],
    });
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    let settled: Vec<(SettlementOutcome, SettlementEnd)> = w
        .mother
        .journal()
        .unwrap()
        .into_iter()
        .filter(|e| e.mission == id)
        .filter_map(|e| match e.event {
            Event::Transition {
                trigger: Trigger::Settled { outcome, end, .. },
            } => Some((outcome, end)),
            _ => None,
        })
        .collect();
    assert_eq!(
        settled,
        vec![(
            SettlementOutcome::Published {
                recovery_required: true,
                interrupted: vec![PublicationCause::Cancelled],
            },
            SettlementEnd::Block(BlockReason::FilePublicationRecoveryRequired),
        )]
    );
}
