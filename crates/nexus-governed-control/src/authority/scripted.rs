//! A scripted evidence sink for the lock and ordering tests (tests only).
//!
//! It can hold one record until released (a slow sink), fail or panic on a
//! phase, call back into the authority on the recording thread (a
//! re-entrant sink), and probe from another thread, while each record is
//! written, that no authority lock is held: a lock held by the recording
//! thread blocks the probe until the record returns, and the probe gives
//! up after `PROBE_BOUND`, counted as a violation.

use super::evidence::{
    EvidencePhase, EvidenceRecord, EvidenceSink, EvidenceUnavailable, MemoryEvidence,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a probe may wait for the authority's locks.
pub(crate) const PROBE_BOUND: Duration = Duration::from_secs(2);
/// The longest a held record waits for its release (a test that forgets to
/// release fails rather than hangs).
const HOLD_BOUND: Duration = Duration::from_secs(30);

type Probe = Arc<dyn Fn() + Send + Sync>;
type Reenter = Arc<dyn Fn(&EvidenceRecord) + Send + Sync>;

#[derive(Default)]
struct Script {
    hold: Option<EvidencePhase>,
    fail: Option<EvidencePhase>,
    panic: Option<EvidencePhase>,
    probe: Option<Probe>,
    reenter: Option<Reenter>,
}

#[derive(Default)]
struct Gate {
    held: bool,
    released: bool,
}

pub(crate) struct ScriptedSink {
    pub(crate) memory: MemoryEvidence,
    script: Mutex<Script>,
    gate: Mutex<Gate>,
    moved: Condvar,
    pub(crate) probes: AtomicUsize,
    pub(crate) violations: AtomicUsize,
}

impl ScriptedSink {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            memory: MemoryEvidence::new(100_000),
            script: Mutex::new(Script::default()),
            gate: Mutex::new(Gate::default()),
            moved: Condvar::new(),
            probes: AtomicUsize::new(0),
            violations: AtomicUsize::new(0),
        })
    }

    /// Hold the next record of `phase` until `release`.
    pub(crate) fn hold_next(&self, phase: EvidencePhase) {
        *self.gate.lock().unwrap() = Gate::default();
        self.script.lock().unwrap().hold = Some(phase);
    }

    /// Wait (bounded) until the held record is being written.
    pub(crate) fn wait_held(&self) {
        let deadline = Instant::now() + HOLD_BOUND;
        let mut gate = self.gate.lock().unwrap();
        while !gate.held {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the record was never held");
            gate = self.moved.wait_timeout(gate, left).unwrap().0;
        }
    }

    pub(crate) fn release(&self) {
        self.gate.lock().unwrap().released = true;
        self.moved.notify_all();
    }

    pub(crate) fn fail_on(&self, phase: Option<EvidencePhase>) {
        self.script.lock().unwrap().fail = phase;
    }

    pub(crate) fn panic_on(&self, phase: Option<EvidencePhase>) {
        self.script.lock().unwrap().panic = phase;
    }

    /// While every record is written, run `probe` on another thread and
    /// count a violation if it does not finish within `PROBE_BOUND`.
    pub(crate) fn probe_with(&self, probe: impl Fn() + Send + Sync + 'static) {
        self.script.lock().unwrap().probe = Some(Arc::new(probe));
    }

    /// While every record is written, run `reenter` on the recording thread.
    pub(crate) fn reenter_with(&self, reenter: impl Fn(&EvidenceRecord) + Send + Sync + 'static) {
        self.script.lock().unwrap().reenter = Some(Arc::new(reenter));
    }

    pub(crate) fn violations(&self) -> usize {
        self.violations.load(Ordering::SeqCst)
    }

    pub(crate) fn probes(&self) -> usize {
        self.probes.load(Ordering::SeqCst)
    }

    pub(crate) fn phases(&self) -> Vec<EvidencePhase> {
        self.memory.records().iter().map(|r| r.phase).collect()
    }
}

impl EvidenceSink for ScriptedSink {
    fn record(&self, record: &EvidenceRecord) -> Result<(), EvidenceUnavailable> {
        let (hold, fail, panic, probe, reenter) = {
            let mut script = self.script.lock().unwrap();
            let hold = script.hold == Some(record.phase);
            if hold {
                script.hold = None;
            }
            (
                hold,
                script.fail == Some(record.phase),
                script.panic == Some(record.phase),
                script.probe.clone(),
                script.reenter.clone(),
            )
        };
        if let Some(probe) = probe {
            self.probes.fetch_add(1, Ordering::SeqCst);
            let (done, finished) = mpsc::channel();
            std::thread::spawn(move || {
                probe();
                let _ = done.send(());
            });
            if finished.recv_timeout(PROBE_BOUND).is_err() {
                self.violations.fetch_add(1, Ordering::SeqCst);
            }
        }
        if let Some(reenter) = reenter {
            reenter(record);
        }
        if hold {
            let deadline = Instant::now() + HOLD_BOUND;
            let mut gate = self.gate.lock().unwrap();
            gate.held = true;
            self.moved.notify_all();
            while !gate.released && Instant::now() < deadline {
                let left = deadline.saturating_duration_since(Instant::now());
                gate = self.moved.wait_timeout(gate, left).unwrap().0;
            }
        }
        if panic {
            panic!("a sink that panics (test)");
        }
        if fail {
            return Err(EvidenceUnavailable);
        }
        self.memory.record(record)
    }
}

/// Run `work` on another thread and wait for it within a bound: a lock left
/// held (across a record, a launch, a callback) would block it.
pub(crate) fn within<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(work());
    });
    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("the operation waited on a lock held across external work")
}
