use crate::writer::FileChange;
use nexus_sdk::audit::{AuditEvent, AuditTrail, EventType};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorError {
    PathEscape(String),
    MissingOriginal(String),
    ConflictDetected(String),
    Io(String),
    ValidationFailed(String),
}

impl Display for EditorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorError::PathEscape(path) => write!(f, "path escapes project root: {path}"),
            EditorError::MissingOriginal(path) => {
                write!(f, "cannot modify non-existent file: {path}")
            }
            EditorError::ConflictDetected(path) => write!(f, "conflict detected for file: {path}"),
            EditorError::Io(reason) => write!(f, "io error: {reason}"),
            EditorError::ValidationFailed(reason) => write!(f, "validation failed: {reason}"),
        }
    }
}

impl std::error::Error for EditorError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub changed_files: Vec<String>,
    pub diffs: Vec<FileDiff>,
    pub backups_saved: usize,
}

#[derive(Debug, Clone)]
struct Backup {
    existed: bool,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct PlannedChange {
    relative_path: String,
    absolute_path: PathBuf,
    change: FileChange,
}

#[derive(Debug)]
pub struct MultiFileEditor {
    project_root: PathBuf,
    observed_versions: HashMap<String, u128>,
    audit_trail: AuditTrail,
    agent_id: Uuid,
}

impl MultiFileEditor {
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self, EditorError> {
        let root = project_root.as_ref();
        if !root.exists() {
            return Err(EditorError::Io(format!(
                "project root '{}' does not exist",
                root.display()
            )));
        }
        if !root.is_dir() {
            return Err(EditorError::Io(format!(
                "project root '{}' must be a directory",
                root.display()
            )));
        }

        Ok(Self {
            project_root: root.to_path_buf(),
            observed_versions: HashMap::new(),
            audit_trail: AuditTrail::new(),
            agent_id: Uuid::new_v4(),
        })
    }

    pub fn read_file(&mut self, relative_path: &str) -> Result<String, EditorError> {
        let absolute = resolve_path(self.project_root.as_path(), relative_path)?;
        let content = fs::read_to_string(absolute.as_path()).map_err(|error| {
            EditorError::Io(format!("failed to read '{}': {error}", absolute.display()))
        })?;
        let version = file_version(absolute.as_path()).unwrap_or(0);
        self.observed_versions
            .insert(relative_path.to_string(), version);
        Ok(content)
    }

    pub fn apply_changeset(
        &mut self,
        changes: Vec<FileChange>,
    ) -> Result<ApplyResult, EditorError> {
        self.apply_changeset_with(changes, || Ok(()))
    }

    pub fn apply_changeset_with<F>(
        &mut self,
        changes: Vec<FileChange>,
        validator: F,
    ) -> Result<ApplyResult, EditorError>
    where
        F: FnOnce() -> Result<(), String>,
    {
        if changes.is_empty() {
            return Ok(ApplyResult {
                changed_files: Vec::new(),
                diffs: Vec::new(),
                backups_saved: 0,
            });
        }

        let planned = self.plan_changes(changes)?;
        self.check_conflicts(planned.as_slice())?;

        let mut backups = HashMap::<String, Backup>::new();
        for change in &planned {
            if backups.contains_key(change.relative_path.as_str()) {
                continue;
            }
            let backup = read_backup(change.absolute_path.as_path())?;
            backups.insert(change.relative_path.clone(), backup);
        }

        for change in &planned {
            if let Err(error) = self.apply_single(change) {
                rollback(self.project_root.as_path(), &backups)?;
                if let Err(e) = self.audit_trail
                    .append_event(
                        self.agent_id,
                        EventType::Error,
                        json!({
                            "tool": "editor.apply_changeset",
                            "error": error.to_string(),
                            "rolled_back": true,
                        }),
                    ) {
                    tracing::error!("Audit append failed: {e}");
                }
                return Err(error);
            }
        }

        if let Err(reason) = validator() {
            rollback(self.project_root.as_path(), &backups)?;
            let error = EditorError::ValidationFailed(reason);
            if let Err(e) = self.audit_trail
                .append_event(
                    self.agent_id,
                    EventType::Error,
                    json!({
                        "tool": "editor.apply_changeset",
                        "error": error.to_string(),
                        "rolled_back": true,
                    }),
                ) {
                tracing::error!("Audit append failed: {e}");
            }
            return Err(error);
        }

        let changed_files = planned
            .iter()
            .map(|change| change.relative_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut diffs = Vec::new();
        for path in &changed_files {
            let before = backups
                .get(path.as_str())
                .map(|backup| String::from_utf8_lossy(backup.bytes.as_slice()).to_string())
                .unwrap_or_default();
            let absolute = resolve_path(self.project_root.as_path(), path.as_str())?;
            let after = if absolute.exists() {
                fs::read_to_string(absolute.as_path()).map_err(|error| {
                    EditorError::Io(format!(
                        "failed to read updated file '{}': {error}",
                        absolute.display()
                    ))
                })?
            } else {
                String::new()
            };
            diffs.push(FileDiff {
                path: path.clone(),
                diff: unified_diff(path.as_str(), before.as_str(), after.as_str()),
            });
        }

        for path in &changed_files {
            let absolute = resolve_path(self.project_root.as_path(), path.as_str())?;
            if absolute.exists() {
                if let Some(version) = file_version(absolute.as_path()) {
                    self.observed_versions.insert(path.clone(), version);
                }
            } else {
                self.observed_versions.remove(path);
            }
        }

        if let Err(e) = self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "tool": "editor.apply_changeset",
                    "changed_files": changed_files,
                    "backups_saved": backups.len(),
                }),
            ) {
            tracing::error!("Audit append failed: {e}");
        }

