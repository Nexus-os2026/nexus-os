//! Distributed system scaffolding interfaces.
//!
//! This module intentionally provides interface and local-only defaults.
//! No networking, consensus, or replication transport is implemented here.

pub mod consensus;
pub mod discovery;
pub mod identity;
pub mod replication;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DistError {
    #[error("distributed operation is not implemented")]
    NotImplemented,
    #[error("operation is local-only in current mode")]
    LocalOnly,
    #[error("network is unavailable")]
    NetworkUnavailable,
    #[error("peer '{0}' is unreachable")]
    PeerUnreachable(String),
    #[error("invalid proposal: {0}")]
    InvalidProposal(String),
    #[error("invalid distributed state: {0}")]
    InvalidState(String),
}
