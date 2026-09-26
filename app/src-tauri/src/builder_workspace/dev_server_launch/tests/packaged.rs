//! P0-002C4C3 packaged launch tests (`nexus_packaged_toolchain` builds; CI's
//! packaged step on Linux, Windows and macOS). Each uses the real assembled
//! toolchain verified against the embedded manifest, the sealed spawn of its
//! Node under the exact production permission arguments, the backend-owned
//! lifecycle and, unless a fixture script is named, the real Nexus entry and
//! loopback preview server. Governed storage paths contain spaces.
use super::super::super::tests::generated;
use super::super::super::trusted_toolchain::verify_assembled;
use super::super::super::{
    provision_planned_workspace, run_plan, Audit, BuilderWorkspaceAuthority, WriteResult,
};
use super::*;
use nexus_kernel::workspace_authority::WorkspaceAuthorityRegistry;
use std::collections::BTreeMap;

const PROMPT: &str = "a saas landing page with pricing";
const NAME: &str = "Nexus Fixture";

fn assembled_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("builder-toolchain")
        .canonicalize()
        .unwrap()
}

fn wait_until(deadline: Instant, mut done: impl FnMut() -> bool) -> bool {
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

// ── Governed project fixture ──────────────────────────────────────────────

/// A registered project provisioned through the production path from the
/// deterministic scaffold, under a storage root whose path contains spaces.
struct Governed {
    base: PathBuf,
    authority: Arc<BuilderWorkspaceAuthority>,
    id: String,
    root: PathBuf,
    events: Arc<Mutex<Vec<Value>>>,
}

impl Governed {
    fn new() -> Self {
        let base = temporary("packaged");
        let storage = base.join("Builder Storage");
        let authority = Arc::new(
            BuilderWorkspaceAuthority::provision(
                Arc::new(WorkspaceAuthorityRegistry::new()),
                &storage,
            )
            .unwrap(),
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let audit = recorder(&events);
        let plan = run_plan(&authority, Arc::clone(&audit), PROMPT, |_| Ok(generated())).unwrap();
        provision_planned_workspace(&authority, &plan.project_id, PROMPT, NAME, audit).unwrap();
        let root = PathBuf::from(&plan.project_dir);
        assert!(root.to_str().unwrap().contains(' '));
        Self {
            base,
            authority,
            id: plan.project_id,
            root,
            events,
        }
    }

    fn audit(&self) -> Audit {
        recorder(&self.events)
    }

    fn start(&self, settings: &LaunchSettings) -> WriteResult<String> {
        self.authority
            .dev_server_start_with(&self.id, self.audit(), settings)
    }

    fn stop(&self) {
        self.authority
            .dev_server_stop(&self.id, self.audit())
            .unwrap();
    }

    /// The backend-owned lifecycle state (`stopped` when nothing is owned).
    fn owned(&self) -> &'static str {
        self.authority
            .lifecycle
            .owned_status(Uuid::parse_str(&self.id).unwrap())
            .map_or("stopped", |status| status.label())
    }

    fn status(&self) -> String {
        self.authority
            .dev_server_status(&self.id, self.audit())
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn event(&self, operation: &str, outcome: &str, reason: &str) -> bool {
        self.event_since(0, operation, outcome, reason)
    }

    /// An audit event recorded at or after position `from`.
    fn event_since(&self, from: usize, operation: &str, outcome: &str, reason: &str) -> bool {
        self.events.lock().unwrap()[from..].iter().any(|event| {
            event["operation"] == operation
                && event["outcome"] == outcome
                && event["reason"] == reason
        })
    }

    fn never_running(&self) -> bool {
        !self.event(
            "builder.devserver.lifecycle.started",
            "succeeded",
            "running",
        )
    }

    /// A fixture script inside React (readable under the permission model).
    fn script(&self, name: &str, source: &str) -> PathBuf {
        let dir = self.root.join(REACT).join("c4c3-fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, source).unwrap();
        path.canonicalize().unwrap()
    }

    fn runtime(&self, child: &str) -> PathBuf {
        self.root.join(RUNTIME).join(child)
    }
}

impl Drop for Governed {
    fn drop(&mut self) {
        let quiet: Audit = Arc::new(|_| {});
        let _ = self
            .authority
            .shutdown_dev_servers(Instant::now() + Duration::from_secs(10), &quiet);
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn recorder(events: &Arc<Mutex<Vec<Value>>>) -> Audit {
    let events = Arc::clone(events);
    Arc::new(move |event| events.lock().unwrap().push(event))
}

/// Observes the launched root (test-only) and captures bounded stderr.
#[derive(Default)]
struct Observed {
    pids: Arc<Mutex<Vec<u32>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl Observed {
    fn settings(&self) -> LaunchSettings {
        self.settings_with(None)
    }

    fn settings_with(&self, script: Option<PathBuf>) -> LaunchSettings {
        let pids = Arc::clone(&self.pids);
        LaunchSettings {
            toolchain: ToolchainSource::Assembled(assembled_root()),
            readiness_wait: READINESS_WAIT,
            probe_wait: PROBE_WAIT,
            test: Hooks {
                script,
                spawned: Some(Arc::new(move |pid| pids.lock().unwrap().push(pid))),
                stderr: Some(Arc::clone(&self.stderr)),
            },
        }
    }

    fn spawned(&self) -> usize {
        self.pids.lock().unwrap().len()
    }

    fn pid(&self) -> u32 {
        let pids = self.pids.lock().unwrap();
        assert_eq!(pids.len(), 1, "{pids:?}");
        pids[0]
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned()
    }
}

// ── Loopback HTTP and process evidence (test-only) ────────────────────────

fn port_of(url: &str) -> u16 {
    url.strip_prefix("http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix('/'))
        .and_then(|port| port.parse().ok())
        .unwrap_or_else(|| panic!("not a loopback preview URL: {url}"))
}

/// One raw exchange, read until the server closes (bounded).
fn exchange(port: u16, request: &str) -> (String, String) {
    let mut stream = TcpStream::connect((LOOPBACK, port)).unwrap();
    stream.set_read_timeout(Some(WAIT)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response).into_owned();
    match text.split_once("\r\n\r\n") {
        Some((head, body)) => (head.to_owned(), body.to_owned()),
        None => (text, String::new()),
    }
}

fn get(port: u16, target: &str, headers: &str) -> (String, String) {
    exchange(
        port,
        &format!(
            "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{headers}Connection: close\r\n\r\n"
        ),
    )
}

fn code(head: &str) -> u16 {
    head.split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

fn closed(port: u16) -> bool {
    wait_until(Instant::now() + WAIT, || {
        TcpStream::connect((LOOPBACK, port)).is_err()
    })
}

/// Live processes as (pid, parent pid, process group).
#[cfg(target_os = "linux")]
fn processes() -> Vec<(u32, u32, u32)> {
    let mut all = Vec::new();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // After the parenthesised command: state, ppid, pgrp, ...
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if let (Some(ppid), Some(pgrp)) = (fields.get(1), fields.get(2)) {
            if let (Ok(ppid), Ok(pgrp)) = (ppid.parse(), pgrp.parse()) {
                all.push((pid, ppid, pgrp));
            }
        }
    }
    all
}

#[cfg(target_os = "macos")]
fn processes() -> Vec<(u32, u32, u32)> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-A", "-o", "pid=,ppid=,pgid="])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields: Vec<u32> = line
                .split_whitespace()
                .filter_map(|field| field.parse().ok())
                .collect();
            (fields.len() == 3).then(|| (fields[0], fields[1], fields[2]))
        })
        .collect()
}

#[cfg(windows)]
fn processes() -> Vec<(u32, u32, u32)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut all = Vec::new();
    // SAFETY: a process snapshot handle is closed once; the entry is a
    // zeroed PROCESSENTRY32W whose dwSize is set as the API requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        assert_ne!(snapshot, INVALID_HANDLE_VALUE);
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            all.push((entry.th32ProcessID, entry.th32ParentProcessID, 0));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    all
}

/// Every local address the operating system reports listening on TCP `port`
/// (IPv4 and IPv6): independent evidence of the preview's bind address.
#[cfg(target_os = "linux")]
fn listeners(port: u16) -> Vec<String> {
    let mut found = Vec::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = std::fs::read_to_string(table) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (Some(local), Some(state)) = (fields.get(1), fields.get(3)) else {
                continue;
            };
            if let Some((address, hex)) = local.split_once(':') {
                if *state == "0A" && u16::from_str_radix(hex, 16).ok() == Some(port) {
                    found.push(address.to_owned());
                }
            }
        }
    }
    found
}

#[cfg(target_os = "macos")]
fn listeners(port: u16) -> Vec<String> {
    let output = std::process::Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fn"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(str::to_owned)
        .collect()
}

#[cfg(windows)]
fn listeners(port: u16) -> Vec<String> {
    let mut found = Vec::new();
    for protocol in ["TCP", "TCPv6"] {
        let output = std::process::Command::new("netstat")
            .args(["-ano", "-p", protocol])
            .output()
            .unwrap();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 4
                && fields[3] == "LISTENING"
                && fields[1].ends_with(&format!(":{port}"))
            {
                found.push(fields[1].to_owned());
            }
        }
    }
    found
}

