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

use crate::webview_boundary::APP_COMMANDS;

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

/// The app origin the navigation guard admits is resolved exactly as tauri
/// 2.10.3 resolves the app window's URL: the configured `devUrl` only in dev
/// mode (`tauri::is_dev()`, tauri's `cfg(dev)`), otherwise the `tauri`
/// protocol origin, which is `tauri://localhost` on Linux/macOS and
/// `http://tauri.localhost` (`https://` with `useHttpsScheme`) on Windows. Every
/// platform's value is checked on every platform.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p0_fg_webview_app_origin_is_resolved_like_tauri_resolves_the_app_url() {
    use crate::webview_boundary::AppOrigin;
    use tauri::utils::config::FrontendDist;
    use tauri::Url;

    let ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let config = ctx.config().clone();
    let main = config
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .expect("main window config");
    let origin = |u: &str| AppOrigin::of(&Url::parse(u).unwrap()).unwrap();

    // Dev mode: the configured devUrl, on every platform.
    for windows_os in [false, true] {
        assert_eq!(
            AppOrigin::resolve(&config, &main, true, windows_os),
            Some(origin("http://localhost:1420"))
        );
    }
    // Production: the tauri protocol origin for the platform.
    assert_eq!(
        AppOrigin::resolve(&config, &main, false, false),
        Some(origin("tauri://localhost"))
    );
    assert_eq!(
        AppOrigin::resolve(&config, &main, false, true),
        Some(origin("http://tauri.localhost"))
    );
    let mut https_main = main.clone();
    https_main.use_https_scheme = true;
    assert_eq!(
        AppOrigin::resolve(&config, &https_main, false, true),
        Some(origin("https://tauri.localhost"))
    );
    // useHttpsScheme only changes the Windows form.
    assert_eq!(
        AppOrigin::resolve(&config, &https_main, false, false),
        Some(origin("tauri://localhost"))
    );
    // Dev mode without a devUrl falls back to the protocol origin.
    let mut no_dev_url = config.clone();
    no_dev_url.build.dev_url = None;
    assert_eq!(
        AppOrigin::resolve(&no_dev_url, &main, true, false),
        Some(origin("tauri://localhost"))
    );
    assert_eq!(
        AppOrigin::resolve(&no_dev_url, &main, true, true),
        Some(origin("http://tauri.localhost"))
    );
    // A URL frontendDist would be the production app origin (tauri loads it);
    // this app's frontendDist is a local directory (pinned by the conf guard).
    let mut url_dist = config.clone();
    url_dist.build.frontend_dist = Some(FrontendDist::Url(
        Url::parse("https://frontend.example/").unwrap(),
    ));
    assert_eq!(
        AppOrigin::resolve(&url_dist, &main, false, false),
        Some(origin("https://frontend.example"))
    );
    assert_eq!(
        AppOrigin::resolve(&url_dist, &main, true, false),
        Some(origin("http://localhost:1420")),
        "in dev mode tauri loads devUrl, not frontendDist"
    );
}

