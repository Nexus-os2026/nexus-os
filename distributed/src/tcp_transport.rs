//! Real TCP networking for distributed governance layer.
//!
//! Length-prefixed framing, exponential backoff reconnection, and wire message
//! serialization for node-to-node communication.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Retry policy with exponential backoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_retries: u32,
}

impl RetryPolicy {
    pub fn next_delay(&self, retry_count: u32) -> Duration {
        let delay = self
            .base_delay_ms
            .saturating_mul(2u64.saturating_pow(retry_count));
        let capped = delay.min(self.max_delay_ms);
        Duration::from_millis(capped)
    }

    pub fn should_retry(&self, count: u32) -> bool {
        count < self.max_retries
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
            max_retries: 5,
        }
    }
}

/// Configuration for TCP connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub max_retries: u32,
    pub base_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
}

impl ConnectionConfig {
    pub fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy {
            base_delay_ms: self.base_retry_delay_ms,
            max_delay_ms: self.max_retry_delay_ms,
            max_retries: self.max_retries,
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: 5,
            read_timeout_secs: 10,
            max_retries: 5,
            base_retry_delay_ms: 1000,
            max_retry_delay_ms: 30_000,
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Wire message types for node-to-node communication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMessageType {
    Heartbeat,
    AuditSync,
    QuorumPropose,
    QuorumVote,
    ReplicationFull,
    ReplicationDelta,
    AuthChallenge,
    AuthResponse,
}

/// A framed message sent over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMessage {
    pub message_id: Uuid,
    pub sender_node_id: Uuid,
    pub message_type: WireMessageType,
    pub timestamp: u64,
    pub payload: serde_json::Value,
}

impl WireMessage {
    pub fn new(
        message_type: WireMessageType,
        sender_node_id: Uuid,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            message_id: Uuid::new_v4(),
            sender_node_id,
            message_type,
            timestamp: unix_now(),
            payload,
        }
    }

    pub fn heartbeat(sender: Uuid) -> Self {
        Self::new(WireMessageType::Heartbeat, sender, serde_json::json!({}))
    }
}

/// Serialize a WireMessage into a length-prefixed frame.
/// Format: 4-byte big-endian length prefix + JSON payload bytes.
pub fn frame_message(msg: &WireMessage) -> Result<Vec<u8>, TcpTransportError> {
    let json_bytes = serde_json::to_vec(msg).map_err(|e| TcpTransportError::Serialization {
        details: e.to_string(),
    })?;
    let len = json_bytes.len() as u32;
    let mut frame = Vec::with_capacity(4 + json_bytes.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&json_bytes);
    Ok(frame)
}

/// Read a length-prefixed frame from a reader and deserialize into a WireMessage.
pub fn read_framed_message<R: Read>(reader: &mut R) -> Result<WireMessage, TcpTransportError> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .map_err(|e| TcpTransportError::Io {
            details: e.to_string(),
        })?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        return Err(TcpTransportError::FrameTooLarge { size: len });
    }

    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| TcpTransportError::Io {
            details: e.to_string(),
        })?;

    serde_json::from_slice(&payload).map_err(|e| TcpTransportError::Serialization {
        details: e.to_string(),
    })
}

/// State of a connection to a remote node.
#[derive(Debug)]
pub struct NodeConnection {
    pub node_id: Uuid,
    pub addr: SocketAddr,
    pub stream: Option<TcpStream>,
    pub connected: bool,
    pub last_attempt: Option<Instant>,
    pub retry_count: u32,
    pub auth_verified: bool,
}

impl NodeConnection {
    fn new(node_id: Uuid, addr: SocketAddr) -> Self {
        Self {
            node_id,
            addr,
            stream: None,
            connected: false,
            last_attempt: None,
            retry_count: 0,
            auth_verified: false,
        }
    }
}

/// Errors from the TCP transport layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TcpTransportError {
    Io { details: String },
    Serialization { details: String },
    NotConnected { node_id: Uuid },
    ConnectionFailed { addr: String, details: String },
    FrameTooLarge { size: usize },
    BindFailed { addr: String, details: String },
}

