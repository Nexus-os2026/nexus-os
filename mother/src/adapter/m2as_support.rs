//! M2a-S (Codex REJECT, oracle first): test support for the real-broker
//! oracles in `m2as_fence.rs`, `m2as_adopt.rs` and `m2as_model.rs`.
//!
//! - [`Disk`]: a journal store whose appends fail (now, or after a count)
//!   and whose poisoning fails, on demand; it can measure the world at the
//!   first failure (the moment the coordinator breaks).
//! - [`Gated`]: the real port with hooks run just before it performs an
//!   effect or asks the broker about an approval (a deterministic
//!   interleaving: the hook is the other thread's work, done at exactly
//!   that point).
//! - [`HookLane`], [`HookConfirmer`]: the same inside a lane call and inside
//!   the Owner's consent dialog.
//! - [`WorldServer`]: the world, counting every request that reaches it.

use super::{mother_agent, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{
    ApprovalStatus, EffectOutcome, EffectPort, EffectRefusal, EffectRequest, PendingAction,
};
use crate::journal::{JournalError, JournalStore, MemoryJournal, MissionId};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole};
use crate::oracles::KeyedLane;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    MissionConfirmation, ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::authority::Authority;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

// ---------------------------------------------------------------- disk

type Measure = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct DiskState {
    /// Appends fail once this many more have succeeded.
    fail_after: Option<usize>,
    poison_fails: bool,
    failures: usize,
    on_failure: Option<Measure>,
}

/// A journal store over a memory journal whose appends and poisoning can
/// be made to fail.
#[derive(Clone)]
pub(crate) struct Disk {
    pub inner: MemoryJournal,
    state: Arc<Mutex<DiskState>>,
}

impl Disk {
    pub fn over(inner: MemoryJournal) -> Self {
        Self {
            inner,
            state: Arc::new(Mutex::new(DiskState::default())),
        }
    }

    /// Appends fail now (`true`) or work (`false`); poisoning likewise.
    pub fn fail(&self, appends: bool, poison: bool) {
        self.fail_after(appends.then_some(0), poison);
    }

    /// Appends fail once `n` more have succeeded (`None`: they work).
    pub fn fail_after(&self, n: Option<usize>, poison: bool) {
        let mut state = self.state.lock().unwrap();
        state.fail_after = n;
        state.poison_fails = poison;
    }

    /// How many appends failed.
    pub fn failures(&self) -> usize {
        self.state.lock().unwrap().failures
    }

    /// Run `measure` at each failing append (under the coordinator's lock,
    /// at the moment it breaks).
    pub fn on_failure(&self, measure: Option<Measure>) {
        self.state.lock().unwrap().on_failure = measure;
    }
}

impl JournalStore for Disk {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.inner.load()
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let measure = {
            let mut state = self.state.lock().unwrap();
            match state.fail_after.as_mut() {
                Some(0) => {
                    state.failures += 1;
                    Some(state.on_failure.clone())
                }
                Some(n) => {
                    *n -= 1;
                    None
                }
                None => None,
            }
        };
        if let Some(measure) = measure {
            if let Some(measure) = measure {
                measure();
            }
            return Err(JournalError::Store("injected append failure".into()));
        }
        self.inner.append(line)
    }

    fn poison(&mut self) -> Result<(), JournalError> {
        if self.state.lock().unwrap().poison_fails {
            return Err(JournalError::Store("injected poison failure".into()));
        }
        self.inner.poison()
    }
}

// ---------------------------------------------------------------- hooks

type Hook = Box<dyn FnOnce() + Send>;

/// One armed hook: it runs on the call after `skip` more calls, once.
#[derive(Clone, Default)]
pub(crate) struct HookSlot(Arc<Mutex<Option<(usize, Hook)>>>);

impl HookSlot {
    pub fn arm(&self, skip: usize, hook: impl FnOnce() + Send + 'static) {
        *self.0.lock().unwrap() = Some((skip, Box::new(hook)));
    }

    pub fn disarm(&self) {
        *self.0.lock().unwrap() = None;
    }

    pub fn armed(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }

    fn fire(&self) {
        let hook = {
            let mut slot = self.0.lock().unwrap();
            match slot.as_mut() {
                Some((0, _)) => slot.take().map(|(_, hook)| hook),
                Some((n, _)) => {
                    *n -= 1;
                    None
                }
                None => None,
            }
        };
        if let Some(hook) = hook {
            hook();
        }
    }
}

/// The real port, with hooks just before a perform and an approval query.
pub(crate) struct Gated {
    pub inner: GovernedPort,
    pub before_perform: HookSlot,
    pub before_approval: HookSlot,
}

impl Gated {
    pub fn over(control: &Arc<GovernedControl>) -> Self {
        Self {
            inner: GovernedPort::new(control.clone()),
            before_perform: HookSlot::default(),
            before_approval: HookSlot::default(),
        }
    }
}

