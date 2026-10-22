//! Phase Two P2H tests: a run's sandboxed verification lifecycle, the
//! owner's native single-use launch approval, the result bound into the
//! review, and the Apply gates. `p2h_nc_*` are the mission's controls (37-42
//! and the lifecycle invariants).

use super::p1_apply::{approve_and_apply, default_edits, env, verified, verified_with, Env};
use super::*;
use crate::coding_run::verifier::VerifierLaunchApproval as Approval;
use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::sync::atomic::AtomicBool;

struct Launcher {
    yes: bool,
    shown: RefCell<Vec<VerifierLaunchRequest>>,
}

impl Launcher {
    fn yes() -> Self {
        Self {
            yes: true,
            shown: RefCell::new(Vec::new()),
        }
    }
    fn no() -> Self {
        Self {
            yes: false,
            shown: RefCell::new(Vec::new()),
        }
    }
}

impl VerifierLaunchConfirmer for Launcher {
    fn confirm_launch(&self, request: &VerifierLaunchRequest) -> bool {
        self.shown.borrow_mut().push(request.clone());
        self.yes
    }
}

fn inputs(toolchain_generation: u64) -> VerifierInputs {
    VerifierInputs {
        profile_hash: [1; 32],
        toolchain_digest: [2; 32],
        toolchain_generation,
        sandbox_policy_hash: [4; 32],
        resource_policy_hash: [5; 32],
    }
}

fn facts() -> VerifierLaunchFacts {
    VerifierLaunchFacts {
        profile_display: "Rust library tests (offline)".to_string(),
        wall_timeout_secs: 600,
        memory_max_bytes: 4 << 30,
        cpus: 4,
        processes: 256,
    }
}

fn stream(fill: u8) -> StreamSummary {
    StreamSummary {
        bytes: 100,
        sha256: [fill; 32],
        truncated: false,
    }
}

fn outcome(exit: VerifierExit, cleanup: VerifierCleanup) -> VerifierOutcome {
    VerifierOutcome {
        exit,
        duration_ms: 1500,
        stdout: stream(7),
        stderr: stream(8),
        cleanup,
    }
}