/// Listening only on 127.0.0.1 (never 0.0.0.0, `::` or another address).
fn assert_loopback_only(port: u16) {
    let expected = if cfg!(target_os = "linux") {
        "0100007F".to_owned()
    } else {
        format!("127.0.0.1:{port}")
    };
    assert_eq!(listeners(port), vec![expected]);
}

fn alive(pid: u32) -> bool {
    processes().iter().any(|process| process.0 == pid)
}

fn children(pid: u32) -> Vec<u32> {
    processes()
        .into_iter()
        .filter(|process| process.1 == pid && process.0 != pid)
        .map(|process| process.0)
        .collect()
}

/// The root was terminated and reaped, and nothing it could have created
/// survives: its process group (Unix) or child list (Windows) is empty.
fn assert_gone(pid: u32, context: &str) {
    assert!(
        wait_until(soon(), || !alive(pid)),
        "{context}: root still alive"
    );
    #[cfg(unix)]
    assert!(
        wait_until(soon(), || processes().iter().all(|p| p.2 != pid)),
        "{context}: process group not empty"
    );
    #[cfg(windows)]
    assert!(children(pid).is_empty(), "{context}: children survive");
}

// ── Filesystem evidence ───────────────────────────────────────────────────

/// Every entry below `root` except `skip` (relative path → kind and bytes),
/// never following links.
fn snapshot(root: &Path, skip: Option<&Path>) -> BTreeMap<PathBuf, (&'static str, Vec<u8>)> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if Some(path.as_path()) == skip {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if metadata.is_dir() {
                pending.push(path);
                entries.insert(relative, ("dir", Vec::new()));
            } else if metadata.is_file() {
                entries.insert(relative, ("file", std::fs::read(&path).unwrap()));
            } else {
                entries.insert(relative, ("other", Vec::new()));
            }
        }
    }
    entries
}

