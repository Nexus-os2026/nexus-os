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
//!
//! The manager has nothing to stop or kill a unit with: the backend never
//! acts on a unit by its name (P2-V1-R3B-I4-R3). A unit goes only when the
//! manager unloads it by itself, and a test may then load a foreign unit
//! under the same name ([`State::unload`], [`State::load_foreign`]).
//!
//! Every unit invocation has its own random-like identity and every cgroup
//! directory its own kernel ID, never reused (P2-V1-R3B-I4-R3-R1). An
//! accepted start's identity is captured by the production rule
//! ([`captured_instance`]) from the signals systemd would send, or from
//! scripted others ([`Capture`]); the manager answers a unit's
//! `InvocationID` and `ControlGroupId` from the truth it keeps or a
//! scripted other answer, and a test may replace a unit's directory under
//! the manager's record ([`State::swap_directory`]).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::manager::{
    captured_instance, Invocation, Manager, Presence, Remote, ScopeRequest, Started, UnitChange,
    UnitInstance,
};
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

/// How the start's own signals reach the backend, from which the
/// production rule ([`captured_instance`]) captures the identity of the
/// invocation the start began (P2-V1-R3B-I4-R3-R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Capture {
    /// What systemd sends: the pristine unit's null identity, then its new
    /// identity, between the reply and the start job's removal (`done`).
    Exact,
    /// The start job's removal never arrives within the bound.
    Timeout,
    /// The start job ends `failed`.
    JobFailed,
    /// The change carries a malformed identity (another length or type).
    Malformed,
    /// The change carries only the null identity.
    Null,
    /// A second, different identity also appears inside the window.
    Two,
    /// The identity is sent only after the start job's removal.
    Late,
    /// The removal's serial is not above the reply's (the sender's serials
    /// wrapped).
    Wrapped,
}

/// What another client of the manager does once an accepted start's
/// identity is captured, before the proof (P2-V1-R3B-I4-R3-R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interference {
    None,
    /// The manager unloads this request's unit, and the client loads a
    /// unit of its own under the same name, its cgroup at the same path
    /// holding this process of its own and the helper (moved there).
    Replace(u32),
    /// The client removes the unit's directory and creates another at the
    /// same path, holding this process of its own and the helper; the
    /// manager still records the unit's own directory.
    Swap(u32),
}

/// A process in a unit's cgroup beyond the placed helper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phantom {
    None,
    /// Ended by `cgroup.kill`.
    UntilKill,
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
    /// After the start's effect, before any reply.
    Start,
    /// After the start's success reply, while its identity is captured.
    Capture,
    GetUnit,
    UnitInstance,
    UnitId,
    ControlGroup,
    ControlGroupId,
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
    GetUnit(String),
    /// The unit's `Id`, at the object path GetUnit returned.
    UnitId(String),
    /// The unit's `ControlGroup`, at the object path GetUnit returned.
    ControlGroup(String),
    /// The unit's `InvocationID`, at the object path GetUnit returned.
    UnitInstance(String),
    /// The unit's `ControlGroupId`, at the object path GetUnit returned.
    ControlGroupId(String),
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
    /// The manager's record of its cgroup: the directory it realized.
    pub cgroup: Option<usize>,
    /// This invocation's identity.
    pub instance: UnitInstance,
}

#[derive(Debug, Clone)]
pub(crate) struct Cgroup {
    pub path: String,
    /// The kernel's ID of this directory: never reused.
    pub id: u64,
    pub v2: bool,
    pub removed: bool,
    pub members: BTreeSet<u32>,
    /// `cgroup.procs` lists the members.
    pub listed: bool,
    /// `cgroup.kill` ended every member.
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
            Phantom::Forever => true,
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
    /// How the refusal of a colliding request (exactly `UnitExists`, no
    /// effect) reaches the backend: delivered, or lost like any other reply
    /// (P2-V1-R3B-I4-R2).
    pub collision_reply: Reply,
    /// The foreign unit's cgroup holds this process of its own (a model
    /// process id: the model never signals a real process).
    pub foreign_member: Option<u32>,
    pub placement: Placement,
    /// How the accepted start's own signals reach the backend.
    pub capture: Capture,
    /// The start's signals cannot be watched: the request is never sent.
    pub watch_fails: bool,
    /// What another client does once the identity is captured.
    pub interference: Interference,
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
    /// `InvocationID` reads, one answer per read (`Wrong`: another valid
    /// identity).
    pub unit_instance: Script<Property>,
    /// `ControlGroupId` reads (`Wrong`: another directory's ID).
    pub control_group_id: Property,
    pub runtime_max: Property,
    pub oom_policy: Property,
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
    /// The last identity or directory ID handed out.
    ids: u64,
}

impl Default for State {
    /// Every call delivered, every proof passing.
    fn default() -> Self {
        Self {
            start_effect: true,
            start_reply: Reply::Delivered,
            collide: false,
            collision_holds_helper: false,
            collision_reply: Reply::Delivered,
            foreign_member: None,
            placement: Placement::Immediate,
            capture: Capture::Exact,
            watch_fails: false,
            interference: Interference::None,
            unit_loaded: true,
            cgroup_v2: true,
            listed: true,
            limit_mismatch: None,
            phantom: Phantom::None,
            get_unit: Script::always(Reply::Delivered),
            unit_id: Text::Exact,
            control_group: Text::Exact,
            unit_instance: Script::always(Property::Exact),
            control_group_id: Property::Exact,
            runtime_max: Property::Exact,
            oom_policy: Property::Exact,
            opens_left: None,
            panic_at: None,
            broken: false,
            units: BTreeMap::new(),
            cgroups: Vec::new(),
            membership: HashMap::new(),
            calls: Vec::new(),
            placing: None,
            ids: 0x1000,
        }
    }
}

