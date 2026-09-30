//! Phase One desktop guards. `p1_tm_*`: the Time Machine "what if" residual
//! (charter §12) stays closed: it takes no caller input, only denies, and
//! can no longer change an agent's fuel, force an agent's state or toggle
//! Warden review.

use super::*;
use crate::phase0_surface::{closed, Closure};

const WHAT_IF: &str = "time_machine_what_if";

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

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p1_tm_01_what_if_takes_no_input_and_only_denies() {
    let reason = Closure::SimulationReplay.reason();
    assert!(reason.len() <= 160 && !reason.contains('/'), "{reason}");
    assert_eq!(
        crate::runtime::time_machine_what_if(),
        Err(closed(WHAT_IF, Closure::SimulationReplay))
    );
}

#[test]
fn p1_tm_02_what_if_changes_no_fuel_state_or_warden_review() {
    let state = AppState::new_in_memory();
    let manifest = serde_json::json!({
        "name": "what-if-probe",
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
    let snapshot = |state: &AppState| {
        let supervisor = state.supervisor.lock().unwrap();
        let handle = supervisor.get_agent(id).unwrap();
        (handle.remaining_fuel, handle.state)
    };
    let before = snapshot(&state);
    // Every former what-if request is now only a string the handler cannot
    // even receive: the registered handler takes no arguments.
    #[cfg(all(
        feature = "tauri-runtime",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    for _request in [
        (format!("agent://{id}/fuel_remaining"), "999999999"),
        (format!("agent://{id}/status"), "Running"),
        ("governance.enable_warden_review".to_string(), "false"),
    ] {
        assert!(crate::runtime::time_machine_what_if().is_err());
    }
    assert_eq!(snapshot(&state), before, "fuel and state unchanged");
}

#[test]
fn p1_tm_03_no_production_code_performs_what_if_mutation() {
    for (path, text) in production_sources() {
        for needle in [
            "time-machine-what-if",
            "time_machine.what_if",
            "variable_key",
            "super::time_machine_what_if",
        ] {
            assert!(
                !text.contains(needle),
                "{} still contains {needle}",
                path.display()
            );
        }
    }
    let model_hub = include_str!("commands/model_hub.rs");
    assert!(!model_hub.contains("fn time_machine_what_if"));
    // Warden review is written only through the reviewed configuration
    // paths, never from a what-if request.
    assert!(!model_hub.contains("enable_warden_review"));
}

// ── P1-08 / §14: the governed coding surface ────────────────────────────────

const CODING_COMMANDS: [&str; 12] = [
    "coding_select_project",
    "coding_list_projects",
    "coding_list_local_models",
    "coding_start_run",
    "coding_status",
    "coding_list_runs",
    "coding_approve_apply",
    "coding_restore_run",
    "coding_discard_run",
    // Phase Two: governed sandboxed verification (run id and profile name).
    "coding_verification_profiles",
    "coding_start_verification",
    "coding_retry_verification_cleanup",
];

/// The parameter list of `fn name(` in `src`.
fn params_of(src: &str, name: &str) -> String {
    let needle = format!("fn {name}(");
    let at = src
        .find(&needle)
        .unwrap_or_else(|| panic!("{name} not found"));
    assert_eq!(src.matches(&needle).count(), 1, "{name} defined once");
    let start = at + needle.len();
    let mut depth = 1;
    let mut end = start;
    for (i, c) in src[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    src[start..end].to_string()
}

#[test]
fn p1_g_01_coding_commands_take_only_opaque_ids_and_choices() {
    let lib = include_str!("lib.rs");
    let registered = include_str!("webview_boundary/app_commands.rs");
    for command in CODING_COMMANDS {
        assert!(registered.contains(&format!("\"{command}\"")), "{command}");
        let params = params_of(lib, command);
        let mut parts = Vec::new();
        let (mut depth, mut current) = (0, String::new());
        for c in params.chars() {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(std::mem::take(&mut current));
                    continue;
                }
                _ => {}
            }
            current.push(c);
        }
        parts.push(current);
        for param in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let (name, ty) = param.split_once(':').expect("typed parameter");
            let (name, ty) = (name.trim(), ty.trim());
            assert!(
                [
                    "app",
                    "state",
                    "project_id",
                    "write_scope",
                    "protected_scope",
                    "task",
                    "model",
                    "run_id",
                    "profile"
                ]
                .contains(&name),
                "{command} takes an unexpected input {name}"
            );
            for banned in [
                "path", "Path", "approv", "confirm", "bool", "grant", "Grant",
            ] {
                assert!(
                    !param.contains(banned),
                    "{command} takes {param}: a path, grant or approval is never an input"
                );
            }
            assert!(
                ty.starts_with("tauri::") || ty == "String" || ty == "Vec<String>",
                "{command}: {name} has type {ty}"
            );
        }
    }
}

