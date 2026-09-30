//! Phase One P1-06 tests: native owner approval and stale/conflict-safe
//! apply. The confirmer is a stand-in for the desktop's native dialog.
//! `p1_a_nc_*` are negative controls.

use super::*;
use crate::coding_run::apply::{HookPoint, APPLY_HOOK};
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;

struct Picker(PathBuf);

impl FolderPicker for Picker {
    fn pick_folder(&self) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

/// A native confirmation stand-in: answers `yes`, remembers what it showed.
struct Confirm {
    yes: bool,
    shown: RefCell<Vec<ConfirmationRequest>>,
}

impl Confirm {
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

impl OwnerConfirmer for Confirm {
    fn confirm(&self, request: &ConfirmationRequest) -> bool {
        self.shown.borrow_mut().push(request.clone());
        self.yes
    }
}

pub(super) struct Env {
    pub(super) f: Fixture,
    pub(super) projects: ProjectRegistry,
    pub(super) info: ProjectInfo,
}

pub(super) fn env() -> Env {
    let f = fixture();
    let state = f.staging_parent.parent().unwrap().join("nexus-state");
    std::fs::create_dir(&state).unwrap();
    let projects = ProjectRegistry::new(Arc::clone(&f.registry), &state);
    let info = projects.select(&Picker(f.project.clone())).unwrap();
    Env { f, projects, info }
}

pub(super) fn verified_with(
    e: &Env,
    ledger: Arc<dyn LedgerStore>,
    edits: Vec<CandidateEdit>,
) -> (CodingRun, WorkspaceBinding) {
    let binding = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    };
    let grant = e.projects.grant_for_run(e.info.id, binding).unwrap();
    let mut run = CodingRun::create_for_project(ledger, grant, default_scopes()).unwrap();
    run.grant(&parent(&e.f)).unwrap();
    run.snapshot().unwrap();
    for edit in edits {
        run.edit(edit).unwrap();
    }
    assert!(run.verify_structural().unwrap().passed());
    (run, binding)
}

pub(super) fn verified(e: &Env, edits: Vec<CandidateEdit>) -> (CodingRun, WorkspaceBinding) {
    verified_with(e, Arc::clone(&e.f.ledger) as Arc<dyn LedgerStore>, edits)
}

pub(super) fn default_edits() -> Vec<CandidateEdit> {
    vec![
        replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"),
        create("src/new/deep.rs", "pub fn deep() {}\n"),
    ]
}

pub(super) fn approve_and_apply(
    e: &Env,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    run.apply(approval, grant, &parent(&e.f))
}

fn read(e: &Env, path: &str) -> String {
    std::fs::read_to_string(e.f.project.join(path)).unwrap()
}

fn with_hook<T>(
    hook: impl FnMut(HookPoint, &str) -> bool + 'static,
    body: impl FnOnce() -> T,
) -> T {
    APPLY_HOOK.with(|h| *h.borrow_mut() = Some(Box::new(hook)));
    let result = body();
    APPLY_HOOK.with(|h| *h.borrow_mut() = None);
    result
}

fn preimage_store(e: &Env, run: &CodingRun) -> PathBuf {
    e.f.staging_parent.join(format!("{}.apply", run.id()))
}

// ── Positive behaviour ──────────────────────────────────────────────────────

#[test]
fn p1_a_01_native_approval_then_apply_writes_exactly_the_candidate() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let staging_grant = run.staging_grant_for_test().unwrap();
    let confirm = Confirm::yes();
    let approval = run.request_approval(&e.info.name, &confirm).unwrap();
    let shown = confirm.shown.borrow()[0].clone();
    assert_eq!(shown.kind, ConfirmationKind::Apply);
    assert_eq!(shown.project_name, "project");
    assert_eq!(shown.run_id, run.id());
    assert_eq!(
        (shown.files(), shown.creates, shown.replaces, shown.deletes),
        (2, 1, 1, 0)
    );
    let verification = run.verification().unwrap().clone();
    assert!(verification
        .candidate_manifest_hash
        .to_hex()
        .starts_with(&shown.candidate_short));
    assert!(verification
        .base_manifest_hash
        .to_hex()
        .starts_with(&shown.base_short));
    let message = shown.message();
    assert!(message.contains("project") && message.contains(&run.id().to_string()));
    assert!(message.contains("replace src/lib.rs") && message.contains("create src/new/deep.rs"));
    assert_eq!(approval.binding().run_id, run.id());

    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let apply_grant = grant.grant_id();
    let report = run.apply(approval, grant, &parent(&e.f)).unwrap();
    assert_eq!(
        report.files,
        vec![
            (rel("src/lib.rs"), ChangeKind::Replace),
            (rel("src/new/deep.rs"), ChangeKind::Create)
        ]
    );
    assert_eq!(read(&e, "src/lib.rs"), "pub fn answer() -> u32 { 42 }\n");
    assert_eq!(read(&e, "src/new/deep.rs"), "pub fn deep() {}\n");
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert_eq!(run.applied_files(), report.files);
    // No temporary names remain in the project.
    assert!(digest(&e.f.project)
        .keys()
        .all(|k| !k.contains(".nexus-coding-run-tmp-")));
    // The pre-image of the replaced file is in private run storage.
    let store = preimage_store(&e, &run);
    assert_eq!(
        std::fs::read_to_string(store.join("p00000")).unwrap(),
        "pub fn answer() -> u32 { 41 }\n"
    );
    // The write grant was revoked after the apply; staging authority was
    // already closed and was never used for the project.
    assert!(e.f.registry.resolve(apply_grant, binding).is_err());
    assert_eq!(
        e.f.registry.resolve(staging_grant, binding).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant
    );
    let kinds = kinds(&e.f, &run);
    for kind in ["approval.granted", "apply.prepared", "apply.completed"] {
        assert!(kinds.contains(&kind.to_string()), "{kind}");
    }
    assert_eq!(kinds.iter().filter(|k| *k == "apply.op").count(), 2);
    // A second apply is impossible.
    assert_eq!(
        run.request_approval(&e.info.name, &Confirm::yes())
            .unwrap_err(),
        ApplyError::InvalidState
    );
}

