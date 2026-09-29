//! P0 item D — LIVE native boundary harness.
//!
//! Runs the REAL Wry runtime on the process main thread with the PRODUCTION
//! context (`generate_context!`: the embedded app ACL, the capabilities, the
//! window config and the CSP) and the PRODUCTION
//! `webview_boundary::build_main_window` (navigation guard + new-window
//! denial), drives the privileged document with `webview.eval()`, and observes
//! real effects: request logs of loopback servers the harness runs, marker
//! files written by a benign handler bound to the real command name
//! `list_agents`, page-load events, the window list and, on Linux, the
//! engine's `create` signal. No signal is elapsed time.
//!
//! # Origin modes and build profiles, one process per mode
//!
//! A plain `cargo test` builds tauri without `custom-protocol`, i.e. tauri's
//! dev profile (`tauri::is_dev()`), where the app window loads `devUrl`; there
//! the harness runs its checks twice, each time in a child process of its own
//! (a native event loop can be created only once per process). Built with
//! `--features tauri/custom-protocol` (the release profile; `app/dist` must
//! exist, since it is embedded) tauri ignores `devUrl`, so only the
//! app-origin mode runs — on tauri's release code path and the guard's
//! `tauri::is_dev() == false` branch, exactly as in the shipped app.
//!
//! * **dev-origin** — the context as generated: the privileged document is
//!   `devUrl`, `http://localhost:1420`, served by the harness itself (no dev
//!   server exists in CI). No CSP applies (tauri adds it only to documents it
//!   serves), so frames and loopback fetches are observable.
//! * **app-origin** — the same context with `devUrl` removed and the harness's
//!   asset provider in place of the embedded `dist`: tauri then loads its
//!   PRODUCTION origin — `tauri://localhost` on Linux/macOS,
//!   `http://tauri.localhost` on Windows — through its own protocol handler,
//!   with the production CSP header. In the dev profile the guard reaches that
//!   origin through its `devUrl`-absent fallback; in the release profile
//!   through the release branch. (The harness's document replaces the embedded
//!   `dist` in both, so the checks drive a document they control.) The
//!   production origin of every platform is also unit-tested
//!   (`p0_fg_webview_app_origin_is_resolved_like_tauri_resolves_the_app_url`).
//!
//! Loopback servers (no DNS, no off-host traffic even if a claim were false):
//! `[::1]:1420` + `127.0.0.1:1420` ("app log": the app origin in dev-origin
//! mode, the refused dev-server origin in app-origin mode) and `[::1]:1421` +
//! `127.0.0.1:1421` ("remote log": always a non-app origin). A non-app
//! document reports, to its own server, whether it holds the injected bridge
//! and the outcome of a governed invoke, including the rejection text.
//!
//! # Checks, and what each one proves
//!
//! "Discriminating" checks FAIL with their protection removed (negative-control
//! runs are recorded in the item-D report); "regression" checks pass with or
//! without the protection on the pinned stack (tauri 2.10.3, wry 0.54.4) and
//! are kept to catch a future change, not as evidence of the fix.
//!
//! 1. Main-frame invoke (both modes): the app document invokes `list_agents`
//!    and the marker appears. Positive control: without it every refusal
//!    below would be vacuous. It fails if the capability, or the navigation
//!    guard, refuses the app origin (in app-origin mode: the production one).
//! 2. Sandboxed scripted `srcdoc` subframe (dev-origin): no governed command.
//!    REGRESSION: on Linux/macOS the frame never gets the key (tauri's init
//!    scripts are main-frame-only there); on Windows its request carries
//!    `Origin: null`, which tauri rejects before the ACL.
//! 3. Cross-origin, NON-sandboxed loopback subframe (dev-origin). Windows:
//!    WebView2 does not consult the guard for subframes and injects the key
//!    into every frame, so the frame must report "bridge present" and its
//!    invoke must be rejected with the ACL's text — DISCRIMINATING for the
//!    ACL. Linux/macOS: REGRESSION — the guard is consulted for subframe
//!    navigations and cancels it (observed on Linux), and without the guard
//!    it loads without a key (main-frame-only injection; observed on Linux).
//! 4. New windows, `window.open` and `<a target=_blank>` (both modes): no
//!    window appears and the target is never fetched. On Linux the harness
//!    lets script popups reach the engine's `create` signal and observes it.
//!    REGRESSION: the pinned wry 0.54.4 source refuses new windows when no
//!    handler is set, on all three platforms, so the production `Deny`
//!    handler is a safeguard. Observed only on Linux (this check passes with
//!    the handler removed); for Windows and macOS that is source reading,
//!    not observation.
//! 5. Script navigations of the privileged document (both modes) are
//!    cancelled before any request or page load: to the non-app origin
//!    `http://[::1]:1421` (DISCRIMINATING for `on_navigation`), and to
//!    look-alikes of the app origin — another host, another port, the dev
//!    server in app-origin mode, other `tauri://` hosts/ports (DISCRIMINATING
//!    for the exact-origin rule; the pre-fix predicate admitted them). If one
//!    is not cancelled, the check also reports whether the ACL refused the
//!    resulting non-app document, and restores the app document.
//! 6. A `302` from the app origin to the non-app origin (dev-origin): the
//!    guard must cancel the redirect before the target is requested — on Linux
//!    (observed: WebKitGTK consults the guard for redirects) and Windows
//!    (WebView2 raises `NavigationStarting` for redirects) —
//!    DISCRIMINATING for `on_navigation`. macOS was not observed: there a
//!    followed redirect passes only if the resulting non-app document held
//!    the bridge and the ACL rejected its invoke. No governed command may run
//!    in any case.
//!
//! Where the guard fails to cancel a navigation to the non-app origin (check
//! 5) or a redirect (check 6), the failure also reports what the ACL did with
//! the non-app document's invoke. With the production guard in place no
//! non-app document reaches IPC on Linux, so the ACL is exercised live there
//! only as a backstop, in the negative-control runs (guard removed: the ACL
//! rejects; guard and ACL removed: the invoke is accepted).
//!
//! An informational observation (dev-origin) documents the remaining
//! limitation: a same-origin, NON-sandboxed `srcdoc` frame counts as the local
//! app origin; this is why every in-app iframe must carry `sandbox=""`
//! (enforced by the frontend guard). It is reported, not asserted.
//!
//! # Hygiene and determinism
//!
//! Everything the harness writes goes to one per-run directory under the
//! system temp dir (never `HOME`), removed at the end: the markers, and on
//! Linux the webview engine's data, cache and config (the children get
//! `XDG_*_HOME` inside it). On Windows and macOS the engine keeps its data in
//! the platform's per-user location (WebView2 under the app identifier's
//! local-data folder; WKWebView's default store), so run the harness on CI
//! runners or a throw-away account there. Readiness is event-driven with
//! bounded deadlines; a refusal has no positive signal, so each refusal is
//! observed over a bounded window, after a reachability control (dev-origin)
//! showed that the target answers this webview. Every check runs and every
//! failure is reported; a failed precondition, or a missed deadline (each
//! child has a watchdog; the parent bounds each child too), is a FAILURE.
//!
//! `harness = false` (own `fn main`, main-thread event loop — required on
//! macOS); gated behind `--ignored`: `cargo test --test webview_boundary_live
//! -- --ignored` (under `xvfb-run -a` on Linux).

