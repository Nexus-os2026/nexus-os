//! Agent migration — move a running agent between Nexus OS instances.

use super::MeshError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of an in-progress agent migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    Preparing,
    Transferring,
    Resuming,
    Complete,
    Failed,
}

/// Self-contained package that captures everything needed to resume an agent on
/// another node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPackage {
    pub agent_id: Uuid,
    pub genome: serde_json::Value,
    pub consciousness_state: serde_json::Value,
    pub task_context: serde_json::Value,
    pub conversation_history: Vec<String>,
    pub source_peer: Uuid,
    pub created_at: u64,
    pub checksum: String,
}

/// Manages agent migration lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMigration {
    local_peer_id: Uuid,
    migrations: HashMap<Uuid, MigrationStatus>,
}

impl AgentMigration {
    /// Create a new migration manager.
    pub fn new(local_peer_id: Uuid) -> Self {
        Self {
            local_peer_id,
            migrations: HashMap::new(),
        }
    }

    /// Begin preparing a migration for the given agent.
    pub fn prepare_migration(&mut self, agent_id: Uuid) -> Result<MigrationStatus, MeshError> {
        if self.migrations.contains_key(&agent_id) {
            return Err(MeshError::MigrationFailed(
                agent_id.to_string(),
                "migration already in progress".into(),
            ));
        }
        self.migrations.insert(agent_id, MigrationStatus::Preparing);
        Ok(MigrationStatus::Preparing)
    }

    /// Serialize an agent's full state into a `MigrationPackage`.
    pub fn serialize_agent(
        &mut self,
        agent_id: Uuid,
        genome: serde_json::Value,
        consciousness_state: serde_json::Value,
        task_context: serde_json::Value,
        conversation_history: Vec<String>,
        timestamp: u64,
    ) -> Result<MigrationPackage, MeshError> {
        // Must have been prepared first
        match self.migrations.get(&agent_id) {
            Some(MigrationStatus::Preparing) => {}
            Some(status) => {
                return Err(MeshError::MigrationFailed(
                    agent_id.to_string(),
                    format!("invalid status for serialization: {:?}", status),
                ));
            }
            None => {
                return Err(MeshError::MigrationFailed(
                    agent_id.to_string(),
                    "migration not prepared".into(),
                ));
            }
        }

        let package = MigrationPackage {
            agent_id,
            genome,
            consciousness_state,
            task_context,
            conversation_history,
            source_peer: self.local_peer_id,
            created_at: timestamp,
            checksum: String::new(), // filled below
        };

        let checksum = Self::compute_checksum(&package);
        let package = MigrationPackage {
            checksum,
            ..package
        };

        self.migrations
            .insert(agent_id, MigrationStatus::Transferring);
        Ok(package)
    }

    /// Deserialize a received migration package and mark it as resuming.
    pub fn deserialize_agent(
        &mut self,
        package: &MigrationPackage,
    ) -> Result<MigrationStatus, MeshError> {
        self.verify_migration(package)?;
        self.migrations
            .insert(package.agent_id, MigrationStatus::Resuming);
        Ok(MigrationStatus::Resuming)
    }

    /// Verify the integrity of a migration package via its checksum.
    pub fn verify_migration(&self, package: &MigrationPackage) -> Result<(), MeshError> {
        let mut check_copy = package.clone();
        check_copy.checksum = String::new();
        let expected = Self::compute_checksum(&check_copy);

        if expected != package.checksum {
            return Err(MeshError::ChecksumMismatch {
                expected,
                actual: package.checksum.clone(),
            });
        }
        Ok(())
    }

    /// Mark a migration as complete.
    pub fn complete_migration(&mut self, agent_id: &Uuid) -> Result<(), MeshError> {
        match self.migrations.get(agent_id) {
            Some(MigrationStatus::Resuming) => {
                self.migrations.insert(*agent_id, MigrationStatus::Complete);
                Ok(())
            }
            _ => Err(MeshError::MigrationFailed(
                agent_id.to_string(),
                "not in resuming state".into(),
            )),
        }
    }

    /// Get the current migration status for an agent.
    pub fn status(&self, agent_id: &Uuid) -> Option<&MigrationStatus> {
        self.migrations.get(agent_id)
    }

    // --- helpers ---

    fn compute_checksum(package: &MigrationPackage) -> String {
        let bytes = serde_json::to_vec(package).unwrap_or_default();
        let digest = Sha256::digest(&bytes);
        format!("{:x}", digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_migration_lifecycle() {
        let peer_a = Uuid::new_v4();
        let peer_b = Uuid::new_v4();
        let agent = Uuid::new_v4();

        let mut source = AgentMigration::new(peer_a);
        source.prepare_migration(agent).unwrap();

        let package = source
            .serialize_agent(
                agent,
                serde_json::json!({"type": "researcher"}),
                serde_json::json!({"mood": "focused"}),
                serde_json::json!({"current_task": "analysis"}),
                vec!["hello".into(), "world".into()],
                1000,
            )
            .unwrap();

        assert_eq!(source.status(&agent), Some(&MigrationStatus::Transferring));

        // Destination receives the package
        let mut dest = AgentMigration::new(peer_b);
        let status = dest.deserialize_agent(&package).unwrap();
        assert_eq!(status, MigrationStatus::Resuming);

        dest.complete_migration(&agent).unwrap();
        assert_eq!(dest.status(&agent), Some(&MigrationStatus::Complete));
    }

    #[test]
    fn verify_detects_tampered_package() {
        let peer = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let mut mig = AgentMigration::new(peer);
        mig.prepare_migration(agent).unwrap();

        let mut package = mig
            .serialize_agent(
                agent,
                serde_json::json!({}),
                serde_json::json!({}),
                serde_json::json!({}),
                vec![],
                500,
            )
            .unwrap();

        // Tamper
        package.genome = serde_json::json!({"type": "malicious"});
        let result = mig.verify_migration(&package);
        assert!(result.is_err());
    }

    #[test]
    fn double_prepare_rejected() {
        let peer = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let mut mig = AgentMigration::new(peer);
        mig.prepare_migration(agent).unwrap();
        assert!(mig.prepare_migration(agent).is_err());
    }

    #[test]
    fn serialize_without_prepare_rejected() {
        let peer = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let mut mig = AgentMigration::new(peer);
        let result = mig.serialize_agent(
            agent,
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            vec![],
            0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn migration_package_serde_roundtrip() {
        let pkg = MigrationPackage {
            agent_id: Uuid::new_v4(),
            genome: serde_json::json!({"x": 1}),
            consciousness_state: serde_json::json!({}),
            task_context: serde_json::json!({}),
            conversation_history: vec!["hi".into()],
            source_peer: Uuid::new_v4(),
            created_at: 42,
            checksum: "abc123".into(),
        };
        let json = serde_json::to_string(&pkg).unwrap();
        let back: MigrationPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.created_at, 42);
        assert_eq!(back.checksum, "abc123");
    }
}
