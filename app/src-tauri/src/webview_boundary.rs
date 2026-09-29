//! P0 item D — the privileged webview boundary.
//!
//! Two native layers close the "any script in the app origin can call every
//! IPC command" gap that the `null` CSP left open. Sources cited are the
//! pinned tauri 2.10.3 and wry 0.54.4.
//!
//! * **Origin ACL.** `app_commands::APP_COMMANDS` is the exact set of
//!   application (non-plugin) commands registered by `generate_handler!`.
//!   `build.rs` passes it to `tauri_build::AppManifest::commands`; with an app
//!   manifest present tauri checks every app command against the ACL
//!   (`webview/mod.rs` 1801-1830) instead of skipping it.
//!   `capabilities/app-commands.json` grants them to window `main` at the
//!   **local** origin only. Tauri classifies each request natively, never from
//!   a frontend-supplied field: the URL it checks is the request's `Origin`
//!   header on the custom-protocol IPC path (`ipc/protocol.rs` 490-498), or
//!   the URL wry reports on the `postMessage` fallback path (on Linux that is
//!   the main frame's URL, `webkitgtk/mod.rs` 640-650). The request is
//!   `Origin::Local` only if that URL is the app origin (`is_local_url`,
//!   `webview/mod.rs` 1680-1720; this app registers no custom URI scheme);
//!   anything else is `Origin::Remote`, matches no capability and is refused
//!   with the ACL's "not allowed" error.
//!
//!   The ACL distinguishes origins and windows, not frames. What keeps
//!   subframes and other documents from command authority, per platform:
//!   - **Linux and macOS:** tauri's init scripts, which carry the invoke key,
//!     are injected into the main frame only (`for_main_frame_only`,
//!     `manager/webview.rs` 156 and 542; WebKitGTK `TopFrame`,
//!     `webkitgtk/mod.rs` 720-729; WKWebView `forMainFrameOnly`,
//!     `wkwebview/mod.rs` 776-786). A subframe has no key of its own.
//!   - **Windows:** WebView2 adds the init scripts to every frame
//!     (`webview2/mod.rs` 493-494; wry `lib.rs` 1007), so every subframe holds
//!     the key. A cross-origin subframe's request carries its own `Origin` and
//!     is refused by the ACL; a sandboxed (opaque-origin) frame sends
//!     `Origin: null`, which tauri rejects before the ACL ("Origin header is
//!     not a valid URL").
//!   - **All platforms:** a main-frame document at a non-app origin (one the
//!     navigation guard below failed to cancel) holds the key and is refused
//!     by the ACL. A **same-origin, non-sandboxed** frame — an `about:blank` or
//!     `srcdoc` frame without `sandbox`, or an app-origin `src` — counts as
//!     `Origin::Local` on every platform: it can reach the parent's bridge
//!     (Linux/macOS) or holds the key itself (Windows). The frontend rule is
//!     therefore load-bearing: every in-app iframe renders inline `srcDoc`
//!     with `sandbox=""` (opaque origin, no script) and no iframe is created
//!     from script (guarded by
//!     `p0_002c5c_frontend_html_sinks_are_escaped_and_previews_sandboxed`).
//!     This is the remaining limitation of the native layer.
//!   - Tauri 2.10.3 exempts `plugin:__TAURI_CHANNEL__|fetch` from the ACL
//!     (`webview/mod.rs` 1803-1804); this app uses no IPC channels.
//! * **Navigation / new windows.** [`build_main_window`] builds the one
//!   privileged window with a navigation guard that admits only the exact app
//!   origin (see `live::navigation_allowed`) and a new-window handler that
//!   denies every request. The guard is consulted for main-frame navigations
//!   on every platform, and for subframe navigations on Linux (observed) and
//!   macOS (wry routes every navigation action to it there); on Windows it
//!   sees `NavigationStarting`, the main frame only. For new windows, the
//!   pinned wry 0.54.4 source refuses them when no handler is set
//!   (WebKitGTK's `create` signal is connected only with a handler,
//!   `webkitgtk/mod.rs` 486-489; WebView2 marks the request handled,
//!   `webview2/mod.rs` 784-786; the WKWebView UI delegate returns no
//!   webview), so the explicit `Deny` handler is a safeguard against a change
//!   of that default, not the closure itself. That default is observed live
//!   only on Linux (the harness passes with the handler removed); for Windows
//!   and macOS it rests on reading the source, not on observation.
//!
//! Redirects and subframes, as observed live on Linux by
//! `tests/webview_boundary_live.rs`: WebKitGTK consults the guard for server
//! redirects and for subframe navigations, so a `302` from the app origin to
//! a non-app origin is cancelled before the target is requested, and so is a
//! cross-origin subframe. On Windows the guard sees WebView2's
//! `NavigationStarting` (`webview2/mod.rs` 673-692), which is raised for
//! main-frame navigations including redirects, but not for subframes. macOS
//! was not observed in this environment (wry routes every WKWebView
//! navigation action to the guard, `wkwebview/navigation.rs` 50-83). With the
//! exact-origin rule, the only admitted document fetched over HTTP at all is
//! the app origin itself: in a release build the `tauri` protocol, which
//! tauri serves from the embedded assets and never answers with a redirect
//! (tauri 2.10.3 `protocol/tauri.rs` 212-219); in a dev build the configured
//! `devUrl` dev server. `about:blank`, `about:srcdoc` and `blob:` documents
//! make no HTTP request. A document that nevertheless lands on a non-app
//! origin is still refused every application command by the origin ACL
//! (observed live with the guard removed): the ACL, not the navigation guard,
//! is the IPC boundary.
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

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod live {
    use tauri::utils::config::{Config, FrontendDist, WindowConfig};
    use tauri::webview::NewWindowResponse;
    use tauri::{Manager, Url, WebviewWindowBuilder};

    /// The one origin the privileged document is served from: scheme, host and
    /// port (the explicit port, or the scheme's default port if it has one).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct AppOrigin {
        scheme: String,
        host: String,
        port: Option<u16>,
    }

    impl AppOrigin {
        /// The origin of the document tauri loads for the app window, resolved
        /// exactly as tauri 2.10.3 resolves `WebviewUrl::App`
        /// (`AppManager::get_app_url` and `tauri_protocol_url`,
        /// `manager/mod.rs` 331-360):
        ///
        /// * `dev` (tauri's `cfg(dev)`, i.e. tauri built without its
        ///   `custom-protocol` feature — the value of `tauri::is_dev()`): the
        ///   configured `devUrl`, if any;
        /// * otherwise: `frontendDist` if it is a URL (this app's is a local
        ///   directory, pinned by the fg_webview guard);
        /// * else the `tauri` protocol URL: `tauri://localhost`, or on Windows
        ///   `http://tauri.localhost` (`https://` when the window sets
        ///   `useHttpsScheme`).
        pub(crate) fn resolve(
            config: &Config,
            window: &WindowConfig,
            dev: bool,
            windows_os: bool,
        ) -> Option<Self> {
            let configured = if dev {
                config.build.dev_url.clone()
            } else {
                match &config.build.frontend_dist {
                    Some(FrontendDist::Url(url)) => Some(url.clone()),
                    _ => None,
                }
            };
            let url = match configured {
                Some(url) => url,
                None => {
                    let protocol = match (windows_os, window.use_https_scheme) {
                        (true, true) => "https://tauri.localhost",
                        (true, false) => "http://tauri.localhost",
                        (false, _) => "tauri://localhost",
                    };
                    Url::parse(protocol).ok()?
                }
            };
            Self::of(&url)
        }

        /// The origin of `url`, if it has a host.
        pub(crate) fn of(url: &Url) -> Option<Self> {
            Some(Self {
                scheme: url.scheme().to_owned(),
                host: url.host_str()?.to_owned(),
                port: url.port_or_known_default(),
            })
        }

        /// Whether `url` is at exactly this origin and carries no userinfo.
        /// Hosts are compared as the `url` crate normalises them: lowercased
        /// for the special schemes (`http`, `https`), verbatim for the others
        /// (`tauri`); a trailing-dot host is a different host.
        fn matches(&self, url: &Url) -> bool {
            url.username().is_empty()
                && url.password().is_none()
                && url.scheme() == self.scheme
                && url.host_str() == Some(self.host.as_str())
                && url.port_or_known_default() == self.port
        }
    }

    /// The `on_navigation` guard of the privileged window: whether its main
    /// document (or, on engines that consult the guard for them, a subframe)
    /// may navigate to `url`.
    ///
    /// Admitted, and nothing else:
    /// * the exact app origin ([`AppOrigin::resolve`]) — scheme, host and port
    ///   all equal, no userinfo;
    /// * exactly `about:blank` and `about:srcdoc` (no query, no fragment) —
    ///   the empty document and the sandboxed in-app `srcDoc` previews;
    /// * `blob:` URLs whose creator origin is the app origin — the object URLs
    ///   the app document creates for downloads.
    ///
    /// Everything else is refused: other schemes (`data:`, `file:`,
    /// `javascript:`, `asset:`, `ipc:`, …), other hosts (including the other
    /// loopback spellings and `*.localhost` names), other ports, the dev server
    /// origin in a production build and the production origin in a dev build.
    pub(crate) fn navigation_allowed(url: &Url, app: &AppOrigin) -> bool {
        match url.scheme() {
            "about" => {
                url.query().is_none()
                    && url.fragment().is_none()
                    && matches!(url.path(), "blank" | "srcdoc")
            }
            "blob" => Url::parse(url.path()).is_ok_and(|creator| app.matches(&creator)),
            _ => app.matches(url),
        }
    }

    /// Build the single privileged window from its `tauri.conf.json` config
    /// (which sets `"create": false` so Tauri does not auto-create it), adding
    /// the navigation guard and an unconditional new-window denial.
    ///
    /// The guard's app origin is resolved with `tauri::is_dev()` — the same
    /// condition under which tauri loads `devUrl` — so the dev server origin is
    /// admitted only when it is the app origin.
    ///
    /// New windows are denied on every platform; the app never opens a second
    /// window, and it must not open URLs in the OS browser (that would be a
    /// PATH-helper launch, item I).
    ///
    /// `pub` (but `#[doc(hidden)]`) only so the native boundary harness
    /// (`tests/webview_boundary_live.rs`) can build the real privileged window
    /// with the production guards; it is not a stable public API.
    #[doc(hidden)]
    pub fn build_main_window(app: &tauri::App) -> tauri::Result<()> {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == "main")
            .cloned()
            .expect("tauri.conf.json must define the 'main' window");
        let app_origin = AppOrigin::resolve(app.config(), &config, tauri::is_dev(), cfg!(windows))
            .ok_or(tauri::Error::InvalidWebviewUrl(
                "the privileged window's app origin could not be resolved",
            ))?;
        WebviewWindowBuilder::from_config(app.handle(), &config)?
            .on_navigation(move |url| navigation_allowed(url, &app_origin))
            .on_new_window(|_url, _features| NewWindowResponse::Deny)
            .build()?;
        Ok(())
    }
}

#[cfg(all(
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[doc(hidden)]
pub use live::build_main_window;

#[cfg(all(
    test,
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub(crate) use live::{navigation_allowed, AppOrigin};
