//! M2c-A (N2): the broker Mother's tests build. Mother's production code
//! builds none (the host crate does); this module is compiled for tests
//! only. A `TestHome` has the shape the adapter's home had: the owner's
//! side of the broker is its `control` (never reachable from a port); its
//! ports get their doors from the test helpers, each with the Owner's
//! stated answer.

use crate::adapter::{create_private, FileJournal, FilePrivate};
use crate::journal::JournalError;
use nexus_governed_control::authority::clock::{Clock, SystemClock};
use nexus_governed_control::authority::evidence::{
    EvidenceProjection, EvidenceRecord, MemoryEvidence,
};
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

/// A broker built as the desktop builds it, and Mother's journal file. One
/// per process: the broker claims the process's emergency-stop controller.
pub(crate) struct TestHome {
    pub dir: PathBuf,
    pub control: Arc<GovernedControl>,
    /// M2c-B: a projection of every record the broker writes, inert until
    /// a test arms it (it then holds the writer of a matching record).
    pub gate: Arc<GatedProjection>,
    /// M2d-C: the broker's clock, the system's until a test moves it or
    /// arms it.
    pub clock: Arc<TestClock>,
}

static NEXT: AtomicU64 = AtomicU64::new(0);

impl TestHome {
    /// Open (or create) the home at `dir`, an absolute path.
    pub fn open(dir: &Path) -> Result<Self, String> {
        if !dir.is_absolute() {
            return Err("the home must be an absolute path".into());
        }
        create_private(dir).map_err(|e| e.to_string())?;
        let broker = dir.join("broker");
        let gate = Arc::new(GatedProjection::default());
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256)), gate.clone()],
        )
        .map_err(|e| format!("evidence store: {}", e.as_str()))?;
        let clock = Arc::new(TestClock::default());
        let control = GovernedControl::new(&broker, Vault::Disabled, evidence, clock.clone())
            .map_err(|e| format!("broker: {e:?}"))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            control: Arc::new(control),
            gate,
            clock,
        })
    }

    /// A new home under the system's temporary directory: refused if it
    /// already exists.
    pub fn fresh_temp(label: &str) -> Result<Self, String> {
        // Wall time in nanoseconds and a counter: unique enough across
        // processes without asking the host for this process's id.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "nexus-mother-test-{label}-{nanos}-{}",
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        if dir.exists() {
            return Err("the home already exists".into());
        }
        Self::open(&dir)
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }

    pub fn journal(&self) -> Result<FileJournal, JournalError> {
        FileJournal::open(&self.journal_path())
    }

    /// P0: Mother's private store in this home.
    pub fn private(&self) -> FilePrivate {
        FilePrivate::open(&self.dir.join("private")).expect("the private store opens")
    }
}

type Due = (Box<dyn Fn() -> bool + Send>, Box<dyn FnOnce() + Send>);

/// M2d-C: the system's clocks, moved forward by `advance` (both at once,
/// and never back), with one armed action: the first reading taken once
/// its condition holds runs it, on the reading thread, before the reading
/// returns. Mother's tests cannot reach the broker's own hooks (they are
/// per crate): a reading the broker takes at a step is where they act.
#[derive(Default)]
pub(crate) struct TestClock {
    system: SystemClock,
    offset_ms: AtomicU64,
    due: Mutex<Option<Due>>,
}

impl TestClock {
    /// Move both clocks forward by `by`.
    pub fn advance(&self, by: Duration) {
        let by = u64::try_from(by.as_millis()).unwrap_or(u64::MAX);
        self.offset_ms.fetch_add(by, Ordering::SeqCst);
    }

    /// Run `then` at the first reading taken once `when` holds.
    pub fn arm(
        &self,
        when: impl Fn() -> bool + Send + 'static,
        then: impl FnOnce() + Send + 'static,
    ) {
        *self.due.lock().unwrap() = Some((Box::new(when), Box::new(then)));
    }

    /// Whether the armed action is still waiting (it has not run).
    pub fn armed(&self) -> bool {
        self.due.lock().unwrap().is_some()
    }

    /// Disarm, and say whether the action had still to run.
    pub fn disarm(&self) -> bool {
        self.due.lock().unwrap().take().is_some()
    }

    fn reading(&self) -> u64 {
        let due = {
            let mut due = self.due.lock().unwrap();
            if due.as_ref().is_some_and(|(when, _)| when()) {
                due.take()
            } else {
                None
            }
        };
        if let Some((_, then)) = due {
            then();
        }
        self.offset_ms.load(Ordering::SeqCst)
    }
}

