//! Checked cleanup observations for the live sandbox suite (P2-V1-R1, R2).
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
//! is bounded in time and output.
//!
//! The query's processes stay owned until they are ended. The query leads
//! its own process group, and its leader is this process's child: until the
//! leader is reaped, its pid anchors the group's id, which no other group can
//! then be given. Whatever the query did (answered, timed out, wrote too much
//! or could not be read), its group is ended with SIGKILL while the leader is
//! still unreaped and the output still open, and only then is the leader
//! reaped; the leader's exit and the end of its output are never taken as the
//! end of its group. An answer is returned only once its group is ended, and
//! a query whose group cannot be confirmed ended stays owned in its error,
//! for an explicit retry. This owns the query's processes; it confines
//! nothing: a process that leaves the group is not reached.
//!
//! A retained boundary is judged only after its explicit, bounded cleanup:
//! [`settle`] keeps the owner every failed attempt returns, [`judge_retained`]
//! keeps every failure, and an owner still unconfirmed after the attempts is
//! released explicitly ([`release`]), once reported, never dropped silently.
//!
//! Shared by the live suite (`phase2_live_sandbox.rs`) and its fixture
//! controls (`phase2_cleanup_observation.rs`, which never reach a real user
//! manager); each uses a part.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::ffi::{CString, OsString};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Bound on one query, as the sandbox bounds each call to the user manager.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on one query's output, both streams together.
pub const QUERY_OUTPUT_LIMIT: usize = 64 * 1024;
/// Bound on reaping a query's leader once its group is ended.
const REAP_TIMEOUT: Duration = Duration::from_secs(5);
/// Bound on the explicit attempts made for an owner whose cleanup is not
/// confirmed (each attempt is bounded itself).
pub const EXPLICIT_ATTEMPTS: usize = 3;
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

/// Why a query was stopped before it answered.
#[derive(Debug)]
pub enum Stop {
    Timeout,
    OutputLimit,
    Io(io::Error),
}

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "the query did not finish in time"),
            Self::OutputLimit => write!(f, "the query wrote more than its output bound"),
            Self::Io(error) => write!(f, "the query could not be read or awaited ({error})"),
        }
    }
}

/// Why an observation has no answer.
#[derive(Debug)]
pub enum ObservationError {
    /// The user runtime directory or its bus is not what it must be.
    Runtime(String),
    /// The query could not be started.
    Spawn(io::Error),
    /// The query was stopped before it answered; its process group was
    /// ended, and its leader reaped with this status.
    Stopped { stop: Stop, status: ExitStatus },
    /// The query's process group could not be confirmed ended: the query is
    /// still owned here.
    Unfinalized(Box<Unfinalized>),
    /// The query failed.
    Failed { status: ExitStatus, stderr: String },
    /// The query succeeded but wrote to its error output.
    Diagnostics(String),
    /// The answer is not a list of verifier scopes.
    Malformed(String),
}

/// A query whose process group could not be confirmed ended, still owned.
#[derive(Debug)]
pub struct Unfinalized {
    /// The query, its leader unreaped: for an explicit retry of
    /// [`OwnedQuery::finalize`].
    pub query: OwnedQuery,
    /// What came first: the query's answer, or why it was stopped.
    pub first: Result<Answer, Stop>,
    /// Why its group is not confirmed ended.
    pub failure: io::Error,
}

fn came_first(first: &Result<Answer, Stop>) -> String {
    match first {
        Ok(answer) => format!("the query answered (leader {})", answer.status),
        Err(stop) => stop.to_string(),
    }
}