#[test]
fn p1_a_02_replacement_keeps_the_file_mode_and_creations_are_plain_files() {
    let e = env();
    let util = e.f.project.join("src/util.rs");
    std::fs::set_permissions(&util, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (mut run, binding) = verified(
        &e,
        vec![
            replace("src/util.rs", "pub fn helper() { }\n"),
            create("src/made.rs", "pub fn made() {}\n"),
        ],
    );
    approve_and_apply(&e, &mut run, binding).unwrap();
    let mode = |p: &str| {
        std::fs::metadata(e.f.project.join(p))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("src/util.rs"), 0o755);
    assert_eq!(mode("src/made.rs") & 0o600, 0o600);
    assert_eq!(read(&e, "src/util.rs"), "pub fn helper() { }\n");
}

// ── Negative controls ───────────────────────────────────────────────────────

#[test]
fn p1_a_nc_01_a_declined_confirmation_grants_nothing() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, _) = verified(&e, default_edits());
    assert_eq!(
        run.request_approval(&e.info.name, &Confirm::no())
            .unwrap_err(),
        ApplyError::Refused(Refusal::Declined)
    );
    assert!(kinds(&e.f, &run).contains(&"approval.declined".to_string()));
    assert_eq!(run.apply_state(), ApplyState::NotApplied);
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p1_a_nc_02_approval_for_one_run_or_candidate_cannot_apply_another() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut a, binding_a) = verified(&e, vec![replace("src/lib.rs", "// A\n")]);
    let (mut b, binding_b) = verified(&e, vec![replace("src/lib.rs", "// B\n")]);
    let approval_a = a.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let _approval_b = b.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    assert_ne!(
        a.verification().unwrap().candidate_manifest_hash,
        b.verification().unwrap().candidate_manifest_hash
    );
    let grant_b = e.projects.grant_for_apply(e.info.id, binding_b).unwrap();
    assert_eq!(
        b.apply(approval_a, grant_b, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    assert_eq!(digest(&e.f.project), before);
    // The attempt consumed B's pending approval: B needs a fresh one.
    let fresh_a = a.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    // A's approval with a write grant issued for run B is refused too.
    let grant_for_b = e.projects.grant_for_apply(e.info.id, binding_b).unwrap();
    assert_eq!(
        a.apply(fresh_a, grant_for_b, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::WrongProject)
    );
    assert_eq!(digest(&e.f.project), before);
    assert!(kinds(&e.f, &b).contains(&"apply.rejected".to_string()));
    let _ = binding_a;
}

#[test]
fn p1_a_nc_03_an_owner_edit_after_approval_causes_zero_writes() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    std::fs::write(e.f.project.join("src/lib.rs"), "// the owner's edit\n").unwrap();
    let before = digest(&e.f.project);
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::Stale("src/lib.rs".to_string()))
    );
    assert_eq!(digest(&e.f.project), before, "zero writes");
    assert!(!preimage_store(&e, &run).exists());
    assert_eq!(run.apply_state(), ApplyState::NotApplied);

    // The approval was consumed; after the owner reverts, a fresh approval
    // applies cleanly.
    std::fs::write(
        e.f.project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 41 }\n",
    )
    .unwrap();
    approve_and_apply(&e, &mut run, binding).unwrap();
    assert_eq!(read(&e, "src/lib.rs"), "pub fn answer() -> u32 { 42 }\n");
}

