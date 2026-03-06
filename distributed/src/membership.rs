//! Membership management with heartbeat-based failure detection.

use crate::node::{ClusterView, NodeIdentity, NodeState};
use crate::transport::{LocalTransport, Message, MessageKind, Transport};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MembershipConfig {
    pub heartbeat_interval_ms: u64,
    pub suspect_after_missed: u32,
    pub down_after_missed: u32,
}

impl Default for MembershipConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: 1000,
            suspect_after_missed: 3,
            down_after_missed: 5,
        }
    }
}

#[derive(Debug)]
pub struct MembershipManager {
    local_id: Uuid,
    config: MembershipConfig,
    cluster: ClusterView,
    heartbeat_counters: HashMap<Uuid, u32>,
    transport: LocalTransport,
}

impl MembershipManager {
    pub fn new(
        local_identity: NodeIdentity,
        config: MembershipConfig,
        quorum_size: usize,
        transport: LocalTransport,
    ) -> Self {
        let local_id = local_identity.id;
        let mut cluster = ClusterView::new(quorum_size);
        cluster.add_node(local_identity, NodeState::Active);

        Self {
            local_id,
            config,
            cluster,
            heartbeat_counters: HashMap::new(),
            transport,
        }
    }

    pub fn join(&mut self, identity: NodeIdentity) {
        let node_id = identity.id;
        self.cluster.add_node(identity, NodeState::Joining);
        self.heartbeat_counters.insert(node_id, 0);
        self.cluster.set_state(node_id, NodeState::Active);

        let payload = node_id.as_bytes().to_vec();
        let _ = self.transport.send(Message {
            from: self.local_id,
            to: node_id,
            kind: MessageKind::JoinRequest,
            payload,
        });
    }

    pub fn leave(&mut self, node_id: Uuid) {
        self.cluster.set_state(node_id, NodeState::Leaving);
        self.heartbeat_counters.remove(&node_id);
        self.cluster.remove_node(node_id);

        let payload = node_id.as_bytes().to_vec();
        let _ = self
            .transport
            .broadcast(self.local_id, MessageKind::LeaveNotice, payload);
    }

    pub fn heartbeat(&mut self, from_node: Uuid) {
        if let Some(counter) = self.heartbeat_counters.get_mut(&from_node) {
            *counter = 0;
        }
        if self.cluster.get_state(from_node) == Some(NodeState::Suspect) {
            self.cluster.set_state(from_node, NodeState::Active);
        }
    }

    /// Increment missed-heartbeat counters and update node states.
    pub fn check_health(&mut self) {
        let suspect_threshold = self.config.suspect_after_missed;
        let down_threshold = self.config.down_after_missed;

        let node_ids: Vec<Uuid> = self.heartbeat_counters.keys().copied().collect();
        for node_id in node_ids {
            if let Some(counter) = self.heartbeat_counters.get_mut(&node_id) {
                *counter += 1;
                let missed = *counter;

                if missed >= down_threshold {
                    self.cluster.set_state(node_id, NodeState::Down);
                } else if missed >= suspect_threshold {
                    self.cluster.set_state(node_id, NodeState::Suspect);
                }
            }
        }
    }

    pub fn active_nodes(&self) -> Vec<&NodeIdentity> {
        self.cluster
            .members
            .iter()
            .filter(|(_, s)| *s == NodeState::Active)
            .map(|(n, _)| n)
            .collect()
    }

    pub fn cluster_view(&self) -> &ClusterView {
        &self.cluster
    }

    pub fn local_id(&self) -> Uuid {
        self.local_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeIdentity;
    use std::net::SocketAddr;

    fn make_node(name: &str) -> NodeIdentity {
        NodeIdentity {
            id: Uuid::new_v4(),
            name: name.to_string(),
            addr: "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            public_key: vec![0; 32],
            capabilities: vec!["audit".to_string()],
            joined_at: 1000,
        }
    }

    #[test]
    fn join_and_leave_lifecycle() {
        let transport = LocalTransport::new();
        let local = make_node("local");
        let local_id = local.id;
        transport.register_node(local_id);

        let peer = make_node("peer");
        let peer_id = peer.id;
        transport.register_node(peer_id);

        let mut mgr = MembershipManager::new(local, MembershipConfig::default(), 1, transport);

        mgr.join(peer.clone());
        assert_eq!(mgr.active_nodes().len(), 2);

        mgr.leave(peer_id);
        assert_eq!(mgr.active_nodes().len(), 1);
        assert!(mgr.cluster_view().get_state(peer_id).is_none());
    }

    #[test]
    fn heartbeat_failure_detection() {
        let transport = LocalTransport::new();
        let local = make_node("local");
        transport.register_node(local.id);

        let peer = make_node("peer");
        let peer_id = peer.id;
        transport.register_node(peer_id);

        let config = MembershipConfig {
            heartbeat_interval_ms: 100,
            suspect_after_missed: 2,
            down_after_missed: 4,
        };

        let mut mgr = MembershipManager::new(local, config, 1, transport);
        mgr.join(peer.clone());

        // Miss 2 heartbeats -> Suspect
        mgr.check_health();
        mgr.check_health();
        assert_eq!(
            mgr.cluster_view().get_state(peer_id),
            Some(NodeState::Suspect)
        );

        // Heartbeat recovers to Active
        mgr.heartbeat(peer_id);
        assert_eq!(
            mgr.cluster_view().get_state(peer_id),
            Some(NodeState::Active)
        );

        // Miss 4 heartbeats -> Down
        mgr.check_health();
        mgr.check_health();
        mgr.check_health();
        mgr.check_health();
        assert_eq!(mgr.cluster_view().get_state(peer_id), Some(NodeState::Down));
    }

    // Re-create the peer node with same id for the join call
    fn make_node_with_id(name: &str, id: Uuid) -> NodeIdentity {
        NodeIdentity {
            id,
            name: name.to_string(),
            addr: "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            public_key: vec![0; 32],
            capabilities: vec!["audit".to_string()],
            joined_at: 1000,
        }
    }

    #[test]
    fn heartbeat_resets_suspect_to_active() {
        let transport = LocalTransport::new();
        let local = make_node("local");
        transport.register_node(local.id);

        let peer = make_node("peer");
        let peer_id = peer.id;
        transport.register_node(peer_id);

        let config = MembershipConfig {
            heartbeat_interval_ms: 100,
            suspect_after_missed: 1,
            down_after_missed: 3,
        };

        let mut mgr = MembershipManager::new(local, config, 1, transport);
        let peer_copy = make_node_with_id("peer", peer_id);
        mgr.join(peer_copy);

        mgr.check_health();
        assert_eq!(
            mgr.cluster_view().get_state(peer_id),
            Some(NodeState::Suspect)
        );

        mgr.heartbeat(peer_id);
        assert_eq!(
            mgr.cluster_view().get_state(peer_id),
            Some(NodeState::Active)
        );
    }
}
