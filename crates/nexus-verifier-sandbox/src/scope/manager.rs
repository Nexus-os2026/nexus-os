//! The systemd user manager operations the scope module needs, behind one
//! narrow internal interface (P2-V1-R3B-I4).
//!
//! Every operation is a remote call whose effect and whose reply are
//! independent: a reply can be lost after the manager acted, and a timeout
//! or a broken transport proves nothing about what the manager did. So each
//! outcome is either a definite answer the protocol establishes (a decoded
//! reply, or one of systemd's own error names that say what happened) or
//! [`Remote::Uncertain`]. Nothing here decides what an outcome means for
//! ownership; [`super::pending`] does, and only from independent
//! observations.
//!
//! Production uses [`ZbusManager`]: the fixed destination, object path and
//! interfaces below, on a connection to the user manager bus of this
//! process's real uid. The interface is private to the crate; the
//! deterministic test manager exists only in test builds.

use std::io;
use std::time::Duration;

use super::ScopeError;
use crate::policy::ResourcePolicy;

const SYSTEMD: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// systemd's answer to GetUnit for a unit it has not loaded.
const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";
/// systemd's answer to StartTransientUnit for a name already loaded: the
/// request was refused before it could have any effect.
const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";

/// Bound on each D-Bus call.
pub const BUS_CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// One remote operation's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Remote<T> {
    /// The manager answered, and the answer is what it says.
    Answered(T),
    /// No knowledge: a timeout, a broken transport, an unexpected error or a
    /// reply that does not decode. Never evidence of absence or of success.
    Uncertain(String),
}

/// What StartTransientUnit returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Started {
    /// A job object was returned: the manager created the unit.
    Accepted,
    /// The manager refused because a unit of that name is already loaded:
    /// this request had no effect, and that unit is not this request's.
    Collision,
    /// Anything else: the request may or may not have taken effect.
    Uncertain(String),
}

/// Whether the manager has a unit loaded under a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Presence {
    /// Loaded, at this object path.
    Present(String),
    /// Not loaded (systemd's `NoSuchUnit`).
    Absent,
}

/// What one StartTransientUnit request asks for: a scope named `unit`
/// holding exactly the helper (`helper_pid`, the locator of the backend's
/// own retained, unreaped child), with `limits`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScopeRequest<'a> {
    pub unit: &'a str,
    pub helper_pid: u32,
    pub limits: &'a ResourcePolicy,
}

/// The manager operations of the scope module. Destination, object path and
/// interfaces are fixed by the implementation; callers name only the
/// backend-generated unit and the object path the manager itself returned.
pub(crate) trait Manager: Send + Sync {
    fn start_scope(&self, request: &ScopeRequest<'_>) -> Started;
    /// StopUnit. Its outcome is never a confirmation of anything.
    fn stop_unit(&self, unit: &str) -> Remote<()>;
    fn get_unit(&self, unit: &str) -> Remote<Presence>;
    /// The scope's `RuntimeMaxUSec`, at the object path GetUnit returned;
    /// `Answered(None)`: a value that is not an unsigned integer.
    fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>>;
    /// The scope's `OOMPolicy`, at the object path GetUnit returned;
    /// `Answered(None)`: a value that is not a string.
    fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>>;
}

/// The user manager over its D-Bus interface.
pub(crate) struct ZbusManager {
    /// Shut down, never dropped in place: a retained pending scope may be
    /// dropped on a thread inside an asynchronous runtime.
    runtime: Option<tokio::runtime::Runtime>,
    connection: zbus::Connection,
}

impl ZbusManager {
    /// Connect to the user manager bus socket at `path`, which must be a
    /// socket owned by this process's real uid.
    pub(crate) fn connect_at(path: &str) -> Result<Self, ScopeError> {
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
            runtime: Some(runtime),
            connection,
        })
    }

    /// One bounded call to the fixed destination. A decoded reply is
    /// `Ok(Ok(reply))`; an error reply is `Ok(Err(error name))`; anything
    /// else (a timeout, a transport failure, a reply that does not decode)
    /// is `Err(reason)`.
    fn call<B, R>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<Result<R, String>, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
    {
        let Some(runtime) = self.runtime.as_ref() else {
            return Err("the manager connection is shut down".into());
        };
        runtime.block_on(async {
            let reply = match tokio::time::timeout(
                BUS_CALL_TIMEOUT,
                self.connection
                    .call_method(Some(SYSTEMD), path, Some(interface), method, body),
            )
            .await
            {
                Err(_) => return Err(format!("{method} timed out")),
                Ok(Err(zbus::Error::MethodError(name, _, _))) => {
                    return Ok(Err(name.as_str().to_string()))
                }
                Ok(Err(error)) => return Err(error.to_string()),
                Ok(Ok(reply)) => reply,
            };
            reply
                .body()
                .deserialize::<R>()
                .map(Ok)
                .map_err(|e| format!("{method}: malformed reply: {e}"))
        })
    }
}

impl Manager for ZbusManager {
    fn start_scope(&self, request: &ScopeRequest<'_>) -> Started {
        use zbus::zvariant::Value;
        let limits = request.limits;
        let properties: Vec<(&str, Value<'_>)> = vec![
            ("Description", Value::from("Nexus verifier execution")),
            ("PIDs", Value::from(vec![request.helper_pid])),
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
        match self.call::<_, zbus::zvariant::OwnedObjectPath>(
            SYSTEMD_PATH,
            MANAGER,
            "StartTransientUnit",
            &(request.unit, "fail", properties, aux),
        ) {
            Ok(Ok(_job)) => Started::Accepted,
            Ok(Err(name)) if name == UNIT_EXISTS => Started::Collision,
            Ok(Err(name)) => Started::Uncertain(format!("StartTransientUnit: {name}")),
            Err(reason) => Started::Uncertain(reason),
        }
    }

    fn stop_unit(&self, unit: &str) -> Remote<()> {
        match self.call::<_, zbus::zvariant::OwnedObjectPath>(
            SYSTEMD_PATH,
            MANAGER,
            "StopUnit",
            &(unit, "replace"),
        ) {
            Ok(Ok(_job)) => Remote::Answered(()),
            Ok(Err(name)) => Remote::Uncertain(format!("StopUnit: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn get_unit(&self, unit: &str) -> Remote<Presence> {
        match self.call::<_, zbus::zvariant::OwnedObjectPath>(
            SYSTEMD_PATH,
            MANAGER,
            "GetUnit",
            &(unit,),
        ) {
            Ok(Ok(path)) => Remote::Answered(Presence::Present(path.as_str().to_string())),
            Ok(Err(name)) if name == NO_SUCH_UNIT => Remote::Answered(Presence::Absent),
            Ok(Err(name)) => Remote::Uncertain(format!("GetUnit: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "RuntimeMaxUSec"),
        ) {
            Ok(Ok(value)) => Remote::Answered(u64::try_from(value).ok()),
            Ok(Err(name)) => Remote::Uncertain(format!("Get RuntimeMaxUSec: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "OOMPolicy"),
        ) {
            Ok(Ok(value)) => Remote::Answered(String::try_from(value).ok()),
            Ok(Err(name)) => Remote::Uncertain(format!("Get OOMPolicy: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }
}

impl Drop for ZbusManager {
    /// Shut the runtime down without blocking, wherever the last owner of a
    /// pending scope is dropped; the connection goes after it, as before.
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
