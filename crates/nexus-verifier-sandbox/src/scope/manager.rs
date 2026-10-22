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
//!
//! **The started unit's own identity (P2-V1-R3B-I4-R3-R1).** A unit name is
//! reusable; the identity of one invocation of a unit is its
//! `InvocationID`, a random 128-bit value systemd draws on every start and
//! never lets a caller set (`docs/evidence/p2-v1-r3b-i4-r3-r1-candidate-ownership/`).
//! An accepted start captures the identity of the invocation it started from
//! that start's own job: before the request is sent, the manager is
//! subscribed and two signals are matched, the start job's `JobRemoved` and
//! `PropertiesChanged` on the unit's object path; systemd sends the unit's
//! pending change, `InvocationID` included, before it removes the job, and
//! the unit cannot be unloaded while the job is installed. The sender's own
//! message serials order the reply, those changes and the removal
//! ([`captured_instance`]). Nothing else ever yields an identity, and
//! nothing captured is ever authority by itself: it is only compared with
//! what the manager reports later.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::ScopeError;
use crate::policy::ResourcePolicy;

const SYSTEMD: &str = "org.freedesktop.systemd1";
const SYSTEMD_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const UNIT_INTERFACE: &str = "org.freedesktop.systemd1.Unit";
const SCOPE_INTERFACE: &str = "org.freedesktop.systemd1.Scope";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// systemd's answer to GetUnit for a unit it has not loaded.
const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";
/// systemd's answer to StartTransientUnit for a name already loaded: the
/// request was refused before it could have any effect.
const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";
/// systemd's answer to a second Subscribe from the same client.
const ALREADY_SUBSCRIBED: &str = "org.freedesktop.systemd1.AlreadySubscribed";
/// The prefix of every unit's object path.
const UNIT_PATH_PREFIX: &str = "/org/freedesktop/systemd1/unit/";
/// Room for the few signals one start's watch receives.
const WATCH_QUEUE: usize = 64;

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

/// The identity of one invocation of a unit: its `InvocationID`, exactly 16
/// bytes, never all zero. Crate-private and never serialized; made only from
/// the manager's own property bytes ([`Self::from_property`]), never from a
/// caller, a unit name or a path. Holding one grants nothing: it is only
/// compared with what the manager reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnitInstance([u8; 16]);

impl UnitInstance {
    /// The manager's encoding (`ay`): exactly 16 bytes, not all zero (a unit
    /// never started encodes an empty array).
    pub(crate) fn from_property(bytes: &[u8]) -> Option<Self> {
        let id: [u8; 16] = bytes.try_into().ok()?;
        (id != [0; 16]).then_some(Self(id))
    }

    /// The 16 bytes, for the live host qualification's comparison with the
    /// manager's own answer.
    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub(crate) fn bytes(&self) -> [u8; 16] {
        self.0
    }
}

/// What StartTransientUnit returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Started {
    /// The success reply (a job path) was delivered: the manager created
    /// the unit for this request. With the identity of the invocation this
    /// start began, captured from the start's own job
    /// ([`captured_instance`]), or why none was.
    Accepted(Result<UnitInstance, String>),
    /// The manager refused because a unit of that name is already loaded:
    /// this request had no effect, and that unit is not this request's.
    Collision,
    /// Anything else: the request may or may not have taken effect.
    Uncertain(String),
    /// The request was never sent (its signals could not be watched first):
    /// nothing can exist.
    NotIssued(String),
}

/// What one `PropertiesChanged` for the started unit's unit interface said
/// of its `InvocationID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invocation {
    /// The empty array of a unit not yet started.
    Null,
    Id(UnitInstance),
    /// Another length, all zero, or another type.
    Malformed,
}

/// One such change, by the serial its sender gave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnitChange {
    pub serial: u32,
    pub invocation: Invocation,
}

/// The identity of the unit invocation an accepted start began: the start
/// job's `JobRemoved` reports `done` and has a serial above the reply's, and
/// among the unit's changes from the same sender with serials strictly
/// between the two, exactly one distinct identity appears and none is
/// malformed. While the job is installed the unit cannot be unloaded and the
/// job is its only start, so such an identity is the one this start drew
/// (the analysis in the evidence). Anything else is no identity: fail closed.
pub(crate) fn captured_instance(
    reply_serial: u32,
    removed_serial: u32,
    result: &str,
    changes: &[UnitChange],
) -> Result<UnitInstance, String> {
    if result != "done" {
        return Err(format!("the start job ended {result}"));
    }
    if removed_serial <= reply_serial {
        return Err("the start job's removal is not after its reply".into());
    }
    let mut captured: Option<UnitInstance> = None;
    for change in changes
        .iter()
        .filter(|change| change.serial > reply_serial && change.serial < removed_serial)
    {
        match change.invocation {
            Invocation::Null => {}
            Invocation::Malformed => return Err("a malformed invocation identity".into()),
            Invocation::Id(id) => match captured {
                Some(seen) if seen != id => return Err("more than one invocation identity".into()),
                _ => captured = Some(id),
            },
        }
    }
    captured.ok_or_else(|| "no invocation identity before the start job's removal".into())
}

