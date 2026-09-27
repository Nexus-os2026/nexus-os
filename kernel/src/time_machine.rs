//! Time Machine — system-wide undo/redo engine for Nexus OS.
//!
//! Captures reversible operations at checkpoint boundaries.  Each checkpoint
//! records the before/after state of every change so the system can roll back
//! file writes, agent state mutations, and config edits.
//!
//! P0-002C5B: a recorded path is never filesystem authority. A file change is
//! recorded only under a live [`WorkspaceGrant`] for an authenticated agent and
//! run, as a relative path beneath the grant's root together with the root's
//! identity. Replay re-resolves that grant against the live registry for the
//! same agent and run, re-checks the root identity and each file's expected
//! identity and content, and refuses before any mutation when anything
//! differs. Without a [`FileAuthority`], a checkpoint that records a file
//! change cannot be replayed at all; production replay holds none. Pathname
//! checks are not OS isolation: a same-user process racing the namespace
//! between check and use is out of scope.

use crate::governed_path::{self, FileIdentity};
use crate::manifest::FsPermissionLevel;
use crate::workspace_authority::{
    WorkspaceAuthorityRegistry, WorkspaceBinding, WorkspaceGrant, WorkspaceGrantId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TimeMachineError {
    CheckpointNotFound(String),
    UndoFailed(String),
    RedoFailed(String),
    Io(String),
    CapacityExceeded(usize),
    EmptyHistory,
    /// The checkpoint records a file change and no backend file authority
    /// was supplied, so nothing was applied.
    FileAuthorityRequired,
    /// A recorded file change could not be authorized or its target is not in
    /// the expected state, so nothing was applied.
    FileDenied {
        change: usize,
        reason: &'static str,
    },
    /// File changes were applied up to `applied` of `total` before `reason`
    /// stopped the replay. The checkpoint was not marked as replayed.
    PartiallyApplied {
        applied: usize,
        total: usize,
        reason: &'static str,
    },
}

impl fmt::Display for TimeMachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckpointNotFound(id) => write!(f, "checkpoint not found: {id}"),
            Self::UndoFailed(msg) => write!(f, "undo failed: {msg}"),
            Self::RedoFailed(msg) => write!(f, "redo failed: {msg}"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::CapacityExceeded(n) => write!(f, "capacity exceeded: {n} checkpoints"),
            Self::EmptyHistory => write!(f, "no checkpoints to undo"),
            Self::FileAuthorityRequired => write!(
                f,
                "file changes need backend workspace authority, which is not available; nothing was applied"
            ),
            Self::FileDenied { change, reason } => {
                write!(f, "file change {change} denied: {reason}; nothing was applied")
            }
            Self::PartiallyApplied {
                applied,
                total,
                reason,
            } => write!(
                f,
                "file replay stopped after {applied} of {total} changes: {reason}; the checkpoint was not marked as replayed"
            ),
        }
    }
}

impl std::error::Error for TimeMachineError {}

// ---------------------------------------------------------------------------
// File authority
// ---------------------------------------------------------------------------

/// Backend authority for recording or replaying file changes: the live grant
/// registry and the agent/run binding the backend took from trusted execution
/// state. It is never deserialized, and a checkpoint cannot carry one.
pub struct FileAuthority<'a> {
    registry: &'a WorkspaceAuthorityRegistry,
    binding: WorkspaceBinding,
}

impl<'a> FileAuthority<'a> {
    pub fn new(registry: &'a WorkspaceAuthorityRegistry, binding: WorkspaceBinding) -> Self {
        Self { registry, binding }
    }

