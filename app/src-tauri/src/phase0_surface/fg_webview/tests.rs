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

/// `src` with its Rust comments removed — line, block (nested) and doc
/// comments — and string, raw-string and char literals kept verbatim, so a
/// text guard cannot be satisfied by commented-out code.
fn strip_rust_comments(src: &str) -> String {
    fn ident(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == b'/' && next == Some(b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && next == Some(b'*') {
            let mut depth = 0usize;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(b' ');
        } else if c == b'r'
            && matches!(next, Some(b'"') | Some(b'#'))
            && (i == 0 || !ident(b[i - 1]) || (b[i - 1] == b'b' && (i < 2 || !ident(b[i - 2]))))
        {
            // A raw string: r"..", r#".."#, br"..".
            let mut j = i + 1;
            while b.get(j) == Some(&b'#') {
                j += 1;
            }
            let hashes = j - i - 1;
            if b.get(j) != Some(&b'"') {
                out.push(c);
                i += 1;
                continue;
            }
            let mut k = j + 1;
            while k < b.len() {
                if b[k] == b'"' && b[k + 1..].iter().take_while(|&&h| h == b'#').count() >= hashes {
                    k += 1 + hashes;
                    break;
                }
                k += 1;
            }
            out.extend_from_slice(&b[i..k.min(b.len())]);
            i = k;
        } else if c == b'"' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(b.len());
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if c == b'\'' && next == Some(b'\\') {
            // An escaped char literal: '\n', '\'', '\u{..}'.
            let mut j = i + 3;
            while j < b.len() && b[j] != b'\'' {
                j += 1;
            }
            let end = (j + 1).min(b.len());
            out.extend_from_slice(&b[i..end]);
            i = end;
        } else if c == b'\'' && b.get(i + 2) == Some(&b'\'') {
            // A one-byte char literal such as '"' (a lifetime is left alone).
            out.extend_from_slice(&b[i..i + 3]);
            i += 3;
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).expect("comment stripping keeps UTF-8 boundaries")
}

/// The comment stripper keeps code and literals and drops comments.
#[test]
fn p0_fg_webview_comment_stripper_drops_only_comments() {
    let src = concat!(
        "let a = \"tauri://localhost\"; // .on_navigation(x)\n",
        "/* outer /* .on_new_window(y) */ still comment */ let b = 1;\n",
        "/// doc .setup(z)\n",
        "let r = r#\"raw // not a comment\"#; let q = '\"'; let e = '\\''; fn f<'a>(_: &'a str) {}\n",
        "//! inner doc build_main_window(app)?;\n",
        "call(); // trailing",
    );
    let out = strip_rust_comments(src);
    for kept in [
        "let a = \"tauri://localhost\";",
        "let b = 1;",
        "r#\"raw // not a comment\"#",
        "let q = '\"';",
        "let e = '\\'';",
        "fn f<'a>(_: &'a str) {}",
        "call();",
    ] {
        assert!(out.contains(kept), "stripper must keep `{kept}`: {out}");
    }
    for dropped in [
        ".on_navigation(",
        ".on_new_window(",
        ".setup(",
        "build_main_window",
        "trailing",
    ] {
        assert!(
            !out.contains(dropped),
            "stripper must drop `{dropped}`: {out}"
        );
    }
}

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
/// is never emitted and app commands stay ungoverned. Checked on the build
/// script with its comments removed, so a commented-out call does not pass.
#[test]
fn p0_fg_webview_build_script_emits_the_app_manifest() {
    let build = strip_rust_comments(BUILD_RS);
    assert!(
        build.contains("mod app_commands;"),
        "build.rs must include the APP_COMMANDS list"
    );
    assert!(
        build.contains("AppManifest::new().commands(app_commands::APP_COMMANDS)"),
        "build.rs must pass APP_COMMANDS to tauri_build::AppManifest::commands"
    );
    assert!(
        build.contains("tauri_build::try_build("),
        "build.rs must call tauri_build::try_build with the app manifest"
    );
    assert!(
        !build.contains("tauri_build::build()"),
        "build.rs must not also run the manifest-less tauri_build::build()"
    );
}

/// The app-command capability must grant every command — and only those — to
/// window `main` at the local origin, with no remote URL and no platform
/// narrowing. tauri-build compiles every file in `capabilities/` (and any
/// inline capability in `tauri.conf.json`), so the exact file set is pinned and
/// the only other capability, `default.json`, is pinned to its core
/// permissions for window `main` at the local origin.
#[test]
fn p0_fg_webview_capability_is_local_main_only() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .expect("capabilities directory")
        .map(|e| {
            e.expect("capabilities entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["app-commands.json", "default.json"],
        "capabilities/ must hold exactly the two reviewed capability files"
    );
    let default: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("default.json")).expect("default.json"),
    )
    .expect("default.json parses");
    let mut keys: Vec<&str> = default
        .as_object()
        .expect("default.json object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "$schema",
            "description",
            "identifier",
            "permissions",
            "windows"
        ],
        "default.json: no remote, local, webviews or platforms override"
    );
    assert_eq!(default["identifier"], "default");
    assert_eq!(default["windows"], serde_json::json!(["main"]));
    assert_eq!(
        default["permissions"],
        serde_json::json!([
            "core:default",
            "core:event:default",
            "core:event:allow-listen",
            "core:event:allow-unlisten",
            "core:event:allow-emit",
            "core:event:allow-emit-to",
            "core:window:default",
            "core:webview:default",
            "core:app:default"
        ]),
        "default.json grants only the reviewed core permissions"
    );

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

/// The CSP is a real, restrictive policy (defence in depth), compared
/// directive by directive with the exact source sets, so an added source,
/// directive or duplicate fails; the security block carries nothing else (no
/// devCsp, no inline capability, no asset protocol); the privileged window
/// loads the app URL and is not auto-created without its guards; the app URLs
/// are the reviewed ones; and the bundle-time config merge touches only the
/// bundle.
#[test]
fn p0_fg_webview_conf_has_restrictive_csp_and_guarded_window() {
    use std::collections::{BTreeMap, BTreeSet};

    let conf: serde_json::Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json parses");
    let security = conf["app"]["security"]
        .as_object()
        .expect("security object");
    assert_eq!(
        security.keys().collect::<Vec<_>>(),
        ["csp"],
        "app.security must hold only the CSP"
    );
    let csp = security["csp"]
        .as_str()
        .expect("csp must be a string, not null");

    let mut directives: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for directive in csp.split(';').map(str::trim).filter(|d| !d.is_empty()) {
        let mut parts = directive.split_whitespace();
        let name = parts.next().expect("directive name").to_ascii_lowercase();
        let sources: BTreeSet<String> = parts.map(str::to_owned).collect();
        assert!(
            directives.insert(name.clone(), sources).is_none(),
            "duplicate CSP directive `{name}`"
        );
    }
    let expected: BTreeMap<String, BTreeSet<String>> = [
        ("default-src", "'self'"),
        ("script-src", "'self'"),
        ("style-src", "'self' 'unsafe-inline'"),
        ("font-src", "'self'"),
        ("img-src", "'self' data:"),
        ("media-src", "'self'"),
        ("connect-src", "'self' ipc: http://ipc.localhost"),
        ("frame-src", "'self'"),
        ("child-src", "'self'"),
        ("worker-src", "'self'"),
        ("object-src", "'none'"),
        ("base-uri", "'self'"),
        ("form-action", "'none'"),
        ("frame-ancestors", "'none'"),
    ]
    .into_iter()
    .map(|(name, sources)| {
        (
            name.to_owned(),
            sources.split_whitespace().map(str::to_owned).collect(),
        )
    })
    .collect();
    assert_eq!(
        directives, expected,
        "the CSP must be exactly the reviewed policy"
    );

    let windows = conf["app"]["windows"].as_array().expect("windows array");
    assert_eq!(windows.len(), 1, "exactly one configured window");
    let main = &windows[0];
    assert_eq!(main["label"], "main");
    assert_eq!(
        main["create"],
        serde_json::Value::Bool(false),
        "the main window must not be auto-created; it is built with its boundary in setup()"
    );
    for key in [
        "url",
        "useHttpsScheme",
        "dataDirectory",
        "additionalBrowserArgs",
        "proxyUrl",
    ] {
        assert!(
            main.get(key).is_none(),
            "the main window keeps the app URL and default engine settings (`{key}`)"
        );
    }
    assert_eq!(conf["build"]["devUrl"], "http://localhost:1420");
    assert_eq!(
        conf["build"]["frontendDist"], "../dist",
        "the production app document is the embedded dist, never a URL"
    );

    // Every other config file tauri can merge over this one — the
    // platform-specific `tauri.<platform>.conf.json` files it merges
    // automatically, and the `--config` files the release workflow passes —
    // may only touch the bundle. (Found by scanning, so a new merge file is
    // checked too.)
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut merges = 0;
    for entry in std::fs::read_dir(dir).expect("app/src-tauri") {
        let name = entry
            .expect("app/src-tauri entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        let lower = name.to_ascii_lowercase();
        if lower == "tauri.conf.json" || !lower.starts_with("tauri") {
            continue;
        }
        assert!(
            lower.starts_with("tauri.") && lower.ends_with(".conf.json"),
            "{name}: only JSON tauri.<name>.conf.json config files are reviewed"
        );
        let merge: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(&name)).expect("read merge config"),
        )
        .unwrap_or_else(|e| panic!("{name} parses: {e}"));
        assert_eq!(
            merge
                .as_object()
                .expect("merge object")
                .keys()
                .collect::<Vec<_>>(),
            ["bundle"],
            "{name}: a merged config must not change the app, window or security"
        );
        merges += 1;
    }
    assert_eq!(merges, 1, "the one reviewed release-time config merge");
}