use std::borrow::Cow;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{Manager, Runtime, WebviewWindow};

/// Per-child scratch directory (inside the parent's per-run temp dir).
const DIR_ENV: &str = "NEXUS_WEBVIEW_LIVE_DIR";
const CHILD_FLAG: &str = "--nexus-webview-live-child";
/// A child's own deadline; the parent's bound is larger.
const CHILD_DEADLINE: Duration = Duration::from_secs(150);
const PARENT_DEADLINE: Duration = Duration::from_secs(210);
/// Observation window for a refusal (no positive signal exists for it).
const REFUSAL_WINDOW: Duration = Duration::from_secs(2);

static MARKER_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The non-app origin (IPv6 loopback literal), refused in every mode.
const REMOTE_ORIGIN: &str = "http://[::1]:1421";

const APP_DOC: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>nexus-live-app</title></head><body>nexus live boundary harness</body></html>";

// Served from the NON-app origin. It reports whether it holds the injected
// bridge, attempts a governed invoke, and reports the outcome — on rejection
// with the rejection text — to its own server. `role` names the check.
const REMOTE_DOC: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>nexus-live-remote</title></head><body>remote origin<script>\
var R=(new URLSearchParams(location.search).get('role')||'none').replace(/[^a-z]/g,'');\
var I=window.__TAURI_INTERNALS__;var b=(I&&I.invoke)?1:0;\
fetch('/probe?role='+R+'&bridge='+b);\
if(b){I.invoke('list_agents',{tag:R.toUpperCase()}).then(\
function(){fetch('/probe?role='+R+'&accepted=1')},\
function(e){fetch('/probe?role='+R+'&rejected='+encodeURIComponent(String(e)).slice(0,1800))});}\
</script></body></html>";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    DevOrigin,
    AppOrigin,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::DevOrigin => "dev-origin",
            Mode::AppOrigin => "app-origin",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [Mode::DevOrigin, Mode::AppOrigin]
            .into_iter()
            .find(|m| m.name() == s)
    }
}

fn marker_path(tag: &str) -> PathBuf {
    MARKER_DIR
        .get()
        .expect("marker dir set")
        .join(format!("marker_{tag}"))
}

/// A benign handler bound to the real ACL command name `list_agents`. It
/// writes a marker named by the caller's `tag`. Reaching it proves the request
/// passed the origin ACL and was dispatched; absence proves the opposite.
#[tauri::command]
fn list_agents(tag: String) -> Result<(), String> {
    if tag.is_empty()
        || tag.len() > 32
        || !tag
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("bad tag".into());
    }
    std::fs::write(marker_path(&tag), b"1").map_err(|e| e.to_string())
}

/// A failed precondition: the run cannot continue.
fn fail(msg: &str) -> ! {
    eprintln!("FAIL webview_boundary_live: {msg}");
    std::process::exit(101);
}

