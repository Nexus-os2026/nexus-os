//! P0-002C4C3: governed Builder dev-server launch.
//!
//! `launch` is only ever the LifecycleRegistry launcher: it runs after the
//! registry reserved a generation and revalidated the retained target, with no
//! lock held. Every launch freshly verifies the packaged toolchain (a
//! verification is never cached or reused), walks the React content, and
//! revalidates every identity immediately before the process exists. It then
//! runs exactly the verified Nexus Node against the verified Nexus entry
//! through `ResourceLimiter::spawn_sealed`:
//!
//! * the Node permission model: read only the toolchain, React and the runtime
//!   home, tmp and Vite cache; write only those three runtime children;
//!   native addons (the entry's module guard loads code, including addons,
//!   only from the verified toolchain); no child-process, worker, WASI,
//!   inspector or OpenSSL-store permission;
//! * an empty sealed environment: only the runtime home and temp directory;
//! * the governed `runtime/` directory as working directory.
//!
//! The readiness proof accepts exactly one bounded line, the canonical JSON
//! `{"event":"ready","host":"127.0.0.1","port":N}`, then a bounded `GET /`
//! to that loopback port must return 200 with this launch's probe token (no
//! redirect is followed). Only then may the registry publish Running.
//!
//! Not claimed: filesystem or network sandboxing; safe execution of arbitrary
//! project JavaScript; protection against a hostile same-user process racing
//! filesystem state (the React walk, identity checks and verification are
//! point-in-time, and hard links are not detected); atomic toolchain
//! verification; protection from every Node permission-model limitation (a
//! native addon is outside the model; only the verified toolchain's load);
//! production web-server suitability.
use super::process_lifecycle::{Launched, ProofFailure};
use super::trusted_toolchain::{verify_installed, ToolchainError, VerifiedToolchain};
use super::workspace_provisioning::{ProvisionedWorkspace, REACT, RUNTIME};
use super::DevServerTarget;
use nexus_kernel::resource_limiter::{
    ResourceLimiter, ResourceOutput, ResourceReader, SealedEnvironment, SealedSpawnSpec,
};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// The only address the preview binds and the probe connects to.
pub(super) const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;
const READINESS_WAIT: Duration = Duration::from_secs(60);
const PROBE_WAIT: Duration = Duration::from_secs(20);
const READINESS_LINE_BYTES: usize = 256;
const PROBE_HEAD_BYTES: usize = 16 * 1024;
const POLL: Duration = Duration::from_millis(50);
const TOKEN_HEADER: &str = "x-nexus-preview-token";
const MAX_REACT_ENTRIES: usize = 10_000;
const MAX_REACT_DEPTH: usize = 32;
const RUNTIME_HOME: &str = "home";
const RUNTIME_TMP: &str = "tmp";
const RUNTIME_CACHE: &str = "vite-cache";

/// Where a launch verifies the toolchain. Production has exactly one source:
/// the installed toolchain beside this executable, against the embedded
/// manifest. No caller, project or frontend value selects a toolchain, Node
/// or entry path.
pub(super) enum ToolchainSource {
    Installed,
    /// Test builds only: the real assembled toolchain at `root`, verified
    /// against the same embedded manifest.
    #[cfg(test)]
    Assembled(PathBuf),
}

impl ToolchainSource {
    fn verify(&self) -> Result<VerifiedToolchain, ToolchainError> {
        match self {
            Self::Installed => verify_installed(),
            #[cfg(test)]
            Self::Assembled(root) => super::trusted_toolchain::verify_assembled(root),
        }
    }
}

/// Fixed production settings; tests narrow the bounds and add observation.
pub(super) struct LaunchSettings {
    pub(super) toolchain: ToolchainSource,
    pub(super) readiness_wait: Duration,
    pub(super) probe_wait: Duration,
    #[cfg(test)]
    pub(super) test: tests::Hooks,
}