/// The model's identity number `n`: 16 bytes, never all zero.
pub(crate) fn model_instance(n: u64) -> UnitInstance {
    let mut bytes = [0x5a; 16];
    bytes[..8].copy_from_slice(&n.to_le_bytes());
    UnitInstance::from_property(&bytes).expect("a valid identity")
}

/// A valid identity that is not `instance`.
pub(crate) fn other_instance(instance: UnitInstance) -> UnitInstance {
    let mut bytes = instance.bytes();
    bytes[15] ^= 0xff;
    UnitInstance::from_property(&bytes).expect("a valid identity")
}

impl State {
    /// A fresh identity or directory ID: never handed out before.
    fn fresh(&mut self) -> u64 {
        self.ids += 1;
        self.ids
    }

    /// A new unit invocation's identity.
    pub(crate) fn fresh_instance(&mut self) -> UnitInstance {
        let n = self.fresh();
        model_instance(n)
    }

    /// Add `cgroup` as a new directory, with a kernel ID of its own; its
    /// index.
    pub(crate) fn add_cgroup(&mut self, mut cgroup: Cgroup) -> usize {
        cgroup.id = self.fresh();
        self.cgroups.push(cgroup);
        self.cgroups.len() - 1
    }

    /// Someone removes the directory the manager realized for `unit` and
    /// creates another at the same path, with the same limits, holding
    /// `members` (moved there; a model process id never signals a real
    /// process). The manager still records the unit's own directory: its
    /// path, which is the new one's too, and its ID. Returns the new
    /// directory's index.
    pub(crate) fn swap_directory(&mut self, unit: &str, members: &[u32]) -> usize {
        let index = self.units[unit].cgroup.expect("the unit's cgroup");
        let mut replacement = self.cgroups[index].clone();
        self.cgroups[index].removed = true;
        self.cgroups[index].members.clear();
        replacement.members.clear();
        replacement.killed = false;
        replacement.kills = 0;
        replacement.phantom = Phantom::None;
        let new = self.add_cgroup(replacement);
        for member in members {
            self.place(*member, new);
        }
        new
    }

    /// The kernel reports `pid` in the cgroup at `index`.
    pub(crate) fn place(&mut self, pid: u32, index: usize) {
        self.cgroups[index].members.insert(pid);
        self.membership
            .insert(pid, self.cgroups[index].path.clone());
    }

    /// The manager unloads `unit` by itself, as systemd collects a unit
    /// once nothing is left in it: its cgroup is removed, every member
    /// reported in that removed cgroup, and the name is free again. Never a
    /// request of the backend; the test arranges that nothing is left.
    pub(crate) fn unload(&mut self, unit: &str) {
        let Some(unloaded) = self.units.remove(unit) else {
            return;
        };
        if let Some(index) = unloaded.cgroup {
            let cgroup = &mut self.cgroups[index];
            cgroup.removed = true;
            let path = cgroup.path.clone();
            for member in cgroup.members.clone() {
                self.membership.insert(member, format!("{path} (deleted)"));
            }
        }
    }

