use super::DistError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeyBytes(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub id: NodeId,
    pub public_key: PublicKeyBytes,
    pub capabilities: Vec<String>,
}

pub trait NodeRegistry {
    fn register_self(&mut self, identity: NodeIdentity) -> Result<(), DistError>;
    fn discover_peers(&self) -> Result<Vec<NodeIdentity>, DistError>;
    fn verify_peer(&self, peer: &NodeId) -> Result<bool, DistError>;
}

#[derive(Debug, Clone, Default)]
pub struct LocalOnlyRegistry {
    self_identity: Option<NodeIdentity>,
}

impl LocalOnlyRegistry {
    pub fn new() -> Self {
        Self {
            self_identity: None,
        }
    }
}

impl NodeRegistry for LocalOnlyRegistry {
    fn register_self(&mut self, identity: NodeIdentity) -> Result<(), DistError> {
        self.self_identity = Some(identity);
        Ok(())
    }

    fn discover_peers(&self) -> Result<Vec<NodeIdentity>, DistError> {
        if let Some(identity) = &self.self_identity {
            return Ok(vec![identity.clone()]);
        }
        Ok(Vec::new())
    }

    fn verify_peer(&self, peer: &NodeId) -> Result<bool, DistError> {
        Ok(self
            .self_identity
            .as_ref()
            .map(|identity| identity.id == *peer)
            .unwrap_or(false))
    }
}
