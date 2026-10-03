//! A deterministic simulation of the systemd user manager and of the kernel
//! for the scope module's ownership tests (P2-V1-R3B-I4, -R1). Test builds
//! only: nothing here contacts a bus, reads `/proc` for a decision or
//! creates a cgroup, and nothing here is production authority.
//!
//! The model's manager answers every call from a script: a reply delivered,
//! an effect whose reply is lost, no effect with the reply lost, a timeout,
//! a disconnection or a malformed reply; a unit's `Id` and `ControlGroup`
//! are the truth it keeps or a scripted other answer. The model's kernel
//! keeps each helper's membership and the cgroups the manager created, and
//! can report the helper in a cgroup that is not the unit's. One call can
//! be scripted to panic inside the manager or the kernel layer. Every call
//! is logged, so a test can assert what was and was not asked.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::manager::{Manager, Presence, Remote, ScopeRequest, Started};
use super::native::{CgroupDir, Native};
use super::{expected_limit_files, Controller, ScopeError, ScopeManager, Timing};
use crate::launcher::{Helper, HelperProgram};
use crate::policy::ResourcePolicy;

/// The parent of every cgroup the model's manager creates.
pub(crate) const SLICE: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice";
/// The cgroup every helper is in until the model places it.
pub(crate) const HOME: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice/tests.scope";

/// Short bounds, so that an unconfirmable case ends quickly.
pub(crate) const TIMING: Timing = Timing {
    placement: Duration::from_millis(150),
    settle: Duration::from_millis(150),
    poll: Duration::from_millis(1),
};

/// How one remote call ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reply {
    /// The reply is delivered.
    Delivered,
    /// An error reply that is not one of systemd's definite answers.
    Error,
    /// No reply within the bound.
    Timeout,
    /// The connection breaks: this call and every later one is uncertain.
    Disconnect,
    /// A reply that does not decode.
    Malformed,
}

impl Reply {
    /// The reason an uncertain outcome carries, as the production manager
    /// words it.
    fn reason(self, method: &str) -> String {
        match self {
            Self::Delivered | Self::Error => format!("{method}: org.freedesktop.DBus.Error.Failed"),
            Self::Timeout => format!("{method} timed out"),
            Self::Disconnect => "the connection is closed".to_string(),
            Self::Malformed => format!("{method}: malformed reply"),
        }
    }
}

/// What a unit's `Id` or `ControlGroup` read answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Text {
    /// The truth the model keeps: the unit's name, or the path of the
    /// cgroup the manager created for it (empty without one).
    Exact,
    /// This text instead (the unit's name substituted for `{unit}`, the
    /// slice's path for `{slice}`).
    Is(&'static str),
    /// A value of another type.
    WrongType,
    Uncertain(Reply),
}

/// What a property read answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Property {
    Exact,
    Wrong,
    /// A value of another type.
    WrongType,
    Uncertain(Reply),
}

/// A process in a unit's cgroup beyond the placed helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phantom {
    None,
    /// Ended by `cgroup.kill`.
    UntilKill,
    /// Ended only by an effective StopUnit.
    UntilStop,
    /// Ended by nothing, until the test releases it.
    Forever,
}

/// When the model's kernel shows the helper in its new cgroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Immediate,
    /// After this many membership reads.
    AfterReads(u32),
    Never,
    /// In a cgroup of this path instead, which the manager created for the
    /// unit (the unit name substituted for `{unit}`, the slice's path for
    /// `{slice}`).
    Elsewhere(&'static str),
    /// The manager creates the unit's cgroup where it always does, but the
    /// kernel reports the helper in another cgroup, of this path and with
    /// the same limits, that is not the unit's.
    Beside(&'static str),
}

/// A model call that panics instead of answering, once, after any effect
/// it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Start,
    Stop,
    GetUnit,
    UnitId,
    ControlGroup,
    RuntimeMax,
    Membership,
    Open,
}

/// The template `path` with the unit's name and the slice's path.
pub(crate) fn expand(path: &str, unit: &str) -> String {
    path.replace("{unit}", unit).replace("{slice}", SLICE)
}

/// Answers consumed one per call, then one answer for every later call.
#[derive(Debug, Clone)]
pub(crate) struct Script<T: Copy> {
    queue: VecDeque<T>,
    then: T,
}

impl<T: Copy> Script<T> {
    pub(crate) fn always(answer: T) -> Self {
        Self {
            queue: VecDeque::new(),
            then: answer,
        }
    }

    pub(crate) fn first(first: impl IntoIterator<Item = T>, then: T) -> Self {
        Self {
            queue: first.into_iter().collect(),
            then,
        }
    }

    fn next(&mut self) -> T {
        self.queue.pop_front().unwrap_or(self.then)
    }
}

