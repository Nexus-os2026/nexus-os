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
//!   privileged window with a navigation guard that admits only the exact app
//!   origin (see `live::navigation_allowed`) and denies every new window
//!   request on every platform.
//!
//! Redirects (observed live by `tests/webview_boundary_live.rs`): on WebKitGTK
//! (Linux) a server `3xx` redirect is followed by the engine without the
//! navigation guard being consulted — wry 0.54.4 routes only the engine's
//! navigation-action policy decisions to the guard (`webkitgtk/mod.rs`
//! 547-575), and the redirected request reaches the target. On Windows,
//! WebView2 raises `NavigationStarting` for redirects too, and wry routes that
//! event to the guard (`webview2/mod.rs` 673-692). macOS was not observed in
//! this environment. With the exact-origin rule the only admitted document
//! that is fetched over HTTP at all is the app origin itself: in a release
//! build that is the `tauri` protocol, which tauri serves from the embedded
//! assets and never answers with a redirect (tauri 2.10.3 `protocol/tauri.rs`
//! 212-219); in a dev build it is the configured `devUrl` dev server, which
//! is trusted development tooling and could redirect. `about:blank`,
//! `about:srcdoc` and `blob:` documents make no HTTP request. A document that
//! nevertheless lands on a non-app origin is still refused every application
//! command by the origin ACL above: the ACL, not the navigation guard, is the
//! IPC boundary.
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
    pub(crate) fn build_main_window(app: &tauri::App) -> tauri::Result<()> {
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
pub(crate) use live::build_main_window;

#[cfg(all(
    test,
    feature = "tauri-runtime",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub(crate) use live::{navigation_allowed, AppOrigin};
