//! The live harness's entry and its checked cleanup observations (P2-V1-R1,
//! R2, R3A).
//!
//! Entry: [`dispatch`] chooses what the live harness executable is from its
//! arguments alone, before anything else happens: the live suite only with no
//! argument at all, the observation-only mode only with exactly
//! `--cleanup-observation`, a probe or driver mode the suite itself starts only
//! by its own first argument, and a refusal (a failure) for anything else,
//! never the suite. Where the sandbox does not exist, only the suite's own
//! "unavailable" note remains; the observation and every other mode fail.
//!
//! Scopes: one bounded call, made in this process, on the user bus of this
//! process's real uid: `org.freedesktop.systemd1.Manager.ListUnitsByPatterns`
//! with no state filter and the one pattern `nexus-verifier-*.scope`, which the
//! user manager answers with the matching units it has loaded. No process is
//! started. The bus is `/run/user/<uid>/bus`, checked first as the sandbox
//! checks it (reached from `/` without following a symlink, `/run` and
//! `/run/user` root-owned and writable by no one else, `/run/user/<uid>` a
//! private tmpfs directory of the uid, the bus a socket of the uid), held by
//! its own descriptor and connected through that descriptor: never resolved by
//! name again and never taken from the environment. The connection
//! authenticates with EXTERNAL only (this process's own uid). One absolute
//! budget covers reaching the bus, authenticating and the call; a reply is
//! accepted only as exactly the documented `a(ssssssouso)` list within its
//! bounds (its body checked before it is decoded), each unit a distinct
//! verifier scope. An error, a timeout or anything else is never "no scopes".
//! The connection and its tasks belong to the observation's own runtime, which
//! ends before the observation returns. Unit names are observation data, never
//! authority to stop or kill anything.
//!
//! Workspaces: the entries of `/run/user/<uid>/nexus-verifier`, opened beneath
//! the checked runtime directory's descriptor without following a symlink, and
//! checked to be an owner-only directory of the uid on the runtime directory's
//! filesystem before it is listed. It may be absent only as a genuinely missing
//! final component of the checked runtime directory: absent at observation
//! time, which says nothing about what happened before.
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
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Loaded verifier scopes, by unit name.
pub type Scopes = BTreeSet<String>;

/// Bound on the explicit attempts made for an owner whose cleanup is not
/// confirmed (each attempt is bounded itself).
pub const EXPLICIT_ATTEMPTS: usize = 3;

/// The observation-only mode's one argument.
pub const CLEANUP_OBSERVATION: &str = "--cleanup-observation";

/// Bound on a diagnostic shown in a report line, in characters.
pub const MAX_DIAGNOSTIC: usize = 512;

/// The modes the live suite itself starts (the probe, inside the sandbox or
/// outside it, and its drivers), each by its own first argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Probe,
    ProbeDaemon,
    Ablation,
    DenyUnshareDriver,
    DenyLayerDriver,
    ParentDeathDriver,
}

impl Mode {
    pub const ALL: [(&'static str, Mode); 6] = [
        ("--probe", Mode::Probe),
        ("--probe-daemon", Mode::ProbeDaemon),
        ("--ablation", Mode::Ablation),
        ("--deny-unshare-driver", Mode::DenyUnshareDriver),
        ("--deny-layer-driver", Mode::DenyLayerDriver),
        ("--parent-death-driver", Mode::ParentDeathDriver),
    ];
}

/// Why the arguments select nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Not even a program name.
    NoProgram,
    /// An argument that is not UTF-8.
    NotUtf8,
    /// A first argument that names no mode.
    Unknown(String),
    /// The observation-only mode takes no argument (no path, uid, bus,
    /// command or target): this many were given.
    ObservationArguments(usize),
    /// Neither the sandbox nor its observation exists on this platform.
    Unsupported(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgram => write!(f, "no program name: nothing runs"),
            Self::NotUtf8 => write!(f, "an argument is not UTF-8: nothing runs"),
            Self::Unknown(argument) => {
                write!(f, "unknown mode {}: nothing runs", diagnostic(argument))
            }
            Self::ObservationArguments(count) => write!(
                f,
                "{CLEANUP_OBSERVATION} takes no argument ({count} given): nothing runs"
            ),
            Self::Unsupported(mode) => write!(
                f,
                "{} is unavailable here: the verifier sandbox and its cleanup observation exist \
                 only on x86_64 Linux",
                diagnostic(mode)
            ),
        }
    }
}

