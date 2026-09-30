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
//!
//! P0-002C5B adds workspace-wide regression guards (at the end of this file)
//! for its families: governed curl invocations, counted ambient state roots,
//! counted identifier joins, and serialized records that choose no authority.

use super::{closed, Closure};

/// Every desktop IPC command closed by P0-002C5A, with its closure.
const CLOSED_COMMANDS: &[(&str, Closure)] = &[
    // Phase One charter §12: caller strings set fuel, agent state or Warden
    // review.
    ("time_machine_what_if", Closure::SimulationReplay),
    // P2-ENTRY-H1: replaying a recorded checkpoint restored agent fuel,
    // memories, state (forced across illegal transitions) or Warden review.
    ("time_machine_undo", Closure::CheckpointReplay),
    ("time_machine_redo", Closure::CheckpointReplay),
    ("time_machine_undo_checkpoint", Closure::CheckpointReplay),
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
    // E5: code- or authority-bearing resources found through the process
    // working directory, the environment or a developer checkout path.
    ("self_rewrite_analyze", Closure::AmbientResource),
    ("self_rewrite_test_patch", Closure::AmbientResource),
    ("self_rewrite_rollback", Closure::AmbientResource),
    ("genesis_analyze_gap", Closure::AmbientResource),
    ("genesis_preview_agent", Closure::AmbientResource),
    ("genesis_create_agent", Closure::AmbientResource),
    ("genesis_store_pattern", Closure::AmbientResource),
    ("genesis_list_generated", Closure::AmbientResource),
    ("genesis_delete_agent", Closure::AmbientResource),
    ("get_agent_genome", Closure::AmbientResource),
    ("mutate_agent", Closure::AmbientResource),
    ("breed_agents", Closure::AmbientResource),
    ("get_agent_lineage", Closure::AmbientResource),
    ("generate_all_genomes", Closure::AmbientResource),
    ("evolve_population", Closure::AmbientResource),
    ("force_evolve_agent", Closure::AmbientResource),
    ("trigger_immune_scan", Closure::AmbientResource),
    ("get_git_repo_status", Closure::AmbientResource),
    ("voice_start_listening", Closure::AmbientResource),
    ("voice_pipeline_health", Closure::AmbientResource),
    ("transcribe_push_to_talk", Closure::AmbientResource),
    ("cm_execute_validation_run", Closure::AmbientResource),
    ("cm_list_validation_runs", Closure::AmbientResource),
    ("cm_get_validation_run", Closure::AmbientResource),
    ("cm_three_way_comparison", Closure::AmbientResource),
    // P0-FINAL-GATE C5: the A/B route read an environment provider key
    // through a fallback chain for a fixed Groq endpoint.
    ("cm_run_ab_validation", Closure::AmbientResource),
    ("memory_save", Closure::AmbientResource),
    ("memory_load", Closure::AmbientResource),
    ("memory_list_agents", Closure::AmbientResource),
    ("mcp2_server_handle", Closure::AmbientResource),
    // C5C: OS keyboard and mouse input from the interface or a model.
    ("computer_control_execute_action", Closure::OsInput),
    ("start_computer_action", Closure::OsInput),
    // C5C: a raw output path for the browser bridge (never started) to write.
    ("browser_screenshot", Closure::FileSelection),
    // C5C (Architect decision): screen capture and capture plus analysis
    // requested over desktop IPC. These are denied unconditionally. The
    // enabling branch of `computer_control_toggle` is refused as well, but
    // its disabling branch stays open, so that command is not listed here
    // (see `p0_002c5c_no_desktop_route_observes_the_screen`).
    (
        "computer_control_capture_screen",
        Closure::ScreenObservation,
    ),
    ("capture_screen", Closure::ScreenObservation),
    ("analyze_screen", Closure::ScreenObservation),
    ("nx_computer_use_screenshot", Closure::ScreenObservation),
    // P0-FINAL-GATE item G: a caller's boolean, or the call itself, is not
    // human approval.
    ("nx_consent_respond", Closure::ApprovalRequired),
    ("nx_agent_approve", Closure::ApprovalRequired),
    ("self_rewrite_apply_patch", Closure::ApprovalRequired),
    // P0-FINAL-GATE items B and F: a caller-chosen destination or peer is not
    // egress authority.
    ("api_client_request", Closure::NetworkDestination),
    ("a2a_discover_agent", Closure::NetworkDestination),
    ("a2a_send_task", Closure::NetworkDestination),
    ("a2a_get_task_status", Closure::NetworkDestination),
    ("a2a_cancel_task", Closure::NetworkDestination),
    ("a2a_crate_send_task", Closure::NetworkDestination),
    ("a2a_crate_get_task", Closure::NetworkDestination),
    ("a2a_crate_discover_agent", Closure::NetworkDestination),
    ("mcp_host_connect", Closure::NetworkDestination),
    ("mcp_host_call_tool", Closure::NetworkDestination),
    (
        "builder_theme_extract_from_url",
        Closure::NetworkDestination,
    ),
    ("nexus_link_send_model", Closure::PeerTransfer),
    // P0-FINAL-GATE item C: a credential would be placed on a process command
    // line.
    ("perception_init", Closure::CredentialTransport),
    // P0-FINAL-GATE item I: no helper program is started or run to report on
    // it.
    ("is_ollama_installed", Closure::HelperLaunch),
    // P0-FINAL-GATE items A and H: the surface's only effect was to store a
    // credential or token outside an approved secret store.
    ("builder_deploy_store_credentials", Closure::SecretStorage),
    ("builder_backend_connect", Closure::SecretStorage),
    ("email_start_oauth", Closure::SecretStorage),
    ("integration_start_oauth", Closure::SecretStorage),
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
    let mut handlers = handlers!(
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
            builder_check_cli_auth, builder_authenticate_cli, self_rewrite_analyze,
            self_rewrite_test_patch, self_rewrite_rollback, genesis_analyze_gap,
            genesis_preview_agent, genesis_create_agent, genesis_store_pattern,
            genesis_list_generated, genesis_delete_agent, get_agent_genome, mutate_agent,
            breed_agents, get_agent_lineage, generate_all_genomes, evolve_population,
            force_evolve_agent, trigger_immune_scan, get_git_repo_status,
            voice_start_listening, voice_pipeline_health, transcribe_push_to_talk,
            computer_control_execute_action, start_computer_action,
            computer_control_capture_screen, capture_screen, analyze_screen,
            api_client_request, a2a_discover_agent, a2a_send_task, a2a_get_task_status,
            a2a_cancel_task, mcp_host_connect, mcp_host_call_tool,
            builder_theme_extract_from_url, nexus_link_send_model, is_ollama_installed,
            builder_deploy_store_credentials, builder_backend_connect, email_start_oauth,
            integration_start_oauth, self_rewrite_apply_patch, time_machine_what_if,
            time_machine_undo, time_machine_redo, time_machine_undo_checkpoint,
        ],
        crate::commands::flash => [
            flash_profile_model, flash_auto_configure, flash_create_session,
            flash_estimate_performance, flash_run_benchmark, flash_enable_speculative,
        ],
        crate::commands::crate_bridges => [
            cc_execute_action, mcp2_client_add, mcp2_client_discover, mcp2_client_call,
            cm_execute_validation_run, cm_list_validation_runs, cm_get_validation_run,
            cm_three_way_comparison, memory_save, memory_load, memory_list_agents,
            mcp2_server_handle, browser_screenshot, a2a_crate_send_task, a2a_crate_get_task,
            a2a_crate_discover_agent, perception_init, cm_run_ab_validation,
        ],
        crate::nx_bridge::commands => [
            nx_agent_run, nx_chat, nx_tool, nx_consent_respond, nx_agent_approve,
        ],
        crate::commands::orchestration => [run_content_pipeline],
    );
    // The nx screenshot handler is async: the real handler's future is run
    // to completion.
    handlers.push((
        "nx_computer_use_screenshot",
        (|| {
            tauri::async_runtime::block_on(crate::nx_bridge::commands::nx_computer_use_screenshot())
                .map(|_| ())
        }) as fn() -> Result<(), String>,
    ));
    handlers
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
        Closure::AmbientResource,
        Closure::OsInput,
        Closure::ScreenObservation,
        Closure::NetworkDestination,
        Closure::PeerTransfer,
        Closure::CredentialTransport,
        Closure::HelperLaunch,
        Closure::SecretStorage,
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

