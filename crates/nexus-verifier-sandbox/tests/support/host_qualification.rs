//! The live harness's host-qualification probe (P2-V1-R3B-I4-Q1): bounded,
//! read-mostly observations of the real systemd user manager and the real
//! kernel, made beside the production path to show that the facts the
//! accepted scope mechanism depends on hold on the supported host.
//!
//! Nothing here is production code or production authority: this file is
//! part of the live harness (`tests/`), compiled into no library and reached
//! by no normal build. Its connection is its own, to the checked user bus of
//! this process's real uid (`cleanup_observation::user_bus`: the socket
//! `/run/user/<uid>/bus` of the uid, beneath a checked runtime directory,
//! held by its own descriptor), authenticated with EXTERNAL only. Every call
//! is bounded by [`CALL_TIMEOUT`]. A reply is either the documented return,
//! decoded, or the manager's error with its exact name; a timeout, a
//! transport failure or a reply that does not decode is an error of the
//! probe, never an answer.
//!
//! The strings it returns (unit names, object paths, `Id`, `ControlGroup`,
//! kernel membership) are evidence for the Architect, never authority: the
//! production proof remains `scope::pending`'s, over the retained descriptor
//! and the crate's own fixed-destination manager. The one thing the probe
//! creates (a deliberately existing, uniquely named transient scope for the
//! collision case) it owns and ends itself.
#![allow(dead_code)]

use std::fmt;
use std::io;
use std::time::Duration;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::cleanup_observation::{diagnostic, user_bus, Host, UserBus};

/// systemd's definite answers the production manager relies on
/// (`scope/manager.rs`): the exact D-Bus error names. The desktop guard
/// `p2_g_10` pins these to the production constants.
pub const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";
pub const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";

/// The user manager's bus name, object and interfaces.
pub const SYSTEMD: &str = "org.freedesktop.systemd1";
pub const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
pub const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";
pub const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";
pub const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";
pub const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// Bound on each call: reaching the bus and authenticating (once), and every
/// method call.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// `statfs` magic of the unified cgroup v2 hierarchy.
pub const CGROUP2_SUPER_MAGIC: i64 = 0x6367_7270;
/// `statfs` magic of a tmpfs (the runtime directory).
pub const TMPFS_MAGIC: i64 = 0x0102_1994;

/// What the manager answered a call with.
#[derive(Debug)]
pub enum Answer<T> {
    /// The documented return, decoded.
    Returned(T),
    /// The manager's error, by its exact name.
    Refused { name: String, message: String },
}

/// Why the probe has no answer. Never read as an answer of any kind.
#[derive(Debug)]
pub enum ProbeError {
    Bus(String),
    Timeout,
    Malformed(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(why) => write!(f, "bus: {why}"),
            Self::Timeout => write!(f, "no answer within {:?}", CALL_TIMEOUT),
            Self::Malformed(why) => write!(f, "the reply does not decode: {why}"),
        }
    }
}

/// One connection to the checked user bus, for bounded calls.
pub struct Probe {
    connection: Option<zbus::Connection>,
    runtime: Option<tokio::runtime::Runtime>,
    /// The checked socket's descriptor, held while the connection lives.
    _bus: UserBus,
}

