//! P0-002C5A reachability guard for closed desktop surfaces.
//!
//! `CLOSED_COMMANDS` is the classified registry of IPC commands that Phase
//! Zero closes. For every entry the guard proves, from the registered handler
//! itself, that the command:
//! - is still registered exactly once (callers receive the bounded reason,
//!   not an unknown-command error);
//! - accepts no caller input (its handler takes no parameters);
//! - only denies (its whole body is the `closed(..)` error for its closure);
//! - returns exactly that bounded reason when invoked.
//!
//! Re-opening a command therefore fails this guard until the command is
//! removed from `CLOSED_COMMANDS` together with an Architect-approved
//! authority mechanism, and the inventory
//! (`docs/security/phase0-c5-authority-inventory.md`) is updated.

use super::{closed, Closure};

/// Every desktop IPC command closed by P0-002C5A, with its closure.
const CLOSED_COMMANDS: &[(&str, Closure)] = &[
    // E1: a raw user-selected path is not authority.
    ("file_manager_list", Closure::FileSelection),
    ("file_manager_read", Closure::FileSelection),
    ("file_manager_write", Closure::FileSelection),
    ("file_manager_create_dir", Closure::FileSelection),
    ("file_manager_delete", Closure::FileSelection),
    ("file_manager_rename", Closure::FileSelection),
    ("analyze_media_file", Closure::FileSelection),
    ("index_document", Closure::FileSelection),
    ("cogfs_index_file", Closure::FileSelection),
    ("cogfs_watch_directory", Closure::FileSelection),
    ("db_connect", Closure::FileSelection),
    ("db_execute_query", Closure::FileSelection),
    ("db_list_tables", Closure::FileSelection),
    ("db_export_table", Closure::FileSelection),
    ("db_disconnect", Closure::FileSelection),
    ("voice_load_whisper_model", Closure::FileSelection),
    ("airgap_create_bundle", Closure::FileSelection),
    ("airgap_validate_bundle", Closure::FileSelection),
    ("airgap_install_bundle", Closure::FileSelection),
    ("backup_verify", Closure::FileSelection),
    ("backup_restore", Closure::FileSelection),
    ("flash_profile_model", Closure::FileSelection),
    ("flash_auto_configure", Closure::FileSelection),
    ("flash_create_session", Closure::FileSelection),
    ("flash_estimate_performance", Closure::FileSelection),
    ("flash_run_benchmark", Closure::FileSelection),
    ("flash_enable_speculative", Closure::FileSelection),
    // E2: the retired legacy Builder raw-path surface.
    ("conduct_build", Closure::LegacyBuilder),
    ("conduct_build_streaming", Closure::LegacyBuilder),
    ("read_build_file", Closure::LegacyBuilder),
    ("builder_list_projects", Closure::LegacyBuilder),
    ("builder_load_project", Closure::LegacyBuilder),
    ("builder_delete_project", Closure::LegacyBuilder),
    ("builder_read_preview", Closure::LegacyBuilder),
    ("builder_list_checkpoints", Closure::LegacyBuilder),
    ("builder_rollback", Closure::LegacyBuilder),
    ("builder_init_checkpoint", Closure::LegacyBuilder),
    ("builder_iterate", Closure::LegacyBuilder),
    ("builder_load_plan", Closure::LegacyBuilder),
    ("builder_archive_project", Closure::LegacyBuilder),
    ("builder_unarchive_project", Closure::LegacyBuilder),
    ("builder_export_project", Closure::LegacyBuilder),
    ("builder_save_state", Closure::LegacyBuilder),
    ("builder_load_state", Closure::LegacyBuilder),
    ("builder_visual_edit_token", Closure::LegacyBuilder),
    ("builder_visual_edit_text", Closure::LegacyBuilder),
    ("builder_deploy", Closure::LegacyBuilder),
    ("builder_deploy_rollback", Closure::LegacyBuilder),
    ("builder_quality_check", Closure::LegacyBuilder),
    ("builder_quality_auto_fix", Closure::LegacyBuilder),
    ("builder_quality_auto_fix_all", Closure::LegacyBuilder),
    ("builder_conversion_check", Closure::LegacyBuilder),
    ("builder_conversion_auto_fix", Closure::LegacyBuilder),
    ("builder_collab_start_hosting", Closure::LegacyBuilder),
    ("builder_collab_leave", Closure::LegacyBuilder),
    ("builder_collab_invite", Closure::LegacyBuilder),
    ("builder_collab_set_role", Closure::LegacyBuilder),
    ("builder_collab_add_comment", Closure::LegacyBuilder),
    ("builder_collab_get_comments", Closure::LegacyBuilder),
    ("builder_collab_resolve_comment", Closure::LegacyBuilder),
    ("builder_import_design", Closure::LegacyBuilder),
    ("builder_generate_variants", Closure::LegacyBuilder),
    ("builder_generate_section_variants", Closure::LegacyBuilder),
    ("builder_theme_apply", Closure::LegacyBuilder),
    ("builder_theme_get_current", Closure::LegacyBuilder),
    ("builder_theme_export", Closure::LegacyBuilder),
    ("builder_generate_image", Closure::LegacyBuilder),
    ("builder_generate_all_images", Closure::LegacyBuilder),
    ("builder_generate_trust_pack", Closure::LegacyBuilder),
    ("builder_get_audit_trail", Closure::LegacyBuilder),
    ("builder_export_audit_trail", Closure::LegacyBuilder),
    ("builder_deploy_history", Closure::LegacyBuilder),
    ("builder_deploy_diff", Closure::LegacyBuilder),
    ("builder_deploy_rollback_to", Closure::LegacyBuilder),
    ("builder_deploy_share_info", Closure::LegacyBuilder),
    ("builder_deploy_drift", Closure::LegacyBuilder),
    // E3: arbitrary program text is not process authority, and a caller's
    // assertion is not user approval.
    ("terminal_execute", Closure::ProcessExecution),
    ("terminal_execute_approved", Closure::ApprovalRequired),
    ("factory_create_project", Closure::FileSelection),
    ("factory_build_project", Closure::ProcessExecution),
    ("factory_test_project", Closure::ProcessExecution),
    ("factory_run_pipeline", Closure::ProcessExecution),
    ("cc_execute_action", Closure::ProcessExecution),
    ("mcp2_client_add", Closure::ProcessExecution),
    ("mcp2_client_discover", Closure::ProcessExecution),
    ("mcp2_client_call", Closure::ProcessExecution),
    ("nx_agent_run", Closure::ProcessExecution),
    ("detect_claude_code_cli", Closure::ExternalCliAgent),
    ("detect_codex_cli", Closure::ExternalCliAgent),
    ("trigger_claude_code_login", Closure::ExternalCliAgent),
    ("trigger_codex_cli_login", Closure::ExternalCliAgent),
    ("builder_check_cli_auth", Closure::ExternalCliAgent),
    ("builder_authenticate_cli", Closure::ExternalCliAgent),
    // E4: agent execution rooted at the process working directory.
    ("nx_chat", Closure::AgentExecution),
    ("nx_tool", Closure::AgentExecution),
    ("run_content_pipeline", Closure::AgentExecution),
];