    /// Resolves the grant for this binding and requires write permission and
    /// a root that is still an existing canonical directory.
    fn writable_grant(&self, grant: WorkspaceGrantId) -> Result<WorkspaceGrant, &'static str> {
        let grant = self
            .registry
            .resolve(grant, self.binding)
            .map_err(|_| "workspace grant is not live for this agent and run")?;
        if *grant.permission() != FsPermissionLevel::ReadWrite {
            return Err("workspace grant does not permit writes");
        }
        governed_path::existing_root(grant.root()).map_err(|_| "workspace root changed")?;
        Ok(grant)
    }

    /// The absolute target of a recorded file, after re-resolving its grant
    /// for the same agent and run and re-checking the root identity.
    fn target(&self, file: &GrantedFile) -> Result<(PathBuf, PathBuf), &'static str> {
        if file.agent_id != self.binding.agent_id || file.run_id != self.binding.run_id {
            return Err("recorded agent or run does not match");
        }
        let grant = self.writable_grant(file.grant)?;
        if governed_path::identity_of(grant.root()) != Ok(file.root) {
            return Err("workspace root changed");
        }
        let target = governed_path::join_relative(grant.root(), &file.relative)
            .map_err(|_| "recorded path is not a valid relative path")?;
        Ok((grant.root().to_path_buf(), target))
    }
}

/// A recorded file target: backend facts captured when the change was
/// recorded. None of it is authority on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantedFile {
    pub grant: WorkspaceGrantId,
    pub agent_id: Uuid,
    pub run_id: Uuid,
    /// Identity of the grant root when the change was recorded.
    pub root: FileIdentity,
    /// `/`-separated path beneath the grant root.
    pub relative: String,
}

// ---------------------------------------------------------------------------
// Change tracking
// ---------------------------------------------------------------------------

/// `identity` on a file change is the native identity of the file this entry
/// last left in place, or `None` when it left no file. Replay deletes or
/// replaces a file only while it still has that identity and content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEntry {
    /// `before: None` means the write created the file.
    FileWrite {
        file: GrantedFile,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
        identity: Option<FileIdentity>,
    },
    FileDelete {
        file: GrantedFile,
        before: Vec<u8>,
        identity: Option<FileIdentity>,
    },
    FileCreate {
        file: GrantedFile,
        after: Vec<u8>,
        identity: Option<FileIdentity>,
    },
    AgentStateChange {
        agent_id: String,
        field: String,
        before: Value,
        after: Value,
    },
    ConfigChange {
        key: String,
        before: Value,
        after: Value,
    },
}

impl ChangeEntry {
    /// The recorded file target of a file change.
    pub fn file(&self) -> Option<&GrantedFile> {
        match self {
            Self::FileWrite { file, .. }
            | Self::FileDelete { file, .. }
            | Self::FileCreate { file, .. } => Some(file),
            Self::AgentStateChange { .. } | Self::ConfigChange { .. } => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Checkpoint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub label: String,
    pub timestamp: u64,
    pub agent_id: Option<String>,
    pub changes: Vec<ChangeEntry>,
    pub undone: bool,
}

// ---------------------------------------------------------------------------
// UndoAction — returned to callers for applying non-file changes
// ---------------------------------------------------------------------------

/// A non-file change for the caller to apply. File changes are applied only
/// inside the Time Machine, under [`FileAuthority`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    RestoreAgentState {
        agent_id: String,
        field: String,
        value: Value,
    },
    RestoreConfig {
        key: String,
        value: Value,
    },
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMachineConfig {
    pub max_checkpoints: usize,
    pub max_file_size_bytes: u64,
    pub auto_checkpoint: bool,
}

impl Default for TimeMachineConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 200,
            max_file_size_bytes: 10_485_760, // 10 MB
            auto_checkpoint: true,
        }
    }
}

// ---------------------------------------------------------------------------
// CheckpointBuilder
// ---------------------------------------------------------------------------

pub struct CheckpointBuilder {
    label: String,
    agent_id: Option<String>,
    changes: Vec<ChangeEntry>,
    max_file_size: u64,
}

impl CheckpointBuilder {
    pub fn new(label: &str, agent_id: Option<String>, max_file_size: u64) -> Self {
        Self {
            label: label.to_string(),
            agent_id,
            changes: Vec::new(),
            max_file_size,
        }
    }