impl Probe {
    /// Connect to the checked user bus of this process's real uid.
    pub fn connect() -> Result<Self, ProbeError> {
        let host = Host::real();
        let bus = user_bus(&host).map_err(|error| ProbeError::Bus(error.to_string()))?;
        // SAFETY: geteuid has no preconditions.
        let euid = unsafe { libc::geteuid() };
        if euid != host.uid {
            return Err(ProbeError::Bus(format!(
                "the effective uid {euid} is not uid {}",
                host.uid
            )));
        }
        // Through the checked socket's own descriptor: never a name resolved
        // again.
        let path = bus.path();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|error| ProbeError::Bus(format!("no runtime: {error}")))?;
        let connection = runtime.block_on(async {
            tokio::time::timeout(CALL_TIMEOUT, async {
                let stream = tokio::net::UnixStream::connect(path)
                    .await
                    .map_err(|error| ProbeError::Bus(format!("connect: {error}")))?;
                zbus::connection::Builder::unix_stream(stream)
                    .auth_mechanism(zbus::AuthMechanism::External)
                    .build()
                    .await
                    .map_err(|error| ProbeError::Bus(format!("authenticate: {error}")))
            })
            .await
            .map_err(|_| ProbeError::Timeout)?
        })?;
        Ok(Self {
            connection: Some(connection),
            runtime: Some(runtime),
            _bus: bus,
        })
    }

    /// One bounded call to the fixed destination.
    fn call<B, R>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<Answer<R>, ProbeError>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        let (Some(runtime), Some(connection)) = (self.runtime.as_ref(), self.connection.as_ref())
        else {
            return Err(ProbeError::Bus("the probe is shut down".into()));
        };
        runtime.block_on(async {
            let reply = tokio::time::timeout(
                CALL_TIMEOUT,
                connection.call_method(Some(SYSTEMD), path, Some(interface), method, body),
            )
            .await
            .map_err(|_| ProbeError::Timeout)?;
            match reply {
                Ok(reply) => reply
                    .body()
                    .deserialize::<R>()
                    .map(Answer::Returned)
                    .map_err(|error| ProbeError::Malformed(diagnostic(error))),
                Err(zbus::Error::MethodError(name, message, _)) => Ok(Answer::Refused {
                    name: name.as_str().to_string(),
                    message: diagnostic(message.unwrap_or_default()),
                }),
                Err(other) => Err(ProbeError::Bus(diagnostic(other))),
            }
        })
    }

    /// `GetUnit(name)`: the unit's object path, or the manager's error.
    pub fn get_unit(&self, unit: &str) -> Result<Answer<OwnedObjectPath>, ProbeError> {
        self.call(SYSTEMD_PATH, MANAGER_INTERFACE, "GetUnit", &(unit,))
    }

    /// `Properties.Get(interface, name)` at `object`.
    pub fn property(
        &self,
        object: &str,
        interface: &str,
        name: &str,
    ) -> Result<Answer<OwnedValue>, ProbeError> {
        self.call(object, PROPERTIES, "Get", &(interface, name))
    }

    /// `StartTransientUnit(unit, "fail", ...)` for a scope holding exactly
    /// `pid`: the deliberately existing unit of the collision case. The
    /// caller owns `pid` and the unit it creates, and ends both.
    pub fn start_transient_scope(
        &self,
        unit: &str,
        pid: u32,
    ) -> Result<Answer<OwnedObjectPath>, ProbeError> {
        let properties: Vec<(&str, Value<'_>)> = vec![
            (
                "Description",
                Value::from("Nexus P2-V1-R3B-I4-Q1 host qualification (owned by the live harness)"),
            ),
            ("PIDs", Value::from(vec![pid])),
            ("KillSignal", Value::from(libc::SIGKILL)),
            ("CollectMode", Value::from("inactive-or-failed")),
        ];
        let aux: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();
        self.call(
            SYSTEMD_PATH,
            MANAGER_INTERFACE,
            "StartTransientUnit",
            &(unit, "fail", properties, aux),
        )
    }

    /// `StopUnit(unit, "replace")`: for the unit this harness created only.
    /// Its reply is diagnostics; whether the unit is gone is observed.
    pub fn stop_unit(&self, unit: &str) -> Result<Answer<OwnedObjectPath>, ProbeError> {
        self.call(
            SYSTEMD_PATH,
            MANAGER_INTERFACE,
            "StopUnit",
            &(unit, "replace"),
        )
    }

    /// Whether the manager answers `GetUnit(unit)` with exactly
    /// [`NO_SUCH_UNIT`].
    pub fn is_no_such_unit(&self, unit: &str) -> bool {
        matches!(self.get_unit(unit), Ok(Answer::Refused { name, .. }) if name == NO_SUCH_UNIT)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        // The connection ends on its runtime, which is then shut down without
        // blocking (a panicking case may drop the probe anywhere).
        if let Some(runtime) = self.runtime.take() {
            if let Some(connection) = self.connection.take() {
                runtime.block_on(async move { drop(connection) });
            }
            runtime.shutdown_background();
        }
    }
}

/// A fresh, unpredictable unit name in the verifier scopes' form,
/// `nexus-verifier-<tag><32 hex>.scope`: never created before, and (with
/// its prefix) visible to the cleanup observation if it were ever left.
pub fn fresh_unit_name(tag: &str) -> String {
    let mut bytes = [0u8; 16];
    // SAFETY: getrandom fills the buffer it is given.
    let n = unsafe { libc::getrandom(bytes.as_mut_ptr().cast(), bytes.len(), 0) };
    assert_eq!(n, bytes.len() as isize, "getrandom");
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("nexus-verifier-{tag}{hex}.scope")
}

/// What `/proc/<pid>/cgroup` says, read independently of the production
/// kernel view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    /// The file's text, as read.
    pub raw: String,
    /// The unified hierarchy's path: present only when the file is exactly
    /// one line of the form `0::<path>` (the shape I4 requires; a hybrid or
    /// v1 host has other lines and yields none).
    pub unified: Option<String>,
}

impl Membership {
    /// The path without the kernel's ` (deleted)` mark, and whether the mark
    /// was there (the cgroup was removed before the process was reaped).
    pub fn path_and_deleted(&self) -> Option<(&str, bool)> {
        let unified = self.unified.as_deref()?;
        Some(match unified.strip_suffix(" (deleted)") {
            Some(path) => (path, true),
            None => (unified, false),
        })
    }
}

/// Read `/proc/<pid>/cgroup`.
pub fn membership(pid: u32) -> io::Result<Membership> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
    let mut lines = raw.lines();
    let unified = match (lines.next(), lines.next()) {
        (Some(line), None) => line.strip_prefix("0::").map(str::to_string),
        _ => None,
    };
    Ok(Membership { raw, unified })
}

/// Whether `path` is absolute and in normal form: no empty, `.` or `..`
/// component (the form the production locator requires).
pub fn is_normal_absolute(path: &str) -> bool {
    path.strip_prefix('/').is_some_and(|relative| {
        relative
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
    })
}

/// The last component of a cgroup path.
pub fn last_component(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The process state letter of `/proc/<pid>/stat` (`R`, `S`, `Z`, ...).
pub fn process_state(pid: u32) -> io::Result<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    stat.rsplit_once(") ")
        .and_then(|(_, rest)| rest.chars().next())
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))
}

/// The filesystem type magic of `path`.
pub fn filesystem_magic(path: &str) -> io::Result<i64> {
    let c = std::ffi::CString::new(path)?;
    // SAFETY: statfs is plain data; statfs fills it.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: a NUL-terminated path and one statfs structure to fill.
    if unsafe { libc::statfs(c.as_ptr(), &mut fs) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fs.f_type)
}
