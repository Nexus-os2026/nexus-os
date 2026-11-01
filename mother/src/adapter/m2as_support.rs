//! M2a-S (Codex REJECT, oracle first): test support for the real-broker
//! oracles in `m2as_fence.rs`, `m2as_adopt.rs` and `m2as_model.rs`.
//!
//! - [`Disk`]: a journal store whose appends fail (now, or after a count)
//!   and whose poisoning fails, on demand; it can measure the world at the
//!   first failure (the moment the coordinator breaks).
//! - the real port's test-only hooks (`GovernedPort::before_perform`,
//!   `before_approval`), run just before it performs an effect or asks the
//!   broker about an approval: a deterministic interleaving (the hook is
//!   the other thread's work, done at exactly that point);
//! - [`HookLane`] (and `oracles::m2as_hooks::HookConfirmer`): the same inside
//!   a lane call and inside the Owner's consent dialog.
//! - [`WorldServer`]: the world, counting every request that reaches it.

use super::{mother_agent, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::journal::{JournalError, JournalStore, MemoryJournal};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput, LaneRole};
pub(crate) use crate::oracles::m2as_hooks::{HookConfirmer, HookSlot, OwnerApproves};
use crate::oracles::KeyedLane;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::RunId;
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

// ---------------------------------------------------------------- rig

/// A Mother process over `store` and the shared broker: a port of its own
/// (gated), hooked keyed lanes.
pub(crate) struct Rig {
    pub port: Arc<GovernedPort>,
    pub mother: Arc<Coordinator<GovernedPort>>,
    pub lane_hook: HookSlot,
}

pub(crate) fn rig(control: &Arc<GovernedControl>, store: Disk) -> Result<Rig, CoordinatorError> {
    let port = Arc::new(GovernedPort::new(control.clone()));
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