/// One logged call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Call {
    /// StartTransientUnit: the unit and the helper's process id.
    Start(String, u32),
    Stop(String),
    GetUnit(String),
    /// The unit's `Id`, at the object path GetUnit returned.
    UnitId(String),
    /// The unit's `ControlGroup`, at the object path GetUnit returned.
    ControlGroup(String),
    RuntimeMax(String),
    OomPolicy(String),
    /// A membership read: the helper's process id, and whether it was then
    /// still this process's unreaped child.
    Membership(u32, bool),
    Open(String),
    Kill(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Unit {
    /// Created by a request of this backend (not a foreign unit).
    pub ours: bool,
    pub cgroup: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct Cgroup {
    pub path: String,
    pub v2: bool,
    pub removed: bool,
    pub members: BTreeSet<u32>,
    /// `cgroup.procs` lists the members.
    pub listed: bool,
    /// `cgroup.kill` (or a stop) ended every member.
    pub killed: bool,
    pub phantom: Phantom,
    pub limits: ResourcePolicy,
    /// This limit file holds `max` instead of the requested value.
    pub limit_mismatch: Option<&'static str>,
    pub kills: u32,
}

impl Cgroup {
    pub(crate) fn populated(&self) -> bool {
        let phantom = match self.phantom {
            Phantom::None => false,
            Phantom::UntilKill => !self.killed,
            Phantom::UntilStop | Phantom::Forever => true,
        };
        !self.removed && (phantom || (!self.killed && !self.members.is_empty()))
    }
}

/// The scripts and the modelled world.
#[derive(Debug)]
pub(crate) struct State {
    // StartTransientUnit.
    pub start_effect: bool,
    pub start_reply: Reply,
    /// A unit of the requested name is already loaded (foreign).
    pub collide: bool,
    /// The foreign unit's cgroup holds the helper.
    pub collision_holds_helper: bool,
    pub placement: Placement,
    /// The created unit is loaded in the manager (`false`: the kernel has
    /// a cgroup of its name the manager does not know).
    pub unit_loaded: bool,
    pub cgroup_v2: bool,
    pub listed: bool,
    pub limit_mismatch: Option<&'static str>,
    pub phantom: Phantom,
    // The other calls.
    pub get_unit: Script<Reply>,
    pub unit_id: Text,
    pub control_group: Text,
    pub runtime_max: Property,
    pub oom_policy: Property,
    /// StopUnit: whether it takes effect, and its reply.
    pub stop: Script<(bool, Reply)>,
    /// Opens that still succeed (`None`: every one).
    pub opens_left: Option<u32>,
    /// The next call of this kind panics (then the script is spent).
    pub panic_at: Option<Op>,
    // The world.
    pub broken: bool,
    pub units: BTreeMap<String, Unit>,
    pub cgroups: Vec<Cgroup>,
    pub membership: HashMap<u32, String>,
    pub calls: Vec<Call>,
    placing: Option<(u32, usize, u32)>,
}

impl Default for State {
    /// Every call delivered, every proof passing.
    fn default() -> Self {
        Self {
            start_effect: true,
            start_reply: Reply::Delivered,
            collide: false,
            collision_holds_helper: false,
            placement: Placement::Immediate,
            unit_loaded: true,
            cgroup_v2: true,
            listed: true,
            limit_mismatch: None,
            phantom: Phantom::None,
            get_unit: Script::always(Reply::Delivered),
            unit_id: Text::Exact,
            control_group: Text::Exact,
            runtime_max: Property::Exact,
            oom_policy: Property::Exact,
            stop: Script::always((true, Reply::Delivered)),
            opens_left: None,
            panic_at: None,
            broken: false,
            units: BTreeMap::new(),
            cgroups: Vec::new(),
            membership: HashMap::new(),
            calls: Vec::new(),
            placing: None,
        }
    }
}

impl State {
    /// The kernel reports `pid` in the cgroup at `index`.
    pub(crate) fn place(&mut self, pid: u32, index: usize) {
        self.cgroups[index].members.insert(pid);
        self.membership
            .insert(pid, self.cgroups[index].path.clone());
    }

    /// An effective StopUnit: the manager ends everything in the unit's
    /// cgroup and, once it is empty, removes it and unloads the unit.
    fn stopped(&mut self, unit: &str) {
        let Some(index) = self.units.get(unit).map(|unit| unit.cgroup) else {
            return;
        };
        if let Some(index) = index {
            let cgroup = &mut self.cgroups[index];
            cgroup.killed = true;
            if cgroup.phantom == Phantom::UntilStop {
                cgroup.phantom = Phantom::None;
            }
            if cgroup.populated() {
                return;
            }
            cgroup.removed = true;
            let path = cgroup.path.clone();
            for member in cgroup.members.clone() {
                self.membership.insert(member, format!("{path} (deleted)"));
            }
        }
        self.units.remove(unit);
    }

    fn connection(&mut self, reply: Reply) -> Reply {
        if self.broken {
            return Reply::Disconnect;
        }
        if reply == Reply::Disconnect {
            self.broken = true;
        }
        reply
    }

    /// The unit the backend requested (the last StartTransientUnit).
    pub(crate) fn requested(&self) -> Option<String> {
        self.calls.iter().rev().find_map(|call| match call {
            Call::Start(unit, _) => Some(unit.clone()),
            _ => None,
        })
    }

    /// The cgroup the model created for the requested unit.
    pub(crate) fn created(&self) -> Option<&Cgroup> {
        let unit = self.requested()?;
        self.cgroups
            .iter()
            .find(|cgroup| cgroup.path == format!("{SLICE}/{unit}"))
    }

    pub(crate) fn count(&self, call: impl Fn(&Call) -> bool) -> usize {
        self.calls.iter().filter(|logged| call(logged)).count()
    }

    /// Whether the call of kind `op` is scripted to panic (spending the
    /// script).
    fn panics(&mut self, op: Op) -> bool {
        if self.panic_at == Some(op) {
            self.panic_at = None;
            return true;
        }
        false
    }
}

/// Panic as the manager or the kernel layer would, with the model's lock
/// released.
fn panic_in(state: MutexGuard<'_, State>, op: Op) -> ! {
    drop(state);
    panic!("a panic inside the model's {op:?}")
}

/// The shared model.
#[derive(Debug, Clone, Default)]
pub(crate) struct World(Arc<Mutex<State>>);

impl World {
    pub(crate) fn new(script: impl FnOnce(&mut State)) -> Self {
        let world = Self::default();
        script(&mut world.lock());
        world
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn controller(&self) -> Controller {
        Controller {
            manager: Box::new(FakeManager(self.clone())),
            native: Box::new(FakeNative(self.clone())),
            timing: TIMING,
        }
    }

    /// A scope manager over this model.
    pub(crate) fn scopes(&self) -> ScopeManager {
        ScopeManager::with_controller(self.controller())
    }

    /// Release every process the model kept alive and let StopUnit act.
    pub(crate) fn release(&self) {
        let mut state = self.lock();
        for cgroup in &mut state.cgroups {
            cgroup.phantom = Phantom::None;
        }
        state.stop = Script::always((true, Reply::Delivered));
        state.get_unit = Script::always(Reply::Delivered);
        state.broken = false;
        state.opens_left = None;
        state.panic_at = None;
    }
}

struct FakeManager(World);

impl Manager for FakeManager {
    fn start_scope(&self, request: &ScopeRequest<'_>) -> Started {
        let mut state = self.0.lock();
        state
            .calls
            .push(Call::Start(request.unit.to_string(), request.helper_pid));
        if state.broken {
            return Started::Uncertain(Reply::Disconnect.reason("StartTransientUnit"));
        }
        if state.collide || state.units.contains_key(request.unit) {
            if !state.units.contains_key(request.unit) {
                let cgroup = state.collision_holds_helper.then(|| {
                    let index = state.cgroups.len();
                    state.cgroups.push(Cgroup {
                        path: format!("{SLICE}/{}", request.unit),
                        v2: true,
                        removed: false,
                        members: BTreeSet::new(),
                        listed: true,
                        killed: false,
                        phantom: Phantom::None,
                        limits: *request.limits,
                        limit_mismatch: None,
                        kills: 0,
                    });
                    state.place(request.helper_pid, index);
                    index
                });
                state.units.insert(
                    request.unit.to_string(),
                    Unit {
                        ours: false,
                        cgroup,
                    },
                );
            }
            return Started::Collision;
        }
        assert!(
            state.start_effect || state.start_reply != Reply::Delivered,
            "a delivered start always created its unit"
        );
        if state.start_effect {
            let path = match state.placement {
                Placement::Elsewhere(path) => expand(path, request.unit),
                _ => format!("{SLICE}/{}", request.unit),
            };
            let index = state.cgroups.len();
            let cgroup = Cgroup {
                path,
                v2: state.cgroup_v2,
                removed: false,
                members: BTreeSet::new(),
                listed: state.listed,
                killed: false,
                phantom: state.phantom,
                limits: *request.limits,
                limit_mismatch: state.limit_mismatch,
                kills: 0,
            };
            state.cgroups.push(cgroup);
            if state.unit_loaded {
                state.units.insert(
                    request.unit.to_string(),
                    Unit {
                        ours: true,
                        cgroup: Some(index),
                    },
                );
            }
            match state.placement {
                Placement::Immediate | Placement::Elsewhere(_) => {
                    state.place(request.helper_pid, index)
                }
                Placement::Beside(path) => {
                    let mut beside = state.cgroups[index].clone();
                    beside.path = expand(path, request.unit);
                    state.cgroups.push(beside);
                    let beside = state.cgroups.len() - 1;
                    state.place(request.helper_pid, beside);
                }
                Placement::AfterReads(reads) => {
                    state.placing = Some((request.helper_pid, index, reads))
                }
                Placement::Never => {}
            }
        }
        if state.panics(Op::Start) {
            panic_in(state, Op::Start);
        }
        let reply = state.start_reply;
        match state.connection(reply) {
            Reply::Delivered => Started::Accepted,
            reply => Started::Uncertain(reply.reason("StartTransientUnit")),
        }
    }

    fn stop_unit(&self, unit: &str) -> Remote<()> {
        let mut state = self.0.lock();
        state.calls.push(Call::Stop(unit.to_string()));
        if state.broken {
            return Remote::Uncertain(Reply::Disconnect.reason("StopUnit"));
        }
        let (effect, reply) = state.stop.next();
        if effect {
            state.stopped(unit);
        }
        if state.panics(Op::Stop) {
            panic_in(state, Op::Stop);
        }
        match state.connection(reply) {
            Reply::Delivered => Remote::Answered(()),
            reply => Remote::Uncertain(reply.reason("StopUnit")),
        }
    }

    fn get_unit(&self, unit: &str) -> Remote<Presence> {
        let mut state = self.0.lock();
        state.calls.push(Call::GetUnit(unit.to_string()));
        if state.panics(Op::GetUnit) {
            panic_in(state, Op::GetUnit);
        }
        let reply = state.get_unit.next();
        match state.connection(reply) {
            Reply::Delivered if state.units.contains_key(unit) => {
                Remote::Answered(Presence::Present(format!("/unit/{unit}")))
            }
            Reply::Delivered => Remote::Answered(Presence::Absent),
            reply => Remote::Uncertain(reply.reason("GetUnit")),
        }
    }

    fn runtime_max_usec(&self, unit_path: &str) -> Remote<Option<u64>> {
        let mut state = self.0.lock();
        state.calls.push(Call::RuntimeMax(unit_path.to_string()));
        if state.panics(Op::RuntimeMax) {
            panic_in(state, Op::RuntimeMax);
        }
        let expected = backstop(&state, unit_path);
        match state.runtime_max {
            Property::Uncertain(reply) => {
                Remote::Uncertain(state.connection(reply).reason("Get RuntimeMaxUSec"))
            }
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            Property::Exact => Remote::Answered(Some(expected)),
            Property::Wrong => Remote::Answered(Some(expected + 1)),
            Property::WrongType => Remote::Answered(None),
        }
    }

    fn oom_policy(&self, unit_path: &str) -> Remote<Option<String>> {
        let mut state = self.0.lock();
        state.calls.push(Call::OomPolicy(unit_path.to_string()));
        match state.oom_policy {
            Property::Uncertain(reply) => {
                Remote::Uncertain(state.connection(reply).reason("Get OOMPolicy"))
            }
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            Property::Exact => Remote::Answered(Some("continue".to_string())),
            Property::Wrong => Remote::Answered(Some("stop".to_string())),
            Property::WrongType => Remote::Answered(None),
        }
    }

    fn unit_id(&self, unit_path: &str) -> Remote<Option<String>> {
        let mut state = self.0.lock();
        state.calls.push(Call::UnitId(unit_path.to_string()));
        if state.panics(Op::UnitId) {
            panic_in(state, Op::UnitId);
        }
        let unit = unit_path.strip_prefix("/unit/").unwrap_or_default();
        match state.unit_id {
            Text::Uncertain(reply) => Remote::Uncertain(state.connection(reply).reason("Get Id")),
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            Text::Exact => Remote::Answered(Some(unit.to_string())),
            Text::Is(text) => Remote::Answered(Some(expand(text, unit))),
            Text::WrongType => Remote::Answered(None),
        }
    }

    fn control_group(&self, unit_path: &str) -> Remote<Option<String>> {
        let mut state = self.0.lock();
        state.calls.push(Call::ControlGroup(unit_path.to_string()));
        if state.panics(Op::ControlGroup) {
            panic_in(state, Op::ControlGroup);
        }
        let unit = unit_path.strip_prefix("/unit/").unwrap_or_default();
        match state.control_group {
            Text::Uncertain(reply) => {
                Remote::Uncertain(state.connection(reply).reason("Get ControlGroup"))
            }
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            // The cgroup the manager created for the unit; none: empty.
            Text::Exact => Remote::Answered(Some(
                state
                    .units
                    .get(unit)
                    .and_then(|unit| unit.cgroup)
                    .map(|index| state.cgroups[index].path.clone())
                    .unwrap_or_default(),
            )),
            Text::Is(text) => Remote::Answered(Some(expand(text, unit))),
            Text::WrongType => Remote::Answered(None),
        }
    }
}

/// The runtime backstop the model's unit at `unit_path` was created with.
fn backstop(state: &State, unit_path: &str) -> u64 {
    unit_path
        .strip_prefix("/unit/")
        .and_then(|unit| state.units.get(unit))
        .and_then(|unit| unit.cgroup)
        .map_or(0, |index| {
            state.cgroups[index].limits.runtime_backstop_secs * 1_000_000
        })
}

/// Whether `pid` is still this process's unreaped child (running or a
/// zombie). Diagnostics only: it neither reaps nor signals.
pub(crate) fn unreaped(pid: u32) -> bool {
    // SAFETY: siginfo_t is plain data; waitid fills it.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: WNOWAIT leaves the child as it is.
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    rc == 0
}

struct FakeNative(World);

impl Native for FakeNative {
    fn membership(&self, helper: &Helper) -> io::Result<Option<String>> {
        let pid = helper.pid();
        let mut state = self.0.lock();
        state.calls.push(Call::Membership(pid, unreaped(pid)));
        if state.panics(Op::Membership) {
            panic_in(state, Op::Membership);
        }
        if let Some((placing, index, reads)) = state.placing {
            if placing == pid {
                if reads == 0 {
                    state.placing = None;
                    state.place(pid, index);
                } else {
                    state.placing = Some((placing, index, reads - 1));
                }
            }
        }
        Ok(Some(
            state
                .membership
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| HOME.to_string()),
        ))
    }

    fn open(&self, path: &str) -> Result<Box<dyn CgroupDir>, ScopeError> {
        let mut state = self.0.lock();
        state.calls.push(Call::Open(path.to_string()));
        if state.panics(Op::Open) {
            panic_in(state, Op::Open);
        }
        if let Some(left) = state.opens_left {
            if left == 0 {
                return Err(ScopeError::Io(io::Error::from_raw_os_error(libc::EACCES)));
            }
            state.opens_left = Some(left - 1);
        }
        let Some(index) = state
            .cgroups
            .iter()
            .rposition(|cgroup| cgroup.path == path && !cgroup.removed)
        else {
            return Err(ScopeError::Io(io::Error::from_raw_os_error(libc::ENOENT)));
        };
        if !state.cgroups[index].v2 {
            return Err(ScopeError::Mismatch("not a cgroup v2 directory"));
        }
        Ok(Box::new(FakeDir {
            world: self.0.clone(),
            index,
        }))
    }
}

/// A model cgroup, retained "by descriptor": it stays this cgroup even once
/// removed.
struct FakeDir {
    world: World,
    index: usize,
}

impl std::fmt::Debug for FakeDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeDir")
            .field("index", &self.index)
            .finish()
    }
}

