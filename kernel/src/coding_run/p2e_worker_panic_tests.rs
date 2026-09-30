//! P2-ENTRY-H1-R1 tests: a panic in the governed worker body fails the run
//! closed through the real boundary, [`CodingRun::guard_worker`], which the
//! desktop's coding worker thread wraps around all of its work. The run's
//! staging grant and its own project read grant are revoked first, the run
//! requires recovery (`WorkerPanicked`, or the stronger revocation failure),
//! the panic payload is never recorded or returned, and the owner's project is
//! untouched. `p2e_r1_*` are the repair's controls.

use super::p1_apply::{default_edits, env, Env};
use super::p2e_read_grant::{
    assert_read_revoked, final_state_event, live, owned_run, staged_owned, verified_owned,
    RevokeFails, Yes,
};
use super::*;
use crate::workspace_authority::WorkspaceAuthorityError;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Distinctive fragments of the panic payload; no record or result may hold
/// any of them.
const PAYLOAD_TOKENS: [&str; 3] = [
    "P2E-H1-R1-PAYLOAD",
    "/home/owner/secret-project",
    "token=hunter2",
];

fn panic_with_payload() {
    panic!(
        "{}: {} {}",
        PAYLOAD_TOKENS[0], PAYLOAD_TOKENS[1], PAYLOAD_TOKENS[2]
    );
}

fn panicked(reason: RecoveryReason) -> Result<(), WorkerPanic> {
    Err(WorkerPanic {
        state: RunState::RecoveryRequired(reason),
    })
}

fn assert_staging_revoked(e: &Env, grant: WorkspaceGrantId, binding: WorkspaceBinding) {
    assert_eq!(
        e.f.registry.resolve(grant, binding).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant,
        "the staging grant must be revoked"
    );
}

/// Nothing of the payload reached the run's durable record.
fn assert_no_payload_recorded(e: &Env, run: &CodingRun) {
    for record in e.f.ledger.verify_run(run.id().ledger_key()).unwrap() {
        for token in PAYLOAD_TOKENS {
            assert!(
                !record.payload.contains(token),
                "{}: {}",
                record.event_kind,
                record.payload
            );
        }
    }
}

/// A local model that proposes one edit, then panics on its next turn.
struct PanicsOnSecondTurn {
    pin: ModelPin,
    calls: AtomicUsize,
}

impl LocalModel for PanicsOnSecondTurn {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }
    fn complete(
        &self,
        _messages: &[ModelMessage],
        _timeout: Duration,
        _max: u64,
    ) -> Result<String, ModelError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(json!({"action": "edit", "edits": [
                {"path": "src/lib.rs", "op": "replace", "content": "pub fn answer() -> u32 { 42 }\n"}
            ]})
            .to_string());
        }
        panic_with_payload();
        unreachable!()
    }
}

#[test]
fn p2e_r1_01_a_panic_while_staged_closes_both_grants_and_requires_recovery() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    assert!(live(&e, read, binding) && live(&e, staging, binding));
    let outcome = run.guard_worker(|run| {
        run.edit(default_edits().remove(0)).unwrap();
        panic_with_payload();
    });
    assert_eq!(outcome, panicked(RecoveryReason::WorkerPanicked));
    assert_eq!(
        run.state(),
        RunState::RecoveryRequired(RecoveryReason::WorkerPanicked)
    );
    assert_read_revoked(&e, read, binding);
    assert_staging_revoked(&e, staging, binding);
    assert_eq!(
        final_state_event(&e, &run),
        (
            "run.recovery_required".to_string(),
            "WorkerPanicked".to_string()
        )
    );
    // The staging copy is kept for an explicit discard; no removal is claimed.
    assert_eq!(run.cleanup_status(), CleanupStatus::NotStarted);
    assert!(run.staging_path_for_test().unwrap().is_dir());
    // The owner's project was never touched, and nothing of the payload was
    // recorded or returned.
    assert_eq!(digest(&e.f.project), before);
    assert_no_payload_recorded(&e, &run);
    for token in PAYLOAD_TOKENS {
        assert!(!format!("{outcome:?}").contains(token));
    }
    // An explicit discard then removes the staging copy; the outcome stays.
    assert_eq!(run.discard_staging(), Ok(CleanupStatus::Discarded));
    assert_eq!(
        run.state(),
        RunState::RecoveryRequired(RecoveryReason::WorkerPanicked)
    );
}

#[test]
fn p2e_r1_02_a_panic_before_staging_revokes_the_read_grant() {
    let e = env();
    let (mut run, binding, read) = owned_run(&e);
    assert!(live(&e, read, binding));
    let outcome = run.guard_worker(|_run| panic_with_payload());
    assert_eq!(outcome, panicked(RecoveryReason::WorkerPanicked));
    assert_read_revoked(&e, read, binding);
    assert!(run.staging_path_for_test().is_none(), "nothing staged");
    assert_no_payload_recorded(&e, &run);
}