#[test]
fn p1_a_nc_04_a_creation_that_now_exists_causes_zero_writes() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    write(&e.f.project.join("src/new/deep.rs"), "// owner made this\n");
    let before = digest(&e.f.project);
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Stale("src/new/deep.rs".to_string()))
    );
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p1_a_nc_05_a_replaced_root_causes_zero_writes() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let moved = e.f.project.with_file_name("moved");
    std::fs::rename(&e.f.project, &moved).unwrap();
    std::fs::create_dir(&e.f.project).unwrap();
    let (old, new) = (digest(&moved), digest(&e.f.project));
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::IdentityChanged)
    );
    assert_eq!((digest(&moved), digest(&e.f.project)), (old, new));
    // And the registry refuses to issue a new write grant for it.
    assert_eq!(
        e.projects.grant_for_apply(e.info.id, binding).unwrap_err(),
        ProjectError::IdentityChanged
    );
}

#[test]
fn p1_a_nc_06_symlinked_targets_and_parents_cause_zero_writes() {
    let e = env();
    let outside = e.f.project.parent().unwrap().join("outside.rs");
    std::fs::write(&outside, "// outside\n").unwrap();

    let (mut run, binding) = verified(&e, default_edits());
    std::fs::remove_file(e.f.project.join("src/lib.rs")).unwrap();
    std::os::unix::fs::symlink(&outside, e.f.project.join("src/lib.rs")).unwrap();
    let before = digest(&e.f.project);
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Redirect("src/lib.rs".to_string()))
    );
    assert_eq!(digest(&e.f.project), before);
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "// outside\n");

    let e = env();
    let elsewhere = e.f.project.parent().unwrap().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let (mut run, binding) = verified(&e, vec![create("src/new/deep.rs", "x\n")]);
    std::os::unix::fs::symlink(&elsewhere, e.f.project.join("src/new")).unwrap();
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Redirect("src/new/deep.rs".to_string()))
    );
    assert!(std::fs::read_dir(&elsewhere).unwrap().next().is_none());
}