/// The object path the manager gives the unit `name`: systemd's
/// `unit_dbus_path_from_name` (every byte but an ASCII letter, or a digit
/// after the first byte, escaped as `_` and two lowercase hex digits).
pub(crate) fn unit_object_path(name: &str) -> String {
    let mut path = String::from(UNIT_PATH_PREFIX);
    if name.is_empty() {
        path.push('_');
    }
    for (index, byte) in name.bytes().enumerate() {
        if byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit()) {
            path.push(char::from(byte));
        } else {
            path.push_str(&format!("_{byte:02x}"));
        }
    }
    path
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
///
/// One request acts (the start, for a fresh name); every other one only
/// reads, except `Subscribe`, which acts on no unit: it asks the manager to
/// send its signals, so that a start can capture its own identity.
/// Nothing here acts on a unit by its name or its object path
/// (P2-V1-R3B-I4-R3): a name is a locator and evidence, never a retained
/// identity, so a pending operation is ended only through the cgroup it
/// retained by descriptor, once that cgroup is bound to the invocation the
/// start began (`super::pending`, P2-V1-R3B-I4-R3-R1).
pub(crate) trait Manager: Send + Sync {
    fn start_scope(&self, request: &ScopeRequest<'_>) -> Started;
    fn get_unit(&self, unit: &str) -> Remote<Presence>;
    /// The scope's `RuntimeMaxUSec`, at the object path GetUnit returned;
    /// `Answered(None)`: a value that is not an unsigned integer.
    fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>>;
    /// The scope's `OOMPolicy`, at the object path GetUnit returned;
    /// `Answered(None)`: a value that is not a string.
    fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>>;
    /// The unit's primary name (`Id`, of the unit interface), at the object
    /// path GetUnit returned; `Answered(None)`: a value that is not a string.
    fn unit_id(&self, unit_path: &str) -> Remote<Option<String>>;
    /// The scope's control group path (`ControlGroup`), at the object path
    /// GetUnit returned; `Answered(None)`: a value that is not a string.
    fn control_group(&self, unit_path: &str) -> Remote<Option<String>>;
    /// The unit's `InvocationID`, at the object path GetUnit returned;
    /// `Answered(None)`: not exactly 16 bytes, all zero (never started) or of
    /// another type.
    fn unit_instance(&self, unit_path: &str) -> Remote<Option<UnitInstance>>;
    /// The scope's `ControlGroupId` (the kernel's ID of its cgroup
    /// directory), at the object path GetUnit returned; `Answered(None)`: a
    /// value that is not an unsigned integer.
    fn control_group_id(&self, unit_path: &str) -> Remote<Option<u64>>;
}

/// The user manager over its D-Bus interface.
pub(crate) struct ZbusManager {
    /// Shut down, never dropped in place: a retained pending scope may be
    /// dropped on a thread inside an asynchronous runtime.
    runtime: Option<tokio::runtime::Runtime>,
    connection: zbus::Connection,
    /// The manager sends this connection its signals (`Subscribe`).
    subscribed: AtomicBool,
}

/// One start's two matched signals, registered before its request is sent.
/// Dropped only inside its runtime's context, unwinding included: dropping a
/// matched stream queues the removal of its match on that runtime.
struct StartWatch {
    runtime: tokio::runtime::Handle,
    removed: Option<zbus::MessageStream>,
    changed: Option<zbus::MessageStream>,
}

impl Drop for StartWatch {
    fn drop(&mut self) {
        let _context = self.runtime.enter();
        self.removed = None;
        self.changed = None;
    }
}

/// What binds a start's signals to its reply: the reply's sender (the
/// manager's unique name, set by the bus) and serial, and the job path.
struct StartReply {
    sender: String,
    serial: u32,
    job: String,
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
            subscribed: AtomicBool::new(false),
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
        self.call_reply(path, interface, method, body)
            .map(|reply| reply.map(|(_, value)| value))
    }