fn toolchain_addon(root: &Path) -> PathBuf {
    let mut found = Vec::new();
    let mut pending = vec![root.join("node_modules")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "node") {
                found.push(path);
            }
        }
    }
    found.sort();
    // Prefer the Rolldown binding (every target has one); never fsevents.
    found
        .iter()
        .find(|path| path.to_string_lossy().contains("rolldown-binding"))
        .or_else(|| found.iter().find(|path| !path.ends_with("fsevents.node")))
        .cloned()
        .expect("a toolchain native addon")
}

/// A module that proves execution by writing `marker` (JSON-quoted path).
fn hostile(marker: &Path) -> String {
    let marker = serde_json::to_string(marker.to_str().unwrap()).unwrap();
    format!("require('node:fs').writeFileSync({marker}, 'executed');\nmodule.exports = {{}};\n")
}

// ── Fixture scripts (used instead of the entry; same Node arguments) ──────

const SILENT: &str = "setInterval(() => {}, 1000);\n";

fn announce(line: &str) -> String {
    let line = serde_json::to_string(&format!("{line}\n")).unwrap();
    format!("process.stdout.write({line});\nsetInterval(() => {{}}, 1000);\n")
}

/// A loopback server answering `status`, with or without this launch's probe
/// token; it records that Node started (in its runtime temp directory), then
/// announces readiness after `delay_ms`, optionally exiting after the first
/// response.
fn server(status: u16, token: bool, delay_ms: u64, exit_after_first: bool) -> String {
    format!(
        r#"const http = require('node:http');
const {{ probeToken }} = JSON.parse(process.argv[2]);
const server = http.createServer((req, res) => {{
  const headers = {{ 'content-type': 'text/plain', connection: 'close' }};
  if ({token}) headers['x-nexus-preview-token'] = probeToken;
  if ({status} === 302) headers.location = 'http://127.0.0.1:9/';
  res.writeHead({status}, headers);
  res.end('fixture\n');
  if ({exit_after_first}) res.on('finish', () => process.exit(0));
}});
server.listen({{ port: 0, host: '127.0.0.1' }}, () => {{
  const fs = require('node:fs');
  fs.writeFileSync(require('node:path').join(require('node:os').tmpdir(), 'started'), 'x');
  const line = JSON.stringify({{ event: 'ready', host: '127.0.0.1', port: server.address().port }});
  setTimeout(() => process.stdout.write(line + '\n'), {delay_ms});
}});
"#
    )
}

// ── End to end ────────────────────────────────────────────────────────────