#[test]
fn p1_a_nc_07_a_mid_apply_failure_rolls_back_every_applied_file() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding) = verified(&e, default_edits());
    let result = with_hook(
        |point, path| point == HookPoint::BeforeWrite && path == "src/new/deep.rs",
        || approve_and_apply(&e, &mut run, binding),
    );
    assert_eq!(
        result.unwrap_err(),
        ApplyError::RolledBack {
            failed: "src/new/deep.rs".to_string()
        }
    );
    assert_eq!(
        digest(&e.f.project),
        before,
        "the replaced file was restored"
    );
    assert_eq!(run.apply_state(), ApplyState::RolledBack);
    assert!(kinds(&e.f, &run).contains(&"apply.rolled_back".to_string()));
    assert!(!kinds(&e.f, &run).contains(&"apply.completed".to_string()));
    assert!(!preimage_store(&e, &run).exists());

    // A failure after directories were created removes them too.
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding) = verified(
        &e,
        vec![
            create("src/new/deep.rs", "x\n"),
            replace("src/util.rs", "y\n"),
        ],
    );
    let result = with_hook(
        |point, path| point == HookPoint::BeforeWrite && path == "src/util.rs",
        || approve_and_apply(&e, &mut run, binding),
    );
    assert!(matches!(result, Err(ApplyError::RolledBack { .. })));
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p1_a_nc_08_a_concurrent_edit_during_the_exchange_is_never_overwritten() {
    let e = env();
    let lib = e.f.project.join("src/lib.rs");
    let (mut run, binding) = verified(&e, default_edits());
    let target = lib.clone();
    let result = with_hook(
        move |point, path| {
            if point == HookPoint::BeforeExchange && path == "src/lib.rs" {
                // The owner saves a new version (a new file) at this moment.
                let tmp = target.with_file_name("lib.rs.owner");
                std::fs::write(&tmp, "// owner saved\n").unwrap();
                std::fs::rename(&tmp, &target).unwrap();
            }
            false
        },
        || approve_and_apply(&e, &mut run, binding),
    );
    assert!(matches!(result, Err(ApplyError::RolledBack { .. })));
    assert_eq!(std::fs::read_to_string(&lib).unwrap(), "// owner saved\n");
    assert!(!e.f.project.join("src/new").exists());
}

#[test]
fn p1_a_nc_09_an_undoable_failure_is_reported_as_recovery_required() {
    let e = env();
    let (mut run, binding) = verified(
        &e,
        vec![
            replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"),
            replace("src/util.rs", "y\n"),
        ],
    );
    let lib = e.f.project.join("src/lib.rs");
    let result = with_hook(
        move |point, path| {
            if point == HookPoint::BeforeWrite && path == "src/util.rs" {
                // The owner edits the already-applied file, then the next
                // write fails: rollback must not overwrite the owner.
                std::fs::write(&lib, "// owner edit after apply\n").unwrap();
                return true;
            }
            false
        },
        || approve_and_apply(&e, &mut run, binding),
    );
    assert_eq!(
        result.unwrap_err(),
        ApplyError::RecoveryRequired {
            failed: "src/util.rs".to_string(),
            unrestored: vec!["src/lib.rs".to_string()]
        }
    );
    assert_eq!(run.apply_state(), ApplyState::RecoveryRequired);
    assert_eq!(read(&e, "src/lib.rs"), "// owner edit after apply\n");
    assert!(
        preimage_store(&e, &run).join("p00000").is_file(),
        "pre-images kept"
    );
    assert!(kinds(&e.f, &run).contains(&"apply.recovery_required".to_string()));
}

#[test]
fn p1_a_nc_10_read_only_revoked_or_foreign_grants_cannot_apply() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding) = verified(&e, default_edits());
    // A run (read-only) grant is not write authority.
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let read_only = e.projects.grant_for_run(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, read_only, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::AuthorityDenied)
    );
    // A revoked write grant.
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    let revoked = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    e.f.registry.revoke(revoked.grant_id(), binding).unwrap();
    assert_eq!(
        run.apply(approval, revoked, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::AuthorityDenied)
    );
    // A write grant from another registry (another authority).
    let other_authority = Arc::new(WorkspaceAuthorityRegistry::new());
    let state = e.f.staging_parent.parent().unwrap().join("nexus-state");
    let other = ProjectRegistry::new(other_authority, &state);
    let other_info = other.select(&Picker(e.f.project.clone())).unwrap();
    let foreign = other.grant_for_apply(other_info.id, binding).unwrap();
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    assert_eq!(
        run.apply(approval, foreign, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::WrongProject)
    );
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p1_a_nc_11_a_changed_or_invalid_candidate_cannot_be_approved_or_applied() {
    let e = env();
    let before = digest(&e.f.project);
    // Changed staging after approval: refused, verification withdrawn.
    let (mut run, binding) = verified(&e, default_edits());
    let approval = run.request_approval(&e.info.name, &Confirm::yes()).unwrap();
    std::fs::write(
        run.staging_path_for_test().unwrap().join("src/lib.rs"),
        "pub fn evil() {}\n",
    )
    .unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.apply(approval, grant, &parent(&e.f)).unwrap_err(),
        ApplyError::Refused(Refusal::CandidateChanged)
    );
    assert!(run.verification().is_none());
    assert!(run.request_approval(&e.info.name, &Confirm::yes()).is_err());

    // A structurally rejected candidate never reaches approval.
    let grant = e.projects.grant_for_run(e.info.id, binding).unwrap();
    let mut rejected = CodingRun::create_for_project(
        Arc::clone(&e.f.ledger) as Arc<dyn LedgerStore>,
        grant,
        default_scopes(),
    )
    .unwrap();
    rejected.grant(&parent(&e.f)).unwrap();
    rejected.snapshot().unwrap();
    rejected.edit(replace("src/lib.rs", "x\n")).unwrap();
    std::fs::write(
        rejected
            .staging_path_for_test()
            .unwrap()
            .join("tests/check.rs"),
        "// off\n",
    )
    .unwrap();
    assert!(!rejected.verify_structural().unwrap().passed());
    assert_eq!(
        rejected
            .request_approval(&e.info.name, &Confirm::yes())
            .unwrap_err(),
        ApplyError::InvalidState
    );
    assert_eq!(digest(&e.f.project), before);
}