impl CgroupDir for FakeDir {
    fn read(&self, name: &str) -> io::Result<String> {
        let state = self.world.lock();
        let cgroup = &state.cgroups[self.index];
        if cgroup.removed {
            return Err(io::Error::from_raw_os_error(libc::ENOENT));
        }
        match name {
            "cgroup.events" => Ok(format!(
                "populated {}\nfrozen 0\n",
                u8::from(cgroup.populated())
            )),
            "cgroup.procs" if cgroup.listed && !cgroup.killed => Ok(cgroup
                .members
                .iter()
                .map(|pid| format!("{pid}\n"))
                .collect()),
            "cgroup.procs" => Ok(String::new()),
            "memory.events" => Ok("low 0\nhigh 0\nmax 0\noom 0\noom_kill 0\n".to_string()),
            "pids.events" => Ok("max 0\n".to_string()),
            file => expected_limit_files(&cgroup.limits)
                .into_iter()
                .find(|(limit, _)| *limit == file)
                .map(|(limit, value)| {
                    if cgroup.limit_mismatch == Some(limit) {
                        "max\n".to_string()
                    } else {
                        format!("{value}\n")
                    }
                })
                .ok_or_else(|| io::Error::from_raw_os_error(libc::ENOENT)),
        }
    }

    fn kill(&self) -> io::Result<()> {
        let mut state = self.world.lock();
        let path = state.cgroups[self.index].path.clone();
        state.calls.push(Call::Kill(path));
        let cgroup = &mut state.cgroups[self.index];
        if cgroup.removed {
            return Err(io::Error::from_raw_os_error(libc::ENOENT));
        }
        cgroup.kills += 1;
        cgroup.killed = true;
        Ok(())
    }

