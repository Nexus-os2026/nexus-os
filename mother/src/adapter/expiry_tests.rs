//! Codex review MINOR (oracle first): what the broker's record of a
//! waiting action says, by the port's mapping. Expired by either clock: the
//! wall clock (the view's expiry) or the monotonic deadline the port took
//! before asking (which a wall clock set back does not delay).

use super::status_of;
use crate::effect::ApprovalStatus;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::authority::ids::{AgentId, CommitmentId, RunId};

const CREATED: u64 = 1_000_000;
const EXPIRES: u64 = CREATED + 600_000;
const DEADLINE: u64 = 50_000 + 600_000;

fn view(state: CommitmentState) -> CommitmentView {
    CommitmentView {
        id: CommitmentId::parse("cmt-00000000000000000000000000000001").unwrap(),
        agent: AgentId::new("mother").unwrap(),
        run: RunId::parse("run-00000000000000000000000000000001").unwrap(),
        kind: CapabilityKind::Egress,
        class: EffectClass::R2,
        operation: "egress.request",
        target: "http://127.0.0.1/".into(),
        summary: vec![],
        state,
        requires_approval: true,
        created_wall_ms: CREATED,
        expires_wall_ms: EXPIRES,
        binding_short: "0000".into(),
    }
}

#[test]
fn expiry_the_monotonic_deadline_expires_a_waiting_action_whatever_the_wall_clock() {
    for state in [CommitmentState::Prepared, CommitmentState::Authorized] {
        // The wall clock was set back before the action was created; the
        // monotonic deadline has passed.
        for wall in [0, CREATED - 1, CREATED, EXPIRES - 1] {
            assert_eq!(
                status_of(&view(state), wall, DEADLINE, DEADLINE),
                ApprovalStatus::Expired,
                "{state:?} wall {wall}"
            );
            assert_eq!(
                status_of(&view(state), wall, DEADLINE + 1, DEADLINE),
                ApprovalStatus::Expired,
                "{state:?} wall {wall}"
            );
        }
        // Neither clock has passed: still waiting.
        assert_eq!(
            status_of(&view(state), EXPIRES - 1, DEADLINE - 1, DEADLINE),
            ApprovalStatus::Waiting
        );
        // The wall clock alone has passed.
        assert_eq!(
            status_of(&view(state), EXPIRES, DEADLINE - 1, DEADLINE),
            ApprovalStatus::Expired
        );
    }
}

#[test]
fn expiry_the_other_states_map_as_the_broker_records_them() {
    let late = (EXPIRES + 1, DEADLINE + 1);
    let cases = [
        // An approved action that is running is not expired by the clock.
        (CommitmentState::Executing, ApprovalStatus::Waiting),
        (CommitmentState::Succeeded, ApprovalStatus::Approved),
        (CommitmentState::Denied, ApprovalStatus::Denied),
        (CommitmentState::Expired, ApprovalStatus::Expired),
        (
            CommitmentState::Failed,
            ApprovalStatus::Lapsed {
                class: "failed".into(),
            },
        ),
        (
            CommitmentState::Cancelled,
            ApprovalStatus::Lapsed {
                class: "cancelled".into(),
            },
        ),
        (
            CommitmentState::Revoked,
            ApprovalStatus::Lapsed {
                class: "revoked".into(),
            },
        ),
    ];
    for (state, expected) in cases {
        assert_eq!(
            status_of(&view(state), late.0, late.1, DEADLINE),
            expected,
            "{state:?}"
        );
        assert_eq!(
            status_of(&view(state), CREATED, 0, DEADLINE),
            expected,
            "{state:?}"
        );
    }
}

/// Codex re-review MINOR (a), oracle first: a journaled deadline is taken
/// no later than the action's remaining lifetime by the broker's
/// wall-clock expiry, and never later than what was journaled.
#[test]
fn expiry_an_adopted_deadline_is_clamped_to_the_remaining_lifetime() {
    use super::adopted_deadline;
    // Journaled honestly (earlier than the bound): kept.
    assert_eq!(
        adopted_deadline(5_000, 1_000, EXPIRES - 10_000, EXPIRES),
        5_000
    );
    // Tampered: clamped to now + (expires - wall now).
    assert_eq!(
        adopted_deadline(u64::MAX, 1_000, EXPIRES - 10_000, EXPIRES),
        11_000
    );
    // Past the wall-clock expiry: no later than now.
    assert_eq!(
        adopted_deadline(u64::MAX, 1_000, EXPIRES + 5, EXPIRES),
        1_000
    );
}
