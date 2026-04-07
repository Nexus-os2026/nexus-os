use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agent::AgentAction;
use crate::learning::pattern::match_score;

/// A record of a past agent run for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub task: String,
    pub steps: Vec<MemoryStep>,
    pub success: bool,
    pub total_duration_ms: u64,
    pub fuel_consumed: u64,
    pub timestamp: DateTime<Utc>,
}

/// A single step within a memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStep {
    pub step_number: u32,
    pub actions: Vec<AgentAction>,
    pub screenshot_hash: String,
    pub app_context: String,
    pub duration_ms: u64,
}

/// In-memory store of past agent runs
pub struct ActionMemory {
    entries: Vec<MemoryEntry>,
    max_entries: usize,
    file_path: PathBuf,
}

impl ActionMemory {
    /// Create a new action memory with given path and max entries
    pub fn new(file_path: PathBuf, max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            file_path,
        }
    }

    /// Create with default path and capacity
    pub fn with_default_path() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let path = PathBuf::from(home).join(".nexus").join("agent_memory.json");
        Self::new(path, 1000)
    }

    /// Load entries from disk
    pub fn load(&mut self) -> Result<(), String> {
        if !self.file_path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read memory file: {e}"))?;
        match serde_json::from_str(&data) {
            Ok(entries) => {
                self.entries = entries;
                Ok(())
            }
            Err(e) => {
                warn!("Corrupted memory file, starting fresh: {e}");
                self.entries = Vec::new();
                Ok(())
            }
        }
    }

    /// Save entries to disk
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        let data = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| format!("Failed to serialize memory: {e}"))?;
        std::fs::write(&self.file_path, data)
            .map_err(|e| format!("Failed to write memory file: {e}"))?;
        Ok(())
    }

    /// Record a new entry, evicting the oldest if over max capacity
    pub fn record(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Find entries with similar task descriptions
    pub fn find_similar(&self, task: &str) -> Vec<&MemoryEntry> {
        let mut scored: Vec<(&MemoryEntry, f32)> = self
            .entries
            .iter()
            .map(|e| (e, match_score(task, &e.task)))
            .filter(|(_, s)| *s > 0.3)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(e, _)| e).collect()
    }

    /// Get success rate for tasks matching a pattern
    pub fn get_success_rate(&self, task_pattern: &str) -> f32 {
        let similar: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| match_score(task_pattern, &e.task) > 0.3)
            .collect();
        if similar.is_empty() {
            return 0.0;
        }
        let successes = similar.iter().filter(|e| e.success).count();
        successes as f32 / similar.len() as f32
    }

    /// Total lifetime actions across all entries
    pub fn total_actions(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| e.steps.iter().map(|s| s.actions.len() as u64).sum::<u64>())
            .sum()
    }

    /// Total lifetime fuel consumed
    pub fn total_fuel(&self) -> u64 {
        self.entries.iter().map(|e| e.fuel_consumed).sum()
    }

    /// Get all entries
    pub fn entries(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(task: &str, success: bool, fuel: u64) -> MemoryEntry {
        MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            task: task.to_string(),
            steps: vec![MemoryStep {
                step_number: 1,
                actions: vec![
                    AgentAction::Click {
                        x: 100,
                        y: 200,
                        button: "left".to_string(),
                    },
                    AgentAction::Type {
                        text: "hello".to_string(),
                    },
                ],
                screenshot_hash: "abc123".to_string(),
                app_context: "Terminal".to_string(),
                duration_ms: 500,
            }],
            success,
            total_duration_ms: 1000,
            fuel_consumed: fuel,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_memory_entry_creation() {
        let entry = make_entry("run cargo test", true, 100);
        assert_eq!(entry.task, "run cargo test");
        assert!(entry.success);
        assert_eq!(entry.fuel_consumed, 100);
    }

    #[test]
    fn test_memory_record_and_retrieve() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 100);

        mem.record(make_entry("task a", true, 50));
        mem.record(make_entry("task b", false, 30));

        assert_eq!(mem.len(), 2);
        assert_eq!(mem.entries()[0].task, "task a");
        assert_eq!(mem.entries()[1].task, "task b");
    }

    #[test]
    fn test_memory_max_entries_eviction() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 3);

        for i in 0..5 {
            mem.record(make_entry(&format!("task {i}"), true, 10));
        }

        assert_eq!(mem.len(), 3);
        // Oldest entries (0, 1) should be evicted
        assert_eq!(mem.entries()[0].task, "task 2");
    }

    #[test]
    fn test_memory_find_similar() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 100);

        mem.record(make_entry("run cargo test", true, 50));
        mem.record(make_entry("open browser chrome", true, 30));
        mem.record(make_entry("run cargo build", true, 40));

        let similar = mem.find_similar("run cargo test");
        assert!(!similar.is_empty());
        assert_eq!(similar[0].task, "run cargo test");
    }

    #[test]
    fn test_memory_success_rate() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 100);

        mem.record(make_entry("run cargo test", true, 50));
        mem.record(make_entry("run cargo test again", true, 50));
        mem.record(make_entry("run cargo test suite", false, 50));

        let rate = mem.get_success_rate("run cargo test");
        assert!(rate > 0.5);
    }

    #[test]
    fn test_memory_save_load() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");

        let mut mem = ActionMemory::new(path.clone(), 100);
        mem.record(make_entry("task a", true, 50));
        mem.record(make_entry("task b", false, 30));
        mem.save().expect("save");

        let mut mem2 = ActionMemory::new(path, 100);
        mem2.load().expect("load");
        assert_eq!(mem2.len(), 2);
    }

    #[test]
    fn test_memory_total_actions() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 100);

        // Each entry has 1 step with 2 actions
        mem.record(make_entry("task a", true, 50));
        mem.record(make_entry("task b", true, 30));

        assert_eq!(mem.total_actions(), 4);
    }

    #[test]
    fn test_memory_total_fuel() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("memory.json");
        let mut mem = ActionMemory::new(path, 100);

        mem.record(make_entry("task a", true, 50));
        mem.record(make_entry("task b", true, 30));

        assert_eq!(mem.total_fuel(), 80);
    }
}
