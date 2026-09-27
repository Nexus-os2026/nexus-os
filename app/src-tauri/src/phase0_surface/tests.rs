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
];

const LIB_RS: &str = include_str!("../lib.rs");

/// Source of the module that defines a registered handler (`""` is the
/// `runtime` module in lib.rs).
fn module_source(module: &str) -> &'static str {
    match module {
        "" => LIB_RS,
        "commands::flash" => include_str!("../commands/flash.rs"),
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
            builder_deploy_share_info, builder_deploy_drift,
        ],
        crate::commands::flash => [
            flash_profile_model, flash_auto_configure, flash_create_session,
            flash_estimate_performance, flash_run_benchmark, flash_enable_speculative,
        ],
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
    for closure in [Closure::FileSelection, Closure::LegacyBuilder] {
        let reason = closure.reason();
        assert!(reason.contains("Phase Zero"), "{reason}");
        assert!(reason.len() <= 160, "{reason}");
        assert!(!reason.contains('/') && !reason.contains('\\'), "{reason}");
        assert_eq!(closed("surface", closure), format!("surface: {reason}"));
    }
}
