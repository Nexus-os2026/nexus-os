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
//!
//! **Uncertain remote operations (P2-V1-R3B-I4).** StartTransientUnit and
//! StopUnit are remote calls whose effect and reply are independent: a
//! timeout or a broken transport proves nothing about what the manager did,
//! and a reply proves nothing about what is left. So from the moment a start
//! request may have been dispatched, the possible scope is owned as a
//! [`PendingScope`] until independent observation either proves it (then,
//! and only then, it becomes a [`Scope`]) or confirms that nothing it may
//! have created can still hold or receive a process. A pending scope is bound
//! to the retained helper, the backend-generated unit name (a locator, never
//! authority), the expected limits and the manager connection that issued
//! the request; it retains the candidate cgroup by descriptor as soon as the
//! kernel reports the helper in it, so no later failed proof can lose it. It
//! is owned by the execution (and its retained boundary) and stays retryable
//! after the [`ScopeManager`] that started it is gone. Nothing is
//! reconstructed from a unit name, a process id or a path, and nothing
//! sweeps units by name. The states and transitions are documented in
//! `scope/pending.rs`.

use std::io;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::launcher::Helper;
use crate::policy::ResourcePolicy;

mod manager;
mod native;
mod pending;
#[cfg(test)]
pub(crate) mod tests;

pub use manager::BUS_CALL_TIMEOUT;
#[cfg(test)]
pub(crate) use pending::PendingState;
pub(crate) use pending::ScopeBoundary;
pub use pending::{PendingScope, StartFailed};

use manager::{Manager, ZbusManager};
use native::{CgroupDir, Kernel, Native};

/// Bound on waiting for systemd to move the helper into its scope.
pub const PLACEMENT_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on confirming that what a pending scope may have created is gone,
/// after each StopUnit attempt (P2-V1-R3B-I4).
pub const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

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

/// The bounds a controller observes with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timing {
    /// Waiting for the helper to appear in the new scope.
    pub placement: Duration,
    /// Confirming a pending scope gone after a StopUnit attempt.
    pub settle: Duration,
    /// Between two observations.
    pub poll: Duration,
}

impl Timing {
    pub(crate) const PRODUCTION: Self = Self {
        placement: PLACEMENT_TIMEOUT,
        settle: SETTLE_TIMEOUT,
        poll: Duration::from_millis(5),
    };
}

/// The manager connection and the kernel observations one [`ScopeManager`]
/// works through, shared (reference-counted) with every pending scope it
/// issued, so that a retained boundary can reconcile its operation on the
/// very connection that issued it after the [`ScopeManager`] is dropped.
pub(crate) struct Controller {
    pub(crate) manager: Box<dyn Manager>,
    pub(crate) native: Box<dyn Native>,
    pub(crate) timing: Timing,
}

/// A connection to the systemd user manager of this process's real uid.
pub struct ScopeManager {
    controller: Arc<Controller>,
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
        Ok(Self {
            controller: Arc::new(Controller {
                manager: Box::new(ZbusManager::connect_at(path)?),
                native: Box::new(Kernel),
                timing: Timing::PRODUCTION,
            }),
        })
    }

    /// A manager over a deterministic test controller: unit tests only.
    #[cfg(test)]
    pub(crate) fn with_controller(controller: Controller) -> Self {
        Self {
            controller: Arc::new(controller),
        }
    }

    /// A new scope operation for `helper`, the backend's own unreaped child,
    /// with `limits`: a fresh backend-generated unit name, nothing issued
    /// yet. The caller owns it before any request can have an effect.
    pub(crate) fn prepare(
        &self,
        helper: &Helper,
        limits: &ResourcePolicy,
    ) -> Result<Box<PendingScope>, ScopeError> {
        let unit = format!("nexus-verifier-{}.scope", random_hex()?);
        Ok(Box::new(PendingScope::new(
            Arc::clone(&self.controller),
            unit,
            helper,
            limits,
        )))
    }

    /// Create a scope holding exactly `helper`, the backend's own unreaped
    /// child, with `limits`, and prove it. The helper's process id is only
    /// the locator of that retained child.
    ///
    /// When no scope is proven, whatever the attempt may have created is
    /// settled first (a panic while proving included, as before): the
    /// failure carries it, owned, unless it was confirmed gone
    /// (P2-V1-R3B-I4).
    pub fn start(&self, helper: &Helper, limits: &ResourcePolicy) -> Result<Scope, StartFailed> {
        let pending = self.prepare(helper, limits).map_err(|error| StartFailed {
            error,
            unresolved: None,
        })?;
        let mut boundary = ScopeBoundary::Pending(pending);
        let error = match catch_unwind(AssertUnwindSafe(|| boundary.establish(helper, None))) {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error),
            Err(panic) => {
                if let ScopeBoundary::Pending(pending) = &mut boundary {
                    let _ = catch_unwind(AssertUnwindSafe(|| pending.settle(helper)));
                }
                resume_unwind(panic)
            }
        };
        match boundary {
            ScopeBoundary::Proven(scope) => Ok(scope),
            boundary => {
                let mut unresolved = boundary.into_pending();
                if unresolved
                    .as_mut()
                    .is_some_and(|pending| pending.settle(helper))
                {
                    unresolved = None;
                }
                Err(StartFailed {
                    error: error.unwrap_or(ScopeError::Mismatch("scope not proven")),
                    unresolved,
                })
            }
        }
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

/// `cgroup.events`' `populated` field, through the retained descriptor.
fn read_populated(dir: &dyn CgroupDir) -> io::Result<Occupancy> {
    let events = dir.read("cgroup.events")?;
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

/// Whether any process remains in the retained cgroup. A retained cgroup is
/// a non-root cgroup v2 directory (verified by its file system, and named
/// for its unit), and every one has `cgroup.events`; that file goes only
/// with the directory, whose listing must then be empty.
fn occupancy(dir: &dyn CgroupDir) -> io::Result<Occupancy> {
    match read_populated(dir) {
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => {
            if dir.listing_is_empty()? {
                Ok(Occupancy::Removed)
            } else {
                Err(error)
            }
        }
        other => other,
    }
}

/// Every limit file holds exactly the expected text.
fn verify_limits(dir: &dyn CgroupDir, limits: &ResourcePolicy) -> Result<(), ScopeError> {
    for (file, expected) in expected_limit_files(limits) {
        let actual = dir.read(file).map_err(ScopeError::Io)?;
        if actual.trim_end() != expected {
            return Err(ScopeError::Mismatch(file));
        }
    }
    Ok(())
}

/// A verifier execution's cgroup, retained by descriptor: a proven scope.
/// The only way to one is a [`PendingScope`] whose every proof passed.
#[derive(Debug)]
pub struct Scope {
    unit: String,
    dir: Box<dyn CgroupDir>,
}

impl Scope {
    /// The backend-generated unit name (display and diagnostics only).
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Kill every process in the scope (`cgroup.kill`).
    pub fn kill(&self) -> io::Result<()> {
        self.dir.kill()
    }

    /// Whether any process remains in the scope.
    pub fn occupancy(&self) -> io::Result<Occupancy> {
        occupancy(self.dir.as_ref())
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
            self.dir
                .read(file)?
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
