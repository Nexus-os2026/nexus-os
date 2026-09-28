//! P0-FINAL-GATE-CLOSURE guards for item D (privileged webview, navigation and
//! IPC origin).
//!
//! This file's name ends in `tests.rs`, so the `phase0_surface` production
//! scanners skip it; it may name guarded spellings freely.
//!
//! These guards keep the two native controls honest:
//!
//! * the app ACL manifest lists *every* registered application command, so the
//!   moment `has_app_manifest` is true no registered command is accidentally
//!   left unreachable, and none is granted that is not registered;
//! * the app-command capability grants those commands to window `main` at the
//!   **local** origin only — never a remote URL — so origin enforcement cannot
//!   be silently widened;
//! * `build.rs` really feeds the list into the manifest, `tauri.conf.json`
//!   carries a non-null CSP and does not auto-create the unguarded window, and
//!   the navigation predicate refuses non-app origins.

use crate::webview_boundary::{navigation_allowed_parts, APP_COMMANDS};

const LIB_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
const APP_CAPABILITY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/capabilities/app-commands.json"
));
const BUILD_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
const TAURI_CONF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"));
const BOUNDARY_RS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/webview_boundary.rs"
));

/// The command names in the `generate_handler![..]` registry, read the same way
/// the C5 reachability guard reads them: strip line comments, split on commas,
/// take the final `::` segment.
fn registered_command_names() -> Vec<String> {
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
        .map(|entry| {
            entry
                .trim_start_matches("crate::")
                .rsplit("::")
                .next()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The app manifest command list must equal the live registry exactly, so it
/// can never silently drift: a missing command would make that command
/// unreachable once the manifest is enforced, and an extra one would grant a
/// command that does not exist.
#[test]
fn p0_fg_webview_app_manifest_lists_every_registered_command() {
    let mut registered = registered_command_names();
    registered.sort();
    registered.dedup();

    // APP_COMMANDS is the sole source of truth shared with build.rs.
    let listed: Vec<String> = APP_COMMANDS.iter().map(|s| s.to_string()).collect();
    // It must already be sorted and unique (deterministic generation).
    let mut sorted = listed.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(listed, sorted, "APP_COMMANDS must be sorted and unique");

    assert_eq!(
        listed, registered,
        "APP_COMMANDS must equal the generate_handler! registry exactly; \
         regenerate src/webview_boundary/app_commands.rs and capabilities/app-commands.json"
    );
    // Matches the recorded desktop surface (inventory: 804 registered commands).
    assert_eq!(APP_COMMANDS.len(), 804, "registered command count changed");
}

/// build.rs must actually feed APP_COMMANDS into the app manifest, or the ACL
/// is never emitted and app commands stay ungoverned.
#[test]
fn p0_fg_webview_build_script_emits_the_app_manifest() {
    assert!(
        BUILD_RS.contains("AppManifest::new().commands(app_commands::APP_COMMANDS)"),
        "build.rs must pass APP_COMMANDS to tauri_build::AppManifest::commands"
    );
    assert!(
        BUILD_RS.contains("try_build"),
        "build.rs must call tauri_build::try_build with the app manifest"
    );
}

/// The app-command capability must grant every command — and only those — to
/// window `main` at the local origin, with no remote URL and no platform
/// narrowing.
#[test]
fn p0_fg_webview_capability_is_local_main_only() {
    let cap: serde_json::Value =
        serde_json::from_str(APP_CAPABILITY).expect("app-commands.json parses");

    assert_eq!(cap["identifier"], "app-commands");
    // local defaults to true in Tauri; we set it explicitly and require it.
    assert_eq!(cap["local"], serde_json::Value::Bool(true), "must be local");
    assert!(
        cap.get("remote").is_none(),
        "capability must grant no remote URL"
    );
    assert!(
        cap.get("platforms").is_none(),
        "capability must apply to every platform"
    );
    assert_eq!(
        cap["windows"],
        serde_json::json!(["main"]),
        "commands are granted to the main window only"
    );
    assert!(cap.get("webviews").is_none(), "no per-webview widening");

    let perms: Vec<String> = cap["permissions"]
        .as_array()
        .expect("permissions array")
        .iter()
        .map(|p| p.as_str().expect("permission is a string").to_string())
        .collect();

    // Exactly one allow-<slug> per registered command, and nothing else.
    let mut expected: Vec<String> = APP_COMMANDS
        .iter()
        .map(|c| format!("allow-{}", c.replace('_', "-")))
        .collect();
    expected.sort();
    let mut got = perms.clone();
    got.sort();
    assert_eq!(
        got, expected,
        "capability must allow exactly the registered commands (allow-<slug>)"
    );
    // No plugin-prefixed or wildcard entries slipping in.
    for p in &perms {
        assert!(
            p.starts_with("allow-") && !p.contains(':') && !p.contains('*'),
            "unexpected permission entry: {p}"
        );
    }
}

/// The CSP is a real, restrictive policy (defence in depth), and the privileged
/// window is not auto-created without the navigation/new-window guards.
#[test]
fn p0_fg_webview_conf_has_restrictive_csp_and_guarded_window() {
    let conf: serde_json::Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json parses");
    let csp = &conf["app"]["security"]["csp"];
    let csp = csp.as_str().expect("csp must be a string, not null");
    for directive in [
        "default-src 'self'",
        "script-src 'self'",
        "connect-src 'self' ipc:",
        "frame-src 'self'",
        "object-src 'none'",
        "base-uri 'self'",
        "frame-ancestors 'none'",
    ] {
        assert!(csp.contains(directive), "CSP missing `{directive}`");
    }
    // No wildcard script or remote-frame sources, and no eval.
    assert!(
        !csp.contains("script-src 'self' http"),
        "script-src must stay 'self'"
    );
    assert!(!csp.contains("unsafe-eval"), "no unsafe-eval");

    let windows = conf["app"]["windows"].as_array().expect("windows array");
    let main = windows
        .iter()
        .find(|w| w["label"] == "main")
        .expect("a window labelled main");
    assert_eq!(
        main["create"],
        serde_json::Value::Bool(false),
        "the main window must not be auto-created; it is built with its boundary in setup()"
    );
}

/// The navigation predicate: only the app's own origin (and internal
/// blank/sandboxed documents) may load; every remote origin is refused, and the
/// loopback dev origin only in debug builds.
#[test]
fn p0_fg_webview_navigation_predicate_refuses_non_app_origins() {
    // Allowed — the app document and internal/sandboxed documents.
    assert!(navigation_allowed_parts("tauri", Some("localhost"), false));
    assert!(navigation_allowed_parts(
        "http",
        Some("tauri.localhost"),
        false
    ));
    assert!(navigation_allowed_parts(
        "https",
        Some("tauri.localhost"),
        false
    ));
    assert!(navigation_allowed_parts("about", None, false)); // about:blank / about:srcdoc
    assert!(navigation_allowed_parts("data", None, false));
    assert!(navigation_allowed_parts("blob", None, false));

    // Dev server origin: debug only.
    assert!(navigation_allowed_parts("http", Some("localhost"), true));
    assert!(navigation_allowed_parts("http", Some("127.0.0.1"), true));
    assert!(!navigation_allowed_parts("http", Some("localhost"), false));
    assert!(!navigation_allowed_parts("http", Some("127.0.0.1"), false));

    // Refused — remote origins and look-alikes, on every build.
    for dev in [true, false] {
        assert!(!navigation_allowed_parts(
            "https",
            Some("evil.example"),
            dev
        ));
        assert!(!navigation_allowed_parts("http", Some("evil.example"), dev));
        assert!(!navigation_allowed_parts(
            "https",
            Some("tauri.localhost.evil.example"),
            dev
        ));
        assert!(!navigation_allowed_parts(
            "https",
            Some("evil.tauri.localhost"),
            dev
        ));
        assert!(!navigation_allowed_parts("file", None, dev));
        assert!(!navigation_allowed_parts("javascript", None, dev));
    }
}

/// The privileged window must be built with both boundary handlers wired: the
/// navigation guard (the pure predicate, with the dev flag from the build
/// profile) and an unconditional new-window denial that covers every platform
/// (Linux included). This checks the wiring; the pure predicate is covered by
/// the test above, and wry's cancel/deny behaviour is verified from the pinned
/// wry 0.54.4 source.
#[test]
fn p0_fg_webview_main_window_wires_navigation_and_newwindow_guards() {
    assert!(
        BOUNDARY_RS.contains("WebviewWindowBuilder::from_config(app.handle(), &config)"),
        "the main window must be built from its config in setup()"
    );
    assert!(
        BOUNDARY_RS.contains(".on_navigation(navigation_allowed)"),
        "the main window must install the navigation guard"
    );
    assert!(
        BOUNDARY_RS.contains(".on_new_window(|_url, _features| NewWindowResponse::Deny)"),
        "the main window must deny every new window on every platform"
    );
    assert!(
        BOUNDARY_RS.contains(
            "navigation_allowed_parts(url.scheme(), url.host_str(), cfg!(debug_assertions))"
        ),
        "the navigation guard must delegate to the pure predicate with the build-profile dev flag"
    );
}

/// Native origin enforcement (N3), on the real ACL that tauri-build embedded
/// into this crate. Every application command is invocable only from the local
/// app origin on the `main` window; a remote origin, a loopback origin (a
/// navigated page or a cross-origin/loopback subframe) and any other
/// window/webview are refused — the same `resolve_access` call the IPC layer
/// makes on every request, so this holds even when the caller presents a valid
/// invoke key. Deterministic and headless; no window or event loop is created.
#[cfg(feature = "tauri-runtime")]
#[test]
fn p0_fg_webview_app_command_ipc_is_local_main_only() {
    use tauri::ipc::Origin;

    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let authority = ctx.runtime_authority_mut();

    let remote = Origin::Remote {
        url: "https://evil.example/".parse().unwrap(),
    };
    let loopback = Origin::Remote {
        url: "http://127.0.0.1:15173/".parse().unwrap(),
    };

    // A representative spread of the registry, including a closed command
    // (`capture_screen`) and an approval command (`swarm_approve`): the ACL
    // grants all of them at the local origin, and the handler/closure decides
    // the rest. If any registered command were missing from the manifest it
    // would be refused here even at the local origin.
    for cmd in [
        "list_agents",
        "send_chat",
        "capture_screen",
        "swarm_approve",
        "api_client_request",
        "workspace_usage",
    ] {
        // Allowed: local origin, main window/webview.
        assert!(
            authority
                .resolve_access(cmd, "main", "main", &Origin::Local)
                .is_some(),
            "{cmd} must be invocable from the local app origin on the main window"
        );
        // Refused: a navigated remote page.
        assert!(
            authority
                .resolve_access(cmd, "main", "main", &remote)
                .is_none(),
            "{cmd} must be refused from a remote origin (navigated page)"
        );
        // Refused: a cross-origin / loopback subframe.
        assert!(
            authority
                .resolve_access(cmd, "main", "main", &loopback)
                .is_none(),
            "{cmd} must be refused from a loopback/subframe origin"
        );
        // Refused: any other window/webview label, even at the local origin.
        assert!(
            authority
                .resolve_access(cmd, "other", "other", &Origin::Local)
                .is_none(),
            "{cmd} must be granted to the main window/webview only"
        );
    }
}
