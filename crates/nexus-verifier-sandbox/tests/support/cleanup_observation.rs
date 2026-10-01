//! Checked cleanup observations for the live sandbox suite (P2-V1-R1).
//!
//! The live suite counts the verifier scopes the user manager has loaded
//! before and after an execution. An observation is the exact set of loaded
//! `nexus-verifier-*.scope` units, or an error saying why there is no answer:
//! a query that could not start, failed, timed out, wrote too much or
//! answered anything else is never "no scopes". Unit names are identifiers
//! for the record only, never authority to stop or kill anything.
//!
//! The query is `systemctl --user` on the user bus of this process's real
//! uid, `/run/user/<uid>/bus`, checked first as the sandbox checks it:
//! reached from `/` without following a symlink, `/run` and `/run/user`
//! root-owned and writable by no one else, `/run/user/<uid>` a private tmpfs
//! directory of the uid and the bus a socket of the uid. Only `systemctl`
//! itself is found through `PATH`; the query's whole environment is that bus
//! address and fixed output settings, so an inherited
//! `DBUS_SESSION_BUS_ADDRESS` or `XDG_RUNTIME_DIR` can never redirect it. It
//! is bounded in time and output and runs in its own process group, which is
//! killed, and the query reaped, when a bound is reached.
//!
//! Shared by the live suite (`phase2_live_sandbox.rs`) and its fixture
//! controls (`phase2_cleanup_observation.rs`, which never reach a real user
//! manager); each uses a part.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::ffi::{CString, OsString};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Bound on one query, as the sandbox bounds each call to the user manager.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on one query's output, both streams together.
pub const QUERY_OUTPUT_LIMIT: usize = 64 * 1024;
/// Bound on reaping a query that was killed.
const REAP_TIMEOUT: Duration = Duration::from_secs(5);
/// The units observed.
pub const SCOPE_PATTERN: &str = "nexus-verifier-*.scope";
/// The query's arguments: no pager, no legend, no glyphs, nothing
/// ellipsized, loaded units in every state.
pub const SCOPE_QUERY_ARGS: [&str; 8] = [
    "--user",
    "--no-pager",
    "--legend=no",
    "--plain",
    "--full",
    "--all",
    "list-units",
    SCOPE_PATTERN,
];
const TMPFS_MAGIC: i64 = 0x0102_1994;

/// Loaded verifier scopes, by unit name.
pub type Scopes = BTreeSet<String>;

/// Why an observation has no answer.
#[derive(Debug)]
pub enum ObservationError {
    /// The user runtime directory or its bus is not what it must be.
    Runtime(String),
    /// The query could not be started.
    Spawn(io::Error),
    /// The query did not finish in time. It was killed; this is its reaped
    /// status (`None`: it could not be reaped in time either).
    Timeout(Option<ExitStatus>),
    /// The query wrote more than the bound. It was killed (as on a timeout).
    OutputLimit(Option<ExitStatus>),
    /// Reading the answer or waiting for the query failed. It was killed (as
    /// on a timeout).
    Io {
        error: io::Error,
        reaped: Option<ExitStatus>,
    },
    /// The query failed.
    Failed { status: ExitStatus, stderr: String },
    /// The query succeeded but wrote to its error output.
    Diagnostics(String),
    /// The answer is not a list of verifier scopes.
    Malformed(String),
}

fn reaped(status: &Option<ExitStatus>) -> String {
    status.map_or("not reaped".to_string(), |status| {
        format!("reaped: {status}")
    })
}

