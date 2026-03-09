//! Distributed governance layer for cross-node audit replication, membership, and transport.

pub mod device_pairing;
pub mod gossip;
pub mod immutable_audit;
pub mod membership;
pub mod node;
pub mod quorum;
pub mod replication;
pub mod tcp_transport;
pub mod transport;
pub mod verification;
