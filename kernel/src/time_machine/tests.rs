use super::*;
use crate::workspace_authority::WorkspaceAuthoritySource;
use serde_json::json;

fn small_config() -> TimeMachineConfig {
    TimeMachineConfig {
        max_checkpoints: 200,
        max_file_size_bytes: 10_485_760,
        auto_checkpoint: true,
    }
}

/// A backend-issued grant over a fresh canonical root, plus a victim file
/// outside that root which no replay may touch.
struct Fixture {
    _dir: tempfile::TempDir,
    base: PathBuf,
    root: PathBuf,
    victim: PathBuf,
    registry: WorkspaceAuthorityRegistry,
    binding: WorkspaceBinding,
    grant: WorkspaceGrantId,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let root = base.join("governed");
        std::fs::create_dir(&root).unwrap();
        let outside = base.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let victim = outside.join("victim.txt");
        std::fs::write(&victim, b"original").unwrap();
        let registry = WorkspaceAuthorityRegistry::new();
        let binding = WorkspaceBinding {
            agent_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
        };
        let grant = registry
            .issue_trusted_root(
                &root,
                binding,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                None,
            )
            .unwrap();
        Self {
            _dir: dir,
            base,
            root,
            victim,
            registry,
            binding,
            grant,
        }
    }

    fn authority(&self) -> FileAuthority<'_> {
        FileAuthority::new(&self.registry, self.binding)
    }

    fn victim_intact(&self) -> bool {
        std::fs::read(&self.victim).ok().as_deref() == Some(b"original".as_slice())
    }

    /// Creates `relative` beneath the root and commits a checkpoint recording it.
    fn created(&self, tm: &mut TimeMachine, relative: &str, content: &[u8]) -> String {
        let path = governed_path::join_relative(&self.root, relative).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let mut builder = tm.begin_checkpoint("create", None);
        builder
            .record_file_create(&self.authority(), self.grant, relative, content.to_vec())
            .unwrap();
        tm.commit_checkpoint(builder.build()).unwrap().0
    }
}

/// Replaces every recorded relative path in `tm`'s latest checkpoint.
fn tamper_relative(tm: &mut TimeMachine, relative: &str) {
    let cp = tm.checkpoints.last_mut().unwrap();
    for change in &mut cp.changes {
        if let ChangeEntry::FileWrite { file, .. }
        | ChangeEntry::FileDelete { file, .. }
        | ChangeEntry::FileCreate { file, .. } = change
        {
            file.relative = relative.to_string();
        }
    }
}

fn denied(result: Result<(Checkpoint, Vec<UndoAction>), TimeMachineError>) -> bool {
    matches!(result, Err(TimeMachineError::FileDenied { .. }))
}

// ---------------------------------------------------------------------------
// Grant-bound replay
// ---------------------------------------------------------------------------

#[test]
fn granted_create_undo_redo_round_trip() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "notes/a.txt", b"created");
    let file = f.root.join("notes").join("a.txt");

    tm.undo_with(Some(&f.authority())).unwrap();
    assert!(!file.exists());
    tm.redo_with(Some(&f.authority())).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"created");
    // The redo recorded the new file's identity, so it can be undone again.
    tm.undo_with(Some(&f.authority())).unwrap();
    assert!(!file.exists());
    assert!(f.victim_intact());
}

#[test]
fn granted_write_undo_restores_previous_content_and_redo_reapplies() {
    let f = Fixture::new();
    let file = f.root.join("doc.txt");
    std::fs::write(&file, b"v1").unwrap();
    std::fs::write(&file, b"v2").unwrap();
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("write", None);
    builder
        .record_file_write(
            &f.authority(),
            f.grant,
            "doc.txt",
            Some(b"v1".to_vec()),
            b"v2".to_vec(),
        )
        .unwrap();
    tm.commit_checkpoint(builder.build()).unwrap();

    tm.undo_with(Some(&f.authority())).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"v1");
    tm.redo_with(Some(&f.authority())).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"v2");
    tm.undo_with(Some(&f.authority())).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"v1");
}