// ---------------------------------------------------------------- parent ---

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == CHILD_FLAG) {
        let mode = args
            .get(i + 1)
            .and_then(|m| Mode::parse(m))
            .unwrap_or_else(|| fail("child: unknown mode"));
        child_main(mode);
    }
    if !args.iter().any(|a| a == "--ignored") {
        println!("test webview_boundary_live ... ignored (pass `-- --ignored` to run)");
        return;
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let run_dir =
        std::env::temp_dir().join(format!("nexus-webview-live-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&run_dir).expect("create the per-run temp dir");

    // A release (custom-protocol) build ignores devUrl, so only the
    // app-origin mode exists there.
    let modes: &[Mode] = if tauri::is_dev() {
        &[Mode::DevOrigin, Mode::AppOrigin]
    } else {
        &[Mode::AppOrigin]
    };
    println!(
        "[live] tauri profile: {}",
        if tauri::is_dev() {
            "dev (tauri without custom-protocol)"
        } else {
            "release (tauri custom-protocol, embedded dist)"
        }
    );
    let mut failures = Vec::new();
    for &mode in modes {
        match run_child(mode, &run_dir) {
            Ok(()) => println!("[live] {}: all checks passed", mode.name()),
            Err(e) => {
                eprintln!("[live] {}: FAILED: {e}", mode.name());
                failures.push(mode.name());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&run_dir);
    if run_dir.exists() {
        failures.push("cleanup of the per-run temp dir");
    }
    if failures.is_empty() {
        println!("test webview_boundary_live ... ok");
    } else {
        println!(
            "test webview_boundary_live ... FAILED ({})",
            failures.join(", ")
        );
        std::process::exit(101);
    }
}

/// Run one origin mode in a child process with its own scratch directory, and
/// wait for it with a bounded deadline.
fn run_child(mode: Mode, run_dir: &std::path::Path) -> Result<(), String> {
    let dir = run_dir.join(mode.name());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(CHILD_FLAG).arg(mode.name()).env(DIR_ENV, &dir);
    if cfg!(target_os = "linux") {
        // The engine's per-user data (WebKitGTK website data and caches, and
        // tauri's app-local-data webview directory) goes to the per-run dir.
        for (var, sub) in [
            ("XDG_DATA_HOME", "xdg-data"),
            ("XDG_CACHE_HOME", "xdg-cache"),
            ("XDG_CONFIG_HOME", "xdg-config"),
            ("XDG_STATE_HOME", "xdg-state"),
        ] {
            let path = dir.join(sub);
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            cmd.env(var, path);
        }
        // Mesa's shader cache is flushed by the engine's GPU/web process after
        // the child exits, which would re-create files in the per-run dir
        // after the parent removed it; the harness needs no shader cache.
        cmd.env("MESA_SHADER_CACHE_DISABLE", "true");
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait());
    });
    match rx.recv_timeout(PARENT_DEADLINE) {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(format!("child exited with {status}")),
        Ok(Err(e)) => Err(format!("wait: {e}")),
        Err(_) => Err(format!(
            "child did not finish within {PARENT_DEADLINE:?} (its own watchdog should have \
             ended it first)"
        )),
    }
}

// ----------------------------------------------------------------- child ---

type Log = Arc<Mutex<Vec<String>>>;
type PageLoads = Arc<Mutex<Vec<(bool, String)>>>;

fn child_main(mode: Mode) -> ! {
    let dir = PathBuf::from(std::env::var_os(DIR_ENV).unwrap_or_else(|| fail("child: no dir")));
    if !dir.starts_with(std::env::temp_dir()) || !dir.is_dir() {
        fail("child: the scratch dir must be an existing dir under the system temp dir");
    }
    MARKER_DIR.set(dir).expect("set marker dir");

    let phase = Arc::new(Mutex::new(String::from("startup")));
    {
        let phase = phase.clone();
        std::thread::spawn(move || {
            std::thread::sleep(CHILD_DEADLINE);
            let p = phase.lock().map(|g| g.clone()).unwrap_or_default();
            fail(&format!("deadline missed while in phase `{p}`"));
        });
    }

    let servers = start_servers();
    let failures = run_live(mode, &phase, &servers);
    if failures.is_empty() {
        std::process::exit(0);
    }
    for f in &failures {
        eprintln!("FAIL webview_boundary_live [{}]: {f}", mode.name());
    }
    std::process::exit(101);
}

/// A loopback origin the harness serves: the 1420 origin or the non-app one.
#[derive(Clone, Copy)]
enum Role {
    App,
    Remote,
}

struct Servers {
    app_log: Log,
    remote_log: Log,
}

fn handle_conn(mut s: TcpStream, role: Role, log: &Mutex<Vec<String>>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 16 * 1024 {
                    break;
                }
            }
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let path = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("")
        .to_string();
    if let Ok(mut l) = log.lock() {
        l.push(path.clone());
    }
    let ok = |doc: &str| {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            doc.len(),
            doc
        )
    };
    let empty = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\n\
                 Connection: close\r\n\r\n"
        .to_string();
    let resp = match role {
        _ if path.starts_with("/probe") => empty,
        Role::App if path == "/redirect" => format!(
            "HTTP/1.1 302 Found\r\nLocation: {REMOTE_ORIGIN}/?role=redirect\r\n\
             Content-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        Role::App => ok(APP_DOC),
        Role::Remote => ok(REMOTE_DOC),
    };
    let _ = s.write_all(resp.as_bytes());
    let _ = s.flush();
}

fn serve(addr: &str, role: Role, log: Log) {
    let listener =
        TcpListener::bind(addr).unwrap_or_else(|e| fail(&format!("could not bind {addr}: {e}")));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let log = log.clone();
            std::thread::spawn(move || handle_conn(stream, role, &log));
        }
    });
}

fn start_servers() -> Servers {
    let app_log: Log = Arc::default();
    let remote_log: Log = Arc::default();
    for addr in ["[::1]:1420", "127.0.0.1:1420"] {
        serve(addr, Role::App, app_log.clone());
    }
    for addr in ["[::1]:1421", "127.0.0.1:1421"] {
        serve(addr, Role::Remote, remote_log.clone());
    }
    Servers {
        app_log,
        remote_log,
    }
}

/// The app-origin mode's asset provider: the app document for every path
/// (tauri falls back to `index.html`), with every requested key logged.
struct HarnessAssets {
    log: Log,
}

impl<R: Runtime> tauri::Assets<R> for HarnessAssets {
    fn get(&self, key: &tauri::utils::assets::AssetKey) -> Option<Cow<'_, [u8]>> {
        let key = key.as_ref().trim_start_matches('/').to_string();
        if let Ok(mut l) = self.log.lock() {
            l.push(key.clone());
        }
        if key.starts_with("alive-") {
            return Some(Cow::Borrowed(b""));
        }
        (key == "index.html").then(|| Cow::Borrowed(APP_DOC.as_bytes()))
    }

    fn iter(&self) -> Box<tauri::utils::assets::AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }

    fn csp_hashes(
        &self,
        _html_path: &tauri::utils::assets::AssetKey,
    ) -> Box<dyn Iterator<Item = tauri::utils::assets::CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

