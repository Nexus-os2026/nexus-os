//! Mesh peer discovery — find other Nexus OS instances on the network.

use super::MeshError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of a discovered peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    Discovered,
    Connected,
    Authenticated,
    Disconnected,
    Unreachable,
}

/// Information about a peer Nexus OS instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: Uuid,
    pub address: String,
    pub port: u16,
    pub name: String,
    pub discovered_at: u64,
    pub last_seen: u64,
    pub status: PeerStatus,
    pub capabilities: Vec<String>,
}

/// Discovers and tracks peer Nexus OS instances in the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshDiscovery {
    local_id: Uuid,
    peers: HashMap<Uuid, PeerInfo>,
}

impl MeshDiscovery {
    /// Create a new discovery service for the given local instance.
    pub fn new(local_id: Uuid) -> Self {
        Self {
            local_id,
            peers: HashMap::new(),
        }
    }

    /// Simulate mDNS-style local discovery by returning any configured peers.
    ///
    /// In a real deployment this would broadcast/listen on the local network;
    /// here it simply returns whatever peers have been added.
    pub fn discover_local(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }

    /// Register a new peer in the discovery table.
    pub fn add_peer(&mut self, peer: PeerInfo) -> Result<(), MeshError> {
        if peer.peer_id == self.local_id {
            return Err(MeshError::PeerAlreadyExists(
                "cannot add self as peer".into(),
            ));
        }
        if self.peers.contains_key(&peer.peer_id) {
            return Err(MeshError::PeerAlreadyExists(peer.peer_id.to_string()));
        }
        self.peers.insert(peer.peer_id, peer);
        Ok(())
    }

    /// Remove a peer from the discovery table.
    pub fn remove_peer(&mut self, peer_id: &Uuid) -> Result<PeerInfo, MeshError> {
        self.peers
            .remove(peer_id)
            .ok_or_else(|| MeshError::PeerNotFound(peer_id.to_string()))
    }

    /// List all known peers.
    pub fn list_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }

    /// Update the status of an existing peer.
    pub fn update_peer_status(
        &mut self,
        peer_id: &Uuid,
        status: PeerStatus,
        timestamp: u64,
    ) -> Result<(), MeshError> {
        let peer = self
            .peers
            .get_mut(peer_id)
            .ok_or_else(|| MeshError::PeerNotFound(peer_id.to_string()))?;
        peer.status = status;
        peer.last_seen = timestamp;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(name: &str) -> PeerInfo {
        PeerInfo {
            peer_id: Uuid::new_v4(),
            address: "192.168.1.10".into(),
            port: 9000,
            name: name.into(),
            discovered_at: 1000,
            last_seen: 1000,
            status: PeerStatus::Discovered,
            capabilities: vec!["compute".into(), "storage".into()],
        }
    }

    #[test]
    fn add_and_list_peers() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);

        let p1 = make_peer("node-alpha");
        let p2 = make_peer("node-beta");
        let id1 = p1.peer_id;

        disc.add_peer(p1).unwrap();
        disc.add_peer(p2).unwrap();

        assert_eq!(disc.list_peers().len(), 2);
        assert_eq!(disc.discover_local().len(), 2);

        disc.remove_peer(&id1).unwrap();
        assert_eq!(disc.list_peers().len(), 1);
    }

    #[test]
    fn cannot_add_self() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);
        let mut p = make_peer("self");
        p.peer_id = local;
        assert!(disc.add_peer(p).is_err());
    }

    #[test]
    fn duplicate_peer_rejected() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);
        let p = make_peer("dup");
        let p2 = p.clone();
        disc.add_peer(p).unwrap();
        assert!(disc.add_peer(p2).is_err());
    }

    #[test]
    fn update_status() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);
        let p = make_peer("node");
        let id = p.peer_id;
        disc.add_peer(p).unwrap();

        disc.update_peer_status(&id, PeerStatus::Connected, 2000)
            .unwrap();
        let peer = disc.peers.get(&id).unwrap();
        assert_eq!(peer.status, PeerStatus::Connected);
        assert_eq!(peer.last_seen, 2000);
    }

    #[test]
    fn update_unknown_peer_fails() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);
        let result = disc.update_peer_status(&Uuid::new_v4(), PeerStatus::Disconnected, 3000);
        assert!(result.is_err());
    }

    #[test]
    fn remove_unknown_peer_fails() {
        let local = Uuid::new_v4();
        let mut disc = MeshDiscovery::new(local);
        assert!(disc.remove_peer(&Uuid::new_v4()).is_err());
    }

    #[test]
    fn peer_info_serde_roundtrip() {
        let p = make_peer("serde-test");
        let json = serde_json::to_string(&p).unwrap();
        let back: PeerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "serde-test");
        assert_eq!(back.status, PeerStatus::Discovered);
    }
}