    fn listing_is_empty(&self) -> io::Result<bool> {
        Ok(self.world.lock().cgroups[self.index].removed)
    }
}

// ---------------------------------------------------------------------------
// Scope-level tests: the live harness's direct owner (`execution::place`,
// which owns the helper and its scope operation together) and the
// crate-private boundary it shares with executions (P2-V1-R3B-I4-R1: a
// normal build has no public direct scope start).

use super::{Occupancy, ScopeBoundary};
use crate::execution::{place, Cleanup, PlacementFailed, RetainedBoundary};

const LIMITS: ResourcePolicy = ResourcePolicy::RUST_OFFLINE_V1;

/// A live stand-in helper (`cat` holds its control socket open).
fn helper() -> Helper {
    Helper::spawn(&HelperProgram::at("/bin/cat")).unwrap().0
}

fn finish(mut helper: Helper) {
    let _ = helper.kill();
    helper.reap().unwrap();
}

/// A placement over `world` that must fail.
fn refused(world: &World) -> (PlacementFailed, u32) {
    let helper = helper();
    let pid = helper.pid();
    match place(&world.scopes(), helper, &LIMITS) {
        Ok(placed) => panic!("a scope was proven: {placed:?}"),
        Err(failed) => (failed, pid),
    }
}

/// The retained boundary of a cleanup that must be unconfirmed.
fn retained(cleanup: Cleanup) -> RetainedBoundary {
    match cleanup {
        Cleanup::Failed(boundary) => boundary,
        Cleanup::Confirmed => panic!("cleanup reported confirmed without proof"),
    }
}

/// Give up a boundary that can never be confirmed: its drop (defense only)
/// kills the helper and must leave it unreaped; this test then reaps it, as
/// this process's own child.
pub(crate) fn abandon(boundary: RetainedBoundary, pid: u32) {
    drop(boundary);
    assert!(unreaped(pid), "a dropped boundary reaped the helper");
    // SAFETY: an unreaped child of this process; nothing else reaps it.
    let reaped = unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) };
    assert_eq!(reaped, pid as libc::pid_t);
}

