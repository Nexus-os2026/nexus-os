//! P2-ENTRY-H1 desktop guards, added before any Phase Two authority work.
//!
//! `p2e_h1_tm_*`: Time Machine undo, redo and undo-to-checkpoint are closed
//! commands. They take no input, only deny, and leave agent state, fuel,
//! memories, the saved governance configuration and owner files unchanged;
//! the desktop keeps no checkpoint-replay path. (The workspace-wide guard
//! that only the kernel safety halt forces an agent state is in
//! `phase0_surface/tests.rs`, beside the production-text scanner it uses.)
//!
//! `p2e_h1_rg_d*`: the governed coding flow's failure, discard and start
//! failure paths leave no run read grant live. The kernel's `p2e_h1_rg_*`
//! tests cover the run's own lifecycle.

use super::*;
use crate::phase0_surface::{closed, Closure};

const REPLAY_COMMANDS: [&str; 3] = [
    "time_machine_undo",
    "time_machine_redo",
    "time_machine_undo_checkpoint",
];

/// A closed replay command's name and its registered handler, invoked with no
/// input.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
type ReplayHandler = (&'static str, fn() -> Result<String, String>);

/// The registered handlers of the closed replay commands.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn replay_handlers() -> [ReplayHandler; 3] {
    [
        ("time_machine_undo", crate::runtime::time_machine_undo),
        ("time_machine_redo", crate::runtime::time_machine_redo),
        (
            "time_machine_undo_checkpoint",
            crate::runtime::time_machine_undo_checkpoint,
        ),
    ]
}

fn production_sources() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "tests" {
                    walk(&path, out);
                }
            } else if name.ends_with(".rs") && !name.ends_with("tests.rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut out,
    );
    assert!(out.len() > 20, "desktop sources not found");
    out
}

/// A temporary owner directory, removed when dropped.
struct OwnerDir(std::path::PathBuf);

impl OwnerDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("nexus-p2e-h1-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }
}

impl Drop for OwnerDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p2e_h1_tm_01_replay_commands_take_no_input_and_only_deny() {
    let reason = Closure::CheckpointReplay.reason();
    assert!(reason.len() <= 160, "{reason}");
    assert!(!reason.contains('/') && !reason.contains('\\'), "{reason}");
    for (name, handler) in replay_handlers() {
        assert_eq!(
            handler(),
            Err(closed(name, Closure::CheckpointReplay)),
            "{name}"
        );
    }
    let lib = include_str!("lib.rs");
    for name in REPLAY_COMMANDS {
        assert_eq!(
            lib.matches(&format!("fn {name}()")).count(),
            1,
            "{name} stays registered and takes no input"
        );
    }
}

#[test]
fn p2e_h1_tm_02_replay_attempts_change_no_agent_state_fuel_memory_config_or_file() {
    use nexus_kernel::lifecycle::AgentState;
    use nexus_kernel::manifest::FsPermissionLevel;
    use nexus_kernel::time_machine::FileAuthority;
    use nexus_kernel::workspace_authority::{
        WorkspaceAuthorityRegistry, WorkspaceAuthoritySource, WorkspaceBinding,
    };

    let state = AppState::new_in_memory();
    let manifest = serde_json::json!({
        "name": "replay-probe",
        "version": "2.0.0",
        "capabilities": ["llm.query"],
        "fuel_budget": 10000,
        "schedule": null,
        "llm_model": "local"
    })
    .to_string();
    let id = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(parse_agent_manifest_json(&manifest).unwrap())
        .unwrap();
    let agent = id.to_string();
    state
        .db
        .save_memory(&agent, "episodic", "kept", "\"kept\"")
        .unwrap();

    // An owner file, recorded as changed beneath a live write grant.
    let owner = OwnerDir::new();
    std::fs::write(owner.0.join("owner.txt"), "owner's current bytes").unwrap();
    let registry = WorkspaceAuthorityRegistry::new();
    let binding = WorkspaceBinding {
        agent_id: id,
        run_id: uuid::Uuid::new_v4(),
    };
    let grant = registry
        .issue_trusted_root(
            &owner.0,
            binding,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadWrite,
            None,
        )
        .unwrap();

    // One checkpoint holding everything a replay used to restore: a state
    // the lifecycle forbids from Running (so replay forced it), refunded
    // fuel, no memories, the opposite Warden review and older file bytes.
    let (fuel, status) = {
        let supervisor = state.supervisor.lock().unwrap();
        let handle = supervisor.get_agent(id).unwrap();
        (handle.remaining_fuel, handle.state)
    };
    assert_eq!(status, AgentState::Running);
    let mut builder = state
        .supervisor
        .lock()
        .unwrap()
        .time_machine()
        .begin_checkpoint("replay-probe", Some(agent.clone()));
    builder.record_agent_state(
        &agent,
        "status",
        serde_json::json!("Created"),
        serde_json::json!("Running"),
    );
    builder.record_agent_state(
        &agent,
        "fuel_remaining",
        serde_json::json!(fuel + 999_999),
        serde_json::json!(fuel),
    );
    builder.record_agent_state(
        &agent,
        "memories",
        serde_json::json!([]),
        serde_json::json!(["kept"]),
    );
    builder.record_config_change(
        "governance.enable_warden_review",
        serde_json::json!(false),
        serde_json::json!(true),
    );
    builder
        .record_file_write(
            &FileAuthority::new(&registry, binding),
            grant,
            "owner.txt",
            Some(b"older bytes".to_vec()),
            b"owner's current bytes".to_vec(),
        )
        .unwrap();
    assert_eq!(builder.change_count(), 5);
    let checkpoint = commit_time_machine_checkpoint(&state, builder.build()).unwrap();

    let observe = |state: &AppState| {
        let supervisor = state.supervisor.lock().unwrap();
        let handle = supervisor.get_agent(id).unwrap();
        let history: Vec<(String, bool)> = supervisor
            .time_machine()
            .list_checkpoints()
            .iter()
            .map(|checkpoint| (checkpoint.id.clone(), checkpoint.undone))
            .collect();
        let memories: Vec<String> = state
            .db
            .load_memories(&agent, None, 250)
            .unwrap()
            .into_iter()
            .map(|memory| memory.key)
            .collect();
        (
            handle.state,
            handle.remaining_fuel,
            memories,
            history,
            std::fs::read(owner.0.join("owner.txt")).unwrap(),
        )
    };
    let before = observe(&state);
    assert!(before.3.contains(&(checkpoint, false)));

    // Every replay request is refused before anything is read or changed;
    // the handlers cannot even receive a checkpoint id.
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    for (name, handler) in replay_handlers() {
        assert_eq!(handler(), Err(closed(name, Closure::CheckpointReplay)));
    }

    let after = observe(&state);
    assert_eq!(
        after, before,
        "agent state, fuel, memories and file unchanged"
    );
    // No checkpoint was undone or redone, so no replay action exists to apply
    // to governance configuration; `p2e_h1_tm_04` observes the saved
    // configuration itself in an isolated child process.
    assert!(after.3.iter().all(|(_, undone)| !undone));
}

