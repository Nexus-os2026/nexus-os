use crate::tuf::{TufClient, TufError, TufRepository};
use nexus_marketplace::package::{
    verify_attestation, verify_package, MarketplaceError, SignedPackageBundle,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    DownloadFailed(String),
    NoAgentsAvailable,
    CanaryFailed(String),
    RollbackFailed(String),
    Tuf(TufError),
    Marketplace(MarketplaceError),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::DownloadFailed(package_id) => {
                write!(f, "failed to download update package '{package_id}'")
            }
            PipelineError::NoAgentsAvailable => write!(f, "no agents available for canary update"),
            PipelineError::CanaryFailed(reason) => write!(f, "canary deployment failed: {reason}"),
            PipelineError::RollbackFailed(reason) => write!(f, "rollback failed: {reason}"),
            PipelineError::Tuf(error) => write!(f, "{error}"),
            PipelineError::Marketplace(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PipelineError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineOutcome {
    pub package_id: String,
    pub target_version: String,
    pub canary_agent_id: Uuid,
    pub applied_agent_ids: Vec<Uuid>,
    pub rolled_back: bool,
}

pub trait UpdateSource {
    fn download(&self, package_id: &str) -> Option<SignedPackageBundle>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryUpdateSource {
    packages: HashMap<String, SignedPackageBundle>,
}

impl InMemoryUpdateSource {
    pub fn add_package(&mut self, package: SignedPackageBundle) {
        self.packages.insert(package.package_id.clone(), package);
    }
}

impl UpdateSource for InMemoryUpdateSource {
    fn download(&self, package_id: &str) -> Option<SignedPackageBundle> {
        self.packages.get(package_id).cloned()
    }
}

#[derive(Debug, Clone)]
pub struct ManagedAgent {
    pub agent_id: Uuid,
    version_history: Vec<String>,
    fail_on_versions: HashSet<String>,
}

impl ManagedAgent {
    pub fn new(agent_id: Uuid, version: &str) -> Self {
        Self {
            agent_id,
            version_history: vec![version.to_string()],
            fail_on_versions: HashSet::new(),
        }
    }

    pub fn with_failure_version(agent_id: Uuid, version: &str, fail_version: &str) -> Self {
        let mut fail_on_versions = HashSet::new();
        let _ = fail_on_versions.insert(fail_version.to_string());
        Self {
            agent_id,
            version_history: vec![version.to_string()],
            fail_on_versions,
        }
    }

    pub fn current_version(&self) -> &str {
        self.version_history
            .last()
            .map(String::as_str)
            .unwrap_or_default()
    }

    fn apply_update(&mut self, version: &str) -> Result<(), String> {
        if self.fail_on_versions.contains(version) {
            return Err(format!(
                "agent {} reported runtime errors on {}",
                self.agent_id, version
            ));
        }
        if self.current_version() == version {
            return Ok(());
        }
        self.version_history.push(version.to_string());
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        if self.version_history.len() <= 1 {
            return Ok(());
        }
        self.version_history.pop();
        Ok(())
    }
}

pub struct UpdatePipeline {
    agents: Vec<ManagedAgent>,
}

impl UpdatePipeline {
    pub fn new(agents: Vec<ManagedAgent>) -> Self {
        Self { agents }
    }

    pub fn agents(&self) -> &[ManagedAgent] {
        &self.agents
    }

    pub fn apply_signed_update(
        &mut self,
        source: &dyn UpdateSource,
        package_id: &str,
        target_name: &str,
        repository: &TufRepository,
        tuf_client: &mut TufClient,
    ) -> Result<PipelineOutcome, PipelineError> {
        if self.agents.is_empty() {
            return Err(PipelineError::NoAgentsAvailable);
        }

        let package = source
            .download(package_id)
            .ok_or_else(|| PipelineError::DownloadFailed(package_id.to_string()))?;

        let _ = tuf_client
            .verify_and_select_target(repository, target_name)
            .map_err(PipelineError::Tuf)?;

        verify_attestation(&package).map_err(PipelineError::Marketplace)?;
        verify_package(&package).map_err(PipelineError::Marketplace)?;

        let target_version = package.metadata.version.clone();
        let canary_agent_id = self.agents[0].agent_id;

        self.agents[0]
            .apply_update(target_version.as_str())
            .map_err(PipelineError::CanaryFailed)?;
        let mut applied_indices = vec![0_usize];

        for index in 1..self.agents.len() {
            if let Err(error) = self.agents[index].apply_update(target_version.as_str()) {
                self.rollback_agents(applied_indices.as_slice())?;
                return Err(PipelineError::CanaryFailed(error));
            }
            applied_indices.push(index);
        }

        let applied_agent_ids = applied_indices
            .iter()
            .map(|index| self.agents[*index].agent_id)
            .collect::<Vec<_>>();
        Ok(PipelineOutcome {
            package_id: package_id.to_string(),
            target_version,
            canary_agent_id,
            applied_agent_ids,
            rolled_back: false,
        })
    }

    fn rollback_agents(&mut self, indices: &[usize]) -> Result<(), PipelineError> {
        for index in indices.iter().rev() {
            self.agents[*index]
                .rollback()
                .map_err(PipelineError::RollbackFailed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryUpdateSource, ManagedAgent, PipelineError, UpdatePipeline};
    use crate::tuf::{
        RoleDefinition, RootMetadata, SnapshotMetadata, TargetDescription, TargetsMetadata,
        TimestampMetadata, TufClient, TufRepository, TufRole,
    };
    use ed25519_dalek::SigningKey;
    use nexus_marketplace::package::{create_unsigned_bundle, sign_package, PackageMetadata};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use uuid::Uuid;

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
            expires_unix: 100_000,
            roles,
        }
    }

    fn tuf_repo(target_name: &str, version: &str) -> TufRepository {
        let mut targets = BTreeMap::new();
        targets.insert(
            target_name.to_string(),
            TargetDescription {
                version: version.to_string(),
                length: 1024,
                hashes: vec!["sha256:abc".to_string()],
            },
        );

        TufRepository {
            root: root_metadata(),
            targets: TargetsMetadata {
                version: 1,
                expires_unix: 100_000,
                targets,
            },
            snapshot: SnapshotMetadata {
                version: 1,
                expires_unix: 100_000,
                targets_version: 1,
            },
            timestamp: TimestampMetadata {
                version: 1,
                expires_unix: 100_000,
                snapshot_version: 1,
            },
        }
    }

    fn signed_package(version: &str) -> nexus_marketplace::package::SignedPackageBundle {
        let metadata = PackageMetadata {
            name: "nexus-agent-runtime".to_string(),
            version: version.to_string(),
            description: "Secure runtime update".to_string(),
            capabilities: vec!["llm.query".to_string()],
            tags: vec!["runtime".to_string()],
            author_id: "release-bot".to_string(),
        };
        let unsigned = create_unsigned_bundle(
            r#"name = "nexus-agent-runtime"
version = "1.1.0"
capabilities = ["llm.query"]
fuel_budget = 5000
"#,
            "fn run() { /* updated runtime */ }",
            metadata,
            "https://github.com/nexus-os/runtime",
            "nexus-release",
        )
        .expect("unsigned bundle should be created");
        sign_package(unsigned, &SigningKey::from_bytes(&[11_u8; 32]))
            .expect("bundle should be signed")
    }

    #[test]
    fn test_canary_deploy_success() {
        let agents = vec![
            ManagedAgent::new(Uuid::new_v4(), "1.0.0"),
            ManagedAgent::new(Uuid::new_v4(), "1.0.0"),
            ManagedAgent::new(Uuid::new_v4(), "1.0.0"),
        ];
        let mut pipeline = UpdatePipeline::new(agents);
        let package = signed_package("1.1.0");
        let package_id = package.package_id.clone();
        let mut source = InMemoryUpdateSource::default();
        source.add_package(package);

        let mut tuf_client =
            TufClient::with_clock(root_metadata(), 0, 0, 0, Arc::new(|| 100_u64));
        let repo = tuf_repo("nexus-agent-runtime", "1.1.0");

        let outcome = pipeline
            .apply_signed_update(
                &source,
                package_id.as_str(),
                "nexus-agent-runtime",
                &repo,
                &mut tuf_client,
            )
            .expect("canary update should succeed");

        assert_eq!(outcome.applied_agent_ids.len(), 3);
        for agent in pipeline.agents() {
            assert_eq!(agent.current_version(), "1.1.0");
        }
    }

    #[test]
    fn test_canary_deploy_failure_rollback() {
        let agents = vec![
            ManagedAgent::with_failure_version(Uuid::new_v4(), "1.0.0", "1.1.0"),
            ManagedAgent::new(Uuid::new_v4(), "1.0.0"),
            ManagedAgent::new(Uuid::new_v4(), "1.0.0"),
        ];
        let mut pipeline = UpdatePipeline::new(agents);
        let package = signed_package("1.1.0");
        let package_id = package.package_id.clone();
        let mut source = InMemoryUpdateSource::default();
        source.add_package(package);

        let mut tuf_client =
            TufClient::with_clock(root_metadata(), 0, 0, 0, Arc::new(|| 100_u64));
        let repo = tuf_repo("nexus-agent-runtime", "1.1.0");

        let result = pipeline.apply_signed_update(
            &source,
            package_id.as_str(),
            "nexus-agent-runtime",
            &repo,
            &mut tuf_client,
        );
        assert!(matches!(result, Err(PipelineError::CanaryFailed(_))));

        for agent in pipeline.agents() {
            assert_eq!(agent.current_version(), "1.0.0");
        }
    }
}
