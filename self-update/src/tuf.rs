use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TufRole {
    Root,
    Targets,
    Snapshot,
    Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub key_ids: Vec<String>,
    pub threshold: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootMetadata {
    pub version: u64,
    pub expires_unix: u64,
    pub roles: BTreeMap<TufRole, RoleDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetDescription {
    pub version: String,
    pub length: u64,
    pub hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetsMetadata {
    pub version: u64,
    pub expires_unix: u64,
    pub targets: BTreeMap<String, TargetDescription>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub version: u64,
    pub expires_unix: u64,
    pub targets_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimestampMetadata {
    pub version: u64,
    pub expires_unix: u64,
    pub snapshot_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TufRepository {
    pub root: RootMetadata,
    pub targets: TargetsMetadata,
    pub snapshot: SnapshotMetadata,
    pub timestamp: TimestampMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TufError {
    RollbackDetected,
    FreezeAttackDetected,
    KeyRotationInvalid,
    MetadataInconsistent,
    TargetNotFound(String),
    VersionParseError(String),
}

impl std::fmt::Display for TufError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TufError::RollbackDetected => write!(f, "rollback detected"),
            TufError::FreezeAttackDetected => write!(f, "freeze attack detected"),
            TufError::KeyRotationInvalid => write!(f, "root key rotation invalid"),
            TufError::MetadataInconsistent => write!(f, "metadata is inconsistent"),
            TufError::TargetNotFound(target) => write!(f, "target '{target}' not found"),
            TufError::VersionParseError(version) => {
                write!(f, "version string '{version}' is not semver-like")
            }
        }
    }
}

impl std::error::Error for TufError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTarget {
    pub name: String,
    pub version: String,
    pub length: u64,
    pub hashes: Vec<String>,
}

pub struct TufClient {
    trusted_root: RootMetadata,
    trusted_targets_version: u64,
    trusted_snapshot_version: u64,
    trusted_timestamp_version: u64,
    installed_targets: HashMap<String, String>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl TufClient {
    pub fn new(
        root: RootMetadata,
        initial_targets_version: u64,
        initial_snapshot_version: u64,
        initial_timestamp_version: u64,
    ) -> Self {
        Self::with_clock(
            root,
            initial_targets_version,
            initial_snapshot_version,
            initial_timestamp_version,
            Arc::new(|| 0_u64),
        )
    }

    pub fn with_clock(
        root: RootMetadata,
        initial_targets_version: u64,
        initial_snapshot_version: u64,
        initial_timestamp_version: u64,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            trusted_root: root,
            trusted_targets_version: initial_targets_version,
            trusted_snapshot_version: initial_snapshot_version,
            trusted_timestamp_version: initial_timestamp_version,
            installed_targets: HashMap::new(),
            clock,
        }
    }

    pub fn verify_and_select_target(
        &mut self,
        repository: &TufRepository,
        target_name: &str,
    ) -> Result<VerifiedTarget, TufError> {
        let now = (self.clock)();
        self.ensure_not_expired(repository, now)?;
        self.update_root_if_needed(&repository.root)?;
        self.enforce_monotonic_versions(repository)?;
        self.enforce_metadata_linking(repository)?;

        let target = repository
            .targets
            .targets
            .get(target_name)
            .ok_or_else(|| TufError::TargetNotFound(target_name.to_string()))?;

        if let Some(installed) = self.installed_targets.get(target_name) {
            let installed_v = parse_semver(installed)?;
            let candidate_v = parse_semver(target.version.as_str())?;
            if candidate_v < installed_v {
                return Err(TufError::RollbackDetected);
            }
        }

        self.trusted_timestamp_version = repository.timestamp.version;
        self.trusted_snapshot_version = repository.snapshot.version;
        self.trusted_targets_version = repository.targets.version;
        self.installed_targets
            .insert(target_name.to_string(), target.version.clone());

        Ok(VerifiedTarget {
            name: target_name.to_string(),
            version: target.version.clone(),
            length: target.length,
            hashes: target.hashes.clone(),
        })
    }

    pub fn installed_version(&self, target_name: &str) -> Option<&str> {
        self.installed_targets.get(target_name).map(|s| s.as_str())
    }

    pub fn rotate_root_keys(&mut self, new_root: RootMetadata) -> Result<(), TufError> {
        self.update_root_if_needed(&new_root)
    }

    fn ensure_not_expired(&self, repository: &TufRepository, now: u64) -> Result<(), TufError> {
        let metadata_expired = repository.root.expires_unix < now
            || repository.targets.expires_unix < now
            || repository.snapshot.expires_unix < now
            || repository.timestamp.expires_unix < now;

        if metadata_expired {
            return Err(TufError::FreezeAttackDetected);
        }

        Ok(())
    }

    fn update_root_if_needed(&mut self, candidate: &RootMetadata) -> Result<(), TufError> {
        if candidate.version < self.trusted_root.version {
            return Err(TufError::RollbackDetected);
        }
        if candidate.version == self.trusted_root.version {
            return Ok(());
        }

        let old_root_keys = self.role_keys(&self.trusted_root, &TufRole::Root);
        let new_root_keys = self.role_keys(candidate, &TufRole::Root);
        if old_root_keys.is_empty() || new_root_keys.is_empty() {
            return Err(TufError::KeyRotationInvalid);
        }

        let intersection = old_root_keys
            .intersection(&new_root_keys)
            .collect::<Vec<_>>();
        if intersection.is_empty() {
            return Err(TufError::KeyRotationInvalid);
        }

        self.trusted_root = candidate.clone();
        Ok(())
    }

    fn role_keys(&self, root: &RootMetadata, role: &TufRole) -> BTreeSet<String> {
        root.roles
            .get(role)
            .map(|def| def.key_ids.iter().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_default()
    }

    fn enforce_monotonic_versions(&self, repository: &TufRepository) -> Result<(), TufError> {
        if repository.timestamp.version < self.trusted_timestamp_version
            || repository.snapshot.version < self.trusted_snapshot_version
            || repository.targets.version < self.trusted_targets_version
        {
            return Err(TufError::RollbackDetected);
        }

        Ok(())
    }

    fn enforce_metadata_linking(&self, repository: &TufRepository) -> Result<(), TufError> {
        if repository.timestamp.snapshot_version != repository.snapshot.version {
            return Err(TufError::MetadataInconsistent);
        }
        if repository.snapshot.targets_version != repository.targets.version {
            return Err(TufError::MetadataInconsistent);
        }
        Ok(())
    }
}

pub fn parse_semver(input: &str) -> Result<(u64, u64, u64), TufError> {
    let normalized = input.trim().trim_start_matches('v');
    let mut parts = normalized.split('.');
    let major = parts
        .next()
        .ok_or_else(|| TufError::VersionParseError(input.to_string()))?
        .parse::<u64>()
        .map_err(|_| TufError::VersionParseError(input.to_string()))?;
    let minor = parts
        .next()
        .ok_or_else(|| TufError::VersionParseError(input.to_string()))?
        .parse::<u64>()
        .map_err(|_| TufError::VersionParseError(input.to_string()))?;
    let patch = parts
        .next()
        .ok_or_else(|| TufError::VersionParseError(input.to_string()))?
        .parse::<u64>()
        .map_err(|_| TufError::VersionParseError(input.to_string()))?;

    Ok((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::{
        RoleDefinition, RootMetadata, SnapshotMetadata, TargetDescription, TargetsMetadata,
        TimestampMetadata, TufClient, TufError, TufRepository, TufRole,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn baseline_root(version: u64, expires_unix: u64, root_key_id: &str) -> RootMetadata {
        let mut roles = BTreeMap::new();
        roles.insert(
            TufRole::Root,
            RoleDefinition {
                key_ids: vec![root_key_id.to_string()],
                threshold: 1,
            },
        );
        roles.insert(
            TufRole::Targets,
            RoleDefinition {
                key_ids: vec!["targets-key".to_string()],
                threshold: 1,
            },
        );
        roles.insert(
            TufRole::Snapshot,
            RoleDefinition {
                key_ids: vec!["snapshot-key".to_string()],
                threshold: 1,
            },
        );
        roles.insert(
            TufRole::Timestamp,
            RoleDefinition {
                key_ids: vec!["timestamp-key".to_string()],
                threshold: 1,
            },
        );
        RootMetadata {
            version,
            expires_unix,
            roles,
        }
    }

    fn repository_for_version(
        metadata_version: u64,
        dependency_name: &str,
        dependency_version: &str,
    ) -> TufRepository {
        let root = baseline_root(metadata_version.max(1), 10_000, "root-key-a");
        let mut targets = BTreeMap::new();
        targets.insert(
            dependency_name.to_string(),
            TargetDescription {
                version: dependency_version.to_string(),
                length: 123,
                hashes: vec!["sha256:abc".to_string()],
            },
        );
        TufRepository {
            root,
            targets: TargetsMetadata {
                version: metadata_version,
                expires_unix: 10_000,
                targets,
            },
            snapshot: SnapshotMetadata {
                version: metadata_version,
                expires_unix: 10_000,
                targets_version: metadata_version,
            },
            timestamp: TimestampMetadata {
                version: metadata_version,
                expires_unix: 10_000,
                snapshot_version: metadata_version,
            },
        }
    }

    #[test]
    fn test_tuf_rollback_protection() {
        let root = baseline_root(1, 10_000, "root-key-a");
        let mut client = TufClient::with_clock(root, 0, 0, 0, Arc::new(|| 100));

        let newer = repository_for_version(1, "nexus-connectors-web", "1.2.0");
        let first = client.verify_and_select_target(&newer, "nexus-connectors-web");
        assert!(first.is_ok());

        let older = repository_for_version(2, "nexus-connectors-web", "1.1.0");
        let result = client.verify_and_select_target(&older, "nexus-connectors-web");
        assert_eq!(result, Err(TufError::RollbackDetected));
    }
}