#[test]
fn i4_01_a_confirmed_start_with_every_proof_is_proven() {
    let world = World::default();
    let placed = place(&world.scopes(), helper(), &LIMITS).unwrap();
    let pid = placed.helper().unwrap().pid();
    let scope = placed.scope().expect("the scope is proven");
    let state = world.lock();
    let unit = state.requested().unwrap();
    assert_eq!(scope.unit(), unit);
    assert!(unit.starts_with("nexus-verifier-") && unit.ends_with(".scope"));
    // Proven through the kernel's view of the helper, the retained cgroup,
    // the manager's unit bound to it and the manager's properties, and
    // nothing was stopped.
    assert_eq!(
        state.calls,
        [
            Call::Start(unit.clone(), pid),
            Call::Membership(pid, true),
            Call::Open(format!("{SLICE}/{unit}")),
            Call::GetUnit(unit.clone()),
            Call::UnitId(format!("/unit/{unit}")),
            Call::ControlGroup(format!("/unit/{unit}")),
            Call::RuntimeMax(format!("/unit/{unit}")),
            Call::OomPolicy(format!("/unit/{unit}")),
        ]
    );
    drop(state);
    assert_eq!(scope.occupancy().unwrap(), Occupancy::Populated);
    assert!(placed.end().is_confirmed());
    assert!(!unreaped(pid));
}

#[test]
fn i4_02_a_start_without_effect_whose_reply_is_lost_is_never_proven_absent() {
    // I4-R1: the manager would answer that no unit of the name is loaded,
    // and the helper is outside it; neither confirms an uncertain request.
    for reply in [Reply::Timeout, Reply::Error, Reply::Malformed] {
        let world = World::new(|state| {
            state.start_effect = false;
            state.start_reply = reply;
        });
        let (failed, pid) = refused(&world);
        assert!(
            matches!(failed.error, Some(ScopeError::Bus(_))),
            "{reply:?}"
        );
        let boundary = retained(failed.cleanup);
        assert!(
            boundary.holds_scope() && boundary.holds_helper() && unreaped(pid),
            "{reply:?}"
        );
        let state = world.lock();
        assert!(state.units.is_empty(), "{reply:?}: nothing was created");
        // The manager's absence is never asked for as evidence; nothing was
        // opened.
        assert_eq!(state.count(|call| matches!(call, Call::GetUnit(_))), 0);
        assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 0);
        drop(state);
        abandon(boundary, pid);
    }
}

#[test]
fn i4_03_a_start_with_effect_whose_reply_is_lost_is_discovered_and_proven() {
    for (reply, placement) in [
        (Reply::Timeout, Placement::Immediate),
        (Reply::Malformed, Placement::AfterReads(3)),
        (Reply::Error, Placement::Immediate),
    ] {
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = placement;
        });
        let placed = place(&world.scopes(), helper(), &LIMITS).unwrap_or_else(|failed| {
            panic!("{reply:?}: the unit a lost-reply start created was not discovered and proven: {failed:?}")
        });
        assert_eq!(
            Some(placed.scope().unwrap().unit().to_string()),
            world.lock().requested()
        );
        assert_eq!(
            world.lock().count(|call| matches!(call, Call::Stop(_))),
            0,
            "{reply:?}"
        );
        assert!(placed.end().is_confirmed());
    }
    // The connection broke with the request: the scope cannot be proven,
    // and what was created is settled through its retained cgroup.
    let world = World::new(|state| state.start_reply = Reply::Disconnect);
    let (failed, pid) = refused(&world);
    assert!(matches!(failed.error, Some(ScopeError::Bus(_))));
    assert!(failed.cleanup.is_confirmed(), "{failed:?}");
    assert!(world.lock().created().unwrap().killed);
    assert!(!unreaped(pid));
}

#[test]
fn i4_03_a_start_whose_effect_cannot_be_confirmed_gone_stays_owned_with_its_helper() {
    // Created, reply lost, the helper never placed, the connection gone:
    // nothing can establish that the unit is gone, so the failure carries
    // the operation and its helper, owned together.
    let world = World::new(|state| {
        state.start_reply = Reply::Disconnect;
        state.placement = Placement::Never;
    });
    let (failed, pid) = refused(&world);
    let boundary = retained(failed.cleanup);
    let observed = boundary
        .pending()
        .expect("the unresolved operation is retained");
    assert!(observed.issued && !observed.accepted && !observed.settled && !observed.candidate);
    assert!(boundary.holds_helper() && unreaped(pid));
    let boundary = boundary.retry().unwrap_err();
    // I4-R1: once the manager answers again (and acts on StopUnit), nothing
    // without a candidate confirms it either.
    world.release();
    let boundary = boundary.retry().unwrap_err();
    assert!(boundary.holds_scope() && boundary.holds_helper() && unreaped(pid));
    abandon(boundary, pid);
}

#[test]
fn i4_a_failed_placement_settles_with_its_helper_and_reports_the_proof_failure() {
    // A proof that fails on the retained cgroup: settling ends what is in
    // it, and the failure names the proof.
    let world = World::new(|state| state.limit_mismatch = Some("pids.max"));
    let (failed, pid) = refused(&world);
    assert!(matches!(
        failed.error,
        Some(ScopeError::Mismatch("pids.max"))
    ));
    assert!(failed.cleanup.is_confirmed());
    let state = world.lock();
    assert!(state.created().unwrap().killed);
    assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 1);
    drop(state);
    assert!(!unreaped(pid), "settled, then reaped");
}

#[test]
fn i4_a_pending_operation_is_bound_to_its_own_helper() {
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let (mut bound, other) = (helper(), helper());
    let scopes = world.scopes();
    let mut boundary = ScopeBoundary::Pending(scopes.prepare(&bound, &LIMITS).unwrap());
    assert!(matches!(
        boundary.establish(&bound, None),
        Err(ScopeError::NotPlaced)
    ));
    let ScopeBoundary::Pending(pending) = &mut boundary else {
        panic!("the unresolved operation is not held: {boundary:?}");
    };
    // As the finalizer settles: its bound helper killed, kept unreaped.
    let _ = bound.kill();
    // Another helper's membership is no evidence for this operation.
    world.release();
    assert!(
        !pending.reconcile(Some(&other), None),
        "another helper's membership settled the operation"
    );
    assert_eq!(
        world
            .lock()
            .count(|call| matches!(call, Call::Membership(pid, _) if *pid == other.pid())),
        0,
        "another helper's membership settled the operation"
    );
    assert!(pending.reconcile(Some(&bound), None));
    // A prepared operation is never issued for another helper.
    let mut boundary = ScopeBoundary::Pending(scopes.prepare(&bound, &LIMITS).unwrap());
    assert!(matches!(
        boundary.establish(&other, None),
        Err(ScopeError::Mismatch("scope operation"))
    ));
    assert!(matches!(&boundary, ScopeBoundary::Pending(pending) if !pending.state().issued));
    finish(bound);
    finish(other);
}