#[test]
fn p0_002c4c3_packaged_governed_launch_serves_the_project_end_to_end() {
    // 1. A governed, registered, provisioned project (path with spaces).
    let g = Governed::new();
    let react = g.root.join(REACT);
    let runtime = g.root.join(RUNTIME);
    // 2. The real toolchain verifies against the embedded manifest.
    verify_assembled(&assembled_root()).expect("assembled toolchain verifies");
    // Project configuration that would execute if any tool loaded it.
    let markers = g.runtime("tmp");
    for config in ["vite.config.js", "postcss.config.js", "tailwind.config.js"] {
        let source = hostile(&markers.join(config));
        g.authority
            .write_file(&g.id, config, source.as_bytes(), g.audit())
            .unwrap();
    }
    let react_before = snapshot(&react, None);
    let outside_runtime = snapshot(&g.base, Some(&runtime));
    // 3-5. START through the production seam: fresh verification, sealed
    // Node, readiness, probe; only the loopback URL is returned.
    let observed = Observed::default();
    let url = g
        .start(&observed.settings())
        .unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
    let port = port_of(&url);
    assert_eq!(url, format!("http://127.0.0.1:{port}/"));
    let pid = observed.pid();
    assert_eq!(g.status(), "running");
    assert_loopback_only(port);
    // 6-7. GET / serves this project's own HTML (every line of its
    // index.html; Vite only adds its client script).
    let (head, body) = get(port, "/", "Accept: text/html\r\n");
    assert_eq!(code(&head), 200, "{head}\n{}", observed.stderr());
    let html = std::fs::read_to_string(react.join("index.html")).unwrap();
    assert!(html.contains("<title>") && html.contains(r#"<div id="root"></div>"#));
    for line in html.lines().filter(|line| !line.trim().is_empty()) {
        assert!(body.contains(line), "{line:?} missing from:\n{body}");
    }
    // 8. A project JavaScript module, transformed by the trusted toolchain.
    let (head, body) = get(port, "/src/main.tsx", "");
    assert_eq!(code(&head), 200, "{head}\n{}", observed.stderr());
    assert!(
        head.to_ascii_lowercase()
            .contains("content-type: text/javascript"),
        "{head}"
    );
    assert!(body.contains("createRoot"), "{body:.600}");
    let (head, _) = get(port, "/src/App.tsx", "");
    assert_eq!(code(&head), 200, "{head}");
    // Project CSS through the Nexus-owned Tailwind/PostCSS (preflight ran).
    let (head, body) = get(port, "/src/index.css", "");
    assert_eq!(code(&head), 200, "{head}\n{}", observed.stderr());
    assert!(body.contains("box-sizing"), "{body:.600}");
    // 9. The project root is unmodified.
    assert_eq!(snapshot(&react, None), react_before);
    // 10. Writes happened only under runtime/ (the dependency cache).
    assert_eq!(snapshot(&g.base, Some(&runtime)), outside_runtime);
    assert!(g.runtime("vite-cache").read_dir().unwrap().next().is_some());
    for config in ["vite.config.js", "postcss.config.js", "tailwind.config.js"] {
        assert!(!markers.join(config).exists(), "{config} was executed");
    }
    assert!(children(pid).is_empty(), "the preview created a process");
    // 11. STOP.
    g.stop();
    // 12. The owned tree was finalized.
    assert_gone(pid, "stop");
    // 13. The port is closed.
    assert!(closed(port));
    // 14. Status is stopped.
    assert_eq!(g.status(), "stopped");
    // The toolchain was never modified; audit never carries the URL or paths.
    verify_assembled(&assembled_root()).expect("toolchain unchanged");
    let audit = serde_json::to_string(&*g.events.lock().unwrap()).unwrap();
    for private in [
        "127.0.0.1",
        "http://",
        g.base.to_str().unwrap(),
        assembled_root().to_str().unwrap(),
        "\"pid\"",
        "\"port\"",
        "\"url\"",
    ] {
        assert!(!audit.contains(private), "{private} in audit");
    }
    assert!(g.event("builder.devserver.start", "succeeded", "running"));
}

// ── Node permission model ─────────────────────────────────────────────────

const PERMISSION_PROBE: &str = r#"import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
const cases = __CASES__;
const require = createRequire(import.meta.url);
const results = {};
async function attempt(name, action) {
  try {
    await action();
    results[name] = 'ok';
  } catch (error) {
    results[name] = error?.code ?? String(error?.message ?? error);
  }
}
const request = JSON.parse(process.argv[2]);
results.request = Object.keys(request).sort().join(',');
await attempt('read_project', () => fs.readFileSync(path.join(cases.react, 'index.html')));
await attempt('write_project', () => fs.writeFileSync(path.join(cases.react, 'written.txt'), 'x'));
await attempt('mkdir_project', () => fs.mkdirSync(path.join(cases.react, 'made')));
await attempt('read_toolchain', () => fs.readFileSync(cases.entry));
await attempt('write_toolchain', () => fs.writeFileSync(path.join(cases.toolchain, 'written.txt'), 'x'));
await attempt('write_cache', () => fs.writeFileSync(path.join(cases.cache, 'probe.txt'), 'x'));
await attempt('read_cache', () => fs.readFileSync(path.join(cases.cache, 'probe.txt')));
await attempt('write_home', () => fs.writeFileSync(path.join(cases.home, 'probe.txt'), 'x'));
await attempt('write_tmp', () => fs.writeFileSync(path.join(cases.tmp, 'probe.txt'), 'x'));
await attempt('read_project_state', () => fs.readFileSync(path.join(cases.project, 'builder_state.json')));
await attempt('list_storage', () => fs.readdirSync(cases.storage));
await attempt('list_runtime', () => fs.readdirSync(cases.runtime));
await attempt('list_env', () => fs.readdirSync(cases.env));
await attempt('read_outside', () => fs.readFileSync(cases.outsideFile));
await attempt('list_user_home', () => fs.readdirSync(cases.userHome));
await attempt('write_outside', () => fs.writeFileSync(path.join(cases.outside, 'written.txt'), 'x'));
await attempt('write_project_dir', () => fs.writeFileSync(path.join(cases.project, 'written.txt'), 'x'));
await attempt('write_runtime', () => fs.writeFileSync(path.join(cases.runtime, 'written.txt'), 'x'));
await attempt('write_env', () => fs.writeFileSync(path.join(cases.env, 'written.txt'), 'x'));
const childProcess = require('node:child_process');
await attempt('spawn', () => childProcess.spawnSync(process.execPath, ['--version']));
await attempt('exec', () => new Promise((resolve, reject) => {
  childProcess.exec('net use', (error) => (error ? reject(error) : resolve()));
}));
await attempt('worker', () => new (require('node:worker_threads').Worker)('0', { eval: true }));
await attempt('inspector', () => require('node:inspector').open(0));
await attempt('wasi', () => new (require('node:wasi').WASI)({ version: 'preview1' }));
await attempt('toolchain_addon', () => require(cases.toolchainAddon));
const guard = await import(pathToFileURL(path.join(cases.toolchain, 'entry', 'module-guard.mjs')).href);
guard.installModuleGuard(cases.toolchain);
await attempt('guarded_toolchain_addon', () => require(cases.toolchainAddon));
await attempt('project_addon', () => require(path.join(cases.react, 'evil.node')));
await attempt('project_addon_import', () => import(pathToFileURL(path.join(cases.react, 'evil.node')).href));
await attempt('ancestor_addon', () => require(path.join(cases.project, 'evil.node')));
results.has = {
  react_write: process.permission.has('fs.write', cases.react),
  toolchain_write: process.permission.has('fs.write', cases.toolchain),
  child: process.permission.has('child'),
  worker: process.permission.has('worker'),
  wasi: process.permission.has('wasi'),
  inspector: process.permission.has('inspector'),
  addon: process.permission.has('addon'),
};
process.stdout.write(JSON.stringify(results));
"#;

#[test]
fn p0_002c4c3_packaged_node_permission_model_confines_the_runtime() {
    let g = Governed::new();
    let verified = verify_assembled(&assembled_root()).unwrap();
    let paths = LaunchPaths::new(&verified, &g.root).unwrap();
    let react = g.root.join(REACT);
    let outside = temporary("outside");
    std::fs::write(outside.join("secret"), b"outside").unwrap();
    // A genuine, loadable native addon planted as project content and in the
    // project directory (an ancestor of React). Located from the verified
    // canonical root (`paths` holds the spellings Node receives).
    let addon = toolchain_addon(verified.root());
    std::fs::copy(&addon, react.join("evil.node")).unwrap();
    std::fs::copy(&addon, g.root.join("evil.node")).unwrap();
    let user_home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .expect("a user home");
    let show = |path: &Path| path.to_str().unwrap().to_owned();
    let project = node_spelling(&g.root).unwrap();
    let cases = json!({
        "react": show(&paths.react),
        "entry": show(&paths.entry),
        "toolchain": show(&paths.toolchain),
        "cache": show(&paths.cache),
        "home": show(&paths.home),
        "tmp": show(&paths.tmp),
        "project": show(&project),
        "storage": show(project.parent().unwrap()),
        "runtime": show(&project.join(RUNTIME)),
        "env": show(&project.join(RUNTIME).join("env")),
        "outside": show(&node_spelling(&outside).unwrap()),
        "outsideFile": show(&node_spelling(&outside).unwrap().join("secret")),
        "userHome": show(&user_home),
        "toolchainAddon": show(&node_spelling(&addon).unwrap()),
    });
    let script = g.script(
        "permission-probe.mjs",
        &PERMISSION_PROBE.replace("__CASES__", &cases.to_string()),
    );
    // Exactly the production Node arguments, environment and working
    // directory; only the script differs from a real launch.
    let arguments = node_arguments(&paths, &node_spelling(&script).unwrap(), TOKEN).unwrap();
    let mut child = ResourceLimiter::default()
        .spawn_sealed(&sealed_spec(&paths, arguments).unwrap())
        .unwrap();
    let (mut stdout, stderr) = (child.take_stdout().unwrap(), child.take_stderr().unwrap());
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Some(Arc::clone(&captured));
    drain(stderr, move |bytes| capture(&sink, bytes)).unwrap();
    let (sender, received) = sync_channel(1);
    thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        let _ = sender.send(output);
    });
    let output = received.recv_timeout(Duration::from_secs(60));
    // The probe ends by itself; finalize once its exit is observed (bounded).
    // Darwin refuses SIGKILL for a group whose root is still exiting: the
    // lifecycle absorbs that by retrying finalization, this direct probe by
    // waiting for the observed exit.
    let exited = wait_until(soon(), || matches!(child.poll_exit(), Ok(Some(_))));
    child.terminate_and_reap(soon()).unwrap();
    let stderr = String::from_utf8_lossy(&captured.lock().unwrap()).into_owned();
    let output = output.unwrap_or_else(|_| panic!("probe timed out: {stderr}"));
    assert!(exited, "the permission probe did not exit: {stderr}");
    let results: Value =
        serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}\n{stderr}"));
    let denied = "ERR_ACCESS_DENIED";
    for (name, expected) in [
        ("request", "cacheDir,probeToken,projectRoot"),
        ("read_project", "ok"),
        ("write_project", denied),
        ("mkdir_project", denied),
        ("read_toolchain", "ok"),
        ("write_toolchain", denied),
        ("write_cache", "ok"),
        ("read_cache", "ok"),
        ("write_home", "ok"),
        ("write_tmp", "ok"),
        ("read_project_state", denied),
        ("list_storage", denied),
        ("list_runtime", denied),
        ("list_env", denied),
        ("read_outside", denied),
        ("list_user_home", denied),
        ("write_outside", denied),
        ("write_project_dir", denied),
        ("write_runtime", denied),
        ("write_env", denied),
        ("spawn", denied),
        ("exec", denied),
        ("worker", denied),
        ("inspector", denied),
        ("wasi", denied),
        ("toolchain_addon", "ok"),
        ("guarded_toolchain_addon", "ok"),
        ("project_addon", "ERR_NEXUS_MODULE_DENIED"),
        ("project_addon_import", "ERR_NEXUS_MODULE_DENIED"),
    ] {
        assert_eq!(results[name], expected, "{name}: {results}\n{stderr}");
    }
    // Outside every granted location: denied by the guard or the model.
    assert!(
        results["ancestor_addon"] == "ERR_NEXUS_MODULE_DENIED"
            || results["ancestor_addon"] == denied,
        "{results}"
    );
    assert_eq!(
        results["has"],
        json!({"react_write": false, "toolchain_write": false, "child": false,
            "worker": false, "wasi": false, "inspector": false, "addon": true})
    );
    // Nothing was written where writes were denied.
    for path in [
        react.join("written.txt"),
        react.join("made"),
        g.root.join("written.txt"),
        g.root.join(RUNTIME).join("written.txt"),
        g.runtime("env").join("written.txt"),
        outside.join("written.txt"),
        assembled_root().join("written.txt"),
    ] {
        assert!(!path.exists(), "{path:?}");
    }
    std::fs::remove_dir_all(&outside).unwrap();
}