const LIB_RS: &str = include_str!("../lib.rs");

/// Source of the module that defines a registered handler (`""` is the
/// `runtime` module in lib.rs).
fn module_source(module: &str) -> &'static str {
    match module {
        "" => LIB_RS,
        "commands::flash" => include_str!("../commands/flash.rs"),
        "commands::crate_bridges" => include_str!("../commands/crate_bridges.rs"),
        "commands::orchestration" => include_str!("../commands/orchestration.rs"),
        "nx_bridge::commands" => include_str!("../nx_bridge/commands.rs"),
        other => panic!("closed handler module {other} is not mapped in the guard"),
    }
}

/// Entries of the desktop `generate_handler![..]` list, in order.
fn registered_handlers() -> Vec<String> {
    let start = LIB_RS
        .find("generate_handler![")
        .expect("desktop command registration")
        + "generate_handler![".len();
    let end = start + LIB_RS[start..].find(']').expect("end of registration");
    LIB_RS[start..end]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

/// (module path, command name) of a registered handler entry.
fn split_handler(entry: &str) -> (&str, &str) {
    let path = entry.trim_start_matches("crate::");
    path.rsplit_once("::").unwrap_or(("", path))
}

fn without_whitespace(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Index just past the brace closing the block that opens at `open`.
/// String literals are skipped; closed-handler bodies contain no others.
fn block_end(src: &str, open: usize) -> usize {
    let bytes = src.as_bytes();
    let (mut depth, mut i, mut in_string) = (0usize, open, false);
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_string => i += 1,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("unterminated handler body");
}

/// Parameter list and body (both whitespace-free) of the only `fn <name>` in
/// `src`.
fn handler_shape(src: &str, name: &str) -> (String, String) {
    let needle = format!("fn {name}(");
    let mut found = src.match_indices(&needle).map(|(at, _)| at);
    let at = found
        .next()
        .unwrap_or_else(|| panic!("{name}: handler not found"));
    assert!(
        found.next().is_none(),
        "{name}: handler defined more than once"
    );
    let params_start = at + needle.len() - 1;
    let params_end = params_start + src[params_start..].find(')').unwrap() + 1;
    let open = params_end + src[params_end..].find('{').unwrap();
    let body = &src[open + 1..block_end(src, open) - 1];
    (
        without_whitespace(&src[params_start..params_end]),
        without_whitespace(body).replace(",)", ")"),
    )
}

#[test]
fn closed_commands_stay_registered_take_no_input_and_only_deny() {
    let handlers = registered_handlers();
    for (i, (command, closure)) in CLOSED_COMMANDS.iter().enumerate() {
        assert!(
            !CLOSED_COMMANDS[..i].iter().any(|(seen, _)| seen == command),
            "{command} is listed twice"
        );
        let entries: Vec<_> = handlers
            .iter()
            .filter(|entry| split_handler(entry).1 == *command)
            .collect();
        assert_eq!(entries.len(), 1, "{command} must stay registered once");
        let (module, name) = split_handler(entries[0]);
        let (params, body) = handler_shape(module_source(module), name);
        assert_eq!(params, "()", "{command} must accept no caller input");
        assert_eq!(
            body,
            format!(
                "Err(crate::phase0_surface::closed(\"{command}\",crate::phase0_surface::Closure::{closure:?}))"
            ),
            "{command} must only deny"
        );
    }
}

/// A closed handler's command name and a no-input invocation of it.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
type ClosedHandler = (&'static str, fn() -> Result<(), String>);

/// Invokes every closed handler with no input.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn closed_handlers() -> Vec<ClosedHandler> {
    macro_rules! handlers {
        ($($module:path => [$($name:ident),* $(,)?]),* $(,)?) => {{
            use crate::runtime;
            vec![$($((
                stringify!($name),
                (|| { use $module as m; m::$name().map(|_| ()) }) as fn() -> Result<(), String>,
            )),*),*]
        }};
    }
    handlers!(
        runtime => [
            file_manager_list, file_manager_read, file_manager_write, file_manager_create_dir,
            file_manager_delete, file_manager_rename, analyze_media_file, index_document,
            cogfs_index_file, cogfs_watch_directory, db_connect, db_execute_query,
            db_list_tables, db_export_table, db_disconnect, voice_load_whisper_model,
            airgap_create_bundle,
            airgap_validate_bundle, airgap_install_bundle, backup_verify, backup_restore,
            conduct_build, conduct_build_streaming, read_build_file, builder_list_projects,
            builder_load_project, builder_delete_project, builder_read_preview,
            builder_list_checkpoints, builder_rollback, builder_init_checkpoint,
            builder_iterate, builder_load_plan, builder_archive_project,
            builder_unarchive_project, builder_export_project, builder_save_state,
            builder_load_state, builder_visual_edit_token, builder_visual_edit_text,
            builder_deploy, builder_deploy_rollback, builder_quality_check,
            builder_quality_auto_fix, builder_quality_auto_fix_all, builder_conversion_check,
            builder_conversion_auto_fix, builder_collab_start_hosting, builder_collab_leave,
            builder_collab_invite, builder_collab_set_role, builder_collab_add_comment,
            builder_collab_get_comments, builder_collab_resolve_comment,
            builder_import_design, builder_generate_variants,
            builder_generate_section_variants, builder_theme_apply, builder_theme_get_current,
            builder_theme_export, builder_generate_image, builder_generate_all_images,
            builder_generate_trust_pack, builder_get_audit_trail, builder_export_audit_trail,
            builder_deploy_history, builder_deploy_diff, builder_deploy_rollback_to,
            builder_deploy_share_info, builder_deploy_drift, terminal_execute,
            terminal_execute_approved, factory_create_project, factory_build_project,
            factory_test_project, factory_run_pipeline, detect_claude_code_cli,
            detect_codex_cli, trigger_claude_code_login, trigger_codex_cli_login,
            builder_check_cli_auth, builder_authenticate_cli,
        ],
        crate::commands::flash => [
            flash_profile_model, flash_auto_configure, flash_create_session,
            flash_estimate_performance, flash_run_benchmark, flash_enable_speculative,
        ],
        crate::commands::crate_bridges => [
            cc_execute_action, mcp2_client_add, mcp2_client_discover, mcp2_client_call,
        ],
        crate::nx_bridge::commands => [nx_agent_run, nx_chat, nx_tool],
        crate::commands::orchestration => [run_content_pipeline],
    )
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn closed_handlers_return_only_their_bounded_reason() {
    let handlers = closed_handlers();
    assert_eq!(handlers.len(), CLOSED_COMMANDS.len());
    for (command, closure) in CLOSED_COMMANDS {
        let calls: Vec<_> = handlers
            .iter()
            .filter(|(name, _)| name == command)
            .collect();
        assert_eq!(calls.len(), 1, "{command} must be invoked by the guard");
        assert_eq!((calls[0].1)(), Err(closed(command, *closure)), "{command}");
    }
}

#[test]
fn closure_reasons_are_bounded_and_echo_no_input() {
    for closure in [
        Closure::FileSelection,
        Closure::LegacyBuilder,
        Closure::ProcessExecution,
        Closure::ApprovalRequired,
        Closure::ExternalCliAgent,
        Closure::AgentExecution,
    ] {
        let reason = closure.reason();
        assert!(reason.contains("Phase Zero"), "{reason}");
        assert!(reason.len() <= 160, "{reason}");
        assert!(!reason.contains('/') && !reason.contains('\\'), "{reason}");
        assert_eq!(closed("surface", closure), format!("surface: {reason}"));
    }
}

/// `execute_tool` never runs a process: a tool needing approval is refused as
/// such (a caller cannot approve it), and any other tool for lack of backend
/// authority. Each refusal leaves the filesystem untouched.
#[test]
fn execute_tool_refuses_before_any_process_runs() {
    let state = crate::AppState::new_in_memory();
    let base = std::env::temp_dir().join(format!("nexus-c5a-tool-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let victim = base.join("victim.txt");
    std::fs::write(&victim, "kept").unwrap();
    let created = base.join("created");
    let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
    let cases = [
        // Needs approval: destructive, and custom programs.
        (
            serde_json::json!({"FileRemove": {"path": path(&victim)}}),
            Closure::ApprovalRequired,
        ),
        (
            serde_json::json!({"Custom": {"program": "touch", "args": [path(&created)], "requires_approval": false}}),
            Closure::ApprovalRequired,
        ),
        // Would run a process with no backend authority.
        (
            serde_json::json!({"MakeDirectory": {"path": path(&created)}}),
            Closure::ProcessExecution,
        ),
        (serde_json::json!("GitStatus"), Closure::ProcessExecution),
    ];
    for (tool, closure) in cases {
        assert_eq!(
            crate::execute_tool(&state, tool.to_string()),
            Err(closed("execute_tool", closure)),
            "{tool}"
        );
    }
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "kept");
    assert!(!created.exists());
    std::fs::remove_dir_all(&base).unwrap();
}

/// The production agent executor refuses filesystem, process, OS-input and
/// cwd-rooted actions before they reach an actuator, and has no workspace.
#[test]
fn production_agent_executor_refuses_filesystem_and_process_actions() {
    use nexus_kernel::cognitive::loop_runtime::ActionExecutor;
    use nexus_kernel::cognitive::PlannedAction;

    let state = crate::AppState::new_in_memory();
    let memory = std::sync::Arc::new(nexus_kernel::cognitive::AgentMemoryManager::new(Box::new(
        crate::DbMemoryStore {
            db: state.db.clone(),
        },
    )));
    let executor = crate::phase0_agent_executor(&state, memory);
    let mut audit = state.audit.clone();
    let agent = uuid::Uuid::new_v4().to_string();
    let base = std::env::temp_dir().join(format!("nexus-c5a-agent-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let target = base.join("written.txt");
    let target = target.to_string_lossy().into_owned();
    let refused = [
        serde_json::json!({"type": "FileRead", "path": target}),
        serde_json::json!({"type": "FileWrite", "path": target, "content": "x"}),
        serde_json::json!({"type": "ShellCommand", "command": "touch", "args": [target]}),
        serde_json::json!({"type": "DockerCommand", "subcommand": "run", "args": ["-v", "/:/host", "img"]}),
        serde_json::json!({"type": "CodeExecute", "language": "python", "code": "open('x','w')"}),
        serde_json::json!({"type": "ApiCall", "method": "POST", "url": "https://example.com", "body": "@/etc/passwd"}),
        serde_json::json!({"type": "ImageGenerate", "prompt": "p", "output_path": target}),
        serde_json::json!({"type": "TextToSpeech", "text": "t", "output_path": target}),
        serde_json::json!({"type": "BrowserAutomate", "start_url": "https://example.com", "actions": []}),
        serde_json::json!({"type": "CaptureScreen"}),
        serde_json::json!({"type": "KeyboardType", "text": "t"}),
        // A fetch is web-only: a file URL is a local path under another name.
        serde_json::json!({"type": "WebFetch", "url": format!("file://{target}")}),
        serde_json::json!({"type": "WebFetch", "url": " FILE:///etc/passwd"}),
    ];
    for action in refused {
        let parsed: PlannedAction = match serde_json::from_value(action.clone()) {
            Ok(parsed) => parsed,
            Err(error) => panic!("fixture {action} must parse: {error}"),
        };
        assert_eq!(
            executor.execute(&agent, &parsed, &mut audit, true),
            Err(closed(parsed.action_type(), Closure::AgentExecution)),
            "{action}"
        );
    }
    assert!(!std::path::Path::new(&target).exists());
    std::fs::remove_dir_all(&base).unwrap();
    // Actions holding no filesystem or process authority still run.
    assert_eq!(
        executor.execute(&agent, &PlannedAction::Noop, &mut audit, false),
        Ok("ok".to_string())
    );
}

fn production_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                production_sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with("tests.rs"))
        {
            out.push(path);
        }
    }
}

/// No desktop production source starts an external CLI agent or bypasses its
/// permission checks, and the desktop swarm registers no external CLI agent
/// provider. (The providers themselves fail closed in `nexus-connectors-llm`.)
#[test]
fn desktop_sources_start_no_external_cli_agent() {
    let mut files = Vec::new();
    production_sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 20, "desktop sources not found");
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for forbidden in [
            concat!("dangerously", "-skip-permissions"),
            concat!("Command::new(\"", "claude\")"),
            concat!("Command::new(\"", "codex\")"),
        ] {
            assert!(!text.contains(forbidden), "{}: {forbidden}", path.display());
        }
    }
    let swarm = include_str!("../commands/swarm.rs");
    assert!(!swarm.contains(concat!("CodexCli", "Provider")));
}