fn set_phase(phase: &Mutex<String>, s: &str) {
    if let Ok(mut g) = phase.lock() {
        *g = s.to_string();
    }
    eprintln!("[live] phase: {s}");
}

/// Process one batch of pending events. `run_iteration` is deprecated in favour
/// of `run_return`, but a bounded phased driver needs to regain control between
/// steps; a ticker (see `run_live`) wakes the loop at a fixed cadence so this
/// returns even when the webview is idle, which also bounds its CPU use.
#[allow(deprecated)]
fn pump_once<R: Runtime>(app: &mut tauri::App<R>) {
    app.run_iteration(|_, _| {});
}

/// Pump the event loop until `cond` holds or `budget` elapses; returns whether
/// `cond` held. The signal is always `cond`, never elapsed time.
fn pump_until<R: Runtime>(
    app: &mut tauri::App<R>,
    budget: Duration,
    mut cond: impl FnMut() -> bool,
) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        pump_once(app);
    }
}

/// `scheme://host[:port]` of a URL (explicit port only).
fn origin_of(u: &str) -> String {
    match tauri::Url::parse(u) {
        Ok(url) => format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            url.port().map(|p| format!(":{p}")).unwrap_or_default()
        ),
        Err(_) => u.to_string(),
    }
}

fn logged(log: &Mutex<Vec<String>>, path: &str) -> bool {
    log.lock()
        .map(|l| l.iter().any(|x| x == path))
        .unwrap_or(false)
}

/// The first logged path starting with `prefix`, if any.
fn logged_prefix(log: &Mutex<Vec<String>>, prefix: &str) -> Option<String> {
    log.lock()
        .ok()
        .and_then(|l| l.iter().find(|x| x.starts_with(prefix)).cloned())
}

fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `text` is tauri's ACL rejection for `list_agents` from a document
/// whose URL starts with `url_prefix` (tauri 2.10.3 `resolve_access_message`
/// in a debug build, or the release-build summary).
fn is_acl_rejection(text: &str, url_prefix: &str) -> bool {
    text.contains(&format!(
        "list_agents not allowed on window \"main\", webview \"main\", URL: {url_prefix}"
    )) || text.contains("Command list_agents not allowed by ACL")
}

/// What a non-app document with `role` reported.
struct RemoteReport {
    bridge: Option<bool>,
    accepted: bool,
    rejected: Option<String>,
}

impl RemoteReport {
    fn of(log: &Mutex<Vec<String>>, role: &str) -> Self {
        let bridge = if logged(log, &format!("/probe?role={role}&bridge=1")) {
            Some(true)
        } else if logged(log, &format!("/probe?role={role}&bridge=0")) {
            Some(false)
        } else {
            None
        };
        let prefix = format!("/probe?role={role}&rejected=");
        Self {
            bridge,
            accepted: logged(log, &format!("/probe?role={role}&accepted=1")),
            rejected: logged_prefix(log, &prefix).map(|p| percent_decode(&p[prefix.len()..])),
        }
    }

    fn settled(&self) -> bool {
        self.bridge == Some(false) || self.accepted || self.rejected.is_some()
    }

    /// A one-line account of what the ACL did with this document's invoke.
    fn acl_outcome(&self, url_prefix: &str) -> String {
        match (self.bridge, self.accepted, &self.rejected) {
            (_, true, _) => "its governed invoke was ACCEPTED".into(),
            (_, _, Some(t)) if is_acl_rejection(t, url_prefix) => {
                format!("its governed invoke was REJECTED by the ACL: {t:?}")
            }
            (_, _, Some(t)) => format!("its invoke was rejected, not by the ACL: {t:?}"),
            (Some(false), _, _) => "it had no bridge".into(),
            _ => "it reported nothing".into(),
        }
    }
}

fn page_loaded(loads: &Mutex<Vec<(bool, String)>>, pred: impl Fn(bool, &str) -> bool) -> bool {
    loads
        .lock()
        .map(|l| l.iter().any(|(finished, url)| pred(*finished, url)))
        .unwrap_or(false)
}

/// How many times tauri's protocol handler asked for the app document.
fn index_requests(assets: &Mutex<Vec<String>>) -> usize {
    assets
        .lock()
        .map(|l| l.iter().filter(|k| k.as_str() == "index.html").count())
        .unwrap_or(0)
}

/// A script-navigation target that must be refused, and the server log (if
/// any) on which a request for it would appear.
struct NavTarget {
    what: &'static str,
    url: String,
    log: Option<Log>,
}

