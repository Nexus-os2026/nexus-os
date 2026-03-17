//! Distributed Consciousness Mesh — multiple Nexus OS instances form one shared mind.
//!
//! Sub-modules handle peer discovery, state synchronisation, agent migration,
//! distributed task execution, and a shared knowledge graph.

pub mod discovery;
pub mod execution;
pub mod migration;
pub mod shared_memory;
pub mod sync;

pub use discovery::{MeshDiscovery, PeerInfo, PeerStatus};
pub use execution::{DistributedExecutor, DistributedTask, SubTask, TaskStatus};
pub use migration::{AgentMigration, MigrationPackage, MigrationStatus};
pub use shared_memory::{SharedEntry, SharedMemory, Visibility};
pub use sync::{ConsciousnessSync, SyncDelta, SyncState};

use thiserror::Error;

/// Errors that can occur within the consciousness mesh.
#[derive(Debug, Clone, PartialEq, Eq, Error, serde::Serialize, serde::Deserialize)]
pub enum MeshError {
    #[error("peer not found: {0}")]
    PeerNotFound(String),
    #[error("peer unreachable: {0}")]
    PeerUnreachable(String),
    #[error("sync conflict on agent {0}: {1}")]
    SyncConflict(String, String),
    #[error("migration failed for agent {0}: {1}")]
    MigrationFailed(String, String),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("task execution failed: {0}")]
    TaskFailed(String),
    #[error("shared memory error: {0}")]
    SharedMemoryError(String),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("peer already exists: {0}")]
    PeerAlreadyExists(String),
    #[error("not authorized: {0}")]
    NotAuthorized(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_error_display() {
        let err = MeshError::PeerNotFound("abc-123".into());
        assert_eq!(err.to_string(), "peer not found: abc-123");
    }

    #[test]
    fn mesh_error_serde_roundtrip() {
        let err = MeshError::ChecksumMismatch {
            expected: "aaa".into(),
            actual: "bbb".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let back: MeshError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }
}
