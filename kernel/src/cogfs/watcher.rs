//! File watcher — tracks directories and detects file changes for re-indexing.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for which paths and file types to watch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Directories to watch.
    pub paths: Vec<String>,
    /// File extensions to include (e.g., ["rs", "md", "txt"]).
    pub extensions: Vec<String>,
    /// Glob patterns to ignore (e.g., ["target/*", "node_modules/*"]).
    pub ignore_patterns: Vec<String>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            extensions: vec![
                "txt".into(),
                "md".into(),
                "rs".into(),
                "py".into(),
                "js".into(),
                "ts".into(),
                "json".into(),
                "toml".into(),
                "yaml".into(),
                "yml".into(),
                "html".into(),
                "css".into(),
            ],
            ignore_patterns: vec![
                "target/*".into(),
                "node_modules/*".into(),
                ".git/*".into(),
                "*.lock".into(),
            ],
        }
    }
}

/// A tracked file with modification metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    /// Absolute file path.
    pub path: String,
    /// Last known modification time.
    pub last_modified: DateTime<Utc>,
    /// Content hash at last index time.
    pub last_hash: Option<String>,
    /// Whether this file needs re-indexing.
    pub dirty: bool,
}

/// Change event detected by the watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Path of the changed file.
    pub path: String,
    /// Type of change.
    pub change_type: ChangeType,
    /// When the change was detected.
    pub detected_at: DateTime<Utc>,
}

/// Type of file change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// File was created.
    Created,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
}

/// Watches configured directories and tracks file changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWatcher {
    /// Current watch configuration.
    pub config: WatchConfig,
    /// Tracked files keyed by path.
    tracked: HashMap<String, TrackedFile>,
    /// Pending change events.
    pending_events: Vec<FileChangeEvent>,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new(WatchConfig::default())
    }
}

impl FileWatcher {
    /// Create a new file watcher with the given config.
    pub fn new(config: WatchConfig) -> Self {
        Self {
            config,
            tracked: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    /// Add a directory to the watch list.
    pub fn add_watch(&mut self, path: &str) {
        if !self.config.paths.contains(&path.to_string()) {
            self.config.paths.push(path.to_string());
        }
    }

    /// Remove a directory from the watch list.
    pub fn remove_watch(&mut self, path: &str) -> bool {
        let before = self.config.paths.len();
        self.config.paths.retain(|p| p != path);
        self.config.paths.len() < before
    }

    /// List all currently watched directories.
    pub fn list_watches(&self) -> &[String] {
        &self.config.paths
    }

    /// Report a file as seen with its current modification time.
    ///
    /// If the file is new or its modification time has changed, it is marked dirty
    /// and a change event is emitted.
    pub fn report_file(&mut self, path: &str, modified: DateTime<Utc>, content_hash: &str) {
        if let Some(tracked) = self.tracked.get_mut(path) {
            if tracked.last_hash.as_deref() != Some(content_hash) {
                tracked.last_modified = modified;
                tracked.last_hash = Some(content_hash.to_string());
                tracked.dirty = true;
                self.pending_events.push(FileChangeEvent {
                    id: Uuid::new_v4(),
                    path: path.to_string(),
                    change_type: ChangeType::Modified,
                    detected_at: Utc::now(),
                });
            }
        } else {
            self.tracked.insert(
                path.to_string(),
                TrackedFile {
                    path: path.to_string(),
                    last_modified: modified,
                    last_hash: Some(content_hash.to_string()),
                    dirty: true,
                },
            );
            self.pending_events.push(FileChangeEvent {
                id: Uuid::new_v4(),
                path: path.to_string(),
                change_type: ChangeType::Created,
                detected_at: Utc::now(),
            });
        }
    }

    /// Report a file as deleted.
    pub fn report_deleted(&mut self, path: &str) {
        if self.tracked.remove(path).is_some() {
            self.pending_events.push(FileChangeEvent {
                id: Uuid::new_v4(),
                path: path.to_string(),
                change_type: ChangeType::Deleted,
                detected_at: Utc::now(),
            });
        }
    }

    /// Mark a file as no longer dirty (after re-indexing).
    pub fn mark_clean(&mut self, path: &str) {
        if let Some(tracked) = self.tracked.get_mut(path) {
            tracked.dirty = false;
        }
    }

    /// Get all files that need re-indexing.
    pub fn dirty_files(&self) -> Vec<&TrackedFile> {
        self.tracked.values().filter(|t| t.dirty).collect()
    }

    /// Drain and return all pending change events.
    pub fn drain_events(&mut self) -> Vec<FileChangeEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Check whether a path should be watched based on extension and ignore patterns.
    pub fn should_watch(&self, path: &str) -> bool {
        // Check extension
        let ext_match = if self.config.extensions.is_empty() {
            true
        } else {
            path.rsplit('.')
                .next()
                .map(|ext| self.config.extensions.iter().any(|e| e == ext))
                .unwrap_or(false)
        };

        if !ext_match {
            return false;
        }

        // Check ignore patterns (simple glob matching)
        for pattern in &self.config.ignore_patterns {
            if Self::matches_glob(path, pattern) {
                return false;
            }
        }

        true
    }

    /// Simple glob matching (supports * as wildcard).
    fn matches_glob(path: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                if !prefix.is_empty() && !suffix.is_empty() {
                    return path.contains(prefix) && path.ends_with(suffix);
                } else if !prefix.is_empty() {
                    return path.contains(prefix);
                } else if !suffix.is_empty() {
                    return path.ends_with(suffix);
                }
                return true; // pattern is just "*"
            }
        }
        path.contains(pattern)
    }

