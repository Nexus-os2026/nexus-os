//! Distributed governance layer for cross-node audit replication, membership, and transport.

pub mod bft_bridge;
pub mod device_pairing;
pub mod ghost_protocol;
pub mod global_network;
pub mod gossip;
pub mod immutable_audit;
pub mod membership;
pub mod node;
pub mod pbft;
pub mod quorum;
pub mod replication;
pub mod tcp_transport;
pub mod transport;
pub mod verification;