#[test]
fn granted_delete_undo_restores_and_redo_deletes_only_the_restored_file() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("delete", None);
    builder
        .record_file_delete(&f.authority(), f.grant, "gone/x.txt", b"kept".to_vec())
        .unwrap();
    tm.commit_checkpoint(builder.build()).unwrap();

    tm.undo_with(Some(&f.authority())).unwrap();
    let file = f.root.join("gone").join("x.txt");
    assert_eq!(std::fs::read(&file).unwrap(), b"kept");
    tm.redo_with(Some(&f.authority())).unwrap();
    assert!(!file.exists());
}

#[test]
fn file_changes_without_authority_are_refused_and_left_untouched() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    let id = f.created(&mut tm, "a.txt", b"kept");
    let file = f.root.join("a.txt");

    assert!(matches!(
        tm.undo(),
        Err(TimeMachineError::FileAuthorityRequired)
    ));
    assert!(matches!(
        tm.undo_checkpoint(&id),
        Err(TimeMachineError::FileAuthorityRequired)
    ));
    assert!(file.exists());
    assert!(!tm.get_checkpoint(&id).unwrap().undone);
    assert!(tm.redo_stack.is_empty());
    assert!(matches!(tm.redo(), Err(TimeMachineError::RedoFailed(_))));
}

// ---------------------------------------------------------------------------
// Raw and hostile recorded paths
// ---------------------------------------------------------------------------

#[test]
fn legacy_raw_path_entries_do_not_deserialize() {
    for legacy in [
        r#"{"FileCreate":{"path":"/etc/passwd","after":[1]}}"#,
        r#"{"FileWrite":{"path":"/etc/passwd","before":null,"after":[1]}}"#,
        r#"{"FileDelete":{"path":"C:\\Windows\\x","before":[1]}}"#,
    ] {
        assert!(
            serde_json::from_str::<ChangeEntry>(legacy).is_err(),
            "{legacy}"
        );
    }
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"x");
    let mut entry = serde_json::to_value(&tm.checkpoints[0].changes[0]).unwrap();
    entry["FileCreate"]["file"]["path"] = json!("/etc/passwd");
    assert!(serde_json::from_value::<ChangeEntry>(entry).is_err());
}

#[test]
fn arbitrary_absolute_path_entries_cannot_undo_or_redo() {
    let f = Fixture::new();
    let absolute = f.victim.to_string_lossy().into_owned();
    for hostile in [
        absolute.as_str(),
        "/etc/passwd",
        "C:\\Windows\\win.ini",
        "\\\\server\\share\\x",
    ] {
        let mut tm = TimeMachine::new(small_config());
        f.created(&mut tm, "a.txt", b"original");
        tamper_relative(&mut tm, hostile);
        assert!(denied(tm.undo_with(Some(&f.authority()))), "{hostile}");
        assert!(f.victim_intact());
        assert!(f.root.join("a.txt").exists());
    }
}

#[test]
fn absolute_entries_are_refused_even_when_they_name_the_recorded_file() {
    // Only the grant-relative path is authority. An absolute spelling of the
    // very object that was recorded, inside the root or through a hard link
    // outside it, still cannot undo: its identity and content would match.
    let f = Fixture::new();
    let inside = f.root.join("a.txt");
    let linked = f.base.join("outside").join("linked.txt");
    for spelling in [&inside, &linked] {
        let mut tm = TimeMachine::new(small_config());
        f.created(&mut tm, "a.txt", b"original");
        if spelling == &linked {
            std::fs::hard_link(&inside, &linked).unwrap();
        }
        tamper_relative(&mut tm, &spelling.to_string_lossy());
        assert!(
            denied(tm.undo_with(Some(&f.authority()))),
            "{}",
            spelling.display()
        );
        assert!(inside.exists() && spelling.exists());
        assert!(f.victim_intact());
    }
}

#[test]
fn parent_traversal_cannot_escape_the_grant_root() {
    let f = Fixture::new();
    for hostile in [
        "../outside/victim.txt",
        "a/../../outside/victim.txt",
        "./a.txt",
        "a//b",
    ] {
        let mut tm = TimeMachine::new(small_config());
        f.created(&mut tm, "a.txt", b"original");
        tamper_relative(&mut tm, hostile);
        assert!(denied(tm.undo_with(Some(&f.authority()))), "{hostile}");
        assert!(f.victim_intact());
    }
    let tm = TimeMachine::new(small_config());
    assert!(tm
        .begin_checkpoint("x", None)
        .record_file_create(
            &f.authority(),
            f.grant,
            "../outside/victim.txt",
            b"original".to_vec()
        )
        .is_err());
    assert!(f.victim_intact());
}