/// P0-002C5C: agent-to-agent delegation has no path around the production
/// executor. The cognitive loop hands every planned action, A2A delegation
/// included, to `Phase0AgentExecutor`. For an agent holding the delegation,
/// filesystem and process capabilities (`a2a.delegate` is outside the
/// capability registry, so only a stored record could carry it), the
/// executor refuses a delegation toward a local file (an A2A filesystem
/// action), a delegation toward a peer (an A2A process action: the transport
/// runs a client process), a file write and a shell command, and nothing is
/// sent, read or created. A permitted action runs under the same policy.
#[test]
fn p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor() {
    use nexus_kernel::cognitive::{
        AgentGoal, AgentMemoryManager, CognitivePlanner, PlannedAction, PlannerLlm,
    };

    struct FixedPlan(String);
    impl PlannerLlm for FixedPlan {
        fn plan_query(&self, _: &str) -> Result<String, nexus_kernel::errors::AgentError> {
            Ok(self.0.clone())
        }
    }

    let state = crate::AppState::new_in_memory();
    let mut manifest = nexus_kernel::manifest::parse_manifest(
        r#"
name = "c5c-delegator"
version = "1.0.0"
capabilities = ["llm.query", "fs.read", "fs.write", "process.exec"]
fuel_budget = 10000
autonomy_level = 5
"#,
    )
    .unwrap();
    manifest.capabilities.push("a2a.delegate".into());
    let agent = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    let memory = std::sync::Arc::new(AgentMemoryManager::new(Box::new(crate::DbMemoryStore {
        db: state.db.clone(),
    })));
    let executor = crate::phase0_agent_executor(&state, memory.clone());
    let run = |action: &serde_json::Value| {
        let plan = serde_json::json!([{"action": action, "description": "step"}]);
        state
            .cognitive_runtime
            .assign_goal(&agent, AgentGoal::new("C5C dispatch".into(), 5))
            .unwrap();
        crate::run_cognitive_cycle(
            &state,
            &agent,
            &CognitivePlanner::new(Box::new(FixedPlan(plan.to_string()))),
            &memory,
            &executor,
        )
        .unwrap()
    };

    let peer = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    peer.set_nonblocking(true).unwrap();
    let peer_url = format!("http://{}/", peer.local_addr().unwrap());
    let base = std::env::temp_dir().join(format!("nexus-c5c-a2a-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    let kept = base.join("kept.txt");
    std::fs::write(&kept, "kept").unwrap();
    let created = base.join("created.txt");
    let (kept_path, created_path) = (
        kept.to_string_lossy().into_owned(),
        created.to_string_lossy().into_owned(),
    );
    let refused = [
        serde_json::json!({"type": "A2aDelegation", "agent_url": format!("file://{kept_path}"), "message": "summarize the notes"}),
        serde_json::json!({"type": "A2aDelegation", "agent_url": peer_url, "message": "summarize the notes"}),
        serde_json::json!({"type": "FileWrite", "path": created_path, "content": "x"}),
        serde_json::json!({"type": "ShellCommand", "command": "touch", "args": [created_path]}),
    ];
    for action in &refused {
        let parsed: PlannedAction = serde_json::from_value(action.clone()).unwrap();
        let result = run(action);
        assert_eq!(result.steps_executed, 0, "{action}");
        assert_eq!(
            result.blocked_reason,
            Some(closed(parsed.action_type(), Closure::AgentExecution)),
            "{action}"
        );
    }
    assert!(matches!(
        peer.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    ));
    assert!(!created.exists());
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "kept");
    std::fs::remove_dir_all(&base).unwrap();

    let permitted = run(&serde_json::json!(
        {"type": "MemoryStore", "key": "c5c", "value": "same policy", "memory_type": "episodic"}
    ));
    assert_eq!(permitted.steps_executed, 1, "{permitted:?}");
    assert_eq!(permitted.blocked_reason, None);
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

/// Nexus Code entry points that run, prefer or register the external Claude
/// CLI agent. The desktop nx bridge must use their `_without_cli_agents` forms.
const NEXUS_CODE_CLI_AGENT_ENTRY_POINTS: &[&str] = &[
    "diagnose()",
    "NxConfig::load()",
    "App::new(",
    "check_claude_cli_available(",
    "ClaudeCliProvider",
    // P0-002C5C: these no-CLI forms still read the working directory's
    // NEXUSCODE.md and .nxrc; the desktop uses the `_for_desktop` forms.
    "load_without_cli_agents(",
    "diagnose_without_cli_agents(",
    "new_without_cli_agents(",
];

/// Whether `text` names `path` as a whole identifier path, not as the tail of
/// a longer identifier (`App::new(` does not match `TuiApp::new(`).
fn names(text: &str, path: &str) -> bool {
    text.match_indices(path).any(|(at, _)| {
        !text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// No desktop production source starts an external CLI agent or bypasses its
/// permission checks, and the desktop swarm registers no external CLI agent
/// provider. (The providers themselves fail closed in `nexus-connectors-llm`.)
/// The nx bridge configures, diagnoses and builds Nexus Code without running,
/// preferring or registering the Claude CLI.
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
        for entry_point in NEXUS_CODE_CLI_AGENT_ENTRY_POINTS {
            assert!(
                !names(&text, entry_point),
                "{}: {entry_point} runs, prefers or registers an external CLI agent",
                path.display()
            );
        }
    }
    let swarm = include_str!("../commands/swarm.rs");
    assert!(!swarm.contains(concat!("CodexCli", "Provider")));
    let bridge = include_str!("../nx_bridge/mod.rs");
    for required in [
        "NxConfig::load_for_desktop(",
        "setup::diagnose_for_desktop()",
        "App::new_for_desktop(",
        "nexus_state_path(\"nexus-code/config.toml\")",
        "nexus_state_path(\"nexus-code/memory.json\")",
    ] {
        assert!(bridge.contains(required), "nx bridge: {required}");
    }
}

/// P0-002C5C: the desktop's Nexus Code entry points read nothing from the
/// process working directory, a project or git ancestor, or the platform
/// configuration and data directories. (The standalone `nx` terminal keeps its
/// project-local files through the other entry points.)
#[test]
fn desktop_nexus_code_takes_no_configuration_from_the_working_directory() {
    let config = production_text(include_str!("../../../../nexus-code/src/config.rs"));
    let setup = production_text(include_str!("../../../../nexus-code/src/setup.rs"));
    let app = production_text(include_str!("../../../../nexus-code/src/app.rs"));
    let body = |src: &str, name: &str| {
        let at = src.find(name).unwrap_or_else(|| panic!("{name}"));
        let open = at + src[at..].find('{').unwrap();
        without_whitespace(&src[open + 1..block_end(src, open) - 1])
    };
    let load = body(&config, "pub fn load_for_desktop(");
    let new = body(&app, "pub fn new_for_desktop(");
    for (what, text) in [("load_for_desktop", &load), ("new_for_desktop", &new)] {
        for forbidden in [
            "NEXUSCODE",
            "nxrc",
            "current_dir",
            "dirs::",
            "config_dir",
            "data_dir",
            "Path::new(\"",
            "load_with(",
            "build(",
        ] {
            assert!(!text.contains(forbidden), "{what}: {forbidden}: {text}");
        }
    }
    assert!(load.contains("auto_detect_provider(false)"), "{load}");
    assert!(load.contains("filter(|path|path.is_absolute())"), "{load}");
    assert!(
        new.contains("build_with(config,false,memory_path)"),
        "{new}"
    );
    assert_eq!(
        body(&setup, "pub fn diagnose_for_desktop("),
        "diagnose_with(false,false)"
    );
    assert!(without_whitespace(&setup).contains("has_nexuscode_md:project&&"));
}

/// Ambient roots no desktop production source may use, with the only approved
/// occurrences (each exactly counted) and why.
const AMBIENT_ROOTS: &[&str] = &[
    "set_current_dir",
    "current_dir()",
    "current_exe()",
    "CARGO_MANIFEST_DIR",
    "FLASH_MODEL_PATH",
    "NEXUS_WORKSPACE_ROOT",
    "MeasurementState::new(",
    "\"agents/",
    "\"data/",
    "\"services/",
    "\"kernel/src",
    "\"crates/",
];
const APPROVED_AMBIENT: &[(&str, &str, usize, &str)] = &[
    (
        "builder_workspace/trusted_toolchain.rs",
        "current_exe()",
        1,
        "C4D2: the packaged Builder toolchain is located from the installed executable",
    ),
    (
        "commands/chat_llm.rs",
        "CARGO_MANIFEST_DIR",
        1,
        "prebuilt manifests: #[cfg(test)]-only developer checkout source",
    ),
];

/// No desktop production source takes authority or a code-bearing resource
/// from the process working directory, the executable's ancestors, the
/// developer checkout or a path-selecting environment variable.
#[test]
fn desktop_sources_hold_no_ambient_authority_roots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    production_sources(&root, &mut files);
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap();
        for needle in AMBIENT_ROOTS {
            let found = text.matches(needle).count();
            let approved = APPROVED_AMBIENT
                .iter()
                .find(|(file, approved, _, _)| *file == relative && approved == needle)
                .map_or(0, |(_, _, count, _)| *count);
            assert_eq!(found, approved, "{relative}: {needle}");
        }
    }
    // The approved checkout source is compiled only into tests: production
    // resolves no prebuilt manifest directory.
    let chat = include_str!("../commands/chat_llm.rs");
    let at = chat
        .find("fn resolve_prebuilt_manifest_dir_uncached()")
        .expect("prebuilt resolver");
    let open = at + chat[at..].find('{').unwrap();
    let body = without_whitespace(&chat[open + 1..block_end(chat, open) - 1]);
    assert!(body.starts_with("#[cfg(test)]{"), "{body}");
    assert!(body.ends_with("#[cfg(not(test))]{None}"), "{body}");
}

/// Latent unsafe implementations that stay compiled (in other crates) but have
/// no desktop production caller: each needle must not appear in any desktop
/// production source. Wiring one back in fails here until it gains approved
/// authority and this registry and the inventory are updated.
const LATENT_UNSAFE_APIS: &[(&str, &str)] = &[
    (
        "execute_typed_tool",
        "typed tools run programs in a caller or process cwd",
    ),
    (
        "Conductor::new(",
        "legacy Builder build pipeline writing under a raw output dir",
    ),
    (
        "web_builder_agent::checkpoint",
        "legacy raw-path Builder checkpoints, rollback and delete",
    ),
    (
        "web_builder_agent::llm_codegen",
        "legacy Builder writers rooted at a raw output dir",
    ),
    (
        "web_builder_agent::dev_server",
        "legacy unsealed npm/npx launch",
    ),
    ("GenesisEngine", "genesis manifests under an ambient base"),
    (
        "restore_backup",
        "archive entries choose restore targets (C5B)",
    ),
    (
        "cc_cmds::execute_action",
        "computer control runs sh -c on caller text",
    ),
    (
        "mcp2_cmds::mcp_client_add_server",
        "MCP stdio client registers a caller-chosen program",
    ),
    (
        "mcp2_cmds::mcp_client_discover_tools",
        "MCP stdio client spawns a caller-registered program",
    ),
    (
        "mcp2_cmds::mcp_client_call_tool",
        "MCP stdio client spawns a caller-registered program",
    ),
    (
        "mcp2_cmds::mcp_server_handle_request",
        "MCP tools read cwd-relative files",
    ),
    (
        "memory_cmds::memory_save",
        "agent memory persisted under a cwd-relative dir",
    ),
    (
        "memory_cmds::memory_load",
        "agent memory loaded from a cwd-relative dir",
    ),
    (
        "memory_cmds::memory_list_agents",
        "agent memory listed from a cwd-relative dir",
    ),
    (
        "run_agent_loop",
        "nexus-code and computer-use agent loops (cwd root, CLI agent, OS input)",
    ),
    (
        "ContentPipeline",
        "content pipeline writes files and runs git through the shell",
    ),
    (
        "record_file_",
        "Time Machine file entries replayed from raw paths (C5B)",
    ),
    (
        "ActuatorRegistry::with_defaults",
        "full actuator set including shell, code and docker",
    ),
    // Dormant process-, code- and OS-input-bearing APIs in the desktop's
    // dependency closure. Nothing in desktop production calls them; each is
    // named so that a new wiring must be classified first.
    (
        "coder_agent::terminal",
        "coder terminal runs raw command text through the shell",
    ),
    (
        "coder_agent::test_runner",
        "coder test runner runs shell text in a project directory",
    ),
    (
        "coder_agent::fix_loop",
        "coder fix loop runs tests through the shell",
    ),
    (
        "nexus_code::agent",
        "nexus-code agent loop and sub-agents (process cwd root)",
    ),
    (
        "nexus_code::tools",
        "nexus-code bash, file, git, test-runner and sub-agent tools",
    ),
    (
        "tool_registry.get(",
        "direct execution of a registered nexus-code tool",
    ),
    ("VisionAnalyzer", "computer-use vision runs the Claude CLI"),
    (
        "nexus_sdk::typed_tools",
        "SDK typed tools run npm, python and pip",
    ),
    ("WasmtimeSandbox", "WASM agents with host tool functions"),
    ("WasmAgent", "WASM agents with host tool functions"),
    ("ShadowSandbox", "speculative WASM sandbox with host tools"),
    (
        ".build_project(",
        "factory pipeline runs build commands through the shell",
    ),
    (
        ".test_project(",
        "factory pipeline runs test commands through the shell",
    ),
    (
        ".deploy_project(",
        "factory pipeline runs deploy commands through the shell",
    ),
    (
        ".run_full_pipeline(",
        "factory pipeline runs its commands through the shell",
    ),
    ("GovernedShell", "shell actuator"),
    ("GovernedFilesystem", "workspace filesystem actuator"),
    ("CodeExecuteActuator", "code execution actuator"),
    ("DockerActuator", "docker actuator"),
    ("BrowserActuator", "browser automation actuator (node)"),
    ("ComputerUseActuator", "computer-use actuator"),
    ("InputControlActuator", "OS input actuator"),
    ("ScreenCaptureActuator", "screen capture actuator"),
    (
        "GovernedApiClient",
        "API actuator (curl with caller URLs and bodies)",
    ),
    ("ImageGenActuator", "image generation actuator (process)"),
    ("TtsActuator", "speech synthesis actuator (process)"),
    ("SelfEvolutionActuator", "self-evolution actuator"),
    // P0-002C5C: kernel computer-control OS input (xdotool / osascript
    // keystrokes and clicks); both desktop commands that built it are closed.
    (
        "InputAction",
        "OS keyboard and mouse input chosen by the interface or a model",
    ),
    // P0-002C5B: file replay needs a live workspace grant, and the desktop
    // holds none for Time Machine, so it replayed agent state and config only.
    (
        "FileAuthority::new(",
        "Time Machine file replay authority (no desktop grant binding)",
    ),
    (".undo_with(", "Time Machine file replay (C5B: latent)"),
    (".redo_with(", "Time Machine file replay (C5B: latent)"),
    (
        "undo_checkpoint_with(",
        "Time Machine file replay (C5B: latent)",
    ),
    // P2-ENTRY-H1: undo, redo and undo-to-checkpoint are closed commands, and
    // the agent-state, fuel, memory and configuration replay is removed. The
    // forced lifecycle transition belongs to the kernel safety halt alone.
    (
        ".undo()",
        "Time Machine checkpoint replay (P2-ENTRY-H1: closed)",
    ),
    (
        ".redo()",
        "Time Machine checkpoint replay (P2-ENTRY-H1: closed)",
    ),
    (
        ".undo_checkpoint(",
        "Time Machine checkpoint replay (P2-ENTRY-H1: closed)",
    ),
    (
        "UndoAction",
        "Time Machine replay of agent state, fuel, memories or configuration (P2-ENTRY-H1: closed)",
    ),
    (
        "force_transition_agent_state",
        "a lifecycle transition forced past the state machine (kernel safety halt only)",
    ),
    (
        "with_default_path()",
        "computer-use learning stores falling back to a /tmp home",
    ),
    (
        "poll_platform(",
        "message polling (Telegram voice notes in shared temp)",
    ),
    (
        "receive_model(",
        "Nexus Link receive joins a peer-chosen file name",
    ),
    (
        "RetentionBuffer::new(",
        "audit archive defaulting to shared temp",
    ),
    // P0-002C5C: the kernel MCP server builds the full default actuator
    // registry; the desktop only lists its tools. Invoking one would run an
    // actuator outside Phase0AgentExecutor.
    (
        ".invoke_tool(",
        "kernel MCP tool invocation over the full actuator registry",
    ),
    (
        "execute_input_action(",
        "kernel OS keyboard and mouse input (xdotool / osascript)",
    ),
    (
        "tauri_commands::screenshot(",
        "browser bridge screenshot to a caller-chosen output path",
    ),
    (
        "coder_agent::llm_codegen",
        "coder writers rooted at a raw output dir",
    ),
    // P0-002C5C recount: latent process and network APIs of the desktop
    // closure that no earlier needle named.
    (
        "coder_agent::git",
        "coder git helpers run git in a project directory",
    ),
    (
        "netlify::create_site(",
        "Builder deploy upload with a stored provider token",
    ),
    (
        "netlify::deploy(",
        "Builder deploy upload with a stored provider token",
    ),
    (
        "netlify::rollback(",
        "Builder deploy rollback with a stored provider token",
    ),
    (
        "cloudflare::create_site(",
        "Builder deploy upload with a stored provider token",
    ),
    (
        "cloudflare::deploy(",
        "Builder deploy upload with a stored provider token",
    ),
    (
        "cloudflare::rollback(",
        "Builder deploy rollback with a stored provider token",
    ),
    (
        "vercel::deploy(",
        "Builder deploy upload with a stored provider token",
    ),
    (
        "vercel::rollback(",
        "Builder deploy rollback with a stored provider token",
    ),
    (
        "image_gen::generate_image(",
        "Builder image generation fetching a URL the API returns",
    ),
    (
        "image_gen::generate_all_images(",
        "Builder image generation fetching URLs the API returns",
    ),
    ("OidcClient", "OIDC discovery and token exchange"),
    (
        "send_response(",
        "messaging adapter sends with stored bot tokens",
    ),
    (
        "send_consent_prompt(",
        "messaging adapter sends with stored bot tokens",
    ),
    (
        ".send_message(",
        "messaging adapter sends with stored bot tokens",
    ),
    (
        "MessagingBridge",
        "messaging polling and routing into agents",
    ),
    (
        "poll_and_route",
        "messaging polling and routing into agents",
    ),
    ("MatrixAdapter", "Matrix messaging adapter"),
    ("WebhookAdapter", "outbound webhook messaging adapter"),
    (
        "with_base_and_token(",
        "swarm provider with a caller base URL",
    ),
    (
        "with_base_and_key(",
        "swarm provider with a caller base URL",
    ),
    (
        "TcpTransportManager",
        "distributed TCP transport listener and peers",
    ),
    ("nexus_code::bench", "SWE-bench git runs (nx binary only)"),
    (
        "nexus_code::commands",
        "nx slash commands (git in the working directory)",
    ),
    (
        "McpManager",
        "nexus-code MCP servers started from configuration",
    ),
    (
        "mcp_manager",
        "nexus-code MCP servers started from configuration",
    ),
    (
        "router.complete(",
        "nexus-code provider completions (nx_chat is closed)",
    ),
    (
        "router.stream(",
        "nexus-code provider completions (nx_chat is closed)",
    ),
    (
        "StdioMcpClient",
        "MCP stdio client spawning a caller command",
    ),
    (
        "server_runtime",
        "protocols HTTP gateway listener (server binaries only)",
    ),
    (
        ".execute_action(",
        "computer-control engine and actuator registry actions",
    ),
    (
        "nexus_computer_use::input",
        "computer-use OS input controllers",
    ),
    ("KeyboardController", "computer-use OS keyboard input"),
    ("MouseController", "computer-use OS mouse input"),
    ("AppRegistry", "computer-use application launch registry"),
    (
        "GovernedControlEngine",
        "computer-control engine (sh -c and OS input)",
    ),
    // P0-002C5C recount: latent filesystem APIs of the desktop closure that
    // no earlier needle named.
    (
        "PreferenceStore",
        "adaptation preferences stored under a caller-chosen dir",
    ),
    (
        "coder_agent::analyzer",
        "coder analysis reading a caller-chosen project",
    ),
    (
        "coder_agent::editor",
        "coder multi-file editor writing a caller-chosen project",
    ),
    (
        "MultiFileEditor",
        "coder multi-file editor writing a caller-chosen project",
    ),
    (
        "ProjectInitializer",
        "coder project initializer rooted at the working directory",
    ),
    (
        "coder_agent::watcher",
        "coder file watcher over a caller-chosen project",
    ),
    (
        "detect_style(",
        "coder style detection reading a caller-chosen file",
    ),
    (
        "run_social_poster_from_manifest",
        "social-poster manifest and database from caller paths",
    ),
    ("load_manifest(", "agent manifest read from a caller path"),
    (
        "save_cost_tracker(",
        "Builder cost tracker written under a raw project dir",
    ),
    (
        "save_attribution_log(",
        "Builder collaboration log written under a raw project dir",
    ),
    (
        "load_attribution_log(",
        "Builder collaboration log read from a raw project dir",
    ),
    (
        "save_plan_artefacts(",
        "Builder plan artefacts written under a raw project dir",
    ),
    (
        "save_to_file(",
        "session and vector stores written to a caller path",
    ),
    (
        "load_from_file(",
        "session and vector stores read from a caller path",
    ),
    (
        "LocalSlmProvider",
        "local SLM loading model and tokenizer files (feature local-slm)",
    ),
    (
        "RagPipeline::load(",
        "RAG index read from a caller-chosen dir",
    ),
    ("rag.save(", "RAG index written to a caller-chosen dir"),
    (
        "ContentCalendar",
        "content calendar stored at a caller-chosen path",
    ),
    (
        "VisionLoop",
        "control vision loop writing screenshots to a caller-chosen dir",
    ),
    (
        "MemoryPersistence",
        "agent memory persistence under a caller-chosen dir",
    ),
    (
        "write_report(",
        "capability report written to a caller path",
    ),
    (
        "ModelStorage::with_dir(",
        "flash model storage at a caller-chosen dir",
    ),
    (
        "MemoryKernelState::new(",
        "memory kernel databases under a caller-chosen dir",
    ),
    (
        "MemoryAuditLog",
        "memory audit database at a caller-chosen path",
    ),
    (
        "DevicePairingManager",
        "device pairings and keys under caller-chosen paths",
    ),
    (
        "FileAuditStore",
        "distributed audit blocks under a caller-chosen dir",
    ),
    (
        "enforce_retention(",
        "backup retention pruning a caller-chosen dir",
    ),
    ("decrypt_file(", "in-place decryption of a caller path"),
    (
        "rotate_encryption_key(",
        "re-encryption of every database under a caller-chosen dir",
    ),
    (
        "SealedKeyStore",
        "sealed key store (machine-derived secret)",
    ),
    ("derive_machine_secret", "machine-derived key material"),
    (
        "load_all(",
        "persisted agent identities read from a directory",
    ),
    ("GovernanceDb", "governance database at a caller path"),
    (
        "EvidenceFile",
        "replay evidence written to and read from caller paths",
    ),
    (
        "verifier::verify_file(",
        "replay evidence read from a caller path",
    ),
    (
        "open_in_memory(",
        "marketplace registry constructor (no desktop caller)",
    ),
    (
        "App::init(",
        "nexus-code NEXUSCODE.md written into a project dir",
    ),
    (
        "ChatRepl",
        "nexus-code REPL rooted at the working directory",
    ),
    (
        "project_dir(",
        "nexus-code project root from the working directory",
    ),
    ("NexusCodeMd", "NEXUSCODE.md read from a project dir"),
    ("SavedSession", "nexus-code session files"),
    (
        "init_nexuscode_md(",
        "NEXUSCODE.md written into a caller dir",
    ),
    (
        "nexus_code::tui",
        "nexus-code TUI rooted at the working directory",
    ),
    (
        "http_gateway",
        "protocols HTTP gateway (server binaries only)",
    ),
    (
        "ResumableWorkflowEngine",
        "workflow checkpoints under a caller-chosen dir",
    ),
    (
        "enable_retention(",
        "audit archive defaulting to shared temp",
    ),
    (
        "BridgeDaemon",
        "messaging polling into agents (voice notes in shared temp)",
    ),
    (
        "run_polling_loop(",
        "messaging polling into agents (voice notes in shared temp)",
    ),
    (
        "ProviderSelectionConfig::from_env(",
        "provider selection from the environment (LLM_PROVIDER, FLASH_MODEL_PATH)",
    ),
    (
        "ResourceLimiter::default().spawn(",
        "unsealed resource-limited spawn (the Builder uses spawn_sealed)",
    ),
    (".spawn_actuator(", "actuator process spawn"),
    // P0-002C5C (Architect decision): screen observation. The four desktop
    // capture routes are closed. The capture, analysis and emergency-stop
    // APIs behind them must gain no other desktop caller, whether called
    // directly, reached through a module path or imported. Three kernel
    // capture names still sit unused in the shared command-module import
    // blocks; `p0_002c5c_no_desktop_route_observes_the_screen` covers them.
    (
        "take_screenshot",
        "direct computer-use screen capture (grim, scrot, import)",
    ),
    ("ScreenshotOptions", "computer-use screen capture options"),
    (
        "nexus_computer_use::capture",
        "computer-use screen capture module",
    ),
    ("computer_control::capture_screen", "kernel screen capture"),
    ("computer_control::capture_window", "kernel window capture"),
    ("capture_window(", "kernel window capture"),
    (
        "capture_and_store_window",
        "kernel window capture to a stored file",
    ),
    (".capture_screen(", "computer-control engine screen capture"),
    ("query_vision_model", "screen image sent to a vision model"),
    (
        "detect_vision_model",
        "vision model selection for screen analysis",
    ),
    ("reset_emergency_kill_switch", "clears the emergency stop"),
    (
        "ComputerControlEngine::enable",
        "enables the computer-control engine",
    ),
    // P0-FINAL-GATE items B, C, F and I: clients whose destination, peer or
    // helper the caller or PATH would choose.
    (
        ".discover_agent(",
        "A2A discovery of a caller-chosen agent URL",
    ),
    (".send_task(", "A2A task sent to a caller-chosen agent URL"),
    (
        ".get_task_status(",
        "A2A status from a caller-chosen agent URL",
    ),
    (".cancel_task(", "A2A cancel at a caller-chosen agent URL"),
    (
        "a2a_crate_cmds::a2a_crate_send_task",
        "A2A crate send to a caller-chosen URL",
    ),
    (
        "a2a_crate_cmds::a2a_crate_get_task",
        "A2A crate remote status lookup",
    ),
    (
        "a2a_crate_cmds::a2a_crate_discover_agent",
        "A2A crate discovery of a caller-chosen URL",
    ),
    (
        ".connect_server(",
        "MCP host connection to a caller-registered URL",
    ),
    (
        ".call_tool(",
        "MCP host tool call to a caller-registered server",
    ),
    (
        "extract_theme_from_url",
        "theme fetch from a caller-chosen URL",
    ),
    (".send_model(", "Nexus Link model transfer to a peer"),
    ("discover_peer_models(", "Nexus Link peer model listing"),
    (
        "init_provider(",
        "perception provider holding an interface key for curl",
    ),
    (
        "Command::new(\"ollama\")",
        "starting or running the ollama program from PATH",
    ),
    (
        "Command::new(\"which\")",
        "running which from PATH to report on a program",
    ),
    // P0-FINAL-GATE C5: capability-measurement real-inference runners and
    // clients (their only desktop route is closed).
    (
        "tauri_commands::run_ab_validation",
        "capability-measurement A/B run with a provider key and live model calls",
    ),
    (
        "run_batch_evaluation",
        "capability-measurement batch run with a provider key and live model calls",
    ),
    (
        "execute_validation_run_real",
        "capability-measurement validation run with live model calls",
    ),
    (
        "NimClient",
        "capability-measurement client posting to the Groq endpoint with a provider key",
    ),
    (
        "OpenRouterClient",
        "capability-measurement client posting to OpenRouter with a provider key",
    ),
    // P0-FINAL-GATE item G: remote chat text as a consent decision.
    (
        "parse_consent_reply(",
        "messaging chat reply parsed as a consent decision",
    ),
];

/// The only approved construction of the kernel action executor: the Phase
/// Zero agent executor, which never receives a workspace root.
const APPROVED_EXECUTOR: (&str, &str, usize) =
    ("commands/cognitive.rs", "RegistryExecutor::new(", 1);

#[test]
fn latent_unsafe_apis_have_no_desktop_production_caller() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    production_sources(&root, &mut files);
    let mut executors = 0;
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path).unwrap();
        for (needle, why) in LATENT_UNSAFE_APIS {
            assert!(!text.contains(needle), "{relative}: {needle} ({why})");
        }
        let found = text.matches(APPROVED_EXECUTOR.1).count();
        if found > 0 {
            assert_eq!(relative, APPROVED_EXECUTOR.0, "{relative}: executor");
        }
        executors += found;
    }
    assert_eq!(executors, APPROVED_EXECUTOR.2);
    let executor = include_str!("../commands/cognitive.rs");
    let at = executor
        .find("fn phase0_agent_executor(")
        .expect("production agent executor");
    let open = at + executor[at..].find('{').unwrap();
    let body: String = executor[open + 1..block_end(executor, open) - 1]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect();
    let body = without_whitespace(&body);
    assert!(
        body.starts_with("Phase0AgentExecutor{inner:nexus_kernel::cognitive::RegistryExecutor::new(std::path::PathBuf::new(),"),
        "{body}"
    );
}