    /// [`Self::call`], with the reply message itself.
    fn call_reply<B, R>(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        body: &B,
    ) -> Result<Result<(zbus::Message, R), String>, String>
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
            let value = reply
                .body()
                .deserialize::<R>()
                .map_err(|e| format!("{method}: malformed reply: {e}"))?;
            Ok(Ok((reply, value)))
        })
    }

    /// Before a start's request exists: the manager subscribed (once per
    /// connection), and its `JobRemoved` for the unit and the unit's
    /// `PropertiesChanged` on its unit interface matched. Bounded.
    fn watch_start(&self, unit: &str) -> Result<StartWatch, String> {
        if !self.subscribed.load(Ordering::Acquire) {
            match self.call::<_, ()>(SYSTEMD_PATH, MANAGER, "Subscribe", &()) {
                Ok(Ok(())) => {}
                Ok(Err(name)) if name == ALREADY_SUBSCRIBED => {}
                Ok(Err(name)) => return Err(format!("Subscribe: {name}")),
                Err(reason) => return Err(reason),
            }
            self.subscribed.store(true, Ordering::Release);
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return Err("the manager connection is shut down".into());
        };
        let object = unit_object_path(unit);
        runtime.block_on(async {
            let watch = async {
                use zbus::message::Type;
                let removed = zbus::MatchRule::builder()
                    .msg_type(Type::Signal)
                    .sender(SYSTEMD)?
                    .path(SYSTEMD_PATH)?
                    .interface(MANAGER)?
                    .member("JobRemoved")?
                    .arg(2, unit)?
                    .build();
                let changed = zbus::MatchRule::builder()
                    .msg_type(Type::Signal)
                    .sender(SYSTEMD)?
                    .path(object.as_str())?
                    .interface(PROPERTIES)?
                    .member("PropertiesChanged")?
                    .arg(0, UNIT_INTERFACE)?
                    .build();
                Ok::<_, zbus::Error>(StartWatch {
                    runtime: runtime.handle().clone(),
                    removed: Some(
                        zbus::MessageStream::for_match_rule(
                            removed,
                            &self.connection,
                            Some(WATCH_QUEUE),
                        )
                        .await?,
                    ),
                    changed: Some(
                        zbus::MessageStream::for_match_rule(
                            changed,
                            &self.connection,
                            Some(WATCH_QUEUE),
                        )
                        .await?,
                    ),
                })
            };
            match tokio::time::timeout(BUS_CALL_TIMEOUT, watch).await {
                Ok(Ok(watch)) => Ok(watch),
                Ok(Err(error)) => Err(format!("watching the start: {error}")),
                Err(_) => Err("watching the start timed out".into()),
            }
        })
    }

    /// The identity of the invocation an accepted start began, from its own
    /// watched signals ([`captured_instance`]): the start job's
    /// `JobRemoved`, and every change of the unit delivered before it, all
    /// from the sender of the reply. Both watches are read together, so that
    /// neither can hold the other back. Bounded; anything missing captures
    /// nothing.
    fn capture(&self, mut watch: StartWatch, reply: &StartReply) -> Result<UnitInstance, String> {
        use zbus::export::futures_util::future::{select, Either};
        use zbus::export::futures_util::{FutureExt, StreamExt};
        let Some(runtime) = self.runtime.as_ref() else {
            return Err("the manager connection is shut down".into());
        };
        let (Some(removed), Some(changed)) = (watch.removed.as_mut(), watch.changed.as_mut())
        else {
            return Err("the start's signals are not watched".into());
        };
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + BUS_CALL_TIMEOUT;
            let mut changes = Vec::new();
            let (removed_serial, result) = loop {
                match tokio::time::timeout_at(deadline, select(removed.next(), changed.next()))
                    .await
                {
                    Err(_) => return Err("JobRemoved timed out".into()),
                    Ok(Either::Left((Some(Ok(message)), _))) => {
                        if let Some(removal) = removal_of(&message, reply) {
                            break removal;
                        }
                    }
                    Ok(Either::Right((Some(Ok(message)), _))) => {
                        changes.extend(change_of(&message, reply));
                    }
                    Ok(_) => return Err("the start's signals: the watch ended".into()),
                }
            };
            // The socket reader hands each message to every matching watch
            // before it reads the next, so every change delivered before the
            // removal is queued by now.
            while let Some(Some(Ok(message))) = changed.next().now_or_never() {
                changes.extend(change_of(&message, reply));
            }
            captured_instance(reply.serial, removed_serial, &result, &changes)
        })
    }
}

/// Whether `message` was sent by the connection whose unique name the bus
/// gave the reply (the bus sets the sender; a client cannot).
fn sent_by(message: &zbus::Message, sender: &str) -> bool {
    message
        .header()
        .sender()
        .is_some_and(|name| name.as_str() == sender)
}

