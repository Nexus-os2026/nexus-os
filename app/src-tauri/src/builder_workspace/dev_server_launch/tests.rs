//! P0-002C4C3 launch tests and the test-only launch hooks.
//!
//! Every build: the readiness record and its bounded reader, the loopback
//! probe, the Node path spelling, the exact Node permission arguments and
//! sealed specification, the React walk and the governed runtime identities.
//! Packaged builds only (`nexus_packaged_toolchain`; CI's packaged step on
//! Linux, Windows and macOS): the real assembled Node, embedded manifest,
//! Nexus entry, permission model, sealed spawn, lifecycle and loopback server.
use super::super::tests::{registered, Fixture};
use super::*;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, PoisonError};

const STDERR_CAPTURE: usize = 64 * 1024;
const WAIT: Duration = Duration::from_secs(20);
const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// Test-only launch observation and substitution. Production settings never
/// carry hooks (`LaunchSettings::production`).
#[derive(Default)]
pub(in crate::builder_workspace) struct Hooks {
    /// Runs this script (inside a granted location) instead of the verified
    /// entry, through the same spelling conversion, with identical permission
    /// arguments, environment, working directory and launch request.
    pub(in crate::builder_workspace) script: Option<PathBuf>,
    pub(in crate::builder_workspace) spawned: Option<Arc<dyn Fn(u32) + Send + Sync>>,
    pub(in crate::builder_workspace) stderr: Option<Arc<Mutex<Vec<u8>>>>,
}

impl Hooks {
    pub(super) fn spawned(&self, id: u32) {
        if let Some(hook) = &self.spawned {
            hook(id);
        }
    }
}

pub(super) fn capture(sink: &Option<Arc<Mutex<Vec<u8>>>>, bytes: &[u8]) {
    if let Some(sink) = sink {
        let mut captured = sink.lock().unwrap_or_else(PoisonError::into_inner);
        let room = STDERR_CAPTURE.saturating_sub(captured.len());
        captured.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }
}

fn soon() -> Instant {
    Instant::now() + WAIT
}

// ── Readiness record and bounded reader ───────────────────────────────────