/// The browser agent's Python bridge is never started by its session code.
#[test]
fn browser_bridge_is_never_started() {
    let session = include_str!("../../../../crates/nexus-browser-agent/src/session.rs");
    let commands = include_str!("../../../../crates/nexus-browser-agent/src/tauri_commands.rs");
    for source in [session, commands] {
        assert!(!source.contains(".start("), "browser bridge start");
    }
}

/// P2-ENTRY-H1: `Supervisor::force_transition_agent_state` sets an agent's
/// state past the lifecycle state machine. Its only production callers are
/// the kernel safety halt's two fallbacks, which crush a runaway agent to
/// `Stopping` or `Stopped`; no Time Machine, desktop or other workspace path
/// reaches it. (`LATENT_UNSAFE_APIS` also bars it from the desktop.)
#[test]
fn p2e_h1_only_the_safety_halt_forces_an_agent_state() {
    let mut sites = Vec::new();
    for (path, text) in workspace_production_sources() {
        for (at, _) in text.match_indices("force_transition_agent_state") {
            let context: String = text[at..].chars().take(240).collect();
            sites.push((path.clone(), context));
        }
    }
    assert!(
        sites
            .iter()
            .all(|(path, _)| path == "kernel/src/supervisor.rs"),
        "{sites:#?}"
    );
    // The definition, and two calls that both carry the safety-halt reason.
    assert_eq!(sites.len(), 3, "{sites:#?}");
    let definitions = sites
        .iter()
        .filter(|(_, context)| {
            without_whitespace(context).starts_with("force_transition_agent_state(&mutself,")
        })
        .count();
    let halts = sites
        .iter()
        .filter(|(_, context)| context.contains("\"safety-halt-forced\""))
        .count();
    assert_eq!((definitions, halts), (1, 2), "{sites:#?}");
}

// ── P0-002C5B regression guards ─────────────────────────────────────────
//
// These scan the production Rust sources of the whole workspace (every
// member except the benchmarks, which are outside the desktop closure), not
// only the desktop crate: the C5B families live in the kernel, connectors and
// shared crates. Comments and `#[cfg(test)]` / `#[cfg(any(test, ..))]` items
// are removed first, so a guard counts only compiled production code.

/// Directories that hold no production source: build output, dependencies,
/// benchmarks, integration tests, examples, benches and fixtures. Dot
/// directories are skipped as well.
const NOT_PRODUCTION_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "benchmarks",
    "tests",
    "examples",
    "benches",
    "fixtures",
];

/// (workspace-relative path, production text) of every production Rust
/// source beneath a `src` directory of the workspace.
fn workspace_production_sources() -> Vec<(String, String)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && !NOT_PRODUCTION_DIRS.contains(&name.as_str()) {
                    walk(root, &path, out);
                }
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let in_src = relative.split('/').any(|component| component == "src");
            if in_src
                && name.ends_with(".rs")
                && !name.ends_with("tests.rs")
                && !name.ends_with("_test.rs")
                && name != "build.rs"
            {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((relative, production_text(&text)));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(out.len() > 500, "workspace sources not found");
    out
}

/// End of the string, raw-string or char literal starting at `i`, if one
/// starts there (a lifetime is not a literal).
fn literal_end(b: &[u8], i: usize) -> Option<usize> {
    let ident = |at: usize| b[at].is_ascii_alphanumeric() || b[at] == b'_';
    match b[i] {
        b'"' => {
            let mut j = i + 1;
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(b.len())
        }
        b'r' if i == 0 || !ident(i - 1) || (b[i - 1] == b'b' && (i == 1 || !ident(i - 2))) => {
            let mut j = i + 1;
            while j < b.len() && b[j] == b'#' {
                j += 1;
            }
            if j >= b.len() || b[j] != b'"' {
                return None;
            }
            let hashes = j - i - 1;
            let mut k = j + 1;
            while k < b.len() {
                if b[k] == b'"' && b[k + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes {
                    return Some(k + 1 + hashes);
                }
                k += 1;
            }
            Some(b.len())
        }
        b'\'' if i + 2 < b.len() => {
            if b[i + 1] == b'\\' {
                let close = b[i + 3..].iter().position(|&c| c == b'\'')?;
                return Some(i + 3 + close + 1);
            }
            let width = match b[i + 1] {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };
            (b.get(i + 1 + width) == Some(&b'\'')).then_some(i + 2 + width)
        }
        _ => None,
    }
}

/// End of the comment starting at `i`, if one starts there. A line comment
/// ends before its newline; block comments nest.
fn comment_end(b: &[u8], i: usize) -> Option<usize> {
    if b[i..].starts_with(b"//") {
        return Some(
            b[i..]
                .iter()
                .position(|&c| c == b'\n')
                .map_or(b.len(), |at| i + at),
        );
    }
    if !b[i..].starts_with(b"/*") {
        return None;
    }
    let (mut depth, mut j) = (0usize, i);
    while j < b.len() {
        if b[j..].starts_with(b"/*") {
            depth += 1;
            j += 2;
        } else if b[j..].starts_with(b"*/") {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return Some(j);
            }
        } else {
            j += 1;
        }
    }
    Some(b.len())
}