#[test]
fn p2e_h1_tm_03_the_desktop_keeps_no_checkpoint_replay_path() {
    for (file, text) in [
        ("model_hub.rs", include_str!("commands/model_hub.rs")),
        ("cognitive.rs", include_str!("commands/cognitive.rs")),
    ] {
        for needle in [
            "fn time_machine_undo",
            "fn time_machine_redo",
            "apply_non_file_undo_actions",
            "restore_agent_memories",
            "time-machine-undo",
            "time_machine.undo",
            "time_machine.redo",
        ] {
            assert!(!text.contains(needle), "{file} still contains {needle}");
        }
    }
    // The desktop still records checkpoints: its only mutable use of the
    // Time Machine commits one.
    let mut commits = 0;
    for (path, text) in production_sources() {
        for (at, _) in text.match_indices("time_machine_mut()") {
            let next: String = text[at + "time_machine_mut()".len()..]
                .chars()
                .filter(|c| !c.is_whitespace())
                .take(".commit_checkpoint(".len())
                .collect();
            assert_eq!(next, ".commit_checkpoint(", "{}", path.display());
            commits += 1;
        }
    }
    assert_eq!(commits, 1);
}

/// The saved governance configuration (Warden review) is unchanged by every
/// replay request, observed directly. `NEXUS_CONFIG_PATH` is process state,
/// so the observation runs in a child process that owns a private
/// configuration file: no other test can write it between the observations,
/// and the developer's own configuration is never read or written.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p2e_h1_tm_04_replay_attempts_leave_the_saved_governance_configuration_unchanged() {
    const CHILD: &str = "NEXUS_P2E_H1_CONFIG_CHILD";
    const NAME: &str = "p2_entry_tests::p2e_h1_tm_04_replay_attempts_leave_the_saved_governance_configuration_unchanged";
    const WITNESS: &str = "p2e-h1-tm-04 configuration witness";
    if std::env::var(CHILD).as_deref() == Ok(NAME) {
        use nexus_kernel::config::{config_path, load_config, save_config};
        // The private file: first-run default, then Warden review set to a
        // value the recorded checkpoint would undo.
        let path = config_path().unwrap();
        let mut config = load_config().unwrap();
        config.governance.enable_warden_review = true;
        save_config(&config).unwrap();
        let state = AppState::new_in_memory();
        let mut builder = state
            .supervisor
            .lock()
            .unwrap()
            .time_machine()
            .begin_checkpoint("config-replay-probe", None);
        builder.record_config_change(
            "governance.enable_warden_review",
            serde_json::json!(false),
            serde_json::json!(true),
        );
        commit_time_machine_checkpoint(&state, builder.build()).unwrap();
        let before = std::fs::read(&path).unwrap();
        for (name, handler) in replay_handlers() {
            assert_eq!(handler(), Err(closed(name, Closure::CheckpointReplay)));
        }
        assert_eq!(std::fs::read(&path).unwrap(), before, "configuration bytes");
        assert!(load_config().unwrap().governance.enable_warden_review);
        eprintln!("\n{WITNESS}");
        return;
    }
    let owner = OwnerDir::new();
    let (stdout, stderr) = (owner.0.join("stdout"), owner.0.join("stderr"));
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
        .env(CHILD, NAME)
        .env("NEXUS_ORACLE_EPHEMERAL", "1")
        .env("NEXUS_DB_PATH", ":memory:")
        .env("NEXUS_CONFIG_PATH", owner.0.join("config.toml"))
        .stdin(std::process::Stdio::null())
        .stdout(std::fs::File::create(&stdout).unwrap())
        .stderr(std::fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    // Bounded: the parent owns the child and kills and reaps it at the deadline.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("p2e_h1_tm_04 child exceeded its deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let stderr = std::fs::read_to_string(&stderr).unwrap();
    assert!(
        status.success(),
        "{status}\n{}\n{stderr}",
        std::fs::read_to_string(&stdout).unwrap()
    );
    assert_eq!(
        stderr.lines().filter(|line| *line == WITNESS).count(),
        1,
        "the child must run the observation exactly once\n{stderr}"
    );
}

