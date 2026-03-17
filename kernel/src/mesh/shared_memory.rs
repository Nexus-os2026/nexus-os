//! Shared memory — distributed knowledge graph across the consciousness mesh.

use super::MeshError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Controls which peers can see a shared entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Only visible on the owning node.
    LocalOnly,
    /// Visible to every peer in the mesh.
    MeshShared,
    /// Visible only to the listed peers.
    PeerSpecific(Vec<Uuid>),
}

/// A single entry in the distributed knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub owner_peer: Uuid,
    pub visibility: Visibility,
    pub version: u64,
    pub updated_at: u64,
}

/// Distributed key-value knowledge graph shared across mesh peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemory {
    local_peer_id: Uuid,
    entries: HashMap<String, SharedEntry>,
}

impl SharedMemory {
    /// Create a new shared memory store for the local peer.
    pub fn new(local_peer_id: Uuid) -> Self {
        Self {
            local_peer_id,
            entries: HashMap::new(),
        }
    }

    /// Insert or update an entry in shared memory.
    pub fn share_entry(
        &mut self,
        key: &str,
        value: serde_json::Value,
        visibility: Visibility,
        timestamp: u64,
    ) -> SharedEntry {
        let version = self.entries.get(key).map(|e| e.version + 1).unwrap_or(1);

        let entry = SharedEntry {
            key: key.to_string(),
            value,
            owner_peer: self.local_peer_id,
            visibility,
            version,
            updated_at: timestamp,
        };
        self.entries.insert(key.to_string(), entry.clone());
        entry
    }

    /// Query entries visible to the given peer.  If `peer_id` is `None`, returns
    /// entries visible to the local peer.
    pub fn query_shared(&self, peer_id: Option<&Uuid>) -> Vec<&SharedEntry> {
        let viewer = peer_id.unwrap_or(&self.local_peer_id);
        self.entries
            .values()
            .filter(|e| match &e.visibility {
                Visibility::LocalOnly => e.owner_peer == *viewer,
                Visibility::MeshShared => true,
                Visibility::PeerSpecific(peers) => {
                    peers.contains(viewer) || e.owner_peer == *viewer
                }
            })
            .collect()
    }

    /// Produce a list of entries that should be replicated to the given peer,
    /// respecting visibility rules.
    pub fn replicate_to_peer(&self, peer_id: &Uuid) -> Vec<SharedEntry> {
        self.entries
            .values()
            .filter(|e| match &e.visibility {
                Visibility::LocalOnly => false,
                Visibility::MeshShared => true,
                Visibility::PeerSpecific(peers) => peers.contains(peer_id),
            })
            .cloned()
            .collect()
    }

    /// Return all entries regardless of visibility (admin view).
    pub fn get_all_shared(&self) -> Vec<&SharedEntry> {
        self.entries.values().collect()
    }

    /// Ingest an entry received from a remote peer (used during replication).
    pub fn ingest_remote_entry(&mut self, entry: SharedEntry) -> Result<(), MeshError> {
        if let Some(existing) = self.entries.get(&entry.key) {
            if entry.version <= existing.version {
                return Err(MeshError::SharedMemoryError(format!(
                    "stale version {} for key '{}' (current {})",
                    entry.version, entry.key, existing.version
                )));
            }
        }
        self.entries.insert(entry.key.clone(), entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_and_query() {
        let local = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        mem.share_entry(
            "greeting",
            serde_json::json!("hello"),
            Visibility::MeshShared,
            100,
        );
        mem.share_entry(
            "secret",
            serde_json::json!("shhh"),
            Visibility::LocalOnly,
            100,
        );

        // Local peer sees both
        let visible = mem.query_shared(None);
        assert_eq!(visible.len(), 2);

        // Remote peer sees only MeshShared
        let remote = Uuid::new_v4();
        let visible = mem.query_shared(Some(&remote));
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].key, "greeting");
    }

    #[test]
    fn peer_specific_visibility() {
        let local = Uuid::new_v4();
        let peer_a = Uuid::new_v4();
        let peer_b = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        mem.share_entry(
            "restricted",
            serde_json::json!("data"),
            Visibility::PeerSpecific(vec![peer_a]),
            200,
        );

        let a_view = mem.query_shared(Some(&peer_a));
        assert_eq!(a_view.len(), 1);

        let b_view = mem.query_shared(Some(&peer_b));
        assert_eq!(b_view.len(), 0);
    }

    #[test]
    fn version_increments() {
        let local = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        let e1 = mem.share_entry("k", serde_json::json!(1), Visibility::MeshShared, 100);
        assert_eq!(e1.version, 1);

        let e2 = mem.share_entry("k", serde_json::json!(2), Visibility::MeshShared, 200);
        assert_eq!(e2.version, 2);
    }

    #[test]
    fn replicate_to_peer_respects_visibility() {
        let local = Uuid::new_v4();
        let peer = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        mem.share_entry("public", serde_json::json!(1), Visibility::MeshShared, 100);
        mem.share_entry("private", serde_json::json!(2), Visibility::LocalOnly, 100);
        mem.share_entry(
            "targeted",
            serde_json::json!(3),
            Visibility::PeerSpecific(vec![peer]),
            100,
        );

        let replicated = mem.replicate_to_peer(&peer);
        assert_eq!(replicated.len(), 2);
        let keys: Vec<&str> = replicated.iter().map(|e| e.key.as_str()).collect();
        assert!(keys.contains(&"public"));
        assert!(keys.contains(&"targeted"));
    }

    #[test]
    fn ingest_remote_entry() {
        let local = Uuid::new_v4();
        let remote = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        let entry = SharedEntry {
            key: "remote-key".into(),
            value: serde_json::json!("remote-val"),
            owner_peer: remote,
            visibility: Visibility::MeshShared,
            version: 1,
            updated_at: 500,
        };
        mem.ingest_remote_entry(entry).unwrap();
        assert_eq!(mem.get_all_shared().len(), 1);
    }

    #[test]
    fn ingest_stale_entry_rejected() {
        let local = Uuid::new_v4();
        let remote = Uuid::new_v4();
        let mut mem = SharedMemory::new(local);

        mem.share_entry("k", serde_json::json!(1), Visibility::MeshShared, 100);

        let stale = SharedEntry {
            key: "k".into(),
            value: serde_json::json!("old"),
            owner_peer: remote,
            visibility: Visibility::MeshShared,
            version: 1,
            updated_at: 50,
        };
        assert!(mem.ingest_remote_entry(stale).is_err());
    }

    #[test]
    fn shared_entry_serde_roundtrip() {
        let entry = SharedEntry {
            key: "test".into(),
            value: serde_json::json!({"nested": true}),
            owner_peer: Uuid::new_v4(),
            visibility: Visibility::PeerSpecific(vec![Uuid::new_v4()]),
            version: 3,
            updated_at: 999,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: SharedEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key, "test");
        assert_eq!(back.version, 3);
    }
}