#[test]
fn i4_owners_are_self_contained_values() {
    // Movable to any thread and borrowing nothing: an unresolved operation,
    // a retained boundary and the harness's owners outlive whatever created
    // them.
    fn owned<T: Send + 'static>() {}
    owned::<super::PendingScope>();
    owned::<super::Scope>();
    owned::<ScopeManager>();
    owned::<RetainedBoundary>();
    owned::<crate::execution::ScopedHelper>();
    owned::<PlacementFailed>();
}

/// The names of the items `source` declares public in a build without the
/// test or harness configuration: `pub` items (never `pub(crate)`), outside
/// any item or block gated by `cfg(test)` or the `live-sandbox-harness`
/// feature. A tripwire over rustfmt-formatted source; the authoritative
/// check of the normal build's surface compiles an external caller.
fn normal_public(source: &str) -> Vec<String> {
    const ITEMS: [&str; 9] = [
        "fn ", "struct ", "enum ", "trait ", "use ", "mod ", "const ", "static ", "type ",
    ];
    let mut names = Vec::new();
    let mut gated = false;
    let mut skip_until: Option<String> = None;
    for line in source.lines() {
        if let Some(end) = &skip_until {
            if line == end {
                skip_until = None;
            }
            continue;
        }
        let item = line.trim_start();
        let indent = &line[..line.len() - item.len()];
        if item.starts_with("#[") {
            gated |= item.contains("cfg(test)")
                || item.contains("cfg(any(test, feature = \"live-sandbox-harness\"))");
            continue;
        }
        if item.starts_with("//") || item.is_empty() {
            continue;
        }
        if gated {
            gated = false;
            // A gated item with a body: everything up to its closing brace.
            if item.ends_with('{') {
                skip_until = Some(format!("{indent}}}"));
            }
            continue;
        }
        if let Some(rest) = item.strip_prefix("pub ") {
            if let Some(kind) = ITEMS.iter().find(|kind| rest.starts_with(**kind)) {
                let declared = &rest[kind.len()..];
                let name: String = if *kind == "use " {
                    declared.trim_end_matches(';').to_string()
                } else {
                    declared
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect()
                };
                names.push(format!("{kind}{name}"));
            }
        }
    }
    names
}

#[test]
fn i4r1_04_a_normal_build_constructs_a_manager_only_by_connect() {
    // P2-V1-R3B-I4-R1: no caller-selected bus, no other manager.
    let constructors: Vec<_> = normal_public(include_str!("../scope.rs"))
        .into_iter()
        .filter(|item| item.starts_with("fn connect") || item.starts_with("fn with_"))
        .collect();
    assert_eq!(
        constructors,
        ["fn connect"],
        "a normal build exposes a caller-selected manager socket"
    );
    assert_eq!(
        normal_public(include_str!("manager.rs")),
        ["const BUS_CALL_TIMEOUT"]
    );
    assert!(normal_public(include_str!("native.rs")).is_empty());
}

#[test]
fn i4r1_05_a_normal_build_holds_no_scope_operation_apart_from_its_helper() {
    // No public pending operation, no public settling, and no failure that
    // carries a pending operation without its helper.
    assert!(
        normal_public(include_str!("pending.rs")).is_empty(),
        "a normal build exposes a pending scope operation"
    );
    for source in [
        include_str!("../scope.rs"),
        include_str!("pending.rs"),
        include_str!("../execution.rs"),
    ] {
        assert!(
            !source.contains(concat!("Start", "Failed")),
            "a normal build exposes a pending scope operation"
        );
    }
}

#[test]
fn i4r1_x_a_normal_build_starts_a_scope_only_within_run() {
    // The whole normal-build surface of the scope and execution modules:
    // `ScopeManager::connect`, then `execution::run`.
    assert_eq!(
        normal_public(include_str!("../scope.rs")),
        [
            "use manager::BUS_CALL_TIMEOUT",
            "const PLACEMENT_TIMEOUT",
            "const SETTLE_TIMEOUT",
            "enum ScopeError",
            "fn expected_limit_files",
            "struct ScopeManager",
            "fn connect",
            "struct ScopeEvents",
            "enum Occupancy",
            "struct Scope",
            "fn unit",
            "fn kill",
            "fn occupancy",
            "fn wait_empty",
            "fn events",
        ],
        "a normal build exposes a direct scope start"
    );
    assert_eq!(
        normal_public(include_str!("../execution.rs")),
        [
            "const FINALIZE_TIMEOUT",
            "struct StreamRecord",
            "enum EndedBy",
            "enum NotRun",
            "enum Cleanup",
            "fn is_confirmed",
            "struct RetainedBoundary",
            "fn retry",
            "struct ExecutionReport",
            "enum ExitClass",
            "fn classify",
            "fn run",
        ],
        "a normal build exposes a direct scope start"
    );
}

#[test]
fn i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names() {
    // P2-V1-R3B-I4-Q1: the only two error names read as definite answers are
    // systemd's own (bus-common-errors.h, v255), each in exactly one arm;
    // every other error is uncertain. The live host qualification asserts
    // the same literals against the real user manager (`p2q` cases).
    let code: String = include_str!("manager.rs")
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "const NO_SUCH_UNIT: &str = \"org.freedesktop.systemd1.NoSuchUnit\";",
        "const UNIT_EXISTS: &str = \"org.freedesktop.systemd1.UnitExists\";",
        "Ok(Err(name)) if name == UNIT_EXISTS => Started::Collision,",
        "Ok(Err(name)) if name == NO_SUCH_UNIT => Remote::Answered(Presence::Absent),",
        "Ok(Err(name)) => Started::Uncertain(format!(\"StartTransientUnit: {name}\")),",
        "Ok(Err(name)) => Remote::Uncertain(format!(\"GetUnit: {name}\")),",
    ] {
        assert_eq!(code.matches(needle).count(), 1, "{needle}");
    }
    // No other path answers a collision or an absence.
    assert_eq!(code.matches("=> Started::Collision").count(), 1);
    assert_eq!(code.matches("Presence::Absent").count(), 1);
    assert_eq!(code.matches("NoSuchUnit").count(), 1);
    assert_eq!(code.matches("UnitExists").count(), 1);
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-Q1-R1: the collision qualification's first start. The live
// harness's H6 makes it only through `execution::place`, never by a request
// of its own, and holds nothing by a unit name; these are that start's
// outcomes over the model, with what each may and may not touch (the
// controls Q1-R1-NC1 to NC4 mutate them).

