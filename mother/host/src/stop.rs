//! Memory-only host admission and separately owned synchronous Stop workers.
use nexus_governed_control::governed::GovernedControl;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) enum Route {
    Mission,
    Door,
    Run,
    Files,
    Lane,
    Inference,
    Model,
}
impl Route {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 7] = [
        Self::Mission,
        Self::Door,
        Self::Run,
        Self::Files,
        Self::Lane,
        Self::Inference,
        Self::Model,
    ];
}

#[derive(Default)]
pub(crate) struct Gate {
    recovery: AtomicBool,
    closing: AtomicBool,
    failed: AtomicBool,
    epoch: AtomicU64,
}
impl Gate {
    pub(crate) fn admit(&self, _route: Route) -> bool {
        !self.recovery() && !self.closing() && !self.failed()
    }
    pub(crate) fn recovery(&self) -> bool {
        self.recovery.load(Ordering::SeqCst)
    }
    pub(crate) fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }
    pub(crate) fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }
    pub(crate) fn fail(&self) {
        self.failed.store(true, Ordering::SeqCst);
        self.close();
    }
    pub(crate) fn emergency(&self) {
        // This monotone restriction precedes every later lock and spawn.
        self.recovery.store(true, Ordering::SeqCst);
        if self
            .epoch
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .is_err()
        {
            self.fail();
        }
    }
}

struct Worker {
    id: u64,
    spawning: bool,
    handle: Option<JoinHandle<()>>,
}
#[derive(Default)]
struct State {
    next: u64,
    workers: Vec<Worker>,
    coalesced: u64,
    finished: u64,
    failed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub epoch: u64,
    pub outstanding: usize,
    pub coalesced: u64,
    pub finished: u64,
    pub failed: bool,
}
type Body = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct StopRoute {
    pub(crate) gate: Arc<Gate>,
    body: Mutex<Option<Body>>,
    bound: AtomicBool,
    state: Mutex<State>,
    changed: Condvar,
}
impl StopRoute {
    pub(crate) fn new(gate: Arc<Gate>) -> Arc<Self> {
        Arc::new(Self {
            gate,
            body: Mutex::new(None),
            bound: AtomicBool::new(false),
            state: Mutex::default(),
            changed: Condvar::new(),
        })
    }
    #[cfg(test)]
    pub(crate) fn with_body(gate: Arc<Gate>, body: Body) -> Arc<Self> {
        let route = Self::new(gate);
        *route.body.lock().unwrap() = Some(body);
        route.bound.store(true, Ordering::SeqCst);
        route
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| {
            self.gate.fail();
            let mut state = p.into_inner();
            state.failed = true;
            state
        })
    }
    pub(crate) fn bind(self: &Arc<Self>, owner: Arc<GovernedControl>) -> bool {
        if self.bound.swap(true, Ordering::SeqCst) {
            self.gate.fail();
            return false;
        }
        let Ok(mut body) = self.body.lock() else {
            self.gate.fail();
            return false;
        };
        *body = Some(Arc::new(move || {
            owner.emergency_stop();
        }));
        drop(body);
        self.dispatch_pending();
        true
    }
    pub(crate) fn press(self: &Arc<Self>) {
        self.gate.emergency();
        {
            let mut state = self.lock();
            if state.workers.len() >= 4 {
                match state.coalesced.checked_add(1) {
                    Some(n) => state.coalesced = n,
                    None => {
                        state.failed = true;
                        self.gate.fail();
                    }
                }
                self.changed.notify_all();
                return;
            }
            let Some(id) = state.next.checked_add(1) else {
                state.failed = true;
                self.gate.fail();
                return;
            };
            state.next = id;
            state.workers.push(Worker {
                id,
                spawning: false,
                handle: None,
            });
        }
        self.dispatch_pending();
    }
    fn dispatch_pending(self: &Arc<Self>) {
        let body = self.body.lock().ok().and_then(|body| body.clone());
        let Some(body) = body else {
            return;
        };
        loop {
            let id = {
                let mut state = self.lock();
                let Some(worker) = state.workers.iter_mut().find(|w| !w.spawning) else {
                    return;
                };
                worker.spawning = true;
                worker.id
            };
            let route = self.clone();
            let body = body.clone();
            let handle = std::thread::Builder::new()
                .name("hands-stop".into())
                .spawn(move || {
                    let ok =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body())).is_ok();
                    if !ok {
                        route.gate.fail();
                        route.lock().failed = true;
                    }
                    route.changed.notify_all();
                });
            let mut state = self.lock();
            match handle {
                Ok(handle) => {
                    // The reserved entry cannot be reaped until this owned
                    // handle is registered AND reports completion.
                    if let Some(worker) = state.workers.iter_mut().find(|w| w.id == id) {
                        worker.handle = Some(handle);
                    } else {
                        state.failed = true;
                        self.gate.fail();
                    }
                }
                Err(_) => {
                    state.workers.retain(|w| w.id != id);
                    state.failed = true;
                    self.gate.fail();
                }
            }
            self.changed.notify_all();
        }
    }
    /// Only after stdin and every Stop worker have joined. Transfer the
    /// backend reference into the owned cleanup worker before process exit.
    pub(crate) fn take_body(&self) -> Option<Body> {
        self.body.lock().ok().and_then(|mut body| body.take())
    }
    pub(crate) fn reap(&self) {
        let handles = {
            let mut state = self.lock();
            let mut handles = Vec::new();
            let mut at = 0;
            while at < state.workers.len() {
                if state.workers[at]
                    .handle
                    .as_ref()
                    .is_some_and(|h| h.is_finished())
                {
                    if let Some(handle) = state.workers.remove(at).handle {
                        handles.push(handle);
                    }
                } else {
                    at += 1;
                }
            }
            handles
        };
        for handle in handles {
            let ok = handle.join().is_ok();
            let mut state = self.lock();
            if !ok {
                state.failed = true;
                self.gate.fail();
            }
            match state.finished.checked_add(1) {
                Some(n) => state.finished = n,
                None => {
                    state.failed = true;
                    self.gate.fail();
                }
            }
        }
    }
    pub(crate) fn receipt(&self) -> Receipt {
        let state = self.lock();
        Receipt {
            epoch: self.gate.epoch(),
            outstanding: state.workers.len(),
            coalesced: state.coalesced,
            finished: state.finished,
            failed: state.failed,
        }
    }
    pub(crate) fn join_until(&self, deadline: Instant) -> bool {
        loop {
            self.reap();
            let state = self.lock();
            if state.workers.is_empty() {
                return !state.failed;
            }
            if Instant::now() >= deadline {
                return false;
            }
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10));
            let _wait = self.changed.wait_timeout(state, wait);
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_handles(&self) -> usize {
        self.lock().workers.len()
    }
}