impl std::fmt::Display for TcpTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { details } => write!(f, "IO error: {details}"),
            Self::Serialization { details } => write!(f, "serialization error: {details}"),
            Self::NotConnected { node_id } => write!(f, "not connected to node {node_id}"),
            Self::ConnectionFailed { addr, details } => {
                write!(f, "connection to {addr} failed: {details}")
            }
            Self::FrameTooLarge { size } => write!(f, "frame too large: {size} bytes"),
            Self::BindFailed { addr, details } => {
                write!(f, "bind to {addr} failed: {details}")
            }
        }
    }
}

/// TCP transport manager for node-to-node communication.
#[derive(Debug)]
pub struct TcpTransportManager {
    pub node_id: Uuid,
    pub config: ConnectionConfig,
    connections: HashMap<Uuid, NodeConnection>,
    listener: Option<TcpListener>,
}

impl TcpTransportManager {
    pub fn new(node_id: Uuid, config: ConnectionConfig) -> Self {
        Self {
            node_id,
            config,
            connections: HashMap::new(),
            listener: None,
        }
    }

    /// Bind a TCP listener on the given address.
    pub fn bind(&mut self, addr: SocketAddr) -> Result<(), TcpTransportError> {
        let listener = TcpListener::bind(addr).map_err(|e| TcpTransportError::BindFailed {
            addr: addr.to_string(),
            details: e.to_string(),
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|e| TcpTransportError::Io {
                details: e.to_string(),
            })?;
        self.listener = Some(listener);
        Ok(())
    }

    /// Connect to a remote node at the given address.
    pub fn connect(
        &mut self,
        remote_node_id: Uuid,
        addr: SocketAddr,
    ) -> Result<(), TcpTransportError> {
        let timeout = Duration::from_secs(self.config.connect_timeout_secs);
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| {
            TcpTransportError::ConnectionFailed {
                addr: addr.to_string(),
                details: e.to_string(),
            }
        })?;

        let read_timeout = Duration::from_secs(self.config.read_timeout_secs);
        stream
            .set_read_timeout(Some(read_timeout))
            .map_err(|e| TcpTransportError::Io {
                details: e.to_string(),
            })?;

        let mut conn = NodeConnection::new(remote_node_id, addr);
        conn.stream = Some(stream);
        conn.connected = true;
        conn.last_attempt = Some(Instant::now());
        conn.retry_count = 0;
        self.connections.insert(remote_node_id, conn);
        Ok(())
    }

    /// Send a message to a specific connected node.
    pub fn send_message(
        &mut self,
        target: Uuid,
        msg: &WireMessage,
    ) -> Result<(), TcpTransportError> {
        let conn = self
            .connections
            .get_mut(&target)
            .ok_or(TcpTransportError::NotConnected { node_id: target })?;

        if !conn.connected {
            return Err(TcpTransportError::NotConnected { node_id: target });
        }

        let frame = frame_message(msg)?;
        let stream = conn
            .stream
            .as_mut()
            .ok_or(TcpTransportError::NotConnected { node_id: target })?;

        stream
            .write_all(&frame)
            .map_err(|e| TcpTransportError::Io {
                details: e.to_string(),
            })?;
        stream.flush().map_err(|e| TcpTransportError::Io {
            details: e.to_string(),
        })?;

        Ok(())
    }

    /// Receive a message from a specific connected node.
    pub fn recv_message(&mut self, source: Uuid) -> Result<WireMessage, TcpTransportError> {
        let conn = self
            .connections
            .get_mut(&source)
            .ok_or(TcpTransportError::NotConnected { node_id: source })?;

        if !conn.connected {
            return Err(TcpTransportError::NotConnected { node_id: source });
        }

        let stream = conn
            .stream
            .as_mut()
            .ok_or(TcpTransportError::NotConnected { node_id: source })?;

        read_framed_message(stream)
    }

