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
    ("memory_save", Closure::AmbientResource),
    ("memory_load", Closure::AmbientResource),
    ("memory_list_agents", Closure::AmbientResource),
    ("mcp2_server_handle", Closure::AmbientResource),
    // C5C: OS keyboard and mouse input from the interface or a model.
    ("computer_control_execute_action", Closure::OsInput),
    ("start_computer_action", Closure::OsInput),
    // C5C: a raw output path for the browser bridge (never started) to write.
    ("browser_screenshot", Closure::FileSelection),
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
            builder_check_cli_auth, builder_authenticate_cli, self_rewrite_analyze,
            self_rewrite_test_patch, self_rewrite_rollback, genesis_analyze_gap,
            genesis_preview_agent, genesis_create_agent, genesis_store_pattern,
            genesis_list_generated, genesis_delete_agent, get_agent_genome, mutate_agent,
            breed_agents, get_agent_lineage, generate_all_genomes, evolve_population,
            force_evolve_agent, trigger_immune_scan, get_git_repo_status,
            voice_start_listening, voice_pipeline_health, transcribe_push_to_talk,
            computer_control_execute_action, start_computer_action,
        ],
        crate::commands::flash => [
            flash_profile_model, flash_auto_configure, flash_create_session,
            flash_estimate_performance, flash_run_benchmark, flash_enable_speculative,
        ],
        crate::commands::crate_bridges => [
            cc_execute_action, mcp2_client_add, mcp2_client_discover, mcp2_client_call,
            cm_execute_validation_run, cm_list_validation_runs, cm_get_validation_run,
            cm_three_way_comparison, memory_save, memory_load, memory_list_agents,
            mcp2_server_handle, browser_screenshot,
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
        Closure::AmbientResource,
        Closure::OsInput,
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
    // holds none for Time Machine, so it replays agent state and config only.
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
    ("app/src-tauri/src/commands/apps.rs", 1),
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
        "final gate: configuration key derivation input",
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
        "nexus-code/src/main.rs",
        "\"/tmp",
        2,
        "the nx terminal binary, outside the desktop",
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
            3,
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
        "save_config_with(state,config,load_current_security_baseline,save_nexus_config,)"
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
    // Restored agent records pass the stored-manifest authority check.
    let agents = without_whitespace(&production_text(include_str!("../commands/agents.rs")));
    assert!(agents
        .contains("ifletErr(error)=nexus_kernel::manifest::validate_stored_manifest(&manifest){"));
}

/// Benchmark-only process sites (P0-002C5C): ungoverned curl and `date`
/// invocations in the conductor benchmark binaries. They are outside the
/// production guards (`NOT_PRODUCTION_DIRS`), no other member depends on a
/// benchmark crate, and the installers ship only the desktop.
const BENCHMARK_PROCESS_SITES: &[(&str, usize)] = &[
    ("benchmarks/conductor-bench/src/cloud_models_bench.rs", 2),
    (
        "benchmarks/conductor-bench/src/inference_consistency_bench.rs",
        2,
    ),
    ("benchmarks/conductor-bench/src/local_vs_cloud_battle.rs", 3),
    ("benchmarks/conductor-bench/src/nim_cloud_bench.rs", 2),
    ("benchmarks/conductor-bench/src/real_agent_validation.rs", 1),
];

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
        source.contains(&format!("#[test]\nfn {name}()"))
            || source.contains(&format!("#[test]\n    fn {name}()"))
    };
    let nexus_code = include_str!("../../../../nexus-code/tests/phase0_desktop_config.rs");
    let kernel_loop = include_str!("../../../../kernel/src/cognitive/loop_runtime.rs");
    let identity = include_str!("../../../../kernel/src/identity_home.rs");
    let lib_tests = include_str!("../lib_tests.rs");
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
    ] {
        for guard in guards {
            assert!(
                is_test(source, guard),
                "{regression}: guard {guard} is missing"
            );
        }
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
    for (relative, text) in &files {
        let count = text.matches("dangerouslySetInnerHTML={").count();
        if count > 0 {
            sinks.push((relative.clone(), count));
        }
        for (at, _) in text.match_indices("srcDoc=") {
            let open = text[..at]
                .rfind("<iframe")
                .expect("srcDoc outside an iframe");
            let close = at + text[at..].find("/>").expect("iframe element end");
            let element = &text[open..close];
            let sandbox = element
                .split("sandbox=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or_else(|| panic!("{relative}: srcDoc iframe without sandbox"));
            assert!(
                !(sandbox.contains("allow-scripts") && sandbox.contains("allow-same-origin")),
                "{relative}: sandboxed preview may run script with the app's origin"
            );
            previews += 1;
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
