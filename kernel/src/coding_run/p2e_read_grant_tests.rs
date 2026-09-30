//! P2-ENTRY-H1 tests: a run created for an owner-selected project owns its
//! fresh read grant and revokes it as soon as it no longer needs project read
//! authority: when structural verification passes, or at any earlier terminal
//! transition. Apply and restore use their own fresh write grants. The
//! grant's expiry and `Drop` are only backstops, and a failed revocation is
//! never reported as a clean outcome. `p2e_h1_rg_*` are the mission's
//! controls.

use super::p1_apply::{default_edits, env, Env};
use super::*;
use crate::workspace_authority::WorkspaceAuthorityError;
use std::sync::Mutex as StdMutex;

struct Yes;

impl OwnerConfirmer for Yes {
    fn confirm(&self, _request: &ConfirmationRequest) -> bool {
        true
    }
}

/// A scripted local model; `finish` once its answers run out.
struct Scripted {
    pin: ModelPin,
    answers: StdMutex<Vec<String>>,
    fail: bool,
}

impl Scripted {
    fn new(answers: Vec<Value>) -> Self {
        Self {
            pin: local_model::pin_for_test("qwen2.5-coder:7b"),
            answers: StdMutex::new(answers.into_iter().map(|a| a.to_string()).collect()),
            fail: false,
        }
    }

    fn unavailable() -> Self {
        Self {
            fail: true,
            ..Self::new(Vec::new())
        }
    }
}

impl LocalModel for Scripted {
    fn pin(&self) -> &ModelPin {
        &self.pin
    }
    fn complete(
        &self,
        _messages: &[ModelMessage],
        _timeout: Duration,
        _max: u64,
    ) -> Result<String, ModelError> {
        if self.fail {
            return Err(ModelError::Unavailable);
        }
        let mut answers = self.answers.lock().unwrap();
        Ok(if answers.is_empty() {
            json!({"action": "finish", "summary": "done"}).to_string()
        } else {
            answers.remove(0)
        })
    }
}

/// Makes revoking a run's read grant fail until dropped.
struct RevokeFails;

impl RevokeFails {
    fn set() -> Self {
        PROJECT_READ_REVOKE_FAILS.with(|fails| fails.set(true));
        Self
    }
}

impl Drop for RevokeFails {
    fn drop(&mut self) {
        PROJECT_READ_REVOKE_FAILS.with(|fails| fails.set(false));
    }
}

fn binding() -> WorkspaceBinding {
    WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    }
}

/// A run over the owner-selected fixture project, holding its own read
/// grant: (run, binding, read grant).
fn owned_run_with(
    e: &Env,
    ledger: Arc<dyn LedgerStore>,
) -> (CodingRun, WorkspaceBinding, WorkspaceGrantId) {
    let binding = binding();
    let grant = e.projects.grant_for_run(e.info.id, binding).unwrap();
    let read = grant.grant_id();
    let run = CodingRun::create_for_project(ledger, grant, default_scopes()).unwrap();
    assert_eq!(run.project_read_grant_for_test(), Some(read));
    (run, binding, read)
}

fn owned_run(e: &Env) -> (CodingRun, WorkspaceBinding, WorkspaceGrantId) {
    owned_run_with(e, Arc::clone(&e.f.ledger) as Arc<dyn LedgerStore>)
}

/// An owned run that has staged the project.
fn staged_owned(e: &Env) -> (CodingRun, WorkspaceBinding, WorkspaceGrantId) {
    let (mut run, binding, read) = owned_run(e);
    run.grant(&parent(&e.f)).unwrap();
    run.snapshot().unwrap();
    (run, binding, read)
}

/// An owned run whose candidate passed structural verification.
fn verified_owned(e: &Env) -> (CodingRun, WorkspaceBinding, WorkspaceGrantId) {
    let (mut run, binding, read) = staged_owned(e);
    for edit in default_edits() {
        run.edit(edit).unwrap();
    }
    assert!(run.verify_structural().unwrap().passed());
    (run, binding, read)
}

fn live(e: &Env, grant: WorkspaceGrantId, binding: WorkspaceBinding) -> bool {
    e.f.registry.resolve(grant, binding).is_ok()
}

fn assert_read_revoked(e: &Env, grant: WorkspaceGrantId, binding: WorkspaceBinding) {
    assert_eq!(
        e.f.registry.resolve(grant, binding).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant,
        "the run's read grant must be revoked"
    );
}