#[test]
fn original_files_are_never_deleted_because_an_entry_says_create() {
    let f = Fixture::new();
    // An original file, never created by any checkpoint.
    let important = f.root.join("important.txt");
    std::fs::write(&important, b"precious").unwrap();

    // A crafted entry claiming the file was created, with a foreign identity.
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "decoy.txt", b"precious");
    tamper_relative(&mut tm, "important.txt");
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert_eq!(std::fs::read(&important).unwrap(), b"precious");

    // The same entry with no recorded identity at all.
    if let ChangeEntry::FileCreate { identity, .. } = &mut tm.checkpoints[0].changes[0] {
        *identity = None;
    }
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert_eq!(std::fs::read(&important).unwrap(), b"precious");

    // A recorded file replaced by another with the same content.
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "replaced.txt", b"same");
    let replaced = f.root.join("replaced.txt");
    std::fs::rename(&replaced, f.root.join("moved.txt")).unwrap();
    std::fs::write(&replaced, b"same").unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(replaced.exists());
}

#[test]
fn changed_content_denies_undo() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"recorded");
    let file = f.root.join("a.txt");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(b" and edited")
        .unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert_eq!(std::fs::read(&file).unwrap(), b"recorded and edited");
}

// ---------------------------------------------------------------------------
// Workspace, binding and root identity
// ---------------------------------------------------------------------------

#[test]
fn wrong_workspace_cannot_restore() {
    let f = Fixture::new();
    let other_root = f.base.join("other");
    std::fs::create_dir(&other_root).unwrap();
    let other = f
        .registry
        .issue_trusted_root(
            &other_root,
            f.binding,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadWrite,
            None,
        )
        .unwrap();
    std::fs::write(other_root.join("a.txt"), b"theirs").unwrap();

    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"theirs");
    if let ChangeEntry::FileCreate { file, .. } = &mut tm.checkpoints[0].changes[0] {
        file.grant = other;
    }
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(other_root.join("a.txt").exists());
    assert!(f.root.join("a.txt").exists());
}

#[test]
fn wrong_run_or_agent_cannot_restore() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"kept");
    for binding in [
        WorkspaceBinding {
            agent_id: f.binding.agent_id,
            run_id: Uuid::new_v4(),
        },
        WorkspaceBinding {
            agent_id: Uuid::new_v4(),
            run_id: f.binding.run_id,
        },
    ] {
        let authority = FileAuthority::new(&f.registry, binding);
        assert!(denied(tm.undo_with(Some(&authority))));
        assert!(f.root.join("a.txt").exists());
    }
    // A forged entry claiming another binding does not match this authority.
    if let ChangeEntry::FileCreate { file, .. } = &mut tm.checkpoints[0].changes[0] {
        file.run_id = Uuid::new_v4();
    }
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.root.join("a.txt").exists());
}

#[test]
fn replaced_governed_root_denies() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"kept");
    let old = f.base.join("governed.old");
    std::fs::rename(&f.root, &old).unwrap();
    std::fs::create_dir(&f.root).unwrap();
    std::fs::write(f.root.join("a.txt"), b"kept").unwrap();

    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(old.join("a.txt").exists());
    assert!(f.root.join("a.txt").exists());
}

#[test]
fn revoked_grant_denies() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"kept");
    f.registry.revoke(f.grant, f.binding).unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.root.join("a.txt").exists());
}

