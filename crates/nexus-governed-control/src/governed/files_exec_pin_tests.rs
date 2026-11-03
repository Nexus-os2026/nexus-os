//! M2d-C round 4 (settlement design v2, rule 2, and the binding addendum's
//! retention lifecycle; pre-registered before the API exists): a Files
//! commitment is pinned from its admission, with its run, and released only
//! by an exact, idempotent, owner-scoped acknowledgement of a conclusive
//! outcome. At capacity new proposals and runs are refused; a pinned entry
//! is never evicted, and its door still reads it. After the door is gone, a
//! replacement door of the same agent may only acknowledge a conclusive
//! obligation (ack-only ownership): it is shown nothing more, and an
//! uncertain outcome is never unpinned. A child of the B2 oracles.

use super::*;
use crate::authority::commitment::PublicationClass;
use crate::governed::Lookup;

fn mine(door: &AgentDoor, id: CommitmentId) -> CommitmentView {
    match door.try_commitment(id).expect("readable") {
        Lookup::Mine(view) => view,
        other => panic!("the door's own commitment: {other:?}"),
    }
}

fn pinned(f: &Fx, id: CommitmentId) -> bool {
    f.gov.authority().commitments().pinned_for_test(id)
}

/// The commitment registry is full: every entry, final ones included,
/// counts until it is pruned.
fn fill_commitments(f: &Fx) {
    let commitments = f.gov.authority().commitments();
    commitments.set_capacity_for_test(commitments.len_for_test());
}

/// A published File stays at capacity, readable; another proposal is
/// refused `capacity` (never by evicting it). Its exact ack releases it and
/// is idempotent; the next proposal then finds room.
#[test]
fn m2d_c_r4_a_pinned_commitment_survives_capacity_until_acked() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.execute(&view).expect("published");
    assert!(pinned(&f, view.id), "pinned from its admission");
    fill_commitments(&f);
    assert_eq!(class(f.propose("b.txt", "b\n")), "capacity");
    assert!(matches!(
        mine(&f.door, view.id).publication,
        PublicationClass::Published { .. }
    ));
    f.door.ack_commitment(view.id).expect("acknowledged");
    assert!(!pinned(&f, view.id));
    f.door.ack_commitment(view.id).expect("idempotent");
    f.propose("b.txt", "b\n").expect("room once released");
}

/// The run of a pinned commitment is retained through its cancellation:
/// at the runs registry's capacity, a new run is refused rather than the
/// retained one pruned, and the door still reads its commitment. Released
/// by the ack, the run is pruned and a new run finds room.
#[test]
fn m2d_c_r4_a_pinned_run_survives_run_pruning() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.execute(&view).expect("published");
    let _ = f.door.cancel_run(f.run).unwrap().write();
    let runs = f.gov.authority().runs();
    runs.set_capacity_for_test(runs.len_for_test());
    assert_eq!(class(f.door.open_run()), "capacity");
    assert_eq!(mine(&f.door, view.id).run, f.run, "still its door's");
    f.door.ack_commitment(view.id).unwrap();
    f.door.open_run().expect("room once released");
}

/// Exact and conclusive only: a pending commitment cannot be acknowledged
/// (and stays pinned); acknowledging one never releases another, however
/// often it is repeated; another agent's door, and another live door of
/// the same agent while the run is this door's, are refused.
#[test]
fn m2d_c_r4_an_ack_is_exact_conclusive_and_owner_scoped() {
    let f = Fx::new();
    let done = f.ready(NAME, CONTENT);
    f.execute(&done).expect("published");
    let waiting = f.propose("b.txt", "b\n").unwrap();
    assert!(f.door.ack_commitment(waiting.id).is_err(), "pending");
    assert!(pinned(&f, waiting.id));
    let other = AgentId::new("other").unwrap();
    let foreign = f.gov.issue_agent_door(other, f.owner.clone()).unwrap();
    assert!(foreign.ack_commitment(done.id).is_err(), "another agent's");
    let twin = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    assert!(twin.ack_commitment(done.id).is_err(), "a live door's run");
    assert!(pinned(&f, done.id));
    for _ in 0..3 {
        f.door.ack_commitment(done.id).unwrap();
    }
    assert!(!pinned(&f, done.id));
    assert!(pinned(&f, waiting.id), "never another commitment");
}

/// After its door is dropped, a replacement door of the same agent may
/// acknowledge a conclusive obligation and nothing more: it is not shown
/// the commitment, joins nothing, and cannot unpin an uncertain one.
#[test]
fn m2d_c_r4_a_replacement_door_has_ack_only_ownership() {
    let mut f = Fx::new();
    let done = f.ready(NAME, CONTENT);
    f.execute(&done).expect("published");
    let replacement = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    let old = std::mem::replace(&mut f.door, Arc::new(replacement));
    drop(old);
    assert!(matches!(
        f.door.try_commitment(done.id).unwrap(),
        Lookup::NotMine { .. }
    ));
    f.door.ack_commitment(done.id).expect("ack-only ownership");
    assert!(!pinned(&f, done.id));
    assert!(matches!(
        f.door.try_commitment(done.id).unwrap(),
        Lookup::NotMine { .. }
    ));

    // An uncertain outcome stays pinned, whoever asks.
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.gov
        .files()
        .hooks()
        .arm(at::FILE_BEFORE_FINAL, || panic!("inside the apply"));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.execute(&view)));
    assert_eq!(
        mine(&f.door, view.id).publication,
        PublicationClass::Uncertain
    );
    assert!(f.door.ack_commitment(view.id).is_err(), "uncertain");
    assert!(pinned(&f, view.id));
}

/// A repeated ack after the release is a no-op whatever became of the run
/// since (pruned once it was cancelled and released): the startup re-ack
/// of an obligation already acknowledged succeeds.
#[test]
fn m2d_c_r4_a_repeated_ack_after_the_run_is_pruned_is_a_no_op() {
    let f = Fx::new();
    let view = f.ready(NAME, CONTENT);
    f.execute(&view).expect("published");
    let _ = f.door.cancel_run(f.run).unwrap().write();
    f.door.ack_commitment(view.id).unwrap();
    let runs = f.gov.authority().runs();
    runs.set_capacity_for_test(runs.len_for_test());
    f.door.open_run().expect("the released run was pruned");
    f.door.ack_commitment(view.id).expect("still a no-op");
}
