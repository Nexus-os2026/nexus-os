//! M2d-C round 5, decision D3 (pre-registered before the repair), against
//! the recording port: Mother's own deadline never lapses a File by itself.
//! When the port says a waited File expired, was denied or lapsed, Mother
//! withdraws first, then takes the one snapshot: only "confirmed not
//! published" ends as the ordinary denial or lapse (acknowledged); a File
//! published meanwhile ends in its truthful `Settled` line and a Block;
//! pending or uncertain ends `file_publication_uncertain`, never
//! acknowledged.

use super::{contract_with, world, World};
use crate::adapter::p0_support::action;
use crate::effect::{ApprovalStatus, EffectOutcome, Publication};
use crate::evidence::{BlockReason, SettlementEnd, SettlementOutcome};
use crate::journal::{Event, MissionId};
use crate::state::{MissionState, Trigger};
use std::sync::Arc;

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

fn terminal(w: &World, id: MissionId) -> Vec<(SettlementOutcome, SettlementEnd)> {
    w.mother
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
        .collect()
}

fn lapsed(w: &World, id: MissionId) -> bool {
    w.mother.journal().unwrap().into_iter().any(|e| {
        e.mission == id
            && matches!(
                e.event,
                Event::Transition {
                    trigger: Trigger::ApprovalLapsed { .. } | Trigger::ActionDenied { .. }
                }
            )
    })
}

/// D3: an expiry (Mother's deadline) while the File was in fact published
/// meanwhile: withdrawn first (the snapshot is read after it), then the
/// truthful record and a Block, acknowledged; never an ordinary lapse.
#[test]
fn m2d_c_r5_d3_an_expired_wait_reads_the_file_after_withdrawing() {
    for status in [
        ApprovalStatus::Expired,
        ApprovalStatus::Denied,
        ApprovalStatus::Lapsed {
            class: "revoked".into(),
        },
    ] {
        let w = world();
        let id = waiting(&w);
        w.port.approval_reply(status.clone());
        w.port.set_publication(Some(Publication::Published {
            recovery_required: false,
            interrupted: vec![],
        }));
        w.port.hold_publication();
        let mother = Arc::clone(&w.mother);
        let advancing = std::thread::spawn(move || mother.advance(id).map(|a| a.to));
        assert!(w.port.wait_publication_entered(), "{status:?}");
        assert!(w.port.is_cancelled(id), "{status:?}: withdrawn first");
        w.port.release_publication();
        assert_eq!(advancing.join().unwrap(), Ok(MissionState::Blocked));
        assert!(!lapsed(&w, id), "{status:?}: never an ordinary lapse");
        assert_eq!(
            terminal(&w, id),
            vec![(
                SettlementOutcome::Published {
                    recovery_required: false,
                    interrupted: vec![],
                },
                SettlementEnd::Block(BlockReason::FilePublicationApprovalDeadline),
            )],
            "{status:?}"
        );
        assert_eq!(w.port.acked(), vec![action(1)], "{status:?}");
    }
}

/// D3: confirmed not published is the only ordinary lapse (acknowledged);
/// pending or uncertain is `file_publication_uncertain`, kept pinned.
#[test]
fn m2d_c_r5_d3_only_confirmed_not_published_lapses() {
    let w = world();
    let id = waiting(&w);
    w.port.approval_reply(ApprovalStatus::Expired);
    w.port
        .set_publication(Some(Publication::ConfirmedNotPublished));
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("approval_expired"));
    assert!(lapsed(&w, id));
    assert_eq!(w.port.acked(), vec![action(1)]);

    for read in [
        Some(Publication::Pending),
        Some(Publication::Uncertain),
        None,
    ] {
        let w = world();
        let id = waiting(&w);
        w.port.approval_reply(ApprovalStatus::Expired);
        w.port.set_publication(read.clone());
        assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
        assert_eq!(
            w.mother.reason(id).as_deref(),
            Some("file_publication_uncertain"),
            "{read:?}"
        );
        assert!(!lapsed(&w, id), "{read:?}");
        assert!(w.port.acked().is_empty(), "{read:?}");
    }
}
