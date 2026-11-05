//! The reader only engages the original ingress. The reporter owns dispatch,
//! bounded native workers, exact event receipts, and acknowledged delivery.
use nexus_governed_control::host_session::{
    close::{Producer, ProducerKind},
    stop::StopPhase,
    HostSession, SessionIngress,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

struct Worker {
    generation: u64,
    handle: JoinHandle<bool>,
    producer: Producer,
}
#[derive(Default)]
struct State {
    workers: Vec<Worker>,
    deferred: Vec<u64>,
    finished: u64,
    failed_workers: u64,
    failed: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Receipt {
    pub epoch: u64,
    pub outstanding: usize,
    pub coalesced: u64,
    pub finished: u64,
    pub failed_workers: u64,
    pub failed: bool,
}
pub(crate) struct StopRoute {
    pub(crate) ingress: SessionIngress,
    session: OnceLock<Arc<HostSession>>,
    state: Mutex<State>,
    #[cfg(test)]
    pub(crate) hold_dispatch: AtomicBool,
}
impl StopRoute {
    pub(crate) fn new(ingress: SessionIngress) -> Arc<Self> {
        Arc::new(Self {
            ingress,
            session: OnceLock::new(),
            state: Mutex::default(),
            #[cfg(test)]
            hold_dispatch: AtomicBool::new(false),
        })
    }
    pub(crate) fn bind(&self, session: Arc<HostSession>) -> bool {
        if self.session.set(session).is_err() {
            self.ingress.fail();
            return false;
        }
        true
    }
    pub(crate) fn press(&self) {
        // No broker, metadata hold, spawn, rendering, or I/O on the reader.
        let _event = self.ingress.emergency();
    }
    pub(crate) fn dispatch_pending(&self) {
        #[cfg(test)]
        if self.hold_dispatch.load(Ordering::SeqCst) {
            return;
        }
        let Some(session) = self.session.get() else {
            return;
        };
        let Ok(events) = session.stop_events() else {
            self.ingress.fail();
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            self.ingress.fail();
            return;
        };
        for observed in events {
            if observed.phase != StopPhase::Pending
                || state
                    .workers
                    .iter()
                    .any(|w| w.generation == observed.event.generation())
            {
                continue;
            }
            if state.workers.len() == 4 {
                if !state.deferred.contains(&observed.event.generation()) {
                    state.deferred.push(observed.event.generation());
                }
                continue;
            }
            let producer = match self.ingress.producer(ProducerKind::StopWorker) {
                Ok(p) => p,
                Err(_) => {
                    self.ingress.fail();
                    state.failed = true;
                    return;
                }
            };
            let owner = session.clone();
            let event = observed.event;
            match std::thread::Builder::new()
                .name("hands-stop".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        owner.record_stop(&event)
                    }));
                    let ok = matches!(result, Ok(Ok(_)));
                    if !ok {
                        owner.ingress().fail();
                    }
                    ok
                }) {
                Ok(handle) => state.workers.push(Worker {
                    generation: event.generation(),
                    handle,
                    producer,
                }),
                Err(_) => {
                    let _failed = session.stop_spawn_failed(&event);
                    state.failed = true;
                    state.failed_workers += 1;
                    drop(producer);
                }
            }
        }
    }
    pub(crate) fn reap(&self) {
        let Ok(mut state) = self.state.lock() else {
            self.ingress.fail();
            return;
        };
        let mut at = 0;
        while at < state.workers.len() {
            if !state.workers[at].handle.is_finished() {
                at += 1;
                continue;
            }
            let worker = state.workers.remove(at);
            let ok = matches!(worker.handle.join(), Ok(true));
            worker.producer.joined();
            if ok {
                state.finished += 1;
            } else {
                state.failed_workers += 1;
                state.failed = true;
                self.ingress.fail();
            }
        }
    }
    pub(crate) fn receipt(&self) -> Receipt {
        let epoch = self.ingress.generation();
        match self.state.lock() {
            Ok(state) => Receipt {
                epoch,
                outstanding: epoch.saturating_sub(state.finished + state.failed_workers) as usize,
                coalesced: state.deferred.len() as u64,
                finished: state.finished,
                failed_workers: state.failed_workers,
                failed: state.failed,
            },
            Err(_) => {
                self.ingress.fail();
                Receipt {
                    epoch,
                    failed: true,
                    outstanding: epoch as usize,
                    ..Receipt::default()
                }
            }
        }
    }
    pub(crate) fn join_until(&self, deadline: Instant) -> bool {
        loop {
            self.reap();
            self.dispatch_pending();
            let receipt = self.receipt();
            if receipt.outstanding == 0 {
                return !receipt.failed;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_handles(&self) -> usize {
        self.state.lock().unwrap().workers.len()
    }
}

pub(crate) fn report_receipts(screen: Arc<crate::terminal::Shared>, end: Arc<AtomicBool>) {
    use crate::terminal::Failure;
    use std::sync::mpsc;
    let mut received = 0;
    let mut snapshot = None;
    let mut coalesced = 0;
    let mut outcomes = [false; 256];
    let mut deliveries = Vec::<mpsc::Receiver<Result<(), Failure>>>::new();
    loop {
        screen.stop.reap();
        screen.stop.dispatch_pending();
        let receipt = screen.stop.receipt();
        let mut frames = Vec::new();
        while received < receipt.epoch {
            received += 1;
            frames.push(format!(
                "STOP REQUEST RECEIVED epoch={received}\n{}\nRECOVERY ONLY\n",
                crate::terminal::EMERGENCY_NOTICE
            ));
        }
        while coalesced < receipt.coalesced {
            coalesced += 1;
            frames.push("STOP COALESCED; recording remains pending\n".into());
        }
        if let Some(session) = screen.stop.session.get() {
            let current = session.stop_snapshot();
            if snapshot != Some(current) && (current.kernel_stopped || current.broker_stopped) {
                frames.push(format!(
                    "kernel_stopped={} broker_stopped={} stop_number={} generation={}\n",
                    current.kernel_stopped,
                    current.broker_stopped,
                    current.stop_number,
                    current.policy_generation
                ));
            }
            snapshot = Some(current);
            match session.stop_events() {
                Ok(events) => {
                    for observed in events {
                        let at = observed.event.generation().saturating_sub(1) as usize;
                        if at >= outcomes.len() {
                            screen.fail(Failure::Capacity);
                            break;
                        }
                        if outcomes[at] {
                            continue;
                        }
                        let text = match observed.phase {
                        StopPhase::Finished(result) => format!("kernel_stopped={} broker_stopped={} stop_number={} generation={}\nSTOP RECORDING COMPLETED count={} (synchronous phase returned)\nSTOP RECORD durability={:?} owed={} uncertain={}\nSTOP WORKERS outstanding={}\n", result.snapshot.kernel_stopped, result.snapshot.broker_stopped, result.native_stop_number, result.snapshot.policy_generation, observed.event.generation(), result.record, result.consequences.owed, result.consequences.uncertain, receipt.outstanding),
                        StopPhase::Failed | StopPhase::Uncertain => format!("STOP RECORDING FAILED event={} outstanding={}\n", observed.event.generation(), receipt.outstanding),
                        _ => continue,
                    };
                        outcomes[at] = true;
                        frames.push(text);
                    }
                }
                Err(_) => screen.fail(Failure::Input),
            }
        }
        for text in frames {
            match screen.send(text) {
                Ok(ack) => deliveries.push(ack),
                Err(_) => return, // fatal delivery debt remains in Shared
            }
        }
        let mut at = 0;
        while at < deliveries.len() {
            match deliveries[at].try_recv() {
                Ok(Ok(())) => {
                    deliveries.swap_remove(at);
                }
                Err(mpsc::TryRecvError::Empty) => at += 1,
                _ => {
                    screen.fail(Failure::Output);
                    return;
                }
            }
        }
        if deliveries.len() > 64 {
            screen.fail(Failure::Capacity);
            return;
        }
        if end.load(Ordering::SeqCst) && deliveries.is_empty() {
            break;
        }
        if screen
            .stop
            .ingress
            .close_deadline()
            .is_some_and(|d| Instant::now() >= d)
        {
            if !deliveries.is_empty() {
                screen.fail(Failure::Output);
            }
            break;
        }
        screen.wait();
    }
}
