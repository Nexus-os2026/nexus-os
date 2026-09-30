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

const CODING_COMMANDS: [&str; 9] = [
    "coding_select_project",
    "coding_list_projects",
    "coding_list_local_models",
    "coding_start_run",
    "coding_status",
    "coding_list_runs",
    "coding_approve_apply",
    "coding_restore_run",
    "coding_discard_run",
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
                    "run_id"
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