/// The privileged document loads nothing from a third party: `index.html`
/// names no remote URL (it used to pull a Google Fonts stylesheet, i.e. remote
/// CSS applied to the approval screens, on every launch), and the CSP admits
/// no remote font or style origin and no `asset:` source (the asset protocol
/// is not compiled in: tauri's `protocol-asset` feature is off).
#[test]
fn p0_fg_webview_privileged_document_loads_no_third_party_resources() {
    let index_html = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../index.html"));
    for remote in ["http://", "https://", "//fonts.", "@import"] {
        assert!(
            !index_html.contains(remote),
            "app/index.html must not reference a remote resource ({remote})"
        );
    }
    let conf: serde_json::Value = serde_json::from_str(TAURI_CONF).expect("tauri.conf.json parses");
    let csp = conf["app"]["security"]["csp"].as_str().expect("csp string");
    for gone in ["googleapis", "gstatic", "asset:", "asset.localhost"] {
        assert!(!csp.contains(gone), "CSP must not admit {gone}");
    }
    let cargo_toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
    assert!(
        !cargo_toml.contains("protocol-asset"),
        "the asset protocol is not part of the app"
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
/// navigation guard with the app origin resolved by `tauri::is_dev()`, and an
/// unconditional new-window denial; `setup()` must build it; and nothing else
/// in the desktop sources may build a window or webview. Checked on the
/// sources with comments removed, so commented-out wiring does not pass. The
/// predicate itself is covered above and exercised live by
/// `tests/webview_boundary_live.rs`.
#[test]
fn p0_fg_webview_main_window_wires_navigation_and_newwindow_guards() {
    let boundary = strip_rust_comments(BOUNDARY_RS);
    for (needle, what) in [
        (
            "WebviewWindowBuilder::from_config(app.handle(), &config)?",
            "the main window must be built from its config",
        ),
        (
            ".on_navigation(move |url| navigation_allowed(url, &app_origin))",
            "the main window must install the navigation guard",
        ),
        (
            ".on_new_window(|_url, _features| NewWindowResponse::Deny)",
            "the main window must deny every new window on every platform",
        ),
        (
            "AppOrigin::resolve(app.config(), &config, tauri::is_dev(), cfg!(windows))",
            "the guard's app origin must be resolved with tauri::is_dev(), the condition that selects devUrl",
        ),
    ] {
        assert_eq!(boundary.matches(needle).count(), 1, "{what}");
    }
    for once in [
        ".on_navigation(",
        ".on_new_window(",
        "WebviewWindowBuilder::",
    ] {
        assert_eq!(
            boundary.matches(once).count(),
            1,
            "`{once}` must appear exactly once in the boundary module"
        );
    }
    assert!(
        !boundary.contains("debug_assertions"),
        "the dev origin must not be keyed on the build profile"
    );

    let lib = strip_rust_comments(LIB_RS);
    assert_eq!(
        lib.matches("crate::webview_boundary::build_main_window(app)?;")
            .count(),
        1,
        "setup() must build the privileged window through the boundary"
    );
    let setup = lib.find(".setup(|app| {").expect("setup closure");
    let call = lib
        .find("crate::webview_boundary::build_main_window(app)?;")
        .expect("boundary call");
    assert!(call > setup, "the boundary call must be inside setup()");

    // No other window or webview construction in the desktop sources.
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("source dir") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "tests") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with("tests.rs"))
            {
                out.push(path);
            }
        }
    }
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    assert!(files.len() > 20, "desktop sources not found");
    for file in files {
        if file.ends_with("webview_boundary.rs") {
            continue;
        }
        let text = strip_rust_comments(&std::fs::read_to_string(&file).expect("read source"));
        for builder in [
            "WebviewWindowBuilder",
            "WebviewBuilder",
            "WindowBuilder",
            "add_child(",
            ".on_navigation(",
            ".on_new_window(",
        ] {
            assert!(
                !text.contains(builder),
                "{}: only webview_boundary.rs may build a window or webview ({builder})",
                file.display()
            );
        }
    }
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

    // Every registered command (the list the fg_webview registry guard pins
    // to the live generate_handler! registration): the ACL grants each one
    // at the local origin on the main window, and the handler/closure decides
    // the rest. A command missing from the manifest would be refused here
    // even at the local origin.
    assert_eq!(APP_COMMANDS.len(), 804);
    for &cmd in APP_COMMANDS {
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
