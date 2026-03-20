//! Transport abstraction with in-memory implementation for testing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub from: Uuid,
    pub to: Uuid,
    pub kind: MessageKind,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageKind {
    Heartbeat,
    AuditEvent,
    FullSync,
    DeltaSync,
    JoinRequest,
    LeaveNotice,
    /// Announce latest chain state: latest_hash + sequence_number in payload.
    GossipAnnounce,
    /// Request blocks in a sequence range: from_sequence + to_sequence in payload.
    GossipRequestBlocks,
    /// Send serialized blocks in response to a request.
    GossipSendBlocks,
    /// Alert that a block at a given sequence has mismatched hash.
    GossipTamperAlert,
}

pub trait Transport: Send + Sync {
    fn send(&self, msg: Message) -> Result<(), TransportError>;
    fn recv(&self, node_id: Uuid) -> Result<Vec<Message>, TransportError>;
    fn broadcast(
        &self,
        from: Uuid,
        kind: MessageKind,
        payload: Vec<u8>,
    ) -> Result<(), TransportError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    NodeNotFound(Uuid),
    SendFailed(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::NodeNotFound(id) => write!(f, "node not found: {id}"),
            TransportError::SendFailed(reason) => write!(f, "send failed: {reason}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// In-memory transport for testing. Messages are stored per destination node.
#[derive(Debug, Clone)]
pub struct LocalTransport {
    inboxes: Arc<Mutex<HashMap<Uuid, Vec<Message>>>>,
    registered: Arc<Mutex<Vec<Uuid>>>,
}

impl LocalTransport {
    pub fn new() -> Self {
        Self {
            inboxes: Arc::new(Mutex::new(HashMap::new())),
            registered: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register_node(&self, node_id: Uuid) {
        let mut reg = self.registered.lock().unwrap_or_else(|p| p.into_inner());
        if !reg.contains(&node_id) {
            reg.push(node_id);
        }
        let mut inboxes = self.inboxes.lock().unwrap_or_else(|p| p.into_inner());
        inboxes.entry(node_id).or_default();
    }
}

impl Default for LocalTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for LocalTransport {
    fn send(&self, msg: Message) -> Result<(), TransportError> {
        let mut inboxes = self.inboxes.lock().unwrap_or_else(|p| p.into_inner());
        let inbox = inboxes
            .get_mut(&msg.to)
            .ok_or(TransportError::NodeNotFound(msg.to))?;
        inbox.push(msg);
        Ok(())
    }

    fn recv(&self, node_id: Uuid) -> Result<Vec<Message>, TransportError> {
        let mut inboxes = self.inboxes.lock().unwrap_or_else(|p| p.into_inner());
        let inbox = inboxes
            .get_mut(&node_id)
            .ok_or(TransportError::NodeNotFound(node_id))?;
        Ok(std::mem::take(inbox))
    }

    fn broadcast(
        &self,
        from: Uuid,
        kind: MessageKind,
        payload: Vec<u8>,
    ) -> Result<(), TransportError> {
        let registered = self
            .registered
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for node_id in registered {
            if node_id != from {
                self.send(Message {
                    from,
                    to: node_id,
                    kind: kind.clone(),
                    payload: payload.clone(),
                })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_transport_send_recv() {
        let transport = LocalTransport::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        transport.register_node(a);
        transport.register_node(b);

        transport
            .send(Message {
                from: a,
                to: b,
                kind: MessageKind::Heartbeat,
                payload: vec![1, 2, 3],
            })
            .unwrap();

        let msgs = transport.recv(b).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from, a);
        assert_eq!(msgs[0].kind, MessageKind::Heartbeat);
    }

    #[test]
    fn broadcast_reaches_all_except_sender() {
        let transport = LocalTransport::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        transport.register_node(a);
        transport.register_node(b);
        transport.register_node(c);

        transport
            .broadcast(a, MessageKind::AuditEvent, vec![42])
            .unwrap();

        assert_eq!(transport.recv(a).unwrap().len(), 0);
        assert_eq!(transport.recv(b).unwrap().len(), 1);
        assert_eq!(transport.recv(c).unwrap().len(), 1);
    }
}
