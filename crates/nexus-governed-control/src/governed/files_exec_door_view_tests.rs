//! M2d-C (the Architect's decision on the missing broker API; binding §11
//! M4′): a door sees a Files publication's interruption. The view of a
//! Succeeded commitment carries exactly the controlled codes its effect
//! reported beside the success (the Finished record's `interrupted`
//! detail), in the registry's order; the view of any other commitment
//! carries none. Mother reads it only through `AgentDoor::commitment`
//! (`Lookup::Mine`). Pre-registered before the field exists. A child of the
//! B2 oracles, for their fixture.

use super::*;
use crate::authority::commitment::CommitmentState;
use crate::governed::Lookup;

/// The broker's interruption registry, in its order.
const REGISTRY: [&str; 7] = [
    "emergency_stop",
    "cancelled",
    "revoked",
    "expired",
    "commitment_expired",
    "write_grant_revoked",
    "write_grant_expired",
];

fn door_view(f: &Fx, id: CommitmentId) -> CommitmentView {
    match f.door.commitment(id) {
        Lookup::Mine(view) => view,
        other => panic!("the door's own commitment: {other:?}"),
    }
}

/// The Finished record's `interrupted` values, in the registry's order.
fn recorded(f: &Fx, id: CommitmentId) -> Vec<String> {
    let detail = f.finished(id).detail;
    REGISTRY
        .iter()
        .filter(|code| detail.iter().any(|(k, v)| k == "interrupted" && v == *code))
        .map(|code| code.to_string())
        .collect()
}

/// Every cause after the handoff: the one publication completes, and the
/// door's view names the cause, exactly as the evidence does.
#[test]
fn m2d_c_the_door_sees_an_interruption_after_the_handoff() {
    for (cause, code) in [
        (Cause::EmergencyStop, "emergency_stop"),
        (Cause::Cancel, "cancelled"),
        (Cause::Revoke, "revoked"),
        (Cause::GrantExpiry, "expired"),
        (Cause::DeadlineMonotonic, "commitment_expired"),
        (Cause::DeadlineWall, "commitment_expired"),
        (Cause::KernelStop, "emergency_stop"),
        (Cause::WriteGrant, "write_grant_revoked"),
    ] {
        let f = cause.fixture();
        let view = f.ready(NAME, CONTENT);
        assert!(door_view(&f, view.id).interrupted.is_empty(), "{cause:?}");
        cause.arm(&f, at::FILE_AFTER_FINAL);
        f.execute(&view).expect("the one publication completes");
        let seen = door_view(&f, view.id);
        assert_eq!(seen.state, CommitmentState::Succeeded, "{cause:?}");
        assert!(seen.interrupted.contains(&code), "{cause:?}: {seen:?}");
        let expected = recorded(&f, view.id);
        assert_eq!(seen.interrupted, expected, "{cause:?}: the evidence's");
    }
}

/// Nothing interrupted, or nothing published: no code. Another door of the
/// same agent is not shown the commitment at all.
#[test]
fn m2d_c_the_door_sees_no_interruption_otherwise() {
    let f = Fx::new();
    f.create(NAME, CONTENT).expect("a plain success");
    let plain = f
        .gov
        .authority()
        .commitments()
        .views_of_run(f.run)
        .into_iter()
        .find(|v| v.state == CommitmentState::Succeeded)
        .expect("the success");
    let seen = door_view(&f, plain.id);
    assert!(seen.interrupted.is_empty(), "{seen:?}");
    assert!(recorded(&f, plain.id).is_empty());
    let other = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    assert!(matches!(other.commitment(plain.id), Lookup::NotMine { .. }));

    let g = Fx::new();
    let view = g.ready(NAME, CONTENT);
    Cause::Revoke.arm(&g, at::FILE_BEFORE_FINAL);
    assert!(g.execute(&view).is_err(), "ordering A publishes nothing");
    let seen = door_view(&g, view.id);
    assert_ne!(seen.state, CommitmentState::Succeeded);
    assert!(seen.interrupted.is_empty(), "{seen:?}");
}
