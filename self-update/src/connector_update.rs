use crate::tuf::{TufClient, TufError, TufRepository};
use nexus_connectors_core::connector::HealthStatus;
use nexus_connectors_core::registry::ConnectorRegistry;
use nexus_marketplace::package::{
    verify_attestation, verify_package, MarketplaceError, SignedPackageBundle,
};
use std::collections::HashMap;
use uuid::Uuid;

pub trait ConnectorUpdateSource {
    fn download_for_connector(&self, connector_id: &str) -> Option<SignedPackageBundle>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryConnectorUpdateSource {
    packages_by_connector_id: HashMap<String, SignedPackageBundle>,
}

impl InMemoryConnectorUpdateSource {
    pub fn add_package(&mut self, connector_id: &str, package: SignedPackageBundle) {
        self.packages_by_connector_id
            .insert(connector_id.to_string(), package);
    }
}

impl ConnectorUpdateSource for InMemoryConnectorUpdateSource {
    fn download_for_connector(&self, connector_id: &str) -> Option<SignedPackageBundle> {
        self.packages_by_connector_id.get(connector_id).cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorUpdateError {
    NoConnectorFailures,
    DownloadFailed(String),
    Tuf(TufError),
    Marketplace(MarketplaceError),
}

impl std::fmt::Display for ConnectorUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectorUpdateError::NoConnectorFailures => {
                write!(f, "no connector failures detected")
            }
            ConnectorUpdateError::DownloadFailed(connector_id) => {
                write!(
                    f,
                    "failed to download update for connector '{connector_id}'"
                )
            }
            ConnectorUpdateError::Tuf(error) => write!(f, "{error}"),
            ConnectorUpdateError::Marketplace(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConnectorUpdateError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedAgent {
    pub agent_id: Uuid,
    pub connector_id: String,
    pub restarted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorUpdateOutcome {
    pub updated_connectors: Vec<String>,
    pub restarted_agents: Vec<Uuid>,
}

#[derive(Debug, Default)]
pub struct ConnectorAutoUpdater {
    connector_versions: HashMap<String, String>,
    affected_agents: Vec<AffectedAgent>,
}

impl ConnectorAutoUpdater {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_connector_version(&mut self, connector_id: &str, version: &str) {
        self.connector_versions
            .insert(connector_id.to_string(), version.to_string());
    }

    pub fn register_affected_agent(&mut self, agent_id: Uuid, connector_id: &str) {
        self.affected_agents.push(AffectedAgent {
            agent_id,
            connector_id: connector_id.to_string(),
            restarted: false,
        });
    }

    pub fn detect_api_change_failures(&self, registry: &ConnectorRegistry) -> Vec<String> {
        let mut failures = registry
            .health_check_all()
            .into_iter()
            .filter_map(|(connector_id, status)| match status {
                HealthStatus::Healthy => None,
                HealthStatus::Degraded(_) | HealthStatus::Unhealthy(_) => Some(connector_id),
            })
            .collect::<Vec<_>>();
        failures.sort();
        failures
    }

    pub fn update_failed_connectors(
        &mut self,
        failing_connectors: &[String],
        source: &dyn ConnectorUpdateSource,
        repository: &TufRepository,
        tuf_client: &mut TufClient,
    ) -> Result<ConnectorUpdateOutcome, ConnectorUpdateError> {
        if failing_connectors.is_empty() {
            return Err(ConnectorUpdateError::NoConnectorFailures);
        }

        let mut updated_connectors = Vec::new();
        let mut restarted_agents = Vec::new();

        for connector_id in failing_connectors {
            let package = source
                .download_for_connector(connector_id.as_str())
                .ok_or_else(|| ConnectorUpdateError::DownloadFailed(connector_id.clone()))?;
            self.verify_connector_update(connector_id.as_str(), &package, repository, tuf_client)?;

            self.connector_versions
                .insert(connector_id.clone(), package.metadata.version.clone());
            updated_connectors.push(connector_id.clone());

            for agent in &mut self.affected_agents {
                if agent.connector_id == *connector_id {
                    agent.restarted = true;
                    restarted_agents.push(agent.agent_id);
                }
            }
        }

        restarted_agents.sort();
        restarted_agents.dedup();
        updated_connectors.sort();
        updated_connectors.dedup();

        Ok(ConnectorUpdateOutcome {
            updated_connectors,
            restarted_agents,
        })
    }

    pub fn connector_version(&self, connector_id: &str) -> Option<&str> {
        self.connector_versions
            .get(connector_id)
            .map(|value| value.as_str())
    }

    pub fn affected_agents(&self) -> &[AffectedAgent] {
        &self.affected_agents
    }

    fn verify_connector_update(
        &self,
        connector_id: &str,
        package: &SignedPackageBundle,
        repository: &TufRepository,
        tuf_client: &mut TufClient,
    ) -> Result<(), ConnectorUpdateError> {
        let _ = tuf_client
            .verify_and_select_target(repository, connector_id)
            .map_err(ConnectorUpdateError::Tuf)?;
        verify_attestation(package).map_err(ConnectorUpdateError::Marketplace)?;
        verify_package(package).map_err(ConnectorUpdateError::Marketplace)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectorAutoUpdater, InMemoryConnectorUpdateSource};
    use crate::tuf::{
        RoleDefinition, RootMetadata, SnapshotMetadata, TargetDescription, TargetsMetadata,
        TimestampMetadata, TufClient, TufRepository, TufRole,
    };
    use nexus_connectors_core::connector::{Connector, HealthStatus, RetryPolicy};
    use nexus_connectors_core::registry::ConnectorRegistry;
    use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
    use nexus_kernel::errors::AgentError;
    use nexus_marketplace::package::{create_unsigned_bundle, sign_package, PackageMetadata};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MockFailingConnector;

    impl Connector for MockFailingConnector {
        fn id(&self) -> &str {
            "social.facebook"
        }
        fn name(&self) -> &str {
            "Mock Facebook Connector"
        }
        fn required_capabilities(&self) -> Vec<String> {
            vec!["social.post".to_string()]
        }
        fn health_check(&self) -> Result<HealthStatus, AgentError> {
            Ok(HealthStatus::Unhealthy("api schema changed".to_string()))
        }
        fn retry_policy(&self) -> RetryPolicy {
            RetryPolicy {
                max_retries: 1,
                backoff_ms: 100,
                backoff_multiplier: 2.0,
            }
        }
        fn degrade_gracefully(&self) -> bool {
            true
        }
    }

    fn root_metadata() -> RootMetadata {
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
            expires_unix: 10_000,
            roles,
        }
    }

    fn repo(connector_id: &str, version: &str) -> TufRepository {
        let mut targets = BTreeMap::new();
        targets.insert(
            connector_id.to_string(),
            TargetDescription {
                version: version.to_string(),
                length: 128,
                hashes: vec!["sha256:connector".to_string()],
            },
        );
        TufRepository {
            root: root_metadata(),
            targets: TargetsMetadata {
                version: 1,
                expires_unix: 10_000,
                targets,
            },
            snapshot: SnapshotMetadata {
                version: 1,
                expires_unix: 10_000,
                targets_version: 1,
            },
            timestamp: TimestampMetadata {
                version: 1,
                expires_unix: 10_000,
                snapshot_version: 1,
            },
        }
    }

    fn connector_package(
        connector_id: &str,
        version: &str,
    ) -> nexus_marketplace::package::SignedPackageBundle {
        let unsigned = create_unsigned_bundle(
            r#"name = "social-facebook"
version = "1.2.0"
capabilities = ["social.post"]
fuel_budget = 2000
"#,
            "fn connector_patch() {}",
            PackageMetadata {
                name: connector_id.to_string(),
                version: version.to_string(),
                description: "connector hotfix".to_string(),
                capabilities: vec!["social.post".to_string()],
                tags: vec!["connector".to_string()],
                author_id: "nexus-release".to_string(),
            },
            "https://github.com/nexus-os/connectors",
            "nexus-release",
        )
        .expect("unsigned connector package should build");
        sign_package(
            unsigned,
            &CryptoIdentity::from_bytes(SignatureAlgorithm::Ed25519, &[13_u8; 32]).unwrap(),
        )
        .expect("signed connector package should build")
    }

    #[test]
    fn test_connector_failure_detection_and_update() {
        let mut registry = ConnectorRegistry::new();
        let register = registry.register(Arc::new(MockFailingConnector));
        assert!(register.is_ok());

        let mut updater = ConnectorAutoUpdater::new();
        updater.set_connector_version("social.facebook", "1.1.0");
        let affected_agent = Uuid::new_v4();
        updater.register_affected_agent(affected_agent, "social.facebook");

        let failures = updater.detect_api_change_failures(&registry);
        assert_eq!(failures, vec!["social.facebook".to_string()]);

        let package = connector_package("social.facebook", "1.2.0");
        let mut source = InMemoryConnectorUpdateSource::default();
        source.add_package("social.facebook", package);

        let mut tuf_client = TufClient::with_clock(root_metadata(), 0, 0, 0, Arc::new(|| 100_u64));
        let repository = repo("social.facebook", "1.2.0");

        let outcome = updater
            .update_failed_connectors(&failures, &source, &repository, &mut tuf_client)
            .expect("connector update should succeed");

        assert_eq!(outcome.restarted_agents, vec![affected_agent]);
        assert_eq!(updater.connector_version("social.facebook"), Some("1.2.0"));
        assert!(updater
            .affected_agents()
            .iter()
            .all(|agent| agent.restarted));
    }
}