/// The serial and result of the start job's own `JobRemoved`, if `message`
/// is it: from the reply's sender, for exactly the reply's job path.
fn removal_of(message: &zbus::Message, reply: &StartReply) -> Option<(u32, String)> {
    if !sent_by(message, &reply.sender) {
        return None;
    }
    let (_, job, _, result) = message
        .body()
        .deserialize::<(u32, zbus::zvariant::OwnedObjectPath, String, String)>()
        .ok()?;
    (job.as_str() == reply.job).then(|| (message.primary_header().serial_num().get(), result))
}

/// What one `PropertiesChanged` from the reply's sender, on the unit
/// interface, says of `InvocationID`; `None` when it says nothing of it.
fn change_of(message: &zbus::Message, reply: &StartReply) -> Option<UnitChange> {
    if !sent_by(message, &reply.sender) {
        return None;
    }
    let (interface, changed, _invalidated) = message
        .body()
        .deserialize::<(
            String,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            Vec<String>,
        )>()
        .ok()?;
    if interface != UNIT_INTERFACE {
        return None;
    }
    let value = changed.get("InvocationID")?.try_clone().ok()?;
    Some(UnitChange {
        serial: message.primary_header().serial_num().get(),
        invocation: invocation_of(value),
    })
}

/// What the manager's `InvocationID` value says: the null identity (an
/// empty `ay`: a unit never started), an identity (an `ay` of exactly 16
/// bytes, not all zero), or nothing usable (another length, all zero, or
/// another type).
pub(crate) fn invocation_of(value: zbus::zvariant::OwnedValue) -> Invocation {
    match Vec::<u8>::try_from(value) {
        Ok(bytes) if bytes.is_empty() => Invocation::Null,
        Ok(bytes) => {
            UnitInstance::from_property(&bytes).map_or(Invocation::Malformed, Invocation::Id)
        }
        Err(_) => Invocation::Malformed,
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
        // The start's own signals are matched before its request exists, or
        // the request is never sent.
        let watch = match self.watch_start(request.unit) {
            Ok(watch) => watch,
            Err(reason) => return Started::NotIssued(reason),
        };
        match self.call_reply::<_, zbus::zvariant::OwnedObjectPath>(
            SYSTEMD_PATH,
            MANAGER,
            "StartTransientUnit",
            &(request.unit, "fail", properties, aux),
        ) {
            Ok(Ok((reply, job))) => Started::Accepted(match reply.header().sender() {
                Some(sender) => self.capture(
                    watch,
                    &StartReply {
                        sender: sender.as_str().to_string(),
                        serial: reply.primary_header().serial_num().get(),
                        job: job.as_str().to_string(),
                    },
                ),
                None => Err("the start's reply has no sender".into()),
            }),
            Ok(Err(name)) if name == UNIT_EXISTS => Started::Collision,
            Ok(Err(name)) => Started::Uncertain(format!("StartTransientUnit: {name}")),
            Err(reason) => Started::Uncertain(reason),
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

    fn unit_id(&self, unit_path: &str) -> Remote<Option<String>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(UNIT_INTERFACE, "Id"),
        ) {
            Ok(Ok(value)) => Remote::Answered(String::try_from(value).ok()),
            Ok(Err(name)) => Remote::Uncertain(format!("Get Id: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn control_group(&self, unit_path: &str) -> Remote<Option<String>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "ControlGroup"),
        ) {
            Ok(Ok(value)) => Remote::Answered(String::try_from(value).ok()),
            Ok(Err(name)) => Remote::Uncertain(format!("Get ControlGroup: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn unit_instance(&self, unit_path: &str) -> Remote<Option<UnitInstance>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(UNIT_INTERFACE, "InvocationID"),
        ) {
            Ok(Ok(value)) => Remote::Answered(match invocation_of(value) {
                Invocation::Id(id) => Some(id),
                Invocation::Null | Invocation::Malformed => None,
            }),
            Ok(Err(name)) => Remote::Uncertain(format!("Get InvocationID: {name}")),
            Err(reason) => Remote::Uncertain(reason),
        }
    }

    fn control_group_id(&self, unit_path: &str) -> Remote<Option<u64>> {
        match self.call::<_, zbus::zvariant::OwnedValue>(
            unit_path,
            PROPERTIES,
            "Get",
            &(SCOPE_INTERFACE, "ControlGroupId"),
        ) {
            Ok(Ok(value)) => Remote::Answered(u64::try_from(value).ok()),
            Ok(Err(name)) => Remote::Uncertain(format!("Get ControlGroupId: {name}")),
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