#[test]
fn p1_g_02_the_coding_path_has_no_process_git_shell_or_cloud_side_door() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let kernel = root.join("../../kernel/src/coding_run");
    let mut files = vec![(
        "coding_flow.rs".to_string(),
        include_str!("coding_flow.rs").to_string(),
    )];
    for entry in std::fs::read_dir(&kernel).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with(".rs") && !name.ends_with("tests.rs") {
            files.push((name, std::fs::read_to_string(&path).unwrap()));
        }
    }
    files.push((
        "coding_run.rs".to_string(),
        std::fs::read_to_string(root.join("../../kernel/src/coding_run.rs")).unwrap(),
    ));
    assert!(files.len() >= 11, "coding sources not found");
    for (name, text) in &files {
        for needle in [
            "std::process",
            "Command::new",
            "tokio::process",
            "git2",
            "gix::",
            "\"git\"",
            "nexus_code",
            "software_factory",
            "terminal_execute",
            "coder_agent",
            "curl",
            "TcpStream",
            "UdpSocket",
            "time_machine",
            "npm ",
            "pip ",
            "cargo ",
            "api.openai.com",
            "api.anthropic.com",
            "generativelanguage",
            "openrouter",
            "groq",
            "huggingface",
            "LlmProvider",
            "GovernedLlmGateway",
            "select_provider",
        ] {
            assert!(
                !text.to_lowercase().contains(&needle.to_lowercase()),
                "{name} contains {needle}"
            );
        }
    }
    // HTTP appears only in the loopback-only local model client.
    for (name, text) in &files {
        if text.contains("reqwest") {
            assert_eq!(name, "local_model.rs", "{name} uses an HTTP client");
        }
    }
}

#[test]
fn p1_g_03_the_webview_holds_no_dialog_or_file_system_permission() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in [
        "capabilities/default.json",
        "capabilities/app-commands.json",
    ] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        for needle in ["dialog", "fs:", "\"fs"] {
            assert!(!text.contains(needle), "{file} grants {needle}");
        }
    }
    let conf = std::fs::read_to_string(root.join("tauri.conf.json")).unwrap();
    assert!(
        !conf.contains("\"dialog\""),
        "no dialog plugin configuration"
    );
    let lib = include_str!("lib.rs");
    assert_eq!(lib.matches("tauri_plugin_dialog::init()").count(), 1);
    assert!(
        !lib.contains("tauri_plugin_fs::init"),
        "the fs plugin is never registered"
    );
}

#[test]
fn p1_g_04_native_dialogs_are_the_only_picker_and_confirmer() {
    let mut pickers = 0;
    let mut confirmers = 0;
    for (path, text) in production_sources() {
        let p = text.matches("impl FolderPicker for").count();
        let c = text.matches("impl OwnerConfirmer for").count();
        if p + c > 0 {
            assert!(path.ends_with("coding_flow.rs"), "{}", path.display());
            assert!(text.contains("impl FolderPicker for NativeDialogs"));
            assert!(text.contains("impl OwnerConfirmer for NativeDialogs"));
        }
        pickers += p;
        confirmers += c;
    }
    assert_eq!((pickers, confirmers), (1, 1));
}