impl LaunchSettings {
    pub(super) fn production() -> Self {
        Self {
            toolchain: ToolchainSource::Installed,
            readiness_wait: READINESS_WAIT,
            probe_wait: PROBE_WAIT,
            #[cfg(test)]
            test: tests::Hooks::default(),
        }
    }
}

/// A proven loopback preview. Only its port leaves the backend, inside the
/// fixed loopback URL.
pub(super) struct PreviewEndpoint {
    port: u16,
}

impl PreviewEndpoint {
    pub(super) fn url(&self) -> String {
        format!("http://{LOOPBACK}:{}/", self.port)
    }
}

/// Bounded client error for a launch denied before any process existed.
pub(super) fn denial(reason: &str) -> &'static str {
    match reason {
        "toolchain_unavailable" | "toolchain_rejected" => "trusted toolchain unavailable",
        "registration" => "registration identity denied",
        "workspace" | "react" => "React identity denied",
        "runtime" | "runtime_child" => "runtime identity denied",
        reason if reason.starts_with("react_") => "React content denied",
        "path_unsupported" => "workspace location unsupported",
        _ => "launch failed",
    }
}

/// The lifecycle launcher. `Err` only while no process exists; after spawn
/// the tree is always returned, and any later failure is the proof's, so the
/// registry finalizes the tree.
pub(super) fn launch(
    target: &DevServerTarget,
    settings: &LaunchSettings,
    revalidate: &dyn Fn() -> Result<(), &'static str>,
) -> Result<Launched<PreviewEndpoint>, &'static str> {
    let workspace = target.project.workspace.get().ok_or("workspace")?;
    let root = &target.project.root;
    walk_react(&root.join(REACT), workspace)?;
    // Freshly for this launch, immediately before the spawn; never cached.
    let toolchain = settings.toolchain.verify().map_err(|error| match error {
        ToolchainError::Unavailable => "toolchain_unavailable",
        _ => "toolchain_rejected",
    })?;
    let paths = LaunchPaths::new(&toolchain, root)?;
    let token = Uuid::new_v4().simple().to_string();
    #[cfg(not(test))]
    let script = paths.entry.clone();
    #[cfg(test)]
    let script = match &settings.test.script {
        Some(script) => node_spelling(script)?,
        None => paths.entry.clone(),
    };
    let spec = sealed_spec(&paths, node_arguments(&paths, &script, &token)?)?;
    // Last check before the process exists; nothing runs in between.
    revalidate()?;
    let mut child = ResourceLimiter::default()
        .spawn_sealed(&spec)
        .map_err(|_| "spawn_failed")?;
    #[cfg(test)]
    settings.test.spawned(child.id());
    let readiness_deadline = Instant::now() + settings.readiness_wait;
    let probe_wait = settings.probe_wait;
    let pipes = (child.take_stdout(), child.take_stderr());
    #[cfg(test)]
    let stderr_sink = settings.test.stderr.clone();
    let proof = move |cancelled: &dyn Fn() -> bool| {
        let (Some(stdout), Some(stderr)) = pipes else {
            return Err(ProofFailure::Failed("pipes_unavailable"));
        };
        #[cfg(not(test))]
        drain(stderr, |_| {})?;
        #[cfg(test)]
        drain(stderr, move |bytes| tests::capture(&stderr_sink, bytes))?;
        let readiness = read_readiness(stdout)?;
        let port = await_readiness(&readiness, readiness_deadline, cancelled)?;
        probe(port, &token, Instant::now() + probe_wait, cancelled)?;
        Ok(PreviewEndpoint { port })
    };
    Ok(Launched {
        tree: Box::new(child),
        proof: Box::new(proof),
    })
}

// ── Locations ─────────────────────────────────────────────────────────────

