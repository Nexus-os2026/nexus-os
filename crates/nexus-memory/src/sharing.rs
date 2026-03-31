//! Cross-agent memory sharing with taint tracking.
//!
//! When Agent B reads memories shared by Agent A, any knowledge B derives
//! carries a taint marker.  If A revokes access, B's tainted entries are
//! flagged for review.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::*;

/// Taint marker tracking cross-agent information flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaintMarker {
    /// Agent that originally shared the memory.
    pub source_agent_id: String,
    /// The specific memory entry that was shared.
    pub source_memory_id: MemoryId,
    /// When the tainted entry was created.
    pub inherited_at: DateTime<Utc>,
    /// Propagation distance (1 = directly derived, 2+ = transitive).
    pub distance: u32,
}

/// Result of revoking a share.
#[derive(Debug, Clone)]
pub struct RevocationResult {
    /// Entry IDs that carry taint from the revoked source.
    pub tainted_entry_ids: Vec<MemoryId>,
    /// Number of taint markers removed.
    pub taint_markers_removed: usize,
}

/// Tracks all sharing relationships and taint propagation.
pub struct SharingManager {
    /// Active shares: (owner, grantee) → access.
    active_shares: HashMap<(String, String), MemoryAccess>,
    /// Taint records: entry_id → taint markers.
    taint_records: HashMap<MemoryId, Vec<TaintMarker>>,
    /// Maximum taint propagation depth.
    max_taint_depth: u32,
    /// Default share TTL in days.
    default_share_ttl_days: u64,
    /// Share creation timestamps for expiry tracking.
    share_timestamps: HashMap<(String, String), DateTime<Utc>>,
}

impl SharingManager {
    /// Creates a new sharing manager with defaults.
    pub fn new() -> Self {
        Self::with_config(3, 30)
    }

    /// Creates a sharing manager with custom config.
    pub fn with_config(max_taint_depth: u32, default_share_ttl_days: u64) -> Self {
        Self {
            active_shares: HashMap::new(),
            taint_records: HashMap::new(),
            max_taint_depth,
            default_share_ttl_days,
            share_timestamps: HashMap::new(),
        }
    }

    /// Registers a share between two agents.
    pub fn register_share(&mut self, owner_id: &str, grantee_id: &str, access: MemoryAccess) {
        let key = (owner_id.to_string(), grantee_id.to_string());
        self.share_timestamps.insert(key.clone(), Utc::now());
        self.active_shares.insert(key, access);
    }

    /// Revokes a share and returns IDs of tainted entries.
    pub fn revoke_share(&mut self, owner_id: &str, grantee_id: &str) -> RevocationResult {
        let key = (owner_id.to_string(), grantee_id.to_string());
        self.active_shares.remove(&key);
        self.share_timestamps.remove(&key);

        // Find all tainted entries from this source
        let mut tainted_ids = Vec::new();
        let mut markers_removed = 0usize;

        for (entry_id, markers) in &mut self.taint_records {
            let before = markers.len();
            markers.retain(|m| m.source_agent_id != owner_id);
            let removed = before - markers.len();
            if removed > 0 {
                tainted_ids.push(*entry_id);
                markers_removed += removed;
            }
        }

        // Clean up empty taint records
        self.taint_records.retain(|_, v| !v.is_empty());

        RevocationResult {
            tainted_entry_ids: tainted_ids,
            taint_markers_removed: markers_removed,
        }
    }

    /// Records a taint marker on an entry.
    pub fn record_taint(
        &mut self,
        entry_id: MemoryId,
        source_agent_id: &str,
        source_memory_id: MemoryId,
        distance: u32,
    ) -> Result<(), MemoryError> {
        if distance > self.max_taint_depth {
            return Err(MemoryError::ValidationError(format!(
                "Taint propagation depth {distance} exceeds maximum {}",
                self.max_taint_depth
            )));
        }

        let markers = self.taint_records.entry(entry_id).or_default();
        markers.push(TaintMarker {
            source_agent_id: source_agent_id.to_string(),
            source_memory_id,
            inherited_at: Utc::now(),
            distance,
        });
        Ok(())
    }

    /// Propagates taint from a source entry to a derived entry.
    pub fn propagate_taint(
        &mut self,
        derived_entry_id: MemoryId,
        source_entry_id: MemoryId,
    ) -> Result<(), MemoryError> {
        let source_markers = match self.taint_records.get(&source_entry_id) {
            Some(markers) => markers.clone(),
            None => return Ok(()), // source has no taint, nothing to propagate
        };

        for marker in source_markers {
            let new_distance = marker.distance + 1;
            if new_distance > self.max_taint_depth {
                continue; // silently stop propagation at max depth
            }
            self.record_taint(
                derived_entry_id,
                &marker.source_agent_id,
                marker.source_memory_id,
                new_distance,
            )?;
        }
        Ok(())
    }

    /// Returns taint markers for an entry.
    pub fn get_taint(&self, entry_id: MemoryId) -> Option<&Vec<TaintMarker>> {
        self.taint_records.get(&entry_id)
    }

    /// Returns `true` if the entry has any taint markers.
    pub fn is_tainted(&self, entry_id: MemoryId) -> bool {
        self.taint_records
            .get(&entry_id)
            .is_some_and(|v| !v.is_empty())
    }

    /// Returns `true` if the entry carries taint from the given agent.
    pub fn tainted_by(&self, entry_id: MemoryId, agent_id: &str) -> bool {
        self.taint_records
            .get(&entry_id)
            .is_some_and(|markers| markers.iter().any(|m| m.source_agent_id == agent_id))
    }