fn input_dir(e: &Env) -> OwnedFd {
    let path =
        e.f.staging_parent
            .parent()
            .unwrap()
            .join(format!("p2h-input-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    OwnedFd::from(std::fs::File::open(&path).unwrap())
}

/// Prepare, materialize and approve a launch of `inputs`.
fn approved(e: &Env, run: &mut CodingRun, inputs: VerifierInputs) -> Approval {
    run.prepare_verification(inputs).unwrap();
    run.materialize_verification_input(input_dir(e)).unwrap();
    run.request_verification_approval(&facts(), &Launcher::yes())
        .unwrap()
}

/// A verification that started with `inputs(1)`.
fn launched(e: &Env, run: &mut CodingRun) -> ExecutionGeneration {
    let approval = approved(e, run, inputs(1));
    run.begin_verification(approval, inputs(1)).unwrap()
}

/// A finalized verification with `exit` and `cleanup`.
fn finished(
    e: &Env,
    run: &mut CodingRun,
    exit: VerifierExit,
    cleanup: VerifierCleanup,
) -> VerificationResult {
    let generation = launched(e, run);
    run.verification_running(generation).unwrap();
    run.verification_finalizing(generation).unwrap();
    run.finish_verification(generation, outcome(exit, cleanup))
        .unwrap()
}

fn verification_events(e: &Env, run: &CodingRun) -> Vec<String> {
    e.f.ledger
        .verify_run(run.id().ledger_key())
        .unwrap()
        .into_iter()
        .map(|record| record.event_kind)
        .filter(|kind| kind.starts_with("verify.") && kind != "verify.structural")
        .collect()
}

#[test]
fn p2h_the_lifecycle_binds_the_result_into_the_review() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    let before = run.review().unwrap();
    assert_eq!(before.binding.verification, VerifierMarker::NoResult);
    assert!(before.verification.is_none());

    let binding = run.prepare_verification(inputs(1)).unwrap();
    assert_eq!(run.verification_phase(), VerificationPhase::Prepared);
    assert_eq!(binding.run_id, run.id());
    assert_eq!(
        binding.candidate_manifest_hash,
        run.verification().unwrap().candidate_manifest_hash
    );
    run.materialize_verification_input(input_dir(&e)).unwrap();
    assert_eq!(run.verification_phase(), VerificationPhase::Materialized);
    let launcher = Launcher::yes();
    let approval = run
        .request_verification_approval(&facts(), &launcher)
        .unwrap();
    assert_eq!(*approval.binding(), binding);
    assert_eq!(run.verification_phase(), VerificationPhase::Approved);
    // The native prompt: bounded facts, no path or secret.
    let shown = launcher.shown.borrow()[0].clone();
    assert!(shown.message().contains("no network access"));
    assert!(shown
        .message()
        .contains("600 s, 4096 MiB memory, 4 CPUs, 256 processes"));
    assert!(!shown.message().contains('/'));

    let generation = run.begin_verification(approval, inputs(1)).unwrap();
    assert_eq!(generation.get(), 1);
    assert_eq!(run.verification_phase(), VerificationPhase::Starting);
    run.verification_running(generation).unwrap();
    run.verification_finalizing(generation).unwrap();
    let result = run
        .finish_verification(
            generation,
            outcome(VerifierExit::Passed, VerifierCleanup::Confirmed),
        )
        .unwrap();
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    assert!(result.is_for(&binding));
    assert_eq!(run.latest_verification(), Some(&result));

    let after = run.review().unwrap();
    assert_eq!(
        after.binding.verification,
        VerifierMarker::Result(result.binding_hash())
    );
    assert_eq!(after.verification, Some(result));
    assert_ne!(after.binding.hash(), before.binding.hash());
    assert_eq!(
        verification_events(&e, &run),
        [
            "verify.prepared",
            "verify.approval_granted",
            "verify.launch",
            "verify.result"
        ]
    );
}

#[test]
fn p2h_nc_37_an_approval_for_another_launch_or_run_cannot_start_one() {
    let e = env();
    let (mut first, _) = verified(&e, default_edits());
    let (mut second, _) = verified(&e, default_edits());
    // Nothing approved yet: even an in-crate approval starts nothing.
    let foreign = approved(&e, &mut first, inputs(1));
    assert_eq!(
        second.begin_verification(foreign, inputs(1)),
        Err(VerificationError::InvalidState)
    );
    // Another run's approval of its own launch is not this run's.
    // `second` holds its own approval; it is set aside unused.
    let _own = approved(&e, &mut second, inputs(1));
    let other_binding = *approved(&e, &mut first, inputs(1)).binding();
    let forged = Approval::confirmed(other_binding);
    assert_eq!(
        second.begin_verification(forged, inputs(1)),
        Err(VerificationError::Stale)
    );
    assert_eq!(second.verification_phase(), VerificationPhase::Idle);
    assert!(!verification_events(&e, &second).contains(&"verify.launch".to_string()));
}

#[test]
fn p2h_nc_38_a_stale_approval_cannot_launch() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    // The toolchain was verified again: another generation, another launch.
    let approval = approved(&e, &mut run, inputs(1));
    assert_eq!(
        run.begin_verification(approval, inputs(2)),
        Err(VerificationError::Stale)
    );
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    // Declined: nothing is approved, and it is recorded.
    run.prepare_verification(inputs(1)).unwrap();
    run.materialize_verification_input(input_dir(&e)).unwrap();
    assert_eq!(
        run.request_verification_approval(&facts(), &Launcher::no())
            .err(),
        Some(VerificationError::Declined)
    );
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    // Approval needs the prepared launch to be materialized first.
    run.prepare_verification(inputs(1)).unwrap();
    assert_eq!(
        run.request_verification_approval(&facts(), &Launcher::yes())
            .err(),
        Some(VerificationError::InvalidState)
    );
    assert!(verification_events(&e, &run).contains(&"verify.approval_declined".to_string()));
    assert!(!verification_events(&e, &run).contains(&"verify.launch".to_string()));
}

#[test]
fn p2h_nc_39_a_result_binds_only_this_run_and_candidate() {
    let e = env();
    let (mut first, _) = verified(&e, default_edits());
    let (mut second, _) = verified(&e, default_edits());
    let result = finished(
        &e,
        &mut first,
        VerifierExit::Passed,
        VerifierCleanup::Confirmed,
    );
    assert_eq!(result.run_id, first.id());
    // The other run's review binds no result.
    assert_eq!(
        second.review().unwrap().binding.verification,
        VerifierMarker::NoResult
    );
    // A finish for a generation that is not running binds nothing.
    let generation = launched(&e, &mut second);
    let wrong = ExecutionGeneration::FIRST.next().unwrap();
    assert_ne!(wrong, generation);
    assert_eq!(
        second.finish_verification(
            wrong,
            outcome(VerifierExit::Passed, VerifierCleanup::Confirmed)
        ),
        Err(VerificationError::InvalidState)
    );
    assert!(second.latest_verification().is_none());
}