#[test]
fn serialized_entries_after_restart_recreate_no_authority() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"kept");
    let stored = serde_json::to_string(&tm.checkpoints[0]).unwrap();

    // A restart: fresh registry and Time Machine; the same binding even
    // re-issues a grant over the same root.
    let registry = WorkspaceAuthorityRegistry::new();
    registry
        .issue_trusted_root(
            &f.root,
            f.binding,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadWrite,
            None,
        )
        .unwrap();
    let mut restarted = TimeMachine::new(small_config());
    restarted
        .commit_checkpoint(serde_json::from_str(&stored).unwrap())
        .unwrap();
    assert!(matches!(
        restarted.undo(),
        Err(TimeMachineError::FileAuthorityRequired)
    ));
    let authority = FileAuthority::new(&registry, f.binding);
    assert!(denied(restarted.undo_with(Some(&authority))));
    assert!(f.root.join("a.txt").exists());
}

#[test]
fn read_only_grants_cannot_record_or_replay() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"kept");
    let registry = WorkspaceAuthorityRegistry::new();
    let read_only = registry
        .issue_trusted_root(
            &f.root,
            f.binding,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            None,
        )
        .unwrap();
    let authority = FileAuthority::new(&registry, f.binding);
    if let ChangeEntry::FileCreate { file, .. } = &mut tm.checkpoints[0].changes[0] {
        file.grant = read_only;
    }
    assert!(denied(tm.undo_with(Some(&authority))));
    assert!(tm
        .begin_checkpoint("x", None)
        .record_file_create(&authority, read_only, "a.txt", b"kept".to_vec())
        .is_err());
}

// ---------------------------------------------------------------------------
// Redirects
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn unix_redirected_targets_and_parents_deny() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "a.txt", b"original");
    let file = f.root.join("a.txt");
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&f.victim, &file).unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.victim_intact());

    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "dir/b.txt", b"original");
    let dir = f.root.join("dir");
    std::fs::rename(&dir, f.base.join("dir.old")).unwrap();
    std::os::unix::fs::symlink(f.victim.parent().unwrap(), &dir).unwrap();
    std::fs::copy(&f.victim, f.victim.with_file_name("b.txt")).unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.victim.with_file_name("b.txt").exists());
}

#[cfg(windows)]
#[test]
fn windows_reparse_redirected_parents_deny() {
    use std::os::windows::fs::symlink_dir;
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    f.created(&mut tm, "dir/b.txt", b"original");
    let dir = f.root.join("dir");
    std::fs::rename(&dir, f.base.join("dir.old")).unwrap();
    symlink_dir(f.victim.parent().unwrap(), &dir)
        .expect("native Windows test requires symlink creation privilege");
    std::fs::copy(&f.victim, f.victim.with_file_name("b.txt")).unwrap();
    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.victim.with_file_name("b.txt").exists());
}

// ---------------------------------------------------------------------------
// Partial failure
// ---------------------------------------------------------------------------

#[test]
fn a_denied_change_applies_nothing() {
    let f = Fixture::new();
    for name in ["one.txt", "two.txt"] {
        std::fs::write(f.root.join(name), b"x").unwrap();
    }
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("two", None);
    for name in ["one.txt", "two.txt"] {
        builder
            .record_file_create(&f.authority(), f.grant, name, b"x".to_vec())
            .unwrap();
    }
    let id = tm.commit_checkpoint(builder.build()).unwrap().0;
    std::fs::write(f.root.join("one.txt"), b"edited").unwrap();

    assert!(denied(tm.undo_with(Some(&f.authority()))));
    assert!(f.root.join("two.txt").exists());
    assert!(!tm.get_checkpoint(&id).unwrap().undone);
}