/// Backend-derived locations: `node_*` as Node receives them, and the
/// retained canonical spellings the kernel's sealed spawn requires.
pub(super) struct LaunchPaths {
    pub(super) node: PathBuf,
    pub(super) entry: PathBuf,
    pub(super) toolchain: PathBuf,
    pub(super) react: PathBuf,
    pub(super) home: PathBuf,
    pub(super) tmp: PathBuf,
    pub(super) cache: PathBuf,
    runtime_dir: PathBuf,
    home_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl LaunchPaths {
    pub(super) fn new(
        toolchain: &VerifiedToolchain,
        project_root: &Path,
    ) -> Result<Self, &'static str> {
        let runtime = project_root.join(RUNTIME);
        Ok(Self {
            node: node_spelling(&toolchain.node_executable())?,
            entry: node_spelling(&toolchain.entry_module())?,
            toolchain: node_spelling(toolchain.root())?,
            react: node_spelling(&project_root.join(REACT))?,
            home: node_spelling(&runtime.join(RUNTIME_HOME))?,
            tmp: node_spelling(&runtime.join(RUNTIME_TMP))?,
            cache: node_spelling(&runtime.join(RUNTIME_CACHE))?,
            home_dir: runtime.join(RUNTIME_HOME),
            tmp_dir: runtime.join(RUNTIME_TMP),
            runtime_dir: runtime,
        })
    }
}

/// The one backend-owned conversion of a retained canonical path into the
/// spelling Node receives, which must resolve back to exactly that path.
/// Windows: only a verbatim local-drive path, as its plain long form; UNC,
/// verbatim UNC, device, relative and 8.3-alias spellings are rejected (a
/// mapped network drive canonicalizes to UNC). Everywhere: UTF-8 only, no
/// control characters and no `*` (a Node permission wildcard).
pub(super) fn node_spelling(canonical: &Path) -> Result<PathBuf, &'static str> {
    let text = canonical.to_str().ok_or("path_unsupported")?;
    if !canonical.is_absolute() || text.contains('*') || text.chars().any(char::is_control) {
        return Err("path_unsupported");
    }
    let spelling = plain_spelling(canonical)?;
    let resolved = spelling.canonicalize().map_err(|_| "path_unsupported")?;
    // Textual identity: `Path` equality would ignore `.` and repeated or
    // trailing separators.
    if resolved.as_os_str() != canonical.as_os_str() {
        return Err("path_unsupported");
    }
    Ok(spelling)
}

#[cfg(windows)]
fn plain_spelling(canonical: &Path) -> Result<PathBuf, &'static str> {
    use std::path::{Component, Prefix};
    let mut components = canonical.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err("path_unsupported");
    };
    let Prefix::VerbatimDisk(letter) = prefix.kind() else {
        return Err("path_unsupported");
    };
    if components.next() != Some(Component::RootDir) {
        return Err("path_unsupported");
    }
    let mut plain = PathBuf::from(format!("{}:\\", char::from(letter)));
    for component in components {
        match component {
            Component::Normal(name) => plain.push(name),
            _ => return Err("path_unsupported"),
        }
    }
    Ok(plain)
}

#[cfg(not(windows))]
fn plain_spelling(canonical: &Path) -> Result<PathBuf, &'static str> {
    Ok(canonical.to_path_buf())
}

// ── Node invocation ───────────────────────────────────────────────────────

/// The Node permission model of every Builder launch, one argument per flag
/// and path (no shell, no quoting). Never `*`, never a project ancestor,
/// storage root, user profile, OS temp or `runtime/` itself; never
/// child-process, worker, WASI, inspector or OpenSSL-store permission.
pub(super) fn permission_arguments(paths: &LaunchPaths) -> Vec<OsString> {
    let grant = |access: &str, path: &Path| {
        let mut argument = OsString::from(format!("--allow-fs-{access}="));
        argument.push(path);
        argument
    };
    vec![
        OsString::from("--permission"),
        OsString::from("--allow-addons"),
        grant("read", &paths.toolchain),
        grant("read", &paths.react),
        grant("read", &paths.home),
        grant("read", &paths.tmp),
        grant("read", &paths.cache),
        grant("write", &paths.home),
        grant("write", &paths.tmp),
        grant("write", &paths.cache),
    ]
}

