use crate::tuf::{parse_semver, TufClient, TufError, TufRepository};
use nexus_marketplace::scanner::policy_lint;
use nexus_marketplace::trust::CapabilityRisk;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorError {
    DependencyNotTracked(String),
    InvalidVersion(String),
    ApprovalRequired(String),
    SafetyGateFailed(String),
    Tuf(TufError),
}

impl std::fmt::Display for MonitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorError::DependencyNotTracked(name) => {
                write!(f, "dependency '{name}' is not tracked")
            }
            MonitorError::InvalidVersion(version) => {
                write!(f, "version '{version}' is not valid semver")
            }
            MonitorError::ApprovalRequired(name) => {
                write!(f, "major update for '{name}' requires manual approval")
            }
            MonitorError::SafetyGateFailed(name) => {
                write!(f, "safety gate failed for '{name}'")
            }
            MonitorError::Tuf(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for MonitorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    UpToDate,
    AutoInstalled(String),
    ApprovalRequired(String),
    NotDue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedDependency {
    current_version: String,
    last_checked_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMajorUpdate {
    candidate_version: String,
}

pub struct DependencyMonitor {
    tracked: HashMap<String, TrackedDependency>,
    pending_major: HashMap<String, PendingMajorUpdate>,
    check_interval_seconds: u64,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl DependencyMonitor {
    pub fn new(check_interval_seconds: u64) -> Self {
        Self::with_clock(check_interval_seconds, Arc::new(|| 0_u64))
    }

    pub fn with_clock(
        check_interval_seconds: u64,
        clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            tracked: HashMap::new(),
            pending_major: HashMap::new(),
            check_interval_seconds,
            clock,
        }
    }

    pub fn track_dependency(&mut self, name: &str, current_version: &str) {
        self.tracked.insert(
            name.to_string(),
            TrackedDependency {
                current_version: current_version.to_string(),
                last_checked_unix: 0,
            },
        );
    }

    pub fn current_version(&self, name: &str) -> Option<&str> {
        self.tracked.get(name).map(|d| d.current_version.as_str())
    }

    pub fn check_for_update(
        &mut self,
        name: &str,
        candidate_version: &str,
        repository: &TufRepository,
        tuf_client: &mut TufClient,
    ) -> Result<UpdateOutcome, MonitorError> {
        let now = (self.clock)();
        let tracked = self
            .tracked
            .get_mut(name)
            .ok_or_else(|| MonitorError::DependencyNotTracked(name.to_string()))?;

        let due = now
            .saturating_sub(tracked.last_checked_unix)
            >= self.check_interval_seconds;
        if !due {
            return Ok(UpdateOutcome::NotDue);
        }
        tracked.last_checked_unix = now;

        let current = parse_semver(tracked.current_version.as_str())
            .map_err(|_| MonitorError::InvalidVersion(tracked.current_version.clone()))?;
        let candidate = parse_semver(candidate_version)
            .map_err(|_| MonitorError::InvalidVersion(candidate_version.to_string()))?;

        if candidate <= current {
            return Ok(UpdateOutcome::UpToDate);
        }

        let _verified = tuf_client
            .verify_and_select_target(repository, name)
            .map_err(MonitorError::Tuf)?;

        let declared_capabilities = vec!["llm.query".to_string(), "web.search".to_string()];
        let lint = policy_lint(declared_capabilities.as_slice());
        if lint.risk == CapabilityRisk::High {
            return Err(MonitorError::SafetyGateFailed(name.to_string()));
        }

        if candidate.0 > current.0 {
            self.pending_major.insert(
                name.to_string(),
                PendingMajorUpdate {
                    candidate_version: candidate_version.to_string(),
                },
            );
            return Ok(UpdateOutcome::ApprovalRequired(candidate_version.to_string()));
        }

        tracked.current_version = candidate_version.to_string();
        Ok(UpdateOutcome::AutoInstalled(candidate_version.to_string()))
    }

    pub fn approve_major_update(&mut self, name: &str) -> Result<String, MonitorError> {
        let pending = self
            .pending_major
            .remove(name)
            .ok_or_else(|| MonitorError::ApprovalRequired(name.to_string()))?;
        let tracked = self
            .tracked
            .get_mut(name)
            .ok_or_else(|| MonitorError::DependencyNotTracked(name.to_string()))?;
        tracked.current_version = pending.candidate_version.clone();
        Ok(pending.candidate_version)
    }
}

#[cfg(test)]
mod tests {
    use super::{DependencyMonitor, UpdateOutcome};
    use crate::tuf::{
        RoleDefinition, RootMetadata, SnapshotMetadata, TargetDescription, TargetsMetadata,
        TimestampMetadata, TufClient, TufRepository, TufRole,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn root() -> RootMetadata {
        let mut roles = BTreeMap::new();
        roles.insert(
            TufRole::Root,
            RoleDefinition {
                key_ids: vec!["root-key".to_string()],
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
            version: 1,
            expires_unix: 100_000,
            roles,
        }
    }

    fn repository_for(dependency_name: &str, version: &str) -> TufRepository {
        let mut targets = BTreeMap::new();
        targets.insert(
            dependency_name.to_string(),
            TargetDescription {
                version: version.to_string(),
                length: 7,
                hashes: vec!["sha256:123".to_string()],
            },
        );

        TufRepository {
            root: root(),
            targets: TargetsMetadata {
                version: 2,
                expires_unix: 100_000,
                targets,
            },
            snapshot: SnapshotMetadata {
                version: 2,
                expires_unix: 100_000,
                targets_version: 2,
            },
            timestamp: TimestampMetadata {
                version: 2,
                expires_unix: 100_000,
                snapshot_version: 2,
            },
        }
    }

    #[test]
    fn test_auto_update_minor() {
        let mut monitor = DependencyMonitor::with_clock(60, Arc::new(|| 120));
        monitor.track_dependency("nexus-marketplace", "1.0.0");

        let mut tuf = TufClient::with_clock(root(), 0, 0, 0, Arc::new(|| 120));
        let repository = repository_for("nexus-marketplace", "1.1.0");

        let result = monitor
            .check_for_update("nexus-marketplace", "1.1.0", &repository, &mut tuf)
            .expect("minor update should verify and install");

        assert_eq!(result, UpdateOutcome::AutoInstalled("1.1.0".to_string()));
        assert_eq!(monitor.current_version("nexus-marketplace"), Some("1.1.0"));
    }
}