#[test]
fn partial_failure_is_truthful_and_fail_closed() {
    let f = Fixture::new();
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("two deletes", None);
    for name in ["late.txt", "early.txt"] {
        builder
            .record_file_delete(&f.authority(), f.grant, name, name.as_bytes().to_vec())
            .unwrap();
    }
    let id = tm.commit_checkpoint(builder.build()).unwrap().0;

    // Undo runs in reverse: early.txt is restored, then late.txt fails.
    FAIL_APPLY.with(|fail| fail.set(Some("late.txt")));
    let result = tm.undo_with(Some(&f.authority()));
    FAIL_APPLY.with(|fail| fail.set(None));
    let error = result.unwrap_err();
    assert!(matches!(
        error,
        TimeMachineError::PartiallyApplied {
            applied: 1,
            total: 2,
            ..
        }
    ));
    assert!(error.to_string().contains("1 of 2"));
    assert!(f.root.join("early.txt").exists());
    assert!(!f.root.join("late.txt").exists());
    assert!(!tm.get_checkpoint(&id).unwrap().undone);
    assert!(tm.redo_stack.is_empty());
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

#[test]
fn recording_requires_a_live_grant_a_valid_path_and_the_real_state() {
    let f = Fixture::new();
    std::fs::write(f.root.join("a.txt"), b"real").unwrap();
    let tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("x", None);
    let stranger = FileAuthority::new(
        &f.registry,
        WorkspaceBinding {
            agent_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
        },
    );
    assert!(builder
        .record_file_create(&stranger, f.grant, "a.txt", b"real".to_vec())
        .is_err());
    for hostile in ["", "/etc/passwd", "../a.txt", "C:\\a.txt", "a.txt:ads"] {
        assert!(builder
            .record_file_create(&f.authority(), f.grant, hostile, b"real".to_vec())
            .is_err());
    }
    assert!(builder
        .record_file_create(
            &f.authority(),
            f.grant,
            "a.txt",
            b"not the content".to_vec()
        )
        .is_err());
    assert!(builder
        .record_file_delete(&f.authority(), f.grant, "a.txt", b"real".to_vec())
        .is_err());
    builder
        .record_file_create(&f.authority(), f.grant, "a.txt", b"real".to_vec())
        .unwrap();
    assert!(builder
        .record_file_write(&f.authority(), f.grant, "a.txt", None, b"real".to_vec())
        .is_err());
    assert_eq!(builder.change_count(), 1);
}

#[test]
fn oversized_file_changes_are_skipped() {
    let f = Fixture::new();
    let tm = TimeMachine::new(TimeMachineConfig {
        max_file_size_bytes: 100,
        ..TimeMachineConfig::default()
    });
    let mut builder = tm.begin_checkpoint("big", None);
    builder
        .record_file_write(
            &f.authority(),
            f.grant,
            "big.bin",
            Some(vec![0; 50]),
            vec![0; 200],
        )
        .unwrap();
    builder
        .record_file_write(
            &f.authority(),
            f.grant,
            "big.bin",
            Some(vec![0; 200]),
            vec![0; 50],
        )
        .unwrap();
    builder
        .record_file_create(&f.authority(), f.grant, "huge.bin", vec![0; 200])
        .unwrap();
    builder
        .record_file_delete(&f.authority(), f.grant, "huge.bin", vec![0; 200])
        .unwrap();
    assert_eq!(builder.change_count(), 0);
}

// ---------------------------------------------------------------------------
// Non-file changes (unchanged behaviour)
// ---------------------------------------------------------------------------

#[test]
fn test_config_defaults() {
    let cfg = TimeMachineConfig::default();
    assert_eq!(cfg.max_checkpoints, 200);
    assert_eq!(cfg.max_file_size_bytes, 10_485_760);
    assert!(cfg.auto_checkpoint);
}

#[test]
fn test_create_checkpoint() {
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("test cp", None);
    builder.record_agent_state("a1", "fuel", json!(100), json!(90));
    builder.record_config_change("theme", json!("dark"), json!("light"));
    let cp = builder.build();
    assert_eq!(cp.changes.len(), 2);
    assert!(!cp.undone);

    let (id, evicted) = tm.commit_checkpoint(cp).unwrap();
    assert_eq!(evicted, 0);
    assert_eq!(tm.checkpoint_count(), 1);
    assert!(tm.get_checkpoint(&id).is_some());
}

#[test]
fn test_capacity_eviction() {
    let cfg = TimeMachineConfig {
        max_checkpoints: 3,
        ..TimeMachineConfig::default()
    };
    let mut tm = TimeMachine::new(cfg);

    let mut ids = Vec::new();
    for i in 0..5 {
        let builder = tm.begin_checkpoint(&format!("cp{i}"), None);
        let cp = builder.build();
        let (id, _) = tm.commit_checkpoint(cp).unwrap();
        ids.push(id);
    }

    assert_eq!(tm.checkpoint_count(), 3);
    assert!(tm.get_checkpoint(&ids[0]).is_none());
    assert!(tm.get_checkpoint(&ids[1]).is_none());
    assert!(tm.get_checkpoint(&ids[2]).is_some());
    assert!(tm.get_checkpoint(&ids[3]).is_some());
    assert!(tm.get_checkpoint(&ids[4]).is_some());
}

#[test]
fn test_undo_agent_state() {
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("state change", Some("agent-x".into()));
    builder.record_agent_state("agent-x", "autonomy_level", json!(3), json!(5));
    let cp = builder.build();
    tm.commit_checkpoint(cp).unwrap();

    let (_, actions) = tm.undo().unwrap();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        UndoAction::RestoreAgentState {
            agent_id,
            field,
            value,
        } => {
            assert_eq!(agent_id, "agent-x");
            assert_eq!(field, "autonomy_level");
            assert_eq!(*value, json!(3));
        }
        other => panic!("expected RestoreAgentState, got {other:?}"),
    }
}