/// Permission arguments, the script, then the one launch request: the React
/// root, the Vite cache and this launch's probe token.
fn node_arguments(
    paths: &LaunchPaths,
    script: &Path,
    token: &str,
) -> Result<Vec<OsString>, &'static str> {
    let path = |path: &Path| path.to_str().map(str::to_owned).ok_or("path_unsupported");
    let request = serde_json::json!({
        "cacheDir": path(&paths.cache)?,
        "probeToken": token,
        "projectRoot": path(&paths.react)?,
    });
    let mut arguments = permission_arguments(paths);
    arguments.push(script.as_os_str().to_owned());
    arguments.push(request.to_string().into());
    Ok(arguments)
}

/// The sealed launch: the verified Node only, an empty environment apart from
/// the runtime home and temp directory, and `runtime/` as working directory.
fn sealed_spec(
    paths: &LaunchPaths,
    arguments: Vec<OsString>,
) -> Result<SealedSpawnSpec, &'static str> {
    let environment = SealedEnvironment::builder()
        .home_dir(&paths.home_dir)
        .and_then(|builder| builder.temp_dir(&paths.tmp_dir))
        .and_then(|builder| builder.build())
        .map_err(|_| "environment")?;
    Ok(SealedSpawnSpec {
        program: paths.node.clone(),
        args: arguments,
        current_dir: paths.runtime_dir.clone(),
        environment,
        stdout: ResourceOutput::Piped,
        stderr: ResourceOutput::Piped,
    })
}

// ── React content ─────────────────────────────────────────────────────────

/// Point-in-time pre-launch walk of the React content from a no-follow handle
/// bound to the retained React identity: only directories and regular files;
/// a symlink, junction or other reparse point, FIFO, socket, device or other
/// special entry denies the launch. Bounded in entries and depth. It does not
/// prevent a hostile same-user writer changing the tree afterwards.
fn walk_react(react: &Path, workspace: &ProvisionedWorkspace) -> Result<(), &'static str> {
    native::walk_react(react, workspace, MAX_REACT_ENTRIES)
}

#[cfg(unix)]
mod native {
    //! Descriptor-relative traversal that never follows a link: each child is
    //! resolved relative to its parent's descriptor, never from a string path.
    use super::{ProvisionedWorkspace, MAX_REACT_DEPTH};
    use rustix::fs::{fstat, openat, statat, AtFlags, Dir, FileType, Mode, OFlags, CWD};
    use std::os::fd::{AsFd, BorrowedFd};
    use std::path::Path;

    fn directory_flags() -> OFlags {
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::DIRECTORY
    }