/// End of what a `#[cfg(..)]` attribute ending at `i` applies to. An item
/// (or statement) ends at its `;` outside any bracket or at the brace closing
/// its block. A field, variant, match arm or argument also ends at its `,`,
/// or just before the bracket closing the enclosing list.
fn item_end(b: &[u8], mut i: usize) -> usize {
    let first = b[i..]
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .map_or(b.len(), |at| i + at);
    let word: String = b[first..]
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
        .map(|&c| c as char)
        .collect();
    let item = b.get(first) == Some(&b'#')
        || [
            "mod",
            "fn",
            "pub",
            "use",
            "impl",
            "struct",
            "enum",
            "const",
            "static",
            "type",
            "trait",
            "macro_rules",
            "async",
            "unsafe",
            "extern",
            "let",
        ]
        .contains(&word.as_str());
    let (mut nesting, mut braces) = (0usize, 0usize);
    while i < b.len() {
        if let Some(end) = literal_end(b, i).or_else(|| comment_end(b, i)) {
            i = end;
            continue;
        }
        let outermost = nesting == 0 && braces == 0;
        match b[i] {
            b'(' | b'[' => nesting += 1,
            b')' | b']' if outermost => return i,
            b')' | b']' => nesting = nesting.saturating_sub(1),
            b'{' => braces += 1,
            b'}' if braces == 0 => return i,
            b'}' => {
                braces -= 1;
                if braces == 0 && nesting == 0 {
                    return i + 1;
                }
            }
            b';' if outermost => return i + 1,
            b',' if outermost && !item => return i + 1,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Production text of a Rust source: comments removed, and every item under
/// `#[cfg(test)]` or `#[cfg(any(test, ..))]` removed. Literals are kept and
/// are never read as delimiters or attributes.
fn production_text(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if let Some(end) = comment_end(b, i) {
            out.push(b' ');
            i = end;
        } else if b[i..].starts_with(b"#[cfg(test)]") || b[i..].starts_with(b"#[cfg(any(test") {
            let attribute = i + b[i..].windows(2).position(|w| w == b")]").unwrap() + 2;
            i = item_end(b, attribute);
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("cuts fall on ASCII boundaries")
}

#[test]
fn production_text_drops_comments_and_test_items_but_keeps_literals() {
    let src = concat!(
        "fn a() { let s = \"// not a comment #[cfg(test)]\"; } // tail\n",
        "/* block /* nested */ */ fn b<'a>(x: &'a str) -> char { '}' }\n",
        "#[cfg(test)]\nmod tests { fn t() { let _ = \"}\"; } }\n",
        "#[cfg(any(test, feature = \"x\"))]\npub fn helper() -> [u8; 2] { [1, 2] }\n",
        "#[cfg(test)]\nuse std::fmt;\n",
        "fn c() -> &'static str { r#\"raw \" // kept\"# }\n",
        "fn d() -> S { S { #[cfg(test)] probe: 1, kept_field: 2 } }\n",
        "fn e(x: u8) { match x { #[cfg(test)] 0 => gone(), _ => kept_arm() } }\n",
    );
    let text = production_text(src);
    assert!(text.contains("\"// not a comment #[cfg(test)]\""), "{text}");
    assert!(!text.contains("tail") && !text.contains("nested"), "{text}");
    assert!(
        text.contains("fn b<'a>(x: &'a str) -> char { '}' }"),
        "{text}"
    );
    assert!(
        !text.contains("mod tests") && !text.contains("helper"),
        "{text}"
    );
    assert!(!text.contains("std::fmt"), "{text}");
    assert!(text.contains("r#\"raw \" // kept\"#"), "{text}");
    assert!(
        !text.contains("probe") && text.contains("kept_field: 2 } }"),
        "{text}"
    );
    assert!(
        !text.contains("gone") && text.contains("_ => kept_arm() } }"),
        "{text}"
    );
}

/// Every production curl invocation of the workspace, per file. Each one
/// takes caller, model, configuration or provider values only as data: the
/// file must start curl with `-q` and an http(s)-only protocol allowlist
/// (the `CURL_HTTP_ONLY`/`CURL_HTTPS_ONLY` constants or literal `--proto`),
/// put the URL after `--`, send bodies with `--data-raw` or the fixed
/// `--data-binary @-` stdin form, and use no file-reading or config option.
const CURL_SITES: &[(&str, usize)] = &[
    ("app/src-tauri/src/commands/chat_llm.rs", 2),
    ("connectors/core/src/validation.rs", 1),
    ("connectors/llm/src/model_hub.rs", 4),
    ("connectors/llm/src/providers/mod.rs", 2),
    ("connectors/llm/src/providers/ollama.rs", 3),
    ("connectors/web/src/reader.rs", 1),
    ("connectors/web/src/search.rs", 2),
    (
        "crates/nexus-capability-measurement/src/evaluation/nim_client.rs",
        1,
    ),
    (
        "crates/nexus-capability-measurement/src/evaluation/openrouter_client.rs",
        1,
    ),
    ("crates/nexus-external-tools/src/adapter.rs", 1),
    ("crates/nexus-mcp/src/tools.rs", 2),
    ("crates/nexus-memory/src/embedding.rs", 1),
    ("crates/nexus-perception/src/vision.rs", 1),
    ("kernel/src/actuators/api.rs", 1),
    ("kernel/src/actuators/image_gen.rs", 3),
    ("kernel/src/actuators/tts.rs", 1),
    ("kernel/src/actuators/web.rs", 2),
    ("kernel/src/computer_control.rs", 2),
    ("kernel/src/protocols/a2a_client.rs", 2),
    ("protocols/src/mcp_client.rs", 1),
];

/// curl options that read a file, a config or form data, or upload a file.
const CURL_FILE_OPTIONS: &[&str] = &[
    "\"-d\"",
    "\"--data\"",
    "\"--data-ascii\"",
    "\"--data-urlencode\"",
    "\"-F\"",
    "\"--form\"",
    "\"-T\"",
    "\"--upload-file\"",
    "\"-K\"",
    "\"--config\"",
    "\"--url\"",
];

/// C5B URL / curl family: no caller value becomes curl syntax, a `file:`
/// URL or an `@file` body anywhere in the workspace's production code.
#[test]
fn p0_002c5b_curl_invocations_keep_caller_values_out_of_curl_syntax() {
    let mut found = Vec::new();
    for (relative, text) in workspace_production_sources() {
        let invocations =
            text.matches("Command::new(\"curl\")").count() + text.matches("(\"curl\",").count();
        if invocations == 0 {
            continue;
        }
        found.push((relative.clone(), invocations));
        for option in CURL_FILE_OPTIONS {
            assert!(!text.contains(option), "{relative}: curl {option}");
        }
        assert_eq!(
            text.matches("\"--data-binary\"").count(),
            text.matches("\"@-\"").count(),
            "{relative}: --data-binary must read only the fixed stdin marker"
        );
        let first_q = text.matches("\"-q\"").count() + text.matches("CURL_HTTP").count();
        let allowlisted = text.matches("CURL_HTTP").count() + text.matches("\"--proto\"").count();
        let terminated = text.matches("\"--\"").count();
        for (what, count) in [
            ("-q first", first_q),
            ("protocol allowlist", allowlisted),
            ("-- before the URL", terminated),
        ] {
            assert!(count >= invocations, "{relative}: {what}");
        }
    }
    let expected: Vec<_> = CURL_SITES
        .iter()
        .map(|(file, count)| (file.to_string(), *count))
        .collect();
    assert_eq!(
        found, expected,
        "a new curl site must be governed and classified"
    );
}

/// P0-002C5C: every production curl site is bounded in time (a total `-m`
/// or `--max-time`, or for the model download a `--speed-time` stall bound,
/// with its size watched by the caller) and in response size
/// (`--max-filesize`). C5B described all sites as bounded in both; fourteen
/// had no size bound until C5C.
#[test]
fn p0_002c5c_every_production_curl_site_is_bounded_in_time_and_size() {
    let mut sites = 0;
    for (relative, text) in workspace_production_sources() {
        let invocations =
            text.matches("Command::new(\"curl\")").count() + text.matches("(\"curl\",").count();
        if invocations == 0 {
            continue;
        }
        let time = text.matches("\"-m\"").count()
            + text.matches("\"--max-time\"").count()
            + text.matches("\"--speed-time\"").count();
        let size = text.matches("\"--max-filesize\"").count();
        assert!(
            time >= invocations,
            "{relative}: {time} time bounds, {invocations} sites"
        );
        assert!(
            size >= invocations,
            "{relative}: {size} size bounds, {invocations} sites"
        );
        sites += invocations;
    }
    assert_eq!(
        sites,
        CURL_SITES.iter().map(|(_, count)| count).sum::<usize>()
    );
}

/// Ambient per-user roots: HOME and platform directories, the shared temp
/// directory and the working directory.
const STATE_ROOT_NEEDLES: &[&str] = &[
    "var(\"HOME\")",
    "var_os(\"HOME\")",
    "home_dir()",
    "dirs::",
    "\"~/",
    "temp_dir()",
    "\"/tmp",
    "PathBuf::from(\".\")",
    "\".\".into()",
];

/// Every remaining production use of an ambient root, exactly counted, and
/// why it grants no reachable authority. Desktop state derives from the
/// validated identity home (`nexus_kernel::identity_home`) instead.
const APPROVED_STATE_ROOTS: &[(&str, &str, usize, &str)] = &[
    (
        "app/src-tauri/src/oracle_runtime.rs",
        "var_os(\"HOME\")",
        1,
        "validated identity-home policy (delegates to the kernel)",
    ),
    (
        "app/src-tauri/src/oracle_runtime.rs",
        "home_dir()",
        1,
        "Windows native profile only when HOME is absent",
    ),
    (
        "app/src-tauri/src/oracle_runtime.rs",
        "dirs::",
        1,
        "Windows native profile only when HOME is absent",
    ),
    (
        "kernel/src/identity_home.rs",
        "var_os(\"HOME\")",
        1,
        "the validated identity-home policy",
    ),
    (
        "kernel/src/identity_home.rs",
        "home_dir()",
        1,
        "Windows native profile only when HOME is absent",
    ),
    (
        "kernel/src/identity_home.rs",
        "dirs::",
        1,
        "Windows native profile only when HOME is absent",
    ),
    (
        "kernel/src/config.rs",
        "var_os(\"HOME\")",
        1,
        "legacy configuration key input: reads legacy files, never keys a new or changed credential (item A)",
    ),
    (
        "kernel/src/hardware_security/tee_backend.rs",
        "temp_dir()",
        1,
        "final gate: unreached TEE key directory",
    ),
    (
        "agents/web-builder/src/checkpoint.rs",
        "var(\"HOME\")",
        2,
        "latent legacy Builder checkpoints (E2)",
    ),
    (
        "agents/web-builder/src/checkpoint.rs",
        "\".\".into()",
        2,
        "latent legacy Builder checkpoints (E2)",
    ),
    (
        "agents/web-builder/src/project.rs",
        "var(\"HOME\")",
        1,
        "latent legacy Builder project listing (E2)",
    ),
    (
        "agents/web-builder/src/project.rs",
        "\".\".into()",
        1,
        "latent legacy Builder project listing (E2)",
    ),
    (
        "connectors/llm/src/providers/codex_cli.rs",
        "var(\"HOME\")",
        1,
        "latent Codex auth check (its only caller is closed)",
    ),
    (
        "connectors/messaging/src/telegram.rs",
        "temp_dir()",
        1,
        "latent: the desktop never polls the message gateway",
    ),
    (
        "control/src/vision/loop.rs",
        "temp_dir()",
        1,
        "latent: no desktop caller of the control vision loop",
    ),
    (
        "crates/nexus-browser-agent/src/actions.rs",
        "\"/tmp",
        1,
        "latent: the browser bridge is never started",
    ),
    (
        "crates/nexus-computer-use/src/learning/memory.rs",
        "var(\"HOME\")",
        1,
        "latent with_default_path (the desktop passes an identity-home path)",
    ),
    (
        "crates/nexus-computer-use/src/learning/memory.rs",
        "\"/tmp",
        1,
        "latent with_default_path (the desktop passes an identity-home path)",
    ),
    (
        "crates/nexus-computer-use/src/learning/pattern.rs",
        "var(\"HOME\")",
        1,
        "latent with_default_path (the desktop passes an identity-home path)",
    ),
    (
        "crates/nexus-computer-use/src/learning/pattern.rs",
        "\"/tmp",
        1,
        "latent with_default_path (the desktop passes an identity-home path)",
    ),
    (
        "crates/nexus-swarm/src/providers/codex_cli.rs",
        "home_dir()",
        1,
        "swarm Codex provider, never registered by the desktop",
    ),
    (
        "crates/nexus-swarm/src/providers/codex_cli.rs",
        "dirs::",
        1,
        "swarm Codex provider, never registered by the desktop",
    ),
    (
        "crates/nexus-swarm/src/providers/codex_cli.rs",
        "\"~/",
        1,
        "swarm Codex provider, never registered by the desktop",
    ),
    (
        "crates/nexus-world-simulation/src/engine.rs",
        "\"/tmp",
        1,
        "a configuration string the simulator never opens",
    ),
    (
        "kernel/src/audit/retention.rs",
        "\"/tmp",
        1,
        "latent audit archive (benchmarks only)",
    ),
    (
        "nexus-code/src/app.rs",
        "dirs::",
        1,
        "an absolute platform data directory or none",
    ),
    (
        "nexus-code/src/config.rs",
        "dirs::",
        1,
        "Nexus Code user config read from the platform config directory (C5C inventory)",
    ),
    (
        "nexus-code/src/commands/memory_cmd.rs",
        "dirs::",
        4,
        "latent Nexus Code slash command (nx chat is closed)",
    ),
    (
        "nexus-code/src/commands/memory_cmd.rs",
        "PathBuf::from(\".\")",
        4,
        "latent Nexus Code slash command (nx chat is closed)",
    ),
    (
        "nexus-code/src/commands/session.rs",
        "dirs::",
        1,
        "latent Nexus Code slash command (nx chat is closed)",
    ),
    (
        "nexus-code/src/commands/session.rs",
        "PathBuf::from(\".\")",
        1,
        "latent Nexus Code slash command (nx chat is closed)",
    ),
    (
        "nexus-code/src/llm/providers/claude_cli.rs",
        "PathBuf::from(\".\")",
        1,
        "Claude CLI provider, never registered by the desktop",
    ),
    (
        "nexus-code/src/tools/glob.rs",
        "\".\".into()",
        2,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_analyze.rs",
        "home_dir()",
        1,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_analyze.rs",
        "dirs::",
        1,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_analyze.rs",
        "temp_dir()",
        1,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_capture.rs",
        "home_dir()",
        1,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_capture.rs",
        "dirs::",
        1,
        "latent nexus_code::tools",
    ),
    (
        "nexus-code/src/tools/screen_capture.rs",
        "PathBuf::from(\".\")",
        1,
        "latent nexus_code::tools",
    ),
    (
        "cli/src/lib.rs",
        "var(\"HOME\")",
        1,
        "the nexus CLI, outside the desktop",
    ),
    (
        "cli/src/lib.rs",
        "\"/tmp",
        1,
        "the nexus CLI, outside the desktop",
    ),
    (
        "cli/src/router.rs",
        "var_os(\"HOME\")",
        1,
        "the nexus CLI, outside the desktop",
    ),
    (
        "cli/src/router.rs",
        "\"~/",
        4,
        "the nexus CLI, outside the desktop",
    ),
    (
        "crates/nexus-ui-repair/src/driver/loop_.rs",
        "var(\"HOME\")",
        1,
        "developer tool outside the desktop",
    ),
    (
        "crates/nexus-ui-repair/src/driver/loop_.rs",
        "\"/tmp",
        1,
        "developer tool outside the desktop",
    ),
    (
        "crates/nexus-ui-repair/src/governance/acl.rs",
        "var(\"HOME\")",
        1,
        "developer tool outside the desktop",
    ),
    (
        "crates/nexus-ui-repair/src/governance/xvfb_session.rs",
        "\"/tmp",
        1,
        "developer tool outside the desktop",
    ),
];

/// C5B identity-home and private-temp families: no production code reads
/// HOME, a platform directory, the shared temp directory or `.` as a state
/// root except the counted, classified uses above.
#[test]
fn p0_002c5b_state_roots_take_no_home_cwd_or_shared_temp_fallback() {
    for (relative, text) in workspace_production_sources() {
        for needle in STATE_ROOT_NEEDLES {
            let found = text.matches(needle).count();
            let approved = APPROVED_STATE_ROOTS
                .iter()
                .find(|(file, approved, _, _)| *file == relative && approved == needle)
                .map_or(0, |(_, _, count, _)| *count);
            assert_eq!(found, approved, "{relative}: {needle}");
        }
    }
}

/// C5B identifier-join family: the migrated stores name files only through
/// their grammars, and the pre-C5B raw joins stay gone.
#[test]
fn p0_002c5b_identifier_joins_stay_behind_their_grammars() {
    let sources: std::collections::HashMap<_, _> =
        workspace_production_sources().into_iter().collect();
    let text = |file: &str| {
        sources
            .get(file)
            .unwrap_or_else(|| panic!("{file} not found"))
    };
    // Joins that remain sit behind a grammar or an allowlist, each counted;
    // the pre-C5B spellings that bypassed them stay gone.
    for (file, spelling, count) in [
        // identified_file (lowercase grammar) and the email stems name their
        // file only through stored_file, which refuses a case alias (C5C).
        (
            "app/src-tauri/src/commands/apps.rs",
            "join(format!(\"{id}.json\"))",
            0,
        ),
        (
            "app/src-tauri/src/commands/apps.rs",
            "join(format!(\"{stem}.json\"))",
            0,
        ),
        ("app/src-tauri/src/commands/apps.rs", "dir.join(name)", 1),
        // read_messaging_token: an allowlisted &'static str platform.
        (
            "app/src-tauri/src/commands/apps.rs",
            "join(format!(\"{platform}.json\"))",
            1,
        ),
        // gmail/outlook only: an allowlist match or a &'static str.
        (
            "app/src-tauri/src/commands/apps.rs",
            "join(format!(\"{provider}_tokens.json\"))",
            2,
        ),
        // nx_session_file: the name's storage stem, never the raw name, and
        // never a stored file that differs only by case (C5C).
        (
            "app/src-tauri/src/nx_bridge/commands.rs",
            "sessions_dir.join(file)",
            1,
        ),
        (
            "app/src-tauri/src/nx_bridge/commands.rs",
            "join(format!(\"{}.json\", name))",
            0,
        ),
        // Store ids use the lowercase storage grammar, never the plain one.
        (
            "app/src-tauri/src/commands/apps.rs",
            "governed_path::validate_identifier(",
            0,
        ),
        // generate_model_config, after validate_hf_filename.
        ("connectors/llm/src/model_hub.rs", "join(filename)", 1),
        // ModelStorage::model_path, after validate_model_filename.
        (
            "crates/nexus-flash-infer/src/downloader.rs",
            "base_dir.join(filename)",
            1,
        ),
        ("connectors/llm/src/model_hub.rs", "replace('/', \"__\")", 0),
        (
            "connectors/llm/src/nexus_link.rs",
            "models_dir).join(filename)",
            0,
        ),
        ("app/src-tauri/src/lib.rs", "join(&filename_clone)", 0),
        ("sdk/src/memory.rs", "format!(\"{agent_id}.json\")", 0),
    ] {
        assert_eq!(
            text(file).matches(spelling).count(),
            count,
            "{file}: {spelling}"
        );
    }
    for (file, required, at_least) in [
        ("app/src-tauri/src/commands/apps.rs", "identified_file(", 6),
        ("app/src-tauri/src/commands/apps.rs", "stored_file(", 4),
        ("app/src-tauri/src/commands/apps.rs", "case_exact_entry(", 1),
        (
            "app/src-tauri/src/nx_bridge/commands.rs",
            "case_exact_entry(",
            1,
        ),
        ("app/src-tauri/src/commands/apps.rs", "storage_stem(", 2),
        (
            "app/src-tauri/src/nx_bridge/commands.rs",
            "validate_identifier(",
            1,
        ),
        (
            "app/src-tauri/src/nx_bridge/commands.rs",
            "storage_stem(",
            1,
        ),
        (
            "app/src-tauri/src/commands/apps.rs",
            "validate_storage_identifier(",
            1,
        ),
        ("connectors/llm/src/model_hub.rs", "case_exact_relative(", 1),
        (
            "crates/nexus-flash-infer/src/downloader.rs",
            "case_exact(",
            3,
        ),
        (
            "connectors/llm/src/model_hub.rs",
            "validate_hf_model_id(",
            2,
        ),
        (
            "connectors/llm/src/model_hub.rs",
            "validate_hf_filename(",
            2,
        ),
        (
            "connectors/llm/src/nexus_link.rs",
            "regular_file_beneath(",
            1,
        ),
        (
            "crates/nexus-flash-infer/src/downloader.rs",
            "validate_model_filename(",
            2,
        ),
        ("sdk/src/memory.rs", "Uuid::parse_str(", 1),
    ] {
        let found = text(file).matches(required).count();
        assert!(found >= at_least, "{file}: {required} {found} < {at_least}");
    }
}

/// C5B serialized-record and consent-policy families: a stored path chooses
/// no policy, queue, key or replay target.
#[test]
fn p0_002c5b_serialized_records_choose_no_authority() {
    for (relative, text) in workspace_production_sources() {
        for needle in ["ConsentPolicyEngine::load(", "ApprovalQueue::file_backed("] {
            assert!(!text.contains(needle), "{relative}: {needle}");
        }
    }
    // The consent runtime refuses a manifest path before touching anything.
    let consent = production_text(include_str!("../../../../kernel/src/consent.rs"));
    let at = consent
        .find("pub fn from_manifest(")
        .expect("ConsentRuntime::from_manifest");
    let open = at + consent[at..].find('{').unwrap();
    let body = without_whitespace(&consent[open + 1..block_end(&consent, open) - 1]);
    assert!(
        body.starts_with("ifconsent_policy_path.is_some(){returnErr(AgentError::ManifestError(crate::manifest::CONSENT_POLICY_PATH_REFUSED.to_string(),));}"),
        "{body}"
    );
    for manifest in [
        "name = \"a\"\nversion = \"1\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 10\nconsent_policy_path = \"/etc/nexus/consent.toml\"\n",
        "name = \"a\"\nversion = \"1\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 10\nconsent_policy_path = \"../consent.toml\"\n",
    ] {
        assert!(nexus_kernel::manifest::parse_manifest(manifest).is_err());
    }
    // Interface config saves cannot choose the vault key source, and without
    // a loaded current security section nothing authorizes a save.
    let chat = production_text(include_str!("../commands/chat_llm.rs"));
    let fn_body = |name: &str| {
        let at = chat.find(name).unwrap_or_else(|| panic!("{name}"));
        let open = at + chat[at..].find('{').unwrap();
        without_whitespace(&chat[open + 1..block_end(&chat, open) - 1])
    };
    assert_eq!(
        fn_body("pub(crate) fn save_config("),
        "save_config_with(state,config,load_current_security_baseline,save_keeping_stored_credentials,)"
    );
    // P0-002C5C: the interface never reads a stored credential back.
    assert_eq!(
        fn_body("pub(crate) fn get_config("),
        "load_config().map(redacted_config).map_err(agent_error)"
    );
    let body = fn_body("fn save_config_with<");
    assert!(!body.contains("unwrap_or"), "{body}");
    assert!(
        body.contains("Err(_)=>returnErr(deny_config_save(state,BASELINE_UNAVAILABLE)),"),
        "{body}"
    );
    let check = body
        .find("ifconfig.security!=current{")
        .expect("save_config compares the security section");
    assert!(check < body.find("write(&config)").unwrap(), "{body}");
    // The interface baseline comes only from an existing configuration: the
    // helper never creates, writes, migrates or substitutes a default.
    let config = production_text(include_str!("../../../../kernel/src/config.rs"));
    let kernel_body = |name: &str| {
        let at = config.find(name).unwrap_or_else(|| panic!("{name}"));
        let open = at + config[at..].find('{').unwrap();
        without_whitespace(&config[open + 1..block_end(&config, open) - 1])
    };
    assert_eq!(
        kernel_body("pub fn load_current_security_baseline("),
        "letpath=config_path().map_err(|_|SecurityBaselineUnavailable)?;load_security_baseline_from_path(&path)"
    );
    let helper = kernel_body("pub fn load_security_baseline_from_path(");
    assert!(
        helper.starts_with("letraw=fs::read_to_string(path).map_err(|_|SecurityBaselineUnavailable)?;ifraw.trim().is_empty(){returnErr(SecurityBaselineUnavailable);}"),
        "{helper}"
    );
    for forbidden in [
        "save_config_to_path(",
        "fs::write(",
        "create_dir",
        "::default()",
        "unwrap_or",
        "exists()",
    ] {
        assert!(!helper.contains(forbidden), "{forbidden}: {helper}");
    }
    // No production producer records Time Machine file entries.
    let conductor = production_text(include_str!("../../../../agents/conductor/src/lib.rs"));
    assert!(!conductor.contains("record_file_"));
}

// ── P0-002C5C final trust-surface guards ────────────────────────────────

/// Desktop production sources, with comments and test items removed.
fn desktop_production_texts() -> Vec<(String, String)> {
    let mut files = Vec::new();
    production_sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 20, "desktop sources not found");
    files
        .into_iter()
        .map(|path| {
            let text = production_text(&std::fs::read_to_string(&path).unwrap());
            (path.display().to_string(), text)
        })
        .collect()
}

/// P0-002C5C: notification text is data on every platform (an argument
/// after option parsing, an AppleScript `argv` item, or an environment value
/// read by a fixed PowerShell script), and every production caller passes a
/// literal message.
#[test]
fn p0_002c5c_notification_text_never_becomes_a_script() {
    use crate::commands::trust_security::{
        notification_invocation, NOTIFICATION_TEXT_ENV, WINDOWS_NOTIFICATION_SCRIPT,
    };
    for message in [
        "'); Remove-Item -Recurse -Force $HOME; ('",
        "$(Start-Process calc)",
        "\" & do shell script \"id\" & \"",
        "--help",
        "-u critical",
        "line\nbreak",
        "",
    ] {
        let linux = notification_invocation("linux", message).unwrap();
        assert_eq!(linux.program, "notify-send");
        assert_eq!(linux.args, ["--", "Nexus OS", message]);
        assert_eq!(linux.env, None);
        let macos = notification_invocation("macos", message).unwrap();
        assert_eq!(macos.program, "osascript");
        assert_eq!(
            macos.args,
            [
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                "Nexus OS",
                message
            ]
        );
        assert_eq!(macos.env, None);
        let windows = notification_invocation("windows", message).unwrap();
        assert_eq!(windows.program, "powershell");
        assert_eq!(
            windows.args,
            [
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                WINDOWS_NOTIFICATION_SCRIPT
            ]
        );
        assert_eq!(
            windows.env,
            Some((NOTIFICATION_TEXT_ENV, message.to_string()))
        );
    }
    assert!(WINDOWS_NOTIFICATION_SCRIPT.contains(&format!("$env:{NOTIFICATION_TEXT_ENV}")));
    assert_eq!(notification_invocation("freebsd", "x"), None);

    let mut callers = 0;
    for (path, text) in desktop_production_texts() {
        for (at, call) in text.match_indices("show_desktop_notification(") {
            if text[..at].ends_with("fn ") {
                continue;
            }
            let argument = text[at + call.len()..].trim_start();
            assert!(
                argument.starts_with('"'),
                "{path}: notification text must be a literal"
            );
            callers += 1;
        }
    }
    assert_eq!(callers, 1);
}

/// Operator state-location overrides and where each is read. Every mention
/// is a `var_os` read: the kernel resolvers pass it to `operator_override`
/// (a non-empty absolute path, or no location), and the legacy-database
/// cleanup only checks that it is set.
const OPERATOR_OVERRIDE_READS: &[(&str, &str, usize)] = &[
    ("app/src-tauri/src/commands/chat_llm.rs", "NEXUS_DB_PATH", 1),
    ("kernel/src/config.rs", "NEXUS_CONFIG_PATH", 1),
    ("kernel/src/identity_home.rs", "NEXUS_DB_PATH", 1),
];

/// P0-002C5C: `NEXUS_DB_PATH` and `NEXUS_CONFIG_PATH` are launch
/// configuration only. No production code sets or removes an environment
/// variable except the provider-key facade, whose variable names are a fixed
/// allowlist of API-key names; the overrides are read only at the approved
/// sites, and only through `operator_override`.
#[test]
fn p0_002c5c_operator_overrides_stay_launch_configuration() {
    let mut mutations = Vec::new();
    let mut reads = Vec::new();
    for (relative, text) in workspace_production_sources() {
        let count = text.matches("set_var(").count() + text.matches("remove_var(").count();
        if count > 0 {
            mutations.push((relative.clone(), count));
        }
        for name in ["NEXUS_DB_PATH", "NEXUS_CONFIG_PATH"] {
            let mentions = text.matches(&format!("\"{name}\"")).count();
            if mentions > 0 {
                let via_var_os = text.matches(&format!("var_os(\"{name}\")")).count();
                assert_eq!(
                    via_var_os, mentions,
                    "{relative}: {name} read other than by var_os"
                );
                reads.push((relative.clone(), name, mentions));
            }
        }
    }
    assert_eq!(
        mutations,
        [("app/src-tauri/src/commands/chat_llm.rs".to_string(), 1)]
    );
    let approved: Vec<_> = OPERATOR_OVERRIDE_READS
        .iter()
        .map(|(file, name, count)| (file.to_string(), *name, *count))
        .collect();
    assert_eq!(reads, approved);

    // The one environment write names only API-key variables.
    let chat = production_text(include_str!("../commands/chat_llm.rs"));
    let at = chat
        .find("pub(crate) fn save_provider_api_key(")
        .expect("save_provider_api_key");
    let open = at + chat[at..].find('{').unwrap();
    let body = &chat[open..block_end(&chat, open)];
    assert!(body.contains("std::env::set_var(env_name, &api_key);"));
    let names: Vec<&str> = body
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|literal| literal.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .filter(|literal| literal.len() > 1)
        .collect();
    assert_eq!(names.len(), 7, "{names:?}");
    for name in names {
        assert!(name.ends_with("_API_KEY"), "{name}");
    }

    // Both kernel resolvers pass the override to `operator_override`.
    let identity = without_whitespace(&production_text(include_str!(
        "../../../../kernel/src/identity_home.rs"
    )));
    assert!(identity.contains(
        "ifletSome(path)=std::env::var_os(\"NEXUS_DB_PATH\"){returnoperator_override(path);}"
    ));
    let config = without_whitespace(&production_text(include_str!(
        "../../../../kernel/src/config.rs"
    )));
    assert!(config.contains(
        "ifletSome(path)=env::var_os(\"NEXUS_CONFIG_PATH\"){returncrate::identity_home::operator_override(path)"
    ));
}

/// P0-002C5C: a tool call runs at the registered agent's autonomy level,
/// never at a higher level the caller claims, and must name a registered
/// agent.
#[test]
fn p0_002c5c_tool_calls_run_at_the_registered_agents_autonomy() {
    use crate::commands::crate_bridges::tool_call_autonomy;
    let state = crate::AppState::new_in_memory();
    let manifest = nexus_kernel::manifest::parse_manifest(
        "name = \"c5c-tools\"\nversion = \"1.0.0\"\ncapabilities = [\"llm.query\"]\nfuel_budget = 100\nautonomy_level = 2\n",
    )
    .unwrap();
    let agent = state
        .supervisor
        .lock()
        .unwrap()
        .start_agent(manifest)
        .unwrap()
        .to_string();
    assert_eq!(tool_call_autonomy(&state, &agent, 5), Ok(2));
    assert_eq!(tool_call_autonomy(&state, &agent, 1), Ok(1));
    for unregistered in ["", "agent-1", &uuid::Uuid::new_v4().to_string()] {
        assert_eq!(
            tool_call_autonomy(&state, unregistered, 5),
            Err("tools_execute: agent_id must name a registered agent".to_string())
        );
    }
    // The webhook tool needs L4+: an L2 agent claiming L5 is refused.
    let level = tool_call_autonomy(&state, &agent, 5).unwrap();
    let refused = nexus_external_tools::tauri_commands::tools_execute(
        &state.external_tools,
        &agent,
        level,
        "webhook",
        r#"{"url": "https://example.com/hook"}"#,
    )
    .unwrap_err();
    assert!(refused.contains("requires L4+, agent is L2"), "{refused}");
    // The command resolves the level before the engine sees it.
    let bridges = without_whitespace(&production_text(include_str!(
        "../commands/crate_bridges.rs"
    )));
    assert!(bridges.contains(
        "letautonomy_level=tool_call_autonomy(&state,&agent_id,autonomy_level)?;tools_cmds::tools_execute(&state.external_tools,&agent_id,autonomy_level,"
    ));
}

/// P0-002C5C: the agent loop dispatches every planned action through its
/// executor and holds no transport of its own; the swarm coder stays
/// LLM-only (it parses generated files and writes none).
#[test]
fn p0_002c5c_no_delegation_path_runs_around_the_executor() {
    let runtime = production_text(include_str!(
        "../../../../kernel/src/cognitive/loop_runtime.rs"
    ));
    for forbidden in ["A2aClient", "a2a_client", "send_task(", "discover_agent("] {
        assert!(!runtime.contains(forbidden), "cognitive loop: {forbidden}");
    }
    let run_cycle = runtime
        .find("pub fn run_cycle_with_evolution(")
        .expect("run_cycle_with_evolution");
    let open = run_cycle + runtime[run_cycle..].find('{').unwrap();
    let body = &runtime[open..block_end(&runtime, open)];
    assert_eq!(body.matches(".execute(").count(), 1, "one dispatch site");
    assert!(body.contains("executor.execute(agent_id, &action_clone, audit, requires_hitl)"));
    for forbidden in [
        "execute_action(",
        "registry.",
        "PlannedAction::A2aDelegation",
    ] {
        assert!(!body.contains(forbidden), "cycle dispatch: {forbidden}");
    }

    let artisan = production_text(include_str!("../../../../agents/coder/src/swarm_entry.rs"));
    for forbidden in [
        "generate_code_with_llm",
        "generate_code_decomposed",
        "std::fs::",
        "tokio::fs::",
        "Command::new",
        "terminal::",
        "test_runner::",
        "fix_loop::",
        "writer::",
    ] {
        assert!(!artisan.contains(forbidden), "swarm coder: {forbidden}");
    }
    // The integration router is built only from the default configuration,
    // so no integration provider (GitHub, GitLab, Jira, Slack, Teams,
    // Discord, Telegram, ServiceNow, webhook) is ever instantiated.
    let lib = without_whitespace(&production_text(include_str!("../lib.rs")));
    assert_eq!(lib.matches("IntegrationRouter::from_config(").count(), 1);
    assert!(lib.contains(
        "IntegrationRouter::from_config(&nexus_integrations::IntegrationConfig::default(),"
    ));
    for (relative, text) in desktop_production_texts() {
        for forbidden in ["IntegrationConfig{", "IntegrationConfig {", ".set_consent("] {
            assert!(!text.contains(forbidden), "{relative}: {forbidden}");
        }
    }
    // The desktop swarm's social-post adapter only drafts.
    let swarm = without_whitespace(&production_text(include_str!("../commands/swarm.rs")));
    assert_eq!(swarm.matches("HeraldAdapter::new(").count(), 1);
    assert!(
        swarm.contains("Arc::clone(&herald_db),).drafts_only(),"),
        "{swarm}"
    );
    // Restored agent records pass the stored-manifest authority check.
    let agents = without_whitespace(&production_text(include_str!("../commands/agents.rs")));
    assert!(agents
        .contains("ifletErr(error)=nexus_kernel::manifest::validate_stored_manifest(&manifest){"));
}

/// Benchmark-only process sites (P0-002C5C): ungoverned curl and `date`
/// invocations in the benchmark packages. There are none: the five conductor
/// benchmarks that had them are withdrawn (their entry points only deny; see
/// `benchmarks/conductor-bench/tests/phase0_withdrawal.rs`). A new site fails
/// until it is classified here; no other member depends on a benchmark
/// crate, and the installers ship only the desktop.
const BENCHMARK_PROCESS_SITES: &[(&str, usize)] = &[];

/// P0-002C5C: benchmark curl and `date` sites stay counted and
/// benchmark-only: a new one, or a dependency on a benchmark crate from any
/// other member, fails.
#[test]
fn p0_002c5c_benchmark_process_sites_stay_benchmark_only() {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, usize)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "target") {
                    walk(root, &path, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let sites = text.matches("Command::new(\"curl\")").count()
                    + text.matches("(\"curl\",").count()
                    + text.matches("Command::new(\"date\")").count();
                if sites > 0 {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((relative, sites));
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    walk(&root, &root.join("benchmarks"), &mut found);
    let expected: Vec<_> = BENCHMARK_PROCESS_SITES
        .iter()
        .map(|(file, count)| (file.to_string(), *count))
        .collect();
    assert_eq!(
        found, expected,
        "a benchmark process site must be classified"
    );

    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let members: Vec<&str> = manifest
        .split("members")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap()
        .split('"')
        .skip(1)
        .step_by(2)
        .collect();
    assert!(members.len() > 60, "{members:?}");
    for member in members {
        if member.starts_with("benchmarks") {
            continue;
        }
        let cargo = std::fs::read_to_string(root.join(member).join("Cargo.toml")).unwrap();
        for crate_name in ["nexus-benchmarks", "nexus-conductor-benchmark"] {
            assert!(
                !cargo.contains(crate_name),
                "{member} depends on {crate_name}"
            );
        }
    }
}

/// P0-002C5C: the final trust-surface guard. Every regression class named
/// for the Phase Zero surface has at least one guard; removing or renaming a
/// guard, a registry or a cross-crate closure test fails here.
#[test]
fn p0_002c5c_final_trust_surface_guard_is_complete() {
    let own = include_str!("tests.rs");
    let is_test = |source: &str, name: &str| {
        // A Windows checkout may carry CRLF line endings.
        let source = source.replace("\r\n", "\n");
        source.contains(&format!("#[test]\nfn {name}()"))
            || source.contains(&format!("#[test]\n    fn {name}()"))
    };
    let nexus_code = include_str!("../../../../nexus-code/tests/phase0_desktop_config.rs");
    let kernel_loop = include_str!("../../../../kernel/src/cognitive/loop_runtime.rs");
    let identity = include_str!("../../../../kernel/src/identity_home.rs");
    let lib_tests = include_str!("../lib_tests.rs");
    let egress = include_str!("../../../../kernel/src/firewall/egress.rs");
    let fg_egress = include_str!("fg_egress/tests.rs");
    let fg_secrets = include_str!("fg_secrets/tests.rs");
    let kernel_config = include_str!("../../../../kernel/src/config.rs");
    let fg_standalone = include_str!("fg_standalone/tests.rs");
    let bench_withdrawal =
        include_str!("../../../../benchmarks/conductor-bench/tests/phase0_withdrawal.rs");
    let fg_reliability = include_str!("fg_reliability/tests.rs");
    let fg_approval = include_str!("fg_approval/tests.rs");
    let fg_webview = include_str!("fg_webview/tests.rs");
    let scheduled_tests = include_str!("../commands/cognitive/scheduled_tests.rs");
    let computer_use_loop =
        include_str!("../../../../crates/nexus-computer-use/src/agent/loop_controller.rs");
    let measurement_client = include_str!(
        "../../../../crates/nexus-capability-measurement/src/evaluation/nim_client.rs"
    );
    let p2_entry = include_str!("../p2_entry_tests.rs");
    let mut pinned = std::collections::HashSet::new();
    for (regression, source, guards) in [
        (
            "a closed command reopened",
            own,
            &[
                "closed_commands_stay_registered_take_no_input_and_only_deny",
                "closed_handlers_return_only_their_bounded_reason",
                "closure_reasons_are_bounded_and_echo_no_input",
            ][..],
        ),
        // P2-ENTRY-H1: Time Machine checkpoint replay stays closed.
        (
            "Time Machine checkpoint replay reopened, or an agent state forced outside the kernel safety halt",
            own,
            &["p2e_h1_only_the_safety_halt_forces_an_agent_state"][..],
        ),
        (
            "Time Machine checkpoint replay reopened, or an agent state forced outside the kernel safety halt",
            p2_entry,
            &[
                "p2e_h1_tm_01_replay_commands_take_no_input_and_only_deny",
                "p2e_h1_tm_02_replay_attempts_change_no_agent_state_fuel_memory_config_or_file",
                "p2e_h1_tm_03_the_desktop_keeps_no_checkpoint_replay_path",
                "p2e_h1_tm_04_replay_attempts_leave_the_saved_governance_configuration_unchanged",
            ][..],
        ),
        (
            "a legacy Builder raw path or a latent actuator wired into the desktop",
            own,
            &["latent_unsafe_apis_have_no_desktop_production_caller"][..],
        ),
        (
            "a CLI agent re-enabled",
            own,
            &["desktop_sources_start_no_external_cli_agent"][..],
        ),
        (
            "working-directory Nexus Code configuration loaded by the desktop",
            own,
            &["desktop_nexus_code_takes_no_configuration_from_the_working_directory"][..],
        ),
        (
            "working-directory Nexus Code configuration loaded by the desktop",
            nexus_code,
            &["desktop_entry_points_ignore_hostile_project_files_in_the_working_directory"][..],
        ),
        (
            "an A2A or other delegation bypassing the executor",
            own,
            &[
                "p0_002c5c_no_delegation_path_runs_around_the_executor",
                "p0_002c5c_a2a_and_agent_actions_are_decided_by_the_production_executor",
                "production_agent_executor_refuses_filesystem_and_process_actions",
            ][..],
        ),
        (
            "an A2A or other delegation bypassing the executor",
            kernel_loop,
            &["p0_002c5c_a2a_delegation_is_decided_by_the_executor"][..],
        ),
        (
            "an unclassified or unbounded curl or benchmark process site",
            own,
            &[
                "p0_002c5b_curl_invocations_keep_caller_values_out_of_curl_syntax",
                "p0_002c5c_every_production_curl_site_is_bounded_in_time_and_size",
                "p0_002c5c_benchmark_process_sites_stay_benchmark_only",
            ][..],
        ),
        (
            "a new ambient root",
            own,
            &[
                "desktop_sources_hold_no_ambient_authority_roots",
                "p0_002c5b_state_roots_take_no_home_cwd_or_shared_temp_fallback",
            ][..],
        ),
        (
            "an identifier-to-file join without a grammar or stem",
            own,
            &["p0_002c5b_identifier_joins_stay_behind_their_grammars"][..],
        ),
        (
            "a manifest or persisted path becoming authority",
            own,
            &["p0_002c5b_serialized_records_choose_no_authority"][..],
        ),
        (
            "a manifest or persisted path becoming authority",
            lib_tests,
            &[
                "p0_002c5b_persisted_agents_naming_a_consent_policy_path_are_not_restored",
                "p0_002c5c_persisted_agents_holding_unregistered_authority_are_not_restored",
            ][..],
        ),
        (
            "an operator path override becoming interface- or model-controlled",
            own,
            &["p0_002c5c_operator_overrides_stay_launch_configuration"][..],
        ),
        (
            "an operator path override becoming interface- or model-controlled",
            identity,
            &["p0_002c5c_operator_overrides_are_absolute_or_no_location"][..],
        ),
        (
            "notification text becoming a script",
            own,
            &["p0_002c5c_notification_text_never_becomes_a_script"][..],
        ),
        (
            "a caller-asserted tool autonomy level",
            own,
            &["p0_002c5c_tool_calls_run_at_the_registered_agents_autonomy"][..],
        ),
        (
            "model, remote or note text rendered as markup in the webview",
            own,
            &["p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed"][..],
        ),
        (
            "screen observation, or enabling it, over desktop IPC",
            own,
            &["p0_002c5c_no_desktop_route_observes_the_screen"][..],
        ),
        (
            "screen observation, or enabling it, over desktop IPC",
            lib_tests,
            &["p0_002c5c_screen_observation_requests_are_denied_and_change_nothing"][..],
        ),
        (
            "an egress entry admitting another scheme, port, host or path",
            egress,
            &[
                "p0_002c5c_entries_admit_only_whole_hosts_and_path_segments",
                "p0_002c5c_explicit_schemes_and_ports_are_enforced",
                "p0_002c5c_legacy_scheme_less_entries_keep_their_documented_meaning",
                "p0_002c5c_malformed_or_ambiguous_endpoints_admit_nothing",
            ][..],
        ),
        (
            "a caller-chosen destination or peer reached from the desktop",
            fg_egress,
            &[
                "p0_fg_caller_chosen_destinations_are_closed_commands",
                "p0_fg_closed_destination_handlers_return_only_their_reason",
                "p0_fg_the_desktop_calls_no_caller_chosen_destination_client",
                "p0_fg_agent_web_fetch_is_not_egress_authority",
                "p0_fg_desktop_tool_calls_reach_no_caller_chosen_destination",
            ][..],
        ),
        (
            "an Ollama address from the interface or a stored record",
            fg_egress,
            &[
                "p0_fg_the_ollama_address_is_backend_configuration",
                "p0_fg_caller_ollama_addresses_are_refused_before_anything_connects",
                "p0_fg_the_persisted_ollama_address_chooses_no_destination",
            ][..],
        ),
        (
            "a credential on a process command line or in a returned error",
            fg_egress,
            &[
                "p0_fg_no_reachable_credential_reaches_a_curl_command_line",
                "p0_fg_perception_takes_no_key_and_sends_nothing",
                "p0_fg_messaging_errors_never_carry_the_bot_token",
            ][..],
        ),
        (
            "a helper started or run from PATH",
            fg_egress,
            &["p0_fg_nexus_starts_no_ollama_and_runs_no_helper_to_find_it"][..],
        ),
        (
            "an egress closure reason that echoes input",
            fg_egress,
            &["p0_fg_egress_closure_reasons_are_bounded_and_echo_no_input"][..],
        ),
        (
            "a new or changed credential under an ambient key, an unvalidated vault key source, or a token persisted outside an approved secret store",
            fg_secrets,
            &[
                "p0_fg_a_configuration_writes_check_key_material_before_writing",
                "p0_fg_a_desktop_config_key_material_comes_from_the_launch_environment",
                "p0_fg_interface_saves_keep_the_ollama_endpoint_backend_owned",
                "p0_fg_e_vault_key_sources_are_validated_on_what_is_read",
                "p0_fg_secret_storage_closure_reason_is_bounded",
                "p0_fg_a_deploy_credentials_are_never_newly_stored",
                "p0_fg_h_sign_in_flows_persist_no_token",
                "p0_fg_h_messaging_tokens_are_never_copied_to_plaintext_files",
                "p0_fg_h_api_client_collections_are_checked_before_writing",
            ][..],
        ),
        (
            "a new or changed credential under an ambient key, an unvalidated vault key source, or a token persisted outside an approved secret store",
            kernel_config,
            &[
                "p0_fg_a_new_or_changed_credentials_need_the_operator_key",
                "p0_fg_a_legacy_ciphertext_still_reads_and_is_never_rewritten",
                "p0_fg_a_an_unreadable_configuration_is_never_overwritten",
            ][..],
        ),
        (
            "a withdrawn standalone binary, alias, recipe or workflow reactivated",
            fg_standalone,
            &[
                "p0_fg_standalone_every_binary_target_is_inventoried",
                "p0_fg_standalone_every_example_target_is_inventoried",
                "p0_fg_standalone_no_alias_reaches_a_withdrawn_entry",
                "p0_fg_standalone_only_the_desktop_embeds_nexus_code",
                "p0_fg_standalone_computer_use_is_not_re_exposed",
                "p0_fg_standalone_every_recipe_is_inventoried",
                "p0_fg_standalone_no_workflow_ships_a_standalone_binary",
                "p0_fg_standalone_every_bench_target_is_inventoried",
                "p0_fg_standalone_withdrawn_packages_run_no_other_build_script",
                "p0_fg_standalone_readme_names_every_withdrawn_binary",
            ][..],
        ),
        (
            "a withdrawn standalone binary, alias, recipe or workflow reactivated",
            bench_withdrawal,
            &[
                "p0_bench_every_withdrawn_benchmark_invocation_is_withdrawn",
                "p0_bench_withdrawn_entries_and_package_targets_are_pinned",
                "p0_bench_docs_describe_the_withdrawn_benchmarks_as_withdrawn",
            ][..],
        ),
        (
            "an unbounded resource surface reachable from the interface",
            fg_reliability,
            &[
                "p0_fg_k_approved_limits_are_pinned",
                "p0_fg_k_stress_persona_count_is_refused_outside_its_bound",
                "p0_fg_k_parallel_simulation_count_is_refused_outside_its_bound",
                "p0_fg_k_dilated_session_iterations_are_refused_outside_their_bound",
                "p0_fg_k_agent_schedules_fire_at_most_once_per_minute",
                "p0_fg_k_a_scheduled_tick_never_overlaps_the_agents_running_loop",
                "p0_fg_k_the_frontend_error_command_uses_the_bounded_log",
                "p0_fg_k_build_records_outside_the_bounds_are_refused",
                "p0_fg_k_adversarial_rounds_are_refused_outside_their_bound",
                "p0_fg_k_temporal_fork_limits_are_refused_and_never_stored",
                "p0_fg_k_an_older_loops_exit_keeps_a_newer_loops_cancellation_entry",
            ][..],
        ),
        (
            "an unbounded resource surface reachable from the interface",
            fg_approval,
            &[
                "p0_fg_k_simulation_and_arena_bounds_precede_any_work",
                "p0_fg_manifest_schedules_are_checked_before_any_state_change",
            ][..],
        ),
        (
            "an unbounded resource surface reachable from the interface",
            scheduled_tests,
            &["p0_fg_sub_minute_manifest_schedules_fail_create_and_start"][..],
        ),
        (
            "a caller's boolean, name or IPC call treated as human approval",
            scheduled_tests,
            &["p0_fg_scheduled_ticks_refuse_a_transcendent_agent_before_any_state_change"][..],
        ),
        (
            "an unbounded resource surface reachable from the interface",
            lib_tests,
            &[
                "p0_fg_parallel_simulation_variants_are_bounded_before_any_model_call",
                "p0_fg_adversarial_session_rounds_are_bounded_before_any_work",
                "p0_fg_refused_manifest_schedules_fail_create_and_start",
            ][..],
        ),
        (
            "a closed capability-measurement route reopened, or a provider key sent to another provider's endpoint",
            fg_approval,
            &[
                "p0_fg_c5_ab_validation_route_is_closed_before_any_input",
                "p0_fg_c5_measurement_clients_take_only_the_groq_key",
                "p0_fg_c5_desktop_reaches_only_in_memory_measurement",
                "p0_fg_source_lists_follow_their_directories",
                "p0_fg_directory_guards_read_their_directories",
            ][..],
        ),
        (
            "a closed capability-measurement route reopened, or a provider key sent to another provider's endpoint",
            measurement_client,
            &["p0_fg_groq_client_key_never_falls_back_to_another_provider"][..],
        ),
        (
            "a caller's boolean, name or IPC call treated as human approval",
            fg_approval,
            &[
                "p0_fg_g_transcendent_agents_are_refused_before_any_state_change",
                "p0_fg_g_goal_loop_and_tool_routes_check_for_transcendent_agents_first",
                "p0_fg_g_enabled_warden_review_denies_without_any_lookup",
                "p0_fg_g_l6_checks_use_the_named_bound",
                "p0_fg_g_caller_asserted_approval_commands_only_deny",
                "p0_fg_g_consent_decisions_record_no_caller_identity",
                "p0_fg_g_self_improvement_acceptance_is_recorded_truthfully",
                "p0_fg_g_self_improvement_report_counts_only_applied_changes",
                "p0_fg_g_no_actuator_reads_the_hitl_approval_flag",
            ][..],
        ),
        (
            "a caller's boolean, name or IPC call treated as human approval",
            lib_tests,
            &[
                "p0_fg_transcendent_creation_is_refused_and_changes_nothing",
                "p0_fg_transcendent_activation_is_refused_and_changes_nothing",
                "p0_fg_transcendent_approval_is_refused_and_changes_nothing",
                "p0_fg_stored_transcendent_records_are_not_registered_and_stay_stored",
                "p0_fg_startup_registers_no_transcendent_agent_on_any_run",
                "p0_fg_goal_loop_and_tool_routes_refuse_a_transcendent_agent",
                "p0_fg_enabled_warden_review_denies_and_no_stand_in_can_allow",
                "p0_fg_stored_levels_above_l6_count_as_transcendent",
                "p0_fg_transcendent_check_matches_every_spelling_of_a_stored_id",
                "p0_fg_transcendent_resume_is_refused_and_changes_nothing",
                "p0_fg_desktop_consent_resolutions_record_the_interface_label",
                "p0_fg_desktop_approvals_do_not_reach_the_kernel_consent_queue",
                "p0_fg_self_improvement_acceptance_claims_no_hitl_approval",
                "p0_fg_self_improvement_report_counts_no_recorded_acceptance_as_applied",
            ][..],
        ),
        (
            "end of input or a read error taken as approval (E6)",
            computer_use_loop,
            &[
                "test_approval_eof_aborts_instead_of_approving",
                "test_approval_read_error_aborts",
                "test_approval_modify_requires_an_entered_replacement",
            ][..],
        ),
        (
            "end of input or a read error taken as approval (E6)",
            fg_approval,
            &["p0_fg_g_eof_or_a_read_error_is_never_an_approval"][..],
        ),
        (
            "a non-app origin, frame or navigation reaching an application command",
            fg_webview,
            &[
                "p0_fg_webview_app_manifest_lists_every_registered_command",
                "p0_fg_webview_build_script_emits_the_app_manifest",
                "p0_fg_webview_capability_is_local_main_only",
                "p0_fg_webview_conf_has_restrictive_csp_and_guarded_window",
                "p0_fg_webview_privileged_document_loads_no_third_party_resources",
                "p0_fg_webview_app_origin_is_resolved_like_tauri_resolves_the_app_url",
                "p0_fg_webview_navigation_admits_only_the_exact_app_origin",
                "p0_fg_webview_main_window_wires_navigation_and_newwindow_guards",
                "p0_fg_webview_boundary_exposes_only_build_main_window",
                "p0_fg_webview_app_command_ipc_is_local_main_only",
                "p0_fg_webview_comment_stripper_drops_only_comments",
            ][..],
        ),
        (
            "a helper, download or messaging request left unowned or unbounded",
            fg_egress,
            &[
                "p0_fg_the_application_exit_ends_in_flight_model_downloads",
                "p0_fg_model_registration_uses_the_authorized_ollama_address",
                "p0_fg_messaging_requests_are_bounded_in_time_and_size",
                "p0_r1_email_requests_are_bounded_and_follow_no_redirect",
                "p0_r1_deploy_token_requests_use_the_bounded_client",
            ][..],
        ),
        (
            "a new or changed credential under an ambient key, an unvalidated vault key source, or a token persisted outside an approved secret store",
            fg_secrets,
            &[
                "p0_fg_a_backend_protection_changes_are_audited",
                "p0_fg_a_no_desktop_test_builds_the_real_application_state",
            ][..],
        ),
        (
            "a withdrawn standalone binary, alias, recipe or workflow reactivated",
            fg_standalone,
            &[
                "p0_fg_standalone_gitlab_includes_are_recognized",
                "p0_fg_standalone_shipping_recognizers_catch_probes",
            ][..],
        ),
        (
            "an unbounded resource surface reachable from the interface",
            fg_reliability,
            &[
                "p0_fg_k_an_in_memory_state_loop_writes_no_identity_home_database",
                "p0_fg_dep_wasmtime_uses_no_dynamic_component_val_api",
            ][..],
        ),
    ] {
        for guard in guards {
            pinned.insert(*guard);
            assert!(
                is_test(source, guard),
                "{regression}: guard {guard} is missing"
            );
        }
    }
    // P0-FINAL-GATE: every test in a Final Gate guard module is pinned above,
    // so no guard can be removed or renamed without failing this test.
    for (module, source) in [
        ("fg_approval", fg_approval),
        ("fg_egress", fg_egress),
        ("fg_reliability", fg_reliability),
        ("fg_secrets", fg_secrets),
        ("fg_standalone", fg_standalone),
        ("fg_webview", fg_webview),
    ] {
        let source = source.replace("\r\n", "\n");
        let mut lines = source.lines();
        let mut found = 0;
        while let Some(line) = lines.next() {
            if line.trim() != "#[test]" {
                continue;
            }
            let name = lines
                .by_ref()
                .map(str::trim)
                .find(|next| !next.starts_with("#["))
                .and_then(|next| next.strip_prefix("fn "))
                .and_then(|rest| rest.split('(').next())
                .unwrap_or_else(|| panic!("{module}: a #[test] without a following fn"));
            found += 1;
            assert!(
                pinned.contains(name),
                "{module}: guard {name} is not pinned in the final trust-surface guard"
            );
        }
        assert!(found > 0, "{module}: no guard found");
    }
    for registry in [
        "const CLOSED_COMMANDS: &[(&str, Closure)]",
        "const LATENT_UNSAFE_APIS: &[(&str, &str)]",
        "const AMBIENT_ROOTS: &[&str]",
        "const CURL_SITES: &[(&str, usize)]",
        "const APPROVED_STATE_ROOTS: &[(&str, &str, usize, &str)]",
        "const NEXUS_CODE_CLI_AGENT_ENTRY_POINTS: &[&str]",
        "const OPERATOR_OVERRIDE_READS: &[(&str, &str, usize)]",
        "const BENCHMARK_PROCESS_SITES: &[(&str, usize)]",
        "const FRONTEND_HTML_SINKS: &[(&str, usize, &str)]",
    ] {
        assert!(own.contains(registry), "registry {registry} is missing");
    }
    assert!(
        fg_egress.contains("const CREDENTIAL_CURL_SITES: &[(&str, usize, &str)]"),
        "registry CREDENTIAL_CURL_SITES is missing"
    );
    for registry in [
        "const BINARY_TARGETS: &[(&str, &str, &str, Disposition)]",
        "const EXAMPLE_TARGETS: &[(&str, &str, &str)]",
        "const ALIAS_NEEDLES: &[(&str, &[&str])]",
        "const RECIPE_FILES: &[&str]",
        "const WITHDRAWN_BINARIES: &[&str]",
        "const BENCH_TARGETS: &[(&str, &str, &str)]",
        "const PROTOCOLS_BUILD_SCRIPT: &str",
    ] {
        assert!(
            fg_standalone.contains(registry),
            "registry {registry} is missing"
        );
    }
}

/// Frontend raw-HTML sinks (P0-002C5C) and what makes each safe. The webview
/// CSP is `null`, so markup built from model, remote or note text would run
/// with access to every IPC command.
const FRONTEND_HTML_SINKS: &[(&str, usize, &str)] = &[
    (
        "src/components/browser/BuildMode.tsx",
        1,
        "highlightCode escapes the generated code first",
    ),
    (
        "src/components/builder/ShareDialog.tsx",
        1,
        "the backend QR code SVG (qrcode crate, no text)",
    ),
    (
        "src/pages/AiChatHub.tsx",
        3,
        "renderChatContent escapes model text first",
    ),
    (
        "src/pages/ApiClient.tsx",
        1,
        "highlightJson escapes the response body first",
    ),
    (
        "src/pages/NotesApp.tsx",
        1,
        "renderNoteMarkdown escapes note text first; http(s) links only",
    ),
];

/// The top-level, trimmed arguments of the JS call whose `(` is at byte
/// `open`, honouring quotes, template literals and bracket nesting (`None` if
/// the call is unterminated). `f()` has no arguments.
fn js_call_args(text: &str, open: usize) -> Option<Vec<String>> {
    let b = text.as_bytes();
    if b.get(open) != Some(&b'(') {
        return None;
    }
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut start = open + 1;
    let mut i = open + 1;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = Some(c),
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b')' => {
                let last = text[start..i].trim();
                if !last.is_empty() || !args.is_empty() {
                    args.push(last.to_owned());
                }
                return Some(args);
            }
            b',' if depth == 0 => {
                args.push(text[start..i].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `text` with its JS comments (`// ..` and `/* .. */`, including
/// `/*@vite-ignore*/`) removed and string and template literals kept. Regex
/// literals are not recognised, so a quote inside one can keep a later comment
/// in place; callers check the raw text as well.
fn strip_js_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i = text[i + 2..]
                .find("*/")
                .map_or(b.len(), |end| i + 2 + end + 2);
            out.push(b' ');
            continue;
        }
        if matches!(c, b'"' | b'\'' | b'`') {
            quote = Some(c);
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8(out).expect("comment stripping keeps UTF-8 boundaries")
}

/// Whether `arg` is exactly one string literal ('..', ".." or a template
/// literal without substitutions), so the value is fixed in the source.
fn is_js_string_literal(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() >= 2
        && matches!(b[0], b'"' | b'\'' | b'`')
        && b[b.len() - 1] == b[0]
        && !arg[1..arg.len() - 1].contains(b[0] as char)
        && !(b[0] == b'`' && arg.contains("${"))
}

/// The opening tag of the JSX/HTML element starting at `at` (`<iframe ...>` or
/// `<iframe ... />`), with comments removed. Quoted strings, template
/// literals and `{..}` expressions are honoured, so a `>` inside an
/// expression (an arrow function, a type argument) does not end the tag.
fn jsx_opening_tag(text: &str, at: usize) -> String {
    let b = text.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = at;
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = text[i + 2..].find("*/").expect("end of a comment in a tag");
            i += 2 + end + 2;
            out.push(b' ');
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') && depth > 0 {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        match c {
            b'"' | b'\'' | b'`' => quote = Some(c),
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'>' if depth == 0 => {
                out.push(c);
                return String::from_utf8(out).expect("tag text is UTF-8");
            }
            _ => {}
        }
        out.push(c);
        i += 1;
    }
    panic!("unterminated tag at byte {at}");
}

/// P0-002C5C: every raw-HTML sink in the frontend is counted and renders
/// escaped text; every `srcDoc` preview iframe is sandboxed so that it never
/// runs script with the app's origin; the crash screen escapes its message.
#[test]
fn p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed() {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !matches!(name.as_str(), "__tests__" | "test" | "node_modules") {
                    walk(root, &path, out);
                }
            } else if (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.contains(".test.")
            {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    walk(&app, &app.join("src"), &mut files);
    assert!(files.len() > 100, "frontend sources not found");

    let mut sinks = Vec::new();
    let mut previews = 0;
    let mut object_urls = 0;
    for (relative, text) in &files {
        let count = text.matches("dangerouslySetInnerHTML={").count();
        if count > 0 {
            sinks.push((relative.clone(), count));
        }
        // Every iframe must render inline `srcDoc` content with exactly
        // `sandbox=""` (no token at all: no scripts, opaque origin, no
        // navigation of the app document) and no `src` URL. Under item D a
        // preview then has an opaque origin and no script, so it can neither
        // run in nor reach the IPC of the privileged window — on Windows every
        // frame receives the invoke key, and a same-origin frame counts as the
        // local app origin on every platform, so this rule is load-bearing.
        // The opening tag is parsed with its quotes, `{..}` expressions and
        // comments, so closing-tag iframes and `>` inside expressions are
        // handled, and a props spread (which could override `sandbox`) fails.
        // (Before this closure a remote page was embedded with `allow-scripts
        // allow-same-origin allow-popups`, and a loopback dev server with
        // `allow-same-origin`; both are removed.)
        assert_eq!(
            text.to_ascii_lowercase().matches("<iframe").count(),
            text.matches("<iframe").count(),
            "{relative}: iframe tags must be spelled `<iframe`"
        );
        for (at, _) in text.match_indices("<iframe") {
            let tag = jsx_opening_tag(text, at);
            let compact: String = tag.split_whitespace().collect();
            assert!(
                !compact.contains("{..."),
                "{relative}: iframe props spread could override its sandbox: {tag}"
            );
            assert!(
                compact.matches("sandbox").count() == 1 && compact.contains("sandbox=\"\""),
                "{relative}: an iframe must carry exactly sandbox=\"\": {tag}"
            );
            assert_eq!(
                compact.matches("srcDoc=").count(),
                1,
                "{relative}: iframe must render inline srcDoc content: {tag}"
            );
            assert!(
                !compact.contains("src="),
                "{relative}: iframe must not load a remote or loopback src URL: {tag}"
            );
            previews += 1;
        }
        // No iframe built from script (createElement, createElementNS,
        // React.createElement, innerHTML strings): only the JSX tags above,
        // whose sandbox this guard can see, may create one.
        let lower = text.to_ascii_lowercase();
        for literal in ["\"iframe\"", "'iframe'", "`iframe`"] {
            assert!(
                !lower.contains(literal),
                "{relative}: iframes must not be created from script ({literal})"
            );
        }
        // Script may not create an element whose tag is not fixed in the
        // source, nor a frame-like element, nor touch any element's sandbox:
        // no `.sandbox` access, and no set/remove/toggle of a `sandbox`
        // attribute.
        for (at, _) in text.match_indices("createElement") {
            let rest = &text[at + "createElement".len()..];
            let name_end = if rest.starts_with("NS(") { 2 } else { 0 };
            if !rest[name_end..].starts_with('(') {
                continue;
            }
            let args = js_call_args(text, at + "createElement".len() + name_end)
                .unwrap_or_else(|| panic!("{relative}: unterminated createElement call"));
            let tag_index = usize::from(name_end == 2);
            let tag = args.get(tag_index).map(String::as_str).unwrap_or_default();
            assert!(
                is_js_string_literal(tag),
                "{relative}: createElement needs a literal tag, not `{tag}`"
            );
            let tag = tag[1..tag.len() - 1].to_ascii_lowercase();
            assert!(
                ![
                    "iframe",
                    "frame",
                    "frameset",
                    "object",
                    "embed",
                    "portal",
                    "fencedframe"
                ]
                .contains(&tag.as_str()),
                "{relative}: script may not create a `{tag}` element"
            );
        }
        for (at, _) in text.match_indices(".sandbox") {
            let next = text[at + ".sandbox".len()..].chars().next().unwrap_or(' ');
            // `.sandboxed`, `.sandboxId` etc. are other identifiers.
            assert!(
                next.is_ascii_alphanumeric() || next == '_' || next == '$',
                "{relative}: script may not read or change an element's sandbox"
            );
        }
        let compact_lower: String = lower.split_whitespace().collect();
        for api in [
            "setattribute(",
            "removeattribute(",
            "toggleattribute(",
            "setattributens(",
        ] {
            for quote in ['"', '\'', '`'] {
                let needle = format!("{api}{quote}sandbox");
                assert!(
                    !compact_lower.contains(&needle),
                    "{relative}: script may not change a sandbox attribute ({needle})"
                );
            }
        }
        // P0 item D: the navigation guard admits `blob:` URLs created by the
        // app origin (downloads), and a scriptable document at such a URL
        // would run as the local app origin with the invoke key. So every
        // object URL comes from a `new Blob(parts, { type: "<literal>" })`
        // whose type is a reviewed, non-scriptable literal (a ternary between
        // such literals is allowed); nothing can re-type or wrap one: no
        // `new File(`, no `Response(..).blob()`, no three-argument `slice(`
        // (whose third argument is a content type); and the number of
        // `createObjectURL(` call sites is pinned, so a new one is reviewed.
        object_urls += text.matches("createObjectURL(").count();
        for forbidden in ["new File(", ".blob()", "new Response("] {
            assert!(
                !text.contains(forbidden),
                "{relative}: `{forbidden}` could create an untyped or re-typed Blob"
            );
        }
        for (at, _) in text.match_indices(".slice(") {
            let args = js_call_args(text, at + ".slice".len())
                .unwrap_or_else(|| panic!("{relative}: unterminated `.slice(` call"));
            assert!(
                args.len() <= 2,
                "{relative}: a three-argument `.slice(` can re-type a Blob: {args:?}"
            );
        }
        for (at, _) in text.match_indices("new Blob(") {
            let args = js_call_args(text, at + "new Blob".len())
                .unwrap_or_else(|| panic!("{relative}: unterminated `new Blob(` call"));
            assert_eq!(
                args.len(),
                2,
                "{relative}: a Blob takes its parts and a literal `{{ type }}`: {args:?}"
            );
            let options = args[1]
                .strip_prefix('{')
                .and_then(|o| o.strip_suffix('}'))
                .unwrap_or_else(|| panic!("{relative}: Blob options must be an object literal"))
                .trim();
            let expr = options
                .strip_prefix("type:")
                .unwrap_or_else(|| {
                    panic!("{relative}: Blob options must be exactly `{{ type: .. }}`: {options}")
                })
                .trim();
            let results = expr.split_once('?').map_or(expr, |(_, branches)| branches);
            for value in results.split(':').map(str::trim) {
                let literal = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .filter(|v| !v.contains('"'))
                    .unwrap_or_else(|| {
                        panic!("{relative}: a Blob type must be a string literal: {expr}")
                    });
                assert!(
                    [
                        "text/plain",
                        "text/csv",
                        "application/json",
                        "text/markdown;charset=utf-8",
                        "application/octet-stream",
                    ]
                    .contains(&literal),
                    "{relative}: Blob type `{literal}` is not a reviewed, non-scriptable type"
                );
            }
        }
        if relative != "src/main.tsx" {
            assert!(
                !text.contains("innerHTML ="),
                "{relative}: innerHTML assignment"
            );
        }
        for forbidden in ["insertAdjacentHTML", "outerHTML =", "document.write"] {
            assert!(!text.contains(forbidden), "{relative}: {forbidden}");
        }
        // P0 item D closures enforced across the frontend: no Monaco editor (it
        // injected a remote CDN script into the app origin), no collaboration
        // WebSocket (yjs / y-websocket), and no direct remote fetch carrying
        // secrets from the privileged origin. Imports are matched with all
        // whitespace removed, in every quote style, for static, side-effect,
        // re-export, dynamic `import(..)` and `require(..)` forms. (None of
        // these packages is a dependency in package.json.)
        // Comments (e.g. `/*@vite-ignore*/`) are removed before matching,
        // and the raw text is checked too. Every dynamic `import(..)` must
        // name one string literal (no concatenation, variable or comment),
        // except the one pinned dialog-plugin importer in KnowledgeGraph.tsx.
        for (at, _) in text.match_indices("import(") {
            let before = text[..at].chars().next_back().unwrap_or(' ');
            if before.is_ascii_alphanumeric() || before == '_' || before == '$' || before == '.' {
                continue;
            }
            let args = js_call_args(text, at + "import".len())
                .unwrap_or_else(|| panic!("{relative}: unterminated import("));
            let pinned = relative == "src/pages/KnowledgeGraph.tsx" && args == ["specifier"];
            assert!(
                pinned || (args.len() == 1 && is_js_string_literal(&args[0])),
                "{relative}: a dynamic import must name one string literal: {args:?}"
            );
        }
        let stripped: String = strip_js_comments(text).split_whitespace().collect();
        let raw: String = text.split_whitespace().collect();
        for module in [
            "@monaco-editor/",
            "monaco-editor",
            "yjs",
            "y-websocket",
            "y-protocols",
        ] {
            for quote in ['"', '\'', '`'] {
                for form in ["from", "import", "import(", "require("] {
                    let needle = format!("{form}{quote}{module}");
                    assert!(
                        !raw.contains(&needle) && !stripped.contains(&needle),
                        "{relative}: `{module}` is unavailable in Phase Zero ({needle})"
                    );
                }
            }
        }
        assert!(
            !text.contains("new WebsocketProvider("),
            "{relative}: the collaboration WebSocket is disabled in Phase Zero"
        );
        for remote_fetch in ["fetch(\"http", "fetch('http", "fetch(`http"] {
            assert!(
                !text.contains(remote_fetch),
                "{relative}: no direct remote fetch from the webview ({remote_fetch})"
            );
        }
        if text.contains("new Function(") {
            // The one dynamic import names a fixed module; the dialog plugin
            // is not installed, so it resolves to nothing.
            assert_eq!(relative, "src/pages/KnowledgeGraph.tsx");
            assert_eq!(text.matches("new Function(").count(), 1);
            assert!(text.contains("importer(\"@tauri-apps/plugin-dialog\")"));
        }
        // Links and window targets taken from data must be http(s).
        for (at, needle) in text.match_indices("href={") {
            let value = &text[at + needle.len()..];
            assert!(
                value.starts_with("safeHttpUrl(") || value.starts_with('"'),
                "{relative}: href from data without safeHttpUrl"
            );
        }
        for (at, needle) in text.match_indices("window.open(") {
            let target = &text[at + needle.len()..];
            assert!(
                target.starts_with("url,")
                    || (relative == "src/pages/AiChatHub.tsx"
                        && target.starts_with("`file://${indexPath}`")),
                "{relative}: window.open target not checked"
            );
        }
    }
    let expected: Vec<_> = FRONTEND_HTML_SINKS
        .iter()
        .map(|(file, count, _)| (file.to_string(), *count))
        .collect();
    assert_eq!(sinks, expected, "a raw-HTML sink must be classified");
    assert!(previews >= 6, "{previews}");
    assert_eq!(
        object_urls, 8,
        "the reviewed createObjectURL( call sites (all typed downloads); review any new one"
    );

    let source = |file: &str| {
        &files
            .iter()
            .find(|(relative, _)| relative == file)
            .unwrap_or_else(|| panic!("{file}"))
            .1
    };
    let chat = source("src/pages/AiChatHub.tsx");
    assert!(chat.contains("renderChatContent(content)"));
    assert!(!chat.contains("const highlightCode"));
    let notes = source("src/pages/NotesApp.tsx");
    assert!(notes.contains("renderNoteMarkdown(selectedNote.content)"));
    assert!(!notes.contains("function renderMarkdown"));
    let main = source("src/main.tsx");
    assert!(main.contains("${escapeHtml(e.message || \"Unknown error\")}"));
    let safe = source("src/lib/safeHtml.ts");
    for escaped in ["&amp;", "&lt;", "&gt;", "&quot;", "&#39;"] {
        assert!(safe.contains(escaped), "escapeHtml: {escaped}");
    }
    assert!(safe.contains("return escapeHtml(text)"));
    assert!(safe.contains("let html = escapeHtml(md)"));
}

/// Kernel capture APIs that the shared command-module import blocks still
/// name, unused, under `#![allow(unused_imports)]` (P0-002C5C). Outside a
/// `use` item they must not appear, and no `use` item may rename them.
const IMPORTED_CAPTURE_APIS: &[&str] = &[
    "capture_and_store_screen",
    "capture_and_analyze_screen",
    "analyze_stored_screenshot",
];

/// The `use` items of production text, and everything else.
fn use_items_and_code(text: &str) -> (String, String) {
    let (mut uses, mut code, mut in_use) = (String::new(), String::new(), false);
    for line in text.lines() {
        let trimmed = line.trim_start();
        let starts_use = ["use ", "pub use ", "pub(crate) use ", "pub(super) use "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix));
        if in_use || starts_use {
            uses.push_str(line);
            uses.push('\n');
            in_use = !line.contains(';');
        } else {
            code.push_str(line);
            code.push('\n');
        }
    }
    (uses, code)
}

/// Whitespace-free body of the first item whose text starts at `signature`.
fn body_after(src: &str, signature: &str) -> String {
    let at = src.find(signature).unwrap_or_else(|| panic!("{signature}"));
    let open = at + src[at..].find('{').unwrap();
    without_whitespace(&src[open + 1..block_end(src, open) - 1])
}

/// P0-002C5C (Architect decision): no unbrokered desktop IPC route captures
/// the screen, starts capture plus analysis, enables observation, or rearms
/// it after a disable or the emergency stop.
///
/// - The four capture routes are closed (`CLOSED_COMMANDS`).
/// - `computer_control_toggle` refuses its enabling branch before it reads or
///   changes state. Disabling, status, history, `stop_computer_action`, the
///   input status and nx readiness stay registered and available.
/// - No desktop production code does any of the following:
///   - calls a capture or vision API (the `LATENT_UNSAFE_APIS` needles; here,
///     the imported kernel names and every `capture_screen(` call);
///   - renames one of those APIs;
///   - enables the engine;
///   - clears the emergency stop.
/// - The emergency-stop shortcut still sets the kill switch and disables the
///   engine.
/// - Omniscience stays an in-memory placeholder:
///   - `start()` only sets a flag;
///   - `capture_context` only stores a context it is given;
///   - neither the kernel module nor the desktop wrapper reaches a capture
///     primitive, a process, a worker thread or the network.
///
///   Wiring one of these in fails here and needs a separately approved
///   mission.
#[test]
fn p0_002c5c_no_desktop_route_observes_the_screen() {
    for command in [
        "computer_control_capture_screen",
        "capture_screen",
        "analyze_screen",
        "nx_computer_use_screenshot",
    ] {
        assert!(
            CLOSED_COMMANDS.contains(&(command, Closure::ScreenObservation)),
            "{command} must stay closed as screen observation"
        );
    }
    let handlers = registered_handlers();
    for available in [
        "computer_control_toggle",
        "computer_control_status",
        "computer_control_get_history",
        "stop_computer_action",
        "get_input_control_status",
        "nx_computer_use_status",
    ] {
        let registered = handlers
            .iter()
            .filter(|entry| split_handler(entry).1 == available)
            .count();
        assert_eq!(registered, 1, "{available} must stay registered");
        assert!(
            !CLOSED_COMMANDS
                .iter()
                .any(|(command, _)| *command == available),
            "{available} must stay available"
        );
    }

    // The toggle refuses enabling before it reads or changes any state.
    let trust = production_text(include_str!("../commands/trust_security.rs"));
    let toggle = body_after(&trust, "fn computer_control_toggle(").replace(",)", ")");
    assert!(
        toggle.starts_with(concat!(
            "ifenabled{returnErr(crate::phase0_surface::closed(\"computer_control_toggle\",",
            "crate::phase0_surface::Closure::ScreenObservation));}",
        )),
        "{toggle}"
    );

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    production_sources(&root, &mut files);
    assert!(files.len() > 20, "desktop sources not found");
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = production_text(&std::fs::read_to_string(&path).unwrap());
        let (uses, code) = use_items_and_code(&text);
        for api in IMPORTED_CAPTURE_APIS {
            assert!(!names(&code, api), "{relative}: {api} is used");
            assert!(
                !uses.contains(&format!("{api} as")),
                "{relative}: {api} is renamed"
            );
        }
        // Every `capture_screen(` is a closed handler's definition or the
        // tail of a longer name, never a call of a capture API.
        for (at, _) in code.match_indices("capture_screen(") {
            let definition = code[..at].trim_end().ends_with("fn");
            let longer_name = code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            assert!(
                definition || longer_name,
                "{relative}: calls a capture_screen API"
            );
        }
        if code.contains(".computer_control") {
            assert!(
                !code.contains(".enable()"),
                "{relative}: enables the computer-control engine"
            );
        }
    }

    // The emergency stop sets the kill switch and disables the engine.
    let lib = production_text(LIB_RS);
    assert_eq!(lib.matches("activate_emergency_kill_switch();").count(), 1);
    let stop = lib.find("activate_emergency_kill_switch();").unwrap();
    let upto = lib[stop..]
        .find("log_event(")
        .expect("emergency stop audit");
    let stop_handler = without_whitespace(&lib[stop..stop + upto]);
    assert!(
        stop_handler.contains(".computer_control.lock()")
            && stop_handler.contains("engine.disable();"),
        "{stop_handler}"
    );

    // Omniscience is an in-memory placeholder, not screen observation.
    let screen = production_text(include_str!("../../../../kernel/src/omniscience/screen.rs"));
    assert_eq!(
        body_after(&screen, "pub fn start(&mut self)"),
        "self.active=true;"
    );
    assert_eq!(
        body_after(
            &screen,
            "pub fn capture_context(&mut self, context: ScreenContext)"
        ),
        "ifself.history.len()>=self.max_history{self.history.pop_front();}self.history.push_back(context);"
    );
    for (file, source) in [
        (
            "mod.rs",
            include_str!("../../../../kernel/src/omniscience/mod.rs"),
        ),
        (
            "screen.rs",
            include_str!("../../../../kernel/src/omniscience/screen.rs"),
        ),
        (
            "apps.rs",
            include_str!("../../../../kernel/src/omniscience/apps.rs"),
        ),
        (
            "executor.rs",
            include_str!("../../../../kernel/src/omniscience/executor.rs"),
        ),
        (
            "intent.rs",
            include_str!("../../../../kernel/src/omniscience/intent.rs"),
        ),
        (
            "assistant.rs",
            include_str!("../../../../kernel/src/omniscience/assistant.rs"),
        ),
    ] {
        let text = production_text(source);
        for forbidden in [
            "std::process",
            "Command::new",
            "spawn(",
            "std::thread",
            "tokio::",
            "std::net",
            "std::fs",
            "reqwest",
            "curl",
            "computer_control",
            "nexus_computer_use",
            "capture_screen",
            "take_screenshot",
            "screencapture",
        ] {
            assert!(!text.contains(forbidden), "omniscience/{file}: {forbidden}");
        }
    }
    let advanced = production_text(include_str!("../commands/advanced.rs"));
    assert_eq!(
        body_after(&advanced, "fn omniscience_enable(interval_ms: u64)"),
        "letmutscreen=omniscience_engine().lock().unwrap_or_else(|p|p.into_inner());screen.set_capture_interval_ms(interval_ms);screen.start();Ok(())"
    );
    assert_eq!(
        body_after(&advanced, "fn omniscience_get_screen_context()"),
        "letscreen=omniscience_engine().lock().unwrap_or_else(|p|p.into_inner());letcontext=screen.get_rolling_context(1);serde_json::to_value(&context).map_err(|e|e.to_string())"
    );
}
