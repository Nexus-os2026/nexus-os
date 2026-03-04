use super::identity::NodeIdentity;
use super::DistError;

pub trait DiscoveryProtocol {
    fn announce(&mut self, identity: &NodeIdentity) -> Result<(), DistError>;
    fn listen(&self) -> Result<Vec<NodeIdentity>, DistError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoOpDiscovery;

impl NoOpDiscovery {
    pub fn new() -> Self {
        Self
    }
}

impl DiscoveryProtocol for NoOpDiscovery {
    fn announce(&mut self, _identity: &NodeIdentity) -> Result<(), DistError> {
        Ok(())
    }

    fn listen(&self) -> Result<Vec<NodeIdentity>, DistError> {
        Ok(Vec::new())
    }
}