/// The navigation guard admits exactly the app origin (scheme, host and port,
/// no userinfo), `about:blank`, `about:srcdoc`, and `blob:` URLs created by the
/// app origin — for each origin the app can run at — and refuses every
/// scheme, host, port, userinfo, case and trailing-dot variant, `data:`, and
/// the other profile's origin.
#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[test]
fn p0_fg_webview_navigation_admits_only_the_exact_app_origin() {
    use crate::webview_boundary::{navigation_allowed, AppOrigin};
    use tauri::Url;

    fn check(app: &str, admitted: &[&str], refused: &[&str]) {
        let app_origin = AppOrigin::of(&Url::parse(app).unwrap()).unwrap();
        for u in admitted {
            let url = Url::parse(u).unwrap_or_else(|e| panic!("{u}: {e}"));
            assert!(
                navigation_allowed(&url, &app_origin),
                "app origin {app}: `{u}` must be admitted"
            );
        }
        for u in refused {
            let url = Url::parse(u).unwrap_or_else(|e| panic!("{u}: {e}"));
            assert!(
                !navigation_allowed(&url, &app_origin),
                "app origin {app}: `{u}` must be refused"
            );
        }
    }

    // Refused whatever the app origin is.
    let always_refused = [
        "https://evil.example/",
        "http://evil.example/",
        "http://[::1]:1421/",
        "http://127.0.0.1:1420/",
        "http://[::1]:1420/",
        "http://asset.localhost/",
        "http://ipc.localhost/list_agents",
        "http://evil.tauri.localhost/",
        "https://tauri.localhost.evil.example/",
        "asset://localhost/etc/passwd",
        "ipc://localhost/list_agents",
        "file:///etc/passwd",
        "data:text/html,<script>1</script>",
        "data:text/plain,hi",
        "javascript:alert(1)",
        "about:config",
        "about:blank?x=1",
        "about:blank#top",
        "about:srcdoc#frag",
        "blob:null/0b0e7a8e-0000-4000-8000-000000000000",
        "blob:http://[::1]:1421/0b0e7a8e-0000-4000-8000-000000000000",
        "blob:https://evil.example/0b0e7a8e-0000-4000-8000-000000000000",
        "ws://localhost:1420/",
        "ftp://localhost/",
    ];
    let internal = ["about:blank", "about:srcdoc"];

    // Linux / macOS production origin.
    let mut refused = always_refused.to_vec();
    refused.extend([
        "tauri://localhost:1420/",
        "tauri://localhost:80/",
        "tauri://evil/",
        "tauri://localhost./",
        "tauri://LOCALHOST/",
        "tauri://user@localhost/",
        "tauri://user:pw@localhost/",
        "http://tauri.localhost/",
        "https://tauri.localhost/",
        "http://localhost:1420/",
        "http://localhost/",
        "blob:http://localhost:1420/0b0e7a8e-0000-4000-8000-000000000000",
    ]);
    let mut admitted = internal.to_vec();
    admitted.extend([
        "tauri://localhost",
        "tauri://localhost/",
        "tauri://localhost/index.html?view=agents#top",
        "blob:tauri://localhost/0b0e7a8e-0000-4000-8000-000000000000",
    ]);
    check("tauri://localhost", &admitted, &refused);

    // Windows production origin (http).
    let mut refused = always_refused.to_vec();
    refused.extend([
        "http://tauri.localhost:8080/",
        "http://tauri.localhost:1420/",
        "https://tauri.localhost/",
        "http://tauri.localhost./",
        "http://user@tauri.localhost/",
        "http://user:pw@tauri.localhost/",
        "tauri://localhost/",
        "http://localhost:1420/",
        "http://localhost/",
    ]);
    let mut admitted = internal.to_vec();
    admitted.extend([
        "http://tauri.localhost",
        "http://tauri.localhost/index.html",
        // The default port and upper-case host are the same origin once the
        // URL is normalised (special scheme).
        "http://tauri.localhost:80/",
        "http://TAURI.LOCALHOST/",
        "blob:http://tauri.localhost/0b0e7a8e-0000-4000-8000-000000000000",
    ]);
    check("http://tauri.localhost", &admitted, &refused);

    // Windows production origin with useHttpsScheme.
    let mut refused = always_refused.to_vec();
    refused.extend(["http://tauri.localhost/", "https://tauri.localhost:8443/"]);
    let mut admitted = internal.to_vec();
    admitted.extend(["https://tauri.localhost/", "https://tauri.localhost:443/"]);
    check("https://tauri.localhost", &admitted, &refused);

    // Dev mode: the devUrl origin only.
    let mut refused = always_refused.to_vec();
    refused.extend([
        "http://localhost:1421/",
        "http://localhost/",
        "https://localhost:1420/",
        "http://localhost.:1420/",
        "http://user@localhost:1420/",
        "http://user:pw@localhost:1420/",
        "tauri://localhost/",
        "http://tauri.localhost/",
        "blob:tauri://localhost/0b0e7a8e-0000-4000-8000-000000000000",
    ]);
    let mut admitted = internal.to_vec();
    admitted.extend([
        "http://localhost:1420",
        "http://localhost:1420/src/main.tsx?t=1#x",
        "http://LOCALHOST:1420/",
        "blob:http://localhost:1420/0b0e7a8e-0000-4000-8000-000000000000",
    ]);
    check("http://localhost:1420", &admitted, &refused);
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
        BOUNDARY_RS.contains(".on_navigation(move |url| navigation_allowed(url, &app_origin))"),
        "the main window must install the navigation guard"
    );
    assert!(
        BOUNDARY_RS.contains(".on_new_window(|_url, _features| NewWindowResponse::Deny)"),
        "the main window must deny every new window on every platform"
    );
    assert!(
        BOUNDARY_RS.contains(
            "AppOrigin::resolve(app.config(), &config, tauri::is_dev(), cfg!(windows))"
        ),
        "the guard's app origin must be resolved with tauri::is_dev(), the condition that selects devUrl"
    );
    assert!(
        !BOUNDARY_RS.contains("debug_assertions"),
        "the dev origin must not be keyed on the build profile"
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
