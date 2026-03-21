//! Persistent schedule storage — survives app restarts.
//!
//! Uses a JSON file on disk. Schedules are loaded on startup and persisted on every mutation.

use super::error::SchedulerError;
use super::trigger::{ScheduleEntry, ScheduleId};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Mutex;

/// File-backed schedule persistence.
pub struct ScheduleStore {
    schedules: Mutex<Vec<ScheduleEntry>>,
    file_path: PathBuf,
}

impl ScheduleStore {
    /// Create a new store, loading any persisted schedules from `data_dir/schedules.json`.
    pub fn new(data_dir: &std::path::Path) -> Self {
        let file_path = data_dir.join("schedules.json");
        let schedules = if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
                Err(e) => {
                    eprintln!("[scheduler] failed to load schedules: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Self {
            schedules: Mutex::new(schedules),
            file_path,
        }
    }

    /// Add a new schedule entry. Returns its ID.
    pub fn add(&self, entry: ScheduleEntry) -> Result<ScheduleId, SchedulerError> {
        let id = entry.id;
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        schedules.push(entry);
        self.persist(&schedules)?;
        Ok(id)
    }

    /// Remove a schedule by ID.
    pub fn remove(&self, id: &ScheduleId) -> Result<bool, SchedulerError> {
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        let before = schedules.len();
        schedules.retain(|s| s.id != *id);
        let removed = schedules.len() < before;
        self.persist(&schedules)?;
        Ok(removed)
    }

    /// Update an existing schedule entry in place.
    pub fn update(&self, entry: ScheduleEntry) -> Result<bool, SchedulerError> {
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = schedules.iter_mut().find(|s| s.id == entry.id) {
            *existing = entry;
            self.persist(&schedules)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get a schedule by ID.
    pub fn get(&self, id: &ScheduleId) -> Option<ScheduleEntry> {
        let schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        schedules.iter().find(|s| s.id == *id).cloned()
    }

    /// List all schedules.
    pub fn list(&self) -> Vec<ScheduleEntry> {
        let schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        schedules.clone()
    }

    /// List only enabled schedules.
    pub fn list_enabled(&self) -> Vec<ScheduleEntry> {
        let schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        schedules.iter().filter(|s| s.enabled).cloned().collect()
    }

    /// Enable a schedule.
    pub fn enable(&self, id: &ScheduleId) -> Result<bool, SchedulerError> {
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = schedules.iter_mut().find(|s| s.id == *id) {
            entry.enabled = true;
            self.persist(&schedules)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Disable a schedule.
    pub fn disable(&self, id: &ScheduleId) -> Result<bool, SchedulerError> {
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = schedules.iter_mut().find(|s| s.id == *id) {
            entry.enabled = false;
            self.persist(&schedules)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Record a completed run: update last_run, bump run_count, set next_run.
    /// Auto-disables if max_runs reached.
    pub fn record_run(
        &self,
        id: &ScheduleId,
        next_run: Option<DateTime<Utc>>,
    ) -> Result<(), SchedulerError> {
        let mut schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = schedules.iter_mut().find(|s| s.id == *id) {
            entry.last_run = Some(Utc::now());
            entry.next_run = next_run;
            entry.run_count += 1;

            if let Some(max) = entry.max_runs {
                if entry.run_count >= max {
                    entry.enabled = false;
                    eprintln!(
                        "[scheduler] {} reached max runs ({}), disabling",
                        entry.name, max
                    );
                }
            }
        }
        self.persist(&schedules)
    }

    /// Return the number of stored schedules.
    pub fn len(&self) -> usize {
        let schedules = self.schedules.lock().unwrap_or_else(|p| p.into_inner());
        schedules.len()
    }

    /// Returns true if no schedules are stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn persist(&self, schedules: &[ScheduleEntry]) -> Result<(), SchedulerError> {
        let json = serde_json::to_string_pretty(schedules)
            .map_err(|e| SchedulerError::Serialization(e.to_string()))?;
        std::fs::write(&self.file_path, json).map_err(|e| SchedulerError::Io(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::trigger::{ScheduledTask, TriggerType};

    fn sample_entry(name: &str) -> ScheduleEntry {
        ScheduleEntry::new(
            "agent-1".to_string(),
            name.to_string(),
            TriggerType::Interval { seconds: 300 },
            ScheduledTask {
                task_type: "run_agent".to_string(),
                parameters: serde_json::json!({"agent_did": "agent-1"}),
                timeout_seconds: 60,
            },
        )
    }

    #[test]
    fn add_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let entry = sample_entry("test-1");
        let id = entry.id;
        store.add(entry).unwrap();

        let retrieved = store.get(&id).unwrap();
        assert_eq!(retrieved.name, "test-1");
    }

    #[test]
    fn remove_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let entry = sample_entry("to-remove");
        let id = entry.id;
        store.add(entry).unwrap();
        assert_eq!(store.len(), 1);

        let removed = store.remove(&id).unwrap();
        assert!(removed);
        assert!(store.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let removed = store.remove(&uuid::Uuid::new_v4()).unwrap();
        assert!(!removed);
    }

    #[test]
    fn update_existing() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let mut entry = sample_entry("original");
        let id = entry.id;
        store.add(entry.clone()).unwrap();

        entry.name = "updated".to_string();
        let updated = store.update(entry).unwrap();
        assert!(updated);

        let retrieved = store.get(&id).unwrap();
        assert_eq!(retrieved.name, "updated");
    }

    #[test]
    fn enable_disable() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let entry = sample_entry("toggleable");
        let id = entry.id;
        store.add(entry).unwrap();

        store.disable(&id).unwrap();
        assert!(!store.get(&id).unwrap().enabled);
        assert!(store.list_enabled().is_empty());

        store.enable(&id).unwrap();
        assert!(store.get(&id).unwrap().enabled);
        assert_eq!(store.list_enabled().len(), 1);
    }

    #[test]
    fn record_run_increments_count() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let entry = sample_entry("counter");
        let id = entry.id;
        store.add(entry).unwrap();

        store.record_run(&id, None).unwrap();
        let updated = store.get(&id).unwrap();
        assert_eq!(updated.run_count, 1);
        assert!(updated.last_run.is_some());
    }

    #[test]
    fn max_runs_disables_schedule() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        let mut entry = sample_entry("limited");
        entry.max_runs = Some(2);
        let id = entry.id;
        store.add(entry).unwrap();

        store.record_run(&id, None).unwrap();
        assert!(store.get(&id).unwrap().enabled);

        store.record_run(&id, None).unwrap();
        assert!(!store.get(&id).unwrap().enabled);
    }

    #[test]
    fn persistence_survives_reload() {
        let dir = tempfile::tempdir().unwrap();
        let entry = sample_entry("persisted");
        let id = entry.id;

        {
            let store = ScheduleStore::new(dir.path());
            store.add(entry).unwrap();
        }

        // New store instance reads from disk
        let store2 = ScheduleStore::new(dir.path());
        let retrieved = store2.get(&id).unwrap();
        assert_eq!(retrieved.name, "persisted");
    }

    #[test]
    fn list_returns_all() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());
        store.add(sample_entry("a")).unwrap();
        store.add(sample_entry("b")).unwrap();
        store.add(sample_entry("c")).unwrap();
        assert_eq!(store.list().len(), 3);
    }
}
