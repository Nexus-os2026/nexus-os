//! The adapter: the one module of Mother that touches the host.
//!
//! [`GovernedPort`] is the EffectPort over the governed-control broker:
//! each mission acts as the agent `mother` in an agent run of its own, under
//! the Owner's standing grants (an R2 action waits for the Owner's native
//! approval). It never opens an owner run, authorizes, executes, grants,
//! revokes or resumes. The broker has no file or spend route, so those
//! effects are refused here.
//!
//! [`FileJournal`] keeps the mission journal in a file in Mother's home
//! (Mother's own state, not an effect on the world). [`Home`] builds both
//! from a directory the host chooses.

use crate::effect::{EffectOutcome, EffectPort, EffectRefusal, EffectRequest};
use crate::journal::{JournalError, JournalStore, MissionId};
use nexus_governed_control::authority::ids::{AgentId, RunId};
use nexus_governed_control::governed::GovernedControl;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod real_broker;
#[cfg(test)]
mod scan;

/// The agent name Mother acts under in the broker.
pub const MOTHER_AGENT: &str = "mother";

/// Mother's broker identity.
pub fn mother_agent() -> AgentId {
    todo!("M1 implementation follows the oracle commit")
}

#[allow(dead_code)]
#[derive(Default)]
struct Runs {
    open: BTreeMap<MissionId, RunId>,
    ended: BTreeSet<MissionId>,
}

/// The EffectPort over the governed-control broker.
pub struct GovernedPort {
    #[allow(dead_code)]
    control: Arc<GovernedControl>,
    #[allow(dead_code)]
    runs: Mutex<Runs>,
    #[allow(dead_code)]
    performed: AtomicUsize,
}

impl GovernedPort {
    pub fn new(control: Arc<GovernedControl>) -> Self {
        let _ = control;
        todo!("M1 implementation follows the oracle commit")
    }

    /// How many effects Mother has handed to this port.
    pub fn performed(&self) -> usize {
        todo!("M1 implementation follows the oracle commit")
    }

    /// The broker run a mission opened, if any.
    pub fn run_of(&self, mission: MissionId) -> Option<RunId> {
        let _ = mission;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn control(&self) -> &Arc<GovernedControl> {
        &self.control
    }
}

impl EffectPort for GovernedPort {
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        let _ = (mission, effect);
        todo!("M1 implementation follows the oracle commit")
    }

    fn cancel(&self, mission: MissionId) {
        let _ = mission;
        todo!("M1 implementation follows the oracle commit")
    }

    fn finish(&self, mission: MissionId) {
        let _ = mission;
        todo!("M1 implementation follows the oracle commit")
    }

    fn emergency_stopped(&self) -> bool {
        todo!("M1 implementation follows the oracle commit")
    }
}

/// The journal in a file: one entry per line, appended and synced.
pub struct FileJournal {
    #[allow(dead_code)]
    path: PathBuf,
}

impl FileJournal {
    /// The journal at `path` (created empty if absent). `path` must be a
    /// regular file, not a link.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        let _ = path;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl JournalStore for FileJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        todo!("M1 implementation follows the oracle commit")
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let _ = line;
        todo!("M1 implementation follows the oracle commit")
    }
}

/// Mother's home: the broker built as the desktop builds it, and the
/// journal file. One per process: the broker claims the process's
/// emergency-stop controller.
pub struct Home {
    pub dir: PathBuf,
    pub control: Arc<GovernedControl>,
}

/// Why a home cannot be opened.
#[derive(Debug)]
pub struct HomeError(pub String);

impl Home {
    /// Open (or create) the home at `dir`, an absolute path the host
    /// chose.
    pub fn open(dir: &Path) -> Result<Self, HomeError> {
        let _ = dir;
        todo!("M1 implementation follows the oracle commit")
    }

    /// A new home under the system's temporary directory (examples).
    pub fn fresh_temp(label: &str) -> Result<Self, HomeError> {
        let _ = label;
        todo!("M1 implementation follows the oracle commit")
    }

    pub fn journal_path(&self) -> PathBuf {
        self.dir.join("mother-journal.jsonl")
    }

    pub fn journal(&self) -> Result<FileJournal, JournalError> {
        FileJournal::open(&self.journal_path())
    }
}