#[test]
fn p1_a_nc_12_an_unrecordable_plan_causes_zero_writes() {
    let e = env();
    let before = digest(&e.f.project);
    // 0 created, 1–2 allocation, 3 granted, 4–5 snapshot, 6–9 two edits,
    // 10 verify, 11 review, 12 approval, 13 apply.prepared.
    let store = FailingStore::new(&e.f.ledger, 13);
    let (mut run, binding) = verified_with(&e, store, default_edits());
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Unrecorded)
    );
    assert_eq!(digest(&e.f.project), before);
    assert!(!preimage_store(&e, &run).exists());
    assert_eq!(run.apply_state(), ApplyState::NotApplied);
}

#[test]
fn p1_a_nc_13_an_unrecordable_completion_is_undone() {
    let e = env();
    let before = digest(&e.f.project);
    // ... 13 apply.prepared, 14–15 apply.op, 16 apply.completed.
    let store = FailingStore::new(&e.f.ledger, 16);
    let (mut run, binding) = verified_with(&e, store, default_edits());
    assert_eq!(
        approve_and_apply(&e, &mut run, binding).unwrap_err(),
        ApplyError::RolledBack {
            failed: "apply.completed".to_string()
        }
    );
    assert_eq!(digest(&e.f.project), before);
    assert_eq!(run.apply_state(), ApplyState::RolledBack);
}

#[test]
fn p1_a_nc_14_confirmation_and_review_text_cannot_disguise_a_change() {
    let e = env();
    // A model-chosen name with printf directives and a right-to-left
    // override that would display "src/evil\u{202e}sr.txt" as "...txt.rs".
    let hostile = "src/100%s%n\u{202e}sr.txt";
    let (mut run, _) = verified(
        &e,
        vec![create(hostile, "line one\n\u{202e}hidden\u{200b}\n")],
    );
    let confirm = Confirm::yes();
    run.request_approval(&e.info.name, &confirm).unwrap();
    let shown = confirm.shown.borrow()[0].clone();
    let message = shown.message();
    assert!(message.contains("⟨U+202E⟩"), "{message}");
    assert!(!message.contains('\u{202e}'));
    // The kernel keeps `%` literal; the native adapter escapes it for GTK.
    assert!(message.contains("100%s%n"));

    let review = run.review().unwrap();
    let TextDiff::Unified { text, .. } = &review.changes[0].diff else {
        panic!("diff expected");
    };
    let safe = display_safe(text, true);
    assert!(safe.contains("⟨U+202E⟩hidden⟨U+200B⟩\n"));
    assert!(!safe.contains('\u{202e}') && !safe.contains('\u{200b}'));
    assert_eq!(display_safe("a\nb\tc\u{0}", true), "a\nb\tc⟨U+0000⟩");
    assert_eq!(display_safe("a\nb", false), "a⟨U+000A⟩b");
}