    /// Total number of tracked files.
    pub fn tracked_count(&self) -> usize {
        self.tracked.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove_watch() {
        let mut watcher = FileWatcher::default();
        watcher.add_watch("/home/user/project");
        assert!(watcher
            .list_watches()
            .contains(&"/home/user/project".to_string()));

        watcher.add_watch("/home/user/project"); // duplicate
        assert_eq!(
            watcher
                .list_watches()
                .iter()
                .filter(|p| *p == "/home/user/project")
                .count(),
            1
        );

        assert!(watcher.remove_watch("/home/user/project"));
        assert!(!watcher
            .list_watches()
            .contains(&"/home/user/project".to_string()));
        assert!(!watcher.remove_watch("/nonexistent"));
    }

    #[test]
    fn test_report_new_file() {
        let mut watcher = FileWatcher::default();
        let now = Utc::now();
        watcher.report_file("src/main.rs", now, "abc123");

        assert_eq!(watcher.tracked_count(), 1);
        assert_eq!(watcher.dirty_files().len(), 1);
        assert_eq!(watcher.dirty_files()[0].path, "src/main.rs");

        let events = watcher.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].change_type, ChangeType::Created);
    }

    #[test]
    fn test_report_modified_file() {
        let mut watcher = FileWatcher::default();
        let now = Utc::now();
        watcher.report_file("src/main.rs", now, "abc123");
        watcher.mark_clean("src/main.rs");
        watcher.drain_events();

        // Report same hash — no change
        watcher.report_file("src/main.rs", now, "abc123");
        assert!(watcher.dirty_files().is_empty());
        assert!(watcher.drain_events().is_empty());

        // Report different hash — modified
        watcher.report_file("src/main.rs", now, "def456");
        assert_eq!(watcher.dirty_files().len(), 1);
        let events = watcher.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_report_deleted_file() {
        let mut watcher = FileWatcher::default();
        watcher.report_file("src/main.rs", Utc::now(), "abc");
        watcher.drain_events();

        watcher.report_deleted("src/main.rs");
        assert_eq!(watcher.tracked_count(), 0);
        let events = watcher.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].change_type, ChangeType::Deleted);
    }

    #[test]
    fn test_should_watch() {
        let watcher = FileWatcher::default();
        assert!(watcher.should_watch("src/main.rs"));
        assert!(watcher.should_watch("docs/readme.md"));
        assert!(!watcher.should_watch("image.png"));
        assert!(!watcher.should_watch("target/debug/binary"));
        assert!(!watcher.should_watch("node_modules/lodash/index.js"));
        assert!(!watcher.should_watch(".git/HEAD"));
        assert!(!watcher.should_watch("Cargo.lock"));
    }

    #[test]
    fn test_mark_clean() {
        let mut watcher = FileWatcher::default();
        watcher.report_file("a.rs", Utc::now(), "hash1");
        assert_eq!(watcher.dirty_files().len(), 1);

        watcher.mark_clean("a.rs");
        assert!(watcher.dirty_files().is_empty());
    }

    #[test]
    fn test_drain_events_clears() {
        let mut watcher = FileWatcher::default();
        watcher.report_file("a.rs", Utc::now(), "h1");
        watcher.report_file("b.rs", Utc::now(), "h2");
        let events = watcher.drain_events();
        assert_eq!(events.len(), 2);

        // Second drain should be empty
        let events2 = watcher.drain_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_delete_unknown_file_no_event() {
        let mut watcher = FileWatcher::default();
        watcher.report_deleted("nonexistent.rs");
        assert!(watcher.drain_events().is_empty());
    }

    #[test]
    fn test_default_config() {
        let config = WatchConfig::default();
        assert!(config.extensions.contains(&"rs".to_string()));
        assert!(config.extensions.contains(&"md".to_string()));
        assert!(config.ignore_patterns.contains(&"target/*".to_string()));
    }
}
