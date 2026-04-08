//! Persistent improvement state — stores metrics, proposals, and system defaults.
//!
//! Data is stored at `~/.nexus/builder_improvement_store.json`. All local, no telemetry.

use crate::self_improve::metrics::ProjectMetrics;
use crate::self_improve::mod_types::SystemDefaults;
use crate::self_improve::proposer::Proposal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Types ─────────────────────────────────────────────────────────────────

/// Full improvement state persisted to disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImprovementStore {
    pub metrics: Vec<ProjectMetrics>,
    pub proposals: Vec<Proposal>,
    pub defaults: SystemDefaults,
    /// Monotonically increasing version — increments on each applied change.
    pub version: u32,
    /// Serialized previous defaults per applied proposal (for rollback).
    #[serde(default)]
    pub rollback_snapshots: std::collections::HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

// ─── Persistence ───────────────────────────────────────────────────────────

/// Default store path.
pub fn store_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home)
        .join(".nexus")
        .join("builder_improvement_store.json")
}

/// Load the improvement store from disk. Returns default if file doesn't exist.
pub fn load_store() -> Result<ImprovementStore, StoreError> {
    load_store_from(&store_path())
}

/// Load from a specific path.
pub fn load_store_from(path: &std::path::Path) -> Result<ImprovementStore, StoreError> {
    match std::fs::read_to_string(path) {
        Ok(json) => Ok(serde_json::from_str(&json)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ImprovementStore::default()),
        Err(e) => Err(StoreError::Io(e)),
    }
}

/// Save the improvement store to disk.
pub fn save_store(store: &ImprovementStore) -> Result<(), StoreError> {
    save_store_to(store, &store_path())
}

/// Save to a specific path.
pub fn save_store_to(store: &ImprovementStore, path: &std::path::Path) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(store)?;
    std::fs::write(path, json)?;
    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("nexus-si-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");

        let mut store = ImprovementStore {
            version: 3,
            ..Default::default()
        };
        store
            .defaults
            .content_prompt_hints
            .push("Use numbers".into());

        save_store_to(&store, &path).unwrap();
        let loaded = load_store_from(&path).unwrap();
        assert_eq!(loaded.version, 3);
        assert_eq!(loaded.defaults.content_prompt_hints, vec!["Use numbers"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_missing_file_returns_default() {
        let path = std::env::temp_dir().join("nonexistent-si-store.json");
        let store = load_store_from(&path).unwrap();
        assert_eq!(store.version, 0);
        assert!(store.metrics.is_empty());
    }

    #[test]
    fn test_version_tracks_changes() {
        let mut store = ImprovementStore::default();
        assert_eq!(store.version, 0);
        store.version += 1;
        assert_eq!(store.version, 1);
    }

    #[test]
    fn test_default_store_empty() {
        let store = ImprovementStore::default();
        assert!(store.metrics.is_empty());
        assert!(store.proposals.is_empty());
        assert!(store.defaults.palette_rankings.is_empty());
        assert_eq!(store.version, 0);
    }
}