    /// Records a completed write beneath a live grant. `before: None` means the
    /// write created the file. Oversized contents are skipped, as before.
    pub fn record_file_write(
        &mut self,
        authority: &FileAuthority<'_>,
        grant: WorkspaceGrantId,
        relative: &str,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    ) -> Result<(), TimeMachineError> {
        let oversized = before
            .as_ref()
            .is_some_and(|b| b.len() as u64 > self.max_file_size);
        if oversized || after.len() as u64 > self.max_file_size {
            return Ok(());
        }
        let (file, target) = self.granted(authority, grant, relative, false)?;
        let identity = present_identity(&target, &after)?;
        self.changes.push(ChangeEntry::FileWrite {
            file,
            before,
            after,
            identity: Some(identity),
        });
        Ok(())
    }

    /// Records a completed file creation beneath a live grant.
    pub fn record_file_create(
        &mut self,
        authority: &FileAuthority<'_>,
        grant: WorkspaceGrantId,
        relative: &str,
        after: Vec<u8>,
    ) -> Result<(), TimeMachineError> {
        if after.len() as u64 > self.max_file_size {
            return Ok(());
        }
        let (file, target) = self.granted(authority, grant, relative, false)?;
        let identity = present_identity(&target, &after)?;
        self.changes.push(ChangeEntry::FileCreate {
            file,
            after,
            identity: Some(identity),
        });
        Ok(())
    }

    /// Records a completed file deletion beneath a live grant.
    pub fn record_file_delete(
        &mut self,
        authority: &FileAuthority<'_>,
        grant: WorkspaceGrantId,
        relative: &str,
        before: Vec<u8>,
    ) -> Result<(), TimeMachineError> {
        if before.len() as u64 > self.max_file_size {
            return Ok(());
        }
        // The deleted file's parent directories may be gone as well.
        let (file, target) = self.granted(authority, grant, relative, true)?;
        if !absent(&target).map_err(record_denied)? {
            return Err(record_denied("deleted file still exists"));
        }
        self.changes.push(ChangeEntry::FileDelete {
            file,
            before,
            identity: None,
        });
        Ok(())
    }

    fn granted(
        &self,
        authority: &FileAuthority<'_>,
        grant: WorkspaceGrantId,
        relative: &str,
        missing_parents: bool,
    ) -> Result<(GrantedFile, PathBuf), TimeMachineError> {
        let resolved = authority.writable_grant(grant).map_err(record_denied)?;
        let root = governed_path::identity_of(resolved.root())
            .map_err(|_| record_denied("workspace root changed"))?;
        if self.changes.iter().any(|c| {
            c.file()
                .is_some_and(|f| f.grant == grant && f.relative == relative)
        }) {
            return Err(record_denied("file already changed in this checkpoint"));
        }
        let target = governed_path::join_relative(resolved.root(), relative)
            .map_err(|_| record_denied("not a valid relative path"))?;
        check_parents(resolved.root(), relative, missing_parents).map_err(record_denied)?;
        let file = GrantedFile {
            grant,
            agent_id: authority.binding.agent_id,
            run_id: authority.binding.run_id,
            root,
            relative: relative.to_string(),
        };
        Ok((file, target))
    }

    pub fn record_agent_state(&mut self, agent_id: &str, field: &str, before: Value, after: Value) {
        self.changes.push(ChangeEntry::AgentStateChange {
            agent_id: agent_id.to_string(),
            field: field.to_string(),
            before,
            after,
        });
    }

