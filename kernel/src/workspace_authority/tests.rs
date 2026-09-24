use super::*;
use std::process::Command;
use std::sync::{mpsc, Arc, Barrier};
use std::time::Duration;
use tempfile::TempDir;

fn binding() -> WorkspaceBinding {
    WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    }
}

fn issue(
    registry: &WorkspaceAuthorityRegistry,
    root: &Path,
    owner: WorkspaceBinding,
    permission: FsPermissionLevel,
) -> WorkspaceGrantId {
    registry
        .issue_trusted_root(
            root,
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            permission,
            None,
        )
        .unwrap()
}

fn fixture() -> (TempDir, WorkspaceAuthorityRegistry, WorkspaceBinding) {
    (
        TempDir::new().unwrap(),
        WorkspaceAuthorityRegistry::new(),
        binding(),
    )
}

#[test]
fn p0_002c1_valid_issue_is_canonical_bound_and_non_mutating() {
    let (temp, registry, owner) = fixture();
    let root = temp.path().join("project-A");
    std::fs::create_dir(&root).unwrap();
    let before = SystemTime::now();
    let expiry = before + Duration::from_secs(3600);
    let id = registry
        .issue_trusted_root(
            &root.join("."),
            owner,
            WorkspaceAuthoritySource::UserSelected,
            FsPermissionLevel::ReadWrite,
            Some(expiry),
        )
        .unwrap();
    let serialized = serde_json::to_value(id).unwrap();
    assert!(Uuid::parse_str(serialized.as_str().unwrap()).is_ok());
    let grant = registry.resolve(id, owner).unwrap();
    assert_eq!(grant.id(), id);
    assert_eq!(grant.root(), root.canonicalize().unwrap());
    assert_eq!(grant.binding(), owner);
    assert_eq!(grant.source(), WorkspaceAuthoritySource::UserSelected);
    assert_eq!(grant.permission(), &FsPermissionLevel::ReadWrite);
    assert_eq!(grant.parent(), None);
    assert!(grant.issued_at() >= before && grant.issued_at() <= SystemTime::now());
    assert_eq!(grant.expires_at(), Some(expiry));
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    assert_eq!(registry.resolve(id, owner).unwrap().root(), grant.root());
}

#[test]
fn p0_002c1_relative_root_denied() {
    let (_, registry, owner) = fixture();
    for root in ["", ".", "..", "project-A"] {
        assert_eq!(
            registry.issue_trusted_root(
                Path::new(root),
                owner,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadOnly,
                None,
            ),
            Err(WorkspaceAuthorityError::InvalidRoot)
        );
    }
}

#[test]
fn p0_002c1_missing_root_denied_without_mutation() {
    let (temp, registry, owner) = fixture();
    let missing = temp.path().join("nonexistent/subdir");
    assert_eq!(
        registry.issue_trusted_root(
            &missing,
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadWrite,
            None,
        ),
        Err(WorkspaceAuthorityError::InvalidRoot)
    );
    assert!(!missing.exists());
    assert!(!temp.path().join("nonexistent").exists());
    assert!(registry.grants.read().unwrap().is_empty());
}

#[test]
fn p0_002c1_file_root_denied() {
    let (temp, registry, owner) = fixture();
    let file = temp.path().join("file");
    std::fs::write(&file, "evidence").unwrap();
    assert_eq!(
        registry.issue_trusted_root(
            &file,
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            None,
        ),
        Err(WorkspaceAuthorityError::InvalidRoot)
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), "evidence");
}

#[test]
fn p0_002c1_unissued_handle_and_root_bearing_json_cannot_grant_authority() {
    let (_, registry, owner) = fixture();
    let id: WorkspaceGrantId =
        serde_json::from_value(serde_json::json!(Uuid::new_v4().to_string())).unwrap();
    assert_eq!(
        registry.resolve(id, owner).unwrap_err(),
        WorkspaceAuthorityError::UnknownGrant
    );
    assert_eq!(
        registry.revoke(id, owner),
        Err(WorkspaceAuthorityError::UnknownGrant)
    );
    assert_eq!(
        registry.narrow(
            id,
            owner,
            owner.agent_id,
            Path::new("."),
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::UnknownGrant)
    );
    for value in [
        serde_json::json!({"id": id, "root": "/", "permission": "ReadWrite"}),
        serde_json::json!("/"),
        serde_json::Value::Null,
    ] {
        assert!(serde_json::from_value::<WorkspaceGrantId>(value).is_err());
    }
    assert!(registry.grants.read().unwrap().is_empty());
}

