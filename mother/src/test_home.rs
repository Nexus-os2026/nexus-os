//! M2c-A (N2): the broker Mother's tests build. Mother's production code
//! builds none (the host crate does); this module is compiled for tests
//! only. A `TestHome` has the shape the adapter's home had: the owner's
//! side of the broker is its `control` (never reachable from a port); its
//! ports get their doors from the test helpers, each with the Owner's
//! stated answer.

use crate::adapter::{create_private, FileJournal};
use crate::journal::JournalError;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::GovernedControl;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A broker built as the desktop builds it, and Mother's journal file. One
/// per process: the broker claims the process's emergency-stop controller.
pub(crate) struct TestHome {
    pub dir: PathBuf,
    pub control: Arc<GovernedControl>,
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
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256))],
        )
        .map_err(|e| format!("evidence store: {}", e.as_str()))?;
        let control = GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence,
            Arc::new(SystemClock::default()),
        )
        .map_err(|e| format!("broker: {e:?}"))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            control: Arc::new(control),
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
}