impl EffectPort for Gated {
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        self.before_perform.fire();
        self.inner.perform(mission, effect)
    }
    fn cancel(&self, mission: MissionId) {
        self.inner.cancel(mission)
    }
    fn finish(&self, mission: MissionId) {
        self.inner.finish(mission)
    }
    fn emergency_stopped(&self) -> bool {
        self.inner.emergency_stopped()
    }
    fn broker(&self) -> &Authority {
        self.inner.broker()
    }
    fn approval(&self, mission: MissionId) -> ApprovalStatus {
        self.before_approval.fire();
        self.inner.approval(mission)
    }
    fn pending(&self, mission: MissionId) -> Option<PendingAction> {
        self.inner.pending(mission)
    }
    fn adopt(&self, mission: MissionId, pending: &PendingAction) {
        self.inner.adopt(mission, pending)
    }
    fn settled(&self, pending: &PendingAction) -> bool {
        self.inner.settled(pending)
    }
    fn withdraw(&self, mission: MissionId) -> Result<(), EffectRefusal> {
        self.inner.withdraw(mission)
    }
    fn fence(&self) {
        self.inner.fence()
    }
}

/// A keyed lane with a hook run inside its call, before it answers.
pub(crate) struct HookLane {
    inner: KeyedLane,
    hook: HookSlot,
}

impl Lane for HookLane {
    fn identity(&self) -> LaneIdentity {
        self.inner.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        self.hook.fire();
        self.inner.invoke(input)
    }
}

/// The Owner at the mission dialog: runs its hook while asked, then
/// answers `yes`.
pub(crate) struct HookConfirmer {
    pub hook: HookSlot,
    pub yes: bool,
}

impl ControlConfirmer for HookConfirmer {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.hook.fire();
        self.yes
    }
}

/// The Owner approving an action at the broker's native dialog.
pub(crate) struct OwnerApproves;
impl ControlConfirmer for OwnerApproves {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

// ---------------------------------------------------------------- rig

/// A Mother process over `store` and the shared broker: a port of its own
/// (gated), hooked keyed lanes.
pub(crate) struct Rig {
    pub port: Arc<Gated>,
    pub mother: Arc<Coordinator<Gated>>,
    pub lane_hook: HookSlot,
}

pub(crate) fn rig(control: &Arc<GovernedControl>, store: Disk) -> Result<Rig, CoordinatorError> {
    let port = Arc::new(Gated::over(control));
    let lane_hook = HookSlot::default();
    let lanes: Vec<Arc<dyn Lane>> = LaneRole::ALL
        .iter()
        .map(|role| {
            Arc::new(HookLane {
                inner: KeyedLane::new(*role),
                hook: lane_hook.clone(),
            }) as Arc<dyn Lane>
        })
        .collect();
    let mother = Coordinator::open(
        store,
        port.clone(),
        Lanes::new(lanes).unwrap(),
        Arc::new(SystemClock::default()),
    )?;
    Ok(Rig {
        port,
        mother: Arc::new(mother),
        lane_hook,
    })
}

/// Commitments of `runs` that executed or may (approved), at the broker.
pub(crate) fn executed(control: &GovernedControl, runs: &BTreeSet<RunId>) -> usize {
    runs.iter()
        .flat_map(|run| control.authority().commitments().views_of_run(*run))
        .filter(|v| {
            matches!(
                v.state,
                CommitmentState::Authorized
                    | CommitmentState::Executing
                    | CommitmentState::Succeeded
                    | CommitmentState::Failed
            )
        })
        .count()
}

/// Commitments of `runs` that the Owner could still approve or execute.
pub(crate) fn open_requests(
    control: &GovernedControl,
    runs: &BTreeSet<RunId>,
) -> Vec<CommitmentView> {
    runs.iter()
        .flat_map(|run| control.authority().commitments().views_of_run(*run))
        .filter(|v| {
            matches!(
                v.state,
                CommitmentState::Prepared | CommitmentState::Authorized
            )
        })
        .collect()
}

/// Whether the Owner could still run `view` at the broker (tries both
/// steps; an approval that succeeds is then executed).
pub(crate) fn owner_runs(control: &GovernedControl, view: &CommitmentView) -> bool {
    let agent = mother_agent();
    let authorized = view.state == CommitmentState::Authorized
        || control
            .authorize(view.id, &agent, view.run, &OwnerApproves)
            .is_ok();
    authorized && control.execute(view.id, &agent, view.run).is_ok()
}

// ---------------------------------------------------------------- world

/// The world: answers every request on the listener and keeps its line.
pub(crate) struct WorldServer {
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WorldServer {
    pub fn start(listener: &TcpListener) -> Self {
        let listener = listener.try_clone().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, stopping) = (requests.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let mut request = [0u8; 4096];
                        let n = stream.read(&mut request).unwrap_or(0);
                        let line = String::from_utf8_lossy(&request[..n])
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_string();
                        seen.lock().unwrap().push(line);
                        let _ = stream.write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nmother",
                        );
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        });
        Self {
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    /// A handle on what reached the world, for a measure taken elsewhere.
    pub fn counter(&self) -> Arc<Mutex<Vec<String>>> {
        self.requests.clone()
    }

    /// Whether a request whose line contains `part` reached the world.
    pub fn saw(&self, part: &str) -> bool {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains(part))
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