#[test]
fn p0_002c1_wrong_owner_and_project_run_denied_for_all_operations() {
    let (temp, registry, owner) = fixture();
    let id = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadWrite);
    for (wrong, expected) in [
        (
            WorkspaceBinding {
                agent_id: Uuid::new_v4(),
                ..owner
            },
            WorkspaceAuthorityError::WrongOwner,
        ),
        (
            WorkspaceBinding {
                run_id: Uuid::new_v4(),
                ..owner
            },
            WorkspaceAuthorityError::WrongContext,
        ),
    ] {
        assert_eq!(registry.resolve(id, wrong).unwrap_err(), expected);
        assert_eq!(registry.revoke(id, wrong), Err(expected));
        assert_eq!(
            registry.narrow(
                id,
                wrong,
                wrong.agent_id,
                temp.path(),
                FsPermissionLevel::ReadOnly
            ),
            Err(expected)
        );
    }
    assert!(registry.resolve(id, owner).is_ok());
}

#[test]
fn p0_002c1_revocation_denies_resolution_and_narrowing() {
    let (temp, registry, owner) = fixture();
    let id = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadWrite);
    registry.revoke(id, owner).unwrap();
    registry.revoke(id, owner).unwrap();
    assert_eq!(
        registry.resolve(id, owner).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant
    );
    assert_eq!(
        registry.narrow(
            id,
            owner,
            owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::RevokedGrant)
    );
}

#[test]
fn p0_002c1_cwd_independence() {
    const CHILD: &str = "NEXUS_P0_002C1_CWD_CHILD";
    const NAME: &str = "workspace_authority::tests::p0_002c1_cwd_independence";
    const WITNESS: &str = "P0-002C1 cwd child completed";
    if std::env::var(CHILD).as_deref() == Ok(NAME) {
        let (temp, registry, owner) = fixture();
        let other = TempDir::new().unwrap();
        let id = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadOnly);
        let root = registry.resolve(id, owner).unwrap().root().to_path_buf();
        let original = std::env::current_dir().unwrap();
        // This process runs exactly this test; no parallel test can see the cwd.
        std::env::set_current_dir(other.path()).unwrap();
        assert_eq!(registry.resolve(id, owner).unwrap().root(), root);
        std::env::set_current_dir(original).unwrap();
        eprintln!("{WITNESS}");
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env(CHILD, NAME)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| *line == WITNESS)
            .count(),
        1
    );
}