/// Everything a check needs, for one child.
struct Live<'a, R: Runtime> {
    mode: Mode,
    app: tauri::App<R>,
    webview: WebviewWindow<R>,
    phase: &'a Mutex<String>,
    servers: &'a Servers,
    page_loads: PageLoads,
    asset_log: Log,
    app_url: tauri::Url,
    expected_origin: &'static str,
    failures: Vec<String>,
    probes: usize,
    #[cfg(target_os = "linux")]
    create_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl<R: Runtime> Live<'_, R> {
    fn m(&self) -> &'static str {
        self.mode.name()
    }

    fn check_failed(&mut self, msg: String) {
        eprintln!("[live] {} CHECK FAILED: {msg}", self.m());
        self.failures.push(msg);
    }

    fn pump_until(&mut self, budget: Duration, cond: impl FnMut() -> bool) -> bool {
        pump_until(&mut self.app, budget, cond)
    }

    fn eval(&self, js: impl Into<String>) {
        self.webview.eval(js).expect("eval into the app document");
    }

    fn document_origin(&self) -> String {
        origin_of(
            &self
                .webview
                .url()
                .map(|u| u.to_string())
                .unwrap_or_default(),
        )
    }

    /// Whether a script evaluated now runs in the app-origin document: it
    /// fetches `/alive-<n>` relative to its own document, which reaches the
    /// harness only from the app origin (the app server in dev-origin mode,
    /// tauri's protocol handler in app-origin mode).
    fn prove_live(&mut self) -> bool {
        self.probes += 1;
        let key = format!("alive-{}", self.probes);
        self.eval(format!("fetch('/{key}')"));
        let (app_log, assets) = (self.servers.app_log.clone(), self.asset_log.clone());
        let path = format!("/{key}");
        let mode = self.mode;
        let from_app_origin = move || match mode {
            Mode::DevOrigin => logged(&app_log, &path),
            Mode::AppOrigin => assets.lock().map(|l| l.contains(&key)).unwrap_or(false),
        };
        pump_until(&mut self.app, Duration::from_secs(10), from_app_origin)
    }

    /// Bring the app document back after a navigation that should have been
    /// refused was not, so the remaining checks still run at the app origin.
    fn restore_app_document(&mut self) {
        let before = self.page_loads.lock().map(|l| l.len()).unwrap_or(0);
        let _ = self.webview.navigate(self.app_url.clone());
        let (loads, origin) = (self.page_loads.clone(), self.expected_origin);
        let loaded = self.pump_until(Duration::from_secs(20), || {
            loads
                .lock()
                .map(|l| {
                    l.iter()
                        .skip(before)
                        .any(|(finished, url)| *finished && origin_of(url) == origin)
                })
                .unwrap_or(false)
        });
        if !loaded || !self.prove_live() {
            fail(&format!(
                "{}: could not restore the app document after a failed check",
                self.m()
            ));
        }
    }
}

fn run_live(mode: Mode, phase: &Mutex<String>, servers: &Servers) -> Vec<String> {
    let m = mode.name();
    let asset_log: Log = Arc::default();
    let page_loads: PageLoads = Arc::default();

    set_phase(phase, "build");
    let mut ctx: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let expected_origin = match mode {
        Mode::DevOrigin => "http://localhost:1420",
        Mode::AppOrigin if cfg!(windows) => "http://tauri.localhost",
        Mode::AppOrigin => "tauri://localhost",
    };
    if mode == Mode::AppOrigin {
        // What a release build does with this config: no devUrl, and the app
        // document from tauri's own protocol handler (here, the harness's
        // asset provider in place of the embedded dist). A release build
        // ignores devUrl by itself, so there the config stays as generated.
        if tauri::is_dev() {
            ctx.config_mut().build.dev_url = None;
        }
        ctx.set_assets(Box::new(HarnessAssets {
            log: asset_log.clone(),
        }));
    }
    let loads = page_loads.clone();
    let mut app = tauri::Builder::<tauri::Wry>::default()
        .invoke_handler(tauri::generate_handler![list_agents])
        .on_page_load(move |_webview, payload| {
            let finished = matches!(payload.event(), tauri::webview::PageLoadEvent::Finished);
            if let Ok(mut l) = loads.lock() {
                l.push((finished, payload.url().to_string()));
            }
        })
        .setup(|app| {
            // The PRODUCTION privileged window with its navigation/new-window
            // boundary, built from the real config.
            nexus_desktop_backend::webview_boundary::build_main_window(app)?;
            Ok(())
        })
        .build(ctx)
        .expect("build app");

    // Ticker: a wake cadence for the event loop (readiness is always an event
    // or an observed effect — never this interval).
    {
        let handle = app.handle().clone();
        std::thread::spawn(move || {
            while handle.run_on_main_thread(|| {}).is_ok() {
                std::thread::sleep(Duration::from_millis(25));
            }
        });
    }

    // Tauri 2 runs setup — and so the production build_main_window — on the
    // first loop iteration, not in `build()`.
    set_phase(phase, "setup: build_main_window");
    let window_deadline = Instant::now() + Duration::from_secs(30);
    let webview = loop {
        pump_once(&mut app);
        if let Some(w) = app.get_webview_window("main") {
            break w;
        }
        if Instant::now() >= window_deadline {
            fail("build_main_window did not create the `main` window");
        }
    };

    set_phase(phase, "app document load");
    if !pump_until(&mut app, Duration::from_secs(60), || {
        page_loaded(&page_loads, |finished, url| {
            finished && origin_of(url) == expected_origin
        })
    }) {
        fail(&format!(
            "{m}: the app document never finished loading at {expected_origin} (page loads: {:?})",
            page_loads.lock().map(|l| l.clone()).unwrap_or_default()
        ));
    }
    let app_url = page_loads
        .lock()
        .ok()
        .and_then(|l| {
            l.iter()
                .find(|(finished, url)| *finished && origin_of(url) == expected_origin)
                .and_then(|(_, url)| tauri::Url::parse(url).ok())
        })
        .expect("app document URL");

    // Linux: let script-initiated popups reach the engine's `create` signal
    // (and so wry's new-window handler, i.e. the production on_new_window), and
    // observe that signal. wry connected its handler when the webview was built;
    // on Deny it returns no widget, so the emission continues to this observer,
    // which counts it and also returns no widget.
    #[cfg(target_os = "linux")]
    let create_count = {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use webkit2gtk::{SettingsExt, WebViewExt};
        let count = Arc::new(AtomicUsize::new(0));
        let ready = Arc::new(AtomicBool::new(false));
        let (c, r) = (count.clone(), ready.clone());
        webview
            .with_webview(move |pw| {
                let wv = pw.inner();
                if let Some(settings) = WebViewExt::settings(&wv) {
                    settings.set_javascript_can_open_windows_automatically(true);
                }
                wv.connect_create(move |_, _| {
                    c.fetch_add(1, Ordering::SeqCst);
                    None
                });
                r.store(true, Ordering::SeqCst);
            })
            .expect("with_webview");
        if !pump_until(&mut app, Duration::from_secs(10), || {
            ready.load(Ordering::SeqCst)
        }) {
            fail("could not configure the test webview");
        }
        count
    };

    let mut live = Live {
        mode,
        app,
        webview,
        phase,
        servers,
        page_loads,
        asset_log,
        app_url,
        expected_origin,
        failures: Vec::new(),
        probes: 0,
        #[cfg(target_os = "linux")]
        create_count,
    };

    check_main_frame_invoke(&mut live);
    if mode == Mode::DevOrigin {
        check_subframes(&mut live);
    }
    check_new_windows(&mut live);
    check_script_navigations(&mut live);
    if mode == Mode::DevOrigin {
        check_redirect(&mut live);
    }

    set_phase(phase, "done");
    if live.failures.is_empty() {
        eprintln!("[live] {m}: all boundary checks passed");
    }
    live.failures
}

