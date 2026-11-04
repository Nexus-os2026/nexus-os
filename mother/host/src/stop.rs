//! Memory-only host admission and separately owned synchronous Stop workers.
use nexus_governed_control::authority::file_session::HaltCell;
use nexus_governed_control::governed::GovernedControl;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
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
    pub(crate) port: Arc<nexus_mother::adapter::AdmissionGate>,
    backend: OnceLock<HaltCell>,
    recovery: AtomicBool,
    closing: AtomicBool,
    failed: AtomicBool,
    epoch: AtomicU64,
    admitted: [AtomicU64; 7],
}
impl Gate {
    pub(crate) fn admit(&self, route: Route) -> bool {
        if self.backend_halted()
            || self.port.is_closed()
            || self.recovery()
            || self.closing()
            || self.failed()
        {
            return false;
        }
        // Counts successful host admission boundaries, not backend success.
        // A dispatch admitted before emergency may finish after it; the
        // backend's native final checks still govern publication.
        if self.admitted[route as usize]
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            .is_err()
        {
            self.fail();
            return false;
        }
        true
    }
    pub(crate) fn backend_halted(&self) -> bool {
        self.backend.get().is_some_and(HaltCell::is_engaged)
    }
    fn engage_backend(&self) {
        if let Some(cell) = self.backend.get() {
            cell.engage_emergency();
        }
    }
    fn bind_backend(&self, cell: HaltCell) -> bool {
        if self.backend.set(cell).is_err() {
            self.fail();
            return false;
        }
        // No admission is offered before bind returns. Together with the
        // second read in emergency(), this covers ingress during startup bind.
        if self.recovery() {
            self.engage_backend();
        }
        true
    }
    pub(crate) fn admitted(&self) -> [u64; 7] {
        std::array::from_fn(|i| self.admitted[i].load(Ordering::SeqCst))
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
        self.port.close();
        self.closing.store(true, Ordering::SeqCst);
    }
    pub(crate) fn fail(&self) {
        self.failed.store(true, Ordering::SeqCst);
        self.close();
    }
    pub(crate) fn emergency(&self) {
        // The backend is the authority; the host flags only mirror it.
        // This atomic ingress precedes every lock and worker reservation.
        self.engage_backend();
        self.port.close();
        self.recovery.store(true, Ordering::SeqCst);
        // An ingress before/during bind must reach the newly installed cell.
        self.engage_backend();
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
    handle: Option<JoinHandle<bool>>,
}
#[derive(Default)]
struct State {
    next: u64,
    workers: Vec<Worker>,
    coalesced: u64,
    finished: u64,
    failed_workers: u64,
    failed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub epoch: u64,
    pub outstanding: usize,
    pub coalesced: u64,
    pub finished: u64,
    pub failed_workers: u64,
    pub failed: bool,
}
type Body = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct StopRoute {
    pub(crate) gate: Arc<Gate>,
    body: Mutex<Option<Body>>,
    bound: AtomicBool,
    state: Mutex<State>,
    changed: Condvar,
    #[cfg(test)]
    pub(crate) hold_dispatch: AtomicBool,
}
impl StopRoute {
    pub(crate) fn new(gate: Arc<Gate>) -> Arc<Self> {
        Arc::new(Self {
            gate,
            body: Mutex::new(None),
            bound: AtomicBool::new(false),
            state: Mutex::default(),
            changed: Condvar::new(),
            #[cfg(test)]
            hold_dispatch: AtomicBool::new(false),
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
        if !self.gate.bind_backend(owner.file_session_halt()) {
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
    pub(crate) fn dispatch_pending(self: &Arc<Self>) {
        #[cfg(test)]
        if self.hold_dispatch.load(Ordering::SeqCst) {
            return;
        }
        let body = match self.body.lock() {
            Ok(body) => body.clone(),
            Err(_) => {
                self.gate.fail();
                self.lock().failed = true;
                return;
            }
        };
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
                    ok
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
                    state.failed_workers = state.failed_workers.saturating_add(1);
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
        match self.body.lock() {
            Ok(mut body) => body.take(),
            Err(p) => {
                self.gate.fail();
                p.into_inner().take()
            }
        }
    }
    pub(crate) fn reap(&self) {
        // Finished handles join without waiting for work. Retain the small
        // memory hold through accounting so no observer sees an empty worker
        // table before its last failure/completion is accounted for.
        let mut state = self.lock();
        let mut at = 0;
        while at < state.workers.len() {
            if !state.workers[at]
                .handle
                .as_ref()
                .is_some_and(|h| h.is_finished())
            {
                at += 1;
                continue;
            }
            let worker = state.workers.remove(at);
            let ok = worker.handle.is_some_and(|h| matches!(h.join(), Ok(true)));
            if !ok {
                state.failed = true;
                self.gate.fail();
            }
            let count = if ok {
                &mut state.finished
            } else {
                &mut state.failed_workers
            };
            match count.checked_add(1) {
                Some(n) => *count = n,
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
            failed_workers: state.failed_workers,
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

/// Owns the receipt cursor from process start. Cumulative counters are replayed
/// individually, never sampled away when startup completes or events coalesce.
pub(crate) fn report_receipts(screen: Arc<crate::terminal::Shared>, end: Arc<AtomicBool>) {
    let mut last = Receipt {
        epoch: 0,
        outstanding: 0,
        coalesced: 0,
        finished: 0,
        failed_workers: 0,
        failed: false,
    };
    loop {
        screen.stop.reap();
        let receipt = screen.stop.receipt();
        for previous in last.epoch..receipt.epoch {
            let epoch = previous + 1;
            screen.emit(&format!(
                "STOP REQUEST RECEIVED epoch={epoch}\nRECOVERY ONLY\n"
            ));
        }
        for _ in last.coalesced..receipt.coalesced {
            screen.emit("STOP COALESCED\n");
        }
        for previous in last.finished..receipt.finished {
            let count = previous + 1;
            screen.emit(&format!("STOP RECORDING COMPLETED count={count} (durability requires evidence health)\nSTOP WORKERS outstanding={}\n", receipt.outstanding));
        }
        for previous in last.failed_workers..receipt.failed_workers {
            let count = previous + 1;
            screen.emit(&format!(
                "STOP RECORDING FAILED failed_workers={count} outstanding={}\n",
                receipt.outstanding
            ));
        }
        if receipt.failed && !last.failed && receipt.failed_workers == 0 {
            screen.emit("STOP ROUTE FAILED\n");
        }
        last = receipt;
        if end.load(Ordering::SeqCst) {
            break;
        }
        screen.wait();
    }
}
