//! Global Agent Network — registry of network nodes for cross-device
//! agent coordination and future DID-based identity verification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node in the global agent network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNode {
    pub node_id: String,
    pub display_name: String,
    pub address: String,
    pub capabilities: Vec<String>,
    pub joined_at: u64,
    pub is_online: bool,
    /// Ed25519 public key hex for future DID verification.
    #[serde(default)]
    pub node_pubkey: Option<String>,
}

/// Registry managing known network nodes.
#[derive(Debug, Clone, Default)]
pub struct GlobalNetwork {
    nodes: HashMap<String, NetworkNode>,
}

impl GlobalNetwork {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Register a node.  If a node with the same `node_id` already exists
    /// and the incoming node carries a `node_pubkey`, the pubkey is updated.
    pub fn register_node(&mut self, node: NetworkNode) {
        if let Some(existing) = self.nodes.get_mut(&node.node_id) {
            if node.node_pubkey.is_some() {
                existing.node_pubkey = node.node_pubkey;
            }
        } else {
            self.nodes.insert(node.node_id.clone(), node);
        }
    }

    pub fn get_node(&self, node_id: &str) -> Option<&NetworkNode> {
        self.nodes.get(node_id)
    }

    pub fn remove_node(&mut self, node_id: &str) -> bool {
        self.nodes.remove(node_id).is_some()
    }

    pub fn list_nodes(&self) -> Vec<&NetworkNode> {
        self.nodes.values().collect()
    }

    pub fn online_count(&self) -> usize {
        self.nodes.values().filter(|n| n.is_online).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, pubkey: Option<&str>) -> NetworkNode {
        NetworkNode {
            node_id: id.to_string(),
            display_name: format!("Node {id}"),
            address: "127.0.0.1:9000".to_string(),
            capabilities: vec!["audit".to_string()],
            joined_at: 1000,
            is_online: true,
            node_pubkey: pubkey.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut net = GlobalNetwork::new();
        net.register_node(make_node("a", None));
        net.register_node(make_node("b", None));
        assert_eq!(net.list_nodes().len(), 2);
    }

    #[test]
    fn test_register_updates_pubkey() {
        let mut net = GlobalNetwork::new();
        net.register_node(make_node("a", None));
        assert!(net.get_node("a").unwrap().node_pubkey.is_none());

        // Re-register with pubkey → existing node gets updated
        net.register_node(make_node("a", Some("ed25519_abc123")));
        assert_eq!(
            net.get_node("a").unwrap().node_pubkey.as_deref(),
            Some("ed25519_abc123")
        );
    }

    #[test]
    fn test_node_pubkey_optional() {
        let mut net = GlobalNetwork::new();

        // Node without pubkey
        let node_no_key = make_node("no-key", None);
        net.register_node(node_no_key);
        assert!(net.get_node("no-key").unwrap().node_pubkey.is_none());

        // Node with pubkey
        let node_with_key = make_node("with-key", Some("abc123def456"));
        net.register_node(node_with_key);
        assert_eq!(
            net.get_node("with-key").unwrap().node_pubkey.as_deref(),
            Some("abc123def456")
        );
    }

    #[test]
    fn test_remove_node() {
        let mut net = GlobalNetwork::new();
        net.register_node(make_node("a", None));
        assert!(net.remove_node("a"));
        assert!(!net.remove_node("a"));
        assert_eq!(net.list_nodes().len(), 0);
    }

    #[test]
    fn test_online_count() {
        let mut net = GlobalNetwork::new();
        let mut offline = make_node("off", None);
        offline.is_online = false;
        net.register_node(make_node("on", None));
        net.register_node(offline);
        assert_eq!(net.online_count(), 1);
    }
}