/// Check 1 (positive control; a failure is fatal).
fn check_main_frame_invoke<R: Runtime>(live: &mut Live<'_, R>) {
    set_phase(live.phase, "1 main-frame invoke (positive control)");
    live.eval("window.__TAURI_INTERNALS__.invoke('list_agents',{tag:'MAIN'})");
    if !live.pump_until(Duration::from_secs(30), || marker_path("MAIN").exists()) {
        fail(&format!(
            "{}: the app document at {} could not invoke a governed command",
            live.m(),
            live.expected_origin
        ));
    }
    eprintln!(
        "[live] {} 1: the app document at {} invoked list_agents (Origin::Local) [positive control]",
        live.m(),
        live.expected_origin
    );
}

/// Checks 2 and 3 and the same-origin observation (dev-origin only: the app
/// document carries no CSP there, so frames and their reports can load).
fn check_subframes<R: Runtime>(live: &mut Live<'_, R>) {
    let m = live.m();
    let app_log = live.servers.app_log.clone();
    let remote_log = live.servers.remote_log.clone();
    set_phase(live.phase, "2/3 subframes");
    // 2. Sandboxed scripted srcdoc frame: its own bridge (if any) and the
    //    parent's bridge.
    live.eval(
        r#"var f=document.createElement('iframe');
        f.setAttribute('sandbox','allow-scripts');
        f.srcdoc="<script>"+
          "var I=window.__TAURI_INTERNALS__;var b=(I&&I.invoke)?1:0;"+
          "fetch('http://127.0.0.1:1420/probe?sub_bridge='+b,{mode:'no-cors'});"+
          "if(b){I.invoke('list_agents',{tag:'SUB'}).then(function(){},function(e){"+
          "fetch('http://127.0.0.1:1420/probe?sub_rejected='+encodeURIComponent(String(e)).slice(0,600),{mode:'no-cors'})})}"+
          "try{parent.__TAURI_INTERNALS__.invoke('list_agents',{tag:'SUBPARENT'})}catch(e){}"+
          "<\/script>";
        document.body.appendChild(f);"#,
    );
    // 3. Cross-origin, non-sandboxed loopback frame (REMOTE_DOC, role=frame).
    live.eval(format!(
        "var g=document.createElement('iframe');g.src='{REMOTE_ORIGIN}/?role=frame';\
         document.body.appendChild(g);"
    ));
    // Observation: a same-origin, non-sandboxed srcdoc frame (the limitation).
    live.eval(
        r#"var h=document.createElement('iframe');
        h.srcdoc="<script>try{parent.__TAURI_INTERNALS__.invoke('list_agents',{tag:'SAMEORIGIN'})}catch(e){}<\/script>";
        document.body.appendChild(h);"#,
    );
    // Bounded: where the guard cancels the cross-origin frame it never
    // reports, and the window bounds that absence.
    live.pump_until(Duration::from_secs(4), || {
        RemoteReport::of(&remote_log, "frame").settled()
            && (logged(&app_log, "/probe?sub_bridge=0") || logged(&app_log, "/probe?sub_bridge=1"))
            && marker_path("SAMEORIGIN").exists()
    });

    // 2.
    if marker_path("SUB").exists() || marker_path("SUBPARENT").exists() {
        live.check_failed("2: a sandboxed subframe reached a governed command".into());
    } else {
        let sub_bridge = if logged(&app_log, "/probe?sub_bridge=1") {
            "present"
        } else if logged(&app_log, "/probe?sub_bridge=0") {
            "absent"
        } else {
            "unreported"
        };
        let rejection = logged_prefix(&app_log, "/probe?sub_rejected=")
            .map(|p| percent_decode(&p["/probe?sub_rejected=".len()..]))
            .unwrap_or_else(|| "none reported".into());
        eprintln!(
            "[live] {m} 2: sandboxed srcdoc subframe: no governed command ran; its bridge was \
             {sub_bridge}; rejection: {rejection} [regression check]"
        );
    }

    // 3.
    let requested = logged(&remote_log, "/?role=frame");
    let report = RemoteReport::of(&remote_log, "frame");
    let url_prefix = format!("{REMOTE_ORIGIN}/");
    if marker_path("FRAME").exists() || report.accepted {
        live.check_failed(format!(
            "3: a cross-origin subframe invoked a governed command ({})",
            report.acl_outcome(&url_prefix)
        ));
    } else if !requested {
        if cfg!(windows) {
            live.check_failed(
                "3: the cross-origin subframe never loaded (WebView2 does not consult the guard \
                 for subframes; the check would be vacuous)"
                    .into(),
            );
        } else {
            eprintln!(
                "[live] {m} 3: cross-origin subframe: navigation cancelled by the guard (no \
                 request) [regression check on this platform]"
            );
        }
    } else if report.bridge == Some(true) {
        match &report.rejected {
            Some(t) if is_acl_rejection(t, &url_prefix) => eprintln!(
                "[live] {m} 3: cross-origin subframe: bridge PRESENT, {} [discriminating]",
                report.acl_outcome(&url_prefix)
            ),
            _ => live.check_failed(format!(
                "3: the cross-origin subframe held the bridge, but {}",
                report.acl_outcome(&url_prefix)
            )),
        }
    } else if cfg!(windows) {
        live.check_failed(format!(
            "3: the cross-origin subframe loaded without a bridge on Windows (expected WebView2 \
             to inject the key into every frame; the check would be vacuous): {}",
            report.acl_outcome(&url_prefix)
        ));
    } else {
        eprintln!(
            "[live] {m} 3: cross-origin subframe: loaded without the bridge (main-frame-only \
             injection) [regression check on this platform]"
        );
    }

    // Observation (not asserted): the documented limitation.
    eprintln!(
        "[live] {m} observation: a same-origin, non-sandboxed srcdoc frame {} a governed command \
         (documented limitation: the ACL is per origin and window, not per frame; every in-app \
         iframe carries sandbox=\"\")",
        if marker_path("SAMEORIGIN").exists() {
            "REACHED"
        } else {
            "did not reach"
        }
    );
}