#[test]
fn test_empty_undo_error() {
    let mut tm = TimeMachine::new(small_config());
    assert!(matches!(
        tm.undo().unwrap_err(),
        TimeMachineError::EmptyHistory
    ));
}

#[test]
fn test_redo_empty_error() {
    let mut tm = TimeMachine::new(small_config());
    assert!(matches!(
        tm.redo().unwrap_err(),
        TimeMachineError::RedoFailed(_)
    ));
}

#[test]
fn test_new_checkpoint_clears_redo() {
    let mut tm = TimeMachine::new(small_config());

    let mut builder = tm.begin_checkpoint("first", None);
    builder.record_agent_state("a", "f", json!(1), json!(2));
    tm.commit_checkpoint(builder.build()).unwrap();

    tm.undo().unwrap();
    assert_eq!(tm.redo_stack.len(), 1);

    let builder2 = tm.begin_checkpoint("second", None);
    tm.commit_checkpoint(builder2.build()).unwrap();
    assert!(tm.redo_stack.is_empty());
}

#[test]
fn test_redo_after_undo_of_non_file_changes() {
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("state", None);
    builder.record_agent_state("a", "status", json!("stopped"), json!("running"));
    tm.commit_checkpoint(builder.build()).unwrap();
    tm.undo().unwrap();
    let (cp, actions) = tm.redo().unwrap();
    assert!(!cp.undone);
    assert!(matches!(
        &actions[0],
        UndoAction::RestoreAgentState { value, .. } if *value == json!("running")
    ));
}

#[test]
fn test_undo_already_undone() {
    let mut tm = TimeMachine::new(small_config());
    let builder = tm.begin_checkpoint("only", None);
    tm.commit_checkpoint(builder.build()).unwrap();

    tm.undo().unwrap();
    assert!(matches!(
        tm.undo().unwrap_err(),
        TimeMachineError::EmptyHistory
    ));
}

#[test]
fn test_selective_undo_already_undone() {
    let mut tm = TimeMachine::new(small_config());
    let builder = tm.begin_checkpoint("cp", None);
    let (id, _) = tm.commit_checkpoint(builder.build()).unwrap();

    tm.undo_checkpoint(&id).unwrap();
    assert!(matches!(
        tm.undo_checkpoint(&id).unwrap_err(),
        TimeMachineError::UndoFailed(_)
    ));
    assert!(tm.redo_stack.is_empty());
}

#[test]
fn test_selective_undo_not_found() {
    let mut tm = TimeMachine::new(small_config());
    assert!(matches!(
        tm.undo_checkpoint("nonexistent-id").unwrap_err(),
        TimeMachineError::CheckpointNotFound(_)
    ));
}

#[test]
fn test_undo_config_change() {
    let mut tm = TimeMachine::new(small_config());
    let mut builder = tm.begin_checkpoint("config edit", None);
    builder.record_config_change("theme", json!("dark"), json!("light"));
    tm.commit_checkpoint(builder.build()).unwrap();

    let (_, actions) = tm.undo().unwrap();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        UndoAction::RestoreConfig { key, value } => {
            assert_eq!(key, "theme");
            assert_eq!(*value, json!("dark"));
        }
        other => panic!("expected RestoreConfig, got {other:?}"),
    }
}