    /// Removes shares older than the TTL.  Returns expired (owner, grantee) pairs.
    pub fn cleanup_expired_shares(&mut self) -> Vec<(String, String)> {
        let cutoff = Utc::now() - chrono::Duration::days(self.default_share_ttl_days as i64);
        let mut expired = Vec::new();

        self.share_timestamps.retain(|key, ts| {
            if *ts < cutoff {
                expired.push(key.clone());
                false
            } else {
                true
            }
        });

        for key in &expired {
            self.active_shares.remove(key);
        }

        expired
    }

    /// Returns the number of active shares.
    pub fn active_share_count(&self) -> usize {
        self.active_shares.len()
    }

    /// Returns the number of entries with taint records.
    pub fn tainted_entry_count(&self) -> usize {
        self.taint_records.len()
    }

    /// Returns `true` if a share is active between owner and grantee.
    pub fn is_share_active(&self, owner_id: &str, grantee_id: &str) -> bool {
        self.active_shares
            .contains_key(&(owner_id.to_string(), grantee_id.to_string()))
    }
}

impl Default for SharingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn register_and_verify_share() {
        let mut mgr = SharingManager::new();
        mgr.register_share(
            "alice",
            "bob",
            MemoryAccess {
                read: vec![MemoryType::Semantic],
                write: vec![],
                search: true,
                share: false,
            },
        );
        assert!(mgr.is_share_active("alice", "bob"));
        assert_eq!(mgr.active_share_count(), 1);
    }

    #[test]
    fn revoke_returns_tainted_ids() {
        let mut mgr = SharingManager::new();
        mgr.register_share(
            "alice",
            "bob",
            MemoryAccess {
                read: vec![MemoryType::Semantic],
                write: vec![],
                search: false,
                share: false,
            },
        );

        let entry_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        mgr.record_taint(entry_id, "alice", source_id, 1).unwrap();

        let result = mgr.revoke_share("alice", "bob");
        assert!(result.tainted_entry_ids.contains(&entry_id));
        assert_eq!(result.taint_markers_removed, 1);
        assert!(!mgr.is_share_active("alice", "bob"));
    }

    #[test]
    fn record_taint_on_entry() {
        let mut mgr = SharingManager::new();
        let entry = Uuid::new_v4();
        let source = Uuid::new_v4();
        mgr.record_taint(entry, "alice", source, 1).unwrap();

        assert!(mgr.is_tainted(entry));
        assert!(mgr.tainted_by(entry, "alice"));
        assert!(!mgr.tainted_by(entry, "bob"));
    }

    #[test]
    fn propagate_taint_increments_distance() {
        let mut mgr = SharingManager::new();
        let source_entry = Uuid::new_v4();
        let derived_entry = Uuid::new_v4();
        let origin = Uuid::new_v4();

        mgr.record_taint(source_entry, "alice", origin, 1).unwrap();
        mgr.propagate_taint(derived_entry, source_entry).unwrap();

        let markers = mgr.get_taint(derived_entry).unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].distance, 2);
    }

    #[test]
    fn max_taint_depth_enforced() {
        let mut mgr = SharingManager::with_config(2, 30);
        let entry = Uuid::new_v4();
        let source = Uuid::new_v4();

        assert!(mgr.record_taint(entry, "alice", source, 1).is_ok());
        assert!(mgr.record_taint(entry, "alice", source, 2).is_ok());
        assert!(mgr.record_taint(entry, "alice", source, 3).is_err());
    }

    #[test]
    fn propagation_stops_at_max_depth() {
        let mut mgr = SharingManager::with_config(2, 30);
        let e1 = Uuid::new_v4();
        let e2 = Uuid::new_v4();
        let e3 = Uuid::new_v4();
        let origin = Uuid::new_v4();

        mgr.record_taint(e1, "alice", origin, 1).unwrap();
        mgr.propagate_taint(e2, e1).unwrap(); // distance=2
        mgr.propagate_taint(e3, e2).unwrap(); // distance=3, exceeds max — silently skipped

        assert!(mgr.is_tainted(e2));
        assert!(!mgr.is_tainted(e3)); // stopped at max depth
    }

    #[test]
    fn is_tainted_false_for_clean() {
        let mgr = SharingManager::new();
        assert!(!mgr.is_tainted(Uuid::new_v4()));
    }

    #[test]
    fn cleanup_expired_shares() {
        let mut mgr = SharingManager::with_config(3, 0); // 0-day TTL = expire immediately
        mgr.register_share(
            "a",
            "b",
            MemoryAccess {
                read: vec![],
                write: vec![],
                search: false,
                share: false,
            },
        );
        // Manually backdate the timestamp
        let key = ("a".to_string(), "b".to_string());
        mgr.share_timestamps
            .insert(key, Utc::now() - chrono::Duration::days(1));

        let expired = mgr.cleanup_expired_shares();
        assert_eq!(expired.len(), 1);
        assert_eq!(mgr.active_share_count(), 0);
    }

    #[test]
    fn multiple_taint_sources() {
        let mut mgr = SharingManager::new();
        let entry = Uuid::new_v4();
        mgr.record_taint(entry, "alice", Uuid::new_v4(), 1).unwrap();
        mgr.record_taint(entry, "bob", Uuid::new_v4(), 1).unwrap();

        let markers = mgr.get_taint(entry).unwrap();
        assert_eq!(markers.len(), 2);
        assert!(mgr.tainted_by(entry, "alice"));
        assert!(mgr.tainted_by(entry, "bob"));
    }

    #[test]
    fn tainted_entry_count() {
        let mut mgr = SharingManager::new();
        mgr.record_taint(Uuid::new_v4(), "a", Uuid::new_v4(), 1)
            .unwrap();
        mgr.record_taint(Uuid::new_v4(), "a", Uuid::new_v4(), 1)
            .unwrap();
        assert_eq!(mgr.tainted_entry_count(), 2);
    }
}