impl fmt::Display for ObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(why) => write!(f, "no usable user manager bus: {why}"),
            Self::Spawn(error) => write!(f, "the query could not be started: {error}"),
            Self::Timeout(status) => write!(
                f,
                "the query did not finish in time and was killed ({})",
                reaped(status)
            ),
            Self::OutputLimit(status) => write!(
                f,
                "the query wrote more than {QUERY_OUTPUT_LIMIT} bytes and was killed ({})",
                reaped(status)
            ),
            Self::Io {
                error,
                reaped: status,
            } => write!(
                f,
                "the query could not be read or awaited ({error}) and was killed ({})",
                reaped(status)
            ),
            Self::Failed { status, stderr } => write!(f, "the query failed ({status}): {stderr}"),
            Self::Diagnostics(stderr) => write!(f, "the query reported: {stderr}"),
            Self::Malformed(why) => write!(f, "the answer is not a list of verifier scopes: {why}"),
        }
    }
}

/// Where the user runtime directory is checked, and against what. The real
/// host is `/`, root and a tmpfs; the fixture controls substitute their own.
pub struct Host<'a> {
    pub root: &'a Path,
    pub uid: u32,
    pub root_owner: u32,
    pub fs_magic: Option<i64>,
}

impl Host<'static> {
    /// This process's host, for its real uid.
    pub fn real() -> Self {
        Host {
            root: Path::new("/"),
            // SAFETY: getuid has no preconditions.
            uid: unsafe { libc::getuid() },
            root_owner: 0,
            fs_magic: Some(TMPFS_MAGIC),
        }
    }
}

fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

fn open_dir(path: &Path) -> io::Result<OwnedFd> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: path is NUL-terminated; open returns a new descriptor owned
    // here on success.
    let fd = check(unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    })?;
    // SAFETY: open succeeded, so fd is new and owned here.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The directory `name` beneath `dir`, never through a symlink at `name`.
fn open_dir_at(dir: &OwnedFd, name: &str) -> io::Result<OwnedFd> {
    let name = CString::new(name)?;
    // SAFETY: name is NUL-terminated; openat returns a new descriptor owned
    // here on success.
    let fd = check(unsafe {
        libc::openat(
            dir.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    })?;
    // SAFETY: openat succeeded, so fd is new and owned here.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn fstat(fd: &OwnedFd) -> io::Result<libc::stat> {
    // SAFETY: stat is plain data; fstat fills it.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat writes one stat structure.
    check(unsafe { libc::fstat(fd.as_raw_fd(), &mut st) })?;
    Ok(st)
}

/// `lstat` of `name` beneath `dir`.
fn stat_at(dir: &OwnedFd, name: &str) -> io::Result<libc::stat> {
    let name = CString::new(name)?;
    // SAFETY: stat is plain data; fstatat fills it.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: name is NUL-terminated; fstatat writes one stat structure.
    check(unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            &mut st,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    })?;
    Ok(st)
}

/// The filesystem type (`statfs.f_type`) of the object `path` names.
pub fn filesystem_type(path: &Path) -> io::Result<i64> {
    filesystem_type_of(&open_dir(path)?)
}

fn filesystem_type_of(fd: &OwnedFd) -> io::Result<i64> {
    // SAFETY: statfs is plain data; fstatfs fills it.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: fstatfs writes one statfs structure.
    check(unsafe { libc::fstatfs(fd.as_raw_fd(), &mut fs) })?;
    Ok(fs.f_type)
}