impl fmt::Display for ObservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(why) => write!(f, "no usable user manager bus: {why}"),
            Self::Spawn(error) => write!(f, "the query could not be started: {error}"),
            Self::Stopped { stop, status } => {
                write!(f, "{stop}; its process group was ended (leader {status})")
            }
            Self::Unfinalized(unfinalized) => write!(
                f,
                "{}; its process group is not confirmed ended ({})",
                came_first(&unfinalized.first),
                unfinalized.failure
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

/// The process operations a query's finalization uses: [`PROCESS`], or a
/// fixture's injected failures.
#[derive(Debug, Clone, Copy)]
pub struct Ops {
    /// The leader's status once it has exited, read without reaping it.
    pub exited: fn(&Child) -> io::Result<Option<ExitStatus>>,
    /// SIGKILL to every member of a process group.
    pub signal_group: fn(libc::pid_t) -> io::Result<()>,
    /// Reap the leader, waiting until the deadline at most.
    pub reap: fn(&mut Child, Instant) -> io::Result<Option<ExitStatus>>,
}

/// The real process operations.
pub const PROCESS: Ops = Ops {
    exited: leader_exited,
    signal_group: kill_group,
    reap: wait_until,
};

/// One query: its program, its arguments and its whole environment.
pub struct Query {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub timeout: Duration,
    pub output_limit: usize,
    pub ops: Ops,
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
        ops: PROCESS,
    })
}

/// A finished query: its exit status and its output.
#[derive(Debug)]
pub struct Answer {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// A started query's processes, owned through its leader: the child this
/// observer spawned, which leads the query's process group. Until the leader
/// is reaped, its pid anchors the group's id, so signalling the group reaches
/// only the query's own members. Finalizing ends the group, then reaps the
/// leader.
#[derive(Debug)]
pub struct OwnedQuery {
    leader: Child,
    group: libc::pid_t,
    /// The unreaped leader still anchors the group id. Cleared once the
    /// leader is reaped, or when reaping it failed and its state is unknown:
    /// the group is never signalled again.
    anchored: bool,
    ops: Ops,
}

impl OwnedQuery {
    /// The leader's pid (fixture controls).
    pub fn leader(&self) -> u32 {
        self.leader.id()
    }

    /// The process operations to use from now on (fixture controls: a later
    /// explicit retry without the injected failure).
    pub fn with_ops(mut self, ops: Ops) -> Self {
        self.ops = ops;
        self
    }