/// What the live harness executable can be. The live harness gives the real
/// entries; the fixture controls stand in for each, so that no selection is
/// ever proved by running the real suite.
pub trait Entry {
    type Exit;
    /// The live suite.
    fn suite(&mut self) -> Self::Exit;
    /// The observation-only mode.
    fn cleanup_observation(&mut self) -> Self::Exit;
    /// A mode the live suite itself starts, with the arguments after its own.
    fn mode(&mut self, mode: Mode, args: &[String]) -> Self::Exit;
    /// Nothing runs: the arguments select nothing here.
    fn refuse(&mut self, refusal: Refusal) -> Self::Exit;
}

/// Run exactly the entry `args` (the program name first) select, before
/// anything else happens: the suite only for no argument at all, the
/// observation only for exactly [`CLEANUP_OBSERVATION`], a probe or driver
/// mode only by its own first argument, and a refusal for anything else. Where
/// the sandbox does not exist (`supported` false), the suite's own entry is
/// kept (it says the sandbox is unavailable) and everything else is refused.
pub fn dispatch<E: Entry>(args: &[OsString], supported: bool, entry: &mut E) -> E::Exit {
    let Some((_program, rest)) = args.split_first() else {
        return entry.refuse(Refusal::NoProgram);
    };
    let Some(rest) = rest
        .iter()
        .map(|argument| argument.to_str().map(str::to_owned))
        .collect::<Option<Vec<String>>>()
    else {
        return entry.refuse(Refusal::NotUtf8);
    };
    let Some(first) = rest.first() else {
        return entry.suite();
    };
    if first == CLEANUP_OBSERVATION {
        if !supported {
            return entry.refuse(Refusal::Unsupported(first.clone()));
        }
        if rest.len() != 1 {
            return entry.refuse(Refusal::ObservationArguments(rest.len() - 1));
        }
        return entry.cleanup_observation();
    }
    match Mode::ALL.iter().find(|(flag, _)| flag == first) {
        Some(&(_, mode)) if supported => entry.mode(mode, &rest[1..]),
        Some(_) => entry.refuse(Refusal::Unsupported(first.clone())),
        None => entry.refuse(Refusal::Unknown(first.clone())),
    }
}

/// `text` made safe and short for one report line: control characters (and
/// quotes and backslashes) escaped, at most [`MAX_DIAGNOSTIC`] characters.
pub fn diagnostic(text: impl fmt::Display) -> String {
    let text = text.to_string();
    let mut out = String::new();
    for (count, c) in text.chars().flat_map(char::escape_debug).enumerate() {
        if count == MAX_DIAGNOSTIC {
            out.push_str("...");
            break;
        }
        out.push(c);
    }
    out
}