/// The checked user bus of `host.uid`: `/run/user/<uid>/bus` beneath
/// `host.root`.
pub fn user_bus(host: &Host<'_>) -> Result<PathBuf, ObservationError> {
    let unusable = |what: &str, error: io::Error| {
        ObservationError::Runtime(format!("{what} cannot be inspected: {error}"))
    };
    let mut dir = open_dir(host.root).map_err(|error| unusable("/", error))?;
    for (name, path) in [("run", "/run"), ("user", "/run/user")] {
        dir = open_dir_at(&dir, name).map_err(|error| unusable(path, error))?;
        let st = fstat(&dir).map_err(|error| unusable(path, error))?;
        if st.st_uid != host.root_owner || st.st_mode & 0o022 != 0 {
            return Err(ObservationError::Runtime(format!(
                "{path} is not root-owned or is writable by others"
            )));
        }
    }
    let uid = host.uid.to_string();
    let runtime_path = format!("/run/user/{uid}");
    let runtime = open_dir_at(&dir, &uid).map_err(|error| unusable(&runtime_path, error))?;
    let st = fstat(&runtime).map_err(|error| unusable(&runtime_path, error))?;
    if st.st_uid != host.uid || st.st_mode & 0o7777 != 0o700 {
        return Err(ObservationError::Runtime(format!(
            "{runtime_path} is not a private directory of uid {uid}"
        )));
    }
    if let Some(magic) = host.fs_magic {
        let found = filesystem_type_of(&runtime).map_err(|error| unusable(&runtime_path, error))?;
        if found != magic {
            return Err(ObservationError::Runtime(format!(
                "{runtime_path} is not a tmpfs"
            )));
        }
    }
    let bus = stat_at(&runtime, "bus").map_err(|error| unusable("the user bus", error))?;
    if bus.st_mode & libc::S_IFMT != libc::S_IFSOCK || bus.st_uid != host.uid {
        return Err(ObservationError::Runtime(format!(
            "{runtime_path}/bus is not a socket of uid {uid}"
        )));
    }
    Ok(host.root.join("run/user").join(uid).join("bus"))
}

/// `systemctl` from this process's `PATH` (absolute entries only).
fn systemctl() -> Result<PathBuf, ObservationError> {
    let missing = || {
        ObservationError::Spawn(io::Error::new(
            io::ErrorKind::NotFound,
            "no systemctl on PATH",
        ))
    };
    let path = std::env::var_os("PATH").ok_or_else(missing)?;
    std::env::split_paths(&path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("systemctl"))
        .find(|program| {
            program
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .ok_or_else(missing)
}

/// One query: its program, its arguments and its whole environment.
pub struct Query {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
    pub output_limit: usize,
}

/// The query of the loaded verifier scopes on the user bus at `bus`, run by
/// `program` after `prefix` (only the fixture controls give one).
pub fn scope_query(
    program: PathBuf,
    prefix: &[&str],
    bus: &Path,
) -> Result<Query, ObservationError> {
    let plain = bus.to_str().filter(|bus| {
        bus.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_/.".contains(&byte))
    });
    let Some(bus) = plain else {
        return Err(ObservationError::Runtime(format!(
            "{} is not a plain bus path",
            bus.display()
        )));
    };
    let args = prefix.iter().chain(SCOPE_QUERY_ARGS.iter());
    let env = [
        ("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={bus}")),
        ("LC_ALL", "C".to_string()),
        ("SYSTEMD_COLORS", "0".to_string()),
        ("SYSTEMD_URLIFY", "0".to_string()),
    ];
    Ok(Query {
        program,
        args: args.map(OsString::from).collect(),
        env: env
            .into_iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect(),
        timeout: QUERY_TIMEOUT,
        output_limit: QUERY_OUTPUT_LIMIT,
    })
}

/// A finished query: its exit status and its output.
#[derive(Debug)]
pub struct Answer {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

enum Stop {
    Timeout,
    OutputLimit,
    Io(io::Error),
}

/// Run `query` to completion within its bounds.
pub fn run(query: &Query) -> Result<Answer, ObservationError> {
    let mut child = Command::new(&query.program)
        .args(&query.args)
        .env_clear()
        .envs(query.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(ObservationError::Spawn)?;
    let deadline = Instant::now() + query.timeout;
    let mut pipes = [
        child
            .stdout
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
        child
            .stderr
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
    ];
    let stop = match collect(&mut pipes, deadline, query.output_limit) {
        Ok([stdout, stderr]) => match wait_until(&mut child, deadline) {
            Ok(Some(status)) => {
                return Ok(Answer {
                    status,
                    stdout,
                    stderr,
                })
            }
            Ok(None) => Stop::Timeout,
            Err(error) => Stop::Io(error),
        },
        Err(stop) => stop,
    };
    // A stopped query is ended by this kill while its output is still open,
    // never by a pipe closed under it.
    let reaped = end(&mut child);
    drop(pipes);
    Err(match stop {
        Stop::Timeout => ObservationError::Timeout(reaped),
        Stop::OutputLimit => ObservationError::OutputLimit(reaped),
        Stop::Io(error) => ObservationError::Io { error, reaped },
    })
}

fn nonblocking(file: &File) -> io::Result<()> {
    // SAFETY: F_GETFL on a descriptor owned by `file`.
    let flags = check(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) })?;
    // SAFETY: F_SETFL on the same descriptor.
    check(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) })?;
    Ok(())
}

