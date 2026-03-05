//! Distributed system scaffolding interfaces.
//!
//! This module intentionally provides interface and local-only defaults.
//! No networking, consensus, or replication transport is implemented here.

pub mod consensus;
pub mod discovery;
pub mod identity;
pub mod replication;

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DistError {
    NotImplemented,
    LocalOnly,
    NetworkUnavailable,
    PeerUnreachable(String),
    InvalidProposal(String),
    InvalidState(String),
}

impl Display for DistError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DistError::NotImplemented => write!(f, "distributed operation is not implemented"),
            DistError::LocalOnly => write!(f, "operation is local-only in current mode"),
            DistError::NetworkUnavailable => write!(f, "network is unavailable"),
            DistError::PeerUnreachable(peer) => write!(f, "peer '{peer}' is unreachable"),
            DistError::InvalidProposal(reason) => write!(f, "invalid proposal: {reason}"),
            DistError::InvalidState(reason) => write!(f, "invalid distributed state: {reason}"),
        }
    }
}

impl std::error::Error for DistError {}
