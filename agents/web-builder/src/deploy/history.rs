//! Deploy History — persistent record of every deployment with status tracking.
//!
//! Each deploy gets an entry with build hash, file manifest, quality score.
//! The history tracks which deploy is Live vs Superseded vs RolledBack.
//! Max 50 entries per project — oldest are pruned.

use super::diff::{compute_diff, DeployDiff};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Maximum number of entries before pruning.
const MAX_ENTRIES: usize = 50;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Status of a deploy entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployStatus {
    Live,
    Superseded,
    RolledBack,
    Failed,
}

/// A single file in the deploy manifest (path + hash, no content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifestEntry {
    pub path: String,
    pub hash: String,
    pub size: u64,
}

/// A single deploy history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployHistoryEntry {
    pub id: String,
    pub deploy_id: String,
    pub provider: String,
    pub site_id: String,
    pub url: String,
    pub build_hash: String,
    pub timestamp: String,
    pub status: DeployStatus,
    pub quality_score: Option<u32>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub cost: f64,
    pub model_attribution: Vec<String>,
    pub files_manifest: Vec<FileManifestEntry>,
    pub signature: Option<String>,
}

/// Result of deploy drift check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployDriftStatus {
    InSync,
    Drifted { changes: usize },
    NeverDeployed,
}

/// The full deploy history for a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeployHistory {
    pub entries: Vec<DeployHistoryEntry>,
}

impl DeployHistory {
    /// Add a new deploy entry. Marks any previous Live entry as Superseded.
    /// Prunes oldest entries if exceeding MAX_ENTRIES.
    pub fn record_deploy(&mut self, entry: DeployHistoryEntry) {
        // Mark any existing Live entries as Superseded
        for e in &mut self.entries {
            if e.status == DeployStatus::Live {
                e.status = DeployStatus::Superseded;
            }
        }

        self.entries.push(entry);

        // Prune oldest if over limit
        while self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }

    /// Get the currently live deploy (if any).
    pub fn current(&self) -> Option<&DeployHistoryEntry> {
        self.entries
            .iter()
            .rfind(|e| e.status == DeployStatus::Live)
    }

    /// Get all entries, newest first.
    pub fn all_newest_first(&self) -> Vec<&DeployHistoryEntry> {
        let mut sorted: Vec<&DeployHistoryEntry> = self.entries.iter().collect();
        sorted.reverse();
        sorted
    }

    /// Get entry by ID.
    pub fn get(&self, id: &str) -> Option<&DeployHistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Record a rollback: mark `from_id` as RolledBack, mark `to_id` as Live.
    pub fn record_rollback(&mut self, from_id: &str, to_id: &str) {
        for e in &mut self.entries {
            if e.id == from_id {
                e.status = DeployStatus::RolledBack;
            }
            if e.id == to_id {
                e.status = DeployStatus::Live;
            }
        }
    }

    /// Compute diff between two deploys by their IDs.
    pub fn diff(&self, from_id: &str, to_id: &str) -> Option<DeployDiff> {
        let from = self.get(from_id)?;
        let to = self.get(to_id)?;
        Some(compute_diff(
            &from.files_manifest,
            &to.files_manifest,
            from_id,
            to_id,
            &from.build_hash,
            &to.build_hash,
        ))
    }
}

// ─── Drift Detection ────────────────────────────────────────────────────────

/// Check if the current build matches the live deploy.
pub fn check_deploy_drift(current_build_hash: &str, history: &DeployHistory) -> DeployDriftStatus {
    match history.current() {
        None => DeployDriftStatus::NeverDeployed,
        Some(live) => {
            if live.build_hash == current_build_hash {
                DeployDriftStatus::InSync
            } else {
                // Count changed files between current and live (approximate)
                DeployDriftStatus::Drifted { changes: 1 }
            }
        }
    }
}

// ─── Persistence ────────────────────────────────────────────────────────────

const HISTORY_FILE: &str = "deploy_history_v2.json";

/// Save deploy history to the project directory.
pub fn save_history(project_dir: &Path, history: &DeployHistory) -> Result<(), String> {
    let path = project_dir.join(HISTORY_FILE);
    let json =
        serde_json::to_string_pretty(history).map_err(|e| format!("serialize history: {e}"))?;
    let _ = std::fs::create_dir_all(project_dir);
    std::fs::write(&path, json).map_err(|e| format!("write history: {e}"))
}