/// Read both output streams (`pipes`: stdout, stderr) until both close,
/// within `deadline` and `limit`. A stream is released only at its end.
fn collect(
    pipes: &mut [Option<File>; 2],
    deadline: Instant,
    limit: usize,
) -> Result<[Vec<u8>; 2], Stop> {
    if pipes.iter().any(Option::is_none) {
        return Err(Stop::Io(io::Error::other(
            "the query's output is not piped",
        )));
    }
    for pipe in pipes.iter().flatten() {
        nonblocking(pipe).map_err(Stop::Io)?;
    }
    let mut output = [Vec::new(), Vec::new()];
    let mut written = 0;
    let mut chunk = [0u8; 8192];
    while pipes.iter().any(Option::is_some) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Stop::Timeout);
        }
        let mut polled = pipes.each_ref().map(|pipe| libc::pollfd {
            fd: pipe.as_ref().map_or(-1, |pipe| pipe.as_raw_fd()),
            events: libc::POLLIN,
            revents: 0,
        });
        // Rounded up, so the last wait does not spin.
        let millis = i32::try_from(left.as_micros().div_ceil(1000)).unwrap_or(i32::MAX);
        // SAFETY: two initialized pollfd structures; poll ignores a negative
        // descriptor.
        if let Err(error) = check(unsafe { libc::poll(polled.as_mut_ptr(), 2, millis) }) {
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(Stop::Io(error));
        }
        for (index, ready) in polled.iter().enumerate() {
            let Some(pipe) = pipes[index].as_mut().filter(|_| ready.revents != 0) else {
                continue;
            };
            let closed = loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => break true,
                    Ok(read) => {
                        written += read;
                        if written > limit {
                            return Err(Stop::OutputLimit);
                        }
                        output[index].extend_from_slice(&chunk[..read]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break false,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(Stop::Io(error)),
                }
            };
            if closed {
                pipes[index] = None;
            }
        }
    }
    Ok(output)
}

/// Reap `child` if it exits by `deadline`.
fn wait_until(child: &mut Child, deadline: Instant) -> io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Kill the query's process group, then reap the query within a bound. The
/// query leads that group and is not yet reaped, so the group is still its
/// own.
fn end(child: &mut Child) -> Option<ExitStatus> {
    if let Ok(group) = libc::pid_t::try_from(child.id()) {
        // SAFETY: signals only the process group this query leads.
        unsafe { libc::kill(-group, libc::SIGKILL) };
    }
    wait_until(child, Instant::now() + REAP_TIMEOUT)
        .ok()
        .flatten()
}

fn excerpt(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim()
        .chars()
        .take(512)
        .collect()
}

fn is_verifier_scope(unit: &str) -> bool {
    unit.strip_prefix("nexus-verifier-")
        .and_then(|rest| rest.strip_suffix(".scope"))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b":-_.\\@".contains(&byte))
        })
}

/// A unit state as systemd names it: `active`, `not-found`, ...
fn is_state(state: &str) -> bool {
    state
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && state
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
}

