//! Dedicated lite fixture; earlier-PR homes and tests stay byte-identical.
use super::{create_private, FileJournal, FilePrivate};
use crate::test_home::GatedProjection;
use nexus_governed_control::authority::clock::{Clock, SystemClock};
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct LiteHome {
    pub dir: PathBuf,
    pub control: Arc<GovernedControl>,
    pub gate: Arc<GatedProjection>,
    pub clock: Arc<TestClock>,
    pub evidence: Arc<PrimaryEvidence>,
}
impl LiteHome {
    pub fn open(dir: &Path) -> Result<Self, String> {
        assert!(nexus_governed_control::files::make_non_dumpable());
        create_private(dir).map_err(|e| e.to_string())?;
        let broker = dir.join("broker");
        let gate = Arc::new(GatedProjection::default());
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence/evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256)), gate.clone()],
        )
        .map_err(|e| e.as_str().to_string())?;
        let clock = Arc::new(TestClock::default());
        let control = Arc::new(
            GovernedControl::new(&broker, Vault::Disabled, evidence.clone(), clock.clone())
                .map_err(|e| format!("{e:?}"))?,
        );
        Ok(Self {
            dir: dir.to_path_buf(),
            control,
            gate,
            clock,
            evidence,
        })
    }
    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }
    pub fn journal(&self) -> Result<FileJournal, crate::journal::JournalError> {
        FileJournal::open(&self.journal_path())
    }
    pub fn private(&self) -> FilePrivate {
        FilePrivate::open(&self.dir.join("private")).unwrap()
    }
}

/// Clock boundary after the kernel's apply.prepared record, before publication.
pub(crate) fn past_the_start(
    home: &LiteHome,
    run: nexus_governed_control::authority::ids::RunId,
) -> impl Fn() -> bool + Send + 'static {
    let (gate, run) = (home.gate.clone(), run.to_string());
    let files = home.dir.join("broker/files");
    let since = Mutex::new(None);
    move || {
        if !gate
            .seen()
            .iter()
            .any(|s| s.phase == "started" && s.run.as_deref() == Some(run.as_str()))
        {
            return false;
        }
        let now: Vec<_> = ["ledger.db", "ledger.db-wal"]
            .iter()
            .map(|name| {
                std::fs::metadata(files.join(name))
                    .ok()
                    .map(|m| (m.len(), m.modified().ok()))
            })
            .collect();
        let mut since = since.lock().unwrap();
        match since.as_ref() {
            None => {
                *since = Some(now);
                false
            }
            Some(before) => *before != now,
        }
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