/// Load deploy history from the project directory.
pub fn load_history(project_dir: &Path) -> DeployHistory {
    let path = project_dir.join(HISTORY_FILE);
    if !path.exists() {
        return DeployHistory::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, hash: &str, status: DeployStatus) -> DeployHistoryEntry {
        DeployHistoryEntry {
            id: id.into(),
            deploy_id: format!("dep-{id}"),
            provider: "netlify".into(),
            site_id: "site-1".into(),
            url: "https://test.netlify.app".into(),
            build_hash: hash.into(),
            timestamp: format!("2026-04-0{id}T12:00:00Z"),
            status,
            quality_score: Some(90),
            file_count: 10,
            total_bytes: 5000,
            cost: 0.10,
            model_attribution: vec!["sonnet".into()],
            files_manifest: vec![FileManifestEntry {
                path: "index.html".into(),
                hash: format!("filehash-{id}"),
                size: 500,
            }],
            signature: None,
        }
    }

    #[test]
    fn test_record_deploy_adds_entry() {
        let mut h = DeployHistory::default();
        let e = make_entry("1", "hash1", DeployStatus::Live);
        h.record_deploy(e);
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0].status, DeployStatus::Live);
    }

    #[test]
    fn test_record_deploy_marks_previous_superseded() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));
        assert_eq!(h.entries[0].status, DeployStatus::Superseded);
        assert_eq!(h.entries[1].status, DeployStatus::Live);
    }

    #[test]
    fn test_current_returns_live() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));
        let current = h.current().unwrap();
        assert_eq!(current.id, "2");
        assert_eq!(current.status, DeployStatus::Live);
    }

    #[test]
    fn test_all_returns_newest_first() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));
        h.record_deploy(make_entry("3", "hash3", DeployStatus::Live));
        let all = h.all_newest_first();
        assert_eq!(all[0].id, "3");
        assert_eq!(all[1].id, "2");
        assert_eq!(all[2].id, "1");
    }

    #[test]
    fn test_get_by_id() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));
        assert!(h.get("1").is_some());
        assert_eq!(h.get("1").unwrap().build_hash, "hash1");
        assert!(h.get("99").is_none());
    }

    #[test]
    fn test_record_rollback_updates_statuses() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));
        // Now "2" is Live, "1" is Superseded
        h.record_rollback("2", "1");
        assert_eq!(h.get("2").unwrap().status, DeployStatus::RolledBack);
        assert_eq!(h.get("1").unwrap().status, DeployStatus::Live);
    }

    #[test]
    fn test_max_entries_prunes_oldest() {
        let mut h = DeployHistory::default();
        for i in 0..55 {
            h.record_deploy(make_entry(
                &i.to_string(),
                &format!("hash{i}"),
                DeployStatus::Live,
            ));
        }
        assert_eq!(h.entries.len(), MAX_ENTRIES);
        // Oldest (0-4) should have been pruned
        assert!(h.get("0").is_none());
        assert!(h.get("4").is_none());
        assert!(h.get("5").is_some());
    }

    #[test]
    fn test_empty_history() {
        let h = DeployHistory::default();
        assert!(h.current().is_none());
        assert!(h.all_newest_first().is_empty());
    }

    #[test]
    fn test_drift_in_sync() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "abc123", DeployStatus::Live));
        assert_eq!(check_deploy_drift("abc123", &h), DeployDriftStatus::InSync);
    }

    #[test]
    fn test_drift_drifted() {
        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "abc123", DeployStatus::Live));
        assert!(matches!(
            check_deploy_drift("xyz789", &h),
            DeployDriftStatus::Drifted { .. }
        ));
    }

    #[test]
    fn test_drift_never_deployed() {
        let h = DeployHistory::default();
        assert_eq!(
            check_deploy_drift("abc123", &h),
            DeployDriftStatus::NeverDeployed
        );
    }

    #[test]
    fn test_save_and_load_history() {
        let dir = std::env::temp_dir().join(format!("nexus-dh-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut h = DeployHistory::default();
        h.record_deploy(make_entry("1", "hash1", DeployStatus::Live));
        h.record_deploy(make_entry("2", "hash2", DeployStatus::Live));

        save_history(&dir, &h).unwrap();
        let loaded = load_history(&dir);
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].status, DeployStatus::Superseded);
        assert_eq!(loaded.entries[1].status, DeployStatus::Live);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