    /// Accept an incoming connection from the listener.
    pub fn accept_connection(&mut self, remote_node_id: Uuid) -> Result<(), TcpTransportError> {
        let listener = self.listener.as_ref().ok_or(TcpTransportError::Io {
            details: "no listener bound".to_string(),
        })?;

        let (stream, addr) = listener.accept().map_err(|e| TcpTransportError::Io {
            details: e.to_string(),
        })?;

        let read_timeout = Duration::from_secs(self.config.read_timeout_secs);
        stream
            .set_read_timeout(Some(read_timeout))
            .map_err(|e| TcpTransportError::Io {
                details: e.to_string(),
            })?;

        let mut conn = NodeConnection::new(remote_node_id, addr);
        conn.stream = Some(stream);
        conn.connected = true;
        self.connections.insert(remote_node_id, conn);
        Ok(())
    }

    /// Broadcast a message to all connected nodes.
    pub fn broadcast_message(&mut self, msg: &WireMessage) -> Vec<(Uuid, TcpTransportError)> {
        let frame = match frame_message(msg) {
            Ok(f) => f,
            Err(e) => {
                let ids: Vec<Uuid> = self.connections.keys().copied().collect();
                return ids.into_iter().map(|id| (id, e.clone())).collect();
            }
        };

        let mut errors = Vec::new();

        for (node_id, conn) in &mut self.connections {
            if !conn.connected {
                errors.push((
                    *node_id,
                    TcpTransportError::NotConnected { node_id: *node_id },
                ));
                continue;
            }

            if let Some(stream) = conn.stream.as_mut() {
                if let Err(e) = stream.write_all(&frame).and_then(|_| stream.flush()) {
                    errors.push((
                        *node_id,
                        TcpTransportError::Io {
                            details: e.to_string(),
                        },
                    ));
                }
            } else {
                errors.push((
                    *node_id,
                    TcpTransportError::NotConnected { node_id: *node_id },
                ));
            }
        }

        errors
    }

    /// Disconnect from a specific node.
    pub fn disconnect(&mut self, node_id: Uuid) {
        if let Some(mut conn) = self.connections.remove(&node_id) {
            conn.connected = false;
            // Drop the stream by removing it
            conn.stream.take();
        }
    }