fn record(port: &str) -> Vec<u8> {
    format!(r#"{{"event":"ready","host":"127.0.0.1","port":{port}}}"#).into_bytes()
}

#[test]
fn p0_002c4c3_readiness_accepts_only_the_exact_record() {
    for (port, expected) in [("1", 1), ("5173", 5173), ("65535", 65535)] {
        assert_eq!(parse_readiness(&record(port)), Readiness::Ready(expected));
    }
    let mut malformed: Vec<Vec<u8>> = [
        "0", "65536", "05173", "-1", "5173.0", "1e3", "\"5173\"", "", " 5173", "99999",
    ]
    .iter()
    .map(|port| record(port))
    .collect();
    for host in [
        "0.0.0.0",
        "localhost",
        "::1",
        "[::1]",
        "127.0.0.2",
        "192.168.1.2",
        "example.com",
        "",
    ] {
        malformed.push(format!(r#"{{"event":"ready","host":"{host}","port":5173}}"#).into_bytes());
    }
    for text in [
        r#"{"host":"127.0.0.1","event":"ready","port":5173}"#,
        r#"{"event":"ready","host":"127.0.0.1","port":5173,"path":"/x"}"#,
        r#"{"event":"ready","host":"127.0.0.1","port":5173,"pid":1}"#,
        r#"{"event":"ready","host":"127.0.0.1","port":5173}x"#,
        r#" {"event":"ready","host":"127.0.0.1","port":5173}"#,
        r#"{"event":"ready", "host":"127.0.0.1","port":5173}"#,
        r#"{"event":"started","host":"127.0.0.1","port":5173}"#,
        r#"{"event":"ready","host":"127.0.0.1"}"#,
        r#"{"event":"ready","event":"ready","host":"127.0.0.1","port":5173}"#,
        "{\"event\":\"ready\",\"host\":\"127.0.0.1\",\"port\":5173}\r",
        "ready",
        "",
    ] {
        malformed.push(text.as_bytes().to_vec());
    }
    malformed.push(vec![0xff, 0xfe]);
    for line in malformed {
        assert_eq!(
            parse_readiness(&line),
            Readiness::Malformed,
            "{}",
            String::from_utf8_lossy(&line)
        );
    }
}

/// Delivers bytes in the given chunks, then end of stream.
struct Chunks(VecDeque<Vec<u8>>);

impl Read for Chunks {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let Some(mut chunk) = self.0.pop_front() else {
            return Ok(0);
        };
        let count = chunk.len().min(buffer.len());
        buffer[..count].copy_from_slice(&chunk[..count]);
        if count < chunk.len() {
            self.0.push_front(chunk.split_off(count));
        }
        Ok(count)
    }
}

fn chunks(parts: &[&[u8]]) -> Chunks {
    Chunks(parts.iter().map(|part| part.to_vec()).collect())
}

/// Fails once with `kind`, then reads from the inner stream.
struct FailingOnce(Option<std::io::ErrorKind>, Chunks);

impl Read for FailingOnce {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self.0.take() {
            Some(kind) => Err(kind.into()),
            None => self.1.read(buffer),
        }
    }
}

#[test]
fn p0_002c4c3_readiness_line_is_single_and_bounded() {
    let line = record("4321");
    let (a, rest) = line.split_at(10);
    let (b, c) = rest.split_at(17);
    // One line across reads; anything after it is not part of the record.
    let mut stream = chunks(&[a, b, c, b"\nsecond line\n", b"more"]);
    assert_eq!(first_line(&mut stream), Readiness::Ready(4321));
    // End of stream before a newline, or nothing at all.
    assert_eq!(first_line(&mut chunks(&[&line])), Readiness::Ended);
    assert_eq!(first_line(&mut chunks(&[])), Readiness::Ended);
    // More than the bound without a newline, in one or many writes.
    let long = vec![b'x'; READINESS_LINE_BYTES + 1];
    assert_eq!(first_line(&mut chunks(&[&long])), Readiness::Oversized);
    let huge = vec![b'x'; 1 << 20];
    let mut stream = chunks(&[&huge]);
    assert_eq!(first_line(&mut stream), Readiness::Oversized);
    // Only the bound (plus one byte) was consumed from the stream.
    let unread: usize = stream.0.iter().map(Vec::len).sum();
    assert_eq!(unread, (1 << 20) - READINESS_LINE_BYTES - 1);
    let many: Vec<&[u8]> = vec![b"xx"; READINESS_LINE_BYTES];
    assert_eq!(first_line(&mut chunks(&many)), Readiness::Oversized);
    // A line of exactly the bound is read, and judged malformed.
    let mut exact = vec![b'x'; READINESS_LINE_BYTES];
    exact.push(b'\n');
    assert_eq!(first_line(&mut chunks(&[&exact])), Readiness::Malformed);
    // Interruptions are retried; any other read error ends the record.
    let mut complete = line.clone();
    complete.push(b'\n');
    let interrupted = Some(std::io::ErrorKind::Interrupted);
    assert_eq!(
        first_line(&mut FailingOnce(interrupted, chunks(&[&complete]))),
        Readiness::Ready(4321)
    );
    let broken = Some(std::io::ErrorKind::BrokenPipe);
    assert_eq!(
        first_line(&mut FailingOnce(broken, chunks(&[&complete]))),
        Readiness::Ended
    );
}

#[test]
fn p0_002c4c3_readiness_wait_is_bounded_and_cancellable() {
    let (_sender, silent) = sync_channel::<Readiness>(1);
    let began = Instant::now();
    assert_eq!(
        await_readiness(
            &silent,
            Instant::now() + Duration::from_millis(200),
            &|| false
        ),
        Err(ProofFailure::Failed("readiness_timeout"))
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    let began = Instant::now();
    assert_eq!(
        await_readiness(&silent, soon(), &|| true),
        Err(ProofFailure::Cancelled)
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    for (readiness, expected) in [
        (Readiness::Ready(9), Ok(9)),
        (
            Readiness::Malformed,
            Err(ProofFailure::Failed("readiness_malformed")),
        ),
        (
            Readiness::Oversized,
            Err(ProofFailure::Failed("readiness_oversized")),
        ),
        (
            Readiness::Ended,
            Err(ProofFailure::Failed("exited_before_ready")),
        ),
    ] {
        let (sender, receiver) = sync_channel(1);
        sender.send(readiness).unwrap();
        assert_eq!(await_readiness(&receiver, soon(), &|| false), expected);
    }
    let (sender, receiver) = sync_channel::<Readiness>(1);
    drop(sender);
    assert_eq!(
        await_readiness(&receiver, soon(), &|| false),
        Err(ProofFailure::Failed("exited_before_ready"))
    );
}

// ── Loopback probe ────────────────────────────────────────────────────────

/// One loopback connection: records the request head, answers `response`.
fn respond_once(response: Vec<u8>) -> (u16, thread::JoinHandle<String>) {
    let listener = TcpListener::bind((LOOPBACK, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        let mut head = Vec::new();
        let mut buffer = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => head.extend_from_slice(&buffer[..read]),
            }
        }
        let _ = stream.write_all(&response);
        String::from_utf8_lossy(&head).into_owned()
    });
    (port, server)
}

#[test]
fn p0_002c4c3_probe_accepts_only_a_tokened_200_and_follows_no_redirect() {
    for header in ["x-nexus-preview-token", "X-Nexus-Preview-Token"] {
        let ok = format!("HTTP/1.1 200 OK\r\n{header}: {TOKEN}\r\nContent-Length: 0\r\n\r\n");
        let (port, server) = respond_once(ok.into_bytes());
        assert_eq!(probe(port, TOKEN, soon(), &|| false), Ok(()));
        let request = server.join().unwrap();
        // Exactly GET / to the announced loopback authority.
        assert!(request.starts_with("GET / HTTP/1.1\r\n"), "{request}");
        assert!(
            request.contains(&format!("\r\nHost: 127.0.0.1:{port}\r\n")),
            "{request}"
        );
    }
    // A redirect target that must never be contacted.
    let elsewhere = TcpListener::bind((LOOPBACK, 0)).unwrap();
    elsewhere.set_nonblocking(true).unwrap();
    let target = elsewhere.local_addr().unwrap().port();
    let other = "f".repeat(32);
    for response in [
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_owned(),
        format!("HTTP/1.1 200 OK\r\nx-nexus-preview-token: {other}\r\n\r\n"),
        format!("HTTP/1.1 200 OK\r\nx-nexus-preview-token: {TOKEN}x\r\n\r\n"),
        format!(
            "HTTP/1.1 200 OK\r\nx-nexus-preview-token: {TOKEN}\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"
        ),
        format!(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{target}/\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"
        ),
        format!(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: /elsewhere\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"
        ),
        format!("HTTP/1.1 500 Internal Server Error\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"),
        format!("HTTP/1.1 403 Forbidden\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"),
        format!("HTTP/1.0 200 OK\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"),
        format!("HTTP/1.1 2000 OK\r\nx-nexus-preview-token: {TOKEN}\r\n\r\n"),
        format!("x-nexus-preview-token: {TOKEN}\r\n\r\n"),
        format!(
            "HTTP/1.1 200 OK\r\nx-nexus-preview-token: {TOKEN}\r\nx-padding: {}\r\n\r\n",
            "x".repeat(PROBE_HEAD_BYTES)
        ),
        "garbage".to_owned(),
        String::new(),
    ] {
        let (port, server) = respond_once(response.clone().into_bytes());
        assert_eq!(
            probe(port, TOKEN, soon(), &|| false),
            Err(ProofFailure::Failed("probe_failed")),
            "{response:.120}"
        );
        server.join().unwrap();
    }
    assert!(elsewhere.accept().is_err(), "a redirect was followed");
    // Nothing listening.
    let closed = TcpListener::bind((LOOPBACK, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_eq!(
        probe(closed, TOKEN, soon(), &|| false),
        Err(ProofFailure::Failed("probe_failed"))
    );
    // A listener that never answers: bounded by the deadline, or cancelled.
    let silent = TcpListener::bind((LOOPBACK, 0)).unwrap();
    let port = silent.local_addr().unwrap().port();
    let began = Instant::now();
    assert_eq!(
        probe(
            port,
            TOKEN,
            Instant::now() + Duration::from_millis(300),
            &|| false
        ),
        Err(ProofFailure::Failed("probe_failed"))
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!(
        probe(port, TOKEN, soon(), &|| true),
        Err(ProofFailure::Cancelled)
    );
    drop(silent);
}

// ── Node path spelling ────────────────────────────────────────────────────

/// A canonical temporary directory whose name contains spaces.
pub(super) fn temporary(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus c4c3 {name} {}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn p0_002c4c3_node_spelling_accepts_only_the_retained_canonical_form() {
    let base = temporary("spelling");
    let dir = base.join("with space");
    std::fs::create_dir(&dir).unwrap();
    let canonical = dir.canonicalize().unwrap();
    let spelled = node_spelling(&canonical).unwrap();
    assert!(spelled.to_str().unwrap().ends_with("with space"));
    #[cfg(unix)]
    assert_eq!(spelled, canonical);
    #[cfg(windows)]
    {
        let verbatim = canonical.to_str().unwrap();
        assert!(verbatim.starts_with(r"\\?\"), "{verbatim}");
        assert_eq!(spelled, PathBuf::from(&verbatim[4..]));
        assert_eq!(spelled.to_str().unwrap().as_bytes()[1], b':');
    }
    let separator = std::path::MAIN_SEPARATOR;
    let text = canonical.to_str().unwrap();
    let (parent, name) = text.rsplit_once(separator).unwrap();
    for rejected in [
        PathBuf::from("relative").join("dir"),
        canonical.join("missing"),
        canonical.join("..").join("with space"),
        canonical.join("."),
        PathBuf::from(format!("{text}{separator}")),
        PathBuf::from(format!("{parent}{separator}{separator}{name}")),
        PathBuf::from(format!("{parent}{separator}.{separator}{name}")),
    ] {
        assert_eq!(
            node_spelling(&rejected),
            Err("path_unsupported"),
            "{rejected:?}"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        // A Node permission wildcard, a control character and non-UTF-8.
        for name in [&b"a*b"[..], b"line\nbreak", b"\xff\xfe"] {
            let path = base.join(std::ffi::OsStr::from_bytes(name));
            if std::fs::create_dir(&path).is_err() {
                // The filesystem refuses the name itself (e.g. APFS rejects
                // non-UTF-8): the spelling check alone is exercised.
                assert_eq!(node_spelling(&path), Err("path_unsupported"), "{path:?}");
                continue;
            }
            let path = path.canonicalize().unwrap();
            assert_eq!(node_spelling(&path), Err("path_unsupported"), "{path:?}");
        }
        // Another spelling of the same directory (a symlink) is not the path.
        let link = base.join("alias");
        std::os::unix::fs::symlink(&canonical, &link).unwrap();
        assert_eq!(node_spelling(&link), Err("path_unsupported"));
    }
    #[cfg(windows)]
    windows_spellings(&base, &canonical, &spelled);
    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(windows)]
fn windows_spellings(base: &Path, canonical: &Path, spelled: &Path) {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    // Only a verbatim local-drive path: never its plain spelling as input, UNC,
    // verbatim UNC, device or GLOBALROOT paths, or a wildcard.
    let text = canonical.to_str().unwrap();
    for rejected in [
        spelled.to_path_buf(),
        PathBuf::from(r"\\server\share\dir"),
        PathBuf::from(r"\\?\UNC\server\share\dir"),
        PathBuf::from(format!(r"\\.\{}", &text[4..])),
        PathBuf::from(format!(r"\\?\GLOBALROOT\{}", &text[4..])),
        PathBuf::from(format!("{text}*")),
    ] {
        assert_eq!(
            node_spelling(&rejected),
            Err("path_unsupported"),
            "{rejected:?}"
        );
    }
    // A directory symlink spelling is not the retained path.
    let link = base.join("alias");
    std::os::windows::fs::symlink_dir(canonical, &link)
        .expect("native Windows test requires symlink creation privilege");
    assert_eq!(node_spelling(&link), Err("path_unsupported"));
    // 8.3 aliases of long names, in verbatim and plain form. At least one
    // alias must be exercised on this machine.
    let short = |long: &Path| -> Option<PathBuf> {
        let wide: Vec<u16> = long.as_os_str().encode_wide().chain([0]).collect();
        let mut buffer = vec![0u16; 1024];
        // SAFETY: NUL-terminated input; the output buffer length is passed.
        let written = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetShortPathNameW(
                wide.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        } as usize;
        (written > 0 && written < buffer.len())
            .then(|| PathBuf::from(std::ffi::OsString::from_wide(&buffer[..written])))
            .filter(|alias| alias.as_os_str() != long.as_os_str())
    };
    let mut aliases = Vec::new();
    for long in [
        canonical.to_path_buf(),
        std::env::temp_dir().canonicalize().unwrap(),
        PathBuf::from(r"\\?\C:\Program Files"),
    ] {
        aliases.extend(short(&long));
    }
    if let Some(temp) = std::env::var_os("TEMP") {
        let temp = temp.to_string_lossy().into_owned();
        if temp.contains('~') {
            aliases.push(PathBuf::from(format!(r"\\?\{temp}")));
        }
    }
    assert!(!aliases.is_empty(), "no 8.3 alias available to exercise");
    for alias in aliases {
        let plain = alias
            .to_str()
            .unwrap()
            .trim_start_matches(r"\\?\")
            .to_owned();
        for spelling in [alias.clone(), PathBuf::from(&plain)] {
            assert_eq!(
                node_spelling(&spelling),
                Err("path_unsupported"),
                "{spelling:?}"
            );
        }
    }
}

// ── Exact Node invocation ─────────────────────────────────────────────────

fn synthetic_paths(base: &Path) -> LaunchPaths {
    let toolchain = base.join("toolchain");
    let project = base.join("Builder Storage").join("project");
    let runtime = project.join(RUNTIME);
    LaunchPaths {
        node: toolchain.join("node").join("node"),
        entry: toolchain.join("entry").join("nexus-builder.mjs"),
        toolchain: toolchain.clone(),
        react: project.join(REACT),
        home: runtime.join(RUNTIME_HOME),
        tmp: runtime.join(RUNTIME_TMP),
        cache: runtime.join(RUNTIME_CACHE),
        home_dir: runtime.join(RUNTIME_HOME),
        tmp_dir: runtime.join(RUNTIME_TMP),
        runtime_dir: runtime,
    }
}

fn texts(arguments: &[OsString]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| argument.to_str().unwrap().to_owned())
        .collect()
}

#[test]
fn p0_002c4c3_node_permission_arguments_are_exact() {
    let base = PathBuf::from(if cfg!(windows) {
        r"C:\nexus base"
    } else {
        "/nexus base"
    });
    let paths = synthetic_paths(&base);
    let show = |path: &Path| path.to_str().unwrap().to_owned();
    let expected = vec![
        "--permission".to_owned(),
        "--allow-addons".to_owned(),
        format!("--allow-fs-read={}", show(&paths.toolchain)),
        format!("--allow-fs-read={}", show(&paths.react)),
        format!("--allow-fs-read={}", show(&paths.home)),
        format!("--allow-fs-read={}", show(&paths.tmp)),
        format!("--allow-fs-read={}", show(&paths.cache)),
        format!("--allow-fs-write={}", show(&paths.home)),
        format!("--allow-fs-write={}", show(&paths.tmp)),
        format!("--allow-fs-write={}", show(&paths.cache)),
    ];
    let permissions = texts(&permission_arguments(&paths));
    assert_eq!(permissions, expected);
    let project = base.join("Builder Storage").join("project");
    for argument in &permissions {
        for forbidden in [
            "--allow-child-process",
            "--allow-worker",
            "--allow-wasi",
            "--allow-inspector",
            "--allow-openssl-store",
            "--allow-net",
            "--permission-audit",
        ] {
            assert!(!argument.starts_with(forbidden), "{argument}");
        }
        assert!(!argument.contains('*'), "{argument}");
        // Never the storage root, the project, runtime/ itself or runtime/env.
        for denied in [
            base.clone(),
            base.join("Builder Storage"),
            project.clone(),
            project.join(RUNTIME),
            project.join(RUNTIME).join("env"),
        ] {
            assert!(
                !argument.ends_with(&format!("={}", show(&denied))),
                "{argument}"
            );
        }
    }
    // React and the toolchain are never writable.
    let writes: Vec<&String> = permissions
        .iter()
        .filter(|argument| argument.starts_with("--allow-fs-write="))
        .collect();
    assert_eq!(writes.len(), 3);
    assert!(writes
        .iter()
        .all(|w| !w.contains("react") && !w.contains("toolchain")));
    // Flags, then the script, then exactly one launch request.
    let arguments = node_arguments(&paths, &paths.entry, TOKEN).unwrap();
    assert_eq!(arguments.len(), expected.len() + 2);
    assert_eq!(texts(&arguments[..expected.len()]), expected);
    assert_eq!(PathBuf::from(&arguments[expected.len()]), paths.entry);
    let request: Value =
        serde_json::from_str(arguments[expected.len() + 1].to_str().unwrap()).unwrap();
    assert_eq!(
        request,
        json!({"cacheDir": show(&paths.cache), "probeToken": TOKEN, "projectRoot": show(&paths.react)})
    );
}

#[test]
fn p0_002c4c3_sealed_launch_specification_is_exact() {
    let f = Fixture::new();
    let (_, root) = registered(&f);
    let runtime = root.join(RUNTIME);
    let paths = LaunchPaths {
        node: root.join("node"),
        entry: root.join("entry.mjs"),
        toolchain: root.join("toolchain"),
        react: root.join(REACT),
        home: runtime.join(RUNTIME_HOME),
        tmp: runtime.join(RUNTIME_TMP),
        cache: runtime.join(RUNTIME_CACHE),
        home_dir: runtime.join(RUNTIME_HOME),
        tmp_dir: runtime.join(RUNTIME_TMP),
        runtime_dir: runtime.clone(),
    };
    let arguments = node_arguments(&paths, &paths.entry, TOKEN).unwrap();
    let spec = sealed_spec(&paths, arguments.clone()).unwrap();
    assert_eq!(spec.program, paths.node);
    assert_eq!(spec.args, arguments);
    // The governed runtime root is the working directory, never React.
    assert_eq!(spec.current_dir, runtime);
    assert!(matches!(spec.stdout, ResourceOutput::Piped));
    assert!(matches!(spec.stderr, ResourceOutput::Piped));
    // Nothing but the typed runtime home and temp directory: no caller
    // variable (PATH, NODE_OPTIONS, BROWSER, npm configuration or .env).
    assert_eq!(
        format!("{:?}", spec.environment),
        "SealedEnvironment { variables: [], .. }"
    );
}

// ── React content and governed runtime identities ─────────────────────────

fn walk_with(f: &Fixture, id: &str, budget: usize) -> Result<(), &'static str> {
    let project = f
        .authority
        .catalog
        .lookup(Uuid::parse_str(id).unwrap())
        .unwrap();
    native::walk_react(
        &project.root.join(REACT),
        project.workspace.get().unwrap(),
        budget,
    )
}

fn walk(f: &Fixture, id: &str) -> Result<(), &'static str> {
    walk_with(f, id, MAX_REACT_ENTRIES)
}

pub(super) fn plant_content(react: &Path) {
    for (path, content) in [
        ("index.html", "<!doctype html><div id=\"root\"></div>"),
        ("src/main.tsx", "export {}\n"),
        ("src/pages/deep/Home.tsx", "export {}\n"),
        ("public/favicon.svg", "<svg/>"),
    ] {
        let file = path
            .split('/')
            .fold(react.to_path_buf(), |at, part| at.join(part));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, content).unwrap();
    }
}

#[test]
fn p0_002c4c3_react_walk_accepts_only_directories_and_regular_files() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join(REACT);
    plant_content(&react);
    std::fs::create_dir(react.join("empty")).unwrap();
    assert_eq!(walk(&f, &id), Ok(()));
    let outside = temporary("outside");
    std::fs::write(outside.join("secret"), b"outside").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        for (name, target) in [
            ("leak.ts", outside.join("secret")),
            ("linked", outside.clone()),
            ("dangling", outside.join("missing")),
        ] {
            let link = react.join("src").join(name);
            symlink(&target, &link).unwrap();
            assert_eq!(walk(&f, &id), Err("react_redirected"), "{name}");
            std::fs::remove_file(&link).unwrap();
        }
        // A FIFO (never opened: it would block a reader).
        let fifo = react.join("src").join("pipe");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        assert_eq!(walk(&f, &id), Err("react_special"));
        std::fs::remove_file(&fifo).unwrap();
        // A socket, bound where the path length allows and moved into React.
        let bound = PathBuf::from(format!(
            "/tmp/nx-{}",
            &Uuid::new_v4().simple().to_string()[..8]
        ));
        let listener = std::os::unix::net::UnixListener::bind(&bound).unwrap();
        let socket = react.join("public").join("socket");
        std::fs::rename(&bound, &socket).unwrap();
        assert_eq!(walk(&f, &id), Err("react_special"));
        drop(listener);
        std::fs::remove_file(&socket).unwrap();
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let file = react.join("src").join("leak.ts");
        symlink_file(outside.join("secret"), &file)
            .expect("native Windows test requires symlink creation privilege");
        assert_eq!(walk(&f, &id), Err("react_redirected"));
        std::fs::remove_file(&file).unwrap();
        let dir = react.join("src").join("linked");
        symlink_dir(&outside, &dir)
            .expect("native Windows test requires symlink creation privilege");
        assert_eq!(walk(&f, &id), Err("react_redirected"));
        std::fs::remove_dir(&dir).unwrap();
        // A directory junction (a mount-point reparse point).
        let junction = react.join("public").join("junction");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        assert_eq!(walk(&f, &id), Err("react_redirected"));
        std::fs::remove_dir(&junction).unwrap();
    }
    assert_eq!(walk(&f, &id), Ok(()));
    // Bounded: depth and entries.
    let mut deep = react.join("d");
    for _ in 0..MAX_REACT_DEPTH {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    assert_eq!(walk(&f, &id), Err("react_limit"));
    std::fs::remove_dir_all(react.join("d")).unwrap();
    // index.html, src, main.tsx, pages, deep, Home.tsx, public, favicon, empty.
    assert_eq!(walk_with(&f, &id, 9), Ok(()));
    assert_eq!(walk_with(&f, &id, 8), Err("react_limit"));
    // Bound to the retained React identity, never a replacement directory.
    std::fs::rename(&react, root.join("react-original")).unwrap();
    std::fs::create_dir(&react).unwrap();
    assert_eq!(walk(&f, &id), Err("react"));
    std::fs::remove_dir_all(&outside).unwrap();
}

#[test]
fn p0_002c4c3_runtime_children_must_be_the_retained_directories() {
    use super::super::workspace_provisioning::RUNTIME_CHILDREN;
    for child in RUNTIME_CHILDREN {
        let f = Fixture::new();
        let (id, root) = registered(&f);
        let path = root.join(RUNTIME).join(child);
        let original = path.with_extension("original");
        let start = |f: &Fixture| f.authority.dev_server_start(&id, f.audit());
        // Replaced by a new directory at the same path: never adopted.
        std::fs::rename(&path, &original).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(start(&f), Err("runtime identity denied"), "{child}");
        std::fs::remove_dir(&path).unwrap();
        // Redirected to the original directory by a link.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&original, &path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&original, &path)
            .expect("native Windows test requires symlink creation privilege");
        assert_eq!(start(&f), Err("runtime identity denied"), "{child}");
        #[cfg(unix)]
        std::fs::remove_file(&path).unwrap();
        #[cfg(windows)]
        std::fs::remove_dir(&path).unwrap();
        // Missing.
        assert_eq!(start(&f), Err("runtime identity denied"), "{child}");
        // Denied at selection: the lifecycle was never reached.
        assert!(!f
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event["operation"] == "builder.devserver.lifecycle.reserve"));
        // The retained directory itself restores governed use.
        std::fs::rename(&original, &path).unwrap();
        assert_eq!(start(&f), Err("trusted toolchain unavailable"), "{child}");
    }
}

#[cfg(not(nexus_packaged_toolchain))]
#[test]
fn p0_002c4c3_unpackaged_start_fails_closed_before_any_process() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    plant_content(&root.join(REACT));
    let spawned = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&spawned);
    let mut settings = LaunchSettings::production();
    settings.test.spawned = Some(Arc::new(move |pid| observed.lock().unwrap().push(pid)));
    for _ in 0..2 {
        assert_eq!(
            f.authority.dev_server_start_with(&id, f.audit(), &settings),
            Err("trusted toolchain unavailable")
        );
    }
    // Without an embedded manifest nothing verifies, not even a toolchain
    // tree that exists on disk (here: an arbitrary directory).
    let tree = temporary("unpackaged toolchain");
    settings.toolchain = ToolchainSource::Assembled(tree.clone());
    assert_eq!(
        f.authority.dev_server_start_with(&id, f.audit(), &settings),
        Err("trusted toolchain unavailable")
    );
    std::fs::remove_dir_all(&tree).unwrap();
    // No system Node, PATH, repository node_modules, npm or npx fallback.
    assert!(spawned.lock().unwrap().is_empty());
    assert!(f.events.lock().unwrap().iter().any(|event| {
        event["operation"] == "builder.devserver.start"
            && event["outcome"] == "failed"
            && event["reason"] == "toolchain_unavailable"
    }));
    assert_eq!(
        f.authority.dev_server_status(&id, f.audit()).unwrap(),
        json!({"status": "stopped"})
    );
}

#[test]
fn p0_002c4c3_denials_and_urls_are_bounded() {
    assert_eq!(
        PreviewEndpoint { port: 5173 }.url(),
        "http://127.0.0.1:5173/"
    );
    for (reason, client) in [
        ("toolchain_unavailable", "trusted toolchain unavailable"),
        ("toolchain_rejected", "trusted toolchain unavailable"),
        ("registration", "registration identity denied"),
        ("react", "React identity denied"),
        ("runtime_child", "runtime identity denied"),
        ("react_redirected", "React content denied"),
        ("react_special", "React content denied"),
        ("react_limit", "React content denied"),
        ("path_unsupported", "workspace location unsupported"),
        ("spawn_failed", "launch failed"),
        ("environment", "launch failed"),
    ] {
        assert_eq!(denial(reason), client, "{reason}");
    }
}

#[cfg(nexus_packaged_toolchain)]
mod packaged;