impl Clock for TestClock {
    fn monotonic_ms(&self) -> u64 {
        let offset = self.reading();
        self.system.monotonic_ms().saturating_add(offset)
    }

    fn wall_ms(&self) -> u64 {
        let offset = self.reading();
        self.system.wall_ms().saturating_add(offset)
    }
}

/// The longest a gated writer is held (a safety bound: tests release it by
/// opening the gate, never by this). Longer than the tests' bounded joins,
/// so that a call waiting on a held writer is seen to wait.
const HOLD: Duration = Duration::from_secs(30);

/// One record the broker wrote, as the projection saw it: its phase (by
/// name), its run, and the thread that wrote it.
#[derive(Clone, Debug)]
pub(crate) struct Seen {
    pub phase: String,
    pub run: Option<String>,
    pub thread: ThreadId,
}

type OnHold = Box<dyn FnOnce() + Send>;

struct Armed {
    phase: &'static str,
    run: Option<String>,
    on_hold: Option<OnHold>,
}

#[derive(Default)]
struct GateState {
    seen: Vec<Seen>,
    armed: Option<Armed>,
    held: usize,
    open: bool,
}

/// M2c-B: a projection that sees every record (after its commit, on the
/// writer's thread, with no store lock held) and, once armed, holds the
/// writer of each matching record until the test opens the gate. Mother's
/// tests cannot use the broker's own test hooks (they are per crate): this
/// is how they stall a broker write.
#[derive(Default)]
pub(crate) struct GatedProjection {
    state: Mutex<GateState>,
    changed: Condvar,
}

impl GatedProjection {
    /// Hold every writer of a `phase` record (of `run`, if given) until
    /// [`GatedProjection::open`].
    pub fn arm(&self, phase: &'static str, run: Option<RunId>) {
        self.arm_inner(phase, run, None);
    }

    /// As [`GatedProjection::arm`], and run `on_hold` once, on the writer's
    /// thread, when the first matching record arrives (before it is held).
    pub fn arm_with(
        &self,
        phase: &'static str,
        run: Option<RunId>,
        on_hold: impl FnOnce() + Send + 'static,
    ) {
        self.arm_inner(phase, run, Some(Box::new(on_hold)));
    }

    fn arm_inner(&self, phase: &'static str, run: Option<RunId>, on_hold: Option<OnHold>) {
        let mut state = self.state.lock().unwrap();
        state.armed = Some(Armed {
            phase,
            run: run.map(|r| r.to_string()),
            on_hold,
        });
        state.open = false;
        state.held = 0;
    }

    /// Whether a writer is held at the gate now (waits for one, bounded).
    pub fn wait_held(&self) -> bool {
        let state = self.state.lock().unwrap();
        let (state, _) = self
            .changed
            .wait_timeout_while(state, HOLD, |s| s.held == 0)
            .unwrap();
        state.held > 0
    }

    /// Release every held writer and disarm.
    pub fn open(&self) {
        let mut state = self.state.lock().unwrap();
        state.open = true;
        state.armed = None;
        self.changed.notify_all();
    }

    /// Every record seen so far.
    pub fn seen(&self) -> Vec<Seen> {
        self.state.lock().unwrap().seen.clone()
    }

    /// How many records of `phase` were seen.
    pub fn count(&self, phase: &str) -> usize {
        self.seen().iter().filter(|s| s.phase == phase).count()
    }
}

impl EvidenceProjection for GatedProjection {
    fn project(&self, record: &EvidenceRecord) {
        let phase = record.phase.as_str();
        let mut state = self.state.lock().unwrap();
        state.seen.push(Seen {
            phase: phase.to_string(),
            run: record.run.clone(),
            thread: std::thread::current().id(),
        });
        let matches = state.armed.as_ref().is_some_and(|armed| {
            armed.phase == phase && (armed.run.is_none() || armed.run == record.run)
        });
        if !matches || state.open {
            return;
        }
        let on_hold = state.armed.as_mut().and_then(|armed| armed.on_hold.take());
        if let Some(on_hold) = on_hold {
            drop(state);
            on_hold();
            state = self.state.lock().unwrap();
        }
        state.held += 1;
        self.changed.notify_all();
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, HOLD, |s| !s.open)
            .unwrap();
        state.held -= 1;
    }
}
