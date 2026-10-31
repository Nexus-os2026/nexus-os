//! Denial means zero effect: the Owner's no, and the broker's refusal.

use super::*;

#[test]
fn deny_the_owner_s_no_means_zero_effect_and_no_lane() {
    let w = world();
    let id = w.submitted(contract_with(
        "ASK-RESEARCHERA-NETWORK ASK-BUILDER-FILE ASK-BUILDER-PROCESS ASK-SENTINEL-MODELTOOL",
    ));
    assert_eq!(
        w.mother.request_consent(id, &No),
        Err(CoordinatorError::Denied)
    );
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert_eq!(w.mother.consented_hash(id), None);
    for _ in 0..6 {
        assert!(w.mother.advance(id).is_err());
    }
    // Asking again cannot turn a no into a yes, nor can a revision.
    assert!(w.mother.request_consent(id, &Yes).is_err());
    assert!(w.mother.revise(id, contract()).is_err());
    assert!(w.mother.submit(id).is_err());
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Denied));
    assert!(w.mother.advance(id).is_err());
    assert!(w.mother.request_consent(id, &Yes).is_err());
    assert_eq!(w.port.calls(), vec![], "a denied mission reached the port");
    assert_eq!(w.keyed_calls(), 0, "a lane ran for a denied mission");
}

#[test]
fn deny_a_broker_refusal_blocks_the_mission_and_nothing_more_is_dispatched() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Err(refusal("no_covering_grant")));
    let built = w.mother.advance(id).unwrap();
    assert_eq!(built.to, MissionState::Blocked);
    assert_eq!(
        w.mother.reason(id).as_deref(),
        Some("broker_refused:no_covering_grant")
    );
    assert_eq!(w.port.effects(), vec![(id, EffectKind::Network)]);
    assert!(w.port.is_cancelled(id));
    assert!(w.mother.advance(id).is_err());
    let w = w.restart();
    assert_eq!(w.mother.state(id), Some(MissionState::Blocked));
    assert!(w.mother.advance(id).is_err());
    assert_eq!(w.port.effects().len(), 1);
}

/// M2a (Owner decision): Mother waits for the broker's approval instead of
/// blocking. This replaces M1's `..._blocks_the_mission`.
#[test]
fn deny_an_effect_awaiting_the_owner_s_native_approval_waits_and_sends_nothing_more() {
    let w = world();
    let id = w.approved(contract_with("ASK-BUILDER-NETWORK ASK-BUILDER-PROCESS"));
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    w.port.reply(Ok(EffectOutcome::AwaitingApproval));
    let built = w.mother.advance(id).unwrap();
    assert_eq!(built.to, MissionState::AwaitingApproval);
    assert_eq!(w.mother.reason(id), None);
    assert_eq!(w.port.effects().len(), 1);
    assert!(!w.port.is_cancelled(id), "the pending action is not withdrawn");
}

#[test]
fn deny_a_file_request_that_is_not_relative_never_reaches_the_port() {
    for path in [
        "/etc/passwd",
        "../outside",
        "a/../../b",
        "",
        "C:\\x",
        "\\\\host\\x",
    ] {
        let lane = Arc::new(
            crate::lane::ScriptedLane::new(LaneRole::Builder, "writes a file", Verdict::Continue)
                .asking(vec![EffectRequest::File {
                    relative_path: path.into(),
                    write: true,
                }]),
        );
        let mut lanes: Vec<Arc<dyn Lane>> = vec![lane];
        lanes.extend(
            keyed_lanes()
                .into_iter()
                .filter(|l| l.identity().role != LaneRole::Builder),
        );
        let w2 = world_with(lanes);
        let id = w2.approved(contract());
        w2.mother.advance(id).unwrap();
        w2.mother.advance(id).unwrap();
        assert_eq!(
            w2.mother.advance(id).unwrap().to,
            MissionState::Blocked,
            "{path}"
        );
        assert_eq!(
            w2.mother.reason(id).as_deref(),
            Some("file_request_not_relative"),
            "{path}"
        );
        assert_eq!(w2.port.effects(), vec![], "{path}");
    }
}

#[test]
fn deny_a_file_request_without_a_requested_folder_never_reaches_the_port() {
    let w = world();
    let mut c = contract_with("ASK-BUILDER-FILE");
    c.requested_folder = None;
    let id = w.approved(c);
    w.mother.advance(id).unwrap();
    w.mother.advance(id).unwrap();
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Blocked);
    assert_eq!(w.mother.reason(id).as_deref(), Some("no_requested_folder"));
    assert_eq!(w.port.effects(), vec![]);
}