    /// Another client of the manager loads a unit of its own under `unit`
    /// (the name free again), in a new cgroup of the same path and with the
    /// same limits as the backend's, holding `member`, a process of its own
    /// (a model process id: the model never signals a real process).
    /// Returns that cgroup's index.
    pub(crate) fn load_foreign(&mut self, unit: &str, member: u32) -> usize {
        assert!(!self.units.contains_key(unit), "the name is still loaded");
        let index = self.add_cgroup(Cgroup {
            path: format!("{SLICE}/{unit}"),
            id: 0,
            v2: true,
            removed: false,
            members: BTreeSet::new(),
            listed: true,
            killed: false,
            phantom: Phantom::None,
            limits: ResourcePolicy::RUST_OFFLINE_V1,
            limit_mismatch: None,
            kills: 0,
        });
        self.place(member, index);
        let instance = self.fresh_instance();
        self.units.insert(
            unit.to_string(),
            Unit {
                ours: false,
                cgroup: Some(index),
                instance,
            },
        );
        index
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

    /// Release every process the model kept alive and let every call be
    /// answered. Units stay loaded: only the manager unloads one
    /// ([`State::unload`]).
    pub(crate) fn release(&self) {
        let mut state = self.lock();
        for cgroup in &mut state.cgroups {
            cgroup.phantom = Phantom::None;
        }
        state.get_unit = Script::always(Reply::Delivered);
        state.broken = false;
        state.opens_left = None;
        state.panic_at = None;
    }
}

struct FakeManager(World);

/// The identity the backend captures for an accepted start that began
/// `instance`: the production rule over the signals the script sends.
fn captured(capture: Capture, instance: UnitInstance) -> Result<UnitInstance, String> {
    const REPLY: u32 = 100;
    let change = |serial, invocation| UnitChange { serial, invocation };
    let (removed, result, changes) = match capture {
        Capture::Exact => (
            110,
            "done",
            vec![
                change(101, Invocation::Null),
                change(105, Invocation::Id(instance)),
            ],
        ),
        Capture::Timeout => return Err("JobRemoved timed out".to_string()),
        Capture::JobFailed => (110, "failed", vec![change(105, Invocation::Id(instance))]),
        Capture::Malformed => (110, "done", vec![change(105, Invocation::Malformed)]),
        Capture::Null => (110, "done", vec![change(105, Invocation::Null)]),
        Capture::Two => (
            110,
            "done",
            vec![
                change(105, Invocation::Id(instance)),
                change(107, Invocation::Id(other_instance(instance))),
            ],
        ),
        Capture::Late => (110, "done", vec![change(111, Invocation::Id(instance))]),
        Capture::Wrapped => (90, "done", vec![change(105, Invocation::Id(instance))]),
    };
    captured_instance(REPLY, removed, result, &changes)
}

impl Manager for FakeManager {
    fn start_scope(&self, request: &ScopeRequest<'_>) -> Started {
        let mut state = self.0.lock();
        if state.watch_fails {
            // Its signals could not be watched: the request is never sent.
            return Started::NotIssued("watching the start timed out".to_string());
        }
        state
            .calls
            .push(Call::Start(request.unit.to_string(), request.helper_pid));
        if state.broken {
            return Started::Uncertain(Reply::Disconnect.reason("StartTransientUnit"));
        }
        if state.collide || state.units.contains_key(request.unit) {
            if !state.units.contains_key(request.unit) {
                let holds_helper = state.collision_holds_helper;
                let foreign = state.foreign_member;
                let cgroup = (holds_helper || foreign.is_some()).then(|| {
                    let index = state.add_cgroup(Cgroup {
                        path: format!("{SLICE}/{}", request.unit),
                        id: 0,
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
                    if holds_helper {
                        state.place(request.helper_pid, index);
                    }
                    if let Some(member) = foreign {
                        state.place(member, index);
                    }
                    index
                });
                let instance = state.fresh_instance();
                state.units.insert(
                    request.unit.to_string(),
                    Unit {
                        ours: false,
                        cgroup,
                        instance,
                    },
                );
            }
            // Refused before any effect; the refusal's reply can be lost like
            // any other, and then the backend sees an uncertain start.
            let reply = state.collision_reply;
            return match state.connection(reply) {
                Reply::Delivered => Started::Collision,
                reply => Started::Uncertain(reply.reason("StartTransientUnit")),
            };
        }
        assert!(
            state.start_effect || state.start_reply != Reply::Delivered,
            "a delivered start always created its unit"
        );
        let mut started = None;
        if state.start_effect {
            let path = match state.placement {
                Placement::Elsewhere(path) => expand(path, request.unit),
                _ => format!("{SLICE}/{}", request.unit),
            };
            let cgroup = Cgroup {
                path,
                id: 0,
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
            let index = state.add_cgroup(cgroup);
            let instance = state.fresh_instance();
            started = Some(instance);
            if state.unit_loaded {
                state.units.insert(
                    request.unit.to_string(),
                    Unit {
                        ours: true,
                        cgroup: Some(index),
                        instance,
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
                    let beside = state.add_cgroup(beside);
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
            Reply::Delivered => {
                // The success reply is delivered; the identity is captured
                // from the start's own signals before anything is returned.
                if state.panics(Op::Capture) {
                    panic_in(state, Op::Capture);
                }
                let instance = started.expect("a delivered start created its unit");
                let captured = captured(state.capture, instance);
                match state.interference {
                    Interference::None => {}
                    Interference::Replace(member) => {
                        state.unload(request.unit);
                        let index = state.load_foreign(request.unit, member);
                        state.place(request.helper_pid, index);
                    }
                    Interference::Swap(member) => {
                        state.swap_directory(request.unit, &[request.helper_pid, member]);
                    }
                }
                Started::Accepted(captured)
            }
            reply => Started::Uncertain(reply.reason("StartTransientUnit")),
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

    fn unit_instance(&self, unit_path: &str) -> Remote<Option<UnitInstance>> {
        let mut state = self.0.lock();
        state.calls.push(Call::UnitInstance(unit_path.to_string()));
        if state.panics(Op::UnitInstance) {
            panic_in(state, Op::UnitInstance);
        }
        let unit = unit_path.strip_prefix("/unit/").unwrap_or_default();
        let current = state.units.get(unit).map(|unit| unit.instance);
        match state.unit_instance.next() {
            Property::Uncertain(reply) => {
                Remote::Uncertain(state.connection(reply).reason("Get InvocationID"))
            }
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            Property::Exact => Remote::Answered(current),
            Property::Wrong => Remote::Answered(Some(other_instance(
                current.unwrap_or_else(|| model_instance(1)),
            ))),
            Property::WrongType => Remote::Answered(None),
        }
    }

    fn control_group_id(&self, unit_path: &str) -> Remote<Option<u64>> {
        let mut state = self.0.lock();
        state
            .calls
            .push(Call::ControlGroupId(unit_path.to_string()));
        if state.panics(Op::ControlGroupId) {
            panic_in(state, Op::ControlGroupId);
        }
        let unit = unit_path.strip_prefix("/unit/").unwrap_or_default();
        // The manager's record: the directory it realized for the unit (0
        // without one, as systemd answers).
        let recorded = state
            .units
            .get(unit)
            .and_then(|unit| unit.cgroup)
            .map_or(0, |index| state.cgroups[index].id);
        match state.control_group_id {
            Property::Uncertain(reply) => {
                Remote::Uncertain(state.connection(reply).reason("Get ControlGroupId"))
            }
            _ if state.broken => Remote::Uncertain(Reply::Disconnect.reason("Get")),
            Property::Exact => Remote::Answered(Some(recorded)),
            Property::Wrong => Remote::Answered(Some(recorded + 0x10_0000)),
            Property::WrongType => Remote::Answered(None),
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

    fn cgroup_id(&self) -> io::Result<u64> {
        Ok(self.world.lock().cgroups[self.index].id)
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
    // the manager's unit bound to it (P2-V1-R3B-I4-R3-R1: the invocation the
    // start captured, read before and after its `Id`, control group and the
    // directory's ID) and the manager's properties, still that invocation;
    // nothing was stopped.
    let object = format!("/unit/{unit}");
    assert_eq!(
        state.calls,
        [
            Call::Start(unit.clone(), pid),
            Call::Membership(pid, true),
            Call::Open(format!("{SLICE}/{unit}")),
            Call::Membership(pid, true),
            Call::GetUnit(unit.clone()),
            Call::UnitInstance(object.clone()),
            Call::UnitId(object.clone()),
            Call::ControlGroup(object.clone()),
            Call::ControlGroupId(object.clone()),
            Call::UnitInstance(object.clone()),
            Call::RuntimeMax(object.clone()),
            Call::OomPolicy(object.clone()),
            Call::UnitInstance(object),
        ]
    );
    let (instance, id) = (state.units[&unit].instance, state.created().unwrap().id);
    drop(state);
    assert_eq!(
        scope.invocation_id(),
        instance.bytes(),
        "the scope is bound to another invocation"
    );
    assert_eq!(scope.cgroup_id().unwrap(), id);
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
fn i4_03_a_start_with_effect_whose_reply_is_lost_is_never_proven_or_ended() {
    // P2-V1-R3B-I4-R3-R1: the request created the unit and its job placed
    // the helper, but the reply was lost (a timeout, a reply that does not
    // decode, an unexpected error, a broken connection). Nothing identifies
    // the invocation it may have started, so the cgroup the helper is in is
    // never opened, proven, launched into or ended: the operation stays owned
    // with its helper unreaped, through retries once the manager answers
    // again (the accepted availability cost).
    for (reply, placement) in [
        (Reply::Timeout, Placement::Immediate),
        (Reply::Malformed, Placement::AfterReads(3)),
        (Reply::Error, Placement::Immediate),
        (Reply::Disconnect, Placement::Immediate),
    ] {
        let world = World::new(|state| {
            state.start_reply = reply;
            state.placement = placement;
        });
        let (failed, pid) = refused(&world);
        assert!(
            matches!(failed.error, Some(ScopeError::Bus(_))),
            "{reply:?}: {:?}",
            failed.error
        );
        let boundary = retained(failed.cleanup);
        let observed = boundary
            .pending()
            .expect("the unresolved operation is retained");
        assert!(
            observed.issued
                && !observed.accepted
                && !observed.instance
                && !observed.candidate
                && !observed.owned
                && !observed.settled,
            "{reply:?}: {observed:?}"
        );
        assert!(boundary.holds_helper() && unreaped(pid), "{reply:?}");
        world.release();
        let boundary = boundary.retry().unwrap_err();
        let state = world.lock();
        assert_eq!(
            state.count(|call| matches!(call, Call::Open(_) | Call::Kill(_))),
            0,
            "{reply:?}: what an uncertain start may have created was opened or ended"
        );
        let created = state.created().unwrap();
        assert!(created.kills == 0 && !created.killed, "{reply:?}");
        assert!(state.units[&state.requested().unwrap()].ours, "{reply:?}");
        drop(state);
        abandon(boundary, pid);
    }
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
    // I4-R1: once the manager answers again, nothing without a candidate
    // confirms it either.
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
    let world = World::new(|state| state.placement = Placement::Never);
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
    // The manager has unloaded the unit by itself (P2-V1-R3B-I4-R3: nothing
    // stops it), so only the bound helper's position is left to observe;
    // another helper's membership is no evidence for this operation.
    {
        let mut state = world.lock();
        let unit = state.requested().unwrap();
        state.unload(&unit);
    }
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
    // operation; the manager's absence does not confirm it, and
    // (P2-V1-R3B-I4-R2, -R3) nothing is ever acted upon by the unit's name.
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
            // The manager answers again (GetUnit would answer NoSuchUnit
            // without the effect, the unit with it) and the helper is
            // outside. Still nothing confirms the operation, and the unit the
            // effect created stays loaded: nothing acts on it by its name.
            world.release();
            let Err(boundary) = boundary.retry() else {
                panic!("{case}: an uncertain first start was confirmed without proof");
            };
            assert!(
                boundary.holds_scope() && boundary.holds_helper() && unreaped(pid),
                "{case}: the helper was reaped while unresolved"
            );
            assert_eq!(
                world.lock().units.len(),
                usize::from(effect),
                "{case}: an uncertain first start was stopped by its name"
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

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3: no scope operation acts on a unit by its name. Tripwires
// over rustfmt-formatted source beside the behavioural tests
// (`execution::tests::i4r3_*`): the manager can start a scope and read,
// nothing else; settling asks it only whether the unit is loaded; and the
// recorded start reply changes only what that absence can confirm.

/// `source` without its `//` comments.
fn code_of(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of every item `head` opens in `source`, each up to `end`.
fn items<'a>(source: &'a str, head: &str, end: &str) -> Vec<&'a str> {
    let bodies: Vec<&str> = source
        .split(head)
        .skip(1)
        .map(|rest| rest.split(end).next().unwrap_or_default())
        .collect();
    assert!(!bodies.is_empty(), "{head} not found");
    bodies
}

/// The four arguments (object path, interface, method, body) of every
/// request `manager` makes through its one bounded call
/// (`self.call::<…>(…)` or `self.call_reply::<…>(…)`), in source order,
/// whitespace-normalized.
fn manager_requests(manager: &str) -> Vec<[String; 4]> {
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut requests = Vec::new();
    for (at, head) in manager.match_indices("self.call") {
        let rest = &manager[at + head.len()..];
        let Some(rest) = rest
            .strip_prefix("::<")
            .or_else(|| rest.strip_prefix("_reply::<"))
        else {
            // The forwarding inside `call` itself.
            continue;
        };
        // Past the turbofish.
        let mut depth = 1;
        let mut end = rest.len();
        for (index, c) in rest.char_indices() {
            match c {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let rest = rest[end..]
            .strip_prefix('(')
            .expect("a request's arguments");
        let mut depth = 0;
        let mut arguments = vec![String::new()];
        for c in rest.chars() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' if depth == 0 => break,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    arguments.push(String::new());
                    continue;
                }
                _ => {}
            }
            arguments.last_mut().expect("an argument").push(c);
        }
        let arguments: Vec<String> = arguments
            .iter()
            .map(|argument| words(argument))
            .filter(|argument| !argument.is_empty())
            .collect();
        requests.push(arguments.try_into().expect("four arguments"));
    }
    requests
}

#[test]
fn i4r3_10_the_manager_can_only_start_a_scope_and_read() {
    let manager = code_of(include_str!("manager.rs"));
    // The trait: one request that acts (a start, for a fresh name); every
    // other one reads (P2-V1-R3B-I4-R3-R1 adds the unit's `InvocationID` and
    // `ControlGroupId`).
    let declared: Vec<&str> = items(&manager, "pub(crate) trait Manager: Send + Sync {", "\n}\n")
        [0]
    .lines()
    .filter_map(|line| line.trim().strip_prefix("fn "))
    .map(|rest| rest.split('(').next().unwrap_or_default())
    .collect();
    assert_eq!(
        declared,
        [
            "start_scope",
            "get_unit",
            "runtime_max_usec",
            "oom_policy",
            "unit_id",
            "control_group",
            "unit_instance",
            "control_group_id"
        ],
        "the manager can act on a unit by its name"
    );
    // Every request goes through the one bounded call, to these methods
    // only: Subscribe (it acts on no unit: the manager sends this connection
    // its signals, so that a start captures its own identity),
    // StartTransientUnit, GetUnit, and property reads.
    assert_eq!(manager.matches(".call_method(").count(), 1);
    assert_eq!(
        manager
            .matches("self.call_reply(path, interface, method, body)")
            .count(),
        1
    );
    assert_eq!(
        manager.matches("self.call").count(),
        10,
        "the manager can act on a unit by its name"
    );
    assert_eq!(
        manager_requests(&manager),
        [
            ["SYSTEMD_PATH", "MANAGER", "\"Subscribe\"", "&()"],
            [
                "SYSTEMD_PATH",
                "MANAGER",
                "\"StartTransientUnit\"",
                "&(request.unit, \"fail\", properties, aux)",
            ],
            ["SYSTEMD_PATH", "MANAGER", "\"GetUnit\"", "&(unit,)"],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(SCOPE_INTERFACE, \"RuntimeMaxUSec\")",
            ],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(SCOPE_INTERFACE, \"OOMPolicy\")",
            ],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(UNIT_INTERFACE, \"Id\")"
            ],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(SCOPE_INTERFACE, \"ControlGroup\")",
            ],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(UNIT_INTERFACE, \"InvocationID\")",
            ],
            [
                "unit_path",
                "PROPERTIES",
                "\"Get\"",
                "&(SCOPE_INTERFACE, \"ControlGroupId\")",
            ],
        ],
        "the manager can act on a unit by its name"
    );
    for needle in [
        "StopUnit",
        "KillUnit",
        "RestartUnit",
        "ReloadUnit",
        "ResetFailed",
        "AbandonScope",
        "QueueSignal",
        "FreezeUnit",
        "ThawUnit",
        "SetUnitProperties",
        "AttachProcesses",
        "\"Stop\"",
        "\"Kill\"",
        "\"Set\"",
        "\"Unref\"",
        "stop_unit",
        "kill_unit",
    ] {
        assert!(
            !manager.contains(needle),
            "the manager can act on a unit by its name: {needle}"
        );
    }
}

#[test]
fn i4r3_10_settling_never_acts_on_a_unit_by_its_name() {
    let pending = code_of(include_str!("pending.rs"));
    // Every manager request of a pending operation: the start, the
    // binding's and the proof's reads, and GetUnit once more, as the
    // evidence of absence.
    let mut requests: Vec<&str> = pending
        .split("controller.manager.")
        .skip(1)
        .map(|call| call.split('(').next().unwrap_or_default())
        .collect();
    requests.sort_unstable();
    assert_eq!(
        requests,
        [
            "control_group",
            "control_group_id",
            "get_unit",
            "get_unit",
            "oom_policy",
            "runtime_max_usec",
            "start_scope",
            "unit_id",
            "unit_instance"
        ],
        "settling acts on a unit through the manager"
    );
    // Settling, observing and the drop backstop ask the manager nothing
    // themselves: only reads, through the binding (`bind`) and `absent`.
    for head in [
        "    pub(crate) fn reconcile(",
        "    fn observe(",
        "    fn acquire(",
        "    fn own(",
        "    fn gone(",
        "    fn end_owned(",
        "    pub(crate) fn end_now(",
    ] {
        for body in items(&pending, head, "\n    }\n") {
            assert!(
                !body.contains("manager"),
                "settling acts on a unit through the manager: {head}"
            );
        }
    }
    assert!(
        !items(&pending, "    fn bind(", "\n    }\n")[0].contains("start_scope"),
        "settling acts on a unit through the manager"
    );
    assert!(
        items(&pending, "    fn gone(", "\n    }\n")[0]
            .contains("self.accepted && absent(controller, &self.unit, helper)"),
        "settling acts on a unit through the manager"
    );
    let absent = items(&pending, "\nfn absent(", "\n}\n")[0];
    assert_eq!(
        absent.matches("manager").count(),
        1,
        "settling acts on a unit through the manager"
    );
    assert!(absent.contains("controller.manager.get_unit(unit),"));
    assert!(!items(&pending, "\nfn outside(", "\n}\n")[0].contains("manager"));
    // No stop or kill by a unit name anywhere in the crate's production
    // sources, nor what served one.
    for (file, source) in [
        ("scope.rs", include_str!("../scope.rs")),
        ("scope/manager.rs", include_str!("manager.rs")),
        ("scope/pending.rs", include_str!("pending.rs")),
        ("scope/native.rs", include_str!("native.rs")),
        ("execution.rs", include_str!("../execution.rs")),
        ("launcher.rs", include_str!("../launcher.rs")),
        ("fault.rs", include_str!("../fault.rs")),
    ] {
        let source = code_of(source);
        for needle in [
            "StopUnit",
            "stop_unit",
            "KillUnit",
            "kill_unit",
            "stop_authorized",
            "last_stop",
            "ScopeStop",
        ] {
            assert!(
                !source.contains(needle),
                "a stop by name in {file}: {needle}"
            );
        }
    }
}

#[test]
fn i4r3_10_a_recorded_start_reply_changes_only_what_absence_confirms() {
    let pending = code_of(include_str!("pending.rs"));
    // Set only in the delivered reply's arm, after the post-dispatch fault
    // point: a panic before the reply is recorded grants nothing.
    assert!(
        items(&pending, "Started::Accepted(captured) => {", "}")[0]
            .contains("self.accepted = true;"),
        "a start is accepted other than in the delivered reply's arm"
    );
    assert_eq!(
        pending.matches("self.accepted = true;").count(),
        1,
        "a start is accepted other than in the delivered reply's arm"
    );
    assert_eq!(
        pending.matches("accepted = ").count(),
        1,
        "a start is accepted other than in the delivered reply's arm"
    );
    assert!(
        pending.find("fault::at(fault, FaultPoint::AfterScopeStart);")
            < pending.find("self.accepted = true;")
    );
    // Read only where the manager's absence is weighed (`gone`), and by the
    // test and debug views: never by anything that acts.
    for (read, count) in [
        ("self.accepted", 4),
        ("self.accepted && absent(controller, &self.unit, helper)", 1),
        ("accepted: self.accepted,", 1),
        (".field(\"accepted\", &self.accepted)", 1),
    ] {
        assert_eq!(
            pending.matches(read).count(),
            count,
            "the recorded start reply gates more than what the manager's absence can confirm: {read}"
        );
    }
    // A pending operation stays crate-private, uncopied and unserialized
    // (and see `i4r1_05`, the accepted API guards and the normal-build
    // probes).
    assert!(!pending.contains("impl Clone for PendingScope"));
    assert!(pending.contains("\npub(crate) struct PendingScope {"));
    let declaration = pending
        .split("\npub(crate) struct PendingScope {")
        .next()
        .unwrap_or_default();
    assert!(
        !declaration
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .contains("derive"),
        "a pending operation derives a capability"
    );
}

// ---------------------------------------------------------------------------
// P2-V1-R3B-I4-R3-R1: the identity of the unit invocation a start began, and
// the separation of an observed candidate, cleanup authority and scope
// authority. The production decoding and capture rule over exact values,
// and tripwires over rustfmt-formatted source beside the behavioural tests
// (`execution::tests::r3r1_*`).

#[test]
fn r3r1_05_the_identity_decoding_and_the_capture_rule_fail_closed() {
    use super::manager::invocation_of;
    use zbus::zvariant::{OwnedValue, Value};
    let owned = |value: Value<'_>| -> OwnedValue { value.try_to_owned().expect("an owned value") };
    // The manager's encoding (`ay`): exactly 16 bytes, not all zero, is an
    // identity, and the empty array the null identity of a unit never
    // started; any other value is none.
    let bytes: Vec<u8> = (1..=16).collect();
    let id = UnitInstance::from_property(&bytes).expect("an identity");
    assert_eq!(id.bytes().as_slice(), bytes.as_slice());
    assert_eq!(
        invocation_of(owned(Value::from(bytes.clone()))),
        Invocation::Id(id)
    );
    assert_eq!(
        invocation_of(owned(Value::from(Vec::<u8>::new()))),
        Invocation::Null
    );
    for (value, what) in [
        (owned(Value::from(vec![0u8; 16])), "all zero"),
        (owned(Value::from(vec![7u8; 15])), "15 bytes"),
        (owned(Value::from(vec![7u8; 17])), "17 bytes"),
        (owned(Value::from(vec![7u32; 16])), "16 integers"),
        (owned(Value::from(42u64)), "an integer"),
        (owned(Value::from("0123456789abcdef")), "a string"),
        (
            owned(Value::Value(Box::new(Value::from(bytes.clone())))),
            "a variant inside the variant",
        ),
    ] {
        assert_eq!(
            invocation_of(value),
            Invocation::Malformed,
            "a wrong, zero or malformed identity was accepted: {what}"
        );
    }
    for wrong in [&[0u8; 16][..], &bytes[..15], &[7u8; 17][..], &[][..]] {
        assert_eq!(
            UnitInstance::from_property(wrong),
            None,
            "a wrong, zero or malformed identity was accepted: {wrong:?}"
        );
    }
    // The capture rule: the start job's removal reports `done` after the
    // reply, and between the two exactly one distinct identity appears from
    // the sender, none malformed. What systemd sends:
    let (a, b) = (model_instance(1), model_instance(2));
    let change = |serial, invocation| UnitChange { serial, invocation };
    assert_eq!(
        captured_instance(
            100,
            110,
            "done",
            &[
                change(101, Invocation::Null),
                change(105, Invocation::Id(a))
            ]
        ),
        Ok(a)
    );
    // The same identity twice is one; what lies outside the window (at its
    // bounds included) is not this start's.
    assert_eq!(
        captured_instance(
            100,
            110,
            "done",
            &[
                change(99, Invocation::Id(b)),
                change(100, Invocation::Malformed),
                change(103, Invocation::Id(a)),
                change(105, Invocation::Id(a)),
                change(110, Invocation::Id(b)),
                change(111, Invocation::Malformed),
            ]
        ),
        Ok(a)
    );
    let refused: [(u32, u32, &str, Vec<UnitChange>, &str); 13] = [
        (100, 110, "done", vec![], "no change"),
        (
            100,
            110,
            "done",
            vec![change(105, Invocation::Null)],
            "the null identity",
        ),
        (
            100,
            110,
            "done",
            vec![
                change(103, Invocation::Id(a)),
                change(105, Invocation::Malformed),
            ],
            "a malformed identity",
        ),
        (
            100,
            110,
            "done",
            vec![
                change(103, Invocation::Id(a)),
                change(105, Invocation::Id(b)),
            ],
            "two identities",
        ),
        (
            100,
            110,
            "done",
            vec![
                change(100, Invocation::Id(a)),
                change(110, Invocation::Id(a)),
            ],
            "the window's bounds",
        ),
        (
            100,
            110,
            "done",
            vec![change(111, Invocation::Id(a))],
            "after the removal",
        ),
        (
            100,
            110,
            "done",
            vec![change(99, Invocation::Id(a))],
            "before the reply",
        ),
        (
            100,
            110,
            "failed",
            vec![change(105, Invocation::Id(a))],
            "a failed job",
        ),
        (
            100,
            110,
            "canceled",
            vec![change(105, Invocation::Id(a))],
            "a canceled job",
        ),
        (
            100,
            110,
            "timeout",
            vec![change(105, Invocation::Id(a))],
            "a timed-out job",
        ),
        (
            100,
            110,
            "",
            vec![change(105, Invocation::Id(a))],
            "no result",
        ),
        (
            100,
            100,
            "done",
            vec![change(105, Invocation::Id(a))],
            "a removal at the reply",
        ),
        (
            100,
            90,
            "done",
            vec![change(95, Invocation::Id(a))],
            "wrapped serials",
        ),
    ];
    for (reply, removed, result, changes, what) in refused {
        assert!(
            captured_instance(reply, removed, result, &changes).is_err(),
            "an identity was captured from {what}"
        );
    }
}

#[test]
fn r3r1_x_a_start_watches_its_unit_s_own_object_path() {
    // systemd's `unit_dbus_path_from_name` (`bus_label_escape`): every byte
    // but an ASCII letter, or a digit after the first byte, as `_` and two
    // lowercase hex digits; the empty name as `_`.
    use super::manager::unit_object_path;
    for (name, path) in [
        (
            "nexus-verifier-0123456789abcdef0123456789abcdef.scope",
            "/org/freedesktop/systemd1/unit/nexus_2dverifier_2d0123456789abcdef0123456789abcdef_2escope",
        ),
        ("1a.scope", "/org/freedesktop/systemd1/unit/_31a_2escope"),
        ("a_b", "/org/freedesktop/systemd1/unit/a_5fb"),
        ("\u{e9}", "/org/freedesktop/systemd1/unit/_c3_a9"),
        ("", "/org/freedesktop/systemd1/unit/_"),
    ] {
        assert_eq!(unit_object_path(name), path, "{name:?}");
    }
}

#[test]
fn r3r1_11_the_identity_and_the_ownership_have_no_surface() {
    let manager = code_of(include_str!("manager.rs"));
    let pending = code_of(include_str!("pending.rs"));
    let scope = code_of(include_str!("../scope.rs"));
    let native = code_of(include_str!("native.rs"));
    // No public minting of an identity: the type and its one constructor
    // are crate-private, it is made only from the manager's own property
    // value, and nothing converts into one, defaults one or decodes one.
    assert!(manager.contains(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub(crate) struct UnitInstance([u8; 16]);"
    ));
    assert_eq!(
        manager.matches("UnitInstance::from_property(").count(),
        1,
        "an identity is made other than from the manager's property"
    );
    assert!(items(&manager, "pub(crate) fn invocation_of(", "\n}\n")[0]
        .contains("UnitInstance::from_property(&bytes)"));
    for (file, source) in [
        ("scope/manager.rs", &manager),
        ("scope/pending.rs", &pending),
        ("scope.rs", &scope),
        ("scope/native.rs", &native),
    ] {
        for line in source.lines().filter(|line| line.contains("#[derive(")) {
            assert!(
                !line.contains("Serialize") && !line.contains("Deserialize"),
                "{file}: a serializable scope type: {line}"
            );
        }
        for needle in [
            "for UnitInstance",
            "impl serde",
            "Candidate {}",
            "impl Clone for Candidate",
        ] {
            assert!(
                !source.contains(needle),
                "{file}: an identity or a candidate gains a conversion: {needle}"
            );
        }
    }
    // No caller-supplied identity: a request names only the unit, the
    // helper and the limits; an identity is recorded only from an accepted
    // start's own capture, in that one arm.
    let request = items(&manager, "pub(crate) struct ScopeRequest<'a> {", "\n}\n")[0];
    assert_eq!(
        request
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>(),
        [
            "    pub unit: &'a str,",
            "    pub helper_pid: u32,",
            "    pub limits: &'a ResourcePolicy,"
        ]
    );
    assert_eq!(manager.matches("Started::Accepted(").count(), 1);
    assert_eq!(
        pending.matches("self.instance = ").count(),
        1,
        "an identity is recorded other than from an accepted start's own capture"
    );
    assert!(items(&pending, "Started::Accepted(captured) => {", "}")[0]
        .contains("self.instance = Some(captured.map_err(ScopeError::Bus)?);"));
    // No frontend identity: nothing an execution reports or a normal build
    // exposes names it; the scope's harness accessors only read it.
    for (file, source) in [
        ("execution.rs", include_str!("../execution.rs")),
        ("protocol.rs", include_str!("../protocol.rs")),
        ("lib.rs", include_str!("../lib.rs")),
    ] {
        let source = code_of(source);
        for needle in ["UnitInstance", "InvocationID", "invocation_id", "cgroup_id"] {
            assert!(!source.contains(needle), "{file} names {needle}");
        }
    }
    for accessor in [
        "    pub fn invocation_id(&self) -> [u8; 16] {",
        "    pub fn cgroup_id(&self) -> io::Result<u64> {",
    ] {
        let before = scope
            .split(accessor)
            .next()
            .unwrap_or_default()
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default();
        assert_eq!(
            before.trim(),
            "#[cfg(any(test, feature = \"live-sandbox-harness\"))]",
            "{accessor}"
        );
    }
    // No start or stop seam in a normal build: the one start is the pending
    // operation's own (see also `i4r1_04`, `i4r1_x` and `i4r3_10_*`).
    assert_eq!(pending.matches("start_scope(").count(), 1);
    assert!(manager.contains("\npub(crate) trait Manager: Send + Sync {"));
    // An observed candidate and cleanup authority are separate: the one
    // `cgroup.kill` of a pending operation goes through the owned candidate
    // only; ownership is set in one place, once the binding closed; a
    // candidate is made unowned, where it is opened, and never rebuilt.
    assert_eq!(pending.matches(".kill()").count(), 2);
    assert!(items(&pending, "    fn end_owned(", "\n    }\n")[0]
        .contains("if let Some(owned) = self.owned_dir() {"));
    assert!(
        items(&pending, "    pub(crate) fn end_now(", "\n    }\n")[0].contains("self.end_owned();")
    );
    assert!(items(&pending, "    fn owned_dir(", "\n    }\n")[0]
        .contains(".filter(|candidate| candidate.owned)"));
    assert_eq!(pending.matches("owned = true").count(), 1);
    let bind = items(&pending, "    fn bind(", "\n    }\n")[0];
    assert!(bind.ends_with(
        "same_instance(controller, &unit_path, instance)?;\n        candidate.owned = true;\n        Ok(unit_path)"
    ));
    assert_eq!(pending.matches("owned: false,").count(), 2);
    assert_eq!(pending.matches("Candidate {").count(), 3);
    for item in ["\nstruct Candidate {", "\npub(crate) struct PendingScope {"] {
        let before = pending.split(item).next().unwrap_or_default();
        assert!(
            !before
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or_default()
                .contains("derive"),
            "an observed or owned candidate derives a capability: {item}"
        );
    }
    assert_eq!(pending.matches("controller.native.open(").count(), 2);
    // Cleanup authority and scope authority are separate: the proven scope
    // is made in one place, after every proof and the promotion point, from
    // an owned candidate only.
    assert_eq!(pending.matches("Self::Proven(Scope {").count(), 1);
    let establish = items(&pending, "    pub(crate) fn establish(", "\n    }\n")[0];
    assert!(
        establish.find("pending.issue_and_prove(helper, fault)?;")
            < establish.find("fault::at(fault, FaultPoint::ScopePromotion);")
            && establish.find("fault::at(fault, FaultPoint::ScopePromotion);")
                < establish.find("take_if(|candidate| candidate.owned)")
            && establish.find("take_if(|candidate| candidate.owned)")
                < establish.find("Self::Proven(Scope {")
    );
}

#[test]
fn r3r1_x_the_host_qualification_reads_the_identity_production_records() {
    // P2-V1-R3B-I4-R3-R1: the live gate's H3/H4 case qualifies, on the real
    // host and the same proven scope, the facts production now trusts: the
    // unit's `InvocationID` is an `ay` of 16 bytes, not all zero, equal to
    // the identity production recorded, read again after the other reads;
    // `ControlGroupId` is a `t` equal to the retained directory's kernel
    // cgroup ID; the helper's membership stays the bound cgroup. A
    // same-name replacement's other identity rests on the primary source,
    // without another state-mutating case: the count stays 39.
    let harness = code_of(include_str!("../../tests/phase2_live_sandbox.rs"));
    let case = items(
        &harness,
        "        pub fn h3_h4_binding(scopes: &ScopeManager) {",
        "\n        }\n",
    )[0];
    for needle in [
        "let recorded = placed.scope().unwrap().invocation_id();",
        "\"ControlGroupId\"",
        ".cgroup_id()",
        "control_group_id, retained,",
        "hq::membership(pid).unwrap().unified.as_deref(),",
    ] {
        assert_eq!(case.matches(needle).count(), 1, "{needle}");
    }
    assert_eq!(
        case.matches("invocation_property(name, &probe, &object)")
            .count(),
        2
    );
    let property = items(&harness, "        fn invocation_property(", "\n        }\n")[0];
    for needle in [
        "probe.property(object, UNIT_INTERFACE, \"InvocationID\")",
        "\"ay\",",
        "assert_ne!(id, [0; 16],",
    ] {
        assert!(property.contains(needle), "{needle}");
    }
    let documented = include_str!("../../tests/phase2_live_sandbox.rs");
    assert!(documented.contains("(`sd_id128_randomize`, the evidence of"));
    assert!(documented.contains("state-mutating case; the live count stays 39."));
    assert!(harness.contains("let mut passed = 10;"));
    assert!(harness.contains("let scoped: [ScopedCase; 29]"));
}