#[cfg(target_os = "linux")]
mod read_grant {
    use crate::coding_flow::{discard_run, end_failed};
    use nexus_kernel::coding_run::{
        CodingRun, FolderPicker, LedgerStore, ProjectRegistry, RunScopes, RunState, ScopeEntry,
        ScopeSet,
    };
    use nexus_kernel::workspace_authority::{
        WorkspaceAuthorityError, WorkspaceAuthorityRegistry, WorkspaceBinding, WorkspaceGrantId,
    };
    use nexus_persistence::coding_run_ledger::CodingRunLedger;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct Picker(PathBuf);

    impl FolderPicker for Picker {
        fn pick_folder(&self) -> Option<PathBuf> {
            Some(self.0.clone())
        }
    }

    struct Case {
        root: PathBuf,
        authority: Arc<WorkspaceAuthorityRegistry>,
        binding: WorkspaceBinding,
        read: WorkspaceGrantId,
        run: CodingRun,
    }

    impl Drop for Case {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A created run over a native-picked temporary project, with the run
    /// read grant the desktop issues.
    fn case() -> Case {
        let root = std::env::temp_dir().join(format!("nexus-p2e-h1-flow-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("project/src")).unwrap();
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(root.join("project/src/lib.rs"), "pub fn a() {}\n").unwrap();
        let root = root.canonicalize().unwrap();
        let authority = Arc::new(WorkspaceAuthorityRegistry::new());
        let projects = ProjectRegistry::new(Arc::clone(&authority), &root.join("state"));
        let info = projects.select(&Picker(root.join("project"))).unwrap();
        let binding = WorkspaceBinding {
            agent_id: uuid::Uuid::new_v4(),
            run_id: uuid::Uuid::new_v4(),
        };
        let grant = projects.grant_for_run(info.id, binding).unwrap();
        let read = grant.grant_id();
        let ledger: Arc<dyn LedgerStore> =
            Arc::new(CodingRunLedger::open(&root.join("ledger.db")).unwrap());
        let scopes = RunScopes::new(
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::default(),
        )
        .unwrap();
        let run = CodingRun::create_for_project(ledger, grant, scopes).unwrap();
        assert!(authority.resolve(read, binding).is_ok());
        Case {
            root,
            authority,
            binding,
            read,
            run,
        }
    }

    fn revoked(c: &Case) -> bool {
        matches!(
            c.authority.resolve(c.read, c.binding),
            Err(WorkspaceAuthorityError::RevokedGrant)
        )
    }

    #[test]
    fn p2e_h1_rg_d1_a_run_that_fails_before_review_closes_its_read_grant() {
        let mut c = case();
        let (stage, message) = end_failed(&mut c.run, "The model could not be pinned.".into());
        assert_eq!(
            (stage, message.as_str()),
            ("failed", "The model could not be pinned.")
        );
        assert_eq!(c.run.state(), RunState::Cancelled);
        assert!(revoked(&c));
    }

    #[test]
    fn p2e_h1_rg_d2_a_discarded_run_leaves_no_read_grant_live() {
        let mut c = case();
        let (stage, _) = discard_run(&mut c.run);
        assert_eq!(stage, "discarded");
        assert!(revoked(&c));
    }

    #[test]
    fn p2e_h1_rg_d3_a_run_that_cannot_start_is_ended_at_once() {
        // `start_run` ends a run whose worker thread cannot be spawned (its
        // read grant closes), instead of leaving it preparing until expiry.
        let flow = include_str!("coding_flow.rs");
        assert_eq!(flow.matches(".spawn(").count(), 1);
        let spawn = flow.find(".spawn(move || work(").unwrap();
        let rest = &flow[spawn..];
        let failure = &rest[..rest.find("Ok(StartView").unwrap()];
        for needle in [
            ".is_err()",
            "claim(&slot, &[\"preparing\"], \"failed\")?",
            "end_failed(&mut run,",
            "slot.refresh(&run, stage, Some(message));",
            "return Err(",
        ] {
            assert!(failure.contains(needle), "{needle}: {failure}");
        }
    }
}
