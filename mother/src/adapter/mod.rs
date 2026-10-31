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
use nexus_governed_control::authority::clock::{Clock, SystemClock};
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::ids::{AgentId, RunId};
use nexus_governed_control::authority::run::RunOrigin;
use nexus_governed_control::broker::Vault;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::{AgentOutcome, GovernedControl, Intent};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    AgentId::new(MOTHER_AGENT).expect("a valid agent name")
}

#[derive(Default)]
struct Runs {
    open: BTreeMap<MissionId, RunId>,
    ended: BTreeSet<MissionId>,
}

/// The EffectPort over the governed-control broker.
pub struct GovernedPort {
    control: Arc<GovernedControl>,
    runs: Mutex<Runs>,
    performed: AtomicUsize,
    cancel_failures: AtomicUsize,
}

impl GovernedPort {
    pub fn new(control: Arc<GovernedControl>) -> Self {
        Self {
            control,
            runs: Mutex::new(Runs::default()),
            performed: AtomicUsize::new(0),
            cancel_failures: AtomicUsize::new(0),
        }
    }

    /// How many effects Mother has handed to this port.
    pub fn performed(&self) -> usize {
        self.performed.load(Ordering::SeqCst)
    }

    /// How many run cancellations the broker refused.
    pub fn cancel_failures(&self) -> usize {
        self.cancel_failures.load(Ordering::SeqCst)
    }

    /// The broker run a mission opened, if any.
    pub fn run_of(&self, mission: MissionId) -> Option<RunId> {
        self.lock().open.get(&mission).copied()
    }

    pub fn control(&self) -> &Arc<GovernedControl> {
        &self.control
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Runs> {
        self.runs.lock().expect("mother port")
    }

    /// The mission's agent run, opened on its first effect. No lock is
    /// held across the broker call; a mission that ended meanwhile gets no
    /// run (the one just opened is cancelled).
    fn run_for(&self, mission: MissionId) -> Result<RunId, EffectRefusal> {
        {
            let runs = self.lock();
            if runs.ended.contains(&mission) {
                return Err(refused("mission_cancelled"));
            }
            if let Some(run) = runs.open.get(&mission) {
                return Ok(*run);
            }
        }
        let run = self
            .control
            .open_run(mother_agent(), RunOrigin::AgentGoal)
            .map_err(|e| refused(e.class()))?;
        let mut runs = self.lock();
        if runs.ended.contains(&mission) || runs.open.contains_key(&mission) {
            drop(runs);
            self.cancel_run(run);
            return Err(refused("mission_cancelled"));
        }
        runs.open.insert(mission, run);
        Ok(run)
    }

    fn cancel_run(&self, run: RunId) {
        if self.control.cancel_run(run).is_err() {
            self.cancel_failures.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Mark the mission ended; its run, if it opened one.
    fn end(&self, mission: MissionId) -> Option<RunId> {
        let mut runs = self.lock();
        runs.ended.insert(mission);
        runs.open.get(&mission).copied()
    }
}

fn refused(class: &str) -> EffectRefusal {
    EffectRefusal {
        class: class.into(),
    }
}

impl EffectPort for GovernedPort {
    fn perform(
        &self,
        mission: MissionId,
        effect: &EffectRequest,
    ) -> Result<EffectOutcome, EffectRefusal> {
        self.performed.fetch_add(1, Ordering::SeqCst);
        let intent = match effect {
            EffectRequest::Network(intent) => Intent::Request(intent.clone()),
            EffectRequest::Process(intent) => Intent::Tool(intent.clone()),
            EffectRequest::ModelTool(intent) => Intent::Connector(intent.clone()),
            // The broker has no file or spend route.
            EffectRequest::File { .. } | EffectRequest::Spend { .. } => {
                return Err(refused("no_broker_route"))
            }
        };
        let run = self.run_for(mission)?;
        match self.control.agent_action(&mother_agent(), run, &intent) {
            Ok(AgentOutcome::Done(output)) => Ok(EffectOutcome::Done { text: output.text }),
            Ok(AgentOutcome::AwaitingApproval(_)) => Ok(EffectOutcome::AwaitingApproval),
            Err(e) => Err(refused(e.class())),
        }
    }

    fn cancel(&self, mission: MissionId) {
        if let Some(run) = self.end(mission) {
            self.cancel_run(run);
        }
    }

    fn finish(&self, mission: MissionId) {
        if let Some(run) = self.end(mission) {
            self.control.finish_when_settled(run);
        }
    }

    fn emergency_stopped(&self) -> bool {
        self.control.authority().runs().is_stopped()
    }
}

/// The journal in a file: one entry per line, appended and synced.
pub struct FileJournal {
    path: PathBuf,
}

fn store_error(e: std::io::Error) -> JournalError {
    JournalError::Store(e.kind().to_string())
}

impl FileJournal {
    /// The journal at `path` (created empty if absent). `path` must be a
    /// regular file, not a link.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if !meta.file_type().is_file() => {
                return Err(JournalError::Store(
                    "the journal is not a regular file".into(),
                ))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                OpenOptions::new()
                    .create_new(true)
                    .append(true)
                    .open(path)
                    .map_err(store_error)?;
            }
            Err(e) => return Err(store_error(e)),
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl JournalStore for FileJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        let text = std::fs::read_to_string(&self.path).map_err(store_error)?;
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        // Every line ends in a newline: the last piece is empty, or torn.
        if !text.ends_with('\n') {
            return Err(JournalError::Malformed { line: lines.len() });
        }
        Ok(lines[..lines.len() - 1].to_vec())
    }

    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if line.contains('\n') {
            return Err(JournalError::Store("a journal line holds a newline".into()));
        }
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.path)
            .map_err(store_error)?;
        file.write_all(format!("{line}\n").as_bytes())
            .map_err(store_error)?;
        file.sync_data().map_err(store_error)
    }
}

/// Create `dir` and any missing directory above it private to this user
/// (0700): the broker refuses a root below a directory others can change.
#[cfg(unix)]
fn create_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
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
        if !dir.is_absolute() {
            return Err(HomeError("the home must be an absolute path".into()));
        }
        create_private(dir).map_err(|e| HomeError(e.to_string()))?;
        let broker = dir.join("broker");
        let evidence = PrimaryEvidence::open(
            &broker.join("p3-evidence").join("evidence.sqlite3"),
            vec![Arc::new(MemoryEvidence::new(256))],
        )
        .map_err(|e| HomeError(format!("evidence store: {}", e.as_str())))?;
        let control = GovernedControl::new(
            &broker,
            Vault::Disabled,
            evidence,
            Arc::new(SystemClock::default()),
        )
        .map_err(|e| HomeError(format!("broker: {e:?}")))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            control: Arc::new(control),
        })
    }

    /// A new home under the system's temporary directory (examples and
    /// tests): refused if it already exists.
    pub fn fresh_temp(label: &str) -> Result<Self, HomeError> {
        let wall = SystemClock::default().wall_ms();
        let dir = std::env::temp_dir().join(format!(
            "nexus-mother-{label}-{}-{wall}",
            std::process::id()
        ));
        if dir.exists() {
            return Err(HomeError("the home already exists".into()));
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