    pub fn record_config_change(&mut self, key: &str, before: Value, after: Value) {
        self.changes.push(ChangeEntry::ConfigChange {
            key: key.to_string(),
            before,
            after,
        });
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    pub fn build(self) -> Checkpoint {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Checkpoint {
            id: Uuid::new_v4().to_string(),
            label: self.label,
            timestamp: now,
            agent_id: self.agent_id,
            changes: self.changes,
            undone: false,
        }
    }
}

fn record_denied(reason: &'static str) -> TimeMachineError {
    TimeMachineError::UndoFailed(format!("file change not recorded: {reason}"))
}

/// The identity of the regular file at `target`, which must hold `content`.
fn present_identity(target: &Path, content: &[u8]) -> Result<FileIdentity, TimeMachineError> {
    let identity =
        governed_path::identity_of(target).map_err(|_| record_denied("file is not present"))?;
    if !holds(target, content).map_err(record_denied)? {
        return Err(record_denied("file does not hold the recorded content"));
    }
    Ok(identity)
}

// ---------------------------------------------------------------------------
// TimeMachine
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TimeMachine {
    config: TimeMachineConfig,
    checkpoints: Vec<Checkpoint>,
    redo_stack: Vec<Checkpoint>,
}

impl Default for TimeMachine {
    fn default() -> Self {
        Self::new(TimeMachineConfig::default())
    }
}

impl TimeMachine {
    pub fn new(config: TimeMachineConfig) -> Self {
        Self {
            config,
            checkpoints: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn begin_checkpoint(&self, label: &str, agent_id: Option<String>) -> CheckpointBuilder {
        CheckpointBuilder::new(label, agent_id, self.config.max_file_size_bytes)
    }

    /// Commit a checkpoint. Returns `(id, evicted_count)`.
    pub fn commit_checkpoint(
        &mut self,
        checkpoint: Checkpoint,
    ) -> Result<(String, usize), TimeMachineError> {
        let id = checkpoint.id.clone();
        self.checkpoints.push(checkpoint);

        // New action invalidates redo history.
        self.redo_stack.clear();

        // Evict oldest if over capacity.
        let mut evicted = 0;
        while self.checkpoints.len() > self.config.max_checkpoints {
            self.checkpoints.remove(0);
            evicted += 1;
        }

        Ok((id, evicted))
    }

    /// Undo without file authority: a checkpoint with no file change is undone;
    /// one that records a file change is refused and left untouched.
    pub fn undo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        self.undo_with(None)
    }

    pub fn undo_with(
        &mut self,
        authority: Option<&FileAuthority<'_>>,
    ) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        // Find most recent non-undone checkpoint.
        let idx = self
            .checkpoints
            .iter()
            .rposition(|c| !c.undone)
            .ok_or(TimeMachineError::EmptyHistory)?;

        replay_files(
            &mut self.checkpoints[idx].changes,
            Direction::Undo,
            authority,
        )?;
        self.checkpoints[idx].undone = true;
        let cp = self.checkpoints[idx].clone();
        let non_file = reverse_changes(&cp.changes);

        self.redo_stack.push(cp.clone());
        Ok((cp, non_file))
    }

    /// Redo without file authority; see [`TimeMachine::undo`].
    pub fn redo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        self.redo_with(None)
    }

    pub fn redo_with(
        &mut self,
        authority: Option<&FileAuthority<'_>>,
    ) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        let cp = self
            .redo_stack
            .last_mut()
            .ok_or(TimeMachineError::RedoFailed("nothing to redo".into()))?;
        replay_files(&mut cp.changes, Direction::Redo, authority)?;

        let mut cp = self
            .redo_stack
            .pop()
            .ok_or(TimeMachineError::RedoFailed("nothing to redo".into()))?;
        cp.undone = false;
        let non_file = forward_changes(&cp.changes);