/// Check 4.
fn check_new_windows<R: Runtime>(live: &mut Live<'_, R>) {
    let m = live.m();
    let remote_log = live.servers.remote_log.clone();
    set_phase(live.phase, "4 new-window attempts");
    let win_baseline = live.app.webview_windows().len();
    for (what, role) in [
        ("window.open", "winopen"),
        ("anchor target=_blank", "anchor"),
    ] {
        let target = format!("{REMOTE_ORIGIN}/?role={role}");
        live.eval(if role == "winopen" {
            format!("window.open('{target}','_blank')")
        } else {
            format!(
                "var a=document.createElement('a');a.href='{target}';a.target='_blank';\
                 document.body.appendChild(a);a.click()"
            )
        });
        // Linux: the engine's `create` signal is the decision point (wry's
        // handler, connected first, has already returned the production Deny
        // when this observer runs). Elsewhere there is no decision signal, so
        // the refusal is observed over a bounded window: a window that opened
        // would load its target (a page-load event and a server request).
        #[cfg(target_os = "linux")]
        {
            use std::sync::atomic::Ordering;
            let count = live.create_count.clone();
            let before = count.load(Ordering::SeqCst);
            if !live.pump_until(Duration::from_secs(10), || {
                count.load(Ordering::SeqCst) > before
            }) {
                live.check_failed(format!(
                    "4: {what} never reached the engine's create signal (check is vacuous)"
                ));
                continue;
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let loads = live.page_loads.clone();
            let log = remote_log.clone();
            live.pump_until(REFUSAL_WINDOW, || {
                logged(&log, &format!("/?role={role}"))
                    || page_loaded(&loads, |_, url| url.contains(&format!("role={role}")))
            });
        }
        let windows = live.app.webview_windows();
        if windows.len() != win_baseline {
            live.check_failed(format!("4: {what} created a new window"));
            for (label, w) in windows {
                if label != "main" {
                    let _ = w.close();
                }
            }
        } else if logged(&remote_log, &format!("/?role={role}")) {
            live.check_failed(format!("4: {what} fetched its target"));
        } else {
            eprintln!(
                "[live] {m} 4: {what}: no window, target never fetched{} [regression check]",
                if cfg!(target_os = "linux") {
                    "; it reached the engine create signal and no window was created"
                } else {
                    ""
                }
            );
        }
    }
}

/// Check 5.
fn check_script_navigations<R: Runtime>(live: &mut Live<'_, R>) {
    let m = live.m();
    let app_log = live.servers.app_log.clone();
    let remote_log = live.servers.remote_log.clone();
    set_phase(live.phase, "5 script navigations to non-app origins");
    let target = |what: &'static str, url: &str, log: Option<&Log>| NavTarget {
        what,
        url: url.to_string(),
        log: log.cloned(),
    };
    let mut targets = vec![target(
        "the non-app origin",
        &format!("{REMOTE_ORIGIN}/?role=navigated"),
        Some(&remote_log),
    )];
    match live.mode {
        Mode::DevOrigin => {
            targets.push(target(
                "another host, same port",
                "http://127.0.0.1:1420/?nav=host",
                Some(&app_log),
            ));
            targets.push(target(
                "another port, same host",
                "http://localhost:1421/?nav=port",
                Some(&remote_log),
            ));
        }
        Mode::AppOrigin => {
            targets.push(target(
                "the dev-server origin",
                "http://localhost:1420/?nav=devurl",
                Some(&app_log),
            ));
            targets.push(target(
                "a loopback host",
                "http://127.0.0.1:1420/?nav=host",
                Some(&app_log),
            ));
            if cfg!(windows) {
                targets.push(target(
                    "the app host on another port",
                    "http://tauri.localhost:1421/?nav=port",
                    Some(&remote_log),
                ));
            } else {
                targets.push(target(
                    "the app scheme on another port",
                    "tauri://localhost:1421/?nav=port",
                    None,
                ));
                targets.push(target(
                    "the app scheme on another host",
                    "tauri://nexus/?nav=host",
                    None,
                ));
            }
        }
    }
    if live.mode == Mode::DevOrigin {
        // Reachability control: each server-observed target answers this
        // webview, so a navigation that was not cancelled would be observed.
        for (i, t) in targets.iter().enumerate() {
            if t.log.is_some() {
                live.eval(format!(
                    "fetch('{}/probe?reach={i}',{{mode:'no-cors'}})",
                    origin_of(&t.url)
                ));
            }
        }
        let reached = live.pump_until(Duration::from_secs(10), || {
            targets.iter().enumerate().all(|(i, t)| {
                t.log
                    .as_ref()
                    .is_none_or(|l| logged(l, &format!("/probe?reach={i}")))
            })
        });
        if !reached {
            fail(&format!(
                "{m}: a navigation target is not reachable from the webview"
            ));
        }
    }
    for NavTarget { what, url, log } in &targets {
        let path = &url[url.find("/?").unwrap_or(0)..];
        let target_origin = origin_of(url);
        let index_before = index_requests(&live.asset_log);
        let loads_before = live.page_loads.lock().map(|l| l.len()).unwrap_or(0);
        live.eval(format!("window.location.href='{url}'"));
        let (loads, assets) = (live.page_loads.clone(), live.asset_log.clone());
        let proceeded = move || {
            loads
                .lock()
                .map(|l| {
                    l.iter()
                        .skip(loads_before)
                        .any(|(_, u)| origin_of(u) == target_origin)
                })
                .unwrap_or(false)
                || log.as_ref().is_some_and(|l| logged(l, path))
                || index_requests(&assets) != index_before
        };
        let left = live.pump_until(REFUSAL_WINDOW, proceeded);
        if !left && live.document_origin() == live.expected_origin {
            eprintln!(
                "[live] {m} 5: navigation to {what} ({url}) cancelled: no request, no page \
                 load [discriminating]"
            );
            continue;
        }
        // Not cancelled. Let that navigation finish (so it cannot land after
        // the restore), report what the ACL did with the non-app document's
        // invoke, then restore the app document.
        let (loads, target_origin) = (live.page_loads.clone(), origin_of(url));
        live.pump_until(Duration::from_secs(10), || {
            loads
                .lock()
                .map(|l| {
                    l.iter()
                        .skip(loads_before)
                        .any(|(finished, u)| *finished && origin_of(u) == target_origin)
                })
                .unwrap_or(false)
        });
        let mut detail = String::new();
        if url.contains("role=navigated") {
            let log = live.servers.remote_log.clone();
            live.pump_until(Duration::from_secs(5), || {
                RemoteReport::of(&log, "navigated").settled()
            });
            detail = format!(
                "; the non-app document {}",
                RemoteReport::of(&log, "navigated").acl_outcome(&format!("{REMOTE_ORIGIN}/"))
            );
        }
        live.check_failed(format!(
            "5: a script navigation to {what} ({url}) was not cancelled (document `{}`){detail}",
            live.webview
                .url()
                .map(|u| u.to_string())
                .unwrap_or_default()
        ));
        live.restore_app_document();
    }
}

