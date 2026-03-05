use super::identity::NodeId;
use super::DistError;
use crate::audit::AuditEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationAck {
    pub accepted: bool,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncReport {
    pub peer: NodeId,
    pub synced_events: u64,
    pub mode: String,
}

pub trait EventReplicator {
    fn replicate_event(&mut self, event: &AuditEvent) -> Result<ReplicationAck, DistError>;
    fn receive_event(&mut self, from: &NodeId, event: AuditEvent) -> Result<(), DistError>;
    fn sync_state(&mut self, peer: &NodeId) -> Result<SyncReport, DistError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoOpReplicator {
    pub received_events: u64,
}

impl NoOpReplicator {
    pub fn new() -> Self {
        Self { received_events: 0 }
    }
}

impl EventReplicator for NoOpReplicator {
    fn replicate_event(&mut self, _event: &AuditEvent) -> Result<ReplicationAck, DistError> {
        Ok(ReplicationAck {
            accepted: true,
            mode: "local_only".to_string(),
        })
    }

    fn receive_event(&mut self, _from: &NodeId, _event: AuditEvent) -> Result<(), DistError> {
        self.received_events = self.received_events.saturating_add(1);
        Ok(())
    }

    fn sync_state(&mut self, peer: &NodeId) -> Result<SyncReport, DistError> {
        Ok(SyncReport {
            peer: peer.clone(),
            synced_events: 0,
            mode: "local_only".to_string(),
        })
    }
}
