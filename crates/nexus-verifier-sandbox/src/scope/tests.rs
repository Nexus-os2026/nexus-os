//! A deterministic simulation of the systemd user manager and of the kernel
//! for the scope module's ownership tests (P2-V1-R3B-I4). Test builds only:
//! nothing here contacts a bus, reads `/proc` for a decision or creates a
//! cgroup, and nothing here is production authority.
//!
//! The model's manager answers every call from a script: a reply delivered,
//! an effect whose reply is lost, no effect with the reply lost, a timeout,
//! a disconnection or a malformed reply. The model's kernel keeps each
//! helper's membership and the cgroups the manager created. Every call is
//! logged, so a test can assert what was and was not asked.

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
    /// In a cgroup of this path instead (the unit name substituted for
    /// `{unit}`).
    Elsewhere(&'static str),
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
    pub runtime_max: Property,
    pub oom_policy: Property,
    /// StopUnit: whether it takes effect, and its reply.
    pub stop: Script<(bool, Reply)>,
    /// Opens that still succeed (`None`: every one).
    pub opens_left: Option<u32>,
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
            runtime_max: Property::Exact,
            oom_policy: Property::Exact,
            stop: Script::always((true, Reply::Delivered)),
            opens_left: None,
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
    fn place(&mut self, pid: u32, index: usize) {
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
                Placement::Elsewhere(path) => path.replace("{unit}", request.unit),
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
                Placement::AfterReads(reads) => {
                    state.placing = Some((request.helper_pid, index, reads))
                }
                Placement::Never => {}
            }
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
        match state.connection(reply) {
            Reply::Delivered => Remote::Answered(()),
            reply => Remote::Uncertain(reply.reason("StopUnit")),
        }
    }

    fn get_unit(&self, unit: &str) -> Remote<Presence> {
        let mut state = self.0.lock();
        state.calls.push(Call::GetUnit(unit.to_string()));
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
// Scope-level tests: the public `ScopeManager::start` and `PendingScope`.

const LIMITS: ResourcePolicy = ResourcePolicy::RUST_OFFLINE_V1;

/// A live stand-in helper (`cat` holds its control socket open).
fn helper() -> Helper {
    Helper::spawn(&HelperProgram::at("/bin/cat")).unwrap().0
}

fn finish(mut helper: Helper) {
    let _ = helper.kill();
    helper.reap().unwrap();
}

#[test]
fn i4_01_a_confirmed_start_with_every_proof_is_proven() {
    let world = World::default();
    let helper = helper();
    let scope = world.scopes().start(&helper, &LIMITS).unwrap();
    let state = world.lock();
    let unit = state.requested().unwrap();
    assert_eq!(scope.unit(), unit);
    assert!(unit.starts_with("nexus-verifier-") && unit.ends_with(".scope"));
    // Proven through the kernel's view of the helper, the retained cgroup
    // and the manager's properties, and nothing was stopped.
    assert_eq!(
        state.calls,
        [
            Call::Start(unit.clone(), helper.pid()),
            Call::Membership(helper.pid(), true),
            Call::Open(format!("{SLICE}/{unit}")),
            Call::GetUnit(unit.clone()),
            Call::RuntimeMax(format!("/unit/{unit}")),
            Call::OomPolicy(format!("/unit/{unit}")),
        ]
    );
    drop(state);
    assert_eq!(
        scope.occupancy().unwrap(),
        crate::scope::Occupancy::Populated
    );
    finish(helper);
}

#[test]
fn i4_02_a_start_without_effect_whose_reply_is_lost_is_proven_absent() {
    for reply in [Reply::Timeout, Reply::Error, Reply::Malformed] {
        let world = World::new(|state| {
            state.start_effect = false;
            state.start_reply = reply;
        });
        let helper = helper();
        let failed = world.scopes().start(&helper, &LIMITS).unwrap_err();
        assert!(matches!(failed.error, ScopeError::Bus(_)), "{reply:?}");
        assert!(failed.unresolved.is_none(), "{reply:?}: absence was proven");
        let state = world.lock();
        // Proven by the manager, on the issuing connection after the
        // request, and by the kernel (when the request fails and again when
        // the operation is settled); nothing to stop, nothing opened.
        assert!(matches!(state.calls[0], Call::Start(..)));
        assert_eq!(state.count(|call| matches!(call, Call::GetUnit(_))), 2);
        assert_eq!(state.count(|call| matches!(call, Call::Stop(_))), 0);
        assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 0);
        drop(state);
        finish(helper);
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
        let helper = helper();
        let started = world.scopes().start(&helper, &LIMITS);
        let scope = started.unwrap_or_else(|failed| {
            panic!("{reply:?}: the unit a lost-reply start created was not discovered and proven: {failed:?}")
        });
        assert_eq!(Some(scope.unit().to_string()), world.lock().requested());
        assert_eq!(
            world.lock().count(|call| matches!(call, Call::Stop(_))),
            0,
            "{reply:?}"
        );
        finish(helper);
    }
    // The connection broke with the request: the scope cannot be proven,
    // and what was created is settled through its retained cgroup.
    let world = World::new(|state| state.start_reply = Reply::Disconnect);
    let helper = helper();
    let failed = world.scopes().start(&helper, &LIMITS).unwrap_err();
    assert!(matches!(failed.error, ScopeError::Bus(_)));
    assert!(failed.unresolved.is_none(), "{failed:?}");
    assert!(world.lock().created().unwrap().killed);
    finish(helper);
}