    pub(super) fn walk_react(
        react: &Path,
        workspace: &ProvisionedWorkspace,
        mut budget: usize,
    ) -> Result<(), &'static str> {
        let root = std::fs::File::from(
            openat(CWD, react, directory_flags(), Mode::empty()).map_err(|_| "react")?,
        );
        workspace
            .validate_react_handle(&root)
            .map_err(|_| "react")?;
        walk(root.as_fd(), 0, &mut budget)
    }

    fn walk(dir: BorrowedFd<'_>, depth: usize, budget: &mut usize) -> Result<(), &'static str> {
        let mut names = Vec::new();
        for entry in Dir::read_from(dir).map_err(|_| "react_unreadable")? {
            let entry = entry.map_err(|_| "react_unreadable")?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            *budget = budget.checked_sub(1).ok_or("react_limit")?;
            names.push(name.to_owned());
        }
        for name in names {
            let observed = statat(dir, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| "react_unreadable")?;
            match FileType::from_raw_mode(observed.st_mode) {
                FileType::RegularFile => {}
                FileType::Directory => {
                    if depth + 1 >= MAX_REACT_DEPTH {
                        return Err("react_limit");
                    }
                    let child = openat(dir, name.as_c_str(), directory_flags(), Mode::empty())
                        .map_err(|_| "react_changed")?;
                    let opened = fstat(&child).map_err(|_| "react_unreadable")?;
                    if (opened.st_dev, opened.st_ino) != (observed.st_dev, observed.st_ino) {
                        return Err("react_changed");
                    }
                    walk(child.as_fd(), depth + 1, budget)?;
                }
                FileType::Symlink => return Err("react_redirected"),
                _ => return Err("react_special"),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod native {
    //! Handle-retaining traversal that never follows or accepts a reparse
    //! point: each directory is held (no delete sharing, so it cannot be
    //! renamed or replaced) while its entries are checked.
    use super::{ProvisionedWorkspace, MAX_REACT_DEPTH};
    use std::fs::{File, Metadata, OpenOptions};
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    fn redirected(metadata: &Metadata) -> bool {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    fn open_dir(path: &Path) -> Result<File, &'static str> {
        let handle = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| "react_unreadable")?;
        let metadata = handle.metadata().map_err(|_| "react_unreadable")?;
        if redirected(&metadata) {
            return Err("react_redirected");
        }
        if !metadata.is_dir() {
            return Err("react_changed");
        }
        Ok(handle)
    }

    pub(super) fn walk_react(
        react: &Path,
        workspace: &ProvisionedWorkspace,
        mut budget: usize,
    ) -> Result<(), &'static str> {
        let root = open_dir(react).map_err(|_| "react")?;
        workspace
            .validate_react_handle(&root)
            .map_err(|_| "react")?;
        walk(react, &root, 0, &mut budget)
    }

    // `_held` pins the directory while its entries are processed.
    fn walk(
        dir: &Path,
        _held: &File,
        depth: usize,
        budget: &mut usize,
    ) -> Result<(), &'static str> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|_| "react_unreadable")? {
            let entry = entry.map_err(|_| "react_unreadable")?;
            *budget = budget.checked_sub(1).ok_or("react_limit")?;
            // Directory-entry metadata never follows a reparse point.
            let metadata = entry.metadata().map_err(|_| "react_unreadable")?;
            entries.push((entry.path(), metadata));
        }
        for (path, metadata) in entries {
            if redirected(&metadata) {
                return Err("react_redirected");
            }
            if metadata.is_dir() {
                if depth + 1 >= MAX_REACT_DEPTH {
                    return Err("react_limit");
                }
                let child = open_dir(&path)?;
                walk(&path, &child, depth + 1, budget)?;
            } else if !metadata.is_file() {
                return Err("react_special");
            }
        }
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
mod native {
    use super::ProvisionedWorkspace;
    use std::path::Path;

    pub(super) fn walk_react(
        _: &Path,
        _: &ProvisionedWorkspace,
        _: usize,
    ) -> Result<(), &'static str> {
        Err("react_unreadable")
    }
}

// ── Readiness and probe ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Readiness {
    Ready(u16),
    Malformed,
    Oversized,
    Ended,
}

/// Drains a pipe to completion on its own thread so the child never blocks on
/// it; bytes are handed only to `sink` (production discards them).
fn drain(
    mut pipe: ResourceReader,
    mut sink: impl FnMut(&[u8]) + Send + 'static,
) -> Result<(), ProofFailure> {
    thread::Builder::new()
        .name("nexus-builder-output".into())
        .spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => sink(&buffer[..read]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
        })
        .map(drop)
        .map_err(|_| ProofFailure::Failed("reader_unavailable"))
}

/// Reads the first stdout line (bounded) on its own thread, reports it once,
/// then keeps draining stdout and discards everything after it.
fn read_readiness(mut stdout: ResourceReader) -> Result<Receiver<Readiness>, ProofFailure> {
    let (sender, receiver) = sync_channel(1);
    thread::Builder::new()
        .name("nexus-builder-readiness".into())
        .spawn(move || {
            let _ = sender.send(first_line(&mut stdout));
            let _ = std::io::copy(&mut stdout, &mut std::io::sink());
        })
        .map_err(|_| ProofFailure::Failed("reader_unavailable"))?;
    Ok(receiver)
}

