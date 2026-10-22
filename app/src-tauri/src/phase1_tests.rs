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

/// The governed coding path, as (file name, module path, source): the
/// kernel's coding-run modules, the desktop flow and its sandboxed
/// verification. Test files are not production.
fn coding_path_sources() -> Vec<(String, Vec<String>, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: std::path::PathBuf| std::fs::read_to_string(path).unwrap();
    let module = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    let kernel = root.join("../../kernel/src");
    let mut out = vec![(
        "coding_run.rs".to_string(),
        module(&["crate", "coding_run"]),
        read(kernel.join("coding_run.rs")),
    )];
    let mut names: Vec<String> = std::fs::read_dir(kernel.join("coding_run"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".rs") && !name.ends_with("tests.rs"))
        .collect();
    names.sort();
    for name in names {
        let stem = name.trim_end_matches(".rs").to_string();
        let text = read(kernel.join("coding_run").join(&name));
        out.push((name, module(&["crate", "coding_run", &stem]), text));
    }
    out.push((
        "coding_flow.rs".to_string(),
        module(&["crate", "coding_flow"]),
        read(root.join("src/coding_flow.rs")),
    ));
    out.push((
        "verification.rs".to_string(),
        module(&["crate", "coding_flow", "verification"]),
        read(root.join("src/coding_flow/verification.rs")),
    ));
    out
}

/// std modules the coding path may name: none can run a process or open a
/// connection (`std::process`, `std::net`, `std::env` and the platform
/// process and socket extensions are outside).
const CODING_PATH_STD: &[&str] = &[
    "any",
    "array",
    "ascii",
    "borrow",
    "boxed",
    "cell",
    "char",
    "clone",
    "cmp",
    "collections",
    "convert",
    "default",
    "error",
    "ffi",
    "fmt",
    "fs",
    "hash",
    "hint",
    "io",
    "iter",
    "marker",
    "mem",
    "num",
    "ops",
    "option",
    "os::fd",
    "os::unix::ffi",
    "os::unix::fs",
    "os::unix::io",
    "panic",
    "path",
    "rc",
    "result",
    "slice",
    "str",
    "string",
    "sync",
    "thread",
    "time",
    "vec",
];

/// The rest of the coding path's closed world: (path, whether everything
/// below it is included too). The kernel's coding-run modules:
const CODING_PATH_KERNEL: &[(&str, bool)] = &[
    ("crate::coding_run", true),
    ("crate::manifest::FsPermissionLevel", true),
    ("crate::workspace_authority", true),
    ("crate::identity_home::identity_home", false),
    ("hex", true),
    ("serde", true),
    ("serde_json", true),
    ("sha2", true),
    ("thiserror", true),
    ("uuid", true),
    ("nix::fcntl::renameat2", false),
    ("nix::fcntl::RenameFlags", true),
    ("nix::libc", false),
    ("nix::libc::O_CLOEXEC", false),
    ("nix::libc::O_DIRECTORY", false),
    ("nix::libc::O_NOFOLLOW", false),
    ("nix::libc::O_NONBLOCK", false),
    ("nexus_persistence::coding_run_ledger", true),
];

/// ... the loopback-only local model client alone, besides:
const CODING_PATH_LOCAL_MODEL: &[(&str, bool)] = &[
    ("reqwest", true),
    ("url", true),
    ("std::net::IpAddr", true),
    ("std::net::Ipv4Addr", true),
    ("crate::governed_http::http_url", false),
];

