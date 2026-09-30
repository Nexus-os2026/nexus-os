//! Phase One P1-07 tests: owner-triggered restore of one run's apply.
//! `p1_r_nc_*` are negative controls.

use super::p1_apply::{approve_and_apply, default_edits, env, verified, Env};
use super::*;
use std::cell::RefCell;

struct Confirm {
    yes: bool,
    shown: RefCell<Vec<ConfirmationRequest>>,
}

impl OwnerConfirmer for Confirm {
    fn confirm(&self, request: &ConfirmationRequest) -> bool {
        self.shown.borrow_mut().push(request.clone());
        self.yes
    }
}

fn yes() -> Confirm {
    Confirm {
        yes: true,
        shown: RefCell::new(Vec::new()),
    }
}

/// A project with one run applied; returns the pre-apply digest.
fn applied(e: &Env, edits: Vec<CandidateEdit>) -> (CodingRun, WorkspaceBinding) {
    let (mut run, binding) = verified(e, edits);
    approve_and_apply(e, &mut run, binding).unwrap();
    (run, binding)
}

fn restore(
    e: &Env,
    run: &mut CodingRun,
    binding: WorkspaceBinding,
) -> Result<ApplyReport, ApplyError> {
    let approval = run.request_restore(&e.info.name, &yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    run.restore(approval, grant)
}

// ── Positive behaviour ──────────────────────────────────────────────────────

#[test]
fn p1_r_01_restore_returns_the_project_to_its_pre_apply_state() {
    let e = env();
    let before = digest(&e.f.project);
    let (mut run, binding) = applied(&e, default_edits());
    assert_ne!(digest(&e.f.project), before);
    let store = e.f.staging_parent.join(format!("{}.apply", run.id()));
    assert!(store.is_dir());

    let confirm = yes();
    let approval = run.request_restore(&e.info.name, &confirm).unwrap();
    assert_eq!(approval.kind(), ConfirmationKind::Restore);
    let shown = confirm.shown.borrow()[0].clone();
    assert_eq!(shown.kind, ConfirmationKind::Restore);
    assert_eq!(shown.files(), 2);
    assert!(shown.message().contains("undo"));
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let grant_id = grant.grant_id();
    let report = run.restore(approval, grant).unwrap();
    assert_eq!(report.files.len(), 2);
    assert_eq!(
        digest(&e.f.project),
        before,
        "created files and directories removed, pre-images back"
    );
    assert_eq!(run.apply_state(), ApplyState::Restored);
    assert!(
        !store.exists(),
        "the pre-image store is discarded after a restore"
    );
    assert!(e.f.registry.resolve(grant_id, binding).is_err());
    let kinds = kinds(&e.f, &run);
    for kind in ["restore.approved", "restore.prepared", "restore.completed"] {
        assert!(kinds.contains(&kind.to_string()), "{kind}");
    }
}

// ── Negative controls ───────────────────────────────────────────────────────

#[test]
fn p1_r_nc_01_an_owner_edit_after_apply_blocks_the_whole_restore() {
    let e = env();
    let (mut run, binding) = applied(&e, default_edits());
    std::fs::write(
        e.f.project.join("src/lib.rs"),
        "// owner edit after apply\n",
    )
    .unwrap();
    let after_edit = digest(&e.f.project);
    assert_eq!(
        restore(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Stale("src/lib.rs".to_string()))
    );
    assert_eq!(digest(&e.f.project), after_edit, "nothing was touched");
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert!(kinds(&e.f, &run).contains(&"restore.rejected".to_string()));

    // Once the file again holds exactly what Nexus wrote, restore proceeds.
    std::fs::write(
        e.f.project.join("src/lib.rs"),
        "pub fn answer() -> u32 { 42 }\n",
    )
    .unwrap();
    restore(&e, &mut run, binding).unwrap();
    assert_eq!(
        std::fs::read_to_string(e.f.project.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> u32 { 41 }\n"
    );
}

#[test]
fn p1_r_nc_02_a_created_file_must_still_be_the_file_nexus_created() {
    let e = env();
    let (mut run, binding) = applied(&e, default_edits());
    // Same content, but a different file (an editor's save-by-rename).
    let deep = e.f.project.join("src/new/deep.rs");
    let tmp = deep.with_file_name("deep.rs.save");
    std::fs::write(&tmp, "pub fn deep() {}\n").unwrap();
    std::fs::rename(&tmp, &deep).unwrap();
    let state = digest(&e.f.project);
    assert_eq!(
        restore(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Stale("src/new/deep.rs".to_string()))
    );
    assert_eq!(digest(&e.f.project), state);
}

#[test]
fn p1_r_nc_03_restore_authority_belongs_to_one_run() {
    let e = env();
    let (mut a, binding_a) = applied(&e, vec![replace("src/lib.rs", "// A\n")]);
    let (mut b, binding_b) = applied(&e, vec![replace("src/util.rs", "// B\n")]);
    let state = digest(&e.f.project);
    let approval_a = a.request_restore(&e.info.name, &yes()).unwrap();
    let _approval_b = b.request_restore(&e.info.name, &yes()).unwrap();
    let grant_b = e.projects.grant_for_apply(e.info.id, binding_b).unwrap();
    assert_eq!(
        b.restore(approval_a, grant_b).unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    let approval_a = a.request_restore(&e.info.name, &yes()).unwrap();
    let grant_for_b = e.projects.grant_for_apply(e.info.id, binding_b).unwrap();
    assert_eq!(
        a.restore(approval_a, grant_for_b).unwrap_err(),
        ApplyError::Refused(Refusal::WrongProject)
    );
    assert_eq!(digest(&e.f.project), state);
    // Each run can still restore its own apply.
    restore(&e, &mut a, binding_a).unwrap();
    restore(&e, &mut b, binding_b).unwrap();
}

#[test]
fn p1_r_nc_04_approvals_are_not_interchangeable_and_restore_needs_an_apply() {
    let e = env();
    // No apply: nothing to restore.
    let (mut fresh, _) = verified(&e, default_edits());
    assert_eq!(
        fresh.request_restore(&e.info.name, &yes()).unwrap_err(),
        ApplyError::InvalidState
    );
    // An apply approval is not a restore approval.
    let apply_approval = fresh.request_approval(&e.info.name, &yes()).unwrap();
    let (mut run, binding) = applied(&e, vec![replace("src/util.rs", "// u\n")]);
    let _pending = run.request_restore(&e.info.name, &yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    assert_eq!(
        run.restore(apply_approval, grant).unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    // A restore approval is not an apply approval.
    let restore_approval = run.request_restore(&e.info.name, &yes()).unwrap();
    let (mut other, other_binding) = verified(&e, vec![replace("src/lib.rs", "// o\n")]);
    let _ = other.request_approval(&e.info.name, &yes()).unwrap();
    let grant = e
        .projects
        .grant_for_apply(e.info.id, other_binding)
        .unwrap();
    assert_eq!(
        other
            .apply(restore_approval, grant, &parent(&e.f))
            .unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    // A second apply approval of the same binding, obtained before the
    // apply, cannot serve as the restore approval.
    let (mut twice, twice_binding) = verified(&e, vec![replace("src/lib.rs", "// t\n")]);
    let first = twice.request_approval(&e.info.name, &yes()).unwrap();
    let second = twice.request_approval(&e.info.name, &yes()).unwrap();
    assert_eq!(first.binding(), second.binding());
    let grant = e
        .projects
        .grant_for_apply(e.info.id, twice_binding)
        .unwrap();
    twice.apply(first, grant, &parent(&e.f)).unwrap();
    let _pending = twice.request_restore(&e.info.name, &yes()).unwrap();
    let applied_state = digest(&e.f.project);
    let grant = e
        .projects
        .grant_for_apply(e.info.id, twice_binding)
        .unwrap();
    assert_eq!(
        twice.restore(second, grant).unwrap_err(),
        ApplyError::Refused(Refusal::ApprovalMismatch)
    );
    assert_eq!(digest(&e.f.project), applied_state);
    assert_eq!(twice.apply_state(), ApplyState::Applied);
    restore(&e, &mut twice, twice_binding).unwrap();

    // After a restore, there is nothing left to restore.
    restore(&e, &mut run, binding).unwrap();
    assert_eq!(
        run.request_restore(&e.info.name, &yes()).unwrap_err(),
        ApplyError::InvalidState
    );
}

#[test]
fn p1_r_nc_05_a_declined_restore_changes_nothing() {
    let e = env();
    let (mut run, _) = applied(&e, default_edits());
    let state = digest(&e.f.project);
    let no = Confirm {
        yes: false,
        shown: RefCell::new(Vec::new()),
    };
    assert_eq!(
        run.request_restore(&e.info.name, &no).unwrap_err(),
        ApplyError::Refused(Refusal::Declined)
    );
    assert_eq!(digest(&e.f.project), state);
    assert_eq!(run.apply_state(), ApplyState::Applied);
    assert!(kinds(&e.f, &run).contains(&"restore.declined".to_string()));
}

#[test]
fn p1_r_nc_06_a_tampered_preimage_blocks_the_restore() {
    let e = env();
    let (mut run, binding) = applied(&e, default_edits());
    let store = e.f.staging_parent.join(format!("{}.apply", run.id()));
    std::fs::write(store.join("p00000"), "// swapped pre-image\n").unwrap();
    let state = digest(&e.f.project);
    assert_eq!(
        restore(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::PreimageUnavailable)
    );
    assert_eq!(digest(&e.f.project), state);
}

#[test]
fn p1_r_nc_07_a_replaced_root_or_moved_directory_blocks_the_restore() {
    let e = env();
    let (mut run, binding) = applied(&e, default_edits());
    let approval = run.request_restore(&e.info.name, &yes()).unwrap();
    let grant = e.projects.grant_for_apply(e.info.id, binding).unwrap();
    let moved = e.f.project.with_file_name("moved");
    std::fs::rename(&e.f.project, &moved).unwrap();
    std::fs::create_dir(&e.f.project).unwrap();
    let state = digest(&moved);
    assert_eq!(
        run.restore(approval, grant).unwrap_err(),
        ApplyError::Refused(Refusal::IdentityChanged)
    );
    assert_eq!(digest(&moved), state);
    std::fs::remove_dir(&e.f.project).unwrap();
    std::fs::rename(&moved, &e.f.project).unwrap();

    // A subdirectory moved aside and replaced by a look-alike: the path no
    // longer reaches the directory Nexus wrote into.
    let src = e.f.project.join("src");
    std::fs::rename(&src, e.f.project.join("src.moved")).unwrap();
    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("lib.rs"), "pub fn answer() -> u32 { 42 }\n").unwrap();
    let state = digest(&e.f.project);
    assert!(matches!(
        restore(&e, &mut run, binding).unwrap_err(),
        ApplyError::Refused(Refusal::Stale(_))
    ));
    assert_eq!(digest(&e.f.project), state);
}

#[test]
fn p1_r_nc_08_a_rolled_back_apply_has_nothing_to_restore() {
    let e = env();
    let (mut run, binding) = verified(&e, default_edits());
    let result = {
        use crate::coding_run::apply::{HookPoint, APPLY_HOOK};
        APPLY_HOOK.with(|h| {
            *h.borrow_mut() = Some(Box::new(|point, path| {
                point == HookPoint::BeforeWrite && path == "src/new/deep.rs"
            }))
        });
        let result = approve_and_apply(&e, &mut run, binding);
        APPLY_HOOK.with(|h| *h.borrow_mut() = None);
        result
    };
    assert!(matches!(result, Err(ApplyError::RolledBack { .. })));
    assert_eq!(
        run.request_restore(&e.info.name, &yes()).unwrap_err(),
        ApplyError::InvalidState
    );
}

#[test]
fn p1_r_nc_09_restore_uses_no_time_machine_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in [
        "coding_run.rs",
        "coding_run/apply.rs",
        "coding_run/fsops.rs",
    ] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        for needle in [
            "time_machine",
            "TimeMachine",
            "what_if",
            "crate::checkpoint",
        ] {
            assert!(!text.contains(needle), "{file} references {needle}");
        }
    }
}