#[test]
fn p2e_r1_03_a_model_panic_inside_the_worker_fails_closed() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    let model = PanicsOnSecondTurn {
        pin: local_model::pin_for_test("qwen2.5-coder:7b"),
        calls: AtomicUsize::new(0),
    };
    run.pin_model(model.pin().clone()).unwrap();
    // The first turn's edit reached staging; the second turn panicked.
    let outcome = run.guard_worker(|run| run_worker(run, &model, "Make answer() return 42."));
    assert_eq!(
        outcome.map(|_| ()),
        panicked(RecoveryReason::WorkerPanicked)
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    assert_read_revoked(&e, read, binding);
    assert_staging_revoked(&e, staging, binding);
    // The half-done candidate can never be verified or approved.
    assert!(matches!(
        run.verify_structural(),
        Err(RunError::InvalidState { .. })
    ));
    assert!(matches!(
        run.request_approval(&e.info.name, &Yes),
        Err(ApplyError::InvalidState)
    ));
    assert_eq!(digest(&e.f.project), before);
    assert_no_payload_recorded(&e, &run);
}

#[test]
fn p2e_r1_04_a_panic_after_verification_withdraws_the_candidate() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding, read) = verified_owned(&e);
    assert_read_revoked(&e, read, binding);
    let outcome = run.guard_worker(|run| {
        run.review().unwrap();
        panic_with_payload();
    });
    assert_eq!(outcome, panicked(RecoveryReason::WorkerPanicked));
    // No stale verification survives, so apply cannot even be requested.
    assert!(run.verification().is_none());
    assert!(matches!(
        run.request_approval(&e.info.name, &Yes),
        Err(ApplyError::InvalidState)
    ));
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p2e_r1_05_a_failed_read_revocation_is_reported_not_hidden() {
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    {
        let _fails = RevokeFails::set();
        let outcome = run.guard_worker(|_run| panic_with_payload());
        assert_eq!(outcome, panicked(RecoveryReason::ProjectRevocationFailed));
        assert!(live(&e, read, binding), "the live read grant is reported");
        assert_staging_revoked(&e, staging, binding);
        assert_eq!(
            final_state_event(&e, &run),
            (
                "run.recovery_required".to_string(),
                "ProjectRevocationFailed".to_string()
            )
        );
    }
    // An explicit discard retries the closure; the outcome is not rewritten.
    assert_eq!(run.discard_staging(), Ok(CleanupStatus::Discarded));
    assert_read_revoked(&e, read, binding);
    assert_eq!(
        run.state(),
        RunState::RecoveryRequired(RecoveryReason::ProjectRevocationFailed)
    );
}

#[test]
fn p2e_r1_06_a_failed_staging_revocation_takes_precedence() {
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    run.substitute_unrevocable_staging_grant_for_test();
    let outcome = run.guard_worker(|_run| panic_with_payload());
    assert_eq!(outcome, panicked(RecoveryReason::StagingRevocationFailed));
    assert!(
        live(&e, staging, binding),
        "the live staging grant is reported"
    );
    // Closing the read grant does not depend on the staging closure.
    assert_read_revoked(&e, read, binding);

    // Both closures failing keeps the staging failure, never WorkerPanicked.
    let (mut run, binding, read) = staged_owned(&e);
    run.substitute_unrevocable_staging_grant_for_test();
    let _fails = RevokeFails::set();
    let outcome = run.guard_worker(|_run| panic_with_payload());
    assert_eq!(outcome, panicked(RecoveryReason::StagingRevocationFailed));
    assert!(live(&e, read, binding));
}

#[test]
fn p2e_r1_07_a_body_that_does_not_panic_is_untouched() {
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    let state = run.guard_worker(|run| {
        for edit in default_edits() {
            run.edit(edit).unwrap();
        }
        run.state()
    });
    assert_eq!(state, Ok(RunState::Candidate));
    assert!(live(&e, read, binding) && live(&e, staging, binding));
    // The run continues normally: verification passes and closes the read
    // grant at the H1 point.
    assert!(run.verify_structural().unwrap().passed());
    assert_read_revoked(&e, read, binding);

    // A body's own error passes through unchanged and ends nothing.
    let (mut run, binding, read) = staged_owned(&e);
    let refused = run.guard_worker(|run| run.edit(replace("tests/check.rs", "// weakened\n")));
    assert_eq!(
        refused,
        Ok(Err(RunError::EditRejected(EditRejection::ProtectedInput)))
    );
    assert_eq!(run.state(), RunState::Staged);
    assert!(live(&e, read, binding));
}

#[test]
fn p2e_r1_08_a_panic_after_the_run_ended_keeps_its_recorded_outcome() {
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    let staging = run.staging_grant_for_test().unwrap();
    let outcome = run.guard_worker(|run| {
        run.cancel().unwrap();
        panic_with_payload();
    });
    // Terminal outcomes are never rewritten; the authority is closed.
    assert_eq!(
        outcome,
        Err(WorkerPanic {
            state: RunState::Cancelled
        })
    );
    assert_read_revoked(&e, read, binding);
    assert_staging_revoked(&e, staging, binding);
    assert_eq!(
        final_state_event(&e, &run),
        ("run.cancelled".to_string(), "cancelled".to_string())
    );
    assert_no_payload_recorded(&e, &run);
}