/// Every membership read of `pid` was made while it was still this
/// process's unreaped child.
fn read_unreaped(world: &World, pid: u32) -> bool {
    world
        .lock()
        .calls
        .iter()
        .all(|call| !matches!(call, Call::Membership(read, false) if *read == pid))
}

#[test]
fn i4q1r1_nc1_a_first_start_refused_as_loaded_ends_only_the_harness_s_helper() {
    // The first start is answered exactly `UnitExists` (the model's
    // collision): the unit of that name is foreign. It is never stopped,
    // killed or claimed (nothing is opened, read or proven for it), and the
    // only thing ended is the helper the harness owns: at once when it is
    // outside that unit, and only once it is outside when the foreign unit's
    // cgroup is reported holding it.
    for holds in [false, true] {
        let world = World::new(|state| {
            state.collide = true;
            state.collision_holds_helper = holds;
        });
        let (failed, pid) = refused(&world);
        assert!(
            matches!(&failed.error, Some(ScopeError::Bus(reason)) if reason.contains("the unit exists")),
            "{holds}: {:?}",
            failed.error
        );
        if holds {
            let boundary = retained(failed.cleanup);
            assert!(boundary.holds_helper() && unreaped(pid), "{holds}");
            let boundary = boundary.retry().unwrap_err();
            world.lock().membership.remove(&pid);
            boundary.retry().unwrap();
        } else {
            assert!(failed.cleanup.is_confirmed(), "{holds}");
        }
        let state = world.lock();
        let unit = state.requested().unwrap();
        assert_eq!(
            state.count(|call| matches!(call, Call::Stop(_))),
            0,
            "{holds}: the foreign unit was stopped"
        );
        assert!(
            state.units.get(&unit).is_some_and(|unit| !unit.ours),
            "{holds}: the foreign unit was stopped"
        );
        assert!(
            state.count(|call| matches!(call, Call::Kill(_))) == 0
                && state
                    .cgroups
                    .iter()
                    .all(|cgroup| cgroup.kills == 0 && !cgroup.killed),
            "{holds}: the foreign unit was killed"
        );
        assert_eq!(
            state.count(|call| matches!(
                call,
                Call::Open(_)
                    | Call::GetUnit(_)
                    | Call::UnitId(_)
                    | Call::ControlGroup(_)
                    | Call::RuntimeMax(_)
                    | Call::OomPolicy(_)
            )),
            0,
            "{holds}: the foreign unit was claimed"
        );
        drop(state);
        assert!(read_unreaped(&world, pid), "{holds}");
        assert!(
            !unreaped(pid),
            "{holds}: the harness's own helper was not ended"
        );
    }
}

#[test]
fn i4q1r1_nc2_an_uncertain_first_start_keeps_its_helper_unreaped_and_unconfirmed() {
    // No delivered reply (a timeout, a broken connection, an unexpected
    // error, a reply that does not decode), with and without an effect, the
    // helper never placed: nothing confirms the operation. Its helper is
    // killed but stays this process's unreaped child, owned with the
    // operation; neither a delivered and effective StopUnit nor the
    // manager's absence confirms it.
    for effect in [false, true] {
        for reply in [
            Reply::Timeout,
            Reply::Disconnect,
            Reply::Error,
            Reply::Malformed,
        ] {
            let case = format!("{reply:?}, effect {effect}");
            let world = World::new(|state| {
                state.start_effect = effect;
                state.start_reply = reply;
                state.placement = Placement::Never;
            });
            let (failed, pid) = refused(&world);
            assert!(
                matches!(failed.error, Some(ScopeError::Bus(_))),
                "{case}: {:?}",
                failed.error
            );
            let Cleanup::Failed(boundary) = failed.cleanup else {
                panic!("{case}: an uncertain first start was confirmed without proof");
            };
            let observed = boundary
                .pending()
                .expect("the unresolved operation is retained");
            assert!(
                observed.issued
                    && !observed.accepted
                    && !observed.collided
                    && !observed.candidate
                    && !observed.settled,
                "{case}: {observed:?}"
            );
            assert!(
                boundary.holds_helper() && unreaped(pid),
                "{case}: the helper was reaped while unresolved"
            );
            // The manager answers again: StopUnit acts and its reply is
            // delivered, GetUnit would answer NoSuchUnit, the helper is
            // outside. Still nothing confirms the operation.
            world.release();
            let Err(boundary) = boundary.retry() else {
                panic!("{case}: an uncertain first start was confirmed without proof");
            };
            assert!(
                boundary.holds_scope() && boundary.holds_helper() && unreaped(pid),
                "{case}: the helper was reaped while unresolved"
            );
            assert!(
                world.lock().count(|call| matches!(call, Call::Stop(_))) >= 1,
                "{case}"
            );
            assert!(
                read_unreaped(&world, pid),
                "{case}: the helper was reaped while unresolved"
            );
            abandon(boundary, pid);
        }
    }
}