    /// End the query: SIGKILL to every member of its group while the
    /// unreaped leader anchors the group id, then reap the leader within the
    /// bound. The leader's status only once both succeeded; otherwise the
    /// query stays owned, with why.
    pub fn finalize(mut self) -> Result<ExitStatus, (Self, io::Error)> {
        if !self.anchored {
            return Err((
                self,
                io::Error::other(
                    "the leader's state is unknown: its group is never signalled again",
                ),
            ));
        }
        if let Err(error) = (self.ops.signal_group)(self.group) {
            return Err((self, error));
        }
        match (self.ops.reap)(&mut self.leader, Instant::now() + REAP_TIMEOUT) {
            Ok(Some(status)) => {
                self.anchored = false;
                Ok(status)
            }
            Ok(None) => Err((
                self,
                io::Error::new(io::ErrorKind::TimedOut, "the leader was not reaped in time"),
            )),
            Err(error) => {
                self.anchored = false;
                Err((self, error))
            }
        }
    }
}

impl Drop for OwnedQuery {
    /// Defense in depth for a query released without a confirmed
    /// finalization: while the unreaped leader still anchors the group id,
    /// SIGKILL the group and try once to reap the leader. Never a
    /// confirmation.
    fn drop(&mut self) {
        if self.anchored {
            let _ = (self.ops.signal_group)(self.group);
            let _ = self.leader.try_wait();
        }
    }
}

/// The leader's status once it has exited, read without reaping it: the
/// leader stays this process's unreaped child, still anchoring its group.
fn leader_exited(leader: &Child) -> io::Result<Option<ExitStatus>> {
    // SAFETY: siginfo_t is plain data; waitid fills it.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: waits for this process's own child only, without reaping it
    // (WNOWAIT) and without blocking (WNOHANG).
    check(unsafe {
        libc::waitid(
            libc::P_PID,
            leader.id(),
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    })?;
    // SAFETY: waitid filled `info`; its pid is 0 while the child runs.
    if unsafe { info.si_pid() } == 0 {
        return Ok(None);
    }
    // SAFETY: as above, for an exited child.
    let value = unsafe { info.si_status() };
    let raw = match info.si_code {
        libc::CLD_EXITED => (value & 0xff) << 8,
        libc::CLD_KILLED => value & 0x7f,
        libc::CLD_DUMPED => (value & 0x7f) | 0x80,
        code => return Err(io::Error::other(format!("unexpected child state {code}"))),
    };
    Ok(Some(ExitStatus::from_raw(raw)))
}

/// SIGKILL to every member of process group `group`.
fn kill_group(group: libc::pid_t) -> io::Result<()> {
    // SAFETY: signals one process group, whose id the caller's unreaped
    // leader anchors.
    check(unsafe { libc::kill(-group, libc::SIGKILL) }).map(drop)
}

/// Run `query` to completion within its bounds. Whatever happens, the
/// query's process group is ended while its leader is still unreaped and its
/// output still open, and only then is the leader reaped: an answer comes
/// only with its group ended, and a query whose group cannot be confirmed
/// ended stays owned in the error.
pub fn run(query: &Query) -> Result<Answer, ObservationError> {
    let mut leader = Command::new(&query.program)
        .args(&query.args)
        .env_clear()
        .envs(query.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(ObservationError::Spawn)?;
    let mut pipes = [
        leader
            .stdout
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
        leader
            .stderr
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
    ];
    let Ok(group) = libc::pid_t::try_from(leader.id()) else {
        // A pid always fits; without a group id, end the leader alone.
        let _ = leader.kill();
        let _ = leader.wait();
        return Err(ObservationError::Spawn(io::Error::other(
            "the leader's pid is not a process group id",
        )));
    };
    let owned = OwnedQuery {
        leader,
        group,
        anchored: true,
        ops: query.ops,
    };
    let deadline = Instant::now() + query.timeout;
    let first = match collect(&mut pipes, deadline, query.output_limit) {
        Ok([stdout, stderr]) => match exit_by(&owned, deadline) {
            Ok(Some(status)) => Ok(Answer {
                status,
                stdout,
                stderr,
            }),
            Ok(None) => Err(Stop::Timeout),
            Err(error) => Err(Stop::Io(error)),
        },
        Err(stop) => Err(stop),
    };
    let finalized = owned.finalize();
    drop(pipes);
    match (first, finalized) {
        (Ok(answer), Ok(status)) => Ok(Answer { status, ..answer }),
        (Err(stop), Ok(status)) => Err(ObservationError::Stopped { stop, status }),
        (first, Err((query, failure))) => {
            Err(ObservationError::Unfinalized(Box::new(Unfinalized {
                query,
                first,
                failure,
            })))
        }
    }
}

/// Wait, without reaping, until the leader exits or `deadline` passes.
fn exit_by(query: &OwnedQuery, deadline: Instant) -> io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = (query.ops.exited)(&query.leader)? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
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

/// `describe()`, or `otherwise` should it panic: describing never unwinds
/// through an owner the caller still holds.
fn contained<T>(describe: impl FnOnce() -> T, otherwise: T) -> T {
    catch_unwind(AssertUnwindSafe(describe)).unwrap_or(otherwise)
}

fn joined(errors: &[io::Error]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// An observation failure, made reportable without losing what it owns: a
/// query whose process group is not confirmed ended is finalized again
/// explicitly (at most [`EXPLICIT_ATTEMPTS`] times) and, if still
/// unconfirmed, released to its drop backstop only once that is described.
/// The failure itself always remains one.
pub fn reported(error: ObservationError) -> String {
    let unfinalized = match error {
        ObservationError::Unfinalized(unfinalized) => *unfinalized,
        other => {
            return contained(
                || other.to_string(),
                "(the failure could not be described)".to_string(),
            )
        }
    };
    let Unfinalized {
        query,
        first,
        failure,
    } = unfinalized;
    let mut failures = vec![failure];
    let settled = settle(query, EXPLICIT_ATTEMPTS, |query| {
        query.finalize().map(drop).map_err(|(query, error)| {
            failures.push(error);
            query
        })
    });
    let what = contained(
        || came_first(&first),
        "(the query could not be described)".to_string(),
    );
    match settled {
        Settled::Confirmed(attempt) => contained(
            || {
                format!(
                    "{what}; its process group was confirmed ended only on explicit attempt \
                     {attempt} ({})",
                    joined(&failures)
                )
            },
            "(the failure could not be described)".to_string(),
        ),
        Settled::Unconfirmed(query, attempts) => {
            let why = contained(
                || {
                    format!(
                        "{what}; its process group is still not confirmed ended after \
                         {attempts} explicit attempts ({})",
                        joined(&failures)
                    )
                },
                "(the failure could not be described)".to_string(),
            );
            release(query, "an unconfirmed query", &[why])
        }
    }
}

/// What an owner's explicit cleanup came to.
#[derive(Debug)]
pub enum Settled<B> {
    /// An attempt confirmed the cleanup: this many were made.
    Confirmed(usize),
    /// Every attempt failed: the owner, still held, and how many were made.
    Unconfirmed(B, usize),
}

/// Retry `owner`'s cleanup explicitly, at most `attempts` times, keeping the
/// owner each failed attempt returns. `retry` must not unwind (a retained
/// boundary's never does).
pub fn settle<B>(
    mut owner: B,
    attempts: usize,
    mut retry: impl FnMut(B) -> Result<(), B>,
) -> Settled<B> {
    for attempt in 1..=attempts {
        owner = match retry(owner) {
            Ok(()) => return Settled::Confirmed(attempt),
            Err(owner) => owner,
        };
    }
    Settled::Unconfirmed(owner, attempts)
}

/// A retained boundary, judged after its explicit cleanup.
#[derive(Debug)]
pub enum Verdict<B> {
    /// The cleanup is confirmed and every check holds.
    Passed,
    /// The cleanup is confirmed, but these checks failed.
    Failed(Vec<String>),
    /// The cleanup is still unconfirmed after the bounded attempts: the
    /// owner, still held for explicit recovery or release, and every failure.
    Unconfirmed { owner: B, failures: Vec<String> },
}

/// Judge a retained boundary only after its explicit cleanup. `failures`
/// come from the evidence taken while it was held, and `observed` is the
/// scope observation taken then (already reported, its own query ended). The
/// cleanup is settled first, whatever they were. Every failure is kept: an
/// observation failure even when the cleanup is then confirmed, a cleanup
/// confirmed only after the first attempt, and an unconfirmed cleanup, which
/// keeps its owner.
pub fn judge_retained<B>(
    mut failures: Vec<String>,
    observed: Result<Scopes, String>,
    before: &Scopes,
    owner: B,
    attempts: usize,
    retry: impl FnMut(B) -> Result<(), B>,
) -> Verdict<B> {
    let settled = settle(owner, attempts, retry);
    let judged = contained(
        || {
            let mut found = Vec::new();
            match &observed {
                Err(error) => found.push(format!(
                    "the scope could not be observed while the boundary was retained: {error}"
                )),
                Ok(kept) if kept.len() <= before.len() => found.push(format!(
                    "the scope is not kept: {} loaded verifier scopes {kept:?}, not more than \
                     the {} before",
                    kept.len(),
                    before.len()
                )),
                Ok(_) => {}
            }
            match &settled {
                Settled::Confirmed(1) => {}
                Settled::Confirmed(attempt) => found.push(format!(
                    "the cleanup was confirmed only on explicit attempt {attempt}"
                )),
                Settled::Unconfirmed(_, attempts) => found.push(format!(
                    "the cleanup is still unconfirmed after {attempts} explicit attempts"
                )),
            }
            found
        },
        vec!["(the failures could not be described)".to_string()],
    );
    failures.extend(judged);
    match settled {
        Settled::Confirmed(_) if failures.is_empty() => Verdict::Passed,
        Settled::Confirmed(_) => Verdict::Failed(failures),
        Settled::Unconfirmed(owner, _) => Verdict::Unconfirmed { owner, failures },
    }
}

/// Release an owner whose cleanup stays unconfirmed after the bounded
/// attempts, explicitly and only once reported: dropping it runs its
/// defense-in-depth backstop (a retained boundary's or a query's), which is
/// never a confirmation. Returns the report.
pub fn release<B>(owner: B, what: &str, failures: &[String]) -> String {
    let report = contained(
        || format!("{what}: {}", failures.join("; ")),
        "(the failures could not be described)".to_string(),
    );
    let _ = writeln!(
        io::stderr(),
        "{report}: releasing the unconfirmed owner to its drop backstop, never a confirmation"
    );
    drop(owner);
    report
}