/// The final state-bearing ledger event of a run and its reason.
fn final_state_event(e: &Env, run: &CodingRun) -> (String, String) {
    let record =
        e.f.ledger
            .verify_run(run.id().ledger_key())
            .unwrap()
            .into_iter()
            .rev()
            .find(|record| {
                [
                    "run.cancelled",
                    "run.revoked",
                    "run.failed",
                    "run.recovery_required",
                    "verify.structural",
                ]
                .contains(&record.event_kind.as_str())
            })
            .unwrap();
    let payload: Value = serde_json::from_str(&record.payload).unwrap();
    let reason = payload["reason"].as_str().unwrap_or_default().to_string();
    (record.event_kind, reason)
}

#[test]
fn p2e_h1_rg_01_the_read_grant_lives_only_until_verification_passes() {
    let e = env();
    let (mut run, binding, read) = owned_run(&e);
    assert!(live(&e, read, binding), "issued for the run");
    run.grant(&parent(&e.f)).unwrap();
    assert!(live(&e, read, binding) && run.project_handle_retained_for_test());
    run.snapshot().unwrap();
    assert!(
        !run.project_handle_retained_for_test(),
        "the project handle is kept only for the snapshot copy"
    );
    // The worker phase still re-checks the project before every step.
    assert!(live(&e, read, binding));
    for edit in default_edits() {
        run.edit(edit).unwrap();
        assert!(live(&e, read, binding));
    }
    assert!(run.verify_structural().unwrap().passed());
    assert_eq!(run.state(), RunState::StructurallyVerified);
    assert_read_revoked(&e, read, binding);
    // Review needs no project authority.
    let review = run.review().unwrap();
    assert_eq!(review.changes.len(), 2);
    assert_read_revoked(&e, read, binding);
}

#[test]
fn p2e_h1_rg_02_a_revoked_read_grant_serves_no_run_and_its_recorded_id_grants_nothing() {
    let e = env();
    let (run, binding, read) = verified_owned(&e);
    assert_read_revoked(&e, read, binding);
    // The id the ledger recorded is only a name.
    let created =
        e.f.ledger
            .verify_run(run.id().ledger_key())
            .unwrap()
            .into_iter()
            .find(|record| record.event_kind == "run.created")
            .unwrap();
    let payload: Value = serde_json::from_str(&created.payload).unwrap();
    let recorded: WorkspaceGrantId =
        serde_json::from_value(payload["project_grant"].clone()).unwrap();
    assert_eq!(recorded, read);
    let other = self::binding();
    assert!(e.f.registry.resolve(recorded, other).is_err());
    // A run built on it, for the same or another binding, gets nothing.
    for (who, expected) in [
        (binding, RunError::Revoked(RevocationReason::GrantRevoked)),
        (other, RunError::AuthorityDenied),
    ] {
        let mut stolen = CodingRun::create(
            Arc::clone(&e.f.ledger) as Arc<dyn LedgerStore>,
            Arc::clone(&e.f.registry),
            recorded,
            who,
            default_scopes(),
        )
        .unwrap();
        assert_eq!(stolen.grant(&parent(&e.f)), Err(expected));
        assert!(stolen.state().is_terminal());
        assert!(stolen.staging_path_for_test().is_none(), "nothing staged");
    }
}

#[test]
fn p2e_h1_rg_03_apply_and_restore_use_their_own_fresh_write_grants() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding, read) = verified_owned(&e);
    assert_read_revoked(&e, read, binding);

    let approval = run.request_approval(&e.info.name, &Yes).unwrap();
    let write = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let apply_grant = write.grant_id();
    assert_ne!(apply_grant, read);
    assert_eq!(
        *e.f.registry
            .resolve(apply_grant, binding)
            .unwrap()
            .permission(),
        FsPermissionLevel::ReadWrite
    );
    run.apply(approval, write, &parent(&e.f)).unwrap();
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert!(e.f.registry.resolve(apply_grant, binding).is_err());
    assert_ne!(digest(&e.f.project), before);
    assert_read_revoked(&e, read, binding);

    let approval = run.request_restore(&e.info.name, &Yes).unwrap();
    let write = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let restore_grant = write.grant_id();
    assert!(restore_grant != read && restore_grant != apply_grant);
    run.restore(approval, write).unwrap();
    assert_eq!(run.apply_state(), ApplyState::Restored);
    assert!(e.f.registry.resolve(restore_grant, binding).is_err());
    assert_eq!(digest(&e.f.project), before);
    assert_read_revoked(&e, read, binding);
}