/// Check 6 (dev-origin only: in app-origin mode the app document is served by
/// tauri's protocol handler, which never redirects).
fn check_redirect<R: Runtime>(live: &mut Live<'_, R>) {
    let m = live.m();
    let app_log = live.servers.app_log.clone();
    let remote_log = live.servers.remote_log.clone();
    set_phase(live.phase, "6 redirect to the non-app origin");
    if !live.prove_live() {
        fail(&format!(
            "{m}: the app document is not live before the redirect check"
        ));
    }
    live.eval("window.location.href='http://localhost:1420/redirect'");
    // Decisive outcome: the non-app document reports its invoke result, or the
    // bounded window passes with nothing loaded there (redirect cancelled).
    live.pump_until(Duration::from_secs(10), || {
        RemoteReport::of(&remote_log, "redirect").settled() || marker_path("REDIRECT").exists()
    });
    if !logged(&app_log, "/redirect") {
        live.check_failed("6: the /redirect navigation never reached the app-origin server".into());
        return;
    }
    let report = RemoteReport::of(&remote_log, "redirect");
    let url_prefix = format!("{REMOTE_ORIGIN}/");
    // The authoritative assertion, on every platform: no governed command ran
    // for a document at the non-app origin.
    if marker_path("REDIRECT").exists() || report.accepted {
        live.check_failed(format!(
            "6: a non-app document reached via a redirect invoked a governed command ({})",
            report.acl_outcome(&url_prefix)
        ));
        return;
    }
    if !logged(&remote_log, "/?role=redirect") {
        if live.document_origin() != live.expected_origin {
            live.check_failed(format!(
                "6: unexpected document after a cancelled redirect: `{}`",
                live.webview
                    .url()
                    .map(|u| u.to_string())
                    .unwrap_or_default()
            ));
        } else {
            eprintln!(
                "[live] {m} 6: redirect CANCELLED by the navigation guard before the target was \
                 requested [discriminating]"
            );
        }
        return;
    }
    // Followed. On Linux (observed) and Windows (WebView2 raises
    // `NavigationStarting` for redirects) the guard is consulted for
    // redirects, so following one is a failure of the guard; the non-app
    // document's ACL outcome is reported with it. macOS was not observed: there
    // a followed redirect passes only if the resulting document held the
    // bridge and the ACL rejected its invoke, naming its own (non-app) URL.
    let acl_refused = report.bridge == Some(true)
        && report
            .rejected
            .as_deref()
            .is_some_and(|t| is_acl_rejection(t, &url_prefix));
    if cfg!(any(target_os = "linux", windows)) {
        live.check_failed(format!(
            "6: a redirect to {REMOTE_ORIGIN} was followed without the navigation guard \
             cancelling it; the non-app document {}",
            report.acl_outcome(&url_prefix)
        ));
    } else if acl_refused {
        eprintln!(
            "[live] {m} 6: redirect FOLLOWED to {REMOTE_ORIGIN} without the guard cancelling it; \
             that document held the bridge and {} [regression check on this platform]",
            report.acl_outcome(&url_prefix)
        );
    } else {
        live.check_failed(format!(
            "6: the redirect was followed and the non-app document's outcome is not an ACL \
             refusal of a bridged invoke: {}",
            report.acl_outcome(&url_prefix)
        ));
    }
}
