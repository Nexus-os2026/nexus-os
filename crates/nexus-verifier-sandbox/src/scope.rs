//! Backend-owned cgroup v2 scopes for verifier executions (Linux).
//!
//! The backend asks the systemd user manager, over its fixed D-Bus
//! interface, for a transient scope holding exactly the helper process, with
//! the profile's limits. systemd is only the mechanism that creates and
//! limits the cgroup; none of its sandbox properties is part of the boundary.
//!
//! Nothing is taken on trust: the bus socket is derived from this process's
//! real uid (never the environment) and must be a socket owned by that uid;
//! every call is bounded; the helper's membership is proven from the
//! kernel's view of the backend's own unreaped child; the cgroup directory is
//! then retained by descriptor, must be a populated cgroup v2 directory
//! containing the helper, and must carry exactly the requested limits. Later
//! operations (`cgroup.kill`, counters, emptiness) go through that retained
//! descriptor, never through a path string or the unit name.
//!
//! systemd removes a scope's cgroup once it is empty. The kernel removes a
//! cgroup only while it is unpopulated, and a removed cgroup can never hold a
//! process again, so a retained directory that has been removed is empty for
//! good. Removal is recognised only through the retained descriptor: the
//! `cgroup.events` file every live cgroup has is gone and the directory lists
//! nothing.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use crate::fault::{self, Fault, FaultPoint};
use crate::launcher::Helper;
use crate::policy::ResourcePolicy;
use crate::sys;