#[test]
fn i4q1r1_nc3_a_panic_after_the_first_start_is_dispatched_keeps_one_owner() {
    // A panic inside StartTransientUnit after its effect and before any
    // reply, or while its outcome is observed: it never crosses
    // `execution::place`, the operation is never split from its helper, and
    // the helper is never reaped while the operation is unresolved (its
    // process id stays reserved: no reused id can be attached).
    let worlds: [(Op, Placement); 4] = [
        (Op::Start, Placement::Never),
        (Op::Start, Placement::AfterReads(2)),
        (Op::Membership, Placement::Never),
        (Op::Open, Placement::Immediate),
    ];
    for (op, placement) in worlds {
        let case = format!("{op:?}, {placement:?}");
        let world = World::new(|state| {
            state.placement = placement;
            state.panic_at = Some(op);
        });
        let helper = helper();
        let pid = helper.pid();
        let Ok(placed) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            place(&world.scopes(), helper, &LIMITS)
        })) else {
            panic!("{case}: a panic crossed the owner of the first start");
        };
        let failed = match placed {
            Ok(placed) => panic!("{case}: a panicked start was proven: {placed:?}"),
            Err(failed) => failed,
        };
        assert!(failed.error.is_none(), "{case}: {:?}", failed.error);
        match failed.cleanup {
            Cleanup::Failed(boundary) => {
                assert!(
                    boundary.holds_helper() && unreaped(pid),
                    "{case}: the operation was split from its helper"
                );
                if let Some(observed) = boundary.pending() {
                    assert!(observed.issued && !observed.settled, "{case}: {observed:?}");
                }
                world.release();
                match boundary.retry() {
                    Ok(()) => assert!(!unreaped(pid), "{case}"),
                    Err(boundary) => abandon(boundary, pid),
                }
            }
            Cleanup::Confirmed => {
                // Only with nothing left: no cgroup of the model holds a
                // process, and the helper is reaped.
                assert!(
                    world
                        .lock()
                        .cgroups
                        .iter()
                        .all(|cgroup| !cgroup.populated()),
                    "{case}: confirmed with a populated cgroup"
                );
                assert!(!unreaped(pid), "{case}");
            }
        }
        assert!(
            read_unreaped(&world, pid),
            "{case}: the helper was reaped while unresolved"
        );
    }
}

#[test]
fn i4q1r1_nc4_a_proven_first_start_is_released_by_its_owner_and_observed() {
    // The accepted path: the scope H6 holds while its request without
    // processes is refused is the production owner's proven scope. It is
    // released as the live harness's `released` does: `cgroup.kill` of
    // exactly the created unit's cgroup through the retained descriptor, the
    // helper reaped only then (the scope is proven), the scope observed
    // empty, the owner's end confirmed; nothing is stopped by a name and no
    // other cgroup is touched.
    let world = World::default();
    let stand_in = helper();
    let pid = stand_in.pid();
    let mut placed = place(&world.scopes(), stand_in, &LIMITS).unwrap();
    let unit = world.lock().requested().unwrap();
    let created = format!("{SLICE}/{unit}");
    let proven = world.lock().calls.len();
    let scope = placed.scope().expect("the scope is proven");
    assert_eq!(scope.unit(), unit);
    assert_eq!(scope.occupancy().unwrap(), Occupancy::Populated);
    scope.kill().unwrap();
    // The model's `cgroup.kill` ends no real process: as the kernel would.
    // SAFETY: the stand-in is this process's unreaped child.
    assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
    assert!(unreaped(pid));
    placed.reap_helper().unwrap();
    assert!(!unreaped(pid));
    assert!(placed
        .scope()
        .unwrap()
        .wait_empty(Duration::from_secs(1))
        .unwrap());
    assert!(
        placed.end().is_confirmed(),
        "the released scope was not confirmed"
    );
    let state = world.lock();
    assert!(
        state.calls[proven..]
            .iter()
            .all(|call| *call == Call::Kill(created.clone())),
        "the release touched more than the created unit's cgroup: {:?}",
        &state.calls[proven..]
    );
    assert_eq!(
        state.count(|call| matches!(call, Call::Stop(_))),
        0,
        "the created unit was stopped by a name"
    );
    assert!(state.cgroups.iter().all(|cgroup| !cgroup.populated()));
    drop(state);
    // A scope that stays populated is never confirmed: the owner's end
    // confirms only what it observed empty, and keeps the rest.
    let world = World::new(|state| state.phantom = Phantom::Forever);
    let placed = place(&world.scopes(), helper(), &LIMITS).unwrap();
    let Cleanup::Failed(boundary) = placed.end() else {
        panic!("the owner confirmed a scope that was never observed empty");
    };
    assert!(boundary.holds_scope());
    world.release();
    boundary.retry().unwrap();
}

#[test]
fn i4q1r1_connect_derives_the_bus_from_the_real_uid_alone() {
    // P2-V1-R3B-I4-Q1-R1 (H1): the one manager a normal build constructs is
    // the real uid's user bus, its path derived from `getuid` alone and
    // connected by that explicit address (zbus 4.4.0's `Builder::address`
    // reads no environment; only its session and system builders do). The
    // scope module reads no environment and names no ambient bus. The live
    // H1 case relies on this pin instead of changing the environment of the
    // multi-threaded live harness.
    let code = |source: &str| -> String {
        source
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let sources = [
        ("scope.rs", code(include_str!("../scope.rs"))),
        ("scope/manager.rs", code(include_str!("manager.rs"))),
        ("scope/pending.rs", code(include_str!("pending.rs"))),
        ("scope/native.rs", code(include_str!("native.rs"))),
    ];
    for (file, source) in &sources {
        for needle in [
            "std::env",
            "env::var",
            "var_os",
            "getenv",
            "set_var",
            "Builder::session",
            "Builder::system",
            "Connection::session",
            "Connection::system",
            "Address::session",
            "Address::system",
            "DBUS_",
            "XDG_RUNTIME_DIR",
        ] {
            assert!(
                !source.contains(needle),
                "the scope module reads the environment or names an ambient bus: {needle} in {file}"
            );
        }
    }
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let scope = &sources[0].1;
    let connect = scope
        .split("pub fn connect() -> Result<Self, ScopeError> {")
        .nth(1)
        .and_then(|rest| rest.split("\n    }\n").next())
        .expect("ScopeManager's connect()");
    assert_eq!(
        words(connect),
        concat!(
            "let uid = unsafe { libc::getuid() }; ",
            "Self::connect_to(&format!(\"/run/",
            "user/{uid}/bus\"))"
        ),
        "connect() derives its bus from more than the real uid"
    );
    // The one path to a connection: connect() and the harness's checks of
    // its checks (`connect_at`), both through the same owner-checked socket.
    assert_eq!(scope.matches("Self::connect_to(").count(), 2);
    assert_eq!(
        scope
            .matches(concat!("Box::new(Zbus", "Manager::connect_at(path)?)"))
            .count(),
        1
    );
    let manager = &sources[1].1;
    for needle in [
        "let address = format!(\"unix:path={path}\");",
        "zbus::connection::Builder::address(address.as_str())?",
    ] {
        assert_eq!(manager.matches(needle).count(), 1, "{needle}");
    }
    assert_eq!(manager.matches("connection::Builder::").count(), 1);
}