// ── Nexus HTTP gate on the real preview ───────────────────────────────────

#[test]
fn p0_002c4c3_packaged_http_gate_denies_foreign_hosts_origins_and_editor_routes() {
    let g = Governed::new();
    let observed = Observed::default();
    let url = g
        .start(&observed.settings())
        .unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
    let port = port_of(&url);
    let pid = observed.pid();
    let host = format!("127.0.0.1:{port}");
    let other = if port == 65535 { 1 } else { port + 1 };
    let request = |target: &str, headers: &str| {
        code(
            &exchange(
                port,
                &format!("GET {target} HTTP/1.1\r\n{headers}Connection: close\r\n\r\n"),
            )
            .0,
        )
    };
    // Host must be exactly this loopback authority.
    for bad in [
        "evil.example".to_owned(),
        format!("localhost:{port}"),
        format!("127.0.0.1:{other}"),
        "127.0.0.1".to_owned(),
        format!("[::1]:{port}"),
        format!("0.0.0.0:{port}"),
        format!("127.0.0.1:{port}.evil.example"),
        format!("127.0.0.1:{port}@evil.example"),
    ] {
        assert_eq!(request("/", &format!("Host: {bad}\r\n")), 403, "{bad}");
    }
    assert_eq!(code(&exchange(port, "GET / HTTP/1.0\r\n\r\n").0), 403);
    let duplicate = request("/", &format!("Host: {host}\r\nHost: {host}\r\n"));
    assert!(matches!(duplicate, 400 | 403), "{duplicate}");
    // Origin, if present, must be this exact loopback origin.
    for bad in [
        "http://evil.example".to_owned(),
        "null".to_owned(),
        format!("http://localhost:{port}"),
        format!("https://127.0.0.1:{port}"),
        format!("http://127.0.0.1:{other}"),
        format!("http://{host}/"),
    ] {
        assert_eq!(
            request("/", &format!("Host: {host}\r\nOrigin: {bad}\r\n")),
            403,
            "{bad}"
        );
    }
    assert_eq!(
        request(
            "/",
            &format!("Host: {host}\r\nOrigin: http://{host}\r\nOrigin: http://{host}\r\n")
        ),
        403
    );
    let (head, _) = get(
        port,
        "/",
        &format!("Accept: text/html\r\nOrigin: http://{host}\r\n"),
    );
    assert_eq!(code(&head), 200, "{head}");
    // No permissive CORS anywhere.
    assert!(
        !head
            .to_ascii_lowercase()
            .contains("access-control-allow-origin"),
        "{head}"
    );
    let (head, _) = exchange(
        port,
        &format!(
            "OPTIONS / HTTP/1.1\r\nHost: {host}\r\nOrigin: http://evil.example\r\nAccess-Control-Request-Method: GET\r\nConnection: close\r\n\r\n"
        ),
    );
    assert!(matches!(code(&head), 403 | 405), "{head}");
    assert!(
        !head.to_ascii_lowercase().contains("access-control-allow"),
        "{head}"
    );
    // The editor-launch route in every spelling, and other request forms.
    let host_header = format!("Host: {host}\r\n");
    for target in [
        "/__open-in-editor",
        "/__open-in-editor?file=src/main.tsx",
        "/__OPEN-IN-EDITOR?file=src/main.tsx",
        "/__Open-In-Editor?file=src/main.tsx",
        "/%5f%5fopen-in-editor?file=src/main.tsx",
        "/%5F%5Fopen-in-editor?file=src/main.tsx",
        "/%255f%255fopen-in-editor?file=src/main.tsx",
        "/__open%2Din%2Deditor?file=src/main.tsx",
        "/__open-in-editor%3Ffile=src/main.tsx",
        "/src/../__open-in-editor?file=src/main.tsx",
        "/./__open-in-editor?file=src/main.tsx",
        "/src/%2e%2e/__open-in-editor?file=src/main.tsx",
        "/__open-in-editor/?file=src/main.tsx",
    ] {
        assert_eq!(request(target, &host_header), 403, "{target}");
    }
    assert_eq!(request("/%E0%A4%A", &host_header), 400);
    assert_eq!(request("http://evil.example/", &host_header), 400);
    assert_eq!(request("//evil.example/", &host_header), 400);
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let (head, _) = exchange(
            port,
            &format!("{method} / HTTP/1.1\r\n{host_header}Content-Length: 0\r\nConnection: close\r\n\r\n"),
        );
        assert_eq!(code(&head), 405, "{method}");
    }
    // No WebSocket (HMR is off and upgrades are refused).
    let (head, _) = exchange(
        port,
        &format!(
            "GET / HTTP/1.1\r\n{host_header}Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        ),
    );
    assert_ne!(code(&head), 101, "{head}");
    // The preview stays Running and serving, and never created a process.
    assert_eq!(g.status(), "running");
    assert_eq!(code(&get(port, "/", "Accept: text/html\r\n").0), 200);
    assert!(children(pid).is_empty(), "the preview created a process");
    assert!(alive(pid));
    g.stop();
    assert_gone(pid, "stop");
    assert!(closed(port));
}