#[test]
fn p2h_nc_40_a_rerun_invalidates_the_owner_approval() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    finished(
        &e,
        &mut run,
        VerifierExit::Passed,
        VerifierCleanup::Confirmed,
    );
    // Approved against the first result…
    let owner = run
        .request_approval(&e.info.name, &p1_apply::Confirm::yes())
        .unwrap();
    // …then verified again: the approval is no longer this review.
    let second = finished(
        &e,
        &mut run,
        VerifierExit::Failed { exit_code: 101 },
        VerifierCleanup::Confirmed,
    );
    assert_eq!(second.generation.get(), 2);
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(owner, grant, &parent(&e.f)).err(),
        Some(ApplyError::Refused(Refusal::ApprovalMismatch))
    );
    assert_eq!(run.apply_state(), ApplyState::NotApplied);
}

#[test]
fn p2h_nc_41_an_active_or_uncleaned_verification_blocks_apply() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let owner = run
        .request_approval(&e.info.name, &p1_apply::Confirm::yes())
        .unwrap();
    let generation = launched(&e, &mut run);
    for step in 0..3 {
        match step {
            1 => run.verification_running(generation).unwrap(),
            2 => run.verification_finalizing(generation).unwrap(),
            _ => {}
        }
        assert_eq!(
            run.request_approval(&e.info.name, &p1_apply::Confirm::yes())
                .err(),
            Some(ApplyError::Refused(Refusal::VerificationInProgress)),
            "{:?}",
            run.verification_phase()
        );
        // Another verification cannot be prepared meanwhile.
        assert_eq!(
            run.prepare_verification(inputs(1)),
            Err(VerificationError::InvalidState)
        );
    }
    // An approval obtained earlier cannot be used either.
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(owner, grant, &parent(&e.f)).err(),
        Some(ApplyError::Refused(Refusal::VerificationInProgress))
    );
    // An unconfirmed cleanup keeps Apply refused until it is confirmed.
    run.finish_verification(
        generation,
        outcome(VerifierExit::Passed, VerifierCleanup::Failed),
    )
    .unwrap();
    assert_eq!(run.verification_phase(), VerificationPhase::CleanupFailed);
    assert_eq!(
        run.request_approval(&e.info.name, &p1_apply::Confirm::yes())
            .err(),
        Some(ApplyError::Refused(Refusal::VerificationCleanupFailed))
    );
    assert_eq!(
        run.prepare_verification(inputs(1)),
        Err(VerificationError::InvalidState)
    );
    run.confirm_verification_cleanup(generation).unwrap();
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    approve_and_apply(&e, &mut run, binding).unwrap();
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert!(verification_events(&e, &run).contains(&"verify.cleanup".to_string()));
}

#[test]
fn p2r1_a_panicked_verification_keeps_apply_refused_until_its_cleanup_is_confirmed() {
    // What the desktop records when its verification thread panics with the
    // execution's boundary retained, from every active phase: an unknown
    // result whose cleanup is unconfirmed.
    for active in 0..3 {
        let e = env();
        let (mut run, binding) = verified(&e, default_edits());
        let generation = launched(&e, &mut run);
        if active >= 1 {
            run.verification_running(generation).unwrap();
        }
        if active >= 2 {
            run.verification_finalizing(generation).unwrap();
        }
        let result = run
            .finish_verification(
                generation,
                outcome(VerifierExit::CleanupFailed, VerifierCleanup::Failed),
            )
            .unwrap();
        assert!(!result.outcome.exit.passed());
        assert_eq!(run.verification_phase(), VerificationPhase::CleanupFailed);
        assert_eq!(
            run.request_approval(&e.info.name, &p1_apply::Confirm::yes())
                .err(),
            Some(ApplyError::Refused(Refusal::VerificationCleanupFailed)),
            "{active}"
        );
        // Only the retained boundary's confirmed cleanup releases Apply.
        run.confirm_verification_cleanup(generation).unwrap();
        approve_and_apply(&e, &mut run, binding).unwrap();
        assert_eq!(run.apply_state(), ApplyState::Applied);
    }
    // A panic whose cleanup was confirmed is an unknown result: advisory,
    // never a pass.
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let result = finished(
        &e,
        &mut run,
        VerifierExit::SandboxFailed,
        VerifierCleanup::Confirmed,
    );
    assert!(!result.outcome.exit.passed());
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    approve_and_apply(&e, &mut run, binding).unwrap();
}