        // Put back in checkpoints.
        self.checkpoints.push(cp.clone());
        Ok((cp, non_file))
    }

    /// Selective undo without file authority; see [`TimeMachine::undo`].
    pub fn undo_checkpoint(
        &mut self,
        id: &str,
    ) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        self.undo_checkpoint_with(id, None)
    }

    pub fn undo_checkpoint_with(
        &mut self,
        id: &str,
        authority: Option<&FileAuthority<'_>>,
    ) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        let idx = self
            .checkpoints
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| TimeMachineError::CheckpointNotFound(id.to_string()))?;

        if self.checkpoints[idx].undone {
            return Err(TimeMachineError::UndoFailed(
                "checkpoint already undone".into(),
            ));
        }

        replay_files(
            &mut self.checkpoints[idx].changes,
            Direction::Undo,
            authority,
        )?;
        self.checkpoints[idx].undone = true;
        let cp = self.checkpoints[idx].clone();
        let non_file = reverse_changes(&cp.changes);

        // Selective undo does not push to redo stack.
        Ok((cp, non_file))
    }

    pub fn list_checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    pub fn get_checkpoint(&self, id: &str) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == id)
    }

    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn config(&self) -> &TimeMachineConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Non-file changes
// ---------------------------------------------------------------------------

fn reverse_changes(changes: &[ChangeEntry]) -> Vec<UndoAction> {
    changes
        .iter()
        .filter_map(|entry| match entry {
            ChangeEntry::AgentStateChange {
                agent_id,
                field,
                before,
                ..
            } => Some(UndoAction::RestoreAgentState {
                agent_id: agent_id.clone(),
                field: field.clone(),
                value: before.clone(),
            }),
            ChangeEntry::ConfigChange { key, before, .. } => Some(UndoAction::RestoreConfig {
                key: key.clone(),
                value: before.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn forward_changes(changes: &[ChangeEntry]) -> Vec<UndoAction> {
    changes
        .iter()
        .filter_map(|entry| match entry {
            ChangeEntry::AgentStateChange {
                agent_id,
                field,
                after,
                ..
            } => Some(UndoAction::RestoreAgentState {
                agent_id: agent_id.clone(),
                field: field.clone(),
                value: after.clone(),
            }),
            ChangeEntry::ConfigChange { key, after, .. } => Some(UndoAction::RestoreConfig {
                key: key.clone(),
                value: after.clone(),
            }),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// File replay
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

/// The state a file must be in before a step.
enum Expect<'a> {
    Absent,
    Present(FileIdentity, &'a [u8]),
}

enum Effect<'a> {
    Delete,
    Write(&'a [u8]),
}

struct Step<'a> {
    change: usize,
    expect: Expect<'a>,
    effect: Effect<'a>,
}

/// A file step whose authority, target and preconditions were all checked.
struct Checked<'a> {
    step: Step<'a>,
    root: PathBuf,
    target: PathBuf,
    relative: &'a str,
}

/// Plans, fully checks, then applies every file change of a checkpoint in
/// `direction`. Nothing is mutated unless every change is authorized and every
/// target is in its expected state. Identities of files left in place are
/// written back into `changes` for later replay.
fn replay_files(
    changes: &mut [ChangeEntry],
    direction: Direction,
    authority: Option<&FileAuthority<'_>>,
) -> Result<(), TimeMachineError> {
    let mut order: Vec<usize> = (0..changes.len())
        .filter(|&i| changes[i].file().is_some())
        .collect();
    if order.is_empty() {
        return Ok(());
    }
    let authority = authority.ok_or(TimeMachineError::FileAuthorityRequired)?;
    if matches!(direction, Direction::Undo) {
        order.reverse();
    }

    let left = {
        let changes: &[ChangeEntry] = changes;
        let mut checked = Vec::with_capacity(order.len());
        let mut targets: Vec<(WorkspaceGrantId, &str)> = Vec::new();
        for &change in &order {
            let denied = |reason| TimeMachineError::FileDenied { change, reason };
            let file = changes[change].file().ok_or(denied("not a file change"))?;
            if targets.contains(&(file.grant, file.relative.as_str())) {
                return Err(denied("file changed twice in this checkpoint"));
            }
            targets.push((file.grant, file.relative.as_str()));
            let step = plan(change, &changes[change], direction).map_err(denied)?;
            let (root, target) = authority.target(file).map_err(denied)?;
            let creates = matches!(step.expect, Expect::Absent);
            check_parents(&root, &file.relative, creates).map_err(denied)?;
            check_expect(&target, &step.expect).map_err(denied)?;
            checked.push(Checked {
                step,
                root,
                target,
                relative: file.relative.as_str(),
            });
        }

        let total = checked.len();
        let mut left: Vec<(usize, Option<FileIdentity>)> = Vec::with_capacity(total);
        let mut failure = None;
        for (applied, item) in checked.iter().enumerate() {
            match apply(item) {
                Ok(identity) => left.push((item.step.change, identity)),
                Err(reason) => {
                    failure = Some(TimeMachineError::PartiallyApplied {
                        applied,
                        total,
                        reason,
                    });
                    break;
                }
            }
        }
        (left, failure)
    };

    let (left, failure) = left;
    for (change, identity) in left {
        if let ChangeEntry::FileWrite { identity: slot, .. }
        | ChangeEntry::FileDelete { identity: slot, .. }
        | ChangeEntry::FileCreate { identity: slot, .. } = &mut changes[change]
        {
            *slot = identity;
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn plan(
    change: usize,
    entry: &ChangeEntry,
    direction: Direction,
) -> Result<Step<'_>, &'static str> {
    let present = |identity: &Option<FileIdentity>, content| match identity {
        Some(identity) => Ok(Expect::Present(*identity, content)),
        None => Err("recorded file identity is missing"),
    };
    let (expect, effect) = match (entry, direction) {
        (
            ChangeEntry::FileCreate {
                after, identity, ..
            },
            Direction::Undo,
        )
        | (
            ChangeEntry::FileWrite {
                before: None,
                after,
                identity,
                ..
            },
            Direction::Undo,
        ) => (present(identity, after)?, Effect::Delete),
        (ChangeEntry::FileCreate { after, .. }, Direction::Redo)
        | (
            ChangeEntry::FileWrite {
                before: None,
                after,
                ..
            },
            Direction::Redo,
        ) => (Expect::Absent, Effect::Write(after)),
        (
            ChangeEntry::FileWrite {
                before: Some(before),
                after,
                identity,
                ..
            },
            Direction::Undo,
        ) => (present(identity, after)?, Effect::Write(before)),
        (
            ChangeEntry::FileWrite {
                before: Some(before),
                after,
                identity,
                ..
            },
            Direction::Redo,
        ) => (present(identity, before)?, Effect::Write(after)),
        (ChangeEntry::FileDelete { before, .. }, Direction::Undo) => {
            (Expect::Absent, Effect::Write(before))
        }
        (
            ChangeEntry::FileDelete {
                before, identity, ..
            },
            Direction::Redo,
        ) => (present(identity, before)?, Effect::Delete),
        _ => return Err("not a file change"),
    };
    Ok(Step {
        change,
        expect,
        effect,
    })
}

/// Every existing directory between `root` and the target must be a real
/// directory, not a redirect. Missing ones are allowed only when the step will
/// create the target.
fn check_parents(root: &Path, relative: &str, may_create: bool) -> Result<(), &'static str> {
    let components: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_path_buf();
    for component in &components[..components.len().saturating_sub(1)] {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if governed_path::is_redirect(&metadata) => {
                return Err("a parent directory is a redirect")
            }
            Ok(metadata) if !metadata.is_dir() => return Err("a parent is not a directory"),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && may_create => {
                return Ok(())
            }
            Err(_) => return Err("a parent directory is unavailable"),
        }
    }
    Ok(())
}

fn check_expect(target: &Path, expect: &Expect<'_>) -> Result<(), &'static str> {
    match expect {
        Expect::Absent => {
            if absent(target)? {
                Ok(())
            } else {
                Err("target already exists")
            }
        }
        Expect::Present(identity, content) => {
            if governed_path::identity_of(target) != Ok(*identity) {
                return Err("target is not the recorded file");
            }
            if !holds(target, content)? {
                return Err("target content changed since it was recorded");
            }
            Ok(())
        }
    }
}

fn absent(target: &Path) -> Result<bool, &'static str> {
    match std::fs::symlink_metadata(target) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err("target is unavailable"),
    }
}

/// Whether the regular file at `target` holds exactly `content`.
fn holds(target: &Path, content: &[u8]) -> Result<bool, &'static str> {
    let metadata = std::fs::symlink_metadata(target).map_err(|_| "target is unavailable")?;
    if !metadata.is_file() || governed_path::is_redirect(&metadata) {
        return Err("target is not a regular file");
    }
    if metadata.len() != content.len() as u64 {
        return Ok(false);
    }
    let mut current = Vec::with_capacity(content.len());
    std::fs::File::open(target)
        .and_then(|file| {
            file.take(content.len() as u64 + 1)
                .read_to_end(&mut current)
        })
        .map_err(|_| "target is unavailable")?;
    Ok(current == content)
}

#[cfg(test)]
thread_local! {
    /// Test-only fault injection: fail the apply step for this relative path.
    static FAIL_APPLY: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Applies one checked step and returns the identity of the file it leaves.
fn apply(item: &Checked<'_>) -> Result<Option<FileIdentity>, &'static str> {
    #[cfg(test)]
    if FAIL_APPLY.with(|fail| fail.get()) == Some(item.relative) {
        return Err("injected failure");
    }
    let target = &item.target;
    // Re-check immediately before mutating; the earlier pass checked every
    // step before any of them ran.
    check_expect(target, &item.step.expect)?;
    match (&item.step.expect, &item.step.effect) {
        (Expect::Present(..), Effect::Delete) => {
            std::fs::remove_file(target).map_err(|_| "could not delete the target")?;
            Ok(None)
        }
        (Expect::Absent, Effect::Write(content)) => {
            create_parents(&item.root, item.relative)?;
            write_new(target, content)?;
            governed_path::identity_of(target)
                .map(Some)
                .map_err(|_| "written file is unavailable")
        }
        (Expect::Present(identity, _), Effect::Write(content)) => {
            let parent = target.parent().ok_or("target has no parent")?;
            let temporary = parent.join(format!(".nexus-time-machine-{}.tmp", Uuid::new_v4()));
            write_new(&temporary, content)?;
            if governed_path::identity_of(target) != Ok(*identity) {
                let _ = std::fs::remove_file(&temporary);
                return Err("target is not the recorded file");
            }
            if std::fs::rename(&temporary, target).is_err() {
                let _ = std::fs::remove_file(&temporary);
                return Err("could not replace the target");
            }
            governed_path::identity_of(target)
                .map(Some)
                .map_err(|_| "written file is unavailable")
        }
        (Expect::Absent, Effect::Delete) => Err("nothing to delete"),
    }
}

/// Creates missing directories between `root` and the target, one validated
/// component at a time; an existing redirect or non-directory stops it.
fn create_parents(root: &Path, relative: &str) -> Result<(), &'static str> {
    let components: Vec<&str> = relative.split('/').collect();
    let mut path = root.to_path_buf();
    for component in &components[..components.len().saturating_sub(1)] {
        path.push(component);
        match std::fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata =
                    std::fs::symlink_metadata(&path).map_err(|_| "a parent is unavailable")?;
                if governed_path::is_redirect(&metadata) || !metadata.is_dir() {
                    return Err("a parent directory is a redirect");
                }
            }
            Err(_) => return Err("could not create a parent directory"),
        }
    }
    Ok(())
}

/// Creates `path` exclusively; an existing entry, including a redirect, fails.
fn write_new(path: &Path, content: &[u8]) -> Result<(), &'static str> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "could not create the file")?;
    if file
        .write_all(content)
        .and_then(|_| file.sync_all())
        .is_err()
    {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err("could not write the file");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