/// ... and the desktop flow and its verification, the verifier sandbox item
/// by item: its one execution route and the backend objects verification
/// hands it (the installed helper and toolchain only).
const CODING_PATH_DESKTOP: &[(&str, bool)] = &[
    ("crate::coding_flow", true),
    ("crate::AppState", false),
    (
        "crate::commands::chat_llm::authorized_ollama_base_url",
        false,
    ),
    ("hex", true),
    ("serde", true),
    ("uuid", true),
    ("nexus_kernel::coding_run", true),
    ("nexus_kernel::identity_home::nexus_state_dir", false),
    ("nexus_kernel::identity_home::nexus_state_path", false),
    (
        "nexus_kernel::workspace_authority::WorkspaceAuthorityRegistry",
        false,
    ),
    ("nexus_kernel::workspace_authority::WorkspaceBinding", false),
    ("nexus_persistence::coding_run_ledger", true),
    ("tauri::AppHandle", false),
    ("tauri::Wry", false),
    ("tauri::async_runtime::spawn_blocking", false),
    ("tauri_plugin_dialog::DialogExt", false),
    ("tauri_plugin_dialog::MessageDialogButtons", true),
    ("tauri_plugin_dialog::MessageDialogKind", true),
    ("nexus_verifier_sandbox::applicability", false),
    ("nexus_verifier_sandbox::applicability::check", false),
    ("nexus_verifier_sandbox::execution", false),
    ("nexus_verifier_sandbox::execution::run", false),
    ("nexus_verifier_sandbox::execution::Cleanup", false),
    ("nexus_verifier_sandbox::execution::ExecutionReport", false),
    ("nexus_verifier_sandbox::execution::ExitClass", true),
    ("nexus_verifier_sandbox::execution::RetainedBoundary", false),
    (
        "nexus_verifier_sandbox::execution::RetainedBoundary::retry",
        false,
    ),
    ("nexus_verifier_sandbox::execution::StreamRecord", false),
    (
        "nexus_verifier_sandbox::execution::StreamRecord::default",
        false,
    ),
    ("nexus_verifier_sandbox::launcher::HelperProgram", false),
    (
        "nexus_verifier_sandbox::launcher::HelperProgram::installed",
        false,
    ),
    ("nexus_verifier_sandbox::policy::SandboxPolicy", false),
    ("nexus_verifier_sandbox::policy::SandboxPolicy::V1", false),
    ("nexus_verifier_sandbox::profile::VerifierProfile", false),
    ("nexus_verifier_sandbox::profile::VerifierProfileId", true),
    ("nexus_verifier_sandbox::profile_launch::launch_spec", false),
    (
        "nexus_verifier_sandbox::profile_launch::LaunchSetupError",
        true,
    ),
    ("nexus_verifier_sandbox::scope::ScopeManager", false),
    (
        "nexus_verifier_sandbox::scope::ScopeManager::connect",
        false,
    ),
    (
        "nexus_verifier_sandbox::toolchain::VerifiedVerifierToolchain",
        false,
    ),
    (
        "nexus_verifier_sandbox::toolchain::VerifiedVerifierToolchain::installed",
        false,
    ),
    ("nexus_verifier_sandbox::workspace::Area", true),
    (
        "nexus_verifier_sandbox::workspace::RetainedWorkspace",
        false,
    ),
    ("nexus_verifier_sandbox::workspace::Workspace", false),
    (
        "nexus_verifier_sandbox::workspace::Workspace::create",
        false,
    ),
    ("nexus_verifier_sandbox::workspace::WorkspaceRoot", false),
    (
        "nexus_verifier_sandbox::workspace::WorkspaceRoot::derive",
        false,
    ),
    ("nexus_verifier_sandbox::workspace::workspaces_path", false),
];

const PRIMITIVES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
    "i128", "isize", "f32", "f64",
];

/// Violations of the coding path's closed world in `sources`.
fn coding_path_violations(sources: &[(String, Vec<String>, String)]) -> Vec<String> {
    use crate::phase0_surface::rust_paths::{
        constructs_process, ends_process, opens_network, starts_with, Analysis,
    };
    let within = |path: &[String], allowed: &[(&str, bool)]| {
        allowed.iter().any(|(entry, below)| {
            let depth = entry.split("::").count();
            starts_with(path, entry) && (*below || path.len() == depth)
        })
    };
    let mut out = Vec::new();
    for (name, module, text) in sources {
        let kernel = module.get(1).is_some_and(|m| m == "coding_run");
        let local_model = name == "local_model.rs";
        let module: Vec<&str> = module.iter().map(String::as_str).collect();
        let analysis = Analysis::new(text, &module);
        for o in analysis.production() {
            // A lone unbound name is a local (a variable, a function, a type);
            // every other path, and every declaration, is judged.
            let lone = o.declaration.is_none()
                && !o.written.abs
                && o.written.segs.len() == 1
                && !analysis.binds(&o.written.segs[0]);
            let tool = o.attribute
                && ["clippy", "rustfmt", "rustdoc"].contains(&o.written.segs[0].as_str());
            if lone || tool {
                continue;
            }
            for path in &o.resolved {
                let shown = path.join("::");
                let at = format!(
                    "{name}:{} {}",
                    o.line,
                    o.item.as_deref().unwrap_or("<module>")
                );
                if constructs_process(path) || ends_process(path) {
                    out.push(format!("{at}: runs or signals a process ({shown})"));
                }
                if opens_network(path) && !(local_model && starts_with(path, "reqwest")) {
                    out.push(format!("{at}: opens a connection ({shown})"));
                }
                if ["git2", "gix"].iter().any(|git| starts_with(path, git)) {
                    out.push(format!("{at}: uses git ({shown})"));
                }
                let root = path[0].as_str();
                let allowed = root.starts_with(|c: char| c.is_ascii_uppercase())
                    || (PRIMITIVES.contains(&root) && path.len() > 1)
                    || (matches!(root, "std" | "core" | "alloc")
                        && (root != "std"
                            || CODING_PATH_STD.iter().any(|m| starts_with(&path[1..], m))))
                    || within(
                        path,
                        if kernel {
                            CODING_PATH_KERNEL
                        } else {
                            CODING_PATH_DESKTOP
                        },
                    )
                    || (local_model && within(path, CODING_PATH_LOCAL_MODEL));
                if !allowed {
                    out.push(format!(
                        "{at}: outside the coding path's closed world ({shown})"
                    ));
                }
            }
        }
    }
    out
}

