//! Node identity, state tracking, and cluster view.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub id: Uuid,
    pub name: String,
    pub addr: SocketAddr,
    pub public_key: Vec<u8>,
    pub capabilities: Vec<String>,
    pub joined_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Joining,
    Active,
    Suspect,
    Down,
    Leaving,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterView {
    pub members: Vec<(NodeIdentity, NodeState)>,
    pub version: u64,
    pub quorum_size: usize,
}

impl ClusterView {
    pub fn new(quorum_size: usize) -> Self {
        Self {
            members: Vec::new(),
            version: 0,
            quorum_size,
        }
    }

    pub fn add_node(&mut self, identity: NodeIdentity, state: NodeState) {
        if !self.members.iter().any(|(n, _)| n.id == identity.id) {
            self.members.push((identity, state));
            self.version += 1;
        }
    }

    pub fn remove_node(&mut self, node_id: Uuid) {
        let before = self.members.len();
        self.members.retain(|(n, _)| n.id != node_id);
        if self.members.len() != before {
            self.version += 1;
        }
    }

    pub fn set_state(&mut self, node_id: Uuid, state: NodeState) {
        if let Some((_, s)) = self.members.iter_mut().find(|(n, _)| n.id == node_id) {
            *s = state;
            self.version += 1;
        }
    }

    pub fn get_state(&self, node_id: Uuid) -> Option<NodeState> {
        self.members
            .iter()
            .find(|(n, _)| n.id == node_id)
            .map(|(_, s)| *s)
    }

    pub fn active_count(&self) -> usize {
        self.members
            .iter()
            .filter(|(_, s)| *s == NodeState::Active)
            .count()
    }

    pub fn has_quorum(&self) -> bool {
        self.active_count() >= self.quorum_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(name: &str) -> NodeIdentity {
        NodeIdentity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            addr: "127.0.0.1:8000".parse().unwrap(),
            public_key: vec![0; 32],
            capabilities: vec!["audit".to_string()],
            joined_at: 1000,
        }
    }

    #[test]
    fn cluster_view_lifecycle() {
        let mut view = ClusterView::new(2);
        let n1 = test_node("node-1");
        let n2 = test_node("node-2");
        let n1_id = n1.id;
        let n2_id = n2.id;

        view.add_node(n1, NodeState::Active);
        view.add_node(n2, NodeState::Active);
        assert_eq!(view.version, 2);
        assert!(view.has_quorum());

        view.set_state(n1_id, NodeState::Down);
        assert_eq!(view.active_count(), 1);
        assert!(!view.has_quorum());

        view.remove_node(n2_id);
        assert_eq!(view.members.len(), 1);
    }
}