    /// Attempt to reconnect to a previously known node.
    pub fn reconnect(&mut self, node_id: Uuid) -> Result<(), TcpTransportError> {
        let addr = self
            .connections
            .get(&node_id)
            .map(|c| c.addr)
            .ok_or(TcpTransportError::NotConnected { node_id })?;

        let retry_count = self
            .connections
            .get(&node_id)
            .map(|c| c.retry_count)
            .unwrap_or(0);

        if retry_count >= self.config.max_retries {
            return Err(TcpTransportError::ConnectionFailed {
                addr: addr.to_string(),
                details: format!("max retries ({}) exceeded", self.config.max_retries),
            });
        }

        // Update retry count before attempting
        if let Some(conn) = self.connections.get_mut(&node_id) {
            conn.retry_count += 1;
            conn.last_attempt = Some(Instant::now());
        }

        let timeout = Duration::from_secs(self.config.connect_timeout_secs);
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                let read_timeout = Duration::from_secs(self.config.read_timeout_secs);
                let _ = stream.set_read_timeout(Some(read_timeout));

                if let Some(conn) = self.connections.get_mut(&node_id) {
                    conn.stream = Some(stream);
                    conn.connected = true;
                    conn.retry_count = 0;
                }
                Ok(())
            }
            Err(e) => Err(TcpTransportError::ConnectionFailed {
                addr: addr.to_string(),
                details: e.to_string(),
            }),
        }
    }

    /// List all currently connected node IDs.
    pub fn connected_nodes(&self) -> Vec<Uuid> {
        self.connections
            .iter()
            .filter(|(_, c)| c.connected)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Check if a specific node is connected.
    pub fn is_connected(&self, node_id: Uuid) -> bool {
        self.connections
            .get(&node_id)
            .map(|c| c.connected)
            .unwrap_or(false)
    }

    /// Get the bound listener address.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn retry_policy_exponential_backoff() {
        let policy = RetryPolicy {
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
            max_retries: 5,
        };

        assert_eq!(policy.next_delay(0), Duration::from_millis(1000));
        assert_eq!(policy.next_delay(1), Duration::from_millis(2000));
        assert_eq!(policy.next_delay(2), Duration::from_millis(4000));
        assert_eq!(policy.next_delay(3), Duration::from_millis(8000));
        assert_eq!(policy.next_delay(4), Duration::from_millis(16000));
        // Capped at max
        assert_eq!(policy.next_delay(5), Duration::from_millis(30_000));
        assert_eq!(policy.next_delay(10), Duration::from_millis(30_000));
    }

    #[test]
    fn retry_policy_should_retry() {
        let policy = RetryPolicy {
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
            max_retries: 3,
        };

        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(10));
    }

    #[test]
    fn wire_message_round_trip() {
        let sender = Uuid::new_v4();
        let msg = WireMessage::new(
            WireMessageType::AuditSync,
            sender,
            json!({"events": [1, 2, 3]}),
        );

        let serialized = serde_json::to_vec(&msg).unwrap();
        let deserialized: WireMessage = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(deserialized.message_id, msg.message_id);
        assert_eq!(deserialized.message_type, WireMessageType::AuditSync);
        assert_eq!(deserialized.sender_node_id, sender);
        assert_eq!(deserialized.timestamp, msg.timestamp);
        assert_eq!(deserialized.payload, json!({"events": [1, 2, 3]}));
    }

    #[test]
    fn framing_round_trip() {
        let sender = Uuid::new_v4();
        let msg = WireMessage::new(
            WireMessageType::Heartbeat,
            sender,
            json!({"status": "alive"}),
        );

        let frame = frame_message(&msg).unwrap();

        // First 4 bytes are length prefix
        let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(len, frame.len() - 4);

        // Read it back
        let mut cursor = Cursor::new(frame);
        let decoded = read_framed_message(&mut cursor).unwrap();

        assert_eq!(decoded.message_id, msg.message_id);
        assert_eq!(decoded.message_type, WireMessageType::Heartbeat);
        assert_eq!(decoded.sender_node_id, sender);
    }

    #[test]
    fn frame_too_large_rejected() {
        // Craft a frame with length > 16MB
        let mut fake_frame = Vec::new();
        let big_len: u32 = 17 * 1024 * 1024;
        fake_frame.extend_from_slice(&big_len.to_be_bytes());
        // Don't need to add payload — it should reject on length alone
        fake_frame.extend_from_slice(&[0u8; 64]);

        let mut cursor = Cursor::new(fake_frame);
        let result = read_framed_message(&mut cursor);
        assert!(matches!(
            result,
            Err(TcpTransportError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn default_connection_config() {
        let config = ConnectionConfig::default();
        assert_eq!(config.connect_timeout_secs, 5);
        assert_eq!(config.read_timeout_secs, 10);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_retry_delay_ms, 1000);
        assert_eq!(config.max_retry_delay_ms, 30_000);
    }

    #[test]
    fn heartbeat_message_convenience() {
        let sender = Uuid::new_v4();
        let msg = WireMessage::heartbeat(sender);
        assert_eq!(msg.message_type, WireMessageType::Heartbeat);
        assert_eq!(msg.sender_node_id, sender);
        assert!(msg.timestamp > 0);
        assert_eq!(msg.payload, json!({}));
    }

    #[test]
    fn all_wire_message_types_serialize() {
        let types = vec![
            WireMessageType::Heartbeat,
            WireMessageType::AuditSync,
            WireMessageType::QuorumPropose,
            WireMessageType::QuorumVote,
            WireMessageType::ReplicationFull,
            WireMessageType::ReplicationDelta,
            WireMessageType::AuthChallenge,
            WireMessageType::AuthResponse,
        ];
        let sender = Uuid::new_v4();

        for msg_type in types {
            let msg = WireMessage::new(msg_type.clone(), sender, json!(null));
            let frame = frame_message(&msg).unwrap();
            let mut cursor = Cursor::new(frame);
            let decoded = read_framed_message(&mut cursor).unwrap();
            assert_eq!(decoded.message_type, msg_type);
        }
    }

    #[test]
    fn localhost_tcp_send_recv() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let config = ConnectionConfig::default();

        // Manager A listens
        let mut mgr_a = TcpTransportManager::new(node_a, config.clone());
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        mgr_a.bind(addr).unwrap();
        let listen_addr = mgr_a.local_addr().unwrap();

        // Manager B connects to A
        let mut mgr_b = TcpTransportManager::new(node_b, config);
        mgr_b.connect(node_a, listen_addr).unwrap();

        // A accepts the connection
        // Give the OS a moment to register the connection
        std::thread::sleep(Duration::from_millis(50));
        mgr_a.accept_connection(node_b).unwrap();

        assert!(mgr_b.is_connected(node_a));
        assert!(mgr_a.is_connected(node_b));

        // B sends a message to A
        let msg = WireMessage::new(
            WireMessageType::AuditSync,
            node_b,
            json!({"data": "hello from B"}),
        );
        mgr_b.send_message(node_a, &msg).unwrap();

        // A receives
        let received = mgr_a.recv_message(node_b).unwrap();
        assert_eq!(received.message_type, WireMessageType::AuditSync);
        assert_eq!(received.sender_node_id, node_b);
        assert_eq!(received.payload["data"], "hello from B");
    }

    #[test]
    fn broadcast_sends_to_all_connected() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let node_c = Uuid::new_v4();
        let config = ConnectionConfig::default();

        // A listens
        let mut mgr_a = TcpTransportManager::new(node_a, config.clone());
        mgr_a.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr_a = mgr_a.local_addr().unwrap();

        // B and C connect to A
        let mut mgr_b = TcpTransportManager::new(node_b, config.clone());
        mgr_b.connect(node_a, addr_a).unwrap();

        std::thread::sleep(Duration::from_millis(50));
        mgr_a.accept_connection(node_b).unwrap();

        let mut mgr_c = TcpTransportManager::new(node_c, config);
        mgr_c.connect(node_a, addr_a).unwrap();

        std::thread::sleep(Duration::from_millis(50));
        mgr_a.accept_connection(node_c).unwrap();

        assert_eq!(mgr_a.connected_nodes().len(), 2);

        // A broadcasts
        let msg = WireMessage::new(
            WireMessageType::Heartbeat,
            node_a,
            json!({"broadcast": true}),
        );
        let errors = mgr_a.broadcast_message(&msg);
        assert!(errors.is_empty(), "broadcast errors: {:?}", errors);

        // B and C both receive
        let from_b = mgr_b.recv_message(node_a).unwrap();
        assert_eq!(from_b.message_type, WireMessageType::Heartbeat);

        let from_c = mgr_c.recv_message(node_a).unwrap();
        assert_eq!(from_c.message_type, WireMessageType::Heartbeat);
    }

    #[test]
    fn disconnect_removes_node() {
        let node_a = Uuid::new_v4();
        let node_b = Uuid::new_v4();
        let config = ConnectionConfig::default();

        let mut mgr_a = TcpTransportManager::new(node_a, config.clone());
        mgr_a.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr_a = mgr_a.local_addr().unwrap();

        let mut mgr_b = TcpTransportManager::new(node_b, config);
        mgr_b.connect(node_a, addr_a).unwrap();

        std::thread::sleep(Duration::from_millis(50));
        mgr_a.accept_connection(node_b).unwrap();

        assert!(mgr_a.is_connected(node_b));
        mgr_a.disconnect(node_b);
        assert!(!mgr_a.is_connected(node_b));
        assert!(mgr_a.connected_nodes().is_empty());
    }

    #[test]
    fn send_to_unconnected_node_fails() {
        let node_a = Uuid::new_v4();
        let unknown = Uuid::new_v4();
        let config = ConnectionConfig::default();

        let mut mgr = TcpTransportManager::new(node_a, config);
        let msg = WireMessage::heartbeat(node_a);

        let result = mgr.send_message(unknown, &msg);
        assert!(matches!(
            result,
            Err(TcpTransportError::NotConnected { .. })
        ));
    }

    #[test]
    fn recv_from_unconnected_node_fails() {
        let node_a = Uuid::new_v4();
        let unknown = Uuid::new_v4();
        let config = ConnectionConfig::default();

        let mut mgr = TcpTransportManager::new(node_a, config);
        let result = mgr.recv_message(unknown);
        assert!(matches!(
            result,
            Err(TcpTransportError::NotConnected { .. })
        ));
    }
}