/// `describe()`, or `otherwise` should it panic: describing never unwinds
/// through an owner the caller still holds.
pub fn contained<T>(describe: impl FnOnce() -> T, otherwise: T) -> T {
    catch_unwind(AssertUnwindSafe(describe)).unwrap_or(otherwise)
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
/// scope observation taken then (already reported). The cleanup is settled
/// first, whatever they were. Every failure is kept: an observation failure
/// even when the cleanup is then confirmed, a cleanup confirmed only after the
/// first attempt, and an unconfirmed cleanup, which keeps its owner.
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
/// defense-in-depth backstop (a retained boundary's), which is never a
/// confirmation. Returns the report.
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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use linux::*;

/// The observations themselves, which exist only where the sandbox does.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use std::ffi::{CStr, CString};
    use std::fmt;
    use std::future::Future;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;
    use std::time::{Duration, Instant};

    use zbus::zvariant::OwnedObjectPath;

    use super::{contained, diagnostic, Scopes};

    /// Bound on one observation: reaching the bus, authenticating and the
    /// call, together, as the sandbox bounds each call to the user manager.
    pub const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(10);
    /// The user manager's bus name, object and interface.
    pub const MANAGER: &str = "org.freedesktop.systemd1";
    pub const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
    pub const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
    /// The one method called: `ListUnitsByPatterns(as states, as patterns)`.
    pub const LIST_UNITS: &str = "ListUnitsByPatterns";
    /// No state filter: loaded units in every state.
    pub const STATES: [&str; 0] = [];
    /// The units observed.
    pub const SCOPE_PATTERN: &str = "nexus-verifier-*.scope";
    pub const PATTERNS: [&str; 1] = [SCOPE_PATTERN];
    /// The documented reply: (name, description, load state, active state,
    /// sub state, following, unit path, job id, job type, job path) per unit.
    pub const REPLY_SIGNATURE: &str = "a(ssssssouso)";
    /// Bound on an accepted reply's body, checked before it is decoded.
    pub const MAX_REPLY_BODY: usize = 64 * 1024;
    /// Bound on the units an accepted reply lists.
    pub const MAX_UNITS: usize = 256;
    /// Bound on a unit name (systemd's own) and on `following`.
    pub const MAX_NAME: usize = 255;
    /// Bound on a state or job type.
    pub const MAX_STATE: usize = 64;
    /// Bound on a description or object path.
    pub const MAX_TEXT: usize = 1024;
    /// The verification workspaces directory, beneath the runtime directory.
    pub const WORKSPACES: &str = "nexus-verifier";
    /// Bound on the workspace entries read and named in a report.
    pub const MAX_LISTED: usize = 64;
    const TMPFS_MAGIC: i64 = 0x0102_1994;

    /// One unit of the documented reply.
    pub type UnitRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        OwnedObjectPath,
        u32,
        String,
        OwnedObjectPath,
    );

    /// Why an observation has no answer. Every text is bounded
    /// ([`diagnostic`]).
    #[derive(Debug)]
    pub enum ObservationError {
        /// The user runtime directory or its bus is not what it must be.
        Runtime(String),
        /// The bus could not be reached or authenticated with, the connection
        /// ended, or the call could not be made.
        Bus(String),
        /// The user manager answered the call with an error.
        Refused { name: String, message: String },
        /// No answer by the observation's deadline.
        Timeout,
        /// The answer is not a list of verifier scopes within its bounds.
        Malformed(String),
        /// The workspaces directory could not be inspected, or is not what it
        /// must be.
        Inspection(String),
        /// The observation itself panicked.
        Panicked,
    }

    impl fmt::Display for ObservationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Runtime(why) => write!(f, "no usable user runtime directory or bus: {why}"),
                Self::Bus(why) => write!(f, "{why}"),
                Self::Refused { name, message } => {
                    write!(
                        f,
                        "the user manager refused the listing ({name}): {message}"
                    )
                }
                Self::Timeout => write!(f, "no answer by the observation's deadline"),
                Self::Malformed(why) => {
                    write!(f, "the answer is not a list of verifier scopes: {why}")
                }
                Self::Inspection(why) => write!(f, "{why}"),
                Self::Panicked => write!(f, "the observation panicked"),
            }
        }
    }

    /// Where the user runtime directory is checked, and against what. The
    /// real host is `/`, root, a tmpfs and this process's real uid; the
    /// fixture controls substitute their own.
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

    fn open_at(dir: &OwnedFd, name: &str, flags: libc::c_int) -> io::Result<OwnedFd> {
        let name = CString::new(name)?;
        // SAFETY: name is NUL-terminated; openat returns a new descriptor
        // owned here on success.
        let fd = check(unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })?;
        // SAFETY: openat succeeded, so fd is new and owned here.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    /// The directory `name` beneath `dir`, never through a symlink at
    /// `name`.
    pub fn open_dir_at(dir: &OwnedFd, name: &str) -> io::Result<OwnedFd> {
        open_at(dir, name, libc::O_RDONLY | libc::O_DIRECTORY)
    }

    pub fn fstat(fd: &OwnedFd) -> io::Result<libc::stat> {
        // SAFETY: stat is plain data; fstat fills it.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fstat writes one stat structure.
        check(unsafe { libc::fstat(fd.as_raw_fd(), &mut st) })?;
        Ok(st)
    }

    /// The filesystem type (`statfs.f_type`) of the object `path` names.
    pub fn filesystem_type(path: &Path) -> io::Result<i64> {
        filesystem_type_of(&open_dir(path)?)
    }

    pub fn filesystem_type_of(fd: &OwnedFd) -> io::Result<i64> {
        // SAFETY: statfs is plain data; fstatfs fills it.
        let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: fstatfs writes one statfs structure.
        check(unsafe { libc::fstatfs(fd.as_raw_fd(), &mut fs) })?;
        Ok(fs.f_type)
    }

    /// What a directory held: at most the bound's names (sorted), and
    /// whether there were others.
    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub struct Listing {
        pub names: Vec<String>,
        pub more: bool,
    }

    impl Listing {
        pub fn is_empty(&self) -> bool {
            self.names.is_empty() && !self.more
        }
    }

    /// The entries of the directory `dir` (`.` and `..` aside), read
    /// through its own descriptor without changing anything: at most `limit`
    /// names, then `more` if another entry exists.
    pub fn list_dir(dir: &OwnedFd, limit: usize) -> io::Result<Listing> {
        // SAFETY: F_DUPFD_CLOEXEC returns a new descriptor for the same open
        // directory, owned by the stream below.
        let own = check(unsafe { libc::fcntl(dir.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) })?;
        // SAFETY: own is a directory descriptor; fdopendir takes ownership of
        // it on success.
        let stream = unsafe { libc::fdopendir(own) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            // SAFETY: fdopendir failed, so own is still this function's.
            unsafe { libc::close(own) };
            return Err(error);
        }
        struct Stream(*mut libc::DIR);
        impl Drop for Stream {
            fn drop(&mut self) {
                // SAFETY: the stream opened above, closed once.
                unsafe { libc::closedir(self.0) };
            }
        }
        let stream = Stream(stream);
        // SAFETY: a valid stream; the duplicate shares the offset, so read
        // from the start.
        unsafe { libc::rewinddir(stream.0) };
        let mut listing = Listing::default();
        loop {
            // SAFETY: errno is this thread's; cleared to tell the end of the
            // stream from an error.
            unsafe { *libc::__errno_location() = 0 };
            // SAFETY: a valid stream.
            let entry = unsafe { libc::readdir64(stream.0) };
            if entry.is_null() {
                // SAFETY: as above.
                let errno = unsafe { *libc::__errno_location() };
                if errno != 0 {
                    return Err(io::Error::from_raw_os_error(errno));
                }
                listing.names.sort();
                return Ok(listing);
            }
            // SAFETY: readdir64 returned an entry whose name is
            // NUL-terminated.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if listing.names.len() == limit {
                listing.more = true;
                listing.names.sort();
                return Ok(listing);
            }
            listing
                .names
                .push(String::from_utf8_lossy(name).into_owned());
        }
    }

    /// The filesystem operations an inspection uses: [`Fs::real`], or a
    /// fixture's injected failures.
    pub struct Fs<'a> {
        pub open_dir_at: &'a dyn Fn(&OwnedFd, &str) -> io::Result<OwnedFd>,
        pub fstat: &'a dyn Fn(&OwnedFd) -> io::Result<libc::stat>,
        pub fs_type: &'a dyn Fn(&OwnedFd) -> io::Result<i64>,
        pub list: &'a dyn Fn(&OwnedFd, usize) -> io::Result<Listing>,
    }

    impl Fs<'static> {
        pub fn real() -> Self {
            Fs {
                open_dir_at: &open_dir_at,
                fstat: &fstat,
                fs_type: &filesystem_type_of,
                list: &list_dir,
            }
        }
    }

    /// The checked runtime directory `/run/user/<uid>` beneath `host.root`:
    /// its descriptor and its stat.
    fn runtime_dir(
        host: &Host<'_>,
        fs: &Fs<'_>,
    ) -> Result<(OwnedFd, libc::stat), ObservationError> {
        let runtime_path = format!("/run/user/{}", host.uid);
        let unusable = |what: &str, error: io::Error| {
            ObservationError::Runtime(diagnostic(format!("{what} cannot be inspected: {error}")))
        };
        let mut dir = open_dir(host.root).map_err(|error| unusable("/", error))?;
        for (name, path) in [("run", "/run"), ("user", "/run/user")] {
            dir = (fs.open_dir_at)(&dir, name).map_err(|error| unusable(path, error))?;
            let st = (fs.fstat)(&dir).map_err(|error| unusable(path, error))?;
            if st.st_uid != host.root_owner || st.st_mode & 0o022 != 0 {
                return Err(ObservationError::Runtime(format!(
                    "{path} is not root-owned or is writable by others"
                )));
            }
        }
        let runtime = (fs.open_dir_at)(&dir, &host.uid.to_string())
            .map_err(|error| unusable(&runtime_path, error))?;
        let st = (fs.fstat)(&runtime).map_err(|error| unusable(&runtime_path, error))?;
        if st.st_mode & libc::S_IFMT != libc::S_IFDIR
            || st.st_uid != host.uid
            || st.st_mode & 0o7777 != 0o700
        {
            return Err(ObservationError::Runtime(format!(
                "{runtime_path} is not a private directory of uid {}",
                host.uid
            )));
        }
        if let Some(magic) = host.fs_magic {
            let found = (fs.fs_type)(&runtime).map_err(|error| unusable(&runtime_path, error))?;
            if found != magic {
                return Err(ObservationError::Runtime(format!(
                    "{runtime_path} is not a tmpfs"
                )));
            }
        }
        Ok((runtime, st))
    }

    /// The checked user bus of `host.uid`: the socket `/run/user/<uid>/bus`
    /// beneath `host.root`, held by its own `O_PATH` descriptor. Connecting
    /// goes through that descriptor, so the socket connected to is the one
    /// checked, never a name resolved again.
    pub struct UserBus {
        socket: OwnedFd,
    }

    impl UserBus {
        /// This process's own path to the checked socket.
        fn path(&self) -> PathBuf {
            PathBuf::from(format!("/proc/self/fd/{}", self.socket.as_raw_fd()))
        }

        /// The checked socket's device and inode.
        pub fn identity(&self) -> io::Result<(u64, u64)> {
            let st = fstat(&self.socket)?;
            Ok((st.st_dev, st.st_ino))
        }
    }

    pub fn user_bus(host: &Host<'_>) -> Result<UserBus, ObservationError> {
        let (runtime, _) = runtime_dir(host, &Fs::real())?;
        let bus = format!("/run/user/{}/bus", host.uid);
        let socket = open_at(&runtime, "bus", libc::O_PATH).map_err(|error| {
            ObservationError::Runtime(diagnostic(format!("{bus} cannot be inspected: {error}")))
        })?;
        let st = fstat(&socket).map_err(|error| {
            ObservationError::Runtime(diagnostic(format!("{bus} cannot be inspected: {error}")))
        })?;
        if st.st_mode & libc::S_IFMT != libc::S_IFSOCK || st.st_uid != host.uid {
            return Err(ObservationError::Runtime(format!(
                "{bus} is not a socket of uid {}",
                host.uid
            )));
        }
        Ok(UserBus { socket })
    }

    /// The verifier scopes the user manager of this process's real uid has
    /// loaded: one observation, within [`OBSERVATION_TIMEOUT`].
    pub fn observe_scopes() -> Result<Scopes, ObservationError> {
        observe_scopes_by(Instant::now() + OBSERVATION_TIMEOUT)
    }

    /// The same, by `deadline` (and never later than [`OBSERVATION_TIMEOUT`]
    /// from now): a caller's deadline is never extended.
    pub fn observe_scopes_by(deadline: Instant) -> Result<Scopes, ObservationError> {
        observe_scopes_on(&Host::real(), deadline)
    }

    /// One observation on the checked user bus of `host`, by `deadline`
    /// (and never later than [`OBSERVATION_TIMEOUT`] from now).
    pub fn observe_scopes_on(
        host: &Host<'_>,
        deadline: Instant,
    ) -> Result<Scopes, ObservationError> {
        let deadline = deadline.min(Instant::now() + OBSERVATION_TIMEOUT);
        let bus = user_bus(host)?;
        // SAFETY: geteuid has no preconditions.
        let euid = unsafe { libc::geteuid() };
        if euid != host.uid {
            return Err(ObservationError::Runtime(format!(
                "the effective uid {euid} is not uid {}: EXTERNAL authentication would not name \
                 the checked bus's owner",
                host.uid
            )));
        }
        let path = bus.path();
        let scopes = list_scopes(move || tokio::net::UnixStream::connect(path), deadline);
        // The checked socket's descriptor is held until the observation ended.
        drop(bus);
        scopes
    }

    fn bus_error(what: &str, error: impl fmt::Display) -> ObservationError {
        ObservationError::Bus(diagnostic(format!("{what}: {error}")))
    }

    fn call_error(error: zbus::Error) -> ObservationError {
        match error {
            zbus::Error::MethodError(name, message, _) => ObservationError::Refused {
                name: diagnostic(name),
                message: diagnostic(message.unwrap_or_default()),
            },
            other => bus_error("the listing call failed", other),
        }
    }

    /// One observation over the stream `connect` opens: authenticate
    /// (EXTERNAL only: this process's own uid), call the manager's listing
    /// once, decode the reply. Everything up to the reply is bounded by
    /// `deadline` together, on a runtime of this observation's own, which
    /// ends (and with it the connection and every task of it) before the
    /// reply is decoded and this returns. Nothing is retried and nothing
    /// outlives the observation.
    pub fn list_scopes<C, F>(connect: C, deadline: Instant) -> Result<Scopes, ObservationError>
    where
        C: FnOnce() -> F,
        F: Future<Output = io::Result<tokio::net::UnixStream>>,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|error| bus_error("no runtime for the observation", error))?;
        let answer = runtime.block_on(async {
            let call = async {
                let stream = connect()
                    .await
                    .map_err(|error| bus_error("the user bus could not be reached", error))?;
                let connection = zbus::connection::Builder::unix_stream(stream)
                    .auth_mechanism(zbus::AuthMechanism::External)
                    .build()
                    .await
                    .map_err(|error| bus_error("the user bus connection failed", error))?;
                connection
                    .call_method(
                        Some(MANAGER),
                        MANAGER_PATH,
                        Some(MANAGER_INTERFACE),
                        LIST_UNITS,
                        &(&STATES[..], &PATTERNS[..]),
                    )
                    .await
                    .map_err(call_error)
            };
            tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), call).await
        });
        // The observation's runtime ends here, before anything is judged:
        // with it go the connection and every task it started.
        drop(runtime);
        match answer {
            Err(_elapsed) => Err(ObservationError::Timeout),
            Ok(Err(error)) => Err(error),
            Ok(Ok(reply)) => decode_scopes(&reply),
        }
    }

    fn malformed(why: impl fmt::Display) -> ObservationError {
        ObservationError::Malformed(diagnostic(why))
    }

    /// A unit name as systemd writes one: `[A-Za-z0-9:_.\\@-]`, at most
    /// [`MAX_NAME`] bytes.
    fn is_unit_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= MAX_NAME
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b":-_.\\@".contains(&byte))
    }

    /// A verifier scope's unit name: `nexus-verifier-<name>.scope`.
    pub fn is_verifier_scope(unit: &str) -> bool {
        is_unit_name(unit)
            && unit
                .strip_prefix("nexus-verifier-")
                .and_then(|rest| rest.strip_suffix(".scope"))
                .is_some_and(|name| !name.is_empty())
    }

    /// A state as systemd names one (`active`, `not-found`, ...), at most
    /// [`MAX_STATE`] bytes.
    fn is_state(state: &str) -> bool {
        state.len() <= MAX_STATE
            && state
                .bytes()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
            && state
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    }

    fn check_row(row: &UnitRow) -> Result<(), ObservationError> {
        let (name, description, load, active, sub, following, unit_path, _job, job_type, job_path) =
            row;
        if !is_verifier_scope(name) {
            return Err(malformed(format!("{name:?} is not a verifier scope")));
        }
        for (what, state) in [("load", load), ("active", active), ("sub", sub)] {
            if !is_state(state) {
                return Err(malformed(format!(
                    "{name}: unexpected {what} state {state:?}"
                )));
            }
        }
        if !(job_type.is_empty() || is_state(job_type)) {
            return Err(malformed(format!(
                "{name}: unexpected job type {job_type:?}"
            )));
        }
        if !(following.is_empty() || is_unit_name(following)) {
            return Err(malformed(format!(
                "{name}: unexpected following unit {following:?}"
            )));
        }
        for (what, text) in [
            ("description", description.as_str()),
            ("unit path", unit_path.as_str()),
            ("job path", job_path.as_str()),
        ] {
            if text.len() > MAX_TEXT {
                return Err(malformed(format!(
                    "{name}: its {what} is longer than {MAX_TEXT} bytes"
                )));
            }
        }
        Ok(())
    }

    /// The scopes a reply lists: exactly the documented `a(ssssssouso)`, its
    /// body no larger than [`MAX_REPLY_BODY`] (checked before it is decoded)
    /// and nothing but that one list (every byte of it consumed by decoding
    /// it), no descriptors, at most [`MAX_UNITS`] units, each a distinct
    /// verifier scope with bounded fields. Anything else is malformed, never
    /// "no scopes".
    pub fn decode_scopes(reply: &zbus::Message) -> Result<Scopes, ObservationError> {
        if reply.message_type() != zbus::message::Type::MethodReturn {
            return Err(malformed(format!(
                "a {:?} message, not a method return",
                reply.message_type()
            )));
        }
        let body = reply.body();
        if body.len() > MAX_REPLY_BODY {
            return Err(malformed(format!(
                "its body is {} bytes, more than {MAX_REPLY_BODY}",
                body.len()
            )));
        }
        if !reply.data().fds().is_empty() {
            return Err(malformed("it carries file descriptors"));
        }
        match body.signature() {
            Some(signature) if signature.as_str() == REPLY_SIGNATURE => {}
            other => {
                return Err(malformed(format!(
                    "its signature is {:?}, not {REPLY_SIGNATURE}",
                    other.map(|signature| signature.as_str().to_string())
                )))
            }
        }
        // Decoded from the body's own data (its context and descriptors),
        // keeping how many bytes the list took: the body must be that list
        // and nothing more.
        let (rows, consumed): (Vec<UnitRow>, usize) = body
            .data()
            .deserialize_for_dynamic_signature(REPLY_SIGNATURE)
            .map_err(|error| malformed(format!("it cannot be decoded: {error}")))?;
        if consumed != body.len() {
            return Err(malformed(format!(
                "{} bytes of its body come after the list it declares (the list took {consumed} \
                 of {})",
                body.len().abs_diff(consumed),
                body.len()
            )));
        }
        if rows.len() > MAX_UNITS {
            return Err(malformed(format!(
                "{} units, more than {MAX_UNITS}",
                rows.len()
            )));
        }
        let mut scopes = Scopes::new();
        for row in &rows {
            check_row(row)?;
            if !scopes.insert(row.0.clone()) {
                return Err(malformed(format!("{} is listed twice", row.0)));
            }
        }
        Ok(scopes)
    }

    /// Observe with `observe` until `done` holds of the loaded scopes or the
    /// deadline `within` from now passes. Every observation is given that one
    /// deadline (it never extends it). A failed observation ends the wait
    /// with that failure: it never counts as done.
    pub fn wait_for(
        within: Duration,
        mut observe: impl FnMut(Instant) -> Result<Scopes, ObservationError>,
        mut done: impl FnMut(&Scopes) -> bool,
    ) -> Result<bool, ObservationError> {
        let deadline = Instant::now() + within;
        loop {
            if done(&observe(deadline)?) {
                return Ok(true);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            std::thread::sleep(left.min(Duration::from_millis(20)));
        }
    }

    /// What the workspaces directory held when it was observed.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Workspaces {
        /// `nexus-verifier` was absent beneath the checked runtime directory
        /// at observation time (which says nothing about any earlier time).
        Absent,
        /// Its entries.
        Listed(Listing),
    }

    /// The verification workspaces of `host`'s uid.
    pub fn observe_workspaces(host: &Host<'_>) -> Result<Workspaces, ObservationError> {
        observe_workspaces_with(host, &Fs::real())
    }

    /// The same, through `fs`: the entries of `/run/user/<uid>/nexus-verifier`
    /// beneath `host.root`, opened beneath the checked runtime directory's
    /// descriptor (never through a symlink) and checked to be an owner-only
    /// directory of the uid on the runtime directory's filesystem before it
    /// is listed. Absent only as a missing final component of a runtime
    /// directory still there; any other failure is an error, never "none".
    pub fn observe_workspaces_with(
        host: &Host<'_>,
        fs: &Fs<'_>,
    ) -> Result<Workspaces, ObservationError> {
        let (runtime, runtime_st) = runtime_dir(host, fs)?;
        let path = format!("/run/user/{}/{WORKSPACES}", host.uid);
        let inspection = |what: &str, error: io::Error| {
            ObservationError::Inspection(diagnostic(format!("{path} {what}: {error}")))
        };
        let dir = match (fs.open_dir_at)(&runtime, WORKSPACES) {
            Ok(dir) => dir,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                // The permitted absence: nothing by that name in the checked
                // runtime directory, which is still there.
                let st = (fs.fstat)(&runtime)
                    .map_err(|error| inspection("cannot be inspected", error))?;
                if st.st_nlink == 0 {
                    return Err(ObservationError::Inspection(format!(
                        "/run/user/{} was removed while it was inspected",
                        host.uid
                    )));
                }
                return Ok(Workspaces::Absent);
            }
            Err(error) => return Err(inspection("cannot be opened", error)),
        };
        let st = (fs.fstat)(&dir).map_err(|error| inspection("cannot be inspected", error))?;
        if st.st_mode & libc::S_IFMT != libc::S_IFDIR
            || st.st_uid != host.uid
            || st.st_mode & 0o7777 != 0o700
            || st.st_dev != runtime_st.st_dev
        {
            return Err(ObservationError::Inspection(format!(
                "{path} is not an owner-only directory of uid {} on the runtime directory's \
                 filesystem",
                host.uid
            )));
        }
        let listing =
            (fs.list)(&dir, MAX_LISTED).map_err(|error| inspection("cannot be listed", error))?;
        Ok(Workspaces::Listed(listing))
    }

    /// Both observations, reported to `out` one line each (a GitHub
    /// annotation for every failure or finding): true only if both answered
    /// and found nothing. Each is attempted whatever the other did, a panic
    /// in one included, and neither result hides the other.
    pub fn report_cleanup(
        scopes: impl FnOnce() -> Result<Scopes, ObservationError>,
        workspaces: impl FnOnce() -> Result<Workspaces, ObservationError>,
        out: &mut dyn FnMut(String),
    ) -> bool {
        let scopes = contained(scopes, Err(ObservationError::Panicked));
        let workspaces = contained(workspaces, Err(ObservationError::Panicked));
        let mut clean = true;
        match scopes {
            Ok(found) if found.is_empty() => {
                out("verifier scopes loaded by the user manager: none".to_string())
            }
            Ok(found) => {
                clean = false;
                let named: Vec<String> = found.iter().take(16).map(diagnostic).collect();
                let more = if found.len() > named.len() {
                    format!(" and {} more", found.len() - named.len())
                } else {
                    String::new()
                };
                out(format!(
                    "::error::a verifier scope was left behind: {}{more}",
                    named.join(", ")
                ));
            }
            Err(error) => {
                clean = false;
                out(format!(
                    "::error::the verifier scope observation failed: {error}"
                ));
            }
        }
        match workspaces {
            Ok(Workspaces::Absent) => out(format!(
                "verification workspaces: none ({WORKSPACES} absent at observation time)"
            )),
            Ok(Workspaces::Listed(listing)) if listing.is_empty() => {
                out("verification workspaces: none".to_string())
            }
            Ok(Workspaces::Listed(listing)) => {
                clean = false;
                let more = if listing.more { ", and more" } else { "" };
                out(format!(
                    "::error::a verification workspace was left behind: {}{more}",
                    listing
                        .names
                        .iter()
                        .map(|name| format!("{name:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Err(error) => {
                clean = false;
                out(format!(
                    "::error::the verification workspace observation failed: {error}"
                ));
            }
        }
        clean
    }

    /// The observation-only mode: both observations of this process's own
    /// host, reported on standard output. It holds no authority and stops,
    /// kills, creates, repairs or removes nothing; success only if both
    /// answered and found nothing.
    pub fn cleanup_observation_mode() -> ExitCode {
        let host = Host::real();
        let clean = report_cleanup(observe_scopes, || observe_workspaces(&host), &mut |line| {
            println!("{line}")
        });
        if clean {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}