#[test]
fn p2e_h1_rg_04_every_earlier_end_revokes_the_read_grant() {
    // Cancelled before anything was opened, and after granting.
    let e = env();
    let (mut run, binding, read) = owned_run(&e);
    run.cancel().unwrap();
    assert_read_revoked(&e, read, binding);
    let (mut run, binding, read) = owned_run(&e);
    run.grant(&parent(&e.f)).unwrap();
    run.cancel().unwrap();
    assert_read_revoked(&e, read, binding);
    assert!(!run.project_handle_retained_for_test());

    // Revoked explicitly while staged.
    let (mut run, binding, read) = staged_owned(&e);
    run.revoke().unwrap();
    assert_eq!(run.state(), RunState::Revoked(RevocationReason::Explicit));
    assert_read_revoked(&e, read, binding);

    // Failed: the project folder was replaced before the snapshot.
    let e = env();
    let (mut run, binding, read) = owned_run(&e);
    run.grant(&parent(&e.f)).unwrap();
    let moved = e.f.project.with_file_name("moved");
    std::fs::rename(&e.f.project, &moved).unwrap();
    std::fs::create_dir(&e.f.project).unwrap();
    assert_eq!(run.snapshot(), Err(RunError::IdentityChanged));
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::IdentityChanged)
    );
    assert_read_revoked(&e, read, binding);

    // Failed: structural verification rejected the candidate.
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    run.edit(replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"))
        .unwrap();
    let staging = run.staging_path_for_test().unwrap();
    std::fs::write(staging.join("tests/check.rs"), "// weakened\n").unwrap();
    assert!(!run.verify_structural().unwrap().passed());
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::StructuralRejected)
    );
    assert_read_revoked(&e, read, binding);

    // Recovery required: a rejected edit could not be recorded.
    let e = env();
    let store = FailingStore::new(&e.f.ledger, FIRST_EDIT_EVENT);
    let (mut run, binding, read) = owned_run_with(&e, store);
    run.grant(&parent(&e.f)).unwrap();
    run.snapshot().unwrap();
    assert_eq!(
        run.edit(replace("tests/check.rs", "// weakened\n")),
        Err(RunError::RecoveryRequired(
            RecoveryReason::RejectionNotRecorded
        ))
    );
    assert_read_revoked(&e, read, binding);
}

#[test]
fn p2e_h1_rg_05_every_worker_ending_revokes_the_read_grant() {
    // The model proposed nothing: the worker cancels the run.
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    let model = Scripted::new(Vec::new());
    run.pin_model(model.pin().clone()).unwrap();
    let report = run_worker(&mut run, &model, "Change nothing.").unwrap();
    assert!(report.verification.is_none());
    assert_eq!(run.state(), RunState::Cancelled);
    assert_read_revoked(&e, read, binding);

    // The model failed: the worker ends the run.
    let (mut run, binding, read) = staged_owned(&e);
    let model = Scripted::unavailable();
    run.pin_model(model.pin().clone()).unwrap();
    assert!(run_worker(&mut run, &model, "Make answer() return 42.").is_err());
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::ModelUnavailable)
    );
    assert_read_revoked(&e, read, binding);

    // The model proposed a verified candidate: revoked before review.
    let (mut run, binding, read) = staged_owned(&e);
    let model = Scripted::new(vec![json!({"action": "edit", "edits": [
        {"path": "src/lib.rs", "op": "replace", "content": "pub fn answer() -> u32 { 42 }\n"}
    ]})]);
    run.pin_model(model.pin().clone()).unwrap();
    let report = run_worker(&mut run, &model, "Make answer() return 42.").unwrap();
    assert!(report.verification.unwrap().passed());
    assert_eq!(run.state(), RunState::StructurallyVerified);
    assert_read_revoked(&e, read, binding);
}

#[test]
fn p2e_h1_rg_06_discard_retries_and_leaves_no_read_grant_live() {
    // A verified run discarded without applying.
    let e = env();
    let (mut run, binding, read) = verified_owned(&e);
    assert_eq!(run.discard_staging(), Ok(CleanupStatus::Discarded));
    assert_eq!(run.discard_staging(), Ok(CleanupStatus::Discarded));
    assert_read_revoked(&e, read, binding);

    // A run whose end could not revoke it: discard retries.
    let (mut run, binding, read) = staged_owned(&e);
    {
        let _fails = RevokeFails::set();
        assert_eq!(
            run.cancel(),
            Err(RunError::RecoveryRequired(
                RecoveryReason::ProjectRevocationFailed
            ))
        );
        assert!(live(&e, read, binding), "reported, not hidden");
    }
    assert_eq!(run.discard_staging(), Ok(CleanupStatus::Discarded));
    assert_read_revoked(&e, read, binding);
    // The outcome is not rewritten as clean.
    assert_eq!(
        run.state(),
        RunState::RecoveryRequired(RecoveryReason::ProjectRevocationFailed)
    );
}