#[test]
fn i4_03_a_start_whose_effect_cannot_be_confirmed_gone_is_returned_owned() {
    // Created, reply lost, the helper never placed, the connection gone:
    // nothing can establish that the unit is gone, so the failure carries
    // the operation, owned.
    let world = World::new(|state| {
        state.start_reply = Reply::Disconnect;
        state.placement = Placement::Never;
    });
    let helper = helper();
    let failed = world.scopes().start(&helper, &LIMITS).unwrap_err();
    let mut pending = failed.unresolved.expect("the uncertain operation is owned");
    let observed = pending.state();
    assert!(observed.issued && !observed.settled && !observed.candidate);
    assert!(
        !pending.settle(&helper),
        "nothing is confirmed while uncertain"
    );
    // Once the manager answers again on the same connection, settling it
    // confirms it gone.
    world.release();
    assert!(pending.settle(&helper));
    assert!(!world.lock().units.contains_key(pending.unit()));
    finish(helper);
}

#[test]
fn i4_start_failed_settles_with_the_live_helper_and_reports_the_proof_failure() {
    // A proof that fails on the retained cgroup: settling ends what is in
    // it, and the failure names the proof.
    let world = World::new(|state| state.limit_mismatch = Some("pids.max"));
    let helper = helper();
    let failed = world.scopes().start(&helper, &LIMITS).unwrap_err();
    assert!(matches!(failed.error, ScopeError::Mismatch("pids.max")));
    assert!(failed.unresolved.is_none());
    let state = world.lock();
    assert!(state.created().unwrap().killed);
    assert_eq!(state.count(|call| matches!(call, Call::Open(_))), 1);
    drop(state);
    finish(helper);
}

#[test]
fn i4_a_pending_operation_is_bound_to_its_own_helper() {
    let world = World::new(|state| {
        state.placement = Placement::Never;
        state.stop = Script::always((false, Reply::Timeout));
    });
    let (bound, other) = (helper(), helper());
    let failed = world.scopes().start(&bound, &LIMITS).unwrap_err();
    let mut pending = failed.unresolved.expect("the unit is still loaded");
    // Another helper's membership is no evidence for this operation.
    world.release();
    assert!(
        !pending.settle(&other),
        "another helper's membership settled the operation"
    );
    assert_eq!(
        world
            .lock()
            .count(|call| matches!(call, Call::Membership(pid, _) if *pid == other.pid())),
        0,
        "another helper's membership settled the operation"
    );
    assert!(pending.settle(&bound));
    // A prepared operation is never issued for another helper.
    let scopes = world.scopes();
    let mut boundary = super::ScopeBoundary::Pending(scopes.prepare(&bound, &LIMITS).unwrap());
    assert!(matches!(
        boundary.establish(&other, None),
        Err(ScopeError::Mismatch("scope operation"))
    ));
    let unresolved = boundary.into_pending().unwrap();
    assert!(!unresolved.state().issued);
    finish(bound);
    finish(other);
}

#[test]
fn i4_owners_are_self_contained_values() {
    // Movable to any thread and borrowing nothing: an unresolved operation
    // and a retained boundary outlive whatever created them.
    fn owned<T: Send + 'static>() {}
    owned::<super::PendingScope>();
    owned::<super::StartFailed>();
    owned::<super::Scope>();
    owned::<ScopeManager>();
    owned::<crate::execution::RetainedBoundary>();
}