#[test]
fn p1_g_05_only_the_governed_commands_reach_the_coding_flow() {
    for (path, text) in production_sources() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if text.contains("coding_flow") {
            assert!(
                name == "lib.rs" || name == "coding_flow.rs",
                "{} reaches the coding flow",
                path.display()
            );
        }
    }
    let lib = include_str!("lib.rs");
    assert_eq!(
        lib.matches("crate::coding_flow::ipc::").count(),
        CODING_COMMANDS.len(),
        "each governed command delegates exactly once"
    );
    for command in CODING_COMMANDS {
        let at = lib.find(&format!("fn {command}(")).unwrap();
        let body = &lib[at..at + lib[at..].find("\n    }\n").unwrap()];
        assert!(body.contains("crate::coding_flow::ipc::"), "{command}");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn p1_g_06_native_dialog_text_prints_percent_signs_literally() {
    use crate::coding_flow::native_dialog_text;
    assert_eq!(native_dialog_text("src/100%s%n.rs"), "src/100%%s%%n.rs");
    assert_eq!(native_dialog_text("50%% done"), "50%%%% done");
    assert_eq!(native_dialog_text("no directives"), "no directives");
    // Every `%` is doubled, so GTK's printf sees no conversion directive.
    let escaped = native_dialog_text("%d%x%s%p%n%%");
    assert!(escaped.split("%%").all(|piece| !piece.contains('%')));
    let source = include_str!("coding_flow.rs");
    assert!(source.contains(".message(native_dialog_text(&request.message()))"));
}

#[test]
fn p1_g_07_owner_actions_claim_the_run_and_never_block_the_main_thread() {
    let flow = include_str!("coding_flow.rs");
    // The run lock is taken only by `claim` (owner actions) and the worker
    // thread; owner actions change the stage under the display lock first.
    assert_eq!(flow.matches("slot.run.lock()").count(), 2);
    for action in [
        "\"applying\")?",
        "\"restoring\")?",
        "\"discarding\",\n            )?",
    ] {
        assert!(flow.contains(action), "{action} must go through claim");
    }
    // Commands that may wait on a run or a native dialog are async and run
    // their work on the blocking pool, never on the main thread.
    let lib = include_str!("lib.rs");
    for command in [
        "coding_select_project",
        "coding_list_local_models",
        "coding_approve_apply",
        "coding_restore_run",
        "coding_discard_run",
    ] {
        assert!(lib.contains(&format!("async fn {command}(")), "{command}");
    }
}

// ── Final closure: desktop outcomes never claim more than happened ──────────

#[cfg(target_os = "linux")]
mod outcomes {
    use crate::coding_flow::{discard_outcome, discard_run, end_failed, failure_outcome};
    use nexus_kernel::coding_run::{
        CleanupStatus, CodingRun, FolderPicker, LedgerFailure, LedgerStore, ProjectRegistry,
        RecoveryReason, RunError, RunScopes, RunState, ScopeEntry, ScopeSet,
    };
    use nexus_kernel::workspace_authority::{
        WorkspaceAuthorityRegistry, WorkspaceBinding, WorkspaceGrantId,
    };
    use nexus_persistence::coding_run_ledger::{CodingRunLedger, LedgerRecord, NewLedgerEvent};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Fails the append with index `fail_at` (0 is run.created).
    struct FailAt {
        inner: CodingRunLedger,
        fail_at: usize,
        calls: AtomicUsize,
    }

    impl LedgerStore for FailAt {
        fn append(&self, event: NewLedgerEvent<'_>) -> Result<LedgerRecord, LedgerFailure> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == self.fail_at {
                return Err(LedgerFailure::Unavailable);
            }
            LedgerStore::append(&self.inner, event)
        }
        fn verified_records(&self, run: uuid::Uuid) -> Result<Vec<LedgerRecord>, LedgerFailure> {
            self.inner.verified_records(run)
        }
    }

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
        project_grant: WorkspaceGrantId,
        run: CodingRun,
    }

    impl Drop for Case {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A created run over a native-picked temporary project; the ledger
    /// fails the append with index `fail_at`.
    fn case(fail_at: usize) -> Case {
        let root = std::env::temp_dir().join(format!("nexus-p1-outcome-{}", uuid::Uuid::new_v4()));
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
        let project_grant = grant.grant_id();
        let ledger = Arc::new(FailAt {
            inner: CodingRunLedger::open(&root.join("ledger.db")).unwrap(),
            fail_at,
            calls: AtomicUsize::new(0),
        });
        let scopes = RunScopes::new(
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::default(),
        )
        .unwrap();
        let run = CodingRun::create_for_project(ledger, grant, scopes).unwrap();
        Case {
            root,
            authority,
            binding,
            project_grant,
            run,
        }
    }

    #[test]
    fn p1_g_08_a_discard_whose_cancellation_fails_is_not_discarded() {
        // 0 run.created, 1 run.cancelled (fails).
        let mut c = case(1);
        let (stage, message) = discard_run(&mut c.run);
        assert_eq!(stage, "recovery_required");
        assert!(!message.contains("was discarded"), "{message}");
        assert_eq!(
            c.run.state(),
            RunState::RecoveryRequired(RecoveryReason::TerminalNotRecorded)
        );
        // P2-ENTRY-H1: authority closure never depends on the ledger, so the
        // run's own read grant is revoked even though the record failed.
        assert!(
            c.authority.resolve(c.project_grant, c.binding).is_err(),
            "the run's read grant is revoked when the run ends"
        );
    }

    #[test]
    fn p1_g_09_a_failed_staging_cleanup_is_not_discarded() {
        for cleanup in [Ok(CleanupStatus::DiscardFailed), Err(RunError::StagingIo)] {
            let (stage, message) = discard_outcome(Ok(()), cleanup, RunState::Cancelled);
            assert_eq!(stage, "recovery_required");
            assert!(!message.contains("was discarded"), "{message}");
            assert!(message.contains("could not be removed"), "{message}");
        }
        // A run left requiring recovery is not "discarded" either.
        let (stage, _) = discard_outcome(
            Ok(()),
            Ok(CleanupStatus::Discarded),
            RunState::RecoveryRequired(RecoveryReason::StagingRevocationFailed),
        );
        assert_eq!(stage, "recovery_required");
    }

    #[test]
    fn p1_g_10_a_clean_discard_is_discarded() {
        let mut c = case(usize::MAX);
        let (stage, message) = discard_run(&mut c.run);
        assert_eq!(
            (stage, message.as_str()),
            ("discarded", "The run was discarded.")
        );
        assert_eq!(c.run.state(), RunState::Cancelled);
        // P2-ENTRY-H1: a discarded run leaves no read grant live.
        assert!(c.authority.resolve(c.project_grant, c.binding).is_err());
        assert_eq!(
            discard_outcome(Ok(()), Ok(CleanupStatus::Discarded), RunState::Cancelled).0,
            "discarded"
        );
    }

    #[test]
    fn p1_g_11_a_setup_failure_whose_cancel_fails_is_recovery_required() {
        let mut c = case(1);
        let (stage, message) = end_failed(&mut c.run, "The project could not be staged.".into());
        assert_eq!(stage, "recovery_required");
        assert!(message.starts_with("The project could not be staged."));
        assert!(matches!(c.run.state(), RunState::RecoveryRequired(_)));

        let mut c = case(usize::MAX);
        let (stage, message) = end_failed(&mut c.run, "The model could not be pinned.".into());
        assert_eq!(
            (stage, message.as_str()),
            ("failed", "The model could not be pinned.")
        );
        assert_eq!(c.run.state(), RunState::Cancelled);

        // A run already terminal in recovery is shown as such.
        let (stage, _) = failure_outcome(
            Ok(()),
            RunState::RecoveryRequired(RecoveryReason::OutcomeNotRecorded),
            "Worker failed.".into(),
        );
        assert_eq!(stage, "recovery_required");
    }

    #[test]
    fn p1_g_12_the_coding_flow_ignores_no_run_or_grant_result() {
        let flow = include_str!("coding_flow.rs");
        for ignored in ["let _ = run.", "let _ = grant.", ".revoke("] {
            assert!(!flow.contains(ignored), "coding_flow.rs contains {ignored}");
        }
        assert_eq!(
            flow.matches("\"discarded\"").count(),
            2,
            "one outcome and one busy check"
        );
    }
}

