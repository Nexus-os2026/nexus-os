//! Time Machine — system-wide undo/redo engine for Nexus OS.
//!
//! Captures reversible operations at checkpoint boundaries.  Each checkpoint
//! records the before/after state of every change so the system can roll back
//! file writes, agent state mutations, and config edits.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
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
        }
    }
}

impl std::error::Error for TimeMachineError {}

// ---------------------------------------------------------------------------
// Change tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEntry {
    FileWrite {
        path: String,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    },
    FileDelete {
        path: String,
        before: Vec<u8>,
    },
    FileCreate {
        path: String,
        after: Vec<u8>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UndoAction {
    RestoreFile {
        path: String,
        content: Option<Vec<u8>>,
    },
    DeleteFile {
        path: String,
    },
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

    pub fn record_file_write(&mut self, path: &str, before: Option<Vec<u8>>, after: Vec<u8>) {
        // Skip files that exceed the size limit in either direction.
        if after.len() as u64 > self.max_file_size {
            return;
        }
        if let Some(ref b) = before {
            if b.len() as u64 > self.max_file_size {
                return;
            }
        }
        self.changes.push(ChangeEntry::FileWrite {
            path: path.to_string(),
            before,
            after,
        });
    }

    pub fn record_file_create(&mut self, path: &str, after: Vec<u8>) {
        if after.len() as u64 > self.max_file_size {
            return;
        }
        self.changes.push(ChangeEntry::FileCreate {
            path: path.to_string(),
            after,
        });
    }

    pub fn record_file_delete(&mut self, path: &str, before: Vec<u8>) {
        if before.len() as u64 > self.max_file_size {
            return;
        }
        self.changes.push(ChangeEntry::FileDelete {
            path: path.to_string(),
            before,
        });
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

    pub fn undo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        // Find most recent non-undone checkpoint.
        let idx = self
            .checkpoints
            .iter()
            .rposition(|c| !c.undone)
            .ok_or(TimeMachineError::EmptyHistory)?;

        self.checkpoints[idx].undone = true;
        let cp = self.checkpoints[idx].clone();

        let actions = reverse_changes(&cp.changes);
        apply_file_actions(&actions)?;

        let non_file = actions.into_iter().filter(|a| !is_file_action(a)).collect();

        self.redo_stack.push(cp.clone());
        Ok((cp, non_file))
    }

    pub fn redo(&mut self) -> Result<(Checkpoint, Vec<UndoAction>), TimeMachineError> {
        let mut cp = self
            .redo_stack
            .pop()
            .ok_or(TimeMachineError::RedoFailed("nothing to redo".into()))?;

        cp.undone = false;

        let actions = forward_changes(&cp.changes);
        apply_file_actions(&actions)?;

        let non_file = actions.into_iter().filter(|a| !is_file_action(a)).collect();

        // Put back in checkpoints.
        self.checkpoints.push(cp.clone());
        Ok((cp, non_file))
    }

    pub fn undo_checkpoint(
        &mut self,
        id: &str,
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

        self.checkpoints[idx].undone = true;
        let cp = self.checkpoints[idx].clone();

        let actions = reverse_changes(&cp.changes);
        apply_file_actions(&actions)?;

        let non_file = actions.into_iter().filter(|a| !is_file_action(a)).collect();

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
// Helpers
// ---------------------------------------------------------------------------

fn reverse_changes(changes: &[ChangeEntry]) -> Vec<UndoAction> {
    changes
        .iter()
        .map(|entry| match entry {
            ChangeEntry::FileWrite { path, before, .. } => match before {
                Some(bytes) => UndoAction::RestoreFile {
                    path: path.clone(),
                    content: Some(bytes.clone()),
                },
                None => UndoAction::DeleteFile { path: path.clone() },
            },
            ChangeEntry::FileCreate { path, .. } => UndoAction::DeleteFile { path: path.clone() },
            ChangeEntry::FileDelete { path, before } => UndoAction::RestoreFile {
                path: path.clone(),
                content: Some(before.clone()),
            },
            ChangeEntry::AgentStateChange {
                agent_id,
                field,
                before,
                ..
            } => UndoAction::RestoreAgentState {
                agent_id: agent_id.clone(),
                field: field.clone(),
                value: before.clone(),
            },
            ChangeEntry::ConfigChange { key, before, .. } => UndoAction::RestoreConfig {
                key: key.clone(),
                value: before.clone(),
            },
        })
        .collect()
}

fn forward_changes(changes: &[ChangeEntry]) -> Vec<UndoAction> {
    changes
        .iter()
        .map(|entry| match entry {
            ChangeEntry::FileWrite { path, after, .. } => UndoAction::RestoreFile {
                path: path.clone(),
                content: Some(after.clone()),
            },
            ChangeEntry::FileCreate { path, after } => UndoAction::RestoreFile {
                path: path.clone(),
                content: Some(after.clone()),
            },
            ChangeEntry::FileDelete { path, .. } => UndoAction::DeleteFile { path: path.clone() },
            ChangeEntry::AgentStateChange {
                agent_id,
                field,
                after,
                ..
            } => UndoAction::RestoreAgentState {
                agent_id: agent_id.clone(),
                field: field.clone(),
                value: after.clone(),
            },
            ChangeEntry::ConfigChange { key, after, .. } => UndoAction::RestoreConfig {
                key: key.clone(),
                value: after.clone(),
            },
        })
        .collect()
}

fn is_file_action(action: &UndoAction) -> bool {
    matches!(
        action,
        UndoAction::RestoreFile { .. } | UndoAction::DeleteFile { .. }
    )
}

fn apply_file_actions(actions: &[UndoAction]) -> Result<(), TimeMachineError> {
    let mut errors = Vec::new();
    for action in actions {
        match action {
            UndoAction::RestoreFile {
                path,
                content: Some(bytes),
            } => {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        errors.push(format!("create_dir_all {}: {e}", parent.display()));
                        continue;
                    }
                }
                if let Err(e) = std::fs::write(path, bytes) {
                    errors.push(format!("write {path}: {e}"));
                }
            }
            UndoAction::RestoreFile { content: None, .. } => {}
            UndoAction::DeleteFile { path } => {
                if let Err(e) = std::fs::remove_file(path) {
                    errors.push(format!("remove {path}: {e}"));
                }
            }
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(TimeMachineError::Io(errors.join("; ")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn small_config() -> TimeMachineConfig {
        TimeMachineConfig {
            max_checkpoints: 200,
            max_file_size_bytes: 10_485_760,
            auto_checkpoint: true,
        }
    }

    #[test]
    fn test_config_defaults() {
        let cfg = TimeMachineConfig::default();
        assert_eq!(cfg.max_checkpoints, 200);
        assert_eq!(cfg.max_file_size_bytes, 10_485_760);
        assert!(cfg.auto_checkpoint);
    }

    #[test]
    fn test_create_checkpoint() {
        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("test cp", None);
        builder.record_agent_state("a1", "fuel", json!(100), json!(90));
        builder.record_config_change("theme", json!("dark"), json!("light"));
        let cp = builder.build();
        assert_eq!(cp.changes.len(), 2);
        assert!(!cp.undone);

        let (id, evicted) = tm.commit_checkpoint(cp).unwrap();
        assert_eq!(evicted, 0);
        assert_eq!(tm.checkpoint_count(), 1);
        assert!(tm.get_checkpoint(&id).is_some());
    }

    #[test]
    fn test_undo_file_write() {
        let dir = std::env::temp_dir().join(format!("tm_test_write_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.txt");
        std::fs::write(&file_path, b"original").unwrap();

        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("edit file", None);
        builder.record_file_write(
            file_path.to_str().unwrap(),
            Some(b"original".to_vec()),
            b"modified".to_vec(),
        );
        let cp = builder.build();
        tm.commit_checkpoint(cp).unwrap();

        // Overwrite the file to simulate the write having happened.
        std::fs::write(&file_path, b"modified").unwrap();

        let (undone_cp, non_file) = tm.undo().unwrap();
        assert!(undone_cp.undone);
        assert!(non_file.is_empty());
        assert_eq!(std::fs::read(&file_path).unwrap(), b"original");

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_undo_file_create() {
        let dir = std::env::temp_dir().join(format!("tm_test_create_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("new_file.txt");

        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("create file", None);
        builder.record_file_create(file_path.to_str().unwrap(), b"content".to_vec());
        let cp = builder.build();
        tm.commit_checkpoint(cp).unwrap();

        // Create the file to simulate the action.
        std::fs::write(&file_path, b"content").unwrap();
        assert!(file_path.exists());

        let (_, non_file) = tm.undo().unwrap();
        assert!(non_file.is_empty());
        assert!(!file_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_undo_file_delete() {
        let dir = std::env::temp_dir().join(format!("tm_test_delete_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("doomed.txt");
        std::fs::write(&file_path, b"precious data").unwrap();

        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("delete file", None);
        builder.record_file_delete(file_path.to_str().unwrap(), b"precious data".to_vec());
        let cp = builder.build();
        tm.commit_checkpoint(cp).unwrap();

        // Simulate the delete.
        std::fs::remove_file(&file_path).unwrap();
        assert!(!file_path.exists());

        let (_, non_file) = tm.undo().unwrap();
        assert!(non_file.is_empty());
        assert!(file_path.exists());
        assert_eq!(std::fs::read(&file_path).unwrap(), b"precious data");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_redo_after_undo() {
        let dir = std::env::temp_dir().join(format!("tm_test_redo_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("redo.txt");
        std::fs::write(&file_path, b"before").unwrap();

        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("edit", None);
        builder.record_file_write(
            file_path.to_str().unwrap(),
            Some(b"before".to_vec()),
            b"after".to_vec(),
        );
        let cp = builder.build();
        let (id, _) = tm.commit_checkpoint(cp).unwrap();

        // Simulate the write.
        std::fs::write(&file_path, b"after").unwrap();

        // Undo.
        tm.undo().unwrap();
        assert_eq!(std::fs::read(&file_path).unwrap(), b"before");
        assert_eq!(tm.redo_stack.len(), 1);

        // Redo.
        let (redo_cp, _) = tm.redo().unwrap();
        assert!(!redo_cp.undone);
        assert_eq!(redo_cp.id, id);
        assert_eq!(std::fs::read(&file_path).unwrap(), b"after");
        assert!(tm.redo_stack.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_selective_undo() {
        let mut tm = TimeMachine::new(small_config());

        // Create 3 checkpoints with agent state changes.
        let ids: Vec<String> = (0..3)
            .map(|i| {
                let mut builder =
                    tm.begin_checkpoint(&format!("cp{i}"), Some(format!("agent-{i}")));
                builder.record_agent_state(
                    &format!("agent-{i}"),
                    "fuel",
                    json!(100 - i * 10),
                    json!(90 - i * 10),
                );
                let cp = builder.build();
                tm.commit_checkpoint(cp).unwrap().0
            })
            .collect();

        // Undo the middle one.
        let (cp, actions) = tm.undo_checkpoint(&ids[1]).unwrap();
        assert_eq!(cp.id, ids[1]);
        assert!(cp.undone);
        assert_eq!(actions.len(), 1);

        // The other two should be unaffected.
        assert!(!tm.get_checkpoint(&ids[0]).unwrap().undone);
        assert!(!tm.get_checkpoint(&ids[2]).unwrap().undone);

        // Selective undo does not push to redo stack.
        assert!(tm.redo_stack.is_empty());
    }

    #[test]
    fn test_capacity_eviction() {
        let cfg = TimeMachineConfig {
            max_checkpoints: 3,
            ..TimeMachineConfig::default()
        };
        let mut tm = TimeMachine::new(cfg);

        let mut ids = Vec::new();
        for i in 0..5 {
            let builder = tm.begin_checkpoint(&format!("cp{i}"), None);
            let cp = builder.build();
            let (id, _) = tm.commit_checkpoint(cp).unwrap();
            ids.push(id);
        }

        assert_eq!(tm.checkpoint_count(), 3);
        // First two should have been evicted.
        assert!(tm.get_checkpoint(&ids[0]).is_none());
        assert!(tm.get_checkpoint(&ids[1]).is_none());
        // Last three should remain.
        assert!(tm.get_checkpoint(&ids[2]).is_some());
        assert!(tm.get_checkpoint(&ids[3]).is_some());
        assert!(tm.get_checkpoint(&ids[4]).is_some());
    }

    #[test]
    fn test_large_file_skip() {
        let cfg = TimeMachineConfig {
            max_file_size_bytes: 100,
            ..TimeMachineConfig::default()
        };
        let mut tm = TimeMachine::new(cfg);
        let mut builder = tm.begin_checkpoint("big write", None);

        // 200-byte content should be skipped.
        let big = vec![0u8; 200];
        builder.record_file_write("/tmp/big.bin", Some(vec![0u8; 50]), big);
        assert_eq!(builder.change_count(), 0);

        // Small content should be kept.
        builder.record_file_write("/tmp/small.txt", None, vec![0u8; 50]);
        assert_eq!(builder.change_count(), 1);

        let cp = builder.build();
        assert_eq!(cp.changes.len(), 1);
        tm.commit_checkpoint(cp).unwrap();
    }

    #[test]
    fn test_undo_agent_state() {
        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("state change", Some("agent-x".into()));
        builder.record_agent_state("agent-x", "autonomy_level", json!(3), json!(5));
        let cp = builder.build();
        tm.commit_checkpoint(cp).unwrap();

        let (_, actions) = tm.undo().unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UndoAction::RestoreAgentState {
                agent_id,
                field,
                value,
            } => {
                assert_eq!(agent_id, "agent-x");
                assert_eq!(field, "autonomy_level");
                assert_eq!(*value, json!(3));
            }
            other => panic!("expected RestoreAgentState, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_undo_error() {
        let mut tm = TimeMachine::new(small_config());
        let result = tm.undo();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TimeMachineError::EmptyHistory
        ));
    }

    #[test]
    fn test_redo_empty_error() {
        let mut tm = TimeMachine::new(small_config());
        let result = tm.redo();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TimeMachineError::RedoFailed(_)
        ));
    }

    #[test]
    fn test_new_checkpoint_clears_redo() {
        let mut tm = TimeMachine::new(small_config());

        let mut builder = tm.begin_checkpoint("first", None);
        builder.record_agent_state("a", "f", json!(1), json!(2));
        let cp = builder.build();
        tm.commit_checkpoint(cp).unwrap();

        // Undo to populate redo stack.
        tm.undo().unwrap();
        assert_eq!(tm.redo_stack.len(), 1);

        // New checkpoint clears redo.
        let builder2 = tm.begin_checkpoint("second", None);
        let cp2 = builder2.build();
        tm.commit_checkpoint(cp2).unwrap();
        assert!(tm.redo_stack.is_empty());
    }

    #[test]
    fn test_checkpoint_builder_change_count() {
        let mut builder = CheckpointBuilder::new("test", None, 10_485_760);
        assert_eq!(builder.change_count(), 0);

        builder.record_file_create("/a", vec![1]);
        builder.record_file_delete("/b", vec![2]);
        builder.record_agent_state("x", "f", json!(1), json!(2));
        assert_eq!(builder.change_count(), 3);
    }

    #[test]
    fn test_large_before_file_skip() {
        let cfg = TimeMachineConfig {
            max_file_size_bytes: 100,
            ..TimeMachineConfig::default()
        };
        let tm = TimeMachine::new(cfg);
        let mut builder = tm.begin_checkpoint("big before", None);

        // Large before content should also cause a skip.
        builder.record_file_write("/tmp/x.txt", Some(vec![0u8; 200]), vec![0u8; 50]);
        assert_eq!(builder.change_count(), 0);
    }

    #[test]
    fn test_file_create_large_skip() {
        let cfg = TimeMachineConfig {
            max_file_size_bytes: 100,
            ..TimeMachineConfig::default()
        };
        let tm = TimeMachine::new(cfg);
        let mut builder = tm.begin_checkpoint("big create", None);
        builder.record_file_create("/tmp/huge.bin", vec![0u8; 200]);
        assert_eq!(builder.change_count(), 0);
    }

    #[test]
    fn test_file_delete_large_skip() {
        let cfg = TimeMachineConfig {
            max_file_size_bytes: 100,
            ..TimeMachineConfig::default()
        };
        let tm = TimeMachine::new(cfg);
        let mut builder = tm.begin_checkpoint("big delete", None);
        builder.record_file_delete("/tmp/huge.bin", vec![0u8; 200]);
        assert_eq!(builder.change_count(), 0);
    }

    #[test]
    fn test_undo_already_undone() {
        let mut tm = TimeMachine::new(small_config());
        let builder = tm.begin_checkpoint("only", None);
        tm.commit_checkpoint(builder.build()).unwrap();

        tm.undo().unwrap();
        // Second undo should fail — nothing left to undo.
        assert!(matches!(
            tm.undo().unwrap_err(),
            TimeMachineError::EmptyHistory
        ));
    }

    #[test]
    fn test_selective_undo_already_undone() {
        let mut tm = TimeMachine::new(small_config());
        let builder = tm.begin_checkpoint("cp", None);
        let cp = builder.build();
        let (id, _) = tm.commit_checkpoint(cp).unwrap();

        tm.undo_checkpoint(&id).unwrap();
        let result = tm.undo_checkpoint(&id);
        assert!(matches!(
            result.unwrap_err(),
            TimeMachineError::UndoFailed(_)
        ));
    }

    #[test]
    fn test_selective_undo_not_found() {
        let mut tm = TimeMachine::new(small_config());
        let result = tm.undo_checkpoint("nonexistent-id");
        assert!(matches!(
            result.unwrap_err(),
            TimeMachineError::CheckpointNotFound(_)
        ));
    }

    #[test]
    fn test_undo_config_change() {
        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("config edit", None);
        builder.record_config_change("theme", json!("dark"), json!("light"));
        tm.commit_checkpoint(builder.build()).unwrap();

        let (_, actions) = tm.undo().unwrap();
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            UndoAction::RestoreConfig { key, value } => {
                assert_eq!(key, "theme");
                assert_eq!(*value, json!("dark"));
            }
            other => panic!("expected RestoreConfig, got {other:?}"),
        }
    }

    #[test]
    fn test_undo_file_write_no_before() {
        // FileWrite with before=None means the file didn't exist — undo should delete it.
        let dir = std::env::temp_dir().join(format!("tm_test_nobefore_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("new.txt");

        let mut tm = TimeMachine::new(small_config());
        let mut builder = tm.begin_checkpoint("write new", None);
        builder.record_file_write(file_path.to_str().unwrap(), None, b"data".to_vec());
        tm.commit_checkpoint(builder.build()).unwrap();

        // Simulate write.
        std::fs::write(&file_path, b"data").unwrap();

        tm.undo().unwrap();
        assert!(!file_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
