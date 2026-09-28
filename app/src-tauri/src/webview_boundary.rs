//! P0 item D — the privileged webview boundary.
//!
//! Two layers close the "any script in the app origin can call every IPC
//! command" gap that the `null` CSP left open:
//!
//! * **Origin ACL (native).** `app_commands::APP_COMMANDS` is the exact set of
//!   application (non-plugin) commands registered by `generate_handler!`.
//!   `build.rs` passes it to `tauri_build::AppManifest::commands`, which turns
//!   the runtime into "an app command is refused unless a capability allows
//!   it". `capabilities/app-commands.json` allows them for window `main` at the
//!   **local** app origin only. Tauri derives `Origin::Local` vs
//!   `Origin::Remote` from trustworthy native context (the request's `Origin`
//!   header on the custom-protocol IPC path, or the frame URL on the
//!   `postMessage` path — never a frontend-supplied field), so a subframe, a
//!   `srcdoc`/`about:blank`/`data:` document or a navigated remote page is
//!   `Origin::Remote`, matches no capability, and is refused even if it holds
//!   the injected invoke key.
//! * **Navigation / new-window (native).** [`build_main_window`] builds the one
//!   privileged window with an [`navigation_allowed`] guard that refuses to let
//!   the privileged document be replaced by anything but the app's own origin,
//!   and denies every new window request on every platform.
//!
//! Neither layer trusts the CSP to prove the boundary; the CSP is a separate
//! defence-in-depth control in `tauri.conf.json`.

// The running binary does not read the list: `build.rs` includes this file
// directly (`#[path]`) and tauri-build embeds the resolved ACL into the binary.
// The crate compiles it only for the fg_webview guard, which checks it against
// the live `generate_handler!` registry.
#[cfg(test)]
pub(crate) mod app_commands;
#[cfg(test)]
pub(crate) use app_commands::APP_COMMANDS;

/// Whether the privileged main webview may navigate to a document with this
/// `scheme` and `host`. This is the pure core of the `on_navigation` guard,
/// kept dependency-free so it is always compiled and directly unit-tested.
///
/// Allowed:
/// * `tauri:` — the production app document (`tauri://localhost`).
/// * `http`/`https` to host `tauri.localhost` — the app document on Windows and
///   Android, where the custom protocol is tunnelled over http(s).
/// * `http`/`https` to `localhost`/`127.0.0.1` — **only** when `dev` is true
///   (the `tauri dev` server, `http://localhost:1420`). Refused in release, so
///   a release build can never be navigated to a loopback service.
/// * `about:`/`data:`/`blob:` — internal blank documents and the sandboxed,
///   script-free in-app previews. These never reach an application command
///   (opaque or `Origin::Remote`), so allowing them keeps previews working
///   without widening authority.
///
/// Everything else — notably `http`/`https` to any other host, and `file:` —
/// is refused: untrusted content can never replace the privileged document.
pub(crate) fn navigation_allowed_parts(scheme: &str, host: Option<&str>, dev: bool) -> bool {
    match scheme {
        "tauri" => true,
        "http" | "https" => match host {
            Some("tauri.localhost") => true,
            Some("localhost") | Some("127.0.0.1") => dev,
            _ => false,
        },
        "about" | "data" | "blob" => true,
        _ => false,
    }
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod live {
    use super::navigation_allowed_parts;
    use tauri::webview::NewWindowResponse;
    use tauri::{Manager, Url, WebviewWindowBuilder};

    /// The `on_navigation` guard for the privileged window. `dev` follows the
    /// build profile: the dev server origin is allowed only in debug builds.
    pub(crate) fn navigation_allowed(url: &Url) -> bool {
        navigation_allowed_parts(url.scheme(), url.host_str(), cfg!(debug_assertions))
    }

    /// Build the single privileged window from its `tauri.conf.json` config
    /// (which sets `"create": false` so Tauri does not auto-create it), adding
    /// the navigation guard and an unconditional new-window denial.
    ///
    /// New windows are denied on every platform (on Linux this also suppresses
    /// WebKitGTK's default "create" behaviour, which otherwise opens an
    /// un-governed webview); the app never opens a second window, and it must
    /// not open URLs in the OS browser (that would be a PATH-helper launch,
    /// item I).
    pub(crate) fn build_main_window(app: &tauri::App) -> tauri::Result<()> {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == "main")
            .cloned()
            .expect("tauri.conf.json must define the 'main' window");
        WebviewWindowBuilder::from_config(app.handle(), &config)?
            .on_navigation(navigation_allowed)
            .on_new_window(|_url, _features| NewWindowResponse::Deny)
            .build()?;
        Ok(())
    }
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub(crate) use live::build_main_window;