#[test]
fn p2e_h1_rg_07_a_failed_revocation_is_never_reported_as_clean() {
    // Verification passes, but the read grant cannot be closed: the run
    // requires recovery, has no verified candidate and cannot be approved.
    let e = env();
    let (mut run, binding, read) = staged_owned(&e);
    for edit in default_edits() {
        run.edit(edit).unwrap();
    }
    let _fails = RevokeFails::set();
    assert_eq!(
        run.verify_structural(),
        Err(RunError::RecoveryRequired(
            RecoveryReason::ProjectRevocationFailed
        ))
    );
    assert_eq!(
        run.state(),
        RunState::RecoveryRequired(RecoveryReason::ProjectRevocationFailed)
    );
    assert!(run.verification().is_none());
    assert!(matches!(
        run.request_approval(&e.info.name, &Yes),
        Err(ApplyError::InvalidState)
    ));
    assert!(live(&e, read, binding), "the state tells the truth");
    assert_eq!(
        final_state_event(&e, &run),
        (
            "run.recovery_required".to_string(),
            "ProjectRevocationFailed".to_string()
        )
    );

    // A worker that proposed nothing cannot finish cleanly either.
    let (mut run, _, _) = staged_owned(&e);
    let model = Scripted::new(Vec::new());
    run.pin_model(model.pin().clone()).unwrap();
    assert_eq!(
        run_worker(&mut run, &model, "Change nothing.").unwrap_err(),
        WorkerError::Run(RunError::RecoveryRequired(
            RecoveryReason::ProjectRevocationFailed
        ))
    );

    // An unrecorded end keeps the more severe authority failure.
    let store = FailingStore::new(&e.f.ledger, FIRST_TERMINAL_AFTER_STAGING);
    let (mut run, _, _) = owned_run_with(&e, store);
    run.grant(&parent(&e.f)).unwrap();
    run.snapshot().unwrap();
    assert_eq!(
        run.cancel(),
        Err(RunError::RecoveryRequired(
            RecoveryReason::ProjectRevocationFailed
        ))
    );
}

#[test]
fn p2e_h1_rg_08_expiry_and_drop_remain_backstops() {
    // The grant still carries its bounded expiry.
    let e = env();
    let issued = SystemTime::now();
    let (run, binding, read) = owned_run(&e);
    let expiry =
        e.f.registry
            .resolve(read, binding)
            .unwrap()
            .expires_at()
            .unwrap();
    assert!(expiry > issued && expiry <= SystemTime::now() + RUN_GRANT_LIFETIME);
    // A run dropped mid-way still closes its read grant.
    drop(run);
    assert_read_revoked(&e, read, binding);
    let (run, binding, read) = staged_owned(&e);
    drop(run);
    assert_read_revoked(&e, read, binding);

    // A run whose creation cannot be recorded does not keep one.
    let binding = self::binding();
    let grant = e.projects.grant_for_run(e.info.id, binding).unwrap();
    let read = grant.grant_id();
    let store = FailingStore::new(&e.f.ledger, 0);
    assert!(matches!(
        CodingRun::create_for_project(store, grant, default_scopes()),
        Err(RunError::Ledger(_))
    ));
    assert_read_revoked(&e, read, binding);

    // An expired read grant is denied: the run ends at its first use, and
    // still closes cleanly. The wait is bounded and deterministic: the run is
    // first used only once the expiry has passed.
    let binding = self::binding();
    let expiry = SystemTime::now() + Duration::from_millis(300);
    let short =
        e.f.registry
            .issue_trusted_root(
                &e.f.project,
                binding,
                WorkspaceAuthoritySource::UserSelected,
                FsPermissionLevel::ReadOnly,
                Some(expiry),
            )
            .unwrap();
    let identity = DirHandle::open_absolute(&e.f.project).unwrap().identity();
    let grant = ProjectGrant {
        project: e.info.id,
        grant: short,
        binding,
        identity,
        authority: Arc::clone(&e.f.registry),
    };
    let mut run = CodingRun::create_for_project(
        Arc::clone(&e.f.ledger) as Arc<dyn LedgerStore>,
        grant,
        default_scopes(),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while SystemTime::now() <= expiry {
        assert!(std::time::Instant::now() < deadline, "expiry not reached");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        run.grant(&parent(&e.f)),
        Err(RunError::Revoked(RevocationReason::GrantExpired))
    );
    assert_eq!(
        run.state(),
        RunState::Revoked(RevocationReason::GrantExpired)
    );
    assert!(run.staging_path_for_test().is_none(), "nothing staged");
    assert_read_revoked(&e, short, binding);
}