// ── Trusted toolchain freshness ───────────────────────────────────────────

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn flip_last_byte(path: &Path) -> Vec<u8> {
    let original = std::fs::read(path).unwrap();
    let mut changed = original.clone();
    let last = changed.len() - 1;
    changed[last] ^= 0x01;
    std::fs::write(path, changed).unwrap();
    original
}

#[test]
fn p0_002c4c3_packaged_toolchain_tampering_is_denied_before_spawn() {
    let g = Governed::new();
    let holder = temporary("toolchain copy");
    copy_tree(&assembled_root(), &holder.join("toolchain"));
    let copy = holder.join("toolchain").canonicalize().unwrap();
    let launch = || {
        let observed = Observed::default();
        let mut settings = observed.settings();
        settings.toolchain = ToolchainSource::Assembled(copy.clone());
        (g.start(&settings), observed)
    };
    // A denied launch: no process, and its own audit says the toolchain was
    // rejected (not merely unavailable).
    let denied = |context: &str| {
        let from = g.events.lock().unwrap().len();
        let (started, observed) = launch();
        assert_eq!(started, Err("trusted toolchain unavailable"), "{context}");
        assert_eq!(
            observed.spawned(),
            0,
            "{context}: spawned despite tampering"
        );
        assert!(
            g.event_since(
                from,
                "builder.devserver.start",
                "failed",
                "toolchain_rejected"
            ),
            "{context}"
        );
    };
    // The intact copy verifies and launches (every launch verifies afresh).
    let (started, observed) = launch();
    started.unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
    let pid = observed.pid();
    g.stop();
    assert_gone(pid, "intact copy");
    let node = copy
        .join("node")
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    let entry = copy.join("entry").join("nexus-builder.mjs");
    let addon = toolchain_addon(&copy);
    for changed in [&node, &entry, &addon] {
        let original = flip_last_byte(changed);
        denied(&format!("{changed:?}"));
        std::fs::write(changed, original).unwrap();
    }
    let extra = copy.join("node_modules").join("unexpected.js");
    std::fs::write(&extra, b"module.exports = {};\n").unwrap();
    denied("extra file");
    std::fs::remove_file(&extra).unwrap();
    // Restored bytes verify and launch again: verification is never cached.
    let (started, observed) = launch();
    started.unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
    let pid = observed.pid();
    g.stop();
    assert_gone(pid, "restored copy");
    let _ = std::fs::remove_dir_all(&holder);
}