fn first_line(stdout: &mut dyn Read) -> Readiness {
    let mut buffer = [0u8; READINESS_LINE_BYTES + 1];
    let mut filled = 0;
    loop {
        if filled == buffer.len() {
            return Readiness::Oversized;
        }
        match stdout.read(&mut buffer[filled..]) {
            Ok(0) => return Readiness::Ended,
            Ok(read) => {
                let start = filled;
                filled += read;
                if let Some(end) = buffer[start..filled].iter().position(|b| *b == b'\n') {
                    return parse_readiness(&buffer[..start + end]);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Readiness::Ended,
        }
    }
}

/// Exactly the canonical JSON the Nexus entry writes: fixed field order, no
/// whitespace, host 127.0.0.1 and a decimal port 1..=65535. Nothing else.
fn parse_readiness(line: &[u8]) -> Readiness {
    const PREFIX: &str = r#"{"event":"ready","host":"127.0.0.1","port":"#;
    let digits = std::str::from_utf8(line)
        .ok()
        .and_then(|text| text.strip_prefix(PREFIX))
        .and_then(|rest| rest.strip_suffix('}'));
    match digits {
        Some(digits)
            if (1..=5).contains(&digits.len())
                && !digits.starts_with('0')
                && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            digits
                .parse::<u16>()
                .map_or(Readiness::Malformed, Readiness::Ready)
        }
        _ => Readiness::Malformed,
    }
}

fn await_readiness(
    readiness: &Receiver<Readiness>,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<u16, ProofFailure> {
    loop {
        if cancelled() {
            return Err(ProofFailure::Cancelled);
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(ProofFailure::Failed("readiness_timeout"));
        }
        match readiness.recv_timeout(POLL.min(deadline - now)) {
            Ok(Readiness::Ready(port)) => return Ok(port),
            Ok(Readiness::Malformed) => return Err(ProofFailure::Failed("readiness_malformed")),
            Ok(Readiness::Oversized) => return Err(ProofFailure::Failed("readiness_oversized")),
            Ok(Readiness::Ended) | Err(RecvTimeoutError::Disconnected) => {
                return Err(ProofFailure::Failed("exited_before_ready"))
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// One bounded `GET /` to the announced loopback port. Success is exactly an
/// HTTP/1.1 200 carrying this launch's probe token once; any other status,
/// including a redirect (never followed), fails.
fn probe(
    port: u16,
    token: &str,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), ProofFailure> {
    let failed = ProofFailure::Failed("probe_failed");
    let remaining = || deadline.saturating_duration_since(Instant::now());
    if remaining().is_zero() {
        return Err(failed);
    }
    let address = SocketAddr::from((LOOPBACK, port));
    let mut stream = TcpStream::connect_timeout(&address, remaining()).map_err(|_| failed)?;
    stream
        .set_write_timeout(Some(remaining().max(POLL)))
        .map_err(|_| failed)?;
    stream.set_read_timeout(Some(POLL)).map_err(|_| failed)?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {LOOPBACK}:{port}\r\nAccept: text/html\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).map_err(|_| failed)?;
    let mut head = Vec::new();
    let mut buffer = [0u8; 4096];
    let end = loop {
        if cancelled() {
            return Err(ProofFailure::Cancelled);
        }
        if remaining().is_zero() {
            return Err(failed);
        }
        match stream.read(&mut buffer) {
            Ok(0) => return Err(failed),
            Ok(read) => {
                head.extend_from_slice(&buffer[..read]);
                if let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                    break end;
                }
                if head.len() > PROBE_HEAD_BYTES {
                    return Err(failed);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(failed),
        }
    };
    if end <= PROBE_HEAD_BYTES && accepted(&head[..end], token) {
        Ok(())
    } else {
        Err(failed)
    }
}

fn accepted(head: &[u8], token: &str) -> bool {
    let Ok(head) = std::str::from_utf8(head) else {
        return false;
    };
    let mut lines = head.split("\r\n");
    if !lines
        .next()
        .unwrap_or_default()
        .starts_with("HTTP/1.1 200 ")
    {
        return false;
    }
    let tokens: Vec<&str> = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case(TOKEN_HEADER))
        .map(|(_, value)| value.trim())
        .collect();
    tokens == [token]
}

#[cfg(test)]
pub(super) mod tests;