// ── Post-completion hygiene: generated application ACL ──────────────────────

/// Post-completion hygiene: the committed generated app ACL agrees exactly
/// with the live `APP_COMMANDS`, in both directions.
///
/// tauri-build compiles every per-command file in the ignored
/// `permissions/autogenerated/` directory into `gen/schemas/`, so a renamed or
/// removed command can leave its old permission in the committed schemas
/// (as `coding_run_status` once did after its rename to `coding_status`).
/// Such a stale definition grants nothing by itself (only capabilities grant),
/// but the committed artifacts must describe exactly the live command set.
///
/// For every live command, `allow-<slug>` must allow exactly that command and
/// `deny-<slug>` must deny exactly it. No other application permission,
/// permission set or default may exist, so any permission for a command
/// absent from `APP_COMMANDS` fails. The desktop and Linux schemas must
/// enumerate exactly the same application permission identifiers.
#[test]
fn p1_g_webview_generated_app_acl_matches_the_live_command_manifest() {
    use std::collections::BTreeSet;
    let schemas = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gen/schemas");
    let read = |name: &str| {
        std::fs::read_to_string(schemas.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let slug = |command: &str| command.replace('_', "-");
    use crate::webview_boundary::APP_COMMANDS;
    let live: BTreeSet<&str> = APP_COMMANDS.iter().copied().collect();
    assert_eq!(
        live.len(),
        APP_COMMANDS.len(),
        "APP_COMMANDS has duplicates"
    );

    let manifests: serde_json::Value =
        serde_json::from_str(&read("acl-manifests.json")).expect("acl-manifests.json parses");
    let app = &manifests["__app-acl__"];
    assert!(app.is_object(), "the generated app ACL is present");
    assert!(app["default_permission"].is_null(), "no generated default");
    assert_eq!(
        app["permission_sets"],
        serde_json::json!({}),
        "no generated permission sets"
    );
    let permissions = app["permissions"]
        .as_object()
        .expect("generated app permissions");

    // Every generated application permission names only live commands.
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for (id, definition) in permissions {
        assert_eq!(definition["identifier"], id.as_str(), "{id}: identifier");
        for side in ["allow", "deny"] {
            for command in definition["commands"][side]
                .as_array()
                .unwrap_or_else(|| panic!("{id}: commands.{side}"))
            {
                let command = command.as_str().expect("command name");
                assert!(
                    live.contains(command),
                    "{id} is a stale generated permission for `{command}`, which is not in APP_COMMANDS"
                );
                referenced.insert(command.to_owned());
            }
        }
    }
    let referenced: BTreeSet<&str> = referenced.iter().map(String::as_str).collect();
    assert_eq!(
        referenced, live,
        "every live command has generated permissions"
    );

    // Every live command has exactly its own allow and deny permission.
    let mut expected: BTreeSet<String> = BTreeSet::new();
    for command in APP_COMMANDS {
        let (allow, deny) = (
            format!("allow-{}", slug(command)),
            format!("deny-{}", slug(command)),
        );
        assert_eq!(
            permissions.get(&allow).map(|d| &d["commands"]),
            Some(&serde_json::json!({ "allow": [command], "deny": [] })),
            "{allow} must allow exactly `{command}`"
        );
        assert_eq!(
            permissions.get(&deny).map(|d| &d["commands"]),
            Some(&serde_json::json!({ "allow": [], "deny": [command] })),
            "{deny} must deny exactly `{command}`"
        );
        expected.insert(allow);
        expected.insert(deny);
    }
    let generated: BTreeSet<String> = permissions.keys().cloned().collect();
    assert_eq!(
        generated, expected,
        "the generated app ACL holds exactly the live commands' permissions"
    );

    // The platform schemas enumerate the same application permissions
    // (plugin permissions carry a `plugin:` prefix and are not app ones).
    for name in ["desktop-schema.json", "linux-schema.json"] {
        let text = read(name);
        let app_ids: BTreeSet<String> = text
            .match_indices("\"const\": \"")
            .map(|(at, needle)| {
                let rest = &text[at + needle.len()..];
                rest[..rest.find('"').expect("closing quote")].to_owned()
            })
            .filter(|id| !id.contains(':'))
            .collect();
        assert_eq!(
            app_ids, expected,
            "{name} enumerates exactly the live application permissions"
        );
    }

    // The instance that prompted this guard.
    assert!(!generated.contains("allow-coding-run-status"));
    assert!(generated.contains("allow-coding-status"));
}