// ── Readiness failures ────────────────────────────────────────────────────

#[test]
fn p0_002c4c3_packaged_readiness_failures_never_reach_running() {
    let g = Governed::new();
    let short = Duration::from_secs(3);
    for (name, source, wait, reason) in [
        (
            "exit.cjs",
            "process.exit(3);\n".to_owned(),
            READINESS_WAIT,
            "exited_before_ready",
        ),
        (
            "malformed.cjs",
            announce(r#"{"event":"ready","host":"127.0.0.1","port":"x"}"#),
            READINESS_WAIT,
            "readiness_malformed",
        ),
        (
            "wrong-host.cjs",
            announce(r#"{"event":"ready","host":"0.0.0.0","port":4000}"#),
            READINESS_WAIT,
            "readiness_malformed",
        ),
        (
            "port-zero.cjs",
            announce(r#"{"event":"ready","host":"127.0.0.1","port":0}"#),
            READINESS_WAIT,
            "readiness_malformed",
        ),
        (
            "oversized.cjs",
            "process.stdout.write('x'.repeat(4096));\nsetInterval(() => {}, 1000);\n".to_owned(),
            READINESS_WAIT,
            "readiness_oversized",
        ),
        ("silent.cjs", SILENT.to_owned(), short, "readiness_timeout"),
        (
            "probe-500.cjs",
            server(500, true, 0, false),
            READINESS_WAIT,
            "probe_failed",
        ),
        (
            "probe-no-token.cjs",
            server(200, false, 0, false),
            READINESS_WAIT,
            "probe_failed",
        ),
        (
            "probe-redirect.cjs",
            server(302, true, 0, false),
            READINESS_WAIT,
            "probe_failed",
        ),
    ] {
        let observed = Observed::default();
        let mut settings = observed.settings_with(Some(g.script(name, &source)));
        settings.readiness_wait = wait;
        // This case's own audit events only.
        let from = g.events.lock().unwrap().len();
        let began = Instant::now();
        assert_eq!(
            g.start(&settings),
            Err("dev server not ready"),
            "{name}: {}",
            observed.stderr()
        );
        assert!(
            began.elapsed() < wait + PROBE_WAIT + Duration::from_secs(15),
            "{name}"
        );
        // Terminated and reaped by the starting thread; no orphan.
        assert_gone(observed.pid(), name);
        assert_eq!(g.status(), "stopped", "{name}");
        assert!(
            g.event_since(
                from,
                "builder.devserver.lifecycle.not_ready",
                "succeeded",
                reason
            ),
            "{name}"
        );
        assert!(
            g.event_since(from, "builder.devserver.start", "failed", reason),
            "{name}"
        );
        assert!(g.never_running(), "{name}");
    }
}

// ── Lifecycle races with the real Node process ────────────────────────────

#[test]
fn p0_002c4c3_packaged_stop_or_shutdown_during_starting_finalizes_the_tree() {
    for shutdown in [false, true] {
        let g = Arc::new(Governed::new());
        let observed = Observed::default();
        let settings = observed.settings_with(Some(g.script("silent.cjs", SILENT)));
        let starter = {
            let g = Arc::clone(&g);
            thread::spawn(move || g.start(&settings))
        };
        // The tree exists and its readiness is pending.
        assert!(wait_until(soon(), || observed.spawned() == 1));
        assert_eq!(g.status(), "starting");
        if shutdown {
            g.authority
                .shutdown_dev_servers(soon(), &g.audit())
                .unwrap();
        } else {
            g.stop();
        }
        assert_eq!(starter.join().unwrap(), Err("start cancelled"));
        assert_gone(observed.pid(), if shutdown { "shutdown" } else { "stop" });
        assert_eq!(g.owned(), "stopped");
        assert!(g.never_running());
        if shutdown {
            let observed = Observed::default();
            assert_eq!(g.start(&observed.settings()), Err("shutting down"));
            assert_eq!(observed.spawned(), 0);
        }
    }
}

#[test]
fn p0_002c4c3_packaged_duplicate_start_is_denied_without_a_second_process() {
    let g = Arc::new(Governed::new());
    let observed = Observed::default();
    let settings =
        observed.settings_with(Some(g.script("slow.cjs", &server(200, true, 3000, false))));
    let starter = {
        let g = Arc::clone(&g);
        thread::spawn(move || g.start(&settings))
    };
    assert!(wait_until(soon(), || observed.spawned() == 1));
    let second = Observed::default();
    for _ in 0..3 {
        assert_eq!(g.start(&second.settings()), Err("dev server busy"));
    }
    let url = starter
        .join()
        .unwrap()
        .unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
    // Also while Running.
    assert_eq!(g.start(&second.settings()), Err("dev server busy"));
    assert_eq!(second.spawned(), 0);
    g.stop();
    assert_gone(observed.pid(), "stop");
    assert!(closed(port_of(&url)));
}

#[test]
fn p0_002c4c3_packaged_identity_change_during_launch_is_never_running() {
    for case in ["registration", "react", "runtime_child"] {
        let g = Arc::new(Governed::new());
        let observed = Observed::default();
        let script = g.script("slow.cjs", &server(200, true, 3000, false));
        let mut settings = observed.settings_with(Some(script));
        if case == "registration" {
            // The registration is tombstoned as soon as the process exists.
            let catalog = Arc::clone(&g.authority.catalog);
            let project = catalog.lookup(Uuid::parse_str(&g.id).unwrap()).unwrap();
            let pids = Arc::clone(&observed.pids);
            settings.test.spawned = Some(Arc::new(move |pid| {
                pids.lock().unwrap().push(pid);
                catalog.invalidate(&project).unwrap();
            }));
        }
        let started = g.runtime("tmp").join("started");
        let starter = {
            let g = Arc::clone(&g);
            thread::spawn(move || g.start(&settings))
        };
        if case != "registration" {
            // Node is running and readiness is pending: replace a retained
            // directory (never adopted).
            assert!(
                wait_until(soon(), || started.exists()),
                "{}",
                observed.stderr()
            );
            let path = match case {
                "react" => g.root.join(REACT),
                _ => g.runtime("env"),
            };
            std::fs::rename(&path, path.with_extension("moved")).unwrap();
            std::fs::create_dir(&path).unwrap();
        }
        assert_eq!(
            starter.join().unwrap(),
            Err("identity denied"),
            "{case}: {}",
            observed.stderr()
        );
        assert_gone(observed.pid(), case);
        assert_eq!(g.owned(), "stopped", "{case}");
        assert!(
            g.event("builder.devserver.lifecycle.invalidated", "succeeded", case),
            "{case}"
        );
        assert!(g.never_running(), "{case}");
    }
}

#[test]
fn p0_002c4c3_packaged_root_exit_right_after_readiness_is_finalized() {
    let g = Governed::new();
    let observed = Observed::default();
    let script = g.script("serve-once.cjs", &server(200, true, 0, true));
    let started = g.start(&observed.settings_with(Some(script)));
    assert!(
        matches!(started, Ok(_) | Err("dev server exited")),
        "{started:?}: {}",
        observed.stderr()
    );
    assert_gone(observed.pid(), "early exit");
    assert!(wait_until(soon(), || g.owned() == "stopped"));
    assert!(wait_until(soon(), || g.event(
        "builder.devserver.lifecycle.exited",
        "succeeded",
        "exited"
    )));
}

#[test]
fn p0_002c4c3_packaged_monitor_stops_the_preview_on_identity_invalidation() {
    for case in ["react", "runtime_child"] {
        let g = Governed::new();
        let observed = Observed::default();
        let script = g.script("serve.cjs", &server(200, true, 0, false));
        let url = g
            .start(&observed.settings_with(Some(script)))
            .unwrap_or_else(|error| panic!("{error}: {}", observed.stderr()));
        let (port, pid) = (port_of(&url), observed.pid());
        assert_eq!(g.owned(), "running");
        let path = match case {
            "react" => g.root.join(REACT),
            _ => g.runtime("vite-cache"),
        };
        std::fs::rename(&path, path.with_extension("moved")).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(wait_until(soon(), || g.owned() == "stopped"), "{case}");
        assert_gone(pid, case);
        assert!(closed(port), "{case}");
        // The owner audits after completion.
        assert!(
            wait_until(soon(), || g.event(
                "builder.devserver.lifecycle.invalidated",
                "succeeded",
                case
            )),
            "{case}"
        );
    }
}

#[test]
fn p0_002c4c3_packaged_react_redirect_is_denied_before_spawn() {
    let g = Governed::new();
    let outside = temporary("outside");
    std::fs::write(outside.join("secret.ts"), b"export const secret = 1;\n").unwrap();
    let link = g.root.join(REACT).join("src").join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&outside, &link)
        .expect("native Windows test requires symlink creation privilege");
    let observed = Observed::default();
    assert_eq!(g.start(&observed.settings()), Err("React content denied"));
    assert_eq!(observed.spawned(), 0);
    assert!(g.event("builder.devserver.start", "failed", "react_redirected"));
    #[cfg(unix)]
    std::fs::remove_file(&link).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(&link).unwrap();
    std::fs::remove_dir_all(&outside).unwrap();
}