#[test]
fn p2h_nc_42_a_failed_verification_with_clean_cleanup_is_advisory() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let result = finished(
        &e,
        &mut run,
        VerifierExit::Failed { exit_code: 101 },
        VerifierCleanup::Confirmed,
    );
    assert!(!result.outcome.exit.passed());
    let review = run.review().unwrap();
    assert_eq!(review.verification, Some(result));
    let report = approve_and_apply(&e, &mut run, binding).unwrap();
    assert_eq!(report.files.len(), 2);
    assert_eq!(run.apply_state(), ApplyState::Applied);
    // Applied: no further verification.
    assert_eq!(
        run.prepare_verification(inputs(1)),
        Err(VerificationError::InvalidState)
    );
}

#[test]
fn p2h_generations_only_increase_and_one_execution_runs_at_a_time() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    let first = finished(
        &e,
        &mut run,
        VerifierExit::Passed,
        VerifierCleanup::Confirmed,
    );
    let second = finished(
        &e,
        &mut run,
        VerifierExit::TimedOut,
        VerifierCleanup::Confirmed,
    );
    assert!(second.generation > first.generation);
    assert_eq!(run.latest_verification(), Some(&second));
    // Out-of-order steps are refused.
    let generation = launched(&e, &mut run);
    assert_eq!(
        run.verification_running(ExecutionGeneration::FIRST),
        Err(VerificationError::InvalidState)
    );
    run.verification_running(generation).unwrap();
    assert_eq!(
        run.verification_running(generation),
        Err(VerificationError::InvalidState)
    );
    assert!(run.materialize_verification_input(input_dir(&e)).is_err());
    assert_eq!(
        run.abandon_verification(),
        Err(VerificationError::InvalidState)
    );
    assert_eq!(
        run.confirm_verification_cleanup(generation),
        Err(VerificationError::InvalidState)
    );
}

/// A ledger that can be switched to fail every append.
struct SwitchStore {
    inner: Arc<CodingRunLedger>,
    fail: AtomicBool,
}

impl LedgerStore for SwitchStore {
    fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(LedgerFailure::Unavailable);
        }
        LedgerStore::append(self.inner.as_ref(), event)
    }
    fn verified_records(&self, run: Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
        self.inner.verified_records(run)
    }
}

#[test]
fn p2h_an_unrecordable_step_does_not_happen() {
    let e = env();
    let store = Arc::new(SwitchStore {
        inner: Arc::clone(&e.f.ledger),
        fail: AtomicBool::new(false),
    });
    let (mut run, _) = verified_with(
        &e,
        Arc::clone(&store) as Arc<dyn LedgerStore>,
        default_edits(),
    );
    store.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        run.prepare_verification(inputs(1)),
        Err(VerificationError::Unrecorded)
    );
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    store.fail.store(false, Ordering::SeqCst);
    let approval = approved(&e, &mut run, inputs(1));
    store.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        run.begin_verification(approval, inputs(1)),
        Err(VerificationError::Unrecorded)
    );
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
    store.fail.store(false, Ordering::SeqCst);
    let generation = launched(&e, &mut run);
    store.fail.store(true, Ordering::SeqCst);
    // An unrecorded result is never review evidence.
    assert_eq!(
        run.finish_verification(
            generation,
            outcome(VerifierExit::Passed, VerifierCleanup::Confirmed)
        ),
        Err(VerificationError::Unrecorded)
    );
    assert!(run.latest_verification().is_none());
    assert_eq!(run.verification_phase(), VerificationPhase::Idle);
}

#[test]
fn p2h_applicability_sees_exactly_the_verified_candidate() {
    let e = env();
    let (run, _) = verified(&e, default_edits());
    let candidate = run.verification_candidate().unwrap();
    let paths = candidate.paths();
    assert!(paths.contains("src/lib.rs") && paths.contains("src/new/deep.rs"));
    assert!(!paths.iter().any(|p| p.contains(".git")));
    assert_eq!(
        candidate.read("src/lib.rs", 1 << 20).unwrap(),
        b"pub fn answer() -> u32 { 42 }\n"
    );
    assert!(candidate.read("src/lib.rs", 4).is_none(), "bounded");
    assert!(candidate.read("missing.rs", 1 << 20).is_none());
    assert!(candidate.read("../escape", 1 << 20).is_none());
}