/// The governed coding path's closed world (XA-L-04). Every path its sources
/// name (the kernel's coding-run modules, the desktop flow and its sandboxed
/// verification) is resolved structurally through every import spelling
/// (aliases, grouped and nested imports, globs, `extern crate`, type
/// aliases) and must stay inside an explicit set: the coding path's own
/// modules, the kernel trust modules it builds on, the persistence ledger,
/// the verifier sandbox items verification uses, the native dialog, and std
/// modules that can neither run a process nor open a connection. Within it
/// nothing constructs, replaces or signals a process, opens a socket or uses
/// git, and HTTP and the address types appear only in the loopback-only
/// local model client. A process spawn spelled through any alias therefore
/// fails here in whichever coding-path module it is added; the negative
/// controls inject the XA-001 K10c mutant and its variants into the real
/// sources.
#[test]
fn p1_g_13_the_coding_path_is_a_closed_world_under_every_import_spelling() {
    let real = coding_path_sources();
    assert!(real.len() >= 13, "coding sources not found");
    assert_eq!(coding_path_violations(&real), Vec::<String>::new());

    let k10c = "\nuse std::{process::Command as Spawn};\n#[allow(dead_code)]\nfn xa_mutant_shell(text: &str) {\n    let _ = Spawn::new(\"sh\").arg(\"-c\").arg(text).status();\n}\n";
    let with = |file: &str, text: &str| -> Vec<(String, Vec<String>, String)> {
        assert!(
            real.iter().any(|(name, _, _)| name == file),
            "{file} not found"
        );
        real.iter()
            .map(|(name, module, source)| {
                let source = if name == file {
                    format!("{source}\n{text}\n")
                } else {
                    source.clone()
                };
                (name.clone(), module.clone(), source)
            })
            .collect()
    };
    for (file, text) in [
        ("review.rs", k10c),
        ("worker.rs", k10c),
        ("verification.rs", k10c),
        ("coding_flow.rs", "use std as s;\nfn f() { let _ = s::process::Command::new(\"git\"); }"),
        ("apply.rs", "extern crate std as q;\nfn f() { let _ = q::process::Command::new(\"sh\"); }"),
        ("project.rs", "type Tool = std::process::Command;\nfn f() { let _ = Tool::new(\"sh\"); }"),
        ("ledger.rs", "fn f() { use std::process::*; let _ = Command::new(\"sh\"); }"),
        ("fsops.rs", "fn f() { let _ = unsafe { libc::fork() }; }"),
        ("structural.rs", "fn f() { let _ = std::net::TcpStream::connect(\"192.0.2.1:80\"); }"),
        ("local_model.rs", "fn f() { let _ = std::net::TcpStream::connect(\"127.0.0.1:1\"); }"),
        ("scope.rs", "fn f() { let _ = std::env::var(\"PATH\"); }"),
        ("coding_run.rs", "fn f() { let _ = duct::cmd!(\"sh\"); }"),
        ("verifier.rs", "fn f() { crate::actuators::shell::run(\"sh\"); }"),
        ("coding_flow.rs", "fn f() { let _ = nexus_kernel::actuators::shell::run(\"sh\"); }"),
        ("verification.rs", "fn f() { let _ = nexus_verifier_sandbox::launcher::HelperProgram::at(\"/tmp/h\"); }"),
        ("verification.rs", "fn f() { let _ = nexus_verifier_sandbox::toolchain::VerifiedVerifierToolchain::development(p); }"),
        ("manifest.rs", "fn f() { let _ = git2::Repository::open(\".\"); }"),
        ("worker.rs", "use nix::unistd as u;\nfn f() { let _ = unsafe { u::fork() }; }"),
    ] {
        assert!(
            !coding_path_violations(&with(file, text)).is_empty(),
            "the negative control was not detected in {file}: {text:?}"
        );
    }
    // Test-only code is not production.
    assert_eq!(
        coding_path_violations(&with(
            "review.rs",
            "#[cfg(test)]\nmod more_tests { fn f() { let _ = std::process::Command::new(\"sh\"); } }",
        )),
        Vec::<String>::new()
    );
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