#[test]
fn p0_002c1_valid_child_and_same_root_inheritance() {
    let (temp, registry, owner) = fixture();
    let path = temp.path().join("project-A/agents/forge");
    std::fs::create_dir_all(&path).unwrap();
    let parent = issue(
        &registry,
        &temp.path().join("project-A"),
        owner,
        FsPermissionLevel::ReadWrite,
    );
    let child_owner = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        ..owner
    };
    let child = registry
        .narrow(
            parent,
            owner,
            child_owner.agent_id,
            &path,
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    let grant = registry.resolve(child, child_owner).unwrap();
    assert_eq!(grant.root(), path.canonicalize().unwrap());
    assert_eq!(grant.parent(), Some(parent));
    assert_eq!(grant.source(), WorkspaceAuthoritySource::ParentGrant);
    assert_eq!(grant.binding(), child_owner);
    assert_eq!(grant.permission(), &FsPermissionLevel::ReadOnly);
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    let same = registry
        .narrow(
            child,
            child_owner,
            child_owner.agent_id,
            &path,
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    assert_eq!(
        registry.resolve(same, child_owner).unwrap().root(),
        grant.root()
    );
    assert_eq!(
        registry.resolve(child, owner).unwrap_err(),
        WorkspaceAuthorityError::WrongOwner
    );
}

#[test]
fn p0_002c1_ancestor_sibling_prefix_and_parent_traversal_widening_denied() {
    let (temp, registry, owner) = fixture();
    for path in [
        "project-A/agents/forge",
        "project-A/agents/other",
        "project-B",
        "project-A-evil",
    ] {
        std::fs::create_dir_all(temp.path().join(path)).unwrap();
    }
    let project = temp.path().join("project-A");
    let parent = issue(&registry, &project, owner, FsPermissionLevel::ReadWrite);
    for outside in [
        temp.path().to_path_buf(),
        temp.path().join("project-B"),
        temp.path().join("project-A-evil"),
        project.join("../project-B"),
    ] {
        assert_eq!(
            registry.narrow(
                parent,
                owner,
                owner.agent_id,
                &outside,
                FsPermissionLevel::ReadOnly
            ),
            Err(WorkspaceAuthorityError::RootWidening)
        );
    }
    let child = registry
        .narrow(
            parent,
            owner,
            owner.agent_id,
            &project.join("agents/forge"),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    for outside in [&project, &project.join("agents/other")] {
        assert_eq!(
            registry.narrow(
                child,
                owner,
                owner.agent_id,
                outside,
                FsPermissionLevel::ReadOnly
            ),
            Err(WorkspaceAuthorityError::RootWidening)
        );
    }
}

#[test]
fn p0_002c1_permission_escalation_denied_and_all_subsets_succeed() {
    let (temp, registry, owner) = fixture();
    use FsPermissionLevel::{Deny, ReadOnly, ReadWrite};
    let levels = [Deny, ReadOnly, ReadWrite];
    for (parent_rank, parent_permission) in levels.iter().enumerate() {
        let parent = issue(&registry, temp.path(), owner, parent_permission.clone());
        for (child_rank, child_permission) in levels.iter().enumerate() {
            let result = registry.narrow(
                parent,
                owner,
                owner.agent_id,
                temp.path(),
                child_permission.clone(),
            );
            if child_rank > parent_rank {
                assert_eq!(result, Err(WorkspaceAuthorityError::PermissionWidening));
            } else {
                let child = registry.resolve(result.unwrap(), owner).unwrap();
                assert_eq!(child.permission(), child_permission);
            }
        }
    }
}

#[test]
fn p0_002c1_ancestor_revocation_invalidates_only_its_subtree() {
    let (temp, registry, owner) = fixture();
    let parent = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadWrite);
    let child_owner = WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        ..owner
    };
    let child = registry
        .narrow(
            parent,
            owner,
            child_owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    let grandchild = registry
        .narrow(
            child,
            child_owner,
            child_owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    let sibling = registry
        .narrow(
            parent,
            owner,
            owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    registry.revoke(child, child_owner).unwrap();
    assert!(registry.resolve(parent, owner).is_ok());
    assert!(registry.resolve(sibling, owner).is_ok());
    for id in [child, grandchild] {
        assert_eq!(
            registry.resolve(id, child_owner).unwrap_err(),
            WorkspaceAuthorityError::RevokedGrant
        );
        assert_eq!(
            registry.narrow(
                id,
                child_owner,
                child_owner.agent_id,
                temp.path(),
                FsPermissionLevel::ReadOnly
            ),
            Err(WorkspaceAuthorityError::RevokedGrant)
        );
    }
    registry.revoke(parent, owner).unwrap();
    assert_eq!(
        registry.resolve(sibling, owner).unwrap_err(),
        WorkspaceAuthorityError::RevokedGrant
    );
}

#[test]
fn p0_002c1_invalid_child_roots_never_mutate_filesystem() {
    let (temp, registry, owner) = fixture();
    let parent = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadWrite);
    let file = temp.path().join("file");
    std::fs::write(&file, "evidence").unwrap();
    for root in [PathBuf::from("."), temp.path().join("missing/child"), file] {
        assert_eq!(
            registry.narrow(
                parent,
                owner,
                owner.agent_id,
                &root,
                FsPermissionLevel::ReadOnly
            ),
            Err(WorkspaceAuthorityError::InvalidRoot)
        );
    }
    assert!(!temp.path().join("missing").exists());
}

#[cfg(unix)]
#[test]
fn p0_002c1_symlinks_are_canonicalized_and_cannot_widen() {
    use std::os::unix::fs::symlink;
    let (temp, registry, owner) = fixture();
    let project = temp.path().join("project");
    let inside = project.join("inside");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir(&outside).unwrap();
    symlink(&project, temp.path().join("alias")).unwrap();
    symlink(&inside, project.join("inside-link")).unwrap();
    symlink(&outside, project.join("outside-link")).unwrap();
    symlink(temp.path().join("missing"), project.join("dangling-link")).unwrap();
    let parent = issue(
        &registry,
        &temp.path().join("alias"),
        owner,
        FsPermissionLevel::ReadWrite,
    );
    assert_eq!(
        registry.resolve(parent, owner).unwrap().root(),
        project.canonicalize().unwrap()
    );
    let child = registry
        .narrow(
            parent,
            owner,
            owner.agent_id,
            &project.join("inside-link"),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    assert_eq!(
        registry.resolve(child, owner).unwrap().root(),
        inside.canonicalize().unwrap()
    );
    assert_eq!(
        registry.narrow(
            parent,
            owner,
            owner.agent_id,
            &project.join("outside-link"),
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::RootWidening)
    );
    assert_eq!(
        registry.narrow(
            parent,
            owner,
            owner.agent_id,
            &project.join("dangling-link"),
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::InvalidRoot)
    );
    assert!(!temp.path().join("missing").exists());
}

#[test]
fn p0_002c1_lookup_never_recreates_or_recanonicalizes_a_root() {
    let (temp, registry, owner) = fixture();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let parent = issue(&registry, &root, owner, FsPermissionLevel::ReadOnly);
    let canonical = root.canonicalize().unwrap();
    std::fs::remove_dir(&root).unwrap();
    assert_eq!(registry.resolve(parent, owner).unwrap().root(), canonical);
    assert!(!root.exists());
    assert_eq!(
        registry.narrow(
            parent,
            owner,
            owner.agent_id,
            &root,
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::InvalidRoot)
    );
    assert!(!root.exists());
}

#[test]
fn p0_002c1_expiry_is_inherited_and_fails_closed_at_deadline() {
    let (temp, registry, owner) = fixture();
    let expiry = SystemTime::now() + Duration::from_secs(3600);
    let parent = registry
        .issue_trusted_root(
            temp.path(),
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadWrite,
            Some(expiry),
        )
        .unwrap();
    let child = registry
        .narrow(
            parent,
            owner,
            owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly,
        )
        .unwrap();
    assert_eq!(
        registry.resolve(child, owner).unwrap().expires_at(),
        Some(expiry)
    );
    // Exercise the same lifecycle check used by resolve/narrow with a fixed
    // deadline, without sleeping or racing the wall clock.
    // Windows SystemTime cannot represent a one-nanosecond separation.
    let before = expiry - Duration::from_millis(1);
    let after = expiry + Duration::from_millis(1);
    assert!(before < expiry && expiry < after);
    let grants = registry.grants.read().unwrap();
    for id in [parent, child] {
        assert!(active_grant(&grants, id, owner, before).is_ok());
        assert_eq!(
            active_grant(&grants, id, owner, expiry).unwrap_err(),
            WorkspaceAuthorityError::ExpiredGrant
        );
        assert_eq!(
            active_grant(&grants, id, owner, after).unwrap_err(),
            WorkspaceAuthorityError::ExpiredGrant
        );
    }
    drop(grants);
    assert_eq!(
        registry.issue_trusted_root(
            temp.path(),
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            Some(SystemTime::UNIX_EPOCH)
        ),
        Err(WorkspaceAuthorityError::InvalidExpiry)
    );
}

#[test]
fn p0_002c1_root_issuance_rejects_fake_parent_provenance_and_empty_bindings() {
    let (temp, registry, owner) = fixture();
    assert_eq!(
        registry.issue_trusted_root(
            temp.path(),
            owner,
            WorkspaceAuthoritySource::ParentGrant,
            FsPermissionLevel::ReadOnly,
            None
        ),
        Err(WorkspaceAuthorityError::InvalidSource)
    );
    for invalid in [
        WorkspaceBinding {
            agent_id: Uuid::nil(),
            ..owner
        },
        WorkspaceBinding {
            run_id: Uuid::nil(),
            ..owner
        },
    ] {
        assert_eq!(
            registry.issue_trusted_root(
                temp.path(),
                invalid,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadOnly,
                None
            ),
            Err(WorkspaceAuthorityError::InvalidBinding)
        );
    }
    assert!(registry.grants.read().unwrap().is_empty());
}

#[test]
fn p0_002c1_concurrent_independent_grants_and_revocation() {
    const WORKERS: usize = 8;
    let temp = TempDir::new().unwrap();
    let registry = Arc::new(WorkspaceAuthorityRegistry::new());
    let ready = Arc::new(Barrier::new(WORKERS + 1));
    let revoked = Arc::new(Barrier::new(WORKERS + 1));
    let (send, receive) = mpsc::channel();
    let mut workers = Vec::new();
    for index in 0..WORKERS {
        let path = temp.path().join(format!("project-{index}"));
        std::fs::create_dir(&path).unwrap();
        let registry = Arc::clone(&registry);
        let ready = Arc::clone(&ready);
        let revoked = Arc::clone(&revoked);
        let send = send.clone();
        workers.push(std::thread::spawn(move || {
            let owner = binding();
            let id = issue(&registry, &path, owner, FsPermissionLevel::ReadOnly);
            assert_eq!(
                registry.resolve(id, owner).unwrap().root(),
                path.canonicalize().unwrap()
            );
            send.send((index, id, owner)).unwrap();
            ready.wait();
            revoked.wait();
            if index == 0 {
                assert_eq!(
                    registry.resolve(id, owner).unwrap_err(),
                    WorkspaceAuthorityError::RevokedGrant
                );
            } else {
                assert_eq!(
                    registry.resolve(id, owner).unwrap().root(),
                    path.canonicalize().unwrap()
                );
            }
        }));
    }
    let grants: Vec<_> = (0..WORKERS).map(|_| receive.recv().unwrap()).collect();
    let ids: std::collections::HashSet<_> = grants.iter().map(|(_, id, _)| *id).collect();
    assert_eq!(ids.len(), WORKERS);
    ready.wait();
    let (_, id, owner) = grants.iter().find(|(index, _, _)| *index == 0).unwrap();
    registry.revoke(*id, *owner).unwrap();
    revoked.wait();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn p0_002c1_registry_instances_do_not_share_authority() {
    let (temp, registry, owner) = fixture();
    let id = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadOnly);
    assert_eq!(
        WorkspaceAuthorityRegistry::new()
            .resolve(id, owner)
            .unwrap_err(),
        WorkspaceAuthorityError::UnknownGrant
    );
}

#[test]
fn p0_002c1_poisoned_registry_fails_closed() {
    let (temp, registry, owner) = fixture();
    let id = issue(&registry, temp.path(), owner, FsPermissionLevel::ReadOnly);
    let _ = std::panic::catch_unwind(|| {
        let _guard = registry.grants.write().unwrap();
        panic!("poison registry in isolated fixture");
    });
    assert_eq!(
        registry.resolve(id, owner).unwrap_err(),
        WorkspaceAuthorityError::RegistryUnavailable
    );
    assert_eq!(
        registry.revoke(id, owner),
        Err(WorkspaceAuthorityError::RegistryUnavailable)
    );
    assert_eq!(
        registry.narrow(
            id,
            owner,
            owner.agent_id,
            temp.path(),
            FsPermissionLevel::ReadOnly
        ),
        Err(WorkspaceAuthorityError::RegistryUnavailable)
    );
    assert_eq!(
        registry.issue_trusted_root(
            temp.path(),
            owner,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            None
        ),
        Err(WorkspaceAuthorityError::RegistryUnavailable)
    );
}