        Ok(ApplyResult {
            changed_files: diffs.iter().map(|entry| entry.path.clone()).collect(),
            diffs,
            backups_saved: backups.len(),
        })
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit_trail.events()
    }

    fn plan_changes(&self, changes: Vec<FileChange>) -> Result<Vec<PlannedChange>, EditorError> {
        let mut planned = Vec::new();
        for change in changes {
            let relative_path = match &change {
                FileChange::Create(path, _) => path.clone(),
                FileChange::Modify(path, _, _) => path.clone(),
                FileChange::Delete(path) => path.clone(),
            };
            let absolute = resolve_path(self.project_root.as_path(), relative_path.as_str())?;
            planned.push(PlannedChange {
                relative_path,
                absolute_path: absolute,
                change,
            });
        }
        Ok(planned)
    }

    fn check_conflicts(&self, planned: &[PlannedChange]) -> Result<(), EditorError> {
        for change in planned {
            if let Some(previous_version) =
                self.observed_versions.get(change.relative_path.as_str())
            {
                let current = file_version(change.absolute_path.as_path()).unwrap_or(0);
                if current != *previous_version {
                    return Err(EditorError::ConflictDetected(change.relative_path.clone()));
                }
            }
        }
        Ok(())
    }

    fn apply_single(&self, planned: &PlannedChange) -> Result<(), EditorError> {
        match &planned.change {
            FileChange::Create(_, content) => {
                if planned.absolute_path.exists() {
                    return Err(EditorError::ConflictDetected(planned.relative_path.clone()));
                }
                if let Some(parent) = planned.absolute_path.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        EditorError::Io(format!(
                            "failed creating parent '{}': {error}",
                            parent.display()
                        ))
                    })?;
                }
                fs::write(planned.absolute_path.as_path(), content).map_err(|error| {
                    EditorError::Io(format!(
                        "failed creating '{}': {error}",
                        planned.relative_path
                    ))
                })?;
                Ok(())
            }
            FileChange::Modify(_, old, new) => {
                if !planned.absolute_path.exists() {
                    return Err(EditorError::MissingOriginal(planned.relative_path.clone()));
                }
                let current =
                    fs::read_to_string(planned.absolute_path.as_path()).map_err(|error| {
                        EditorError::Io(format!(
                            "failed reading '{}' before modify: {error}",
                            planned.relative_path
                        ))
                    })?;
                if !old.is_empty() && current != *old {
                    return Err(EditorError::ConflictDetected(planned.relative_path.clone()));
                }
                fs::write(planned.absolute_path.as_path(), new).map_err(|error| {
                    EditorError::Io(format!(
                        "failed modifying '{}': {error}",
                        planned.relative_path
                    ))
                })?;
                Ok(())
            }
            FileChange::Delete(_) => {
                if planned.absolute_path.exists() {
                    fs::remove_file(planned.absolute_path.as_path()).map_err(|error| {
                        EditorError::Io(format!(
                            "failed deleting '{}': {error}",
                            planned.relative_path
                        ))
                    })?;
                }
                Ok(())
            }
        }
    }
}

fn resolve_path(project_root: &Path, relative_path: &str) -> Result<PathBuf, EditorError> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(EditorError::PathEscape(relative_path.to_string()));
    }
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(EditorError::PathEscape(relative_path.to_string()))
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }
    Ok(project_root.join(path))
}

fn read_backup(path: &Path) -> Result<Backup, EditorError> {
    if !path.exists() {
        return Ok(Backup {
            existed: false,
            bytes: Vec::new(),
        });
    }
    let bytes = fs::read(path).map_err(|error| {
        EditorError::Io(format!("failed backing up '{}': {error}", path.display()))
    })?;
    Ok(Backup {
        existed: true,
        bytes,
    })
}

fn rollback(project_root: &Path, backups: &HashMap<String, Backup>) -> Result<(), EditorError> {
    for (relative_path, backup) in backups {
        let absolute = resolve_path(project_root, relative_path.as_str())?;
        if backup.existed {
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    EditorError::Io(format!(
                        "failed creating rollback parent '{}': {error}",
                        parent.display()
                    ))
                })?;
            }
            fs::write(absolute.as_path(), backup.bytes.as_slice()).map_err(|error| {
                EditorError::Io(format!(
                    "failed restoring '{}': {error}",
                    absolute.display()
                ))
            })?;
        } else if absolute.exists() {
            fs::remove_file(absolute.as_path()).map_err(|error| {
                EditorError::Io(format!(
                    "failed removing rolled-back file '{}': {error}",
                    absolute.display()
                ))
            })?;
        }
    }
    Ok(())
}

fn file_version(path: &Path) -> Option<u128> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_nanos())
}

fn unified_diff(path: &str, old: &str, new: &str) -> String {
    let mut diff = String::new();
    diff.push_str(format!("--- a/{path}\n").as_str());
    diff.push_str(format!("+++ b/{path}\n").as_str());
    diff.push_str("@@\n");
    for line in old.lines() {
        diff.push_str(format!("-{line}\n").as_str());
    }
    for line in new.lines() {
        diff.push_str(format!("+{line}\n").as_str());
    }
    diff
}