const SYSTEMD: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const CGROUP2_SUPER_MAGIC: i64 = 0x6367_7270;
/// Bound on each D-Bus call.
pub const BUS_CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on waiting for systemd to move the helper into its scope.
pub const PLACEMENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum ScopeError {
    /// No usable user manager bus (missing, not a socket, wrong owner).
    BusUnavailable(io::Error),
    /// A D-Bus call failed or timed out.
    Bus(String),
    /// The helper did not appear in the new scope in time.
    NotPlaced,
    /// The scope is not what was requested (not cgroup v2, not populated,
    /// helper absent, limits or policy differ).
    Mismatch(&'static str),
    Io(io::Error),
}

/// The fixed text each limit file must hold for `limits`. An out-of-memory
/// kill ends only the chosen process, never the whole scope, so the backend
/// can still read the scope's counters afterwards.
pub fn expected_limit_files(limits: &ResourcePolicy) -> [(&'static str, String); 5] {
    let quota = limits.cpu_quota_us_per_sec / 10; // cpu.max period is 100 ms
    [
        ("memory.max", limits.memory_max_bytes.to_string()),
        ("memory.swap.max", limits.swap_max_bytes.to_string()),
        ("memory.oom.group", "0".to_string()),
        ("pids.max", limits.pids_max.to_string()),
        ("cpu.max", format!("{quota} 100000")),
    ]
}

/// A connection to the systemd user manager of this process's real uid.
pub struct ScopeManager {
    runtime: tokio::runtime::Runtime,
    connection: zbus::Connection,
}

impl ScopeManager {
    /// Connect to `/run/user/<uid>/bus`, derived from the real uid.
    pub fn connect() -> Result<Self, ScopeError> {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        Self::connect_at(&format!("/run/user/{uid}/bus"))
    }

    /// Connect to the user manager bus socket at `path`, which must be a
    /// socket owned by this process's real uid.
    pub fn connect_at(path: &str) -> Result<Self, ScopeError> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let metadata = std::fs::symlink_metadata(path).map_err(ScopeError::BusUnavailable)?;
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        if !metadata.file_type().is_socket() || metadata.uid() != uid {
            return Err(ScopeError::BusUnavailable(io::Error::from_raw_os_error(
                libc::EACCES,
            )));
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(ScopeError::Io)?;
        let address = format!("unix:path={path}");
        let connection = runtime
            .block_on(async {
                tokio::time::timeout(BUS_CALL_TIMEOUT, async {
                    zbus::connection::Builder::address(address.as_str())?
                        .build()
                        .await
                })
                .await
            })
            .map_err(|_| ScopeError::Bus("connection timed out".into()))?
            .map_err(|e| ScopeError::Bus(e.to_string()))?;
        Ok(Self {
            runtime,
            connection,
        })
    }

    fn call<B, R>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<R, ScopeError>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        self.runtime.block_on(async {
            let reply = tokio::time::timeout(
                BUS_CALL_TIMEOUT,
                self.connection
                    .call_method(Some(SYSTEMD), path, Some(interface), method, body),
            )
            .await
            .map_err(|_| ScopeError::Bus(format!("{method} timed out")))?
            .map_err(|e| ScopeError::Bus(e.to_string()))?;
            reply
                .body()
                .deserialize::<R>()
                .map_err(|e| ScopeError::Bus(e.to_string()))
        })
    }

    /// Create a scope holding exactly `helper`, the backend's own unreaped
    /// child, with `limits`, and prove it. The helper's process id is only
    /// the locator of that retained child.
    pub fn start(&self, helper: &Helper, limits: &ResourcePolicy) -> Result<Scope, ScopeError> {
        self.start_with_fault(helper, limits, None)
    }

    /// [`Self::start`], with the live panic controls' fault injection.
    pub(crate) fn start_with_fault(
        &self,
        helper: &Helper,
        limits: &ResourcePolicy,
        fault: Option<Fault>,
    ) -> Result<Scope, ScopeError> {
        use zbus::zvariant::Value;
        let helper_pid = helper.pid();
        let unit = format!("nexus-verifier-{}.scope", random_hex()?);
        let properties: Vec<(&str, Value<'_>)> = vec![
            ("Description", Value::from("Nexus verifier execution")),
            ("PIDs", Value::from(vec![helper_pid])),
            ("MemoryAccounting", Value::from(true)),
            ("MemoryMax", Value::from(limits.memory_max_bytes)),
            ("MemorySwapMax", Value::from(limits.swap_max_bytes)),
            ("TasksAccounting", Value::from(true)),
            ("TasksMax", Value::from(limits.pids_max)),
            ("CPUAccounting", Value::from(true)),
            (
                "CPUQuotaPerSecUSec",
                Value::from(limits.cpu_quota_us_per_sec),
            ),
            (
                "RuntimeMaxUSec",
                Value::from(limits.runtime_backstop_secs * 1_000_000),
            ),
            ("KillSignal", Value::from(libc::SIGKILL)),
            // The manager must not stop the scope when the kernel kills one
            // process for memory: the backend classifies that itself.
            ("OOMPolicy", Value::from("continue")),
            ("CollectMode", Value::from("inactive-or-failed")),
        ];
        let aux: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();
        let _job: zbus::zvariant::OwnedObjectPath = self.call(
            SYSTEMD_PATH,
            MANAGER,
            "StartTransientUnit",
            &(unit.as_str(), "fail", properties, aux),
        )?;
        // The unit is this call's own creation: one that is not proven, a
        // panic while proving it included, is stopped here (a helper that
        // never entered it leaves it empty, and an empty scope is never
        // stopped by its manager). The panic then continues to the
        // execution, which still owns the helper.
        let proven = catch_unwind(AssertUnwindSafe(|| {
            self.prove(helper_pid, unit.clone(), limits, fault)
        }));
        if !matches!(proven, Ok(Ok(_))) {
            let _: Result<zbus::zvariant::OwnedObjectPath, _> = self.call(
                SYSTEMD_PATH,
                MANAGER,
                "StopUnit",
                &(unit.as_str(), "replace"),
            );
        }
        proven.unwrap_or_else(|panic| resume_unwind(panic))
    }

    fn prove(
        &self,
        helper_pid: u32,
        unit: String,
        limits: &ResourcePolicy,
        fault: Option<Fault>,
    ) -> Result<Scope, ScopeError> {
        let path = wait_for_placement(helper_pid, &unit)?;
        fault::at(fault, FaultPoint::ScopeProof);
        let scope = Scope::open(&path, unit, helper_pid)?;
        scope.verify_limits(limits)?;
        let unit_path: zbus::zvariant::OwnedObjectPath =
            self.call(SYSTEMD_PATH, MANAGER, "GetUnit", &(scope.unit.as_str(),))?;
        let backstop: zbus::zvariant::OwnedValue = self.call(
            unit_path.as_str(),
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "RuntimeMaxUSec"),
        )?;
        if u64::try_from(backstop).ok() != Some(limits.runtime_backstop_secs * 1_000_000) {
            return Err(ScopeError::Mismatch("runtime backstop"));
        }
        let oom_policy: zbus::zvariant::OwnedValue = self.call(
            unit_path.as_str(),
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "OOMPolicy"),
        )?;
        if String::try_from(oom_policy).ok().as_deref() != Some("continue") {
            return Err(ScopeError::Mismatch("out-of-memory policy"));
        }
        Ok(scope)
    }
}