/// The scopes a successful query listed: one line per loaded verifier scope
/// (`UNIT LOAD ACTIVE SUB DESCRIPTION`; blank lines aside), each once.
pub fn parse_scopes(stdout: &[u8]) -> Result<Scopes, ObservationError> {
    let text = std::str::from_utf8(stdout)
        .map_err(|_| ObservationError::Malformed("the answer is not UTF-8".to_string()))?;
    let mut scopes = Scopes::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split_whitespace();
        let unit = fields.next().unwrap_or_default();
        let states: Vec<&str> = fields.take(3).collect();
        if !is_verifier_scope(unit) || states.len() != 3 || !states.iter().all(|s| is_state(s)) {
            return Err(ObservationError::Malformed(format!(
                "unexpected line {line:?}"
            )));
        }
        if !scopes.insert(unit.to_string()) {
            return Err(ObservationError::Malformed(format!(
                "{unit} is listed twice"
            )));
        }
    }
    Ok(scopes)
}

/// Run `query` and read its answer: the loaded verifier scopes. Only a
/// successful query that wrote nothing to its error output answers.
pub fn observe(query: &Query) -> Result<Scopes, ObservationError> {
    let answer = run(query)?;
    if !answer.status.success() {
        return Err(ObservationError::Failed {
            status: answer.status,
            stderr: excerpt(&answer.stderr),
        });
    }
    if !answer.stderr.is_empty() {
        return Err(ObservationError::Diagnostics(excerpt(&answer.stderr)));
    }
    parse_scopes(&answer.stdout)
}

/// The verifier scopes the user manager of this process's real uid has
/// loaded.
pub fn observe_scopes() -> Result<Scopes, ObservationError> {
    let bus = user_bus(&Host::real())?;
    observe(&scope_query(systemctl()?, &[], &bus)?)
}

/// Observe with `observe` until `done` holds of the loaded scopes or
/// `deadline` passes. A failed observation ends the wait with that failure:
/// it never counts as done.
pub fn wait_for(
    deadline: Duration,
    mut observe: impl FnMut() -> Result<Scopes, ObservationError>,
    mut done: impl FnMut(&Scopes) -> bool,
) -> Result<bool, ObservationError> {
    let start = Instant::now();
    loop {
        if done(&observe()?) {
            return Ok(true);
        }
        if start.elapsed() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// How a retained boundary's check failed.
#[derive(Debug)]
pub enum Retained {
    /// The scopes could not be observed while the boundary held its scope;
    /// whether the explicit retry then confirmed the cleanup is kept with it.
    Unobserved {
        error: ObservationError,
        retried: bool,
    },
    /// The explicit retry did not confirm the cleanup; whether the scope was
    /// observed as kept is kept with it.
    NotConfirmed { kept: bool },
    /// No more verifier scopes were loaded than before the execution.
    NotKept { before: usize, observed: Scopes },
}

impl fmt::Display for Retained {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let confirmed = |retried: bool| {
            if retried {
                "confirmed"
            } else {
                "did not confirm"
            }
        };
        match self {
            Self::Unobserved { error, retried } => write!(
                f,
                "the scope could not be observed while the boundary was retained ({error}); \
                 the explicit retry {} the cleanup",
                confirmed(*retried)
            ),
            Self::NotConfirmed { kept } => write!(
                f,
                "the retry did not confirm the cleanup (the scope was {}observed as kept)",
                if *kept { "" } else { "not " }
            ),
            Self::NotKept { before, observed } => write!(
                f,
                "the scope is not kept: {} loaded verifier scopes {observed:?}, not more than \
                 the {before} before the execution",
                observed.len()
            ),
        }
    }
}

/// Judge a retained boundary only after its explicit retry. `observed` was
/// taken while the boundary still held its scope; `retry` always runs, first,
/// whatever that observation was. A failed observation stays a failure even
/// when the retry then confirms the cleanup.
pub fn after_retry(
    observed: Result<Scopes, ObservationError>,
    before: &Scopes,
    retry: impl FnOnce() -> bool,
) -> Result<(), Retained> {
    let retried = retry();
    let observed = observed.map_err(|error| Retained::Unobserved { error, retried })?;
    let kept = observed.len() > before.len();
    if !retried {
        return Err(Retained::NotConfirmed { kept });
    }
    if !kept {
        return Err(Retained::NotKept {
            before: before.len(),
            observed,
        });
    }
    Ok(())
}
