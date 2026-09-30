//! Phase Two P2E tests: the verified candidate is materialized into a
//! verification input through the retained staging handle (the revoked
//! staging grant is never reopened) and proven against the verified
//! candidate manifest; a changed input invalidates the execution, and no
//! failure here costs the owner the reviewable candidate. `p2e_input_*` are
//! the milestone's controls.

use super::p1_apply::{approve_and_apply, default_edits, env, verified, Env};
use super::*;
use std::os::fd::OwnedFd;
use std::os::unix::fs::symlink;

/// A fresh, empty input directory and a descriptor of it.
fn input_dir(e: &Env) -> (PathBuf, OwnedFd) {
    let path =
        e.f.staging_parent
            .parent()
            .unwrap()
            .join(format!("verification-input-{}", Uuid::new_v4()));
    std::fs::create_dir(&path).unwrap();
    let fd = reopen(&path);
    (path, fd)
}

fn reopen(path: &Path) -> OwnedFd {
    OwnedFd::from(std::fs::File::open(path).unwrap())
}

fn materialized(e: &Env, run: &mut CodingRun) -> PathBuf {
    let (path, fd) = input_dir(e);
    run.materialize_verification_input(fd).unwrap();
    path
}

#[test]
fn p2e_input_materializes_exactly_the_verified_candidate() {
    let e = env();
    let project_before = digest(&e.f.project);
    let (mut run, binding) = verified(&e, default_edits());
    let staging_grant = run.staging_grant_for_test().unwrap();
    let (path, fd) = input_dir(&e);
    let hash = run.materialize_verification_input(fd).unwrap();
    assert_eq!(hash, run.verification().unwrap().candidate_manifest_hash);
    // Exactly the candidate's files and bytes: those of staging.
    let staging = run.staging_path_for_test().unwrap();
    assert_eq!(digest(&path), digest(&staging));
    assert_eq!(
        std::fs::read_to_string(path.join("src/lib.rs")).unwrap(),
        "pub fn answer() -> u32 { 42 }\n"
    );
    assert!(path.join("src/new/deep.rs").is_file());
    run.check_verification_input(reopen(&path)).unwrap();
    // The run is unchanged, its staging grant stays revoked and the owner's
    // project is untouched.
    assert_eq!(run.state(), RunState::StructurallyVerified);
    assert!(run.verification().is_some());
    assert_eq!(
        e.f.registry.resolve(staging_grant, binding).unwrap_err(),
        crate::workspace_authority::WorkspaceAuthorityError::RevokedGrant
    );
    assert_eq!(digest(&e.f.project), project_before);
    run.review().unwrap();
}

#[test]
fn p2e_input_rescan_detects_every_change() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    type Mutation = fn(&Path);
    let mutations: [(&str, Mutation); 8] = [
        ("changed bytes", |p| {
            std::fs::write(p.join("src/lib.rs"), "pub fn answer() -> u32 { 0 }\n").unwrap()
        }),
        ("extra file", |p| {
            std::fs::write(p.join("src/extra.rs"), "").unwrap()
        }),
        ("removed file", |p| {
            std::fs::remove_file(p.join("src/util.rs")).unwrap()
        }),
        ("renamed file", |p| {
            std::fs::rename(p.join("src/util.rs"), p.join("src/moved.rs")).unwrap()
        }),
        ("symlink", |p| {
            symlink("/etc/hostname", p.join("src/link.rs")).unwrap()
        }),
        ("hard link", |p| {
            std::fs::hard_link(p.join("src/util.rs"), p.join("docs/util.rs")).unwrap()
        }),
        ("special file", |p| {
            nix::unistd::mkfifo(&p.join("src/fifo"), nix::sys::stat::Mode::S_IRWXU).unwrap()
        }),
        ("git metadata", |p| {
            std::fs::create_dir(p.join(".git")).unwrap();
            std::fs::write(p.join(".git/config"), "").unwrap();
        }),
    ];
    for (what, mutate) in mutations {
        let path = materialized(&e, &mut run);
        run.check_verification_input(reopen(&path)).unwrap();
        mutate(&path);
        assert_eq!(
            run.check_verification_input(reopen(&path)),
            Err(RunError::CandidateChanged),
            "{what}"
        );
    }
    assert_eq!(run.state(), RunState::StructurallyVerified);
}

#[test]
fn p2e_input_must_be_an_empty_directory_and_failures_leave_the_run() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    let (path, _) = input_dir(&e);
    std::fs::write(path.join("planted.rs"), "").unwrap();
    assert_eq!(
        run.materialize_verification_input(reopen(&path)),
        Err(RunError::VerificationInput)
    );
    // Refused before anything of the candidate was written.
    let names: Vec<_> = std::fs::read_dir(&path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["planted.rs"]);
    let file = path.join("planted.rs");
    assert_eq!(
        run.materialize_verification_input(reopen(&file)),
        Err(RunError::VerificationInput)
    );
    assert_eq!(run.state(), RunState::StructurallyVerified);
    assert!(run.verification().is_some());
    run.review().unwrap();
}

#[test]
fn p2e_input_withdraws_a_candidate_changed_in_staging() {
    let e = env();
    let (mut run, _) = verified(&e, default_edits());
    let staging = run.staging_path_for_test().unwrap();
    std::fs::write(staging.join("src/util.rs"), "pub fn changed() {}\n").unwrap();
    let (path, fd) = input_dir(&e);
    assert_eq!(
        run.materialize_verification_input(fd),
        Err(RunError::CandidateChanged)
    );
    // Nothing of the changed candidate reaches a verifier, and it can never
    // be approved.
    assert!(!path.join("src/util.rs").exists());
    assert!(run.verification().is_none());
    assert!(run.review().is_err());
    let (_, fd) = input_dir(&e);
    assert!(run.materialize_verification_input(fd).is_err());
}

#[test]
fn p2e_input_requires_a_verified_candidate_not_yet_applied() {
    let e = env();
    let mut staged = staged_run(&e.f);
    let (_, fd) = input_dir(&e);
    assert!(matches!(
        staged.materialize_verification_input(fd),
        Err(RunError::InvalidState { .. })
    ));
    let (mut run, binding) = verified(&e, default_edits());
    approve_and_apply(&e, &mut run, binding).unwrap();
    let (_, fd) = input_dir(&e);
    assert!(matches!(
        run.materialize_verification_input(fd),
        Err(RunError::InvalidState { .. })
    ));
}