fn random_hex() -> Result<String, ScopeError> {
    let mut bytes = [0u8; 16];
    // SAFETY: getrandom fills the buffer it is given.
    let n = unsafe { libc::getrandom(bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    if n != bytes.len() as isize {
        return Err(ScopeError::Io(io::Error::last_os_error()));
    }
    Ok(hex::encode(bytes))
}

/// Wait until the kernel reports the helper in a cgroup whose last component
/// is `unit`, and return that cgroup's path (relative to the cgroup root).
fn wait_for_placement(helper_pid: u32, unit: &str) -> Result<String, ScopeError> {
    let start = Instant::now();
    loop {
        let text = std::fs::read_to_string(format!("/proc/{helper_pid}/cgroup"))
            .map_err(ScopeError::Io)?;
        if let Some(path) = text.trim_end().strip_prefix("0::") {
            if path.rsplit('/').next() == Some(unit) && !path.contains("..") {
                return Ok(path.to_string());
            }
        }
        if start.elapsed() >= PLACEMENT_TIMEOUT {
            return Err(ScopeError::NotPlaced);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Counters the kernel keeps for the scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScopeEvents {
    /// `memory.events` `oom_kill`.
    pub oom_kills: u64,
    /// `pids.events` `max`: forks refused at the process limit.
    pub pids_max: u64,
}

/// What the kernel reports about the processes in a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupancy {
    /// `cgroup.events` reports `populated 1`.
    Populated,
    /// `cgroup.events` reports `populated 0`.
    Empty,
    /// The cgroup has been removed, which the kernel allows only once it is
    /// unpopulated; it can never hold a process again.
    Removed,
}

/// A verifier execution's cgroup, retained by descriptor.
#[derive(Debug)]
pub struct Scope {
    unit: String,
    dir: OwnedFd,
}

impl Scope {
    fn open(path: &str, unit: String, helper_pid: u32) -> Result<Self, ScopeError> {
        let full = CString::new(format!("/sys/fs/cgroup{path}"))
            .map_err(|_| ScopeError::Mismatch("cgroup path"))?;
        // SAFETY: a NUL-terminated path; open returns a new descriptor.
        let fd = unsafe {
            libc::open(
                full.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(ScopeError::Io(io::Error::last_os_error()));
        }
        // SAFETY: open succeeded, so fd is new and owned here.
        let dir = unsafe { OwnedFd::from_raw_fd(fd) };
        // SAFETY: statfs is plain data; fstatfs fills it.
        let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: fstatfs writes one statfs structure.
        if unsafe { libc::fstatfs(dir.as_raw_fd(), &mut fs) } != 0
            || fs.f_type != CGROUP2_SUPER_MAGIC
        {
            return Err(ScopeError::Mismatch("not a cgroup v2 directory"));
        }
        let scope = Self { unit, dir };
        // A live, non-root cgroup: its `cgroup.events` exists and reports
        // the helper's presence. Only then can a later absence of that file
        // mean removal.
        if scope.read_populated().map_err(ScopeError::Io)? != Occupancy::Populated {
            return Err(ScopeError::Mismatch("scope not populated"));
        }
        let procs = scope.read("cgroup.procs").map_err(ScopeError::Io)?;
        if !procs
            .lines()
            .any(|line| line.trim() == helper_pid.to_string())
        {
            return Err(ScopeError::Mismatch("helper not in the scope"));
        }
        Ok(scope)
    }

    fn open_file(&self, name: &str, flags: libc::c_int) -> io::Result<std::fs::File> {
        let name = CString::new(name).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        // SAFETY: openat relative to the retained directory; a new
        // descriptor is returned on success.
        let fd = unsafe {
            libc::openat(
                self.dir.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat succeeded, so fd is new and owned here.
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }

    fn read(&self, name: &str) -> io::Result<String> {
        use std::io::Read;
        let mut text = String::new();
        self.open_file(name, libc::O_RDONLY)?
            .read_to_string(&mut text)?;
        Ok(text)
    }

    fn verify_limits(&self, limits: &ResourcePolicy) -> Result<(), ScopeError> {
        for (file, expected) in expected_limit_files(limits) {
            let actual = self.read(file).map_err(ScopeError::Io)?;
            if actual.trim_end() != expected {
                return Err(ScopeError::Mismatch(file));
            }
        }
        Ok(())
    }

    /// The backend-generated unit name (display and diagnostics only).
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Kill every process in the scope (`cgroup.kill`).
    pub fn kill(&self) -> io::Result<()> {
        use std::io::Write;
        self.open_file("cgroup.kill", libc::O_WRONLY)?
            .write_all(b"1")
    }

    fn read_populated(&self) -> io::Result<Occupancy> {
        let events = self.read("cgroup.events")?;
        match events
            .lines()
            .find_map(|line| line.strip_prefix("populated "))
            .map(str::trim)
        {
            Some("1") => Ok(Occupancy::Populated),
            Some("0") => Ok(Occupancy::Empty),
            _ => Err(io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Whether any process remains in the scope.
    pub fn occupancy(&self) -> io::Result<Occupancy> {
        match self.read_populated() {
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
                // `cgroup.events` existed when the scope was opened; it goes
                // only with the directory, whose listing must then be empty.
                if sys::directory_is_empty(self.dir.as_fd())? {
                    Ok(Occupancy::Removed)
                } else {
                    Err(error)
                }
            }
            other => other,
        }
    }

    /// Wait up to `timeout` for the scope to be empty or removed.
    pub fn wait_empty(&self, timeout: Duration) -> io::Result<bool> {
        let start = Instant::now();
        loop {
            if self.occupancy()? != Occupancy::Populated {
                return Ok(true);
            }
            if start.elapsed() >= timeout {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The scope's counters. They exist only while the scope does: the
    /// helper holds the scope after its final report until they are read.
    pub fn events(&self) -> io::Result<ScopeEvents> {
        let counter = |file: &str, key: &str| -> io::Result<u64> {
            self.read(file)?
                .lines()
                .find_map(|line| line.strip_prefix(key)?.trim().parse().ok())
                .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))
        };
        Ok(ScopeEvents {
            oom_kills: counter("memory.events", "oom_kill ")?,
            pids_max: counter("pids.events", "max ")?,
        })
    }
}
